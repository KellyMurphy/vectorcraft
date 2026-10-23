//! Sew Order panel (Window › Sew Order): the design sewn step by step. A preview shows what is on
//! the fabric after each object (sewn objects in their threads, the current one bright, the rest
//! still to come in grey), a scrubber steps through, and the list below shows each object with a
//! picture of its stitches, dragged or moved with buttons to change the order. The Travel settings
//! (trims and jumps between objects) live here too. Reads `embroidery.order` and the stitch plan;
//! acts through `embroidery.reorder`, `embroidery.sortByThread`, `embroidery.resetOrder` and
//! `embroidery.setup`.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, vec2};
use serde_json::{Value, json};
use vectorcraft_doc::{EmbroiderySetup, StitchKind};
use vectorcraft_geom::Point;

use super::{pstate, set_pstate};
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

pub const ID: &str = "sewOrder";
const LABEL_W: f32 = 112.0;
const FIELD_W: f32 = 76.0;
const ROW_H: f32 = 30.0;
/// The most objects the preview draws (a huge design shows its first ones).
const MAX_PREVIEW_OBJECTS: usize = 2000;

/// What the preview draws for one object: its runs in mm and its thread.
#[derive(Clone)]
struct Drawn {
    id: u64,
    thread: Color32,
    runs: Vec<Vec<Point>>,
}

/// The objects as sewn, from the plan (kept per plan, since a plan outlives many frames).
fn drawn(ctx: &egui::Context, memo: &crate::canvas::StitchMemo) -> std::sync::Arc<Vec<Drawn>> {
    let key = egui::Id::new(("sew-order-drawn", std::sync::Arc::as_ptr(&memo.plan) as usize));
    if let Some(d) = ctx.data(|d| d.get_temp::<std::sync::Arc<Vec<Drawn>>>(key)) {
        return d;
    }
    let plan = &memo.plan;
    // Each object's thread: the colour block its first stitch falls in.
    let mut block = 0usize;
    let mut blocks: Vec<usize> = Vec::with_capacity(plan.needles.len());
    for n in &plan.needles {
        blocks.push(block);
        if n.cmd == vectorcraft_embroidery::Cmd::ColorChange {
            block += 1;
        }
    }
    let list: Vec<Drawn> = plan
        .objects
        .iter()
        .take(MAX_PREVIEW_OBJECTS)
        .map(|o| {
            let b = o.needles.clone().filter_map(|i| blocks.get(i)).next_back().copied().unwrap_or(0);
            let [r, g, bl] = plan.threads.get(b).copied().unwrap_or([0; 3]);
            Drawn { id: o.id.0, thread: Color32::from_rgb(r, g, bl), runs: plan.runs_in(o.needles.clone()) }
        })
        .collect();
    let list = std::sync::Arc::new(list);
    ctx.data_mut(|d| d.insert_temp(key, list.clone()));
    list
}

/// `embroidery.order`, run again only when the document or the selection changes.
fn order(app: &mut VectorcraftApp, ctx: &egui::Context) -> Value {
    let Some(st) = app.session.active() else { return Value::Null };
    let key = (st.uid, st.revision);
    let id = egui::Id::new("sew-order-info");
    if let Some((k, v)) = ctx.data(|d| d.get_temp::<((u64, u64), Value)>(id))
        && k == key
    {
        return v;
    }
    let mut v = app.session.execute("embroidery.order", &json!({})).unwrap_or(Value::Null);
    if let (Some(o), Ok(info)) = (v.as_object_mut(), app.session.execute("embroidery.info", &json!({}))) {
        o.insert("setup".into(), info["setup"].clone());
        o.insert("minutes".into(), info["design"]["minutes"].clone());
    }
    ctx.data_mut(|d| d.insert_temp(id, (key, v.clone())));
    v
}

fn rgb(hex: &str) -> Option<[u8; 3]> {
    let h = hex.trim().trim_start_matches('#');
    let c = |i: usize| h.get(i..i + 2).and_then(|s| u8::from_str_radix(s, 16).ok());
    (h.len() == 6).then_some([c(0)?, c(2)?, c(4)?])
}

