//! Fabric profiles: the settings a fabric wants (density, compensation, how much underlay), set on
//! the document ([`vectorcraft_doc::EmbroiderySetup::fabric`]) and used for whatever an object's
//! own stitch settings leave unset. Six starters are made here; others come from `.vcfabric` files
//! (specified in `docs/formats/vcfabric.md`).
//!
//! The starters' numbers are starting points from general digitizing practice (stretchy and
//! lofty fabrics want more underlay and compensation, leather fewer needle holes); the sew-out
//! test sheets (`embroidery.testSheet`) are how they get checked on real fabric.

use vectorcraft_doc::{Fabric, StitchKind, StitchSettings};

use crate::generate::AutoUnderlay;

/// The file extension of fabric profile files.
pub const FILE_EXT: &str = "vcfabric";
/// The `format` a fabric profile file declares.
pub const FILE_FORMAT: &str = "vectorcraft-fabric";
/// The file format version written; files of a later version aren't read.
pub const FILE_VERSION: u32 = 1;
/// The largest fabric profile file read, bytes.
pub const MAX_FILE_BYTES: usize = 1 << 20;

/// The starter profiles' ids, in the order they are listed.
pub const BUILTIN: [&str; 6] = ["woven", "knit", "towel", "cap", "leather", "denim"];

/// A starter profile by id (one of [`BUILTIN`]).
pub fn builtin(id: &str) -> Option<Fabric> {
    // (name, satin spacing, fill spacing, pull mm, pull %, push, centre under, zigzag over, shape edge over)
    let (name, satin, fill, pull, pct, push, center, zigzag, edge, about) = match id {
        "woven" => (
            "Woven",
            0.4,
            0.4,
            0.2,
            0.0,
            0.0,
            2.0,
            5.0,
            2.5,
            "Stable woven cotton, twill or canvas: the stitches' usual settings, a little pull compensation.",
        ),
        "knit" => (
            "Knit",
            0.45,
            0.45,
            0.3,
            5.0,
            0.2,
            1.5,
            4.0,
            2.0,
            "T-shirts and polos: stretchy, so more underlay and compensation and a slightly open density.",
        ),
        "towel" => (
            "Fleece / Towel",
            0.35,
            0.38,
            0.3,
            5.0,
            0.2,
            1.0,
            3.0,
            1.5,
            "Lofty pile: dense stitches and heavy underlay so the pile doesn't poke through.",
        ),
        "cap" => ("Cap", 0.4, 0.45, 0.35, 5.0, 0.3, 1.5, 4.0, 2.0, "Structured caps: sewn on a curve, so more compensation, especially push."),
        "leather" => ("Leather", 0.5, 0.6, 0.1, 0.0, 0.0, 3.0, 7.0, 3.5, "Leather and vinyl: every hole stays, so fewer stitches, little underlay."),
        "denim" => ("Denim", 0.4, 0.4, 0.2, 2.0, 0.1, 2.0, 5.0, 2.5, "Heavy woven denim: like woven, with a little more compensation."),
        _ => return None,
    };
    Some(Fabric {
        id: id.into(),
        name: name.into(),
        description: Some(about.into()),
        satin_spacing: Some(satin),
        fill_spacing: Some(fill),
        pull: Some(pull),
        pull_percent: Some(pct),
        push: Some(push),
        satin_center_under: Some(center),
        satin_zigzag_over: Some(zigzag),
        shape_edge_over: Some(edge),
        ..Fabric::default()
    })
}

/// A finite value of `v`, inside `range`.
fn within(v: Option<f64>, range: std::ops::RangeInclusive<f64>) -> Option<f64> {
    v.filter(|x| x.is_finite()).map(|x| x.clamp(*range.start(), *range.end()))
}

/// `s` with what it leaves unset taken from fabric `f` (its spacing, compensation). Auto is left
/// as it is: what it picks is filled in when it has picked.
pub fn fill_in(s: &StitchSettings, f: &Fabric) -> StitchSettings {
    let mut t = s.clone();
    if t.unknown.is_some() {
        return t;
    }
    let spacing = match t.kind {
        StitchKind::Satin | StitchKind::SatinShape => f.satin_spacing,
        StitchKind::Fill => f.fill_spacing,
        _ => None,
    };
    if t.spacing.is_none() {
        t.spacing = within(spacing, StitchSettings::SPACING);
    }
    let compensated = matches!(t.kind, StitchKind::Satin | StitchKind::SatinShape | StitchKind::Fill);
    if compensated {
        if t.pull.is_none() {
            t.pull = within(f.pull, StitchSettings::PULL);
        }
        if t.pull_percent.is_none() {
            t.pull_percent = within(f.pull_percent, StitchSettings::PULL_PERCENT);
        }
    }
    if matches!(t.kind, StitchKind::Satin | StitchKind::Fill) && t.push.is_none() {
        t.push = within(f.push, StitchSettings::PUSH);
    }
    t
}

/// The auto underlay's thresholds for fabric `f` (none: the defaults).
pub fn auto_underlay(f: Option<&Fabric>) -> AutoUnderlay {
    let d = AutoUnderlay::default();
    let Some(f) = f else { return d };
    let pick = |v: Option<f64>, dflt: f64| within(v, Fabric::THRESHOLD).unwrap_or(dflt);
    let center = pick(f.satin_center_under, d.satin_center_under);
    AutoUnderlay {
        satin_center_under: center,
        // Never below the centre-walk threshold.
        satin_zigzag_over: pick(f.satin_zigzag_over, d.satin_zigzag_over).max(center),
        shape_edge_over: pick(f.shape_edge_over, d.shape_edge_over),
    }
}

