# Stitch style file (`.vcstyle`), version 1

A stitch style is a named set of stitch settings: the stitch type and any of its settings. Applying it to
objects gives them those settings in one step; they keep no link to the style afterwards. The file holds
one style, with who made it and under what licence, so it can be shared. Documents keep the styles they
use in [`styles`](vectorcraft-embroidery.md#stitch-styles-styles).

Machine-readable schema: [vcstyle.schema.json](vcstyle.schema.json). Example:
[examples/patch-border.vcstyle](examples/patch-border.vcstyle).

## File

- UTF-8 JSON, one object, at most **1 MB** (2²⁰ bytes).
- Extension `.vcstyle`. There is no registered media type; serve it as `application/json`.

```json
{
  "format": "vectorcraft-stitch-style",
  "version": 1,
  "units": "mm",
  "id": "patch-border",
  "name": "Patch Border",
  "stitch": {
    "kind": "satin",
    "width": 5,
    "spacing": 0.38,
    "pull": 0.25,
    "underlay": "auto"
  }
}
```

## Fields

| Field | Type | Meaning |
|---|---|---|
| `format` | string, `"vectorcraft-stitch-style"` | Names the format. |
| `version` | integer, 1 | The format version. A reader refuses a later version. |
| `units` | string, `"mm"` | All lengths are millimetres. |
| `id` | string | Lower case letters and digits joined by dashes. Made from `name` when absent or empty. |
| `name` | string | Shown to the user. `id` when empty. |
| `author`, `license`, `source`, `description` | string | Credits and notes: who made it, its licence (an SPDX identifier, or text), where it came from, what it is for. Information only. |
| `stitch` | object | The settings: a [stitch settings object](vectorcraft-embroidery.md#stitch-settings-stitch), as objects in documents have, with its `kind`. Required. |

A setting left out of `stitch` takes the stitch's default (or the document's fabric's, where the fabric has
one) on the objects the style is applied to, as on any object.

## Applying a style

A style replaces an object's stitch settings with its `stitch`, except what belongs to that one object,
which the object keeps:

- `startAt`, `angles` and `rails` (they are placed on the object's own shape);
- `trimBefore`, `trimAfter` and `stopBefore` (they are about where the object falls in the sewing order);
- `thread`, when the style has none (so a style without a thread keeps the object's colour).

A file may hold those members; a reader leaves them out of the style. A style naming a border `pattern`, a
`motif` or an embroidery `font` can be applied only in a document that has it (or where it is built in).

A reader refuses a file whose `stitch` it can't read (a `kind` or other value it doesn't know) or whose
settings are out of range or don't suit the stitch type (such as a fill with a satin `underlay`). Unknown
members of the file and of `stitch` are kept and written back.

## Starter styles

VectorCraft makes three styles itself (they aren't files):

| Id | Name | Settings |
|---|---|---|
| `small-lettering` | Small Lettering | `auto`, `spacing` 0.35, `pull` 0.15: type 5–10 mm tall |
| `bold-border` | Bold Border | `satin`, `width` 4, `pull` 0.2, `underlay` `auto` |
| `light-fill` | Light Background Fill | `fill`, `spacing` 0.8, `angle` 45, no underlay |

A document's or file's style with a starter's id is a different style; documents number it when loading
it (`bold-border-2`).
