//! Embroidery panel (Window › Embroidery). A header above the tabs holds Stitch Preview, the design
//! in one line, Export in the machine's format (DST, PES, JEF, EXP, VP3, U01) and a badge for the
//! checker's findings. The Stitch tab holds the selected objects' stitch, chosen from a grid of
//! stitched pictures, their thread and trims, and the stitch's own settings (fill patterns and
//! border patterns picked from pictures too); the Setup tab, what the design is sewn on and with and
//! its sewing settings; Check & Export, its numbers, threads and files and the checker's findings;
//! Tools, the digitizing and lettering tools. The sewing order has a panel of its own (`sew_order`). Reads
//! `embroidery.info`, acts through `embroidery.set` and `embroidery.clear`.

use egui::{Color32, Rect, Sense, Stroke, Ui, vec2};
use serde_json::{Value, json};
use vectorcraft_doc::{EmbroiderySetup, NodeId, RunStyle, Split, SplitPattern, StitchKind, StitchSettings, Underlay};

use super::{selection_len, stitch_art};
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

pub const ID: &str = "embroidery";
/// The width the panel is laid out to fit. Docked in the default column its content gets 290
/// points (the column's 310 less the body's margins); the rest is room for a scroll bar.
const PANEL_W: f32 = 270.0;
const LABEL_W: f32 = 114.0;
const FIELD_W: f32 = 76.0;
/// A dropdown, text field or picker that fills its row beside the label.
const WIDE_W: f32 = PANEL_W - LABEL_W - 8.0;

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
    let id = egui::Id::unique("embroidery-info");
    if let Some((k, v)) = ctx.data(|d| d.get_temp::<(Key, Value)>(id))
        && k == key
    {
        return v;
    }
    let mut v = app.session.execute("embroidery.info", &json!({})).unwrap_or(Value::Null);
    if let (Some(o), Ok(check)) = (v.as_object_mut(), app.session.execute("embroidery.check", &json!({}))) {
        o.insert("check".into(), check);
    }
    if let (Some(o), Ok(list)) = (v["selection"].as_object_mut(), app.session.execute("embroidery.borderPatterns", &json!({}))) {
        o.insert("borderPatterns".into(), list["patterns"].clone());
    }
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

/// What `add` draws in the column beside the labels, wrapping onto more lines there when one
/// isn't enough: a row of buttons or a long note is never cut off at the panel's edge.
fn indented(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.add_space(LABEL_W + 8.0);
        let width = ui.available_width();
        // A row's height from the start: with none, the row centres an empty block and what is
        // drawn hangs half a row low.
        ui.allocate_ui_with_layout(vec2(width, 24.0), egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true), add);
    });
}

/// A dropdown of names filling its row beside the label. A name too long for the row is cut
/// short with an ellipsis: the dropdown never widens past the panel's edge.
fn names_dropdown(ui: &mut Ui, id: impl std::hash::Hash, current: &str, options: &[&str]) -> Option<usize> {
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
    widgets::dropdown_names(ui, id, current, options, WIDE_W)
}

/// The panel's tabs, by task: the selection's stitches, what the design is sewn on and with, what
/// it came to (its numbers, threads and problems, and the files made from it), and the tools.
const TABS: [&str; 4] = ["Stitch", "Setup", "Check & Export", "Tools"];
const SETUP_TAB: usize = 1;
const CHECK_TAB: usize = 2;
const TOOLS_TAB: usize = 3;

/// Which tab is open (kept between sessions). Named afresh with the task tabs: the index kept for
/// the old Stitch, Design, Tools tabs means something else now.
fn tab_id() -> egui::Id {
    egui::Id::unique("embroidery-panel-task-tab")
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let info = info(app, ui.ctx());
    // One width for every tab, so the panel doesn't jump as they change.
    ui.set_min_width(PANEL_W);
    header(app, ui, &info);
    let mut tab = ui.ctx().data_mut(|d| d.get_persisted::<usize>(tab_id())).unwrap_or(0).min(TABS.len() - 1);
    let names = TABS.map(|t| tl!(t));
    if let Some(i) = widgets::tab_bar(ui, &names, tab) {
        tab = i;
        ui.ctx().data_mut(|d| d.insert_persisted(tab_id(), i));
    }
    ui.add_space(4.0);
    match tab {
        SETUP_TAB => setup_tab(app, ui, &info["setup"]),
        CHECK_TAB => {
            check_tab(app, ui, &info["design"]);
            widgets::divider(ui);
            checks(app, ui, &info["check"]);
        }
        TOOLS_TAB => tools(app, ui, &info["selection"]),
        _ => stitch_tab(app, ui, &info),
    }
}

/// Open tab `tab` of the panel (`CHECK_TAB`, …).
fn open_tab(ui: &Ui, tab: usize) {
    ui.ctx().data_mut(|d| d.insert_persisted(tab_id(), tab));
}

/// The Stitch tab: the selected objects' stitch, thread, trims and settings.
fn stitch_tab(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    let sel = &info["selection"];
    let stitched = sel["stitched"].as_u64().unwrap_or(0);
    let unstitched = sel["unstitched"].as_u64().unwrap_or(0);
    let any = stitched + unstitched > 0;
    let kind = sel["type"].as_str().and_then(StitchKind::parse).filter(|_| unstitched == 0);

    setup_prompt(app, ui, info);
    widgets::section_header(ui, tl!("Stitch"));
    if !any {
        widgets::dim_label(
            ui,
            if selection_len(app) == 0 { tl!("Select paths to stitch.") } else { tl!("Only paths, compound paths and type can be stitched.") },
        );
    } else {
        stitch_picker(app, ui, kind);
        match kind {
            Some(k) => {
                widgets::dim_label(ui, tl!(kind_blurb(k)));
            }
            None if sel["type"] == json!("unknown") => {
                widgets::dim_label(
                    ui,
                    tl!("These stitch settings are from a newer VectorCraft: kept as they are, but not sewn. Choose a stitch to replace them."),
                );
            }
            None if stitched == 0 => {
                widgets::dim_label(ui, tl!("Choose a stitch to sew the selection with."));
            }
            None if unstitched > 0 => {
                widgets::dim_label(ui, tl!("Some selected objects aren't stitched: choose a stitch for all of them."));
            }
            None => {
                widgets::dim_label(ui, tl!("The selected objects use different stitches."));
            }
        }
        ui.horizontal(|ui| {
            styles_menu(app, ui, kind.is_some() && stitched > 0);
            if stitched > 0
                && widgets::flat_button(ui, tl!("Remove Stitches"), 110.0).on_hover_text(tl!("The selection is no longer stitched")).clicked()
            {
                app.run("embroidery.clear", json!({})).ok();
            }
        });
    }
    if stitched > 0 && unstitched == 0 {
        widgets::divider(ui);
        widgets::section_header(ui, tl!("Thread"));
        thread_row(app, ui, sel);
        trim_row(app, ui, sel);
        stop_row(app, ui, sel);
        if selects_type(app) {
            font_row(app, ui, sel);
        }
        if let Some(k) = kind {
            widgets::divider(ui);
            parameters(app, ui, k, sel);
        }
    }
}

/// Once something is stitched while the document has no fabric or no hoop: a short prompt with
/// both menus, since the fabric fills in density, compensation and underlay for everything left
/// unset and the hoop is what the design is checked against. Gone once both are chosen, or for good
/// when dismissed.
fn setup_prompt(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    if app.ui.embroidery_setup_prompt_off || info["design"]["objects"].as_u64().unwrap_or(0) == 0 {
        return;
    }
    let Some((fabric, hoop)) = app.session.active().map(|d| (d.doc.embroidery.fabric.is_some(), d.doc.embroidery.hoop.is_some())) else {
        return;
    };
    if fabric && hoop {
        return;
    }
    // No frame: the rows keep the width they are laid out to everywhere else.
    widgets::section_header(ui, tl!("Fabric and Hoop"));
    widgets::dim_label(
        ui,
        tl!("Choose the fabric and the hoop: the fabric sets the density, compensation and underlay of every stitch that doesn't set its own."),
    );
    if !fabric {
        fabric_row(app, ui);
    }
    if !hoop {
        hoop_row(app, ui);
    }
    indented(ui, |ui| {
        if widgets::flat_button(ui, tl!("Dismiss"), 70.0).on_hover_text(tl!("Don't show this again (Setup has both menus)")).clicked() {
            app.ui.embroidery_setup_prompt_off = true;
        }
    });
    widgets::divider(ui);
}

/// Above the tabs, whichever is open: Stitch Preview and Realistic, the design in one line, and
/// Export with a badge for what the checker found (it opens the checker's list).
fn header(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    preview_toggles(app, ui);
    let d = &info["design"];
    let stitched = d["objects"].as_u64().unwrap_or(0) > 0;
    let t = Tokens::get(ui.ctx());
    if stitched {
        ui.add(egui::Label::new(egui::RichText::new(summary(d)).size(11.5).color(t.text_dim)).truncate())
            .on_hover_text(tl!("The stitched design: its stitches, size, threads and sewing time"));
    }
    ui.horizontal_wrapped(|ui| {
        let label = export_label(app);
        if ui.add_enabled_ui(stitched, |ui| widgets::flat_button(ui, &label, 96.0)).inner.clicked() {
            let format = machine_format(app);
            app.run("file.exportAs", json!({ "format": format })).ok();
        }
        let findings = info["check"]["findings"].as_array().map_or(0, Vec::len);
        if findings > 0 {
            let text = crate::i18n::tn(u64::try_from(findings).unwrap_or(u64::MAX), "{n} problem", "{n} problems");
            let badge = egui::Button::new(egui::RichText::new(format!("⚠ {text}")).color(t.warning)).frame(false);
            if ui.add(badge).on_hover_text(tl!("What the design checker found: open its list")).clicked() {
                open_tab(ui, CHECK_TAB);
            }
        }
    });
    ui.add_space(2.0);
}

/// "1062 stitches · 38.8 × 24.7 mm · 2 threads · 1.4 min": the design's numbers in one line.
fn summary(d: &Value) -> String {
    let f = |k: &str| d[k].as_f64().unwrap_or(0.0);
    let threads = u64::try_from(d["threads"].as_array().map_or(0, Vec::len)).unwrap_or(u64::MAX);
    let minutes = crate::i18n::fmt(tl!("{m} min"), &[("m", &f("minutes").to_string())]);
    format!(
        "{} · {:.1} × {:.1} mm · {} · {minutes}",
        crate::i18n::tn(d["stitches"].as_u64().unwrap_or(0), "{n} stitch", "{n} stitches"),
        f("widthMm"),
        f("heightMm"),
        crate::i18n::tn(threads, "{n} thread", "{n} threads"),
    )
}

/// Stitch Preview and Realistic, in the header: what a stitch looks like is seen on the canvas
/// only with the preview on, so its switch is always in reach.
fn preview_toggles(app: &mut VectorcraftApp, ui: &mut Ui) {
    ui.horizontal_wrapped(|ui| {
        let preview = app.ui.view.stitch_preview;
        if widgets::check(ui, tl!("Stitch Preview"), preview, true) {
            app.run("view.stitchPreview", json!({})).ok();
        }
        let realistic = preview && app.ui.view.realistic_stitches;
        if widgets::check(ui, tl!("Realistic"), realistic, true) {
            // Off while the preview shows turns only the realism off; on shows the preview too.
            if realistic || !app.ui.view.realistic_stitches {
                app.run("view.realisticStitches", json!({})).ok();
            } else {
                app.run("view.stitchPreview", json!({})).ok();
            }
        }
    });
}

/// The Tools tab: one collapsible section per tool, closed until opened (each remembers whether it
/// was left open), so the tab is a short list to choose from rather than every form at once.
fn tools(app: &mut VectorcraftApp, ui: &mut Ui, sel: &Value) {
    tool_section(ui, "applique", tl!("Appliqué"), |ui| {
        widgets::dim_label(ui, tl!("Make each selected closed shape an appliqué: placement line, stop, tack-down, stop, cover"));
        ui.horizontal(|ui| {
            if widgets::flat_button(ui, tl!("Appliqué"), 90.0).clicked()
                && let Err(e) = app.run("embroidery.applique", json!({}))
            {
                app.status(e);
            }
            if widgets::flat_button(ui, tl!("Cut File…"), 80.0)
                .on_hover_text(tl!("The appliqué pieces' outlines packed onto cutter mats, in a new document to export as SVG or DXF"))
                .clicked()
                && let Err(e) = app.run("embroidery.cutFile", json!({}))
            {
                app.status(e);
            }
        });
    });
    // A border pattern or a motif from an opened design's stitches (manual stitches).
    if sel["type"] == json!(StitchKind::Manual.id()) {
        tool_section(ui, "extract", tl!("From Stitches"), |ui| extract_row(app, ui));
    }
    tool_section(ui, "digitize", tl!("Auto-Digitize"), |ui| digitize_row(app, ui));
    tool_section(ui, "lettering", tl!("Lettering"), |ui| lettering(app, ui));
    tool_section(ui, "fonts", tl!("Embroidery Fonts"), |ui| {
        ui.horizontal_wrapped(|ui| {
            if widgets::flat_button(ui, tl!("Load Embroidery Font…"), 150.0).clicked() {
                load_font(app);
            }
            if widgets::flat_button(ui, tl!("Import Ink/Stitch Font…"), 160.0).clicked() {
                crate::picks::button(app, |app| {
                    let Some(dir) = crate::picks::folder(app) else { return };
                    match app.run("embroidery.importInkstitchFont", json!({ "path": dir })) {
                        Ok(r) => app.status(format!("Loaded embroidery font {}", r["name"].as_str().unwrap_or_default())),
                        Err(e) => app.status(e),
                    }
                });
            }
            add_font_folder_button(app, ui);
        });
        // The Font Editor is a different mode of work: a panel of its own.
        if widgets::flat_button(ui, tl!("Font Editor…"), 104.0)
            .on_hover_text(tl!("Make an embroidery font, or edit the one in this font document"))
            .clicked()
        {
            app.run("window.panel", json!({"panel": FONT_EDITOR_ID})).ok();
        }
    });
    if let Some(dir) = app.session.embroidery_libraries.dir().map(str::to_string) {
        tool_section(ui, "library", tl!("Library"), |ui| {
            if widgets::flat_button(ui, tl!("Show in Folder"), 104.0)
                .on_hover_text(tl!("Your embroidery library: border patterns, fabrics, hoops and thread charts for any design"))
                .clicked()
            {
                show_library_folder(app, &dir);
            }
        });
    }
}

/// A Tools section: its header toggles it, and it stays as it was left (closed at first).
fn tool_section(ui: &mut Ui, key: &str, label: &str, add: impl FnOnce(&mut Ui)) {
    let id = egui::Id::unique(("embroidery-tool-open", key.to_string()));
    let open = ui.ctx().data_mut(|d| d.get_persisted::<bool>(id)).unwrap_or(false);
    if widgets::section_toggle(ui, label, open) {
        ui.ctx().data_mut(|d| d.insert_persisted(id, !open));
    }
    if open {
        ui.indent(id, add);
        ui.add_space(4.0);
    }
}

/// The Embroidery Font Editor panel (Window › Embroidery Font Editor, or Font Editor… in the
/// Embroidery panel's Tools): a new font, or the open font document's details, metrics, kerning,
/// sample line and saving.
pub const FONT_EDITOR_ID: &str = "embroideryFontEditor";

pub fn show_font_editor(app: &mut VectorcraftApp, ui: &mut Ui) {
    ui.set_min_width(PANEL_W);
    font_editor(app, ui);
}

/// Monogram layouts: `embroidery.monogram`'s ids and their names.
fn monogram_layouts() -> [(&'static str, &'static str); 6] {
    [
        ("classic", tl!("Classic")),
        ("three", tl!("Three Equal")),
        ("two", tl!("Two Letters")),
        ("stacked", tl!("Stacked")),
        ("circle", tl!("Circle")),
        ("diamond", tl!("Diamond")),
    ]
}

/// The selected object when it is one of `embroidery.monogram`'s groups.
fn selected_monogram(app: &VectorcraftApp) -> Option<NodeId> {
    let d = app.session.active()?;
    let [id] = d.selection.objects.as_slice() else { return None };
    vectorcraft_engine::cmd::embroidery_text::is_monogram(d.doc.node(*id)?).then_some(*id)
}

/// The selected point type's bounds and baseline (`None` unless one point type is selected).
fn selected_point_type(app: &VectorcraftApp) -> Option<(vectorcraft_geom::Rect, f64)> {
    let d = app.session.active()?;
    let [id] = d.selection.objects.as_slice() else { return None };
    let n = d.doc.node(*id)?;
    let vectorcraft_doc::NodeKind::Text(t) = &n.kind else { return None };
    (t.kind == vectorcraft_doc::TextKind::Point).then_some(())?;
    Some((n.geometric_bounds()?, t.xf.translation().y))
}

/// Lettering: the selected point type put on an arc, and monograms.
/// Auto-Digitize: the selected logo (an image, a traced image or a group of filled shapes) made
/// into stitch objects in one step.
fn digitize_row(app: &mut VectorcraftApp, ui: &mut Ui) {
    widgets::dim_label(ui, tl!("Select a logo (an image or a group of flat-colour shapes): it is traced and each area stitched in its colour."));
    let row = ui.horizontal(|ui| {
        let tool = widgets::flat_button(ui, tl!("Stitch Fill Tool"), 120.0)
            .on_hover_text(tl!(
                "Click inside any closed area to stitch it in its colour; Alt-click to outline it (Alt-Shift-click: a satin border); Shift-click to sew a thin line along its centre"
            ))
            .clicked();
        (widgets::flat_button(ui, tl!("Auto-Digitize"), 120.0).clicked(), tool)
    });
    let (digitize, tool) = row.inner;
    let mut centre = kept(ui, "digitize-lines", "auto") == "centreline";
    ui.checkbox(&mut centre, tl!("Thin Lines as Centre Lines"))
        .on_hover_text(tl!("Sew parts narrower than 2 mm, such as traced lines and handwriting, as one line along their centre"));
    keep(ui, "digitize-lines", if centre { "centreline" } else { "auto" }.into());
    if tool && let Err(e) = app.run("tool.select", json!({"tool": "stitchFill"})) {
        app.status(e);
    }
    if digitize {
        match app.run("embroidery.digitize", json!({"lines": if centre { "centreline" } else { "auto" }})) {
            Ok(r) => app.status(format!(
                "Digitized {} areas in {} threads ({} merged): {} warnings",
                r["objects"],
                r["threads"].as_array().map_or(0, Vec::len),
                r["merged"],
                r["findings"]["warning"]
            )),
            Err(e) => app.status(e),
        }
    }
}

