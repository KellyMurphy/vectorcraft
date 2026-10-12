//! Stitch generation: geometry (mm) to runs of needle points.
//!
//! Every function returns runs: point lists the needle sews in order, the first point being where
//! the needle goes down first. Points are mm in document coordinates (y down).

use kurbo::{Affine, BezPath, PathEl, Point, Vec2};
use vectorcraft_doc::{StitchSettings, Underlay};
use vectorcraft_geom::{FillRule, PathData};

use crate::{MAX_NEEDLES, PT_MM};

/// How far a flattened curve may stray from the true one, mm.
const FLATTEN_MM: f64 = 0.05;
/// A turn sharper than this keeps its corner point in running stitch (cos of 30°).
const CORNER_COS: f64 = 0.866;
/// The longest stitch generated, mm (DST moves at most 12.1 mm at once).
pub(crate) const MAX_STITCH: f64 = 12.0;
/// The most rows one fill may have.
const MAX_ROWS: usize = 200_000;

/// Each subpath of `path` flattened to a polyline in mm, with whether it is closed (a closed one
/// repeats its first point at the end). Non-finite points are left out.
pub fn polylines(path: &PathData) -> Vec<(Vec<Point>, bool)> {
    let mut out = vec![];
    for sp in &path.subpaths {
        let mut bp = BezPath::new();
        sp.to_bezpath_into(&mut bp);
        let bp = Affine::scale(PT_MM) * bp;
        let mut line: Vec<Point> = vec![];
        let mut closed = false;
        kurbo::flatten(bp, FLATTEN_MM, |el| match el {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => {
                if p.is_finite() && line.last() != Some(&p) {
                    line.push(p)
                }
            }
            PathEl::ClosePath => closed = true,
            _ => {}
        });
        if closed
            && let Some(&f) = line.first()
            && line.last() != Some(&f)
        {
            line.push(f);
        }
        if line.len() > 1 {
            out.push((line, closed));
        }
    }
    out
}

/// Manual stitch: the needle goes down at every anchor of each subpath, as drawn.
pub fn manual(path: &PathData) -> Vec<Vec<Point>> {
    path.subpaths
        .iter()
        .filter_map(|sp| {
            let mut pts: Vec<Point> = sp.anchors.iter().map(|a| Point::new(a.p.x * PT_MM, a.p.y * PT_MM)).filter(|p| p.is_finite()).collect();
            if sp.closed
                && let Some(&f) = pts.first()
            {
                pts.push(f);
            }
            (!pts.is_empty()).then_some(pts)
        })
        .collect()
}

fn length(line: &[Point]) -> f64 {
    line.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// Points along `line` every `step` mm (from the first point, ending on the last), keeping the
/// corners sharper than 30° when `corners`.
pub(crate) fn resample(line: &[Point], step: f64, corners: bool) -> Vec<Point> {
    let Some(&first) = line.first() else { return vec![] };
    if !step.is_finite() || step <= 0.0 || length(line) / step > MAX_NEEDLES as f64 {
        return vec![];
    }
    let mut out = vec![first];
    // Distance travelled since the last point put out.
    let mut since = 0.0;
    for (i, w) in line.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        let seg = a.distance(b);
        if seg <= 0.0 {
            continue;
        }
        let mut t = step - since;
        while t < seg - 1e-9 {
            out.push(a.lerp(b, t / seg));
            t += step;
        }
        since = seg - (t - step);
        let corner = corners
            && line.get(i + 2).is_some_and(|&c| {
                let (u, v) = (b - a, c - b);
                let (lu, lv) = (u.hypot(), v.hypot());
                lu > 0.0 && lv > 0.0 && u.dot(v) / (lu * lv) < CORNER_COS
            });
        if corner && since > 1e-9 {
            // A needle point just before the corner moves onto it rather than leave a tiny stitch.
            let n = out.len();
            let fits = n > 1 && out.get(n - 2).is_some_and(|q| q.distance(b) <= MAX_STITCH);
            match out.last_mut() {
                Some(l) if fits && l.distance(b) < step * 0.25 => *l = b,
                _ => out.push(b),
            }
            since = 0.0;
        }
    }
    let last = line.last().copied().unwrap_or(first);
    match out.last() {
        // A last stitch much shorter than the others lands with the one before it, unless that
        // makes a stitch longer than any allowed.
        Some(&p) if p.distance(last) < step * 0.25 && out.len() > 1 && out.get(out.len() - 2).is_some_and(|q| q.distance(last) <= MAX_STITCH) => {
            if let Some(l) = out.last_mut() {
                *l = last;
            }
        }
        Some(&p) if p.distance(last) > 1e-9 => out.push(last),
        _ => {}
    }
    out
}

