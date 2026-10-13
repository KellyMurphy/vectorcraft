//! Centre lines of a shape: its strokes, for satin that follows them (lettering).
//!
//! The shape is rasterized finely, each inside pixel's distance to the edge is measured (an
//! exact Euclidean distance transform), and the pixels are thinned to a one-pixel skeleton
//! (Zhang–Suen). The skeleton is traced into strokes between its ends and junctions; short spurs
//! (thinning noise at corners) are pruned; strokes are smoothed, and those that end at a tip of
//! the shape are extended to reach it (thinning stops a stroke's half-width short of its tip).
//! Each stroke point keeps the distance to the edge there: half the stroke's width.

use kurbo::{Point, Vec2};
use vectorcraft_geom::FillRule;

use crate::generate::row_spans;

/// The most pixels a side of the raster may have.
const MAX_SIDE: f64 = 700.0;
/// The finest raster, mm per pixel.
const MIN_PIXEL: f64 = 0.05;
/// Spurs shorter than this many times the stroke radius where they join are pruned.
const SPUR: f64 = 1.6;

/// A centre line: points (mm) and the distance from each to the shape's edge (mm).
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub points: Vec<Point>,
    pub radius: Vec<f64>,
    /// How far (mm) from its start and end the stroke runs inside another stroke: it joins a
    /// junction that two longer strokes pass through, so its satin starts at their edge.
    pub trim: (f64, f64),
}

/// A binary raster of the shape with its distance transform.
struct Grid {
    w: usize,
    h: usize,
    /// mm per pixel, and the mm position of pixel (0, 0)'s centre.
    px: f64,
    origin: Point,
    on: Vec<bool>,
}

impl Grid {
    fn get(&self, x: isize, y: isize) -> bool {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return false;
        }
        self.on.get(y as usize * self.w + x as usize).copied().unwrap_or(false)
    }

    fn set(&mut self, x: usize, y: usize, v: bool) {
        if let Some(c) = self.on.get_mut(y * self.w + x) {
            *c = v;
        }
    }

    fn mm(&self, x: usize, y: usize) -> Point {
        Point::new(self.origin.x + x as f64 * self.px, self.origin.y + y as f64 * self.px)
    }

    /// The 8 neighbours, clockwise from north: N, NE, E, SE, S, SW, W, NW.
    fn ring(&self, x: usize, y: usize) -> [bool; 8] {
        let (x, y) = (x as isize, y as isize);
        [
            self.get(x, y - 1),
            self.get(x + 1, y - 1),
            self.get(x + 1, y),
            self.get(x + 1, y + 1),
            self.get(x, y + 1),
            self.get(x - 1, y + 1),
            self.get(x - 1, y),
            self.get(x - 1, y - 1),
        ]
    }

    fn neighbours(&self, x: usize, y: usize) -> Vec<(usize, usize)> {
        const D: [(isize, isize); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];
        D.iter().map(|(dx, dy)| (x as isize + dx, y as isize + dy)).filter(|&(a, b)| self.get(a, b)).map(|(a, b)| (a as usize, b as usize)).collect()
    }
}

