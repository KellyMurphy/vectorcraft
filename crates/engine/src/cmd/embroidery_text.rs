//! Lettering layouts for embroidery (E8.2): type on a circle arc for badges, and monograms.
//!
//! Type set this way is ordinary live type (type on a path, point type), editable as any other,
//! and sewn from its letters as laid out, embroidery fonts included.

use serde_json::{Value, json};
use vectorcraft_doc::{Node, NodeKind, StitchKind, StitchSettings, TextKind, TextObject, TextRun};
use vectorcraft_geom::{Affine, PathData, Point, Rect};

use super::{CommandSpec, EngineError, Result, Session, bad, cmd, has_doc, id_param, str_param};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "embroidery.textArc",
            "Type on an Arc…",
            ["Object", "Embroidery"],
            None,
            "{text?: the words (left out: the selected point type is put on the arc instead), cx, cy: the circle's centre (pt), radius: pt (the band's inner edge), side?: top|bottom (top: read left to right over the top, letters standing outward; bottom: read left to right under the bottom, upright, in the same band), size?, font?, style?, color? (as text.create), tracking?: thousandths of an em (0), stitch?: auto|none (auto: the type is set to be sewn, Auto stitches)} type on a circle arc as type on a path, centred at the top or bottom of the circle, for curved badge lettering in one step: the band runs from the radius outward by the cap height, so top and bottom lines of one size share it → {id}",
            has_doc,
            text_arc
        ),
        cmd!(
            "embroidery.monogram",
            "Monogram…",
            ["Object", "Embroidery"],
            None,
            "{letters: as many as the layout has (with id: its own unless given), layout?: classic (3 letters: a large centre initial between two smaller) | three (3 equal) | two (2 equal) | stacked (3 one above another) | circle (3: the outer letters shorter, to sit in a circle) | diamond (3: the outer letters short, to sit in a diamond) (classic), cx?, cy?: the centre (pt; the artboard's centre, or the monogram replaced), height?: pt (the largest letter's; 144), font?, style?, color? (as text.create), frame?: none|satin|a border pattern's id (none; satin: a 2 mm satin ring; a pattern: that border round it; a circle, a diamond for the diamond layout), stitch?: auto|none (auto), id?: a monogram to lay out again (replaced at its centre; what is left out stays as it was: its recipe is kept in its object data)} a monogram as a group of live type, one point type object per letter, placed and sized by the layout's letter boxes (each still editable), with an optional frame → {id, letters: [ids], frame?}",
            has_doc,
            monogram
        ),
    ]
}

/// The arc's span either side of the top or bottom, degrees: room for long lines.
const SPAN: f64 = 150.0;

