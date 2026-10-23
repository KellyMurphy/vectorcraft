use std::sync::Arc;

use kurbo::Point;
use proptest::prelude::*;
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Document, Node, StitchKind, StitchSettings};
use vectorcraft_geom::{Anchor, FillRule, PathData, SubPath};

use super::*;

mod machine_formats;

pub(crate) fn poly(pts: &[(f64, f64)], closed: bool) -> PathData {
    PathData::single(SubPath { anchors: pts.iter().map(|&(x, y)| Anchor::corner(Point::new(x, y))).collect(), closed })
}

/// A document holding `nodes` (made with `f`, given the document for ids) on its first layer.
pub(crate) fn doc_with(f: impl FnOnce(&mut Document) -> Vec<Node>) -> Document {
    let mut d = Document::new(500.0, 500.0);
    let nodes = f(&mut d);
    let layer = Arc::make_mut(&mut d.layers[0]);
    layer.children_mut().unwrap().extend(nodes.into_iter().map(Arc::new));
    d
}

pub(crate) fn stitched(d: &mut Document, path: PathData, kind: StitchKind, stroke: [u8; 3]) -> Node {
    let mut n = Node::path(d.alloc_id(), path, Appearance::basic(Paint::None, Paint::solid(Color::rgb8(stroke[0], stroke[1], stroke[2])), 1.0));
    n.extras_mut().stitch = Some(Box::new(StitchSettings::new(kind)));
    n
}

fn mm(pt: f64) -> f64 {
    pt * PT_MM
}

fn stitch_lengths(plan: &Plan) -> Vec<f64> {
    plan.runs().iter().flat_map(|(_, r)| r.windows(2).map(|w| w[0].distance(w[1])).collect::<Vec<_>>()).collect()
}

#[test]
fn running_stitch_spaces_stitches_evenly_and_ends_on_the_path_end() {
    // 72 pt = 25.4 mm, at 2.5 mm stitches: 10 full stitches and a 0.4 mm remainder merged in.
    let line = [Point::new(0.0, 0.0), Point::new(25.4, 0.0)];
    let r = generate::running(&line, 2.5);
    assert_eq!(r.first(), Some(&Point::new(0.0, 0.0)));
    assert_eq!(r.last(), Some(&Point::new(25.4, 0.0)));
    for w in r.windows(2) {
        let d = w[0].distance(w[1]);
        assert!(d > 0.5 && d <= 2.5 + 0.4 + 1e-9, "{d}");
    }
}

#[test]
fn running_stitch_keeps_sharp_corners() {
    let line = [Point::new(0.0, 0.0), Point::new(10.3, 0.0), Point::new(10.3, 10.3)];
    let r = generate::running(&line, 2.5);
    assert!(r.contains(&Point::new(10.3, 0.0)), "{r:?}");
}

#[test]
fn bean_stitch_sews_each_stitch_three_times() {
    let r = generate::bean(&[Point::new(0.0, 0.0), Point::new(2.0, 0.0), Point::new(4.0, 0.0)], 3);
    assert_eq!(r.len(), 1 + 3 * 2);
    assert_eq!(r.last(), Some(&Point::new(4.0, 0.0)));
}

#[test]
fn satin_zigzags_across_the_column_width() {
    let line = [Point::new(0.0, 0.0), Point::new(20.0, 0.0)];
    let r = generate::satin(&line, false, 4.0, 0.4);
    assert!(r.len() > 40);
    for (i, p) in r.iter().enumerate() {
        assert!((p.y.abs() - 2.0).abs() < 1e-9, "{p:?}");
        // Sides alternate.
        if let Some(q) = r.get(i + 1) {
            assert!(p.y * q.y < 0.0);
        }
    }
}

#[test]
fn fill_covers_a_square_with_one_back_and_forth_run() {
    let sq = vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(10.0, 10.0), Point::new(0.0, 10.0)];
    let runs = generate::fill(&[sq], FillRule::NonZero, 0.0, 0.5, 3.0, 3).unwrap();
    assert_eq!(runs.len(), 1);
    let run = &runs[0];
    let ys: std::collections::BTreeSet<i64> = run.iter().map(|p| (p.y * 1000.0).round() as i64).collect();
    assert_eq!(ys.len(), 20, "one row every 0.5 mm");
    for p in run {
        assert!((-1e-9..=10.0 + 1e-9).contains(&p.x) && (0.0..=10.0).contains(&p.y));
    }
    for w in run.windows(2) {
        assert!(w[0].distance(w[1]) <= 3.0 + 1e-9);
    }
}

#[test]
fn fill_leaves_holes_and_splits_separate_parts() {
    // A frame (outer square, inner square the other way round): the rows split around the hole.
    let outer = vec![Point::new(0.0, 0.0), Point::new(30.0, 0.0), Point::new(30.0, 30.0), Point::new(0.0, 30.0)];
    let inner = vec![Point::new(10.0, 10.0), Point::new(10.0, 20.0), Point::new(20.0, 20.0), Point::new(20.0, 10.0)];
    let runs = generate::fill(&[outer, inner], FillRule::NonZero, 0.0, 0.5, 3.0, 3).unwrap();
    for p in runs.iter().flatten() {
        let in_hole = p.x > 10.0 + 1e-6 && p.x < 20.0 - 1e-6 && p.y > 10.0 + 1e-6 && p.y < 20.0 - 1e-6;
        assert!(!in_hole, "{p:?} in the hole");
    }
    // The parts either side of the hole are joined by travel round it: one run, no jump.
    assert_eq!(runs.len(), 1);
    for w in runs[0].windows(2) {
        let mid = w[0].lerp(w[1], 0.5);
        let in_hole = mid.x > 10.0 + 1e-6 && mid.x < 20.0 - 1e-6 && mid.y > 10.0 + 1e-6 && mid.y < 20.0 - 1e-6;
        assert!(!in_hole, "travel {:?}–{:?} crosses the hole", w[0], w[1]);
    }
}

#[test]
fn fill_rows_follow_the_angle() {
    let sq = vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(10.0, 10.0), Point::new(0.0, 10.0)];
    let runs = generate::fill(&[sq], FillRule::NonZero, 90.0, 0.5, 3.0, 3).unwrap();
    // Vertical rows: most stitches move along y only.
    let run = &runs[0];
    let vertical = run.windows(2).filter(|w| (w[0].x - w[1].x).abs() < 1e-6).count();
    assert!(vertical * 2 > run.len(), "{vertical} of {}", run.len());
}

#[test]
fn huge_fills_are_refused() {
    let sq = vec![Point::new(0.0, 0.0), Point::new(1e7, 0.0), Point::new(1e7, 1e7), Point::new(0.0, 1e7)];
    assert!(generate::fill(&[sq], FillRule::NonZero, 0.0, 0.4, 3.0, 3).is_err());
}

#[test]
fn plan_follows_paint_order_with_colour_changes_and_trims() {
    let d = doc_with(|d| {
        vec![
            stitched(d, poly(&[(0.0, 0.0), (72.0, 0.0)], false), StitchKind::Running, [255, 0, 0]),
            stitched(d, poly(&[(0.0, 72.0), (72.0, 72.0)], false), StitchKind::Running, [255, 0, 0]),
            stitched(d, poly(&[(0.0, 144.0), (72.0, 144.0)], false), StitchKind::Bean, [0, 0, 255]),
        ]
    });
    let plan = plan(&d);
    assert_eq!(plan.threads, vec![[255, 0, 0], [0, 0, 255]]);
    let s = plan.stats();
    assert_eq!(s.color_changes, 1);
    assert_eq!(s.trims, 3, "between the first two lines, before the change and at the end");
    assert_eq!(plan.needles.first().unwrap().cmd, Cmd::Jump);
    assert_eq!(plan.needles.last().unwrap().cmd, Cmd::End);
    let b = s.bounds.unwrap();
    assert!((b[2] - 25.4).abs() < 1e-9 && (b[3] - mm(144.0)).abs() < 1e-9);
    assert!(plan.warnings.is_empty());
}

#[test]
fn hidden_and_unstitched_objects_are_left_out_and_other_kinds_warned() {
    let d = doc_with(|d| {
        let mut hidden = stitched(d, poly(&[(0.0, 0.0), (72.0, 0.0)], false), StitchKind::Running, [0; 3]);
        hidden.visible = false;
        let plain = Node::path(d.alloc_id(), poly(&[(0.0, 0.0), (72.0, 0.0)], false), Appearance::default_art());
        let mut group = Node::group(d.alloc_id(), vec![]);
        group.extras_mut().stitch = Some(Box::new(StitchSettings::default()));
        vec![hidden, plain, group]
    });
    let p = plan(&d);
    assert!(p.needles.is_empty());
    assert_eq!(p.warnings.len(), 1);
    assert!(!has_stitches(&Document::new(10.0, 10.0)));
    assert!(has_stitches(&d));
}

#[test]
fn thread_comes_from_settings_then_stroke_or_fill() {
    let d = doc_with(|d| {
        let mut fill = Node::path(
            d.alloc_id(),
            poly(&[(0.0, 0.0), (72.0, 0.0), (72.0, 72.0)], true),
            Appearance::basic(Paint::solid(Color::rgb8(0, 255, 0)), Paint::solid(Color::rgb8(9, 9, 9)), 1.0),
        );
        fill.extras_mut().stitch = Some(Box::new(StitchSettings::new(StitchKind::Fill)));
        let mut over = stitched(d, poly(&[(0.0, 0.0), (72.0, 0.0)], false), StitchKind::Running, [1, 1, 1]);
        over.extras_mut().stitch.as_mut().unwrap().thread = Some([7, 8, 9]);
        vec![fill, over]
    });
    assert_eq!(plan(&d).threads, vec![[0, 255, 0], [7, 8, 9]]);
}

#[test]
fn dst_records_round_trip_every_move() {
    for dx in -121..=121 {
        for dy in [-121, -40, 0, 1, 81, 121] {
            let rec = dst::tests_record(dx, dy, 0);
            assert_eq!(dst::tests_decode(&rec), (dx, dy), "{rec:02X?}");
        }
    }
}

#[test]
fn dst_writes_a_512_byte_header_and_ends_with_an_end_record() {
    let d = doc_with(|d| vec![stitched(d, poly(&[(0.0, 0.0), (72.0, 0.0)], false), StitchKind::Running, [0; 3])]);
    let bytes = write_dst(&plan(&d), "Test design with a long name");
    assert!(is_dst(&bytes));
    assert!(bytes.starts_with(b"LA:Test design with\r"));
    assert_eq!(bytes[511], b' ');
    assert_eq!(&bytes[bytes.len() - 3..], &[0x00, 0x00, 0xF3]);
    assert_eq!((bytes.len() - 512) % 3, 0);
    let header = String::from_utf8_lossy(&bytes[..512]);
    // 25.4 mm wide, centred: ±127 units.
    assert!(header.contains("\r+X:  127\r-X:  127\r"), "{header}");
}

