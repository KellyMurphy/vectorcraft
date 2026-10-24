//! Lettering: Auto keeps small letters legible (their holes open), type on a path and letter
//! spacing are sewn as laid out. Tests of installed fonts skip when the font isn't there.

use super::*;

/// A document of one line of `text` in `family` at `size_mm` (its type size), stitched `kind`.
pub(crate) fn type_doc(text: &str, family: &str, size_mm: f64, kind: StitchKind) -> Document {
    let style = vectorcraft_doc::CharStyle { font_family: family.to_string(), size: size_mm / PT_MM, ..Default::default() };
    doc_with(|d| {
        let t = vectorcraft_doc::TextObject::point(Point::new(20.0, 120.0), text, style);
        let mut n = Node::new(d.alloc_id(), vectorcraft_doc::NodeKind::Text(Box::new(t)));
        n.extras_mut().stitch = Some(Box::new(StitchSettings::new(kind)));
        vec![n]
    })
}

pub(crate) fn installed(family: &str) -> bool {
    vectorcraft_text::FontDb::global().has_family(family)
}

fn signed_area(p: &[Point]) -> f64 {
    let n = p.len();
    (0..n).map(|k| p[k].x * p[(k + 1) % n].y - p[(k + 1) % n].x * p[k].y).sum::<f64>() / 2.0
}

