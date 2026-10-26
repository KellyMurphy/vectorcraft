//! One module per stitch type: how each turns an object's paths into runs of needle points.
//! [`sew`] is the list of them (a new [`StitchKind`] doesn't compile until it is added). Shared
//! geometry (flattening, resampling, satin columns, fill rows) stays in [`crate::generate`]; Auto,
//! which picks one of these per shape, is in the crate root.

use kurbo::Point;
use vectorcraft_doc::{BorderPattern, StitchKind, StitchSettings};
use vectorcraft_geom::{FillRule, PathData};

use crate::generate;

mod bean;
mod fill;
mod manual;
mod motif_fill;
mod pattern_border;
mod running;
mod satin;
mod satin_shape;
mod zigzag;

/// What a stitch type sews: an object's settings and paths.
pub struct Input<'a> {
    pub settings: &'a StitchSettings,
    /// The object's stroke width, mm (satins and zigzags default to it).
    pub stroke_mm: f64,
    pub paths: &'a [&'a PathData],
    pub rule: FillRule,
    /// The document's border patterns.
    pub patterns: &'a [BorderPattern],
    /// The document's motifs.
    pub motifs: &'a [vectorcraft_doc::Motif],
    /// What an `auto` underlay becomes (the document's fabric's thresholds).
    pub auto: generate::AutoUnderlay,
    /// Areas sewn later on top of the object (fills cut them out), and how far its rows still
    /// reach under them, mm.
    pub cuts: &'a [generate::Cut],
    pub overlap: f64,
    /// How lettering is written (print or script), for satin by pen strokes.
    pub hand: crate::penstrokes::Hand,
}

impl Input<'_> {
    /// Each subpath flattened to mm, with whether it is closed.
    pub fn lines(&self) -> Vec<(Vec<Point>, bool)> {
        self.paths.iter().flat_map(|p| generate::polylines(p)).collect()
    }

    /// The subpaths as outlines of an area, mm.
    pub fn polys(&self) -> Vec<Vec<Point>> {
        self.lines().into_iter().map(|(l, _)| l).collect()
    }
}

/// What a stitch type made: runs of needle points (mm, in sewing order) and warnings about the art.
/// No runs (all empty) means nothing is sewn.
#[derive(Default)]
pub struct Output {
    pub runs: Vec<Vec<Point>>,
    pub warnings: Vec<String>,
}

/// Sew `i` with its stitch type. Auto is decided before this (it picks one of the others).
pub fn sew(i: &Input) -> Output {
    match i.settings.kind {
        StitchKind::Running | StitchKind::Auto => running::sew(i),
        StitchKind::Bean => bean::sew(i),
        StitchKind::Satin => satin::sew(i),
        StitchKind::Zigzag => zigzag::sew(i),
        StitchKind::SatinShape => satin_shape::sew(i),
        StitchKind::Fill => fill::sew(i),
        StitchKind::PatternBorder => pattern_border::sew(i),
        StitchKind::MotifFill => motif_fill::sew(i),
        StitchKind::Manual => manual::sew(i),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_geom::{Anchor, SubPath};

    /// A 40 × 4 mm bar, in points: a shape every stitch type suits (Satin Shape needs a stroke,
    /// not a blob).
    fn rect() -> PathData {
        let mm = |x: f64, y: f64| Anchor::corner(Point::new(x / crate::PT_MM, y / crate::PT_MM));
        PathData::single(SubPath { anchors: vec![mm(0.0, 0.0), mm(40.0, 0.0), mm(40.0, 4.0), mm(0.0, 4.0)], closed: true })
    }

    #[test]
    fn every_stitch_type_sews_a_plain_shape_cleanly() {
        let path = rect();
        for k in StitchKind::ALL.into_iter().filter(|k| *k != StitchKind::Auto) {
            let mut settings = StitchSettings::new(k);
            if k == StitchKind::MotifFill {
                // A motif small enough for a 4 mm bar.
                (settings.motif, settings.motif_size) = (Some("dots".into()), Some(4.0));
            }
            let out = sew(&Input {
                settings: &settings,
                stroke_mm: 1.0,
                paths: &[&path],
                rule: FillRule::NonZero,
                patterns: &[],
                motifs: &[],
                auto: Default::default(),
                cuts: &[],
                overlap: 0.0,
                hand: crate::penstrokes::Hand::Print,
            });
            assert!(out.warnings.is_empty(), "{}: {:?}", k.id(), out.warnings);
            let needles: usize = out.runs.iter().map(Vec::len).sum();
            assert!(needles >= 4, "{}: {needles} needle points", k.id());
            // Every needle point is on or near the shape (a band at most a pattern's width out).
            for p in out.runs.iter().flatten() {
                assert!(p.x > -7.0 && p.x < 47.0 && p.y > -7.0 && p.y < 11.0, "{}: {p:?}", k.id());
            }
        }
    }

    #[test]
    fn problems_are_warnings_not_runs() {
        let path = rect();
        let settings = StitchSettings { pattern: Some("no-such-pattern".into()), ..StitchSettings::new(StitchKind::PatternBorder) };
        let out = sew(&Input {
            settings: &settings,
            stroke_mm: 1.0,
            paths: &[&path],
            rule: FillRule::NonZero,
            patterns: &[],
            motifs: &[],
            auto: Default::default(),
            cuts: &[],
            overlap: 0.0,
            hand: crate::penstrokes::Hand::Print,
        });
        assert!(out.runs.iter().all(Vec::is_empty) && out.warnings.len() == 1, "{:?}", out.warnings);
    }
}
