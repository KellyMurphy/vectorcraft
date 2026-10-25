//! Pen strokes: a letter digitized the way it is written, rather than piece by piece of its centre
//! line (lettering's satin, E8 lettering quality).
//!
//! A letter's skeleton ([`crate::skeleton::strokes`]) breaks at every junction, so satin that
//! follows it stops short at each one and fans round it. Here the pieces are put back together into
//! the strokes a pen would make, by common rules of penmanship:
//!
//! - **Through a junction** the pieces that carry straight on are one stroke: a `t`'s crossbar is
//!   one stroke across its stem, not two arms.
//! - **Print** letters break at sharp corners: an `L` is a downstroke and a stroke out to the right,
//!   an `E` its spine and three arms. **Script** never breaks at a corner: the pen keeps going,
//!   following the flow of the letter.
//! - **Order** (print): downstrokes first, left to right, each pulled from the top down; then
//!   slanted strokes and curves, from the top; then crossbars and arms, top to bottom, each drawn
//!   left to right. Loops start at their top. Script goes left to right, as written.
//! - **Overlaps**: a stroke keeps its own width through a junction, and where a later stroke crosses
//!   an earlier one only a tuck under the earlier stroke's edge is sewn, so the crossing isn't sewn
//!   twice.

use kurbo::{Point, Vec2};

use crate::skeleton::Stroke;

/// How a letter is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hand {
    /// Upright or slanted print: strokes break at sharp corners.
    Print,
    /// Joined script: strokes follow the letter's flow, never breaking at a corner.
    Script,
}

/// A stroke as a pen makes it: its centre line (mm, in the order it is drawn) and the half-width
/// at each point.
#[derive(Clone, Debug, PartialEq)]
pub struct PenStroke {
    pub points: Vec<Point>,
    pub half: Vec<f64>,
    pub closed: bool,
    /// Whether its start and its end stop at a junction (another stroke runs on there), rather
    /// than at a tip of the letter: the pen carries such an end on across that stroke
    /// ([`carry_through`]).
    pub at_junction: (bool, bool),
}

/// A turn sharper than this breaks a print stroke (degrees).
const CORNER: f64 = 55.0;
/// Pieces that turn no more than this through a junction carry straight on (degrees).
const STRAIGHT_PRINT: f64 = 35.0;
const STRAIGHT_SCRIPT: f64 = 60.0;
/// Within this many degrees of upright (after the letter's slant), a stroke is a downstroke; of
/// level, a crossbar.
const UPRIGHT: f64 = 30.0;
/// How far a later stroke reaches under an earlier one it crosses, mm.
pub const TUCK: f64 = 0.4;

/// The family or style names of joined script faces.
pub fn hand_of(family: &str, style: &str) -> Hand {
    let name = format!("{family} {style}").to_ascii_lowercase();
    let script = ["script", "brush", "hand", "cursive", "calligraph", "signature", "copperplate", "chancery"];
    if script.iter().any(|w| name.contains(w)) { Hand::Script } else { Hand::Print }
}

/// An end of a piece: which piece, and whether it is its start.
#[derive(Clone, Copy, Debug, PartialEq)]
struct End {
    piece: usize,
    start: bool,
}

