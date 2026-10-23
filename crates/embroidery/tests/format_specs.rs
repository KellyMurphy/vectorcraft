//! The file format specifications in `docs/formats/` stay in step with the code: their JSON
//! Schemas list exactly the fields, enum values and ranges the code writes and reads, their pages
//! mention every field, and their example files read.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use kurbo::Point;
use serde_json::Value;
use vectorcraft_doc::{
    BorderPattern, ChartThread, EmbroiderySetup, Fabric, Hoop, NodeId, PartStitch, PatternPart, SewOut, SewResult, SewSwatch, StitchKind,
    StitchSettings, Underlay,
};
use vectorcraft_embroidery::params::{self, ValueKind};
use vectorcraft_embroidery::{borders, fabrics, hoops, motifs, threads};

fn formats_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/formats")
}

fn read(name: &str) -> String {
    std::fs::read_to_string(formats_dir().join(name)).unwrap_or_else(|e| panic!("docs/formats/{name}: {e}"))
}

fn json(name: &str) -> Value {
    serde_json::from_str(&read(name)).unwrap_or_else(|e| panic!("docs/formats/{name} isn't JSON: {e}"))
}

/// The keys of a JSON object.
fn keys(v: &Value) -> BTreeSet<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

/// The strings of a JSON array.
fn strings(v: &Value) -> BTreeSet<String> {
    v.as_array().unwrap_or_else(|| panic!("not an array: {v}")).iter().map(|s| s.as_str().unwrap().to_string()).collect()
}

/// Every part stitch (a new variant fails to compile here until it is listed, and so checked).
fn part_stitches() -> Vec<PartStitch> {
    let all = [PartStitch::Running, PartStitch::Bean, PartStitch::Satin, PartStitch::Manual];
    for s in all {
        match s {
            PartStitch::Running | PartStitch::Bean | PartStitch::Satin | PartStitch::Manual => {}
        }
    }
    all.to_vec()
}

fn serde_name<T: serde::Serialize>(v: T) -> String {
    serde_json::to_value(v).unwrap().as_str().unwrap().to_string()
}

/// A border pattern with every field set, so its JSON has every key.
fn full_pattern() -> BorderPattern {
    // `extra` and `unknown` hold what a newer version wrote; empty, they are never written.
    let part = PatternPart { stitch: PartStitch::Satin, points: vec![[0.0, 0.0], [4.0, 0.0]], width: Some(1.0), extra: Default::default() };
    BorderPattern {
        id: "full".into(),
        name: "Full".into(),
        author: Some("a".into()),
        license: Some("CC0-1.0".into()),
        source: Some("s".into()),
        description: Some("d".into()),
        width: 6.0,
        length: 4.0,
        repeat: vec![part.clone()],
        corner: vec![part],
        extra: Default::default(),
        unknown: None,
    }
}

/// Stitch settings with every field set away from its default, so its JSON has every key (a new
/// field fails to compile here until it is set, and so checked against the spec).
fn full_stitch() -> StitchSettings {
    StitchSettings {
        kind: StitchKind::Fill,
        length: Some(1.0),
        spacing: Some(1.0),
        angle: 10.0,
        width: Some(1.0),
        thread: Some([1, 2, 3]),
        pull: Some(1.0),
        pull_percent: Some(5.0),
        push: Some(0.5),
        underlay: Underlay::Fill,
        stagger: Some(4),
        motif: Some("waves".into()),
        motif_size: Some(10.0),
        pattern: Some("waves".into()),
        flip: true,
        passes: Some(5),
        thin_under: Some(1.0),
        wide_over: Some(7.0),
        border: Some(2.0),
        rails: true,
        start_at: Some([1.0, 2.0]),
        puff: Some(3.0),
        keep_covered: true,
        trim_before: true,
        trim_after: true,
        extra: Default::default(),
        unknown: None,
    }
}

fn full_setup() -> EmbroiderySetup {
    EmbroiderySetup {
        trim_jumps_over: Some(5.0),
        stitch_moves_under: 2.0,
        overlap: 1.0,
        min_stitch: 0.5,
        auto_start: false,
        speed: 800.0,
        machine_format: "pes".into(),
        order: vec![NodeId(1)],
        patterns: vec![full_pattern()],
        sew_out: Some(Box::new(full_sew_out())),
        fabric: Some(Box::new(full_fabric())),
        hoop: Some(Box::new(full_hoop())),
        charts: vec![full_chart()],
        chart: Some("full".into()),
        extra: Default::default(),
    }
}

