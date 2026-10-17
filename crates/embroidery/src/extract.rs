//! Border pattern extraction: the repeat of a stitched border (a stretch of an opened DST
//! design), found and cut out as a [`BorderPattern`] that joins end to end.
//!
//! The border is a run of needle points along a roughly straight stretch. Its repeat is the shift
//! that lines the run up with itself: first sought as a number of stitches `k` after which every
//! needle point recurs moved the same way (designs whose repeats were copied), else as a distance
//! along the border at which the stitched line, shifted, lies on itself (repeats sewn with
//! differing stitch counts). With a stitch count, every whole repeat found is averaged into one,
//! which cancels the file's rounding (DST: 0.1 mm).

use kurbo::{Point, Vec2};
use vectorcraft_doc::{BorderPattern, PartStitch, PatternPart};
use vectorcraft_geom::FillRule;

use crate::generate;

/// The most needle points looked at.
const MAX_POINTS: usize = 50_000;
/// Needle points farther than this from the origin (mm) are left out: no design is 100 m across.
const MAX_COORD: f64 = 100_000.0;
/// How far (mm, root mean square) the needle points of one repeat may sit from those of the next
/// for them to count as the same repeat.
const MATCH_MM: f64 = 0.3;
/// How far (mm, mean) the shifted line may sit from itself for a distance to be a repeat.
const LINE_MATCH_MM: f64 = 0.25;
/// The shortest and longest repeats sought, mm (longer ones can be given).
const MIN_PERIOD: f64 = 1.0;
const MAX_PERIOD: f64 = 200.0;
/// The step distances are tried at, mm.
const PERIOD_STEP: f64 = 0.05;
/// The most needle points in one repeat sought by stitch count.
const MAX_COUNT: usize = 2_000;
/// The most needle points compared at each count or distance tried (evenly spaced).
const SAMPLE: usize = 2_000;

/// A border pattern found in a run of stitches, and how well it fits.
#[derive(Clone, Debug)]
pub struct Extracted {
    pub pattern: BorderPattern,
    /// The repeat's length, mm.
    pub period: f64,
    /// Needle points in one repeat.
    pub stitches: usize,
    /// Whole repeats found in the stretch.
    pub repeats: usize,
    /// How far, on average, the repeats differ from the pattern, mm.
    pub error: f64,
}

/// The needle points to look at: of `runs` (mm), the longest unbroken stretch inside `region`
/// (closed outlines, mm; none: everything).
pub fn stretch(runs: &[Vec<Point>], region: Option<&[Vec<Point>]>) -> Vec<Point> {
    let mut best: Vec<Point> = vec![];
    for run in runs {
        let mut cur: Vec<Point> = vec![];
        for &p in run.iter().filter(|p| p.is_finite()) {
            if region.is_none_or(|r| generate::inside(r, FillRule::NonZero, p)) {
                cur.push(p);
            } else if !cur.is_empty() {
                if cur.len() > best.len() {
                    best = std::mem::take(&mut cur);
                }
                cur.clear();
            }
        }
        if cur.len() > best.len() {
            best = cur;
        }
    }
    best.truncate(MAX_POINTS);
    best
}

/// The border pattern repeating along `pts` (needle points in sewing order, mm), named `name`.
/// `period` (mm) skips the search for the repeat's length. Errors say why nothing was found
/// (too few stitches, no repeat, a curving stretch).
pub fn border(pts: &[Point], name: &str, period: Option<f64>) -> Result<Extracted, String> {
    let pts: Vec<Point> =
        pts.iter().copied().filter(|p| p.is_finite() && p.x.abs() <= MAX_COORD && p.y.abs() <= MAX_COORD).take(MAX_POINTS).collect();
    if pts.len() < 6 {
        return Err("select a stretch of stitches with at least two repeats in it".into());
    }
    let found = match period {
        Some(t) if !(t.is_finite() && t > 0.0) => return Err("the repeat length must be a positive number of mm".into()),
        Some(t) => by_count(&pts, Some(t)).or_else(|| by_distance(&pts, Some(t))),
        // The shift at which the line lies on itself is the repeat's length; a stitch count
        // repeating at that length (not at two repeats, where stitches can nearly line up again)
        // gives the averaged repeat.
        None => match by_distance(&pts, None) {
            Some(d) => by_count(&pts, Some(d.period)).or(Some(d)),
            None => by_count(&pts, None),
        },
    };
    let Some(f) = found else {
        return Err("no repeat found in these stitches: select a straight stretch of a border with at least two repeats \
                    (or give the repeat's length)"
            .into());
    };
    Ok(finish(f, name))
}

