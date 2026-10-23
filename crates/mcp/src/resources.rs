//! MCP resources: the fixed documents, the templates an agent can address parts of the
//! document by, and reading either.
//!
//! The templates matter for long sessions. `vectorcraft://document/json` returns the whole model,
//! which on a real document is thousands of lines an agent pays for again on every change; the
//! templates hand out one object, one command definition or one effect's parameters instead, and
//! they are what `completion/complete` suggests ids and names from.

use serde_json::{Value, json};

use crate::backend::Backend;
use crate::prompts::Live;

/// The active document's summary: artboards, layer tree, selection, history.
pub const DOC_URI: &str = "vectorcraft://document";
/// The whole document model as JSON.
pub const DOC_JSON_URI: &str = "vectorcraft://document/json";

/// The live command catalog, including enabled state.
pub const COMMANDS_URI: &str = "vectorcraft://commands";

/// A document served as it is written in `docs/` (guides and file format specifications).
pub struct TextDoc {
    pub uri: &'static str,
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub mime: &'static str,
    pub text: &'static str,
}

/// Every guide and specification, in `resources/list` order. Each file in `docs/formats/` is
/// listed here (tested), so a new file format is readable by agents from the day it lands.
pub static TEXT_DOCS: &[TextDoc] = &[
    TextDoc {
        uri: "vectorcraft://guide/embroidery",
        name: "guide-embroidery",
        title: "Embroidery guide",
        description: "How VectorCraft digitizes art for machine embroidery: stitch types and their settings (lengths in mm), type, sew order, travel, border patterns, machine files (DST, PES, JEF, EXP) and every embroidery command",
        mime: "text/markdown",
        text: include_str!("../../../docs/embroidery.md"),
    },
    TextDoc {
        uri: "vectorcraft://spec/formats",
        name: "spec-formats",
        title: "File format specifications (index)",
        description: "VectorCraft's own open file formats and what they have in common",
        mime: "text/markdown",
        text: include_str!("../../../docs/formats/README.md"),
    },
    TextDoc {
        uri: "vectorcraft://spec/vcborder",
        name: "spec-vcborder",
        title: "Border pattern file (.vcborder) specification",
        description: "The open JSON format for embroidery border patterns: fields, the pattern's space, parts, corners and limits",
        mime: "text/markdown",
        text: include_str!("../../../docs/formats/vcborder.md"),
    },
    TextDoc {
        uri: "vectorcraft://spec/vcborder.schema.json",
        name: "spec-vcborder-schema",
        title: "Border pattern file JSON Schema",
        description: "JSON Schema (draft 2020-12) for .vcborder files",
        mime: "application/schema+json",
        text: include_str!("../../../docs/formats/vcborder.schema.json"),
    },
    TextDoc {
        uri: "vectorcraft://spec/vcthreads",
        name: "spec-vcthreads",
        title: "Thread chart file (.vcthreads) specification",
        description: "The open JSON format for embroidery thread charts (code, name, colour) and how colours are matched to threads",
        mime: "text/markdown",
        text: include_str!("../../../docs/formats/vcthreads.md"),
    },
    TextDoc {
        uri: "vectorcraft://spec/vcthreads.schema.json",
        name: "spec-vcthreads-schema",
        title: "Thread chart file JSON Schema",
        description: "JSON Schema (draft 2020-12) for .vcthreads files",
        mime: "application/schema+json",
        text: include_str!("../../../docs/formats/vcthreads.schema.json"),
    },
    TextDoc {
        uri: "vectorcraft://spec/vchoop",
        name: "spec-vchoop",
        title: "Hoop list file (.vchoop) specification",
        description: "The open JSON format for embroidery hoops: sewing fields and how a design fits",
        mime: "text/markdown",
        text: include_str!("../../../docs/formats/vchoop.md"),
    },
    TextDoc {
        uri: "vectorcraft://spec/vchoop.schema.json",
        name: "spec-vchoop-schema",
        title: "Hoop list file JSON Schema",
        description: "JSON Schema (draft 2020-12) for .vchoop files",
        mime: "application/schema+json",
        text: include_str!("../../../docs/formats/vchoop.schema.json"),
    },
    TextDoc {
        uri: "vectorcraft://spec/vcfabric",
        name: "spec-vcfabric",
        title: "Fabric profile file (.vcfabric) specification",
        description: "The open JSON format for embroidery fabric profiles: density, compensation and automatic underlay thresholds",
        mime: "text/markdown",
        text: include_str!("../../../docs/formats/vcfabric.md"),
    },
    TextDoc {
        uri: "vectorcraft://spec/vcfabric.schema.json",
        name: "spec-vcfabric-schema",
        title: "Fabric profile file JSON Schema",
        description: "JSON Schema (draft 2020-12) for .vcfabric files",
        mime: "application/schema+json",
        text: include_str!("../../../docs/formats/vcfabric.schema.json"),
    },
    TextDoc {
        uri: "vectorcraft://spec/vectorcraft-embroidery",
        name: "spec-vectorcraft-embroidery",
        title: "Embroidery data in documents (specification)",
        description: "Stitch settings on objects and the document's embroidery setup inside .vectorcraft files: every kind, default and range",
        mime: "text/markdown",
        text: include_str!("../../../docs/formats/vectorcraft-embroidery.md"),
    },
    TextDoc {
        uri: "vectorcraft://spec/vectorcraft-embroidery.schema.json",
        name: "spec-vectorcraft-embroidery-schema",
        title: "Embroidery data JSON Schema",
        description: "JSON Schema (draft 2020-12) for stitch settings and the embroidery setup",
        mime: "application/schema+json",
        text: include_str!("../../../docs/formats/vectorcraft-embroidery.schema.json"),
    },
];

