//! Contour fill (E8.3a): rings that follow the outline, each a row's spacing inside the last.
//!
//! The shape's distance field ([`crate::skeleton::field`]) is traced at a level every `spacing`
//! mm in from the edge (marching squares, each crossing interpolated along the pixel edge), so a
//! ring is the set of points that far inside the shape: rings round a hole grow out from it,
//! and a ring splits where the shape narrows into two lobes. The rings are then sewn in one run
//! where they can be: each from its point nearest where the needle is, then a short step to the
//! next ring, the nearest one left. A step longer than a few rows starts a new run (the needle
//! jumps, or travels, between them as between any two runs).
//!
//! The spiral fill (E8.3b, [`spiral`]) sews the same rings with no step between them: each ring
//! blends into the next over one turn.

use kurbo::Point;
use vectorcraft_geom::FillRule;

use crate::generate;
use crate::skeleton::{Field, field};

/// The most rings a fill sews (a huge area at the finest spacing makes fewer than this).
const MAX_RINGS: usize = 4000;
/// A step to the next ring longer than this many row spacings starts a new run.
const JOIN_ROWS: f64 = 3.0;

/// The rings of `polys` (filled by `rule`) at `spacing` mm: the first `first` mm inside the edge,
/// each next one `spacing` further in. None when the shape is empty or too thin for one ring.
pub fn rings(polys: &[Vec<Point>], rule: FillRule, spacing: f64, first: f64) -> Option<Vec<Vec<Point>>> {
    leveled(polys, rule, spacing, first).map(|r| r.into_iter().map(|(_, ring)| ring).collect())
}

/// [`rings`], each with its level: 0 for the rings `first` in, 1 for the next, and so on.
fn leveled(polys: &[Vec<Point>], rule: FillRule, spacing: f64, first: f64) -> Option<Vec<(usize, Vec<Point>)>> {
    if !spacing.is_finite() || spacing <= 0.0 || !first.is_finite() {
        return None;
    }
    let f = field(polys, rule)?;
    let deepest = f.d.iter().copied().fold(0.0, f64::max);
    if deepest <= 0.0 {
        return None;
    }
    // A shape thinner than the first ring's depth gets one ring down its middle.
    let first = if first < deepest { first.max(f.px * 0.5) } else { deepest * 0.5 };
    let count = (((deepest - first) / spacing).floor() as usize + 1).min(MAX_RINGS);
    let levels: Vec<f64> = (0..count).map(|k| first + k as f64 * spacing).filter(|l| *l < deepest).collect();
    let rings = trace(&f, &levels);
    (!rings.is_empty()).then_some(rings)
}

/// Where a crossing lies: on the horizontal pixel edge right of (x, y) or the vertical one below.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Edge {
    vertical: bool,
    x: usize,
    y: usize,
}

