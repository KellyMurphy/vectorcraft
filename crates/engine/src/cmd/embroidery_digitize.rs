//! Auto-digitize (E8.6): a logo (an image, an Image Trace group, or flat-colour vector art) made
//! into stitch objects in one step (`embroidery.digitize`): traced to flat-colour regions, specks
//! merged into the region round them, each region sewn as Auto (which picks bean, satin shape, or
//! fill with a border by its widths) in its colour's thread (the chart's nearest, when one is
//! set), the sewing ordered to cut travel, and the adviser's fixes made where spots come out too
//! dense. The result is ordinary stitch objects, all one undo step.

use serde_json::{Value, json};
use vectorcraft_doc::{Document, Node, NodeId, NodeKind};
use vectorcraft_embroidery::{PT_MM, advise, check};
use vectorcraft_geom::{FillRule, Rect};
use vectorcraft_pathops as po;

use super::{CommandSpec, Result, Session, bad, cmd, has_doc};

/// The most regions digitized.
const MAX_REGIONS: usize = 2000;
/// The most rounds of adviser fixes.
const MAX_FIX_ROUNDS: usize = 8;
/// With `lines: centreline`, parts narrower than this (mm, their typical width) are sewn along
/// their centre lines.
const CENTRELINE_UNDER: f64 = 2.0;

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "embroidery.digitize",
            "Auto-Digitize",
            ["Object", "Embroidery"],
            None,
            "{id?: an image or an Image Trace group (traced as a flat-colour logo) or a group of filled paths (default: the selection), colors?: 2-10 (the logo's colours when tracing; default found automatically), minArea?: mm² (regions smaller than this are merged into the region round them or else left unstitched; default 2), keepWhite?: false (sew white areas too: by default white is the fabric and is left out of a trace), order?: optimize|keep (optimize: the design's sewing order grouped by thread and nearest first; the default for traced art), fix?: true (make the adviser's fixes where spots come out too dense), lines?: auto|centreline (centreline: parts narrower than 2 mm, traced lines and handwriting, are sewn as one line along their centre, running stitch under 1 mm wide, else satin as wide as the part; auto: Auto sews them as their width suits)} turn a logo into stitch objects as one undo step → {id: the group, objects (each Auto: it picks bean, satin shape or fill with a border by the area's widths as it sews), threads, merged, dropped, centrelines: parts sewn along their centre lines, fixes: [labels made], findings: {error, warning, note}}",
            has_doc,
            digitize
        ),
        cmd!(
            "embroidery.fillAt",
            "Stitch Fill",
            ["Object", "Embroidery"],
            None,
            "{x: document points, y: document points (a point inside a closed area: a shape or where shapes and paths cross as Live Paint finds areas), outline?: false|true|running|satin (sew the area's edge instead of filling it: true or running, a running-stitch outline, the tool's Alt-click; satin, a satin border, Alt-Shift-click), centreline?: false|auto|running|satin (sew a thin area, a traced line or handwriting, as one line along its centre instead, the tool's Shift-click: auto picks running stitch for a line thinner than 1 mm, else satin as wide as the area), width?: 0.3-12 mm (a satin outline's width, default 2; a satin centre line's, default the area's own width), thread?: \"#rrggbb\" (default: the colour of the top shape there)} add that area as a new stitch object on top (Auto: fill with a border or satin shape by its width; or its outline; or its centre line) as one undo step; the Stitch Fill tool's click → {id, areaMm2, outline, centreline, type: the stitch type, widthMm: a satin's width or null}",
            has_doc,
            fill_at
        ),
    ]
}

/// Run `f`'s commands as one undo step, all undone when it fails.
fn in_one_step<T>(s: &mut Session, f: impl FnOnce(&mut Session) -> Result<T>) -> Result<T> {
    let scope = s.begin_undo_scope();
    let r = f(s);
    if let Some(scope) = scope {
        s.end_undo_scope(scope, r.is_err());
    }
    r
}

/// Whether `n` is an image, or an Image Trace group (its first child the image).
fn traceable(n: &Node) -> bool {
    match &n.kind {
        NodeKind::Image(_) => true,
        NodeKind::Group { .. } => n.children().and_then(|c| c.first()).is_some_and(|i| matches!(i.kind, NodeKind::Image(_))),
        _ => false,
    }
}

/// The filled paths and compound paths under `n` (itself included), in paint order.
fn regions(n: &Node, out: &mut Vec<NodeId>) {
    match &n.kind {
        NodeKind::Path { .. } | NodeKind::Compound { .. } => {
            if !matches!(n.appearance.fill_paint(), vectorcraft_color::Paint::None) {
                out.push(n.id);
            }
        }
        _ => {
            for c in n.children().into_iter().flatten() {
                regions(c, out);
            }
        }
    }
}

/// A plain path's outline and fill rule (compound paths aren't merged).
fn plain(doc: &Document, id: NodeId) -> Option<(vectorcraft_geom::PathData, FillRule)> {
    match &doc.node(id)?.kind {
        NodeKind::Path { path, rule, .. } => Some((path.clone(), *rule)),
        _ => None,
    }
}

