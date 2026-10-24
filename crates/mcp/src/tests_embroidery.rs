//! Embroidery over MCP: the whole workflow through the server, and the rules that keep every
//! embroidery feature reachable by agents as it lands (a command with a menu path that the guide
//! documents; a stitch type the prompt, `embroidery.set` and the guide know; a setting both
//! `embroidery.set` and `embroidery.info` describe; a file format served as a resource).

use std::collections::BTreeSet;

use serde_json::{Value, json};
use vectorcraft_doc::StitchKind;
use vectorcraft_engine::cmd::{CommandSpec, command_specs};

use crate::resources::TEXT_DOCS;
use crate::{Headless, Server};

fn rpc(s: &mut Server, method: &str, params: Value) -> Value {
    s.handle(json!({"jsonrpc":"2.0","id":1,"method":method,"params":params})).unwrap()
}

fn call(s: &mut Server, name: &str, arguments: Value) -> Value {
    let r = rpc(s, "tools/call", json!({"name":name,"arguments":arguments}));
    assert!(r.get("error").is_none(), "{r}");
    assert_eq!(r["result"]["isError"], false, "{name}: {r}");
    r["result"].clone()
}

fn payload(r: &Value) -> Value {
    serde_json::from_str(r["content"][0]["text"].as_str().unwrap()).unwrap()
}

fn run(s: &mut Server, command: &str, params: Value) -> Value {
    payload(&call(s, "run_command", json!({"command": command, "params": params})))
}

fn png(r: &Value) -> image::RgbaImage {
    let bytes = vectorcraft_format::base64_decode(r["content"][0]["data"].as_str().unwrap()).unwrap();
    image::load_from_memory(&bytes).unwrap().to_rgba8()
}

fn guide() -> &'static str {
    TEXT_DOCS.iter().find(|d| d.uri == "vectorcraft://guide/embroidery").unwrap().text
}

fn spec(id: &str) -> &'static CommandSpec {
    command_specs().iter().find(|c| c.id == id).unwrap_or_else(|| panic!("no command {id}"))
}

#[test]
fn an_agent_digitizes_previews_and_exports() {
    let mut s = Server::new(Box::new(Headless::with_document()));
    // The server tells agents about embroidery, and where the guide is.
    let init = rpc(&mut s, "initialize", json!({"protocolVersion":"2025-06-18"}));
    let instructions = init["result"]["instructions"].as_str().unwrap();
    assert!(instructions.contains("embroidery.set") && instructions.contains("vectorcraft://guide/embroidery"), "{instructions}");

    call(&mut s, "draw_shape", json!({"shape": "rectangle", "x": 100, "y": 100, "width": 200, "height": 120, "fill": "#1f6f8b"}));
    run(&mut s, "embroidery.set", json!({"type": "fill", "border": 2}));
    let info = run(&mut s, "embroidery.info", json!({}));
    assert!(info["design"]["stitches"].as_u64().unwrap() > 1000, "{info}");
    assert_eq!(info["selection"]["type"], "fill");

    // render_preview draws the stitches on request, and leaves the document alone.
    let before = call(&mut s, "doc_inspect", json!({}));
    let art = png(&call(&mut s, "render_preview", json!({"max_side": 300})));
    let sewn = png(&call(&mut s, "render_preview", json!({"max_side": 300, "stitches": true})));
    assert_eq!(art.dimensions(), sewn.dimensions());
    // Outside the art, the preview shows fabric where the plain render shows white paper.
    let corner = |img: &image::RgbaImage| *img.get_pixel(3, 3);
    assert_eq!(corner(&art).0[..3], [255, 255, 255]);
    let [r, g, b, _] = corner(&sewn).0;
    assert!(r > g && g > b && b < 240, "fabric, not paper: {r} {g} {b}");
    assert_eq!(before, call(&mut s, "doc_inspect", json!({})));
    assert_eq!(rpc(&mut s, "tools/call", json!({"name": "render_preview", "arguments": {"stitches": "yes"}}))["result"]["isError"], true);

    // Exported as DST through the export tool.
    let out = payload(&call(&mut s, "export", json!({"format": "dst"})));
    let dst = vectorcraft_format::base64_decode(out["dataBase64"].as_str().unwrap()).unwrap();
    assert!(dst.starts_with(b"LA:"));
    // And as PES, which keeps the thread colour, opened again.
    run(&mut s, "embroidery.setup", json!({"machineFormat": "pes"}));
    let out = payload(&call(&mut s, "export", json!({"format": "pes"})));
    let pes = out["dataBase64"].as_str().unwrap().to_string();
    assert!(vectorcraft_format::base64_decode(&pes).unwrap().starts_with(b"#PES0060"));
    let thread = info["design"]["threads"].clone();
    run(&mut s, "document.open", json!({"name": "design.pes", "dataBase64": pes}));
    let back = run(&mut s, "embroidery.info", json!({}));
    assert_eq!(back["design"]["threads"], thread);
    assert_eq!(back["design"]["stitches"], info["design"]["stitches"]);

    // The prompt walks through it; the guide and the specs are readable.
    let p = rpc(&mut s, "prompts/get", json!({"name": "embroidery", "arguments": {"stitch": "satin", "path": "out.dst"}}));
    let text = p["result"]["messages"][0]["content"]["text"].as_str().unwrap();
    assert!(text.contains("embroidery.set {type: satin}") && text.contains("out.dst") && text.contains("render_preview {stitches: true}"), "{text}");
    let g = rpc(&mut s, "resources/read", json!({"uri": "vectorcraft://guide/embroidery"}));
    assert_eq!(g["result"]["contents"][0]["mimeType"], "text/markdown");
    assert!(g["result"]["contents"][0]["text"].as_str().unwrap().starts_with("# Embroidery"));
}

