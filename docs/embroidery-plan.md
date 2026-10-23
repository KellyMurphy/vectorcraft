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
| **E7.5** Start/stop points and auto-sequencing (**partly done**: start points, automatic entry for lines, Optimize; canvas handles, branching and hidden travel still to do) | Per-object start and stop points (handles on the canvas). Within a thread, objects are ordered and each started at the end nearest the last stop, cutting jumps and trims. Running-stitch networks are sewn by branching (each line once, travel doubled under it). Travel hidden under fills that are sewn later. | L |
| **E7.6** Small-stitch filter (**done**) | A document minimum stitch length (from the fabric profile). Shorter stitches are merged or dropped everywhere, satin columns included. Short stitches on tight satin curves, as satin shape already does. | S |
| **E7.7** Checker rules (**done**) | Stitches over 7 mm (snag risk) or 12 mm (impossible); satin under 1 mm or over 10 mm wide; objects too small to sew; density hot spots (stacked layers, from a density map); open shapes set to fill; jumps and trims over budget; design beyond the hoop; type below its safe size. | M |
| **E7.8** Hoops (**done**: common nominal sizes, no makers' lists) | Hoop templates as data: size, sewing field and machine family, from manufacturers' public spec sheets (sizes only). The design is placed in a hoop, overflow is checked (E7.7), and the hoop is drawn in Stitch Preview and on the canvas. Open `.vchoop` lists (spec + schema). | M |
| **E7.9** Stitch player and inspection (**done**, in the Sew Order panel) | A player in Stitch Preview: play, step and scrub the sewing in order, by stitch or by colour stop; per-object stitch counts and the time to sew at a set speed. | M |
| **E7.10** Thread charts (**done**; the user chose (b): no maker publishes a chart under redistributable terms yet, so none ships; users load their own) | Threads by brand and number, matched to the nearest colour, shown in the panel and sew order. An open `.vcthreads` chart format (spec + schema). **Needs a decision** (see below) on where chart data may come from. | M |
| **E7.11** Production worksheet (**done**; its words are English for now) | A printable sheet (PDF through the existing PDF export, and on screen): design picture, size, stitch count, hoop, start point, colour sequence with thread names, numbers and chips, estimated time and thread used, notes. | M |
| **E7.12** PES, JEF, EXP (**done**; PES keeps thread colours in its version 6 list; no maker's palette ships, so PEC and JEF thread numbers are one per distinct colour; checked against pyembroidery, not yet sewn) | Read and write, from published format descriptions, cited in `docs/embroidery.md`. Colours come through where the format keeps them (PES and JEF keep thread colours). Fuzzed readers. Checked by hand against an independent reader, as DST was. | L |
| **E7.13** Lettering basics | Verify and test type on a path, stitched. Letter spacing and baseline controls checked against stitched type. Minimum safe sizes per stitch kind, which the checker (E7.7) warns about. | S |
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

| Step | What | Size |
|---|---|---|
| **E8.1** Embroidery fonts | The planned E5 sprint: an open font format (spec + schema), a font editor (one artboard per letter, metrics, kerning, a live sample line), and type set in embroidery fonts sewn from their letters. Fonts live in craft-fonts. | XL |
| **E8.2** Monograms and text effects | Monogram layouts (2 and 3 letters, sizes and positions), text on arcs and in envelopes, all as live type that stays editable. | M |
| **E8.3** Fill families | **Contour** (rings following the outline), **spiral**, **radial/circular** (around a set centre), **guided** (rows bending along guide lines drawn in the object), **meander/stipple** (a running stitch wandering through the area without crossing itself). Each one is a kind module (E6.3) with its own parameters (E6.2). | L |
| **E8.4** Motif fills and extraction | The planned E5 sprint: stitched motif tiles filling a shape, motif extraction from a DST region (two repeat offsets), and an open `.vcmotif` format (spec + schema). User carving patterns use the same format. | L |
| **E8.5** Library folders | User folders for border patterns, motifs, fonts, thread charts, fabric profiles and hoops, listed in the panels (on the existing swatch-library folder support), shared by copying files. | M |
| **E8.6** Auto-digitize | Image Trace → regions → a stitch kind for each region by its shape and size (extending Auto) → sequence (E7.5) → check (E7.7). **Click-to-fill** and **click-to-outline** on a traced or placed image, or on overlapping paths (as Live Paint finds regions). | XL |
| **E8.7** Density adviser | The checker's density map drawn over the design, with a suggested fix per finding (lower density, add trims, remove the overlap), applied in one click. | M |
| **E8.8** Appliqué and cut files | The planned E5 sprint: placement line, tack-down and cover from a shape, with stops; nested cutter export (SVG/DXF; Brother FCM only from a public specification). | L |

**Gate G2 (fabric):** a lettering sample in an embroidery font at 5–30 mm, each new fill family on a
10 × 10 cm swatch, an auto-digitized logo with no hand fixes, and an appliqué patch.

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

## Order of work

E5.5 and E5.6 done → E6.1–E6.6 → E7 (E7.1–E7.3 first, as the quality basics; E7.12 formats can run in
parallel) → G1 → E8, with E10 (E10.1–E10.2 first) alongside → G2 and GC → E9 → G3. `ROADMAP.md`'s E row and `docs/embroidery.md` are updated as each step
lands.
