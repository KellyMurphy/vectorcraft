//! The design checker: rules run over a stitch plan, each finding naming its object and where on it
//! the problem is, so the Embroidery panel can list them, Stitch Preview can mark them and agents
//! can read them (`embroidery.check`). A new rule is an entry in [`RULES`].

use kurbo::Point;
use vectorcraft_doc::{Document, NodeId};

use crate::{Cmd, LeftOut, MAX_NEEDLES, Plan};

/// How serious a finding is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// The design won't sew as drawn.
    Error,
    /// It may sew badly (snags, gaps, puckering).
    Warning,
    /// Worth knowing.
    Note,
}

impl Severity {
    pub fn id(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Note => "note",
        }
    }
}

/// One problem the checker found.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    /// The rule that found it ([`Rule::id`]).
    pub rule: &'static str,
    pub severity: Severity,
    pub message: String,
    /// The object it is about (none: the whole design).
    pub object: Option<NodeId>,
    /// Where, mm in document space (none: the object as a whole).
    pub at: Option<Point>,
}

/// A check run over a plan.
pub struct Rule {
    pub id: &'static str,
    /// A short name, shown in the panel and in summaries.
    pub label: &'static str,
    /// What it looks for and how to fix it.
    pub help: &'static str,
    /// Whether the plan's own warnings already say what it finds (exports don't repeat them).
    pub in_plan_warnings: bool,
    pub run: fn(&Document, &Plan) -> Vec<Finding>,
}

/// Stitches longer than this snag on wear, mm.
pub const LONG_STITCH_MM: f64 = 7.0;

/// Every rule, in the order their findings are listed (within each severity).
pub static RULES: &[Rule] = &[
    Rule {
        id: "truncated",
        label: "Design cut short",
        help: "The design has more needle moves than a file can hold; the rest is left out. Split it into several designs.",
        in_plan_warnings: true,
        run: |_, plan| {
            if !plan.truncated {
                return vec![];
            }
            vec![Finding {
                rule: "truncated",
                severity: Severity::Error,
                message: format!("the design has more than {MAX_NEEDLES} needle moves: the rest is left out"),
                object: None,
                at: None,
            }]
        },
    },
    Rule {
        id: "left-out",
        label: "Not sewn as set",
        help: "Objects that are stitched but can't be sewn (not a path, compound path or type; settings from a newer VectorCraft), or are sewn whole although a mask hides part of them.",
        in_plan_warnings: true,
        run: |_, plan| {
            plan.left_out
                .iter()
                .map(|(id, why)| {
                    let (severity, message) = match why {
                        LeftOut::CantStitch => (Severity::Warning, "can't be stitched (only paths, compound paths and type are): left out"),
                        LeftOut::Newer => (Severity::Warning, "has stitch settings from a newer VectorCraft that this version can't read: not sewn"),
                        LeftOut::Masked => (Severity::Note, "is under a clipping or opacity mask, which doesn't apply to stitches: sewn whole"),
                    };
                    Finding { rule: "left-out", severity, message: message.into(), object: Some(*id), at: None }
                })
                .collect()
        },
    },
    Rule {
        id: "art",
        label: "Art the stitch can't follow",
        help: "What the stitch type said about the object's shape (a satin shape with no strokes, a motif too fine, a border pattern that is missing…).",
        in_plan_warnings: true,
        run: |_, plan| {
            plan.objects
                .iter()
                .flat_map(|o| {
                    let at = first_stitch(plan, o);
                    o.warnings.iter().map(move |w| Finding { rule: "art", severity: Severity::Warning, message: w.clone(), object: Some(o.id), at })
                })
                .collect()
        },
    },
    Rule {
        id: "no-stitches",
        label: "Makes no stitches",
        help: "A stitched object that makes no stitches: empty, or too small for its stitch type.",
        in_plan_warnings: false,
        run: |_, plan| {
            plan.objects
                .iter()
                .filter(|o| !plan.needles.get(o.needles.clone()).unwrap_or_default().iter().any(|n| n.cmd == Cmd::Stitch))
                .map(|o| Finding {
                    rule: "no-stitches",
                    severity: Severity::Warning,
                    message: "makes no stitches (empty, or too small for its stitch type)".into(),
                    object: Some(o.id),
                    at: None,
                })
                .collect()
        },
    },
    Rule {
        id: "long-stitch",
        label: "Long stitches",
        help: "Stitches longer than 7 mm snag and loosen with wear. Shorten the stitch length, narrow the satin, or fill a shape that wide.",
        in_plan_warnings: false,
        run: |_, plan| {
            let mut out = vec![];
            for o in &plan.objects {
                let (mut count, mut longest, mut at) = (0usize, 0.0f64, None);
                for i in o.needles.clone() {
                    let (Some(n), Some(prev)) = (plan.needles.get(i), i.checked_sub(1).and_then(|j| plan.needles.get(j))) else { continue };
                    if n.cmd != Cmd::Stitch {
                        continue;
                    }
                    let (a, b) = (Point::new(prev.x, prev.y), Point::new(n.x, n.y));
                    let len = a.distance(b);
                    if len > LONG_STITCH_MM {
                        count += 1;
                        if len > longest {
                            (longest, at) = (len, Some(a.midpoint(b)));
                        }
                    }
                }
                if count > 0 {
                    let what = if count == 1 { "stitch" } else { "stitches" };
                    out.push(Finding {
                        rule: "long-stitch",
                        severity: Severity::Warning,
                        message: format!("has {count} {what} longer than {LONG_STITCH_MM} mm (the longest {longest:.1} mm): they can snag"),
                        object: Some(o.id),
                        at,
                    });
                }
            }
            out
        },
    },
];