/// The area of region `id`, mm².
fn area_mm2(doc: &Document, id: NodeId) -> f64 {
    let n = doc.node(id);
    let pt2 = match n.map(|n| &n.kind) {
        Some(NodeKind::Path { path, rule, .. }) => po::try_area(path, *rule).unwrap_or(0.0),
        Some(NodeKind::Compound { children, rule }) => {
            let paths: Vec<&vectorcraft_geom::PathData> = children.iter().filter_map(|c| c.path_data()).collect();
            let all = paths.iter().map(|p| (*p, *rule)).collect::<Vec<_>>();
            po::try_unite_all(&all).ok().map_or(0.0, |u| po::try_area(&u, FillRule::NonZero).unwrap_or(0.0))
        }
        _ => 0.0,
    };
    pt2.abs() * PT_MM * PT_MM
}

fn bounds(doc: &Document, id: NodeId) -> Option<Rect> {
    doc.node(id).and_then(Node::geometric_bounds)
}

/// Region `id`'s outline (a path, or a compound path's paths together) and fill rule.
fn outline(doc: &Document, id: NodeId) -> Option<(vectorcraft_geom::PathData, FillRule)> {
    match &doc.node(id)?.kind {
        NodeKind::Path { path, rule, .. } => Some((path.clone(), *rule)),
        NodeKind::Compound { children, rule } => {
            let subpaths = children.iter().filter_map(|c| c.path_data()).flat_map(|p| p.subpaths.iter().cloned()).collect();
            Some((vectorcraft_geom::PathData { subpaths }, *rule))
        }
        _ => None,
    }
}

/// Region `id`'s centre lines and width (mm), when it is a thin line ([`CENTRELINE_UNDER`]).
fn thin_line(doc: &Document, id: NodeId) -> Option<(vectorcraft_geom::PathData, f64)> {
    let (path, rule) = outline(doc, id)?;
    let polys: Vec<Vec<kurbo::Point>> = vectorcraft_embroidery::generate::polylines(&path).into_iter().map(|(l, _)| l).collect();
    if vectorcraft_embroidery::stroke_width(&polys) >= CENTRELINE_UNDER {
        return None;
    }
    centreline_path(&path, rule)
}

/// A region and its area, mm².
type Area = (NodeId, f64);

/// Merge each region under `min_area` mm² into the region round it (the smallest plain path whose
/// box holds its box), else leave it unstitched. Gives the regions kept, how many were merged and
/// how many dropped.
fn merge_specks(s: &mut Session, ids: Vec<NodeId>, min_area: f64) -> Result<(Vec<NodeId>, usize, usize)> {
    let doc = s.doc()?.doc.clone();
    let areas: Vec<(NodeId, f64)> = ids.iter().map(|id| (*id, area_mm2(&doc, *id))).collect();
    let (small, mut keep): (Vec<Area>, Vec<Area>) = areas.into_iter().partition(|(_, a)| *a < min_area);
    if small.is_empty() {
        return Ok((keep.into_iter().map(|(id, _)| id).collect(), 0, 0));
    }
    let (mut merged, mut dropped) = (0, 0);
    s.edit("Auto-Digitize", |d, _| {
        for (speck, _) in &small {
            let Some(b) = bounds(d, *speck) else {
                dropped += 1;
                continue;
            };
            let grown = b.inflate(1.0, 1.0);
            let host = keep
                .iter()
                .filter_map(|(id, _)| {
                    bounds(d, *id).filter(|r| r.contains_rect(b) || r.inflate(0.5, 0.5).contains_rect(grown)).map(|r| (*id, r.area()))
                })
                .filter(|(id, _)| plain(d, *id).is_some())
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(id, _)| id);
            let united = host.zip(plain(d, *speck)).and_then(|(h, (sp, sr))| {
                let (hp, hr) = plain(d, h)?;
                po::try_unite_all(&[(&hp, hr), (&sp, sr)]).ok().map(|u| (h, u))
            });
            match united {
                Some((h, u)) => {
                    if let Some(p) = d.node_mut(h).and_then(Node::path_data_mut) {
                        *p = u;
                    }
                    if let Some(NodeKind::Path { rule, .. }) = d.node_mut(h).map(|n| &mut n.kind) {
                        *rule = FillRule::NonZero;
                    }
                    d.remove(*speck)?;
                    merged += 1;
                }
                None => dropped += 1,
            }
        }
        Ok(())
    })?;
    keep.retain(|(id, _)| s.doc().is_ok_and(|st| st.doc.node(*id).is_some()));
    Ok((keep.into_iter().map(|(id, _)| id).collect(), merged, dropped))
}