/// Fit the plan's box into `rect` with a margin: mm → screen.
fn fit(rect: Rect, bounds: [f64; 4]) -> impl Fn(Point) -> Pos2 {
    let inner = rect.shrink(10.0);
    let (w, h) = ((bounds[2] - bounds[0]).max(0.1), (bounds[3] - bounds[1]).max(0.1));
    let scale = (f64::from(inner.width()) / w).min(f64::from(inner.height()) / h);
    let (cx, cy) = (bounds[0] + w / 2.0, bounds[1] + h / 2.0);
    let (ox, oy) = (f64::from(inner.center().x) - cx * scale, f64::from(inner.center().y) - cy * scale);
    move |p: Point| Pos2::new((p.x * scale + ox) as f32, (p.y * scale + oy) as f32)
}

/// The preview: the fabric with the first `step` objects on it, the `step`th bright.
fn preview(ui: &mut Ui, objects: &[Drawn], bounds: Option<[f64; 4]>, step: usize) {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let h = (w * 0.62).clamp(120.0, 220.0);
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let [r, g, b, a] = vectorcraft_embroidery::preview::FABRIC;
    ui.painter().rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(r, g, b, a));
    ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    let Some(bounds) = bounds else {
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, tl!("Nothing is stitched yet."), egui::FontId::proportional(12.0), t.text_dim);
        return;
    };
    let at = fit(rect, bounds);
    let painter = ui.painter_at(rect);
    // Still to come: faint. Sewn: in their threads. The current one: wider, with a halo.
    let faint = Color32::from_rgba_unmultiplied(90, 80, 70, 70);
    for (i, o) in objects.iter().enumerate() {
        let (width, color) = if i + 1 < step {
            (1.2, o.thread)
        } else if i + 1 == step {
            (2.2, o.thread)
        } else {
            (0.8, faint)
        };
        if i + 1 == step {
            for run in &o.runs {
                painter.add(egui::Shape::line(run.iter().map(|p| at(*p)).collect(), Stroke::new(5.0, Color32::from_white_alpha(120))));
            }
        }
        for run in &o.runs {
            painter.add(egui::Shape::line(run.iter().map(|p| at(*p)).collect(), Stroke::new(width, color)));
        }
    }
}

/// The design's stitches for the player: each run (a stretch sewn without a jump) with its thread
/// and the index of its first needle move, the needle moves where the thread changes, and how
/// many needle moves there are.
struct Needles {
    runs: Vec<(Color32, usize, Vec<Point>)>,
    stops: Vec<usize>,
    total: usize,
}

/// The player's stitches, kept per plan.
fn needles(ctx: &egui::Context, memo: &crate::canvas::StitchMemo) -> std::sync::Arc<Needles> {
    use vectorcraft_embroidery::Cmd;
    let key = egui::Id::new(("sew-player-needles", std::sync::Arc::as_ptr(&memo.plan) as usize));
    if let Some(n) = ctx.data(|d| d.get_temp::<std::sync::Arc<Needles>>(key)) {
        return n;
    }
    let plan = &memo.plan;
    let color = |b: usize| {
        let [r, g, bl] = plan.threads.get(b).copied().unwrap_or([0; 3]);
        Color32::from_rgb(r, g, bl)
    };
    let (mut runs, mut stops) = (vec![], vec![]);
    let mut block = 0usize;
    let mut cur: Option<(usize, Vec<Point>)> = None;
    for (i, n) in plan.needles.iter().enumerate() {
        match n.cmd {
            Cmd::Stitch => cur.get_or_insert_with(|| (i, vec![])).1.push(Point::new(n.x, n.y)),
            Cmd::Jump | Cmd::Trim | Cmd::End | Cmd::ColorChange => {
                if let Some((start, pts)) = cur.take() {
                    runs.push((color(block), start, pts));
                }
                if n.cmd == Cmd::ColorChange {
                    block += 1;
                    stops.push(i);
                }
                // A jump's end is where the next run starts.
                if n.cmd == Cmd::Jump {
                    cur = Some((i, vec![Point::new(n.x, n.y)]));
                }
            }
        }
    }
    if let Some((start, pts)) = cur {
        runs.push((color(block), start, pts));
    }
    let out = std::sync::Arc::new(Needles { runs, stops, total: plan.needles.len() });
    ctx.data_mut(|d| d.insert_temp(key, out.clone()));
    out
}

