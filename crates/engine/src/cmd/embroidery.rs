//! Object › Embroidery: stitch settings on paths and compound paths (`embroidery.set`,
//! `embroidery.clear`), the design they make (`embroidery.info`, `embroidery.stitches`, the
//! Stitch Preview), and DST files (`document.export {format: dst}`, `document.open` of a `.dst`;
//! see `fileio::embroidery`).

use serde_json::{Value, json};
use vectorcraft_doc::{Document, EmbroiderySetup, NodeId, NodeKind, StitchKind, StitchSettings, Underlay};
use vectorcraft_embroidery::Plan;

use super::overprint::edit_counted;
use super::*;

const SET: &str = "embroidery.set";
/// The most points `embroidery.stitches` returns unless asked for fewer.
const STITCHES_LIMIT: usize = 500_000;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "embroidery.set",
            "Stitch Settings",
            [],
            None,
            "{type?: auto|running|bean|satin|zigzag|satinShape|fill|patternBorder|manual (patternBorder: a border pattern repeated along the path, bending with its curves, a whole number of repeats between corners; auto: by stroke width, thinUnder?: mm (0.2–3; 1) → bean outline, wideOver?: mm (2–12; 7) → fill with a satin border, else satin shape) (default: the object's own, else running), length?: mm (stitch length, 0.3–12; running/bean 2.5, fill 4), spacing?: mm (satin zigzag, satin shape, zigzag stitch or fill row spacing, 0.1–10; satin and fill 0.4, zigzag 2), angle?: degrees (fill rows, counter-clockwise), width?: mm (satin column or zigzag, 0.3–12; satin: the stroke width, zigzag: 3; pattern border: the band, 1–40, default the pattern's own), pattern?: a border pattern id: waves|scallops|chain|diamonds or one loaded into the document (embroidery.borderPatterns) | null (pattern border: the pattern; waves), flip?: bool (pattern border: mirrored across the path), thread?: \"#rrggbb\" | [r, g, b] (default: the stroke for path stitches, the fill for fills), pull?: mm (satin and satin shape pull compensation, 0–3; 0), underlay?: none|center|edge|zigzag (satin) | none|center (satin shape) | none|fill (fill), stagger?: 2–8 (fill rows before the needle holes line up again; 3), passes?: 3|5|7|9 (bean; 3), border?: mm (fill: a satin border over the edges, 0.5–6) | null (none), motif?: diamonds|waves|circles|scales|herringbone | null (fill: a motif carved into the fill, its needle points on the motif's lines), motifSize?: mm (3–60; 10), trimBefore?: bool, trimAfter?: bool (cut the thread before / after the object, however close the next stitch), rails?: bool (satin: between the object's two longest subpaths as its edges, other subpaths crossing both as rungs setting the stitch direction), ids?} stitch the paths, compound paths and type objects in ids or the selection (groups: those inside; type is sewn letter by letter, each letter in its characters' colour: use auto for lettering), as one undo step; null resets length, spacing, width, thread, pull, stagger or passes to the default → {changed}",
            has_doc,
            set
        ),
        cmd!(
            "embroidery.setup",
            "Embroidery Setup",
            [],
            None,
            "{trimJumpsOver?: mm (0.5–100; 3) | null (never trim on length: only thread changes and objects' trimBefore/trimAfter cut the thread), stitchMovesUnder?: mm (0–5; 1: moves to the next stitch up to this long are sewn, longer ones jump)} the document's travel settings, as one undo step → {trimJumpsOver, stitchMovesUnder}",
            has_doc,
            setup
        ),
        cmd!(
            query "embroidery.order",
            "Sew Order",
            [],
            None,
            "{} the stitched objects in the order they are sewn → {order: [{id, name, stitch: type id, thread: \"#rrggbb\"}], custom: bool (false: paint order)}. Whatever is sewn later lies on top on the fabric",
            has_doc,
            order
        ),
        cmd!(
            "embroidery.reorder",
            "Change Sew Order",
            [],
            None,
            "{to: first|earlier|later|last, ids?} move the stitched objects in ids or the selection (groups: those inside) in the sewing order, or {order: [ids]} set it (those first, in that order; the others after, in paint order); one undo step → {order: [ids]}",
            has_doc,
            reorder
        ),
        cmd!(
            "embroidery.sortByThread",
            "Sew by Thread",
            [],
            None,
            "{} group the sewing order by thread colour (colours in the order they first come, each keeping its objects' order), so each thread is sewn once where layering allows; one undo step → {order: [ids], colorChanges}",
            has_doc,
            sort_by_thread
        ),
        cmd!(
            "embroidery.resetOrder",
            "Sew in Paint Order",
            [],
            None,
            "{} sew the objects in paint order again (bottom first); one undo step → {order: [ids]}",
            has_doc,
            reset_order
        ),
        cmd!(
            query "embroidery.borderPatterns",
            "Border Patterns",
            [],
            None,
            "{} the border patterns pattern borders can sew: the document's own (loaded from .vcborder files) then the starters → {patterns: [{id, name, builtin, width, length (mm, the pattern's own size), corner: has a corner motif, satin: has satin parts (sewn at the object's spacing), author, license, source, description, used: objects sewing with it}]}",
            has_doc,
            border_patterns
        ),
        cmd!(
            "embroidery.loadBorderPattern",
            "Load Border Pattern…",
            [],
            None,
            "{path? | data?: the file's text} load a .vcborder border pattern file into the document (kept in it, so the document sews the same anywhere), as one undo step. Its id is the file's (else made from its name); one already taken by a different pattern gets a number (rope-2…), and a pattern the document (or the starters) already has is not added again → {id, name, added}",
            has_doc,
            load_border_pattern
        ),
        cmd!(
            "embroidery.saveBorderPattern",
            "Save Border Pattern…",
            [],
            None,
            "{pattern: id (the document's or a starter), path?} write a border pattern as a .vcborder file (JSON; specified in docs/formats/vcborder.md) → {path}, or without path {data: the file's text}",
            has_doc,
            save_border_pattern
        ),
        cmd!(
            "embroidery.removeBorderPattern",
            "Remove Border Pattern",
            [],
            None,
            "{pattern: id} remove one of the document's border patterns, as one undo step (not one objects still sew with; starters can't be removed) → {removed: id}",
            has_doc,
            remove_border_pattern
        ),
        cmd!(
            "embroidery.clear",
            "Remove Stitches",
            [],
            None,
            "{ids?} the objects in ids or the selection (groups: those inside) are no longer stitched, as one undo step → {changed}",
            has_doc,
            clear
        ),
        cmd!(
            query "embroidery.info",
            "Embroidery Info",
            [],
            None,
            "{ids?} → {design: {objects, stitches, jumps, trims, colorChanges, threads: [\"#rrggbb\"], widthMm, heightMm, warnings}, setup: {trimJumpsOver, stitchMovesUnder} (embroidery.setup), selection: {stitched, unstitched, type, length, spacing, angle, width, thread, pull, underlay, stagger, passes, border, motif, motifSize, pattern, flip, thinUnder, wideOver, rails, trimBefore, trimAfter} (each null when the stitched objects differ, or none is)}",
            has_doc,
            info
        ),
        cmd!(
            query "embroidery.stitches",
            "Embroidery Stitches",
            [],
            None,
            "{limit?: 500000 (most points)} the design's stitches as the machine sews them → {threads: [\"#rrggbb\"], runs: [{thread, points: [[x, y], …]}] (mm, document coordinates, y down; a run is sewn without a jump or trim), jumps: [[x0, y0, x1, y1]], truncated}",
            has_doc,
            stitches
        ),
    ]
}

