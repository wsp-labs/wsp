// SPDX-License-Identifier: AGPL-3.0-only
//! The local road's rules the TS tests do not pin, each read off packages/runtime/src/local-exec.ts at 65f6b89c7 and
//! the host spec's rule 6.34 table. Specs: spec/local_exec_reap.md, spec/local_exec_stream.md. Every case that starts
//! a process runs in a process group of its own.
mod support;

use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use futures::StreamExt;
use indexmap::IndexSet;
use wsp_adapters::port::{Attached, AttachRunOptions, ExecOptions as Launch, ExecStream, ExecStreamFactory};
use wsp_core::local_exec::reap::{end_marker, leftovers, orphans_of, parse_etime, pump_of, reap_run, release_held, release_late, GroupRow, Lines, TreeTally};
use wsp_core::local_exec::{agent_pid_of, local_exec_stream, LocalExecOptions};

fn tmp() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("wsp-localexec-pinned-").tempdir().unwrap()
}

fn write(path: &str, text: &str) {
    std::fs::write(path, text).unwrap();
}

fn exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}

#[test]
fn the_tree_s_reading_never_falls_when_a_process_is_read_lower() {
    let mut t = TreeTally::default();
    // The TS test's own rows: 999 leads, 1000 is in its group, 500 and 600 name each other as parents.
    let rows = "999 1 999 0:00.50\n1000 999 999 0:01.00\n500 600 500 0:01.00\n600 500 600 0:01.00\n";
    assert_eq!(t.add(rows, 999), 150);
    // 1000 burned 50 more; 999 is read lower, a new process on that number, so its whole reading counts.
    assert_eq!(t.add("999 1 999 0:00.20\n1000 999 999 0:01.50\n", 999), 150 + 20 + 50);
    // 1000 exited: the total stays.
    assert_eq!(t.add("999 1 999 0:00.20\n", 999), 220);
    // A child in a session of its own, under the leader by its parent chain, counts; a stranger does not.
    assert_eq!(t.add("999 1 999 0:00.20\n2000 999 2000 0:00.30\n3000 1 3000 9:00.00\n", 999), 250);
    // Blank lines, a short row and a time ps would not print count nothing and break nothing.
    assert_eq!(t.add("\n   \n4000\n999 1 999 nonsense\n", 999), 250);
}

#[test]
fn leftovers_are_orphans_less_the_pump_and_everything_under_them() {
    let row = |pid, ppid| GroupRow { pid, ppid };
    // 10 leads; 11 and 12 are its children (the agent and a server); 20 is an orphan the turn left, 21 under it; 30
    // and 31 are the pump's two processes, orphans too.
    let rows = vec![row(10, 1), row(11, 10), row(12, 11), row(20, 1), row(21, 20), row(22, 21), row(30, 1), row(31, 1)];
    assert_eq!(orphans_of(&rows, 10), vec![20, 30, 31]);
    let pump: IndexSet<i64> = [30, 31].into_iter().collect();
    let mut left = leftovers(&rows, 10, &pump);
    left.sort();
    assert_eq!(left, vec![20, 21, 22]);
    assert!(leftovers(&rows, 10, &[20, 30, 31].into_iter().collect()).is_empty());
}

#[test]
fn the_pump_s_pids_are_the_positive_safe_integers_of_its_file() {
    let dir = tmp();
    let base = dir.path().join("aaaaaaaaaaaa").to_string_lossy().into_owned();
    assert!(pump_of(&base).is_empty());
    write(&format!("{base}.pump"), "123\n\n-4\nabc\n5.5\n0\n456\n");
    assert_eq!(pump_of(&base).into_iter().collect::<Vec<_>>(), vec![123, 456]);
}

#[test]
fn started_at_reads_the_elapsed_forms_ps_prints() {
    let now = 1_000_000_000;
    assert_eq!(parse_etime("01:02\n", now), Some(now - 62_000));
    assert_eq!(parse_etime("  02:00:01", now), Some(now - 7_201_000));
    assert_eq!(parse_etime("1-00:00:01", now), Some(now - 86_401_000));
    assert_eq!(parse_etime("5", now), None);
    assert_eq!(parse_etime("", now), None);
    assert_eq!(parse_etime("ab:cd", now), None);
}

