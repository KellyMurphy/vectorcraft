//! Type set in an embroidery font (E8.1b): a type object's letters made from an embroidery font's
//! glyphs ([`vectorcraft_doc::EmbroideryFont`]) instead of its outline font's.
//!
//! Point type is laid out from the embroidery font alone, so it needs no outline font: its lines
//! (at the type's line breaks and leading), each letter's advance and the font's kerning pairs,
//! the type's tracking, manual kerning, baseline shift and scaling, ligatures the font has, and
//! the line placed by the paragraph's alignment round the type's origin. Area type and type on a
//! path take their lines and places from the type's own layout (`vectorcraft_text::layout`), so
//! they wrap and follow the path as on the canvas: along a straight line the letters are respaced
//! by the embroidery font and the line moved to keep its alignment; on a path, or vertical, each
//! letter takes its outline-font letter's place and turn. Without an outline font to lay them out
//! they are set as point type ([`Lettering::as_point`]). A character the font lacks is reported
//! with where it goes, for the planner to sew from the outline font.

use kurbo::{Affine, Point};
use vectorcraft_doc::{CharStyle, EmbroideryFont, FontGlyph, GlyphObject, Justify, TextKind, TextObject};
use vectorcraft_geom::{FillRule, PathData};

use crate::fonts;

/// One stitch object of a letter, placed: in document space (points).
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    /// The type's run the letter is in (its colour).
    pub run: usize,
    pub object: GlyphObject,
    pub path: PathData,
    pub rule: FillRule,
}

/// A character the font lacks, and where it goes.
#[derive(Clone, Debug, PartialEq)]
pub struct Missing {
    pub run: usize,
    /// Its byte in the type's plain text, and its characters.
    pub byte: usize,
    pub chars: String,
    /// Its pen position on the baseline and the baseline's turn, text space.
    pub pen: Point,
    pub angle: f64,
}

/// A type object set in an embroidery font.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lettering {
    /// Every letter's stitch objects, letter by letter in reading order.
    pub placed: Vec<Placed>,
    /// Characters the font lacks (spaces and line breaks aside).
    pub missing: Vec<Missing>,
    /// The cap height the type is set at, mm (the largest of its runs'), for the font's size range.
    pub cap_mm: f64,
    /// Area type or type on a path set as point type: no outline font could lay it out.
    pub as_point: bool,
}

/// The paragraph breaks of type.
fn is_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// `t` set in `font`.
pub fn set(t: &TextObject, font: &EmbroideryFont) -> Lettering {
    let mut out = Lettering::default();
    if matches!(t.kind, TextKind::Point) && !t.vertical {
        point(t, font, &mut out);
        return out;
    }
    let db = vectorcraft_text::FontDb::global();
    let lay = vectorcraft_text::layout(db, t);
    if lay.glyphs.is_empty() {
        out.as_point = true;
        point(t, font, &mut out);
        return out;
    }
    laid_out(t, font, &lay, &mut out);
    out
}

/// One character (or ligature) to set: its run, byte, characters and glyph.
struct Cluster<'a> {
    run: usize,
    byte: usize,
    chars: String,
    glyph: Option<&'a FontGlyph>,
}

/// `t`'s lines of clusters: ligatures the font has taken greedily, longest first.
fn clusters<'a>(t: &TextObject, font: &'a EmbroideryFont) -> Vec<Vec<Cluster<'a>>> {
    let mut lines: Vec<Vec<Cluster<'a>>> = vec![vec![]];
    let mut byte = 0usize;
    for (run, r) in t.runs.iter().enumerate() {
        if r.inline.is_some() {
            byte += r.text.len();
            continue;
        }
        let chars: Vec<(usize, char)> = r.text.char_indices().collect();
        let mut i = 0;
        while let Some(&(at, c)) = chars.get(i) {
            if is_break(c) {
                lines.push(vec![]);
                i += 1;
                continue;
            }
            // The longest run of characters the font has a glyph for, else the one.
            let mut taken = 1;
            for n in (2..=fonts::MAX_GLYPH_CHARS.min(chars.len() - i)).rev() {
                let s: String = chars.iter().skip(i).take(n).map(|(_, c)| *c).collect();
                if !s.chars().any(is_break) && font.glyph(&s).is_some() {
                    taken = n;
                    break;
                }
            }
            let s: String = chars.iter().skip(i).take(taken).map(|(_, c)| *c).collect();
            let glyph = font.glyph(&s);
            if let Some(line) = lines.last_mut() {
                line.push(Cluster { run, byte: byte + at, chars: s, glyph });
            }
            i += taken;
        }
        byte += r.text.len();
    }
    lines
}

fn style_of(t: &TextObject, run: usize) -> CharStyle {
    t.runs.get(run).map(|r| r.style.clone()).unwrap_or_else(|| t.first_style())
}