/// The preview while the player plays: the stitches sewn up to needle move `at`, the needle where
/// it is.
fn played(ui: &mut Ui, n: &Needles, bounds: Option<[f64; 4]>, at: usize) {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let h = (w * 0.62).clamp(120.0, 220.0);
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let [r, g, b, a] = vectorcraft_embroidery::preview::FABRIC;
    ui.painter().rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(r, g, b, a));
    ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    let Some(bounds) = bounds else { return };
    let to = fit(rect, bounds);
    let painter = ui.painter_at(rect);
    let mut needle = None;
    for (color, start, pts) in &n.runs {
        if *start > at {
            break;
        }
        let shown = (at - start + 1).min(pts.len());
        let line: Vec<Pos2> = pts.iter().take(shown).map(|p| to(*p)).collect();
        if let Some(last) = line.last() {
            needle = Some(*last);
        }
        painter.add(egui::Shape::line(line, Stroke::new(1.3, *color)));
    }
    if let Some(p) = needle {
        painter.circle_stroke(p, 5.0, Stroke::new(2.0, t.accent));
    }
}

/// The scrub bar under the preview: a track with a mark per object, the sewn part in the accent
/// colour; click or drag along it to choose how many objects are shown. The step chosen.
fn scrubber(ui: &mut Ui, step: usize, n: usize) -> usize {
    let t = Tokens::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::click_and_drag());
    let resp = resp.on_hover_text(tl!("Step through the sewing"));
    let track = Rect::from_center_size(rect.center(), vec2(rect.width() - 16.0, 4.0));
    let at = |k: usize| track.left() + track.width() * (k as f32 / n as f32);
    ui.painter().rect_filled(track, 2.0, t.input_border);
    ui.painter().rect_filled(Rect::from_min_max(track.left_top(), Pos2::new(at(step), track.bottom())), 2.0, t.accent);
    if n <= 60 {
        for k in 1..n {
            ui.painter().vline(at(k), track.y_range(), Stroke::new(1.0, t.panel));
        }
    }
    let knob = Pos2::new(at(step), track.center().y);
    ui.painter().circle_filled(knob, 6.0, t.accent);
    ui.painter().circle_stroke(knob, 6.0, Stroke::new(1.5, t.panel));
    let mut chosen = step;
    if (resp.clicked() || resp.dragged())
        && let Some(p) = resp.interact_pointer_pos()
    {
        let f = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
        chosen = ((f * n as f32).ceil() as usize).clamp(1, n);
    }
    chosen
}

