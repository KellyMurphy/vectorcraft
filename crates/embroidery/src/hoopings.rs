//! Hoopings: a design too big for its hoop, split into a grid of areas, each sewn as a design of its
//! own after the fabric is hooped again ([`vectorcraft_doc::EmbroiderySetup::hoopings`],
//! `embroidery.splitHoops`). Each stitch belongs to the area its middle lies in; a hooping also sews
//! the stitches up to `overlap` mm past its seams, so the pieces meet without a gap. A fill's rows are
//! cut where they cross a seam, a satin column between its stitches (square to the column), and a cut
//! run is locked on both sides. Running-stitch crosses on each seam line the fabric up: the hooping
//! sewn first sews them last, the next one sews them first.

use kurbo::Point;
use vectorcraft_doc::{Hoop, Hoopings};

use crate::{Cmd, Needle, Plan, PlanStop};

/// Room kept inside the hoop's field round each area, past the overlap, mm: half the longest stitch
/// (a stitch kept by its middle reaches that far past it), which also holds the alignment marks.
pub const MARGIN: f64 = 6.0;
/// An alignment mark's arms reach this far from its middle, mm.
pub const MARK_ARM: f64 = 4.0;
/// The running stitches of the marks, mm.
const MARK_STITCH: f64 = 2.0;
/// How far a mark sits in from each end of its seam, mm (at most a quarter of the seam).
const MARK_INSET: f64 = 15.0;
/// The basting box lies this far inside the hoop's field, mm.
pub const BASTE_INSET: f64 = 3.0;
/// The basting box's stitches, mm (long: it is picked out afterwards).
const BASTE_STITCH: f64 = 5.0;
/// The lock stitches where a run is cut at a seam, mm.
const TIE: f64 = 0.6;
/// The smallest area a hooping can have, mm.
const MIN_AREA: f64 = 10.0;

/// The field a hooping can use, w × h mm, as it lies on the design (turned with it, `rotation`
/// degrees clockwise): a round hoop's largest square.
pub fn usable_field(hoop: &Hoop, rotation: u16) -> (f64, f64) {
    let (w, h) = crate::hoops::size(hoop);
    let (w, h) = if hoop.round {
        let s = w.min(h) / std::f64::consts::SQRT_2;
        (s, s)
    } else {
        (w, h)
    };
    crate::hoops::as_sewn(w, h, rotation)
}

/// The overlap to sew with, mm (in range).
pub fn overlap(h: &Hoopings) -> f64 {
    if h.overlap.is_finite() { h.overlap.clamp(*Hoopings::OVERLAP.start(), *Hoopings::OVERLAP.end()) } else { Hoopings::DEFAULT_OVERLAP }
}

/// A grid of equal areas over `bounds` (mm, `[minX, minY, maxX, maxY]`), each small enough that it,
/// `overlap` mm past each side and the marks' margin fit a hoop field `field` (mm, as it lies on
/// the design). Marks on, no basting box.
pub fn grid(bounds: [f64; 4], field: (f64, f64), overlap: f64) -> Result<Hoopings, String> {
    if !bounds.iter().chain([&field.0, &field.1]).all(|v| v.is_finite()) || bounds[2] < bounds[0] || bounds[3] < bounds[1] {
        return Err("the design has no size to split".into());
    }
    let overlap = if overlap.is_finite() { overlap.clamp(*Hoopings::OVERLAP.start(), *Hoopings::OVERLAP.end()) } else { Hoopings::DEFAULT_OVERLAP };
    let room = 2.0 * (overlap + MARGIN);
    let (cw, ch) = (field.0 - room, field.1 - room);
    if cw < MIN_AREA || ch < MIN_AREA {
        return Err(format!("the hoop's field ({:.0} × {:.0} mm) is too small to split with a {overlap} mm overlap", field.0, field.1));
    }
    let edges = |lo: f64, hi: f64, cell: f64| -> Result<Vec<f64>, String> {
        let span = hi - lo;
        let n = (span / cell).ceil().max(1.0);
        if n > Hoopings::MAX_SIDE as f64 {
            return Err(format!(
                "the design ({:.0} × {:.0} mm) needs more than {} hoopings across or down: choose a larger hoop",
                bounds[2] - bounds[0],
                bounds[3] - bounds[1],
                Hoopings::MAX_SIDE
            ));
        }
        let n = n as usize;
        Ok((0..=n).map(|i| lo + span * i as f64 / n as f64).collect())
    };
    Ok(Hoopings { columns: edges(bounds[0], bounds[2], cw)?, rows: edges(bounds[1], bounds[3], ch)?, overlap, ..Hoopings::default() })
}

