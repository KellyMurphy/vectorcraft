//! Janome JEF.
//!
//! Layout, from the public description of the format
//! (<https://edutechwiki.unige.ch/en/Embroidery_format_JEF>), little-endian: a header of
//! 116 + 8 × N bytes (N threads): `u32` offset of the stitches, `u32` 0x14, the date as 14 ASCII
//! digits (`YYYYMMDDHHMMSS`) and two zero bytes, `u32` N, `u32` the stitch data's length in 2-byte
//! units, `u32` hoop code (0: 110 × 110 mm, 1: 50 × 50, 2: 140 × 200, 4: 200 × 200), the design's
//! extents from the hoop's centre (left, top, right, bottom, 0.1 mm), then for the 110 × 110,
//! 50 × 50 and 140 × 200 hoops and a custom one, the room left on each side (all −1 when the design
//! doesn't fit), then N `u32` thread numbers and N `u32` 0x0D. The stitches are two signed bytes,
//! x then y, up to ±127 units of 0.1 mm, y up; 0x80 starts a command `80 cc dx dy`: `cc` 0x01 a
//! colour change, 0x02 a jump (a jump of zero is a trim), and `80 10` ends the design.
//!
//! Thread numbers index Janome's thread palette. VectorCraft ships no maker's palette (as with
//! thread charts, see `docs/embroidery.md` › Thread charts), so it writes a number per distinct
//! colour (1, 2, 3 …) and reads each number as a colour of its own: blocks of one thread keep one
//! colour, but the machine shows its own colour for each number.

use crate::machine::{Op, Reader, ascii, bounds, ops, steps, u32_at};
use crate::{MAX_NEEDLES, Plan, palette_color};

const MAX_STEP: i64 = 127;
const ESC: u8 = 0x80;
const COLOR: u8 = 0x01;
const JUMP: u8 = 0x02;
const END: u8 = 0x10;
/// The header before the thread lists.
const FIXED: usize = 116;
/// More threads than any design has: past it a header is taken as damaged.
const MAX_THREADS: usize = 1000;
/// The hoops the header describes: (code, width, height), mm, smallest first.
const HOOPS: [(u32, i64, i64); 4] = [(1, 50, 50), (0, 110, 110), (2, 140, 200), (4, 200, 200)];

fn byte(v: i64) -> u8 {
    (v.clamp(-MAX_STEP, MAX_STEP) as i8) as u8
}

/// Does `bytes` look like a JEF header (its stitch offset agrees with its thread count, and the
/// date is digits)?
pub fn is_jef(bytes: &[u8]) -> bool {
    let (Some(off), Some(n)) = (u32_at(bytes, 0), u32_at(bytes, 24)) else { return false };
    let n = n as usize;
    n <= MAX_THREADS
        && off as usize == FIXED + 8 * n
        && bytes.len() >= off as usize
        && bytes.get(8..22).is_some_and(|d| d.iter().all(|c| c.is_ascii_digit() || *c == 0))
}

/// The number written for each colour block: one per distinct colour, from 1.
fn numbers(threads: &[[u8; 3]]) -> Vec<u32> {
    let mut seen: Vec<[u8; 3]> = vec![];
    threads
        .iter()
        .map(|t| {
            let i = seen.iter().position(|s| s == t).unwrap_or_else(|| {
                seen.push(*t);
                seen.len() - 1
            });
            i as u32 + 1
        })
        .collect()
}

