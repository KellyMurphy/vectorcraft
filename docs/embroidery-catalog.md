# Embroidery feature catalog

Status: 62 features checked against the code on 2026-11-06. Done 36, partly done 12, missing 14 (estimated, by
reading the code and the guide, not by sewing: nothing is marked done on fabric until the sew-out gates pass).

This is the feature catalog from the embroidery research (Kelly, 2026-10-16) that
[embroidery.md › Development plan](embroidery.md#development-plan-for-future-steps) summarizes and
[embroidery-plan.md](embroidery-plan.md) turns into steps. Each feature is something at least one surveyed
application ships, found in that application's public pages (sources below). Vendor names are given only
to say where a feature is described; nothing is copied from any of them (the clean-room rules in
[CLAUDE.md](../CLAUDE.md) apply).

- **Priority:** P0 = needed to digitize and sew a typical design end to end; P1 = expected by serious
  hobbyists and small shops; P2 = differentiators and pro production features.
- **Status:** **Done**, **Partly** (what's missing is named) or **Missing**. "Plan" names the step in
  [embroidery-plan.md](embroidery-plan.md) that covers what's left.

## Summary

| Priority | Features | Done | Partly | Missing |
|---|---|---|---|---|
| P0 | 18 | 17 | 1 | 0 |
| P1 | 26 | 18 | 5 | 3 |
| P2 | 18 | 1 | 6 | 11 |
| **All** | **62** | **36** | **12** | **14** |

| Area | Features | Done | Partly | Missing |
|---|---|---|---|---|
| Drawing and vector editing | 11 | 9 | 2 | 0 |
| Stitch types and fills | 18 | 6 | 2 | 10 |
| Lettering and monograms | 8 | 4 | 2 | 2 |
| Quality, sequencing and automation | 12 | 10 | 1 | 1 |
| Colour, preview, output and production | 13 | 7 | 5 | 1 |

**P0 left:** one partly-done P0 feature: the XXX format (E9.3 added VP3 and U01; XXX's only public
description is a stub that leaves out its header, stitch bytes and colour table). Run styles (E11.1) and split satin (E11.2) landed.

**Catalog gaps (E11).** The check found features, or pieces of them, that no plan step covered; they are
now [plan phase E11](embroidery-plan.md#catalog-gaps--e11): run styles and split satin (P0, first), then
multi-hoop, batch export, deferred stitch generation and centreline digitizing (P1; centreline done,
before G1), and the P2 items after G2. Only blending thread colours across an area has no step yet.

## Drawing and vector editing

| Feature | What it does | Seen in | Priority | Status | Where / plan |
|---|---|---|---|---|---|
| Bezier pen, shapes, node editing | Draw and reshape objects with true curves, not point chains | All | P0 | **Done** | The vector editor |
| Boolean path operations | Union, cut, intersect and weld shapes into new objects | Embrilliance L3, BERNINA 9 weld | P0 | **Done** | Pathfinder, Shape Builder |
| Vector import | Bring in SVG, AI, EMF and PDF art with colours and nodes intact | Pulse, Wilcom (CorelDRAW), Embrilliance | P0 | **Done** | The editor's importers |
| Image import and autotrace | Trace bitmaps into editable vector shapes | Pulse Autotrace and Image Vectorizer | P1 | **Done** | Image Trace; `embroidery.digitize` (logo mode) |
| Snap-to-object and precision grids | Align and place points exactly against other objects | Wilcom 2026 | P1 | **Done** | Smart guides, snapping, grids |
| Offsets, borders, inflate/deflate | Generate outlines and offset copies of any shape | Wilcom Advanced Offset, Embrilliance | P1 | **Done** | Offset Path; fill satin border, Auto border |
| Slice tools | Split an object along a line or curve into two objects | Pulse rectangle and curved slice | P1 | **Done** | Knife, Divide, Split, Split into Grid |
| Styles and presets | Save a set of stitch properties and apply it with one click | Embrilliance QuickStyles, Wilcom Styles | P1 | **Done** | E11.3: stitch styles (`.vcstyle`, Stitch Styles menu, three starters), and graphic styles carry stitch settings |
| Delayed stitch generation | Recalculate stitches only after a shape edit finishes | Embrilliance L3 | P1 | **Done** | The stitch cache regenerates only changed objects; E11.6: Preferences › Performance › Regenerate Stitches: When the Edit Ends waits for a drag to end |
| Mirror merge, circle and matrix copy | Make symmetric, radial and grid repeats of objects | Wilcom, Brother PE-DESIGN 11 | P2 | **Partly** | Repeat (mirror, radial, grid) in the editor; not checked that live Repeat objects stitch (checked as part of E11.4) |
| Continue digitizing | Extend an existing closed object instead of starting a new one | Wilcom 2026, Hatch | P2 | **Partly** | The pen continues open paths; closed objects can't be extended. Plan: E11.12 |

## Stitch types and fills

| Feature | What it does | Seen in | Priority | Status | Where / plan |
|---|---|---|---|---|---|
| Run variants | Single, bean, 3- and 5-pass, chain, stem, backstitch, sashiko | Embrilliance (7 run styles), Pulse | P0 | **Done** | Running with a style: single, chain, stem, backstitch, sashiko (E11.1); Bean with odd passes (3, 5…) |
| Manual stitch | Place each needle penetration by hand | All | P0 | **Done** | Manual stitch |
| Satin column | Two-rail satin with stitch-angle inclines, auto-incline and split stitches for wide columns | All; Ink/Stitch adds E-, S- and zigzag satin | P0 | **Done** | Satin between rails, Satin Shape, Zigzag, stitch angle lines (E8.9); wide stitches split, staggered or at random, columns up to 30 mm (E11.2) |
| Satin border (steil) | A satin of set width that follows a single path | Pulse Steil Tool, Embrilliance | P0 | **Done** | Satin on a path; fill satin border |
| Tatami fill with holes | Row fill that respects inner holes, with stitch length, spacing and angle | All | P0 | **Done** | Fill (tatami 2–8, brick), routing (E3.5) |
| Programmable and carved fill editor | Edit the penetration pattern of a fill, or carve lines through it | Pulse Carved Fill Pattern Editor, Wilcom Carving Stamp | P1 | **Partly** | Built-in stagger patterns and carved motifs; no editor for the penetration pattern |
| Motif run and motif fill | Repeat a small user motif along a line or across an area | Embrilliance, Wilcom, Brother decorative fill | P1 | **Done** | Pattern Border (`.vcborder`), Motif Fill (`.vcmotif`), extraction of both (E8.4) |
| Contour, circular, spiral and radial fills | Fills that echo the outline or radiate from a centre | Ink/Stitch, Brother, Pulse | P1 | **Missing** | E8.3a–c (unblocked: G1's test sheet passed) |
| Guided and curved fill | Fill rows bend to follow a guide line you draw | Ink/Stitch Guided Fill, Embrilliance Curved Fill | P1 | **Missing** | E8.3d (unblocked) |
| Meander and stipple | Quilting-style wandering line that fills an area | Ink/Stitch, Embrilliance | P1 | **Missing** | E8.3e (unblocked) |
| Gradient density and colour blends | Fade density, or blend two or more thread colours across an area | Wilcom Multi Blend, Pulse Auto Colour Blend, Ink/Stitch linear gradient | P2 | **Missing** | E9.1 (cascade/gradient density); colour blends: not planned yet |
| Specialty fills | Fractal, fur, cascade and wave fills | Pulse Maestro | P2 | **Missing** | E9.1 |
| Cross stitch | Even X stitches on a grid, like hand cross-stitch | Ink/Stitch 3.3, Embird, Pulse | P2 | **Missing** | E9.1 |
| Ripple stitch | Repeating echo rings or rows generated from a line | Ink/Stitch | P2 | **Missing** | E9.1 |
| Embossing and line carving | Engrave lines or motifs into a fill by changing penetrations | Embrilliance, Pulse Line and Region Carving | P2 | **Partly** | Carved motifs in fills; no carving along a drawn line. Plan: E11.12 |
| Tartan fill | Plaid stripes built from fill layers | Ink/Stitch | P2 | **Missing** | E9.1 |
| Hand-stitch effects | Candlewicking, French knots, fringe, eyelets, jagged edge | Embrilliance, Wilcom, Pulse | P2 | **Missing** | E11.13 |
| Redwork automation | Single-colour line art sewn in a continuous double-run path | Wilcom 2026, Ink/Stitch | P2 | **Missing** | Branching (E7.5) is the base for it. Plan: E11.8 |

## Lettering and monograms

| Feature | What it does | Seen in | Priority | Status | Where / plan |
|---|---|---|---|---|---|
| TrueType/OpenType to stitches | Convert any installed font into satin or fill lettering | Pulse, Wilcom, Embird Font Engine | P0 | **Done** | Live type, Auto per letter, pen-stroke satin |
| Kerning and spacing | Auto and visual kerning; letter, word and line spacing | Pulse (Visual Kerning), Ink/Stitch 3.3 | P0 | **Done** | Font kerning pairs, tracking, manual kerning, leading; `embroidery.font.kern` |
| Pre-digitized font library and font creator | Hand-tuned fonts that scale cleanly, plus a tool to make and publish your own | All; Pulse Font Creator, Embrilliance font publishing | P1 | **Partly** | `.vcfont`, font editor, tracer, Ink/Stitch import (E8.1). No starter font published yet: E8.1d |
| Text on path, envelopes, circle text | Lay text along curves, warp it, or set it in circles and spirals | Ink/Stitch, Embrilliance, Pulse | P1 | **Done** | `embroidery.textArc`, envelopes and warps (E8.2) |
| Monograms | Templates for two- and three-letter monograms with frames | Hatch, Embrilliance, Wilcom | P1 | **Done** | `embroidery.monogram` (E8.2) |
| Small-lettering fonts | Fonts built to stay legible below about 6 mm | Wilcom, Pulse | P2 | **Partly** | Auto sews small type as narrow satin; the checker warns on small holes. No hand-made small fonts |
| Batch names | Make one design per name from a list (team names, name drop) | Wilcom Team Names, Ink/Stitch batch lettering | P2 | **Missing** | E11.9 |
| Keyboard design collections | Map any set of designs to keys and type with them | Hatch, BERNINA 9 | P2 | **Missing** | E11.10 |

## Quality, sequencing and automation

| Feature | What it does | Seen in | Priority | Status | Where / plan |
|---|---|---|---|---|---|
| Automatic underlay | Centre run, edge run, zigzag and tatami underlay chosen per object | Pulse (4 types), Wilcom | P0 | **Done** | E7.1 |
| Pull and push compensation | Widen or shorten objects to offset thread pull, set per axis | Pulse, Wilcom, Ink/Stitch 3.3 | P0 | **Done** | E7.2; grain- and curve-aware compensation: E10 |
| Trims, ties and jumps | Auto trims, tie-in/tie-off, minimum jump length, start and stop points | mySewnet, Pulse | P0 | **Done** | Travel settings, Trim Before/After, tie-ins and tie-offs, start handles |
| Branching and auto-sequencing | Order objects and route travel under later fills to cut trims | Wilcom, BERNINA 9, Pulse | P1 | **Done** | E7.5: `embroidery.optimizeOrder`, branching, hidden travel |
| Fabric profiles | Pick a fabric and get density, underlay and compensation to match | Hatch, Wilcom | P1 | **Done** | E7.3; woven and knit starters checked on fabric (G1 test sheet, 2026-11-09), measured profiles in E10.1 |
| Density advisor | Heat map of stitches per mm², with one-click reduction | mySewnet | P1 | **Done** | E8.7: View › Stitch Density, `embroidery.fix` |
| Small-stitch filter and corner control | Remove tiny stitches; short stitches at sharp satin corners | Embrilliance, Pulse Custom Cornering | P1 | **Done** | E7.6 shortest stitch; fanned satin corners |
| Design checker | List problems per object (gaps, long stitches, bad shapes) | Ink/Stitch Troubleshoot Objects | P1 | **Done** | E7.7: `embroidery.check`, Check list |
| Auto-digitize from image | Click-to-fill, click-to-outline, centreline and full auto, after colour reduction | Hatch (10 tools), Chroma, Pulse AutoDigitizer | P1 | **Done** | Full auto, click-to-fill, click-to-outline in running stitch or satin (E8.6, E11.7), centreline by click (Shift-click) and in full auto (`lines: centreline`; E11.7) |
| Stitch and runtime estimator | Stitch count, thread use, run time and a price quote | Pulse Quotation Estimator, Wilcom Estimator | P1 | **Partly** | Stitch count, sewing time and thread length on the worksheet. Bobbin use and price: E9.4 |
| Photo stitch | Turn photos into stitched shading | Hatch PhotoStitch, Embird Sfumato, Brother, Chroma | P2 | **Missing** | E11.14 |
| Overlap removal | Drop stitches hidden under later objects | Pulse Remove Overlap Stitches | P2 | **Done** | E7.4 |

## Colour, preview, output and production

| Feature | What it does | Seen in | Priority | Status | Where / plan |
|---|---|---|---|---|---|
| Thread charts and matching | Palettes from major thread makers, nearest-match recolouring, custom palettes | Ink/Stitch (60+ makers), Hatch | P0 | **Done** | E7.10: `.vcthreads`, `embroidery.matchThreads`. Ships only charts published for free use |
| Colour sequence list and recolour | See and reorder colour stops; swap a colour across the design | Wilcom Color Object List, all | P0 | **Done** | Sew Order panel, `embroidery.sortByThread` |
| Stitch player | Replay the stitch-out at variable speed, stop by stop | Wilcom, Ink/Stitch simulator, Embrilliance | P0 | **Done** | E7.9 |
| Machine file formats | Read and write DST, PES, EXP, JEF, VP3, XXX and U01 at minimum | pyembroidery, all | P0 | **Partly** | DST, PES, JEF, EXP read and write (E7.12); VP3 and U01 read and write (E9.3). XXX: not yet, its public description is a stub (no header layout, stitch encoding or colour table) |
| Production worksheet and templates | PDF with colour stops, thread names, stitch count and run time; 1:1 print with centre marks | Ink/Stitch, Embrilliance, Pulse | P0 | **Done** | E7.11: `embroidery.worksheet` |
| Realistic render and mockups | Thread-shaded view, fabric backgrounds, preview on 3D garments, real-size view | Chroma, Pulse Ambassador, mySewnet | P1 | **Partly** | View › Realistic Stitches draws shaded thread (sheen by stitch angle, shadows) on seven code-drawn fabric textures in any colour (E9.2a); Object › Embroidery › Garment Mockup… shows the design at real size on a T-shirt, polo, hoodie, cap, tote or towel drawn in code, at the usual placements (E9.2b); View › Real Size shows the design at its true size on a screen calibrated with View › Calibrate Real Size… (E9.2c). Missing: 3D garments (the mockups are flat front views) |
| Import, edit and batch export | Open stitch files for stitch-level edits; export many formats at once | Ink/Stitch, Embird, Wilcom Export Docker | P1 | **Done** | Machine files open as editable manual-stitch paths; one design per artboard on export; E11.5: several formats at once (`document.export {formats}`, Export As › Also Export) |
| Hoops and multi-hoop | Hoop library, rotate hoop, split large designs with alignment and basting stitches | BERNINA 9, mySewnet, Embird | P1 | **Done** | Hoop library and fit check (E7.8); Rotate in Hoop and Split into Hoopings with alignment marks and basting box, a file per hooping (E11.4) |
| Appliqué | Placement, tack-down and cover steps, with or without holes, plus cutter files (SVG) | All; BERNINA 9, Embrilliance | P1 | **Done** | E8.8: `embroidery.applique`, nested cut files |
| Extensions and user content | Plugins, plus user folders for patterns, motifs, fonts and palettes | Ink/Stitch extensions, Embrilliance motif publishing | P1 | **Partly** | Library folders (E8.5). Plug-ins for stitch generators: E9.6 |
| Specialty techniques | CutWork, 3D foam/puff, faux-merrow patch edges, sequins, beading, chenille | BERNINA, Embrilliance, Wilcom, Pulse | P2 | **Partly** | Puff with perforation (E7.14). The rest: E11.15 |
| Design library | Catalogue with thumbnails and search; file-browser previews | Wilcom Library, Pulse Librarian, Embird Iconizer | P2 | **Missing** | E11.11 |
| Machine transfer and cloud | Wi-Fi send to machine; browser editor and web API | Hatch EmbroideryConnect, Brother, Wilcom Workspace | P2 | **Partly** | The web app and MCP give a browser editor and an API. Machine transfer: E9.5 |

## Applications surveyed

| Application | Segment | Known for |
|---|---|---|
| [Wilcom EmbroideryStudio 2026](https://www.swingdesign.com/collections/wilcom-embroiderystudio-2026-full-license-software/products/wilcom-embroiderystudio-designing-digitizing-software-2026-full-license) | Professional | Branching and auto-sequencing, blends, motif and carving effects, sequins, chenille, team names |
| [Tajima DG/ML by Pulse](https://embroidery.pulsemicro.com/dg15-levels.html) | Professional | Programmed and carved fills, fractal, wave and fur fills, auto sequencing, a quotation estimator |
| [Hatch Embroidery 3](https://hatchembroidery.com/features/auto-digitizing) | Prosumer | Auto-digitizing tools, thread matching, appliqué, keyboard fonts |
| [Embrilliance StitchArtist L1 to L3](https://embrilliance.com/product/stitchartist-level-3) | Prosumer | Run and satin styles, motif and emboss publishing, path operations, delayed stitch generation |
| [Brother PE-DESIGN 11](https://support.brother.com/g/s/hf/ped_info/ped11/new_features/en/ped11_newfeatures.html) | Home | Decorative fills, spiral and round fills, circle and matrix copy, wireless transfer |
| [BERNINA Embroidery Software 9](https://www.bernina.com/en-AU/Campaigns-AU/V9/Difference-between-Embroidery-Software-8-and-9) | Home and pro | Branching, weld, rotate hoop, CutWork, appliqué with holes |
| [Ricoma Chroma](https://eu.ricoma.com/en-eu/products/chroma-digitizing-software) | Commercial entry | Realistic view, 3D garment preview, photo-to-stitch |
| [Embird + plugins](https://www.embird.net/sw/embird/features.htm) | Home | File manager and converter, photo stitch, cross stitch, font engine |
| [mySewnet](https://softwarehelp.mysewnet.com/MSW/120/StitchEditor/Topics/Density_Advisor.htm) | Home | Density Advisor heat map, stitch optimizer, auto trims |
| [Ink/Stitch 3.3](https://inkstitch.org/features) | Open source (GPL; nothing is copied from it) | Guided, meander, contour, circular, tartan and cross-stitch fills; ripple; lettering on a path; worksheets |

[pyembroidery](https://github.com/EmbroidePy/pyembroidery) is a file I/O reference, used only as an
independent reader in tests.

## Revision history

- 2026-11-01: added from the 2026-10-16 research notes, with each feature's status against the code.
- 2026-11-01: features no step covered linked to the new plan phase E11.
- 2026-11-01: run variants done (E11.1); click-to-outline was already there (E8.6b), only centreline is missing.
- 2026-11-01: satin column done: split satin (E11.2).
- 2026-11-02: styles and presets done: stitch styles (E11.3).
- 2026-11-04: auto-digitize from image done: satin outlines and centreline digitizing (E11.7).
- 2026-11-06: realistic render and mockups: shaded thread on fabric textures (E9.2a), garment mockups (E9.2b), real-size view with screen calibration (E9.2c); still partly done (no 3D garments).
- 2026-11-06: machine file formats: VP3 and U01 read and write (E9.3); still partly done, XXX has no usable public description.
