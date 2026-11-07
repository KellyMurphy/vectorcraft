//! Garment mockups (E9.2b): the design sewn on a T-shirt, polo shirt, hoodie, cap, tote bag or
//! towel, at its real size and where such designs usually go, so a customer can see it before
//! anything is sewn.
//!
//! Every garment is drawn here in code from simple shapes (panels of fabric, seams, ribbing,
//! buttons, cords) at an adult size in millimetres; nothing is traced from photographs or another
//! program's artwork. The fabric is the Realistic view's ([`crate::realistic`]), shaded so the
//! garment reads as cloth (edges falling away from the light, a few soft folds, a shadow on the
//! backdrop), and the design is drawn with the same shaded threads.
//!
//! Where designs go and how big they usually are is common trade practice (a left-chest logo about
//! 3½–4 in wide, a cap front at most about 4½ × 2¼ in), not any one maker's rule: a design larger
//! than the usual size is still drawn, with a warning.

use kurbo::{Affine, BezPath, Point, Rect};

use crate::Plan;
use crate::realistic::{self, Canvas, Image, Look, Surface, Texture, smoothstep};

/// A garment a design can be shown on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Garment {
    TShirt,
    Polo,
    Hoodie,
    Cap,
    Tote,
    Towel,
}

/// Where on a garment a design goes: the centre of the design (mm, in the garment's drawing, x = 0
/// on its centre line, the wearer's left on the right as seen from the front) and the largest size
/// usually sewn there (width, height, mm).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub id: &'static str,
    pub label: &'static str,
    pub center: (f64, f64),
    pub max: (f64, f64),
}

const fn place(id: &'static str, label: &'static str, center: (f64, f64), max: (f64, f64)) -> Placement {
    Placement { id, label, center, max }
}

const LEFT_CHEST: &str = "leftChest";
const RIGHT_CHEST: &str = "rightChest";
const CENTER: &str = "center";
const LOWER: &str = "lower";

impl Garment {
    pub const ALL: [Garment; 6] = [Garment::TShirt, Garment::Polo, Garment::Hoodie, Garment::Cap, Garment::Tote, Garment::Towel];

    pub fn id(self) -> &'static str {
        match self {
            Garment::TShirt => "tshirt",
            Garment::Polo => "polo",
            Garment::Hoodie => "hoodie",
            Garment::Cap => "cap",
            Garment::Tote => "tote",
            Garment::Towel => "towel",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Garment::TShirt => "T-Shirt",
            Garment::Polo => "Polo Shirt",
            Garment::Hoodie => "Hoodie",
            Garment::Cap => "Cap",
            Garment::Tote => "Tote Bag",
            Garment::Towel => "Towel",
        }
    }

    pub fn from_id(id: &str) -> Option<Garment> {
        Garment::ALL.into_iter().find(|g| g.id().eq_ignore_ascii_case(id))
    }

    /// The fabric it is made of.
    pub fn texture(self) -> Texture {
        match self {
            Garment::TShirt | Garment::Polo | Garment::Hoodie => Texture::Knit,
            Garment::Cap => Texture::Twill,
            Garment::Tote => Texture::Plain,
            Garment::Towel => Texture::Terry,
        }
    }

    /// Its colour unless one is chosen (sRGB).
    pub fn default_color(self) -> [u8; 3] {
        match self {
            Garment::TShirt => [242, 242, 238],
            Garment::Polo => [34, 46, 82],
            Garment::Hoodie => [146, 148, 152],
            Garment::Cap => [44, 44, 48],
            Garment::Tote => [228, 216, 192],
            Garment::Towel => [248, 247, 243],
        }
    }

    /// Where designs go on it; the first is where one goes unless another is chosen.
    pub fn placements(self) -> &'static [Placement] {
        const TSHIRT: &[Placement] = &[
            place(LEFT_CHEST, "Left Chest", (95.0, 190.0), (100.0, 100.0)),
            place(RIGHT_CHEST, "Right Chest", (-95.0, 190.0), (100.0, 100.0)),
            place(CENTER, "Center Front", (0.0, 280.0), (280.0, 280.0)),
        ];
        const POLO: &[Placement] =
            &[place(LEFT_CHEST, "Left Chest", (100.0, 200.0), (100.0, 100.0)), place(RIGHT_CHEST, "Right Chest", (-100.0, 200.0), (100.0, 100.0))];
        const HOODIE: &[Placement] = &[
            place(CENTER, "Center Front", (0.0, 250.0), (250.0, 180.0)),
            place(LEFT_CHEST, "Left Chest", (105.0, 185.0), (100.0, 100.0)),
            place(RIGHT_CHEST, "Right Chest", (-105.0, 185.0), (100.0, 100.0)),
        ];
        const CAP: &[Placement] = &[place(CENTER, "Front", (0.0, 88.0), (115.0, 57.0))];
        const TOTE: &[Placement] = &[place(CENTER, "Center", (0.0, 215.0), (280.0, 280.0))];
        const TOWEL: &[Placement] =
            &[place(LOWER, "Above the Border", (0.0, 500.0), (250.0, 120.0)), place(CENTER, "Center", (0.0, 330.0), (300.0, 300.0))];
        match self {
            Garment::TShirt => TSHIRT,
            Garment::Polo => POLO,
            Garment::Hoodie => HOODIE,
            Garment::Cap => CAP,
            Garment::Tote => TOTE,
            Garment::Towel => TOWEL,
        }
    }

    /// Its placement `id` (`None`: its usual one).
    pub fn placement(self, id: Option<&str>) -> Option<Placement> {
        match id {
            None => self.placements().first().copied(),
            Some(id) => self.placements().iter().find(|p| p.id.eq_ignore_ascii_case(id)).copied(),
        }
    }
}

/// Every placement id any garment has (for parameter docs and pickers).
pub fn placement_ids() -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = vec![];
    for g in Garment::ALL {
        for p in g.placements() {
            if !ids.contains(&p.id) {
                ids.push(p.id);
            }
        }
    }
    ids
}

/// What to draw.
#[derive(Clone, Debug)]
pub struct Spec {
    pub garment: Garment,
    pub placement: Placement,
    /// The fabric (texture and colour).
    pub look: Look,
    /// Close to the design rather than the whole garment.
    pub close_up: bool,
    /// The image's width, pixels (its height follows).
    pub width: u32,
}