#[test]
fn dst_round_trips_a_design() {
    let d = doc_with(|d| {
        let mut sq = stitched(d, poly(&[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)], true), StitchKind::Fill, [0; 3]);
        sq.appearance = Appearance::basic(Paint::solid(Color::rgb8(200, 0, 0)), Paint::None, 0.0).into();
        vec![sq, stitched(d, poly(&[(0.0, 150.0), (300.0, 150.0)], false), StitchKind::Satin, [0, 0, 200])]
    });
    let p = plan(&d);
    let back = read_dst(&write_dst(&p, "x")).unwrap();
    assert!(back.warnings.is_empty(), "{:?}", back.warnings);
    let (a, b) = (p.stats(), back.stats());
    assert_eq!(a.color_changes, b.color_changes);
    assert_eq!(back.threads.len(), 2);
    // Every stitch lands within rounding of where it was planned, relative to the design.
    let (wa, wb) = (a.bounds.unwrap(), b.bounds.unwrap());
    assert!(((wa[2] - wa[0]) - (wb[2] - wb[0])).abs() <= 0.1 + 1e-9);
    assert!(((wa[3] - wa[1]) - (wb[3] - wb[1])).abs() <= 0.1 + 1e-9);
    assert!(b.stitches >= a.stitches);
}

#[test]
fn long_moves_are_split_into_short_records() {
    let plan = Plan {
        threads: vec![[0; 3]],
        needles: vec![
            Needle { x: 0.0, y: 0.0, cmd: Cmd::Jump },
            Needle { x: 50.0, y: 0.0, cmd: Cmd::Stitch },
            Needle { x: 50.0, y: 0.0, cmd: Cmd::End },
        ],
        ..Plan::default()
    };
    let back = read_dst(&write_dst(&plan, "")).unwrap();
    let stitches: Vec<&Needle> = back.needles.iter().filter(|n| n.cmd == Cmd::Stitch).collect();
    assert_eq!(stitches.len(), 5, "50 mm in moves of at most 12.1 mm");
    assert!((stitches.last().unwrap().x - stitches.first().unwrap().x - 40.0).abs() < 0.2);
}

#[test]
fn imported_designs_export_the_same_stitches() {
    let d = doc_with(|d| vec![stitched(d, poly(&[(0.0, 0.0), (72.0, 30.0), (10.0, 90.0)], false), StitchKind::Running, [0; 3])]);
    let first = read_dst(&write_dst(&plan(&d), "x")).unwrap();
    let doc = to_document(&first, "x");
    assert_eq!(doc.layers.len(), 1);
    let again = read_dst(&write_dst(&plan(&doc), "x")).unwrap();
    assert_eq!(first.stats().stitches, again.stats().stitches);
    assert_eq!(stitch_lengths(&first).len(), stitch_lengths(&again).len());
    for (a, b) in stitch_lengths(&first).iter().zip(stitch_lengths(&again)) {
        assert!((a - b).abs() < 0.15, "{a} vs {b}");
    }
}

#[test]
fn not_dst_is_an_error() {
    assert_eq!(read_dst(b"%PDF-1.7"), Err(DstError::NotDst));
    assert!(!is_dst(b"LA:"));
}

#[test]
fn palette_colours_are_distinct() {
    let v: std::collections::BTreeSet<[u8; 3]> = (0..12).map(palette_color).collect();
    assert_eq!(v.len(), 12);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Any bytes after a DST header read without panicking, into finite points.
    #[test]
    fn dst_reader_survives_any_body(body in proptest::collection::vec(any::<u8>(), 0..3000)) {
        let mut bytes = b"LA:fuzz\rST:      1\r".to_vec();
        bytes.resize(512, b' ');
        bytes.extend(body);
        let plan = read_dst(&bytes).unwrap();
        prop_assert!(plan.needles.iter().all(|n| n.x.is_finite() && n.y.is_finite()));
        let _ = to_document(&plan, "fuzz");
    }

    /// Any bytes at all: an error or a plan, never a panic.
    #[test]
    fn dst_reader_survives_anything(bytes in proptest::collection::vec(any::<u8>(), 0..1200)) {
        let _ = read_dst(&bytes);
    }

    /// Fills of arbitrary polygons stay inside their bounds and keep stitches short.
    #[test]
    fn fills_stay_inside_and_short(
        pts in proptest::collection::vec((-50.0f64..50.0, -50.0f64..50.0), 3..12),
        angle in -180.0f64..180.0,
        spacing in 0.2f64..3.0,
        len in 1.0f64..8.0,
    ) {
        let poly: Vec<Point> = pts.iter().map(|&(x, y)| Point::new(x, y)).collect();
        let runs = generate::fill(std::slice::from_ref(&poly), FillRule::EvenOdd, angle, spacing, len, 3).unwrap();
        let (x0, x1) = poly.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| (a.min(p.x), b.max(p.x)));
        let (y0, y1) = poly.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| (a.min(p.y), b.max(p.y)));
        for p in runs.iter().flatten() {
            prop_assert!(p.x >= x0 - 1e-6 && p.x <= x1 + 1e-6 && p.y >= y0 - 1e-6 && p.y <= y1 + 1e-6);
        }
        for r in &runs {
            for w in r.windows(2) {
                prop_assert!(w[0].distance(w[1]) <= len + 1e-6);
            }
        }
    }

    /// Writing then reading a random walk of stitches comes back within 0.1 mm per point.
    #[test]
    fn dst_round_trips_random_walks(steps in proptest::collection::vec((-30.0f64..30.0, -30.0f64..30.0), 1..60)) {
        let mut needles = vec![Needle { x: 0.0, y: 0.0, cmd: Cmd::Stitch }];
        let (mut x, mut y) = (0.0, 0.0);
        for (dx, dy) in steps {
            x += dx;
            y += dy;
            needles.push(Needle { x, y, cmd: Cmd::Stitch });
        }
        let plan = Plan { threads: vec![[0; 3]], needles, ..Plan::default() };
        let back = read_dst(&write_dst(&plan, "")).unwrap();
        let (a, b) = (plan.stats().bounds.unwrap(), back.stats().bounds.unwrap());
        prop_assert!(((a[2] - a[0]) - (b[2] - b[0])).abs() <= 0.11);
        prop_assert!(((a[3] - a[1]) - (b[3] - b[1])).abs() <= 0.11);
    }
}

/// A one-object document stitched with `s` along `path`.
fn plan_of(path: PathData, s: StitchSettings) -> Plan {
    let d = doc_with(|d| {
        let mut n = stitched(d, path, s.kind, [0; 3]);
        n.extras_mut().stitch = Some(Box::new(s));
        vec![n]
    });
    plan(&d)
}

#[test]
fn bean_passes_repeat_each_stitch() {
    let run = [Point::new(0.0, 0.0), Point::new(2.0, 0.0)];
    assert_eq!(generate::bean(&run, 5).len(), 1 + 5);
    let five = plan_of(poly(&[(0.0, 0.0), (72.0, 0.0)], false), StitchSettings { passes: Some(5), ..StitchSettings::new(StitchKind::Bean) });
    let three = plan_of(poly(&[(0.0, 0.0), (72.0, 0.0)], false), StitchSettings::new(StitchKind::Bean));
    assert!(five.stats().stitches > three.stats().stitches);
}

#[test]
fn pull_compensation_widens_the_column() {
    let line = poly(&[(0.0, 0.0), (72.0, 0.0)], false);
    let base = StitchSettings { width: Some(4.0), ..StitchSettings::new(StitchKind::Satin) };
    let pulled = StitchSettings { pull: Some(0.6), ..base.clone() };
    let h = |p: &Plan| p.stats().bounds.map(|b| b[3] - b[1]).unwrap();
    assert!((h(&plan_of(line.clone(), base)) - 4.0).abs() < 1e-6);
    assert!((h(&plan_of(line, pulled)) - 4.6).abs() < 1e-6);
}

#[test]
fn satin_underlays_stay_inside_and_join_the_top_stitches() {
    let line = poly(&[(0.0, 0.0), (72.0, 0.0)], false);
    for u in [Underlay::Center, Underlay::Edge, Underlay::Zigzag] {
        let s = StitchSettings { width: Some(4.0), underlay: u, ..StitchSettings::new(StitchKind::Satin) };
        let p = plan_of(line.clone(), s);
        let st = p.stats();
        // Underlay and column are sewn without a jump between them (only the jump to the start).
        assert_eq!(st.jumps, 1, "{u:?}");
        let runs = p.runs();
        let under: Vec<Point> = runs[0].1.iter().take(10).copied().collect();
        for q in &under {
            assert!(q.y.abs() <= 2.0 - 0.4 + 1e-6, "{u:?}: {q:?} outside the underlay band");
        }
        assert!(st.stitches > plan_of(line.clone(), StitchSettings { width: Some(4.0), ..StitchSettings::new(StitchKind::Satin) }).stats().stitches);
    }
}

#[test]
fn fill_underlay_runs_across_the_top_rows() {
    let sq = poly(&[(0.0, 0.0), (72.0, 0.0), (72.0, 72.0), (0.0, 72.0)], true);
    let plain = plan_of(sq.clone(), StitchSettings::new(StitchKind::Fill));
    let under = plan_of(sq, StitchSettings { underlay: Underlay::Fill, ..StitchSettings::new(StitchKind::Fill) });
    assert!(under.stats().stitches > plain.stats().stitches);
    // The first stitches (underlay) run vertically: rows at 90° to the 0° top rows.
    let first = &under.runs()[0].1;
    let vertical = first.windows(2).filter(|w| (w[0].x - w[1].x).abs() < 1e-6).count();
    assert!(vertical * 2 > first.len());
}

#[test]
fn stagger_changes_the_needle_pattern() {
    let sq = vec![Point::new(0.0, 0.0), Point::new(20.0, 0.0), Point::new(20.0, 20.0), Point::new(0.0, 20.0)];
    let xs = |stagger| -> std::collections::BTreeSet<i64> {
        generate::fill(std::slice::from_ref(&sq), FillRule::NonZero, 0.0, 1.0, 4.0, stagger).unwrap()[0]
            .iter()
            .map(|p| (p.x * 100.0).round() as i64)
            .collect()
    };
    // Inner needle columns: 4 mm apart shifted by 1/stagger of a stitch.
    assert!(xs(2).len() < xs(4).len());
}

/// A multi-object design touching every kind of travel: close runs, far jumps, colour changes.
fn busy_doc() -> Document {
    doc_with(|d| {
        let mut fill = stitched(d, poly(&[(0.0, 0.0), (90.0, 0.0), (90.0, 60.0), (0.0, 60.0)], true), StitchKind::Fill, [200, 0, 0]);
        fill.extras_mut().stitch.as_mut().unwrap().underlay = Underlay::Fill;
        vec![
            fill,
            stitched(d, poly(&[(120.0, 0.0), (200.0, 40.0)], false), StitchKind::Satin, [0, 0, 200]),
            stitched(d, poly(&[(120.0, 80.0), (200.0, 80.0)], false), StitchKind::Bean, [0, 0, 200]),
            stitched(d, poly(&[(0.0, 120.0), (60.0, 160.0), (0.0, 200.0)], true), StitchKind::Running, [0, 150, 0]),
        ]
    })
}

#[test]
fn every_run_is_anchored_after_jumps_trims_and_colour_changes() {
    let p = plan(&busy_doc());
    let n = &p.needles;
    for i in 1..n.len() {
        if n[i].cmd == Cmd::Stitch && n[i - 1].cmd != Cmd::Stitch {
            // The needle goes down where the frame stopped, before moving.
            assert_eq!((n[i].x, n[i].y), (n[i - 1].x, n[i - 1].y), "needle {i} sews from an unanchored point");
        }
    }
}