fn text_arc(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.textArc";
    let num = |k: &str| p.get(k).and_then(Value::as_f64).filter(|v| v.is_finite());
    let (Some(cx), Some(cy)) = (num("cx"), num("cy")) else { return Err(bad(C, "`cx` and `cy` (the circle's centre) are missing")) };
    let radius = num("radius").ok_or_else(|| bad(C, "`radius` is missing"))?;
    if !(1.0..=100_000.0).contains(&radius) || cx.abs() > 1e7 || cy.abs() > 1e7 {
        return Err(bad(C, "`radius` must be 1–100000 pt and the centre on the page"));
    }
    let top = match str_param(p, "side").unwrap_or("top") {
        "top" => true,
        "bottom" => false,
        o => return Err(bad(C, format!("side must be top|bottom, got {o}"))),
    };
    let stitched = stitch_param(p, C)?;
    let tracking = num("tracking").unwrap_or(0.0);
    if tracking.abs() > 2000.0 {
        return Err(bad(C, "`tracking` must be -2000–2000"));
    }
    // The words: given, or the selected point type's runs.
    let replace = if str_param(p, "text").is_none() {
        let d = s.doc()?;
        let [id] = d.selection.objects.as_slice() else { return Err(bad(C, "give `text`, or select one point type to put on the arc")) };
        let node = d.doc.node(*id).ok_or(EngineError::NoNode(*id))?;
        let NodeKind::Text(t) = &node.kind else { return Err(bad(C, "the selection isn't type: give `text`")) };
        if t.kind != TextKind::Point {
            return Err(bad(C, "the selected type is area type or type on a path already: select point type"));
        }
        Some((*id, t.runs.clone(), node.extras().stitch.clone()))
    } else {
        None
    };
    let mut runs = match &replace {
        Some((_, runs, _)) => runs.clone(),
        None => vec![TextRun { text: str_param(p, "text").unwrap_or("").to_string(), style: super::create::new_type_style(s, p), inline: None }],
    };
    if p.get("tracking").is_some() {
        for r in &mut runs {
            r.style.tracking = tracking;
        }
    }
    let size = runs.first().map_or(12.0, |r| r.style.size);
    let cap = cap_height(&runs).unwrap_or(size * 0.7);
    // Bottom type stands on the band's outer edge, its capitals reaching in to the radius.
    let r = if top { radius } else { radius + cap };
    let (start, sweep) = if top { (270.0 - SPAN, 2.0 * SPAN) } else { (90.0 + SPAN, -2.0 * SPAN) };
    let arc = kurbo::Arc::new(Point::new(cx, cy), (r, r), f64::to_radians(start), f64::to_radians(sweep), 0.0);
    let path = PathData::from_bezpath(&kurbo::Shape::to_path(&arc, 0.01));
    let mut t = TextObject { kind: TextKind::OnPath { path, start: 0.0, end: None }, runs, ..TextObject::point(Point::ZERO, "", Default::default()) };
    t.para = super::create::new_type_para();
    super::create::new_type_alignment(s, &json!({"justify": "center"}), C, &mut t)?;
    super::typecmd::refresh_bounds(&mut t);
    let parent = s.doc()?.target_parent()?;
    let id = s.edit("Type on an Arc", |d, sel| {
        let id = d.alloc_id();
        let mut n = Node::new(id, NodeKind::Text(Box::new(t)));
        n.appearance = vectorcraft_doc::Appearance::default().into();
        n.extras_mut().stitch = match &replace {
            Some((_, _, own)) if own.is_some() => own.clone(),
            _ if stitched => Some(auto()),
            _ => None,
        };
        match &replace {
            Some((old, _, _)) => {
                let (par, idx, _) = d.position(*old).ok_or(EngineError::NoNode(*old))?;
                d.remove(*old)?;
                d.insert(par, idx, n)?;
            }
            None => {
                d.insert(parent, usize::MAX, n)?;
            }
        }
        sel.set([id]);
        Ok(id)
    })?;
    Ok(super::typecmd::with_overflow(s, &[id], json!({"id": id.0})))
}

/// `stitch`: whether new type is set to be sewn (auto) or not (none).
fn stitch_param(p: &Value, c: &str) -> Result<bool> {
    match str_param(p, "stitch").unwrap_or("auto") {
        "auto" => Ok(true),
        "none" => Ok(false),
        o => Err(bad(c, format!("stitch must be auto|none, got {o}"))),
    }
}

/// Auto stitches, as new lettering gets.
fn auto() -> Box<StitchSettings> {
    Box::new(StitchSettings { kind: StitchKind::Auto, ..Default::default() })
}

/// The cap height of type in `runs`' first style, pt: its H's height as laid out (`None` without
/// a font that has one).
fn cap_height(runs: &[TextRun]) -> Option<f64> {
    let style = runs.first()?.style.clone();
    let t = TextObject::point(Point::ZERO, "H", style);
    let b = ink_bounds(&t)?;
    (b.height() > 0.0 && b.height().is_finite()).then_some(b.height())
}

/// The bounds of type `t`'s letters' ink, in its own space (`None` without a font).
fn ink_bounds(t: &TextObject) -> Option<Rect> {
    let lay = vectorcraft_text::layout(vectorcraft_text::FontDb::global(), t);
    lay.glyphs.iter().map(|g| kurbo::Shape::bounding_box(&g.outline)).filter(|b| b.area() > 0.0).reduce(|a, b| a.union(b))
}

/// A monogram layout: one box per letter (centre x, centre y, width, height, in heights of the
/// monogram, round 0, 0), filled by the letters in order, and whether its frame is a diamond
/// (else a circle).
pub(crate) struct Layout {
    pub id: &'static str,
    pub boxes: &'static [[f64; 4]],
    pub diamond: bool,
}

