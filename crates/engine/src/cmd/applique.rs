//! Object › Embroidery › Appliqué (`embroidery.applique`): a closed shape turned into the
//! appliqué sequence, a group of three stitched copies sewn in order in one thread. First a
//! **placement line** (a running stitch on the outline, showing where the fabric goes), then a stop
//! to place the fabric, then a **tack-down** just inside the outline holding it, then a stop to trim
//! the fabric back to the tack-down, then a **cover** over the raw edge. The group takes the shape's
//! place (and its id), so it stays where the shape was in the sewing order.
//!
//! `embroidery.cutFile` exports the pieces' outlines for a fabric cutter, nested onto mats
//! ([`vectorcraft_embroidery::nest`]).

use serde_json::{Value, json};
use vectorcraft_doc::{Document, Node, NodeId, NodeKind, StitchKind, StitchSettings, Underlay};
use vectorcraft_geom::{FillRule, PathData};

use super::*;

const C: &str = "embroidery.applique";
/// The group's name, and its parts' names in sewing order.
pub const GROUP_NAME: &str = "Appliqué";
const PARTS: [&str; 3] = ["Placement Line", "Tack-Down", "Cover"];
/// Cover widths, mm.
const WIDTH: std::ops::RangeInclusive<f64> = 1.0..=10.0;
/// Tack-down insets, mm.
const INSET: std::ops::RangeInclusive<f64> = 0.0..=3.0;
const DEFAULT_WIDTH: f64 = 3.0;
const DEFAULT_INSET: f64 = 0.8;
/// A zigzag tack-down's width and spacing, mm.
const TACK_ZIGZAG: (f64, f64) = (1.2, 1.5);
/// How much of the cover reaches past the tack-down's inner side, at least, mm: the fabric is trimmed
/// close to the tack-down, and the cover must hide its cut edge.
const COVER_MARGIN: f64 = 0.3;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "embroidery.applique",
            "Appliqué",
            ["Object", "Embroidery"],
            None,
            "{ids?: [closed paths or compound paths] (default: the selection), cover?: satin|zigzag|bean|none (satin: what covers the fabric's raw edge), width?: mm (1–10; 3: the satin or zigzag cover's width, centred on the outline), tackDown?: running|zigzag|bean (running: what holds the fabric down), inset?: mm (0–3; 0.8: how far inside the outline the tack-down runs)} turn each shape into an appliqué group, one undo step: a placement line on the outline (running stitch), a stop to place the fabric, the tack-down, a stop to trim the fabric, then the cover. The group (named Appliqué) takes the shape's place and id; its three parts are ordinary stitched objects in one thread (the shape's), sewn in that order. The cover must reach past the tack-down's inner side (a zigzag tack-down is 1.2 mm wide) → {groups: [{id, placement, tackDown, cover}]}",
            has_doc,
            applique
        ),
        cut_spec(),
    ]
}

/// The choices of `key`: one of `ids`, else `default`.
fn choice(p: &Value, key: &str, ids: &[&'static str], default: &'static str) -> Result<&'static str> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => {
            let t = v.as_str().map(|t| t.trim().to_ascii_lowercase()).unwrap_or_default();
            ids.iter().copied().find(|id| *id == t).ok_or_else(|| bad(C, format!("`{key}` must be one of {}", ids.join(", "))))
        }
    }
}

/// `key` in mm within `range`, else `default`.
fn mm_param(p: &Value, key: &str, range: &std::ops::RangeInclusive<f64>, default: f64) -> Result<f64> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_f64()
            .filter(|x| range.contains(x))
            .ok_or_else(|| bad(C, format!("`{key}` must be a number of mm from {} to {}", range.start(), range.end()))),
    }
}

/// How each appliqué is made.
struct Recipe {
    cover: &'static str,
    width: f64,
    tack: &'static str,
    inset: f64,
}