/// A fabric profile file as stored: the format header, then the profile's fields.
#[derive(serde::Serialize, serde::Deserialize)]
struct File {
    format: String,
    version: u32,
    #[serde(default = "mm")]
    units: String,
    #[serde(flatten)]
    fabric: Fabric,
}

fn mm() -> String {
    "mm".into()
}

/// Whether `f`'s numbers can be used (finite; ranges are clamped when used).
pub fn check(f: &Fabric) -> Result<(), String> {
    let values = [
        ("satinSpacing", f.satin_spacing),
        ("fillSpacing", f.fill_spacing),
        ("pull", f.pull),
        ("pullPercent", f.pull_percent),
        ("push", f.push),
        ("satinCenterUnder", f.satin_center_under),
        ("satinZigzagOver", f.satin_zigzag_over),
        ("shapeEdgeOver", f.shape_edge_over),
    ];
    match values.iter().find(|(_, v)| v.is_some_and(|x| !x.is_finite() || x < 0.0)) {
        Some((k, _)) => Err(format!("a fabric profile's `{k}` must be a number of at least 0")),
        None => Ok(()),
    }
}

/// Read a `.vcfabric` file: JSON with `format` [`FILE_FORMAT`], `version` and the profile
/// ([`Fabric`]), checked ([`check`]). Its id is its own, else made from its name.
pub fn read_file(bytes: &[u8]) -> Result<Fabric, String> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(format!("a fabric profile file is at most {} MB", MAX_FILE_BYTES >> 20));
    }
    let f: File = serde_json::from_slice(bytes).map_err(|e| format!("not a fabric profile file: {e}"))?;
    if f.format != FILE_FORMAT {
        return Err(format!("not a fabric profile file (its format is `{}`)", f.format));
    }
    if f.version > FILE_VERSION {
        return Err(format!("this fabric profile file is version {}: this VectorCraft reads up to version {FILE_VERSION}", f.version));
    }
    if f.units != "mm" {
        return Err(format!("fabric profile files measure in mm, not `{}`", f.units));
    }
    let mut p = f.fabric;
    p.id = crate::borders::slug(if p.id.trim().is_empty() { &p.name } else { &p.id });
    if p.name.trim().is_empty() {
        p.name.clone_from(&p.id);
    }
    check(&p)?;
    Ok(p)
}

/// `f` as a `.vcfabric` file.
pub fn write_file(f: &Fabric) -> Result<String, String> {
    let file = File { format: FILE_FORMAT.into(), version: FILE_VERSION, units: mm(), fabric: f.clone() };
    serde_json::to_string_pretty(&file).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starters_read_back_and_have_every_setting() {
        for id in BUILTIN {
            let f = builtin(id).unwrap();
            assert_eq!(read_file(write_file(&f).unwrap().as_bytes()).unwrap(), f, "{id}");
            assert!(f.satin_spacing.is_some() && f.push.is_some() && f.shape_edge_over.is_some(), "{id}");
            let a = auto_underlay(Some(&f));
            assert!(a.satin_center_under <= a.satin_zigzag_over, "{id}");
        }
        assert!(builtin("silk").is_none());
    }

    #[test]
    fn a_fabric_fills_in_only_what_objects_leave_unset() {
        let knit = builtin("knit").unwrap();
        let s = fill_in(&StitchSettings::new(StitchKind::Fill), &knit);
        assert_eq!((s.spacing, s.pull, s.pull_percent, s.push), (Some(0.45), Some(0.3), Some(5.0), Some(0.2)));
        let own = StitchSettings { spacing: Some(0.3), push: Some(0.0), ..StitchSettings::new(StitchKind::Satin) };
        let s = fill_in(&own, &knit);
        assert_eq!((s.spacing, s.push, s.pull), (Some(0.3), Some(0.0), Some(0.3)), "the object's own settings win");
        // Running stitches take nothing from it; Auto waits until it has picked.
        assert_eq!(fill_in(&StitchSettings::new(StitchKind::Running), &knit), StitchSettings::new(StitchKind::Running));
        assert_eq!(fill_in(&StitchSettings::new(StitchKind::Auto), &knit).spacing, None);
    }

    #[test]
    fn bad_files_are_refused() {
        let mut v: serde_json::Value = serde_json::from_str(&write_file(&builtin("cap").unwrap()).unwrap()).unwrap();
        v["pull"] = serde_json::json!(-1.0);
        assert!(read_file(v.to_string().as_bytes()).is_err());
        assert!(read_file(br#"{"format": "vectorcraft-border-pattern", "version": 1}"#).is_err());
        assert!(read_file(br#"{"format": "vectorcraft-fabric", "version": 99, "name": "x"}"#).is_err());
        let named = read_file(br#"{"format": "vectorcraft-fabric", "version": 1, "name": "My Polo Shirt"}"#).unwrap();
        assert_eq!(named.id, "my-polo-shirt");
        // Out-of-range thresholds are clamped when used.
        let wild = Fabric { satin_center_under: Some(50.0), satin_zigzag_over: Some(1.0), ..Fabric::default() };
        let a = auto_underlay(Some(&wild));
        assert_eq!((a.satin_center_under, a.satin_zigzag_over), (12.0, 12.0));
    }
}
