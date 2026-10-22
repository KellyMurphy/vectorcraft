//! Thread charts: a maker's threads (code, name, colour), loaded into the document from `.vcthreads`
//! files ([`vectorcraft_doc::EmbroiderySetup::charts`], specified in `docs/formats/vcthreads.md`),
//! and the nearest thread to each colour of the design, by perceived colour difference (CIEDE2000
//! between the colours in CIE Lab, D65).
//!
//! VectorCraft ships no makers' charts: their numbers and colours are the makers' data, and none is
//! published under terms that allow redistributing it. Users load charts they have the right to use.

use vectorcraft_doc::{ChartThread, ThreadChart};

/// The file extension of thread chart files.
pub const FILE_EXT: &str = "vcthreads";
/// The `format` a thread chart file declares.
pub const FILE_FORMAT: &str = "vectorcraft-thread-chart";
/// The file format version written; files of a later version aren't read.
pub const FILE_VERSION: u32 = 1;
/// The largest thread chart file read, bytes, and the most threads it holds.
pub const MAX_FILE_BYTES: usize = 4 << 20;
pub const MAX_THREADS: usize = 20_000;

/// `#rrggbb` (or `rrggbb`) as sRGB.
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    let c = |i: usize| h.get(i..i + 2).and_then(|t| u8::from_str_radix(t, 16).ok());
    (h.len() == 6).then_some([c(0)?, c(2)?, c(4)?])
}

