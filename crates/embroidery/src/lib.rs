//! Embroidery: turns a document's stitched objects into a machine stitch plan, and reads and
//! writes the DST format embroidery machines take.
//!
//! An object is stitched when it carries [`StitchSettings`] ([`vectorcraft_doc::Node::stitch`]).
//! [`plan`] walks the visible art in paint order (bottom first) and generates each object's
//! stitches (see [`generate`]), joining objects with jumps, trims and colour changes. A
//! [`Plan`] is a list of needle moves in millimetres, document coordinates (y down).
//!
//! - `generate`: running, bean, satin, fill and manual stitches from geometry.
//! - `dst`: the Tajima DST format (writer and reader).
//! - `import`: a plan as editable art (one manual-stitch path per run of stitches).

mod dst;
pub mod generate;
mod import;

use std::sync::Arc;

use kurbo::Point;
use serde::Serialize;
use vectorcraft_color::Paint;
use vectorcraft_doc::{Document, Node, NodeKind, StitchKind, StitchSettings, Underlay};
use vectorcraft_geom::{FillRule, PathData};

pub use dst::{DstError, is_dst, read_dst, write_dst};
pub use import::to_document;

/// Millimetres per point.
pub const PT_MM: f64 = 25.4 / 72.0;
/// The most needle moves a plan holds: past it the rest of the design is left out (with a
/// warning). Real designs have well under a million.
pub const MAX_NEEDLES: usize = 2_000_000;
/// Moves to the next stitch shorter than this are sewn; longer ones jump, mm.
pub const JUMP_MIN: f64 = 1.0;
/// Jumps longer than this cut the thread first, mm.
pub const TRIM_MIN: f64 = 3.0;

/// What the needle does at a [`Needle`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Cmd {
    /// Sew to the point.
    Stitch,
    /// Move to the point without sewing.
    Jump,
    /// Cut the thread here.
    Trim,
    /// Stop for the next thread (the next of [`Plan::threads`]).
    ColorChange,
    /// The end of the design.
    End,
}

/// One needle move, mm.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Needle {
    pub x: f64,
    pub y: f64,
    pub cmd: Cmd,
}

/// A stitch plan: needle moves in sewing order, and the thread of each colour block.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Plan {
    /// One thread per colour block: the first block sews `threads[0]`, each
    /// [`Cmd::ColorChange`] moves on to the next.
    pub threads: Vec<[u8; 3]>,
    pub needles: Vec<Needle>,
    /// Problems with the art (objects that can't be stitched, a design cut short).
    #[serde(skip)]
    pub warnings: Vec<String>,
}

/// A plan's counts and size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub stitches: usize,
    pub jumps: usize,
    pub trims: usize,
    pub color_changes: usize,
    /// Bounds of the stitches, mm: `[minX, minY, maxX, maxY]`.
    pub bounds: Option<[f64; 4]>,
}

impl Plan {
    pub fn stats(&self) -> Stats {
        let mut s = Stats::default();
        for n in &self.needles {
            match n.cmd {
                Cmd::Stitch => {
                    s.stitches += 1;
                    let b = s.bounds.get_or_insert([n.x, n.y, n.x, n.y]);
                    *b = [b[0].min(n.x), b[1].min(n.y), b[2].max(n.x), b[3].max(n.y)];
                }
                Cmd::Jump => s.jumps += 1,
                Cmd::Trim => s.trims += 1,
                Cmd::ColorChange => s.color_changes += 1,
                Cmd::End => {}
            }
        }
        s
    }

