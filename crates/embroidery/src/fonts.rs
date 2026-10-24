//! Embroidery font files (`.vcfont`, specified in `docs/formats/vcfont.md`): reading, checking and
//! writing [`EmbroideryFont`]s, a letter's outlines ([`outline`]), and a small font drawn here in
//! code for tests and examples ([`sample`]). Fonts themselves live in craft-fonts or wherever users
//! keep them, never in this repository.

use kurbo::BezPath;
use vectorcraft_doc::{EmbroideryFont, FontGlyph, GlyphObject, KernPair, StitchKind, StitchSettings};

/// The font file's extension.
pub const FILE_EXT: &str = "vcfont";
/// The `format` a font file declares.
pub const FILE_FORMAT: &str = "vectorcraft-embroidery-font";
/// The file format version written; files of a later version aren't read.
pub const FILE_VERSION: u32 = 1;
/// The largest font file read, bytes.
pub const MAX_FILE_BYTES: usize = 32 << 20;
/// The most glyphs a font has.
pub const MAX_GLYPHS: usize = 4000;
/// The most stitch objects a glyph has.
pub const MAX_OBJECTS: usize = 256;
/// The most characters a glyph sets (a ligature).
pub const MAX_GLYPH_CHARS: usize = 8;
/// The most path elements in a font, all glyphs together.
pub const MAX_PATH_ELEMENTS: usize = 2_000_000;
/// The most kerning pairs.
pub const MAX_KERNING: usize = 100_000;
/// Units per em a font may have.
pub const UNITS_PER_EM: std::ops::RangeInclusive<f64> = 16.0..=16384.0;
/// Cap heights a font may be made for, mm.
pub const SIZES: std::ops::RangeInclusive<f64> = 1.0..=300.0;

/// A font file as stored: the format header, then the font's fields.
#[derive(serde::Serialize, serde::Deserialize)]
struct File {
    format: String,
    version: u32,
    #[serde(flatten)]
    font: EmbroideryFont,
}

/// Read a `.vcfont` file: JSON with `format` [`FILE_FORMAT`], `version` and the font, checked
/// ([`check`]). Its id is its own, else made from its name.
pub fn read_file(bytes: &[u8]) -> Result<EmbroideryFont, String> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(format!("an embroidery font file is at most {} MB", MAX_FILE_BYTES >> 20));
    }
    let f: File = serde_json::from_slice(bytes).map_err(|e| format!("not an embroidery font file: {e}"))?;
    if f.format != FILE_FORMAT {
        return Err(format!("not an embroidery font file (its format is `{}`)", f.format));
    }
    if f.version > FILE_VERSION {
        return Err(format!("this embroidery font file is version {}: this VectorCraft reads up to version {FILE_VERSION}", f.version));
    }
    let mut font = f.font;
    font.id = crate::borders::slug(if font.id.trim().is_empty() { &font.name } else { &font.id });
    if font.name.trim().is_empty() {
        font.name.clone_from(&font.id);
    }
    check(&font)?;
    Ok(font)
}

/// `font` as a `.vcfont` file.
pub fn write_file(font: &EmbroideryFont) -> Result<String, String> {
    check(font)?;
    let f = File { format: FILE_FORMAT.into(), version: FILE_VERSION, font: font.clone() };
    serde_json::to_string_pretty(&f).map_err(|e| e.to_string())
}