/// `pieces` (a letter's skeleton) put back together into pen strokes, in the order they are drawn.
pub fn pen_strokes(pieces: &[Stroke], hand: Hand) -> Vec<PenStroke> {
    let pieces: Vec<&Stroke> = pieces.iter().filter(|s| s.points.len() >= 2 && s.points.len() == s.radius.len()).collect();
    if pieces.is_empty() {
        return vec![];
    }
    let at = |e: End| -> Point {
        let p = pieces.get(e.piece).map(|s| &s.points);
        p.and_then(|p| if e.start { p.first() } else { p.last() }).copied().unwrap_or_default()
    };
    // Which piece is closed on itself (a ring: an O).
    let ring = |s: &Stroke| s.points.len() > 3 && s.points.first().zip(s.points.last()).is_some_and(|(a, b)| a.distance(*b) < 1e-6);
    // Junctions: piece ends close together (within the stroke's width there).
    let mut ends: Vec<End> = vec![];
    for (i, s) in pieces.iter().enumerate() {
        if !ring(s) {
            ends.push(End { piece: i, start: true });
            ends.push(End { piece: i, start: false });
        }
    }
    let half_at = |e: End| -> f64 {
        let r = pieces.get(e.piece).map(|s| &s.radius);
        r.and_then(|r| if e.start { r.first() } else { r.last() }).copied().unwrap_or(0.5)
    };
    let mut node_of: Vec<usize> = vec![usize::MAX; ends.len()];
    let mut nodes = 0;
    for i in 0..ends.len() {
        if node_of.get(i) != Some(&usize::MAX) {
            continue;
        }
        let (Some(&ei), n) = (ends.get(i), nodes) else { continue };
        nodes += 1;
        for j in i..ends.len() {
            let Some(&ej) = ends.get(j) else { continue };
            let reach = (half_at(ei).max(half_at(ej)) * 1.5).max(0.3);
            if node_of.get(j) == Some(&usize::MAX)
                && at(ei).distance(at(ej)) <= reach
                && let Some(nj) = node_of.get_mut(j)
            {
                *nj = n;
            }
        }
    }
    // The way a piece leaves its end (into the piece), over about a stroke's width.
    let leaving = |e: End| -> Vec2 {
        let Some(s) = pieces.get(e.piece) else { return Vec2::ZERO };
        let n = s.points.len();
        let span = ((half_at(e) * 3.0).max(0.6), n);
        let p0 = at(e);
        let walk: Box<dyn Iterator<Item = &Point>> = if e.start { Box::new(s.points.iter()) } else { Box::new(s.points.iter().rev()) };
        let q = walk
            .skip(1)
            .find(|q| q.distance(p0) >= span.0)
            .copied()
            .unwrap_or_else(|| if e.start { s.points.last().copied().unwrap_or(p0) } else { s.points.first().copied().unwrap_or(p0) });
        let d = q - p0;
        if d.hypot() > 0.0 { d / d.hypot() } else { Vec2::ZERO }
    };
    // Through each junction, pair the ends that carry straight on: most nearly opposite first.
    let straight = match hand {
        Hand::Print => STRAIGHT_PRINT,
        Hand::Script => STRAIGHT_SCRIPT,
    };
    let mut partner: Vec<Option<usize>> = vec![None; ends.len()];
    for n in 0..nodes {
        let here: Vec<usize> = (0..ends.len()).filter(|i| node_of.get(*i) == Some(&n)).collect();
        if here.len() < 2 {
            continue;
        }
        let mut pairs: Vec<(f64, usize, usize)> = vec![];
        for (a, &i) in here.iter().enumerate() {
            for &j in here.iter().skip(a + 1) {
                let (Some(&ei), Some(&ej)) = (ends.get(i), ends.get(j)) else { continue };
                if ei.piece == ej.piece {
                    continue;
                }
                // Coming in along one and leaving along the other: the turn between them.
                let turn = (-leaving(ei)).dot(leaving(ej)).clamp(-1.0, 1.0).acos().to_degrees();
                if turn <= straight {
                    pairs.push((turn, i, j));
                }
            }
        }
        pairs.sort_by(|x, y| x.0.total_cmp(&y.0));
        for (_, i, j) in pairs {
            if partner.get(i).copied().flatten().is_none() && partner.get(j).copied().flatten().is_none() {
                if let Some(p) = partner.get_mut(i) {
                    *p = Some(j);
                }
                if let Some(p) = partner.get_mut(j) {
                    *p = Some(i);
                }
            }
        }
    }
    let end_index = |piece: usize, start: bool| ends.iter().position(|e| e.piece == piece && e.start == start);
    // Chains of pieces joined through junctions.
    let mut used = vec![false; pieces.len()];
    let mut chains: Vec<PenStroke> = vec![];
    for (i, s) in pieces.iter().enumerate() {
        if ring(s) {
            used[i] = true;
            chains.push(PenStroke { points: s.points.clone(), half: s.radius.clone(), closed: true, at_junction: (false, false) });
        }
    }
    // Start chains at piece ends that have no partner (tips, or ends at a junction nothing carries on
    // from), then any left (loops of pieces).
    let mut starts: Vec<(usize, bool)> = vec![];
    for (k, e) in ends.iter().enumerate() {
        if partner.get(k).copied().flatten().is_none() {
            starts.push((e.piece, e.start));
        }
    }
    for i in 0..pieces.len() {
        starts.push((i, true));
    }
    for (first, from_start) in starts {
        if used.get(first).copied().unwrap_or(true) {
            continue;
        }
        let mut points: Vec<Point> = vec![];
        let mut half: Vec<f64> = vec![];
        let (mut piece, mut forward) = (first, from_start);
        // Whether an end of the chain is at a junction: other piece ends meet there.
        let crowded = |piece: usize, start: bool| {
            end_index(piece, start).and_then(|k| node_of.get(k)).is_some_and(|n| node_of.iter().filter(|m| *m == n).count() > 1)
        };
        let first_at = crowded(first, from_start);
        let mut last_at = false;
        let mut guard = 0;
        loop {
            guard += 1;
            let Some(s) = pieces.get(piece) else { break };
            if used.get(piece).copied().unwrap_or(true) || guard > pieces.len() + 1 {
                break;
            }
            if let Some(u) = used.get_mut(piece) {
                *u = true;
            }
            let (pts, rad): (Vec<Point>, Vec<f64>) = if forward {
                (s.points.clone(), s.radius.clone())
            } else {
                (s.points.iter().rev().copied().collect(), s.radius.iter().rev().copied().collect())
            };
            // Skip the joining point where it repeats.
            let skip = usize::from(points.last().zip(pts.first()).is_some_and(|(a, b)| a.distance(*b) < 1e-9));
            points.extend(pts.iter().skip(skip));
            half.extend(rad.iter().skip(skip));
            // On through the far end's partner, if any.
            let Some(far) = end_index(piece, !forward) else { break };
            last_at = crowded(piece, !forward);
            let Some(next) = partner.get(far).copied().flatten().and_then(|k| ends.get(k).copied()) else { break };
            piece = next.piece;
            forward = next.start;
        }
        if points.len() >= 2 {
            let closed = points.len() > 3 && points.first().zip(points.last()).is_some_and(|(a, b)| a.distance(*b) < 0.3);
            chains.push(PenStroke { points, half, closed, at_junction: (first_at, last_at && !closed) });
        }
    }
    // Inside a junction the skeleton's half-width swells to the junction's: a stroke keeps its own.
    for c in &mut chains {
        keep_own_width(c);
    }
    let mut strokes: Vec<PenStroke> = if hand == Hand::Print { chains.into_iter().flat_map(split_corners).collect() } else { chains };
    order(&mut strokes, hand);
    strokes
}

