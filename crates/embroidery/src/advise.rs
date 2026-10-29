//! The density adviser (E8.7): one-click fixes for the design checker's findings. A fix is an
//! ordinary command (`embroidery.set` or `edit.clear`), so it is undoable; it is offered only when,
//! tried on a copy of the design, it clears its finding, or for a dense spot at least thins it,
//! without making a new one. A spot of several stacked layers may need two fixes: the adviser
//! re-checks after each.

use std::collections::HashSet;

use serde_json::{Map, Value, json};
use vectorcraft_doc::{Document, NodeId, StitchKind, StitchSettings, Underlay};

use crate::check::{self, DENSITY_MAX, Density, Finding};
use crate::{Cmd, Plan};

/// The most fixes tried on a copy of the design for one finding (each is a new plan).
const MAX_TRIES: usize = 12;
/// The most fixes offered for one finding.
const MAX_FIXES: usize = 4;
/// Density is lowered at most to this many times the stitch type's default spacing: sparser
/// doesn't cover.
const SPARSEST: f64 = 2.5;

/// What a fix does to its object.
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    /// Set these stitch settings (`embroidery.set` params).
    Set(Map<String, Value>),
    /// Delete the object (a stitched copy of another).
    Delete,
}

/// A suggested fix for a finding.
#[derive(Clone, Debug, PartialEq)]
pub struct Fix {
    /// What kind of fix: `lower-density`, `remove-overlap`, `lighter-underlay`, `drop-duplicate`,
    /// `trim-before`.
    pub id: &'static str,
    /// What it does, for a menu (names its object).
    pub label: String,
    /// The object it changes (not always the finding's: overlap is removed from the one below).
    pub object: NodeId,
    pub change: Change,
    /// Whether it clears the finding (else it only thins the spot).
    pub clears: bool,
    /// The needle holes left in the finding's spot (dense findings).
    pub holes: Option<usize>,
}

impl Fix {
    /// The command that makes the fix, and its params.
    pub fn command(&self) -> (&'static str, Value) {
        match &self.change {
            Change::Set(m) => {
                let mut p = m.clone();
                p.insert("ids".into(), json!([self.object.0]));
                ("embroidery.set", Value::Object(p))
            }
            Change::Delete => ("edit.clear", json!({ "ids": [self.object.0] })),
        }
    }

    /// `doc` with the fix made (none: its object is gone, isn't stitched, or can't take it).
    pub fn apply(&self, doc: &Document) -> Option<Document> {
        let mut d = doc.clone();
        match &self.change {
            Change::Delete => {
                d.remove(self.object).ok()?;
            }
            Change::Set(m) => {
                let n = d.node_mut(self.object)?;
                let mut s = n.extras().stitch.as_deref()?.clone();
                for (k, v) in m {
                    let p = crate::params::param(k)?;
                    p.parse(v).ok()?;
                    (p.set)(&mut s, v);
                }
                n.extras_mut().stitch = Some(Box::new(s));
            }
        }
        Some(d)
    }
}

/// The fixes for `finding` (one of `check::check(doc, plan)`) that clear it, or thin its spot,
/// without making a new finding: those that clear it first. `replan` plans a changed copy of `doc` (the session's cached planner).
pub fn fixes(doc: &Document, plan: &Plan, finding: &Finding, replan: &mut dyn FnMut(&Document) -> Plan) -> Vec<Fix> {
    let before: HashSet<(&str, Option<NodeId>)> = check::check(doc, plan).iter().map(|f| (f.rule, f.object)).collect();
    let mut out = vec![];
    let mut tries = 0;
    let spot = finding.at.map(Density::cell).filter(|_| finding.rule == "dense");
    let holes_before = spot.map(|c| Density::of(plan).get(c));
    'groups: for group in candidates(doc, plan, finding) {
        // The first of a group's alternatives (a density each lighter than the last) that clears
        // the finding, else the one that thins the spot most.
        let mut best: Option<Fix> = None;
        for mut fix in group {
            if tries >= MAX_TRIES {
                break 'groups;
            }
            let Some(d) = fix.apply(doc) else { continue };
            tries += 1;
            let p = replan(&d);
            let after = check::check(&d, &p);
            if after.iter().any(|f| !before.contains(&(f.rule, f.object))) {
                continue;
            }
            fix.holes = spot.map(|c| Density::of(&p).get(c));
            fix.clears = cleared(finding, &p, &after);
            let thinner = matches!((fix.holes, holes_before), (Some(a), Some(b)) if a < b);
            if fix.clears {
                best = Some(fix);
                break;
            }
            if thinner && best.as_ref().is_none_or(|b| fix.holes < b.holes) {
                best = Some(fix);
            }
        }
        out.extend(best);
    }
    out.sort_by_key(|f| (!f.clears, f.holes));
    out.truncate(MAX_FIXES);
    out
}

