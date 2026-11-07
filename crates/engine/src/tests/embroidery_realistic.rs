//! The Realistic view (E9.2): the fabric's look in the document, and the rendered preview.

use serde_json::{Value, json};

use crate::*;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

/// A small badge: a tatami circle, a satin ring round it, a star in satin shape and a running line.
fn badge() -> Session {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 200, "height": 200}));
    run(&mut s, "paint.setFill", json!({"color": "#2050a0"}));
    run(&mut s, "shape.ellipse", json!({"x": 30, "y": 30, "width": 140, "height": 140}));
    run(&mut s, "embroidery.set", json!({"type": "fill"}));
    run(&mut s, "select.none", json!({}));
    run(&mut s, "paint.setFill", json!({"none": true}));
    run(&mut s, "paint.setStroke", json!({"color": "#e0b020"}));
    run(&mut s, "shape.ellipse", json!({"x": 30, "y": 30, "width": 140, "height": 140}));
    run(&mut s, "embroidery.set", json!({"type": "satin", "width": 4}));
    run(&mut s, "select.none", json!({}));
    run(&mut s, "paint.setStroke", json!({"none": true}));
    run(&mut s, "paint.setFill", json!({"color": "#f0f0f0"}));
    run(&mut s, "shape.star", json!({"cx": 100, "cy": 100, "radius1": 45, "radius2": 20, "points": 5}));
    run(&mut s, "embroidery.set", json!({"type": "satinShape"}));
    s
}

fn png(r: &Value) -> Vec<u8> {
    vectorcraft_format::base64_decode(r["pngBase64"].as_str().unwrap()).unwrap()
}

#[test]
fn the_realistic_preview_renders_the_design_on_the_documents_fabric() {
    let mut s = badge();
    let flat = run(&mut s, "embroidery.preview", json!({"scale": 2}));
    let real = run(&mut s, "embroidery.preview", json!({"scale": 2, "style": "realistic"}));
    assert_eq!((real["width"].as_u64(), real["height"].as_u64()), (Some(400), Some(400)));
    assert_ne!(png(&flat), png(&real));
    if let Some(dir) = std::env::var_os("EMB_DUMP") {
        std::fs::write(std::path::Path::new(&dir).join("realistic-plain.png"), png(&real)).unwrap();
    }
    let pixel = |r: &Value, x: u32, y: u32| {
        let img = image::load_from_memory(&png(r)).unwrap().to_rgba8();
        img.get_pixel(x, y).0
    };
    // A corner of the artboard is bare fabric: undyed linen until a colour is set.
    let corner = pixel(&real, 6, 6);
    assert!(corner[0] > 180 && corner[3] == 255, "{corner:?}");

    let r = run(&mut s, "embroidery.setup", json!({"fabricTexture": "knit", "fabricColor": "#1e2a44"}));
    assert_eq!((r["fabricTexture"].as_str(), r["fabricColor"].as_str()), (Some("knit"), Some("#1e2a44")));
    let navy = run(&mut s, "embroidery.preview", json!({"scale": 2, "style": "realistic"}));
    let corner = pixel(&navy, 6, 6);
    assert!(corner[2] > corner[0] && corner[0] < 60, "navy knit: {corner:?}");
    if let Some(dir) = std::env::var_os("EMB_DUMP") {
        std::fs::write(std::path::Path::new(&dir).join("realistic-navy-knit.png"), png(&navy)).unwrap();
        for t in ["twill", "denim", "terry", "leather", "felt"] {
            run(&mut s, "embroidery.setup", json!({"fabricTexture": t, "fabricColor": null}));
            let r = run(&mut s, "embroidery.preview", json!({"scale": 4, "style": "realistic"}));
            std::fs::write(std::path::Path::new(&dir).join(format!("realistic-{t}.png")), png(&r)).unwrap();
        }
        run(&mut s, "embroidery.setup", json!({"fabricTexture": "knit", "fabricColor": "#1e2a44"}));
    }

    // Saved with the document, and one undo step.
    let saved = vectorcraft_format::save(&s.doc().unwrap().doc, false);
    let back = vectorcraft_format::load(&saved).unwrap();
    assert_eq!(back.embroidery.preview, s.doc().unwrap().doc.embroidery.preview);
    run(&mut s, "edit.undo", json!({}));
    assert_ne!(run(&mut s, "embroidery.setup", json!({}))["fabricColor"], "#1e2a44");
    // Back to the fabric profile's texture.
    run(&mut s, "embroidery.setFabric", json!({"fabric": "denim"}));
    let r = run(&mut s, "embroidery.setup", json!({"fabricTexture": null, "fabricColor": null}));
    assert_eq!((r["fabricTexture"].as_str(), r["fabricColor"].as_str()), (Some("denim"), Some("#3a5480")));
    assert!(s.doc().unwrap().doc.embroidery.preview.is_none(), "nothing set: nothing kept");

    for bad in [json!({"fabricTexture": "silk"}), json!({"fabricColor": "navy"}), json!({"fabricColor": 3})] {
        assert!(s.execute("embroidery.setup", &bad).is_err(), "{bad}");
    }
    assert!(s.execute("embroidery.preview", &json!({"style": "glossy"})).is_err());
}

