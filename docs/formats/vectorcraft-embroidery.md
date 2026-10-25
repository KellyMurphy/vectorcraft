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
| `width` | number, mm | satin, zigzag; pattern border | satin and zigzag 0.3–12; pattern border 1–40 | satin: the stroke width (3 without one); zigzag 3; pattern border: the pattern's own width |
| `thread` | `[r, g, b]`, each 0–255 (sRGB) | all | | the stroke's colour for stitches along the path, the fill's for fills, satin shapes and auto |
| `pull` | number, mm | satin, satin shape, fill, auto | 0–3 | 0 |
| `pullPercent` | number, % | satin, satin shape, fill, auto | 0–30 | 0 |
| `push` | number, mm | satin, fill, auto | 0–3 | 0 |
| `underlay` | string | satin: `none` `auto` `center` `edge` `zigzag`; satin shape: `none` `auto` `center` `edge`; fill: `none` `auto` `fill` | | `none` (programs give new objects `auto`) |
| `stagger` | integer | fill | 2–8 | 3 |
| `motif` | string | fill | `diamonds` `waves` `circles` `scales` `herringbone` | none |
| `motifSize` | number, mm | fill with a motif | 3–60 | 10 |
| `pattern` | string, a border pattern id | pattern border | | `waves` |
| `flip` | boolean | pattern border | | false |
| `passes` | integer, odd | bean | 3–9 | 3 |
| `thinUnder` | number, mm | auto | 0.2–3 | 1 |
| `wideOver` | number, mm | auto | 2–12, and at least `thinUnder` | 7 |
| `border` | number, mm | fill, auto | 0.5–6 | none (auto: 2 on the shapes it fills) |
| `rails` | boolean | satin | | false |
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
| `running` | Single stitches along the path. |
| `bean` | Running stitch with each stitch sewn `passes` times. |
| `satin` | A zigzag column centred on the path, `width` wide. With `rails`, the object's two longest subpaths are the column's edges and other subpaths crossing both set the stitch direction. |
| `zigzag` | An open zigzag along the path, `width` wide, a point every `spacing`. |
| `satinShape` | Satin following a closed shape's strokes from edge to edge (lettering). |
| `fill` | Rows of staggered running stitches covering the shape, optionally carved with a `motif` and edged with a satin `border`. |
| `patternBorder` | The border pattern `pattern` repeated along the path (see [vcborder.md](vcborder.md)). |
| `manual` | The needle goes down at every anchor point, as drawn. |
| `auto` | Chosen per shape (per letter, for type) by stroke width: under `thinUnder` a bean outline, over `wideOver` a fill with a satin border, between them a satin shape. |

