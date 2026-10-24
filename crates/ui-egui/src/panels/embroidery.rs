//! Embroidery panel (Window › Embroidery): the selected objects' stitch, chosen from a grid of
//! stitched pictures, their thread and trims, and the stitch's own settings (fill patterns and
//! border patterns picked from pictures too); then the design's numbers with Stitch Preview and
//! Export in the machine's format (DST, PES, JEF, EXP), and the design checker's findings. The sewing order has a panel of its own
//! (`sew_order`). Reads `embroidery.info`, acts through `embroidery.set` and `embroidery.clear`.

use egui::{Color32, Rect, Sense, Stroke, Ui, vec2};
use serde_json::{Value, json};
use vectorcraft_doc::{NodeId, StitchKind, StitchSettings, Underlay};

use super::{selection_len, stitch_art};
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

pub const ID: &str = "embroidery";
const LABEL_W: f32 = 128.0;
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

/// The panel's tabs: the selection's stitches, the design as a whole, and the tools.
const TABS: [&str; 3] = ["Stitch", "Design", "Tools"];

/// Which tab is open (kept between sessions).
fn tab_id() -> egui::Id {
    egui::Id::new("embroidery-panel-tab")
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let info = info(app, ui.ctx());
    // One width for every tab, so the panel doesn't jump as they change.
    ui.set_min_width(300.0);
    let mut tab = ui.ctx().data_mut(|d| d.get_persisted::<usize>(tab_id())).unwrap_or(0).min(TABS.len() - 1);
    let names = [tl!("Stitch"), tl!("Design"), tl!("Tools")];
    if let Some(i) = widgets::tab_bar(ui, &names, tab) {
        tab = i;
        ui.ctx().data_mut(|d| d.insert_persisted(tab_id(), i));
    }
    ui.add_space(4.0);
    match tab {
        1 => {
            design(app, ui, &info["design"]);
            widgets::divider(ui);
            checks(app, ui, &info["check"]);
        }
        2 => tools(app, ui, &info["selection"]),
        _ => stitch_tab(app, ui, &info),
    }
}

/// The Stitch tab: the selected objects' stitch, thread, trims and settings.
fn stitch_tab(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    let sel = &info["selection"];
    let stitched = sel["stitched"].as_u64().unwrap_or(0);
    let unstitched = sel["unstitched"].as_u64().unwrap_or(0);
    let any = stitched + unstitched > 0;
    let kind = sel["type"].as_str().and_then(StitchKind::parse).filter(|_| unstitched == 0);

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
        if stitched > 0 && widgets::flat_button(ui, tl!("Remove Stitches"), 110.0).on_hover_text(tl!("The selection is no longer stitched")).clicked()
        {
            app.run("embroidery.clear", json!({})).ok();
        }
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
    // What the checker found, and where to read it.
    let findings = info["check"]["findings"].as_array().map_or(0, Vec::len);
    if findings > 0 {
        widgets::divider(ui);
        let t = Tokens::get(ui.ctx());
        let text = tl!("{n} problems found: see Design").replace("{n}", &findings.to_string());
        if ui.add(egui::Button::new(egui::RichText::new(text).color(t.warning)).frame(false)).clicked() {
            ui.ctx().data_mut(|d| d.insert_persisted(tab_id(), 1usize));
        }
    }
}

/// The Tools tab: appliqué, border extraction, fonts and the library folder.
fn tools(app: &mut VectorcraftApp, ui: &mut Ui, sel: &Value) {
    widgets::section_header(ui, tl!("Appliqué"));
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
    // A border pattern from an opened design's stitches (manual stitches).
    if sel["type"] == json!(StitchKind::Manual.id()) {
        extract_row(app, ui);
    }
    widgets::divider(ui);
    widgets::section_header(ui, tl!("Embroidery Fonts"));
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
    if let Some(dir) = app.session.embroidery_libraries.dir().map(str::to_string) {
        widgets::divider(ui);
        widgets::section_header(ui, tl!("Library"));
        if widgets::flat_button(ui, tl!("Show in Folder"), 104.0)
            .on_hover_text(tl!("Your embroidery library: border patterns, fabrics, hoops and thread charts for any design"))
            .clicked()
        {
            show_library_folder(app, &dir);
        }
    }
}

