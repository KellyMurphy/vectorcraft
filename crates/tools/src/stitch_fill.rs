//! The Stitch Fill tool (auto-digitize, E8.6b): a click inside any closed area — a shape, or a
//! region where shapes and paths cross, as Live Paint finds them — adds that area as a new stitch
//! object on top, sewn Auto in the colour of the shape it lies in; Alt-click adds a running-stitch
//! outline of it instead. Each click is `embroidery.fillAt`, one undo step.

use kurbo::Point;
use serde_json::json;

use crate::{Action, Cursor, Mods, PointerEvent, PointerKind, Tool, ToolContext};

#[derive(Default)]
pub struct StitchFillTool;

impl Tool for StitchFillTool {
    fn id(&self) -> &'static str {
        "stitchFill"
    }

    fn pointer(&mut self, _cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        match ev.kind {
            PointerKind::Down => vec![Action::Exec("embroidery.fillAt".into(), json!({ "x": ev.pos.x, "y": ev.pos.y, "outline": ev.mods.alt }))],
            _ => vec![],
        }
    }

    fn cursor(&self, _cx: &ToolContext, _p: Point, _mods: Mods) -> Cursor {
        Cursor::LivePaintBucket
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use vectorcraft_doc::{Document, Selection};

    #[test]
    fn a_click_fills_the_area_and_alt_click_outlines_it() {
        let d = Document::new(500.0, 500.0);
        let (s, p) = (Selection::default(), paint());
        let c = cx(&d, &s, &p);
        let mut t = crate::create("stitchFill");
        assert_eq!(t.cursor(&c, Point::new(10.0, 10.0), Mods::default()), Cursor::LivePaintBucket);
        assert_eq!(
            t.pointer(&c, &PointerEvent::new(PointerKind::Down, 10.0, 20.0)),
            vec![Action::Exec("embroidery.fillAt".into(), json!({"x": 10.0, "y": 20.0, "outline": false}))]
        );
        let mut alt = PointerEvent::new(PointerKind::Down, 10.0, 20.0);
        alt.mods.alt = true;
        assert_eq!(t.pointer(&c, &alt), vec![Action::Exec("embroidery.fillAt".into(), json!({"x": 10.0, "y": 20.0, "outline": true}))]);
        assert!(t.pointer(&c, &PointerEvent::new(PointerKind::Up, 10.0, 20.0)).is_empty());
    }
}
