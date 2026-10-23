//! Brother PES (with its PEC section, the part machines sew from).
//!
//! Layout, from the public descriptions of the format
//! (<https://edutechwiki.unige.ch/en/Embroidery_format_PES>,
//! <https://edutechwiki.unige.ch/en/Embroidery_format_PEC>), little-endian unless noted:
//!
//! - **PES header:** `#PES` and a 4-digit version, then the `u32` offset of the PEC section.
//!   Version 6 (`0060`) goes on with `u16` 1, `02`, five strings (`u8` length, then the text:
//!   design name, category, author, keywords, comments), eighteen `u16` settings (hoop width and
//!   height in mm among them), an image path (`u8` length), an affine transform (six `f32`), three
//!   `u16` pattern counts, then the **thread list**: `u16` count, and for each thread its catalogue
//!   number (`u8` length + text), red, green, blue, a zero byte, `u32` type (0x0A, custom), and
//!   description, brand and chart (each `u8` length + text). Then `u16` the number of PES objects.
//!   VectorCraft writes none (the design lives in the PEC section, which machines read), then nine
//!   zero bytes, as other open tools write.
//! - **PEC section:** `LA:` and a 16-character label, 0x0D, filler, the thumbnail's size (6 bytes ×
//!   38 rows), at 48 the number of colour blocks minus one and from 49 a palette number per block,
//!   filled with spaces to 512. Then `00 00`, the graphics' offset from byte 512 (3 bytes), `31 FF
//!   F0`, the design's width and height (`u16`, 0.1 mm), `E0 01 B0 01`, and from byte 528 the
//!   stitches.
//! - **Stitches:** relative moves in 0.1 mm, y down, x then y. A byte under 0x80 is a 7-bit signed
//!   move; with its top bit set, two bytes hold a 12-bit signed move (big-endian) and flags: 0x10
//!   jump, 0x20 trim. `FE B0 n` changes colour (n alternates 2, 1); `FF` ends the stitches. The
//!   first move is a jump from the design's top-left corner.
//! - **Thumbnails:** after the stitches, 48 × 38 pixels at 1 bit each, 6 bytes a row, the least
//!   significant bit leftmost: one of the whole design, then one per colour block. Version 6 ends
//!   with a blank colour addendum (0xFF, 127 spaces) and `00 00`.
//!
//! The PEC palette numbers index Brother's thread palette. VectorCraft ships no maker's palette
//! (see `docs/embroidery.md` › Thread charts): it writes a number per distinct colour, and keeps the
//! real colours in the version 6 thread list, which it reads back. Files without that list
//! (versions 1–5) open with a colour per palette number.

use crate::machine::{Op, Reader, ascii, bounds, ops, steps, u16_at, u32_at};
use crate::{MAX_NEEDLES, Plan, palette_color};

const PEC_LABEL: usize = 16;
const THUMB_W: usize = 48;
const THUMB_H: usize = 38;
const THUMB_BYTES: usize = THUMB_W / 8 * THUMB_H;
/// Where the stitches start, from the PEC section's start.
const PEC_STITCHES: usize = 528;
/// The longest stitch written in one move (longer ones are split), and the longest jump.
const MAX_STITCH: i64 = 127;
const MAX_JUMP: i64 = 2047;
const JUMP: u8 = 0x10;
const TRIM: u8 = 0x20;
/// The most colour blocks the PEC header counts (one byte).
const MAX_BLOCKS: usize = 256;

/// Does `bytes` start like a PES file?
pub fn is_pes(bytes: &[u8]) -> bool {
    bytes.starts_with(b"#PES")
}

/// Hoop sizes the header can name (mm), smallest first.
const HOOPS: [(u16, u16); 3] = [(100, 100), (130, 180), (200, 200)];

/// A PEC move: a short byte when it fits (and `flags` is 0), else two bytes with the flags.
fn put_move(out: &mut Vec<u8>, v: i64, long: bool, flags: u8) {
    if !long {
        out.push((v as i8 as u8) & 0x7F);
    } else {
        let v = (v.clamp(-MAX_JUMP, MAX_JUMP) as i16 as u16) & 0x0FFF;
        out.push(0x80 | flags | (v >> 8) as u8);
        out.push((v & 0xFF) as u8);
    }
}

fn short(v: i64) -> bool {
    (-64..=63).contains(&v)
}

/// One move, x then y; both long when either is, or when flagged.
fn put_pair(out: &mut Vec<u8>, dx: i64, dy: i64, flags: u8) {
    let long = flags != 0 || !short(dx) || !short(dy);
    put_move(out, dx, long, flags);
    put_move(out, dy, long, flags);
}

