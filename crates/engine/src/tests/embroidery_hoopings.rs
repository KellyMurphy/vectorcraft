//! Designs bigger than one hooping (E11.4): turned in the hoop, and split into hoopings.

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

#[test]
fn a_design_too_big_for_the_hoop_is_split_exported_per_hooping_and_joined() {
    // 300 × 100 mm: three hoopings across a 130 × 180 mm hoop.
    let mm = |v: f64| v / vectorcraft_embroidery::PT_MM;
    let mut s = fill(mm(300.0), mm(100.0));
    assert!(s.execute("embroidery.splitHoops", &json!({})).is_err(), "no hoop yet");
    let r = run(&mut s, "embroidery.splitHoops", json!({"hoop": "5x7", "basting": true}));
    let h = &r["hoopings"];
    assert_eq!(
        (h["columns"].as_u64(), h["rows"].as_u64(), h["marks"].as_bool(), h["basting"].as_bool()),
        (Some(3), Some(1), Some(true), Some(true)),
        "{r}"
    );
    let pieces = h["pieces"].as_array().unwrap();
    assert_eq!(pieces.len(), 3);
    assert!(pieces.iter().all(|p| p["fits"] == true), "{r}");
    let total = run(&mut s, "embroidery.info", json!({}))["design"]["stitches"].as_u64().unwrap();
    assert_eq!(pieces.iter().map(|p| p["own"].as_u64().unwrap()).sum::<u64>(), total, "every stitch in one hooping");
    let hoops = run(&mut s, "embroidery.hoops", json!({}));
    assert_eq!((hoops["fit"].as_str(), hoops["hoopings"].as_u64()), (Some("split"), Some(3)));
    assert!(hoop_findings(&mut s).is_empty(), "each hooping fits: {:?}", hoop_findings(&mut s));

    // A file per hooping, in sewing order, each fitting the hoop.
    let e = run(&mut s, "document.export", json!({"format": "dst"}));
    let files = e["files"].as_array().unwrap();
    let names: Vec<&str> = files.iter().map(|f| f["name"].as_str().unwrap()).collect();
    assert!(names.len() == 3 && names.iter().enumerate().all(|(i, n)| n.ends_with(&format!("-hoop{}.dst", i + 1))), "{names:?}");
    for f in files {
        let b = vectorcraft_embroidery::read_dst(&b64(f)).unwrap().stats().bounds.unwrap();
        assert!(b[2] - b[0] <= 130.0 && b[3] - b[1] <= 180.0, "{b:?}");
    }
    let whole = run(&mut s, "document.export", json!({"format": "pes", "hoopings": false}));
    assert!(whole.get("files").is_none(), "hoopings: false writes the whole design");

    // Turned in the hoop, the hoopings are remade on a turned field: two across.
    run(&mut s, "embroidery.setup", json!({"hoopRotation": 90}));
    let r = run(&mut s, "embroidery.splitHoops", json!({"overlap": 4}));
    assert_eq!((r["hoopings"]["columns"].as_u64(), r["hoopings"]["overlap"].as_f64()), (Some(2), Some(4.0)), "{r}");
    assert_eq!(r["hoopings"]["basting"], true, "the split's own settings are kept");
    // Grown past its hoopings, the checker reports the hooping that no longer fits.
    run(&mut s, "embroidery.setup", json!({"hoopRotation": 0}));
    assert!(hoop_findings(&mut s).iter().any(|f| f["message"].as_str().unwrap().contains("split the design again")));

    // Joined again: one file.
    run(&mut s, "embroidery.splitHoops", json!({"clear": true}));
    assert_eq!(run(&mut s, "embroidery.hoopings", json!({}))["hoopings"], Value::Null);
    assert!(run(&mut s, "document.export", json!({"format": "dst"})).get("files").is_none());
    assert!(s.execute("document.export", &json!({"format": "dst", "hoopings": true})).is_err());
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(run(&mut s, "embroidery.hoops", json!({}))["hoopings"], 2, "one undo step");

    for bad in [json!({"overlap": 20}), json!({"marks": "yes"}), json!({"hoop": "9x9"})] {
        assert!(s.execute("embroidery.splitHoops", &bad).is_err(), "{bad}");
    }
}

#[test]
fn hoopings_export_in_several_formats_at_once() {
    // Batch export (E11.5) of a split design: a file per format per hooping.
    let mut s = fill(300.0 / vectorcraft_embroidery::PT_MM, 100.0 / vectorcraft_embroidery::PT_MM);
    run(&mut s, "embroidery.splitHoops", json!({"hoop": "5x7"}));
    let r = run(&mut s, "document.export", json!({"formats": ["dst", "pes"]}));
    for (e, ext) in r["exports"].as_array().unwrap().iter().zip(["dst", "pes"]) {
        let names: Vec<&str> = e["files"].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap()).collect();
        assert!(names.len() == 3 && names.iter().enumerate().all(|(i, n)| n.ends_with(&format!("-hoop{}.{ext}", i + 1))), "{names:?}");
    }
    let dir = std::env::temp_dir().join(format!("vc-hooping-batch-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let r = run(&mut s, "document.export", json!({"formats": ["dst", "pes"], "path": dir.join("Big.dst").to_str().unwrap()}));
    let files = r["files"].as_array().unwrap();
    assert_eq!(files.len(), 6, "{r}");
    assert!(files.iter().all(|f| std::path::Path::new(f.as_str().unwrap()).exists()), "{r}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_design_that_fits_isnt_split() {
    let mut s = fill(100.0, 100.0);
    run(&mut s, "embroidery.setHoop", json!({"hoop": "5x7"}));
    let e = s.execute("embroidery.splitHoops", &json!({})).unwrap_err().to_string();
    assert!(e.contains("fits the hoop"), "{e}");
}
