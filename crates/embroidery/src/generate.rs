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
pub(crate) const UNDERLAY_LEN: f64 = 2.0;
/// Zigzag and fill underlay spacing, mm.
const UNDERLAY_SPACING: f64 = 2.0;
/// A satin shape's zig as made: its centre, direction, the stroke's half-width there, whether it
/// is at a corner, and its two needle points.
type Made = (Point, Vec2, f64, bool, (Point, Point));

/// A satin shape's stroke turning more than this (radians) over a few zigs is at a corner.
const CORNER_TURN: f64 = 0.44;
/// Needle points on one side of a satin closer than this (mm) are crowded: every other one is
/// shortened.
const SHORT_GAP: f64 = 0.25;
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
    /// The corner point.
    at: Point,
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
            corners.push(Corner { piece: k, at: v, miter, t_in, t_out, left_inside });
            continue;
        }
        for j in 0..=steps {
            let n = n_in.lerp(n_out, j as f64 / steps as f64);
            let n = if n.hypot() > 1e-9 { n / n.hypot() } else { n_in };
            let mut z = Zig::new(v, n, None);
            z.pin(left_inside, miter);
            zigs.push(z);
        }
        corners.push(Corner { piece: k, at: v, miter, t_in, t_out, left_inside });
    }
    // Inner points near a corner that would cross past its miter point land on it instead (a
    // curving piece's far points can lie "past" it too: only those within reach of the corner).
    for c in &corners {
        let near = c.miter.distance(c.at) + 2.0 * half + spacing;
        for z in zigs.iter_mut().filter(|z| z.centre.distance(c.at) <= near) {
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

/// A satin's needle points from its zigs (each a pair across the column), alternating sides,
/// with short stitches: on a tight curve a side's needle points crowd together, and every other
/// one stops partway, so the holes don't pile up in one spot. Also how many points were crowded.
pub(crate) fn short_stitches(zig: &[(Point, Point)]) -> (Vec<Point>, usize) {
    let mut last = [None::<Point>; 2];
    let mut crowded = 0;
    let mut out = Vec::with_capacity(zig.len());
    for (k, (l, rt)) in zig.iter().enumerate() {
        let side = k % 2;
        let (end, other) = if side == 0 { (*l, *rt) } else { (*rt, *l) };
        let crowd = last.get(side).copied().flatten().is_some_and(|q| q.distance(end) < SHORT_GAP);
        crowded += usize::from(crowd);
        out.push(if crowd { end.lerp(other, 0.4) } else { end });
        if !crowd && let Some(slot) = last.get_mut(side) {
            *slot = Some(end);
        }
    }
    (out, crowded)
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

/// What an `auto` underlay becomes, by stitch and width (mm). The fabric profile (E7.3) will set
/// these; [`AutoUnderlay::default`] suits a stable woven fabric.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoUnderlay {
    /// Satin columns narrower than this get a centre walk.
    pub satin_center_under: f64,
    /// Satin columns wider than this get an edge walk and then a zigzag; those between, an edge
    /// walk.
    pub satin_zigzag_over: f64,
    /// Satin shapes whose strokes are wider than this get an edge walk; thinner, a centre walk.
    pub shape_edge_over: f64,
}

impl Default for AutoUnderlay {
    fn default() -> Self {
        Self { satin_center_under: 2.0, satin_zigzag_over: 5.0, shape_edge_over: 2.5 }
    }
}

impl AutoUnderlay {
    /// The underlays of a satin column `width` mm wide, in sewing order.
    pub fn satin(&self, width: f64) -> &'static [Underlay] {
        if width < self.satin_center_under {
            &[Underlay::Center]
        } else if width <= self.satin_zigzag_over {
            &[Underlay::Edge]
        } else {
            &[Underlay::Edge, Underlay::Zigzag]
        }
    }

    /// The underlay of a satin shape whose strokes are `stroke` mm wide.
    pub fn shape(&self, stroke: f64) -> Underlay {
        if stroke > self.shape_edge_over { Underlay::Edge } else { Underlay::Center }
    }
}

/// The underlay of a satin column `width` mm wide along `line`, and whether it ends where the
/// line ends (so the top stitches go back the other way). `auto` sews what [`AutoUnderlay`]
/// picks for the width, one underlay after the other.
pub fn satin_underlay(line: &[Point], closed: bool, width: f64, kind: Underlay) -> (Vec<Point>, bool) {
    satin_underlay_with(line, closed, width, kind, &AutoUnderlay::default())
}

/// [`satin_underlay`], `auto` picking by `auto`'s thresholds.
pub fn satin_underlay_with(line: &[Point], closed: bool, width: f64, kind: Underlay, auto: &AutoUnderlay) -> (Vec<Point>, bool) {
    if kind == Underlay::Auto {
        let mut out = vec![];
        let mut back = false;
        for u in auto.satin(width) {
            let (part, end) = satin_underlay(line, closed, width, *u);
            out.extend(part);
            back = end;
        }
        return (out, back);
    }
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
        Underlay::None | Underlay::Fill | Underlay::Auto => (vec![], false),
    }
}

/// `line` with `d` mm taken off each end (push compensation); at least 0.5 mm of it is kept,
/// from its middle.
pub fn trim_ends(line: &[Point], d: f64) -> Vec<Point> {
    let total: f64 = line.windows(2).map(|w| w[0].distance(w[1])).sum();
    if !(d.is_finite() && d > 0.0 && total.is_finite()) || line.len() < 2 {
        return line.to_vec();
    }
    let d = d.min(((total - 0.5) / 2.0).max(0.0));
    let (from, to) = (d, total - d);
    let mut out = vec![];
    let mut at = 0.0;
    for w in line.windows(2) {
        let (a, b) = (w[0], w[1]);
        let l = a.distance(b);
        let (s, e) = (at, at + l);
        if e >= from && s <= to && l > 0.0 {
            let p = |t: f64| a.lerp(b, ((t - s) / l).clamp(0.0, 1.0));
            if out.is_empty() {
                out.push(p(from.max(s)));
            }
            out.push(p(to.min(e)));
        }
        at = e;
    }
    if out.len() < 2 { line.to_vec() } else { out }
}

/// The perforation's stitch length, mm: short, so the needle holes cut the foam.
pub const PERFORATION_LEN: f64 = 1.0;

/// `line` lengthened by `d` mm at each end, straight on from its end stretches.
pub fn extend_ends(line: &[Point], d: f64) -> Vec<Point> {
    let mut out = line.to_vec();
    if !(d.is_finite() && d > 0.0) || line.len() < 2 {
        return out;
    }
    let ahead = |a: Point, b: Point| {
        let v = b - a;
        let len = v.hypot();
        (len > 1e-9).then(|| b + v * (d / len))
    };
    if let (Some(a), Some(b)) = (line.get(1), line.first())
        && let Some(p) = ahead(*a, *b)
    {
        out.insert(0, p);
    }
    let n = line.len();
    if let (Some(a), Some(b)) = (line.get(n - 2), line.last())
        && let Some(p) = ahead(*a, *b)
    {
        out.push(p);
    }
    out
}

