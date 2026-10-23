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
    /// Chosen by the stitch and its width (see `vectorcraft_embroidery::generate::AutoUnderlay`):
    /// narrow satin a centre walk, medium an edge walk, wide an edge walk and a zigzag; fills an
    /// edge walk and fill underlay; satin shapes a centre or edge walk by stroke width. What new
    /// objects get.
    Auto,
    /// Satin: a running stitch along the centre line.
    Center,
    /// Satin: running stitches just inside both edges of the column. Satin shape: a running
    /// stitch just inside the shape's outline.
    Edge,
    /// Satin: a sparse zigzag inside the column.
    Zigzag,
    /// Fill: sparse rows across the top rows.
    Fill,
}

impl Underlay {
    pub const ALL: [Underlay; 6] = [Self::None, Self::Auto, Self::Center, Self::Edge, Self::Zigzag, Self::Fill];

    pub fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Auto => "auto",
            Self::Center => "center",
            Self::Edge => "edge",
            Self::Zigzag => "zigzag",
            Self::Fill => "fill",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Auto => "Auto",
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
            StitchKind::Satin => &[Self::None, Self::Auto, Self::Center, Self::Edge, Self::Zigzag],
            StitchKind::Fill => &[Self::None, Self::Auto, Self::Fill],
            StitchKind::SatinShape => &[Self::None, Self::Auto, Self::Center, Self::Edge],
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
    /// Pull compensation, mm: how much wider a satin is sewn than drawn, and how much longer a
    /// fill's rows (half at each end), since the thread pulls the fabric in along the stitches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull: Option<f64>,
    /// More pull compensation, as a percentage of the satin's width or the fill row's length,
    /// added to [`StitchSettings::pull`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_percent: Option<f64>,
    /// Push compensation, mm: how much shorter a satin column is sewn than drawn (half at each
    /// end), and a fill across its rows, since the stitches push the fabric out at their ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push: Option<f64>,
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
    /// Where the needle starts, mm in document space: running, bean and zigzag stitches enter
    /// their path at its point nearest it (a closed running path there, an open one from its
    /// nearer end). `None`: nearest where the needle is ([`EmbroiderySetup::auto_start`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_at: Option<[f64; 2]>,
    /// Puff (3D foam) embroidery: the foam's thickness, mm. A puffed satin is sewn after a stop
    /// for placing the foam, dense and with no underlay, its open ends capped, then perforated
    /// with short running stitches all round its edge so the excess foam tears away.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub puff: Option<f64>,
    /// Fill: keep the rows under objects sewn later on top of it (normally cut away, leaving
    /// [`EmbroiderySetup::overlap`]).
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub keep_covered: bool,
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
    /// How far a fill's rows still reach under an object sewn later on top of it, mm: the rest
    /// of the covered rows is cut away (overlap removal).
    #[serde(skip_serializing_if = "is_default_overlap")]
    pub overlap: f64,
    /// The shortest stitch sewn, mm: needle points closer than this to the one before are left
    /// out (manual stitches are sewn as drawn). A fabric may ask for longer
    /// ([`Fabric::min_stitch`]).
    #[serde(skip_serializing_if = "is_default_min_stitch")]
    pub min_stitch: f64,
    /// Running, bean and zigzag stitches without a start point of their own enter their paths
    /// nearest where the needle is (closed running paths at their nearest point, open paths from
    /// their nearer end, separate parts nearest first).
    #[serde(skip_serializing_if = "is_true")]
    pub auto_start: bool,
    /// The machine's sewing speed, stitches per minute (for the sewing time estimate).
    #[serde(skip_serializing_if = "is_default_speed")]
    pub speed: f64,
    /// The machine file format the design is exported in by default (`dst`, `pes`, `jef` or
    /// `exp`; see [`EmbroiderySetup::MACHINE_FORMATS`]). A format a newer VectorCraft named is kept
    /// as written, and DST is used in its place.
    #[serde(skip_serializing_if = "is_dst")]
    pub machine_format: String,
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
    pub sew_out: Option<Box<SewOut>>,
    /// The fabric the design is for: its settings fill in what objects leave unset (`None`: the
    /// stitches' own defaults).
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "de_fabric", serialize_with = "ser_fabric")]
    pub fabric: Option<Box<Fabric>>,
    /// The hoop the design is sewn in: its sewing field, centred on the design as machines centre
    /// it (`None`: no hoop chosen, nothing checked).
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "de_hoop", serialize_with = "ser_hoop")]
    pub hoop: Option<Box<Hoop>>,
    /// Thread charts loaded into the document (from `.vcthreads` files), kept with it.
    #[serde(skip_serializing_if = "Vec::is_empty", deserialize_with = "de_charts", serialize_with = "ser_charts")]
    pub charts: Vec<ThreadChart>,
    /// The chart the design's colours are matched against (an id of [`EmbroiderySetup::charts`];
    /// `None`: none).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart: Option<String>,
    /// Members a newer VectorCraft wrote that this version doesn't know, kept and written back.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for EmbroiderySetup {
    fn default() -> Self {
        Self {
            trim_jumps_over: Some(Self::DEFAULT_TRIM),
            stitch_moves_under: Self::DEFAULT_STITCH_MOVES,
            overlap: Self::DEFAULT_OVERLAP,
            min_stitch: Self::DEFAULT_MIN_STITCH,
            auto_start: true,
            speed: Self::DEFAULT_SPEED,
            machine_format: Self::DEFAULT_MACHINE_FORMAT.to_string(),
            order: vec![],
            patterns: vec![],
            sew_out: None,
            fabric: None,
            hoop: None,
            charts: vec![],
            chart: None,
            extra: Default::default(),
        }
    }
}

