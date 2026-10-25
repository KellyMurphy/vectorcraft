//! Embroidery through the commands: stitch settings, the design info and stitches, DST, PES, JEF
//! and EXP export and open.

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
fn pes_jef_and_exp_export_and_open_again() {
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "fill"})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 150, "y": 10, "width": 100, "height": 20})).unwrap();
    s.execute("embroidery.set", &json!({"type": "satin", "width": 2, "thread": [0, 0, 255]})).unwrap();
    let before = info(&mut s)["design"].clone();
    for (format, magic) in [("pes", &b"#PES0060"[..]), ("jef", &[][..]), ("exp", &[][..])] {
        let r = s.execute("document.export", &json!({ "format": format })).unwrap();
        let bytes = b64(&r);
        assert!(bytes.starts_with(magic), "{format}");
        let name = format!("design.{format}");
        let o = s.execute("document.open", &json!({"name": name, "dataBase64": vectorcraft_format::base64_encode(&bytes)})).unwrap();
        assert_eq!(o["format"], format);
        let after = info(&mut s)["design"].clone();
        assert_eq!(after["colorChanges"], before["colorChanges"], "{format}");
        assert!((after["widthMm"].as_f64().unwrap() - before["widthMm"].as_f64().unwrap()).abs() <= 0.2, "{format}");
        assert!((after["heightMm"].as_f64().unwrap() - before["heightMm"].as_f64().unwrap()).abs() <= 0.2, "{format}");
        let threads = after["threads"].as_array().unwrap().clone();
        assert_eq!(threads.len(), 2, "{format}");
        if format == "pes" {
            // PES keeps the thread colours.
            assert_eq!(threads, before["threads"].as_array().unwrap().clone());
            assert!(o["warnings"].as_array().unwrap().is_empty(), "{}", o["warnings"]);
        } else {
            assert!(o["warnings"].to_string().contains("colour of its own"), "{format}: {}", o["warnings"]);
        }
        s.execute("file.close", &json!({})).unwrap();
    }
    // JEF by its content, whatever its name; EXP only by name.
    let jef = b64(&s.execute("document.export", &json!({"format": "jef"})).unwrap());
    assert_eq!(load::detect("x.bin", &jef).map(|f| f.id), Some("jef"));
    let exp = b64(&s.execute("document.export", &json!({"format": "exp"})).unwrap());
    assert_eq!(load::detect("x.exp", &exp).map(|f| f.id), Some("exp"));
    assert!(OPEN_EXTS.contains(&"pes") && PLACE_EXTS.contains(&"jef") && UNASSOCIATED_EXTS.contains(&"exp"));
}

#[test]
fn hide_travel_is_set_and_checked() {
    let mut s = session();
    assert_eq!(info(&mut s)["setup"]["hideTravel"], true);
    let r = s.execute("embroidery.setup", &json!({"hideTravel": false})).unwrap();
    assert_eq!(r["hideTravel"], false);
    assert!(!s.doc().unwrap().doc.embroidery.hide_travel);
    assert!(s.execute("embroidery.setup", &json!({"hideTravel": "no"})).is_err());
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(s.doc().unwrap().doc.embroidery.hide_travel);
}

#[test]
fn machine_format_is_set_and_checked() {
    let mut s = session();
    let r = s.execute("embroidery.setup", &json!({"machineFormat": "PES"})).unwrap();
    assert_eq!(r["machineFormat"], "pes");
    assert_eq!(s.doc().unwrap().doc.embroidery.machine_format, "pes");
    assert!(s.execute("embroidery.setup", &json!({"machineFormat": "vp3"})).is_err());
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().doc.embroidery.machine_format(), "dst");
    // A format a newer VectorCraft named is kept, and DST is used in its place.
    let mut e = vectorcraft_doc::EmbroiderySetup { machine_format: "xyz".into(), ..Default::default() };
    assert_eq!(e.machine_format(), "dst");
    let json = serde_json::to_value(&e).unwrap();
    assert_eq!(json["machineFormat"], "xyz");
    e.machine_format = "dst".into();
    assert!(serde_json::to_value(&e).unwrap().get("machineFormat").is_none(), "the default isn't written");
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
    assert_eq!(
        info(&mut s)["setup"],
        json!({"trimJumpsOver": 3.0, "stitchMovesUnder": 1.0, "overlap": 0.5, "minStitch": 0.3, "autoStart": true, "hideTravel": true, "speed": 600.0, "machineFormat": "dst"})
    );
    let r = s.execute("embroidery.setup", &json!({"trimJumpsOver": 8, "stitchMovesUnder": 2})).unwrap();
    assert_eq!(
        r,
        json!({"trimJumpsOver": 8.0, "stitchMovesUnder": 2.0, "overlap": 0.5, "minStitch": 0.3, "autoStart": true, "hideTravel": true, "speed": 600.0, "machineFormat": "dst"})
    );
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

