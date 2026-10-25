//! The Stitch Angle tool: construction lines across a satin (a column, a satin shape, Auto) that
//! set its stitches' direction where they cross it (`StitchSettings::angles`); between lines the
//! direction turns gradually from one to the next.
//!
//! With a satin selected, a drag across it draws a line (Shift: in 45° steps); dragging a line's
//! end moves it; Alt-click or double-click on a line removes it. With nothing suitable selected,
//! a click on a satin selects it. Each change is `embroidery.set {angles}`, one undo step.

use kurbo::{Line as Seg, ParamCurveNearest, Point};
use serde_json::json;
use vectorcraft_doc::{Node, NodeId, StitchKind};

use crate::{Action, Cursor, Mods, Overlay, PointerEvent, PointerKind, Tool, ToolContext, ToolKey};

/// Millimetres per point (angle lines are in mm).
const PT_MM: f64 = 25.4 / 72.0;
/// The lines' colour (RGB).
pub const COLOR: [u8; 3] = [0xe0, 0x7a, 0x10];
/// The ends' size on screen, pixels.
const END: f32 = 7.0;

/// Whether `n` sews satin the lines can turn.
pub fn takes_angles(n: &Node) -> bool {
    n.extras()
        .stitch
        .as_deref()
        .is_some_and(|s| s.unknown.is_none() && matches!(s.kind, StitchKind::Satin | StitchKind::SatinShape | StitchKind::Auto))
}

/// The selected satins and their lines (mm).
fn targets(cx: &ToolContext) -> Vec<(NodeId, Vec<[f64; 4]>)> {
    cx.selection
        .objects
        .iter()
        .filter_map(|id| {
            let n = cx.doc.node(*id).filter(|n| takes_angles(n))?;
            let lines = n.extras().stitch.as_deref().map(|s| s.angles.clone()).unwrap_or_default();
            Some((*id, lines))
        })
        .collect()
}

/// A line's ends in document points.
fn ends(l: &[f64; 4]) -> [Point; 2] {
    [Point::new(l[0] / PT_MM, l[1] / PT_MM), Point::new(l[2] / PT_MM, l[3] / PT_MM)]
}

/// A line from `a` to `b` (points) as stored (mm, to a thousandth).
fn stored(a: Point, b: Point) -> [f64; 4] {
    let r = |v: f64| (v * PT_MM * 1000.0).round() / 1000.0;
    [r(a.x), r(a.y), r(b.x), r(b.y)]
}

fn set(id: NodeId, lines: &[[f64; 4]]) -> (String, serde_json::Value) {
    ("embroidery.set".into(), json!({ "ids": [id.0], "angles": lines }))
}

/// A line's end found under the pointer: its object, the object's lines, the line and which end.
type EndHit = (NodeId, Vec<[f64; 4]>, usize, usize);
/// A line found under the pointer: its object, the object's lines and the line.
type LineHit = (NodeId, Vec<[f64; 4]>, usize);

/// The end of a line under `p`.
fn end_at(cx: &ToolContext, p: Point) -> Option<EndHit> {
    let tol = cx.point_tol();
    let mut best: Option<(f64, EndHit)> = None;
    for (id, lines) in targets(cx) {
        for (i, l) in lines.iter().enumerate() {
            for (e, q) in ends(l).into_iter().enumerate() {
                let d = q.distance(p);
                if d <= tol && best.as_ref().is_none_or(|(b, _)| d < *b) {
                    best = Some((d, (id, lines.clone(), i, e)));
                }
            }
        }
    }
    best.map(|(_, h)| h)
}

/// The line under `p`.
fn line_at(cx: &ToolContext, p: Point) -> Option<LineHit> {
    let tol = cx.tol(5.0);
    let mut best: Option<(f64, LineHit)> = None;
    for (id, lines) in targets(cx) {
        for (i, l) in lines.iter().enumerate() {
            let [a, b] = ends(l);
            let d = Seg::new(a, b).nearest(p, 1e-6).distance_sq.sqrt();
            if d <= tol && best.as_ref().is_none_or(|(b, _)| d < *b) {
                best = Some((d, (id, lines.clone(), i)));
            }
        }
    }
    best.map(|(_, h)| h)
}

