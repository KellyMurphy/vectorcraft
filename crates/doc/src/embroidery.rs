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
    /// Parallel rows of staggered running stitches covering the shape (tatami fill).
    Fill,
    /// The needle goes down at every anchor, as drawn (imported designs).
    Manual,
}

impl StitchKind {
    pub const ALL: [StitchKind; 5] = [Self::Running, Self::Bean, Self::Satin, Self::Fill, Self::Manual];

    pub fn id(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Bean => "bean",
            Self::Satin => "satin",
            Self::Fill => "fill",
            Self::Manual => "manual",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "Running Stitch",
            Self::Bean => "Bean Stitch",
            Self::Satin => "Satin Column",
            Self::Fill => "Fill",
            Self::Manual => "Manual Stitch",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id().eq_ignore_ascii_case(s.trim()))
    }

    /// Stitches along the path (rather than covering its area): the thread defaults to the stroke.
    pub fn follows_path(self) -> bool {
        self != Self::Fill
    }

    /// The default stitch length, mm.
    pub fn default_length(self) -> f64 {
        match self {
            Self::Fill => 4.0,
            _ => 2.5,
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
}

impl StitchSettings {
    /// Allowed stitch lengths, mm: below this range the needle cuts the fabric; above it the
    /// stitch snags (and DST can't move further than 12.1 mm in one record).
    pub const LENGTH: std::ops::RangeInclusive<f64> = 0.3..=12.0;
    /// Allowed satin and fill spacing, mm.
    pub const SPACING: std::ops::RangeInclusive<f64> = 0.1..=5.0;
    /// Allowed satin widths, mm.
    pub const WIDTH: std::ops::RangeInclusive<f64> = 0.3..=12.0;
    pub const DEFAULT_SPACING: f64 = 0.4;
    /// Satin width when the object has no stroke and no width is set, mm.
    pub const DEFAULT_WIDTH: f64 = 3.0;

    pub fn new(kind: StitchKind) -> Self {
        Self { kind, ..Self::default() }
    }

    /// The stitch length to sew with, mm (always within [`Self::LENGTH`]).
    pub fn length(&self) -> f64 {
        clamp(self.length, self.kind.default_length(), Self::LENGTH)
    }

    /// The satin or fill spacing to sew with, mm (always within [`Self::SPACING`]).
    pub fn spacing(&self) -> f64 {
        clamp(self.spacing, Self::DEFAULT_SPACING, Self::SPACING)
    }

    /// The satin width, mm, given the stroke width in mm (always within [`Self::WIDTH`]).
    pub fn width(&self, stroke_mm: f64) -> f64 {
        let fallback = if stroke_mm.is_finite() && stroke_mm > 0.0 { stroke_mm } else { Self::DEFAULT_WIDTH };
        clamp(self.width, fallback, Self::WIDTH)
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
        assert_eq!(s.spacing(), 5.0);
        assert_eq!(s.width(2.0), 0.3);
        assert_eq!(s.angle(), 0.0);
        assert_eq!(StitchSettings::default().width(f64::NAN), StitchSettings::DEFAULT_WIDTH);
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