/// A repeat found: its needle points in border space (x along from its start, y across, right of
/// travel positive), its length and fit.
struct Found {
    points: Vec<[f64; 2]>,
    period: f64,
    repeats: usize,
    error: f64,
}

/// `p` in the frame along unit direction `d` from `o`: (along, across to the right of travel on
/// the page).
fn frame(p: Point, o: Point, d: Vec2) -> (f64, f64) {
    let v = p - o;
    (v.dot(d), v.dot(Vec2::new(-d.y, d.x)))
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v.get(v.len() / 2).copied().unwrap_or(0.0)
}

/// The repeat as a stitch count: the smallest `k` after which every needle point recurs moved by
/// one shift (to within [`MATCH_MM`]), that shift about `want` mm long when given; the repeats
/// averaged into one.
fn by_count(pts: &[Point], want: Option<f64>) -> Option<Found> {
    let n = pts.len();
    let mut tried: Vec<(usize, Vec2, f64)> = vec![];
    for k in 3..=(n / 2).min(MAX_COUNT) {
        let stride = ((n - k) / SAMPLE).max(1);
        let shifts: Vec<Vec2> = (0..n - k).step_by(stride).filter_map(|i| Some(*pts.get(i + k)? - *pts.get(i)?)).collect();
        let d = Vec2::new(median(shifts.iter().map(|s| s.x).collect()), median(shifts.iter().map(|s| s.y).collect()));
        if d.hypot() < MIN_PERIOD || want.is_some_and(|w| (d.hypot() - w).abs() > (0.02 * w).max(0.2)) {
            continue;
        }
        let rms = (shifts.iter().map(|s| (*s - d).hypot2()).sum::<f64>() / shifts.len().max(1) as f64).sqrt();
        tried.push((k, d, rms));
    }
    let best = tried.iter().map(|t| t.2).fold(f64::INFINITY, f64::min);
    if best > MATCH_MM {
        return None;
    }
    // The smallest count that fits about as well (twice the repeat fits too).
    let &(k, shift, _) = tried.iter().find(|t| t.2 <= (best * 1.5).max(best + 0.05))?;
    let period = shift.hypot();
    let dir = shift / period;
    let origin = *pts.first()?;
    let local: Vec<(f64, f64)> = pts.iter().map(|&p| frame(p, origin, dir)).collect();
    // Average every whole repeat (each moved back by its number of periods).
    let repeats = (n - 1) / k;
    let mean: Vec<(f64, f64)> = (0..k)
        .map(|j| {
            let (mut u, mut v) = (0.0, 0.0);
            for m in 0..repeats {
                let (a, b) = local.get(j + m * k).copied().unwrap_or_default();
                u += a - m as f64 * period;
                v += b;
            }
            (u / repeats as f64, v / repeats as f64)
        })
        .collect();
    let error = (0..repeats)
        .flat_map(|m| (0..k).map(move |j| (m, j)))
        .filter_map(|(m, j)| {
            let (a, b) = local.get(j + m * k)?;
            let (u, v) = mean.get(j)?;
            Some((a - m as f64 * period - u).hypot(b - v))
        })
        .sum::<f64>()
        / (repeats * k) as f64;
    Some(from_cycle(&mean, period, repeats, error))
}