/// A satin column's perforation: short running stitches all round the column `width` mm wide
/// along `line` (down one edge, round the end, back along the other), so the foam under it is cut.
pub fn perforation(line: &[Point], closed: bool, width: f64) -> Vec<Vec<Point>> {
    let c = resample(line, PERFORATION_LEN, true);
    let n = normals(&c, closed);
    let half = width / 2.0;
    let left: Vec<Point> = c.iter().zip(&n).map(|(p, v)| *p + *v * half).collect();
    let right: Vec<Point> = c.iter().zip(&n).map(|(p, v)| *p - *v * half).collect();
    if closed {
        // Each edge a ring of its own.
        return [left, right].into_iter().map(|r| running(&r, PERFORATION_LEN)).filter(|r| r.len() >= 2).collect();
    }
    let mut ring: Vec<Point> = left;
    ring.extend(right.into_iter().rev());
    if let Some(first) = ring.first().copied() {
        ring.push(first);
    }
    vec![running(&ring, PERFORATION_LEN)]
}

/// An area's perforation: short running stitches round each of `polys`' outlines.
pub fn outline_perforation(polys: &[Vec<Point>]) -> Vec<Vec<Point>> {
    polys
        .iter()
        .filter(|p| p.len() >= 3)
        .map(|p| {
            let mut ring = p.clone();
            if let (Some(a), Some(b)) = (ring.first().copied(), ring.last().copied())
                && a.distance(b) > 1e-9
            {
                ring.push(a);
            }
            running(&ring, PERFORATION_LEN)
        })
        .filter(|r| r.len() >= 2)
        .collect()
}

/// An area's edge walk: a running stitch round each of `polys`' outlines, [`UNDERLAY_INSET`]
/// inside the shape (filled by `rule`), one run per outline. Where the shape is too thin to step
/// inside, the walk keeps to the outline.
pub fn edge_walk(polys: &[Vec<Point>], rule: FillRule) -> Vec<Vec<Point>> {
    let mut out = vec![];
    for p in polys {
        let (Some(first), Some(last)) = (p.first(), p.last()) else { continue };
        if p.len() < 3 {
            continue;
        }
        let mut ring = p.clone();
        if first.distance(*last) > 1e-9 {
            ring.push(*first);
        }
        let c = resample(&ring, UNDERLAY_LEN, true);
        let n = normals(&c, true);
        let walk: Vec<Point> = c
            .iter()
            .zip(&n)
            .map(|(q, nv)| {
                let (a, b) = (*q + *nv * UNDERLAY_INSET, *q - *nv * UNDERLAY_INSET);
                if inside(polys, rule, a) {
                    a
                } else if inside(polys, rule, b) {
                    b
                } else {
                    *q
                }
            })
            .collect();
        if walk.len() >= 2 {
            out.push(walk);
        }
    }
    out
}

/// A fill's underlay: sparse rows across the top rows' `angle`.
pub fn fill_underlay(polys: &[Vec<Point>], rule: FillRule, angle: f64) -> Result<Vec<Vec<Point>>, String> {
    fill(polys, rule, angle + 90.0, UNDERLAY_SPACING, UNDERLAY_FILL_LEN, 2)
}

/// [`fill_underlay`] with the areas `cuts` cut out as from the top rows (see [`fill_compensated`]).
pub fn fill_underlay_cut(polys: &[Vec<Point>], rule: FillRule, angle: f64, cuts: &[Cut], overlap: f64) -> Result<Vec<Vec<Point>>, String> {
    fill_compensated(polys, rule, angle + 90.0, UNDERLAY_SPACING, UNDERLAY_FILL_LEN, 2, &[], RowComp::default(), cuts, overlap)
}

/// `runs` with the points under any of `cuts` left out, each run split where it goes under one.
pub fn outside_cuts(runs: Vec<Vec<Point>>, cuts: &[Cut]) -> Vec<Vec<Point>> {
    if cuts.is_empty() {
        return runs;
    }
    let mut out = vec![];
    for run in runs {
        let mut part: Vec<Point> = vec![];
        for p in run {
            if cuts.iter().any(|c| inside(&c.polys, c.rule, p)) {
                if part.len() >= 2 {
                    out.push(std::mem::take(&mut part));
                }
                part.clear();
            } else {
                part.push(p);
            }
        }
        if part.len() >= 2 {
            out.push(part);
        }
    }
    out
}

/// Tatami fill: rows `spacing` mm apart at `angle` degrees (counter-clockwise on the page) across
/// the area of the closed `polys` (filled by `rule`), each row running stitches up to `len` mm
/// long, staggered row to row so the needle holes line up again only every `stagger` rows (2: a
/// brick pattern, 3: tatami…). Rows that touch are sewn as one
/// run, back and forth; separate parts of the shape become separate runs.
pub fn fill(polys: &[Vec<Point>], rule: FillRule, angle: f64, spacing: f64, len: f64, stagger: u8) -> Result<Vec<Vec<Point>>, String> {
    fill_carved(polys, rule, angle, spacing, len, stagger, &[])
}

/// [`fill`] carved with the motif segments `carve` (mm, page space; see [`crate::motifs`]): where
/// a motif line crosses a row the needle goes down on it, and the row's own needle points too
/// near it are left out, so the motif shows as a line of needle holes (an embossed pattern).
pub fn fill_carved(
    polys: &[Vec<Point>],
    rule: FillRule,
    angle: f64,
    spacing: f64,
    len: f64,
    stagger: u8,
    carve: &[(Point, Point)],
) -> Result<Vec<Vec<Point>>, String> {
    fill_compensated(polys, rule, angle, spacing, len, stagger, carve, RowComp::default(), &[], 0.0)
}

/// An area sewn on top of a fill (overlap removal): its outlines, mm, and how they fill. Its
/// rows' stretches under it are cut away.
#[derive(Clone, Debug, PartialEq)]
pub struct Cut {
    pub polys: Vec<Vec<Point>>,
    pub rule: FillRule,
}

/// Where both `a` and `b` are (sorted, separate intervals).
fn intersect(a: &[(f64, f64)], b: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out = vec![];
    for &(p, q) in a {
        for &(r, s) in b {
            let (lo, hi) = (p.max(r), q.min(s));
            if hi > lo {
                out.push((lo, hi));
            }
        }
    }
    out
}

/// `spans` less `holes`, each hole `overlap` mm short of the cut's edge at either end, except an
/// end where the cut reaches past the span's own end (no gap can show there, so no rim is kept);
/// pieces shorter than [`MIN_SPAN`] are left out.
fn subtract(spans: Vec<(f64, f64)>, holes: &[(f64, f64)], overlap: f64) -> Vec<(f64, f64)> {
    let mut out = spans;
    for &(c0, c1) in holes {
        out = out
            .into_iter()
            .flat_map(|(a, b)| {
                let h0 = if c0 <= a + overlap { a } else { c0 + overlap };
                let h1 = if c1 >= b - overlap { b } else { c1 - overlap };
                if h1 <= h0 || h1 <= a || h0 >= b {
                    vec![(a, b)]
                } else {
                    [(a, h0.min(b)), (h1.max(a), b)].into_iter().filter(|(p, q)| q - p >= MIN_SPAN).collect()
                }
            })
            .collect();
    }
    out
}

