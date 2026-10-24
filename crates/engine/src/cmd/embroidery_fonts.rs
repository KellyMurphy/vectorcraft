//! Embroidery fonts in a document (E8.1): `.vcfont` files loaded into it (from a file, data or the
//! embroidery library's `fonts/`), listed, saved and removed. The document keeps a copy of each font
//! it uses (`document.embroidery.fonts`), so its type sews the same where the font isn't installed.

use serde_json::{Value, json};
use vectorcraft_doc::EmbroideryFont;
use vectorcraft_embroidery::fonts;

use super::embroidery_lib::library_entry;
use super::overprint::edit_counted;
use super::swatchlib::FileData;
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "embroidery.fonts",
            "Embroidery Fonts",
            ["Window", "Embroidery"],
            None,
            "{} the embroidery fonts loaded into the document (the library's are listed by embroidery.libraries {kind: fonts}) → {fonts: [{id, name, family, style, author, license, source, description, fontVersion, sizes: [smallest, largest] cap heights mm | null, glyphs (count), chars (the single characters it sets, in order), supported (false: from a newer VectorCraft, kept but not usable; only id and name are given)}]}",
            has_doc,
            list
        ),
        cmd!(
            "embroidery.loadFont",
            "Load Embroidery Font…",
            ["Window", "Embroidery"],
            None,
            "{path? | data?: the file's text | library?: an embroidery library entry (embroidery.libraries, kind fonts: its id or name)} load a .vcfont embroidery font (specified in docs/formats/vcfont.md) into the document, as one undo step. Its id is the file's; one the document already uses for a different font gets a number (script-2…), and a font the document already has is not added again → {id, name, glyphs, added}",
            has_doc,
            load
        ),
        cmd!(
            "embroidery.saveFont",
            "Save Embroidery Font…",
            ["Window", "Embroidery"],
            None,
            "{font: id (one of the document's), path?, user?: false} write an embroidery font as a .vcfont file → {path}, or without path {data: the file's text} (`user: true`: into the embroidery library folder instead → {path, library: its id})",
            has_doc,
            save
        ),
        cmd!(
            "embroidery.removeFont",
            "Remove Embroidery Font",
            ["Window", "Embroidery"],
            None,
            "{font: id} remove an embroidery font from the document, as one undo step → {removed: id}",
            has_doc,
            remove
        ),
    ]
}

/// The single characters `f` sets, in glyph order.
fn chars(f: &EmbroideryFont) -> String {
    f.glyphs.iter().filter(|g| g.chars.chars().count() == 1).map(|g| g.chars.as_str()).collect()
}

fn list(s: &mut Session, _: &Value) -> Result<Value> {
    let fonts: Vec<Value> = s
        .doc()?
        .doc
        .embroidery
        .fonts
        .iter()
        .map(|f| {
            if f.unknown.is_some() {
                return json!({"id": f.id, "name": f.name, "supported": false});
            }
            json!({
                "id": f.id, "name": f.name, "family": f.family, "style": f.style, "author": f.author, "license": f.license,
                "source": f.source, "description": f.description, "fontVersion": f.version, "sizes": f.sizes,
                "glyphs": f.glyphs.len(), "chars": chars(f), "supported": true,
            })
        })
        .collect();
    Ok(json!({ "fonts": fonts }))
}

fn load(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.loadFont";
    let mut font = match library_entry(&s.embroidery_libraries.fonts, p, C)? {
        Some((_, f)) => (*f).clone(),
        None => {
            let bytes = match (str_param(p, "path"), str_param(p, "data")) {
                (Some(path), _) => super::fileio::read_file(path)?,
                (None, Some(text)) => text.as_bytes().to_vec(),
                _ => return Err(bad(C, "give `path`, `data` or `library` (see embroidery.libraries)")),
            };
            fonts::read_file(&bytes).map_err(|e| bad(C, e))?
        }
    };
    let mine = &s.doc()?.doc.embroidery.fonts;
    if mine.contains(&font) {
        return Ok(json!({"id": font.id, "name": font.name, "glyphs": font.glyphs.len(), "added": false}));
    }
    let taken = |id: &str| mine.iter().any(|f| f.id == id);
    if taken(&font.id) {
        let base = font.id.clone();
        font.id = (2..10_000).map(|n| format!("{base}-{n}")).find(|id| !taken(id)).ok_or_else(|| bad(C, format!("too many fonts named `{base}`")))?;
    }
    let out = json!({"id": font.id, "name": font.name, "glyphs": font.glyphs.len(), "added": true});
    edit_counted(s, "Load Embroidery Font", |d| {
        d.embroidery.fonts.push(font);
        Ok(1)
    })?;
    Ok(out)
}

/// The document's font `p.font`.
fn font_param(s: &Session, p: &Value, cmd: &str) -> Result<EmbroideryFont> {
    let id = str_param(p, "font").map(str::trim).ok_or_else(|| bad(cmd, "`font` (a font's id, see embroidery.fonts) is missing"))?;
    s.doc()?
        .doc
        .embroidery
        .fonts
        .iter()
        .find(|f| f.id == id && f.unknown.is_none())
        .cloned()
        .ok_or_else(|| bad(cmd, format!("the document has no embroidery font `{id}`")))
}