#[test]
fn pattern_borders_are_set_checked_and_sewn() {
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "patternBorder", "pattern": "Diamonds", "width": 20, "flip": true})).unwrap();
    let got = info(&mut s);
    let sel = &got["selection"];
    assert_eq!(
        (sel["type"].clone(), sel["pattern"].clone(), sel["width"].clone(), sel["flip"].clone()),
        (json!("patternBorder"), json!("diamonds"), json!(20.0), json!(true))
    );
    assert!(got["design"]["stitches"].as_u64().unwrap() > 100, "{got}");
    assert!(got["design"]["warnings"].as_array().unwrap().is_empty(), "{got}");
    assert!(s.execute("embroidery.set", &json!({"pattern": "stars"})).is_err());
    assert!(s.execute("embroidery.set", &json!({"width": 41})).is_err());
    // 20 mm suits a pattern border, not a satin.
    assert!(s.execute("embroidery.set", &json!({"type": "satin", "width": 20})).is_err());
    s.execute("embroidery.set", &json!({"pattern": null})).unwrap();
    assert_eq!(info(&mut s)["selection"]["pattern"], "waves");
}

#[test]
fn border_pattern_files_load_into_the_document_and_save() {
    let mut s = session();
    // A starter saved as a file, renamed: a pattern of the document's own.
    let data = s.execute("embroidery.saveBorderPattern", &json!({"pattern": "chain"})).unwrap()["data"].as_str().unwrap().to_string();
    let again = s.execute("embroidery.loadBorderPattern", &json!({"data": data})).unwrap();
    assert_eq!((again["id"].clone(), again["added"].clone()), (json!("chain"), json!(false)), "the starter itself");
    let mine = data.replace("\"id\": \"chain\"", "\"id\": \"\"").replace("\"name\": \"Chain\"", "\"name\": \"Rope\"");
    let r = s.execute("embroidery.loadBorderPattern", &json!({"data": mine})).unwrap();
    assert_eq!((r["id"].clone(), r["added"].clone()), (json!("rope"), json!(true)));
    let changed = mine.replace("\"width\": 6.0", "\"width\": 5.0");
    assert_eq!(s.execute("embroidery.loadBorderPattern", &json!({"data": changed})).unwrap()["id"], "rope-2", "a different pattern, same id");
    let list = s.execute("embroidery.borderPatterns", &json!({})).unwrap();
    let ids: Vec<&str> = list["patterns"].as_array().unwrap().iter().map(|p| p["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["rope", "rope-2", "waves", "scallops", "chain", "diamonds"]);
    // Sewn with it; kept in the saved document.
    s.execute("embroidery.set", &json!({"type": "patternBorder", "pattern": "Rope"})).unwrap();
    assert!(info(&mut s)["design"]["stitches"].as_u64().unwrap() > 50);
    assert!(s.execute("embroidery.set", &json!({"pattern": "stars"})).is_err());
    assert!(s.execute("embroidery.removeBorderPattern", &json!({"pattern": "rope"})).is_err(), "in use");
    assert!(s.execute("embroidery.removeBorderPattern", &json!({"pattern": "waves"})).is_err(), "a starter");
    s.execute("embroidery.removeBorderPattern", &json!({"pattern": "rope-2"})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.execute("embroidery.borderPatterns", &json!({})).unwrap()["patterns"].as_array().unwrap().len(), 6);
    let doc = &s.doc().unwrap().doc;
    let back = vectorcraft_format::load(&vectorcraft_format::save(doc, false)).unwrap();
    assert_eq!(back.embroidery.patterns.len(), 2);
    assert!(s.execute("embroidery.loadBorderPattern", &json!({"data": "{}"})).is_err());
}

#[test]
fn a_border_is_extracted_from_an_opened_design() {
    let mut s = session();
    // A design with a border: a starter pattern sewn along a line, written as DST and opened.
    s.execute("file.new", &json!({"width": 400, "height": 200})).unwrap();
    s.execute("paint.setStroke", &json!({"color": "#204080"})).unwrap();
    let line = s.execute("path.create", &json!({"d": "M 20 50 L 380 50"})).unwrap();
    s.execute("embroidery.set", &json!({"ids": [line["id"]], "type": "patternBorder", "pattern": "diamonds"})).unwrap();
    let bytes = b64(&s.execute("document.export", &json!({"format": "dst"})).unwrap());
    s.execute("document.open", &json!({"name": "border.dst", "dataBase64": vectorcraft_format::base64_encode(&bytes)})).unwrap();
    s.execute("select.all", &json!({})).unwrap();
    let e = s.execute("embroidery.extractBorderPattern", &json!({"license": "CC0-1.0"})).unwrap();
    // 127 mm of 8 mm diamonds: 16 repeats of 7.94 mm.
    let period = e["period"].as_f64().unwrap();
    assert!((period - 127.0 / 16.0).abs() < 0.05, "{e}");
    assert!(e["repeats"].as_u64().unwrap() >= 10 && e["error"].as_f64().unwrap() < 0.3, "{e}");
    assert_eq!(e["name"], "border border");
    let list = s.execute("embroidery.borderPatterns", &json!({})).unwrap();
    let mine = &list["patterns"][0];
    assert_eq!(
        (mine["id"].clone(), mine["license"].clone(), mine["source"].clone()),
        (e["id"].clone(), json!("CC0-1.0"), json!("extracted from border.dst"))
    );
    // Saved as a file, it reads back.
    let data = s.execute("embroidery.saveBorderPattern", &json!({"pattern": e["id"]})).unwrap();
    assert!(vectorcraft_embroidery::borders::read_file(data["data"].as_str().unwrap().as_bytes()).is_ok());
    // Nothing stitched selected: an error, not a pattern.
    s.execute("select.none", &json!({})).unwrap();
    assert!(s.execute("embroidery.extractBorderPattern", &json!({})).is_err());
    assert!(s.execute("embroidery.extractBorderPattern", &json!({"period": -1})).is_err());
}

#[test]
fn a_document_from_a_newer_version_opens_saves_and_can_be_restitched() {
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "satin"})).unwrap();
    // Saved, then edited as a newer VectorCraft would write it: an unknown stitch kind on the
    // object, a member this version doesn't know, and a pattern with an unknown part stitch.
    let saved = s.execute("document.export", &json!({"format": "vectorcraft"})).unwrap();
    let bytes = b64(&saved);
    let mut v: Value = serde_json::from_slice(&bytes).unwrap();
    let mut found = false;
    fn walk(v: &mut Value, found: &mut bool) {
        if let Some(st) = v.get_mut("stitch") {
            *st = json!({"kind": "crossStitch", "cellSize": 2.5});
            *found = true;
        }
        match v {
            Value::Object(o) => o.values_mut().for_each(|c| walk(c, found)),
            Value::Array(a) => a.iter_mut().for_each(|c| walk(c, found)),
            _ => {}
        }
    }
    walk(&mut v["document"], &mut found);
    assert!(found);
    v["document"]["embroidery"] = json!({"trimJumpsOver": 3.0, "stitchMovesUnder": 1.0, "patterns": [
        {"id": "lace", "name": "Lace", "width": 6.0, "length": 8.0, "repeat": [{"stitch": "tripleSatin", "points": [[0.0, 0.0]]}]}
    ]});
    let newer = serde_json::to_vec(&v).unwrap();
    s.execute("document.open", &json!({"name": "newer.vectorcraft", "dataBase64": vectorcraft_format::base64_encode(&newer)})).unwrap();
    s.execute("select.all", &json!({})).unwrap();
    let got = info(&mut s);
    assert_eq!(got["selection"]["type"], "unknown", "{got}");
    assert!(got["design"]["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("newer VectorCraft")), "{got}");
    let pats = s.execute("embroidery.borderPatterns", &json!({})).unwrap();
    assert_eq!((pats["patterns"][0]["id"].clone(), pats["patterns"][0]["supported"].clone()), (json!("lace"), json!(false)));
    // Saved again, what this version couldn't read is still there.
    let again = b64(&s.execute("document.export", &json!({"format": "vectorcraft"})).unwrap());
    let text = String::from_utf8_lossy(&again).to_string();
    assert!(text.contains("crossStitch") && text.contains("cellSize") && text.contains("tripleSatin"));
    // Choosing a stitch replaces the unknown settings; the newer pattern can be removed.
    s.execute("embroidery.set", &json!({"type": "fill"})).unwrap();
    assert_eq!(info(&mut s)["selection"]["type"], "fill");
    s.execute("embroidery.removeBorderPattern", &json!({"pattern": "lace"})).unwrap();
    assert!(s.execute("embroidery.set", &json!({"type": "patternBorder", "pattern": "lace"})).is_err());
}

#[test]
fn the_design_check_finds_and_places_problems() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 400, "height": 300})).unwrap();
    let wide = s.execute("path.create", &json!({"d": "M 20 50 L 380 50"})).unwrap()["id"].as_u64().unwrap();
    s.execute("embroidery.set", &json!({"type": "satin", "width": 10})).unwrap();
    s.execute("select.none", &json!({})).unwrap();
    let fine = s.execute("path.create", &json!({"d": "M 20 150 L 380 150"})).unwrap()["id"].as_u64().unwrap();
    s.execute("embroidery.set", &json!({"type": "running"})).unwrap();
    let r = s.execute("embroidery.check", &json!({})).unwrap();
    let findings = r["findings"].as_array().unwrap();
    let long = findings.iter().find(|f| f["rule"] == "long-stitch").expect("10 mm satin stitches are long");
    assert_eq!((long["object"].clone(), long["severity"].clone(), long["objectName"].clone()), (json!(wide), json!("warning"), json!("Path")));
    let at = long["at"].as_array().unwrap();
    // On the satin, in document points.
    assert!((at[1].as_f64().unwrap() - 50.0).abs() < 20.0 && at[0].as_f64().unwrap() > 20.0, "{long}");
    assert_eq!(r["counts"]["warning"], findings.len());
    assert!(r["rules"].as_array().unwrap().iter().any(|x| x["id"] == "long-stitch"));
    // Asked about the fine line only: nothing.
    assert!(s.execute("embroidery.check", &json!({"ids": [fine]})).unwrap()["findings"].as_array().unwrap().is_empty());
    // The export says so too.
    let export = s.execute("document.export", &json!({"format": "dst"})).unwrap();
    assert!(
        export["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().starts_with("design check: 1 object: long stitches")),
        "{export}"
    );
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

#[test]
fn a_test_sheet_opens_as_a_new_document_with_its_form() {
    let mut s = Session::new();
    let r = s.execute("embroidery.testSheet", &json!({"sections": ["satinWidth", "fillAngle"]})).unwrap();
    assert_eq!(r["index"], 0, "a new document, made without one open");
    assert_eq!(r["sections"].as_array().unwrap().len(), 2);
    assert_eq!(r["swatches"], 13, "9 satin widths and 4 fill angles");
    assert_eq!(s.doc().unwrap().doc.artboards.len(), 2);
    let form = s.execute("embroidery.sewOut", &json!({})).unwrap();
    let swatches = form["sewOut"]["swatches"].as_array().unwrap();
    assert_eq!(swatches.len(), 13);
    assert_eq!((swatches[0]["id"].as_str(), swatches[0]["result"].as_str()), (Some("A1"), Some("untested")));
    assert_eq!(swatches[9]["id"], "E1", "named by the section's letter");
    assert_eq!(form["sewOut"]["counts"], json!({"pass": 0, "fail": 0, "untested": 13}));
    // The swatch's objects are the document's.
    let id = vectorcraft_doc::NodeId(swatches[0]["objects"][0].as_u64().unwrap());
    assert!(s.doc().unwrap().doc.node(id).is_some_and(|n| n.extras().stitch.is_some()));
    assert!(s.execute("embroidery.testSheet", &json!({"sections": ["nope"]})).is_err());
    assert!(s.execute("embroidery.testSheet", &json!({"sections": "satinWidth"})).is_err());
}

#[test]
fn the_sew_out_form_records_results_as_one_undo_step() {
    let mut s = Session::new();
    s.execute("embroidery.testSheet", &json!({"sections": ["satinWidth"]})).unwrap();
    let r = s
        .execute(
            "embroidery.recordSewOut",
            &json!({"fabric": " denim ", "machine": "home", "results": [
                {"swatch": "A1", "result": "pass"},
                {"swatch": "a9", "result": "fail", "notes": "loops at the edges"},
            ]}),
        )
        .unwrap();
    assert_eq!(r["counts"], json!({"pass": 1, "fail": 1, "untested": 7}));
    let form = s.execute("embroidery.sewOut", &json!({})).unwrap();
    assert_eq!(form["sewOut"]["fabric"], "denim");
    assert_eq!(form["sewOut"]["swatches"][8]["notes"], "loops at the edges");
    // Back to untested; a note alone keeps the result.
    s.execute("embroidery.recordSewOut", &json!({"results": [{"swatch": "A1", "result": "untested"}, {"swatch": "A9", "notes": "ok after all"}]}))
        .unwrap();
    let form = s.execute("embroidery.sewOut", &json!({})).unwrap();
    assert_eq!(form["sewOut"]["swatches"][0]["result"], "untested");
    assert_eq!(form["sewOut"]["swatches"][8]["result"], "fail");
    s.execute("edit.undo", &json!({})).unwrap();
    let form = s.execute("embroidery.sewOut", &json!({})).unwrap();
    assert_eq!(form["sewOut"]["swatches"][0]["result"], "pass", "one undo step");
    // Saved with the document.
    let saved = s.execute("document.export", &json!({"format": "vectorcraft"})).unwrap();
    let back = vectorcraft_format::load(&b64(&saved)).unwrap();
    assert_eq!(back.embroidery.sew_out.as_ref().map(|r| r.machine.as_str()), Some("home"));
    for bad in
        [json!({"results": [{"swatch": "Z9", "result": "pass"}]}), json!({"results": [{"swatch": "A1", "result": "maybe"}]}), json!({"fabric": 3})]
    {
        assert!(s.execute("embroidery.recordSewOut", &bad).is_err(), "{bad}");
    }
    // A document without a test sheet has no form.
    s.execute("file.new", &json!({})).unwrap();
    assert_eq!(s.execute("embroidery.sewOut", &json!({})).unwrap()["sewOut"], Value::Null);
    assert!(s.execute("embroidery.recordSewOut", &json!({"fabric": "x"})).is_err());
}

#[test]
fn dst_export_writes_a_design_per_artboard() {
    let mut s = Session::new();
    s.execute("embroidery.testSheet", &json!({"sections": ["satinWidth", "fillDensity"]})).unwrap();
    let whole = vectorcraft_embroidery::read_dst(&b64(&s.execute("document.export", &json!({"format": "dst"})).unwrap())).unwrap();
    let one = s.execute("document.export", &json!({"format": "dst", "artboard": 1})).unwrap();
    let fills = vectorcraft_embroidery::read_dst(&b64(&one)).unwrap();
    let w = |p: &vectorcraft_embroidery::Plan| p.stats().bounds.map(|b| b[2] - b[0]).unwrap();
    assert!(w(&fills) < 100.0 && w(&whole) > 150.0, "one section's design: {} of {}", w(&fills), w(&whole));
    let both = s.execute("document.export", &json!({"format": "dst", "range": "all"})).unwrap();
    let files = both["files"].as_array().unwrap();
    assert_eq!(files.len(), 2, "{both}");
    let total: usize = files.iter().map(|f| vectorcraft_embroidery::read_dst(&b64(f)).unwrap().stats().stitches).sum();
    // Lines are entered nearest the needle, which starts elsewhere in a section sewn alone: a
    // few lock stitches differ.
    let whole_n = whole.stats().stitches;
    assert!(total.abs_diff(whole_n) * 100 <= whole_n, "the sections together are the whole sheet: {total} vs {whole_n}");
    assert!(s.execute("document.export", &json!({"format": "dst", "artboard": 5})).is_err());
}

#[test]
fn new_stitched_objects_get_the_automatic_underlay() {
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "fill"})).unwrap();
    assert_eq!(info(&mut s)["selection"]["underlay"], "auto");
    // Changing the stitch keeps it; a stitch without underlays reports none.
    s.execute("embroidery.set", &json!({"type": "satinShape"})).unwrap();
    assert_eq!(info(&mut s)["selection"]["underlay"], "auto");
    s.execute("embroidery.set", &json!({"type": "running"})).unwrap();
    assert_eq!(info(&mut s)["selection"]["underlay"], "none");
    // A choice wins, and null goes back to none.
    s.execute("embroidery.set", &json!({"type": "satin", "underlay": "edge"})).unwrap();
    assert_eq!(info(&mut s)["selection"]["underlay"], "edge");
    s.execute("embroidery.set", &json!({"underlay": null})).unwrap();
    assert_eq!(info(&mut s)["selection"]["underlay"], "none");
    assert!(s.execute("embroidery.set", &json!({"type": "running", "underlay": "auto"})).is_err(), "running stitches take no underlay");
}