fn is_dst(v: &String) -> bool {
    v == EmbroiderySetup::DEFAULT_MACHINE_FORMAT
}

fn is_default_speed(v: &f64) -> bool {
    *v == EmbroiderySetup::DEFAULT_SPEED
}

fn is_true(v: &bool) -> bool {
    *v
}

fn is_default_min_stitch(v: &f64) -> bool {
    *v == EmbroiderySetup::DEFAULT_MIN_STITCH
}

fn is_default_overlap(v: &f64) -> bool {
    *v == EmbroiderySetup::DEFAULT_OVERLAP
}

/// A fabric profile: the settings a fabric wants (density, compensation, how much underlay), used
/// for whatever an object's own stitch settings leave unset. Each `None` keeps the stitch's own
/// default.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Fabric {
    /// Lower case letters, digits and dashes (`knit`).
    pub id: String,
    pub name: String,
    /// Who made it, its licence and where it came from (information only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Satin, zigzag-free satin shape spacing, mm ([`StitchSettings::spacing`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub satin_spacing: Option<f64>,
    /// Fill row spacing, mm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_spacing: Option<f64>,
    /// Pull compensation, mm and %, and push compensation, mm ([`StitchSettings::pull`]…).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pull: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pull_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push: Option<f64>,
    /// The auto underlay's thresholds, mm: satin columns narrower than `satinCenterUnder` get a
    /// centre walk, wider than `satinZigzagOver` an edge walk and a zigzag; satin shapes with
    /// strokes wider than `shapeEdgeOver` an edge walk.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub satin_center_under: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub satin_zigzag_over: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape_edge_over: Option<f64>,
    /// The shortest stitch this fabric takes, mm (the document's is used when it is longer).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_stitch: Option<f64>,
    /// Members a newer VectorCraft wrote that this version doesn't know, kept and written back.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
    /// A fabric this version can't read (a value of a type it doesn't expect), kept exactly as
    /// written so saving doesn't lose it. It isn't used: the stitches keep their own defaults.
    #[serde(skip)]
    pub unknown: Option<Box<serde_json::Value>>,
}

/// A thread chart: a brand's (or a person's) threads, each with its code, name and colour.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ThreadChart {
    /// Lower case letters, digits and dashes.
    pub id: String,
    pub name: String,
    /// The thread maker and line, and the thread's weight (information only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<String>,
    /// Who made the chart, its licence and where it came from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub threads: Vec<ChartThread>,
    /// Members a newer VectorCraft wrote that this version doesn't know, kept and written back.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
    /// A chart this version can't read, kept exactly as written; it isn't used.
    #[serde(skip)]
    pub unknown: Option<Box<serde_json::Value>>,
}

/// One thread of a chart.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChartThread {
    /// The maker's number for it (`1801`).
    pub code: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Its colour, `#rrggbb` (sRGB).
    pub color: String,
    /// Members a newer VectorCraft wrote that this version doesn't know, kept and written back.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

fn de_charts<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<ThreadChart>, D::Error> {
    let v = Vec::<serde_json::Value>::deserialize(d)?;
    Ok(v.into_iter()
        .map(|v| match ThreadChart::deserialize(&v) {
            Ok(c) => c,
            Err(_) => {
                let text = |k: &str| v.get(k).and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
                ThreadChart { id: text("id"), name: text("name"), unknown: Some(Box::new(v.clone())), ..ThreadChart::default() }
            }
        })
        .collect())
}

