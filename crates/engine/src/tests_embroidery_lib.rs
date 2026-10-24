//! Embroidery library folders (E8.5): listing, saving into, adding to and loading from the
//! embroidery library folder's border patterns, fabrics, hoops and thread charts.

use serde_json::{Value, json};
use vectorcraft_doc::{ChartThread, ThreadChart};

use super::*;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
    s
}

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

/// A fresh scratch folder for one test.
fn temp_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("vc-embroidery-lib-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn with_folder(name: &str) -> (Session, std::path::PathBuf) {
    let dir = temp_dir(name);
    let mut s = session();
    s.embroidery_libraries.set_dir(Some(dir.to_string_lossy().to_string()));
    (s, dir)
}

fn entries(s: &mut Session, kind: &str) -> Vec<Value> {
    run(s, "embroidery.libraries", json!({ "kind": kind }))["libraries"].as_array().unwrap().clone()
}

fn chart_file(id: &str) -> String {
    let chart = ThreadChart {
        id: id.into(),
        name: "Mine".into(),
        threads: vec![ChartThread { code: "1".into(), color: "#ff0000".into(), ..Default::default() }],
        ..Default::default()
    };
    vectorcraft_embroidery::threads::write_file(&chart).unwrap()
}

#[test]
fn without_a_folder_nothing_is_listed_and_nothing_written() {
    let mut s = session();
    let r = run(&mut s, "embroidery.libraries", json!({}));
    assert_eq!(r, json!({"folder": null, "libraries": [], "skipped": []}));
    assert!(s.execute("embroidery.addToLibrary", &json!({"data": chart_file("a"), "name": "A.vcthreads"})).is_err());
    assert!(s.execute("embroidery.saveFabric", &json!({"fabric": "woven", "user": true})).is_err());
    assert!(s.execute("embroidery.libraries", &json!({"kind": "motifs"})).is_err(), "not a kind yet");
}

#[test]
fn saved_entries_are_listed_and_load_into_the_document() {
    let (mut s, dir) = with_folder("save");
    let r = run(&mut s, "embroidery.saveBorderPattern", json!({"pattern": "scallops", "user": true}));
    assert_eq!(r["library"], "user/Scallops.vcborder");
    assert!(std::path::Path::new(r["path"].as_str().unwrap()).starts_with(dir.join("borders")));
    run(&mut s, "embroidery.saveFabric", json!({"fabric": "knit", "user": true}));
    run(&mut s, "embroidery.saveHoop", json!({"hoops": ["5x7", "8x8"], "user": true, "name": "My hoops"}));
    run(&mut s, "embroidery.addToLibrary", json!({"data": chart_file("mine"), "name": "Mine.vcthreads"}));

    let all = run(&mut s, "embroidery.libraries", json!({}));
    let kinds: Vec<&str> = all["libraries"].as_array().unwrap().iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["borders", "fabrics", "hoops", "threads"]);
    assert_eq!(entries(&mut s, "borders")[0]["pattern"], "scallops");
    assert_eq!(entries(&mut s, "hoops")[0]["hoops"], json!(["5x7", "8x8"]));
    assert_eq!(entries(&mut s, "threads")[0]["threads"], 1);

    // Loaded by id or by name, each into the document.
    let r = run(&mut s, "embroidery.loadFabric", json!({"library": "user/Knit.vcfabric"}));
    assert_eq!(r["fabric"]["id"], "knit");
    let r = run(&mut s, "embroidery.loadHoop", json!({"library": "my hoops", "hoop": "8x8"}));
    assert_eq!(r["hoops"], json!(["5x7", "8x8"]));
    assert_eq!(s.doc().unwrap().doc.embroidery.hoop.as_ref().map(|h| h.id.as_str()), Some("8x8"));
    let r = run(&mut s, "embroidery.loadThreadChart", json!({"library": "Mine"}));
    assert_eq!(r["id"], "mine");
    // The starter scallops is the same pattern, so the document doesn't take a copy.
    let r = run(&mut s, "embroidery.loadBorderPattern", json!({"library": "user/Scallops.vcborder"}));
    assert_eq!((r["id"].as_str(), r["added"].as_bool()), (Some("scallops"), Some(false)));
    assert!(s.execute("embroidery.loadFabric", &json!({"library": "nope"})).is_err());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn adding_never_replaces_and_bad_files_are_reported() {
    let (mut s, dir) = with_folder("add");
    let r = run(&mut s, "embroidery.addToLibrary", json!({"data": chart_file("one"), "name": "Threads.vcthreads"}));
    assert_eq!((r["id"].as_str(), r["copied"].as_bool()), (Some("user/Threads.vcthreads"), Some(true)));
    // The same file again: nothing written.
    let r = run(&mut s, "embroidery.addToLibrary", json!({"data": chart_file("one"), "name": "Threads.vcthreads"}));
    assert_eq!((r["id"].as_str(), r["copied"].as_bool()), (Some("user/Threads.vcthreads"), Some(false)));
    // A different file under the same name gets a number.
    let r = run(&mut s, "embroidery.addToLibrary", json!({"data": chart_file("two"), "name": "Threads.vcthreads"}));
    assert_eq!(r["id"], "user/Threads 2.vcthreads");
    // The kind given, the file is stored under its extension.
    let r = run(&mut s, "embroidery.addToLibrary", json!({"data": chart_file("three"), "name": "x.json", "kind": "threads"}));
    assert_eq!(r["id"], "user/x.vcthreads");
    // Files that don't read are refused, and ones dropped in the folder are listed as skipped.
    assert!(s.execute("embroidery.addToLibrary", &json!({"data": "{}", "name": "Bad.vcthreads"})).is_err());
    assert!(s.execute("embroidery.addToLibrary", &json!({"data": chart_file("x"), "name": "x.txt"})).is_err());
    std::fs::create_dir_all(dir.join("fabrics")).unwrap();
    std::fs::write(dir.join("fabrics").join("Broken.vcfabric"), "not json").unwrap();
    std::fs::write(dir.join("threads").join("Dropped.vcthreads"), chart_file("dropped")).unwrap();
    let r = run(&mut s, "embroidery.rescanLibraries", json!({}));
    assert_eq!(r["skipped"].as_array().unwrap().len(), 1);
    assert_eq!(r["skipped"][0]["kind"], "fabrics");
    assert_eq!(entries(&mut s, "threads").len(), 4);
    let _ = std::fs::remove_dir_all(dir);
}
