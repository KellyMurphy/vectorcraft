//! Embroidery: the stitch settings an object carries ([`crate::Node::stitch`]). The stitches
//! themselves are generated from them by `vectorcraft-embroidery`; nothing here draws or prints.
//!
//! Lengths are millimetres, the unit embroidery machines and their files work in.

use serde::{Deserialize, Serialize};

/// How an object is stitched.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StitchKind {
    /// Single stitches along the path.
    #[default]
    Running,
    /// Running stitch sewn three times over each stitch (a bolder line).
    Bean,
    /// A zigzag column centred on the path, as wide as the stroke (or [`StitchSettings::width`]).
    Satin,
    /// An open zigzag along the path: points [`StitchSettings::spacing`] apart, alternating
    /// side to side across [`StitchSettings::width`] (decorative lines, appliqué edges).
    Zigzag,
    /// Satin following a closed shape's strokes, from edge to edge (lettering, outlined type).
    #[serde(rename = "satinShape")]
    SatinShape,
    /// Parallel rows of staggered running stitches covering the shape (tatami fill).
    Fill,
    /// The needle goes down at every anchor, as drawn (imported designs).
    Manual,
    /// A border pattern ([`StitchSettings::pattern`]) repeated along the path, bending with its
    /// curves, a whole number of repeats between corners.
    #[serde(rename = "patternBorder")]
    PatternBorder,
    /// Chosen by the shape's stroke width ([`StitchSettings::thin_under`],
    /// [`StitchSettings::wide_over`]): thin → bean outline, medium → satin shape, wide → fill
    /// with a satin border (lettering: per letter).
    Auto,
}

impl StitchKind {
    pub const ALL: [StitchKind; 9] =
        [Self::Auto, Self::Running, Self::Bean, Self::Satin, Self::Zigzag, Self::SatinShape, Self::Fill, Self::PatternBorder, Self::Manual];

    pub fn id(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Bean => "bean",
            Self::Satin => "satin",
            Self::Fill => "fill",
            Self::Zigzag => "zigzag",
            Self::SatinShape => "satinShape",
            Self::Manual => "manual",
            Self::PatternBorder => "patternBorder",
            Self::Auto => "auto",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "Running Stitch",
            Self::Bean => "Bean Stitch",
            Self::Satin => "Satin Column",
            Self::Fill => "Fill",
            Self::Zigzag => "Zigzag Stitch",
            Self::SatinShape => "Satin Shape",
            Self::Manual => "Manual Stitch",
            Self::PatternBorder => "Pattern Border",
            Self::Auto => "Auto",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id().eq_ignore_ascii_case(s.trim()))
    }

    /// Stitches along the path (rather than covering its area): the thread defaults to the stroke.
    pub fn follows_path(self) -> bool {
        !matches!(self, Self::Fill | Self::SatinShape | Self::Auto)
    }

    /// The default stitch length, mm.
    pub fn default_length(self) -> f64 {
        match self {
            Self::Fill => 4.0,
            // Border patterns are small, curvy figures.
            Self::PatternBorder => 1.5,
            _ => 2.5,
        }
    }
}

/// Stitches sewn under the top stitches to hold the fabric and lift the top stitches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Underlay {
    #[default]
    None,
    /// Satin: a running stitch along the centre line.
    Center,
    /// Satin: running stitches just inside both edges of the column.
    Edge,
    /// Satin: a sparse zigzag inside the column.
    Zigzag,
    /// Fill: sparse rows across the top rows.
    Fill,
}

impl Underlay {
    pub const ALL: [Underlay; 5] = [Self::None, Self::Center, Self::Edge, Self::Zigzag, Self::Fill];

    pub fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Center => "center",
            Self::Edge => "edge",
            Self::Zigzag => "zigzag",
            Self::Fill => "fill",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Center => "Center Walk",
            Self::Edge => "Edge Walk",
            Self::Zigzag => "Zigzag",
            Self::Fill => "Fill Underlay",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|u| u.id().eq_ignore_ascii_case(s.trim()))
    }

    /// The underlays a stitch kind sews (the first is none).
    pub fn for_kind(kind: StitchKind) -> &'static [Underlay] {
        match kind {
            StitchKind::Satin => &[Self::None, Self::Center, Self::Edge, Self::Zigzag],
            StitchKind::Fill => &[Self::None, Self::Fill],
            StitchKind::SatinShape => &[Self::None, Self::Center],
            _ => &[Self::None],
        }
    }
}

