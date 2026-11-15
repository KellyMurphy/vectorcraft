//! Sew-Out Test panel (Window › Sew-Out Test): calibration test sheets, and the form recording
//! what they showed on fabric. Without a test sheet it offers to make one (`embroidery.testSheet`,
//! with the sections ticked here); with one, it lists the swatches by section, each marked Pass or
//! Fail with a note, and the fabric, machine and notes of the whole sew-out
//! (`embroidery.recordSewOut`). Clicking a swatch selects its objects. A compensation swatch takes
//! the width and length it measures sewn, and the Compensation section shows the compensation
//! they fit (`embroidery.fitCompensation`), to use as the document's fabric or save as a
//! `.vcfabric`. Reads `embroidery.sewOut`.

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
    let mut v = app.session.execute("embroidery.sewOut", &json!({})).unwrap_or(Value::Null);
    // The compensation the measured swatches fit, reported only (null: none measured yet).
    if let Some(o) = v.as_object_mut() {
        o.insert("fit".into(), app.session.execute("embroidery.fitCompensation", &json!({"apply": false})).unwrap_or(Value::Null));
    }
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
            widgets::subheader(ui, &format!("{}  {}", s["letter"].as_str().unwrap_or_default(), section_title(s)));
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
                    if let Some(p) = measurements(ui, w) {
                        record = Some(p);
                    }
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
    if swatches.iter().any(|w| w.get("angle").is_some()) {
        widgets::divider(ui);
        compensation(app, ui, &f["fit"]);
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

/// An opened compensation swatch's sewn width and length, as measured between its marks: the
/// `embroidery.recordSewOut` params of a changed one.
fn measurements(ui: &mut Ui, w: &Value) -> Option<Value> {
    w.get("angle")?;
    let id = w["id"].as_str().unwrap_or_default();
    let mut out = None;
    for (key, drawn, label, tip) in [
        ("measuredWidth", "drawnWidth", tl!("Sewn Width:"), tl!("Along the stitches, between the marks")),
        ("measuredLength", "drawnLength", tl!("Sewn Length:"), tl!("Across the stitches, between the marks")),
    ] {
        ui.horizontal(|ui| {
            ui.add_space(26.0);
            widgets::label_row(ui, label, LABEL_W + 20.0, |ui| {
                if let Some(v) = widgets::mixed_field(ui, ("sew-out-measure", id, key), w[key].as_f64(), " mm", 1, 70.0) {
                    out = Some(json!({"results": [{"swatch": id, key: v}]}));
                }
                if let Some(d) = w[drawn].as_f64() {
                    ui.label(egui::RichText::new(format!("/ {d:.1} mm")).size(11.0).color(Tokens::get(ui.ctx()).text_dim));
                }
            });
        })
        .response
        .on_hover_text(tip);
    }
    out
}

/// One line of the fit: the compensation `f` asks for, led by `what` (an angle, or All).
fn fit_line(what: &str, f: &Value) -> String {
    let (pull, percent) = (f["pull"].as_f64().unwrap_or(0.0), f["pullPercent"].as_f64().unwrap_or(0.0));
    let text = match f["push"].as_f64() {
        Some(_) => tl!("{angle}: pull {pull} mm + {percent} %, push {push} mm"),
        None => tl!("{angle}: pull {pull} mm + {percent} %"),
    };
    text.replace("{angle}", what)
        .replace("{pull}", &format!("{pull:.2}"))
        .replace("{percent}", &format!("{percent:.1}"))
        .replace("{push}", &format!("{:.2}", f["push"].as_f64().unwrap_or(0.0)))
}

/// The compensation the measured swatches fit (`fit`: `embroidery.fitCompensation`'s report, null
/// with none measured): a line for each stitch direction and one for all, and the buttons that
/// make it the document's fabric or save it as a `.vcfabric`.
fn compensation(app: &mut VectorcraftApp, ui: &mut Ui, fit: &Value) {
    widgets::subheader(ui, tl!("Compensation"));
    if fit.is_null() {
        widgets::dim_label(ui, tl!("Click a compensation swatch and enter what it measures sewn, to fit the compensation."));
        return;
    }
    let t = Tokens::get(ui.ctx());
    for a in fit["angles"].as_array().into_iter().flatten() {
        let line = fit_line(&format!("{}°", a["angle"].as_f64().unwrap_or(0.0)), a);
        ui.label(egui::RichText::new(line).size(11.5).color(t.text_dim));
    }
    ui.label(egui::RichText::new(fit_line(tl!("All"), fit)).size(11.5).color(t.text));
    ui.add_space(2.0);
    ui.horizontal_wrapped(|ui| {
        if widgets::flat_button(ui, tl!("Use as Fabric"), 96.0).on_hover_text(tl!("Make the fitted compensation the document's fabric")).clicked() {
            app.run("embroidery.fitCompensation", json!({})).ok();
        }
        if widgets::flat_button(ui, tl!("Save Fabric…"), 96.0)
            .on_hover_text(tl!("Save the fitted fabric as a file (.vcfabric), to load for designs sewn on this fabric"))
            .clicked()
        {
            match app.run("embroidery.fitCompensation", json!({})) {
                Ok(r) => {
                    let id = r["fabric"]["id"].as_str().filter(|i| !i.is_empty()).unwrap_or("fabric").to_string();
                    let file = format!("{id}.{}", vectorcraft_embroidery::fabrics::FILE_EXT);
                    crate::picks::button(app, move |app| {
                        if let Err(e) = crate::io::save_command_output_named(app, "embroidery.saveFabric", &file, json!({})) {
                            app.status(e);
                        }
                    });
                }
                Err(e) => app.status(e),
            }
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
    // A measured compensation swatch: its kind and its drawn and sewn width, short enough to show.
    let label = match (w["measuredWidth"].as_f64(), w["drawnWidth"].as_f64()) {
        (Some(m), Some(d)) => {
            let kind = w["label"].as_str().unwrap_or_default().split(' ').next().unwrap_or_default();
            format!("{kind} {d:.1} → {m:.1} mm")
        }
        _ => w["label"].as_str().unwrap_or_default().to_string(),
    };
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

/// A section's name in the UI's language, with a compensation section's angle (`Compensation 45°`).
fn section_title(s: &Value) -> String {
    let name = tl!(s["name"].as_str().unwrap_or_default());
    match s["angle"].as_f64() {
        Some(a) => format!("{name} {a}°"),
        None => name.to_string(),
    }
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
    // The sections ticked against their default: on ones turned off, the compensation ones on.
    let mut flipped: Vec<String> = pstate(ui.ctx(), "sew-out-flipped");
    let sections = &vectorcraft_embroidery::testsheet::SECTIONS;
    let ticked = |flipped: &[String], s: &vectorcraft_embroidery::testsheet::Section| s.default != flipped.iter().any(|o| o == s.id);
    // The compensation sheet's sections last, under a note of their own.
    let listed = sections.iter().filter(|s| s.angle.is_none()).chain(sections.iter().filter(|s| s.angle.is_some()));
    let mut noted = false;
    for s in listed {
        if s.angle.is_some() && !noted {
            noted = true;
            ui.add_space(4.0);
            widgets::dim_label(
                ui,
                tl!("Compensation sheet: sewn with no compensation, to measure how far the fabric draws in at each stitch direction."),
            );
        }
        let title = match s.angle {
            Some(a) => format!("{} {a}°", tl!(s.name)),
            None => tl!(s.name).to_string(),
        };
        let on = ticked(&flipped, s);
        if widgets::check(ui, &format!("{}  {title}", s.letter), on, true) {
            if flipped.iter().any(|o| o == s.id) {
                flipped.retain(|o| o != s.id);
            } else {
                flipped.push(s.id.to_string());
            }
        }
    }
    let chosen: Vec<&str> = sections.iter().filter(|s| ticked(&flipped, s)).map(|s| s.id).collect();
    set_pstate(ui.ctx(), "sew-out-flipped", flipped.clone());
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

    /// The compensation sections are listed last and unticked; a sheet of them shows each
    /// section's angle.
    #[test]
    fn the_compensation_sections_are_offered_unticked_and_named_by_angle() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let shown = frame(&mut app);
        let at = |want: &str| shown.iter().position(|t| t.contains(want)).unwrap_or_else(|| panic!("{want} in {shown:?}"));
        assert!(at("Puff") < at("Compensation 0°") && at("Compensation 0°") < at("Compensation 135°"), "{shown:?}");
        app.session.execute("embroidery.testSheet", &json!({"sections": ["compensation45"]})).unwrap();
        let shown = frame(&mut app);
        for want in ["L  Compensation 45°", "L1", "Satin 2 mm wide, stitches at 45°", "L5"] {
            assert!(shown.iter().any(|t| t == want), "{want} in {shown:?}");
        }
    }

    /// E10.1b: an opened compensation swatch takes its sewn width and length; once some are
    /// measured, the Compensation section shows the fit for each direction and for all, and Use as
    /// Fabric makes it the document's fabric.
    #[test]
    fn measured_swatches_show_the_fitted_compensation() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.execute("embroidery.testSheet", &json!({"sections": ["compensation0"]})).unwrap();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let shown = |app: &mut VectorcraftApp| {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| show(app, ui));
            out.textures_delta.clear();
            let mut v = vec![];
            out.shapes.iter().for_each(|c| texts(&c.shape, &mut v));
            v
        };
        let first = shown(&mut app);
        assert!(first.iter().any(|t| t.starts_with("Click a compensation swatch")), "{first:?}");
        set_pstate(&ctx, "sew-out-open", "K2".to_string());
        let open = shown(&mut app);
        assert!(open.iter().any(|t| t == "Sewn Width:") && open.iter().any(|t| t == "/ 4.0 mm"), "{open:?}");
        let results: Vec<Value> = [(1, 2.0), (2, 4.0), (3, 6.0), (4, 8.0)]
            .iter()
            .map(|(n, w)| json!({"swatch": format!("K{n}"), "measuredWidth": w - 0.4, "measuredLength": 30.3}))
            .collect();
        app.session.execute("embroidery.recordSewOut", &json!({ "results": results })).unwrap();
        let fitted = shown(&mut app);
        for want in ["0°: pull 0.40 mm + 0.0 %, push 0.30 mm", "All: pull 0.40 mm + 0.0 %, push 0.30 mm", "Use as Fabric", "Save Fabric…"] {
            assert!(fitted.iter().any(|t| t == want), "{want} in {fitted:?}");
        }
        assert!(fitted.iter().any(|t| t == "Satin 4.0 → 3.6 mm"), "a measured row shows its width: {fitted:?}");
    }
}
