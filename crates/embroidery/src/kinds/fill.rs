//! Fill: staggered rows covering the shape (tatami), with fill underlay, an optional carved motif
//! and an optional satin border over the edges.

use kurbo::Point;
use vectorcraft_doc::Underlay;

use super::{Input, Output};
use crate::{generate, motifs};

pub(super) fn sew(i: &Input) -> Output {
    let s = i.settings;
    let mut out = Output::default();
    let polys = i.polys();
    let under = match s.underlay() {
        Underlay::Fill => generate::fill_underlay(&polys, i.rule, s.angle()),
        // Auto: the outline walked first, then rows across the top rows.
        Underlay::Auto => {
            generate::fill_underlay(&polys, i.rule, s.angle()).map(|rows| generate::edge_walk(&polys, i.rule).into_iter().chain(rows).collect())
        }
        _ => Ok(vec![]),
    };
    // A carved motif over the shape's box (none when unknown or too fine to carve).
    let carve = match s.motif.as_deref() {
        Some(id) => {
            let (mut lo, mut hi) = (Point::new(f64::INFINITY, f64::INFINITY), Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
            for p in polys.iter().flatten() {
                lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
                hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
            }
            let segs = motifs::tiled(id, s.motif_size(), lo, hi);
            if segs.is_none() {
                out.warnings.push(format!("the motif `{id}` can't be carved here (unknown, or too small for so large a fill): plain fill"));
            }
            segs.unwrap_or_default()
        }
        None => vec![],
    };
    let top = generate::fill_carved(&polys, i.rule, s.angle(), s.spacing(), s.length(), s.stagger(), &carve);
    match under.and_then(|u| Ok((u, top?))) {
        Ok((u, top)) => {
            out.runs.extend(u.into_iter().chain(top));
            // The border: a satin over each edge, sewn last so it covers the rows' ends.
            if let Some(w) = s.border() {
                out.runs.extend(polys.iter().map(|p| generate::border(p, w)));
            }
        }
        // Nothing is sewn when the rows can't be made.
        Err(e) => out.warnings.push(e),
    }
    out
}
