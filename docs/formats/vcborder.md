# Border pattern file (`.vcborder`), version 1

A border pattern is one repeat of a stitched border. A **Pattern Border** stitch sews it end to end along a
path, bending it with the path's curves and fitting a whole number of repeats between sharp corners (see
[Pattern borders](../embroidery.md#pattern-borders)). The file holds one pattern, with who made it and under
what licence, so it can be shared.

Machine-readable schema: [vcborder.schema.json](vcborder.schema.json). Example:
[examples/rope.vcborder](examples/rope.vcborder).

## File

- UTF-8 JSON, one object, at most **8 MB** (8 × 2²⁰ bytes).
- Extension `.vcborder`. There is no registered media type; serve it as `application/json`.

```json
{
  "format": "vectorcraft-border-pattern",
  "version": 1,
  "units": "mm",
  "id": "rope",
  "name": "Rope",
  "author": "A. Digitizer",
  "license": "CC-BY-4.0",
  "source": "https://example.org/rope",
  "description": "A twisted rope, for frames.",
  "width": 6,
  "length": 8,
  "repeat": [
    { "stitch": "bean", "points": [[0, 0], [1, -2.5], [3, -2.5], [5, 2.5], [7, 2.5], [8, 0]] }
  ],
  "corner": [
    { "stitch": "bean", "points": [[0, 0], [3, -3], [6, 0], [3, 3], [0, 0]] }
  ]
}
```

## Fields

| Field | Type | Required | Meaning |
|---|---|---|---|
| `format` | string | yes | Always `"vectorcraft-border-pattern"`. |
| `version` | integer | yes | `1`. Readers refuse a version newer than they know. |
| `units` | string | no | `"mm"`, the only unit. Any other value is refused. |
| `id` | string | no | What documents call the pattern. See [Ids](#ids). |
| `name` | string | yes | The pattern's name, shown to users. |
| `author` | string | no | Who made it. |
| `license` | string | no | Its licence: an [SPDX identifier](https://spdx.org/licenses/) (`CC0-1.0`, `CC-BY-4.0`…) or text. |
| `source` | string | no | Where it came from: a URL, or the design it was taken from. |
| `description` | string | no | Free text. |
| `width` | number | yes | The band's width at the pattern's own size, mm. 0.01–1000. |
| `length` | number | yes | One repeat's length at the pattern's own size, mm. 0.01–1000. |
| `repeat` | array of [parts](#parts) | yes | One repeat. Must hold at least one point. |
| `corner` | array of [parts](#parts) | no | A motif sewn at each sharp corner. Absent or empty: none. |

`author`, `license`, `source` and `description` are information only: they change nothing that is sewn.
Programs should show the author and licence where users choose patterns, and keep all four when they copy
a pattern. Fields a reader doesn't know are kept with the pattern and written back when it is saved. A
file with a part `stitch` the reader doesn't know is refused with a message. A pattern like that inside a
document is kept as written but not used (see
[Embroidery data in documents](vectorcraft-embroidery.md#compatibility)).

### Ids

An id is lower-case letters (of any script) and digits, in runs joined by single dashes, at most about 64
bytes of UTF-8 (`rope`, `rope-twist-2`). A reader turns whatever the file gives into this form: it
lower-cases it, turns each run of other characters into one dash, trims dashes at the ends and stops
once 64 bytes are reached. When `id` is missing or
empty it is made the same way from `name`, and when nothing is left it is `pattern`. So `"Rope Twist 2"`
becomes `rope-twist-2`.

The starter patterns built into VectorCraft use the ids `waves`, `scallops`, `chain` and `diamonds`. A
program loading a pattern into a document that already has a different pattern under the same id, or
whose id is a starter's, gives it a free numbered id (`rope-2`, `rope-3`…). The very same pattern isn't
added twice.

## The pattern's space

Points are `[x, y]` in millimetres at the pattern's own size:

- **x** runs along the border, from 0 to `length`.
- **y** runs across it, from −`width`/2 to `width`/2, **positive to the right of the direction of travel**
  as seen on the page (y pointing down). On a path drawn clockwise, positive y is the inside.

Points may stray outside the band; the band's width is what the border's width setting scales.

When sewn, the pattern is scaled so `width` becomes the border's width (x and y alike). Each repeat is then
stretched or squeezed slightly along x so a whole number of repeats fits the stretch of path, and bent so
its x axis follows the path.

## Parts

A part is a line and how to sew it:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `stitch` | `"running"` \| `"bean"` \| `"satin"` \| `"manual"` | no (`"running"`) | How the line is sewn. |
| `points` | array of `[x, y]` | yes | The line, in the pattern's space. |
| `width` | number | no | Satin only: the column's width, mm at the pattern's size (1 mm when absent). 0.01–1000. |

- **running**: single stitches along the line, at the stitch length set on the object.
- **bean**: running stitch with each stitch sewn three times (forward, back, forward).
- **satin**: a zigzag column `width` wide centred on the line (as sewn: never narrower than 0.3 mm or
  wider than 12 mm), at the density set on the object.
- **manual**: the needle goes down at each point, as given. Use it for patterns taken from stitched
  designs, where the points are needle points.

Running, bean and satin lines bend with the path, because they are divided into short pieces before
they are placed along it. Manual points are placed one by one. Every stitch longer than 12 mm is split.

### Order and continuity

The parts of `repeat` are sewn in order as **one continuous line**. Each part should start where the one
before it ends, because the needle goes straight from one to the next. The repeat enters at the first
point of its first part and leaves at the last point of its last part. That last point should be the first
point moved `length` along x (same y), so one repeat leads into the next without a gap. The starter
patterns all do this.

Consecutive running parts that meet, and likewise consecutive bean parts, are sewn as one line, so there
is no extra needle point where they join.

## Corners

A path's **sharp corners** are where it turns by more than 50°. Gentler turns are curves the pattern bends
round. Each stretch between corners (or between a corner and an end) gets a whole number of repeats; a
stretch shorter than half a repeat gets none.

When `corner` has parts, a motif is sewn at every corner:

- It is drawn round the corner point at (0, 0), scaled like the repeat.
- Its x axis points along the direction of travel through the corner (halfway between the directions into
  and out of the corner), and its y axis runs across it as in the repeat.
- It should start and end at (0, 0).
- The repeats stop short of the corner by as far as the motif reaches from (0, 0). The needle runs along
  the path from the last repeat to the corner, sews the motif, and runs on to the next stretch: the border
  is one continuous run.

Without corner parts, the border ends at each corner and starts again after it (the thread is tied off and
in again). A closed path with no sharp corners is sewn all the way round and meets itself.

## Limits

A reader refuses a file, with a message, when:

- it is over 8 MB, isn't JSON, or isn't an object of this format;
- `format` isn't `"vectorcraft-border-pattern"`, `version` is newer than it knows, or `units` isn't `"mm"`;
- `width`, `length` or a part's `width` is outside 0.01–1000 mm, or isn't a number;
- a point isn't a pair of finite numbers within ±1,000,000;
- `repeat` holds no points;
- the parts of `repeat` and `corner` hold more than 100,000 points in all.

A program sewing a pattern also refuses to sew it along a path where its repeats would hold more than
2,000,000 of the pattern's points in all (a tiny pattern on a very long path).

## Versions

**1** (VectorCraft, 2026-10): the first version.
