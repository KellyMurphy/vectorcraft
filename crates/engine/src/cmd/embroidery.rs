//! Object › Embroidery: stitch settings on paths and compound paths (`embroidery.set`,
//! `embroidery.clear`), the design they make (`embroidery.info`, `embroidery.stitches`, the
//! Stitch Preview), and DST files (`document.export {format: dst}`, `document.open` of a `.dst`;
//! see `fileio::embroidery`).

use serde_json::{Value, json};
use vectorcraft_doc::{Document, EmbroiderySetup, NodeId, NodeKind, StitchKind, StitchSettings, Underlay};
use vectorcraft_embroidery::Plan;

use super::overprint::edit_counted;
use super::*;

const SET: &str = "embroidery.set";
/// The most points `embroidery.stitches` returns unless asked for fewer.
const STITCHES_LIMIT: usize = 500_000;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "embroidery.set",
            "Stitch Settings",
            [],
            None,
            "{type?: running|bean|satin|zigzag|satinShape|fill|manual (default: the object's own, else running), length?: mm (stitch length, 0.3–12; running/bean 2.5, fill 4), spacing?: mm (satin zigzag, satin shape, zigzag stitch or fill row spacing, 0.1–10; satin and fill 0.4, zigzag 2), angle?: degrees (fill rows, counter-clockwise), width?: mm (satin column or zigzag, 0.3–12; satin: the stroke width, zigzag: 3), thread?: \"#rrggbb\" | [r, g, b] (default: the stroke for path stitches, the fill for fills), pull?: mm (satin and satin shape pull compensation, 0–3; 0), underlay?: none|center|edge|zigzag (satin) | none|center (satin shape) | none|fill (fill), stagger?: 2–8 (fill rows before the needle holes line up again; 3), passes?: 3|5|7|9 (bean; 3), border?: mm (fill: a satin border over the edges, 0.5–6) | null (none), trimBefore?: bool, trimAfter?: bool (cut the thread before / after the object, however close the next stitch), ids?} stitch the paths and compound paths in ids or the selection (groups: those inside), as one undo step; null resets length, spacing, width, thread, pull, stagger or passes to the default → {changed}",
            has_doc,
            set
        ),
        cmd!(
            "embroidery.setup",
            "Embroidery Setup",
            [],
            None,
            "{trimJumpsOver?: mm (0.5–100; 3) | null (never trim on length: only thread changes and objects' trimBefore/trimAfter cut the thread), stitchMovesUnder?: mm (0–5; 1: moves to the next stitch up to this long are sewn, longer ones jump)} the document's travel settings, as one undo step → {trimJumpsOver, stitchMovesUnder}",
            has_doc,
            setup
        ),
        cmd!(
            "embroidery.clear",
            "Remove Stitches",
            [],
            None,
            "{ids?} the objects in ids or the selection (groups: those inside) are no longer stitched, as one undo step → {changed}",
            has_doc,
            clear
        ),
        cmd!(
            query "embroidery.info",
            "Embroidery Info",
            [],
            None,
            "{ids?} → {design: {objects, stitches, jumps, trims, colorChanges, threads: [\"#rrggbb\"], widthMm, heightMm, warnings}, setup: {trimJumpsOver, stitchMovesUnder} (embroidery.setup), selection: {stitched, unstitched, type, length, spacing, angle, width, thread, pull, underlay, stagger, passes, border, trimBefore, trimAfter} (each null when the stitched objects differ, or none is)}",
            has_doc,
            info
        ),
        cmd!(
            query "embroidery.stitches",
            "Embroidery Stitches",
            [],
            None,
            "{limit?: 500000 (most points)} the design's stitches as the machine sews them → {threads: [\"#rrggbb\"], runs: [{thread, points: [[x, y], …]}] (mm, document coordinates, y down; a run is sewn without a jump or trim), jumps: [[x0, y0, x1, y1]], truncated}",
            has_doc,
            stitches
        ),
    ]
}

pub(crate) fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

/// A thread colour: `"#rrggbb"` (or `rrggbb`) or `[r, g, b]` (0–255).
fn parse_thread(v: &Value) -> Option<[u8; 3]> {
    match v {
        Value::String(s) => {
            let s = s.trim().trim_start_matches('#');
            if s.len() != 6 || !s.is_ascii() {
                return None;
            }
            let c = |i: usize| s.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok());
            Some([c(0)?, c(2)?, c(4)?])
        }
        Value::Array(a) if a.len() == 3 => {
            let c = |i: usize| a.get(i)?.as_u64().and_then(|v| u8::try_from(v).ok());
            Some([c(0)?, c(1)?, c(2)?])
        }
        _ => None,
    }
}