/// A thread chart with every field set (a new field fails to compile here until it is set).
fn full_chart() -> vectorcraft_doc::ThreadChart {
    vectorcraft_doc::ThreadChart {
        id: "full".into(),
        name: "Full".into(),
        brand: Some("b".into()),
        weight: Some("40".into()),
        author: Some("a".into()),
        license: Some("CC0-1.0".into()),
        source: Some("s".into()),
        description: Some("d".into()),
        threads: vec![ChartThread { code: "1".into(), name: "One".into(), color: "#102030".into(), extra: Default::default() }],
        extra: Default::default(),
        unknown: None,
    }
}

#[test]
fn vcthreads_schema_page_and_example_match_the_code() {
    let schema = json("vcthreads.schema.json");
    assert_eq!(schema["properties"]["format"]["const"], threads::FILE_FORMAT);
    assert_eq!(schema["properties"]["version"]["maximum"].as_u64(), Some(u64::from(threads::FILE_VERSION)));
    assert_eq!(schema["properties"]["threads"]["maxItems"].as_u64(), Some(threads::MAX_THREADS as u64));
    let file: Value = serde_json::from_str(&threads::write_file(&full_chart()).unwrap()).unwrap();
    assert_eq!(keys(&schema["properties"]), keys(&file), "the file's fields");
    assert_eq!(keys(&schema["$defs"]["thread"]["properties"]), keys(&file["threads"][0]), "a thread's fields");
    let page = read("vcthreads.md");
    for k in keys(&file).iter().chain(keys(&file["threads"][0]).iter()) {
        assert!(page.contains(&format!("`{k}`")), "vcthreads.md describes `{k}`");
    }
    assert!(page.contains("20,000 threads") && page.contains(&format!("{} MB", threads::MAX_FILE_BYTES >> 20)));
    let ex = threads::read_file(read("examples/sample.vcthreads").as_bytes()).unwrap();
    let start = page.find("```json").unwrap() + 7;
    let end = start + page.get(start..).unwrap().find("```").unwrap();
    assert_eq!(threads::read_file(page.get(start..end).unwrap().as_bytes()).unwrap(), ex, "the page's example is the example file");
    // The document's charts have the file's fields, less the header.
    let doc = json("vectorcraft-embroidery.schema.json");
    let chart_keys = keys(&doc["$defs"]["setup"]["properties"]["charts"]["items"]["properties"]);
    assert_eq!(chart_keys, keys(&serde_json::to_value(full_chart()).unwrap()));
}

/// A hoop with every field set (a new field fails to compile here until it is set).
fn full_hoop() -> Hoop {
    Hoop {
        id: "h".into(),
        name: "H".into(),
        width: 100.0,
        height: 120.0,
        round: true,
        machines: Some("m".into()),
        description: Some("d".into()),
        extra: Default::default(),
        unknown: None,
    }
}

#[test]
fn vchoop_schema_page_and_example_match_the_code() {
    let schema = json("vchoop.schema.json");
    assert_eq!(schema["properties"]["format"]["const"], hoops::FILE_FORMAT);
    assert_eq!(schema["properties"]["version"]["maximum"].as_u64(), Some(u64::from(hoops::FILE_VERSION)));
    assert_eq!(schema["properties"]["hoops"]["maxItems"].as_u64(), Some(hoops::MAX_HOOPS as u64));
    let file: Value = serde_json::from_str(&hoops::write_file(&[full_hoop()]).unwrap()).unwrap();
    assert_eq!(keys(&schema["properties"]), keys(&file), "the file's fields");
    assert_eq!(keys(&schema["$defs"]["hoop"]["properties"]), keys(&file["hoops"][0]), "a hoop's fields");
    let page = read("vchoop.md");
    for k in keys(&file).iter().chain(keys(&file["hoops"][0]).iter()) {
        assert!(page.contains(&format!("`{k}`")), "vchoop.md describes `{k}`");
    }
    for id in hoops::BUILTIN {
        assert!(page.contains(&format!("`{id}`")), "vchoop.md lists `{id}`");
    }
    assert!(page.contains("1,000 hoops") && page.contains(&format!("{} MB", hoops::MAX_FILE_BYTES >> 20)));
    let ex = hoops::read_file(read("examples/frames.vchoop").as_bytes()).unwrap();
    let start = page.find("```json").unwrap() + 7;
    let end = start + page.get(start..).unwrap().find("```").unwrap();
    assert_eq!(hoops::read_file(page.get(start..end).unwrap().as_bytes()).unwrap(), ex, "the page's example is the example file");
}

