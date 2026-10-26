//! Motifs ([`Motif`]): one tile of a pattern repeated both ways across an area, on a lattice.
//!
//! A **carve** motif is line art a fill's needle points fall on, so it shows in the stitching (an
//! embossed pattern); the five built in are drawn here in code, on a unit tile. A **stitched**
//! motif is sewn tile by tile as stitches of its own (a motif fill). Both are stored as data, in
//! documents and in `.vcmotif` files (see `docs/formats/vcmotif.md`).

use kurbo::{Point, Vec2};
use vectorcraft_doc::{Motif, MotifKind, PartStitch, PatternPart};

/// The built-in carve motifs, by id.
pub const MOTIFS: [&str; 5] = ["diamonds", "waves", "circles", "scales", "herringbone"];
/// The built-in stitched motifs (for motif fills), by id.
pub const STITCHED: [&str; 5] = ["stars", "flowers", "crosses", "hearts", "dots"];
/// The stitched motif a motif fill sews when it names none.
pub const DEFAULT_STITCHED: &str = "stars";

/// The most motif segments one fill carves: past it the motif is left out (a huge fill or a tiny
/// motif), with a warning.
pub const MAX_SEGMENTS: usize = 200_000;
/// The most points a motif's parts have.
pub const MAX_MOTIF_POINTS: usize = 100_000;
/// A motif's size, and the length of its offsets, mm.
const MOTIF_SIZE: std::ops::RangeInclusive<f64> = 0.01..=1000.0;
/// The file extension of a motif file.
pub const FILE_EXT: &str = "vcmotif";
/// A motif file's `format`.
pub const FILE_FORMAT: &str = "vectorcraft-motif";
/// The newest motif file version this one reads and writes.
pub const FILE_VERSION: u32 = 1;
/// The largest motif file read.
pub const MAX_FILE_BYTES: usize = 8 << 20;

/// A built-in motif's lines on the unit tile (0–1 each way, y down), and whether alternate rows of
/// tiles shift half a tile (a brick layout).
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

/// A built-in stitched motif's tile (one of [`STITCHED`]), drawn in code: its size (mm), lattice
/// and parts, a figure centred on the tile's origin.
fn stitched_tile(id: &str) -> Option<(f64, [[f64; 2]; 2], Vec<PatternPart>)> {
    use std::f64::consts::{PI, TAU};
    let line = |stitch: PartStitch, pts: Vec<[f64; 2]>| PatternPart { stitch, points: pts, ..Default::default() };
    let around = |n: usize, r: &dyn Fn(f64) -> f64| -> Vec<[f64; 2]> {
        (0..=n)
            .map(|k| {
                let a = TAU * k as f64 / n as f64 - PI / 2.0;
                [r(a) * a.cos(), r(a) * a.sin()]
            })
            .collect()
    };
    Some(match id {
        // A five-pointed star outline in bean stitch.
        "stars" => {
            let pts: Vec<[f64; 2]> = (0..=10)
                .map(|k| {
                    let a = PI * k as f64 / 5.0 - PI / 2.0;
                    let r = if k % 2 == 0 { 3.0 } else { 1.2 };
                    [r * a.cos(), r * a.sin()]
                })
                .collect();
            (8.0, [[8.0, 0.0], [4.0, 7.0]], vec![line(PartStitch::Bean, pts)])
        }
        // Five rounded petals round a centre, one running line.
        "flowers" => {
            (8.0, [[8.0, 0.0], [4.0, 7.0]], vec![line(PartStitch::Running, around(80, &|a| 1.0 + 2.0 * ((2.5 * (a + PI / 2.0)).cos()).abs()))])
        }
        "crosses" => (
            7.0,
            [[7.0, 0.0], [3.5, 6.0]],
            vec![line(PartStitch::Bean, vec![[-1.5, -1.5], [1.5, 1.5]]), line(PartStitch::Bean, vec![[1.5, -1.5], [-1.5, 1.5]])],
        ),
        "hearts" => {
            let pts: Vec<[f64; 2]> = (0..=48)
                .map(|k| {
                    let t = TAU * k as f64 / 48.0;
                    let x = 16.0 * t.sin().powi(3);
                    let y = -(13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos());
                    [x * 3.0 / 16.0, (y + 2.0) * 3.0 / 16.0]
                })
                .collect();
            (8.0, [[8.0, 0.0], [4.0, 7.0]], vec![line(PartStitch::Bean, pts)])
        }
        // A short satin bar: a dot.
        "dots" => (
            4.0,
            [[4.0, 0.0], [2.0, 3.5]],
            vec![PatternPart { stitch: PartStitch::Satin, points: vec![[-0.5, 0.0], [0.5, 0.0]], width: Some(1.6), ..Default::default() }],
        ),
        _ => return None,
    })
}