/// The largest and smallest image width, pixels.
pub const WIDTH: std::ops::RangeInclusive<u32> = 200..=4000;

/// A drawn mockup.
#[derive(Clone, Debug)]
pub struct Mockup {
    pub image: Image,
    /// The part of the garment's drawing shown, mm.
    pub view: Rect,
    /// The design's size, mm (`None`: nothing is stitched).
    pub design: Option<(f64, f64)>,
    pub warnings: Vec<String>,
}

/// Draw `plan` on the garment of `spec`.
pub fn render(plan: &Plan, spec: &Spec) -> Result<Mockup, String> {
    if !WIDTH.contains(&spec.width) {
        return Err(format!("the width must be {} to {} pixels", WIDTH.start(), WIDTH.end()));
    }
    let shape = shape(spec.garment);
    let bounds = crate::machine::bounds(plan);
    let design = bounds.map(|(lo, hi)| (hi.0 - lo.0, hi.1 - lo.1));
    let pl = spec.placement;
    let mut warnings = vec![];
    match design {
        None => warnings.push("Nothing is stitched yet: the garment is shown bare.".to_string()),
        Some((w, h)) if w > pl.max.0 + 0.5 || h > pl.max.1 + 0.5 => warnings.push(format!(
            "The design is {w:.0} × {h:.0} mm; a {} design on a {} is usually at most {:.0} × {:.0} mm.",
            pl.label.to_lowercase(),
            spec.garment.label().to_lowercase(),
            pl.max.0,
            pl.max.1
        )),
        Some(_) => {}
    }
    // The design's centre goes to the placement's.
    let place = match bounds {
        Some((lo, hi)) => Affine::translate((pl.center.0 - (lo.0 + hi.0) / 2.0, pl.center.1 - (lo.1 + hi.1) / 2.0)),
        None => Affine::IDENTITY,
    };
    let garment_box = shape.bounds();
    let view = if spec.close_up {
        let (dw, dh) = design.unwrap_or(pl.max);
        let side = dw.max(dh).max(pl.max.0.max(pl.max.1) * 0.6) * 1.6 + 30.0;
        Rect::from_center_size(Point::new(pl.center.0, pl.center.1), (side, side))
    } else {
        garment_box.inflate(40.0, 40.0)
    };
    let scale = f64::from(spec.width) / view.width();
    let height = (view.height() * scale).round();
    if !(height.is_finite() && height >= 1.0) {
        return Err("the view can't be drawn".into());
    }
    let height = height as u32;
    if u64::from(spec.width) * u64::from(height) > realistic::MAX_PIXELS {
        return Err(format!("an image {} × {height} pixels is too big to draw", spec.width));
    }
    let to_px = Affine::scale(scale) * Affine::translate((-view.x0, -view.y0));
    let (w, h) = (spec.width as usize, height as usize);
    let mut cv = Canvas { w, h, px: vec![[0.0; 4]; w * h] };
    backdrop(&mut cv);
    let mask = union_mask(&shape, to_px, w, h);
    drop_shadow(&mut cv, &mask, scale);
    let light = blur(&mask, w, h, (18.0 * scale).round().max(1.0) as usize);
    let surface = Surface::new(&spec.look, scale);
    let inv = to_px.inverse();
    for panel in &shape.panels {
        let polys: Vec<Vec<[f64; 2]>> = panel.polys.iter().map(|p| p.iter().map(|q| px(to_px, *q)).collect()).collect();
        let cover = coverage(&polys, w, h);
        let rib = (2.2 * scale > 3.0).then_some(2.2);
        for (i, &c) in cover.iter().enumerate() {
            if c <= 0.0 {
                continue;
            }
            let (x, y) = (i % w, i / w);
            let p = inv * Point::new(x as f64 + 0.5, y as f64 + 0.5);
            let mut col = surface.at(p.x, p.y);
            let edge = 0.70 + 0.30 * smoothstep(0.30, 0.95, light.get(i).copied().unwrap_or(1.0));
            let fold = 1.0 + 0.10 * (realistic::noise(p.x / 120.0 + 3.0, p.y / 170.0) - 0.5);
            let slope = 1.04 - 0.08 * ((p.y - garment_box.y0) / garment_box.height().max(1.0)) as f32;
            let ribbing = match (panel.rib, rib) {
                (true, Some(pitch)) => 0.92 + 0.08 * (std::f64::consts::TAU * p.x / pitch).cos() as f32,
                _ => 1.0,
            };
            let k = panel.tone * edge * fold * slope * ribbing;
            for v in &mut col {
                *v *= k;
            }
            cv.blend(x, y, col, c);
        }
        // Where one panel lies over another, its edge casts a thin shadow.
        if panel.outlined {
            for poly in &polys {
                let mut closed = poly.clone();
                if let Some(first) = poly.first() {
                    closed.push(*first);
                }
                groove(&mut cv, &closed, (0.7 * scale) as f32, 0.22);
            }
        }
    }
    let fabric_rgb = realistic::rgb(spec.look.color);
    for d in &shape.details {
        match d {
            Detail::Stitch(pts) => {
                let pts: Vec<[f64; 2]> = pts.iter().map(|q| px(to_px, *q)).collect();
                topstitch(&mut cv, &pts, scale, fabric_rgb);
            }
            Detail::Seam(pts) => {
                let pts: Vec<[f64; 2]> = pts.iter().map(|q| px(to_px, *q)).collect();
                groove(&mut cv, &pts, (0.9 * scale) as f32, 0.28);
            }
            Detail::Fold(pts, width) => {
                let pts: Vec<[f64; 2]> = pts.iter().map(|q| px(to_px, *q)).collect();
                groove(&mut cv, &pts, (*width * 2.5 * scale) as f32, 0.06);
            }
            Detail::Cord(pts, width) => {
                let pts: Vec<[f64; 2]> = pts.iter().map(|q| px(to_px, *q)).collect();
                cord(&mut cv, &pts, (*width * scale / 2.0) as f32, fabric_rgb);
            }
            Detail::Button(c, r) => button(&mut cv, px(to_px, *c), (*r * scale) as f32, fabric_rgb),
        }
    }
    realistic::threads(&mut cv, plan, to_px * place, scale as f32);
    let mut rgba = Vec::with_capacity(w * h * 4);
    for p in &cv.px {
        rgba.extend([realistic::to_u8(p[0]), realistic::to_u8(p[1]), realistic::to_u8(p[2]), 255]);
    }
    Ok(Mockup { image: Image { width: spec.width, height, rgba }, view, design, warnings })
}

