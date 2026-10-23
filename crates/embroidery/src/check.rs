//! The design checker: rules run over a stitch plan, each finding naming its object and where on it
//! the problem is, so the Embroidery panel can list them, Stitch Preview can mark them and agents
//! can read them (`embroidery.check`). A new rule is an entry in [`RULES`].

use std::collections::HashMap;

use kurbo::Point;
use vectorcraft_doc::{Document, NodeId, NodeKind, StitchKind, StitchSettings};

use crate::{Cmd, LeftOut, MAX_NEEDLES, PT_MM, Plan, PlannedObject};

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
/// Stitches longer than this can't be sewn in one move (DST's longest is 12.1 mm), mm.
pub const IMPOSSIBLE_STITCH_MM: f64 = 12.1;
/// Satin columns narrower or wider than these sew badly, mm.
pub const SATIN_NARROW_MM: f64 = 1.0;
pub const SATIN_WIDE_MM: f64 = 10.0;
/// Objects whose stitches fit in a box this small (both ways) are too small to sew, mm.
pub const TINY_MM: f64 = 1.0;
/// The density map's cell, mm, and the most needle holes a cell takes before it is a hot spot.
pub const DENSITY_CELL_MM: f64 = 2.0;
pub const DENSITY_MAX: usize = 40;
/// Puff satin narrower or wider than these doesn't rise well, mm.
pub const PUFF_NARROW_MM: f64 = 3.0;
pub const PUFF_WIDE_MM: f64 = 10.0;
/// More trims or jumps than these make a slow design.
pub const TRIMS_MAX: usize = 40;
pub const JUMPS_MAX: usize = 150;

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
        id: "impossible-stitch",
        label: "Stitches too long to sew",
        help: "Stitches longer than 12 mm can't be sewn in one move. Add points along the path, or use a stitch type that splits long stitches.",
        in_plan_warnings: false,
        run: |_, plan| {
            let mut out = vec![];
            for o in &plan.objects {
                let (count, longest, at) = stitches_over(plan, o, IMPOSSIBLE_STITCH_MM);
                if count > 0 {
                    out.push(Finding {
                        rule: "impossible-stitch",
                        severity: Severity::Error,
                        message: format!(
                            "has {count} stitches longer than {IMPOSSIBLE_STITCH_MM} mm (the longest {longest:.1} mm): a machine can't sew them"
                        ),
                        object: Some(o.id),
                        at,
                    });
                }
            }
            out
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
                let (count, longest, at) = stitches_over(plan, o, LONG_STITCH_MM);
                // Those too long to sew at all are another rule's.
                if count > 0 && longest <= IMPOSSIBLE_STITCH_MM {
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
    Rule {
        id: "hoop",
        label: "Design bigger than the hoop",
        help: "The design must fit the hoop's sewing field. Choose a larger hoop, turn the design a quarter turn, scale it down, or split it into designs sewn one after another.",
        in_plan_warnings: false,
        run: |doc, plan| {
            let Some(hoop) = doc.embroidery.hoop.as_ref().filter(|h| h.unknown.is_none()) else { return vec![] };
            let Some(b) = plan.stats().bounds else { return vec![] };
            let (w, h) = (b[2] - b[0], b[3] - b[1]);
            let (fw, fh) = crate::hoops::size(hoop);
            let (severity, message) = match crate::hoops::fit(w, h, hoop) {
                crate::hoops::Fit::Fits => return vec![],
                crate::hoops::Fit::FitsTurned => {
                    (Severity::Warning, format!("the design ({w:.0} × {h:.0} mm) fits the {fw:.0} × {fh:.0} mm hoop only turned a quarter turn"))
                }
                crate::hoops::Fit::TooBig => (Severity::Error, format!("the design ({w:.0} × {h:.0} mm) doesn't fit the {fw:.0} × {fh:.0} mm hoop")),
            };
            vec![Finding { rule: "hoop", severity, message, object: None, at: None }]
        },
    },
    Rule {
        id: "satin-width",
        label: "Satin too narrow or too wide",
        help: "Satin under 1 mm wide doesn't cover (use a running or bean stitch); over 10 mm its stitches are long and loose (fill the shape instead).",
        in_plan_warnings: false,
        run: |doc, plan| {
            let mut out = vec![];
            for o in &plan.objects {
                let Some(n) = doc.node(o.id) else { continue };
                let Some(s) = n.extras().stitch.as_deref().filter(|s| s.kind == StitchKind::Satin && !s.rails && s.unknown.is_none()) else {
                    continue;
                };
                let w = s.width(n.appearance.stroke_width() * PT_MM);
                let message = if w < SATIN_NARROW_MM {
                    format!("is a satin {w:.1} mm wide: too narrow to cover (use a running or bean stitch)")
                } else if w > SATIN_WIDE_MM {
                    format!("is a satin {w:.1} mm wide: its stitches are long and loose (fill it instead)")
                } else {
                    continue;
                };
                out.push(Finding { rule: "satin-width", severity: Severity::Warning, message, object: Some(o.id), at: first_stitch(plan, o) });
            }
            out
        },
    },
    Rule {
        id: "tiny",
        label: "Too small to sew",
        help: "Objects whose stitches fit in a 1 mm box sew as a knot of thread. Make them larger or remove them.",
        in_plan_warnings: false,
        run: |_, plan| {
            plan.objects
                .iter()
                .filter_map(|o| {
                    // Its own stitches: from where it starts (the first jump into it; what comes before
                    // locks the thread of the object before).
                    let all = plan.needles.get(o.needles.clone())?;
                    let own = all.iter().position(|n| n.cmd == Cmd::Jump).map_or(all, |i| all.get(i..).unwrap_or_default());
                    let pts: Vec<Point> = own.iter().filter(|n| n.cmd == Cmd::Stitch).map(|n| Point::new(n.x, n.y)).collect();
                    let first = *pts.first()?;
                    let (lo, hi) = pts
                        .iter()
                        .fold((first, first), |(lo, hi), p| (Point::new(lo.x.min(p.x), lo.y.min(p.y)), Point::new(hi.x.max(p.x), hi.y.max(p.y))));
                    let (w, h) = (hi.x - lo.x, hi.y - lo.y);
                    (w < TINY_MM && h < TINY_MM).then(|| Finding {
                        rule: "tiny",
                        severity: Severity::Warning,
                        message: format!("sews only {w:.1} × {h:.1} mm: too small to sew cleanly"),
                        object: Some(o.id),
                        at: Some(lo.midpoint(hi)),
                    })
                })
                .collect()
        },
    },
    Rule {
        id: "open-fill",
        label: "Open shape set to fill",
        help: "A fill closes open paths with a straight line, which may not be the shape meant. Close the path, or use a stitch along it.",
        in_plan_warnings: false,
        run: |doc, plan| {
            plan.objects
                .iter()
                .filter(|o| {
                    let Some(n) = doc.node(o.id) else { return false };
                    let area = n
                        .extras()
                        .stitch
                        .as_deref()
                        .is_some_and(|s| matches!(s.kind, StitchKind::Fill | StitchKind::SatinShape) && s.unknown.is_none());
                    let open = |p: &vectorcraft_geom::PathData| p.subpaths.iter().any(|sp| !sp.closed && sp.anchors.len() > 2);
                    let has_open = match &n.kind {
                        NodeKind::Path { path, .. } => open(path),
                        NodeKind::Compound { children, .. } => children.iter().filter_map(|c| c.path_data()).any(open),
                        _ => false,
                    };
                    area && has_open
                })
                .map(|o| Finding {
                    rule: "open-fill",
                    severity: Severity::Note,
                    message: "is an open path set to fill: it is closed with a straight line".into(),
                    object: Some(o.id),
                    at: first_stitch(plan, o),
                })
                .collect()
        },
    },
    Rule {
        id: "dense",
        label: "Too many needle holes in one place",
        help: "Where objects stack up, so many needle holes go into one spot that the fabric tears or the needle breaks. Cut away what is covered, lower the density, or remove a layer.",
        in_plan_warnings: false,
        run: |_, plan| {
            let cell = |p: &crate::Needle| ((p.x / DENSITY_CELL_MM).floor() as i64, (p.y / DENSITY_CELL_MM).floor() as i64);
            let mut map: HashMap<(i64, i64), usize> = HashMap::new();
            for n in plan.needles.iter().filter(|n| n.cmd == Cmd::Stitch && n.x.is_finite() && n.y.is_finite()) {
                *map.entry(cell(n)).or_default() += 1;
            }
            let mut out = vec![];
            for o in &plan.objects {
                let worst = plan
                    .needles
                    .get(o.needles.clone())
                    .unwrap_or_default()
                    .iter()
                    .filter(|n| n.cmd == Cmd::Stitch && n.x.is_finite() && n.y.is_finite())
                    .map(|n| (map.get(&cell(n)).copied().unwrap_or(0), cell(n)))
                    .max();
                if let Some((count, (cx, cy))) = worst.filter(|(c, _)| *c > DENSITY_MAX) {
                    let at = Point::new((cx as f64 + 0.5) * DENSITY_CELL_MM, (cy as f64 + 0.5) * DENSITY_CELL_MM);
                    out.push(Finding {
                        rule: "dense",
                        severity: Severity::Warning,
                        message: format!("is part of a spot with {count} needle holes in {DENSITY_CELL_MM} × {DENSITY_CELL_MM} mm: too dense"),
                        object: Some(o.id),
                        at: Some(at),
                    });
                }
            }
            out
        },
    },
    Rule {
        id: "travel",
        label: "Many trims or jumps",
        help: "Every trim and jump slows the machine and leaves thread ends. Sew by Thread, reorder objects, or join nearby parts.",
        in_plan_warnings: false,
        run: |_, plan| {
            let st = plan.stats();
            let mut out = vec![];
            if st.trims > TRIMS_MAX {
                out.push(Finding {
                    rule: "travel",
                    severity: Severity::Note,
                    message: format!("the design has {} trims: Sew by Thread or reordering may cut them", st.trims),
                    object: None,
                    at: None,
                });
            }
            if st.jumps > JUMPS_MAX {
                out.push(Finding {
                    rule: "travel",
                    severity: Severity::Note,
                    message: format!("the design has {} jumps: reordering or joining nearby parts may cut them", st.jumps),
                    object: None,
                    at: None,
                });
            }
            out
        },
    },
    Rule {
        id: "puff",
        label: "Puff that won't rise well",
        help: "Puff needs satin 3 to 10 mm wide over foam: narrower it doesn't rise, wider its stitches don't hold the foam. Flat objects sewn after the foam is laid sew through it: sew them first.",
        in_plan_warnings: false,
        run: |doc, plan| {
            let mut out = vec![];
            let mut foam_down = false;
            for o in &plan.objects {
                let Some(n) = doc.node(o.id) else { continue };
                let Some(s) = n.extras().stitch.as_deref().filter(|s| s.unknown.is_none()) else { continue };
                if s.puffed() {
                    foam_down = true;
                    if s.kind == StitchKind::Satin {
                        let w = s.width(n.appearance.stroke_width() * PT_MM);
                        if !(PUFF_NARROW_MM..=PUFF_WIDE_MM).contains(&w) {
                            out.push(Finding {
                                rule: "puff",
                                severity: Severity::Warning,
                                message: format!("is puff {w:.1} mm wide: puff rises well from {PUFF_NARROW_MM} to {PUFF_WIDE_MM} mm"),
                                object: Some(o.id),
                                at: first_stitch(plan, o),
                            });
                        }
                    }
                } else if foam_down {
                    out.push(Finding {
                        rule: "puff",
                        severity: Severity::Warning,
                        message: "is sewn flat after the foam is laid: it sews through the foam (sew it before the puff)".into(),
                        object: Some(o.id),
                        at: first_stitch(plan, o),
                    });
                }
            }
            out
        },
    },
    Rule {
        id: "small-type",
        label: "Type too small",
        help: "Lettering below its safe size closes up and can't be read: satin lettering needs 4 mm or more, filled lettering 6 mm, running or bean lettering 2.5 mm. Letters' holes (the inside of an o, a or e) under 1 mm across close up on fabric whatever the stitch, so a font with small holes needs larger type. Make the type larger, or use a running stitch.",
        in_plan_warnings: false,
        run: |doc, plan| {
            let mut out = vec![];
            for o in &plan.objects {
                let Some(n) = doc.node(o.id) else { continue };
                let (NodeKind::Text(t), Some(s)) = (&n.kind, n.extras().stitch.as_deref()) else { continue };
                if s.unknown.is_some() {
                    continue;
                }
                let size = t.runs.iter().map(|r| r.style.size).fold(f64::INFINITY, f64::min);
                let size = if size.is_finite() { size } else { t.first_style().size } * PT_MM;
                let least = safe_type_size(s);
                if size.is_finite() && size < least {
                    out.push(Finding {
                        rule: "small-type",
                        severity: Severity::Warning,
                        message: format!("is type {size:.1} mm high: {} lettering needs {least} mm or more to be read", s.kind.id()),
                        object: Some(o.id),
                        at: first_stitch(plan, o),
                    });
                    continue;
                }
                // Its letters' holes: a stitch that covers the letter (not a running outline)
                // closes those under MIN_HOLE across.
                if matches!(s.kind, StitchKind::Running | StitchKind::Bean | StitchKind::Manual) || !size.is_finite() || size <= 0.0 {
                    continue;
                }
                let hole = crate::glyph_parts(t)
                    .iter()
                    .filter_map(|(_, p)| crate::narrowest_hole(&crate::generate::polylines(p).into_iter().map(|(l, _)| l).collect::<Vec<_>>()))
                    .min_by(f64::total_cmp);
                if let Some(h) = hole.filter(|h| *h < MIN_HOLE && *h > 0.0) {
                    let needed = (size * MIN_HOLE / h * 2.0).ceil() / 2.0;
                    out.push(Finding {
                        rule: "small-type",
                        severity: Severity::Warning,
                        message: format!(
                            "has letters whose holes are {h:.1} mm across at {size:.1} mm: under {MIN_HOLE} mm they close up on fabric. Make the type {needed} mm or more, or use a more open font"
                        ),
                        object: Some(o.id),
                        at: first_stitch(plan, o),
                    });
                }
            }
            out
        },
    },
];

/// The narrowest a letter's hole (counter) can be and stay open when sewn, mm across.
pub const MIN_HOLE: f64 = 1.0;

/// The smallest type (its size, mm) stitch settings `s` sew legibly.
pub fn safe_type_size(s: &StitchSettings) -> f64 {
    match s.kind {
        StitchKind::Running | StitchKind::Bean | StitchKind::Manual => 2.5,
        StitchKind::Fill => 6.0,
        _ => 4.0,
    }
}

/// An object's stitches longer than `limit` mm: how many, the longest, and its middle.
fn stitches_over(plan: &Plan, o: &PlannedObject, limit: f64) -> (usize, f64, Option<Point>) {
    let (mut count, mut longest, mut at) = (0usize, 0.0f64, None);
    for i in o.needles.clone() {
        let (Some(n), Some(prev)) = (plan.needles.get(i), i.checked_sub(1).and_then(|j| plan.needles.get(j))) else { continue };
        if n.cmd != Cmd::Stitch {
            continue;
        }
        let (a, b) = (Point::new(prev.x, prev.y), Point::new(n.x, n.y));
        let len = a.distance(b);
        if len > limit {
            count += 1;
            if len > longest {
                (longest, at) = (len, Some(a.midpoint(b)));
            }
        }
    }
    (count, longest, at)
}

/// The object's first stitch, if it makes one.
fn first_stitch(plan: &Plan, o: &PlannedObject) -> Option<Point> {
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

    fn rules_found(d: &Document) -> Vec<&'static str> {
        let mut r: Vec<&str> = check(d, &crate::plan(d)).iter().map(|f| f.rule).collect();
        r.dedup();
        r
    }

    #[test]
    fn the_production_rules_find_what_they_look_for() {
        use vectorcraft_doc::StitchSettings;
        let with = |n: &mut vectorcraft_doc::Node, s: StitchSettings| n.extras_mut().stitch = Some(Box::new(s));
        // A 0.6 mm satin, an 11 mm satin, a dot, an open path set to fill.
        let d = doc_with(|d| {
            let mut thin = stitched(d, poly(&[(0.0, 0.0), (100.0, 0.0)], false), StitchKind::Satin, [0; 3]);
            with(&mut thin, StitchSettings { width: Some(0.6), ..StitchSettings::new(StitchKind::Satin) });
            let mut wide = stitched(d, poly(&[(0.0, 100.0), (100.0, 100.0)], false), StitchKind::Satin, [0; 3]);
            with(&mut wide, StitchSettings { width: Some(11.0), ..StitchSettings::new(StitchKind::Satin) });
            let dot = stitched(d, poly(&[(0.0, 200.0), (1.0, 200.0)], false), StitchKind::Running, [0; 3]);
            let open = stitched(d, poly(&[(200.0, 0.0), (300.0, 0.0), (300.0, 100.0)], false), StitchKind::Fill, [0; 3]);
            vec![thin, wide, dot, open]
        });
        let found = check(&d, &crate::plan(&d));
        let n = |rule: &str| found.iter().filter(|f| f.rule == rule).count();
        assert_eq!(n("satin-width"), 2, "{found:?}");
        assert_eq!(n("tiny"), 1, "{found:?}");
        assert_eq!(n("open-fill"), 1, "{found:?}");
        assert_eq!(n("impossible-stitch"), 0, "satins are split at 12 mm");
        // Three fills stacked on one spot, each keeping its covered rows: too dense.
        let stacked = doc_with(|d| {
            (0..3)
                .map(|_| {
                    let mut n = stitched(d, poly(&[(0.0, 0.0), (60.0, 0.0), (60.0, 60.0), (0.0, 60.0)], true), StitchKind::Fill, [0; 3]);
                    with(
                        &mut n,
                        StitchSettings { keep_covered: true, underlay: vectorcraft_doc::Underlay::Auto, ..StitchSettings::new(StitchKind::Fill) },
                    );
                    n
                })
                .collect()
        });
        assert!(rules_found(&stacked).contains(&"dense"), "{:?}", rules_found(&stacked));
        // Cut away (the default), the same stack isn't.
        let cut = doc_with(|d| {
            (0..3)
                .map(|_| {
                    let mut n = stitched(d, poly(&[(0.0, 0.0), (60.0, 0.0), (60.0, 60.0), (0.0, 60.0)], true), StitchKind::Fill, [0; 3]);
                    with(&mut n, StitchSettings { underlay: vectorcraft_doc::Underlay::Auto, ..StitchSettings::new(StitchKind::Fill) });
                    n
                })
                .collect()
        });
        assert!(!rules_found(&cut).contains(&"dense"), "{:?}", rules_found(&cut));
        // The test sheet: only what it is made to show (long stitches; lettering down to sizes
        // whose holes close, when a type face is there) and its many trims.
        let sheet = crate::testsheet::build(&[]).unwrap();
        for rule in rules_found(&sheet.doc) {
            assert!(matches!(rule, "long-stitch" | "travel" | "small-type"), "{rule}");
        }
    }

    #[test]
    fn small_type_is_found_by_stitch() {
        use vectorcraft_doc::StitchSettings;
        let s = |k| StitchSettings::new(k);
        assert_eq!(safe_type_size(&s(StitchKind::Running)), 2.5);
        assert_eq!(safe_type_size(&s(StitchKind::Fill)), 6.0);
        assert_eq!(safe_type_size(&s(StitchKind::Auto)), 4.0);
        // 3 mm type in Auto: below 4 mm.
        let d = doc_with(|d| {
            let style = vectorcraft_doc::CharStyle { size: 3.0 / crate::PT_MM, ..Default::default() };
            let t = vectorcraft_doc::TextObject::point(Point::new(10.0, 100.0), "Hag", style);
            let mut n = vectorcraft_doc::Node::new(d.alloc_id(), NodeKind::Text(Box::new(t)));
            n.extras_mut().stitch = Some(Box::new(s(StitchKind::Auto)));
            vec![n]
        });
        let plan = crate::plan(&d);
        // Without a type face nothing is sewn, and nothing is checked.
        if plan.stats().stitches > 0 {
            assert!(check(&d, &plan).iter().any(|f| f.rule == "small-type"));
        }
    }
}