fn lettering(app: &mut VectorcraftApp, ui: &mut Ui) {
    // Type on an arc through where the selected type is.
    let (mut radius, mut bottom) = (kept(ui, "arc-radius", "40"), kept(ui, "arc-side", "top") == "bottom");
    ui.horizontal(|ui| {
        dim_label_w(ui, tl!("Radius:"));
        if let Some(v) = widgets::text_field(ui, "emb-arc-radius", Some(&radius), 50.0, 1) {
            radius = v;
        }
        ui.label("mm");
        ui.checkbox(&mut bottom, tl!("Bottom"));
    });
    keep(ui, "arc-radius", radius.clone());
    keep(ui, "arc-side", if bottom { "bottom" } else { "top" }.into());
    let point = selected_point_type(app);
    let r_mm = radius.trim().parse::<f64>().ok().filter(|r| r.is_finite() && *r > 0.0);
    let can = point.is_some() && r_mm.is_some();
    if ui
        .add_enabled(can, egui::Button::new(tl!("Type on an Arc")))
        .on_hover_text(tl!(
            "Put the selected point type on a circle arc through where it is: over the top, or under the bottom reading left to right"
        ))
        .clicked()
        && let (Some((b, baseline)), Some(r)) = (point, r_mm)
    {
        let r = r / vectorcraft_embroidery::PT_MM;
        // The arc's top passes along the type's baseline; its bottom along the type's top.
        let cy = if bottom { b.y0 - r } else { baseline + r };
        let p = json!({"cx": b.center().x, "cy": cy, "radius": r, "side": if bottom { "bottom" } else { "top" }});
        if let Err(e) = app.run("embroidery.textArc", p) {
            app.status(e);
        }
    }
    ui.add_space(4.0);
    // A monogram, or the selected one laid out again: selecting one shows its recipe.
    let editing = selected_monogram(app);
    if let Some(id) = editing
        && kept(ui, "mono-for", "") != id.0.to_string()
        && let Some(n) = app.session.active().and_then(|d| d.doc.node(id))
    {
        use vectorcraft_engine::cmd::embroidery_text::recipe;
        for (key, field) in [("monogram-letters", "mono-letters"), ("monogram-layout", "mono-layout"), ("monogram-frame", "mono-frame")] {
            if let Some(v) = recipe(n, key) {
                keep(ui, field, v);
            }
        }
        keep(ui, "mono-for", id.0.to_string());
    }
    let (mut letters, mut layout, mut frame) = (kept(ui, "mono-letters", "ABC"), kept(ui, "mono-layout", "classic"), kept(ui, "mono-frame", "none"));
    widgets::label_row(ui, tl!("Letters:"), LABEL_W, |ui| {
        if let Some(v) = widgets::text_field(ui, "emb-mono-letters", Some(&letters), 60.0, 1) {
            letters = v;
        }
    });
    widgets::label_row(ui, tl!("Layout:"), LABEL_W, |ui| {
        let layouts = monogram_layouts();
        let current = layouts.iter().find(|(id, _)| *id == layout).map_or("", |(_, n)| n);
        if let Some(id) = widgets::combo(ui, "emb-mono-layout", current, WIDE_W, |ui| {
            let mut chosen = None;
            for (id, name) in layouts {
                if widgets::menu_item_name(ui, name, true, layout == id) {
                    chosen = Some(id.to_string());
                }
            }
            chosen
        }) {
            layout = id;
        }
    });
    widgets::label_row(ui, tl!("Frame:"), LABEL_W, |ui| {
        let name = |id: &str| match id {
            "none" => tl!("No Frame").to_string(),
            "satin" => tl!("Satin Ring").to_string(),
            id => vectorcraft_embroidery::borders::builtin(id).map_or_else(|| id.to_string(), |b| b.name),
        };
        if let Some(id) = widgets::combo(ui, "emb-mono-frame", &name(&frame), WIDE_W, |ui| {
            let mut chosen = None;
            for id in ["none", "satin"].into_iter().chain(vectorcraft_embroidery::borders::BUILTIN) {
                if widgets::menu_item_name(ui, &name(id), true, frame == id) {
                    chosen = Some(id.to_string());
                }
            }
            chosen
        }) {
            frame = id;
        }
    });
    let label = if editing.is_some() { tl!("Update Monogram") } else { tl!("Monogram") };
    if widgets::flat_button(ui, label, 120.0)
        .on_hover_text(tl!("Letters as live type placed by the layout, each still editable, with an optional frame"))
        .clicked()
    {
        let mut p = json!({"letters": letters, "layout": layout, "frame": frame});
        if let Some(id) = editing {
            p["id"] = json!(id.0);
        }
        if let Err(e) = app.run("embroidery.monogram", p) {
            app.status(e);
        }
    }
    keep(ui, "mono-letters", letters);
    keep(ui, "mono-layout", layout);
    keep(ui, "mono-frame", frame);
}

/// A dim label at the panel's label width.
fn dim_label_w(ui: &mut Ui, text: &str) {
    ui.add_sized(vec2(LABEL_W, 18.0), egui::Label::new(egui::RichText::new(text).color(Tokens::get(ui.ctx()).text_dim)));
}

/// A text the panel keeps between frames (what is being typed for a command not run yet).
fn kept(ui: &Ui, key: &str, default: &str) -> String {
    ui.ctx().data_mut(|d| d.get_temp::<String>(egui::Id::unique(("emb-font-editor", key)))).unwrap_or_else(|| default.to_string())
}

fn keep(ui: &Ui, key: &str, value: String) {
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique(("emb-font-editor", key)), value));
}

/// The Font Editor: a new font or one to edit; in a font document, its details, metrics, kerning,
/// sample line and saving.
fn font_editor(app: &mut VectorcraftApp, ui: &mut Ui) {
    let editing = app.session.active().and_then(|d| d.doc.embroidery.font_editor.as_deref().cloned());
    let Some(font) = editing else {
        widgets::dim_label(ui, tl!("Make an embroidery font: one artboard per letter, digitized with the ordinary stitches."));
        let (mut name, mut chars, mut from) = (kept(ui, "name", "Untitled Font"), kept(ui, "chars", "A-Za-z0-9 .,!?'&-"), kept(ui, "from", ""));
        widgets::label_row(ui, tl!("Name:"), LABEL_W, |ui| {
            if let Some(v) = widgets::text_field(ui, "emb-font-new-name", Some(&name), WIDE_W, 1) {
                name = v;
            }
        });
        widgets::label_row(ui, tl!("Characters:"), LABEL_W, |ui| {
            if let Some(v) = widgets::text_field(ui, "emb-font-new-chars", Some(&chars), WIDE_W, 1) {
                chars = v;
            }
        });
        ui.scope(|ui| {
            widgets::label_row(ui, tl!("Trace:"), LABEL_W, |ui| {
                // An installed font, picked from the font menu (typing a family's name exactly is
                // too easy to get wrong).
                let current = if from.is_empty() { tl!("None") } else { from.as_str() };
                let pick = crate::font_menu::font_menu(ui, "emb-font-new-from", current, WIDE_W - 28.0, None, crate::font_menu::MenuLook::of(app));
                if let Some((family, _)) = crate::font_menu::picked(app, pick) {
                    from = family;
                }
                if !from.is_empty() && widgets::flat_button(ui, "×", 22.0).on_hover_text(tl!("None")).clicked() {
                    from.clear();
                }
            })
        })
        .response
        .on_hover_text(tl!("An installed font whose letters go on a locked template layer to trace over (left empty: none)"));
        keep(ui, "name", name.clone());
        keep(ui, "chars", chars.clone());
        keep(ui, "from", from.clone());
        ui.horizontal_wrapped(|ui| {
            if widgets::flat_button(ui, tl!("New Embroidery Font"), 150.0).clicked() {
                let mut p = json!({ "name": name, "chars": chars });
                if !from.trim().is_empty() {
                    p["from"] = json!(from.trim());
                }
                if let Err(e) = app.run("embroidery.font.new", p) {
                    app.status(e);
                }
            }
            if !from.trim().is_empty()
                && widgets::flat_button(ui, tl!("Trace Font"), 80.0)
                    .on_hover_text(tl!("Digitize the Trace font's letters automatically, each pen stroke a satin column (only for fonts whose licence allows derivatives)"))
                    .clicked()
            {
                // Traced a few letters a frame, with a progress window (see `font_trace`).
                crate::font_trace::start(app, json!({ "from": from.trim(), "chars": chars }));
            }
            if widgets::flat_button(ui, tl!("Edit Font…"), 90.0).clicked() {
                crate::picks::button(app, |app| {
                    let ext: &'static [&'static str] = &[vectorcraft_embroidery::fonts::FILE_EXT];
                    let pick = crate::FilePick { filters: vec![crate::FilePick::filter("Embroidery fonts", ext)], ..Default::default() };
                    let Some(path) = crate::picks::open(app, &pick) else { return };
                    if let Err(e) = app.run("embroidery.font.open", json!({ "path": path })) {
                        app.status(e);
                    }
                });
            }
        });
        return;
    };
    // The font's details.
    for (label, key, value) in [
        (tl!("Name:"), "name", Some(font.name.clone())),
        (tl!("Author:"), "author", font.author.clone()),
        (tl!("License:"), "license", font.license.clone()),
    ] {
        widgets::label_row(ui, label, LABEL_W, |ui| {
            if let Some(v) = widgets::text_field(ui, ("emb-font", key), Some(value.as_deref().unwrap_or("")), WIDE_W, 1)
                && let Err(e) = app.run("embroidery.font.setup", json!({ key: v }))
            {
                app.status(e);
            }
        });
    }
    // Metrics, font units.
    for (label, key, value) in [
        (tl!("Ascender:"), "ascender", font.ascender),
        (tl!("Cap Height:"), "capHeight", font.cap_height),
        (tl!("x Height:"), "xHeight", font.x_height),
        (tl!("Descender:"), "descender", font.descender),
    ] {
        widgets::label_row(ui, label, LABEL_W, |ui| {
            if let Some(v) = widgets::mixed_field(ui, ("emb-font", key), Some(value), "", 0, FIELD_W)
                && let Err(e) = app.run("embroidery.font.setup", json!({ key: v }))
            {
                app.status(e);
            }
        });
    }
    // Kerning pairs, and one to add.
    widgets::label_row(ui, tl!("Kerning:"), LABEL_W, |ui| {
        ui.vertical(|ui| {
            for k in &font.kerning {
                ui.horizontal(|ui| {
                    ui.label(format!("{} {}  {}", k.left, k.right, k.value));
                    if widgets::flat_button(ui, "×", 22.0).clicked() {
                        app.run("embroidery.font.kern", json!({"left": k.left, "right": k.right, "value": null})).ok();
                    }
                });
            }
            let (mut pair, mut value) = (kept(ui, "pair", ""), kept(ui, "kern", "-50"));
            ui.horizontal(|ui| {
                if let Some(v) = widgets::text_field(ui, "emb-font-kern-pair", Some(&pair), 40.0, 1) {
                    pair = v;
                }
                if let Some(v) = widgets::text_field(ui, "emb-font-kern-value", Some(&value), 50.0, 1) {
                    value = v;
                }
                let mut chars = pair.chars();
                if let (Some(a), Some(b), None) = (chars.next(), chars.next(), chars.next())
                    && let Ok(v) = value.trim().parse::<f64>()
                    && widgets::flat_button(ui, tl!("Add"), 46.0).clicked()
                    && let Err(e) = app.run("embroidery.font.kern", json!({"left": a.to_string(), "right": b.to_string(), "value": v}))
                {
                    app.status(e);
                }
            });
            keep(ui, "pair", pair);
            keep(ui, "kern", value);
            widgets::dim_label(ui, tl!("Two letters and font units: negative sets them closer"));
        });
    });
    // The sample line.
    let mut sample = kept(ui, "sample", "Sample");
    ui.horizontal(|ui| {
        if let Some(v) = widgets::text_field(ui, "emb-font-sample", Some(&sample), 150.0, 1) {
            sample = v;
        }
        if widgets::flat_button(ui, tl!("Update Sample"), 110.0)
            .on_hover_text(tl!("A line of type below the letters, sewn from the font as it is now"))
            .clicked()
            && let Err(e) = app.run("embroidery.font.sample", json!({ "text": sample }))
        {
            app.status(e);
        }
    });
    keep(ui, "sample", sample);
    ui.horizontal(|ui| {
        if widgets::flat_button(ui, tl!("Save Font…"), 100.0).clicked() {
            let file = format!("{}.{}", font.name, vectorcraft_embroidery::fonts::FILE_EXT);
            crate::picks::button(app, move |app| {
                if let Err(e) = crate::io::save_command_output_named(app, "embroidery.font.save", &file, json!({})) {
                    app.status(e);
                }
            });
        }
        if app.session.embroidery_libraries.dir().is_some() && widgets::flat_button(ui, tl!("Add to Library"), 104.0).clicked() {
            match app.run("embroidery.font.save", json!({"user": true})) {
                Ok(_) => app.status(format!("Added {} to the embroidery library", font.name)),
                Err(e) => app.status(e),
            }
        }
    });
}

/// Load a `.vcfont` embroidery font the user picks into the document.
fn load_font(app: &mut VectorcraftApp) {
    if let Some(f) = app.services.open_async.as_mut() {
        f();
        return;
    }
    crate::picks::button(app, |app| {
        let ext: &'static [&'static str] = &[vectorcraft_embroidery::fonts::FILE_EXT];
        let pick = crate::FilePick { filters: vec![crate::FilePick::filter("Embroidery fonts", ext)], ..Default::default() };
        let Some(path) = crate::picks::open(app, &pick) else { return };
        match app.run("embroidery.loadFont", json!({ "path": path })) {
            Ok(r) => app.status(format!("Loaded embroidery font {}", r["name"].as_str().unwrap_or_default())),
            Err(e) => app.status(e),
        }
    });
}

/// Add Font Folder…: a folder of embroidery fonts to list, read where they are.
fn add_font_folder_button(app: &mut VectorcraftApp, ui: &mut Ui) {
    if widgets::flat_button(ui, tl!("Add Font Folder…"), 120.0)
        .on_hover_text(tl!("A folder of embroidery fonts to list here, such as Ink/Stitch font folders you installed; they are read where they are"))
        .clicked()
    {
        let folders: Vec<String> = app.session.embroidery_libraries.font_folders().to_vec();
        crate::picks::button(app, move |app| {
            let Some(dir) = crate::picks::folder(app) else { return };
            let mut all = folders.clone();
            all.push(dir);
            match app.run("embroidery.fontFolders", json!({ "folders": all })) {
                Ok(r) => app.status(format!("{} embroidery fonts found", r["fonts"].as_array().map_or(0, Vec::len))),
                Err(e) => app.status(e),
            }
        });
    }
}

/// What each stitch type does, in a line (the picker's hint).
pub(crate) fn kind_blurb(k: StitchKind) -> &'static str {
    match k {
        StitchKind::Auto => "Picks a stitch for each shape (each letter) by how wide its strokes are.",
        StitchKind::Running => "Single stitches along the path: outlines and details.",
        StitchKind::Bean => "Each stitch sewn three times: a bolder line.",
        StitchKind::Satin => "A dense zigzag column along the path, as wide as the stroke.",
        StitchKind::Zigzag => "An open zigzag along the path: decorative lines, appliqué edges.",
        StitchKind::SatinShape => "Satin that follows a shape's strokes from edge to edge: lettering.",
        StitchKind::Fill => "Rows of stitches covering the shape, with a pattern.",
        StitchKind::PatternBorder => "A border pattern repeated along the path.",
        StitchKind::MotifFill => "A small stitched motif repeated across the shape, the fabric showing between.",
        StitchKind::Manual => "The needle goes down at every anchor, as drawn.",
    }
}

/// The stitch types as a grid of stitched pictures; the selection's is outlined.
fn stitch_picker(app: &mut VectorcraftApp, ui: &mut Ui, current: Option<StitchKind>) {
    let cols = 3;
    let gap = 4.0;
    let tile_w = ((ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32).floor().max(60.0);
    let size = vec2(tile_w, 58.0);
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = vec2(gap, gap);
        for row in StitchKind::ALL.chunks(cols) {
            ui.horizontal(|ui| {
                for k in row {
                    let thumb = stitch_art::kind(ui.ctx(), *k);
                    let r = stitch_art::tile(ui, k.id(), size, &thumb, tl!(k.label()), current == Some(*k), false).on_hover_text(tl!(kind_blurb(*k)));
                    if r.clicked() && current != Some(*k) {
                        set(app, json!({"type": k.id()}));
                    }
                }
            });
        }
    });
}

/// A picker button showing `thumb` and `label`, opening a grid of `entries` (label, picture) on
/// click; the chosen entry's index. `dots` draws needle points (fill patterns).
fn picture_picker(
    ui: &mut Ui,
    id: &str,
    label: &str,
    thumb: Option<&stitch_art::Thumb>,
    entries: &[(String, stitch_art::Thumb)],
    selected: Option<usize>,
    dots: bool,
) -> Option<usize> {
    let t = Tokens::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(vec2(WIDE_W, 36.0), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    ui.painter().rect_filled(rect, 3.0, t.input);
    ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    let pic = Rect::from_min_size(rect.min, vec2(56.0, rect.height()));
    if let Some(th) = thumb {
        stitch_art::draw(ui.painter(), pic, th, t.text, dots.then_some(t.text_dim));
    }
    // The name, clipped short of the arrow.
    let name = Rect::from_min_max(pic.right_top(), egui::pos2(rect.right() - 18.0, rect.bottom()));
    ui.painter_at(name).text(name.left_center() + vec2(6.0, 0.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.0), t.text);
    let arrow = Rect::from_center_size(rect.right_center() - vec2(10.0, 0.0), vec2(8.0, 5.0));
    ui.painter().add(egui::Shape::convex_polygon(vec![arrow.left_top(), arrow.right_top(), arrow.center_bottom()], t.text_dim, Stroke::NONE));
    let cols = 3;
    let size = vec2(96.0, 72.0);
    egui::Popup::menu(&resp)
        .id(egui::Id::unique(("picture-picker", id)))
        .show(|ui| {
            ui.set_min_width(size.x * cols as f32 + 12.0);
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            let mut chosen = None;
            for (r, row) in entries.chunks(cols).enumerate() {
                ui.horizontal(|ui| {
                    for (c, (name, th)) in row.iter().enumerate() {
                        let i = r * cols + c;
                        if stitch_art::tile(ui, (id, i), size, th, name, selected == Some(i), dots).clicked() {
                            chosen = Some(i);
                        }
                    }
                });
            }
            chosen
        })
        .and_then(|r| r.inner)
}

/// Running › Style: single stitches and the hand-stitch styles, each as a picture.
fn run_style_picker(app: &mut VectorcraftApp, ui: &mut Ui, sel: &Value) {
    let ctx = ui.ctx().clone();
    let entries: Vec<(String, stitch_art::Thumb)> =
        RunStyle::ALL.iter().map(|s| (tl!(s.label()).to_string(), stitch_art::run_style(&ctx, *s))).collect();
    let selected = sel["style"].as_str().and_then(RunStyle::parse).and_then(|s| RunStyle::ALL.iter().position(|x| *x == s));
    ui.scope(|ui| {
        widgets::label_row(ui, tl!("Style:"), LABEL_W, |ui| {
            let (label, thumb) = selected.and_then(|i| entries.get(i)).map(|(l, th)| (l.clone(), Some(th))).unwrap_or_default();
            if let Some(s) = picture_picker(ui, "run-style", &label, thumb, &entries, selected, false).and_then(|i| RunStyle::ALL.get(i)) {
                set(app, json!({"style": s.id()}));
            }
        })
    })
    .response
    .on_hover_text(
        selected
            .and_then(|i| RunStyle::ALL.get(i))
            .map_or(tl!("How the line is sewn: single stitches or a hand-stitch look"), |s| tl!(run_style_blurb(*s))),
    );
}

/// What a running stitch style looks like, for its tooltip.
fn run_style_blurb(s: RunStyle) -> &'static str {
    match s {
        RunStyle::Single => "Single stitches along the path.",
        RunStyle::Chain => "Linked loops, like a hand chain stitch.",
        RunStyle::Stem => "Slanted stitches overlapping by half: a twisted cord.",
        RunStyle::Backstitch => "Each stitch forward, then half a stitch back: a heavier, smooth line.",
        RunStyle::Sashiko => "Bold dashes with gaps between them.",
    }
}

