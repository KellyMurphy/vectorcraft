//! Stitch styles (E11.3): named sets of stitch settings kept in the document
//! (`EmbroiderySetup::styles`), listed, saved from an object, applied with one click, loaded from
//! `.vcstyle` files (or the embroidery library's `styles/`) and removed. Applying a style copies
//! its settings: objects keep no link to it.

use serde_json::{Value, json};
use vectorcraft_doc::{Document, MotifKind, StitchKind, StitchSettings, StitchStyle};
use vectorcraft_embroidery::styles;

use super::overprint::edit_counted;
use super::swatchlib::FileData;
use super::{CommandSpec, Result, Session, bad, cmd, has_doc, id_param, str_param, targets};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "embroidery.styles",
            "Stitch Styles",
            ["Window", "Embroidery"],
            None,
            "{} the stitch styles: the document's own (saved or loaded) then the starters → {styles: [{id, name, builtin, supported (false: from a newer VectorCraft, kept but not usable; only id and name are given), settings: the style's settings as embroidery.set takes them (type, width…), author, license, source, description}], selected: the id of the style whose settings the first selected stitched object has | null}",
            has_doc,
            list_styles
        ),
        cmd!(
            "embroidery.saveStyle",
            "Save Stitch Style…",
            ["Window", "Embroidery"],
            None,
            "{name?, id?: object (default: the first selected stitched object), style?: a style's id or name (instead of an object: write that style out), description?, path?, user?: false} make a stitch style from an object's stitch settings (less what belongs to that object: its start point, stitch angle lines, rails, trims and stops) and keep it in the document, as one undo step; a style of the same id is replaced. With `path` (or `user: true`: into the embroidery library folder) it is also written as a .vcstyle file (specified in docs/formats/vcstyle.md). With `style`, that style is written out instead (→ {path}, or without path {data: the file's text}) → {id, name, replaced, path?, library?}",
            has_doc,
            save_style
        ),
        cmd!(
            "embroidery.applyStyle",
            "Apply Stitch Style",
            ["Window", "Embroidery"],
            None,
            "{style?: a style's id or name (the document's, else a starter: small-lettering|bold-border|light-fill) | library?: an embroidery library entry (embroidery.libraries, kind styles: its id or name; copied into the document), ids?} stitch the paths, compound paths and type objects in ids or the selection (groups: those inside) with the style's settings, replacing their own but keeping each object's start point, stitch angle lines, rails, trims and stops, and its thread colour when the style sets none; they keep no link to the style. One undo step → {style: id, changed}",
            has_doc,
            apply_style
        ),
        cmd!(
            "embroidery.loadStyle",
            "Load Stitch Style…",
            ["Window", "Embroidery"],
            None,
            "{path? | data?: the file's text | library?: an embroidery library entry (embroidery.libraries, kind styles: its id or name)} load a .vcstyle stitch style file into the document, as one undo step. Its id is the file's (else made from its name); one already taken by a different style, or a starter's, gets a number (bold-border-2…), and a style the document already has is not added again → {id, name, added}",
            has_doc,
            load_style
        ),
        cmd!(
            "embroidery.removeStyle",
            "Remove Stitch Style",
            ["Window", "Embroidery"],
            None,
            "{style: id} remove one of the document's stitch styles, as one undo step (objects keep the settings it gave them; starters can't be removed) → {removed: id}",
            has_doc,
            remove_style
        ),
    ]
}

/// Settings `s` as `embroidery.set` takes them (`type` for the document's `kind`).
fn settings_json(s: &StitchSettings) -> Value {
    let mut v = serde_json::to_value(s).unwrap_or_default();
    if let Some(o) = v.as_object_mut()
        && let Some(kind) = o.remove("kind")
    {
        o.insert("type".into(), kind);
    }
    v
}

fn style_json(st: &StitchStyle, builtin: bool) -> Value {
    if st.unknown.is_some() {
        // From a newer version: listed so it can be removed, but it can't be used.
        return json!({"id": st.id, "name": st.name, "builtin": false, "supported": false});
    }
    json!({
        "supported": true, "id": st.id, "name": st.name, "builtin": builtin, "settings": settings_json(&st.stitch),
        "author": st.author, "license": st.license, "source": st.source, "description": st.description,
    })
}

