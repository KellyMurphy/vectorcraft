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
            "{trimJumpsOver?: mm (0.5–100; 3) | null (never trim on length: only thread changes and objects' trimBefore/trimAfter cut the thread), stitchMovesUnder?: mm (0–5; 1: moves to the next stitch up to this long are sewn, longer ones jump)} the document's travel settings, as one undo step → {trimJumpsOver, stitchMovesUnder}",
            has_doc,
            setup
        ),
        cmd!(
            query "embroidery.order",
            "Sew Order",
            ["Window", "Embroidery"],
            None,
            "{} the stitched objects in the order they are sewn → {order: [{id, name, stitch: type id, thread: \"#rrggbb\"}], custom: bool (false: paint order)}. Whatever is sewn later lies on top on the fabric",
            has_doc,
            order
        ),
        cmd!(
            "embroidery.reorder",
            "Change Sew Order",
            ["Window", "Embroidery"],
            None,
            "{to: first|earlier|later|last, ids?} move the stitched objects in ids or the selection (groups: those inside) in the sewing order, or {order: [ids]} set it (those first, in that order; the others after, in paint order); one undo step → {order: [ids]}",
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
            "{{ids?}} → {{design: {{objects, stitches, jumps, trims, colorChanges, threads: [\"#rrggbb\"], widthMm, heightMm, warnings}}, setup: {{trimJumpsOver, stitchMovesUnder}} (embroidery.setup), selection: {{stitched, unstitched, {}}} (each null when the stitched objects differ, or none is; type is `unknown` for settings from a newer VectorCraft)}}",
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
        // Settings from a newer version this one can't read are replaced, not edited.
        let mut s = old.filter(|o| o.unknown.is_none()).cloned().unwrap_or_default();
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
            edits.suits(&edits.apply(d.node(*id).and_then(|n| n.stitch.as_deref())))?;
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
            let new = edits.apply(node.stitch.as_deref());
            if node.stitch.as_deref() != Some(&new) {
                node.stitch = Some(Box::new(new));
                n += 1;
            }
        }
        Ok(n)
    })?;
    Ok(json!({ "changed": changed }))
}

fn setup_json(e: &EmbroiderySetup) -> Value {
    json!({ "trimJumpsOver": e.trim_over(), "stitchMovesUnder": e.stitch_under() })
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
    doc.walk(|node| n += usize::from(node.stitch.as_deref().is_some_and(|s| s.kind == StitchKind::PatternBorder && s.pattern() == id)));
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
        if n.stitch.as_deref().is_some_and(|t| t.kind == StitchKind::Manual) {
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
    let list: Vec<Value> = vectorcraft_embroidery::sew_order(doc)
        .into_iter()
        .map(|n| {
            json!({
                "id": n.id.0,
                "name": object_name(n),
                "stitch": n.stitch.as_deref().map(StitchSettings::kind_id),
                "thread": hex(vectorcraft_embroidery::thread_of_node(n)),
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
                && node.stitch.take().is_some()
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
    doc.walk(|n| objects += usize::from(n.stitch.is_some()));
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
    })
}

fn info(s: &mut Session, p: &Value) -> Result<Value> {
    let ids = stitchable(s, &targets(s, p)?)?;
    let doc = &s.doc()?.doc;
    let plan = s.stitch_plan(doc);
    let all: Vec<&StitchSettings> = ids.iter().filter_map(|id| doc.node(*id)?.stitch.as_deref()).collect();
    let mut selection = json!({ "stitched": all.len(), "unstitched": ids.len() - all.len() });
    for param in params::PARAMS {
        selection[param.key] = same(all.iter().map(|s| (param.get)(s)));
    }
    Ok(json!({ "design": design(doc, &plan), "setup": setup_json(&doc.embroidery), "selection": selection }))
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
