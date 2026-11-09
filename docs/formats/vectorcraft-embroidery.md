# Embroidery data in `.vectorcraft` documents

A `.vectorcraft` document (described in the `vectorcraft-format` crate: UTF-8 JSON, optionally gzipped,
`{"format": "vectorcraft", "version": …, "document": {…}}`) carries embroidery in two places:

- each stitched object's **stitch settings**, as its `stitch` member;
- the document's **embroidery setup**, as `document.embroidery`: travel between stitches, the sewing
  order and the border patterns the document uses.

Both are left out when they hold nothing, so a document without embroidery has neither. Machine-readable
schema for both: [vectorcraft-embroidery.schema.json](vectorcraft-embroidery.schema.json). All lengths are
millimetres, whatever the document's units.

Stitches themselves (needle points) aren't stored: a program generates them from the settings and the
object's geometry each time, so editing a path changes its stitches.

## Stitch settings (`stitch`)

Paths, compound paths and type objects may have a `stitch` object, next to the node's `id` and `kind`:

```json
{ "id": 12, "kind": { "type": "path", … }, "stitch": { "kind": "satin", "width": 4, "underlay": "edge", "pull": 0.3 } }
```

Every member but `kind` may be left out, which means its default. Members that don't apply to the kind
are kept but have no effect.