/// Pen positions along a line of `clusters` from x = 0, spaced by the font: each cluster's x, and
/// the line's width.
fn spaced(t: &TextObject, font: &EmbroideryFont, clusters: &[(usize, &str, Option<&FontGlyph>, f64)]) -> (Vec<f64>, f64) {
    let upem = font.units_per_em.max(1.0);
    let mut xs = Vec::with_capacity(clusters.len());
    let mut x = 0.0;
    let mut prev: Option<&str> = None;
    for (run, chars, glyph, fallback_advance) in clusters {
        let st = style_of(t, *run);
        let k = st.size / upem;
        // Manual kerning replaces the font's pairs (as for outline fonts).
        x += match (st.kerning, prev) {
            (Some(manual), Some(_)) => manual / 1000.0 * st.size,
            (None, Some(p)) => font.kern(p, chars) * k,
            _ => 0.0,
        };
        xs.push(x);
        x += match glyph {
            Some(g) => g.advance * k * st.h_scale / 100.0 + st.tracking / 1000.0 * st.size,
            None => *fallback_advance,
        };
        prev = Some(chars);
    }
    (xs, x)
}

/// How far a line `width` wide moves so it keeps `t`'s alignment, starting at 0: `room` is the
/// width it is aligned in (0 for point type, aligned round its origin).
fn align(t: &TextObject, width: f64, room: f64) -> f64 {
    match t.para.justify {
        Justify::Center | Justify::JustifyCenter => (room - width) / 2.0,
        Justify::Right | Justify::JustifyRight => room - width,
        _ => 0.0,
    }
}

/// Point type: laid out from the font alone.
fn point(t: &TextObject, font: &EmbroideryFont, out: &mut Lettering) {
    let mut y = 0.0;
    for (li, line) in clusters(t, font).iter().enumerate() {
        // Each line's leading is its largest (Auto: 120% of the size).
        let lead = line.iter().map(|c| style_of(t, c.run)).map(|s| s.leading.unwrap_or(s.size * 1.2)).fold(0.0, f64::max);
        if li > 0 {
            y += if lead > 0.0 { lead } else { t.first_style().size * 1.2 };
        }
        // A missing character takes half an em.
        let row: Vec<(usize, &str, Option<&FontGlyph>, f64)> =
            line.iter().map(|c| (c.run, c.chars.as_str(), c.glyph, style_of(t, c.run).size * 0.5)).collect();
        let (xs, width) = spaced(t, font, &row);
        let shift = align(t, width, 0.0);
        for (c, x) in line.iter().zip(xs) {
            let st = style_of(t, c.run);
            let pen = Point::new(x + shift, y - st.baseline_shift);
            put(t, font, out, c.run, c.byte, &c.chars, c.glyph, pen, 0.0);
        }
    }
}

/// Area type and type on a path: the type's own layout, respaced along straight lines.
fn laid_out(t: &TextObject, font: &EmbroideryFont, lay: &vectorcraft_text::TextLayout, out: &mut Lettering) {
    let text = t.plain_text();
    let inline = |gi: usize| lay.inlines.iter().any(|i| i.glyph == gi);
    let lines = lay.glyphs.iter().map(|g| g.line).max().map_or(0, |m| m + 1);
    for line in 0..lines {
        let glyphs: Vec<&vectorcraft_text::PositionedGlyph> =
            lay.glyphs.iter().enumerate().filter(|(gi, g)| g.line == line && !inline(*gi)).map(|(_, g)| g).collect();
        let (Some(first), Some(last)) = (glyphs.first(), glyphs.last()) else { continue };
        let row: Vec<(usize, &str, Option<&FontGlyph>, f64)> = glyphs
            .iter()
            .map(|g| {
                let chars = text.get(g.byte..g.byte + g.len).unwrap_or_default();
                (g.run, chars, font.glyph(chars), g.advance)
            })
            .collect();
        let straight = !t.vertical && glyphs.iter().all(|g| g.angle.abs() < 1e-9 && !g.rtl);
        let pens: Vec<Point> = if straight {
            let (xs, width) = spaced(t, font, &row);
            let room = last.origin.x + last.advance - first.origin.x;
            // Aligned as the line was: within the room it took.
            let shift = first.origin.x + align(t, width, room);
            glyphs.iter().zip(xs).map(|(g, x)| Point::new(x + shift, g.origin.y)).collect()
        } else {
            glyphs.iter().map(|g| g.origin).collect()
        };
        for ((g, (run, chars, glyph, _)), pen) in glyphs.iter().zip(&row).zip(pens) {
            put(t, font, out, *run, g.byte, chars, *glyph, pen, g.angle);
        }
    }
}

/// Place one letter's objects (or report it missing) at `pen` on a baseline turned `angle`.
#[allow(clippy::too_many_arguments)]
fn put(
    t: &TextObject,
    font: &EmbroideryFont,
    out: &mut Lettering,
    run: usize,
    byte: usize,
    chars: &str,
    glyph: Option<&FontGlyph>,
    pen: Point,
    angle: f64,
) {
    let upem = font.units_per_em.max(1.0);
    let st = style_of(t, run);
    out.cap_mm = out.cap_mm.max(st.size * font.cap_height / upem * crate::PT_MM);
    let Some(g) = glyph else {
        if !chars.chars().all(char::is_whitespace) {
            out.missing.push(Missing { run, byte, chars: chars.to_string(), pen, angle });
        }
        return;
    };
    let k = st.size / upem;
    // Font units (y up) → text space (y down) at the letter's place, then the document.
    let a =
        t.xf * Affine::translate(pen.to_vec2()) * Affine::rotate(angle) * Affine::scale_non_uniform(k * st.h_scale / 100.0, -k * st.v_scale / 100.0);
    for o in &g.objects {
        let Some(bez) = fonts::outline(o) else { continue };
        let rule = if o.rule.as_deref() == Some("evenodd") { FillRule::EvenOdd } else { FillRule::NonZero };
        out.placed.push(Placed { run, object: o.clone(), path: PathData::from_bezpath(&(a * bez)), rule });
    }
}