#[test]
fn the_end_marker_is_made_from_the_run_s_own_name() {
    assert_eq!(end_marker("/x/runs/0123456789ab"), "__WSP_EOF_0123456789ab__");
}

#[test]
fn release_held_appends_only_into_an_empty_channel_and_drops_the_seed() {
    let dir = tmp();
    let base = |id: &str| dir.path().join(id).to_string_lossy().into_owned();
    // Claimed, the channel empty: the seed goes in once and the held file goes.
    let a = base("aaaaaaaaaaaa");
    std::fs::create_dir(format!("{a}.d")).unwrap();
    write(&format!("{a}.in"), "");
    write(&format!("{a}.held"), "hello\n");
    release_held(&a);
    assert_eq!(support::read(&format!("{a}.in")), "hello\n");
    assert!(!exists(&format!("{a}.held")));
    release_held(&a);
    assert_eq!(support::read(&format!("{a}.in")), "hello\n", "no second copy");
    // A channel that already took a line: the seed is dropped, not appended.
    let b = base("bbbbbbbbbbbb");
    std::fs::create_dir(format!("{b}.d")).unwrap();
    write(&format!("{b}.in"), "hello\n");
    write(&format!("{b}.held"), "hello\n");
    release_held(&b);
    assert_eq!(support::read(&format!("{b}.in")), "hello\n");
    assert!(!exists(&format!("{b}.held")));
    // A run whose claim is gone takes nothing, and the seed still goes.
    let c = base("cccccccccccc");
    write(&format!("{c}.in"), "");
    write(&format!("{c}.held"), "hello\n");
    release_held(&c);
    assert_eq!(support::read(&format!("{c}.in")), "");
    assert!(!exists(&format!("{c}.held")));
}

#[test]
fn release_late_goes_in_once_by_its_key() {
    let dir = tmp();
    let a = dir.path().join("aaaaaaaaaaaa").to_string_lossy().into_owned();
    std::fs::create_dir(format!("{a}.d")).unwrap();
    write(&format!("{a}.in"), "seed\n");
    write(&format!("{a}.late.key"), "k1");
    write(&format!("{a}.late"), "yes\n");
    release_late(&a);
    assert_eq!(support::read(&format!("{a}.in")), "seed\nyes\n");
    assert_eq!(support::read(&format!("{a}.late.went")), "k1");
    assert_eq!(support::mode_of(&format!("{a}.late.went")), "600");
    assert!(!exists(&format!("{a}.late")) && !exists(&format!("{a}.late.key")));
    // The same words under a new key go in again; the same key goes in no second time.
    write(&format!("{a}.late.key"), "k2");
    write(&format!("{a}.late"), "yes\n");
    release_late(&a);
    write(&format!("{a}.late.key"), "k2");
    write(&format!("{a}.late"), "yes\n");
    release_late(&a);
    assert_eq!(support::read(&format!("{a}.in")), "seed\nyes\nyes\n");
    // A run that ended takes no late line, and the line still goes.
    write(&format!("{a}.exit"), "0\n");
    write(&format!("{a}.late.key"), "k3");
    write(&format!("{a}.late"), "after\n");
    release_late(&a);
    assert_eq!(support::read(&format!("{a}.in")), "seed\nyes\nyes\n");
    assert!(!exists(&format!("{a}.late")));
    // A late line with no key beside a run that never released one: no key equals no went, so it does not go in.
    let b = dir.path().join("bbbbbbbbbbbb").to_string_lossy().into_owned();
    std::fs::create_dir(format!("{b}.d")).unwrap();
    write(&format!("{b}.in"), "seed\n");
    write(&format!("{b}.late"), "keyless\n");
    release_late(&b);
    assert_eq!(support::read(&format!("{b}.in")), "seed\n");
    assert!(!exists(&format!("{b}.late")) && !exists(&format!("{b}.late.went")));
}