#[test]
fn the_document_fabric_is_set_edited_saved_and_loaded() {
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "fill"})).unwrap();
    let plain = info(&mut s)["design"]["stitches"].as_u64().unwrap();
    let list = s.execute("embroidery.fabrics", &json!({})).unwrap();
    assert_eq!(list["fabric"], Value::Null);
    assert_eq!(list["starters"].as_array().unwrap().len(), 6);
    // Leather: a more open fill.
    let r = s.execute("embroidery.setFabric", &json!({"fabric": "leather"})).unwrap();
    assert_eq!(r["fabric"]["fillSpacing"], 0.6);
    assert!(info(&mut s)["design"]["stitches"].as_u64().unwrap() < plain);
    // Edited: the given settings on the document's fabric; null clears one.
    let r = s.execute("embroidery.setFabric", &json!({"name": "Saddle", "fillSpacing": 0.4, "push": null})).unwrap();
    assert_eq!((r["fabric"]["name"].as_str(), r["fabric"]["fillSpacing"].as_f64(), r["fabric"].get("push")), (Some("Saddle"), Some(0.4), None));
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.execute("embroidery.fabrics", &json!({})).unwrap()["fabric"]["fillSpacing"], 0.6, "one undo step");
    // Saved and loaded again.
    let text = s.execute("embroidery.saveFabric", &json!({})).unwrap()["data"].as_str().unwrap().to_string();
    assert!(text.contains("\"vectorcraft-fabric\""));
    s.execute("embroidery.setFabric", &json!({"fabric": null})).unwrap();
    assert_eq!(info(&mut s)["design"]["stitches"].as_u64().unwrap(), plain);
    let r = s.execute("embroidery.loadFabric", &json!({"data": text})).unwrap();
    assert_eq!(r["fabric"]["id"], "leather");
    let knit = s.execute("embroidery.saveFabric", &json!({"fabric": "knit"})).unwrap();
    assert!(knit["data"].as_str().unwrap().contains("\"Knit\""));
    for bad in [json!({"fabric": "silk"}), json!({"pull": 9}), json!({"pullPercent": "x"}), json!({"name": ""})] {
        assert!(s.execute("embroidery.setFabric", &bad).is_err(), "{bad}");
    }
    assert!(s.execute("embroidery.loadFabric", &json!({"data": "{}"})).is_err());
    s.execute("embroidery.setFabric", &json!({"fabric": null})).unwrap();
    assert!(s.execute("embroidery.saveFabric", &json!({})).is_err(), "no fabric to save");
}

