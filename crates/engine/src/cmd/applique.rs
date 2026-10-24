//! Object › Embroidery › Appliqué (`embroidery.applique`): a closed shape turned into the
//! appliqué sequence, a group of three stitched copies sewn in order in one thread. First a
//! **placement line** (a running stitch on the outline, showing where the fabric goes), then a stop
//! to place the fabric, then a **tack-down** just inside the outline holding it, then a stop to trim
//! the fabric back to the tack-down, then a **cover** over the raw edge. The group takes the shape's
//! place (and its id), so it stays where the shape was in the sewing order.

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
    vec![cmd!(
        "embroidery.applique",
        "Appliqué",
        ["Object", "Embroidery"],
        None,
        "{ids?: [closed paths or compound paths] (default: the selection), cover?: satin|zigzag|bean|none (satin: what covers the fabric's raw edge), width?: mm (1–10; 3: the satin or zigzag cover's width, centred on the outline), tackDown?: running|zigzag|bean (running: what holds the fabric down), inset?: mm (0–3; 0.8: how far inside the outline the tack-down runs)} turn each shape into an appliqué group, one undo step: a placement line on the outline (running stitch), a stop to place the fabric, the tack-down, a stop to trim the fabric, then the cover. The group (named Appliqué) takes the shape's place and id; its three parts are ordinary stitched objects in one thread (the shape's), sewn in that order. The cover must reach past the tack-down's inner side (a zigzag tack-down is 1.2 mm wide) → {groups: [{id, placement, tackDown, cover}]}",
        has_doc,
        applique
    )]
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