/// A fabric profile with every field set (a new field fails to compile here until it is set).
fn full_fabric() -> Fabric {
    Fabric {
        id: "full".into(),
        name: "Full".into(),
        author: Some("a".into()),
        license: Some("CC0-1.0".into()),
        source: Some("s".into()),
        description: Some("d".into()),
        satin_spacing: Some(0.4),
        fill_spacing: Some(0.45),
        pull: Some(0.3),
        pull_percent: Some(5.0),
        push: Some(0.2),
        satin_center_under: Some(1.5),
        satin_zigzag_over: Some(4.0),
        shape_edge_over: Some(2.0),
        min_stitch: Some(0.4),
        extra: Default::default(),
        unknown: None,
    }
}

#[test]
fn vcfabric_schema_matches_the_code() {
    let schema = json("vcfabric.schema.json");
    let props = &schema["properties"];
    assert_eq!(props["format"]["const"], fabrics::FILE_FORMAT);
    assert_eq!(props["version"]["maximum"].as_u64(), Some(u64::from(fabrics::FILE_VERSION)));
    let file: Value = serde_json::from_str(&fabrics::write_file(&full_fabric()).unwrap()).unwrap();
    assert_eq!(keys(props), keys(&file), "the schema's fields are the file's");
    range_is(&props["satinSpacing"], StitchSettings::SPACING, "satinSpacing");
    range_is(&props["fillSpacing"], StitchSettings::SPACING, "fillSpacing");
    range_is(&props["pull"], StitchSettings::PULL, "pull");
    range_is(&props["pullPercent"], StitchSettings::PULL_PERCENT, "pullPercent");
    range_is(&props["push"], StitchSettings::PUSH, "push");
    for k in ["satinCenterUnder", "satinZigzagOver", "shapeEdgeOver"] {
        range_is(&props[k], Fabric::THRESHOLD, k);
    }
    // The document's fabric has the file's fields, less the header.
    let doc = json("vectorcraft-embroidery.schema.json");
    assert_eq!(keys(&doc["$defs"]["fabric"]["properties"]), keys(&serde_json::to_value(full_fabric()).unwrap()));
}

#[test]
fn vcfabric_page_describes_every_field_and_starter() {
    let page = read("vcfabric.md");
    let file: Value = serde_json::from_str(&fabrics::write_file(&full_fabric()).unwrap()).unwrap();
    for k in keys(&file) {
        assert!(page.contains(&format!("`{k}`")), "vcfabric.md describes `{k}`");
    }
    for id in fabrics::BUILTIN {
        assert!(page.contains(&format!("`{id}`")), "vcfabric.md lists the starter `{id}`");
    }
    assert!(page.contains(&format!("{} MB", fabrics::MAX_FILE_BYTES >> 20)), "the size limit");
    // The example file reads, and the page's example is it, credits aside.
    let ex = fabrics::read_file(read("examples/polo.vcfabric").as_bytes()).unwrap();
    let start = page.find("```json").unwrap() + 7;
    let end = start + page.get(start..).unwrap().find("```").unwrap();
    let shown = fabrics::read_file(page.get(start..end).unwrap().as_bytes()).unwrap();
    assert_eq!(Fabric { author: None, license: None, description: None, ..shown }, Fabric { author: None, license: None, description: None, ..ex });
}

/// A sew-out record with every field set.
fn full_sew_out() -> SewOut {
    let swatch = SewSwatch {
        id: "A1".into(),
        section: "satinWidth".into(),
        label: "Satin 1 mm wide".into(),
        objects: vec![NodeId(2)],
        result: "pass".into(),
        notes: "n".into(),
        extra: Default::default(),
    };
    SewOut { fabric: "f".into(), machine: "m".into(), notes: "n".into(), swatches: vec![swatch], extra: Default::default() }
}

/// `range` matches the schema property's `minimum` and `maximum`.
fn range_is(prop: &Value, range: std::ops::RangeInclusive<f64>, what: &str) {
    assert_eq!(prop["minimum"].as_f64(), Some(*range.start()), "{what} minimum");
    assert_eq!(prop["maximum"].as_f64(), Some(*range.end()), "{what} maximum");
}

