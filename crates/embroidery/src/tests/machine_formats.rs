//! PES, JEF and EXP: round trips, headers, and readers that survive any bytes.

use super::*;

const DATE: [i64; 6] = [2026, 10, 22, 9, 30, 5];

#[derive(Clone, Copy, Debug)]
enum Fmt {
    Pes,
    Jef,
    Exp,
}

const ALL: [Fmt; 3] = [Fmt::Pes, Fmt::Jef, Fmt::Exp];

fn write(f: Fmt, p: &Plan) -> Vec<u8> {
    match f {
        Fmt::Pes => write_pes(p, "test design").0,
        Fmt::Jef => write_jef(p, DATE),
        Fmt::Exp => write_exp(p),
    }
}

fn read(f: Fmt, b: &[u8]) -> Plan {
    match f {
        Fmt::Pes => read_pes(b).unwrap(),
        Fmt::Jef => read_jef(b).unwrap(),
        Fmt::Exp => read_exp(b),
    }
}

fn stitches(p: &Plan) -> Vec<Point> {
    p.needles.iter().filter(|n| n.cmd == Cmd::Stitch).map(|n| Point::new(n.x, n.y)).collect()
}

/// Where the needle ends up: the last stitch or jump.
fn last_move(p: &Plan) -> Point {
    let n = p.needles.iter().rev().find(|n| matches!(n.cmd, Cmd::Stitch | Cmd::Jump)).unwrap();
    Point::new(n.x, n.y)
}

/// Every planned stitch is sewn where it was planned (within the formats' 0.1 mm), relative to
/// where the design ends (files keep relative moves).
fn same_stitches(planned: &Plan, back: &Plan, what: &str) {
    let (a, b) = (stitches(planned), stitches(back));
    let shift = last_move(back) - last_move(planned);
    let mut j = 0;
    for p in &a {
        let want = *p + shift;
        // In order: the read-back has the same stitches, plus the splits of long ones.
        while j < b.len() && b[j].distance(want) > 0.15 {
            j += 1;
        }
        assert!(j < b.len(), "{what}: stitch at {p:?} not sewn (shift {shift:?})");
    }
    assert!(b.len() >= a.len(), "{what}: {} stitches back from {}", b.len(), a.len());
}

#[test]
fn every_format_round_trips_a_busy_design() {
    let p = plan(&busy_doc());
    let s = p.stats();
    for f in ALL {
        let back = read(f, &write(f, &p));
        assert!(back.warnings.is_empty(), "{f:?}: {:?}", back.warnings);
        same_stitches(&p, &back, &format!("{f:?}"));
        let b = back.stats();
        assert_eq!(b.color_changes, s.color_changes, "{f:?}");
        assert_eq!(back.threads.len(), p.threads.len(), "{f:?}");
        let (wa, wb) = (s.bounds.unwrap(), b.bounds.unwrap());
        assert!(((wa[2] - wa[0]) - (wb[2] - wb[0])).abs() <= 0.1 + 1e-9, "{f:?}");
        assert!(((wa[3] - wa[1]) - (wb[3] - wb[1])).abs() <= 0.1 + 1e-9, "{f:?}");
        // PES and JEF have trim codes; EXP's trims come back as the jumps that make them.
        if !matches!(f, Fmt::Exp) {
            assert_eq!(b.trims, s.trims, "{f:?}");
        }
    }
}

#[test]
fn pes_keeps_thread_colours_and_jef_keeps_which_blocks_share_a_thread() {
    let d = doc_with(|d| {
        vec![
            stitched(d, poly(&[(0.0, 0.0), (90.0, 0.0)], false), StitchKind::Running, [200, 10, 30]),
            stitched(d, poly(&[(0.0, 40.0), (90.0, 40.0)], false), StitchKind::Running, [5, 6, 7]),
            stitched(d, poly(&[(0.0, 80.0), (90.0, 80.0)], false), StitchKind::Running, [200, 10, 30]),
        ]
    });
    let p = plan(&d);
    assert_eq!(p.threads, vec![[200, 10, 30], [5, 6, 7], [200, 10, 30]]);
    assert_eq!(read(Fmt::Pes, &write(Fmt::Pes, &p)).threads, p.threads);
    let jef = read(Fmt::Jef, &write(Fmt::Jef, &p)).threads;
    assert_eq!(jef.len(), 3);
    assert!(jef[0] == jef[2] && jef[0] != jef[1], "{jef:?}");
    // EXP has no colours: a distinct one per block.
    let exp = read(Fmt::Exp, &write(Fmt::Exp, &p)).threads;
    assert_eq!(exp.len(), 3);
}