/// Where the half-width rises well above the stroke's own (inside a junction), use the width
/// either side of the swell.
fn keep_own_width(s: &mut PenStroke) {
    let n = s.half.len();
    if n < 3 {
        return;
    }
    let mut sorted = s.half.clone();
    sorted.sort_by(f64::total_cmp);
    let own = sorted.get(n / 2).copied().unwrap_or(0.0);
    let limit = own * 1.12;
    let mut i = 0;
    while i < n {
        if s.half.get(i).is_some_and(|h| *h > limit) {
            let a = i;
            while i < n && s.half.get(i).is_some_and(|h| *h > limit) {
                i += 1;
            }
            // Across the swell, from the width before it to the width after.
            let before = a.checked_sub(1).and_then(|k| s.half.get(k)).copied().unwrap_or(own).min(limit);
            let after = s.half.get(i).copied().unwrap_or(own).min(limit);
            let len = (i - a + 1) as f64;
            for k in a..i {
                if let Some(h) = s.half.get_mut(k) {
                    let t = (k - a + 1) as f64 / len;
                    *h = before + (after - before) * t;
                }
            }
        } else {
            i += 1;
        }
    }
}

/// Break `s` at its sharp corners (print).
fn split_corners(s: PenStroke) -> Vec<PenStroke> {
    let n = s.points.len();
    if n < 5 || s.closed {
        return vec![s];
    }
    let mut cuts: Vec<usize> = vec![];
    // The turn at each point, over about a stroke's width either side.
    let mut k = 1;
    while k + 1 < n {
        let w = s.half.get(k).copied().unwrap_or(0.5).max(0.3) * 2.0;
        let back = (0..k).rev().find(|j| s.points.get(*j).zip(s.points.get(k)).is_some_and(|(a, b)| a.distance(*b) >= w));
        let ahead = (k + 1..n).find(|j| s.points.get(*j).zip(s.points.get(k)).is_some_and(|(a, b)| a.distance(*b) >= w));
        if let (Some(b), Some(f), Some(&p)) = (back, ahead, s.points.get(k)) {
            let (Some(&pb), Some(&pf)) = (s.points.get(b), s.points.get(f)) else { break };
            let (u, v) = (p - pb, pf - p);
            if u.hypot() > 0.0 && v.hypot() > 0.0 {
                let turn = (u.dot(v) / (u.hypot() * v.hypot())).clamp(-1.0, 1.0).acos().to_degrees();
                if turn > CORNER && cuts.last().is_none_or(|c| k - c > 2) {
                    // The sharpest point of this corner.
                    cuts.push(k);
                    k = f;
                    continue;
                }
            }
        }
        k += 1;
    }
    if cuts.is_empty() {
        return vec![s];
    }
    let mut out = vec![];
    let mut from = 0;
    for c in cuts.into_iter().chain(std::iter::once(n - 1)) {
        let pts: Vec<Point> = s.points.get(from..=c).unwrap_or_default().to_vec();
        let half: Vec<f64> = s.half.get(from..=c).unwrap_or_default().to_vec();
        if pts.len() >= 2 {
            // At a corner the pen carries each stroke on to the letter's edge, as through a junction:
            // the corner's outer square is sewn by whichever stroke comes first.
            let at = (from != 0 || s.at_junction.0, c != n - 1 || s.at_junction.1);
            out.push(PenStroke { points: pts, half, closed: false, at_junction: at });
        }
        from = c;
    }
    out
}

