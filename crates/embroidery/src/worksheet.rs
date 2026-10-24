//! The production worksheet (`embroidery.worksheet`): a printable page for whoever sews the design.
//! It shows a picture of the design in its stitches; its size, stitch count, trims, hoop, fabric
//! and sewing time; the thread sequence with colour chips, the chart threads matched to them
//! ([`crate::threads`]) and an estimate of the thread each uses; and a box for notes. It is an
//! ordinary document, so it prints and exports like any other.

use std::sync::Arc;

use kurbo::Point;
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, CharStyle, Document, LayerColor, Node, NodeKind, TextObject};
use vectorcraft_geom::{Anchor, PathData, Rect, SubPath};

use crate::{Cmd, PT_MM, Plan};

/// Thread sewn per mm of stitch, counting what the stitches take up into the fabric (an
/// estimate; real take-up depends on tension and fabric).
pub const TAKE_UP: f64 = 1.2;
/// The page's margin and the picture's size, mm.
const MARGIN: f64 = 15.0;
const PICTURE: f64 = 100.0;
/// The rows of the thread sequence that fit before the notes box, mm.
const ROW: f64 = 7.0;

/// A page size: A4 or US Letter, mm.
pub fn page_size(letter: bool) -> (f64, f64) {
    if letter { (215.9, 279.4) } else { (210.0, 297.0) }
}

/// Thread sewn in each colour block of `plan`, mm (stitch lengths times [`TAKE_UP`]).
pub fn thread_lengths(plan: &Plan) -> Vec<f64> {
    let mut out = vec![0.0; plan.threads.len().max(1)];
    let mut block = 0usize;
    let mut prev: Option<Point> = None;
    for n in &plan.needles {
        let p = Point::new(n.x, n.y);
        match n.cmd {
            Cmd::Stitch => {
                if let (Some(a), Some(len)) = (prev, out.get_mut(block)) {
                    *len += a.distance(p) * TAKE_UP;
                }
                prev = Some(p);
            }
            Cmd::ColorChange => {
                block += 1;
                prev = None;
            }
            Cmd::Jump | Cmd::Trim | Cmd::End => prev = Some(p),
        }
    }
    out.truncate(plan.threads.len());
    out
}

