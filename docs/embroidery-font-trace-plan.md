# Embroidery font tracing plan

Status: proposed. Written 2026-11-12.

Trace Font (`embroidery.font.trace`) turns an installed outline font into an embroidery font: each
letter becomes satin columns drawn as two rails. It gets many letters wrong. This plan records
what was measured, why it happens, and how to rebuild the tracer so it works from the letter's
outline instead of from pixels. The feature plan is in [embroidery-plan.md](embroidery-plan.md)
(E8.1, embroidery fonts); this file covers only the quality of tracing.

The same code sews Satin Shape and live type that has no embroidery font
(`generate::satin_shape_angled`), so most steps here improve those too.

## Where the code is

| Stage | File | What it does |
| --- | --- | --- |
| Centre line | `crates/embroidery/src/skeleton.rs` | Rasterizes the shape, thins it to a one-pixel skeleton (Zhang–Suen), cuts it into pieces at junctions, prunes spurs |
| Pen strokes | `crates/embroidery/src/penstrokes.rs` | Rejoins pieces through junctions by angle, splits at corners (print), orders them |
| Satin | `pen_parts`, `satin_pen`, `pen_suits` in `crates/embroidery/src/generate.rs` | Casts rays sideways from the centre line to the outline; measures coverage |
| Glyph objects | `trace` in `crates/embroidery/src/font_doc.rs` | Ray hits become rails, or the whole outline is kept as Auto |
| Rails | `crates/embroidery/src/rails.rs` | Sews a satin between two rails; supports rungs |
| Command | `crates/engine/src/cmd/embroidery_font_editor.rs` | `embroidery.font.trace`, `trace_letter` and its cache |
| Progress window | `crates/ui-egui/src/font_trace.rs` | |

## What was measured (2026-11-12)

Letters were traced with `embroidery.font.trace` in a release build and looked at with
`embroidery.preview`, one artboard per letter. Fonts were ones installed on the test machine.

| Font | Result |
| --- | --- |
| Arial, A–Z a–z 0–9 (62) | 10 fall back to a plain outline, among them W, R, D, d, p |
| Arial N | the diagonal is a short stub and a bar, with gaps at both stems |
| Arial s | 4 separate satin pieces with gaps between them |
| Arial A | the legs kink where the crossbar joins |
| Times New Roman H | 11 satin objects: each serif is its own small column |
| Times New Roman m, K | 13 and 12 objects |
| Impact, 6 letters tried | all 6 fall back to outline |
| Segoe Script, 5 letters tried | 4 fall back; the k has a knot at its junction |

The ignored test `report_pen_strokes` (`crates/embroidery/src/tests.rs`) prints, per letter, the
number of pen strokes, whether pen satin is used and the bare share. It runs at 72 pt, not at the
editor's em, so it passes letters that Trace Font rejects: at 72 pt, 42 of Impact's 65 letters
pass. Run it before and after each step:

```
EMB_FONT="Arial,Times New Roman" EMB_TEXT="ABC…" cargo test -p vectorcraft-embroidery report_pen_strokes -- --ignored --nocapture
```

Trace Font was not tried on the web build, on non-Latin letters or on accented letters.

## Causes

Each was found by reading the code; the first two were also confirmed by the measurements above.

1. **Suitability is judged at the wrong size.** `font_doc::trace` works at the editor's em
   (`EM_MM`, 100 mm) and compares against limits in absolute mm: the longest stitch
   (`MAX_STITCH`, 12 mm), the tuck under a crossing stroke (0.4 mm), the tip inset (0.15 mm). A
   heavy font's stems are wider than 12 mm at that size, so they are clipped, the letter comes out
   bare and is rejected.
2. **Smooth curves are cut as corners.** `split_corners` measures the turn of the centre line over
   two stroke widths either side. A tight curve (s, a, e, 2, 3, 5) turns more than 55° over that
   distance.
3. **Topology is thrown away and guessed again.** `skeleton::strokes` knows which pieces meet at
   which node but returns only the pieces. `penstrokes::junctions` regroups piece ends by distance
   (1.5 × the half-width, from the first end found, not transitively). Wide junctions are missed
   and unrelated ends that are close together are joined.