fn digitize(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.digitize";
    let min_area = p.get("minArea").and_then(Value::as_f64).filter(|v| v.is_finite()).unwrap_or(2.0);
    if !(0.0..=1000.0).contains(&min_area) {
        return Err(bad(C, "`minArea` must be from 0 to 1000 mm²"));
    }
    let colors = match p.get("colors") {
        None | Some(Value::Null) => None,
        Some(v) => Some(v.as_u64().filter(|n| (2..=10).contains(n)).ok_or_else(|| bad(C, "`colors` must be from 2 to 10"))?),
    };
    let keep_white = p.get("keepWhite").and_then(Value::as_bool).unwrap_or(false);
    let fix = p.get("fix").and_then(Value::as_bool).unwrap_or(true);
    let order = super::str_param(p, "order");
    if order.is_some_and(|o| !matches!(o, "optimize" | "keep")) {
        return Err(bad(C, "`order` must be optimize or keep"));
    }
    let centre = match p.get("lines") {
        None | Some(Value::Null) => false,
        Some(v) => match v.as_str() {
            Some("auto") => false,
            Some("centreline") => true,
            _ => return Err(bad(C, "`lines` must be auto or centreline")),
        },
    };
    let target = match p.get("id").and_then(Value::as_u64) {
        Some(id) => NodeId(id),
        None => {
            let sel = &s.doc()?.selection;
            let ids: Vec<NodeId> = sel.whole();
            match ids.as_slice() {
                [one] => *one,
                [] => return Err(bad(C, "select a logo (an image, a traced image, or a group of filled shapes)")),
                _ => return Err(bad(C, "select one logo: group its shapes first")),
            }
        }
    };
    let node = s.doc()?.doc.node(target).cloned().ok_or_else(|| bad(C, format!("no object {}", target.0)))?;
    let traced = traceable(&node);
    in_one_step(s, |s| {
        // Flat-colour regions: traced from the image, or the art as it is.
        let group = if traced {
            let mut params = json!({"mode": "logo", "ignoreWhite": !keep_white, "method": "abutting"});
            if let Some(n) = colors {
                params["colors"] = json!(n);
            }
            let r = s.execute("imageTrace.makeAndExpand", &json!({"id": target.0, "params": params}))?;
            r["id"].as_u64().map(NodeId).ok_or_else(|| bad(C, "the trace made no paths"))?
        } else {
            target
        };
        let mut ids = vec![];
        if let Some(n) = s.doc()?.doc.node(group) {
            regions(n, &mut ids);
        }
        if ids.is_empty() {
            return Err(bad(C, "no filled areas to stitch: the logo traced to nothing (try keepWhite, or more colors)"));
        }
        if ids.len() > MAX_REGIONS {
            return Err(bad(
                C,
                format!("the logo has {} areas: at most {MAX_REGIONS} are digitized (simplify it, or trace with fewer colors)", ids.len()),
            ));
        }
        let (ids, merged, dropped) = merge_specks(s, ids, min_area)?;
        // Each region Auto, in its colour's thread (matched to the chart when one is set).
        let doc = s.doc()?.doc.clone();
        let chart = doc.embroidery.chart.as_ref().and_then(|id| doc.embroidery.charts.iter().find(|c| &c.id == id && c.unknown.is_none()));
        let mut by_thread: Vec<(String, Vec<u64>)> = vec![];
        // Thin parts sewn along their centre lines: (region, thread, (line, width mm)).
        let mut lines: Vec<(NodeId, [u8; 3], (vectorcraft_geom::PathData, f64))> = vec![];
        let mut centrelines = 0;
        for id in &ids {
            let Some(n) = doc.node(*id) else { continue };
            let rgb = match n.appearance.fill_paint() {
                vectorcraft_color::Paint::Solid { color, .. } => {
                    let [r, g, b, _] = color.to_rgba8(1.0);
                    [r, g, b]
                }
                _ => [0, 0, 0],
            };
            let thread = chart
                .and_then(|c| vectorcraft_embroidery::threads::nearest(c, rgb))
                .and_then(|(t, _)| vectorcraft_embroidery::threads::parse_hex(&t.color))
                .unwrap_or(rgb);
            let hex = vectorcraft_embroidery::params::hex(thread);
            if !by_thread.iter().any(|(h, _)| *h == hex) {
                by_thread.push((hex.clone(), vec![]));
            }
            if centre && let Some(line) = thin_line(&doc, *id) {
                lines.push((*id, thread, line));
            } else if let Some((_, v)) = by_thread.iter_mut().find(|(h, _)| *h == hex) {
                v.push(id.0);
            }
        }
        for (hex, v) in by_thread.iter().filter(|(_, v)| !v.is_empty()) {
            s.execute("embroidery.set", &json!({"ids": v, "type": "auto", "thread": hex}))?;
        }
        if !lines.is_empty() {
            s.edit("Auto-Digitize", |d, _| {
                for (id, rgb, (path, width)) in lines.drain(..) {
                    let Some(n) = d.node_mut(id) else { continue };
                    let mut st = centreline_stitch(width, None);
                    st.thread = Some(rgb);
                    let paint = vectorcraft_color::Paint::solid(vectorcraft_color::Color::rgb8(rgb[0], rgb[1], rgb[2]));
                    let stroke = st.width.map_or(1.0, |w| w / PT_MM);
                    n.kind = NodeKind::Path { path, rule: FillRule::NonZero, live: None, clipping: false, guide: false };
                    n.appearance = vectorcraft_doc::Appearance::basic(vectorcraft_color::Paint::None, paint, stroke).into();
                    n.extras_mut().stitch = Some(Box::new(st));
                    centrelines += 1;
                }
                Ok(())
            })?;
        }
        if order.map_or(traced, |o| o == "optimize") {
            s.execute("embroidery.optimizeOrder", &json!({}))?;
        }
        // The adviser's fixes, a round at a time, while the new objects have dense spots.
        let mut made = vec![];
        if fix {
            for _ in 0..MAX_FIX_ROUNDS {
                let doc = s.doc()?.doc.clone();
                let plan = s.stitch_plan(&doc);
                let Some(f) = check::check(&doc, &plan).into_iter().find(|f| f.rule == "dense" && f.object.is_some_and(|o| ids.contains(&o))) else {
                    break;
                };
                let mut replan = |d: &Document| s.stitch_plan(d);
                let Some(x) = advise::fixes(&doc, &plan, &f, &mut replan).into_iter().find(|x| x.id != "drop-duplicate") else { break };
                let (command, params) = x.command();
                s.execute(command, &params)?;
                made.push(x.label);
            }
        }
        let doc = s.doc()?.doc.clone();
        let plan = s.stitch_plan(&doc);
        let found = check::check(&doc, &plan);
        let count = |sev: check::Severity| found.iter().filter(|f| f.severity == sev).count();
        Ok(json!({
            "id": group.0,
            "objects": ids.len(),
            "threads": by_thread.iter().map(|(h, _)| h.clone()).collect::<Vec<_>>(),
            "merged": merged,
            "dropped": dropped,
            "centrelines": centrelines,
            "fixes": made,
            "findings": {"error": count(check::Severity::Error), "warning": count(check::Severity::Warning), "note": count(check::Severity::Note)},
        }))
    })
}

