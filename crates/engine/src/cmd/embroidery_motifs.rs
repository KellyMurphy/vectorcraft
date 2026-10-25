//! Motifs (E8.4a): `.vcmotif` tiles kept in the document (`EmbroiderySetup::motifs`), listed,
//! loaded (from a file or the embroidery library), saved and removed. Carve motifs are carved
//! into fills (`embroidery.set {motif}`); stitched motifs are sewn tile by tile.

use serde_json::{Value, json};
use vectorcraft_doc::{Document, Motif, StitchKind};
use vectorcraft_embroidery::motifs;

use super::overprint::edit_counted;
use super::swatchlib::FileData;
use super::*;

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "embroidery.motifs",
            "Motifs",
            ["Window", "Embroidery"],
            None,
            "{} the motifs fills can use: the document's own (loaded from .vcmotif files) then the built-in carve motifs → {motifs: [{id, name, builtin, supported (false: from a newer VectorCraft, kept but not usable; only id, name and used are given), motifKind: stitched|carve, size (mm, the tile's own size), offsets: [[x, y], [x, y]] (mm, the lattice), author, license, source, description, used: objects using it}]}",
            has_doc,
            list_motifs
        ),
        cmd!(
            "embroidery.loadMotif",
            "Load Motif…",
            ["Window", "Embroidery"],
            None,
            "{path? | data?: the file's text | library?: an embroidery library entry (embroidery.libraries, kind motifs: its id or name)} load a .vcmotif motif file into the document (kept in it, so the document sews the same anywhere), as one undo step. Its id is the file's (else made from its name); one already taken by a different motif, or a built-in one's, gets a number (stars-2…), and a motif the document (or the built-in ones) already has is not added again → {id, name, motifKind, added}",
            has_doc,
            load_motif
        ),
        cmd!(
            "embroidery.saveMotif",
            "Save Motif…",
            ["Window", "Embroidery"],
            None,
            "{motif: id (the document's or a built-in one), path?, user?: false} write a motif as a .vcmotif file (JSON; specified in docs/formats/vcmotif.md) → {path}, or without path {data: the file's text} (`user: true`: into the embroidery library folder instead → {path, library: its id})",
            has_doc,
            save_motif
        ),
        cmd!(
            "embroidery.removeMotif",
            "Remove Motif",
            ["Window", "Embroidery"],
            None,
            "{motif: id} remove one of the document's motifs, as one undo step (not one objects still use; built-in ones can't be removed) → {removed: id}",
            has_doc,
            remove_motif
        ),
    ]
}

/// How many objects use motif `id`.
fn users(doc: &Document, id: &str) -> usize {
    let mut n = 0;
    doc.walk(|node| n += usize::from(node.extras().stitch.as_deref().is_some_and(|s| s.kind == StitchKind::Fill && s.motif.as_deref() == Some(id))));
    n
}

fn list_motifs(s: &mut Session, _: &Value) -> Result<Value> {
    let doc = &s.doc()?.doc;
    let built_in = motifs::MOTIFS.iter().filter_map(|id| motifs::builtin(id)).map(|m| (m, true));
    let list: Vec<Value> = doc
        .embroidery
        .motifs
        .iter()
        .cloned()
        .map(|m| (m, false))
        .chain(built_in)
        .map(|(m, builtin)| {
            if m.unknown.is_some() {
                // From a newer version: listed so it can be removed, but it can't be used.
                return json!({"id": m.id, "name": m.name, "builtin": false, "supported": false, "used": users(doc, &m.id)});
            }
            json!({
                "supported": true, "id": m.id, "name": m.name, "builtin": builtin, "motifKind": m.kind, "size": m.size,
                "offsets": m.offsets, "author": m.author, "license": m.license, "source": m.source, "description": m.description,
                "used": users(doc, &m.id),
            })
        })
        .collect();
    Ok(json!({ "motifs": list }))
}

