# Embroidery

VectorCraft can turn artwork into machine embroidery. Give paths **stitch settings**, check the result
with **View › Stitch Preview**, and export a **DST** file for the embroidery machine. This is early work
(milestones E1–E2): the stitches are correct, but stitch quality still lacks what dedicated digitizing
software adds (see [What's next](#whats-next)).

## Stitching objects

Select paths or compound paths (or groups holding them) and choose **Object › Embroidery**:

| Stitch | What it sews | Settings |
|---|---|---|
| Running Stitch | Single stitches along the path (outlines, details). Sharp corners keep a needle point. | `length` (2.5 mm) |
| Bean Stitch | Running stitch sewn three times over each stitch: a bolder line. | `length` (2.5 mm) |
| Satin Column | A zigzag centred on the path, as wide as the stroke (borders, lettering strokes). | `width` (the stroke width, else 3 mm), `spacing` (0.4 mm) |
| Fill | Rows of staggered running stitches covering the shape (tatami). Holes and separate parts follow the fill rule. | `angle` (0°), `spacing` (0.4 mm), `length` (4 mm) |
| Manual Stitch | The needle goes down at every anchor, as drawn. Imported designs use it. | — |

**Remove Stitches** takes the settings off. Settings are saved in the `.vectorcraft` file, and editing a
path changes its stitches.

The thread colour is the stroke's colour for path stitches and the fill's for fills, unless `thread` is set.
Hidden objects, guides and clipping paths are not stitched. Type, images and symbols can't be stitched
yet: outline the type first (Type › Create Outlines).

Objects are sewn in paint order, bottom first. Between objects the needle jumps, and it trims the thread
before a jump longer than 3 mm and before each colour change.

### Commands

| Command | Params |
|---|---|
| `embroidery.set` | `{type?: running\|bean\|satin\|fill\|manual, length?, spacing?, angle?, width?, thread?: "#rrggbb"\|[r,g,b], ids?}`. Lengths are in mm; `null` resets one to its default. |
| `embroidery.clear` | `{ids?}` |
| `embroidery.info` | → the design (`stitches`, `jumps`, `trims`, `colorChanges`, `threads`, `widthMm`, `heightMm`, `warnings`) and the selection's settings |
| `embroidery.stitches` | `{limit?}` → the stitches as sewn: `runs` of points in mm, `jumps`, `threads` |
| `view.stitchPreview` | toggles Stitch Preview |

Allowed ranges: stitch length 0.3–12 mm, spacing 0.1–5 mm, satin width 0.3–12 mm.

## DST files

`document.export {format: "dst"}` (File › Export As › Embroidery (DST)) writes the design, centred on its
middle, where the machine starts. Opening a `.dst` file creates one manual-stitch path for each run of
stitches, grouped by colour block. DST stores no thread colours, so each block gets a distinct colour;
recolour the paths' strokes, or set `thread`, before sewing.

The format is implemented from public descriptions of it
([edutechwiki: Embroidery format DST](https://edutechwiki.unige.ch/en/Embroidery_format_DST)): a
512-byte text header, then 3-byte records that each move the needle up to 12.1 mm. Longer moves are
split. A trim is written as three short jumps that return to where they started.

## What's next

- **E3, quality:** underlay under satins and fills, pull compensation, fill routing that walks the edge
  instead of jumping between parts, clean satin corners (today the zigzag bunches at sharp turns), satin
  between two rails, tie-in and tie-off stitches, and an
  Embroidery panel.
- **E4, breadth:** PES, JEF, EXP and VP3; hoop templates; auto-digitizing traced art.

Clean-room note: no code from Ink/Stitch (GPL) or other digitizers is used. Formats come from public
documentation, cited above.