#[cfg(test)]
mod tests {
    use vectorcraft_doc::{CharStyle, TextObject};

    use super::*;

    fn point_type(s: &str, size: f64) -> TextObject {
        let style = CharStyle { size, ..CharStyle::default() };
        TextObject::point(Point::new(100.0, 200.0), s, style)
    }

    fn left(l: &Lettering, i: usize) -> f64 {
        l.placed[i].path.bounds().map_or(f64::NAN, |b| b.x0)
    }

    #[test]
    fn letters_are_spaced_by_the_embroidery_font() {
        let font = fonts::sample();
        // 100 pt type: one font unit is 0.1 pt.
        let l = set(&point_type("IL", 100.0), &font);
        assert!(l.missing.is_empty() && !l.as_point);
        // I's stroke at 75 units, L's at I's advance (250) + 75.
        assert!((left(&l, 0) - 107.5).abs() < 1e-6 && (left(&l, 1) - 132.5).abs() < 1e-6 && (left(&l, 2) - 142.5).abs() < 1e-6);
        // The baseline: letters stand on y = 200, 70 pt tall.
        let b = l.placed[0].path.bounds().unwrap();
        assert!((b.y1 - 200.0).abs() < 1e-6 && (b.y0 - 130.0).abs() < 1e-6, "{b:?}");
        // The L-T pair kerns closer by 120 units: T's stem at 52 - 12 + 25.
        let lt = set(&point_type("LT", 100.0), &font);
        assert!((left(&lt, 2) - (100.0 + 52.0 - 12.0 + 25.0)).abs() < 1e-6, "{}", left(&lt, 2));
        // Cap height: 70 pt.
        assert!((l.cap_mm - 70.0 * crate::PT_MM).abs() < 1e-9);
    }

    #[test]
    fn tracking_and_manual_kerning_apply() {
        let font = fonts::sample();
        let mut t = point_type("LT", 100.0);
        t.runs[0].style.tracking = 100.0;
        let tracked = left(&set(&t, &font), 2);
        t.runs[0].style.tracking = 0.0;
        t.runs[0].style.kerning = Some(0.0);
        let unkerned = left(&set(&t, &font), 2);
        // +100/1000 em of tracking; manual 0 kerning drops the font's -120.
        assert!((tracked - (100.0 + 52.0 + 10.0 - 12.0 + 25.0)).abs() < 1e-6, "{tracked}");
        assert!((unkerned - (100.0 + 52.0 + 25.0)).abs() < 1e-6, "{unkerned}");
    }

    #[test]
    fn lines_break_lead_and_align() {
        let font = fonts::sample();
        let mut t = point_type("I\nI", 100.0);
        let l = set(&t, &font);
        let (a, b) = (l.placed[0].path.bounds().unwrap(), l.placed[1].path.bounds().unwrap());
        assert!((b.y1 - a.y1 - 120.0).abs() < 1e-6, "Auto leading: 120% of 100 pt");
        // Centred round the origin: "I" is 25 pt wide, so it starts 12.5 pt left of it.
        t.para.justify = Justify::Center;
        let c = set(&t, &font);
        assert!((left(&c, 0) - (100.0 - 12.5 + 7.5)).abs() < 1e-6, "{}", left(&c, 0));
        t.para.justify = Justify::Right;
        assert!((left(&set(&t, &font), 0) - (100.0 - 25.0 + 7.5)).abs() < 1e-6);
    }

    #[test]
    fn missing_characters_are_reported_where_they_go() {
        let l = set(&point_type("I Z", 50.0), &fonts::sample());
        assert_eq!(l.missing.len(), 1);
        let m = &l.missing[0];
        assert_eq!((m.chars.as_str(), m.byte), ("Z", 2));
        // After I (12.5 pt) and the space (15 pt).
        assert!((m.pen.x - 27.5).abs() < 1e-6, "{:?}", m.pen);
        assert_eq!(l.placed.len(), 1, "the space has no objects");
    }

    #[test]
    fn ligatures_the_font_has_are_used() {
        let mut font = fonts::sample();
        font.glyphs.push(FontGlyph { chars: "LT".into(), advance: 900.0, objects: vec![], ..Default::default() });
        let l = set(&point_type("LTI", 100.0), &font);
        // The I after the ligature's 90 pt.
        assert_eq!(l.placed.len(), 1);
        assert!((left(&l, 0) - (100.0 + 90.0 + 7.5)).abs() < 1e-6, "{}", left(&l, 0));
    }
}