fn load_motif(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.loadMotif";
    let m = match super::embroidery_lib::library_entry(&s.embroidery_libraries.motifs, p, C)? {
        Some((_, m)) => (*m).clone(),
        None => motifs::read_file(&super::embroidery::file_param(p, C)?).map_err(|e| bad(C, e))?,
    };
    let kind = m.kind;
    let name = m.name.clone();
    let (id, added) = add_motif(s, m, "Load Motif", C)?;
    Ok(json!({"id": id, "name": name, "motifKind": kind, "added": added}))
}

/// Add `m` to the document's motifs as undo step `label`: under its id, else (the id taken by a
/// different motif, or a built-in one's) a numbered one. The very same motif already there isn't
/// added again. → (its id, whether it was added).
pub(crate) fn add_motif(s: &mut Session, mut m: Motif, label: &str, cmd: &str) -> Result<(String, bool)> {
    let mine = &s.doc()?.doc.embroidery.motifs;
    let same = |q: &Motif| q.id == m.id && *q == m;
    if mine.iter().any(same) || motifs::builtin(&m.id).is_some_and(|q| same(&q)) {
        return Ok((m.id, false));
    }
    let taken = |id: &str| motifs::MOTIFS.contains(&id) || mine.iter().any(|q| q.id == id);
    if taken(&m.id) {
        let base = m.id.clone();
        let Some(id) = (2..10_000).map(|n| format!("{base}-{n}")).find(|id| !taken(id)) else {
            return Err(bad(cmd, format!("too many motifs named `{base}`")));
        };
        m.id = id;
    }
    let id = m.id.clone();
    edit_counted(s, label, |d| {
        d.embroidery.motifs.push(m);
        Ok(1)
    })?;
    Ok((id, true))
}

/// The `motif` param: a motif the document can use.
fn motif_param(s: &Session, p: &Value, cmd: &str) -> Result<Motif> {
    let id = str_param(p, "motif").map(|id| id.trim().to_ascii_lowercase()).ok_or_else(|| bad(cmd, "`motif` (a motif id) is missing"))?;
    motifs::find(&s.doc()?.doc.embroidery.motifs, &id).ok_or_else(|| bad(cmd, format!("no motif `{id}` (see embroidery.motifs)")))
}

fn save_motif(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.saveMotif";
    let m = motif_param(s, p, C)?;
    let text = motifs::write_file(&m).map_err(|e| bad(C, e))?;
    let mut out = json!({});
    s.embroidery_libraries.motifs.write(p, &m.name, motifs::FILE_EXT, FileData::Text(text), &mut out, C)?;
    Ok(out)
}

