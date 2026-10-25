//! The stitch settings, described once: each one's command key, value and range, the stitch
//! types it applies to, its panel label and tooltip, and how it is read and written. The engine
//! parses `embroidery.set` and reports `embroidery.info` from this table, its parameter
//! descriptions are made from it, and the Embroidery panel draws a row for any setting a stitch
//! type uses that it has no row of its own for. A new setting is one entry here (plus the field on
//! [`StitchSettings`]).

use std::ops::RangeInclusive;

use serde_json::{Value, json};
use vectorcraft_doc::{StitchKind, StitchSettings, Underlay};

/// What a setting holds, for parsing, the panel's generic row and the parameter description.
#[derive(Clone, Debug)]
pub enum ValueKind {
    /// The stitch type (`type`).
    Kind,
    /// A length in mm within the range.
    Mm(RangeInclusive<f64>),
    /// A percentage within the range.
    Percent(RangeInclusive<f64>),
    /// Degrees (any finite number).
    Degrees,
    /// A whole number within the range (only odd ones when `odd`).
    Whole { range: RangeInclusive<u8>, odd: bool },
    /// True or false.
    Flag,
    /// One of these ids.
    Choice(&'static [&'static str]),
    /// An id checked elsewhere (a border pattern: the document's or a starter).
    Id,
    /// A thread colour: `"#rrggbb"` or `[r, g, b]`.
    Thread,
    /// A point, `[x, y]` mm.
    Point,
    /// Lines, `[[x1, y1, x2, y2], …]` mm (at most [`StitchSettings::MAX_ANGLES`]).
    Lines,
}

/// Whether a setting suits the settings it was given for; the error says why not.
pub type Suits = fn(&StitchSettings) -> Result<(), String>;

/// One stitch setting.
pub struct Param {
    /// The key in `embroidery.set`, `embroidery.info` and the document (`type` is the document's
    /// `kind`).
    pub key: &'static str,
    pub value: ValueKind,
    /// The stitch types it applies to (empty: every type).
    pub kinds: &'static [StitchKind],
    /// The panel's label (with its colon) and tooltip; both are translated.
    pub label: &'static str,
    pub help: &'static str,
    /// What `embroidery.set`'s description says about it (after `key?: `).
    pub doc: &'static str,
    /// Whether `null` resets it to its default.
    pub nullable: bool,
    /// The value `embroidery.info` reports for these settings.
    pub get: fn(&StitchSettings) -> Value,
    /// Set it from a parameter value (checked first with [`Param::parse`]).
    pub set: fn(&mut StitchSettings, &Value),
    /// Whether it suits settings it was given for (`None`: always): the stitch type they end
    /// up with must use it.
    pub suits: Option<Suits>,
}

use StitchKind as K;

const ALL: &[StitchKind] = &[];
/// Whether `v` is JSON null.
fn null(v: &Value) -> bool {
    v.is_null()
}
fn f(v: &Value) -> Option<f64> {
    if null(v) { None } else { v.as_f64() }
}
fn whole(v: &Value) -> Option<u8> {
    if null(v) { None } else { v.as_u64().and_then(|n| u8::try_from(n).ok()) }
}
fn text(v: &Value) -> Option<String> {
    if null(v) { None } else { v.as_str().map(|s| s.trim().to_ascii_lowercase()) }
}
fn point(v: &Value) -> Option<[f64; 2]> {
    let a = v.as_array()?;
    let (x, y) = (a.first()?.as_f64()?, a.get(1)?.as_f64()?);
    (a.len() == 2 && x.is_finite() && y.is_finite()).then_some([x, y])
}
fn lines(v: &Value) -> Option<Vec<[f64; 4]>> {
    let a = v.as_array()?;
    if a.len() > StitchSettings::MAX_ANGLES {
        return None;
    }
    a.iter()
        .map(|l| {
            let l = l.as_array().filter(|l| l.len() == 4)?;
            let mut out = [0.0; 4];
            for (o, x) in out.iter_mut().zip(l) {
                *o = x.as_f64().filter(|x| x.is_finite())?;
            }
            Some(out)
        })
        .collect()
}
fn flag(v: &Value) -> bool {
    v.as_bool().unwrap_or_default()
}

