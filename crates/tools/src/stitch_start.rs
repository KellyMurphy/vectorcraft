//! Stitch start handles: with Stitch Preview on, a selected running, bean or zigzag object shows
//! where its stitching starts: a filled dot at its Start At point, or a hollow one at its path's
//! first point while the start is automatic (nearest where the needle is). The Selection tool drags
//! the dot along the path to set `startAt` (`embroidery.set`, one undo step); a double-click puts
//! the start back to automatic.

use kurbo::{BezPath, ParamCurveNearest, Point};
use serde_json::json;
use vectorcraft_doc::{NodeId, NodeKind, StitchKind};

use crate::{Action, Overlay, PointerEvent, ToolContext};

/// Millimetres per point (`startAt` is in mm).
const PT_MM: f64 = 25.4 / 72.0;
/// The handles' colour (RGB).
pub const COLOR: [u8; 3] = [0x2e, 0xa0, 0x43];
/// The handles' size on screen, pixels across.
const SIZE: f32 = 10.0;

/// One object's start handle, in document points.
#[derive(Clone, Debug, PartialEq)]
pub struct StartHandle {
    pub id: NodeId,
    pub at: Point,
    /// Set by Start At (else automatic).
    pub fixed: bool,
    /// The object's paths, which the handle slides along.
    path: BezPath,
}

/// The selection's start handles (none unless Stitch Preview is on).
pub fn handles(cx: &ToolContext) -> Vec<StartHandle> {
    if !cx.stitch_preview {
        return vec![];
    }
    let mut out = vec![];
    for id in &cx.selection.objects {
        let Some(n) = cx.doc.node(*id) else { continue };
        let Some(s) = n.extras().stitch.as_deref().filter(|s| s.unknown.is_none()) else { continue };
        if !matches!(s.kind, StitchKind::Running | StitchKind::Bean | StitchKind::Zigzag) {
            continue;
        }
        let path = match &n.kind {
            NodeKind::Path { path, .. } => path.to_bezpath(),
            NodeKind::Compound { children, .. } => {
                let mut bp = BezPath::new();
                for c in children.iter().filter_map(|c| c.path_data()) {
                    bp.extend(c.to_bezpath());
                }
                bp
            }
            _ => continue,
        };
        let start = s.start_at.filter(|p| p.iter().all(|v| v.is_finite())).map(|[x, y]| Point::new(x / PT_MM, y / PT_MM));
        let at = match start {
            // Shown where it is sewn: the path's point nearest the Start At point.
            Some(p) => nearest(&path, p).unwrap_or(p),
            None => {
                let Some(first) = path.elements().first().and_then(|e| e.end_point()) else { continue };
                first
            }
        };
        out.push(StartHandle { id: *id, at, fixed: start.is_some(), path });
    }
    out
}

/// The point of `path` nearest `p`.
fn nearest(path: &BezPath, p: Point) -> Option<Point> {
    path.segments().map(|seg| seg.eval(seg.nearest(p, 1e-3).t)).min_by(|a, b| a.distance(p).total_cmp(&b.distance(p)))
}

use kurbo::ParamCurve;

/// The handle under `p`, if any.
fn hit(cx: &ToolContext, p: Point) -> Option<StartHandle> {
    let tol = cx.tol(6.0);
    handles(cx).into_iter().filter(|h| h.at.distance(p) <= tol).min_by(|a, b| a.at.distance(p).total_cmp(&b.at.distance(p)))
}

/// Is `p` over a start handle?
pub fn over_handle(cx: &ToolContext, p: Point) -> bool {
    hit(cx, p).is_some()
}

/// A double-click on a handle: back to an automatic start.
pub fn double_click(cx: &ToolContext, p: Point) -> Option<Action> {
    let h = hit(cx, p)?;
    h.fixed.then(|| Action::Exec("embroidery.set".into(), json!({ "ids": [h.id.0], "startAt": null })))
}

/// Dragging a start handle along its path.
#[derive(Clone, Debug)]
pub struct StartDrag {
    handle: StartHandle,
    start: Point,
    began: bool,
    at: Point,
}

impl StartDrag {
    /// Start a drag when the press `ev` is on a start handle.
    pub fn hit(cx: &ToolContext, ev: &PointerEvent) -> Option<Self> {
        let handle = hit(cx, ev.pos)?;
        let at = handle.at;
        Some(Self { handle, start: ev.pos, began: false, at })
    }

    pub fn drag(&mut self, cx: &ToolContext, p: Point) -> Vec<Action> {
        let mut out = vec![];
        if !self.began {
            if p.distance(self.start) < cx.tol(3.0) {
                return out;
            }
            self.began = true;
            out.push(Action::Begin("Stitch Start".into()));
        }
        self.at = nearest(&self.handle.path, p).unwrap_or(p);
        let mm = [(self.at.x * PT_MM * 1000.0).round() / 1000.0, (self.at.y * PT_MM * 1000.0).round() / 1000.0];
        out.push(Action::Preview("embroidery.set".into(), json!({ "ids": [self.handle.id.0], "startAt": mm })));
        out
    }

