//! Embroidery files: `document.export {format: dst | pes | jef | exp | vp3 | u01}` writes the
//! stitched objects' design ([`vectorcraft_embroidery::plan`]); `document.open` and Place read a
//! `.dst`, `.pes`, `.jef`, `.exp`, `.vp3` or `.u01` design as manual-stitch paths, a group per
//! colour block.
//! `artboard`/`artboards`/`range` write a design per artboard; a design split into hoopings
//! (`embroidery.splitHoops`) is written a file per hooping.

use serde::Deserialize;
use serde_json::Value;
use vectorcraft_doc::{Document, NodeId};

use super::super::{EngineError, Result, bad};
use super::{ArtboardPick, Encoded, FormatOption};

const C: &str = "document.export";

/// The machine formats' stitches are the objects' own; the one option picks the hoopings.
pub(super) const OPTIONS: &[FormatOption] = &[FormatOption {
    name: "hoopings",
    ty: "boolean",
    default: "true",
    description: "a design split into hoopings (embroidery.splitHoops) is written a file per hooping, in sewing order (name-hoop1.dst…); false: the whole design in one file",
}];

#[derive(Default, Deserialize)]
#[serde(default)]
struct Params {
    #[serde(flatten)]
    boards: ArtboardPick,
    hoopings: Option<bool>,
}

