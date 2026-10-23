//! Object › Embroidery: stitch settings on paths and compound paths (`embroidery.set`,
//! `embroidery.clear`), the design they make (`embroidery.info`, `embroidery.stitches`, the
//! Stitch Preview), and DST files (`document.export {format: dst}`, `document.open` of a `.dst`;
//! see `fileio::embroidery`).

use serde_json::{Value, json};
use vectorcraft_doc::{Document, EmbroiderySetup, NodeId, NodeKind, StitchKind, StitchSettings};
use vectorcraft_embroidery::{Plan, params};

use super::overprint::edit_counted;
use super::*;

const SET: &str = "embroidery.set";
/// The most points `embroidery.stitches` returns unless asked for fewer.
const STITCHES_LIMIT: usize = 500_000;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: "embroidery.set",
            label: "Stitch Settings",
            menu: &["Object", "Embroidery"],
            shortcut: None,
            params: set_doc(),
            enabled: has_doc,
            run: set,
            journal: true,
        },
        cmd!(
            "embroidery.setup",
            "Embroidery Setup",
            ["Window", "Embroidery"],
            None,
            "{trimJumpsOver?: mm (0.5–100; 3) | null (never trim on length: only thread changes and objects' trimBefore/trimAfter cut the thread), stitchMovesUnder?: mm (0–5; 1: moves to the next stitch up to this long are sewn, longer ones jump), overlap?: mm (0–3; 0.5: how far a fill's rows still reach under objects sewn later on top of it; the rest of the covered rows is cut away), minStitch?: mm (0–2; 0.3: needle points closer than this to the one before are left out; the document's fabric may ask for longer), autoStart?: bool (true: running, bean and zigzag lines without a startAt are entered nearest where the needle is), hideTravel?: bool (true: travel under a fill or satin shape sewn later is sewn there as running stitches instead of jumped and trimmed), speed?: stitches per minute (100–1500; 600: the machine's, for the sewing time estimate), machineFormat?: dst|pes|jef|exp (dst: the file format the panels' Export button writes, for the machine the design is sewn on)} the document's travel, overlap, shortest stitch, speed and machine format, as one undo step → {trimJumpsOver, stitchMovesUnder, overlap, minStitch (as sewn: the larger of the document's and its fabric's), autoStart, hideTravel, speed, machineFormat}",
            has_doc,
            setup
        ),
        cmd!(
            query "embroidery.order",
            "Sew Order",
            ["Window", "Embroidery"],
            None,
            "{} the stitched objects in the order they are sewn → {order: [{id, name, stitch: type id, thread: \"#rrggbb\", stitches: how many it sews}], custom: bool (false: paint order)}. Whatever is sewn later lies on top on the fabric",
            has_doc,
            order
        ),
        cmd!(
            "embroidery.reorder",
            "Change Sew Order",
            ["Window", "Embroidery"],
            None,
            "{to: first|earlier|later|last, ids?} or {order: [ids]} move the stitched objects in ids or the selection (groups: those inside) in the sewing order (to), or set the order (those first, in that order; the others after, in paint order); one undo step → {order: [ids]}",
            has_doc,
            reorder
        ),
        cmd!(
            "embroidery.sortByThread",
            "Sew by Thread",
            ["Window", "Embroidery"],
            None,
            "{} group the sewing order by thread colour (colours in the order they first come, each keeping its objects' order), so each thread is sewn once where layering allows; one undo step → {order: [ids], colorChanges}",
            has_doc,
            sort_by_thread
        ),
        cmd!(
            "embroidery.optimizeOrder",
            "Optimize Sew Order",
            ["Window", "Embroidery"],
            None,
            "{} set the sewing order to cut travel: objects grouped by thread (colours in the order they first come), each thread's objects sewn nearest first from where the last one ended. What is sewn later lies on top, so check overlapping objects of one colour afterwards; one undo step → {order: [ids], travelBefore, travelAfter (mm the needle jumps), colorChanges}",
            has_doc,
            optimize_order
        ),
        cmd!(
            "embroidery.resetOrder",
            "Sew in Paint Order",
            ["Window", "Embroidery"],
            None,
            "{} sew the objects in paint order again (bottom first); one undo step → {order: [ids]}",
            has_doc,
            reset_order
        ),
        cmd!(
            query "embroidery.borderPatterns",
            "Border Patterns",
            ["Window", "Embroidery"],
            None,
            "{} the border patterns pattern borders can sew: the document's own (loaded from .vcborder files) then the starters → {patterns: [{id, name, builtin, supported (false: from a newer VectorCraft, kept but not usable; only id, name and used are given), width, length (mm, the pattern's own size), corner: has a corner motif, satin: has satin parts (sewn at the object's spacing), author, license, source, description, used: objects sewing with it}]}",
            has_doc,
            border_patterns
        ),
        cmd!(
            "embroidery.loadBorderPattern",
            "Load Border Pattern…",
            ["Window", "Embroidery"],
            None,
            "{path? | data?: the file's text} load a .vcborder border pattern file into the document (kept in it, so the document sews the same anywhere), as one undo step. Its id is the file's (else made from its name); one already taken by a different pattern gets a number (rope-2…), and a pattern the document (or the starters) already has is not added again → {id, name, added}",
            has_doc,
            load_border_pattern
        ),
        cmd!(
            "embroidery.extractBorderPattern",
            "Extract Border Pattern",
            ["Window", "Embroidery"],
            None,
            "{ids?, name?, period?: mm (skip the search for the repeat's length), author?, license?, source? (default: the document's title)} find the repeat of a stitched border in the selected manual-stitch paths (an opened DST design; with a closed unstitched path selected too, only the stitches inside it: draw one over the stretch to take) and add one repeat, joining end to end, to the document's border patterns (save it with embroidery.saveBorderPattern), as one undo step. Pick a straight stretch with two or more repeats → {id, name, period: the repeat's length, mm, width: mm, stitches: needle points a repeat, repeats: whole repeats found, error: mm the repeats differ from the pattern on average}",
            has_doc,
            extract_border_pattern
        ),
        cmd!(
            "embroidery.saveBorderPattern",
            "Save Border Pattern…",
            ["Window", "Embroidery"],
            None,
            "{pattern: id (the document's or a starter), path?} write a border pattern as a .vcborder file (JSON; specified in docs/formats/vcborder.md) → {path}, or without path {data: the file's text}",
            has_doc,
            save_border_pattern
        ),
        cmd!(
            "embroidery.removeBorderPattern",
            "Remove Border Pattern",
            ["Window", "Embroidery"],
            None,
            "{pattern: id} remove one of the document's border patterns, as one undo step (not one objects still sew with; starters can't be removed) → {removed: id}",
            has_doc,
            remove_border_pattern
        ),
        cmd!(
            "embroidery.clear",
            "Remove Stitches",
            ["Object", "Embroidery"],
            None,
            "{ids?} the objects in ids or the selection (groups: those inside) are no longer stitched, as one undo step → {changed}",
            has_doc,
            clear
        ),
        CommandSpec {
            id: "embroidery.info",
            label: "Embroidery Info",
            menu: &["Window", "Embroidery"],
            shortcut: None,
            params: info_doc(),
            enabled: has_doc,
            run: info,
            journal: false,
        },
        cmd!(
            query "embroidery.preview",
            "Render Stitch Preview",
            ["View"],
            None,
            "{scale?: pixels per point (0.01-16; 1), artboard?: index (0)} the artboard as Stitch Preview shows it, as a PNG: the stitches in their thread colours on fabric over the art, jumps as thin lines; the document is not changed → {width, height, pngBase64}",
            has_doc,
            preview
        ),
        cmd!(
            query "embroidery.check",
            "Check Design",
            ["Window", "Embroidery"],
            None,
            "{ids?: only findings about these objects (groups: those inside) and the design as a whole} run the design checker on the design as it is sewn → {findings: [{rule, severity: error|warning|note, message, object: id | null (the design), objectName, at: [x, y] | null (document points; null: the object as a whole)}] (errors first, then in sewing order), counts: {error, warning, note}, rules: [{id, label, help}]}",
            has_doc,
            check
        ),
        cmd!(
            query "embroidery.threadCharts",
            "Thread Charts",
            ["Window", "Embroidery"],
            None,
            "{} the thread charts loaded into the document (VectorCraft ships none: load charts you have the right to use) → {chart: id | null (the one the design's colours are matched against), charts: [{id, name, brand, weight, threads: count, author, license, source, description, supported}]}",
            has_doc,
            thread_charts
        ),
        cmd!(
            "embroidery.loadThreadChart",
            "Load Thread Chart…",
            ["Window", "Embroidery"],
            None,
            "{path? | data?: the file's text} load a .vcthreads thread chart file (specified in docs/formats/vcthreads.md) into the document, replacing one with its id, and match against it when none is chosen; one undo step → {id, name, threads}",
            has_doc,
            load_thread_chart
        ),
        cmd!(
            "embroidery.setThreadChart",
            "Thread Chart",
            ["Window", "Embroidery"],
            None,
            "{chart: id | null} the chart the design's colours are matched against (null: none); one undo step → {chart}",
            has_doc,
            set_thread_chart
        ),
        cmd!(
            "embroidery.removeThreadChart",
            "Remove Thread Chart",
            ["Window", "Embroidery"],
            None,
            "{chart: id} remove a thread chart from the document; one undo step → {removed}",
            has_doc,
            remove_thread_chart
        ),
        cmd!(
            "embroidery.saveThreadChart",
            "Save Thread Chart…",
            ["Window", "Embroidery"],
            None,
            "{chart: id, path?} write one of the document's thread charts as a .vcthreads file → {path}, or without path {data: the file's text}",
            has_doc,
            save_thread_chart
        ),
        cmd!(
            query "embroidery.matchThreads",
            "Match Threads",
            ["Window", "Embroidery"],
            None,
            "{chart?: id (default: the document's chosen chart)} each thread colour of the design (in sewing order) and its nearest thread in the chart, by CIEDE2000 colour difference (under about 2: hard to tell apart) → {chart, threads: [{color: \"#rrggbb\", code, name, threadColor, deltaE}]}",
            has_doc,
            match_threads
        ),
        cmd!(
            query "embroidery.hoops",
            "Hoops",
            ["Window", "Embroidery"],
            None,
            "{} the document's hoop and the common ones → {hoop: {id, name, width, height (the sewing field, mm), round, machines, description} | null (none chosen), common: [the same, for 4x4, 5x7, 6x10, 8x8, 8x12, cap], fit: fits|turned|tooBig|null (the design in the document's hoop; turned: only a quarter turn), designMm: [width, height] | null}",
            has_doc,
            hoops
        ),
        cmd!(
            "embroidery.setHoop",
            "Hoop",
            ["Window", "Embroidery"],
            None,
            "{hoop?: a common hoop's id (4x4|5x7|6x10|8x8|8x12|cap) or null (none), name?, width?, height? (mm, 10–2000: the sewing field), round?} set the document's hoop, whose sewing field the design must fit (checked; drawn in Stitch Preview, centred on the design): the hoop named (else the document's, else a 100 × 100 mm one), then the settings given; one undo step → the same as embroidery.hoops",
            has_doc,
            set_hoop
        ),
        cmd!(
            "embroidery.loadHoop",
            "Load Hoop…",
            ["Window", "Embroidery"],
            None,
            "{path? | data?: the file's text, hoop?: id (default: the file's first)} make a hoop from a .vchoop hoop list file (specified in docs/formats/vchoop.md) the document's hoop; one undo step → the same as embroidery.hoops, and hoops: [ids in the file]",
            has_doc,
            load_hoop
        ),
        cmd!(
            "embroidery.saveHoop",
            "Save Hoop…",
            ["Window", "Embroidery"],
            None,
            "{hoops?: [ids of common hoops] (default: the document's hoop), path?} write hoops as a .vchoop file → {path}, or without path {data: the file's text}",
            has_doc,
            save_hoop
        ),
        cmd!(
            query "embroidery.fabrics",
            "Fabric Profiles",
            ["Window", "Embroidery"],
            None,
            "{} the document's fabric and the starter profiles → {fabric: {id, name, author, license, source, description, satinSpacing, fillSpacing, pull, pullPercent, push, satinCenterUnder, satinZigzagOver, shapeEdgeOver, minStitch} | null (none: the stitches' own defaults), starters: [the same, for woven, knit, towel, cap, leather, denim]}. A fabric's settings fill in what objects' stitch settings leave unset",
            has_doc,
            fabrics
        ),
        cmd!(
            "embroidery.setFabric",
            "Fabric",
            ["Window", "Embroidery"],
            None,
            "{fabric?: a starter's id (woven|knit|towel|cap|leather|denim) or null (no fabric), name?, satinSpacing?, fillSpacing? (mm, 0.1–10), pull? (mm, 0–3), pullPercent? (%, 0–30), push? (mm, 0–3), satinCenterUnder?, satinZigzagOver?, shapeEdgeOver? (mm, 0.3–12; the auto underlay's thresholds), minStitch? (mm, 0–2; the shortest stitch it takes)} set the document's fabric, whose settings fill in what objects leave unset: the starter named (else the document's fabric, else an empty one), then the settings given (null clears one); one undo step → {fabric: the same as embroidery.fabrics | null}",
            has_doc,
            set_fabric
        ),
        cmd!(
            "embroidery.loadFabric",
            "Load Fabric…",
            ["Window", "Embroidery"],
            None,
            "{path? | data?: the file's text} load a .vcfabric fabric profile file (specified in docs/formats/vcfabric.md) as the document's fabric; one undo step → {fabric}",
            has_doc,
            load_fabric
        ),
        cmd!(
            "embroidery.saveFabric",
            "Save Fabric…",
            ["Window", "Embroidery"],
            None,
            "{fabric?: a starter's id (default: the document's fabric), path?} write a fabric profile as a .vcfabric file → {path}, or without path {data: the file's text}",
            has_doc,
            save_fabric
        ),
        cmd!(
            "embroidery.worksheet",
            "Production Worksheet",
            ["Window", "Embroidery"],
            None,
            "{page?: a4|letter (a4), format?: pdf, path?} a printable production worksheet for the design: its picture in stitches; size, stitches, thread changes, trims and jumps, hoop, fabric and sewing time; the thread sequence with colour chips, the chosen thread chart's matched threads and the thread each uses (estimate); a box for notes. Opened as a new document → {index, threads}; with format pdf, written to path → {path}, or without path {dataBase64}",
            has_doc,
            worksheet
        ),
        cmd!(
            "embroidery.testSheet",
            "New Test Sheet…",
            ["Window", "Embroidery"],
            None,
            "{sections?: [satinWidth|satinDensity|pullCompensation|fillDensity|fillAngle|underlay|lettering] (default: all)} a new document of calibration swatches to sew on real fabric: one artboard per section, each a 100 × 100 mm design whose swatches change one setting (the others at their defaults) and are labelled in stitches. Its sew-out form (embroidery.sewOut, embroidery.recordSewOut) lists the swatches; document.export {format: dst, artboard} writes one section → {index, title, sections: [{id, name, artboard, swatches}], swatches, skipped: [{section, reason}] (lettering needs a type face)}",
            always,
            test_sheet
        ),
        cmd!(
            query "embroidery.sewOut",
            "Sew-Out Test",
            ["Window", "Embroidery"],
            None,
            "{} the document's sew-out form: the test sheet's swatches (embroidery.testSheet) and what was seen when they were sewn → {sewOut: null (no test sheet) | {fabric, machine, notes, swatches: [{id (A3: the section's letter, the swatch's number), section, label, objects: [ids], result: pass|fail|untested, notes}], counts: {pass, fail, untested}}, sections: [{id, letter, name}]}",
            has_doc,
            sew_out
        ),
        cmd!(
            "embroidery.recordSewOut",
            "Record Sew-Out",
            ["Window", "Embroidery"],
            None,
            "{fabric?, machine?, notes?, results?: [{swatch: id (A3), result?: pass|fail|untested, notes?}]} record what the sew-out showed in the test sheet's form, as one undo step → {counts: {pass, fail, untested}}",
            has_doc,
            record_sew_out
        ),
        cmd!(
            query "embroidery.stitches",
            "Embroidery Stitches",
            ["Window", "Embroidery"],
            None,
            "{limit?: 500000 (most points)} the design's stitches as the machine sews them → {threads: [\"#rrggbb\"], runs: [{thread, points: [[x, y], …]}] (mm, document coordinates, y down; a run is sewn without a jump or trim), jumps: [[x0, y0, x1, y1]], truncated}",
            has_doc,
            stitches
        ),
    ]
}