/// `cargo test -p vectorcraft-embroidery print_test_sheet_findings -- --ignored --nocapture`: what the
/// checker says about the whole test sheet, and the densest spots of single and stacked objects,
/// for setting the rules' limits.
#[cfg(test)]
mod calibrate {
    #[test]
    #[ignore]
    fn print_test_sheet_findings() {
        let sheet = crate::testsheet::build(&[]).unwrap();
        let plan = crate::plan(&sheet.doc);
        for f in super::check(&sheet.doc, &plan) {
            println!("{} {:?} {}", f.rule, f.severity, f.message);
        }
        let st = plan.stats();
        println!("trims {} jumps {}", st.trims, st.jumps);
        println!("sheet max cell {}", max_cell(&plan));
        use crate::tests::{doc_with, poly, stitched};
        use vectorcraft_doc::{StitchKind, StitchSettings};
        for (kind, layers) in [(StitchKind::Satin, 1), (StitchKind::Satin, 3), (StitchKind::Fill, 1), (StitchKind::Fill, 3)] {
            let d = doc_with(|d| {
                (0..layers)
                    .map(|_| {
                        let path = if kind == StitchKind::Satin {
                            poly(&[(0.0, 0.0), (100.0, 0.0)], false)
                        } else {
                            poly(&[(0.0, 0.0), (60.0, 0.0), (60.0, 60.0), (0.0, 60.0)], true)
                        };
                        let mut n = stitched(d, path, kind, [0; 3]);
                        n.extras_mut().stitch = Some(Box::new(StitchSettings {
                            width: Some(4.0),
                            keep_covered: true,
                            underlay: vectorcraft_doc::Underlay::Auto,
                            ..StitchSettings::new(kind)
                        }));
                        n
                    })
                    .collect()
            });
            println!("{kind:?} x{layers}: max cell {}", max_cell(&crate::plan(&d)));
        }
    }

    fn max_cell(plan: &crate::Plan) -> usize {
        let mut m: std::collections::HashMap<(i64, i64), usize> = Default::default();
        for n in plan.needles.iter().filter(|n| n.cmd == crate::Cmd::Stitch) {
            *m.entry(((n.x / 2.0).floor() as i64, (n.y / 2.0).floor() as i64)).or_default() += 1;
        }
        m.values().copied().max().unwrap_or(0)
    }
}