pub(crate) fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

/// A thread colour: `"#rrggbb"` (or `rrggbb`) or `[r, g, b]` (0–255).
fn parse_thread(v: &Value) -> Option<[u8; 3]> {
    match v {
        Value::String(s) => {
            let s = s.trim().trim_start_matches('#');
            if s.len() != 6 || !s.is_ascii() {
                return None;
            }
            let c = |i: usize| s.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok());
            Some([c(0)?, c(2)?, c(4)?])
        }
        Value::Array(a) if a.len() == 3 => {
            let c = |i: usize| a.get(i)?.as_u64().and_then(|v| u8::try_from(v).ok());
            Some([c(0)?, c(1)?, c(2)?])
        }
        _ => None,
    }
}

/// An optional measurement param: absent → leave it, `null` → the default, a number → checked
/// against `range`.
fn measure(p: &Value, key: &str, range: std::ops::RangeInclusive<f64>) -> Result<Option<Option<f64>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => match v.as_f64() {
            Some(x) if range.contains(&x) => Ok(Some(Some(x))),
            _ => Err(bad(SET, format!("`{key}` must be a number from {} to {} (mm), or null", range.start(), range.end()))),
        },
    }
}

/// The edits `embroidery.set` asks for.
struct Edits {
    kind: Option<StitchKind>,
    length: Option<Option<f64>>,
    spacing: Option<Option<f64>>,
    width: Option<Option<f64>>,
    angle: Option<f64>,
    thread: Option<Option<[u8; 3]>>,
    pull: Option<Option<f64>>,
    underlay: Option<Underlay>,
    stagger: Option<Option<u8>>,
    passes: Option<Option<u8>>,
    trim_before: Option<bool>,
    trim_after: Option<bool>,
    rails: Option<bool>,
    motif: Option<Option<String>>,
    motif_size: Option<Option<f64>>,
    pattern: Option<Option<String>>,
    flip: Option<bool>,
    border: Option<Option<f64>>,
    thin_under: Option<Option<f64>>,
    wide_over: Option<Option<f64>>,
}