/// `embroidery.set`'s description, made once from the settings table.
fn set_doc() -> &'static str {
    static DOC: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DOC.get_or_init(params::set_doc).as_str()
}

/// `embroidery.info`'s description, the selection's settings listed from the settings table.
fn info_doc() -> &'static str {
    static DOC: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DOC.get_or_init(|| {
        format!(
            "{{ids?}} → {{design: {{objects, stitches, jumps, trims, colorChanges, threads: [\"#rrggbb\"], widthMm, heightMm, warnings}}, setup: {{trimJumpsOver, stitchMovesUnder}} (embroidery.setup), selection: {{stitched, unstitched, {}}} (as sewn: the document's fabric fills in what objects leave unset; each null when the stitched objects differ, or none is; type is `unknown` for settings from a newer VectorCraft)}}",
            params::info_keys()
        )
    })
    .as_str()
}

pub(crate) fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

/// A thread colour: `"#rrggbb"` (or `rrggbb`) or `[r, g, b]` (0–255).
#[cfg(test)]
fn parse_thread(v: &Value) -> Option<[u8; 3]> {
    params::parse_thread(v)
}

/// The settings `embroidery.set` was given, each checked against its entry in the settings
/// table ([`params::PARAMS`]).
struct Edits(Vec<(&'static params::Param, Value)>);

impl Edits {
    fn parse(p: &Value) -> Result<Self> {
        let mut given = vec![];
        for param in params::PARAMS {
            if let Some(v) = p.get(param.key) {
                param.parse(v).map_err(|e| bad(SET, e))?;
                given.push((param, v.clone()));
            }
        }
        Ok(Self(given))
    }

    fn apply(&self, old: Option<&StitchSettings>) -> StitchSettings {
        // Settings from a newer version this one can't read are replaced, not edited. A newly
        // stitched object gets the automatic underlay.
        let fresh = || StitchSettings { underlay: vectorcraft_doc::Underlay::Auto, ..StitchSettings::default() };
        let mut s = old.filter(|o| o.unknown.is_none()).cloned().unwrap_or_else(fresh);
        for (param, v) in &self.0 {
            (param.set)(&mut s, v);
        }
        s
    }

    /// Whether the settings given suit `s`, the settings an object ends up with.
    fn suits(&self, s: &StitchSettings) -> Result<()> {
        for (param, _) in &self.0 {
            if let Some(check) = param.suits {
                check(s).map_err(|e| bad(SET, e))?;
            }
        }
        Ok(())
    }

    /// The border pattern asked for, if one is.
    fn pattern(&self) -> Option<String> {
        self.0.iter().find(|(p, _)| p.key == "pattern").and_then(|(_, v)| v.as_str()).map(|id| id.trim().to_ascii_lowercase())
    }
}

/// The paths, compound paths and type objects `ids` stand for (groups and layers: those inside;
/// not the paths inside a compound path, which stitches as one), guides left out.
fn stitchable(s: &Session, ids: &[NodeId]) -> Result<Vec<NodeId>> {
    let d = &s.doc()?.doc;
    Ok(leaf_targets(s, ids)?
        .into_iter()
        .filter(|id| {
            matches!(d.node(*id).map(|n| &n.kind), Some(NodeKind::Path { guide: false, .. } | NodeKind::Compound { .. } | NodeKind::Text(_)))
        })
        .collect())
}

fn set(s: &mut Session, p: &Value) -> Result<Value> {
    let edits = Edits::parse(p)?;
    let ids = stitchable(s, &targets(s, p)?)?;
    if ids.is_empty() {
        return Err(bad(SET, "select paths, compound paths or type to stitch"));
    }
    // Each setting must suit the stitch every target ends up with (an underlay its type uses, a
    // width in its range).
    {
        let d = &s.doc()?.doc;
        for id in &ids {
            edits.suits(&edits.apply(d.node(*id).and_then(|n| n.extras().stitch.as_deref())))?;
        }
    }
    if let Some(id) = edits.pattern() {
        let mine = &s.doc()?.doc.embroidery.patterns;
        if vectorcraft_embroidery::borders::find(mine, &id).is_none() {
            let ids: Vec<&str> = mine.iter().map(|p| p.id.as_str()).chain(vectorcraft_embroidery::borders::BUILTIN).collect();
            return Err(bad(SET, format!("no border pattern `{id}` (there are {}; embroidery.loadBorderPattern adds one)", ids.join(", "))));
        }
    }
    let changed = edit_counted(s, "Stitch Settings", |d| {
        let mut n = 0;
        for id in &ids {
            let Some(node) = d.node_mut(*id) else { continue };
            let new = edits.apply(node.extras().stitch.as_deref());
            if node.extras().stitch.as_deref() != Some(&new) {
                node.extras_mut().stitch = Some(Box::new(new));
                n += 1;
            }
        }
        Ok(n)
    })?;
    Ok(json!({ "changed": changed }))
}

fn setup_json(e: &EmbroiderySetup) -> Value {
    json!({ "trimJumpsOver": e.trim_over(), "stitchMovesUnder": e.stitch_under(), "overlap": e.overlap(), "minStitch": e.min_stitch(), "autoStart": e.auto_start, "hideTravel": e.hide_travel, "speed": e.speed(), "machineFormat": e.machine_format() })
}

fn setup(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.setup";
    let mut new = s.doc()?.doc.embroidery.clone();
    match p.get("trimJumpsOver") {
        None => {}
        Some(Value::Null) => new.trim_jumps_over = None,
        Some(v) => match v.as_f64() {
            Some(x) if EmbroiderySetup::TRIM.contains(&x) => new.trim_jumps_over = Some(x),
            _ => return Err(bad(C, "`trimJumpsOver` must be a number from 0.5 to 100 (mm), or null (never trim on length)")),
        },
    }
    if let Some(v) = p.get("stitchMovesUnder") {
        match v.as_f64() {
            Some(x) if EmbroiderySetup::STITCH_MOVES.contains(&x) => new.stitch_moves_under = x,
            _ => return Err(bad(C, "`stitchMovesUnder` must be a number from 0 to 5 (mm)")),
        }
    }
    if let Some(v) = p.get("speed") {
        match v.as_f64() {
            Some(x) if EmbroiderySetup::SPEED.contains(&x) => new.speed = x,
            _ => return Err(bad(C, "`speed` must be a number from 100 to 1500 (stitches per minute)")),
        }
    }
    if let Some(v) = p.get("machineFormat") {
        match v.as_str().map(str::to_ascii_lowercase) {
            Some(f) if EmbroiderySetup::MACHINE_FORMATS.contains(&f.as_str()) => new.machine_format = f,
            _ => return Err(bad(C, "`machineFormat` must be dst, pes, jef or exp")),
        }
    }
    if let Some(v) = p.get("hideTravel") {
        new.hide_travel = v.as_bool().ok_or_else(|| bad(C, "`hideTravel` must be true or false"))?;
    }
    if let Some(v) = p.get("autoStart") {
        new.auto_start = v.as_bool().ok_or_else(|| bad(C, "`autoStart` must be true or false"))?;
    }
    if let Some(v) = p.get("minStitch") {
        match v.as_f64() {
            Some(x) if EmbroiderySetup::MIN_STITCH.contains(&x) => new.min_stitch = x,
            _ => return Err(bad(C, "`minStitch` must be a number from 0 to 2 (mm)")),
        }
    }
    if let Some(v) = p.get("overlap") {
        match v.as_f64() {
            Some(x) if EmbroiderySetup::OVERLAP.contains(&x) => new.overlap = x,
            _ => return Err(bad(C, "`overlap` must be a number from 0 to 3 (mm)")),
        }
    }
    if new != s.doc()?.doc.embroidery {
        let e = new.clone();
        edit_counted(s, "Embroidery Setup", |d| {
            d.embroidery = e;
            Ok(1)
        })?;
    }
    Ok(setup_json(&new))
}

/// How many objects of `doc` sew pattern `id`.
fn pattern_users(doc: &Document, id: &str) -> usize {
    let mut n = 0;
    doc.walk(|node| n += usize::from(node.extras().stitch.as_deref().is_some_and(|s| s.kind == StitchKind::PatternBorder && s.pattern() == id)));
    n
}

fn border_patterns(s: &mut Session, _: &Value) -> Result<Value> {
    use vectorcraft_embroidery::borders;
    let doc = &s.doc()?.doc;
    let starters = borders::BUILTIN.iter().filter_map(|id| borders::builtin(id)).map(|p| (p, true));
    let list: Vec<Value> = doc
        .embroidery
        .patterns
        .iter()
        .cloned()
        .map(|p| (p, false))
        .chain(starters)
        .map(|(p, builtin)| {
            if p.unknown.is_some() {
                // From a newer version: listed so it can be removed, but it can't be used.
                return json!({"id": p.id, "name": p.name, "builtin": false, "supported": false, "used": pattern_users(doc, &p.id)});
            }
            json!({
                "supported": true,
                "id": p.id, "name": p.name, "builtin": builtin, "width": p.width, "length": p.length,
                "corner": !p.corner.is_empty(), "author": p.author,
                "satin": p.repeat.iter().chain(&p.corner).any(|q| q.stitch == vectorcraft_doc::PartStitch::Satin), "license": p.license, "source": p.source,
                "description": p.description, "used": pattern_users(doc, &p.id),
            })
        })
        .collect();
    Ok(json!({ "patterns": list }))
}

fn load_border_pattern(s: &mut Session, p: &Value) -> Result<Value> {
    use vectorcraft_embroidery::borders;
    const C: &str = "embroidery.loadBorderPattern";
    let bytes = match (str_param(p, "path"), str_param(p, "data")) {
        (Some(path), _) => super::fileio::read_file(path)?,
        (None, Some(text)) => text.as_bytes().to_vec(),
        _ => return Err(bad(C, "give `path` or `data`")),
    };
    let pat = borders::read_file(&bytes).map_err(|e| bad(C, e))?;
    let (id, added) = add_pattern(s, pat.clone(), "Load Border Pattern", C)?;
    Ok(json!({"id": id, "name": pat.name, "added": added}))
}

/// Add `pat` to the document's border patterns as undo step `label`: under its id, else (the id
/// taken by a different pattern, or a starter's) a numbered one. The very same pattern already
/// there isn't added again. → (its id, whether it was added).
fn add_pattern(s: &mut Session, mut pat: vectorcraft_doc::BorderPattern, label: &str, cmd: &str) -> Result<(String, bool)> {
    use vectorcraft_embroidery::borders;
    let mine = &s.doc()?.doc.embroidery.patterns;
    let same = |q: &vectorcraft_doc::BorderPattern| q.id == pat.id && *q == pat;
    if mine.iter().any(same) || borders::builtin(&pat.id).is_some_and(|q| same(&q)) {
        return Ok((pat.id, false));
    }
    let taken = |id: &str| borders::BUILTIN.contains(&id) || mine.iter().any(|q| q.id == id);
    if taken(&pat.id) {
        let base = pat.id.clone();
        let Some(id) = (2..10_000).map(|n| format!("{base}-{n}")).find(|id| !taken(id)) else {
            return Err(bad(cmd, format!("too many patterns named `{base}`")));
        };
        pat.id = id;
    }
    let id = pat.id.clone();
    edit_counted(s, label, |d| {
        d.embroidery.patterns.push(pat);
        Ok(1)
    })?;
    Ok((id, true))
}

fn extract_border_pattern(s: &mut Session, p: &Value) -> Result<Value> {
    use vectorcraft_embroidery::{extract, generate};
    const C: &str = "embroidery.extractBorderPattern";
    let period = match p.get("period") {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            v.as_f64().filter(|t| t.is_finite() && *t > 0.0 && *t <= 1000.0).ok_or_else(|| bad(C, "`period` must be a length in mm (up to 1000)"))?,
        ),
    };
    let text = |key: &str| str_param(p, key).map(str::trim).filter(|t| !t.is_empty()).map(str::to_string);
    let ids = leaf_targets(s, &targets(s, p)?)?;
    let st = s.doc()?;
    let doc = &st.doc;
    // The stitches: manual-stitch paths (an opened design); the stretch: any other closed path
    // selected with them, drawn over the part to take.
    let (mut runs, mut region) = (vec![], vec![]);
    for id in &ids {
        let Some(n) = doc.node(*id) else { continue };
        let Some(path) = n.path_data() else { continue };
        if n.extras().stitch.as_deref().is_some_and(|t| t.kind == StitchKind::Manual) {
            runs.extend(generate::manual(path));
        } else {
            region.extend(generate::polylines(path).into_iter().filter(|(_, closed)| *closed).map(|(l, _)| l));
        }
    }
    if runs.is_empty() {
        return Err(bad(
            C,
            "select the stitches of a border (an opened DST design's manual stitches), and optionally a shape over the stretch to take",
        ));
    }
    let pts = extract::stretch(&runs, (!region.is_empty()).then_some(region.as_slice()));
    let title = st.title();
    let name = text("name").unwrap_or_else(|| format!("{} border", title.rsplit_once('.').map_or(title.as_str(), |(stem, _)| stem)));
    let mut e = extract::border(&pts, &name, period).map_err(|e| bad(C, e))?;
    e.pattern.author = text("author");
    e.pattern.license = text("license");
    e.pattern.source = Some(text("source").unwrap_or_else(|| format!("extracted from {title}")));
    let (id, _) = add_pattern(s, e.pattern.clone(), "Extract Border Pattern", C)?;
    let r = |v: f64| (v * 100.0).round() / 100.0;
    Ok(json!({
        "id": id, "name": e.pattern.name, "period": r(e.period), "width": r(e.pattern.width), "stitches": e.stitches,
        "repeats": e.repeats, "error": r(e.error),
    }))
}

/// The `pattern` param: a border pattern the document can sew.
fn pattern_param(s: &Session, p: &Value, cmd: &str) -> Result<vectorcraft_doc::BorderPattern> {
    let id =
        str_param(p, "pattern").map(|id| id.trim().to_ascii_lowercase()).ok_or_else(|| bad(cmd, "`pattern` (a border pattern id) is missing"))?;
    vectorcraft_embroidery::borders::find(&s.doc()?.doc.embroidery.patterns, &id).ok_or_else(|| bad(cmd, format!("no border pattern `{id}`")))
}

fn save_border_pattern(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.saveBorderPattern";
    let pat = pattern_param(s, p, C)?;
    let text = vectorcraft_embroidery::borders::write_file(&pat).map_err(|e| bad(C, e))?;
    match str_param(p, "path") {
        Some(path) => {
            super::fileio::write_file(path, text.as_bytes())?;
            Ok(json!({"path": path}))
        }
        None => Ok(json!({"data": text})),
    }
}

fn remove_border_pattern(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.removeBorderPattern";
    // Found among the document's own, readable or not (one from a newer version can be removed).
    let id = str_param(p, "pattern").map(|id| id.trim().to_ascii_lowercase()).ok_or_else(|| bad(C, "`pattern` (a border pattern id) is missing"))?;
    let Some(pat) = s.doc()?.doc.embroidery.patterns.iter().find(|q| q.id == id).cloned() else {
        let why = if vectorcraft_embroidery::borders::builtin(&id).is_some() {
            "is a starter pattern: it can't be removed"
        } else {
            "isn't one of the document's patterns"
        };
        return Err(bad(C, format!("`{id}` {why}")));
    };
    let users = pattern_users(&s.doc()?.doc, &pat.id);
    if users > 0 {
        return Err(bad(C, format!("{users} object(s) sew with `{}`: give them another pattern first", pat.id)));
    }
    let id = pat.id.clone();
    edit_counted(s, "Remove Border Pattern", |d| {
        d.embroidery.patterns.retain(|q| q.id != id);
        Ok(1)
    })?;
    Ok(json!({"removed": pat.id}))
}

/// An object's name for the sewing order: its own, else what it is.
fn object_name(n: &vectorcraft_doc::Node) -> String {
    if let Some(name) = n.name.as_deref().filter(|s| !s.is_empty()) {
        return name.to_string();
    }
    match &n.kind {
        NodeKind::Text(t) => {
            let text: String = t.plain_text().chars().take(24).collect();
            format!("\u{201c}{}\u{201d}", text.trim())
        }
        NodeKind::Compound { .. } => "Compound Path".into(),
        _ => "Path".into(),
    }
}

/// The ids of the objects `doc` sews, in sewing order.
fn sewn_ids(doc: &Document) -> Vec<NodeId> {
    vectorcraft_embroidery::sew_order(doc).iter().map(|n| n.id).collect()
}

fn order(s: &mut Session, _: &Value) -> Result<Value> {
    let doc = &s.doc()?.doc;
    let plan = s.stitch_plan(doc);
    let count = |id: NodeId| {
        plan.objects.iter().find(|o| o.id == id).map_or(0, |o| {
            plan.needles.get(o.needles.clone()).unwrap_or_default().iter().filter(|n| n.cmd == vectorcraft_embroidery::Cmd::Stitch).count()
        })
    };
    let list: Vec<Value> = vectorcraft_embroidery::sew_order(doc)
        .into_iter()
        .map(|n| {
            json!({
                "id": n.id.0,
                "name": object_name(n),
                "stitch": n.extras().stitch.as_deref().map(StitchSettings::kind_id),
                "thread": hex(vectorcraft_embroidery::thread_of_node(n)),
                "stitches": count(n.id),
            })
        })
        .collect();
    Ok(json!({ "order": list, "custom": !doc.embroidery.order.is_empty() }))
}

/// Set the sewing order to `ids` (one undo step), returning it.
fn set_order(s: &mut Session, label: &str, ids: Vec<NodeId>) -> Result<Value> {
    let changed = s.doc()?.doc.embroidery.order != ids;
    if changed {
        let ids = ids.clone();
        edit_counted(s, label, |d| {
            d.embroidery.order = ids;
            Ok(1)
        })?;
    }
    Ok(json!({ "order": ids.iter().map(|i| i.0).collect::<Vec<_>>() }))
}

fn reorder(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.reorder";
    let current = sewn_ids(&s.doc()?.doc);
    if let Some(list) = p.get("order") {
        let ids: Vec<NodeId> = list
            .as_array()
            .ok_or_else(|| bad(C, "`order` must be a list of object ids"))?
            .iter()
            .map(|v| v.as_u64().map(NodeId).ok_or_else(|| bad(C, "`order` must be a list of object ids")))
            .collect::<Result<_>>()?;
        if let Some(id) = ids.iter().find(|id| !current.contains(id)) {
            return Err(bad(C, format!("object {} isn't a stitched object", id.0)));
        }
        let mut new: Vec<NodeId> = vec![];
        for id in ids {
            if !new.contains(&id) {
                new.push(id);
            }
        }
        let rest: Vec<NodeId> = current.iter().filter(|id| !new.contains(id)).copied().collect();
        new.extend(rest);
        return set_order(s, "Change Sew Order", new);
    }
    let to = str_param(p, "to").ok_or_else(|| bad(C, "give `to` (first, earlier, later, last) or `order`"))?;
    let picked: Vec<NodeId> = stitchable(s, &targets(s, p)?)?.into_iter().filter(|id| current.contains(id)).collect();
    if picked.is_empty() {
        return Err(bad(C, "select stitched objects to move in the sewing order"));
    }
    let is = |id: &NodeId| picked.contains(id);
    let mut new = current.clone();
    match to {
        "first" => new = current.iter().filter(|i| is(i)).chain(current.iter().filter(|i| !is(i))).copied().collect(),
        "last" => new = current.iter().filter(|i| !is(i)).chain(current.iter().filter(|i| is(i))).copied().collect(),
        // One place each, a block of moved objects keeping together.
        "earlier" => {
            for k in 1..new.len() {
                if new.get(k).is_some_and(is) && new.get(k - 1).is_some_and(|i| !is(i)) {
                    new.swap(k, k - 1);
                }
            }
        }
        "later" => {
            for k in (0..new.len().saturating_sub(1)).rev() {
                if new.get(k).is_some_and(is) && new.get(k + 1).is_some_and(|i| !is(i)) {
                    new.swap(k, k + 1);
                }
            }
        }
        _ => return Err(bad(C, "`to` must be first, earlier, later or last")),
    }
    set_order(s, "Change Sew Order", new)
}

fn sort_by_thread(s: &mut Session, _: &Value) -> Result<Value> {
    let doc = &s.doc()?.doc;
    let sewn: Vec<(NodeId, [u8; 3])> =
        vectorcraft_embroidery::sew_order(doc).into_iter().map(|n| (n.id, vectorcraft_embroidery::thread_of_node(n))).collect();
    let mut threads: Vec<[u8; 3]> = vec![];
    for (_, t) in &sewn {
        if !threads.contains(t) {
            threads.push(*t);
        }
    }
    let new: Vec<NodeId> = threads.iter().flat_map(|t| sewn.iter().filter(move |(_, u)| u == t).map(|(id, _)| *id)).collect();
    let mut r = set_order(s, "Sew by Thread", new)?;
    r["colorChanges"] = json!(threads.len().saturating_sub(1));
    Ok(r)
}

/// The most objects `embroidery.optimizeOrder` orders (it compares every pair).
const OPTIMIZE_MAX: usize = 5000;

/// An object as sewn: its id, its thread, and where its stitches start and end (none: it makes
/// none).
type Sewn = (NodeId, [u8; 3], Option<(kurbo::Point, kurbo::Point)>);

/// How far the needle jumps in `plan`, mm.
fn travel_mm(plan: &Plan) -> f64 {
    let mut at: Option<(f64, f64)> = None;
    let mut sum = 0.0;
    for n in &plan.needles {
        if n.cmd == vectorcraft_embroidery::Cmd::Jump
            && let Some((x, y)) = at
        {
            sum += (n.x - x).hypot(n.y - y);
        }
        at = Some((n.x, n.y));
    }
    sum
}

fn optimize_order(s: &mut Session, _: &Value) -> Result<Value> {
    use kurbo::Point;
    const C: &str = "embroidery.optimizeOrder";
    let doc = s.doc()?.doc.clone();
    let plan = s.stitch_plan(&doc);
    if plan.objects.len() > OPTIMIZE_MAX {
        return Err(bad(C, format!("the design has {} objects: at most {OPTIMIZE_MAX} are ordered", plan.objects.len())));
    }
    // Each object: its thread, and where its stitches start and end (none: it makes none).
    let stitch = |i: usize| plan.needles.get(i).filter(|n| n.cmd == vectorcraft_embroidery::Cmd::Stitch).map(|n| Point::new(n.x, n.y));
    let objs: Vec<Sewn> = plan
        .objects
        .iter()
        .map(|o| {
            let thread = doc.node(o.id).map_or([0; 3], vectorcraft_embroidery::thread_of_node);
            let first = o.needles.clone().find_map(stitch);
            let last = o.needles.clone().rev().find_map(stitch);
            (o.id, thread, first.zip(last))
        })
        .collect();
    let mut threads: Vec<[u8; 3]> = vec![];
    for (_, t, _) in &objs {
        if !threads.contains(t) {
            threads.push(*t);
        }
    }
    let mut order: Vec<NodeId> = vec![];
    let mut at: Option<Point> = None;
    for t in &threads {
        let mut left: Vec<&Sewn> = objs.iter().filter(|o| o.1 == *t).collect();
        while !left.is_empty() {
            // The nearest start (or end: lines are entered from either) to where the needle is.
            let pick = match at {
                None => 0,
                Some(p) => left
                    .iter()
                    .enumerate()
                    .map(|(i, o)| (i, o.2.map_or(f64::INFINITY, |(a, b)| a.distance(p).min(b.distance(p)))))
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map_or(0, |(i, _)| i),
            };
            let o = left.remove(pick.min(left.len().saturating_sub(1)));
            if let Some((a, b)) = o.2 {
                at = Some(match at {
                    Some(p) if b.distance(p) < a.distance(p) => a,
                    _ => b,
                });
            }
            order.push(o.0);
        }
    }
    let before = travel_mm(&plan);
    let mut r = set_order(s, "Optimize Sew Order", order)?;
    let after = travel_mm(&s.stitch_plan(&s.doc()?.doc));
    let round = |v: f64| (v * 10.0).round() / 10.0;
    r["travelBefore"] = json!(round(before));
    r["travelAfter"] = json!(round(after));
    r["colorChanges"] = json!(threads.len().saturating_sub(1));
    Ok(r)
}

fn reset_order(s: &mut Session, _: &Value) -> Result<Value> {
    let r = set_order(s, "Sew in Paint Order", vec![])?;
    Ok(
        json!({ "order": sewn_ids(&s.doc()?.doc).iter().map(|i| i.0).collect::<Vec<_>>(), "custom": r["order"].as_array().is_some_and(|a| !a.is_empty()) }),
    )
}

fn clear(s: &mut Session, p: &Value) -> Result<Value> {
    let ids = leaf_targets(s, &targets(s, p)?)?;
    let changed = edit_counted(s, "Remove Stitches", |d| {
        let mut n = 0;
        for id in &ids {
            if let Some(node) = d.node_mut(*id)
                && node.extras_mut().stitch.take().is_some()
            {
                n += 1;
            }
        }
        Ok(n)
    })?;
    Ok(json!({ "changed": changed }))
}

/// The one value every item shares (`null` when they differ or there are none).
fn same<T: PartialEq + serde::Serialize>(mut it: impl Iterator<Item = T>) -> Value {
    let Some(first) = it.next() else { return Value::Null };
    if it.all(|v| v == first) { json!(first) } else { Value::Null }
}

/// `doc`'s design summary.
pub(crate) fn design(doc: &Document, plan: &Plan) -> Value {
    let st = plan.stats();
    let mut objects = 0;
    doc.walk(|n| objects += usize::from(n.extras().stitch.is_some()));
    let (w, h) = st.bounds.map(|b| (b[2] - b[0], b[3] - b[1])).unwrap_or((0.0, 0.0));
    json!({
        "objects": objects,
        "stitches": st.stitches,
        "jumps": st.jumps,
        "trims": st.trims,
        "colorChanges": st.color_changes,
        "threads": plan.threads.iter().map(|t| hex(*t)).collect::<Vec<_>>(),
        "widthMm": (w * 10.0).round() / 10.0,
        "heightMm": (h * 10.0).round() / 10.0,
        "warnings": plan.warnings,
        "minutes": (vectorcraft_embroidery::sewing_seconds(plan, doc.embroidery.speed()) / 6.0).round() / 10.0,
    })
}

fn info(s: &mut Session, p: &Value) -> Result<Value> {
    let ids = stitchable(s, &targets(s, p)?)?;
    let doc = &s.doc()?.doc;
    let plan = s.stitch_plan(doc);
    // Reported as sewn: the document's fabric fills in what the objects leave unset.
    let fabric = doc.embroidery.fabric.as_ref().filter(|f| f.unknown.is_none());
    let all: Vec<StitchSettings> = ids
        .iter()
        .filter_map(|id| doc.node(*id)?.extras().stitch.as_deref())
        .map(|s| fabric.map_or_else(|| s.clone(), |f| vectorcraft_embroidery::fabrics::fill_in(s, f)))
        .collect();
    let mut selection = json!({ "stitched": all.len(), "unstitched": ids.len() - all.len() });
    for param in params::PARAMS {
        selection[param.key] = same(all.iter().map(|s| (param.get)(s)));
    }
    Ok(json!({ "design": design(doc, &plan), "setup": setup_json(&doc.embroidery), "selection": selection }))
}

fn check(s: &mut Session, p: &Value) -> Result<Value> {
    use vectorcraft_embroidery::check;
    let only = match ids_param(p, "ids") {
        Some(ids) => Some(leaf_targets(s, &ids)?),
        None => None,
    };
    let doc = &s.doc()?.doc;
    let plan = s.stitch_plan(doc);
    let findings: Vec<check::Finding> = check::check(doc, &plan)
        .into_iter()
        .filter(|f| match (&only, f.object) {
            (Some(ids), Some(id)) => ids.contains(&id),
            _ => true,
        })
        .collect();
    let pt = |v: f64| (v / vectorcraft_embroidery::PT_MM * 100.0).round() / 100.0;
    let list: Vec<Value> = findings
        .iter()
        .map(|f| {
            json!({
                "rule": f.rule, "severity": f.severity.id(), "message": f.message,
                "object": f.object.map(|id| id.0),
                "objectName": f.object.and_then(|id| doc.node(id)).map(object_name),
                "at": f.at.map(|a| [pt(a.x), pt(a.y)]),
            })
        })
        .collect();
    let count = |sev: check::Severity| findings.iter().filter(|f| f.severity == sev).count();
    let rules: Vec<Value> = check::RULES.iter().map(|r| json!({"id": r.id, "label": r.label, "help": r.help})).collect();
    Ok(json!({
        "findings": list,
        "counts": {"error": count(check::Severity::Error), "warning": count(check::Severity::Warning), "note": count(check::Severity::Note)},
        "rules": rules,
    }))
}

fn thread_charts(s: &mut Session, _: &Value) -> Result<Value> {
    let e = &s.doc()?.doc.embroidery;
    let charts: Vec<Value> = e
        .charts
        .iter()
        .map(|c| {
            json!({
                "id": c.id, "name": c.name, "brand": c.brand, "weight": c.weight, "threads": c.threads.len(),
                "author": c.author, "license": c.license, "source": c.source, "description": c.description,
                "supported": c.unknown.is_none(),
            })
        })
        .collect();
    Ok(json!({"chart": e.chart, "charts": charts}))
}

fn load_thread_chart(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.loadThreadChart";
    let bytes = match (str_param(p, "path"), str_param(p, "data")) {
        (Some(path), _) => super::fileio::read_file(path)?,
        (None, Some(text)) => text.as_bytes().to_vec(),
        _ => return Err(bad(C, "give `path` or `data`")),
    };
    let chart = vectorcraft_embroidery::threads::read_file(&bytes).map_err(|e| bad(C, e))?;
    let out = json!({"id": chart.id, "name": chart.name, "threads": chart.threads.len()});
    edit_counted(s, "Load Thread Chart", |d| {
        let e = &mut d.embroidery;
        e.charts.retain(|c| c.id != chart.id);
        if e.chart.is_none() {
            e.chart = Some(chart.id.clone());
        }
        e.charts.push(chart);
        Ok(1)
    })?;
    Ok(out)
}

fn set_thread_chart(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.setThreadChart";
    let chart = match p.get("chart") {
        Some(Value::Null) => None,
        Some(Value::String(id)) => {
            let id = id.trim().to_string();
            if !s.doc()?.doc.embroidery.charts.iter().any(|c| c.id == id && c.unknown.is_none()) {
                return Err(bad(C, format!("the document has no thread chart `{id}` (embroidery.loadThreadChart adds one)")));
            }
            Some(id)
        }
        _ => return Err(bad(C, "`chart` must be a chart's id, or null")),
    };
    if s.doc()?.doc.embroidery.chart != chart {
        let c = chart.clone();
        edit_counted(s, "Thread Chart", |d| {
            d.embroidery.chart = c;
            Ok(1)
        })?;
    }
    Ok(json!({"chart": chart}))
}

fn remove_thread_chart(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.removeThreadChart";
    let id = str_param(p, "chart").map(str::trim).ok_or_else(|| bad(C, "`chart` (a chart's id) is missing"))?.to_string();
    if !s.doc()?.doc.embroidery.charts.iter().any(|c| c.id == id) {
        return Err(bad(C, format!("the document has no thread chart `{id}`")));
    }
    let gone = id.clone();
    edit_counted(s, "Remove Thread Chart", |d| {
        d.embroidery.charts.retain(|c| c.id != gone);
        if d.embroidery.chart.as_deref() == Some(gone.as_str()) {
            d.embroidery.chart = None;
        }
        Ok(1)
    })?;
    Ok(json!({"removed": id}))
}

fn save_thread_chart(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.saveThreadChart";
    let id = str_param(p, "chart").map(str::trim).ok_or_else(|| bad(C, "`chart` (a chart's id) is missing"))?;
    let chart = s
        .doc()?
        .doc
        .embroidery
        .charts
        .iter()
        .find(|c| c.id == id && c.unknown.is_none())
        .cloned()
        .ok_or_else(|| bad(C, format!("the document has no thread chart `{id}`")))?;
    let text = vectorcraft_embroidery::threads::write_file(&chart).map_err(|e| bad(C, e))?;
    match str_param(p, "path") {
        Some(path) => {
            super::fileio::write_file(path, text.as_bytes())?;
            Ok(json!({"path": path}))
        }
        None => Ok(json!({"data": text})),
    }
}

fn match_threads(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.matchThreads";
    let doc = &s.doc()?.doc;
    let id = match str_param(p, "chart") {
        Some(id) => Some(id.trim().to_string()),
        None => doc.embroidery.chart.clone(),
    };
    let Some(id) = id else { return Err(bad(C, "no thread chart chosen: load one (embroidery.loadThreadChart) or name it (`chart`)")) };
    let chart = doc
        .embroidery
        .charts
        .iter()
        .find(|c| c.id == id && c.unknown.is_none())
        .ok_or_else(|| bad(C, format!("the document has no thread chart `{id}`")))?
        .clone();
    let plan = s.stitch_plan(&s.doc()?.doc);
    let threads: Vec<Value> = plan
        .threads
        .iter()
        .map(|t| match vectorcraft_embroidery::threads::nearest(&chart, *t) {
            Some((n, d)) => json!({"color": hex(*t), "code": n.code, "name": n.name, "threadColor": n.color, "deltaE": (d * 10.0).round() / 10.0}),
            None => json!({"color": hex(*t), "code": null, "name": null, "threadColor": null, "deltaE": null}),
        })
        .collect();
    Ok(json!({"chart": chart.id, "threads": threads}))
}

fn hoop_json(h: Option<&vectorcraft_doc::Hoop>) -> Value {
    match h {
        Some(vectorcraft_doc::Hoop { unknown: Some(raw), .. }) => json!({"unsupported": true, "data": raw}),
        Some(h) => serde_json::to_value(h).unwrap_or(Value::Null),
        None => Value::Null,
    }
}

fn hoops(s: &mut Session, _: &Value) -> Result<Value> {
    use vectorcraft_embroidery::hoops;
    let doc = &s.doc()?.doc;
    let common: Vec<Value> = hoops::BUILTIN.iter().filter_map(|id| hoops::builtin(id)).map(|h| hoop_json(Some(&h))).collect();
    let plan = s.stitch_plan(doc);
    let size = plan.stats().bounds.map(|b| (b[2] - b[0], b[3] - b[1]));
    let fit = match (doc.embroidery.hoop.as_ref().filter(|h| h.unknown.is_none()), size) {
        (Some(h), Some((w, hh))) => json!(match hoops::fit(w, hh, h) {
            hoops::Fit::Fits => "fits",
            hoops::Fit::FitsTurned => "turned",
            hoops::Fit::TooBig => "tooBig",
        }),
        _ => Value::Null,
    };
    let r = |v: f64| (v * 10.0).round() / 10.0;
    Ok(json!({
        "hoop": hoop_json(doc.embroidery.hoop.as_deref()),
        "common": common,
        "fit": fit,
        "designMm": size.map(|(w, h)| [r(w), r(h)]),
    }))
}

/// Make `h` the document's hoop (`None`: none) as undo step `label`; the hoops listing after.
fn put_hoop(s: &mut Session, h: Option<vectorcraft_doc::Hoop>, label: &str) -> Result<Value> {
    let h = h.map(Box::new);
    if s.doc()?.doc.embroidery.hoop != h {
        edit_counted(s, label, |d| {
            d.embroidery.hoop = h;
            Ok(1)
        })?;
    }
    hoops(s, &Value::Null)
}

fn set_hoop(s: &mut Session, p: &Value) -> Result<Value> {
    use vectorcraft_doc::Hoop;
    use vectorcraft_embroidery::hoops;
    const C: &str = "embroidery.setHoop";
    let mut h = match p.get("hoop") {
        Some(Value::Null) => return put_hoop(s, None, "Hoop"),
        Some(v) => {
            let id = v.as_str().map(|t| t.trim().to_ascii_lowercase()).unwrap_or_default();
            hoops::builtin(&id).ok_or_else(|| bad(C, format!("`hoop` must be one of {}, or null", hoops::BUILTIN.join(", "))))?
        }
        None => s.doc()?.doc.embroidery.hoop.as_deref().cloned().filter(|h| h.unknown.is_none()).unwrap_or_else(|| Hoop {
            id: "custom".into(),
            name: "Custom".into(),
            width: 100.0,
            height: 100.0,
            ..Hoop::default()
        }),
    };
    if let Some(v) = p.get("name") {
        let name = v.as_str().map(str::trim).filter(|t| !t.is_empty()).ok_or_else(|| bad(C, "`name` must be text"))?;
        h.name = name.to_string();
    }
    for (key, field) in [("width", &mut h.width), ("height", &mut h.height)] {
        if let Some(v) = p.get(key) {
            match v.as_f64() {
                Some(x) if Hoop::SIZE.contains(&x) => *field = x,
                _ => return Err(bad(C, format!("`{key}` must be a number from 10 to 2000 (mm)"))),
            }
        }
    }
    if let Some(v) = p.get("round") {
        h.round = v.as_bool().ok_or_else(|| bad(C, "`round` must be true or false"))?;
    }
    put_hoop(s, Some(h), "Hoop")
}

fn load_hoop(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.loadHoop";
    let bytes = match (str_param(p, "path"), str_param(p, "data")) {
        (Some(path), _) => super::fileio::read_file(path)?,
        (None, Some(text)) => text.as_bytes().to_vec(),
        _ => return Err(bad(C, "give `path` or `data`")),
    };
    let list = vectorcraft_embroidery::hoops::read_file(&bytes).map_err(|e| bad(C, e))?;
    let ids: Vec<String> = list.iter().map(|h| h.id.clone()).collect();
    let h = match str_param(p, "hoop") {
        Some(id) => list.into_iter().find(|h| h.id.eq_ignore_ascii_case(id.trim())).ok_or_else(|| bad(C, format!("the file has no hoop `{id}`")))?,
        None => list.into_iter().next().ok_or_else(|| bad(C, "the file has no hoops"))?,
    };
    let mut r = put_hoop(s, Some(h), "Load Hoop")?;
    r["hoops"] = json!(ids);
    Ok(r)
}

fn save_hoop(s: &mut Session, p: &Value) -> Result<Value> {
    use vectorcraft_embroidery::hoops;
    const C: &str = "embroidery.saveHoop";
    let list: Vec<vectorcraft_doc::Hoop> = match p.get("hoops") {
        Some(Value::Array(ids)) => ids
            .iter()
            .map(|v| {
                let id = v.as_str().unwrap_or_default();
                hoops::builtin(&id.trim().to_ascii_lowercase()).ok_or_else(|| bad(C, format!("no common hoop `{id}`")))
            })
            .collect::<Result<_>>()?,
        Some(_) => return Err(bad(C, "`hoops` must be a list of common hoop ids")),
        None => vec![
            s.doc()?
                .doc
                .embroidery
                .hoop
                .as_deref()
                .cloned()
                .filter(|h| h.unknown.is_none())
                .ok_or_else(|| bad(C, "the document has no hoop: name common ones (`hoops`)"))?,
        ],
    };
    if list.is_empty() {
        return Err(bad(C, "name at least one hoop"));
    }
    let text = hoops::write_file(&list).map_err(|e| bad(C, e))?;
    match str_param(p, "path") {
        Some(path) => {
            super::fileio::write_file(path, text.as_bytes())?;
            Ok(json!({"path": path}))
        }
        None => Ok(json!({"data": text})),
    }
}

fn fabric_json(f: Option<&vectorcraft_doc::Fabric>) -> Value {
    match f {
        // From a newer VectorCraft: as written, marked as not used.
        Some(vectorcraft_doc::Fabric { unknown: Some(raw), .. }) => json!({"unsupported": true, "data": raw}),
        Some(f) => serde_json::to_value(f).unwrap_or(Value::Null),
        None => Value::Null,
    }
}

fn fabrics(s: &mut Session, _: &Value) -> Result<Value> {
    use vectorcraft_embroidery::fabrics;
    let starters: Vec<Value> = fabrics::BUILTIN.iter().filter_map(|id| fabrics::builtin(id)).map(|f| fabric_json(Some(&f))).collect();
    Ok(json!({"fabric": fabric_json(s.doc()?.doc.embroidery.fabric.as_deref()), "starters": starters}))
}

/// Make `f` the document's fabric (`None`: none) as undo step `label`.
fn put_fabric(s: &mut Session, f: Option<vectorcraft_doc::Fabric>, label: &str) -> Result<Value> {
    let f = f.map(Box::new);
    let out = json!({"fabric": fabric_json(f.as_deref())});
    if s.doc()?.doc.embroidery.fabric != f {
        edit_counted(s, label, |d| {
            d.embroidery.fabric = f;
            Ok(1)
        })?;
    }
    Ok(out)
}

fn set_fabric(s: &mut Session, p: &Value) -> Result<Value> {
    use vectorcraft_doc::{Fabric, StitchSettings as S};
    use vectorcraft_embroidery::fabrics;
    const C: &str = "embroidery.setFabric";
    let mut f = match p.get("fabric") {
        Some(Value::Null) => return put_fabric(s, None, "Fabric"),
        Some(v) => {
            let id = v.as_str().map(|t| t.trim().to_ascii_lowercase()).unwrap_or_default();
            fabrics::builtin(&id).ok_or_else(|| bad(C, format!("`fabric` must be one of {}, or null", fabrics::BUILTIN.join(", "))))?
        }
        None => s.doc()?.doc.embroidery.fabric.as_deref().cloned().unwrap_or_else(|| Fabric {
            id: "custom".into(),
            name: "Custom".into(),
            ..Fabric::default()
        }),
    };
    if let Some(v) = p.get("name") {
        let name = v.as_str().map(str::trim).filter(|t| !t.is_empty()).ok_or_else(|| bad(C, "`name` must be text"))?;
        f.name = name.to_string();
    }
    type Field = fn(&mut Fabric) -> &mut Option<f64>;
    let fields: [(&str, std::ops::RangeInclusive<f64>, &str, Field); 9] = [
        ("satinSpacing", S::SPACING, "mm", |f| &mut f.satin_spacing),
        ("fillSpacing", S::SPACING, "mm", |f| &mut f.fill_spacing),
        ("pull", S::PULL, "mm", |f| &mut f.pull),
        ("pullPercent", S::PULL_PERCENT, "%", |f| &mut f.pull_percent),
        ("push", S::PUSH, "mm", |f| &mut f.push),
        ("satinCenterUnder", Fabric::THRESHOLD, "mm", |f| &mut f.satin_center_under),
        ("satinZigzagOver", Fabric::THRESHOLD, "mm", |f| &mut f.satin_zigzag_over),
        ("shapeEdgeOver", Fabric::THRESHOLD, "mm", |f| &mut f.shape_edge_over),
        ("minStitch", vectorcraft_doc::EmbroiderySetup::MIN_STITCH, "mm", |f| &mut f.min_stitch),
    ];
    for (key, range, unit, field) in fields {
        match p.get(key) {
            None => {}
            Some(Value::Null) => *field(&mut f) = None,
            Some(v) => match v.as_f64() {
                Some(x) if range.contains(&x) => *field(&mut f) = Some(x),
                _ => return Err(bad(C, format!("`{key}` must be a number from {} to {} ({unit}), or null", range.start(), range.end()))),
            },
        }
    }
    put_fabric(s, Some(f), "Fabric")
}

fn load_fabric(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.loadFabric";
    let bytes = match (str_param(p, "path"), str_param(p, "data")) {
        (Some(path), _) => super::fileio::read_file(path)?,
        (None, Some(text)) => text.as_bytes().to_vec(),
        _ => return Err(bad(C, "give `path` or `data`")),
    };
    let f = vectorcraft_embroidery::fabrics::read_file(&bytes).map_err(|e| bad(C, e))?;
    put_fabric(s, Some(f), "Load Fabric")
}

fn save_fabric(s: &mut Session, p: &Value) -> Result<Value> {
    use vectorcraft_embroidery::fabrics;
    const C: &str = "embroidery.saveFabric";
    let f = match str_param(p, "fabric") {
        Some(id) => fabrics::builtin(&id.trim().to_ascii_lowercase()).ok_or_else(|| bad(C, format!("no starter fabric `{id}`")))?,
        None => s.doc()?.doc.embroidery.fabric.as_deref().cloned().ok_or_else(|| bad(C, "the document has no fabric: name a starter (`fabric`)"))?,
    };
    let text = fabrics::write_file(&f).map_err(|e| bad(C, e))?;
    match str_param(p, "path") {
        Some(path) => {
            super::fileio::write_file(path, text.as_bytes())?;
            Ok(json!({"path": path}))
        }
        None => Ok(json!({"data": text})),
    }
}

fn worksheet(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.worksheet";
    let letter = match str_param(p, "page").map(|v| v.trim().to_ascii_lowercase()) {
        None => false,
        Some(v) if v == "a4" => false,
        Some(v) if v == "letter" => true,
        Some(_) => return Err(bad(C, "`page` must be a4 or letter")),
    };
    let doc = s.doc()?.doc.clone();
    let plan = s.stitch_plan(&doc);
    if plan.stats().stitches == 0 {
        return Err(bad(C, "nothing is stitched: give paths stitch settings first (embroidery.set)"));
    }
    let sheet = vectorcraft_embroidery::worksheet::build(&doc, &plan, letter);
    match str_param(p, "format") {
        None => {
            let index = s.add_document(sheet, None);
            Ok(json!({"index": index, "threads": plan.threads.len()}))
        }
        Some(f) if f.eq_ignore_ascii_case("pdf") => {
            let bytes = super::fileio::encode(&sheet, "pdf", &json!({}))?;
            match str_param(p, "path") {
                Some(path) => {
                    super::fileio::write_file(path, &bytes)?;
                    Ok(json!({"path": path}))
                }
                None => Ok(json!({"dataBase64": vectorcraft_format::base64_encode(&bytes)})),
            }
        }
        Some(_) => Err(bad(C, "`format` must be pdf (or left out: a new document)")),
    }
}

fn test_sheet(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.testSheet";
    let ids: Vec<String> = match p.get("sections") {
        None | Some(Value::Null) => vec![],
        Some(Value::Array(a)) => a
            .iter()
            .map(|v| v.as_str().map(str::to_string).ok_or_else(|| bad(C, "`sections` must be a list of section ids")))
            .collect::<Result<_>>()?,
        Some(_) => return Err(bad(C, "`sections` must be a list of section ids")),
    };
    let sheet = vectorcraft_embroidery::testsheet::build(&ids).map_err(|e| bad(C, e))?;
    let sections: Vec<Value> =
        sheet.sections.iter().map(|x| json!({"id": x.id, "name": x.name, "artboard": x.artboard, "swatches": x.swatches})).collect();
    let skipped: Vec<Value> = sheet.skipped.iter().map(|(id, why)| json!({"section": id, "reason": why})).collect();
    let swatches: usize = sheet.sections.iter().map(|x| x.swatches).sum();
    let title = sheet.doc.title.clone();
    let index = s.add_document(sheet.doc, None);
    Ok(json!({"index": index, "title": title, "sections": sections, "swatches": swatches, "skipped": skipped}))
}

/// How many swatches passed, failed and are still to judge.
fn sew_counts(r: &vectorcraft_doc::SewOut) -> Value {
    use vectorcraft_doc::SewResult;
    let n = |want: SewResult| r.swatches.iter().filter(|w| w.result() == want).count();
    json!({"pass": n(SewResult::Pass), "fail": n(SewResult::Fail), "untested": n(SewResult::Untested)})
}

fn result_id(r: vectorcraft_doc::SewResult) -> &'static str {
    match r {
        vectorcraft_doc::SewResult::Untested => "untested",
        other => other.id(),
    }
}

