//! Motif extraction (E8.4c): the tile of a pattern that repeats both ways across a stitched area
//! (an opened design's fill or background), found and cut out as a stitched [`Motif`].
//!
//! The repeat is a lattice: two shifts that each move the stitching onto itself. Shifts are
//! proposed by how often one needle point sits that far from another (an autocorrelation of the
//! needle points), scored by how closely the needle points, moved by the shift, land on needle points,
//! and refined to [`REFINE_STEP`]. The two shortest independent shifts that fit are the lattice,
//! reduced to its shortest basis. One tile is the stitches whose middles fall in one cell of the
//! lattice: their copies at every lattice point give back the whole pattern, each stitch once, so
//! neighbouring tiles meet without gaps or doubled stitches.

use std::collections::HashMap;

use kurbo::{Point, Vec2};
use vectorcraft_doc::{Motif, MotifKind, PartStitch, PatternPart};
use vectorcraft_geom::FillRule;

use crate::generate;

/// The most needle points looked at.
const MAX_POINTS: usize = 50_000;
/// Needle points farther than this from the origin (mm) are left out.
const MAX_COORD: f64 = 100_000.0;
/// The shortest and longest repeat shifts sought, mm.
const MIN_SHIFT: f64 = 1.0;
const MAX_SHIFT: f64 = 60.0;
/// The needle points the shifts are proposed from and scored on (evenly spaced).
const SAMPLE: usize = 400;
/// The size of a bin of the autocorrelation, mm.
const BIN: f64 = 0.5;
/// Shifts proposed for scoring, most often seen first.
const CANDIDATES: usize = 80;
/// How far (mm, the median) the shifted line may sit from itself for a shift to be a repeat.
const FIT_MM: f64 = 0.2;
/// The step shifts are refined at, mm.
const REFINE_STEP: f64 = 0.05;
/// Two shifts closer to one line than this (the sine of the angle between them) aren't a lattice.
const MIN_SINE: f64 = 0.25;

/// A motif found in an area of stitches, and how well it fits.
#[derive(Clone, Debug)]
pub struct ExtractedMotif {
    pub motif: Motif,
    /// Needle points in one tile.
    pub stitches: usize,
    /// How far, on average, the stitching shifted by either offset lies from itself, mm.
    pub error: f64,
    /// Whole tiles the area holds (roughly: its area over a cell's).
    pub tiles: usize,
}

/// The needle points in cells, to find how far a point is from the nearest one. A repeat moves
/// needle points onto needle points: measuring to the stitched line instead would let a short
/// shift along a straight stitch pass for one.
struct Lines {
    cells: HashMap<(i64, i64), Vec<Point>>,
}

impl Lines {
    const CELL: f64 = 1.0;
    /// Farther than this counts as this far.
    const CAP: f64 = 1.5;

    fn new(runs: &[Vec<Point>]) -> Self {
        let mut cells: HashMap<(i64, i64), Vec<Point>> = HashMap::new();
        for &p in runs.iter().flatten() {
            cells.entry(Self::key(p)).or_default().push(p);
        }
        Self { cells }
    }

    fn key(p: Point) -> (i64, i64) {
        ((p.x / Self::CELL).floor() as i64, (p.y / Self::CELL).floor() as i64)
    }

    fn distance(&self, p: Point) -> f64 {
        let (cx, cy) = Self::key(p);
        let mut best = Self::CAP;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for q in self.cells.get(&(cx.saturating_add(dx), cy.saturating_add(dy))).into_iter().flatten() {
                    best = best.min(p.distance(*q));
                }
            }
        }
        best
    }
}

/// How closely the stitching, moved by `v`, lies on itself: the median distance of the sample
/// points moved by `v` from the nearest needle point, over those that land in the area (`inside`). None when too
/// few do (the shift is too long for the area).
fn fit(lines: &Lines, sample: &[Point], v: Vec2, inside: &dyn Fn(Point) -> bool) -> Option<f64> {
    let mut d: Vec<f64> = sample.iter().map(|p| *p + v).filter(|q| inside(*q)).map(|q| lines.distance(q)).collect();
    if d.len() < 8 || d.len() * 3 < sample.len() {
        return None;
    }
    d.sort_by(f64::total_cmp);
    d.get(d.len() / 2).copied()
}

