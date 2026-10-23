//! Branching: running-stitch lines that meet (end to end, or an end on another line) are sewn as
//! one network in one pass, each line once, the way back along a branch sewn over the stitches
//! already there (travel doubled under it), so a tree or a mesh of lines needs no jumps or trims.

use kurbo::Point;

/// Ends closer than this to each other or to another line meet there, mm.
pub const MEET: f64 = 0.25;
/// Networks of more lines or points than these are sewn line by line (the search would be slow).
const MAX_LINES: usize = 1000;
const MAX_POINTS: usize = 20_000;

/// One stretch between two meeting points, stitched.
struct Edge {
    a: usize,
    b: usize,
    /// Its needle points from `a` to `b`.
    pts: Vec<Point>,
    len: f64,
}

/// Find the node at `p` among `nodes`, or add one.
fn node(nodes: &mut Vec<Point>, p: Point) -> usize {
    if let Some(i) = nodes.iter().position(|q| q.distance(p) <= MEET) {
        return i;
    }
    nodes.push(p);
    nodes.len() - 1
}

/// The point of the polyline `l` nearest `p`: (segment index, point, distance).
fn nearest_on(l: &[Point], p: Point) -> Option<(usize, Point, f64)> {
    let mut best: Option<(usize, Point, f64)> = None;
    for (k, w) in l.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        let ab = b - a;
        let t = if ab.hypot2() > 0.0 { ((p - a).dot(ab) / ab.hypot2()).clamp(0.0, 1.0) } else { 0.0 };
        let q = a + ab * t;
        let d = q.distance(p);
        if best.is_none_or(|(_, _, bd)| d < bd) {
            best = Some((k, q, d));
        }
    }
    best
}

