//! Nesting for cut files (appliqué fabric pieces for a cutter): pieces packed onto mats without
//! overlapping, a set gap apart, as few mats as they fit on.
//!
//! The method is raster bottom-left placement, a standard approach in the nesting literature: each
//! mat is a grid of [`CELL`]-sized cells, each piece (in each orientation it may take) the cells its
//! outline touches or encloses, so a piece's real outline always lies inside its cells. Pieces go
//! largest first, each at the free place nearest the mat's top left (fewest rows down, then fewest
//! columns across) over every orientation and mat, opening a new mat when none has room. A piece
//! is grown by the gap in cells before it is tested against what is placed, so real outlines end
//! up at least the gap apart. Coarse but safe: a piece never overlaps another or the gap.

use kurbo::{Affine, Point};

/// The grid's cell, mm.
pub const CELL: f64 = 1.0;
/// The largest mat side, mm.
pub const MAX_MAT: f64 = 1500.0;
/// The most pieces nested at once.
pub const MAX_PIECES: usize = 500;
/// The most points of a piece's outline.
pub const MAX_POINTS: usize = 100_000;

/// How pieces may turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotate {
    /// As drawn (a fabric with a grain or a print that must run one way).
    None,
    /// In quarter turns.
    Quarter,
    /// Every 15°.
    Any,
}

impl Rotate {
    fn angles(self) -> Vec<f64> {
        match self {
            Rotate::None => vec![0.0],
            Rotate::Quarter => vec![0.0, 90.0, 180.0, 270.0],
            Rotate::Any => (0..24).map(|i| f64::from(i) * 15.0).collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// The mat's width and height, mm.
    pub mat: (f64, f64),
    /// Kept clear round the mat's edge, mm.
    pub margin: f64,
    /// The least distance between pieces, mm.
    pub gap: f64,
    pub rotate: Rotate,
    /// Pieces may be turned over (fabric with no right side).
    pub flip: bool,
}

/// Where a piece went: `transform` maps its outline (mm, as given) onto mat `mat` (mm from the mat's
/// top left).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub piece: usize,
    pub mat: usize,
    pub angle: f64,
    pub flipped: bool,
    pub transform: Affine,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Nested {
    pub placements: Vec<Placement>,
    /// Pieces too big for an empty mat (or with no area).
    pub unplaced: Vec<usize>,
    pub mats: usize,
}

/// A piece in one orientation: its cells, row by row, as bit words.
struct Shape {
    transform: Affine,
    angle: f64,
    flipped: bool,
    cols: usize,
    rows: usize,
    bits: Vec<Vec<u64>>,
    grown: Grown,
}

/// The shape grown by the gap: its cells and where they start, relative to the shape's.
struct Grown {
    cols: usize,
    by: usize,
    bits: Vec<Vec<u64>>,
}

/// A mat's taken cells.
struct Mat {
    cols: usize,
    rows: usize,
    bits: Vec<Vec<u64>>,
}

fn words(cols: usize) -> usize {
    cols.div_ceil(64)
}

fn set(row: &mut [u64], c: usize) {
    if let Some(w) = row.get_mut(c / 64) {
        *w |= 1u64 << (c % 64);
    }
}

fn get(row: &[u64], c: usize) -> bool {
    row.get(c / 64).is_some_and(|w| w & (1u64 << (c % 64)) != 0)
}

/// Bits `row` of a shape, moved `x` columns right: the word `i` of the moved row.
fn shifted(row: &[u64], x: usize, i: usize) -> u64 {
    let (q, r) = (x / 64, x % 64);
    let lo = i.checked_sub(q).and_then(|j| row.get(j)).copied().unwrap_or(0);
    if r == 0 {
        return lo;
    }
    let hi = i.checked_sub(q + 1).and_then(|j| row.get(j)).copied().unwrap_or(0);
    (lo << r) | (hi >> (64 - r))
}

/// The cells `rings` (closed outlines, mm, already in grid position: none below zero) touch or
/// enclose, on a grid `cols` × `rows`.
fn raster(rings: &[Vec<Point>], cols: usize, rows: usize) -> Vec<Vec<u64>> {
    let mut bits = vec![vec![0u64; words(cols)]; rows];
    let cell = |p: Point| ((p.x / CELL).floor().max(0.0) as usize, (p.y / CELL).floor().max(0.0) as usize);
    // The edges' cells: walked in small steps, so no cell an edge passes through is missed.
    for ring in rings {
        for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)) {
            let steps = ((a.distance(*b) / (CELL / 4.0)).ceil() as usize).clamp(1, 1_000_000);
            for k in 0..=steps {
                let (c, r) = cell(a.lerp(*b, k as f64 / steps as f64));
                if let Some(row) = bits.get_mut(r.min(rows.saturating_sub(1))) {
                    set(row, c.min(cols.saturating_sub(1)));
                }
            }
        }
    }
    // Inside: each row's cell centres by the even-odd rule (holes in a piece are cut out with it, so
    // they would only waste fabric if filled; the cells over a hole are left free).
    for (r, row) in bits.iter_mut().enumerate() {
        let y = (r as f64 + 0.5) * CELL;
        let mut xs: Vec<f64> = vec![];
        for ring in rings {
            for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)) {
                if (a.y <= y) != (b.y <= y) {
                    xs.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
                }
            }
        }
        xs.sort_by(f64::total_cmp);
        for [x0, x1] in xs.as_chunks::<2>().0 {
            // The cells whose centres lie between the two crossings.
            let first = (x0 / CELL - 0.5).ceil().max(0.0);
            let last = (x1 / CELL - 0.5).floor().min(cols as f64 - 1.0);
            if last >= first {
                for c in first as usize..=last as usize {
                    set(row, c);
                }
            }
        }
    }
    bits
}

