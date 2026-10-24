//! Embroidery: turns a document's stitched objects into a machine stitch plan, and reads and
//! writes the formats embroidery machines take (DST, PES, JEF, EXP).
//!
//! An object is stitched when it carries [`StitchSettings`] ([`vectorcraft_doc::Node::stitch`]).
//! [`plan`] walks the visible art in paint order (bottom first) and generates each object's
//! stitches (see [`generate`]), joining objects with jumps, trims and colour changes. A
//! [`Plan`] is a list of needle moves in millimetres, document coordinates (y down).
//!
//! - `generate`: running, bean, satin, fill and manual stitches from geometry.
//! - `dst`: the Tajima DST format (writer and reader).
//! - `exp`, `jef`, `pes`: the Melco EXP, Janome JEF and Brother PES formats (writers and readers),
//!   sharing `machine`'s needle moves.
//! - `import`: a plan as editable art (one manual-stitch path per run of stitches).

pub mod borders;
pub mod check;
mod dst;
mod exp;
pub mod extract;
pub mod fabrics;
pub mod fonts;
pub mod generate;
pub mod hoops;
mod import;
pub mod inkstitch;
mod jef;
pub mod kinds;
pub mod lettering;
mod machine;
pub mod motifs;
pub mod nest;
pub mod network;
pub mod params;
pub mod penstrokes;
mod pes;
pub mod preview;
pub mod rails;
pub mod skeleton;
pub mod testsheet;
pub mod threads;
pub mod worksheet;

use std::collections::HashMap;
use std::sync::Arc;

use kurbo::Point;
use serde::Serialize;
use vectorcraft_color::Paint;
use vectorcraft_doc::{Appearance, Document, EmbroiderySetup, Node, NodeId, NodeKind, StitchKind, StitchSettings, Underlay};
use vectorcraft_geom::{FillRule, PathData};

pub use dst::{DstError, is_dst, read_dst, write_dst};
pub use exp::{read_exp, write_exp};
pub use import::to_document;
pub use jef::{is_jef, read_jef, write_jef};
pub use pes::{is_pes, read_pes, write_pes};

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
    /// The objects sewn, in sewing order: which needle moves each made, and its own warnings
    /// (for the design checker, [`check`]).
    #[serde(skip)]
    pub objects: Vec<PlannedObject>,
    /// Stitched objects that aren't sewn as set, and why.
    #[serde(skip)]
    pub left_out: Vec<(NodeId, LeftOut)>,
    /// Whether the design was cut short at [`MAX_NEEDLES`].
    #[serde(skip)]
    pub truncated: bool,
    /// The machine's stops for something done by hand, in sewing order.
    #[serde(skip)]
    pub stops: Vec<PlanStop>,
}

/// A stop of a [`Plan`] for something done by hand.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlanStop {
    /// The colour block sewn after it ([`Plan::threads`]); block 0 with no stop before it means
    /// before the machine starts.
    pub block: usize,
    /// What it is for: an id of [`StitchSettings::STOPS`], or `foam` (puff).
    pub kind: String,
}

/// One object of a plan.
#[derive(Clone, Debug, PartialEq)]
pub struct PlannedObject {
    pub id: NodeId,
    /// Its needle moves: `needles[range]` of the plan (ties, trims and jumps into it included).
    pub needles: std::ops::Range<usize>,
    /// What its stitch type said about its art.
    pub warnings: Vec<String>,
}

/// Why a stitched object isn't sewn as set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeftOut {
    /// Only paths, compound paths and type can be stitched.
    CantStitch,
    /// Its settings come from a newer VectorCraft.
    Newer,
    /// Sewn whole: a clipping or opacity mask over it doesn't apply to stitches.
    Masked,
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

    /// The runs of stitches within `needles[range]` (an object's span, [`PlannedObject::needles`]),
    /// each starting at the point the needle sews from.
    pub fn runs_in(&self, range: std::ops::Range<usize>) -> Vec<Vec<Point>> {
        let mut out: Vec<Vec<Point>> = vec![];
        // Where the needle was before the span: the point the first stitch is sewn from.
        let mut cur: Option<Point> = range.start.checked_sub(1).and_then(|i| self.needles.get(i)).map(|n| Point::new(n.x, n.y));
        let mut run: Vec<Point> = vec![];
        for n in self.needles.get(range).unwrap_or_default() {
            let p = Point::new(n.x, n.y);
            match n.cmd {
                Cmd::Stitch => {
                    if run.is_empty() {
                        run.push(cur.unwrap_or(p));
                    }
                    if run.last() != Some(&p) {
                        run.push(p);
                    }
                    cur = Some(p);
                }
                Cmd::Jump => {
                    if run.len() > 1 {
                        out.push(std::mem::take(&mut run));
                    }
                    run.clear();
                    cur = Some(p);
                }
                _ => {
                    if run.len() > 1 {
                        out.push(std::mem::take(&mut run));
                    }
                    run.clear();
                }
            }
        }
        if run.len() > 1 {
            out.push(run);
        }
        out
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
    /// Needle points closer than this to the one before are left out, mm.
    min_stitch: f64,
    /// Lines without a start point are entered nearest where the needle is.
    auto_start: bool,
    /// The foam has been laid (the stop for it is behind).
    foam_down: bool,
    /// Travel under fills sewn later is sewn rather than jumped.
    hide_travel: bool,
    /// The areas of the fills that hide travel sewn before them.
    hiders: Hiders,
    /// The object being sewn (its place in the sewing order).
    index: usize,
}

/// The longest stitch of travel sewn under a later fill, mm.
pub const TRAVEL_LEN: f64 = 2.5;
/// How far inside a later fill's edge hidden travel stays, mm.
pub const TRAVEL_MARGIN: f64 = 0.5;
/// The side of the grid cells that index the hiding areas, mm.
const HIDER_CELL: f64 = 10.0;
/// More cells than this and an area is checked for every move instead.
const HIDER_CELLS: i64 = 400;

