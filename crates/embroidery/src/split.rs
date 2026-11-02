//! Split satin: a satin stitch wider than [`StitchSettings::split_at`] is sewn as the fewest pieces
//! no longer than [`StitchSettings::split_length`], so a column can be wider than one stitch spans
//! and its long stitches don't snag. The needle points the splits add move along the stitch from
//! one zig to the next ([`SplitPattern`]) so they don't line up into a seam down the column.
//!
//! It works on finished needle points, so every satin (a column, between rails, a satin shape, a
//! zigzag) is split the same way, underlay included.

use kurbo::Point;
use vectorcraft_doc::{SplitPattern, StitchSettings};

/// How an object's wide stitches are split.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rule {
    /// Stitches longer than this are split, mm.
    pub at: f64,
    /// The longest piece, mm.
    pub length: f64,
    pub pattern: SplitPattern,
}

impl Rule {
    /// The object's rule, or `None` when its stitches aren't split.
    pub fn of(s: &StitchSettings) -> Option<Self> {
        s.splits().then(|| Self { at: s.split_at(), length: s.split_length(), pattern: s.split_pattern })
    }
}

/// `run` split by `rule`, or unchanged with none.
pub fn run(run: Vec<Point>, rule: Option<Rule>) -> Vec<Point> {
    match rule {
        Some(r) => satin(&run, &r),
        None => run,
    }
}

/// The most pieces one stitch is split into (a 30 mm column in 2 mm pieces needs 15).
const MAX_PIECES: usize = 32;

/// `run` with every stitch longer than `rule.at` split. Where the splits fall is measured from the
/// same side of the column on every stitch (a satin's stitches alternate direction), and each
/// stitch's are shifted from the one before's.
pub fn satin(run: &[Point], rule: &Rule) -> Vec<Point> {
    let (at, length) = (rule.at, rule.length.max(0.1));
    let mut out = Vec::with_capacity(run.len());
    // The last split stitch: its ends, whether it was measured from its end, and its shift.
    let mut last: Option<(Point, Point, bool, f64)> = None;
    let mut count = 0u64;
    for (k, &p) in run.iter().enumerate() {
        let Some(&a) = k.checked_sub(1).and_then(|j| run.get(j)) else {
            out.push(p);
            continue;
        };
        let len = a.distance(p);
        if !len.is_finite() || len <= at {
            out.push(p);
            continue;
        }
        // Measured from the far end when this stitch starts on the side the last one ended.
        let flipped = match last {
            Some((s, e, f, _)) => (a.distance(e) < a.distance(s)) != f,
            None => false,
        };
        let shift = shift(rule.pattern, count, last.map(|l| l.3));
        count += 1;
        last = Some((a, p, flipped, shift));
        let n = ((len / length).ceil() as usize).clamp(2, MAX_PIECES);
        let mut ts: Vec<f64> = (0..=n)
            .map(|i| (i as f64 + shift) / n as f64)
            .filter(|t| *t > 1e-6 && *t < 1.0 - 1e-6)
            .map(|t| if flipped { 1.0 - t } else { t })
            .collect();
        ts.sort_by(f64::total_cmp);
        out.extend(ts.into_iter().map(|t| a.lerp(p, t)));
        out.push(p);
    }
    out
}

/// How far along a split the `count`th split stitch's needle points are shifted, as a fraction
/// of one piece: stagger steps by a third, random picks a repeatable amount at least 0.15 from
/// the last (and between 0.2 and 0.8, so no end piece is tiny).
fn shift(pattern: SplitPattern, count: u64, last: Option<f64>) -> f64 {
    match pattern {
        SplitPattern::Stagger => (count % 3) as f64 / 3.0,
        SplitPattern::Random => {
            let s = 0.2 + 0.6 * unit(count);
            match last {
                Some(l) if (s - l).abs() < 0.15 => {
                    if l < 0.5 {
                        l + 0.3
                    } else {
                        l - 0.3
                    }
                }
                _ => s,
            }
        }
    }
}