/// The document's styles, then the starters.
fn all_styles(doc: &Document) -> Vec<(StitchStyle, bool)> {
    let starters = styles::BUILTIN.iter().filter_map(|id| styles::builtin(id)).map(|s| (s, true));
    doc.embroidery.styles.iter().cloned().map(|s| (s, false)).chain(starters).collect()
}

fn list_styles(s: &mut Session, _: &Value) -> Result<Value> {
    let st = s.doc()?;
    let doc = &st.doc;
    let all = all_styles(doc);
    // The first selected stitched object's settings, as a style would keep them.
    let mine = super::embroidery::stitchable(s, st.selection.subjects())?
        .into_iter()
        .find_map(|id| doc.node(id).and_then(|n| n.extras().stitch.as_deref().filter(|t| t.unknown.is_none()).map(StitchSettings::as_style)));
    let selected = mine.and_then(|m| all.iter().find(|(st, _)| st.unknown.is_none() && st.stitch.as_style() == m).map(|(st, _)| st.id.clone()));
    Ok(json!({"styles": all.iter().map(|(st, b)| style_json(st, *b)).collect::<Vec<_>>(), "selected": selected}))
}

/// The style `style` names (the document's, else a starter).
fn style_param(s: &Session, p: &Value, cmd: &str) -> Result<StitchStyle> {
    let key =
        str_param(p, "style").map(str::trim).filter(|k| !k.is_empty()).ok_or_else(|| bad(cmd, "`style` (a stitch style's id or name) is missing"))?;
    styles::find(&s.doc()?.doc.embroidery.styles, key).ok_or_else(|| bad(cmd, format!("no stitch style `{key}` (see embroidery.styles)")))
}

fn save_style(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.saveStyle";
    let text = |key: &str| str_param(p, key).map(str::trim).filter(|t| !t.is_empty()).map(str::to_string);
    if p.get("style").is_some() {
        let st = style_param(s, p, C)?;
        let data = styles::write_file(&st).map_err(|e| bad(C, e))?;
        let mut out = json!({"id": st.id, "name": st.name});
        s.embroidery_libraries.styles.write(p, &st.name, styles::FILE_EXT, FileData::Text(data), &mut out, C)?;
        return Ok(out);
    }
    let ids = match id_param(p, "id") {
        Some(id) => vec![id],
        None => s.doc()?.selection.subjects().to_vec(),
    };
    let doc = &s.doc()?.doc;
    let stitch = super::embroidery::stitchable(s, &ids)?
        .into_iter()
        .find_map(|id| doc.node(id).and_then(|n| n.extras().stitch.as_deref().cloned()))
        .ok_or_else(|| bad(C, "select a stitched object to make the style from (or pass `id`, or `style` to write one out)"))?;
    if stitch.unknown.is_some() {
        return Err(bad(C, "the object's stitch settings are from a newer VectorCraft: they can't be made into a style"));
    }
    let name = match text("name") {
        Some(n) => n,
        None => super::unique_name("Stitch Style", |n| doc.embroidery.styles.iter().any(|st| st.name == n)),
    };
    let id = vectorcraft_embroidery::borders::slug(&name);
    if id.is_empty() {
        return Err(bad(C, "give the style a `name` with a letter or digit in it"));
    }
    let st =
        StitchStyle { id: id.clone(), name: name.clone(), description: text("description"), stitch: stitch.as_style(), ..StitchStyle::default() };
    styles::check(&st).map_err(|e| bad(C, e))?;
    let replaced = doc.embroidery.styles.iter().any(|q| q.id == id);
    let new = st.clone();
    edit_counted(s, "Save Stitch Style", |d| {
        match d.embroidery.styles.iter_mut().find(|q| q.id == new.id) {
            Some(q) if *q == new => return Ok(0),
            Some(q) => *q = new,
            None => d.embroidery.styles.push(new),
        }
        Ok(1)
    })?;
    let mut out = json!({"id": id, "name": name, "replaced": replaced});
    if p.get("path").is_some() || p.get("user").is_some_and(|u| u.as_bool() == Some(true)) {
        let data = styles::write_file(&st).map_err(|e| bad(C, e))?;
        s.embroidery_libraries.styles.write(p, &st.name, styles::FILE_EXT, FileData::Text(data), &mut out, C)?;
    }
    Ok(out)
}