/// The closed rings of field `f` at each of `levels` with their level's index, innermost level last.
fn trace(f: &Field, levels: &[f64]) -> Vec<(usize, Vec<Point>)> {
    let at = |x: usize, y: usize| f.d.get(y * f.w + x).copied().unwrap_or(0.0);
    let mm = |x: f64, y: f64| Point::new(f.origin.x + x * f.px, f.origin.y + y * f.px);
    // Each level's segments, as pairs of the pixel edges they cross.
    let mut segs: Vec<Vec<(Edge, Edge)>> = vec![vec![]; levels.len()];
    for y in 0..f.h.saturating_sub(1) {
        for x in 0..f.w.saturating_sub(1) {
            let v = [at(x, y), at(x + 1, y), at(x + 1, y + 1), at(x, y + 1)];
            let (lo, hi) = (v.iter().copied().fold(f64::INFINITY, f64::min), v.iter().copied().fold(0.0, f64::max));
            // The levels crossing this cell (levels are ascending): a corner at the level counts as
            // outside it, so a level equal to the lowest corner can still cross.
            let from = levels.partition_point(|l| *l < lo);
            let to = levels.partition_point(|l| *l <= hi);
            for (k, level) in levels.iter().enumerate().take(to).skip(from) {
                cell(x, y, v, *level, segs.get_mut(k));
            }
        }
    }
    let point = |e: Edge, level: f64| {
        let (a, b, ax, ay, bx, by) = if e.vertical {
            (at(e.x, e.y), at(e.x, e.y + 1), e.x, e.y, e.x, e.y + 1)
        } else {
            (at(e.x, e.y), at(e.x + 1, e.y), e.x, e.y, e.x + 1, e.y)
        };
        let t = if (b - a).abs() > 1e-12 { ((level - a) / (b - a)).clamp(0.0, 1.0) } else { 0.5 };
        mm(ax as f64 + (bx as f64 - ax as f64) * t, ay as f64 + (by as f64 - ay as f64) * t)
    };
    let mut out = vec![];
    for (k, (level, segs)) in levels.iter().zip(segs).enumerate() {
        for ring in link(&segs) {
            let pts: Vec<Point> = ring.into_iter().map(|e| point(e, *level)).collect();
            if pts.len() >= 3 {
                out.push((k, pts));
            }
        }
    }
    out
}

/// The segments of one marching-squares cell at `level` (corners `v`: top left, top right,
/// bottom right, bottom left), added to `out`.
fn cell(x: usize, y: usize, v: [f64; 4], level: f64, out: Option<&mut Vec<(Edge, Edge)>>) {
    let Some(out) = out else { return };
    let top = Edge { vertical: false, x, y };
    let right = Edge { vertical: true, x: x + 1, y };
    let bottom = Edge { vertical: false, x, y: y + 1 };
    let left = Edge { vertical: true, x, y };
    let inside = v.map(|c| c > level);
    let case = usize::from(inside[0]) | usize::from(inside[1]) << 1 | usize::from(inside[2]) << 2 | usize::from(inside[3]) << 3;
    // The saddles join the way the cell's middle says.
    let middle = (v[0] + v[1] + v[2] + v[3]) / 4.0 > level;
    match case {
        1 | 14 => out.push((left, top)),
        2 | 13 => out.push((top, right)),
        3 | 12 => out.push((left, right)),
        4 | 11 => out.push((right, bottom)),
        6 | 9 => out.push((top, bottom)),
        7 | 8 => out.push((left, bottom)),
        5 if middle => out.extend([(left, bottom), (top, right)]),
        5 => out.extend([(left, top), (right, bottom)]),
        10 if middle => out.extend([(left, top), (right, bottom)]),
        10 => out.extend([(left, bottom), (top, right)]),
        _ => {}
    }
}

/// The segments joined end to end into closed rings (every crossing is shared by exactly two
/// segments of a closed level set; a broken chain is kept as far as it goes).
fn link(segs: &[(Edge, Edge)]) -> Vec<Vec<Edge>> {
    let mut by_edge: std::collections::HashMap<Edge, Vec<usize>> = std::collections::HashMap::new();
    for (i, (a, b)) in segs.iter().enumerate() {
        by_edge.entry(*a).or_default().push(i);
        by_edge.entry(*b).or_default().push(i);
    }
    let mut used = vec![false; segs.len()];
    let mut rings = vec![];
    for start in 0..segs.len() {
        if used.get(start).copied().unwrap_or(true) {
            continue;
        }
        let Some(&(a, b)) = segs.get(start) else { continue };
        if let Some(u) = used.get_mut(start) {
            *u = true;
        }
        let mut ring = vec![a, b];
        let mut at = b;
        // At most every segment once.
        for _ in 0..segs.len() {
            let next = by_edge.get(&at).and_then(|l| l.iter().copied().find(|i| !used.get(*i).copied().unwrap_or(true)));
            let Some(i) = next else { break };
            if let Some(u) = used.get_mut(i) {
                *u = true;
            }
            let Some(&(p, q)) = segs.get(i) else { break };
            at = if p == at { q } else { p };
            if at == a {
                break;
            }
            ring.push(at);
        }
        rings.push(ring);
    }
    rings
}