/// A valid grid's edges: 2 to [`Hoopings::MAX_SIDE`] + 1 finite, increasing values each way.
fn edges(h: &Hoopings) -> Option<(&[f64], &[f64])> {
    let ok = |e: &[f64]| (2..=Hoopings::MAX_SIDE + 1).contains(&e.len()) && e.iter().all(|v| v.is_finite()) && e.windows(2).all(|w| w[0] < w[1]);
    (h.unknown.is_none() && ok(&h.columns) && ok(&h.rows)).then_some((h.columns.as_slice(), h.rows.as_slice()))
}

/// The grid's areas in sewing order (row by row from the top, each row from the left), mm:
/// `[minX, minY, maxX, maxY]`. `None` when `h` isn't a grid this version can use.
pub fn areas(h: &Hoopings) -> Option<Vec<[f64; 4]>> {
    let (cols, rows) = edges(h)?;
    Some(rows.windows(2).flat_map(|r| cols.windows(2).map(move |c| [c[0], r[0], c[1], r[1]])).collect())
}

/// Which of `edges`' spans `v` lies in: past the outer edges, the outer spans.
fn span(edges: &[f64], v: f64) -> usize {
    edges.get(1..edges.len().saturating_sub(1)).map_or(0, |inner| inner.iter().filter(|e| v >= **e).count())
}

/// Whether `v` lies in span `i` of `edges` or within `reach` of it (the outer spans reach on
/// without end).
fn near(edges: &[f64], i: usize, v: f64, reach: f64) -> bool {
    let last = edges.len().saturating_sub(2);
    let lo = edges.get(i).copied().unwrap_or(f64::NEG_INFINITY);
    let hi = edges.get(i + 1).copied().unwrap_or(f64::INFINITY);
    (i == 0 || v >= lo - reach) && (i == last || v < hi + reach)
}

/// An alignment mark: a running-stitch cross on a seam, at `at` (mm), sewn last by hooping `first`
/// and first by hooping `next` (indexes in sewing order).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub at: Point,
    pub first: usize,
    pub next: usize,
}

/// The marks on the seams of `h`'s grid, two on each seam between areas `sewn` (by index; marks to
/// an area that isn't sewn are left out).
pub fn marks(h: &Hoopings, sewn: &[bool]) -> Vec<Mark> {
    let Some((cols, rows)) = edges(h) else { return vec![] };
    let (nc, nr) = (cols.len() - 1, rows.len() - 1);
    let on = |k: usize| sewn.get(k).copied().unwrap_or(false);
    let pair = |a: f64, b: f64| {
        let inset = MARK_INSET.min((b - a) / 4.0);
        [a + inset, b - inset]
    };
    let mut out = vec![];
    for r in 0..nr {
        for c in 0..nc {
            let k = r * nc + c;
            let (Some(&x0), Some(&x1), Some(&y0), Some(&y1)) = (cols.get(c), cols.get(c + 1), rows.get(r), rows.get(r + 1)) else { continue };
            // The seam on the right, then the one below.
            if c + 1 < nc && on(k) && on(k + 1) {
                out.extend(pair(y0, y1).map(|y| Mark { at: Point::new(x1, y), first: k, next: k + 1 }));
            }
            if r + 1 < nr && on(k) && on(k + nc) {
                out.extend(pair(x0, x1).map(|x| Mark { at: Point::new(x, y1), first: k, next: k + nc }));
            }
        }
    }
    out
}

/// One hooping's design.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    /// Its place in the grid's sewing order ([`areas`]).
    pub index: usize,
    /// Its area, mm (`[minX, minY, maxX, maxY]`); the hoop's field is centred on it.
    pub area: [f64; 4],
    /// What it sews, in the design's own place (mm, not yet turned for the hoop).
    pub plan: Plan,
    /// The design's stitches whose middle lies in its area (each stitch is in exactly one piece's).
    pub own: usize,
}