/// The repeat from one cycle of averaged points (along, across): centred across the band and
/// started where it crosses the band's middle, so it enters and leaves on the centre line.
fn from_cycle(cycle: &[(f64, f64)], period: f64, repeats: usize, error: f64) -> Found {
    let (lo, hi) = cycle.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), p| (l.min(p.1), h.max(p.1)));
    let mid = (lo + hi) / 2.0;
    let start = (0..cycle.len()).min_by(|&a, &b| {
        let off = |i: usize| cycle.get(i).map_or(f64::INFINITY, |p| (p.1 - mid).abs());
        off(a).total_cmp(&off(b))
    });
    let start = start.unwrap_or(0);
    let k = cycle.len();
    let (u0, _) = cycle.get(start).copied().unwrap_or_default();
    let mut points: Vec<[f64; 2]> = (0..k)
        .map(|j| {
            // Points of the cycle before the start come from the next repeat.
            let i = (start + j) % k;
            let (u, v) = cycle.get(i).copied().unwrap_or_default();
            let wrap = if start + j >= k { period } else { 0.0 };
            [u + wrap - u0, v - mid]
        })
        .collect();
    if let Some(&[x, y]) = points.first() {
        // The last point is the first of the next repeat: the pattern joins end to end.
        points.push([x + period, y]);
    }
    Found { points, period, repeats, error }
}

/// The repeat as a distance along the border: the shortest shift at which the stitched line lies
/// on itself (to within [`LINE_MATCH_MM`]), or `given`. The needle points of one repeat are cut out
/// as they are.
fn by_distance(pts: &[Point], given: Option<f64>) -> Option<Found> {
    // The border's direction: its principal axis, pointing the way it is sewn.
    let c = pts.iter().fold(Vec2::ZERO, |a, p| a + p.to_vec2()) / pts.len() as f64;
    let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
    for p in pts {
        let v = p.to_vec2() - c;
        sxx += v.x * v.x;
        sxy += v.x * v.y;
        syy += v.y * v.y;
    }
    let angle = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let mut dir = Vec2::new(angle.cos(), angle.sin());
    if let (Some(&a), Some(&b)) = (pts.first(), pts.last())
        && (b - a).dot(dir) < 0.0
    {
        dir = -dir;
    }
    let origin = *pts.first()?;
    let local: Vec<(f64, f64)> = pts.iter().map(|&p| frame(p, origin, dir)).collect();
    let (umin, umax) = local.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), p| (l.min(p.0), h.max(p.0)));
    let span = umax - umin;
    let grid = Grid::new(&local);
    let sample: Vec<(f64, f64)> = local.iter().step_by((local.len() / SAMPLE).max(1)).copied().collect();
    let fit = |t: f64| grid.mean_distance(&sample, t, umin, umax);
    let (period, error) = match given {
        Some(t) => (t, fit(t)?),
        None => {
            // Tried from short to long; the first good fit, then the best of the dip it starts (a
            // pattern of long lines fits loosely well before its true repeat).
            let steps = (((span / 2.0).min(MAX_PERIOD) - MIN_PERIOD) / PERIOD_STEP).floor();
            if !(steps.is_finite() && steps >= 1.0) {
                return None;
            }
            let first = (0..=steps as usize).map(|s| MIN_PERIOD + s as f64 * PERIOD_STEP).find(|&t| fit(t).is_some_and(|e| e <= LINE_MATCH_MM))?;
            let limit = (span / 2.0).min(MAX_PERIOD);
            let mut best = (first, fit(first)?);
            let mut t = first;
            while t < limit {
                t += PERIOD_STEP / 2.0;
                let Some(e) = fit(t) else { break };
                if e > LINE_MATCH_MM {
                    break;
                }
                if e < best.1 {
                    best = (t, e);
                }
            }
            best
        }
    };
    if period > span {
        return None;
    }
    // One repeat: from the needle point nearest the middle of the band in the first repeat, to
    // the one a period on.
    let (lo, hi) = local.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), p| (l.min(p.1), h.max(p.1)));
    let mid = (lo + hi) / 2.0;
    let start = (0..local.len()).filter(|&i| local.get(i).is_some_and(|p| p.0 <= umin + period)).min_by(|&a, &b| {
        let off = |i: usize| local.get(i).map_or(f64::INFINITY, |p| (p.1 - mid).abs());
        off(a).total_cmp(&off(b))
    })?;
    let (u0, v0) = *local.get(start)?;
    let target = (u0 + period, v0);
    let end = (start + 1..local.len()).filter(|&i| local.get(i).is_some_and(|p| (p.0 - target.0).abs() < period / 2.0)).min_by(|&a, &b| {
        let d = |i: usize| local.get(i).map_or(f64::INFINITY, |p| (p.0 - target.0).hypot(p.1 - target.1));
        d(a).total_cmp(&d(b))
    })?;
    let mut points: Vec<[f64; 2]> = local.get(start..end)?.iter().map(|&(u, v)| [u - u0, v - mid]).collect();
    points.push([period, v0 - mid]);
    let repeats = (span / period).floor() as usize;
    Some(Found { points, period, repeats, error })
}