/// An object's stitch settings. `None` fields take the kind's defaults.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StitchSettings {
    pub kind: StitchKind,
    /// Stitch length, mm (running, bean and fill; [`StitchKind::default_length`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<f64>,
    /// Distance between zigzags (satin) or rows (fill), mm ([`StitchSettings::DEFAULT_SPACING`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spacing: Option<f64>,
    /// Fill row direction, degrees counter-clockwise from horizontal.
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub angle: f64,
    /// Satin column width, mm (`None`: the stroke width).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// Thread colour as sRGB (`None`: the object's stroke for path stitches, its fill for fills).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<[u8; 3]>,
    /// Satin pull compensation, mm: how much wider the column is sewn than drawn, since the
    /// thread pulls the fabric in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull: Option<f64>,
    /// Underlay sewn first ([`Underlay::for_kind`] lists the ones each kind uses; others are
    /// ignored).
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub underlay: Underlay,
    /// Fill pattern: rows before the needle holes line up again (2: brick, 3: tatami…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stagger: Option<u8>,
    /// Fill: a motif carved into the fill (see `vectorcraft_embroidery::motifs`: diamonds,
    /// waves, circles, scales, herringbone), its needle points falling on the motif's lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motif: Option<String>,
    /// The carved motif's repeat, mm ([`StitchSettings::DEFAULT_MOTIF_SIZE`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motif_size: Option<f64>,
    /// Pattern border: the border pattern's id (`None`: [`StitchSettings::DEFAULT_PATTERN`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Pattern border: the pattern mirrored across the path.
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub flip: bool,
    /// Bean stitch: how many times each stitch is sewn (odd).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passes: Option<u8>,
    /// Auto: shapes whose strokes are thinner than this (mm) get a bean outline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thin_under: Option<f64>,
    /// Auto: shapes whose strokes are wider than this (mm) are filled, with a satin border.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wide_over: Option<f64>,
    /// Fill: a satin border this wide (mm) sewn over the shape's edges after the fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<f64>,
    /// Satin: the object's two longest subpaths are the column's edges (rails), the column as wide
    /// as they are apart; other subpaths crossing both are rungs, setting the stitch direction.
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub rails: bool,
    /// Cut the thread before the object starts, however close it is to the stitch before.
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub trim_before: bool,
    /// Cut the thread after the object ends.
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub trim_after: bool,
    /// Members a newer VectorCraft wrote that this version doesn't know, kept and written back.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
    /// Settings this version can't read (a stitch type, underlay or value from a newer
    /// VectorCraft), kept exactly as written so saving doesn't lose them. Such an object isn't
    /// sewn; the other fields are left at their defaults. See [`de_stitch`].
    #[serde(skip)]
    pub unknown: Option<Box<serde_json::Value>>,
}

/// Read a node's `stitch`: settings this version can't read are kept as written in
/// [`StitchSettings::unknown`] instead of failing the whole document.
pub fn de_stitch<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Box<StitchSettings>>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.map(|v| match StitchSettings::deserialize(&v) {
        Ok(s) => Box::new(s),
        Err(_) => Box::new(StitchSettings { unknown: Some(Box::new(v)), ..StitchSettings::default() }),
    }))
}

/// Write a node's `stitch`: settings kept from a newer version go back as they were read.
pub fn ser_stitch<S: serde::Serializer>(s: &Option<Box<StitchSettings>>, ser: S) -> Result<S::Ok, S::Error> {
    match s.as_deref() {
        Some(StitchSettings { unknown: Some(raw), .. }) => raw.serialize(ser),
        Some(st) => st.serialize(ser),
        None => ser.serialize_none(),
    }
}

/// Read a document's border patterns: one this version can't read (a part stitch from a newer
/// VectorCraft) is kept as written in [`BorderPattern::unknown`], with its id and name.
pub fn de_patterns<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<BorderPattern>, D::Error> {
    let v = Vec::<serde_json::Value>::deserialize(d)?;
    Ok(v.into_iter()
        .map(|v| match BorderPattern::deserialize(&v) {
            Ok(p) => p,
            Err(_) => {
                let text = |k: &str| v.get(k).and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
                BorderPattern { id: text("id"), name: text("name"), unknown: Some(Box::new(v.clone())), ..BorderPattern::default() }
            }
        })
        .collect())
}