fn inside(p: Point, poly: &[Point]) -> bool {
    let mut c = false;
    let n = poly.len();
    for k in 0..n {
        let (a, b) = (poly[k], poly[(k + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            c = !c;
        }
    }
    c
}

/// The holes (counters) of a type document's letters, mm: each a point well inside it and the
/// distance from there to the hole's edge.
pub(crate) fn holes(d: &Document) -> Vec<(Point, f64)> {
    let mut out = vec![];
    d.walk(|n| {
        let vectorcraft_doc::NodeKind::Text(t) = &n.kind else { return };
        for (_, path) in super::super::glyph_parts(t) {
            let polys: Vec<Vec<Point>> = generate::polylines(&path).into_iter().map(|(l, _)| l).filter(|l| l.len() > 2).collect();
            let Some(outer) = polys.iter().max_by(|a, b| signed_area(a).abs().total_cmp(&signed_area(b).abs())) else { continue };
            let sign = signed_area(outer).signum();
            for h in polys.iter().filter(|p| signed_area(p).signum() != sign && signed_area(p).abs() > 0.05) {
                // The grid point inside the hole farthest from its edge.
                let (lo, hi) = h.iter().fold((Point::new(f64::MAX, f64::MAX), Point::new(f64::MIN, f64::MIN)), |(lo, hi), p| {
                    (Point::new(lo.x.min(p.x), lo.y.min(p.y)), Point::new(hi.x.max(p.x), hi.y.max(p.y)))
                });
                let mut best = (lo, 0.0);
                for i in 0..=20 {
                    for j in 0..=20 {
                        let p = Point::new(lo.x + (hi.x - lo.x) * i as f64 / 20.0, lo.y + (hi.y - lo.y) * j as f64 / 20.0);
                        if !inside(p, h) {
                            continue;
                        }
                        let edge = h.iter().zip(h.iter().cycle().skip(1)).map(|(a, b)| seg_dist(p, *a, *b)).fold(f64::MAX, f64::min);
                        if edge > best.1 {
                            best = (p, edge);
                        }
                    }
                }
                if best.1 > 0.0 {
                    out.push(best);
                }
            }
        }
    });
    out
}

fn seg_dist(p: Point, a: Point, b: Point) -> f64 {
    let ab = b - a;
    let t = if ab.hypot2() > 0.0 { ((p - a).dot(ab) / ab.hypot2()).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

/// How far the nearest stitch passes from `p`, mm.
pub(crate) fn clearance(plan: &Plan, p: Point) -> f64 {
    let mut best = f64::MAX;
    for w in plan.needles.windows(2) {
        if w[1].cmd == Cmd::Stitch {
            best = best.min(seg_dist(p, Point::new(w[0].x, w[0].y), Point::new(w[1].x, w[1].y)));
        }
    }
    best
}

/// `EMB_DUMP=1 cargo test -p vectorcraft-embroidery report_counters -- --ignored --nocapture`:
/// for type sizes and fonts, how open Auto keeps the letters' holes.
#[test]
#[ignore]
fn report_counters() {
    for family in ["Impact", "Arial", "Arial Black", "Times New Roman"] {
        if !installed(family) {
            continue;
        }
        for size in [4.0, 5.5, 7.0, 10.0, 15.0] {
            let d = type_doc("ROAD bag", family, size, StitchKind::Auto);
            let p = plan(&d);
            let hs = holes(&d);
            // Holes under 1 mm across close on fabric whatever the stitches do.
            let sewable: Vec<_> = hs.iter().filter(|(_, r)| *r >= 0.5).collect();
            let shut = sewable.iter().filter(|(c, r)| clearance(&p, *c) < (0.5 * r).min(0.3)).count();
            let worst = sewable.iter().map(|(c, r)| clearance(&p, *c) / r).fold(f64::MAX, f64::min);
            println!(
                "{family:16} {size:5.1} mm: {} holes, {} over 1 mm, {shut} of those shut, worst clearance {:.2} of the hole, {} stitches",
                hs.len(),
                sewable.len(),
                worst,
                p.stats().stitches
            );
        }
    }
}

/// Holes this many mm across (twice the distance from their middle to their edge) or more are
/// expected to stay open.
const SEWABLE_HOLE_R: f64 = 0.5;

fn shut_holes(d: &Document) -> (usize, usize) {
    let p = plan(d);
    let hs: Vec<_> = holes(d).into_iter().filter(|(_, r)| *r >= SEWABLE_HOLE_R).collect();
    (hs.iter().filter(|(c, r)| clearance(&p, *c) < (0.5 * r).min(0.3)).count(), hs.len())
}

#[test]
fn auto_lettering_keeps_holes_of_a_millimetre_or_more_open() {
    for (family, sizes) in [("Arial", &[5.5, 10.0, 15.0][..]), ("Times New Roman", &[10.0, 15.0][..]), ("Arial Black", &[5.5, 10.0, 15.0][..])] {
        if !installed(family) {
            continue;
        }
        for &size in sizes {
            let (shut, of) = shut_holes(&type_doc("ROAD bag", family, size, StitchKind::Auto));
            assert_eq!(shut, 0, "{family} at {size} mm: {shut} of {of} holes stitched over");
        }
    }
}

#[test]
fn small_type_is_sewn_as_narrow_satin_not_outlined() {
    if !installed("Times New Roman") {
        return;
    }
    // At 7 mm Times's strokes are under satin's 1 mm threshold: Auto widens them to narrow satin
    // columns, where an outline would be a few long stitches round each letter.
    let auto = plan(&type_doc("bag", "Times New Roman", 7.0, StitchKind::Auto));
    let bean = plan(&type_doc("bag", "Times New Roman", 7.0, StitchKind::Bean));
    assert!(auto.stats().stitches > 2 * bean.stats().stitches / 3, "{} vs {}", auto.stats().stitches, bean.stats().stitches);
    let lens = stitch_lengths(&auto);
    let short = lens.iter().filter(|l| **l < 1.6).count();
    assert!(short * 10 >= lens.len() * 8, "satin columns are short stitches: {short} of {}", lens.len());
}

#[test]
fn auto_border_fits_the_letter() {
    let ring = |r: f64, ccw: bool| -> Vec<Point> {
        let mut v: Vec<Point> = (0..64)
            .map(|k| {
                let a = k as f64 / 64.0 * std::f64::consts::TAU;
                Point::new(r * a.cos(), r * a.sin())
            })
            .collect();
        if !ccw {
            v.reverse();
        }
        v
    };
    // An o 10 mm across with a 6 mm hole: strokes 2 mm, the hole 6 mm.
    let o = vec![ring(5.0, true), ring(3.0, false)];
    let h = narrowest_hole(&o).unwrap();
    assert!((h - 6.0).abs() < 0.2, "the 6 mm hole: {h}");
    let s = StitchSettings::new(StitchKind::Auto);
    let w = auto_border(&s, &o).unwrap();
    assert!(w <= 0.45 * stroke_width(&o) + 1e-9 && w >= AUTO_BORDER_LEAST, "{w}");
    // A thin letter gets none; a border the user set is kept.
    let thin = vec![ring(5.0, true), ring(4.2, false)];
    assert_eq!(auto_border(&s, &thin), None);
    let set = StitchSettings { border: Some(1.5), ..StitchSettings::new(StitchKind::Auto) };
    assert_eq!(auto_border(&set, &thin), Some(1.5));
    assert_eq!(narrowest_hole(&[ring(5.0, true)]), None);
}

#[test]
fn type_on_a_path_is_sewn_along_it() {
    if !installed("Arial") {
        return;
    }
    let r = 30.0 / PT_MM;
    let c = Point::new(250.0, 250.0);
    let style = vectorcraft_doc::CharStyle { font_family: "Arial".into(), size: 8.0 / PT_MM, ..Default::default() };
    let d = doc_with(|d| {
        let mut t = vectorcraft_doc::TextObject::point(Point::ZERO, "ROUND AND ROUND", style);
        let circle = vectorcraft_geom::shapes::ellipse(kurbo::Rect::new(c.x - r, c.y - r, c.x + r, c.y + r));
        t.kind = vectorcraft_doc::TextKind::OnPath { path: circle, start: 0.0, end: None };
        let mut n = Node::new(d.alloc_id(), vectorcraft_doc::NodeKind::Text(Box::new(t)));
        n.extras_mut().stitch = Some(Box::new(StitchSettings::new(StitchKind::Auto)));
        vec![n]
    });
    let p = plan(&d);
    assert!(p.stats().stitches > 500, "{}", p.stats().stitches);
    // Every stitch lies on the ring the letters stand on: within a type size of the circle.
    let centre = Point::new(c.x * PT_MM, c.y * PT_MM);
    for n in p.needles.iter().filter(|n| n.cmd == Cmd::Stitch) {
        let off = (Point::new(n.x, n.y).distance(centre) - 30.0).abs();
        assert!(off <= 8.5, "a stitch {off:.1} mm off the path");
    }
    // And round it: the letters span well over a quarter of the circle.
    let quarters: std::collections::BTreeSet<(bool, bool)> =
        p.needles.iter().filter(|n| n.cmd == Cmd::Stitch).map(|n| (n.x > centre.x, n.y > centre.y)).collect();
    assert!(quarters.len() >= 2, "{quarters:?}");
}

/// The stitched design's width and top, mm.
fn extent(d: &Document) -> (f64, f64) {
    let b = plan(d).stats().bounds.unwrap();
    (b[2] - b[0], b[1])
}

#[test]
fn letter_spacing_and_baseline_shift_move_the_stitches() {
    if !installed("Arial") {
        return;
    }
    let with = |tracking: f64, shift: f64| {
        let mut d = type_doc("IIII", "Arial", 10.0, StitchKind::Auto);
        let id = d.layers[0].children().unwrap()[0].id;
        if let Some(vectorcraft_doc::NodeKind::Text(t)) = d.node_mut(id).map(|n| &mut n.kind) {
            for r in &mut t.runs {
                r.style.tracking = tracking;
                r.style.baseline_shift = shift;
            }
        }
        d
    };
    let (w0, top0) = extent(&with(0.0, 0.0));
    // 500/1000 em more after each letter: three gaps of 5 mm.
    let (w1, _) = extent(&with(500.0, 0.0));
    assert!((w1 - w0 - 15.0).abs() < 0.6, "{w0} → {w1}");
    // A baseline shift of 10 pt lifts every stitch 10 pt.
    let (w2, top2) = extent(&with(0.0, 10.0));
    assert!((w2 - w0).abs() < 0.2);
    assert!(((top0 - top2) - 10.0 * PT_MM).abs() < 0.2, "{top0} → {top2}");
}

#[test]
fn the_checker_warns_of_holes_too_small_to_stay_open() {
    if !installed("Impact") {
        return;
    }
    let d = type_doc("ROAD", "Impact", 10.0, StitchKind::Auto);
    let found = check::check(&d, &plan(&d));
    let f = found.iter().find(|f| f.rule == "small-type").expect("Impact's holes at 10 mm are under 1 mm");
    assert!(f.message.contains("holes") && f.message.contains("mm or more"), "{}", f.message);
    // Arial's are open at 10 mm.
    if installed("Arial") {
        let d = type_doc("ROAD", "Arial", 10.0, StitchKind::Auto);
        assert!(check::check(&d, &plan(&d)).iter().all(|f| f.rule != "small-type"));
    }
}

/// Type set in an embroidery font (`font`) is sewn from the font's letters, needing no outline
/// font; what the font lacks, or a font the document lacks, is warned about.
#[test]
fn type_in_an_embroidery_font_is_sewn_from_its_letters() {
    let with_font = |text: &str, size_mm: f64, font: &str| {
        let mut d = type_doc(text, "No Such Family", size_mm, StitchKind::Auto);
        d.embroidery.fonts = vec![crate::fonts::sample()];
        let layer = Arc::make_mut(&mut d.layers[0]);
        let n = Arc::make_mut(&mut layer.children_mut().unwrap()[0]);
        n.extras_mut().stitch.as_mut().unwrap().font = Some(font.into());
        plan(&d)
    };
    // 20 mm type: caps 14 mm, inside the sample's 6–40 mm.
    let p = with_font("HELLO", 20.0, "vectorcraft-sample-sans");
    assert!(p.stats().stitches > 200, "{}", p.stats().stitches);
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
    let b = p.stats().bounds.unwrap();
    // H E L L O: 650 + 560 + 520 + 520 + 760 units of advance at 20 mm per em, about 60 mm wide, 14 mm tall.
    assert!((b[3] - b[1] - 14.0).abs() < 1.5, "{b:?}");
    assert!(b[2] - b[0] > 50.0 && b[2] - b[0] < 62.0, "{b:?}");
    // Characters it lacks: warned (no outline font here, so left out too).
    let p = with_font("HAT", 20.0, "vectorcraft-sample-sans");
    assert!(p.warnings.iter().any(|w| w.contains("has no `A`")), "{:?}", p.warnings);
    // Too small for the font's sizes.
    let p = with_font("HI", 4.0, "vectorcraft-sample-sans");
    assert!(p.warnings.iter().any(|w| w.contains("is made for capitals 6–40 mm")), "{:?}", p.warnings);
    // A font the document doesn't have.
    let p = with_font("HI", 20.0, "missing-font");
    assert!(p.warnings.iter().any(|w| w.contains("`missing-font`, which the document doesn't have")), "{:?}", p.warnings);
}