/// A built-in motif by id (one of [`MOTIFS`] or [`STITCHED`]), as data: a carve motif on a unit
/// tile (size 1 mm, scaled by the fill's motif size), a stitched one at its own size.
pub fn builtin(id: &str) -> Option<Motif> {
    if let Some((size, offsets, parts)) = stitched_tile(id) {
        let name = id.get(..1).map(str::to_uppercase).unwrap_or_default() + id.get(1..).unwrap_or_default();
        return Some(Motif {
            id: id.to_string(),
            name,
            author: Some("VectorCraft contributors".into()),
            license: Some("CC0-1.0".into()),
            kind: MotifKind::Stitched,
            size,
            offsets,
            parts,
            ..Motif::default()
        });
    }
    let (lines, brick) = tile(id)?;
    let name = id.get(..1).map(str::to_uppercase).unwrap_or_default() + id.get(1..).unwrap_or_default();
    Some(Motif {
        id: id.to_string(),
        name,
        author: Some("VectorCraft contributors".into()),
        license: Some("CC0-1.0".into()),
        kind: MotifKind::Carve,
        size: 1.0,
        offsets: [[1.0, 0.0], if brick { [0.5, 1.0] } else { [0.0, 1.0] }],
        parts: lines
            .into_iter()
            .map(|l| PatternPart { stitch: PartStitch::Running, points: l.iter().map(|q| [q.x, q.y]).collect(), ..Default::default() })
            .collect(),
        ..Motif::default()
    })
}

/// The motif `id`: the document's own (`motifs`) first, then the built-in ones. A document motif
/// this version can't read (from a newer VectorCraft) is none.
pub fn find(motifs: &[Motif], id: &str) -> Option<Motif> {
    match motifs.iter().find(|m| m.id == id) {
        Some(m) if m.unknown.is_some() => None,
        Some(m) => Some(m.clone()),
        None => builtin(id),
    }
}

/// The motif `id` repeated at `size` mm over the box `lo`–`hi` (mm, page space), as segments.
/// None for an unknown motif or one that would take too many segments.
pub fn tiled(id: &str, size: f64, lo: Point, hi: Point) -> Option<Vec<(Point, Point)>> {
    tiled_motif(&builtin(id)?, size, lo, hi)
}

/// Motif `m` (its lines) repeated at `size` mm (the size its own `size` is drawn at) over the box
/// `lo`–`hi` (mm, page space), as segments: a tile at every lattice point whose tile meets the
/// box, the lattice anchored to the page so neighbouring fills with one motif line up. None when
/// it would take too many segments or isn't a lattice.
pub fn tiled_motif(m: &Motif, size: f64, lo: Point, hi: Point) -> Option<Vec<(Point, Point)>> {
    let tiles = lattice(m, size, lo, hi)?;
    let k = size / m.size;
    let per = m.parts.iter().map(|p| p.points.len().saturating_sub(1)).sum::<usize>();
    if tiles.len().saturating_mul(per) > MAX_SEGMENTS {
        return None;
    }
    let mut out = Vec::with_capacity(tiles.len() * per);
    for o in tiles {
        for part in &m.parts {
            let pts: Vec<Point> = part.points.iter().map(|&[x, y]| o + Vec2::new(x, y) * k).collect();
            out.extend(pts.windows(2).map(|w| (w[0], w[1])));
        }
    }
    Some(out)
}