/// Add `st` to `d`'s styles: under its id, else (the id taken by a different style, or a
/// starter's) a numbered one. The very same style already there isn't added again. → (its id,
/// whether it was added).
fn add_style(d: &mut Document, mut st: StitchStyle, cmd: &str) -> Result<(String, bool)> {
    let mine = &d.embroidery.styles;
    // The same style, under its id or one numbered when it was first loaded.
    let numbered =
        |id: &str| id == st.id || id.strip_prefix(st.id.as_str()).and_then(|n| n.strip_prefix('-')).is_some_and(|n| n.parse::<u32>().is_ok());
    let same = |q: &StitchStyle| numbered(&q.id) && *q == StitchStyle { id: q.id.clone(), ..st.clone() };
    if let Some(q) = mine.iter().find(|q| same(q)) {
        return Ok((q.id.clone(), false));
    }
    if styles::builtin(&st.id).is_some_and(|q| same(&q)) {
        return Ok((st.id, false));
    }
    let taken = |id: &str| styles::builtin(id).is_some() || mine.iter().any(|q| q.id == id);
    if taken(&st.id) {
        let base = st.id.clone();
        let Some(id) = (2..10_000).map(|n| format!("{base}-{n}")).find(|id| !taken(id)) else {
            return Err(bad(cmd, format!("too many stitch styles named `{base}`")));
        };
        st.id = id;
    }
    let id = st.id.clone();
    d.embroidery.styles.push(st);
    Ok((id, true))
}

fn load_style(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.loadStyle";
    let st = match super::embroidery_lib::library_entry(&s.embroidery_libraries.styles, p, C)? {
        Some((_, st)) => (*st).clone(),
        None => styles::read_file(&super::embroidery::file_param(p, C)?).map_err(|e| bad(C, e))?,
    };
    let name = st.name.clone();
    let mut out = (String::new(), false);
    edit_counted(s, "Load Stitch Style", |d| {
        out = add_style(d, st, C)?;
        Ok(usize::from(out.1))
    })?;
    Ok(json!({"id": out.0, "name": name, "added": out.1}))
}

/// Why `t` can't be sewn in `doc`: a border pattern, motif or embroidery font it names that the
/// document doesn't have.
fn missing(doc: &Document, t: &StitchSettings) -> Option<String> {
    use vectorcraft_embroidery::{borders, motifs};
    let e = &doc.embroidery;
    if t.kind == StitchKind::PatternBorder
        && let Some(id) = &t.pattern
        && borders::find(&e.patterns, id).is_none()
    {
        return Some(format!("its border pattern `{id}` isn't in the document (embroidery.loadBorderPattern adds it)"));
    }
    if let Some(id) = &t.motif {
        let want = match t.kind {
            StitchKind::Fill => Some(MotifKind::Carve),
            StitchKind::MotifFill => Some(MotifKind::Stitched),
            _ => None,
        };
        if let Some(want) = want
            && motifs::find(&e.motifs, id).is_none_or(|m| m.kind != want)
        {
            return Some(format!("its motif `{id}` isn't in the document (embroidery.loadMotif adds it)"));
        }
    }
    if let Some(id) = &t.font
        && !e.fonts.iter().any(|f| f.id == *id && f.unknown.is_none())
    {
        return Some(format!("its embroidery font `{id}` isn't in the document (embroidery.loadFont adds it)"));
    }
    None
}

