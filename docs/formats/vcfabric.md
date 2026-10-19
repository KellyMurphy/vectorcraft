# Fabric profile file (`.vcfabric`), version 1

A fabric profile holds the settings a fabric wants: how dense the stitches are, how much push and pull
compensation, and how much underlay the automatic underlay gives. A document's fabric (see
[`fabric`](vectorcraft-embroidery.md#fabric-fabric)) fills in whatever an object's own stitch settings
leave unset; an object's own settings always win. The file holds one profile, with who made it and under
what licence, so it can be shared.

Machine-readable schema: [vcfabric.schema.json](vcfabric.schema.json). Example:
[examples/polo.vcfabric](examples/polo.vcfabric).

## File

- UTF-8 JSON, one object, at most **1 MB** (2²⁰ bytes).
- Extension `.vcfabric`. There is no registered media type; serve it as `application/json`.

```json
{
  "format": "vectorcraft-fabric",
  "version": 1,
  "units": "mm",
  "id": "polo",
  "name": "Polo Shirt",
  "author": "A. Digitizer",
  "license": "CC0-1.0",
  "description": "Pique knit with a cut-away stabilizer.",
  "satinSpacing": 0.42,
  "fillSpacing": 0.45,
  "pull": 0.3,
  "pullPercent": 5,
  "push": 0.2,
  "satinCenterUnder": 1.5,
  "satinZigzagOver": 4,
  "shapeEdgeOver": 2
}
```

## Fields

Every field but `format` and `version` may be left out. A setting left out keeps the stitch's own default.

| Field | Type | Range | Meaning |
|---|---|---|---|
| `format` | string | `"vectorcraft-fabric"` | Names the format. |
| `version` | integer | 1 | The format version. A reader refuses a later version. |
| `units` | string | `"mm"` | All lengths are millimetres. |
| `id` | string | | Lower case letters and digits joined by dashes. Made from `name` when absent or empty. |
| `name` | string | | Shown to the user. `id` when empty. |
| `author`, `license`, `source`, `description` | string | | Credits and notes: who made it, its licence (an SPDX identifier, or text), where it came from. Information only. |
| `satinSpacing` | number, mm | 0.1–10 | Spacing of satin columns and satin shapes. |
| `fillSpacing` | number, mm | 0.1–10 | Row spacing of fills. |
| `pull` | number, mm | 0–3 | Pull compensation of satins, satin shapes and fills. |
| `pullPercent` | number, % | 0–30 | More pull compensation, as a percentage of the width or row length. |
| `push` | number, mm | 0–3 | Push compensation of satin columns and fills. |
| `satinCenterUnder` | number, mm | 0.3–12 | Automatic underlay: satin columns narrower than this get a centre walk. |
| `satinZigzagOver` | number, mm | 0.3–12, at least `satinCenterUnder` | Automatic underlay: satin columns wider than this get an edge walk and a zigzag; those between, an edge walk. |
| `shapeEdgeOver` | number, mm | 0.3–12 | Automatic underlay: satin shapes with strokes wider than this get an edge walk; thinner, a centre walk. |

The settings mean what the stitch settings of the same names mean (see
[vectorcraft-embroidery.md](vectorcraft-embroidery.md#stitch-settings-stitch)). A reader refuses a file
whose numbers are negative or not finite, and uses the nearest value in range for one outside its range.
Unknown fields are kept and written back.

## Starter profiles

VectorCraft makes six profiles itself (they aren't files): `woven`, `knit`, `towel` (fleece and towels),
`cap`, `leather` and `denim`. Their numbers are starting points from general digitizing practice:
stretchy and lofty fabrics want more underlay and compensation, and leather wants fewer needle holes.
Sew-out test sheets are how they get checked on real fabric.
