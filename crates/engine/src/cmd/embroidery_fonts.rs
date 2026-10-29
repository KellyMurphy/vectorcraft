//! Embroidery fonts in a document (E8.1): `.vcfont` files loaded into it (from a file, data or the
//! embroidery library's `fonts/`), listed, saved and removed. The document keeps a copy of each font
//! it uses (`document.embroidery.fonts`), so its type sews the same where the font isn't installed.
//!
//! Ink/Stitch font folders (E8.1e) are converted when loaded (`vectorcraft_embroidery::inkstitch`):
//! from a folder given, or found in the library's `fonts/` or the font folders the user set
//! (`embroidery.fontFolders`). VectorCraft ships none; users install their own.

use serde_json::{Value, json};
use vectorcraft_doc::EmbroideryFont;
use vectorcraft_embroidery::fonts;

use super::embroidery_lib::library_entry;
use super::overprint::edit_counted;
use super::swatchlib::FileData;
use super::{CommandSpec, Result, Session, bad, bool_or, cmd, has_doc, str_param};

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
            "embroidery.importInkstitchFont",
            "Import Ink/Stitch Font…",
            ["Window", "Embroidery"],
            None,
            "{path: an Ink/Stitch font folder (holding font.json and ltr.svg or an ltr folder of glyph SVGs), save?: path of a .vcfont file to write it to, user?: false (write it into the embroidery library's fonts folder too)} convert an Ink/Stitch embroidery font (from Ink/Stitch's public documentation: GlyphLayer-… layers on a baseline guide, inkstitch: attributes mapped onto VectorCraft's stitch types by meaning) and load it into the document, as one undo step. Each font has its own licence (read its LICENSE): it is carried into the font → {id, name, glyphs, added, license, report: {skipped: [layers left out, with why], unmapped: [Ink/Stitch attributes not mapped yet], commands (visual commands found, not mapped yet), baseline (how it was found)}, path?, library?}",
            has_doc,
            import_inkstitch
        ),
        cmd!(
            "embroidery.fontFolders",
            "Embroidery Font Folders…",
            ["Window", "Embroidery"],
            None,
            "{folders?: [paths] (left out: only list)} the folders VectorCraft looks in for Ink/Stitch embroidery fonts besides the embroidery library's fonts folder: each folder that holds a font.json is a font, else each of its subfolders that does (an Ink/Stitch install's fonts folder, a folder of downloaded fonts). Fonts are read where they are, never copied; the list is kept with the embroidery library → {folders, fonts: [{id (inkstitch/<folder>), name, path}], skipped: [{path, error}]}",
            always,
            font_folders
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
    // An Ink/Stitch font folder found in the library or a font folder.
    if let Some(ink) = str_param(p, "library").and_then(|k| s.embroidery_libraries.inkstitch_font(k.trim())).cloned() {
        let (font, report) = read_inkstitch(&ink.path, C)?;
        let mut out = add_font(s, font, "Load Embroidery Font", C)?;
        out["report"] = report_json(&report);
        return Ok(out);
    }
    let font = match library_entry(&s.embroidery_libraries.fonts, p, C)? {
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
    add_font(s, font, "Load Embroidery Font", C)
}

/// Add `font` to the document as undo step `label`: under its id, else (the id taken by a different
/// font) a numbered one; the very same font already there isn't added again.
fn add_font(s: &mut Session, mut font: EmbroideryFont, label: &str, cmd: &str) -> Result<Value> {
    let mine = &s.doc()?.doc.embroidery.fonts;
    if mine.contains(&font) {
        return Ok(json!({"id": font.id, "name": font.name, "glyphs": font.glyphs.len(), "license": font.license, "added": false}));
    }
    let taken = |id: &str| mine.iter().any(|f| f.id == id);
    if taken(&font.id) {
        let base = font.id.clone();
        font.id =
            (2..10_000).map(|n| format!("{base}-{n}")).find(|id| !taken(id)).ok_or_else(|| bad(cmd, format!("too many fonts named `{base}`")))?;
    }
    let out = json!({"id": font.id, "name": font.name, "glyphs": font.glyphs.len(), "license": font.license, "added": true});
    edit_counted(s, label, |d| {
        d.embroidery.fonts.push(font);
        Ok(1)
    })?;
    Ok(out)
}

fn report_json(r: &vectorcraft_embroidery::inkstitch::Report) -> Value {
    json!({"glyphs": r.glyphs, "skipped": r.skipped, "unmapped": r.unmapped, "commands": r.commands, "baseline": r.baseline})
}

/// Read Ink/Stitch font folder `dir` (`font.json`, `ltr.svg` or the `ltr` folder's SVGs, the
/// `LICENSE`) and convert it.
fn read_inkstitch(dir: &str, cmd: &str) -> Result<(EmbroideryFont, vectorcraft_embroidery::inkstitch::Report)> {
    use vectorcraft_embroidery::inkstitch;
    let d = std::path::Path::new(dir);
    let at = |name: &str| d.join(name).to_string_lossy().to_string();
    let json =
        super::fileio::read_file(&at("font.json")).map_err(|e| bad(cmd, format!("{dir} isn't an Ink/Stitch font folder (no font.json: {e})")))?;
    let mut svgs: Vec<(String, Vec<u8>)> = vec![];
    if d.join("ltr.svg").is_file() {
        svgs.push(("ltr.svg".into(), super::fileio::read_file(&at("ltr.svg"))?));
    } else if d.join("ltr").is_dir() {
        crate::file_access::check_read(&at("ltr")).map_err(|e| bad(cmd, e))?;
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(d.join("ltr"))
            .map_err(|e| bad(cmd, e.to_string()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("svg")))
            .collect();
        files.sort();
        if files.len() > inkstitch::MAX_SVGS {
            return Err(bad(cmd, format!("a font is read from at most {} glyph files", inkstitch::MAX_SVGS)));
        }
        for f in files {
            let name = f.file_name().map_or_else(String::new, |n| n.to_string_lossy().to_string());
            svgs.push((name, super::fileio::read_file(&f.to_string_lossy())?));
        }
    }
    let license = ["LICENSE", "LICENSE.txt", "LICENSE.md", "license.txt"]
        .iter()
        .find(|n| d.join(n).is_file())
        .and_then(|n| super::fileio::read_file(&at(n)).ok())
        .map(|b| String::from_utf8_lossy(&b).into_owned());
    let folder = d.file_name().map_or_else(|| dir.to_string(), |n| n.to_string_lossy().to_string());
    let src = inkstitch::Source {
        name: &folder,
        json: &json,
        svgs: svgs.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect(),
        license: license.as_deref(),
    };
    inkstitch::import(&src).map_err(|e| bad(cmd, format!("{folder}: {e}")))
}

fn import_inkstitch(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.importInkstitchFont";
    let dir = str_param(p, "path").ok_or_else(|| bad(C, "`path` (an Ink/Stitch font folder) is missing"))?;
    let (font, report) = read_inkstitch(dir, C)?;
    let mut out = add_font(s, font.clone(), "Import Ink/Stitch Font", C)?;
    out["report"] = report_json(&report);
    let text = fonts::write_file(&font).map_err(|e| bad(C, e))?;
    if let Some(path) = str_param(p, "save") {
        super::fileio::write_file(path, text.as_bytes())?;
        out["path"] = json!(path);
    }
    if bool_or(p, "user", false) {
        let mut lib = json!({});
        s.embroidery_libraries.fonts.write(&json!({"user": true}), &font.name, fonts::FILE_EXT, FileData::Text(text), &mut lib, C)?;
        out["library"] = lib["library"].clone();
    }
    Ok(out)
}

fn always(_: &Session) -> std::result::Result<(), String> {
    Ok(())
}

fn font_folders(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.fontFolders";
    match p.get("folders") {
        None | Some(Value::Null) => {}
        Some(Value::Array(a)) => {
            let folders: Vec<String> =
                a.iter().map(|v| v.as_str().map(str::to_string).ok_or_else(|| bad(C, "`folders` must be a list of paths"))).collect::<Result<_>>()?;
            for f in &folders {
                crate::file_access::check_read(f).map_err(|e| bad(C, e))?;
            }
            s.embroidery_libraries.set_font_folders(folders).map_err(|e| bad(C, e))?;
        }
        Some(_) => return Err(bad(C, "`folders` must be a list of paths")),
    }
    let l = &s.embroidery_libraries;
    let fonts: Vec<Value> = l.inkstitch.iter().map(|f| json!({"id": f.id, "name": f.name, "path": f.path})).collect();
    let skipped: Vec<Value> = l.skipped(Some("fonts")).into_iter().filter(|v| v["path"].as_str().is_some_and(|p| !p.ends_with(".vcfont"))).collect();
    Ok(json!({"folders": l.font_folders(), "fonts": fonts, "skipped": skipped}))
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

    /// An Ink/Stitch font folder written for the test from the public documentation.
    fn ink_folder(root: &std::path::Path, name: &str) -> std::path::PathBuf {
        let d = root.join(name);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("font.json"), r#"{"name": "Little Block", "size": 100, "horiz_adv_x_default": 60}"#).unwrap();
        std::fs::write(
            d.join("ltr.svg"),
            r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" viewBox="0 0 300 200">
  <sodipodi:guide position="0,150" inkscape:label="baseline"/>
  <g inkscape:groupmode="layer" inkscape:label="GlyphLayer-I"><path d="M 10 50 L 30 50 L 30 150 L 10 150 Z" style="fill:#000"/></g>
  <g inkscape:groupmode="layer" inkscape:label="GlyphLayer-H"><path d="M 0 50 L 15 50 L 15 150 L 0 150 Z M 35 50 L 50 50 L 50 150 L 35 150 Z M 15 90 L 35 90 L 35 105 L 15 105 Z" style="fill:#000"/></g>
</svg>"##,
        )
        .unwrap();
        std::fs::write(d.join("LICENSE"), "Made for the VectorCraft tests. CC0-1.0").unwrap();
        d
    }

    #[test]
    fn ink_stitch_fonts_import_and_install_by_folder() {
        let root = std::env::temp_dir().join(format!("vc-ink-fonts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let folder = ink_folder(&root.join("downloads"), "little_block");
        let mut s = session();
        // Imported from the folder.
        let r = s.execute("embroidery.importInkstitchFont", &json!({"path": folder.to_string_lossy()})).unwrap();
        assert_eq!((r["name"].as_str(), r["glyphs"].as_u64(), r["added"].as_bool()), (Some("Little Block"), Some(2), Some(true)), "{r}");
        assert_eq!(r["report"]["baseline"].as_str().map(|b| b.contains("guide")), Some(true));
        // Installed by pointing VectorCraft at the folder holding it: listed, loaded by name.
        s.embroidery_libraries.set_dir(Some(root.join("Embroidery").to_string_lossy().to_string()));
        let f = s.execute("embroidery.fontFolders", &json!({"folders": [root.join("downloads").to_string_lossy()]})).unwrap();
        assert_eq!(f["fonts"][0]["id"], "inkstitch/little-block", "{f}");
        let libs = s.execute("embroidery.libraries", &json!({"kind": "fonts"})).unwrap();
        assert!(libs["libraries"].as_array().unwrap().iter().any(|e| e["format"] == "inkstitch"));
        let r = s.execute("embroidery.loadFont", &json!({"library": "Little Block"})).unwrap();
        assert_eq!(r["added"], false, "the same font as imported");
        // Kept with the library: a new session finds the folder again.
        let mut again = session();
        again.embroidery_libraries.set_dir(Some(root.join("Embroidery").to_string_lossy().to_string()));
        assert_eq!(again.embroidery_libraries.inkstitch.len(), 1);
        // Or put in the library's fonts folder.
        ink_folder(&root.join("Embroidery").join("fonts"), "second_block");
        let f = again.execute("embroidery.rescanLibraries", &json!({})).unwrap();
        assert_eq!(f["libraries"].as_array().unwrap().iter().filter(|e| e["format"] == "inkstitch").count(), 2, "{f}");
        assert!(s.execute("embroidery.importInkstitchFont", &json!({"path": root.to_string_lossy()})).is_err(), "not a font folder");
        let _ = std::fs::remove_dir_all(root);
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
