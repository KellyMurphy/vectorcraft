//! Little stitched pictures for the embroidery panels: each stitch type sewn on a small shape, each
//! fill pattern on a swatch, each border pattern along a line, drawn from the real generators so
//! what the picture shows is what the setting sews. Thumbnails are runs in mm, made once and kept
//! in the egui context.

use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, Ui, vec2};
use vectorcraft_doc::{BorderPattern, StitchKind, StitchSettings};
use vectorcraft_embroidery::{borders, generate, kinds, motifs};
use vectorcraft_geom::Point;
use vectorcraft_geom::{Anchor, FillRule, PathData, SubPath};

use crate::theme::Tokens;

/// A picture: runs of needle points, mm, and their box.
#[derive(Clone, Debug, Default)]
pub struct Thumb {
    pub runs: Vec<Vec<Point>>,
    pub lo: Point,
    pub hi: Point,
}

impl Thumb {
    /// A picture of `runs` as they are (an object's stitches).
    pub fn of(runs: &[Vec<Point>]) -> Self {
        Self::new(runs.to_vec())
    }

    fn new(runs: Vec<Vec<Point>>) -> Self {
        let (mut lo, mut hi) = (Point::new(f64::INFINITY, f64::INFINITY), Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
        for p in runs.iter().flatten() {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        if !(lo.x.is_finite() && hi.x.is_finite()) {
            (lo, hi) = (Point::ZERO, Point::new(1.0, 1.0));
        }
        Self { runs, lo, hi }
    }

    /// Made for `key` once, then kept.
    fn cached(ctx: &egui::Context, key: &str, make: impl FnOnce() -> Thumb) -> Thumb {
        let id = egui::Id::new(("stitch-art", key));
        if let Some(t) = ctx.data(|d| d.get_temp::<Thumb>(id)) {
            return t;
        }
        let t = make();
        ctx.data_mut(|d| d.insert_temp(id, t.clone()));
        t
    }
}

const PT_MM: f64 = vectorcraft_embroidery::PT_MM;

/// A path from points in mm.
fn path_mm(pts: &[(f64, f64)], closed: bool) -> PathData {
    PathData::single(SubPath { anchors: pts.iter().map(|&(x, y)| Anchor::corner(Point::new(x / PT_MM, y / PT_MM))).collect(), closed })
}

/// A gentle S curve, 14 mm long.
fn wave() -> PathData {
    let pts: Vec<(f64, f64)> = (0..=28).map(|k| (k as f64 * 0.5, 1.4 * (k as f64 / 28.0 * std::f64::consts::TAU).sin())).collect();
    path_mm(&pts, false)
}

/// A bar 14 × 2.4 mm.
fn bar(y: f64) -> PathData {
    path_mm(&[(0.0, y), (14.0, y), (14.0, y + 2.4), (0.0, y + 2.4)], true)
}

/// A square 10 × 8 mm.
fn block() -> PathData {
    path_mm(&[(0.0, 0.0), (10.0, 0.0), (10.0, 8.0), (0.0, 8.0)], true)
}

fn sew(settings: &StitchSettings, path: &PathData, stroke_mm: f64, patterns: &[BorderPattern]) -> Vec<Vec<Point>> {
    kinds::sew(&kinds::Input {
        settings,
        stroke_mm,
        paths: &[path],
        rule: FillRule::NonZero,
        patterns,
        motifs: &[],
        auto: Default::default(),
        cuts: &[],
        overlap: 0.0,
        hand: vectorcraft_embroidery::penstrokes::Hand::Print,
    })
    .runs
}

/// The stitch type `k` on a shape that suits it.
pub fn kind(ctx: &egui::Context, k: StitchKind) -> Thumb {
    Thumb::cached(ctx, &format!("kind:{}", k.id()), || {
        let mut s = StitchSettings::new(k);
        let runs = match k {
            StitchKind::Running => sew(&s, &wave(), 1.0, &[]),
            StitchKind::Bean => sew(&s, &wave(), 1.0, &[]),
            StitchKind::Satin => {
                s.width = Some(2.2);
                s.spacing = Some(0.6);
                sew(&s, &wave(), 2.2, &[])
            }
            StitchKind::Zigzag => {
                s.width = Some(2.4);
                s.spacing = Some(1.2);
                sew(&s, &wave(), 1.0, &[])
            }
            StitchKind::SatinShape => {
                s.spacing = Some(0.6);
                sew(&s, &bar(0.0), 1.0, &[])
            }
            StitchKind::Fill => {
                s.spacing = Some(0.8);
                s.length = Some(2.2);
                sew(&s, &block(), 1.0, &[])
            }
            StitchKind::PatternBorder => {
                s.pattern = Some("diamonds".into());
                s.width = Some(3.0);
                sew(&s, &path_mm(&[(0.0, 0.0), (14.0, 0.0)], false), 1.0, &[])
            }
            StitchKind::Manual => {
                let pts: Vec<(f64, f64)> = (0..7).map(|k| (k as f64 * 2.3, if k % 2 == 0 { 0.0 } else { 2.2 })).collect();
                sew(&s, &path_mm(&pts, false), 1.0, &[])
            }
            // Auto: a satin shape on a bar, a bean line under it (what it picks by width).
            StitchKind::Auto => {
                let mut satin = StitchSettings::new(StitchKind::SatinShape);
                satin.spacing = Some(0.6);
                let mut runs = sew(&satin, &bar(0.0), 1.0, &[]);
                runs.extend(sew(&StitchSettings::new(StitchKind::Running), &path_mm(&[(0.0, 4.2), (14.0, 4.2)], false), 1.0, &[]));
                runs
            }
        };
        Thumb::new(runs)
    })
}

/// A fill pattern on a 12 × 8 mm swatch: `stagger` rows (brick, tatami…) or a carved `motif`.
pub fn fill_pattern(ctx: &egui::Context, stagger: u8, motif: Option<&str>) -> Thumb {
    Thumb::cached(ctx, &format!("fill:{stagger}:{}", motif.unwrap_or("")), || {
        let polys = vec![vec![Point::new(0.0, 0.0), Point::new(12.0, 0.0), Point::new(12.0, 8.0), Point::new(0.0, 8.0)]];
        let carve = motif.and_then(|m| motifs::tiled(m, 4.0, Point::ZERO, Point::new(12.0, 8.0))).unwrap_or_default();
        let (spacing, len) = if motif.is_some() { (0.55, 2.0) } else { (0.7, 2.2) };
        let runs = generate::fill_carved(&polys, FillRule::NonZero, 0.0, spacing, len, stagger, &carve).unwrap_or_default();
        Thumb::new(runs)
    })
}

/// A border pattern along a 16 mm line, 4 mm wide.
pub fn border(ctx: &egui::Context, pattern: &BorderPattern) -> Thumb {
    // The document's patterns can change: keyed by content, not just id.
    let key = format!("border:{}:{}:{}:{}", pattern.id, pattern.width, pattern.length, pattern.repeat.iter().map(|p| p.points.len()).sum::<usize>());
    Thumb::cached(ctx, &key, || {
        let line = [Point::new(0.0, 0.0), Point::new(16.0, 0.0)];
        let runs = borders::sew(pattern, &line, false, 4.0, false, 1.5, 0.4).unwrap_or_default();
        Thumb::new(runs)
    })
}

/// Draw `thumb` fitted into `rect`: stitches as lines in `thread`, the needle points as dots when
/// `dots` (what tells a fill pattern from another).
pub fn draw(painter: &egui::Painter, rect: Rect, thumb: &Thumb, thread: Color32, dots: Option<Color32>) {
    let inner = rect.shrink(4.0);
    let (w, h) = ((thumb.hi.x - thumb.lo.x).max(0.1), (thumb.hi.y - thumb.lo.y).max(0.1));
    let scale = (f64::from(inner.width()) / w).min(f64::from(inner.height()) / h);
    let (ox, oy) = (f64::from(inner.center().x) - (thumb.lo.x + w / 2.0) * scale, f64::from(inner.center().y) - (thumb.lo.y + h / 2.0) * scale);
    let at = |p: &Point| Pos2::new((p.x * scale + ox) as f32, (p.y * scale + oy) as f32);
    // With dots, the needle holes are the picture (a fill pattern is its holes' diagonals): the
    // thread goes faint behind them.
    let line = if dots.is_some() { thread.gamma_multiply(0.45) } else { thread };
    for run in &thumb.runs {
        let pts: Vec<Pos2> = run.iter().map(at).collect();
        painter.add(egui::Shape::line(pts, Stroke::new(1.0, line)));
    }
    if let Some(c) = dots {
        for p in thumb.runs.iter().flatten() {
            painter.circle_filled(at(p), 1.5, c);
        }
    }
}

/// A clickable tile: the picture over a recessed swatch, `label` under it, outlined when `selected`.
pub fn tile(ui: &mut Ui, id: impl std::hash::Hash, size: egui::Vec2, thumb: &Thumb, label: &str, selected: bool, dots: bool) -> Response {
    let t = Tokens::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let _ = id;
    if ui.is_rect_visible(rect) {
        let pic = Rect::from_min_size(rect.min, vec2(rect.width(), rect.height() - 16.0));
        let fill = if selected {
            t.row_selected
        } else if resp.hovered() {
            t.hover
        } else {
            t.input
        };
        ui.painter().rect_filled(rect, 4.0, fill);
        if selected {
            ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.5, t.accent), egui::StrokeKind::Inside);
        } else {
            ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
        }
        let ink = if selected { t.text_strong } else { t.text };
        draw(ui.painter(), pic, thumb, ink, dots.then_some(ink));
        ui.painter().text(
            Pos2::new(rect.center().x, rect.bottom() - 8.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(10.5),
            if selected { t.text_strong } else { t.text_dim },
        );
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_stitch_type_and_pattern_has_a_picture() {
        let ctx = egui::Context::default();
        for k in StitchKind::ALL {
            let t = kind(&ctx, k);
            assert!(t.runs.iter().map(Vec::len).sum::<usize>() >= 4, "{}", k.id());
            assert!(t.hi.x > t.lo.x && t.hi.y >= t.lo.y, "{}", k.id());
        }
        for stagger in StitchSettings::STAGGER {
            assert!(!fill_pattern(&ctx, stagger, None).runs.is_empty(), "stagger {stagger}");
        }
        for m in motifs::MOTIFS {
            assert!(!fill_pattern(&ctx, 3, Some(m)).runs.is_empty(), "{m}");
        }
        for id in borders::BUILTIN {
            assert!(!border(&ctx, &borders::builtin(id).unwrap()).runs.is_empty(), "{id}");
        }
        // Tatami patterns differ from each other (the needle holes' diagonals): not the same picture.
        let a: Vec<Point> = fill_pattern(&ctx, 4, None).runs.concat();
        let b: Vec<Point> = fill_pattern(&ctx, 7, None).runs.concat();
        assert_ne!(a, b);
    }
}
