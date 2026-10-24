# Embroidery implementation plan

The working plan for turning VectorCraft into a full embroidery design studio: digitizing, lettering, quality
control and production output, built as new code from public documentation. The user guide is
[embroidery.md](embroidery.md); this page says what gets built, in what order, and how each step is checked.

## Ground rules

- **Clean-room, as for Adobe.** Behaviour comes only from public material: product pages, published manuals,
  help sites, public forum posts and published file-format descriptions, cited by URL. Never install, run,
  automate or screenshot other digitizing software (Wilcom, Hatch, Pulse/Tajima DG/ML, Embrilliance, Brother,
  BERNINA, Ink/Stitch), and never compare our output against theirs. Never copy their code (Ink/Stitch is GPL),
  fonts, designs, motifs, patterns or thread data. Feature names are fine; their wording and artwork aren't.
  Reading another program's **file formats** is interoperability and is allowed, from their public
  documentation (as DST and PES were): VectorCraft opens Ink/Stitch embroidery fonts the user has (E8.1e)
  without reading Ink/Stitch's code, running it, or shipping its fonts beyond what each font's own licence
  allows.
- **Every step ships whole:** command(s) with params docs, panel UI, translations, `docs/embroidery.md`, tests,
  fuzzing for any new input, and a spec in `docs/formats/` for any new file format (with a schema and a drift
  test). One task id per commit; `cargo xtask ci` green.
- **Every step is MCP-ready** when it lands, so agents can use and test it: its commands have a menu path
  and a full parameter description and appear in the guide's command table (served as
  `vectorcraft://guide/embroidery`); a new stitch type is in the `embroidery` prompt; a new setting is
  described by `embroidery.set` and reported by `embroidery.info`; a new file format's spec is served as a
  `vectorcraft://spec/…` resource; anything visual can be seen with `render_preview {stitches: true}` (extend
  `embroidery.preview` when a step adds something Stitch Preview shows). `crates/mcp/src/tests_embroidery.rs`
  fails until each holds; extend it with each step's own MCP round trip.
