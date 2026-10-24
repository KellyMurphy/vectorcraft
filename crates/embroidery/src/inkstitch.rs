//! Ink/Stitch embroidery fonts (E8.1e), opened as embroidery fonts so their lettering can be used
//! and converted to `.vcfont`. VectorCraft ships none: each user installs the ones they want.
//!
//! The format is taken from Ink/Stitch's public documentation only (cited in
//! `docs/formats/inkstitch-fonts.md`): never from its source code (GPL), never by running it.
//! A font is a folder holding `font.json` (its metrics: the reference letter's `size`,
//! `horiz_adv_x_default`, per-glyph `horiz_adv_x`, `hkern` pairs where a positive value sets a pair
//! closer), glyph SVGs (`ltr.svg`, or several in an `ltr` folder) and a `LICENSE`. Each glyph is an
//! SVG layer labelled `GlyphLayer-<character>`, placed against a guide labelled `baseline`; stitch
//! settings are attributes in the `http://inkstitch.org/namespace` namespace on each path. Where the
//! documentation leaves something open (which way the baseline guide's position runs, how `hkern`
//! is laid out), the reader accepts each reasonable reading and keeps the one the font itself bears
//! out, and the import report says what it couldn't map rather than guessing.
//!
//! Stitches are always made by VectorCraft's own stitch types: an imported font sews by the meaning
//! of its settings, not needle for needle as Ink/Stitch would.

use std::collections::{BTreeMap, BTreeSet};

use kurbo::{Affine, BezPath, Shape};
use serde_json::Value;
use vectorcraft_doc::{EmbroideryFont, FontGlyph, GlyphObject, KernPair, StitchKind, StitchSettings, Underlay};

use crate::fonts;

/// The namespace of Ink/Stitch's attributes.
pub const NAMESPACE: &str = "http://inkstitch.org/namespace";
const INKSCAPE: &str = "http://www.inkscape.org/namespaces/inkscape";
const SODIPODI: &str = "http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd";
/// The Ink/Stitch attributes the reader maps (or accepts), as `docs/formats/inkstitch-fonts.md` lists
/// them.
pub const MAPPED: [&str; 17] = [
    "satin_column",
    "zigzag_spacing_mm",
    "pull_compensation_mm",
    "center_walk_underlay",
    "contour_underlay",
    "zigzag_underlay",
    "manual_stitch",
    "auto_fill",
    "angle",
    "row_spacing_mm",
    "max_stitch_length_mm",
    "staggers",
    "fill_underlay",
    "expand_mm",
    "running_stitch_length_mm",
    "bean_stitch_repeats",
    "repeats",
];
/// The largest glyph SVG read, bytes.
pub const MAX_SVG_BYTES: usize = 64 << 20;
/// The most glyph SVGs a font is read from.
pub const MAX_SVGS: usize = 64;
/// The longest licence text kept, bytes.
const MAX_LICENSE: usize = 8 << 10;

/// What an import read, and what it couldn't.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub glyphs: usize,
    /// Glyph layers left out, each with why.
    pub skipped: Vec<String>,
    /// Ink/Stitch attributes found that VectorCraft doesn't map (the objects sew with its defaults
    /// for them).
    pub unmapped: BTreeSet<String>,
    /// Visual commands (stops, trims, fill start and end, ignore) found: not mapped yet.
    pub commands: usize,
    /// How the baseline was found.
    pub baseline: String,
}

/// A font folder's files, read by the caller.
pub struct Source<'a> {
    /// The folder's name (the font's name when `font.json` has none).
    pub name: &'a str,
    pub json: &'a [u8],
    /// The left-to-right glyph SVGs: `ltr.svg`, or those of the `ltr` folder, in name order.
    pub svgs: Vec<(&'a str, &'a [u8])>,
    pub license: Option<&'a str>,
}

/// A glyph as found in a layer: its characters, its objects (paths in SVG space) and their settings.
struct Found {
    chars: String,
    objects: Vec<(BezPath, StitchSettings, bool)>,
}

