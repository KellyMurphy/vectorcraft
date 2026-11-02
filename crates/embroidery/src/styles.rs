//! Stitch styles: named sets of stitch settings, kept in the document
//! ([`vectorcraft_doc::EmbroiderySetup::styles`]) and copied onto objects when applied. Three
//! starters are made here; others come from `.vcstyle` files (specified in
//! `docs/formats/vcstyle.md`).

use vectorcraft_doc::{StitchKind, StitchSettings, StitchStyle, Underlay};

use crate::params;

/// The file extension of stitch style files.
pub const FILE_EXT: &str = "vcstyle";
/// The `format` a stitch style file declares.
pub const FILE_FORMAT: &str = "vectorcraft-stitch-style";
/// The file format version written; files of a later version aren't read.
pub const FILE_VERSION: u32 = 1;
/// The largest stitch style file read, bytes.
pub const MAX_FILE_BYTES: usize = 1 << 20;

/// The starter styles' ids, in the order they are listed.
pub const BUILTIN: [&str; 3] = ["small-lettering", "bold-border", "light-fill"];

/// A starter style by id (one of [`BUILTIN`]).
pub fn builtin(id: &str) -> Option<StitchStyle> {
    let (name, about, stitch) = match id {
        "small-lettering" => (
            "Small Lettering",
            "Type 5–10 mm tall: letters picked by stroke width, a little denser, with a little pull compensation.",
            StitchSettings { spacing: Some(0.35), pull: Some(0.15), ..StitchSettings::new(StitchKind::Auto) },
        ),
        "bold-border" => (
            "Bold Border",
            "A 4 mm satin border with automatic underlay, for outlining patches and shapes.",
            StitchSettings { width: Some(4.0), pull: Some(0.2), underlay: Underlay::Auto, ..StitchSettings::new(StitchKind::Satin) },
        ),
        "light-fill" => (
            "Light Background Fill",
            "An open fill for large backgrounds: fewer stitches, the fabric shows a little, no underlay.",
            StitchSettings { spacing: Some(0.8), angle: 45.0, ..StitchSettings::new(StitchKind::Fill) },
        ),
        _ => return None,
    };
    Some(StitchStyle { id: id.into(), name: name.into(), description: Some(about.into()), stitch, ..StitchStyle::default() })
}

/// Style `key` (an id, or a name in any case) among `mine`, else a starter. Styles this version
/// can't read are skipped.
pub fn find(mine: &[StitchStyle], key: &str) -> Option<StitchStyle> {
    let key = key.trim();
    let usable = || mine.iter().filter(|s| s.unknown.is_none());
    usable()
        .find(|s| s.id == key)
        .or_else(|| usable().find(|s| s.name.eq_ignore_ascii_case(key)))
        .cloned()
        .or_else(|| builtin(key))
        .or_else(|| BUILTIN.iter().filter_map(|id| builtin(id)).find(|s| s.name.eq_ignore_ascii_case(key)))
}

/// Whether `s`'s settings can be used: each is checked as `embroidery.set` checks it (its range,
/// and that it suits the stitch type).
pub fn check(s: &StitchStyle) -> Result<(), String> {
    let serde_json::Value::Object(fields) = serde_json::to_value(&s.stitch).map_err(|e| e.to_string())? else {
        return Err("a stitch style's `stitch` must be an object".into());
    };
    for (key, v) in &fields {
        // The document calls the stitch type `kind`; embroidery.set calls it `type`.
        let Some(p) = params::param(if key == "kind" { "type" } else { key }) else { continue };
        p.parse(v).map_err(|e| format!("a stitch style's settings: {e}"))?;
        if let Some(suits) = p.suits {
            suits(&s.stitch).map_err(|e| format!("a stitch style's settings: {e}"))?;
        }
    }
    Ok(())
}

/// A stitch style file as stored: the format header, then the style's fields.
#[derive(serde::Serialize, serde::Deserialize)]
struct File {
    format: String,
    version: u32,
    #[serde(default = "mm")]
    units: String,
    #[serde(flatten)]
    style: StitchStyle,
}

fn mm() -> String {
    "mm".into()
}