/// Running stitch along `line`, stitches `len` mm long.
pub fn running(line: &[Point], len: f64) -> Vec<Point> {
    resample(line, len, true)
}

/// Bean stitch: each running stitch sewn `passes` times (odd: forward, back, forward…).
pub fn bean(run: &[Point], passes: u8) -> Vec<Point> {
    let Some(&first) = run.first() else { return vec![] };
    let mut out = vec![first];
    for w in run.windows(2) {
        out.push(w[1]);
        for _ in 0..passes.saturating_sub(1) / 2 {
            out.extend([w[0], w[1]]);
        }
    }
    out
}

/// How far inside the column edge underlay runs, mm.
const UNDERLAY_INSET: f64 = 0.4;
/// Underlay stitch length, mm.
const UNDERLAY_LEN: f64 = 2.0;
/// Zigzag and fill underlay spacing, mm.
const UNDERLAY_SPACING: f64 = 2.0;
/// Fill underlay stitch length, mm.
const UNDERLAY_FILL_LEN: f64 = 3.0;

/// The unit normal (left of the direction of travel) at each point of `c`.
pub(crate) fn normals(c: &[Point], closed: bool) -> Vec<Vec2> {
    let n = c.len();
    (0..n)
        .map(|i| {
            let (prev, next) = if closed {
                // The last point repeats the first: its neighbours wrap around past it.
                (if i == 0 { n.saturating_sub(2) } else { i - 1 }, if i + 1 >= n { 1.min(n - 1) } else { i + 1 })
            } else {
                (i.saturating_sub(1), (i + 1).min(n - 1))
            };
            let t = match (c.get(next), c.get(prev)) {
                (Some(&a), Some(&b)) => a - b,
                _ => Vec2::ZERO,
            };
            let l = t.hypot();
            if l > 0.0 { Vec2::new(-t.y / l, t.x / l) } else { Vec2::new(0.0, 1.0) }
        })
        .collect()
}

/// One zig of a satin column: the centre point, its left normal, the piece of the line it
/// belongs to (fans: none), and the point its inner side takes instead of `centre ± normal ×
/// half` (at a corner).
struct Zig {
    centre: Point,
    normal: Vec2,
    piece: Option<usize>,
    left: Option<Point>,
    right: Option<Point>,
}

impl Zig {
    fn new(centre: Point, normal: Vec2, piece: Option<usize>) -> Self {
        Self { centre, normal, piece, left: None, right: None }
    }

    /// Where its left (or right) side would go without an override.
    fn side(&self, left: bool, half: f64) -> Point {
        self.centre + self.normal * if left { half } else { -half }
    }

    fn pin(&mut self, left: bool, at: Point) {
        if left { self.left = Some(at) } else { self.right = Some(at) }
    }
}

/// A sharp turn of a satin column: its inner miter point, the directions in and out, and which
/// side is inside.
struct Corner {
    /// The pieces before and after it.
    piece: usize,
    miter: Point,
    t_in: Vec2,
    t_out: Vec2,
    left_inside: bool,
}

/// Is the turn at `b` (from `a`, on to `c`) sharp enough to be a corner?
fn is_corner(a: Point, b: Point, c: Point) -> bool {
    let (u, v) = (b - a, c - b);
    let (lu, lv) = (u.hypot(), v.hypot());
    lu > 0.0 && lv > 0.0 && u.dot(v) / (lu * lv) < CORNER_COS
}

/// A closed satin line started again halfway along its longest segment, so no corner falls on
/// the seam (an open line as it is). The underlay and the column both start there.
fn reseam(line: &[Point], closed: bool) -> Vec<Point> {
    let mut line = line.to_vec();
    if closed && line.len() > 3 {
        // The closing point repeats the first: rotate the open ring, then close it again.
        line.pop();
        let n = line.len();
        let seg = |k: usize| line.get(k).zip(line.get((k + 1) % n)).map_or(0.0, |(a, b)| a.distance(*b));
        if let Some(i) = (0..n).max_by(|&i, &j| seg(i).total_cmp(&seg(j)))
            && let (Some(&a), Some(&b)) = (line.get(i), line.get((i + 1) % n))
        {
            let mid = a.lerp(b, 0.5);
            let mut ring = vec![mid];
            ring.extend((1..=n).filter_map(|k| line.get((i + k) % n).copied()));
            ring.push(mid);
            line = ring;
        }
    }
    line
}