/// The areas that hide travel (fills and satin shapes, by their outlines), each with its place in
/// the sewing order, indexed by a grid of their boxes.
#[derive(Default)]
struct Hiders {
    areas: Vec<(usize, generate::Cut)>,
    cells: HashMap<(i64, i64), Vec<usize>>,
    /// Areas too big for the grid.
    big: Vec<usize>,
}

impl Hiders {
    fn new(items: &[&Arc<Node>]) -> Self {
        let mut h = Hiders::default();
        for (i, n) in items.iter().enumerate() {
            let Some(cut) = hide_area(n) else { continue };
            let (mut lo, mut hi) = (Point::new(f64::INFINITY, f64::INFINITY), Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
            for p in cut.polys.iter().flatten() {
                lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
                hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
            }
            if !(lo.x.is_finite() && hi.x.is_finite()) {
                continue;
            }
            let k = h.areas.len();
            h.areas.push((i, cut));
            let (c0, c1) = (cell(lo), cell(hi));
            if (c1.0 - c0.0 + 1).saturating_mul(c1.1 - c0.1 + 1) > HIDER_CELLS {
                h.big.push(k);
                continue;
            }
            for x in c0.0..=c1.0 {
                for y in c0.1..=c1.1 {
                    h.cells.entry((x, y)).or_default().push(k);
                }
            }
        }
        h
    }

    /// Is `p` at least [`TRAVEL_MARGIN`] inside an area sewn after object `after`?
    fn hides(&self, p: Point, after: usize) -> bool {
        let near = self.cells.get(&cell(p)).into_iter().flatten().chain(&self.big);
        near.filter_map(|k| self.areas.get(*k)).filter(|(i, _)| *i > after).any(|(_, cut)| {
            [(0.0, 0.0), (1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)]
                .iter()
                .all(|(dx, dy)| generate::inside(&cut.polys, cut.rule, Point::new(p.x + dx * TRAVEL_MARGIN, p.y + dy * TRAVEL_MARGIN)))
        })
    }
}

/// The grid cell of `p`.
fn cell(p: Point) -> (i64, i64) {
    let f = |v: f64| (v / HIDER_CELL).floor().clamp(-1e9, 1e9) as i64;
    (f(p.x), f(p.y))
}

/// The area a fill or satin shape covers, which hides travel sewn before it (`None`: anything
/// else, and puffed objects, whose foam is laid after).
fn hide_area(n: &Node) -> Option<generate::Cut> {
    let s = n.extras().stitch.as_deref()?;
    if s.unknown.is_some() || s.puffed() || !matches!(s.kind, StitchKind::Fill | StitchKind::SatinShape) {
        return None;
    }
    let (paths, rule): (Vec<PathData>, FillRule) = match &n.kind {
        NodeKind::Path { path, rule, .. } => (vec![path.clone()], *rule),
        NodeKind::Compound { children, rule } => (children.iter().filter_map(|c| c.path_data().cloned()).collect(), *rule),
        NodeKind::Text(t) => (glyph_parts(t).into_iter().map(|(_, p)| p).collect(), FillRule::NonZero),
        _ => return None,
    };
    let polys: Vec<Vec<Point>> = paths.iter().flat_map(generate::polylines).filter(|(l, _)| l.len() >= 3).map(|(l, _)| l).collect();
    (!polys.is_empty()).then_some(generate::Cut { polys, rule })
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
            min_stitch: setup.min_stitch(),
            auto_start: setup.auto_start,
            foam_down: false,
            hide_travel: setup.hide_travel,
            hiders: Hiders::default(),
            index: 0,
        }
    }

    /// Whether the move from `a` to `b` lies under fills sewn after the current object, at
    /// least [`TRAVEL_MARGIN`] inside them all the way.
    fn hidden(&self, a: Point, b: Point) -> bool {
        let n = ((a.distance(b) / 0.4).ceil() as usize).clamp(1, 2000);
        (0..=n).all(|k| self.hiders.hides(a.lerp(b, k as f64 / n as f64), self.index))
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

    /// The needle point before the last one, when both are stitches.
    fn before_last(&self) -> Option<Point> {
        let n = self.plan.needles.len();
        let (a, b) = (self.plan.needles.get(n.checked_sub(2)?)?, self.plan.needles.get(n - 1)?);
        (a.cmd == Cmd::Stitch && b.cmd == Cmd::Stitch).then(|| Point::new(a.x, a.y))
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
            // Under a fill sewn later: sewn there as running stitches, which it hides.
            Some(c) if self.hide_travel && self.sewing() && self.hidden(c, first) => {
                let n = ((c.distance(first) / TRAVEL_LEN).ceil() as usize).max(1);
                for k in 1..=n {
                    self.push(c.lerp(first, k as f64 / n as f64), Cmd::Stitch);
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
        // Stitches shorter than the shortest allowed are merged into the next, unless that would
        // make a stitch too long to sew (manual stitches, `tie` off, are sewn as drawn). The run
        // still ends where it ends.
        let min = if tie { self.min_stitch } else { 0.0 };
        let mut at = first;
        let last = rest.len().saturating_sub(1);
        for (k, p) in rest.iter().enumerate() {
            if min > 0.0 && at.distance(*p) < min {
                if k < last {
                    // Left out when the stitch that takes its place can still be sewn.
                    if rest.get(k + 1).is_some_and(|q| at.distance(*q) <= generate::MAX_STITCH) {
                        continue;
                    }
                } else if at.distance(*p) <= 1e-9 {
                    continue;
                } else if at != first && self.before_last().is_some_and(|b| b.distance(*p) <= generate::MAX_STITCH) {
                    // The end is too close to the point before: that point goes, so the run
                    // still ends where it ends.
                    self.plan.needles.pop();
                }
            }
            if !self.push(*p, Cmd::Stitch) {
                break;
            }
            at = *p;
        }
        self.cur = Some(pts.last().copied().unwrap_or(first));
    }

    /// Trim Before / Trim After: cut the thread where the needle is (locking it first), unless it
    /// is cut already or a thread change before `next` (the thread to sew next) will cut it.
    /// Sew one object: its warnings, then its pieces.
    fn apply(&mut self, sewn: &Sewn) {
        self.plan.warnings.extend(sewn.warnings.iter().cloned());
        for p in &sewn.pieces {
            if p.trim_before {
                self.force_trim(p.thread);
            }
            if let Some(kind) = &p.stop {
                self.stop(p.thread, kind);
            }
            if p.foam && !self.foam_down {
                self.foam_down = true;
                self.stop(p.thread, "foam");
            }
            let from = match p.orient {
                Orient::Free { start: Some(s), rotate } => Some((s, rotate)),
                Orient::Free { start: None, rotate } if self.auto_start => self.cur.map(|c| (c, rotate)),
                _ => None,
            };
            let turned;
            let runs = match from {
                Some((at, rotate)) => {
                    turned = entered(&p.runs, at, rotate);
                    &turned
                }
                None => &p.runs,
            };
            for r in runs {
                self.run(p.thread, r, p.tie);
            }
            if p.trim_after {
                self.force_trim(p.thread);
            }
        }
    }

    /// Stop for `kind` (foam to lay, fabric to place or trim), sewing on in `thread` after it: a
    /// thread change, to the same thread or the next one. Nothing sewn yet, it is done before
    /// the start; a stop just made here serves for this one too.
    fn stop(&mut self, thread: [u8; 3], kind: &str) {
        let Some(at) = self.cur else {
            self.plan.stops.push(PlanStop { block: 0, kind: kind.to_string() });
            return;
        };
        if self.plan.needles.last().is_none_or(|n| n.cmd != Cmd::ColorChange) {
            self.trim(at);
            self.push(at, Cmd::ColorChange);
            self.plan.threads.push(thread);
            self.thread = Some(thread);
        }
        let block = self.plan.threads.len().saturating_sub(1);
        self.plan.stops.push(PlanStop { block, kind: kind.to_string() });
    }

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

/// The stitch plan of `doc`'s visible stitched objects, in sewing order.
pub fn plan(doc: &Document) -> Plan {
    plan_cached(doc, &mut StitchCache::default())
}

/// [`plan`], sewing again only the objects `cache` doesn't hold as they are now.
pub fn plan_cached(doc: &Document, cache: &mut StitchCache) -> Plan {
    let overlap = doc.embroidery.overlap();
    let e = &doc.embroidery;
    if cache.patterns != e.patterns || cache.fonts != e.fonts || cache.fabric != e.fabric || cache.overlap != overlap {
        cache.entries.clear();
        cache.patterns.clone_from(&e.patterns);
        cache.fonts.clone_from(&e.fonts);
        cache.fabric.clone_from(&doc.embroidery.fabric);
        cache.overlap = overlap;
    }
    (cache.reused, cache.sewn) = (0, 0);
    let mut b = Builder::new(&doc.embroidery);
    let mut tally = Tally::default();
    // Only the objects sewn now are kept for the next plan.
    let mut next = HashMap::new();
    let mut objects = vec![];
    let items = ordered(doc, &mut tally);
    let covers = coverers(&items);
    if b.hide_travel {
        b.hiders = Hiders::new(&items);
    }
    for (i, n) in items.iter().enumerate() {
        b.index = i;
        let over: Vec<Arc<Node>> = covers.get(i).into_iter().flatten().filter_map(|j| items.get(*j).map(|c| Arc::clone(c))).collect();
        let sewn = cache.get(n, over, &mut next);
        let start = b.plan.needles.len();
        b.apply(&sewn);
        objects.push(PlannedObject { id: n.id, needles: start..b.plan.needles.len(), warnings: sewn.warnings.clone() });
    }
    cache.entries = next;
    let truncated = b.full;
    let mut plan = b.finish();
    plan.objects = objects;
    plan.truncated = truncated;
    plan.left_out = std::mem::take(&mut tally.ids);
    let what = |n: usize| if n == 1 { "object" } else { "objects" };
    if tally.skipped > 0 {
        let n = tally.skipped;
        plan.warnings.push(format!("{n} stitched {} can't be stitched yet (only paths, compound paths and type are): left out", what(n)));
    }
    if tally.newer > 0 {
        let n = tally.newer;
        plan.warnings.push(format!(
            "{n} stitched {} use stitch settings from a newer VectorCraft that this version can't read: kept as they are, but not sewn",
            what(n)
        ));
    }
    if tally.clipped > 0 {
        let n = tally.clipped;
        plan.warnings.push(format!("{n} stitched {} under a clipping or opacity mask are stitched whole: masks don't apply to stitches", what(n)));
    }
    plan
}

/// Seconds a trim takes (the machine cuts and moves on).
pub const TRIM_SECONDS: f64 = 5.0;
/// Seconds a thread change takes (the machine stops; someone changes the thread).
pub const CHANGE_SECONDS: f64 = 30.0;

/// How long `plan` takes to sew at `speed` stitches per minute, seconds: its stitches, plus
/// [`TRIM_SECONDS`] per trim and [`CHANGE_SECONDS`] per thread change (an estimate: real machines
/// slow down for long stitches and frame moves).
pub fn sewing_seconds(plan: &Plan, speed: f64) -> f64 {
    let st = plan.stats();
    let spm = if speed.is_finite() && speed > 0.0 { speed } else { EmbroiderySetup::DEFAULT_SPEED };
    st.stitches as f64 * 60.0 / spm + st.trims as f64 * TRIM_SECONDS + st.color_changes as f64 * CHANGE_SECONDS
}

/// Whether any object in `doc` carries stitch settings.
pub fn has_stitches(doc: &Document) -> bool {
    let mut any = false;
    doc.walk(|n| any |= n.extras().stitch.is_some());
    any
}

/// The objects `doc` sews, in the order it sews them: those in the document's sewing order
/// ([`EmbroiderySetup::order`]) first, in that order, then the others in paint order.
pub fn sew_order(doc: &Document) -> Vec<&Node> {
    ordered(doc, &mut Tally::default()).into_iter().map(|n| n.as_ref()).collect()
}

fn ordered<'a>(doc: &'a Document, tally: &mut Tally) -> Vec<&'a Arc<Node>> {
    let mut items: Vec<&Arc<Node>> = vec![];
    for l in &doc.layers {
        collect(l, &mut items, tally, false);
    }
    let order = &doc.embroidery.order;
    if order.is_empty() {
        return items;
    }
    // Listed ones first (each once, in list order), then the rest as painted.
    let mut out: Vec<&Arc<Node>> = vec![];
    for id in order {
        if let Some(&n) = items.iter().find(|n| n.id == *id)
            && !out.iter().any(|o| o.id == n.id)
        {
            out.push(n);
        }
    }
    out.extend(items.into_iter().filter(|n| !order.contains(&n.id)));
    out
}

/// What the walk counts for its warnings.
#[derive(Default)]
struct Tally {
    /// Stitched objects of kinds that can't be stitched.
    skipped: usize,
    /// Stitched objects under a clipping mask or opacity mask, stitched whole.
    clipped: usize,
    /// Objects whose stitch settings came from a newer VectorCraft (kept, not sewn).
    newer: usize,
    /// Each of those objects, and why.
    ids: Vec<(NodeId, LeftOut)>,
}

/// The objects under `n` that are sewn, in paint order.
fn collect<'a>(n: &'a Arc<Node>, items: &mut Vec<&'a Arc<Node>>, tally: &mut Tally, masked: bool) {
    // Hidden art isn't sewn, and neither are template layers (art to trace over) nor layers that
    // don't print.
    let off = matches!(n.kind, NodeKind::Layer { template: true, .. } | NodeKind::Layer { printable: false, .. });
    if !n.visible || off {
        return;
    }
    let masked = masked || n.extras().mask.is_some() || matches!(n.kind, NodeKind::Group { clip: true, .. } | NodeKind::Layer { clip: true, .. });
    if n.extras().stitch.as_deref().is_some_and(|s| s.unknown.is_some()) {
        tally.newer += 1;
        tally.ids.push((n.id, LeftOut::Newer));
    } else if n.extras().stitch.is_some() {
        let sewn = match &n.kind {
            NodeKind::Path { guide: false, clipping: false, .. } | NodeKind::Compound { .. } | NodeKind::Text(_) => true,
            NodeKind::Path { .. } => false,
            _ => {
                tally.skipped += 1;
                tally.ids.push((n.id, LeftOut::CantStitch));
                false
            }
        };
        if sewn {
            items.push(n);
            tally.clipped += usize::from(masked);
            if masked {
                tally.ids.push((n.id, LeftOut::Masked));
            }
        }
    }
    if let Some(children) = n.children() {
        for c in children {
            collect(c, items, tally, masked);
        }
    }
}