fn px(to_px: Affine, p: Point) -> [f64; 2] {
    let q = to_px * p;
    [q.x, q.y]
}

// ---- Drawing ----

/// A plain studio backdrop, lighter at the top.
fn backdrop(cv: &mut Canvas) {
    let h = cv.h.max(1) as f32;
    for y in 0..cv.h {
        let t = y as f32 / h;
        let v = 0.95 - 0.10 * t;
        for x in 0..cv.w {
            if let Some(p) = cv.get(x, y) {
                *p = [v, v, v * 0.985, 1.0];
            }
        }
    }
}

/// How much of each pixel any of the garment's panels covers.
fn union_mask(shape: &Shape, to_px: Affine, w: usize, h: usize) -> Vec<f32> {
    let mut mask = vec![0.0f32; w * h];
    for panel in &shape.panels {
        let polys: Vec<Vec<[f64; 2]>> = panel.polys.iter().map(|p| p.iter().map(|q| px(to_px, *q)).collect()).collect();
        for (m, c) in mask.iter_mut().zip(coverage(&polys, w, h)) {
            *m = m.max(c);
        }
    }
    mask
}

/// The garment's soft shadow on the backdrop, below and right of it.
fn drop_shadow(cv: &mut Canvas, mask: &[f32], scale: f64) {
    let (w, h) = (cv.w, cv.h);
    let soft = blur(mask, w, h, (9.0 * scale).round().max(1.0) as usize);
    let (dx, dy) = ((4.0 * scale).round() as isize, (9.0 * scale).round() as isize);
    for y in 0..h {
        for x in 0..w {
            // Beyond the image's edge the garment goes on as it is at the edge.
            let (sx, sy) = ((x as isize - dx).max(0) as usize, (y as isize - dy).max(0) as usize);
            let Some(&s) = soft.get(sy * w + sx) else { continue };
            if s > 0.0 {
                cv.shade(x, y, 0.32 * s);
            }
        }
    }
}

/// `src` (w × h) blurred: three box passes each way, about a Gaussian of radius `r` pixels.
fn blur(src: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let mut a = src.to_vec();
    let mut b = vec![0.0f32; a.len()];
    let r = r.max(1);
    for _ in 0..3 {
        box_pass(&a, &mut b, w, h, r, true);
        box_pass(&b, &mut a, w, h, r, false);
    }
    a
}

/// One box blur pass along rows (`across`) or columns. Beyond the image's edge each line goes on
/// as it is at the edge (a garment cut off by a close-up's frame doesn't darken toward it).
fn box_pass(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize, across: bool) {
    let (n, lines) = if across { (w, h) } else { (h, w) };
    if n == 0 {
        return;
    }
    let last = n as isize - 1;
    let at = |line: usize, i: isize| {
        let i = i.clamp(0, last) as usize;
        if across { line * w + i } else { i * w + line }
    };
    let r = r as isize;
    let norm = 1.0 / (2 * r + 1) as f32;
    for line in 0..lines {
        let get = |i: isize| src.get(at(line, i)).copied().unwrap_or(0.0);
        let mut sum: f32 = (-r..=r).map(get).sum();
        for i in 0..n as isize {
            if let Some(d) = dst.get_mut(at(line, i)) {
                *d = (sum * norm).max(0.0);
            }
            sum += get(i + r + 1) - get(i - r);
        }
    }
}

/// How much of each pixel (w × h) the polygons cover (even-odd; pixel coordinates), with four
/// samples down each pixel and exact coverage across it.
fn coverage(polys: &[Vec<[f64; 2]>], w: usize, h: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; w * h];
    let (mut y_lo, mut y_hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in polys.iter().flatten() {
        if !(p[0].is_finite() && p[1].is_finite()) {
            return out;
        }
        y_lo = y_lo.min(p[1]);
        y_hi = y_hi.max(p[1]);
    }
    if !(y_lo.is_finite() && y_hi.is_finite()) {
        return out;
    }
    let y0 = y_lo.floor().max(0.0) as usize;
    let y1 = (y_hi.ceil().max(0.0) as usize).min(h);
    const SUB: usize = 4;
    let mut xs: Vec<f64> = vec![];
    for y in y0..y1 {
        for s in 0..SUB {
            let sy = y as f64 + (s as f64 + 0.5) / SUB as f64;
            xs.clear();
            for poly in polys {
                let n = poly.len();
                for i in 0..n {
                    let (Some(a), Some(b)) = (poly.get(i), poly.get((i + 1) % n)) else { continue };
                    if (a[1] <= sy) != (b[1] <= sy) {
                        xs.push(a[0] + (sy - a[1]) / (b[1] - a[1]) * (b[0] - a[0]));
                    }
                }
            }
            xs.sort_by(f64::total_cmp);
            for &[a, b] in xs.as_chunks::<2>().0 {
                let (xa, xb) = (a.clamp(0.0, w as f64), b.clamp(0.0, w as f64));
                if xb <= xa {
                    continue;
                }
                let row = y * w;
                let (ia, ib) = (xa.floor() as usize, xb.floor() as usize);
                let add = |out: &mut Vec<f32>, i: usize, v: f64| {
                    if let Some(c) = out.get_mut(row + i) {
                        *c += (v / SUB as f64) as f32;
                    }
                };
                if ia == ib {
                    add(&mut out, ia, xb - xa);
                } else {
                    add(&mut out, ia, (ia + 1) as f64 - xa);
                    for i in ia + 1..ib.min(w) {
                        add(&mut out, i, 1.0);
                    }
                    if ib < w {
                        add(&mut out, ib, xb - ib as f64);
                    }
                }
            }
        }
    }
    for c in &mut out {
        *c = c.clamp(0.0, 1.0);
    }
    out
}

