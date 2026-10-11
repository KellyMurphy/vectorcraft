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
Hidden objects, guides, clipping paths, template layers and non-printing layers are not stitched. Type,
images and symbols can't be stitched yet: outline the type first (Type › Create Outlines). Clipping and
opacity masks don't apply to stitches: a masked object is stitched whole, with a warning.

Objects are sewn in paint order, bottom first. Between objects the needle jumps, and it trims the thread
before a long jump and before each colour change. The **Travel** settings (the Embroidery panel, or
`embroidery.setup`) belong to the document:

- **Trim Jumps Over** (3 mm): a jump longer than this cuts the thread first. Turn it off to cut only at thread
  changes and at objects set to trim.
- **Sew Moves Under** (1 mm): a move to the next stitch up to this long is sewn as a stitch instead of jumped
  (never longer than the trim length).

Each object can also force a cut: **Trim Before** cuts the thread before it starts, however close it is to the
stitch before; **Trim After** cuts it after the object ends. A thread change cuts anyway, so it never cuts
twice. Every run of stitches that starts after a jump,
trim or colour change goes down where the frame stopped and locks the thread with two tiny stitches (a
tie-in); every trim is preceded by a tie-off. Manual stitches are sewn exactly as drawn, without ties.

At a sharp corner a satin column fans round the turn: the inner side holds at the corner's inner miter point
while the outer side sweeps round, so the stitches neither pile up inside the turn nor leave a gap outside it.
A closed satin path starts halfway along its longest side, away from its corners. No stitch is longer than
12 mm: wider satins (with pull compensation) are sewn at 12 mm.

### Commands

| Command | Params |
|---|---|
| `embroidery.set` | `{type?: running\|bean\|satin\|fill\|manual, length?, spacing?, angle?, width?, thread?: "#rrggbb"\|[r,g,b], pull?, underlay?, stagger?, passes?, trimBefore?, trimAfter?, ids?}`. Lengths are in mm; `null` resets one to its default. |
| `embroidery.clear` | `{ids?}` |
| `embroidery.setup` | `{trimJumpsOver?: mm \| null, stitchMovesUnder?: mm}`: the document's travel settings |
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

DST files VectorCraft writes have been checked against an independent reader,
[pyembroidery](https://github.com/EmbroidePy/pyembroidery) (used as a test tool only; none of its code is in
VectorCraft): the stitch, jump, trim and colour-change counts and the design size agree.

The format is implemented from public descriptions of it
([edutechwiki: Embroidery format DST](https://edutechwiki.unige.ch/en/Embroidery_format_DST)): a
512-byte text header, then 3-byte records that each move the needle up to 12.1 mm. Longer moves are
split. A trim is written as three short jumps that return to where they started.

## What's next

- **E3, quality:** fill routing that travels along the edge between parts instead of jumping, satin between
  two rails, and fill patterns beyond tatami (motif and embossed fills: they will join the Fill section's
  Pattern menu). Underlay, pull compensation, tie-ins and tie-offs, satin corners and the Embroidery panel
  are done.
- **E4, breadth:** PES, JEF, EXP and VP3; hoop templates; auto-digitizing traced art.

Clean-room note: no code from Ink/Stitch (GPL) or other digitizers is used. Formats come from public
documentation, cited above.