    /// Each run of stitches (what the needle sews between jumps, trims and colour changes), with
    /// the index of its thread. A run starts at the point the needle sews from.
    pub fn runs(&self) -> Vec<(usize, Vec<Point>)> {
        let mut out: Vec<(usize, Vec<Point>)> = vec![];
        let mut thread = 0;
        let mut cur: Option<Point> = None;
        let mut run: Vec<Point> = vec![];
        let mut flush = |run: &mut Vec<Point>, thread: usize| {
            if run.len() > 1 {
                out.push((thread, std::mem::take(run)));
            }
            run.clear();
        };
        for n in &self.needles {
            let p = Point::new(n.x, n.y);
            match n.cmd {
                Cmd::Stitch => {
                    if run.is_empty() {
                        run.push(cur.unwrap_or(p));
                    }
                    run.push(p);
                    cur = Some(p);
                }
                Cmd::Jump => {
                    flush(&mut run, thread);
                    cur = Some(p);
                }
                Cmd::Trim => flush(&mut run, thread),
                Cmd::ColorChange => {
                    flush(&mut run, thread);
                    thread += 1;
                }
                Cmd::End => flush(&mut run, thread),
            }
        }
        flush(&mut run, thread);
        out
    }
}

/// Builds a [`Plan`] run by run.
struct Builder {
    plan: Plan,
    cur: Option<Point>,
    thread: Option<[u8; 3]>,
    full: bool,
}

impl Builder {
    fn new() -> Self {
        Self { plan: Plan::default(), cur: None, thread: None, full: false }
    }

    fn push(&mut self, p: Point, cmd: Cmd) -> bool {
        if self.plan.needles.len() >= MAX_NEEDLES {
            if !self.full {
                self.full = true;
                self.plan.warnings.push(format!("the design has more than {MAX_NEEDLES} needle moves: the rest is left out"));
            }
            return false;
        }
        self.plan.needles.push(Needle { x: p.x, y: p.y, cmd });
        true
    }

    /// Sew `pts` in `thread`: change thread if needed, travel to the first point (a stitch when
    /// close, else a jump, trimming first when far), then stitch the rest.
    fn run(&mut self, thread: [u8; 3], pts: &[Point]) {
        let Some((first, rest)) = pts.split_first() else { return };
        if self.full {
            return;
        }
        if self.thread != Some(thread) {
            if self.thread.is_some() {
                let at = self.cur.unwrap_or(*first);
                self.push(at, Cmd::Trim);
                self.push(at, Cmd::ColorChange);
            }
            self.plan.threads.push(thread);
            self.thread = Some(thread);
        }
        match self.cur {
            Some(c) if c.distance(*first) <= JUMP_MIN => {
                if c.distance(*first) > 1e-9 {
                    self.push(*first, Cmd::Stitch);
                }
            }
            Some(c) => {
                if c.distance(*first) > TRIM_MIN && self.plan.needles.last().is_some_and(|n| n.cmd == Cmd::Stitch) {
                    self.push(c, Cmd::Trim);
                }
                self.push(*first, Cmd::Jump);
            }
            None => {
                self.push(*first, Cmd::Jump);
            }
        }
        for p in rest {
            if !self.push(*p, Cmd::Stitch) {
                break;
            }
        }
        self.cur = Some(pts.last().copied().unwrap_or(*first));
    }

    fn finish(mut self) -> Plan {
        if let Some(c) = self.cur {
            if self.plan.needles.last().is_some_and(|n| n.cmd == Cmd::Stitch) {
                self.push(c, Cmd::Trim);
            }
            self.push(c, Cmd::End);
        }
        self.plan
    }
}

/// The stitch plan of `doc`'s visible stitched objects, in paint order.
pub fn plan(doc: &Document) -> Plan {
    let mut b = Builder::new();
    let mut skipped = 0;
    for l in &doc.layers {
        walk(l, &mut b, &mut skipped);
    }
    let mut plan = b.finish();
    if skipped > 0 {
        let what = if skipped == 1 { "object" } else { "objects" };
        plan.warnings.push(format!("{skipped} stitched {what} can't be stitched yet (only paths and compound paths are): left out"));
    }
    plan
}

/// Whether any object in `doc` carries stitch settings.
pub fn has_stitches(doc: &Document) -> bool {
    let mut any = false;
    doc.walk(|n| any |= n.stitch.is_some());
    any
}

