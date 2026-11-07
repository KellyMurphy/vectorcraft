//! The Realistic view of a design (E9.2): each stitch drawn as a round, twisted thread lit from the
//! upper left, lying on a fabric texture drawn in code, so the design shows as it will look sewn.
//! Satin and fill rows catch the light by their stitch angle (a thread shines across its length,
//! not along it), threads cast soft shadows on the fabric and on the threads sewn before them, and
//! a jump that isn't trimmed shows as the loose thread it leaves.
//!
//! Nothing here comes from another program: the thread is a cylinder shaded with a diffuse term and
//! an anisotropic highlight (the classic hair/fibre model), and the fabrics are small procedural
//! patterns (a plain weave, a 2/1 twill, jersey loops, terry loops, leather grain, felt fibres).

use kurbo::{Affine, Point, Rect};
use vectorcraft_doc::Document;

use crate::{Cmd, Plan};

/// The thread's width as it lies on the fabric, mm: a 40-weight thread flattens to a little wider
/// than it is round, so satin and fill rows sewn 0.4 mm apart close into one surface.
pub const THREAD_MM: f64 = 0.45;

/// The largest image rendered, in pixels.
pub const MAX_PIXELS: u64 = 40_000_000;

/// How a fabric's surface is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Texture {
    /// A plain weave: warp and weft over one, under one (cotton, linen, poplin).
    Plain,
    /// A 2/1 twill in one colour (cap fronts, drill, gabardine).
    Twill,
    /// A 3/1 twill with a dyed warp and an undyed weft (denim).
    Denim,
    /// Jersey knit loops (T-shirts, polo shirts).
    Knit,
    /// Terry loops (towels, bathrobes).
    Terry,
    /// Leather grain.
    Leather,
    /// Pressed fibres (felt, fleece seen from a distance).
    Felt,
}

impl Texture {
    pub const ALL: [Texture; 7] = [Texture::Plain, Texture::Twill, Texture::Denim, Texture::Knit, Texture::Terry, Texture::Leather, Texture::Felt];

    pub fn id(self) -> &'static str {
        match self {
            Texture::Plain => "plain",
            Texture::Twill => "twill",
            Texture::Denim => "denim",
            Texture::Knit => "knit",
            Texture::Terry => "terry",
            Texture::Leather => "leather",
            Texture::Felt => "felt",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Texture::Plain => "Plain Weave",
            Texture::Twill => "Twill",
            Texture::Denim => "Denim",
            Texture::Knit => "Knit",
            Texture::Terry => "Terry",
            Texture::Leather => "Leather",
            Texture::Felt => "Felt",
        }
    }

    pub fn from_id(id: &str) -> Option<Texture> {
        Texture::ALL.into_iter().find(|t| t.id() == id)
    }

    /// The texture of a built-in fabric profile ([`crate::fabrics::BUILTIN`]); others are plain.
    pub fn of_fabric(id: &str) -> Texture {
        match id {
            "knit" => Texture::Knit,
            "towel" => Texture::Terry,
            "cap" => Texture::Twill,
            "leather" => Texture::Leather,
            "denim" => Texture::Denim,
            _ => Texture::Plain,
        }
    }

    /// The colour the fabric has unless one is set (sRGB).
    pub fn default_color(self) -> [u8; 3] {
        match self {
            Texture::Plain => [236, 230, 214],
            Texture::Twill => [196, 182, 150],
            Texture::Denim => [58, 84, 128],
            Texture::Knit => [238, 238, 234],
            Texture::Terry => [248, 247, 243],
            Texture::Leather => [112, 72, 46],
            Texture::Felt => [150, 150, 152],
        }
    }
}

/// The [`vectorcraft_doc::PreviewLook::texture`] that draws no fabric: only the thread, over what
/// is under it.
pub const NO_FABRIC: &str = "none";

/// What the fabric looks like.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    pub texture: Texture,
    pub color: [u8; 3],
    /// Whether the fabric is drawn at all (`false`: the threads alone, on a transparent image).
    pub fabric: bool,
}