fn ser_charts<S: serde::Serializer>(v: &[ThreadChart], ser: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    let mut seq = ser.serialize_seq(Some(v.len()))?;
    for c in v {
        match &c.unknown {
            Some(raw) => seq.serialize_element(raw)?,
            None => seq.serialize_element(c)?,
        }
    }
    seq.end()
}

/// An embroidery hoop: the area a machine can sew in it (its sewing field), mm.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Hoop {
    /// Lower case letters, digits and dashes (`5x7`).
    pub id: String,
    pub name: String,
    /// The sewing field's width (across the machine) and height, mm.
    pub width: f64,
    pub height: f64,
    /// A round field: the largest circle in `width` × `height`.
    #[serde(skip_serializing_if = "crate::skip::is_default")]
    pub round: bool,
    /// Which machines take it, in words (information only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machines: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Members a newer VectorCraft wrote that this version doesn't know, kept and written back.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
    /// A hoop this version can't read, kept exactly as written; it isn't used.
    #[serde(skip)]
    pub unknown: Option<Box<serde_json::Value>>,
}

impl Hoop {
    /// Allowed sewing field sizes, mm.
    pub const SIZE: std::ops::RangeInclusive<f64> = 10.0..=2000.0;
}

fn de_hoop<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Box<Hoop>>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.map(|v| match Hoop::deserialize(&v) {
        Ok(h) => Box::new(h),
        Err(_) => Box::new(Hoop { unknown: Some(Box::new(v)), ..Hoop::default() }),
    }))
}

fn ser_hoop<S: serde::Serializer>(h: &Option<Box<Hoop>>, ser: S) -> Result<S::Ok, S::Error> {
    match h.as_deref() {
        Some(Hoop { unknown: Some(raw), .. }) => raw.serialize(ser),
        Some(h) => h.serialize(ser),
        None => ser.serialize_none(),
    }
}

/// Read a document's fabric: one this version can't read is kept as written in
/// [`Fabric::unknown`] instead of failing the document.
fn de_fabric<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Box<Fabric>>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.map(|v| match Fabric::deserialize(&v) {
        Ok(f) => Box::new(f),
        Err(_) => Box::new(Fabric { unknown: Some(Box::new(v)), ..Fabric::default() }),
    }))
}

/// Write a document's fabric: one kept from a newer version goes back as it was read.
fn ser_fabric<S: serde::Serializer>(f: &Option<Box<Fabric>>, ser: S) -> Result<S::Ok, S::Error> {
    match f.as_deref() {
        Some(Fabric { unknown: Some(raw), .. }) => raw.serialize(ser),
        Some(f) => f.serialize(ser),
        None => ser.serialize_none(),
    }
}

impl Fabric {
    /// Allowed auto underlay thresholds, mm.
    pub const THRESHOLD: std::ops::RangeInclusive<f64> = 0.3..=12.0;
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
    pub const DEFAULT_OVERLAP: f64 = 0.5;
    pub const DEFAULT_MIN_STITCH: f64 = 0.3;
    pub const DEFAULT_SPEED: f64 = 600.0;
    pub const DEFAULT_MACHINE_FORMAT: &str = "dst";
    /// The machine file formats a design can be exported in by default: Tajima DST, Brother PES,
    /// Janome JEF, Melco EXP.
    pub const MACHINE_FORMATS: [&str; 4] = ["dst", "pes", "jef", "exp"];
    /// Allowed machine speeds, stitches per minute.
    pub const SPEED: std::ops::RangeInclusive<f64> = 100.0..=1500.0;
    /// Allowed shortest stitches, mm.
    pub const MIN_STITCH: std::ops::RangeInclusive<f64> = 0.0..=2.0;
    /// Allowed overlaps, mm.
    pub const OVERLAP: std::ops::RangeInclusive<f64> = 0.0..=3.0;
    /// Allowed trim lengths, mm.
    pub const TRIM: std::ops::RangeInclusive<f64> = 0.5..=100.0;
    /// Allowed sewn-move lengths, mm (a sewn move is a stitch: at most a stitch long).
    pub const STITCH_MOVES: std::ops::RangeInclusive<f64> = 0.0..=5.0;

