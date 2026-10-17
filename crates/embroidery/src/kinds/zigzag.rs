//! Zigzag stitch: an open zigzag along the path, swinging side to side.

use super::{Input, Output};
use crate::generate;

pub(super) fn sew(i: &Input) -> Output {
    let s = i.settings;
    let w = s.width(i.stroke_mm);
    Output { runs: i.lines().iter().map(|(l, closed)| generate::zigzag(l, *closed, w, s.spacing())).collect(), ..Output::default() }
}
