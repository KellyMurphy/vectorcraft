//! The embroidery font editor (E8.1c): a font document with one artboard per glyph
//! (`vectorcraft_embroidery::font_doc`), its details, metrics and kerning, a live sample line set in
//! the font being made, and saving it as a `.vcfont`.

use serde_json::{Value, json};
use vectorcraft_doc::{CharStyle, EmbroideryFont, KernPair, Node, NodeKind, StitchKind, StitchSettings, TextObject};
use vectorcraft_embroidery::{PT_MM, font_doc, fonts};

use super::embroidery_lib::library_entry;
use super::overprint::edit_counted;
use super::swatchlib::FileData;
use super::*;

/// The name of the sample line's type object.
pub const SAMPLE: &str = "Sample";
/// The id the font being edited has in its document's fonts, for the sample line.
fn sample_id(font: &EmbroideryFont) -> String {
    format!("editing-{}", if font.id.is_empty() { "font" } else { &font.id })
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "embroidery.font.new",
            "New Embroidery Font…",
            ["Window", "Embroidery"],
            None,
            "{name?: the font's name (\"Untitled Font\"), chars?: the characters to make with ranges written A-Z (\"A-Za-z0-9 .!?&-\"), from?: an installed font's family (its letters on a locked template layer to trace over, never part of the font), author?, license?} a new embroidery font document, in a new tab: one artboard per glyph on one baseline (an em is 100 mm; an artboard's width is the glyph's advance, its name the glyph's characters, `space` for the space), ruler guides at the ascender, cap height, x-height, baseline and descender, and a Letters layer to digitize on with the ordinary tools and stitch types → {index, glyphs}",
            always,
            new_font
        ),
        cmd!(
            "embroidery.font.open",
            "Open Embroidery Font…",
            ["Window", "Embroidery"],
            None,
            "{path? | data?: the file's text | library?: an embroidery library entry | font?: the id of one of the document's fonts} open an embroidery font for editing, as a font document in a new tab (see embroidery.font.new) → {index, glyphs}",
            always,
            open_font
        ),
        cmd!(
            query "embroidery.font.info",
            "Embroidery Font Info",
            ["Window", "Embroidery"],
            None,
            "{} the font document's font, as it would be saved → {font: {id, name, family, style, author, license, source, description, fontVersion, unitsPerEm, ascender, descender, capHeight, xHeight, sizes}, glyphs: [{chars, advance, objects}], empty: [chars of glyphs with nothing drawn], kerning: [{left, right, value}]}, or {font: null} for an ordinary document",
            has_doc,
            info
        ),
        cmd!(
            "embroidery.font.setup",
            "Embroidery Font Setup",
            ["Window", "Embroidery"],
            None,
            "{name?, family?, style?, author?, license?: SPDX id or text, source?: the font it was traced from (information only), description?, fontVersion?, ascender?, descender?, capHeight?, xHeight?: font units, sizes?: [smallest, largest] cap heights mm | null} the font document's details and metrics, one undo step → the same as embroidery.font.info",
            has_doc,
            setup
        ),
        cmd!(
            "embroidery.font.kern",
            "Embroidery Font Kerning",
            ["Window", "Embroidery"],
            None,
            "{left: characters, right: characters, value: font units (negative: closer) | null (no kerning for the pair)} set a kerning pair of the font document, one undo step → {kerning: [{left, right, value}]}",
            has_doc,
            kern
        ),
        cmd!(
            "embroidery.font.sample",
            "Embroidery Font Sample",
            ["Window", "Embroidery"],
            None,
            "{text?: the sample (\"Sample\"), size?: mm per em (40)} set a line of type in the font being made, below its artboards, sewn from its glyphs as they are now (the font, as embroidery.font.save would write it, goes into the document's fonts as editing-<id>; run again after changing glyphs). One undo step → {id (the type object), font}",
            has_doc,
            sample
        ),
        cmd!(
            "embroidery.font.save",
            "Save Embroidery Font…",
            ["Window", "Embroidery"],
            None,
            "{path?, user?: false (into the embroidery library's fonts folder)} write the font document's font as a .vcfont file (specified in docs/formats/vcfont.md) → {path} | {data: the file's text}, glyphs (count), empty: [chars of glyphs with nothing drawn], library? (with user)",
            has_doc,
            save
        ),
    ]
}

fn always(_: &Session) -> std::result::Result<(), String> {
    Ok(())
}

