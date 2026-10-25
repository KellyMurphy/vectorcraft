//! The embroidery font editor's documents (E8.1c): an embroidery font laid out for editing, one
//! artboard per glyph, and the font read back from such a document.
//!
//! The font's em is [`EM_MM`] across. Every glyph's artboard stands on one baseline, so ruler guides
//! for the baseline, x-height, cap height, ascender and descender run across them all; an artboard's
//! width is its glyph's advance and its name the glyph's characters (`space` for the space). Glyphs
//! are digitized on the *Letters* layer with the ordinary tools and stitch types; a locked template
//! layer can hold an installed font's letters to trace over, and is never part of the font. The
//! font's own details (name, licence, metrics, kerning) are kept with the document
//! (`document.embroidery.fontEditor`).

use std::sync::Arc;

use kurbo::{Affine, BezPath, Point};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Artboard, Document, EmbroideryFont, FontGlyph, GlyphObject, Guide, LayerColor, Node, NodeKind};
use vectorcraft_geom::{FillRule, PathData, Rect};

use crate::{PT_MM, fonts};

/// One em, mm, in the editor.
pub const EM_MM: f64 = 100.0;
/// Space round each glyph's artboard above the ascender and below the descender, em.
const PAD_EM: f64 = 0.1;
/// Space between artboards, em.
const GAP_EM: f64 = 0.15;
/// The narrowest artboard, em (a glyph with no advance still has room to draw in).
const MIN_WIDTH_EM: f64 = 0.2;
/// The name of the artboard for the space character.
pub const SPACE: &str = "space";
/// The layers of a font document.
pub const LETTERS: &str = "Letters";
pub const TEMPLATE: &str = "Template";
/// The most glyphs a new font document gets.
pub const MAX_NEW_GLYPHS: usize = 512;

/// Points per font unit of `font` in the editor.
fn unit_pt(font: &EmbroideryFont) -> f64 {
    EM_MM / PT_MM / font.units_per_em.max(1.0)
}

/// The baseline's y in a font document of `font`, points.
pub fn baseline(font: &EmbroideryFont) -> f64 {
    (PAD_EM * font.units_per_em + font.ascender) * unit_pt(font)
}

/// The characters `spec` names: each character of it, ranges `A-Z` taken in full; spaces as one
/// glyph, duplicates once.
pub fn chars(spec: &str) -> Vec<String> {
    let cs: Vec<char> = spec.chars().collect();
    let mut out: Vec<String> = vec![];
    let mut k = 0;
    while let Some(&c) = cs.get(k) {
        let range = cs.get(k + 1) == Some(&'-') && cs.get(k + 2).is_some_and(|e| *e > c);
        let push = |out: &mut Vec<String>, c: char| {
            let s = c.to_string();
            if !out.contains(&s) && out.len() < MAX_NEW_GLYPHS {
                out.push(s);
            }
        };
        if range {
            let end = cs.get(k + 2).copied().unwrap_or(c);
            for x in c..=end {
                push(&mut out, x);
            }
            k += 3;
        } else {
            push(&mut out, c);
            k += 1;
        }
    }
    out
}

/// The artboard name for glyph `chars`.
fn board_name(chars: &str) -> String {
    if chars == " " { SPACE.to_string() } else { chars.to_string() }
}

/// The characters artboard `name` stands for.
fn board_chars(name: &str) -> String {
    if name == SPACE { " ".to_string() } else { name.to_string() }
}

/// A traced letter: its outline and advance, em units (y up).
pub type Traced = (BezPath, f64);

