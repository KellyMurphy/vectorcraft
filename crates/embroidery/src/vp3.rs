//! Pfaff / Husqvarna Viking VP3.
//!
//! Layout, from the public descriptions of the format
//! (<https://edutechwiki.unige.ch/en/Embroidery_format_VP3>, and
//! <http://www.jasonweiler.com/VP3FileFormatInfo.html> for the stitch encoding), big-endian:
//!
//! - `%vsm%` and a zero byte; a `u16` byte length and a UTF-16BE "produced by" string.
//! - The file section: the tag `00 02 00`, a `u32` count of the bytes after it to the end of the
//!   file, a notes string (`u16` length, UTF-16BE), the design's extents (`s32` right, −top, left,
//!   −bottom, in µm), a `u32` neither description explains (written 0), then `00`, the number of
//!   colour blocks, `0C 00`, the number of designs.
//! - The design: `00 03 00`, a `u32` count to the end of the design, its centre (`s32` x, −y, µm),
//!   three zero bytes, its extents about the centre (`s32` −half width, +half width, −half height,
//!   +half height, µm), its width and height (`s32`, µm), a notes string, `64 64`, `s32` 4096, 0,
//!   0, 4096, `xxPP`, `01 00`, the production string again and a `u16` number of colour blocks.
//! - Each colour block: `00 05 00`, a `u32` count to the next block, the block's start relative to
//!   the centre (`s32` x, y, µm), its thread (a count of colour entries — 1 — a mixing flag, per
//!   entry the colour as `RR GG BB`, a byte of parts and a `u16` length; then material, weight and
//!   three `u16`-length UTF-8 strings: catalogue number, description, brand), the move from its
//!   first stitch to its last (`s32` x, y, µm), then the stitch data: `00 01 00`, a `u32` count to
//!   the end of the stitches, `0A F6 00`, the stitches, and after them one zero byte.
//!
//! Every `u32` count is of the bytes after it. Positions in the header are y up (negated); the
//! stitches are moves from the last point, 0.1 mm, with y down as on screen: two signed bytes; `80 01` and `80 02` around two `s16`s make a long move, which machines
//! take as a jump; `80 03` ends the design and `80 00` is nothing. VP3 has no trim code: machines
//! trim at a colour change, and at jumps as long as their trim setting says (the descriptions note
//! that long moves of 25.5 mm or more are cut). Some writers put `80 03` where they cut the thread
//! in the middle of a design, so a reader takes it as a trim there rather than stopping.

use crate::machine::{Op, bounds, ops, steps};
use crate::{Cmd, MAX_NEEDLES, Needle, Plan, palette_color};

const MAGIC: &[u8] = b"%vsm%\0";
const PRODUCED_BY: &str = "Produced by VectorCraft";
/// The longest short move, 0.1 mm (0x80 starts a command).
const SHORT: i64 = 127;
/// The longest long move, 0.1 mm.
const LONG: i64 = 32767;
/// A long move this long (0.1 mm) on either axis is read as cut: the thread is trimmed.
const TRIM_JUMP: i64 = 255;
/// More colour blocks than any design has: past it a file is taken as damaged.
const MAX_BLOCKS: usize = 10_000;
/// µm per file unit of 0.1 mm.
const UM: i64 = 100;

/// Does `bytes` start as a VP3 file does?
pub fn is_vp3(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_be_bytes());
}

fn put_i32(out: &mut Vec<u8>, v: i64) {
    out.extend_from_slice(&(v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32).to_be_bytes());
}

/// A UTF-16BE string after its `u16` byte length.
fn put_utf16(out: &mut Vec<u8>, s: &str) {
    let units: Vec<u16> = s.encode_utf16().take(1000).collect();
    put_u16(out, (units.len() * 2) as u16);
    for u in units {
        out.extend_from_slice(&u.to_be_bytes());
    }
}

/// A UTF-8 string after its `u16` byte length.
fn put_utf8(out: &mut Vec<u8>, s: &str) {
    let b = s.as_bytes();
    let n = b.len().min(1000);
    put_u16(out, n as u16);
    out.extend_from_slice(b.get(..n).unwrap_or_default());
}

