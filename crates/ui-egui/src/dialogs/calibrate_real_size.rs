//! View › Calibrate Real Size…: measures the screen's true density for View › Real Size. The
//! dialog draws a bar as long as a reference the user holds against the screen (a bank card's
//! width, 85.60 mm by ISO/IEC 7810 ID-1, or 100 mm of a ruler) at the density it has; dragging the
//! bar's end until it matches the reference, or typing what the bar measures, sets the density.
//! OK keeps it as Preferences › General › Screen Resolution for Real Size (`screenPpi`); Reset
//! goes back to not calibrated (the system's reference density).
//!
//! Fields: `ppi` (device pixels per inch), `reference` (`card` | `ruler`), `measured` (mm, text,
//! what the bar measures; applied with Apply) and `auto` (not calibrated: OK stores 0).

use serde_json::{Value, json};

use super::{DialogResult, DialogSpec, form};
use crate::state::Dialog;
use crate::theme::Tokens;
use crate::{VectorcraftApp, widgets};

/// The dialog kind of Calibrate Real Size.
pub(crate) const KIND: &str = "calibrateRealSize";

pub(super) const SPEC: DialogSpec = DialogSpec {
    heading: |_| tl!("Calibrate Real Size").into(),
    body,
    confirm,
    reset: Some(reset),
    min_width: 420.0,
    inputs: |_| &["ppi", "reference", "measured", "auto"],
    ..DialogSpec::FORM
};

/// The references, their lengths in mm.
const CARD_MM: f64 = 85.60;
const RULER_MM: f64 = 100.0;

/// Width of the label column.
const LABEL_W: f32 = 120.0;

fn reference_mm(d: &Dialog) -> f64 {
    if d.str("reference") == "ruler" { RULER_MM } else { CARD_MM }
}

/// The fields the dialog opens with: the density View › Real Size uses now.
pub(crate) fn fields(app: &VectorcraftApp) -> Value {
    let (ppi, calibrated) = crate::canvas::screen_ppi(app.session.prefs.screen_ppi, app.canvas_density);
    json!({ "ppi": round(ppi), "reference": "card", "measured": "", "auto": !calibrated })
}

/// Open the dialog.
pub(crate) fn open(app: &mut VectorcraftApp) {
    let fields = fields(app);
    app.ui.dialog = Some(Dialog::new(KIND, fields));
}

fn round(ppi: f64) -> f64 {
    (ppi * 10.0).round() / 10.0
}

/// The density in the dialog, kept within the range calibration takes.
fn ppi(d: &Dialog) -> f64 {
    let v = d.f64("ppi", 96.0);
    if v.is_finite() { v.clamp(*crate::canvas::PPI_RANGE.start(), *crate::canvas::PPI_RANGE.end()) } else { 96.0 }
}

fn set_ppi(d: &mut Dialog, ppi: f64) {
    if ppi.is_finite() {
        let ppi = ppi.clamp(*crate::canvas::PPI_RANGE.start(), *crate::canvas::PPI_RANGE.end());
        d.fields.insert("ppi".into(), json!(round(ppi)));
        d.fields.insert("auto".into(), json!(false));
    }
}

