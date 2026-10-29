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
    /// Needle holes per cell (View › Stitch Density).
    pub(crate) density: std::sync::Arc<vectorcraft_embroidery::check::Density>,
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
        let density = vectorcraft_embroidery::check::Density::of(&plan);
        *cache = Some(StitchMemo {
            doc: doc.clone(),
            plan: std::sync::Arc::new(plan),
            findings: std::sync::Arc::new(findings),
            density: std::sync::Arc::new(density),
        });
    }
    cache.get_or_insert_with(|| StitchMemo { doc: doc.clone(), plan: Default::default(), findings: Default::default(), density: Default::default() })
}

/// Stitch Preview over the canvas (`area`): the stitches, the density map when it is on, and the
/// checker's findings.
pub(super) fn draw_preview(
    app: &mut VectorcraftApp,
    painter: &egui::Painter,
    xf: &Xf,
    doc: &std::sync::Arc<vectorcraft_doc::Document>,
    area: egui::Rect,
    t: &Tokens,
) {
    let density = app.ui.view.stitch_density;
    let memo = stitch_plan(&app.session, &mut app.ui.stitch_cache, doc);
    draw_stitches(painter, xf, doc, &memo.plan);
    if density {
        draw_density(painter, xf, &memo.density, area, t);
    }
    draw_findings(painter, xf, &memo.findings, t);
}

/// View › Stitch Density: each cell of the density map tinted by its needle holes against the
/// checker's limit (green when light, amber near the limit, red past it), and a legend in the
/// canvas's corner (`area`).
pub(super) fn draw_density(painter: &egui::Painter, xf: &Xf, density: &vectorcraft_embroidery::check::Density, area: egui::Rect, t: &Tokens) {
    use vectorcraft_embroidery::check::{DENSITY_CELL_MM, DENSITY_MAX, Density};
    let size = DENSITY_CELL_MM / vectorcraft_embroidery::PT_MM;
    let shade = |holes: usize| {
        let r = holes as f32 / DENSITY_MAX as f32;
        let base = if r > 1.0 {
            t.error
        } else if r >= 0.75 {
            t.warning
        } else {
            t.success
        };
        base.gamma_multiply((0.25 + 0.5 * r.min(1.0)).min(0.75))
    };
    for (&(cx, cy), &holes) in &density.counts {
        // Cells under a quarter of the limit are left clear, so the stitches show.
        if holes * 4 < DENSITY_MAX {
            continue;
        }
        let a = xf.to_screen(Point::new(cx as f64 * size, cy as f64 * size));
        let b = xf.to_screen(Point::new((cx + 1) as f64 * size, (cy + 1) as f64 * size));
        let cell = egui::Rect::from_two_pos(a, b);
        if cell.intersects(area) {
            painter.rect_filled(cell, 0.0, shade(holes));
        }
    }
    // The legend: holes per mm² at a quarter of the limit, three quarters, and past it.
    let font = egui::FontId::proportional(11.0);
    let mut at = area.left_bottom() + egui::vec2(10.0, -14.0);
    let title = tl!("Needle holes per mm²");
    let g = painter.layout_no_wrap(title.to_string(), font.clone(), t.text);
    let rows = [(DENSITY_MAX / 4, "{n}"), (DENSITY_MAX * 3 / 4, "{n}"), (DENSITY_MAX + 1, tl!("over {n}: too dense"))];
    let box_ = egui::Rect::from_min_size(
        area.left_bottom() + egui::vec2(4.0, -24.0 - 16.0 * rows.len() as f32),
        egui::vec2(g.size().x.max(130.0) + 12.0, 20.0 + 16.0 * rows.len() as f32),
    );
    painter.rect_filled(box_, 3.0, t.panel.gamma_multiply(0.9));
    for (holes, label) in rows.iter().rev() {
        let swatch = egui::Rect::from_min_size(at - egui::vec2(0.0, 5.0), egui::vec2(10.0, 10.0));
        painter.rect_filled(swatch, 1.0, shade(*holes));
        let per = if *holes > DENSITY_MAX { Density::per_mm2(DENSITY_MAX) } else { Density::per_mm2(*holes) };
        let text = label.replace("{n}", &format!("{per:.1}"));
        painter.text(at + egui::vec2(16.0, 0.0), egui::Align2::LEFT_CENTER, text, font.clone(), t.text);
        at.y -= 16.0;
    }
    painter.galley(at - egui::vec2(0.0, 7.0), g, t.text);
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
