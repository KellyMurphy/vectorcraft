//! Distorted art sewn as it is drawn (E8.2a): an object inside envelopes (Object › Envelope
//! Distort) and one with Warp effects (Effect › Warp) is sewn from its distorted outlines, type
//! letter by letter, not from the art before the distortion.
//!
//! Envelopes map points through [`EnvelopeMap`], as the canvas does; Warp effects through
//! [`warp_point`], with the reference box the effects use (the bounds of the whole object's
//! outlines, then of each effect's result). With the innermost envelope's Distort Appearance on,
//! the object's own effects apply before the envelopes, else after them, as the canvas draws it.

use std::sync::Arc;

use serde_json::Value;
use vectorcraft_doc::live::{EnvelopeMap, WarpStyle, map_nonlinear, warp_point};
use vectorcraft_doc::{Node, NodeKind};
use vectorcraft_geom::{PathData, Point, Rect};

/// What distorts an object's art: the envelopes it sits in, innermost first.
#[derive(Clone, Copy, Default)]
pub(crate) struct Warp<'a> {
    pub envelopes: &'a [&'a Arc<Node>],
}

impl Warp<'_> {
    /// Whether `n`'s art is sewn distorted: inside an envelope or with a Warp effect.
    pub fn bends(&self, n: &Node) -> bool {
        !self.envelopes.is_empty() || warps(n).next().is_some()
    }

    /// `paths` (all of `n`'s art) distorted as the canvas draws them.
    pub fn apply(&self, n: &Node, paths: &mut [&mut PathData]) {
        if !self.bends(n) {
            return;
        }
        let first = self.envelopes.first().and_then(|e| match &e.kind {
            NodeKind::Envelope { options, .. } => Some(options.distort_appearance),
            _ => None,
        });
        let effects_first = first.unwrap_or(true);
        if effects_first {
            effects(n, paths);
        }
        for e in self.envelopes {
            let NodeKind::Envelope { fidelity, .. } = &e.kind else { continue };
            let Some(map) = EnvelopeMap::of(e) else { continue };
            let src = map.src();
            // Cut as finely as the canvas does at this Fidelity.
            let fid = if fidelity.is_finite() { fidelity.clamp(0.0, 100.0) / 100.0 } else { 0.5 };
            let piece = src.width().hypot(src.height()).max(1e-6) / (8.0 + fid * 56.0);
            for p in paths.iter_mut() {
                **p = finite(map_nonlinear(p, piece, |q| map.map(q)));
            }
        }
        if !effects_first {
            effects(n, paths);
        }
    }

    /// Other geometry effects `n` has, which aren't sewn (their ids).
    pub fn unsewn(n: &Node) -> Vec<String> {
        let geometry = |id: &str| {
            id.starts_with("distort.") || id.starts_with("path.") || id.starts_with("stylize.roundCorners") || id.starts_with("stylize.scribble")
        };
        n.appearance.effects.iter().filter(|e| e.visible && geometry(&e.id) && e.id != "path.outlineObject").map(|e| e.id.clone()).collect()
    }
}

/// `n`'s visible Warp effects, in order: style and params.
fn warps(n: &Node) -> impl Iterator<Item = (WarpStyle, &Value)> {
    n.appearance.effects.iter().filter(|e| e.visible).filter_map(|e| Some((e.id.strip_prefix("warp.").and_then(WarpStyle::from_id)?, &e.params)))
}

/// `n`'s Warp effects applied to `paths` as one piece, each with the bounds of what it is given.
fn effects(n: &Node, paths: &mut [&mut PathData]) {
    for (style, params) in warps(n) {
        let Some(b) = paths.iter().filter_map(|p| p.bounds()).reduce(|a, b| a.union(b)) else { return };
        for p in paths.iter_mut() {
            **p = finite(warp(p, b, style, params));
        }
    }
}

/// A Warp effect of `style` over box `b` (its params as the Effect › Warp dialog sets them).
fn warp(path: &PathData, b: Rect, style: WarpStyle, p: &Value) -> PathData {
    let num = |k: &str, d: f64| p.get(k).and_then(Value::as_f64).filter(|v| v.is_finite()).unwrap_or(d).clamp(-100.0, 100.0) / 100.0;
    let (bend, dh, dv) = (num("bend", 50.0), num("horizontal", 0.0), num("vertical", 0.0));
    let vertical = p.get("orientation").and_then(Value::as_str).is_some_and(|o| o.eq_ignore_ascii_case("vertical"));
    let c = b.center();
    let (hw, hh) = ((b.width() / 2.0).max(1e-9), (b.height() / 2.0).max(1e-9));
    let f = |q: Point| {
        let (x, y) = ((q.x - c.x) / hw, (q.y - c.y) / hh);
        let (x2, y2) = if vertical {
            let (a, b2) = warp_point(style, bend, dh, dv, hw / hh, y, x);
            (b2, a)
        } else {
            warp_point(style, bend, dh, dv, hh / hw, x, y)
        };
        Point::new(c.x + x2 * hw, c.y + y2 * hh)
    };
    map_nonlinear(path, b.width().hypot(b.height()) / 24.0, f)
}

/// `p`, or nothing when a map sent a point to infinity (a degenerate envelope).
fn finite(p: PathData) -> PathData {
    if p.bounds().is_some_and(|b| b.x0.is_finite() && b.y0.is_finite() && b.x1.is_finite() && b.y1.is_finite()) { p } else { PathData::default() }
}