/// sRGB as CIE Lab (D65).
pub fn lab(rgb: [u8; 3]) -> [f64; 3] {
    let lin = |c: u8| {
        let v = f64::from(c) / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    let [r, g, b] = rgb.map(lin);
    let x = (0.4124564 * r + 0.3575761 * g + 0.1804375 * b) / 0.95047;
    let y = 0.2126729 * r + 0.7151522 * g + 0.0721750 * b;
    let z = (0.0193339 * r + 0.1191920 * g + 0.9503041 * b) / 1.08883;
    let f = |t: f64| if t > 216.0 / 24389.0 { t.cbrt() } else { (24389.0 / 27.0 * t + 16.0) / 116.0 };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// The CIEDE2000 colour difference between two Lab colours.
pub fn ciede2000(p: [f64; 3], q: [f64; 3]) -> f64 {
    let [l1, a1, b1] = p;
    let [l2, a2, b2] = q;
    let c1 = a1.hypot(b1);
    let c2 = a2.hypot(b2);
    let cm = (c1 + c2) / 2.0;
    let g = 0.5 * (1.0 - (cm.powi(7) / (cm.powi(7) + 25f64.powi(7))).sqrt());
    let (a1p, a2p) = ((1.0 + g) * a1, (1.0 + g) * a2);
    let (c1p, c2p) = (a1p.hypot(b1), a2p.hypot(b2));
    let hue = |b: f64, a: f64| {
        if b == 0.0 && a == 0.0 {
            0.0
        } else {
            let h = b.atan2(a).to_degrees();
            if h < 0.0 { h + 360.0 } else { h }
        }
    };
    let (h1p, h2p) = (hue(b1, a1p), hue(b2, a2p));
    let dl = l2 - l1;
    let dc = c2p - c1p;
    let dh = if c1p * c2p == 0.0 {
        0.0
    } else if (h2p - h1p).abs() <= 180.0 {
        h2p - h1p
    } else if h2p - h1p > 180.0 {
        h2p - h1p - 360.0
    } else {
        h2p - h1p + 360.0
    };
    let dhh = 2.0 * (c1p * c2p).sqrt() * (dh / 2.0).to_radians().sin();
    let lm = (l1 + l2) / 2.0;
    let cmp = (c1p + c2p) / 2.0;
    let hm = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };
    let t = 1.0 - 0.17 * (hm - 30.0).to_radians().cos() + 0.24 * (2.0 * hm).to_radians().cos() + 0.32 * (3.0 * hm + 6.0).to_radians().cos()
        - 0.20 * (4.0 * hm - 63.0).to_radians().cos();
    let dtheta = 30.0 * (-((hm - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (cmp.powi(7) / (cmp.powi(7) + 25f64.powi(7))).sqrt();
    let sl = 1.0 + 0.015 * (lm - 50.0).powi(2) / (20.0 + (lm - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cmp;
    let sh = 1.0 + 0.015 * cmp * t;
    let rt = -(2.0 * dtheta).to_radians().sin() * rc;
    ((dl / sl).powi(2) + (dc / sc).powi(2) + (dhh / sh).powi(2) + rt * (dc / sc) * (dhh / sh)).sqrt()
}

/// The chart thread nearest `rgb`, and how far it is (CIEDE2000; under about 2 is hard to tell
/// apart).
pub fn nearest(chart: &ThreadChart, rgb: [u8; 3]) -> Option<(&ChartThread, f64)> {
    let want = lab(rgb);
    chart.threads.iter().filter_map(|t| Some((t, ciede2000(want, lab(parse_hex(&t.color)?))))).min_by(|a, b| a.1.total_cmp(&b.1))
}

/// A thread chart file as stored: the format header, then the chart's fields.
#[derive(serde::Serialize, serde::Deserialize)]
struct File {
    format: String,
    version: u32,
    #[serde(flatten)]
    chart: ThreadChart,
}

/// Read a `.vcthreads` file: JSON with `format` [`FILE_FORMAT`], `version` and the chart, 1 to
/// [`MAX_THREADS`] threads each with a code and an `#rrggbb` colour. Its id is its own, else made
/// from its name.
pub fn read_file(bytes: &[u8]) -> Result<ThreadChart, String> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(format!("a thread chart file is at most {} MB", MAX_FILE_BYTES >> 20));
    }
    let f: File = serde_json::from_slice(bytes).map_err(|e| format!("not a thread chart file: {e}"))?;
    if f.format != FILE_FORMAT {
        return Err(format!("not a thread chart file (its format is `{}`)", f.format));
    }
    if f.version > FILE_VERSION {
        return Err(format!("this thread chart file is version {}: this VectorCraft reads up to version {FILE_VERSION}", f.version));
    }
    let mut c = f.chart;
    if c.threads.is_empty() || c.threads.len() > MAX_THREADS {
        return Err(format!("a thread chart holds 1 to {MAX_THREADS} threads"));
    }
    if let Some(t) = c.threads.iter().find(|t| t.code.trim().is_empty() || parse_hex(&t.color).is_none()) {
        return Err(format!("thread `{}` needs a code and a colour #rrggbb", t.code));
    }
    c.id = crate::borders::slug(if c.id.trim().is_empty() { &c.name } else { &c.id });
    if c.name.trim().is_empty() {
        c.name.clone_from(&c.id);
    }
    Ok(c)
}

/// `c` as a `.vcthreads` file.
pub fn write_file(c: &ThreadChart) -> Result<String, String> {
    let f = File { format: FILE_FORMAT.into(), version: FILE_VERSION, chart: c.clone() };
    serde_json::to_string_pretty(&f).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ciede2000_matches_published_test_pairs() {
        // Pairs from Sharma, Wu and Dalal's CIEDE2000 test data (pairs 1, 7 and 17).
        let cases = [
            ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
            ([50.0, 0.0, 0.0], [50.0, -1.0, 2.0], 2.3669),
            ([50.0, 2.5, 0.0], [73.0, 25.0, -18.0], 27.1492),
        ];
        for (p, q, want) in cases {
            assert!((ciede2000(p, q) - want).abs() < 1e-3, "{} vs {want}", ciede2000(p, q));
        }
        assert!(ciede2000(lab([255, 0, 0]), lab([255, 0, 0])).abs() < 1e-9);
    }

    #[test]
    fn the_nearest_thread_is_found_and_files_read_back() {
        let t =
            |code: &str, color: &str| ChartThread { code: code.into(), name: format!("Thread {code}"), color: color.into(), ..Default::default() };
        let chart = ThreadChart {
            id: "c".into(),
            name: "C".into(),
            threads: vec![t("1", "#ff0000"), t("2", "#00a000"), t("3", "#2040c0")],
            ..Default::default()
        };
        let (near, d) = nearest(&chart, [250, 10, 10]).unwrap();
        assert_eq!(near.code, "1");
        assert!(d < 5.0, "{d}");
        assert_eq!(nearest(&chart, [30, 60, 200]).map(|n| n.0.code.as_str()), Some("3"));
        assert_eq!(read_file(write_file(&chart).unwrap().as_bytes()).unwrap(), chart);
        assert!(
            read_file(br#"{"format": "vectorcraft-thread-chart", "version": 1, "name": "x", "threads": [{"code": "1", "color": "red"}]}"#).is_err()
        );
        assert!(read_file(br#"{"format": "vectorcraft-thread-chart", "version": 1, "name": "x", "threads": []}"#).is_err());
        assert_eq!(parse_hex("#0A0b0C"), Some([10, 11, 12]));
    }
}