/// Sew one stitched object.
fn sew_node(n: &Node, b: &mut Sewer) {
    let Some(s) = n.extras().stitch.as_deref() else { return };
    match &n.kind {
        NodeKind::Path { path, rule, .. } => object(n, s, &[path], *rule, b),
        NodeKind::Compound { children, rule } => {
            let paths: Vec<&PathData> = children.iter().filter_map(|c| c.path_data()).collect();
            object(n, s, &paths, *rule, b);
        }
        NodeKind::Text(t) => text(t, s, b),
        _ => {}
    }
}

/// The thread an object is sewn in (type: its first characters' colour).
pub fn thread_of_node(n: &Node) -> [u8; 3] {
    let Some(s) = n.extras().stitch.as_deref() else { return [0; 3] };
    match &n.kind {
        NodeKind::Text(t) => thread_of(&t.first_style().appearance(), s),
        _ => thread_of(&n.appearance, s),
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

fn object(n: &Node, s: &StitchSettings, paths: &[&PathData], rule: FillRule, b: &mut Sewer) {
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

/// The narrowest hole (a letter's counter) among `polys`, mm across: the diameter of the largest
/// circle that fits in it (`None`: no holes). A hole is a ring wound the other way from the
/// largest one.
pub fn narrowest_hole(polys: &[Vec<Point>]) -> Option<f64> {
    let area = |p: &Vec<Point>| -> f64 {
        let n = p.len();
        (0..n).filter_map(|k| Some((p.get(k)?, p.get((k + 1) % n)?))).map(|(a, b)| a.x * b.y - b.x * a.y).sum::<f64>() / 2.0
    };
    let outer = polys.iter().map(area).max_by(|a, b| a.abs().total_cmp(&b.abs()))?;
    polys
        .iter()
        .filter(|p| p.len() > 2 && area(p).signum() != outer.signum() && area(p).abs() > 1e-3)
        .map(|p| 2.0 * inscribed_radius(p))
        .min_by(f64::total_cmp)
}

/// The radius of the largest circle inside the polygon `poly` (approximately: a grid search,
/// refined twice round the best point).
fn inscribed_radius(poly: &[Point]) -> f64 {
    let n = poly.len();
    let inside = |p: Point| {
        let mut c = false;
        for k in 0..n {
            let (Some(a), Some(b)) = (poly.get(k), poly.get((k + 1) % n)) else { continue };
            if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                c = !c;
            }
        }
        c
    };
    let edge = |p: Point| -> f64 {
        (0..n)
            .filter_map(|k| Some((*poly.get(k)?, *poly.get((k + 1) % n)?)))
            .map(|(a, b)| {
                let ab = b - a;
                let t = if ab.hypot2() > 0.0 { ((p - a).dot(ab) / ab.hypot2()).clamp(0.0, 1.0) } else { 0.0 };
                p.distance(a + ab * t)
            })
            .fold(f64::INFINITY, f64::min)
    };
    let (mut lo, mut hi) = (Point::new(f64::INFINITY, f64::INFINITY), Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
    for p in poly {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    if !(lo.x.is_finite() && hi.x.is_finite()) {
        return 0.0;
    }
    const STEPS: usize = 16;
    let mut best = (lo.midpoint(hi), 0.0f64);
    let (mut centre, mut half) = (lo.midpoint(hi), kurbo::Vec2::new((hi.x - lo.x) / 2.0, (hi.y - lo.y) / 2.0));
    for _ in 0..3 {
        for i in 0..=STEPS {
            for j in 0..=STEPS {
                let p = Point::new(
                    centre.x - half.x + 2.0 * half.x * i as f64 / STEPS as f64,
                    centre.y - half.y + 2.0 * half.y * j as f64 / STEPS as f64,
                );
                if inside(p) {
                    let d = edge(p);
                    if d > best.1 {
                        best = (p, d);
                    }
                }
            }
        }
        // Again, finer, round the best point so far.
        centre = best.0;
        half *= 2.0 / STEPS as f64;
    }
    best.1
}

/// The border Auto gives a filled letter or shape `polys` (mm wide): the user's when set, else
/// the default narrowed so that, centred on the edge, it covers under half of the narrowest
/// stroke and of the narrowest hole (a border wider than that closes small letters up), and none
/// when that leaves it too thin to be a satin.
pub fn auto_border(s: &StitchSettings, polys: &[Vec<Point>]) -> Option<f64> {
    if s.border.is_some() {
        return s.border();
    }
    let room = narrowest_hole(polys).map_or(stroke_width(polys), |h| h.min(stroke_width(polys)));
    let w = StitchSettings::DEFAULT_BORDER.min(AUTO_BORDER_SHARE * room);
    (w >= AUTO_BORDER_LEAST).then_some(w)
}

/// The narrowest satin column Auto sews a letter's strokes with, mm: thinner strokes (small
/// type, hairlines) are widened to it.
pub const LETTER_COLUMN: f64 = 0.8;

/// The share of a letter's narrowest stroke or hole Auto's border may cover.
pub const AUTO_BORDER_SHARE: f64 = 0.45;
/// The narrowest border Auto sews, mm: thinner, the letter is filled without one.
pub const AUTO_BORDER_LEAST: f64 = 0.6;

/// Auto's fill for a shape too wide or too heavy for satin: fill with a border fitted to it
/// ([`auto_border`]).
pub(crate) fn auto_fill(s: &StitchSettings, polys: &[Vec<Point>]) -> StitchSettings {
    let mut t = auto_settings(s, f64::INFINITY);
    t.border = auto_border(s, polys);
    t
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
        t.underlay = Underlay::Auto;
    } else {
        t.kind = StitchKind::Fill;
        t.underlay = Underlay::Auto;
        t.border = Some(s.border().unwrap_or(StitchSettings::DEFAULT_BORDER));
        t.length = None;
    }
    t
}

/// Sew a type object letter by letter (its glyphs as laid out, then its underlines and
/// strikethroughs), each in the thread of its characters' colour. Inline graphics are left out.
fn text(t: &vectorcraft_doc::TextObject, s: &StitchSettings, b: &mut Sewer) {
    let style = |run: usize| t.runs.get(run).map(|r| r.style.clone()).unwrap_or_else(|| t.first_style());
    if let Some(id) = s.font.as_deref() {
        match b.fonts.iter().find(|f| f.id == id && f.unknown.is_none()) {
            Some(font) => {
                let font = font.clone();
                return in_font(t, s, &font, b);
            }
            None => {
                b.out.warnings.push(format!("type is set in the embroidery font `{id}`, which the document doesn't have: sewn from its outline font"))
            }
        }
    }
    let parts = glyph_parts(t);
    // Letters are written as their face is: script faces by the flow of the pen.
    let first = t.first_style();
    b.hand = penstrokes::hand_of(&first.font_family, &first.font_style);
    if s.kind == StitchKind::SatinShape {
        return satin_letters(t, s, &parts, b);
    }
    if s.kind != StitchKind::Auto {
        for (run, path) in &parts {
            let a = style(*run).appearance();
            sew(s, thread_of(&a, s), a.stroke_width() * PT_MM, &[path], FillRule::NonZero, b);
        }
        return;
    }
    // Auto decides for the whole object, so its letters look alike: when a quarter or more of
    // those that would be satin can't take it (heavy type), they are all filled.
    let mut choices: Vec<Choice> = parts.iter().map(|(_, p)| auto_choice(s, &[p], FillRule::NonZero, b, true)).collect();
    let tried = choices.iter().filter(|c| c.tried_satin()).count();
    let poor = choices.iter().filter(|c| matches!(c, Choice::Sew(_, true))).count();
    if tried > 0 && poor * 4 >= tried {
        for ((_, path), c) in parts.iter().zip(choices.iter_mut()).filter(|(_, c)| c.tried_satin()) {
            let polys: Vec<Vec<Point>> = generate::polylines(path).into_iter().map(|(l, _)| l).collect();
            *c = Choice::Sew(b.fabric_in(&auto_fill(s, &polys)), true);
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

/// Type sewn as satin shapes, letter by letter, where satin can cover them: letters it can't
/// (strokes too heavy for satin to reach their corners and junctions) are sewn as a fill with a
/// satin border instead, as Auto does, and the whole object is when a quarter or more of its
/// letters are, so its letters look alike.
fn satin_letters(t: &vectorcraft_doc::TextObject, s: &StitchSettings, parts: &[(usize, PathData)], b: &mut Sewer) {
    let style = |run: usize| t.runs.get(run).map(|r| r.style.clone()).unwrap_or_else(|| t.first_style());
    let polys: Vec<Vec<Vec<Point>>> = parts.iter().map(|(_, p)| generate::polylines(p).into_iter().map(|(l, _)| l).collect()).collect();
    let poor: Vec<bool> = polys
        .iter()
        .map(|p| {
            let s = b.fabric_in(s);
            !p.is_empty() && generate::satin_shape_hand(p, FillRule::NonZero, s.spacing(), 0.0, false, 0.0, b.hand).1.poor()
        })
        .collect();
    let n_poor = poor.iter().filter(|p| **p).count();
    let all = n_poor > 0 && n_poor * 4 >= parts.len();
    if n_poor > 0 {
        b.out.warnings.push(if all {
            "these letters are too heavy for satin to cover (Satin Shape): all of them are sewn as a fill with a satin border; Auto does the same, or choose a lighter font".to_string()
        } else {
            format!("{n_poor} letters are too heavy for satin to cover (Satin Shape): sewn as a fill with a satin border")
        });
    }
    for (((run, path), p), bad) in parts.iter().zip(&polys).zip(&poor) {
        let a = style(*run).appearance();
        let settings = if all || *bad { b.fabric_in(&auto_fill(s, p)) } else { s.clone() };
        sew(&settings, thread_of(&a, s), a.stroke_width() * PT_MM, &[path], FillRule::NonZero, b);
    }
}

/// Type `t` sewn from embroidery font `font` (E8.1b): each letter's stitch objects with their own
/// settings, in the colour of the run they are in unless they name a thread; characters the font
/// lacks from the outline font, as `s` says, where the embroidery letter would be.
fn in_font(t: &vectorcraft_doc::TextObject, s: &StitchSettings, font: &vectorcraft_doc::EmbroideryFont, b: &mut Sewer) {
    let style = |run: usize| t.runs.get(run).map(|r| r.style.clone()).unwrap_or_else(|| t.first_style());
    let l = lettering::set(t, font);
    if l.as_point {
        b.out
            .warnings
            .push("area type or type on a path in an embroidery font is set as point type here: no outline font is installed to lay it out".into());
    }
    if let Some([lo, hi]) = font.sizes
        && l.cap_mm > 0.0
        && !(lo - 1e-6..=hi + 1e-6).contains(&l.cap_mm)
    {
        b.out.warnings.push(format!("{} is made for capitals {lo}–{hi} mm tall; this type's are {:.1} mm", font.name, l.cap_mm));
    }
    for p in &l.placed {
        let a = style(p.run).appearance();
        let settings = b.fabric_in(&p.object.stitch);
        let thread = thread_of(&a, &settings);
        sew(&settings, thread, 0.0, &[&p.path], p.rule, b);
    }
    if l.missing.is_empty() {
        return;
    }
    // The missing characters, from the outline font, moved to where they go.
    let db = vectorcraft_text::FontDb::global();
    let lay = vectorcraft_text::layout(db, t);
    let mut absent = vec![];
    for m in &l.missing {
        let Some(g) = lay.glyphs.iter().find(|g| g.byte == m.byte) else {
            absent.push(m.chars.clone());
            continue;
        };
        let to = kurbo::Affine::translate(m.pen.to_vec2()) * kurbo::Affine::rotate(m.angle - g.angle) * kurbo::Affine::translate(-g.origin.to_vec2());
        let path = PathData::from_bezpath(&(to * g.outline.clone())).transformed(t.xf);
        if path.is_empty() {
            continue;
        }
        let a = style(m.run).appearance();
        let mut own = s.clone();
        own.font = None;
        sew(&own, thread_of(&a, &own), a.stroke_width() * PT_MM, &[&path], FillRule::NonZero, b);
    }
    let all: Vec<&str> = l.missing.iter().map(|m| m.chars.as_str()).collect();
    b.out.warnings.push(format!(
        "{} has no {}: sewn from the outline font",
        font.name,
        all.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join(", ")
    ));
    if !absent.is_empty() {
        b.out.warnings.push(format!("no outline font has {}: left out", absent.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join(", ")));
    }
}

/// A type object's letters as laid out (then its underlines and strikethroughs), each with the
/// run it is in, inline graphics left out.
fn glyph_parts(t: &vectorcraft_doc::TextObject) -> Vec<(usize, PathData)> {
    let db = vectorcraft_text::FontDb::global();
    let lay = vectorcraft_text::layout(db, t);
    let glyphs = lay.glyphs.iter().enumerate().filter(|(gi, _)| !lay.inlines.iter().any(|i| i.glyph == *gi)).map(|(_, g)| (g.run, g.outline.clone()));
    let bars = vectorcraft_text::decorations(&lay, db, t);
    glyphs.chain(bars).map(|(run, outline)| (run, PathData::from_bezpath(&outline).transformed(t.xf))).filter(|(_, p)| !p.is_empty()).collect()
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
/// wide for the curve it turns, or merging into a blob, is filled instead). A letter
/// (`lettering`) thinner than satin's threshold is sewn as satin columns widened to
/// [`LETTER_COLUMN`], not outlined: small type keeps its shape.
fn auto_choice(s: &StitchSettings, paths: &[&PathData], rule: FillRule, b: &Sewer, lettering: bool) -> Choice {
    let polys: Vec<Vec<Point>> = paths.iter().flat_map(|p| generate::polylines(p)).map(|(l, _)| l).collect();
    let width = stroke_width(&polys);
    let mut t = b.fabric_in(&auto_settings(s, width));
    let narrow = lettering && t.kind == StitchKind::Bean && width > 0.0;
    if narrow {
        t = b.fabric_in(&auto_settings(s, s.auto_thresholds().0));
    }
    if t.kind == StitchKind::Fill {
        return Choice::Sew(b.fabric_in(&auto_fill(s, &polys)), false);
    }
    if t.kind != StitchKind::SatinShape {
        return Choice::Sew(t, false);
    }
    let underlay = match t.underlay() {
        Underlay::Auto => b.auto.shape(width),
        u => u,
    };
    let least = if narrow { LETTER_COLUMN } else { 0.0 };
    let (runs, q) = generate::satin_shape_hand(&polys, rule, t.spacing(), t.pull_for(width), underlay == Underlay::Center, least, b.hand);
    if q.poor() || runs.is_empty() {
        return Choice::Sew(b.fabric_in(&auto_fill(s, &polys)), true);
    }
    // An edge walk goes in first, under the satin.
    let walk = if underlay == Underlay::Edge { generate::edge_walk(&polys, rule) } else { vec![] };
    Choice::Satin(t, walk.into_iter().chain(runs).collect())
}

/// Sew `paths` with `s` in `thread` (`stroke_mm`: the stroke's width, for satins).
fn sew(s: &StitchSettings, thread: [u8; 3], stroke_mm: f64, paths: &[&PathData], rule: FillRule, b: &mut Sewer) {
    if s.kind == StitchKind::Auto {
        match auto_choice(s, paths, rule, b, false) {
            Choice::Satin(t, runs) => emit(&t, thread, runs, b),
            Choice::Sew(t, _) => sew(&t, thread, stroke_mm, paths, rule, b),
        }
        return;
    }
    let s = &b.fabric_in(s);
    let out = kinds::sew(&kinds::Input {
        settings: s,
        stroke_mm,
        paths,
        rule,
        patterns: b.patterns,
        auto: b.auto,
        cuts: &b.cuts,
        overlap: b.overlap,
        hand: b.hand,
    });
    b.out.warnings.extend(out.warnings);
    emit(s, thread, out.runs, b);
}

/// An object's `runs` in `thread`, cutting the thread before or after it as `s` asks.
fn emit(s: &StitchSettings, thread: [u8; 3], runs: Vec<Vec<Point>>, b: &mut Sewer) {
    if runs.iter().all(|r| r.is_empty()) {
        return;
    }
    // Manual stitches are sewn exactly as drawn: no lock stitches added.
    let tie = s.kind != StitchKind::Manual;
    // Lines whose parts may be taken in any order and either way (not fills or satins, whose
    // underlay comes first, nor manual stitches).
    let free = matches!(s.kind, StitchKind::Running | StitchKind::Bean | StitchKind::Zigzag);
    let start = s.start_at.filter(|p| p.iter().all(|v| v.is_finite())).map(|[x, y]| Point::new(x, y));
    let orient = match (free, start) {
        (false, _) => Orient::Fixed,
        (true, start) => Orient::Free { start, rotate: s.kind == StitchKind::Running },
    };
    let foam = s.puffed();
    // The object's stop comes before its first piece.
    let stop = if b.out.pieces.is_empty() { s.stop_before.clone() } else { None };
    b.out.pieces.push(Piece { thread, runs, tie, trim_before: s.trim_before, trim_after: s.trim_after, orient, foam, stop });
}

/// One object's stitches: pieces in sewing order and warnings about its art. What
/// [`StitchCache`] keeps for each object.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sewn {
    pieces: Vec<Piece>,
    warnings: Vec<String>,
}

/// Runs in one thread, with how they are locked and cut.
#[derive(Clone, Debug, PartialEq)]
struct Piece {
    thread: [u8; 3],
    runs: Vec<Vec<Point>>,
    /// Lock the thread at each run's start and before each trim (not for manual stitches).
    tie: bool,
    trim_before: bool,
    trim_after: bool,
    /// How the plan may enter the runs.
    orient: Orient,
    /// Sewn over foam: the machine stops before the first such piece, for the foam to be laid.
    foam: bool,
    /// The machine stops before it ([`StitchSettings::stop_before`]).
    stop: Option<String>,
}

/// How a piece's runs may be entered.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Orient {
    /// As made.
    Fixed,
    /// In any order and either way, from `start` or else where the needle is (when the document
    /// asks); a closed run may start anywhere along it when `rotate`.
    Free { start: Option<Point>, rotate: bool },
}

/// `runs` ordered and turned to be entered from `at`: each next the one with an end (or, when
/// `rotate`, a point of a closed run) nearest where the last one ended.
fn entered(runs: &[Vec<Point>], mut at: Point, rotate: bool) -> Vec<Vec<Point>> {
    let mut left: Vec<Vec<Point>> = runs.iter().filter(|r| !r.is_empty()).cloned().collect();
    let mut out = vec![];
    while !left.is_empty() {
        // (distance, run, how: start, end, or a point of a closed run)
        let mut best: Option<(f64, usize, EnterAt)> = None;
        for (i, r) in left.iter().enumerate() {
            let (Some(first), Some(last)) = (r.first(), r.last()) else { continue };
            let closed = r.len() > 2 && first.distance(*last) < 1e-6;
            let mut consider = |d: f64, how: EnterAt| {
                if best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, i, how));
                }
            };
            if closed && rotate {
                for (k, p) in r.iter().enumerate() {
                    consider(p.distance(at), EnterAt::At(k));
                }
            } else {
                consider(first.distance(at), EnterAt::Start);
                if !closed {
                    consider(last.distance(at), EnterAt::End);
                }
            }
        }
        let Some((_, i, how)) = best else { break };
        let r = left.remove(i);
        let run = match how {
            EnterAt::Start => r,
            EnterAt::End => r.into_iter().rev().collect(),
            EnterAt::At(k) => {
                // A closed run (its last point its first) from its kth point round to it again.
                let n = r.len().saturating_sub(1);
                (0..=n).filter_map(|j| r.get((k + j) % n.max(1))).copied().collect()
            }
        };
        if let Some(end) = run.last() {
            at = *end;
        }
        out.push(run);
    }
    out
}

/// Where a run is entered.
#[derive(Clone, Copy, Debug, PartialEq)]
enum EnterAt {
    Start,
    End,
    At(usize),
}

/// Sews one object into a [`Sewn`].
struct Sewer<'a> {
    /// The document's own border patterns ([`EmbroiderySetup::patterns`]).
    patterns: &'a [vectorcraft_doc::BorderPattern],
    /// The document's embroidery fonts ([`EmbroiderySetup::fonts`]).
    fonts: &'a [vectorcraft_doc::EmbroideryFont],
    /// The document's fabric ([`EmbroiderySetup::fabric`]), and the auto underlay it asks for.
    fabric: Option<&'a vectorcraft_doc::Fabric>,
    auto: generate::AutoUnderlay,
    /// The areas sewn later on top of the object, and the document's overlap, mm.
    cuts: Vec<generate::Cut>,
    overlap: f64,
    /// How the type being sewn is written (print or script), for satin by pen strokes.
    hand: penstrokes::Hand,
    out: Sewn,
}

impl Sewer<'_> {
    /// `s` with what it leaves unset taken from the document's fabric.
    fn fabric_in(&self, s: &StitchSettings) -> StitchSettings {
        match self.fabric {
            Some(f) => fabrics::fill_in(s, f),
            None => s.clone(),
        }
    }
}