/// The way `s` runs: its angle from upright, degrees (0 upright, 90 level), from end to end.
fn lean(s: &PenStroke) -> f64 {
    let (Some(a), Some(b)) = (s.points.first(), s.points.last()) else { return 90.0 };
    let d = *b - *a;
    if d.hypot() < 1e-9 {
        return 90.0;
    }
    d.x.abs().atan2(d.y.abs()).to_degrees()
}

/// Put `strokes` in the order and direction a pen draws them.
fn order(strokes: &mut [PenStroke], hand: Hand) {
    let top = |s: &PenStroke| s.points.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let left = |s: &PenStroke| s.points.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    for s in strokes.iter_mut() {
        if s.closed {
            // A loop starts at its top.
            if let Some(k) = s.points.iter().enumerate().min_by(|a, b| a.1.y.total_cmp(&b.1.y)).map(|(k, _)| k) {
                let n = s.points.len();
                if n > 2 && k > 0 {
                    // Drop the closing point, turn, close again.
                    s.points.pop();
                    s.half.pop();
                    let (kp, kh) = (k.min(s.points.len().saturating_sub(1)), k.min(s.half.len().saturating_sub(1)));
                    s.points.rotate_left(kp);
                    s.half.rotate_left(kh);
                    if let (Some(&p), Some(&h)) = (s.points.first(), s.half.first()) {
                        s.points.push(p);
                        s.half.push(h);
                    }
                }
            }
            continue;
        }
        let (Some(a), Some(b)) = (s.points.first().copied(), s.points.last().copied()) else { continue };
        let flip = match hand {
            // Downstrokes from the top, the others left to right (a level stroke) or from the top.
            Hand::Print if lean(s) <= 90.0 - UPRIGHT => a.y > b.y,
            Hand::Print => a.x > b.x,
            Hand::Script => a.x > b.x + 1e-6 || ((a.x - b.x).abs() <= 1e-6 && a.y > b.y),
        };
        if flip {
            s.points.reverse();
            s.half.reverse();
            s.at_junction = (s.at_junction.1, s.at_junction.0);
        }
    }
    let class = |s: &PenStroke| -> u8 {
        if hand == Hand::Script {
            return 0;
        }
        let l = lean(s);
        if s.closed {
            1
        } else if l <= UPRIGHT {
            0
        } else if l >= 90.0 - UPRIGHT {
            2
        } else {
            1
        }
    };
    strokes.sort_by(|a, b| {
        let (ca, cb) = (class(a), class(b));
        ca.cmp(&cb).then_with(|| match (hand, ca) {
            (Hand::Script, _) | (_, 0) => left(a).total_cmp(&left(b)).then(top(a).total_cmp(&top(b))),
            _ => top(a).total_cmp(&top(b)).then(left(a).total_cmp(&left(b))),
        })
    });
}