#[test]
fn jef_header_adds_up() {
    let p = plan(&busy_doc());
    let b = write_jef(&p, DATE);
    assert!(is_jef(&b));
    let u = |at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let n = u(24) as usize;
    assert_eq!(n, p.threads.len());
    assert_eq!(u(0) as usize, 116 + 8 * n);
    assert_eq!(&b[8..22], b"20261022093005");
    // The point count is the stitch data's length in 2-byte units, ending `80 10`.
    assert_eq!(u(28) as usize * 2, b.len() - u(0) as usize);
    assert!(b.ends_with(&[0x80, 0x10]));
    // The busy design is about 70 × 70 mm: the 110 × 110 hoop (code 0), with room on each side.
    assert_eq!(u(32), 0);
    let ext: Vec<u32> = (0..4).map(|i| u(36 + 4 * i)).collect();
    let room: Vec<i32> = (0..4).map(|i| u(52 + 4 * i) as i32).collect();
    for (e, r) in ext.iter().zip(&room) {
        assert_eq!(*e as i32 + r, 550, "{ext:?} {room:?}");
    }
    // Too big for the 50 × 50 hoop: all −1.
    assert!((0..4).all(|i| u(68 + 4 * i) == u32::MAX));
    assert!((0..n).all(|i| u(116 + 4 * n + 4 * i) == 0x0D));
}

#[test]
fn pes_header_and_pec_section() {
    let p = plan(&busy_doc());
    let (b, warnings) = write_pes(&p, "Busy");
    assert!(warnings.is_empty());
    assert!(b.starts_with(b"#PES0060") && is_pes(&b));
    let pec = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
    assert!(b[pec..].starts_with(b"LA:Busy            \r"));
    assert_eq!(b[pec + 34], 6);
    assert_eq!(b[pec + 35], 38);
    assert_eq!(b[pec + 48] as usize, p.threads.len() - 1);
    // Palette numbers: one per distinct colour.
    assert_eq!(p.threads.len(), 3);
    assert_eq!(&b[pec + 49..pec + 52], &[1, 2, 3]);
    // The stitches start with a long jump; the graphics follow the end code.
    assert_eq!(b[pec + 528] & 0xF0, 0x90);
    let g = u32::from_le_bytes([b[pec + 514], b[pec + 515], b[pec + 516], 0]) as usize;
    assert_eq!(b[pec + 512 + g - 1], 0xFF);
    let thumbs = 228 * (1 + p.threads.len());
    assert_eq!(b.len(), pec + 512 + g + thumbs + 130);
    // The design's size, 0.1 mm.
    let s = p.stats().bounds.unwrap();
    let w = u16::from_le_bytes([b[pec + 520], b[pec + 521]]) as f64;
    assert!((w / 10.0 - (s[2] - s[0])).abs() <= 0.11, "{w}");
}

#[test]
fn long_moves_split_and_short_ones_stay_short() {
    let p = Plan {
        threads: vec![[0; 3]],
        needles: vec![
            Needle { x: 0.0, y: 0.0, cmd: Cmd::Jump },
            Needle { x: 1.0, y: 0.0, cmd: Cmd::Stitch },
            Needle { x: 51.0, y: 0.0, cmd: Cmd::Stitch },
            Needle { x: 351.0, y: 20.0, cmd: Cmd::Jump },
            Needle { x: 352.0, y: 20.0, cmd: Cmd::Stitch },
            Needle { x: 352.0, y: 20.0, cmd: Cmd::End },
        ],
        ..Plan::default()
    };
    for f in ALL {
        let back = read(f, &write(f, &p));
        same_stitches(&p, &back, &format!("{f:?}"));
        let s = stitches(&back);
        let longest = s.windows(2).map(|w| w[0].distance(w[1])).filter(|d| *d < 100.0).fold(0.0, f64::max);
        assert!(longest <= 12.8, "{f:?}: {longest}");
        let b = back.stats().bounds.unwrap();
        assert!((b[2] - b[0] - 351.0).abs() <= 0.1, "{f:?} {b:?}");
    }
}