/// The worksheet of `doc`, whose design is `plan`, on an A4 page (US Letter when `letter`).
pub fn build(doc: &Document, plan: &Plan, letter: bool) -> Document {
    let (pw, ph) = page_size(letter);
    let mut out = Document::new(pw / PT_MM, ph / PT_MM);
    out.title = format!("{} worksheet", doc.title);
    out.units = vectorcraft_doc::Unit::Millimeters;
    let mut nodes: Vec<Arc<Node>> = vec![];
    let ink = [20, 20, 20];
    let text = |out: &mut Document, s: &str, x: f64, y: f64, size_pt: f64| {
        let style = CharStyle { size: size_pt, fill: solid(ink), ..CharStyle::default() };
        let t = TextObject::point(Point::new(x / PT_MM, y / PT_MM), s, style);
        Arc::new(Node::new(out.alloc_id(), NodeKind::Text(Box::new(t))))
    };
    // The title.
    nodes.push(text(&mut out, &doc.title, MARGIN, MARGIN + 6.0, 18.0));

    // The design's picture, in its stitches, fitted in a box.
    let top = MARGIN + 14.0;
    nodes.push(Arc::new(rect_node(&mut out, Rect::new(MARGIN, top, MARGIN + PICTURE, top + PICTURE), None, Some([180, 180, 180]))));
    let st = plan.stats();
    if let Some(b) = st.bounds {
        let (w, h) = ((b[2] - b[0]).max(0.1), (b[3] - b[1]).max(0.1));
        let k = ((PICTURE - 8.0) / w).min((PICTURE - 8.0) / h);
        let (ox, oy) = (MARGIN + (PICTURE - w * k) / 2.0 - b[0] * k, top + (PICTURE - h * k) / 2.0 - b[1] * k);
        for (thread, run) in plan.runs() {
            let color = plan.threads.get(thread).copied().unwrap_or([0; 3]);
            let pts: Vec<Point> = run.iter().map(|p| Point::new(p.x * k + ox, p.y * k + oy)).collect();
            if pts.len() >= 2 {
                let path = mm_path(&[pts], false);
                nodes.push(Arc::new(Node::path(out.alloc_id(), path, Appearance::basic(Paint::None, solid(color), 0.25 / PT_MM))));
            }
        }
    }

    // The facts, beside the picture.
    let (w, h) = st.bounds.map_or((0.0, 0.0), |b| (b[2] - b[0], b[3] - b[1]));
    let e = &doc.embroidery;
    let minutes = crate::sewing_seconds(plan, e.speed()) / 60.0;
    let hoop = e.hoop.as_deref().filter(|h| h.unknown.is_none()).map_or_else(
        || "none chosen".to_string(),
        |h| {
            let (hw, hh) = crate::hoops::size(h);
            // Its name, with its size unless the name gives it; and whether the design fits.
            let name = if h.name.contains("mm") { h.name.clone() } else { format!("{} ({hw:.0} × {hh:.0} mm)", h.name) };
            let fit = match crate::hoops::fit(w, st.bounds.map_or(0.0, |b| b[3] - b[1]), h) {
                crate::hoops::Fit::Fits => "",
                crate::hoops::Fit::FitsTurned => ": fits only turned a quarter turn",
                crate::hoops::Fit::TooBig => ": the design doesn't fit",
            };
            format!("{name}{fit}")
        },
    );
    let fabric = e.fabric.as_deref().filter(|f| f.unknown.is_none()).map_or("none chosen".to_string(), |f| f.name.clone());
    let facts = [
        format!("Size: {w:.1} × {h:.1} mm"),
        format!("Stitches: {}", st.stitches),
        format!("Thread changes: {}", st.color_changes),
        format!("Trims: {}   Jumps: {}", st.trims, st.jumps),
        format!("Hoop: {hoop}"),
        format!("Fabric: {fabric}"),
        format!("Sewing time: about {minutes:.0} min at {:.0} stitches/min", e.speed()),
    ];
    let x = MARGIN + PICTURE + 8.0;
    for (i, f) in facts.iter().enumerate() {
        nodes.push(text(&mut out, f, x, top + 6.0 + i as f64 * 7.0, 10.0));
    }

    // The thread sequence.
    let chart = e.chart.as_deref().and_then(|id| e.charts.iter().find(|c| c.id == id && c.unknown.is_none()));
    let lengths = thread_lengths(plan);
    let mut y = top + PICTURE + 12.0;
    nodes.push(text(&mut out, "Thread sequence", MARGIN, y, 12.0));
    y += 4.0;
    let notes_top = ph - MARGIN - 30.0;
    let rows_fit = (((notes_top - 4.0 - y) / ROW).floor().max(0.0)) as usize;
    for (i, t) in plan.threads.iter().enumerate() {
        if i >= rows_fit {
            let more = plan.threads.len() - i;
            nodes.push(text(&mut out, &format!("… and {more} more"), MARGIN, y + 5.0, 10.0));
            break;
        }
        nodes.push(Arc::new(rect_node(&mut out, Rect::new(MARGIN, y + 1.0, MARGIN + 5.0, y + 6.0), Some(*t), Some([80, 80, 80]))));
        let hex = format!("#{:02x}{:02x}{:02x}", t[0], t[1], t[2]);
        let matched = chart.and_then(|c| crate::threads::nearest(c, *t)).map(|(m, d)| format!("{} {} (ΔE {d:.1})", m.code, m.name));
        let meters = lengths.get(i).copied().unwrap_or(0.0) / 1000.0;
        let mut line = format!("{}.  {hex}   {}   {meters:.1} m", i + 1, matched.unwrap_or_default());
        // What to do by hand when the machine stops before this thread.
        let stops: Vec<&str> = plan.stops.iter().filter(|s| s.block == i).map(|s| stop_label(&s.kind)).collect();
        if !stops.is_empty() {
            line.push_str(&format!("   (stop first: {})", stops.join(", ")));
        }
        nodes.push(text(&mut out, &line, MARGIN + 8.0, y + 5.0, 10.0));
        y += ROW;
    }

    // Notes.
    nodes.push(text(&mut out, "Notes", MARGIN, notes_top - 1.5, 10.0));
    nodes.push(Arc::new(rect_node(&mut out, Rect::new(MARGIN, notes_top, pw - MARGIN, ph - MARGIN), None, Some([180, 180, 180]))));

    let mut layer = Node::layer(out.alloc_id(), "Worksheet", LayerColor::Preset(0));
    if let Some(c) = layer.children_mut() {
        *c = nodes;
    }
    out.layers = vec![Arc::new(layer)];
    out
}