/// The shape `polys` (filled by `rule`) rasterized; none when it is empty or degenerate.
fn rasterize(polys: &[Vec<Point>], rule: FillRule) -> Option<Grid> {
    let (mut lo, mut hi) = (Point::new(f64::INFINITY, f64::INFINITY), Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
    for p in polys.iter().flatten() {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let size = (hi.x - lo.x).max(hi.y - lo.y);
    if !size.is_finite() || size <= 0.0 {
        return None;
    }
    let px = (size / MAX_SIDE).max(MIN_PIXEL);
    // A pixel of margin all round, so the shape's edge is always background.
    let w = ((hi.x - lo.x) / px).ceil() as usize + 3;
    let h = ((hi.y - lo.y) / px).ceil() as usize + 3;
    let origin = Point::new(lo.x - px, lo.y - px);
    let mut g = Grid { w, h, px, origin, on: vec![false; w * h] };
    for y in 0..h {
        let cy = origin.y + y as f64 * px;
        for (a, b) in row_spans(polys, rule, cy) {
            let x0 = ((a - origin.x) / px).ceil().max(0.0) as usize;
            let x1 = ((b - origin.x) / px).floor().min(w as f64 - 1.0).max(-1.0);
            if x1 < 0.0 {
                continue;
            }
            for x in x0..=(x1 as usize) {
                g.set(x, y, true);
            }
        }
    }
    Some(g)
}

/// 1-D squared distance transform of `f` (Felzenszwalb–Huttenlocher).
fn edt_1d(f: &[f64]) -> Vec<f64> {
    let n = f.len();
    let mut d = vec![0.0; n];
    let mut v = vec![0usize; n];
    let mut z = vec![0.0; n + 1];
    let mut k = 0usize;
    let fv = |i: usize| f.get(i).copied().unwrap_or(f64::INFINITY);
    let first = (0..n).find(|&i| fv(i).is_finite());
    let Some(first) = first else { return vec![f64::INFINITY; n] };
    if let Some(s) = v.get_mut(0) {
        *s = first;
    }
    if let (Some(a), Some(b)) = (z.get_mut(0), Some(f64::NEG_INFINITY)) {
        *a = b;
    }
    if let Some(a) = z.get_mut(1) {
        *a = f64::INFINITY;
    }
    for q in (first + 1)..n {
        if !fv(q).is_finite() {
            continue;
        }
        loop {
            let vk = v.get(k).copied().unwrap_or(0);
            let s = ((fv(q) + (q * q) as f64) - (fv(vk) + (vk * vk) as f64)) / (2.0 * q as f64 - 2.0 * vk as f64);
            if k > 0 && s <= z.get(k).copied().unwrap_or(f64::NEG_INFINITY) {
                k -= 1;
                continue;
            }
            k += 1;
            if let Some(a) = v.get_mut(k) {
                *a = q;
            }
            if let Some(a) = z.get_mut(k) {
                *a = s;
            }
            if let Some(a) = z.get_mut(k + 1) {
                *a = f64::INFINITY;
            }
            break;
        }
    }
    let mut k = 0usize;
    for (q, out) in d.iter_mut().enumerate() {
        while z.get(k + 1).is_some_and(|&b| b < q as f64) {
            k += 1;
        }
        let vk = v.get(k).copied().unwrap_or(0);
        *out = (q as f64 - vk as f64).powi(2) + fv(vk);
    }
    d
}

/// Distance (pixels) from each inside pixel to the nearest outside pixel.
fn distances(g: &Grid) -> Vec<f64> {
    let (w, h) = (g.w, g.h);
    let mut d: Vec<f64> = g.on.iter().map(|&on| if on { f64::INFINITY } else { 0.0 }).collect();
    for x in 0..w {
        let col: Vec<f64> = (0..h).map(|y| d.get(y * w + x).copied().unwrap_or(0.0)).collect();
        for (y, v) in edt_1d(&col).into_iter().enumerate() {
            if let Some(c) = d.get_mut(y * w + x) {
                *c = v;
            }
        }
    }
    for y in 0..h {
        let row: Vec<f64> = d.get(y * w..(y + 1) * w).map(<[f64]>::to_vec).unwrap_or_default();
        for (x, v) in edt_1d(&row).into_iter().enumerate() {
            if let Some(c) = d.get_mut(y * w + x) {
                *c = v.sqrt();
            }
        }
    }
    d
}

/// Zhang–Suen thinning, in place.
fn thin(g: &mut Grid) {
    let mut changed = true;
    let mut rounds = 0;
    while changed && rounds < g.w.max(g.h) {
        changed = false;
        rounds += 1;
        for step in 0..2 {
            let mut gone = vec![];
            for y in 0..g.h {
                for x in 0..g.w {
                    if !g.get(x as isize, y as isize) {
                        continue;
                    }
                    let r = g.ring(x, y);
                    let b = r.iter().filter(|&&v| v).count();
                    if !(2..=6).contains(&b) {
                        continue;
                    }
                    // 0→1 transitions round the ring.
                    let a = (0..8).filter(|&i| !r[i] && r[(i + 1) % 8]).count();
                    if a != 1 {
                        continue;
                    }
                    let (n, e, s, w) = (r[0], r[2], r[4], r[6]);
                    let ok = if step == 0 { !(n && e && s) && !(e && s && w) } else { !(n && e && w) && !(n && s && w) };
                    if ok {
                        gone.push((x, y));
                    }
                }
            }
            changed |= !gone.is_empty();
            for (x, y) in gone {
                g.set(x, y, false);
            }
        }
    }
    // Staircase pixels: a pixel whose removal keeps the line connected (it touches two
    // 4-neighbours that touch each other diagonally) only makes false junctions.
    for y in 0..g.h {
        for x in 0..g.w {
            if !g.get(x as isize, y as isize) {
                continue;
            }
            let r = g.ring(x, y);
            let (n, ne, e, se, s, sw, w, nw) = (r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7]);
            let corner = (n && e && !sw && !s && !w) || (e && s && !nw && !n && !w) || (s && w && !ne && !n && !e) || (w && n && !se && !s && !e);
            if corner && r.iter().filter(|&&v| v).count() == 2 {
                g.set(x, y, false);
            }
        }
    }
}

/// A path of skeleton pixels between two nodes (ends or junctions), or round a loop.
struct Path {
    px: Vec<(usize, usize)>,
    /// The nodes at its start and end (indexes into the node degrees); none for a loop.
    ends: Option<(usize, usize)>,
}

/// The skeleton's paths and how many path ends meet at each node. Touching junction pixels form
/// one node (thinning leaves small clumps), and a "node" where just two paths meet is no node:
/// the paths are joined through it.
fn trace(g: &Grid) -> (Vec<Path>, Vec<usize>) {
    let w = g.w;
    let idx = |x: usize, y: usize| y * w + x;
    let deg = |x: usize, y: usize| g.neighbours(x, y).len();
    let on = |x: usize, y: usize| g.get(x as isize, y as isize);
    // Nodes: clusters of touching pixels that aren't plain path pixels (exactly 2 neighbours).
    let mut node_of: Vec<Option<usize>> = vec![None; g.w * g.h];
    let mut nodes = 0usize;
    for y in 0..g.h {
        for x in 0..g.w {
            if !on(x, y) || deg(x, y) == 2 || node_of.get(idx(x, y)).copied().flatten().is_some() {
                continue;
            }
            let id = nodes;
            nodes += 1;
            let mut stack = vec![(x, y)];
            while let Some((cx, cy)) = stack.pop() {
                match node_of.get_mut(idx(cx, cy)) {
                    Some(slot @ None) => *slot = Some(id),
                    _ => continue,
                }
                stack.extend(g.neighbours(cx, cy).into_iter().filter(|&(nx, ny)| deg(nx, ny) != 2));
            }
        }
    }
    let node = |x: usize, y: usize| node_of.get(idx(x, y)).copied().flatten();
    let mut used = vec![false; g.w * g.h];
    let mut paths: Vec<Path> = vec![];
    for y in 0..g.h {
        for x in 0..g.w {
            let Some(start) = node(x, y) else { continue };
            for (nx, ny) in g.neighbours(x, y) {
                if node(nx, ny).is_some() || used.get(idx(nx, ny)).copied().unwrap_or(true) {
                    continue;
                }
                let mut px = vec![(x, y), (nx, ny)];
                if let Some(u) = used.get_mut(idx(nx, ny)) {
                    *u = true;
                }
                let (mut prev, mut cur) = ((x, y), (nx, ny));
                let mut end = None;
                loop {
                    // Leaving the start node, don't step straight back into it.
                    let next = g.neighbours(cur.0, cur.1).into_iter().find(|&p| {
                        p != prev
                            && match node(p.0, p.1) {
                                Some(n) => !(n == start && px.len() <= 2),
                                None => !used.get(idx(p.0, p.1)).copied().unwrap_or(true),
                            }
                    });
                    let Some(p) = next else { break };
                    px.push(p);
                    if let Some(n) = node(p.0, p.1) {
                        end = Some(n);
                        break;
                    }
                    if let Some(u) = used.get_mut(idx(p.0, p.1)) {
                        *u = true;
                    }
                    (prev, cur) = (cur, p);
                }
                // A path that runs out without reaching a node ends where it stops (shouldn't
                // happen on a clean skeleton): count that pixel as its own end.
                let end = match end {
                    Some(e) => e,
                    None => {
                        nodes += 1;
                        nodes - 1
                    }
                };
                paths.push(Path { px, ends: Some((start, end)) });
            }
        }
    }
    // Loops with no node on them (an O).
    for y in 0..g.h {
        for x in 0..g.w {
            if !on(x, y) || node(x, y).is_some() || used.get(idx(x, y)).copied().unwrap_or(true) {
                continue;
            }
            let mut px = vec![(x, y)];
            if let Some(u) = used.get_mut(idx(x, y)) {
                *u = true;
            }
            let mut cur = (x, y);
            while let Some(p) =
                g.neighbours(cur.0, cur.1).into_iter().find(|p| node(p.0, p.1).is_none() && !used.get(idx(p.0, p.1)).copied().unwrap_or(true))
            {
                if let Some(u) = used.get_mut(idx(p.0, p.1)) {
                    *u = true;
                }
                px.push(p);
                cur = p;
            }
            px.push((x, y));
            paths.push(Path { px, ends: None });
        }
    }
    // Join paths through nodes where exactly two path ends meet.
    loop {
        let mut degree = vec![0usize; nodes];
        for p in &paths {
            if let Some((a, b)) = p.ends {
                for n in [a, b] {
                    if let Some(d) = degree.get_mut(n) {
                        *d += 1;
                    }
                }
            }
        }
        let Some(n) = (0..nodes).find(|&n| degree.get(n) == Some(&2)) else { return (paths, degree) };
        let at: Vec<usize> = (0..paths.len()).filter(|&i| paths.get(i).and_then(|p| p.ends).is_some_and(|(a, b)| a == n || b == n)).collect();
        match at.as_slice() {
            // Both ends of one path: it is a loop.
            [i] => {
                if let Some(p) = paths.get_mut(*i) {
                    p.ends = None;
                    if let Some(&f) = p.px.first() {
                        p.px.push(f);
                    }
                }
            }
            [i, j] => {
                let (i, j) = (*i.min(j), *i.max(j));
                let second = paths.remove(j);
                let Some(first) = paths.get_mut(i) else { return (paths, degree) };
                let (Some((a1, b1)), Some((a2, b2))) = (first.ends, second.ends) else { return (paths, degree) };
                // Orient so the first ends at n and the second starts there.
                if a1 == n {
                    first.px.reverse();
                }
                let (far1, mut tail, far2) = if a2 == n {
                    (if a1 == n { b1 } else { a1 }, second.px, b2)
                } else {
                    (if a1 == n { b1 } else { a1 }, second.px.into_iter().rev().collect(), a2)
                };
                if !tail.is_empty() {
                    tail.remove(0);
                }
                first.px.extend(tail);
                first.ends = Some((far1, far2));
            }
            _ => return (paths, degree),
        }
    }
}

/// Moving average over `points` (ends kept; a closed path wraps).
fn smooth(points: &[Point], closed: bool, half: usize) -> Vec<Point> {
    // A closed path repeats its first point at the end: smooth the ring, then close it again.
    if closed && points.len() > 3 && points.first() == points.last() {
        let mut ring = smooth(points.get(..points.len() - 1).unwrap_or_default(), true, half);
        if let Some(&f) = ring.first() {
            ring.push(f);
        }
        return ring;
    }
    let n = points.len();
    if n < 3 || half == 0 {
        return points.to_vec();
    }
    (0..n)
        .map(|i| {
            if !closed && (i == 0 || i == n - 1) {
                return points.get(i).copied().unwrap_or(Point::ZERO);
            }
            let reach = if closed { half } else { half.min(i).min(n - 1 - i) };
            let mut sum = Vec2::ZERO;
            let mut count = 0.0;
            for k in 0..=2 * reach {
                let j = if closed { (i + n - reach + k) % n } else { i - reach + k };
                if let Some(p) = points.get(j) {
                    sum += p.to_vec2();
                    count += 1.0;
                }
            }
            if count > 0.0 { (sum / count).to_point() } else { points.get(i).copied().unwrap_or(Point::ZERO) }
        })
        .collect()
}

/// Distance from `o` along unit `d` to the shape's edge (none within `max`).
pub fn cast(polys: &[Vec<Point>], o: Point, d: Vec2, max: f64) -> Option<f64> {
    let mut best: Option<f64> = None;
    for poly in polys {
        let n = poly.len();
        for k in 0..n {
            let (Some(&a), Some(&b)) = (poly.get(k), poly.get((k + 1) % n)) else { continue };
            let e = b - a;
            let den = d.cross(e);
            if den.abs() < 1e-12 {
                continue;
            }
            let t = (a - o).cross(e) / den;
            let u = (a - o).cross(d) / den;
            if t > 1e-9 && (0.0..=1.0).contains(&u) && t <= max && best.is_none_or(|b| t < b) {
                best = Some(t);
            }
        }
    }
    best
}

/// The nearest point of the shape's edge to `o` on the side `d` points to (ahead of `o`), within
/// `max`.
pub fn nearest(polys: &[Vec<Point>], o: Point, d: Vec2, max: f64) -> Option<Point> {
    let mut best: Option<(f64, Point)> = None;
    for poly in polys {
        let n = poly.len();
        for k in 0..n {
            let (Some(&a), Some(&b)) = (poly.get(k), poly.get((k + 1) % n)) else { continue };
            let e = b - a;
            let l2 = e.hypot2();
            let t = if l2 > 0.0 { ((o - a).dot(e) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let q = a + e * t;
            let dist = q.distance(o);
            if (q - o).dot(d) > 1e-9 && dist <= max && best.is_none_or(|(bd, _)| dist < bd) {
                best = Some((dist, q));
            }
        }
    }
    best.map(|(_, q)| q)
}

/// The strokes of the shape `polys` (mm, filled by `rule`).
pub fn strokes(polys: &[Vec<Point>], rule: FillRule) -> Vec<Stroke> {
    let Some(mut g) = rasterize(polys, rule) else { return vec![] };
    let dist = distances(&g);
    let radius_at = |g: &Grid, (x, y): (usize, usize)| dist.get(y * g.w + x).copied().unwrap_or(0.0) * g.px;
    thin(&mut g);
    // Prune spurs: a path from a free end to a junction, shorter than the junction is thick.
    let (mut paths, mut degree) = trace(&g);
    for _ in 0..4 {
        // Spur candidates by the junction they hang from: (junction, length, path).
        let mut spurs: Vec<(usize, f64, usize)> = vec![];
        for (i, p) in paths.iter().enumerate() {
            let Some((a, b)) = p.ends else { continue };
            let d = |n: usize| degree.get(n).copied().unwrap_or(0);
            let (node, join) = match (d(a), d(b)) {
                (1, j) if j >= 3 => (b, p.px.last()),
                (j, 1) if j >= 3 => (a, p.px.first()),
                _ => continue,
            };
            let Some(&join) = join else { continue };
            let len = p.px.len() as f64 * g.px;
            if len < SPUR * radius_at(&g, join) {
                spurs.push((node, len, i));
            }
        }
        // Each junction keeps at least two branches: prune its shortest spurs only, so a fat
        // shape (whose every branch looks like a spur) keeps its main stroke.
        spurs.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.total_cmp(&y.1)));
        let mut pruned = false;
        let mut taken: Vec<(usize, usize)> = vec![];
        for (node, _, i) in spurs {
            let done = taken.iter().find(|(n, _)| *n == node).map_or(0, |(_, c)| *c);
            if done + 2 >= degree.get(node).copied().unwrap_or(0) {
                continue;
            }
            match taken.iter_mut().find(|(n, _)| *n == node) {
                Some((_, c)) => *c += 1,
                None => taken.push((node, 1)),
            }
            let Some(p) = paths.get(i) else { continue };
            let join = if p.ends.is_some_and(|(a, _)| degree.get(a) == Some(&1)) { p.px.last() } else { p.px.first() };
            for &px in p.px.iter().filter(|&q| Some(q) != join) {
                g.set(px.0, px.1, false);
            }
            pruned = true;
        }
        if !pruned {
            break;
        }
        (paths, degree) = trace(&g);
    }
    let paths: Vec<Path> = paths.into_iter().filter(|p| p.px.len() >= 2).collect();
    // At each junction the two longest strokes pass through; the others are trimmed back by the
    // junction's half-width (where they would only sew over the strokes passing through).
    let mut trims = vec![(0.0, 0.0); paths.len()];
    for node in 0..degree.len() {
        if degree.get(node).copied().unwrap_or(0) < 3 {
            continue;
        }
        let mut ends: Vec<(usize, bool, usize)> = vec![];
        for (i, p) in paths.iter().enumerate() {
            if let Some((a, b)) = p.ends {
                if a == node {
                    ends.push((i, true, p.px.len()));
                }
                if b == node {
                    ends.push((i, false, p.px.len()));
                }
            }
        }
        ends.sort_by_key(|e| std::cmp::Reverse(e.2));
        for &(i, at_start, _) in ends.iter().skip(2) {
            let Some(p) = paths.get(i) else { continue };
            let at = if at_start { p.px.first() } else { p.px.last() };
            let r = at.map_or(0.0, |&q| radius_at(&g, q));
            if let Some(t) = trims.get_mut(i) {
                if at_start { t.0 = r } else { t.1 = r }
            }
        }
    }
    paths
        .into_iter()
        .zip(trims)
        .map(|(p, trim)| {
            let pts: Vec<Point> = p.px.iter().map(|&(x, y)| g.mm(x, y)).collect();
            let radius: Vec<f64> = p.px.iter().map(|&q| radius_at(&g, q)).collect();
            let closed = p.ends.is_none();
            let mut s = Stroke { points: smooth(&pts, closed, 3), radius, trim };
            if let Some((a, b)) = p.ends {
                // Thinning stops a stroke short of a tip: reach out to the edge.
                if degree.get(b) == Some(&1) {
                    extend(&mut s, polys, false);
                }
                if degree.get(a) == Some(&1) {
                    extend(&mut s, polys, true);
                }
            }
            s
        })
        .collect()
}

/// Extend the stroke's start (`at_start`) or end toward the shape's edge.
fn extend(s: &mut Stroke, polys: &[Vec<Point>], at_start: bool) {
    let n = s.points.len();
    if n < 2 {
        return;
    }
    let (end, back) = if at_start { (0, (n - 1).min(4)) } else { (n - 1, n.saturating_sub(5)) };
    let (Some(&p), Some(&q), Some(&r)) = (s.points.get(end), s.points.get(back), s.radius.get(end)) else { return };
    let d = p - q;
    let l = d.hypot();
    if l <= 0.0 {
        return;
    }
    let d = d / l;
    let reach = cast(polys, p, d, 2.0 * r.max(0.05)).unwrap_or(r);
    // Stop a little inside the edge, so the satin's end stitch covers it without poking out.
    let reach = (reach - 0.15).max(0.0);
    if reach <= 0.05 {
        return;
    }
    let tip = p + d * reach;
    let rt = (r * 0.5).max(0.05);
    if at_start {
        s.points.insert(0, tip);
        s.radius.insert(0, rt);
    } else {
        s.points.push(tip);
        s.radius.push(rt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![Point::new(x0, y0), Point::new(x1, y0), Point::new(x1, y1), Point::new(x0, y1)]
    }

    #[test]
    fn a_bar_has_one_stroke_down_its_middle_reaching_its_ends() {
        let s = strokes(&[rect(0.0, 0.0, 30.0, 4.0)], FillRule::NonZero);
        assert_eq!(s.len(), 1, "{s:?}");
        let st = &s[0];
        for (p, r) in st.points.iter().zip(&st.radius) {
            assert!((p.y - 2.0).abs() < 0.3, "{p:?} off the middle");
            assert!(*r <= 2.2);
        }
        let xs: Vec<f64> = st.points.iter().map(|p| p.x).collect();
        let (lo, hi) = (xs.iter().cloned().fold(f64::INFINITY, f64::min), xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
        assert!(lo < 1.0 && hi > 29.0, "reaches {lo}..{hi}");
    }

    #[test]
    fn a_t_has_three_strokes_meeting() {
        let t = [rect(0.0, 0.0, 30.0, 4.0), rect(13.0, 4.0, 17.0, 30.0)];
        // Two overlapping-free rectangles touching: one T shape.
        let s = strokes(&t, FillRule::NonZero);
        assert!((2..=3).contains(&s.len()), "{}", s.len());
        let len: f64 = s.iter().map(|st| st.points.windows(2).map(|w| w[0].distance(w[1])).sum::<f64>()).sum();
        assert!(len > 50.0 && len < 62.0, "total {len}");
    }

    #[test]
    fn a_ring_is_one_closed_stroke() {
        let outer = rect(0.0, 0.0, 20.0, 20.0);
        let inner: Vec<Point> = rect(4.0, 4.0, 16.0, 16.0).into_iter().rev().collect();
        let s = strokes(&[outer, inner], FillRule::NonZero);
        assert_eq!(s.len(), 1, "{}", s.len());
        assert_eq!(s[0].points.first(), s[0].points.last());
    }

    #[test]
    fn rays_hit_the_nearest_edge() {
        let r = [rect(0.0, 0.0, 10.0, 4.0)];
        assert_eq!(cast(&r, Point::new(5.0, 2.0), Vec2::new(0.0, 1.0), 10.0), Some(2.0));
        assert_eq!(cast(&r, Point::new(5.0, 2.0), Vec2::new(0.0, 1.0), 1.0), None);
    }

    #[test]
    fn degenerate_shapes_have_no_strokes() {
        assert!(strokes(&[], FillRule::NonZero).is_empty());
        assert!(strokes(&[vec![Point::new(1.0, 1.0); 3]], FillRule::NonZero).is_empty());
    }
}
