//! Satin shape: satin following a closed shape's strokes from edge to edge (lettering); a shape
//! with no strokes to follow is filled instead.

use vectorcraft_doc::{StitchKind, StitchSettings, Underlay};

use super::{Input, Output};
use crate::generate;

pub(super) fn sew(i: &Input) -> Output {
    let s = i.settings;
    let mut out = Output::default();
    let polys = i.polys();
    let satin = generate::satin_shape(&polys, i.rule, s.spacing(), s.pull(), s.underlay() == Underlay::Center);
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
    }
    out
}