/// Read an Ink/Stitch font folder as an embroidery font.
pub fn import(src: &Source) -> Result<(EmbroideryFont, Report), String> {
    let mut report = Report::default();
    let json: Value = serde_json::from_slice(src.json).map_err(|e| format!("font.json isn't JSON: {e}"))?;
    if src.svgs.is_empty() {
        return Err("the folder has no ltr.svg (or ltr folder) of glyphs".into());
    }
    if src.svgs.len() > MAX_SVGS {
        return Err(format!("a font is read from at most {MAX_SVGS} glyph files"));
    }
    let mut found: Vec<Found> = vec![];
    let mut guides: Vec<(f64, f64)> = vec![];
    for (file, bytes) in &src.svgs {
        if bytes.len() > MAX_SVG_BYTES {
            return Err(format!("{file} is over {} MB", MAX_SVG_BYTES >> 20));
        }
        let text = std::str::from_utf8(bytes).map_err(|_| format!("{file} isn't UTF-8 text"))?;
        read_svg(text, &mut found, &mut guides, &mut report).map_err(|e| format!("{file}: {e}"))?;
    }
    if found.is_empty() {
        return Err("no GlyphLayer-… layers were found".into());
    }
    // The baseline: the guide's position read y-down, or y-up from the page's foot, whichever the
    // glyphs stand on (the median distance of their feet to it is the smaller).
    let feet: Vec<f64> = found.iter().filter_map(|f| bottom(&f.objects)).collect();
    let median_gap = |y: f64| {
        let mut d: Vec<f64> = feet.iter().map(|b| (b - y).abs()).collect();
        d.sort_by(f64::total_cmp);
        d.get(d.len() / 2).copied().unwrap_or(f64::INFINITY)
    };
    let baseline = match guides.first() {
        Some(&(down, up)) => {
            if median_gap(down) <= median_gap(up) {
                report.baseline = "the baseline guide (read from the top)".into();
                down
            } else {
                report.baseline = "the baseline guide (read from the page's foot)".into();
                up
            }
        }
        None => {
            // No guide: where most glyphs stand.
            let mut f = feet.clone();
            f.sort_by(f64::total_cmp);
            report.baseline = "no baseline guide: where most glyphs stand".into();
            f.get(f.len() / 2).copied().unwrap_or(0.0)
        }
    };
    // SVG space (y down) → font units (y up from the baseline).
    let to_font = Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, baseline]);
    let adv_default = json.get("horiz_adv_x_default").and_then(Value::as_f64).filter(|v| v.is_finite() && *v > 0.0);
    let advances = json.get("horiz_adv_x").and_then(Value::as_object);
    let mut glyphs: Vec<FontGlyph> = vec![];
    let mut seen = BTreeSet::new();
    let (mut top, mut low) = (0.0f64, 0.0f64);
    for f in found {
        if !seen.insert(f.chars.clone()) {
            report.skipped.push(format!("`{}`: a second layer for it", f.chars));
            continue;
        }
        let mut objects = vec![];
        let mut right = 0.0f64;
        for (path, stitch, evenodd) in f.objects {
            let p = to_font * path;
            let b = p.bounding_box();
            if b.is_finite() {
                (top, low, right) = (top.max(b.y1), low.min(b.y0), right.max(b.x1));
            }
            objects.push(GlyphObject { path: p.to_svg(), rule: evenodd.then(|| "evenodd".to_string()), stitch, ..Default::default() });
        }
        let own = advances.and_then(|m| m.get(&f.chars)).and_then(Value::as_f64).filter(|v| v.is_finite() && *v >= 0.0);
        let advance = own.or(adv_default).unwrap_or(right.max(0.0));
        glyphs.push(FontGlyph { chars: f.chars, advance, objects, ..Default::default() });
    }
    report.glyphs = glyphs.len();
    // The reference letter's height (`size`) is the cap height; one em is taken as 10/7 of it.
    let cap = json.get("size").and_then(Value::as_f64).filter(|v| v.is_finite() && *v > 0.0).or_else(|| {
        glyphs
            .iter()
            .find(|g| g.chars == "M" || g.chars == "H")
            .and_then(|g| g.objects.iter().filter_map(fonts::outline).map(|p| p.bounding_box().y1).reduce(f64::max))
    });
    let cap = cap.unwrap_or(top.max(1.0));
    let upem = (cap / 0.7).clamp(*fonts::UNITS_PER_EM.start(), *fonts::UNITS_PER_EM.end());
    let x_height = glyphs
        .iter()
        .find(|g| g.chars == "x")
        .and_then(|g| g.objects.iter().filter_map(fonts::outline).map(|p| p.bounding_box().y1).reduce(f64::max));
    let name = json.get("name").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty()).unwrap_or(src.name).to_string();
    let font = EmbroideryFont {
        id: crate::borders::slug(&name),
        name: name.clone(),
        description: json.get("description").and_then(Value::as_str).map(str::to_string),
        license: src.license.map(license),
        source: Some(format!("Ink/Stitch font folder `{}`", src.name)),
        units_per_em: upem,
        ascender: top.max(cap),
        descender: low.min(0.0),
        cap_height: cap,
        x_height: x_height.unwrap_or(cap * 0.7),
        kerning: kerning(json.get("hkern")),
        glyphs,
        ..Default::default()
    };
    fonts::check(&font)?;
    Ok((font, report))
}

