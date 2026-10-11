//! Embroidery panel (Window › Embroidery): the selected objects' stitch type, thread and the
//! parameters of that type (satin density, width, pull compensation and underlay; fill angle,
//! density, stitch length, pattern and underlay; bean passes), then the whole design's numbers
//! with Stitch Preview and Export DST. Reads `embroidery.info`, acts through `embroidery.set` and
//! `embroidery.clear`.

use egui::{Color32, Ui, vec2};
use serde_json::{Value, json};
use vectorcraft_doc::{EmbroiderySetup, NodeId, StitchKind, StitchSettings, Underlay};

use super::selection_len;
use crate::VectorcraftApp;
use crate::widgets;

pub const ID: &str = "embroidery";
const LABEL_W: f32 = 112.0;
const FIELD_W: f32 = 76.0;

/// A fill pattern's name: the rows before the needle holes line up again (2: brick).
fn pattern_label(rows: u8) -> String {
    if rows == 2 { tl!("Brick").to_string() } else { format!("{} {rows}", tl!("Tatami")) }
}

/// `embroidery.info`, run again only when the document or the selection changes.
fn info(app: &mut VectorcraftApp, ctx: &egui::Context) -> Value {
    type Key = (u64, u64, usize, Option<NodeId>, Option<NodeId>);
    let Some(st) = app.session.active() else { return Value::Null };
    let sel = &st.selection.objects;
    let key: Key = (st.uid, st.revision, sel.len(), sel.first().copied(), sel.last().copied());
    let id = egui::Id::new("embroidery-info");
    if let Some((k, v)) = ctx.data(|d| d.get_temp::<(Key, Value)>(id))
        && k == key
    {
        return v;
    }
    let v = app.session.execute("embroidery.info", &json!({})).unwrap_or(Value::Null);
    ctx.data_mut(|d| d.insert_temp(id, (key, v.clone())));
    v
}

fn set(app: &mut VectorcraftApp, p: Value) {
    app.run("embroidery.set", p).ok();
}

/// `#rrggbb` → sRGB.
fn rgb(hex: &str) -> Option<[u8; 3]> {
    let h = hex.trim().trim_start_matches('#');
    let c = |i: usize| h.get(i..i + 2).and_then(|s| u8::from_str_radix(s, 16).ok());
    (h.len() == 6).then_some([c(0)?, c(2)?, c(4)?])
}

fn chip(ui: &mut Ui, c: [u8; 3], size: f32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(size, size), egui::Sense::hover());
    ui.painter().rect_filled(r, 2.0, Color32::from_rgb(c[0], c[1], c[2]));
    ui.painter().rect_stroke(r, 2.0, egui::Stroke::new(1.0, Color32::from_gray(110)), egui::StrokeKind::Inside);
    resp
}

/// A millimetre field for `key`: blank when the objects differ.
fn mm(app: &mut VectorcraftApp, ui: &mut Ui, label: &str, key: &str, value: Option<f64>, range: std::ops::RangeInclusive<f64>, tip: &str) {
    ui.scope(|ui| {
        widgets::label_row(ui, label, LABEL_W, |ui| {
            if let Some(v) = widgets::mixed_field(ui, ("emb", key), value, " mm", 2, FIELD_W) {
                set(app, json!({ key: v.clamp(*range.start(), *range.end()) }));
            }
        })
    })
    .response
    .on_hover_text(tip);
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let info = info(app, ui.ctx());
    let sel = &info["selection"];
    let stitched = sel["stitched"].as_u64().unwrap_or(0);
    let unstitched = sel["unstitched"].as_u64().unwrap_or(0);
    let any = stitched + unstitched > 0;
    let kind = sel["type"].as_str().and_then(StitchKind::parse).filter(|_| unstitched == 0);

    // Stitch type.
    widgets::label_row(ui, tl!("Stitch:"), LABEL_W, |ui| {
        ui.add_enabled_ui(any, |ui| {
            let mut labels = vec![tl!("Not Stitched")];
            labels.extend(StitchKind::ALL.iter().map(|k| tl!(k.label())));
            let current = match (kind, stitched, unstitched) {
                (Some(k), _, _) => tl!(k.label()),
                (None, 0, _) => tl!("Not Stitched"),
                _ => "",
            };
            match widgets::dropdown(ui, "emb-kind", current, &labels, 150.0) {
                Some(0) => {
                    app.run("embroidery.clear", json!({})).ok();
                }
                Some(i) => {
                    if let Some(k) = StitchKind::ALL.get(i - 1) {
                        set(app, json!({"type": k.id()}));
                    }
                }
                None => {}
            }
        });
    });
    if !any {
        widgets::dim_label(
            ui,
            if selection_len(app) == 0 { tl!("Select paths to stitch.") } else { tl!("Only paths and compound paths can be stitched.") },
        );
    } else if stitched > 0 && unstitched == 0 {
        thread_row(app, ui, sel);
        trim_row(app, ui, sel);
        widgets::divider(ui);
        match kind {
            Some(k) => parameters(app, ui, k, sel),
            None => {
                widgets::dim_label(ui, tl!("The selected objects use different stitches."));
            }
        }
    } else if stitched > 0 {
        widgets::dim_label(ui, tl!("Some selected objects aren't stitched."));
    }
    widgets::divider(ui);
    travel(app, ui, &info["setup"]);
    widgets::divider(ui);
    design(app, ui, &info["design"]);
}