/// The rings sewn as runs of needle points: each ring resampled to `length`, entered at its point
/// nearest the needle and sewn all the way round, then a step to the nearest ring left; a step of
/// more than [`JOIN_ROWS`] rows starts a new run.
pub fn runs(rings: Vec<Vec<Point>>, spacing: f64, length: f64) -> Vec<Vec<Point>> {
    let mut left: Vec<Vec<Point>> =
        rings.into_iter().map(|r| at_most(&close(generate::resample(&close(r), length, true)), length)).filter(|r| r.len() >= 3).collect();
    let mut out: Vec<Vec<Point>> = vec![];
    let mut needle: Option<Point> = None;
    while !left.is_empty() {
        // The ring with the point nearest the needle (the outermost first: the first ring is
        // traced first).
        let (ring, at) = match needle {
            None => (0, 0),
            Some(n) => {
                let mut best = (0, 0, f64::INFINITY);
                for (r, ring) in left.iter().enumerate() {
                    for (i, p) in ring.iter().enumerate() {
                        let d = p.distance_squared(n);
                        if d < best.2 {
                            best = (r, i, d);
                        }
                    }
                }
                (best.0, best.1)
            }
        };
        let r = left.swap_remove(ring);
        // From `at` round to `at` again (the ring's last point repeats its first).
        let body = r.get(..r.len().saturating_sub(1)).unwrap_or(&r);
        let mut sewn: Vec<Point> = body.iter().cycle().skip(at).take(body.len()).copied().collect();
        if let Some(first) = sewn.first().copied() {
            sewn.push(first);
        }
        let joins = match (needle, sewn.first()) {
            (Some(n), Some(f)) => n.distance(*f) <= JOIN_ROWS * spacing,
            _ => false,
        };
        needle = sewn.last().copied();
        match out.last_mut() {
            Some(run) if joins => run.extend(sewn),
            _ => out.push(sewn),
        }
    }
    out
}

/// Spiral fill (E8.3b): the rings of `polys` (as [`rings`]) sewn as spirals, each ring blended
/// into the next over one turn so there is no step between them. A spiral follows a chain of
/// rings, each a row inside the last; where the chain splits (lobes) or meets another (round a
/// hole), the spiral ends and the next one starts nearest the needle, as [`runs`] joins rings.
/// The outermost and innermost rings of a chain are sewn whole, so the edge and the middle are
/// covered as a contour fill covers them. None when the shape is too thin for one ring.
pub fn spiral(polys: &[Vec<Point>], rule: FillRule, spacing: f64, first: f64, length: f64) -> Option<Vec<Vec<Point>>> {
    if !length.is_finite() || length <= 0.0 {
        return None;
    }
    let found = leveled(polys, rule, spacing, first)?;
    let rings: Vec<(usize, Vec<Point>)> =
        found.into_iter().map(|(k, r)| (k, ccw(close(generate::resample(&close(r), length, true))))).filter(|(_, r)| r.len() >= 4).collect();
    let spirals: Vec<Vec<Point>> = chains(&rings, spacing).iter().map(|c| at_most(&turns(c, length), length)).collect();
    Some(join_ends(spirals, spacing))
}

