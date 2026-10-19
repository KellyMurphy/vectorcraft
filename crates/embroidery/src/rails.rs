//! Satin between two rails: a satin column drawn as its two edges (one object, two subpaths),
//! the stitches spanning from one to the other, so the column is as wide as the rails are
//! apart. Further short subpaths crossing both rails are **rungs**: each sets the stitches'
//! direction where it crosses; between rungs the rails are divided evenly by length.

use kurbo::Point;
use vectorcraft_doc::Underlay;

use crate::generate::{MAX_STITCH, running, short_stitches, split_long};

/// Edge-walk underlay inset from the rails, mm.
const INSET: f64 = 0.4;
/// Underlay stitch and zigzag spacing, mm.
const UNDERLAY_LEN: f64 = 2.0;

/// A polyline measured by length.
#[derive(Clone, Debug)]
struct Rail {
    pts: Vec<Point>,
    cum: Vec<f64>,
}

impl Rail {
    fn new(pts: Vec<Point>) -> Self {
        let mut cum = vec![0.0];
        for w in pts.windows(2) {
            cum.push(cum.last().copied().unwrap_or(0.0) + w[0].distance(w[1]));
        }
        Self { pts, cum }
    }

    fn len(&self) -> f64 {
        self.cum.last().copied().unwrap_or(0.0)
    }

    /// The point `s` mm along.
    fn at(&self, s: f64) -> Point {
        let s = s.clamp(0.0, self.len());
        let k = self.cum.partition_point(|&c| c < s).saturating_sub(1);
        match (self.pts.get(k), self.pts.get(k + 1), self.cum.get(k), self.cum.get(k + 1)) {
            (Some(&a), Some(&b), Some(&ca), Some(&cb)) if cb > ca => a.lerp(b, (s - ca) / (cb - ca)),
            (Some(&a), ..) => a,
            _ => Point::ZERO,
        }
    }

    /// Where the polyline `line` first crosses this rail: the distance along the rail.
    fn crossing(&self, line: &[Point]) -> Option<f64> {
        for w in line.windows(2) {
            for (k, r) in self.pts.windows(2).enumerate() {
                let (p, d) = (w[0], w[1] - w[0]);
                let (q, e) = (r[0], r[1] - r[0]);
                let den = d.cross(e);
                if den.abs() < 1e-12 {
                    continue;
                }
                let t = (q - p).cross(e) / den;
                let u = (q - p).cross(d) / den;
                if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                    return Some(self.cum.get(k).copied().unwrap_or(0.0) + u * e.hypot());
                }
            }
        }
        None
    }
}

/// A satin column's two rails and its rungs (as (distance along A, distance along B), in order).
pub struct Rails {
    a: Rail,
    b: Rail,
    rungs: Vec<(f64, f64)>,
}

/// The rails of an object whose subpaths are `lines` (polylines, mm, with whether each is
/// closed): its two longest subpaths, both open or both closed, the second turned to run the
/// same way as the first (a closed one also started opposite the first's start); the other open
/// subpaths that cross both are rungs. None when the object has fewer than two such subpaths.
pub fn find(lines: &[(Vec<Point>, bool)]) -> Option<Rails> {
    let length = |l: &[Point]| l.windows(2).map(|w| w[0].distance(w[1])).sum::<f64>();
    let mut order: Vec<usize> = (0..lines.len()).filter(|&i| lines.get(i).is_some_and(|(l, _)| l.len() > 1)).collect();
    order.sort_by(|&i, &j| {
        let li = lines.get(i).map_or(0.0, |(l, _)| length(l));
        let lj = lines.get(j).map_or(0.0, |(l, _)| length(l));
        lj.total_cmp(&li)
    });
    let (&ia, &ib) = (order.first()?, order.get(1)?);
    let ((a, ca), (b, cb)) = (lines.get(ia)?, lines.get(ib)?);
    if ca != cb || length(a) <= 0.0 || length(b) <= 0.0 {
        return None;
    }
    let mut b = b.clone();
    let (a0, a1) = (*a.first()?, *a.last()?);
    if *ca {
        // Same winding, then start B nearest A's start.
        let area = |l: &[Point]| l.windows(2).map(|w| w[0].x * w[1].y - w[1].x * w[0].y).sum::<f64>();
        if area(a) * area(&b) < 0.0 {
            b.reverse();
        }
        b.pop();
        let start = (0..b.len()).min_by(|&i, &j| {
            let d = |k: usize| b.get(k).map_or(f64::INFINITY, |p| p.distance(a0));
            d(i).total_cmp(&d(j))
        })?;
        b.rotate_left(start);
        let first = *b.first()?;
        b.push(first);
    } else {
        let (b0, b1) = (*b.first()?, *b.last()?);
        if a0.distance(b0) + a1.distance(b1) > a0.distance(b1) + a1.distance(b0) {
            b.reverse();
        }
    }
    let (ra, rb) = (Rail::new(a.clone()), Rail::new(b));
    let mut rungs: Vec<(f64, f64)> = order
        .iter()
        .skip(2)
        .filter_map(|&i| lines.get(i))
        .filter(|(_, closed)| !closed)
        .filter_map(|(l, _)| Some((ra.crossing(l)?, rb.crossing(l)?)))
        .collect();
    rungs.sort_by(|x, y| x.0.total_cmp(&y.0));
    // Rungs that cross each other can't both hold: keep those that go forward on both rails.
    let mut kept: Vec<(f64, f64)> = vec![];
    for r in rungs {
        if kept.last().is_none_or(|l| r.0 > l.0 && r.1 > l.1) {
            kept.push(r);
        }
    }
    Some(Rails { a: ra, b: rb, rungs: kept })
}

