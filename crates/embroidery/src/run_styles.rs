//! Running stitch styles ([`RunStyle`]): machine versions of hand stitches along a line. Each
//! style repeats a small figure along the line, fitted to whole repeats between sharp corners
//! (the figure starts again at each corner, as a pattern border's does), and keeps every stitch
//! within the longest a machine sews.

use kurbo::{Point, Vec2};
use vectorcraft_doc::{RunStyle, StitchSettings};

use crate::MAX_NEEDLES;
use crate::generate;

/// Sashiko dashes longer than this (mm) are sewn in several stitches, so they follow a curve.
const DASH_STITCH: f64 = 4.0;

/// `line` (mm; a closed one repeats its first point) sewn in `s`'s running stitch style.
pub fn run(line: &[Point], closed: bool, s: &StitchSettings) -> Vec<Point> {
    let len = s.length();
    let style = s.run_style();
    if style == RunStyle::Single {
        return generate::running(line, len);
    }
    let (pieces, ring) = generate::pieces(line, closed);
    // Too many repeats for any machine: nothing, as running stitch does.
    let total: f64 = pieces.iter().map(|p| Walk::new(p).len).sum();
    if !total.is_finite() || total / len * 12.0 > MAX_NEEDLES as f64 {
        return vec![];
    }
    let mut out: Vec<Point> = vec![];
    for piece in &pieces {
        let w = Walk::new(piece);
        if w.len <= 1e-9 {
            continue;
        }
        let part = match style {
            RunStyle::Single => generate::running(piece, len),
            RunStyle::Chain => chain(&w, len, s.run_width()),
            RunStyle::Stem => stem(&w, len, s.run_width(), s.flip),
            // Needle points every half stitch: each stitch forward spans two of them.
            RunStyle::Backstitch => backstitch(&w, len / 2.0),
            RunStyle::Sashiko => sashiko(&w, len, s.gap(), ring),
        };
        // Each piece starts where the last one ended (at the corner between them).
        let skip = usize::from(out.last().zip(part.first()).is_some_and(|(a, b)| a.distance(*b) < 1e-9));
        out.extend(part.into_iter().skip(skip));
    }
    generate::split_long(&out)
}

/// A polyline measured along its length: points at a distance along it, and the direction there.
struct Walk<'a> {
    line: &'a [Point],
    /// The distance along the line to each point.
    at: Vec<f64>,
    len: f64,
}

impl<'a> Walk<'a> {
    fn new(line: &'a [Point]) -> Self {
        let mut at = Vec::with_capacity(line.len());
        let mut d = 0.0;
        for (i, p) in line.iter().enumerate() {
            if let Some(q) = i.checked_sub(1).and_then(|j| line.get(j)) {
                d += q.distance(*p);
            }
            at.push(d);
        }
        Self { line, at, len: d }
    }

    /// The point `d` mm along the line (clamped to it) and the unit normal to the left of the
    /// direction of travel there.
    fn point(&self, d: f64) -> (Point, Vec2) {
        let d = d.clamp(0.0, self.len);
        // The segment holding `d`: the last point at or before it.
        let i = self.at.partition_point(|&a| a <= d).saturating_sub(1).min(self.line.len().saturating_sub(2));
        let (Some(&a), Some(&b), Some(&da)) = (self.line.get(i), self.line.get(i + 1), self.at.get(i)) else {
            return (self.line.first().copied().unwrap_or_default(), Vec2::new(0.0, 1.0));
        };
        let seg = a.distance(b);
        let p = if seg > 0.0 { a.lerp(b, ((d - da) / seg).clamp(0.0, 1.0)) } else { a };
        let t = b - a;
        let n = if seg > 0.0 { Vec2::new(-t.y / seg, t.x / seg) } else { Vec2::new(0.0, 1.0) };
        (p, n)
    }

    /// The point `d` mm along, moved `off` mm to the left of the line (right when negative).
    fn beside(&self, d: f64, off: f64) -> Point {
        let (p, n) = self.point(d);
        p + n * off
    }