    pub fn finish(self) -> Vec<Action> {
        if self.began { vec![Action::Commit] } else { vec![] }
    }

    /// The handle where the drag has it.
    pub fn overlays(&self) -> Vec<Overlay> {
        vec![Overlay::CurvePoint { p: self.at, color: COLOR, filled: true, size: SIZE }]
    }
}

/// The start handles, drawn (the one being dragged by its drag instead).
pub fn overlays(cx: &ToolContext, dragging: Option<NodeId>) -> Vec<Overlay> {
    handles(cx)
        .into_iter()
        .filter(|h| Some(h.id) != dragging)
        .map(|h| Overlay::CurvePoint { p: h.at, color: COLOR, filled: h.fixed, size: SIZE })
        .collect()
}

impl StartDrag {
    /// The object whose start is being dragged.
    pub fn id(&self) -> NodeId {
        self.handle.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use crate::{Cursor, PointerKind};
    use vectorcraft_doc::{Appearance, Document, Node, Selection, StitchSettings};
    use vectorcraft_geom::{Anchor, PathData, SubPath};

    /// A running-stitch line from (100, 100) to (300, 100), selected.
    fn line(kind: StitchKind, start_at: Option<[f64; 2]>) -> (Document, NodeId) {
        let mut d = Document::new(500.0, 500.0);
        let l = d.layers[0].id;
        let id = d.alloc_id();
        let path = PathData::single(SubPath {
            anchors: vec![Anchor::corner(Point::new(100.0, 100.0)), Anchor::corner(Point::new(300.0, 100.0))],
            closed: false,
        });
        let mut n = Node::path(id, path, Appearance::default_art());
        n.extras_mut().stitch = Some(Box::new(StitchSettings { start_at, ..StitchSettings::new(kind) }));
        d.insert(Some(l), 0, n).unwrap();
        (d, id)
    }

    fn on(d: &Document, id: NodeId) -> (Selection, crate::PaintDefaults) {
        let mut s = Selection::default();
        s.objects.push(id);
        let _ = d;
        (s, paint())
    }

    #[test]
    fn handles_show_with_stitch_preview_on_selected_lines() {
        let (d, id) = line(StitchKind::Running, None);
        let (s, p) = on(&d, id);
        assert!(handles(&cx(&d, &s, &p)).is_empty(), "off without Stitch Preview");
        let c = ToolContext { stitch_preview: true, ..cx(&d, &s, &p) };
        let h = handles(&c);
        assert_eq!(h.len(), 1);
        assert_eq!((h[0].at, h[0].fixed), (Point::new(100.0, 100.0), false), "automatic: hollow, at the first point");
        // A Start At point shows where it is sewn: the path's nearest point.
        let (d, id) = line(StitchKind::Bean, Some([200.0 * PT_MM, 110.0 * PT_MM]));
        let (s, p) = on(&d, id);
        let c = ToolContext { stitch_preview: true, ..cx(&d, &s, &p) };
        let h = handles(&c);
        assert!(h[0].fixed && h[0].at.distance(Point::new(200.0, 100.0)) < 1e-6, "{h:?}");
        // Fills have no handle.
        let (d, id) = line(StitchKind::Fill, None);
        let (s, p) = on(&d, id);
        assert!(handles(&ToolContext { stitch_preview: true, ..cx(&d, &s, &p) }).is_empty());
    }

    #[test]
    fn the_selection_tool_drags_the_start_along_the_path() {
        let (d, id) = line(StitchKind::Running, None);
        let (s, p) = on(&d, id);
        let c = ToolContext { stitch_preview: true, ..cx(&d, &s, &p) };
        let mut t = crate::create("selection");
        assert_eq!(t.cursor(&c, Point::new(100.0, 100.0), Default::default()), Cursor::ArrowHollow);
        assert!(t.pointer(&c, &PointerEvent::new(PointerKind::Down, 100.0, 100.0)).is_empty());
        // Dragged to (250, 130): the start lands on the path at (250, 100).
        let a = t.pointer(&c, &PointerEvent::new(PointerKind::Drag, 250.0, 130.0));
        let mm = |v: f64| (v * PT_MM * 1000.0).round() / 1000.0;
        assert_eq!(
            a,
            vec![
                Action::Begin("Stitch Start".into()),
                Action::Preview("embroidery.set".into(), json!({"ids": [id.0], "startAt": [mm(250.0), mm(100.0)]}))
            ]
        );
        assert_eq!(t.pointer(&c, &PointerEvent::new(PointerKind::Up, 250.0, 130.0)), vec![Action::Commit]);
    }

    #[test]
    fn a_double_click_makes_the_start_automatic_again() {
        let (d, id) = line(StitchKind::Running, Some([150.0 * PT_MM, 100.0 * PT_MM]));
        let (s, p) = on(&d, id);
        let c = ToolContext { stitch_preview: true, ..cx(&d, &s, &p) };
        let a = crate::create("selection").pointer(&c, &PointerEvent::new(PointerKind::DoubleClick, 150.0, 100.0));
        assert_eq!(a, vec![Action::Exec("embroidery.set".into(), json!({"ids": [id.0], "startAt": null}))]);
    }
}