fn sew_out(s: &mut Session, _: &Value) -> Result<Value> {
    let sections: Vec<Value> =
        vectorcraft_embroidery::testsheet::SECTIONS.iter().map(|x| json!({"id": x.id, "letter": x.letter.to_string(), "name": x.name})).collect();
    let Some(r) = s.doc()?.doc.embroidery.sew_out.as_ref() else {
        return Ok(json!({"sewOut": null, "sections": sections}));
    };
    let swatches: Vec<Value> = r
        .swatches
        .iter()
        .map(|w| {
            json!({
                "id": w.id, "section": w.section, "label": w.label, "objects": w.objects.iter().map(|i| i.0).collect::<Vec<_>>(),
                "result": result_id(w.result()), "notes": w.notes,
            })
        })
        .collect();
    Ok(json!({
        "sewOut": {"fabric": r.fabric, "machine": r.machine, "notes": r.notes, "swatches": swatches, "counts": sew_counts(r)},
        "sections": sections,
    }))
}

fn record_sew_out(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.recordSewOut";
    let Some(mut r) = s.doc()?.doc.embroidery.sew_out.clone() else {
        return Err(bad(C, "the document has no sew-out form: make a test sheet first (embroidery.testSheet)"));
    };
    let text = |key: &str| -> Result<Option<String>> {
        match p.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(t)) => Ok(Some(t.trim().to_string())),
            Some(_) => Err(bad(C, format!("`{key}` must be text"))),
        }
    };
    if let Some(t) = text("fabric")? {
        r.fabric = t;
    }
    if let Some(t) = text("machine")? {
        r.machine = t;
    }
    if let Some(t) = text("notes")? {
        r.notes = t;
    }
    match p.get("results") {
        None | Some(Value::Null) => {}
        Some(Value::Array(list)) => {
            for item in list {
                let id = str_param(item, "swatch").map(str::trim).ok_or_else(|| bad(C, "each result names its `swatch` (A3)"))?;
                let Some(w) = r.swatches.iter_mut().find(|w| w.id.eq_ignore_ascii_case(id)) else {
                    return Err(bad(C, format!("no swatch `{id}` on the test sheet")));
                };
                match item.get("result") {
                    None | Some(Value::Null) => {}
                    Some(v) => {
                        let res = v.as_str().and_then(vectorcraft_doc::SewResult::parse);
                        w.result = res.ok_or_else(|| bad(C, "`result` must be pass, fail or untested"))?.id().to_string();
                    }
                }
                match item.get("notes") {
                    None | Some(Value::Null) => {}
                    Some(Value::String(t)) => w.notes = t.trim().to_string(),
                    Some(_) => return Err(bad(C, "`notes` must be text")),
                }
            }
        }
        Some(_) => return Err(bad(C, "`results` must be a list of {swatch, result?, notes?}")),
    }
    let counts = sew_counts(&r);
    if s.doc()?.doc.embroidery.sew_out.as_ref() != Some(&r) {
        edit_counted(s, "Record Sew-Out", |d| {
            d.embroidery.sew_out = Some(r);
            Ok(1)
        })?;
    }
    Ok(json!({ "counts": counts }))
}