/// Carry each end of `s` that stops at a junction straight on across the stroke there, to the far
/// side of the letter (`inside` tells whether a point is in it): the pen draws a crossbar right
/// across a stem, so a short arm beyond it (which the centre line misses) is sewn as part of the
/// crossbar. Inside the stroke it crosses, the crossing is left out as any overlap is
/// ([`under`]).
pub fn carry_through(s: &mut PenStroke, step: f64, inside: impl Fn(Point) -> bool) {
    for at_start in [true, false] {
        let on = if at_start { s.at_junction.0 } else { s.at_junction.1 };
        let n = s.points.len();
        if !on || n < 2 || s.closed {
            continue;
        }
        let (end, back) = if at_start { (0, (n - 1).min(6)) } else { (n - 1, n.saturating_sub(7)) };
        let (Some(&p), Some(&q), Some(&h)) = (s.points.get(end), s.points.get(back), s.half.get(end)) else { continue };
        let d = p - q;
        if d.hypot() < 1e-9 || step <= 0.0 {
            continue;
        }
        let d = d / d.hypot();
        // As far as the letter goes on that way, within a few stroke widths.
        let limit = (h * 8.0).max(1.0);
        let mut t = step;
        while t < limit && inside(p + d * t) {
            t += step;
        }
        let reach = t - step - 0.15;
        if reach < step {
            continue;
        }
        let added: Vec<Point> = (1..=(reach / step).floor() as usize).map(|k| p + d * (k as f64 * step)).collect();
        let widths = vec![h; added.len()];
        if at_start {
            let mut pts: Vec<Point> = added.into_iter().rev().collect();
            pts.extend(s.points.iter());
            let mut hs = widths;
            hs.extend(s.half.iter());
            (s.points, s.half) = (pts, hs);
        } else {
            s.points.extend(added);
            s.half.extend(widths);
        }
    }
}