/// Write a document's border patterns, those kept from a newer version as they were read.
pub fn ser_patterns<S: serde::Serializer>(v: &[BorderPattern], ser: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    let mut seq = ser.serialize_seq(Some(v.len()))?;
    for p in v {
        match &p.unknown {
            Some(raw) => seq.serialize_element(raw)?,
            None => seq.serialize_element(p)?,
        }
    }
    seq.end()
}

/// How a [`PatternPart`] is sewn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PartStitch {
    /// Running stitch along the part's line (the object's stitch length).
    #[default]
    Running,
    /// Bean stitch (each stitch sewn three times) along the line.
    Bean,
    /// A satin column [`PatternPart::width`] wide along the line.
    Satin,
    /// The needle goes down at each of the part's points, as given (patterns taken from a
    /// stitched design).
    Manual,
}

/// One piece of a border pattern: a line in the pattern's own space (mm at its own size: x along
/// the border, 0 to [`BorderPattern::length`]; y across it, from −width/2 to width/2, positive
/// to the right of the direction of travel on the page, y being down), and how it is sewn.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternPart {
    #[serde(default)]
    pub stitch: PartStitch,
    pub points: Vec<[f64; 2]>,
    /// Satin: the column's width, mm at the pattern's size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// Members a newer VectorCraft wrote that this version doesn't know, kept and written back.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// A border pattern: one repeat of a stitched border, sewn end to end along a path by
/// [`StitchKind::PatternBorder`]. The repeat's parts are sewn in order as one continuous line
/// (each part starts where the one before ends), entering at its first point and leaving at its
/// last, which is the next repeat's first point moved `length` along.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BorderPattern {
    /// What [`StitchSettings::pattern`] calls it (lower case letters, digits and dashes).
    #[serde(default)]
    pub id: String,
    pub name: String,
    /// Who made it, its licence (an SPDX id such as `CC0-1.0`, or text) and where it came from
    /// (a URL, or the design it was taken from). Information only: nothing is sewn differently.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The band's width at the pattern's own size, mm.
    pub width: f64,
    /// One repeat's length at the pattern's own size, mm.
    pub length: f64,
    pub repeat: Vec<PatternPart>,
    /// A motif sewn at each sharp corner, centred on it (x along the direction of travel through
    /// the corner, halfway between in and out; y across it), starting and ending at the corner. Empty: the border ends at each corner and
    /// starts again after it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corner: Vec<PatternPart>,
    /// Members a newer VectorCraft wrote that this version doesn't know, kept and written back.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
    /// A pattern from a newer VectorCraft this version can't read, kept as written (see
    /// [`de_patterns`]). It isn't sewn.
    #[serde(skip)]
    pub unknown: Option<Box<serde_json::Value>>,
}

/// The document's embroidery settings: how the needle travels between stitches
/// ([`crate::Document::embroidery`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EmbroiderySetup {
    /// Jumps longer than this cut the thread first, mm (`None`: only thread changes and objects'
    /// own Trim Before / Trim After cut it).
    pub trim_jumps_over: Option<f64>,
    /// Moves to the next stitch up to this long are sewn as a stitch rather than jumped, mm.
    pub stitch_moves_under: f64,
    /// The order objects are sewn in: these first, in this order, then any other stitched objects
    /// in paint order (empty: paint order). Ids of objects no longer there are ignored.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<crate::NodeId>,
    /// Border patterns loaded into the document (from `.vcborder` files), kept with it so it sews
    /// the same anywhere; [`StitchSettings::pattern`] names them by id.
    #[serde(skip_serializing_if = "Vec::is_empty", deserialize_with = "de_patterns", serialize_with = "ser_patterns")]
    pub patterns: Vec<BorderPattern>,
    /// A sew-out test: the swatches of a test sheet (`embroidery.testSheet`) and what was seen
    /// when they were sewn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sew_out: Option<SewOut>,
    /// Members a newer VectorCraft wrote that this version doesn't know, kept and written back.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for EmbroiderySetup {
    fn default() -> Self {
        Self {
            trim_jumps_over: Some(Self::DEFAULT_TRIM),
            stitch_moves_under: Self::DEFAULT_STITCH_MOVES,
            order: vec![],
            patterns: vec![],
            sew_out: None,
            extra: Default::default(),
        }
    }
}