/// `body` after the tag `tag` and the `u32` count of its bytes.
fn section(out: &mut Vec<u8>, tag: u8, body: &[u8]) {
    out.extend_from_slice(&[0, tag, 0]);
    out.extend_from_slice(&(body.len().min(u32::MAX as usize) as u32).to_be_bytes());
    out.extend_from_slice(body);
}

fn byte(v: i64) -> u8 {
    (v.clamp(-SHORT, SHORT) as i8) as u8
}

/// One colour block being written: where it starts (0.1 mm from the centre, y down) and its stitch
/// bytes.
struct Block {
    start: (i64, i64),
    end: (i64, i64),
    stitches: Vec<u8>,
}

/// `plan` as a VP3 file, centred on the design's middle, and warnings for what VP3 can't hold.
pub fn write_vp3(plan: &Plan) -> (Vec<u8>, Vec<String>) {
    let mut warnings = vec![];
    let centre = bounds(plan).map(|(lo, hi)| ((lo.0 + hi.0) / 2.0, (lo.1 + hi.1) / 2.0)).unwrap_or((0.0, 0.0));
    let mut blocks = vec![Block { start: (0, 0), end: (0, 0), stitches: vec![] }];
    let mut pos = (0i64, 0i64);
    // Extents, 0.1 mm, y down: (min x, min y, max x, max y).
    let mut ext = (0i64, 0i64, 0i64, 0i64);
    let mut trims = 0usize;
    for op in ops(plan, centre) {
        let Some(b) = blocks.last_mut() else { break };
        match op {
            Op::Stitch(x, y) => {
                for (dx, dy) in steps(pos, (x, y), SHORT) {
                    b.stitches.extend_from_slice(&[byte(dx), byte(dy)]);
                }
                pos = (x, y);
            }
            Op::Jump(x, y) => {
                for (dx, dy) in steps(pos, (x, y), LONG) {
                    b.stitches.extend_from_slice(&[0x80, 0x01]);
                    b.stitches.extend_from_slice(&(dx.clamp(-LONG, LONG) as i16).to_be_bytes());
                    b.stitches.extend_from_slice(&(dy.clamp(-LONG, LONG) as i16).to_be_bytes());
                    b.stitches.extend_from_slice(&[0x80, 0x02]);
                }
                pos = (x, y);
            }
            Op::Trim => trims += 1,
            Op::Color => {
                b.end = pos;
                blocks.push(Block { start: pos, end: pos, stitches: vec![] });
            }
        }
        if let Op::Stitch(x, y) | Op::Jump(x, y) = op {
            ext = (ext.0.min(x), ext.1.min(y), ext.2.max(x), ext.3.max(y));
        }
    }
    if let Some(b) = blocks.last_mut() {
        b.end = pos;
        b.stitches.extend_from_slice(&[0x80, 0x03]);
    }
    if trims > 0 {
        warnings.push(format!(
            "VP3 has no trim code: the design's {trims} trims are left to the machine, which trims at colour changes and at long jumps as its settings say"
        ));
    }
    let n = blocks.len();
    // The colour blocks.
    let mut design_blocks = vec![];
    for (i, b) in blocks.iter().enumerate() {
        let mut body = vec![];
        put_i32(&mut body, b.start.0 * UM);
        put_i32(&mut body, -b.start.1 * UM);
        let [r, g, bl] = plan.threads.get(i).copied().unwrap_or_else(|| palette_color(i));
        // One colour entry: hard transition; the colour, 10 parts, no length; rayon, 40 weight.
        body.extend_from_slice(&[1, 0, r, g, bl, 0x0A, 0, 0, 0x05, 0x28]);
        for s in ["", "", ""] {
            put_utf8(&mut body, s);
        }
        put_i32(&mut body, (b.end.0 - b.start.0) * UM);
        put_i32(&mut body, -(b.end.1 - b.start.1) * UM);
        let mut stitches = vec![0x0A, 0xF6, 0x00];
        stitches.extend_from_slice(&b.stitches);
        section(&mut body, 0x01, &stitches);
        body.push(0);
        section(&mut design_blocks, 0x05, &body);
    }
    // The design.
    let (half_w, half_h) = (((ext.2 - ext.0) * UM + 1) / 2, ((ext.3 - ext.1) * UM + 1) / 2);
    let mut design = vec![];
    put_i32(&mut design, 0);
    put_i32(&mut design, 0);
    design.extend_from_slice(&[0, 0, 0]);
    for v in [-half_w, half_w, -half_h, half_h, 2 * half_w, 2 * half_h] {
        put_i32(&mut design, v);
    }
    put_utf16(&mut design, "");
    design.extend_from_slice(&[0x64, 0x64]);
    for v in [4096, 0, 0, 4096] {
        put_i32(&mut design, v);
    }
    design.extend_from_slice(b"xxPP");
    design.extend_from_slice(&[0x01, 0x00]);
    put_utf16(&mut design, PRODUCED_BY);
    put_u16(&mut design, n.min(u16::MAX as usize) as u16);
    design.extend_from_slice(&design_blocks);
    // The file.
    let mut file = vec![];
    put_utf16(&mut file, "");
    for v in [ext.2 * UM, -ext.1 * UM, ext.0 * UM, -ext.3 * UM] {
        put_i32(&mut file, v);
    }
    put_i32(&mut file, 0);
    file.extend_from_slice(&[0, n.min(255) as u8, 0x0C, 0, 1]);
    section(&mut file, 0x03, &design);
    let mut out = MAGIC.to_vec();
    put_utf16(&mut out, PRODUCED_BY);
    section(&mut out, 0x02, &file);
    (out, warnings)
}

