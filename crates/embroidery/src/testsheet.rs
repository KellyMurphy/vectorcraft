//! Sew-out test sheets (`embroidery.testSheet`): calibration swatches to sew on real fabric, each
//! section changing one setting (satin width and density, pull compensation, fill density and
//! angle, underlays, lettering size) while the others keep their defaults. Each section is a
//! design of its own, on its own artboard, fitting a 100 × 100 mm sewing field; each swatch is
//! labelled in stitches (a single-stroke font drawn here, so no type face is needed), and the
//! sheet's record ([`vectorcraft_doc::SewOut`]) lists the swatches for the sew-out form.

use std::sync::Arc;

use kurbo::{Point, Shape};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{
    Appearance, Artboard, CharStyle, Document, LayerColor, Node, NodeKind, SewOut, SewSwatch, StitchKind, StitchSettings, TextObject, Underlay, Unit,
};
use vectorcraft_geom::{Anchor, PathData, Rect, SubPath};

use crate::PT_MM;

/// The sewing field each section fits, mm (a 100 × 100 mm hoop's, the smallest common one).
pub const FIELD_MM: f64 = 100.0;
/// The thread every swatch is sewn in (one colour: no stops).
pub const THREAD: [u8; 3] = [32, 64, 144];
/// The word sewn in the lettering section.
pub const SAMPLE_TEXT: &str = "Hag";
/// Space between the field's edge and the swatches, mm.
const MARGIN: f64 = 5.0;
/// Space between swatches, mm.
const GAP: f64 = 4.0;
/// Space between the sections' artboards, mm.
const ARTBOARD_GAP: f64 = 20.0;
/// Letter heights of the section title and the swatch labels, mm.
const TITLE_H: f64 = 4.5;
const LABEL_H: f64 = 3.5;
/// Space between a swatch and its label, mm.
const LABEL_GAP: f64 = 1.5;
/// The labels' stitch length, mm (short, for small letters).
const LABEL_STITCH: f64 = 1.0;
/// Satin columns' length and fill squares' side, mm.
const SATIN_LEN: f64 = 30.0;
const SQUARE: f64 = 18.0;
/// The pull compensation marks: their length, and their gap from the column's end, mm.
const TICK: f64 = 3.0;
const TICK_GAP: f64 = 1.5;

/// A section of the test sheet.
pub struct Section {
    /// Its id (`sewOut` swatches' `section`).
    pub id: &'static str,
    /// The letter its swatches are named by (`A3`).
    pub letter: char,
    /// Its name, as the UI shows it.
    pub name: &'static str,
    /// Its title, as sewn (capitals and digits only).
    sewn: &'static str,
}

pub const SECTIONS: [Section; 8] = [
    Section { id: "satinWidth", letter: 'A', name: "Satin Width", sewn: "SATIN WIDTH MM" },
    Section { id: "satinDensity", letter: 'B', name: "Satin Density", sewn: "SATIN SPACING MM" },
    Section { id: "pullCompensation", letter: 'C', name: "Pull Compensation", sewn: "PULL COMP MM" },
    Section { id: "fillDensity", letter: 'D', name: "Fill Density", sewn: "FILL SPACING MM" },
    Section { id: "fillAngle", letter: 'E', name: "Fill Angle", sewn: "FILL ANGLE" },
    Section { id: "underlay", letter: 'F', name: "Underlay", sewn: "UNDERLAY" },
    Section { id: "lettering", letter: 'G', name: "Lettering", sewn: "LETTERING MM" },
    Section { id: "puff", letter: 'H', name: "Puff", sewn: "PUFF 3MM FOAM" },
];

