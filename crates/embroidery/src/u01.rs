//! Barudan U01 (FDR; also `.u00` to `.u99`).
//!
//! Layout, from the public description of the format
//! (<https://edutechwiki.unige.ch/en/Embroidery_format_U%3F%3F>), little-endian: 128 bytes of
//! ASCII `0`; at 0x80 the design's extents (`s16` left, bottom, right, top, 0.1 mm, y up), a `u32`
//! 0, the `u32` number of records, the last position (`s16` x, y); the rest of the 256-byte header
//! is zeros (a machine that edited the file may keep its own data there). From 0x100 come
//! three-byte records: a control byte (bit 7 always set, bit 6 the move's y is negative, bit 5 its
//! x is negative, bits 0–4 the command), then the y and x distances, 0–255 units of 0.1 mm.
//!
//! Commands: 0 stitch, 1 jump (2–5: the same at high or low speed), 6 and 7 trims (7 is the one
//! nearly every file uses), 8 a stop, 9 to 17 change to needle 1 to 9, 18 the end. Some writers
//! end with 24 (0x18) instead, which the description doesn't otherwise use: it is read as the end
//! too.
//!
//! A U01 file names needles, not colours: the machine sews each needle's block with whatever
//! thread is on it. VectorCraft writes a needle per distinct colour (1, 2, 3 …, at most 9, then
//! starting again) and reads each needle as a colour of its own.

use crate::machine::{Op, Reader, bounds, ops, steps};
use crate::{MAX_NEEDLES, Plan, palette_color};

/// Where the records start.
const BODY: usize = 0x100;
/// The longest move one record makes, 0.1 mm: a jump's.
const MAX_STEP: i64 = 255;
/// The longest stitch written, 0.1 mm: machines sew nothing longer (as the other formats split
/// theirs).
const MAX_STITCH: i64 = 127;
const STITCH: u8 = 0;
const JUMP: u8 = 1;
const TRIM: u8 = 7;
const STOP: u8 = 8;
const NEEDLE_1: u8 = 9;
/// The highest needle a record can name.
pub const NEEDLES: u8 = 9;
const END: u8 = 18;
/// The end as some writers put it.
const END_HEX: u8 = 0x18;

/// A record: command `cmd`, moving (dx, dy) (0.1 mm, y up).
fn record(out: &mut Vec<u8>, cmd: u8, dx: i64, dy: i64) {
    let mut c = 0x80 | (cmd & 0x1F);
    if dy < 0 {
        c |= 0x40;
    }
    if dx < 0 {
        c |= 0x20;
    }
    out.extend_from_slice(&[c, dy.unsigned_abs().min(255) as u8, dx.unsigned_abs().min(255) as u8]);
}

/// The needle written for each colour block: one per distinct colour, from 1, at most
/// [`NEEDLES`] (then starting again), and whether there were more colours than needles.
fn needles(threads: &[[u8; 3]]) -> (Vec<u8>, bool) {
    let mut seen: Vec<[u8; 3]> = vec![];
    let out = threads
        .iter()
        .map(|t| {
            let i = seen.iter().position(|s| s == t).unwrap_or_else(|| {
                seen.push(*t);
                seen.len() - 1
            });
            (i % usize::from(NEEDLES)) as u8 + 1
        })
        .collect();
    (out, seen.len() > usize::from(NEEDLES))
}