/// `lines` (polylines in mm, with whether each is closed) as the runs to sew: each connected
/// network as one run, sewn by `stitch` (a stretch's needle points from its polyline) and, where
/// the run sews a stretch for the first time, `bold` (the bean's repeats; the way back along a
/// stretch goes over its points once). Lines that meet nothing are each a run of their own, as
/// before. `None`: nothing meets (or too many lines), sew the lines one by one.
pub fn runs(lines: &[(Vec<Point>, bool)], stitch: &dyn Fn(&[Point]) -> Vec<Point>, bold: &dyn Fn(&[Point]) -> Vec<Point>) -> Option<Vec<Vec<Point>>> {
    if lines.len() < 2 || lines.len() > MAX_LINES || lines.iter().map(|(l, _)| l.len()).sum::<usize>() > MAX_POINTS {
        return None;
    }
    // Close each closed line, and split lines where another line's end touches them.
    let mut polys: Vec<Vec<Point>> = lines
        .iter()
        .map(|(l, closed)| {
            let mut l = l.clone();
            if *closed
                && let (Some(&f), Some(&e)) = (l.first(), l.last())
                && f.distance(e) > 1e-9
            {
                l.push(f);
            }
            l
        })
        .filter(|l| l.len() >= 2)
        .collect();
    let ends: Vec<(usize, Point)> = polys
        .iter()
        .enumerate()
        .zip(lines)
        .filter(|(_, (_, closed))| !closed)
        .flat_map(|((i, l), _)| [l.first().copied(), l.last().copied()].into_iter().flatten().map(move |p| (i, p)))
        .collect();
    let mut touched = false;
    for (owner, p) in &ends {
        for (j, l) in polys.iter_mut().enumerate() {
            if j == *owner {
                continue;
            }
            let Some((k, q, d)) = nearest_on(l, *p) else { continue };
            if d > MEET {
                continue;
            }
            touched = true;
            // Mid-line (not at its ends): a vertex there, so the line splits.
            let at_end = l.first().is_some_and(|f| f.distance(q) <= MEET) || l.last().is_some_and(|e| e.distance(q) <= MEET);
            let at_vertex = l.iter().any(|v| v.distance(q) <= 1e-6);
            if !at_end && !at_vertex {
                l.insert(k + 1, q);
            }
        }
    }
    // Ends meeting ends count too.
    if !touched {
        touched = ends.iter().enumerate().any(|(i, (oi, p))| ends.iter().skip(i + 1).any(|(oj, q)| oi != oj && p.distance(*q) <= MEET));
    }
    if !touched {
        return None;
    }
    // Nodes: every line's ends, and every vertex another line's end lands on.
    let mut nodes: Vec<Point> = vec![];
    for l in &polys {
        for p in [l.first(), l.last()].into_iter().flatten() {
            node(&mut nodes, *p);
        }
    }
    for (_, p) in &ends {
        node(&mut nodes, *p);
    }
    let mut edges: Vec<Edge> = vec![];
    for l in &polys {
        // Split at every vertex that is a node (but the first).
        let mut cur: Vec<Point> = vec![];
        let mut from: Option<usize> = None;
        for (k, &p) in l.iter().enumerate() {
            cur.push(p);
            let at = nodes.iter().position(|q| q.distance(p) <= MEET);
            if k == 0 {
                from = Some(at.unwrap_or_else(|| node(&mut nodes, p)));
                continue;
            }
            if let (Some(to), Some(a)) = (at.or_else(|| (k + 1 == l.len()).then(|| node(&mut nodes, p))), from)
                && cur.len() >= 2
            {
                let pts = stitch(&cur);
                let len = cur.windows(2).map(|w| w[0].distance(w[1])).sum();
                if pts.len() >= 2 {
                    edges.push(Edge { a, b: to, pts, len });
                }
                cur = vec![p];
                from = Some(to);
            }
        }
    }
    // Which edges meet at each node.
    let mut at: Vec<Vec<usize>> = vec![vec![]; nodes.len()];
    for (e, edge) in edges.iter().enumerate() {
        if let Some(v) = at.get_mut(edge.a) {
            v.push(e);
        }
        if edge.b != edge.a
            && let Some(v) = at.get_mut(edge.b)
        {
            v.push(e);
        }
    }
    // Shortest first at each node: the longest branch comes last, where the run can end.
    for v in &mut at {
        v.sort_by(|x, y| edges.get(*x).map(|e| e.len).unwrap_or(0.0).total_cmp(&edges.get(*y).map(|e| e.len).unwrap_or(0.0)));
    }
    let mut done = vec![false; edges.len()];
    let mut out = vec![];
    // Each network, from an end of a branch (a node with one line) where there is one.
    while let Some(first) = (0..edges.len()).find(|e| !done[*e]) {
        let start = {
            let mut seen = vec![false; nodes.len()];
            let mut todo = vec![edges[first].a];
            let mut leaf = None;
            while let Some(n) = todo.pop() {
                if std::mem::replace(&mut seen[n], true) {
                    continue;
                }
                let degree = at[n].len();
                if degree == 1 && leaf.is_none() {
                    leaf = Some(n);
                }
                for e in &at[n] {
                    let other = if edges[*e].a == n { edges[*e].b } else { edges[*e].a };
                    todo.push(other);
                }
            }
            leaf.unwrap_or(edges[first].a)
        };
        let mut run: Vec<Point> = vec![nodes[start]];
        // Where the last newly sewn stretch ended in `run`: travel after it isn't needed.
        let mut sewn_to = 1usize;
        // (node, the edge and direction it was reached by).
        let mut stack: Vec<(usize, Option<(usize, bool)>)> = vec![(start, None)];
        while let Some(&(n, via)) = stack.last() {
            let next = at[n].iter().copied().find(|e| !done[*e]);
            match next {
                Some(e) => {
                    done[e] = true;
                    let forward = edges[e].a == n;
                    let mut pts = edges[e].pts.clone();
                    if !forward {
                        pts.reverse();
                    }
                    run.extend(bold(&pts).into_iter().skip(1));
                    sewn_to = run.len();
                    let other = if forward { edges[e].b } else { edges[e].a };
                    stack.push((other, Some((e, forward))));
                }
                None => {
                    stack.pop();
                    // Back the way it came, over the stitches there.
                    if let Some((e, forward)) = via {
                        let mut pts = edges[e].pts.clone();
                        if forward {
                            pts.reverse();
                        }
                        run.extend(pts.into_iter().skip(1));
                    }
                }
            }
        }
        run.truncate(sewn_to);
        if run.len() >= 2 {
            out.push(run);
        }
    }
    Some(out)
}
