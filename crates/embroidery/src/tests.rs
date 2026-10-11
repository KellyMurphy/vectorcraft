use std::sync::Arc;

use kurbo::Point;
use proptest::prelude::*;
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Document, Node, StitchKind, StitchSettings};
use vectorcraft_geom::{Anchor, FillRule, PathData, SubPath};

use super::*;

fn poly(pts: &[(f64, f64)], closed: bool) -> PathData {
    PathData::single(SubPath { anchors: pts.iter().map(|&(x, y)| Anchor::corner(Point::new(x, y))).collect(), closed })
}

/// A document holding `nodes` (made with `f`, given the document for ids) on its first layer.
fn doc_with(f: impl FnOnce(&mut Document) -> Vec<Node>) -> Document {
    let mut d = Document::new(500.0, 500.0);
    let nodes = f(&mut d);
    let layer = Arc::make_mut(&mut d.layers[0]);
    layer.children_mut().unwrap().extend(nodes.into_iter().map(Arc::new));
    d
}

fn stitched(d: &mut Document, path: PathData, kind: StitchKind, stroke: [u8; 3]) -> Node {
    let mut n = Node::path(d.alloc_id(), path, Appearance::basic(Paint::None, Paint::solid(Color::rgb8(stroke[0], stroke[1], stroke[2])), 1.0));
    n.stitch = Some(Box::new(StitchSettings::new(kind)));
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
    assert!(runs.len() >= 2);
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
        group.stitch = Some(Box::new(StitchSettings::default()));
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
        fill.stitch = Some(Box::new(StitchSettings::new(StitchKind::Fill)));
        let mut over = stitched(d, poly(&[(0.0, 0.0), (72.0, 0.0)], false), StitchKind::Running, [1, 1, 1]);
        over.stitch.as_mut().unwrap().thread = Some([7, 8, 9]);
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
        sq.appearance = Appearance::basic(Paint::solid(Color::rgb8(200, 0, 0)), Paint::None, 0.0);
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
        warnings: vec![],
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
        let plan = Plan { threads: vec![[0; 3]], needles, warnings: vec![] };
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
        n.stitch = Some(Box::new(s));
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