impl Recipe {
    fn parse(p: &Value) -> Result<Recipe> {
        let r = Recipe {
            cover: choice(p, "cover", &["satin", "zigzag", "bean", "none"], "satin")?,
            width: mm_param(p, "width", &WIDTH, DEFAULT_WIDTH)?,
            tack: choice(p, "tackDown", &["running", "zigzag", "bean"], "running")?,
            inset: mm_param(p, "inset", &INSET, DEFAULT_INSET)?,
        };
        // A satin or zigzag cover, centred on the outline, must reach past the tack-down's inner
        // side, where the fabric is trimmed. A bean cover is a line: it covers nothing.
        if matches!(r.cover, "satin" | "zigzag") {
            let tack_inner = r.inset + if r.tack == "zigzag" { TACK_ZIGZAG.0 / 2.0 } else { 0.0 };
            let reach = r.width / 2.0;
            if reach < tack_inner + COVER_MARGIN {
                return Err(bad(
                    C,
                    format!(
                        "a {} mm cover reaches {reach} mm inside the outline, not past the tack-down at {tack_inner} mm: make it at least {} mm wide or the inset smaller",
                        r.width,
                        2.0 * (tack_inner + COVER_MARGIN)
                    ),
                ));
            }
        }
        Ok(r)
    }

    /// The settings of part `i` (0 placement, 1 tack-down, 2 cover).
    fn settings(&self, i: usize) -> StitchSettings {
        let kind = |id: &str| match id {
            "satin" => StitchKind::Satin,
            "zigzag" => StitchKind::Zigzag,
            "bean" => StitchKind::Bean,
            _ => StitchKind::Running,
        };
        match i {
            0 => StitchSettings::new(StitchKind::Running),
            1 => {
                let mut s = StitchSettings::new(kind(self.tack));
                if self.tack == "zigzag" {
                    (s.width, s.spacing) = (Some(TACK_ZIGZAG.0), Some(TACK_ZIGZAG.1));
                }
                s.stop_before = Some("placeFabric".into());
                s
            }
            _ => {
                let mut s = StitchSettings::new(kind(self.cover));
                if matches!(self.cover, "satin" | "zigzag") {
                    s.width = Some(self.width);
                }
                // Over fabric: no underlay of its own, the tack-down holds it.
                s.underlay = Underlay::None;
                s.stop_before = Some("trimFabric".into());
                s
            }
        }
    }
}

/// `n`'s outline when it is a closed path or a compound of closed paths.
fn outline(n: &Node) -> Option<(PathData, FillRule)> {
    match &n.kind {
        NodeKind::Path { path, rule, guide: false, clipping: false, .. } if path.is_closed() => Some((path.clone(), *rule)),
        NodeKind::Compound { children, rule } => {
            let parts: Vec<&PathData> = children.iter().filter_map(|c| c.path_data()).collect();
            (!parts.is_empty() && parts.iter().all(|p| p.is_closed()))
                .then(|| (PathData::new(parts.into_iter().flat_map(|p| p.subpaths.iter().cloned()).collect()), *rule))
        }
        _ => None,
    }
}

/// The appliqué group for shape `id`, replacing it: its id and its parts' ids.
fn make(d: &mut Document, id: NodeId, r: &Recipe) -> Result<(NodeId, [NodeId; 3])> {
    let shape = d.node(id).cloned().ok_or(EngineError::NoNode(id))?;
    let (path, rule) = outline(&shape).ok_or_else(|| bad(C, format!("object {} isn't a closed path or compound path", id.0)))?;
    // The tack-down, inside the outline (a negative offset insets a closed area).
    let inset = if r.inset > 0.0 {
        vectorcraft_pathops::offset_path(&path, -r.inset / vectorcraft_embroidery::PT_MM, vectorcraft_pathops::Join::Round, 4.0)
    } else {
        path.clone()
    };
    if inset.is_empty() {
        return Err(bad(C, format!("object {} is too small for a tack-down {} mm inside it", id.0, r.inset)));
    }
    let mut parts = vec![];
    let mut ids = [NodeId(0); 3];
    for (i, name) in PARTS.iter().enumerate() {
        let mut n = d.reid(&shape);
        if i == 1 {
            // A plain path: a live shape would redraw itself as the outline.
            n.kind = NodeKind::Path { path: inset.clone(), rule, live: None, clipping: false, guide: false };
        }
        n.name = Some((*name).to_string());
        n.extras_mut().stitch = Some(Box::new(r.settings(i)));
        ids[i] = n.id;
        parts.push(std::sync::Arc::new(n));
    }
    let mut g = Node::group(id, parts);
    g.name = Some(GROUP_NAME.into());
    g.visible = shape.visible;
    g.locked = shape.locked;
    *d.node_mut(id).ok_or(EngineError::NoNode(id))? = g;
    Ok((id, ids))
}

