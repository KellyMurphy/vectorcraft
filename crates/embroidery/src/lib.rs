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
pub mod rails;
pub mod skeleton;

use std::sync::Arc;

use kurbo::Point;
use serde::Serialize;
use vectorcraft_color::Paint;
use vectorcraft_doc::{Appearance, Document, EmbroiderySetup, Node, NodeKind, StitchKind, StitchSettings, Underlay};
use vectorcraft_geom::{FillRule, PathData};

pub use dst::{DstError, is_dst, read_dst, write_dst};
pub use import::to_document;

/// Millimetres per point.
pub const PT_MM: f64 = 25.4 / 72.0;
/// The most needle moves a plan holds: past it the rest of the design is left out (with a
/// warning). Real designs have well under a million.
pub const MAX_NEEDLES: usize = 2_000_000;

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
                    // A stitch in place (the needle going down where it jumped to) adds no point.
                    if run.last() != Some(&p) {
                        run.push(p);
                    }
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
    /// The run being sewn locks its thread ([`Builder::run`]).
    tie: bool,
    /// Jumps longer than this trim first (`None`: never on length), mm.
    trim_over: Option<f64>,
    /// Moves up to this long are sewn rather than jumped, mm.
    stitch_under: f64,
}

impl Builder {
    fn new(setup: &EmbroiderySetup) -> Self {
        Self {
            plan: Plan::default(),
            cur: None,
            thread: None,
            full: false,
            tie: true,
            trim_over: setup.trim_over(),
            stitch_under: setup.stitch_under(),
        }
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

    /// Whether the needle last sewed (rather than jumped, trimmed, changed thread or started).
    fn sewing(&self) -> bool {
        self.plan.needles.last().is_some_and(|n| n.cmd == Cmd::Stitch)
    }

    /// Lock the thread before a trim: tiny stitches back along the last stitch and forward again.
    fn tie_off(&mut self) {
        let mut sewn = self.plan.needles.iter().rev().take_while(|n| n.cmd == Cmd::Stitch).map(|n| Point::new(n.x, n.y));
        let Some(end) = sewn.next() else { return };
        let Some(before) = sewn.find(|p| p.distance(end) > 0.05) else { return };
        let Some(back) = tie_point(end, before) else { return };
        for p in [back, end, back, end] {
            self.push(p, Cmd::Stitch);
        }
    }

    /// Cut the thread where the needle is, locking it first.
    fn trim(&mut self, at: Point) {
        if self.sewing() {
            if self.tie {
                self.tie_off();
            }
            self.push(at, Cmd::Trim);
        }
    }

    /// Sew `pts` in `thread`: change thread if needed, travel to the first point (a stitch when
    /// close, else a jump, trimming first when far), then stitch the rest. A run that starts
    /// after a jump, trim or thread change goes down at its first point and locks the thread
    /// with tiny stitches when `tie` (a jump moves the frame without sewing, so the thread isn't
    /// held there).
    fn run(&mut self, thread: [u8; 3], pts: &[Point], tie: bool) {
        let Some((&first, rest)) = pts.split_first() else { return };
        if self.full {
            return;
        }
        // Ties belong to the run before a trim, and to this one after its start.
        let ending = self.tie;
        if self.thread != Some(thread) {
            if self.thread.is_some() {
                let at = self.cur.unwrap_or(first);
                self.tie = ending;
                self.trim(at);
                self.push(at, Cmd::ColorChange);
            }
            self.plan.threads.push(thread);
            self.thread = Some(thread);
        }
        match self.cur {
            Some(c) if self.sewing() && c.distance(first) <= self.stitch_under => {
                if c.distance(first) > 1e-9 {
                    self.push(first, Cmd::Stitch);
                }
            }
            Some(c) => {
                if self.trim_over.is_some_and(|t| c.distance(first) > t) {
                    self.tie = ending;
                    self.trim(c);
                }
                if c.distance(first) > 1e-9 {
                    self.push(first, Cmd::Jump);
                }
            }
            None => {
                self.push(first, Cmd::Jump);
            }
        }
        self.tie = tie;
        if !self.sewing() {
            self.push(first, Cmd::Stitch);
            if let Some(ahead) = rest.iter().find_map(|p| tie_point(first, *p)).filter(|_| tie) {
                for p in [ahead, first, ahead, first] {
                    self.push(p, Cmd::Stitch);
                }
            }
        }
        for p in rest {
            if !self.push(*p, Cmd::Stitch) {
                break;
            }
        }
        self.cur = Some(pts.last().copied().unwrap_or(first));
    }

    /// Trim Before / Trim After: cut the thread where the needle is (locking it first), unless it
    /// is cut already or a thread change before `next` (the thread to sew next) will cut it.
    fn force_trim(&mut self, next: [u8; 3]) {
        if let Some(c) = self.cur
            && self.thread == Some(next)
        {
            // The tie-off is the last run's: none after manual stitches.
            self.trim(c);
        }
    }

    fn finish(mut self) -> Plan {
        if let Some(c) = self.cur {
            self.trim(c);
            self.push(c, Cmd::End);
        }
        self.plan
    }
}

/// How long the lock stitches of a tie-in or tie-off are, mm.
const TIE_MM: f64 = 0.6;

/// The point a tie stitch reaches from `at` toward `toward`: [`TIE_MM`] along, at most half way;
/// none when the two points coincide.
fn tie_point(at: Point, toward: Point) -> Option<Point> {
    let d = at.distance(toward);
    (d > 0.05 && d.is_finite()).then(|| at.lerp(toward, (TIE_MM / d).min(0.5)))
}

/// The stitch plan of `doc`'s visible stitched objects, in paint order.
pub fn plan(doc: &Document) -> Plan {
    let mut b = Builder::new(&doc.embroidery);
    let mut tally = Tally::default();
    for l in &doc.layers {
        walk(l, &mut b, &mut tally, false);
    }
    let mut plan = b.finish();
    let what = |n: usize| if n == 1 { "object" } else { "objects" };
    if tally.skipped > 0 {
        let n = tally.skipped;
        plan.warnings.push(format!("{n} stitched {} can't be stitched yet (only paths, compound paths and type are): left out", what(n)));
    }
    if tally.clipped > 0 {
        let n = tally.clipped;
        plan.warnings.push(format!("{n} stitched {} under a clipping or opacity mask are stitched whole: masks don't apply to stitches", what(n)));
    }
    plan
}

/// Whether any object in `doc` carries stitch settings.
pub fn has_stitches(doc: &Document) -> bool {
    let mut any = false;
    doc.walk(|n| any |= n.stitch.is_some());
    any
}

/// What the walk counts for its warnings.
#[derive(Default)]
struct Tally {
    /// Stitched objects of kinds that can't be stitched.
    skipped: usize,
    /// Stitched objects under a clipping mask or opacity mask, stitched whole.
    clipped: usize,
}

fn walk(n: &Arc<Node>, b: &mut Builder, tally: &mut Tally, masked: bool) {
    // Hidden art isn't sewn, and neither are template layers (art to trace over) nor layers that
    // don't print.
    let off = matches!(n.kind, NodeKind::Layer { template: true, .. } | NodeKind::Layer { printable: false, .. });
    if !n.visible || off {
        return;
    }
    let masked = masked || n.mask.is_some() || matches!(n.kind, NodeKind::Group { clip: true, .. } | NodeKind::Layer { clip: true, .. });
    if let Some(s) = n.stitch.as_deref() {
        let stitched = match &n.kind {
            NodeKind::Path { path, rule, guide: false, clipping: false, .. } => {
                object(n, s, &[path], *rule, b);
                true
            }
            NodeKind::Compound { children, rule } => {
                let paths: Vec<&PathData> = children.iter().filter_map(|c| c.path_data()).collect();
                object(n, s, &paths, *rule, b);
                true
            }
            NodeKind::Text(t) => {
                text(t, s, b);
                true
            }
            NodeKind::Path { .. } => false,
            _ => {
                tally.skipped += 1;
                false
            }
        };
        tally.clipped += usize::from(stitched && masked);
    }
    if let Some(children) = n.children() {
        for c in children {
            walk(c, b, tally, masked);
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
fn thread_of(a: &Appearance, s: &StitchSettings) -> [u8; 3] {
    let fill = || solid(&a.fill_paint());
    let stroke = || solid(&a.stroke_paint());
    let paint = if s.kind.follows_path() { stroke().or_else(fill) } else { fill().or_else(stroke) };
    s.thread.or(paint).unwrap_or([0, 0, 0])
}

fn object(n: &Node, s: &StitchSettings, paths: &[&PathData], rule: FillRule, b: &mut Builder) {
    sew(s, thread_of(&n.appearance, s), n.appearance.stroke_width() * PT_MM, paths, rule, b);
}

/// The typical stroke width of the closed shape `polys`, mm: twice its area over its perimeter
/// (exact for a long even stroke; for lettering, the average of its thick and thin strokes).
pub fn stroke_width(polys: &[Vec<Point>]) -> f64 {
    let mut area = 0.0;
    let mut perimeter = 0.0;
    for p in polys {
        let n = p.len();
        for k in 0..n {
            let (Some(a), Some(b)) = (p.get(k), p.get((k + 1) % n)) else { continue };
            area += a.x * b.y - b.x * a.y;
            perimeter += a.distance(*b);
        }
    }
    if perimeter > 0.0 { (area / 2.0).abs() * 2.0 / perimeter } else { 0.0 }
}

/// Auto's choice for a shape `width` mm wide in its strokes: the settings it is sewn with.
pub fn auto_settings(s: &StitchSettings, width: f64) -> StitchSettings {
    let (thin, wide) = s.auto_thresholds();
    let mut t = s.clone();
    if width < thin {
        t.kind = StitchKind::Bean;
        t.length = None;
    } else if width <= wide {
        t.kind = StitchKind::SatinShape;
        t.underlay = Underlay::Center;
    } else {
        t.kind = StitchKind::Fill;
        t.underlay = Underlay::Fill;
        t.border = Some(s.border().unwrap_or(StitchSettings::DEFAULT_BORDER));
        t.length = None;
    }
    t
}

/// Sew a type object letter by letter (its glyphs as laid out, then its underlines and
/// strikethroughs), each in the thread of its characters' colour. Inline graphics are left out.
fn text(t: &vectorcraft_doc::TextObject, s: &StitchSettings, b: &mut Builder) {
    let db = vectorcraft_text::FontDb::global();
    let lay = vectorcraft_text::layout(db, t);
    let style = |run: usize| t.runs.get(run).map(|r| r.style.clone()).unwrap_or_else(|| t.first_style());
    let glyphs = lay.glyphs.iter().enumerate().filter(|(gi, _)| !lay.inlines.iter().any(|i| i.glyph == *gi)).map(|(_, g)| (g.run, g.outline.clone()));
    let bars = vectorcraft_text::decorations(&lay, db, t);
    let parts: Vec<(usize, PathData)> =
        glyphs.chain(bars).map(|(run, outline)| (run, PathData::from_bezpath(&outline).transformed(t.xf))).filter(|(_, p)| !p.is_empty()).collect();
    if s.kind != StitchKind::Auto {
        for (run, path) in &parts {
            let a = style(*run).appearance();
            sew(s, thread_of(&a, s), a.stroke_width() * PT_MM, &[path], FillRule::NonZero, b);
        }
        return;
    }
    // Auto decides for the whole object, so its letters look alike: when a quarter or more of
    // those that would be satin can't take it (heavy type), they are all filled.
    let mut choices: Vec<Choice> = parts.iter().map(|(_, p)| auto_choice(s, &[p], FillRule::NonZero)).collect();
    let tried = choices.iter().filter(|c| c.tried_satin()).count();
    let poor = choices.iter().filter(|c| matches!(c, Choice::Sew(_, true))).count();
    if tried > 0 && poor * 4 >= tried {
        for c in choices.iter_mut().filter(|c| c.tried_satin()) {
            *c = Choice::Sew(auto_settings(s, f64::INFINITY), true);
        }
    }
    for ((run, path), choice) in parts.iter().zip(choices) {
        let a = style(*run).appearance();
        let thread = thread_of(&a, s);
        match choice {
            Choice::Satin(t, runs) => emit(&t, thread, runs, b),
            Choice::Sew(t, _) => sew(&t, thread, a.stroke_width() * PT_MM, &[path], FillRule::NonZero, b),
        }
    }
}

/// What Auto sews a shape with.
enum Choice {
    /// Satin that suits it: its settings and runs.
    Satin(StitchSettings, Vec<Vec<Point>>),
    /// Sewn with these settings; `true` when satin was tried and didn't suit the shape.
    Sew(StitchSettings, bool),
}

impl Choice {
    fn tried_satin(&self) -> bool {
        matches!(self, Choice::Satin(..) | Choice::Sew(_, true))
    }
}

/// Auto's choice for `paths`: by stroke width, satin only where it suits the shape (a stroke too
/// wide for the curve it turns, or merging into a blob, is filled instead).
fn auto_choice(s: &StitchSettings, paths: &[&PathData], rule: FillRule) -> Choice {
    let polys: Vec<Vec<Point>> = paths.iter().flat_map(|p| generate::polylines(p)).map(|(l, _)| l).collect();
    let t = auto_settings(s, stroke_width(&polys));
    if t.kind != StitchKind::SatinShape {
        return Choice::Sew(t, false);
    }
    let (runs, q) = generate::satin_shape_checked(&polys, rule, t.spacing(), t.pull(), true);
    if !q.poor() && !runs.is_empty() { Choice::Satin(t, runs) } else { Choice::Sew(auto_settings(s, f64::INFINITY), true) }
}

/// Sew `paths` with `s` in `thread` (`stroke_mm`: the stroke's width, for satins).
fn sew(s: &StitchSettings, thread: [u8; 3], stroke_mm: f64, paths: &[&PathData], rule: FillRule, b: &mut Builder) {
    if s.kind == StitchKind::Auto {
        match auto_choice(s, paths, rule) {
            Choice::Satin(t, runs) => emit(&t, thread, runs, b),
            Choice::Sew(t, _) => sew(&t, thread, stroke_mm, paths, rule, b),
        }
        return;
    }
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
                    Ok((u, top)) => {
                        runs.extend(u.into_iter().chain(top));
                        // The border: a satin over each edge, sewn last so it covers the rows' ends.
                        if let Some(w) = s.border() {
                            runs.extend(polys.iter().map(|p| generate::border(p, w)));
                        }
                    }
                    Err(e) => {
                        b.plan.warnings.push(e);
                        return;
                    }
                }
            }
            StitchKind::Satin => {
                // Capped: a wider stitch can't be sewn in one move (DST: 12.1 mm), and splitting it would put
                // a needle hole in the middle of the column.
                let w = (s.width(stroke_mm) + s.pull()).min(*StitchSettings::WIDTH.end());
                // Between Rails: the object's two longest subpaths are the column's edges (any
                // others crossing both are rungs), the column as wide as they are apart.
                let rails = if s.rails { rails::find(&lines) } else { None };
                if s.rails && rails.is_none() {
                    b.plan
                        .warnings
                        .push("a satin set Between Rails needs two subpaths, both open or both closed: sewn along its paths instead".into());
                }
                if let Some(r) = &rails {
                    let (run, narrowed) = r.satin(s.spacing(), s.pull(), s.underlay());
                    if narrowed {
                        b.plan.warnings.push(
                            "a satin between rails is wider than a stitch can span (12 mm) in places: narrowed there (fill areas that wide)".into(),
                        );
                    }
                    runs.push(run);
                }
                for (l, closed) in lines.iter().filter(|_| rails.is_none()) {
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
            StitchKind::SatinShape => {
                let polys: Vec<Vec<Point>> = lines.into_iter().map(|(l, _)| l).collect();
                let satin = generate::satin_shape(&polys, rule, s.spacing(), s.pull(), s.underlay() == Underlay::Center);
                if satin.is_empty() && !polys.is_empty() {
                    // No strokes to follow (a blob, not a letter): fill it instead.
                    match generate::fill(
                        &polys,
                        rule,
                        45.0,
                        StitchSettings::DEFAULT_SPACING,
                        StitchKind::Fill.default_length(),
                        StitchSettings::DEFAULT_STAGGER,
                    ) {
                        Ok(f) if !f.is_empty() => {
                            b.plan.warnings.push("a satin shape has no strokes to follow (it is too round or wide): filled instead".into());
                            runs.extend(f);
                        }
                        Ok(_) => {}
                        Err(e) => b.plan.warnings.push(e),
                    }
                } else {
                    runs.extend(satin);
                }
            }
            StitchKind::Zigzag => {
                let w = s.width(stroke_mm);
                runs.extend(lines.iter().map(|(l, closed)| generate::zigzag(l, *closed, w, s.spacing())));
            }
            StitchKind::Bean => runs.extend(lines.iter().map(|(l, _)| generate::bean(&generate::running(l, s.length()), s.passes()))),
            _ => runs.extend(lines.iter().map(|(l, _)| generate::running(l, s.length()))),
        }
    }
    emit(s, thread, runs, b);
}

/// Sew an object's `runs`, cutting the thread before or after it as `s` asks.
fn emit(s: &StitchSettings, thread: [u8; 3], runs: Vec<Vec<Point>>, b: &mut Builder) {
    // Manual stitches are sewn exactly as drawn: no lock stitches added.
    let tie = s.kind != StitchKind::Manual;
    if runs.iter().all(|r| r.is_empty()) {
        return;
    }
    if s.trim_before {
        b.force_trim(thread);
    }
    for r in runs {
        b.run(thread, &r, tie);
    }
    if s.trim_after {
        b.force_trim(thread);
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
