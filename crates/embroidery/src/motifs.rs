//! Motifs for carved fills: line drawings repeated across a fill, where the fill's needle points
//! fall on the lines so the motif shows in the stitching (an embossed pattern). All are drawn here
//! in code, on a unit tile.

use kurbo::Point;

/// The motifs, by id.
pub const MOTIFS: [&str; 5] = ["diamonds", "waves", "circles", "scales", "herringbone"];

/// The most motif segments one fill carves: past it the motif is left out (a huge fill or a tiny
/// motif), with a warning.
pub const MAX_SEGMENTS: usize = 200_000;

/// A motif's lines on the unit tile (0–1 each way, y down), and whether alternate rows of tiles
/// shift half a tile (a brick layout).
fn tile(id: &str) -> Option<(Vec<Vec<Point>>, bool)> {
    let ring = |cx: f64, cy: f64, r: f64, a0: f64, a1: f64, n: usize| -> Vec<Point> {
        (0..=n)
            .map(|k| {
                let a = a0 + (a1 - a0) * k as f64 / n as f64;
                Point::new(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect()
    };
    let p = |x: f64, y: f64| Point::new(x, y);
    let pi = std::f64::consts::PI;
    Some(match id {
        "diamonds" => (vec![vec![p(0.0, 0.5), p(0.5, 0.0), p(1.0, 0.5), p(0.5, 1.0), p(0.0, 0.5)]], false),
        "waves" => (
            vec![
                (0..=24)
                    .map(|k| {
                        let x = k as f64 / 24.0;
                        p(x, 0.5 + 0.25 * (2.0 * pi * x).sin())
                    })
                    .collect(),
            ],
            false,
        ),
        "circles" => (vec![ring(0.5, 0.5, 0.36, 0.0, 2.0 * pi, 32)], false),
        // Fish scales: half circles hanging from the tile's top edge, rows shifted half a tile.
        "scales" => (vec![ring(0.5, 0.0, 0.5, 0.0, pi, 24)], true),
        "herringbone" => (vec![vec![p(0.0, 0.0), p(0.5, 0.5), p(1.0, 0.0)], vec![p(0.0, 0.5), p(0.5, 1.0), p(1.0, 0.5)]], false),
        _ => return None,
    })
}

/// The motif `id` repeated at `size` mm over the box `lo`–`hi` (mm, page space), as segments.
/// None for an unknown motif or one that would take too many segments.
pub fn tiled(id: &str, size: f64, lo: Point, hi: Point) -> Option<Vec<(Point, Point)>> {
    let (lines, brick) = tile(id)?;
    if !(size.is_finite() && size > 0.0) {
        return None;
    }
    let cols = ((hi.x - lo.x) / size).ceil() + 2.0;
    let rows = ((hi.y - lo.y) / size).ceil() + 1.0;
    let per = lines.iter().map(|l| l.len().saturating_sub(1)).sum::<usize>() as f64;
    if !(cols.is_finite() && rows.is_finite()) || cols * rows * per > MAX_SEGMENTS as f64 {
        return None;
    }
    // Tiles anchored to the page grid, so neighbouring fills with one motif line up.
    let (c0, r0) = ((lo.x / size).floor() as i64 - 1, (lo.y / size).floor() as i64);
    let mut out = vec![];
    for r in r0..r0 + rows as i64 {
        let shift = if brick && r.rem_euclid(2) == 1 { 0.5 } else { 0.0 };
        for c in c0..c0 + cols as i64 {
            let o = Point::new((c as f64 + shift) * size, r as f64 * size);
            for l in &lines {
                out.extend(l.windows(2).map(|w| (o + w[0].to_vec2() * size, o + w[1].to_vec2() * size)));
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_motif_tiles_its_box() {
        for id in MOTIFS {
            let segs = tiled(id, 10.0, Point::new(0.0, 0.0), Point::new(30.0, 20.0)).unwrap();
            assert!(segs.len() >= 6, "{id}");
            // The box is covered: segments reach past its sides (to within a tile, top and bottom:
            // a wave runs through the middle of its tile).
            let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
            for (a, b) in &segs {
                for p in [a, b] {
                    x0 = x0.min(p.x);
                    y0 = y0.min(p.y);
                    x1 = x1.max(p.x);
                    y1 = y1.max(p.y);
                }
            }
            assert!(x0 <= 0.0 && y0 <= 10.0 && x1 >= 30.0 && y1 >= 10.0, "{id}: {x0},{y0} {x1},{y1}");
        }
    }

    #[test]
    fn unknown_motifs_and_huge_tilings_are_refused() {
        assert!(tiled("stars", 10.0, Point::ZERO, Point::new(10.0, 10.0)).is_none());
        assert!(tiled("circles", 0.01, Point::ZERO, Point::new(1000.0, 1000.0)).is_none());
        assert!(tiled("circles", f64::NAN, Point::ZERO, Point::new(10.0, 10.0)).is_none());
    }
}
