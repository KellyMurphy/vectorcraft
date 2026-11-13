# Embroidery UI improvement plan

Status: Phase 1 (UI1.1 to UI1.5) and UI2.1 done; the rest proposed. Written 2026-11-11.

This plan comes from a review of the three embroidery panels: reading their code and driving the
app through each tab with a small design (a filled rectangle and a satin line). It lists what to
change so the embroidery UI is easier to learn and use. The feature plan is in
[embroidery-plan.md](embroidery-plan.md); this file covers only how the existing features are
presented.

The screenshots for the review came from a 0.21.0 release build, older than the code that was
read. Before starting a step, check in the current app that the problem is still there.

## Where the code is

| Panel | File | Opens from |
| --- | --- | --- |
| Embroidery (tabs Stitch, Design, Tools) | `crates/ui-egui/src/panels/embroidery.rs` | Window › Embroidery, the dock icon strip |
| Sew Order | `crates/ui-egui/src/panels/sew_order.rs` | Window › Sew Order, the Design tab |
| Sew-Out Test | `crates/ui-egui/src/panels/sew_out.rs` | Window › Sew-Out Test, the Design tab |
| Stitch pictures | `crates/ui-egui/src/panels/stitch_art.rs` | the pickers |
| Menu | `crates/ui-egui/src/menus/tree.rs` (Object › Embroidery) | |

## Rules for every step

- Panels stay thin: they read engine state and act through `app.run(id, params)`. A step that
  needs new behaviour adds or extends a command, with tests.
- Every new or changed label needs a row in each i18n catalog (`crates/ui-egui/src/i18n/*.tsv`).
- Cover panels with headless egui frame tests, as the existing tests in each panel file do.
- Look at the result in the app at the default dock width and popped out from the icon strip.
- One step per commit, named by its id here (`UI1.1: …`).

## Phase 1: fix what is broken or misleading

Small changes that fix the first run. Do these first.

### UI1.1 Content clipped at the default dock width

Seen: with the panel docked as a tab beside Properties (column 310 points wide), the right edge is
cut off. The third stitch tile, the thread hex field, the Pattern picker's arrow, the Fabric Look
dropdown and Sew Order's Paint Order button are all partly hidden.

Likely cause: `show` sets a minimum width of 300, the label column is 128 (`LABEL_W`), and the
widgets beside it have fixed widths of 150 to 170, which together are wider than the column.

Change:
- Narrow the label column and let the widget beside it fill the remaining width.
- Lay out button rows with wrapping, so a row that doesn't fit breaks instead of clipping.
- Compute the stitch tiles from the width that is actually visible.

Done when: nothing is clipped at 310 points, in all three tabs and in Sew Order, and a frame test
asserts that no widget extends past the panel's right edge.

Status: done 2026-11-11. The Embroidery panel is laid out to 270 points (`PANEL_W`), inside the
290 the default column gives its content. Notes and buttons that shared a row with a field (the
thread's Use Object Color, a satin's Use Stroke, the Load…, Save… and Add to Library buttons, the
hoopings controls) now sit in the column under it and wrap there (`indented`). Dropdowns of names
cut a long name short instead of growing. Sew Order's row of order buttons wraps. Each panel has
a `nothing_is_cut_off_at_the_default_dock_width` test.

### UI1.2 Panel cut off when popped out from the icon strip

Seen: Embroidery is the last icon in the strip, so its pop-out opens near the bottom of the window
and shows only about 150 points: the tabs and the first row of stitch tiles.

Change: find out whether the pop-out's position is general docking behaviour or particular to this
panel. A pop-out that doesn't fit below its icon should move up to fit, and scroll when it is
taller than the window. If that is general docking code, fix it there for every panel.

Done when: the popped-out panel shows its whole Stitch tab, or scrolls, at the default window size.

Status: done 2026-11-11. The cause was general, not the position: a pop-out already moves up to
fit, but it is sized to the content its panel last drew, and a scrolling panel body never reported
more than the body's own height. A pop-out opened on a short panel (Embroidery with nothing
selected) kept that height when the panel grew. The panel body now reports the height its content
needs (`PanelContent::body` in `panel_docking.rs`), which fixes it for every scrolling icon panel
and floating panel. The Embroidery panel's floating size is now 290 × 660, so its pop-out can grow
that tall and scrolls beyond it. Test: `a_flyout_grows_with_its_panels_content`.

### UI1.3 Choosing a stitch shows nothing on the canvas

Seen: Stitch Preview is off by default and its checkbox is at the bottom of the Design tab. A new
user picks Fill and the artwork looks the same.