/// The monogram layouts (as `embroidery.monogram`'s params list them).
pub(crate) const LAYOUTS: &[Layout] = &[
    Layout { id: "classic", boxes: &[[-0.7, 0.0, 0.5, 0.62], [0.0, 0.0, 0.75, 1.0], [0.7, 0.0, 0.5, 0.62]], diamond: false },
    Layout { id: "three", boxes: &[[-0.7, 0.0, 0.62, 1.0], [0.0, 0.0, 0.62, 1.0], [0.7, 0.0, 0.62, 1.0]], diamond: false },
    Layout { id: "two", boxes: &[[-0.38, 0.0, 0.68, 1.0], [0.38, 0.0, 0.68, 1.0]], diamond: false },
    Layout { id: "stacked", boxes: &[[0.0, -0.7, 0.8, 0.6], [0.0, 0.0, 0.8, 0.6], [0.0, 0.7, 0.8, 0.6]], diamond: false },
    Layout { id: "circle", boxes: &[[-0.6, 0.0, 0.42, 0.7], [0.0, 0.0, 0.62, 1.0], [0.6, 0.0, 0.42, 0.7]], diamond: false },
    Layout { id: "diamond", boxes: &[[-0.55, 0.0, 0.36, 0.45], [0.0, 0.0, 0.6, 1.0], [0.55, 0.0, 0.36, 0.45]], diamond: true },
];

/// How far a frame stands off the letters' boxes, in heights of the monogram.
const FRAME_GAP: f64 = 0.12;
/// A satin frame's width, mm.
const FRAME_SATIN: f64 = 2.0;
/// The name a monogram group carries.
const MONOGRAM: &str = "Monogram";
/// The keys of a monogram's recipe, kept in its group's object data (`data-*` in SVG): what
/// laying it out again starts from.
pub const RECIPE: [&str; 4] = ["monogram-letters", "monogram-layout", "monogram-height", "monogram-frame"];

/// A monogram's recipe value `key` (one of [`RECIPE`]).
pub fn recipe(n: &Node, key: &str) -> Option<String> {
    let e = n.extras();
    e.attrs.as_deref()?.data.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
}

/// Whether `n` is a monogram `embroidery.monogram` made: a group holding type, with its recipe.
pub fn is_monogram(n: &Node) -> bool {
    recipe(n, "monogram-layout").is_some()
        && matches!(&n.kind, NodeKind::Group { children, .. } if children.iter().any(|c| matches!(c.kind, NodeKind::Text(_))))
}