/// An optional measurement param: absent → leave it, `null` → the default, a number → checked
/// against `range`.
fn measure(p: &Value, key: &str, range: std::ops::RangeInclusive<f64>) -> Result<Option<Option<f64>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => match v.as_f64() {
            Some(x) if range.contains(&x) => Ok(Some(Some(x))),
            _ => Err(bad(SET, format!("`{key}` must be a number from {} to {} (mm), or null", range.start(), range.end()))),
        },
    }
}

/// The edits `embroidery.set` asks for.
struct Edits {
    kind: Option<StitchKind>,
    length: Option<Option<f64>>,
    spacing: Option<Option<f64>>,
    width: Option<Option<f64>>,
    angle: Option<f64>,
    thread: Option<Option<[u8; 3]>>,
    pull: Option<Option<f64>>,
    underlay: Option<Underlay>,
    stagger: Option<Option<u8>>,
    passes: Option<Option<u8>>,
    trim_before: Option<bool>,
    trim_after: Option<bool>,
    border: Option<Option<f64>>,
}

/// An optional true/false param.
fn flag(p: &Value, key: &str) -> Result<Option<bool>> {
    p.get(key).map(|v| v.as_bool().ok_or_else(|| bad(SET, format!("`{key}` must be true or false")))).transpose()
}

/// An optional whole-number param in `range` (odd only when `odd`): absent → leave it, `null` →
/// the default.
fn count(p: &Value, key: &str, range: std::ops::RangeInclusive<u8>, odd: bool) -> Result<Option<Option<u8>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => match v.as_u64().and_then(|n| u8::try_from(n).ok()) {
            Some(n) if range.contains(&n) && (!odd || n % 2 == 1) => Ok(Some(Some(n))),
            _ => {
                let what = if odd { "an odd whole number" } else { "a whole number" };
                Err(bad(SET, format!("`{key}` must be {what} from {} to {}, or null", range.start(), range.end())))
            }
        },
    }
}

impl Edits {
    fn parse(p: &Value) -> Result<Self> {
        let kind = match p.get("type") {
            None | Some(Value::Null) => None,
            Some(v) => Some(v.as_str().and_then(StitchKind::parse).ok_or_else(|| {
                bad(SET, "`type` must be running, bean, satin, zigzag, satinShape, fill or manual (embroidery.clear removes stitches)")
            })?),
        };
        let angle = match p.get("angle") {
            None => None,
            Some(v) => Some(v.as_f64().filter(|a| a.is_finite()).ok_or_else(|| bad(SET, "`angle` must be a number of degrees"))?),
        };
        let thread = match p.get("thread") {
            None => None,
            Some(Value::Null) => Some(None),
            Some(v) => Some(Some(parse_thread(v).ok_or_else(|| bad(SET, "`thread` must be \"#rrggbb\", [r, g, b] or null"))?)),
        };
        let underlay = match p.get("underlay") {
            None => None,
            Some(Value::Null) => Some(Underlay::None),
            Some(v) => Some(
                v.as_str()
                    .and_then(Underlay::parse)
                    .ok_or_else(|| bad(SET, "`underlay` must be none, center, edge, zigzag (satin) or fill (fill)"))?,
            ),
        };
        Ok(Self {
            kind,
            pull: measure(p, "pull", StitchSettings::PULL)?,
            underlay,
            stagger: count(p, "stagger", StitchSettings::STAGGER, false)?,
            passes: count(p, "passes", StitchSettings::PASSES, true)?,
            trim_before: flag(p, "trimBefore")?,
            border: measure(p, "border", StitchSettings::BORDER)?,
            trim_after: flag(p, "trimAfter")?,
            length: measure(p, "length", StitchSettings::LENGTH)?,
            spacing: measure(p, "spacing", StitchSettings::SPACING)?,
            width: measure(p, "width", StitchSettings::WIDTH)?,
            angle,
            thread,
        })
    }

