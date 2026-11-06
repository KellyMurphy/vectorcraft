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
/// document changed since), for panels that draw the design; during a drag whose stitches wait
/// for it to end, the design from before it.
pub(crate) fn stitch_memo(app: &mut VectorcraftApp) -> Option<StitchMemo> {
    let doc = app.session.stitches_deferred().or_else(|| Some(app.session.active()?.doc.clone()))?;
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

/// View › Stitch Preview over the canvas (`area`), when it is on: the stitches, the density map
/// when that is on too, and the checker's findings.
pub(super) fn draw_preview(
    app: &mut VectorcraftApp,
    painter: &egui::Painter,
    xf: &Xf,
    doc: &std::sync::Arc<vectorcraft_doc::Document>,
    area: egui::Rect,
    t: &Tokens,
) {
    if !app.ui.view.stitch_preview {
        return;
    }
    let density = app.ui.view.stitch_density;
    // During a tool's drag with Regenerate Stitches: When the Edit Ends, the design from before
    // the drag, and the outlines of the stitched objects the drag changed.
    let before = app.session.stitches_deferred();
    let shown = before.as_ref().unwrap_or(doc);
    let memo = stitch_plan(&app.session, &mut app.ui.stitch_cache, shown).clone();
    if app.ui.view.realistic_stitches {
        draw_realistic(app, painter, xf, shown, &memo.plan);
    } else {
        draw_stitches(painter, xf, shown, &memo.plan);
    }
    draw_hoops(painter, xf, shown, &memo.plan);
    if density {
        draw_density(painter, xf, &memo.density, area, t);
    }
    draw_findings(painter, xf, &memo.findings, t);
    if let Some(before) = &before {
        draw_changed_outlines(painter, xf, before, doc, t);
    }
}

/// The stitched objects of `doc` that differ from `before` (a drag moved or reshaped them), as
/// dashed outlines where they are now: their stitches are sewn again when the drag ends.
fn draw_changed_outlines(painter: &egui::Painter, xf: &Xf, before: &vectorcraft_doc::Document, doc: &vectorcraft_doc::Document, t: &Tokens) {
    let symbols = doc.symbol_boxes();
    let stroke = Stroke::new(1.5, t.accent);
    doc.walk(|n| {
        if n.extras().stitch.is_none() || before.node(n.id).is_some_and(|b| std::ptr::eq(b, n)) {
            return;
        }
        for line in to_screen_path(&super::outline::node_outline(n, &symbols), xf) {
            painter.extend(Shape::dashed_line(&line, stroke, 6.0, 4.0));
        }
    });
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

/// The hoop's sewing field, centred on the design; split into hoopings, one round each hooping's
/// area, numbered in sewing order.
fn draw_hoops(painter: &egui::Painter, xf: &Xf, doc: &vectorcraft_doc::Document, plan: &vectorcraft_embroidery::Plan) {
    use vectorcraft_embroidery::PT_MM;
    let at = |x: f64, y: f64| xf.to_screen(Point::new(x / PT_MM, y / PT_MM));
    let round = doc.embroidery.hoop.as_ref().is_some_and(|h| h.round);
    let fields = vectorcraft_embroidery::preview::hoop_fields(doc, plan);
    let [r, g, b] = vectorcraft_embroidery::preview::HOOP;
    let hoop_color = Color32::from_rgb(r, g, b);
    for (i, (c, w, h)) in fields.iter().enumerate() {
        let outline = vectorcraft_embroidery::preview::hoop_outline(*c, *w, *h, round);
        for line in to_screen_path(&outline.to_bezpath(), xf) {
            painter.extend(Shape::dashed_line(&line, Stroke::new(1.5, hoop_color), 8.0, 5.0));
        }
        if fields.len() > 1 {
            painter.text(at(c.x, c.y), egui::Align2::CENTER_CENTER, (i + 1).to_string(), egui::FontId::proportional(18.0), hoop_color);
        }
    }
}

/// How long the view must stay still before Realistic Stitches draws it again, seconds; until then
/// the last picture is moved with the view.
const REALISTIC_STILL: f64 = 0.15;

/// What View › Realistic Stitches drew, and for which design and view.
#[derive(Clone)]
pub struct RealisticFrame {
    plan: std::sync::Arc<vectorcraft_embroidery::Plan>,
    look: vectorcraft_embroidery::realistic::Look,
    fabric: Vec<vectorcraft_geom::Rect>,
    xf: Xf,
    ppp: f32,
    texture: egui::TextureHandle,
    /// The view last seen, and when it was first seen: the picture is drawn again once it has stayed
    /// put for [`REALISTIC_STILL`].
    seen: Option<(Xf, f64)>,
}

impl std::fmt::Debug for RealisticFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RealisticFrame").field("look", &self.look).field("xf", &self.xf).field("ppp", &self.ppp).finish_non_exhaustive()
    }
}

