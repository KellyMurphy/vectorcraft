//! Motif fill: a stitched motif sewn tile by tile across the area on its lattice, every whole tile
//! inside the shape (tiles the edge would cut are left out), row by row, the fabric showing
//! between. Each tile is its own run, so the needle jumps (and trims, per the document's travel
//! settings) between tiles rather than leaving travel stitches across the open fabric.

use kurbo::{Affine, Point};
use vectorcraft_doc::MotifKind;

use super::{Input, Output};
use crate::{generate, motifs};

pub(super) fn sew(i: &Input) -> Output {
    let s = i.settings;
    let mut out = Output::default();
    let polys = i.polys();
    if polys.is_empty() {
        return out;
    }
    let id = s.motif.as_deref().unwrap_or(motifs::DEFAULT_STITCHED);
    let Some(m) = motifs::find(i.motifs, id).filter(|m| m.kind == MotifKind::Stitched) else {
        out.warnings.push(format!("a motif fill needs a stitched motif: `{id}` isn't one this document has; nothing sewn"));
        return out;
    };
    let size = s.motif_size();
    // The lattice turned by the fill's angle (counter-clockwise on the page, y down): the shape is
    // turned back, tiled square to the page, and the tiles turned on to it.
    let a = s.angle().to_radians();
    let (to_page, from_page) = (Affine::rotate(-a), Affine::rotate(a));
    let turned: Vec<Vec<Point>> = polys.iter().map(|l| l.iter().map(|p| from_page * *p).collect()).collect();
    let (mut lo, mut hi) = (Point::new(f64::INFINITY, f64::INFINITY), Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
    for p in turned.iter().flatten() {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let Some(tiles) = motifs::lattice(&m, size, lo, hi) else {
        out.warnings.push(format!("the motif `{id}` is too small for so large an area: make it larger"));
        return out;
    };
    let k = size / m.size;
    // A tile is sewn when every point of it is inside the shape.
    let points: Vec<Point> = m.parts.iter().flat_map(|p| p.points.iter()).map(|&[x, y]| Point::new(x * k, y * k)).collect();
    let mut kept: Vec<Point> = tiles.into_iter().filter(|o| points.iter().all(|q| generate::inside(&turned, i.rule, *o + q.to_vec2()))).collect();
    if kept.is_empty() {
        out.warnings.push(format!("no whole `{id}` motif fits inside the shape: make the motif smaller"));
        return out;
    }
    // Row by row (rows of the lattice's first offset), back and forth.
    let row = |o: &Point| (o.y * 1000.0).round() as i64;
    kept.sort_by(|p, q| row(p).cmp(&row(q)).then(p.x.total_cmp(&q.x)));
    let mut rows: Vec<Vec<Point>> = vec![];
    for o in kept {
        match rows.last_mut() {
            Some(r) if r.last().is_some_and(|l| row(l) == row(&o)) => r.push(o),
            _ => rows.push(vec![o]),
        }
    }
    for (n, mut r) in rows.into_iter().enumerate() {
        if n % 2 == 1 {
            r.reverse();
        }
        for o in r {
            let place = |q: Point| to_page * (o + q.to_vec2());
            let run = motifs::tile_run(&m, k, &place, s.length(), s.spacing());
            if run.len() > 1 {
                out.runs.push(run);
            }
        }
    }
    out
}