#[test]
fn trims_follow_tie_offs_and_runs_start_with_tie_ins() {
    let p = plan(&busy_doc());
    let n = &p.needles;
    let mut trims = 0;
    for i in 0..n.len() {
        if n[i].cmd == Cmd::Trim && i >= 4 {
            trims += 1;
            let tie = &n[i - 4..i];
            assert!(tie.iter().all(|t| t.cmd == Cmd::Stitch));
            // Back and forth over a short stitch, ending where the thread is cut.
            assert!((tie[0].x - tie[1].x).hypot(tie[0].y - tie[1].y) <= TIE_MM_TEST + 1e-9);
            assert_eq!((tie[3].x, tie[3].y), (n[i].x, n[i].y));
        }
        if n[i].cmd == Cmd::Jump && n.get(i + 1).is_some_and(|m| m.cmd == Cmd::Stitch) {
            let ties = &n[i + 1..(i + 6).min(n.len())];
            assert_eq!(ties.len(), 5, "an anchor and four lock stitches");
            assert_eq!((ties[0].x, ties[0].y), (ties[2].x, ties[2].y));
            assert_eq!((ties[0].x, ties[0].y), (ties[4].x, ties[4].y));
        }
    }
    assert!(trims >= 3);
}

const TIE_MM_TEST: f64 = 0.6;

#[test]
fn manual_stitches_get_no_lock_stitches() {
    let mut s = StitchSettings::new(StitchKind::Manual);
    s.thread = Some([0; 3]);
    let p = plan_of(poly(&[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)], false), s);
    let stitches: Vec<&Needle> = p.needles.iter().filter(|n| n.cmd == Cmd::Stitch).collect();
    // The anchor, then the two anchors after it; the trim at the end has no tie-off before it.
    assert_eq!(stitches.len(), 3);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Whatever the paths, every stitch after a non-stitch is anchored, and none is longer than
    /// the longest stitch any setting allows.
    #[test]
    fn plans_are_anchored_and_stitches_bounded(
        lines in proptest::collection::vec(proptest::collection::vec((0.0f64..300.0, 0.0f64..300.0), 2..8), 1..5),
        kind in 0usize..4,
        wide in any::<bool>(),
    ) {
        let kinds = [StitchKind::Running, StitchKind::Bean, StitchKind::Satin, StitchKind::Fill];
        let d = doc_with(|d| lines.iter().enumerate().map(|(i, l)| {
            let mut n = stitched(d, poly(l, kinds[kind] == StitchKind::Fill), kinds[kind], [(i * 60) as u8, 0, 0]);
            if wide {
                // The widest satin with the most pull compensation.
                let mut e = n.extras_mut();
                let s = e.stitch.as_mut().unwrap();
                s.width = Some(12.0);
                s.pull = Some(3.0);
                s.length = Some(12.0);
            }
            n
        }).collect());
        let p = plan(&d);
        let n = &p.needles;
        for i in 1..n.len() {
            if n[i].cmd == Cmd::Stitch {
                if n[i - 1].cmd != Cmd::Stitch {
                    prop_assert_eq!((n[i].x, n[i].y), (n[i - 1].x, n[i - 1].y));
                } else {
                    let len = (n[i].x - n[i - 1].x).hypot(n[i].y - n[i - 1].y);
                    prop_assert!(len <= 12.1, "stitch {len} mm: DST moves at most 12.1 mm");
                }
            }
        }
    }
}

#[test]
fn template_and_non_printing_layers_are_not_sewn() {
    for edit in [|c: &mut bool, _: &mut bool| *c = true, |_: &mut bool, p: &mut bool| *p = false] {
        let mut d = doc_with(|d| vec![stitched(d, poly(&[(0.0, 0.0), (72.0, 0.0)], false), StitchKind::Running, [0; 3])]);
        if let vectorcraft_doc::NodeKind::Layer { template, printable, .. } = &mut Arc::make_mut(&mut d.layers[0]).kind {
            edit(template, printable);
        }
        assert!(plan(&d).needles.is_empty());
    }
}

#[test]
fn masked_objects_are_stitched_with_a_warning() {
    let d = doc_with(|d| {
        let line = stitched(d, poly(&[(0.0, 0.0), (72.0, 0.0)], false), StitchKind::Running, [0; 3]);
        let clip = Node::path(d.alloc_id(), poly(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], true), Appearance::default());
        let mut g = Node::group(d.alloc_id(), vec![Arc::new(clip), Arc::new(line)]);
        if let vectorcraft_doc::NodeKind::Group { clip, .. } = &mut g.kind {
            *clip = true;
        }
        vec![g]
    });
    let p = plan(&d);
    assert!(p.stats().stitches > 0);
    assert!(p.warnings.iter().any(|w| w.contains("mask")), "{:?}", p.warnings);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Fill stitches never leave the shape: every stitch's middle is inside it (joins between
    /// rows of concave shapes included).
    #[test]
    fn fill_stitches_stay_inside_concave_shapes(
        pts in proptest::collection::vec((0.0f64..60.0, 0.0f64..60.0), 3..10),
        angle in 0.0f64..180.0,
    ) {
        // A star-shaped polygon (angles sorted around the centre): concave, never self-crossing.
        let mut pts: Vec<Point> = pts.iter().map(|&(x, y)| Point::new(x, y)).collect();
        let c = Point::new(30.0, 30.0);
        pts.sort_by(|a, b| (a.y - c.y).atan2(a.x - c.x).total_cmp(&(b.y - c.y).atan2(b.x - c.x)));
        let runs = generate::fill(std::slice::from_ref(&pts), FillRule::NonZero, angle, 0.5, 3.0, 3).unwrap();
        let polys = [pts.clone()];
        for r in &runs {
            for w in r.windows(2) {
                let mid = w[0].lerp(w[1], 0.5);
                prop_assert!(generate::tests_inside(&polys, mid), "{:?}–{:?} leaves the shape", w[0], w[1]);
            }
        }
    }
}

/// Distance from `p` to the polyline `line`.
fn to_line(p: Point, line: &[Point]) -> f64 {
    line.windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.hypot2()).clamp(0.0, 1.0);
            p.distance(a + ab * t)
        })
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn satin_corners_stay_in_the_column_and_fan_round() {
    for (pts, closed) in [
        (vec![Point::new(0.0, 0.0), Point::new(20.0, 0.0), Point::new(20.0, 20.0), Point::new(0.0, 20.0)], true),
        (vec![Point::new(0.0, 0.0), Point::new(20.0, 0.0), Point::new(5.0, 8.0)], false),
    ] {
        let mut line = pts.clone();
        if closed {
            line.push(pts[0]);
        }
        let z = generate::satin(&line, closed, 4.0, 0.4);
        for p in &z {
            // Every needle point is within the column: a half-width from the line, or at a corner's
            // miter point (√2 half-widths at a right angle, at most 3 at a hairpin).
            assert!(to_line(*p, &line) <= 2.0 * 3.0 + 1e-6, "{p:?} outside the column");
        }
        // Around each corner the outer edge is covered: consecutive outer points are close.
        for w in z.windows(3) {
            assert!(w[0].distance(w[2]) <= 2.0, "a gap between {:?} and {:?}", w[0], w[2]);
        }
    }
}

/// `EMB_DUMP=folder cargo test -p vectorcraft-embroidery dump_samples -- --ignored` writes sample
/// designs as DST for checking in other embroidery software.
#[test]
#[ignore]
fn dump_samples() {
    let Some(dir) = std::env::var_os("EMB_DUMP") else { return };
    let dir = std::path::PathBuf::from(dir);
    let sq = poly(&[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)], true);
    let zig = poly(&[(0.0, 150.0), (60.0, 130.0), (20.0, 200.0), (100.0, 180.0)], false);
    let d = doc_with(|d| {
        let mut a = stitched(d, sq.clone(), StitchKind::Satin, [20, 50, 90]);
        a.extras_mut().stitch.as_mut().unwrap().width = Some(5.0);
        a.extras_mut().stitch.as_mut().unwrap().underlay = Underlay::Edge;
        let mut b = stitched(d, zig.clone(), StitchKind::Satin, [200, 30, 30]);
        b.extras_mut().stitch.as_mut().unwrap().width = Some(3.0);
        vec![a, b]
    });
    std::fs::write(dir.join("satin_corners.dst"), write_dst(&plan(&d), "satin corners")).unwrap();
    std::fs::write(dir.join("busy.dst"), write_dst(&plan(&busy_doc()), "busy")).unwrap();
    let zz = doc_with(|d| {
        let wave = poly(&[(0.0, 0.0), (80.0, 0.0), (120.0, 40.0), (200.0, 40.0)], false);
        let mut a = stitched(d, wave, StitchKind::Zigzag, [30, 120, 60]);
        a.extras_mut().stitch.as_mut().unwrap().width = Some(4.0);
        let ring = poly(&[(20.0, 80.0), (100.0, 80.0), (100.0, 140.0), (20.0, 140.0)], true);
        vec![a, stitched(d, ring, StitchKind::Zigzag, [150, 40, 160])]
    });
    std::fs::write(dir.join("zigzag.dst"), write_dst(&plan(&zz), "zigzag")).unwrap();
    // A leaf: two curved rails meeting at the tips, a rung across the middle; and a swelling stroke.
    let curve = |sag: f64| {
        (0..=40)
            .map(|k| {
                let t = k as f64 / 40.0;
                (t * 200.0, 60.0 - sag * (std::f64::consts::PI * t).sin())
            })
            .collect::<Vec<_>>()
    };
    let leaf = PathData::new(vec![
        SubPath { anchors: curve(40.0).into_iter().map(|(x, y)| Anchor::corner(Point::new(x, y))).collect(), closed: false },
        SubPath { anchors: curve(-20.0).into_iter().map(|(x, y)| Anchor::corner(Point::new(x, y))).collect(), closed: false },
        SubPath { anchors: vec![Anchor::corner(Point::new(80.0, 0.0)), Anchor::corner(Point::new(120.0, 120.0))], closed: false },
    ]);
    let d = doc_with(|d| {
        let mut n = Node::path(d.alloc_id(), leaf, Appearance::basic(Paint::None, Paint::solid(Color::rgb8(40, 120, 40)), 1.0));
        n.extras_mut().stitch = Some(Box::new(StitchSettings { rails: true, underlay: Underlay::Center, ..StitchSettings::new(StitchKind::Satin) }));
        vec![n]
    });
    std::fs::write(dir.join("rails.dst"), write_dst(&plan(&d), "rails")).unwrap();
    // One 30 mm square per carved motif.
    let d = doc_with(|d| {
        crate::motifs::MOTIFS
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let x = i as f64 * 100.0;
                let mut n = Node::path(
                    d.alloc_id(),
                    poly(&[(x, 0.0), (x + 85.0, 0.0), (x + 85.0, 85.0), (x, 85.0)], true),
                    Appearance::basic(Paint::solid(Color::rgb8(30, 60, 140)), Paint::None, 0.0),
                );
                n.extras_mut().stitch = Some(Box::new(StitchSettings {
                    motif: Some((*m).to_string()),
                    motif_size: Some(10.0),
                    spacing: Some(0.5),
                    length: Some(4.0),
                    ..StitchSettings::new(StitchKind::Fill)
                }));
                n
            })
            .collect()
    });
    std::fs::write(dir.join("motifs.dst"), write_dst(&plan(&d), "motifs")).unwrap();
    // Each starter border pattern (a row each) round a rectangle, along an S curve and round a
    // circle, 6 mm wide.
    let pt = |x: f64, y: f64| (x / PT_MM, y / PT_MM);
    let d = doc_with(|d| {
        let mut out = vec![];
        for (i, id) in crate::borders::BUILTIN.iter().enumerate() {
            let y = i as f64 * 60.0;
            let rect = poly(&[pt(0.0, y), pt(70.0, y), pt(70.0, y + 40.0), pt(0.0, y + 40.0)], true);
            let s: Vec<(f64, f64)> = (0..=80)
                .map(|k| {
                    let t = k as f64 / 80.0;
                    pt(90.0 + 80.0 * t, y + 20.0 + 15.0 * (2.0 * std::f64::consts::PI * t).sin())
                })
                .collect();
            let circle: Vec<(f64, f64)> = (0..72)
                .map(|k| {
                    let a = k as f64 * std::f64::consts::TAU / 72.0;
                    pt(210.0 + 20.0 * a.cos(), y + 20.0 + 20.0 * a.sin())
                })
                .collect();
            for path in [rect, poly(&s, false), poly(&circle, true)] {
                let mut n = stitched(d, path, StitchKind::PatternBorder, palette_color(i));
                n.extras_mut().stitch.as_mut().unwrap().pattern = Some((*id).to_string());
                out.push(n);
            }
        }
        out
    });
    let p = plan(&d);
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
    std::fs::write(dir.join("borders.dst"), write_dst(&p, "borders")).unwrap();
}