/// A thread colour: `"#rrggbb"` (or `rrggbb`) or `[r, g, b]` (0–255).
pub fn parse_thread(v: &Value) -> Option<[u8; 3]> {
    match v {
        Value::String(s) => {
            let s = s.trim().trim_start_matches('#');
            if s.len() != 6 || !s.is_ascii() {
                return None;
            }
            let c = |i: usize| s.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok());
            Some([c(0)?, c(2)?, c(4)?])
        }
        Value::Array(a) if a.len() == 3 => {
            let c = |i: usize| a.get(i)?.as_u64().and_then(|v| u8::try_from(v).ok());
            Some([c(0)?, c(1)?, c(2)?])
        }
        _ => None,
    }
}

/// `#rrggbb`.
pub fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

/// Every stitch setting, in the order `embroidery.set` describes them.
pub static PARAMS: &[Param] = &[
    Param {
        key: "type",
        value: ValueKind::Kind,
        kinds: ALL,
        label: "Stitch:",
        help: "How the object is stitched",
        doc: "auto|running|bean|satin|zigzag|satinShape|fill|patternBorder|manual (patternBorder: a border pattern repeated along the path, bending with its curves, a whole number of repeats between corners; auto: by stroke width, thinUnder?: mm (0.2–3; 1) → bean outline (letters: narrow satin columns), wideOver?: mm (2–12; 7) → fill with a satin border, else satin shape) (default: the object's own, else running)",
        nullable: true,
        get: |s| json!(s.kind_id()),
        set: |s, v| {
            if let Some(k) = v.as_str().and_then(StitchKind::parse) {
                s.kind = k;
            }
        },
        suits: None,
    },
    Param {
        key: "length",
        value: ValueKind::Mm(StitchSettings::LENGTH),
        kinds: &[K::Running, K::Bean, K::Fill, K::PatternBorder],
        label: "Stitch Length:",
        help: "How long each stitch is",
        doc: "mm (stitch length, 0.3–12; running/bean 2.5, fill 4, pattern border 1.5)",
        nullable: true,
        get: |s| json!(s.length()),
        set: |s, v| s.length = f(v),
        suits: None,
    },
    Param {
        key: "spacing",
        value: ValueKind::Mm(StitchSettings::SPACING),
        kinds: &[K::Satin, K::Zigzag, K::SatinShape, K::Fill, K::Auto, K::PatternBorder],
        label: "Density:",
        help: "Distance between zigzags or rows: smaller is denser",
        doc: "mm (satin zigzag, satin shape, zigzag stitch or fill row spacing, 0.1–10; satin and fill 0.4, zigzag 2)",
        nullable: true,
        get: |s| json!(s.spacing()),
        set: |s, v| s.spacing = f(v),
        suits: None,
    },
    Param {
        key: "angle",
        value: ValueKind::Degrees,
        kinds: &[K::Fill],
        label: "Angle:",
        help: "The direction of the fill's rows",
        doc: "degrees (fill rows, counter-clockwise)",
        nullable: false,
        get: |s| json!(s.angle()),
        set: |s, v| s.angle = f(v).unwrap_or_default() % 360.0,
        suits: None,
    },
    Param {
        key: "width",
        value: ValueKind::Mm(*StitchSettings::WIDTH.start()..=*StitchSettings::PATTERN_WIDTH.end()),
        kinds: &[K::Satin, K::Zigzag, K::PatternBorder],
        label: "Width:",
        help: "How wide the column, zigzag or border is",
        doc: "mm (satin column or zigzag, 0.3–12; satin: the stroke width, zigzag: 3; pattern border: the band, 1–40, default the pattern's own)",
        nullable: true,
        get: |s| json!(s.width),
        set: |s, v| s.width = f(v),
        // Pattern borders span 1–40 mm, satins and zigzags 0.3–12 mm.
        suits: Some(|s| {
            let r = if s.kind == K::PatternBorder { StitchSettings::PATTERN_WIDTH } else { StitchSettings::WIDTH };
            match s.width {
                Some(w) if !r.contains(&w) => Err(format!("`width` must be from {} to {} mm for these stitches", r.start(), r.end())),
                _ => Ok(()),
            }
        }),
    },
    Param {
        key: "pattern",
        value: ValueKind::Id,
        kinds: &[K::PatternBorder],
        label: "Pattern:",
        help: "The border pattern repeated along the path",
        doc: "a border pattern id: waves|scallops|chain|diamonds or one loaded into the document (embroidery.borderPatterns) | null (pattern border: the pattern; waves)",
        nullable: true,
        get: |s| json!(s.pattern()),
        set: |s, v| s.pattern = text(v),
        suits: None,
    },
    Param {
        key: "flip",
        value: ValueKind::Flag,
        kinds: &[K::PatternBorder],
        label: "Flip",
        help: "Mirror the pattern across the path",
        doc: "bool (pattern border: mirrored across the path)",
        nullable: false,
        get: |s| json!(s.flip),
        set: |s, v| s.flip = flag(v),
        suits: None,
    },
    Param {
        key: "thread",
        value: ValueKind::Thread,
        kinds: ALL,
        label: "Thread:",
        help: "The thread colour (unset: the object's stroke or fill colour)",
        doc: "\"#rrggbb\" | [r, g, b] (default: the stroke for path stitches, the fill for fills)",
        nullable: true,
        get: |s| json!(s.thread.map(hex)),
        set: |s, v| s.thread = parse_thread(v),
        suits: None,
    },
    Param {
        key: "pull",
        value: ValueKind::Mm(StitchSettings::PULL),
        kinds: &[K::Satin, K::SatinShape, K::Fill, K::Auto],
        label: "Pull Compensation:",
        help: "Sews the column this much wider, since the thread pulls the fabric in",
        doc: "mm (pull compensation, 0–3; 0: satins sewn this much wider, fill rows this much longer, half at each end)",
        nullable: true,
        get: |s| json!(s.pull()),
        set: |s, v| s.pull = f(v),
        suits: None,
    },
    Param {
        key: "pullPercent",
        value: ValueKind::Percent(StitchSettings::PULL_PERCENT),
        kinds: &[K::Satin, K::SatinShape, K::Fill, K::Auto],
        label: "Pull Compensation (%):",
        help: "More pull compensation, as a share of the width (satin) or row length (fill)",
        doc: "% (0–30; 0: more pull compensation, a share of the satin's width or the fill row's length, added to pull)",
        nullable: true,
        get: |s| json!(s.pull_percent()),
        set: |s, v| s.pull_percent = f(v),
        suits: None,
    },
    Param {
        key: "push",
        value: ValueKind::Mm(StitchSettings::PUSH),
        kinds: &[K::Satin, K::Fill, K::Auto],
        label: "Push Compensation:",
        help: "Sews columns this much shorter at their ends, and fills across their rows, since the stitches push the fabric out",
        doc: "mm (push compensation, 0–3; 0: satin columns along a path sewn this much shorter, half at each end; fills this much shorter across their rows)",
        nullable: true,
        get: |s| json!(s.push()),
        set: |s, v| s.push = f(v),
        suits: None,
    },
    Param {
        key: "startAt",
        value: ValueKind::Point,
        kinds: &[K::Running, K::Bean, K::Zigzag],
        label: "Start At:",
        help: "Where the needle starts: the path is entered at its point nearest it",
        doc: "[x, y] mm in document space (null: nearest where the needle is; the path is entered at its point nearest it)",
        nullable: true,
        get: |s| json!(s.start_at),
        set: |s, v| s.start_at = point(v),
        suits: None,
    },
    Param {
        key: "angles",
        value: ValueKind::Lines,
        kinds: &[K::Satin, K::SatinShape, K::Auto],
        label: "Stitch Angles:",
        help: "Lines across the satin setting its stitches' direction where they cross it, turning gradually between them (the Stitch Angle tool draws them)",
        doc: "[[x1 y1 x2 y2] …] mm in document space; at most 256; null or [] for none: stitch angle lines; where one crosses the satin's centre line (a column's path; a satin shape's strokes) its stitches take the line's angle to it and turn gradually between lines by distance along it; at most 60° off square; between rails they are rungs",
        nullable: true,
        get: |s| json!(s.angles),
        set: |s, v| s.angles = lines(v).unwrap_or_default(),
        suits: None,
    },
    Param {
        key: "puff",
        value: ValueKind::Mm(StitchSettings::PUFF),
        kinds: &[K::Satin, K::SatinShape, K::Auto],
        label: "Puff Foam:",
        help: "Sew over 3D foam this thick: a stop to place the foam, dense satin with no underlay, capped ends, and a perforation all round so the excess foam tears away",
        doc: "mm (foam thickness, 1–6; null: not puffed; a stop to place the foam, dense satin at 0.25 mm unless spacing is set, no underlay, open ends capped, then short running stitches all round the edge to perforate the foam)",
        nullable: true,
        get: |s| json!(s.puff()),
        set: |s, v| s.puff = f(v),
        suits: None,
    },
    Param {
        key: "keepCovered",
        value: ValueKind::Flag,
        kinds: &[K::Fill],
        label: "Keep Covered Rows",
        help: "Sew this fill's rows under objects sewn later on top of it too (normally they are cut away)",
        doc: "bool (false: the rows under objects sewn later on top of the fill are cut away, leaving the document's overlap)",
        nullable: false,
        get: |s| json!(s.keep_covered),
        set: |s, v| s.keep_covered = flag(v),
        suits: None,
    },
    Param {
        key: "underlay",
        value: ValueKind::Choice(&["none", "auto", "center", "edge", "zigzag", "fill"]),
        kinds: &[K::Satin, K::SatinShape, K::Fill],
        label: "Underlay:",
        help: "Stitches sewn first to hold the fabric and lift the top stitches",
        doc: "none|auto|center|edge|zigzag (satin) | none|auto|center|edge (satin shape) | none|auto|fill (fill); auto (what new objects get) picks by stitch and width",
        nullable: true,
        get: |s| json!(s.underlay().id()),
        set: |s, v| s.underlay = text(v).as_deref().and_then(Underlay::parse).unwrap_or_default(),
        // An underlay must suit the stitch the object ends up with.
        suits: Some(|s| {
            let u = s.underlay;
            if u == Underlay::None || Underlay::for_kind(s.kind).contains(&u) {
                Ok(())
            } else {
                Err(format!("{} stitches have no {} underlay", s.kind.id(), u.id()))
            }
        }),
    },
    Param {
        key: "stagger",
        value: ValueKind::Whole { range: StitchSettings::STAGGER, odd: false },
        kinds: &[K::Fill],
        label: "Pattern:",
        help: "Rows before the needle holes line up again (2: brick, 3–8: tatami)",
        doc: "2–8 (fill rows before the needle holes line up again; 3)",
        nullable: true,
        get: |s| json!(s.stagger()),
        set: |s, v| s.stagger = whole(v),
        suits: None,
    },
    Param {
        key: "passes",
        value: ValueKind::Whole { range: StitchSettings::PASSES, odd: true },
        kinds: &[K::Bean],
        label: "Passes:",
        help: "How many times each stitch is sewn",
        doc: "3|5|7|9 (bean; 3)",
        nullable: true,
        get: |s| json!(s.passes()),
        set: |s, v| s.passes = whole(v),
        suits: None,
    },
    Param {
        key: "border",
        value: ValueKind::Mm(StitchSettings::BORDER),
        kinds: &[K::Fill, K::Auto],
        label: "Border:",
        help: "A satin border sewn over the shape's edges, after the fill",
        doc: "mm (fill: a satin border over the edges, 0.5–6) | null (none)",
        nullable: true,
        get: |s| json!(s.border()),
        set: |s, v| s.border = f(v),
        suits: None,
    },
    Param {
        key: "motif",
        value: ValueKind::Choice(&crate::motifs::MOTIFS),
        kinds: &[K::Fill],
        label: "Motif:",
        help: "A motif carved into the fill",
        doc: "diamonds|waves|circles|scales|herringbone | null (fill: a motif carved into the fill, its needle points on the motif's lines)",
        nullable: true,
        get: |s| json!(s.motif),
        set: |s, v| s.motif = text(v),
        suits: None,
    },
    Param {
        key: "motifSize",
        value: ValueKind::Mm(StitchSettings::MOTIF_SIZE),
        kinds: &[K::Fill],
        label: "Motif Size:",
        help: "How large one repeat of the motif is",
        doc: "mm (3–60; 10)",
        nullable: true,
        get: |s| json!(s.motif_size()),
        set: |s, v| s.motif_size = f(v),
        suits: None,
    },
    Param {
        key: "thinUnder",
        value: ValueKind::Mm(StitchSettings::THIN),
        kinds: &[K::Auto],
        label: "Bean Outline Under:",
        help: "Thinner strokes get a bean stitch round their outline",
        doc: "mm (auto: strokes thinner than this get a bean outline, a letter's narrow satin columns at least 0.8 mm wide; 0.2–3; 1)",
        nullable: true,
        get: |s| json!(s.auto_thresholds().0),
        set: |s, v| s.thin_under = f(v),
        suits: None,
    },
    Param {
        key: "wideOver",
        value: ValueKind::Mm(StitchSettings::WIDE),
        kinds: &[K::Auto],
        label: "Fill Over:",
        help: "Wider strokes are filled, with a satin border; strokes in between get satin",
        doc: "mm (auto: strokes wider than this are filled with a satin border, 2–12; 7)",
        nullable: true,
        get: |s| json!(s.auto_thresholds().1),
        set: |s, v| s.wide_over = f(v),
        suits: None,
    },
    Param {
        key: "rails",
        value: ValueKind::Flag,
        kinds: &[K::Satin],
        label: "Between Rails",
        help: "The object's two paths are the column's edges; lines crossing both set the stitch direction",
        doc: "bool (satin: between the object's two longest subpaths as its edges, other subpaths crossing both as rungs setting the stitch direction)",
        nullable: false,
        get: |s| json!(s.rails),
        set: |s, v| s.rails = flag(v),
        suits: None,
    },
    Param {
        key: "trimBefore",
        value: ValueKind::Flag,
        kinds: ALL,
        label: "Before",
        help: "Cut the thread before this object, however close it starts",
        doc: "bool (cut the thread before the object, however close the stitch before)",
        nullable: false,
        get: |s| json!(s.trim_before),
        set: |s, v| s.trim_before = flag(v),
        suits: None,
    },
    Param {
        key: "stopBefore",
        value: ValueKind::Choice(&StitchSettings::STOPS),
        kinds: ALL,
        label: "Stop Before:",
        help: "Stop the machine before this object: to change something by hand, place appliqué fabric, or trim it",
        doc: "stop|placeFabric|trimFabric | null (stop the machine before the object, written as a colour change to the same thread: a plain stop, placing appliqué fabric, or trimming it; the worksheet says which)",
        nullable: true,
        get: |s| json!(s.stop_before),
        set: |s, v| {
            s.stop_before = text(v).and_then(|t| StitchSettings::STOPS.iter().find(|id| id.eq_ignore_ascii_case(&t)).map(|id| id.to_string()))
        },
        suits: None,
    },
    Param {
        key: "font",
        value: ValueKind::Id,
        kinds: ALL,
        label: "Embroidery Font:",
        help: "Type only: sew the letters from this embroidery font's stitch objects instead of the outline font",
        doc: "an embroidery font's id (one loaded into the document: embroidery.fonts) | null (type only: each letter sewn from the font's own stitch objects, spaced by its advances and kerning with the type's size, tracking and alignment; characters it lacks are sewn from the outline font, with a warning; null: the outline font, with this object's stitch)",
        nullable: true,
        get: |s| json!(s.font),
        set: |s, v| s.font = text(v),
        suits: None,
    },
    Param {
        key: "trimAfter",
        value: ValueKind::Flag,
        kinds: ALL,
        label: "After",
        help: "Cut the thread after this object",
        doc: "bool (cut the thread after the object, however close the next stitch)",
        nullable: false,
        get: |s| json!(s.trim_after),
        set: |s, v| s.trim_after = flag(v),
        suits: None,
    },
];