fn applique(s: &mut Session, p: &Value) -> Result<Value> {
    let r = Recipe::parse(p)?;
    let ids = targets(s, p)?;
    if ids.is_empty() {
        return Err(bad(C, "select closed paths or compound paths to make appliqué from"));
    }
    {
        let d = &s.doc()?.doc;
        if let Some(id) = ids.iter().find(|id| d.node(**id).and_then(outline).is_none()) {
            return Err(bad(C, format!("object {} isn't a closed path or compound path", id.0)));
        }
    }
    let made = s.edit("Appliqué", |d, sel| {
        let made = ids.iter().map(|id| make(d, *id, &r)).collect::<Result<Vec<_>>>()?;
        sel.set(made.iter().map(|(g, _)| *g));
        Ok(made)
    })?;
    let groups: Vec<Value> = made.iter().map(|(g, [a, b, c])| json!({"id": g.0, "placement": a.0, "tackDown": b.0, "cover": c.0})).collect();
    Ok(json!({ "groups": groups }))
}

/// Cutter mats by name: width and height, mm.
const MATS: [(&str, (f64, f64)); 2] = [("12x12", (304.8, 304.8)), ("12x24", (304.8, 609.6))];
/// Space between the mats' artboards in the new document, mm.
const MAT_GAP: f64 = 20.0;

fn cut_spec() -> CommandSpec {
    cmd!(
        "embroidery.cutFile",
        "Appliqué Cut File…",
        ["Object", "Embroidery"],
        None,
        "{ids?: [appliqué groups or closed shapes] (default: the selection, else every appliqué group), allowance?: mm (0–10; 0: how much bigger than the placement line each piece is cut, so the fabric reaches under the tack-down), gap?: mm (0–20; 3: the least distance between pieces), mat?: 12x12|12x24 (inches) | {width, height} (mm, 10–1500) (12x12), margin?: mm (0–50; 5: kept clear round the mat's edge), rotate?: none|quarter|any (quarter: how pieces may turn to fit; none for a fabric with a grain or print that must run one way; any: every 15°), flip?: false (pieces may be turned over: fabric with no right side), format?: svg|dxf (left out: a new document with one artboard per mat), path?: file to write (several mats: name-1.svg, name-2.svg…)} the appliqué pieces' cut outlines (each appliqué group's placement line, else the shape's outline, grown by the allowance) nested onto as few cutter mats as they fit on, without overlapping and the gap apart, as cut lines (hairline paths, no fill) for a fabric cutter. Pieces too big for a mat are reported, not dropped → {mats, pieces (placed), unplaced: [ids], index (the new document) | files: [paths] | files: [{mat, dataBase64}]}",
        has_doc,
        cut_file
    )
}

/// The cut outline of `n`: an appliqué group's placement line, else its own when closed.
fn cut_outline(n: &Node) -> Option<(PathData, FillRule)> {
    if is_applique(n) {
        let line = n.children()?.iter().find(|c| c.name.as_deref() == PARTS.first().copied())?;
        return outline(line);
    }
    outline(n)
}