/// The stitched line's segments in cells, to find how far a point is from it.
struct Grid {
    cell: f64,
    cells: std::collections::HashMap<(i64, i64), Vec<usize>>,
    segs: Vec<((f64, f64), (f64, f64))>,
}

impl Grid {
    const CELL: f64 = 1.0;
    /// Farther than this counts as this far.
    const CAP: f64 = 1.5;

    fn new(line: &[(f64, f64)]) -> Self {
        let mut cells: std::collections::HashMap<(i64, i64), Vec<usize>> = Default::default();
        let segs: Vec<_> = line.windows(2).map(|w| (w[0], w[1])).collect();
        let key = |v: f64| (v / Self::CELL).floor() as i64;
        for (i, &((ax, ay), (bx, by))) in segs.iter().enumerate() {
            // A long stitch is filed under every cell along it (capped: a stitch is at most ~12 mm).
            let n = ((bx - ax).hypot(by - ay) / Self::CELL).ceil().clamp(1.0, 64.0) as usize;
            for s in 0..=n {
                let t = s as f64 / n as f64;
                let e = cells.entry((key(ax + (bx - ax) * t), key(ay + (by - ay) * t))).or_default();
                if e.last() != Some(&i) {
                    e.push(i);
                }
            }
        }
        Self { cell: Self::CELL, cells, segs }
    }

    fn distance(&self, p: (f64, f64)) -> f64 {
        let (cx, cy) = ((p.0 / self.cell).floor() as i64, (p.1 / self.cell).floor() as i64);
        let mut best = Self::CAP;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &i in self.cells.get(&(cx.saturating_add(dx), cy.saturating_add(dy))).into_iter().flatten() {
                    let Some(&((ax, ay), (bx, by))) = self.segs.get(i) else { continue };
                    let (ex, ey) = (bx - ax, by - ay);
                    let l2 = ex * ex + ey * ey;
                    let t = if l2 > 0.0 { (((p.0 - ax) * ex + (p.1 - ay) * ey) / l2).clamp(0.0, 1.0) } else { 0.0 };
                    best = best.min((p.0 - ax - ex * t).hypot(p.1 - ay - ey * t));
                }
            }
        }
        best
    }

    /// How far, on average, the points shifted `t` along lie from the line, counting those that
    /// land over it (none: too few do).
    fn mean_distance(&self, pts: &[(f64, f64)], t: f64, umin: f64, umax: f64) -> Option<f64> {
        let (mut sum, mut n) = (0.0, 0usize);
        for &(u, v) in pts {
            if u + t <= umax && u + t >= umin {
                sum += self.distance((u + t, v));
                n += 1;
            }
        }
        (n >= 4 && n * 4 >= pts.len()).then(|| sum / n as f64)
    }
}