/// `bits` grown by `by` cells every way (a square round each cell).
fn grow(bits: &[Vec<u64>], cols: usize, by: usize) -> Grown {
    let (gc, gr) = (cols + 2 * by, bits.len() + 2 * by);
    let mut out = vec![vec![0u64; words(gc)]; gr];
    for (r, row) in bits.iter().enumerate() {
        for c in (0..cols).filter(|c| get(row, *c)) {
            for orow in out.iter_mut().skip(r).take(2 * by + 1) {
                for cc in c..=c + 2 * by {
                    set(orow, cc);
                }
            }
        }
    }
    Grown { cols: gc, by, bits: out }
}

/// Piece `rings` turned by `angle` (and flipped first when `flip`), moved so its cells start at 0.
fn shape(rings: &[Vec<Point>], angle: f64, flip: bool, gap_cells: usize) -> Option<Shape> {
    let turn = Affine::rotate(angle.to_radians()) * if flip { Affine::FLIP_X } else { Affine::IDENTITY };
    let turned: Vec<Vec<Point>> = rings.iter().map(|r| r.iter().map(|p| turn * *p).collect()).collect();
    let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in turned.iter().flatten() {
        (x0, y0, x1, y1) = (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y));
    }
    if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) || x1 - x0 > MAX_MAT || y1 - y0 > MAX_MAT {
        return None;
    }
    let to_zero = Affine::translate((-x0, -y0));
    let placed: Vec<Vec<Point>> = turned.iter().map(|r| r.iter().map(|p| to_zero * *p).collect()).collect();
    let cols = ((x1 - x0) / CELL).floor() as usize + 1;
    let rows = ((y1 - y0) / CELL).floor() as usize + 1;
    let bits = raster(&placed, cols, rows);
    let grown = grow(&bits, cols, gap_cells);
    Some(Shape { transform: to_zero * turn, angle, flipped: flip, cols, rows, bits, grown })
}

impl Mat {
    fn new(cols: usize, rows: usize) -> Mat {
        Mat { cols, rows, bits: vec![vec![0u64; words(cols)]; rows] }
    }

