// SPDX-License-Identifier: AGPL-3.0-only
//! machine_exec_rules: the cut, the idle clock, when a tree is worth reading, the guest's awk programs and the line
//! cutter. Spec: spec/machine_exec_rules.md. Every expected value was printed by the TS under node (2026-10-25) or
//! is the TS test's own.
mod support;

use std::process::Command;

use wsp_adapters::port::ExecFailureKind;
use wsp_core::machine_exec::{reads_work, turn_activity, turn_cut, LineCut, TurnLimits, LEFTOVERS_AWK, TREE_WORK_AWK};

const WALL_WORDS: &str = "send to continue where it stopped, or change the limit in Settings > Computers";

#[test]
fn turn_cut_picks_the_wall_before_the_idle_and_says_each_limit_it_read() {
    let limits = TurnLimits { idle_ms: 1000.0, deadline_ms: 2000.0 };
    let wall = turn_cut(limits, 2000, 5000).expect("the wall cut");
    assert_eq!(wall.message, format!("stopped after 0m 02s at the 0m turn limit; {WALL_WORDS}"));
    assert_eq!(wall.kind, ExecFailureKind::Other);
    let idle = turn_cut(limits, 1999, 1000).expect("the idle cut");
    assert_eq!(idle.message, "stopped after 0m 02s with no output for 0m");
    assert_eq!(idle.kind, ExecFailureKind::Other);
    assert!(turn_cut(limits, 1999, 999).is_none());
    // The TS tests' own lines, each from the limit that rule read.
    assert_eq!(turn_cut(TurnLimits { idle_ms: 600_000.0, deadline_ms: 21_600_000.0 }, 900_000, 600_000).unwrap().message, "stopped after 15m 00s with no output for 10m");
    assert_eq!(
        turn_cut(TurnLimits { idle_ms: 600_000.0, deadline_ms: 7_200_000.0 }, 7_200_000, 0).unwrap().message,
        format!("stopped after 2h 00m 00s at the 2h turn limit; {WALL_WORDS}")
    );
}

#[test]
fn a_cut_never_comes_from_an_infinite_limit() {
    let none = TurnLimits { idle_ms: f64::INFINITY, deadline_ms: f64::INFINITY };
    assert!(turn_cut(none, 1_000_000_000_000, 1_000_000_000_000).is_none());
    let idle_only = TurnLimits { idle_ms: 100.0, deadline_ms: f64::INFINITY };
    let idle = turn_cut(idle_only, 1_000_000_000_000, 130).expect("the idle cut");
    assert!(idle.message.starts_with("stopped after ") && idle.message.ends_with(" with no output for 0m"), "{}", idle.message);
    assert!(turn_cut(idle_only, 1_000_000_000_000, 99).is_none());
}

#[test]
fn the_idle_clock_moves_on_work_past_the_floor_and_rebases_on_a_fall() {
    let mut a = turn_activity(0);
    a.read(100, 1000);
    assert_eq!(a.quiet_ms(1000), 1000, "the first reading is only a base");
    a.read(104, 2000);
    assert_eq!(a.quiet_ms(2000), 2000, "4 ticks in a second is under the floor of 5");
    a.read(106, 2000);
    assert_eq!(a.quiet_ms(2000), 0, "6 ticks over the base read at 1000 clears it");
    a.read(50, 3000);
    assert_eq!(a.quiet_ms(3000), 1000, "a reading below the base is a new base");
    a.read(51, 3000);
    assert_eq!(a.quiet_ms(3000), 0, "with no time passed the floor is 1 tick");
    a.touch(500);
    assert_eq!(a.quiet_ms(3000), 2500, "touch sets the clock even when it is earlier");
    a.hold(250);
    assert_eq!(a.quiet_ms(3000), 2250);
    let mut fresh = turn_activity(7_000);
    assert_eq!(fresh.quiet_ms(7_000), 0);
    fresh.read(10, 7_500);
    fresh.read(10, 9_000);
    assert_eq!(fresh.quiet_ms(9_000), 2_000, "no work moved, nothing moves");
}

#[test]
fn reads_work_only_with_a_finite_limit_and_half_of_it_quiet() {
    assert!(!reads_work(f64::INFINITY, 1_000_000_000_000));
    assert!(!reads_work(1000.0, 499));
    assert!(reads_work(1000.0, 500));
    assert!(!reads_work(f64::NAN, 1));
}

#[test]
fn the_awk_programs_are_the_ts_strings() {
    let ts: serde_json::Value = serde_json::from_str(include_str!("fixtures/machine_exec_shell.json")).unwrap();
    assert_eq!(TREE_WORK_AWK, ts["TREE_WORK_AWK"].as_str().unwrap());
    assert_eq!(LEFTOVERS_AWK, ts["LEFTOVERS_AWK"].as_str().unwrap());
}