/// The stitches of objects already sewn, kept between plans so a plan sews again only what
/// changed. An object counts as unchanged when it is the very same node: documents share the nodes
/// an edit doesn't touch, and since an entry holds its node, editing one copies it rather than
/// changing it in place. A change to the document's border patterns, fonts, fabric or overlap
/// empties the cache.
#[derive(Default)]
pub struct StitchCache {
    entries: HashMap<NodeId, Entry>,
    patterns: Vec<vectorcraft_doc::BorderPattern>,
    fonts: Vec<vectorcraft_doc::EmbroideryFont>,
    fabric: Option<Box<vectorcraft_doc::Fabric>>,
    overlap: f64,
    /// Objects the last plan took from the cache and sewed again.
    pub reused: usize,
    pub sewn: usize,
}

/// A cached object: the node sewn, the objects on top of it it was cut by, and its stitches.
type Entry = (Arc<Node>, Vec<Arc<Node>>, Arc<Sewn>);

impl StitchCache {
    /// `n`'s stitches: kept ones when `n` is the node they were sewn from and the same objects
    /// (`over`) lie on top of it, else sewn now.
    fn get(&mut self, n: &Arc<Node>, over: Vec<Arc<Node>>, next: &mut HashMap<NodeId, Entry>) -> Arc<Sewn> {
        let same = |a: &[Arc<Node>]| a.len() == over.len() && a.iter().zip(&over).all(|(p, q)| Arc::ptr_eq(p, q));
        let sewn = match self.entries.get(&n.id) {
            Some((node, was_over, sewn)) if Arc::ptr_eq(node, n) && same(was_over) => {
                self.reused += 1;
                sewn.clone()
            }
            _ => {
                self.sewn += 1;
                // A fabric this version can't read isn't used.
                let fabric = self.fabric.as_deref().filter(|f| f.unknown.is_none());
                let cuts = over.iter().filter_map(|c| cover_area(c)).collect();
                let mut b = Sewer {
                    patterns: &self.patterns,
                    fonts: &self.fonts,
                    fabric,
                    auto: fabrics::auto_underlay(fabric),
                    cuts,
                    overlap: self.overlap,
                    hand: penstrokes::Hand::Print,
                    out: Sewn::default(),
                };
                sew_node(n, &mut b);
                Arc::new(b.out)
            }
        };
        next.insert(n.id, (n.clone(), over, sewn.clone()));
        sewn
    }
}

