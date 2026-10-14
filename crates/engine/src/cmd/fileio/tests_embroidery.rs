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
    // A closed 4 × 35.3 mm outline at 3 mm stitches (47–50 points), plus the tie-in and tie-off
    // (4 lock stitches each).
    assert!(pts.len() >= 47 + 8 && pts.len() <= 50 + 8, "{}", pts.len());
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

#[test]
fn satin_fill_and_bean_parameters_are_set_and_checked() {
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "satin", "pull": 0.4, "underlay": "zigzag"})).unwrap();
    let sel = &info(&mut s)["selection"];
    assert_eq!((sel["pull"].clone(), sel["underlay"].clone()), (json!(0.4), json!("zigzag")));
    // A fill has no zigzag underlay; switching type keeps the setting but stops using it.
    assert!(s.execute("embroidery.set", &json!({"type": "fill", "underlay": "zigzag"})).is_err());
    s.execute("embroidery.set", &json!({"type": "fill", "stagger": 2})).unwrap();
    let sel = &info(&mut s)["selection"];
    assert_eq!((sel["underlay"].clone(), sel["stagger"].clone()), (json!("none"), json!(2)));
    s.execute("embroidery.set", &json!({"underlay": "fill"})).unwrap();
    assert_eq!(info(&mut s)["selection"]["underlay"], "fill");
    s.execute("embroidery.set", &json!({"type": "bean", "passes": 5})).unwrap();
    assert_eq!(info(&mut s)["selection"]["passes"], 5);
    for p in [json!({"passes": 4}), json!({"stagger": 9}), json!({"pull": -1}), json!({"underlay": "wavy"})] {
        assert!(s.execute("embroidery.set", &p).is_err(), "{p}");
    }
    s.execute("embroidery.set", &json!({"passes": null})).unwrap();
    assert_eq!(info(&mut s)["selection"]["passes"], 3);
}

#[test]
fn setup_sets_the_documents_trim_and_jump_lengths() {
    let mut s = session();
    assert_eq!(info(&mut s)["setup"], json!({"trimJumpsOver": 3.0, "stitchMovesUnder": 1.0}));
    let r = s.execute("embroidery.setup", &json!({"trimJumpsOver": 8, "stitchMovesUnder": 2})).unwrap();
    assert_eq!(r, json!({"trimJumpsOver": 8.0, "stitchMovesUnder": 2.0}));
    s.execute("embroidery.setup", &json!({"trimJumpsOver": null})).unwrap();
    assert_eq!(info(&mut s)["setup"]["trimJumpsOver"], Value::Null);
    for p in [json!({"trimJumpsOver": 0}), json!({"trimJumpsOver": "x"}), json!({"stitchMovesUnder": 9})] {
        assert!(s.execute("embroidery.setup", &p).is_err(), "{p}");
    }
    // One undo step each; undo goes back to the defaults.
    s.execute("edit.undo", &json!({})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(info(&mut s)["setup"]["trimJumpsOver"], 3.0);
    // Saved with the document.
    s.execute("embroidery.setup", &json!({"trimJumpsOver": 12})).unwrap();
    let saved = b64(&s.execute("document.export", &json!({"format": "vectorcraft"})).unwrap());
    s.execute("document.open", &json!({"name": "x.vectorcraft", "dataBase64": vectorcraft_format::base64_encode(&saved)})).unwrap();
    assert_eq!(info(&mut s)["setup"]["trimJumpsOver"], 12.0);
}

#[test]
fn trim_before_and_after_are_set_per_object() {
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "running", "trimBefore": true, "trimAfter": true})).unwrap();
    let sel = &info(&mut s)["selection"];
    assert_eq!((sel["trimBefore"].clone(), sel["trimAfter"].clone()), (json!(true), json!(true)));
    assert!(s.execute("embroidery.set", &json!({"trimAfter": "yes"})).is_err());
}

