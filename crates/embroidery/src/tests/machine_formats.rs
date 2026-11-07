//! PES, JEF, EXP, VP3 and U01: round trips, headers, and readers that survive any bytes.

use super::*;

const DATE: [i64; 6] = [2026, 10, 22, 9, 30, 5];

#[derive(Clone, Copy, Debug)]
enum Fmt {
    Pes,
    Jef,
    Exp,
    Vp3,
    U01,
}

const ALL: [Fmt; 5] = [Fmt::Pes, Fmt::Jef, Fmt::Exp, Fmt::Vp3, Fmt::U01];

fn write(f: Fmt, p: &Plan) -> Vec<u8> {
    match f {
        Fmt::Pes => write_pes(p, "test design").0,
        Fmt::Jef => write_jef(p, DATE),
        Fmt::Exp => write_exp(p),
        Fmt::Vp3 => write_vp3(p).0,
        Fmt::U01 => write_u01(p).0,
    }
}

fn read(f: Fmt, b: &[u8]) -> Plan {
    match f {
        Fmt::Pes => read_pes(b).unwrap(),
        Fmt::Jef => read_jef(b).unwrap(),
        Fmt::Exp => read_exp(b),
        Fmt::Vp3 => read_vp3(b).unwrap(),
        Fmt::U01 => read_u01(b).unwrap(),
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
        // PES, JEF and U01 have trim codes; EXP's trims come back as the jumps that make them,
        // and VP3 has none (the machine trims at long jumps).
        if !matches!(f, Fmt::Exp | Fmt::Vp3) {
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
    assert_eq!(read(Fmt::Vp3, &write(Fmt::Vp3, &p)).threads, p.threads);
    for f in [Fmt::Jef, Fmt::U01] {
        let t = read(f, &write(f, &p)).threads;
        assert_eq!(t.len(), 3, "{f:?}");
        assert!(t[0] == t[2] && t[0] != t[1], "{f:?}: {t:?}");
    }
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

#[test]
fn vp3_sections_add_up() {
    let p = plan(&busy_doc());
    let (b, warnings) = write_vp3(&p);
    assert!(is_vp3(&b) && b.starts_with(b"%vsm%\0"));
    // The busy design trims: VP3 can't say so, and the writer says that.
    assert_eq!(warnings.len(), usize::from(p.stats().trims > 0), "{warnings:?}");
    let be32 = |at: usize| u32::from_be_bytes(b[at..at + 4].try_into().unwrap()) as usize;
    let be16 = |at: usize| u16::from_be_bytes(b[at..at + 2].try_into().unwrap()) as usize;
    // The production string, then the file section counting every byte after it.
    let at = 6 + 2 + be16(6);
    assert_eq!(&b[at..at + 3], &[0, 2, 0]);
    assert_eq!(be32(at + 3), b.len() - (at + 7));
    // Its colour blocks, as many as the plan's (after the notes, the extents and a word).
    let blocks = b[at + 7 + 2 + 16 + 4 + 1] as usize;
    assert_eq!(blocks, p.threads.len());
    // The first block's thread is the plan's first colour.
    let first = b.windows(3).position(|w| w == [0, 5, 0]).unwrap();
    assert_eq!(&b[first + 15..first + 20], &[1, 0, p.threads[0][0], p.threads[0][1], p.threads[0][2]]);
    assert!(b.windows(2).any(|w| w == [0x80, 0x03]));
}

#[test]
fn u01_header_and_records() {
    let p = plan(&busy_doc());
    let (b, warnings) = write_u01(&p);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(is_u01(&b) && b[..128].iter().all(|&c| c == b'0'));
    let n = u32::from_le_bytes(b[0x8C..0x90].try_into().unwrap()) as usize;
    assert_eq!(b.len(), 0x100 + 3 * n);
    // Starts naming needle 1, ends with the end code; every control byte has its top bit set.
    assert_eq!(b[0x100], 0x80 | 9);
    assert_eq!(b[b.len() - 3], 0x80 | 18);
    assert!(b[0x100..].chunks(3).all(|r| r[0] & 0x80 != 0));
    // The extents: right minus left is the design's width.
    let s16 = |at: usize| i16::from_le_bytes(b[at..at + 2].try_into().unwrap()) as f64 / 10.0;
    let s = p.stats().bounds.unwrap();
    assert!((s16(0x84) - s16(0x80) - (s[2] - s[0])).abs() <= 0.11);
    assert!((s16(0x86) - s16(0x82) - (s[3] - s[1])).abs() <= 0.11);
}

#[test]
fn u01_names_at_most_nine_needles() {
    let threads: Vec<[u8; 3]> = (0..12u8).map(|i| [i * 20, 0, 0]).collect();
    let mut needles = vec![];
    for i in 0..12 {
        needles.push(Needle { x: f64::from(i), y: 0.0, cmd: Cmd::Stitch });
        needles.push(Needle { x: f64::from(i) + 0.5, y: 0.0, cmd: Cmd::Stitch });
        if i < 11 {
            needles.push(Needle { x: f64::from(i) + 0.5, y: 0.0, cmd: Cmd::ColorChange });
        }
    }
    let p = Plan { threads, needles, ..Plan::default() };
    let (b, warnings) = write_u01(&p);
    assert_eq!(warnings.len(), 1);
    assert!(b[0x100..].chunks(3).all(|r| (r[0] & 0x1F) <= 18));
    let back = read_u01(&b).unwrap();
    assert_eq!(back.stats().color_changes, 11);
}

#[test]
fn not_vp3_or_u01_is_an_error() {
    assert!(read_vp3(b"%PDF-1.7").is_err());
    assert!(read_vp3(b"%vsm%\0\0").is_err());
    assert!(read_u01(b"short").is_err());
    assert!(!is_u01(&[b'0'; 100]));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Any bytes: an error or a plan of finite points, never a panic.
    #[test]
    fn readers_survive_anything(bytes in proptest::collection::vec(any::<u8>(), 0..2000)) {
        for p in [read_pes(&bytes).ok(), read_jef(&bytes).ok(), Some(read_exp(&bytes)), read_vp3(&bytes).ok(), read_u01(&bytes).ok()].into_iter().flatten() {
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
                Fmt::Vp3 => read_vp3(&b).ok(),
                Fmt::U01 => read_u01(&b).ok(),
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

/// `EMB_ORACLE=folder cargo test -p vectorcraft-embroidery read_oracle_files -- --ignored` reads
/// each `.vp3`, `.u01`, `.pes`, `.jef`, `.exp` and `.dst` file in the folder (made by other
/// embroidery software, to check the readers against) and writes `name.ext.txt` beside it: the
/// stitch count, colour changes, trims, bounds and threads read.
#[test]
#[ignore]
fn read_oracle_files() {
    let Some(dir) = std::env::var_os("EMB_ORACLE") else { return };
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let path = e.path();
        let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
        let b = std::fs::read(&path).unwrap();
        let p = match ext.as_str() {
            "vp3" => read_vp3(&b).unwrap(),
            "u01" => read_u01(&b).unwrap(),
            "pes" => read_pes(&b).unwrap(),
            "jef" => read_jef(&b).unwrap(),
            "exp" => read_exp(&b),
            "dst" => read_dst(&b).unwrap(),
            _ => continue,
        };
        let s = p.stats();
        let first: Vec<(f64, f64)> = p.needles.iter().filter(|n| n.cmd == Cmd::Stitch).take(5).map(|n| (n.x, n.y)).collect();
        std::fs::write(
            path.with_extension(format!("{ext}.txt")),
            format!(
                "stitches {} jumps {} trims {} colors {} bounds {:?} threads {:?} first {:?} warnings {:?}\n",
                s.stitches, s.jumps, s.trims, s.color_changes, s.bounds, p.threads, first, p.warnings
            ),
        )
        .unwrap();
    }
}

/// `EMB_DUMP=folder cargo test -p vectorcraft-embroidery dump_machine_formats -- --ignored` writes
/// a design as DST, PES, JEF, EXP, VP3 and U01 for checking in other embroidery software.
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
