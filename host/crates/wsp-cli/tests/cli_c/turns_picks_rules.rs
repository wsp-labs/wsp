// SPDX-License-Identifier: AGPL-3.0-only
//! Hand-written tests of `turns_picks`: the rules the recorded cases do not state on their own, each named for what it
//! protects. The recorded cases (`turns_picks.rs`) hold the Rust to the TS value for value; these hold the invariants
//! (the guest rule, the cloud-off words) and the orders of a frame, and tie each ported TS test to its recorded case.
//! Spec: `spec/turns-picks.md`, whose rule table names these tests.
use std::collections::BTreeSet;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use indexmap::IndexMap;
use serde_json::json;
use wsp_cli::verbs::turns_help as t;
use wsp_wire::views::session::ThreadView;

use super::common::{case, cases, holds};
use super::drivers_turns_picks as d;

const UNIT: &str = "turns_picks";

fn recorded(func: &str, name: &str) -> super::common::Case {
    case(UNIT, func, name, "<cwd>")
}

/// A named pipe: a read of it blocks for ever, so a door that touches the path hangs where it should have refused.
fn pipe_in(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    let c = CString::new(path.as_os_str().as_bytes()).expect("a path");
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0, "mkfifo");
    path
}

fn within<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(3)).expect("the call touched a path it should not have: it is blocked on the pipe")
}

fn guest_line() -> String {
    recorded("filesFrom", "elsewhere refuses before any path is touched").error.expect("a recorded refusal").message
}

fn thread_row() -> ThreadView {
    serde_json::from_value(json!({ "id": "row_1", "workspaceId": "ws_1", "harness": "claude", "startedBy": "person", "status": "completed", "title": "hello", "sessionId": "row_1", "turns": 1, "ran": true })).expect("a thread row")
}

fn keys(c: &super::common::Case) -> Vec<String> {
    c.value.as_ref().and_then(|v| v.as_object()).expect("a frame").keys().cloned().collect()
}

fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(|s| s.to_string()).collect()
}

/// Guest rule (host invariant of AGENTS.md "a caller on a machine never has this computer's disk read"): a line from a
/// machine that names a file is refused before the path is resolved, statted or opened, in one sentence whatever the path is.
#[test]
fn guest_files_from_refuses_before_any_path_is_touched() {
    let dir = tempfile::tempdir().expect("folder");
    let pipe = pipe_in(dir.path(), "pipe");
    let folder = dir.path().to_string_lossy().into_owned();
    let present = dir.path().join("present.txt");
    std::fs::write(&present, "hello").expect("file");
    let want = guest_line();
    for path in [pipe.to_string_lossy().into_owned(), present.to_string_lossy().into_owned(), folder, "no-such-file-here.txt".to_string()] {
        let got = within(move || t::files_from(&[path], true));
        let e = got.expect_err("a guest names a file");
        assert_eq!(e.message, want);
        assert_eq!(e.kind.as_ref().map(|k| k.as_str()), Some("usage"));
    }
}

/// Guest rule, through the two doors that read files: the start of a thread and a message into one.
#[test]
fn guest_opening_of_and_message_to_refuse_before_any_path_is_touched() {
    let dir = tempfile::tempdir().expect("folder");
    let pipe = pipe_in(dir.path(), "pipe").to_string_lossy().into_owned();
    let want = guest_line();
    let p = pipe.clone();
    let opening = within(move || {
        let opts = t::OpeningOpts { files: vec![p], elsewhere: true, ..Default::default() };
        t::opening_of(&IndexMap::new(), &t::OpeningAt::Workspace { id: "ws-1".into() }, "go", &opts)
    });
    assert_eq!(opening.expect_err("a guest").message, want);
    let sent = within(move || t::message_to(&thread_row(), "again", &t::Picks::default(), &[pipe], true));
    assert_eq!(sent.expect_err("a guest").message, want);
}

/// A path that is a pipe is no file: refused as one that is not a file, without opening it.
#[test]
fn a_pipe_is_no_file_and_is_not_opened() {
    let dir = tempfile::tempdir().expect("folder");
    let pipe = pipe_in(dir.path(), "pipe").to_string_lossy().into_owned();
    let shown = pipe.clone();
    let got = within(move || t::files_from(&[pipe], false));
    let e = got.expect_err("a pipe is no file");
    assert!(e.message.starts_with(&format!("there is no file at {shown} on this computer")), "{}", e.message);
    assert_eq!(e.kind.as_ref().map(|k| k.as_str()), Some("usage"));
}

/// The head an image is told by is twelve bytes: the longest of the four types (WebP's RIFF header and its word).
#[test]
fn image_head_is_twelve_bytes() {
    assert_eq!(t::IMAGE_HEAD_BYTES, 12);
    holds(UNIT, "filesFrom", "a webp is told by its twelve byte head", d::files_from);
    holds(UNIT, "filesFrom", "a webp cut at eleven bytes is a plain file", d::files_from);
}