Type objects are sewn letter by letter, each letter in its characters' colour unless `thread` is set.

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
| `machineFormat` | string: `dst`, `pes`, `jef` or `exp` | The machine file format the Embroidery and Sew-Out Test panels' Export button writes (any format can still be exported). A value a newer VectorCraft wrote is kept, and DST is used in its place. | `dst` |
| `hideTravel` | boolean | Travel to the next stitch that lies under a fill or satin shape sewn later (at least 0.5 mm inside it all the way) is sewn there as running stitches up to 2.5 mm long, which that object covers, rather than jumped and trimmed. | true |
| `autoStart` | boolean | Running, bean and zigzag stitches without a `startAt` are entered nearest where the needle is: a closed running path at its nearest point, an open path from its nearer end, separate parts nearest first. | true |
| `minStitch` | number, mm (0–2) | The shortest stitch sewn: a needle point closer than this to the one before is left out (the last point of a run moves the one before it instead). The fabric's `minStitch` is used when it is longer. Manual stitches are sewn as drawn. | 0.3 |
| `overlap` | number, mm (0–3) | **Overlap removal.** A fill's rows under an object sewn later on top of it (a fill, satin shape or Auto object by its outline, type by its letters, a satin column along a path by its band) are cut away (its underlay too), except this far in from the covering object's edge, so the two still meet; where the covering object reaches past the fill's own edge no rim is kept. A fill with `keepCovered` keeps all its rows. | 0.5 |
| `order` | array of object ids | The sewing order: these objects first, in this order, then any other stitched objects in paint order (bottom first). Ids of objects no longer there are ignored. | empty: paint order |
| `patterns` | array of border patterns | The border patterns the document uses beyond the starters, so it sews the same anywhere. | empty |
| `charts` | array of thread charts, see [Thread charts](#thread-charts-charts-chart) | Thread charts loaded into the document. | empty |
| `chart` | string, a chart's `id` | The chart the design's colours are matched against. | none |
| `fontEditor` | an embroidery font | An embroidery font document's font (made by the font editor): its details, metrics and kerning, with no glyphs; its glyphs are the document's artboards, one per glyph named for its characters (`space` for the space), the baseline shared. | none: an ordinary document |
| `fonts` | array of embroidery fonts | The embroidery fonts the document uses, each with the fields of a [font file](vcfont.md) other than `format` and `version`, so its type sews the same where the font isn't installed. A font a reader can't read is kept as written and not used. | empty |
| `hoop` | object, see [Hoop](#hoop-hoop) | The hoop the design is sewn in: its sewing field is checked against the design and drawn in Stitch Preview. | none: nothing checked |
| `fabric` | object, see [Fabric](#fabric-fabric) | The fabric the design is for: its settings fill in what objects leave unset. | none: the stitches' own defaults |
| `sewOut` | object, see [Sew-out test](#sew-out-test-sewout) | The swatches of a test sheet and what was seen when they were sewn. | none |

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

## Hoop (`hoop`)

`document.embroidery.hoop` is one hoop of a [hoop list file](vchoop.md#fields): `id`, `name`, `width` and
`height` (the sewing field, mm), `round`, `machines`, `description`. The field is centred on the design's
stitches (machines centre designs). Readers keep a `hoop` they can't read exactly as written and check
nothing against it.

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

Sections (each changes one setting; the others keep their defaults):

| Letter | `section` | Swatches |
|---|---|---|
| A | `satinWidth` | Satin columns 30 mm long, 1, 2, 3, 4, 5, 6, 7, 8 and 10 mm wide. |
| B | `satinDensity` | 4 mm satin columns at spacing 0.3, 0.35, 0.4, 0.5, 0.6 and 0.8 mm. |
| C | `pullCompensation` | 3 mm satin columns with pull compensation 0 to 0.5 mm, each between running-stitch marks at the drawn edges. |
| D | `fillDensity` | 18 mm fill squares at row spacing 0.3, 0.4, 0.5 and 0.6 mm. |
| E | `fillAngle` | 18 mm fill squares at 0°, 45°, 90° and 135°. |
| F | `underlay` | 5 mm satin columns with each satin underlay (`auto` included), then 18 mm fill squares with each fill underlay. |
| H | `puff` | 3 mm puff (satin over 3 mm foam) 3, 4, 5, 6 and 8 mm wide; the design stops for the foam after its title. |
| G | `lettering` | The word "Hag" in auto stitches at 4, 6, 8, 10, 15 and 20 mm type sizes (left out when no type face is available). |

## Compatibility

New stitch types, settings and patterns are added without a new document version, so a document made
by a newer VectorCraft opens in an older one. A reader must:

- **Keep members it doesn't know**, in a `stitch` object, in `document.embroidery`, in a pattern, in a part, in
  `sewOut` or in a swatch, and write them back when it saves.
- **Keep a `stitch` object it can't read** (a `kind` or `underlay` value it doesn't know, or a value of the
  wrong type, such as `thread` past 255) exactly as written, not sew that object, and warn. Choosing a
  stitch for the object replaces it.
- **Keep a `fabric` it can't read** (a value of a type it doesn't expect) exactly as written, and sew as if
  there were none.
- **Keep a pattern it can't read** (a part `stitch` it doesn't know) exactly as written, not sew with it,
  and warn. It can still be removed.

Never refuse the whole document because of embroidery data. VectorCraft follows these rules from
2026-10-17 (E6.1). Earlier builds refuse a document with a `kind`, `underlay` or part `stitch` they don't
know: `patternBorder` (added 2026-10-16) doesn't open in them.