/// Each segment of polyline `pts` (pixels), `f` called with its ends.
fn segments(pts: &[[f64; 2]], mut f: impl FnMut([f32; 2], [f32; 2])) {
    for s in pts.windows(2) {
        let (a, b) = (s[0], s[1]);
        f([a[0] as f32, a[1] as f32], [b[0] as f32, b[1] as f32]);
    }
}

/// Darken along `pts`, `k` at the line fading out `r` pixels from it.
fn groove(cv: &mut Canvas, pts: &[[f64; 2]], r: f32, k: f32) {
    let r = r.max(0.6);
    let size = (cv.w, cv.h);
    let mut seen = vec![0.0f32; 0];
    // Overlapping segments shouldn't darken twice: keep the strongest per pixel.
    let mut touched: Vec<(usize, usize, f32)> = vec![];
    segments(pts, |a, b| {
        let d = [b[0] - a[0], b[1] - a[1]];
        let len2 = d[0] * d[0] + d[1] * d[1];
        realistic::capsule_rows(a, b, r, size, |y, x0, x1| {
            for x in x0..x1 {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                let t = realistic::nearest_t(p, a, d, len2);
                let dist = ((p[0] - a[0] - d[0] * t).powi(2) + (p[1] - a[1] - d[1] * t).powi(2)).sqrt();
                let v = k * (1.0 - smoothstep(0.0, r, dist));
                if v > 0.0 {
                    touched.push((x, y, v));
                }
            }
        });
    });
    if touched.is_empty() {
        return;
    }
    seen.resize(cv.w * cv.h, 0.0);
    for (x, y, v) in touched {
        if let Some(s) = seen.get_mut(y * cv.w + x) {
            *s = s.max(v);
        }
    }
    for (i, &v) in seen.iter().enumerate() {
        if v > 0.0 {
            cv.shade(i % cv.w, i / cv.w, v);
        }
    }
}

/// Topstitching along `pts`: short stitches of thread a little darker than the fabric, each
/// sunk in a slight groove.
fn topstitch(cv: &mut Canvas, pts: &[[f64; 2]], scale: f64, fabric: [f32; 3]) {
    let (on, off) = (3.0 * scale, 1.6 * scale);
    let r = (0.32 * scale).max(0.35) as f32;
    let lum = 0.3 * fabric[0] + 0.59 * fabric[1] + 0.11 * fabric[2];
    // Thread matched to the fabric reads a little darker (pale fabric) or lighter (dark fabric).
    let color = if lum > 0.5 { fabric.map(|c| c * 0.72) } else { fabric.map(|c| c * 0.6 + 0.16) };
    let size = (cv.w, cv.h);
    let mut walked = 0.0f64;
    for s in pts.windows(2) {
        let (a, b) = (s[0], s[1]);
        let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        if !(len.is_finite() && len > 0.0) {
            continue;
        }
        let mut t = 0.0f64;
        while t < len {
            let phase = (walked + t) % (on + off);
            let rest = if phase < on { on - phase } else { on + off - phase };
            let end = (t + rest).min(len);
            if phase < on {
                let p = |u: f64| [(a[0] + (b[0] - a[0]) * u / len) as f32, (a[1] + (b[1] - a[1]) * u / len) as f32];
                let (pa, pb) = (p(t), p(end));
                let d = [pb[0] - pa[0], pb[1] - pa[1]];
                let len2 = d[0] * d[0] + d[1] * d[1];
                realistic::capsule_rows(pa, pb, r, size, |y, x0, x1| {
                    for x in x0..x1 {
                        let q = [x as f32 + 0.5, y as f32 + 0.5];
                        let u = realistic::nearest_t(q, pa, d, len2);
                        let dist = ((q[0] - pa[0] - d[0] * u).powi(2) + (q[1] - pa[1] - d[1] * u).powi(2)).sqrt();
                        let a = (r - dist + 0.5).clamp(0.0, 1.0) * 0.85;
                        if a > 0.0 {
                            cv.blend(x, y, color, a);
                        }
                    }
                });
            }
            if end <= t {
                break;
            }
            t = end;
        }
        walked += len;
    }
}

/// A round cord (a drawstring) along `pts`, radius `r` pixels.
fn cord(cv: &mut Canvas, pts: &[[f64; 2]], r: f32, fabric: [f32; 3]) {
    let r = r.max(0.6);
    let size = (cv.w, cv.h);
    let color = fabric.map(|c| c * 0.9 + 0.06);
    segments(pts, |a, b| {
        let d = [b[0] - a[0], b[1] - a[1]];
        let len2 = d[0] * d[0] + d[1] * d[1];
        realistic::capsule_rows(a, b, r, size, |y, x0, x1| {
            for x in x0..x1 {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                let t = realistic::nearest_t(p, a, d, len2);
                let off = [p[0] - a[0] - d[0] * t, p[1] - a[1] - d[1] * t];
                let dist = (off[0] * off[0] + off[1] * off[1]).sqrt();
                let cover = (r - dist + 0.5).clamp(0.0, 1.0);
                if cover > 0.0 {
                    // Lit from the left: brighter on that side of the cord.
                    let side = if len2 > 0.0 { (off[0] * d[1] - off[1] * d[0]) / len2.sqrt() / r } else { 0.0 };
                    let lit = 0.85 + 0.2 * (1.0 - (dist / r).powi(2)).max(0.0).sqrt() - 0.1 * side;
                    cv.blend(x, y, color.map(|c| c * lit), cover);
                }
            }
        });
    });
}

