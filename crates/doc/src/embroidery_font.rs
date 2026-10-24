//! Embroidery fonts: lettering digitized for embroidery, each letter a set of stitch objects
//! (outlines with their stitch settings) rather than needle points, so letters scale and their
//! stitches are made at each size. Stored in `.vcfont` files (`vectorcraft_embroidery::fonts`,
//! specified in `docs/formats/vcfont.md`) and kept in the documents that use them.
//!
//! Outlines and metrics are in font units with y up from the baseline, as in other font formats;
//! stitch settings keep their own units (mm), so a letter sewn at any size keeps its density.

use serde::{Deserialize, Serialize};

use crate::StitchSettings;

/// An embroidery font.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EmbroideryFont {
    /// Lower case letters, digits and dashes.
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// Who made it, its licence (an SPDX id such as `OFL-1.1`, or text), where it came from (a URL,
    /// or the font it was digitized from: information only), and its version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The font's own version (`fontVersion` in files: `version` is the file format's).
    #[serde(rename = "fontVersion", skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Font units per em: the type size, set in points, is this many units.
    pub units_per_em: f64,
    /// Above the baseline (positive) and below it (negative), font units.
    pub ascender: f64,
    pub descender: f64,
    pub cap_height: f64,
    pub x_height: f64,
    /// The cap heights the font is made for, mm: `[smallest, largest]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sizes: Option<[f64; 2]>,
    pub glyphs: Vec<FontGlyph>,
    /// Pairs set closer (negative) or further apart (positive) than their advances say.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub kerning: Vec<KernPair>,
    /// Members a newer VectorCraft wrote that this version doesn't know, kept and written back.
    #[serde(flatten, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
    /// A font this version can't read, kept exactly as written; it isn't used.
    #[serde(skip)]
    pub unknown: Option<Box<serde_json::Value>>,
}

impl Default for EmbroideryFont {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            family: None,
            style: None,
            author: None,
            license: None,
            source: None,
            description: None,
            version: None,
            units_per_em: 1000.0,
            ascender: 800.0,
            descender: -200.0,
            cap_height: 700.0,
            x_height: 500.0,
            sizes: None,
            glyphs: vec![],
            kerning: vec![],
            extra: Default::default(),
            unknown: None,
        }
    }
}

impl EmbroideryFont {
    /// The glyph for `chars` (one character, or several for a ligature).
    pub fn glyph(&self, chars: &str) -> Option<&FontGlyph> {
        self.glyphs.iter().find(|g| g.chars == chars)
    }

    /// The kerning between `left` and `right`, font units (0 when none).
    pub fn kern(&self, left: &str, right: &str) -> f64 {
        self.kerning.iter().find(|k| k.left == left && k.right == right).map_or(0.0, |k| k.value)
    }
}

/// One letter (or ligature, or symbol) of an embroidery font.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FontGlyph {
    /// The character it sets, or the characters of a ligature.
    pub chars: String,
    /// How far the pen moves after it, font units.
    pub advance: f64,
    /// Its stitch objects, in sewing order.
    pub objects: Vec<GlyphObject>,
    /// Where the needle comes in and leaves, font units: letters of script fonts join here
    /// without a trim.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<[f64; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit: Option<[f64; 2]>,
    #[serde(flatten, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// One stitch object of a glyph: an outline, and how it is stitched.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GlyphObject {
    /// The outline as SVG path data (`M`, `L`, `C`, `Q`, `Z`…), font units, y up.
    pub path: String,
    /// Fill rule for the outline's subpaths: `nonzero` (default) or `evenodd`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    pub stitch: StitchSettings,
    #[serde(flatten, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// A kerning pair.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KernPair {
    pub left: String,
    pub right: String,
    /// Font units added to the space between them (negative: closer).
    pub value: f64,
}

/// Read a document's embroidery fonts, keeping any this version can't read as they were written.
pub fn de_fonts<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<EmbroideryFont>, D::Error> {
    let v = Vec::<serde_json::Value>::deserialize(d)?;
    Ok(v.into_iter()
        .map(|v| match EmbroideryFont::deserialize(&v) {
            Ok(f) => f,
            Err(_) => {
                let text = |k: &str| v.get(k).and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
                EmbroideryFont { id: text("id"), name: text("name"), unknown: Some(Box::new(v.clone())), ..EmbroideryFont::default() }
            }
        })
        .collect())
}

/// Write a document's embroidery fonts, those kept from a newer version as they were read.
pub fn ser_fonts<S: serde::Serializer>(v: &[EmbroideryFont], ser: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    let mut seq = ser.serialize_seq(Some(v.len()))?;
    for f in v {
        match &f.unknown {
            Some(raw) => seq.serialize_element(raw)?,
            None => seq.serialize_element(f)?,
        }
    }
    seq.end()
}