/// Trim Before / Trim After for the selected objects (blank where they differ).
fn trim_row(app: &mut VectorcraftApp, ui: &mut Ui, sel: &Value) {
    widgets::label_row(ui, tl!("Trim:"), LABEL_W, |ui| {
        for (label, key, tip) in [
            (tl!("Before"), "trimBefore", tl!("Cut the thread before this object, however close it starts")),
            (tl!("After"), "trimAfter", tl!("Cut the thread after this object")),
        ] {
            let v = sel[key].as_bool();
            if widgets::check3_tip(ui, label, v, true, tip) {
                set(app, json!({ key: v != Some(true) }));
            }
            ui.add_space(6.0);
        }
    });
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
}

fn thread_row(app: &mut VectorcraftApp, ui: &mut Ui, sel: &Value) {
    let thread = sel["thread"].as_str().and_then(rgb);
    widgets::label_row(ui, tl!("Thread:"), LABEL_W, |ui| {
        match thread {
            Some(c) => {
                chip(ui, c, 18.0);
            }
            None => {
                ui.label(tl!("Object color")).on_hover_text(tl!("The stroke's color for stitches along the path, the fill's for fills"));
            }
        }
        let hex = sel["thread"].as_str().unwrap_or("").trim_start_matches('#').to_string();
        if let Some(h) = widgets::hex_field(ui, "emb-thread", &hex)
            && rgb(&h).is_some()
        {
            set(app, json!({"thread": format!("#{}", h.trim_start_matches('#'))}));
        }
        if thread.is_some() && widgets::flat_button(ui, tl!("Use Object Color"), 110.0).clicked() {
            set(app, json!({"thread": null}));
        }
    });
}

fn parameters(app: &mut VectorcraftApp, ui: &mut Ui, kind: StitchKind, sel: &Value) {
    let num = |k: &str| sel[k].as_f64();
    match kind {
        StitchKind::Running | StitchKind::Bean => {
            mm(app, ui, tl!("Stitch Length:"), "length", num("length"), StitchSettings::LENGTH, tl!("How long each stitch is"));
            if kind == StitchKind::Bean {
                widgets::label_row(ui, tl!("Passes:"), LABEL_W, |ui| {
                    let opts = ["3", "5", "7", "9"];
                    let cur = sel["passes"].as_u64().map(|p| p.to_string()).unwrap_or_default();
                    if let Some(i) = widgets::dropdown_names(ui, "emb-passes", &cur, &opts, FIELD_W) {
                        set(app, json!({"passes": 3 + 2 * i}));
                    }
                });
            }
        }
        StitchKind::Satin => {
            widgets::section_header(ui, tl!("Satin Column"));
            widgets::label_row(ui, tl!("Width:"), LABEL_W, |ui| {
                let width = sel["width"].as_f64();
                if let Some(v) = widgets::mixed_field(ui, ("emb", "width"), width, " mm", 2, FIELD_W) {
                    let r = StitchSettings::WIDTH;
                    set(app, json!({"width": v.clamp(*r.start(), *r.end())}));
                }
                if width.is_none() {
                    widgets::dim_label(ui, tl!("the stroke width"));
                } else if widgets::flat_button(ui, tl!("Use Stroke"), 80.0).clicked() {
                    set(app, json!({"width": null}));
                }
            });
            mm(app, ui, tl!("Density:"), "spacing", num("spacing"), StitchSettings::SPACING, tl!("Distance between zigzags: smaller is denser"));
            mm(
                app,
                ui,
                tl!("Pull Compensation:"),
                "pull",
                num("pull"),
                StitchSettings::PULL,
                tl!("Sews the column this much wider, since the thread pulls the fabric in"),
            );
            underlay_row(app, ui, kind, sel);
        }
        StitchKind::Fill => {
            widgets::section_header(ui, tl!("Fill"));
            widgets::label_row(ui, tl!("Angle:"), LABEL_W, |ui| {
                if let Some(v) = widgets::mixed_field(ui, ("emb", "angle"), num("angle"), "°", 1, FIELD_W) {
                    set(app, json!({"angle": v}));
                }
            });
            mm(app, ui, tl!("Density:"), "spacing", num("spacing"), StitchSettings::SPACING, tl!("Distance between rows: smaller is denser"));
            mm(app, ui, tl!("Stitch Length:"), "length", num("length"), StitchSettings::LENGTH, tl!("How long each stitch is"));
            widgets::label_row(ui, tl!("Pattern:"), LABEL_W, |ui| {
                let rows: Vec<u8> = StitchSettings::STAGGER.collect();
                let names: Vec<String> = rows.iter().map(|r| pattern_label(*r)).collect();
                let labels: Vec<&str> = names.iter().map(String::as_str).collect();
                let cur = sel["stagger"].as_u64().and_then(|s| u8::try_from(s).ok()).map(pattern_label).unwrap_or_default();
                if let Some(n) = widgets::dropdown_names(ui, "emb-pattern", &cur, &labels, 150.0).and_then(|i| rows.get(i)) {
                    set(app, json!({"stagger": n}));
                }
            });
            underlay_row(app, ui, kind, sel);
        }
        StitchKind::Manual => {
            widgets::dim_label(ui, tl!("The needle goes down at every anchor point, as drawn."));
        }
    }
}