/// The pattern from a repeat found.
fn finish(f: Found, name: &str) -> Extracted {
    let half = f.points.iter().map(|p| p[1].abs()).fold(0.0, f64::max);
    let stitches = f.points.len().saturating_sub(1);
    let name = if name.trim().is_empty() { "Extracted border".to_string() } else { name.trim().to_string() };
    let pattern = BorderPattern {
        id: crate::borders::slug(&name),
        name,
        // Width of the band the stitches fill (at least a millimetre, for a nearly flat line).
        width: (2.0 * half).max(1.0),
        length: f.period,
        repeat: vec![PatternPart { stitch: PartStitch::Manual, points: f.points, width: None, ..Default::default() }],
        ..BorderPattern::default()
    };
    Extracted { pattern, period: f.period, stitches, repeats: f.repeats, error: f.error }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A border sewn with a fixed number of needle points per repeat (copied repeats), along
    /// direction `angle` (degrees), rounded to DST's 0.1 mm.
    fn copied(repeats: usize, angle: f64) -> Vec<Point> {
        let motif = [(0.0, 0.0), (1.5, -2.0), (3.0, -2.5), (4.5, -1.0), (5.0, 1.5), (6.5, 2.5), (8.0, 1.0)];
        let (s, c) = angle.to_radians().sin_cos();
        let mut out = vec![];
        for r in 0..repeats {
            for &(x, y) in &motif {
                let (x, y) = (x + r as f64 * 9.0, y);
                let p = Point::new(20.0 + x * c - y * s, 30.0 + x * s + y * c);
                out.push(Point::new((p.x * 10.0).round() / 10.0, (p.y * 10.0).round() / 10.0));
            }
        }
        out
    }

    #[test]
    fn a_copied_repeat_is_found_by_its_stitch_count_and_averaged() {
        for angle in [0.0, 30.0, 180.0, -75.0] {
            let e = border(&copied(6, angle), "Zig", None).unwrap();
            assert_eq!((e.stitches, e.repeats), (7, 5), "{angle}°");
            assert!((e.period - 9.0).abs() < 0.05, "{angle}°: {}", e.period);
            assert!(e.error < 0.1, "{angle}°: {}", e.error);
            let pts = &e.pattern.repeat[0].points;
            let (first, last) = (pts[0], pts[pts.len() - 1]);
            assert!((last[0] - first[0] - e.period).abs() < 1e-9 && (last[1] - first[1]).abs() < 1e-9, "joins end to end");
            assert!((e.pattern.width - 5.0).abs() < 0.2, "{angle}°: the band is 5 mm: {}", e.pattern.width);
            crate::borders::check(&e.pattern).unwrap();
        }
    }

    #[test]
    fn a_repeat_with_differing_stitch_counts_is_found_by_distance() {
        // A starter pattern sewn along a line: its stitches don't fall at the same places in
        // each repeat.
        let p = crate::borders::builtin("waves").unwrap();
        let runs = crate::borders::sew(&p, &[Point::new(0.0, 0.0), Point::new(70.0, 70.0)], false, 6.0, false, 1.7, 0.4).unwrap();
        // Single stitches (no bean back-and-forth), rounded like a DST.
        let pts: Vec<Point> = runs[0].iter().step_by(3).map(|p| Point::new((p.x * 10.0).round() / 10.0, (p.y * 10.0).round() / 10.0)).collect();
        let e = border(&pts, "Waves again", None).unwrap();
        let expected = 70.0 * std::f64::consts::SQRT_2 / (70.0 * std::f64::consts::SQRT_2 / 10.0).round();
        assert!((e.period - expected).abs() < 0.3, "{} vs {expected}", e.period);
        assert!((e.pattern.width - 5.0).abs() < 0.6, "the wave is 5 mm from crest to trough: {}", e.pattern.width);
        assert_eq!(e.pattern.id, "waves-again");
        // Sewn again, the pattern makes a wave of the same size.
        let again =
            crate::borders::sew(&e.pattern, &[Point::new(0.0, 0.0), Point::new(100.0, 0.0)], false, e.pattern.width, false, 1.5, 0.4).unwrap();
        let top = again.iter().flatten().map(|q| q.y.abs()).fold(0.0, f64::max);
        assert!((top - 2.5).abs() < 0.6, "{top}");
    }

    #[test]
    fn a_loose_early_fit_doesnt_cut_the_repeat_short() {
        // Diamonds are long diagonal lines: shifted a little short of their repeat they already lie
        // near themselves. The repeat found must be the true one (8.18 mm here), not the first
        // shift that fits loosely.
        let p = crate::borders::builtin("diamonds").unwrap();
        let runs = crate::borders::sew(&p, &[Point::new(0.0, 0.0), Point::new(147.2, 0.0)], false, 6.0, false, 1.5, 0.4).unwrap();
        let pts: Vec<Point> = runs[0].iter().map(|p| Point::new((p.x * 10.0).round() / 10.0, (p.y * 10.0).round() / 10.0)).collect();
        let e = border(&pts, "d", None).unwrap();
        assert!((e.period - 147.2 / 18.0).abs() < 0.05, "{}", e.period);
    }

    #[test]
    fn a_given_length_is_used() {
        let e = border(&copied(5, 0.0), "x", Some(9.0)).unwrap();
        assert!((e.period - 9.0).abs() < 1e-9 && e.error < 0.2);
    }

    #[test]
    fn what_doesnt_repeat_is_refused() {
        assert!(border(&[Point::ZERO; 3], "x", None).is_err());
        // A spiral never repeats.
        let spiral: Vec<Point> = (0..300)
            .map(|i| {
                let a = i as f64 * 0.2;
                Point::new(a * 3.0 * a.cos(), a * 3.0 * a.sin())
            })
            .collect();
        assert!(border(&spiral, "x", None).is_err());
        assert!(border(&copied(5, 0.0), "x", Some(f64::NAN)).is_err());
        assert!(border(&copied(5, 0.0), "x", Some(1000.0)).is_err(), "longer than the stretch");
    }

    #[test]
    fn a_long_border_is_found_quickly() {
        // 3,000 repeats: 21,000 needle points.
        let t = std::time::Instant::now();
        let e = border(&copied(3000, 20.0), "Long", None).unwrap();
        assert!((e.period - 9.0).abs() < 0.05 && e.stitches == 7, "{} {}", e.period, e.stitches);
        assert!(t.elapsed().as_secs_f64() < 20.0, "{:?}", t.elapsed());
    }

    proptest::proptest! {
        // Selected stitches are untrusted: any points give a pattern or an error, never a panic,
        // and a pattern found is one that can be sewn.
        #[test]
        fn any_points_extract_or_are_refused(
            pts in proptest::collection::vec(proptest::prop_oneof![
                (proptest::num::f64::ANY, proptest::num::f64::ANY),
                (-50.0..50.0, -50.0..50.0),
            ], 0..120),
            period in proptest::option::of(proptest::prop_oneof![proptest::num::f64::ANY, 0.5..40.0]),
        ) {
            let pts: Vec<Point> = pts.into_iter().map(|(x, y)| Point::new(x, y)).collect();
            if let Ok(e) = border(&pts, "p", period) {
                proptest::prop_assert!(crate::borders::check(&e.pattern).is_ok(), "{:?}", e.pattern);
            }
        }
    }

    #[test]
    fn the_stretch_is_the_longest_run_inside_the_region() {
        let runs = vec![(0..50).map(|i| Point::new(i as f64, 0.0)).collect::<Vec<_>>(), (0..10).map(|i| Point::new(i as f64, 5.0)).collect()];
        assert_eq!(stretch(&runs, None).len(), 50);
        let region = vec![vec![Point::new(10.0, -1.0), Point::new(30.0, -1.0), Point::new(30.0, 1.0), Point::new(10.0, 1.0)]];
        let s = stretch(&runs, Some(&region));
        assert_eq!((s.len(), s[0].x), (21, 10.0));
    }
}
