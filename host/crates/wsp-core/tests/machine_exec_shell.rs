// SPDX-License-Identifier: AGPL-3.0-only
//! The shell text the machine road sends, against the text the TS sent for the same calls (tests/fixtures/
//! machine_exec_shell.json, printed under node by a recording machine that answers as this file's does). Specs:
//! spec/machine_exec_factory.md rules 3-6, 9, 10 and spec/machine_exec_stream.md rules 4, 9-14. The TS order of calls
//! is its microtask order, so the comparison is of the set of calls; their text is byte for byte.
mod support;

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::Engine as _;
use regex::Regex;
use wsp_adapters::port::{AttachRunOptions, ExecOptions as Launch, ExecStream, ExecStreamFactory, Settle};
use wsp_core::machine_exec::factory::{
    attach_probe, claim, claim_mkdir, grace_checks, holds_run_probe, minted, reap_script, release_held_sh, release_late_sh, run_script, sweep_listing, sweep_page,
};
use wsp_core::machine_exec::stream::{append_before, close_input_cmd, end_leftovers_cmd, js_parse_int, poll_cmd, reap_read, signal_cmd};
use wsp_core::machine_exec::{machine_exec_stream, MachineExecOptions};
use wsp_engine::errors::Result;
use wsp_engine::machine::{ExecOptions, ExecResult, MachineKind, MachineLife, MachineState, RunOptions, SnapshotOptions};
use wsp_engine::Machine;

fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("fixtures/machine_exec_shell.json")).unwrap()
}

/// Every base64 payload decoded between « and », every 12-hex word read `<hex>`, as the fixture was written.
fn norm(cmd: &str) -> String {
    let b64 = Regex::new(r"printf %s '([A-Za-z0-9+/=]+)' \| base64 -d").unwrap();
    let decoded = b64.replace_all(cmd, |c: &regex::Captures| {
        let text = String::from_utf8_lossy(&base64::engine::general_purpose::STANDARD.decode(&c[1]).unwrap()).into_owned();
        format!("printf %s «{text}» | base64 -d")
    });
    Regex::new(r"[0-9a-f]{12}").unwrap().replace_all(&decoded, "<hex>").into_owned()
}

fn expected(key: &str) -> BTreeSet<String> {
    fixture()[key].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect()
}

/// The TS probe's recording machine: answers each kind of exec as the fixture's machine did.
struct Recorder {
    calls: Mutex<Vec<String>>,
}

