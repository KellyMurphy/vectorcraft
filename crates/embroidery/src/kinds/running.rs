//! Running stitch: single stitches along the path, sharp corners keeping a needle point. Lines
//! that meet are sewn as one network ([`crate::network`]).

use super::{Input, Output};
use crate::{generate, network};

pub(super) fn sew(i: &Input) -> Output {
    let len = i.settings.length();
    let lines = i.lines();
    let stitch = |l: &[kurbo::Point]| generate::running(l, len);
    let runs = network::runs(&lines, &stitch, &|p| p.to_vec()).unwrap_or_else(|| lines.iter().map(|(l, _)| stitch(l)).collect());
    Output { runs, ..Output::default() }
}