fn monogram(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.monogram";
    // What it replaces (its recipe the defaults), and where it goes.
    let replace = match p.get("id") {
        Some(_) => {
            let id = id_param(p, "id").ok_or_else(|| bad(C, "`id` must be a monogram's id"))?;
            let n = s.doc()?.doc.node(id).ok_or(EngineError::NoNode(id))?;
            if !is_monogram(n) {
                return Err(bad(C, "`id` isn't a monogram (a group embroidery.monogram made)"));
            }
            let was = |k: &str| recipe(n, k);
            Some((id, n.geometric_bounds().map(|b| b.center()), RECIPE.map(was)))
        }
        None => None,
    };
    let was = |i: usize| replace.as_ref().and_then(|(_, _, r)| r.get(i).cloned().flatten());
    let (old_letters, old_layout, old_height, old_frame) = (was(0), was(1), was(2), was(3));
    let layout_id = str_param(p, "layout").or(old_layout.as_deref()).unwrap_or("classic");
    let layout = LAYOUTS
        .iter()
        .find(|l| l.id == layout_id)
        .ok_or_else(|| bad(C, format!("layout must be {}, got {layout_id}", LAYOUTS.iter().map(|l| l.id).collect::<Vec<_>>().join("|"))))?;
    let letters: Vec<String> =
        str_param(p, "letters").or(old_letters.as_deref()).unwrap_or("").chars().filter(|c| !c.is_whitespace()).map(String::from).collect();
    if letters.len() != layout.boxes.len() {
        return Err(bad(C, format!("the {} layout takes {} letters, got {}", layout.id, layout.boxes.len(), letters.len())));
    }
    let num = |k: &str| p.get(k).and_then(Value::as_f64).filter(|v| v.is_finite());
    let height = num("height").or_else(|| old_height.as_deref().and_then(|h| h.parse::<f64>().ok())).unwrap_or(144.0);
    if !(4.0..=10_000.0).contains(&height) {
        return Err(bad(C, "`height` must be 4–10000 pt"));
    }
    let stitched = stitch_param(p, C)?;
    let frame = str_param(p, "frame").or(old_frame.as_deref()).unwrap_or("none").to_string();
    let pattern = match frame.as_str() {
        "none" | "satin" => None,
        id if vectorcraft_embroidery::borders::builtin(id).is_some() || s.doc()?.doc.embroidery.patterns.iter().any(|b| b.id == id) => {
            Some(id.to_string())
        }
        o => return Err(bad(C, format!("frame must be none|satin or a border pattern's id, got {o}"))),
    };
    let centre = match (num("cx"), num("cy")) {
        (Some(x), Some(y)) => Point::new(x, y),
        _ => replace
            .as_ref()
            .and_then(|(_, c, _)| *c)
            .or_else(|| s.doc().ok().and_then(|d| d.doc.artboards.first().map(|a| a.rect.center())))
            .unwrap_or(Point::new(height, height)),
    };
    if centre.x.abs() > 1e7 || centre.y.abs() > 1e7 {
        return Err(bad(C, "the centre must be on the page"));
    }
    let style = super::create::new_type_style(s, p);
    // Each letter fitted to its box by its ink.
    let mut texts = vec![];
    let mut boxes = Rect::from_points(centre, centre);
    for (letter, [bx, by, bw, bh]) in letters.iter().zip(layout.boxes) {
        let at = Point::new(centre.x + bx * height, centre.y + by * height);
        let (w, h) = (bw * height, bh * height);
        let mut t = TextObject::point(Point::ZERO, letter, vectorcraft_doc::CharStyle { size: 100.0, ..style.clone() });
        t.para = super::create::new_type_para();
        let (size, origin) = match ink_bounds(&t).filter(|b| b.width() > 0.0 && b.height() > 0.0) {
            Some(b) => {
                let k = (h / b.height()).min(w / b.width());
                (100.0 * k, at - b.center().to_vec2() * k)
            }
            // No font to measure: capitals about 0.7 em tall.
            None => (h / 0.7, Point::new(at.x - h * 0.35, at.y + h * 0.5)),
        };
        for r in &mut t.runs {
            r.style.size = size.clamp(0.1, 1296.0);
        }
        t.xf = Affine::translate(origin.to_vec2());
        super::create::new_type_alignment(s, &json!({}), C, &mut t)?;
        super::typecmd::refresh_bounds(&mut t);
        boxes = boxes.union(Rect::from_center_size(at, (w, h)));
        texts.push(t);
    }
    // The frame round them.
    let gap = FRAME_GAP * height;
    let frame_path = (frame != "none").then(|| {
        if layout.diamond {
            vectorcraft_geom::shapes::polygon(centre, boxes.width() / 2.0 + boxes.height() / 2.0 + gap * 1.5, 4, 0.0)
        } else {
            let r = (boxes.width() / 2.0).hypot(boxes.height() / 2.0) + gap;
            vectorcraft_geom::shapes::ellipse(Rect::from_center_size(centre, (2.0 * r, 2.0 * r)))
        }
    });
    let ink = style.fill.clone();
    let parent = s.doc()?.target_parent()?;
    let (gid, ids, frame_id) = s.edit(if replace.is_some() { "Update Monogram" } else { "Monogram" }, |d, sel| {
        let mut children = vec![];
        let mut ids = vec![];
        for t in texts {
            let id = d.alloc_id();
            let mut n = Node::new(id, NodeKind::Text(Box::new(t)));
            n.appearance = vectorcraft_doc::Appearance::default().into();
            if stitched {
                n.extras_mut().stitch = Some(auto());
            }
            ids.push(id);
            children.push(std::sync::Arc::new(n));
        }
        let mut frame_id = None;
        if let Some(path) = frame_path {
            let id = d.alloc_id();
            let mut n = Node::path(
                id,
                path,
                vectorcraft_doc::Appearance::basic(vectorcraft_color::Paint::None, ink, FRAME_SATIN / vectorcraft_embroidery::PT_MM),
            );
            n.name = Some("Frame".into());
            if stitched {
                let kind = if pattern.is_some() { StitchKind::PatternBorder } else { StitchKind::Satin };
                n.extras_mut().stitch = Some(Box::new(StitchSettings { kind, pattern, width: Some(FRAME_SATIN), ..Default::default() }));
            }
            frame_id = Some(id);
            children.push(std::sync::Arc::new(n));
        }
        let gid = d.alloc_id();
        let mut g = Node::group(gid, children);
        g.name = Some(MONOGRAM.into());
        let recipe = [letters.concat(), layout.id.to_string(), format!("{height}"), frame.clone()];
        g.extras_mut().attrs = Some(Box::new(vectorcraft_doc::ObjectAttributes {
            data: RECIPE.iter().map(|k| k.to_string()).zip(recipe).collect(),
            ..Default::default()
        }));
        match &replace {
            Some((old, _, _)) => {
                let (par, idx, _) = d.position(*old).ok_or(EngineError::NoNode(*old))?;
                d.remove(*old)?;
                d.insert(par, idx, g)?;
            }
            None => {
                d.insert(parent, usize::MAX, g)?;
            }
        }
        sel.set([gid]);
        Ok((gid, ids, frame_id))
    })?;
    let mut out = json!({"id": gid.0, "letters": ids.iter().map(|i| i.0).collect::<Vec<_>>()});
    if let Some(f) = frame_id {
        out["frame"] = json!(f.0);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 400, "height": 400})).unwrap();
        s
    }

    fn node(s: &Session, id: &Value) -> Node {
        s.doc().unwrap().doc.node(vectorcraft_doc::NodeId(id.as_u64().unwrap())).unwrap().clone()
    }

    fn text_of(s: &Session, id: &Value) -> TextObject {
        let n = node(s, id);
        let NodeKind::Text(t) = &n.kind else { panic!("not type") };
        (**t).clone()
    }

    /// The middle of type on a path's path.
    fn middle(t: &TextObject) -> Point {
        let TextKind::OnPath { path, .. } = &t.kind else { panic!("on a path") };
        let arc = vectorcraft_geom::ArcPath::new(path);
        arc.at(arc.len() / 2.0).0
    }

    #[test]
    fn top_and_bottom_lines_share_the_band() {
        let mut s = session();
        let top = s.execute("embroidery.textArc", &json!({"text": "TOP", "cx": 200, "cy": 200, "radius": 100, "size": 24})).unwrap();
        let t = text_of(&s, &top["id"]);
        // The path runs over the top: its middle is the circle's top.
        let mid = middle(&t);
        assert!((mid.x - 200.0).abs() < 0.5 && (mid.y - 100.0).abs() < 0.5, "{mid:?}");
        assert_eq!(t.para.justify, vectorcraft_doc::Justify::Center);
        assert_eq!(node(&s, &top["id"]).extras().stitch.as_deref().map(|s| s.kind), Some(StitchKind::Auto));
        let bottom =
            s.execute("embroidery.textArc", &json!({"text": "BOTTOM", "cx": 200, "cy": 200, "radius": 100, "size": 24, "side": "bottom"})).unwrap();
        // Under the bottom, a cap height further out.
        let mid = middle(&text_of(&s, &bottom["id"]));
        assert!((mid.x - 200.0).abs() < 0.5 && mid.y > 300.0 + 10.0 && mid.y < 300.0 + 24.0, "{mid:?}");
        // One undo step each.
        s.execute("edit.undo", &json!({})).unwrap();
        assert!(s.doc().unwrap().doc.node(vectorcraft_doc::NodeId(bottom["id"].as_u64().unwrap())).is_none());
    }

    #[test]
    fn selected_point_type_goes_on_the_arc() {
        let mut s = session();
        let made = s.execute("text.create", &json!({"x": 10, "y": 50, "text": "Club", "size": 30})).unwrap();
        let r = s.execute("embroidery.textArc", &json!({"cx": 200, "cy": 200, "radius": 80, "stitch": "none", "tracking": 100})).unwrap();
        assert!(s.doc().unwrap().doc.node(vectorcraft_doc::NodeId(made["id"].as_u64().unwrap())).is_none(), "replaced");
        let t = text_of(&s, &r["id"]);
        assert_eq!(t.runs.first().map(|r| (r.text.as_str(), r.style.size, r.style.tracking)), Some(("Club", 30.0, 100.0)));
        assert!(node(&s, &r["id"]).extras().stitch.is_none());
    }

    #[test]
    fn bad_arcs_are_refused() {
        let mut s = session();
        for p in [
            json!({"text": "A", "cx": 0, "cy": 0}),
            json!({"text": "A", "cx": 0, "cy": 0, "radius": 0}),
            json!({"text": "A", "cx": 0, "cy": 0, "radius": 50, "side": "left"}),
            json!({"text": "A", "cx": 0, "cy": 0, "radius": 50, "tracking": 1e9}),
            json!({"cx": 0, "cy": 0, "radius": 50}),
        ] {
            assert!(s.execute("embroidery.textArc", &p).is_err(), "{p}");
        }
    }

    #[test]
    fn every_monogram_layout_keeps_its_letters_in_their_boxes() {
        let mut s = session();
        for l in LAYOUTS {
            let letters: String = "ABC".chars().take(l.boxes.len()).collect();
            let r = s
                .execute("embroidery.monogram", &json!({"letters": letters, "layout": l.id, "cx": 200, "cy": 200, "height": 100, "frame": "satin"}))
                .unwrap();
            let ids = r["letters"].as_array().unwrap();
            assert_eq!(ids.len(), l.boxes.len());
            for (id, [bx, by, bw, bh]) in ids.iter().zip(l.boxes) {
                let n = node(&s, id);
                let NodeKind::Text(t) = &n.kind else { panic!("type") };
                assert_eq!(t.kind, TextKind::Point, "editable point type");
                assert_eq!(n.extras().stitch.as_deref().map(|s| s.kind), Some(StitchKind::Auto));
                // Its ink inside its box, filling it one way (where a font lays it out).
                let Some(ink) = ink_bounds(t) else { continue };
                let ink = t.xf.transform_rect_bbox(ink);
                let b = Rect::from_center_size(Point::new(200.0 + bx * 100.0, 200.0 + by * 100.0), (bw * 100.0, bh * 100.0));
                assert!(
                    b.inflate(0.5, 0.5).contains(Point::new(ink.x0, ink.y0)) && b.inflate(0.5, 0.5).contains(Point::new(ink.x1, ink.y1)),
                    "{}: {ink:?} in {b:?}",
                    l.id
                );
                assert!(ink.height() > b.height() - 1.0 || ink.width() > b.width() - 1.0, "{}: {ink:?} {b:?}", l.id);
            }
            assert_eq!(node(&s, &r["frame"]).extras().stitch.as_deref().map(|s| s.kind), Some(StitchKind::Satin));
        }
    }

    #[test]
    fn a_monogram_is_laid_out_again_in_place() {
        let mut s = session();
        let r = s.execute("embroidery.monogram", &json!({"letters": "KLM", "cx": 100, "cy": 100, "height": 60, "frame": "waves"})).unwrap();
        assert_eq!(node(&s, &r["frame"]).extras().stitch.as_deref().map(|s| s.kind), Some(StitchKind::PatternBorder));
        let again = s.execute("embroidery.monogram", &json!({"letters": "KM", "layout": "two", "id": r["id"], "frame": "none"})).unwrap();
        assert!(s.doc().unwrap().doc.node(vectorcraft_doc::NodeId(r["id"].as_u64().unwrap())).is_none());
        let g = node(&s, &again["id"]);
        assert!(is_monogram(&g));
        let c = g.geometric_bounds().unwrap().center();
        assert!((c.x - 100.0).abs() < 3.0 && (c.y - 100.0).abs() < 3.0, "kept its place: {c:?}");
        assert!(again.get("frame").is_none());
        assert_eq!(RECIPE.map(|k| recipe(&g, k)), ["KM", "two", "60", "none"].map(|v| Some(v.to_string())), "the height kept");
        // Only what changes need be given.
        let third = s.execute("embroidery.monogram", &json!({"id": again["id"], "frame": "satin"})).unwrap();
        assert!(third.get("frame").is_some());
        assert_eq!(recipe(&node(&s, &third["id"]), "monogram-letters").as_deref(), Some("KM"));
        // A small one warns that it is too small to read (where a font lays it out).
        let small = s.execute("embroidery.monogram", &json!({"letters": "ABC", "height": 12, "cx": 300, "cy": 300})).unwrap();
        let doc = &s.doc().unwrap().doc;
        let plan = vectorcraft_embroidery::plan(doc);
        if plan.stats().stitches == 0 {
            return;
        }
        let found = vectorcraft_embroidery::check::check(doc, &plan);
        let ids: Vec<u64> = small["letters"].as_array().unwrap().iter().filter_map(Value::as_u64).collect();
        assert!(found.iter().any(|f| f.rule == "small-type" && f.object.is_some_and(|o| ids.contains(&o.0))), "{found:?}");
    }

    #[test]
    fn bad_monograms_are_refused() {
        let mut s = session();
        for p in [
            json!({"letters": "AB"}),
            json!({"letters": "ABC", "layout": "spiral"}),
            json!({"letters": "ABC", "height": 1}),
            json!({"letters": "ABC", "frame": "no-such-pattern"}),
            json!({"letters": "ABC", "id": 99999}),
            json!({"letters": "ABC", "stitch": "satin"}),
        ] {
            assert!(s.execute("embroidery.monogram", &p).is_err(), "{p}");
        }
    }
}