/// The satin under `p` (to select it).
fn satin_at(cx: &ToolContext, p: Point) -> Option<NodeId> {
    let hit = vectorcraft_doc::hit::hit_test(cx.doc, p, cx.hit_options())?;
    cx.doc.node(hit.leaf).filter(|n| takes_angles(n)).map(|n| n.id)
}

/// `to` from `from`, in 45° steps when `snap`.
fn constrained(from: Point, to: Point, snap: bool) -> Point {
    if !snap {
        return to;
    }
    let d = to - from;
    let step = std::f64::consts::FRAC_PI_4;
    let a = (d.y.atan2(d.x) / step).round() * step;
    from + kurbo::Vec2::new(a.cos(), a.sin()) * d.hypot()
}

#[derive(Clone, Debug)]
enum Drag {
    /// Drawing a new line on `id`.
    New { id: NodeId, lines: Vec<[f64; 4]>, from: Point, to: Point, began: bool },
    /// Moving end `end` of line `index` of `id`.
    End { id: NodeId, lines: Vec<[f64; 4]>, index: usize, end: usize, start: Point, began: bool },
    /// A press that may become a click (selecting a satin).
    Click { at: Point },
}

#[derive(Default)]
pub struct StitchAngleTool {
    drag: Option<Drag>,
}