impl Rails {
    /// Pairs of points across the column every `spacing` mm (along the longer rail), the rails
    /// moved toward each other by `inset` mm each.
    fn pairs(&self, spacing: f64, inset: f64) -> Vec<(Point, Point)> {
        let mut controls = vec![(0.0, 0.0)];
        controls.extend(self.rungs.iter().copied());
        controls.push((self.a.len(), self.b.len()));
        let mut out = vec![];
        for w in controls.windows(2) {
            let ((a0, b0), (a1, b1)) = (w[0], w[1]);
            let span = (a1 - a0).max(b1 - b0).max(0.0);
            let n = ((span / spacing).ceil() as usize).clamp(1, 100_000);
            for k in 0..n {
                let f = k as f64 / n as f64;
                out.push((self.a.at(a0 + (a1 - a0) * f), self.b.at(b0 + (b1 - b0) * f)));
            }
        }
        out.push((self.a.at(self.a.len()), self.b.at(self.b.len())));
        if inset > 0.0 {
            out = out
                .into_iter()
                .map(|(p, q)| {
                    let d = q.distance(p);
                    let t = if d > 0.0 { (inset / d).min(0.5) } else { 0.0 };
                    (p.lerp(q, t), q.lerp(p, t))
                })
                .collect();
        }
        out
    }

    /// The satin: its underlay (`underlay`), then the column, every `spacing` mm, each stitch
    /// reaching `pull` mm past the rails in all (half each side), no stitch longer than can be
    /// sewn. One run; and whether the column had to be narrowed somewhere to be sewable.
    pub fn satin(&self, spacing: f64, pull: f64, pull_percent: f64, underlay: Underlay, auto: &crate::generate::AutoUnderlay) -> (Vec<Point>, bool) {
        let most = (MAX_STITCH - spacing).max(0.3);
        // The pull at a rung `len` mm long: the mm, plus the percentage of its length.
        let pull_at = |len: f64| pull + len * pull_percent / 100.0;
        let narrowed = self.pairs(spacing, 0.0).iter().any(|(p, q)| p.distance(*q) + pull_at(p.distance(*q)) > most);
        let mut zig: Vec<(Point, Point)> = self
            .pairs(spacing, 0.0)
            .into_iter()
            .map(|(p, q)| {
                let d = q - p;
                let len = d.hypot();
                if len <= 1e-9 {
                    return (p, q);
                }
                let u = d / len;
                let pull = pull_at(len);
                let (p, q) = (p - u * (pull / 2.0), q + u * (pull / 2.0));
                if len + pull > most {
                    let mid = p.midpoint(q);
                    let k = most / (len + pull);
                    (mid + (p - mid) * k, mid + (q - mid) * k)
                } else {
                    (p, q)
                }
            })
            .collect();
        let mut run: Vec<Point> = vec![];
        // Auto: by the column's mean width.
        let underlays: &[Underlay] = match underlay {
            Underlay::Auto => {
                let widths: Vec<f64> = zig.iter().map(|(p, q)| p.distance(*q)).collect();
                let mean = if widths.is_empty() { 0.0 } else { widths.iter().sum::<f64>() / widths.len() as f64 };
                auto.satin(mean)
            }
            _ => std::slice::from_ref(&underlay),
        };
        let mut back = false;
        for u in underlays {
            back = self.underlay(*u, &mut run);
        }
        if back {
            zig.reverse();
        }
        run.extend(short_stitches(&zig).0);
        // A wide column's underlay zigs, and the move from the underlay onto the column, are
        // split where too long to sew at once (the column sews over them).
        (split_long(&run), narrowed)
    }