/// A font document for `font`: its glyphs (and any of `extra` it lacks, empty, as wide as their
/// traced letter or 0.6 em) each on an artboard, their objects on the Letters layer, and `template`
/// (each character's outline, em units with y up, and advance) on a locked template layer to trace
/// over.
pub fn document(font: &EmbroideryFont, extra: &[String], template: &dyn Fn(&str) -> Option<Traced>) -> Document {
    let u = unit_pt(font);
    let upem = font.units_per_em.max(1.0);
    let base = baseline(font);
    let height = (2.0 * PAD_EM * upem + font.ascender - font.descender) * u;
    let mut glyphs: Vec<FontGlyph> = font.glyphs.clone();
    for c in extra {
        if !glyphs.iter().any(|g| &g.chars == c) {
            let traced = template(c).map(|(_, a)| a).filter(|a| a.is_finite() && *a > 0.0);
            let advance = traced.map_or(if c == " " { 0.3 * upem } else { 0.6 * upem }, |a| a * upem);
            glyphs.push(FontGlyph { chars: c.clone(), advance, ..Default::default() });
        }
    }
    let mut doc = Document::new(EM_MM / PT_MM, height);
    doc.title = format!("{} (embroidery font)", font.name);
    doc.units = vectorcraft_doc::Unit::Millimeters;
    doc.artboards.clear();
    let mut letters: Vec<Arc<Node>> = vec![];
    let mut traced: Vec<Arc<Node>> = vec![];
    let mut x = 0.0;
    for (i, g) in glyphs.iter().enumerate() {
        let width = g.advance.max(MIN_WIDTH_EM * upem) * u;
        doc.artboards.push(Artboard {
            id: u32::try_from(i + 1).unwrap_or(u32::MAX),
            name: board_name(&g.chars),
            rect: Rect::new(x, 0.0, x + width, height),
            show_center_mark: false,
            show_cross_hairs: false,
        });
        // Font units (y up from the baseline) → this artboard (points, y down).
        let to_doc = Affine::new([u, 0.0, 0.0, -u, x, base]);
        for o in &g.objects {
            let Some(path) = fonts::outline(o) else { continue };
            let mut n =
                Node::path(doc.alloc_id(), PathData::from_bezpath(&(to_doc * path)), Appearance::basic(solid([40, 40, 40]), Paint::None, 0.0));
            if o.rule.as_deref() == Some("evenodd")
                && let NodeKind::Path { rule, .. } = &mut n.kind
            {
                *rule = FillRule::EvenOdd;
            }
            n.extras_mut().stitch = Some(Box::new(o.stitch.clone()));
            letters.push(Arc::new(n));
        }
        if let Some((t, _)) = template(&g.chars) {
            let em = Affine::new([upem * u, 0.0, 0.0, -upem * u, x, base]);
            traced.push(Arc::new(Node::path(
                doc.alloc_id(),
                PathData::from_bezpath(&(em * t)),
                Appearance::basic(solid([200, 200, 210]), Paint::None, 0.0),
            )));
        }
        x += width + GAP_EM * upem * u;
    }
    // The metric lines, across every artboard.
    for v in [font.ascender, font.cap_height, font.x_height, 0.0, font.descender] {
        doc.guides.push(Guide { vertical: false, pos: base - v * u, artboard: None, layer: None });
    }
    let mut template_layer = Node::layer(doc.alloc_id(), TEMPLATE, LayerColor::Preset(1));
    if let NodeKind::Layer { template, children, .. } = &mut template_layer.kind {
        *template = true;
        *children = traced;
    }
    template_layer.locked = true;
    let mut letters_layer = Node::layer(doc.alloc_id(), LETTERS, LayerColor::Preset(0));
    if let Some(c) = letters_layer.children_mut() {
        *c = letters;
    }
    doc.layers = vec![Arc::new(template_layer), Arc::new(letters_layer)];
    let mut meta = font.clone();
    meta.glyphs.clear();
    doc.embroidery.font_editor = Some(Box::new(meta));
    doc
}