/// A round button, radius `r` pixels, with its two holes.
fn button(cv: &mut Canvas, c: [f64; 2], r: f32, fabric: [f32; 3]) {
    let r = r.max(1.0);
    let lum = 0.3 * fabric[0] + 0.59 * fabric[1] + 0.11 * fabric[2];
    let base = if lum > 0.5 { fabric.map(|v| v * 0.92) } else { fabric.map(|v| v * 0.8 + 0.1) };
    let (cx, cy) = (c[0] as f32, c[1] as f32);
    let x0 = (cx - r - 2.0).floor().max(0.0) as usize;
    let y0 = (cy - r - 2.0).floor().max(0.0) as usize;
    let x1 = ((cx + r + 2.0).ceil().max(0.0) as usize).min(cv.w);
    let y1 = ((cy + r + 2.0).ceil().max(0.0) as usize).min(cv.h);
    for y in y0..y1 {
        for x in x0..x1 {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let d = (dx * dx + dy * dy).sqrt();
            // Its shadow, then the button: a dished disc lit from the upper left.
            let sd = ((dx - r * 0.2).powi(2) + (dy - r * 0.3).powi(2)).sqrt();
            let s = 1.0 - smoothstep(r * 0.9, r * 1.25, sd);
            if s > 0.0 {
                cv.shade(x, y, 0.25 * s);
            }
            let cover = (r - d + 0.5).clamp(0.0, 1.0);
            if cover <= 0.0 {
                continue;
            }
            let rim = smoothstep(0.65, 0.95, d / r);
            let light = 1.0 + 0.18 * rim * (-(dx + dy) / (d.max(1e-3) * 1.414));
            let hole = [(-0.28f32, 0.0f32), (0.28, 0.0)].iter().any(|(hx, hy)| ((dx / r - hx).powi(2) + (dy / r - hy).powi(2)).sqrt() < 0.13);
            let k = if hole { 0.45 } else { light * 0.97 };
            cv.blend(x, y, base.map(|v| v * k), cover);
        }
    }
}

// ---- Garments ----

/// A garment drawn flat from the front, mm.
struct Shape {
    /// Drawn in order, later over earlier.
    panels: Vec<Panel>,
    /// Drawn over the panels, in order.
    details: Vec<Detail>,
}

impl Shape {
    fn bounds(&self) -> Rect {
        let mut r: Option<Rect> = None;
        for p in self.panels.iter().flat_map(|p| p.polys.iter().flatten()) {
            r = Some(match r {
                Some(r) => r.union_pt(*p),
                None => Rect::from_points(*p, *p),
            });
        }
        r.unwrap_or(Rect::new(0.0, 0.0, 1.0, 1.0))
    }
}

/// A piece of fabric: its outline (polygons, even-odd), how light it is (1: lit like the rest;
/// less: turned away or inside the garment), and whether it is ribbed.
struct Panel {
    polys: Vec<Vec<Point>>,
    tone: f32,
    rib: bool,
    /// A thin shadow round its edge (it lies over what is drawn before it).
    outlined: bool,
}

impl Panel {
    fn new(poly: Vec<Point>) -> Panel {
        Panel { polys: vec![poly], tone: 1.0, rib: false, outlined: false }
    }

    fn tone(mut self, t: f32) -> Panel {
        self.tone = t;
        self
    }

    fn rib(mut self) -> Panel {
        self.rib = true;
        self
    }

    fn outlined(mut self) -> Panel {
        self.outlined = true;
        self
    }
}

enum Detail {
    /// Topstitching.
    Stitch(Vec<Point>),
    /// Where two panels are sewn together.
    Seam(Vec<Point>),
    /// A soft fold in the cloth, this wide (mm).
    Fold(Vec<Point>, f64),
    /// A drawstring, this thick (mm).
    Cord(Vec<Point>, f64),
    /// A button: its centre and radius (mm).
    Button(Point, f64),
}

/// A path's points, flattened to 0.4 mm.
fn flat(path: &BezPath) -> Vec<Point> {
    let mut pts: Vec<Point> = vec![];
    kurbo::flatten(path, 0.4, |el| match el {
        kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => pts.push(p),
        _ => {}
    });
    pts
}

/// Half an outline from the centre line round the right side (x ≥ 0), made whole: the right half,
/// then the left half mirrored back to the start.
fn mirrored(half: &BezPath) -> Vec<Point> {
    let right = flat(half);
    let mut all = right.clone();
    all.extend(right.iter().rev().map(|p| Point::new(-p.x, p.y)));
    all
}

/// `pts` mirrored to the left side.
fn mirror(pts: &[Point]) -> Vec<Point> {
    pts.iter().map(|p| Point::new(-p.x, p.y)).collect()
}

/// A polyline `d` mm to its left (as walked) — its right for negative `d`.
fn offset(pts: &[Point], d: f64) -> Vec<Point> {
    let n = pts.len();
    (0..n)
        .filter_map(|i| {
            let p = *pts.get(i)?;
            let a = *pts.get(i.saturating_sub(1))?;
            let b = *pts.get((i + 1).min(n - 1))?;
            let t = b - a;
            let len = t.hypot();
            if len < 1e-9 {
                return Some(p);
            }
            Some(p + kurbo::Vec2::new(t.y / len, -t.x / len) * d)
        })
        .collect()
}

fn path(start: (f64, f64), f: impl FnOnce(&mut BezPath)) -> BezPath {
    let mut b = BezPath::new();
    b.move_to(start);
    f(&mut b);
    b
}

/// A polyline from a few points.
fn line(pts: &[(f64, f64)]) -> Vec<Point> {
    pts.iter().map(|&(x, y)| Point::new(x, y)).collect()
}

/// The two sides of a symmetric detail.
fn both(pts: Vec<Point>, out: &mut Vec<Detail>, f: fn(Vec<Point>) -> Detail) {
    out.push(f(mirror(&pts)));
    out.push(f(pts));
}

fn shape(g: Garment) -> Shape {
    match g {
        Garment::TShirt => tshirt(),
        Garment::Polo => polo(),
        Garment::Hoodie => hoodie(),
        Garment::Cap => cap(),
        Garment::Tote => tote(),
        Garment::Towel => towel(),
    }
}

/// A short-sleeved shirt's body (right half): the neckline from the front centre `neck_drop` mm
/// below the shoulders, the sleeve to `sleeve_end`, the hem 725 mm down.
fn shirt_half(neck_drop: f64, sleeve: [(f64, f64); 2]) -> BezPath {
    path((0.0, neck_drop), |b| {
        b.curve_to((55.0, neck_drop), (92.0, neck_drop * 0.5), (95.0, 0.0));
        b.line_to((232.0, 42.0));
        b.quad_to((320.0, 92.0), sleeve[0]);
        b.line_to(sleeve[1]);
        b.quad_to((285.0, 262.0), (262.0, 250.0));
        b.quad_to((258.0, 480.0), (270.0, 722.0));
        b.quad_to((140.0, 730.0), (0.0, 728.0));
    })
}