#[test]
fn the_selection_reports_what_is_sewn_with_the_fabric() {
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "fill"})).unwrap();
    s.execute("embroidery.setFabric", &json!({"fabric": "knit"})).unwrap();
    let sel = info(&mut s)["selection"].clone();
    assert_eq!((sel["spacing"].as_f64(), sel["pull"].as_f64(), sel["push"].as_f64()), (Some(0.45), Some(0.3), Some(0.2)));
    // The object's own setting wins.
    s.execute("embroidery.set", &json!({"spacing": 0.5})).unwrap();
    assert_eq!(info(&mut s)["selection"]["spacing"].as_f64(), Some(0.5));
}

#[test]
fn the_document_hoop_is_set_checked_saved_and_loaded() {
    // A 100 pt (35 mm) fill square.
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "fill"})).unwrap();
    let r = s.execute("embroidery.hoops", &json!({})).unwrap();
    assert_eq!((r["hoop"].clone(), r["fit"].clone(), r["common"].as_array().map(Vec::len)), (Value::Null, Value::Null, Some(6)));
    let r = s.execute("embroidery.setHoop", &json!({"hoop": "4x4"})).unwrap();
    assert_eq!(r["fit"], "fits");
    let check =
        |s: &mut Session| s.execute("embroidery.check", &json!({})).unwrap()["findings"].as_array().unwrap().iter().any(|f| f["rule"] == "hoop");
    assert!(!check(&mut s));
    // A 20 × 50 mm field: the 35 mm square doesn't fit; 40 × 20 only turned.
    let r = s.execute("embroidery.setHoop", &json!({"width": 20, "height": 50, "name": "Tiny"})).unwrap();
    assert_eq!((r["fit"].as_str(), r["hoop"]["name"].as_str()), (Some("tooBig"), Some("Tiny")));
    assert!(check(&mut s), "the checker reports it");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.execute("embroidery.hoops", &json!({})).unwrap()["fit"], "fits", "one undo step");
    // Saved and loaded again.
    let text = s.execute("embroidery.saveHoop", &json!({})).unwrap()["data"].as_str().unwrap().to_string();
    s.execute("embroidery.setHoop", &json!({"hoop": null})).unwrap();
    let r = s.execute("embroidery.loadHoop", &json!({"data": text})).unwrap();
    assert_eq!((r["hoop"]["id"].as_str(), r["hoops"].clone()), (Some("4x4"), json!(["4x4"])));
    let two = s.execute("embroidery.saveHoop", &json!({"hoops": ["5x7", "cap"]})).unwrap()["data"].as_str().unwrap().to_string();
    assert_eq!(s.execute("embroidery.loadHoop", &json!({"data": two, "hoop": "cap"})).unwrap()["hoop"]["height"], 60.0);
    for bad in [json!({"hoop": "9x9"}), json!({"width": 5}), json!({"round": "yes"})] {
        assert!(s.execute("embroidery.setHoop", &bad).is_err(), "{bad}");
    }
}