    fn apply(&self, old: Option<&StitchSettings>) -> StitchSettings {
        let mut s = old.cloned().unwrap_or_default();
        if let Some(k) = self.kind {
            s.kind = k;
        }
        if let Some(v) = self.length {
            s.length = v;
        }
        if let Some(v) = self.spacing {
            s.spacing = v;
        }
        if let Some(v) = self.width {
            s.width = v;
        }
        if let Some(v) = self.angle {
            s.angle = v % 360.0;
        }
        if let Some(v) = self.thread {
            s.thread = v;
        }
        if let Some(v) = self.pull {
            s.pull = v;
        }
        if let Some(v) = self.underlay {
            s.underlay = v;
        }
        if let Some(v) = self.stagger {
            s.stagger = v;
        }
        if let Some(v) = self.passes {
            s.passes = v;
        }
        if let Some(v) = self.border {
            s.border = v;
        }
        if let Some(v) = self.trim_before {
            s.trim_before = v;
        }
        if let Some(v) = self.trim_after {
            s.trim_after = v;
        }
        s
    }
}

/// The paths and compound paths `ids` stand for (groups and layers: those inside; not the paths
/// inside a compound path, which stitches as one), guides left out.
fn stitchable(s: &Session, ids: &[NodeId]) -> Result<Vec<NodeId>> {
    let d = &s.doc()?.doc;
    Ok(leaf_targets(s, ids)?
        .into_iter()
        .filter(|id| matches!(d.node(*id).map(|n| &n.kind), Some(NodeKind::Path { guide: false, .. } | NodeKind::Compound { .. })))
        .collect())
}

fn set(s: &mut Session, p: &Value) -> Result<Value> {
    let edits = Edits::parse(p)?;
    let ids = stitchable(s, &targets(s, p)?)?;
    if ids.is_empty() {
        return Err(bad(SET, "select paths or compound paths to stitch"));
    }
    // An underlay asked for must suit the stitch every target ends up with.
    if let Some(u) = edits.underlay.filter(|u| *u != Underlay::None) {
        let d = &s.doc()?.doc;
        if let Some(k) =
            ids.iter().map(|id| edits.apply(d.node(*id).and_then(|n| n.stitch.as_deref())).kind).find(|k| !Underlay::for_kind(*k).contains(&u))
        {
            return Err(bad(SET, format!("{} stitches have no {} underlay", k.id(), u.id())));
        }
    }
    let changed = edit_counted(s, "Stitch Settings", |d| {
        let mut n = 0;
        for id in &ids {
            let Some(node) = d.node_mut(*id) else { continue };
            let new = edits.apply(node.stitch.as_deref());
            if node.stitch.as_deref() != Some(&new) {
                node.stitch = Some(Box::new(new));
                n += 1;
            }
        }
        Ok(n)
    })?;
    Ok(json!({ "changed": changed }))
}

fn setup_json(e: &EmbroiderySetup) -> Value {
    json!({ "trimJumpsOver": e.trim_over(), "stitchMovesUnder": e.stitch_under() })
}

fn setup(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.setup";
    let mut new = s.doc()?.doc.embroidery.clone();
    match p.get("trimJumpsOver") {
        None => {}
        Some(Value::Null) => new.trim_jumps_over = None,
        Some(v) => match v.as_f64() {
            Some(x) if EmbroiderySetup::TRIM.contains(&x) => new.trim_jumps_over = Some(x),
            _ => return Err(bad(C, "`trimJumpsOver` must be a number from 0.5 to 100 (mm), or null (never trim on length)")),
        },
    }
    if let Some(v) = p.get("stitchMovesUnder") {
        match v.as_f64() {
            Some(x) if EmbroiderySetup::STITCH_MOVES.contains(&x) => new.stitch_moves_under = x,
            _ => return Err(bad(C, "`stitchMovesUnder` must be a number from 0 to 5 (mm)")),
        }
    }
    if new != s.doc()?.doc.embroidery {
        let e = new.clone();
        edit_counted(s, "Embroidery Setup", |d| {
            d.embroidery = e;
            Ok(1)
        })?;
    }
    Ok(setup_json(&new))
}

fn clear(s: &mut Session, p: &Value) -> Result<Value> {
    let ids = leaf_targets(s, &targets(s, p)?)?;
    let changed = edit_counted(s, "Remove Stitches", |d| {
        let mut n = 0;
        for id in &ids {
            if let Some(node) = d.node_mut(*id)
                && node.stitch.take().is_some()
            {
                n += 1;
            }
        }
        Ok(n)
    })?;
    Ok(json!({ "changed": changed }))
}