/// A test sheet: the document, and where its sections went.
pub struct Sheet {
    pub doc: Document,
    pub sections: Vec<SheetSection>,
    /// Sections asked for but left out, and why.
    pub skipped: Vec<(&'static str, String)>,
}

/// One section on the sheet.
pub struct SheetSection {
    pub id: &'static str,
    pub name: &'static str,
    /// Its artboard's index.
    pub artboard: usize,
    pub swatches: usize,
}

/// A test sheet of the sections `ids` (empty: all of them), in the order [`SECTIONS`] lists them.
pub fn build(ids: &[String]) -> Result<Sheet, String> {
    if let Some(bad) = ids.iter().find(|id| !SECTIONS.iter().any(|s| s.id == id.as_str())) {
        let all: Vec<&str> = SECTIONS.iter().map(|s| s.id).collect();
        return Err(format!("no test sheet section `{bad}` (there are {})", all.join(", ")));
    }
    let wanted: Vec<&Section> = SECTIONS.iter().filter(|s| ids.is_empty() || ids.iter().any(|i| i == s.id)).collect();
    let mut doc = Document::new(FIELD_MM / PT_MM, FIELD_MM / PT_MM);
    doc.title = "Sew-Out Test Sheet".into();
    doc.units = Unit::Millimeters;
    doc.artboards.clear();
    let mut record = SewOut::default();
    let (mut groups, mut sections, mut skipped) = (vec![], vec![], vec![]);
    for section in wanted {
        let cells = match cells(section.id) {
            Ok(c) => c,
            Err(why) => {
                skipped.push((section.id, why));
                continue;
            }
        };
        let index = doc.artboards.len();
        let left = index as f64 * (FIELD_MM + ARTBOARD_GAP);
        let (group, swatches) = place(&mut doc, section, &cells, left)?;
        doc.artboards.push(Artboard {
            id: u32::try_from(index + 1).unwrap_or(u32::MAX),
            name: format!("{} {}", section.letter, section.name),
            rect: Rect::new(left / PT_MM, 0.0, (left + FIELD_MM) / PT_MM, FIELD_MM / PT_MM),
            show_center_mark: true,
            show_cross_hairs: false,
            locked: false,
            show_video_safe_areas: false,
            video_ruler_pixel_aspect_ratio: 1.0,
            color: None,
        });
        sections.push(SheetSection { id: section.id, name: section.name, artboard: index, swatches: swatches.len() });
        record.swatches.extend(swatches);
        groups.push(Arc::new(group));
    }
    if sections.is_empty() {
        let why: Vec<String> = skipped.iter().map(|(id, why)| format!("{id}: {why}")).collect();
        return Err(format!("no section can be made ({})", why.join("; ")));
    }
    let mut layer = Node::layer(doc.alloc_id(), "Test Sheet", LayerColor::Preset(0));
    if let Some(c) = layer.children_mut() {
        *c = groups;
    }
    doc.layers = vec![Arc::new(layer)];
    doc.embroidery.sew_out = Some(Box::new(record));
    Ok(Sheet { doc, sections, skipped })
}

/// What a swatch sews, put with its top-left corner at a point (mm): its objects.
type Make = Box<dyn Fn(&mut Document, Point) -> Vec<Node>>;

/// One swatch before it is placed: its size (mm, without the label), its label as sewn, what it
/// tests in words, and what makes it.
struct Cell {
    w: f64,
    h: f64,
    label: String,
    words: String,
    make: Make,
}

/// A number as a label: `0.35`, `4` (no trailing zeros).
fn num(v: f64) -> String {
    format!("{v}")
}

/// `kind` as a new object gets it (auto underlay), in the sheet's thread.
fn settings(kind: StitchKind) -> StitchSettings {
    StitchSettings { thread: Some(THREAD), underlay: Underlay::Auto, ..StitchSettings::new(kind) }
}

/// A satin column `width` mm wide sewn with `s`, upright.
fn satin_cell(width: f64, s: StitchSettings, label: String, words: String) -> Cell {
    let make: Make = Box::new(move |d, at| vec![column(d, at, width, s.clone())]);
    Cell { w: width, h: SATIN_LEN, label, words, make }
}

/// A fill square sewn with `s`.
fn square_cell(s: StitchSettings, label: String, words: String) -> Cell {
    let make: Make = Box::new(move |d, at| vec![square(d, at, s.clone())]);
    Cell { w: SQUARE, h: SQUARE, label, words, make }
}

/// The swatches of section `id`; an error when it can't be made here (no type face to set
/// lettering in).
fn cells(id: &str) -> Result<Vec<Cell>, String> {
    let satin = |width: f64| StitchSettings { width: Some(width), ..settings(StitchKind::Satin) };
    let fill = || settings(StitchKind::Fill);
    Ok(match id {
        "satinWidth" => [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 10.0]
            .into_iter()
            .map(|w| satin_cell(w, satin(w), num(w), format!("Satin {} mm wide", num(w))))
            .collect(),
        "satinDensity" => [0.3, 0.35, 0.4, 0.5, 0.6, 0.8]
            .into_iter()
            .map(|sp| satin_cell(4.0, StitchSettings { spacing: Some(sp), ..satin(4.0) }, num(sp), format!("Satin spacing {} mm", num(sp))))
            .collect(),
        "pullCompensation" => [0.0, 0.1, 0.2, 0.3, 0.4, 0.5]
            .into_iter()
            .map(|pull| {
                let s = StitchSettings { pull: Some(pull), ..satin(3.0) };
                let make: Make = Box::new(move |d, at| {
                    let top = Point::new(at.x, at.y + TICK + TICK_GAP);
                    vec![column(d, top, 3.0, s.clone()), ticks(d, top, 3.0)]
                });
                Cell { w: 3.0, h: SATIN_LEN + 2.0 * (TICK + TICK_GAP), label: num(pull), words: format!("Pull compensation {} mm", num(pull)), make }
            })
            .collect(),
        "fillDensity" => [0.3, 0.4, 0.5, 0.6]
            .into_iter()
            .map(|sp| square_cell(StitchSettings { spacing: Some(sp), ..fill() }, num(sp), format!("Fill spacing {} mm", num(sp))))
            .collect(),
        "fillAngle" => [0.0, 45.0, 90.0, 135.0]
            .into_iter()
            .map(|a| square_cell(StitchSettings { angle: a, ..fill() }, num(a), format!("Fill angle {}°", num(a))))
            .collect(),
        "underlay" => {
            let column = Underlay::for_kind(StitchKind::Satin).iter().map(|u| {
                satin_cell(5.0, StitchSettings { underlay: *u, ..satin(5.0) }, u.id().to_ascii_uppercase(), format!("Satin underlay: {}", u.label()))
            });
            let squares = Underlay::for_kind(StitchKind::Fill).iter().map(|u| {
                square_cell(StitchSettings { underlay: *u, ..fill() }, u.id().to_ascii_uppercase(), format!("Fill underlay: {}", u.label()))
            });
            column.chain(squares).collect()
        }
        "puff" => [3.0, 4.0, 5.0, 6.0, 8.0]
            .into_iter()
            .map(|w| satin_cell(w, StitchSettings { puff: Some(3.0), ..satin(w) }, num(w), format!("Puff {} mm wide on 3 mm foam", num(w))))
            .collect(),
        "lettering" => {
            let mut out = vec![];
            for size in [4.0, 6.0, 8.0, 10.0, 15.0, 20.0] {
                let text = type_object(size);
                let Some(b) = type_bounds(&text) else { return Err("no type face is available to set lettering in".into()) };
                let make: Make = Box::new(move |d, at| {
                    let mut t = text.clone();
                    // The type's box (points, from its origin) at the swatch's corner.
                    t.xf = kurbo::Affine::translate((at.x / PT_MM - b.x0, at.y / PT_MM - b.y0));
                    let mut n = Node::new(d.alloc_id(), NodeKind::Text(Box::new(t)));
                    n.extras_mut().stitch = Some(Box::new(settings(StitchKind::Auto)));
                    vec![n]
                });
                out.push(Cell { w: b.width() * PT_MM, h: b.height() * PT_MM, label: num(size), words: format!("Lettering {} mm", num(size)), make });
            }
            out
        }
        _ => return Err(format!("no section `{id}`")),
    })
}

/// [`SAMPLE_TEXT`] at `size_mm`, in the thread's colour, its origin at zero.
fn type_object(size_mm: f64) -> TextObject {
    let style = CharStyle { size: size_mm / PT_MM, fill: thread_paint(), ..CharStyle::default() };
    TextObject::point(Point::ZERO, SAMPLE_TEXT, style)
}

/// The box of `t`'s letters, points from its origin (none when no type face lays it out).
fn type_bounds(t: &TextObject) -> Option<kurbo::Rect> {
    let path = vectorcraft_text::layout(vectorcraft_text::FontDb::global(), t).to_bezpath();
    let b = path.bounding_box();
    (path.elements().len() > 1 && b.width() > 0.0 && b.height() > 0.0 && b.width().is_finite() && b.height().is_finite()).then_some(b)
}

/// Lay `cells` out in rows inside the field starting `left` mm across: the section's title, then
/// each swatch with its label under it. → the section's group and its swatches' records.
fn place(doc: &mut Document, section: &Section, cells: &[Cell], left: f64) -> Result<(Node, Vec<SewSwatch>), String> {
    let mut children = vec![];
    if let Some(title) = label(doc, &format!("{} {}", section.letter, section.sewn), Point::new(left + MARGIN, MARGIN), TITLE_H) {
        children.push(Arc::new(title));
    }
    let (mut x, mut y, mut row) = (MARGIN, MARGIN + TITLE_H + GAP, 0.0f64);
    let end = FIELD_MM - MARGIN;
    let mut swatches = vec![];
    let mut flat: Vec<Arc<Node>> = vec![];
    for (i, cell) in cells.iter().enumerate() {
        let lw = label_width(&cell.label, LABEL_H);
        let (cw, ch) = (cell.w.max(lw), cell.h + LABEL_GAP + LABEL_H);
        if x + cw > end && x > MARGIN {
            (x, y, row) = (MARGIN, y + row + GAP, 0.0);
        }
        if x + cw > end || y + ch > end {
            return Err(format!("the {} section's swatch {} doesn't fit a {FIELD_MM} mm field", section.id, i + 1));
        }
        let mut nodes = (cell.make)(doc, Point::new(left + x + (cw - cell.w) / 2.0, y));
        let puffed = nodes.iter().any(|n| n.extras().stitch.as_deref().is_some_and(StitchSettings::puffed));
        let label = label(doc, &cell.label, Point::new(left + x + (cw - lw) / 2.0, y + cell.h + LABEL_GAP), LABEL_H);
        let mut objects: Vec<_> = nodes.iter().map(|n| n.id).collect();
        if let Some(l) = label {
            objects.push(l.id);
            // A puffed swatch's label is sewn flat before the foam is laid, with the title.
            if puffed {
                flat.push(Arc::new(l));
            } else {
                nodes.push(l);
            }
        }
        let id = format!("{}{}", section.letter, i + 1);
        let mut group = Node::group(doc.alloc_id(), nodes.into_iter().map(Arc::new).collect());
        group.name = Some(format!("{id} {}", cell.words));
        children.push(Arc::new(group));
        swatches.push(SewSwatch { id, section: section.id.into(), label: cell.words.clone(), objects, ..SewSwatch::default() });
        x += cw + GAP;
        row = row.max(ch);
    }
    // Flat labels of puffed swatches go first, after the title.
    let at = children.len().min(1);
    children.splice(at..at, flat);
    let mut group = Node::group(doc.alloc_id(), children);
    group.name = Some(format!("{} {}", section.letter, section.name));
    Ok((group, swatches))
}

fn thread_paint() -> Paint {
    let [r, g, b] = THREAD;
    Paint::solid(Color::rgb8(r, g, b))
}

/// Open or closed lines (mm) as a path (points).
fn lines(runs: &[Vec<Point>], closed: bool) -> PathData {
    PathData::new(
        runs.iter()
            .map(|run| SubPath { anchors: run.iter().map(|p| Anchor::corner(Point::new(p.x / PT_MM, p.y / PT_MM))).collect(), closed })
            .collect(),
    )
}

/// An upright satin column `width` mm wide from `top` (its top-left, mm), drawn as a stroke that
/// wide.
fn column(d: &mut Document, top: Point, width: f64, s: StitchSettings) -> Node {
    let x = top.x + width / 2.0;
    let path = lines(&[vec![Point::new(x, top.y), Point::new(x, top.y + SATIN_LEN)]], false);
    let mut n = Node::path(d.alloc_id(), path, Appearance::basic(Paint::None, thread_paint(), width / PT_MM));
    n.extras_mut().stitch = Some(Box::new(s));
    n
}

/// Running-stitch marks at the drawn edges of a column `width` mm wide from `top`, above and
/// below it: the sewn column's edges show against them how far the thread pulled it in.
fn ticks(d: &mut Document, top: Point, width: f64) -> Node {
    let mut runs = vec![];
    for x in [top.x, top.x + width] {
        runs.push(vec![Point::new(x, top.y - TICK_GAP - TICK), Point::new(x, top.y - TICK_GAP)]);
        let below = top.y + SATIN_LEN + TICK_GAP;
        runs.push(vec![Point::new(x, below), Point::new(x, below + TICK)]);
    }
    let mut n = Node::path(d.alloc_id(), lines(&runs, false), Appearance::basic(Paint::None, thread_paint(), 0.3 / PT_MM));
    n.extras_mut().stitch = Some(Box::new(StitchSettings { length: Some(LABEL_STITCH), ..settings(StitchKind::Running) }));
    n
}

/// A fill square from `top` (its top-left, mm).
fn square(d: &mut Document, top: Point, s: StitchSettings) -> Node {
    let (x0, y0, x1, y1) = (top.x, top.y, top.x + SQUARE, top.y + SQUARE);
    let path = lines(&[vec![Point::new(x0, y0), Point::new(x1, y0), Point::new(x1, y1), Point::new(x0, y1)]], true);
    let mut n = Node::path(d.alloc_id(), path, Appearance::basic(thread_paint(), Paint::None, 0.0));
    n.extras_mut().stitch = Some(Box::new(s));
    n
}

/// `text` sewn in bean stitch, `height` mm tall, from `top` (its top-left, mm); none when it
/// has nothing to sew.
fn label(d: &mut Document, text: &str, top: Point, height: f64) -> Option<Node> {
    let runs = label_runs(text, top, height);
    if runs.is_empty() {
        return None;
    }
    let mut n = Node::path(d.alloc_id(), lines(&runs, false), Appearance::basic(Paint::None, thread_paint(), 0.3 / PT_MM));
    n.name = Some(text.to_string());
    n.extras_mut().stitch = Some(Box::new(StitchSettings { length: Some(LABEL_STITCH), ..settings(StitchKind::Bean) }));
    Some(n)
}

/// The label font's grid: a capital is [`UNITS_H`] units tall; letters are 4 units wide and one
/// unit apart.
const UNITS_H: f64 = 6.0;
const SPACE: f64 = 1.0;

/// How wide `text` is in the label font at `height` mm.
pub fn label_width(text: &str, height: f64) -> f64 {
    let units: f64 = text.chars().map(|c| glyph(c).0 + SPACE).sum::<f64>() - SPACE;
    units.max(0.0) * height / UNITS_H
}

/// `text`'s strokes in the label font, `height` mm tall from `top` (mm): one line per stroke.
pub fn label_runs(text: &str, top: Point, height: f64) -> Vec<Vec<Point>> {
    let k = height / UNITS_H;
    let mut x = 0.0;
    let mut out = vec![];
    for c in text.chars() {
        let (w, strokes) = glyph(c);
        for s in strokes {
            out.push(s.iter().map(|(gx, gy)| Point::new(top.x + (x + gx) * k, top.y + gy * k)).collect());
        }
        x += w + SPACE;
    }
    out
}

type Strokes = &'static [&'static [(f64, f64)]];