#[test]
fn optimizing_the_order_groups_threads_and_cuts_travel() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 600, "height": 400})).unwrap();
    // Red, blue, red, blue squares far apart, alternating: by paint order every square is a
    // thread change and a long jump.
    for (k, color) in ["#ff0000", "#0000ff", "#ff0000", "#0000ff"].iter().enumerate() {
        s.execute("select.none", &json!({})).unwrap();
        s.execute("paint.setFill", &json!({"color": color})).unwrap();
        let x = if k % 2 == 0 { 20 + k * 10 } else { 450 - k * 10 };
        s.execute("shape.rectangle", &json!({"x": x, "y": 20 + k * 80, "width": 40, "height": 40})).unwrap();
        s.execute("embroidery.set", &json!({"type": "fill"})).unwrap();
    }
    let before = info(&mut s)["design"]["colorChanges"].as_u64().unwrap();
    let r = s.execute("embroidery.optimizeOrder", &json!({})).unwrap();
    assert_eq!(r["colorChanges"], 1);
    assert!(info(&mut s)["design"]["colorChanges"].as_u64().unwrap() < before);
    assert!(r["travelAfter"].as_f64().unwrap() < r["travelBefore"].as_f64().unwrap(), "{r}");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(info(&mut s)["design"]["colorChanges"].as_u64().unwrap(), before, "one undo step");
    // A start point is set and cleared like any setting.
    s.execute("embroidery.set", &json!({"type": "running", "startAt": [10.0, 20.0]})).unwrap();
    assert_eq!(info(&mut s)["selection"]["startAt"], json!([10.0, 20.0]));
    s.execute("embroidery.set", &json!({"startAt": null})).unwrap();
    assert_eq!(info(&mut s)["selection"]["startAt"], Value::Null);
    assert!(s.execute("embroidery.set", &json!({"startAt": [1.0]})).is_err());
}