/// Whether `n` covers what is under it when sewn on top (overlap removal): areas (fill, satin
/// shape, Auto) and satin columns along a path.
fn covers(n: &Node) -> bool {
    n.extras().stitch.as_deref().is_some_and(|s| {
        s.unknown.is_none()
            && (matches!(s.kind, StitchKind::Fill | StitchKind::SatinShape | StitchKind::Auto) || (s.kind == StitchKind::Satin && !s.rails))
    })
}

/// For each of `items` (in sewing order) that is a fill whose covered rows are cut away, the
/// later items that may lie on top of it (their boxes meet).
fn coverers(items: &[&Arc<Node>]) -> Vec<Vec<usize>> {
    let fills: Vec<bool> = items
        .iter()
        .map(|n| n.extras().stitch.as_deref().is_some_and(|s| s.unknown.is_none() && s.kind == StitchKind::Fill && !s.keep_covered))
        .collect();
    if !fills.iter().any(|f| *f) {
        return vec![];
    }
    let boxes: Vec<Option<vectorcraft_geom::Rect>> = items.iter().map(|n| n.visual_bounds()).collect();
    let can: Vec<bool> = items.iter().map(|n| covers(n)).collect();
    let meet = |a: &vectorcraft_geom::Rect, b: &vectorcraft_geom::Rect| a.x0 < b.x1 && b.x0 < a.x1 && a.y0 < b.y1 && b.y0 < a.y1;
    (0..items.len())
        .map(|i| {
            if fills.get(i) != Some(&true) {
                return vec![];
            }
            let mine = boxes.get(i).copied().flatten();
            (i + 1..items.len())
                .filter(|j| can.get(*j) == Some(&true))
                .filter(|j| match (mine, boxes.get(*j).copied().flatten()) {
                    (Some(a), Some(b)) => meet(&a, &b),
                    // A box unknown: it may.
                    _ => true,
                })
                .collect()
        })
        .collect()
}