/// The visible, unlocked objects of the visible, unlocked layers that can bound the area at `p`:
/// those whose box holds it, and those whose box meets theirs (a path crossing a shape cuts it
/// though its box may miss the point), a few rounds out.
fn objects_at(doc: &Document, p: kurbo::Point) -> Vec<NodeId> {
    let all: Vec<(NodeId, Rect)> = doc
        .layers
        .iter()
        .filter(|l| l.visible && !l.locked)
        .flat_map(|l| l.children().into_iter().flatten())
        .filter(|n| n.visible && !n.locked)
        .filter_map(|n| n.geometric_bounds().map(|b| (n.id, b.inflate(1.0, 1.0))))
        .collect();
    let mut keep: Vec<bool> = all.iter().map(|(_, b)| b.contains(p)).collect();
    for _ in 0..3 {
        let reach: Vec<Rect> = all.iter().zip(&keep).filter(|(_, k)| **k).map(|((_, b), _)| *b).collect();
        let mut grew = false;
        for ((_, b), k) in all.iter().zip(keep.iter_mut()) {
            if !*k && reach.iter().any(|r| r.intersect(*b).area() > 0.0) {
                *k = true;
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    all.iter().zip(&keep).filter(|(_, k)| **k).map(|((id, _), _)| *id).collect()
}

/// How an area is sewn by `embroidery.fillAt`.
#[derive(Clone, Copy, PartialEq)]
enum Sew {
    /// Auto: fill with a border, or satin shape, by its width.
    Area,
    /// Its edge, as running stitch or (`satin`) a satin border.
    Outline { satin: bool },
    /// Along its centre line: running stitch, satin, or (`None`) whichever its width suits.
    Centreline { satin: Option<bool> },
}

/// `outline` and `centreline` from `embroidery.fillAt`'s params.
fn sew_param(p: &Value) -> std::result::Result<Sew, &'static str> {
    let outline = match p.get("outline") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => None,
        Some(Value::Bool(true)) => Some(false),
        Some(v) => match v.as_str() {
            Some("running") => Some(false),
            Some("satin") => Some(true),
            _ => return Err("`outline` must be true, false, running or satin"),
        },
    };
    let centre = match p.get("centreline") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => None,
        Some(Value::Bool(true)) => Some(None),
        Some(v) => match v.as_str() {
            Some("auto") => Some(None),
            Some("running") => Some(Some(false)),
            Some("satin") => Some(Some(true)),
            _ => return Err("`centreline` must be true, false, auto, running or satin"),
        },
    };
    match (outline, centre) {
        (Some(_), Some(_)) => Err("give `outline` or `centreline`, not both"),
        (Some(satin), None) => Ok(Sew::Outline { satin }),
        (None, Some(satin)) => Ok(Sew::Centreline { satin }),
        (None, None) => Ok(Sew::Area),
    }
}

/// The centre lines of the closed area `path` (document points) as an open path in points, and
/// the area's width along them, mm. `None`: too small to have any.
pub(crate) fn centreline_path(path: &vectorcraft_geom::PathData, rule: FillRule) -> Option<(vectorcraft_geom::PathData, f64)> {
    let polys: Vec<Vec<kurbo::Point>> = vectorcraft_embroidery::generate::polylines(path).into_iter().map(|(l, _)| l).collect();
    let lines = vectorcraft_embroidery::skeleton::centrelines(&polys, rule);
    if lines.is_empty() {
        return None;
    }
    let width = vectorcraft_embroidery::skeleton::centreline_width(&lines);
    let subpaths = lines
        .iter()
        .map(|l| {
            let pts: Vec<kurbo::Point> = l.points.iter().map(|p| kurbo::Point::new(p.x / PT_MM, p.y / PT_MM)).collect();
            // A closed line repeats its start; the subpath closes back to it instead.
            let pts = if l.closed { pts.split_last().map_or(pts.clone(), |(_, rest)| rest.to_vec()) } else { pts };
            vectorcraft_geom::SubPath::polyline(&pts, l.closed)
        })
        .collect();
    Some((vectorcraft_geom::PathData { subpaths }, width))
}

