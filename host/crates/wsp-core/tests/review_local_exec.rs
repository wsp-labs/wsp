// Review probe for the local exec road: the helpers and the tests that FAIL on 11db4a4e6. Each asserts what the TS
// answered on the same input (packages/runtime/test/zz-review-local-exec.test.ts). Goes in host/crates/wsp-core/tests/.
#![allow(dead_code)]
mod support;

use std::os::unix::fs::symlink;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use futures::StreamExt;
use tokio::sync::oneshot;
use wsp_adapters::port::{
    AttachRunOptions, Attached, ExecFailure, ExecOptions as Launch, ExecStream, ExecStreamFactory, Settle, WriteOutcome,
};
use wsp_core::local_exec::{local_exec_stream, LocalExecFactory, LocalExecOptions};

use support::strays::{alive, record, spawn_reaped};
use support::{sleep_ms, within};

struct Case {
    _dir: tempfile::TempDir,
    root: String,
    run_dir: String,
    ending: Mutex<Vec<Arc<dyn ExecStream>>>,
}

impl Case {
    fn new() -> Self {
        let dir = tempfile::Builder::new().prefix("wsp-rev-le-").tempdir().expect("a temp root");
        let root = dir.path().to_str().expect("a utf-8 path").to_string();
        let run_dir = format!("{root}/runs");
        Case { _dir: dir, root, run_dir, ending: Mutex::new(Vec::new()) }
    }
    fn opts(&self) -> LocalExecOptions {
        LocalExecOptions { poll_ms: Some(50), ..LocalExecOptions::new(self.root.clone(), self.run_dir.clone()) }
    }
    fn factory(&self) -> Arc<LocalExecFactory> {
        local_exec_stream(self.opts(), None)
    }
    fn ended(&self, stream: Arc<dyn ExecStream>) -> Arc<dyn ExecStream> {
        self.ending.lock().unwrap().push(stream.clone());
        stream
    }
    fn go(&self, command: &str) -> Arc<dyn ExecStream> {
        self.ended(self.factory().launch(command, Launch::default()))
    }
    fn files(&self) -> Vec<String> {
        files(&self.run_dir)
    }
}

impl Drop for Case {
    fn drop(&mut self) {
        for stream in self.ending.lock().unwrap().drain(..) {
            stream.kill();
        }
        support::strays::sweep();
        unfreeze(&self.root);
    }
}

fn unfreeze(dir: &str) {
    let _ = Command::new("chattr").args(["-R", "-i", dir]).stderr(Stdio::null()).status();
}

fn files(dir: &str) -> Vec<String> {
    match std::fs::read_dir(dir) {
        Ok(d) => {
            let mut v: Vec<String> = d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
            v.sort();
            v
        }
        Err(_) => vec!["<no dir>".to_string()],
    }
}

fn suffixes(names: &[String]) -> Vec<String> {
    names.iter().map(|n| n.split_once('.').map(|(_, s)| s.to_string()).unwrap_or_else(|| n.clone())).collect()
}

async fn drain(stream: &dyn ExecStream) -> (Vec<String>, Option<ExecFailure>) {
    support::collect(stream).await
}

fn ms_since(t: Instant) -> u64 {
    t.elapsed().as_millis() as u64
}

fn with_input(lines: &[&str]) -> Launch {
    Launch { input: Some(lines.iter().map(|s| s.to_string()).collect()), ..Launch::default() }
}

fn settle_on(rx: oneshot::Receiver<()>) -> Settle {
    Box::pin(async move { rx.await.map_err(|_| "dropped".to_string()) })
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as f64
}

fn sleeper() -> i32 {
    let mut cmd = Command::new("sleep");
    cmd.arg("60").stdout(Stdio::null()).stderr(Stdio::null());
    spawn_reaped(cmd)
}

// ---------------------------------------------------------------------------------------------------------------
// differences

// F4 MINOR: TS local-exec.ts:779
own_group_test!(sweep_answers_the_runs_in_name_order_as_readdir_gives_them_in_node, 30_000, {
    let c = Case::new();
    std::fs::create_dir_all(&c.run_dir).unwrap();
    for n in [
        "0000000000aa",
        "000000000003",
        "000000000001",
        "0000000000ff",
        "ffffffffffff",
        "000000000002",
        "0123456789ab",
        "abcdefabcdef",
        "000000000010",
        "00000000000a",
        "00000000000b",
        "00000000000c",
    ] {
        std::fs::create_dir(format!("{}/{n}.d", c.run_dir)).unwrap();
    }
    let got = c.factory().sweep(&[]).expect("a sweep").await.expect("an answer");
    let got: Vec<String> = got.iter().map(|p| p[c.run_dir.len() + 1..].to_string()).collect();
    let mut sorted = got.clone();
    sorted.sort();
    assert_eq!(got, sorted, "node's readdirSync sorts by name; the sweep answered the directory's own order");
});

