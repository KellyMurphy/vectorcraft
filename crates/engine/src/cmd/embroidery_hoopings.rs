//! Multi-hoop designs (E11.4b): a design too big for its hoop split into a grid of hoopings
//! (`EmbroiderySetup::hoopings`), each sewn as a design of its own with alignment marks on the
//! seams; `document.export` then writes a file per hooping (see `fileio::embroidery`).

use serde_json::{Value, json};
use vectorcraft_doc::{Document, Hoopings};
use vectorcraft_embroidery::{Plan, hoopings, hoops};

use super::overprint::edit_counted;
use super::{CommandSpec, Result, Session, bad, cmd, has_doc, str_param};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "embroidery.splitHoops",
            "Split into Hoopings",
            ["Window", "Embroidery"],
            None,
            "{hoop?: 4x4|5x7|6x10|8x8|8x12|cap (also made the document's hoop; default: the document's), overlap?: mm (0–10; 2, or the split's: how far each hooping sews past its seams so the pieces meet), marks?: bool (true: running-stitch crosses on each seam, sewn last by the hooping before and first by the next, to line the fabric up by), basting?: bool (false: a running-stitch box round each hooping's field, sewn first)} or {clear: true} split a design too big for the hoop into a grid of hoopings, each fitting the hoop (turned by hoopRotation), sewn row by row from the top, each row from the left; or sew it in one hooping again. A stitch belongs to the hooping its middle lies in; fill rows and satin columns are cut where they cross a seam and locked. Splitting again (after edits, or with another overlap) remakes the grid. One undo step; document.export then writes one file per hooping (`name-hoop1.dst`…) → as embroidery.hoopings",
            has_doc,
            split_hoops
        ),
        cmd!(
            query "embroidery.hoopings",
            "Hoopings",
            ["Window", "Embroidery"],
            None,
            "{} the document's split into hoopings → {hoopings: null (sewn in one hooping) | {unsupported: true, data} (from a newer VectorCraft: kept, not used) | {columns, rows (the grid's size), overlap, marks, basting, pieces: [{number (sewing order, from 1; areas with nothing to sew are left out), area: [minX, minY, maxX, maxY] (mm), stitches (what the hooping sews, its marks and overlap included), own (the design's stitches whose middle lies in its area), fits (its file fits the hoop; null: no hoop chosen)}]}}",
            has_doc,
            hoopings_query
        ),
    ]
}

fn r(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// The document's hoopings as `embroidery.hoopings` gives them.
pub(crate) fn hoopings_json(doc: &Document, plan: &Plan) -> Value {
    let Some(h) = doc.embroidery.hoopings.as_deref() else { return Value::Null };
    if let Some(raw) = &h.unknown {
        return json!({"unsupported": true, "data": raw});
    }
    if hoopings::areas(h).is_none() {
        // A grid this version can't use (hand-edited, or from a newer VectorCraft).
        return json!({"unsupported": true, "data": h});
    }
    let hoop = doc.embroidery.hoop.as_deref().filter(|h| h.unknown.is_none());
    let rotation = doc.embroidery.hoop_rotation();
    let field = hoop.map_or((0.0, 0.0), |hoop| hoopings::usable_field(hoop, rotation));
    let pieces: Vec<Value> = hoopings::split(plan, h, field, doc.embroidery.trim_over())
        .iter()
        .enumerate()
        .map(|(i, p)| {
            json!({
                "number": i + 1,
                "area": p.area.map(r),
                "stitches": p.plan.stats().stitches,
                "own": p.own,
                "fits": hoop.map(|hoop| hoopings::fits(p, hoop, rotation)),
            })
        })
        .collect();
    json!({
        "columns": h.columns.len() - 1,
        "rows": h.rows.len() - 1,
        "overlap": hoopings::overlap(h),
        "marks": h.marks,
        "basting": h.basting,
        "pieces": pieces,
    })
}

fn hoopings_query(s: &mut Session, _: &Value) -> Result<Value> {
    let doc = &s.doc()?.doc;
    let plan = s.stitch_plan(doc);
    Ok(json!({ "hoopings": hoopings_json(doc, &plan) }))
}

fn split_hoops(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.splitHoops";
    if p.get("clear").and_then(Value::as_bool) == Some(true) {
        if s.doc()?.doc.embroidery.hoopings.is_some() {
            edit_counted(s, "Join Hoopings", |d| {
                d.embroidery.hoopings = None;
                Ok(1)
            })?;
        }
        return hoopings_query(s, &Value::Null);
    }
    let doc = &s.doc()?.doc;
    let named = match str_param(p, "hoop") {
        Some(id) => Some(
            hoops::builtin(&id.trim().to_ascii_lowercase()).ok_or_else(|| bad(C, format!("`hoop` must be one of {}", hoops::BUILTIN.join(", "))))?,
        ),
        None => None,
    };
    let hoop = named
        .clone()
        .or_else(|| doc.embroidery.hoop.as_deref().filter(|h| h.unknown.is_none()).cloned())
        .ok_or_else(|| bad(C, "choose a hoop first (embroidery.setHoop), or name one (`hoop`)"))?;
    let old = doc.embroidery.hoopings.as_deref().filter(|h| h.unknown.is_none());
    let overlap = match p.get("overlap") {
        None => old.map_or(Hoopings::DEFAULT_OVERLAP, hoopings::overlap),
        Some(v) => v.as_f64().filter(|x| Hoopings::OVERLAP.contains(x)).ok_or_else(|| bad(C, "`overlap` must be a number from 0 to 10 (mm)"))?,
    };
    let flag = |key: &str, default: bool| -> Result<bool> {
        match p.get(key) {
            None => Ok(default),
            Some(v) => v.as_bool().ok_or_else(|| bad(C, format!("`{key}` must be true or false"))),
        }
    };
    let marks = flag("marks", old.is_none_or(|h| h.marks))?;
    let basting = flag("basting", old.is_some_and(|h| h.basting))?;
    let plan = s.stitch_plan(doc);
    let b = plan.stats().bounds.ok_or_else(|| bad(C, "nothing to stitch: give paths stitch settings first"))?;
    let rotation = doc.embroidery.hoop_rotation();
    let (w, h) = hoops::as_sewn(b[2] - b[0], b[3] - b[1], rotation);
    if hoops::fit(w, h, &hoop) == hoops::Fit::Fits {
        return Err(bad(C, format!("the design ({w:.0} × {h:.0} mm as sewn) fits the hoop in one hooping: nothing to split")));
    }
    let grid = hoopings::grid(b, hoopings::usable_field(&hoop, rotation), overlap).map_err(|e| bad(C, e))?;
    let split = Hoopings { marks, basting, ..grid };
    edit_counted(s, "Split into Hoopings", |d| {
        if let Some(h) = named {
            d.embroidery.hoop = Some(Box::new(h));
        }
        d.embroidery.hoopings = Some(Box::new(split));
        Ok(1)
    })?;
    hoopings_query(s, &Value::Null)
}