4. **The skeleton is distorted at junctions.** Thinning bends the centre line toward a junction
   and splits a crossing into two three-way joins with a short bridge. `leaving` measures
   direction inside that zone, so strokes that go straight on (the diagonals of N, K, M, W, X)
   don't pair.
5. **Serifs become strokes.** A spur is pruned only when shorter than 1.6 × the radius where it
   joins (`SPUR`); serifs are longer.
6. **Stroke contrast is flattened.** `keep_own_width` clamps half-widths above 1.12 × the stroke's
   median. It is meant for the swell inside a junction, but it also clips the thick parts of a
   modulated stroke (Times, scripts), which leaves fabric bare and triggers the fallback.
7. **Rails are ray samples.** A rail is the row of ray hits, one per mm (`RAIL_STEP`): jagged, not
   the outline's curves, and without rungs, so stitch direction through curves and joins is not
   controlled. Rays (`skeleton::cast`) also stop at interior edges when a font's contours overlap.
8. **The fallback is all or nothing.** One bad junction sends the whole letter to outline, so a
   traced font mixes satin letters with filled ones.
9. **The check measures coverage only.** `SatinQuality` has no stroke count, gap or jump measure,
   so Times H passes with 11 pieces.
10. **Order ignores travel.** `order` sorts by class (downstrokes, slanted strokes, crossbars), so
    a letter has a jump between most strokes.
11. **Script is told by the font's name.** `hand_of` looks for words such as "script" and "brush";
    other script faces are traced as print.
12. **Parts without a stroke are lost.** `satin_pen` fills them (`uncovered_parts`, a full stop,
    some dots), and that fill counts toward suitability, but `trace` emits only the rails.
    Not confirmed on a letter: Arial's i kept its dot.

## Phase 1: fixes to the current tracer

Small changes that help now and give a baseline for Phase 2. Each has a test that fails before it.

### FT1.1 Judge a letter at sewing size

Run the suitability check and the limits in mm at the size the font will be sewn, not at 100 mm
per em: the middle of the font's `sizes` range when set, else a default cap height (propose
10 mm). Rails are still written in font units.

Done when: Impact's letters are no longer all rejected, and a test traces a synthetic heavy letter
(strokes 15% of the em wide) to rails.

### FT1.2 Don't cut smooth curves

In `split_corners`, break only where the outline itself has a corner near the turn (a vertex
whose two sides differ in direction by more than a threshold), not on centre-line turn alone.

Done when: a synthetic S (two half rings joined) is one stroke in print, and an L still breaks.

### FT1.3 Keep the skeleton's junctions

`skeleton::strokes` returns each piece's node at either end. `pen_strokes` uses those nodes
instead of `junctions`, merges nodes closer together than the stroke is wide (a crossing becomes
one four-way junction), and measures `leaving` from beyond the junction's radius.

Done when: a synthetic N and X each trace to their expected strokes (3 and 2) with no gap at the
joins.

### FT1.4 Emit the parts without strokes

`trace` adds a fill object for each part `uncovered_parts` finds.

Done when: a synthetic letter with a detached round dot keeps the dot.

## Phase 2: trace from the outline

The rebuild. A letter's rails become pieces of its own outline.

### FT2.1 Clean outline

Union the glyph's contours (non-zero) into simple, non-overlapping paths, keeping curves. Use the
existing path boolean code if it is reachable from the embroidery crate under the layering rules;
check with `cargo xtask layers`.

### FT2.2 Stroke graph by triangulation

Triangulate the flattened outline with a constrained Delaunay triangulation. Triangles inside the
shape with one inside edge are tips, with two are stroke body, with three are junctions. Chains of
body triangles are strokes; their two sides are stretches of the outline, and their width is known
at every point. This replaces rasterizing, thinning, spur pruning and ray casting, and has no
resolution limit.

