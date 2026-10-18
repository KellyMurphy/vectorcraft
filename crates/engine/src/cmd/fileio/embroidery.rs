//! Embroidery files: `document.export {format: dst}` writes the stitched objects' design
//! ([`vectorcraft_embroidery::plan`]); `document.open` and Place read a `.dst` design as
//! manual-stitch paths, a group per colour block. `artboard`/`artboards`/`range` write a design per
//! artboard.

use serde::Deserialize;
use serde_json::Value;
use vectorcraft_doc::{Document, NodeId};

use super::super::*;
use super::{ArtboardPick, Encoded, FormatOption};

const C: &str = "document.export";

/// DST takes no options: its stitches are the objects' own.
pub(super) const OPTIONS: &[FormatOption] = &[];

#[derive(Default, Deserialize)]
#[serde(default)]
struct Params {
    #[serde(flatten)]
    boards: ArtboardPick,
}

/// `doc`'s stitched objects as a DST design; with `artboard`, `artboards` or `range`, a design
/// per artboard, of the stitched objects centred on it (a test sheet's sections).
pub(super) fn encode(doc: &Document, p: &Value) -> Result<Encoded> {
    let params: Params = serde_json::from_value(p.clone()).map_err(|e| bad(C, format!("DST: {e}")))?;
    let Some(boards) = params.boards.resolve(doc.artboards.len()).map_err(|e| bad(C, e))? else {
        let (bytes, warnings) = design(doc)?;
        return Ok(Encoded { warnings, ..Encoded::one(bytes) });
    };
    let mut out = Encoded::default();
    for i in &boards {
        let part = on_artboard(doc, *i);
        if !vectorcraft_embroidery::has_stitches(&part) && boards.len() > 1 {
            let name = doc.artboards.get(*i).map(|a| a.name.clone()).unwrap_or_default();
            out.warnings.push(format!("artboard {} ({name}) has nothing stitched: no file for it", i + 1));
            continue;
        }
        let (bytes, warnings) = design(&part)?;
        out.files.push((Some(*i), bytes));
        out.warnings.extend(warnings);
    }
    if out.files.is_empty() {
        return Err(bad(C, "nothing to stitch on those artboards"));
    }
    Ok(out)
}

/// `doc` with only the stitched objects whose middle lies on artboard `index` still stitched.
fn on_artboard(doc: &Document, index: usize) -> Document {
    let Some(rect) = doc.artboards.get(index).map(|a| a.rect) else { return doc.clone() };
    let mut off: Vec<NodeId> = vec![];
    doc.walk(|n| {
        if n.extras().stitch.is_some() && !n.geometric_bounds().is_some_and(|b| rect.contains(b.center())) {
            off.push(n.id);
        }
    });
    let mut d = doc.clone();
    for id in off {
        if let Some(n) = d.node_mut(id) {
            n.extras_mut().stitch = None;
        }
    }
    d
}

/// `doc`'s design as DST, and its warnings.
fn design(doc: &Document) -> Result<(Vec<u8>, Vec<String>)> {
    if !vectorcraft_embroidery::has_stitches(doc) {
        return Err(bad(C, "nothing to stitch: give paths stitch settings first (Object › Embroidery, embroidery.set)"));
    }
    let plan = vectorcraft_embroidery::plan(doc);
    if plan.stats().stitches == 0 {
        return Err(bad(C, "the stitched objects make no stitches (hidden, empty or too small)"));
    }
    let bytes = vectorcraft_embroidery::write_dst(&plan, &doc.title);
    // The design checker's findings the plan's warnings don't already give (long stitches…).
    let mut warnings = plan.warnings.clone();
    warnings.extend(vectorcraft_embroidery::check::summary(&vectorcraft_embroidery::check::check(doc, &plan)));
    Ok((bytes, warnings))
}

/// A DST design as a new document titled `title`.
pub(super) fn import(title: &str, bytes: &[u8]) -> Result<(Document, Vec<String>)> {
    let plan = vectorcraft_embroidery::read_dst(bytes).map_err(|e| EngineError::Other(e.to_string()))?;
    let mut warnings = plan.warnings.clone();
    if plan.stats().stitches == 0 {
        warnings.push("the design has no stitches".into());
    }
    if plan.threads.len() > 1 {
        warnings.push("DST files hold no thread colours: each colour block got a colour of its own".into());
    }
    Ok((vectorcraft_embroidery::to_document(&plan, title), warnings))
}