/// The area `n` covers when sewn on top of a fill (see [`covers`]): areas by their outlines (type:
/// its letters), satin columns by their band.
fn cover_area(n: &Node) -> Option<generate::Cut> {
    let s = n.extras().stitch.as_deref()?;
    if !covers(n) {
        return None;
    }
    let area = s.kind != StitchKind::Satin;
    let (paths, rule): (Vec<PathData>, FillRule) = match &n.kind {
        NodeKind::Path { path, rule, .. } => (vec![path.clone()], *rule),
        NodeKind::Compound { children, rule } => (children.iter().filter_map(|c| c.path_data().cloned()).collect(), *rule),
        NodeKind::Text(t) if area => (glyph_parts(t).into_iter().map(|(_, p)| p).collect(), FillRule::NonZero),
        _ => return None,
    };
    let lines: Vec<(Vec<Point>, bool)> = paths.iter().flat_map(generate::polylines).collect();
    if area {
        let polys: Vec<Vec<Point>> = lines.into_iter().map(|(l, _)| l).filter(|l| l.len() >= 3).collect();
        return (!polys.is_empty()).then_some(generate::Cut { polys, rule });
    }
    // A satin column: a quad either side of each stretch of its path, all turning the same way.
    let half = s.width(n.appearance.stroke_width() * PT_MM) / 2.0;
    let mut quads = vec![];
    for (l, _) in &lines {
        for w in l.windows(2) {
            let (a, b) = (w[0], w[1]);
            let d = b - a;
            let len = d.hypot();
            if len <= 1e-9 || !len.is_finite() {
                continue;
            }
            let side = kurbo::Vec2::new(-d.y, d.x) * (half / len);
            quads.push(vec![a + side, b + side, b - side, a - side]);
        }
    }
    (!quads.is_empty()).then_some(generate::Cut { polys: quads, rule: FillRule::NonZero })
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