/// The font a font document makes: its details, and a glyph for each artboard named for one to
/// eight characters, holding the stitched paths of its non-template layers whose middle lies on
/// that artboard.
pub fn font(doc: &Document) -> Result<EmbroideryFont, String> {
    let mut font = doc
        .embroidery
        .font_editor
        .as_deref()
        .cloned()
        .ok_or("this isn't an embroidery font document (embroidery.font.new or embroidery.font.open makes one)")?;
    let u = unit_pt(&font);
    let base = baseline(&font);
    // Each artboard named for a glyph, with that glyph.
    let mut boards: Vec<(Rect, FontGlyph)> = vec![];
    for ab in &doc.artboards {
        let chars = board_chars(ab.name.trim_matches(|c: char| c != ' ' && c.is_whitespace()));
        let n = chars.chars().count();
        if n == 0 || n > fonts::MAX_GLYPH_CHARS || boards.iter().any(|(_, g)| g.chars == chars) {
            continue;
        }
        boards.push((ab.rect, FontGlyph { chars, advance: ab.rect.width() / u, ..Default::default() }));
    }
    // Each stitched object to the glyph whose artboard holds its middle.
    let mut visit = |n: &Node| {
        let Some(stitch) = n.extras().stitch.as_deref() else { return };
        let (path, rule) = match &n.kind {
            NodeKind::Path { path, rule, guide: false, .. } => (path.clone(), *rule),
            NodeKind::Compound { children, rule } => {
                (PathData::new(children.iter().filter_map(|c| c.path_data()).flat_map(|p| p.subpaths.clone()).collect()), *rule)
            }
            _ => return,
        };
        let Some(b) = path.bounds() else { return };
        let mid = Point::new((b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0);
        let Some((ab, g)) = boards.iter_mut().find(|(r, _)| r.contains(mid)).map(|(r, g)| (*r, g)) else { return };
        // Points on this artboard → font units, y up from the baseline.
        let to_font = Affine::new([1.0 / u, 0.0, 0.0, -1.0 / u, -ab.x0 / u, base / u]);
        let bez = to_font * path.to_bezpath();
        g.objects.push(GlyphObject {
            path: bez.to_svg(),
            rule: (rule == FillRule::EvenOdd).then(|| "evenodd".to_string()),
            stitch: stitch.clone(),
            ..Default::default()
        });
    };
    for layer in &doc.layers {
        if matches!(layer.kind, NodeKind::Layer { template: true, .. }) {
            continue;
        }
        layer.walk(&mut visit);
    }
    font.glyphs = boards.into_iter().map(|(_, g)| g).collect();
    fonts::check(&font)?;
    Ok(font)
}

fn solid([r, g, b]: [u8; 3]) -> Paint {
    Paint::solid(Color::rgb8(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_specs_take_ranges_and_lists() {
        assert_eq!(chars("A-E"), ["A", "B", "C", "D", "E"]);
        assert_eq!(chars("ab a"), ["a", "b", " "]);
        assert_eq!(chars("-a"), ["-", "a"], "a dash first is a character");
    }

    #[test]
    fn a_font_round_trips_through_its_document() {
        let f = fonts::sample();
        let doc = document(&f, &[], &|_| None);
        assert_eq!(doc.artboards.len(), f.glyphs.len());
        assert_eq!(doc.artboards[0].name, SPACE);
        assert_eq!(doc.guides.len(), 5);
        let back = font(&doc).unwrap();
        assert_eq!(back.glyphs.len(), f.glyphs.len());
        for (a, b) in f.glyphs.iter().zip(&back.glyphs) {
            assert_eq!((a.chars.as_str(), a.objects.len()), (b.chars.as_str(), b.objects.len()));
            assert!((a.advance - b.advance).abs() < 1e-6, "{} advance", a.chars);
            // The same outline, to rounding.
            for (oa, ob) in a.objects.iter().zip(&b.objects) {
                let (pa, pb) = (fonts::outline(oa).unwrap(), fonts::outline(ob).unwrap());
                let (ba, bb) = (kurbo::Shape::bounding_box(&pa), kurbo::Shape::bounding_box(&pb));
                assert!((ba.x0 - bb.x0).abs() < 1e-6 && (ba.y1 - bb.y1).abs() < 1e-6, "{}: {ba:?} {bb:?}", a.chars);
                assert_eq!(oa.stitch, ob.stitch);
            }
        }
        assert_eq!((back.name.as_str(), back.kerning.len()), (f.name.as_str(), f.kerning.len()));
    }

    #[test]
    fn a_new_font_has_artboards_and_a_template() {
        let mut f = EmbroideryFont { name: "Mine".into(), ..Default::default() };
        f.glyphs.clear();
        let square = |_: &str| {
            let mut b = BezPath::new();
            b.move_to((0.1, 0.0));
            b.line_to((0.5, 0.0));
            b.line_to((0.5, 0.7));
            b.close_path();
            Some((b, 0.6))
        };
        let doc = document(&f, &chars("A-C"), &square);
        assert_eq!(doc.artboards.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["A", "B", "C"]);
        let template = &doc.layers[0];
        assert!(matches!(template.kind, NodeKind::Layer { template: true, .. }) && template.locked);
        assert_eq!(template.children().unwrap().len(), 3);
        // Each artboard as wide as its traced letter's advance (0.6 em here).
        let em_pt = EM_MM / PT_MM;
        assert!(doc.artboards.iter().all(|a| (a.rect.width() - 0.6 * em_pt).abs() < 1e-6));
        // Nothing drawn yet: the template isn't part of the font, so its glyphs are empty.
        let back = font(&doc);
        assert!(back.unwrap().glyphs.iter().all(|g| g.objects.is_empty()));
    }
}