/// What the person at the machine does at a stop of `kind` ([`crate::PlanStop`]).
fn stop_label(kind: &str) -> &'static str {
    match kind {
        "foam" => "lay the foam",
        "placeFabric" => "place the fabric",
        "trimFabric" => "trim the fabric",
        _ => "see the notes",
    }
}

fn solid([r, g, b]: [u8; 3]) -> Paint {
    Paint::solid(Color::rgb8(r, g, b))
}

/// Lines given in mm as a path in points.
fn mm_path(runs: &[Vec<Point>], closed: bool) -> PathData {
    PathData::new(
        runs.iter()
            .map(|run| SubPath { anchors: run.iter().map(|p| Anchor::corner(Point::new(p.x / PT_MM, p.y / PT_MM))).collect(), closed })
            .collect(),
    )
}

/// A rectangle (mm) filled and stroked as given.
fn rect_node(d: &mut Document, r: Rect, fill: Option<[u8; 3]>, stroke: Option<[u8; 3]>) -> Node {
    let corners = vec![Point::new(r.x0, r.y0), Point::new(r.x1, r.y0), Point::new(r.x1, r.y1), Point::new(r.x0, r.y1)];
    let look = Appearance::basic(fill.map_or(Paint::None, solid), stroke.map_or(Paint::None, solid), 0.3 / PT_MM);
    Node::path(d.alloc_id(), mm_path(&[corners], true), look)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_worksheet_lists_the_design_and_its_threads() {
        let sheet = crate::testsheet::build(&["satinWidth".into(), "fillAngle".into()]).unwrap();
        let plan = crate::plan(&sheet.doc);
        let w = build(&sheet.doc, &plan, false);
        assert_eq!(w.artboards.len(), 1);
        let mut texts = vec![];
        let mut paths = 0;
        w.walk(|n| match &n.kind {
            NodeKind::Text(t) => texts.push(t.plain_text()),
            NodeKind::Path { .. } => paths += 1,
            _ => {}
        });
        assert!(texts.iter().any(|t| t.starts_with("Stitches: ")), "{texts:?}");
        assert!(texts.iter().any(|t| t.starts_with("1.  #2040") && t.ends_with(" m")), "{texts:?}");
        assert!(paths > 20, "the picture draws the stitches: {paths}");
        // Thread length: the stitches' length with take-up, in one block here.
        let lengths = thread_lengths(&plan);
        assert_eq!(lengths.len(), 1);
        assert!(lengths[0] > 1000.0, "{lengths:?}");
        assert_eq!(page_size(true), (215.9, 279.4));
    }

    #[test]
    fn the_thread_sequence_says_what_to_do_at_each_stop() {
        let sheet = crate::testsheet::build(&["satinWidth".into()]).unwrap();
        let mut plan = crate::plan(&sheet.doc);
        let t = plan.threads.first().copied().unwrap();
        plan.threads.push(t);
        plan.stops = vec![crate::PlanStop { block: 1, kind: "placeFabric".into() }, crate::PlanStop { block: 1, kind: "foam".into() }];
        let w = build(&sheet.doc, &plan, false);
        let mut texts = vec![];
        w.walk(|n| {
            if let NodeKind::Text(t) = &n.kind {
                texts.push(t.plain_text());
            }
        });
        assert!(texts.iter().any(|t| t.starts_with("2.  ") && t.ends_with("(stop first: place the fabric, lay the foam)")), "{texts:?}");
        assert!(texts.iter().any(|t| t.starts_with("1.  ") && !t.contains("stop first")), "{texts:?}");
    }
}