/// A 48 × 38 one-bit picture of `runs` (polylines in 0.1 mm from the design's corner, of size
/// `size`), framed.
fn thumbnail(runs: &[Vec<(i64, i64)>], size: (i64, i64)) -> [u8; THUMB_BYTES] {
    let mut px = [0u8; THUMB_BYTES];
    let mut set = |x: i64, y: i64| {
        if (0..THUMB_W as i64).contains(&x)
            && (0..THUMB_H as i64).contains(&y)
            && let Some(b) = px.get_mut(y as usize * (THUMB_W / 8) + x as usize / 8)
        {
            *b |= 1 << (x % 8);
        }
    };
    // The frame.
    for x in 2..46 {
        set(x, 1);
        set(x, 36);
    }
    for y in 1..37 {
        set(2, y);
        set(45, y);
    }
    // The stitches, scaled into the frame keeping their proportions.
    let (iw, ih) = (38.0, 30.0);
    let scale = (iw / size.0.max(1) as f64).min(ih / size.1.max(1) as f64);
    let (ox, oy) = (4.0 + (iw - size.0 as f64 * scale) / 2.0, 4.0 + (ih - size.1 as f64 * scale) / 2.0);
    let at = |p: (i64, i64)| (ox + p.0 as f64 * scale, oy + p.1 as f64 * scale);
    for run in runs {
        for w in run.windows(2) {
            let (Some(&a), Some(&b)) = (w.first(), w.get(1)) else { continue };
            let (a, b) = (at(a), at(b));
            let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil() as i64).clamp(1, 100);
            for i in 0..=n {
                let t = i as f64 / n as f64;
                set((a.0 + (b.0 - a.0) * t).round() as i64, (a.1 + (b.1 - a.1) * t).round() as i64);
            }
        }
    }
    px
}

/// The palette number written for each colour block: one per distinct colour, 1 to 64.
fn numbers(threads: &[[u8; 3]]) -> Vec<u8> {
    let mut seen: Vec<[u8; 3]> = vec![];
    threads
        .iter()
        .map(|t| {
            let i = seen.iter().position(|s| s == t).unwrap_or_else(|| {
                seen.push(*t);
                seen.len() - 1
            });
            (i % 64) as u8 + 1
        })
        .collect()
}

/// The PEC section of `plan`, labelled `label`; warnings when something couldn't be kept.
fn write_pec(plan: &Plan, label: &str, threads: &[[u8; 3]], warnings: &mut Vec<String>) -> Vec<u8> {
    let (lo, hi) = bounds(plan).unwrap_or(((0.0, 0.0), (0.0, 0.0)));
    let size = (((hi.0 - lo.0) * 10.0).round() as i64, ((hi.1 - lo.1) * 10.0).round() as i64);
    let mut st = vec![];
    let mut pos = (0i64, 0i64);
    let mut started = false;
    let mut changes = 0usize;
    // Polylines for the thumbnails, per colour block.
    let mut runs: Vec<Vec<Vec<(i64, i64)>>> = vec![vec![]];
    for op in ops(plan, lo) {
        match op {
            Op::Stitch(x, y) => {
                if !started {
                    for (dx, dy) in steps(pos, (x, y), MAX_JUMP) {
                        put_pair(&mut st, dx, dy, JUMP);
                    }
                    pos = (x, y);
                    started = true;
                }
                for (dx, dy) in steps(pos, (x, y), MAX_STITCH) {
                    put_pair(&mut st, dx, dy, 0);
                }
                if let Some(block) = runs.last_mut() {
                    match block.last_mut() {
                        Some(run) if run.last() == Some(&pos) => run.push((x, y)),
                        _ => block.push(vec![pos, (x, y)]),
                    }
                }
                pos = (x, y);
            }
            Op::Jump(x, y) => {
                for (dx, dy) in steps(pos, (x, y), MAX_JUMP) {
                    if (dx, dy) != (0, 0) || !started {
                        put_pair(&mut st, dx, dy, JUMP);
                    }
                }
                pos = (x, y);
                started = true;
            }
            Op::Trim => put_pair(&mut st, 0, 0, TRIM),
            Op::Color => {
                st.extend_from_slice(&[0xFE, 0xB0, if changes.is_multiple_of(2) { 2 } else { 1 }]);
                changes += 1;
                runs.push(vec![]);
            }
        }
    }
    st.push(0xFF);
    let blocks = threads.len().clamp(1, MAX_BLOCKS);
    if threads.len() > MAX_BLOCKS {
        warnings.push(format!("PEC lists at most {MAX_BLOCKS} colour blocks: the design has {}", threads.len()));
    }
    let mut out = Vec::with_capacity(PEC_STITCHES + st.len() + THUMB_BYTES * (blocks + 1));
    out.extend_from_slice(format!("LA:{:<PEC_LABEL$}\r", ascii(label, PEC_LABEL)).as_bytes());
    out.resize(31, b' ');
    out.extend_from_slice(&[0xFF, 0xFF, 0x00, (THUMB_W / 8) as u8, THUMB_H as u8]);
    out.extend_from_slice(&[0x20, 0x20, 0x20, 0x20, 0x64, 0x20, 0x00, 0x20, 0x00, 0x20, 0x20, 0x20]);
    out.push((blocks - 1) as u8);
    out.extend(numbers(threads).into_iter().take(blocks));
    out.resize(512, b' ');
    let graphics = (16 + st.len()).min(0xFF_FFFF) as u32;
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&graphics.to_le_bytes()[..3]);
    out.extend_from_slice(&[0x31, 0xFF, 0xF0]);
    out.extend_from_slice(&(size.0.clamp(0, i16::MAX as i64) as u16).to_le_bytes());
    out.extend_from_slice(&(size.1.clamp(0, i16::MAX as i64) as u16).to_le_bytes());
    out.extend_from_slice(&[0xE0, 0x01, 0xB0, 0x01]);
    out.extend_from_slice(&st);
    let all: Vec<Vec<(i64, i64)>> = runs.iter().flatten().cloned().collect();
    out.extend_from_slice(&thumbnail(&all, size));
    for i in 0..blocks {
        out.extend_from_slice(&thumbnail(runs.get(i).map(Vec::as_slice).unwrap_or_default(), size));
    }
    out
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    let s = ascii(s, 255);
    out.push(s.len() as u8);
    out.extend_from_slice(s.as_bytes());
}