/// The lowest point of a glyph's objects, SVG space (the largest y).
fn bottom(objects: &[(BezPath, StitchSettings, bool)]) -> Option<f64> {
    objects.iter().map(|(p, _, _)| p.bounding_box().y1).filter(|y| y.is_finite()).reduce(f64::max)
}

/// The licence: its SPDX id when the text names a well-known one, else the text itself.
fn license(text: &str) -> String {
    let t = text.trim();
    let has = |s: &str| t.to_ascii_lowercase().contains(&s.to_ascii_lowercase());
    if has("SIL Open Font License") {
        return "OFL-1.1".into();
    }
    if has("Attribution-NonCommercial-ShareAlike 4.0") || has("CC BY-NC-SA 4.0") {
        return "CC-BY-NC-SA-4.0".into();
    }
    let mut end = t.len().min(MAX_LICENSE);
    while !t.is_char_boundary(end) {
        end -= 1;
    }
    t.get(..end).unwrap_or_default().to_string()
}

/// `hkern`, as pairs: an object of two-character keys (`"AV": 20`), or of first characters each
/// holding the second's (`"A": {"V": 20}`). A positive Ink/Stitch value sets the pair closer.
fn kerning(v: Option<&Value>) -> Vec<KernPair> {
    let Some(m) = v.and_then(Value::as_object) else { return vec![] };
    let mut out = vec![];
    for (k, v) in m {
        match v {
            Value::Number(n) => {
                let mut c = k.chars();
                if let (Some(a), Some(b), None, Some(x)) = (c.next(), c.next(), c.next(), n.as_f64()) {
                    out.push(KernPair { left: a.to_string(), right: b.to_string(), value: -x });
                }
            }
            Value::Object(inner) => {
                for (r, x) in inner {
                    if let Some(x) = x.as_f64() {
                        out.push(KernPair { left: k.clone(), right: r.clone(), value: -x });
                    }
                }
            }
            _ => {}
        }
        if out.len() >= fonts::MAX_KERNING {
            break;
        }
    }
    out.retain(|k| k.value.is_finite());
    out
}