/// `line` split at its sharp corners into pieces (each ends where the next starts), and whether
/// it is one closed ring without corners.
fn pieces(line: &[Point], closed: bool) -> (Vec<Vec<Point>>, bool) {
    let corners: Vec<usize> = (1..line.len().saturating_sub(1))
        .filter(|&i| matches!((line.get(i - 1), line.get(i), line.get(i + 1)), (Some(&a), Some(&b), Some(&c)) if is_corner(a, b, c)))
        .collect();
    if corners.is_empty() {
        return (vec![line.to_vec()], closed);
    }
    let mut out = vec![];
    let mut start = 0;
    for &c in &corners {
        out.push(line.get(start..=c).unwrap_or_default().to_vec());
        start = c;
    }
    out.push(line.get(start..).unwrap_or_default().to_vec());
    (out, false)
}

/// Satin column: a zigzag `width` mm wide centred on `line`, a zig every `spacing` mm. At a sharp
/// corner the inner side's needle points stop at the inner miter point and the column fans
/// round it, so the stitches neither pile up inside the turn nor leave a gap outside it.
pub fn satin(line: &[Point], closed: bool, width: f64, spacing: f64) -> Vec<Point> {
    column(line, closed, width, spacing, true)
}

/// A zigzag `width` mm wide along `line`, a point every `spacing` mm, its inner side held at
/// each sharp corner's inner miter point. With `fan`, the outer side sweeps round the corner in
/// steps (a dense satin); without, one point at the outer miter turns it (an open zigzag).
fn column(line: &[Point], closed: bool, width: f64, spacing: f64, fan: bool) -> Vec<Point> {
    let half = width / 2.0;
    let line = reseam(line, closed);
    let (pieces, ring) = pieces(&line, closed);
    let mut zigs: Vec<Zig> = vec![];
    let mut corners: Vec<Corner> = vec![];
    for (k, piece) in pieces.iter().enumerate() {
        let c = resample(piece, spacing, false);
        // The corner point a piece starts at belongs to the fan before it.
        zigs.extend(c.iter().zip(normals(&c, ring)).skip(usize::from(k > 0)).map(|(p, n)| Zig::new(*p, n, Some(k))));
        let (Some(next), Some(&v)) = (pieces.get(k + 1), piece.last()) else { continue };
        let before = piece.iter().rev().find(|p| p.distance(v) > 1e-9).copied();
        let after = next.iter().find(|p| p.distance(v) > 1e-9).copied();
        let (Some(before), Some(after)) = (before, after) else { continue };
        let (t_in, t_out) = ((v - before) / (v - before).hypot(), (after - v) / (after - v).hypot());
        let (n_in, n_out) = (Vec2::new(-t_in.y, t_in.x), Vec2::new(-t_out.y, t_out.x));
        // Turning toward the left normal: the left side is inside.
        let left_inside = t_out.dot(n_in) > 0.0;
        let sign = if left_inside { 1.0 } else { -1.0 };
        // Where the two inner edges meet, held near the corner: within 3 half-widths (hairpins),
        // and close enough that no stitch from the outer edge to it is too long to sew.
        let bis = (n_in + n_out) * sign;
        let miter = if bis.hypot() > 0.2 { v + bis * (2.0 * half / bis.hypot2()) } else { v };
        let reach = (3.0 * half).min(MAX_STITCH - half - spacing).max(0.0);
        let miter = if miter.distance(v) > reach { v + (miter - v) * (reach / miter.distance(v)) } else { miter };
        // The fan: the outer side sweeps round the corner while the inner side holds the miter.
        let turn = n_in.dot(n_out).clamp(-1.0, 1.0).acos();
        let steps = ((turn * half / spacing).ceil() as usize).clamp(1, 64);
        if !fan {
            // One zig across the corner: the outer miter opposite the inner one.
            let bis = n_in + n_out;
            let n = if bis.hypot() > 1e-9 { bis / bis.hypot() } else { n_in };
            let mut z = Zig::new(v, n, None);
            z.pin(left_inside, miter);
            z.pin(!left_inside, v + (v - miter));
            zigs.push(z);
            corners.push(Corner { piece: k, miter, t_in, t_out, left_inside });
            continue;
        }
        for j in 0..=steps {
            let n = n_in.lerp(n_out, j as f64 / steps as f64);
            let n = if n.hypot() > 1e-9 { n / n.hypot() } else { n_in };
            let mut z = Zig::new(v, n, None);
            z.pin(left_inside, miter);
            zigs.push(z);
        }
        corners.push(Corner { piece: k, miter, t_in, t_out, left_inside });
    }
    // Inner points that would cross past a corner's miter point land on it instead.
    for c in &corners {
        for z in zigs.iter_mut() {
            let inner = z.side(c.left_inside, half);
            let past = match z.piece {
                Some(p) if p == c.piece => (inner - c.miter).dot(c.t_in) > 0.0,
                Some(p) if p == c.piece + 1 => (inner - c.miter).dot(c.t_out) < 0.0,
                _ => false,
            };
            if past {
                z.pin(c.left_inside, c.miter);
            }
        }
    }
    zigs.iter()
        .enumerate()
        .map(|(i, z)| {
            let left = i % 2 == 0;
            let pinned = if left { z.left } else { z.right };
            pinned.unwrap_or(z.side(left, half))
        })
        .collect()
}

