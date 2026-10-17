# Embroidery

VectorCraft can turn artwork into machine embroidery. Give paths **stitch settings**, check the result
with **View › Stitch Preview**, and export a **DST** file for the embroidery machine. This is early work
(milestones E1–E2): the stitches are correct, but stitch quality still lacks what dedicated digitizing
software adds (see [What's next](#whats-next)).

## Stitching objects

Select paths or compound paths (or groups holding them) and choose **Object › Embroidery**:

| Stitch | What it sews | Settings |
|---|---|---|
| Auto | Picks per shape (per letter, for type) by how wide its strokes are: under **Bean Outline Under** (1 mm) a bean stitch round the outline, over **Fill Over** (7 mm) a tatami fill with fill underlay and a satin border, in between a satin shape with a centre-walk underlay. The stroke width is twice the shape's area over its perimeter. | `thinUnder` (1 mm), `wideOver` (7 mm), `spacing`, `pull`, `border` (for filled shapes; 2 mm) |
| Running Stitch | Single stitches along the path (outlines, details). Sharp corners keep a needle point. | `length` (2.5 mm) |
| Bean Stitch | Running stitch sewn several times over each stitch: a bolder line. | `length` (2.5 mm), `passes` (3, 5, 7 or 9; 3) |
| Satin Column | A zigzag centred on the path, as wide as the stroke (borders, lettering strokes). **Between Rails**: the object's two longest subpaths are the column's edges and the satin spans between them, as wide as they are apart (columns that swell and taper); other subpaths crossing both are rungs, each setting the stitch direction where it crosses. Draw the two edges, join them (Object › Compound Path) and tick Between Rails. | `width` (the stroke width, else 3 mm), `spacing` (density, 0.4 mm), `pull` (pull compensation, 0 mm), `underlay` (none, center, edge, zigzag), `rails` (off) |
| Zigzag Stitch | An open zigzag along the path, swinging side to side (decorative lines, appliqué edges). Corners fan like a satin's. | `width` (3 mm; not the stroke), `spacing` (between points, 2 mm) |
| Satin Shape | Satin that follows a closed shape's strokes from edge to edge, curving with them (lettering, outlined type, swashes). The shape's centre lines are found automatically; each zig reaches the shape's edges either side, so thick and thin strokes keep their contrast. | `spacing` (density, 0.4 mm), `pull` (0 mm), `underlay` (none, center) |
| Fill | Rows of staggered running stitches covering the shape (tatami). Holes and separate parts follow the fill rule. | `angle` (0°), `spacing` (density, 0.4 mm), `length` (4 mm), `stagger` (pattern: 2 brick, 3–8 tatami; 3), `motif` (a carved pattern: diamonds, waves, circles, scales, herringbone; none) and `motifSize` (its repeat, 3–60 mm; 10), `underlay` (none, fill), `border` (a satin border over the edges, 0.5–6 mm; none) |
| Pattern Border | A border pattern repeated end to end along the path, bending with its curves (decorative borders, frames). See [Pattern borders](#pattern-borders). | `pattern` (waves, scallops, chain, diamonds; waves), `width` (the band, 1–40 mm; the pattern's own, 6 mm), `flip` (off), `length` (1.5 mm), `spacing` (satin parts, 0.4 mm) |
| Manual Stitch | The needle goes down at every anchor, as drawn. Imported designs use it. | — |

**Window › Embroidery** shows the same settings for the selection: the stitch type, the thread, and the
selected type's parameters (Density is the spacing). Below them it shows the whole design (stitch count,
size, trims and jumps, threads) with a Stitch Preview switch and **Export DST…**.

**Underlay** is sewn before the top stitches to hold the fabric and lift the top stitches. For satin, it can be a
**center walk** (a running stitch down the middle), an **edge walk** (just inside both edges) or a sparse
**zigzag**. For fills, a **fill underlay** is a set of sparse rows at right angles to the top rows. **Pull
compensation** sews a satin column wider than drawn, because the thread pulls the fabric in.

The fill **pattern** sets how many rows the stitches take before their needle holes line up again: 2 makes a
brick pattern, 3 to 8 make tatami patterns with longer diagonals. The same menu has **carved motifs**
(Diamonds, Waves, Circles, Scales, Herringbone): the motif repeats across the fill, every Motif Size, and
where its lines cross a row the needle goes down on them (the row's own needle points near them are left
out), so the motif shows in the fill as lines of needle holes, an embossed pattern, while the fill stays
one even layer. The motifs are drawn in code. A motif line running nearly along the rows crosses few of
them, so it shows faintly (the tops of the circles at a 0° fill): turn the fill's angle to change which parts
show strongest.

### Pattern borders

A **Pattern Border** sews one repeat of a border pattern after another along the path, like a pattern brush.
The repeats bend with the path's curves, and their length is adjusted slightly so a whole number of them fits
each stretch between sharp corners (turns over 50°); a stretch shorter than half a repeat gets none. At a corner
the pattern's **corner motif** is sewn when it has one (Diamonds has a four-petal rosette), centred on the
corner, and the border runs through it without a break; a pattern without one ends at the corner and starts
again after it. A closed path with no corners repeats all the way round and meets itself. **Width** scales the
whole pattern (stitch length and satin density stay as set; stitches that would be longer than 12 mm are
split). **Flip** mirrors it across the path, which puts scallops on the other side. The starter patterns,
drawn in code, are Waves (a bean-stitch wave), Scallops (satin arches on one edge), Chain (linked bean-stitch
loops) and Diamonds (diamonds on a centre line, with a rosette at corners). On curves tighter than half the
band's width the inside of the pattern crowds together: widen the curve or narrow the band.

More patterns come from **border pattern files** (`.vcborder`): **Load…** under the Pattern menu (or File ›
Open, or dropping the file on the window) adds one to the document, and the selected pattern borders take it.
Loaded patterns are kept in the document, so it sews the same on any computer, and are listed in the Pattern
menu before the starters, with their author and licence. **Save…** writes the chosen pattern, a starter
included, as a file. A pattern with the same id as one already in the document but different content is
added under a numbered id (`rope-2`); the very same pattern isn't added twice.

#### The border pattern file format (`.vcborder`, version 1)

An open format: anyone may read and write it. A file is UTF-8 JSON (at most 8 MB) holding one pattern:

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
    { "stitch": "bean", "points": [[0, -2], [4, 2], [8, -2]] }
  ],
  "corner": [
    { "stitch": "running", "points": [[0, 0], [3, 0], [0, 3], [0, 0]] }
  ]
}
```

| Field | |
|---|---|
| `format` | Always `vectorcraft-border-pattern`. |
| `version` | `1`. A reader refuses versions newer than it knows; fields it doesn't know are ignored. |
| `units` | `mm` (the only unit; may be left out). |
| `id` | What documents call the pattern: lower-case letters, digits and dashes. Left out, it is made from `name`. |
| `name` | Shown in the Pattern menu. |
| `author`, `license`, `source`, `description` | Optional text. `license` is best an [SPDX id](https://spdx.org/licenses/). `source` says where the pattern came from: a URL, or the design it was taken from. Information only: they change nothing that is sewn. |
| `width`, `length` | The band's width and one repeat's length at the pattern's own size, mm (0.01–1000). |
| `repeat` | One repeat: a list of **parts** sewn in order as one continuous line. |
| `corner` | Optional: a motif sewn at each sharp corner, as parts. |

The pattern's space: **x** runs along the border from 0 to `length`; **y** runs across it, from −`width`/2 to
`width`/2, **positive to the right** of the direction of travel as seen on the page (on a path drawn
clockwise, the inside). The repeat enters at its first point and leaves at its last, which should be the first point moved
`length` along x, so repeats join end to end without a gap. Each part should start where the one before it
ends: the needle sews straight from one to the next.

A **part** is `{ "stitch": "running" | "bean" | "satin" | "manual", "points": [[x, y], …], "width"?: mm }`.
Running, bean and satin parts are lines: they bend with the path, and their stitches are made at the object's
stitch length (bean: each stitch sewn three times) and, for satin, as a column `width` wide (1 mm if left
out) at the object's density. **Manual** points are needle points, sewn as given (patterns taken from stitched
designs). Everything scales with the border's width.

The **corner** motif is drawn round the corner point at (0, 0), with x along the direction of travel through
the corner (halfway between the directions in and out) and y across it as in the repeat, and should start and
end at (0, 0). The repeats stop
short of the corner by as far as the motif reaches from it.

A reader checks a file before use: a file that isn't this format, sizes out of range, numbers that aren't
finite, an empty repeat or more than 100,000 points are refused with a message.

**Remove Stitches** takes the settings off. Settings are saved in the `.vectorcraft` file, and editing a
path changes its stitches.

The thread colour is the stroke's colour for path stitches and the fill's for fills, unless `thread` is set.
Hidden objects, guides, clipping paths, template layers and non-printing layers are not stitched. Images and
symbols can't be stitched.

### Type

Type objects are stitched as they are: no need to create outlines first, and editing the text, font or size
changes the stitches. Each letter is sewn on its own with the object's settings (then underlines and
strikethroughs), in the thread of its characters' colour, so multicoloured type changes thread. **Auto** suits
lettering: small type gets a bean outline, most lettering satin that follows each letter's strokes, and large
display type a fill with a satin border. Parts of a letter that have no strokes (the dot of an i, a full stop)
are filled.

Auto checks each letter's satin before using it. Satin suits a letter when its stitches can span the strokes
edge to edge without fanning round tight curves or crowding their needle points: heavy type round small
counters (Impact, Arial Black) fails that check, and so do strokes that merge into wide wedges (Impact's w).
A letter that fails is filled with a satin border instead. Auto decides for the whole type object, so its
letters look alike: when a quarter or more of them fail, they are all filled; otherwise only the ones that
fail are. Heavy display type therefore comes out as fill with satin borders, regular-weight type as satin. Clipping and
opacity masks don't apply to stitches: a masked object is stitched whole, with a warning.

A fill whose rows fall into separate parts (round a hole, between a letter's strokes) sews them nearest first
and travels between them instead of jumping: by a straight run of stitches where that stays inside the shape
(the rows sewn next cover it), else along the outline (up to 40 mm), stepping off and back on in straight
lines inside the shape. Only where neither works does the needle jump (and trim).

### Sew order

Objects are sewn in paint order, bottom first, until you change it: the Embroidery panel's **Sew Order** lists
the stitched objects as they are sewn (with their threads and stitches). Click one to select it; drag it up
or down the list, or use **Sew First**, **Sew Earlier**, **Sew Later** and **Sew Last** on the selection.
**By Thread** groups the order by thread colour (colours in the order they first come, each keeping its
objects' order), so each thread is sewn once; **Paint Order** goes back. The order is saved with the
document; objects added later are sewn after the ones in it, and deleted ones drop out. Whatever is sewn
later lies on top on the fabric, so Stitch Preview and the exported design follow the sewing order, not the
paint order. Commands: `embroidery.order`, `embroidery.reorder {to: first|earlier|later|last, ids?}` or
`{order: [ids]}`, `embroidery.sortByThread`, `embroidery.resetOrder`.

Objects are sewn one after another. Between objects the needle jumps, and it trims the thread
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
| `embroidery.set` | `{type?: auto\|running\|bean\|satin\|zigzag\|satinShape\|fill\|patternBorder\|manual, length?, spacing?, angle?, width?, thread?: "#rrggbb"\|[r,g,b], pull?, underlay?, stagger?, passes?, pattern?, flip?, trimBefore?, trimAfter?, ids?}`. Lengths are in mm; `null` resets one to its default. |
| `embroidery.clear` | `{ids?}` |
| `embroidery.borderPatterns` | → the patterns pattern borders can use (the document's, then the starters) |
| `embroidery.loadBorderPattern` | `{path? \| data?}`: load a `.vcborder` file into the document → `{id, name, added}` |
| `embroidery.saveBorderPattern` | `{pattern, path?}`: write a pattern as a `.vcborder` file (without `path`: `{data}`) |
| `embroidery.removeBorderPattern` | `{pattern}`: remove one of the document's patterns that nothing sews with |
| `embroidery.setup` | `{trimJumpsOver?: mm \| null, stitchMovesUnder?: mm}`: the document's travel settings |
| `embroidery.info` | → the design (`stitches`, `jumps`, `trims`, `colorChanges`, `threads`, `widthMm`, `heightMm`, `warnings`) and the selection's settings |
| `embroidery.stitches` | `{limit?}` → the stitches as sewn: `runs` of points in mm, `jumps`, `threads` |
| `view.stitchPreview` | toggles Stitch Preview |

Allowed ranges: stitch length 0.3–12 mm, spacing 0.1–10 mm, satin and zigzag width 0.3–12 mm, pattern border width 1–40 mm, pull compensation 0–3 mm,
stagger 2–8 rows, bean passes 3–9 (odd).

### How Satin Shape finds strokes

The shape is rasterized finely (up to 700 pixels a side, never coarser than needed), each pixel's distance to
the edge is measured, and the pixels are thinned to a one-pixel skeleton. The skeleton is traced into strokes
between their ends and junctions; short spurs that thinning leaves at corners are pruned, strokes are
smoothed, and stroke ends are extended to the shape's tips. Each zig of the satin crosses its stroke where it is narrowest (the
normal turned up to 35° either way, smoothed along the stroke), reaching the shape's edges at most twice
the stroke's half-width away; a zig with no edge that near on one side is inside a junction and is left
out. Where strokes meet, the two longest pass through and the others start at their edge. Where the stroke turns sharply (a w's points),
the satin fans round the corner: its outer side reaches out to the corner, its inner side holds at the notch,
and extra zigs go in wherever the needle points on either side would spread more than one and a half
spacings apart. On a tight curve, every other needle point on the crowded side stops halfway (short
stitches). Parts of the shape no stroke runs through (round dots) are filled. Known limits: serifs are covered only partly (they are too short to be strokes of their own), and
crossings (an &, an 8) are sewn twice where strokes overlap. Strokes wider than about 7 mm make long,
loose satin stitches: fill those shapes instead.

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

- **E3, quality: done.** Underlay, pull compensation, tie-ins and tie-offs, satin corners, fill routing, satin
  between rails, carved motif fills and the Embroidery panel.
- **E4, breadth:** PES, JEF, EXP and VP3; hoop templates; auto-digitizing traced art.
- **E5, future sprints:**
  - **Embroidery fonts:** lettering digitized for embroidery (satin columns laid out by hand per letter), for
    crisp small type and heavy display type where Auto falls back to fill today. Three parts:
    - **An open font format**, documented here and versioned, that anyone can read and write. Decided: a
      self-contained file, not data embedded in a regular font (modifying fonts breaks most licences, and
      font tools strip unknown tables) and not an extension that depends on an installed font (it breaks when
      that font changes, can't be shared without it, and copies its outlines). It may record the font it was
      digitized from, as information only. Each letter is
      stored as stitch objects (shapes with their stitch settings), not needle points, so letters scale and
      their stitches are generated at each size; with the font's metrics (baseline, cap height, x-height,
      advances), kerning pairs, recommended size range, and author and licence information.
    - **A font editor:** a document with one artboard per letter (a new font can start from an installed
      font's outlines on a template layer, as a tracing guide that isn't saved in the font), digitized with
      the ordinary tools and stitch types, metrics and kerning set on it, a live sample line to test words, saved straight to the format.
    - **Using them:** embroidery fonts appear in the type controls; type set in one is sewn from its letters.
      Like other fonts they live in [craft-fonts](https://github.com/storytold/craft-fonts), not in this
      repository, each with its licence.
  - **Motif extraction:** open a DST design, select a region of its stitches, and find the pattern that
    repeats there: the two offsets at which the stitches line up with a shifted copy of themselves, found by
    comparing the region with shifted versions of itself, then adjustable by hand. One tile is extracted as
    stitch objects, its edges trimmed to join seamlessly with its neighbours, and saved as a **pattern file**:
    an open, documented format (like the embroidery fonts: self-contained, with author and licence). Saved
    patterns appear in the Fill section's Pattern menu (the motif fills) and tile across any shape. Only
    designs the user has the right to reuse should be extracted from; the pattern records where it came from.
  - **Border patterns:** the **Pattern Border** stitch, its starter patterns and the
    [border pattern file](#the-border-pattern-file-format-vcborder-version-1) are done. Still to come:
    **extraction**: select a stretch of a DST border, find its one repeat offset, and extract a segment that
    joins end to end seamlessly, saved as a border pattern file.
  - **Embroidery for agents (MCP):** the commands already work over MCP (`run_command`, `export` with
    `dst`, `open_file`), but agents aren't told about them. Mention embroidery in the server's instructions;
    give the commands their Object › Embroidery menu path in the command list; an `embroidery` prompt that
    walks through digitizing a selection and exporting it; this guide served as an MCP resource
    (`vectorcraft://guide/embroidery`), including that embroidery lengths are in mm; an embroidery section in
    [mcp.md](mcp.md); and stitches drawn into `render_preview`, so agents can see the result.
  - **Appliqué from a shape:** one command turns a closed shape into the appliqué sequence, as separate
    objects in sewing order: a **placement line** (running stitch showing where the fabric goes, then a stop),
    a **tack-down** (running or zigzag holding the fabric, then a stop to trim it) and a **satin cover** over the
    raw edge.
    - **Cut files for the appliqué fabric:** export the pieces' placement outlines (optionally grown by an
      allowance so the fabric reaches under the tack-down) for a fabric cutter. The pieces are **nested**: packed
      into the smallest area without overlapping, a set gap apart (e.g. 3 mm), rotated or flipped where the fabric
      allows, within the cutter's mat (12 × 12 in, 12 × 24 in…), continuing on further mats when they don't fit.
      Formats: SVG and DXF (both written already), which Brother CanvasWorkspace, Cricut Design Space and
      Silhouette Studio read; Brother's own FCM for ScanNCut machines only from a public specification (the
      clean-room rule), else through CanvasWorkspace from SVG.

Clean-room note: no code from Ink/Stitch (GPL) or other digitizers is used. Formats come from public
documentation, cited above.