/// The shirt details both shirts share: armhole seams, sleeve and bottom hems, folds.
fn shirt_details(sleeve: [(f64, f64); 2], d: &mut Vec<Detail>) {
    let armhole = flat(&path((232.0, 42.0), |b| b.curve_to((250.0, 110.0), (264.0, 190.0), (262.0, 250.0))));
    both(armhole, d, Detail::Seam);
    // The sleeve hem, 20 mm in from its edge.
    let (a, b) = (Point::new(sleeve[0].0, sleeve[0].1), Point::new(sleeve[1].0, sleeve[1].1));
    let along = a - b;
    let n = kurbo::Vec2::new(-along.y, along.x) / along.hypot().max(1e-9);
    let inward = if n.x < 0.0 { n } else { -n };
    let hem = vec![a + inward * 20.0 - along * 0.04, b + inward * 20.0 + along * 0.04];
    both(hem, d, Detail::Stitch);
    let bottom = flat(&path((-268.0, 702.0), |b| b.quad_to((0.0, 712.0), (268.0, 702.0))));
    d.push(Detail::Stitch(bottom));
    for f in [line(&[(250.0, 280.0), (190.0, 340.0), (150.0, 420.0)]), line(&[(90.0, 560.0), (60.0, 690.0)])] {
        let pts = flat(&path((f[0].x, f[0].y), |b| {
            for p in &f[1..] {
                b.line_to(*p);
            }
        }));
        both(pts, d, |p| Detail::Fold(p, 14.0));
    }
}

fn tshirt() -> Shape {
    let sleeve = [(395.0, 178.0), (318.0, 292.0)];
    let body = mirrored(&shirt_half(95.0, sleeve));
    // Inside the back of the neck, seen through the neck opening.
    let back = mirrored(&path((0.0, 24.0), |b| {
        b.quad_to((60.0, 22.0), (95.0, 0.0));
        b.line_to((95.0, 110.0));
        b.line_to((0.0, 110.0));
    }));
    let back_rib = {
        let curve = flat(&path((-95.0, 0.0), |b| b.quad_to((0.0, 48.0), (95.0, 0.0))));
        let mut p = curve.clone();
        p.extend(offset(&curve, -13.0).into_iter().rev());
        p
    };
    let neckline = flat(&path((-95.0, 0.0), |b| {
        b.curve_to((-92.0, 47.5), (-55.0, 95.0), (0.0, 95.0));
        b.curve_to((55.0, 95.0), (92.0, 47.5), (95.0, 0.0));
    }));
    let mut rib = neckline.clone();
    rib.extend(offset(&neckline, 17.0).into_iter().rev());
    let mut d = vec![Detail::Stitch(offset(&neckline, 19.5))];
    shirt_details(sleeve, &mut d);
    Shape {
        panels: vec![
            Panel::new(back).tone(0.62),
            Panel::new(back_rib).tone(0.70).rib(),
            Panel::new(body),
            Panel::new(rib).tone(0.98).rib().outlined(),
        ],
        details: d,
    }
}

fn polo() -> Shape {
    let sleeve = [(372.0, 162.0), (305.0, 268.0)];
    let body = mirrored(&shirt_half(40.0, sleeve));
    let inside = mirrored(&path((0.0, 0.0), |b| {
        b.line_to((75.0, 2.0));
        b.line_to((20.0, 80.0));
        b.line_to((0.0, 80.0));
    }));
    let collar = mirrored(&path((0.0, -10.0), |b| {
        b.quad_to((70.0, -10.0), (102.0, 2.0));
        b.quad_to((125.0, 30.0), (132.0, 66.0));
        b.line_to((56.0, 128.0));
        b.quad_to((30.0, 96.0), (16.0, 70.0));
        b.quad_to((60.0, 30.0), (60.0, 10.0));
        b.quad_to((30.0, 4.0), (0.0, 4.0));
    }));
    let placket = line(&[(-17.0, 70.0), (17.0, 70.0), (17.0, 215.0), (-17.0, 215.0)]);
    // Rib cuffs on the sleeves.
    let (a, b) = (Point::new(sleeve[0].0, sleeve[0].1), Point::new(sleeve[1].0, sleeve[1].1));
    let along = a - b;
    let n = kurbo::Vec2::new(-along.y, along.x) / along.hypot().max(1e-9);
    let inward = if n.x < 0.0 { n } else { -n };
    let cuff = vec![a, b, b + inward * 26.0, a + inward * 26.0];
    let mut d =
        vec![Detail::Stitch(line(&[(-13.0, 74.0), (-13.0, 211.0), (13.0, 211.0), (13.0, 74.0)])), Detail::Seam(line(&[(0.0, 74.0), (0.0, 212.0)]))];
    shirt_details(sleeve, &mut d);
    // Its own cuffs replace the hem stitching of the shirt's.
    d.retain(|x| !matches!(x, Detail::Stitch(p) if p.len() == 2));
    for y in [105.0, 150.0, 195.0] {
        d.push(Detail::Button(Point::new(0.0, y), 5.5));
    }
    Shape {
        panels: vec![
            Panel::new(inside).tone(0.55),
            Panel::new(body),
            Panel::new(mirror(&cuff)).rib().outlined(),
            Panel::new(cuff).rib().outlined(),
            Panel::new(placket).outlined(),
            Panel::new(collar).rib().tone(1.12).outlined(),
        ],
        details: d,
    }
}