const O: &[(f64, f64)] = &[(1.0, 0.0), (3.0, 0.0), (4.0, 1.0), (4.0, 5.0), (3.0, 6.0), (1.0, 6.0), (0.0, 5.0), (0.0, 1.0), (1.0, 0.0)];
const P: &[(f64, f64)] = &[(0.0, 6.0), (0.0, 0.0), (3.0, 0.0), (4.0, 1.0), (4.0, 2.0), (3.0, 3.0), (0.0, 3.0)];

/// A character of the label font: its width (units) and strokes (x right, y down, in units).
/// Lower case is drawn as capitals; characters it lacks are a blank.
fn glyph(c: char) -> (f64, Strokes) {
    match c.to_ascii_uppercase() {
        'A' => (4.0, &[&[(0.0, 6.0), (2.0, 0.0), (4.0, 6.0)], &[(0.7, 4.0), (3.3, 4.0)]]),
        'B' => (
            4.0,
            &[
                &[(0.0, 6.0), (0.0, 0.0), (3.0, 0.0), (4.0, 1.0), (4.0, 2.0), (3.0, 3.0), (0.0, 3.0)],
                &[(3.0, 3.0), (4.0, 4.0), (4.0, 5.0), (3.0, 6.0), (0.0, 6.0)],
            ],
        ),
        'C' => (4.0, &[&[(4.0, 1.0), (3.0, 0.0), (1.0, 0.0), (0.0, 1.0), (0.0, 5.0), (1.0, 6.0), (3.0, 6.0), (4.0, 5.0)]]),
        'D' => (4.0, &[&[(0.0, 0.0), (0.0, 6.0), (2.5, 6.0), (4.0, 4.5), (4.0, 1.5), (2.5, 0.0), (0.0, 0.0)]]),
        'E' => (4.0, &[&[(4.0, 0.0), (0.0, 0.0), (0.0, 6.0), (4.0, 6.0)], &[(0.0, 3.0), (3.0, 3.0)]]),
        'F' => (4.0, &[&[(4.0, 0.0), (0.0, 0.0), (0.0, 6.0)], &[(0.0, 3.0), (3.0, 3.0)]]),
        'G' => (4.0, &[&[(4.0, 1.0), (3.0, 0.0), (1.0, 0.0), (0.0, 1.0), (0.0, 5.0), (1.0, 6.0), (3.0, 6.0), (4.0, 5.0), (4.0, 3.5), (2.5, 3.5)]]),
        'H' => (4.0, &[&[(0.0, 0.0), (0.0, 6.0)], &[(4.0, 0.0), (4.0, 6.0)], &[(0.0, 3.0), (4.0, 3.0)]]),
        'I' => (2.0, &[&[(0.0, 0.0), (2.0, 0.0)], &[(1.0, 0.0), (1.0, 6.0)], &[(0.0, 6.0), (2.0, 6.0)]]),
        'J' => (4.0, &[&[(4.0, 0.0), (4.0, 5.0), (3.0, 6.0), (1.0, 6.0), (0.0, 5.0)]]),
        'K' => (4.0, &[&[(0.0, 0.0), (0.0, 6.0)], &[(4.0, 0.0), (0.0, 3.5)], &[(1.3, 2.6), (4.0, 6.0)]]),
        'L' => (4.0, &[&[(0.0, 0.0), (0.0, 6.0), (4.0, 6.0)]]),
        'M' => (4.0, &[&[(0.0, 6.0), (0.0, 0.0), (2.0, 3.0), (4.0, 0.0), (4.0, 6.0)]]),
        'N' => (4.0, &[&[(0.0, 6.0), (0.0, 0.0), (4.0, 6.0), (4.0, 0.0)]]),
        'O' | '0' => (4.0, &[O]),
        'P' => (4.0, &[P]),
        'Q' => (4.0, &[O, &[(2.5, 4.5), (4.0, 6.0)]]),
        'R' => (4.0, &[P, &[(2.0, 3.0), (4.0, 6.0)]]),
        'S' => (
            4.0,
            &[&[
                (4.0, 1.0),
                (3.0, 0.0),
                (1.0, 0.0),
                (0.0, 1.0),
                (0.0, 2.0),
                (1.0, 3.0),
                (3.0, 3.0),
                (4.0, 4.0),
                (4.0, 5.0),
                (3.0, 6.0),
                (1.0, 6.0),
                (0.0, 5.0),
            ]],
        ),
        'T' => (4.0, &[&[(0.0, 0.0), (4.0, 0.0)], &[(2.0, 0.0), (2.0, 6.0)]]),
        'U' => (4.0, &[&[(0.0, 0.0), (0.0, 5.0), (1.0, 6.0), (3.0, 6.0), (4.0, 5.0), (4.0, 0.0)]]),
        'V' => (4.0, &[&[(0.0, 0.0), (2.0, 6.0), (4.0, 0.0)]]),
        'W' => (4.0, &[&[(0.0, 0.0), (1.0, 6.0), (2.0, 2.0), (3.0, 6.0), (4.0, 0.0)]]),
        'X' => (4.0, &[&[(0.0, 0.0), (4.0, 6.0)], &[(4.0, 0.0), (0.0, 6.0)]]),
        'Y' => (4.0, &[&[(0.0, 0.0), (2.0, 3.0), (4.0, 0.0)], &[(2.0, 3.0), (2.0, 6.0)]]),
        'Z' => (4.0, &[&[(0.0, 0.0), (4.0, 0.0), (0.0, 6.0), (4.0, 6.0)]]),
        '1' => (3.0, &[&[(0.5, 1.0), (1.5, 0.0), (1.5, 6.0)], &[(0.0, 6.0), (3.0, 6.0)]]),
        '2' => (4.0, &[&[(0.0, 1.0), (1.0, 0.0), (3.0, 0.0), (4.0, 1.0), (4.0, 2.0), (0.0, 6.0), (4.0, 6.0)]]),
        '3' => (
            4.0,
            &[
                &[(0.0, 1.0), (1.0, 0.0), (3.0, 0.0), (4.0, 1.0), (4.0, 2.0), (3.0, 3.0), (1.5, 3.0)],
                &[(3.0, 3.0), (4.0, 4.0), (4.0, 5.0), (3.0, 6.0), (1.0, 6.0), (0.0, 5.0)],
            ],
        ),
        '4' => (4.0, &[&[(3.0, 6.0), (3.0, 0.0), (0.0, 4.0), (4.0, 4.0)]]),
        '5' => (4.0, &[&[(4.0, 0.0), (0.0, 0.0), (0.0, 3.0), (3.0, 3.0), (4.0, 4.0), (4.0, 5.0), (3.0, 6.0), (0.0, 6.0)]]),
        '6' => (4.0, &[&[(3.5, 0.0), (1.0, 0.0), (0.0, 1.0), (0.0, 5.0), (1.0, 6.0), (3.0, 6.0), (4.0, 5.0), (4.0, 4.0), (3.0, 3.0), (0.0, 3.0)]]),
        '7' => (4.0, &[&[(0.0, 0.0), (4.0, 0.0), (1.5, 6.0)]]),
        '8' => (
            4.0,
            &[&[
                (1.0, 3.0),
                (0.0, 2.0),
                (0.0, 1.0),
                (1.0, 0.0),
                (3.0, 0.0),
                (4.0, 1.0),
                (4.0, 2.0),
                (3.0, 3.0),
                (1.0, 3.0),
                (0.0, 4.0),
                (0.0, 5.0),
                (1.0, 6.0),
                (3.0, 6.0),
                (4.0, 5.0),
                (4.0, 4.0),
                (3.0, 3.0),
            ]],
        ),
        '9' => (4.0, &[&[(4.0, 3.0), (1.0, 3.0), (0.0, 2.0), (0.0, 1.0), (1.0, 0.0), (3.0, 0.0), (4.0, 1.0), (4.0, 5.0), (3.0, 6.0), (0.5, 6.0)]]),
        // A dot: a small square, so it makes stitches at small sizes.
        '.' => (1.4, &[&[(0.4, 5.4), (1.0, 5.4), (1.0, 6.0), (0.4, 6.0), (0.4, 5.4)]]),
        '-' => (3.0, &[&[(0.0, 3.0), (3.0, 3.0)]]),
        '/' => (3.0, &[&[(0.0, 6.0), (3.0, 0.0)]]),
        _ => (2.5, &[]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_section_fits_its_field_and_lists_its_swatches() {
        let sheet = build(&[]).unwrap();
        let made: Vec<&str> = sheet.sections.iter().map(|s| s.id).collect();
        let skipped: Vec<&str> = sheet.skipped.iter().map(|s| s.0).collect();
        assert_eq!(made.len() + skipped.len(), SECTIONS.len());
        assert!(skipped.iter().all(|s| *s == "lettering"), "only lettering needs a type face: {skipped:?}");
        let record = sheet.doc.embroidery.sew_out.as_ref().unwrap();
        assert_eq!(record.swatches.len(), sheet.sections.iter().map(|s| s.swatches).sum::<usize>());
        assert_eq!(record.swatches.first().map(|s| (s.id.as_str(), s.label.as_str())), Some(("A1", "Satin 1 mm wide")));
        // Each swatch's objects lie inside its section's artboard, and each is stitched.
        for sw in &record.swatches {
            let sec = sheet.sections.iter().find(|s| s.id == sw.section).unwrap();
            let board = sheet.doc.artboards[sec.artboard].rect;
            assert!(!sw.objects.is_empty(), "{}", sw.id);
            for id in &sw.objects {
                let n = sheet.doc.node(*id).unwrap();
                assert!(n.extras().stitch.is_some(), "{} {id:?}", sw.id);
                if let Some(b) = n.geometric_bounds() {
                    assert!(b.x0 >= board.x0 && b.x1 <= board.x1 && b.y0 >= board.y0 && b.y1 <= board.y1, "{} {b:?} in {board:?}", sw.id);
                }
            }
        }
        // It sews, in one thread, with nothing the checker calls an error but the wide satins
        // meant to test the limit.
        let plan = crate::plan(&sheet.doc);
        assert!(plan.stats().stitches > 10_000, "{}", plan.stats().stitches);
        assert!(plan.threads.iter().all(|t| *t == THREAD), "one thread (puff stops for the foam with it): {:?}", plan.threads);
        assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);
    }

    #[test]
    fn sections_are_picked_by_id() {
        let sheet = build(&["fillAngle".into(), "satinWidth".into()]).unwrap();
        let ids: Vec<&str> = sheet.sections.iter().map(|s| s.id).collect();
        assert_eq!(ids, ["satinWidth", "fillAngle"], "in the sheet's order");
        assert_eq!(sheet.doc.artboards.len(), 2);
        assert!(build(&["nope".into()]).err().unwrap().contains("satinWidth"));
    }

    #[test]
    fn labels_have_strokes_for_what_they_sew() {
        for s in SECTIONS {
            for c in s.sewn.chars().filter(|c| *c != ' ') {
                assert!(!glyph(c).1.is_empty(), "{c} in {}", s.sewn);
            }
        }
        for c in "0123456789.-".chars() {
            assert!(!glyph(c).1.is_empty(), "{c}");
        }
        assert_eq!(label_runs("A1", Point::ZERO, 6.0).len(), 4);
        assert!((label_width("AB", 6.0) - 9.0).abs() < 1e-9);
        // Every stroke stays inside its letter's box.
        for c in ('A'..='Z').chain('0'..='9') {
            let (w, strokes) = glyph(c);
            assert!(strokes.iter().flat_map(|s| s.iter()).all(|(x, y)| (0.0..=w).contains(x) && (0.0..=UNITS_H).contains(y)), "{c}");
        }
    }
}