fn body(_app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let t = Tokens::get(ui.ctx());
    let ppp = ui.ctx().pixels_per_point().max(0.1);
    let refs = [("card", tl!("Bank Card Width (85.60 mm)")), ("ruler", tl!("100 mm on a Ruler"))];
    form::choice(ui, d, "reference", tl!("Reference"), (LABEL_W, 220.0), &refs);
    ui.add_space(6.0);
    let hint = tl!("Hold the card or ruler against the screen and drag the end of the bar until the bar is exactly as long.");
    ui.add(egui::Label::new(egui::RichText::new(hint).color(t.text_dim)).wrap());
    ui.add_space(8.0);
    // The bar: the reference's length at the dialog's density, in screen points.
    let mm = reference_mm(d);
    let len = (mm / 25.4 * ppi(d) / f64::from(ppp)) as f32;
    let height = 28.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(len + 16.0, height + 14.0), egui::Sense::hover());
    let bar = egui::Rect::from_min_size(rect.min, egui::vec2(len, height));
    let painter = ui.painter_at(rect);
    painter.rect_filled(bar, 2.0, t.accent.gamma_multiply(0.35));
    painter.rect_stroke(bar, 2.0, egui::Stroke::new(1.0, t.accent), egui::StrokeKind::Inside);
    // A tick every 10 mm, longer every 50.
    let per_mm = len / mm as f32;
    let mut k = 0;
    while (k as f32) * 10.0 <= mm as f32 + 0.01 {
        let x = bar.left() + k as f32 * 10.0 * per_mm;
        let h = if k % 5 == 0 { height * 0.6 } else { height * 0.35 };
        painter.line_segment([egui::pos2(x, bar.bottom()), egui::pos2(x, bar.bottom() - h)], egui::Stroke::new(1.0, t.text));
        k += 1;
    }
    // The handle at the bar's end.
    let handle = egui::Rect::from_center_size(egui::pos2(bar.right(), bar.center().y), egui::vec2(12.0, height + 10.0));
    let id = ui.make_persistent_id("calibrate-handle");
    let r = ui.interact(handle, id, egui::Sense::drag()).on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    painter.rect_filled(handle.shrink2(egui::vec2(3.0, 0.0)), 2.0, if r.hovered() || r.dragged() { t.accent } else { t.text });
    if r.dragged() {
        let new_len = f64::from((len + r.drag_delta().x).max(20.0));
        set_ppi(d, new_len * f64::from(ppp) * 25.4 / mm);
    }
    ui.add_space(6.0);
    // Or type it: the density itself, or what the bar measures now.
    widgets::label_row(ui, tl!("Resolution:"), LABEL_W, |ui| {
        if form::text(ui, d, "ppi", 80.0) {
            d.fields.insert("auto".into(), json!(false));
        }
        widgets::dim_label(ui, "ppi");
    });
    widgets::label_row(ui, tl!("The Bar Measures:"), LABEL_W, |ui| {
        form::text(ui, d, "measured", 80.0);
        widgets::dim_label(ui, "mm");
        let measured = d.f64("measured", 0.0);
        let ok = measured.is_finite() && measured > 1.0;
        if ui.add_enabled_ui(ok, |ui| widgets::flat_button(ui, tl!("Apply"), 56.0)).inner.clicked() {
            // The bar drew `mm` at this density but measures `measured`.
            let now = ppi(d);
            set_ppi(d, now * mm / measured);
            d.fields.insert("measured".into(), json!(""));
        }
    });
    if d.bool("auto") {
        ui.add_space(4.0);
        widgets::dim_label(ui, tl!("Not calibrated: the system's display scale is assumed."));
    }
    false
}

fn reset(app: &VectorcraftApp, d: &mut Dialog) {
    let (ppi, _) = crate::canvas::screen_ppi(0.0, app.canvas_density);
    d.fields.insert("ppi".into(), json!(round(ppi)));
    d.fields.insert("measured".into(), json!(""));
    d.fields.insert("auto".into(), json!(true));
}

fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> DialogResult {
    let value = if d.bool("auto") { 0.0 } else { ppi(d) };
    app.run("prefs.set", json!({ "key": "screenPpi", "value": value }))?;
    app.ui.dialog = None;
    Ok(json!({ "screenPpi": value }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> VectorcraftApp {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.canvas_density = (1.25, 1.0);
        app
    }

    #[test]
    fn it_opens_on_the_density_in_use_and_stores_what_was_set() {
        let mut app = app();
        open(&mut app);
        let d = app.ui.dialog.clone().unwrap();
        assert_eq!(d.kind, KIND);
        // Not calibrated: 96 per inch at the 125% display scale.
        assert_eq!(d.f64("ppi", 0.0), 120.0);
        assert!(d.bool("auto"));
        let mut d = d;
        set_ppi(&mut d, 189.7);
        confirm(&mut app, &d).unwrap();
        assert!(app.ui.dialog.is_none());
        assert_eq!(app.session.prefs.screen_ppi, 189.7);
        open(&mut app);
        let mut d = app.ui.dialog.clone().unwrap();
        assert_eq!((d.f64("ppi", 0.0), d.bool("auto")), (189.7, false));
        // Reset: back to not calibrated.
        reset(&app, &mut d);
        confirm(&mut app, &d).unwrap();
        assert_eq!(app.session.prefs.screen_ppi, 0.0);
    }

    #[test]
    fn real_size_zooms_so_a_millimetre_is_a_millimetre() {
        let mut app = app();
        app.session.execute("file.new", &json!({"width": 200, "height": 100})).unwrap();
        app.session.execute("shape.rectangle", &json!({"x": 20, "y": 20, "width": 40, "height": 30})).unwrap();
        app.session.execute("select.none", &json!({})).unwrap();
        // Not calibrated: 96 per inch at the system's scale, a CSS inch: 96 screen points an inch.
        let r = app.run("view.realSize", json!({})).unwrap();
        assert_eq!(r["calibrated"], json!(false));
        assert!((r["zoom"].as_f64().unwrap() - 96.0 / 72.0).abs() < 1e-9, "{r}");
        // A 180 ppi screen drawn at 1.25 device pixels a point: an inch is 144 points.
        app.run("prefs.set", json!({"key": "screenPpi", "value": 180})).unwrap();
        let r = app.run("view.realSize", json!({})).unwrap();
        assert_eq!((r["ppi"].as_f64(), r["calibrated"].as_bool()), (Some(180.0), Some(true)));
        let v = app.view().unwrap();
        assert!((v.zoom - 2.0).abs() < 1e-9, "{}", v.zoom);
        // Centred on the art.
        assert_eq!((v.center.x, v.center.y), (40.0, 35.0));
        // The app's own UI scaling doesn't change the true size.
        app.canvas_density = (2.5, 2.0);
        let r = app.run("view.realSize", json!({})).unwrap();
        assert!((r["zoom"].as_f64().unwrap() - 1.0).abs() < 1e-9, "{r}");
        // An uncalibrated estimate leaves the UI scaling out too.
        app.run("prefs.set", json!({"key": "screenPpi", "value": 0})).unwrap();
        let r = app.run("view.realSize", json!({})).unwrap();
        assert_eq!(r["ppi"].as_f64(), Some(120.0));
    }

    #[test]
    fn a_measured_bar_sets_the_density_and_out_of_range_values_are_held_in() {
        let mut d = Dialog::new(KIND, json!({"ppi": 100, "reference": "ruler", "measured": "", "auto": true}));
        // A 100 mm bar drawn at 100 ppi that measures 80 mm: the screen is denser, 125 ppi.
        let (now, mm) = (ppi(&d), reference_mm(&d));
        set_ppi(&mut d, now * mm / 80.0);
        assert_eq!(ppi(&d), 125.0);
        assert!(!d.bool("auto"));
        set_ppi(&mut d, 5.0);
        assert_eq!(ppi(&d), 30.0);
        d.fields.insert("ppi".into(), json!("lots"));
        assert_eq!(ppi(&d), 96.0);
    }

    #[test]
    fn the_dialog_draws_its_bar_at_the_density() {
        let mut app = app();
        open(&mut app);
        let mut d = app.ui.dialog.clone().unwrap();
        let ctx = egui::Context::default();
        let mut drawn = false;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            drawn = !body(&mut app, ui, &mut d);
        });
        out.textures_delta.clear();
        // The hint and the reference's name are drawn.
        let mut texts = vec![];
        fn collect(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
                _ => {}
            }
        }
        for c in &out.shapes {
            collect(&c.shape, &mut texts);
        }
        assert!(texts.iter().any(|t| t.contains("85.60 mm")), "{texts:?}");
        assert!(drawn);
        assert_eq!(d.str("reference"), "card");
    }
}
