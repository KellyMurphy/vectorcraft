//! Tajima DST, the format nearly every embroidery machine reads.
//!
//! Layout, from the public descriptions of the format
//! (<https://edutechwiki.unige.ch/en/Embroidery_format_DST>): a 512-byte text header
//! (`LA:` label, `ST:` record count, `CO:` colour changes, `+X:`/`-X:`/`+Y:`/`-Y:` extents,
//! `AX:`/`AY:` end point, `MX:`/`MY:`, `PD:`, then 0x1A and spaces), then 3-byte records. Each
//! record moves the needle by up to ±121 units (0.1 mm) on each axis, written in balanced ternary
//! (1, 3, 9, 27, 81) as plus/minus bits; the third byte's top bits mark a jump (0x80) or a colour
//! change (0xC0), and `00 00 F3` ends the design. Y points up. The format holds no thread colours.
//!
//! A trim is written as three jumps that come back to where they started, which machines read
//! as "cut the thread".

use std::fmt;

use crate::{Cmd, MAX_NEEDLES, Needle, Plan, palette_color};

const HEADER: usize = 512;
/// The longest move one record makes, 0.1 mm.
const MAX_STEP: i64 = 121;
/// Weights of the balanced-ternary digits.
const WEIGHTS: [i64; 5] = [1, 3, 9, 27, 81];
/// For each digit: the (plus, minus) bit of an axis, as (byte, mask).
type Bits = [((usize, u8), (usize, u8)); 5];
const X_BITS: Bits = [((0, 0x01), (0, 0x02)), ((1, 0x01), (1, 0x02)), ((0, 0x04), (0, 0x08)), ((1, 0x04), (1, 0x08)), ((2, 0x04), (2, 0x08))];
const Y_BITS: Bits = [((0, 0x80), (0, 0x40)), ((1, 0x80), (1, 0x40)), ((0, 0x20), (0, 0x10)), ((1, 0x20), (1, 0x10)), ((2, 0x20), (2, 0x10))];
const JUMP: u8 = 0x80;
const COLOR: u8 = 0xC0;
const END: [u8; 3] = [0x00, 0x00, 0xF3];

#[derive(Debug, Clone, PartialEq)]
pub enum DstError {
    NotDst,
}

impl fmt::Display for DstError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DstError::NotDst => write!(f, "not a DST embroidery file (no `LA:` header)"),
        }
    }
}

impl std::error::Error for DstError {}

/// Does `bytes` start like a DST header?
pub fn is_dst(bytes: &[u8]) -> bool {
    bytes.starts_with(b"LA:") && bytes.get(..HEADER.min(bytes.len())).is_some_and(|h| h.windows(4).any(|w| w == b"\rST:"))
}

/// The balanced-ternary bits of a move of `v` (|v| ≤ 121) on one axis.
fn put(v: i64, bits: &Bits, rec: &mut [u8; 3]) {
    let mut v = v;
    for &(plus, minus) in bits {
        let r = v.rem_euclid(3);
        let (bit, next) = match r {
            0 => (None, v / 3),
            1 => (Some(plus), (v - 1) / 3),
            _ => (Some(minus), (v + 1) / 3),
        };
        if let Some((byte, mask)) = bit
            && let Some(b) = rec.get_mut(byte)
        {
            *b |= mask;
        }
        v = next;
    }
}

fn record(dx: i64, dy: i64, flags: u8) -> [u8; 3] {
    let mut rec = [0, 0, flags | 0x03];
    put(dx, &X_BITS, &mut rec);
    put(dy, &Y_BITS, &mut rec);
    rec
}

fn take(v: i64, bits: &Bits, rec: &[u8; 3]) -> i64 {
    let on = |(byte, mask): (usize, u8)| rec.get(byte).is_some_and(|b| b & mask != 0);
    bits.iter().zip(WEIGHTS).map(|(&(p, m), w)| (on(p) as i64 - on(m) as i64) * w).sum::<i64>() + v
}

/// Writes records, splitting long moves.
struct Writer {
    out: Vec<u8>,
    pos: (i64, i64),
    records: usize,
    colors: usize,
    min: (i64, i64),
    max: (i64, i64),
}

impl Writer {
    fn raw(&mut self, rec: [u8; 3]) {
        self.out.extend_from_slice(&rec);
        self.records += 1;
    }

    /// Move to `to` in records of at most 121 units a side, all stitches or all jumps.
    fn go(&mut self, to: (i64, i64), flags: u8) {
        let (dx, dy) = (to.0 - self.pos.0, to.1 - self.pos.1);
        let steps = ((dx.abs().max(dy.abs()) + MAX_STEP - 1) / MAX_STEP).max(1);
        let (start, mut prev) = (self.pos, (0, 0));
        for i in 1..=steps {
            let at = ((dx * i) as f64 / steps as f64).round() as i64;
            let bt = ((dy * i) as f64 / steps as f64).round() as i64;
            self.raw(record(at - prev.0, bt - prev.1, flags));
            prev = (at, bt);
        }
        self.pos = (start.0 + dx, start.1 + dy);
        self.min = (self.min.0.min(self.pos.0), self.min.1.min(self.pos.1));
        self.max = (self.max.0.max(self.pos.0), self.max.1.max(self.pos.1));
    }
}