/// The shortest piece of a fill row sewn after overlap removal, mm.
const MIN_SPAN: f64 = 0.5;

/// A fill's compensation (see [`fill_compensated`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RowComp {
    /// Each row sewn this much longer, mm (half at each end).
    pub pull: f64,
    /// Each row sewn this much longer again, as a percentage of its length.
    pub pull_percent: f64,
    /// The fill sewn this much shorter across its rows, mm (rows within half of it of an edge
    /// across them are left out).
    pub push: f64,
}

/// [`fill_carved`] with pull and push compensation `comp`, and the areas `cuts` (objects sewn on
/// top) cut out of its rows, the rows still reaching `overlap` mm under them.
#[allow(clippy::too_many_arguments)]
pub fn fill_compensated(
    polys: &[Vec<Point>],
    rule: FillRule,
    angle: f64,
    spacing: f64,
    len: f64,
    stagger: u8,
    carve: &[(Point, Point)],
    comp: RowComp,
    cuts: &[Cut],
    overlap: f64,
) -> Result<Vec<Vec<Point>>, String> {
    let stagger = usize::from(stagger.max(1));
    if !(spacing.is_finite() && len.is_finite()) || spacing <= 0.0 || len <= 0.0 {
        return Ok(vec![]);
    }
    // Turn the shape so the rows run along x (y down: page counter-clockwise is a negative turn).
    let rot = Affine::rotate(angle.to_radians());
    let inv = rot.inverse();
    let shape = polys;
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
    let half_push = if comp.push.is_finite() { comp.push.max(0.0) / 2.0 } else { 0.0 };
    let overlap = if overlap.is_finite() { overlap.max(0.0) } else { 0.0 };
    let cuts: Vec<(Vec<Vec<Point>>, FillRule)> =
        cuts.iter().map(|c| (c.polys.iter().map(|p| p.iter().map(|q| rot * *q).collect()).collect(), c.rule)).collect();
    while y < y1 {
        let mut row = row_spans(&polys, rule, y);
        // Push: a span whose middle is within half the push of an edge across the rows is
        // left out.
        if half_push > 0.0 {
            row.retain(|&(a, b)| {
                let m = (a + b) / 2.0;
                inside(&polys, rule, Point::new(m, y - half_push)) && inside(&polys, rule, Point::new(m, y + half_push))
            });
        }
        // Overlap removal: what lies under each cut, less the overlap all round, is cut away.
        for (cut, cut_rule) in &cuts {
            let mut hole = row_spans(cut, *cut_rule, y);
            if overlap > 0.0 {
                // Across the rows: only where the cut reaches past the overlap both ways.
                hole = intersect(&intersect(&hole, &row_spans(cut, *cut_rule, y - overlap)), &row_spans(cut, *cut_rule, y + overlap));
            }
            if !hole.is_empty() {
                row = subtract(row, &hole, overlap);
            }
        }
        spans.push(row);
        y += spacing;
    }
    // Pull: each row sewn longer at both ends (the lanes are still joined by the rows as drawn).
    let pull = |a: f64, b: f64| {
        let e = (comp.pull + (b - a).abs() * comp.pull_percent / 100.0) / 2.0;
        if e.is_finite() { e.max(0.0) } else { 0.0 }
    };
    let row_y = |r: usize| y0 + spacing / 2.0 + r as f64 * spacing;
    // Where the motif crosses each row (x, in row space), found per row from the rows each
    // segment spans.
    let mut marks: Vec<Vec<f64>> = vec![vec![]; spans.len()];
    for &(a, b) in carve {
        let (a, b) = (rot * a, rot * b);
        if !(a.is_finite() && b.is_finite()) || (a.y - b.y).abs() < 1e-12 {
            continue;
        }
        let (lo, hi) = (a.y.min(b.y), a.y.max(b.y));
        let first = ((lo - y0 - spacing / 2.0) / spacing).ceil().max(0.0) as usize;
        let last = ((hi - y0 - spacing / 2.0) / spacing).floor();
        if last < 0.0 {
            continue;
        }
        for r in first..=(last as usize).min(spans.len().saturating_sub(1)) {
            let y = row_y(r);
            if let Some(m) = marks.get_mut(r) {
                m.push(a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y));
            }
        }
    }
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
                // Where the row ends as drawn (the next row joins from there).
                let to = if forward { b } else { a };
                let e = pull(a, b);
                let (sewn_from, sewn_to) = if forward { (a - e, b + e) } else { (b + e, a - e) };
                row(&mut run, row_y(r), sewn_from, sewn_to, len, (r % stagger) as f64 / stagger as f64, marks.get(r).map_or(&[][..], Vec::as_slice));
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
    Ok(route(shape, rule, out, len))
}

/// The longest walk along a fill's outline between two of its parts, mm: further, the needle
/// jumps (and trims) instead.
const WALK_MAX: f64 = 40.0;
/// Travel stitch length, mm.
const TRAVEL_LEN: f64 = 2.0;