#[test]
fn vcborder_schema_matches_the_code() {
    let schema = json("vcborder.schema.json");
    let props = &schema["properties"];
    assert_eq!(props["format"]["const"], borders::FILE_FORMAT);
    assert_eq!(props["version"]["maximum"].as_u64(), Some(u64::from(borders::FILE_VERSION)));
    // Every key a file has, and no other.
    let file: Value = serde_json::from_str(&borders::write_file(&full_pattern()).unwrap()).unwrap();
    assert_eq!(keys(props), keys(&file), "the schema's fields are the file's");
    let part = &schema["$defs"]["part"]["properties"];
    assert_eq!(keys(part), keys(&file["repeat"][0]), "a part's fields");
    let stitches: BTreeSet<String> = part_stitches().into_iter().map(serde_name).collect();
    assert_eq!(strings(&part["stitch"]["enum"]), stitches);
    // Required: what a file can't be read without.
    for field in strings(&schema["required"]) {
        if matches!(field.as_str(), "format" | "version") {
            continue;
        }
        let mut v = file.clone();
        v.as_object_mut().unwrap().remove(&field);
        assert!(borders::read_file(v.to_string().as_bytes()).is_err(), "a file without `{field}` is refused");
    }
}

#[test]
fn vcborder_page_describes_every_field_and_the_limits() {
    let page = read("vcborder.md");
    let file: Value = serde_json::from_str(&borders::write_file(&full_pattern()).unwrap()).unwrap();
    for k in keys(&file).iter().chain(keys(&file["repeat"][0]).iter()) {
        assert!(page.contains(&format!("`{k}`")), "vcborder.md describes `{k}`");
    }
    for s in part_stitches() {
        assert!(page.contains(&format!("`\"{}\"`", serde_name(s))), "vcborder.md describes the {s:?} stitch");
    }
    for id in borders::BUILTIN {
        assert!(page.contains(&format!("`{id}`")), "vcborder.md lists the starter `{id}`");
    }
    assert_eq!(borders::MAX_PATTERN_POINTS, 100_000, "the point limit vcborder.md gives");
    assert!(page.contains("100,000 points"), "the point limit");
    assert!(page.contains(&format!("{} MB", borders::MAX_FILE_BYTES >> 20)), "the size limit");
}

#[test]
fn the_example_border_pattern_reads_and_sews() {
    let p = borders::read_file(read("examples/rope.vcborder").as_bytes()).unwrap();
    assert_eq!((p.id.as_str(), p.corner.is_empty()), ("rope", false));
    let square = [Point::new(0.0, 0.0), Point::new(60.0, 0.0), Point::new(60.0, 40.0), Point::new(0.0, 40.0), Point::new(0.0, 0.0)];
    let runs = borders::sew(&p, &square, true, 6.0, false, 1.5, 0.4).unwrap();
    assert_eq!(runs.len(), 1, "one run through the corner motifs");
    // The example on the page is the example file's pattern, credits aside.
    let page = read("vcborder.md");
    let start = page.find("```json").unwrap() + 7;
    let end = start + page.get(start..).unwrap().find("```").unwrap();
    let shown = borders::read_file(page.get(start..end).unwrap().as_bytes()).unwrap();
    assert_eq!((shown.width, shown.length, &shown.repeat, &shown.corner), (p.width, p.length, &p.repeat, &p.corner));
}