| Member | Type | Applies to | Range | Default |
|---|---|---|---|---|
| `kind` | string, see [Kinds](#kinds) | all | | `running` |
| `length` | number, mm | running, bean, fill, pattern border | 0.3–12 | fill 4; pattern border 1.5; others 2.5 |
| `spacing` | number, mm | satin, zigzag, satin shape, fill, auto, pattern border (its satin parts) | 0.1–10 | zigzag 2; others 0.4 |
| `angle` | number, degrees counter-clockwise | fill | any | 0 |
| `width` | number, mm | satin, zigzag; pattern border; running (chain and stem styles) | satin and zigzag 0.3–12, or 0.3–30 with `split` `auto`; pattern border 1–40; running 0.2–6 | satin: the stroke width (3 without one); zigzag 3; pattern border: the pattern's own width; chain 1.5, stem 0.4 (not the stroke) |
| `style` | string: `single` `chain` `stem` `backstitch` `sashiko` | running | | `single`. See [Running stitch styles](#running-stitch-styles). |
| `gap` | number, mm | running (sashiko style) | 0.5–6 | 1.5 |
| `split` | string: `auto` `off` | satin, satin shape, zigzag | | `auto`. See [Split satin](#split-satin). |
| `splitAt` | number, mm | satin, satin shape, zigzag (`split` `auto`) | 4–12 | 7 |
| `splitLength` | number, mm | satin, satin shape, zigzag (`split` `auto`) | 2–8 | 4 |
| `splitPattern` | string: `stagger` `random` | satin, satin shape, zigzag (`split` `auto`) | | `stagger` |
| `thread` | `[r, g, b]`, each 0–255 (sRGB) | all | | the stroke's colour for stitches along the path, the fill's for fills, satin shapes and auto |
| `pull` | number, mm | satin, satin shape, fill, auto | 0–3 | 0 |
| `pullPercent` | number, % | satin, satin shape, fill, auto | 0–30 | 0 |
| `push` | number, mm | satin, fill, auto | 0–3 | 0 |
| `underlay` | string | satin: `none` `auto` `center` `edge` `zigzag`; satin shape: `none` `auto` `center` `edge`; fill: `none` `auto` `fill` | | `none` (programs give new objects `auto`) |
| `stagger` | integer | fill | 2–8 | 3 |
| `motif` | string | fill | a carve motif's id: built in `diamonds` `waves` `circles` `scales` `herringbone`, or one of `motifs` | none |
| `motifSize` | number, mm | fill with a motif | 3–60 | 10 |
| `pattern` | string, a border pattern id | pattern border | | `waves` |
| `flip` | boolean | pattern border; running (stem style) | | false. A pattern border's pattern mirrored across the path; a stem stitch slanted to the right of the path instead of the left. |
| `passes` | integer, odd | bean | 3–9 | 3 |
| `thinUnder` | number, mm | auto | 0.2–3 | 1 |
| `wideOver` | number, mm | auto | 2–12, and at least `thinUnder` | 7 |
| `border` | number, mm | fill, auto | 0.5–6 | none (auto: 2 on the shapes it fills) |
| `rails` | boolean | satin | | false |
| `angles` | array of `[x1, y1, x2, y2]`, mm in document space (at most 256) | satin, satin shape, auto | | none. Stitch angle lines: where one crosses the satin's centre line (a column's path, a satin shape's strokes) the stitches take its angle relative to that line, and between lines the angle turns gradually, by distance along it; before the first and after the last the nearest line's holds; at most 60° off square. Between rails they are rungs. A stroke no line crosses keeps its own direction. They move with the object. |
| `startAt` | `[x, y]`, mm in document space | running, bean, zigzag | | none: nearest where the needle is (see `autoStart`). The path is entered at its point nearest it: a closed running path there, an open path from its nearer end. |
| `puff` | number, mm | satin, satin shape, auto | 1–6 | none: not puffed. Puff (3D foam): see below. |
| `keepCovered` | boolean | fill | | false: rows under objects sewn later on top are cut away (see `overlap`) |
| `trimBefore` | boolean | all | | false |
| `trimAfter` | boolean | all | | false |
| `font` | string, a font's `id` | type | | none: the outline font. The embroidery font (of `fonts`) the type's letters are sewn from, each from its stitch objects; a reader that can't find it sews the outline font. |
| `stopBefore` | `stop`, `placeFabric` or `trimFabric` | all | | none. The machine stops before the object for something done by hand (a plain stop, placing appliqué fabric, trimming it); an id a reader doesn't know is a plain stop. |

**Compensation.** The thread pulls the fabric in along the stitches and pushes it out at their ends. `pull` (mm) plus `pullPercent` (of the satin's width, or of each fill row's length) sews a satin that much wider and each fill row that much longer, half at each end. `push` sews a satin column along a path that much shorter (half at each end; not a closed one, nor a satin between rails) and a fill that much shorter across its rows (a row whose middle is within half of it of an edge across the rows is left out). A satin shape takes `pullPercent` of its stroke width.

**Puff.** A satin or satin shape with `puff` is sewn over foam that thick: the design stops for the foam
to be laid before the first such object (a stop, written as a thread change to the same thread, unless
nothing is sewn before it); it is sewn dense (`spacing` 0.25 mm unless set) and with no underlay; an
open column is lengthened by half the foam's thickness at each end, capping the foam's cut edge; and
after it comes a perforation, running stitches 1 mm long all round its edge, so the needle holes cut the
foam and the excess tears away cleanly.

Readers don't refuse out-of-range or non-finite numbers in a document: they use the nearest value in range,
or the default for one that isn't a number. `thread`, `stagger` and `passes` must be whole numbers from 0 to
255; otherwise the object's stitch settings can't be read and are kept as described under
[Compatibility](#compatibility). An underlay the kind doesn't use counts as `none`.

`auto` picks the underlay by stitch and width: a satin column under 2 mm wide gets `center`, 2–5 mm
`edge`, wider `edge` then `zigzag`; a fill gets an edge walk (a running stitch just inside its outline)
then `fill`; a satin shape whose strokes are over 2.5 mm wide gets `edge` (an edge walk round its
outline), thinner ones `center`. Builds from before 2026-10-18 don't know `auto` and keep such an object
unsewn (see [Compatibility](#compatibility)).

### Kinds

| `kind` | What is sewn |
|---|---|
| `running` | Stitches along the path: single ones, or a hand-stitch look by `style` (see [Running stitch styles](#running-stitch-styles)). |
| `bean` | Running stitch with each stitch sewn `passes` times. |
| `satin` | A zigzag column centred on the path, `width` wide. With `rails`, the object's two longest subpaths are the column's edges and other subpaths crossing both set the stitch direction. |
| `zigzag` | An open zigzag along the path, `width` wide, a point every `spacing`. |
| `satinShape` | Satin following a closed shape's strokes from edge to edge (lettering). |
| `fill` | Rows of staggered running stitches covering the shape, optionally carved with a `motif` and edged with a satin `border`. |
| `motifFill` | The stitched motif `motif` (`stars` when unset; see [vcmotif.md](vcmotif.md)) sewn tile by tile across the shape on its lattice, at `motifSize` and turned by `angle`: every whole tile inside the shape, the fabric showing between. |
| `patternBorder` | The border pattern `pattern` repeated along the path (see [vcborder.md](vcborder.md)). |
| `manual` | The needle goes down at every anchor point, as drawn. |
| `auto` | Chosen per shape (per letter, for type) by stroke width: under `thinUnder` a bean outline, over `wideOver` a fill with a satin border, between them a satin shape. |

Type objects are sewn letter by letter, each letter in its characters' colour unless `thread` is set.

### Running stitch styles

A running stitch's `style` sews its line as a machine version of a hand stitch. Each style repeats a figure
along the line, stretched or shrunk to a whole number of repeats between sharp corners (turns over 30°),
where it starts again; no stitch is longer than 12 mm (longer ones are split). Lines that meet are each
sewn on their own (a single running stitch sews them as one network).

| `style` | What is sewn |
|---|---|
| `single` | Single stitches `length` long. |
| `chain` | Linked loops, each `length` long and `width` wide: the loop's left side forward to its tip, its right side back to its start, then the right side again to the tip, where the next loop starts. |
| `stem` | Stitches about `length` long slanting across a band `width` wide, from the right edge to the left edge (the reverse with `flip`), each starting halfway back along the one before. |
| `backstitch` | A needle point every half `length`; from each, a stitch `length` long forward and half of one back, after a first half step forward and back, so every stretch is sewn three times. |
| `sashiko` | Dashes about `length` long with gaps about `gap` long, each dash sewn forward, back and forward, each gap crossed by one stitch; the line starts and ends on a dash (a closed line without corners has a gap where it closes too). Dashes over 4 mm are sewn in several stitches to follow curves. |

A reader that doesn't know `style` sews the line as single running stitch (and keeps the member, as
[Compatibility](#compatibility) says). Builds from before 2026-11-01 do so.

### Split satin

With `split` `auto` (the default), a satin, satin shape or zigzag stitch longer than `splitAt` is sewn as
the fewest equal pieces no longer than `splitLength` (at least two), its extra needle points on the
stitch's line. Underlay stitches that long are split the same way. A column can then be up to 30 mm
wide (12 mm with `split` `off`); a satin shape's stitches stay within 12 mm either way.

The extra points are measured from the same side of the column on every stitch (a satin's stitches
alternate direction) and shifted along it from one stitch to the next, as a fraction of one piece:

| `splitPattern` | Shift |
|---|---|
| `stagger` | 0, ⅓, ⅔, repeating; with a shift, the first and last pieces are shorter (one more piece). |
| `random` | Between 0.2 and 0.8, repeatable (the same design sews the same way), at least 0.15 from the stitch before. |

A reader that doesn't know these members sews the satin unsplit, capped at 12 mm (and keeps them).
Builds from before 2026-11-01 do so.

## Embroidery setup (`document.embroidery`)

```json
"embroidery": {
  "trimJumpsOver": 3,
  "stitchMovesUnder": 1,
  "order": [12, 7, 30],
  "patterns": [ { "id": "rope", "name": "Rope", "width": 6, "length": 8, "repeat": [ … ] } ]
}
```

| Member | Type | Meaning | Default |
|---|---|---|---|
| `trimJumpsOver` | number, mm (0.5–100), or `null` | Jumps longer than this cut the thread first. `null`: never on length (only thread changes and objects' `trimBefore`/`trimAfter` cut it). | 3 |
| `stitchMovesUnder` | number, mm (0–5) | Moves to the next stitch up to this long are sewn as a stitch rather than jumped; never more than `trimJumpsOver`. | 1 |
| `speed` | number, stitches per minute (100–1500) | The machine's sewing speed, for the sewing time estimate (information only: nothing is sewn differently). | 600 |
| `machineFormat` | string: `dst`, `pes`, `jef`, `exp`, `vp3` or `u01` | The machine file format the Embroidery and Sew-Out Test panels' Export button writes (any format can still be exported). A value a newer VectorCraft wrote is kept, and DST is used in its place. | `dst` |
| `hideTravel` | boolean | Travel to the next stitch that lies under a fill or satin shape sewn later (at least 0.5 mm inside it all the way) is sewn there as running stitches up to 2.5 mm long, which that object covers, rather than jumped and trimmed. | true |
| `autoStart` | boolean | Running, bean and zigzag stitches without a `startAt` are entered nearest where the needle is: a closed running path at its nearest point, an open path from its nearer end, separate parts nearest first. | true |
| `minStitch` | number, mm (0–2) | The shortest stitch sewn: a needle point closer than this to the one before is left out (the last point of a run moves the one before it instead). The fabric's `minStitch` is used when it is longer. Manual stitches are sewn as drawn. | 0.3 |
| `overlap` | number, mm (0–3) | **Overlap removal.** A fill's rows under an object sewn later on top of it (a fill, satin shape or Auto object by its outline, type by its letters, a satin column along a path by its band) are cut away (its underlay too), except this far in from the covering object's edge, so the two still meet; where the covering object reaches past the fill's own edge no rim is kept. A fill with `keepCovered` keeps all its rows. | 0.5 |
| `order` | array of object ids | The sewing order: these objects first, in this order, then any other stitched objects in paint order (bottom first). Ids of objects no longer there are ignored. | empty: paint order |
| `motifs` | array of motifs | The motifs the document uses beyond the built-in ones, each with the fields of a [motif file](vcmotif.md#fields) other than `format`, `version` and `units`, with a non-empty `id`; stitch settings name carve motifs by `id` in `motif`, found before a built-in one with the same id. A motif a reader can't read is kept as written and not used. | empty |
| `patterns` | array of border patterns | The border patterns the document uses beyond the starters, so it sews the same anywhere. | empty |
| `charts` | array of thread charts, see [Thread charts](#thread-charts-charts-chart) | Thread charts loaded into the document. | empty |
| `chart` | string, a chart's `id` | The chart the design's colours are matched against. | none |
| `styles` | array of stitch styles, see [Stitch styles](#stitch-styles-styles) | Stitch styles kept with the document. | empty |
| `fontEditor` | an embroidery font | An embroidery font document's font (made by the font editor): its details, metrics and kerning, with no glyphs; its glyphs are the document's artboards, one per glyph named for its characters (`space` for the space), the baseline shared. | none: an ordinary document |
| `fonts` | array of embroidery fonts | The embroidery fonts the document uses, each with the fields of a [font file](vcfont.md) other than `format` and `version`, so its type sews the same where the font isn't installed. A font a reader can't read is kept as written and not used. | empty |
| `hoop` | object, see [Hoop](#hoop-hoop) | The hoop the design is sewn in: its sewing field is checked against the design and drawn in Stitch Preview. | none: nothing checked |
| `hoopRotation` | integer: 0, 90, 180 or 270 | Degrees clockwise the design is turned as it is written to a machine file, about the middle of its stitches, so a tall design fits a wide hoop. The hoop check uses the turned design, and Stitch Preview draws the hoop's field turned the other way round the design as drawn. Readers use any other value as 0 and keep it as written. | 0 |
| `hoopings` | object, see [Hoopings](#hoopings-hoopings) | The design split into a grid of hoopings, each sewn as a design of its own. | none: sewn in one hooping |
| `fabric` | object, see [Fabric](#fabric-fabric) | The fabric the design is for: its settings fill in what objects leave unset. | none: the stitches' own defaults |
| `sewOut` | object, see [Sew-out test](#sew-out-test-sewout) | The swatches of a test sheet and what was seen when they were sewn. | none |
| `preview` | object, see [Realistic view](#realistic-view-preview) | How the Realistic view draws the fabric the design is sewn on. Changes nothing that is sewn. | none: as the fabric suggests |

Each entry of `patterns` has the fields of a [border pattern file](vcborder.md#fields) other than `format`,
`version` and `units`, with a non-empty `id`. Stitch settings name them by `id` in `pattern`; a document's
pattern is found before a starter with the same id. A `pattern` naming neither isn't sewn, and the program
warns.

Readers clamp `trimJumpsOver` and `stitchMovesUnder` into range like stitch settings.

## Thread charts (`charts`, `chart`)

Each entry of `charts` has the fields of a [thread chart file](vcthreads.md#fields) other than `format`
and `version`. `chart` names the one the design's colours are matched against (see
[Matching](vcthreads.md#matching)). Readers keep a chart they can't read exactly as written and don't use
it.

## Stitch styles (`styles`)

Each entry of `styles` has the fields of a [stitch style file](vcstyle.md#fields) other than `format`,
`version` and `units`: `id`, `name`, the credits and `stitch`. Objects don't refer to them: applying a
style copies its settings onto objects ([Applying a style](vcstyle.md#applying-a-style)). Readers keep a
style they can't read (a `stitch` they can't read) exactly as written and don't apply it.

## Hoop (`hoop`)

`document.embroidery.hoop` is one hoop of a [hoop list file](vchoop.md#fields): `id`, `name`, `width` and
`height` (the sewing field, mm), `round`, `machines`, `description`. The field is centred on the design's
stitches (machines centre designs). Readers keep a `hoop` they can't read exactly as written and check
nothing against it.

## Hoopings (`hoopings`)

`document.embroidery.hoopings` splits a design too big for its hoop into a grid of areas, each sewn as a
design of its own after the fabric is hooped again (`embroidery.splitHoops`). Positions are mm in the
stitch plan's space: document points × 25.4 / 72, y down.

```json
"hoopings": { "columns": [10, 110, 210], "rows": [5, 95], "overlap": 2, "basting": true }
```

| Member | Type | Meaning | Default |
|---|---|---|---|
| `columns` | array of 2 to 9 increasing numbers, mm | The columns' edges, left to right: n + 1 for n columns. The inner ones are the seams. | required |
| `rows` | array of 2 to 9 increasing numbers, mm | The rows' edges, top to bottom, likewise. | required |
| `overlap` | number, mm (0–10) | How far each hooping sews past its seams, so neighbouring pieces meet without a gap. | 2 |
| `marks` | boolean | Alignment marks: two running-stitch crosses (arms 4 mm from the middle, 2 mm stitches) on each seam between two hoopings that sew something, a quarter of the seam (at most 15 mm) in from each end. The hooping sewn first sews them last; the next sews them first, so the fabric can be lined up by them. | true |
| `basting` | boolean | A running-stitch box (5 mm stitches) 3 mm inside the hoop's field round each hooping, sewn first, to hold the fabric to the stabilizer. | false |

The areas are sewn row by row from the top, each row from the left; an area with none of the design's
stitches is left out. Each stitch (from the needle point before it to its own) belongs to the area its
middle lies in, the outer areas reaching on without end, so every stitch is in exactly one area. A hooping
sews the stitches of its area and those whose middle lies within `overlap` of it; a run cut at the edge of
that is locked with a short stitch back (0.6 mm) on each side. The hoop's field is centred on each area,
turned by `hoopRotation`, and each hooping's file is written turned like a whole design. Readers keep
`hoopings` they can't read (or a grid that isn't 2 to 9 finite, increasing edges each way) exactly as
written and sew the design in one hooping.

## Fabric (`fabric`)

`document.embroidery.fabric` has the fields of a [fabric profile file](vcfabric.md#fields) other than
`format`, `version` and `units`. Each of its settings fills in that setting for every object that leaves it
unset, as if the object had it:

- `satinSpacing`: the `spacing` of satins and satin shapes; `fillSpacing`: the `spacing` of fills.
- `pull` and `pullPercent`: those of satins, satin shapes and fills; `push`: that of satins and fills.
- `satinCenterUnder`, `satinZigzagOver` and `shapeEdgeOver`: the thresholds an `auto` underlay picks by
  (in place of 2, 5 and 2.5 mm).
- `minStitch`: the shortest stitch, when longer than the document's own `minStitch`.

An object's Auto stitch takes them once it has picked what to sew each shape with. Readers keep unknown
members of `fabric`.

## Realistic view (`preview`)

How View › Realistic Stitches and `embroidery.preview {style: realistic}` draw the fabric under the
stitches. It is only a picture: nothing sewn depends on it.

| Field | Type | Meaning | Default |
|---|---|---|---|
| `texture` | string: `plain`, `twill`, `denim`, `knit`, `terry`, `leather`, `felt` or `none` | The fabric's surface; `none` draws no fabric, only the threads over what is under them. | the texture of `fabric`'s starter (`knit` for knit, `terry` for towel, `twill` for cap, `leather`, `denim`), else `plain` |
| `color` | string, `#rrggbb` | The fabric's colour. | the texture's own |

Readers draw a `texture` or `color` they don't recognise as unset, and keep it, and any other member, as
written.

## Sew-out test (`sewOut`)

`embroidery.testSheet` makes a test sheet: a new document of calibration swatches, one artboard (a
100 × 100 mm design) per section, every swatch labelled in stitches. The record of the swatches, and what
the person who sewed them saw, is `document.embroidery.sewOut`:

```json
"sewOut": {
  "fabric": "cotton twill, tear-away",
  "machine": "single-needle, 600 spm",
  "notes": "",
  "swatches": [
    { "id": "A3", "section": "satinWidth", "label": "Satin 3 mm wide", "objects": [41, 42], "result": "pass" },
    { "id": "A9", "section": "satinWidth", "label": "Satin 10 mm wide", "objects": [57, 58], "result": "fail", "notes": "loops" }
  ]
}
```

| Member | Type | Meaning | Default |
|---|---|---|---|
| `fabric` | string | The fabric and stabilizer sewn on. | empty |
| `machine` | string | The machine, needle and speed. | empty |
| `notes` | string | Notes on the whole sew-out. | empty |
| `swatches` | array | One entry per swatch, in the sheet's order. | empty |

Each swatch:

| Member | Type | Meaning | Default |
|---|---|---|---|
| `id` | string | Its name on the sheet: the section's letter, then its number (`A3`). | |
| `section` | string | The section, one of the ids below. | |
| `label` | string | What it tests, in words. | |
| `objects` | array of object ids | The objects it is sewn from, its sewn label included. | empty |
| `result` | string | `pass`, `fail`, or empty: not judged yet. A reader treats any other value as not judged, and keeps it. | empty |
| `notes` | string | What was seen. | empty |
| `angle` | number, degrees | A compensation swatch's stitch direction, counter-clockwise from the sheet's horizontal, the fabric hooped with its grain running top to bottom. | none |
| `drawnWidth` | number, mm | A compensation swatch's drawn width along the stitches (a satin column's width, a fill square's side along its rows), where the thread pulls the fabric in. | none |
| `drawnLength` | number, mm | Its drawn length across the stitches (a satin column's length, a fill square's side across its rows), where the stitch ends push the fabric out. | none |
| `measuredWidth` | number, mm | The width measured on the sewn swatch, between the marks at its drawn edges. | none |
| `measuredLength` | number, mm | The length measured on the sewn swatch. | none |

Sections (each changes one setting; the others keep their defaults). A test sheet made without naming
its sections has all but the compensation sections; `compensation` names those four, the compensation
sheet:

| Letter | `section` | Swatches |
|---|---|---|
| A | `satinWidth` | Satin columns 30 mm long, 1, 2, 3, 4, 5, 6, 7, 8 and 10 mm wide. |
| B | `satinDensity` | 4 mm satin columns at spacing 0.3, 0.35, 0.4, 0.5, 0.6 and 0.8 mm. |
| C | `pullCompensation` | 3 mm satin columns with pull compensation 0 to 0.5 mm, each between running-stitch marks at the drawn edges. |
| D | `fillDensity` | 18 mm fill squares at row spacing 0.3, 0.4, 0.5 and 0.6 mm. |
| E | `fillAngle` | 18 mm fill squares at 0°, 45°, 90° and 135°. |
| F | `underlay` | 5 mm satin columns with each satin underlay (`auto` included), then 18 mm fill squares with each fill underlay. |
| G | `lettering` | The word "Hag" in auto stitches at 4, 6, 8, 10, 15 and 20 mm type sizes (left out when no type face is available). |
| H | `runStyles` | Running stitch in each `style` (single, chain, stem, backstitch, sashiko) on a 22 × 14 mm S curve ending in a sharp corner. |
| I | `splitSatin` | 30 mm satin columns 8, 12 and 20 mm wide split with the defaults (staggered), 12 mm split at random, and 12 mm not split to compare. |
| K | `compensation0` | The compensation sheet at 0° (the stitches across the grain): satin columns 30 mm long, 2, 4, 6 and 8 mm wide, and an 18 mm fill square, sewn with `pull`, `pullPercent` and `push` at 0, each between running-stitch marks at its drawn edges. The title stays upright; the swatches are laid out with the satins' stitches and the fill's rows across, then turned to the section's angle. |
| L | `compensation45` | The same at 45°. |
| M | `compensation90` | The same at 90° (the stitches along the grain). |
| N | `compensation135` | The same at 135°. Listed before `puff`, which stays last; the letters K to N were added after J (2026-11-09). |
| J | `puff` | 3 mm puff (satin over 3 mm foam) 3, 4, 5, 6 and 8 mm wide; the design stops for the foam after its title. Last, since anything sewn after the foam is laid sews through it (before 2026-11-01 it was H, then I). |

## Compatibility

New stitch types, settings and patterns are added without a new document version, so a document made
by a newer VectorCraft opens in an older one. A reader must:

- **Keep members it doesn't know**, in a `stitch` object, in `document.embroidery`, in a pattern, in a part, in
  `sewOut` or in a swatch, and write them back when it saves.
- **Keep a `stitch` object it can't read** (a `kind`, `underlay`, `style`, `split` or `splitPattern` value it doesn't know, or a value of the
  wrong type, such as `thread` past 255) exactly as written, not sew that object, and warn. Choosing a
  stitch for the object replaces it.
- **Keep a style it can't read** (in `styles`, or a graphic style's `stitch`) exactly as written, and not
  apply it.
- **Keep a `fabric` it can't read** (a value of a type it doesn't expect) exactly as written, and sew as if
  there were none.
- **Keep a pattern it can't read** (a part `stitch` it doesn't know) exactly as written, not sew with it,
  and warn. It can still be removed.

Never refuse the whole document because of embroidery data. VectorCraft follows these rules from
2026-10-17 (E6.1). Earlier builds refuse a document with a `kind`, `underlay` or part `stitch` they don't
know: `patternBorder` (added 2026-10-16) doesn't open in them.