/// The point of ring `only` (any ring when none) of the outline `polys` nearest `p`: (ring,
/// distance along the ring from its first point, distance from `p`).
fn on_outline(polys: &[Vec<Point>], p: Point, only: Option<usize>) -> Option<(usize, f64, f64)> {
    let mut best: Option<(f64, usize, f64)> = None;
    for (r, ring) in polys.iter().enumerate().filter(|(r, _)| only.is_none_or(|o| o == *r)) {
        let n = ring.len();
        let mut along = 0.0;
        for k in 0..n {
            let (Some(&a), Some(&b)) = (ring.get(k), ring.get((k + 1) % n)) else { continue };
            let e = b - a;
            let l2 = e.hypot2();
            let t = if l2 > 0.0 { ((p - a).dot(e) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let d = p.distance(a + e * t);
            if best.is_none_or(|(bd, _, _)| d < bd) {
                best = Some((d, r, along + t * l2.sqrt()));
            }
            along += l2.sqrt();
        }
    }
    best.map(|(d, r, s)| (r, s, d))
}

/// The outline of ring `ring` from distance `from` to `to` along it, the shorter way round.
fn walk(polys: &[Vec<Point>], ring: usize, from: f64, to: f64) -> Option<Vec<Point>> {
    let poly = polys.get(ring)?;
    let n = poly.len();
    let lens: Vec<f64> = (0..n).map(|k| poly.get(k).zip(poly.get((k + 1) % n)).map_or(0.0, |(a, b)| a.distance(*b))).collect();
    let total: f64 = lens.iter().sum();
    if total <= 0.0 {
        return None;
    }
    let forward = (to - from).rem_euclid(total);
    let (dist, dir) = if forward <= total - forward { (forward, 1.0) } else { (total - forward, -1.0) };
    // Points at distance `s` along the ring.
    let at = |s: f64| -> Option<Point> {
        let mut s = s.rem_euclid(total);
        for k in 0..n {
            let l = lens.get(k).copied().unwrap_or(0.0);
            if s <= l || k == n - 1 {
                let (a, b) = (*poly.get(k)?, *poly.get((k + 1) % n)?);
                return Some(if l > 0.0 { a.lerp(b, (s / l).min(1.0)) } else { a });
            }
            s -= l;
        }
        None
    };
    // Step along, keeping the ring's corners.
    let mut out = vec![at(from)?];
    let mut s = 0.0;
    let mut cum = vec![0.0];
    for l in &lens {
        cum.push(cum.last().copied().unwrap_or(0.0) + l);
    }
    let mut corners: Vec<f64> = cum
        .iter()
        .map(|c| if dir > 0.0 { (c - from).rem_euclid(total) } else { (from - c).rem_euclid(total) })
        .filter(|d| *d > 1e-9 && *d < dist)
        .collect();
    corners.sort_by(f64::total_cmp);
    for d in corners {
        if d > s {
            out.push(at(from + dir * d)?);
            s = d;
        }
    }
    out.push(at(from + dir * dist)?);
    Some(out)
}

/// Join a fill's separate parts (`lanes`, each sewn without a jump) into as few runs as can be:
/// nearest part next (sewn backward when its far end is nearer), reached by a straight run of
/// stitches when that stays inside the shape (the rows sewn next cover it), else by a walk along
/// the outline when both ends lie on the same outline and the walk is short. Otherwise the part
/// starts a run of its own (the needle jumps there).
fn route(polys: &[Vec<Point>], rule: FillRule, lanes: Vec<Vec<Point>>, len: f64) -> Vec<Vec<Point>> {
    let mut lanes: Vec<Vec<Point>> = lanes.into_iter().filter(|l| l.len() > 1).collect();
    let mut out: Vec<Vec<Point>> = vec![];
    let travel_len = TRAVEL_LEN.min(len);
    while !lanes.is_empty() {
        let at = out.last().and_then(|r| r.last()).copied();
        // The nearest part, by either end.
        let pick = lanes
            .iter()
            .enumerate()
            .flat_map(|(i, l)| {
                let d = |p: Option<&Point>| match (at, p) {
                    (Some(a), Some(p)) => a.distance(*p),
                    _ => 0.0,
                };
                [(i, false, d(l.first())), (i, true, d(l.last()))]
            })
            .min_by(|a, b| a.2.total_cmp(&b.2));
        let Some((i, backward, _)) = pick else { break };
        let mut lane = lanes.remove(i);
        if backward {
            lane.reverse();
        }
        let (Some(from), Some(&to)) = (at, lane.first()) else {
            out.push(lane);
            continue;
        };
        // Checked every 0.3 mm: a narrow notch mustn't slip between the checks.
        let checks = ((from.distance(to) / 0.3).ceil() as usize).clamp(2, 4000);
        let inside_line = (1..checks).all(|k| inside(polys, rule, from.lerp(to, k as f64 / checks as f64)));
        let path = if from.distance(to) < 1e-9 || inside_line {
            Some(vec![from, to])
        } else {
            // Along one of the outlines: straight to it, along it, straight off it, each straight
            // leg inside the shape; the shortest such way.
            let straight = |a: Point, b: Point| {
                let n = ((a.distance(b) / 0.3).ceil() as usize).clamp(1, 4000);
                (1..n).all(|k| inside(polys, rule, a.lerp(b, k as f64 / n as f64)))
            };
            let length = |w: &[Point]| w.windows(2).map(|p| p[0].distance(p[1])).sum::<f64>();
            (0..polys.len())
                .filter_map(|ring| {
                    let (_, sa, _) = on_outline(polys, from, Some(ring))?;
                    let (_, sb, _) = on_outline(polys, to, Some(ring))?;
                    let mut w = walk(polys, ring, sa, sb)?;
                    let (&on, &off) = (w.first()?, w.last()?);
                    if !straight(from, on) || !straight(off, to) {
                        return None;
                    }
                    w.insert(0, from);
                    w.push(to);
                    Some(w)
                })
                .filter(|w| length(w) <= WALK_MAX)
                .min_by(|a, b| length(a).total_cmp(&length(b)))
        };
        match (path, out.last_mut()) {
            (Some(p), Some(run)) => {
                // The travel as running stitches along it (its corners kept, no stitch longer than
                // the travel length), then the part.
                for q in p.into_iter().skip(1) {
                    sew(run, q, travel_len);
                }
                run.extend(lane.into_iter().skip(1));
            }
            _ => out.push(lane),
        }
    }
    out
}

/// Is `p` inside the area `polys` fill (or within 0.05 mm of its edge)?
pub(crate) fn inside(polys: &[Vec<Point>], rule: FillRule, p: Point) -> bool {
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
    // Already there (a walk reaching a corner it stands on): no stitch.
    if run.last().is_none_or(|l| l.distance(p) > 1e-9) {
        run.push(p);
    }
}

/// One fill row from `from` to `to` at `y`, joined to `run` (whose end is where the needle is),
/// in stitches no longer than `len`: the row's inner needle points sit on a grid shifted by
/// `shift` of a stitch, and on the motif `marks` (x where a carving line crosses the row), the
/// grid points near a mark left out.
fn row(run: &mut Vec<Point>, y: f64, from: f64, to: f64, len: f64, shift: f64, marks: &[f64]) {
    sew(run, Point::new(from, y), len);
    let offset = shift * len;
    let margin = (len * 0.25).max(0.3);
    let gap = (len * 0.3).clamp(0.3, 0.8);
    let (lo, hi) = (from.min(to), from.max(to));
    let marks: Vec<f64> = marks.iter().copied().filter(|m| *m > lo + margin && *m < hi - margin).collect();
    let first = ((lo + margin - offset) / len).ceil();
    let last = ((hi - margin - offset) / len).floor();
    let mut points: Vec<f64> = if last >= first {
        (first as i64..=last as i64).map(|k| k as f64 * len + offset).filter(|x| marks.iter().all(|m| (x - m).abs() >= gap)).collect()
    } else {
        vec![]
    };
    points.extend(marks);
    points.sort_by(f64::total_cmp);
    if to < from {
        points.reverse();
    }
    for x in points {
        sew(run, Point::new(x, y), len);
    }
    sew(run, Point::new(to, y), len);
}

#[cfg(test)]
pub(crate) fn tests_inside(polys: &[Vec<Point>], p: Point) -> bool {
    // On the outline counts as inside (corners included, however sharp).
    let on_edge = polys.iter().any(|poly| {
        let n = poly.len();
        (0..n).any(|k| {
            let (a, b) = (poly[k], poly[(k + 1) % n]);
            let e = b - a;
            let t = if e.hypot2() > 0.0 { ((p - a).dot(e) / e.hypot2()).clamp(0.0, 1.0) } else { 0.0 };
            p.distance(a + e * t) < 0.05
        })
    });
    if on_edge {
        return true;
    }
    // Else probe a hair either side.
    [(0.0, 0.0), (0.0, 0.05), (0.0, -0.05), (0.05, 0.0), (-0.05, 0.0)]
        .iter()
        .any(|&(dx, dy)| inside(polys, FillRule::NonZero, Point::new(p.x + dx, p.y + dy)))
}

/// Points every `step` mm along `line` (from its first point, ending on its last) with `values`
/// interpolated at them.
pub(crate) fn resample_with(line: &[Point], values: &[f64], step: f64) -> (Vec<Point>, Vec<f64>) {
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
    satin_shape_checked(polys, rule, spacing, pull, underlay).0
}

/// How well a satin shape suits its shape (0–1 each; lower is better).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SatinQuality {
    /// Of the zig positions along the strokes, those with no edge near on one side (inside a
    /// junction or a blob, where satin can't span the shape): left out.
    pub skipped: f64,
    /// Of the needle points, those crowding the one before on their side (a tight inner curve).
    pub crowded: f64,
    /// Of the zigs, those fanning: the needle points on one side three times further apart than
    /// on the other (a stroke wide for the curve it turns, round a small counter).
    pub fanned: f64,
    /// Of the shape's area, what no stitch covers ([`uncovered`]): bare fabric where the satin
    /// doesn't reach (a heavy letter's corners and junctions).
    pub uncovered: f64,
}

impl SatinQuality {
    /// Satin that leaves out or crowds this much looks wrong: fill the shape instead.
    pub fn poor(&self) -> bool {
        self.skipped > 0.2
            || self.crowded > 0.2
            || self.fanned > 0.2
            || self.skipped + self.crowded + self.fanned > 0.28
            || self.uncovered > UNCOVERED_LIMIT
    }
}

/// [`satin_shape`] and how well it suits the shape.
pub fn satin_shape_checked(polys: &[Vec<Point>], rule: FillRule, spacing: f64, pull: f64, underlay: bool) -> (Vec<Vec<Point>>, SatinQuality) {
    satin_shape_at_least(polys, rule, spacing, pull, underlay, 0.0)
}

/// [`satin_shape_checked`], each zig at least `min_width` mm across (widened evenly about its
/// middle): thin strokes, as small lettering has, sewn as narrow satin columns rather than lost.
pub fn satin_shape_at_least(
    polys: &[Vec<Point>],
    rule: FillRule,
    spacing: f64,
    pull: f64,
    underlay: bool,
    min_width: f64,
) -> (Vec<Vec<Point>>, SatinQuality) {
    satin_shape_hand(polys, rule, spacing, pull, underlay, min_width, crate::penstrokes::Hand::Print)
}

/// [`satin_shape_at_least`] for a shape written in `hand`: sewn by pen strokes
/// ([`satin_pen`]) when they suit it, else piece by piece of its centre line.
pub fn satin_shape_hand(
    polys: &[Vec<Point>],
    rule: FillRule,
    spacing: f64,
    pull: f64,
    underlay: bool,
    min_width: f64,
    hand: crate::penstrokes::Hand,
) -> (Vec<Vec<Point>>, SatinQuality) {
    let strokes = crate::skeleton::strokes(polys, rule);
    if let Some((runs, q)) = satin_pen(polys, rule, &strokes, spacing, pull, underlay, min_width, hand)
        && pen_suits(&q)
    {
        return (runs, q);
    }
    satin_pieces(polys, rule, strokes, spacing, pull, underlay, min_width)
}

/// A stretch of a pen stroke's satin: its centre points and its zigs (each a pair across it).
pub type SatinPart = (Vec<Point>, Vec<(Point, Point)>);

/// Pen strokes that leave more than this bare (before patching) don't suit the shape: it is sewn
/// piece by piece, or filled.
const PEN_BARE: f64 = 0.05;

/// A shape's pen strokes ([`crate::penstrokes`]) as the stretches of satin they sew: each stroke at
/// its own width from end to end, through junctions and on to the edge at corners, in the order and
/// direction a pen draws it, zigs `spacing` apart; where it crosses a stroke drawn before, only the
/// tuck under that stroke's edge is kept. With how many zigs there are and how many fan (one side
/// three times the other). None when the shape has no strokes.
pub fn pen_parts(
    polys: &[Vec<Point>],
    rule: FillRule,
    pieces: &[crate::skeleton::Stroke],
    spacing: f64,
    pull: f64,
    min_width: f64,
    hand: crate::penstrokes::Hand,
) -> Option<(Vec<SatinPart>, usize, usize)> {
    use crate::penstrokes::{pen_strokes, under};
    let mut pens = pen_strokes(pieces, hand);
    if pens.is_empty() {
        return None;
    }
    for pen in &mut pens {
        crate::penstrokes::carry_through(pen, spacing.max(0.1), |q| inside(polys, rule, q));
    }
    let (mut zigs, mut fanned) = (0usize, 0usize);
    let mut all: Vec<SatinPart> = vec![];
    for (i, pen) in pens.iter().enumerate() {
        let drawn = pens.get(..i).unwrap_or_default();
        let (c, h) = resample_with(&pen.points, &pen.half, spacing);
        let base = normals(&c, pen.closed);
        // The stroke's own half-width: where its centre line narrows toward a tip, its zigs still
        // reach out to the outline up to it.
        let own = {
            let mut s = h.clone();
            s.sort_by(f64::total_cmp);
            s.get(s.len() / 2).copied().unwrap_or(0.0)
        };
        // The stroke's zigs, split where it runs under a stroke drawn before.
        let mut parts: Vec<SatinPart> = vec![(vec![], vec![])];
        for ((p, n), half) in c.iter().zip(&base).zip(&h) {
            if under(*p, drawn) {
                if parts.last().is_some_and(|(_, z)| !z.is_empty()) {
                    parts.push((vec![], vec![]));
                }
                continue;
            }
            let reach = half.max(own);
            let half = (half + pull / 2.0).max(min_width / 2.0).min(MAX_STITCH / 2.0);
            // Out to the outline, as far as the stroke's own width (more across a tip than its
            // centre line's distance to the edge says), else its width here.
            let side = |d: Vec2| {
                crate::skeleton::cast(polys, *p, d, reach * 1.25).map_or(half, |e| (e + pull / 2.0).min((reach + pull / 2.0).min(MAX_STITCH / 2.0)))
            };
            let (l, r) = (side(-*n), side(*n));
            if l + r < 0.05 {
                continue;
            }
            zigs += 1;
            if l.max(r) > 3.0 * l.min(r).max(0.05) {
                fanned += 1;
            }
            if let Some((centres, zig)) = parts.last_mut() {
                centres.push(*p);
                zig.push((*p - *n * l, *p + *n * r));
            }
        }
        all.extend(parts.into_iter().filter(|(_, z)| z.len() >= 2));
    }
    Some((all, zigs, fanned))
}

/// Satin by pen strokes ([`pen_parts`]), with a centre walk under each stretch when `underlay`.
/// What is still bare is filled under it. None when the shape has no strokes.
#[allow(clippy::too_many_arguments)]
pub fn satin_pen(
    polys: &[Vec<Point>],
    rule: FillRule,
    pieces: &[crate::skeleton::Stroke],
    spacing: f64,
    pull: f64,
    underlay: bool,
    min_width: f64,
    hand: crate::penstrokes::Hand,
) -> Option<(Vec<Vec<Point>>, SatinQuality)> {
    let (parts, zigs, fanned) = pen_parts(polys, rule, pieces, spacing, pull, min_width, hand)?;
    let mut out = uncovered_parts(polys, rule, pieces);
    let (mut points, mut crowded) = (0usize, 0usize);
    for (centres, zig) in parts {
        let (needles, crowd) = short_stitches(&zig);
        points += needles.len();
        crowded += crowd;
        let mut run = vec![];
        if underlay {
            // A walk down the stroke's centre, then the satin back over it.
            run.extend(running(&centres, UNDERLAY_LEN));
            run.extend(needles.iter().rev());
        } else {
            run.extend(needles);
        }
        if run.len() > 1 {
            out.push(split_long(&run));
        }
    }
    if out.is_empty() {
        return None;
    }
    let bare = uncovered(polys, rule, &out);
    if bare > 0.0 {
        let patches = patch_bare(polys, rule, &out, StitchSettings::DEFAULT_SPACING);
        out.splice(0..0, patches);
    }
    let frac = |n: usize, of: usize| if of == 0 { 0.0 } else { n as f64 / of as f64 };
    Some((out, SatinQuality { skipped: 0.0, crowded: frac(crowded, points), fanned: frac(fanned, zigs), uncovered: bare }))
}

/// Whether pen strokes suit a shape whose pen satin came out as `q`.
pub fn pen_suits(q: &SatinQuality) -> bool {
    !q.poor() && q.uncovered <= PEN_BARE
}

/// Satin piece by piece of the shape's centre line (`strokes`): the way shapes that aren't
/// written as pen strokes are sewn.
fn satin_pieces(
    polys: &[Vec<Point>],
    rule: FillRule,
    mut strokes: Vec<crate::skeleton::Stroke>,
    spacing: f64,
    pull: f64,
    underlay: bool,
    min_width: f64,
) -> (Vec<Vec<Point>>, SatinQuality) {
    let (mut positions, mut skipped, mut points, mut crowded) = (0usize, 0usize, 0usize, 0usize);
    let (mut zigs, mut fanned) = (0usize, 0usize);
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
        if flip {
            s.trim = (s.trim.1, s.trim.0);
        }
        let closed = s.points.len() > 3 && s.points.first() == s.points.last();
        let mut run = vec![];
        let (line, radius, trim) = if underlay {
            run.extend(running(&s.points, UNDERLAY_LEN));
            (s.points.iter().rev().copied().collect::<Vec<_>>(), s.radius.iter().rev().copied().collect::<Vec<_>>(), (s.trim.1, s.trim.0))
        } else {
            (s.points.clone(), s.radius.clone(), s.trim)
        };
        let (c, r) = resample_with(&line, &radius, spacing);
        let base = normals(&c, closed);
        // Distance along the line, to leave out the trimmed ends.
        let mut along = vec![0.0; c.len()];
        for k in 1..c.len() {
            if let (Some(&p), Some(&q), Some(&prev)) = (c.get(k - 1), c.get(k), along.get(k - 1))
                && let Some(a) = along.get_mut(k)
            {
                *a = prev + p.distance(q);
            }
        }
        let total = along.last().copied().unwrap_or(0.0);
        // Where the stroke turns sharply (a w's points), the centre line cuts the corner: the zigs
        // there stay square to it and their outer side reaches further, out to the corner.
        let corner: Vec<bool> = (0..base.len())
            .map(|k| {
                // Over about one and a half half-widths either side: the centre line rounds a
                // corner over about the stroke's width.
                let w = ((1.5 * r.get(k).copied().unwrap_or(0.0) / spacing).round() as usize).clamp(3, 40);
                let a = base.get(k.saturating_sub(w)).copied().unwrap_or(Vec2::ZERO);
                let b = base.get((k + w).min(base.len().saturating_sub(1))).copied().unwrap_or(Vec2::ZERO);
                a.dot(b).clamp(-1.0, 1.0).acos() > CORNER_TURN
            })
            .collect();
        // Each zig crosses the stroke where it is narrowest: the normal turned up to 35° either
        // way, the angle smoothed along the stroke so neighbouring stitches lie alike.
        let turns: Vec<f64> = c
            .iter()
            .zip(&base)
            .zip(&r)
            .zip(&corner)
            .map(|(((p, n), r), &corner)| {
                if corner {
                    return 0.0;
                }
                let cap = (2.0 * r).clamp(0.3, MAX_STITCH / 2.0);
                (-5..=5)
                    .map(|k| k as f64 * 7.0_f64.to_radians())
                    .filter_map(|t| {
                        let d = Vec2::new(n.x * t.cos() - n.y * t.sin(), n.x * t.sin() + n.y * t.cos());
                        let a = crate::skeleton::cast(polys, *p, d, cap)?;
                        let b = crate::skeleton::cast(polys, *p, -d, cap)?;
                        // Much narrower than the stroke is wide: that crossing cuts the stroke's
                        // end (or a corner), not across it.
                        (a + b >= 1.2 * r || t == 0.0).then_some((a + b + t.abs() * 0.05, t))
                    })
                    .min_by(|x, y| x.0.total_cmp(&y.0))
                    .map_or(0.0, |(_, t)| t)
            })
            .collect();
        let turns: Vec<f64> = (0..turns.len())
            .map(|k| {
                let lo = k.saturating_sub(2);
                let hi = (k + 2).min(turns.len().saturating_sub(1));
                let w = turns.get(lo..=hi).unwrap_or_default();
                if w.is_empty() { 0.0 } else { w.iter().sum::<f64>() / w.len() as f64 }
            })
            .collect();
        // One zig at `p` across direction `d`: its two needle points, or none where satin can't
        // span (no edge near on a side: inside a junction, which other strokes cover, or a blob).
        // Out to the edge each side, but not past twice the stroke's half-width `r` there (at a
        // junction the edge across may be far down another stroke); at a sharp turn the outer
        // side may reach twice as far, out to the corner, and if the ray misses the inside (it
        // runs up a stroke past the notch) the nearest edge point there is taken, as a miter.
        let make = |p: Point, d: Vec2, r: f64, at_corner: bool| -> Option<(Point, Point)> {
            let cap = (2.0 * r).clamp(0.3, MAX_STITCH / 2.0);
            let reach = if at_corner { (2.0 * cap).min(MAX_STITCH / 2.0) } else { cap };
            let side = |dir: Vec2| -> Option<(Point, f64)> {
                match crate::skeleton::cast(polys, p, dir, reach) {
                    Some(t) => Some((p + dir * t, t)),
                    None if at_corner => crate::skeleton::nearest(polys, p, dir, cap).map(|q| (q, q.distance(p))),
                    None => None,
                }
            };
            let ((mut lp, ld), (mut rp, rd)) = (side(d)?, side(-d)?);
            if ld.min(rd) > cap {
                return None;
            }
            // Pull compensation widens the stitch at both ends; the diagonal to the next zig
            // must be sewable too.
            let across = lp - rp;
            let len = across.hypot();
            if len > 1e-9 {
                let u = across / len;
                // At least `min_width` across, compensation included.
                let grow = pull.max(min_width - len);
                lp += u * (grow / 2.0);
                rp -= u * (grow / 2.0);
                let pull = grow;
                let most = (MAX_STITCH - spacing).max(0.3);
                if len + pull > most {
                    let mid = lp.midpoint(rp);
                    let k = most / (len + pull);
                    (lp, rp) = (mid + (lp - mid) * k, mid + (rp - mid) * k);
                }
            }
            Some((lp, rp))
        };
        let mut made: Vec<Made> = vec![];
        for (k, (((p, n), r), t)) in c.iter().zip(&base).zip(&r).zip(&turns).enumerate() {
            let s_at = along.get(k).copied().unwrap_or(0.0);
            if s_at < trim.0 || s_at > total - trim.1 {
                continue;
            }
            let d = Vec2::new(n.x * t.cos() - n.y * t.sin(), n.x * t.sin() + n.y * t.cos());
            let at_corner = corner.get(k).copied().unwrap_or(false);
            positions += 1;
            match make(*p, d, *r, at_corner) {
                Some(z) => made.push((*p, d, *r, at_corner, z)),
                None => skipped += 1,
            }
        }
        // Round a curve the outer side's needle points spread apart: more zigs go in between,
        // their direction swept from one to the next, until neither side gaps (a fan).
        let mut zig: Vec<(Point, Point)> = vec![];
        for (i, &(p, d, r, at_corner, z)) in made.iter().enumerate() {
            if let Some(&(q, e, rq, cq, zq)) = made.get(i + 1) {
                zig.push(z);
                let gap = z.0.distance(zq.0).max(z.1.distance(zq.1));
                if gap > 1.5 * spacing && p.distance(q) < 2.0 * spacing {
                    let extra = ((gap / spacing).ceil() as usize).saturating_sub(1).min(64);
                    for m in 1..=extra {
                        let f = m as f64 / (extra + 1) as f64;
                        let dir = d.lerp(e, f);
                        let len = dir.hypot();
                        if len < 1e-9 {
                            continue;
                        }
                        if let Some(z) = make(p.lerp(q, f), dir / len, r + (rq - r) * f, at_corner || cq) {
                            zig.push(z);
                        }
                    }
                }
            } else {
                zig.push(z);
            }
        }
        for w in zig.windows(2) {
            let (dl, dr) = (w[0].0.distance(w[1].0), w[0].1.distance(w[1].1));
            let (lo, hi) = (dl.min(dr), dl.max(dr));
            zigs += 1;
            fanned += usize::from(hi > 2.0 * spacing && hi > 3.0 * lo);
        }
        let (pts, crowd) = short_stitches(&zig);
        points += pts.len();
        crowded += crowd;
        run.extend(pts);
        if let Some(&end) = run.last() {
            at = Some(end);
        }
        if run.len() > 1 {
            out.push(split_long(&run));
        }
    }
    let frac = |n: usize, of: usize| if of == 0 { 0.0 } else { n as f64 / of as f64 };
    let bare = uncovered(polys, rule, &out);
    // What the satin leaves bare (corners and junctions of heavy strokes) is filled, under it.
    if bare > 0.0 && !out.is_empty() {
        let patches = patch_bare(polys, rule, &out, StitchSettings::DEFAULT_SPACING);
        out.splice(0..0, patches);
    }
    (out, SatinQuality { skipped: frac(skipped, positions), crowded: frac(crowded, points), fanned: frac(fanned, zigs), uncovered: bare })
}