/// A sew-out test record: the swatches of a test sheet, the fabric and machine they were sewn
/// on, and what was seen.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SewOut {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub fabric: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub machine: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub notes: String,
    pub swatches: Vec<SewSwatch>,
    /// Members a newer VectorCraft wrote that this version doesn't know, kept and written back.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// One swatch of a test sheet: the objects it is sewn from and the result seen.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SewSwatch {
    /// Its name on the sheet: the section's letter and its number (`A3`).
    pub id: String,
    /// The section's id (`satinWidth`…).
    pub section: String,
    /// What it tests, in words (`Satin 3 mm wide`).
    pub label: String,
    /// The objects it is sewn from (its label included).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub objects: Vec<crate::NodeId>,
    /// `pass`, `fail`, or empty: not judged yet ([`SewSwatch::result`]). Kept as text so a value a
    /// newer VectorCraft writes survives a save.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub result: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub notes: String,
    /// Members a newer VectorCraft wrote that this version doesn't know, kept and written back.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// What was seen when a swatch was sewn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SewResult {
    /// Not judged yet (or a result this version doesn't know).
    Untested,
    Pass,
    Fail,
}

impl SewResult {
    pub const ALL: [SewResult; 3] = [Self::Untested, Self::Pass, Self::Fail];

    pub fn id(self) -> &'static str {
        match self {
            Self::Untested => "",
            Self::Pass => "pass",
            Self::Fail => "fail",
        }
    }

    /// `pass`, `fail`, or `untested` / empty.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "untested" => Some(Self::Untested),
            "pass" => Some(Self::Pass),
            "fail" => Some(Self::Fail),
            _ => None,
        }
    }
}

impl SewSwatch {
    pub fn result(&self) -> SewResult {
        SewResult::parse(&self.result).unwrap_or(SewResult::Untested)
    }
}

impl EmbroiderySetup {
    pub const DEFAULT_TRIM: f64 = 3.0;
    pub const DEFAULT_STITCH_MOVES: f64 = 1.0;
    /// Allowed trim lengths, mm.
    pub const TRIM: std::ops::RangeInclusive<f64> = 0.5..=100.0;
    /// Allowed sewn-move lengths, mm (a sewn move is a stitch: at most a stitch long).
    pub const STITCH_MOVES: std::ops::RangeInclusive<f64> = 0.0..=5.0;

    /// The jump length past which the thread is cut, mm (`None`: never on length).
    pub fn trim_over(&self) -> Option<f64> {
        self.trim_jumps_over.filter(|v| v.is_finite()).map(|v| v.clamp(*Self::TRIM.start(), *Self::TRIM.end()))
    }

    /// The longest move sewn rather than jumped, mm (never past the trim length).
    pub fn stitch_under(&self) -> f64 {
        let v = if self.stitch_moves_under.is_finite() { self.stitch_moves_under } else { Self::DEFAULT_STITCH_MOVES };
        let v = v.clamp(*Self::STITCH_MOVES.start(), *Self::STITCH_MOVES.end());
        self.trim_over().map_or(v, |t| v.min(t))
    }
}