#[test]
fn empty_plans_write_files_that_read() {
    let p = Plan { threads: vec![[1, 2, 3]], ..Plan::default() };
    for f in ALL {
        let back = read(f, &write(f, &p));
        assert_eq!(back.stats().stitches, 0, "{f:?}");
    }
}

#[test]
fn not_pes_or_jef_is_an_error() {
    assert!(read_pes(b"%PDF-1.7").is_err());
    assert!(read_pes(b"#PES0001\xff\xff\xff\x00").is_err());
    assert!(read_jef(b"short").is_err());
    assert!(!is_jef(b"#PES0060 and more bytes than a header needs, but not a JEF header at all......"));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Any bytes: an error or a plan of finite points, never a panic.
    #[test]
    fn readers_survive_anything(bytes in proptest::collection::vec(any::<u8>(), 0..2000)) {
        for p in [read_pes(&bytes).ok(), read_jef(&bytes).ok(), Some(read_exp(&bytes))].into_iter().flatten() {
            prop_assert!(p.needles.iter().all(|n| n.x.is_finite() && n.y.is_finite()));
            let _ = to_document(&p, "fuzz");
        }
    }

    /// Real files with bytes changed or cut short: never a panic.
    #[test]
    fn readers_survive_damaged_files(cut in 0usize..6000, edits in proptest::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 0..24)) {
        let p = plan(&busy_doc());
        for f in ALL {
            let mut b = write(f, &p);
            for (i, v) in &edits {
                let at = i.index(b.len());
                b[at] = *v;
            }
            b.truncate(cut.max(1));
            let back = match f {
                Fmt::Pes => read_pes(&b).ok(),
                Fmt::Jef => read_jef(&b).ok(),
                Fmt::Exp => Some(read_exp(&b)),
            };
            if let Some(back) = back {
                prop_assert!(back.needles.iter().all(|n| n.x.is_finite() && n.y.is_finite()));
            }
        }
    }

    /// Random designs come back with their stitches where they were.
    #[test]
    fn random_designs_round_trip(pts in proptest::collection::vec((-150.0f64..150.0, -150.0f64..150.0, 0u8..10), 2..60)) {
        let mut needles = vec![];
        let mut threads = vec![[0u8, 0, 0]];
        for (x, y, k) in pts {
            let cmd = match k {
                0 => Cmd::Jump,
                1 => Cmd::Trim,
                2 => {
                    threads.push([k, (x.abs() as u8), 9]);
                    Cmd::ColorChange
                }
                _ => Cmd::Stitch,
            };
            needles.push(Needle { x, y, cmd });
        }
        let p = Plan { threads, needles, ..Plan::default() };
        for f in ALL {
            let back = read(f, &write(f, &p));
            prop_assert_eq!(back.stats().color_changes, p.stats().color_changes);
            if p.stats().stitches > 0 {
                same_stitches(&p, &back, &format!("{f:?}"));
            }
        }
    }
}

/// `EMB_DUMP=folder cargo test -p vectorcraft-embroidery dump_machine_formats -- --ignored` writes
/// a design as DST, PES, JEF and EXP for checking in other embroidery software.
#[test]
#[ignore]
fn dump_machine_formats() {
    let Some(dir) = std::env::var_os("EMB_DUMP") else { return };
    let dir = std::path::PathBuf::from(dir);
    let p = plan(&busy_doc());
    std::fs::write(dir.join("busy.dst"), write_dst(&p, "busy")).unwrap();
    for f in ALL {
        std::fs::write(dir.join(format!("busy.{}", format!("{f:?}").to_lowercase())), write(f, &p)).unwrap();
    }
    let s = p.stats();
    std::fs::write(
        dir.join("busy.txt"),
        format!(
            "stitches {} jumps {} trims {} colors {} bounds {:?} threads {:?}\n",
            s.stitches, s.jumps, s.trims, s.color_changes, s.bounds, p.threads
        ),
    )
    .unwrap();
}