#[test]
fn zigzag_stitch_is_set_and_exported() {
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "zigzag", "width": 4, "spacing": 1.5})).unwrap();
    let sel = &info(&mut s)["selection"];
    assert_eq!((sel["type"].clone(), sel["width"].clone(), sel["spacing"].clone()), (json!("zigzag"), json!(4.0), json!(1.5)));
    assert!(s.execute("embroidery.set", &json!({"underlay": "zigzag"})).is_err(), "a zigzag stitch has no underlay");
    let bytes = b64(&s.execute("document.export", &json!({"format": "dst"})).unwrap());
    assert!(vectorcraft_embroidery::read_dst(&bytes).unwrap().stats().stitches > 50);
}

#[test]
fn satin_shape_is_set_and_exported() {
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "satinShape", "underlay": "center", "pull": 0.2})).unwrap();
    assert_eq!(info(&mut s)["selection"]["type"], "satinShape");
    assert!(s.execute("embroidery.set", &json!({"underlay": "zigzag"})).is_err(), "satin shape underlay is a center walk");
    // The test square is a blob, not a letter: it is filled instead, and the design says so.
    let d = &info(&mut s)["design"];
    assert!(d["stitches"].as_u64().unwrap() > 100);
    assert!(d["warnings"].to_string().contains("filled instead"), "{d}");
    // A bar is satin-stitched along its length.
    s.execute("shape.rectangle", &json!({"x": 10, "y": 150, "width": 200, "height": 12})).unwrap();
    s.execute("embroidery.set", &json!({"type": "satinShape"})).unwrap();
    let bytes = b64(&s.execute("document.export", &json!({"format": "dst"})).unwrap());
    assert!(vectorcraft_embroidery::read_dst(&bytes).unwrap().stats().stitches > 100);
}

#[test]
fn fill_border_is_set_and_cleared() {
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "fill", "border": 2.5})).unwrap();
    assert_eq!(info(&mut s)["selection"]["border"], 2.5);
    assert!(s.execute("embroidery.set", &json!({"border": 9})).is_err());
    s.execute("embroidery.set", &json!({"border": null})).unwrap();
    assert_eq!(info(&mut s)["selection"]["border"], Value::Null);
}

#[test]
fn type_can_be_stitched() {
    let mut s = session();
    s.execute("select.none", &json!({})).unwrap();
    s.execute("text.create", &json!({"x": 10, "y": 300, "text": "Hi"})).unwrap();
    s.execute("embroidery.set", &json!({"type": "auto"})).unwrap();
    let i = info(&mut s);
    assert_eq!(i["selection"]["type"], "auto");
    assert_eq!(i["design"]["objects"], 1);
    // With fonts built in, the letters are sewn; without them, nothing is (and no error).
    if !vectorcraft_text::CRAFT_FONTS.is_empty() {
        assert!(i["design"]["stitches"].as_u64().unwrap() > 50, "{i}");
    }
}

#[test]
fn rails_are_set_per_object() {
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "satin", "rails": true})).unwrap();
    assert_eq!(info(&mut s)["selection"]["rails"], true);
    assert!(s.execute("embroidery.set", &json!({"rails": 1})).is_err());
}

#[test]
fn carved_motifs_are_set_and_checked() {
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "fill", "motif": "Waves", "motifSize": 12})).unwrap();
    let sel = &info(&mut s)["selection"];
    assert_eq!((sel["motif"].clone(), sel["motifSize"].clone()), (json!("waves"), json!(12.0)));
    assert!(s.execute("embroidery.set", &json!({"motif": "stars"})).is_err());
    assert!(s.execute("embroidery.set", &json!({"motifSize": 1})).is_err());
    s.execute("embroidery.set", &json!({"motif": null})).unwrap();
    assert_eq!(info(&mut s)["selection"]["motif"], Value::Null);
}

/// Three stitched rectangles (red, blue, red) left to right; their ids.
fn three(s: &mut Session) -> Vec<u64> {
    s.execute("file.new", &json!({"width": 400, "height": 200})).unwrap();
    let mut ids = vec![];
    for (x, c) in [(10, "#ff0000"), (110, "#0000ff"), (210, "#ff0000")] {
        s.execute("select.none", &json!({})).unwrap();
        s.execute("paint.setFill", &json!({"color": c})).unwrap();
        let r = s.execute("shape.rectangle", &json!({"x": x, "y": 10, "width": 60, "height": 60})).unwrap();
        s.execute("embroidery.set", &json!({"type": "fill"})).unwrap();
        ids.push(r["id"].as_u64().unwrap());
    }
    ids
}