/// Read a `.vcstyle` file: JSON with `format` [`FILE_FORMAT`], `version` and the style
/// ([`StitchStyle`]), checked ([`check`]). Its id is its own, else made from its name.
pub fn read_file(bytes: &[u8]) -> Result<StitchStyle, String> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(format!("a stitch style file is at most {} MB", MAX_FILE_BYTES >> 20));
    }
    let v: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| format!("not a stitch style file: {e}"))?;
    if let Some(f) = v.get("format").and_then(serde_json::Value::as_str).filter(|f| *f != FILE_FORMAT) {
        return Err(format!("not a stitch style file (its format is `{f}`)"));
    }
    if let Some(n) = v.get("version").and_then(serde_json::Value::as_u64).filter(|n| *n > u64::from(FILE_VERSION)) {
        return Err(format!("this stitch style file is version {n}: this VectorCraft reads up to version {FILE_VERSION}"));
    }
    let f: File = serde_json::from_value(v).map_err(|e| format!("not a stitch style file this VectorCraft can read: {e}"))?;
    if f.format != FILE_FORMAT {
        return Err(format!("not a stitch style file (its format is `{}`)", f.format));
    }
    if f.units != "mm" {
        return Err(format!("stitch style files measure in mm, not `{}`", f.units));
    }
    let mut st = f.style;
    st.id = crate::borders::slug(if st.id.trim().is_empty() { &st.name } else { &st.id });
    if st.id.is_empty() {
        return Err("a stitch style needs an id or a name".into());
    }
    if st.name.trim().is_empty() {
        st.name.clone_from(&st.id);
    }
    // What belongs to one object isn't part of a style.
    st.stitch = st.stitch.as_style();
    check(&st)?;
    Ok(st)
}

/// `s` as a `.vcstyle` file.
pub fn write_file(s: &StitchStyle) -> Result<String, String> {
    let file = File { format: FILE_FORMAT.into(), version: FILE_VERSION, units: mm(), style: s.clone() };
    serde_json::to_string_pretty(&file).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starters_read_back_and_are_usable() {
        for id in BUILTIN {
            let s = builtin(id).unwrap();
            assert_eq!(read_file(write_file(&s).unwrap().as_bytes()).unwrap(), s, "{id}");
            check(&s).unwrap();
            assert_eq!(s.stitch, s.stitch.as_style(), "{id}: nothing of one object's");
        }
        assert!(builtin("silk").is_none());
    }

    #[test]
    fn styles_are_found_by_id_or_name() {
        let mine = vec![StitchStyle { id: "bold-border".into(), name: "My Border".into(), ..builtin("bold-border").unwrap() }];
        assert_eq!(find(&mine, "bold-border").unwrap().name, "My Border", "the document's own first");
        assert_eq!(find(&mine, "my border").unwrap().id, "bold-border");
        assert_eq!(find(&mine, "Light Background Fill").unwrap().id, "light-fill");
        assert!(find(&mine, "nope").is_none());
    }

    #[test]
    fn bad_files_are_refused() {
        let file = |stitch: serde_json::Value| serde_json::json!({"format": FILE_FORMAT, "version": 1, "name": "X", "stitch": stitch}).to_string();
        assert!(read_file(file(serde_json::json!({"kind": "satin", "width": 40.0, "split": "off"})).as_bytes()).is_err(), "too wide");
        assert!(read_file(file(serde_json::json!({"kind": "satin", "spacing": 50.0})).as_bytes()).is_err());
        assert!(read_file(file(serde_json::json!({"kind": "fill", "underlay": "zigzag"})).as_bytes()).is_err(), "fills don't zigzag");
        assert!(read_file(file(serde_json::json!({"kind": "laserCut"})).as_bytes()).is_err());
        assert!(read_file(br#"{"format": "vectorcraft-fabric", "version": 1, "name": "x"}"#).is_err());
        assert!(
            read_file(br#"{"format": "vectorcraft-stitch-style", "version": 99, "name": "x", "stitch": {"kind": "laserCut"}}"#)
                .unwrap_err()
                .contains("version 99")
        );
        assert!(read_file(&vec![b' '; MAX_FILE_BYTES + 1]).is_err());
        // Named, it gets an id; one object's start point is left out.
        let s = read_file(file(serde_json::json!({"kind": "running", "startAt": [1.0, 2.0], "length": 3.0})).as_bytes()).unwrap();
        assert_eq!((s.id.as_str(), s.stitch.start_at, s.stitch.length), ("x", None, Some(3.0)));
    }

    proptest::proptest! {
        #[test]
        fn mutated_style_files_never_panic(bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..400), cut in 0usize..2000) {
            let good = write_file(&builtin("bold-border").unwrap()).unwrap();
            let _ = read_file(&bytes);
            let mut b = good.into_bytes();
            b.truncate(cut.min(b.len()));
            if let Ok(s) = read_file(&b) {
                let _ = (check(&s), s.stitch.styled(None));
            }
        }
    }
}