/// Where motif `m`'s tiles go at `size` mm to cover the box `lo`–`hi`: every lattice point whose
/// tile meets the box. None when `size` isn't a size, the offsets aren't a lattice, or there would
/// be more than [`MAX_SEGMENTS`] tiles.
pub fn lattice(m: &Motif, size: f64, lo: Point, hi: Point) -> Option<Vec<Point>> {
    if !(size.is_finite() && size > 0.0 && m.size.is_finite() && m.size > 0.0) || !(lo.is_finite() && hi.is_finite()) {
        return None;
    }
    let k = size / m.size;
    let [[ax, ay], [bx, by]] = m.offsets;
    let (a, b) = (Vec2::new(ax, ay) * k, Vec2::new(bx, by) * k);
    let det = a.cross(b);
    if !(det.is_finite() && det.abs() > 1e-9 * a.hypot2().max(b.hypot2())) {
        return None;
    }
    // The tile's own extent about its origin.
    let pts = m.parts.iter().flat_map(|p| p.points.iter());
    let (mut e0, mut e1) = (Vec2::new(f64::INFINITY, f64::INFINITY), Vec2::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
    for &[x, y] in pts {
        let q = Vec2::new(x, y) * k;
        e0 = Vec2::new(e0.x.min(q.x), e0.y.min(q.y));
        e1 = Vec2::new(e1.x.max(q.x), e1.y.max(q.y));
    }
    if !(e0.x.is_finite() && e1.x.is_finite()) {
        return Some(vec![]);
    }
    // Origins whose tile can meet the box lie in `lo - e1`..`hi - e0`: the lattice coordinates of
    // that region's corners bound them.
    let (r0, r1) = (lo - e1, hi - e0);
    let to_lattice = |p: Point| ((p.x * b.y - p.y * b.x) / det, (a.x * p.y - a.y * p.x) / det);
    let corners = [Point::new(r0.x, r0.y), Point::new(r1.x, r0.y), Point::new(r0.x, r1.y), Point::new(r1.x, r1.y)].map(to_lattice);
    let (mut u0, mut u1, mut v0, mut v1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for (u, v) in corners {
        (u0, u1, v0, v1) = (u0.min(u), u1.max(u), v0.min(v), v1.max(v));
    }
    let (u0, u1, v0, v1) = (u0.floor() - 1.0, u1.ceil() + 1.0, v0.floor() - 1.0, v1.ceil() + 1.0);
    let count = (u1 - u0 + 1.0) * (v1 - v0 + 1.0);
    if !count.is_finite() || count > MAX_SEGMENTS as f64 {
        return None;
    }
    let mut out = vec![];
    for j in v0 as i64..=v1 as i64 {
        for i in u0 as i64..=u1 as i64 {
            let o = (a * i as f64 + b * j as f64).to_point();
            let (t0, t1) = (o + e0, o + e1);
            if t1.x >= lo.x && t0.x <= hi.x && t1.y >= lo.y && t0.y <= hi.y {
                out.push(o);
            }
        }
    }
    Some(out)
}

/// One tile of stitched motif `m` sewn: its parts in order (as a border pattern's), scaled by `k`
/// and put on the fabric by `place` (from the tile's space, scaled, to the page), running and bean
/// lines in stitches `len` long, satin with zigs `spacing` apart.
pub fn tile_run(m: &Motif, k: f64, place: &dyn Fn(Point) -> Point, len: f64, spacing: f64) -> Vec<Point> {
    let mut pieces = vec![];
    let map = |x: f64, y: f64| place(Point::new(x * k, y * k));
    for part in &m.parts {
        crate::borders::add_part(&mut pieces, part, (k, k), &map);
    }
    crate::borders::stitch(pieces, len, spacing)
}

/// A motif file as stored: the format header, then the motif's fields.
#[derive(serde::Serialize, serde::Deserialize)]
struct File {
    format: String,
    version: u32,
    #[serde(default = "mm")]
    units: String,
    #[serde(flatten)]
    motif: Motif,
}

fn mm() -> String {
    "mm".into()
}

/// Read a `.vcmotif` file: JSON with `format` [`FILE_FORMAT`], `version` and the motif
/// ([`Motif`]), checked ([`check`]). Its id is its own, else made from its name.
pub fn read_file(bytes: &[u8]) -> Result<Motif, String> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(format!("a motif file is at most {} MB", MAX_FILE_BYTES >> 20));
    }
    let f: File = serde_json::from_slice(bytes).map_err(|e| format!("not a motif file: {e}"))?;
    if f.format != FILE_FORMAT {
        return Err(format!("not a motif file (its format is `{}`)", f.format));
    }
    if f.version > FILE_VERSION {
        return Err(format!("this motif file is version {}: this VectorCraft reads up to version {FILE_VERSION}", f.version));
    }
    if f.units != "mm" {
        return Err(format!("motif files measure in mm, not `{}`", f.units));
    }
    let mut m = f.motif;
    m.id = crate::borders::slug(if m.id.trim().is_empty() { &m.name } else { &m.id });
    if m.name.trim().is_empty() {
        m.name.clone_from(&m.id);
    }
    check(&m)?;
    Ok(m)
}