/// The stitch settings for sewing along a centre line `width` mm wide: running stitch when it is
/// thinner than Auto's thinnest satin (or `satin` says so), else a satin that wide.
pub(crate) fn centreline_stitch(width: f64, satin: Option<bool>) -> vectorcraft_doc::StitchSettings {
    use vectorcraft_doc::{StitchKind, StitchSettings};
    let auto = StitchSettings::new(StitchKind::Auto);
    if satin.unwrap_or(width >= auto.auto_thresholds().0) {
        let w = width.clamp(*StitchSettings::WIDTH.start(), *StitchSettings::WIDTH.end());
        StitchSettings { width: Some((w * 100.0).round() / 100.0), ..StitchSettings::new(StitchKind::Satin) }
    } else {
        StitchSettings::new(StitchKind::Running)
    }
}

fn fill_at(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.fillAt";
    let num = |k: &str| p.get(k).and_then(Value::as_f64).filter(|v| v.is_finite() && v.abs() <= 1e7);
    let (Some(x), Some(y)) = (num("x"), num("y")) else { return Err(bad(C, "`x` and `y` must be a point in document points")) };
    let at = kurbo::Point::new(x, y);
    let sew = sew_param(p).map_err(|e| bad(C, e))?;
    let width = match p.get("width") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let range = vectorcraft_doc::StitchSettings::WIDTH;
            Some(
                v.as_f64()
                    .filter(|w| range.contains(w))
                    .ok_or_else(|| bad(C, format!("`width` must be from {} to {} mm", range.start(), range.end())))?,
            )
        }
    };
    let thread = match p.get("thread") {
        None | Some(Value::Null) => None,
        Some(v) => Some(v.as_str().and_then(vectorcraft_embroidery::threads::parse_hex).ok_or_else(|| bad(C, "`thread` must be \"#rrggbb\""))?),
    };
    let doc = s.doc()?.doc.clone();
    let roots = objects_at(&doc, at);
    let (map, leaves) = vectorcraft_tools::builder::BuilderMap::new(&doc, &roots, false, true);
    let Some((region, _, _)) = map.region_at(at).and_then(|i| map.regions.get(i)) else {
        return Err(bad(C, "no closed area here: click inside a shape, or where shapes and paths enclose an area"));
    };
    let path = region.path.clone();
    // The thread: given, else the fill (then stroke) of the top shape covering the area.
    let top = region.top().and_then(|i| leaves.get(i));
    let rgb = thread.or_else(|| {
        top.and_then(|n| match (n.appearance.fill_paint(), n.appearance.stroke_paint()) {
            (vectorcraft_color::Paint::Solid { color, .. }, _) | (_, vectorcraft_color::Paint::Solid { color, .. }) => {
                let [r, g, b, _] = color.to_rgba8(1.0);
                Some([r, g, b])
            }
            _ => None,
        })
    });
    let rgb = rgb.unwrap_or([0, 0, 0]);
    let paint = vectorcraft_color::Paint::solid(vectorcraft_color::Color::rgb8(rgb[0], rgb[1], rgb[2]));
    let area = po::try_area(&path, FillRule::NonZero).unwrap_or(0.0).abs() * PT_MM * PT_MM;
    use vectorcraft_doc::{Appearance, StitchKind, StitchSettings};
    let none = vectorcraft_color::Paint::None;
    // The path sewn, its look, name and stitch.
    let (path, appearance, name, stitch) = match sew {
        Sew::Area => (path, Appearance::basic(paint, none, 1.0), "Stitch Fill", StitchSettings::new(StitchKind::Auto)),
        Sew::Outline { satin: false } => (path, Appearance::basic(none, paint, 1.0), "Stitch Outline", StitchSettings::new(StitchKind::Running)),
        Sew::Outline { satin: true } => {
            let w = width.unwrap_or(StitchSettings::DEFAULT_BORDER);
            let st = StitchSettings { width: Some(w), ..StitchSettings::new(StitchKind::Satin) };
            (path, Appearance::basic(none, paint, w / PT_MM), "Satin Outline", st)
        }
        Sew::Centreline { satin } => {
            let (line, measured) =
                centreline_path(&path, FillRule::NonZero).ok_or_else(|| bad(C, "this area is too small to find its centre line"))?;
            let mut st = centreline_stitch(measured, satin);
            if st.kind == StitchKind::Satin && width.is_some() {
                st.width = width;
            }
            let stroke = st.width.map_or(1.0, |w| w / PT_MM);
            (line, Appearance::basic(none, paint, stroke), "Stitch Centreline", st)
        }
    };
    let kind = stitch.kind;
    let width = stitch.width;
    let id = s.edit(name, |d, sel| {
        let id = d.alloc_id();
        let mut n = Node::path(id, path, appearance);
        n.name = Some(name.into());
        n.extras_mut().stitch = Some(Box::new(stitch));
        let layer = d.default_layer();
        let index = d.children_mut(layer).map(|c| c.len()).unwrap_or(0);
        d.insert(layer, index, n)?;
        sel.set([id]);
        Ok(id)
    })?;
    let ty = kind.id();
    Ok(json!({
        "id": id.0,
        "areaMm2": (area * 100.0).round() / 100.0,
        "outline": matches!(sew, Sew::Outline { .. }),
        "centreline": matches!(sew, Sew::Centreline { .. }),
        "type": ty,
        "widthMm": width,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_doc::{ImageBlob, ImageObject};
    use vectorcraft_geom::Affine;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 400, "height": 400})).unwrap();
        s
    }

    /// A code-drawn logo: a red disc with a blue bar across it and a one-pixel speck, on white.
    fn logo(s: &mut Session) -> NodeId {
        let r = vectorcraft_trace::Raster::from_fn(200, 200, |x, y| {
            let (dx, dy) = (x as f64 - 100.0, y as f64 - 100.0);
            if (x, y) == (20, 20) {
                [0, 0, 0, 255]
            } else if (90..110).contains(&y) && (30..170).contains(&x) {
                [20, 40, 200, 255]
            } else if dx * dx + dy * dy <= 70.0 * 70.0 {
                [220, 30, 30, 255]
            } else {
                [255, 255, 255, 255]
            }
        });
        let png = r.encode_png();
        s.edit("img", |d, sel| {
            d.images.insert("img1".into(), ImageBlob::new("image/png", png));
            let id = d.alloc_id();
            let l = d.default_layer();
            let image = ImageObject {
                key: "img1".into(),
                width: 200,
                height: 200,
                xf: Affine::translate((50.0, 50.0)),
                link: None,
                placement: Default::default(),
                pixels: None,
            };
            d.insert(l, 0, Node::new(id, NodeKind::Image(Box::new(image))))?;
            sel.set([id]);
            Ok(id)
        })
        .unwrap()
    }

    #[test]
    fn a_logo_image_becomes_stitch_objects_in_one_undo_step() {
        let mut s = session();
        logo(&mut s);
        let r = s.execute("embroidery.digitize", &json!({})).unwrap();
        assert!(r["objects"].as_u64().unwrap() >= 2, "{r}");
        assert_eq!(r["threads"].as_array().unwrap().len(), 2, "red and blue, white left out: {r}");
        assert_eq!(r["findings"]["error"], 0, "{r}");
        let info = s.execute("embroidery.info", &json!({})).unwrap();
        assert!(info["design"]["stitches"].as_u64().unwrap() > 500, "{info}");
        // One undo step puts the image back.
        s.execute("edit.undo", &json!({})).unwrap();
        let doc = &s.doc().unwrap().doc;
        assert!(doc.layers.iter().flat_map(|l| l.children().into_iter().flatten()).any(|n| matches!(n.kind, NodeKind::Image(_))));
        assert_eq!(s.execute("embroidery.info", &json!({})).unwrap()["design"]["stitches"], 0);
    }

    #[test]
    fn vector_art_is_digitized_as_it_is_and_specks_merge() {
        let mut s = session();
        let big = s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 100, "height": 60})).unwrap();
        let speck = s.execute("shape.rectangle", &json!({"x": 40, "y": 30, "width": 2, "height": 2})).unwrap();
        let thin = s.execute("shape.rectangle", &json!({"x": 10, "y": 100, "width": 100, "height": 2})).unwrap();
        s.execute("select.set", &json!({"ids": [big["id"], speck["id"], thin["id"]]})).unwrap();
        s.execute("object.group", &json!({})).unwrap();
        let r = s.execute("embroidery.digitize", &json!({"minArea": 1})).unwrap();
        assert_eq!((r["objects"].as_u64(), r["merged"].as_u64()), (Some(2), Some(1)), "{r}");
        assert!(s.doc().unwrap().doc.node(NodeId(speck["id"].as_u64().unwrap())).is_none(), "the speck went into the rectangle round it");
        // Auto picks by width: a wide fill and a thin line.
        let plan = s.stitch_plan(&s.doc().unwrap().doc);
        assert_eq!(plan.objects.len(), 2);
    }

    #[test]
    fn a_click_inside_crossing_shapes_stitches_just_that_area() {
        let mut s = session();
        // Two overlapping squares: the click lands in their overlap only.
        s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap();
        s.execute("shape.rectangle", &json!({"x": 50, "y": 50, "width": 100, "height": 100})).unwrap();
        let r = s.execute("embroidery.fillAt", &json!({"x": 75, "y": 75, "thread": "#123456"})).unwrap();
        let mm = PT_MM * PT_MM;
        assert!((r["areaMm2"].as_f64().unwrap() - 2500.0 * mm).abs() < 1.0, "{r}");
        let id = NodeId(r["id"].as_u64().unwrap());
        let doc = &s.doc().unwrap().doc;
        let n = doc.node(id).unwrap();
        assert_eq!(n.extras().stitch.as_deref().unwrap().kind, vectorcraft_doc::StitchKind::Auto);
        assert_eq!(vectorcraft_embroidery::thread_of_node(n), [0x12, 0x34, 0x56]);
        assert_eq!(s.doc().unwrap().selection.whole(), vec![id]);
        // A bar crossing a disc, the click beside the bar: only the disc's part left of it.
        let mut s2 = session();
        s2.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap();
        s2.execute("shape.rectangle", &json!({"x": 40, "y": -20, "width": 20, "height": 140})).unwrap();
        let r2 = s2.execute("embroidery.fillAt", &json!({"x": 10, "y": 50})).unwrap();
        assert!((r2["areaMm2"].as_f64().unwrap() - 4000.0 * mm).abs() < 1.0, "the part left of the bar: {r2}");
        // Alt: an outline; one undo step each.
        let r = s.execute("embroidery.fillAt", &json!({"x": 10, "y": 10, "outline": true})).unwrap();
        let n = s.doc().unwrap().doc.node(NodeId(r["id"].as_u64().unwrap())).cloned().unwrap();
        assert_eq!(n.extras().stitch.as_deref().unwrap().kind, vectorcraft_doc::StitchKind::Running);
        s.execute("edit.undo", &json!({})).unwrap();
        assert!(s.doc().unwrap().doc.node(n.id).is_none());
        // Outside every shape, and bad params.
        assert!(s.execute("embroidery.fillAt", &json!({"x": 300, "y": 300})).is_err());
        assert!(s.execute("embroidery.fillAt", &json!({"x": 75})).is_err());
        assert!(s.execute("embroidery.fillAt", &json!({"x": 75, "y": 75, "thread": "red"})).is_err());
    }

    /// E11.7: a satin outline, and thin areas sewn along their centre lines.
    #[test]
    fn a_click_outlines_in_satin_or_follows_a_thin_areas_centre() {
        let mm = |v: f64| v / PT_MM;
        let mut s = session();
        s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": mm(40.0), "height": mm(40.0)})).unwrap();
        let r = s.execute("embroidery.fillAt", &json!({"x": mm(20.0), "y": mm(20.0), "outline": "satin", "width": 3})).unwrap();
        assert_eq!((r["type"].as_str(), r["widthMm"].as_f64(), r["outline"].as_bool()), (Some("satin"), Some(3.0), Some(true)), "{r}");
        let n = s.doc().unwrap().doc.node(NodeId(r["id"].as_u64().unwrap())).cloned().unwrap();
        assert!(matches!(n.appearance.fill_paint(), vectorcraft_color::Paint::None));
        assert!((n.appearance.stroke_width() - mm(3.0)).abs() < 1e-6, "the stroke shows the satin's width");
        // A thin bar (0.6 mm): running stitch along its middle; a 3 mm bar: a 3 mm satin.
        let mut s = session();
        s.execute("shape.rectangle", &json!({"x": mm(10.0), "y": mm(10.0), "width": mm(30.0), "height": mm(0.6)})).unwrap();
        s.execute("shape.rectangle", &json!({"x": mm(10.0), "y": mm(30.0), "width": mm(30.0), "height": mm(3.0)})).unwrap();
        let thin = s.execute("embroidery.fillAt", &json!({"x": mm(25.0), "y": mm(10.3), "centreline": "auto"})).unwrap();
        assert_eq!((thin["type"].as_str(), thin["centreline"].as_bool()), (Some("running"), Some(true)), "{thin}");
        let line = s.doc().unwrap().doc.node(NodeId(thin["id"].as_u64().unwrap())).cloned().unwrap();
        let path = line.path_data().unwrap();
        assert_eq!(path.subpaths.len(), 1);
        assert!(!path.subpaths[0].closed);
        for a in &path.subpaths[0].anchors {
            assert!((a.p.y - mm(10.3)).abs() < mm(0.2), "on the bar's middle: {:?}", a.p);
        }
        let wide = s.execute("embroidery.fillAt", &json!({"x": mm(25.0), "y": mm(31.5), "centreline": true})).unwrap();
        assert_eq!(wide["type"], "satin", "{wide}");
        assert!((wide["widthMm"].as_f64().unwrap() - 3.0).abs() < 0.5, "{wide}");
        let forced = s.execute("embroidery.fillAt", &json!({"x": mm(25.0), "y": mm(31.5), "centreline": "running"})).unwrap();
        assert_eq!(forced["type"], "running");
        let info = s.execute("embroidery.info", &json!({})).unwrap();
        assert!(info["design"]["stitches"].as_u64().unwrap() > 20, "{info}");
        // Bad params.
        for p in [
            json!({"x": mm(25.0), "y": mm(31.5), "outline": "zigzag"}),
            json!({"x": mm(25.0), "y": mm(31.5), "centreline": 3}),
            json!({"x": mm(25.0), "y": mm(31.5), "centreline": "auto", "outline": true}),
            json!({"x": mm(25.0), "y": mm(31.5), "outline": "satin", "width": 40}),
        ] {
            assert!(s.execute("embroidery.fillAt", &p).is_err(), "{p}");
        }
    }

    #[test]
    fn digitize_can_sew_thin_parts_along_their_centre_lines() {
        let mm = |v: f64| v / PT_MM;
        let build = |s: &mut Session| {
            let big = s.execute("shape.rectangle", &json!({"x": mm(10.0), "y": mm(10.0), "width": mm(30.0), "height": mm(20.0)})).unwrap();
            let thin = s.execute("shape.rectangle", &json!({"x": mm(10.0), "y": mm(40.0), "width": mm(30.0), "height": mm(0.8)})).unwrap();
            s.execute("select.set", &json!({"ids": [big["id"], thin["id"]]})).unwrap();
            s.execute("object.group", &json!({})).unwrap();
            NodeId(thin["id"].as_u64().unwrap())
        };
        let mut s = session();
        let thin = build(&mut s);
        let r = s.execute("embroidery.digitize", &json!({"lines": "centreline", "minArea": 0})).unwrap();
        assert_eq!((r["objects"].as_u64(), r["centrelines"].as_u64()), (Some(2), Some(1)), "{r}");
        let n = s.doc().unwrap().doc.node(thin).cloned().unwrap();
        let st = n.extras().stitch.as_deref().unwrap();
        assert_eq!(st.kind, vectorcraft_doc::StitchKind::Running);
        assert!(n.path_data().unwrap().subpaths.iter().all(|sp| !sp.closed), "an open line now");
        assert_eq!(r["findings"]["error"], 0, "{r}");
        // By default, Auto sews it as an area.
        let mut s = session();
        let thin = build(&mut s);
        let r = s.execute("embroidery.digitize", &json!({"minArea": 0})).unwrap();
        assert_eq!(r["centrelines"], 0);
        assert_eq!(s.doc().unwrap().doc.node(thin).unwrap().extras().stitch.as_deref().unwrap().kind, vectorcraft_doc::StitchKind::Auto);
        assert!(s.execute("embroidery.digitize", &json!({"lines": "skeleton"})).is_err());
    }

    /// A code-drawn logo `name` of 200 × 200 px: the pixel at (x, y).
    fn corpus_pixel(name: &str, x: u32, y: u32) -> [u8; 4] {
        let (dx, dy) = (x as f64 - 100.0, y as f64 - 100.0);
        let r = dx.hypot(dy);
        let white = [255, 255, 255, 255];
        match name {
            // Flat shapes: a disc, a ring and a square.
            "shapes" => match () {
                _ if r <= 40.0 => [200, 20, 20, 255],
                _ if (60.0..75.0).contains(&r) => [20, 120, 40, 255],
                _ if (10..50).contains(&x) && (150..190).contains(&y) => [20, 40, 200, 255],
                _ => white,
            },
            // Thin lines: a grid of 3 px bars.
            "lines" => {
                if (20..180).contains(&x) && (20..180).contains(&y) && (x % 40 < 3 || y % 40 < 3) {
                    [30, 30, 30, 255]
                } else {
                    white
                }
            }
            // A badge: a disc with a band, and a small star-like speck field.
            _ => match () {
                _ if (88..112).contains(&y) && (25..175).contains(&x) => [240, 200, 20, 255],
                _ if r <= 80.0 => [20, 30, 120, 255],
                _ if x.is_multiple_of(37) && y.is_multiple_of(41) => [20, 30, 120, 255],
                _ => white,
            },
        }
    }

    fn place(s: &mut Session, r: &vectorcraft_trace::Raster) {
        let png = r.encode_png();
        let (w, h) = (r.width, r.height);
        s.edit("img", |d, sel| {
            d.images.insert("img1".into(), ImageBlob::new("image/png", png));
            let id = d.alloc_id();
            let l = d.default_layer();
            let image = ImageObject {
                key: "img1".into(),
                width: w,
                height: h,
                xf: Affine::translate((50.0, 50.0)),
                link: None,
                placement: Default::default(),
                pixels: None,
            };
            d.insert(l, 0, Node::new(id, NodeKind::Image(Box::new(image))))?;
            sel.set([id]);
            Ok(id)
        })
        .unwrap();
    }

    /// E8.6c: each corpus logo digitizes with no error findings and within its budget of trims,
    /// stitches and warnings (recorded when it was built: lower them as digitizing improves, never
    /// raise them to let a change through).
    #[test]
    fn the_logo_corpus_stays_within_its_scores() {
        // (logo, most trims, most stitches, most warnings)
        for (name, trims, stitches, warnings) in [("shapes", 13, 2_400, 0), ("lines", 29, 1_900, 0), ("badge", 11, 4_800, 0)] {
            let mut s = session();
            place(&mut s, &vectorcraft_trace::Raster::from_fn(200, 200, |x, y| corpus_pixel(name, x, y)));
            let r = s.execute("embroidery.digitize", &json!({})).unwrap();
            let info = s.execute("embroidery.info", &json!({})).unwrap();
            let d = &info["design"];
            let score = (d["trims"].as_u64().unwrap(), d["stitches"].as_u64().unwrap(), r["findings"]["warning"].as_u64().unwrap());
            eprintln!("{name}: trims {} stitches {} warnings {} ({r})", score.0, score.1, score.2);
            assert_eq!(r["findings"]["error"], 0, "{name}: {r}");
            assert!(score.0 <= trims && score.1 <= stitches && score.2 <= warnings, "{name} got worse: {score:?} {r}");
            assert!(score.1 > 100, "{name} sewed nothing: {r}");
        }
    }

    #[test]
    fn a_photo_or_gradient_is_refused() {
        let mut s = session();
        place(&mut s, &vectorcraft_trace::Raster::from_fn(200, 200, |x, y| [(x + y) as u8, x as u8, y as u8, 255]));
        let r = s.execute("embroidery.digitize", &json!({}));
        assert!(r.is_err(), "{r:?}");
        // Refused, nothing changed.
        assert_eq!(s.execute("embroidery.info", &json!({})).unwrap()["design"]["stitches"], 0);
    }

    #[test]
    fn bad_params_are_refused() {
        let mut s = session();
        assert!(s.execute("embroidery.digitize", &json!({})).is_err(), "nothing selected");
        logo(&mut s);
        for p in [json!({"colors": 1}), json!({"colors": 11}), json!({"minArea": -1}), json!({"order": "random"}), json!({"id": 999_999})] {
            assert!(s.execute("embroidery.digitize", &p).is_err(), "{p}");
        }
    }
}
