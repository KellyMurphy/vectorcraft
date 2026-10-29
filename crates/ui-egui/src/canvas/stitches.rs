//! View › Stitch Preview: the embroidery design's stitches on fabric over the art, the hoop's
//! sewing field and the design checker's findings.

use super::outline::to_screen_path;
use super::{Color32, Point, Shape, Stroke, Tokens, VectorcraftApp, Xf};

// Stitch Preview's fabric (undyed linen, mostly hiding the art under the stitches) and thread
// width (a common 40-weight thread), shared with the rendered preview (`embroidery.preview`).
use vectorcraft_embroidery::preview::{FABRIC, JUMP_ALPHA, JUMP_WIDTH_PT, THREAD_MM};

/// Stitch Preview's plan of `doc`, made again only when the document changed.
/// Stitch Preview's plan of a document, and the design checker's findings on it.
#[derive(Clone, Debug)]
pub struct StitchMemo {
    doc: std::sync::Arc<vectorcraft_doc::Document>,
    pub(crate) plan: std::sync::Arc<vectorcraft_embroidery::Plan>,
    pub(crate) findings: std::sync::Arc<Vec<vectorcraft_embroidery::check::Finding>>,
}

/// The active document's stitch plan and findings, as Stitch Preview has them (made now if the
/// document changed since), for panels that draw the design.
pub(crate) fn stitch_memo(app: &mut VectorcraftApp) -> Option<StitchMemo> {
    let doc = app.session.active()?.doc.clone();
    Some(stitch_plan(&app.session, &mut app.ui.stitch_cache, &doc).clone())
}

pub(super) fn stitch_plan<'a>(
    session: &vectorcraft_engine::Session,
    cache: &'a mut Option<StitchMemo>,
    doc: &std::sync::Arc<vectorcraft_doc::Document>,
) -> &'a StitchMemo {
    if !cache.as_ref().is_some_and(|m| std::sync::Arc::ptr_eq(&m.doc, doc)) {
        let plan = session.stitch_plan(doc);
        let findings = vectorcraft_embroidery::check::check(doc, &plan);
        *cache = Some(StitchMemo { doc: doc.clone(), plan: std::sync::Arc::new(plan), findings: std::sync::Arc::new(findings) });
    }
    cache.get_or_insert_with(|| StitchMemo { doc: doc.clone(), plan: Default::default(), findings: Default::default() })
}

/// A ring at each finding of the design checker that has a place, in its severity's colour.
pub(super) fn draw_findings(painter: &egui::Painter, xf: &Xf, findings: &[vectorcraft_embroidery::check::Finding], t: &Tokens) {
    use vectorcraft_embroidery::check::Severity;
    for f in findings {
        let Some(at) = f.at else { continue };
        let color = match f.severity {
            Severity::Error => t.error,
            Severity::Warning => t.warning,
            Severity::Note => t.text_dim,
        };
        let p = xf.to_screen(Point::new(at.x / vectorcraft_embroidery::PT_MM, at.y / vectorcraft_embroidery::PT_MM));
        painter.circle_stroke(p, 9.0, Stroke::new(2.0, color));
        painter.circle_stroke(p, 11.0, Stroke::new(1.0, Color32::from_white_alpha(160)));
    }
}

/// A run of stitches drawn as thread: each stitch its own segment, ending at its needle holes. (As
/// one line, the joins of a satin's sharp zigs are mitred into spikes far past the needle holes,
/// which looks much rougher than the stitching is.)
pub(crate) fn thread_stitches(painter: &egui::Painter, pts: &[egui::Pos2], stroke: Stroke) {
    painter.extend(pts.windows(2).filter(|w| w[0] != w[1]).map(|w| Shape::line_segment([w[0], w[1]], stroke)));
}

/// The stitches of `plan` in their thread colours on fabric-coloured artboards, jumps as thin
/// lines.
pub(super) fn draw_stitches(painter: &egui::Painter, xf: &Xf, doc: &vectorcraft_doc::Document, plan: &vectorcraft_embroidery::Plan) {
    use vectorcraft_embroidery::{Cmd, PT_MM};
    let [r, g, b, a] = FABRIC;
    let fabric = Color32::from_rgba_unmultiplied(r, g, b, a);
    for ab in &doc.artboards {
        painter.add(Shape::convex_polygon(xf.quad(ab.rect), fabric, Stroke::NONE));
    }
    let at = |x: f64, y: f64| xf.to_screen(Point::new(x / PT_MM, y / PT_MM));
    let width = ((THREAD_MM / PT_MM) * xf.zoom).clamp(0.5, 8.0) as f32;
    for (thread, run) in plan.runs() {
        let [r, g, b] = plan.threads.get(thread).copied().unwrap_or([0; 3]);
        let pts: Vec<egui::Pos2> = run.iter().map(|p| at(p.x, p.y)).collect();
        thread_stitches(painter, &pts, Stroke::new(width, Color32::from_rgb(r, g, b)));
    }
    // The hoop's sewing field, centred on the design.
    if let Some(hoop) = doc.embroidery.hoop.as_ref().filter(|h| h.unknown.is_none())
        && let Some((c, w, h)) = vectorcraft_embroidery::hoops::field(plan, hoop)
    {
        let outline = vectorcraft_embroidery::preview::hoop_outline(c, w, h, hoop.round);
        let [r, g, b] = vectorcraft_embroidery::preview::HOOP;
        for line in to_screen_path(&outline.to_bezpath(), xf) {
            painter.extend(Shape::dashed_line(&line, Stroke::new(1.5, Color32::from_rgb(r, g, b)), 8.0, 5.0));
        }
    }
    let jump = Stroke::new(JUMP_WIDTH_PT as f32, Color32::from_black_alpha(JUMP_ALPHA));
    let mut from = None;
    for n in &plan.needles {
        let p = at(n.x, n.y);
        if n.cmd == Cmd::Jump
            && let Some(f) = from
        {
            painter.line_segment([f, p], jump);
        }
        from = Some(p);
    }
}
