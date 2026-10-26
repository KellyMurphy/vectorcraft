//! Sew-Out Test panel (Window › Sew-Out Test): calibration test sheets, and the form recording
//! what they showed on fabric. Without a test sheet it offers to make one (`embroidery.testSheet`,
//! with the sections ticked here); with one, it lists the swatches by section, each marked Pass or
//! Fail with a note, and the fabric, machine and notes of the whole sew-out
//! (`embroidery.recordSewOut`). Clicking a swatch selects its objects. Reads `embroidery.sewOut`.

use egui::{Color32, Rect, Sense, Stroke, Ui, vec2};
use serde_json::{Value, json};

use super::{pstate, set_pstate};
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

pub const ID: &str = "sewOut";
const LABEL_W: f32 = 64.0;
const ROW_H: f32 = 24.0;
const TOGGLE_W: f32 = 38.0;

/// `embroidery.sewOut`, run again only when the document changes.
fn form(app: &mut VectorcraftApp, ctx: &egui::Context) -> Value {
    let Some(st) = app.session.active() else { return Value::Null };
    let key = (st.uid, st.revision);
    let id = egui::Id::unique("sew-out-form");
    if let Some((k, v)) = ctx.data(|d| d.get_temp::<((u64, u64), Value)>(id))
        && k == key
    {
        return v;
    }
    let v = app.session.execute("embroidery.sewOut", &json!({})).unwrap_or(Value::Null);
    ctx.data_mut(|d| d.insert_temp(id, (key, v.clone())));
    v
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let f = form(app, ui.ctx());
    if f["sewOut"].is_null() {
        new_sheet(app, ui);
        return;
    }
    let r = &f["sewOut"];
    let swatches = r["swatches"].as_array().cloned().unwrap_or_default();
    summary(ui, &r["counts"]);
    ui.add_space(4.0);
    for (key, label) in [("fabric", tl!("Fabric:")), ("machine", tl!("Machine:"))] {
        widgets::label_row(ui, label, LABEL_W, |ui| {
            let w = ui.available_width();
            if let Some(text) = widgets::text_field(ui, ("sew-out", key), r[key].as_str(), w, 1) {
                app.run("embroidery.recordSewOut", json!({ key: text })).ok();
            }
        });
    }
    widgets::divider(ui);

    let sections = f["sections"].as_array().cloned().unwrap_or_default();
    let open: String = pstate(ui.ctx(), "sew-out-open");
    let mut opened: Option<String> = None;
    let mut record: Option<Value> = None;
    let height = (ui.available_height() - 150.0).clamp(ROW_H * 4.0, ROW_H * 18.0);
    egui::ScrollArea::vertical().id_salt("sew-out-list").max_height(height).show(ui, |ui| {
        for s in &sections {
            let id = s["id"].as_str().unwrap_or_default();
            let rows: Vec<&Value> = swatches.iter().filter(|w| w["section"] == id).collect();
            if rows.is_empty() {
                continue;
            }
            widgets::subheader(ui, &format!("{}  {}", s["letter"].as_str().unwrap_or_default(), tl!(s["name"].as_str().unwrap_or_default())));
            for w in rows {
                let (click, set) = swatch_row(ui, w, open == w["id"].as_str().unwrap_or_default());
                if let Some(result) = set {
                    record = Some(json!({"results": [{"swatch": w["id"], "result": result}]}));
                }
                if click {
                    let ids = w["objects"].clone();
                    app.run("select.set", json!({ "ids": ids })).ok();
                    let me = w["id"].as_str().unwrap_or_default().to_string();
                    opened = Some(if open == me { String::new() } else { me });
                }
                if open == w["id"].as_str().unwrap_or_default() {
                    ui.horizontal(|ui| {
                        ui.add_space(26.0);
                        let width = ui.available_width() - 4.0;
                        if let Some(text) = widgets::text_field(ui, ("sew-out-note", w["id"].as_str()), w["notes"].as_str(), width, 2) {
                            record = Some(json!({"results": [{"swatch": w["id"], "notes": text}]}));
                        }
                    });
                    ui.add_space(2.0);
                }
            }
        }
    });
    if let Some(o) = opened {
        set_pstate(ui.ctx(), "sew-out-open", o);
    }
    if let Some(p) = record {
        app.run("embroidery.recordSewOut", p).ok();
    }
    widgets::dim_label(ui, tl!("Click a swatch to select it and add a note."));
    widgets::divider(ui);
    widgets::field_label(ui, tl!("Notes:"));
    let w = ui.available_width();
    if let Some(text) = widgets::text_field(ui, ("sew-out", "notes"), r["notes"].as_str(), w, 3) {
        app.run("embroidery.recordSewOut", json!({ "notes": text })).ok();
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let label = super::embroidery::export_label(app);
        if widgets::flat_button(ui, &label, 96.0).on_hover_text(tl!("Each section as a design of its own (one file per artboard)")).clicked() {
            let format = super::embroidery::machine_format(app);
            crate::dialogs::open_export_as(app, Some(format), &json!({"useArtboards": true}));
        }
        if widgets::flat_button(ui, tl!("New Test Sheet…"), 110.0).on_hover_text(tl!("Another test sheet, for another fabric or machine")).clicked()
        {
            app.run("embroidery.testSheet", json!({})).ok();
        }
    });
}