/// The guide or specification at `uri`, if it is one.
pub fn text(uri: &str) -> Option<&'static TextDoc> {
    TEXT_DOCS.iter().find(|d| d.uri == uri)
}

/// Why a resource could not be read. The distinction matters: the server turns it into `-32002`
/// (no such resource) or `-32602` (bad URI), and guessing from the message text would not hold.
#[derive(Debug)]
pub enum ReadError {
    /// The URI names no resource or template this server serves.
    NotFound(String),
    /// The template has no value, or the value names nothing.
    Invalid(String),
    /// The backend could not be asked at all.
    Backend(String),
}

/// One addressable template: the URI with `{var}`, and what its variable can be.
pub struct Template {
    pub uri: &'static str,
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub mime: &'static str,
    pub live: Live,
}

/// Every template, in `resources/templates/list` order.
pub static TEMPLATES: &[Template] = &[
    Template {
        uri: "vectorcraft://object/{id}",
        name: "object",
        title: "One layer or object",
        description: "The layer or object with this id, with its children, bounds and paint (document.node {summary: true}). Large containers can be sliced with run_command document.node {id, summary, depth, childLimit}: truncated levels report childCount",
        mime: "application/json",
        live: Live::Objects,
    },
    Template {
        uri: "vectorcraft://command/{id}",
        name: "command",
        title: "One command",
        description: "The command with this id: label, menu path, shortcut, parameter description and whether it is enabled now (engine.commands)",
        mime: "application/json",
        live: Live::Commands,
    },
    Template {
        uri: "vectorcraft://effect/{id}",
        name: "effect",
        title: "One live effect",
        description: "The live effect with this id and its parameters with their defaults (effect.list)",
        mime: "application/json",
        live: Live::Effects,
    },
    Template {
        uri: "vectorcraft://swatch/{name}",
        name: "swatch",
        title: "One swatch",
        description: "The swatch or gradient/pattern swatch with this name (swatch.list)",
        mime: "application/json",
        live: Live::Swatches,
    },
];

/// `resources/list`.
pub fn list() -> Value {
    let mut list = vec![
        json!({"uri": DOC_URI, "name": "document", "title": "Active document (summary)", "description": "Layer tree, artboards, selection and history of the active document (document.inspect)", "mimeType": "application/json"}),
        json!({"uri": DOC_JSON_URI, "name": "document-json", "title": "Active document (full model)", "description": "The complete document model as JSON (document.json)", "mimeType": "application/json"}),
        json!({"uri": COMMANDS_URI, "name": "commands", "title": "Command catalog", "description": "Command ids, labels, parameter descriptions and current enabled state", "mimeType": "application/json"}),
    ];
    list.extend(TEXT_DOCS.iter().map(|d| json!({"uri": d.uri, "name": d.name, "title": d.title, "description": d.description, "mimeType": d.mime})));
    json!({ "resources": list })
}