fn apply_style(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.applyStyle";
    let from_library = super::embroidery_lib::library_entry(&s.embroidery_libraries.styles, p, C)?.map(|(_, st)| (*st).clone());
    let st = match &from_library {
        Some(st) => st.clone(),
        None => style_param(s, p, C)?,
    };
    if st.unknown.is_some() {
        return Err(bad(C, format!("the stitch style `{}` is from a newer VectorCraft: it can't be applied", st.id)));
    }
    let ids = super::embroidery::stitchable(s, &targets(s, p)?)?;
    if ids.is_empty() {
        return Err(bad(C, "select paths, compound paths or type to stitch"));
    }
    if let Some(why) = missing(&s.doc()?.doc, &st.stitch) {
        return Err(bad(C, format!("the stitch style `{}` can't be used here: {why}", st.id)));
    }
    let (mut id, mut changed) = (st.id.clone(), 0);
    edit_counted(s, "Apply Stitch Style", |d| {
        let mut added = false;
        // A library style is kept in the document, so it sews the same anywhere.
        if let Some(lib) = from_library {
            (id, added) = add_style(d, lib, C)?;
        }
        for nid in &ids {
            let Some(node) = d.node_mut(*nid) else { continue };
            let new = st.stitch.styled(node.extras().stitch.as_deref());
            if node.extras().stitch.as_deref() != Some(&new) {
                node.extras_mut().stitch = Some(Box::new(new));
                changed += 1;
            }
        }
        Ok(changed + usize::from(added))
    })?;
    Ok(json!({"style": id, "changed": changed}))
}

