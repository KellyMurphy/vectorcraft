//! Embroidery through the commands: stitch settings, the design info and stitches, DST export
//! and open.

use serde_json::{Value, json};

use super::*;

fn b64(v: &Value) -> Vec<u8> {
    vectorcraft_format::base64_decode(v["dataBase64"].as_str().expect("dataBase64")).unwrap()
}

/// A document with a filled square and a stroked line, the square selected.
fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 400, "height": 400})).unwrap();
    s.execute("paint.setFill", &json!({"color": "#ff0000"})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 100, "height": 100})).unwrap();
    s
}

fn info(s: &mut Session) -> Value {
    s.execute("embroidery.info", &json!({})).unwrap()
}

#[test]
fn set_stitches_the_selection_as_one_undo_step() {
    let mut s = session();
    assert_eq!(info(&mut s)["design"]["stitches"], 0);
    let r = s.execute("embroidery.set", &json!({"type": "fill", "angle": 45, "spacing": 0.5})).unwrap();
    assert_eq!(r["changed"], 1);
    let i = info(&mut s);
    assert_eq!(i["selection"]["type"], "fill");
    assert_eq!(i["selection"]["spacing"], 0.5);
    assert_eq!(i["selection"]["angle"], 45.0);
    assert_eq!(i["design"]["objects"], 1);
    assert_eq!(i["design"]["threads"], json!(["#ff0000"]));
    assert!(i["design"]["stitches"].as_u64().unwrap() > 100);
    // 100 pt ≈ 35.3 mm.
    assert!((i["design"]["widthMm"].as_f64().unwrap() - 35.3).abs() < 0.2, "{i}");
    // Editing keeps the type; null resets to the default.
    s.execute("embroidery.set", &json!({"spacing": null, "thread": "#00ff00"})).unwrap();
    let i = info(&mut s);
    assert_eq!(i["selection"]["type"], "fill");
    assert_eq!(i["selection"]["spacing"], 0.4);
    assert_eq!(i["design"]["threads"], json!(["#00ff00"]));
    s.execute("edit.undo", &json!({})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(info(&mut s)["design"]["objects"], 0);
}

#[test]
fn bad_settings_are_refused() {
    let mut s = session();
    for p in [json!({"type": "cross"}), json!({"length": 50}), json!({"spacing": "x"}), json!({"thread": "red"}), json!({"angle": "x"})] {
        assert!(s.execute("embroidery.set", &p).is_err(), "{p}");
    }
    s.execute("select.none", &json!({})).unwrap();
    assert!(s.execute("embroidery.set", &json!({"type": "running"})).is_err());
}

#[test]
fn clear_removes_stitches() {
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "satin"})).unwrap();
    assert_eq!(s.execute("embroidery.clear", &json!({})).unwrap()["changed"], 1);
    assert_eq!(info(&mut s)["design"]["objects"], 0);
    assert_eq!(s.execute("embroidery.clear", &json!({})).unwrap()["changed"], 0);
}

#[test]
fn stitches_lists_runs_in_mm() {
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "running", "length": 3})).unwrap();
    let r = s.execute("embroidery.stitches", &json!({})).unwrap();
    let runs = r["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 1);
    let pts = runs[0]["points"].as_array().unwrap();
    // A closed 4 × 35.3 mm outline at 3 mm stitches.
    assert!(pts.len() >= 47 && pts.len() <= 50, "{}", pts.len());
    assert_eq!(r["truncated"], false);
    let few = s.execute("embroidery.stitches", &json!({"limit": 5})).unwrap();
    assert_eq!(few["truncated"], true);
}

#[test]
fn dst_exports_and_opens_again() {
    let mut s = session();
    assert!(s.execute("document.export", &json!({"format": "dst"})).is_err(), "nothing stitched yet");
    s.execute("embroidery.set", &json!({"type": "fill"})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 150, "y": 10, "width": 100, "height": 20})).unwrap();
    s.execute("embroidery.set", &json!({"type": "satin", "width": 2, "thread": [0, 0, 255]})).unwrap();
    let before = info(&mut s)["design"].clone();
    let r = s.execute("document.export", &json!({"format": "dst"})).unwrap();
    let bytes = b64(&r);
    assert!(bytes.starts_with(b"LA:"));
    let o = s.execute("document.open", &json!({"name": "design.dst", "dataBase64": vectorcraft_format::base64_encode(&bytes)})).unwrap();
    assert_eq!(o["format"], "dst");
    let after = info(&mut s)["design"].clone();
    assert_eq!(after["colorChanges"], before["colorChanges"]);
    assert_eq!(after["threads"].as_array().unwrap().len(), 2);
    assert!((after["widthMm"].as_f64().unwrap() - before["widthMm"].as_f64().unwrap()).abs() <= 0.2);
    // And out again, the same stitches.
    let again = b64(&s.execute("document.export", &json!({"format": "dst"})).unwrap());
    let count = |b: &[u8]| vectorcraft_embroidery::read_dst(b).unwrap().stats().stitches;
    assert_eq!(count(&again), count(&bytes));
}

#[test]
fn dst_is_listed_and_sniffed() {
    let mut s = Session::new();
    let f = s.execute("document.formats", &json!({})).unwrap();
    assert!(f.to_string().contains("\"dst\""));
    assert!(OPEN_EXTS.contains(&"dst") && PLACE_EXTS.contains(&"dst"));
    let mut bytes = b"LA:x               \rST:      0\r".to_vec();
    bytes.resize(512, b' ');
    bytes.extend([0, 0, 0xF3]);
    // By content, whatever the name.
    assert_eq!(load::detect("x.bin", &bytes).map(|f| f.id), Some("dst"));
}