/// Fill › Pattern: brick, the tatami patterns and the carved motifs, each as a picture.
fn fill_pattern_picker(app: &mut VectorcraftApp, ui: &mut Ui, sel: &Value) {
    let rows: Vec<u8> = StitchSettings::STAGGER.collect();
    let ctx = ui.ctx().clone();
    let mut entries: Vec<(String, stitch_art::Thumb)> = rows.iter().map(|r| (pattern_label(*r), stitch_art::fill_pattern(&ctx, *r, None))).collect();
    entries.extend(vectorcraft_embroidery::motifs::MOTIFS.iter().map(|m| (motif_label(m).to_string(), stitch_art::fill_pattern(&ctx, 3, Some(m)))));
    let selected = match sel["motif"].as_str() {
        Some(m) => vectorcraft_embroidery::motifs::MOTIFS.iter().position(|x| *x == m).map(|i| i + rows.len()),
        None if sel["motif"].is_null() => sel["stagger"].as_u64().and_then(|s| u8::try_from(s).ok()).and_then(|s| rows.iter().position(|r| *r == s)),
        None => None,
    };
    ui.scope(|ui| {
        widgets::label_row(ui, tl!("Pattern:"), LABEL_W, |ui| {
            let (label, thumb) = selected.and_then(|i| entries.get(i)).map(|(l, th)| (l.clone(), Some(th))).unwrap_or_default();
            if let Some(i) = picture_picker(ui, "fill-pattern", &label, thumb, &entries, selected, true) {
                match rows.get(i) {
                    Some(n) => set(app, json!({"stagger": n, "motif": null})),
                    None => {
                        if let Some(m) = vectorcraft_embroidery::motifs::MOTIFS.get(i - rows.len()) {
                            set(app, json!({"motif": m}));
                        }
                    }
                }
            }
        })
    })
    .response
    .on_hover_text(tl!("How the needle holes line up from row to row, or a motif carved into the fill"));
}

/// Motif Fill › Motif: the stitched motifs (the built-in ones, then the document's), each as a
/// picture.
fn motif_fill_picker(app: &mut VectorcraftApp, ui: &mut Ui, sel: &Value) {
    use vectorcraft_doc::MotifKind;
    use vectorcraft_embroidery::motifs;
    let ctx = ui.ctx().clone();
    let mine: Vec<vectorcraft_doc::Motif> = app
        .session
        .active()
        .map(|d| d.doc.embroidery.motifs.iter().filter(|m| m.unknown.is_none() && m.kind == MotifKind::Stitched).cloned().collect())
        .unwrap_or_default();
    let all: Vec<vectorcraft_doc::Motif> = motifs::STITCHED.iter().filter_map(|id| motifs::builtin(id)).chain(mine).collect();
    let entries: Vec<(String, stitch_art::Thumb)> = all
        .iter()
        .map(|m| {
            let name = if motifs::STITCHED.contains(&m.id.as_str()) { motif_label(&m.id).to_string() } else { m.name.clone() };
            (name, stitch_art::motif(&ctx, m))
        })
        .collect();
    let current = sel["motif"].as_str().unwrap_or(motifs::DEFAULT_STITCHED).to_string();
    let selected = all.iter().position(|m| m.id == current);
    ui.scope(|ui| {
        widgets::label_row(ui, tl!("Motif:"), LABEL_W, |ui| {
            let (label, thumb) = selected.and_then(|i| entries.get(i)).map(|(l, th)| (l.clone(), Some(th))).unwrap_or_default();
            if let Some(m) = picture_picker(ui, "motif-fill", &label, thumb, &entries, selected, false).and_then(|i| all.get(i)) {
                set(app, json!({"motif": m.id}));
            }
        })
    })
    .response
    .on_hover_text(tl!("The motif sewn tile by tile across the shape"));
    // As for border patterns: load one from a file, save the chosen one, keep the document's own
    // in the library.
    let mine = selected.and_then(|i| all.get(i)).filter(|m| !motifs::STITCHED.contains(&m.id.as_str())).cloned();
    indented(ui, |ui| {
        if widgets::flat_button(ui, tl!("Load…"), 70.0).on_hover_text(tl!("Load a motif file (.vcmotif) into the document")).clicked() {
            load_motif(app);
        }
        if selected.is_some() && widgets::flat_button(ui, tl!("Save…"), 70.0).on_hover_text(tl!("Save this motif as a file (.vcmotif)")).clicked() {
            let id = current.clone();
            crate::picks::button(app, move |app| {
                let file = format!("{id}.{}", motifs::FILE_EXT);
                if let Err(e) = crate::io::save_command_output_named(app, "embroidery.saveMotif", &file, json!({ "motif": id })) {
                    app.status(e);
                }
            });
        }
        if let Some(m) = mine
            && add_button(app, ui, !in_library(&app.session.embroidery_libraries.motifs, &m))
        {
            add_to_library(app, "embroidery.saveMotif", json!({ "motif": m.id }), &format!("{}.{}", m.name, motifs::FILE_EXT), "motifs");
        }
    });
}

/// Load a motif file the user picks into the document (the web: the picked file arrives later,
/// through [`crate::io::open_bytes`]), and sew the selected motif fills with it.
fn load_motif(app: &mut VectorcraftApp) {
    if let Some(f) = app.services.open_async.as_mut() {
        f();
        return;
    }
    crate::picks::button(app, |app| {
        let ext: &'static [&'static str] = &[vectorcraft_embroidery::motifs::FILE_EXT];
        let pick = crate::FilePick { filters: vec![crate::FilePick::filter("Motifs", ext)], ..Default::default() };
        let Some(path) = crate::picks::open(app, &pick) else { return };
        load_motif_from(app, json!({ "path": path }));
    });
}

/// `embroidery.loadMotif` with `p`, then sew the selected motif fills with it when it is a
/// stitched motif (a carve motif is for fills' patterns).
pub(crate) fn load_motif_from(app: &mut VectorcraftApp, p: Value) {
    match app.run("embroidery.loadMotif", p) {
        Ok(r) => {
            let sel = app.session.execute("embroidery.info", &json!({})).unwrap_or(Value::Null);
            if sel["selection"]["type"] == json!(StitchKind::MotifFill.id()) && r["motifKind"] == json!("stitched") {
                set(app, json!({ "motif": r["id"] }));
            }
            app.status(format!("Loaded motif {}", r["name"].as_str().unwrap_or_default()));
        }
        Err(e) => app.status(e),
    }
}

/// The most findings the Check list shows.
const MAX_CHECK_ROWS: usize = 30;

/// The design checker's findings, errors first: click one to select its object. Stitch Preview
/// marks where each one is.
fn checks(app: &mut VectorcraftApp, ui: &mut Ui, check: &Value) {
    let t = crate::theme::Tokens::get(ui.ctx());
    widgets::section_header(ui, tl!("Check"));
    let findings: Vec<&Value> = check["findings"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    if findings.is_empty() {
        widgets::dim_label(ui, tl!("No problems found."));
        return;
    }
    let rules = check["rules"].as_array().cloned().unwrap_or_default();
    let help = |id: &str| rules.iter().find(|r| r["id"] == id).and_then(|r| r["help"].as_str()).map(|h| tl!(h)).unwrap_or_default();
    for f in findings.iter().take(MAX_CHECK_ROWS) {
        let color = match f["severity"].as_str() {
            Some("error") => t.error,
            Some("warning") => t.warning,
            _ => t.text_dim,
        };
        let message = f["message"].as_str().unwrap_or_default();
        let text = match f["objectName"].as_str() {
            Some(name) => format!("{name}: {message}"),
            None => message.to_string(),
        };
        ui.horizontal(|ui| {
            let (dot, _) = ui.allocate_exact_size(vec2(10.0, 14.0), egui::Sense::hover());
            ui.painter().circle_filled(dot.center(), 4.0, color);
            let row = ui.add(egui::Label::new(text).wrap().sense(egui::Sense::click())).on_hover_text(help(f["rule"].as_str().unwrap_or_default()));
            if row.clicked()
                && let Some(id) = f["object"].as_u64()
            {
                app.run("select.set", json!({ "ids": [id] })).ok();
            }
        });
        if let (Some(object), Some(rule @ ("dense" | "long-stitch"))) = (f["object"].as_u64(), f["rule"].as_str()) {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                fix_menu(app, ui, object, rule);
            });
        }
    }
    if findings.len() > MAX_CHECK_ROWS {
        widgets::dim_label(ui, tl!("Only the first 30 are listed."));
    }
}

/// A finding's Fix menu: the adviser's suggested fixes (embroidery.fixes, worked out when the menu
/// opens and kept while the document stays the same), each made in one click as one undo step.
fn fix_menu(app: &mut VectorcraftApp, ui: &mut Ui, object: u64, rule: &str) {
    let revision = app.session.active().map(|d| (d.uid, d.revision));
    let key = egui::Id::unique(("embroidery.fixes", object, rule));
    ui.menu_button(tl!("Fix…"), |ui| {
        let cached = ui.data(|d| d.get_temp::<(Option<(u64, u64)>, Vec<Value>)>(key)).filter(|(r, _)| *r == revision);
        let list = match cached {
            Some((_, list)) => list,
            None => {
                let list = app
                    .session
                    .execute("embroidery.fixes", &json!({"object": object, "rule": rule}))
                    .ok()
                    .and_then(|r| r["fixes"].as_array().cloned())
                    .unwrap_or_default();
                ui.data_mut(|d| d.insert_temp(key, (revision, list.clone())));
                list
            }
        };
        if list.is_empty() {
            widgets::dim_label(ui, tl!("No one-click fix: change the design by hand (hover the finding for how)."));
        }
        for x in &list {
            let label = x["label"].as_str().unwrap_or_default();
            let tip = if x["clears"] == json!(true) {
                tl!("Clears it").to_string()
            } else {
                tl!("Thins the spot to {n} needle holes; fix again to clear it").replace("{n}", &x["holes"].to_string())
            };
            if ui.button(label).on_hover_text(tip).clicked() {
                let fix = x["fix"].as_str().unwrap_or_default();
                match app.run("embroidery.fix", json!({"object": object, "rule": rule, "fix": fix})) {
                    Ok(r) => app.status(format!("{}: {} warnings left", r["label"].as_str().unwrap_or_default(), r["findings"]["warning"])),
                    Err(e) => app.status(e),
                }
                ui.close();
            }
        }
    });
}

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

/// Whether the selection holds type.
fn selects_type(app: &VectorcraftApp) -> bool {
    app.session
        .active()
        .is_some_and(|d| d.selection.objects.iter().any(|id| matches!(d.doc.node(*id).map(|n| &n.kind), Some(vectorcraft_doc::NodeKind::Text(_)))))
}

/// Embroidery Font (type): none (the outline font), one of the document's, or one of the library's
/// (loaded into the document when picked).
fn font_row(app: &mut VectorcraftApp, ui: &mut Ui, sel: &Value) {
    let mine: Vec<(String, String)> = app
        .session
        .active()
        .map(|d| d.doc.embroidery.fonts.iter().filter(|f| f.unknown.is_none()).map(|f| (f.id.clone(), f.name.clone())).collect())
        .unwrap_or_default();
    let libs = &app.session.embroidery_libraries.fonts;
    let library: Vec<(String, String)> = libs
        .infos()
        .filter(|i| i.category == "user")
        .filter_map(|i| libs.get(&i.id))
        .filter(|(_, f)| !mine.iter().any(|(id, _)| *id == f.id))
        .map(|(i, f)| (i.id, f.name.clone()))
        .chain(app.session.embroidery_libraries.inkstitch.iter().map(|f| (f.id.clone(), f.name.clone())))
        .collect();
    let mut names = vec![tl!("None").to_string()];
    names.extend(mine.iter().map(|(_, n)| n.clone()));
    names.extend(library.iter().map(|(_, n)| library_label(n)));
    let current = sel["font"].as_str();
    let shown = current.and_then(|id| mine.iter().find(|(m, _)| m == id)).map_or_else(|| tl!("None").to_string(), |(_, n)| n.clone());
    ui.scope(|ui| {
        widgets::label_row(ui, tl!("Embroidery Font:"), LABEL_W, |ui| {
            let opts: Vec<&str> = names.iter().map(String::as_str).collect();
            let Some(i) = names_dropdown(ui, "emb-font", &shown, &opts) else { return };
            if i == 0 {
                set(app, json!({"font": null}));
            } else if let Some((id, _)) = mine.get(i - 1) {
                set(app, json!({ "font": id }));
            } else if let Some((lib, _)) = library.get(i - 1 - mine.len()) {
                match app.run("embroidery.loadFont", json!({ "library": lib })) {
                    Ok(r) => set(app, json!({ "font": r["id"] })),
                    Err(e) => app.status(e),
                }
            }
        })
    })
    .response
    .on_hover_text(tl!("Type only: sew the letters from this embroidery font's stitch objects instead of the outline font"));
    indented(ui, |ui| {
        add_font_folder_button(app, ui);
    });
}

/// A stop's name, translated (`None` for no stop).
fn stop_label(id: Option<&str>) -> &'static str {
    match id {
        None => tl!("None"),
        Some("placeFabric") => tl!("Place Fabric"),
        Some("trimFabric") => tl!("Trim Fabric"),
        Some(_) => tl!("Stop"),
    }
}

/// Stop Before: none, or a stop for something done by hand before the object is sewn.
fn stop_row(app: &mut VectorcraftApp, ui: &mut Ui, sel: &Value) {
    let ids: Vec<Option<&str>> = std::iter::once(None).chain(StitchSettings::STOPS.iter().map(|s| Some(*s))).collect();
    let names: Vec<&str> = ids.iter().map(|id| stop_label(*id)).collect();
    let shown = stop_label(sel["stopBefore"].as_str());
    ui.scope(|ui| {
        widgets::label_row(ui, tl!("Stop Before:"), LABEL_W, |ui| {
            if let Some(id) = widgets::dropdown_names(ui, "emb-stop-before", shown, &names, 120.0).and_then(|i| ids.get(i)) {
                set(app, json!({ "stopBefore": id }));
            }
        })
    })
    .response
    .on_hover_text(tl!("Stop the machine before this object: to change something by hand, place appliqué fabric, or trim it"));
}

/// The thread the selection is sewn in: a chip of the colour sewn (its own thread, else the
/// object's colour), which opens a colour picker and the chosen chart's threads; its hex as a
/// second way in; and the chart thread it matches.
fn thread_row(app: &mut VectorcraftApp, ui: &mut Ui, sel: &Value) {
    let own = sel["thread"].as_str().and_then(rgb);
    let sewn = sel["sewn"].as_str().and_then(rgb);
    widgets::label_row(ui, tl!("Color:"), LABEL_W, |ui| {
        if let Some(c) = sewn {
            let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
            ui.painter().rect_filled(r, 3.0, Color32::from_rgb(c[0], c[1], c[2]));
            let edge = if resp.hovered() { Tokens::get(ui.ctx()).accent } else { Color32::from_gray(110) };
            ui.painter().rect_stroke(r, 3.0, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
            let resp = resp.on_hover_text(tl!("Choose the thread's color"));
            let clicked = resp.clicked();
            widgets::popover(&resp, clicked, |ui| thread_picker(app, ui, c));
        }
        let hex = sel["sewn"].as_str().unwrap_or("").trim_start_matches('#').to_string();
        if let Some(h) = widgets::hex_field(ui, "emb-thread", &hex)
            && rgb(&h).is_some()
        {
            set(app, json!({"thread": format!("#{}", h.trim_start_matches('#'))}));
        }
    });
    // On a line of their own: beside the field they ran past the panel's edge.
    indented(ui, |ui| {
        let t = &sel["sewnThread"];
        if let Some(code) = t["code"].as_str() {
            let text = format!("{code} {}", t["name"].as_str().unwrap_or(""));
            ui.add(egui::Label::new(egui::RichText::new(text).color(Tokens::get(ui.ctx()).text_dim)).truncate())
                .on_hover_text(format!("ΔE {}", t["deltaE"]));
        }
        match own {
            Some(_) => {
                if widgets::flat_button(ui, tl!("Use Object Color"), 110.0).clicked() {
                    set(app, json!({"thread": null}));
                }
            }
            None => {
                ui.label(tl!("Object color")).on_hover_text(tl!("The stroke's color for stitches along the path, the fill's for fills"));
            }
        }
    });
}

/// The thread chip's popover: a colour picker, then the chosen chart's threads. A drag in the
/// picker sets the thread once, when it ends, so it is one undo step.
fn thread_picker(app: &mut VectorcraftApp, ui: &mut Ui, sewn: [u8; 3]) {
    let id = egui::Id::unique("emb-thread-picking");
    let mut c = ui.ctx().data(|d| d.get_temp::<[u8; 3]>(id)).unwrap_or(sewn);
    let mut color = Color32::from_rgb(c[0], c[1], c[2]);
    if egui::widgets::color_picker::color_picker_color32(ui, &mut color, egui::widgets::color_picker::Alpha::Opaque) {
        c = [color.r(), color.g(), color.b()];
    }
    if ui.input(|i| i.pointer.any_down()) {
        ui.ctx().data_mut(|d| d.insert_temp(id, c));
    } else {
        ui.ctx().data_mut(|d| d.remove::<[u8; 3]>(id));
        if c != sewn {
            set(app, json!({"thread": format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])}));
        }
    }
    let chart = app.session.active().and_then(|d| {
        let e = &d.doc.embroidery;
        e.chart.as_deref().and_then(|id| e.charts.iter().find(|ch| ch.id == id && ch.unknown.is_none())).cloned()
    });
    let Some(chart) = chart else { return };
    ui.separator();
    ui.label(egui::RichText::new(&chart.name).color(Tokens::get(ui.ctx()).text_dim));
    egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
        ui.set_max_width(260.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
            for t in &chart.threads {
                let Some(tc) = rgb(&t.color) else { continue };
                let (r, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
                ui.painter().rect_filled(r, 2.0, Color32::from_rgb(tc[0], tc[1], tc[2]));
                let edge = if tc == sewn { Tokens::get(ui.ctx()).accent } else { Color32::from_gray(110) };
                ui.painter().rect_stroke(r, 2.0, Stroke::new(if tc == sewn { 2.0 } else { 1.0 }, edge), egui::StrokeKind::Inside);
                if resp.on_hover_text(format!("{} {}", t.code, t.name)).clicked() {
                    set(app, json!({"thread": format!("#{:02x}{:02x}{:02x}", tc[0], tc[1], tc[2])}));
                }
            }
        });
    });
}