fn walk(n: &Arc<Node>, b: &mut Builder, skipped: &mut usize) {
    if !n.visible {
        return;
    }
    if let Some(s) = n.stitch.as_deref() {
        match &n.kind {
            NodeKind::Path { path, rule, guide: false, clipping: false, .. } => object(n, s, &[path], *rule, b),
            NodeKind::Compound { children, rule } => {
                let paths: Vec<&PathData> = children.iter().filter_map(|c| c.path_data()).collect();
                object(n, s, &paths, *rule, b);
            }
            NodeKind::Path { .. } => {}
            _ => *skipped += 1,
        }
    }
    if let Some(children) = n.children() {
        for c in children {
            walk(c, b, skipped);
        }
    }
}

/// The sRGB of a solid paint.
fn solid(p: &Paint) -> Option<[u8; 3]> {
    match p {
        Paint::Solid { color, .. } => {
            let [r, g, b, _] = color.to_rgba8(1.0);
            Some([r, g, b])
        }
        _ => None,
    }
}

/// The thread `n` sews with.
fn thread_of(n: &Node, s: &StitchSettings) -> [u8; 3] {
    let fill = || solid(&n.appearance.fill_paint());
    let stroke = || solid(&n.appearance.stroke_paint());
    let paint = if s.kind.follows_path() { stroke().or_else(fill) } else { fill().or_else(stroke) };
    s.thread.or(paint).unwrap_or([0, 0, 0])
}

fn object(n: &Node, s: &StitchSettings, paths: &[&PathData], rule: FillRule, b: &mut Builder) {
    let thread = thread_of(n, s);
    let mut runs: Vec<Vec<Point>> = vec![];
    if s.kind == StitchKind::Manual {
        runs.extend(paths.iter().flat_map(|p| generate::manual(p)));
    } else {
        let lines: Vec<(Vec<Point>, bool)> = paths.iter().flat_map(|p| generate::polylines(p)).collect();
        match s.kind {
            StitchKind::Fill => {
                let polys: Vec<Vec<Point>> = lines.into_iter().map(|(l, _)| l).collect();
                let under = match s.underlay() {
                    Underlay::Fill => generate::fill_underlay(&polys, rule, s.angle()),
                    _ => Ok(vec![]),
                };
                match under.and_then(|u| Ok((u, generate::fill(&polys, rule, s.angle(), s.spacing(), s.length(), s.stagger())?))) {
                    Ok((u, top)) => runs.extend(u.into_iter().chain(top)),
                    Err(e) => {
                        b.plan.warnings.push(e);
                        return;
                    }
                }
            }
            StitchKind::Satin => {
                let w = s.width(n.appearance.stroke_width() * PT_MM) + s.pull();
                for (l, closed) in &lines {
                    let (under, back) = generate::satin_underlay(l, *closed, w, s.underlay());
                    // After an underlay that ends at the far end, the column is sewn back.
                    let top = if back {
                        let rev: Vec<Point> = l.iter().rev().copied().collect();
                        generate::satin(&rev, *closed, w, s.spacing())
                    } else {
                        generate::satin(l, *closed, w, s.spacing())
                    };
                    // One run: the move from the underlay to the column stays inside the column.
                    runs.push(under.into_iter().chain(top).collect());
                }
            }
            StitchKind::Bean => runs.extend(lines.iter().map(|(l, _)| generate::bean(&generate::running(l, s.length()), s.passes()))),
            _ => runs.extend(lines.iter().map(|(l, _)| generate::running(l, s.length()))),
        }
    }
    for r in runs {
        b.run(thread, &r);
    }
}

/// A distinct thread colour for the `i`th colour block of a design whose file holds no colours
/// (DST): hues a golden angle apart.
pub fn palette_color(i: usize) -> [u8; 3] {
    let h = (i as f64 * 137.507_764) % 360.0;
    let (s, v) = (0.75, 0.8);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let q = |u: f64| ((u + v - c) * 255.0).round().clamp(0.0, 255.0) as u8;
    [q(r), q(g), q(b)]
}

#[cfg(test)]
mod tests;
