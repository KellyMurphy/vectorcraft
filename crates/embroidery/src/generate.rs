//! Stitch generation: geometry (mm) to runs of needle points.
//!
//! Every function returns runs: point lists the needle sews in order, the first point being where
//! the needle goes down first. Points are mm in document coordinates (y down).

use kurbo::{Affine, BezPath, PathEl, Point, Vec2};
use vectorcraft_doc::Underlay;
use vectorcraft_geom::{FillRule, PathData};

use crate::{MAX_NEEDLES, PT_MM};

/// How far a flattened curve may stray from the true one, mm.
const FLATTEN_MM: f64 = 0.05;
/// A turn sharper than this keeps its corner point in running stitch (cos of 30°).
const CORNER_COS: f64 = 0.866;
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
fn resample(line: &[Point], step: f64, corners: bool) -> Vec<Point> {
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
            out.push(b);
            since = 0.0;
        }
    }
    let last = line.last().copied().unwrap_or(first);
    match out.last() {
        // A last stitch much shorter than the others lands with the one before it.
        Some(&p) if out.len() > 1 && p.distance(last) < step * 0.25 => {
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
fn normals(c: &[Point], closed: bool) -> Vec<Vec2> {
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

/// Satin column: a zigzag `width` mm wide centred on `line`, a zig every `spacing` mm.
pub fn satin(line: &[Point], closed: bool, width: f64, spacing: f64) -> Vec<Point> {
    let c = resample(line, spacing, false);
    if c.len() < 2 {
        return c;
    }
    let half = width / 2.0;
    c.iter().zip(normals(&c, closed)).enumerate().map(|(i, (p, n))| *p + n * if i % 2 == 0 { half } else { -half }).collect()
}

/// The underlay of a satin column `width` mm wide along `line`, and whether it ends where the
/// line ends (so the top stitches go back the other way).
pub fn satin_underlay(line: &[Point], closed: bool, width: f64, kind: Underlay) -> (Vec<Point>, bool) {
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
                match next {
                    Some(j) if near(j).is_some_and(|x| (x - to).abs() <= 2.0 * len) => {
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

/// The inside spans of the row at `y`.
fn row_spans(polys: &[Vec<Point>], rule: FillRule, y: f64) -> Vec<(f64, f64)> {
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