/// Host invariant 7 (with the cloud off no line names a cloud), as it reaches `thread_fork` through `thread_at`: with the
/// cloud off a ref that names a machine is only a ref that names no thread, and no machine is read or named.
#[test]
fn cloud_off_a_fork_ref_never_names_a_machine() {
    let mut seen = 0;
    for c in cases(UNIT).into_iter().filter(|c| c.func == "threadFork" && !c.cloud) {
        assert!(!c.frames.iter().any(|f| f.contains("\"workspaces.resolve\"")), "{}: a machine was asked for with the cloud off", c.case);
        if let Some(e) = &c.error {
            assert!(!e.message.contains(" is a machine"), "{}: {}", c.case, e.message);
        }
        holds(UNIT, "threadFork", &c.case, d::thread_fork);
        seen += 1;
    }
    assert!(seen > 20);
}

/// The same ref with the cloud on is refused as a machine, so the cloud-off silence above is the setting and not the data.
#[test]
fn cloud_on_a_fork_ref_names_a_machine() {
    let c = recorded("threadFork", "a ref that is a machine, with the cloud");
    assert!(c.cloud);
    assert!(c.error.expect("a refusal").message.contains("attic-work is a machine"));
    holds(UNIT, "threadFork", "a ref that is a machine, with the cloud", d::thread_fork);
}

/// One sentence for a bad `--access` word at every door that reads one: `picks_of`, `checked_start` and `thread_fork`
/// (and, through `picks_of`, `opening_of` and `message_to`).
#[test]
fn a_bad_access_word_is_one_sentence_at_every_door() {
    let a = recorded("picksOf", "an access that is no word of wsp's").error.expect("refused");
    let b = recorded("checkedStart", "a word that is no access word is refused before the lists are read against").error.expect("refused");
    let c = recorded("threadFork", "a bad access word").error.expect("refused");
    assert_eq!(a, b);
    assert_eq!(a, c);
    holds(UNIT, "picksOf", "an access that is no word of wsp's", d::picks_of);
    holds(UNIT, "checkedStart", "a word that is no access word is refused before the lists are read against", d::checked_start);
    holds(UNIT, "threadFork", "a bad access word", d::thread_fork);
}

/// The order of the frame `opening_of` writes, for each kind of target: target fields, prompt, cwd, harness, notify, turn
/// token, title, replaces, attachments, resume, picks.
#[test]
fn opening_of_field_order_for_a_workspace() {
    let name = "a workspace with every option and a turn token";
    assert_eq!(keys(&recorded("openingOf", name)), strings(&["workspaceId", "prompt", "cwd", "harness", "notify", "turnToken", "title", "replaces", "attachments", "model", "effort", "access", "fast"]));
    holds(UNIT, "openingOf", name, d::opening_of);
}

#[test]
fn opening_of_field_order_for_a_folder_target() {
    let name = "here with project, branch, folder and every option";
    assert_eq!(keys(&recorded("openingOf", name)), strings(&["project", "branch", "prompt", "cwd", "harness", "notify", "turnToken", "title", "replaces", "attachments", "model", "effort", "access", "fast"]));
    holds(UNIT, "openingOf", name, d::opening_of);
}

#[test]
fn opening_of_field_order_for_a_project_fork() {
    let name = "a fork with every option";
    assert_eq!(keys(&recorded("openingOf", name)), strings(&["prompt", "cwd", "harness", "notify", "turnToken", "title", "replaces", "attachments", "model", "effort", "access", "fast"]));
    holds(UNIT, "openingOf", name, d::opening_of);
}

#[test]
fn opening_of_field_order_for_a_thread_fork() {
    let name = "a thread's fork with every option";
    assert_eq!(keys(&recorded("openingOf", name)), strings(&["fork", "branch", "prompt", "cwd", "harness", "notify", "turnToken", "title", "replaces", "attachments", "model", "effort", "access", "fast"]));
    holds(UNIT, "openingOf", name, d::opening_of);
}

#[test]
fn opening_of_field_order_with_a_resume() {
    let name = "a resume after the attachments, before the picks";
    assert_eq!(keys(&recorded("openingOf", name)), strings(&["workspaceId", "prompt", "title", "attachments", "resume", "model"]));
    holds(UNIT, "openingOf", name, d::opening_of);
}

/// `message_to` writes workspaceId, prompt, harness, thread, attachments, picks: no folder, no target, no token.
#[test]
fn message_to_field_order() {
    let name = "a file travels before the picks";
    assert_eq!(keys(&recorded("messageTo", name)), strings(&["workspaceId", "prompt", "harness", "thread", "attachments", "model"]));
    holds(UNIT, "messageTo", name, d::message_to);
    holds(UNIT, "messageTo", "takes no folder, no target and no token", d::message_to);
}