/// `plan` as a U01 file, centred on the design's middle, and warnings for what U01 can't hold.
pub fn write_u01(plan: &Plan) -> (Vec<u8>, Vec<String>) {
    let mut warnings = vec![];
    let centre = bounds(plan).map(|(lo, hi)| ((lo.0 + hi.0) / 2.0, (lo.1 + hi.1) / 2.0)).unwrap_or((0.0, 0.0));
    let plan_ops = ops(plan, centre);
    let blocks = 1 + plan_ops.iter().filter(|o| matches!(o, Op::Color)).count();
    let threads: Vec<[u8; 3]> = (0..blocks).map(|i| plan.threads.get(i).copied().unwrap_or_else(|| palette_color(i))).collect();
    let (needle, wrapped) = needles(&threads);
    if wrapped {
        warnings.push(format!(
            "the design has more than {NEEDLES} colours and U01 names at most {NEEDLES} needles: some needles sew more than one colour"
        ));
    }
    let mut body = vec![];
    record(&mut body, NEEDLE_1 - 1 + needle.first().copied().unwrap_or(1), 0, 0);
    let mut pos = (0i64, 0i64);
    // Extents, 0.1 mm, y down: (min x, min y, max x, max y).
    let mut ext = (0i64, 0i64, 0i64, 0i64);
    let mut block = 0usize;
    for op in plan_ops {
        match op {
            Op::Stitch(x, y) | Op::Jump(x, y) => {
                let (cmd, max) = if matches!(op, Op::Jump(..)) { (JUMP, MAX_STEP) } else { (STITCH, MAX_STITCH) };
                for (dx, dy) in steps(pos, (x, y), max) {
                    record(&mut body, cmd, dx, -dy);
                }
                pos = (x, y);
                ext = (ext.0.min(x), ext.1.min(y), ext.2.max(x), ext.3.max(y));
            }
            Op::Trim => record(&mut body, TRIM, 0, 0),
            Op::Color => {
                block += 1;
                // The same needle again is a stop to change its thread.
                let (prev, next) = (needle.get(block - 1).copied(), needle.get(block).copied().unwrap_or(1));
                let cmd = if prev == Some(next) { STOP } else { NEEDLE_1 - 1 + next };
                record(&mut body, cmd, 0, 0);
            }
        }
    }
    record(&mut body, END, 0, 0);
    let mut out = vec![b'0'; 0x80];
    let s16 = |out: &mut Vec<u8>, v: i64| out.extend_from_slice(&(v.clamp(-32768, 32767) as i16).to_le_bytes());
    // Left, bottom, right, top, y up.
    for v in [ext.0, -ext.3, ext.2, -ext.1] {
        s16(&mut out, v);
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&((body.len() / 3) as u32).to_le_bytes());
    s16(&mut out, pos.0);
    s16(&mut out, -pos.1);
    out.resize(BODY, 0);
    out.extend_from_slice(&body);
    (out, warnings)
}

/// Does `bytes` start as U01 files do (128 ASCII zeros)?
pub fn is_u01(bytes: &[u8]) -> bool {
    bytes.len() >= BODY && bytes.get(..0x80).is_some_and(|h| h.iter().all(|&b| b == b'0'))
}

/// Read a U01 file into a plan: each needle gets a colour of its own, the same needle the same
/// colour. Errors when it is too short to hold the header.
pub fn read_u01(bytes: &[u8]) -> Result<Plan, String> {
    if bytes.len() < BODY {
        return Err("not a U01 embroidery file (it is shorter than its header)".into());
    }
    let mut r = Reader::new(true);
    // Each colour block's needle; the first block's is the first needle named before sewing.
    let mut block_needles: Vec<u8> = vec![];
    let mut needle: Option<u8> = None;
    let mut sewn = false;
    let mut ended = false;
    let mut unknown = 0usize;
    for &[c, y, x] in bytes.get(BODY..).unwrap_or_default().as_chunks::<3>().0 {
        if r.full() || r.plan.needles.len() >= MAX_NEEDLES {
            break;
        }
        let dy = if c & 0x40 != 0 { -i64::from(y) } else { i64::from(y) };
        let dx = if c & 0x20 != 0 { -i64::from(x) } else { i64::from(x) };
        match c & 0x1F {
            STITCH | 2 | 4 => {
                sewn = true;
                r.stitch(dx, dy);
            }
            // Jumps before the first needle is named (to where sewing starts) don't start a block.
            JUMP | 3 | 5 => r.jump(dx, dy),
            6 | TRIM => r.trim(),
            STOP => {
                block_needles.push(needle.unwrap_or(1));
                r.color();
            }
            n @ NEEDLE_1..=17 => {
                let n = n - NEEDLE_1 + 1;
                // The first needle named before anything is sewn is where the design starts.
                if needle.is_none() && !sewn && block_needles.is_empty() {
                    needle = Some(n);
                } else if needle != Some(n) {
                    block_needles.push(needle.unwrap_or(1));
                    needle = Some(n);
                    r.color();
                }
            }
            END | END_HEX => {
                ended = true;
                break;
            }
            _ => unknown += 1,
        }
    }
    block_needles.push(needle.unwrap_or(1));
    if unknown > 0 {
        r.plan.warnings.push(format!("{unknown} commands this reader doesn't know were skipped"));
    }
    let threads: Vec<[u8; 3]> = block_needles.iter().map(|&n| palette_color(usize::from(n))).collect();
    Ok(r.finish(&threads, ended))
}
