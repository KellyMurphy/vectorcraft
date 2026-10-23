//! Melco EXP: no header, only needle moves.
//!
//! Layout, from the public description of the format
//! (<https://edutechwiki.unige.ch/en/Embroidery_format_EXP>): each record is two signed bytes, x then
//! y, a move of up to ±127 units of 0.1 mm from the last point, with y pointing up. An x byte of
//! 0x80 starts a 4-byte control record `80 cc dx dy`: `cc` 0x04 is a jump by (dx, dy), 0x01 a colour
//! change, 0x02 a stitch. The format holds no thread colours, no design size and no end code.
//!
//! Trims: the description gives no trim code (0x80 0x80 is listed as END, and other readers take
//! it as a trim), so a trim is written as DST writes one: three short jumps that come back to where
//! they started, which machines set to trim on jumps read as "cut the thread". On reading, `80 80`
//! with more records after it is a trim, and at the very end the end of the design.

use crate::machine::{Op, Reader, bounds, ops, steps};
use crate::{MAX_NEEDLES, Plan};

/// The longest move one record makes, 0.1 mm.
const MAX_STEP: i64 = 127;
const ESC: u8 = 0x80;
const JUMP: u8 = 0x04;
const COLOR: u8 = 0x01;
const STITCH: u8 = 0x02;
const TRIM_OR_END: u8 = 0x80;

fn byte(v: i64) -> u8 {
    (v.clamp(-MAX_STEP, MAX_STEP) as i8) as u8
}

/// `plan` as an EXP file, centred on the design's middle (where the machine starts).
pub fn write_exp(plan: &Plan) -> Vec<u8> {
    let centre = bounds(plan).map(|(lo, hi)| ((lo.0 + hi.0) / 2.0, (lo.1 + hi.1) / 2.0)).unwrap_or((0.0, 0.0));
    let mut out = Vec::with_capacity(plan.needles.len() * 2);
    let mut pos = (0i64, 0i64);
    for op in ops(plan, centre) {
        match op {
            Op::Stitch(x, y) => {
                for (dx, dy) in steps(pos, (x, y), MAX_STEP) {
                    out.extend_from_slice(&[byte(dx), byte(-dy)]);
                }
                pos = (x, y);
            }
            Op::Jump(x, y) => {
                for (dx, dy) in steps(pos, (x, y), MAX_STEP) {
                    out.extend_from_slice(&[ESC, JUMP, byte(dx), byte(-dy)]);
                }
                pos = (x, y);
            }
            Op::Trim => {
                for d in [2i64, -4, 2] {
                    out.extend_from_slice(&[ESC, JUMP, byte(d), byte(d)]);
                }
            }
            Op::Color => out.extend_from_slice(&[ESC, COLOR, 0, 0]),
        }
    }
    out
}

/// Read an EXP file into a plan (threads from [`crate::palette_color`]: EXP holds no colours).
/// Any bytes read: there is no header to check, so the caller picks EXP by the file's name.
pub fn read_exp(bytes: &[u8]) -> Plan {
    let mut r = Reader::new(true);
    let mut i = 0usize;
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
        let (dx, dy) = (bytes.get(i + 2).map_or(0, |&v| v as i8 as i64), bytes.get(i + 3).map_or(0, |&v| v as i8 as i64));
        i += 4;
        match b {
            JUMP => r.jump(dx, dy),
            STITCH => r.stitch(dx, dy),
            TRIM_OR_END if i >= bytes.len() => break,
            TRIM_OR_END => r.trim(),
            // The description: other odd codes are colour changes, even ones stops (a stop
            // to change thread, to the machine as to the plan).
            _ => {
                if b != COLOR {
                    unknown += 1;
                }
                r.color();
            }
        }
    }
    if unknown > 0 {
        r.plan.warnings.push(format!("{unknown} control codes this reader doesn't know were read as thread stops"));
    }
    r.finish(&[], true)
}
