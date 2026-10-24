# Ink/Stitch font folders (read only)

VectorCraft reads [Ink/Stitch](https://inkstitch.org/) embroidery font folders, which users install
themselves (see [embroidery-fonts.md](../embroidery-fonts.md)), and converts them to
[`.vcfont`](vcfont.md) embroidery fonts. These notes are VectorCraft's own, written from Ink/Stitch's public
documentation only:

- the font creation tutorial, <https://inkstitch.org/tutorials/font-creation/>;
- the XML namespace page, <https://inkstitch.org/namespace/>.

Nothing here comes from Ink/Stitch's source code, which is GPL, or from running Ink/Stitch. Its fonts are
never shipped with VectorCraft. Where the documentation leaves something open, the reader accepts each
reasonable reading and keeps the one the font itself bears out, as noted below. What can't be mapped is
listed in the import report (`embroidery.importInkstitchFont`) rather than guessed.

## The folder

| File | Read as |
|---|---|
| `font.json` | The font's metrics (below). Required. |
| `ltr.svg`, or the `.svg` files of an `ltr` folder (in name order) | The left-to-right glyphs. Up to 64 files of up to 64 MB each. |
| `LICENSE` (also `LICENSE.txt`, `LICENSE.md`) | The font's `license`: `OFL-1.1` or `CC-BY-NC-SA-4.0` when the text names that licence, else the text (up to 8 KB). |
| `rtl.svg`, `ttb.svg`, `btt.svg` | Right-to-left and vertical variants: not read yet. |
| `preview.png` | Not read. |

## `font.json`

| Member | Read as |
|---|---|
| `name` | The font's name; the folder's name when absent. |
| `description` | The font's description. |
| `size` | The height of the reference letter in glyph units, taken as the cap height. One em is 10/7 of it. When absent: the top of `M` or `H`. |
| `horiz_adv_x_default` | Every glyph's advance, unless it has its own. 0 or absent: each glyph's own width. |
| `horiz_adv_x` | An object of characters → advance. |
| `hkern` | Kerning pairs. A positive value sets the pair closer, so it is stored negated. The documentation doesn't give the layout, so both of these are read: two-character keys (`"AV": 20`), or a first character holding the second's (`"A": {"V": 20}`). |

Other members (resize range, sortable) are not read yet.

## Glyph files

- **Glyphs** are SVG `<g>` layers labelled `GlyphLayer-<characters>`. Several characters make a ligature.
  A name with a `.` after its first character (a variant for a typographic feature) and names longer than
  8 characters (symbol names) are skipped, and the report says so. A second layer for the same characters
  is skipped too.
- **The baseline** is the guide labelled `baseline`. The documentation doesn't say which way its position
  is measured, so both readings are tried: from the top, and from the page's foot (its `viewBox` height
  minus the position). The one the glyphs stand on wins: the median distance of their lowest points to it
  is the smaller. With no guide, the baseline is where most glyphs stand. The report says which was used.
- **Paths** in a layer (not in `<defs>`) are its objects, with the transforms of the layer and every group
  between applied. Glyph units become font units with y up from the baseline.
- **Visual commands** (`<use>` of an `#inkstitch_…` symbol: stop, trim, fill start and end, ignore) are
  counted but not mapped yet.

## Stitch settings

Attributes in the `http://inkstitch.org/namespace` namespace, mapped by meaning onto VectorCraft's stitch
types. Lengths stay in mm.

| Ink/Stitch | VectorCraft |
|---|---|
| `satin_column` (true) | Satin Column between rails (the path's two longest subpaths are the rails, the others rungs) |
| `zigzag_spacing_mm` (satin) | `spacing` |
| `pull_compensation_mm` | `pull` |
| `zigzag_underlay`, `contour_underlay`, `center_walk_underlay` | underlay `zigzag`, `edge` or `center` (the first one set, in that order); none set: none |
| `manual_stitch` (true) | Manual Stitch |
| a filled path (`auto_fill` not false, or no stroke) | Fill |
| `angle` | `angle` |
| `row_spacing_mm` | `spacing` |
| `max_stitch_length_mm` | `length` |
| `staggers` | `stagger` (2–8) |
| `fill_underlay` | underlay `fill` (true) or none (false); absent: auto |
| `expand_mm` | `pull` (when above 0) |
| a stroked path | Running Stitch |
| `running_stitch_length_mm` | `length` |
| `bean_stitch_repeats` (n ≥ 1) | Bean Stitch with `passes` 2n + 1 (3–9) |
| `repeats` | accepted, not mapped |

Any other attribute is kept out of the font and listed in the report's `unmapped`. Such an object sews with
VectorCraft's defaults for that setting. Fill and stroke colours aren't used: the letters sew in the type's
colour.