#[tokio::test(flavor = "multi_thread")]
async fn lines_hands_every_queued_line_before_the_error_and_drops_feeds_after_let_go() {
    let lines = Lines::new();
    lines.feed(b"one\ntw");
    lines.feed(b"o\nthree");
    lines.fail(wsp_adapters::port::ExecFailure { kind: wsp_adapters::port::ExecFailureKind::Other, message: "cut".into() });
    let got: Vec<_> = lines.iterate().collect().await;
    assert_eq!(got.len(), 4);
    assert_eq!(got[..3].iter().map(|r| r.as_ref().unwrap().clone()).collect::<Vec<_>>(), vec!["one", "two", "three"]);
    assert_eq!(got[3].as_ref().unwrap_err().message, "cut");
    // A reader dropped after its first line lets go: what is fed after is dropped, and the end hands nothing more.
    let held = Lines::new();
    held.feed(b"first\n");
    let mut reader = held.iterate();
    assert_eq!(reader.next().await.unwrap().unwrap(), "first");
    drop(reader);
    held.feed(b"second\nthird");
    held.end();
    let after: Vec<_> = held.iterate().collect().await;
    assert!(after.is_empty(), "{after:?}");
    // Lines fed before anyone iterates wait in the queue; a tail with no newline comes at the end.
    let early = Lines::new();
    early.feed(b"a\n\nb\r\nc");
    early.end();
    let got: Vec<String> = early.iterate().map(|r| r.unwrap()).collect().await;
    assert_eq!(got, vec!["a", "", "b\r", "c"]);
}

own_group_test!(a_reap_with_no_readable_pid_signals_nothing_and_deletes_the_files, 20_000, {
    let dir = tmp();
    let runs = dir.path().join("runs");
    std::fs::create_dir(&runs).unwrap();
    // A group of this test's own, standing for the run's; its pid written with a word after it, which Number() reads
    // as NaN.
    let mut sleep = std::process::Command::new("sleep");
    sleep.arg("300");
    let pid = support::strays::spawn_reaped(sleep);
    for (id, pid_text) in [("aaaaaaaaaaaa", format!("{pid}x\n")), ("bbbbbbbbbbbb", "0\n".to_string()), ("cccccccccccc", "-5\n".to_string()), ("dddddddddddd", String::new())] {
        let base = runs.join(id).to_string_lossy().into_owned();
        std::fs::create_dir(format!("{base}.d")).unwrap();
        for suffix in ["sh", "log", "agent", "exit", "in", "held", "late", "fifo", "tail", "pump", "late.key", "late.went"] {
            write(&format!("{base}.{suffix}"), "x");
        }
        if !pid_text.is_empty() {
            write(&format!("{base}.pid"), &pid_text);
        }
        let seen = Arc::new(std::sync::Mutex::new(None));
        let s2 = seen.clone();
        reap_run(&base, Some(Box::new(move |end| Box::pin(async move { *s2.lock().unwrap() = Some(end) }))), 2_000).await;
        assert_eq!(*seen.lock().unwrap(), Some(1), "last_read gets the log's size");
        assert!(!exists(&format!("{base}.d")));
        // The two files the reap never takes stay behind.
        let mut left = support::names(runs.to_str().unwrap());
        left.retain(|n| n.starts_with(id));
        assert_eq!(left, vec![format!("{id}.late.key"), format!("{id}.late.went")]);
    }
    assert!(support::strays::alive(pid), "nothing signalled the group");
    support::strays::kill_group(pid);
});

/// A run as a launch leaves it, made by hand: the claim, a pid, a log and the files `extra` names.
fn hand_made(runs: &str, id: &str, pid: Option<i32>, log: &str, extra: &[(&str, &str)]) -> String {
    let base = format!("{runs}/{id}");
    std::fs::create_dir_all(runs).unwrap();
    std::fs::create_dir(format!("{base}.d")).unwrap();
    if let Some(p) = pid {
        write(&format!("{base}.pid"), &format!("{p}\n"));
    }
    write(&format!("{base}.log"), log);
    for (suffix, text) in extra {
        write(&format!("{base}.{suffix}"), text);
    }
    base
}