/// `plan` as a version 6 PES file labelled `label`, with its thread colours; and warnings for
/// what couldn't be kept.
pub fn write_pes(plan: &Plan, label: &str) -> (Vec<u8>, Vec<String>) {
    let mut warnings = vec![];
    let blocks = 1 + plan.needles.iter().filter(|n| n.cmd == crate::Cmd::ColorChange).count();
    let threads: Vec<[u8; 3]> = (0..blocks).map(|i| plan.threads.get(i).copied().unwrap_or_else(|| palette_color(i))).collect();
    let (lo, hi) = bounds(plan).unwrap_or(((0.0, 0.0), (0.0, 0.0)));
    let (w, h) = (hi.0 - lo.0, hi.1 - lo.1);
    let hoop = HOOPS.iter().find(|(hw, hh)| w <= *hw as f64 && h <= *hh as f64).or(HOOPS.last()).copied().unwrap_or((200, 200));
    let mut out = b"#PES0060".to_vec();
    out.extend_from_slice(&[0; 4]); // the PEC offset, filled in below
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(b"02");
    put_str(&mut out, label);
    for _ in 0..4 {
        put_str(&mut out, "");
    }
    for v in [0u16, 0, hoop.0, hoop.1, 0, 200, 200, 100, 100, 100, 7, 19, 1, 1, 0, 100, 1, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.push(0); // no background image
    for v in [1.0f32, 0.0, 0.0, 1.0, 0.0, 0.0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&[0; 6]); // no fill patterns, motifs or feather patterns
    let listed = threads.len().min(u16::MAX as usize);
    out.extend_from_slice(&(listed as u16).to_le_bytes());
    for t in threads.iter().take(listed) {
        put_str(&mut out, "");
        out.extend_from_slice(&[t[0], t[1], t[2], 0]);
        out.extend_from_slice(&0x0Au32.to_le_bytes());
        put_str(&mut out, &format!("#{:02X}{:02X}{:02X}", t[0], t[1], t[2]));
        put_str(&mut out, "");
        put_str(&mut out, "");
    }
    out.extend_from_slice(&[0; 2]); // no PES objects
    out.extend_from_slice(&[0; 9]);
    let pec = out.len() as u32;
    if let Some(b) = out.get_mut(8..12) {
        b.copy_from_slice(&pec.to_le_bytes());
    }
    out.extend_from_slice(&write_pec(plan, label, &threads, &mut warnings));
    // The blank colour addendum of version 6.
    out.push(0xFF);
    out.extend_from_slice(&[b' '; 127]);
    out.extend_from_slice(&[0, 0]);
    (out, warnings)
}

/// Reads PES fields in order; `None` past the end.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn skip(&mut self, n: usize) -> Option<()> {
        self.at = self.at.checked_add(n).filter(|&a| a <= self.bytes.len())?;
        Some(())
    }
    fn u8(&mut self) -> Option<u8> {
        let v = *self.bytes.get(self.at)?;
        self.at += 1;
        Some(v)
    }
    fn u16(&mut self) -> Option<u16> {
        let v = u16_at(self.bytes, self.at)?;
        self.at += 2;
        Some(v)
    }
    fn string(&mut self) -> Option<()> {
        let n = self.u8()? as usize;
        self.skip(n)
    }
}