/// Every awk a guest may carry that this computer has, each as the words that run it.
fn awks_here() -> Vec<Vec<&'static str>> {
    [vec!["awk"], vec!["gawk"], vec!["mawk"], vec!["busybox", "awk"]]
        .into_iter()
        .filter(|w| Command::new("sh").args(["-c", &format!("command -v {}", w[0])]).status().map(|s| s.success()).unwrap_or(false))
        .collect()
}

/// A fake /proc entry, given what the reading looks at in it.
fn fake_stat(dir: &std::path::Path, pid: u32, ppid: u32, pgrp: u32, utime: u64, cutime: u64) -> String {
    std::fs::create_dir_all(dir.join(pid.to_string())).unwrap();
    let stat = dir.join(pid.to_string()).join("stat");
    std::fs::write(&stat, format!("{pid} (rustc) S {ppid} {pgrp} 0 0 -1 0 0 0 0 0 {utime} 0 {cutime} 0 20 0 1 0 0\n")).unwrap();
    stat.to_string_lossy().into_owned()
}

fn read_with(awk: &[&str], root: u32, files: &[String]) -> String {
    let mut args: Vec<String> = vec!["-c".into(), "\"$@\" 2>/dev/null; true".into(), "sh".into()];
    args.extend(awk.iter().map(|s| s.to_string()));
    args.extend(["-v".into(), format!("p={root}"), TREE_WORK_AWK.into()]);
    args.extend(files.iter().cloned());
    String::from_utf8_lossy(&Command::new("sh").args(&args).output().unwrap().stdout).trim().to_string()
}

#[test]
fn a_process_that_exits_between_the_shell_s_glob_and_the_read_leaves_the_run_s_o() {
    let dir = tempfile::tempdir().unwrap();
    let files = vec![fake_stat(dir.path(), 21, 1, 21, 60, 0), dir.path().join("22").join("stat").to_string_lossy().into_owned(), fake_stat(dir.path(), 23, 21, 21, 740, 0)];
    let awks = awks_here();
    assert!(!awks.is_empty());
    for awk in awks {
        assert_eq!((awk.clone(), read_with(&awk, 21, &files)), (awk, (60 + 740).to_string()));
    }
}

#[test]
fn the_run_s_whole_tree_counts_a_tool_command_in_a_session_of_its_own_and_an_orp() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let files = vec![
        fake_stat(d, 30, 1, 30, 10, 0),
        fake_stat(d, 31, 30, 30, 20, 100),
        fake_stat(d, 32, 31, 32, 40, 0),
        fake_stat(d, 33, 32, 32, 300, 0),
        fake_stat(d, 34, 1, 30, 5, 0),
        fake_stat(d, 35, 1, 35, 9_000, 0),
        fake_stat(d, 36, 35, 36, 7_000, 0),
    ];
    for awk in awks_here() {
        assert_eq!((awk.clone(), read_with(&awk, 30, &files)), (awk, (10 + 20 + 100 + 40 + 300 + 5).to_string()));
    }
}

#[test]
fn the_tree_program_over_a_fake_proc_counts_ticks_and_whole_mib_of_io() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let proc_ = |pid: u32, comm: &str, pgrp: u32, utime: u64, stime: u64, io: Option<(u64, u64)>| -> String {
        std::fs::create_dir_all(d.join(pid.to_string())).unwrap();
        std::fs::write(d.join(pid.to_string()).join("stat"), format!("{pid} ({comm}) S 1 {pgrp} 0 0 -1 0 0 0 0 0 {utime} {stime} 0 0 20 0 1 0 0\n")).unwrap();
        if let Some((r, w)) = io {
            std::fs::write(d.join(pid.to_string()).join("io"), format!("rchar: {r}\nwchar: {w}\nread_bytes: 4096\n")).unwrap();
        }
        d.join(pid.to_string()).join("stat").to_string_lossy().into_owned()
    };
    let files = vec![proc_(11, "bash", 11, 30, 12, Some((2 * 1_048_576, 1_048_576))), proc_(12, "node (2)", 11, 400, 100, None), proc_(13, "daemon", 13, 9_000, 9_000, Some((99 * 1_048_576, 0)))];
    let read = |p: u32| -> u64 {
        let mut args = vec!["-v".to_string(), format!("p={p}"), TREE_WORK_AWK.to_string()];
        args.extend(files.iter().cloned());
        String::from_utf8_lossy(&Command::new("awk").args(&args).output().unwrap().stdout).trim().parse().unwrap()
    };
    assert_eq!(read(11), 30 + 12 + 400 + 100 + 3);
    assert_eq!(read(13), 9_000 + 9_000 + 99);
    assert_eq!(read(99), 0);
}

fn read_live(group: u32) -> u64 {
    let out = Command::new("sh").args(["-c", "awk -v p=\"$1\" \"$2\" /proc/[0-9]*/stat 2>/dev/null", "sh", &group.to_string(), TREE_WORK_AWK]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0)
}

