//! Hoops: the sewing field a design must fit, set on the document
//! ([`vectorcraft_doc::EmbroiderySetup::hoop`]). Machines centre a design in the hoop, so the field
//! is centred on the design. Common field sizes are made here; others come from `.vchoop` files
//! (a list of hoops; specified in `docs/formats/vchoop.md`). The sizes are the usual nominal ones;
//! a machine's own field can be a little smaller, so check its manual and set the real size.

use kurbo::Point;
use vectorcraft_doc::Hoop;

use crate::Plan;

/// The file extension of hoop list files.
pub const FILE_EXT: &str = "vchoop";
/// The `format` a hoop list file declares.
pub const FILE_FORMAT: &str = "vectorcraft-hoops";
/// The file format version written; files of a later version aren't read.
pub const FILE_VERSION: u32 = 1;
/// The largest hoop list file read, bytes, and the most hoops it holds.
pub const MAX_FILE_BYTES: usize = 1 << 20;
pub const MAX_HOOPS: usize = 1000;

/// The common hoops' ids, smallest first.
pub const BUILTIN: [&str; 6] = ["4x4", "5x7", "6x10", "8x8", "8x12", "cap"];

/// A common hoop by id (one of [`BUILTIN`]): nominal sewing fields, in mm.
pub fn builtin(id: &str) -> Option<Hoop> {
    let (name, width, height, about) = match id {
        "4x4" => ("100 × 100 mm (4 × 4 in)", 100.0, 100.0, "The smallest common hoop: patches, logos, monograms."),
        "5x7" => ("130 × 180 mm (5 × 7 in)", 130.0, 180.0, "The usual hoop of home machines."),
        "6x10" => ("160 × 260 mm (6 × 10 in)", 160.0, 260.0, "Larger home and small commercial machines."),
        "8x8" => ("200 × 200 mm (8 × 8 in)", 200.0, 200.0, "Square designs: quilt blocks, jacket backs."),
        "8x12" => ("200 × 300 mm (8 × 12 in)", 200.0, 300.0, "Large designs on larger machines."),
        "cap" => ("Cap 130 × 60 mm", 130.0, 60.0, "The front of a cap, sewn on a cap frame: wide and low."),
        _ => return None,
    };
    Some(Hoop { id: id.into(), name: name.into(), width, height, description: Some(about.into()), ..Hoop::default() })
}

/// The hoop's field size, mm (always within [`Hoop::SIZE`]).
pub fn size(h: &Hoop) -> (f64, f64) {
    let c = |v: f64| if v.is_finite() { v.clamp(*Hoop::SIZE.start(), *Hoop::SIZE.end()) } else { *Hoop::SIZE.start() };
    (c(h.width), c(h.height))
}

/// Where the hoop's field lies around `plan`: its centre (the design's) and size, mm. None when
/// the design has no stitches.
pub fn field(plan: &Plan, h: &Hoop) -> Option<(Point, f64, f64)> {
    let b = plan.stats().bounds?;
    let (w, hh) = size(h);
    Some((Point::new((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0), w, hh))
}

/// How a design fits a hoop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    Fits,
    /// Only turned a quarter turn.
    FitsTurned,
    TooBig,
}

/// Whether a design `w` × `h` mm fits hoop `hoop`.
pub fn fit(w: f64, h: f64, hoop: &Hoop) -> Fit {
    let (fw, fh) = size(hoop);
    let inside = |w: f64, h: f64| {
        if hoop.round {
            // Inside the largest circle in the field: the design's corners.
            let r = fw.min(fh) / 2.0;
            (w / 2.0).hypot(h / 2.0) <= r + 1e-9
        } else {
            w <= fw + 1e-9 && h <= fh + 1e-9
        }
    };
    if inside(w, h) {
        Fit::Fits
    } else if inside(h, w) {
        Fit::FitsTurned
    } else {
        Fit::TooBig
    }
}