/// Load a `.vcfont` embroidery font the user picks into the document.
fn load_font(app: &mut VectorcraftApp) {
    if let Some(f) = app.services.open_async.as_mut() {
        f();
        return;
    }
    crate::picks::button(app, |app| {
        let ext: &'static [&'static str] = &[vectorcraft_embroidery::fonts::FILE_EXT];
        let pick = crate::FilePick { filters: vec![("Embroidery fonts", ext)], ..Default::default() };
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
    let (rect, resp) = ui.allocate_exact_size(vec2(170.0, 36.0), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    ui.painter().rect_filled(rect, 3.0, t.input);
    ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    let pic = Rect::from_min_size(rect.min, vec2(56.0, rect.height()));
    if let Some(th) = thumb {
        stitch_art::draw(ui.painter(), pic, th, t.text, dots.then_some(t.text_dim));
    }
    ui.painter().text(pic.right_center() + vec2(6.0, 0.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.0), t.text);
    let arrow = Rect::from_center_size(rect.right_center() - vec2(10.0, 0.0), vec2(8.0, 5.0));
    ui.painter().add(egui::Shape::convex_polygon(vec![arrow.left_top(), arrow.right_top(), arrow.center_bottom()], t.text_dim, Stroke::NONE));
    let cols = 3;
    let size = vec2(96.0, 72.0);
    egui::Popup::menu(&resp)
        .id(egui::Id::new(("picture-picker", id)))
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
    }
    if findings.len() > MAX_CHECK_ROWS {
        widgets::dim_label(ui, tl!("Only the first 30 are listed."));
    }
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
            let Some(i) = widgets::dropdown_names(ui, "emb-font", &shown, &opts, 150.0) else { return };
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
    ui.horizontal(|ui| {
        ui.add_space(LABEL_W + 8.0);
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
            let rails = sel["rails"].as_bool();
            ui.horizontal(|ui| {
                ui.add_space(LABEL_W + 8.0);
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
        }
        StitchKind::Zigzag => {
            widgets::section_header(ui, tl!("Zigzag Stitch"));
            // Unset, a zigzag is 3 mm wide (it doesn't follow the stroke, drawn thin).
            let width = Some(num("width").unwrap_or(StitchSettings::DEFAULT_WIDTH));
            mm(app, ui, tl!("Width:"), "width", width, StitchSettings::WIDTH, tl!("How wide the zigzag swings"));
            mm(app, ui, tl!("Spacing:"), "spacing", num("spacing"), StitchSettings::SPACING, tl!("Distance between the points of the zigzag"));
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
            ui.horizontal(|ui| {
                ui.add_space(LABEL_W + 8.0);
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
                widgets::label_row(ui, tl!("Width:"), LABEL_W, |ui| {
                    let width = sel["width"].as_f64();
                    if let Some(v) = widgets::mixed_field(ui, ("emb", "patternWidth"), width, " mm", 1, FIELD_W) {
                        let r = StitchSettings::PATTERN_WIDTH;
                        set(app, json!({"width": v.clamp(*r.start(), *r.end())}));
                    }
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
            ui.horizontal(|ui| {
                ui.add_space(LABEL_W + 8.0);
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
        StitchKind::Running => &["length"],
        StitchKind::Bean => &["length", "passes"],
        StitchKind::Satin => &["rails", "width", "spacing", "pull", "underlay"],
        StitchKind::Zigzag => &["width", "spacing"],
        StitchKind::SatinShape => &["spacing", "pull", "underlay"],
        StitchKind::Fill => &["angle", "spacing", "length", "stagger", "motif", "motifSize", "underlay", "border"],
        StitchKind::PatternBorder => &["pattern", "width", "flip", "length", "spacing"],
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
                ui.horizontal(|ui| {
                    ui.add_space(LABEL_W + 8.0);
                    if widgets::check3_tip(ui, label, v, true, help) {
                        set(app, json!({ key: v != Some(true) }));
                    }
                });
            }
            ValueKind::Choice(ids) => {
                let cur = sel[key].as_str().unwrap_or_default();
                widgets::label_row(ui, label, LABEL_W, |ui| {
                    if let Some(i) = widgets::dropdown_names(ui, ("emb-generic", key), cur, ids, 150.0)
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
        _ => tl!("Motif"),
    }
}

/// Extract Border Pattern: the repeat of the selected border stitches, added to the document's
/// border patterns.
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
        let pick = crate::FilePick { filters: vec![("Border patterns", ext)], ..Default::default() };
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
            if let Some(i) = widgets::dropdown_names(ui, "emb-fabric", &shown, &opts, 150.0) {
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
    ui.horizontal(|ui| {
        ui.add_space(LABEL_W + 8.0);
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
            if let Some(i) = widgets::dropdown_names(ui, "emb-chart", &shown, &opts, 150.0) {
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
    ui.horizontal(|ui| {
        ui.add_space(LABEL_W + 8.0);
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
        let pick = crate::FilePick { filters: vec![("Thread charts", ext)], ..Default::default() };
        let Some(path) = crate::picks::open(app, &pick) else { return };
        if let Err(e) = app.run("embroidery.loadThreadChart", json!({ "path": path })) {
            app.status(e);
        }
    });
}

/// The document's hoop: a common one, none, or its own (loaded); whether the design fits it.
/// The machine file format the document exports in (DST, PES, JEF, EXP).
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
            if let Some(f) = widgets::dropdown_names(ui, "emb-machine-format", &current, &opts, 150.0).and_then(|i| formats.get(i)) {
                app.run("embroidery.setup", json!({ "machineFormat": f })).ok();
            }
        })
    })
    .response
    .on_hover_text(tl!("The file format your machine reads: DST (Tajima and most machines), PES (Brother), JEF (Janome) or EXP (Melco)"));
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
            if let Some(i) = widgets::dropdown_names(ui, "emb-hoop", &shown, &opts, 150.0) {
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
        ui.horizontal(|ui| {
            ui.add_space(LABEL_W + 8.0);
            if add_button(app, ui, !listed) {
                add_to_library(app, "embroidery.saveHoop", json!({}), &format!("{}.{}", h.name, vectorcraft_embroidery::hoops::FILE_EXT), "hoops");
            }
        });
    }
    if current.is_some() {
        let fit = app.session.execute("embroidery.hoops", &json!({})).ok().and_then(|r| r["fit"].as_str().map(str::to_string));
        let t = Tokens::get(ui.ctx());
        let (text, color) = match fit.as_deref() {
            Some("fits") => (tl!("The design fits the hoop."), t.text_dim),
            Some("turned") => (tl!("The design fits only turned a quarter turn."), t.warning),
            Some("tooBig") => (tl!("The design is bigger than the hoop."), t.error),
            _ => ("", t.text_dim),
        };
        if !text.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(LABEL_W + 8.0);
                ui.label(egui::RichText::new(text).size(11.5).color(color));
            });
        }
    }
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
        let pick = crate::FilePick { filters: vec![("Fabric profiles", ext)], ..Default::default() };
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
    fabric_row(app, ui);
    hoop_row(app, ui);
    machine_row(app, ui);
    chart_row(app, ui);
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
        // Each thread's nearest in the chosen chart, when one is.
        let matched = app.session.execute("embroidery.matchThreads", &json!({})).ok();
        let matched = matched.as_ref().and_then(|m| m["threads"].as_array());
        widgets::label_row(ui, tl!("Threads:"), LABEL_W, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (i, t) in d["threads"].as_array().into_iter().flatten().enumerate() {
                    if let Some(c) = t.as_str().and_then(rgb) {
                        let mut tip = format!("{} {}", i + 1, t.as_str().unwrap_or(""));
                        if let Some(m) = matched.and_then(|m| m.get(i)).filter(|m| !m["code"].is_null()) {
                            tip.push_str(&format!(
                                "\n{} {} (ΔE {})",
                                m["code"].as_str().unwrap_or(""),
                                m["name"].as_str().unwrap_or(""),
                                m["deltaE"]
                            ));
                        }
                        chip(ui, c, 14.0).on_hover_text(tip);
                    }
                }
            });
        });
        if let Some(m) = matched {
            let codes: Vec<&str> = m.iter().filter_map(|t| t["code"].as_str()).collect();
            if !codes.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(LABEL_W + 8.0);
                    ui.label(egui::RichText::new(codes.join(" · ")).size(11.5).color(Tokens::get(ui.ctx()).text_dim));
                });
            }
        }
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
        let label = export_label(app);
        let r = ui.add_enabled_ui(n("objects") > 0, |ui| widgets::flat_button(ui, &label, 96.0)).inner;
        if r.clicked() {
            let format = machine_format(app);
            app.run("file.exportAs", json!({ "format": format })).ok();
        }
    });
    ui.add_space(2.0);
    let r = ui
        .add_enabled_ui(n("objects") > 0, |ui| widgets::flat_button(ui, tl!("Worksheet…"), 96.0))
        .inner
        .on_hover_text(tl!("A printable sheet for whoever sews the design: picture, size, threads, time and notes"));
    if r.clicked() {
        app.run("embroidery.worksheet", json!({})).ok();
    }
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        if widgets::flat_button(ui, tl!("Sew Order…"), 96.0)
            .on_hover_text(tl!("Step through the design as it is sewn, and change the order"))
            .clicked()
        {
            app.run("window.panel", json!({"panel": super::sew_order::ID})).ok();
        }
        if widgets::flat_button(ui, tl!("Sew-Out Test…"), 104.0)
            .on_hover_text(tl!("Calibration swatches to sew, and a form for what they showed"))
            .clicked()
        {
            app.run("window.panel", json!({"panel": super::sew_out::ID})).ok();
        }
    });
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

    /// With an embroidery library folder, the Design section offers Add to Library for the
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
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                design(app, ui, &Value::Null);
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

    #[test]
    fn hex_threads_parse() {
        assert_eq!(rgb("#0a0B0c"), Some([10, 11, 12]));
        assert_eq!(rgb("0a0b0c"), Some([10, 11, 12]));
        assert_eq!(rgb("#0a0b"), None);
        assert_eq!(rgb("#zz0000"), None);
    }
}
