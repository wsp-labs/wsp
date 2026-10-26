// SPDX-License-Identifier: AGPL-3.0-only
//! The exec run unit against the TS code's own answers (tests/fixtures/roads/exec_run.json, the base pinned to
//! abababababab). Spec: spec/exec_run.md.
mod roads_common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use base64::Engine;
use roads_common::*;
use wsp_engine::exec_detached::*;
use wsp_engine::machine::RunOptions;

const BASE: &str = "/tmp/wsp-run/abababababab";

fn b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(s)
}

fn poll(exit: &str, out: &str, err: &str, up: bool) -> String {
    format!("WSP_POLL\n{exit}\n{}\n{}\n{}\nWSP_POLL_END\n", b64(out), b64(err), if up { "up" } else { "down" })
}

#[test]
fn launch_poll_kill_and_clean_commands_match_the_ts() {
    let d = fixture("exec_run.json");
    let calls: Vec<String> = d["calls"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect();
    assert_eq!(poll_command(BASE, 0, 0), calls[1]);
    assert_eq!(poll_command(BASE, 2, 0), calls[2]);
    assert_eq!(clean_command(BASE), calls[3]);
    let kills: Vec<String> = d["killCalls"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect();
    assert_eq!(kill_command("/r/abababababab"), kills[kills.len() - 3]);
    assert_eq!(upload_sequence(
        &[GuestWrite { path: format!("{BASE}.sh"), text: "echo it's".into(), append: false }],
        &["u=$(umask)", "umask 077", &format!("b={BASE}"), "mkdir \"$b.d\" 2>/dev/null || { echo WSP_LAUNCHED; exit 0; }"].map(String::from),
        &[
            "setsid nohup bash -c '( umask \"$1\"; exec bash \"$0.sh\" ) > \"$0.out\" 2> \"$0.err\" < /dev/null; echo $? > \"$0.exit\"' \"$b\" \"$u\" > /dev/null 2>&1 &",
            "echo $! > \"$b.pid\"",
            "echo WSP_LAUNCHED",
        ]
        .map(String::from),
        "abababababab",
        None,
    )
    .unwrap(), vec![calls[0].clone()]);
}

#[tokio::test]
async fn the_launch_is_put_files_of_the_script_behind_its_claim() {
    let d = fixture("exec_run.json");
    let want = d["calls"][0].as_str().unwrap();
    let machine = ScriptedMachine::new("m1", |_c, _o| Ok(res(0, "WSP_LAUNCHED\n", "")));
    let launched = launch(machine.as_ref(), BASE, "echo it's").await.unwrap();
    assert_eq!((launched.exit_code, launched.pieces), (0.into(), 0));
    let calls = machine.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].cmd, want);
    assert!(calls[0].opts.idempotency_key.as_deref().unwrap().ends_with("/last"));
}

#[test]
fn parse_poll_reads_only_a_whole_block() {
    let block = format!("noise\n{}", poll(" 3 ", "out", "err", true));
    assert_eq!(parse_poll(&block), Some(Poll { exit: "3".into(), out: b"out".to_vec(), err: b"err".to_vec(), up: true }));
    // The line five after the start must close the block, and up is the exact word.
    assert_eq!(parse_poll("WSP_POLL\n\n\n\nup\nnot the end\n"), None);
    assert_eq!(parse_poll("WSP_POLL\n\n\n\n up\nWSP_POLL_END"), Some(Poll { exit: "".into(), out: vec![], err: vec![], up: false }));
    assert_eq!(parse_poll(""), None);
    // A leading space before the marker is no marker.
    assert_eq!(parse_poll(" WSP_POLL\n\n\n\nup\nWSP_POLL_END"), None);
}

#[tokio::test]
async fn the_ts_run_reads_and_cleans_as_recorded() {
    let d = fixture("exec_run.json");
    let polls = AtomicUsize::new(0);
    let machine = ScriptedMachine::new("m1", move |cmd, _o| {
        if cmd.contains("echo WSP_LAUNCHED") {
            return Ok(res(0, "WSP_LAUNCHED\n", ""));
        }
        if cmd.contains("echo WSP_POLL") {
            return Ok(res(
                0,
                &if polls.fetch_add(1, Ordering::SeqCst) == 0 { poll("", "a\n", "", true) } else { poll("0", "", "e", false) },
                "",
            ));
        }
        Ok(res(0, "", ""))
    });
    let got = exec_detached(machine.as_ref(), "echo it's", RunOptions { deadline_ms: 60_000, poll_ms: Some(1), ..Default::default() })
        .await
        .unwrap();
    assert_eq!((got.exit_code, got.stdout.as_str(), got.stderr.as_str()), (0.into(), "a\n", "e"));
    let base = machine.cmds()[1].lines().next().unwrap().strip_prefix("b=").unwrap().to_string();
    let want: Vec<String> = d["calls"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().replace(BASE, &base)).collect();
    assert_eq!(machine.cmds(), want);
    assert!(machine.calls.lock().unwrap()[1..]
        .iter()
        .all(|c| c.opts.timeout_ms == Some(INLINE_EXEC_MS) && c.opts.idempotency_key.is_none()));
}