    /// Whether `s` fits with its cells' top left at column `x`, row `y`: inside the mat (`m`
    /// cells of margin), clear of what is taken by the gap.
    fn fits(&self, s: &Shape, x: usize, y: usize, m: usize) -> bool {
        if x + s.cols + m > self.cols || y + s.rows + m > self.rows || x < m || y < m {
            return false;
        }
        let g = &s.grown;
        // The grown cells may reach past the mat's edge (nothing is there).
        let (gx, gy) = (x as isize - g.by as isize, y as isize - g.by as isize);
        for (r, grow_row) in g.bits.iter().enumerate() {
            let Some(mr) = usize::try_from(gy + r as isize).ok().and_then(|mr| self.bits.get(mr)) else { continue };
            if gx >= 0 {
                let gx = gx as usize;
                for (i, w) in mr.iter().enumerate() {
                    if shifted(grow_row, gx, i) & w != 0 {
                        return false;
                    }
                }
            } else {
                // The grown shape starts left of the mat: test cell by cell.
                for c in 0..g.cols {
                    let mc = gx + c as isize;
                    if mc >= 0 && get(grow_row, c) && get(mr, mc as usize) {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn take(&mut self, s: &Shape, x: usize, y: usize) {
        for (r, row) in s.bits.iter().enumerate() {
            if let Some(mr) = self.bits.get_mut(y + r) {
                for c in (0..s.cols).filter(|c| get(row, *c)) {
                    set(mr, x + c);
                }
            }
        }
    }

    /// The free place for `s` nearest the top left, (row, column), if nearer than `before`.
    fn place(&self, s: &Shape, m: usize, before: Option<(usize, usize)>) -> Option<(usize, usize)> {
        let rows = self.rows.checked_sub(s.rows + m)?.min(before.map_or(usize::MAX, |b| b.0));
        let cols = self.cols.checked_sub(s.cols + m)?;
        (m..=rows).find_map(|y| (m..=cols).find(|x| self.fits(s, *x, y, m)).map(|x| (y, x))).filter(|yx| before.is_none_or(|b| *yx < b))
    }

    /// The nearest place over `shapes`: ((row, column), shape).
    fn best(&self, shapes: &[Shape], m: usize) -> Option<((usize, usize), usize)> {
        let mut best: Option<((usize, usize), usize)> = None;
        for (si, s) in shapes.iter().enumerate() {
            if let Some(yx) = self.place(s, m, best.map(|b| b.0)) {
                best = Some((yx, si));
            }
        }
        best
    }
}

/// Nest `pieces` (each a set of closed outlines, mm) as `o` says.
pub fn nest(pieces: &[Vec<Vec<Point>>], o: &Options) -> Nested {
    let mut out = Nested::default();
    let (mw, mh) = o.mat;
    if !(mw.is_finite() && mh.is_finite()) || mw < CELL || mh < CELL || mw > MAX_MAT || mh > MAX_MAT {
        out.unplaced = (0..pieces.len()).collect();
        return out;
    }
    let (cols, rows) = ((mw / CELL).floor() as usize, (mh / CELL).floor() as usize);
    let margin = (o.margin.max(0.0) / CELL).ceil() as usize;
    let gap_cells = (o.gap.clamp(0.0, 100.0) / CELL).ceil() as usize;
    // Largest first: the area of the outline's box.
    let area = |p: &Vec<Vec<Point>>| {
        let pts = p.iter().flatten();
        let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for q in pts {
            (x0, y0, x1, y1) = (x0.min(q.x), y0.min(q.y), x1.max(q.x), y1.max(q.y));
        }
        if x1 > x0 && y1 > y0 { (x1 - x0) * (y1 - y0) } else { 0.0 }
    };
    let mut order: Vec<usize> = (0..pieces.len()).collect();
    order.sort_by(|a, b| area(&pieces[*b]).total_cmp(&area(&pieces[*a])).then(a.cmp(b)));
    let mut mats: Vec<Mat> = vec![];
    for (n, i) in order.into_iter().enumerate() {
        let Some(p) = pieces.get(i) else { continue };
        let points: usize = p.iter().map(Vec::len).sum();
        let finite = p.iter().flatten().all(|q| q.x.is_finite() && q.y.is_finite());
        if n >= MAX_PIECES || points > MAX_POINTS || !finite || area(p) <= 0.0 {
            out.unplaced.push(i);
            continue;
        }
        let flips: &[bool] = if o.flip { &[false, true] } else { &[false] };
        let shapes: Vec<Shape> =
            o.rotate.angles().into_iter().flat_map(|a| flips.iter().map(move |f| (a, *f))).filter_map(|(a, f)| shape(p, a, f, gap_cells)).collect();
        // The first mat with room, each orientation's place nearest the top left.
        let mut found = None;
        for (mi, mat) in mats.iter().enumerate() {
            found = mat.best(&shapes, margin).map(|(yx, si)| (mi, yx, si));
            if found.is_some() {
                break;
            }
        }
        if found.is_none() {
            let fresh = Mat::new(cols, rows);
            if let Some((yx, si)) = fresh.best(&shapes, margin) {
                mats.push(fresh);
                found = Some((mats.len() - 1, yx, si));
            }
        }
        let Some((mi, (y, x), si)) = found else {
            out.unplaced.push(i);
            continue;
        };
        let (Some(mat), Some(s)) = (mats.get_mut(mi), shapes.get(si)) else { continue };
        mat.take(s, x, y);
        let at = Affine::translate((x as f64 * CELL, y as f64 * CELL));
        out.placements.push(Placement { piece: i, mat: mi, angle: s.angle, flipped: s.flipped, transform: at * s.transform });
    }
    out.mats = mats.len();
    out.placements.sort_by_key(|p| p.piece);
    out.unplaced.sort_unstable();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f64, h: f64) -> Vec<Vec<Point>> {
        vec![vec![Point::new(0.0, 0.0), Point::new(w, 0.0), Point::new(w, h), Point::new(0.0, h)]]
    }

    fn opts(mat: (f64, f64), gap: f64) -> Options {
        Options { mat, margin: 0.0, gap, rotate: Rotate::Quarter, flip: false }
    }

    /// The placed pieces' outlines, as polygons on their mats.
    fn placed(pieces: &[Vec<Vec<Point>>], n: &Nested) -> Vec<(usize, Vec<Point>)> {
        n.placements.iter().flat_map(|pl| pieces[pl.piece].iter().map(move |r| (pl.mat, r.iter().map(|p| pl.transform * *p).collect()))).collect()
    }

    fn dist_seg(p: Point, a: Point, b: Point) -> f64 {
        let ab = b - a;
        let t = if ab.hypot2() > 0.0 { ((p - a).dot(ab) / ab.hypot2()).clamp(0.0, 1.0) } else { 0.0 };
        p.distance(a + ab * t)
    }

    fn inside(p: Point, poly: &[Point]) -> bool {
        let mut c = false;
        for (a, b) in poly.iter().zip(poly.iter().cycle().skip(1)) {
            if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                c = !c;
            }
        }
        c
    }

    /// The least distance between two polygons' outlines, or 0 when one reaches into the other.
    fn gap_between(a: &[Point], b: &[Point]) -> f64 {
        if a.iter().any(|p| inside(*p, b)) || b.iter().any(|p| inside(*p, a)) {
            return 0.0;
        }
        let mut d = f64::INFINITY;
        for (p, q) in [(a, b), (b, a)] {
            for v in p {
                for (s, t) in q.iter().zip(q.iter().cycle().skip(1)) {
                    d = d.min(dist_seg(*v, *s, *t));
                }
            }
        }
        d
    }

    #[test]
    fn pieces_never_overlap_and_keep_the_gap() {
        let pieces: Vec<_> =
            [(40.0, 30.0), (50.0, 20.0), (25.0, 25.0), (60.0, 10.0), (35.0, 45.0), (20.0, 70.0)].iter().map(|(w, h)| rect(*w, *h)).collect();
        let n = nest(&pieces, &opts((120.0, 120.0), 3.0));
        assert!(n.unplaced.is_empty());
        let polys = placed(&pieces, &n);
        for (i, (ma, a)) in polys.iter().enumerate() {
            for p in a {
                assert!(p.x >= -1e-9 && p.y >= -1e-9 && p.x <= 120.0 + 1e-9 && p.y <= 120.0 + 1e-9, "{p:?} off the mat");
            }
            for (mb, b) in polys.iter().skip(i + 1) {
                if ma == mb {
                    assert!(gap_between(a, b) >= 3.0 - 1e-9, "{a:?} {b:?}");
                }
            }
        }
    }

    #[test]
    fn pieces_spill_onto_more_mats_and_too_big_ones_are_reported() {
        let pieces: Vec<_> = (0..5).map(|_| rect(80.0, 80.0)).chain([rect(400.0, 10.0)]).collect();
        let n = nest(&pieces, &opts((100.0, 100.0), 3.0));
        assert_eq!(n.mats, 5, "one 80 mm square per 100 mm mat");
        assert_eq!(n.unplaced, vec![5]);
        // Turning lets a long piece fit.
        let n = nest(&[rect(90.0, 20.0)], &Options { rotate: Rotate::None, ..opts((30.0, 100.0), 3.0) });
        assert_eq!(n.unplaced, vec![0]);
        let n = nest(&[rect(90.0, 20.0)], &opts((30.0, 100.0), 3.0));
        assert!(n.unplaced.is_empty());
        assert_eq!(n.placements[0].angle.rem_euclid(180.0), 90.0);
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(40))]
        /// Random triangles and quads, any turn: on their mats, apart by the gap, each placed or
        /// reported.
        #[test]
        fn random_pieces_keep_apart(shapes in proptest::collection::vec(proptest::collection::vec((0.0f64..60.0, 0.0f64..60.0), 3..5), 1..12), gap in 0.5f64..6.0) {
            let pieces: Vec<Vec<Vec<Point>>> = shapes.iter().map(|s| vec![s.iter().map(|(x, y)| Point::new(*x, *y)).collect()]).collect();
            let o = Options { mat: (150.0, 150.0), margin: 2.0, gap, rotate: Rotate::Any, flip: true };
            let n = nest(&pieces, &o);
            proptest::prop_assert_eq!(n.placements.len() + n.unplaced.len(), pieces.len());
            let polys = placed(&pieces, &n);
            for (i, (ma, a)) in polys.iter().enumerate() {
                for p in a {
                    proptest::prop_assert!(p.x >= 2.0 - 1e-6 && p.y >= 2.0 - 1e-6 && p.x <= 148.0 + 1e-6 && p.y <= 148.0 + 1e-6, "{:?} off the mat", p);
                }
                for (mb, b) in polys.iter().skip(i + 1) {
                    if ma == mb {
                        let d = gap_between(a, b);
                        proptest::prop_assert!(d >= gap - 1e-6, "{} < {}", d, gap);
                    }
                }
            }
        }
    }

    /// How long nesting takes: `cargo test -p vectorcraft-embroidery report_nest_time -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn report_nest_time() {
        let pieces: Vec<_> = (0..60).map(|i| rect(20.0 + f64::from(i % 7) * 9.0, 15.0 + f64::from(i % 5) * 11.0)).collect();
        for rotate in [Rotate::None, Rotate::Quarter, Rotate::Any] {
            let t = std::time::Instant::now();
            let n = nest(&pieces, &Options { mat: (304.8, 609.6), margin: 5.0, gap: 3.0, rotate, flip: true });
            println!("{rotate:?}: {} pieces on {} mats in {:?}", n.placements.len(), n.mats, t.elapsed());
        }
    }