/// `doc`'s stitched objects as a design in machine format `id` (`dst`, `pes`, `jef`, `exp`, `vp3`,
/// `u01`);
/// with `artboard`, `artboards` or `range`, a design per artboard, of the stitched objects centred
/// on it (a test sheet's sections); split into hoopings, a design per hooping.
pub(super) fn encode(id: &str, doc: &Document, p: &Value) -> Result<Encoded> {
    let params: Params = serde_json::from_value(p.clone()).map_err(|e| bad(C, format!("{}: {e}", id.to_uppercase())))?;
    let Some(boards) = params.boards.resolve(doc.artboards.len()).map_err(|e| bad(C, e))? else {
        if params.hoopings != Some(false) && doc.embroidery.hoopings.is_some() {
            return hoopings(id, doc);
        }
        if params.hoopings == Some(true) {
            return Err(bad(C, "the design isn't split into hoopings (embroidery.splitHoops)"));
        }
        let (bytes, warnings) = design(id, doc)?;
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
        let (bytes, warnings) = design(id, &part)?;
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

/// `doc`'s design in machine format `id`, and its warnings.
fn design(id: &str, doc: &Document) -> Result<(Vec<u8>, Vec<String>)> {
    if !vectorcraft_embroidery::has_stitches(doc) {
        return Err(bad(C, "nothing to stitch: give paths stitch settings first (Object › Embroidery, embroidery.set)"));
    }
    let plan = vectorcraft_embroidery::plan(doc);
    if plan.stats().stitches == 0 {
        return Err(bad(C, "the stitched objects make no stitches (hidden, empty or too small)"));
    }
    let mut warnings = plan.warnings.clone();
    // The design checker's findings the plan's warnings don't already give (long stitches…).
    warnings.extend(vectorcraft_embroidery::check::summary(&vectorcraft_embroidery::check::check(doc, &plan)));
    // Written as the machine sews it: turned in the hoop.
    let plan = vectorcraft_embroidery::hoops::rotated(plan, doc.embroidery.hoop_rotation());
    Ok((write(id, &plan, &doc.title, &mut warnings), warnings))
}

/// `doc`'s design split into its hoopings: a file in machine format `id` per hooping, in sewing
/// order, each turned in the hoop as the whole design would be.
fn hoopings(id: &str, doc: &Document) -> Result<Encoded> {
    if !vectorcraft_embroidery::has_stitches(doc) {
        return Err(bad(C, "nothing to stitch: give paths stitch settings first (Object › Embroidery, embroidery.set)"));
    }
    let plan = vectorcraft_embroidery::plan(doc);
    let Some((pieces, _)) = vectorcraft_embroidery::hoopings::of_document(doc, &plan) else {
        return Err(bad(
            C,
            "the design's hoopings can't be used (no hoop chosen, nothing stitched, or from a newer VectorCraft): split it again (embroidery.splitHoops), or export it whole (hoopings: false)",
        ));
    };
    let mut out = Encoded { warnings: plan.warnings.clone(), ..Encoded::default() };
    out.warnings.extend(vectorcraft_embroidery::check::summary(&vectorcraft_embroidery::check::check(doc, &plan)));
    let rotation = doc.embroidery.hoop_rotation();
    let mut labels = vec![];
    for (i, piece) in pieces.into_iter().enumerate() {
        let plan = vectorcraft_embroidery::hoops::rotated(piece.plan, rotation);
        let title = format!("{} {}", doc.title, i + 1);
        let bytes = write(id, &plan, &title, &mut out.warnings);
        out.files.push((Some(i), bytes));
        labels.push(format!("hoop{}", i + 1));
    }
    out.labels = Some(labels);
    Ok(out)
}

/// `plan` in machine format `id` (`dst`, `pes`, `jef`, `exp`, `vp3`, `u01`), adding the writer's
/// warnings.
fn write(id: &str, plan: &vectorcraft_embroidery::Plan, title: &str, warnings: &mut Vec<String>) -> Vec<u8> {
    match id {
        "pes" => {
            let (bytes, w) = vectorcraft_embroidery::write_pes(plan, title);
            warnings.extend(w);
            bytes
        }
        "jef" => {
            let t = vectorcraft_doc::metadata::now_unix().unwrap_or(0);
            vectorcraft_embroidery::write_jef(plan, vectorcraft_doc::metadata::civil(t))
        }
        "exp" => vectorcraft_embroidery::write_exp(plan),
        "vp3" => {
            let (bytes, w) = vectorcraft_embroidery::write_vp3(plan);
            warnings.extend(w);
            bytes
        }
        "u01" => {
            let (bytes, w) = vectorcraft_embroidery::write_u01(plan);
            warnings.extend(w);
            bytes
        }
        _ => vectorcraft_embroidery::write_dst(plan, title),
    }
}

/// A design in machine format `id` (`dst`, `pes`, `jef`, `exp`, `vp3`, `u01`) as a new document
/// titled `title`.
pub(super) fn import(id: &str, title: &str, bytes: &[u8]) -> Result<(Document, Vec<String>)> {
    let plan = match id {
        "pes" => vectorcraft_embroidery::read_pes(bytes).map_err(EngineError::Other)?,
        "jef" => vectorcraft_embroidery::read_jef(bytes).map_err(EngineError::Other)?,
        "exp" => vectorcraft_embroidery::read_exp(bytes),
        "vp3" => vectorcraft_embroidery::read_vp3(bytes).map_err(EngineError::Other)?,
        "u01" => vectorcraft_embroidery::read_u01(bytes).map_err(EngineError::Other)?,
        _ => vectorcraft_embroidery::read_dst(bytes).map_err(|e| EngineError::Other(e.to_string()))?,
    };
    let mut warnings = plan.warnings.clone();
    if plan.stats().stitches == 0 {
        warnings.push("the design has no stitches".into());
    }
    let name = id.to_uppercase();
    if plan.threads.len() > 1 {
        match id {
            "dst" | "exp" => warnings.push(format!("{name} files hold no thread colours: each colour block got a colour of its own")),
            "jef" => warnings.push(
                "JEF files name threads by Janome palette number, and VectorCraft has no maker's palette: each number got a colour of its own".into(),
            ),
            "u01" => warnings.push("U01 files name needles, not thread colours: each needle got a colour of its own".into()),
            // A PES file keeps its colours in its version 6 thread list; older ones by number.
            "pes" if bytes.get(4..8) != Some(b"0060") => warnings.push(
                "this PES file names threads by Brother palette number, and VectorCraft has no maker's palette: each number got a colour of its own"
                    .into(),
            ),
            _ => {}
        }
    }
    Ok((vectorcraft_embroidery::to_document(&plan, title), warnings))
}