#[test]
fn settings_from_a_newer_version_are_kept_but_not_sewn() {
    let d = doc_with(|d| {
        let a = stitched(d, poly(&[(0.0, 0.0), (72.0, 0.0)], false), StitchKind::Running, [0; 3]);
        let mut b = stitched(d, poly(&[(0.0, 50.0), (72.0, 50.0)], false), StitchKind::Running, [0; 3]);
        b.extras_mut().stitch.as_mut().unwrap().unknown = Some(Box::new(serde_json::json!({"kind": "crossStitch"})));
        vec![a, b]
    });
    let p = plan(&d);
    assert_eq!(p.runs().len(), 1, "only the readable object is sewn");
    assert!(p.warnings.iter().any(|w| w.contains("newer VectorCraft")), "{:?}", p.warnings);
}

#[test]
fn the_cache_sews_again_only_what_changed() {
    let mut d = busy_doc();
    let mut cache = StitchCache::default();
    let first = plan_cached(&d, &mut cache);
    assert_eq!(first, plan(&d));
    let objects = cache.sewn;
    assert!(objects >= 3 && cache.reused == 0, "{objects}");
    // Nothing changed: everything is reused, and the plan is the same.
    assert_eq!(plan_cached(&d, &mut cache), first);
    assert_eq!((cache.reused, cache.sewn), (objects, 0));
    // Edit one object (the document shares the others): only it is sewn again.
    let layer = Arc::make_mut(&mut d.layers[0]);
    let kids = layer.children_mut().unwrap();
    let n = Arc::make_mut(&mut kids[0]);
    n.extras_mut().stitch.as_mut().unwrap().spacing = Some(0.8);
    let after = plan_cached(&d, &mut cache);
    assert_eq!((cache.reused, cache.sewn), (objects - 1, 1));
    assert_eq!(after, plan(&d), "the cached plan is the plan");
    assert_ne!(after, first);
    // A change to the border patterns sews everything again.
    d.embroidery.patterns.push(crate::borders::builtin("waves").unwrap());
    plan_cached(&d, &mut cache);
    assert_eq!((cache.reused, cache.sewn), (0, objects));
}

/// Two lines `gap` pt apart in one thread, the first with `first`'s settings.
fn two_lines(gap: f64, first: impl FnOnce(&mut StitchSettings), setup: EmbroiderySetup) -> Plan {
    let mut d = doc_with(|d| {
        let mut a = stitched(d, poly(&[(0.0, 0.0), (72.0, 0.0)], false), StitchKind::Running, [0; 3]);
        first(a.extras_mut().stitch.as_mut().unwrap());
        vec![a, stitched(d, poly(&[(72.0 + gap, 0.0), (144.0 + gap, 0.0)], false), StitchKind::Running, [0; 3])]
    });
    d.embroidery = setup;
    plan(&d)
}

/// Trims before the end of the design.
fn trims_between(p: &Plan) -> usize {
    p.stats().trims - 1
}

#[test]
fn jump_trim_length_is_the_documents() {
    // A 5 mm jump (14.2 pt): trimmed at the default 3 mm, not at 10 mm, nor when never trimming.
    let gap = 5.0 / PT_MM;
    assert_eq!(trims_between(&two_lines(gap, |_| {}, EmbroiderySetup::default())), 1);
    let ten = EmbroiderySetup { trim_jumps_over: Some(10.0), ..EmbroiderySetup::default() };
    assert_eq!(trims_between(&two_lines(gap, |_| {}, ten)), 0);
    let never = EmbroiderySetup { trim_jumps_over: None, ..EmbroiderySetup::default() };
    let p = two_lines(gap, |_| {}, never);
    assert_eq!((trims_between(&p), p.stats().jumps), (0, 2), "the jump to the start and the one between");
}

#[test]
fn short_moves_are_sewn_up_to_the_documents_length() {
    // 2 mm apart: jumped by default (moves up to 1 mm are sewn), sewn when moves up to 2.5 mm are.
    let gap = 2.0 / PT_MM;
    assert_eq!(two_lines(gap, |_| {}, EmbroiderySetup::default()).stats().jumps, 2);
    let sew = EmbroiderySetup { stitch_moves_under: 2.5, ..EmbroiderySetup::default() };
    assert_eq!(two_lines(gap, |_| {}, sew).stats().jumps, 1);
}

#[test]
fn objects_can_force_trims_before_and_after() {
    // Touching lines: no trim between them, unless one side asks.
    assert_eq!(trims_between(&two_lines(0.0, |_| {}, EmbroiderySetup::default())), 0);
    let after = two_lines(0.0, |s| s.trim_after = true, EmbroiderySetup::default());
    assert_eq!(trims_between(&after), 1);
    // The next object starts where the first ended: it goes down there and ties in again.
    let n = &after.needles;
    let i = n.iter().position(|n| n.cmd == Cmd::Trim).unwrap();
    assert_eq!(n[i + 1].cmd, Cmd::Stitch);
    assert_eq!((n[i + 1].x, n[i + 1].y), (n[i].x, n[i].y));
    let before = two_lines(0.0, |_| {}, EmbroiderySetup::default());
    assert_eq!(trims_between(&before), 0);
    // Trim Before on the second object.
    let mut d = doc_with(|d| {
        let a = stitched(d, poly(&[(0.0, 0.0), (72.0, 0.0)], false), StitchKind::Running, [0; 3]);
        let mut b = stitched(d, poly(&[(72.0, 0.0), (144.0, 0.0)], false), StitchKind::Running, [0; 3]);
        b.extras_mut().stitch.as_mut().unwrap().trim_before = true;
        vec![a, b]
    });
    assert_eq!(trims_between(&plan(&d)), 1);
    // A thread change trims anyway: no second trim.
    if let Some(n) = Arc::make_mut(&mut d.layers[0]).children_mut().and_then(|c| c.get_mut(1)) {
        Arc::make_mut(n).extras_mut().stitch.as_mut().unwrap().thread = Some([9, 9, 9]);
    }
    let p = plan(&d);
    assert_eq!((trims_between(&p), p.stats().color_changes), (1, 1));
}

#[test]
fn zigzag_swings_across_its_width_at_its_spacing() {
    let line = [Point::new(0.0, 0.0), Point::new(20.0, 0.0)];
    let z = generate::zigzag(&line, false, 3.0, 2.0);
    assert_eq!(z.len(), 11, "a point every 2 mm along 20 mm");
    for (i, p) in z.iter().enumerate() {
        assert!((p.y.abs() - 1.5).abs() < 1e-9);
        assert!((p.x - 2.0 * i as f64).abs() < 1e-9);
    }
    // A wide zigzag with long spacing is narrowed so its diagonals can be sewn.
    let wide = generate::zigzag(&line, false, 12.0, 8.0);
    for w in wide.windows(2) {
        assert!(w[0].distance(w[1]) <= 12.0 + 1e-9);
    }
}

#[test]
fn zigzag_defaults_ignore_the_stroke() {
    let mut s = StitchSettings::new(StitchKind::Zigzag);
    assert_eq!((s.width(10.0), s.spacing()), (StitchSettings::DEFAULT_WIDTH, 2.0));
    s.kind = StitchKind::Satin;
    assert_eq!((s.width(10.0), s.spacing()), (10.0, 0.4));
    // A 1 pt line stitched as a zigzag is 3 mm wide.
    let p = plan_of(poly(&[(0.0, 0.0), (72.0, 0.0)], false), StitchSettings::new(StitchKind::Zigzag));
    let b = p.stats().bounds.unwrap();
    assert!((b[3] - b[1] - 3.0).abs() < 1e-6, "{b:?}");
}

/// The satin of a shape stays on it: every needle point within `slack` mm of the shape.
fn on_shape(polys: &[Vec<Point>], pts: &[Point], slack: f64) -> bool {
    pts.iter().all(|p| {
        generate::tests_inside(polys, *p)
            || [(slack, 0.0), (-slack, 0.0), (0.0, slack), (0.0, -slack)]
                .iter()
                .any(|&(dx, dy)| generate::tests_inside(polys, Point::new(p.x + dx, p.y + dy)))
    })
}

#[test]
fn satin_shape_spans_a_bar_edge_to_edge() {
    let bar = vec![Point::new(0.0, 0.0), Point::new(30.0, 0.0), Point::new(30.0, 4.0), Point::new(0.0, 4.0)];
    let runs = generate::satin_shape(std::slice::from_ref(&bar), FillRule::NonZero, 0.4, 0.0, false);
    assert_eq!(runs.len(), 1);
    let r = &runs[0];
    // Points alternate between the two long edges, across the whole bar.
    let edge = r.iter().filter(|p| p.y.abs() < 0.2 || (p.y - 4.0).abs() < 0.2).count();
    assert!(edge * 10 >= r.len() * 9, "{edge} of {} on the edges", r.len());
    assert!(r.iter().any(|p| p.x < 1.0) && r.iter().any(|p| p.x > 29.0));
    assert!(on_shape(&[bar], r, 0.25));
}

#[test]
fn satin_shape_covers_an_l_and_a_ring_without_leaving_them() {
    let l = vec![
        Point::new(0.0, 0.0),
        Point::new(4.0, 0.0),
        Point::new(4.0, 20.0),
        Point::new(16.0, 20.0),
        Point::new(16.0, 24.0),
        Point::new(0.0, 24.0),
    ];
    let outer = vec![Point::new(30.0, 0.0), Point::new(50.0, 0.0), Point::new(50.0, 20.0), Point::new(30.0, 20.0)];
    let inner: Vec<Point> =
        vec![Point::new(34.0, 4.0), Point::new(46.0, 4.0), Point::new(46.0, 16.0), Point::new(34.0, 16.0)].into_iter().rev().collect();
    for (polys, under) in [(vec![l], false), (vec![outer, inner], true)] {
        let runs = generate::satin_shape(&polys, FillRule::NonZero, 0.4, 0.0, under);
        assert!(!runs.is_empty());
        for r in &runs {
            assert!(on_shape(&polys, r, 0.3), "a needle point leaves the shape");
            for w in r.windows(2) {
                assert!(w[0].distance(w[1]) <= 12.0 + 1e-9);
            }
        }
    }
}