    /// The most whole repeats of `unit` mm that fit, at least one.
    fn repeats(&self, unit: f64) -> usize {
        ((self.len / unit).round() as usize).max(1)
    }
}

/// Chain: linked loops, each `len` long and `width` wide, the loop's left side sewn forward, its
/// right side back to the start, then the right side again to its tip, where the next loop starts
/// (so no stitch crosses the inside of a loop).
fn chain(w: &Walk, len: f64, width: f64) -> Vec<Point> {
    let n = w.repeats(len);
    let unit = w.len / n as f64;
    let half = width / 2.0;
    // Where the loop's sides are sewn through, as fractions of its length, and how far out.
    let side: Vec<(f64, f64)> = [0.25, 0.5, 0.75].iter().map(|&f: &f64| (f, half * (f * std::f64::consts::PI).sin())).collect();
    let mut out = vec![w.beside(0.0, 0.0)];
    for k in 0..n {
        let s0 = k as f64 * unit;
        let tip = w.beside(s0 + unit, 0.0);
        out.extend(side.iter().map(|&(f, o)| w.beside(s0 + f * unit, o)));
        out.push(tip);
        out.extend(side.iter().rev().map(|&(f, o)| w.beside(s0 + f * unit, -o)));
        out.push(w.beside(s0, 0.0));
        out.extend(side.iter().map(|&(f, o)| w.beside(s0 + f * unit, -o)));
        out.push(tip);
    }
    out
}

/// Stem: stitches `len` long slanted across a band `width` wide, each starting halfway back along
/// the last, on the left side of the line (the right when `flip`).
fn stem(w: &Walk, len: f64, width: f64, flip: bool) -> Vec<Point> {
    let steps = w.repeats(len / 2.0).max(2);
    let step = w.len / steps as f64;
    let half = if flip { -width / 2.0 } else { width / 2.0 };
    let mut out = vec![];
    for k in 0..steps - 1 {
        let s0 = k as f64 * step;
        out.push(w.beside(s0, -half));
        out.push(w.beside(s0 + 2.0 * step, half));
    }
    out
}

/// Backstitch: needle points every `step` mm; from each, two steps forward (one stitch) and one
/// back, so every stretch of the line is sewn three times (the first by a step back before the
/// first stitch forward).
fn backstitch(w: &Walk, step: f64) -> Vec<Point> {
    let n = w.repeats(step);
    let unit = w.len / n as f64;
    let p = |k: usize| w.beside(k as f64 * unit, 0.0);
    let mut out = vec![p(0), p(1), p(0)];
    for k in 0..n.saturating_sub(1) {
        out.extend([p(k + 2), p(k + 1)]);
    }
    out.push(p(n));
    out
}