impl Tool for StitchAngleTool {
    fn id(&self) -> &'static str {
        "stitchAngle"
    }

    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        let p = ev.pos;
        match ev.kind {
            PointerKind::Down => {
                if ev.mods.alt
                    && let Some((id, mut lines, i)) = line_at(cx, p)
                {
                    lines.remove(i);
                    let (c, v) = set(id, &lines);
                    return vec![Action::Exec(c, v)];
                }
                if let Some((id, lines, index, end)) = end_at(cx, p) {
                    self.drag = Some(Drag::End { id, lines, index, end, start: p, began: false });
                    return vec![];
                }
                let t = targets(cx);
                // The line goes to the selected satin under the press, else the first one.
                let under = satin_at(cx, p).filter(|id| t.iter().any(|(t, _)| t == id));
                match under.or_else(|| t.first().map(|(id, _)| *id)) {
                    Some(id) => {
                        let lines = t.into_iter().find(|(t, _)| *t == id).map(|(_, l)| l).unwrap_or_default();
                        self.drag = Some(Drag::New { id, lines, from: p, to: p, began: false });
                    }
                    None => self.drag = Some(Drag::Click { at: p }),
                }
                vec![]
            }
            PointerKind::Drag => {
                let tol = cx.tol(3.0);
                match &mut self.drag {
                    Some(Drag::New { id, lines, from, to, began }) => {
                        if !*began && p.distance(*from) < tol {
                            return vec![];
                        }
                        let mut out = vec![];
                        if !*began {
                            *began = true;
                            out.push(Action::Begin("Stitch Angle".into()));
                        }
                        *to = constrained(*from, p, ev.mods.shift);
                        let mut all = lines.clone();
                        all.push(stored(*from, *to));
                        let (c, v) = set(*id, &all);
                        out.push(Action::Preview(c, v));
                        out
                    }
                    Some(Drag::End { id, lines, index, end, start, began }) => {
                        if !*began && p.distance(*start) < tol {
                            return vec![];
                        }
                        let mut out = vec![];
                        if !*began {
                            *began = true;
                            out.push(Action::Begin("Stitch Angle".into()));
                        }
                        let mut all = lines.clone();
                        if let Some(l) = all.get_mut(*index) {
                            let [a, b] = ends(l);
                            let (a, b) = if *end == 0 { (constrained(b, p, ev.mods.shift), b) } else { (a, constrained(a, p, ev.mods.shift)) };
                            *l = stored(a, b);
                        }
                        let (c, v) = set(*id, &all);
                        out.push(Action::Preview(c, v));
                        out
                    }
                    _ => vec![],
                }
            }
            PointerKind::Up => match self.drag.take() {
                Some(Drag::New { began: true, .. } | Drag::End { began: true, .. }) => vec![Action::Commit],
                // A click: select the satin under it (one not selected yet).
                Some(Drag::Click { at } | Drag::New { from: at, began: false, .. }) => match satin_at(cx, at) {
                    Some(id) if !cx.selection.objects.contains(&id) => vec![Action::Exec("select.set".into(), json!({ "ids": [id.0] }))],
                    _ => vec![],
                },
                _ => vec![],
            },
            PointerKind::DoubleClick => match line_at(cx, p) {
                Some((id, mut lines, i)) => {
                    self.drag = None;
                    lines.remove(i);
                    let (c, v) = set(id, &lines);
                    vec![Action::Exec(c, v)]
                }
                None => vec![],
            },
            _ => vec![],
        }
    }

    fn key(&mut self, _cx: &ToolContext, key: ToolKey, _mods: Mods) -> Vec<Action> {
        match (key, self.drag.take()) {
            (ToolKey::Escape, Some(Drag::New { began: true, .. } | Drag::End { began: true, .. })) => vec![Action::Cancel],
            _ => vec![],
        }
    }

    fn busy(&self) -> bool {
        matches!(self.drag, Some(Drag::New { began: true, .. } | Drag::End { began: true, .. }))
    }

    fn overlays(&self, cx: &ToolContext) -> Vec<Overlay> {
        let mut out = vec![];
        for (_, lines) in targets(cx) {
            for l in &lines {
                let [a, b] = ends(l);
                out.push(Overlay::Line { a, b, color: COLOR, dashed: false });
                for p in [a, b] {
                    out.push(Overlay::Anchor { p, color: COLOR, filled: true, size: END });
                }
            }
        }
        if let Some(Drag::New { from, to, began: true, .. }) = &self.drag {
            out.push(Overlay::Line { a: *from, b: *to, color: COLOR, dashed: true });
        }
        out
    }

    fn cursor(&self, cx: &ToolContext, p: Point, mods: Mods) -> Cursor {
        if mods.alt && line_at(cx, p).is_some() {
            return Cursor::PenDelete;
        }
        if end_at(cx, p).is_some() {
            return Cursor::Move;
        }
        if !targets(cx).is_empty() {
            return Cursor::Crosshair;
        }
        if satin_at(cx, p).is_some() { Cursor::Arrow } else { Cursor::NotAllowed }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use vectorcraft_doc::{Appearance, Document, Selection, StitchSettings};
    use vectorcraft_geom::{Anchor, PathData, SubPath};

    /// A satin column from (100, 100) to (300, 100), 10 pt wide, its angle lines `angles`.
    fn column(angles: Vec<[f64; 4]>) -> (Document, NodeId) {
        let mut d = Document::new(500.0, 500.0);
        let l = d.layers[0].id;
        let id = d.alloc_id();
        let path = PathData::single(SubPath {
            anchors: vec![Anchor::corner(Point::new(100.0, 100.0)), Anchor::corner(Point::new(300.0, 100.0))],
            closed: false,
        });
        let ap = Appearance::basic(vectorcraft_color::Paint::None, vectorcraft_color::Paint::solid(vectorcraft_color::Color::BLACK), 10.0);
        let mut n = Node::path(id, path, ap);
        n.extras_mut().stitch = Some(Box::new(StitchSettings { angles, ..StitchSettings::new(StitchKind::Satin) }));
        d.insert(Some(l), 0, n).unwrap();
        (d, id)
    }

    fn selected(id: NodeId) -> Selection {
        let mut s = Selection::default();
        s.objects.push(id);
        s
    }

    fn ev(kind: PointerKind, x: f64, y: f64) -> PointerEvent {
        PointerEvent::new(kind, x, y)
    }

    #[test]
    fn a_drag_across_a_selected_satin_draws_a_line() {
        let (d, id) = column(vec![]);
        let (s, p) = (selected(id), paint());
        let c = cx(&d, &s, &p);
        let mut t = crate::create("stitchAngle");
        assert_eq!(t.cursor(&c, Point::new(200.0, 90.0), Mods::default()), Cursor::Crosshair);
        assert!(t.pointer(&c, &ev(PointerKind::Down, 190.0, 90.0)).is_empty());
        let a = t.pointer(&c, &ev(PointerKind::Drag, 210.0, 110.0));
        assert_eq!(
            a,
            vec![
                Action::Begin("Stitch Angle".into()),
                Action::Preview(
                    "embroidery.set".into(),
                    json!({"ids": [id.0], "angles": [stored(Point::new(190.0, 90.0), Point::new(210.0, 110.0))]})
                )
            ]
        );
        assert_eq!(t.pointer(&c, &ev(PointerKind::Up, 210.0, 110.0)), vec![Action::Commit]);
    }

    #[test]
    fn shift_draws_in_45_degree_steps() {
        let (d, id) = column(vec![]);
        let (s, p) = (selected(id), paint());
        let c = cx(&d, &s, &p);
        let mut t = crate::create("stitchAngle");
        t.pointer(&c, &ev(PointerKind::Down, 200.0, 80.0));
        let mut e = ev(PointerKind::Drag, 203.0, 120.0);
        e.mods.shift = true;
        let a = t.pointer(&c, &e);
        let Some(Action::Preview(_, v)) = a.last() else { panic!("{a:?}") };
        let l = v["angles"][0].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect::<Vec<_>>();
        assert!((l[0] - l[2]).abs() < 1e-3, "straight down: {l:?}");
    }

    #[test]
    fn a_lines_end_drags_and_alt_click_removes_it() {
        let line = stored(Point::new(200.0, 80.0), Point::new(200.0, 120.0));
        let (d, id) = column(vec![line]);
        let (s, p) = (selected(id), paint());
        let c = cx(&d, &s, &p);
        let mut t = crate::create("stitchAngle");
        assert_eq!(t.cursor(&c, Point::new(200.0, 120.0), Mods::default()), Cursor::Move);
        t.pointer(&c, &ev(PointerKind::Down, 200.0, 120.0));
        let a = t.pointer(&c, &ev(PointerKind::Drag, 230.0, 120.0));
        assert_eq!(
            a.last(),
            Some(&Action::Preview(
                "embroidery.set".into(),
                json!({"ids": [id.0], "angles": [stored(Point::new(200.0, 80.0), Point::new(230.0, 120.0))]})
            ))
        );
        assert_eq!(t.pointer(&c, &ev(PointerKind::Up, 230.0, 120.0)), vec![Action::Commit]);
        // Alt over the line: removed.
        let alt = Mods { alt: true, ..Mods::default() };
        assert_eq!(t.cursor(&c, Point::new(201.0, 100.0), alt), Cursor::PenDelete);
        let mut down = ev(PointerKind::Down, 201.0, 100.0);
        down.mods = alt;
        assert_eq!(t.pointer(&c, &down), vec![Action::Exec("embroidery.set".into(), json!({"ids": [id.0], "angles": []}))]);
        // So does a double-click.
        assert_eq!(
            t.pointer(&c, &ev(PointerKind::DoubleClick, 200.0, 95.0)),
            vec![Action::Exec("embroidery.set".into(), json!({"ids": [id.0], "angles": []}))]
        );
    }

    #[test]
    fn with_nothing_selected_a_click_selects_a_satin() {
        let (d, id) = column(vec![]);
        let (s, p) = (Selection::default(), paint());
        let c = cx(&d, &s, &p);
        let mut t = crate::create("stitchAngle");
        assert_eq!(t.cursor(&c, Point::new(200.0, 400.0), Mods::default()), Cursor::NotAllowed);
        assert_eq!(t.cursor(&c, Point::new(200.0, 100.0), Mods::default()), Cursor::Arrow);
        t.pointer(&c, &ev(PointerKind::Down, 200.0, 100.0));
        assert_eq!(t.pointer(&c, &ev(PointerKind::Up, 200.0, 100.0)), vec![Action::Exec("select.set".into(), json!({"ids": [id.0]}))]);
        // The lines show while it is selected.
        let s = selected(id);
        let d = column(vec![stored(Point::new(200.0, 80.0), Point::new(200.0, 120.0))]).0;
        assert_eq!(t.overlays(&cx(&d, &s, &p)).len(), 3);
    }
}