fn sleeper() -> i32 {
    let mut sleep = std::process::Command::new("sleep");
    sleep.arg("300");
    support::strays::spawn_reaped(sleep)
}

async fn attach(runs: &str, run: &str, input: bool, from: Option<u64>) -> Attached {
    let factory = local_exec_stream(LocalExecOptions { poll_ms: Some(10), ..LocalExecOptions::new(runs, runs) }, None);
    factory.attach(run, AttachRunOptions { input, started_at: 0.0, from }).unwrap().await.unwrap()
}

fn stream(a: Attached) -> Arc<dyn ExecStream> {
    match a {
        Attached::Stream(s) => s,
        Attached::Gone => panic!("the run read as gone"),
    }
}

own_group_test!(broken_run_files_read_as_the_ts_reader_reads_them, 30_000, {
    let dir = tmp();
    let runs = dir.path().join("runs").to_string_lossy().into_owned();
    // An exit with words after the number reads its number; the last line with no newline is a line; a blank line is
    // "" and a carriage return stays.
    let base = hand_made(&runs, "aaaaaaaaaaaa", Some(sleeper()), "a\n\nb\r\nlast", &[("exit", "14abc")]);
    let s = stream(attach(&runs, &base, false, None).await);
    assert_eq!(support::lines_ok(&*s).await, vec!["a", "", "b\r", "last"]);
    assert_eq!(s.exited().await, Some(14));
    // An exit with no newline.
    let base = hand_made(&runs, "bbbbbbbbbbbb", Some(sleeper()), "", &[("exit", "3")]);
    let s = stream(attach(&runs, &base, false, None).await);
    assert!(support::lines_ok(&*s).await.is_empty());
    assert_eq!(s.exited().await, Some(3));
    // An exit that is no number settles with no code (decision 2: the TS NaN, which the wire writes as null).
    let base = hand_made(&runs, "cccccccccccc", Some(sleeper()), "", &[("exit", "abc")]);
    let s = stream(attach(&runs, &base, false, None).await);
    support::lines_ok(&*s).await;
    assert_eq!(s.exited().await, None);
    // No pid at all and no exit: the leader reads dead on two polls and the run settles with no code, its files gone.
    let base = hand_made(&runs, "dddddddddddd", None, "partial\n", &[]);
    let s = stream(attach(&runs, &base, false, None).await);
    assert_eq!(support::lines_ok(&*s).await, vec!["partial"]);
    assert_eq!(s.exited().await, None);
    assert!(!exists(&format!("{base}.d")) && !exists(&format!("{base}.log")));
    // A channel holding only the end marker is kept, with nothing handed over.
    let id = "eeeeeeeeeeee";
    let base = hand_made(&runs, id, Some(sleeper()), "", &[("in", &format!("__WSP_EOF_{id}__\n"))]);
    let s = stream(attach(&runs, &base, true, None).await);
    assert_eq!(s.taken(), Some(vec![]));
    s.kill();
    s.exited().await;
    // An empty channel with a held seed beside it: the seed goes in first and the run is kept.
    let base = hand_made(&runs, "ffffffffffff", Some(sleeper()), "", &[("in", ""), ("held", "hello\n")]);
    let s = stream(attach(&runs, &base, true, None).await);
    assert_eq!(s.taken(), Some(vec!["hello".to_string()]));
    assert_eq!(support::read(&format!("{base}.in")), "hello\n");
    s.kill();
    s.exited().await;
    // An empty channel and no held seed: the host died before the prompt was written; the run is reaped and gone.
    let held = sleeper();
    let base = hand_made(&runs, "0123456789ab", Some(held), "", &[("in", "")]);
    assert!(matches!(attach(&runs, &base, true, None).await, Attached::Gone));
    support::strays::gone(held).await;
    assert!(!exists(&format!("{base}.d")) && !exists(&format!("{base}.in")));
});

