//! Embroidery library folders (E8.5): border patterns, fabric profiles, hoop lists and thread
//! charts kept as files in the embroidery library folder, one subfolder per kind, so they can be
//! used in any document and shared by copying files. `embroidery.libraries` lists them, the load
//! commands' `library` parameter copies one into the document (which keeps its own copy, so it
//! sews the same anywhere), and the save commands' `user: true` writes one there. The desktop app
//! sets the folder (next to the preferences); the web and headless sessions have none.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_doc::{BorderPattern, Fabric, Hoop, ThreadChart};
use vectorcraft_embroidery::{borders, fabrics, hoops, threads};

use super::swatchlib::{Libraries, LibraryFile, LibraryInfo};
use super::*;

/// The kinds of embroidery library: id (also the subfolder's name) and file extension.
pub const KINDS: [(&str, &str); 4] = [("borders", "vcborder"), ("fabrics", "vcfabric"), ("hoops", "vchoop"), ("threads", "vcthreads")];

impl LibraryFile for BorderPattern {
    const EXTS: &'static [&'static str] = &["vcborder"];
    fn read(bytes: &[u8], _: &str) -> std::result::Result<Self, String> {
        borders::read_file(bytes)
    }
    fn name(&self) -> &str {
        &self.name
    }
}

impl LibraryFile for Fabric {
    const EXTS: &'static [&'static str] = &["vcfabric"];
    fn read(bytes: &[u8], _: &str) -> std::result::Result<Self, String> {
        fabrics::read_file(bytes)
    }
    fn name(&self) -> &str {
        &self.name
    }
}

impl LibraryFile for ThreadChart {
    const EXTS: &'static [&'static str] = &["vcthreads"];
    fn read(bytes: &[u8], _: &str) -> std::result::Result<Self, String> {
        threads::read_file(bytes)
    }
    fn name(&self) -> &str {
        &self.name
    }
}

/// A `.vchoop` file's hoops, named after the file (the format has no name of its own).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HoopList {
    pub name: String,
    pub hoops: Vec<Hoop>,
}

impl LibraryFile for HoopList {
    const EXTS: &'static [&'static str] = &["vchoop"];
    fn read(bytes: &[u8], stem: &str) -> std::result::Result<Self, String> {
        Ok(HoopList { name: stem.to_string(), hoops: hoops::read_file(bytes)? })
    }
    fn name(&self) -> &str {
        &self.name
    }
}

/// The embroidery library folder and what its subfolders hold.
#[derive(Clone, Debug, Default)]
pub struct EmbroideryLibraries {
    dir: Option<String>,
    pub borders: Libraries<BorderPattern>,
    pub fabrics: Libraries<Fabric>,
    pub hoops: Libraries<HoopList>,
    pub threads: Libraries<ThreadChart>,
}

impl EmbroideryLibraries {
    pub fn dir(&self) -> Option<&str> {
        self.dir.as_deref()
    }

    /// Set the embroidery library folder (its kinds' subfolders are made when something is first
    /// saved there) and list what it holds.
    pub fn set_dir(&mut self, dir: Option<String>) {
        let sub = |kind: &str| dir.as_ref().map(|d| std::path::Path::new(d).join(kind).to_string_lossy().to_string());
        self.borders.set_user_dir(sub("borders"));
        self.fabrics.set_user_dir(sub("fabrics"));
        self.hoops.set_user_dir(sub("hoops"));
        self.threads.set_user_dir(sub("threads"));
        self.dir = dir;
    }

    /// Re-read every subfolder.
    pub fn rescan(&mut self) {
        self.borders.rescan();
        self.fabrics.rescan();
        self.hoops.rescan();
        self.threads.rescan();
    }