/// Whether `finding` is gone from a changed design (`plan`, its findings `after`).
fn cleared(finding: &Finding, plan: &Plan, after: &[Finding]) -> bool {
    match finding.rule {
        // The spot it named is no longer too dense.
        "dense" => finding.at.is_some_and(|at| Density::of(plan).get(Density::cell(at)) <= DENSITY_MAX),
        _ => !after.iter().any(|f| f.rule == finding.rule && f.object == finding.object),
    }
}

/// What the object is called in a menu: its name, else its stitch type and place in the sewing
/// order ("the fill sewn 3rd"), so stacked copies can be told apart.
fn name(doc: &Document, plan: &Plan, id: NodeId) -> String {
    if let Some(name) = doc.node(id).and_then(|n| n.name.clone()).filter(|s| !s.is_empty()) {
        return name;
    }
    let kind = doc.node(id).and_then(|n| n.extras().stitch.as_deref().map(|s| s.kind)).unwrap_or_default();
    let what = kind.label().to_lowercase();
    match plan.objects.iter().position(|o| o.id == id).map(|i| i + 1) {
        Some(n) => {
            let suffix = match (n % 10, n % 100) {
                (1, x) if x != 11 => "st",
                (2, x) if x != 12 => "nd",
                (3, x) if x != 13 => "rd",
                _ => "th",
            };
            format!("the {what} sewn {n}{suffix}")
        }
        None => format!("the {what}"),
    }
}

/// `s` as sewn: the document's fabric fills in what it leaves unset.
fn as_sewn(doc: &Document, s: &StitchSettings) -> StitchSettings {
    match doc.embroidery.fabric.as_deref() {
        Some(f) => crate::fabrics::fill_in(s, f),
        None => s.clone(),
    }
}

/// A lighter underlay than `u` for `kind`, if there is one.
fn lighter(kind: StitchKind, u: Underlay) -> Option<Underlay> {
    let satin = matches!(kind, StitchKind::Satin | StitchKind::Zigzag | StitchKind::SatinShape);
    match u {
        Underlay::None => None,
        Underlay::Zigzag if satin => Some(Underlay::Edge),
        Underlay::Auto | Underlay::Edge if satin => Some(Underlay::Center),
        _ => Some(Underlay::None),
    }
}

fn set(key: &str, v: Value) -> Change {
    let mut m = Map::new();
    m.insert(key.into(), v);
    Change::Set(m)
}