impl Look {
    /// The document's look ([`vectorcraft_doc::EmbroiderySetup::preview`]): its texture, else its
    /// fabric profile's; its colour, else the texture's own; no fabric for [`NO_FABRIC`].
    pub fn of(doc: &Document) -> Look {
        let e = &doc.embroidery;
        let set = e.preview.as_deref();
        let fabric = set.and_then(|p| p.texture.as_deref()) != Some(NO_FABRIC);
        let texture = set
            .and_then(|p| p.texture.as_deref())
            .and_then(Texture::from_id)
            .or_else(|| e.fabric.as_deref().map(|f| Texture::of_fabric(&f.id)))
            .unwrap_or(Texture::Plain);
        let color = set.and_then(|p| p.color.as_deref()).and_then(parse_hex).unwrap_or_else(|| texture.default_color());
        Look { texture, color, fabric }
    }

    /// The look of `texture` in its own colour.
    pub fn plain(texture: Texture) -> Look {
        Look { texture, color: texture.default_color(), fabric: true }
    }
}

/// `#rrggbb` (the `#` optional) as sRGB.
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#').unwrap_or(s);
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let c = |i: usize| h.get(i..i + 2).and_then(|x| u8::from_str_radix(x, 16).ok());
    Some([c(0)?, c(2)?, c(4)?])
}

/// An RGBA image, 8 bits a channel, not premultiplied, rows top to bottom.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Where to draw: the image's size, the transform from design millimetres to its pixels, and the
/// areas (mm) the fabric covers. The stitches are drawn wherever they fall, fabric or not.
#[derive(Clone, Debug)]
pub struct View {
    pub width: u32,
    pub height: u32,
    pub to_px: Affine,
    pub fabric: Vec<Rect>,
}

/// The light: from the upper left, 50° above the fabric (a unit vector; screen y points down, z
/// out of the screen).
const LIGHT: [f32; 3] = [-0.45, -0.55, 0.70];

/// A float RGBA canvas being drawn on (straight alpha, 0–1).
pub(crate) struct Canvas {
    pub(crate) w: usize,
    pub(crate) h: usize,
    pub(crate) px: Vec<[f32; 4]>,
}

impl Canvas {
    pub(crate) fn get(&mut self, x: usize, y: usize) -> Option<&mut [f32; 4]> {
        if x >= self.w || y >= self.h {
            return None;
        }
        self.px.get_mut(y * self.w + x)
    }

    /// `c` over the pixel, `a` of it.
    pub(crate) fn blend(&mut self, x: usize, y: usize, c: [f32; 3], a: f32) {
        let Some(p) = self.get(x, y) else { return };
        let out_a = a + p[3] * (1.0 - a);
        if out_a <= 0.0 {
            return;
        }
        for i in 0..3 {
            p[i] = (c[i] * a + p[i] * p[3] * (1.0 - a)) / out_a;
        }
        p[3] = out_a;
    }

    /// Darken the pixel by `k` (0 none, 1 black), keeping its alpha; a shadow on bare (transparent)
    /// canvas is left out.
    pub(crate) fn shade(&mut self, x: usize, y: usize, k: f32) {
        let Some(p) = self.get(x, y) else { return };
        for c in p.iter_mut().take(3) {
            *c *= 1.0 - k;
        }
    }
}

/// Render `plan` as it would look sewn on `look`'s fabric.
pub fn render(plan: &Plan, look: &Look, view: &View) -> Result<Image, String> {
    let (w, h) = (view.width, view.height);
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(format!("an image {w} × {h} pixels is too big to render (at most {MAX_PIXELS} pixels)"));
    }
    let coeffs = view.to_px.as_coeffs();
    if coeffs.iter().any(|c| !c.is_finite()) || view.to_px.determinant().abs() < 1e-12 {
        return Err("the view can't be drawn".into());
    }
    let (w, h) = (w as usize, h as usize);
    let mut cv = Canvas { w, h, px: vec![[0.0; 4]; w * h] };
    // Pixels per mm (the transform may turn but doesn't stretch one way more than the other).
    let scale = view.to_px.determinant().abs().sqrt();
    if look.fabric {
        fabric(&mut cv, look, view, scale);
    }
    threads(&mut cv, plan, view.to_px, scale as f32);
    let mut rgba = Vec::with_capacity(w * h * 4);
    for p in &cv.px {
        let a = p[3].clamp(0.0, 1.0);
        rgba.extend([to_u8(p[0]), to_u8(p[1]), to_u8(p[2]), (a * 255.0).round() as u8]);
    }
    Ok(Image { width: view.width, height: view.height, rgba })
}

