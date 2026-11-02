//! Zigzag stitch: an open zigzag along the path, swinging side to side.

use super::{Input, Output};
use crate::{generate, split};

pub(super) fn sew(i: &Input) -> Output {
    let s = i.settings;
    let w = s.width(i.stroke_mm);
    let rule = split::Rule::of(s);
    let span = if rule.is_some() { *s.width_range().end() } else { generate::MAX_STITCH };
    let runs = i.lines().iter().map(|(l, closed)| split::run(generate::zigzag_within(l, *closed, w, s.spacing(), span), rule)).collect();
    Output { runs, ..Output::default() }
}
