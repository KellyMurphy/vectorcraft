//! Travel hidden under fills sewn later (E7.5).

use super::*;

/// Two short running lines 20 mm apart, then (`fill_last`) or first a fill under both, in mm
/// (points: ×1/PT_MM).
fn two_lines_and_a_fill(fill_last: bool, fill: &[(f64, f64)]) -> Document {
    let pt = |pts: &[(f64, f64)]| pts.iter().map(|&(x, y)| (x / PT_MM, y / PT_MM)).collect::<Vec<_>>();
    doc_with(|d| {
        let a = stitched(d, poly(&pt(&[(10.0, 20.0), (15.0, 20.0)]), false), StitchKind::Running, [200, 0, 0]);
        let b = stitched(d, poly(&pt(&[(35.0, 20.0), (40.0, 20.0)]), false), StitchKind::Running, [200, 0, 0]);
        let f = stitched(d, poly(&pt(fill), true), StitchKind::Fill, [0, 0, 200]);
        if fill_last { vec![a, b, f] } else { vec![f, a, b] }
    })
}

const UNDER_BOTH: [(f64, f64); 4] = [(5.0, 10.0), (45.0, 10.0), (45.0, 30.0), (5.0, 30.0)];

/// The needle moves between the end of the first line and the start of the second.
fn between(p: &Plan) -> Vec<Needle> {
    let start = p.needles.iter().position(|n| n.cmd == Cmd::Stitch && (n.x - 15.0).abs() < 0.05 && (n.y - 20.0).abs() < 0.05).unwrap();
    let end = p.needles.iter().position(|n| n.cmd == Cmd::Stitch && (n.x - 35.0).abs() < 0.05 && (n.y - 20.0).abs() < 0.05).unwrap();
    p.needles[start..=end].to_vec()
}

#[test]
fn travel_under_a_later_fill_is_sewn_not_trimmed() {
    let p = plan(&two_lines_and_a_fill(true, &UNDER_BOTH));
    let moves = between(&p);
    assert!(moves.iter().all(|n| n.cmd == Cmd::Stitch), "{moves:?}");
    // Running stitches no longer than TRAVEL_LEN, along the straight line between them.
    for w in moves.windows(2) {
        let len = Point::new(w[0].x, w[0].y).distance(Point::new(w[1].x, w[1].y));
        assert!(len <= TRAVEL_LEN + 1e-9, "{len}");
        assert!((w[1].y - 20.0).abs() < 0.6, "the travel stays on the line: {:?}", w[1]);
    }
    // Only the thread change to the fill and the end of the design trim.
    assert_eq!(p.stats().trims, 2, "{:?}", p.stats());
}

#[test]
fn travel_trims_when_not_hidden() {
    // Off, the 20 mm move trims.
    let mut d = two_lines_and_a_fill(true, &UNDER_BOTH);
    d.embroidery.hide_travel = false;
    assert!(between(&plan(&d)).iter().any(|n| n.cmd == Cmd::Trim));
    // A fill sewn first doesn't hide what is sewn after it.
    assert!(between(&plan(&two_lines_and_a_fill(false, &UNDER_BOTH))).iter().any(|n| n.cmd == Cmd::Trim));
    // Nor one the move leaves: here a gap in the middle.
    let left_only = [(5.0, 10.0), (22.0, 10.0), (22.0, 30.0), (5.0, 30.0)];
    assert!(between(&plan(&two_lines_and_a_fill(true, &left_only))).iter().any(|n| n.cmd == Cmd::Trim));
    // Nor one whose edge the move runs too close to (inside, but under the margin).
    let thin = [(5.0, 19.7), (45.0, 19.7), (45.0, 20.3), (5.0, 20.3)];
    assert!(between(&plan(&two_lines_and_a_fill(true, &thin))).iter().any(|n| n.cmd == Cmd::Trim));
}

#[test]
fn hidden_travel_is_saved_and_reported() {
    let d = EmbroiderySetup::default();
    assert!(d.hide_travel);
    let off = EmbroiderySetup { hide_travel: false, ..Default::default() };
    let json = serde_json::to_value(&off).unwrap();
    assert_eq!(json["hideTravel"], false);
    assert!(serde_json::to_value(&d).unwrap().get("hideTravel").is_none(), "the default isn't written");
    let back: EmbroiderySetup = serde_json::from_value(json).unwrap();
    assert!(!back.hide_travel);
}