/// The rings grouped into chains, outermost first: a ring's next is the one ring a level deeper
/// that lies within two rows of it all the way round. (Not the other way: near the middle the
/// rings shrink to a slit faster at their ends than a row a level.) Two such rings (lobes) end
/// the chain, and so does a ring round a hole about to meet the outer ones.
fn chains(rings: &[(usize, Vec<Point>)], spacing: f64) -> Vec<Vec<Vec<Point>>> {
    let near = 2.0 * spacing;
    let mut used = vec![false; rings.len()];
    let mut out = vec![];
    for start in 0..rings.len() {
        if used.get(start).copied().unwrap_or(true) {
            continue;
        }
        let mut chain: Vec<Vec<Point>> = vec![];
        let mut at = start;
        // Each ring joins one chain, so this ends within as many steps as there are rings.
        for _ in 0..rings.len() {
            let Some((level, ring)) = rings.get(at) else { break };
            if let Some(u) = used.get_mut(at) {
                *u = true;
            }
            chain.push(ring.clone());
            let next: Vec<usize> = (0..rings.len())
                .filter(|&j| !used.get(j).copied().unwrap_or(true))
                .filter(|&j| rings.get(j).is_some_and(|(l, r)| *l == level + 1 && within(r, ring, near)))
                .collect();
            match next.as_slice() {
                [only] => at = *only,
                _ => break,
            }
        }
        out.push(chain);
    }
    out
}

/// Whether every point of `a` lies within `d` of polyline `b` (its bounds checked first).
fn within(a: &[Point], b: &[Point], d: f64) -> bool {
    let bounds = |r: &[Point]| {
        r.iter().fold((f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY), |(x0, y0, x1, y1), p| {
            (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y))
        })
    };
    let (ax0, ay0, ax1, ay1) = bounds(a);
    let (bx0, by0, bx1, by1) = bounds(b);
    if ax0 < bx0 - d || ay0 < by0 - d || ax1 > bx1 + d || ay1 > by1 + d {
        return false;
    }
    a.iter().all(|p| b.windows(2).any(|w| to_segment(*p, w[0], w[1]) <= d * d))
}

/// The squared distance from `p` to the segment `a`–`b`.
fn to_segment(p: Point, a: Point, b: Point) -> f64 {
    let ab = b - a;
    let len = ab.hypot2();
    let t = if len > 1e-18 { ((p - a).dot(ab) / len).clamp(0.0, 1.0) } else { 0.0 };
    p.distance_squared(a + ab * t)
}

/// One chain sewn as a spiral: its first ring whole, then each ring blended into the next over a
/// turn (the point a fraction `t` round one ring moved `t` of the way to the point as far round
/// the next), then the last ring whole. Each ring starts at its point nearest the last one's start.
fn turns(chain: &[Vec<Point>], length: f64) -> Vec<Point> {
    let mut aligned: Vec<Vec<Point>> = vec![];
    for ring in chain {
        let ring = match aligned.last().and_then(|r| r.first()) {
            Some(start) => from_nearest(ring, *start),
            None => ring.clone(),
        };
        aligned.push(ring);
    }
    let Some(first) = aligned.first() else { return vec![] };
    let mut out = first.clone();
    for w in aligned.windows(2) {
        let (a, b) = (Walk::new(&w[0]), Walk::new(&w[1]));
        let n = (a.total.max(b.total) / length).ceil().clamp(3.0, 100_000.0) as usize;
        out.extend((1..=n).map(|j| j as f64 / n as f64).map(|t| a.at(t).lerp(b.at(t), t)));
    }
    if aligned.len() > 1
        && let Some(last) = aligned.last()
    {
        out.extend(last.iter().skip(1).copied());
    }
    out
}

/// Closed `ring` (last point repeating the first) started from its point nearest `p`.
fn from_nearest(ring: &[Point], p: Point) -> Vec<Point> {
    let body = ring.get(..ring.len().saturating_sub(1)).unwrap_or(ring);
    let at = body.iter().enumerate().min_by(|a, b| a.1.distance_squared(p).total_cmp(&b.1.distance_squared(p))).map_or(0, |(i, _)| i);
    close(body.iter().cycle().skip(at).take(body.len()).copied().collect())
}