/// `plan` as a JEF file, centred on the design's middle, dated `date` (year, month, day, hour,
/// minute, second).
pub fn write_jef(plan: &Plan, date: [i64; 6]) -> Vec<u8> {
    let centre = bounds(plan).map(|(lo, hi)| ((lo.0 + hi.0) / 2.0, (lo.1 + hi.1) / 2.0)).unwrap_or((0.0, 0.0));
    let mut body = Vec::with_capacity(plan.needles.len() * 2);
    let mut pos = (0i64, 0i64);
    // The extents in 0.1 mm, y down: (min x, min y, max x, max y).
    let mut ext = (0i64, 0i64, 0i64, 0i64);
    let mut colors = 0usize;
    for op in ops(plan, centre) {
        match op {
            Op::Stitch(x, y) | Op::Jump(x, y) => {
                let jump = matches!(op, Op::Jump(..));
                for (dx, dy) in steps(pos, (x, y), MAX_STEP) {
                    if jump {
                        // A jump of zero would read as a trim.
                        if (dx, dy) != (0, 0) {
                            body.extend_from_slice(&[ESC, JUMP, byte(dx), byte(-dy)]);
                        }
                    } else {
                        body.extend_from_slice(&[byte(dx), byte(-dy)]);
                    }
                }
                pos = (x, y);
                ext = (ext.0.min(x), ext.1.min(y), ext.2.max(x), ext.3.max(y));
            }
            Op::Trim => body.extend_from_slice(&[ESC, JUMP, 0, 0]),
            Op::Color => {
                body.extend_from_slice(&[ESC, COLOR, 0, 0]);
                colors += 1;
            }
        }
    }
    body.extend_from_slice(&[ESC, END]);
    // One thread per colour block (a plan's thread list can be longer or shorter).
    let threads: Vec<[u8; 3]> = (0..=colors).map(|i| plan.threads.get(i).copied().unwrap_or_else(|| palette_color(i))).collect();
    let n = threads.len();
    let mut out = Vec::with_capacity(FIXED + 8 * n + body.len());
    let put = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
    put(&mut out, (FIXED + 8 * n) as u32);
    put(&mut out, 0x14);
    let [y, mo, d, h, mi, s] = date.map(|v| v.max(0));
    let stamp = format!("{:04}{:02}{:02}{:02}{:02}{:02}", y.min(9999), mo.min(99), d.min(99), h.min(99), mi.min(99), s.min(99));
    out.extend_from_slice(ascii(&stamp, 14).as_bytes());
    out.resize(22, b'0');
    out.extend_from_slice(&[0, 0]);
    put(&mut out, n as u32);
    put(&mut out, (body.len() / 2) as u32);
    // Distances from the centre to each side, 0.1 mm: left, top, right, bottom.
    let sides = [-ext.0, -ext.1, ext.2, ext.3].map(|v| v.max(0));
    let fits = |w: i64, h: i64| sides[0] <= w * 5 && sides[2] <= w * 5 && sides[1] <= h * 5 && sides[3] <= h * 5;
    let hoop = HOOPS.iter().find(|(_, w, h)| fits(*w, *h)).or(HOOPS.last()).copied().unwrap_or((4, 200, 200));
    put(&mut out, hoop.0);
    for v in sides {
        put(&mut out, v.min(u32::MAX as i64) as u32);
    }
    let room = |w: i64, h: i64| -> [i32; 4] {
        if fits(w, h) { [w * 5 - sides[0], h * 5 - sides[1], w * 5 - sides[2], h * 5 - sides[3]].map(|v| v as i32) } else { [-1; 4] }
    };
    for (w, h) in [(110, 110), (50, 50), (140, 200), (hoop.1, hoop.2)] {
        for v in room(w, h) {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    for v in numbers(&threads) {
        put(&mut out, v);
    }
    for _ in 0..n {
        put(&mut out, 0x0D);
    }
    out.extend_from_slice(&body);
    out
}

/// Read a JEF file into a plan: each thread number gets a colour of its own (VectorCraft has no
/// Janome palette), the same number the same colour. Errors when the header doesn't read.
pub fn read_jef(bytes: &[u8]) -> Result<Plan, String> {
    let (Some(off), Some(n)) = (u32_at(bytes, 0), u32_at(bytes, 24)) else {
        return Err("not a JEF embroidery file (its header is cut short)".into());
    };
    let (off, n) = (off as usize, n as usize);
    if n > MAX_THREADS || off < FIXED || off > bytes.len() {
        return Err("not a JEF embroidery file (its header doesn't add up)".into());
    }
    // Thread numbers, as many as the header has room for.
    let numbers: Vec<u32> = (0..n).map_while(|i| u32_at(bytes, FIXED + 4 * i).filter(|_| FIXED + 4 * i + 4 <= off)).collect();
    let threads: Vec<[u8; 3]> = numbers.iter().map(|&k| palette_color(k as usize)).collect();
    let mut r = Reader::new(true);
    let mut i = off;
    let mut ended = false;
    let mut unknown = 0usize;
    while let (Some(&a), Some(&b)) = (bytes.get(i), bytes.get(i + 1)) {
        if r.full() || r.plan.needles.len() >= MAX_NEEDLES {
            break;
        }
        if a != ESC {
            r.stitch(a as i8 as i64, b as i8 as i64);
            i += 2;
            continue;
        }
        if b == END {
            ended = true;
            break;
        }
        let (dx, dy) = (bytes.get(i + 2).map_or(0, |&v| v as i8 as i64), bytes.get(i + 3).map_or(0, |&v| v as i8 as i64));
        i += 4;
        match b {
            COLOR => r.color(),
            JUMP if (dx, dy) == (0, 0) => r.trim(),
            JUMP => r.jump(dx, dy),
            _ => unknown += 1,
        }
    }
    if unknown > 0 {
        r.plan.warnings.push(format!("{unknown} commands this reader doesn't know were skipped"));
    }
    Ok(r.finish(&threads, ended))
}
