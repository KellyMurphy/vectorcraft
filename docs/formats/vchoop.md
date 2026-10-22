# Hoop list file (`.vchoop`), version 1

A hoop holds the fabric while a machine sews; the design must fit its **sewing field**, the area the
machine can reach in it. A hoop list file holds one or more hoops, so a machine's set of hoops can be
shared. A document's hoop (see [`hoop`](vectorcraft-embroidery.md#hoop-hoop)) is checked against the
design and drawn in Stitch Preview.

Machine-readable schema: [vchoop.schema.json](vchoop.schema.json). Example:
[examples/frames.vchoop](examples/frames.vchoop).

## File

- UTF-8 JSON, one object, at most **1 MB** (2²⁰ bytes) and **1,000 hoops**.
- Extension `.vchoop`. There is no registered media type; serve it as `application/json`.

```json
{
  "format": "vectorcraft-hoops",
  "version": 1,
  "units": "mm",
  "hoops": [
    { "id": "small", "name": "Small frame", "width": 100, "height": 100, "machines": "single-head" },
    { "id": "jacket", "name": "Jacket back", "width": 300, "height": 300, "description": "For jacket backs." },
    { "id": "ring", "name": "Round frame", "width": 150, "height": 150, "round": true }
  ]
}
```

## Fields

| Field | Type | Meaning |
|---|---|---|
| `format` | string | `"vectorcraft-hoops"`. |
| `version` | integer | 1. A reader refuses a later version. |
| `units` | string | `"mm"`: all lengths are millimetres. |
| `hoops` | array | 1 to 1,000 hoops. |

Each hoop:

| Field | Type | Meaning |
|---|---|---|
| `id` | string | Lower case letters and digits joined by dashes. Made from `name` when absent or empty. |
| `name` | string | Shown to the user. `id` when empty. |
| `width`, `height` | number, mm, above 0 | The sewing field: across the machine, and front to back. Programs use 10–2000 mm. |
| `round` | boolean | A round field: the largest circle in `width` × `height`. Default false. |
| `machines` | string | Which machines take it, in words (information only). |
| `description` | string | Notes. |

A reader refuses a file whose hoops lack a finite width or height above 0. Unknown fields are kept and
written back.

## Fitting

Machines centre the design in the hoop, so the field is centred on the design's stitches. A design fits a
rectangular field when its width and height are within the field's; a round one when its corners are
within the circle. A design that fits only turned a quarter turn is reported as such.

## Common hoops

VectorCraft makes six nominal sizes itself (they aren't files): `4x4` (100 × 100 mm), `5x7`
(130 × 180 mm), `6x10` (160 × 260 mm), `8x8` (200 × 200 mm), `8x12` (200 × 300 mm) and `cap`
(130 × 60 mm). A machine's real field can be a little smaller than the nominal size: check its manual.