fn parameters(app: &mut VectorcraftApp, ui: &mut Ui, kind: StitchKind, sel: &Value) {
    let num = |k: &str| sel[k].as_f64();
    match kind {
        StitchKind::Running | StitchKind::Bean => {
            if kind == StitchKind::Running {
                run_style_picker(app, ui, sel);
            }
            mm(app, ui, tl!("Stitch Length:"), "length", num("length"), StitchSettings::LENGTH, tl!("How long each stitch is"));
            if kind == StitchKind::Running {
                // Each style's own settings, shown only for the styles that use them (a mixed
                // selection shows them all).
                let style = sel["style"].as_str().and_then(RunStyle::parse);
                if style.is_none_or(RunStyle::has_width) {
                    mm(
                        app,
                        ui,
                        tl!("Width:"),
                        "width",
                        num("width"),
                        StitchSettings::RUN_WIDTH,
                        tl!("How wide the chain's loops or the stem's slant are"),
                    );
                }
                if style.is_none_or(|s| s == RunStyle::Stem) {
                    let flip = sel["flip"].as_bool();
                    indented(ui, |ui| {
                        if widgets::check3_tip(ui, tl!("Flip"), flip, true, tl!("Slant the stem stitches the other way")) {
                            set(app, json!({"flip": flip != Some(true)}));
                        }
                    });
                }
                if style.is_none_or(|s| s == RunStyle::Sashiko) {
                    mm(app, ui, tl!("Gap:"), "gap", num("gap"), StitchSettings::GAP, tl!("The space between sashiko dashes"));
                }
            }
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
            let rails = sel["rails"].as_bool();
            indented(ui, |ui| {
                if widgets::check3_tip(
                    ui,
                    tl!("Between Rails"),
                    rails,
                    true,
                    tl!("The object's two paths are the column's edges; lines crossing both set the stitch direction"),
                ) {
                    set(app, json!({"rails": rails != Some(true)}));
                }
            });
            if rails != Some(true) {
                let width = sel["width"].as_f64();
                widgets::label_row(ui, tl!("Width:"), LABEL_W, |ui| {
                    if let Some(v) = widgets::mixed_field(ui, ("emb", "width"), width, " mm", 2, FIELD_W) {
                        let r = width_range(sel);
                        set(app, json!({"width": v.clamp(*r.start(), *r.end())}));
                    }
                });
                indented(ui, |ui| {
                    if width.is_none() {
                        widgets::dim_label(ui, tl!("the stroke width"));
                    } else if widgets::flat_button(ui, tl!("Use Stroke"), 80.0).clicked() {
                        set(app, json!({"width": null}));
                    }
                });
            }
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
            split_rows(app, ui, sel);
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
            fill_pattern_picker(app, ui, sel);
            if sel["motif"].is_string() {
                mm(
                    app,
                    ui,
                    tl!("Motif Size:"),
                    "motifSize",
                    num("motifSize"),
                    StitchSettings::MOTIF_SIZE,
                    tl!("How large one repeat of the motif is"),
                );
            }
            underlay_row(app, ui, kind, sel);
            border_row(app, ui, sel);
        }
        StitchKind::MotifFill => {
            widgets::section_header(ui, tl!("Motif Fill"));
            motif_fill_picker(app, ui, sel);
            mm(app, ui, tl!("Motif Size:"), "motifSize", num("motifSize"), StitchSettings::MOTIF_SIZE, tl!("How large one tile of the motif is"));
            widgets::label_row(ui, tl!("Angle:"), LABEL_W, |ui| {
                if let Some(v) = widgets::mixed_field(ui, ("emb", "angle"), num("angle"), "°", 1, FIELD_W) {
                    set(app, json!({"angle": v}));
                }
            });
            mm(app, ui, tl!("Stitch Length:"), "length", num("length"), StitchSettings::LENGTH, tl!("How long each stitch is"));
        }
        StitchKind::Auto => {
            widgets::section_header(ui, tl!("Auto"));
            widgets::dim_label(ui, tl!("Each shape (or letter) is stitched by how wide its strokes are."));
            mm(
                app,
                ui,
                tl!("Bean Outline Under:"),
                "thinUnder",
                num("thinUnder"),
                StitchSettings::THIN,
                tl!("Thinner strokes get a bean stitch round their outline"),
            );
            mm(
                app,
                ui,
                tl!("Fill Over:"),
                "wideOver",
                num("wideOver"),
                StitchSettings::WIDE,
                tl!("Wider strokes are filled, with a satin border; strokes in between get satin"),
            );
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
            border_row(app, ui, sel);
        }
        StitchKind::SatinShape => {
            widgets::section_header(ui, tl!("Satin Shape"));
            widgets::dim_label(ui, tl!("Satin follows the shape's strokes from edge to edge."));
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
            split_rows(app, ui, sel);
        }
        StitchKind::Zigzag => {
            widgets::section_header(ui, tl!("Zigzag Stitch"));
            // Unset, a zigzag is 3 mm wide (it doesn't follow the stroke, drawn thin).
            let width = Some(num("width").unwrap_or(StitchSettings::DEFAULT_WIDTH));
            mm(app, ui, tl!("Width:"), "width", width, width_range(sel), tl!("How wide the zigzag swings"));
            mm(app, ui, tl!("Spacing:"), "spacing", num("spacing"), StitchSettings::SPACING, tl!("Distance between the points of the zigzag"));
            split_rows(app, ui, sel);
        }
        StitchKind::PatternBorder => {
            widgets::section_header(ui, tl!("Pattern Border"));
            widgets::dim_label(ui, tl!("The pattern repeats along the path, a whole number of times between corners."));
            // The document's own patterns, then the starters.
            // Patterns from a newer version can't be used, so they aren't offered.
            let list: Vec<&Value> =
                sel["borderPatterns"].as_array().map(|a| a.iter().filter(|p| p["supported"] != json!(false)).collect()).unwrap_or_default();
            let pattern = sel["pattern"].as_str();
            let name = |p: &Value| -> String {
                let id = p["id"].as_str().unwrap_or_default();
                if p["builtin"] == json!(true) { border_label(id).to_string() } else { p["name"].as_str().unwrap_or(id).to_string() }
            };
            let current = list.iter().find(|p| p["id"].as_str() == pattern).copied();
            // Each pattern sewn along a line, the document's from its own data, the starters
            // from theirs.
            let own: Vec<vectorcraft_doc::BorderPattern> = app.session.active().map(|d| d.doc.embroidery.patterns.clone()).unwrap_or_default();
            let ctx = ui.ctx().clone();
            let entries: Vec<(String, stitch_art::Thumb)> = list
                .iter()
                .map(|p| {
                    let id = p["id"].as_str().unwrap_or_default();
                    let data = own.iter().find(|q| q.id == id).cloned().or_else(|| vectorcraft_embroidery::borders::builtin(id));
                    (name(p), data.map(|d| stitch_art::border(&ctx, &d)).unwrap_or_default())
                })
                .collect();
            // Then the embroidery library's, unless the document or the starters have the same.
            let have = |q: &vectorcraft_doc::BorderPattern| {
                own.iter().any(|o| o == q) || vectorcraft_embroidery::borders::builtin(&q.id).is_some_and(|b| b == *q)
            };
            let libs = &app.session.embroidery_libraries.borders;
            let library: Vec<(String, vectorcraft_doc::BorderPattern)> = libs
                .infos()
                .filter(|i| i.category == "user")
                .filter_map(|i| libs.get(&i.id))
                .filter(|(_, q)| q.unknown.is_none() && !have(q))
                .map(|(i, q)| (i.id, (*q).clone()))
                .collect();
            let mut entries = entries;
            entries.extend(library.iter().map(|(_, q)| (library_label(&q.name), stitch_art::border(&ctx, q))));
            let selected = list.iter().position(|p| p["id"].as_str() == pattern);
            widgets::label_row(ui, tl!("Pattern:"), LABEL_W, |ui| {
                let (label, thumb) = selected.and_then(|i| entries.get(i)).map(|(l, th)| (l.clone(), Some(th))).unwrap_or_default();
                if let Some(i) = picture_picker(ui, "border-pattern", &label, thumb, &entries, selected, false) {
                    match (list.get(i).and_then(|p| p["id"].as_str()), i.checked_sub(list.len()).and_then(|j| library.get(j))) {
                        (Some(id), _) => set(app, json!({"pattern": id})),
                        (None, Some((lib, _))) => load_pattern_from(app, json!({ "library": lib })),
                        _ => {}
                    }
                }
            });
            indented(ui, |ui| {
                if widgets::flat_button(ui, tl!("Load…"), 70.0)
                    .on_hover_text(tl!("Load a border pattern file (.vcborder) into the document"))
                    .clicked()
                {
                    load_pattern(app);
                }
                if let Some(id) = pattern.map(str::to_string)
                    && widgets::flat_button(ui, tl!("Save…"), 70.0).on_hover_text(tl!("Save this border pattern as a file (.vcborder)")).clicked()
                {
                    crate::picks::button(app, move |app| {
                        let file = format!("{id}.{}", vectorcraft_embroidery::borders::FILE_EXT);
                        if let Err(e) = crate::io::save_command_output_named(app, "embroidery.saveBorderPattern", &file, json!({ "pattern": id })) {
                            app.status(e);
                        }
                    });
                }
                if let Some(mine) =
                    current.filter(|p| p["builtin"] != json!(true)).and_then(|p| own.iter().find(|o| p["id"].as_str() == Some(o.id.as_str())))
                    && add_button(app, ui, !in_library(&app.session.embroidery_libraries.borders, mine))
                {
                    let file = format!("{}.{}", mine.name, vectorcraft_embroidery::borders::FILE_EXT);
                    add_to_library(app, "embroidery.saveBorderPattern", json!({ "pattern": mine.id }), &file, "borders");
                }
            });
            if let Some(p) = current.filter(|p| p["builtin"] != json!(true)) {
                let credit = [p["author"].as_str(), p["license"].as_str()].into_iter().flatten().collect::<Vec<_>>().join(" · ");
                if !credit.is_empty() {
                    widgets::dim_label(ui, &credit);
                }
            }
            ui.scope(|ui| {
                let width = sel["width"].as_f64();
                widgets::label_row(ui, tl!("Width:"), LABEL_W, |ui| {
                    if let Some(v) = widgets::mixed_field(ui, ("emb", "patternWidth"), width, " mm", 1, FIELD_W) {
                        let r = StitchSettings::PATTERN_WIDTH;
                        set(app, json!({"width": v.clamp(*r.start(), *r.end())}));
                    }
                });
                indented(ui, |ui| {
                    if width.is_none() {
                        widgets::dim_label(ui, tl!("the pattern's size"));
                    } else if widgets::flat_button(ui, tl!("Pattern Size"), 90.0).clicked() {
                        set(app, json!({"width": null}));
                    }
                })
            })
            .response
            .on_hover_text(tl!("How wide the border is: the pattern scales with it"));
            let flip = sel["flip"].as_bool();
            indented(ui, |ui| {
                if widgets::check3_tip(ui, tl!("Flip"), flip, true, tl!("Mirror the pattern across the path")) {
                    set(app, json!({"flip": flip != Some(true)}));
                }
            });
            mm(app, ui, tl!("Stitch Length:"), "length", num("length"), StitchSettings::LENGTH, tl!("How long each stitch is"));
            // Density only matters to patterns with satin in them.
            if current.is_some_and(|p| p["satin"] == json!(true)) {
                mm(app, ui, tl!("Density:"), "spacing", num("spacing"), StitchSettings::SPACING, tl!("Distance between zigzags: smaller is denser"));
            }
        }
        StitchKind::Manual => {
            widgets::dim_label(ui, tl!("The needle goes down at every anchor point, as drawn."));
        }
    }
    // The settings without rows of their own, folded away.
    let own = own_rows(kind);
    let more =
        vectorcraft_embroidery::params::PARAMS.iter().filter(|p| p.applies(kind) && !own.contains(&p.key) && !ALWAYS_SHOWN.contains(&p.key)).count();
    if more > 0 {
        egui::CollapsingHeader::new(tl!("More Settings ({n})").replace("{n}", &more.to_string()))
            .id_salt(("emb-more", kind.id()))
            .default_open(false)
            .show(ui, |ui| other_settings(app, ui, kind, sel, own));
    }
}

/// Settings every stitch type's panel shows above its own rows (stitch, thread, trims).
const ALWAYS_SHOWN: [&str; 6] = ["type", "thread", "trimBefore", "trimAfter", "stopBefore", "font"];

/// The settings each stitch type's own rows show; any other setting it uses
/// ([`vectorcraft_embroidery::params::PARAMS`]) gets a generic row from [`other_settings`].
fn own_rows(kind: StitchKind) -> &'static [&'static str] {
    match kind {
        StitchKind::Running => &["style", "length", "width", "flip", "gap"],
        StitchKind::Bean => &["length", "passes"],
        StitchKind::Satin => &["rails", "width", "spacing", "pull", "underlay", "split", "splitAt", "splitLength", "splitPattern"],
        StitchKind::Zigzag => &["width", "spacing", "split", "splitAt", "splitLength", "splitPattern"],
        StitchKind::SatinShape => &["spacing", "pull", "underlay", "split", "splitAt", "splitLength", "splitPattern"],
        StitchKind::Fill => &["angle", "spacing", "length", "stagger", "motif", "motifSize", "underlay", "border"],
        StitchKind::PatternBorder => &["pattern", "width", "flip", "length", "spacing"],
        StitchKind::MotifFill => &["motif", "motifSize", "angle", "length", "spacing"],
        StitchKind::Auto => &["thinUnder", "wideOver", "spacing", "pull", "border"],
        StitchKind::Manual => &[],
    }
}

/// A row for each setting `kind` uses that isn't in `own` (the rows it draws itself), drawn from
/// the settings table: a new setting appears here until it is given a row of its own.
fn other_settings(app: &mut VectorcraftApp, ui: &mut Ui, kind: StitchKind, sel: &Value, own: &[&str]) {
    use vectorcraft_embroidery::params::{PARAMS, ValueKind};
    for p in PARAMS.iter().filter(|p| p.applies(kind) && !own.contains(&p.key) && !ALWAYS_SHOWN.contains(&p.key)) {
        let (key, label, help) = (p.key, tl!(p.label), tl!(p.help));
        match &p.value {
            ValueKind::Mm(r) => mm(app, ui, label, key, sel[key].as_f64(), r.clone(), help),
            ValueKind::Percent(r) => {
                ui.scope(|ui| {
                    widgets::label_row(ui, label, LABEL_W, |ui| {
                        if let Some(v) = widgets::mixed_field(ui, ("emb", key), sel[key].as_f64(), " %", 1, FIELD_W) {
                            set(app, json!({ key: v.clamp(*r.start(), *r.end()) }));
                        }
                    })
                })
                .response
                .on_hover_text(help);
            }
            ValueKind::Degrees => {
                ui.scope(|ui| {
                    widgets::label_row(ui, label, LABEL_W, |ui| {
                        if let Some(v) = widgets::mixed_field(ui, ("emb", key), sel[key].as_f64(), "°", 1, FIELD_W) {
                            set(app, json!({ key: v }));
                        }
                    })
                })
                .response
                .on_hover_text(help);
            }
            ValueKind::Whole { range, odd } => {
                let values: Vec<String> = range.clone().filter(|n| !odd || n % 2 == 1).map(|n| n.to_string()).collect();
                let names: Vec<&str> = values.iter().map(String::as_str).collect();
                let cur = sel[key].as_u64().map(|n| n.to_string()).unwrap_or_default();
                widgets::label_row(ui, label, LABEL_W, |ui| {
                    if let Some(i) = widgets::dropdown_names(ui, ("emb-generic", key), &cur, &names, FIELD_W)
                        && let Some(v) = values.get(i).and_then(|v| v.parse::<u64>().ok())
                    {
                        set(app, json!({ key: v }));
                    }
                });
            }
            ValueKind::Flag => {
                let v = sel[key].as_bool();
                indented(ui, |ui| {
                    if widgets::check3_tip(ui, label, v, true, help) {
                        set(app, json!({ key: v != Some(true) }));
                    }
                });
            }
            ValueKind::Choice(ids) => {
                let cur = sel[key].as_str().unwrap_or_default();
                widgets::label_row(ui, label, LABEL_W, |ui| {
                    if let Some(i) = names_dropdown(ui, ("emb-generic", key), cur, ids)
                        && let Some(id) = ids.get(i)
                    {
                        set(app, json!({ key: id }));
                    }
                });
            }
            // A start point: shown, and cleared; set by commands (or, later, a handle).
            ValueKind::Point => {
                let at = sel[key].as_array().and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?]));
                ui.scope(|ui| {
                    widgets::label_row(ui, label, LABEL_W, |ui| match at {
                        Some([x, y]) => {
                            ui.label(format!("{x:.1}, {y:.1} mm"));
                            if widgets::flat_button(ui, tl!("Clear"), 50.0).clicked() {
                                set(app, json!({ key: null }));
                            }
                        }
                        // None set: where the needle is.
                        None => {
                            ui.label(tl!("Auto"));
                        }
                    })
                })
                .response
                .on_hover_text(help);
            }
            // Stitch angle lines: how many, the tool that draws them, and clearing them.
            ValueKind::Lines => {
                let n = sel[key].as_array().map_or(0, Vec::len);
                ui.scope(|ui| {
                    widgets::label_row(ui, label, LABEL_W, |ui| {
                        ui.label(if n == 0 { tl!("None").to_string() } else { n.to_string() });
                        if widgets::flat_button(ui, tl!("Draw"), 50.0)
                            .on_hover_text(tl!("The Stitch Angle tool: drag across the satin to add a line, drag a line's end to move it, Alt-click a line to remove it"))
                            .clicked()
                            && let Err(e) = app.run("tool.select", json!({"tool": "stitchAngle"}))
                        {
                            app.status(e);
                        }
                        if n > 0 && widgets::flat_button(ui, tl!("Clear"), 50.0).clicked() {
                            set(app, json!({ key: null }));
                        }
                    })
                })
                .response
                .on_hover_text(help);
            }
            // The stitch type, the thread and ids have rows of their own above.
            ValueKind::Kind | ValueKind::Thread | ValueKind::Id => {}
        }
    }
}