impl Param {
    /// Whether it applies to stitch type `k`.
    pub fn applies(&self, k: StitchKind) -> bool {
        self.kinds.is_empty() || self.kinds.contains(&k)
    }

    /// Check a parameter value, as `embroidery.set` takes it: the error says what it must be.
    pub fn parse(&self, v: &Value) -> Result<(), String> {
        if v.is_null() {
            return if self.nullable { Ok(()) } else { Err(self.expected()) };
        }
        let ok = match &self.value {
            ValueKind::Kind => v.as_str().and_then(StitchKind::parse).is_some(),
            ValueKind::Mm(r) | ValueKind::Percent(r) => v.as_f64().is_some_and(|x| r.contains(&x)),
            ValueKind::Degrees => v.as_f64().is_some_and(f64::is_finite),
            ValueKind::Whole { range, odd } => {
                v.as_u64().and_then(|n| u8::try_from(n).ok()).is_some_and(|n| range.contains(&n) && (!odd || n % 2 == 1))
            }
            ValueKind::Flag => v.is_boolean(),
            ValueKind::Choice(ids) => v.as_str().is_some_and(|s| {
                let s = s.trim().to_ascii_lowercase();
                ids.iter().any(|id| id.eq_ignore_ascii_case(&s))
            }),
            ValueKind::Id => v.as_str().is_some(),
            ValueKind::Thread => parse_thread(v).is_some(),
            ValueKind::Point => point(v).is_some(),
            ValueKind::Lines => lines(v).is_some(),
        };
        if ok { Ok(()) } else { Err(self.expected()) }
    }