#[test]
fn a_look_from_a_newer_version_is_kept_and_drawn_as_unset() {
    let mut d = vectorcraft_doc::Document::new(100.0, 100.0);
    d.embroidery.preview = serde_json::from_value(json!({"texture": "velvet", "color": 7, "sheen": 2})).unwrap();
    let look = vectorcraft_embroidery::realistic::Look::of(&d);
    assert_eq!(look.texture, vectorcraft_embroidery::realistic::Texture::Plain);
    let p = d.embroidery.preview.as_deref().unwrap();
    assert_eq!(p.texture.as_deref(), Some("velvet"));
    assert!(p.color.is_none() && p.extra.contains_key("sheen"));
}

#[test]
fn a_garment_mockup_opens_at_real_size_with_the_design_on_it() {
    let mut s = badge();
    let source = s.active_index();
    let r = run(&mut s, "embroidery.mockup", json!({"garment": "polo", "width": 600}));
    assert_eq!((r["garment"].as_str(), r["placement"].as_str()), (Some("polo"), Some("leftChest")));
    // The badge is 140 pt (49.4 mm) plus the satin's width: within a left chest's usual 100 mm.
    let size = r["designSize"].as_array().unwrap();
    assert!((50.0..60.0).contains(&size[0].as_f64().unwrap()), "{r}");
    assert_eq!(r["warnings"], json!([]));
    // A new document holding the picture, as wide as the garment shown (mm).
    let index = r["index"].as_u64().unwrap() as usize;
    assert_ne!(Some(index), source);
    let d = &s.doc().unwrap().doc;
    let ab = d.artboards[0].rect;
    let w_mm = ab.width() * vectorcraft_embroidery::PT_MM;
    assert!((700.0..900.0).contains(&w_mm), "a polo shirt is about 800 mm across the sleeves: {w_mm}");
    assert_eq!(d.images.len(), 1);
    assert!(d.title.contains("Polo Shirt"), "{}", d.title);
}

#[test]
fn a_mockup_is_written_as_a_png_and_bad_choices_are_refused() {
    let mut s = badge();
    let r = run(&mut s, "embroidery.mockup", json!({"garment": "cap", "view": "design", "width": 300, "format": "png", "fabricColor": "#b02020"}));
    let img = image::load_from_memory(&png(&r)).unwrap().to_rgba8();
    assert_eq!(img.width(), 300);
    // The badge (52 mm across) is close to a cap front's 57 mm height; the corner is red cap.
    let c = img.get_pixel(2, img.height() / 2).0;
    assert!(c[0] > c[1] + 40, "red cap: {c:?}");
    assert!(r.get("index").is_none(), "no new document with format png");
    for bad in [
        json!({"garment": "kilt"}),
        json!({"garment": "cap", "placement": "leftChest"}),
        json!({"view": "back"}),
        json!({"width": 10}),
        json!({"fabricColor": "red"}),
        json!({"format": "jpeg"}),
    ] {
        let e = s.execute("embroidery.mockup", &bad).unwrap_err().to_string();
        assert!(e.contains("embroidery.mockup"), "{bad}: {e}");
    }
    // A design too big for where it goes is still drawn, with a warning.
    run(&mut s, "select.all", json!({}));
    run(&mut s, "object.scale", json!({"sx": 250, "sy": 250}));
    let r = run(&mut s, "embroidery.mockup", json!({"garment": "tshirt", "width": 300, "format": "png"}));
    assert_eq!(r["warnings"].as_array().map(Vec::len), Some(1), "{r}");
}