/// Fill › Border: a satin over the shape's edges (off, or its width).
fn border_row(app: &mut VectorcraftApp, ui: &mut Ui, sel: &Value) {
    let border = sel["border"].as_f64();
    widgets::label_row(ui, tl!("Border:"), LABEL_W, |ui| {
        if widgets::check3_tip(ui, "", Some(border.is_some()), true, tl!("A satin border sewn over the shape's edges, after the fill")) {
            set(app, json!({"border": if border.is_some() { Value::Null } else { json!(2.0) }}));
        }
        ui.add_enabled_ui(border.is_some(), |ui| {
            if let Some(v) = widgets::mixed_field(ui, ("emb", "border"), border, " mm", 1, FIELD_W) {
                let r = StitchSettings::BORDER;
                set(app, json!({"border": v.clamp(*r.start(), *r.end())}));
            }
        });
    });
}

/// A carved motif's name, translated.
fn motif_label(id: &str) -> &'static str {
    match id {
        "diamonds" => tl!("Diamonds"),
        "waves" => tl!("Waves"),
        "circles" => tl!("Circles"),
        "scales" => tl!("Scales"),
        "herringbone" => tl!("Herringbone"),
        "stars" => tl!("Stars"),
        "flowers" => tl!("Flowers"),
        "crosses" => tl!("Crosses"),
        "hearts" => tl!("Hearts"),
        "dots" => tl!("Dots"),
        _ => tl!("Motif"),
    }
}

/// Extract Border Pattern and Extract Motif: the repeat of the selected border stitches, added to
/// the document's border patterns, or the tile of the selected fill's, to its motifs.
fn extract_row(app: &mut VectorcraftApp, ui: &mut Ui) {
    widgets::divider(ui);
    widgets::dim_label(ui, tl!("Select a straight stretch of a border with two or more repeats, or a shape drawn over it too."));
    if widgets::flat_button(ui, tl!("Extract Border Pattern"), 170.0).clicked() {
        match app.run("embroidery.extractBorderPattern", json!({})) {
            Ok(r) => {
                let period = r["period"].as_f64().unwrap_or_default();
                app.status(format!("Extracted border pattern {}: a {} mm repeat", r["name"].as_str().unwrap_or_default(), period));
            }
            Err(e) => app.status(e),
        }
    }
    widgets::dim_label(ui, tl!("For a fill repeating both ways: select an area with two or more repeats each way."));
    if widgets::flat_button(ui, tl!("Extract Motif"), 170.0).clicked() {
        match app.run("embroidery.extractMotif", json!({})) {
            Ok(r) => app.status(format!(
                "Extracted motif {}: {} stitches a tile, about {} tiles found",
                r["name"].as_str().unwrap_or_default(),
                r["stitches"].as_u64().unwrap_or_default(),
                r["tiles"].as_u64().unwrap_or_default()
            )),
            Err(e) => app.status(e),
        }
    }
}

/// Load a border pattern file the user picks into the document (the web: the picked file arrives
/// later, through [`crate::io::open_bytes`]), and sew the selected pattern borders with it.
fn load_pattern(app: &mut VectorcraftApp) {
    if let Some(f) = app.services.open_async.as_mut() {
        f();
        return;
    }
    crate::picks::button(app, |app| {
        let ext: &'static [&'static str] = &[vectorcraft_embroidery::borders::FILE_EXT];
        let pick = crate::FilePick { filters: vec![crate::FilePick::filter("Border patterns", ext)], ..Default::default() };
        let Some(path) = crate::picks::open(app, &pick) else { return };
        load_pattern_from(app, json!({ "path": path }));
    });
}

/// `embroidery.loadBorderPattern` with `p`, then sew the selected pattern borders with it.
pub(crate) fn load_pattern_from(app: &mut VectorcraftApp, p: Value) {
    match app.run("embroidery.loadBorderPattern", p) {
        Ok(r) => {
            let sel = app.session.execute("embroidery.info", &json!({})).unwrap_or(Value::Null);
            if sel["selection"]["type"] == json!(StitchKind::PatternBorder.id()) {
                set(app, json!({ "pattern": r["id"] }));
            }
            app.status(format!("Loaded border pattern {}", r["name"].as_str().unwrap_or_default()));
        }
        Err(e) => app.status(e),
    }
}

/// A border pattern's name, translated.
fn border_label(id: &str) -> &'static str {
    match id {
        "waves" => tl!("Waves"),
        "scallops" => tl!("Scallops"),
        "chain" => tl!("Chain"),
        "diamonds" => tl!("Diamonds"),
        _ => tl!("Pattern"),
    }
}

/// A starter fabric's name, translated.
/// A starter stitch style's name and description, translated (`None`: not a starter).
fn starter_style(id: &str) -> Option<(&'static str, &'static str)> {
    Some(match id {
        "small-lettering" => {
            (tl!("Small Lettering"), tl!("Type 5–10 mm tall: letters picked by stroke width, a little denser, with a little pull compensation."))
        }
        "bold-border" => (tl!("Bold Border"), tl!("A 4 mm satin border with automatic underlay, for outlining patches and shapes.")),
        "light-fill" => {
            (tl!("Light Background Fill"), tl!("An open fill for large backgrounds: fewer stitches, the fabric shows a little, no underlay."))
        }
        _ => return None,
    })
}

/// The Stitch Styles menu: apply a style (the document's, a starter, the library's) to the
/// selection; with `can_save`, save the selection's settings as one; load or remove one.
fn styles_menu(app: &mut VectorcraftApp, ui: &mut Ui, can_save: bool) {
    ui.menu_button(tl!("Stitch Styles"), |ui| styles_list(app, ui, can_save))
        .response
        .on_hover_text(tl!("Named sets of stitch settings: apply one to the selection, or save the selection's as one"));
}

/// What the Stitch Styles menu holds.
fn styles_list(app: &mut VectorcraftApp, ui: &mut Ui, can_save: bool) {
    {
        let list = app.session.execute("embroidery.styles", &json!({})).unwrap_or_default();
        let selected = list["selected"].as_str().map(str::to_string);
        let styles: Vec<Value> =
            list["styles"].as_array().map(|a| a.iter().filter(|st| st["supported"] == json!(true)).cloned().collect()).unwrap_or_default();
        let apply = |app: &mut VectorcraftApp, p: Value| {
            if let Err(e) = app.run("embroidery.applyStyle", p) {
                app.status(e);
            }
        };
        for st in &styles {
            let id = st["id"].as_str().unwrap_or_default();
            let starter = starter_style(id).filter(|_| st["builtin"] == json!(true));
            let name = starter.map_or_else(|| st["name"].as_str().unwrap_or(id).to_string(), |(n, _)| n.to_string());
            let about = starter.map(|(_, a)| a.to_string()).or_else(|| st["description"].as_str().map(str::to_string));
            let mut r = ui.selectable_label(selected.as_deref() == Some(id), name);
            if let Some(a) = about {
                r = r.on_hover_text(a);
            }
            if r.clicked() {
                apply(app, json!({ "style": id }));
                ui.close();
            }
        }
        let libs = &app.session.embroidery_libraries.styles;
        let library: Vec<(String, String)> =
            libs.infos().filter(|i| i.category == "user").filter_map(|i| libs.get(&i.id)).map(|(i, st)| (i.id, st.name.clone())).collect();
        if !library.is_empty() {
            ui.menu_button(tl!("Library"), |ui| {
                for (id, name) in &library {
                    if ui.button(name).clicked() {
                        apply(app, json!({ "library": id }));
                        ui.close();
                    }
                }
            });
        }
        ui.separator();
        if can_save {
            ui.menu_button(tl!("Save Style"), |ui| {
                let mut name = kept(ui, "style-name", "");
                widgets::label_row(ui, tl!("Name:"), 50.0, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut name).desired_width(140.0).hint_text(tl!("Stitch Style")));
                });
                keep(ui, "style-name", name.clone());
                ui.horizontal(|ui| {
                    let save = |app: &mut VectorcraftApp, user: bool| {
                        let mut p = json!({ "user": user });
                        if !name.trim().is_empty() {
                            p["name"] = json!(name.trim());
                        }
                        match app.run("embroidery.saveStyle", p) {
                            Ok(r) => app.status(format!("Saved stitch style {}", r["name"].as_str().unwrap_or_default())),
                            Err(e) => app.status(e),
                        }
                    };
                    if widgets::flat_button(ui, tl!("Save"), 60.0)
                        .on_hover_text(tl!("Keep the selection's stitch settings in this document"))
                        .clicked()
                    {
                        save(app, false);
                        ui.close();
                    }
                    if app.session.embroidery_libraries.dir().is_some()
                        && widgets::flat_button(ui, tl!("Save to Library"), 110.0)
                            .on_hover_text(tl!("Save a copy in your embroidery library, for any design"))
                            .clicked()
                    {
                        save(app, true);
                        ui.close();
                    }
                });
            });
        }
        if ui.button(tl!("Load Style File…")).clicked() {
            load_style(app);
            ui.close();
        }
        let mine: Vec<&Value> = styles.iter().filter(|st| st["builtin"] == json!(false)).collect();
        if !mine.is_empty() {
            ui.menu_button(tl!("Remove"), |ui| {
                for st in mine {
                    if ui.button(st["name"].as_str().unwrap_or_default()).clicked() {
                        if let Err(e) = app.run("embroidery.removeStyle", json!({ "style": st["id"] })) {
                            app.status(e);
                        }
                        ui.close();
                    }
                }
            });
        }
    }
}

fn load_style(app: &mut VectorcraftApp) {
    if let Some(f) = app.services.open_async.as_mut() {
        f();
        return;
    }
    crate::picks::button(app, |app| {
        let ext: &'static [&'static str] = &[vectorcraft_embroidery::styles::FILE_EXT];
        let pick = crate::FilePick { filters: vec![crate::FilePick::filter("Stitch styles", ext)], ..Default::default() };
        let Some(path) = crate::picks::open(app, &pick) else { return };
        match app.run("embroidery.loadStyle", json!({ "path": path })) {
            Ok(r) => app.status(format!("Stitch style: {}", r["name"].as_str().unwrap_or_default())),
            Err(e) => app.status(e),
        }
    });
}

fn fabric_label(id: &str) -> &'static str {
    match id {
        "woven" => tl!("Woven"),
        "knit" => tl!("Knit"),
        "towel" => tl!("Fleece / Towel"),
        "cap" => tl!("Cap"),
        "leather" => tl!("Leather"),
        "denim" => tl!("Denim"),
        _ => tl!("Custom"),
    }
}

/// The document's fabric: a starter, none, or its own (loaded or edited); Load… and Save….
fn fabric_row(app: &mut VectorcraftApp, ui: &mut Ui) {
    use vectorcraft_embroidery::fabrics;
    let current = app.session.active().and_then(|d| d.doc.embroidery.fabric.as_deref().cloned());
    // A starter as it comes shows under its own name; anything else under the fabric's own.
    let starter = current.as_ref().filter(|f| fabrics::builtin(&f.id).as_ref() == Some(*f)).map(|f| f.id.clone());
    let own = current.as_ref().filter(|_| starter.is_none()).map(|f| f.name.clone());
    let mut names: Vec<String> = vec![tl!("None").to_string()];
    names.extend(fabrics::BUILTIN.iter().map(|id| fabric_label(id).to_string()));
    if let Some(n) = &own {
        names.push(n.clone());
    }
    let first_library = names.len();
    let libs = &app.session.embroidery_libraries.fabrics;
    let library: Vec<(String, String)> =
        libs.infos().filter(|i| i.category == "user").filter_map(|i| libs.get(&i.id)).map(|(i, f)| (i.id, f.name.clone())).collect();
    names.extend(library.iter().map(|(_, n)| library_label(n)));
    let shown = match (&starter, &own) {
        (Some(id), _) => fabric_label(id).to_string(),
        (None, Some(n)) => n.clone(),
        (None, None) => tl!("None").to_string(),
    };
    ui.scope(|ui| {
        widgets::label_row(ui, tl!("Fabric:"), LABEL_W, |ui| {
            let opts: Vec<&str> = names.iter().map(String::as_str).collect();
            if let Some(i) = names_dropdown(ui, "emb-fabric", &shown, &opts) {
                match i {
                    0 => {
                        app.run("embroidery.setFabric", json!({"fabric": null})).ok();
                    }
                    i if i >= first_library => {
                        if let Some((id, _)) = library.get(i - first_library)
                            && let Err(e) = app.run("embroidery.loadFabric", json!({ "library": id }))
                        {
                            app.status(e);
                        }
                    }
                    i => {
                        if let Some(id) = fabrics::BUILTIN.get(i - 1) {
                            app.run("embroidery.setFabric", json!({"fabric": id})).ok();
                        }
                    }
                }
            }
        })
    })
    .response
    .on_hover_text(tl!("The fabric the design is for: its density, compensation and underlay fill in what objects leave unset"));
    indented(ui, |ui| {
        if widgets::flat_button(ui, tl!("Load…"), 70.0).on_hover_text(tl!("Load a fabric profile file (.vcfabric)")).clicked() {
            load_fabric(app);
        }
        if let Some(f) = &current
            && widgets::flat_button(ui, tl!("Save…"), 70.0).on_hover_text(tl!("Save the fabric as a file (.vcfabric)")).clicked()
        {
            let file = format!("{}.{}", if f.id.is_empty() { "fabric" } else { &f.id }, fabrics::FILE_EXT);
            crate::picks::button(app, move |app| {
                if let Err(e) = crate::io::save_command_output_named(app, "embroidery.saveFabric", &file, json!({})) {
                    app.status(e);
                }
            });
        }
        if let Some(f) = current.as_ref().filter(|_| starter.is_none())
            && add_button(app, ui, !in_library(&app.session.embroidery_libraries.fabrics, f))
        {
            add_to_library(app, "embroidery.saveFabric", json!({}), &format!("{}.{}", f.name, fabrics::FILE_EXT), "fabrics");
        }
    });
}

fn texture_label(t: vectorcraft_embroidery::realistic::Texture) -> &'static str {
    use vectorcraft_embroidery::realistic::Texture;
    match t {
        Texture::Plain => tl!("Plain Weave"),
        Texture::Twill => tl!("Twill"),
        Texture::Denim => tl!("Denim"),
        Texture::Knit => tl!("Knit"),
        Texture::Terry => tl!("Terry"),
        Texture::Leather => tl!("Leather"),
        Texture::Felt => tl!("Felt"),
    }
}

/// How View › Realistic Stitches draws the fabric: its texture (as the fabric profile suggests, or
/// one chosen) and colour, or none at all (the threads alone, over the artwork).
fn look_row(app: &mut VectorcraftApp, ui: &mut Ui) {
    use vectorcraft_embroidery::realistic::{Look, NO_FABRIC, Texture};
    let Some(doc) = app.session.active().map(|d| d.doc.clone()) else { return };
    let look = Look::of(&doc);
    let set = doc.embroidery.preview.as_deref().cloned().unwrap_or_default();
    let auto = set.texture.as_deref().and_then(Texture::from_id).is_none();
    let auto = auto && look.fabric;
    // As the Fabric, None, then each texture.
    let mut names: Vec<String> = vec![tl!("As the Fabric").to_string(), tl!("None").to_string()];
    names.extend(Texture::ALL.iter().map(|t| texture_label(*t).to_string()));
    let shown = if !look.fabric {
        tl!("None").to_string()
    } else if auto {
        format!("{} ({})", tl!("As the Fabric"), texture_label(look.texture))
    } else {
        texture_label(look.texture).to_string()
    };
    ui.scope(|ui| {
        widgets::label_row(ui, tl!("Fabric Look:"), LABEL_W, |ui| {
            let opts: Vec<&str> = names.iter().map(String::as_str).collect();
            if let Some(i) = names_dropdown(ui, "emb-look", &shown, &opts) {
                let texture = match i {
                    0 => None,
                    1 => Some(NO_FABRIC),
                    _ => i.checked_sub(2).and_then(|k| Texture::ALL.get(k)).map(|t| t.id()),
                };
                app.run("embroidery.setup", json!({ "fabricTexture": texture })).ok();
            }
        })
    })
    .response
    .on_hover_text(tl!("How Realistic Stitches draws the fabric the design is sewn on"));
    if !look.fabric {
        return;
    }
    // Its colour, on a line of its own under the menu.
    indented(ui, |ui| {
        let mut rgb = look.color;
        if ui.color_edit_button_srgb(&mut rgb).on_hover_text(tl!("The fabric's colour")).changed() {
            let hex = format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
            app.run("embroidery.setup", json!({ "fabricColor": hex })).ok();
        }
        if set.color.is_some() && widgets::flat_button(ui, tl!("Reset"), 48.0).on_hover_text(tl!("The texture's own colour")).clicked() {
            app.run("embroidery.setup", json!({ "fabricColor": null })).ok();
        }
    });
}