Change:
- Turn Stitch Preview on the first time a stitch is assigned in a document that had none. The
  setting is a saved preference: don't turn it back on after the user turns it off.
- Move the Stitch Preview and Realistic toggles to the header (UI2.1). Until that exists, put them
  at the top of the Stitch tab.

Done when: in a fresh profile, assigning the first stitch shows stitches on the canvas.

Status: done 2026-11-12. An embroidery command (or a batch) that gives the active document its
first stitches turns Stitch Preview on, once: a saved flag (`stitch_preview_chosen`) records that,
and also that the user turned the preview (or Realistic, or Stitch Density) on or off, after which
it is never turned on for them. The check runs only for `embroidery.*` commands and batches,
because it walks the document. The Stitch Fill tool's stitches don't trigger it: tool actions run
inside the engine, not through the app's command runner. The Stitch Preview and Realistic toggles
moved from the Design tab to the top of the Stitch tab. Test:
`the_first_stitch_turns_stitch_preview_on_once`.

### UI1.4 The Thread row shows no colour

Seen: with no thread set, the row reads "Object color #" with an empty hex field and no chip. A
shape with the default white fill is sewn in white thread, and nothing says so.

Change:
- Always show a chip of the colour that will be sewn, with a note when it comes from the object.
- With a thread chart chosen, show the matched thread's code and name beside the chip.
- Clicking the chip opens a colour picker (and the chart's threads when there is one). Keep the
  hex field as a secondary way in.
- Remove the doubled label: a "Thread" heading directly above a "Thread:" row.

Done when: the row shows the sewn colour for a selection with and without its own thread.

Status: done 2026-11-12. `embroidery.info` reports the selection's `sewn` colour (its own thread,
else the object's colour, as the stitch plan picks it) and `sewnThread`, its nearest thread in the
chosen chart. The row is now "Color:" under the Thread heading: a chip of the sewn colour, the hex
field showing it, and below them the chart thread's code and name and either Use Object Color or
the "Object color" note. Clicking the chip opens a colour picker (a drag sets the thread once,
when it ends) and the chart's threads as chips. Tests: `the_thread_row_shows_the_colour_sewn`,
`the_selection_reports_the_colour_it_is_sewn_in_and_its_chart_thread`.

### UI1.5 A tooltip names a command id

Seen: the Motif picker's tooltip ends "(embroidery.loadMotif adds more)". Border patterns have
Load… and Save… buttons; motifs have none.

Change: give Motif Fill the same Load…, Save… and Add to Library buttons as Pattern Border, and
remove the command id from the tooltip.

Status: done 2026-11-12. Motif Fill has Load… (a `.vcmotif` file; a stitched motif loaded with a
motif fill selected becomes its motif), Save… (any motif, built-in too) and Add to Library (the
document's own motifs, until the library has them). `.vcmotif` files also open from File › Open
and by dropping them on the window, as border patterns do. The tooltip no longer names a command.
Library motifs are not yet listed in the Motif picker, as library border patterns are. Test:
`motifs_load_save_and_go_to_the_library`.

## Phase 2: structure

### UI2.1 A header above the tabs

Always visible, whichever tab is open:
- the Stitch Preview and Realistic toggles;
- one summary line: stitches, size in mm, threads, sewing time;
- a problems badge with the count of findings, which opens the checker;
- the Export button in the document's machine format.

Status: done. The header sits above the tab bar: the two toggles, a summary line ("1062 stitches ·
38.8 × 24.7 mm · 2 threads · 1.4 min", cut short with an ellipsis when the panel is narrow), and a
row with Export and a "⚠ n problems" badge that opens the Design tab, where the checker's list is
until UI2.2. The Stitch tab's "problems found: see Design" link and the Design tab's Export row are
gone. The Design tab still shows its own Stitches, Size and Threads rows; UI2.2 reorganizes them.

### UI2.2 Tabs by task: Stitch, Setup, Check & Export

The Design tab mixes setup, statistics, view toggles, outputs, links to other panels and the
checker. Replace it:

| Tab | Holds |
| --- | --- |
| Stitch | the selection's stitch, thread and settings (as now) |
| Setup | fabric, preview fabric, hoop, machine format, thread chart; and from Sew Order: Machine Speed, Overlap, Shortest Stitch |
| Check & Export | statistics, the thread list, findings with Fix, Export, Worksheet, Mockup, Real Size |

The thread list becomes rows (chip, chart code and name, stitch count), since that is what the
person at the machine needs. Today it is chips with details only in tooltips.

Sew Order keeps only what is about order and travel: Trim Jumps Over, Sew Moves Under, Hide Travel
Under Fills.

Open question: whether Tools stays a fourth tab or moves out (UI2.4).

### UI2.3 Fabric and hoop as first choices

Fabric and Hoop both default to None, yet the fabric fills in density, compensation and underlay
for everything left unset. When the first stitch is assigned and no fabric is set, show a short
prompt in the panel to choose a fabric and a hoop, with a way to dismiss it.

### UI2.4 Tools tab

Seen: one long scroll of six unrelated features (appliqué, auto-digitize, lettering, font editor,
embroidery fonts, library) with every form always open.

Change:
- Make each feature a collapsible section, closed by default, remembering its state.
- Move the Font Editor to a panel of its own: it is a different mode of work, and it already
  changes its content when a font document is open.
- Add the tools' commands to Object › Embroidery where they are missing.

### UI2.5 An Embroidery workspace, and menu entries

- A workspace preset that docks Embroidery and Sew Order, sets the document's units to
  millimetres and turns Stitch Preview on.
- Object › Embroidery gains entries for the Embroidery panel, Sew Order and Export. Today it has
  only the stitch types, Remove Stitches, Auto-Digitize and Garment Mockup.

## Phase 3: Stitch tab

### UI3.1 Stitch picker

Seen: ten tiles are always expanded and push the settings below the fold. Auto and Satin Shape
have the same picture, Running and Bean nearly the same, and Manual looks like a zigzag.

Change:
- Group the tiles: Auto on its own, then Lines (Running, Bean, Zigzag, Satin Column, Pattern
  Border, Manual), then Areas (Fill, Motif Fill, Satin Shape).
- Dim the kinds that don't suit the selection (an open path can't be filled), with a tooltip
  saying why.
- Redraw the pictures on a sample shape, so each kind looks different from the others.
- Consider collapsing the picker to the chosen kind's row once a stitch is set, with a control to
  open it again.

### UI3.2 Say what is selected

At the top of the Stitch tab: what is selected, its size in mm, its stitch and its stitch count,
for example "1 path · 38.8 × 24.7 mm · Fill · 1,062 stitches". The size matters because documents
are usually in points while every stitch setting is in mm.

### UI3.3 Labels

| Now | Problem | Proposed |
| --- | --- | --- |
| Density: 0.4 mm | a spacing; smaller is denser | Spacing (as Zigzag already says) |
| Bean Outline Under | opaque | Outline strokes thinner than |
| Fill Over | opaque | Fill strokes wider than |
| Fabric Look | reads like Fabric | Preview fabric, beside Realistic |
| Sew Moves Under | opaque | to decide |
| Paint Order (Sew Order) | unclear as an action | Reset Order |

Renaming a label changes its i18n key in every catalog. Command parameter names stay as they are.

### UI3.4 Buttons that can't run

Appliqué, Cut File, Auto-Digitize, Extract Border Pattern and Extract Motif are always enabled and
report failure only in the status bar. Disable each when the selection doesn't qualify, with a
tooltip saying what it needs (each command's `enabled` should already know).

### UI3.5 Findings where the object is

With findings on the selected object, the Stitch tab shows only a link to the Design tab. Show the
selected object's findings inline, with their Fix menus. Clicking a finding in the checker list
should also bring the object into view, not only select it.

## Phase 4: Sew Order

### UI4.1 One transport row

Seen: two rows use the same double-arrow icons, one stepping by object and one by thread. Replace
them with one row: play and pause, a scrubber over the stitches with a tick at each colour change,
and the sewing time. Stepping by object happens in the list.

### UI4.2 Rows that tell objects apart

Seen: every row is named "Path". Lead each row with its stitch type and thread; show the object's
name second, and only when the user has named it.

### UI4.3 Play on the canvas

The player draws in a small picture in the panel. Play on the canvas instead (or as well), so the
sewing is seen on the artwork at the size the user is working at.

## Order of work

1. Phase 1 (UI1.1 to UI1.5): small, and fixes the first run.
2. UI2.1 header, then UI2.2 tabs with UI2.3.
3. UI3.1 to UI3.5.
4. UI2.4, UI2.5 and Phase 4, in any order.

## Not reviewed

- The Sew-Out Test panel's form was read but not exercised in the app. Entering measurements for
  the compensation sheet is feature step E10.1b, not part of this plan.
- The font editor's document mode, the Trace Font progress window, the Garment Mockup dialog and
  the Stitch Fill, Stitch Angle and start-point tools.
- The web build.

## Revision history

- 2026-11-11: first version, from the panel review.
- 2026-11-11: UI1.1 done.
- 2026-11-11: UI1.2 done.
- 2026-11-12: UI1.3, UI1.4 and UI1.5 done.
- 2026-11-12: UI2.1 done.