/// Fills for the parts of a shape no stroke runs through (the dot of an i, a full stop: round
/// parts satin can't follow). A part is an outline not inside another (those are its holes).
fn uncovered_parts(polys: &[Vec<Point>], rule: FillRule, strokes: &[crate::skeleton::Stroke]) -> Vec<Vec<Point>> {
    let within = |p: &Vec<Point>, q: Point| inside(std::slice::from_ref(p), FillRule::EvenOdd, q);
    let mut out = vec![];
    for (i, p) in polys.iter().enumerate() {
        let Some(&first) = p.first() else { continue };
        let hole = polys.iter().enumerate().any(|(j, q)| j != i && within(q, first));
        if hole {
            continue;
        }
        let covered = strokes.iter().flat_map(|s| s.points.iter()).any(|&q| within(p, q));
        if !covered && let Ok(f) = fill(std::slice::from_ref(p), rule, 45.0, StitchSettings::DEFAULT_SPACING, 3.0, StitchSettings::DEFAULT_STAGGER) {
            out.extend(f);
        }
    }
    out
}

/// A fill's satin border: a satin column `width` mm wide along the closed outline `poly`,
/// centred on its edge, at satin density.
pub fn border(poly: &[Point], width: f64) -> Vec<Point> {
    let mut ring = poly.to_vec();
    if let (Some(&f), Some(&l)) = (ring.first(), ring.last())
        && f.distance(l) > 1e-9
    {
        ring.push(f);
    }
    satin(&ring, true, width, StitchSettings::DEFAULT_SPACING)
}

/// Zigzag stitch: an open zigzag `width` mm wide along `line`, its points `spacing` mm apart,
/// fanning round sharp corners like a satin. The width is narrowed where it would make a
/// diagonal stitch longer than can be sewn.
pub fn zigzag(line: &[Point], closed: bool, width: f64, spacing: f64) -> Vec<Point> {
    let width = width.min((MAX_STITCH * MAX_STITCH - spacing * spacing).max(0.0).sqrt());
    column(line, closed, width, spacing, false)
}

/// The underlay of a satin column `width` mm wide along `line`, and whether it ends where the
/// line ends (so the top stitches go back the other way).
pub fn satin_underlay(line: &[Point], closed: bool, width: f64, kind: Underlay) -> (Vec<Point>, bool) {
    let line = &reseam(line, closed)[..];
    let inner = width - 2.0 * UNDERLAY_INSET;
    match kind {
        Underlay::Center => (running(line, UNDERLAY_LEN), true),
        // Too narrow for two edges: down the middle.
        Underlay::Edge | Underlay::Zigzag if inner < 0.5 => (running(line, UNDERLAY_LEN), true),
        Underlay::Edge => {
            let c = resample(line, UNDERLAY_LEN, true);
            let n = normals(&c, closed);
            let half = inner / 2.0;
            let mut out: Vec<Point> = c.iter().zip(&n).map(|(p, n)| *p + *n * half).collect();
            out.extend(c.iter().zip(&n).rev().map(|(p, n)| *p - *n * half));
            (out, false)
        }
        Underlay::Zigzag => (satin(line, closed, inner, UNDERLAY_SPACING), true),
        Underlay::None | Underlay::Fill => (vec![], false),
    }
}