fn remove_motif(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.removeMotif";
    // Found among the document's own, readable or not (one from a newer version can be removed).
    let id = str_param(p, "motif").map(|id| id.trim().to_ascii_lowercase()).ok_or_else(|| bad(C, "`motif` (a motif id) is missing"))?;
    let Some(m) = s.doc()?.doc.embroidery.motifs.iter().find(|q| q.id == id).cloned() else {
        let why = if motifs::builtin(&id).is_some() { "is built in: it can't be removed" } else { "isn't one of the document's motifs" };
        return Err(bad(C, format!("`{id}` {why}")));
    };
    let n = users(&s.doc()?.doc, &m.id);
    if n > 0 {
        return Err(bad(C, format!("{n} object(s) use `{}`: give them another motif first", m.id)));
    }
    let id = m.id.clone();
    edit_counted(s, "Remove Motif", |d| {
        d.embroidery.motifs.retain(|q| q.id != id);
        Ok(1)
    })?;
    Ok(json!({"removed": m.id}))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CROSSES: &str = include_str!("../../../../docs/formats/examples/crosses.vcmotif");

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 400, "height": 400})).unwrap();
        s
    }

    /// A carve motif file: a cross on a square lattice.
    fn carve() -> String {
        CROSSES.replace("\"stitched\"", "\"carve\"").replace("\"crosses\"", "\"carved-crosses\"")
    }

    #[test]
    fn motifs_load_list_save_and_remove() {
        let mut s = session();
        let r = s.execute("embroidery.loadMotif", &json!({"data": CROSSES})).unwrap();
        assert_eq!((r["id"].as_str(), r["added"].as_bool(), r["motifKind"].as_str()), (Some("crosses"), Some(true), Some("stitched")));
        // The very same one isn't added twice; a different one under the same id gets a number.
        assert_eq!(s.execute("embroidery.loadMotif", &json!({"data": CROSSES})).unwrap()["added"], false);
        let other = CROSSES.replace("\"size\": 6", "\"size\": 7");
        assert_eq!(s.execute("embroidery.loadMotif", &json!({"data": other})).unwrap()["id"], "crosses-2");
        // A built-in id is taken too.
        let waves = CROSSES.replace("\"crosses\"", "\"waves\"");
        assert_eq!(s.execute("embroidery.loadMotif", &json!({"data": waves})).unwrap()["id"], "waves-2");
        let list = s.execute("embroidery.motifs", &json!({})).unwrap();
        let ids: Vec<&str> = list["motifs"].as_array().unwrap().iter().filter_map(|m| m["id"].as_str()).collect();
        assert_eq!(ids.first().copied(), Some("crosses"));
        assert!(motifs::MOTIFS.iter().all(|m| ids.contains(m)), "{ids:?}");
        // Saved, it reads back as it was.
        let saved = s.execute("embroidery.saveMotif", &json!({"motif": "crosses"})).unwrap();
        let back = motifs::read_file(saved["data"].as_str().unwrap().as_bytes()).unwrap();
        assert_eq!(back.offsets, [[6.0, 0.0], [3.0, 5.0]]);
        // A built-in one saves too, as data.
        assert!(s.execute("embroidery.saveMotif", &json!({"motif": "scales"})).unwrap()["data"].as_str().unwrap().contains("vectorcraft-motif"));
        // Removed, one undo step; built-in ones stay.
        s.execute("embroidery.removeMotif", &json!({"motif": "crosses-2"})).unwrap();
        assert!(s.execute("embroidery.removeMotif", &json!({"motif": "circles"})).is_err());
        s.execute("edit.undo", &json!({})).unwrap();
        assert!(s.doc().unwrap().doc.embroidery.motifs.iter().any(|m| m.id == "crosses-2"));
        assert!(s.execute("embroidery.loadMotif", &json!({"data": "{}"})).is_err());
    }

    #[test]
    fn a_loaded_carve_motif_carves_a_fill_and_is_kept_while_used() {
        let mut s = session();
        s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 200, "height": 200})).unwrap();
        s.execute("embroidery.set", &json!({"type": "fill"})).unwrap();
        let plain = s.execute("embroidery.stitches", &json!({})).unwrap();
        // A stitched motif can't be carved; nor can one the document doesn't have.
        s.execute("embroidery.loadMotif", &json!({"data": CROSSES})).unwrap();
        assert!(s.execute("embroidery.set", &json!({"motif": "crosses"})).is_err());
        assert!(s.execute("embroidery.set", &json!({"motif": "nothing"})).is_err());
        s.execute("embroidery.loadMotif", &json!({"data": carve()})).unwrap();
        s.execute("embroidery.set", &json!({"motif": "carved-crosses", "motifSize": 12})).unwrap();
        let carved = s.execute("embroidery.stitches", &json!({})).unwrap();
        assert_ne!(carved, plain, "the motif moved the needle points");
        let info = s.execute("embroidery.info", &json!({})).unwrap();
        assert!(info["design"]["warnings"].as_array().is_none_or(|w| w.is_empty()), "{info}");
        // Used: it can't be removed.
        assert!(s.execute("embroidery.removeMotif", &json!({"motif": "carved-crosses"})).is_err());
        // The built-in ones still carve as before.
        s.execute("embroidery.set", &json!({"motif": "scales"})).unwrap();
    }
}