#[test]
fn thread_charts_are_loaded_matched_chosen_and_removed() {
    let chart = r##"{"format": "vectorcraft-thread-chart", "version": 1, "name": "Test Threads",
        "threads": [{"code": "R1", "name": "Red", "color": "#e00000"}, {"code": "B1", "name": "Blue", "color": "#0020c0"}]}"##;
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "fill"})).unwrap();
    assert!(s.execute("embroidery.matchThreads", &json!({})).is_err(), "no chart yet");
    let r = s.execute("embroidery.loadThreadChart", &json!({"data": chart})).unwrap();
    assert_eq!((r["id"].as_str(), r["threads"].as_u64()), (Some("test-threads"), Some(2)));
    let list = s.execute("embroidery.threadCharts", &json!({})).unwrap();
    assert_eq!(list["chart"], "test-threads", "the first chart loaded is matched against");
    // The red square's thread is the chart's red.
    let m = s.execute("embroidery.matchThreads", &json!({})).unwrap();
    assert_eq!((m["threads"][0]["code"].as_str(), m["threads"][0]["color"].as_str()), (Some("R1"), Some("#ff0000")));
    assert!(m["threads"][0]["deltaE"].as_f64().unwrap() < 10.0);
    // Saved, chosen off, removed; one undo step each.
    let text = s.execute("embroidery.saveThreadChart", &json!({"chart": "test-threads"})).unwrap()["data"].as_str().unwrap().to_string();
    assert!(text.contains("\"R1\""));
    s.execute("embroidery.setThreadChart", &json!({"chart": null})).unwrap();
    assert_eq!(s.execute("embroidery.threadCharts", &json!({})).unwrap()["chart"], Value::Null);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.execute("embroidery.threadCharts", &json!({})).unwrap()["chart"], "test-threads");
    s.execute("embroidery.removeThreadChart", &json!({"chart": "test-threads"})).unwrap();
    let list = s.execute("embroidery.threadCharts", &json!({})).unwrap();
    assert_eq!((list["chart"].clone(), list["charts"].as_array().map(Vec::len)), (Value::Null, Some(0)));
    for bad in [json!({"chart": "nope"}), json!({"chart": 3})] {
        assert!(s.execute("embroidery.setThreadChart", &bad).is_err(), "{bad}");
    }
    assert!(s.execute("embroidery.loadThreadChart", &json!({"data": "{}"})).is_err());
}