/// Sashiko: dashes about `len` long with gaps about `gap` long between them, fitted to whole
/// dashes; each dash sewn forward, back and forward again, each gap crossed by one stitch. A
/// closed ring without corners has a gap where it closes too.
fn sashiko(w: &Walk, len: f64, gap: f64, ring: bool) -> Vec<Point> {
    let period = len + gap;
    let (n, scale) = if ring {
        let n = w.repeats(period);
        (n, w.len / (n as f64 * period))
    } else {
        let n = (((w.len + gap) / period).round() as usize).max(1);
        (n, w.len / (n as f64 * len + (n - 1) as f64 * gap))
    };
    let (dash, gap) = (len * scale, gap * scale);
    let stitches = ((dash / DASH_STITCH).ceil() as usize).max(1);
    let mut out = vec![];
    for k in 0..n {
        let s0 = k as f64 * (dash + gap);
        let run: Vec<Point> = (0..=stitches).map(|j| w.beside(s0 + dash * j as f64 / stitches as f64, 0.0)).collect();
        // Forward, back, forward: the gap stitch before it already ends at its start.
        out.extend(run.iter().copied());
        out.extend(run.iter().rev().skip(1).copied());
        out.extend(run.iter().skip(1).copied());
    }
    if ring {
        out.push(w.beside(w.len, 0.0));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_doc::StitchKind;

    fn settings(style: RunStyle) -> StitchSettings {
        StitchSettings { style, ..StitchSettings::new(StitchKind::Running) }
    }

    fn straight() -> Vec<Point> {
        vec![Point::new(0.0, 0.0), Point::new(30.0, 0.0)]
    }

    /// A curve, then a sharp corner, then a straight.
    fn awkward() -> Vec<Point> {
        let mut l: Vec<Point> = (0..=40)
            .map(|i| {
                let a = std::f64::consts::PI * i as f64 / 40.0;
                Point::new(10.0 - 10.0 * a.cos(), 10.0 * a.sin())
            })
            .collect();
        l.push(Point::new(20.0, -15.0));
        l
    }

    fn ring() -> Vec<Point> {
        (0..=64)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / 64.0;
                Point::new(10.0 * a.cos(), 10.0 * a.sin())
            })
            .collect()
    }

    fn stitches(run: &[Point]) -> impl Iterator<Item = f64> + '_ {
        run.windows(2).map(|w| w[0].distance(w[1]))
    }

    #[test]
    fn every_style_stays_within_the_stitch_limits_and_near_the_line() {
        for style in RunStyle::ALL {
            for width in [0.5, 1.5, 6.0] {
                for len in [0.3, 2.5, 12.0] {
                    for (line, closed) in [(straight(), false), (awkward(), false), (ring(), true)] {
                        let s = StitchSettings { width: Some(width), length: Some(len), ..settings(style) };
                        let run = run(&line, closed, &s);
                        assert!(run.len() >= 2, "{}: {} needle points", style.id(), run.len());
                        assert!(
                            stitches(&run).all(|d| d <= generate::MAX_STITCH + 1e-9),
                            "{} len {len} width {width}: a stitch too long",
                            style.id()
                        );
                        // Every needle point is within half the width of the line.
                        let reach = if style.has_width() { s.run_width() / 2.0 } else { 0.0 } + 1e-6;
                        for p in &run {
                            let d = line
                                .windows(2)
                                .map(|w| kurbo::ParamCurveNearest::nearest(&kurbo::Line::new(w[0], w[1]), *p, 1e-9).distance_sq.sqrt())
                                .fold(f64::INFINITY, f64::min);
                            assert!(d <= reach, "{} len {len} width {width}: {p:?} is {d} mm off the line", style.id());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn single_is_plain_running_stitch() {
        let line = awkward();
        assert_eq!(run(&line, false, &settings(RunStyle::Single)), generate::running(&line, 2.5));
    }

    #[test]
    fn styles_start_and_end_on_the_line() {
        for style in [RunStyle::Chain, RunStyle::Backstitch, RunStyle::Sashiko] {
            let run = run(&straight(), false, &settings(style));
            assert_eq!(run.first(), Some(&Point::new(0.0, 0.0)), "{}", style.id());
            assert_eq!(run.last(), Some(&Point::new(30.0, 0.0)), "{}", style.id());
        }
        // A closed ring comes back to where it started, so the needle can enter it anywhere.
        for style in RunStyle::ALL {
            let r = run(&ring(), true, &settings(style));
            if style != RunStyle::Stem {
                assert!(r.first().unwrap().distance(*r.last().unwrap()) < 1e-6, "{}", style.id());
            }
        }
    }

    #[test]
    fn chain_loops_have_both_sides() {
        let s = settings(RunStyle::Chain);
        let run = run(&straight(), false, &s);
        // 30 mm in 2.5 mm loops: 12 loops of 12 needle points each, after the first.
        assert_eq!(run.len(), 1 + 12 * 12);
        let half = s.run_width() / 2.0;
        assert!(run.iter().any(|p| (p.y - half).abs() < 1e-9) && run.iter().any(|p| (p.y + half).abs() < 1e-9));
        // Each loop's tip is on the line, where the next loop starts.
        for k in 1..=12 {
            assert!(run.iter().any(|p| (p.x - 2.5 * k as f64).abs() < 1e-9 && p.y.abs() < 1e-9), "loop {k}");
        }
    }

    #[test]
    fn stem_slants_to_one_side_and_flips() {
        let left = run(&straight(), false, &settings(RunStyle::Stem));
        let right = run(&straight(), false, &StitchSettings { flip: true, ..settings(RunStyle::Stem) });
        // Odd needle points (each stitch's far end) on the left (+y here), even ones on the right.
        assert!(left.iter().skip(1).step_by(2).all(|p| p.y > 0.0) && left.iter().step_by(2).all(|p| p.y < 0.0));
        assert!(right.iter().skip(1).step_by(2).all(|p| p.y < 0.0) && right.iter().step_by(2).all(|p| p.y > 0.0));
        // Each stitch starts halfway back along the one before: 2.5 mm stitches, 1.25 mm apart.
        let starts: Vec<f64> = left.iter().step_by(2).map(|p| p.x).collect();
        assert!(starts.windows(2).all(|w| (w[1] - w[0] - 1.25).abs() < 1e-9), "{starts:?}");
        assert!(left.windows(2).step_by(2).all(|w| (w[1].x - w[0].x - 2.5).abs() < 1e-9));
    }

    #[test]
    fn backstitch_sews_every_stretch_three_times() {
        let run = run(&straight(), false, &settings(RunStyle::Backstitch));
        // Needle points every 1.25 mm (half a 2.5 mm stitch).
        for k in 0..24 {
            let mid = 1.25 * k as f64 + 0.625;
            let over = run.windows(2).filter(|w| w[0].x.min(w[1].x) < mid && w[0].x.max(w[1].x) > mid).count();
            assert_eq!(over, 3, "stretch {k}");
        }
        // Every stitch forward is one stitch length; every step back half of one.
        assert!(run.windows(2).all(|w| [1.25, 2.5].iter().any(|d| (w[0].distance(w[1]) - d).abs() < 1e-9)));
    }

    #[test]
    fn sashiko_leaves_gaps_crossed_once() {
        let s = settings(RunStyle::Sashiko);
        let run = run(&straight(), false, &s);
        // 30 mm of 2.5 mm dashes and 1.5 mm gaps: 8 dashes and 7 gaps, stretched to fit.
        let scale = 30.0 / (8.0 * 2.5 + 7.0 * 1.5);
        let (dash, gap) = (2.5 * scale, 1.5 * scale);
        let over = |x: f64| run.windows(2).filter(|w| w[0].x.min(w[1].x) < x && w[0].x.max(w[1].x) > x).count();
        for k in 0..8 {
            let s0 = k as f64 * (dash + gap);
            assert_eq!(over(s0 + dash / 2.0), 3, "dash {k}");
            if k < 7 {
                assert_eq!(over(s0 + dash + gap / 2.0), 1, "gap {k}");
            }
        }
    }

    #[test]
    fn styles_restart_at_sharp_corners() {
        // An L: each leg holds whole repeats, so the corner is a needle point on the line.
        let l = vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(10.0, 10.0)];
        for style in [RunStyle::Chain, RunStyle::Backstitch, RunStyle::Sashiko] {
            let run = run(&l, false, &settings(style));
            assert!(run.iter().any(|p| p.distance(Point::new(10.0, 0.0)) < 1e-9), "{}", style.id());
        }
    }

    #[test]
    fn degenerate_lines_sew_nothing_or_little() {
        for style in RunStyle::ALL {
            assert!(run(&[], false, &settings(style)).is_empty());
            let dot = [Point::new(1.0, 1.0), Point::new(1.0, 1.0)];
            assert!(run(&dot, false, &settings(style)).len() <= 2, "{}", style.id());
            let bad = [Point::new(0.0, 0.0), Point::new(f64::INFINITY, 0.0)];
            let _ = run(&bad, false, &settings(style));
        }
    }
}
