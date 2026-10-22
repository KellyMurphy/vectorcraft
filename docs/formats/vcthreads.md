# Thread chart file (`.vcthreads`), version 1

A thread chart lists threads: each one's code (the maker's number), name and colour. Loaded into a
document (see [`charts`](vectorcraft-embroidery.md#thread-charts-charts-chart)), the chart chosen gives
each colour of the design its nearest thread, so the design can be sewn in threads one can buy.

VectorCraft ships no makers' charts: their numbers and colours are the makers' data, and none is
published under terms that allow redistributing it. Load charts you have the right to use (a maker's
chart you were given, or one you measured yourself), and share yours with their licence.

Machine-readable schema: [vcthreads.schema.json](vcthreads.schema.json). Example:
[examples/sample.vcthreads](examples/sample.vcthreads) (made-up threads, for testing).

## File

- UTF-8 JSON, one object, at most **4 MB** (4 × 2²⁰ bytes) and **20,000 threads**.
- Extension `.vcthreads`. There is no registered media type; serve it as `application/json`.

```json
{
  "format": "vectorcraft-thread-chart",
  "version": 1,
  "id": "sample",
  "name": "Sample Threads",
  "brand": "Sample Thread Co. (made up)",
  "weight": "40",
  "author": "VectorCraft contributors",
  "license": "CC0-1.0",
  "description": "A made-up chart for testing: not a real maker's threads.",
  "threads": [
    { "code": "S100", "name": "Sample Red", "color": "#c8102e" },
    { "code": "S200", "name": "Sample Green", "color": "#00843d" },
    { "code": "S300", "name": "Sample Blue", "color": "#0033a0" },
    { "code": "S400", "name": "Sample Black", "color": "#101010" },
    { "code": "S500", "name": "Sample White", "color": "#f4f4f0" }
  ]
}
```

## Fields

| Field | Type | Meaning |
|---|---|---|
| `format` | string | `"vectorcraft-thread-chart"`. |
| `version` | integer | 1. A reader refuses a later version. |
| `id` | string | Lower case letters and digits joined by dashes. Made from `name` when absent or empty. |
| `name` | string | Shown to the user. `id` when empty. |
| `brand`, `weight` | string | The thread maker and line, and the thread weight (information only). |
| `author`, `license`, `source`, `description` | string | Who made the chart, its licence (an SPDX identifier, or text), where it came from, notes. |
| `threads` | array | 1 to 20,000 threads. |

Each thread:

| Field | Type | Meaning |
|---|---|---|
| `code` | string, not empty | The maker's number for the thread. |
| `name` | string | Its name. |
| `color` | string `#rrggbb` | Its colour, sRGB. |

A reader refuses a file whose threads lack a code or a valid colour. Unknown fields are kept and written
back.

## Matching

A design colour's thread is the chart thread with the smallest CIEDE2000 colour difference to it, both
colours taken as sRGB and converted to CIE Lab (D65 white). Differences under about 2 are hard to tell
apart; over about 10 the thread is clearly another colour.