/// Closed `ring` wound counter-clockwise (positive area in y-up terms), so chained rings turn
/// the same way.
fn ccw(mut ring: Vec<Point>) -> Vec<Point> {
    let area: f64 = ring.windows(2).map(|w| w[0].x * w[1].y - w[1].x * w[0].y).sum();
    if area < 0.0 {
        ring.reverse();
    }
    ring
}

/// A closed polyline measured along its length, to find the point a fraction of the way round.
struct Walk<'a> {
    pts: &'a [Point],
    lens: Vec<f64>,
    total: f64,
}

impl<'a> Walk<'a> {
    fn new(pts: &'a [Point]) -> Self {
        let mut lens = vec![0.0];
        let mut total = 0.0;
        for w in pts.windows(2) {
            total += w[0].distance(w[1]);
            lens.push(total);
        }
        Walk { pts, lens, total }
    }

    /// The point `t` (0–1) of the way round.
    fn at(&self, t: f64) -> Point {
        let goal = t.clamp(0.0, 1.0) * self.total;
        let i = self.lens.partition_point(|l| *l < goal).max(1);
        match (self.pts.get(i - 1), self.pts.get(i), self.lens.get(i - 1), self.lens.get(i)) {
            (Some(a), Some(b), Some(la), Some(lb)) if lb - la > 1e-12 => a.lerp(*b, (goal - la) / (lb - la)),
            (Some(a), ..) => *a,
            _ => Point::ZERO,
        }
    }
}

/// Open lines joined into runs: each next line is the one with an end nearest the needle (sewn
/// from that end), joined to the run when the step is at most [`JOIN_ROWS`] rows.
fn join_ends(mut left: Vec<Vec<Point>>, spacing: f64) -> Vec<Vec<Point>> {
    left.retain(|l| l.len() >= 2);
    let mut out: Vec<Vec<Point>> = vec![];
    let mut needle: Option<Point> = None;
    while !left.is_empty() {
        let (pick, reverse) = match needle {
            None => (0, false),
            Some(n) => {
                let mut best = (0, false, f64::INFINITY);
                for (i, l) in left.iter().enumerate() {
                    for (rev, end) in [(false, l.first()), (true, l.last())] {
                        if let Some(d) = end.map(|e| e.distance_squared(n))
                            && d < best.2
                        {
                            best = (i, rev, d);
                        }
                    }
                }
                (best.0, best.1)
            }
        };
        let mut line = left.swap_remove(pick);
        if reverse {
            line.reverse();
        }
        let joins = match (needle, line.first()) {
            (Some(n), Some(f)) => n.distance(*f) <= JOIN_ROWS * spacing,
            _ => false,
        };
        needle = line.last().copied();
        match out.last_mut() {
            Some(run) if joins => run.extend(line),
            _ => out.push(line),
        }
    }
    out
}

/// `line` with every stitch longer than `length` split evenly (resampling rounds the number of
/// stitches on a side, so a side a little longer than one stitch stays one).
fn at_most(line: &[Point], length: f64) -> Vec<Point> {
    let mut out: Vec<Point> = line.first().copied().into_iter().collect();
    for w in line.windows(2) {
        let (a, b) = (w[0], w[1]);
        let n = (a.distance(b) / length).ceil().clamp(1.0, 1000.0) as usize;
        out.extend((1..=n).map(|k| a.lerp(b, k as f64 / n as f64)));
    }
    out
}