    /// What a value of it must be.
    pub fn expected(&self) -> String {
        let key = self.key;
        let or_null = if self.nullable { ", or null" } else { "" };
        match &self.value {
            ValueKind::Kind => {
                let ids: Vec<&str> = StitchKind::ALL.iter().map(|k| k.id()).collect();
                format!("`type` must be {} (embroidery.clear removes stitches)", ids.join(", "))
            }
            ValueKind::Mm(r) => format!("`{key}` must be a number from {} to {} (mm){or_null}", r.start(), r.end()),
            ValueKind::Percent(r) => format!("`{key}` must be a number from {} to {} (%){or_null}", r.start(), r.end()),
            ValueKind::Degrees => format!("`{key}` must be a number of degrees"),
            ValueKind::Whole { range, odd } => {
                let what = if *odd { "an odd whole number" } else { "a whole number" };
                format!("`{key}` must be {what} from {} to {}{or_null}", range.start(), range.end())
            }
            ValueKind::Flag => format!("`{key}` must be true or false"),
            ValueKind::Choice(ids) => format!("`{key}` must be one of {}{or_null}", ids.join(", ")),
            ValueKind::Id => format!("`{key}` must be an id{or_null}"),
            ValueKind::Thread => format!("`{key}` must be \"#rrggbb\", [r, g, b]{or_null}"),
            ValueKind::Point => format!("`{key}` must be [x, y] (mm){or_null}"),
            ValueKind::Lines => format!("`{key}` must be [[x1, y1, x2, y2], …] (mm, at most {}){or_null}", StitchSettings::MAX_ANGLES),
        }
    }
}