#[test]
fn the_sewing_time_follows_the_machine_speed() {
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "fill"})).unwrap();
    let at = |s: &mut Session| info(s)["design"]["minutes"].as_f64().unwrap();
    let normal = at(&mut s);
    assert!(normal > 0.0);
    s.execute("embroidery.setup", &json!({"speed": 1200})).unwrap();
    let fast = at(&mut s);
    assert!(fast < normal, "{fast} < {normal}");
    assert!(s.execute("embroidery.setup", &json!({"speed": 5})).is_err());
    // Each object's stitches are listed in the sewing order.
    let order = s.execute("embroidery.order", &json!({})).unwrap();
    assert!(order["order"][0]["stitches"].as_u64().unwrap() > 100);
}

#[test]
fn the_worksheet_opens_or_comes_back_as_a_pdf() {
    let mut s = session();
    assert!(s.execute("embroidery.worksheet", &json!({})).is_err(), "nothing stitched");
    s.execute("embroidery.set", &json!({"type": "fill"})).unwrap();
    let pdf = s.execute("embroidery.worksheet", &json!({"format": "pdf", "page": "letter"})).unwrap();
    let bytes = vectorcraft_format::base64_decode(pdf["dataBase64"].as_str().unwrap()).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    let r = s.execute("embroidery.worksheet", &json!({})).unwrap();
    assert_eq!((r["index"].as_u64(), r["threads"].as_u64()), (Some(1), Some(1)), "a new document");
    assert!(s.doc().unwrap().doc.title.ends_with("worksheet"));
    assert!(s.execute("embroidery.worksheet", &json!({"page": "a3"})).is_err());
}

