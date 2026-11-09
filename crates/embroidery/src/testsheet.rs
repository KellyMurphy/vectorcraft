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
    Appearance, Artboard, CharStyle, Document, LayerColor, Node, NodeKind, RunStyle, SewOut, SewSwatch, Split, SplitPattern, StitchKind,
    StitchSettings, TextObject, Underlay, Unit,
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
/// A run styles swatch's size, mm.
const RUN_W: f64 = 22.0;
const RUN_H: f64 = 14.0;
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
    /// A compensation section's stitch direction, degrees counter-clockwise from the sheet's
    /// horizontal (the fabric hooped with its grain running top to bottom).
    pub angle: Option<f64>,
    /// On a test sheet made without naming its sections. The compensation sections are a sheet
    /// of their own ([`COMPENSATION`]).
    pub default: bool,
}

/// The id that names every compensation section at once.
pub const COMPENSATION: &str = "compensation";

const fn section(id: &'static str, letter: char, name: &'static str, sewn: &'static str) -> Section {
    Section { id, letter, name, sewn, angle: None, default: true }
}

const fn compensation(id: &'static str, letter: char, sewn: &'static str, angle: f64) -> Section {
    Section { id, letter, name: "Compensation", sewn, angle: Some(angle), default: false }
}

pub const SECTIONS: [Section; 14] = [
    section("satinWidth", 'A', "Satin Width", "SATIN WIDTH MM"),
    section("satinDensity", 'B', "Satin Density", "SATIN SPACING MM"),
    section("pullCompensation", 'C', "Pull Compensation", "PULL COMP MM"),
    section("fillDensity", 'D', "Fill Density", "FILL SPACING MM"),
    section("fillAngle", 'E', "Fill Angle", "FILL ANGLE"),
    section("underlay", 'F', "Underlay", "UNDERLAY"),
    section("lettering", 'G', "Lettering", "LETTERING MM"),
    section("runStyles", 'H', "Run Styles", "RUN STYLES"),
    section("splitSatin", 'I', "Split Satin", "SPLIT SATIN MM"),
    // The compensation sheet (E10.1): satin columns and a fill square sewn with no compensation
    // at one stitch direction each, to measure how far the fabric draws in and pushes out.
    compensation("compensation0", 'K', "COMP 0", 0.0),
    compensation("compensation45", 'L', "COMP 45", 45.0),
    compensation("compensation90", 'M', "COMP 90", 90.0),
    compensation("compensation135", 'N', "COMP 135", 135.0),
    // Last: everything sewn after the foam is laid would sew through it.
    section("puff", 'J', "Puff", "PUFF 3MM FOAM"),
];

impl Section {
    /// Its name with its angle, as the UI and the artboard show it (`Compensation 45°`).
    pub fn title(&self) -> String {
        match self.angle {
            Some(a) => format!("{} {}°", self.name, num(a)),
            None => self.name.to_string(),
        }
    }
}

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
    /// A compensation section's stitch direction ([`Section::angle`]).
    pub angle: Option<f64>,
    /// Its artboard's index.
    pub artboard: usize,
    pub swatches: usize,
}