/// The share of a shape's area satin may leave bare (before the bare patches are filled) before
/// the shape isn't one for satin: heavy strokes whose corners and junctions it can't reach.
pub const UNCOVERED_LIMIT: f64 = 0.15;
/// The grid cell [`uncovered`] measures with, mm.
const COVER_CELL: f64 = 0.3;
/// How far from a stitch the fabric still counts as covered, mm (the thread's spread and the
/// spacing between satin zigs).
const COVER_REACH: f64 = 0.35;
/// The most grid cells [`uncovered`] looks at: past it, a coarser grid.
const COVER_MAX_CELLS: f64 = 400_000.0;

/// A grid over a shape: which cells are inside it, and which of those no stitch comes near.
struct Coverage {
    x0: f64,
    y0: f64,
    cell: f64,
    cols: usize,
    rows: usize,
    /// Per cell: inside the shape, and bare (inside, no stitch within [`COVER_REACH`]).
    inside: Vec<bool>,
    bare: Vec<bool>,
}

impl Coverage {
    /// `polys`'s cells, and those `runs` leave bare; none for a shape with no area.
    fn of(polys: &[Vec<Point>], rule: FillRule, runs: &[Vec<Point>]) -> Option<Coverage> {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in polys.iter().flatten() {
            (x0, y0, x1, y1) = (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y));
        }
        if !(x1 > x0 && y1 > y0 && (x1 - x0).is_finite() && (y1 - y0).is_finite()) {
            return None;
        }
        let cell = COVER_CELL.max(((x1 - x0) * (y1 - y0) / COVER_MAX_CELLS).sqrt());
        let (cols, rows) = (((x1 - x0) / cell).ceil() as usize, ((y1 - y0) / cell).ceil() as usize);
        let centre = |gx: usize, gy: usize| Point::new(x0 + (gx as f64 + 0.5) * cell, y0 + (gy as f64 + 0.5) * cell);
        // The cells a stitch passes near.
        let reach = COVER_REACH.max(cell);
        let mut near = vec![false; cols * rows];
        let r = (reach / cell).ceil() as isize;
        for run in runs {
            for w in run.windows(2) {
                let (a, b) = (w[0], w[1]);
                let steps = ((a.distance(b) / (cell / 2.0)).ceil() as usize).clamp(1, 100_000);
                for k in 0..=steps {
                    let p = a.lerp(b, k as f64 / steps as f64);
                    let (cx, cy) = (((p.x - x0) / cell).floor() as isize, ((p.y - y0) / cell).floor() as isize);
                    for dy in -r..=r {
                        for dx in -r..=r {
                            let (gx, gy) = (cx + dx, cy + dy);
                            if gx < 0 || gy < 0 || gx as usize >= cols || gy as usize >= rows {
                                continue;
                            }
                            if centre(gx as usize, gy as usize).distance(p) <= reach
                                && let Some(c) = near.get_mut(gy as usize * cols + gx as usize)
                            {
                                *c = true;
                            }
                        }
                    }
                }
            }
        }
        let mut inside_cells = vec![false; cols * rows];
        let mut bare = vec![false; cols * rows];
        for gy in 0..rows {
            for gx in 0..cols {
                let k = gy * cols + gx;
                if inside(polys, rule, centre(gx, gy)) {
                    if let Some(c) = inside_cells.get_mut(k) {
                        *c = true;
                    }
                    if !near.get(k).copied().unwrap_or(false)
                        && let Some(c) = bare.get_mut(k)
                    {
                        *c = true;
                    }
                }
            }
        }
        let mut c = Coverage { x0, y0, cell, cols, rows, inside: inside_cells, bare };
        c.keep_thick();
        Some(c)
    }

    /// Keep only the bare patches at least [`THICK_CELLS`] cells across somewhere (a cell with
    /// nothing covered that near it all round): thin slivers, such as where a satin stops just
    /// short of a stroke's tip, are the thread's to cover.
    fn keep_thick(&mut self) {
        let (cols, rows) = (self.cols, self.rows);
        let r = THICK_CELLS as isize;
        // Bare on every side, inside the shape: a strip along the outline (where a satin stops just
        // short of a stroke's tip) has the outside on one side, and isn't a gap.
        let open = |c: &Coverage, gx: isize, gy: isize| c.at(&c.bare, gx, gy);
        let core: Vec<bool> = (0..cols * rows)
            .map(|k| {
                let (gx, gy) = ((k % cols) as isize, (k / cols) as isize);
                self.at(&self.bare, gx, gy) && (-r..=r).all(|dy| (-r..=r).all(|dx| open(self, gx + dx, gy + dy)))
            })
            .collect();
        // The patches holding a core cell, grown from the cores through bare cells.
        let mut keep = vec![false; cols * rows];
        let mut stack: Vec<usize> = (0..cols * rows).filter(|k| core.get(*k).copied().unwrap_or(false)).collect();
        while let Some(k) = stack.pop() {
            if keep.get(k).copied().unwrap_or(true) || !self.bare.get(k).copied().unwrap_or(false) {
                continue;
            }
            if let Some(c) = keep.get_mut(k) {
                *c = true;
            }
            let (gx, gy) = (k % cols, k / cols);
            for (dx, dy) in [(-1isize, 0isize), (1, 0), (0, -1), (0, 1)] {
                let (nx, ny) = (gx as isize + dx, gy as isize + dy);
                if self.at(&self.bare, nx, ny) {
                    stack.push(ny as usize * cols + nx as usize);
                }
            }
        }
        self.bare = keep;
    }

    fn fraction(&self) -> f64 {
        let inside = self.inside.iter().filter(|c| **c).count();
        let bare = self.bare.iter().filter(|c| **c).count();
        if inside == 0 { 0.0 } else { bare as f64 / inside as f64 }
    }

    /// Whether cell (`gx`, `gy`) of `v` is set (none outside the grid).
    fn at(&self, v: &[bool], gx: isize, gy: isize) -> bool {
        gx >= 0
            && gy >= 0
            && (gx as usize) < self.cols
            && (gy as usize) < self.rows
            && v.get(gy as usize * self.cols + gx as usize).copied().unwrap_or(false)
    }

    /// Fill rows over the bare cells, each bare patch grown by a cell (so the rows reach under the
    /// satin round it) and kept inside the shape: horizontal rows `spacing` apart, back and forth,
    /// one run per patch. Patches smaller than [`MIN_PATCH`] are left: the thread's spread covers
    /// them.
    fn patch(&self, spacing: f64) -> Vec<Vec<Point>> {
        let (cols, rows) = (self.cols, self.rows);
        // Bare patches, as connected groups of cells.
        let mut seen = vec![false; cols * rows];
        let mut patches: Vec<Vec<(usize, usize)>> = vec![];
        for start in 0..cols * rows {
            if !self.bare.get(start).copied().unwrap_or(false) || seen.get(start).copied().unwrap_or(true) {
                continue;
            }
            let mut cells = vec![];
            let mut stack = vec![start];
            while let Some(k) = stack.pop() {
                if seen.get(k).copied().unwrap_or(true) || !self.bare.get(k).copied().unwrap_or(false) {
                    continue;
                }
                if let Some(s) = seen.get_mut(k) {
                    *s = true;
                }
                let (gx, gy) = (k % cols, k / cols);
                cells.push((gx, gy));
                for (dx, dy) in [(-1isize, 0isize), (1, 0), (0, -1), (0, 1)] {
                    let (nx, ny) = (gx as isize + dx, gy as isize + dy);
                    if self.at(&self.bare, nx, ny) {
                        stack.push(ny as usize * cols + nx as usize);
                    }
                }
            }
            patches.push(cells);
        }
        let min_cells = (MIN_PATCH / (self.cell * self.cell)).ceil() as usize;
        let mut out = vec![];
        for cells in patches.iter().filter(|c| c.len() >= min_cells) {
            // The patch grown by a cell, inside the shape.
            let mut mask = vec![false; cols * rows];
            for &(gx, gy) in cells {
                for dy in -1..=1isize {
                    for dx in -1..=1isize {
                        let (nx, ny) = (gx as isize + dx, gy as isize + dy);
                        if self.at(&self.inside, nx, ny)
                            && let Some(m) = mask.get_mut(ny as usize * cols + nx as usize)
                        {
                            *m = true;
                        }
                    }
                }
            }
            let (ylo, yhi) = cells.iter().fold((usize::MAX, 0), |(lo, hi), &(_, gy)| (lo.min(gy), hi.max(gy)));
            let (ylo, yhi) = (ylo.saturating_sub(1), (yhi + 1).min(rows.saturating_sub(1)));
            let mut run: Vec<Point> = vec![];
            let mut y = self.y0 + ylo as f64 * self.cell + spacing / 2.0;
            let bottom = self.y0 + (yhi + 1) as f64 * self.cell;
            let mut forward = true;
            while y < bottom {
                let gy = ((y - self.y0) / self.cell).floor() as isize;
                // The row's spans of masked cells.
                let mut spans: Vec<(f64, f64)> = vec![];
                let mut gx = 0isize;
                while (gx as usize) < cols {
                    if self.at(&mask, gx, gy) {
                        let s = gx;
                        while (gx as usize) < cols && self.at(&mask, gx, gy) {
                            gx += 1;
                        }
                        spans.push((self.x0 + s as f64 * self.cell, self.x0 + gx as f64 * self.cell));
                    }
                    gx += 1;
                }
                if !forward {
                    spans.reverse();
                }
                for (a, b) in spans {
                    let (a, b) = if forward { (a, b) } else { (b, a) };
                    // Stitches up to the fill length along the row.
                    let n = ((a - b).abs() / vectorcraft_doc::StitchKind::Fill.default_length()).ceil().max(1.0) as usize;
                    for k in 0..=n {
                        run.push(Point::new(a + (b - a) * k as f64 / n as f64, y));
                    }
                }
                forward = !forward;
                y += spacing;
            }
            if run.len() > 1 {
                out.push(split_long(&run));
            }
        }
        out
    }
}

