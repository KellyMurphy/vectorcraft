//! Trace Font with progress: tracing an installed font into an embroidery font takes a while (each
//! letter's pen strokes and satin checks), so the app traces it a few letters a frame, showing a
//! progress window (letters done, a bar, Cancel) and the busy cursor, then runs
//! `embroidery.font.trace`, which finds every letter traced already and opens the font at once.
//! Agents and scripts run the command directly; it does the same work in one go.

use serde_json::Value;
use vectorcraft_engine::cmd::embroidery_font_editor::{trace_chars, trace_letter};

use crate::VectorcraftApp;
use crate::theme::Tokens;

/// How long a frame spends tracing, seconds: the window stays responsive.
const FRAME_BUDGET: f64 = 0.03;

/// A font being traced.
pub struct FontTrace {
    family: String,
    chars: Vec<String>,
    /// How many of `chars` are traced.
    done: usize,
    /// `embroidery.font.trace`'s params, run when all are traced.
    params: Value,
}

impl FontTrace {
    /// How far it has got: letters done and the whole.
    pub fn progress(&self) -> (usize, usize) {
        (self.done, self.chars.len())
    }
}

/// Start tracing with `embroidery.font.trace`'s `params` (its `from` the family).
pub fn start(app: &mut VectorcraftApp, params: Value) {
    let family = params.get("from").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    let chars = trace_chars(&params);
    app.font_trace = Some(FontTrace { family, chars, done: 0, params });
}

/// Each frame: trace the next letters, show the progress, and open the font when all are done.
pub fn poll(app: &mut VectorcraftApp, ctx: &egui::Context) {
    let Some(job) = app.font_trace.as_mut() else { return };
    let started = now();
    while job.done < job.chars.len() && now() - started < FRAME_BUDGET {
        if let Some(c) = job.chars.get(job.done) {
            trace_letter(&job.family, c);
        }
        job.done += 1;
    }
    if job.done >= job.chars.len() {
        let Some(job) = app.font_trace.take() else { return };
        match app.run("embroidery.font.trace", job.params) {
            Ok(r) => {
                let (traced, kept) = (r["traced"].as_u64().unwrap_or(0), r["outlines"].as_u64().unwrap_or(0));
                app.status(crate::i18n::fmt(
                    tl!("Traced {family}: {traced} letters as satin columns, {kept} kept as outlines"),
                    &[("family", &job.family), ("traced", &traced.to_string()), ("kept", &kept.to_string())],
                ));
            }
            Err(e) => app.status(e),
        }
        return;
    }
    let (done, total) = job.progress();
    let family = job.family.clone();
    ctx.set_cursor_icon(egui::CursorIcon::Progress);
    ctx.request_repaint();
    let cancel = crate::dialogs::modal::show(ctx, tl!("Tracing Font"), egui::Id::unique("font-trace-progress"), 0.0, 16, |ui| {
        ui.set_width(320.0);
        crate::dialogs::modal::heading(ui, &crate::i18n::fmt(tl!("Tracing {family}"), &[("family", &family)]));
        ui.add_space(6.0);
        let t = Tokens::get(ui.ctx());
        let part = if total == 0 { 1.0 } else { done as f32 / total as f32 };
        ui.add(egui::ProgressBar::new(part).fill(t.accent).show_percentage());
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(crate::i18n::fmt(tl!("{done} of {total} letters"), &[("done", &done.to_string()), ("total", &total.to_string())]))
                .color(t.text_dim),
        );
        ui.add_space(8.0);
        // Cancel at the right, in a row only as tall as the button.
        ui.horizontal(|ui| {
            ui.add_space((ui.available_width() - 80.0).max(0.0));
            crate::widgets::flat_button(ui, tl!("Cancel"), 80.0).clicked()
        })
        .inner
    });
    // Cancel (or Escape): nothing opens; the letters traced so far stay traced for next time.
    if cancel == Some(true) || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.font_trace = None;
    }
}