/// A test sheet of the sections `ids` (empty: the [`Section::default`] ones; [`COMPENSATION`]:
/// every compensation section), in the order [`SECTIONS`] lists them.
pub fn build(ids: &[String]) -> Result<Sheet, String> {
    if let Some(bad) = ids.iter().find(|id| id.as_str() != COMPENSATION && !SECTIONS.iter().any(|s| s.id == id.as_str())) {
        let all: Vec<&str> = SECTIONS.iter().map(|s| s.id).chain([COMPENSATION]).collect();
        return Err(format!("no test sheet section `{bad}` (there are {})", all.join(", ")));
    }
    let picked = |s: &Section| ids.iter().any(|i| i == s.id || (i == COMPENSATION && s.angle.is_some()));
    let wanted: Vec<&Section> = SECTIONS.iter().filter(|s| if ids.is_empty() { s.default } else { picked(s) }).collect();
    let mut doc = Document::new(FIELD_MM / PT_MM, FIELD_MM / PT_MM);
    doc.title = "Sew-Out Test Sheet".into();
    doc.units = Unit::Millimeters;
    doc.artboards.clear();
    let mut record = SewOut::default();
    let (mut groups, mut sections, mut skipped) = (vec![], vec![], vec![]);
    for section in wanted {
        let index = doc.artboards.len();
        let left = index as f64 * (FIELD_MM + ARTBOARD_GAP);
        let (group, swatches) = match section.angle {
            Some(angle) => place_turned(&mut doc, section, angle, left)?,
            None => match cells(section.id) {
                Ok(cells) => place(&mut doc, section, &cells, left)?,
                Err(why) => {
                    skipped.push((section.id, why));
                    continue;
                }
            },
        };
        doc.artboards.push(Artboard {
            id: u32::try_from(index + 1).unwrap_or(u32::MAX),
            name: format!("{} {}", section.letter, section.title()),
            rect: Rect::new(left / PT_MM, 0.0, (left + FIELD_MM) / PT_MM, FIELD_MM / PT_MM),
            show_center_mark: true,
            show_cross_hairs: false,
            locked: false,
            show_video_safe_areas: false,
            video_ruler_pixel_aspect_ratio: 1.0,
            color: None,
        });
        sections.push(SheetSection { id: section.id, name: section.name, angle: section.angle, artboard: index, swatches: swatches.len() });
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
        "runStyles" => RunStyle::ALL
            .into_iter()
            .map(|style| {
                let s = StitchSettings { style, ..settings(StitchKind::Running) };
                let make: Make = Box::new(move |d, at| vec![run_line(d, at, s.clone())]);
                Cell { w: RUN_W, h: RUN_H, label: style.id().to_ascii_uppercase(), words: format!("Running stitch: {}", style.label()), make }
            })
            .collect(),
        // Split at the default 7 mm into pieces at most 4 mm, staggered; then 12 mm split at
        // random, and 12 mm unsplit to compare.
        "splitSatin" => {
            let mut out: Vec<Cell> =
                [8.0, 12.0, 20.0].into_iter().map(|w| satin_cell(w, satin(w), num(w), format!("Split satin {} mm wide", num(w)))).collect();
            out.push(satin_cell(
                12.0,
                StitchSettings { split_pattern: SplitPattern::Random, ..satin(12.0) },
                "RANDOM".into(),
                "Split satin 12 mm wide, split at random".into(),
            ));
            out.push(satin_cell(12.0, StitchSettings { split: Split::Off, ..satin(12.0) }, "OFF".into(), "Satin 12 mm wide, not split".into()));
            out
        }
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

/// The satin columns' widths on a compensation section, mm.
pub const COMP_WIDTHS: [f64; 4] = [2.0, 4.0, 6.0, 8.0];

/// A compensation section ([`Section::angle`]): satin columns [`COMP_WIDTHS`] wide and a fill
/// square, each between running-stitch marks at its drawn edges and sewn with no compensation.
/// They are laid out upright (the satins' stitches and the fill's rows running across), centred
/// under the title, then turned about the centre so the stitches run at `angle`. The title stays
/// upright. Each swatch's record holds the angle and its drawn width (along the stitches) and
/// length (across them), for the sizes measured on the sewn swatch.
fn place_turned(doc: &mut Document, section: &Section, angle: f64, left: f64) -> Result<(Node, Vec<SewSwatch>), String> {
    let mark = TICK + TICK_GAP;
    let mut children = vec![];
    if let Some(title) = label(doc, &format!("{} {}", section.letter, section.sewn), Point::new(left + MARGIN, MARGIN), TITLE_H) {
        children.push(Arc::new(title));
    }
    let comp = |kind| StitchSettings { pull: Some(0.0), pull_percent: Some(0.0), push: Some(0.0), ..settings(kind) };
    let a = num(angle);
    // Each swatch: its box (marks included), its label, what it tests, its drawn width and length.
    struct Item {
        w: f64,
        h: f64,
        label: String,
        words: String,
        drawn: (f64, f64),
    }
    let mut items: Vec<Item> = COMP_WIDTHS
        .iter()
        .map(|w| Item {
            w: *w,
            h: SATIN_LEN + 2.0 * mark,
            label: num(*w),
            words: format!("Satin {} mm wide, stitches at {a}°", num(*w)),
            drawn: (*w, SATIN_LEN),
        })
        .collect();
    items.push(Item {
        w: SQUARE + 2.0 * mark,
        h: SQUARE + 2.0 * mark,
        label: "FILL".into(),
        words: format!("Fill {} mm square, rows at {a}°", num(SQUARE)),
        drawn: (SQUARE, SQUARE),
    });
    let cell_w: Vec<f64> = items.iter().map(|it| it.w.max(label_width(&it.label, LABEL_H))).collect();
    let total_w = cell_w.iter().sum::<f64>() + GAP * (items.len().saturating_sub(1)) as f64;
    let box_h = items.iter().map(|it| it.h).fold(0.0, f64::max);
    let total_h = box_h + LABEL_GAP + LABEL_H;
    // The area under the title, mm; the turned swatches must stay inside it.
    let (x0, x1, y0, y1) = (left + MARGIN, left + FIELD_MM - MARGIN, MARGIN + TITLE_H + GAP, FIELD_MM - MARGIN);
    let centre = Point::new((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    // Page counter-clockwise is a negative turn (y down), as a fill's angle is.
    let turn = kurbo::Affine::rotate_about(-angle.to_radians(), Point::new(centre.x / PT_MM, centre.y / PT_MM));
    let (mut x, y) = (centre.x - total_w / 2.0, centre.y - total_h / 2.0);
    let mut swatches = vec![];
    for (i, (it, cw)) in items.into_iter().zip(cell_w).enumerate() {
        let at = Point::new(x + (cw - it.w) / 2.0, y + (box_h - it.h) / 2.0);
        let mut nodes = if i < COMP_WIDTHS.len() {
            let top = Point::new(at.x, at.y + mark);
            vec![column(doc, top, it.w, StitchSettings { width: Some(it.w), ..comp(StitchKind::Satin) }), ticks(doc, top, it.w)]
        } else {
            let top = Point::new(at.x + mark, at.y + mark);
            vec![square(doc, top, StitchSettings { angle, ..comp(StitchKind::Fill) }), square_ticks(doc, top)]
        };
        let lw = label_width(&it.label, LABEL_H);
        if let Some(l) = label(doc, &it.label, Point::new(x + (cw - lw) / 2.0, y + box_h + LABEL_GAP), LABEL_H) {
            nodes.push(l);
        }
        for n in &mut nodes {
            n.transform(turn, false);
        }
        let id = format!("{}{}", section.letter, i + 1);
        let objects: Vec<_> = nodes.iter().map(|n| n.id).collect();
        let mut group = Node::group(doc.alloc_id(), nodes.into_iter().map(Arc::new).collect());
        group.name = Some(format!("{id} {}", it.words));
        if let Some(b) = group.geometric_bounds() {
            let inside = b.x0 * PT_MM >= x0 - 1e-6 && b.x1 * PT_MM <= x1 + 1e-6 && b.y0 * PT_MM >= y0 - 1e-6 && b.y1 * PT_MM <= y1 + 1e-6;
            if !inside {
                return Err(format!("the {} section's swatch {} doesn't fit a {FIELD_MM} mm field", section.id, i + 1));
            }
        }
        children.push(Arc::new(group));
        swatches.push(SewSwatch {
            id,
            section: section.id.into(),
            label: it.words,
            objects,
            angle: Some(angle),
            drawn_width: Some(it.drawn.0),
            drawn_length: Some(it.drawn.1),
            ..SewSwatch::default()
        });
        x += cw + GAP;
    }
    let mut group = Node::group(doc.alloc_id(), children);
    group.name = Some(format!("{} {}", section.letter, section.title()));
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
    marks(d, &runs)
}

/// Running-stitch marks at the drawn edges of a fill square from `top`: each edge carried on past
/// both of its corners.
fn square_ticks(d: &mut Document, top: Point) -> Node {
    let (x0, y0, x1, y1) = (top.x, top.y, top.x + SQUARE, top.y + SQUARE);
    let (g, t) = (TICK_GAP, TICK);
    let mut runs = vec![];
    for x in [x0, x1] {
        runs.push(vec![Point::new(x, y0 - g - t), Point::new(x, y0 - g)]);
        runs.push(vec![Point::new(x, y1 + g), Point::new(x, y1 + g + t)]);
    }
    for y in [y0, y1] {
        runs.push(vec![Point::new(x0 - g - t, y), Point::new(x0 - g, y)]);
        runs.push(vec![Point::new(x1 + g, y), Point::new(x1 + g + t, y)]);
    }
    marks(d, &runs)
}

/// Short running-stitch marks along `runs` (mm).
fn marks(d: &mut Document, runs: &[Vec<Point>]) -> Node {
    let mut n = Node::path(d.alloc_id(), lines(runs, false), Appearance::basic(Paint::None, thread_paint(), 0.3 / PT_MM));
    n.extras_mut().stitch = Some(Box::new(StitchSettings { length: Some(LABEL_STITCH), ..settings(StitchKind::Running) }));
    n
}

/// A run styles swatch from `top` (its top-left, mm): an S curve, then a sharp turn into a
/// straight, so the style shows on curves and how it starts again at a corner.
fn run_line(d: &mut Document, top: Point, s: StitchSettings) -> Node {
    let curve = RUN_W * 0.7;
    let mut pts: Vec<Point> = (0..=28)
        .map(|k| {
            let x = curve * k as f64 / 28.0;
            Point::new(top.x + x, top.y + RUN_H / 2.0 + RUN_H * 0.3 * (std::f64::consts::TAU * x / curve).sin())
        })
        .collect();
    pts.push(Point::new(top.x + RUN_W, top.y));
    let mut n = Node::path(d.alloc_id(), lines(&[pts], false), Appearance::basic(Paint::None, thread_paint(), 0.3 / PT_MM));
    n.extras_mut().stitch = Some(Box::new(s));
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
        let all: Vec<String> = SECTIONS.iter().map(|s| s.id.to_string()).collect();
        let sheet = build(&all).unwrap();
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

    /// The direction of the line from `a` to `b`, degrees counter-clockwise from the page's
    /// horizontal (y down), from 0 up to 180.
    fn direction(a: &crate::Needle, b: &crate::Needle) -> f64 {
        (-(b.y - a.y)).atan2(b.x - a.x).to_degrees().rem_euclid(180.0)
    }

    /// How far apart two directions are, degrees (0 to 90).
    fn apart(a: f64, b: f64) -> f64 {
        let d = (a - b).rem_euclid(180.0);
        d.min(180.0 - d)
    }

    /// The most common direction (to 5°) of the stitches longer than `min` mm sewn for `id`.
    fn main_direction(plan: &crate::Plan, id: vectorcraft_doc::NodeId, min: f64) -> f64 {
        let o = plan.objects.iter().find(|o| o.id == id).unwrap();
        let needles = &plan.needles[o.needles.clone()];
        let mut bins = [0usize; 36];
        for w in needles.windows(2) {
            if w[1].cmd == crate::Cmd::Stitch && (w[1].x - w[0].x).hypot(w[1].y - w[0].y) > min {
                bins[(direction(&w[0], &w[1]) / 5.0).round() as usize % 36] += 1;
            }
        }
        let best = (0..36).max_by_key(|i| bins[*i]).unwrap();
        best as f64 * 5.0
    }

    #[test]
    fn the_compensation_sheet_turns_each_section_to_its_stitch_direction() {
        let default = build(&[]).unwrap();
        assert!(default.sections.iter().all(|s| s.angle.is_none()), "a sheet of its own, not on the default one");
        let sheet = build(&[COMPENSATION.into()]).unwrap();
        let angles: Vec<Option<f64>> = sheet.sections.iter().map(|s| s.angle).collect();
        assert_eq!(angles, [Some(0.0), Some(45.0), Some(90.0), Some(135.0)]);
        assert_eq!(sheet.doc.artboards[1].name, "L Compensation 45°");
        let record = sheet.doc.embroidery.sew_out.as_ref().unwrap();
        assert_eq!(record.swatches.len(), 4 * (COMP_WIDTHS.len() + 1));
        let plan = crate::plan(&sheet.doc);
        assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);
        for sw in &record.swatches {
            let angle = sw.angle.unwrap();
            let sec = sheet.sections.iter().find(|s| s.id == sw.section).unwrap();
            assert_eq!(sec.angle, Some(angle), "{}", sw.id);
            let board = sheet.doc.artboards[sec.artboard].rect;
            let shape = sheet.doc.node(sw.objects[0]).unwrap();
            let b = shape.geometric_bounds().unwrap();
            assert!(b.x0 >= board.x0 && b.x1 <= board.x1 && b.y0 >= board.y0 && b.y1 <= board.y1, "{} {b:?} in {board:?}", sw.id);
            // Sewn as drawn: no compensation, whatever the fabric.
            let s = shape.extras().stitch.as_deref().unwrap();
            assert_eq!((s.pull, s.pull_percent, s.push), (Some(0.0), Some(0.0), Some(0.0)), "{}", sw.id);
            let (w, l) = (sw.drawn_width.unwrap(), sw.drawn_length.unwrap());
            // The satin's stitches run across the column at the section's angle, as the fill's rows do.
            let (min, sewn) = match s.kind {
                StitchKind::Satin => {
                    assert_eq!((s.width, l), (Some(w), SATIN_LEN), "{}", sw.id);
                    // 8 mm is split into shorter stitches (split satin's default 7 mm), and 2 mm
                    // stitches slant by the spacing: the direction is checked on 4 and 6 mm.
                    (0.8 * w, (4.0..=6.0).contains(&w))
                }
                _ => {
                    assert_eq!((s.kind, s.angle, w, l), (StitchKind::Fill, angle, SQUARE, SQUARE), "{}", sw.id);
                    (2.0, true)
                }
            };
            if sewn {
                let d = main_direction(&plan, sw.objects[0], min);
                assert!(apart(d, angle) <= 10.0, "{}: stitches at {d}°, want {angle}°", sw.id);
            }
        }
        // The panel's names: every compensation section's angle in its title.
        let names: Vec<String> = SECTIONS.iter().filter(|s| s.angle.is_some()).map(Section::title).collect();
        assert_eq!(names, ["Compensation 0°", "Compensation 45°", "Compensation 90°", "Compensation 135°"]);
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