    /// Add underlay `underlay` to `run`; whether it ends at the column's far end (so the column is
    /// sewn back over it).
    fn underlay(&self, underlay: Underlay, run: &mut Vec<Point>) -> bool {
        let mid = |pairs: &[(Point, Point)]| pairs.iter().map(|(p, q)| p.midpoint(*q)).collect::<Vec<_>>();
        match underlay {
            Underlay::Center => {
                run.extend(running(&mid(&self.pairs(UNDERLAY_LEN, 0.0)), UNDERLAY_LEN));
                true
            }
            Underlay::Edge => {
                let p = self.pairs(UNDERLAY_LEN, INSET);
                run.extend(p.iter().map(|(a, _)| *a));
                run.extend(p.iter().rev().map(|(_, b)| *b));
                false
            }
            Underlay::Zigzag => {
                let p = self.pairs(UNDERLAY_LEN, INSET);
                run.extend(p.iter().enumerate().map(|(k, (a, b))| if k % 2 == 0 { *a } else { *b }));
                true
            }
            Underlay::None | Underlay::Fill | Underlay::Auto => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(pts: &[(f64, f64)]) -> Vec<Point> {
        pts.iter().map(|&(x, y)| Point::new(x, y)).collect()
    }

    #[test]
    fn two_lines_make_a_column_between_them() {
        let r = find(&[(line(&[(0.0, 0.0), (30.0, 0.0)]), false), (line(&[(0.0, 4.0), (30.0, 4.0)]), false)]).unwrap();
        let s = r.satin(0.4, 0.0, 0.0, Underlay::None, &Default::default()).0;
        assert!(s.len() > 70);
        for (k, p) in s.iter().enumerate() {
            assert!(p.y.abs() < 1e-9 || (p.y - 4.0).abs() < 1e-9, "{p:?}");
            if let Some(q) = s.get(k + 1) {
                assert!((p.y - q.y).abs() > 3.9, "consecutive points alternate rails");
            }
        }
    }

    #[test]
    fn a_rail_drawn_backward_is_turned() {
        let r = find(&[(line(&[(0.0, 0.0), (30.0, 0.0)]), false), (line(&[(30.0, 4.0), (0.0, 4.0)]), false)]).unwrap();
        // Stitches go straight across, not diagonally end to end.
        for w in r.satin(0.4, 0.0, 0.0, Underlay::None, &Default::default()).0.windows(2) {
            assert!((w[0].x - w[1].x).abs() < 0.5, "{:?}", w);
        }
    }

    #[test]
    fn a_tapered_column_follows_its_rails_and_pull_widens_it() {
        let r = find(&[(line(&[(0.0, 0.0), (30.0, 0.0)]), false), (line(&[(0.0, 1.0), (30.0, 6.0)]), false)]).unwrap();
        let s = r.satin(0.4, 0.0, 0.0, Underlay::None, &Default::default()).0;
        let at = |x: f64| s.iter().filter(|p| (p.x - x).abs() < 0.3).map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
        assert!((at(0.5) - 1.0).abs() < 0.3 && (at(29.5) - 6.0).abs() < 0.3);
        let pulled = r.satin(0.4, 1.0, 0.0, Underlay::None, &Default::default()).0;
        let hi = pulled.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
        let lo = pulled.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
        assert!(hi > 6.3 && lo < -0.3, "{lo}..{hi}");
    }

    #[test]
    fn rungs_set_the_direction() {
        // A rung from (10, 0) to (20, 4): stitches there lean the same way.
        let r = find(&[
            (line(&[(0.0, 0.0), (30.0, 0.0)]), false),
            (line(&[(0.0, 4.0), (30.0, 4.0)]), false),
            (line(&[(10.0, -1.0), (22.5, 5.0)]), false),
        ])
        .unwrap();
        assert_eq!(r.rungs.len(), 1);
        let s = r.satin(0.4, 0.0, 0.0, Underlay::None, &Default::default()).0;
        // At the rung, a point on the bottom rail at x≈12 pairs with the top near x≈20.
        let near = s.windows(2).find(|w| w[0].y.abs() < 1e-9 && (w[0].x - 12.08).abs() < 0.3).unwrap();
        assert!((near[1].x - 20.4).abs() < 1.0, "{near:?}");
    }

    #[test]
    fn closed_rails_make_a_ring() {
        let ring = |r: f64, n: usize| {
            (0..=n)
                .map(|k| {
                    let a = k as f64 * std::f64::consts::TAU / n as f64;
                    Point::new(r * a.cos(), r * a.sin())
                })
                .collect::<Vec<_>>()
        };
        let r = find(&[(ring(10.0, 48), true), (ring(7.0, 48).into_iter().rev().collect(), true)]).unwrap();
        let s = r.satin(0.4, 0.0, 0.0, Underlay::Center, &Default::default()).0;
        for p in s.iter().skip(40) {
            let d = p.to_vec2().hypot();
            assert!(d > 6.9 && d < 10.1, "{p:?} off the ring");
        }
    }

    #[test]
    fn one_subpath_or_mixed_kinds_have_no_rails() {
        assert!(find(&[(line(&[(0.0, 0.0), (30.0, 0.0)]), false)]).is_none());
        assert!(find(&[(line(&[(0.0, 0.0), (30.0, 0.0)]), false), (line(&[(0.0, 4.0), (30.0, 4.0), (0.0, 4.0)]), true)]).is_none());
    }

    #[test]
    fn stitches_stay_sewable_however_far_apart_the_rails() {
        let r = find(&[(line(&[(0.0, 0.0), (30.0, 0.0)]), false), (line(&[(0.0, 20.0), (30.0, 20.0)]), false)]).unwrap();
        let (s, narrowed) = r.satin(0.4, 2.0, 0.0, Underlay::Zigzag, &Default::default());
        assert!(narrowed, "20 mm apart: narrowed to be sewable");
        for w in s.windows(2) {
            assert!(w[0].distance(w[1]) <= 12.0 + 1e-9);
        }
    }
}