/// The version 6 thread list's colours, if the header reads.
fn v6_threads(bytes: &[u8]) -> Option<Vec<[u8; 3]>> {
    let mut c = Cursor { bytes, at: 12 };
    c.skip(4)?; // u16 1, "02"
    for _ in 0..5 {
        c.string()?;
    }
    c.skip(18 * 2)?;
    c.string()?; // the background image's path
    c.skip(24)?;
    let patterns = [c.u16()?, c.u16()?, c.u16()?];
    if patterns.iter().any(|&n| n != 0) {
        // Programmable fills, motifs and feathers come before the threads, in a layout the
        // descriptions don't give.
        return None;
    }
    let n = c.u16()? as usize;
    let mut out = Vec::with_capacity(n.min(1000));
    for _ in 0..n {
        c.string()?; // catalogue number
        let rgb = [c.u8()?, c.u8()?, c.u8()?];
        c.skip(1 + 4)?;
        for _ in 0..3 {
            c.string()?;
        }
        out.push(rgb);
    }
    Some(out)
}

/// A 12-bit signed value from a long move's two bytes.
fn long(hi: u8, lo: u8) -> i64 {
    let v = (((hi & 0x0F) as i64) << 8) | lo as i64;
    if v >= 0x800 { v - 0x1000 } else { v }
}

/// Read a PES file into a plan: the stitches from its PEC section, the colours from its version 6
/// thread list (else a colour per palette number).
pub fn read_pes(bytes: &[u8]) -> Result<Plan, String> {
    if !is_pes(bytes) {
        return Err("not a PES embroidery file (no `#PES` header)".into());
    }
    let pec = u32_at(bytes, 8).ok_or("the PES header is cut short")? as usize;
    if pec >= bytes.len() {
        return Err("the PES file's PEC section is missing (its offset is past the end)".into());
    }
    let mut warnings = vec![];
    // Colour blocks and their palette numbers.
    let count = bytes.get(pec + 48).map_or(1, |&n| n as usize + 1);
    let numbers: Vec<u8> = (0..count).filter_map(|i| bytes.get(pec + 49 + i).copied()).collect();
    let listed = if bytes.get(4..8) == Some(b"0060") { v6_threads(bytes) } else { None };
    let threads: Vec<[u8; 3]> = match listed {
        // A thread per colour block, in order.
        Some(list) if list.len() == numbers.len() => list,
        // A thread per distinct palette number, in the order they're first used.
        Some(list) if !list.is_empty() => {
            let mut firsts: Vec<u8> = vec![];
            for n in &numbers {
                if !firsts.contains(n) {
                    firsts.push(*n);
                }
            }
            if firsts.len() == list.len() {
                numbers.iter().map(|n| firsts.iter().position(|f| f == n).and_then(|i| list.get(i).copied()).unwrap_or([0, 0, 0])).collect()
            } else {
                warnings.push("the PES thread list doesn't match the colour blocks: colours by palette number".to_string());
                numbers.iter().map(|&n| palette_color(n as usize)).collect()
            }
        }
        _ => numbers.iter().map(|&n| palette_color(n as usize)).collect(),
    };
    let mut r = Reader::new(false);
    let mut i = pec + PEC_STITCHES;
    let mut ended = false;
    let mut first = true;
    loop {
        if r.full() || r.plan.needles.len() >= MAX_NEEDLES {
            break;
        }
        let Some(&b) = bytes.get(i) else { break };
        if b == 0xFF {
            ended = true;
            break;
        }
        if b == 0xFE {
            i += 3;
            r.color();
            continue;
        }
        let mut flags = 0u8;
        let mut read = |i: &mut usize| -> Option<i64> {
            let b = *bytes.get(*i)?;
            if b & 0x80 != 0 {
                let lo = *bytes.get(*i + 1)?;
                flags |= b & 0x70;
                *i += 2;
                Some(long(b, lo))
            } else {
                *i += 1;
                Some(if b >= 0x40 { b as i64 - 0x80 } else { b as i64 })
            }
        };
        let (Some(dx), Some(dy)) = (read(&mut i), read(&mut i)) else { break };
        if flags & TRIM != 0 {
            r.trim();
            if (dx, dy) != (0, 0) {
                r.jump(dx, dy);
            }
        } else if flags & JUMP != 0 || first {
            // The first move only takes the needle to the design's first point.
            r.jump(dx, dy);
        } else {
            r.stitch(dx, dy);
        }
        first = false;
    }
    let mut plan = r.finish(&threads, ended);
    plan.warnings.splice(0..0, warnings);
    Ok(plan)
}