/// A row of the sewing order dragged in the list: its index.
#[derive(Clone, Copy)]
struct OrderDrag(usize);

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let info = order(app, ui.ctx());
    let rows: Vec<Value> = info["order"].as_array().cloned().unwrap_or_default();
    let Some(memo) = crate::canvas::stitch_memo(app) else {
        super::empty_state(ui, "dc-list-view", tl!("No document"), tl!("Open a document to see its sewing order."));
        return;
    };
    let objects = drawn(ui.ctx(), &memo);
    let bounds = memo.plan.stats().bounds;
    let n = rows.len();

    // The step shown: the object the selection is on (just clicked), else the one kept.
    let selected: Vec<u64> = app.session.active().map(|d| d.selection.objects.iter().map(|i| i.0).collect()).unwrap_or_default();
    let kept: usize = pstate(ui.ctx(), "sew-order-step");
    let mut step = kept.clamp(usize::from(n > 0), n);
    if kept == 0 {
        step = n;
    }
    // The player: playing (or paused part way) it shows the sewing stitch by stitch.
    let stitches = needles(ui.ctx(), &memo);
    let mut play: Option<usize> = pstate(ui.ctx(), "sew-play-at");
    let mut playing: bool = pstate(ui.ctx(), "sew-playing");
    match play.filter(|k| *k < stitches.total) {
        Some(k) => played(ui, &stitches, bounds, k),
        None => {
            play = None;
            playing = false;
            preview(ui, &objects, bounds, step);
        }
    }
    if n > 0 {
        ui.add_space(4.0);
        let before = step;
        step = scrubber(ui, step, n);
        if step != before {
            // Scrubbing by object leaves the player.
            (play, playing) = (None, false);
        }
        ui.horizontal(|ui| {
            for (icon, tip, to) in [("chevrons-left", tl!("First object"), 0usize), ("chevron-left", tl!("Previous object"), 1)] {
                if widgets::icon_button_enabled(ui, icon, tip, false, step > 1, 20.0).clicked() {
                    step = if to == 0 { 1 } else { step - 1 };
                }
            }
            for (icon, tip, to) in [("chevron-right", tl!("Next object"), 1usize), ("chevrons-right", tl!("Last object"), 0)] {
                if widgets::icon_button_enabled(ui, icon, tip, false, step < n, 20.0).clicked() {
                    step = if to == 0 { n } else { step + 1 };
                }
            }
            ui.add_space(6.0);
            let name = rows.get(step.saturating_sub(1)).and_then(|r| r["name"].as_str()).unwrap_or("");
            let stitch =
                rows.get(step.saturating_sub(1)).and_then(|r| r["stitch"].as_str()).and_then(StitchKind::parse).map_or("", |k| tl!(k.label()));
            ui.label(egui::RichText::new(format!("{step} / {n}  ·  {name}  ·  {stitch}")).size(11.0).color(t.text_dim));
        });
        // Play, pause, and step from colour stop to colour stop.
        ui.horizontal(|ui| {
            let label = if playing { tl!("Pause") } else { tl!("Play") };
            if widgets::flat_button(ui, label, 64.0).on_hover_text(tl!("Play the sewing stitch by stitch")).clicked() {
                playing = !playing;
                if playing && play.is_none_or(|k| k + 1 >= stitches.total) {
                    play = Some(0);
                }
            }
            let at = play.unwrap_or(stitches.total);
            let prev = stitches.stops.iter().rev().find(|s| **s + 1 < at).copied();
            if widgets::icon_button_enabled(ui, "chevrons-left", tl!("Previous thread"), false, play.is_some(), 20.0).clicked() {
                play = Some(prev.map_or(0, |s| s + 1));
                playing = false;
            }
            let next = stitches.stops.iter().find(|s| **s > at).copied();
            if widgets::icon_button_enabled(ui, "chevrons-right", tl!("Next thread"), false, next.is_some() && play.is_some(), 20.0).clicked()
                && let Some(s) = next
            {
                play = Some(s + 1);
                playing = false;
            }
            if let Some(k) = play {
                ui.label(egui::RichText::new(format!("{} %", (k * 100) / stitches.total.max(1))).size(11.0).color(t.text_dim));
            }
            if let Some(m) = info["minutes"].as_f64() {
                ui.label(egui::RichText::new(format!("{}: {m:.1} min", tl!("Sewing Time"))).size(11.0).color(t.text_dim))
                    .on_hover_text(tl!("At the machine speed set below, with time for trims and thread changes"));
            }
        });
        if playing {
            // About ten seconds for a whole design.
            let k = play.unwrap_or(0) + (stitches.total / 600).max(1);
            if k + 1 >= stitches.total {
                (play, playing) = (None, false);
            } else {
                play = Some(k);
                ui.ctx().request_repaint();
            }
        }
    }
    set_pstate(ui.ctx(), "sew-play-at", play);
    set_pstate(ui.ctx(), "sew-playing", playing);
    widgets::divider(ui);

    if rows.is_empty() {
        widgets::dim_label(ui, tl!("Give paths stitch settings in the Embroidery panel; they are sewn in this order."));
        if widgets::flat_button(ui, tl!("Embroidery…"), 96.0).clicked() {
            app.run("window.panel", json!({"panel": super::embroidery::ID})).ok();
        }
        return;
    }
    let ids: Vec<u64> = rows.iter().filter_map(|r| r["id"].as_u64()).collect();
    let mut moved: Option<(usize, usize)> = None;
    let mut select: Option<(u64, usize)> = None;
    let height = (ui.available_height() - 110.0).clamp(ROW_H * 3.0, ROW_H * 12.0);
    egui::ScrollArea::vertical().id_salt("sew-order-list").max_height(height).show(ui, |ui| {
        for (i, r) in rows.iter().enumerate() {
            let id = r["id"].as_u64().unwrap_or(0);
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click_and_drag());
            if resp.drag_started() {
                egui::DragAndDrop::set_payload(ui.ctx(), OrderDrag(i));
            }
            if selected.contains(&id) {
                ui.painter().rect_filled(rect, 3.0, t.row_selected);
            } else if resp.hovered() {
                ui.painter().rect_filled(rect, 3.0, t.hover);
            }
            if i + 1 == step {
                ui.painter().vline(rect.left() + 1.0, rect.y_range(), Stroke::new(3.0, t.accent));
            }
            if resp.dnd_hover_payload::<OrderDrag>().is_some() {
                ui.painter().hline(rect.x_range(), rect.top(), Stroke::new(2.0, t.accent));
            }
            if let Some(from) = resp.dnd_release_payload::<OrderDrag>() {
                moved = Some((from.0, i));
            }
            let font = egui::FontId::proportional(12.0);
            ui.painter().text(rect.left_center() + vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, format!("{}", i + 1), font.clone(), t.text_dim);
            // The object's stitches, in its thread.
            let pic = Rect::from_min_size(rect.left_top() + vec2(30.0, 3.0), vec2(40.0, ROW_H - 6.0));
            ui.painter().rect_filled(pic, 2.0, t.input);
            if let Some(o) = objects.iter().find(|o| o.id == id) {
                let thumb = super::stitch_art::Thumb::of(&o.runs);
                super::stitch_art::draw(ui.painter(), pic, &thumb, o.thread, None);
            }
            if let Some(c) = r["thread"].as_str().and_then(rgb) {
                let chip = Rect::from_center_size(rect.left_center() + vec2(82.0, 0.0), vec2(10.0, 10.0));
                ui.painter().rect_filled(chip, 2.0, Color32::from_rgb(c[0], c[1], c[2]));
            }
            // The name, clipped short of the stitch type and count on the right.
            let name_rect = Rect::from_min_max(rect.left_top() + vec2(94.0, 0.0), Pos2::new(rect.right() - 120.0, rect.bottom()));
            ui.painter_at(name_rect).text(name_rect.left_center(), egui::Align2::LEFT_CENTER, r["name"].as_str().unwrap_or(""), font.clone(), t.text);
            let stitch = r["stitch"].as_str().and_then(StitchKind::parse).map_or("", |k| tl!(k.label()));
            let count = r["stitches"].as_u64().unwrap_or(0);
            ui.painter().text(
                rect.right_center() - vec2(6.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                format!("{stitch} · {count}"),
                egui::FontId::proportional(11.0),
                t.text_dim,
            );
            if resp.clicked() {
                select = Some((id, i + 1));
            }
        }
    });
    if let Some((id, at)) = select {
        app.run("select.set", json!({ "ids": [id] })).ok();
        step = at;
    }
    if let Some((from, to)) = moved
        && from != to
    {
        let mut new = ids.clone();
        if let Some(id) = new.get(from).copied() {
            new.remove(from);
            let to = if to > from { to - 1 } else { to };
            new.insert(to.min(new.len()), id);
            app.run("embroidery.reorder", json!({"order": new})).ok();
        }
    }
    set_pstate(ui.ctx(), "sew-order-step", step);

    let any = ids.iter().any(|i| selected.contains(i));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        for (icon, tip, to) in [
            ("dc-al-top", tl!("Sew First"), "first"),
            ("dc-arrow-up", tl!("Sew Earlier"), "earlier"),
            ("dc-arrow-down", tl!("Sew Later"), "later"),
            ("dc-al-bottom", tl!("Sew Last"), "last"),
        ] {
            if widgets::icon_button_enabled(ui, icon, tip, false, any, 22.0).clicked() {
                app.run("embroidery.reorder", json!({ "to": to })).ok();
            }
        }
        ui.add_space(6.0);
        if widgets::flat_button(ui, tl!("Optimize"), 70.0)
            .on_hover_text(tl!("Group by thread and sew each thread's objects nearest first, cutting jumps and trims"))
            .clicked()
        {
            app.run("embroidery.optimizeOrder", json!({})).ok();
        }
        if widgets::flat_button(ui, tl!("By Thread"), 70.0)
            .on_hover_text(tl!("Group the sewing order by thread colour, for fewer thread changes"))
            .clicked()
        {
            app.run("embroidery.sortByThread", json!({})).ok();
        }
        let custom = info["custom"] == json!(true);
        if ui
            .add_enabled_ui(custom, |ui| widgets::flat_button(ui, tl!("Paint Order"), 80.0))
            .inner
            .on_hover_text(tl!("Sew in paint order again (bottom first)"))
            .clicked()
        {
            app.run("embroidery.resetOrder", json!({})).ok();
        }
    });
    widgets::dim_label(ui, tl!("Drag a row to move it. Objects sewn later lie on top."));
    ui.add_space(2.0);
    if widgets::flat_button(ui, tl!("Embroidery…"), 96.0).on_hover_text(tl!("The selection's stitch and its settings")).clicked() {
        app.run("window.panel", json!({"panel": super::embroidery::ID})).ok();
    }
    widgets::divider(ui);
    travel(app, ui, &info["setup"]);
}