/// The one value every item shares (`null` when they differ or there are none).
fn same<T: PartialEq + serde::Serialize>(mut it: impl Iterator<Item = T>) -> Value {
    let Some(first) = it.next() else { return Value::Null };
    if it.all(|v| v == first) { json!(first) } else { Value::Null }
}

/// `doc`'s design summary.
pub(crate) fn design(doc: &Document, plan: &Plan) -> Value {
    let st = plan.stats();
    let mut objects = 0;
    doc.walk(|n| objects += usize::from(n.stitch.is_some()));
    let (w, h) = st.bounds.map(|b| (b[2] - b[0], b[3] - b[1])).unwrap_or((0.0, 0.0));
    json!({
        "objects": objects,
        "stitches": st.stitches,
        "jumps": st.jumps,
        "trims": st.trims,
        "colorChanges": st.color_changes,
        "threads": plan.threads.iter().map(|t| hex(*t)).collect::<Vec<_>>(),
        "widthMm": (w * 10.0).round() / 10.0,
        "heightMm": (h * 10.0).round() / 10.0,
        "warnings": plan.warnings,
    })
}

fn info(s: &mut Session, p: &Value) -> Result<Value> {
    let ids = stitchable(s, &targets(s, p)?)?;
    let doc = &s.doc()?.doc;
    let plan = vectorcraft_embroidery::plan(doc);
    let all: Vec<&StitchSettings> = ids.iter().filter_map(|id| doc.node(*id)?.stitch.as_deref()).collect();
    let selection = json!({
        "stitched": all.len(),
        "unstitched": ids.len() - all.len(),
        "type": same(all.iter().map(|s| s.kind.id())),
        "length": same(all.iter().map(|s| s.length())),
        "spacing": same(all.iter().map(|s| s.spacing())),
        "angle": same(all.iter().map(|s| s.angle())),
        "width": same(all.iter().map(|s| s.width)),
        "thread": same(all.iter().map(|s| s.thread.map(hex))),
        "pull": same(all.iter().map(|s| s.pull())),
        "underlay": same(all.iter().map(|s| s.underlay().id())),
        "stagger": same(all.iter().map(|s| s.stagger())),
        "passes": same(all.iter().map(|s| s.passes())),
        "border": same(all.iter().map(|s| s.border())),
        "trimBefore": same(all.iter().map(|s| s.trim_before)),
        "trimAfter": same(all.iter().map(|s| s.trim_after)),
    });
    Ok(json!({ "design": design(doc, &plan), "setup": setup_json(&doc.embroidery), "selection": selection }))
}

fn stitches(s: &mut Session, p: &Value) -> Result<Value> {
    let limit = p.get("limit").and_then(Value::as_u64).map_or(STITCHES_LIMIT, |l| usize::try_from(l).unwrap_or(usize::MAX).min(STITCHES_LIMIT));
    let plan = vectorcraft_embroidery::plan(&s.doc()?.doc);
    let mut left = limit;
    let mut truncated = false;
    let mut runs = vec![];
    for (thread, pts) in plan.runs() {
        if pts.len() > left {
            truncated = true;
            break;
        }
        left -= pts.len();
        let r = |v: f64| (v * 1000.0).round() / 1000.0;
        runs.push(json!({ "thread": thread, "points": pts.iter().map(|q| [r(q.x), r(q.y)]).collect::<Vec<_>>() }));
    }
    let mut jumps = vec![];
    let mut at: Option<(f64, f64)> = None;
    for n in &plan.needles {
        if n.cmd == vectorcraft_embroidery::Cmd::Jump
            && let Some((x, y)) = at
        {
            jumps.push([x, y, n.x, n.y]);
        }
        at = Some((n.x, n.y));
    }
    jumps.truncate(limit);
    Ok(json!({
        "threads": plan.threads.iter().map(|t| hex(*t)).collect::<Vec<_>>(),
        "runs": runs,
        "jumps": jumps,
        "truncated": truncated,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threads_parse_from_hex_and_arrays() {
        assert_eq!(parse_thread(&json!("#ff8000")), Some([255, 128, 0]));
        assert_eq!(parse_thread(&json!("FF8000")), Some([255, 128, 0]));
        assert_eq!(parse_thread(&json!([1, 2, 3])), Some([1, 2, 3]));
        assert_eq!(parse_thread(&json!([1, 2, 300])), None);
        assert_eq!(parse_thread(&json!("#ff80")), None);
        assert_eq!(parse_thread(&json!("#ffé00")), None);
    }
}