/// A repeatable number in [0, 1) for `k` (SplitMix64).
fn unit(k: u64) -> f64 {
    let mut z = k.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate;

    fn rule(pattern: SplitPattern) -> Rule {
        Rule { at: StitchSettings::DEFAULT_SPLIT_AT, length: StitchSettings::DEFAULT_SPLIT_LENGTH, pattern }
    }

    fn longest(run: &[Point]) -> f64 {
        run.windows(2).map(|w| w[0].distance(w[1])).fold(0.0, f64::max)
    }

    /// A straight column along x, `width` wide, its needle points split: for each zig (the points
    /// on one stitch across), the split points' distances from the y = -width/2 side.
    fn split_rows(width: f64, pattern: SplitPattern) -> Vec<Vec<f64>> {
        let line = [Point::new(0.0, 0.0), Point::new(40.0, 0.0)];
        let run = generate::satin(&line, false, width, 0.4);
        let split = satin(&run, &rule(pattern));
        // A stitch goes from one side to the other; the points between are its splits.
        let edge = width / 2.0 - 1e-6;
        let mut rows = vec![];
        let mut row = vec![];
        for p in &split {
            if p.y.abs() >= edge {
                if !row.is_empty() {
                    rows.push(std::mem::take(&mut row));
                }
            } else {
                row.push(p.y + width / 2.0);
            }
        }
        // Across, not in sewing order (every other stitch runs the other way).
        for r in &mut rows {
            r.sort_by(f64::total_cmp);
        }
        rows
    }

    #[test]
    fn no_piece_is_longer_than_the_split_length() {
        for width in [1.0, 4.0, 7.5, 8.0, 12.0, 20.0, 30.0] {
            for pattern in SplitPattern::ALL {
                let line = [Point::new(0.0, 0.0), Point::new(30.0, 5.0), Point::new(10.0, 25.0)];
                let run = generate::satin(&line, false, width, 0.4);
                let split = satin(&run, &rule(pattern));
                let most = longest(&split);
                if longest(&run) > StitchSettings::DEFAULT_SPLIT_AT {
                    assert!(most <= StitchSettings::DEFAULT_SPLIT_LENGTH + 1e-9, "{width} mm {pattern:?}: {most}");
                } else {
                    assert_eq!(split, run, "{width} mm: narrow enough not to split");
                }
            }
        }
    }

    #[test]
    fn split_points_are_on_the_stitch() {
        let run = [Point::new(0.0, 0.0), Point::new(3.0, 20.0), Point::new(3.4, 0.0)];
        let split = satin(&run, &rule(SplitPattern::Random));
        for w in split.windows(3) {
            // Every point between two others on the same stitch is on the line through them.
            let (a, b, c) = (w[0], w[1], w[2]);
            let on = |p: Point, q: Point, r: Point| ((q - p).cross(r - p)).abs() < 1e-6;
            if run.iter().all(|r| r.distance(b) > 1e-9) {
                assert!(on(a, b, c), "{b:?} off the stitch");
            }
        }
        assert!(split.len() > run.len());
    }

    #[test]
    fn neighbouring_zigs_never_split_in_line() {
        for width in [8.0, 12.0, 20.0] {
            for pattern in SplitPattern::ALL {
                let rows = split_rows(width, pattern);
                assert!(rows.len() > 50, "{width} {pattern:?}: {} rows", rows.len());
                for pair in rows.windows(2) {
                    for a in &pair[0] {
                        for b in &pair[1] {
                            // At least 0.15 of a piece apart (a piece is at most 4 mm).
                            assert!((a - b).abs() > 0.1, "{width} mm {pattern:?}: {a} and {b} in line");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn stagger_repeats_every_three_zigs() {
        let rows = split_rows(12.0, SplitPattern::Stagger);
        for w in rows.windows(4).skip(2).take(20) {
            assert_eq!(w[0].len(), w[3].len());
            for (a, b) in w[0].iter().zip(&w[3]) {
                assert!((a - b).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn repeatable_and_safe_on_odd_input() {
        let run = [Point::new(0.0, 0.0), Point::new(0.0, 25.0), Point::new(1.0, 0.0), Point::new(1.0, 25.0)];
        let r = rule(SplitPattern::Random);
        assert_eq!(satin(&run, &r), satin(&run, &r));
        assert!(satin(&[], &r).is_empty());
        let nan = [Point::new(0.0, 0.0), Point::new(f64::NAN, 1.0), Point::new(1e300, -1e300)];
        let out = satin(&nan, &r);
        assert!(out.len() <= nan.len() * (MAX_PIECES + 1));
    }

    #[test]
    fn off_or_other_kinds_dont_split() {
        let mut s = StitchSettings::new(vectorcraft_doc::StitchKind::Satin);
        assert!(Rule::of(&s).is_some());
        s.split = vectorcraft_doc::Split::Off;
        assert!(Rule::of(&s).is_none());
        assert!(Rule::of(&StitchSettings::new(vectorcraft_doc::StitchKind::Fill)).is_none());
    }
}