fn hoodie() -> Shape {
    let hood = mirrored(&path((0.0, -175.0), |b| {
        b.curve_to((95.0, -178.0), (165.0, -110.0), (168.0, -20.0));
        b.quad_to((165.0, 40.0), (120.0, 60.0));
        b.line_to((0.0, 60.0));
    }));
    // The hood's lining, seen where it opens round the neck.
    let lining = mirrored(&path((0.0, -120.0), |b| {
        b.curve_to((70.0, -122.0), (120.0, -70.0), (118.0, -5.0));
        b.quad_to((110.0, 50.0), (0.0, 112.0));
    }));
    let body = mirrored(&path((0.0, 112.0), |b| {
        b.quad_to((70.0, 60.0), (100.0, 0.0));
        b.line_to((240.0, 48.0));
        b.curve_to((300.0, 120.0), (352.0, 330.0), (372.0, 650.0));
        b.line_to((300.0, 650.0));
        b.quad_to((294.0, 420.0), (284.0, 266.0));
        b.quad_to((280.0, 470.0), (282.0, 640.0));
        b.line_to((0.0, 640.0));
    }));
    let cuff = line(&[(372.0, 648.0), (370.0, 720.0), (302.0, 720.0), (300.0, 648.0)]);
    let band = line(&[(-282.0, 638.0), (282.0, 638.0), (278.0, 712.0), (-278.0, 712.0)]);
    let pocket = line(&[(-112.0, 452.0), (112.0, 452.0), (168.0, 596.0), (164.0, 632.0), (-164.0, 632.0), (-168.0, 596.0)]);
    let mut d = vec![];
    both(flat(&path((240.0, 48.0), |b| b.curve_to((262.0, 120.0), (284.0, 200.0), (284.0, 266.0)))), &mut d, Detail::Seam);
    // The pocket's openings.
    both(line(&[(112.0, 452.0), (168.0, 596.0)]), &mut d, Detail::Seam);
    d.push(Detail::Stitch(line(&[
        (-160.0, 627.0),
        (160.0, 627.0),
        (164.0, 598.0),
        (114.0, 470.0),
        (-114.0, 470.0),
        (-164.0, 598.0),
        (-160.0, 627.0),
    ])));
    for f in [line(&[(320.0, 300.0), (335.0, 470.0)]), line(&[(240.0, 320.0), (200.0, 420.0)]), line(&[(110.0, 640.0), (100.0, 520.0)])] {
        both(f, &mut d, |p| Detail::Fold(p, 14.0));
    }
    // The drawstrings, hanging from the hood's edge.
    for x in [-38.0, 38.0] {
        let s = if x < 0.0 { -1.0 } else { 1.0 };
        let cord = flat(&path((x, 88.0), |b| b.curve_to((x + s * 4.0, 180.0), (x - s * 2.0, 260.0), (x + s * 6.0, 330.0))));
        d.push(Detail::Cord(cord, 6.0));
        d.push(Detail::Cord(line(&[(x + s * 6.0, 330.0), (x + s * 7.0, 352.0)]), 7.5));
    }
    Shape {
        panels: vec![
            Panel::new(hood).tone(0.92),
            Panel::new(lining).tone(0.58),
            Panel::new(body),
            Panel::new(mirror(&cuff)).rib().outlined(),
            Panel::new(cuff).rib().outlined(),
            Panel::new(band).rib().outlined(),
            Panel::new(pocket).outlined(),
        ],
        details: d,
    }
}

fn cap() -> Shape {
    let crown = mirrored(&path((0.0, 0.0), |b| {
        b.curve_to((70.0, 0.0), (114.0, 60.0), (114.0, 152.0));
        b.line_to((0.0, 160.0));
    }));
    let brim_edge =
        |drop: f64| flat(&path((-120.0, 144.0 + drop), |b| b.curve_to((-84.0, 262.0 + drop), (84.0, 262.0 + drop), (120.0, 144.0 + drop))));
    let top = flat(&path((120.0, 144.0), |b| b.curve_to((70.0, 166.0), (-70.0, 166.0), (-120.0, 144.0))));
    let mut under = brim_edge(7.0);
    under.extend(top.iter().copied());
    let mut brim = brim_edge(0.0);
    brim.extend(top.iter().copied());
    let mut d = vec![Detail::Seam(line(&[(0.0, 8.0), (0.0, 154.0)]))];
    both(flat(&path((4.0, 4.0), |b| b.curve_to((58.0, 22.0), (92.0, 90.0), (100.0, 152.0)))), &mut d, Detail::Seam);
    // Rows of stitching round the brim.
    let front = brim_edge(0.0);
    for k in 1..=5 {
        // Each row stops short of where the brim meets the crown.
        let top_y = |x: f64| 144.0 + 16.5 * (1.0 - (x / 120.0).powi(2)) + 4.0;
        let row: Vec<Point> = offset(&front, 7.0 * f64::from(k)).into_iter().filter(|p| p.x.abs() < 116.0 && p.y > top_y(p.x)).collect();
        d.push(Detail::Stitch(row));
    }
    d.push(Detail::Stitch(offset(&top, 4.0)));
    d.push(Detail::Button(Point::new(0.0, 5.0), 8.0));
    Shape { panels: vec![Panel::new(crown), Panel::new(under).tone(0.5), Panel::new(brim).tone(1.03).outlined()], details: d }
}

fn tote() -> Shape {
    let handle = |lift: f64| {
        let outer = flat(&path((-108.0, 12.0 - lift), |b| b.curve_to((-108.0, -300.0 - lift), (108.0, -300.0 - lift), (108.0, 12.0 - lift))));
        let inner = flat(&path((78.0, 12.0 - lift), |b| b.curve_to((78.0, -262.0 - lift), (-78.0, -262.0 - lift), (-78.0, 12.0 - lift))));
        let mut p = outer;
        p.extend(inner);
        p
    };
    let bag = mirrored(&path((0.0, 0.0), |b| {
        b.line_to((186.0, 0.0));
        b.quad_to((192.0, 0.0), (192.0, 6.0));
        b.line_to((194.0, 414.0));
        b.quad_to((194.0, 420.0), (188.0, 420.0));
        b.line_to((0.0, 420.0));
    }));
    let mut d = vec![Detail::Stitch(line(&[(-190.0, 26.0), (190.0, 26.0)]))];
    both(line(&[(183.0, 30.0), (185.0, 410.0)]), &mut d, Detail::Stitch);
    for x in [-93.0, 93.0] {
        // The handle sewn on with a box and a cross.
        let (l, r) = (x - 13.0, x + 13.0);
        d.push(Detail::Stitch(line(&[(l, 8.0), (r, 8.0), (r, 44.0), (l, 44.0), (l, 8.0), (r, 44.0)])));
        d.push(Detail::Stitch(line(&[(r, 8.0), (l, 44.0)])));
    }
    d.push(Detail::Stitch(offset(&flat(&path((-104.0, 8.0), |b| b.curve_to((-104.0, -294.0), (104.0, -294.0), (104.0, 8.0)))), 0.0)));
    d.push(Detail::Stitch(flat(&path((82.0, 8.0), |b| b.curve_to((82.0, -268.0), (-82.0, -268.0), (-82.0, 8.0))))));
    d.push(Detail::Fold(line(&[(-120.0, 380.0), (-60.0, 300.0)]), 16.0));
    Shape { panels: vec![Panel::new(handle(14.0)).tone(0.74), Panel::new(bag), Panel::new(handle(0.0)).outlined()], details: d }
}

