//! What Stitch Preview shows, as a document any renderer can draw: the design's stitches in their
//! thread colours on fabric-coloured artboards over the art, jumps as thin lines. The desktop
//! canvas draws the same thing itself (with these colours and widths); `embroidery.preview`
//! renders this document, so agents and headless sessions see the stitches too.

use std::sync::Arc;

use kurbo::Point;
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Document, LayerColor, LineCap, LineJoin, Node};
use vectorcraft_geom::{Anchor, PathData, SubPath};

use crate::{Cmd, PT_MM, Plan};

/// The fabric drawn over the artboards (sRGB and alpha).
pub const FABRIC: [u8; 4] = [236, 230, 214, 235];
/// The thread's drawn width, mm.
pub const THREAD_MM: f64 = 0.35;
/// Jumps: black at this alpha, this wide (points).
pub const JUMP_ALPHA: u8 = 90;
pub const JUMP_WIDTH_PT: f64 = 0.5;

/// `doc` with the design `plan` drawn over its art, on a layer of its own above every other.
pub fn document(doc: &Document, plan: &Plan) -> Document {
    let mut out = doc.clone();
    let pt = |p: Point| Point::new(p.x / PT_MM, p.y / PT_MM);
    let line = |pts: Vec<Point>| PathData::single(SubPath { anchors: pts.into_iter().map(Anchor::corner).collect(), closed: false });
    let mut children: Vec<Arc<Node>> = vec![];
    let [r, g, b, a] = FABRIC;
    for ab in &doc.artboards {
        let rect = ab.rect;
        let corners = [Point::new(rect.x0, rect.y0), Point::new(rect.x1, rect.y0), Point::new(rect.x1, rect.y1), Point::new(rect.x0, rect.y1)];
        let path = PathData::single(SubPath { anchors: corners.into_iter().map(Anchor::corner).collect(), closed: true });
        let mut fabric = Node::path(out.alloc_id(), path, Appearance::basic(Paint::solid(Color::rgb8(r, g, b)), Paint::None, 0.0));
        fabric.opacity = f32::from(a) / 255.0;
        children.push(Arc::new(fabric));
    }
    let thread_pt = THREAD_MM / PT_MM;
    for (thread, run) in plan.runs() {
        let [r, g, b] = plan.threads.get(thread).copied().unwrap_or([0; 3]);
        let mut look = Appearance::basic(Paint::None, Paint::solid(Color::rgb8(r, g, b)), thread_pt);
        if let Some(s) = look.stroke_mut() {
            s.cap = LineCap::Round;
            s.join = LineJoin::Round;
        }
        children.push(Arc::new(Node::path(out.alloc_id(), line(run.into_iter().map(pt).collect()), look)));
    }
    // Each jump a thin line of its own.
    let mut from: Option<Point> = None;
    for n in &plan.needles {
        let p = pt(Point::new(n.x, n.y));
        if n.cmd == Cmd::Jump
            && let Some(f) = from
        {
            let mut jump = Node::path(out.alloc_id(), line(vec![f, p]), Appearance::basic(Paint::None, Paint::solid(Color::BLACK), JUMP_WIDTH_PT));
            jump.opacity = f32::from(JUMP_ALPHA) / 255.0;
            children.push(Arc::new(jump));
        }
        from = Some(p);
    }
    let mut layer = Node::layer(out.alloc_id(), "Stitch Preview", LayerColor::Preset(0));
    if let Some(c) = layer.children_mut() {
        *c = children;
    }
    out.layers.push(Arc::new(layer));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_doc::{NodeKind, StitchKind, StitchSettings};

    #[test]
    fn the_preview_adds_fabric_stitches_and_jumps_on_top() {
        let mut d = Document::new(300.0, 200.0);
        let mut n = Node::path(
            d.alloc_id(),
            PathData::single(SubPath {
                anchors: vec![Anchor::corner(Point::new(10.0, 10.0)), Anchor::corner(Point::new(100.0, 10.0))],
                closed: false,
            }),
            Appearance::basic(Paint::None, Paint::solid(Color::rgb8(200, 0, 0)), 1.0),
        );
        n.extras_mut().stitch = Some(Box::new(StitchSettings::new(StitchKind::Running)));
        let mut m = n.clone();
        m.id = d.alloc_id();
        if let NodeKind::Path { path, .. } = &mut m.kind {
            *path = path.transformed(kurbo::Affine::translate((0.0, 100.0)));
        }
        let layer = Arc::make_mut(&mut d.layers[0]);
        layer.children_mut().unwrap().extend([Arc::new(n), Arc::new(m)]);
        let plan = crate::plan(&d);
        let p = document(&d, &plan);
        assert_eq!(p.layers.len(), d.layers.len() + 1, "one layer on top");
        let top = p.layers.last().unwrap();
        let kids = top.children().unwrap();
        // Fabric, two runs, one jump between them.
        assert_eq!(kids.len(), 1 + 2 + 1, "{}", kids.len());
        assert!(kids[1].appearance.stroke().is_some_and(|s| s.paint == Paint::solid(Color::rgb8(200, 0, 0))));
        // Ids stay unique.
        let mut ids = std::collections::HashSet::new();
        p.walk(|n| assert!(ids.insert(n.id), "{:?}", n.id));
    }
}