/// The document's travel settings: when a jump trims, and how short a move is sewn.
fn travel(app: &mut VectorcraftApp, ui: &mut Ui, setup: &Value) {
    widgets::section_header(ui, tl!("Travel"));
    let trim = setup["trimJumpsOver"].as_f64();
    widgets::label_row(ui, tl!("Trim Jumps Over:"), LABEL_W, |ui| {
        if widgets::check3(ui, "", Some(trim.is_some()), true) {
            let v = if trim.is_some() { Value::Null } else { json!(EmbroiderySetup::DEFAULT_TRIM) };
            app.run("embroidery.setup", json!({ "trimJumpsOver": v })).ok();
        }
        ui.add_enabled_ui(trim.is_some(), |ui| {
            if let Some(v) = widgets::mixed_field(ui, ("emb", "trim"), trim, " mm", 1, FIELD_W) {
                let r = EmbroiderySetup::TRIM;
                app.run("embroidery.setup", json!({ "trimJumpsOver": v.clamp(*r.start(), *r.end()) })).ok();
            }
        });
    });
    widgets::label_row(ui, tl!("Sew Moves Under:"), LABEL_W, |ui| {
        if let Some(v) = widgets::mixed_field(ui, ("emb", "sewMoves"), setup["stitchMovesUnder"].as_f64(), " mm", 1, FIELD_W) {
            let r = EmbroiderySetup::STITCH_MOVES;
            app.run("embroidery.setup", json!({ "stitchMovesUnder": v.clamp(*r.start(), *r.end()) })).ok();
        }
    });
    if trim.is_none() {
        widgets::dim_label(ui, tl!("Only thread changes and objects set to trim cut the thread."));
    }
    let hide = setup["hideTravel"].as_bool().unwrap_or(true);
    if widgets::check_tip(
        ui,
        "Hide Travel Under Fills",
        hide,
        true,
        tl!("Travel that a fill sewn later covers is sewn there as running stitches instead of jumped and trimmed"),
    ) {
        app.run("embroidery.setup", json!({ "hideTravel": !hide })).ok();
    }
    ui.scope(|ui| {
        widgets::label_row(ui, tl!("Machine Speed:"), LABEL_W, |ui| {
            if let Some(v) = widgets::mixed_field(ui, ("emb", "speed"), setup["speed"].as_f64(), " /min", 0, FIELD_W) {
                let r = EmbroiderySetup::SPEED;
                app.run("embroidery.setup", json!({ "speed": v.clamp(*r.start(), *r.end()) })).ok();
            }
        })
    })
    .response
    .on_hover_text(tl!("Stitches per minute, for the sewing time estimate"));
    ui.scope(|ui| {
        widgets::label_row(ui, tl!("Overlap:"), LABEL_W, |ui| {
            if let Some(v) = widgets::mixed_field(ui, ("emb", "overlap"), setup["overlap"].as_f64(), " mm", 1, FIELD_W) {
                let r = EmbroiderySetup::OVERLAP;
                app.run("embroidery.setup", json!({ "overlap": v.clamp(*r.start(), *r.end()) })).ok();
            }
        })
    })
    .response
    .on_hover_text(tl!("How far a fill's rows still reach under objects sewn later on top of it; the rest of the covered rows is cut away"));
    ui.scope(|ui| {
        widgets::label_row(ui, tl!("Shortest Stitch:"), LABEL_W, |ui| {
            if let Some(v) = widgets::mixed_field(ui, ("emb", "minStitch"), setup["minStitch"].as_f64(), " mm", 1, FIELD_W) {
                let r = EmbroiderySetup::MIN_STITCH;
                app.run("embroidery.setup", json!({ "minStitch": v.clamp(*r.start(), *r.end()) })).ok();
            }
        })
    })
    .response
    .on_hover_text(tl!("Needle points closer than this to the one before are left out (the fabric may ask for longer)"));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(s: &egui::Shape, out: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
            _ => {}
        }
    }

    fn frame(app: &mut VectorcraftApp) -> (Vec<String>, usize) {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| show(app, ui));
        out.textures_delta.clear();
        let mut v = vec![];
        let mut lines = 0;
        for c in &out.shapes {
            texts(&c.shape, &mut v);
            fn count(s: &egui::Shape, n: &mut usize) {
                match s {
                    egui::Shape::LineSegment { .. } | egui::Shape::Path(_) => *n += 1,
                    egui::Shape::Vec(v) => v.iter().for_each(|s| count(s, n)),
                    _ => {}
                }
            }
            count(&c.shape, &mut lines);
        }
        (v, lines)
    }

    #[test]
    fn the_list_and_preview_show_the_stitched_objects() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let run = |app: &mut VectorcraftApp, id: &str, p: Value| app.session.execute(id, &p).unwrap();
        run(&mut app, "file.new", json!({"width": 300, "height": 200}));
        let (texts, _) = frame(&mut app);
        assert!(texts.iter().any(|t| t == "Nothing is stitched yet."), "{texts:?}");
        run(&mut app, "shape.rectangle", json!({"x": 10, "y": 10, "width": 100, "height": 60}));
        run(&mut app, "embroidery.set", json!({"type": "fill"}));
        run(&mut app, "select.none", json!({}));
        run(&mut app, "path.create", json!({"d": "M 20 150 L 280 150"}));
        run(&mut app, "embroidery.set", json!({"type": "satin", "width": 3}));
        let (texts, lines) = frame(&mut app);
        assert!(texts.iter().filter(|t| *t == "Path").count() >= 2, "{texts:?}");
        assert!(
            texts.iter().any(|t| t.starts_with("Fill · ")) && texts.iter().any(|t| t.starts_with("Satin Column · ")),
            "each row with its stitch count: {texts:?}"
        );
        assert!(texts.iter().any(|t| t.starts_with("Sewing Time: ")) && texts.iter().any(|t| t == "Play"), "{texts:?}");
        assert!(texts.iter().any(|t| t.starts_with("2 / 2")), "the last step by default: {texts:?}");
        assert!(lines > 10, "the preview and the rows' pictures draw stitches: {lines}");
    }
}
