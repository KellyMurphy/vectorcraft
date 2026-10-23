//! Bean stitch: running stitch with each stitch sewn several times (a bolder line). Lines that
//! meet are sewn as one network ([`crate::network`]), the way back along a branch sewn once over it.

use super::{Input, Output};
use crate::{generate, network};

pub(super) fn sew(i: &Input) -> Output {
    let s = i.settings;
    let lines = i.lines();
    let stitch = |l: &[kurbo::Point]| generate::running(l, s.length());
    let bold = |p: &[kurbo::Point]| generate::bean(p, s.passes());
    let runs = network::runs(&lines, &stitch, &bold).unwrap_or_else(|| lines.iter().map(|(l, _)| bold(&stitch(l))).collect());
    Output { runs, ..Output::default() }
}