/// `EMB_DUMP=folder cargo test -p vectorcraft-embroidery dump_letters -- --ignored` writes
/// satin-shape lettering in installed fonts as DST (skipped for fonts that aren't installed).
#[test]
#[ignore]
fn dump_letters() {
    let Some(dir) = std::env::var_os("EMB_DUMP") else { return };
    let dir = std::path::PathBuf::from(dir);
    let mut needles = vec![];
    for (i, (family, size)) in [("Arial", 90.0), ("Times New Roman", 90.0), ("Arial", 40.0)].iter().enumerate() {
        let style = vectorcraft_doc::CharStyle { font_family: family.to_string(), size: *size, ..Default::default() };
        let t = vectorcraft_doc::TextObject::point(Point::new(0.0, 100.0 * (i + 1) as f64), "Rag&8e", style);
        let lay = vectorcraft_text::layout(vectorcraft_text::FontDb::global(), &t);
        let mut b = super::Builder::new(&EmbroiderySetup::default());
        for g in &lay.glyphs {
            let path = PathData::from_bezpath(&g.outline).transformed(t.xf);
            let polys: Vec<Vec<Point>> = generate::polylines(&path).into_iter().map(|(l, _)| l).collect();
            for r in generate::satin_shape(&polys, FillRule::NonZero, 0.4, 0.2, true) {
                b.run([0, 0, 0], &r, true);
            }
        }
        needles.push(b.finish());
    }
    for (i, p) in needles.iter().enumerate() {
        std::fs::write(dir.join(format!("letters{i}.dst")), write_dst(p, "letters")).unwrap();
    }
}

#[test]
fn satin_shape_objects_take_their_fill_colour_and_cover_the_shape() {
    let d = doc_with(|d| {
        let mut n = Node::path(
            d.alloc_id(),
            poly(&[(0.0, 0.0), (100.0, 0.0), (100.0, 12.0), (0.0, 12.0)], true),
            Appearance::basic(Paint::solid(Color::rgb8(10, 120, 30)), Paint::None, 0.0),
        );
        n.extras_mut().stitch = Some(Box::new(StitchSettings { underlay: Underlay::Center, ..StitchSettings::new(StitchKind::SatinShape) }));
        vec![n]
    });
    let p = plan(&d);
    assert_eq!(p.threads, vec![[10, 120, 30]]);
    let b = p.stats().bounds.unwrap();
    // 100 × 12 pt: 35.3 × 4.2 mm, covered edge to edge.
    assert!(b[2] - b[0] > 34.0 && (b[3] - b[1] - mm(12.0)).abs() < 0.3, "{b:?}");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// Any polygon (self-crossing included): satin shape never panics, and its points are finite
    /// and its stitches sewable.
    #[test]
    fn satin_shape_survives_any_polygon(pts in proptest::collection::vec((-30.0f64..30.0, -30.0f64..30.0), 0..12), spacing in 0.2f64..2.0) {
        let poly: Vec<Point> = pts.iter().map(|&(x, y)| Point::new(x, y)).collect();
        for r in generate::satin_shape(&[poly], FillRule::EvenOdd, spacing, 0.3, true) {
            prop_assert!(r.iter().all(|p| p.is_finite()));
            for w in r.windows(2) {
                prop_assert!(w[0].distance(w[1]) <= 12.0 + 1e-6, "{} mm", w[0].distance(w[1]));
            }
        }
    }
}

#[test]
fn a_satin_shape_with_no_strokes_is_filled_with_a_warning() {
    let p = plan_of(poly(&[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)], true), StitchSettings::new(StitchKind::SatinShape));
    assert!(p.stats().stitches > 100);
    assert!(p.warnings.iter().any(|w| w.contains("filled instead")), "{:?}", p.warnings);
}

#[test]
fn a_fill_border_is_a_satin_over_the_edges_sewn_last() {
    let sq = poly(&[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)], true);
    let plain = plan_of(sq.clone(), StitchSettings::new(StitchKind::Fill));
    let bordered = plan_of(sq, StitchSettings { border: Some(2.0), ..StitchSettings::new(StitchKind::Fill) });
    assert!(bordered.stats().stitches > plain.stats().stitches + 300);
    // Centred on the edge: the design grows by half the border each side.
    let (a, b) = (plain.stats().bounds.unwrap(), bordered.stats().bounds.unwrap());
    assert!(((b[2] - b[0]) - (a[2] - a[0]) - 2.0).abs() < 0.2, "{a:?} {b:?}");
    // The last run is the border: every point within 1 mm of the square's outline.
    let side = mm(100.0);
    let runs = bordered.runs();
    let last = &runs.last().unwrap().1;
    for p in last.iter().skip(5) {
        let d = p.x.abs().min((p.x - side).abs()).min(p.y.abs()).min((p.y - side).abs());
        assert!(d <= 1.0 + 1e-6, "{p:?}");
    }
}

#[test]
fn stroke_width_is_twice_area_over_perimeter() {
    let bar = vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0), Point::new(100.0, 4.0), Point::new(0.0, 4.0)];
    assert!((stroke_width(&[bar]) - 3.85).abs() < 0.01, "a 4 mm bar: {}", stroke_width(&[]));
    assert_eq!(stroke_width(&[]), 0.0);
}

#[test]
fn auto_picks_by_stroke_width() {
    let s = StitchSettings::new(StitchKind::Auto);
    assert_eq!(auto_settings(&s, 0.5).kind, StitchKind::Bean);
    let mid = auto_settings(&s, 3.0);
    assert_eq!((mid.kind, mid.underlay), (StitchKind::SatinShape, Underlay::Auto));
    let wide = auto_settings(&s, 9.0);
    assert_eq!((wide.kind, wide.underlay, wide.border), (StitchKind::Fill, Underlay::Auto, Some(2.0)));
    let tuned = StitchSettings { thin_under: Some(2.0), wide_over: Some(10.0), ..s };
    assert_eq!(auto_settings(&tuned, 1.5).kind, StitchKind::Bean);
    assert_eq!(auto_settings(&tuned, 9.0).kind, StitchKind::SatinShape);
}

#[test]
fn auto_objects_sew_what_they_pick() {
    // A 2 mm-wide bar (5.7 pt): satin; a 30 mm square: fill with a border.
    let bar = plan_of(poly(&[(0.0, 0.0), (200.0, 0.0), (200.0, 5.7), (0.0, 5.7)], true), StitchSettings::new(StitchKind::Auto));
    let sq = plan_of(poly(&[(0.0, 0.0), (85.0, 0.0), (85.0, 85.0), (0.0, 85.0)], true), StitchSettings::new(StitchKind::Auto));
    assert!(bar.stats().stitches > 100 && sq.stats().stitches > bar.stats().stitches);
    assert!(sq.warnings.is_empty() && bar.warnings.is_empty(), "{:?} {:?}", sq.warnings, bar.warnings);
}

/// A document with one line of type, `text` at `size` pt in `color`, stitched with `s`.
fn type_doc(text: &str, size: f64, s: StitchSettings) -> Document {
    doc_with(|d| {
        let style = vectorcraft_doc::CharStyle { size, ..Default::default() };
        let t = vectorcraft_doc::TextObject::point(Point::new(10.0, 100.0), text, style);
        let mut n = Node::new(d.alloc_id(), vectorcraft_doc::NodeKind::Text(Box::new(t)));
        n.extras_mut().stitch = Some(Box::new(s));
        vec![n]
    })
}

/// Whether the default type face lays out (fonts may not be built in).
fn type_lays_out() -> bool {
    let t = vectorcraft_doc::TextObject::point(Point::ZERO, "H", vectorcraft_doc::CharStyle::default());
    !vectorcraft_text::layout(vectorcraft_text::FontDb::global(), &t).glyphs.iter().all(|g| g.outline.elements().is_empty())
}

#[test]
fn type_is_sewn_letter_by_letter() {
    if !type_lays_out() {
        return;
    }
    let p = plan(&type_doc("HI", 72.0, StitchSettings::new(StitchKind::Auto)));
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
    assert!(p.stats().stitches > 200);
    // Two letters: sewn as (at least) two runs, the I after the H.
    let runs = p.runs();
    assert!(runs.len() >= 2);
    let b = p.stats().bounds.unwrap();
    assert!(b[2] - b[0] > 10.0 && b[3] - b[1] > 8.0, "{b:?}");
}

#[test]
fn every_stitch_kind_sews_type() {
    if !type_lays_out() {
        return;
    }
    for k in StitchKind::ALL {
        let p = plan(&type_doc("a", 48.0, StitchSettings::new(k)));
        assert!(p.stats().stitches > 10, "{k:?}");
        assert!(p.needles.iter().all(|n| n.x.is_finite() && n.y.is_finite()));
    }
}

#[test]
fn satin_shape_fills_parts_without_strokes() {
    // An "i": a bar (stroke) and a round-ish dot above it (no stroke): the dot is filled.
    let stem = vec![Point::new(0.0, 6.0), Point::new(2.0, 6.0), Point::new(2.0, 20.0), Point::new(0.0, 20.0)];
    let dot: Vec<Point> = (0..16)
        .map(|k| {
            let a = k as f64 * std::f64::consts::TAU / 16.0;
            Point::new(1.0 + 1.2 * a.cos(), 2.0 + 1.2 * a.sin())
        })
        .collect();
    let runs = generate::satin_shape(&[stem, dot], FillRule::NonZero, 0.4, 0.0, false);
    assert!(runs.iter().flatten().any(|p| p.y < 3.5), "the dot is sewn");
    // A ring's hole is not a part: nothing is sewn inside it.
    let outer = vec![Point::new(0.0, 0.0), Point::new(20.0, 0.0), Point::new(20.0, 20.0), Point::new(0.0, 20.0)];
    let inner: Vec<Point> =
        vec![Point::new(4.0, 4.0), Point::new(16.0, 4.0), Point::new(16.0, 16.0), Point::new(4.0, 16.0)].into_iter().rev().collect();
    let runs = generate::satin_shape(&[outer, inner], FillRule::NonZero, 0.4, 0.0, false);
    assert!(!runs.iter().flatten().any(|p| p.x > 5.0 && p.x < 15.0 && p.y > 5.0 && p.y < 15.0));
}

/// `EMB_DUMP=folder EMB_TEXT=… EMB_FONT=… EMB_SIZE=pt EMB_KIND=auto cargo test -p
/// vectorcraft-embroidery dump_type -- --ignored`: one line of type through the whole plan, as DST.
#[test]
#[ignore]
fn dump_type() {
    let Some(dir) = std::env::var_os("EMB_DUMP") else { return };
    let var = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
    let text = var("EMB_TEXT", "hello world");
    let size: f64 = var("EMB_SIZE", "72").parse().unwrap();
    let kind = StitchKind::parse(&var("EMB_KIND", "auto")).unwrap();
    let style = vectorcraft_doc::CharStyle { font_family: var("EMB_FONT", "Impact"), size, ..Default::default() };
    let d = doc_with(|d| {
        let t = vectorcraft_doc::TextObject::point(Point::new(10.0, 100.0), &text, style);
        let mut n = Node::new(d.alloc_id(), vectorcraft_doc::NodeKind::Text(Box::new(t)));
        n.extras_mut().stitch = Some(Box::new(StitchSettings::new(kind)));
        vec![n]
    });
    let p = plan(&d);
    let st = p.stats();
    println!("stitches {} trims {} jumps {} warnings {:?}", st.stitches, st.trims, st.jumps, p.warnings);
    std::fs::write(std::path::PathBuf::from(dir).join("type.dst"), write_dst(&p, "type")).unwrap();
}