/// A fill's underlay: sparse rows across the top rows' `angle`.
pub fn fill_underlay(polys: &[Vec<Point>], rule: FillRule, angle: f64) -> Result<Vec<Vec<Point>>, String> {
    fill(polys, rule, angle + 90.0, UNDERLAY_SPACING, UNDERLAY_FILL_LEN, 2)
}

/// Tatami fill: rows `spacing` mm apart at `angle` degrees (counter-clockwise on the page) across
/// the area of the closed `polys` (filled by `rule`), each row running stitches up to `len` mm
/// long, staggered row to row so the needle holes line up again only every `stagger` rows (2: a
/// brick pattern, 3: tatami…). Rows that touch are sewn as one
/// run, back and forth; separate parts of the shape become separate runs.
pub fn fill(polys: &[Vec<Point>], rule: FillRule, angle: f64, spacing: f64, len: f64, stagger: u8) -> Result<Vec<Vec<Point>>, String> {
    let stagger = usize::from(stagger.max(1));
    if !(spacing.is_finite() && len.is_finite()) || spacing <= 0.0 || len <= 0.0 {
        return Ok(vec![]);
    }
    // Turn the shape so the rows run along x (y down: page counter-clockwise is a negative turn).
    let rot = Affine::rotate(angle.to_radians());
    let inv = rot.inverse();
    let polys: Vec<Vec<Point>> = polys.iter().map(|p| p.iter().map(|q| rot * *q).collect()).collect();
    let (mut y0, mut y1, mut x0, mut x1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for p in polys.iter().flatten() {
        y0 = y0.min(p.y);
        y1 = y1.max(p.y);
        x0 = x0.min(p.x);
        x1 = x1.max(p.x);
    }
    if !(y1 - y0).is_finite() || y1 <= y0 {
        return Ok(vec![]);
    }
    let rows = ((y1 - y0) / spacing).ceil();
    if rows > MAX_ROWS as f64 || rows * ((x1 - x0) / len + 2.0) > MAX_NEEDLES as f64 {
        return Err(format!("a fill {:.0} × {:.0} mm is too large to stitch: left out", x1 - x0, y1 - y0));
    }
    // Each row's inside spans, left to right.
    let mut spans: Vec<Vec<(f64, f64)>> = vec![];
    let mut y = y0 + spacing / 2.0;
    while y < y1 {
        spans.push(row_spans(&polys, rule, y));
        y += spacing;
    }
    let row_y = |r: usize| y0 + spacing / 2.0 + r as f64 * spacing;
    // Lanes: from the first unsewn span, take the overlapping span of each next row.
    let mut used: Vec<Vec<bool>> = spans.iter().map(|r| vec![false; r.len()]).collect();
    let mut out = vec![];
    for start in 0..spans.len() {
        while let Some(first) = used.get(start).and_then(|u| u.iter().position(|d| !d)) {
            let mut run: Vec<Point> = vec![];
            let (mut r, mut i, mut forward) = (start, first, true);
            while let Some(&(a, b)) = spans.get(r).and_then(|s| s.get(i)) {
                if let Some(u) = used.get_mut(r).and_then(|u| u.get_mut(i)) {
                    *u = true;
                }
                let (from, to) = if forward { (a, b) } else { (b, a) };
                row(&mut run, row_y(r), from, to, len, (r % stagger) as f64 / stagger as f64);
                // The next row starts at the end nearest this row's end; a join longer than two
                // stitches would cross outside the shape (round a hole or notch): a new run instead.
                let next = spans.get(r + 1).and_then(|s| {
                    s.iter().enumerate().position(|(j, &(c, d))| used.get(r + 1).and_then(|u| u.get(j)) == Some(&false) && c < b && d > a)
                });
                let near = |j: usize| spans.get(r + 1).and_then(|s| s.get(j)).map(|&(c, d)| if forward { d } else { c });
                let joins = |x: f64| {
                    let (p, q) = (Point::new(to, row_y(r)), Point::new(x, row_y(r + 1)));
                    (x - to).abs() <= 2.0 * len && [0.25, 0.5, 0.75].iter().all(|t| inside(&polys, rule, p.lerp(q, *t)))
                };
                match next {
                    Some(j) if near(j).is_some_and(joins) => {
                        r += 1;
                        i = j;
                        forward = !forward;
                    }
                    _ => break,
                }
            }
            out.push(run.into_iter().map(|p| inv * p).collect());
        }
    }
    Ok(out)
}

/// Is `p` inside the area `polys` fill (or within 0.05 mm of its edge)?
fn inside(polys: &[Vec<Point>], rule: FillRule, p: Point) -> bool {
    row_spans(polys, rule, p.y).iter().any(|&(a, b)| p.x >= a - 0.05 && p.x <= b + 0.05)
}

/// The inside spans of the row at `y`.
pub(crate) fn row_spans(polys: &[Vec<Point>], rule: FillRule, y: f64) -> Vec<(f64, f64)> {
    let mut xs: Vec<(f64, i32)> = vec![];
    for p in polys {
        let n = p.len();
        for k in 0..n {
            let (a, b) = (p[k], p[(k + 1) % n]);
            if (a.y <= y) != (b.y <= y) {
                let x = a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y);
                xs.push((x, if b.y > a.y { 1 } else { -1 }));
            }
        }
    }
    xs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let inside = |w: i32| match rule {
        FillRule::NonZero => w != 0,
        FillRule::EvenOdd => w % 2 != 0,
    };
    let mut out = vec![];
    let mut w = 0;
    let mut start = 0.0;
    for (x, d) in xs {
        let was = inside(w);
        w += d;
        match (was, inside(w)) {
            (false, true) => start = x,
            (true, false) if x > start => out.push((start, x)),
            _ => {}
        }
    }
    out
}

