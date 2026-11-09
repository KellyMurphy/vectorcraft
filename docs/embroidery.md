# Embroidery

VectorCraft can turn artwork into machine embroidery. Give paths **stitch settings**, check the result
with **View › Stitch Preview**, and export a file for the embroidery machine: **DST**, **PES**, **JEF**, **EXP**, **VP3** or **U01**. This is early work
(milestones E1–E2): the stitches are correct, but stitch quality still lacks what dedicated digitizing
software adds (see [What's next](#whats-next)).

## Stitching objects

Select paths or compound paths (or groups holding them) and choose **Object › Embroidery**:

| Stitch | What it sews | Settings |
|---|---|---|
| Auto | Picks per shape (per letter, for type) by how wide its strokes are: under **Bean Outline Under** (1 mm) a bean stitch round the outline (a letter: narrow satin columns, see [Type](#type)), over **Fill Over** (7 mm) a tatami fill with auto underlay and a satin border fitted to the shape, in between a satin shape with auto underlay. The stroke width is twice the shape's area over its perimeter. | `thinUnder` (1 mm), `wideOver` (7 mm), `spacing`, `pull`, `pullPercent`, `push`, `border` (for filled shapes; 2 mm), `puff` (3D foam, for the letters sewn as satin), `angles` (stitch angle lines, for the shapes sewn as satin; none) |
| Running Stitch | Single stitches along the path (outlines, details). Sharp corners keep a needle point. **Style** sews it as a machine version of a hand stitch instead: **Chain** (linked loops), **Stem** (slanted, overlapping stitches: a twisted cord), **Backstitch** (each stitch forward, then half a stitch back: every stretch sewn three times) or **Sashiko** (dashes sewn three times, a single stitch across each gap). Each style fits whole repeats between sharp corners and starts again at each. | `length` (2.5 mm; a chain loop, a stem stitch, a sashiko dash), `style` (single, chain, stem, backstitch, sashiko; single), `width` (chain and stem: the loops or the slant, 0.2–6 mm; chain 1.5, stem 0.4), `flip` (stem: slanted the other way; off), `gap` (sashiko: between dashes, 0.5–6 mm; 1.5), `startAt` (where the needle starts; none: nearest where it is) |
| Bean Stitch | Running stitch sewn several times over each stitch: a bolder line. | `length` (2.5 mm), `passes` (3, 5, 7 or 9; 3), `startAt` |
| Satin Column | A zigzag centred on the path, as wide as the stroke (borders, lettering strokes). **Between Rails**: the object's two longest subpaths are the column's edges and the satin spans between them, as wide as they are apart (columns that swell and taper); other subpaths crossing both are rungs, each setting the stitch direction where it crosses. Draw the two edges, join them (Object › Compound Path) and tick Between Rails. | `width` (the stroke width, else 3 mm; up to 30 mm split, 12 mm not), `spacing` (density, 0.4 mm), `pull` (pull compensation, 0 mm) and `pullPercent` (% of the width, 0), `push` (push compensation: shorter at the ends, 0 mm), `underlay` (auto, none, center, edge, zigzag), `rails` (off), `split` (auto: stitches longer than `splitAt`, 7 mm, are sewn in pieces at most `splitLength`, 4 mm, long, moved along from stitch to stitch by `splitPattern`, stagger or random; off), `angles` (stitch angle lines; none), `puff` (3D foam thickness; none) |
| Zigzag Stitch | An open zigzag along the path, swinging side to side (decorative lines, appliqué edges). Corners fan like a satin's. | `width` (3 mm; not the stroke), `spacing` (between points, 2 mm), `split` (auto: stitches longer than `splitAt`, 7 mm, are sewn in pieces at most `splitLength`, 4 mm, long, moved along from stitch to stitch by `splitPattern`, stagger or random; off), `startAt` |
| Satin Shape | Satin that follows a closed shape's strokes from edge to edge, curving with them (lettering, outlined type, swashes). The shape's centre lines are found automatically; each zig reaches the shape's edges either side, so thick and thin strokes keep their contrast. Patches the satin can't reach (a heavy stroke's corners and junctions) are filled with short rows under it, so no fabric is left bare; a shape satin can't cover at all (more than 15% bare, or crowded or fanned) is sewn as a fill with a satin border instead, as Auto does, with a warning (type: every letter when a quarter or more need it, so they look alike). | `spacing` (density, 0.4 mm), `pull` (0 mm), `pullPercent` (% of the stroke width, 0), `underlay` (auto, none, center, edge), `split` (auto, strokes up to 12 mm: stitches longer than `splitAt`, 7 mm, are sewn in pieces at most `splitLength`, 4 mm, long, moved along from stitch to stitch by `splitPattern`, stagger or random; off), `angles` (stitch angle lines; none), `puff` (3D foam thickness; none) |
| Fill | Rows of staggered running stitches covering the shape (tatami). Holes and separate parts follow the fill rule. | `angle` (0°), `spacing` (density, 0.4 mm), `length` (4 mm), `stagger` (pattern: 2 brick, 3–8 tatami; 3), `motif` (a carved pattern: diamonds, waves, circles, scales, herringbone; none) and `motifSize` (its repeat, 3–60 mm; 10), `underlay` (auto, none, fill), `pull` and `pullPercent` (rows longer, half at each end; 0), `push` (shorter across the rows; 0 mm), `border` (a satin border over the edges, 0.5–6 mm; none), `keepCovered` (keep the rows under what is sewn on top; off) |
| Motif Fill | A small stitched motif sewn tile by tile across the shape on its lattice, every whole tile inside the shape, the fabric showing between: a light, decorative fill. | `motif` (a stitched motif: stars, flowers, crosses, hearts, dots, or one loaded from a `.vcmotif` file; stars), `motifSize` (one tile, 3–60 mm; 10), `angle` (the lattice turned; 0°), `length` (the motif's running and bean stitches; 1.5 mm), `spacing` (its satin parts' density; 0.4 mm) |
| Pattern Border | A border pattern repeated end to end along the path, bending with its curves (decorative borders, frames). See [Pattern borders](#pattern-borders). | `pattern` (waves, scallops, chain, diamonds; waves), `width` (the band, 1–40 mm; the pattern's own, 6 mm), `flip` (off), `length` (1.5 mm), `spacing` (satin parts, 0.4 mm) |
| Manual Stitch | The needle goes down at every anchor, as drawn. Imported designs use it. | — |

**Window › Embroidery** has three tabs. **Stitch** shows the same for the selection: the stitch, chosen from a grid of stitched
pictures (each drawn by the real generator, with a line saying what it does), the thread and trims, and the
stitch's own settings (Density is the spacing), the rarely changed ones folded under **More Settings**. Fill
patterns and border patterns are picked from pictures too, so Tatami 4 and Tatami 7 can be told apart before
sewing; when the checker finds problems, a line at the foot of the tab says how many and opens Design.
**Design** shows the whole design: the fabric, hoop, machine format (DST, PES, JEF, EXP, VP3 or U01) and thread
chart, its stitch count, size, trims and jumps and threads, a Stitch Preview switch, **Export** in the
machine's format, the worksheet, the Sew Order and Sew-Out Test panels, and the design checker's findings.
**Tools** holds Appliqué and its Cut File, Extract Border Pattern and Extract Motif (for an opened design's stitches),
embroidery fonts (Load, Import Ink/Stitch Font, Add Font Folder) and the library folder. The panel opens
on the tab it was last left on.

**Underlay** is sewn before the top stitches to hold the fabric and lift the top stitches. For satin, it can be a
**center walk** (a running stitch down the middle), an **edge walk** (just inside both edges) or a sparse
**zigzag**. For fills, a **fill underlay** is a set of sparse rows at right angles to the top rows. A satin
shape takes a centre walk along its strokes or an edge walk round its outline. **Auto**, what newly
stitched objects get, picks by stitch and width: satin under 2 mm a centre walk, 2–5 mm an edge walk,
wider an edge walk then a zigzag; fills an edge walk then fill underlay; satin shapes an edge walk when
their strokes are over 2.5 mm wide, else a centre walk. **Pull compensation** sews a satin column wider
than drawn, and a fill's rows longer, because the thread pulls the fabric in along the stitches; it is
given in mm, as a percentage of the width (or row length), or both. **Push compensation** sews a satin
column shorter at its ends, and a fill shorter across its rows, because the stitches push the fabric out
at their ends.

The fill **pattern** sets how many rows the stitches take before their needle holes line up again: 2 makes a
brick pattern, 3 to 8 make tatami patterns with longer diagonals. The same menu has **carved motifs**
(Diamonds, Waves, Circles, Scales, Herringbone): the motif repeats across the fill, every Motif Size, and
where its lines cross a row the needle goes down on them (the row's own needle points near them are left
out), so the motif shows in the fill as lines of needle holes, an embossed pattern, while the fill stays
one even layer. The built-in motifs are drawn in code; more come as **motif files** (`.vcmotif`, an open
format: [docs/formats/vcmotif.md](formats/vcmotif.md)): `embroidery.loadMotif` loads one into the document
(kept in it, so it sews the same anywhere), and a carve motif then carves fills as the built-in ones do
(`embroidery.set {motif}`). A motif file holds one tile and its lattice (two offsets), so its tiles can sit
on a square, brick or hexagonal grid. A motif line running nearly along the rows crosses few of
them, so it shows faintly (the tops of the circles at a 0° fill): turn the fill's angle to change which parts
show strongest.

### Auto-digitizing a logo

Select a logo and choose Object › Embroidery › **Auto-Digitize** (or **Auto-Digitize** in the Embroidery
panel's Tools tab). An image, or an image already traced with Image Trace, is traced as a flat-colour logo
(Image Trace's logo mode: at most ten colours, edges kept sharp, areas that meet without overlapping);
white is taken for the fabric and left out unless `keepWhite` is on. A group of filled shapes is used as
it is. Then:

- areas smaller than 2 mm² (`minArea`) are merged into the area round them, or left unstitched when none
  holds them;
- every area is set to **Auto**, which picks a bean outline for thin areas, a satin shape for narrow ones
  and a fill with a satin border for wide ones as it sews, so later edits are sewn the right way too;
- with **Thin Lines as Centre Lines** on (`lines: centreline`), areas narrower than 2 mm (traced lines,
  handwriting) are sewn instead as one line along their centre: running stitch when under 1 mm wide,
  else a satin as wide as the area. The area becomes an open path along its centre line;
- each area gets the thread of its colour, the thread chart's nearest when the document has one;
- for traced art, the design's sewing order is grouped by thread and sewn nearest first (`order: keep`
  leaves it); traced areas don't overlap, so this never changes what lies on top;
- the density adviser's fixes are made where spots come out too dense (`fix: false` skips them).

It is all one undo step, and the result is ordinary stitch objects in a group: change any of them by
hand. Photos and gradients don't digitize well: trace a flat-colour version.

**The Stitch Fill tool** (in the toolbar with the Stitch Angle tool, or **Stitch Fill Tool** in the Tools
tab) digitizes one area at a time: click inside any closed area, whether a shape or a region where
shapes and paths cross (as Live Paint finds them), and it becomes a new stitch object on top, sewn Auto
in the colour of the shape it lies in. Alt-click adds a running-stitch outline of the area instead, and
Alt-Shift-click a 2 mm satin border along its edge (`outline: satin`, `width` sets it). Shift-click sews
a thin area, such as a traced line or handwriting, as one line along its centre (`centreline`): running
stitch when it is under 1 mm wide, else a satin as wide as the area; the new object is an open path you
can reshape. Each click is one undo step; the art under it is left as it is.

### Stitch angles

A satin's stitches normally run square across it. **Stitch angle lines** set their direction instead:
pick the **Stitch Angle** tool (in the toolbar after the Dimension tool, or **Draw** on the Stitch tab's
Stitch Angles row), select a satin column, satin shape or Auto object, and drag a line across it.
Where a line crosses the satin its stitches take the line's angle; with several lines the angle turns
gradually from one to the next, by distance along the satin, and before the first line and after the last
the nearest one's angle holds. A line sets the angle relative to the satin's path, so on a curving column
the stitches keep the same slant to it as it curves. Shift draws in 45° steps; drag a line's end to move
it; Alt-click or double-click a line to remove it; **Clear** on the Stitch tab removes them all. A satin
shape's strokes each follow the lines that cross them (a stroke no line crosses keeps its own direction),
and between rails the lines are rungs. Stitches turn at most 60° from square, so none runs nearly along the
satin. The lines belong to the object: they move, scale and rotate with it (so does Start At), and are
`embroidery.set {angles: [[x1, y1, x2, y2], …]}` in mm in document space.

### Motif fills

**Motif Fill** fills a shape with a small stitched motif repeated tile by tile on its lattice (square,
brick or hexagonal, as the motif sets), the fabric showing between: a light, decorative fill for
backgrounds and lettering. Pick the motif in the Stitch tab (pictured): the built-in **Stars**,
**Flowers**, **Crosses**, **Hearts** and **Dots**, or a stitched motif loaded from a `.vcmotif` file
(`embroidery.loadMotif`, or the library's `motifs` folder). **Motif Size** sets one tile, **Angle** turns
the lattice. Only whole tiles inside the shape are sewn, so the edge stays clean; tiles are sewn row by row,
back and forth, each its own run, so the needle jumps (trimming as the document's travel settings say)
rather than leaving travel stitches across the open fabric. A shape too small for one whole tile warns:
make the motif smaller.

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

**Extracting a border pattern** from a stitched design: open the design (a DST opens as manual stitches),
select its border stitches and, in the Embroidery panel, choose **Extract Border Pattern**. To take only part
of the border, draw a shape over a straight stretch of it and select the shape too: only the stitches inside
it are used (the longest unbroken run of them). The stretch needs two or more repeats. VectorCraft finds the
distance at which the stitched line lies on itself shifted (the repeat's length, from 1 to 200 mm; the
`period` param sets it instead), and when every repeat has the same number of stitches, it averages all the
repeats into one, which cancels the file's 0.1 mm rounding. The repeat is cut where the stitches cross the
middle of the band, so it joins end to end, and added to the document's patterns as manual stitches, with
"extracted from" the design's name as its source. Save it as a file from the Pattern Border section. Only
take patterns from designs you have the right to reuse. Curved stretches aren't recognized: pick a straight
one.

**Extracting a motif** works the same way for a fill that repeats both ways (a background of stars, a
brick of crosses): select the opened design's stitches, optionally with a shape drawn over the area to take,
and choose **Extract Motif**. The area needs two or more repeats each way. VectorCraft finds the two
shortest shifts that move the needle points back onto needle points (the lattice; the `offsets` param sets
it instead) and cuts out one tile, the stitches whose middles fall in one cell of it, so the tiles meet
without gaps or doubled stitches. The tile is added to the document's motifs as a stitched motif, with
"extracted from" the design's name as its source: pick it for a Motif Fill, or save it as a `.vcmotif` file.
A pattern repeating only one way is a border: use Extract Border Pattern.

The file format is open and documented, so other programs can make patterns too: see the
[`.vcborder` specification](formats/vcborder.md). In short, a file is JSON holding the pattern's name,
author, licence and source, its width and repeat length in mm, one repeat as a list of running, bean,
satin and manual lines, and an optional corner motif.

**Remove Stitches** takes the settings off. Settings are saved in the `.vectorcraft` file (specified in
[Embroidery data in documents](formats/vectorcraft-embroidery.md)), and editing a path changes its
stitches.

The thread colour is the stroke's colour for path stitches and the fill's for fills, unless `thread` is set.
Hidden objects, guides, clipping paths, template layers and non-printing layers are not stitched. Images and
symbols can't be stitched.

### Type

**Letters are sewn the way they are written.** Satin Shape (and Auto, for the letters it sews as satin)
puts each letter's centre lines back together into the strokes a pen makes, by common rules of penmanship:

- Through a junction the pieces that carry straight on are one stroke: a `t`'s crossbar is one stroke
  across its stem, an `H`'s two stems are each one stroke.
- Print letters break at sharp corners: an `L` is a downstroke and a stroke to the right, an `E` its spine
  and three arms. Script faces (named Script, Brush, Hand, Cursive, Calligraphy…) never break at a corner:
  the pen keeps going, following the letter's flow.
- Downstrokes go first, left to right, each pulled from the top; then slanted strokes and curves; then
  crossbars and arms, top to bottom, each drawn left to right. Loops start at their top. Script is sewn
  left to right, as written.
- A stroke keeps its own width through a junction instead of swelling into it, and a stroke that stops at a
  junction or corner is carried on to the letter's edge, as a pen draws a crossbar right across a stem.
  Where a later stroke crosses an earlier one, only a 0.4 mm tuck under the earlier stroke's edge is sewn,
  so crossings aren't sewn twice.

Where pen strokes don't suit a letter (they would leave more than 5% bare), it is sewn piece by piece of its
centre line, or filled when satin can't cover it.

Type on a path, letter spacing (tracking), kerning and baseline shift are sewn as they are laid out: the
letters' stitches follow the path and move with the spacing (checked by the tests).

Type objects are stitched as they are: no need to create outlines first, and editing the text, font or size
changes the stitches. Each letter is sewn on its own with the object's settings (then underlines and
strikethroughs), in the thread of its characters' colour, so multicoloured type changes thread. **Auto** suits
lettering: most lettering gets satin that follows each letter's strokes, and large display type a fill with
a satin border. Small type, whose strokes are thinner than satin's 1 mm threshold, is sewn as **narrow satin
columns** at least 0.8 mm wide (thin strokes are widened evenly), not outlined: an outline of long stitches
round a 5 mm letter loses its shape. Parts of a letter that have no strokes (the dot of an i, a full stop) are
filled.

**Keeping letters open.** A filled letter's satin border sits on its edge, half inside and half out, so a
wide border closes a letter's holes (the inside of an o, a or e) and swells the letter. Auto fits the border
to each letter instead of the usual 2 mm: at most 45% of its narrowest stroke and of its narrowest hole, and
none when that leaves under 0.6 mm (a border you set yourself is kept as set). With that, holes 1 mm across
or more stay open in Arial and Arial Black from 5.5 mm and in Times from 10 mm (checked by the tests, which
look for stitches through each hole's middle). Holes narrower than 1 mm close on fabric whatever the stitch,
so the checker warns about them (below).

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

**Distorted art is sewn as it is drawn.** An object inside an envelope (Object › Envelope Distort: a warp,
a mesh or a top object, nested ones too) and one with a Warp effect (Effect › Warp) is sewn from its
distorted outlines, type letter by letter, so curved and bulged lettering sews as the canvas shows it. The
envelope's Fidelity sets how finely curves are followed; with its Distort Appearance option on, the object's
Warp effects apply before the envelope, else after it, as on the canvas. Type in an embroidery font bends
the same way: its letters' stitch objects are distorted as shapes before they are sewn. Other geometry
effects (Roughen, Zig Zag, Offset Path…) aren't sewn yet: the stitches follow the shape without them, with
a warning.

**Type on an arc.** **Type on an Arc** (Tools tab › Lettering, `embroidery.textArc`) puts the selected
point type on a circle arc through where it is (or sets new words with `text`), for badge lettering in one
step: **Top** reads left to right over the top with the letters standing outward, **Bottom** reads left
to right under the bottom, upright. The radius is the lettering band's inner edge, so a top and a bottom
line of one size share the band. The result is ordinary type on a path, edited as any other.

**Monograms.** **Monogram** (Tools tab › Lettering, `embroidery.monogram`) sets two or three letters as a
group of live point type, one object per letter, each placed and sized to fill its box in a layout:
**Classic** (a large centre initial between two smaller), **Three Equal**, **Two Letters**, **Stacked**,
**Circle** and **Diamond** (the outer letters shorter, to sit in that frame). The frame is optional: a 2 mm
satin ring, or a border pattern round the letters (a circle, a diamond for the Diamond layout). The
group keeps its recipe (letters, layout, height, frame) in its object data, so selecting a monogram shows
it in the panel and **Update Monogram** lays it out again in place with what changed. The checker's
small-type warning covers a monogram's letters like any type.

### Embroidery fonts

An **embroidery font** is lettering digitized for embroidery: each letter is a set of stitch objects (outlines
with their stitch settings) made by hand for sewing, so small type stays crisp and heavy type keeps its
satin, where an ordinary font sewn with Auto has to guess. Fonts are open `.vcfont` files
([specification](formats/vcfont.md)): load one into a document with `embroidery.loadFont` (from a file, or
from the library's `fonts` folder), list them with `embroidery.fonts`, save one with `embroidery.saveFont`.
The document keeps a copy of each font it uses, so it sews the same where the font isn't installed.
VectorCraft ships no embroidery fonts; each font has its own licence (`embroidery.fonts` lists it).

To sew type in one, select the type and pick it from **Embroidery Font** in the Embroidery panel's Thread
section (fonts in the library are loaded into the document when picked), or `embroidery.set {font: id}`.
Each letter is then sewn from the font's own stitch objects, with their own stitch settings, in the type's
colour. Point type is laid out from the embroidery font alone, so it needs no outline font: the font's
advances and kerning pairs, with the type's size, tracking, manual kerning, baseline shift, scaling, line
breaks, leading and alignment, and ligatures the font has. Area type and type on a path keep their lines
and places from their layout (wrapping in the frame, following the path), each straight line respaced by
the embroidery font and kept at its alignment. A character the font lacks is sewn from the outline font
where its letter would be, with a warning; type set outside the font's size range is warned about. For
now the canvas still draws the type in its outline font: Stitch Preview shows the embroidery letters.

**Making a font.** **New Embroidery Font** (Tools tab, `embroidery.font.new`) opens a font document: one
artboard per glyph on a shared baseline (an em is 100 mm; an artboard's width is the glyph's advance, its
name the glyph's characters, `space` for the space), ruler guides at the ascender, cap height, x-height,
baseline and descender, and a **Letters** layer to digitize each glyph on with the ordinary tools and stitch
types (Satin Column between rails is the lettering stitch). Pick an installed font as **Trace** (from the font menu) and its
letters go on a locked **Template** layer to trace over; the template is never part of the font. Widen or
narrow an artboard to change its glyph's advance; set the name, author, licence, metrics and kerning in the
Tools tab's Font section (`embroidery.font.setup`, `embroidery.font.kern`); **Update Sample** sets a line of
type below the artboards sewn from the font as it is now. **Save Font…** writes the `.vcfont`
(`embroidery.font.save`); **Edit Font…** opens one again (`embroidery.font.open`).

**Tracing a font.** **Trace Font** (`embroidery.font.trace`) digitizes the **Trace** font automatically
into a new font document: each letter is put back together into pen strokes, as Satin Shape sews type
(downstrokes first, then crossbars and arms tucked under them; script fonts follow the letter's flow), and
each stroke becomes a satin column between two rails, which scale with the letter. A letter pen strokes
don't suit is kept as its outline, sewn Auto. The metrics come from the font's own H, x, d and p, the
advances from its letters, and the original stays on the template layer to compare. Check and touch up the
letters, then save. Tracing takes a while (a few seconds for a full alphabet): a progress window shows how
many letters are done, with **Cancel** (or Escape), and the pointer shows the busy cursor; letters already
traced are kept, so tracing the same font again is quick. A traced font is a derivative of the font traced: trace only fonts whose licence allows
that (the SIL Open Font License does, under a new name without its Reserved Font Names, with the OFL kept).

**Ink/Stitch fonts** open as embroidery fonts too: install the ones you want by pointing VectorCraft at a
folder of them (**Add Font Folder…** under Embroidery Font) or by putting them in the library's `fonts`
folder, as [Installing embroidery fonts](embroidery-fonts.md) explains; `embroidery.importInkstitchFont`
converts one to a `.vcfont`. How they are read: [formats/inkstitch-fonts.md](formats/inkstitch-fonts.md).

### Sew order

Objects are sewn in paint order, bottom first, until you change it. **Window › Sew Order** is the panel for
it: a preview of the fabric with the design sewn up to any step (sewn objects in their threads, the current
one bright, the rest still faint), a scrub bar and step buttons to go through the sewing, and the list of
stitched objects as they are sewn, each with a picture of its stitches, its thread and stitch. Click one to
select it (the preview steps to it); drag it up or down the list, or use **Sew First**, **Sew Earlier**,
**Sew Later** and **Sew Last** on the selection.
**By Thread** groups the order by thread colour (colours in the order they first come, each keeping its
objects' order), so each thread is sewn once; **Paint Order** goes back. The order is saved with the
document; objects added later are sewn after the ones in it, and deleted ones drop out. Whatever is sewn
later lies on top on the fabric, so Stitch Preview and the exported design follow the sewing order, not the
paint order. Commands: `embroidery.order`, `embroidery.reorder {to: first|earlier|later|last, ids?}` or
`{order: [ids]}`, `embroidery.sortByThread`, `embroidery.resetOrder`.

Objects are sewn one after another. Between objects the needle jumps, and it trims the thread
before a long jump and before each colour change. The **Travel** settings (the Sew Order panel, or
`embroidery.setup`) belong to the document:

- **Trim Jumps Over** (3 mm): a jump longer than this cuts the thread first. Turn it off to cut only at thread
  changes and at objects set to trim.
- **Sew Moves Under** (1 mm): a move to the next stitch up to this long is sewn as a stitch instead of jumped
  (never longer than the trim length).

Each object can also force a cut: **Trim Before** cuts the thread before it starts, however close it is to the
stitch before; **Trim After** cuts it after the object ends. A thread change cuts anyway, so it never cuts
twice. **Stop Before** (`stopBefore`: `stop`, `placeFabric` or `trimFabric`) stops the machine before the
object for something done by hand: a plain stop, placing appliqué fabric, or trimming it. It is written as
a colour change to the same thread (every machine format has one), or is the thread change itself when the
thread changes there; a stop before the first object is done before the machine starts. The worksheet's
thread sequence says what to do at each stop, and `embroidery.info` lists them as the design's `stops`. Every run of stitches that starts after a jump,
trim or colour change goes down where the frame stopped and locks the thread with two tiny stitches (a
tie-in); every trim is preceded by a tie-off. Manual stitches are sewn exactly as drawn, without ties.

At a sharp corner a satin column fans round the turn: the inner side holds at the corner's inner miter point
while the outer side sweeps round, so the stitches neither pile up inside the turn nor leave a gap outside it.
A closed satin path starts halfway along its longest side, away from its corners. No stitch is longer than
12 mm: wider satins (with pull compensation) are sewn at 12 mm.

### Checking the design

The Embroidery panel's **Check** list shows what the design checker finds, errors first, then in sewing
order; click one to select its object, and Stitch Preview marks where each one is (red for errors, amber
for warnings). The rules so far:

- **Long stitches:** stitches over 7 mm, which snag and loosen with wear (shorten the stitch length,
  narrow the satin, or fill a shape that wide).
- **Makes no stitches:** a stitched object that is empty or too small for its stitch type.
- **Art the stitch can't follow:** what the stitch type said about the shape (a satin shape with no
  strokes, a motif too fine, a missing border pattern).
- **Not sewn as set:** objects that can't be stitched or whose settings are from a newer VectorCraft, and
  objects sewn whole under a mask.
- **Design cut short:** more needle moves than a file holds.
- **Stitches too long to sew:** stitches over 12 mm, which no machine sews in one move (an error).
- **Satin too narrow or too wide:** satin under 1 mm doesn't cover; over 10 mm it is long and loose.
- **Too small to sew:** objects whose stitches fit in a 1 mm box.
- **Open shape set to fill:** a fill or satin shape on an open path, which is closed with a straight line.
- **Too many needle holes in one place:** more than 40 needle holes in a 2 × 2 mm spot, where objects
  stack up (a single fill reaches about 25).
- **Many trims or jumps:** more than 40 trims or 150 jumps in the design.
- **Type too small:** lettering under 4 mm (satin, Auto), 6 mm (fill) or 2.5 mm (running, bean); and
  lettering (other than running or bean) whose letters' holes are under 1 mm across, which close up on
  fabric. That depends on the font: Impact's holes are under 1 mm even at 15 mm. The finding gives the
  size at which they open.
- **Design bigger than the hoop:** the design, as sewn (turned by Rotate in Hoop), doesn't fit the
  document's hoop (an error), or fits only turned a quarter turn (a warning naming the turn to set). A
  design split into hoopings is checked hooping by hooping.

Exporting a DST adds a line for each rule that found something to the export's warnings.
`embroidery.check` gives the same findings to agents and scripts, each with the object and where it is.

**Realistic Stitches.** View › **Realistic Stitches** (it turns on Stitch Preview too; the Embroidery
panel has a **Realistic** check beside Stitch Preview) draws the design as it will look sewn: each stitch
a round, twisted thread lit from the upper left, so satin and fill rows catch the light by their stitch
angle; threads cast soft shadows on the fabric and on what was sewn before them; and a jump that isn't
trimmed shows as the loose thread it leaves. The fabric under it is drawn in code: **Plain Weave**,
**Twill**, **Denim**, **Knit**, **Terry**, **Leather** or **Felt**, by default as the document's fabric
profile suggests (knit for Knit, terry for Towel, twill for Cap…). Choose another, and the fabric's
colour (a navy polo, a white towel), in the panel's **Fabric Look** row or with `embroidery.setup
{fabricTexture, fabricColor}`; both are saved with the document and change nothing that is sewn.
**None** (`fabricTexture: "none"`) draws no fabric: only the shaded threads, over the artwork.
Zoomed out, fine weaves blend into an even colour rather than moiré. While the view pans or zooms the
last picture moves with it, and the view is drawn again once it is still. `embroidery.preview {style:
realistic}` renders the same picture as a PNG, and MCP's `render_preview {stitches: "realistic"}` shows
it to agents.

**Garment mockups.** Object › Embroidery › **Garment Mockup…** (or **Mockup…** in the Embroidery panel)
shows the design sewn on a garment, for a customer to approve before anything is sewn: a **T-Shirt**,
**Polo Shirt**, **Hoodie**, **Cap**, **Tote Bag** or **Towel**, each drawn in code at an adult size
(nothing is traced from photographs). The design keeps its real size and is centred where such designs
usually go: **Left Chest** (the wearer's left, about 95 mm from the centre line and 190 mm below the
shoulder), **Right Chest**, **Center** (a T-shirt or hoodie front, a cap front, a tote bag) or **Above
the Border** (a towel). Each place has the largest size usually sewn there (100 × 100 mm on a chest,
115 × 57 mm on a cap front); a design larger than that is still drawn, with a warning. The garment has
its own fabric (knit shirts, a twill cap, a canvas tote, a terry towel) and colour, or the colour chosen
in **Fabric Look**; the thread is drawn as Realistic Stitches draws it. **Whole Garment** shows all of
it; **Close-Up** shows the design and the cloth round it. The mockup opens as a new document the
garment's real size, ready to export or print; `embroidery.mockup {garment, placement, view, width,
fabricTexture, fabricColor, format: png, path?}` writes or returns the PNG instead.

**Real size.** View › **Real Size** (or **Real Size** in the Embroidery panel) zooms so a millimetre of
the design is a millimetre on the screen, centred on the selection or else the design: hold a garment,
a hoop or a ruler against the screen to judge it. Few systems know a screen's true size, so calibrate
it once with View › **Calibrate Real Size…**: hold a bank card (its width is 85.60 mm, ISO/IEC 7810
ID-1) or a ruler (100 mm) against the bar, drag the bar's end until they match (or type what the bar
measures, or the screen's resolution in pixels per inch), and click OK. The result is Preferences ›
General › **Screen Resolution for Real Size** (`screenPpi`, device pixels per inch), kept with the
preferences, not the document; **Reset** in the dialog goes back to not calibrated, where Real Size
assumes the system's reference density (96 per inch at its display scale) and is only roughly to scale.
The app's own UI scaling doesn't change it. Real Size works with Stitch Preview and Realistic Stitches.

**The density adviser.** View › **Stitch Density** (it turns on Stitch Preview too) tints the design by
needle holes per mm²: green where it is light, amber near the limit, red past it (10 per mm², the 40
holes in a 2 × 2 mm spot that the checker counts), with a legend in the canvas's corner. A dense spot or
long stitches in the Check list have a **Fix…** menu of suggested fixes, each tried on a copy of the
design first and offered only when it clears the finding, or at least thins the spot, without causing a
new one:

- lower the top object's density (its row or zigzag spacing) just enough to clear the spot, up to 2.5
  times the stitch type's usual spacing (sparser doesn't cover);
- cut away the rows of a fill below that are covered (turn off Keep Covered);
- a lighter underlay (zigzag to edge walk, edge walk to centre walk, else none);
- delete an object that is a stitched copy of one below;
- for long stitches, trim before the object.

Each fix is one undo step. Where several layers stack, one fix may only thin the spot (the menu says how
many holes it leaves): fix again. `embroidery.density` gives agents the map, `embroidery.fixes` the
suggestions and `embroidery.fix` makes one.

### Thread charts

A thread chart lists a maker's threads: code, name and colour. Load one into the document (the
Embroidery panel's **Thread Chart** › **Load…**, File › Open, or `embroidery.loadThreadChart`) and each
thread colour of the design gets the nearest thread in it, by CIEDE2000 colour difference: the panel
shows the codes under the design's thread chips (hover for names and the difference), and
`embroidery.matchThreads` lists them. VectorCraft ships no makers' charts: their numbers and colours are
the makers' data, and none is published under terms that allow redistributing it, so load charts you
have the right to use, or make your own ([specification](formats/vcthreads.md)).

### Hoops

The Embroidery panel's **Hoop** menu (Design section, `embroidery.setHoop`) sets the hoop the design is
sewn in: a common size (100 × 100, 130 × 180, 160 × 260, 200 × 200, 200 × 300 mm, or a cap's
130 × 60 mm), or one from a `.vchoop` file (File › Open; [specification](formats/vchoop.md)). Its sewing
field is drawn in Stitch Preview as a dashed outline centred on the design, as machines centre it, and
the panel says whether the design fits. The design checker reports a design that doesn't fit (an
error) or fits only turned a quarter turn. The common sizes are nominal; a machine's own field can be a
little smaller, so set its real size (`embroidery.setHoop {width, height}`) when it matters.

**Rotate in Hoop** (under the Hoop menu; `embroidery.setup {hoopRotation: 0 | 90 | 180 | 270}`) turns the
design clockwise as it is written to the machine file, about the middle of its stitches, so a tall
design fits a wide hoop (or a cap frame) without turning the artwork. The hoop check, the panel's fit
note and the worksheet use the turned design, and Stitch Preview draws the hoop's field turned the
other way round the design as drawn. The rotation is saved with the document.

**Split into Hoopings** (the button under the Hoop menu when the design is too big;
`embroidery.splitHoops {hoop?, overlap?, marks?, basting?}`) splits a design too big for the hoop into a
grid of equal areas, each small enough to fit the hoop (turned by Rotate in Hoop) with room to spare,
sewn one after another with the fabric hooped again: row by row from the top, each row from the left
(areas with nothing to sew are left out). Each stitch belongs to the area its middle lies in, so every
stitch is sewn once; each hooping also sews the stitches up to **overlap** (2 mm; 0–10) past its seams
so the pieces meet without a gap. A fill's rows are cut where they cross a seam and a satin column
between its stitches, and each cut run is locked. **Alignment Marks** (on by default) are two
running-stitch crosses on each seam: the hooping sewn first sews them last, and the next sews them first,
so you can line the rehooped fabric up by them (move the design on the machine until the needle meets
them). **Basting Box** sews a running-stitch box round each hooping's field first, to hold the fabric to
the stabilizer. Stitch Preview draws each hooping's field, numbered in sewing order; the panel says
whether each fits; the worksheet lists them; and exporting writes a file per hooping
(`name-hoop1.dst`, `name-hoop2.dst`…; `hoopings: false` writes the whole design). After an edit, split
again: the checker reports a hooping that no longer fits. **Join Hoopings**
(`embroidery.splitHoops {clear: true}`) sews it in one hooping again. Machines centre each file in the
hoop on its stitches (on its basting box when there is one).

### Shortest stitch

Very short stitches cut the fabric, break threads and pile up. Needle points closer than the **Shortest
Stitch** (Sew Order panel, `embroidery.setup {minStitch}`, 0.3 mm) to the one before are left out
everywhere, satin columns included; a fabric can ask for longer (Leather: 1 mm). Manual stitches are sewn
exactly as drawn.

### Appliqué

Appliqué sews a piece of fabric on instead of filling an area with thread. Select closed shapes and choose
**Object › Embroidery › Appliqué** (or the **Appliqué** button in the Embroidery panel, `embroidery.applique`).
Each shape becomes a group named Appliqué, in the shape's place, holding three stitched copies sewn in
order in the shape's thread:

1. **Placement Line**: a running stitch on the outline, showing where the fabric goes. The machine then
   stops (**place the fabric**).
2. **Tack-Down**: just inside the outline (**inset**, 0.8 mm), a running, zigzag or bean stitch holding the
   fabric. The machine stops again (**trim the fabric** back to the tack-down).
3. **Cover**: over the raw edge, centred on the outline: a satin column (**width**, 3 mm) by default, or a
   zigzag, a bean stitch, or none. No underlay: the tack-down holds the fabric.

A satin or zigzag cover must reach past the tack-down's inner side, or the trimmed edge would show; the
command refuses a cover too narrow for its inset. The parts are ordinary stitched objects, so each can be
edited afterwards (or its stops changed with **Stop Before**); the worksheet's thread sequence says what to do
at each stop.

**Cut files.** A fabric cutter can cut the appliqué pieces. **Object › Embroidery › Appliqué Cut File…** (or
**Cut File…** in the panel, `embroidery.cutFile`) takes the selected appliqué groups (none selected: all of
them) or closed shapes and packs their cut outlines onto cutter mats, in a new document with one artboard per
mat, ready for File › Export as SVG or DXF (or written straight to files with `format` and `path`). Each piece
is its placement line, grown by the **allowance** (0 mm; more makes the fabric reach under the tack-down),
cut as a hairline with no fill. Pieces are packed onto as few mats as they fit on (**mat** 12 × 12 in by
default, 12 × 24 in, or any size), at least the **gap** apart (3 mm), clear of the mat's **margin** (5 mm),
turned in quarter turns to fit (**rotate**: none for fabric with a grain or a print that must run one way, or
any angle in 15° steps) and, with **flip**, turned over. A piece too big for a mat is reported, not dropped.
Brother CanvasWorkspace, Cricut Design Space and Silhouette Studio read the SVG. Brother's own FCM format
isn't written: there is no public specification of it.

### Puff (3D foam)

Set **Puff Foam** (`puff`, the foam's thickness, 1–6 mm) on a satin column, satin shape or Auto lettering
to sew it raised over foam. VectorCraft then does what puff needs:

- **A stop for the foam.** The machine stops before the first puffed object, for the foam to be laid
  over the area (written as a stop: a thread change to the same thread). Sew flat parts first: the
  checker warns about flat objects sewn after the foam is down, since they would sew through it.
- **Dense satin, no underlay.** 0.25 mm spacing unless you set one, so the foam doesn't show, and no
  underlay to flatten it.
- **Capped ends.** Open columns are lengthened by half the foam's thickness at each end, closing over
  the foam's cut edge.
- **A perforation.** After each puffed object, running stitches 1 mm long go all round its edge: the
  needle holes cut the foam, so the excess tears away cleanly.

Puff rises well from about 3 to 10 mm wide; the checker warns outside that. The test sheet's **Puff**
section sews 3 mm foam at 3–8 mm.

### Production worksheet

**Worksheet…** in the Embroidery panel (`embroidery.worksheet`) opens a printable page for whoever sews
the design: its picture in stitches; size, stitch count, thread changes, trims and jumps, hoop, fabric
and sewing time; the thread sequence with colour chips, each matched to the chosen thread chart's
nearest thread, with an estimate of the thread it uses (the stitches' length plus 20 % taken up into the
fabric); and a box for notes. It is an ordinary document: print it, or export it as PDF
(`embroidery.worksheet {format: pdf}` gives the PDF straight away). A4 unless `page: letter`.

### Playing the sewing

The Sew Order panel plays the design as the machine sews it: **Play** draws it stitch by stitch in its
threads, the needle ringed; **Pause** stops; the arrows jump to the previous or next thread change.
Dragging the scrub bar or clicking a row goes back to stepping object by object. Each row shows the
object's stitch count, and the panel shows the **Sewing Time**: the stitches at the **Machine Speed**
(Travel, 600 stitches a minute unless set; `embroidery.setup {speed}`), plus 5 seconds a trim and 30
seconds a thread change. `embroidery.info` gives it as the design's `minutes`.

### Sequencing

Running, bean and zigzag lines are entered nearest where the needle is: a closed running path at its
nearest point, an open path from its nearer end, an object's separate parts nearest first
(`embroidery.setup {autoStart: false}` sews them as drawn). `startAt` (`[x, y]` mm) fixes where one
starts. With Stitch Preview on, a selected running, bean or zigzag object shows its **start handle**: a
green dot at its Start At point, hollow at its first point while the start is automatic. Drag it with the
Selection tool to slide the start along the path (one undo step); double-click it to make the start
automatic again. Fills and satins keep their own order, since their underlay comes first. **Optimize** in the Sew
Order panel (`embroidery.optimizeOrder`) groups the objects by thread and sews each thread's objects
nearest first, cutting jumps and trims; what is sewn later lies on top, so check overlapping objects of
one colour afterwards.

**Branching.** Running and bean lines of one object that meet (end to end, or an end touching another line
within 0.25 mm) are sewn as one network in one pass: each line once, the way back along a branch sewn over
its own stitches (a bean's repeats only on the first pass), the longest branch last so it needs no way
back. A tree, a star or a mesh of lines then sews with no jumps or trims inside it. Lines that meet
nothing are sewn one by one, as before.

**Hidden travel.** A move to the next stitch that lies under a fill or satin shape sewn later, at least
0.5 mm inside it all the way, is sewn there as running stitches up to 2.5 mm long instead of jumped and
trimmed: the fill sewn on top covers it (the Sew Order panel's **Hide Travel Under Fills**,
`embroidery.setup {hideTravel}`, on unless turned off). Only moves within one thread are sewn this way.

### Overlap removal

Where a fill lies under something sewn after it (lettering on a filled background, a satin outline over
a fill, one fill over another), the fill's rows under it are cut away, so the stitches don't pile up into
a stiff, bullet-proof patch. The rows still reach **0.5 mm** under the covering object's edge so no gap
shows; the Sew Order panel's **Overlap** (`embroidery.setup {overlap}`) sets that for the document. Fills,
satin shapes, Auto objects and type cover by their outlines, satin columns by their band; running, bean,
zigzag and pattern borders cover nothing. Tick a fill's **Keep Covered Rows** (`keepCovered`) to sew it
whole. The fill's underlay is cut the same way. Where the covering object reaches past the fill's own
edge, no rim is kept (no gap can show there). Moving or reordering what lies on top sews the fill again.

### Fabric profiles

The fabric changes how a design should be sewn: stretchy knits want more underlay and compensation, lofty
towels dense stitches, leather fewer holes. The Embroidery panel's **Fabric** menu (Design section,
`embroidery.setFabric`) sets the document's fabric: **Woven**, **Knit**, **Fleece / Towel**, **Cap**,
**Leather** or **Denim**, or one loaded from a `.vcfabric` file (**Load…**, or File › Open). Its settings
fill in whatever an object leaves unset, so an object's own settings always win:

| Setting | What it fills in |
|---|---|
| `satinSpacing`, `fillSpacing` | the density of satins and satin shapes, and of fills |
| `pull`, `pullPercent`, `push` | the compensation of satins, satin shapes and fills |
| `satinCenterUnder`, `satinZigzagOver`, `shapeEdgeOver` | where the automatic underlay changes (2, 5 and 2.5 mm without a fabric) |
| `minStitch` | the shortest stitch (when longer than the document's own, 0.3 mm) |

The starters' numbers are starting points from general digitizing practice; the sew-out test sheets
(below) are how to check them on your fabric. **Save…** writes the fabric as a `.vcfabric` file to share
([specification](formats/vcfabric.md)).

### Stitch styles

A stitch style is a named set of stitch settings, the stitch type and any of its settings, applied with one
click. The Embroidery panel's **Stitch Styles** menu (next to the stitch picker) lists the document's styles
and three starters: **Small Lettering** (Auto, a little denser, for type 5–10 mm tall), **Bold Border** (a
4 mm satin with automatic underlay) and **Light Background Fill** (an open fill without underlay). Picking
one stitches the selection with it (`embroidery.applyStyle`); the style the selection already matches is
ticked. **Save Style** keeps the selection's settings as a style in the document (`embroidery.saveStyle`;
**Save to Library** also writes it to the embroidery library), and **Load Style File…** (or File › Open)
reads a `.vcstyle` file ([specification](formats/vcstyle.md)).

A style replaces an object's stitch settings, except what belongs to that object: its start point, stitch
angle lines, satin rails, trims and stops, and its thread colour when the style sets none. Objects keep no
link to the style, as with fabric profiles: changing or removing a style later doesn't change them. A style
that names a border pattern, motif or embroidery font can only be applied where the document has it.

Graphic styles (Window › Graphic Styles) carry stitch settings too: **New Graphic Style** made from a
stitched object keeps its settings (less the same per-object ones), and applying the graphic style
stitches the paths and type it styles, so one click gives both the look and the stitching.

### Libraries

Border patterns, motifs, fabric profiles, hoop lists, thread charts, embroidery fonts and stitch styles you
want in every design go in the **embroidery library folder**: a folder called `Embroidery` next to the
preferences, with a subfolder for each kind (`borders`, `motifs`, `fabrics`, `hoops`, `threads`, `fonts`,
`styles`). VectorCraft reads it at start (and on
`embroidery.rescanLibraries`), so files can be shared just by copying them in. A save command with
`user: true` writes there, and `embroidery.addToLibrary` copies a file in (never replacing one: a name
already taken gets a number). `embroidery.libraries` lists what is there, and files that can't be read are
listed with the reason instead of being dropped silently.

In the Embroidery panel, the border pattern picker and the **Fabric**, **Hoop** and **Thread Chart** menus
list the library's entries after the document's own, marked **(Library)**; picking one loads it. **Add to
Library** next to each copies the document's own pattern, fabric, hoop or chart into the library (it is
hidden once the library has the same), and **Show in Folder**, under the Design section, opens the folder.

Loading an entry (`library` on `embroidery.loadBorderPattern`, `loadFabric`, `loadHoop` or
`loadThreadChart`, by its id or name) **copies it into the document**, as loading a file does, so the
design sews the same on a machine without your library. Changing the library file afterwards doesn't
change documents that already use it. The web app has no library folder.

### Editing large designs

Stitches are sewn again only for the objects an edit changes. On a large design even that can make a
drag stutter, so **Preferences › Performance › Regenerate Stitches** (`prefs.set {key:
"regenerateStitches", value: "whileEditing" | "whenEditEnds"}`) chooses when:

- **While Editing** (the default): every frame of a tool's drag sews the moved or reshaped objects again,
  so Stitch Preview and the panel follow the pointer.
- **When the Edit Ends:** during a drag, Stitch Preview, the Embroidery panel and the design checks show
  the design from before the drag, with the changed objects' outlines dashed where they now are. The
  stitches are sewn again when the drag is committed (on release). Dialog previews and typed values are
  not affected.

`vectorcraft-cli perf` budgets one pointer move of such a drag on a 300-object design.

### Sew-out tests

Stitch quality is only proven on fabric. **New Test Sheet…** (Sew-Out Test panel, `embroidery.testSheet`)
makes a new document of calibration swatches, one artboard per section, each a 100 × 100 mm design that
fits the smallest common hoop:

| Section | Swatches |
|---|---|
| A Satin Width | satin columns 1–10 mm wide |
| B Satin Density | 4 mm satin at spacing 0.3–0.8 mm |
| C Pull Compensation | 3 mm satin with pull compensation 0–0.5 mm, between running-stitch marks at the drawn edges |
| D Fill Density | fill squares at row spacing 0.3–0.6 mm |
| E Fill Angle | fill squares at 0°, 45°, 90° and 135° |
| F Underlay | satin with each satin underlay; fill without and with fill underlay |
| G Lettering | "Hag" in Auto at 4–20 mm (needs a type face) |
| H Run Styles | running stitch in each style (single, chain, stem, backstitch, sashiko) on an S curve with a sharp corner |
| I Split Satin | satin 8, 12 and 20 mm wide split (staggered), 12 mm split at random, and 12 mm not split to compare |
| J Puff | satin 3–8 mm wide over 3 mm foam (last, since anything sewn after the foam is laid sews through it) |

Each swatch changes one setting, the others keep their defaults, and each is labelled in stitches with its
value under it (a single-stroke font drawn in code). Everything is sewn in one thread. Export each section
as its own design with `document.export {format: dst, artboard: n}`, or all of them at once with
`range: "all"` (a file per artboard).

After sewing, the **Sew-Out Test** panel is the form: the fabric and machine, and for each swatch
(`A3`: section A, third swatch) Pass or Fail and a note. The record is saved in the document
([`sewOut`](formats/vectorcraft-embroidery.md#sew-out-test-sewout)), so a sheet saved per fabric (woven,
knit) keeps what each showed. `embroidery.sewOut` reads the form; `embroidery.recordSewOut` fills it in.

#### The compensation sheet

The thread pulls the fabric in along the stitches and pushes it out at their ends, and how much depends on
the direction the stitches run against the fabric's grain. The **compensation sheet** measures that. Tick
its sections in the Sew-Out Test panel (they are listed last and start unticked), or ask for
`embroidery.testSheet {sections: ["compensation"]}`:

| Section | Swatches |
|---|---|
| K Compensation 0° | satin columns 30 mm long, 2, 4, 6 and 8 mm wide, and an 18 mm fill square, stitches across the grain |
| L Compensation 45° | the same, turned so the stitches run at 45° |
| M Compensation 90° | the same, stitches along the grain |
| N Compensation 135° | the same at 135° |

Every swatch is sewn with no compensation (`pull`, `pullPercent` and `push` at 0, whatever the fabric
profile says) between running-stitch marks at its drawn edges, so what you measure is the fabric's own
draw-in. Hoop the fabric with its grain (the warp, along the selvage) running from the top of the hoop to
the bottom. After sewing, measure each swatch's width along its stitches and its length across them, and
record them with `embroidery.recordSewOut {results: [{swatch: "L2", measuredWidth: 3.7, measuredLength:
30.6}]}`; the form keeps them with the drawn sizes. Fitting compensation from them, per angle, into a fabric
profile is the next step (E10.1b).

### Commands

| Command | Params |
|---|---|
| `embroidery.set` | `{type?: auto\|running\|bean\|satin\|zigzag\|satinShape\|fill\|patternBorder\|manual, length?, spacing?, angle?, width?, thread?: "#rrggbb"\|[r,g,b], pull?, underlay?, stagger?, passes?, pattern?, flip?, trimBefore?, trimAfter?, ids?}`. Lengths are in mm; `null` resets one to its default. |
| `embroidery.clear` | `{ids?}` |
| `embroidery.borderPatterns` | → the patterns pattern borders can use (the document's, then the starters) |
| `embroidery.loadBorderPattern` | `{path? \| data? \| library?}`: load a `.vcborder` file into the document → `{id, name, added}` |
| `embroidery.saveBorderPattern` | `{pattern, path?, user?}`: write a pattern as a `.vcborder` file (without `path`: `{data}`; `user: true`: into the embroidery library) |
| `embroidery.extractBorderPattern` | `{name?, period?, author?, license?, source?, ids?}`: the repeat of the selected border stitches (only those inside a selected closed shape, if any) as a new document pattern → `{id, name, period, width, stitches, repeats, error}` |
| `embroidery.extractMotif` | `{name?, offsets?, author?, license?, source?, ids?}`: the tile of the pattern repeating both ways in the selected manual stitches (only those inside a selected closed shape, if any) as a new document motif → `{id, name, offsets, stitches, tiles, error}` |
| `embroidery.motifs` | `{}`: the document's motifs, then the built-in carve motifs, with their kind, size, lattice, credits and how many objects use them |
| `embroidery.loadMotif` | `{path? \| data? \| library?}`: load a `.vcmotif` file into the document → `{id, name, motifKind, added}` |
| `embroidery.saveMotif` | `{motif, path?, user?}`: write a motif as a `.vcmotif` file (without `path`: `{data}`; `user: true`: into the embroidery library) |
| `embroidery.removeMotif` | `{motif}`: remove one of the document's motifs that nothing uses |
| `embroidery.removeBorderPattern` | `{pattern}`: remove one of the document's patterns that nothing sews with |
| `embroidery.setup` | `{trimJumpsOver?: mm \| null, stitchMovesUnder?: mm, overlap?: mm, minStitch?: mm, autoStart?, hideTravel?, speed?: stitches/min, machineFormat?: dst \| pes \| jef \| exp, hoopRotation?: 0 \| 90 \| 180 \| 270}`: the document's travel settings, overlap, shortest stitch, machine speed, the format the panels export and the design's turn in the hoop |
| `embroidery.info` | → the design (`stitches`, `jumps`, `trims`, `colorChanges`, `threads`, `widthMm`, `heightMm`, `warnings`) and the selection's settings |
| `embroidery.stitches` | `{limit?}` → the stitches as sewn: `runs` of points in mm, `jumps`, `threads` |
| `embroidery.check` | `{ids?}` → the design checker's `findings` (`rule`, `severity`, `message`, `object`, `at` in document points), `counts` and the `rules` |
| `embroidery.digitize` | `{id?, colors?, minArea?, keepWhite?, order?, fix?, lines?}`: a logo (image, traced image or group of filled shapes) as stitch objects, one undo step; `lines: centreline` sews parts under 2 mm wide along their centre lines → `{id, objects, threads, merged, dropped, centrelines, fixes, findings}` |
| `embroidery.fillAt` | `{x, y, outline?, centreline?, width?, thread?}`: the closed area at a point (document points) as a new stitch object on top: Auto, its outline (`outline: running` or `satin`), or a thin area along its centre line (`centreline: auto`, `running` or `satin`); the Stitch Fill tool's click → `{id, areaMm2, outline, centreline, type, widthMm}` |
| `embroidery.density` | `{min?}` → the density map: `cells` (`x`, `y`, `size` in document points, `holes`), densest first, with the `limit`, `peak` and `perMm2` |
| `embroidery.fixes` | `{object, rule?}` → the suggested `fixes` for the checker's findings on an object (`fix`, `label`, `object` changed, `clears`, `holes` left, the `command` and `params` it runs) |
| `embroidery.fix` | `{object, rule?, fix?}`: make a suggested fix (default the first) as one undo step → `{fix, label, object, clears, findings}` (counts after) |
| `embroidery.preview` | `{scale?, artboard?, style?: flat\|realistic}` → the artboard as Stitch Preview (flat) or Realistic Stitches shows it, as a PNG (`pngBase64`); MCP's `render_preview {stitches: true \| "realistic"}` uses it |
| `embroidery.order` | → the stitched objects in sewing order |
| `embroidery.reorder` | `{to: first\|earlier\|later\|last, ids?}` or `{order: [ids]}`: change the sewing order |
| `embroidery.sortByThread` | group the sewing order by thread colour |
| `embroidery.optimizeOrder` | group by thread and sew each thread's objects nearest first, cutting travel → `travelBefore`, `travelAfter` |
| `embroidery.resetOrder` | sew in paint order again |
| `embroidery.threadCharts` | → the document's thread `charts` and the `chart` matched against |
| `embroidery.loadThreadChart` | `{path? \| data? \| library?}`: load a `.vcthreads` chart into the document |
| `embroidery.setThreadChart` | `{chart: id \| null}`: the chart the design's colours are matched against |
| `embroidery.removeThreadChart` | `{chart}`: remove a chart from the document |
| `embroidery.saveThreadChart` | `{chart, path?, user?}`: write a chart as a `.vcthreads` file |
| `embroidery.matchThreads` | `{chart?}` → each design thread colour's nearest chart thread (`code`, `name`, `threadColor`, `deltaE`) |
| `embroidery.hoops` | → the document's `hoop` (or null), the `common` hoops, whether the design `fit`s (as sewn, turned by `hoopRotation`; `split` when split into hoopings that each fit), the `rotation` and how many `hoopings` |
| `embroidery.setHoop` | `{hoop?: 4x4\|5x7\|6x10\|8x8\|8x12\|cap\|null, name?, width?, height?, round?}`: set the document's hoop |
| `embroidery.loadHoop` | `{path? \| data? \| library?, hoop?}`: a hoop from a `.vchoop` file as the document's |
| `embroidery.saveHoop` | `{hoops?, path?, user?, name?}`: write the document's hoop (or common ones) as a `.vchoop` file |
| `embroidery.splitHoops` | `{hoop?, overlap?: mm, marks?, basting?}` or `{clear: true}`: split a design too big for the hoop into hoopings (one undo step), or sew it in one hooping again → as `embroidery.hoopings` |
| `embroidery.hoopings` | → the document's `hoopings`: the grid's `columns` and `rows`, `overlap`, `marks`, `basting`, and the `pieces` in sewing order (`number`, `area` in mm, `stitches`, `own`, `fits`) |
| `embroidery.fabrics` | → the document's `fabric` (or null) and the six `starters`, with their settings |
| `embroidery.setFabric` | `{fabric?: woven\|knit\|towel\|cap\|leather\|denim\|null, name?, satinSpacing?, fillSpacing?, pull?, pullPercent?, push?, satinCenterUnder?, satinZigzagOver?, shapeEdgeOver?}`: set the document's fabric (a starter, then the settings given) |
| `embroidery.loadFabric` | `{path? \| data? \| library?}`: a `.vcfabric` file as the document's fabric |
| `embroidery.saveFabric` | `{fabric?, path?, user?}`: write the document's fabric (or a starter) as a `.vcfabric` file |
| `embroidery.styles` | → the document's stitch `styles`, then the starters (`id`, `name`, `builtin`, `settings` as `embroidery.set` takes them), and the one the selection matches (`selected`) |
| `embroidery.saveStyle` | `{name?, id?, description?, path?, user?}`: the selected object's stitch settings as a document style (a style of the same id is replaced); `{style, path?, user?}`: write a style as a `.vcstyle` file → `{id, name, replaced}` |
| `embroidery.applyStyle` | `{style? \| library?, ids?}`: stitch the selection with a style's settings, keeping each object's start point, angle lines, rails, trims, stops and (unless the style sets one) thread → `{style, changed}` |
| `embroidery.loadStyle` | `{path? \| data? \| library?}`: load a `.vcstyle` file into the document → `{id, name, added}` |
| `embroidery.removeStyle` | `{style}`: remove one of the document's styles (objects keep their settings) |
| `embroidery.libraries` | `{kind?: borders\|motifs\|fabrics\|hoops\|threads\|fonts\|styles}` → the embroidery library `folder`, its `libraries` (`kind`, `id`, `name`, `path`…) and the files `skipped` as unreadable |
| `embroidery.rescanLibraries` | re-read the embroidery library folder → the same as `embroidery.libraries` |
| `embroidery.addToLibrary` | `{path? \| data? with name?, kind?}`: copy a `.vcborder`, `.vcmotif`, `.vcfabric`, `.vchoop`, `.vcthreads`, `.vcfont` or `.vcstyle` file into the embroidery library (never replacing one) |
| `embroidery.applique` | `{ids?, cover?: satin\|zigzag\|bean\|none, width?, tackDown?: running\|zigzag\|bean, inset?}`: each closed shape becomes an appliqué group (placement line, stop, tack-down, stop, cover) → `{groups: [{id, placement, tackDown, cover}]}` |
| `embroidery.cutFile` | `{ids?, allowance?, gap?, mat?: 12x12\|12x24\|{width, height}, margin?, rotate?: none\|quarter\|any, flip?, format?: svg\|dxf, path?}`: the appliqué pieces nested onto cutter mats → `{mats, pieces, unplaced, index \| files}` |
| `embroidery.fonts` | → the embroidery `fonts` loaded into the document (`id`, `name`, `glyphs`, `chars`, `sizes`, licence…) |
| `embroidery.loadFont` | `{path? \| data? \| library?}`: load a `.vcfont` embroidery font into the document → `{id, name, glyphs, added}` |
| `embroidery.saveFont` | `{font, path?, user?}`: write one of the document's fonts as a `.vcfont` file (`user: true`: into the embroidery library) |
| `embroidery.removeFont` | `{font}`: remove a font from the document |
| `embroidery.font.new` | `{name?, chars?, from?, author?, license?}`: a new embroidery font document (one artboard per glyph; `from`: an installed font to trace on a template layer) → `{index, glyphs}` |
| `embroidery.textArc` | `{text?, cx, cy, radius, side?: top\|bottom, size?, font?, style?, color?, tracking?, stitch?: auto\|none}`: type on a circle arc (pt), or the selected point type put on one → `{id}` |
| `embroidery.monogram` | `{letters, layout?: classic\|three\|two\|stacked\|circle\|diamond, cx?, cy?, height?, font?, style?, color?, frame?: none\|satin\|pattern id, stitch?, id?}`: a monogram group of live type (with `id`: that monogram laid out again) → `{id, letters, frame?}` |
| `embroidery.font.trace` | `{from, chars?, name?, author?, license?}`: an installed font digitized automatically into a font document (pen strokes as satin columns between rails) → `{index, glyphs, traced, outlines}` |
| `embroidery.font.open` | `{path? \| data? \| library? \| font?}`: an embroidery font as a font document, to edit |
| `embroidery.font.info` | → the font document's `font`, `glyphs` (`chars`, `advance`, `objects`), `empty` glyphs and `kerning` |
| `embroidery.font.setup` | `{name?, author?, license?, source?, ascender?, capHeight?, xHeight?, descender?, sizes?…}`: the font's details and metrics |
| `embroidery.font.kern` | `{left, right, value \| null}`: a kerning pair |
| `embroidery.font.sample` | `{text?, size?}`: a line of type below the artboards sewn from the font as it is now |
| `embroidery.font.save` | `{path?, user?}`: write the font as a `.vcfont` file → also `glyphs`, `empty` |
| `embroidery.importInkstitchFont` | `{path, save?, user?}`: convert an Ink/Stitch font folder and load it → `{id, name, glyphs, added, license, report: {skipped, unmapped, commands, baseline}}` |
| `embroidery.fontFolders` | `{folders?}`: the folders searched for Ink/Stitch fonts (read in place) → `{folders, fonts, skipped}` |
| `embroidery.worksheet` | `{page?: a4\|letter, format?: pdf, path?}`: the production worksheet, as a new document or a PDF |
| `embroidery.mockup` | `{garment?: tshirt\|polo\|hoodie\|cap\|tote\|towel, placement?: leftChest\|rightChest\|center\|lower, view?: garment\|design, width?, fabricTexture?, fabricColor?, format?: png, path?}`: the design on a garment drawn in code, at real size where such designs go, as a new document or a PNG (`designSize`, `warnings` when it is bigger than usual there) |
| `embroidery.testSheet` | `{sections?: [ids]}`: a new document of calibration swatches (see Sew-out tests; `compensation` for the compensation sheet) → `{index, sections, swatches, skipped}` |
| `embroidery.sewOut` | → the sew-out form: `{sewOut: {fabric, machine, notes, swatches: [{id, section, label, objects, result, notes, angle?, drawnWidth?, drawnLength?, measuredWidth?, measuredLength?}], counts} \| null, sections}` |
| `embroidery.recordSewOut` | `{fabric?, machine?, notes?, results?: [{swatch, result?: pass\|fail\|untested, notes?, measuredWidth?, measuredLength?}]}`: fill in the form, one undo step |
| `view.stitchPreview` | toggles Stitch Preview |
| `view.realisticStitches` | toggles Realistic Stitches (shaded thread on the fabric's texture; turns Stitch Preview on) |
| `view.realSize` | zooms so the design shows at its true size on the screen, by the calibrated `screenPpi` (or the system's reference density) → `{zoom, ppi, calibrated}` |
| `view.calibrateRealSize` | opens Calibrate Real Size: match a bar to a bank card or ruler held against the screen; OK sets the preference `screenPpi` (`prefs.set {key: screenPpi, value}` sets it directly; 0: not calibrated) |

Agents reach all of these over MCP (`run_command`), with the `embroidery` prompt, this guide as the
resource `vectorcraft://guide/embroidery`, the file format specifications as `vectorcraft://spec/…`, and
`render_preview {stitches: true}` to see the design (`stitches: "realistic"` to see it as sewn). See [mcp.md](mcp.md#embroidery).

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

## Machine files (DST, PES, JEF, EXP, VP3, U01)

`document.export {format: "dst" | "pes" | "jef" | "exp" | "vp3" | "u01"}` (File › Export As › Embroidery
(DST), (PES), (JEF), (EXP), (VP3) or (U01)) writes the design for the machine. The Embroidery panel's **Machine Format** menu
(`embroidery.setup {machineFormat}`, saved with the document) picks the format its **Export** button,
and the Sew-Out Test panel's, writes. Opening any of them (File › Open, Place, `document.open`) creates one
manual-stitch path for each run of stitches, grouped by colour block.

| Format | Machines | Thread colours | Trims |
|---|---|---|---|
| DST (Tajima) | nearly all | not stored: each block opens in a colour of its own | three short jumps back to the same point |
| PES (Brother, Baby Lock) | Brother | kept: the real colours in the version 6 thread list | a trim code |
| JEF (Janome) | Janome | by palette number only (see below) | a trim code (a jump of zero) |
| EXP (Melco) | Melco, Bernina | not stored | three short jumps, as DST |
| VP3 (Pfaff, Husqvarna Viking) | Pfaff, Husqvarna Viking | kept: each colour block's thread colour | none in the format: the machine trims at colour changes and at long jumps, as its settings say (the export warns) |
| U01 (Barudan) | Barudan | by needle number only (see below) | a trim code |

**Thread colours in PES and JEF.** Both formats name threads by number in the maker's own palette, and
the machine shows its palette's colour for each number. VectorCraft ships no maker's palette (as with
[thread charts](#thread-charts)), so it writes a number per distinct colour: blocks sewn in one thread
share a number, but the machine shows its own colours for them. PES keeps the real colours as well, in its
version 6 thread list, which VectorCraft and other software read back. A JEF file, or a PES file older than
version 6, opens with a colour of its own per palette number. Recolour the paths' strokes, or set
`thread`, before sewing.

**Needles in U01.** A U01 file names needles, not colours: the machine sews each block with whatever
thread is on that needle. VectorCraft writes a needle per distinct colour (1, 2, 3 …), at most 9 (the
needles a U01 record can name; past nine colours the needles are used again, with a warning), and a thread
stop where a block uses the needle before it again. A U01 file opens with a colour of its own per needle.

The design is written centred on its middle (DST, JEF, EXP, VP3, U01), where the machine starts, or
from its top-left corner (PES, which machines centre themselves). JEF also records the smallest of its hoops the
design fits (50 × 50, 110 × 110, 140 × 200 or 200 × 200 mm). Moves longer than a format allows are split:
12.1 mm (DST), 12.7 mm (JEF, EXP, VP3 and U01 stitches, and PES's), 25.5 mm (U01 jumps), 204.7 mm (PES
jumps), 3276.7 mm (VP3 jumps).

With `artboard`, `artboards` or `range`, the export writes a design per artboard, each holding the
stitched objects whose middle lies on that artboard (a test sheet's sections, or several designs laid
out in one document).

**Several formats at once.** `document.export {formats: ["dst", "pes"], path}` writes the design in each
format, each file next to `path` and named for its format (`Rose.dst`, `Rose.pes`), for a design sent to
several machines or to a shop that asks for two. In Export As, choosing a machine format shows **Also
Export** with a checkbox for each of the others. The other parameters apply to every format, so with
`useArtboards` (or `artboard`, `artboards`, `range`) it writes a file per artboard and format. Every file
is checked before any is written (a file this document was read from, and lost things reading, is
refused). Without a path, each format's design comes back in `exports`. At most 16 formats.

**Where the formats come from.** Each is implemented from public descriptions of it:
[DST](https://edutechwiki.unige.ch/en/Embroidery_format_DST),
[PES](https://edutechwiki.unige.ch/en/Embroidery_format_PES) with its
[PEC](https://edutechwiki.unige.ch/en/Embroidery_format_PEC) section,
[JEF](https://edutechwiki.unige.ch/en/Embroidery_format_JEF),
[EXP](https://edutechwiki.unige.ch/en/Embroidery_format_EXP),
[VP3](https://edutechwiki.unige.ch/en/Embroidery_format_VP3) (with
[Jason Weiler's notes](http://www.jasonweiler.com/VP3FileFormatInfo.html) on its stitches and header) and
[U01](https://edutechwiki.unige.ch/en/Embroidery_format_U%3F%3F). Where a description leaves a point open
(EXP's units and direction, where PEC's stitches start, VP3's stitch direction and a header word only
one description lists) it was settled by writing small test designs with
an independent open-source tool, [pyembroidery](https://github.com/EmbroidePy/pyembroidery), and reading
its files' bytes (used as a test tool only; none of its code or palettes are in VectorCraft). EXP has no
documented trim code (`80 80` is listed both as the end and, by some readers, as a trim), so VectorCraft
writes trims as DST does and reads `80 80` as a trim, or as the end when nothing follows it. VP3 has
no trim code either; its `80 03` is described as the end, and some writers also put it where they cut
the thread, so VectorCraft reads it as a trim when more stitches follow. VP3's long moves (`80 01` …
`80 02`) are jumps in one description; some readers take the shorter ones as long stitches. U01's end is
command 18 in its description; some writers use 24 (0x18), which is read as the end too.

**Singer XXX** is not supported: its only public description is a stub that leaves out the header's
field sizes (so where the stitches start), how the stitch bytes are signed and the colour table the file
ends with, and VectorCraft implements formats only from public descriptions.

**Checked against an independent reader.** Files VectorCraft writes have been read with pyembroidery: for
each format the stitch and colour-change counts and the design size agree, PES's thread colours come back
exactly, and the trims are there (pyembroidery folds a trim next to a colour change into the change, and
reads EXP trims as the jumps they are). DST has been sewn on a commercial machine: the test sheet, on woven and on knit, passed gate G1's
checks (2026-11-09). The other formats have not yet been sewn.

## What's next

- **E3, quality: done.** Underlay, pull compensation, tie-ins and tie-offs, satin corners, fill routing, satin
  between rails, carved motif fills and the Embroidery panel.
- **E4, breadth:** other formats as usage demands (DST, PES, JEF and EXP are done, E7.12; VP3 and U01, E9.3); hoop templates; auto-digitizing traced art.
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
    - **Ink/Stitch fonts:** Ink/Stitch's embroidery font folders (`font.json` and glyph-layer SVGs) open
      as embroidery fonts and convert to the open format, with their licence carried over (plan step
      E8.1e, from Ink/Stitch's public documentation). VectorCraft ships none: you install the ones you
      want, by pointing VectorCraft at a folder of them or by putting them in its embroidery fonts folder.
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

## Development plan for future steps

This section folds the research notes from the embroidery feature review into the working plan for VectorCraft.
It keeps the product focus on what matters most: turning vector artwork into a clean, machine-friendly design
that sews reliably on real fabric.

### Scope and priorities

The catalog reviewed in the research phase includes 62 features across drawing, stitch types, lettering,
quality and output. They are organized into priority tiers:

- **P0 (must have):** the minimum features to digitize and export a typical design.
- **P1 (expected):** the features used by serious hobbyists and small shops.
- **P2 (differentiators):** advanced production, specialty techniques and automation.

The immediate goal is to build the features that form the “digitizing core”: object editing, stitch quality,
underlay and compensation, travel control, lettering, thread handling and export formats.

### Where the project is strongest, and where the gaps are

VectorCraft is strongest in fills, where the tatami and engraving pattern libraries already cover the core of
many pattern-based workflows. The largest gaps are:

- **Lettering:** automatic font conversion, kerning, monograms and text-on-path
- **Underlay and compensation:** automatic underlay types, axis-specific push/pull compensation, fabric profiles
- **Sequencing:** trims, ties, branching, travel under fills and start/stop control
- **Output:** more machine formats, worksheets, thread charts, stitch player and realistic previews
- **Extensibility:** pattern folders for motifs, fonts, palettes and hoops

This is careful scope control: the team should not chase more advanced feature areas until the basic production
workflow is robust and consistent.

### Phase 1: core production readiness (P0)

Phase 1 is the most important block. It covers the features needed before VectorCraft can produce high-quality
embroidery from real artwork without hand-fixing the result. The plan is to treat these as gates:

- **Object and stitch editing**
  - Bezier editing, boolean path operations, vector import and object-level stitch settings
  - Manual stitch placement, run and bean styles, fill generation and border generation
- **Quality and sewing behavior**
  - Automatic underlay, pull and push compensation, trim and tie handling, jump control, corner handling
  - Satin column support with sensible corner rules and reliable spacing
  - Fill row ordering and routing that avoid gaps, hidden overlaps and excessive jumps
- **Lettering basics**
  - Embroidery-type tools for small text and heavy display text, with the current Auto fallback preserved
  - Metrics and kerning groundwork for later font integration
- **Output and production**
  - DST export, thread assignment, colour sequencing, design inspection and export checks against independent readers
  - Printable production worksheets and thread information for real shops

This phase should also include a design checker that flags bad shapes, long stitches and risky geometry before
export.

### Phase 2: productive design workflow (P1)

Phase 2 turns the core workflow into a usable digitizing environment rather than just a stitch generator. This is
where the software gets closer to the commercial workflow expected by larger hobbyist and small-business users.

Priority items:

- **Advanced stitch types and fills**
  - Satin border tools, guided and curved fills, meander and stipple fills
  - Motif fills and programmable carving patterns
  - Contour, circular, spiral and radial fills
- **Lettering and text production**
  - TrueType/OpenType-to-stitch conversion, text-on-path, monograms and small-lettering fonts
  - Font creation tools and a versioned embroidery font format
- **Automation and sequencing**
  - Branching and auto-sequencing, density advisers, fabric profiles and small-stitch filtering
  - Auto-digitize from bitmap art and click-to-fill / click-to-outline workflows
- **Extensibility**
  - Pattern folders for motifs, fonts, palettes and hoop definitions
  - A content model similar to the existing stitch-pattern folders, so users can share work without vendor lock-in

The pattern-folder approach is already a strong fit for VectorCraft, and it is the best place to extend the product
without building one-off bespoke features.

### Phase 3: advanced production and differentiation (P2)

Phase 3 is where VectorCraft can build a distinctive toolchain beyond the basics. This is not required for a
functional embroidery app, but it is where tooling becomes a professional-grade platform.

Priority items:

- Specialty fills: fractal, fur, cascade, wave, cross-stitch, ripple stitch and tartan fills
- Realistic previews: thread-shaded views, fabric mockups and production estimators
- Appliqué workflows with placement, tack-down, cover stitches and cutter export
- Machine-transfer and cloud workflows, design libraries and extensions
- Machine-format support beyond DST, PES, JEF, EXP, VP3 and U01: others as usage demands (XXX once a public description covers it)

This phase should only begin after the production-quality basics are solid and the app proves its digitizing core on
real fabric.

### Recommended milestone sequence

The roadmap should be built in three stages, with each later phase gated behind the previous one:

1. **E3 / E4 gate:** production-ready stitch generation and export quality
2. **Phase 1 delivery:** the 18 P0 features needed to digitize, letter and export a design that sews well
3. **Phase 2 extension:** pattern libraries, font systems, automation and production workflow
4. **Phase 3 differentiation:** specialty techniques, realistic previews, extensions and advanced output

The key design rule is simple: do not add new fill families or advanced automation while the basic underlay,
compensation and trim behavior is still weak. A fill with no underlay, compensation or trimming will not sew cleanly.

### Open research note

At the research stage, there is a remaining open question: whether VectorCraft shares code with the existing
mcpembroidery server. If it does, several P0 items are a port rather than brand-new work, and the roadmap should
be adjusted to reflect the reuse opportunity.

### Sources used for this plan

- Ink/Stitch feature set and stitch library
- Wilcom EmbroideryStudio 2026 feature list
- Tajima DG/ML by Pulse product lines
- Pulse, Hatch, Embrilliance, Brother and BERNINA product references
- pyembroidery reference for file I/O and machine-format coverage

Clean-room note: no code from Ink/Stitch (GPL) or other digitizers is used. Formats come from public
documentation, cited above.