    /// The shortest stitch to sew, mm: the document's, or its fabric's when that is longer.
    pub fn min_stitch(&self) -> f64 {
        let clamp = |v: f64| v.clamp(*Self::MIN_STITCH.start(), *Self::MIN_STITCH.end());
        let own = if self.min_stitch.is_finite() { clamp(self.min_stitch) } else { Self::DEFAULT_MIN_STITCH };
        let fabric = self.fabric.as_ref().filter(|f| f.unknown.is_none()).and_then(|f| f.min_stitch).filter(|v| v.is_finite()).map(clamp);
        fabric.map_or(own, |f| own.max(f))
    }

    /// The machine format to export in: the document's, or DST for one this version doesn't know.
    pub fn machine_format(&self) -> &'static str {
        Self::MACHINE_FORMATS.iter().find(|f| **f == self.machine_format).copied().unwrap_or(Self::DEFAULT_MACHINE_FORMAT)
    }

    /// The machine speed to estimate with, stitches per minute.
    pub fn speed(&self) -> f64 {
        if self.speed.is_finite() { self.speed.clamp(*Self::SPEED.start(), *Self::SPEED.end()) } else { Self::DEFAULT_SPEED }
    }

    /// The overlap to sew with, mm.
    pub fn overlap(&self) -> f64 {
        if self.overlap.is_finite() { self.overlap.clamp(*Self::OVERLAP.start(), *Self::OVERLAP.end()) } else { Self::DEFAULT_OVERLAP }
    }

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
    /// Allowed foam thicknesses for puff, mm.
    pub const PUFF: std::ops::RangeInclusive<f64> = 1.0..=6.0;
    /// Satin spacing over foam unless set, mm (dense, so the foam doesn't show).
    pub const PUFF_SPACING: f64 = 0.25;
    /// Allowed pull compensation as a percentage of the width or row length.
    pub const PULL_PERCENT: std::ops::RangeInclusive<f64> = 0.0..=30.0;
    /// Allowed push compensation, mm.
    pub const PUSH: std::ops::RangeInclusive<f64> = 0.0..=3.0;
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
        let fallback = if self.kind == StitchKind::Zigzag {
            Self::DEFAULT_ZIGZAG_SPACING
        } else if self.puffed() {
            Self::PUFF_SPACING
        } else {
            Self::DEFAULT_SPACING
        };
        clamp(self.spacing, fallback, Self::SPACING)
    }

    /// The foam's thickness when puffed, mm (always within [`Self::PUFF`]).
    pub fn puff(&self) -> Option<f64> {
        self.puff.filter(|v| v.is_finite()).map(|v| v.clamp(*Self::PUFF.start(), *Self::PUFF.end()))
    }

    /// Whether this is sewn over foam: a puffed satin or satin shape.
    pub fn puffed(&self) -> bool {
        self.puff().is_some() && matches!(self.kind, StitchKind::Satin | StitchKind::SatinShape)
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

    /// The pull compensation as a percentage of the width or row length.
    pub fn pull_percent(&self) -> f64 {
        clamp(self.pull_percent, 0.0, Self::PULL_PERCENT)
    }

    /// All the pull compensation for a satin `width` mm wide (or a fill row that long), mm.
    pub fn pull_for(&self, width: f64) -> f64 {
        let w = if width.is_finite() { width.max(0.0) } else { 0.0 };
        self.pull() + w * self.pull_percent() / 100.0
    }

    /// The push compensation to sew with, mm.
    pub fn push(&self) -> f64 {
        clamp(self.push, 0.0, Self::PUSH)
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
        // Over foam there is no underlay: it would flatten the foam.
        if self.puffed() {
            return Underlay::None;
        }
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
    fn a_fabric_this_version_cant_read_is_kept() {
        let setup = serde_json::json!({"trimJumpsOver": 3.0, "stitchMovesUnder": 1.0, "fabric": {"name": "Silk", "pull": "lots"}});
        let e: EmbroiderySetup = serde_json::from_value(setup.clone()).expect("a strange fabric doesn't fail the document");
        assert!(e.fabric.as_ref().is_some_and(|f| f.unknown.is_some()));
        assert_eq!(serde_json::to_value(&e).unwrap(), setup, "written back as read");
        let ok = serde_json::json!({"trimJumpsOver": 3.0, "stitchMovesUnder": 1.0, "fabric": {"id": "knit", "name": "Knit", "pull": 0.3}});
        let e: EmbroiderySetup = serde_json::from_value(ok.clone()).unwrap();
        assert_eq!(e.fabric.as_ref().and_then(|f| f.pull), Some(0.3));
        assert_eq!(serde_json::to_value(&e).unwrap(), ok);
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