// F1 MAJOR: TS local-exec.ts:345-350, 626-643
own_group_test!(a_write_the_channel_refuses_leaves_the_held_seed_where_it_was, 30_000, {
    let c = Case::new();
    let (_tx, rx) = oneshot::channel::<()>();
    let stream = c.ended(c.factory().launch("sleep 30", Launch { input_after: Some(settle_on(rx)), ..with_input(&["seed"]) }));
    let base = stream.run().expect("a run");
    assert_eq!(support::read(&format!("{base}.held")), "seed\n");
    std::fs::remove_file(format!("{base}.in")).unwrap();
    symlink("/dev/full", format!("{base}.in")).unwrap();
    assert_eq!(stream.write("one".into()).await.unwrap(), WriteOutcome::Gone);
    assert!(std::path::Path::new(&format!("{base}.held")).exists(), "the TS keeps the held seed when the append failed");
    std::fs::remove_file(format!("{base}.in")).unwrap();
    std::fs::write(format!("{base}.in"), "").unwrap();
    assert_eq!(stream.write("two".into()).await.unwrap(), WriteOutcome::Written);
    assert_eq!(support::read(&format!("{base}.in")), "seed\ntwo\n");
});

// F1 MAJOR: TS local-exec.ts:345-350, 762-767
own_group_test!(an_attach_whose_held_seed_cannot_be_appended_fails_and_leaves_the_run_standing, 30_000, {
    let c = Case::new();
    std::fs::create_dir_all(&c.run_dir).unwrap();
    let base = format!("{}/00000000abcd", c.run_dir);
    let pid = sleeper();
    std::fs::create_dir(format!("{base}.d")).unwrap();
    std::fs::write(format!("{base}.pid"), format!("{pid}\n")).unwrap();
    std::fs::write(format!("{base}.log"), "").unwrap();
    symlink("/dev/full", format!("{base}.in")).unwrap();
    std::fs::write(format!("{base}.held"), "seed\n").unwrap();
    let attached = c
        .factory()
        .attach(&base, AttachRunOptions { input: true, started_at: now_ms(), from: None })
        .expect("an attach")
        .await;
    sleep_ms(300).await;
    let outcome = match &attached {
        Ok(Attached::Gone) => "gone".to_string(),
        Ok(Attached::Stream(_)) => "stream".to_string(),
        Err(e) => format!("failed: {}", e.message),
    };
    let left = suffixes(&c.files());
    assert!(outcome.starts_with("failed"), "the TS attach threw; this one answered {outcome}");
    assert!(alive(pid), "the run's process was ended; the TS left it");
    assert!(left.contains(&"held".to_string()), "the held seed was deleted: {left:?}");
});

// F1 MAJOR: TS local-exec.ts:355-366
own_group_test!(a_late_line_the_channel_refuses_stays_parked_and_is_not_marked_gone, 30_000, {
    let c = Case::new();
    let stream = c.ended(c.factory().launch("sleep 30", with_input(&["seed"])));
    let base = stream.run().expect("a run");
    std::fs::remove_file(format!("{base}.in")).unwrap();
    symlink("/dev/full", format!("{base}.in")).unwrap();
    let (tx, rx) = oneshot::channel::<()>();
    stream.write_after("late".into(), settle_on(rx)).expect("a parked line");
    sleep_ms(100).await;
    let parked = suffixes(&c.files());
    assert!(parked.contains(&"late".to_string()) && parked.contains(&"late.key".to_string()), "{parked:?}");
    tx.send(()).unwrap();
    sleep_ms(300).await;
    let after = suffixes(&c.files());
    assert!(after.contains(&"late".to_string()) && after.contains(&"late.key".to_string()), "the parked line was dropped: {after:?}");
    assert!(!after.contains(&"late.went".to_string()), "the line was marked as gone though it never went: {after:?}");
});