/// Whether `n` is a group `embroidery.applique` made.
fn is_applique(n: &Node) -> bool {
    n.name.as_deref() == Some(GROUP_NAME) && n.children().is_some_and(|c| c.iter().any(|k| k.name.as_deref() == PARTS.first().copied()))
}

/// `key`'s mat: a name of [`MATS`] or `{width, height}` mm.
fn mat_param(p: &Value) -> Result<(f64, f64)> {
    let named = || format!("`mat` must be {} or {{width, height}} in mm (10–1500)", MATS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", "));
    match p.get("mat") {
        None | Some(Value::Null) => Ok(MATS[0].1),
        Some(Value::String(s)) => {
            let s = s.trim().to_ascii_lowercase().replace(['×', ' '], "x").replace("in", "");
            MATS.iter().find(|(n, _)| *n == s).map(|(_, m)| *m).ok_or_else(|| bad(CUT, named()))
        }
        Some(v) => {
            let side = |k: &str| v.get(k).and_then(Value::as_f64).filter(|x| (10.0..=vectorcraft_embroidery::nest::MAX_MAT).contains(x));
            match (side("width"), side("height")) {
                (Some(w), Some(h)) => Ok((w, h)),
                _ => Err(bad(CUT, named())),
            }
        }
    }
}

const CUT: &str = "embroidery.cutFile";