own_group_test!(a_stored_from_past_the_log_reads_nothing_until_the_log_grows, 20_000, {
    let dir = tmp();
    let runs = dir.path().join("runs").to_string_lossy().into_owned();
    let base = hand_made(&runs, "aaaaaaaaaaaa", Some(sleeper()), "0123456789", &[]);
    let s = stream(attach(&runs, &base, false, Some(15)).await);
    let mut lines = s.lines();
    assert!(support::within(300, lines.next()).await.is_none(), "nothing past byte 15 yet");
    std::fs::OpenOptions::new().append(true).open(format!("{base}.log")).unwrap().write_all_at_end(b"ABCDEFGHIJ\n");
    assert_eq!(lines.next().await.unwrap().unwrap(), "FGHIJ");
    write(&format!("{base}.exit"), "0\n");
    assert!(lines.next().await.is_none());
    assert_eq!(s.exited().await, Some(0));
});

trait AppendAll {
    fn write_all_at_end(&mut self, bytes: &[u8]);
}
impl AppendAll for std::fs::File {
    fn write_all_at_end(&mut self, bytes: &[u8]) {
        use std::io::Write;
        self.write_all(bytes).unwrap();
    }
}

own_group_test!(the_environment_is_the_turn_s_alone, 20_000, {
    // A variable of this process, which the turn must not see: the launch's environment replaces the host's.
    std::env::set_var("WSP_CORE_C_HOST_ONLY", "leaked");
    let dir = tmp();
    let root = dir.path().to_string_lossy().into_owned();
    let runs = dir.path().join("runs").to_string_lossy().into_owned();
    let factory = local_exec_stream(LocalExecOptions::new(&root, &runs), None);
    let env = [("GIVEN", "yes")].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let s = factory.launch("echo \"${WSP_CORE_C_HOST_ONLY-unset} ${HOME-nohome} $GIVEN ${WSP_AGENT_PID_FILE##*/}\"", Launch { env, ..Default::default() });
    let run = s.run().unwrap();
    let id = run.rsplit('/').next().unwrap().to_string();
    assert_eq!(support::lines_ok(&*s).await, vec![format!("unset nohome yes {id}.agent")]);
    assert_eq!(s.exited().await, Some(0));
    assert_eq!(support::mode_of(&runs), "700");
    let _ = std::fs::metadata(&runs).unwrap().permissions().mode();
});

own_group_test!(a_launch_refused_for_a_secret_starts_nothing_and_fails_its_lines, 10_000, {
    let dir = tmp();
    let root = dir.path().to_string_lossy().into_owned();
    let runs = dir.path().join("runs").to_string_lossy().into_owned();
    let factory = local_exec_stream(LocalExecOptions::new(&root, &runs), None);
    let secret = Some(wsp_adapters::port::ExecSecret { files: None, input: true });
    let refused = factory.try_launch("true", Launch { secret: secret.clone(), ..Default::default() }).err().expect("a refusal");
    assert_eq!(refused.message, "a launch on this computer takes no text holding a value; nothing was started");
    let s = factory.launch("true", Launch { secret, ..Default::default() });
    let (lines, failure) = support::lines_err(&*s).await;
    assert!(lines.is_empty());
    assert_eq!(failure.message, "a launch on this computer takes no text holding a value; nothing was started");
    assert_eq!(s.exited().await, None);
    assert!(!exists(&runs), "nothing was made");
});

own_group_test!(agent_pid_of_reads_a_live_positive_pid_and_nothing_else, 10_000, {
    let dir = tmp();
    let run = dir.path().join("aaaaaaaaaaaa").to_string_lossy().into_owned();
    assert_eq!(agent_pid_of(&run), None);
    let live = sleeper();
    write(&format!("{run}.agent"), &format!(" {live}\n"));
    assert_eq!(agent_pid_of(&run), Some(live));
    for text in ["0", "-1", "abc", "1.5", ""] {
        write(&format!("{run}.agent"), text);
        assert_eq!(agent_pid_of(&run), None, "{text:?}");
    }
    support::strays::kill_group(live);
    support::strays::gone(live).await;
    write(&format!("{run}.agent"), &live.to_string());
    assert_eq!(agent_pid_of(&run), None, "a dead pid");
});
