//! Satin shape: satin following a closed shape's strokes from edge to edge (lettering); a shape
//! with no strokes to follow is filled instead, and one whose strokes satin can't cover (too heavy
//! to reach its corners and junctions) is sewn as a fill with a satin border, as Auto does.

use vectorcraft_doc::{StitchKind, StitchSettings, Underlay};

use super::{Input, Output};
use crate::generate;

pub(super) fn sew(i: &Input) -> Output {
    let s = i.settings;
    let mut out = Output::default();
    let polys = i.polys();
    let underlay = match s.underlay() {
        Underlay::Auto => i.auto.shape(crate::stroke_width(&polys)),
        u => u,
    };
    let pull = s.pull_for(crate::stroke_width(&polys));
    let (satin, quality) = generate::satin_shape_hand(&polys, i.rule, s.spacing(), pull, underlay == Underlay::Center, 0.0, i.hand);
    if !satin.is_empty() && quality.poor() {
        let fill = crate::auto_fill(s, &polys);
        let mut out = super::sew(&Input { settings: &fill, ..*i });
        out.warnings.push("a satin shape is too heavy for satin to cover: sewn as a fill with a satin border (as Auto does)".into());
        return out;
    }
    // An edge walk goes in first, under the satin (none when the shape is filled instead).
    if underlay == Underlay::Edge && !satin.is_empty() {
        out.runs.extend(generate::edge_walk(&polys, i.rule));
    }
    if satin.is_empty() && !polys.is_empty() {
        // No strokes to follow (a blob, not a letter): fill it instead.
        match generate::fill(
            &polys,
            i.rule,
            45.0,
            StitchSettings::DEFAULT_SPACING,
            StitchKind::Fill.default_length(),
            StitchSettings::DEFAULT_STAGGER,
        ) {
            Ok(f) if !f.is_empty() => {
                out.warnings.push("a satin shape has no strokes to follow (it is too round or wide): filled instead".into());
                out.runs.extend(f);
            }
            Ok(_) => {}
            Err(e) => out.warnings.push(e),
        }
    } else {
        out.runs.extend(satin);
        // Over foam: the perforation, all round the shape's outline.
        if s.puffed() {
            out.runs.extend(generate::outline_perforation(&polys));
        }
    }
    out
}