/// The thread chart the design's colours are matched against: none, or one loaded into the
/// document; Load….
fn chart_row(app: &mut VectorcraftApp, ui: &mut Ui) {
    let (charts, chosen): (Vec<(String, String)>, Option<String>) = app
        .session
        .active()
        .map(|d| {
            let e = &d.doc.embroidery;
            (e.charts.iter().filter(|c| c.unknown.is_none()).map(|c| (c.id.clone(), c.name.clone())).collect(), e.chart.clone())
        })
        .unwrap_or_default();
    let mut names: Vec<String> = vec![tl!("None").to_string()];
    names.extend(charts.iter().map(|(_, n)| n.clone()));
    // The library's charts the document doesn't have yet (by their id inside).
    let libs = &app.session.embroidery_libraries.threads;
    let library: Vec<(String, String)> = libs
        .infos()
        .filter(|i| i.category == "user")
        .filter_map(|i| libs.get(&i.id))
        .filter(|(_, c)| !charts.iter().any(|(id, _)| *id == c.id))
        .map(|(i, c)| (i.id, c.name.clone()))
        .collect();
    names.extend(library.iter().map(|(_, n)| library_label(n)));
    let shown = chosen.as_ref().and_then(|id| charts.iter().find(|(c, _)| c == id)).map_or_else(|| tl!("None").to_string(), |(_, n)| n.clone());
    ui.scope(|ui| {
        widgets::label_row(ui, tl!("Thread Chart:"), LABEL_W, |ui| {
            let opts: Vec<&str> = names.iter().map(String::as_str).collect();
            if let Some(i) = names_dropdown(ui, "emb-chart", &shown, &opts) {
                if let Some((lib, _)) = i.checked_sub(charts.len() + 1).and_then(|j| library.get(j)) {
                    // Loaded into the document, then chosen.
                    match app.run("embroidery.loadThreadChart", json!({ "library": lib })) {
                        Ok(r) => {
                            app.run("embroidery.setThreadChart", json!({ "chart": r["id"] })).ok();
                        }
                        Err(e) => app.status(e),
                    }
                } else {
                    let chart = if i == 0 { Value::Null } else { charts.get(i - 1).map_or(Value::Null, |(id, _)| json!(id)) };
                    app.run("embroidery.setThreadChart", json!({ "chart": chart })).ok();
                }
            }
        })
    })
    .response
    .on_hover_text(tl!(
        "Each thread colour of the design gets the nearest thread of this chart. Load charts you have the right to use (.vcthreads)."
    ));
    indented(ui, |ui| {
        if widgets::flat_button(ui, tl!("Load…"), 70.0).on_hover_text(tl!("Load a thread chart file (.vcthreads)")).clicked() {
            load_chart(app);
        }
        let mine = chosen.as_ref().and_then(|id| app.session.active()?.doc.embroidery.charts.iter().find(|c| c.id == *id).cloned());
        if let Some(c) = mine
            && add_button(app, ui, !in_library(&app.session.embroidery_libraries.threads, &c))
        {
            let file = format!("{}.{}", c.name, vectorcraft_embroidery::threads::FILE_EXT);
            add_to_library(app, "embroidery.saveThreadChart", json!({ "chart": c.id }), &file, "threads");
        }
    });
}

/// Load a thread chart file the user picks into the document.
fn load_chart(app: &mut VectorcraftApp) {
    if let Some(f) = app.services.open_async.as_mut() {
        f();
        return;
    }
    crate::picks::button(app, |app| {
        let ext: &'static [&'static str] = &[vectorcraft_embroidery::threads::FILE_EXT];
        let pick = crate::FilePick { filters: vec![crate::FilePick::filter("Thread charts", ext)], ..Default::default() };
        let Some(path) = crate::picks::open(app, &pick) else { return };
        if let Err(e) = app.run("embroidery.loadThreadChart", json!({ "path": path })) {
            app.status(e);
        }
    });
}

/// The document's hoop: a common one, none, or its own (loaded); whether the design fits it.
/// The machine file format the document exports in (DST, PES, JEF, EXP, VP3, U01).
pub(crate) fn machine_format(app: &VectorcraftApp) -> &'static str {
    app.session.active().map_or(vectorcraft_doc::EmbroiderySetup::DEFAULT_MACHINE_FORMAT, |d| d.doc.embroidery.machine_format())
}

/// "Export DST…", in the document's machine format.
pub(crate) fn export_label(app: &VectorcraftApp) -> String {
    tl!("Export {format}…").replace("{format}", &machine_format(app).to_uppercase())
}

fn machine_row(app: &mut VectorcraftApp, ui: &mut Ui) {
    let formats = vectorcraft_doc::EmbroiderySetup::MACHINE_FORMATS;
    let names: Vec<String> = formats.iter().map(|f| f.to_uppercase()).collect();
    let current = machine_format(app).to_uppercase();
    ui.scope(|ui| {
        widgets::label_row(ui, tl!("Machine Format:"), LABEL_W, |ui| {
            let opts: Vec<&str> = names.iter().map(String::as_str).collect();
            if let Some(f) = names_dropdown(ui, "emb-machine-format", &current, &opts).and_then(|i| formats.get(i)) {
                app.run("embroidery.setup", json!({ "machineFormat": f })).ok();
            }
        })
    })
    .response
    .on_hover_text(tl!(
        "The file format your machine reads: DST (Tajima and most machines), PES (Brother), JEF (Janome), EXP (Melco), VP3 (Pfaff, Husqvarna Viking) or U01 (Barudan)"
    ));
}

fn hoop_row(app: &mut VectorcraftApp, ui: &mut Ui) {
    use vectorcraft_embroidery::hoops;
    let current = app.session.active().and_then(|d| d.doc.embroidery.hoop.as_deref().cloned()).filter(|h| h.unknown.is_none());
    let name = |h: &vectorcraft_doc::Hoop| {
        let (w, hh) = hoops::size(h);
        let size = format!("{w:.0} × {hh:.0} mm");
        if h.id == "cap" { format!("{} {size}", tl!("Cap")) } else { size }
    };
    let common: Vec<vectorcraft_doc::Hoop> = hoops::BUILTIN.iter().filter_map(|id| hoops::builtin(id)).collect();
    let starter = current.as_ref().filter(|c| common.iter().any(|h| h == *c));
    let mut names: Vec<String> = vec![tl!("None").to_string()];
    names.extend(common.iter().map(name));
    let own = current.as_ref().filter(|_| starter.is_none()).map(|h| format!("{} ({})", h.name, name(h)));
    if let Some(n) = &own {
        names.push(n.clone());
    }
    // Each hoop of the library's hoop lists.
    let first_library = names.len();
    let libs = &app.session.embroidery_libraries.hoops;
    let library: Vec<(String, vectorcraft_doc::Hoop)> = libs
        .infos()
        .filter(|i| i.category == "user")
        .filter_map(|i| libs.get(&i.id))
        .flat_map(|(i, l)| l.hoops.iter().filter(|h| h.unknown.is_none()).map(|h| (i.id.clone(), h.clone())).collect::<Vec<_>>())
        .collect();
    // Hoop names already give the size ("130 × 180 mm (5 × 7 in)").
    names.extend(library.iter().map(|(_, h)| library_label(if h.name.is_empty() { &h.id } else { &h.name })));
    let shown = match (starter, &own) {
        (Some(h), _) => name(h),
        (None, Some(n)) => n.clone(),
        (None, None) => tl!("None").to_string(),
    };
    ui.scope(|ui| {
        widgets::label_row(ui, tl!("Hoop:"), LABEL_W, |ui| {
            let opts: Vec<&str> = names.iter().map(String::as_str).collect();
            if let Some(i) = names_dropdown(ui, "emb-hoop", &shown, &opts) {
                if let Some((lib, h)) = i.checked_sub(first_library).and_then(|j| library.get(j)) {
                    if let Err(e) = app.run("embroidery.loadHoop", json!({ "library": lib, "hoop": h.id })) {
                        app.status(e);
                    }
                } else {
                    let hoop = if i == 0 { Value::Null } else { common.get(i - 1).map_or(Value::Null, |h| json!(h.id)) };
                    if i == 0 || !hoop.is_null() {
                        app.run("embroidery.setHoop", json!({ "hoop": hoop })).ok();
                    }
                }
            }
        })
    })
    .response
    .on_hover_text(tl!("The hoop the design is sewn in: its sewing field, centred on the design, is checked and shown in Stitch Preview"));
    if let Some(h) = current.as_ref().filter(|_| starter.is_none()) {
        let libs = &app.session.embroidery_libraries.hoops;
        let listed = libs.infos().filter_map(|i| libs.get(&i.id)).any(|(_, l)| l.hoops.contains(h));
        indented(ui, |ui| {
            if add_button(app, ui, !listed) {
                add_to_library(app, "embroidery.saveHoop", json!({}), &format!("{}.{}", h.name, vectorcraft_embroidery::hoops::FILE_EXT), "hoops");
            }
        });
    }
    let rotation = app.session.active().map_or(0, |d| d.doc.embroidery.hoop_rotation());
    if current.is_some() || rotation != 0 {
        rotation_row(app, ui, rotation);
    }
    if current.is_some() {
        let r = app.session.execute("embroidery.hoops", &json!({})).ok();
        let fit = r.as_ref().and_then(|r| r["fit"].as_str().map(str::to_string));
        let split = r.as_ref().and_then(|r| r["hoopings"].as_u64());
        let t = Tokens::get(ui.ctx());
        let (text, color) = match fit.as_deref() {
            Some("fits") => (tl!("The design fits the hoop.").to_string(), t.text_dim),
            Some("turned") => (tl!("The design fits only turned a quarter turn.").to_string(), t.warning),
            Some("split") => (tl!("Split into {n} hoopings that each fit the hoop.").replace("{n}", &split.unwrap_or(0).to_string()), t.text_dim),
            Some("tooBig") => (tl!("The design is bigger than the hoop.").to_string(), t.error),
            _ => (String::new(), t.text_dim),
        };
        if !text.is_empty() {
            indented(ui, |ui| {
                ui.label(egui::RichText::new(text).size(11.5).color(color));
            });
        }
        hoopings_row(app, ui, fit.as_deref() == Some("tooBig"));
    }
}

/// Split a design too big for the hoop into hoopings, or sew it in one again; a split design's
/// alignment marks and basting box.
fn hoopings_row(app: &mut VectorcraftApp, ui: &mut Ui, too_big: bool) {
    let split = app.session.active().and_then(|d| d.doc.embroidery.hoopings.as_deref().cloned()).filter(|h| h.unknown.is_none());
    let run = |app: &mut VectorcraftApp, p: Value| {
        if let Err(e) = app.run("embroidery.splitHoops", p) {
            app.status(e);
        }
    };
    indented(ui, |ui| {
        // Too big: not split yet, or grown past its hoopings since.
        if too_big
            && ui
                .button(tl!("Split into Hoopings"))
                .on_hover_text(tl!("Split the design into areas that each fit the hoop, sewn one after another with the fabric hooped again"))
                .clicked()
        {
            run(app, json!({}));
        }
        if let Some(h) = &split {
            let (mut marks, mut basting) = (h.marks, h.basting);
            if ui
                .checkbox(&mut marks, tl!("Alignment Marks"))
                .on_hover_text(tl!("Running-stitch crosses on each seam, to line the next hooping up by"))
                .changed()
            {
                run(app, json!({ "marks": marks }));
            }
            if ui
                .checkbox(&mut basting, tl!("Basting Box"))
                .on_hover_text(tl!("A running-stitch box round each hooping, sewn first, to hold the fabric to the stabilizer"))
                .changed()
            {
                run(app, json!({ "basting": basting }));
            }
            if ui.button(tl!("Join Hoopings")).on_hover_text(tl!("Sew the design in one hooping again")).clicked() {
                run(app, json!({ "clear": true }));
            }
        }
    });
}

/// The turn the design is given in the hoop, as written to the machine.
fn rotation_row(app: &mut VectorcraftApp, ui: &mut Ui, rotation: u16) {
    const TURNS: [u16; 4] = [0, 90, 180, 270];
    let names: Vec<String> = TURNS.iter().map(|d| format!("{d}°")).collect();
    ui.scope(|ui| {
        widgets::label_row(ui, tl!("Rotate in Hoop:"), LABEL_W, |ui| {
            let opts: Vec<&str> = names.iter().map(String::as_str).collect();
            if let Some(d) = names_dropdown(ui, "emb-hoop-rotation", &format!("{rotation}°"), &opts).and_then(|i| TURNS.get(i)) {
                app.run("embroidery.setup", json!({ "hoopRotation": d })).ok();
            }
        })
    })
    .response
    .on_hover_text(tl!("Turns the design clockwise as it is written to the machine: a quarter turn fits a tall design in a wide hoop"));
}

/// Load a fabric profile file the user picks as the document's fabric (the web: the picked file
/// arrives later, through [`crate::io::open_bytes`]).
fn load_fabric(app: &mut VectorcraftApp) {
    if let Some(f) = app.services.open_async.as_mut() {
        f();
        return;
    }
    crate::picks::button(app, |app| {
        let ext: &'static [&'static str] = &[vectorcraft_embroidery::fabrics::FILE_EXT];
        let pick = crate::FilePick { filters: vec![crate::FilePick::filter("Fabric profiles", ext)], ..Default::default() };
        let Some(path) = crate::picks::open(app, &pick) else { return };
        if let Err(e) = app.run("embroidery.loadFabric", json!({ "path": path })) {
            app.status(e);
        }
    });
}

/// An underlay's name, translated.
fn underlay_label(u: Underlay) -> &'static str {
    match u {
        Underlay::None => tl!("None"),
        Underlay::Auto => tl!("Auto"),
        Underlay::Center => tl!("Center Walk"),
        Underlay::Edge => tl!("Edge Walk"),
        Underlay::Zigzag => tl!("Zigzag"),
        Underlay::Fill => tl!("Fill Underlay"),
    }
}

/// The satin or zigzag widths the selection allows: wider when its stitches are split.
fn width_range(sel: &Value) -> std::ops::RangeInclusive<f64> {
    if sel["split"].as_str() == Some(Split::Off.id()) { StitchSettings::WIDTH } else { StitchSettings::SPLIT_WIDTH }
}

/// Split Stitches, and with it on, where and how the stitches are split.
fn split_rows(app: &mut VectorcraftApp, ui: &mut Ui, sel: &Value) {
    let split = sel["split"].as_str().and_then(Split::parse);
    indented(ui, |ui| {
        let on = split.map(|s| s == Split::Auto);
        if widgets::check3_tip(
            ui,
            tl!("Split Wide Stitches"),
            on,
            true,
            tl!("Stitches wider than Split At are sewn in shorter pieces, so wide columns don't snag (up to 30 mm wide)"),
        ) {
            let to = if on == Some(true) { Split::Off } else { Split::Auto };
            set(app, json!({"split": to.id()}));
        }
    });
    if split == Some(Split::Off) {
        return;
    }
    let num = |k: &str| sel[k].as_f64();
    mm(app, ui, tl!("Split At:"), "splitAt", num("splitAt"), StitchSettings::SPLIT_AT, tl!("Stitches longer than this are split"));
    mm(app, ui, tl!("Split Length:"), "splitLength", num("splitLength"), StitchSettings::SPLIT_LENGTH, tl!("The longest piece of a split stitch"));
    ui.scope(|ui| {
        widgets::label_row(ui, tl!("Split Pattern:"), LABEL_W, |ui| {
            let labels = [tl!("Stagger"), tl!("Random")];
            let cur = match sel["splitPattern"].as_str().and_then(SplitPattern::parse) {
                Some(SplitPattern::Stagger) => labels[0],
                Some(SplitPattern::Random) => labels[1],
                None => "",
            };
            if let Some(p) = widgets::dropdown(ui, "emb-split-pattern", cur, &labels, WIDE_W).and_then(|i| SplitPattern::ALL.get(i)) {
                set(app, json!({"splitPattern": p.id()}));
            }
        })
    })
    .response
    .on_hover_text(tl!("How the split points move from stitch to stitch, so they don't line up into a seam"));
}

fn underlay_row(app: &mut VectorcraftApp, ui: &mut Ui, kind: StitchKind, sel: &Value) {
    let options = Underlay::for_kind(kind);
    widgets::label_row(ui, tl!("Underlay:"), LABEL_W, |ui| {
        let labels: Vec<&str> = options.iter().map(|u| underlay_label(*u)).collect();
        let cur = sel["underlay"].as_str().and_then(Underlay::parse).map_or("", underlay_label);
        if let Some(u) = widgets::dropdown(ui, "emb-underlay", cur, &labels, WIDE_W).and_then(|i| options.get(i)) {
            set(app, json!({"underlay": u.id()}));
        }
    });
}

/// The Setup tab: what the design is sewn on and with (fabric, its look in the preview, hoop,
/// machine format, thread chart), the sewing settings for the whole design, and the Sew-Out Test.
fn setup_tab(app: &mut VectorcraftApp, ui: &mut Ui, setup: &Value) {
    widgets::section_header(ui, tl!("Setup"));
    fabric_row(app, ui);
    look_row(app, ui);
    hoop_row(app, ui);
    machine_row(app, ui);
    chart_row(app, ui);
    widgets::divider(ui);
    widgets::section_header(ui, tl!("Sewing"));
    sewing_rows(app, ui, setup);
    ui.add_space(4.0);
    if widgets::flat_button(ui, tl!("Sew-Out Test…"), 104.0)
        .on_hover_text(tl!("Calibration swatches to sew, and a form for what they showed"))
        .clicked()
    {
        app.run("window.panel", json!({"panel": super::sew_out::ID})).ok();
    }
}

/// One of the design's sewing settings: its label, `embroidery.setup` key, unit, decimals, range
/// and tooltip.
type SewingRow = (&'static str, &'static str, &'static str, usize, std::ops::RangeInclusive<f64>, &'static str);

/// The design's sewing settings: machine speed (for the time estimate), how far fills reach under
/// what is sewn on top, and the shortest stitch kept.
fn sewing_rows(app: &mut VectorcraftApp, ui: &mut Ui, setup: &Value) {
    let rows: [SewingRow; 3] = [
        ("Machine Speed:", "speed", " /min", 0, EmbroiderySetup::SPEED, "Stitches per minute, for the sewing time estimate"),
        (
            "Overlap:",
            "overlap",
            " mm",
            1,
            EmbroiderySetup::OVERLAP,
            "How far a fill's rows still reach under objects sewn later on top of it; the rest of the covered rows is cut away",
        ),
        (
            "Shortest Stitch:",
            "minStitch",
            " mm",
            1,
            EmbroiderySetup::MIN_STITCH,
            "Needle points closer than this to the one before are left out (the fabric may ask for longer)",
        ),
    ];
    for (label, key, unit, decimals, r, tip) in rows {
        ui.scope(|ui| {
            widgets::label_row(ui, tl!(label), LABEL_W, |ui| {
                if let Some(v) = widgets::mixed_field(ui, ("emb", key), setup[key].as_f64(), unit, decimals, FIELD_W) {
                    app.run("embroidery.setup", json!({ key: v.clamp(*r.start(), *r.end()) })).ok();
                }
            })
        })
        .response
        .on_hover_text(tl!(tip));
    }
}