fn same_view(a: &Xf, b: &Xf) -> bool {
    a.rect == b.rect && a.zoom == b.zoom && a.center == b.center && a.rot == b.rot
}

/// View › Realistic Stitches: the design drawn as sewn thread on the fabric's texture
/// (`vectorcraft_embroidery::realistic`), a picture of the whole canvas. A changed design is drawn
/// at once; a changed view once the view is still, the last picture moving with it until then.
fn draw_realistic(
    app: &mut VectorcraftApp,
    painter: &egui::Painter,
    xf: &Xf,
    doc: &vectorcraft_doc::Document,
    plan: &std::sync::Arc<vectorcraft_embroidery::Plan>,
) {
    use vectorcraft_embroidery::{PT_MM, realistic};
    let ctx = painter.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let now = ctx.input(|i| i.time);
    let look = realistic::Look::of(doc);
    let fabric: Vec<vectorcraft_geom::Rect> = doc.artboards.iter().map(|a| a.rect.scale_from_origin(PT_MM)).collect();
    let cache = &mut app.ui.realistic_cache;
    let design_same = cache.as_ref().is_some_and(|c| std::sync::Arc::ptr_eq(&c.plan, plan) && c.look == look && c.fabric == fabric && c.ppp == ppp);
    let view_same = cache.as_ref().is_some_and(|c| same_view(&c.xf, xf));
    let mut redraw = !design_same || cache.is_none();
    if design_same
        && !view_same
        && let Some(c) = cache.as_mut()
    {
        match c.seen {
            Some((v, since)) if same_view(&v, xf) => {
                let left = REALISTIC_STILL - (now - since);
                if left <= 0.0 {
                    redraw = true;
                } else {
                    ctx.request_repaint_after_secs(left as f32);
                }
            }
            _ => {
                c.seen = Some((*xf, now));
                ctx.request_repaint_after_secs(REALISTIC_STILL as f32);
            }
        }
    }
    if redraw {
        let rect = xf.rect;
        let (w, h) = ((rect.width() * ppp).round().max(1.0) as u32, (rect.height() * ppp).round().max(1.0) as u32);
        let to_px = vectorcraft_geom::Affine::scale(f64::from(ppp))
            * vectorcraft_geom::Affine::translate((-f64::from(rect.min.x), -f64::from(rect.min.y)))
            * xf.affine()
            * vectorcraft_geom::Affine::scale(1.0 / PT_MM);
        let view = realistic::View { width: w, height: h, to_px, fabric: fabric.clone() };
        match realistic::render(plan, &look, &view) {
            Ok(img) => {
                let color = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.rgba);
                let texture = match cache.take() {
                    Some(mut c) => {
                        c.texture.set(color, egui::TextureOptions::LINEAR);
                        c.texture
                    }
                    None => ctx.load_texture("realistic-stitches", color, egui::TextureOptions::LINEAR),
                };
                *cache = Some(RealisticFrame { plan: plan.clone(), look, fabric, xf: *xf, ppp, texture, seen: None });
            }
            // Too big a canvas to draw (or a view that can't be drawn): the flat preview instead.
            Err(_) => {
                *cache = None;
                draw_stitches(painter, xf, doc, plan);
                return;
            }
        }
    }
    let Some(c) = cache.as_ref() else { return };
    // The picture where its view put it, moved into this view.
    let r = c.xf.rect;
    let quad = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()].map(|p| xf.to_screen(c.xf.to_doc(p)));
    super::grid::textured_quad(painter, c.texture.id(), &quad, (1.0, 1.0));
}