- Needs a triangulation crate. None is in `Cargo.lock`. Use `spade` (checked 2026-11-12 at
  2.15.1, released 2026-03-24):
  - Licence `MIT OR Apache-2.0`, on the allowlist. Its one dependency new to this workspace,
    `robust` 1.2.0 (exact geometric predicates), is also `MIT OR Apache-2.0`. Its others
    (`hashbrown`, `num-traits`, `smallvec`) are already in `Cargo.lock`.
  - `#![forbid(unsafe_code)]`, `no_std` with `alloc`, no build script, no file, thread or clock
    use in the library. Nothing in it should stop a wasm build, but one was not run: confirm
    with `cargo xtask wasm` when it is added.
  - It panics when a constraint edge crosses another (`add_constraint`, `add_constraint_edge`,
    `bulk_load_cdt`). Use only `try_bulk_load_cdt`, `try_add_constraint` or
    `add_constraint_and_split`, after FT2.1 has removed self-intersections, and handle
    `InsertionError` (a NaN or out-of-range coordinate).
- Input is untrusted: cap the vertex count, check coordinates are finite, and return an error
  rather than panic on degenerate outlines. Extend the property tests, and keep
  `vectorcraft_engine::guard` in mind as the last line only.
- Must build for wasm (`cargo xtask wasm`).

Done when: the graph of synthetic T, L, X, O and a bar has the expected tips, strokes and
junctions, and a property test runs it over random polygons.

### FT2.3 Junctions and corners

- Merge junction triangles that touch or lie within a stroke's width of each other.
- At each junction, pair the strokes that go straight on, using direction measured outside the
  junction and similar width.
- Carry a stroke across a junction by joining the concave outline corners on either side of it.
- Split a stroke only at a true corner of the outline (print); never in script.

### FT2.4 Rails and rungs

- Cut each rail from the outline as curves, in font units.
- Close a stroke's end across a junction along the line between the concave corners.
- Add rungs at junctions and through curves so the stitches turn evenly. `rails.rs` already sews
  rungs.

Done when: a traced O is two closed rails that match its outline, and the stitches across an L's
corner turn without gaps.

### FT2.5 Serifs and tips

A tip branch that is short for the width of the stroke it joins is a serif: it widens that
stroke's end instead of becoming a column. A font-level option drops serifs, for small sizes.

Done when: a synthetic serifed H traces to 3 objects.

### FT2.6 Decide per stroke

For each stroke, at the sizes the font is for: satin when its width is in the satin range, split
satin or fill when wider, a running or bean stitch when it is a hairline. Only a stroke that fails
falls back; the rest of the letter keeps its satin.

### FT2.7 Order by travel

Order strokes and choose their directions to shorten jumps, with travel hidden under satin sewn
later. For script, enter and leave at the joins on the baseline.

### FT2.8 Hand from the letters

Tell script from print by the glyphs (slant, strokes that join at the side bearings), not the
name. The Trace Font dialog lets the user override it.

## Phase 3: measure

### FT3.1 A structure score

Extend the report per letter: objects, jumps, gaps between strokes that should touch, bare share,
and the worst stitch skew. `embroidery.font.trace` returns the totals, and the font editor marks
letters that need a look.

### FT3.2 Golden tests

- Synthetic letters built from rectangles and rings, with expected stroke counts, in the crate's
  tests.
- When `CRAFT_FONTS` is set, expected counts for A–Z a–z 0–9 in one sans and one serif face from
  craft-fonts; skipped when it is empty.

## Order of work

1. FT1.1 to FT1.4, then record the report's numbers as the baseline.
2. FT3.1, so Phase 2 is measured as it is built.
3. FT2.1 to FT2.4: the new tracer behind the old one, compared letter by letter.
4. FT2.5 to FT2.8, then FT3.2, then remove the raster path from tracing if nothing else needs it.
   `skeleton::centrelines` (E11.7) uses the same skeleton: check before removing.

## Open questions

- `spade` in a wasm build: expected to work, not yet built (FT2.2).
- The default sewing size when a font has no `sizes` (FT1.1).
- Whether Satin Shape and live type move to the new tracer at the same time or after it has
  proved itself on fonts.
- How traced letters are checked on fabric: a sew-out of a traced alphabet belongs with G1's
  lettering sample.

## Rules for every step

- Clean room: behaviour comes from public documentation and our own measurements. Never run or
  compare against Adobe software.
- No panics in shipped code; glyph outlines are untrusted input.
- One step per commit, named by its id here (`FT1.1: …`).
- When a step lands, update the progress docs as `AGENTS.md` describes.

## Revision history

- 2026-11-12: first version, from the analysis of Trace Font.
- 2026-11-12: `spade` checked for FT2.2 (licence, dependencies, unsafe code, panics).