/// Seconds, for timing a frame's work.
fn now() -> f64 {
    crate::now_ms() / 1000.0
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_engine::Session;

    use super::*;

    /// An installed font to trace (none: the tests skip).
    fn family() -> Option<&'static str> {
        ["Arial", "Liberation Sans", "DejaVu Sans"].into_iter().find(|f| vectorcraft_text::FontDb::global().has_family(f))
    }

    /// A context drawing as the app does (its fonts and theme).
    fn context() -> egui::Context {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, Default::default());
        ctx
    }

    fn frame(app: &mut VectorcraftApp, ctx: &egui::Context, input: egui::RawInput) -> egui::FullOutput {
        let mut out = ctx.run_ui(input, |ui| poll(app, &ui.ctx().clone()));
        out.textures_delta.clear();
        out
    }

    #[test]
    fn tracing_shows_progress_with_a_busy_cursor_then_opens_the_font() {
        let Some(family) = family() else { return };
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        let ctx = context();
        let docs = app.session.documents().len();
        // Letters no other test traced, so the first frame can't do them all from the cache.
        start(&mut app, json!({ "from": family, "chars": "a-zA-Z0-9", "name": "Progress Test" }));
        let out = frame(&mut app, &ctx, Default::default());
        let (done, total) = app.font_trace.as_ref().map(FontTrace::progress).unwrap();
        assert!(done < total && total == 62, "{done}/{total}");
        assert_eq!(out.platform_output.cursor_icon, egui::CursorIcon::Progress);
        // On to the end: the font opens in a new tab.
        for _ in 0..10_000 {
            if app.font_trace.is_none() {
                break;
            }
            frame(&mut app, &ctx, Default::default());
        }
        assert!(app.font_trace.is_none(), "finished");
        assert_eq!(app.session.documents().len(), docs + 1);
        assert!(app.session.active().is_some_and(|d| d.doc.embroidery.font_editor.is_some()));
    }

    #[test]
    fn escape_cancels_and_opens_nothing() {
        let Some(family) = family() else { return };
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        let ctx = context();
        let docs = app.session.documents().len();
        start(&mut app, json!({ "from": family, "chars": "a-z", "name": "Cancel Test" }));
        frame(&mut app, &ctx, Default::default());
        let esc = egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
        frame(&mut app, &ctx, egui::RawInput { events: vec![esc], ..Default::default() });
        assert!(app.font_trace.is_none());
        assert_eq!(app.session.documents().len(), docs);
    }
}

#[cfg(test)]
mod cancel_tests {
    use serde_json::json;
    use vectorcraft_engine::Session;

    use super::*;

    #[test]
    fn the_cancel_button_cancels() {
        let Some(family) = ["Arial", "Liberation Sans", "DejaVu Sans"].into_iter().find(|f| vectorcraft_text::FontDb::global().has_family(f)) else {
            return;
        };
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, Default::default());
        let screen = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0)));
        let docs = app.session.documents().len();
        start(&mut app, json!({ "from": family, "chars": "a-zA-Z", "name": "Cancel Button Test" }));
        let run = |app: &mut VectorcraftApp, events: Vec<egui::Event>| {
            let mut out = ctx.run_ui(egui::RawInput { screen_rect: screen, events, ..Default::default() }, |ui| poll(app, &ui.ctx().clone()));
            out.textures_delta.clear();
            let mut at = None;
            fn find(s: &egui::Shape, at: &mut Option<egui::Pos2>) {
                match s {
                    egui::Shape::Text(t) if t.galley.text() == "Cancel" => *at = Some(t.visual_bounding_rect().center()),
                    egui::Shape::Vec(v) => v.iter().for_each(|s| find(s, at)),
                    _ => {}
                }
            }
            for c in &out.shapes {
                find(&c.shape, &mut at);
            }
            at
        };
        run(&mut app, vec![]);
        let at = run(&mut app, vec![]).expect("a Cancel button");
        // As the control channel clicks: moved, pressed and released in one frame.
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        run(&mut app, vec![egui::Event::PointerMoved(at), button(true), button(false)]);
        run(&mut app, vec![]);
        assert!(app.font_trace.is_none(), "cancelled");
        assert_eq!(app.session.documents().len(), docs);
    }
}