/// A plan written move by move, with lock stitches where a run is cut.
struct Out {
    plan: Plan,
    at: Option<Point>,
    sewing: bool,
    /// The source's colour block being sewn.
    block: Option<usize>,
    /// Source block → this plan's block.
    blocks: Vec<(usize, usize)>,
    trim_over: Option<f64>,
}

impl Out {
    fn push(&mut self, p: Point, cmd: Cmd) {
        self.plan.needles.push(Needle { x: p.x, y: p.y, cmd });
    }

    /// Sew in the source's block `b` from here on (a colour change when it isn't the one being sewn).
    fn thread(&mut self, b: usize, color: [u8; 3]) {
        if self.block == Some(b) {
            return;
        }
        if self.block.is_some() {
            self.trim();
            let at = self.at.unwrap_or_default();
            self.push(at, Cmd::ColorChange);
        }
        self.blocks.push((b, self.plan.threads.len()));
        self.plan.threads.push(color);
        self.block = Some(b);
    }

    fn trim(&mut self) {
        if self.sewing
            && let Some(at) = self.at
        {
            self.push(at, Cmd::Trim);
            self.sewing = false;
        }
    }

    /// Move to `p` without sewing (cutting the thread first when the jump is long).
    fn jump(&mut self, p: Point) {
        if self.at == Some(p) {
            return;
        }
        if let Some(a) = self.at
            && self.trim_over.is_some_and(|t| a.distance(p) > t)
        {
            self.trim();
        }
        self.push(p, Cmd::Jump);
        self.at = Some(p);
        self.sewing = false;
    }

    fn stitch(&mut self, p: Point) {
        self.push(p, Cmd::Stitch);
        self.at = Some(p);
        self.sewing = true;
    }

    /// Lock the thread at `at`: a short stitch toward `toward` and back.
    fn lock(&mut self, at: Point, toward: Point) {
        let d = at.distance(toward);
        if d > 0.05 && d.is_finite() {
            self.stitch(at.lerp(toward, (TIE / d).min(0.5)));
            self.stitch(at);
        }
    }

    /// Sew the line through `pts` in running stitches up to `len` long, locked at both ends, then
    /// cut the thread.
    fn line(&mut self, pts: &[Point], len: f64) {
        let (Some(&first), Some(&second)) = (pts.first(), pts.get(1)) else { return };
        self.jump(first);
        self.stitch(first);
        self.lock(first, second);
        for w in pts.windows(2) {
            let n = (w[0].distance(w[1]) / len).ceil().clamp(1.0, 1000.0) as usize;
            for i in 1..=n {
                self.stitch(w[0].lerp(w[1], i as f64 / n as f64));
            }
        }
        if let [.., before, last] = pts {
            self.lock(*last, *before);
        }
        self.trim();
    }

    fn cross(&mut self, at: Point) {
        self.line(&[Point::new(at.x - MARK_ARM, at.y), Point::new(at.x + MARK_ARM, at.y)], MARK_STITCH);
        self.line(&[Point::new(at.x, at.y - MARK_ARM), Point::new(at.x, at.y + MARK_ARM)], MARK_STITCH);
    }
}