/// The Check & Export tab: what the design came to (its numbers and its threads in sewing order,
/// as the person at the machine reads them), the files and views made from it, and the sewing
/// order; the checker's findings follow.
fn check_tab(app: &mut VectorcraftApp, ui: &mut Ui, d: &Value) {
    widgets::section_header(ui, tl!("Design"));
    let n = |k: &str| d[k].as_u64().unwrap_or(0);
    let stitched = n("objects") > 0;
    if !stitched {
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
        for w in d["warnings"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            widgets::dim_label(ui, w);
        }
        widgets::divider(ui);
        widgets::section_header(ui, tl!("Threads"));
        thread_rows(app, ui, d);
    }
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        let r = ui
            .add_enabled_ui(stitched, |ui| widgets::flat_button(ui, tl!("Worksheet…"), 96.0))
            .inner
            .on_hover_text(tl!("A printable sheet for whoever sews the design: picture, size, threads, time and notes"));
        if r.clicked() {
            app.run("embroidery.worksheet", json!({})).ok();
        }
        let r = ui
            .add_enabled_ui(stitched, |ui| widgets::flat_button(ui, tl!("Mockup…"), 96.0))
            .inner
            .on_hover_text(tl!("The design sewn on a T-shirt, polo shirt, hoodie, cap, tote bag or towel, at its real size"));
        if r.clicked() {
            // As Object › Embroidery › Garment Mockup… opens it.
            let defaults = json!({"garment": "tshirt", "placement": "leftChest", "view": "garment"});
            let (id, p) = crate::menus::click_target("Garment Mockup…", "embroidery.mockup", &defaults);
            app.run(&id, p).ok();
        }
        if widgets::flat_button_fit(ui, tl!("Real Size"), 64.0)
            .on_hover_text(tl!("Show the design at its true size on this screen (View › Calibrate Real Size… makes it exact)"))
            .clicked()
        {
            app.run("view.realSize", json!({})).ok();
        }
        if widgets::flat_button(ui, tl!("Sew Order…"), 96.0)
            .on_hover_text(tl!("Step through the design as it is sewn, and change the order"))
            .clicked()
        {
            app.run("window.panel", json!({"panel": super::sew_order::ID})).ok();
        }
    });
}

/// One row per thread in sewing order: its colour, its number, its chart code and name when a chart
/// is chosen (else its hex), and how many stitches it sews.
fn thread_rows(app: &mut VectorcraftApp, ui: &mut Ui, d: &Value) {
    let matched = app.session.execute("embroidery.matchThreads", &json!({})).ok();
    let matched = matched.as_ref().and_then(|m| m["threads"].as_array());
    let t = Tokens::get(ui.ctx());
    for (i, thread) in d["threads"].as_array().into_iter().flatten().enumerate() {
        let hex = thread.as_str().unwrap_or("");
        let Some(c) = rgb(hex) else { continue };
        let m = matched.and_then(|m| m.get(i)).filter(|m| !m["code"].is_null());
        let name = match m {
            Some(m) => format!("{} {}", m["code"].as_str().unwrap_or(""), m["name"].as_str().unwrap_or("")),
            None => hex.to_string(),
        };
        let count = d["blockStitches"].get(i).and_then(Value::as_u64).unwrap_or(0);
        ui.horizontal(|ui| {
            let resp = chip(ui, c, 14.0);
            if let Some(m) = m {
                resp.on_hover_text(format!("{hex} (ΔE {})", m["deltaE"]));
            }
            ui.label(egui::RichText::new(format!("{}.", i + 1)).color(t.text_dim));
            // The count keeps its room at the right; a long name is cut short before it.
            let room = (ui.available_width() - 80.0).max(40.0);
            ui.allocate_ui_with_layout(vec2(room, 18.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(egui::Label::new(name).truncate());
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(crate::i18n::tn(count, "{n} stitch", "{n} stitches")).color(t.text_dim));
            });
        });
    }
}

/// "Name (Library)": an embroidery library entry as the pickers list it.
fn library_label(name: &str) -> String {
    format!("{name} ({})", tl!("Library"))
}

/// Whether the embroidery library folder already has `item`.
fn in_library<L: vectorcraft_engine::cmd::swatchlib::LibraryFile + PartialEq>(
    libs: &vectorcraft_engine::cmd::swatchlib::Libraries<L>,
    item: &L,
) -> bool {
    libs.infos().filter_map(|i| libs.get(&i.id)).any(|(_, l)| *l == *item)
}

/// The Add to Library button, shown when there is an embroidery library folder and `show`: clicked?
fn add_button(app: &VectorcraftApp, ui: &mut Ui, show: bool) -> bool {
    show && app.session.embroidery_libraries.dir().is_some()
        && widgets::flat_button(ui, tl!("Add to Library"), 104.0)
            .on_hover_text(tl!("Save a copy in your embroidery library, for any design"))
            .clicked()
}

/// Put what save command `cmd` writes with `p` into the embroidery library as file `file` of
/// `kind`, never replacing a file there (`embroidery.addToLibrary`).
fn add_to_library(app: &mut VectorcraftApp, cmd: &str, p: Value, file: &str, kind: &str) {
    let s = &mut app.session;
    let r = s.execute(cmd, &p).map_err(|e| e.to_string()).and_then(|r| {
        let data = r["data"].as_str().ok_or("nothing to add")?.to_string();
        s.execute("embroidery.addToLibrary", &json!({ "data": data, "name": file, "kind": kind })).map_err(|e| e.to_string())
    });
    match r {
        Ok(r) => app.status(format!("Added {} to the embroidery library", r["name"].as_str().unwrap_or_default())),
        Err(e) => app.status(e),
    }
}

/// Show the embroidery library folder in the file manager (made first, so there is one to show).
fn show_library_folder(app: &mut VectorcraftApp, dir: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Err(e) = std::fs::create_dir_all(dir) {
        app.status(e.to_string());
        return;
    }
    if let Err(e) = crate::io::reveal_path(app, dir) {
        app.status(e);
    }
}

#[cfg(test)]
mod tests {
    use vectorcraft_embroidery::params::{PARAMS, ValueKind, param};