/// `EMB_DUMP=folder EMB_TEXT=e EMB_FONT=Impact cargo test -p vectorcraft-embroidery dump_glyph_debug -- --ignored`:
/// the first glyph's outline, strokes and satin as JSON, for plotting.
#[test]
#[ignore]
fn dump_glyph_debug() {
    let Some(dir) = std::env::var_os("EMB_DUMP") else { return };
    let var = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
    let style = vectorcraft_doc::CharStyle { font_family: var("EMB_FONT", "Impact"), size: 72.0, ..Default::default() };
    let t = vectorcraft_doc::TextObject::point(Point::new(10.0, 100.0), &var("EMB_TEXT", "e"), style);
    let lay = vectorcraft_text::layout(vectorcraft_text::FontDb::global(), &t);
    let g = lay.glyphs.first().unwrap();
    let path = PathData::from_bezpath(&g.outline).transformed(t.xf);
    let polys: Vec<Vec<Point>> = generate::polylines(&path).into_iter().map(|(l, _)| l).collect();
    let strokes = crate::skeleton::strokes(&polys, FillRule::NonZero);
    let satin = generate::satin_shape(&polys, FillRule::NonZero, 0.4, 0.0, false);
    let pts = |v: &[Point]| v.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>();
    let j = serde_json::json!({
        "width": stroke_width(&polys),
        "polys": polys.iter().map(|p| pts(p)).collect::<Vec<_>>(),
        "strokes": strokes.iter().map(|s| serde_json::json!({"points": pts(&s.points), "radius": s.radius})).collect::<Vec<_>>(),
        "satin": satin.iter().map(|r| pts(r)).collect::<Vec<_>>(),
    });
    std::fs::write(std::path::PathBuf::from(dir).join(format!("glyph_{}.json", var("EMB_TEXT", "e"))), j.to_string()).unwrap();
}

/// `EMB_FONT=… EMB_TEXT=… cargo test -p vectorcraft-embroidery report_satin_quality -- --ignored --nocapture`:
/// each glyph's satin quality and stroke width.
#[test]
#[ignore]
fn report_satin_quality() {
    let var = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
    for family in var("EMB_FONT", "Impact,Arial,Times New Roman,Arial Black").split(',') {
        let style =
            vectorcraft_doc::CharStyle { font_family: family.to_string(), size: var("EMB_SIZE", "72").parse().unwrap(), ..Default::default() };
        let text = var("EMB_TEXT", "hello world ABGRSW&8");
        let t = vectorcraft_doc::TextObject::point(Point::new(10.0, 100.0), &text, style);
        let lay = vectorcraft_text::layout(vectorcraft_text::FontDb::global(), &t);
        let mut line = String::new();
        for g in &lay.glyphs {
            let ch = text.get(g.byte..).and_then(|s| s.chars().next()).unwrap_or('?');
            let path = PathData::from_bezpath(&g.outline).transformed(t.xf);
            let polys: Vec<Vec<Point>> = generate::polylines(&path).into_iter().map(|(l, _)| l).collect();
            if polys.is_empty() {
                continue;
            }
            let (_, q) = generate::satin_shape_checked(&polys, FillRule::NonZero, 0.4, 0.0, false);
            line += &format!("{ch}:{:.0}/{:.0}/{:.0}{} ", q.skipped * 100.0, q.crowded * 100.0, q.fanned * 100.0, if q.poor() { "*" } else { "" });
        }
        println!("{family:16} {line}");
    }
}

/// A "D": a half circle closed by a straight side (curves between two corners).
fn d_shape(r: f64) -> Vec<Point> {
    let mut v: Vec<Point> = (0..=24)
        .map(|k| {
            let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * k as f64 / 24.0;
            Point::new(r * a.cos(), r * a.sin())
        })
        .collect();
    v.push(Point::new(0.0, r));
    v
}

#[test]
fn borders_on_curves_between_corners_keep_short_stitches() {
    for w in [1.0, 2.0, 4.0] {
        let b = generate::border(&d_shape(10.0), w);
        for s in b.windows(2) {
            assert!(s[0].distance(s[1]) <= w * 2.5 + 0.5, "{w} mm border: a {} mm stitch", s[0].distance(s[1]));
        }
    }
}

#[test]
fn satin_quality_tells_strokes_from_heavy_bowls() {
    let bar = vec![Point::new(0.0, 0.0), Point::new(40.0, 0.0), Point::new(40.0, 3.0), Point::new(0.0, 3.0)];
    let (_, q) = generate::satin_shape_checked(&[bar], FillRule::NonZero, 0.4, 0.0, false);
    assert!(!q.poor(), "{q:?}");
    // A heavy ring round a tiny counter (heavy type's o): satin fans: poor.
    let ring = |r: f64| {
        (0..48)
            .map(|k| {
                let a = k as f64 * std::f64::consts::TAU / 48.0;
                Point::new(r * a.cos(), r * a.sin())
            })
            .collect::<Vec<_>>()
    };
    let inner: Vec<Point> = ring(0.8).into_iter().rev().collect();
    let (_, q) = generate::satin_shape_checked(&[ring(6.0), inner], FillRule::NonZero, 0.4, 0.0, false);
    assert!(q.poor(), "{q:?}");
    // Auto fills it rather than sew a poor satin.
    let path = PathData::new(vec![
        SubPath { anchors: ring(6.0 / PT_MM).into_iter().map(Anchor::corner).collect(), closed: true },
        SubPath { anchors: ring(0.8 / PT_MM).into_iter().rev().map(Anchor::corner).collect(), closed: true },
    ]);
    let d = doc_with(|d| {
        let mut n = Node::path(d.alloc_id(), path, Appearance::basic(Paint::solid(Color::rgb8(0, 0, 0)), Paint::None, 0.0));
        n.extras_mut().stitch = Some(Box::new(StitchSettings { wide_over: Some(12.0), ..StitchSettings::new(StitchKind::Auto) }));
        vec![n]
    });
    let p = plan(&d);
    // Filled (rows at 0°: many horizontal stitches), not satin.
    let flat: usize = p.runs().iter().map(|(_, r)| r.windows(2).filter(|w| (w[0].y - w[1].y).abs() < 1e-6).count()).sum();
    assert!(flat > 100, "{flat}");
}

#[test]
fn satin_shape_covers_a_sharp_turn_to_its_outer_corner() {
    // A V of 2 mm strokes with a flat foot, like a w's point: the satin reaches the foot.
    let v = vec![
        Point::new(0.0, 0.0),
        Point::new(2.2, 0.0),
        Point::new(6.0, 13.0),
        Point::new(9.8, 0.0),
        Point::new(12.0, 0.0),
        Point::new(7.3, 16.0),
        Point::new(4.7, 16.0),
    ];
    let runs = generate::satin_shape(std::slice::from_ref(&v), FillRule::NonZero, 0.4, 0.0, false);
    let pts: Vec<Point> = runs.into_iter().flatten().collect();
    assert!(pts.iter().any(|p| p.y > 15.5), "nothing sewn down to the foot");
    // Both foot corners are reached (within 1 mm).
    for corner in [Point::new(7.3, 16.0), Point::new(4.7, 16.0)] {
        assert!(pts.iter().any(|p| p.distance(corner) < 1.0), "{corner:?} left bare");
    }
    assert!(on_shape(&[v], &pts, 0.3));
}

/// One object of two subpaths: lines from (0, 0) to (100, 0) pt and (0, 11.34) to (100, 11.34) pt
/// (4 mm apart).
fn two_rails(rails: bool) -> Plan {
    let path = PathData::new(vec![
        SubPath { anchors: vec![Anchor::corner(Point::new(0.0, 0.0)), Anchor::corner(Point::new(100.0, 0.0))], closed: false },
        SubPath { anchors: vec![Anchor::corner(Point::new(0.0, 4.0 / PT_MM)), Anchor::corner(Point::new(100.0, 4.0 / PT_MM))], closed: false },
    ]);
    plan_of(path, StitchSettings { rails, ..StitchSettings::new(StitchKind::Satin) })
}

#[test]
fn between_rails_the_satin_spans_from_one_path_to_the_other() {
    let p = two_rails(true);
    let b = p.stats().bounds.unwrap();
    assert!((b[3] - b[1] - 4.0).abs() < 0.05, "{b:?}");
    assert!(p.warnings.is_empty());
    // Without the setting each path is a satin centre line of its own (the 1 pt stroke: narrow).
    let q = two_rails(false);
    let b = q.stats().bounds.unwrap();
    assert!(b[3] - b[1] > 4.0 && b[3] - b[1] < 5.0, "{b:?}");
}

#[test]
fn rails_on_a_single_path_warn_and_sew_along_it() {
    let p = plan_of(poly(&[(0.0, 0.0), (100.0, 0.0)], false), StitchSettings { rails: true, ..StitchSettings::new(StitchKind::Satin) });
    assert!(p.stats().stitches > 50);
    assert!(p.warnings.iter().any(|w| w.contains("Between Rails")), "{:?}", p.warnings);
}

#[test]
fn fill_travel_on_a_sharp_corner_stays_on_the_outline() {
    // Found by the concave-shape property test: travel walks through this quadrilateral's sharp
    // corner, which must count as on the shape, without a zero-length stitch there.
    let quad: Vec<Point> = [(0.858, 30.715), (43.499, 34.446), (57.380, 37.802), (54.374, 40.836)].iter().map(|&(x, y)| Point::new(x, y)).collect();
    let runs = generate::fill(std::slice::from_ref(&quad), FillRule::NonZero, 0.0, 0.5, 3.0, 3).unwrap();
    for r in &runs {
        for w in r.windows(2) {
            assert!(w[0].distance(w[1]) > 1e-9, "a zero-length stitch at {:?}", w[0]);
            assert!(generate::tests_inside(std::slice::from_ref(&quad), w[0].lerp(w[1], 0.5)));
        }
    }
}

#[test]
fn a_carved_motif_puts_needle_points_on_its_lines() {
    let sq = poly(&[(0.0, 0.0), (113.4, 0.0), (113.4, 113.4), (0.0, 113.4)], true);
    let plain = plan_of(sq.clone(), StitchSettings { length: Some(4.0), ..StitchSettings::new(StitchKind::Fill) });
    let carved = plan_of(
        sq,
        StitchSettings { length: Some(4.0), motif: Some("diamonds".into()), motif_size: Some(10.0), ..StitchSettings::new(StitchKind::Fill) },
    );
    assert!(carved.warnings.is_empty(), "{:?}", carved.warnings);
    // Needle points lie on the diamond lines (|x mod 10 - 5| + |y mod 10 - 5| = 5 for the
    // diamond centred in each 10 mm tile): far more of them than in the plain fill.
    let on_lines = |p: &Plan| {
        p.needles
            .iter()
            .filter(|n| {
                let (x, y) = (n.x.rem_euclid(10.0), n.y.rem_euclid(10.0));
                ((x - 5.0).abs() + (y - 5.0).abs() - 5.0).abs() < 0.05
            })
            .count()
    };
    assert!(on_lines(&carved) > 3 * on_lines(&plain).max(1), "{} vs {}", on_lines(&carved), on_lines(&plain));
    // And stitches stay sewable and inside.
    for (_, r) in carved.runs() {
        for w in r.windows(2) {
            assert!(w[0].distance(w[1]) <= 4.0 + 1e-6);
        }
    }
}

