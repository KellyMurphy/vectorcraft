//! Border patterns: a stitched motif repeated end to end along a path (the Pattern Border
//! stitch), like a pattern brush. The motif bends with the path's curves; its length is adjusted
//! slightly so a whole number of repeats fits each stretch between sharp corners; at a corner the
//! pattern's corner motif goes in, or, when it has none, the border ends and starts again.
//!
//! The starter patterns are drawn here in code ([`builtin`]); others come from `.vcborder` files
//! ([`read_file`], [`write_file`]; specified in `docs/formats/vcborder.md`) and are kept
//! in the document that uses them.

use kurbo::{Point, Vec2};
use vectorcraft_doc::{BorderPattern, PartStitch, PatternPart};

use crate::MAX_NEEDLES;
use crate::generate::{self, MAX_STITCH};

/// The starter patterns, by id.
pub const BUILTIN: [&str; 4] = ["waves", "scallops", "chain", "diamonds"];

/// A turn sharper than this is a corner the border restarts at (cos of 50°): gentler turns are
/// curves the pattern bends round.
const CORNER_COS: f64 = 0.64;
/// Pattern lines are cut into pieces at most this long (mm, as sewn) before they are bent along
/// the path, so they follow its curves.
const BEND_STEP: f64 = 0.25;
/// The most points a pattern may have (its parts and corner together).
pub const MAX_PATTERN_POINTS: usize = 100_000;
/// The sizes a pattern's own width, repeat length and satin widths may have, mm.
const PATTERN_SIZE: std::ops::RangeInclusive<f64> = 0.01..=1000.0;
/// Bean parts sew each stitch this many times.
const BEAN_PASSES: u8 = 3;

/// The border pattern file's extension.
pub const FILE_EXT: &str = "vcborder";
/// The `format` a border pattern file declares.
pub const FILE_FORMAT: &str = "vectorcraft-border-pattern";
/// The file format version written; files of a later version aren't read.
pub const FILE_VERSION: u32 = 1;
/// The largest border pattern file read, bytes.
pub const MAX_FILE_BYTES: usize = 8 << 20;

/// The pattern `id`: the document's own (`patterns`) first, then the starters. A document pattern
/// this version can't read (from a newer VectorCraft) is none.
pub fn find(patterns: &[BorderPattern], id: &str) -> Option<BorderPattern> {
    match patterns.iter().find(|p| p.id == id) {
        Some(p) if p.unknown.is_some() => None,
        Some(p) => Some(p.clone()),
        None => builtin(id),
    }
}

/// `name` as a pattern id: lower case letters and digits, other runs of characters as one dash
/// (`"Rope Twist 2"` → `rope-twist-2`); `pattern` when nothing is left.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
        if out.len() >= 64 {
            break;
        }
    }
    let out = out.trim_end_matches('-');
    if out.is_empty() { "pattern".into() } else { out.to_string() }
}

/// A border pattern file as stored: the format header, then the pattern's fields.
#[derive(serde::Serialize, serde::Deserialize)]
struct File {
    format: String,
    version: u32,
    #[serde(default = "mm")]
    units: String,
    #[serde(flatten)]
    pattern: BorderPattern,
}

fn mm() -> String {
    "mm".into()
}

/// Read a `.vcborder` file: JSON with `format` [`FILE_FORMAT`], `version` and the pattern
/// ([`BorderPattern`]), checked ([`check`]). Its id is its own, else made from its name.
pub fn read_file(bytes: &[u8]) -> Result<BorderPattern, String> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(format!("a border pattern file is at most {} MB", MAX_FILE_BYTES >> 20));
    }
    let f: File = serde_json::from_slice(bytes).map_err(|e| format!("not a border pattern file: {e}"))?;
    if f.format != FILE_FORMAT {
        return Err(format!("not a border pattern file (its format is `{}`)", f.format));
    }
    if f.version > FILE_VERSION {
        return Err(format!("this border pattern file is version {}: this VectorCraft reads up to version {FILE_VERSION}", f.version));
    }
    if f.units != "mm" {
        return Err(format!("border pattern files measure in mm, not `{}`", f.units));
    }
    let mut p = f.pattern;
    p.id = slug(if p.id.trim().is_empty() { &p.name } else { &p.id });
    if p.name.trim().is_empty() {
        p.name.clone_from(&p.id);
    }
    check(&p)?;
    Ok(p)
}

