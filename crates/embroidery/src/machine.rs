//! What the machine formats that store relative needle moves share (EXP, JEF, PES/PEC): a plan
//! as moves in 0.1 mm units, split to a format's longest move, and a plan read back from moves.

use crate::{Cmd, MAX_NEEDLES, Needle, Plan, palette_color};

/// One needle move to an absolute position, 0.1 mm, with y down (as the plan's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Op {
    Stitch(i64, i64),
    Jump(i64, i64),
    Trim,
    Color,
}

/// The bounds of the plan's stitches and jumps, mm: (min, max), or `None` for an empty plan.
pub(crate) fn bounds(plan: &Plan) -> Option<((f64, f64), (f64, f64))> {
    let (mut lo, mut hi) = ((f64::INFINITY, f64::INFINITY), (f64::NEG_INFINITY, f64::NEG_INFINITY));
    for n in plan.needles.iter().filter(|n| matches!(n.cmd, Cmd::Stitch | Cmd::Jump)) {
        if n.x.is_finite() && n.y.is_finite() {
            lo = (lo.0.min(n.x), lo.1.min(n.y));
            hi = (hi.0.max(n.x), hi.1.max(n.y));
        }
    }
    (lo.0.is_finite() && hi.0.is_finite()).then_some((lo, hi))
}

/// The plan's moves in 0.1 mm from `origin` (mm), up to its end; a jump to the first stitch when
/// the plan starts with one.
pub(crate) fn ops(plan: &Plan, origin: (f64, f64)) -> Vec<Op> {
    // Positions far beyond any hoop are clamped (a stray point can't overflow the arithmetic).
    let unit = |v: f64, o: f64| ((v - o) * 10.0).round().clamp(-1e7, 1e7) as i64;
    let mut out = Vec::with_capacity(plan.needles.len() + 1);
    let mut moved = false;
    for n in &plan.needles {
        let at = || (unit(n.x, origin.0), unit(n.y, origin.1));
        match n.cmd {
            Cmd::Stitch if n.x.is_finite() && n.y.is_finite() => {
                // The machine starts at the origin: a design that starts with a stitch
                // gets there by a jump, not by sewing a line to it.
                if !moved {
                    out.push(Op::Jump(at().0, at().1));
                    moved = true;
                }
                out.push(Op::Stitch(at().0, at().1));
            }
            Cmd::Jump if n.x.is_finite() && n.y.is_finite() => {
                moved = true;
                out.push(Op::Jump(at().0, at().1));
            }
            Cmd::Stitch | Cmd::Jump => {}
            Cmd::Trim => out.push(Op::Trim),
            Cmd::ColorChange => out.push(Op::Color),
            Cmd::End => break,
        }
    }
    out
}

/// The steps from `from` to `to`, each at most `max` units on either axis, as equal as rounding
/// allows (one zero step when they are the same point).
pub(crate) fn steps(from: (i64, i64), to: (i64, i64), max: i64) -> Vec<(i64, i64)> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let max = max.max(1);
    let n = ((dx.abs().max(dy.abs()) + max - 1) / max).max(1);
    let mut prev = (0, 0);
    let mut out = Vec::with_capacity(n as usize);
    for i in 1..=n {
        let at = (((dx * i) as f64 / n as f64).round() as i64, ((dy * i) as f64 / n as f64).round() as i64);
        out.push((at.0 - prev.0, at.1 - prev.1));
        prev = at;
    }
    out
}

/// Builds a plan from relative moves in 0.1 mm as a file holds them.
pub(crate) struct Reader {
    pub plan: Plan,
    x: i64,
    y: i64,
    /// Whether the file's y axis points up (EXP, JEF) rather than down (PEC).
    y_up: bool,
    colors: usize,
    full: bool,
}

impl Reader {
    pub fn new(y_up: bool) -> Self {
        Reader { plan: Plan::default(), x: 0, y: 0, y_up, colors: 0, full: false }
    }

    /// Whether the plan is full ([`MAX_NEEDLES`]): the caller stops reading.
    pub fn full(&self) -> bool {
        self.full
    }

    fn push(&mut self, cmd: Cmd) {
        if self.plan.needles.len() >= MAX_NEEDLES {
            if !self.full {
                self.plan.warnings.push(format!("the design has more than {MAX_NEEDLES} needle moves: the rest is left out"));
            }
            self.full = true;
            return;
        }
        let y = if self.y_up { -self.y } else { self.y };
        self.plan.needles.push(Needle { x: self.x as f64 / 10.0, y: y as f64 / 10.0, cmd });
    }

    fn step(&mut self, dx: i64, dy: i64) {
        // Saturating: a hostile file of long moves can't overflow.
        self.x = self.x.saturating_add(dx).clamp(-1e9 as i64, 1e9 as i64);
        self.y = self.y.saturating_add(dy).clamp(-1e9 as i64, 1e9 as i64);
    }

    pub fn stitch(&mut self, dx: i64, dy: i64) {
        self.step(dx, dy);
        self.push(Cmd::Stitch);
    }

    pub fn jump(&mut self, dx: i64, dy: i64) {
        self.step(dx, dy);
        self.push(Cmd::Jump);
    }

    pub fn trim(&mut self) {
        self.push(Cmd::Trim);
    }

    pub fn color(&mut self) {
        self.colors += 1;
        self.push(Cmd::ColorChange);
    }

    /// The plan, ended, with `threads` (one per colour block; missing ones from
    /// [`palette_color`]), and a warning when the file didn't end as it should.
    pub fn finish(mut self, threads: &[[u8; 3]], ended: bool) -> Plan {
        if !ended && !self.full {
            self.plan.warnings.push("the file ends without an end code: it may be cut short".into());
        }
        if let Some(last) = self.plan.needles.last().copied() {
            self.plan.needles.push(Needle { cmd: Cmd::End, ..last });
        }
        self.plan.threads = (0..=self.colors).map(|i| threads.get(i).copied().unwrap_or_else(|| palette_color(i))).collect();
        self.plan
    }
}

/// A little-endian `u32` at `at`, if the bytes are there.
pub(crate) fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

/// A little-endian `u16` at `at`, if the bytes are there.
pub(crate) fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at.checked_add(2)?)?.try_into().ok()?))
}

/// `label`'s printable ASCII, at most `n` characters.
pub(crate) fn ascii(label: &str, n: usize) -> String {
    label.chars().filter(|c| c.is_ascii_graphic() || *c == ' ').take(n).collect()
}