/// `v` moved to the nearby shift that fits best (within ±0.3 mm, in [`REFINE_STEP`] steps).
fn refine(lines: &Lines, sample: &[Point], v: Vec2, inside: &dyn Fn(Point) -> bool) -> Option<(Vec2, f64)> {
    let mut best: Option<(Vec2, f64)> = None;
    let n = (0.3 / REFINE_STEP).round() as i32;
    for i in -n..=n {
        for j in -n..=n {
            let w = v + Vec2::new(f64::from(i) * REFINE_STEP, f64::from(j) * REFINE_STEP);
            if let Some(e) = fit(lines, sample, w, inside)
                && best.is_none_or(|(_, b)| e < b)
            {
                best = Some((w, e));
            }
        }
    }
    best
}

/// One of `v` and `-v`, the same each time (x positive, else y positive).
fn canonical(v: Vec2) -> Vec2 {
    if v.x > 1e-9 || (v.x.abs() <= 1e-9 && v.y > 0.0) { v } else { -v }
}

/// The lattice `a`, `b` reduced to its shortest basis (Lagrange–Gauss).
fn reduce(mut a: Vec2, mut b: Vec2) -> (Vec2, Vec2) {
    for _ in 0..64 {
        if b.hypot2() < a.hypot2() {
            std::mem::swap(&mut a, &mut b);
        }
        let m = (a.dot(b) / a.hypot2()).round();
        if m == 0.0 {
            break;
        }
        b -= a * m;
    }
    if b.hypot2() < a.hypot2() {
        std::mem::swap(&mut a, &mut b);
    }
    (canonical(a), canonical(b))
}

/// The shifts that recur among `pts` (around `sample`'s points), most often first.
fn proposals(pts: &[Point], sample: &[Point]) -> Vec<Vec2> {
    let cell = MAX_SHIFT;
    let key = |p: Point| ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64);
    let mut grid: HashMap<(i64, i64), Vec<Point>> = HashMap::new();
    for &p in pts {
        grid.entry(key(p)).or_default().push(p);
    }
    let bin = |v: Vec2| ((v.x / BIN).round() as i64, (v.y / BIN).round() as i64);
    let mut hist: HashMap<(i64, i64), usize> = HashMap::new();
    for &p in sample {
        let (cx, cy) = key(p);
        // Each sample point counts a shift once, however many of its neighbours share it.
        let mut seen = std::collections::HashSet::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &q in grid.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                    let v = canonical(q - p);
                    let l = v.hypot();
                    if (MIN_SHIFT..=MAX_SHIFT).contains(&l) && seen.insert(bin(v)) {
                        *hist.entry(bin(v)).or_default() += 1;
                    }
                }
            }
        }
    }
    // Bins with their neighbours (a shift rounding across a bin edge counts once in each).
    let mut scored: Vec<((i64, i64), usize)> = hist
        .keys()
        .map(|&(i, j)| {
            let near: usize =
                (-1..=1).flat_map(|a| (-1..=1).map(move |b| (a, b))).map(|(a, b)| hist.get(&(i + a, j + b)).copied().unwrap_or(0)).sum();
            ((i, j), near)
        })
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let least = sample.len() / 3;
    let mut out: Vec<Vec2> = vec![];
    for ((i, j), n) in scored {
        if n < least || out.len() >= CANDIDATES {
            break;
        }
        let v = Vec2::new(i as f64 * BIN, j as f64 * BIN);
        if out.iter().all(|w| (*w - v).hypot() > 2.0 * BIN) {
            out.push(v);
        }
    }
    out
}