/// `ring` with its first point repeated at the end.
fn close(mut ring: Vec<Point>) -> Vec<Point> {
    if let (Some(first), Some(last)) = (ring.first().copied(), ring.last().copied())
        && first.distance(last) > 1e-9
    {
        ring.push(first);
    }
    ring
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Vec<Point> {
        vec![Point::new(x, y), Point::new(x + w, y), Point::new(x + w, y + h), Point::new(x, y + h)]
    }

    /// A 20 × 10 mm rectangle at 1 mm spacing: rings half a row in, then every row, five deep;
    /// every ring is inside the shape at its depth.
    #[test]
    fn rings_follow_the_outline_a_row_apart() {
        let shape = [rect(0.0, 0.0, 20.0, 10.0)];
        let found = rings(&shape, FillRule::NonZero, 1.0, 0.5).unwrap();
        assert_eq!(found.len(), 5, "at 0.5, 1.5, 2.5, 3.5 and 4.5 mm in");
        for (k, r) in found.iter().enumerate() {
            let depth = 0.5 + k as f64;
            for p in r {
                let inside = p.x.min(20.0 - p.x).min(p.y).min(10.0 - p.y);
                assert!((inside - depth).abs() < 0.12, "ring {k}: {p:?} is {inside} in, not {depth}");
            }
        }
    }

    /// A ring round a hole, and a shape that narrows into two lobes splits into a ring each.
    #[test]
    fn holes_and_lobes_get_rings_of_their_own() {
        let holed = [rect(0.0, 0.0, 20.0, 20.0), rect(8.0, 8.0, 4.0, 4.0)];
        let holed_rings = rings(&holed, FillRule::EvenOdd, 1.0, 0.5).unwrap();
        let round_hole = holed_rings.iter().filter(|r| r.iter().all(|p| p.x > 6.0 && p.x < 14.0 && p.y > 6.0 && p.y < 14.0)).count();
        assert!(round_hole >= 1, "a ring round the hole");
        assert!(holed_rings.iter().flatten().all(|p| !(p.x > 8.0 && p.x < 12.0 && p.y > 8.0 && p.y < 12.0)), "nothing in the hole");
        // Two 10 mm squares joined by a 2 mm neck: deep rings are one per square.
        let dumbbell = [vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 4.0),
            Point::new(14.0, 4.0),
            Point::new(14.0, 0.0),
            Point::new(24.0, 0.0),
            Point::new(24.0, 10.0),
            Point::new(14.0, 10.0),
            Point::new(14.0, 6.0),
            Point::new(10.0, 6.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ]];
        let lobes = rings(&dumbbell, FillRule::NonZero, 1.0, 0.5).unwrap();
        let deep: Vec<&Vec<Point>> = lobes.iter().filter(|r| r.iter().all(|p| p.x < 9.0) || r.iter().all(|p| p.x > 15.0)).collect();
        assert!(deep.len() >= 4, "rings in each lobe: {}", deep.len());
    }

    /// Sewn, a plain shape's rings make one run, with no stitch longer than the stitch length
    /// except the short steps between rings.
    #[test]
    fn a_plain_shape_is_sewn_in_one_run() {
        let shape = [rect(0.0, 0.0, 20.0, 10.0)];
        let sewn = runs(rings(&shape, FillRule::NonZero, 0.5, 0.25).unwrap(), 0.5, 3.0);
        assert_eq!(sewn.len(), 1);
        let run = &sewn[0];
        let long: Vec<_> = run.windows(2).filter(|w| w[0].distance(w[1]) > 3.0 + 1e-6).collect();
        assert!(long.is_empty(), "every stitch at most 3 mm: {long:?}");
        assert!(run.iter().all(|p| p.x >= 0.0 && p.x <= 20.0 && p.y >= 0.0 && p.y <= 10.0));
    }

    fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Vec<Point> {
        (0..96).map(|k| k as f64 * std::f64::consts::TAU / 96.0).map(|a| Point::new(cx + rx * a.cos(), cy + ry * a.sin())).collect()
    }

    /// An oval's rings make one chain, sewn as one spiral with no stitch longer than the stitch
    /// length: no step from ring to ring. It starts on the outermost ring and ends in the middle,
    /// and every point is inside the oval.
    #[test]
    fn an_oval_is_sewn_as_one_spiral() {
        let shape = [ellipse(20.0, 15.0, 20.0, 15.0)];
        let found = leveled(&shape, FillRule::NonZero, 1.0, 0.5).unwrap();
        let resampled: Vec<(usize, Vec<Point>)> = found.into_iter().map(|(k, r)| (k, ccw(close(generate::resample(&close(r), 2.0, true))))).collect();
        assert_eq!(chains(&resampled, 1.0).len(), 1, "one chain, outer ring to middle");
        let sewn = spiral(&shape, FillRule::NonZero, 1.0, 0.5, 2.0).unwrap();
        assert_eq!(sewn.len(), 1);
        let run = &sewn[0];
        let long: Vec<_> = run.windows(2).filter(|w| w[0].distance(w[1]) > 2.0 + 1e-6).collect();
        assert!(long.is_empty(), "every stitch at most 2 mm: {long:?}");
        let rim = |p: &Point| ((p.x - 20.0) / 20.0).powi(2) + ((p.y - 15.0) / 15.0).powi(2);
        assert!(run.iter().all(|p| rim(p) < 1.0), "inside the oval");
        let (Some(start), Some(end)) = (run.first(), run.last()) else { panic!() };
        assert!(rim(start) > 0.85, "starts at the edge: {start:?}");
        assert!(rim(end) < 0.2, "ends in the middle: {end:?}");
        // The needle keeps moving inward: the distance in from the edge only grows, turn by turn.
        let depth = |p: &Point| 1.0 - rim(p).sqrt();
        let quarter = run.len() / 4;
        assert!(depth(&run[quarter]) < depth(&run[3 * quarter]));
    }

    /// Two lobes and a hole break the chains; the spirals are joined nearest first and stay out
    /// of the hole.
    #[test]
    fn lobes_and_holes_break_the_spiral() {
        let two = [ellipse(10.0, 10.0, 10.0, 10.0), ellipse(30.0, 10.0, 10.0, 10.0)];
        let sewn = spiral(&two, FillRule::NonZero, 1.0, 0.5, 3.0).unwrap();
        let points: usize = sewn.iter().map(Vec::len).sum();
        assert!(points > 100);
        assert!(sewn.iter().flatten().all(|p| p.distance(Point::new(10.0, 10.0)) < 10.0 || p.distance(Point::new(30.0, 10.0)) < 10.0));
        let holed = [rect(0.0, 0.0, 20.0, 20.0), rect(8.0, 8.0, 4.0, 4.0)];
        let sewn = spiral(&holed, FillRule::EvenOdd, 1.0, 0.5, 3.0).unwrap();
        assert!(sewn.iter().flatten().all(|p| !(p.x > 8.0 && p.x < 12.0 && p.y > 8.0 && p.y < 12.0)), "nothing in the hole");
        assert!(sewn.iter().flatten().any(|p| p.x > 6.0 && p.x < 14.0 && p.y > 6.0 && p.y < 14.0), "sewn round the hole");
    }

    /// Nothing to fill, a shape too thin for the first ring, and bad spacings.
    #[test]
    fn odd_shapes_and_settings() {
        assert_eq!(rings(&[], FillRule::NonZero, 1.0, 0.5), None);
        let thin = [rect(0.0, 0.0, 20.0, 0.6)];
        let one = rings(&thin, FillRule::NonZero, 1.0, 0.5).unwrap();
        assert_eq!(one.len(), 1, "one ring down the middle");
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(rings(&[rect(0.0, 0.0, 5.0, 5.0)], FillRule::NonZero, bad, 0.5), None, "{bad}");
        }
        assert!(runs(vec![], 1.0, 3.0).is_empty());
        assert_eq!(spiral(&[], FillRule::NonZero, 1.0, 0.5, 3.0), None);
        assert_eq!(spiral(&thin, FillRule::NonZero, 1.0, 0.5, 3.0).map(|s| s.len()), Some(1), "one ring down the middle");
        for bad in [0.0, -1.0, f64::NAN] {
            assert_eq!(spiral(&[rect(0.0, 0.0, 5.0, 5.0)], FillRule::NonZero, 1.0, 0.5, bad), None, "{bad}");
        }
    }
}
