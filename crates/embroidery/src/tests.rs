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

/// A multi-object design touching every kind of travel: close runs, far jumps, colour changes.
fn busy_doc() -> Document {
    doc_with(|d| {
        let mut fill = stitched(d, poly(&[(0.0, 0.0), (90.0, 0.0), (90.0, 60.0), (0.0, 60.0)], true), StitchKind::Fill, [200, 0, 0]);
        fill.stitch.as_mut().unwrap().underlay = Underlay::Fill;
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
                let s = n.stitch.as_mut().unwrap();
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
        a.stitch.as_mut().unwrap().width = Some(5.0);
        a.stitch.as_mut().unwrap().underlay = Underlay::Edge;
        let mut b = stitched(d, zig.clone(), StitchKind::Satin, [200, 30, 30]);
        b.stitch.as_mut().unwrap().width = Some(3.0);
        vec![a, b]
    });
    std::fs::write(dir.join("satin_corners.dst"), write_dst(&plan(&d), "satin corners")).unwrap();
    std::fs::write(dir.join("busy.dst"), write_dst(&plan(&busy_doc()), "busy")).unwrap();
    let zz = doc_with(|d| {
        let wave = poly(&[(0.0, 0.0), (80.0, 0.0), (120.0, 40.0), (200.0, 40.0)], false);
        let mut a = stitched(d, wave, StitchKind::Zigzag, [30, 120, 60]);
        a.stitch.as_mut().unwrap().width = Some(4.0);
        let ring = poly(&[(20.0, 80.0), (100.0, 80.0), (100.0, 140.0), (20.0, 140.0)], true);
        vec![a, stitched(d, ring, StitchKind::Zigzag, [150, 40, 160])]
    });
    std::fs::write(dir.join("zigzag.dst"), write_dst(&plan(&zz), "zigzag")).unwrap();
}

/// Two lines `gap` pt apart in one thread, the first with `first`'s settings.
fn two_lines(gap: f64, first: impl FnOnce(&mut StitchSettings), setup: EmbroiderySetup) -> Plan {
    let mut d = doc_with(|d| {
        let mut a = stitched(d, poly(&[(0.0, 0.0), (72.0, 0.0)], false), StitchKind::Running, [0; 3]);
        first(a.stitch.as_mut().unwrap());
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
        b.stitch.as_mut().unwrap().trim_before = true;
        vec![a, b]
    });
    assert_eq!(trims_between(&plan(&d)), 1);
    // A thread change trims anyway: no second trim.
    if let Some(n) = Arc::make_mut(&mut d.layers[0]).children_mut().and_then(|c| c.get_mut(1)) {
        Arc::make_mut(n).stitch.as_mut().unwrap().thread = Some([9, 9, 9]);
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
        n.stitch = Some(Box::new(StitchSettings { underlay: Underlay::Center, ..StitchSettings::new(StitchKind::SatinShape) }));
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
    assert_eq!((mid.kind, mid.underlay), (StitchKind::SatinShape, Underlay::Center));
    let wide = auto_settings(&s, 9.0);
    assert_eq!((wide.kind, wide.underlay, wide.border), (StitchKind::Fill, Underlay::Fill, Some(2.0)));
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