/// `p` as a `.vcborder` file.
pub fn write_file(p: &BorderPattern) -> Result<String, String> {
    let f = File { format: FILE_FORMAT.into(), version: FILE_VERSION, units: mm(), pattern: p.clone() };
    serde_json::to_string_pretty(&f).map_err(|e| e.to_string())
}

/// A starter pattern by id (one of [`BUILTIN`]).
pub fn builtin(id: &str) -> Option<BorderPattern> {
    use std::f64::consts::PI;
    let curve = |n: usize, f: &dyn Fn(f64) -> [f64; 2]| -> Vec<[f64; 2]> { (0..=n).map(|k| f(k as f64 / n as f64)).collect() };
    let part = |stitch: PartStitch, points: Vec<[f64; 2]>| PatternPart { stitch, points, width: None, ..Default::default() };
    Some(match id {
        // A sine wave in bean stitch.
        "waves" => BorderPattern {
            id: "waves".into(),
            name: "Waves".into(),
            author: Some("VectorCraft contributors".into()),
            license: Some("CC0-1.0".into()),
            source: None,
            description: None,
            width: 6.0,
            length: 10.0,
            repeat: vec![part(PartStitch::Bean, curve(32, &|t| [10.0 * t, 2.5 * (2.0 * PI * t).sin()]))],
            corner: vec![],
            ..Default::default()
        },
        // Satin arches standing on one edge of the band.
        "scallops" => BorderPattern {
            id: "scallops".into(),
            name: "Scallops".into(),
            author: Some("VectorCraft contributors".into()),
            license: Some("CC0-1.0".into()),
            source: None,
            description: None,
            width: 6.0,
            length: 6.0,
            repeat: vec![PatternPart {
                stitch: PartStitch::Satin,
                points: curve(24, &|t| [3.0 - 3.0 * (PI * t).cos(), 2.4 - 4.8 * (PI * t).sin()]),
                width: Some(1.2),
                ..Default::default()
            }],
            corner: vec![],
            ..Default::default()
        },
        // Loops linked like a chain (a prolate cycloid), in bean stitch.
        "chain" => BorderPattern {
            id: "chain".into(),
            name: "Chain".into(),
            author: Some("VectorCraft contributors".into()),
            license: Some("CC0-1.0".into()),
            source: None,
            description: None,
            width: 6.0,
            length: 6.0,
            repeat: vec![part(PartStitch::Bean, curve(48, &|t| [6.0 * t - 1.6 * (2.0 * PI * t).sin(), -2.6 * (2.0 * PI * t).cos()]))],
            corner: vec![],
            ..Default::default()
        },
        // Diamonds strung on a centre line, with a four-petal rosette at each corner.
        "diamonds" => BorderPattern {
            id: "diamonds".into(),
            name: "Diamonds".into(),
            author: Some("VectorCraft contributors".into()),
            license: Some("CC0-1.0".into()),
            source: None,
            description: None,
            width: 6.0,
            length: 8.0,
            repeat: vec![part(PartStitch::Bean, vec![[0.0, 0.0], [4.0, 2.6], [8.0, 0.0], [4.0, -2.6], [0.0, 0.0], [8.0, 0.0]])],
            // A rose curve r = R cos 2θ: four petals, from the centre and back to it.
            corner: vec![part(
                PartStitch::Bean,
                curve(96, &|t| {
                    let a = -PI / 4.0 + 2.0 * PI * t;
                    let r = 4.0 * (2.0 * a).cos();
                    [r * a.cos(), r * a.sin()]
                }),
            )],
            ..Default::default()
        },
        _ => return None,
    })
}