own_group_test!(a_child_in_a_session_of_its_own_counts_while_it_burns_cpu_as_an_agent_s_tool, 20_000, {
    if !std::path::Path::new("/proc/1/stat").exists() {
        return;
    }
    use std::io::Read;
    use std::os::unix::process::CommandExt;
    let mut group = Command::new("sh")
        .args(["-c", "setsid sh -c 'while :; do :; done' & echo $!; exec sleep 30"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    let mut first = [0u8; 32];
    let n = group.stdout.as_mut().unwrap().read(&mut first).unwrap();
    let spinner: i32 = String::from_utf8_lossy(&first[..n]).trim().parse().unwrap();
    let before = read_live(group.id());
    support::sleep_ms(600).await;
    let after = read_live(group.id());
    support::strays::kill_group(spinner);
    support::strays::kill_group(group.id() as i32);
    assert!(after - before >= 15, "read {before} then {after}");
});

own_group_test!(a_child_that_burned_cpu_and_was_reaped_stays_in_its_run_s_reading_so_a_build, 20_000, {
    if !std::path::Path::new("/proc/1/stat").exists() {
        return;
    }
    use std::os::unix::process::CommandExt;
    let group = Command::new("sh").args(["-c", "timeout 0.5 sh -c 'while :; do :; done'; exec sleep 30"]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).process_group(0).spawn().unwrap();
    support::sleep_ms(900).await;
    let read = read_live(group.id());
    support::strays::kill_group(group.id() as i32);
    assert!(read >= 20, "read {read}");
});

fn cut(chunks: &[&[u8]]) -> (Vec<String>, Option<String>) {
    let mut c = LineCut::new();
    let mut lines = Vec::new();
    for chunk in chunks {
        lines.extend(c.take(chunk));
    }
    (lines, c.rest())
}

#[test]
fn line_cut_decodes_lossily_as_node_does() {
    let bytes: &[u8] = &[0x61, 0xff, 0xfe, 0x62, 0x0a, 0xe2, 0x82, 0x0a, 0xf0, 0x9f, 0x98, 0x0a, 0xed, 0xa0, 0x80, 0x0a, 0xc0, 0xaf, 0x0a];
    let r = '\u{fffd}';
    assert_eq!(cut(&[bytes]), (vec![format!("a{r}{r}b"), format!("{r}"), format!("{r}"), format!("{r}{r}{r}"), format!("{r}{r}")], None));
}

#[test]
fn line_cut_keeps_cr_and_empty_lines() {
    assert_eq!(cut(&[b"a\r\n\nb\r"]), (vec!["a\r".to_string(), String::new()], Some("b\r".to_string())));
    assert_eq!(cut(&[b""]), (vec![], None));
    let mut c = LineCut::new();
    assert!(c.take(b"part").is_empty());
    assert!(c.take(b"").is_empty(), "an empty chunk keeps the pending tail");
    assert_eq!(c.take(b"ial\n"), vec!["partial".to_string()]);
    assert_eq!(c.rest(), None);
}

#[test]
fn line_cut_keeps_a_character_split_across_chunks() {
    let line = "café ☕ done\n".as_bytes();
    assert_eq!(cut(&[&line[..4], &line[4..]]), (vec!["café ☕ done".to_string()], None));
    let mut c = LineCut::new();
    let mut x = vec![b'x'; 262_143];
    x.push(0xc3);
    assert!(c.take(&x).is_empty());
    assert_eq!(c.take(&[0xa9, b'\n']), vec![format!("{}é", "x".repeat(262_143))]);
}

#[test]
fn line_cut_cuts_only_a_line_strictly_longer_than_16_mib_at_a_character_start() {
    const MAX: usize = 16 * 1024 * 1024;
    // Exactly the cap and its newline: one line.
    let mut exact = vec![b'a'; MAX];
    exact.push(b'\n');
    let (lines, rest) = cut(&[&exact]);
    assert_eq!((lines.len(), lines[0].len(), rest), (1, MAX, None));
    // One byte past the cap, with a three-byte character across the cut: the piece ends before the character.
    let mut over = vec![b'a'; MAX - 1];
    over.extend_from_slice("€".as_bytes());
    over.extend_from_slice(b"z\n");
    let (lines, rest) = cut(&[&over]);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].len(), MAX - 1);
    assert_eq!(lines[1], "€z");
    assert_eq!(rest, None);
    // A tail with no newline past the cap is handed on in pieces as it grows, the rest waiting.
    let mut c = LineCut::new();
    let first = c.take(&vec![b'b'; MAX + 10]);
    assert_eq!(first.iter().map(|l| l.len()).collect::<Vec<_>>(), vec![MAX]);
    assert_eq!(c.rest(), Some("b".repeat(10)));
}
