//! Embroidery files: `document.export {format: dst}` writes the stitched objects' design
//! ([`vectorcraft_embroidery::plan`]); `document.open` and Place read a `.dst` design as
//! manual-stitch paths, a group per colour block.

use serde_json::Value;
use vectorcraft_doc::Document;

use super::super::*;
use super::{Encoded, FormatOption};

const C: &str = "document.export";

/// DST takes no options: its stitches are the objects' own.
pub(super) const OPTIONS: &[FormatOption] = &[];

/// `doc`'s stitched objects as a DST design.
pub(super) fn encode(doc: &Document, _: &Value) -> Result<Encoded> {
    if !vectorcraft_embroidery::has_stitches(doc) {
        return Err(bad(C, "nothing to stitch: give paths stitch settings first (Object › Embroidery, embroidery.set)"));
    }
    let plan = vectorcraft_embroidery::plan(doc);
    if plan.stats().stitches == 0 {
        return Err(bad(C, "the stitched objects make no stitches (hidden, empty or too small)"));
    }
    let bytes = vectorcraft_embroidery::write_dst(&plan, &doc.title);
    Ok(Encoded { warnings: plan.warnings, ..Encoded::one(bytes) })
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