/// Sew from the end of `run` to `p` in stitches no longer than `len`.
fn sew(run: &mut Vec<Point>, p: Point, len: f64) {
    if let Some(&last) = run.last() {
        let steps = (last.distance(p) / len).ceil().max(1.0) as usize;
        for k in 1..steps {
            run.push(last.lerp(p, k as f64 / steps as f64));
        }
    }
    run.push(p);
}

/// One fill row from `from` to `to` at `y`, joined to `run` (whose end is where the needle is),
/// in stitches no longer than `len`: the row's inner needle points sit on a grid shifted by
/// `shift` of a stitch.
fn row(run: &mut Vec<Point>, y: f64, from: f64, to: f64, len: f64, shift: f64) {
    sew(run, Point::new(from, y), len);
    let offset = shift * len;
    let margin = (len * 0.25).max(0.3);
    let (lo, hi) = (from.min(to), from.max(to));
    let first = ((lo + margin - offset) / len).ceil();
    let last = ((hi - margin - offset) / len).floor();
    if last >= first {
        let mut grid: Vec<f64> = (first as i64..=last as i64).map(|k| k as f64 * len + offset).collect();
        if to < from {
            grid.reverse();
        }
        for x in grid {
            sew(run, Point::new(x, y), len);
        }
    }
    sew(run, Point::new(to, y), len);
}

#[cfg(test)]
pub(crate) fn tests_inside(polys: &[Vec<Point>], p: Point) -> bool {
    // On the edge counts as inside: probe a hair either side.
    [(0.0, 0.0), (0.0, 0.05), (0.0, -0.05), (0.05, 0.0), (-0.05, 0.0)]
        .iter()
        .any(|&(dx, dy)| inside(polys, FillRule::NonZero, Point::new(p.x + dx, p.y + dy)))
}