/// Whether `font` is one VectorCraft can use: its numbers finite and in range, its paths SVG path
/// data, its counts within the limits, no character set by two glyphs.
pub fn check(font: &EmbroideryFont) -> Result<(), String> {
    let upem = font.units_per_em;
    if !UNITS_PER_EM.contains(&upem) {
        return Err(format!("unitsPerEm must be {} to {}", UNITS_PER_EM.start(), UNITS_PER_EM.end()));
    }
    let metric = |v: f64| v.is_finite() && v.abs() <= 10.0 * upem;
    for (key, v) in [("ascender", font.ascender), ("descender", font.descender), ("capHeight", font.cap_height), ("xHeight", font.x_height)] {
        if !metric(v) {
            return Err(format!("`{key}` must be a number within ten em of the baseline"));
        }
    }
    if let Some([lo, hi]) = font.sizes
        && !(SIZES.contains(&lo) && SIZES.contains(&hi) && lo <= hi)
    {
        return Err(format!("`sizes` must be [smallest, largest] cap heights from {} to {} mm", SIZES.start(), SIZES.end()));
    }
    if font.glyphs.is_empty() {
        return Err("a font has no glyphs".into());
    }
    if font.glyphs.len() > MAX_GLYPHS {
        return Err(format!("a font has at most {MAX_GLYPHS} glyphs"));
    }
    if font.kerning.len() > MAX_KERNING {
        return Err(format!("a font has at most {MAX_KERNING} kerning pairs"));
    }
    let mut seen = std::collections::HashSet::new();
    let mut elements = 0usize;
    for g in &font.glyphs {
        let n = g.chars.chars().count();
        if n == 0 || n > MAX_GLYPH_CHARS {
            return Err(format!("a glyph sets 1 to {MAX_GLYPH_CHARS} characters (`chars`)"));
        }
        if !seen.insert(g.chars.as_str()) {
            return Err(format!("two glyphs set `{}`", g.chars));
        }
        if !(g.advance.is_finite() && (0.0..=10.0 * upem).contains(&g.advance)) {
            return Err(format!("glyph `{}`: `advance` must be 0 to ten em", g.chars));
        }
        for p in [g.entry, g.exit].into_iter().flatten() {
            if !p.iter().all(|v| metric(*v)) {
                return Err(format!("glyph `{}`: entry and exit points must be within ten em", g.chars));
            }
        }
        if g.objects.len() > MAX_OBJECTS {
            return Err(format!("glyph `{}` has more than {MAX_OBJECTS} objects", g.chars));
        }
        for o in &g.objects {
            if o.rule.as_deref().is_some_and(|r| r != "nonzero" && r != "evenodd") {
                return Err(format!("glyph `{}`: `rule` must be nonzero or evenodd", g.chars));
            }
            let path = BezPath::from_svg(&o.path).map_err(|e| format!("glyph `{}`: its path isn't SVG path data ({e})", g.chars))?;
            elements = elements.saturating_add(path.elements().len());
            if elements > MAX_PATH_ELEMENTS {
                return Err(format!("a font's paths have at most {MAX_PATH_ELEMENTS} elements"));
            }
            if !path.elements().iter().all(|e| e.end_point().is_none_or(|p| metric(p.x) && metric(p.y))) {
                return Err(format!("glyph `{}`: its path reaches further than ten em", g.chars));
            }
        }
    }
    if let Some(k) = font.kerning.iter().find(|k| !(k.value.is_finite() && k.value.abs() <= 10.0 * upem) || k.left.is_empty() || k.right.is_empty()) {
        return Err(format!("kerning `{}` `{}`: a value within ten em between two glyphs' characters", k.left, k.right));
    }
    Ok(())
}

/// Object `o`'s outline, font units with y up (none when it isn't path data: [`check`] refuses that).
pub fn outline(o: &GlyphObject) -> Option<BezPath> {
    BezPath::from_svg(&o.path).ok()
}