/// The setting `key`.
pub fn param(key: &str) -> Option<&'static Param> {
    PARAMS.iter().find(|p| p.key == key)
}

/// `embroidery.set`'s parameter description, made from the table.
pub fn set_doc() -> String {
    let params: Vec<String> = PARAMS.iter().map(|p| format!("{}?: {}", p.key, p.doc)).collect();
    format!(
        "{{{}, ids?}} stitch the paths, compound paths and type objects in ids or the selection (groups: those inside; type is sewn letter by letter, each letter in its characters' colour: use auto for lettering), as one undo step; null resets a setting to its default (settings a type doesn't use are kept but have no effect) → {{changed}}",
        params.join(", ")
    )
}

/// `embroidery.info`'s description of the selection's settings, made from the table.
pub fn info_keys() -> String {
    PARAMS.iter().map(|p| p.key).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_every_kind_parses() {
        let mut keys: Vec<&str> = PARAMS.iter().map(|p| p.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), PARAMS.len());
        let t = param("type").unwrap();
        for k in StitchKind::ALL {
            t.parse(&json!(k.id())).unwrap();
            let mut s = StitchSettings::default();
            (t.set)(&mut s, &json!(k.id()));
            assert_eq!(s.kind, k);
        }
    }

    #[test]
    fn values_are_checked_and_set_as_before() {
        let p = |k: &str| param(k).unwrap();
        assert!(p("length").parse(&json!(0.2)).is_err());
        assert!(p("length").parse(&json!(null)).is_ok());
        assert!(p("angle").parse(&json!(null)).is_err());
        assert!(p("passes").parse(&json!(4)).is_err());
        assert!(p("passes").parse(&json!(5)).is_ok());
        assert!(p("motif").parse(&json!("Waves")).is_ok());
        assert!(p("motif").parse(&json!("stars")).is_err());
        assert!(p("thread").parse(&json!("#zz0000")).is_err());
        let mut s = StitchSettings::default();
        (p("angle").set)(&mut s, &json!(370.0));
        (p("motif").set)(&mut s, &json!(" Waves "));
        (p("thread").set)(&mut s, &json!([1, 2, 3]));
        assert_eq!((s.angle, s.motif.as_deref(), s.thread), (10.0, Some("waves"), Some([1, 2, 3])));
        (p("thread").set)(&mut s, &json!(null));
        assert_eq!(s.thread, None);
        s.kind = K::Satin;
        s.underlay = Underlay::Fill;
        assert!((p("underlay").suits.unwrap())(&s).is_err());
        s.width = Some(20.0);
        assert!((p("width").suits.unwrap())(&s).is_err());
        s.kind = K::PatternBorder;
        assert!((p("width").suits.unwrap())(&s).is_ok());
    }

    #[test]
    fn descriptions_name_every_setting_and_type() {
        let d = set_doc();
        for p in PARAMS {
            assert!(d.contains(&format!("{}?: ", p.key)), "{}", p.key);
        }
        for k in StitchKind::ALL {
            assert!(d.contains(k.id()), "{}", k.id());
        }
    }
}
