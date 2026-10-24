# Installing embroidery fonts

VectorCraft can sew type in **embroidery fonts**: lettering digitized for embroidery, letter by letter, so
small type stays crisp and big type keeps its satin. It reads two kinds:

- **VectorCraft embroidery fonts**: single `.vcfont` files ([format](formats/vcfont.md)).
- **Ink/Stitch fonts**: the font folders of [Ink/Stitch](https://inkstitch.org/), the free embroidery
  extension for Inkscape. Each is a folder holding a `font.json`, one or more glyph files (`ltr.svg`, or
  an `ltr` folder of them) and a `LICENSE`. Ink/Stitch's fonts are listed at
  <https://inkstitch.org/fonts/font-library/>.

VectorCraft doesn't come with any embroidery fonts. You choose the ones you want and install them yourself.

## Every font has its own licence

Read a font's `LICENSE` before you use it, above all in work you sell. Many embroidery fonts are under the
SIL Open Font License, which allows commercial use. Some aren't: a few are for non-commercial use only, and
some name no licence at all. VectorCraft shows each font's licence in `embroidery.fonts` and keeps it with
the font.

## Install a font

Pick either way. Both are just files and folders, and you can use both at once.

### 1. Point VectorCraft at a folder

Use this when you already keep your fonts somewhere, such as an Ink/Stitch install's fonts folder, or a
folder you downloaded fonts into.

1. Select some type, open **Window › Embroidery**, and find **Embroidery Font** in the Thread section.
2. Click **Add Font Folder…** and choose the folder.

Every font in that folder is listed under **Embroidery Font**: the folder itself if it holds a `font.json`,
otherwise each folder inside it that does. Fonts are read where they are and never copied, so a font you
update there is updated here too. Agents and scripts use `embroidery.fontFolders {folders: [...]}`.

### 2. Put the fonts in VectorCraft's fonts folder

1. In the Embroidery panel, click **Show in Folder** (under Design). This opens VectorCraft's embroidery
   library folder.
2. Open its `fonts` folder (create it if it isn't there yet).
3. Copy font folders (Ink/Stitch) or `.vcfont` files into it.

New fonts show up the next time VectorCraft starts, or straight away with `embroidery.rescanLibraries`.

## Use a font

Select the type, then pick the font from **Embroidery Font**. VectorCraft loads it into the document, which
keeps its own copy, so the design still sews on a computer without the font. To convert an Ink/Stitch font
into a `.vcfont` file, use `embroidery.importInkstitchFont {path, save}`.

How Ink/Stitch fonts are read, and what isn't read yet, is set out in
[formats/inkstitch-fonts.md](formats/inkstitch-fonts.md). VectorCraft sews them with its own stitch types,
so the stitches won't match Ink/Stitch's exactly. Sew a sample before stitching a job.