// F5 MINOR: TS local-exec.ts:706-708
own_group_test!(a_launch_that_cannot_make_its_folder_says_what_node_says, 30_000, {
    let c = Case::new();
    let afile = format!("{}/afile", c.root);
    std::fs::write(&afile, "x").unwrap();
    let mut wrong = Vec::new();
    for (run_dir, node_words) in [
        (format!("{afile}/runs"), format!("ENOTDIR: not a directory, mkdir '{afile}/runs'")),
        (afile.clone(), format!("EEXIST: file already exists, mkdir '{afile}'")),
    ] {
        let factory = local_exec_stream(LocalExecOptions::new(c.root.clone(), run_dir.clone()), None);
        let stream = factory.launch("echo", Launch::default());
        let (_, failure) = drain(&*stream).await;
        let message = failure.expect("a refusal").message;
        if message != node_words {
            wrong.push(format!("run folder {run_dir}: node says {node_words:?}, this says {message:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
});

// F11 NIT: TS local-exec.ts:664-675
own_group_test!(a_parked_line_s_files_exist_when_write_after_returns, 30_000, {
    let c = Case::new();
    let stream = c.ended(c.factory().launch("sleep 30", with_input(&["seed"])));
    let base = stream.run().expect("a run");
    let (_tx, rx) = oneshot::channel::<()>();
    stream.write_after("late".into(), settle_on(rx)).expect("a parked line");
    assert!(
        std::path::Path::new(&format!("{base}.late")).exists() && std::path::Path::new(&format!("{base}.late.key")).exists(),
        "the TS writes both files before writeAfter returns; here they come a task later: {:?}",
        suffixes(&c.files())
    );
});

// F9 MINOR: TS local-exec.ts:603
own_group_test!(a_second_lines_call_after_the_first_reader_went_answers_an_empty_stream_at_once, 30_000, {
    let c = Case::new();
    let stream = c.go("echo a; sleep 0.4; echo b; sleep 0.4; echo c");
    let mut first = stream.lines();
    assert_eq!(first.next().await.unwrap().unwrap(), "a");
    drop(first);
    let t = Instant::now();
    let mut second = stream.lines();
    let got = within(300, second.next()).await;
    assert!(matches!(got, Some(None)), "the second reader waited {} ms and answered {got:?}; the contract says empty at once", ms_since(t));
});

// F3 MINOR: TS local-exec.ts:331-332 (needs root with CAP_LINUX_IMMUTABLE and chattr)
own_group_test!(a_claim_the_reap_cannot_take_ends_the_stream_in_the_failure_node_throws, 30_000, {
    let c = Case::new();
    let stream = c.go(
        "f=\"${WSP_AGENT_PID_FILE%.agent}\"; mkdir \"$f.d/x\"; touch \"$f.d/x/y\"; chattr +i \"$f.d/x/y\"; echo ok",
    );
    let (lines, failure) = drain(&*stream).await;
    let code = stream.exited().await;
    let left = c.files();
    unfreeze(&c.root);
    assert_eq!(lines, vec!["ok".to_string()]);
    assert!(left.iter().any(|n| n.ends_with(".d")), "the claim should be left behind: {left:?}");
    assert!(failure.is_some() && code.is_none(), "the TS ended the stream with a failure and no code; this one ended with {failure:?} and {code:?}");
});

// F7 MINOR: TS local-exec.ts:322 (also 489, 252)
own_group_test!(a_pid_file_node_reads_as_a_number_names_the_group_it_names, 60_000, {
    let c = Case::new();
    std::fs::create_dir_all(&c.run_dir).unwrap();
    let forms: [(&str, fn(i32) -> String, bool); 9] = [
        ("plain", |p| format!("{p}"), true),
        ("plus", |p| format!("+{p}"), true),
        ("padded", |p| format!(" {p} \n"), true),
        ("point-zero", |p| format!("{p}.0"), true),
        ("hex", |p| format!("0x{p:x}"), true),
        ("exponent", |p| format!("{p}e0"), true),
        ("crlf", |p| format!("{p}\r\n"), true),
        ("two-lines", |p| format!("{p}\n{p}\n"), false),
        ("trailing-x", |p| format!("{p}x"), false),
    ];
    let mut wrong = Vec::new();
    for (i, (name, form, node_kills)) in forms.iter().enumerate() {
        let pid = sleeper();
        let base = format!("{}/00000000000{i}", c.run_dir);
        std::fs::create_dir(format!("{base}.d")).unwrap();
        std::fs::write(format!("{base}.pid"), form(pid)).unwrap();
        c.factory().sweep(&[]).expect("a sweep").await.unwrap();
        sleep_ms(150).await;
        let killed = !alive(pid);
        if killed != *node_kills {
            wrong.push(format!("{name}: node killed={node_kills}, this killed={killed}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:?}");
});

// F10 MINOR: TS local-exec.ts:312
own_group_test!(an_etime_of_absurd_days_is_no_reading_not_a_panic, 60_000, {
    let c = Case::new();
    let bin = format!("{}/bin", c.root);
    std::fs::create_dir(&bin).unwrap();
    std::fs::write(format!("{bin}/ps"), "#!/bin/sh\necho '99999999999999999999-00:00:01'\n").unwrap();
    std::fs::set_permissions(format!("{bin}/ps"), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    std::env::set_var("PATH", format!("{bin}:{}", std::env::var("PATH").unwrap_or_default()));
    let pid = sleeper();
    std::fs::create_dir_all(&c.run_dir).unwrap();
    let base = format!("{}/00000000feed", c.run_dir);
    std::fs::create_dir(format!("{base}.d")).unwrap();
    std::fs::write(format!("{base}.pid"), format!("{pid}\n")).unwrap();
    let led = wsp_core::local_exec::reap::leads_this_run(&base, pid).await;
    assert!(!led);
});

// F6 MINOR: TS local-exec.ts:734
own_group_test!(an_env_value_node_refuses_is_a_refused_launch_with_its_words, 60_000, {
    let c = Case::new();
    let mut env = wsp_adapters::port::Env::new();
    env.insert("A".into(), "x\0y".into());
    let t = Instant::now();
    let stream = c.factory().launch("echo hi", Launch { env, ..Launch::default() });
    let (lines, failure) = drain(&*stream).await;
    let code = stream.exited().await;
    assert!(
        failure.as_ref().is_some_and(|f| f.message.contains("must be a string without null bytes")),
        "the TS threw \"The property 'options.env['A']' must be a string without null bytes. Received 'x\\x00y'\"; this ended with lines {lines:?}, failure {failure:?}, code {code:?} after {} ms",
        ms_since(t)
    );
});

// F8 MINOR: TS local-exec.ts:455, 580
own_group_test!(a_started_at_that_is_not_a_finite_number_is_read_as_the_ts_reads_it, 60_000, {
    let c = Case::new();
    let mut wrong = Vec::new();
    for (tag, started_at, ts_cuts) in
        [("nan", f64::NAN, false), ("minus-huge", -1e300, true), ("plus-huge", 1e300, false), ("minus-inf", f64::NEG_INFINITY, true), ("zero", 0.0, true)]
    {
        let dir = format!("{}/r-{tag}", c.root);
        std::fs::create_dir_all(&dir).unwrap();
        let base = format!("{dir}/00000000abcd");
        let pid = sleeper();
        std::fs::create_dir(format!("{base}.d")).unwrap();
        std::fs::write(format!("{base}.pid"), format!("{pid}\n")).unwrap();
        std::fs::write(format!("{base}.log"), "").unwrap();
        let factory = local_exec_stream(LocalExecOptions { poll_ms: Some(50), ..LocalExecOptions::new(c.root.clone(), dir.clone()) }, None);
        let attached = factory.attach(&base, AttachRunOptions { input: false, started_at, from: None }).unwrap().await.unwrap();
        let Attached::Stream(stream) = attached else { panic!("gone") };
        let cut = within(800, stream.exited()).await.is_some();
        if cut != ts_cuts {
            wrong.push(format!("{tag}: the TS cuts the turn at once = {ts_cuts}, this = {cut}"));
        }
        stream.kill();
    }
    assert!(wrong.is_empty(), "{wrong:?}");
});

// pid-1 NOTE (same in the TS, not a Rust difference): TS local-exec.ts:488-496
// Run only inside a pid namespace of its own (unshare --pid --fork --mount-proc), where kill(-1) can reach nothing
// outside it; anywhere else this test returns at once.
own_group_test!(a_pid_file_naming_pid_1_is_signalled_as_every_process, 60_000, {
    let crowd = std::fs::read_dir("/proc").map(|d| d.flatten().filter(|e| e.file_name().to_string_lossy().chars().all(|ch| ch.is_ascii_digit())).count()).unwrap_or(9999);
    if std::env::var("WSP_REVIEW_PIDNS").is_err() || crowd > 30 {
        return;
    }
    let c = Case::new();
    std::fs::create_dir_all(&c.run_dir).unwrap();
    let bystander = sleeper();
    let base = format!("{}/00000000beef", c.run_dir);
    std::fs::create_dir(format!("{base}.d")).unwrap();
    std::fs::write(format!("{base}.pid"), "1\n").unwrap();
    std::fs::write(format!("{base}.log"), "").unwrap();
    let attached = c.factory().attach(&base, AttachRunOptions { input: false, started_at: now_ms(), from: None }).unwrap().await.unwrap();
    let Attached::Stream(stream) = attached else { panic!("gone") };
    stream.teardown();
    sleep_ms(300).await;
    assert!(alive(bystander), "a process that shares nothing with the run was sent TERM by teardown() because the run's pid file said 1");
});

// F3 MINOR: TS local-exec.ts:331-332
// Needs a root without CAP_DAC_OVERRIDE, or any other user: capsh --drop=cap_dac_override,cap_dac_read_search -- -c '<this binary> ...'
own_group_test!(permissions_the_command_changes_on_its_own_run_files_end_the_stream_as_in_the_ts, 120_000, {
    let c = Case::new();
    // (tag, body, TS lines, TS failure begins, TS code, TS still running, TS files left)
    type Row = (&'static str, &'static str, Vec<&'static str>, Option<&'static str>, Option<i32>, bool, Vec<&'static str>);
    let rows: Vec<Row> = vec![
        ("pid", "chmod 000 \"$f.pid\"; sleep 0.6; echo end", vec![], None, None, true, vec![]),
        (
            "rundir",
            "chmod 500 \"$(dirname \"$f\")\"; echo hi",
            vec!["hi", "<bash: exit file: Permission denied>"],
            Some("EACCES: permission denied, unlink"),
            None,
            false,
            vec!["d", "log", "pid", "sh"],
        ),
        ("claim", "chmod 000 \"$f.d\"; echo hi", vec!["hi"], None, Some(0), false, vec![]),
        ("log", "chmod 000 \"$f.log\"; echo hi", vec![], None, Some(0), false, vec![]),
    ];
    let mut wrong = Vec::new();
    for (tag, body, want_lines, want_failure, want_code, want_running, want_left) in rows {
        let dir = format!("{}/perm-{tag}", c.root);
        std::fs::create_dir_all(&dir).unwrap();
        let marker = format!("{}/perm-{tag}.pid", c.root);
        let run_dir = format!("{dir}/runs");
        let factory = local_exec_stream(LocalExecOptions { poll_ms: Some(50), ..LocalExecOptions::new(c.root.clone(), run_dir.clone()) }, None);
        let stream = c.ended(factory.launch(&format!("f=\"${{WSP_AGENT_PID_FILE%.agent}}\"; echo $$ > {marker}; {body}"), Launch::default()));
        let (lines, failure) = drain(&*stream).await;
        let code = stream.exited().await;
        sleep_ms(100).await;
        let kid: i32 = support::read(&marker).trim().parse().unwrap();
        let running = alive(kid);
        if running {
            record(kid);
        }
        let left = suffixes(&files(&run_dir));
        let lines: Vec<String> = lines.iter().map(|l| if l.contains("Permission denied") { "<bash: exit file: Permission denied>".to_string() } else { l.clone() }).collect();
        let failure_head = failure.as_ref().map(|f| f.message.clone());
        let got = (lines.clone(), failure_head.clone(), code, running, left.clone());
        let ok = lines == want_lines
            && failure_head.as_deref().map(|m| want_failure.is_some_and(|w| m.starts_with(w))).unwrap_or(want_failure.is_none())
            && code == want_code
            && running == want_running
            && left == want_left;
        if !ok {
            wrong.push(format!("{tag}: TS answered lines {want_lines:?}, failure {want_failure:?}, code {want_code:?}, running {want_running}, left {want_left:?}; this answered {got:?}"));
        }
        // restore, so the temp folder can go
        let _ = Command::new("chmod").args(["-R", "u+rwx", &dir]).status();
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
});

// F2 MINOR: TS local-exec.ts:734-743
own_group_test!(a_turn_starts_with_every_signal_at_its_default_whatever_the_host_ignores, 60_000, {
    let c = Case::new();
    let ask = |c: &Case| c.go("grep -E '^SigIgn:' /proc/$$/status");
    let base = ask(&c);
    let (lines, _) = drain(&*base).await;
    // SAFETY: the test process, which is in a group of its own, ignores four signals the way nohup and a background job do.
    unsafe {
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
        libc::signal(libc::SIGINT, libc::SIG_IGN);
        libc::signal(libc::SIGQUIT, libc::SIG_IGN);
        libc::signal(libc::SIGUSR1, libc::SIG_IGN);
    }
    let again = ask(&c);
    let (lines2, _) = drain(&*again).await;
    assert_eq!(lines2, lines, "node resets every signal to its default in the child (libuv uv__process_child_init), so a host started with nohup or in the background hands its turns nothing ignored");
});