/// Why `p` can't be sewn, if it can't: sizes out of range, no repeat, non-finite points, too
/// many points.
pub fn check(p: &BorderPattern) -> Result<(), String> {
    let size = |v: f64| PATTERN_SIZE.contains(&v);
    if !size(p.width) || !size(p.length) {
        let (lo, hi) = (PATTERN_SIZE.start(), PATTERN_SIZE.end());
        return Err(format!("the border pattern `{}` has a width or repeat length that isn't a size ({lo}–{hi} mm)", p.name));
    }
    if p.repeat.iter().all(|q| q.points.is_empty()) {
        return Err(format!("the border pattern `{}` has nothing in its repeat", p.name));
    }
    let parts = || p.repeat.iter().chain(&p.corner);
    if parts().map(|q| q.points.len()).sum::<usize>() > MAX_PATTERN_POINTS {
        return Err(format!("the border pattern `{}` has more than {MAX_PATTERN_POINTS} points", p.name));
    }
    let finite = |q: &PatternPart| q.points.iter().all(|[x, y]| x.is_finite() && y.is_finite() && x.abs() <= 1e6 && y.abs() <= 1e6);
    if !parts().all(finite) || parts().any(|q| q.width.is_some_and(|w| !size(w))) {
        return Err(format!("the border pattern `{}` has points or widths that aren't numbers", p.name));
    }
    Ok(())
}

/// A stretch of the path between corners, measured along its length.
struct Stretch {
    pts: Vec<Point>,
    /// Distance along it to each point, mm.
    at: Vec<f64>,
    normals: Vec<Vec2>,
}

impl Stretch {
    fn new(pts: Vec<Point>, closed: bool) -> Self {
        let mut at = Vec::with_capacity(pts.len());
        let mut d = 0.0;
        for (i, p) in pts.iter().enumerate() {
            if let Some(q) = i.checked_sub(1).and_then(|j| pts.get(j)) {
                d += q.distance(*p);
            }
            at.push(d);
        }
        let normals = generate::normals(&pts, closed);
        Self { pts, at, normals }
    }

    fn len(&self) -> f64 {
        self.at.last().copied().unwrap_or(0.0)
    }

    /// The point `s` mm along and the normal there (to the right of travel on the page), blended between the points' own
    /// normals so offsets bend smoothly.
    fn frame(&self, s: f64) -> (Point, Vec2) {
        let i = self.at.partition_point(|&a| a <= s).clamp(1, self.at.len().saturating_sub(1).max(1));
        let (Some(&a0), Some(&a1), Some(&p0), Some(&p1)) = (self.at.get(i - 1), self.at.get(i), self.pts.get(i - 1), self.pts.get(i)) else {
            return (self.pts.first().copied().unwrap_or(Point::ZERO), Vec2::new(0.0, 1.0));
        };
        let t = if a1 > a0 { ((s - a0) / (a1 - a0)).clamp(0.0, 1.0) } else { 0.0 };
        let (n0, n1) = (self.normals.get(i - 1).copied().unwrap_or_default(), self.normals.get(i).copied().unwrap_or_default());
        let n = n0.lerp(n1, t);
        let l = n.hypot();
        (p0.lerp(p1, t), if l > 1e-9 { n / l } else { n0 })
    }

    /// The direction of travel at the start (`end` false) or end of the stretch.
    fn direction(&self, end: bool) -> Vec2 {
        let n = self.pts.len();
        let (a, b) = if end { (n.saturating_sub(2), n.saturating_sub(1)) } else { (0, 1) };
        match (self.pts.get(a), self.pts.get(b)) {
            (Some(&p), Some(&q)) => {
                let v = q - p;
                let l = v.hypot();
                if l > 0.0 { v / l } else { Vec2::new(1.0, 0.0) }
            }
            _ => Vec2::new(1.0, 0.0),
        }
    }
}

fn is_corner(a: Point, b: Point, c: Point) -> bool {
    let (u, v) = (b - a, c - b);
    let (lu, lv) = (u.hypot(), v.hypot());
    lu > 0.0 && lv > 0.0 && u.dot(v) / (lu * lv) < CORNER_COS
}