#[tokio::test]
async fn a_down_after_an_up_still_counts_as_the_second() {
    // The count of downs is never reset by an up in between: down, up, down ends the run.
    let answers = Mutex::new(vec![poll("", "x", "", false), poll("", "", "", true), poll("", "", "", false), poll("0", "", "", false)]);
    let machine = ScriptedMachine::new("m1", move |cmd, _o| {
        if cmd.contains("echo WSP_LAUNCHED") {
            Ok(res(0, "WSP_LAUNCHED\n", ""))
        } else if cmd.contains("echo WSP_POLL") {
            Ok(res(0, &answers.lock().unwrap().remove(0), ""))
        } else {
            Ok(res(0, "", ""))
        }
    });
    let got =
        exec_detached(machine.as_ref(), "true", RunOptions { deadline_ms: 10_000, poll_ms: Some(1), ..Default::default() }).await.unwrap();
    assert_eq!(got.exit_code, (-1).into());
    assert_eq!(got.stdout, "x");
    assert_eq!(got.stderr, "wsp: the command ended without reporting an exit code\n");
    assert_eq!(machine.cmds().iter().filter(|c| c.contains("echo WSP_POLL")).count(), 3);
    assert!(machine.cmds().last().unwrap().starts_with("rm -rf /tmp/wsp-run/"));
}

#[tokio::test]
async fn the_note_goes_on_its_own_line() {
    for (err, want) in [
        ("", "wsp: the command ended without reporting an exit code\n"),
        ("half", "half\nwsp: the command ended without reporting an exit code\n"),
        ("whole\n", "whole\nwsp: the command ended without reporting an exit code\n"),
    ] {
        let err = err.to_string();
        let machine = ScriptedMachine::new("m1", move |cmd, _o| {
            if cmd.contains("echo WSP_LAUNCHED") {
                Ok(res(0, "WSP_LAUNCHED\n", ""))
            } else if cmd.contains("echo WSP_POLL") {
                Ok(res(0, &poll("not-a-number", "", &err, false), ""))
            } else {
                Ok(res(0, "", ""))
            }
        });
        let got = exec_detached(machine.as_ref(), "true", RunOptions { deadline_ms: 10_000, poll_ms: Some(1), ..Default::default() })
            .await
            .unwrap();
        assert_eq!((got.exit_code, got.stderr.as_str()), ((-1).into(), want));
    }
}

#[tokio::test]
async fn lines_flush_stdout_then_stderr_and_skip_empty_ones() {
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let s = seen.clone();
    let machine = ScriptedMachine::new("m1", |cmd, _o| {
        if cmd.contains("echo WSP_LAUNCHED") {
            Ok(res(0, "WSP_LAUNCHED\n", ""))
        } else if cmd.contains("echo WSP_POLL") {
            Ok(res(0, &poll("0", "a\n\nb-tail", "e-tail", false), ""))
        } else {
            Ok(res(0, "", ""))
        }
    });
    let got = exec_detached(
        machine.as_ref(),
        "true",
        RunOptions {
            deadline_ms: 10_000,
            poll_ms: Some(1),
            on_line: Some(Arc::new(move |l: &str| s.lock().unwrap().push(l.to_string()))),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!((got.stdout.as_str(), got.stderr.as_str()), ("a\n\nb-tail", "e-tail"));
    assert_eq!(*seen.lock().unwrap(), ["a", "b-tail", "e-tail"]);
}

#[tokio::test]
async fn a_deadline_run_kills_reads_once_more_and_cleans() {
    let machine =
        ScriptedMachine::new(
            "m1",
            |cmd, _o| {
                if cmd.contains("echo WSP_LAUNCHED") {
                    Ok(res(0, "WSP_LAUNCHED\n", ""))
                } else {
                    Ok(res(0, "", ""))
                }
            },
        );
    let got = exec_detached_in(machine.as_ref(), "sleep 9", RunOptions { deadline_ms: 0, poll_ms: Some(1), ..Default::default() }, "/r")
        .await
        .unwrap();
    assert_eq!((got.exit_code, got.stdout.as_str(), got.stderr.as_str()), (DEADLINE_EXIT.into(), "", ""));
    let cmds = machine.cmds();
    let base = cmds[0].lines().find_map(|l| l.strip_prefix("b=")).unwrap().to_string();
    assert!(base.starts_with("/r/") && base.len() == "/r/".len() + 12, "{base}");
    // The tail of every deadline: the kill, one more poll from the offsets read so far, the cleanup.
    assert_eq!(cmds[cmds.len() - 3..].to_vec(), vec![kill_command(&base), poll_command(&base, 0, 0), clean_command(&base)]);
}