/// An installed font's outline of `chars` and its advance, em units with y up (none when it has no
/// such letter).
fn template_outline(family: &str, chars: &str) -> Option<font_doc::Traced> {
    const SIZE: f64 = 1000.0;
    let style = CharStyle { font_family: family.to_string(), size: SIZE, ..CharStyle::default() };
    let t = TextObject::point(kurbo::Point::ORIGIN, chars, style);
    let lay = vectorcraft_text::layout(vectorcraft_text::FontDb::global(), &t);
    let mut out = kurbo::BezPath::new();
    for g in &lay.glyphs {
        out.extend(g.outline.iter());
    }
    let advance = lay.glyphs.iter().map(|g| g.advance).sum::<f64>() / SIZE;
    (!out.elements().is_empty() || advance > 0.0).then(|| (kurbo::Affine::scale_non_uniform(1.0 / SIZE, -1.0 / SIZE) * out, advance))
}

fn new_font(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.font.new";
    let name = str_param(p, "name").map(str::trim).filter(|n| !n.is_empty()).unwrap_or("Untitled Font").to_string();
    let chars = font_doc::chars(str_param(p, "chars").unwrap_or("A-Za-z0-9 .,!?'&-"));
    if chars.is_empty() {
        return Err(bad(C, "`chars` names no characters"));
    }
    let from = str_param(p, "from").map(str::trim).filter(|f| !f.is_empty());
    if let Some(f) = from
        && !vectorcraft_text::FontDb::global().has_family(f)
    {
        return Err(bad(C, format!("no installed font `{f}` to trace (see text.fonts)")));
    }
    let font = EmbroideryFont {
        id: vectorcraft_embroidery::borders::slug(&name),
        name,
        author: str_param(p, "author").map(str::to_string),
        license: str_param(p, "license").map(str::to_string),
        source: from.map(|f| format!("traced from {f} (information only)")),
        ..EmbroideryFont::default()
    };
    let doc = font_doc::document(&font, &chars, &|c| from.and_then(|f| template_outline(f, c)));
    let glyphs = doc.artboards.len();
    let index = s.add_document(doc, None);
    Ok(json!({"index": index, "glyphs": glyphs}))
}

fn open_font(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.font.open";
    let font = if let Some(id) = str_param(p, "font") {
        s.doc()?
            .doc
            .embroidery
            .fonts
            .iter()
            .find(|f| f.id == id.trim() && f.unknown.is_none())
            .cloned()
            .ok_or_else(|| bad(C, format!("the document has no embroidery font `{id}`")))?
    } else if let Some((_, f)) = library_entry(&s.embroidery_libraries.fonts, p, C)? {
        (*f).clone()
    } else {
        let bytes = match (str_param(p, "path"), str_param(p, "data")) {
            (Some(path), _) => super::fileio::read_file(path)?,
            (None, Some(text)) => text.as_bytes().to_vec(),
            _ => return Err(bad(C, "give `path`, `data`, `library` or `font`")),
        };
        fonts::read_file(&bytes).map_err(|e| bad(C, e))?
    };
    let doc = font_doc::document(&font, &[], &|_| None);
    let glyphs = doc.artboards.len();
    let index = s.add_document(doc, None);
    Ok(json!({"index": index, "glyphs": glyphs}))
}

/// The font document's font as it would be saved.
fn built(s: &Session, cmd: &str) -> Result<EmbroideryFont> {
    font_doc::font(&s.doc()?.doc).map_err(|e| bad(cmd, e))
}

fn font_json(f: &EmbroideryFont) -> Value {
    json!({
        "id": f.id, "name": f.name, "family": f.family, "style": f.style, "author": f.author, "license": f.license,
        "source": f.source, "description": f.description, "fontVersion": f.version, "unitsPerEm": f.units_per_em,
        "ascender": f.ascender, "descender": f.descender, "capHeight": f.cap_height, "xHeight": f.x_height, "sizes": f.sizes,
    })
}

fn info(s: &mut Session, _: &Value) -> Result<Value> {
    if s.doc()?.doc.embroidery.font_editor.is_none() {
        return Ok(json!({"font": null}));
    }
    let f = built(s, "embroidery.font.info")?;
    let glyphs: Vec<Value> = f.glyphs.iter().map(|g| json!({"chars": g.chars, "advance": g.advance, "objects": g.objects.len()})).collect();
    let empty: Vec<&str> = f.glyphs.iter().filter(|g| g.objects.is_empty() && g.chars != " ").map(|g| g.chars.as_str()).collect();
    let kerning: Vec<Value> = f.kerning.iter().map(|k| json!({"left": k.left, "right": k.right, "value": k.value})).collect();
    Ok(json!({"font": font_json(&f), "glyphs": glyphs, "empty": empty, "kerning": kerning}))
}

/// The font document's font details, or an error for an ordinary document.
fn meta(s: &Session, cmd: &str) -> Result<EmbroideryFont> {
    s.doc()?
        .doc
        .embroidery
        .font_editor
        .as_deref()
        .cloned()
        .ok_or_else(|| bad(cmd, "this isn't an embroidery font document (embroidery.font.new makes one)"))
}