/// Reads a file's fields in order, failing at its end.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).ok_or_else(cut)?;
        let s = self.bytes.get(self.at..end).ok_or_else(cut)?;
        self.at = end;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(*self.take(1)?.first().ok_or_else(cut)?)
    }

    fn u16(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([*b.first().ok_or_else(cut)?, *b.get(1).ok_or_else(cut)?]))
    }

    fn i32(&mut self) -> Result<i64, String> {
        let b: [u8; 4] = self.take(4)?.try_into().map_err(|_| cut())?;
        Ok(i64::from(i32::from_be_bytes(b)))
    }

    fn u32(&mut self) -> Result<usize, String> {
        let b: [u8; 4] = self.take(4)?.try_into().map_err(|_| cut())?;
        Ok(u32::from_be_bytes(b) as usize)
    }

    /// A string after its `u16` byte length, skipped.
    fn skip_string(&mut self) -> Result<(), String> {
        let n = self.u16()? as usize;
        self.take(n).map(|_| ())
    }

    /// A tag `00 tag 00` and its `u32` count: where the section ends (clamped to the file).
    fn section(&mut self, tag: u8) -> Result<usize, String> {
        let t = self.take(3)?;
        if t != [0, tag, 0] {
            return Err(format!("not a VP3 embroidery file (no section {tag:02X} where one belongs)"));
        }
        let n = self.u32()?;
        Ok(self.at.saturating_add(n).min(self.bytes.len()))
    }
}

fn cut() -> String {
    "the VP3 file is cut short".into()
}

/// Builds the plan from absolute positions in 0.1 mm, y down.
struct Builder {
    plan: Plan,
    full: bool,
}

impl Builder {
    fn push(&mut self, x: i64, y: i64, cmd: Cmd) {
        if self.plan.needles.len() >= MAX_NEEDLES {
            if !self.full {
                self.plan.warnings.push(format!("the design has more than {MAX_NEEDLES} needle moves: the rest is left out"));
            }
            self.full = true;
            return;
        }
        self.plan.needles.push(Needle { x: x as f64 / 10.0, y: y as f64 / 10.0, cmd });
    }
}

