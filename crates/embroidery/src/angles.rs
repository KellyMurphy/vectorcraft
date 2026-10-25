//! Stitch angle lines ([`StitchSettings::angles`]): lines drawn across a satin set the stitches'
//! direction where they cross it, and between two lines the direction turns gradually from one
//! to the other.
//!
//! A line sets the angle **relative to the satin's centre line** where it crosses it (so a line
//! at 30° to a curving column keeps the stitches at 30° to it as it curves). Each zig's angle is
//! that of the lines before and after it, mixed by distance along the centre line; before the
//! first line and after the last, the nearest line's holds. A stroke no line crosses keeps its
//! own direction. Angles are held to [`MAX_TURN`] off square, so a stitch never runs nearly along
//! the column.

use kurbo::{Point, Vec2};
use vectorcraft_doc::StitchSettings;

/// A stitch angle line, mm.
pub type Line = [Point; 2];

/// The most a stitch turns from square across the satin, radians (60°).
pub const MAX_TURN: f64 = std::f64::consts::FRAC_PI_3;

/// An object's angle lines, the unusable ones (not finite, a point long) left out.
pub fn lines(s: &StitchSettings) -> Vec<Line> {
    s.angles
        .iter()
        .take(StitchSettings::MAX_ANGLES)
        .filter(|l| l.iter().all(|v| v.is_finite()))
        .map(|&[x1, y1, x2, y2]| [Point::new(x1, y1), Point::new(x2, y2)])
        .filter(|[a, b]| a.distance(*b) > 1e-6)
        .collect()
}

/// The turns a centre line's angle lines set: where each crosses it (distance along it) and the
/// turn there.
#[derive(Clone, Debug)]
pub struct Turns {
    set: Vec<(f64, f64)>,
}

impl Turns {
    /// The lines that cross centre line `line`, measured along it. None when none does.
    pub fn new(line: &[Point], lines: &[Line]) -> Option<Self> {
        if lines.is_empty() || line.len() < 2 {
            return None;
        }
        let mut set: Vec<(f64, f64)> = vec![];
        for &[a, b] in lines {
            let d = b - a;
            let mut s0 = 0.0;
            for w in line.windows(2) {
                let (p, e) = (w[0], w[1] - w[0]);
                let len = e.hypot();
                let den = d.cross(e);
                if len > 1e-12 && den.abs() > 1e-12 {
                    let t = (p - a).cross(e) / den;
                    let u = (p - a).cross(d) / den;
                    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                        let n = Vec2::new(-e.y / len, e.x / len);
                        let dir = d / d.hypot();
                        // Either way along the line is the same stitch direction: the one nearer
                        // the normal.
                        let dir = if dir.dot(n) < 0.0 { -dir } else { dir };
                        set.push((s0 + u * len, n.cross(dir).atan2(n.dot(dir)).clamp(-MAX_TURN, MAX_TURN)));
                    }
                }
                s0 += len;
            }
        }
        set.sort_by(|x, y| x.0.total_cmp(&y.0));
        (!set.is_empty()).then_some(Self { set })
    }

    /// The turn `s` mm along the centre line: the lines either side mixed by distance, the
    /// nearest one's before the first and after the last.
    pub fn at(&self, s: f64) -> f64 {
        let (Some(&first), Some(&last)) = (self.set.first(), self.set.last()) else { return 0.0 };
        if s <= first.0 {
            return first.1;
        }
        if s >= last.0 {
            return last.1;
        }
        let i = self.set.partition_point(|x| x.0 <= s);
        match (self.set.get(i.wrapping_sub(1)), self.set.get(i)) {
            (Some(&(s0, t0)), Some(&(s1, t1))) if s1 > s0 => t0 + (t1 - t0) * (s - s0) / (s1 - s0),
            (Some(&(_, t0)), _) => t0,
            _ => first.1,
        }
    }
}

/// For each point of centre line `c` (with its unit normals `base`), how far its stitch turns
/// from the normal (radians, the way [`rotate`] turns): set by the lines that cross `c`, mixed
/// between them by distance along it. None when no line crosses it.
pub fn turns(c: &[Point], base: &[Vec2], lines: &[Line]) -> Option<Vec<f64>> {
    if base.len() != c.len() {
        return None;
    }
    let t = Turns::new(c, lines)?;
    let mut s = 0.0;
    let mut out = Vec::with_capacity(c.len());
    for (k, p) in c.iter().enumerate() {
        if let Some(q) = k.checked_sub(1).and_then(|k| c.get(k)) {
            s += q.distance(*p);
        }
        out.push(t.at(s));
    }
    Some(out)
}

/// `n` turned by `t` radians.
pub fn rotate(n: Vec2, t: f64) -> Vec2 {
    let (s, c) = t.sin_cos();
    Vec2::new(n.x * c - n.y * s, n.x * s + n.y * c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn straight() -> (Vec<Point>, Vec<Vec2>) {
        let c: Vec<Point> = (0..=100).map(|i| Point::new(i as f64, 0.0)).collect();
        let base = crate::generate::normals(&c, false);
        (c, base)
    }

    #[test]
    fn one_line_sets_the_angle_everywhere() {
        let (c, base) = straight();
        // A line at 30° from square, crossing at x = 50.
        let t = 30f64.to_radians();
        let l = [Point::new(50.0 - 5.0 * t.sin(), -5.0 * t.cos()), Point::new(50.0 + 5.0 * t.sin(), 5.0 * t.cos())];
        let turns = turns(&c, &base, &[l]).unwrap();
        for v in &turns {
            assert!((v.abs() - t).abs() < 1e-9, "{v}");
        }
        // The stitch direction is the line's (either way round).
        let d = rotate(base[10], turns[10]);
        let dir = (l[1] - l[0]) / (l[1] - l[0]).hypot();
        assert!(d.cross(dir).abs() < 1e-9, "{d:?} {dir:?}");
    }

    #[test]
    fn between_two_lines_the_angle_turns_gradually() {
        let (c, base) = straight();
        let square = [Point::new(20.0, -5.0), Point::new(20.0, 5.0)];
        let slant = [Point::new(75.0, -5.0), Point::new(85.0, 5.0)];
        let turns = turns(&c, &base, &[slant, square]).unwrap();
        let at = |x: usize| turns[x].abs().to_degrees();
        assert!(at(0) < 1e-6 && at(20) < 1e-6, "square before and at the first line");
        assert!((at(80) - 45.0).abs() < 1e-6 && (at(100) - 45.0).abs() < 1e-6, "45° at and after the second");
        // Evenly in between, never jumping.
        assert!((at(50) - 45.0 * 30.0 / 60.0).abs() < 1e-6, "{}", at(50));
        for w in turns.windows(2) {
            assert!((w[1] - w[0]).abs() < 1.0f64.to_radians(), "a gradual turn");
        }
    }

    #[test]
    fn steep_lines_are_held_and_bad_ones_left_out() {
        let (c, base) = straight();
        // Nearly along the column: held to 60° off square.
        let turns = turns(&c, &base, &[[Point::new(40.0, -0.5), Point::new(60.0, 0.5)]]).unwrap();
        assert!((turns[0].abs() - MAX_TURN).abs() < 1e-9);
        assert!(super::turns(&c, &base, &[[Point::new(0.0, 10.0), Point::new(100.0, 10.0)]]).is_none(), "a line not crossing it");
        let s = StitchSettings { angles: vec![[f64::NAN, 0.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 1.0, 1.0]], ..Default::default() };
        assert_eq!(lines(&s).len(), 1);
    }
}