impl StitchSettings {
    /// Allowed stitch lengths, mm: below this range the needle cuts the fabric; above it the
    /// stitch snags (and DST can't move further than 12.1 mm in one record).
    pub const LENGTH: std::ops::RangeInclusive<f64> = 0.3..=12.0;
    /// Allowed satin and fill spacing, mm.
    pub const SPACING: std::ops::RangeInclusive<f64> = 0.1..=10.0;
    /// Allowed satin widths, mm.
    pub const WIDTH: std::ops::RangeInclusive<f64> = 0.3..=12.0;
    pub const DEFAULT_SPACING: f64 = 0.4;
    /// Allowed Auto thresholds, mm.
    pub const THIN: std::ops::RangeInclusive<f64> = 0.2..=3.0;
    pub const WIDE: std::ops::RangeInclusive<f64> = 2.0..=12.0;
    pub const DEFAULT_THIN: f64 = 1.0;
    pub const DEFAULT_WIDE: f64 = 7.0;
    /// The border Auto gives wide shapes when none is set, mm.
    pub const DEFAULT_BORDER: f64 = 2.0;
    /// Allowed carved motif repeats, mm.
    pub const MOTIF_SIZE: std::ops::RangeInclusive<f64> = 3.0..=60.0;
    pub const DEFAULT_MOTIF_SIZE: f64 = 10.0;
    /// Allowed fill border widths, mm.
    pub const BORDER: std::ops::RangeInclusive<f64> = 0.5..=6.0;
    /// Allowed pull compensation, mm.
    pub const PULL: std::ops::RangeInclusive<f64> = 0.0..=3.0;
    /// Allowed fill stagger, rows.
    pub const STAGGER: std::ops::RangeInclusive<u8> = 2..=8;
    pub const DEFAULT_STAGGER: u8 = 3;
    /// Allowed bean passes (odd numbers in this range).
    pub const PASSES: std::ops::RangeInclusive<u8> = 3..=9;
    pub const DEFAULT_PASSES: u8 = 3;
    /// Satin width when the object has no stroke and no width is set, mm.
    pub const DEFAULT_WIDTH: f64 = 3.0;
    /// Zigzag stitch spacing when none is set, mm.
    pub const DEFAULT_ZIGZAG_SPACING: f64 = 2.0;
    /// Allowed pattern border widths, mm (the motif's stitches are split where they'd be longer
    /// than a machine sews).
    pub const PATTERN_WIDTH: std::ops::RangeInclusive<f64> = 1.0..=40.0;
    /// The border pattern sewn when none is set.
    pub const DEFAULT_PATTERN: &'static str = "waves";

    pub fn new(kind: StitchKind) -> Self {
        Self { kind, ..Self::default() }
    }

    /// The stitch length to sew with, mm (always within [`Self::LENGTH`]).
    pub fn length(&self) -> f64 {
        clamp(self.length, self.kind.default_length(), Self::LENGTH)
    }

    /// The satin, zigzag or fill spacing to sew with, mm (always within [`Self::SPACING`]).
    pub fn spacing(&self) -> f64 {
        let fallback = if self.kind == StitchKind::Zigzag { Self::DEFAULT_ZIGZAG_SPACING } else { Self::DEFAULT_SPACING };
        clamp(self.spacing, fallback, Self::SPACING)
    }

    /// The satin or zigzag width, mm, given the stroke width in mm (always within
    /// [`Self::WIDTH`]). A satin defaults to the stroke width; a zigzag, drawn on thin lines,
    /// to [`Self::DEFAULT_WIDTH`].
    pub fn width(&self, stroke_mm: f64) -> f64 {
        let stroke = stroke_mm.is_finite() && stroke_mm > 0.0 && self.kind != StitchKind::Zigzag;
        let fallback = if stroke { stroke_mm } else { Self::DEFAULT_WIDTH };
        clamp(self.width, fallback, Self::WIDTH)
    }

    /// The stitch type's id, or `unknown` for settings from a newer version this one can't read.
    pub fn kind_id(&self) -> &'static str {
        if self.unknown.is_some() { "unknown" } else { self.kind.id() }
    }

    /// The pattern border's width, mm, given the pattern's own (always within
    /// [`Self::PATTERN_WIDTH`]).
    pub fn pattern_width(&self, nominal: f64) -> f64 {
        let fallback = if nominal.is_finite() && nominal > 0.0 { nominal } else { Self::DEFAULT_WIDTH };
        clamp(self.width, fallback, Self::PATTERN_WIDTH)
    }

    /// The border pattern's id to sew.
    pub fn pattern(&self) -> &str {
        self.pattern.as_deref().unwrap_or(Self::DEFAULT_PATTERN)
    }

    /// Auto's thresholds, mm: (thin under, wide over), the wide one never below the thin one.
    pub fn auto_thresholds(&self) -> (f64, f64) {
        let thin = clamp(self.thin_under, Self::DEFAULT_THIN, Self::THIN);
        (thin, clamp(self.wide_over, Self::DEFAULT_WIDE, Self::WIDE).max(thin))
    }

    /// The carved motif's repeat to sew with, mm.
    pub fn motif_size(&self) -> f64 {
        clamp(self.motif_size, Self::DEFAULT_MOTIF_SIZE, Self::MOTIF_SIZE)
    }

    /// The fill border width to sew, mm (none: no border).
    pub fn border(&self) -> Option<f64> {
        self.border.filter(|b| b.is_finite()).map(|b| b.clamp(*Self::BORDER.start(), *Self::BORDER.end()))
    }

    /// The pull compensation to sew with, mm.
    pub fn pull(&self) -> f64 {
        clamp(self.pull, 0.0, Self::PULL)
    }

    /// The fill stagger to sew with, rows.
    pub fn stagger(&self) -> u8 {
        self.stagger.unwrap_or(Self::DEFAULT_STAGGER).clamp(*Self::STAGGER.start(), *Self::STAGGER.end())
    }

    /// The bean passes to sew with (odd).
    pub fn passes(&self) -> u8 {
        let p = self.passes.unwrap_or(Self::DEFAULT_PASSES).clamp(*Self::PASSES.start(), *Self::PASSES.end());
        if p % 2 == 0 { p - 1 } else { p }
    }

    /// The underlay to sew with: none when the kind doesn't use this one.
    pub fn underlay(&self) -> Underlay {
        if Underlay::for_kind(self.kind).contains(&self.underlay) { self.underlay } else { Underlay::None }
    }

    /// The fill angle in degrees (finite).
    pub fn angle(&self) -> f64 {
        if self.angle.is_finite() { self.angle % 360.0 } else { 0.0 }
    }
}