/// Whether `p` lies under one of `drawn` (an earlier stroke), more than [`TUCK`] in from its edge.
pub fn under(p: Point, drawn: &[PenStroke]) -> bool {
    drawn.iter().any(|s| {
        s.points.windows(2).zip(s.half.windows(2)).any(|(w, h)| {
            let (a, b) = (w[0], w[1]);
            let ab = b - a;
            let len2 = ab.hypot2();
            let t = if len2 > 0.0 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
            let half = h[0] + (h[1] - h[0]) * t;
            p.distance(a + ab * t) < half - TUCK
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_geom::FillRule;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![Point::new(x0, y0), Point::new(x1, y0), Point::new(x1, y1), Point::new(x0, y1)]
    }

    /// The union of rectangles as one outline is hard to write; strokes are found from the
    /// skeleton of their union, rasterized from overlapping outlines (non-zero fill).
    fn strokes_of(rects: &[Vec<Point>], hand: Hand) -> Vec<PenStroke> {
        pen_strokes(&crate::skeleton::strokes(rects, FillRule::NonZero), hand)
    }

    fn is_upright(s: &PenStroke) -> bool {
        lean(s) <= UPRIGHT
    }

    #[test]
    fn a_t_is_a_downstroke_then_a_crossbar() {
        // A stem 2 mm wide, 20 tall; a crossbar 2 mm tall crossing it near the top.
        let t = strokes_of(&[rect(9.0, 0.0, 11.0, 20.0), rect(4.0, 4.0, 16.0, 6.0)], Hand::Print);
        assert_eq!(t.len(), 2, "{t:?}");
        assert!(is_upright(&t[0]), "the downstroke first");
        assert!(t[0].points[0].y < t[0].points.last().unwrap().y, "pulled from the top down");
        assert!(!is_upright(&t[1]) && t[1].points[0].x < t[1].points.last().unwrap().x, "the crossbar, left to right");
        // The stem keeps its own width through the junction.
        assert!(t[0].half.iter().all(|h| *h < 1.2), "{:?}", t[0].half);
    }

    #[test]
    fn an_e_is_its_spine_and_three_arms() {
        let e =
            strokes_of(&[rect(0.0, 0.0, 2.0, 20.0), rect(0.0, 0.0, 10.0, 2.0), rect(0.0, 9.0, 8.0, 11.0), rect(0.0, 18.0, 10.0, 20.0)], Hand::Print);
        assert_eq!(e.len(), 4, "{:?}", e.iter().map(|s| (s.points.first(), s.points.last())).collect::<Vec<_>>());
        assert!(is_upright(&e[0]));
        let arms: Vec<f64> = e[1..].iter().map(|s| s.points.iter().map(|p| p.y).sum::<f64>() / s.points.len() as f64).collect();
        assert!(arms.windows(2).all(|w| w[0] < w[1]), "arms top to bottom: {arms:?}");
    }

    #[test]
    fn an_l_breaks_at_its_corner_in_print_not_in_script() {
        let l = [rect(0.0, 0.0, 2.0, 20.0), rect(0.0, 18.0, 12.0, 20.0)];
        assert_eq!(strokes_of(&l, Hand::Print).len(), 2);
        assert_eq!(strokes_of(&l, Hand::Script).len(), 1, "the pen keeps going in script");
    }

    #[test]
    fn crossings_are_sewn_once() {
        let t = strokes_of(&[rect(9.0, 0.0, 11.0, 20.0), rect(4.0, 4.0, 16.0, 6.0)], Hand::Print);
        // The middle of the crossing is under the stem: the crossbar leaves it out.
        assert!(under(Point::new(10.0, 5.0), &t[..1]));
        // Its arms, and the tuck at the stem's edge, are sewn.
        assert!(!under(Point::new(6.0, 5.0), &t[..1]));
        assert!(!under(Point::new(9.0 + TUCK * 0.5, 5.0), &t[..1]));
    }

    /// Sewn by pen strokes, the shape (overlapping outlines, non-zero) is left bare nowhere.
    fn bare(rects: &[Vec<Point>], hand: Hand) -> f64 {
        let pieces = crate::skeleton::strokes(rects, FillRule::NonZero);
        let (runs, q) = crate::generate::satin_pen(rects, FillRule::NonZero, &pieces, 0.4, 0.0, false, 0.0, hand, &[]).unwrap();
        assert!(q.uncovered <= 0.05, "the satin itself leaves {:.1}% bare", q.uncovered * 100.0);
        crate::generate::uncovered(rects, FillRule::NonZero, &runs)
    }

    #[test]
    fn corners_and_short_arms_are_sewn_by_the_strokes_that_reach_them() {
        // An L: the corner's outer square, by the downstroke carried on to the edge.
        assert!(bare(&[rect(0.0, 0.0, 2.0, 20.0), rect(0.0, 18.0, 12.0, 20.0)], Hand::Print) < 0.01);
        // A t whose crossbar reaches only 1 mm left of its stem (a stub the centre line misses):
        // the crossbar is carried across the stem to cover it.
        assert!(bare(&[rect(9.0, 0.0, 11.0, 20.0), rect(8.0, 4.0, 16.0, 6.0)], Hand::Print) < 0.01);
        // An E and an H.
        assert!(
            bare(&[rect(0.0, 0.0, 2.0, 20.0), rect(0.0, 0.0, 10.0, 2.0), rect(0.0, 9.0, 8.0, 11.0), rect(0.0, 18.0, 10.0, 20.0)], Hand::Print) < 0.01
        );
        assert!(bare(&[rect(0.0, 0.0, 2.0, 20.0), rect(10.0, 0.0, 12.0, 20.0), rect(0.0, 9.0, 12.0, 11.0)], Hand::Print) < 0.01);
    }

    #[test]
    fn script_is_told_by_its_name() {
        assert_eq!(hand_of("Brush Script MT", "Italic"), Hand::Script);
        assert_eq!(hand_of("Arial", "Italic"), Hand::Print);
    }
}