/// The counts and a bar of them: passed, failed, still to judge.
fn summary(ui: &mut Ui, counts: &Value) {
    let t = Tokens::get(ui.ctx());
    let n = |k: &str| counts[k].as_u64().unwrap_or(0);
    let (pass, fail, todo) = (n("pass"), n("fail"), n("untested"));
    let total = (pass + fail + todo).max(1) as f32;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
    ui.painter().rect_filled(rect, 3.0, t.input_border);
    let w = |k: u64| rect.width() * k as f32 / total;
    let pass_r = Rect::from_min_size(rect.min, vec2(w(pass), rect.height()));
    ui.painter().rect_filled(pass_r, 3.0, t.success);
    ui.painter().rect_filled(Rect::from_min_size(pass_r.right_top(), vec2(w(fail), rect.height())), 3.0, t.error);
    ui.add_space(2.0);
    let line = format!("{}: {pass}    {}: {fail}    {}: {todo}", tl!("Pass"), tl!("Fail"), tl!("To judge"));
    ui.label(egui::RichText::new(line).size(11.5).color(t.text_dim));
}

/// A swatch's row: its name and what it tests, a note mark, and the Pass and Fail toggles.
/// → (the row was clicked, the result chosen: `pass`, `fail` or `untested` to clear it).
fn swatch_row(ui: &mut Ui, w: &Value, open: bool) -> (bool, Option<&'static str>) {
    let t = Tokens::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    if open {
        ui.painter().rect_filled(rect, 3.0, t.row_selected);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 3.0, t.hover);
    }
    let font = egui::FontId::proportional(12.0);
    ui.painter().text(
        rect.left_center() + vec2(4.0, 0.0),
        egui::Align2::LEFT_CENTER,
        w["id"].as_str().unwrap_or_default(),
        font.clone(),
        t.text_strong,
    );
    let notes = w["notes"].as_str().unwrap_or_default();
    let label = w["label"].as_str().unwrap_or_default();
    // What it tests, clipped short of the toggles.
    let text_rect = Rect::from_min_max(rect.left_top() + vec2(30.0, 0.0), egui::pos2(rect.right() - 2.0 * (TOGGLE_W + 4.0) - 4.0, rect.bottom()));
    ui.painter_at(text_rect).text(text_rect.left_center(), egui::Align2::LEFT_CENTER, label, font, t.text);
    if !notes.is_empty() && !open {
        let dot = rect.left_center() + vec2(24.0, -6.0);
        ui.painter().circle_filled(dot, 2.5, t.text_dim);
    }
    let result = w["result"].as_str().unwrap_or("untested");
    let mut set = None;
    let mut x = rect.right() - 4.0;
    for (value, text, on) in [("fail", tl!("Fail"), t.error), ("pass", tl!("Pass"), t.success)] {
        let r = Rect::from_min_size(egui::pos2(x - TOGGLE_W, rect.top() + 3.0), vec2(TOGGLE_W, ROW_H - 6.0));
        x -= TOGGLE_W + 4.0;
        let b = ui.interact(r, resp.id.with(value), Sense::click());
        let chosen = result == value;
        let (fill, ink) = if chosen { (on, Color32::WHITE) } else { (t.input, t.text_dim) };
        ui.painter().rect(r, 4.0, fill, Stroke::new(1.0, if chosen { on } else { t.input_border }), egui::StrokeKind::Inside);
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(11.0), ink);
        if b.clicked() {
            set = Some(if chosen { "untested" } else { value });
        }
    }
    (resp.clicked() && set.is_none(), set)
}

