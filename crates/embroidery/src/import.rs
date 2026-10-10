//! A stitch plan as editable art: one layer, a group per colour block, a manual-stitch path per
//! run of stitches (the needle points as its anchors), stroked in the thread colour. Exporting the
//! art again sews the same stitches.

use std::sync::Arc;

use kurbo::Point;
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Document, LayerColor, Node, StitchKind, StitchSettings};
use vectorcraft_geom::{Anchor, PathData, Rect, SubPath};

use crate::{Cmd, PT_MM, Plan};

/// Space around the design on its artboard, mm.
const MARGIN_MM: f64 = 10.0;
/// The stroke the thread is drawn with, mm.
const THREAD_MM: f64 = 0.4;

/// `plan` as a new document titled `title`, its artboard fitting the design with a margin.
pub fn to_document(plan: &Plan, title: &str) -> Document {
    let moves = || plan.needles.iter().filter(|n| matches!(n.cmd, Cmd::Stitch | Cmd::Jump));
    let (mut lo, mut hi) = (Point::new(f64::INFINITY, f64::INFINITY), Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
    for n in moves() {
        lo = Point::new(lo.x.min(n.x), lo.y.min(n.y));
        hi = Point::new(hi.x.max(n.x), hi.y.max(n.y));
    }
    if !(lo.x.is_finite() && hi.x.is_finite()) {
        (lo, hi) = (Point::ZERO, Point::ZERO);
    }
    let size = ((hi.x - lo.x + 2.0 * MARGIN_MM) / PT_MM, (hi.y - lo.y + 2.0 * MARGIN_MM) / PT_MM);
    let mut doc = Document::new(size.0, size.1);
    doc.title = title.to_string();
    if let Some(a) = doc.artboards.first_mut() {
        a.rect = Rect::new(0.0, 0.0, size.0, size.1);
    }
    // mm, the design's top-left at the margin → points.
    let at = |p: Point| Point::new((p.x - lo.x + MARGIN_MM) / PT_MM, (p.y - lo.y + MARGIN_MM) / PT_MM);
    let mut groups: Vec<(usize, Vec<Arc<Node>>)> = vec![];
    for (thread, run) in plan.runs() {
        let anchors = run.into_iter().map(|p| Anchor::corner(at(p))).collect();
        let rgb = plan.threads.get(thread).copied().unwrap_or([0, 0, 0]);
        let paint = Paint::solid(Color::rgb8(rgb[0], rgb[1], rgb[2]));
        let mut n = Node::path(
            doc.alloc_id(),
            PathData::single(SubPath { anchors, closed: false }),
            Appearance::basic(Paint::None, paint, THREAD_MM / PT_MM),
        );
        n.stitch = Some(Box::new(StitchSettings { thread: Some(rgb), ..StitchSettings::new(StitchKind::Manual) }));
        match groups.last_mut() {
            Some((t, v)) if *t == thread => v.push(Arc::new(n)),
            _ => groups.push((thread, vec![Arc::new(n)])),
        }
    }
    let mut children = vec![];
    for (thread, runs) in groups {
        let mut g = Node::group(doc.alloc_id(), runs);
        let [r, gr, b] = plan.threads.get(thread).copied().unwrap_or([0, 0, 0]);
        g.name = Some(format!("Thread {} #{r:02X}{gr:02X}{b:02X}", thread + 1));
        children.push(Arc::new(g));
    }
    let mut layer = Node::layer(doc.alloc_id(), "Embroidery", LayerColor::Preset(0));
    if let Some(c) = layer.children_mut() {
        *c = children;
    }
    doc.layers = vec![Arc::new(layer)];
    doc
}