/// Only `WSP_TURN` is the turn's token; the host's own token and look-alike names are not read by `opening_of`.
#[test]
fn opening_of_reads_only_the_turn_token() {
    for name in ["a workspace, the turn token alone", "an empty turn token is no token", "the host token is not the turn token", "other variables are not read"] {
        holds(UNIT, "openingOf", name, d::opening_of);
    }
    assert!(recorded("openingOf", "a workspace, the turn token alone").value.expect("a frame").get("turnToken").is_some());
    for name in ["an empty turn token is no token", "the host token is not the turn token", "other variables are not read"] {
        assert!(recorded("openingOf", name).value.expect("a frame").get("turnToken").is_none(), "{name}");
    }
}

/// The caps are checked against what the files weigh before any file is read whole: a file far over the cap is refused
/// by its weight, in the first thing wrong, and no read of it is needed to say so.
#[test]
fn caps_are_checked_before_any_file_is_read_whole() {
    for name in ["a file one byte over the cap is refused with its weight", "a file far over the cap", "the over-cap file is refused before the empty one behind it", "the empty file is refused before the over-cap one behind it", "six files are one too many", "six files with one empty: the count is the first thing wrong"] {
        holds(UNIT, "filesFrom", name, d::files_from);
    }
}

/// Every refusal these helpers make in their own words is a usage refusal (exit class 3), as the TS stamps it.
#[test]
fn own_refusals_are_usage_refusals() {
    let own = ["Put it in quotes after the flags.", "Name a file that is already here.", "Drop that one and send the rest.", "Name one of those with --agent.", "Name one with --at 1 for the thread's first."];
    let mut seen = BTreeSet::new();
    for c in cases(UNIT) {
        let Some(e) = &c.error else { continue };
        if own.iter().any(|f| e.message.ends_with(f)) {
            assert_eq!(e.kind.as_deref(), Some("usage"), "{} / {}", c.func, c.case);
            seen.insert(c.func.clone());
        }
    }
    assert!(seen.len() >= 3, "{seen:?}");
}

/// The TS tests that import these helpers, each as the recorded case that holds its intent (the case names are the
/// fixture's: a rename there breaks this list).
#[test]
fn ported_ts_tests_are_recorded_cases() {
    let ported: [(&str, &str, &str); 14] = [
        ("resume-flags.test.ts: reads the conversation and whether it runs on a copy (1)", "resumeAsked", "a conversation"),
        ("resume-flags.test.ts: reads the conversation and whether it runs on a copy (2)", "resumeAsked", "a conversation to copy"),
        ("resume-flags.test.ts: reads the conversation and whether it runs on a copy (3)", "resumeAsked", "no conversation, no copy, where named"),
        ("resume-flags.test.ts: is a usage refusal beside --branch", "resumeAsked", "a conversation beside a branch"),
        ("resume-flags.test.ts: is a usage refusal beside --cwd", "resumeAsked", "a conversation beside a folder"),
        ("resume-flags.test.ts: is a usage refusal beside --beside", "resumeAsked", "a conversation beside a thread"),
        ("resume-flags.test.ts: refuses --copy with no conversation to copy", "resumeAsked", "a copy with no conversation"),
        ("verbs.test.ts messageTo: names the thread by its runtime id (1)", "messageTo", "names the thread by its runtime id"),
        ("verbs.test.ts messageTo: names the thread by its runtime id (2)", "messageTo", "a row with a thread id and no session"),
        ("verbs.test.ts messageTo: a row with none by its own id", "messageTo", "a row with no thread id is named by its own id"),
        ("parity.test.ts: a --notify typed once reaches the start as an array", "openingOf", "a notify list of one"),
        ("parity.test.ts: a --notify typed twice reaches the start as an array", "openingOf", "a notify list keeps its order and its repeats"),
        ("verbs-turns.test.ts: a bad --file makes no machine (a fork reads its files first)", "openingOf", "a fork with a file that is not there"),
        ("verbs-turns.test.ts: a bad --model makes no machine (a project's fork reads the project's own agent)", "checkedStart", "a project's fork with its agent's model refused"),
    ];
    for (ts, func, name) in ported {
        let c = recorded(func, name);
        assert_eq!(c.func, func, "{ts}");
    }
    for (_, func, name) in ported {
        match func {
            "resumeAsked" => holds(UNIT, func, name, d::resume_asked),
            "messageTo" => holds(UNIT, func, name, d::message_to),
            "openingOf" => holds(UNIT, func, name, d::opening_of),
            "checkedStart" => holds(UNIT, func, name, d::checked_start),
            other => panic!("no driver for {other}"),
        }
    }
}