#[async_trait(?Send)]
impl Machine for Recorder {
    fn id(&self) -> &str {
        "rec"
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    async fn exec(&self, cmd: &str, _opts: ExecOptions) -> Result<ExecResult> {
        let polls_before = {
            let mut calls = self.calls.lock().unwrap();
            calls.push(cmd.to_string());
            calls.iter().filter(|c| c.contains("__WSP_EOF_")).count()
        };
        let cap = |re: &str| Regex::new(re).unwrap().captures(cmd).map(|c| c[1].to_string());
        let out = if let Some(m) = cap(r"(__WSP_REAPED_[0-9a-f]+__)") {
            format!("\n{m} 0\n")
        } else if let Some(s) = cap(r"(__WSP_EOF_[0-9a-f]+__)") {
            format!("\n{s} {} up \n", if polls_before > 2 { "0" } else { "" })
        } else if cmd.contains("WSP_LAUNCHED") {
            "WSP_LAUNCHED\n".into()
        } else if cmd.contains("echo WSP_OK") {
            "WSP_OK\n".into()
        } else if cmd.starts_with("for d in") {
            "/r/aaaaaaaaaaaa.d\n".into()
        } else if cmd.contains("WSP_RUN") {
            "WSP_RUN\none\n".into()
        } else {
            String::new()
        };
        Ok(ExecResult { exit_code: support::code(0), stdout: out, stderr: String::new() })
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> Result<ExecResult> {
        panic!("not on this road")
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        panic!("not on this road")
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        panic!("not on this road")
    }
    async fn resume(&self, _signal: Option<&wsp_engine::Cancel>) -> Result<()> {
        panic!("not on this road")
    }
    async fn kill(&self) -> Result<()> {
        panic!("not on this road")
    }
    async fn state(&self) -> Result<MachineState> {
        panic!("not on this road")
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        panic!("not on this road")
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        panic!("not on this road")
    }
}

fn recorder() -> (Arc<Recorder>, Arc<dyn Machine>) {
    let r = Arc::new(Recorder { calls: Mutex::new(Vec::new()) });
    (r.clone(), r)
}

async fn drain(s: &dyn ExecStream, calls: &Mutex<Vec<String>>) {
    if let (_, Some(e)) = support::collect(s).await {
        calls.lock().unwrap().push(format!("THREW {}", e.message));
    }
}

fn resolved() -> Settle {
    Box::pin(async { Ok(()) })
}

#[tokio::test(flavor = "multi_thread")]
async fn the_shell_text_is_the_ts_text_byte_for_byte() {
    // A launch with an input and an environment, a write, a held line, rest, the next turn, a close, a TERM, then the
    // idle cut ends it.
    {
        let (rec, machine) = recorder();
        let f = machine_exec_stream(machine, MachineExecOptions { poll_ms: Some(1), run_dir: Some("/r".into()), idle_ms: Some(2.0), deadline_ms: Some(1e9), ..Default::default() }, None);
        let env = [("A", "x y"), ("BAD-KEY", "z")].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let s = f.launch("claude -p 'hi'", Launch { env, input: Some(vec!["seed".into()]), ..Default::default() });
        s.write("more".into()).await.unwrap();
        s.write_after("late".into(), resolved());
        support::sleep_ms(50).await;
        s.rest();
        s.next_turn();
        s.close_input();
        s.teardown();
        drain(&*s, &rec.calls).await;
        support::sleep_ms(50).await;
        let got: BTreeSet<String> = rec.calls.lock().unwrap().iter().map(|c| norm(c)).collect();
        assert_eq!(got, expected("launchedWithInput"));
    }
    // An attach with input, one without, and a sweep naming another run.
    {
        let (rec, machine) = recorder();
        let f = machine_exec_stream(machine, MachineExecOptions { poll_ms: Some(1), run_dir: Some("/r".into()), ..Default::default() }, None);
        let _ = f.attach("/r/bbbbbbbbbbbb", AttachRunOptions { input: true, started_at: 0.0, from: None }).unwrap().await.unwrap();
        let _ = f.attach("/r/bbbbbbbbbbbb", AttachRunOptions { input: false, started_at: 0.0, from: None }).unwrap().await.unwrap();
        let _ = f.sweep(&["/r/cccccccccccc".to_string()]).unwrap().await.unwrap();
        let got: BTreeSet<String> = rec.calls.lock().unwrap().iter().map(|c| norm(c)).collect();
        assert_eq!(got, expected("attachAndSweep"));
    }
    // A held seed that comes due at once, then a kill.
    {
        let (rec, machine) = recorder();
        let f = machine_exec_stream(machine, MachineExecOptions { poll_ms: Some(1), run_dir: Some("/r".into()), ..Default::default() }, None);
        let s = f.launch("echo hi", Launch { input: Some(vec!["seed".into()]), input_after: Some(resolved()), ..Default::default() });
        support::sleep_ms(30).await;
        s.kill();
        drain(&*s, &rec.calls).await;
        support::sleep_ms(50).await;
        let got: BTreeSet<String> = rec.calls.lock().unwrap().iter().map(|c| norm(c)).collect();
        assert_eq!(got, expected("heldSeedKilled"));
    }
}

#[test]
fn the_shell_builders_are_the_ts_text() {
    let b = "/r/0123456789ab";
    let all: BTreeSet<String> = ["launchedWithInput", "attachAndSweep", "heldSeedKilled"].iter().flat_map(|k| expected(k)).collect();
    let has = |text: &str| {
        let n = norm(text);
        assert!(all.iter().any(|c| c.contains(&n)), "the TS sent no call holding:\n{n}");
    };
    has(&release_held_sh(b));
    has(&release_late_sh(b));
    has(&reap_script(b, "", true));
    has(&holds_run_probe(b));
    has(&claim_mkdir(b));
    has(&attach_probe(b, true));
    has(&attach_probe(b, false));
    has(&sweep_listing("/r"));
    has(&sweep_page(&[b.to_string()]));
    has(&poll_cmd(b, "__WSP_EOF_0123456789ab__", 0, false));
    has(&reap_read(b, "__WSP_REAPED_0123456789ab__", 0, None));
    has(&signal_cmd(b, "TERM"));
    has(&signal_cmd(b, "KILL"));
    has(&close_input_cmd(b));
    has(&end_leftovers_cmd(b));
    for line in append_before(b) {
        has(&line);
    }
    assert_eq!(grace_checks(), "1 2 3 4 5 6 7 8 9 10");
    assert_eq!(claim(b), "/r/0123456789ab.d");
    // The script, decoded out of the launch the TS sent.
    let script = format!("{{ echo 500 > /proc/self/oom_score_adj; }} 2>/dev/null; renice 0 $$ >/dev/null 2>&1\n( tail -n +1 -f '{b}'.in > '{b}'.fifo & echo $! > '{b}'.tail )\n{{ claude -p 'hi'\n}} < '{b}'.fifo\necho $? > '{b}'.exit\nkill $(cat '{b}'.tail) 2>/dev/null\n");
    assert_eq!(run_script(b, "claude -p 'hi'", true), script);
    assert_eq!(run_script(b, "echo hi", false), format!("{{ echo 500 > /proc/self/oom_score_adj; }} 2>/dev/null; renice 0 $$ >/dev/null 2>&1\necho hi\necho $? > '{b}'.exit\n"));
    // The poll that asks for the tree's work.
    let work = poll_cmd(b, "__WSP_EOF_0123456789ab__", 7, true);
    assert!(work.contains(&format!("tail -c +8 '{b}'.log 2>/dev/null | head -c 262144 | base64 -w0; P=$(cat '{b}'.pid 2>/dev/null); W=$(awk -v p=\"$P\" '{}' /proc/[0-9]*/stat 2>/dev/null); printf", wsp_core::machine_exec::TREE_WORK_AWK)));
    // A later reap stops at the size the first one read and signals nothing.
    assert_eq!(
        reap_read(b, "__WSP_REAPED_0123456789ab__", 262_144, Some(300_000)),
        format!("S=300000; N=$((S - 262144)); [ $N -gt 262144 ] && N=262144; [ $N -gt 0 ] && tail -c +262145 '{b}'.log 2>/dev/null | head -c $N | base64 -w0; printf '\\n%s %s\\n' __WSP_REAPED_0123456789ab__ \"$((S))\"; [ $((S - 262144)) -gt 262144 ] && exit 0; ")
    );
    assert_eq!(reap_script(b, "READ; ", false), format!("READ; rm -rf '{b}'.*"));
}

#[test]
fn a_handle_is_minted_only_in_the_run_folder_with_12_lowercase_hex() {
    assert!(minted("/tmp/wsp-run", "/tmp/wsp-run/aabbccddeeff"));
    for bad in ["/tmp/wsp-run/../../etc/x", "/tmp/wsp-run/$(id)", "/etc/wsp-run/aabbccddeeff", "/tmp/wsp-run/nothex000000", "/tmp/wsp-run/aabbccddeef", "/tmp/wsp-run/AABBCCDDEEFF", "/tmp/wsp-run/aabbccddeeff/", "/tmp/wsp-run//aabbccddeeff", "/tmp/wsp-runaabbccddeeff"] {
        assert!(!minted("/tmp/wsp-run", bad), "{bad}");
    }
}

#[test]
fn js_parse_int_reads_as_parse_int_does() {
    assert_eq!(js_parse_int("14"), Some(14));
    assert_eq!(js_parse_int("14abc"), Some(14));
    assert_eq!(js_parse_int("  -3\n"), Some(-3));
    assert_eq!(js_parse_int("+7"), Some(7));
    assert_eq!(js_parse_int("abc"), None);
    assert_eq!(js_parse_int(""), None);
    assert_eq!(js_parse_int("1.9"), Some(1));
    assert_eq!(js_parse_int("99999999999999999999"), None);
}