fn setup(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.font.setup";
    let mut f = meta(s, C)?;
    let text = |k: &str| p.get(k).map(|v| v.as_str().map(str::to_string));
    if let Some(Some(n)) = text("name") {
        let n = n.trim().to_string();
        if n.is_empty() {
            return Err(bad(C, "`name` can't be empty"));
        }
        f.id = vectorcraft_embroidery::borders::slug(&n);
        f.name = n;
    }
    for (k, field) in [
        ("family", &mut f.family),
        ("style", &mut f.style),
        ("author", &mut f.author),
        ("license", &mut f.license),
        ("source", &mut f.source),
        ("description", &mut f.description),
        ("fontVersion", &mut f.version),
    ] {
        if let Some(v) = text(k) {
            *field = v.filter(|t| !t.trim().is_empty());
        }
    }
    for (k, field) in [("ascender", &mut f.ascender), ("descender", &mut f.descender), ("capHeight", &mut f.cap_height), ("xHeight", &mut f.x_height)]
    {
        if let Some(v) = p.get(k) {
            *field = v.as_f64().filter(|x| x.is_finite()).ok_or_else(|| bad(C, format!("`{k}` must be a number of font units")))?;
        }
    }
    match p.get("sizes") {
        None => {}
        Some(Value::Null) => f.sizes = None,
        Some(v) => {
            let pair = v.as_array().and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?]));
            f.sizes = Some(pair.ok_or_else(|| bad(C, "`sizes` is [smallest, largest] in mm"))?);
        }
    }
    // Checked as a font (its metrics), its glyphs aside.
    let mut probe = f.clone();
    probe.glyphs = vec![vectorcraft_doc::FontGlyph { chars: "a".into(), ..Default::default() }];
    fonts::check(&probe).map_err(|e| bad(C, e))?;
    edit_counted(s, "Embroidery Font Setup", |d| {
        d.embroidery.font_editor = Some(Box::new(f));
        Ok(1)
    })?;
    info(s, &Value::Null)
}

fn kern(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.font.kern";
    let mut f = meta(s, C)?;
    let side = |k: &str| {
        str_param(p, k)
            .filter(|t| !t.is_empty() && t.chars().count() <= fonts::MAX_GLYPH_CHARS)
            .map(str::to_string)
            .ok_or_else(|| bad(C, format!("`{k}` must be a glyph's characters")))
    };
    let (left, right) = (side("left")?, side("right")?);
    f.kerning.retain(|k| !(k.left == left && k.right == right));
    match p.get("value") {
        Some(Value::Null) => {}
        Some(v) => {
            let value = v
                .as_f64()
                .filter(|x| x.is_finite() && x.abs() <= 10.0 * f.units_per_em)
                .ok_or_else(|| bad(C, "`value` must be font units within ten em"))?;
            f.kerning.push(KernPair { left, right, value });
        }
        None => return Err(bad(C, "`value` is missing (null clears the pair)")),
    }
    let kerning: Vec<Value> = f.kerning.iter().map(|k| json!({"left": k.left, "right": k.right, "value": k.value})).collect();
    edit_counted(s, "Embroidery Font Kerning", |d| {
        d.embroidery.font_editor = Some(Box::new(f));
        Ok(1)
    })?;
    Ok(json!({ "kerning": kerning }))
}

fn sample(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.font.sample";
    let mut f = built(s, C)?;
    let text = str_param(p, "text").filter(|t| !t.is_empty()).unwrap_or(SAMPLE).to_string();
    let em_mm = match p.get("size") {
        None | Some(Value::Null) => 40.0,
        Some(v) => v.as_f64().filter(|x| (2.0..=500.0).contains(x)).ok_or_else(|| bad(C, "`size` is mm per em, 2–500"))?,
    };
    f.id = sample_id(&f);
    let id = f.id.clone();
    let below = s.doc()?.doc.artboards.iter().map(|a| a.rect.y1).fold(0.0, f64::max) + em_mm / PT_MM;
    let node_id = s.edit("Embroidery Font Sample", |d, sel| {
        d.embroidery.fonts.retain(|x| x.id != id);
        d.embroidery.fonts.push(f.clone());
        // The sample line: the one there, its text replaced, else a new one below the artboards.
        let mut existing = None;
        d.walk(|n| {
            if existing.is_none() && n.name.as_deref() == Some(SAMPLE) && matches!(n.kind, NodeKind::Text(_)) {
                existing = Some(n.id);
            }
        });
        let style = CharStyle { size: em_mm / PT_MM, ..CharStyle::default() };
        let t = TextObject::point(kurbo::Point::new(0.0, below), &text, style);
        let mut settings = StitchSettings::new(StitchKind::Auto);
        settings.font = Some(id.clone());
        let nid = match existing {
            Some(nid) => {
                let n = d.node_mut(nid).ok_or(EngineError::NoNode(nid))?;
                n.kind = NodeKind::Text(Box::new(t));
                n.extras_mut().stitch = Some(Box::new(settings));
                nid
            }
            None => {
                let mut n = Node::new(d.alloc_id(), NodeKind::Text(Box::new(t)));
                n.name = Some(SAMPLE.into());
                n.extras_mut().stitch = Some(Box::new(settings));
                let nid = n.id;
                let layer = d.layers.iter().position(|l| l.name.as_deref() == Some(font_doc::LETTERS)).unwrap_or(d.layers.len().saturating_sub(1));
                let l = d.layers.get_mut(layer).ok_or_else(|| EngineError::Other("the document has no layer".into()))?;
                std::sync::Arc::make_mut(l).children_mut().ok_or_else(|| EngineError::Other("not a layer".into()))?.push(std::sync::Arc::new(n));
                nid
            }
        };
        sel.set([nid]);
        Ok(nid)
    })?;
    Ok(json!({"id": node_id.0, "font": id}))
}