/// Read one glyph SVG: its glyph layers into `found`, its baseline guides into `guides` (the
/// position read y-down, and read y-up from the page's foot).
fn read_svg(text: &str, found: &mut Vec<Found>, guides: &mut Vec<(f64, f64)>, report: &mut Report) -> Result<(), String> {
    let opts = roxmltree::ParsingOptions { allow_dtd: false, nodes_limit: 5_000_000, ..Default::default() };
    let doc = roxmltree::Document::parse_with_options(text, opts).map_err(|e| format!("not SVG: {e}"))?;
    let root = doc.root_element();
    let page_height = root
        .attribute("viewBox")
        .and_then(|v| v.split(|c: char| c == ',' || c.is_whitespace()).filter(|s| !s.is_empty()).nth(3).and_then(|h| h.parse::<f64>().ok()))
        .or_else(|| root.attribute("height").and_then(number))
        .unwrap_or(0.0);
    for n in doc.descendants().filter(|n| n.is_element()) {
        let label = n.attribute((INKSCAPE, "label")).or_else(|| n.attribute("label"));
        if n.tag_name().name() == "guide"
            && n.tag_name().namespace().is_none_or(|ns| ns == SODIPODI)
            && label.is_some_and(|l| l.eq_ignore_ascii_case("baseline"))
        {
            if let Some(y) = n.attribute("position").and_then(|p| p.split(',').nth(1)).and_then(number) {
                guides.push((y, page_height - y));
            }
            continue;
        }
        let Some(name) = label.and_then(|l| l.strip_prefix("GlyphLayer-")) else { continue };
        if n.tag_name().name() != "g" {
            continue;
        }
        let count = name.chars().count();
        // A variant for a typographic feature (`a.init`), and long symbol names, aren't letters.
        if count > 1 && name.chars().skip(1).any(|c| c == '.') {
            report.skipped.push(format!("`{name}`: a variant glyph (not imported yet)"));
            continue;
        }
        if count > fonts::MAX_GLYPH_CHARS {
            report.skipped.push(format!("`{name}`: a symbol name, not characters"));
            continue;
        }
        let mut objects = vec![];
        for d in n.descendants().filter(|d| d.is_element()) {
            match d.tag_name().name() {
                "use" => {
                    let href = d.attribute(("http://www.w3.org/1999/xlink", "href")).or_else(|| d.attribute("href")).unwrap_or_default();
                    if href.starts_with("#inkstitch_") {
                        report.commands += 1;
                    }
                }
                "path" if !inside_defs(&d) => {
                    let Some(path) = d.attribute("d").and_then(|s| BezPath::from_svg(s).ok()) else { continue };
                    let xf = transform_of(&d, &n);
                    let (s, evenodd) = settings(&d, report);
                    objects.push((xf * path, s, evenodd));
                }
                _ => {}
            }
        }
        found.push(Found { chars: name.to_string(), objects });
    }
    Ok(())
}

fn inside_defs(n: &roxmltree::Node) -> bool {
    n.ancestors().any(|a| a.tag_name().name() == "defs")
}

fn number(s: &str) -> Option<f64> {
    s.trim().trim_end_matches("px").parse::<f64>().ok().filter(|v| v.is_finite())
}

/// The transforms from `n` up to and including `layer`, combined.
fn transform_of(n: &roxmltree::Node, layer: &roxmltree::Node) -> Affine {
    let mut xf = Affine::IDENTITY;
    for a in n.ancestors() {
        if let Some(t) = a.attribute("transform") {
            xf = parse_transform(t) * xf;
        }
        if a == *layer {
            break;
        }
    }
    xf
}

/// An SVG `transform` list (matrix, translate, scale, rotate, skewX, skewY); what doesn't parse is
/// left out.
fn parse_transform(s: &str) -> Affine {
    let mut xf = Affine::IDENTITY;
    for part in s.split(')') {
        let Some((name, args)) = part.split_once('(') else { continue };
        let v: Vec<f64> = args.split(|c: char| c == ',' || c.is_whitespace()).filter_map(number).collect();
        let at = |i: usize| v.get(i).copied().unwrap_or(0.0);
        let t = match name.trim() {
            "matrix" if v.len() == 6 => Affine::new([at(0), at(1), at(2), at(3), at(4), at(5)]),
            "translate" => Affine::translate((at(0), at(1))),
            "scale" => Affine::scale_non_uniform(at(0), if v.len() > 1 { at(1) } else { at(0) }),
            "rotate" => {
                let r = Affine::rotate(at(0).to_radians());
                if v.len() == 3 { Affine::translate((at(1), at(2))) * r * Affine::translate((-at(1), -at(2))) } else { r }
            }
            "skewX" => Affine::new([1.0, 0.0, at(0).to_radians().tan(), 1.0, 0.0, 0.0]),
            "skewY" => Affine::new([1.0, at(0).to_radians().tan(), 0.0, 1.0, 0.0, 0.0]),
            _ => continue,
        };
        if t.as_coeffs().iter().all(|c| c.is_finite()) {
            xf *= t;
        }
    }
    xf
}

