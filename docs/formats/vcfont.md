# Embroidery font file (`.vcfont`), version 1

An embroidery font: lettering digitized for embroidery. Each letter is a set of **stitch objects**
(outlines, each with the stitch settings it is sewn with), not needle points, so letters scale and
their stitches are made at each size, keeping their density. The file is self-contained: it doesn't
need, extend or embed an installed font. It may name the font it was digitized from, as information
only.

VectorCraft keeps a copy of each font a document uses inside the document (`document.embroidery.fonts`,
see [vectorcraft-embroidery.md](vectorcraft-embroidery.md)), so a design sews the same where the font
isn't installed. JSON Schema: [vcfont.schema.json](vcfont.schema.json). Example:
[examples/sample.vcfont](examples/sample.vcfont).

## Coordinates and units

Outlines, advances and metrics are in **font units**, with **y up** from the baseline, as in other font
formats. `unitsPerEm` font units are one em: type set at 10 mm per em draws a letter 700 units tall
7 mm tall. Stitch settings keep their own units (mm: spacing, lengths, widths), so a letter sewn small or
large has the same density.

## The file

A JSON object:

| Member | Type | Meaning |
|---|---|---|
| `format` | `"vectorcraft-embroidery-font"` | Required. |
| `version` | integer | Required. The file format's version: 1. A reader refuses a later version. |
| `id` | string | Lower-case letters and digits joined by dashes. Made from `name` when absent or empty. |
| `name` | string | The font's name, as listed in the type controls. |
| `family`, `style` | string | Optional: the family it belongs to and its style within it (`Regular`, `Bold`…). |
| `author` | string | Optional: who made it. |
| `license` | string | Optional: an SPDX identifier (`OFL-1.1`, `CC0-1.0`…) or the licence's text. |
| `source` | string | Optional: a URL, or the font it was digitized from (information only). |
| `description` | string | Optional. |
| `fontVersion` | string | Optional: the font's own version (`version` is the file format's). |
| `unitsPerEm` | number | Font units per em, 16–16384 (1000 when absent). |
| `ascender`, `descender` | number | How far letters reach above the baseline (positive) and below it (negative), font units. |
| `capHeight`, `xHeight` | number | The height of capitals and of lower-case `x`, font units. |
| `sizes` | `[smallest, largest]` | Optional: the cap heights in mm the font is made for, 1–300. VectorCraft warns about type set outside them. |
| `glyphs` | array of glyphs | The letters. |
| `kerning` | array of pairs | Optional: pairs set closer or further apart than their advances say. |

Every number must be finite and within ten em of the baseline (advances: 0 to ten em). Members a reader
doesn't know are kept and written back unchanged, so files from newer versions survive a round trip.

### Glyphs

| Member | Type | Meaning |
|---|---|---|
| `chars` | string | The character it sets, or 2–8 characters for a ligature or symbol (`"fi"`). No two glyphs set the same `chars`. |
| `advance` | number | How far the pen moves after it, font units. |
| `objects` | array of objects | Its stitch objects, in sewing order. Empty for a space. |
| `entry`, `exit` | `[x, y]` | Optional: where the needle comes into the letter and leaves it, font units. Script letters join here without a trim. |

### Stitch objects

| Member | Type | Meaning |
|---|---|---|
| `path` | string | The outline as SVG path data (the `d` attribute of an SVG `<path>`: `M`, `L`, `H`, `V`, `C`, `S`, `Q`, `T`, `A`, `Z`, upper or lower case), font units, y up. |
| `rule` | `"nonzero"` or `"evenodd"` | Optional: which parts of overlapping subpaths are inside (`nonzero` when absent). |
| `stitch` | object | How it is stitched: the stitch settings of the [embroidery spec](vectorcraft-embroidery.md#stitch-settings-stitch) (`kind`, `spacing`, `underlay`…), in mm. |

### Kerning pairs

| Member | Type | Meaning |
|---|---|---|
| `left`, `right` | string | The two glyphs' `chars`, in reading order. |
| `value` | number | Font units added between them: negative sets them closer. |

## Limits

A reader refuses a file over 32 MB, more than 4000 glyphs, more than 256 objects in a glyph, more than
2,000,000 path elements in all, or more than 100,000 kerning pairs.

## Example

Two letters of [examples/sample.vcfont](examples/sample.vcfont), with the space:

```json
{
  "format": "vectorcraft-embroidery-font",
  "version": 1,
  "id": "two-letters",
  "name": "Two Letters",
  "license": "CC0-1.0",
  "unitsPerEm": 1000.0,
  "ascender": 750.0,
  "descender": -250.0,
  "capHeight": 700.0,
  "xHeight": 500.0,
  "sizes": [6.0, 40.0],
  "glyphs": [
    { "chars": " ", "advance": 300.0, "objects": [] },
    {
      "chars": "I",
      "advance": 250.0,
      "objects": [{ "path": "M75 0 L175 0 L175 700 L75 700 Z", "stitch": { "kind": "auto" } }]
    },
    {
      "chars": "L",
      "advance": 520.0,
      "objects": [
        { "path": "M75 0 L175 0 L175 700 L75 700 Z", "stitch": { "kind": "auto" } },
        { "path": "M175 0 L470 0 L470 100 L175 100 Z", "stitch": { "kind": "auto" } }
      ]
    }
  ],
  "kerning": [{ "left": "L", "right": "I", "value": -20.0 }]
}
```
