//! Manual stitch: the needle goes down at every anchor, as drawn (imported designs).

use super::{Input, Output};
use crate::generate;

pub(super) fn sew(i: &Input) -> Output {
    Output { runs: i.paths.iter().flat_map(|p| generate::manual(p)).collect(), ..Output::default() }
}
