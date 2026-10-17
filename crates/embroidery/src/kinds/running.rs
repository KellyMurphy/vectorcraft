//! Running stitch: single stitches along the path, sharp corners keeping a needle point.

use super::{Input, Output};
use crate::generate;

pub(super) fn sew(i: &Input) -> Output {
    let len = i.settings.length();
    Output { runs: i.lines().iter().map(|(l, _)| generate::running(l, len)).collect(), ..Output::default() }
}