fn order_ids(s: &mut Session) -> Vec<u64> {
    s.execute("embroidery.order", &json!({})).unwrap()["order"].as_array().unwrap().iter().map(|o| o["id"].as_u64().unwrap()).collect()
}

#[test]
fn objects_are_sewn_in_paint_order_until_reordered() {
    let mut s = Session::new();
    let ids = three(&mut s);
    let o = s.execute("embroidery.order", &json!({})).unwrap();
    assert_eq!(order_ids(&mut s), ids);
    assert_eq!(o["custom"], false);
    assert_eq!(o["order"][1]["thread"], "#0000ff");
    // Move the last one first.
    s.execute("embroidery.reorder", &json!({"to": "first", "ids": [ids[2]]})).unwrap();
    assert_eq!(order_ids(&mut s), vec![ids[2], ids[0], ids[1]]);
    s.execute("embroidery.reorder", &json!({"to": "later", "ids": [ids[2]]})).unwrap();
    assert_eq!(order_ids(&mut s), vec![ids[0], ids[2], ids[1]]);
    s.execute("embroidery.reorder", &json!({"to": "last", "ids": [ids[0]]})).unwrap();
    assert_eq!(order_ids(&mut s), vec![ids[2], ids[1], ids[0]]);
    s.execute("embroidery.reorder", &json!({"to": "earlier", "ids": [ids[0]]})).unwrap();
    assert_eq!(order_ids(&mut s), vec![ids[2], ids[0], ids[1]]);
    // Undo steps back one move at a time.
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(order_ids(&mut s), vec![ids[2], ids[1], ids[0]]);
    // Paint order again.
    s.execute("embroidery.resetOrder", &json!({})).unwrap();
    assert_eq!(order_ids(&mut s), ids);
}

#[test]
fn the_sewing_order_drives_the_design_and_is_saved() {
    let mut s = Session::new();
    let ids = three(&mut s);
    assert_eq!(info(&mut s)["design"]["colorChanges"], 2, "red, blue, red");
    let r = s.execute("embroidery.sortByThread", &json!({})).unwrap();
    assert_eq!(r["colorChanges"], 1);
    assert_eq!(order_ids(&mut s), vec![ids[0], ids[2], ids[1]]);
    assert_eq!(info(&mut s)["design"]["colorChanges"], 1, "both reds, then blue");
    let saved = b64(&s.execute("document.export", &json!({"format": "vectorcraft"})).unwrap());
    s.execute("document.open", &json!({"name": "x.vectorcraft", "dataBase64": vectorcraft_format::base64_encode(&saved)})).unwrap();
    assert_eq!(order_ids(&mut s), vec![ids[0], ids[2], ids[1]]);
}

#[test]
fn an_explicit_order_lists_some_and_the_rest_follow() {
    let mut s = Session::new();
    let ids = three(&mut s);
    s.execute("embroidery.reorder", &json!({"order": [ids[1]]})).unwrap();
    assert_eq!(order_ids(&mut s), vec![ids[1], ids[0], ids[2]]);
    // A new object sews last; a deleted one drops out.
    s.execute("select.none", &json!({})).unwrap();
    let r = s.execute("shape.rectangle", &json!({"x": 300, "y": 100, "width": 40, "height": 40})).unwrap();
    s.execute("embroidery.set", &json!({"type": "fill"})).unwrap();
    let new = r["id"].as_u64().unwrap();
    assert_eq!(order_ids(&mut s), vec![ids[1], ids[0], ids[2], new]);
    s.execute("edit.clear", &json!({"ids": [ids[1]]})).unwrap();
    assert_eq!(order_ids(&mut s), vec![ids[0], ids[2], new]);
    // Bad requests.
    assert!(s.execute("embroidery.reorder", &json!({"order": [99999]})).is_err());
    assert!(s.execute("embroidery.reorder", &json!({"to": "sideways", "ids": [ids[0]]})).is_err());
    assert!(s.execute("embroidery.reorder", &json!({})).is_err());
}