/// The motif repeating across `runs` (stitched lines in sewing order, mm) inside `region` (closed
/// outlines, mm; none: all of them), named `name`. `offsets` (mm) skips the search for the
/// lattice. Errors say why nothing was found (too few stitches, no repeat both ways).
pub fn motif(runs: &[Vec<Point>], region: Option<&[Vec<Point>]>, name: &str, offsets: Option<[Vec2; 2]>) -> Result<ExtractedMotif, String> {
    let keep = |p: &Point| {
        p.is_finite() && p.x.abs() <= MAX_COORD && p.y.abs() <= MAX_COORD && region.is_none_or(|r| generate::inside(r, FillRule::NonZero, *p))
    };
    // The stretches of each run inside the region.
    let mut lines: Vec<Vec<Point>> = vec![];
    let mut count = 0usize;
    for run in runs {
        let mut cur: Vec<Point> = vec![];
        for p in run {
            if keep(p) && count < MAX_POINTS {
                cur.push(*p);
                count += 1;
            } else if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
            }
        }
        if !cur.is_empty() {
            lines.push(cur);
        }
    }
    let pts: Vec<Point> = lines.iter().flatten().copied().collect();
    if pts.len() < 12 {
        return Err("select an area of stitches with at least two repeats each way".into());
    }
    let (mut lo, mut hi) = (Point::new(f64::INFINITY, f64::INFINITY), Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
    for p in &pts {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    // Points landing over the stitched area (its box, or the region given).
    let inside = |q: Point| match region {
        Some(r) => generate::inside(r, FillRule::NonZero, q),
        None => q.x >= lo.x && q.x <= hi.x && q.y >= lo.y && q.y <= hi.y,
    };
    let field = Lines::new(&lines);
    let sample: Vec<Point> = pts.iter().step_by((pts.len() / SAMPLE).max(1)).copied().collect();
    let (a, b, error) = match offsets {
        Some([a, b]) => {
            let (fa, fb) = (fit(&field, &sample, a, &inside), fit(&field, &sample, b, &inside));
            if !(a.hypot() >= MIN_SHIFT && b.hypot() >= MIN_SHIFT && (a.cross(b) / (a.hypot() * b.hypot())).abs() >= MIN_SINE) {
                return Err(format!("the two offsets must each be at least {MIN_SHIFT} mm and not along one line"));
            }
            let (a, b) = reduce(a, b);
            (a, b, (fa.unwrap_or(Lines::CAP) + fb.unwrap_or(Lines::CAP)) / 2.0)
        }
        None => {
            let mut good: Vec<(Vec2, f64)> = proposals(&pts, &sample)
                .into_iter()
                .filter_map(|v| refine(&field, &sample, v, &inside))
                .filter(|(_, e)| *e <= FIT_MM)
                .map(|(v, e)| (canonical(v), e))
                .collect();
            good.sort_by(|x, y| x.0.hypot().total_cmp(&y.0.hypot()));
            let Some(&(a, ea)) = good.first() else {
                return Err("no repeat found in these stitches: select an area of a repeating pattern with at least two repeats each way \
                            (or give the offsets)"
                    .into());
            };
            let Some(&(b, eb)) = good.iter().find(|(v, _)| (a.cross(*v) / (a.hypot() * v.hypot())).abs() >= MIN_SINE) else {
                return Err("the stitches repeat only one way: this looks like a border (Extract Border Pattern), or select more rows".into());
            };
            let (a, b) = reduce(a, b);
            (a, b, (ea + eb) / 2.0)
        }
    };
    // One tile: the stitches whose middles fall in the cell at the area's middle.
    let o = lo.midpoint(hi);
    let det = a.cross(b);
    let cell_of = |p: Point| {
        let d = p - o;
        (d.cross(b) / det, a.cross(d) / det)
    };
    let in_cell = |p: Point, q: Point| {
        let (s, t) = cell_of(p.midpoint(q));
        (0.0..1.0).contains(&s) && (0.0..1.0).contains(&t)
    };
    let mut parts: Vec<PatternPart> = vec![];
    for line in &lines {
        let mut cur: Vec<[f64; 2]> = vec![];
        for w in line.windows(2) {
            if in_cell(w[0], w[1]) {
                if cur.is_empty() {
                    cur.push([w[0].x - o.x, w[0].y - o.y]);
                }
                cur.push([w[1].x - o.x, w[1].y - o.y]);
            } else if !cur.is_empty() {
                parts.push(PatternPart { stitch: PartStitch::Manual, points: std::mem::take(&mut cur), ..Default::default() });
            }
        }
        if cur.len() >= 2 {
            parts.push(PatternPart { stitch: PartStitch::Manual, points: cur, ..Default::default() });
        }
    }
    if parts.is_empty() {
        return Err("no stitches fall in one tile of the repeat found: select a larger area".into());
    }
    let stitches = parts.iter().map(|p| p.points.len()).sum();
    let r = |v: f64| (v * 1000.0).round() / 1000.0;
    let name = if name.trim().is_empty() { "Extracted motif".to_string() } else { name.trim().to_string() };
    let motif = Motif {
        id: crate::borders::slug(&name),
        name,
        kind: MotifKind::Stitched,
        size: r(a.hypot().max(b.hypot())),
        offsets: [[r(a.x), r(a.y)], [r(b.x), r(b.y)]],
        parts,
        ..Motif::default()
    };
    crate::motifs::check(&motif).map_err(|e| format!("what repeats here isn't a usable motif: {e}"))?;
    let area = (hi.x - lo.x) * (hi.y - lo.y);
    let tiles = if det.abs() > 0.0 { (area / det.abs()).floor() as usize } else { 0 };
    Ok(ExtractedMotif { motif, stitches, error, tiles })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A design sewn from small "x" motifs on lattice `a`, `b` (mm), `n` × `n` of them, each a
    /// run of its own, rounded to DST's 0.1 mm.
    fn design(a: Vec2, b: Vec2, n: i32) -> Vec<Vec<Point>> {
        let x = [(-1.0, -1.0), (1.0, 1.0), (0.0, 0.0), (1.0, -1.0), (-1.0, 1.0)];
        let mut runs = vec![];
        for i in 0..n {
            for j in 0..n {
                let o = Point::new(100.0, 100.0) + a * f64::from(i) + b * f64::from(j);
                runs.push(x.iter().map(|&(dx, dy)| Point::new(((o.x + dx) * 10.0).round() / 10.0, ((o.y + dy) * 10.0).round() / 10.0)).collect());
            }
        }
        runs
    }

    /// Whether lattices `(a, b)` and `(c, d)` are the same: each basis vector of one a whole-number
    /// combination of the other's, to within `tol` mm.
    fn same_lattice((a, b): (Vec2, Vec2), (c, d): (Vec2, Vec2), tol: f64) -> bool {
        let whole = |v: Vec2, (p, q): (Vec2, Vec2)| {
            let det = p.cross(q);
            let (s, t) = (v.cross(q) / det, p.cross(v) / det);
            let w = p * s.round() + q * t.round();
            (w - v).hypot() <= tol && s.round().abs() + t.round().abs() > 0.0
        };
        whole(c, (a, b)) && whole(d, (a, b)) && whole(a, (c, d)) && whole(b, (c, d))
    }

    #[test]
    fn square_brick_and_hexagonal_lattices_are_found() {
        for (a, b) in [
            (Vec2::new(8.0, 0.0), Vec2::new(0.0, 8.0)),
            (Vec2::new(7.0, 0.0), Vec2::new(3.5, 6.0)),
            (Vec2::new(6.0, 0.0), Vec2::new(3.0, 5.196)),
            (Vec2::new(6.0, 2.0), Vec2::new(-2.0, 7.0)),
        ] {
            let e = motif(&design(a, b, 6), None, "Crosses", None).unwrap();
            let [[ax, ay], [bx, by]] = e.motif.offsets;
            let found = (Vec2::new(ax, ay), Vec2::new(bx, by));
            assert!(same_lattice(found, (a, b), 0.1), "{a:?} {b:?}: found {found:?}");
            assert!(e.error < 0.1, "{}", e.error);
            assert_eq!(e.motif.kind, MotifKind::Stitched);
            // One tile holds one cross's stitches (its five needle points).
            assert!(e.stitches >= 4 && e.stitches <= 8, "{}", e.stitches);
            crate::motifs::check(&e.motif).unwrap();
        }
    }

    #[test]
    fn the_tile_tiles_back_into_the_pattern() {
        let (a, b) = (Vec2::new(7.0, 0.0), Vec2::new(3.5, 6.0));
        let runs = design(a, b, 6);
        let e = motif(&runs, None, "Crosses", None).unwrap();
        // Laid on its own lattice, the tile's stitches fall on the design's.
        let lines = Lines::new(&runs);
        let [[ax, ay], [bx, by]] = e.motif.offsets;
        let (fa, fb) = (Vec2::new(ax, ay), Vec2::new(bx, by));
        // The tile's origin is the middle of the stitched area.
        let (mut lo, mut hi) = (Point::new(f64::INFINITY, f64::INFINITY), Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
        for p in runs.iter().flatten() {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        let o = lo.midpoint(hi);
        for (i, j) in [(0, 0), (1, 0), (0, 1), (-1, 1)] {
            for part in &e.motif.parts {
                for &[x, y] in &part.points {
                    let p = o + fa * f64::from(i) + fb * f64::from(j) + Vec2::new(x, y);
                    assert!(lines.distance(p) < 0.2, "{p:?}");
                }
            }
        }
    }

    #[test]
    fn crosses_joined_into_one_run_are_found() {
        // As a machine file sews them: one run, the crosses joined by travel stitches. A short
        // shift along a cross's arm keeps the line on itself but not the needle points.
        let (a, b) = (Vec2::new(7.0, 0.0), Vec2::new(3.5, 6.0));
        let run: Vec<Point> = design(a, b, 6).into_iter().flatten().collect();
        let e = motif(&[run], None, "Crosses", None).unwrap();
        let [[ax, ay], [bx, by]] = e.motif.offsets;
        assert!(same_lattice((Vec2::new(ax, ay), Vec2::new(bx, by)), (a, b), 0.1), "{:?}", e.motif.offsets);
        assert!(e.stitches >= 4, "{}", e.stitches);
    }

    #[test]
    fn a_border_one_way_or_scattered_stitches_are_refused() {
        // A row of crosses: repeats only one way.
        let row: Vec<Vec<Point>> = design(Vec2::new(7.0, 0.0), Vec2::new(0.0, 1000.0), 6).into_iter().step_by(6).collect();
        let r = motif(&row, None, "", None);
        assert!(r.is_err(), "{:?}", r.map(|e| e.motif.offsets));
        assert!(motif(&[vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)]], None, "", None).is_err());
        // Bad offsets given.
        let d = design(Vec2::new(8.0, 0.0), Vec2::new(0.0, 8.0), 4);
        assert!(motif(&d, None, "", Some([Vec2::new(8.0, 0.0), Vec2::new(16.0, 0.0)])).is_err());
        // Given offsets skip the search.
        let e = motif(&d, None, "", Some([Vec2::new(8.0, 0.0), Vec2::new(0.0, 8.0)])).unwrap();
        assert!(e.error < 0.1);
    }

    proptest::proptest! {
        #[test]
        fn any_stitches_extract_or_refuse_without_panicking(pts in proptest::collection::vec((-50.0f64..50.0, -50.0f64..50.0), 0..200)) {
            let run: Vec<Point> = pts.into_iter().map(|(x, y)| Point::new(x, y)).collect();
            let _ = motif(&[run], None, "x", None);
        }
    }
}