    /// With an embroidery library folder, the Setup tab offers Add to Library for the
    /// document's own fabric until the library has it, and Show in Folder.
    #[test]
    fn the_design_section_adds_to_the_embroidery_library() {
        fn texts(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
                _ => {}
            }
        }
        fn shown(app: &mut VectorcraftApp) -> Vec<String> {
            let ctx = egui::Context::default();
            open_tools(&ctx);
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                setup_tab(app, ui, &Value::Null);
                tools(app, ui, &Value::Null);
            });
            out.textures_delta.clear();
            let mut shown = vec![];
            out.shapes.iter().for_each(|c| texts(&c.shape, &mut shown));
            shown
        }
        let dir = std::env::temp_dir().join(format!("vc-emb-panel-lib-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        assert!(!shown(&mut app).iter().any(|t| t == "Show in Folder"), "no folder, no button");
        app.session.embroidery_libraries.set_dir(Some(dir.to_string_lossy().to_string()));
        // Knit with its own pull: the document's own fabric, not the starter.
        app.session.execute("embroidery.setFabric", &json!({"fabric": "knit", "pull": 0.4})).unwrap();
        let before = shown(&mut app);
        assert!(before.iter().any(|t| t == "Add to Library") && before.iter().any(|t| t == "Show in Folder"), "{before:?}");
        add_to_library(&mut app, "embroidery.saveFabric", json!({}), "Mine.vcfabric", "fabrics");
        assert_eq!(app.session.embroidery_libraries.list(Some("fabrics")).len(), 1);
        assert!(!shown(&mut app).iter().any(|t| t == "Add to Library"), "the library has it now");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The generic rows draw every kind of setting: shown for every stitch type with none of its
    /// own rows, each setting's label appears (translated labels fall back to English here).
    #[test]
    fn generic_rows_draw_every_kind_of_setting() {
        fn texts(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
                _ => {}
            }
        }
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        app.session.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 50, "height": 50})).unwrap();
        for k in StitchKind::ALL {
            app.session.execute("embroidery.set", &json!({"type": k.id()})).unwrap();
            let sel = app.session.execute("embroidery.info", &json!({})).unwrap()["selection"].clone();
            let ctx = egui::Context::default();
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| other_settings(&mut app, ui, k, &sel, &[]));
            out.textures_delta.clear();
            let mut shown = vec![];
            out.shapes.iter().for_each(|c| texts(&c.shape, &mut shown));
            for p in PARAMS.iter().filter(|p| p.applies(k) && !ALWAYS_SHOWN.contains(&p.key)) {
                if matches!(p.value, ValueKind::Kind | ValueKind::Thread | ValueKind::Id) {
                    continue;
                }
                assert!(shown.iter().any(|t| t == p.label), "{}: no generic row for `{}` in {shown:?}", k.id(), p.key);
            }
        }
    }

    /// Satin, satin shape and zigzag show Split Wide Stitches; its settings only while it is on.
    #[test]
    fn split_rows_show_while_split_is_on() {
        fn texts(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
                _ => {}
            }
        }
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        app.session.execute("path.create", &json!({"d": "M 10 50 L 90 50"})).unwrap();
        let shown = |app: &mut VectorcraftApp, set: Value| {
            app.session.execute("embroidery.set", &set).unwrap();
            let sel = app.session.execute("embroidery.info", &json!({})).unwrap()["selection"].clone();
            let kind = sel["type"].as_str().and_then(StitchKind::parse).unwrap();
            let ctx = egui::Context::default();
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| parameters(app, ui, kind, &sel));
            out.textures_delta.clear();
            let mut shown = vec![];
            out.shapes.iter().for_each(|c| texts(&c.shape, &mut shown));
            shown
        };
        for kind in ["satin", "satinShape", "zigzag"] {
            let on = shown(&mut app, json!({"type": kind, "split": "auto"}));
            for label in ["Split Wide Stitches", "Split At:", "Split Length:", "Split Pattern:", "Stagger"] {
                assert!(on.iter().any(|t| t == label), "{kind}: no `{label}` in {on:?}");
            }
            let off = shown(&mut app, json!({"split": "off"}));
            assert!(off.iter().any(|t| t == "Split Wide Stitches"), "{kind}: {off:?}");
            assert!(!off.iter().any(|t| t == "Split At:"), "{kind}: {off:?}");
        }
    }

    #[test]
    fn the_styles_menu_lists_styles_and_saves_only_one_stitch() {
        fn texts(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
                _ => {}
            }
        }
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        app.session.execute("path.create", &json!({"d": "M 10 50 L 90 50"})).unwrap();
        app.session.execute("embroidery.set", &json!({"type": "satin", "width": 2})).unwrap();
        app.session.execute("embroidery.saveStyle", &json!({"name": "Mine"})).unwrap();
        let shown = |app: &mut VectorcraftApp, can_save: bool| {
            let ctx = egui::Context::default();
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| styles_list(app, ui, can_save));
            out.textures_delta.clear();
            let mut shown = vec![];
            out.shapes.iter().for_each(|c| texts(&c.shape, &mut shown));
            shown
        };
        let all = shown(&mut app, true);
        for label in ["Mine", "Small Lettering", "Bold Border", "Light Background Fill", "Save Style", "Load Style File…", "Remove"] {
            assert!(all.iter().any(|t| t == label), "no `{label}` in {all:?}");
        }
        let mixed = shown(&mut app, false);
        assert!(!mixed.iter().any(|t| t == "Save Style"), "{mixed:?}");
    }

    #[test]
    fn every_setting_a_stitch_uses_has_a_row() {
        for k in StitchKind::ALL {
            for key in own_rows(k) {
                assert!(param(key).is_some_and(|p| p.applies(k)), "{key}: not a setting {} uses", k.id());
            }
            for p in PARAMS.iter().filter(|p| p.applies(k)) {
                let own = own_rows(k).contains(&p.key) || ALWAYS_SHOWN.contains(&p.key);
                let generic = !matches!(p.value, ValueKind::Kind | ValueKind::Thread | ValueKind::Id);
                assert!(own || generic, "{} has no row for `{}`: give it one of its own", k.id(), p.key);
            }
        }
    }

    use super::*;

    /// The panel has an icon in the strip beside the dock (both the classic column and the
    /// docking rail draw `ICON_PANEL_GROUPS`), so it opens with one click.
    #[test]
    fn the_panel_has_an_icon_in_the_dock_strip() {
        assert!(crate::state::ICON_PANEL_GROUPS.iter().any(|g| g.contains(&ID)), "the Embroidery panel has no icon in the strip");
        let row = crate::state::ICON_PANELS.iter().find(|p| p.0 == ID).unwrap();
        assert_eq!(row.1, "Embroidery");
        assert!(crate::icons::exists(row.2), "{} is not a bundled icon", row.2);
    }

    /// Everything the panel draws fits the default dock column: no tile, field, picker or button
    /// row is cut off at its right edge, whatever is selected and whichever tab is open.
    #[test]
    fn nothing_is_cut_off_at_the_default_dock_width() {
        // The default dock column less the panel body's margins (`dock::panel_body`).
        let width = crate::panel_docking::COLUMN_WIDTH - 20.0;
        // The right edge of the right-most thing drawn with tab `tab` open.
        fn right_edge(app: &mut VectorcraftApp, tab: usize, width: f32) -> f32 {
            let ctx = egui::Context::default();
            crate::theme::install_fonts(&ctx);
            ctx.data_mut(|d| d.insert_persisted(tab_id(), tab));
            open_tools(&ctx);
            // A screen wider than the panel, so what runs past the panel isn't clipped away by the
            // screen's edge; the panel is laid out in `width`.
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(width + 400.0, 4000.0));
            let mut right = 0.0f32;
            for k in 0..3 {
                let input = egui::RawInput { time: Some(f64::from(k) * 0.1), screen_rect: Some(screen), ..Default::default() };
                let mut out = ctx.run_ui(input, |ui| {
                    ui.allocate_ui(vec2(width, 4000.0), |ui| {
                        ui.set_max_width(width);
                        show(app, ui);
                    });
                });
                out.textures_delta.clear();
                // What shows: a name clipped short (`painter_at`) is drawn whole but shown only
                // inside its clip.
                right = out
                    .shapes
                    .iter()
                    // An empty text shows nothing, whatever its box (egui gives one the screen's).
                    .filter(|c| !matches!(&c.shape, egui::Shape::Text(t) if t.galley.text().is_empty()))
                    .map(|c| c.shape.visual_bounding_rect().intersect(c.clip_rect))
                    .filter(|r| r.is_positive())
                    .map(|r| r.right())
                    .fold(0.0, f32::max);
            }
            right
        }
        let dir = std::env::temp_dir().join(format!("vc-emb-panel-width-{}", std::process::id()));
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.embroidery_libraries.set_dir(Some(dir.to_string_lossy().to_string()));
        let run = |app: &mut VectorcraftApp, id: &str, p: Value| app.session.execute(id, &p).unwrap();
        run(&mut app, "file.new", json!({"width": 600, "height": 600}));
        // Wider than a 4 × 4 in hoop, so the Setup tab offers to split it.
        run(&mut app, "shape.rectangle", json!({"x": 50, "y": 50, "width": 400, "height": 400}));
        let check = |app: &mut VectorcraftApp, tab: usize, what: &str| {
            let right = right_edge(app, tab, width);
            assert!(right <= width + 0.5, "{what}: drawn out to {right}, past the panel's {width}");
        };
        check(&mut app, 0, "nothing stitched");
        for k in StitchKind::ALL {
            run(&mut app, "embroidery.set", json!({"type": k.id()}));
            check(&mut app, 0, k.id());
        }
        // Rows that grow a button once a value is set: the thread, a satin's and a border's width.
        run(&mut app, "embroidery.set", json!({"type": "satin", "width": 3, "thread": "#cc2030"}));
        check(&mut app, 0, "satin with a width and a thread");
        run(&mut app, "embroidery.set", json!({"type": "patternBorder", "width": 8}));
        check(&mut app, 0, "pattern border with a width");
        // A thread chart (the thread's code and name under its colour) and the document's own
        // motif (Load…, Save… and Add to Library).
        run(&mut app, "embroidery.loadThreadChart", json!({"data": CHART}));
        run(&mut app, "embroidery.set", json!({"type": "fill", "thread": null}));
        check(&mut app, 0, "a thread named by the chart");
        load_own_motif(&mut app);
        run(&mut app, "embroidery.set", json!({"type": "motifFill", "motif": "mine"}));
        check(&mut app, 0, "the document's own motif");
        run(&mut app, "embroidery.set", json!({"type": "fill", "border": 2}));
        check(&mut app, SETUP_TAB, "setup");
        check(&mut app, CHECK_TAB, "check and export");
        // The document's own fabric (Save…, Add to Library) and a hoop the design is too big for.
        run(&mut app, "embroidery.setFabric", json!({"fabric": "knit", "pull": 0.4}));
        run(&mut app, "embroidery.setHoop", json!({"hoop": "4x4"}));
        check(&mut app, SETUP_TAB, "design too big for its hoop");
        run(&mut app, "embroidery.splitHoops", json!({}));
        check(&mut app, SETUP_TAB, "design split into hoopings");
        check(&mut app, CHECK_TAB, "threads of a split design");
        check(&mut app, TOOLS_TAB, "tools");
    }

    /// Open every section of the Tools tab in `ctx`, as if each had been clicked open.
    fn open_tools(ctx: &egui::Context) {
        for key in ["applique", "extract", "digitize", "lettering", "fonts", "library"] {
            ctx.data_mut(|d| d.insert_persisted(egui::Id::unique(("embroidery-tool-open", key.to_string())), true));
        }
    }

    /// A thread chart of one red thread with a long name.
    const CHART: &str = r##"{"format": "vectorcraft-thread-chart", "version": 1, "name": "Test Threads",
        "threads": [{"code": "1801", "name": "Very Long Thread Name For A Red", "color": "#e00000"}]}"##;

    /// A `.vcmotif` file of a motif the built-ins don't have: the stars under id `id`.
    fn motif_file(app: &mut VectorcraftApp, id: &str) -> String {
        let data = app.session.execute("embroidery.saveMotif", &json!({"motif": "stars"})).unwrap();
        let mut m: Value = serde_json::from_str(data["data"].as_str().unwrap()).unwrap();
        // The file holds the motif's own members beside `format` and `version`.
        m["id"] = json!(id);
        m["name"] = json!("My Own Motif With A Long Name");
        m.to_string()
    }

    /// Load a motif of the document's own, `mine`.
    fn load_own_motif(app: &mut VectorcraftApp) {
        let data = motif_file(app, "mine");
        load_motif_from(app, json!({"data": data}));
    }

    /// The texts the panel draws with tab `tab` open, and the colours of the rectangles it fills.
    fn drawn(app: &mut VectorcraftApp, tab: usize) -> (Vec<String>, Vec<Color32>) {
        fn walk(s: &egui::Shape, texts: &mut Vec<String>, fills: &mut Vec<Color32>) {
            match s {
                egui::Shape::Text(t) => texts.push(t.galley.text().to_string()),
                egui::Shape::Rect(r) => fills.push(r.fill),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, texts, fills)),
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        ctx.data_mut(|d| d.insert_persisted(tab_id(), tab));
        let (mut texts, mut fills) = (vec![], vec![]);
        for k in 0..2 {
            let input = egui::RawInput { time: Some(f64::from(k) * 0.1), ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| show(app, ui));
            out.textures_delta.clear();
            (texts, fills) = (vec![], vec![]);
            out.shapes.iter().for_each(|c| walk(&c.shape, &mut texts, &mut fills));
        }
        (texts, fills)
    }

    /// UI1.3: the first stitch in a document turns Stitch Preview on, so choosing a stitch shows
    /// on the canvas; once the user has turned it off it stays off.
    #[test]
    fn the_first_stitch_turns_stitch_preview_on_once() {
        let stitch_new_document = |app: &mut VectorcraftApp| {
            app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
            app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 100, "height": 60})).unwrap();
            app.run("embroidery.set", json!({"type": "fill"})).unwrap();
        };
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 100, "height": 60})).unwrap();
        assert!(!app.ui.view.stitch_preview, "off by default");
        app.run("embroidery.set", json!({"type": "fill"})).unwrap();
        assert!(app.ui.view.stitch_preview && app.ui.view.stitch_preview_chosen, "on with the first stitch");
        app.run("view.stitchPreview", json!({})).unwrap();
        assert!(!app.ui.view.stitch_preview);
        stitch_new_document(&mut app);
        assert!(!app.ui.view.stitch_preview, "turned off by the user, it stays off");
        // Turned on and off before anything is stitched: the user's choice too.
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("view.stitchPreview", json!({})).unwrap();
        app.run("view.stitchPreview", json!({})).unwrap();
        stitch_new_document(&mut app);
        assert!(!app.ui.view.stitch_preview);
        // A command that stitches nothing leaves it alone.
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 100, "height": 60})).unwrap();
        app.run("embroidery.info", json!({})).unwrap();
        assert!(!app.ui.view.stitch_preview && !app.ui.view.stitch_preview_chosen);
        // The toggles are in the header, above the Stitch tab's heading.
        let texts = drawn(&mut app, 0).0;
        let toggle = texts.iter().position(|t| t == "Stitch Preview").expect("the Stitch Preview toggle");
        // The last "Stitch" is the section's heading (the first, the tab's name).
        let heading = texts.iter().rposition(|t| t == "Stitch").expect("the Stitch heading");
        assert!(toggle < heading, "{texts:?}");
    }

    /// UI1.4: the Thread section shows the colour sewn, the object's own when it sets none, and
    /// the chart thread it matches.
    #[test]
    fn the_thread_row_shows_the_colour_sewn() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let run = |app: &mut VectorcraftApp, id: &str, p: Value| app.session.execute(id, &p).unwrap();
        run(&mut app, "file.new", json!({"width": 200, "height": 200}));
        run(&mut app, "paint.setFill", json!({"color": "#ff0000"}));
        run(&mut app, "shape.rectangle", json!({"x": 10, "y": 10, "width": 100, "height": 60}));
        run(&mut app, "embroidery.set", json!({"type": "fill"}));
        let (texts, fills) = drawn(&mut app, 0);
        assert!(fills.contains(&Color32::from_rgb(255, 0, 0)), "a chip of the fill's red");
        assert!(texts.iter().any(|t| t == "Object color") && texts.iter().any(|t| t == "Color:"), "{texts:?}");
        assert!(!texts.iter().any(|t| t == "Thread:"), "one Thread label, the heading");
        // A thread of its own, named by the chart.
        run(&mut app, "embroidery.loadThreadChart", json!({"data": CHART}));
        run(&mut app, "embroidery.set", json!({"thread": "#e01010"}));
        let (texts, fills) = drawn(&mut app, 0);
        assert!(fills.contains(&Color32::from_rgb(0xe0, 0x10, 0x10)));
        assert!(texts.iter().any(|t| t.starts_with("1801 Very Long")), "{texts:?}");
        assert!(texts.iter().any(|t| t == "Use Object Color"));
    }

    /// UI1.5: Motif Fill has Load…, Save… and Add to Library like border patterns, and a motif
    /// loaded from a file is sewn by the selected motif fill.
    #[test]
    fn motifs_load_save_and_go_to_the_library() {
        let dir = std::env::temp_dir().join(format!("vc-emb-panel-motif-{}", std::process::id()));
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.embroidery_libraries.set_dir(Some(dir.to_string_lossy().to_string()));
        let run = |app: &mut VectorcraftApp, id: &str, p: Value| app.session.execute(id, &p).unwrap();
        run(&mut app, "file.new", json!({"width": 200, "height": 200}));
        run(&mut app, "shape.rectangle", json!({"x": 10, "y": 10, "width": 100, "height": 60}));
        run(&mut app, "embroidery.set", json!({"type": "motifFill"}));
        let texts = drawn(&mut app, 0).0;
        assert!(texts.iter().any(|t| t == "Load…") && texts.iter().any(|t| t == "Save…"), "{texts:?}");
        assert!(!texts.iter().any(|t| t == "Add to Library"), "a built-in motif isn't added");
        load_own_motif(&mut app);
        assert_eq!(run(&mut app, "embroidery.info", json!({}))["selection"]["motif"], "mine", "the loaded motif is sewn");
        let texts = drawn(&mut app, 0).0;
        assert!(texts.iter().any(|t| t == "Add to Library"), "{texts:?}");
        add_to_library(&mut app, "embroidery.saveMotif", json!({"motif": "mine"}), "Mine.vcmotif", "motifs");
        assert!(!drawn(&mut app, 0).0.iter().any(|t| t == "Add to Library"), "once there, no button");
        // A motif file opened (File › Open, dropped, or picked on the web) loads the same way.
        let data = motif_file(&mut app, "dropped");
        crate::io::open_bytes(&mut app, "Dropped.vcmotif", data.as_bytes(), None).unwrap();
        assert_eq!(run(&mut app, "embroidery.info", json!({}))["selection"]["motif"], "dropped");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// UI2.1: whichever tab is open, the header shows the preview toggles, the design in one line,
    /// Export, and a badge for the checker's findings that opens the checker's list.
    #[test]
    fn the_header_shows_on_every_tab() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let run = |app: &mut VectorcraftApp, id: &str, p: Value| app.session.execute(id, &p).unwrap();
        run(&mut app, "file.new", json!({"width": 300, "height": 300}));
        // Nothing stitched: the toggles and a disabled Export, no summary, no badge.
        let texts = drawn(&mut app, 0).0;
        assert!(texts.iter().any(|t| t == "Stitch Preview") && texts.iter().any(|t| t.starts_with("Export ")), "{texts:?}");
        assert!(!texts.iter().any(|t| t.contains(" stitches · ")), "{texts:?}");
        // An unsplit 10 mm satin sews, but its stitches are long: the checker has something to say.
        run(&mut app, "path.create", json!({"d": "M 20 150 L 280 150"}));
        run(&mut app, "embroidery.set", json!({"type": "satin", "width": 10, "split": "off"}));
        let info = run(&mut app, "embroidery.info", json!({}));
        let findings = run(&mut app, "embroidery.check", json!({}))["findings"].as_array().map_or(0, Vec::len);
        let stitches = info["design"]["stitches"].as_u64().unwrap();
        for tab in 0..TABS.len() {
            let texts = drawn(&mut app, tab).0;
            assert!(texts.iter().any(|t| t == "Stitch Preview"), "tab {tab}: {texts:?}");
            assert!(
                texts.iter().any(|t| t.starts_with(&format!("{stitches} stitches · ")) && t.contains(" mm · 1 thread · ")),
                "tab {tab}: {texts:?}"
            );
            assert_eq!(texts.iter().filter(|t| t.starts_with("Export ")).count(), 1, "tab {tab}: one Export, in the header: {texts:?}");
            if findings > 0 {
                let badge = format!("⚠ {findings} problem{}", if findings == 1 { "" } else { "s" });
                assert!(texts.contains(&badge), "tab {tab}: {texts:?}");
            }
        }
        assert!(findings > 0, "the satin's long stitches are found");
        assert!(!drawn(&mut app, 0).0.iter().any(|t| t.contains("see Design")), "the Stitch tab's link is gone");
    }

    /// UI2.3: once something is stitched, the Stitch tab asks for the fabric and the hoop the
    /// document lacks, until both are chosen or the prompt is dismissed.
    #[test]
    fn the_stitch_tab_asks_for_a_fabric_and_a_hoop() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let run = |app: &mut VectorcraftApp, id: &str, p: Value| app.session.execute(id, &p).unwrap();
        let prompt = |app: &mut VectorcraftApp| {
            let texts = drawn(app, 0).0;
            let shows = texts.iter().any(|t| t == "Fabric and Hoop");
            (shows, shows && texts.iter().any(|t| t == "Fabric:"), shows && texts.iter().any(|t| t == "Hoop:"))
        };
        run(&mut app, "file.new", json!({"width": 300, "height": 300}));
        run(&mut app, "shape.rectangle", json!({"x": 10, "y": 10, "width": 100, "height": 60}));
        assert_eq!(prompt(&mut app), (false, false, false), "nothing stitched yet");
        run(&mut app, "embroidery.set", json!({"type": "fill"}));
        assert_eq!(prompt(&mut app), (true, true, true), "stitched, with neither");
        run(&mut app, "embroidery.setFabric", json!({"fabric": "knit"}));
        assert_eq!(prompt(&mut app), (true, false, true), "only the hoop is left to choose");
        run(&mut app, "embroidery.setHoop", json!({"hoop": "5x7"}));
        assert_eq!(prompt(&mut app), (false, false, false), "both chosen");
        // Another document without them asks again, until dismissed.
        run(&mut app, "file.new", json!({"width": 300, "height": 300}));
        run(&mut app, "shape.rectangle", json!({"x": 10, "y": 10, "width": 100, "height": 60}));
        run(&mut app, "embroidery.set", json!({"type": "fill"}));
        assert!(prompt(&mut app).0);
        app.ui.embroidery_setup_prompt_off = true;
        assert!(!prompt(&mut app).0, "dismissed for good");
    }

    /// UI2.5: Object › Embroidery opens the Embroidery and Sew Order panels and exports in the
    /// document's machine format; the Embroidery workspace opens the panel with Stitch Preview on.
    #[test]
    fn the_menu_and_the_workspace_reach_the_embroidery_panels() {
        use crate::menus::Item;
        fn find<'a>(items: &'a [Item], label: &str) -> Option<&'a Item> {
            items.iter().find_map(|i| match i {
                Item::Sub(l, sub) if *l == "Embroidery" => sub.iter().find(|s| matches!(s, Item::Cmd(l, ..) if *l == label)),
                Item::Sub(_, sub) => find(sub, label),
                _ => None,
            })
        }
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        let tree: Vec<Item> = crate::menus::menu_tree().into_iter().map(|(name, items)| Item::Sub(name, items)).collect();
        for (label, id, panel) in [("Embroidery Panel", "window.panel", Some(ID)), ("Sew Order", "window.panel", Some(super::super::sew_order::ID))] {
            let Some(Item::Cmd(_, cmd, p)) = find(&tree, label) else { panic!("Object › Embroidery › {label}") };
            assert_eq!((*cmd, p["panel"].as_str()), (id, panel));
        }
        let Some(Item::Cmd(_, cmd, p)) = find(&tree, "Export Stitch File…") else { panic!("Export Stitch File…") };
        assert_eq!((*cmd, p.clone()), ("file.exportAs", json!({"format": "machine"})));
        // `machine` is the document's machine format: Export As opens on it.
        app.run("embroidery.setup", json!({"machineFormat": "pes"})).unwrap();
        app.run("file.exportAs", json!({"format": "machine"})).unwrap();
        let dialog = app.ui.dialog.as_ref().expect("Export As");
        assert_eq!((dialog.kind.as_str(), dialog.fields.get("format").and_then(Value::as_str)), ("exportAs", Some("pes")));
        app.ui.dialog = None;
        // The workspace.
        app.ui.view.stitch_preview = false;
        app.run("window.workspace", json!({"name": "Embroidery"})).unwrap();
        assert_eq!(app.ui.open_panel.as_deref(), Some(ID));
        assert!(app.ui.view.stitch_preview && app.ui.view.rulers);
    }

    /// UI2.4: the Tools tab is a short list of closed sections, each opening its tool; the Font
    /// Editor is a panel of its own; the tools' commands are in Object › Embroidery.
    #[test]
    fn the_tools_are_sections_and_the_font_editor_a_panel() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        let closed = drawn(&mut app, TOOLS_TAB).0;
        for name in ["Appliqué", "Auto-Digitize", "Lettering", "Embroidery Fonts"] {
            assert!(closed.iter().any(|t| t == name), "the {name} section: {closed:?}");
        }
        assert!(!closed.iter().any(|t| t == "Cut File…" || t == "Load Embroidery Font…"), "closed at first: {closed:?}");
        assert!(!closed.iter().any(|t| t == "Font Editor"), "the Font Editor isn't in the tab: {closed:?}");
        // Opened, a section shows its tool.
        let ctx = egui::Context::default();
        ctx.data_mut(|d| d.insert_persisted(tab_id(), TOOLS_TAB));
        ctx.data_mut(|d| d.insert_persisted(egui::Id::unique(("embroidery-tool-open", "fonts".to_string())), true));
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| show(&mut app, ui));
        out.textures_delta.clear();
        let mut texts = vec![];
        fn walk(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        out.shapes.iter().for_each(|c| walk(&c.shape, &mut texts));
        assert!(texts.iter().any(|t| t == "Load Embroidery Font…") && texts.iter().any(|t| t == "Font Editor…"), "{texts:?}");
        assert!(!texts.iter().any(|t| t == "Cut File…"), "the others stay closed: {texts:?}");
        // The Font Editor panel.
        let mut out = egui::Context::default().run_ui(egui::RawInput::default(), |ui| show_font_editor(&mut app, ui));
        out.textures_delta.clear();
        let mut editor = vec![];
        out.shapes.iter().for_each(|c| walk(&c.shape, &mut editor));
        assert!(editor.iter().any(|t| t == "New Embroidery Font"), "{editor:?}");
        app.run("window.panel", json!({"panel": FONT_EDITOR_ID})).unwrap();
        // Object › Embroidery.
        let tree = crate::menus::menu_tree();
        let object = tree.iter().find(|(m, _)| *m == "Object").map(|(_, items)| items).unwrap();
        let Some(crate::menus::Item::Sub(_, emb)) = object.iter().find(|i| matches!(i, crate::menus::Item::Sub("Embroidery", _))) else {
            panic!("Object › Embroidery")
        };
        let ids: Vec<&str> = emb.iter().filter_map(|i| if let crate::menus::Item::Cmd(_, id, _) = i { Some(*id) } else { None }).collect();
        for id in ["embroidery.applique", "embroidery.cutFile", "embroidery.textArc", "embroidery.monogram"] {
            assert!(ids.contains(&id), "{id}: {ids:?}");
        }
    }

    /// UI2.2: the tabs are by task. Setup holds what the design is sewn on and with, and the sewing
    /// settings Sew Order used to; Check & Export lists the threads as rows with their stitches.
    #[test]
    fn the_tabs_are_by_task() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let run = |app: &mut VectorcraftApp, id: &str, p: Value| app.session.execute(id, &p).unwrap();
        run(&mut app, "file.new", json!({"width": 300, "height": 300}));
        run(&mut app, "paint.setFill", json!({"color": "#e00000"}));
        run(&mut app, "shape.rectangle", json!({"x": 10, "y": 10, "width": 100, "height": 60}));
        run(&mut app, "embroidery.set", json!({"type": "fill"}));
        let texts = drawn(&mut app, 0).0;
        for name in ["Stitch", "Setup", "Check & Export", "Tools"] {
            assert!(texts.iter().any(|t| t == name), "the {name} tab: {texts:?}");
        }
        let setup = drawn(&mut app, SETUP_TAB).0;
        for label in ["Fabric:", "Hoop:", "Machine Format:", "Thread Chart:", "Machine Speed:", "Overlap:", "Shortest Stitch:", "Sew-Out Test…"] {
            assert!(setup.iter().any(|t| t == label), "Setup has {label}: {setup:?}");
        }
        assert!(!setup.iter().any(|t| t == "Stitches:"), "the numbers are in Check & Export: {setup:?}");
        // One thread, named by its hex until a chart is chosen, with its stitches.
        let stitches = run(&mut app, "embroidery.info", json!({}))["design"]["stitches"].as_u64().unwrap();
        let check = drawn(&mut app, CHECK_TAB).0;
        assert!(check.iter().any(|t| t == "#e00000") && check.contains(&format!("{stitches} stitches")), "{check:?}");
        assert!(check.iter().any(|t| t == "Worksheet…") && !check.iter().any(|t| t == "Machine Speed:"), "{check:?}");
        run(&mut app, "embroidery.loadThreadChart", json!({"data": CHART}));
        let check = drawn(&mut app, CHECK_TAB).0;
        assert!(check.iter().any(|t| t.starts_with("1801 Very Long")), "the chart's thread names the row: {check:?}");
        // Sew Order keeps only what is about order and travel.
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| super::super::sew_order::show(&mut app, ui));
        out.textures_delta.clear();
        let mut order = vec![];
        fn walk(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        out.shapes.iter().for_each(|c| walk(&c.shape, &mut order));
        assert!(order.iter().any(|t| t == "Trim Jumps Over:") && !order.iter().any(|t| t == "Machine Speed:"), "{order:?}");
    }

    #[test]
    fn hex_threads_parse() {
        assert_eq!(rgb("#0a0B0c"), Some([10, 11, 12]));
        assert_eq!(rgb("0a0b0c"), Some([10, 11, 12]));
        assert_eq!(rgb("#0a0b"), None);
        assert_eq!(rgb("#zz0000"), None);
    }
}
