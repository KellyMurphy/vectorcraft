//! Satin column: a zigzag centred on the path (with underlay), or spanning between two rails.

use kurbo::Point;
use vectorcraft_doc::StitchSettings;

use super::{Input, Output};
use crate::{generate, rails};

pub(super) fn sew(i: &Input) -> Output {
    let s = i.settings;
    let mut out = Output::default();
    let lines = i.lines();
    let angles = crate::angles::lines(s);
    // Capped: a wider stitch can't be sewn in one move (DST: 12.1 mm), and splitting it would put
    // a needle hole in the middle of the column.
    let drawn = s.width(i.stroke_mm);
    let w = (drawn + s.pull_for(drawn)).min(*StitchSettings::WIDTH.end());
    // Between Rails: the object's two longest subpaths are the column's edges (any
    // others crossing both are rungs), the column as wide as they are apart.
    // Stitch angle lines are rungs too.
    let rails = if s.rails {
        let mut with: Vec<(Vec<Point>, bool)> = lines.clone();
        with.extend(angles.iter().map(|[a, b]| (vec![*a, *b], false)));
        rails::find(&with)
    } else {
        None
    };
    if s.rails && rails.is_none() {
        out.warnings.push("a satin set Between Rails needs two subpaths, both open or both closed: sewn along its paths instead".into());
    }
    if let Some(r) = &rails {
        let (run, narrowed) = r.satin(s.spacing(), s.pull(), s.pull_percent(), s.underlay(), &i.auto);
        if narrowed {
            out.warnings
                .push("a satin between rails is wider than a stitch can span (12 mm) in places: narrowed there (fill areas that wide)".into());
        }
        out.runs.push(run);
    }
    let foam = if s.puffed() { s.puff() } else { None };
    for (l, closed) in lines.iter().filter(|_| rails.is_none()) {
        // Push: an open column sewn shorter at both ends. Over foam: longer, capping the foam's
        // cut ends (half its thickness each end).
        let pushed;
        let l = match foam {
            Some(t) if !*closed => {
                pushed = generate::extend_ends(l, t / 2.0);
                &pushed
            }
            _ if *closed || s.push() <= 0.0 => l,
            _ => {
                pushed = generate::trim_ends(l, s.push() / 2.0);
                &pushed
            }
        };
        let (under, back) = generate::satin_underlay_angled(l, *closed, w, s.underlay(), &i.auto, &angles);
        // After an underlay that ends at the far end, the column is sewn back.
        let top = if back {
            let rev: Vec<Point> = l.iter().rev().copied().collect();
            generate::satin_angled(&rev, *closed, w, s.spacing(), &angles)
        } else {
            generate::satin_angled(l, *closed, w, s.spacing(), &angles)
        };
        // One run: the move from the underlay to the column stays inside the column.
        out.runs.push(under.into_iter().chain(top).collect());
        // Over foam: the perforation, all round the column's edge.
        if foam.is_some() {
            out.runs.extend(generate::perforation(l, *closed, w));
        }
    }
    out
}