/// An optional true/false param.
fn flag(p: &Value, key: &str) -> Result<Option<bool>> {
    p.get(key).map(|v| v.as_bool().ok_or_else(|| bad(SET, format!("`{key}` must be true or false")))).transpose()
}

/// An optional whole-number param in `range` (odd only when `odd`): absent → leave it, `null` →
/// the default.
fn count(p: &Value, key: &str, range: std::ops::RangeInclusive<u8>, odd: bool) -> Result<Option<Option<u8>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => match v.as_u64().and_then(|n| u8::try_from(n).ok()) {
            Some(n) if range.contains(&n) && (!odd || n % 2 == 1) => Ok(Some(Some(n))),
            _ => {
                let what = if odd { "an odd whole number" } else { "a whole number" };
                Err(bad(SET, format!("`{key}` must be {what} from {} to {}, or null", range.start(), range.end())))
            }
        },
    }
}

impl Edits {
    fn parse(p: &Value) -> Result<Self> {
        let kind = match p.get("type") {
            None | Some(Value::Null) => None,
            Some(v) => Some(v.as_str().and_then(StitchKind::parse).ok_or_else(|| {
                bad(SET, "`type` must be auto, running, bean, satin, zigzag, satinShape, fill, patternBorder or manual (embroidery.clear removes stitches)")
            })?),
        };
        let angle = match p.get("angle") {
            None => None,
            Some(v) => Some(v.as_f64().filter(|a| a.is_finite()).ok_or_else(|| bad(SET, "`angle` must be a number of degrees"))?),
        };
        let thread = match p.get("thread") {
            None => None,
            Some(Value::Null) => Some(None),
            Some(v) => Some(Some(parse_thread(v).ok_or_else(|| bad(SET, "`thread` must be \"#rrggbb\", [r, g, b] or null"))?)),
        };
        let underlay = match p.get("underlay") {
            None => None,
            Some(Value::Null) => Some(Underlay::None),
            Some(v) => Some(
                v.as_str()
                    .and_then(Underlay::parse)
                    .ok_or_else(|| bad(SET, "`underlay` must be none, center, edge, zigzag (satin) or fill (fill)"))?,
            ),
        };
        Ok(Self {
            kind,
            pull: measure(p, "pull", StitchSettings::PULL)?,
            underlay,
            stagger: count(p, "stagger", StitchSettings::STAGGER, false)?,
            passes: count(p, "passes", StitchSettings::PASSES, true)?,
            trim_before: flag(p, "trimBefore")?,
            border: measure(p, "border", StitchSettings::BORDER)?,
            thin_under: measure(p, "thinUnder", StitchSettings::THIN)?,
            wide_over: measure(p, "wideOver", StitchSettings::WIDE)?,
            trim_after: flag(p, "trimAfter")?,
            rails: flag(p, "rails")?,
            motif: match p.get("motif") {
                None => None,
                Some(Value::Null) => Some(None),
                Some(v) => {
                    let id = v.as_str().map(str::to_ascii_lowercase).filter(|id| vectorcraft_embroidery::motifs::MOTIFS.contains(&id.as_str()));
                    Some(Some(
                        id.ok_or_else(|| bad(SET, format!("`motif` must be one of {} or null", vectorcraft_embroidery::motifs::MOTIFS.join(", "))))?,
                    ))
                }
            },
            motif_size: measure(p, "motifSize", StitchSettings::MOTIF_SIZE)?,
            pattern: match p.get("pattern") {
                None => None,
                Some(Value::Null) => Some(None),
                // Checked against the document's patterns in `set`.
                Some(v) => Some(Some(
                    v.as_str().map(|id| id.trim().to_ascii_lowercase()).ok_or_else(|| bad(SET, "`pattern` must be a border pattern id or null"))?,
                )),
            },
            flip: flag(p, "flip")?,
            length: measure(p, "length", StitchSettings::LENGTH)?,
            spacing: measure(p, "spacing", StitchSettings::SPACING)?,
            // Checked against the stitch each object ends up with in `set`.
            width: measure(p, "width", *StitchSettings::WIDTH.start()..=*StitchSettings::PATTERN_WIDTH.end())?,
            angle,
            thread,
        })
    }

