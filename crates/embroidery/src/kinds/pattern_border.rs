//! Pattern border: a border pattern repeated along the path (see [`crate::borders`]).

use super::{Input, Output};
use crate::borders;

pub(super) fn sew(i: &Input) -> Output {
    let s = i.settings;
    let mut out = Output::default();
    let id = s.pattern();
    let Some(pattern) = borders::find(i.patterns, id) else {
        let newer = i.patterns.iter().any(|p| p.id == id && p.unknown.is_some());
        out.warnings.push(if newer {
            format!("the border pattern `{id}` is from a newer VectorCraft that this version can't read: not sewn")
        } else {
            format!("there is no border pattern `{id}`: not sewn")
        });
        return out;
    };
    let w = s.pattern_width(pattern.width);
    for (l, closed) in &i.lines() {
        match borders::sew(&pattern, l, *closed, w, s.flip, s.length(), s.spacing()) {
            Ok(r) => out.runs.extend(r),
            Err(e) => out.warnings.push(e),
        }
    }
    out
}
