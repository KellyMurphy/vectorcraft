//! Branching: running and bean lines that meet are sewn as one network (E7.5).

use super::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn open(pts: &[(f64, f64)]) -> (Vec<Point>, bool) {
    (pts.iter().map(|&(x, y)| p(x, y)).collect(), false)
}

fn net(lines: &[(Vec<Point>, bool)]) -> Option<Vec<Vec<Point>>> {
    network::runs(lines, &|l| generate::running(l, 2.0), &|pts| pts.to_vec())
}

/// Is `q` among `run`'s needle points?
fn has(run: &[Point], q: Point) -> bool {
    run.iter().any(|r| r.distance(q) < 1e-6)
}

fn length(run: &[Point]) -> f64 {
    run.windows(2).map(|w| w[0].distance(w[1])).sum()
}

#[test]
fn a_y_is_sewn_in_one_run_each_branch_once() {
    // Three 10 mm branches from (0, 0).
    let lines = [open(&[(0.0, 0.0), (0.0, -10.0)]), open(&[(0.0, 0.0), (8.66, 5.0)]), open(&[(0.0, 0.0), (-8.66, 5.0)])];
    let runs = net(&lines).unwrap();
    assert_eq!(runs.len(), 1, "{runs:?}");
    let run = &runs[0];
    for end in [p(0.0, -10.0), p(8.66, 5.0), p(-8.66, 5.0)] {
        assert!(has(run, end), "{end:?}");
    }
    // Each branch once, and two of them back again at most (the last one needs no way back).
    let len = length(run);
    assert!((30.0 - 1e-6..=50.0 + 1e-6).contains(&len), "{len}");
    assert!(run.windows(2).all(|w| w[0].distance(w[1]) <= 2.2), "stitches stay running stitches");
}

#[test]
fn a_t_junction_joins_mid_line() {
    // A bar, and a stem whose end touches the bar's middle.
    let lines = [open(&[(0.0, 0.0), (20.0, 0.0)]), open(&[(10.0, 0.1), (10.0, 15.0)])];
    let runs = net(&lines).unwrap();
    assert_eq!(runs.len(), 1);
    for end in [p(0.0, 0.0), p(20.0, 0.0), p(10.0, 15.0)] {
        assert!(has(&runs[0], end), "{end:?}");
    }
}

#[test]
fn lines_that_dont_meet_are_left_alone_and_loops_join() {
    let apart = [open(&[(0.0, 0.0), (10.0, 0.0)]), open(&[(0.0, 5.0), (10.0, 5.0)])];
    assert!(net(&apart).is_none());
    // A square loop with a tail from one corner: one run round the loop and out the tail.
    let sq: Vec<Point> = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)].iter().map(|&(x, y)| p(x, y)).collect();
    let lines = [(sq, true), open(&[(10.0, 10.0), (20.0, 20.0)])];
    let runs = net(&lines).unwrap();
    assert_eq!(runs.len(), 1, "{runs:?}");
    assert!(has(&runs[0], p(20.0, 20.0)) && has(&runs[0], p(0.0, 10.0)));
}

#[test]
fn a_branching_object_sews_without_jumps_or_trims() {
    let mm = |v: f64| v / PT_MM;
    let tree = PathData {
        subpaths: [[(30.0, 50.0), (30.0, 20.0)], [(30.0, 35.0), (20.0, 25.0)], [(30.0, 30.0), (42.0, 22.0)]]
            .iter()
            .map(|seg| SubPath { anchors: seg.iter().map(|&(x, y)| Anchor::corner(Point::new(mm(x), mm(y)))).collect(), closed: false })
            .collect(),
    };
    for kind in [StitchKind::Running, StitchKind::Bean] {
        let d = doc_with(|d| vec![stitched(d, tree.clone(), kind, [0; 3])]);
        let s = plan(&d).stats();
        // The jump to the start, and the trim at the end: nothing between.
        assert_eq!((s.jumps, s.trims), (1, 1), "{kind:?}: {s:?}");
    }
}

#[test]
fn bean_repeats_each_line_and_goes_back_once() {
    let lines = [open(&[(0.0, 0.0), (10.0, 0.0)]), open(&[(10.0, 0.0), (10.0, 10.0)]), open(&[(10.0, 0.0), (20.0, 0.0)])];
    let stitch = |l: &[Point]| generate::running(l, 2.0);
    let plain = network::runs(&lines, &stitch, &|pts| pts.to_vec()).unwrap();
    let bean = network::runs(&lines, &stitch, &|pts| generate::bean(pts, 3)).unwrap();
    // Bean sews the 30 mm of lines three times; the way back (one 10 mm branch at most) once.
    let (lp, lb) = (length(&plain[0]), length(&bean[0]));
    assert!((lb - lp - 2.0 * 30.0).abs() < 1e-6, "{lp} {lb}");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Random lines (some meeting at shared points): never a panic, every line end reached, and
    /// no stitch longer than a running stitch.
    #[test]
    fn random_networks_reach_every_line(segs in proptest::collection::vec((0usize..6, 0usize..6), 1..12)) {
        let grid = |k: usize| p((k % 3) as f64 * 10.0, (k / 3) as f64 * 10.0);
        let lines: Vec<(Vec<Point>, bool)> = segs.iter().filter(|(a, b)| a != b).map(|&(a, b)| (vec![grid(a), grid(b)], false)).collect();
        if let Some(runs) = net(&lines) {
            for (l, _) in &lines {
                for end in [l[0], l[1]] {
                    prop_assert!(runs.iter().any(|r| has(r, end)), "{end:?} not reached");
                }
            }
            // Running stitch folds a short remainder into a line's last stitch: up to half again.
            for r in &runs {
                prop_assert!(r.windows(2).all(|w| w[0].distance(w[1]) <= 3.0));
            }
        }
    }
}
