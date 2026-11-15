//! Fill: staggered rows covering the shape (tatami) or rings following its outline (contour,
//! [`crate::contour`]), with fill underlay, an optional carved motif (tatami) and an optional satin
//! border over the edges.

use kurbo::Point;
use vectorcraft_doc::{FillStyle, Underlay};

use super::{Input, Output};
use crate::{contour, generate, motifs};

pub(super) fn sew(i: &Input) -> Output {
    let s = i.settings;
    let mut out = Output::default();
    let polys = i.polys();
    // What is sewn on top later is cut out of the underlay too.
    let cuts = if s.keep_covered { &[][..] } else { i.cuts };
    let rows = || generate::fill_underlay_cut(&polys, i.rule, s.angle(), cuts, i.overlap);
    let under = match s.underlay() {
        Underlay::Fill => rows(),
        // Auto: the outline walked first, then rows across the top rows.
        Underlay::Auto => rows().map(|rows| generate::outside_cuts(generate::edge_walk(&polys, i.rule), cuts).into_iter().chain(rows).collect()),
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
            let segs = motifs::find(i.motifs, id)
                .filter(|m| m.kind == vectorcraft_doc::MotifKind::Carve)
                .and_then(|m| motifs::tiled_motif(&m, s.motif_size(), lo, hi));
            if segs.is_none() {
                out.warnings.push(format!("the motif `{id}` can't be carved here (unknown, or too small for so large a fill): plain fill"));
            }
            segs.unwrap_or_default()
        }
        None => vec![],
    };
    let comp = generate::RowComp { pull: s.pull(), pull_percent: s.pull_percent(), push: s.push() };
    let top = match s.fill_style() {
        FillStyle::Tatami => {
            generate::fill_compensated(&polys, i.rule, s.angle(), s.spacing(), s.length(), s.stagger(), &carve, comp, cuts, i.overlap)
        }
        FillStyle::Contour | FillStyle::Spiral => contour_rows(&polys, i, cuts),
    };
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

/// The contour fill's rings as runs (or, for a spiral fill, blended into spirals): the first ring
/// half a row in from the edge, pushed out by half the pull compensation (the rings run along the
/// edge, so the thread draws the fabric in across them as it does across a satin), and cut where
/// objects sewn later lie on top.
fn contour_rows(polys: &[Vec<Point>], i: &Input, cuts: &[generate::Cut]) -> Result<Vec<Vec<Point>>, String> {
    let s = i.settings;
    let first = (s.spacing() - s.pull() - s.spacing() * s.pull_percent() / 100.0) / 2.0;
    let too_small = || format!("the shape is too small for a {} fill: nothing sewn", s.fill_style().id());
    let runs = if s.fill_style() == FillStyle::Spiral {
        contour::spiral(polys, i.rule, s.spacing(), first, s.length()).ok_or_else(too_small)?
    } else {
        contour::runs(contour::rings(polys, i.rule, s.spacing(), first).ok_or_else(too_small)?, s.spacing(), s.length())
    };
    Ok(generate::outside_cuts(runs, cuts))
}