fn towel() -> Shape {
    let towel = line(&[(-210.0, 0.0), (210.0, 0.0), (212.0, 720.0), (-212.0, 720.0)]);
    // The flat woven border band near the bottom.
    let border = line(&[(-211.0, 575.0), (211.0, 575.0), (211.5, 625.0), (-211.5, 625.0)]);
    let d = vec![
        Detail::Stitch(line(&[(-208.0, 12.0), (208.0, 12.0)])),
        Detail::Stitch(line(&[(-210.0, 708.0), (210.0, 708.0)])),
        Detail::Fold(line(&[(-120.0, 40.0), (-110.0, 520.0)]), 22.0),
        Detail::Fold(line(&[(90.0, 30.0), (100.0, 540.0)]), 18.0),
    ];
    Shape { panels: vec![Panel::new(towel), Panel::new(border).tone(0.94).rib().outlined()], details: d }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cmd, Needle};

    fn square(size: f64) -> Plan {
        // Rows of stitches filling a square `size` mm wide.
        let mut needles = vec![];
        let mut y = 0.0;
        let mut flip = false;
        while y <= size {
            let (a, b) = if flip { (size, 0.0) } else { (0.0, size) };
            needles.push(Needle { cmd: Cmd::Stitch, x: a, y });
            needles.push(Needle { cmd: Cmd::Stitch, x: b, y });
            y += 0.4;
            flip = !flip;
        }
        Plan { threads: vec![[200, 30, 30]], needles, ..Default::default() }
    }

    fn spec(g: Garment, close_up: bool) -> Spec {
        Spec {
            garment: g,
            placement: g.placement(None).unwrap(),
            look: Look { texture: g.texture(), color: g.default_color() },
            close_up,
            width: 400,
        }
    }

    fn px(img: &Image, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * img.width + x) * 4) as usize;
        [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2], img.rgba[i + 3]]
    }

    /// Image pixel of garment point `p` (mm).
    fn at(m: &Mockup, p: (f64, f64)) -> [u8; 4] {
        let s = f64::from(m.image.width) / m.view.width();
        px(&m.image, ((p.0 - m.view.x0) * s) as u32, ((p.1 - m.view.y0) * s) as u32)
    }

    #[test]
    fn every_garment_draws_with_the_design_at_its_placement() {
        for g in Garment::ALL {
            assert_eq!(Garment::from_id(g.id()), Some(g));
            for pl in g.placements() {
                let mut sp = spec(g, false);
                sp.placement = *pl;
                let m = render(&square(40.0), &sp).unwrap();
                assert_eq!(m.image.width, 400);
                assert!(m.warnings.is_empty(), "{g:?} {}: {:?}", pl.id, m.warnings);
                // The design's red covers the placement's centre; the garment's fabric is not red.
                let c = at(&m, pl.center);
                assert!(c[0] > c[1] + 50, "{g:?} {}: {c:?}", pl.id);
                // The backdrop shows in the corner, opaque.
                assert_eq!(px(&m.image, 1, 1)[3], 255);
            }
        }
    }

    #[test]
    fn the_garment_is_drawn_in_its_colour_and_the_backdrop_around_it() {
        let mut sp = spec(Garment::Polo, false);
        sp.look.color = [20, 120, 40];
        let m = render(&Plan::default(), &sp).unwrap();
        let body = at(&m, (-150.0, 500.0));
        assert!(body[1] > body[0] + 30 && body[1] > body[2] + 30, "green polo: {body:?}");
        let back = px(&m.image, 2, 2);
        assert!(back[0] > 200 && back[0].abs_diff(back[1]) < 8, "grey backdrop: {back:?}");
        assert_eq!(m.design, None);
        assert_eq!(m.warnings.len(), 1, "nothing stitched");
    }

    #[test]
    fn a_design_too_big_for_its_place_is_drawn_with_a_warning() {
        let m = render(&square(90.0), &spec(Garment::Cap, false)).unwrap();
        assert_eq!(m.design.map(|d| d.0.round()), Some(90.0));
        assert!(m.warnings.iter().any(|w| w.contains("115 × 57")), "{:?}", m.warnings);
    }

    #[test]
    fn a_close_up_shows_the_design_larger() {
        let far = render(&square(40.0), &spec(Garment::TShirt, false)).unwrap();
        let near = render(&square(40.0), &spec(Garment::TShirt, true)).unwrap();
        assert!(near.view.width() < far.view.width() / 3.0);
        assert!(near.view.contains(Point::new(95.0, 190.0)));
    }

    #[test]
    fn bad_sizes_are_refused() {
        let mut sp = spec(Garment::Towel, false);
        sp.width = 50;
        assert!(render(&Plan::default(), &sp).is_err());
        sp.width = 99_999;
        assert!(render(&Plan::default(), &sp).is_err());
    }

    #[test]
    fn placements_are_found_by_id() {
        assert_eq!(Garment::Cap.placement(Some("leftChest")), None);
        assert_eq!(Garment::TShirt.placement(Some("LEFTCHEST")).map(|p| p.id), Some("leftChest"));
        assert_eq!(Garment::Towel.placement(None).map(|p| p.id), Some("lower"));
        assert_eq!(placement_ids(), vec!["leftChest", "rightChest", "center", "lower"]);
    }

    /// Writes every garment's mockup to `$EMB_DUMP` to look at (not run by default).
    #[test]
    #[ignore = "writes images to look at"]
    fn dump_mockups() {
        let Ok(dir) = std::env::var("EMB_DUMP") else { return };
        for g in Garment::ALL {
            for close in [false, true] {
                let mut sp = spec(g, close);
                sp.width = 1000;
                let m = render(&square(50.0), &sp).unwrap();
                let path = format!("{dir}/mockup-{}-{}.ppm", g.id(), if close { "close" } else { "whole" });
                let mut out = format!("P6 {} {} 255\n", m.image.width, m.image.height).into_bytes();
                for c in m.image.rgba.chunks(4) {
                    out.extend(&c[..3]);
                }
                std::fs::write(path, out).unwrap();
            }
        }
    }
}