/// A style property of `n`: from its `style` attribute, else the presentation attribute.
fn style_prop<'a>(n: &roxmltree::Node<'a, 'a>, key: &str) -> Option<&'a str> {
    let from_style =
        n.attribute("style").and_then(|s| s.split(';').filter_map(|kv| kv.split_once(':')).find(|(k, _)| k.trim() == key).map(|(_, v)| v.trim()));
    from_style.or_else(|| n.attribute(key))
}

/// The stitch settings `n`'s Ink/Stitch attributes ask for, mapped by meaning onto VectorCraft's
/// stitch types; whether its fill rule is even-odd.
fn settings(n: &roxmltree::Node, report: &mut Report) -> (StitchSettings, bool) {
    let attrs: BTreeMap<&str, &str> = n.attributes().filter(|a| a.namespace() == Some(NAMESPACE)).map(|a| (a.name(), a.value())).collect();
    let flag = |k: &str| attrs.get(k).is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes"));
    let num = |k: &str| attrs.get(k).and_then(|v| number(v));
    let filled = style_prop(n, "fill").is_none_or(|f| f != "none");
    let stroked = style_prop(n, "stroke").is_some_and(|s| s != "none");
    let evenodd = style_prop(n, "fill-rule") == Some("evenodd");
    let mut used: BTreeSet<&str> = BTreeSet::new();
    let mut take = |k: &'static str| {
        used.insert(k);
    };
    let s = if flag("satin_column") {
        take("satin_column");
        let mut s = StitchSettings::new(StitchKind::Satin);
        s.rails = true;
        for k in ["zigzag_spacing_mm", "pull_compensation_mm", "center_walk_underlay", "contour_underlay", "zigzag_underlay"] {
            take(k);
        }
        s.spacing = num("zigzag_spacing_mm");
        s.pull = num("pull_compensation_mm");
        s.underlay = if flag("zigzag_underlay") {
            Underlay::Zigzag
        } else if flag("contour_underlay") {
            Underlay::Edge
        } else if flag("center_walk_underlay") {
            Underlay::Center
        } else {
            Underlay::None
        };
        s
    } else if flag("manual_stitch") {
        take("manual_stitch");
        StitchSettings::new(StitchKind::Manual)
    } else if filled && !attrs.get("auto_fill").is_some_and(|v| v.eq_ignore_ascii_case("false")) || filled && !stroked {
        for k in ["auto_fill", "angle", "row_spacing_mm", "max_stitch_length_mm", "staggers", "fill_underlay", "expand_mm"] {
            take(k);
        }
        let mut s = StitchSettings::new(StitchKind::Fill);
        s.angle = num("angle").unwrap_or(0.0);
        s.spacing = num("row_spacing_mm");
        s.length = num("max_stitch_length_mm");
        s.stagger = num("staggers").map(|v| v.round().clamp(2.0, 8.0) as u8);
        s.pull = num("expand_mm").filter(|v| *v > 0.0);
        s.underlay = match attrs.get("fill_underlay") {
            Some(_) if flag("fill_underlay") => Underlay::Fill,
            Some(_) => Underlay::None,
            None => Underlay::Auto,
        };
        s
    } else {
        for k in ["running_stitch_length_mm", "bean_stitch_repeats", "repeats"] {
            take(k);
        }
        let bean = num("bean_stitch_repeats").filter(|n| *n >= 1.0);
        let mut s = StitchSettings::new(if bean.is_some() { StitchKind::Bean } else { StitchKind::Running });
        s.length = num("running_stitch_length_mm");
        // n repeats: each stitch sewn 2n + 1 times, within the passes Bean has.
        s.passes = bean.map(|n| (2.0 * n.round() + 1.0).clamp(3.0, 9.0) as u8);
        s
    };
    for k in attrs.keys() {
        if !used.contains(k) {
            report.unmapped.insert((*k).to_string());
        }
    }
    (s, evenodd)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny Ink/Stitch-style font written for these tests from the public documentation: a
    /// baseline guide, a satin `A`, a filled `B`, a running `-`, an `fi` ligature, a variant layer, a
    /// trim command and an attribute VectorCraft doesn't know.
    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
     xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"
     xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd"
     xmlns:inkstitch="http://inkstitch.org/namespace" viewBox="0 0 400 200" height="200">
  <sodipodi:namedview><sodipodi:guide position="0,50" inkscape:label="baseline"/></sodipodi:namedview>
  <g inkscape:groupmode="layer" inkscape:label="GlyphLayer-A">
    <path d="M 0 150 L 20 50 M 20 150 L 40 50 M 5 100 L 35 100" style="fill:none;stroke:#000"
          inkstitch:satin_column="True" inkstitch:zigzag_spacing_mm="0.35" inkstitch:center_walk_underlay="True"/>
    <use xlink:href="#inkstitch_trim" x="40" y="150"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="GlyphLayer-B" transform="translate(50,0)">
    <path d="M 0 50 L 30 50 L 30 150 L 0 150 Z" style="fill:#ff0000" inkstitch:angle="45"
          inkstitch:row_spacing_mm="0.3" inkstitch:fill_underlay="False" inkstitch:sparkle="7"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="GlyphLayer--">
    <path d="M 0 120 L 30 120" style="fill:none;stroke:#000" inkstitch:bean_stitch_repeats="1"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="GlyphLayer-fi">
    <path d="M 0 50 L 10 50 L 10 150 L 0 150 Z" style="fill:#000"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="GlyphLayer-a.init">
    <path d="M 0 0 L 1 1" style="fill:none;stroke:#000"/>
  </g>