/// `m` as a `.vcmotif` file.
pub fn write_file(m: &Motif) -> Result<String, String> {
    let f = File { format: FILE_FORMAT.into(), version: FILE_VERSION, units: mm(), motif: m.clone() };
    serde_json::to_string_pretty(&f).map_err(|e| e.to_string())
}

/// Whether motif `m` can be used: a size, two offsets that make a lattice (neither zero, not
/// along one line), lines in its tile, at most [`MAX_MOTIF_POINTS`] points, all numbers.
pub fn check(m: &Motif) -> Result<(), String> {
    let (lo, hi) = (MOTIF_SIZE.start(), MOTIF_SIZE.end());
    if !MOTIF_SIZE.contains(&m.size) {
        return Err(format!("the motif `{}` has a size that isn't one ({lo}–{hi} mm)", m.name));
    }
    let [[ax, ay], [bx, by]] = m.offsets;
    let (a, b) = (Vec2::new(ax, ay), Vec2::new(bx, by));
    let len_ok = |v: Vec2| v.x.is_finite() && v.y.is_finite() && MOTIF_SIZE.contains(&v.hypot());
    if !len_ok(a) || !len_ok(b) || a.cross(b).abs() < 1e-6 * a.hypot() * b.hypot() {
        return Err(format!("the motif `{}` needs two offsets ({lo}–{hi} mm long) that aren't along one line", m.name));
    }
    if m.parts.iter().all(|p| p.points.len() < 2) {
        return Err(format!("the motif `{}` has nothing in its tile", m.name));
    }
    if m.parts.iter().map(|p| p.points.len()).sum::<usize>() > MAX_MOTIF_POINTS {
        return Err(format!("the motif `{}` has more than {MAX_MOTIF_POINTS} points", m.name));
    }
    let finite = |q: &PatternPart| q.points.iter().all(|[x, y]| x.is_finite() && y.is_finite() && x.abs() <= 1e6 && y.abs() <= 1e6);
    if !m.parts.iter().all(finite) || m.parts.iter().any(|q| q.width.is_some_and(|w| !MOTIF_SIZE.contains(&w))) {
        return Err(format!("the motif `{}` has points or widths that aren't numbers", m.name));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_motif_tiles_its_box() {
        for id in MOTIFS {
            let segs = tiled(id, 10.0, Point::new(0.0, 0.0), Point::new(30.0, 20.0)).unwrap();
            assert!(segs.len() >= 6, "{id}");
            // The box is covered: segments reach to within a tile of each side (a circle sits
            // inside its tile; a wave runs through its middle).
            let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
            for (a, b) in &segs {
                for p in [a, b] {
                    x0 = x0.min(p.x);
                    y0 = y0.min(p.y);
                    x1 = x1.max(p.x);
                    y1 = y1.max(p.y);
                }
            }
            assert!(x0 <= 10.0 && y0 <= 10.0 && x1 >= 20.0 && y1 >= 10.0, "{id}: {x0},{y0} {x1},{y1}");
        }
    }

    #[test]
    fn unknown_motifs_and_huge_tilings_are_refused() {
        assert!(tiled("no-such-motif", 10.0, Point::ZERO, Point::new(10.0, 10.0)).is_none());
        assert!(tiled("circles", 0.01, Point::ZERO, Point::new(1000.0, 1000.0)).is_none());
        assert!(tiled("circles", f64::NAN, Point::ZERO, Point::new(10.0, 10.0)).is_none());
    }

    /// The tiles before motifs became data: a square grid, odd rows half a tile over for brick
    /// motifs, anchored to the page.
    fn old_tiles(brick: bool, size: f64, lo: Point, hi: Point) -> Vec<Point> {
        let cols = ((hi.x - lo.x) / size).ceil() + 2.0;
        let rows = ((hi.y - lo.y) / size).ceil() + 1.0;
        let (c0, r0) = ((lo.x / size).floor() as i64 - 1, (lo.y / size).floor() as i64);
        let mut out = vec![];
        for r in r0..r0 + rows as i64 {
            let shift = if brick && r.rem_euclid(2) == 1 { 0.5 } else { 0.0 };
            for c in c0..c0 + cols as i64 {
                out.push(Point::new((c as f64 + shift) * size, r as f64 * size));
            }
        }
        out
    }

    #[test]
    fn the_built_in_motifs_tile_where_they_did_before() {
        // Every tile the old grid put where its tile meets the box, the lattice puts too.
        for id in MOTIFS {
            let m = builtin(id).unwrap();
            assert_eq!(check(&m), Ok(()));
            let (lo, hi) = (Point::new(3.0, -7.0), Point::new(41.0, 29.0));
            let new = lattice(&m, 6.0, lo, hi).unwrap();
            let brick = m.offsets[1][0] != 0.0;
            let meets = |o: &Point| {
                let pts = m.parts.iter().flat_map(|p| p.points.iter()).map(|[x, y]| *o + Vec2::new(*x, *y) * 6.0);
                let (mut a, mut b) = (Point::new(f64::INFINITY, f64::INFINITY), Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
                for q in pts {
                    a = Point::new(a.x.min(q.x), a.y.min(q.y));
                    b = Point::new(b.x.max(q.x), b.y.max(q.y));
                }
                b.x >= lo.x && a.x <= hi.x && b.y >= lo.y && a.y <= hi.y
            };
            for o in old_tiles(brick, 6.0, lo, hi).into_iter().filter(meets) {
                assert!(new.iter().any(|n| n.distance(o) < 1e-9), "{id}: {o:?}");
            }
            // ...and no tile that doesn't meet the box.
            assert!(new.iter().all(meets), "{id}");
        }
    }

    #[test]
    fn the_stitched_starters_are_motifs_and_sew() {
        for id in STITCHED {
            let m = builtin(id).unwrap();
            assert_eq!((check(&m), m.kind), (Ok(()), MotifKind::Stitched), "{id}");
            assert!(!MOTIFS.contains(&id), "{id} isn't a carve id too");
            let run = tile_run(&m, 1.0, &|p| p, 1.5, 0.4);
            assert!(run.len() >= 4, "{id}: {}", run.len());
            // The figure stays inside its own lattice cell (neighbours don't touch).
            let reach = m.offsets.iter().map(|[x, y]| x.hypot(*y)).fold(f64::INFINITY, f64::min) / 2.0;
            assert!(run.iter().all(|p| p.to_vec2().hypot() <= reach), "{id}");
        }
    }

    #[test]
    fn a_skewed_lattice_covers_its_box() {
        // A hexagonal lattice of dots.
        let m = Motif {
            name: "dots".into(),
            size: 4.0,
            offsets: [[4.0, 0.0], [2.0, 3.464]],
            parts: vec![PatternPart { points: vec![[0.0, 0.0], [0.5, 0.0]], ..Default::default() }],
            ..Motif::default()
        };
        let tiles = lattice(&m, 8.0, Point::ZERO, Point::new(40.0, 40.0)).unwrap();
        // Every point of the box is within a tile's reach of an origin (a cell's diameter).
        for x in 0..=8 {
            for y in 0..=8 {
                let p = Point::new(x as f64 * 5.0, y as f64 * 5.0);
                assert!(tiles.iter().any(|o| o.distance(p) <= 9.3), "{p:?}");
            }
        }
    }

    #[test]
    fn motif_files_round_trip_and_bad_ones_are_refused() {
        let m = builtin("scales").unwrap();
        let back = read_file(write_file(&m).unwrap().as_bytes()).unwrap();
        assert_eq!(back, m);
        let bad = |f: &dyn Fn(&mut Motif)| {
            let mut b = m.clone();
            f(&mut b);
            read_file(write_file(&b).unwrap().as_bytes()).is_err()
        };
        assert!(bad(&|b| b.size = 0.0));
        assert!(bad(&|b| b.offsets = [[1.0, 0.0], [2.0, 0.0]]), "offsets along one line");
        assert!(bad(&|b| b.offsets = [[0.0, 0.0], [0.0, 1.0]]), "a zero offset");
        assert!(bad(&|b| b.parts.clear()));
        assert!(bad(&|b| b.parts[0].points[0] = [f64::NAN, 0.0]));
        assert!(read_file(br#"{"format":"vectorcraft-border-pattern","version":1}"#).is_err());
        assert!(read_file(br#"{"format":"vectorcraft-motif","version":99,"name":"x","size":1,"offsets":[[1,0],[0,1]],"parts":[]}"#).is_err());
    }

    proptest::proptest! {
        #[test]
        fn mutated_motif_files_never_panic(bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..400), cut in 0usize..2000) {
            let good = write_file(&builtin("waves").unwrap()).unwrap();
            let _ = read_file(&bytes);
            let mut b = good.into_bytes();
            b.truncate(cut.min(b.len()));
            let _ = read_file(&b);
        }
    }
}
