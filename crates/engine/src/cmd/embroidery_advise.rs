//! The density adviser (E8.7): the design's density map (`embroidery.density`, Stitch Preview's
//! heat overlay) and one-click fixes for the checker's findings (`embroidery.fixes`,
//! `embroidery.fix`). A fix runs an ordinary command, so it is one undo step.

use serde_json::{Value, json};
use vectorcraft_doc::NodeId;
use vectorcraft_embroidery::{PT_MM, advise, check};

use super::{CommandSpec, Result, Session, bad, cmd, has_doc, str_param};

/// The most cells `embroidery.density` lists (the densest first).
const MAX_CELLS: usize = 5000;

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "embroidery.density",
            "Stitch Density",
            ["Window", "Embroidery"],
            None,
            "{min?: the fewest needle holes a cell listed has (default 1)} the design's density map: how many needle holes go into each square of the design (what the checker's dense rule counts) → {cellMm, limit: the most holes a cell takes before it is too dense, peak: the most in any cell, perMm2: {peak, limit}, cells: [{x, y (pt: the cell's top left), size (pt), holes}] (the densest first; at most 5000)}",
            has_doc,
            density
        ),
        cmd!(
            query "embroidery.fixes",
            "Suggested Fixes",
            ["Window", "Embroidery"],
            None,
            "{object: the finding's object id (embroidery.check), rule?: its rule (default every finding of the object)} the suggested fixes for the checker's findings on an object, each tried on a copy of the design first: offered only when it clears the finding or, for a dense spot, thins it, without making a new finding. Dense spots: lower the top object's density, cut away the covered rows of a fill below (keepCovered off), a lighter underlay, delete a stitched copy of an object below; long stitches: trim before the object → {fixes: [{rule, fix: lower-density|remove-overlap|lighter-underlay|drop-duplicate|trim-before, label, object: the object it changes, clears: whether it clears the finding (else it only thins the spot), holes: the needle holes left in the spot, command, params: what embroidery.fix runs}]}",
            has_doc,
            fixes
        ),
        cmd!(
            "embroidery.fix",
            "Fix",
            ["Window", "Embroidery"],
            None,
            "{object: the finding's object id, rule?: its rule, fix?: which (embroidery.fixes' fix; default the first offered)} make a suggested fix for a checker finding, as one undo step, then check the design again → {fix, label, object, clears, findings: {error, warning, note} after it}",
            has_doc,
            fix
        ),
    ]
}

fn density(s: &mut Session, p: &Value) -> Result<Value> {
    let min = p.get("min").and_then(Value::as_u64).unwrap_or(1).max(1);
    let plan = s.stitch_plan(&s.doc()?.doc);
    let map = check::Density::of(&plan);
    let size = check::DENSITY_CELL_MM / PT_MM;
    let mut cells: Vec<((i64, i64), usize)> = map.counts.iter().map(|(c, n)| (*c, *n)).filter(|(_, n)| *n as u64 >= min).collect();
    cells.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    cells.truncate(MAX_CELLS);
    let r = |v: f64| (v * 100.0).round() / 100.0;
    let list: Vec<Value> =
        cells.iter().map(|((cx, cy), n)| json!({"x": r(*cx as f64 * size), "y": r(*cy as f64 * size), "size": r(size), "holes": n})).collect();
    let peak = map.peak();
    Ok(json!({
        "cellMm": check::DENSITY_CELL_MM,
        "limit": check::DENSITY_MAX,
        "peak": peak,
        "perMm2": {"peak": r(check::Density::per_mm2(peak)), "limit": r(check::Density::per_mm2(check::DENSITY_MAX))},
        "cells": list,
    }))
}

/// The `object` param, and the checker's findings on it (of `rule`, if given) with their fixes.
fn fixes_for(s: &Session, p: &Value, c: &str) -> Result<Vec<(check::Finding, advise::Fix)>> {
    let object = p
        .get("object")
        .and_then(Value::as_u64)
        .map(NodeId)
        .ok_or_else(|| bad(c, "`object` must be the id of an object the checker found something on"))?;
    let rule = str_param(p, "rule");
    let doc = &s.doc()?.doc;
    if doc.node(object).is_none() {
        return Err(bad(c, format!("no object {}", object.0)));
    }
    let plan = s.stitch_plan(doc);
    let found: Vec<check::Finding> =
        check::check(doc, &plan).into_iter().filter(|f| f.object == Some(object) && rule.is_none_or(|r| r == f.rule)).collect();
    if found.is_empty() {
        return Err(bad(c, "the checker finds nothing on that object (embroidery.check lists what it finds)"));
    }
    let mut replan = |d: &vectorcraft_doc::Document| s.stitch_plan(d);
    Ok(found.iter().flat_map(|f| advise::fixes(doc, &plan, f, &mut replan).into_iter().map(move |x| (f.clone(), x))).collect())
}