pub(crate) fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

pub(crate) fn rgb(c: [u8; 3]) -> [f32; 3] {
    c.map(|v| f32::from(v) / 255.0)
}

// ---- Fabric ----

/// A hash of two integers, 0–1.
fn hash(i: i64, j: i64) -> f32 {
    let mut x = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (j as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 32;
    (x >> 40) as f32 / (1u64 << 24) as f32
}

/// Smooth value noise, 0–1, cells one unit wide.
pub(crate) fn noise(x: f64, y: f64) -> f32 {
    let (i, j) = (x.floor(), y.floor());
    let (fx, fy) = ((x - i) as f32, (y - j) as f32);
    let (i, j) = (i as i64, j as i64);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (u, v) = (s(fx), s(fy));
    let a = hash(i, j) + (hash(i + 1, j) - hash(i, j)) * u;
    let b = hash(i, j + 1) + (hash(i + 1, j + 1) - hash(i, j + 1)) * u;
    a + (b - a) * v
}

pub(crate) fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The fabric's pattern at (x, y) mm: a brightness factor around 1 and how far the colour moves
/// toward the undyed weft (denim's white threads).
fn texture_at(t: Texture, x: f64, y: f64) -> (f32, f32) {
    match t {
        Texture::Plain => weave(x, y, 0.3, |i, j| (i + j).rem_euclid(2) == 0, 0.0),
        Texture::Twill => weave(x, y, 0.28, |i, j| (i - j).rem_euclid(3) < 2, 0.0),
        Texture::Denim => weave(x, y, 0.3, |i, j| (i - j).rem_euclid(4) < 3, 0.75),
        Texture::Knit => knit(x, y),
        Texture::Terry => terry(x, y),
        Texture::Leather => leather(x, y),
        Texture::Felt => {
            let n = 0.55 * noise(x / 0.25, y / 0.6) + 0.3 * noise(x / 0.9 + 7.0, y / 0.9) + 0.15 * noise(x / 4.0, y / 4.0 + 3.0);
            (0.88 + 0.24 * n, 0.0)
        }
    }
}

/// A woven fabric, yarns `pitch` mm apart: `warp_over(i, j)` says whether the warp (running down)
/// lies on top in cell (i, j). `weft_tint` moves the colour toward undyed thread where the weft
/// shows.
fn weave(x: f64, y: f64, pitch: f64, warp_over: impl Fn(i64, i64) -> bool, weft_tint: f32) -> (f32, f32) {
    let (gx, gy) = (x / pitch, y / pitch);
    let (i, j) = (gx.floor() as i64, gy.floor() as i64);
    let (fx, fy) = ((gx - gx.floor()) as f32, (gy - gy.floor()) as f32);
    let pi = std::f32::consts::PI;
    if warp_over(i, j) {
        // A warp yarn: round across x, dipping where it goes under at the cell's ends.
        let across = (pi * fx).sin().sqrt();
        let along = 0.82 + 0.18 * (pi * fy).sin();
        (0.72 + 0.36 * across * along * (0.94 + 0.12 * hash(i, 7)), 0.0)
    } else {
        let across = (pi * fy).sin().sqrt();
        let along = 0.82 + 0.18 * (pi * fx).sin();
        (0.72 + 0.36 * across * along * (0.94 + 0.12 * hash(3, j)), weft_tint)
    }
}

/// Jersey: columns of V-shaped loops, 0.9 mm wide and 0.7 mm tall.
fn knit(x: f64, y: f64) -> (f32, f32) {
    let (gx, gy) = (x / 0.9, y / 0.7);
    let (i, j) = (gx.floor() as i64, gy.floor() as i64);
    let fx = (gx - gx.floor()) as f32 - 0.5;
    let fy = (gy - gy.floor()) as f32;
    // Each leg leans in toward the loop's foot.
    let centre = 0.30 - 0.20 * fy;
    let d = (fx.abs() - centre).abs();
    let leg = 0.17;
    let v = if d < leg { 0.78 + 0.34 * (1.0 - (d / leg).powi(2)).sqrt() } else { 0.66 };
    (v * (0.95 + 0.1 * hash(i, j)), 0.0)
}

/// Terry: loops standing up from the cloth, packed close (about 0.6 mm apart) so little of the
/// ground shows between them.
fn terry(x: f64, y: f64) -> (f32, f32) {
    let cell = 0.6;
    let (gx, gy) = (x / cell, y / cell);
    let (ci, cj) = (gx.floor() as i64, gy.floor() as i64);
    let mut top = 0.0_f32;
    for di in -1..=1 {
        for dj in -1..=1 {
            let (i, j) = (ci + di, cj + dj);
            let cx = i as f64 + f64::from(hash(i, j));
            let cy = j as f64 + f64::from(hash(j, i + 11));
            let r = 0.65 + 0.3 * f64::from(hash(i + 5, j - 3));
            let d = ((gx - cx).powi(2) + (gy - cy).powi(2)).sqrt() / r;
            if d < 1.0 {
                // A loop seen from above: a soft dome with a brighter rim where the yarn turns.
                let d = d as f32;
                let dome = (1.0 - d * d).sqrt();
                let rim = 1.0 - ((d - 0.7) / 0.3).powi(2).min(1.0);
                top = top.max(0.55 * dome + 0.45 * rim);
            }
        }
    }
    (0.80 + 0.26 * top, 0.0)
}

/// Leather: a grain of small cells with creases between them, over slow variation.
fn leather(x: f64, y: f64) -> (f32, f32) {
    let cell = 1.1;
    let (gx, gy) = (x / cell, y / cell);
    let (ci, cj) = (gx.floor() as i64, gy.floor() as i64);
    let (mut f1, mut f2) = (f64::MAX, f64::MAX);
    for di in -1..=1 {
        for dj in -1..=1 {
            let (i, j) = (ci + di, cj + dj);
            let d = ((gx - i as f64 - f64::from(hash(i, j))).powi(2) + (gy - j as f64 - f64::from(hash(j + 3, i))).powi(2)).sqrt();
            if d < f1 {
                f2 = f1;
                f1 = d;
            } else if d < f2 {
                f2 = d;
            }
        }
    }
    let crease = smoothstep(0.0, 0.12, (f2 - f1) as f32);
    let slow = noise(x / 6.0, y / 6.0);
    (0.80 + 0.2 * crease + 0.12 * (slow - 0.5), 0.0)
}

/// How finely `t`'s pattern repeats, mm (below about two pixels it is blended away: a pattern finer
/// than the pixels would only make moiré).
fn pitch(t: Texture) -> f64 {
    match t {
        Texture::Plain | Texture::Denim => 0.3,
        Texture::Twill => 0.28,
        Texture::Knit => 0.7,
        Texture::Terry => 0.6,
        Texture::Leather => 1.1,
        Texture::Felt => 0.6,
    }
}

/// The texture's average brightness (what it fades to when too fine to draw).
fn mean(t: Texture) -> f32 {
    match t {
        Texture::Plain | Texture::Twill | Texture::Denim => 0.95,
        Texture::Knit => 0.86,
        Texture::Terry => 1.0,
        Texture::Leather => 0.98,
        Texture::Felt => 1.0,
    }
}

fn fabric(cv: &mut Canvas, look: &Look, view: &View, scale: f64) {
    if view.fabric.is_empty() {
        return;
    }
    let inv = view.to_px.inverse();
    let surface = Surface::new(look, scale);
    // Only the pixels the fabric's areas cover.
    let mut px_box = Rect::new(f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for r in &view.fabric {
        px_box = px_box.union(view.to_px.transform_rect_bbox(*r));
    }
    let x0 = px_box.x0.floor().max(0.0) as usize;
    let y0 = px_box.y0.floor().max(0.0) as usize;
    let x1 = (px_box.x1.ceil().max(0.0) as usize).min(cv.w);
    let y1 = (px_box.y1.ceil().max(0.0) as usize).min(cv.h);
    for y in y0..y1 {
        for x in x0..x1 {
            let p = inv * Point::new(x as f64 + 0.5, y as f64 + 0.5);
            if !view.fabric.iter().any(|r| r.contains(p)) {
                continue;
            }
            cv.blend(x, y, surface.at(p.x, p.y), 1.0);
        }
    }
}

/// A fabric's look at one zoom: its colour at any point, its pattern faded out where it would be
/// finer than the pixels.
pub(crate) struct Surface {
    texture: Texture,
    base: [f32; 3],
    detail: f32,
    flat: f32,
}

impl Surface {
    /// `look` drawn `scale` pixels per mm.
    pub(crate) fn new(look: &Look, scale: f64) -> Surface {
        Surface {
            texture: look.texture,
            base: rgb(look.color),
            detail: smoothstep(1.5, 4.0, (pitch(look.texture) * scale) as f32),
            flat: mean(look.texture),
        }
    }

    /// The colour at (x, y) mm (linear 0–1 sRGB values).
    pub(crate) fn at(&self, x: f64, y: f64) -> [f32; 3] {
        let undyed = [0.90, 0.89, 0.86];
        let (v, tint) = if self.detail > 0.0 { texture_at(self.texture, x, y) } else { (self.flat, 0.0) };
        let v = self.flat + (v - self.flat) * self.detail;
        let tint = tint * self.detail.max(0.35);
        std::array::from_fn(|i| (self.base[i] + (undyed[i] - self.base[i]) * tint) * v)
    }
}

// ---- Thread ----

/// One piece of thread to draw, in pixels.
struct Piece {
    a: [f32; 2],
    b: [f32; 2],
    color: [f32; 3],
}

/// The design's stitches, and the loose thread of jumps that aren't trimmed, in sewing order.
fn pieces(plan: &Plan, to_px: Affine) -> Vec<Piece> {
    let mut out = vec![];
    let mut thread = 0usize;
    let mut at: Option<[f32; 2]> = None;
    let mut cut = true;
    let px = |x: f64, y: f64| {
        let p = to_px * Point::new(x, y);
        [p.x as f32, p.y as f32]
    };
    for n in &plan.needles {
        let p = px(n.x, n.y);
        let color = rgb(plan.threads.get(thread).copied().unwrap_or([0; 3]));
        match n.cmd {
            Cmd::Stitch => {
                if let Some(a) = at {
                    out.push(Piece { a, b: p, color });
                }
                cut = false;
                at = Some(p);
            }
            Cmd::Jump => {
                if let (Some(a), false) = (at, cut) {
                    out.push(Piece { a, b: p, color });
                }
                at = Some(p);
            }
            Cmd::Trim => {
                cut = true;
                at = Some(p);
            }
            Cmd::ColorChange => {
                thread += 1;
                cut = true;
                at = Some(p);
            }
            Cmd::End => break,
        }
    }
    out
}

/// The point of segment `a`–`b` nearest `p`, as a fraction along it.
pub(crate) fn nearest_t(p: [f32; 2], a: [f32; 2], d: [f32; 2], len2: f32) -> f32 {
    if len2 <= 0.0 {
        return 0.0;
    }
    (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / len2).clamp(0.0, 1.0)
}

/// For each row a capsule of radius `r` round `a`–`b` touches, the columns it covers.
pub(crate) fn capsule_rows(a: [f32; 2], b: [f32; 2], r: f32, (w, h): (usize, usize), mut f: impl FnMut(usize, usize, usize)) {
    let y0 = (a[1].min(b[1]) - r - 1.0).floor().max(0.0);
    let y1 = (a[1].max(b[1]) + r + 1.0).ceil().min(h as f32);
    if y0 >= y1 {
        return;
    }
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    for y in y0 as usize..y1 as usize {
        let yc = y as f32 + 0.5;
        // The part of the segment within r of this row, widened by r.
        let (lo, hi) = if dy.abs() < 1e-6 {
            (a[0].min(b[0]), a[0].max(b[0]))
        } else {
            let t0 = ((yc - r - a[1]) / dy).clamp(0.0, 1.0);
            let t1 = ((yc + r - a[1]) / dy).clamp(0.0, 1.0);
            let (xa, xb) = (a[0] + dx * t0, a[0] + dx * t1);
            (xa.min(xb), xa.max(xb))
        };
        let x0 = (lo - r - 1.0).floor().max(0.0) as usize;
        let x1 = ((hi + r + 1.0).ceil().max(0.0) as usize).min(w);
        if x0 < x1 {
            f(y, x0, x1);
        }
    }
}

pub(crate) fn threads(cv: &mut Canvas, plan: &Plan, to_px: Affine, scale: f32) {
    let r = (THREAD_MM as f32 * scale / 2.0).max(0.35);
    // Below a pixel or so a thread's shading can't show: it is drawn flat in its colour, a little
    // darker than its lit side, which is how a design looks from across a room.
    let shaded = smoothstep(0.6, 2.0, r);
    let twist_pitch = 0.55 * scale;
    let twist = smoothstep(3.0, 7.0, twist_pitch) * 0.13;
    let [lx, ly, lz] = LIGHT;
    // The highlight's direction: halfway between the light and the eye (straight above).
    let hn = (lx * lx + ly * ly + (lz + 1.0) * (lz + 1.0)).sqrt();
    let half = [lx / hn, ly / hn, (lz + 1.0) / hn];
    // Shadows fall down and right, away from the light.
    let shadow_off = [r * 0.55, r * 0.75];
    let shadow_r = r * 1.25;
    let size = (cv.w, cv.h);
    for pc in pieces(plan, to_px) {
        let d = [pc.b[0] - pc.a[0], pc.b[1] - pc.a[1]];
        let len2 = d[0] * d[0] + d[1] * d[1];
        let len = len2.sqrt();
        let tan = if len > 1e-6 { [d[0] / len, d[1] / len] } else { [1.0, 0.0] };
        // The shadow, first, on what is already there.
        let (sa, sb) = ([pc.a[0] + shadow_off[0], pc.a[1] + shadow_off[1]], [pc.b[0] + shadow_off[0], pc.b[1] + shadow_off[1]]);
        capsule_rows(sa, sb, shadow_r, size, |y, x0, x1| {
            for x in x0..x1 {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                let t = nearest_t(p, sa, d, len2);
                let dist = ((p[0] - sa[0] - d[0] * t).powi(2) + (p[1] - sa[1] - d[1] * t).powi(2)).sqrt();
                let k = 1.0 - smoothstep(shadow_r * 0.3, shadow_r + 0.5, dist);
                if k > 0.0 {
                    cv.shade(x, y, 0.28 * k * shaded.max(0.4));
                }
            }
        });
        // Anisotropic highlight: how much this stitch's direction turns toward the light.
        let th = tan[0] * half[0] + tan[1] * half[1];
        let sheen = (1.0 - th * th).max(0.0).sqrt().powi(24);
        capsule_rows(pc.a, pc.b, r, size, |y, x0, x1| {
            for x in x0..x1 {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                let t = nearest_t(p, pc.a, d, len2);
                let off = [p[0] - pc.a[0] - d[0] * t, p[1] - pc.a[1] - d[1] * t];
                let dist = (off[0] * off[0] + off[1] * off[1]).sqrt();
                let cover = (r - dist + 0.5).clamp(0.0, 1.0);
                if cover <= 0.0 {
                    continue;
                }
                // The thread's surface normal: round across it, domed at its ends.
                let s = (dist / r).min(1.0);
                let (nx, ny) = if dist > 1e-6 { (off[0] / dist * s, off[1] / dist * s) } else { (0.0, 0.0) };
                let nz = (1.0 - s * s).max(0.0).sqrt();
                let diffuse = (nx * lx + ny * ly + nz * lz).max(0.0);
                let facing = (nx * half[0] + ny * half[1] + nz * half[2]).max(0.0);
                let spec = sheen * facing.powi(6) * 0.55 + facing.powi(40) * 0.12;
                // The ply's twist: faint stripes slanting across the thread.
                let along = t * len;
                let across = off[0] * -tan[1] + off[1] * tan[0];
                let stripe = 1.0 - twist + twist * (std::f32::consts::TAU * (along + across * 0.8) / twist_pitch.max(1e-3)).cos();
                // The thread dives into the fabric at its needle holes.
                let from_end = (along.min(len - along)).max(0.0);
                let hole = 0.78 + 0.22 * smoothstep(0.0, r * 1.6, from_end);
                let lit = (0.32 + 0.78 * diffuse) * stripe * hole;
                let flat = 0.86;
                let light = flat + (lit - flat) * shaded;
                let c = std::array::from_fn(|i| pc.color[i] * light + spec * shaded);
                cv.blend(x, y, c, cover);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Needle;

    fn stitch(x: f64, y: f64) -> Needle {
        Needle { cmd: Cmd::Stitch, x, y }
    }

    fn plan(needles: Vec<Needle>) -> Plan {
        Plan { threads: vec![[200, 20, 20], [20, 20, 200]], needles, ..Default::default() }
    }

    fn view(w: u32, h: u32, scale: f64) -> View {
        View { width: w, height: h, to_px: Affine::scale(scale), fabric: vec![Rect::new(0.0, 0.0, f64::from(w) / scale, f64::from(h) / scale)] }
    }

    fn px(img: &Image, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * img.width + x) * 4) as usize;
        [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2], img.rgba[i + 3]]
    }

    fn look(t: Texture) -> Look {
        Look::plain(t)
    }

    #[test]
    fn the_fabric_covers_its_area_and_nothing_else() {
        let p = plan(vec![]);
        let mut v = view(40, 20, 10.0);
        v.fabric = vec![Rect::new(0.0, 0.0, 2.0, 2.0)];
        let img = render(&p, &look(Texture::Plain), &v).unwrap();
        assert_eq!(px(&img, 5, 5)[3], 255);
        assert_eq!(px(&img, 30, 5)[3], 0, "outside the fabric");
    }

    #[test]
    fn stitches_draw_in_their_thread_colours_over_the_fabric() {
        let p = plan(vec![stitch(1.0, 5.0), stitch(9.0, 5.0), Needle { cmd: Cmd::ColorChange, x: 9.0, y: 5.0 }, stitch(1.0, 8.0), stitch(9.0, 8.0)]);
        let img = render(&p, &look(Texture::Plain), &view(100, 100, 10.0)).unwrap();
        let red = px(&img, 50, 50);
        let blue = px(&img, 50, 80);
        assert!(red[0] > red[2] + 60, "{red:?}");
        assert!(blue[2] > blue[0] + 60, "{blue:?}");
        let fabric = px(&img, 50, 20);
        assert!(fabric[0] > 150 && fabric[1] > 150, "{fabric:?}");
    }

    #[test]
    fn a_trimmed_jump_leaves_no_thread_and_an_untrimmed_one_does() {
        let jump = |x, y| Needle { cmd: Cmd::Jump, x, y };
        let trimmed = plan(vec![stitch(1.0, 5.0), stitch(2.0, 5.0), Needle { cmd: Cmd::Trim, x: 2.0, y: 5.0 }, jump(8.0, 5.0), stitch(9.0, 5.0)]);
        let loose = plan(vec![stitch(1.0, 5.0), stitch(2.0, 5.0), jump(8.0, 5.0), stitch(9.0, 5.0)]);
        let v = view(100, 100, 10.0);
        let mid = |p: &Plan| px(&render(p, &look(Texture::Felt), &v).unwrap(), 50, 50);
        assert!(mid(&trimmed)[1] > 100, "fabric between: {:?}", mid(&trimmed));
        assert!(mid(&loose)[1] < 80, "thread between: {:?}", mid(&loose));
    }

    #[test]
    fn stitches_across_the_light_shine_more_than_along_it() {
        // The light comes from the upper left, so a stitch from lower left to upper right lies
        // across it and catches a highlight; one from upper left to lower right doesn't.
        let mut needles = vec![];
        let mut row = |from: (f64, f64), to: (f64, f64), dx: f64| {
            for k in 0..12 {
                let o = k as f64 * dx;
                needles.push(stitch(from.0 + o, from.1));
                needles.push(stitch(to.0 + o, to.1));
            }
        };
        row((2.0, 12.0), (8.0, 6.0), 0.4);
        let across = plan(needles);
        let along = plan(across.needles.iter().map(|n| stitch(n.x, 18.0 - n.y)).collect());
        let v = view(200, 200, 10.0);
        let bright = |p: &Plan| {
            let img = render(p, &Look { color: [0, 0, 0], ..Look::plain(Texture::Felt) }, &v).unwrap();
            (60..120).flat_map(|y| (60..120).map(move |x| (x, y))).map(|(x, y)| u32::from(px(&img, x, y)[1])).sum::<u32>()
        };
        let thread = |p: &Plan| {
            let mut q = p.clone();
            q.threads = vec![[90, 90, 90]];
            q
        };
        let (sa, sl) = (bright(&thread(&across)), bright(&thread(&along)));
        assert!(sa > sl + sl / 10, "across {sa} along {sl}");
    }

    #[test]
    fn no_fabric_leaves_only_the_threads() {
        let p = plan(vec![stitch(10.0, 10.0), stitch(30.0, 10.0)]);
        let v = view(200, 100, 5.0);
        let img = render(&p, &Look { fabric: false, ..look(Texture::Knit) }, &v).unwrap();
        assert_eq!(px(&img, 150, 80)[3], 0, "no fabric away from the thread");
        assert!(px(&img, 100, 50)[3] > 200, "the thread is still drawn");
    }

    #[test]
    fn every_texture_draws_within_its_colour_range() {
        for t in Texture::ALL {
            let img = render(&plan(vec![]), &look(t), &view(64, 64, 20.0)).unwrap();
            let lum: Vec<u32> = img.rgba.chunks(4).map(|c| u32::from(c[0]) + u32::from(c[1]) + u32::from(c[2])).collect();
            let (lo, hi) = (lum.iter().min().unwrap(), lum.iter().max().unwrap());
            assert!(hi > lo, "{t:?} has a pattern");
            assert_eq!(Texture::from_id(t.id()), Some(t));
        }
        // Zoomed far out the pattern fades to an even colour rather than moiré.
        let img = render(&plan(vec![]), &look(Texture::Plain), &view(64, 64, 0.5)).unwrap();
        assert!(img.rgba.chunks(4).all(|c| c == &img.rgba[0..4]));
    }

    #[test]
    fn bad_views_are_refused_not_drawn() {
        let p = plan(vec![stitch(0.0, 0.0), stitch(1.0, 1.0)]);
        assert!(render(&p, &look(Texture::Plain), &view(0, 10, 1.0)).is_err());
        assert!(render(&p, &look(Texture::Plain), &view(100_000, 100_000, 1.0)).is_err());
        let mut v = view(10, 10, 1.0);
        v.to_px = Affine::scale(0.0);
        assert!(render(&p, &look(Texture::Plain), &v).is_err());
        v.to_px = Affine::scale(f64::NAN);
        assert!(render(&p, &look(Texture::Plain), &v).is_err());
    }

    #[test]
    fn colours_parse_from_hex() {
        assert_eq!(parse_hex("#1e2a44"), Some([0x1e, 0x2a, 0x44]));
        assert_eq!(parse_hex("FFFFFF"), Some([255; 3]));
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex("#ffé00"), None);
    }
}