/// `plan` as a DST file labelled `label` (its first 16 ASCII characters), centred on the
/// design's middle (where the machine starts).
pub fn write_dst(plan: &Plan, label: &str) -> Vec<u8> {
    let moves = plan.needles.iter().filter(|n| matches!(n.cmd, Cmd::Stitch | Cmd::Jump));
    let (mut lo, mut hi) = ((f64::INFINITY, f64::INFINITY), (f64::NEG_INFINITY, f64::NEG_INFINITY));
    for n in moves {
        lo = (lo.0.min(n.x), lo.1.min(n.y));
        hi = (hi.0.max(n.x), hi.1.max(n.y));
    }
    let centre = if lo.0.is_finite() && hi.0.is_finite() { ((lo.0 + hi.0) / 2.0, (lo.1 + hi.1) / 2.0) } else { (0.0, 0.0) };
    // mm (y down) → 0.1 mm units from the centre (y up).
    let units = |n: &Needle| (((n.x - centre.0) * 10.0).round() as i64, (-(n.y - centre.1) * 10.0).round() as i64);
    let mut w = Writer { out: vec![], pos: (0, 0), records: 0, colors: 0, min: (0, 0), max: (0, 0) };
    for n in &plan.needles {
        match n.cmd {
            Cmd::Stitch => w.go(units(n), 0),
            Cmd::Jump => w.go(units(n), JUMP),
            Cmd::Trim => {
                for d in [2, -4, 2] {
                    w.raw(record(d, d, JUMP));
                }
            }
            Cmd::ColorChange => {
                w.raw(record(0, 0, COLOR));
                w.colors += 1;
            }
            Cmd::End => break,
        }
    }
    let records = w.records;
    w.out.extend_from_slice(&END);
    let label: String = label.chars().filter(|c| c.is_ascii_graphic() || *c == ' ').take(16).collect();
    let signed = |v: i64| format!("{}{:>5}", if v < 0 { '-' } else { '+' }, v.unsigned_abs().min(99_999));
    let n5 = |v: i64| v.unsigned_abs().min(99_999);
    let header = format!(
        "LA:{label:<16}\rST:{:>7}\rCO:{:>3}\r+X:{:>5}\r-X:{:>5}\r+Y:{:>5}\r-Y:{:>5}\rAX:{}\rAY:{}\rMX:+    0\rMY:+    0\rPD:******\r",
        records.min(9_999_999),
        w.colors.min(999),
        n5(w.max.0),
        n5(w.min.0),
        n5(w.max.1),
        n5(w.min.1),
        signed(w.pos.0),
        signed(w.pos.1),
    );
    let mut out = header.into_bytes();
    out.push(0x1A);
    out.resize(HEADER, b' ');
    out.extend_from_slice(&w.out);
    out
}

/// Read a DST file into a plan (threads from [`palette_color`]: DST holds no colours).
pub fn read_dst(bytes: &[u8]) -> Result<Plan, DstError> {
    if !is_dst(bytes) {
        return Err(DstError::NotDst);
    }
    let mut plan = Plan::default();
    let body = bytes.get(HEADER..).unwrap_or_default();
    let (mut x, mut y) = (0i64, 0i64);
    let mut colors = 0;
    let mut ended = false;
    let (records, _) = body.as_chunks::<3>();
    for &rec in records {
        if rec[2] & 0xF3 == 0xF3 {
            ended = true;
            break;
        }
        if plan.needles.len() >= MAX_NEEDLES {
            plan.warnings.push(format!("the design has more than {MAX_NEEDLES} needle moves: the rest is left out"));
            break;
        }
        x = take(x, &X_BITS, &rec);
        y = take(y, &Y_BITS, &rec);
        let mm = (x as f64 / 10.0, -(y as f64) / 10.0);
        let cmd = match rec[2] & 0xC0 {
            COLOR => {
                colors += 1;
                Cmd::ColorChange
            }
            JUMP => Cmd::Jump,
            // 0x40 (sequins) and plain stitches both sew.
            _ => Cmd::Stitch,
        };
        plan.needles.push(Needle { x: mm.0, y: mm.1, cmd });
    }
    if !ended {
        plan.warnings.push("the file ends without an end record: it may be cut short".into());
    }
    if let Some(last) = plan.needles.last().copied() {
        plan.needles.push(Needle { cmd: Cmd::End, ..last });
    }
    plan.threads = (0..=colors).map(palette_color).collect();
    Ok(plan)
}

#[cfg(test)]
pub(crate) fn tests_record(dx: i64, dy: i64, flags: u8) -> [u8; 3] {
    record(dx, dy, flags)
}

#[cfg(test)]
pub(crate) fn tests_decode(rec: &[u8; 3]) -> (i64, i64) {
    (take(0, &X_BITS, rec), take(0, &Y_BITS, rec))
}
