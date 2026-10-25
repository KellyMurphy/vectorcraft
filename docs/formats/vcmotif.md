# Motif file (`.vcmotif`), version 1

A motif is one tile of a pattern that repeats both ways across an area, on a lattice: a copy of the tile
at every whole-number sum of two offsets. A **carve** motif is line art that a fill's needle points fall
on, so it shows in the fill's stitching (a carved fill, the stitch setting `motif`). A **stitched** motif
is sewn tile by tile as stitches of its own, the fabric showing between (a motif fill). The file holds
one motif, with who made it and under what licence, so it can be shared. VectorCraft's built-in carve
motifs are this same data, made in code.

Machine-readable schema: [vcmotif.schema.json](vcmotif.schema.json). Example:
[examples/crosses.vcmotif](examples/crosses.vcmotif).

## File

- UTF-8 JSON, one object, at most **8 MB** (8 × 2²⁰ bytes).
- Extension `.vcmotif`. There is no registered media type; serve it as `application/json`.

```json
{
  "format": "vectorcraft-motif",
  "version": 1,
  "units": "mm",
  "id": "crosses",
  "name": "Crosses",
  "author": "VectorCraft contributors",
  "license": "CC0-1.0",
  "source": "drawn for VectorCraft",
  "description": "Small bean-stitch crosses on a brick lattice.",
  "kind": "stitched",
  "size": 6,
  "offsets": [[6, 0], [3, 5]],
  "parts": [
    { "stitch": "bean", "points": [[-1.5, -1.5], [1.5, 1.5]] },
    { "stitch": "bean", "points": [[1.5, -1.5], [-1.5, 1.5]] }
  ]
}
```

## Fields

| Field | Type | Required | Meaning |
|---|---|---|---|
| `format` | string | yes | Always `"vectorcraft-motif"`. |
| `version` | integer | yes | `1`. Readers refuse a version newer than they know. |
| `units` | string | no | `"mm"`, the only unit. Any other value is refused. |
| `id` | string | no | What documents call the motif. Made as a border pattern's is ([Ids](vcborder.md#ids)); when nothing is left it is `pattern`. |
| `name` | string | yes | The motif's name, shown to users. |
| `author` | string | no | Who made it. |
| `license` | string | no | Its licence: an [SPDX identifier](https://spdx.org/licenses/) or text. |
| `source` | string | no | Where it came from: a URL, or the design it was taken from. |
| `description` | string | no | Free text. |
| `kind` | `"stitched"` \| `"carve"` | no (`"stitched"`) | What the motif is used as (see above). |
| `size` | number | yes | The tile's size at its own scale, mm, 0.01–1000: what an object's motif size scales. `offsets` and `points` are mm at this size. |
| `offsets` | two `[x, y]` | yes | The lattice: the offsets from a tile to its neighbours, mm at `size`. Each 0.01–1000 mm long, and not along one line. |
| `parts` | array of [parts](vcborder.md#parts) | yes | The tile's lines, in its own space: at least one with two or more points. |

`author`, `license`, `source` and `description` are information only, as in border patterns. Fields a
reader doesn't know are kept and written back. A file with a part `stitch` the reader doesn't know is
refused; a motif like that inside a document is kept as written but not used.

The built-in carve motifs use the ids `diamonds`, `waves`, `circles`, `scales` and `herringbone`, each a
unit tile (`size` 1) with offsets `[1, 0]` and `[0, 1]` (`scales`: `[0.5, 1]`, rows half a tile over). A
document's motif is found before a built-in one with the same id; loading a motif whose id is taken gives
it a free numbered one, as border patterns do.

## The tile's space

Points are `[x, y]` in millimetres at the motif's `size`, x to the right and y down, from the tile's
origin. Copies of the tile sit at every `i × offsets[0] + j × offsets[1]` (whole numbers `i`, `j`),
anchored to the page's origin, so neighbouring areas with the same motif line up. Lines may reach past
the lattice cell (neighbours overlap) or stay inside it (fabric shows between them).

When used, the motif is scaled so `size` becomes the object's motif size (x and y alike). Over an area,
every tile that meets the area's bounding box is laid; what lies outside the area is left out.

- **Carve**: only the lines' shape matters; the fill's needle points are moved onto them (each part's
  `stitch` is ignored).
- **Stitched**: each tile's parts are sewn in order (as a border pattern's: running, bean, satin or
  manual), the needle going straight from one part to the next.

## Limits

A motif has at most **100,000 points** in its parts. Points are within ±1,000,000 mm. A tiling that would
lay more than 200,000 line segments over an area is not laid, and the program warns.
