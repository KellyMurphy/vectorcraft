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
| Bean Stitch | Running stitch sewn several times over each stitch: a bolder line. | `length` (2.5 mm), `passes` (3, 5, 7 or 9; 3) |
| Satin Column | A zigzag centred on the path, as wide as the stroke (borders, lettering strokes). | `width` (the stroke width, else 3 mm), `spacing` (density, 0.4 mm), `pull` (pull compensation, 0 mm), `underlay` (none, center, edge, zigzag) |
| Fill | Rows of staggered running stitches covering the shape (tatami). Holes and separate parts follow the fill rule. | `angle` (0°), `spacing` (density, 0.4 mm), `length` (4 mm), `stagger` (pattern: 2 brick, 3–8 tatami; 3), `underlay` (none, fill) |
| Manual Stitch | The needle goes down at every anchor, as drawn. Imported designs use it. | — |

**Window › Embroidery** shows the same settings for the selection: the stitch type, the thread, and the
selected type's parameters (Density is the spacing). Below them it shows the whole design (stitch count,
size, trims and jumps, threads) with a Stitch Preview switch and **Export DST…**.

**Underlay** is sewn before the top stitches to hold the fabric and lift the top stitches. For satin, it can be a
**center walk** (a running stitch down the middle), an **edge walk** (just inside both edges) or a sparse
**zigzag**. For fills, a **fill underlay** is a set of sparse rows at right angles to the top rows. **Pull
compensation** sews a satin column wider than drawn, because the thread pulls the fabric in.

The fill **pattern** sets how many rows the stitches take before their needle holes line up again: 2 makes a
brick pattern, 3 to 8 make tatami patterns with longer diagonals.

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

Allowed ranges: stitch length 0.3–12 mm, spacing 0.1–5 mm, satin width 0.3–12 mm, pull compensation 0–3 mm,
stagger 2–8 rows, bean passes 3–9 (odd).

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

- **E3, quality:** fill routing that walks the edge instead of jumping between parts, clean satin corners
  (today the zigzag bunches at sharp turns), satin between two rails, tie-in and tie-off stitches, and fill
  patterns beyond tatami (motif and embossed fills: they will join the Fill section's Pattern menu).
  Underlay, pull compensation and the Embroidery panel are done.
- **E4, breadth:** PES, JEF, EXP and VP3; hoop templates; auto-digitizing traced art.

Clean-room note: no code from Ink/Stitch (GPL) or other digitizers is used. Formats come from public
documentation, cited above.