fn preview(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "embroidery.preview";
    let scale = match p.get("scale") {
        None | Some(Value::Null) => 1.0,
        Some(v) => {
            v.as_f64().filter(|x| x.is_finite() && (0.01..=16.0).contains(x)).ok_or_else(|| bad(C, "`scale` must be a number from 0.01 to 16"))?
        }
    };
    let doc = &s.doc()?.doc;
    let index = match p.get("artboard") {
        None | Some(Value::Null) => 0,
        Some(v) => v.as_u64().and_then(|i| usize::try_from(i).ok()).ok_or_else(|| bad(C, "`artboard` must be an artboard index"))?,
    };
    let rect = doc.artboards.get(index).map(|a| a.rect).ok_or_else(|| bad(C, format!("no artboard {index}")))?;
    vectorcraft_render::raster_size(rect, scale).map_err(|e| bad(C, e))?;
    let plan = s.stitch_plan(doc);
    let shown = vectorcraft_embroidery::preview::document(doc, &plan);
    let img = vectorcraft_render::Renderer::new().render_region(&shown, rect, scale, true);
    let png = img.to_png().map_err(|e| bad(C, e))?;
    Ok(json!({"width": img.width, "height": img.height, "pngBase64": vectorcraft_format::base64_encode(&png)}))
}