/// `line` cut at its sharp corners into stretches, and whether the line is a closed ring without
/// corners. A closed line with corners starts and ends at one of them, so every stretch runs
/// from corner to corner.
fn stretches(line: &[Point], closed: bool) -> (Vec<Vec<Point>>, bool) {
    let mut line: Vec<Point> = line.to_vec();
    line.dedup_by(|a, b| a.distance(*b) < 1e-9);
    let closed = closed && line.len() > 3 && line.first().zip(line.last()).is_some_and(|(a, b)| a.distance(*b) < 1e-6);
    if closed {
        line.pop();
        let m = line.len();
        let corner = |k: usize| match (line.get((k + m - 1) % m), line.get(k), line.get((k + 1) % m)) {
            (Some(&a), Some(&b), Some(&c)) => is_corner(a, b, c),
            _ => false,
        };
        let Some(c0) = (0..m).find(|&k| corner(k)) else {
            if let Some(&f) = line.first() {
                line.push(f);
            }
            return (vec![line], true);
        };
        let mut ring: Vec<Point> = (0..=m).filter_map(|k| line.get((c0 + k) % m).copied()).collect();
        ring.dedup_by(|a, b| a.distance(*b) < 1e-9);
        line = ring;
    }
    let cuts: Vec<usize> = (1..line.len().saturating_sub(1))
        .filter(|&i| matches!((line.get(i - 1), line.get(i), line.get(i + 1)), (Some(&a), Some(&b), Some(&c)) if is_corner(a, b, c)))
        .collect();
    let mut out = vec![];
    let mut start = 0;
    for c in cuts.into_iter().chain([line.len().saturating_sub(1)]) {
        if let Some(s) = line.get(start..=c) {
            out.push(s.to_vec());
        }
        start = c;
    }
    out.retain(|s| s.len() > 1);
    (out, false)
}

/// A line to sew and how: the stitch, the satin width (mm as sewn) and the points.
pub(crate) type Piece = (PartStitch, f64, Vec<Point>);

/// Add `part`, its points put on the fabric by `map` from pattern space, to `pieces`: lines cut
/// into short pieces first (not manual stitches, whose points are needle points), so they bend;
/// running and bean lines joining the line before them carry it on.
pub(crate) fn add_part(pieces: &mut Vec<Piece>, part: &PatternPart, scale: (f64, f64), map: &dyn Fn(f64, f64) -> Point) {
    let mut line: Vec<Point> = vec![];
    for (k, &[x, y]) in part.points.iter().enumerate() {
        if part.stitch != PartStitch::Manual
            && let Some(&[px, py]) = k.checked_sub(1).and_then(|j| part.points.get(j))
        {
            let d = ((x - px) * scale.0).hypot((y - py) * scale.1);
            let n = (d / BEND_STEP).ceil().clamp(1.0, 10_000.0) as usize;
            for j in 1..n {
                let t = j as f64 / n as f64;
                line.push(map(px + (x - px) * t, py + (y - py) * t));
            }
        }
        line.push(map(x, y));
    }
    let width = part.width.unwrap_or(1.0) * scale.1;
    if let Some((stitch, _, prev)) = pieces.last_mut()
        && *stitch == part.stitch
        && matches!(part.stitch, PartStitch::Running | PartStitch::Bean)
        && prev.last().zip(line.first()).is_some_and(|(a, b)| a.distance(*b) < 1e-6)
    {
        prev.extend(line.into_iter().skip(1));
        return;
    }
    if !line.is_empty() {
        pieces.push((part.stitch, width, line));
    }
}

/// The needle points of `pieces`, one after another as one run.
pub(crate) fn stitch(pieces: Vec<Piece>, len: f64, spacing: f64) -> Vec<Point> {
    let mut run: Vec<Point> = vec![];
    for (kind, width, line) in pieces {
        let pts = match kind {
            PartStitch::Running => generate::running(&line, len),
            PartStitch::Bean => generate::bean(&generate::running(&line, len), BEAN_PASSES),
            PartStitch::Satin => generate::satin(&line, false, width.clamp(0.3, MAX_STITCH), spacing),
            PartStitch::Manual => line,
        };
        let skip = usize::from(run.last().zip(pts.first()).is_some_and(|(a, b)| a.distance(*b) < 1e-6));
        run.extend(pts.into_iter().skip(skip));
    }
    // Belt and braces: a point that isn't a number never reaches the machine.
    run.retain(|p| p.is_finite());
    generate::split_long(&run)
}