/// Bare patches smaller than this are left alone, mm².
const MIN_PATCH: f64 = 0.5;
/// How many cells a bare patch must be clear of stitches all round somewhere to count (see
/// [`Coverage::keep_thick`]).
const THICK_CELLS: usize = 1;

/// The share of the shape `polys` no stitch of `runs` comes within [`COVER_REACH`] of (0 for a
/// shape with no area): each cell of a grid over the shape, inside it, counts as covered when a
/// stitch passes that near its centre.
pub fn uncovered(polys: &[Vec<Point>], rule: FillRule, runs: &[Vec<Point>]) -> f64 {
    Coverage::of(polys, rule, runs).map_or(0.0, |c| c.fraction())
}

/// Fill rows for what `runs` leave bare of the shape `polys` (see [`Coverage::patch`]), sewn before
/// them so the satin lies over their edges.
pub fn patch_bare(polys: &[Vec<Point>], rule: FillRule, runs: &[Vec<Point>], spacing: f64) -> Vec<Vec<Point>> {
    Coverage::of(polys, rule, runs).map_or_else(Vec::new, |c| c.patch(spacing))
}

/// `run` with any stitch longer than [`MAX_STITCH`] split into equal shorter ones (a tight curve
/// can swing a zig further than its width).
pub(crate) fn split_long(run: &[Point]) -> Vec<Point> {
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
