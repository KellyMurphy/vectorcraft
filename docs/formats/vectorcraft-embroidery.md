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
| `pull` | number, mm | satin, satin shape, auto | 0–3 | 0 |
| `underlay` | string | satin: `none` `center` `edge` `zigzag`; satin shape: `none` `center`; fill: `none` `fill` | | `none` |
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
| `trimBefore` | boolean | all | | false |
| `trimAfter` | boolean | all | | false |

Readers don't refuse out-of-range or non-finite numbers in a document: they use the nearest value in range,
or the default for one that isn't a number. `thread`, `stagger` and `passes` must be whole numbers from 0 to
255; otherwise the object's stitch settings can't be read and are kept as described under
[Compatibility](#compatibility). An underlay the kind doesn't use counts as `none`.

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
| `order` | array of object ids | The sewing order: these objects first, in this order, then any other stitched objects in paint order (bottom first). Ids of objects no longer there are ignored. | empty: paint order |
| `patterns` | array of border patterns | The border patterns the document uses beyond the starters, so it sews the same anywhere. | empty |

Each entry of `patterns` has the fields of a [border pattern file](vcborder.md#fields) other than `format`,
`version` and `units`, with a non-empty `id`. Stitch settings name them by `id` in `pattern`; a document's
pattern is found before a starter with the same id. A `pattern` naming neither isn't sewn, and the program
warns.

Readers clamp `trimJumpsOver` and `stitchMovesUnder` into range like stitch settings.

## Compatibility

New stitch types, settings and patterns are added without a new document version, so a document made
by a newer VectorCraft opens in an older one. A reader must:

- **Keep members it doesn't know**, in a `stitch` object, in `document.embroidery`, in a pattern or in a
  part, and write them back when it saves.
- **Keep a `stitch` object it can't read** (a `kind` or `underlay` value it doesn't know, or a value of the
  wrong type, such as `thread` past 255) exactly as written, not sew that object, and warn. Choosing a
  stitch for the object replaces it.
- **Keep a pattern it can't read** (a part `stitch` it doesn't know) exactly as written, not sew with it,
  and warn. It can still be removed.

Never refuse the whole document because of embroidery data. VectorCraft follows these rules from
2026-10-17 (E6.1). Earlier builds refuse a document with a `kind`, `underlay` or part `stitch` they don't
know: `patternBorder` (added 2026-10-16) doesn't open in them.