fn stitches(s: &mut Session, p: &Value) -> Result<Value> {
    let limit = p.get("limit").and_then(Value::as_u64).map_or(STITCHES_LIMIT, |l| usize::try_from(l).unwrap_or(usize::MAX).min(STITCHES_LIMIT));
    let plan = s.stitch_plan(&s.doc()?.doc);
    let mut left = limit;
    let mut truncated = false;
    let mut runs = vec![];
    for (thread, pts) in plan.runs() {
        if pts.len() > left {
            truncated = true;
            break;
        }
        left -= pts.len();
        let r = |v: f64| (v * 1000.0).round() / 1000.0;
        runs.push(json!({ "thread": thread, "points": pts.iter().map(|q| [r(q.x), r(q.y)]).collect::<Vec<_>>() }));
    }
    let mut jumps = vec![];
    let mut at: Option<(f64, f64)> = None;
    for n in &plan.needles {
        if n.cmd == vectorcraft_embroidery::Cmd::Jump
            && let Some((x, y)) = at
        {
            jumps.push([x, y, n.x, n.y]);
        }
        at = Some((n.x, n.y));
    }
    jumps.truncate(limit);
    Ok(json!({
        "threads": plan.threads.iter().map(|t| hex(*t)).collect::<Vec<_>>(),
        "runs": runs,
        "jumps": jumps,
        "truncated": truncated,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threads_parse_from_hex_and_arrays() {
        assert_eq!(parse_thread(&json!("#ff8000")), Some([255, 128, 0]));
        assert_eq!(parse_thread(&json!("FF8000")), Some([255, 128, 0]));
        assert_eq!(parse_thread(&json!([1, 2, 3])), Some([1, 2, 3]));
        assert_eq!(parse_thread(&json!([1, 2, 300])), None);
        assert_eq!(parse_thread(&json!("#ff80")), None);
        assert_eq!(parse_thread(&json!("#ffé00")), None);
    }
}