#[test]
fn embroidery_schema_matches_the_code() {
    let schema = json("vectorcraft-embroidery.schema.json");
    let defs = &schema["$defs"];
    let stitch = &defs["stitch"]["properties"];
    assert_eq!(keys(stitch), keys(&serde_json::to_value(full_stitch()).unwrap()), "the stitch settings' fields");
    let kinds: BTreeSet<String> = StitchKind::ALL.iter().map(|k| k.id().to_string()).collect();
    assert_eq!(strings(&stitch["kind"]["enum"]), kinds);
    assert!(StitchKind::ALL.iter().all(|k| serde_name(*k) == k.id()), "kinds are written by id");
    let underlays: BTreeSet<String> = Underlay::ALL.iter().map(|u| u.id().to_string()).collect();
    assert_eq!(strings(&stitch["underlay"]["enum"]), underlays);
    assert_eq!(strings(&stitch["motif"]["enum"]), motifs::MOTIFS.iter().map(|m| m.to_string()).collect());
    // The settings table covers every field a document holds (`type` is the document's `kind`),
    // and its ranges and choices are the schema's.
    let table: BTreeSet<String> = params::PARAMS.iter().map(|p| if p.key == "type" { "kind".to_string() } else { p.key.to_string() }).collect();
    assert_eq!(table, keys(stitch), "the settings table (params::PARAMS) and the schema list the same settings");
    for p in params::PARAMS {
        let prop = &stitch[if p.key == "type" { "kind" } else { p.key }];
        match &p.value {
            ValueKind::Mm(r) | ValueKind::Percent(r) => range_is(prop, r.clone(), p.key),
            ValueKind::Whole { range, odd: false } => range_is(prop, f64::from(*range.start())..=f64::from(*range.end()), p.key),
            ValueKind::Whole { range, odd: true } => {
                let odd: Vec<u64> = range.clone().filter(|n| n % 2 == 1).map(u64::from).collect();
                assert_eq!(prop["enum"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect::<Vec<_>>(), odd, "{}", p.key);
            }
            ValueKind::Choice(ids) => assert_eq!(strings(&prop["enum"]), ids.iter().map(|s| s.to_string()).collect(), "{}", p.key),
            ValueKind::Flag => assert_eq!(prop["type"], "boolean", "{}", p.key),
            ValueKind::Kind | ValueKind::Degrees | ValueKind::Id | ValueKind::Thread | ValueKind::Point => {}
        }
    }
    // The fixed lists the table points at are the code's own.
    assert_eq!(strings(&stitch["motif"]["enum"]), motifs::MOTIFS.iter().map(|m| m.to_string()).collect());

    let setup = &defs["setup"]["properties"];
    assert_eq!(keys(setup), keys(&serde_json::to_value(full_setup()).unwrap()), "the setup's fields");
    range_is(&setup["trimJumpsOver"], EmbroiderySetup::TRIM, "trimJumpsOver");
    range_is(&setup["stitchMovesUnder"], EmbroiderySetup::STITCH_MOVES, "stitchMovesUnder");
    assert_eq!(setup["trimJumpsOver"]["default"].as_f64(), Some(EmbroiderySetup::DEFAULT_TRIM));
    assert_eq!(setup["stitchMovesUnder"]["default"].as_f64(), Some(EmbroiderySetup::DEFAULT_STITCH_MOVES));
    let sew_out = serde_json::to_value(full_sew_out()).unwrap();
    assert_eq!(keys(&defs["sewOut"]["properties"]), keys(&sew_out), "the sew-out's fields");
    let swatch = &defs["swatch"]["properties"];
    assert_eq!(keys(swatch), keys(&sew_out["swatches"][0]), "a swatch's fields");
    let results: BTreeSet<String> = SewResult::ALL.iter().map(|r| r.id().to_string()).collect();
    assert_eq!(strings(&swatch["result"]["enum"]), results);
    // A document's patterns have a border pattern file's fields, less the header.
    assert_eq!(keys(&defs["pattern"]["properties"]), keys(&serde_json::to_value(full_pattern()).unwrap()));
}

#[test]
fn embroidery_page_describes_every_field_and_kind() {
    let page = read("vectorcraft-embroidery.md");
    let sew_out = serde_json::to_value(full_sew_out()).unwrap();
    let fields = keys(&serde_json::to_value(full_stitch()).unwrap())
        .into_iter()
        .chain(keys(&serde_json::to_value(full_setup()).unwrap()))
        .chain(keys(&sew_out))
        .chain(keys(&sew_out["swatches"][0]));
    for k in fields {
        assert!(page.contains(&format!("`{k}`")), "vectorcraft-embroidery.md describes `{k}`");
    }
    for k in StitchKind::ALL {
        assert!(page.contains(&format!("| `{}` |", k.id())), "vectorcraft-embroidery.md describes the {} kind", k.id());
    }
    for u in Underlay::ALL {
        assert!(page.contains(&format!("`{}`", u.id())), "vectorcraft-embroidery.md lists the {} underlay", u.id());
    }
    for r in SewResult::ALL.iter().filter(|r| !r.id().is_empty()) {
        assert!(page.contains(&format!("`{}`", r.id())), "vectorcraft-embroidery.md lists the {} result", r.id());
    }
    for s in vectorcraft_embroidery::testsheet::SECTIONS {
        assert!(page.contains(&format!("`{}`", s.id)), "vectorcraft-embroidery.md lists the {} section", s.id);
    }
}

#[test]
fn the_formats_index_lists_every_spec() {
    let index = read("README.md");
    for entry in std::fs::read_dir(formats_dir()).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().to_string();
        if name != "README.md" && (name.ends_with(".md") || name.ends_with(".json")) {
            assert!(index.contains(&format!("]({name})")), "docs/formats/README.md links {name}");
        }
    }
}