/// A small sans font drawn here in code (straight strokes as filled outlines, Auto stitch), for tests
/// and the format's example: `I L T H E F O -` and the space. CC0.
pub fn sample() -> EmbroideryFont {
    // A stroke 100 units wide; caps 700 tall.
    let rect = |x0: f64, y0: f64, x1: f64, y1: f64| format!("M{x0} {y0} L{x1} {y0} L{x1} {y1} L{x0} {y1} Z");
    let auto = || StitchSettings::new(StitchKind::Auto);
    let glyph = |chars: &str, advance: f64, paths: Vec<String>| FontGlyph {
        chars: chars.into(),
        advance,
        objects: paths.into_iter().map(|path| GlyphObject { path, stitch: auto(), ..Default::default() }).collect(),
        ..Default::default()
    };
    let ring = {
        // Two circles, the inner one the hole (even-odd), drawn with four cubic arcs each.
        let circle = |cx: f64, cy: f64, r: f64| {
            let k = 0.5523 * r;
            format!(
                "M{} {cy} C{} {} {} {} {cx} {} C{} {} {} {} {} {cy} C{} {} {} {} {cx} {} C{} {} {} {} {} {cy} Z",
                cx + r,
                cx + r,
                cy + k,
                cx + k,
                cy + r,
                cy + r,
                cx - k,
                cy + r,
                cx - r,
                cy + k,
                cx - r,
                cx - r,
                cy - k,
                cx - k,
                cy - r,
                cy - r,
                cx + k,
                cy - r,
                cx + r,
                cy - k,
                cx + r
            )
        };
        GlyphObject {
            path: format!("{} {}", circle(350.0, 350.0, 350.0), circle(350.0, 350.0, 250.0)),
            rule: Some("evenodd".into()),
            stitch: auto(),
            ..Default::default()
        }
    };
    EmbroideryFont {
        id: "vectorcraft-sample-sans".into(),
        name: "VectorCraft Sample Sans".into(),
        family: Some("VectorCraft Sample Sans".into()),
        style: Some("Regular".into()),
        author: Some("VectorCraft contributors".into()),
        license: Some("CC0-1.0".into()),
        description: Some("A few straight-stroke capitals drawn in code, for tests and examples.".into()),
        version: Some("1.0".into()),
        units_per_em: 1000.0,
        ascender: 750.0,
        descender: -250.0,
        cap_height: 700.0,
        x_height: 500.0,
        sizes: Some([6.0, 40.0]),
        glyphs: vec![
            glyph(" ", 300.0, vec![]),
            glyph("I", 250.0, vec![rect(75.0, 0.0, 175.0, 700.0)]),
            glyph("L", 520.0, vec![rect(75.0, 0.0, 175.0, 700.0), rect(175.0, 0.0, 470.0, 100.0)]),
            glyph("T", 600.0, vec![rect(250.0, 0.0, 350.0, 600.0), rect(20.0, 600.0, 580.0, 700.0)]),
            glyph("H", 650.0, vec![rect(75.0, 0.0, 175.0, 700.0), rect(475.0, 0.0, 575.0, 700.0), rect(175.0, 300.0, 475.0, 400.0)]),
            glyph(
                "E",
                560.0,
                vec![
                    rect(75.0, 0.0, 175.0, 700.0),
                    rect(175.0, 0.0, 500.0, 100.0),
                    rect(175.0, 300.0, 440.0, 400.0),
                    rect(175.0, 600.0, 500.0, 700.0),
                ],
            ),
            glyph("F", 540.0, vec![rect(75.0, 0.0, 175.0, 700.0), rect(175.0, 300.0, 440.0, 400.0), rect(175.0, 600.0, 500.0, 700.0)]),
            FontGlyph { chars: "O".into(), advance: 760.0, objects: vec![ring], ..Default::default() },
            glyph("-", 400.0, vec![rect(60.0, 300.0, 340.0, 400.0)]),
        ],
        kerning: vec![
            KernPair { left: "L".into(), right: "T".into(), value: -120.0 },
            KernPair { left: "T".into(), right: "O".into(), value: -40.0 },
        ],
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sample_round_trips_through_a_file() {
        let font = sample();
        check(&font).unwrap();
        let text = write_file(&font).unwrap();
        assert!(text.contains("\"format\": \"vectorcraft-embroidery-font\""));
        assert_eq!(read_file(text.as_bytes()).unwrap(), font);
        assert_eq!(font.kern("L", "T"), -120.0);
        assert_eq!(font.kern("T", "L"), 0.0);
        assert!(font.glyph("O").and_then(|g| g.objects.first()).and_then(outline).is_some());
    }

    #[test]
    fn bad_fonts_are_refused_with_a_reason() {
        let bad = |f: &dyn Fn(&mut EmbroideryFont)| {
            let mut font = sample();
            f(&mut font);
            check(&font).unwrap_err()
        };
        assert!(bad(&|f| f.units_per_em = 0.0).contains("unitsPerEm"));
        assert!(bad(&|f| f.ascender = f64::NAN).contains("ascender"));
        assert!(bad(&|f| f.sizes = Some([40.0, 6.0])).contains("sizes"));
        assert!(bad(&|f| f.glyphs[1].chars = "L".into()).contains("two glyphs"));
        assert!(bad(&|f| f.glyphs[1].chars = String::new()).contains("characters"));
        assert!(bad(&|f| f.glyphs[1].advance = -1.0).contains("advance"));
        assert!(bad(&|f| f.glyphs[1].objects[0].path = "M 0 0 X".into()).contains("SVG path"));
        assert!(bad(&|f| f.glyphs[1].objects[0].path = "M 0 0 L 1e9 0".into()).contains("ten em"));
        assert!(bad(&|f| f.glyphs[1].objects[0].rule = Some("winding".into())).contains("rule"));
        assert!(bad(&|f| f.kerning[0].value = f64::INFINITY).contains("kerning"));
        assert!(bad(&|f| f.glyphs.clear()).contains("no glyphs"));
        // Not a font file at all.
        assert!(read_file(b"{}").is_err());
        assert!(read_file(br#"{"format": "vectorcraft-border-pattern", "version": 1}"#).unwrap_err().contains("format"));
        assert!(read_file(br#"{"format": "vectorcraft-embroidery-font", "version": 99}"#).unwrap_err().contains("version 99"));
    }

    #[test]
    fn a_name_alone_makes_the_id() {
        let mut font = sample();
        font.id = String::new();
        font.name = "My Script 2".into();
        let back = read_file(write_file(&font).unwrap().as_bytes()).unwrap();
        assert_eq!(back.id, "my-script-2");
    }

    #[test]
    fn unknown_members_survive_a_round_trip() {
        let mut v: serde_json::Value = serde_json::from_str(&write_file(&sample()).unwrap()).unwrap();
        v["futureThing"] = serde_json::json!({"a": 1});
        v["glyphs"][1]["futureGlyph"] = serde_json::json!(true);
        let font = read_file(v.to_string().as_bytes()).unwrap();
        let again: serde_json::Value = serde_json::from_str(&write_file(&font).unwrap()).unwrap();
        assert_eq!(again["futureThing"], serde_json::json!({"a": 1}));
        assert_eq!(again["glyphs"][1]["futureGlyph"], serde_json::json!(true));
    }

    proptest::proptest! {
        /// Whatever bytes come in, reading a font file returns, never panics.
        #[test]
        fn reading_junk_never_panics(bytes in proptest::collection::vec(proptest::num::u8::ANY, 0..2000)) {
            let _ = read_file(&bytes);
        }

        /// A real font file with one byte changed still only returns a font or an error.
        #[test]
        fn reading_a_damaged_font_never_panics(at in 0usize..4000, byte in proptest::num::u8::ANY) {
            let mut bytes = write_file(&sample()).unwrap().into_bytes();
            let i = at % bytes.len();
            bytes[i] = byte;
            if let Ok(f) = read_file(&bytes) {
                proptest::prop_assert!(check(&f).is_ok());
            }
        }
    }
}