fn save(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.saveFont";
    let font = font_param(s, p, C)?;
    let text = fonts::write_file(&font).map_err(|e| bad(C, e))?;
    let mut out = json!({});
    s.embroidery_libraries.fonts.write(p, &font.name, fonts::FILE_EXT, FileData::Text(text), &mut out, C)?;
    Ok(out)
}

fn remove(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.removeFont";
    let id = str_param(p, "font").map(str::trim).ok_or_else(|| bad(C, "`font` (a font's id) is missing"))?.to_string();
    if !s.doc()?.doc.embroidery.fonts.iter().any(|f| f.id == id) {
        return Err(bad(C, format!("the document has no embroidery font `{id}`")));
    }
    edit_counted(s, "Remove Embroidery Font", |d| {
        d.embroidery.fonts.retain(|f| f.id != id);
        Ok(1)
    })?;
    Ok(json!({ "removed": id }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        s
    }

    #[test]
    fn fonts_load_list_save_and_remove() {
        let mut s = session();
        let data = fonts::write_file(&fonts::sample()).unwrap();
        let r = s.execute("embroidery.loadFont", &json!({ "data": data })).unwrap();
        assert_eq!((r["id"].as_str(), r["added"].as_bool()), (Some("vectorcraft-sample-sans"), Some(true)));
        // The same font again: not added twice.
        let r = s.execute("embroidery.loadFont", &json!({ "data": data })).unwrap();
        assert_eq!(r["added"], false);
        // A different font under the same id gets a number.
        let mut other = fonts::sample();
        other.glyphs.truncate(3);
        let r = s.execute("embroidery.loadFont", &json!({ "data": fonts::write_file(&other).unwrap() })).unwrap();
        assert_eq!(r["id"], "vectorcraft-sample-sans-2");
        let list = s.execute("embroidery.fonts", &json!({})).unwrap();
        assert_eq!(list["fonts"].as_array().unwrap().len(), 2);
        assert_eq!(list["fonts"][0]["chars"], " ILTHEFO-");
        // Saved back, the same font.
        let saved = s.execute("embroidery.saveFont", &json!({"font": "vectorcraft-sample-sans"})).unwrap();
        assert_eq!(fonts::read_file(saved["data"].as_str().unwrap().as_bytes()).unwrap(), fonts::sample());
        s.execute("embroidery.removeFont", &json!({"font": "vectorcraft-sample-sans-2"})).unwrap();
        assert_eq!(s.doc().unwrap().doc.embroidery.fonts.len(), 1);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc().unwrap().doc.embroidery.fonts.len(), 2);
        assert!(s.execute("embroidery.saveFont", &json!({"font": "nope"})).is_err());
        assert!(s.execute("embroidery.loadFont", &json!({"data": "{}"})).is_err());
    }

    #[test]
    fn fonts_go_through_the_library_folder() {
        let dir = std::env::temp_dir().join(format!("vc-emb-fonts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = session();
        s.embroidery_libraries.set_dir(Some(dir.to_string_lossy().to_string()));
        let data = fonts::write_file(&fonts::sample()).unwrap();
        let r = s.execute("embroidery.addToLibrary", &json!({"data": data, "name": "Sample.vcfont"})).unwrap();
        assert_eq!((r["kind"].as_str(), r["id"].as_str()), (Some("fonts"), Some("user/Sample.vcfont")));
        let libs = s.execute("embroidery.libraries", &json!({"kind": "fonts"})).unwrap();
        assert_eq!(libs["libraries"][0]["font"], "vectorcraft-sample-sans");
        let r = s.execute("embroidery.loadFont", &json!({"library": "VectorCraft Sample Sans"})).unwrap();
        assert_eq!(r["added"], true);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn type_is_set_in_a_loaded_font() {
        let mut s = session();
        s.execute("text.create", &json!({"x": 10, "y": 50, "text": "HELLO", "size": 36})).unwrap();
        assert!(
            s.execute("embroidery.set", &json!({"type": "auto", "font": "vectorcraft-sample-sans"}))
                .unwrap_err()
                .to_string()
                .contains("none is loaded")
        );
        s.execute("embroidery.loadFont", &json!({ "data": fonts::write_file(&fonts::sample()).unwrap() })).unwrap();
        s.execute("embroidery.set", &json!({"type": "auto", "font": "vectorcraft-sample-sans"})).unwrap();
        let info = s.execute("embroidery.info", &json!({})).unwrap();
        assert_eq!(info["selection"]["font"], "vectorcraft-sample-sans");
        assert!(info["design"]["stitches"].as_u64().unwrap() > 100, "{info}");
        s.execute("embroidery.set", &json!({"font": null})).unwrap();
        assert_eq!(s.execute("embroidery.info", &json!({})).unwrap()["selection"]["font"], Value::Null);
    }

    #[test]
    fn documents_keep_their_fonts() {
        let mut s = session();
        s.execute("embroidery.loadFont", &json!({ "data": fonts::write_file(&fonts::sample()).unwrap() })).unwrap();
        let doc = &s.doc().unwrap().doc;
        let bytes = vectorcraft_format::save(doc, false);
        let back = vectorcraft_format::load(&bytes).unwrap();
        assert_eq!(back.embroidery.fonts, vec![fonts::sample()]);
    }
}