    fn apply(&self, old: Option<&StitchSettings>) -> StitchSettings {
        let mut s = old.cloned().unwrap_or_default();
        if let Some(k) = self.kind {
            s.kind = k;
        }
        if let Some(v) = self.length {
            s.length = v;
        }
        if let Some(v) = self.spacing {
            s.spacing = v;
        }
        if let Some(v) = self.width {
            s.width = v;
        }
        if let Some(v) = self.angle {
            s.angle = v % 360.0;
        }
        if let Some(v) = self.thread {
            s.thread = v;
        }
        if let Some(v) = self.pull {
            s.pull = v;
        }
        if let Some(v) = self.underlay {
            s.underlay = v;
        }
        if let Some(v) = self.stagger {
            s.stagger = v;
        }
        if let Some(v) = self.passes {
            s.passes = v;
        }
        if let Some(v) = self.border {
            s.border = v;
        }
        if let Some(v) = self.thin_under {
            s.thin_under = v;
        }
        if let Some(v) = self.wide_over {
            s.wide_over = v;
        }
        if let Some(v) = self.rails {
            s.rails = v;
        }
        if let Some(v) = &self.motif {
            s.motif.clone_from(v);
        }
        if let Some(v) = self.motif_size {
            s.motif_size = v;
        }
        if let Some(v) = &self.pattern {
            s.pattern.clone_from(v);
        }
        if let Some(v) = self.flip {
            s.flip = v;
        }
        if let Some(v) = self.trim_before {
            s.trim_before = v;
        }
        if let Some(v) = self.trim_after {
            s.trim_after = v;
        }
        s
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
    // An underlay asked for must suit the stitch every target ends up with.
    if let Some(u) = edits.underlay.filter(|u| *u != Underlay::None) {
        let d = &s.doc()?.doc;
        if let Some(k) =
            ids.iter().map(|id| edits.apply(d.node(*id).and_then(|n| n.stitch.as_deref())).kind).find(|k| !Underlay::for_kind(*k).contains(&u))
        {
            return Err(bad(SET, format!("{} stitches have no {} underlay", k.id(), u.id())));
        }
    }
    if let Some(Some(id)) = &edits.pattern {
        let mine = &s.doc()?.doc.embroidery.patterns;
        if vectorcraft_embroidery::borders::find(mine, id).is_none() {
            let ids: Vec<&str> = mine.iter().map(|p| p.id.as_str()).chain(vectorcraft_embroidery::borders::BUILTIN).collect();
            return Err(bad(SET, format!("no border pattern `{id}` (there are {}; embroidery.loadBorderPattern adds one)", ids.join(", "))));
        }
    }
    // So must a width: pattern borders span 1–40 mm, satins and zigzags 0.3–12 mm.
    if let Some(Some(w)) = edits.width {
        let d = &s.doc()?.doc;
        let range = |k: StitchKind| if k == StitchKind::PatternBorder { StitchSettings::PATTERN_WIDTH } else { StitchSettings::WIDTH };
        if let Some(r) = ids.iter().map(|id| range(edits.apply(d.node(*id).and_then(|n| n.stitch.as_deref())).kind)).find(|r| !r.contains(&w)) {
            return Err(bad(SET, format!("`width` must be from {} to {} mm for these stitches", r.start(), r.end())));
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
            json!({
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
    let mut pat = borders::read_file(&bytes).map_err(|e| bad(C, e))?;
    let mine = &s.doc()?.doc.embroidery.patterns;
    // The same pattern, already there under its id: nothing to add.
    let same = |q: &vectorcraft_doc::BorderPattern| q.id == pat.id && *q == pat;
    if mine.iter().any(same) || borders::builtin(&pat.id).is_some_and(|q| same(&q)) {
        return Ok(json!({"id": pat.id, "name": pat.name, "added": false}));
    }
    let taken = |id: &str| borders::BUILTIN.contains(&id) || mine.iter().any(|q| q.id == id);
    if taken(&pat.id) {
        let base = pat.id.clone();
        let Some(id) = (2..10_000).map(|n| format!("{base}-{n}")).find(|id| !taken(id)) else {
            return Err(bad(C, format!("too many patterns named `{base}`")));
        };
        pat.id = id;
    }
    let (id, name) = (pat.id.clone(), pat.name.clone());
    edit_counted(s, "Load Border Pattern", |d| {
        d.embroidery.patterns.push(pat);
        Ok(1)
    })?;
    Ok(json!({"id": id, "name": name, "added": true}))
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
    let pat = pattern_param(s, p, C)?;
    if !s.doc()?.doc.embroidery.patterns.iter().any(|q| q.id == pat.id) {
        return Err(bad(C, format!("`{}` is a starter pattern: it can't be removed", pat.id)));
    }
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
                "stitch": n.stitch.as_deref().map(|s| s.kind.id()),
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
    let plan = vectorcraft_embroidery::plan(doc);
    let all: Vec<&StitchSettings> = ids.iter().filter_map(|id| doc.node(*id)?.stitch.as_deref()).collect();
    let selection = json!({
        "stitched": all.len(),
        "unstitched": ids.len() - all.len(),
        "type": same(all.iter().map(|s| s.kind.id())),
        "length": same(all.iter().map(|s| s.length())),
        "spacing": same(all.iter().map(|s| s.spacing())),
        "angle": same(all.iter().map(|s| s.angle())),
        "width": same(all.iter().map(|s| s.width)),
        "thread": same(all.iter().map(|s| s.thread.map(hex))),
        "pull": same(all.iter().map(|s| s.pull())),
        "underlay": same(all.iter().map(|s| s.underlay().id())),
        "stagger": same(all.iter().map(|s| s.stagger())),
        "passes": same(all.iter().map(|s| s.passes())),
        "border": same(all.iter().map(|s| s.border())),
        "thinUnder": same(all.iter().map(|s| s.auto_thresholds().0)),
        "wideOver": same(all.iter().map(|s| s.auto_thresholds().1)),
        "rails": same(all.iter().map(|s| s.rails)),
        "motif": same(all.iter().map(|s| s.motif.clone())),
        "motifSize": same(all.iter().map(|s| s.motif_size())),
        "pattern": same(all.iter().map(|s| s.pattern().to_string())),
        "flip": same(all.iter().map(|s| s.flip)),
        "trimBefore": same(all.iter().map(|s| s.trim_before)),
        "trimAfter": same(all.iter().map(|s| s.trim_after)),
    });
    Ok(json!({ "design": design(doc, &plan), "setup": setup_json(&doc.embroidery), "selection": selection }))
}

fn stitches(s: &mut Session, p: &Value) -> Result<Value> {
    let limit = p.get("limit").and_then(Value::as_u64).map_or(STITCHES_LIMIT, |l| usize::try_from(l).unwrap_or(usize::MAX).min(STITCHES_LIMIT));
    let plan = vectorcraft_embroidery::plan(&s.doc()?.doc);
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