    /// The entries of `kind` (every kind for `None`), each as `embroidery.libraries` lists it.
    pub fn list(&self, kind: Option<&str>) -> Vec<Value> {
        let want = |k: &str| kind.is_none_or(|w| w == k);
        let mut out = vec![];
        if want("borders") {
            out.extend(entries(
                &self.borders,
                "borders",
                |p| json!({"pattern": p.id, "author": p.author, "license": p.license, "source": p.source, "supported": p.unknown.is_none()}),
            ));
        }
        if want("fabrics") {
            out.extend(entries(
                &self.fabrics,
                "fabrics",
                |f| json!({"fabric": f.id, "author": f.author, "license": f.license, "source": f.source, "supported": f.unknown.is_none()}),
            ));
        }
        if want("hoops") {
            out.extend(entries(&self.hoops, "hoops", |l| json!({"hoops": l.hoops.iter().map(|h| h.id.as_str()).collect::<Vec<_>>()})));
        }
        if want("threads") {
            out.extend(entries(&self.threads, "threads", |c| {
                json!({"chart": c.id, "brand": c.brand, "threads": c.threads.len(), "author": c.author, "license": c.license, "source": c.source})
            }));
        }
        out
    }

    /// The files of `kind` (every kind for `None`) that couldn't be read, with why.
    pub fn skipped(&self, kind: Option<&str>) -> Vec<Value> {
        let lists = [
            ("borders", self.borders.skipped()),
            ("fabrics", self.fabrics.skipped()),
            ("hoops", self.hoops.skipped()),
            ("threads", self.threads.skipped()),
        ];
        lists
            .into_iter()
            .filter(|(k, _)| kind.is_none_or(|w| w == *k))
            .flat_map(|(k, files)| files.iter().map(move |(path, error)| json!({"kind": k, "path": path, "error": error})))
            .collect()
    }
}

/// The User Defined entries of `libs`, each with `kind`, id, name, path and `more`'s members.
fn entries<L: LibraryFile>(libs: &Libraries<L>, kind: &str, more: impl Fn(&L) -> Value) -> Vec<Value> {
    libs.infos()
        .filter(|i| i.category == "user")
        .filter_map(|i| {
            let (info, lib) = libs.get(&i.id)?;
            let mut v = json!({"kind": kind, "id": info.id, "name": info.name, "path": libs.path(&info.id)});
            if let (Some(v), Value::Object(extra)) = (v.as_object_mut(), more(&lib)) {
                v.extend(extra.into_iter().filter(|(_, x)| !x.is_null()));
            }
            Some(v)
        })
        .collect()
}

/// Library entry `p.library` of `libs` (an id from `embroidery.libraries`, or a name), or `None`
/// without `library`.
pub(crate) fn library_entry<L: LibraryFile>(libs: &Libraries<L>, p: &Value, cmd: &str) -> Result<Option<(LibraryInfo, Arc<L>)>> {
    let Some(key) = str_param(p, "library").map(str::trim) else { return Ok(None) };
    let found = libs.get(key).or_else(|| {
        let id = libs.infos().find(|i| i.category == "user" && i.name.eq_ignore_ascii_case(key))?.id.clone();
        libs.get(&id)
    });
    found.map(Some).ok_or_else(|| bad(cmd, format!("the embroidery library has no `{key}` (see embroidery.libraries)")))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "embroidery.libraries",
            "Embroidery Libraries",
            ["Window", "Embroidery"],
            None,
            "{kind?: borders|fabrics|hoops|threads (default: all)} what the embroidery library folder holds: border patterns (.vcborder), fabric profiles (.vcfabric), hoop lists (.vchoop) and thread charts (.vcthreads), one subfolder each, as last scanned (at start, embroidery.rescanLibraries, and after a save or add). Load one into the document with the load commands' `library` (embroidery.loadBorderPattern, loadFabric, loadHoop, loadThreadChart) → {folder: path | null (none here: the web, headless sessions), libraries: [{kind, id (user/<file>), name, path, and by kind: pattern, fabric, chart (the id it has inside), hoops: [ids], brand, threads (count), author, license, source, supported (false: from a newer VectorCraft)}], skipped: [{kind, path, error}] (files that couldn't be read)}",
            always,
            libraries
        ),
        cmd!(
            query "embroidery.rescanLibraries",
            "Rescan Embroidery Libraries",
            ["Window", "Embroidery"],
            None,
            "{} re-read the embroidery library folder (files added or removed outside VectorCraft) → the same as embroidery.libraries",
            always,
            rescan
        ),
        cmd!(
            "embroidery.addToLibrary",
            "Add to Embroidery Library…",
            ["Window", "Embroidery"],
            None,
            "{path? | data?: the file's text, name?: the file name of `data` (e.g. \"Rope.vcborder\"), kind?: borders|fabrics|hoops|threads (default: from the extension)} copy a .vcborder, .vcfabric, .vchoop or .vcthreads file into the embroidery library folder, after checking it reads. A name the folder already has gets a number (\"Rope 2.vcborder\") and no file is replaced; when the folder already holds the same file, that file is used → {kind, id, name, path, copied: false when nothing was written}",
            has_library_folder,
            add_to_library
        ),
    ]
}