/// Sew `pattern` along `line` (mm; `closed` when it is a closed subpath), the band `width` mm
/// wide (mirrored across the path when `flip`), running and bean parts in stitches `len` long and
/// satin parts with zigs `spacing` apart. The runs: one per stretch between corners, or one for
/// the whole line when the pattern has a corner motif (sewn through each corner).
pub fn sew(pattern: &BorderPattern, line: &[Point], closed: bool, width: f64, flip: bool, len: f64, spacing: f64) -> Result<Vec<Vec<Point>>, String> {
    check(pattern)?;
    if !(width.is_finite() && width > 0.0) {
        return Err("a pattern border needs a width".into());
    }
    let scale = width / pattern.width;
    let ys = if flip { -scale } else { scale };
    let (parts, ring) = stretches(line, closed);
    let parts: Vec<Stretch> = parts.into_iter().map(|p| Stretch::new(p, ring)).collect();
    let motif = !pattern.corner.is_empty() && !ring;
    // How far the corner motif reaches from its corner: the repeats stop that far short of it.
    let reach = if motif { pattern.corner.iter().flat_map(|q| &q.points).map(|[x, y]| x.hypot(*y)).fold(0.0, f64::max) * scale } else { 0.0 };
    let per_repeat = pattern.repeat.iter().map(|q| q.points.len()).sum::<usize>().max(1);
    let total = parts.iter().map(Stretch::len).sum::<f64>();
    let rep = pattern.length * scale;
    if total / rep * per_repeat as f64 > MAX_NEEDLES as f64 {
        return Err(format!("the border pattern `{}` repeats too many times along so long a path: make it wider", pattern.name));
    }
    let closed_corners = closed && !ring && !parts.is_empty() && line.first().zip(line.last()).is_some_and(|(a, b)| a.distance(*b) < 1e-6);
    let count = parts.len();
    let mut runs = vec![];
    let mut pieces: Vec<Piece> = vec![];
    for (k, st) in parts.iter().enumerate() {
        // Corners at the start and end of this stretch (a closed line: every end is one).
        let corner_in = motif && (k > 0 || closed_corners);
        let corner_out = motif && (k + 1 < count || closed_corners);
        let (from, to) = (if corner_in { reach } else { 0.0 }, st.len() - if corner_out { reach } else { 0.0 });
        let avail = to - from;
        let n = if avail >= rep * 0.5 { (avail / rep).round().max(1.0) } else { 0.0 };
        let centre =
            |a: f64, b: f64| PatternPart { stitch: PartStitch::Running, points: vec![[a, 0.0], [b, 0.0]], width: None, ..Default::default() };
        let along = |x: f64, y: f64| {
            let (p, nrm) = st.frame(x);
            p + nrm * (y * ys)
        };
        if corner_in && from > 0.0 {
            // From the corner along the centre line to where the repeats start (under the motif).
            add_part(&mut pieces, &centre(0.0, from), (1.0, 1.0), &|x, _| st.frame(x).0);
        }
        if n > 0.0 {
            let step = avail / n;
            let xs = step / pattern.length;
            for i in 0..n as usize {
                let x0 = from + i as f64 * step;
                for part in &pattern.repeat {
                    add_part(&mut pieces, part, (xs, scale), &|x, y| along(x0 + x * xs, y));
                }
            }
        } else if motif && avail > 0.0 {
            // Too short for a repeat between two corners: the centre line joins them.
            add_part(&mut pieces, &centre(from, to), (1.0, 1.0), &|x, _| st.frame(x).0);
        }
        if corner_out {
            if to < st.len() {
                add_part(&mut pieces, &centre(to, st.len()), (1.0, 1.0), &|x, _| st.frame(x).0);
            }
            // The motif, centred on the corner, x along the direction of travel through it.
            let next = parts.get((k + 1) % count).unwrap_or(st);
            let (t_in, t_out) = (st.direction(true), next.direction(false));
            let mut d = t_in + t_out;
            if d.hypot() < 1e-6 {
                d = Vec2::new(-t_in.y, t_in.x);
            }
            let d = d / d.hypot();
            let m = Vec2::new(-d.y, d.x);
            let at = st.pts.last().copied().unwrap_or(Point::ZERO);
            for part in &pattern.corner {
                add_part(&mut pieces, part, (scale, scale), &|x, y| at + d * (x * scale) + m * (y * ys));
            }
        }
        if !motif {
            runs.push(stitch(std::mem::take(&mut pieces), len, spacing));
        }
    }
    if motif {
        runs.push(stitch(pieces, len, spacing));
    }
    runs.retain(|r| r.len() > 1);
    Ok(runs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f64, h: f64) -> Vec<Point> {
        vec![Point::new(0.0, 0.0), Point::new(w, 0.0), Point::new(w, h), Point::new(0.0, h), Point::new(0.0, 0.0)]
    }

    fn longest(runs: &[Vec<Point>]) -> f64 {
        runs.iter().flat_map(|r| r.windows(2).map(|w| w[0].distance(w[1]))).fold(0.0, f64::max)
    }

    #[test]
    fn starter_patterns_are_seamless() {
        for id in BUILTIN {
            let p = builtin(id).unwrap();
            check(&p).unwrap();
            let first = p.repeat.first().and_then(|q| q.points.first()).unwrap();
            let last = p.repeat.last().and_then(|q| q.points.last()).unwrap();
            assert!((last[0] - first[0] - p.length).abs() < 1e-9 && (last[1] - first[1]).abs() < 1e-9, "{id}: {first:?} → {last:?}");
            for q in &p.corner {
                let (a, b) = (q.points.first().unwrap(), q.points.last().unwrap());
                assert!(a[0].hypot(a[1]) < 1e-9 && b[0].hypot(b[1]) < 1e-9, "{id}: a corner starts and ends at the corner");
            }
        }
        assert!(builtin("stars").is_none());
    }

    #[test]
    fn whole_repeats_fill_a_straight_line_within_the_band() {
        let p = builtin("waves").unwrap();
        // 95 mm of 10 mm repeats: ten, each stretched to 9.5 mm.
        let line = [Point::new(0.0, 0.0), Point::new(95.0, 0.0)];
        let runs = sew(&p, &line, false, 6.0, false, 1.5, 0.4).unwrap();
        assert_eq!(runs.len(), 1);
        let r = &runs[0];
        assert!(r[0].distance(Point::ZERO) < 1e-6 && r.last().unwrap().distance(Point::new(95.0, 0.0)) < 1e-6);
        assert!(r.iter().all(|q| q.y.abs() <= 2.5 + 1e-6 && q.x >= -1e-6 && q.x <= 95.0 + 1e-6));
        // One crest a repeat: the points in the top of the band, in clusters along the line.
        let mut tops: Vec<f64> = r.iter().filter(|q| q.y > 1.5).map(|q| q.x).collect();
        tops.sort_by(f64::total_cmp);
        let crests = 1 + tops.windows(2).filter(|w| w[1] - w[0] > 3.0).count();
        assert_eq!(crests, 10, "{tops:?}");
        // (A last stitch much shorter than the others joins the one before it.)
        assert!(longest(&runs) <= 1.5 * 1.25 + 1e-6);
    }

    #[test]
    fn width_scales_and_flip_mirrors() {
        let p = builtin("chain").unwrap();
        let line = [Point::new(0.0, 0.0), Point::new(60.0, 0.0)];
        let a = sew(&p, &line, false, 12.0, false, 1.5, 0.4).unwrap();
        let b = sew(&p, &line, false, 12.0, true, 1.5, 0.4).unwrap();
        let ymax = a[0].iter().map(|q| q.y.abs()).fold(0.0, f64::max);
        assert!((ymax - 5.2).abs() < 0.1, "twice the 6 mm pattern: {ymax}");
        assert_eq!(a[0].len(), b[0].len());
        assert!(a[0].iter().zip(&b[0]).all(|(p, q)| (p.x - q.x).abs() < 1e-9 && (p.y + q.y).abs() < 1e-9));
    }

    #[test]
    fn corners_restart_the_border_without_a_corner_motif() {
        let p = builtin("scallops").unwrap();
        let runs = sew(&p, &rect(60.0, 30.0), true, 6.0, false, 1.5, 0.4).unwrap();
        assert_eq!(runs.len(), 4, "one run per side");
        assert!(longest(&runs) <= MAX_STITCH);
        // Each side starts at its corner (the scallops stand on one edge of the band, 2.4 mm in).
        let corners = rect(60.0, 30.0);
        for (r, c) in runs.iter().zip(&corners) {
            assert!(r[0].distance(*c) < 4.0, "{:?} near {c:?}", r[0]);
        }
    }

    #[test]
    fn a_corner_motif_is_sewn_at_each_corner_in_one_run() {
        let p = builtin("diamonds").unwrap();
        let runs = sew(&p, &rect(80.0, 50.0), true, 6.0, false, 1.5, 0.4).unwrap();
        assert_eq!(runs.len(), 1);
        let r = &runs[0];
        // The rosette's petals reach 4 mm from each corner, outside the rectangle too.
        for c in rect(80.0, 50.0).iter().take(4) {
            let far = r.iter().filter(|q| q.distance(*c) < 4.2).map(|q| q.distance(*c)).fold(0.0, f64::max);
            assert!(far > 3.5, "a rosette at {c:?}: {far}");
        }
        assert!(r.first().unwrap().distance(*r.last().unwrap()) < 1e-6, "a closed border ends where it began");
        // Outside the corners the band stays within its 3 mm of the outline.
        let off = |q: &Point| q.x.abs().min((q.x - 80.0).abs()).min(q.y.abs()).min((q.y - 50.0).abs());
        assert!(r.iter().all(|q| off(q) <= 4.0 + 1e-6));
    }

    #[test]
    fn a_ring_without_corners_repeats_all_the_way_round() {
        let p = builtin("waves").unwrap();
        let n = 360;
        let circle: Vec<Point> = (0..=n)
            .map(|k| {
                let a = 2.0 * std::f64::consts::PI * (k % n) as f64 / n as f64;
                Point::new(30.0 + 20.0 * a.cos(), 30.0 + 20.0 * a.sin())
            })
            .collect();
        let runs = sew(&p, &circle, true, 6.0, false, 1.5, 0.4).unwrap();
        assert_eq!(runs.len(), 1);
        let r = &runs[0];
        assert!(r.first().unwrap().distance(*r.last().unwrap()) < 1e-6, "the repeats meet where they began");
        assert!(r.iter().all(|q| (q.distance(Point::new(30.0, 30.0)) - 20.0).abs() <= 2.5 + 1e-3));
    }

    #[test]
    fn wide_manual_patterns_are_split_into_sewable_stitches() {
        let p = BorderPattern {
            name: "zag".into(),
            width: 2.0,
            length: 2.0,
            repeat: vec![PatternPart {
                stitch: PartStitch::Manual,
                points: vec![[0.0, -1.0], [1.0, 1.0], [2.0, -1.0]],
                width: None,
                ..Default::default()
            }],
            ..Default::default()
        };
        let runs = sew(&p, &[Point::new(0.0, 0.0), Point::new(200.0, 0.0)], false, 40.0, false, 1.5, 0.4).unwrap();
        assert!(longest(&runs) <= MAX_STITCH + 1e-9);
    }

    #[test]
    fn files_round_trip_and_name_their_pattern() {
        let mut p = builtin("diamonds").unwrap();
        p.id = "My Diamonds!".into();
        let text = write_file(&p).unwrap();
        assert!(text.contains("\"format\": \"vectorcraft-border-pattern\"") && text.contains("\"units\": \"mm\""), "{text}");
        let back = read_file(text.as_bytes()).unwrap();
        assert_eq!(back.id, "my-diamonds");
        assert!(back.extra.is_empty(), "the file's header isn't taken for unknown members: {:?}", back.extra);
        // A member a newer version added survives a read and a write.
        let newer = text.replacen("\"name\"", "\"thumbnail\": \"x.png\", \"name\"", 1);
        let kept = read_file(newer.as_bytes()).unwrap();
        assert_eq!(kept.extra.get("thumbnail"), Some(&serde_json::json!("x.png")));
        assert_eq!(write_file(&kept).unwrap().matches("\"format\"").count(), 1);
        assert_eq!(BorderPattern { id: p.id.clone(), ..back }, p);
        // No id: one is made from the name.
        let v = serde_json::json!({"format": FILE_FORMAT, "version": 1, "name": "Rope Twist 2", "width": 4, "length": 5,
            "repeat": [{"stitch": "running", "points": [[0, 0], [5, 0]]}]});
        let q = read_file(v.to_string().as_bytes()).unwrap();
        assert_eq!((q.id.as_str(), q.license.as_deref()), ("rope-twist-2", None));
        assert_eq!(slug("  --  "), "pattern");
    }

    #[test]
    fn files_that_arent_patterns_are_refused() {
        let ok = serde_json::json!({"format": FILE_FORMAT, "version": 1, "name": "x", "width": 4, "length": 5,
            "repeat": [{"points": [[0, 0], [5, 0]]}]});
        assert!(read_file(ok.to_string().as_bytes()).is_ok());
        for (k, v) in [
            ("format", serde_json::json!("vectorcraft-font")),
            ("version", serde_json::json!(2)),
            ("units", serde_json::json!("in")),
            ("width", serde_json::json!(-1)),
        ] {
            let mut bad = ok.clone();
            bad[k] = v;
            assert!(read_file(bad.to_string().as_bytes()).is_err(), "{k}");
        }
        assert!(read_file(b"").is_err());
        assert!(read_file(b"{\"format\": 1}").is_err());
        assert!(read_file(&vec![b' '; MAX_FILE_BYTES + 1]).is_err());
    }

    #[test]
    fn document_patterns_come_before_the_starters() {
        let mine = BorderPattern { id: "waves".into(), name: "My waves".into(), ..builtin("chain").unwrap() };
        assert_eq!(find(std::slice::from_ref(&mine), "waves").unwrap().name, "My waves");
        assert_eq!(find(&[], "waves").unwrap().name, "Waves");
        assert!(find(&[mine], "stars").is_none());
    }

    proptest::proptest! {
        // Untrusted files: whatever they hold, reading and sewing them returns, never panics.
        #[test]
        fn any_file_reads_or_is_refused(
            width in proptest::prop_oneof![proptest::num::f64::ANY, 0.0..30.0],
            length in proptest::prop_oneof![proptest::num::f64::ANY, 0.0..30.0],
            pts in proptest::collection::vec(proptest::prop_oneof![(proptest::num::f64::ANY, proptest::num::f64::ANY), (-20.0..40.0, -20.0..20.0)], 0..12),
            kind in 0usize..4,
            sat in proptest::option::of(proptest::prop_oneof![proptest::num::f64::ANY, 0.0..15.0]),
            corner in proptest::bool::ANY,
            junk in ".{0,40}",
        ) {
            let stitch = ["running", "bean", "satin", "manual"][kind];
            let points: Vec<[f64; 2]> = pts.iter().map(|&(x, y)| [x, y]).collect();
            let part = serde_json::json!({"stitch": stitch, "points": points, "width": sat});
            let v = serde_json::json!({"format": FILE_FORMAT, "version": 1, "name": junk, "width": width, "length": length,
                "repeat": [part.clone()], "corner": if corner { vec![part] } else { vec![] }});
            let _ = read_file(junk.as_bytes());
            if let Ok(p) = read_file(v.to_string().as_bytes()) {
                let square = [Point::new(0.0, 0.0), Point::new(40.0, 0.0), Point::new(40.0, 40.0), Point::new(0.0, 40.0), Point::new(0.0, 0.0)];
                if let Ok(runs) = sew(&p, &square, true, 8.0, false, 1.5, 0.4) {
                    proptest::prop_assert!(longest(&runs) <= MAX_STITCH + 1e-6);
                }
            }
        }
    }

    #[test]
    fn bad_patterns_and_sizes_are_refused() {
        let good = builtin("waves").unwrap();
        let line = [Point::new(0.0, 0.0), Point::new(50.0, 0.0)];
        for bad in [
            BorderPattern { width: f64::NAN, ..good.clone() },
            BorderPattern { length: 0.0, ..good.clone() },
            BorderPattern { repeat: vec![], ..good.clone() },
            BorderPattern { repeat: vec![PatternPart { points: vec![[f64::INFINITY, 0.0]], ..Default::default() }], ..good.clone() },
        ] {
            assert!(sew(&bad, &line, false, 6.0, false, 1.5, 0.4).is_err());
        }
        assert!(sew(&good, &line, false, f64::NAN, false, 1.5, 0.4).is_err());
        let tiny = BorderPattern { length: 0.001, ..good.clone() };
        assert!(sew(&tiny, &[Point::new(0.0, 0.0), Point::new(1e6, 0.0)], false, 6.0, false, 1.5, 0.4).is_err());
        // A path shorter than half a repeat has no room for one.
        assert!(sew(&good, &[Point::new(0.0, 0.0), Point::new(4.0, 0.0)], false, 6.0, false, 1.5, 0.4).unwrap().is_empty());
    }
}