#[test]
fn an_unknown_motif_falls_back_to_a_plain_fill_with_a_warning() {
    let p = plan_of(
        poly(&[(0.0, 0.0), (60.0, 0.0), (60.0, 60.0), (0.0, 60.0)], true),
        StitchSettings { motif: Some("stars".into()), ..StitchSettings::new(StitchKind::Fill) },
    );
    assert!(p.stats().stitches > 100);
    assert!(p.warnings.iter().any(|w| w.contains("motif")));
}

#[test]
fn auto_underlay_picks_by_satin_width() {
    use vectorcraft_doc::Underlay;
    let line = [Point::new(0.0, 0.0), Point::new(30.0, 0.0)];
    let one = |w: f64, u: Underlay| generate::satin_underlay(&line, false, w, u);
    assert_eq!(one(1.5, Underlay::Auto), one(1.5, Underlay::Center), "narrow: a centre walk");
    assert_eq!(one(3.0, Underlay::Auto), one(3.0, Underlay::Edge), "medium: an edge walk");
    let (edge, _) = one(7.0, Underlay::Edge);
    let (zig, back) = one(7.0, Underlay::Zigzag);
    let wide = one(7.0, Underlay::Auto);
    assert_eq!(wide, (edge.into_iter().chain(zig).collect(), back), "wide: an edge walk, then a zigzag");
}

#[test]
fn auto_underlay_reaches_every_stitch_that_takes_one() {
    use vectorcraft_doc::Underlay;
    let stitches = |path: PathData, s: StitchSettings| plan_of(path, s).stats().stitches;
    let with = |kind: StitchKind, u: Underlay| StitchSettings { underlay: u, width: Some(6.0), ..StitchSettings::new(kind) };
    // A 30 mm fill square: an edge walk and fill underlay, more than fill underlay alone.
    let sq = || poly(&[(0.0, 0.0), (85.0, 0.0), (85.0, 85.0), (0.0, 85.0)], true);
    let (none, fill, auto) = (
        stitches(sq(), with(StitchKind::Fill, Underlay::None)),
        stitches(sq(), with(StitchKind::Fill, Underlay::Fill)),
        stitches(sq(), with(StitchKind::Fill, Underlay::Auto)),
    );
    assert!(none < fill && fill < auto, "{none} {fill} {auto}");
    // A 6 mm satin: edge walk and zigzag.
    let bar = || poly(&[(0.0, 0.0), (100.0, 0.0)], false);
    assert!(stitches(bar(), with(StitchKind::Satin, Underlay::Auto)) > stitches(bar(), with(StitchKind::Satin, Underlay::Edge)));
    // Satin shapes: a 4 mm-wide bar gets an edge walk, a 1.5 mm one a centre walk.
    let shape = |w_mm: f64| poly(&[(0.0, 0.0), (120.0, 0.0), (120.0, w_mm / PT_MM), (0.0, w_mm / PT_MM)], true);
    let s = |u: Underlay| StitchSettings { underlay: u, ..StitchSettings::new(StitchKind::SatinShape) };
    assert_eq!(plan_of(shape(4.0), s(Underlay::Auto)), plan_of(shape(4.0), s(Underlay::Edge)));
    assert_eq!(plan_of(shape(1.5), s(Underlay::Auto)), plan_of(shape(1.5), s(Underlay::Center)));
    assert!(plan_of(shape(4.0), s(Underlay::Edge)).stats().stitches > plan_of(shape(4.0), s(Underlay::None)).stats().stitches);
}

#[test]
fn edge_walks_stay_inside_their_shape() {
    // A square with a square hole: the walk keeps inside the ring, on both outlines.
    let outer = vec![Point::new(0.0, 0.0), Point::new(20.0, 0.0), Point::new(20.0, 20.0), Point::new(0.0, 20.0), Point::new(0.0, 0.0)];
    let hole = vec![Point::new(5.0, 5.0), Point::new(15.0, 5.0), Point::new(15.0, 15.0), Point::new(5.0, 15.0), Point::new(5.0, 5.0)];
    let polys = vec![outer, hole];
    let walks = generate::edge_walk(&polys, FillRule::EvenOdd);
    assert_eq!(walks.len(), 2);
    for p in walks.iter().flatten() {
        assert!(generate::inside(&polys, FillRule::EvenOdd, *p), "{p:?}");
    }
}

/// The sewn design's width and height, mm.
fn size_of(p: &Plan) -> (f64, f64) {
    let b = p.stats().bounds.unwrap();
    (b[2] - b[0], b[3] - b[1])
}

#[test]
fn fill_pull_lengthens_rows_and_push_shortens_across_them() {
    let sq = || poly(&[(0.0, 0.0), (85.0, 0.0), (85.0, 85.0), (0.0, 85.0)], true);
    let fill = |pull: Option<f64>, pct: Option<f64>, push: Option<f64>| {
        size_of(&plan_of(sq(), StitchSettings { pull, pull_percent: pct, push, ..StitchSettings::new(StitchKind::Fill) }))
    };
    let (w, h) = fill(None, None, None);
    let (pw, ph) = fill(Some(1.0), None, None);
    assert!((pw - w - 1.0).abs() < 0.15 && (ph - h).abs() < 0.05, "rows 1 mm longer: {w} → {pw}, {h} → {ph}");
    // 30 mm rows at 2%: 0.6 mm more.
    let (cw, _) = fill(Some(1.0), Some(2.0), None);
    assert!((cw - pw - 0.6).abs() < 0.15, "{pw} → {cw}");
    let (sw, sh) = fill(None, None, Some(2.0));
    assert!(h - sh > 0.5 && h - sh < 2.1 && (sw - w).abs() < 0.05, "shorter across the rows: {h} → {sh}");
}

#[test]
fn satin_push_shortens_open_columns_and_pull_percent_widens_them() {
    let line = || poly(&[(0.0, 0.0), (85.0, 0.0)], false);
    let satin = |s: StitchSettings| size_of(&plan_of(line(), StitchSettings { width: Some(4.0), ..s }));
    let base = StitchSettings::new(StitchKind::Satin);
    let (l, w) = satin(base.clone());
    let (pl, pw) = satin(StitchSettings { push: Some(2.0), ..base.clone() });
    assert!((l - pl - 2.0).abs() < 0.2 && (pw - w).abs() < 0.05, "2 mm shorter: {l} → {pl}");
    // 25% of 4 mm is 1 mm: the same as 1 mm of pull.
    let pct = plan_of(line(), StitchSettings { width: Some(4.0), pull_percent: Some(25.0), ..base.clone() });
    let mm = plan_of(line(), StitchSettings { width: Some(4.0), pull: Some(1.0), ..base });
    assert_eq!(pct, mm);
    assert!((size_of(&mm).1 - w - 1.0).abs() < 0.05);
    // A short column keeps half a millimetre.
    assert!(generate::trim_ends(&[Point::new(0.0, 0.0), Point::new(1.0, 0.0)], 3.0).len() == 2);
}

#[test]
fn the_documents_fabric_fills_in_unset_settings_and_moves_the_underlay_thresholds() {
    use vectorcraft_doc::Underlay;
    // A 30 mm fill square and a 3.5 mm satin with auto underlay, nothing else set.
    let mut d = doc_with(|d| {
        let mut sq = stitched(d, poly(&[(0.0, 0.0), (85.0, 0.0), (85.0, 85.0), (0.0, 85.0)], true), StitchKind::Fill, [0; 3]);
        sq.extras_mut().stitch = Some(Box::new(StitchSettings::new(StitchKind::Fill)));
        let mut bar = stitched(d, poly(&[(0.0, 150.0), (200.0, 150.0)], false), StitchKind::Satin, [0; 3]);
        bar.extras_mut().stitch =
            Some(Box::new(StitchSettings { width: Some(3.5), underlay: Underlay::Auto, ..StitchSettings::new(StitchKind::Satin) }));
        vec![sq, bar]
    });
    let mut cache = StitchCache::default();
    let plain = plan_cached(&d, &mut cache);
    // Leather: a more open fill and less underlay (a 3.5 mm satin gets a centre walk, not an edge walk).
    d.embroidery.fabric = fabrics::builtin("leather").map(Box::new);
    let leather = plan_cached(&d, &mut cache);
    assert_eq!(cache.sewn, 2, "a new fabric sews everything again");
    assert!(leather.stats().stitches < plain.stats().stitches, "{} {}", leather.stats().stitches, plain.stats().stitches);
    // Towels: the 3.5 mm satin is past the zigzag threshold (3 mm): edge walk and zigzag.
    d.embroidery.fabric = fabrics::builtin("towel").map(Box::new);
    let towel = plan_cached(&d, &mut cache);
    assert!(towel.stats().stitches > plain.stats().stitches);
    // An object that sets everything the fabric would gives the same stitches with it or without it.
    let own = |d: &Document| {
        let mut d = d.clone();
        let id = vectorcraft_embroidery_first_stitched(&d);
        if let Some(n) = d.node_mut(id) {
            let s = StitchSettings {
                spacing: Some(0.4),
                pull: Some(0.0),
                pull_percent: Some(0.0),
                push: Some(0.0),
                ..StitchSettings::new(StitchKind::Fill)
            };
            n.extras_mut().stitch = Some(Box::new(s));
        }
        plan(&d)
    };
    d.embroidery.fabric = fabrics::builtin("leather").map(Box::new);
    let mut no_fabric = d.clone();
    no_fabric.embroidery.fabric = None;
    // Leather's shortest stitch is 1 mm.
    no_fabric.embroidery.min_stitch = 1.0;
    assert_eq!(own(&d).objects.first().map(|o| o.needles.len()), own(&no_fabric).objects.first().map(|o| o.needles.len()));
}

/// The first stitched object's id.
fn vectorcraft_embroidery_first_stitched(d: &Document) -> vectorcraft_doc::NodeId {
    sew_order(d).first().map(|n| n.id).unwrap()
}

/// A 30 mm fill square (sewn first) and, sewn after it, `top` across its middle.
fn covered(top: Node, keep: bool, overlap: f64) -> Plan {
    let mut d = doc_with(|d| {
        let mut sq = stitched(d, poly(&[(0.0, 0.0), (85.0, 0.0), (85.0, 85.0), (0.0, 85.0)], true), StitchKind::Fill, [0; 3]);
        sq.extras_mut().stitch = Some(Box::new(StitchSettings { keep_covered: keep, ..StitchSettings::new(StitchKind::Fill) }));
        let mut top = top;
        top.id = d.alloc_id();
        vec![sq, top]
    });
    d.embroidery.overlap = overlap;
    plan(&d)
}

/// The first object's (the fill's) needle moves.
fn fill_moves(p: &Plan) -> usize {
    p.objects.first().map_or(0, |o| o.needles.len())
}