fn remove_style(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.removeStyle";
    // Found among the document's own, readable or not (one from a newer version can be removed).
    let key = str_param(p, "style").map(str::trim).ok_or_else(|| bad(C, "`style` (a stitch style id) is missing"))?;
    let mine = &s.doc()?.doc.embroidery.styles;
    let Some(st) = mine.iter().find(|q| q.id == key).or_else(|| mine.iter().find(|q| q.name.eq_ignore_ascii_case(key))).cloned() else {
        let why = if styles::find(&[], key).is_some() { "is a starter: it can't be removed" } else { "isn't one of the document's stitch styles" };
        return Err(bad(C, format!("`{key}` {why}")));
    };
    edit_counted(s, "Remove Stitch Style", |d| {
        d.embroidery.styles.retain(|q| q.id != st.id);
        Ok(1)
    })?;
    Ok(json!({"removed": st.id}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 400, "height": 400})).unwrap();
        s
    }

    fn stitch(s: &Session, id: &Value) -> StitchSettings {
        let id = vectorcraft_doc::NodeId(id.as_u64().unwrap());
        s.doc().unwrap().doc.node(id).unwrap().extras().stitch.as_deref().cloned().unwrap()
    }

    #[test]
    fn a_style_is_saved_from_an_object_and_applied_to_others() {
        let mut s = session();
        let a = s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 100, "height": 50})).unwrap()["id"].clone();
        s.execute("embroidery.set", &json!({"type": "satin", "width": 3, "spacing": 0.3, "pull": 0.25, "startAt": [1, 1], "trimAfter": true}))
            .unwrap();
        let r = s.execute("embroidery.saveStyle", &json!({"name": "Thin Border"})).unwrap();
        assert_eq!((r["id"].as_str(), r["replaced"].as_bool()), (Some("thin-border"), Some(false)));
        let list = s.execute("embroidery.styles", &json!({})).unwrap();
        assert_eq!(list["selected"], "thin-border", "{list}");
        let first = &list["styles"][0];
        assert_eq!((first["settings"]["type"].as_str(), first["settings"]["width"].as_f64()), (Some("satin"), Some(3.0)));
        assert!(first["settings"].get("startAt").is_none() && first["settings"].get("trimAfter").is_none(), "{first}");
        let ids: Vec<&str> = list["styles"].as_array().unwrap().iter().filter_map(|st| st["id"].as_str()).collect();
        assert!(styles::BUILTIN.iter().all(|b| ids.contains(b)), "{ids:?}");

        // Another object, its own start point and thread: the style replaces the rest.
        let b = s.execute("shape.ellipse", &json!({"x": 200, "y": 200, "width": 80, "height": 80})).unwrap()["id"].clone();
        s.execute("embroidery.set", &json!({"type": "running", "length": 2, "startAt": [5, 5], "thread": "#ff0000"})).unwrap();
        let r = s.execute("embroidery.applyStyle", &json!({"style": "Thin Border"})).unwrap();
        assert_eq!((r["style"].as_str(), r["changed"].as_u64()), (Some("thin-border"), Some(1)));
        let t = stitch(&s, &b);
        assert_eq!((t.kind, t.width, t.spacing, t.length), (StitchKind::Satin, Some(3.0), Some(0.3), None));
        assert_eq!((t.start_at, t.thread), (Some([5.0, 5.0]), Some([255, 0, 0])));
        // The source keeps its own trim; applying again changes nothing; undo is one step.
        assert!(stitch(&s, &a).trim_after);
        assert_eq!(s.execute("embroidery.applyStyle", &json!({"style": "thin-border"})).unwrap()["changed"], 0);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(stitch(&s, &b).kind, StitchKind::Running);

        // Saved again under the same name: replaced.
        s.execute("embroidery.set", &json!({"ids": [a], "width": 5})).unwrap();
        let r = s.execute("embroidery.saveStyle", &json!({"name": "Thin Border", "id": a})).unwrap();
        assert_eq!(r["replaced"], true);
        assert_eq!(s.doc().unwrap().doc.embroidery.styles.len(), 1);
        // A starter applies; unknown names and empty selections are refused.
        s.execute("embroidery.applyStyle", &json!({"style": "light-fill", "ids": [b]})).unwrap();
        assert_eq!(stitch(&s, &b).kind, StitchKind::Fill);
        assert!(s.execute("embroidery.applyStyle", &json!({"style": "nope"})).is_err());
        assert!(s.execute("embroidery.applyStyle", &json!({"style": "bold-border", "ids": []})).is_err());
    }

    #[test]
    fn styles_are_written_loaded_and_removed() {
        let mut s = session();
        let data = s.execute("embroidery.saveStyle", &json!({"style": "bold-border"})).unwrap()["data"].as_str().unwrap().to_string();
        assert!(data.contains(styles::FILE_FORMAT));
        // Its id is a starter's, but it's the same style: not added.
        assert_eq!(s.execute("embroidery.loadStyle", &json!({"data": data})).unwrap()["added"], false);
        let mine = data.replace("\"width\": 4.0", "\"width\": 6.0");
        let r = s.execute("embroidery.loadStyle", &json!({"data": mine})).unwrap();
        assert_eq!((r["id"].as_str(), r["added"].as_bool()), (Some("bold-border-2"), Some(true)));
        assert_eq!(s.execute("embroidery.loadStyle", &json!({"data": mine})).unwrap()["added"], false);
        assert!(s.execute("embroidery.loadStyle", &json!({"data": "{}"})).is_err());
        // Starters can't be removed; the document's can, as one undo step.
        assert!(s.execute("embroidery.removeStyle", &json!({"style": "bold-border"})).is_err());
        assert_eq!(s.execute("embroidery.removeStyle", &json!({"style": "bold-border-2"})).unwrap()["removed"], "bold-border-2");
        assert!(s.doc().unwrap().doc.embroidery.styles.is_empty());
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc().unwrap().doc.embroidery.styles.len(), 1);
    }

    #[test]
    fn a_style_naming_what_the_document_lacks_is_refused() {
        let mut s = session();
        s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 100, "height": 50})).unwrap();
        let file = json!({"format": styles::FILE_FORMAT, "version": 1, "name": "Starry", "stitch": {"kind": "motifFill", "motif": "my-stars"}});
        s.execute("embroidery.loadStyle", &json!({"data": file.to_string()})).unwrap();
        let e = s.execute("embroidery.applyStyle", &json!({"style": "starry"})).unwrap_err().to_string();
        assert!(e.contains("my-stars") && e.contains("loadMotif"), "{e}");
    }
}