fn cut_file(s: &mut Session, p: &Value) -> Result<Value> {
    use vectorcraft_embroidery::{PT_MM, nest};
    let mm = |key: &str, range: std::ops::RangeInclusive<f64>, default: f64| -> Result<f64> {
        match p.get(key) {
            None | Some(Value::Null) => Ok(default),
            Some(v) => v
                .as_f64()
                .filter(|x| range.contains(x))
                .ok_or_else(|| bad(CUT, format!("`{key}` must be a number of mm from {} to {}", range.start(), range.end()))),
        }
    };
    let allowance = mm("allowance", 0.0..=10.0, 0.0)?;
    let gap = mm("gap", 0.0..=20.0, 3.0)?;
    let margin = mm("margin", 0.0..=50.0, 5.0)?;
    let mat = mat_param(p)?;
    let rotate = match str_param(p, "rotate").map(|r| r.trim().to_ascii_lowercase()).as_deref() {
        None | Some("quarter") => nest::Rotate::Quarter,
        Some("none") => nest::Rotate::None,
        Some("any") => nest::Rotate::Any,
        Some(_) => return Err(bad(CUT, "`rotate` must be none, quarter or any")),
    };
    let flip = bool_or(p, "flip", false);
    let format = match str_param(p, "format").map(|f| f.trim().to_ascii_lowercase()) {
        None => None,
        Some(f) if f == "svg" || f == "dxf" => Some(f),
        Some(_) => return Err(bad(CUT, "`format` must be svg or dxf (or left out: a new document)")),
    };
    let doc = &s.doc()?.doc;
    // The pieces: those given or selected, else every appliqué group.
    let mut ids = targets(s, p)?;
    if ids.is_empty() {
        doc.walk(|n| {
            if is_applique(n) {
                ids.push(n.id);
            }
        });
    }
    if ids.is_empty() {
        return Err(bad(CUT, "select appliqué groups or closed shapes (none is in the document)"));
    }
    let mut outlines: Vec<(NodeId, PathData)> = vec![];
    for id in &ids {
        let n = doc.node(*id).ok_or(EngineError::NoNode(*id))?;
        let (path, _) = cut_outline(n).ok_or_else(|| bad(CUT, format!("object {} isn't an appliqué group or a closed shape", id.0)))?;
        let grown =
            if allowance > 0.0 { vectorcraft_pathops::offset_path(&path, allowance / PT_MM, vectorcraft_pathops::Join::Round, 4.0) } else { path };
        outlines.push((*id, grown));
    }
    let rings: Vec<Vec<Vec<kurbo::Point>>> = outlines
        .iter()
        .map(|(_, path)| vectorcraft_embroidery::generate::polylines(path).into_iter().filter(|(_, closed)| *closed).map(|(l, _)| l).collect())
        .collect();
    let nested = nest::nest(&rings, &nest::Options { mat, margin, gap, rotate, flip });
    let unplaced: Vec<u64> = nested.unplaced.iter().filter_map(|i| outlines.get(*i)).map(|(id, _)| id.0).collect();
    if nested.placements.is_empty() {
        return Err(bad(CUT, format!("no piece fits on a {:.0} × {:.0} mm mat with a {margin} mm margin", mat.0, mat.1)));
    }
    let to_mm = kurbo::Affine::scale(PT_MM);
    let to_pt = kurbo::Affine::scale(1.0 / PT_MM);
    let ink = vectorcraft_color::Paint::solid(vectorcraft_color::Color::rgb8(0, 0, 0));
    // A mat's pieces, each moved onto it by `left` mm, into `d`.
    let pieces_on = |d: &mut Document, m: usize, left: f64| -> Vec<std::sync::Arc<Node>> {
        nested
            .placements
            .iter()
            .filter(|pl| pl.mat == m)
            .filter_map(|pl| {
                let (id, path) = outlines.get(pl.piece)?;
                let a = to_pt * kurbo::Affine::translate((left, 0.0)) * pl.transform * to_mm;
                let look = vectorcraft_doc::Appearance::basic(vectorcraft_color::Paint::None, ink.clone(), 0.1 / PT_MM);
                let mut n = Node::path(d.alloc_id(), path.transformed(a), look);
                n.name = Some(format!("Piece {}", id.0));
                Some(std::sync::Arc::new(n))
            })
            .collect()
    };
    let board = |index: usize, left: f64| vectorcraft_doc::Artboard {
        id: u32::try_from(index + 1).unwrap_or(u32::MAX),
        name: format!("Mat {}", index + 1),
        rect: vectorcraft_geom::Rect::new(left / PT_MM, 0.0, (left + mat.0) / PT_MM, mat.1 / PT_MM),
        show_center_mark: false,
        show_cross_hairs: false,
        locked: false,
        show_video_safe_areas: false,
        video_ruler_pixel_aspect_ratio: 1.0,
        color: None,
    };
    let title = format!("{} cut file", doc.title);
    let new_doc = |boards: &[usize]| -> Document {
        let mut d = Document::new(mat.0 / PT_MM, mat.1 / PT_MM);
        d.title.clone_from(&title);
        d.units = vectorcraft_doc::Unit::Millimeters;
        d.artboards.clear();
        let mut nodes = vec![];
        for (k, m) in boards.iter().enumerate() {
            let left = k as f64 * (mat.0 + MAT_GAP);
            nodes.extend(pieces_on(&mut d, *m, left));
            d.artboards.push(board(k, left));
        }
        let mut layer = Node::layer(d.alloc_id(), "Cut", vectorcraft_doc::LayerColor::Preset(0));
        if let Some(c) = layer.children_mut() {
            *c = nodes;
        }
        d.layers = vec![std::sync::Arc::new(layer)];
        d
    };
    let mut out = json!({"mats": nested.mats, "pieces": nested.placements.len(), "unplaced": unplaced});
    match format {
        None => {
            let all: Vec<usize> = (0..nested.mats).collect();
            let d = new_doc(&all);
            out["index"] = json!(s.add_document(d, None));
        }
        Some(f) => {
            let mut files = vec![];
            for m in 0..nested.mats {
                let bytes = super::fileio::encode(&new_doc(&[m]), &f, &json!({}))?;
                match str_param(p, "path") {
                    Some(path) => {
                        let path = if nested.mats == 1 {
                            path.to_string()
                        } else {
                            let pp = std::path::Path::new(path);
                            let stem = pp.file_stem().map_or_else(|| "cut".into(), |x| x.to_string_lossy().to_string());
                            pp.with_file_name(format!("{stem}-{}.{f}", m + 1)).to_string_lossy().to_string()
                        };
                        super::fileio::write_file(&path, &bytes)?;
                        files.push(json!(path));
                    }
                    None => files.push(json!({"mat": m + 1, "dataBase64": vectorcraft_format::base64_encode(&bytes)})),
                }
            }
            out["files"] = json!(files);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A session with a 100 pt square, selected, and its id.
    fn session_with_square() -> (Session, NodeId) {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 300, "height": 300})).unwrap();
        let r = s.execute("shape.rectangle", &json!({"x": 20, "y": 20, "width": 100, "height": 100})).unwrap();
        (s, NodeId(r["id"].as_u64().unwrap()))
    }

    #[test]
    fn a_shape_becomes_the_applique_sequence() {
        let (mut s, shape) = session_with_square();
        let r = s.execute(C, &json!({})).unwrap();
        let g = &r["groups"][0];
        assert_eq!(g["id"], shape.0, "the group takes the shape's id");
        let d = &s.doc().unwrap().doc;
        let group = d.node(shape).unwrap();
        assert_eq!(group.name.as_deref(), Some(GROUP_NAME));
        let kids = group.children().unwrap();
        let names: Vec<_> = kids.iter().map(|k| k.name.clone().unwrap()).collect();
        assert_eq!(names, PARTS);
        let st = |i: usize| kids[i].extras().stitch.as_deref().cloned().unwrap();
        assert_eq!((st(0).kind, st(0).stop_before), (StitchKind::Running, None));
        assert_eq!((st(1).kind, st(1).stop_before.as_deref()), (StitchKind::Running, Some("placeFabric")));
        assert_eq!((st(2).kind, st(2).width, st(2).stop_before.as_deref()), (StitchKind::Satin, Some(3.0), Some("trimFabric")));
        // The tack-down runs inside the outline.
        let (a, b) = (kids[0].geometric_bounds().unwrap(), kids[1].geometric_bounds().unwrap());
        let inset_pt = DEFAULT_INSET / vectorcraft_embroidery::PT_MM;
        assert!((b.x0 - a.x0 - inset_pt).abs() < 0.05 && (a.x1 - b.x1 - inset_pt).abs() < 0.05, "{a:?} {b:?}");
        // Sewn in that order, one thread, with the two stops.
        let info = s.execute("embroidery.info", &json!({})).unwrap();
        assert_eq!(info["design"]["stops"], json!([{"block": 1, "kind": "placeFabric"}, {"block": 2, "kind": "trimFabric"}]));
        assert_eq!(info["design"]["threads"].as_array().unwrap().len(), 3);
        assert!(info["design"]["threads"].as_array().unwrap().windows(2).all(|w| w[0] == w[1]));
        // One undo step back to the shape.
        s.execute("edit.undo", &json!({})).unwrap();
        assert!(s.doc().unwrap().doc.node(shape).unwrap().children().is_none());
    }

    #[test]
    fn cut_files_nest_the_pieces_onto_mats() {
        let (mut s, square) = session_with_square();
        s.execute(C, &json!({})).unwrap();
        let r = s.execute("shape.ellipse", &json!({"x": 150, "y": 150, "width": 80, "height": 60})).unwrap();
        let oval = r["id"].as_u64().unwrap();
        s.execute(C, &json!({"ids": [oval]})).unwrap();
        s.execute("edit.deselectAll", &json!({})).ok();
        // Nothing selected: every appliqué group, on one mat, in a new document.
        let r = s.execute(CUT, &json!({"ids": []})).unwrap();
        assert_eq!((r["mats"].as_u64(), r["pieces"].as_u64()), (Some(1), Some(2)), "{r}");
        let index = r["index"].as_u64().unwrap() as usize;
        let cut = &s.documents()[index].doc;
        assert_eq!(cut.artboards.len(), 1);
        let mut pieces = vec![];
        cut.walk(|n| {
            if n.path_data().is_some() {
                pieces.push(n.geometric_bounds().unwrap());
            }
        });
        assert_eq!(pieces.len(), 2);
        // The 100 pt square, grown by nothing, on the mat inside its margin.
        let board = cut.artboards[0].rect;
        let margin = 5.0 / vectorcraft_embroidery::PT_MM;
        assert!(pieces.iter().all(|b| b.x0 >= board.x0 + margin - 1e-6 && b.y0 >= board.y0 + margin - 1e-6 && b.x1 <= board.x1 && b.y1 <= board.y1));
        assert!(pieces.iter().any(|b| ((b.x1 - b.x0) - 100.0).abs() < 0.01 && ((b.y1 - b.y0) - 100.0).abs() < 0.01), "{pieces:?}");
        // An allowance grows each piece all round.
        s.set_active(0);
        let r = s.execute(CUT, &json!({"ids": [square.0], "allowance": 2})).unwrap();
        let cut = &s.documents()[r["index"].as_u64().unwrap() as usize].doc;
        let mut w = 0.0;
        cut.walk(|n| {
            if let Some(b) = n.path_data().and(n.geometric_bounds()) {
                w = b.x1 - b.x0;
            }
        });
        assert!((w - (100.0 + 4.0 / vectorcraft_embroidery::PT_MM)).abs() < 0.05, "{w}");
        s.set_active(0);
        // SVG and DXF files.
        let dir = std::env::temp_dir().join(format!("vc-cutfile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cut.svg").to_string_lossy().to_string();
        let r = s.execute(CUT, &json!({"ids": [], "format": "svg", "path": path})).unwrap();
        assert_eq!(r["files"], json!([path]));
        assert!(std::fs::read_to_string(&path).unwrap().contains("<path"));
        let r = s.execute(CUT, &json!({"ids": [], "format": "dxf"})).unwrap();
        let dxf = vectorcraft_format::base64_decode(r["files"][0]["dataBase64"].as_str().unwrap()).unwrap();
        assert!(String::from_utf8_lossy(&dxf).contains("ENTITIES"));
        // Small mats: one piece each, written as name-1, name-2.
        let path = dir.join("small.svg").to_string_lossy().to_string();
        let r = s.execute(CUT, &json!({"ids": [], "mat": {"width": 60, "height": 60}, "margin": 2, "format": "svg", "path": path})).unwrap();
        assert_eq!((r["mats"].as_u64(), r["unplaced"].clone()), (Some(2), json!([])), "{r}");
        let small = |k: u32| dir.join(format!("small-{k}.svg")).to_string_lossy().to_string();
        assert_eq!(r["files"], json!([small(1), small(2)]));
        // Too big for any mat: reported.
        let r = s.execute(CUT, &json!({"ids": [], "mat": {"width": 30, "height": 30}, "margin": 0})).unwrap();
        assert_eq!(r["unplaced"], json!([square.0]), "{r}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_cover_must_hide_the_tack_down() {
        let (mut s, _) = session_with_square();
        assert!(s.execute(C, &json!({"width": 1.5, "inset": 0.8})).is_err());
        assert!(s.execute(C, &json!({"width": 2.0, "inset": 0.8, "tackDown": "zigzag"})).is_err());
        // A bean cover is a line: any inset.
        assert!(s.execute(C, &json!({"cover": "bean", "inset": 2.0})).is_ok());
    }

    #[test]
    fn only_closed_shapes_are_made_applique() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 300, "height": 300})).unwrap();
        s.execute("shape.line", &json!({"x1": 0, "y1": 0, "x2": 100, "y2": 0})).unwrap();
        assert!(s.execute(C, &json!({})).is_err());
        let (mut s, _) = session_with_square();
        assert!(s.execute(C, &json!({"cover": "fringe"})).is_err());
        assert!(s.execute(C, &json!({"width": 20})).is_err());
        // Too small for its tack-down.
        s.execute("shape.rectangle", &json!({"x": 200, "y": 200, "width": 2, "height": 2})).unwrap();
        assert!(s.execute(C, &json!({"inset": 3, "width": 10})).is_err());
    }
}