/// `resources/templates/list`.
pub fn templates() -> Value {
    let list: Vec<Value> = TEMPLATES
        .iter()
        .map(|t| json!({"uriTemplate": t.uri, "name": t.name, "title": t.title, "description": t.description, "mimeType": t.mime}))
        .collect();
    json!({ "resourceTemplates": list })
}

/// Which template a URI is an instance of, and what its variable can be.
pub fn template_source(uri: &str) -> (&str, Option<Live>) {
    for t in TEMPLATES {
        let Some((prefix, _)) = t.uri.split_once('{') else { continue };
        if uri.starts_with(prefix) {
            return (t.uri, Some(t.live));
        }
    }
    (uri, None)
}

/// `resources/read`: the document behind a resource URI.
pub fn read(b: &mut dyn Backend, uri: &str) -> Result<Value, ReadError> {
    if uri == COMMANDS_URI {
        return b.call("engine.commands", json!({})).map_err(ReadError::Backend);
    }
    if uri == DOC_URI {
        return b.call("document.inspect", json!({})).map_err(ReadError::Backend);
    }
    if uri == DOC_JSON_URI {
        return exec(b, "document.json").map_err(ReadError::Backend);
    }
    let (template, var) = match TEMPLATES.iter().find(|t| template_source(uri).0 == t.uri) {
        Some(t) => (t, variable(uri, t.uri)),
        None => return Err(ReadError::NotFound(uri.to_string())),
    };
    if var.is_empty() {
        return Err(ReadError::Invalid(format!("{} needs a value for `{}`", template.uri, variable_name(template.uri))));
    }
    let found = match template.name {
        "object" => read_object(b, &var).map_err(ReadError::Backend)?,
        // The command catalogue is a control method; the rest are engine commands.
        "command" => find_in(b.call("engine.commands", json!({})).map_err(ReadError::Backend)?, None, "id", &var),
        "effect" => find_in(exec(b, "effect.list").map_err(ReadError::Backend)?, Some("catalog"), "id", &var),
        "swatch" => find_in(exec(b, "swatch.list").map_err(ReadError::Backend)?, Some("swatches"), "name", &var),
        other => return Err(ReadError::NotFound(format!("no reader for template `{other}`"))),
    };
    Ok(match found {
        Some(v) => v,
        None => return Err(ReadError::Invalid(format!("no {} named `{var}` in the active document", template.name))),
    })
}

/// Run a query engine command. That reaches both backends, where a control method the headless
/// session doesn't implement would not.
fn exec(b: &mut dyn Backend, command: &str) -> Result<Value, String> {
    b.call("engine.execute", json!({"command": command, "params": {}}))
}

/// The part of a URI between the template's `{` and its `}`.
fn variable(uri: &str, template: &str) -> String {
    let Some(open) = template.find('{') else { return String::new() };
    let prefix = &template[..open];
    let rest = uri.strip_prefix(prefix).unwrap_or("");
    let raw = rest.split('}').next().unwrap_or("");
    decode(raw)
}

/// The variable's name, e.g. `id` or `name`.
fn variable_name(template: &str) -> &str {
    template.split_once('{').and_then(|(_, r)| r.split_once('}')).map_or("", |(n, _)| n)
}