/// `v` (or `fallback`) inside `range`; a non-finite value takes `fallback`.
fn clamp(v: Option<f64>, fallback: f64, range: std::ops::RangeInclusive<f64>) -> f64 {
    let v = v.filter(|v| v.is_finite()).unwrap_or(fallback);
    v.clamp(*range.start(), *range.end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_omit_defaults() {
        let s = StitchSettings::new(StitchKind::Fill);
        assert_eq!(serde_json::to_value(&s).unwrap(), serde_json::json!({"kind": "fill"}));
        let s = StitchSettings { kind: StitchKind::Satin, width: Some(4.0), thread: Some([1, 2, 3]), ..Default::default() };
        let back: StitchSettings = serde_json::from_value(serde_json::to_value(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn values_are_clamped_and_finite() {
        let s = StitchSettings { length: Some(f64::NAN), spacing: Some(100.0), width: Some(0.0), angle: f64::INFINITY, ..Default::default() };
        assert_eq!(s.length(), 2.5);
        assert_eq!(s.spacing(), 10.0);
        assert_eq!(s.width(2.0), 0.3);
        assert_eq!(s.angle(), 0.0);
        assert_eq!(StitchSettings::default().width(f64::NAN), StitchSettings::DEFAULT_WIDTH);
    }

    #[test]
    fn extra_settings_default_clamp_and_apply_by_kind() {
        let mut s = StitchSettings::new(StitchKind::Satin);
        assert_eq!((s.pull(), s.stagger(), s.passes(), s.underlay()), (0.0, 3, 3, Underlay::None));
        s.underlay = Underlay::Zigzag;
        s.pull = Some(9.0);
        s.passes = Some(4);
        s.stagger = Some(1);
        assert_eq!((s.pull(), s.stagger(), s.passes(), s.underlay()), (3.0, 2, 3, Underlay::Zigzag));
        s.kind = StitchKind::Fill;
        assert_eq!(s.underlay(), Underlay::None, "zigzag is a satin underlay");
        for u in Underlay::ALL {
            assert_eq!(Underlay::parse(u.id()), Some(u));
        }
    }

    #[test]
    fn setup_defaults_round_trip_and_clamp() {
        let s = EmbroiderySetup::default();
        assert_eq!((s.trim_over(), s.stitch_under()), (Some(3.0), 1.0));
        let never = EmbroiderySetup { trim_jumps_over: None, stitch_moves_under: 9.0, order: vec![crate::NodeId(7)], ..Default::default() };
        let back: EmbroiderySetup = serde_json::from_value(serde_json::to_value(&never).unwrap()).unwrap();
        assert_eq!(back, never);
        assert_eq!((never.trim_over(), never.stitch_under()), (None, 5.0));
        let tight = EmbroiderySetup { trim_jumps_over: Some(0.6), stitch_moves_under: 2.0, ..Default::default() };
        assert_eq!(tight.stitch_under(), 0.6, "a sewn move is never longer than a trimmed jump");
        let missing: EmbroiderySetup = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(missing, EmbroiderySetup::default());
    }

    /// A node as a newer VectorCraft might write it: a stitch kind, an underlay and a member this
    /// version doesn't know.
    fn newer_node() -> serde_json::Value {
        serde_json::json!({
            "id": 7,
            "kind": {"type": "path", "path": {"subpaths": []}},
            "stitch": {"kind": "crossStitch", "underlay": "lattice", "cellSize": 2.5, "length": 3.0}
        })
    }

    #[test]
    fn settings_from_a_newer_version_load_and_save_unchanged() {
        let n: crate::Node = serde_json::from_value(newer_node()).expect("a newer stitch kind doesn't fail the node");
        let s = n.extras().stitch.as_deref().unwrap();
        assert!(s.unknown.is_some());
        assert_eq!(s.kind_id(), "unknown");
        let back = serde_json::to_value(&n).unwrap();
        assert_eq!(back["stitch"], newer_node()["stitch"], "written back exactly as read");
        // Numbers too big for their field are kept the same way.
        let mut big = newer_node();
        big["stitch"] = serde_json::json!({"kind": "satin", "thread": [300, 0, 0]});
        let n: crate::Node = serde_json::from_value(big.clone()).unwrap();
        assert_eq!(serde_json::to_value(&n).unwrap()["stitch"], big["stitch"]);
    }

    #[test]
    fn unknown_members_are_kept_and_known_ones_still_read() {
        let v = serde_json::json!({"kind": "satin", "width": 4.0, "fabricProfile": "knit", "pushComp": {"x": 0.2}});
        let s: StitchSettings = serde_json::from_value(v.clone()).unwrap();
        assert_eq!((s.kind, s.width, s.unknown.is_none()), (StitchKind::Satin, Some(4.0), true));
        assert_eq!(serde_json::to_value(&s).unwrap(), v, "the newer members survive a save");
        let setup = serde_json::json!({"trimJumpsOver": 2.0, "stitchMovesUnder": 1.0, "fabric": "towel"});
        let e: EmbroiderySetup = serde_json::from_value(setup.clone()).unwrap();
        assert_eq!(serde_json::to_value(&e).unwrap(), setup);
    }

    #[test]
    fn patterns_from_a_newer_version_are_kept() {
        let setup = serde_json::json!({
            "trimJumpsOver": 3.0, "stitchMovesUnder": 1.0,
            "patterns": [
                {"id": "rope", "name": "Rope", "width": 6.0, "length": 8.0, "repeat": [{"stitch": "bean", "points": [[0.0, 0.0], [8.0, 0.0]]}]},
                {"id": "lace", "name": "Lace", "width": 6.0, "length": 8.0, "repeat": [{"stitch": "tripleSatin", "points": [[0.0, 0.0]]}]}
            ]
        });
        let e: EmbroiderySetup = serde_json::from_value(setup.clone()).unwrap();
        assert_eq!(e.patterns.len(), 2);
        assert!(e.patterns[0].unknown.is_none() && e.patterns[1].unknown.is_some());
        assert_eq!((e.patterns[1].id.as_str(), e.patterns[1].name.as_str()), ("lace", "Lace"));
        assert_eq!(serde_json::to_value(&e).unwrap(), setup);
    }

    #[test]
    fn sew_outs_round_trip_and_keep_newer_results() {
        let setup = serde_json::json!({
            "trimJumpsOver": 3.0, "stitchMovesUnder": 1.0,
            "sewOut": {
                "fabric": "denim", "machine": "home",
                "swatches": [
                    {"id": "A1", "section": "satinWidth", "label": "Satin 1 mm wide", "objects": [4, 5], "result": "pass"},
                    {"id": "A2", "section": "satinWidth", "label": "Satin 2 mm wide", "result": "puckered", "notes": "loops", "photo": "a2.jpg"}
                ]
            }
        });
        let e: EmbroiderySetup = serde_json::from_value(setup.clone()).unwrap();
        let s = e.sew_out.as_ref().unwrap();
        assert_eq!(s.swatches[0].result(), SewResult::Pass);
        assert_eq!(s.swatches[1].result(), SewResult::Untested, "a result this version doesn't know");
        assert_eq!(serde_json::to_value(&e).unwrap(), setup, "written back as read");
        for r in SewResult::ALL {
            assert_eq!(SewResult::parse(r.id()), Some(r));
        }
    }

    #[test]
    fn kinds_parse_by_id() {
        for k in StitchKind::ALL {
            assert_eq!(StitchKind::parse(k.id()), Some(k));
        }
        assert_eq!(StitchKind::parse(" Satin "), Some(StitchKind::Satin));
        assert_eq!(StitchKind::parse("cross"), None);
    }
}