/// Every embroidery command is listed where agents look for it (a menu path, its parameters
/// described) and documented in the guide served to them.
#[test]
fn every_embroidery_command_is_discoverable() {
    let commands: Vec<&CommandSpec> = command_specs().iter().filter(|c| c.id.starts_with("embroidery.")).collect();
    assert!(commands.len() >= 15, "{}", commands.len());
    for c in commands {
        assert!(!c.menu.is_empty(), "{} needs a menu path (Object › Embroidery, Window › Embroidery…)", c.id);
        assert!(c.params.contains('{'), "{} needs a parameter description", c.id);
        assert!(guide().contains(&format!("`{}`", c.id)), "docs/embroidery.md must document `{}` (it is the guide agents read)", c.id);
    }
}

/// Every stitch type is known to the prompt, described by `embroidery.set` and in the guide.
#[test]
fn every_stitch_type_reaches_agents() {
    let set = spec("embroidery.set").params;
    for k in StitchKind::ALL {
        assert!(crate::prompts::STITCH_TYPES.contains(&k.id()), "the embroidery prompt's stitch list lacks {}", k.id());
        assert!(set.contains(k.id()), "embroidery.set's description lacks the {} type", k.id());
        assert!(guide().contains(k.label()), "the guide lacks the {} stitch", k.label());
    }
    assert_eq!(crate::prompts::STITCH_TYPES.len(), StitchKind::ALL.len(), "the prompt lists a type the engine doesn't have");
}

/// Every machine format reaches agents: the prompt's list, `embroidery.setup`'s description, the
/// formats `export` writes, and the guide.
#[test]
fn every_machine_format_reaches_agents() {
    let setup = spec("embroidery.setup").params;
    assert_eq!(crate::prompts::MACHINE_FORMATS, vectorcraft_doc::EmbroiderySetup::MACHINE_FORMATS);
    for f in vectorcraft_doc::EmbroiderySetup::MACHINE_FORMATS {
        assert!(setup.contains(f), "embroidery.setup's description lacks {f}");
        assert!(vectorcraft_engine::cmd::fileio::format(f).is_some_and(|x| x.read && x.write), "{f} isn't a readable, writable format");
        assert!(guide().contains(&format!("| {} (", f.to_uppercase())), "the guide lacks {f}");
    }
}

/// Every stitch type's row in the guide's table names the settings it uses (from the settings
/// table), so agents reading the guide know what each type takes.
#[test]
fn every_stitch_types_guide_row_names_its_settings() {
    let always = ["type", "thread", "trimBefore", "trimAfter", "stopBefore"];
    for k in StitchKind::ALL {
        let row = guide()
            .lines()
            .find(|l| l.starts_with(&format!("| {} |", k.label())))
            .unwrap_or_else(|| panic!("the guide's table has no row for {}", k.label()));
        for p in vectorcraft_embroidery::params::PARAMS.iter().filter(|p| p.applies(k) && !always.contains(&p.key)) {
            assert!(row.contains(&format!("`{}`", p.key)), "the guide's {} row doesn't mention `{}`", k.label(), p.key);
        }
    }
}

/// Every stitch setting a document can hold (the schema, itself checked against the code) is
/// described by the command that sets it and the one that reports it.
#[test]
fn every_stitch_setting_reaches_agents() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/formats/vectorcraft-embroidery.schema.json");
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let keys: BTreeSet<String> = schema["$defs"]["stitch"]["properties"].as_object().unwrap().keys().cloned().collect();
    let (set, info) = (spec("embroidery.set").params, spec("embroidery.info").params);
    for k in keys {
        // Documents say `kind`; the commands say `type`.
        let k = if k == "kind" { "type".to_string() } else { k };
        assert!(set.contains(&format!("{k}?")), "embroidery.set's description lacks `{k}`");
        assert!(info.contains(&k), "embroidery.info's description lacks `{k}`");
    }
}

/// Every file format specification is served as a resource, so agents can read the formats they
/// write; the resource is the file as it is in the repository.
#[test]
fn every_format_spec_is_a_resource() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/formats");
    let mut found = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !(name.ends_with(".md") || name.ends_with(".json")) {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
        assert!(
            TEXT_DOCS.iter().any(|d| d.text.replace("\r\n", "\n") == text),
            "docs/formats/{name} isn't served: add it to resources::TEXT_DOCS (vectorcraft://spec/…)"
        );
        found += 1;
    }
    assert!(found >= 5);
}