fn save(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.font.save";
    let f = built(s, C)?;
    let text = fonts::write_file(&f).map_err(|e| bad(C, e))?;
    let empty: Vec<&str> = f.glyphs.iter().filter(|g| g.objects.is_empty() && g.chars != " ").map(|g| g.chars.as_str()).collect();
    let mut out = json!({"glyphs": f.glyphs.len(), "empty": empty});
    s.embroidery_libraries.fonts.write(p, &f.name, fonts::FILE_EXT, FileData::Text(text), &mut out, C)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_font_is_made_drawn_kerned_sampled_and_saved() {
        let mut s = Session::new();
        let r = s.execute("embroidery.font.new", &json!({"name": "Block", "chars": "IL", "license": "CC0-1.0"})).unwrap();
        assert_eq!(r["glyphs"], 2);
        let index = r["index"].as_u64().unwrap() as usize;
        assert!(s.set_active(index));
        // Draw an I: a stitched bar on its artboard.
        let ab = s.doc().unwrap().doc.artboards[0].rect;
        let base = font_doc::baseline(&s.doc().unwrap().doc.embroidery.font_editor.as_deref().cloned().unwrap());
        s.execute("shape.rectangle", &json!({"x": ab.x0 + 20.0, "y": base - 200.0, "width": 30, "height": 200})).unwrap();
        s.execute("embroidery.set", &json!({"type": "satinShape"})).unwrap();
        let i = s.execute("embroidery.font.info", &json!({})).unwrap();
        assert_eq!(i["glyphs"][0]["objects"], 1);
        assert_eq!(i["empty"], json!(["L"]));
        s.execute("embroidery.font.kern", &json!({"left": "L", "right": "I", "value": -40})).unwrap();
        s.execute("embroidery.font.setup", &json!({"author": "Me", "sizes": [6, 30]})).unwrap();
        // The sample line sews the I from the font being made.
        let r = s.execute("embroidery.font.sample", &json!({"text": "II"})).unwrap();
        assert_eq!(r["font"], "editing-block");
        let plan = s.execute("embroidery.info", &json!({})).unwrap();
        assert!(plan["design"]["stitches"].as_u64().unwrap() > 50, "{plan}");
        // Saved: the I with its bar 200 pt tall, as font units y up from the baseline.
        let saved = s.execute("embroidery.font.save", &json!({})).unwrap();
        let f = fonts::read_file(saved["data"].as_str().unwrap().as_bytes()).unwrap();
        assert_eq!((f.author.as_deref(), f.sizes, f.kern("L", "I")), (Some("Me"), Some([6.0, 30.0]), -40.0));
        let bar = fonts::outline(&f.glyph("I").unwrap().objects[0]).unwrap();
        let b = kurbo::Shape::bounding_box(&bar);
        let unit_pt = font_doc::EM_MM / PT_MM / f.units_per_em;
        assert!(b.y0.abs() < 1e-6 && (b.y1 - 200.0 / unit_pt).abs() < 1e-6, "{b:?}");
        // The sample line isn't part of the font.
        assert!(f.glyph("S").is_none());
        // And it opens again for editing.
        let r = s.execute("embroidery.font.open", &json!({ "data": saved["data"] })).unwrap();
        assert_eq!(r["glyphs"], 2);
    }

    #[test]
    fn ordinary_documents_are_not_font_documents() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        assert_eq!(s.execute("embroidery.font.info", &json!({})).unwrap()["font"], Value::Null);
        assert!(s.execute("embroidery.font.save", &json!({})).is_err());
        assert!(s.execute("embroidery.font.kern", &json!({"left": "A", "right": "V", "value": -10})).is_err());
        assert!(s.execute("embroidery.font.new", &json!({"from": "No Such Family"})).is_err());
    }
}