/// A hoop list file as stored.
#[derive(serde::Serialize, serde::Deserialize)]
struct File {
    format: String,
    version: u32,
    #[serde(default = "mm")]
    units: String,
    hoops: Vec<Hoop>,
}

fn mm() -> String {
    "mm".into()
}

/// Read a `.vchoop` file: JSON with `format` [`FILE_FORMAT`], `version` and `hoops`, each with a
/// finite positive size. Each hoop's id is its own, else made from its name.
pub fn read_file(bytes: &[u8]) -> Result<Vec<Hoop>, String> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(format!("a hoop file is at most {} MB", MAX_FILE_BYTES >> 20));
    }
    let f: File = serde_json::from_slice(bytes).map_err(|e| format!("not a hoop file: {e}"))?;
    if f.format != FILE_FORMAT {
        return Err(format!("not a hoop file (its format is `{}`)", f.format));
    }
    if f.version > FILE_VERSION {
        return Err(format!("this hoop file is version {}: this VectorCraft reads up to version {FILE_VERSION}", f.version));
    }
    if f.units != "mm" {
        return Err(format!("hoop files measure in mm, not `{}`", f.units));
    }
    if f.hoops.is_empty() || f.hoops.len() > MAX_HOOPS {
        return Err(format!("a hoop file holds 1 to {MAX_HOOPS} hoops"));
    }
    f.hoops
        .into_iter()
        .map(|mut h| {
            if !(h.width.is_finite() && h.height.is_finite() && h.width > 0.0 && h.height > 0.0) {
                return Err(format!("hoop `{}` needs a width and height above 0 (mm)", h.name));
            }
            h.id = crate::borders::slug(if h.id.trim().is_empty() { &h.name } else { &h.id });
            if h.name.trim().is_empty() {
                h.name.clone_from(&h.id);
            }
            Ok(h)
        })
        .collect()
}

/// `hoops` as a `.vchoop` file.
pub fn write_file(hoops: &[Hoop]) -> Result<String, String> {
    let f = File { format: FILE_FORMAT.into(), version: FILE_VERSION, units: mm(), hoops: hoops.to_vec() };
    serde_json::to_string_pretty(&f).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_hoops_read_back_and_fit() {
        let all: Vec<Hoop> = BUILTIN.iter().filter_map(|id| builtin(id)).collect();
        assert_eq!(all.len(), BUILTIN.len());
        assert_eq!(read_file(write_file(&all).unwrap().as_bytes()).unwrap(), all);
        let five = builtin("5x7").unwrap();
        assert_eq!(fit(120.0, 170.0, &five), Fit::Fits);
        assert_eq!(fit(170.0, 120.0, &five), Fit::FitsTurned);
        assert_eq!(fit(140.0, 190.0, &five), Fit::TooBig);
        let round = Hoop { width: 100.0, height: 100.0, round: true, ..Hoop::default() };
        assert_eq!(fit(70.0, 70.0, &round), Fit::Fits, "corners 49.5 mm out");
        assert_eq!(fit(80.0, 80.0, &round), Fit::TooBig, "corners 56.6 mm out");
    }

    #[test]
    fn bad_files_are_refused() {
        assert!(read_file(br#"{"format": "vectorcraft-hoops", "version": 1, "hoops": []}"#).is_err());
        assert!(read_file(br#"{"format": "vectorcraft-hoops", "version": 1, "hoops": [{"name": "x", "width": 0, "height": 10}]}"#).is_err());
        assert!(read_file(br#"{"format": "vectorcraft-hoops", "version": 9, "hoops": [{"name": "x", "width": 10, "height": 10}]}"#).is_err());
        let h =
            read_file(br#"{"format": "vectorcraft-hoops", "version": 1, "hoops": [{"name": "Big Frame", "width": 400, "height": 300}]}"#).unwrap();
        assert_eq!(h.first().map(|h| h.id.as_str()), Some("big-frame"));
    }
}