fn always(_: &Session) -> std::result::Result<(), String> {
    Ok(())
}

fn has_library_folder(s: &Session) -> std::result::Result<(), String> {
    match s.embroidery_libraries.dir() {
        Some(_) => Ok(()),
        None => Err("no embroidery library folder here".into()),
    }
}

/// `p.kind`, checked; `None` when not given.
fn kind_param(p: &Value, cmd: &str) -> Result<Option<&'static str>> {
    let Some(k) = str_param(p, "kind") else { return Ok(None) };
    let k = k.trim().to_ascii_lowercase();
    KINDS
        .iter()
        .map(|(id, _)| *id)
        .find(|id| *id == k)
        .map(Some)
        .ok_or_else(|| bad(cmd, format!("`kind` must be one of {}", KINDS.iter().map(|(id, _)| *id).collect::<Vec<_>>().join(", "))))
}

fn libraries(s: &mut Session, p: &Value) -> Result<Value> {
    let kind = kind_param(p, "embroidery.libraries")?;
    let l = &s.embroidery_libraries;
    Ok(json!({"folder": l.dir(), "libraries": l.list(kind), "skipped": l.skipped(kind)}))
}

fn rescan(s: &mut Session, p: &Value) -> Result<Value> {
    s.embroidery_libraries.rescan();
    libraries(s, p)
}

fn add_to_library(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.addToLibrary";
    let (name, bytes) = match (str_param(p, "path"), str_param(p, "data")) {
        (Some(path), _) => (path.to_string(), super::fileio::read_file(path)?),
        (None, Some(text)) => {
            let name = str_param(p, "name").ok_or_else(|| bad(C, "with `data`, give its file `name` (e.g. \"Rope.vcborder\")"))?;
            (name.to_string(), text.as_bytes().to_vec())
        }
        _ => return Err(bad(C, "give `path` or `data`")),
    };
    let ext = super::fileio::extension(&name);
    let kind = match kind_param(p, C)? {
        Some(k) => k,
        None => KINDS
            .iter()
            .find(|(_, e)| *e == ext)
            .map(|(k, _)| *k)
            .ok_or_else(|| bad(C, format!("`{name}` isn't a .vcborder, .vcfabric, .vchoop or .vcthreads file (or give `kind`)")))?,
    };
    // A file named for another kind (or none) is stored under this kind's extension.
    let want = KINDS.iter().find(|(k, _)| *k == kind).map_or("", |(_, e)| *e);
    let name = if ext == want { name } else { format!("{}.{want}", super::fileio::file_stem(&name)) };
    let l = &mut s.embroidery_libraries;
    let (info, copied) = match kind {
        "borders" => l.borders.add_user_file(&name, &bytes, C).map(|(i, _, c)| (i, c))?,
        "fabrics" => l.fabrics.add_user_file(&name, &bytes, C).map(|(i, _, c)| (i, c))?,
        "hoops" => l.hoops.add_user_file(&name, &bytes, C).map(|(i, _, c)| (i, c))?,
        _ => l.threads.add_user_file(&name, &bytes, C).map(|(i, _, c)| (i, c))?,
    };
    let path = match kind {
        "borders" => l.borders.path(&info.id),
        "fabrics" => l.fabrics.path(&info.id),
        "hoops" => l.hoops.path(&info.id),
        _ => l.threads.path(&info.id),
    };
    Ok(json!({"kind": kind, "id": info.id, "name": info.name, "path": path, "copied": copied}))
}