/// The width and height of the stitches the document sews, mm.
fn stitched_size(s: &Session) -> (f64, f64) {
    let plan = vectorcraft_embroidery::plan(&s.doc().unwrap().doc);
    let stitches = plan.needles.iter().filter(|n| n.cmd == vectorcraft_embroidery::Cmd::Stitch);
    let (lo, hi) =
        stitches.fold(((f64::MAX, f64::MAX), (f64::MIN, f64::MIN)), |(lo, hi), n| ((lo.0.min(n.x), lo.1.min(n.y)), (hi.0.max(n.x), hi.1.max(n.y))));
    (hi.0 - lo.0, hi.1 - lo.1)
}

/// The size of the art envelope `id` draws (its content expanded as the canvas evaluates it), mm.
fn drawn_mm(s: &mut Session, id: u64) -> (f64, f64) {
    s.execute("select.set", &json!({"ids": [id]})).unwrap();
    let r = s.execute("object.envelope.expand", &json!({})).unwrap();
    let doc = &s.doc().unwrap().doc;
    let b = doc.node(vectorcraft_doc::NodeId(r["ids"][0].as_u64().unwrap())).unwrap().geometric_bounds().unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    (b.width() * 25.4 / 72.0, b.height() * 25.4 / 72.0)
}

#[test]
fn art_in_an_envelope_is_sewn_as_the_envelope_bends_it() {
    let mut s = session();
    s.execute("embroidery.set", &json!({"type": "fill"})).unwrap();
    let flat = stitched_size(&s);
    assert!((flat.1 - 100.0 * 25.4 / 72.0).abs() < 1.5, "{flat:?}");
    let env = s.execute("object.envelope.makeWithWarp", &json!({"style": "arc", "bend": 50})).unwrap();
    let id = env["id"].as_u64().unwrap();
    let (bent, want) = (stitched_size(&s), drawn_mm(&mut s, id));
    assert!(bent.0 > flat.0 + 5.0, "the arc bends the square: {bent:?} vs {flat:?}");
    assert!((bent.0 - want.0).abs() < 1.5 && (bent.1 - want.1).abs() < 1.5, "sewn as drawn: {bent:?} vs {want:?}");
    // Editing the envelope sews its art again.
    s.execute("select.set", &json!({"ids": [id]})).unwrap();
    s.execute("object.envelope.resetWithWarp", &json!({"style": "bulge", "bend": -50})).unwrap();
    let (again, want) = (stitched_size(&s), drawn_mm(&mut s, id));
    assert!((again.0 - bent.0).abs() > 3.0, "{again:?} vs {bent:?}");
    assert!((again.0 - want.0).abs() < 1.5 && (again.1 - want.1).abs() < 1.5, "{again:?} vs {want:?}");
}

#[test]
fn type_in_an_envelope_or_with_a_warp_effect_is_sewn_bent() {
    let mut s = session();
    s.execute("select.none", &json!({})).unwrap();
    s.execute("text.create", &json!({"x": 10, "y": 300, "text": "HELLO", "size": 48})).unwrap();
    s.execute("embroidery.set", &json!({"type": "fill"})).unwrap();
    let flat = stitched_size(&s);
    if !flat.0.is_finite() || flat.0 <= 0.0 {
        return; // no font to lay the type out in
    }
    // A Warp effect on the type.
    s.execute("effect.apply", &json!({"effect": "warp.arc", "params": {"bend": 50}})).unwrap();
    let warped = stitched_size(&s);
    assert!(warped.1 > flat.1 * 1.2, "{warped:?} vs {flat:?}");
    s.execute("edit.undo", &json!({})).unwrap();
    assert!((stitched_size(&s).1 - flat.1).abs() < 0.01);
    // An envelope round it.
    let env = s.execute("object.envelope.makeWithWarp", &json!({"style": "arc", "bend": 50})).unwrap();
    let (bent, want) = (stitched_size(&s), drawn_mm(&mut s, env["id"].as_u64().unwrap()));
    assert!(bent.1 > flat.1 * 1.2, "{bent:?} vs {flat:?}");
    assert!((bent.0 - want.0).abs() < 1.5 && (bent.1 - want.1).abs() < 1.5, "sewn as drawn: {bent:?} vs {want:?}");
}
