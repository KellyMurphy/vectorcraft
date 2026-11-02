//! Running stitch: single stitches along the path, sharp corners keeping a needle point. Lines
//! that meet are sewn as one network ([`crate::network`]). A hand-stitch style ([`RunStyle`])
//! sews each line on its own ([`crate::run_styles`]).

use super::{Input, Output};
use crate::{generate, network, run_styles};
use vectorcraft_doc::RunStyle;

pub(super) fn sew(i: &Input) -> Output {
    let len = i.settings.length();
    let lines = i.lines();
    if i.settings.run_style() != RunStyle::Single {
        let runs = lines.iter().map(|(l, closed)| run_styles::run(l, *closed, i.settings)).collect();
        return Output { runs, ..Output::default() };
    }
    let stitch = |l: &[kurbo::Point]| generate::running(l, len);
    let runs = network::runs(&lines, &stitch, &|p| p.to_vec()).unwrap_or_else(|| lines.iter().map(|(l, _)| stitch(l)).collect());
    Output { runs, ..Output::default() }
}