    #[test]
    fn the_margin_is_kept_clear() {
        let n = nest(&[rect(20.0, 20.0)], &Options { margin: 10.0, ..opts((100.0, 100.0), 3.0) });
        let pl = n.placements[0];
        let p = pl.transform * Point::new(0.0, 0.0);
        assert!(p.x >= 10.0 - 1e-9 && p.y >= 10.0 - 1e-9, "{p:?}");
    }

    #[test]
    fn bad_input_is_unplaced_not_a_panic() {
        let nan = vec![vec![Point::new(f64::NAN, 0.0), Point::new(1.0, 1.0), Point::new(0.0, 1.0)]];
        let empty: Vec<Vec<Point>> = vec![];
        let n = nest(&[nan, empty, rect(5.0, 5.0)], &opts((f64::INFINITY, 50.0), 3.0));
        assert_eq!(n.unplaced, vec![0, 1, 2], "an infinite mat places nothing");
        let n = nest(
            &[vec![vec![Point::new(f64::NAN, 0.0), Point::new(1.0, 1.0), Point::new(0.0, 1.0)]], vec![], rect(5.0, 5.0)],
            &opts((50.0, 50.0), 3.0),
        );
        assert_eq!(n.unplaced, vec![0, 1], "a piece with a point that isn't a number, or none");
        let n = nest(&[rect(5.0, 5.0)], &opts((0.0, 0.0), 3.0));
        assert_eq!(n.unplaced, vec![0]);
    }
}
