//! Designs bigger than one hooping (E11.4): turned in the hoop.

use serde_json::{Value, json};

use crate::*;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn b64(v: &Value) -> Vec<u8> {
    vectorcraft_format::base64_decode(v["dataBase64"].as_str().expect("dataBase64")).unwrap()
}

/// A fill `w` × `h` pt at (10, 10).
fn fill(w: f64, h: f64) -> Session {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 1200, "height": 1200}));
    run(&mut s, "paint.setFill", json!({"color": "#3060c0"}));
    run(&mut s, "shape.rectangle", json!({"x": 10, "y": 10, "width": w, "height": h}));
    run(&mut s, "embroidery.set", json!({"type": "fill"}));
    s
}

fn hoop_findings(s: &mut Session) -> Vec<Value> {
    run(s, "embroidery.check", json!({}))["findings"].as_array().unwrap().iter().filter(|f| f["rule"] == "hoop").cloned().collect()
}

#[test]
fn a_wide_design_turned_in_the_hoop_fits_and_is_written_turned() {
    // 170 × 120 mm in a 130 × 180 mm hoop: it fits only turned.
    let mut s = fill(170.0 / vectorcraft_embroidery::PT_MM, 120.0 / vectorcraft_embroidery::PT_MM);
    run(&mut s, "embroidery.setHoop", json!({"hoop": "5x7"}));
    assert_eq!(run(&mut s, "embroidery.hoops", json!({}))["fit"], "turned");
    let f = hoop_findings(&mut s);
    assert!(f.len() == 1 && f[0]["message"].as_str().unwrap().contains("Rotate in Hoop to 90°"), "{f:?}");
    let size = |bytes: &[u8]| {
        let b = vectorcraft_embroidery::read_dst(bytes).unwrap().stats().bounds.unwrap();
        (b[2] - b[0], b[3] - b[1])
    };
    let flat = size(&b64(&run(&mut s, "document.export", json!({"format": "dst"}))));
    assert!(flat.0 > flat.1, "written as drawn: {flat:?}");

    let r = run(&mut s, "embroidery.setup", json!({"hoopRotation": 90}));
    assert_eq!(r["hoopRotation"], 90);
    let r = run(&mut s, "embroidery.hoops", json!({}));
    assert_eq!((r["fit"].as_str(), r["rotation"].as_u64()), (Some("fits"), Some(90)));
    assert!(hoop_findings(&mut s).is_empty(), "the checker sees it turned");
    for format in ["dst", "pes", "jef", "exp"] {
        let bytes = b64(&run(&mut s, "document.export", json!({"format": format})));
        let plan = match format {
            "pes" => vectorcraft_embroidery::read_pes(&bytes).unwrap(),
            "jef" => vectorcraft_embroidery::read_jef(&bytes).unwrap(),
            "exp" => vectorcraft_embroidery::read_exp(&bytes),
            _ => vectorcraft_embroidery::read_dst(&bytes).unwrap(),
        };
        let b = plan.stats().bounds.unwrap();
        let turned = (b[2] - b[0], b[3] - b[1]);
        assert!((turned.0 - flat.1).abs() < 0.5 && (turned.1 - flat.0).abs() < 0.5, "{format} written turned: {turned:?} of {flat:?}");
    }
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(run(&mut s, "embroidery.hoops", json!({}))["rotation"], 0, "one undo step");
    for bad in [json!({"hoopRotation": 45}), json!({"hoopRotation": -90}), json!({"hoopRotation": "90"})] {
        assert!(s.execute("embroidery.setup", &bad).is_err(), "{bad}");
    }
}