/// The fixes worth trying for `finding`, each a group of alternatives tried in turn.
fn candidates(doc: &Document, plan: &Plan, finding: &Finding) -> Vec<Vec<Fix>> {
    let settings = |id: NodeId| doc.node(id).and_then(|n| n.extras().stitch.as_deref().cloned()).filter(|s| s.unknown.is_none());
    match finding.rule {
        "dense" => {
            let Some(at) = finding.at else { return vec![] };
            let density = Density::of(plan);
            let spot = Density::cell(at);
            let total = density.get(spot);
            // Each object's needle holes in the spot, in sewing order.
            let stitches = |o: &crate::PlannedObject| -> Vec<&crate::Needle> {
                plan.needles
                    .get(o.needles.clone())
                    .unwrap_or_default()
                    .iter()
                    .filter(|n| n.cmd == Cmd::Stitch && n.x.is_finite() && n.y.is_finite())
                    .collect()
            };
            let here: Vec<(NodeId, usize)> = plan
                .objects
                .iter()
                .map(|o| (o.id, stitches(o).iter().filter(|n| Density::cell(kurbo::Point::new(n.x, n.y)) == spot).count()))
                .filter(|(_, own)| *own > 0)
                .collect();
            let mut out = vec![];
            // A stitched copy of an object below: drop it (the topmost copy, one fix at a time).
            for (j, (b, _)) in here.iter().enumerate().rev() {
                let same = |a: &NodeId| {
                    let (Some(oa), Some(ob)) = (plan.objects.iter().find(|o| o.id == *a), plan.objects.iter().find(|o| o.id == *b)) else {
                        return false;
                    };
                    let (sa, sb) = (stitches(oa), stitches(ob));
                    // The stitches alike, but for the travel into each.
                    sa.len() == sb.len() && sa.len() > 2 && sa.iter().zip(&sb).skip(2).all(|(p, q)| (p.x - q.x).hypot(p.y - q.y) < 0.05)
                };
                if here.iter().take(j).any(|(a, _)| same(a)) {
                    out.push(vec![Fix {
                        id: "drop-duplicate",
                        label: format!("Delete {}: a copy of one below", name(doc, plan, *b)),
                        object: *b,
                        change: Change::Delete,
                        clears: false,
                        holes: None,
                    }]);
                    break;
                }
            }
            // The top object's density lowered just enough.
            if let Some(&(top, own)) =
                here.iter().rev().find(|(id, _)| settings(*id).is_some_and(|s| crate::params::param("spacing").is_some_and(|p| p.applies(s.kind))))
                && let Some(s) = settings(top)
            {
                let others = total - own;
                let spacing = as_sewn(doc, &s).spacing();
                let sparsest = StitchSettings::new(s.kind).spacing() * SPARSEST;
                if others < DENSITY_MAX && spacing > 0.0 {
                    let need = own as f64 / (DENSITY_MAX - others) as f64;
                    let group: Vec<Fix> = [1.1, 1.3, 1.6]
                        .iter()
                        .map(|m| ((spacing * need * m) / 0.05).ceil() * 0.05)
                        .map(|v| (v * 100.0).round() / 100.0)
                        .filter(|v| *v > spacing && *v <= sparsest)
                        .map(|v| Fix {
                            id: "lower-density",
                            label: format!("Lower {}'s density to {v} mm", name(doc, plan, top)),
                            object: top,
                            change: set("spacing", json!(v)),
                            clears: false,
                            holes: None,
                        })
                        .collect();
                    out.push(group);
                }
            }
            // A fill below keeping its covered rows: cut them away.
            for &(id, _) in here.iter().take(here.len().saturating_sub(1)) {
                if settings(id).is_some_and(|s| s.kind == StitchKind::Fill && s.keep_covered) {
                    out.push(vec![Fix {
                        id: "remove-overlap",
                        label: format!("Cut away the covered rows of {}", name(doc, plan, id)),
                        object: id,
                        change: set("keepCovered", json!(false)),
                        clears: false,
                        holes: None,
                    }]);
                }
            }
            // A lighter underlay, from the top down.
            for &(id, _) in here.iter().rev() {
                if let Some(s) = settings(id)
                    && let Some(u) = lighter(s.kind, s.underlay)
                {
                    let what =
                        if u == Underlay::None { "no underlay".to_string() } else { format!("a lighter underlay ({})", u.label().to_lowercase()) };
                    out.push(vec![Fix {
                        id: "lighter-underlay",
                        label: format!("Give {} {what}", name(doc, plan, id)),
                        object: id,
                        change: set("underlay", json!(u.id())),
                        clears: false,
                        holes: None,
                    }]);
                }
            }
            out
        }
        // A long stitch that is the travel into the object: cut the thread instead.
        "long-stitch" => match finding.object.and_then(|id| settings(id).map(|s| (id, s))) {
            Some((id, s)) if !s.trim_before => {
                vec![vec![Fix {
                    id: "trim-before",
                    label: format!("Trim before {}", name(doc, plan, id)),
                    object: id,
                    change: set("trimBefore", json!(true)),
                    clears: false,
                    holes: None,
                }]]
            }
            _ => vec![],
        },
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{doc_with, poly, stitched};

    fn square(d: &mut Document, s: StitchSettings) -> vectorcraft_doc::Node {
        let mut n = stitched(d, poly(&[(0.0, 0.0), (60.0, 0.0), (60.0, 60.0), (0.0, 60.0)], true), s.kind, [0; 3]);
        n.extras_mut().stitch = Some(Box::new(s));
        n
    }

    /// Each fix offered for each density finding clears it or thins its spot, as it says.
    fn every_fix_works(d: &Document) -> Vec<&'static str> {
        let plan = crate::plan(d);
        let found = check::check(d, &plan);
        let dense: Vec<&Finding> = found.iter().filter(|f| f.rule == "dense").collect();
        assert!(!dense.is_empty(), "{found:?}");
        let mut kinds = vec![];
        for f in dense {
            for fix in fixes(d, &plan, f, &mut crate::plan) {
                let changed = fix.apply(d).unwrap();
                let p = crate::plan(&changed);
                let after = check::check(&changed, &p);
                assert_eq!(cleared(f, &p, &after), fix.clears, "{fix:?}");
                assert!(fix.holes.unwrap() < Density::of(&plan).get(Density::cell(f.at.unwrap())), "thinner: {fix:?}");
                let (cmd, params) = fix.command();
                assert!(matches!(cmd, "embroidery.set" | "edit.clear") && params["ids"][0] == json!(fix.object.0));
                kinds.push(fix.id);
            }
        }
        kinds
    }

    #[test]
    fn a_too_dense_top_fill_is_made_lighter() {
        // A fill of short stitches on close rows over one that keeps the rows under it.
        let d = doc_with(|d| {
            let below = square(d, StitchSettings { keep_covered: true, underlay: Underlay::Auto, ..StitchSettings::new(StitchKind::Fill) });
            let top = square(d, StitchSettings { spacing: Some(0.3), length: Some(1.0), ..StitchSettings::new(StitchKind::Fill) });
            vec![below, top]
        });
        let kinds = every_fix_works(&d);
        assert!(kinds.contains(&"lower-density"), "{kinds:?}");
        // Lowered just enough: not past the sparsest that covers.
        let plan = crate::plan(&d);
        let f = check::check(&d, &plan).into_iter().find(|f| f.rule == "dense").unwrap();
        let fix = fixes(&d, &plan, &f, &mut crate::plan).into_iter().find(|x| x.id == "lower-density").unwrap();
        let Change::Set(m) = &fix.change else { panic!() };
        let v = m["spacing"].as_f64().unwrap();
        assert!(v > 0.3 && v <= StitchSettings::DEFAULT_SPACING * SPARSEST, "{v}");
        assert!(fix.clears, "one object's density alone clears its spot: {fix:?}");
    }

    #[test]
    fn stacked_fills_keeping_their_covered_rows_are_cut_away_or_dropped() {
        let d = doc_with(|d| {
            (0..3)
                .map(|_| square(d, StitchSettings { keep_covered: true, underlay: Underlay::Auto, ..StitchSettings::new(StitchKind::Fill) }))
                .collect()
        });
        let kinds = every_fix_works(&d);
        assert!(kinds.contains(&"drop-duplicate") && kinds.contains(&"remove-overlap"), "{kinds:?}");
        // Fixing again and again clears the spot: each fix thins it.
        let mut d = d;
        for _ in 0..4 {
            let plan = crate::plan(&d);
            let Some(f) = check::check(&d, &plan).into_iter().find(|f| f.rule == "dense") else { return };
            let fix = fixes(&d, &plan, &f, &mut crate::plan).into_iter().next().expect("a fix while still dense");
            d = fix.apply(&d).unwrap();
        }
        panic!("still dense after four fixes");
    }

    #[test]
    fn a_fine_design_gets_no_fixes_and_lighter_underlays_step_down() {
        let d = doc_with(|d| vec![square(d, StitchSettings::new(StitchKind::Fill))]);
        let plan = crate::plan(&d);
        let f = Finding { rule: "tiny", severity: check::Severity::Note, message: String::new(), object: None, at: None };
        assert!(fixes(&d, &plan, &f, &mut crate::plan).is_empty());
        assert_eq!(lighter(StitchKind::Satin, Underlay::Zigzag), Some(Underlay::Edge));
        assert_eq!(lighter(StitchKind::Satin, Underlay::Edge), Some(Underlay::Center));
        assert_eq!(lighter(StitchKind::Fill, Underlay::Auto), Some(Underlay::None));
        assert_eq!(lighter(StitchKind::Fill, Underlay::None), None);
    }
}