/// An underlay's name, translated.
fn underlay_label(u: Underlay) -> &'static str {
    match u {
        Underlay::None => tl!("None"),
        Underlay::Center => tl!("Center Walk"),
        Underlay::Edge => tl!("Edge Walk"),
        Underlay::Zigzag => tl!("Zigzag"),
        Underlay::Fill => tl!("Fill Underlay"),
    }
}

fn underlay_row(app: &mut VectorcraftApp, ui: &mut Ui, kind: StitchKind, sel: &Value) {
    let options = Underlay::for_kind(kind);
    widgets::label_row(ui, tl!("Underlay:"), LABEL_W, |ui| {
        let labels: Vec<&str> = options.iter().map(|u| underlay_label(*u)).collect();
        let cur = sel["underlay"].as_str().and_then(Underlay::parse).map_or("", underlay_label);
        if let Some(u) = widgets::dropdown(ui, "emb-underlay", cur, &labels, 150.0).and_then(|i| options.get(i)) {
            set(app, json!({"underlay": u.id()}));
        }
    });
}

fn design(app: &mut VectorcraftApp, ui: &mut Ui, d: &Value) {
    widgets::section_header(ui, tl!("Design"));
    let n = |k: &str| d[k].as_u64().unwrap_or(0);
    if n("objects") == 0 {
        widgets::dim_label(ui, tl!("Nothing is stitched yet."));
    } else {
        let f = |k: &str| d[k].as_f64().unwrap_or(0.0);
        widgets::label_row(ui, tl!("Stitches:"), LABEL_W, |ui| {
            ui.label(n("stitches").to_string());
        });
        widgets::label_row(ui, tl!("Size:"), LABEL_W, |ui| {
            ui.label(format!("{:.1} × {:.1} mm", f("widthMm"), f("heightMm")));
        });
        widgets::label_row(ui, tl!("Trims / Jumps:"), LABEL_W, |ui| {
            ui.label(format!("{} / {}", n("trims"), n("jumps")));
        });
        widgets::label_row(ui, tl!("Threads:"), LABEL_W, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (i, t) in d["threads"].as_array().into_iter().flatten().enumerate() {
                    if let Some(c) = t.as_str().and_then(rgb) {
                        chip(ui, c, 14.0).on_hover_text(format!("{} {}", i + 1, t.as_str().unwrap_or("")));
                    }
                }
            });
        });
        for w in d["warnings"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            widgets::dim_label(ui, w);
        }
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let preview = app.ui.view.stitch_preview;
        if widgets::check(ui, tl!("Stitch Preview"), preview, true) {
            app.run("view.stitchPreview", json!({})).ok();
        }
        ui.add_space(8.0);
        let r = ui.add_enabled_ui(n("objects") > 0, |ui| widgets::flat_button(ui, tl!("Export DST…"), 96.0)).inner;
        if r.clicked() {
            app.run("file.exportAs", json!({"format": "dst"})).ok();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_threads_parse() {
        assert_eq!(rgb("#0a0B0c"), Some([10, 11, 12]));
        assert_eq!(rgb("0a0b0c"), Some([10, 11, 12]));
        assert_eq!(rgb("#0a0b"), None);
        assert_eq!(rgb("#zz0000"), None);
    }
}