</svg>"##;

    const JSON: &str =
        r#"{"name": "Test Script", "size": 100, "horiz_adv_x_default": 45, "horiz_adv_x": {"A": 42}, "hkern": {"AB": 5, "B": {"A": -3}}}"#;

    fn font() -> (EmbroideryFont, Report) {
        let src = Source {
            name: "test_script",
            json: JSON.as_bytes(),
            svgs: vec![("ltr.svg", SVG.as_bytes())],
            license: Some("Copyright 2026 Someone\nSIL Open Font License 1.1"),
        };
        import(&src).unwrap()
    }

    #[test]
    fn layers_become_glyphs_on_the_baseline() {
        let (f, r) = font();
        assert_eq!(f.name, "Test Script");
        assert_eq!(f.license.as_deref(), Some("OFL-1.1"));
        assert_eq!(r.glyphs, 4, "{r:?}");
        assert!(r.skipped.iter().any(|s| s.contains("a.init")));
        // The guide at 50 read from the page's foot is y = 150, where the glyphs stand.
        assert!(r.baseline.contains("foot"), "{}", r.baseline);
        let b = f.glyph("B").unwrap();
        let bb = fonts::outline(&b.objects[0]).unwrap().bounding_box();
        assert!((bb.y0 - 0.0).abs() < 1e-6 && (bb.y1 - 100.0).abs() < 1e-6, "B stands on the baseline, 100 tall: {bb:?}");
        assert!((bb.x0 - 50.0).abs() < 1e-6, "the layer's transform applies: {bb:?}");
        // Metrics: the reference letter's size is the cap height.
        assert_eq!(f.cap_height, 100.0);
        assert!((f.units_per_em - 100.0 / 0.7).abs() < 1e-9);
    }

    #[test]
    fn settings_map_by_meaning() {
        let (f, r) = font();
        let a = &f.glyph("A").unwrap().objects[0].stitch;
        assert_eq!((a.kind, a.rails, a.spacing, a.underlay), (StitchKind::Satin, true, Some(0.35), Underlay::Center));
        let b = &f.glyph("B").unwrap().objects[0].stitch;
        assert_eq!((b.kind, b.angle, b.spacing, b.underlay), (StitchKind::Fill, 45.0, Some(0.3), Underlay::None));
        let dash = &f.glyph("-").unwrap().objects[0].stitch;
        assert_eq!((dash.kind, dash.passes), (StitchKind::Bean, Some(3)));
        // What isn't mapped is reported; the trim command is counted.
        assert_eq!(r.unmapped.iter().cloned().collect::<Vec<_>>(), vec!["sparkle".to_string()]);
        assert_eq!(r.commands, 1);
    }

    #[test]
    fn advances_and_kerning() {
        let (f, _) = font();
        assert_eq!(f.glyph("A").unwrap().advance, 42.0);
        assert_eq!(f.glyph("B").unwrap().advance, 45.0, "the default");
        // A positive Ink/Stitch value sets the pair closer.
        assert_eq!(f.kern("A", "B"), -5.0);
        assert_eq!(f.kern("B", "A"), 3.0);
        // The converted font round-trips through a .vcfont file.
        assert_eq!(fonts::read_file(fonts::write_file(&f).unwrap().as_bytes()).unwrap(), f);
    }

    /// The notes list every attribute the reader maps, and the folder files and limits it reads.
    #[test]
    fn the_notes_describe_what_is_read() {
        let notes = include_str!("../../../docs/formats/inkstitch-fonts.md");
        for k in MAPPED {
            assert!(notes.contains(&format!("`{k}`")), "inkstitch-fonts.md maps `{k}`");
        }
        for k in [
            "`font.json`",
            "`ltr.svg`",
            "`LICENSE`",
            "`size`",
            "`horiz_adv_x_default`",
            "`horiz_adv_x`",
            "`hkern`",
            "`GlyphLayer-",
            "`baseline`",
            "#inkstitch_",
            "64 files",
            "64 MB",
            NAMESPACE,
        ] {
            assert!(notes.contains(k), "inkstitch-fonts.md describes {k}");
        }
        assert_eq!((MAX_SVGS, MAX_SVG_BYTES >> 20), (64, 64));
    }

    #[test]
    fn bad_folders_are_errors_not_panics() {
        let src = |json: &'static str, svg: &'static str| Source {
            name: "x",
            json: json.as_bytes(),
            svgs: vec![("ltr.svg", svg.as_bytes())],
            license: None,
        };
        assert!(import(&src("not json", SVG)).is_err());
        assert!(import(&src(JSON, "<svg")).is_err());
        assert!(import(&src(JSON, "<svg xmlns=\"http://www.w3.org/2000/svg\"/>")).unwrap_err().contains("GlyphLayer"));
        assert!(import(&Source { name: "x", json: JSON.as_bytes(), svgs: vec![], license: None }).is_err());
        // A DTD (entity expansion) is refused.
        assert!(import(&src(JSON, "<!DOCTYPE svg [<!ENTITY a \"aaaa\">]><svg>&a;</svg>")).is_err());
    }

    proptest::proptest! {
        #[test]
        fn junk_svg_never_panics(svg in "\\PC{0,400}") {
            let src = Source { name: "x", json: JSON.as_bytes(), svgs: vec![("ltr.svg", svg.as_bytes())], license: None };
            let _ = import(&src);
        }

        #[test]
        fn damaged_font_never_panics(at in 0usize..3000, byte in proptest::num::u8::ANY) {
            let mut bytes = SVG.as_bytes().to_vec();
            let i = at % bytes.len();
            bytes[i] = byte;
            let src = Source { name: "x", json: JSON.as_bytes(), svgs: vec![("ltr.svg", &bytes)], license: None };
            if let Ok((f, _)) = import(&src) {
                proptest::prop_assert!(fonts::check(&f).is_ok());
            }
        }
    }
}