/// The object's first stitch, if it makes one.
fn first_stitch(plan: &Plan, o: &crate::PlannedObject) -> Option<Point> {
    plan.needles.get(o.needles.clone())?.iter().find(|n| n.cmd == Cmd::Stitch).map(|n| Point::new(n.x, n.y))
}

/// Every finding of every rule on `plan` (made from `doc`), errors first, then in sewing order.
pub fn check(doc: &Document, plan: &Plan) -> Vec<Finding> {
    let order = |id: Option<NodeId>| id.and_then(|id| plan.objects.iter().position(|o| o.id == id)).unwrap_or(usize::MAX);
    let mut all: Vec<Finding> = RULES.iter().flat_map(|r| (r.run)(doc, plan)).collect();
    all.sort_by_key(|f| (f.severity, order(f.object)));
    all
}

/// One line per rule for `findings` the plan's warnings don't already give, for exports.
pub fn summary(findings: &[Finding]) -> Vec<String> {
    RULES
        .iter()
        .filter(|r| !r.in_plan_warnings)
        .filter_map(|r| {
            let n = findings.iter().filter(|f| f.rule == r.id).count();
            let what = if n == 1 { "object" } else { "objects" };
            (n > 0).then(|| format!("design check: {n} {what}: {}", r.label.to_lowercase()))
        })
        .collect()
}

/// The rule `id`.
pub fn rule(id: &str) -> Option<&'static Rule> {
    RULES.iter().find(|r| r.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{doc_with, poly, stitched};
    use vectorcraft_doc::StitchKind;

    #[test]
    fn rule_ids_are_unique() {
        let mut ids: Vec<&str> = RULES.iter().map(|r| r.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), RULES.len());
    }

    #[test]
    fn findings_name_their_object_and_place() {
        let d = doc_with(|d| {
            // A wide satin (10 mm: long stitches), a fine line, and an empty path.
            let mut wide = stitched(d, poly(&[(0.0, 0.0), (200.0, 0.0)], false), StitchKind::Satin, [200, 0, 0]);
            wide.extras_mut().stitch.as_mut().unwrap().width = Some(10.0);
            let fine = stitched(d, poly(&[(0.0, 100.0), (200.0, 100.0)], false), StitchKind::Running, [0, 0, 200]);
            let empty = stitched(d, poly(&[(0.0, 200.0)], false), StitchKind::Running, [0, 0, 200]);
            vec![wide, fine, empty]
        });
        let plan = crate::plan(&d);
        let found = check(&d, &plan);
        let ids: Vec<NodeId> = plan.objects.iter().map(|o| o.id).collect();
        let long = found.iter().find(|f| f.rule == "long-stitch").expect("the wide satin's stitches are long");
        assert_eq!(long.object, ids.first().copied());
        let at = long.at.unwrap();
        assert!(at.x > 0.0 && at.x < 200.0 * crate::PT_MM && at.y.abs() < 6.0, "on the satin: {at:?}");
        assert!(!found.iter().any(|f| f.rule == "long-stitch" && f.object == ids.get(1).copied()), "the fine line is fine");
        assert!(found.iter().any(|f| f.rule == "no-stitches"), "{found:?}");
        assert_eq!(summary(&found).len(), 2, "{:?}", summary(&found));
    }
}