/// No test sheet yet: what one is, the sections to put on it, and the button that makes it.
fn new_sheet(app: &mut VectorcraftApp, ui: &mut Ui) {
    widgets::section_header(ui, tl!("Test Sheet"));
    widgets::dim_label(
        ui,
        tl!(
            "A test sheet is a set of calibration swatches to sew on your machine and fabric. Each section is a 100 × 100 mm design; after sewing, mark here what each swatch showed."
        ),
    );
    ui.add_space(4.0);
    let mut off: Vec<String> = pstate(ui.ctx(), "sew-out-off");
    for s in vectorcraft_embroidery::testsheet::SECTIONS {
        let on = !off.iter().any(|o| o == s.id);
        if widgets::check(ui, &format!("{}  {}", s.letter, tl!(s.name)), on, true) {
            if on {
                off.push(s.id.to_string());
            } else {
                off.retain(|o| o != s.id);
            }
        }
    }
    let chosen: Vec<&str> = vectorcraft_embroidery::testsheet::SECTIONS.iter().map(|s| s.id).filter(|id| !off.iter().any(|o| o == id)).collect();
    set_pstate(ui.ctx(), "sew-out-off", off.clone());
    ui.add_space(6.0);
    if widgets::primary_button_enabled(ui, tl!("New Test Sheet"), !chosen.is_empty()).clicked() {
        app.run("embroidery.testSheet", json!({ "sections": chosen })).ok();
    }
    widgets::dim_label(ui, tl!("Lettering needs a type face; without one it is left out."));
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

    fn frame(app: &mut VectorcraftApp) -> Vec<String> {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| show(app, ui));
        out.textures_delta.clear();
        let mut v = vec![];
        for c in &out.shapes {
            texts(&c.shape, &mut v);
        }
        v
    }

    #[test]
    fn the_panel_offers_a_test_sheet_then_shows_its_form() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let shown = frame(&mut app);
        assert!(shown.iter().any(|t| t == "New Test Sheet"), "{shown:?}");
        assert!(shown.iter().any(|t| t.contains("Satin Width")), "{shown:?}");
        app.session.execute("embroidery.testSheet", &json!({"sections": ["satinWidth", "fillAngle"]})).unwrap();
        app.session.execute("embroidery.recordSewOut", &json!({"results": [{"swatch": "A1", "result": "fail"}]})).unwrap();
        let shown = frame(&mut app);
        for want in ["A1", "Satin 1 mm wide", "E4", "Fill angle 135°", "Pass", "Fail"] {
            assert!(shown.iter().any(|t| t == want), "{want} in {shown:?}");
        }
        assert!(shown.iter().any(|t| t.contains("Fail: 1") && t.contains("To judge: 12")), "{shown:?}");
    }
}