/// `plan` split into `h`'s hoopings, in sewing order, each with the stitches whose middle lies in its
/// area or within the overlap past it, its basting box (`field`, mm, as the hoop lies on the design)
/// and its alignment marks. Areas with none of the design's stitches are left out. Empty when `h`
/// isn't a grid this version can use.
pub fn split(plan: &Plan, h: &Hoopings, field: (f64, f64), trim_over: Option<f64>) -> Vec<Piece> {
    let (Some((cols, rows)), Some(areas)) = (edges(h), areas(h)) else { return vec![] };
    let nc = cols.len() - 1;
    let reach = overlap(h);
    // Each stitch: its middle and colour block.
    let mut stitches: Vec<Option<(Point, usize)>> = Vec::with_capacity(plan.needles.len());
    let (mut prev, mut block) = (None::<Point>, 0usize);
    for n in &plan.needles {
        let p = Point::new(n.x, n.y);
        let finite = n.x.is_finite() && n.y.is_finite();
        stitches.push(match n.cmd {
            Cmd::Stitch if finite => Some((prev.unwrap_or(p).midpoint(p), block)),
            _ => None,
        });
        match n.cmd {
            Cmd::Stitch | Cmd::Jump if finite => prev = Some(p),
            Cmd::ColorChange => block += 1,
            _ => {}
        }
    }
    let owner = |m: Point| span(rows, m.y) * nc + span(cols, m.x);
    let mut own = vec![0usize; areas.len()];
    for (m, _) in stitches.iter().flatten() {
        if let Some(o) = own.get_mut(owner(*m)) {
            *o += 1;
        }
    }
    let sewn: Vec<bool> = own.iter().map(|n| *n > 0).collect();
    let marks = marks(h, &sewn);
    let color = |b: usize| plan.threads.get(b).copied().unwrap_or_else(|| crate::palette_color(b));
    let mut out = vec![];
    for (k, area) in areas.iter().enumerate() {
        if !sewn.get(k).copied().unwrap_or(false) {
            continue;
        }
        let (r, c) = (k / nc, k % nc);
        let keep = |m: Point| near(cols, c, m.x, reach) && near(rows, r, m.y, reach);
        let first_block = stitches.iter().flatten().find(|(m, _)| keep(*m)).map_or(0, |(_, b)| *b);
        let mut o = Out { plan: Plan::default(), at: None, sewing: false, block: None, blocks: vec![], trim_over };
        o.thread(first_block, color(first_block));
        if h.basting {
            let (cx, cy) = ((area[0] + area[2]) / 2.0, (area[1] + area[3]) / 2.0);
            let (hw, hh) = (field.0 / 2.0 - BASTE_INSET, field.1 / 2.0 - BASTE_INSET);
            let corners = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh), (-hw, -hh)].map(|(x, y)| Point::new(cx + x, cy + y));
            o.line(&corners, BASTE_STITCH);
        }
        if h.marks {
            for m in marks.iter().filter(|m| m.next == k) {
                o.cross(m.at);
            }
        }
        // The design's stitches here, cut where they leave the area and its overlap.
        // Where the needle was in the design, where the last stitch kept started, and whether the
        // design sews on from the needle before (so a run picked up or left here is cut).
        let mut from: Option<Point> = None;
        let mut kept_from: Option<Point> = None;
        let mut sewing_on = false;
        for (n, st) in plan.needles.iter().zip(&stitches) {
            let p = Point::new(n.x, n.y);
            match (n.cmd, st) {
                (Cmd::Stitch, Some((m, b))) => {
                    let start = from.unwrap_or(p);
                    if keep(*m) {
                        o.thread(*b, color(*b));
                        if o.at != Some(start) {
                            o.jump(start);
                            if sewing_on {
                                o.stitch(start);
                                o.lock(start, p);
                            }
                        }
                        o.stitch(p);
                        kept_from = Some(start);
                    } else if let Some(back) = kept_from.take() {
                        // Leaving the area: lock the end sewn so far.
                        if let Some(at) = o.at {
                            o.lock(at, back);
                        }
                    }
                    sewing_on = true;
                    from = Some(p);
                }
                (Cmd::Jump, _) if n.x.is_finite() && n.y.is_finite() => {
                    (sewing_on, kept_from) = (false, None);
                    from = Some(p);
                }
                (Cmd::Trim, _) => {
                    // Cut only where this hooping sewed up to it.
                    if kept_from.is_some() {
                        o.trim();
                    }
                    (sewing_on, kept_from) = (false, None);
                }
                (Cmd::ColorChange, _) => (sewing_on, kept_from) = (false, None),
                _ => {}
            }
        }
        if h.marks {
            for m in marks.iter().filter(|m| m.first == k) {
                o.cross(m.at);
            }
        }
        o.trim();
        if let Some(at) = o.at {
            o.push(at, Cmd::End);
        }
        o.plan.stops = plan
            .stops
            .iter()
            .filter_map(|s| o.blocks.iter().find(|(from, _)| *from == s.block).map(|(_, to)| PlanStop { block: *to, kind: s.kind.clone() }))
            .collect();
        out.push(Piece { index: k, area: *area, plan: o.plan, own: own.get(k).copied().unwrap_or(0) });
    }
    out
}