- **Gates need fabric.** Stitch quality is only proven by sewing. Each phase ends with a sew-out the user does
  on a real machine (see [Sew-out gates](#sew-out-gates)). Work on the next phase may start, but features aren't
  called done until their gate passes.
- **No new fill families while basics are weak** (from the research notes): underlay, compensation and trims
  come before decorative fills.

## What's already done, against the research notes

The notes in [embroidery.md › Development plan](embroidery.md#development-plan-for-future-steps) list
the features needed. Many are already built:

| Research item | Status | Where |
|---|---|---|
| Bezier editing, boolean operations, vector import | **Done** (the vector editor) | VectorCraft core |
| Object-level stitch settings | **Done** | `embroidery.set`, Embroidery panel |
| Manual stitch placement, running, bean | **Done** | Manual, Running, Bean stitches |
| Fill generation, border generation | **Done** | Fill (tatami, brick, carved motifs), fill satin border |
| Satin with corner rules and reliable spacing | **Done** | fanned corners, rails and rungs, satin shape |
| Underlay | **Partly** | chosen by hand per object; no automatic choice by width |
| Pull compensation | **Partly** | satin and satin shape only; no push compensation, no fill compensation |
| Trims, ties, jump control | **Done** | Travel settings, Trim Before/After, tie-ins and tie-offs |
| Fill row ordering and routing | **Done** | E3.5 routing |
| Hidden overlaps | **Missing** | lower objects are sewn whole under upper ones |
| Start/stop points, branching, travel under fills | **Missing** | |
| Thread assignment, colour sequencing | **Done** | thread per object, Sew Order, By Thread |
| DST export, checked by an independent reader | **Done** (checked by hand) | DST in and out |
| Design inspection | **Partly** | `embroidery.info` counts and warnings; no checker, no stitch player |
| Worksheets, thread charts | **Missing** | |
| TrueType/OpenType to stitches | **Done** | live type, Auto per letter |
| Small-lettering and display fonts, font format, font editor | **Planned** | E5 embroidery fonts |
| Text on a path, monograms | **Unverified / missing** | type on a path exists in the editor; its stitching is untested |
| Satin border tools | **Done** | fill border, Auto border |
| Programmable carving patterns | **Partly** | five built-in carved motifs; no user carving patterns |
| Motif fills (stitched tiles) | **Planned** | E5 motif extraction |
| Border patterns | **Done** | Pattern Border, `.vcborder`, extraction (E5.2–E5.5) |
| Pattern folders (motifs, fonts, palettes, hoops) | **Partly** | patterns live in documents and files; no library folders yet |
| Guided, contour, spiral, radial, meander, stipple fills | **Missing** | |
| Auto-digitize from bitmaps, click-to-fill/outline | **Missing** | Image Trace already turns bitmaps into paths |
| Appliqué and cutter export | **Planned** | E5 appliqué |
| PES, EXP, JEF, VP3 | **Missing** | E4 in the guide, Phase 3 in the research notes (see below) |
| Realistic previews, stitch player, estimators | **Partly** | Stitch Preview draws thread on fabric; no shading, player or estimates |
| Specialty fills (fur, ripple, cross-stitch, tartan…) | **Missing** | |

### Corrections to the research notes

- *"Strongest in fills, where the tatami and engraving pattern libraries already cover the core."* Overstated:
  there are 7 stagger patterns and 5 built-in carved motifs, with no user carving library. The strongest areas
  are actually satin (corners, rails, satin shape) and live type.
- *Sequencing gaps "trims, ties"*: trims and ties are done. What's left is start/stop points, branching and
  travel under fills.
- *"the 18 P0 features"*: the 62-feature catalog isn't in the repository, so the P0 list below is rebuilt from
  the Phase 1 bullets. Add the catalog (e.g. as `plan/embroidery-catalog.md`) to check the mapping.
- *Machine formats in Phase 3*: PES (Brother, Baby Lock), JEF (Janome) and EXP (Melco, BERNINA) are what home
  and small-shop machines read. DST alone shuts most hobbyists out, so this plan moves PES/JEF/EXP into Phase 1
  and leaves VP3 and the rest for later.
- *Open research note (mcpembroidery)*: closed. It's unrelated, and nothing is ported from it.
- The guide's opening still says "early work (milestones E1–E2)" and its "What's next" overlaps the
  development plan. Once this plan is agreed, the guide should keep a short "What's next" that links here.

## Foundations first (E6)

Building these first is what keeps Phases 1–3 moving: without them, every new stitch type or parameter
means hand-editing the command parser, the info query, the panel, the docs table and the format schema.

| Step | What | Why it unblocks | Size |
|---|---|---|---|
| **E6.1** Forward-compatible stitch data (**done**) | An unknown `kind`, `underlay` or part stitch in a document is kept as written and sewn as nothing (with a warning), instead of making the whole document fail to load. Spec updated. | Every later step adds kinds; older builds must keep opening newer documents. | S |
| **E6.2** Parameter table (**done**) | One table describes each stitch parameter (key, type, unit, range, default per kind, which kinds use it, label, tooltip). Command parsing, `embroidery.info`, the panel's rows, the docs table and the schema drift test are driven from it. | Phases 1–3 add 40+ parameters; one place to add each. | M |
| **E6.3** One module per stitch kind (**done**) | Split the big `sew()` match into a module per kind (generate, defaults, quality checks), registered in one list. | New fills become a file each, testable alone. | M |
| **E6.4** Stitch cache and budgets (**done**) | Cache each object's stitches by geometry and settings, so editing one object re-sews only it. Add an embroidery case to `vectorcraft-cli perf` (a 300-object design with lettering), with a budget. | Contour fills, auto-digitize and the live checker are too slow without it. | M |
| **E6.5** Design checker framework (**done**) | Findings with severity, object id and location. Shown as a list in the panel (click to select), as marks on the canvas, and as `embroidery.check`; a summary before export. | Phase 1's checker rules plug into it, and so does every later feature's own check. | M |
| **E6.6** Sew-out kit (**done**) | `embroidery.testSheet` makes calibration designs: satin widths 1–10 mm, densities, pull compensation steps, fill angles, underlays, lettering sizes 4–20 mm, each labelled in stitches, one 100 × 100 mm artboard per section (DST export writes one design per artboard). The Sew-Out Test panel is the form: pass/fail and a note per swatch, the fabric and machine, saved in the document (`sewOut`, specified in docs/formats/). | Turns the fabric gates into a repeatable routine. | S |

Also in flight, finishing E5:

- **E5.5** Border pattern extraction (built; awaiting its in-app check and commit).
- **E5.6** Embroidery for agents (MCP): **done**. Menu paths for every command, server instructions, the
  `embroidery` prompt, the guide and format specs as resources, `render_preview {stitches: true}`
  (`embroidery.preview`), and the MCP-readiness tests.

### Adding a stitch setting (after E6.2)

1. The field on `StitchSettings` (`crates/doc/src/embroidery.rs`), optional and skipped when unset, with
   its clamped accessor.
2. Its row in `crates/embroidery/src/params.rs` (`PARAMS`): key, value and range, the stitch types it
   applies to, label, tooltip, description, `get`/`set`, and `suits` if it depends on the type. Parsing,
   `embroidery.set`'s and `embroidery.info`'s descriptions and the info value follow from it, and the
   Embroidery panel shows it with a generic row.
3. Its use in the generator, with tests.
4. The schema (`docs/formats/vectorcraft-embroidery.schema.json`) and the spec's table, and the guide's
   row for each type it applies to; translations for its label and tooltip. Tests name anything
   missing (`format_specs`, `tests_embroidery`, the i18n tests).
5. A row of its own in the panel (`own_rows`) when the generic one isn't good enough.

### Adding a check rule (after E6.5)

An entry in `RULES` (`crates/embroidery/src/check.rs`): id, label, help, whether the plan's warnings
already report it, and `run(doc, plan) -> Vec<Finding>` (severity, message, the object from
`plan.objects`, a place in mm). The panel, Stitch Preview's marks, `embroidery.check` and the export
summary follow; its label and help are translated (the i18n tests list them), and the guide's
"Checking the design" list gets a line.

### Adding a stitch type (after E6.3)

1. The variant on `StitchKind` (`crates/doc/src/embroidery.rs`): its id, label, `ALL`, and whether it
   follows the path.
2. A module in `crates/embroidery/src/kinds/` with `sew(&Input) -> Output` (runs of needle points, and
   warnings about the art), listed in `kinds::sew`; shared geometry goes in `generate.rs`. Its tests next
   to it; `kinds::tests` checks every type sews a plain shape cleanly.
3. The settings it uses: `kinds` lists in `params.rs` (and new settings as in the checklist above).
4. The Object › Embroidery menu item, the panel's section (`own_rows`), the guide's table row, the
   spec's kinds table and schema enum, the MCP prompt's stitch list, translations. Tests name each one
   that is missing.

## Phase 1: production-ready digitizing (P0) — E7

The goal: real artwork digitized, lettered and exported, sewing cleanly on common fabrics without hand fixes.

| Step | What | Size |
|---|---|---|
| **E7.1** Automatic underlay (**done**; thresholds fixed until E7.3) | An `auto` underlay, made the default, chosen by stitch kind and width: narrow satin gets a centre walk, medium gets an edge walk, wide gets edge walk plus zigzag; fills get an edge walk plus fill underlay at right angles; satin shapes get a centre or edge walk by stroke width. Thresholds come from the fabric profile (E7.3). | M |
| **E7.2** Compensation (**done**: pull in mm and %, push; push not yet on satin shapes or between rails) | Pull compensation for fills (rows extended at their ends) and satin shapes by stitch angle. **Push compensation** (columns shortened at their ends along the stitch direction). Applied per axis, in mm or %. | M |
| **E7.3** Fabric profiles (**done**; starters' numbers to be checked by the G1 sew-out; minimum stitch length waits for E7.6) | Document-level fabric presets (woven, knit, fleece/towel, cap, leather, denim) setting density, underlay thresholds, compensation and minimum stitch length; per-object overrides win. Generated in code; saved and loaded as open `.vcfabric` files (spec + schema). | M |
| **E7.4** Overlap removal (**done**; fill borders are still sewn whole) | Fill under a covering fill or satin (higher in the sewing order) is cut away, leaving an overlap margin (0.5 mm by default), so stitches don't pile up. On by default, per-object opt-out. | L |
| **E7.5** Start/stop points and auto-sequencing (**done**: start points with canvas handles, automatic entry for lines, Optimize, hidden travel, branching; stop points follow from the start and the path) | Per-object start and stop points (handles on the canvas). Within a thread, objects are ordered and each started at the end nearest the last stop, cutting jumps and trims. Running-stitch networks are sewn by branching (each line once, travel doubled under it). Travel hidden under fills that are sewn later. | L |
| **E7.6** Small-stitch filter (**done**) | A document minimum stitch length (from the fabric profile). Shorter stitches are merged or dropped everywhere, satin columns included. Short stitches on tight satin curves, as satin shape already does. | S |
| **E7.7** Checker rules (**done**) | Stitches over 7 mm (snag risk) or 12 mm (impossible); satin under 1 mm or over 10 mm wide; objects too small to sew; density hot spots (stacked layers, from a density map); open shapes set to fill; jumps and trims over budget; design beyond the hoop; type below its safe size. | M |
| **E7.8** Hoops (**done**: common nominal sizes, no makers' lists) | Hoop templates as data: size, sewing field and machine family, from manufacturers' public spec sheets (sizes only). The design is placed in a hoop, overflow is checked (E7.7), and the hoop is drawn in Stitch Preview and on the canvas. Open `.vchoop` lists (spec + schema). | M |
| **E7.9** Stitch player and inspection (**done**, in the Sew Order panel) | A player in Stitch Preview: play, step and scrub the sewing in order, by stitch or by colour stop; per-object stitch counts and the time to sew at a set speed. | M |
| **E7.10** Thread charts (**done**; the user chose (b): no maker publishes a chart under redistributable terms yet, so none ships; users load their own) | Threads by brand and number, matched to the nearest colour, shown in the panel and sew order. An open `.vcthreads` chart format (spec + schema). **Needs a decision** (see below) on where chart data may come from. | M |
| **E7.11** Production worksheet (**done**; its words are English for now) | A printable sheet (PDF through the existing PDF export, and on screen): design picture, size, stitch count, hoop, start point, colour sequence with thread names, numbers and chips, estimated time and thread used, notes. | M |
| **E7.12** PES, JEF, EXP (**done**; PES keeps thread colours in its version 6 list; no maker's palette ships, so PEC and JEF thread numbers are one per distinct colour; checked against pyembroidery, not yet sewn) | Read and write, from published format descriptions, cited in `docs/embroidery.md`. Colours come through where the format keeps them (PES and JEF keep thread colours). Fuzzed readers. Checked by hand against an independent reader, as DST was. | L |
| **E7.13** Lettering basics (**done**: type on a path, tracking and baseline shift tested stitched; Auto sews small type as narrow satin (0.8 mm columns) and fits a filled letter's border to its strokes and holes; the checker warns of holes under 1 mm, per font) | Verify and test type on a path, stitched. Letter spacing and baseline controls checked against stitched type. Minimum safe sizes per stitch kind, which the checker (E7.7) warns about. | S |
| **E7.14** Automatic puff (3D foam) embroidery (asked for by the user, 2026-10-22; **done**) | A `puff` setting on satin columns, satin shapes and Auto lettering, with the foam's thickness. A puffed object is sewn after a **stop** for placing the foam, as dense satin (spacing around 0.25 mm) with no underlay over the foam and its open ends **capped** (the column closed over the foam's cut edge). Then comes a **perforation**: a running stitch of short stitches (around 1 mm) all the way round each puffed shape, just inside its edge, so the needle penetrations cut the foam and the excess tears away cleanly. Flat objects are sewn before the stop. The checker warns about puff that is too narrow (under about 3 mm) or too wide (over about 10 mm) to rise, and about thin foam. The test sheet gains a puff section. | M |

**Gate G1 (fabric):** sew the E6.6 test sheet on woven and on knit, and three real designs (a logo with fills
and satins, lettering at 6, 10 and 25 mm, a multicolour badge). Pass: no thread breaks or bird-nests, no gaps
between neighbouring objects, satin edges clean, lettering legible at 6 mm.

## Goal: compensation a step above — E10

Most digitizing tools apply one compensation number per object, tuned by hand. Published research
measures the effect but offers no predictive model or open algorithm. VectorCraft aims to beat that by
measuring fabric behaviour and compensating for direction, layering and curvature. E7.2 (push and
pull, per axis) and E7.3 (fabric profiles) are the base; E10 builds on them once G1 has checked the
starter numbers. Steps can run alongside E8.

| Step | What | Size |
|---|---|---|
| **E10.1** Calibration from sew-outs | Extend the E6.6 kit with a compensation sheet: satin columns at several widths (2, 4, 6, 8 mm) and angles (0°, 45°, 90°, 135° to the fabric grain), and fill squares at the same angles. Each swatch has running-stitch marks at the drawn edges. The user sews it, then enters measured widths and lengths in the Sew-Out Test panel or loads a scan or photo (measured against the marks). VectorCraft fits `pull`, `pullPercent` and `push` per angle and saves them as a `.vcfabric` profile. Measured numbers replace guessed starters. | L |
| **E10.2** Grain-aware compensation | The document (or hoop placement) records the fabric's grain angle. Compensation for each stitch is interpolated by its angle to the warp and weft, from the per-angle values in the profile (E10.1), instead of one value for every direction. Spec and schema: `.vcfabric` gains an optional per-angle table; profiles without one behave as today. | M |
| **E10.3** Curvature-aware satin | Pull and push vary along a satin column with the local radius: on tight curves the inside edge has denser needle penetrations than the outside, so each edge is compensated separately. Applies to satin, satin shape and between rails. | M |
| **E10.4** Layer- and density-aware compensation | Compensation scales with what is already sewn underneath (underlay, lower fills, overlap margins from E7.4), using the checker's density map (E7.7 / E8.7). Areas sewn over heavy layers get less extra pull than bare fabric. | M |
| **E10.5** Distortion preview | Stitch Preview can show the predicted result: the fabric as a spring mesh, each stitch a contracting cable with tension from the thread and fabric profile, solved to show draw-in, gaps between neighbouring objects and puckering before sewing. Validated against E10.1 measurements. A stretch goal; budgeted in `vectorcraft-cli perf`. | XL |

Priority: E10.1 and E10.2 first; they give the biggest visible gain and the data the rest needs.

**Gate GC (fabric):** sew the compensation sheet on woven and knit; fit profiles; then sew a logo and
lettering (6, 10, 25 mm) with the fitted profile at the grain set both straight and at 45°. Pass: satin
widths within ±0.2 mm of drawn, no gaps between neighbouring objects at either angle, and visibly better
alignment than the same design with the starter profile.

Background (public research, cited as sources; nothing copied): Daukantienė & Mikelionytė, "Investigation
of the influence of technology parameters and thread type on embroidered textile element quality",
*AUTEX Research Journal* 20(4), 2020, doi:10.2478/aut-2019-0046 (geometry change depends on fill type,
density and thread); Radavičienė et al., "The influence of the properties of embroidery threads on
buckling of fabric inside of the embroidered element", Kaunas University of Technology
(https://epubl.ktu.edu/object/elaba:3243049/index.html) (the fabric between penetrations is compressed and
buckles); Hu et al., "Modelling Multi-layer Seam Puckering", *Textile Research Journal* 76(9), 2006 (a thread
shrinkage and seam deformation model, the starting point for E10.5); Đỗ Thị Tần, shrinkage of embroidery
patterns on Pe/Co fabric, 2025 (https://vjol.vista.gov.vn/saodo/en/article/view/114635) (shrinkage against
stitch length, thread and needle size).

## Phase 2: a productive digitizing studio (P1) — E8

The goal: the work a serious hobbyist or small shop does every day — lettering in proper embroidery fonts,
monograms, appliqué patches, decorative fills, turning a customer's logo into stitches — done in
VectorCraft without hand-placing stitches, with the user's own fonts, motifs and patterns kept in folders and
shared as open files.

| Step | What | Size | Needs |
|---|---|---|---|
| **E8.5** Library folders (**done**) | User folders for every embroidery library type, listed in the panels, shared by copying files. | M | — |
| **E8.8** Appliqué and cut files | Placement, tack-down and cover from a shape, with stops; nested cutter export. | L | — |
| **E8.1** Embroidery fonts | An open font format, type set in it, a font editor, and Ink/Stitch fonts opened and converted. | XL | E8.5 |
| **E8.2** Monograms and text effects | Monogram layouts, arc and envelope text, stitched as live type. | M | E8.1 for embroidery fonts (works with outline fonts without it) |
| **E8.4** Motif fills and extraction | Stitched tiles filling a shape, extraction from a design, `.vcmotif`, user carving patterns. | L | E8.5 |
| **E8.3** Fill families | Contour, spiral, radial, guided, meander and stipple fills. | L | G1 |
| **E8.7** Density adviser | The density map over the design, with one-click fixes. | M | — |
| **E8.6** Auto-digitize | Traced art to stitch objects; click-to-fill and click-to-outline. | XL | E8.3 helps (more kinds to pick from), E8.7 |

### Order of work

The table is in the order of work. E8.5 comes first because fonts, motifs and every later library need
somewhere to live. Appliqué is next: it is self-contained, uses only stitch kinds that already exist, and its
patch is a G2 item that can be sewn early. Fonts are the largest step and the one the user asked for first.
Fill families wait for G1, as the ground rules say: no new fill families until underlay, compensation and
trims have passed on fabric. E8.7 and E8.6 come last because auto-digitizing leans on everything before it
(more stitch kinds to pick from, the checker to judge the result, the adviser to fix it). E10 (compensation)
runs alongside once G1's numbers are in.

Each step below is split into commits (E8.na, E8.nb…), each shipping whole under the ground rules (command,
panel, translations, guide, tests, fuzzing for new input, spec + schema + drift test for new formats,
MCP-ready).

### E8.5 Library folders (M)

Today border patterns, fabrics, hoops and thread charts are loaded into a document one file at a time
(`embroidery.patterns.load`, `.vcfabric`, `.vchoop`, `.vcthreads`). Swatch and graphic-style libraries
already have a user library folder (`crates/engine/src/cmd/swatchlib.rs`: `Libraries::set_user_dir`,
rescanned at start, User Defined in the library panel). Embroidery reuses that model.

- **E8.5a Folders and scanning (done).** One embroidery library folder (`Embroidery` next to the
  preferences), with a subfolder per type: `borders/`, `fabrics/`, `hoops/`, `threads/` now, and
  `motifs/` and `fonts/` when their formats land (E8.4, E8.1). Scanned at start
  and on `embroidery.library.rescan`; each file is read with its type's reader, size-capped, and a bad file
  is listed with its error instead of stopping the scan. As built: `embroidery.libraries {kind?}` →
  entries with kind, id, name, path, author and licence, plus the files skipped as unreadable;
  `embroidery.rescanLibraries`; `embroidery.addToLibrary {path | data + name, kind?}` (copies a file in,
  never replacing one: a clash gets a number, as swatch libraries do); the four load commands take
  `library` (an entry's id or name) and the four save commands `user: true`. Web: no folder, so nothing is
  listed (the same as swatch libraries on the web). Showing the folder in the file manager belongs to
  E8.5b's panel.
- **E8.5b Panels (done; motifs and fonts join when they exist).** The Embroidery panel's pickers (border patterns, motifs, fabric, hoop, thread chart) list
  three groups: built-in, library, in this document. Picking a library entry copies it into the document
  (`Document.embroidery`), so files still open anywhere; the document copy is used from then on, and a
  newer library version is offered as an update, not applied silently. An "Add to Library…" item on
  document entries. As built: library entries are listed after the document's own, marked "(Library)"
  (border patterns with their pictures; one the document or the starters already have is left out);
  Add to Library on the document's own pattern, fabric, hoop and chart, hidden once the library has the
  same; Show in Folder under the Design section. **Not yet:** the update offer. A border pattern changed
  in the library after a document loaded it is listed again and loads under a numbered id; a chart is
  matched by its id, so a changed one isn't offered.
- **Tests.** Scanning a folder with good, bad, oversized and duplicate files; the copy-into-document rule;
  a document that names a library entry opens without the library; `cargo xtask assets` is unaffected
  (user folders are outside the repo). MCP: the list and add commands in the guide table, a round trip in
  `tests_embroidery`.

### E8.8 Appliqué and cut files (L)

Appliqué sews a piece of fabric on instead of filling an area with thread. Publicly documented sequence:
a placement line, a stop to lay the fabric, a tack-down, a stop to trim the fabric, and a cover over the
raw edge.

- **E8.8a Stops as a setting.** Puff (E7.14) already writes a stop. Generalise it into `stopBefore`
  (on any object: "stop, then sew this") with a message for the worksheet ("Lay the fabric", "Trim the
  fabric"). Written as a colour change to the same thread in DST/EXP and as a stop in PES/JEF where the
  format has one. The sew order, player, worksheet and checker show stops.
- **E8.8b Appliqué from a shape.** `embroidery.applique {ids?, cover: "satin"|"zigzag"|"bean"|"none", width,
  tackDown: "running"|"zigzag"|"doubleRunning", inset, allowance}` turns each closed shape into a group of
  three objects in sewing order, sharing one thread: placement line (running on the outline, then stop
  "Lay the fabric"), tack-down (inset a little inside the outline, then stop "Trim the fabric"), cover
  (satin centred on the outline, wide enough to hide the cut edge, auto underlay off by default over
  fabric). The group remembers it is an appliqué (`appliqué` in `StitchSettings` on the group), so editing
  the shape and re-running updates all three, and the panel shows an Appliqué section (cover, width,
  tack-down, inset, fabric colour for the preview). Stitch Preview draws the fabric piece in its colour
  under the cover. Several appliqué pieces in one design are sequenced so all placement lines come first
  where they don't overlap (one stop instead of one per piece), as an option.
- **E8.8c Cut files.** `embroidery.cutFile {ids?, allowance (mm, default 0), gap (3 mm), mat: "12x12in"|
  "12x24in"|{w,h}, rotate: "none"|"90"|"any", flip, format: "svg"|"dxf", path}` exports the pieces'
  outlines (grown by the allowance) **nested**: packed without overlap, the gap apart, onto as few mats as
  fit, each mat a page (SVG: one file per mat or one file with a group per mat; DXF: one file per mat).
  Nesting from published algorithms (bottom-left placement over no-fit polygons, cited when implemented),
  with a time budget in `vectorcraft-cli perf`. Pieces that don't fit any mat are reported, not dropped.
  The SVG is plain paths in mm, which CanvasWorkspace, Cricut Design Space and Silhouette Studio read;
  Brother's FCM only from a public specification (see the decisions below).
- **Checks.** The checker warns about a cover narrower than the tack-down inset plus the fabric's fray
  (raw edge showing), a piece too small to handle (under about 10 mm), and an appliqué missing its stops.
  The test sheet gains an appliqué section (one patch, three cover widths).
- **Tests.** The sequence's order, stops and threads; editing the shape re-makes the parts; nesting
  never overlaps and respects the gap (property test over random polygons); every piece is placed or
  reported; SVG/DXF output read back.

### E8.1 Embroidery fonts (XL)

Lettering hand-digitized for embroidery: crisp small type and heavy display type where Auto falls back
to fill today. The format is decided (2026-10-12): a self-contained file, letters stored as stitch
objects (shapes with settings), not needle points, so letters scale and are re-sewn at each size.

- **E8.1a The format: `.vcfont`.** JSON, versioned, spec in `docs/formats/vcfont.md` with a schema and a
  drift test (as `.vcborder`). Contents: `name`, `family`, `style`, `author`, `license` (SPDX or text),
  `source` (a font it was digitized from, information only), `version`; metrics in font units with
  `unitsPerEm`, `ascender`, `descender`, `capHeight`, `xHeight`, `baseline`; `sizes` (the recommended
  range in mm of cap height, e.g. 5–25); `glyphs`, each with `char` (one or more Unicode scalars, so
  ligatures fit), `advance`, and `objects` (paths in font units, y up, each with a full `StitchSettings`
  and an order); `kerning` pairs; optional `entry`/`exit` points per glyph so letters can join without a
  trim (script fonts). Unknown members are kept, as in the other formats. Data types live in the doc crate
  (serde), the reader is fuzzed (`format/tests/prop_format.rs`), sizes and counts are capped.
- **E8.1b Setting type in an embroidery font.** Embroidery fonts appear in the type controls in their own
  group (the Character panel's font menu, `type.fonts` with `kind: "embroidery"`). A type object set in one
  is laid out by the font's advances and kerning plus the object's tracking, leading, alignment, baseline
  shift and type on a path (reusing `vectorcraft_text::layout`'s line and path logic with the
  embroidery font's metrics in place of an outline font's). On screen, letters are drawn from their
  objects' outlines (as filled shapes), so the type stays live and editable. When sewn, each letter's
  objects are scaled to the size, their stitches generated by the ordinary kinds (density, underlay and
  compensation stay in mm, not scaled), and letters joined left to right, using entry/exit points where
  given, else the nearest-end rule (E7.5) with a trim over the jump limit. A character the font lacks is
  sewn from a fallback outline font with Auto and reported. The checker warns below or above the font's
  size range.
- **E8.1c The font editor.** `embroidery.font.new {name, from?: installed font, chars?: "A-Za-z0-9…"}`
  makes a font document: one artboard per letter, with guides for baseline, x-height, cap height,
  ascender and descender and the advance as the artboard's right edge (dragged to change it). With `from`,
  each artboard gets the installed font's outline on a locked template layer as a tracing guide (not saved
  in the font, so the font contains only the user's own work). The user digitizes with the ordinary tools
  and stitch types (satin between rails is the main tool for letters). A **Font** panel: metrics, size
  range, licence fields, a kerning table (pairs, with a pair picked by clicking two letters in the sample),
  and a **sample line** that sets a test string in the font being edited, live, with Stitch Preview.
  `embroidery.font.save {path}` writes the `.vcfont` (artboards → glyphs), `embroidery.font.open` opens
  one back as a font document. Validation on save: letters with no objects, objects outside the
  metrics, missing licence.
- **E8.1d A starter font.** At least one embroidery font, made with the editor by us (original work, or
  traced from an OFL font with the OFL's renaming and licence carried), published in craft-fonts with its
  licence; VectorCraft builds and runs without it (`CRAFT_FONTS` empty → no starter font, tests skip). A
  satin sans for 5–25 mm is the most useful first one. **Needs a decision** (below).
- **E8.1e Ink/Stitch embroidery fonts** (asked for by the user, 2026-10-23). Ink/Stitch has a large
  library of embroidery fonts, many under the OFL, and its users make their own. VectorCraft opens them as
  embroidery fonts, so they can be set as type, edited in the font editor and saved as `.vcfont`.
  - *The format, from public documentation only* (cited in `docs/formats/inkstitch-fonts.md`, our own
    notes, with the mapping table below): a font is a folder holding `font.json` (name, the reference
    `size`, `horiz_adv_x_default`, per-glyph `horiz_adv_x`, `hkern` pairs, where a positive value pulls a
    pair closer, the allowed resize range, whether it is sortable), one or more glyph SVGs (`ltr.svg`, an
    `ltr/` folder of several, `rtl.svg`, and newer vertical `ttb.svg`/`btt.svg`), a `LICENSE` and a
    `preview.png`. Each glyph is an SVG layer named `GlyphLayer-<character>` (names with several
    characters are ligatures or symbols), placed against a guide named `baseline`. Stitch settings are
    attributes in the `http://inkstitch.org/namespace` namespace on each path; commands (stop, trim,
    fill start and end, ignore) are `<use>` elements pointing to Ink/Stitch's command symbols. Sources:
    the font creation tutorial (https://inkstitch.org/tutorials/font-creation/) and the namespace page
    (https://inkstitch.org/namespace/). The namespace page dates from 2018 and lists only part of
    today's attributes. Anything it doesn't cover is learned only from font files the user owns,
    looked at locally and never committed, **never from Ink/Stitch's source code** (GPL), and never by
    running Ink/Stitch or comparing its stitches with ours.
  - *Mapping onto our stitch types* (attributes by meaning; stitches are always generated by VectorCraft's
    own generators, so they won't match Ink/Stitch's needle for needle): `satin_column` → Satin Column
    **between rails** (E3's rails and rungs: the path's two subpaths are rails, the rest are rungs), with
    `zigzag_spacing_mm` → spacing, `pull_compensation_mm` → pull, center walk, contour and zigzag
    underlay → our center, edge and zigzag underlay (their insets and lengths where we have them);
    `auto_fill`/fill → Fill, with `angle`, `row_spacing_mm` → spacing, `max_stitch_length_mm` → length,
    `staggers` → stagger, `fill_underlay*` → fill underlay, `expand_mm` → pull; a stroke → Running Stitch
    (`running_stitch_length_mm` → length, `bean_stitch_repeats`/`repeats` → Bean Stitch passes); a zigzag
    stroke → Zigzag; `manual_stitch` → Manual Stitch; stop and trim commands → `stopBefore` (E8.8a) and
    Trim After; fill start and end → start and end points (E7.5); `ignore` → not sewn; fill and stroke
    colours → threads; a sortable font's colour index → the letter's sewing order. An attribute we don't
    know is kept on the object (written back if the font is exported again) and listed in the import
    report, and the object sews with our defaults; a glyph we can't read at all is left out and reported.
  - *Commands:* `embroidery.font.importInkstitch {path: the font's folder (or a .zip of it), save?: path
    of a .vcfont to write, library?: true (into the library's fonts/)}` → the font, with a report:
    glyphs read, glyphs skipped, attributes not mapped, and the licence found. An Ink/Stitch font folder
    dropped on the app or opened with File › Open is offered for import.
  - *Installing fonts (the user's choice, decided 2026-10-23):* VectorCraft ships no Ink/Stitch fonts;
    each user installs the ones they want. Two ways, both as simple as file handling gets:
    1. **Point VectorCraft at a folder.** A preference, *Embroidery Font Folders* (Preferences › Type,
       next to Additional Fonts Folder, and `embroidery.fontFolders {folders: [paths]}`), lists folders
       to look in, such as the fonts folder of an existing Ink/Stitch install or a folder of downloaded
       fonts. Every subfolder holding a `font.json` is a font. Nothing is copied: fonts are read where
       they are, so updating them there updates them here.
    2. **Put the font folders in VectorCraft's own folder:** the embroidery library's `fonts/` (E8.5),
       opened with Show in Folder. Ink/Stitch font folders and `.vcfont` files both go there.
    Fonts found either way are listed in the type controls' embroidery fonts at start and on
    `embroidery.rescanLibraries` (converted when first used, cached by folder and file times). A folder
    that isn't a font, or a font that can't be read, is listed in `embroidery.libraries`' `skipped` with
    the reason. Type set in a font that isn't installed on another computer still sews: the document
    keeps a copy of the letters it uses (as it keeps border patterns), and says which font they came from.
  - *The how-to:* `docs/embroidery-fonts.md`, a short page for users (linked from the guide and from the
    font menu's "How to install fonts…" item): what an Ink/Stitch font folder looks like, the two ways
    above with screenshots of VectorCraft only, that each font has its own licence (read its `LICENSE`
    before using a font in work you sell), and where to find fonts (pointing to Ink/Stitch's public font
    pages by URL; no fonts or font files linked for download from us).
  - *Licences:* the `LICENSE` text is carried into the font's `license` (and its `source` names the
    original font folder). Ink/Stitch's fonts have their own licences: mostly the OFL, some Creative
    Commons (one is CC BY-NC-SA, non-commercial), some derived from GPL-with-font-exception fonts, and
    some with only a credit and no stated licence. VectorCraft shows the licence before a font is used
    in a design and warns on non-commercial or unstated ones. **None ships with VectorCraft or in
    craft-fonts** (decision 9): using them is up to each user.
  - *Fixtures:* test fonts written from scratch for the tests (a few glyphs covering satin with rungs, a
    fill, running and bean strokes, a trim, a stop, kerning pairs, a ligature layer, an `ltr/` folder and
    an `rtl.svg`), never copied from Ink/Stitch's fonts. Real fonts the user has are tried locally,
    through an ignored test that reads `EMB_INKSTITCH_FONTS=dir`.
  - *Tests:* the reader is fuzzed (malformed JSON and SVG, huge glyph counts, deep nesting, missing
    baseline, zip bombs); every mapped attribute's effect is checked; unknown attributes survive a round
    trip; a converted font sets type with the right advances and kerning; the mapping table in the
    notes is checked against the code (drift test).
  - *Size:* L. It needs E8.1a (the font model) and E8.1b (setting type); it can land before the editor
    (E8.1c). An Ink/Stitch font the user installed can serve for G2's lettering sample on their
    machine (decision 5).
  - *Later, if wanted:* exporting a `.vcfont` back to an Ink/Stitch font folder, so fonts made in
    VectorCraft work there too.
- **Tests.** Format round trip and drift; fuzzed reader; layout (advances, kerning, tracking, on a path)
  against hand-computed positions; a letter sewn at 5 and 25 mm has the same density in mm; the editor's
  save/open round trip; a test font generated in code for tests, so nothing depends on craft-fonts.
  MCP: the format spec as `vectorcraft://spec/vcfont`, font commands in the guide table, the prompt says how
  to pick an embroidery font.

### E8.2 Monograms and text effects (M)

- **E8.2a Arc and envelope text.** Check that type in an envelope (Object › Envelope Distort,
  `crates/engine/src/cmd/live.rs`) and type with a warp effect is sewn from its distorted outlines, and
  fix it where it isn't (the sewer must use the live result, not the undistorted letters). Add
  `embroidery.textArc {radius, top|bottom, spacing}`: type on a circle arc with the letters upright on its
  outside or inside, as a preset over type on a path, so badges get curved lettering in one step.
  Embroidery-font type follows the same distortions (its letters' objects are distorted as shapes,
  before sewing).
- **E8.2b Monograms.** `embroidery.monogram {letters: "ABC", layout, font?, frame?}` with layouts
  generated in code: three letters with a large centre initial, three equal, two letters, stacked, in a
  circle, in a diamond. The result is a group of live type objects positioned by the layout (each still
  editable), plus an optional frame from the border patterns or a satin ring. The panel's Monogram section
  re-lays out the group when its letters or layout change. Layouts are data (a small list of letter boxes
  per layout), so more can be added without code.
- **Tests.** Envelope and warp type sews from the distorted outline; arc text positions; every monogram
  layout keeps letters inside their boxes and stays editable; small monograms trigger the size warning.

### E8.4 Motif fills and extraction (L)

Carved motifs (E3) shape a tatami fill's needle points along lines. **Motif fills** are different: a small
stitched tile (a star, a flower, a cross) repeated across the area as stitches of its own, the open
fabric showing between — a light, decorative fill.

- **E8.4a The format: `.vcmotif`.** Like `.vcborder`, for a tile with two repeat offsets: `id`, `name`,
  author, licence, source, `size` (mm), `offsets` (the two lattice vectors), `parts` (as `PatternPart`:
  running, bean, satin, with points), and `kind: "stitched" | "carve"`. A **carve** motif is line art used as
  a carved pattern (as today's five built-in ones, which become `.vcmotif` data generated in code), so user
  carving patterns use the same file. Spec, schema, drift test; stored in the document like border
  patterns, listed from the library folder (E8.5).
- **E8.4b The Motif Fill stitch.** `StitchKind::MotifFill` (a kind module): tiles laid on the lattice over the
  shape at a set `motif`, `motifSize`, `angle` and `motifSpacing`, each whole tile inside the shape sewn
  (tiles cut by the edge are left out, or clipped where the motif allows), joined row by row with travel
  that runs inside the shape (and under the tiles where it can, as E7.5's hidden travel does), trims where
  travel would be too long. An optional outline running stitch. Starters generated in code (stars,
  flowers, crosses, hearts, dots).
- **E8.4c Extraction.** Extend border extraction (`crates/embroidery/src/extract.rs`, one offset) to two:
  select a region of an opened design's stitches; find the two shortest independent offsets at which the
  stitches lie on a shifted copy of themselves (searched over the region's autocorrelation, then refined
  as E5.5 does); show the lattice on the canvas with handles to adjust it; cut one tile, joining its edges
  so neighbours meet without gaps or doubled stitches; save as `.vcmotif` with its source recorded. Only
  designs the user has the right to reuse.
- **Tests.** Format round trip and drift; tiles stay inside the shape; the joins are within the trim
  limit; extraction finds known lattices in code-generated designs (square, brick, hexagonal, with DST
  rounding noise) to 0.1 mm; the carve motifs give the same stitches as before the move to data.

### E8.3 Fill families (L) — after G1

Each is a kind module (E6.3) with its own parameters (E6.2), auto underlay (E7.1), compensation (E7.2),
overlap removal (E7.4) and checker support. One commit each:

- **E8.3a Contour fill.** Rings following the outline, inset by the spacing (offsets of the shape, from
  `pathops`), joined ring to ring at the nearest points so it sews as one run; holes get their own rings
  merging into the outer ones. Setting: `spacing`, `length`, `join: "spiral"|"rings"`.
- **E8.3b Spiral fill.** The contour rings joined into one continuous spiral (each ring blended into the
  next over one turn), no visible join. Best on round and near-round shapes; the checker suggests contour
  where the spiral would distort.
- **E8.3c Radial and circular fill.** Radial: rows from a centre point outward (a handle on the canvas,
  default the centroid), spaced by angle so the spacing at the outer edge matches `spacing` and
  shortened rows inserted near the centre so it doesn't pile up. Circular: concentric circles around the
  centre clipped to the shape (rows as in tatami, but curved).
- **E8.3d Guided fill.** Rows that bend along guide lines drawn inside the object (open subpaths of the
  compound path that cross the shape, as rails' rungs do): each row's direction is blended from the
  nearest guides, so rows flow around curves (petals, hair, ribbons). Without guides it is a tatami fill.
- **E8.3e Meander and stipple.** A running stitch wandering through the area without crossing itself.
  Stipple: a random, smooth, space-filling path (from a spanning-tree walk over a grid inside the shape,
  smoothed, seeded so it's repeatable); meander: the same with longer, flowing curves (a larger grid,
  more smoothing). Settings: `spacing` (how close the path comes to itself), `length`, `seed`. Light
  fills for quilting looks and backgrounds; the checker knows they don't cover.
- **Tests.** Each fill sews a set of plain and awkward shapes (thin, holed, concave, many parts) within
  the shape, with no stitch over the limits and no self-crossing where the kind promises none; repeatable
  by seed; perf budget for a large meander. The test sheet gains a section per fill (the G2 swatches).

### E8.7 Density adviser (M)

The checker's density map (`crates/embroidery/src/check.rs`, the density rule's cells) drawn over the
design as a heat overlay in Stitch Preview (`view.stitchDensity`), with a legend in needle holes per
mm². Each density finding gets a suggested fix the user can apply in one click (and `embroidery.fix
{finding}` for agents): lower the top object's density to a value that clears the limit, turn on overlap
removal for the lower object, swap a fill's underlay for a lighter one, add a trim before a long travel,
or drop a duplicated layer. A fix is an ordinary command, so it's undoable, and the adviser re-checks
after it. Tests: each fix clears its finding on a code-generated design and doesn't create a new one.

### E8.6 Auto-digitize (XL)

Turning a customer's logo into stitches in one step, then fixing what's left by hand.

- **E8.6a Traced art to stitch objects.** `embroidery.digitize {id: image or traced group, colors?, minArea,
  detail}`: Image Trace (logo mode, `crates/engine/src/cmd/buildcmds.rs`) makes flat-colour regions; small
  specks under `minArea` merge into neighbours; each region gets a stitch kind by its shape and size
  (extending Auto's choice: thin → running or bean, narrow → satin shape, wide → fill with border, and the
  E8.3 fills where they suit); colours become threads (matched to a thread chart when one is set); the
  objects are sequenced (E7.5: by thread, then nearest), overlaps removed (E7.4), and checked (E7.7). The
  result is ordinary stitch objects in a group, fully editable. A report lists what was merged or
  dropped.
- **E8.6b Click-to-fill and click-to-outline.** A tool (with its own cursor, per the cursor rule): click
  inside any closed area formed by paths, a traced result or overlapping shapes — the regions Live Paint
  finds (`pathops::pathfinder::regions`) — and it becomes a stitch object (fill, or Auto) in the current
  thread; Alt-click outlines it (running or satin). Image areas under the click are traced on the fly at the
  current colour tolerance. Each click is one undo step.
- **E8.6c Quality pass.** Run the checker and adviser (E8.7) on the result and apply the safe fixes; a test
  corpus of logos generated in code (flat shapes, text, thin lines, gradients refused) with a score
  (trims, stitch count, findings) tracked so changes can't make it worse.
- **Tests.** Region counts and kinds on code-generated logos; the result sews with no error findings;
  repeatable; perf budget for a 1000 × 1000 px logo.

### Gate G2 (fabric)

Sew on woven and knit, using the E6.6 kit's new sections: a lettering sample in an embroidery font at
5, 10, 20 and 30 mm; each new fill family on a 10 × 10 cm swatch; a motif fill swatch; a monogram; an
auto-digitized logo with no hand fixes; an appliqué patch with its cut piece from the cut file. Pass:
lettering legible at 5 mm with clean satin; fills cover (or, for meander, stipple and motif fills, lie
flat) with no puckering beyond the tatami swatch; the appliqué piece fits its placement line within
1 mm and the cover hides the raw edge; the logo sews without thread breaks.

## Phase 3: advanced production and specialties (P2) — E9

| Step | What | Size |
|---|---|---|
| **E9.1** Specialty fills | Ripple, fur (random-length radial stitches), cross-stitch (a grid of crosses from an image or area), tartan/plaid, cascade/gradient density, fractal, wave. | L |
| **E9.2** Realistic preview | Shaded 3D thread drawing (sheen by stitch angle), fabric textures generated in code, garment mockups (only with images whose licences allow it). | L |
| **E9.3** More machine formats | VP3, HUS, XXX and others as users ask, each from a published description. | M each |
| **E9.4** Estimates and costs | Thread length per colour, bobbin use, sewing time per machine speed, cost per piece. | S |
| **E9.5** Machine transfer | Export straight to USB folders laid out as each machine expects; network transfer only where the maker publishes an interface. | M |
| **E9.6** Extensions | A plug-in interface for stitch generators and checkers (extending the WASM plug-in ABI). | L |

**Gate G3 (fabric):** each specialty fill on a swatch, plus a design in each new format sewn on a machine that
reads it.

## Sew-out gates

Only the user can sew, so gates are batched: the E6.6 kit makes the designs, the user sews and fills in the
form, and the findings become fixes before the phase is marked done in `ROADMAP.md`. Between gates, quality is
checked by: the drift and property tests, the design checker on a fixed corpus of test designs (generated in
code), renders compared by eye, and the independent reader for each machine format.

## Decisions for the user

1. **Thread chart data (E7.10).** Brand charts (thread numbers and colours) are the makers' data. Options:
   (a) ship none, and let users load charts they have the right to use; (b) ship charts only where a maker
   publishes them for free use, cited; (c) generic named colours only. (a) plus (c) is the safe default.
   **Decided: (b).**
2. **Machine format priority (E7.12).** PES, JEF and EXP are done. Name the machines you sew on, so
   their formats are checked on fabric first, and any other format (VP3, HUS…) is added.
2a. **Machine thread palettes (E7.12).** PES's PEC section and JEF name threads by number in Brother's and
   Janome's built-in palettes, so a machine shows its palette's colour for each number. VectorCraft ships no
   palette (they are the makers' data, as thread charts are), writes a number per distinct colour and keeps
   PES's real colours in its version 6 list. Options: (a) keep it so; (b) ship the palettes as community
   measurements published under terms that allow redistribution, cited, if any are found; (c) let users
   load a palette as a `.vcthreads` chart and match colours to it on export. (a) now, (c) later, is the
   safe default.
3. **Fabric gates.** Which machine and fabrics the gate sew-outs use.
4. **The research catalog.** Add the 62-feature catalog to the repository (or `plan/`) so the P0/P1/P2 mapping
   can be checked item by item.
5. **The starter embroidery font (E8.1d).** (a) ship none, users digitize their own; (b) we digitize one
   original font with the editor; (c) we digitize one traced from an OFL font, renamed and under the OFL,
   in craft-fonts. (b) or (c) is needed for G2's lettering sample, unless the user sews it with an
   Ink/Stitch font they installed themselves (E8.1e; decision 9: none ships).
6. **Brother FCM (E8.8c).** No public FCM specification is known to us. Options: (a) SVG only, imported
   through CanvasWorkspace (the safe default); (b) FCM if the user finds a public specification. (FCM won't
   be worked out from Brother's software or files, by the clean-room rule.)
7. **Appliqué cover default (E8.8b).** Satin (the common choice) or zigzag (lighter, quicker) as the
   default cover, and whether several pieces share their stops by default.
8. **Order (E8).** E8's order of work puts fonts after library folders and appliqué. If lettering matters most,
   E8.1 can go first and E8.5 follow (fonts would then load one file at a time, as border patterns do now).
9. **Shipping Ink/Stitch fonts (E8.1e).** **Decided (2026-10-23):** none ships. VectorCraft supports
   using them if a user wants to, and each user installs their own as they see fit: by setting a folder
   to look in, or by placing the font folders in VectorCraft's embroidery fonts folder, as a how-to page
   (`docs/embroidery-fonts.md`) explains.

## Order of work

E5.5 and E5.6 done → E6.1–E6.6 → E7 (E7.1–E7.3 first, as the quality basics; E7.12 formats can run in
parallel) → G1 → E8 (in the order of its table: E8.5, E8.8, E8.1, E8.2, E8.4, then E8.3 after G1, E8.7, E8.6), with E10 (E10.1–E10.2 first) alongside → G2 and GC → E9 → G3. `ROADMAP.md`'s E row and `docs/embroidery.md` are updated as each step
lands.
