//! Bean stitch: running stitch with each stitch sewn several times (a bolder line).

use super::{Input, Output};
use crate::generate;

pub(super) fn sew(i: &Input) -> Output {
    let s = i.settings;
    Output { runs: i.lines().iter().map(|(l, _)| generate::bean(&generate::running(l, s.length()), s.passes())).collect(), ..Output::default() }
}