/// Percent-decoding, so a swatch called "Brand 40%" can be addressed as `Brand%2040%25`.
fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(b) = bytes.get(i).copied() {
        if b == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(s.get(i + 1..i + 3).unwrap_or(""), 16)
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(b);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The layer or object with this id in the active document, as its compact summary
/// (bounds, paint labels, subtree: the `document.inspect` shape for one node).
///
/// One `document.node {summary: true}` lookup instead of summarizing the whole document
/// and searching it. A non-numeric id, or one that names nothing, is "not found" (the
/// caller turns it into `-32602`); other failures (no document, an unreachable app) stay
/// backend errors.
fn read_object(b: &mut dyn Backend, id: &str) -> Result<Option<Value>, String> {
    let Ok(id) = id.parse::<u64>() else { return Ok(None) };
    match b.call("engine.execute", json!({"command": "document.node", "params": {"id": id, "summary": true}})) {
        Ok(v) => Ok(Some(v)),
        // `EngineError::NoNode`, as both backends word it.
        Err(e) if e.starts_with("no such object") => Ok(None),
        Err(e) => Err(e),
    }
}

/// The item in a query command's array whose `field` equals `value`.
fn find_in(reply: Value, key: Option<&str>, field: &str, value: &str) -> Option<Value> {
    let items = match key {
        Some(k) => reply.get(k).and_then(Value::as_array).map_or(&[][..], Vec::as_slice),
        None => reply.as_array().map_or(&[][..], Vec::as_slice),
    };
    items.iter().take(MAX_CATALOGUE).find(|i| i.get(field).and_then(Value::as_str) == Some(value)).cloned()
}

/// Catalogs come from the app, but never trust one that grew without bound.
const MAX_CATALOGUE: usize = 20_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables_are_split_out_and_decoded() {
        assert_eq!(variable("vectorcraft://object/42", "vectorcraft://object/{id}"), "42");
        assert_eq!(variable("vectorcraft://swatch/Brand%2040%25", "vectorcraft://swatch/{name}"), "Brand 40%");
        assert_eq!(variable("vectorcraft://swatch/a%zzb", "vectorcraft://swatch/{name}"), "a%zzb");
        assert_eq!(variable("vectorcraft://other", "vectorcraft://object/{id}"), "");
        assert_eq!(variable_name("vectorcraft://swatch/{name}"), "name");
        assert_eq!(variable_name("no template"), "");
    }

    #[test]
    fn templates_are_addressable() {
        assert_eq!(template_source("vectorcraft://object/7"), ("vectorcraft://object/{id}", Some(Live::Objects)));
        assert_eq!(template_source("vectorcraft://command/paint.setFill").1, Some(Live::Commands));
        assert_eq!(template_source("vectorcraft://document").1, None);
    }

    #[test]
    fn lists_are_well_formed() {
        let l = list();
        assert_eq!(
            l["resources"].as_array().map(Vec::len),
            Some(3 + TEXT_DOCS.len()),
            "the document, its JSON, the commands, and the guides and specs"
        );
        let t = templates();
        let items = t["resourceTemplates"].as_array().expect("templates");
        assert_eq!(items.len(), TEMPLATES.len());
        for (entry, t) in items.iter().zip(TEMPLATES) {
            assert_eq!(entry["uriTemplate"], t.uri);
            assert!(t.uri.contains('{'), "{} has no variable", t.uri);
            assert!(t.description.len() > 20, "{} needs a description", t.name);
        }
    }

    #[test]
    fn missing_and_unknown_uris_are_errors() {
        let mut b = Box::new(crate::Headless::with_document());
        assert!(read(b.as_mut(), "vectorcraft://nope").is_err());
        assert!(read(b.as_mut(), "vectorcraft://object/").is_err());
        assert!(read(b.as_mut(), "vectorcraft://object/99999").is_err());
        assert!(read(b.as_mut(), DOC_URI).is_ok());
        // An id that names nothing is a bad value; without a document the backend says why.
        for uri in ["vectorcraft://object/99999", "vectorcraft://object/abc"] {
            assert!(matches!(read(b.as_mut(), uri), Err(ReadError::Invalid(_))), "{uri}");
        }
        let mut empty = Box::new(crate::Headless::new());
        assert!(matches!(read(empty.as_mut(), "vectorcraft://object/1"), Err(ReadError::Backend(_))));
    }

    #[test]
    fn an_object_is_readable_once_it_exists() {
        let mut b = Box::new(crate::Headless::with_document());
        let made = b.call("engine.execute", json!({"command": "shape.rectangle", "params": {"x": 10, "y": 10, "width": 50, "height": 40}})).unwrap();
        let id = made["id"].as_i64().unwrap_or_default().to_string();
        let v = read(b.as_mut(), &format!("vectorcraft://object/{id}")).unwrap();
        assert_eq!(v["id"].to_string(), id);
        assert!(v["bounds"].is_object(), "{v}");
        // Layers resolve too, not just drawn objects.
        let inspect = b.call("document.inspect", json!({})).unwrap();
        let layer = inspect["layers"].as_array().and_then(|l| l.first()).expect("a layer");
        let lid = layer["id"].to_string();
        let v = read(b.as_mut(), &format!("vectorcraft://object/{lid}")).unwrap();
        assert_eq!(v["id"].to_string(), lid);
    }
}