/// Points every `step` mm along `line` (from its first point, ending on its last) with `values`
/// interpolated at them.
fn resample_with(line: &[Point], values: &[f64], step: f64) -> (Vec<Point>, Vec<f64>) {
    let at = |i: usize| (line.get(i).copied(), values.get(i).copied().unwrap_or(0.0));
    let (Some(first), v0) = at(0) else { return (vec![], vec![]) };
    let mut pts = vec![first];
    let mut vals = vec![v0];
    let mut since = 0.0;
    for i in 0..line.len().saturating_sub(1) {
        let ((Some(a), va), (Some(b), vb)) = (at(i), at(i + 1)) else { continue };
        let seg = a.distance(b);
        if seg <= 0.0 {
            continue;
        }
        let mut t = step - since;
        while t < seg - 1e-9 {
            pts.push(a.lerp(b, t / seg));
            vals.push(va + (vb - va) * t / seg);
            t += step;
        }
        since = seg - (t - step);
    }
    if let (Some(&last), Some(&lv)) = (line.last(), values.last())
        && pts.last().is_some_and(|p| p.distance(last) > step * 0.3)
    {
        pts.push(last);
        vals.push(lv);
    }
    (pts, vals)
}

/// Satin that follows a closed shape's strokes (lettering): the shape's centre lines found
/// ([`crate::skeleton::strokes`]), and along each a zigzag every `spacing` mm whose points sit on
/// the shape's edges either side, widened by `pull` mm. With `underlay`, each stroke first gets a
/// running stitch down its middle and the satin comes back over it. Strokes are sewn nearest
/// first; one run each.
pub fn satin_shape(polys: &[Vec<Point>], rule: FillRule, spacing: f64, pull: f64, underlay: bool) -> Vec<Vec<Point>> {
    let mut strokes = crate::skeleton::strokes(polys, rule);
    let mut out = vec![];
    if !strokes.is_empty() {
        out.extend(uncovered_parts(polys, rule, &strokes));
    }
    let mut at: Option<Point> = None;
    while !strokes.is_empty() {
        // The stroke with an end nearest the needle, turned to start there.
        let near = |s: &crate::skeleton::Stroke| -> (f64, bool) {
            let d = |p: Option<&Point>| match (at, p) {
                (Some(a), Some(p)) => a.distance(*p),
                _ => 0.0,
            };
            let (ds, de) = (d(s.points.first()), d(s.points.last()));
            if de < ds { (de, true) } else { (ds, false) }
        };
        let Some((i, flip)) = strokes.iter().enumerate().map(|(i, s)| (i, near(s))).min_by(|a, b| a.1.0.total_cmp(&b.1.0)).map(|(i, (_, f))| (i, f))
        else {
            break;
        };
        let mut s = strokes.remove(i);
        if flip {
            s.points.reverse();
            s.radius.reverse();
        }
        let closed = s.points.len() > 3 && s.points.first() == s.points.last();
        let mut run = vec![];
        let (line, radius) = if underlay {
            run.extend(running(&s.points, UNDERLAY_LEN));
            (s.points.iter().rev().copied().collect::<Vec<_>>(), s.radius.iter().rev().copied().collect::<Vec<_>>())
        } else {
            (s.points.clone(), s.radius.clone())
        };
        let (c, r) = resample_with(&line, &radius, spacing);
        let n = normals(&c, closed);
        for (k, ((p, n), r)) in c.iter().zip(&n).zip(&r).enumerate() {
            // Out to the edge each side, but not past twice the stroke's half-width there (at a
            // junction the edge across may be far down another stroke).
            let cap = (2.0 * r).clamp(0.3, MAX_STITCH / 2.0);
            let left = crate::skeleton::cast(polys, *p, *n, cap).unwrap_or(*r);
            let right = crate::skeleton::cast(polys, *p, -*n, cap).unwrap_or(*r);
            let (mut left, mut right) = (left + pull / 2.0, right + pull / 2.0);
            // The diagonal to the next zig must be sewable too.
            let most = (MAX_STITCH - spacing).max(0.3);
            let span = left + right;
            if span > most {
                (left, right) = (left * most / span, right * most / span);
            }
            run.push(if k % 2 == 0 { *p + *n * left } else { *p - *n * right });
        }
        if let Some(&end) = run.last() {
            at = Some(end);
        }
        if run.len() > 1 {
            out.push(split_long(&run));
        }
    }
    out
}

/// `run` with any stitch longer than [`MAX_STITCH`] split into equal shorter ones (a tight curve
/// can swing a zig further than its width).
fn split_long(run: &[Point]) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(run.len());
    for &p in run {
        if let Some(&last) = out.last() {
            let n = (last.distance(p) / MAX_STITCH).ceil() as usize;
            for k in 1..n {
                out.push(last.lerp(p, k as f64 / n as f64));
            }
        }
        out.push(p);
    }
    out
}