/// Whether `piece`'s file fits `hoop`: its stitches as the machine sews them (turned `rotation`
/// degrees clockwise), centred in the field.
pub fn fits(piece: &Piece, hoop: &Hoop, rotation: u16) -> bool {
    piece.plan.stats().bounds.is_none_or(|b| {
        let (w, h) = crate::hoops::as_sewn(b[2] - b[0], b[3] - b[1], rotation);
        crate::hoops::fit(w, h, hoop) == crate::hoops::Fit::Fits
    })
}

/// `doc`'s design `plan` split into its hoopings ([`split`]), with the hoop they are for. `None`
/// when the document isn't split, has no hoop, or either is from a newer VectorCraft.
pub fn of_document<'a>(doc: &'a vectorcraft_doc::Document, plan: &Plan) -> Option<(Vec<Piece>, &'a Hoop)> {
    let e = &doc.embroidery;
    let h = e.hoopings.as_deref()?;
    let hoop = e.hoop.as_deref().filter(|h| h.unknown.is_none())?;
    let pieces = split(plan, h, usable_field(hoop, e.hoop_rotation()), e.trim_over());
    (!pieces.is_empty()).then_some((pieces, hoop))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fill rows 300 × 100 mm (3 mm stitches, each row joined to the next as fills are), then a
    /// second thread: a short line near the middle.
    fn rows() -> Plan {
        let mut needles = vec![];
        for (i, y) in (0..=50).map(|i| (i, f64::from(i) * 2.0)) {
            let xs: Vec<f64> = (0..=100).map(|j| f64::from(j) * 3.0).collect();
            let xs: Vec<f64> = if i % 2 == 0 { xs } else { xs.into_iter().rev().collect() };
            needles.extend(xs.into_iter().map(|x| Needle { x, y, cmd: Cmd::Stitch }));
        }
        needles.push(Needle { x: 0.0, y: 100.0, cmd: Cmd::Trim });
        needles.push(Needle { x: 0.0, y: 100.0, cmd: Cmd::ColorChange });
        needles.push(Needle { x: 150.0, y: 20.0, cmd: Cmd::Jump });
        needles.extend((0..=20).map(|j| Needle { x: 150.0, y: 20.0 + f64::from(j) * 3.0, cmd: Cmd::Stitch }));
        needles.push(Needle { x: 150.0, y: 80.0, cmd: Cmd::End });
        Plan { threads: vec![[200, 0, 0], [0, 0, 200]], needles, ..Plan::default() }
    }

    fn five_by_seven() -> (f64, f64) {
        usable_field(&crate::hoops::builtin("5x7").unwrap(), 0)
    }

    fn bounds(p: &Plan) -> [f64; 4] {
        p.stats().bounds.unwrap()
    }

    #[test]
    fn a_wide_design_is_split_across_and_every_stitch_lands_in_one_hooping() {
        let plan = rows();
        let h = grid(bounds(&plan), five_by_seven(), 2.0).unwrap();
        // 130 mm less 2 × (2 + 6): 114 mm areas, three across; one row.
        assert_eq!((h.columns.len(), h.rows.len()), (4, 2));
        let pieces = split(&plan, &h, five_by_seven(), Some(3.0));
        assert_eq!(pieces.len(), 3);
        assert_eq!(pieces.iter().map(|p| p.own).sum::<usize>(), plan.stats().stitches, "each stitch is owned once");
        for p in &pieces {
            let b = bounds(&p.plan);
            let (w, hh) = five_by_seven();
            assert!(b[2] - b[0] <= w && b[3] - b[1] <= hh, "piece {} fits the hoop: {b:?}", p.index);
            // Its stitches reach past its area by the overlap and half a stitch, and the marks.
            let reach = 2.0 + MARGIN;
            assert!(b[0] >= p.area[0] - reach && b[2] <= p.area[2] + reach, "piece {}: {b:?} round {:?}", p.index, p.area);
            assert_eq!(p.plan.needles.last().map(|n| n.cmd), Some(Cmd::End));
        }
        // The left piece doesn't sew the middle line: one thread. The middle one sews both.
        assert_eq!(pieces[0].plan.threads, vec![[200, 0, 0]]);
        assert_eq!(pieces[1].plan.threads, plan.threads);
        assert_eq!(pieces[1].plan.stats().color_changes, 1);
    }

    #[test]
    fn neighbours_sew_the_same_marks() {
        let plan = rows();
        let h = grid(bounds(&plan), five_by_seven(), 2.0).unwrap();
        let ms = marks(&h, &[true, true, true]);
        assert_eq!(ms.len(), 4, "two on each of two seams");
        assert!(ms.iter().all(|m| m.next == m.first + 1 && (m.at.x - h.columns[m.next]).abs() < 1e-9));
        let pieces = split(&plan, &h, five_by_seven(), Some(3.0));
        let crosses = |p: &Plan| -> Vec<Point> {
            // A mark's middle is where its two arms' running stitches cross: a stitch point
            // MARK_ARM left and right of it.
            ms.iter()
                .map(|m| m.at)
                .filter(|at| p.needles.iter().any(|n| (n.x - (at.x - MARK_ARM)).abs() < 1e-9 && (n.y - at.y).abs() < 1e-9))
                .collect()
        };
        // The left piece sews the first seam's marks (last); the middle sews them (first) and the
        // second seam's (last); the right sews the second seam's.
        assert_eq!(crosses(&pieces[0].plan).len(), 2);
        assert_eq!(crosses(&pieces[1].plan).len(), 4);
        assert_eq!(crosses(&pieces[2].plan).len(), 2);
        let first_mark = ms[0].at;
        let mark = |p: &Plan| {
            p.needles
                .iter()
                .position(|n| n.cmd == Cmd::Stitch && (n.y - first_mark.y).abs() < 1e-9 && (n.x - (first_mark.x - MARK_ARM)).abs() < 1e-9)
                .unwrap()
        };
        let row = |p: &Plan, y: f64| p.needles.iter().position(|n| n.cmd == Cmd::Stitch && n.y == y).unwrap();
        let last_row = |p: &Plan| p.needles.iter().rposition(|n| n.cmd == Cmd::Stitch && n.y == 100.0).unwrap();
        assert!(mark(&pieces[0].plan) > last_row(&pieces[0].plan), "the first hooping sews it after its design");
        assert!(mark(&pieces[1].plan) < row(&pieces[1].plan, 0.0), "the next sews it before its design");
        let none = Hoopings { marks: false, ..h.clone() };
        assert!(crosses(&split(&plan, &none, five_by_seven(), None)[1].plan).is_empty());
    }

    #[test]
    fn a_basting_box_is_sewn_first_round_the_field() {
        let plan = rows();
        let h = Hoopings { basting: true, ..grid(bounds(&plan), five_by_seven(), 2.0).unwrap() };
        let field = five_by_seven();
        for p in split(&plan, &h, field, Some(3.0)) {
            let b = bounds(&p.plan);
            let (cx, cy) = ((p.area[0] + p.area[2]) / 2.0, (p.area[1] + p.area[3]) / 2.0);
            assert!((b[2] - b[0] - (field.0 - 2.0 * BASTE_INSET)).abs() < 1e-6, "{b:?}");
            assert!(((b[0] + b[2]) / 2.0 - cx).abs() < 1e-6 && ((b[1] + b[3]) / 2.0 - cy).abs() < 1e-6, "centred on its area");
            let first = p.plan.needles.iter().find(|n| n.cmd == Cmd::Stitch).unwrap();
            assert!((first.x - (cx - field.0 / 2.0 + BASTE_INSET)).abs() < 1e-6, "sewn first");
        }
    }

    #[test]
    fn rows_cut_at_a_seam_are_locked_and_meet_with_the_overlap() {
        let plan = rows();
        let h = grid(bounds(&plan), five_by_seven(), 2.0).unwrap();
        let seam = h.columns[1];
        let pieces = split(&plan, &h, five_by_seven(), Some(3.0));
        let reach = |p: &Plan, y: f64| -> (f64, f64) {
            let xs: Vec<f64> = p.needles.iter().filter(|n| n.cmd == Cmd::Stitch && (n.y - y).abs() < 1e-9).map(|n| n.x).collect();
            (xs.iter().copied().fold(f64::INFINITY, f64::min), xs.iter().copied().fold(f64::NEG_INFINITY, f64::max))
        };
        // Row y = 4 (away from the marks): the left piece sews past the seam, the middle one from
        // before it.
        let (left, middle) = (reach(&pieces[0].plan, 4.0), reach(&pieces[1].plan, 4.0));
        assert!(left.1 > seam && left.1 <= seam + 2.0 + 3.0, "{left:?} past {seam}");
        assert!(middle.0 < seam && middle.0 >= seam - 2.0 - 3.0, "{middle:?} before {seam}");
        // Each row is picked up and left again: lock stitches (a short stitch back) at the cut.
        let short = pieces[0]
            .plan
            .needles
            .windows(2)
            .filter(|w| {
                w[0].cmd == Cmd::Stitch && w[1].cmd == Cmd::Stitch && Point::new(w[0].x, w[0].y).distance(Point::new(w[1].x, w[1].y)) < TIE + 1e-6
            })
            .count();
        assert!(short >= 50, "{short} lock stitches");
    }

    #[test]
    fn grids_are_refused_when_they_cant_be_made() {
        let field = five_by_seven();
        assert!(grid([0.0, 0.0, 2000.0, 100.0], field, 2.0).unwrap_err().contains("hoopings across"), "too many");
        assert!(grid([0.0, 0.0, 100.0, 100.0], (25.0, 25.0), 2.0).is_err(), "a tiny field");
        assert!(grid([0.0, 0.0, f64::NAN, 100.0], field, 2.0).is_err());
        let h = grid([0.0, 0.0, 50.0, 50.0], field, f64::NAN).unwrap();
        assert_eq!((h.columns.len(), h.overlap), (2, Hoopings::DEFAULT_OVERLAP), "one area");
        // A round hoop's square; a turned hoop.
        let round = Hoop { width: 100.0, height: 100.0, round: true, ..Hoop::default() };
        assert!((usable_field(&round, 0).0 - 70.71).abs() < 0.01);
        assert_eq!(usable_field(&crate::hoops::builtin("5x7").unwrap(), 90), (180.0, 130.0));
    }

    #[test]
    fn grids_this_version_cant_use_split_nothing() {
        let plan = rows();
        for (columns, rows) in [
            (vec![0.0], vec![0.0, 100.0]),
            (vec![0.0, f64::NAN], vec![0.0, 100.0]),
            (vec![100.0, 0.0], vec![0.0, 100.0]),
            ((0..20).map(f64::from).collect(), vec![0.0, 100.0]),
        ] {
            let h = Hoopings { columns, rows, ..Hoopings::default() };
            assert!(areas(&h).is_none() && split(&plan, &h, five_by_seven(), None).is_empty());
        }
    }

    proptest::proptest! {
        #[test]
        fn any_grid_or_plan_never_panics(
            cols in proptest::collection::vec(-1e4f64..1e4, 0..12),
            rows in proptest::collection::vec(-1e4f64..1e4, 0..12),
            pts in proptest::collection::vec((-500f64..500.0, -500f64..500.0, 0u8..5), 0..300),
            overlap in -5f64..20.0,
        ) {
            let mut cols = cols;
            let mut rows = rows;
            cols.sort_by(f64::total_cmp);
            rows.sort_by(f64::total_cmp);
            let cmd = |c: u8| [Cmd::Stitch, Cmd::Jump, Cmd::Trim, Cmd::ColorChange, Cmd::End][usize::from(c) % 5];
            let plan = Plan { needles: pts.iter().map(|(x, y, c)| Needle { x: *x, y: *y, cmd: cmd(*c) }).collect(), ..Plan::default() };
            let h = Hoopings { columns: cols, rows, overlap, basting: true, ..Hoopings::default() };
            let pieces = split(&plan, &h, (130.0, 180.0), Some(3.0));
            if !pieces.is_empty() {
                proptest::prop_assert_eq!(pieces.iter().map(|p| p.own).sum::<usize>(), plan.stats().stitches);
            }
            if let Some(b) = plan.stats().bounds {
                let _ = grid(b, (130.0, 180.0), overlap).map(|g| split(&plan, &g, (130.0, 180.0), None));
            }
        }
    }
}