#[test]
fn fills_are_cut_away_under_what_is_sewn_on_top() {
    let mut scratch = Document::new(10.0, 10.0);
    // A 10 mm fill bar across the square's middle.
    let base = scratch.clone();
    let bar = || stitched(&mut base.clone(), poly(&[(-5.0, 28.0), (90.0, 28.0), (90.0, 57.0), (-5.0, 57.0)], true), StitchKind::Fill, [9; 3]);
    let kept = fill_moves(&covered(bar(), true, 0.5));
    let cut = fill_moves(&covered(bar(), false, 0.5));
    let cut_close = fill_moves(&covered(bar(), false, 0.0));
    assert!(cut < kept * 3 / 4, "about a third of the rows lie under the bar: {kept} → {cut}");
    assert!(cut_close < cut, "no overlap cuts more: {cut} → {cut_close}");
    // A satin column across it cuts its band; a running stitch cuts nothing.
    let mut satin = stitched(&mut scratch, poly(&[(-5.0, 42.0), (90.0, 42.0)], false), StitchKind::Satin, [9; 3]);
    satin.extras_mut().stitch = Some(Box::new(StitchSettings { width: Some(6.0), ..StitchSettings::new(StitchKind::Satin) }));
    let under_satin = fill_moves(&covered(satin, false, 0.5));
    assert!(under_satin < kept && under_satin > cut, "{kept} {under_satin} {cut}");
    let run = stitched(&mut scratch, poly(&[(-5.0, 42.0), (90.0, 42.0)], false), StitchKind::Running, [9; 3]);
    assert_eq!(fill_moves(&covered(run, false, 0.5)), kept);
    // Nothing is left on top of it: the cut fill's stitches stay outside the bar (less the overlap).
    let p = covered(bar(), false, 0.5);
    let o = p.objects.first().unwrap();
    let ys: Vec<f64> = p.needles[o.needles.clone()].iter().filter(|n| n.cmd == Cmd::Stitch).map(|n| n.y).collect();
    let (top, bottom) = (28.0 * PT_MM + 0.5 + 0.45, 57.0 * PT_MM - 0.5 - 0.45);
    let inside = ys.iter().filter(|y| **y > top && **y < bottom).count();
    assert!(inside * 20 < ys.len(), "{inside} of {} stitches under the bar (travel only)", ys.len());
}

#[test]
fn moving_what_lies_on_top_sews_the_fill_again() {
    let mut d = doc_with(|d| {
        let sq = stitched(d, poly(&[(0.0, 0.0), (85.0, 0.0), (85.0, 85.0), (0.0, 85.0)], true), StitchKind::Fill, [0; 3]);
        let bar = stitched(d, poly(&[(-5.0, 28.0), (90.0, 28.0), (90.0, 57.0), (-5.0, 57.0)], true), StitchKind::Fill, [9; 3]);
        vec![sq, bar]
    });
    let mut cache = StitchCache::default();
    let a = plan_cached(&d, &mut cache);
    let bar_id = sew_order(&d).get(1).map(|n| n.id).unwrap();
    // Move the bar off the square: the square is sewn again, whole.
    if let Some(n) = d.node_mut(bar_id)
        && let NodeKind::Path { path, .. } = &mut n.kind
    {
        *path = poly(&[(200.0, 28.0), (290.0, 28.0), (290.0, 57.0), (200.0, 57.0)], true);
    }
    let b = plan_cached(&d, &mut cache);
    assert_eq!(cache.sewn, 2, "the bar changed, and so did what covers the square");
    assert!(fill_moves(&b) > fill_moves(&a));
    let c = plan_cached(&d, &mut cache);
    assert_eq!((cache.sewn, fill_moves(&c)), (0, fill_moves(&b)), "nothing changed: all from the cache");
}

/// The stitches of `p` shorter than `mm` (moves between two stitch needles), leaving out the
/// lock stitches at a one-run design's start and end (they are meant to be tiny).
fn short_stitches(p: &Plan, mm: f64) -> usize {
    let n = p.needles.len();
    p.needles
        .get(6..n.saturating_sub(6))
        .unwrap_or_default()
        .windows(2)
        .filter(|w| w[0].cmd == Cmd::Stitch && w[1].cmd == Cmd::Stitch && Point::new(w[0].x, w[0].y).distance(Point::new(w[1].x, w[1].y)) < mm)
        .filter(|w| w[0] != w[1])
        .count()
}

#[test]
fn stitches_shorter_than_the_shortest_are_left_out() {
    // A 0.5 mm satin: each zig is a stitch shorter than 1 mm.
    let doc = |kind: StitchKind, min: f64, fabric: Option<f64>| {
        let mut d = doc_with(|d| {
            let mut n = stitched(d, poly(&[(0.0, 0.0), (170.0, 0.0)], false), kind, [0; 3]);
            n.extras_mut().stitch = Some(Box::new(StitchSettings { width: Some(0.5), ..StitchSettings::new(kind) }));
            vec![n]
        });
        d.embroidery.min_stitch = min;
        d.embroidery.fabric = fabric.map(|m| Box::new(vectorcraft_doc::Fabric { min_stitch: Some(m), ..Default::default() }));
        d
    };
    let none = plan(&doc(StitchKind::Satin, 0.0, None));
    assert!(short_stitches(&none, 1.0) > 50, "{}", short_stitches(&none, 1.0));
    let filtered = plan(&doc(StitchKind::Satin, 1.0, None));
    assert_eq!(short_stitches(&filtered, 1.0), 0);
    assert!(filtered.stats().stitches < none.stats().stitches);
    // The run still ends where it ends.
    let end = |p: &Plan| p.needles.iter().rev().find(|n| n.cmd == Cmd::Stitch).map(|n| (n.x, n.y));
    assert_eq!(end(&filtered), end(&none));
    // A fabric asking for longer wins; a shorter one doesn't lower the document's.
    assert_eq!(plan(&doc(StitchKind::Satin, 0.0, Some(1.0))), filtered);
    assert_eq!(plan(&doc(StitchKind::Satin, 1.0, Some(0.1))), filtered);
    // Manual stitches are sewn as drawn.
    assert_eq!(plan(&doc(StitchKind::Manual, 1.0, None)), plan(&doc(StitchKind::Manual, 0.0, None)));
}

/// How far the needle jumps in `p`, mm.
fn jumped(p: &Plan) -> f64 {
    p.needles.windows(2).filter(|w| w[1].cmd == Cmd::Jump).map(|w| Point::new(w[0].x, w[0].y).distance(Point::new(w[1].x, w[1].y))).sum()
}

#[test]
fn lines_are_entered_nearest_where_the_needle_is() {
    // Three lines drawn right to left, one under the other: sewn as drawn, the needle would jump
    // back across each time; entered from their nearer ends it zigzags down instead.
    let d = |auto: bool| {
        let mut d = doc_with(|d| {
            (0..3)
                .map(|k| {
                    let y = k as f64 * 30.0;
                    stitched(d, poly(&[(200.0, y), (0.0, y)], false), StitchKind::Running, [0; 3])
                })
                .collect()
        });
        d.embroidery.auto_start = auto;
        d
    };
    let (fixed, auto) = (plan(&d(false)), plan(&d(true)));
    assert!(jumped(&auto) < jumped(&fixed) / 2.0, "{} {}", jumped(&auto), jumped(&fixed));
    assert_eq!(auto.stats().stitches, fixed.stats().stitches, "the same stitches, entered differently");
}

#[test]
fn a_start_point_sets_where_a_line_begins() {
    // A closed running square, started at its bottom-right corner.
    let corner = Point::new(85.0 * PT_MM, 85.0 * PT_MM);
    let d = doc_with(|d| {
        let mut n = stitched(d, poly(&[(0.0, 0.0), (85.0, 0.0), (85.0, 85.0), (0.0, 85.0)], true), StitchKind::Running, [0; 3]);
        n.extras_mut().stitch = Some(Box::new(StitchSettings { start_at: Some([corner.x, corner.y]), ..StitchSettings::new(StitchKind::Running) }));
        vec![n]
    });
    let p = plan(&d);
    let first = p.needles.iter().find(|n| n.cmd == Cmd::Stitch).map(|n| Point::new(n.x, n.y)).unwrap();
    assert!(first.distance(corner) < 0.01, "{first:?}");
    // Generated runs stay within the machine's reach after turning.
    assert!(generate_lengths_ok(&p));
}

fn generate_lengths_ok(p: &Plan) -> bool {
    p.needles.windows(2).filter(|w| w[1].cmd == Cmd::Stitch).all(|w| Point::new(w[0].x, w[0].y).distance(Point::new(w[1].x, w[1].y)) <= 12.1)
}

#[test]
fn puff_stops_for_the_foam_caps_and_perforates() {
    // A flat running line, then a 5 mm puff satin on 3 mm foam.
    let d = doc_with(|d| {
        let flat = stitched(d, poly(&[(0.0, 100.0), (100.0, 100.0)], false), StitchKind::Running, [0; 3]);
        let mut puff = stitched(d, poly(&[(0.0, 0.0), (100.0, 0.0)], false), StitchKind::Satin, [0; 3]);
        puff.extras_mut().stitch = Some(Box::new(StitchSettings { width: Some(5.0), puff: Some(3.0), ..StitchSettings::new(StitchKind::Satin) }));
        vec![flat, puff]
    });
    let p = plan(&d);
    assert_eq!(p.stats().color_changes, 1, "a stop for the foam");
    assert_eq!(p.threads.len(), 2, "the same thread after the stop");
    let puff = p.objects.get(1).unwrap();
    let pts: Vec<Point> = p.needles[puff.needles.clone()].iter().filter(|n| n.cmd == Cmd::Stitch).map(|n| Point::new(n.x, n.y)).collect();
    // Capped: the column reaches 1.5 mm past each drawn end (100 pt = 35.3 mm long).
    let (lo, hi) = pts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), q| (lo.min(q.x), hi.max(q.x)));
    assert!(lo < -1.4 && hi > 100.0 * PT_MM + 1.4, "{lo} {hi}");
    // Perforated: the last stitches are short, running round both edges (y = ±2.5 mm).
    let tail: Vec<Point> = pts.iter().rev().take(40).copied().collect();
    assert!(tail.windows(2).all(|w| w[0].distance(w[1]) <= 1.05), "short stitches");
    assert!(tail.iter().any(|q| q.y > 2.4) && tail.iter().any(|q| q.y < -2.4), "round both edges");
    // Dense, with no underlay: more stitches than the same satin flat.
    let flat_satin = plan_of(poly(&[(0.0, 0.0), (100.0, 0.0)], false), StitchSettings { width: Some(5.0), ..StitchSettings::new(StitchKind::Satin) });
    assert!(pts.len() > flat_satin.stats().stitches);
    // The checker: the puff is fine; a flat object after it isn't.
    let after = doc_with(|d| {
        let mut puff = stitched(d, poly(&[(0.0, 0.0), (100.0, 0.0)], false), StitchKind::Satin, [0; 3]);
        puff.extras_mut().stitch = Some(Box::new(StitchSettings { width: Some(2.0), puff: Some(3.0), ..StitchSettings::new(StitchKind::Satin) }));
        let flat = stitched(d, poly(&[(0.0, 100.0), (100.0, 100.0)], false), StitchKind::Running, [0; 3]);
        vec![puff, flat]
    });
    let found: Vec<String> = check::check(&after, &plan(&after)).into_iter().filter(|f| f.rule == "puff").map(|f| f.message).collect();
    assert_eq!(found.len(), 2, "too narrow, and a flat object after the foam: {found:?}");
}