fn fix_json(f: &check::Finding, x: &advise::Fix) -> Value {
    let (command, params) = x.command();
    json!({"rule": f.rule, "fix": x.id, "label": x.label, "object": x.object.0, "clears": x.clears, "holes": x.holes, "command": command, "params": params})
}

fn fixes(s: &mut Session, p: &Value) -> Result<Value> {
    let list: Vec<Value> = fixes_for(s, p, "embroidery.fixes")?.iter().map(|(f, x)| fix_json(f, x)).collect();
    Ok(json!({ "fixes": list }))
}

fn fix(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.fix";
    let which = str_param(p, "fix");
    let all = fixes_for(s, p, C)?;
    let Some((_, x)) = all.into_iter().find(|(_, x)| which.is_none_or(|w| w == x.id)) else {
        return Err(bad(C, "no fix offered for that finding: change the design by hand (the rule's help says how)"));
    };
    let (command, params) = x.command();
    s.execute(command, &params)?;
    let doc = &s.doc()?.doc;
    let after = check::check(doc, &s.stitch_plan(doc));
    let count = |sev: check::Severity| after.iter().filter(|f| f.severity == sev).count();
    Ok(json!({
        "fix": x.id, "label": x.label, "object": x.object.0, "clears": x.clears,
        "findings": {"error": count(check::Severity::Error), "warning": count(check::Severity::Warning), "note": count(check::Severity::Note)},
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Five filled squares stacked on one spot, each keeping the rows under the next: too dense.
    /// Gives the top one's id.
    fn dense_session() -> (Session, u64) {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 400, "height": 400})).unwrap();
        let mut top = 0;
        for _ in 0..5 {
            let r = s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 60, "height": 60})).unwrap();
            s.execute("embroidery.set", &json!({"type": "fill", "keepCovered": true, "underlay": "auto"})).unwrap();
            top = r["id"].as_u64().unwrap();
        }
        (s, top)
    }

    fn dense(s: &mut Session) -> Vec<Value> {
        let check = s.execute("embroidery.check", &json!({})).unwrap();
        check["findings"].as_array().unwrap().iter().filter(|f| f["rule"] == "dense").cloned().collect()
    }

    #[test]
    fn the_density_map_lists_the_densest_cells_first() {
        let (mut s, _) = dense_session();
        let d = s.execute("embroidery.density", &json!({})).unwrap();
        let cells = d["cells"].as_array().unwrap();
        assert!(d["peak"].as_u64().unwrap() > d["limit"].as_u64().unwrap(), "{}", d["peak"]);
        assert_eq!(cells.first().unwrap()["holes"], d["peak"]);
        assert!(cells.windows(2).all(|w| w[0]["holes"].as_u64() >= w[1]["holes"].as_u64()));
        let few = s.execute("embroidery.density", &json!({"min": 41})).unwrap();
        assert!(few["cells"].as_array().unwrap().iter().all(|c| c["holes"].as_u64().unwrap() >= 41));
    }

    #[test]
    fn fixes_clear_a_dense_spot_each_one_undo_step() {
        let (mut s, top) = dense_session();
        let f = s.execute("embroidery.fixes", &json!({"object": top, "rule": "dense"})).unwrap();
        let list = f["fixes"].as_array().unwrap();
        assert!(!list.is_empty(), "{f}");
        for x in list {
            assert!(x["holes"].as_u64().is_some() && x["command"].as_str().is_some() && x["params"]["ids"].is_array(), "{x}");
        }
        // Fixed one at a time (the adviser re-checks after each), the spot clears.
        let mut steps = 0;
        while let Some(f) = dense(&mut s).first().cloned() {
            assert!(steps < 8, "still dense after eight fixes");
            s.execute("embroidery.fix", &json!({"object": f["object"], "rule": "dense"})).unwrap();
            steps += 1;
        }
        assert!(steps >= 1);
        s.execute("edit.undo", &json!({})).unwrap();
        assert!(!dense(&mut s).is_empty(), "the last fix is undone in one step");
        // Bad params.
        assert!(s.execute("embroidery.fixes", &json!({})).is_err());
        assert!(s.execute("embroidery.fix", &json!({"object": top, "fix": "nonsense"})).is_err());
        assert!(s.execute("embroidery.fixes", &json!({"object": 999_999})).is_err());
    }
}