/// Read a VP3 file into a plan, with each colour block's thread colour. Errors when its header
/// doesn't read.
pub fn read_vp3(bytes: &[u8]) -> Result<Plan, String> {
    if !is_vp3(bytes) {
        return Err("not a VP3 embroidery file (it doesn't start with %vsm%)".into());
    }
    let mut c = Cursor { bytes, at: MAGIC.len() };
    c.skip_string()?;
    c.section(0x02)?;
    c.skip_string()?;
    c.take(16 + 4 + 5)?;
    c.section(0x03)?;
    // The design's centre (where the hoop has it: the plan keeps positions about the centre, as
    // the other readers keep theirs about the machine's start), three bytes and the extents.
    c.take(8 + 3 + 16 + 8)?;
    c.skip_string()?;
    c.take(2 + 16 + 4 + 2)?;
    c.skip_string()?;
    let count = c.u16()? as usize;
    let mut b = Builder { plan: Plan::default(), full: false };
    let mut threads = vec![];
    // The position, 0.1 mm from the design's centre, y down.
    let mut pos = (0i64, 0i64);
    let mut unknown = 0usize;
    for i in 0..count.min(MAX_BLOCKS) {
        if b.full {
            break;
        }
        let Ok(next) = c.section(0x05) else {
            b.plan.warnings.push(format!("the file ends after {i} of its {count} colour blocks"));
            break;
        };
        let start = (c.i32()? / UM, -c.i32()? / UM);
        // The thread: its first colour entry's colour.
        let entries = c.u8()? as usize;
        c.u8()?;
        let table = c.take(6 * entries)?;
        let rgb = match table {
            [r, g, bl, ..] => [*r, *g, *bl],
            _ => palette_color(i),
        };
        c.take(2)?;
        for _ in 0..3 {
            c.skip_string()?;
        }
        c.take(8)?;
        let stitch_end = c.section(0x01)?;
        c.take(3)?;
        if i > 0 {
            b.push(pos.0, pos.1, Cmd::ColorChange);
        }
        threads.push(rgb);
        if start != pos || b.plan.needles.is_empty() {
            pos = start;
            b.push(pos.0, pos.1, Cmd::Jump);
        }
        let s = bytes.get(c.at..stitch_end).unwrap_or_default();
        let mut k = 0usize;
        while let (Some(&x), Some(&y)) = (s.get(k), s.get(k + 1)) {
            if b.full {
                break;
            }
            if x != 0x80 {
                pos = (pos.0 + i64::from(x as i8), pos.1 + i64::from(y as i8));
                b.push(pos.0, pos.1, Cmd::Stitch);
                k += 2;
                continue;
            }
            k += 2;
            match y {
                0x01 => {
                    let word = |at: usize| s.get(at..at + 2).and_then(|w| w.try_into().ok()).map(|w: [u8; 2]| i64::from(i16::from_be_bytes(w)));
                    let (Some(dx), Some(dy)) = (word(k), word(k + 2)) else { break };
                    k += 4;
                    if s.get(k..k + 2) == Some(&[0x80, 0x02]) {
                        k += 2;
                    }
                    if dx.abs() >= TRIM_JUMP || dy.abs() >= TRIM_JUMP {
                        b.push(pos.0, pos.1, Cmd::Trim);
                    }
                    pos = (pos.0 + dx, pos.1 + dy);
                    b.push(pos.0, pos.1, Cmd::Jump);
                }
                // The end, or (from some writers) a cut in the middle of the design.
                0x03 => {
                    if i + 1 < count || k < s.len() {
                        b.push(pos.0, pos.1, Cmd::Trim);
                    }
                }
                0x00 | 0x02 => {}
                _ => unknown += 1,
            }
        }
        c.at = next.max(c.at);
    }
    if unknown > 0 {
        b.plan.warnings.push(format!("{unknown} commands this reader doesn't know were skipped"));
    }
    // A design cut short is reported where its blocks run out (some writers end without `80 03`).
    if let Some(last) = b.plan.needles.last().copied() {
        b.plan.needles.push(Needle { cmd: Cmd::End, ..last });
    }
    if threads.is_empty() {
        threads.push(palette_color(0));
    }
    b.plan.threads = threads;
    Ok(b.plan)
}
