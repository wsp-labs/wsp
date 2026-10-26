// SPDX-License-Identifier: AGPL-3.0-only
//! Hand-written hard tests of the `ws_reads` unit (`spec/ws-reads.md`): the host invariant and the folder rule, the
//! TS tests that name these helpers, `pick_thread` over rows with no host, and `pipe_bytes` over a loopback listener.
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;
use wsp_cli::verbs::client::IoBytes;
use wsp_cli::verbs::workspaces_help as w;
use wsp_wire::views::session::ThreadView;
use wsp_words::exit::Kind;

use super::common::{cases, holds, Driver};
use super::drivers_ws_reads as d;

const UNIT: &str = "ws_reads";

fn driver_of(func: &str) -> Driver {
    match func {
        "workspaces" => d::workspaces,
        "workspaceStatuses" => d::workspace_statuses,
        "threads" => d::threads,
        "threadOf" => d::thread_of,
        "threadsOf" => d::threads_of,
        "workspaceOf" => d::workspace_of,
        "threadRows" => d::thread_rows,
        "stateLine" => d::state_line,
        "rebuiltLine" => d::rebuilt_line,
        "wokeLine" => d::woke_line,
        "nap" => d::nap,
        "resize" => d::resize,
        "rebuild" => d::rebuild,
        "imageView" => d::image_view,
        "imageLines" => d::image_lines,
        "buildImageAt" => d::build_image_at,
        "const_host_side_vault" => d::const_host_side_vault,
        "const_ssh_pipes_here_line" => d::const_ssh_pipes_here_line,
        "sshWorkspaceOf" => d::ssh_workspace_of,
        other => panic!("no driver for {other}"),
    }
}

fn run(func: &str, name: &str) {
    holds(UNIT, func, name, driver_of(func));
}

/// The lines a recorded case says: what it said and printed, the words of what it refused with, a string it returned.
fn words_of(case: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for key in ["said", "stdout", "stderr"] {
        out.extend(case[key].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(String::from)));
    }
    out.extend(case["streamed"].as_str().map(String::from));
    out.extend(case["error"]["message"].as_str().map(String::from));
    out.extend(case["error"]["fix"].as_str().map(String::from));
    match &case["value"] {
        Value::String(s) => out.push(s.clone()),
        Value::Array(a) => out.extend(a.iter().filter_map(|v| v.as_str().map(String::from))),
        _ => {}
    }
    out
}

fn recorded() -> Vec<Value> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cli_c/ws_reads.json");
    let all: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("fixture parses");
    all["cases"].as_array().expect("cases").clone()
}

/// Host invariant 7 (`packages/host/AGENTS.md`): with the cloud off no line names a cloud, and no word of this unit
/// moves with the setting. Each case recorded with the cloud on is run beside its twin recorded with it off: both
/// must come out of the one Rust helper (it reads no cloud setting), and the words are the same, none naming Solari.
#[test]
fn invariant_7_the_cloud_setting_moves_no_word_of_ws_reads() {
    let all = recorded();
    let twins: Vec<&Value> = all.iter().filter(|c| c["cloud"] == json!(true)).collect();
    assert!(twins.len() >= 10, "the unit records a cloud twin for a first case and every refusal");
    for on in twins {
        let name = on["case"].as_str().unwrap();
        let base = name.strip_suffix(" (cloud)").expect("a cloud twin is named by its twin");
        let off = all
            .iter()
            .find(|c| c["fn"] == on["fn"] && c["case"] == base && c["cloud"] != json!(true))
            .unwrap_or_else(|| panic!("no twin of {name}"));
        for key in ["frames", "said", "stdout", "stderr", "streamed", "value", "error", "hung"] {
            assert_eq!(on[key], off[key], "{} / {name}: {key} moves with the cloud setting", on["fn"]);
        }
        let func = on["fn"].as_str().unwrap();
        run(func, name);
        run(func, base);
    }
    for case in &all {
        for line in words_of(case) {
            assert!(!line.contains("Solari"), "{} / {}: a line names a cloud: {line}", case["fn"], case["case"]);
        }
    }
}

/// The folder rule: a workspace that is not on this computer has no folder read here, whatever a folder of that name
/// holds on this disk. Both cases leave a HEAD file at the thread's folder that would answer another branch.
#[test]
fn folder_rule_a_workspace_off_this_computer_has_no_folder_read() {
    run("threadRows", "a cloud workspace reads no git here and takes the record");
    run("threadRows", "a place workspace never reads a folder here, though one stands there");
    run("threadRows", "a workspace of no kind is a cloud's");
}

/// `boat-add.test.ts`: "names Boat, never box, on every stage line wsp image build boat streams".
#[test]
fn ts_boat_add_names_boat_never_box_on_every_stage_line() {
    let name = "no image after a build of boat-add's shape, an empty build record";
    run("buildImageAt", name);
    let all = recorded();
    let case = all.iter().find(|c| c["fn"] == "buildImageAt" && c["case"] == name).unwrap();
    assert_eq!(case["streamed"], json!("Boat: Creating the machine\n"));
    assert_eq!(case["error"]["message"], json!("no image yet"));
}

/// `buildImageAt` stops following the frames when it returns: a stage frame heard after the reply streams nothing.
#[test]
fn build_image_at_streams_nothing_after_it_returns() {
    let name = "a stage frame heard after the build returned is not streamed";
    run("buildImageAt", name);
    let all = recorded();
    let case = all.iter().find(|c| c["fn"] == "buildImageAt" && c["case"] == name).unwrap();
    assert!(case.get("streamed").is_none(), "the TS streamed {:?}", case["streamed"]);
}

/// `pause-line.test.ts`: the state line of a Boat sub-thread of a project on a cloud that keeps the memory, from the
/// phase of the record and from the listing, in the pause mode of its own computer.
#[test]
fn ts_pause_line_a_disk_pause_reads_stopped_and_a_memory_pause_paused() {
    run("wokeLine", "a disk pause, read as stopped (the listing says asleep)");
    run("wokeLine", "a pause that keeps memory, read as paused");
    run("stateLine", "phase napping on a disk pause");
    run("stateLine", "phase napping on a memory pause");
    run("stateLine", "phase pausing on a memory pause");
}

/// The rule that a wake settles on the listing's word: the line says what the next `wsp workspaces` prints.
#[test]
fn woke_line_is_read_off_the_listing_not_the_phase_it_wrote() {
    run("wokeLine", "the listing reads the machine up and not answering");
    run("wokeLine", "the listing's phase is what it is read by, not the record's");
    run("wokeLine", "the workspace is not in the listing");
}

/// The refusal sentences of `rebuild` are plain errors (no kind stamped): the exit class is the provider's.
#[test]
fn rebuild_refusals_carry_no_kind() {
    for name in
        ["running and reachable, refused", "unreachable is not gone, refused", "a released workspace", "napping on a memory pause, refused"]
    {
        run("rebuild", name);
    }
}

/// `sshWorkspaceOf`: an alias two workspaces share is a usage refusal listing both, with a fix; asked of the list only.
#[test]
fn ssh_alias_shared_by_two_workspaces_is_a_usage_refusal_with_its_fix() {
    run("sshWorkspaceOf", "an alias of two workspaces");
    run("sshWorkspaceOf", "an alias of three workspaces");
    run("sshWorkspaceOf", "a name that is not an alias asks no list");
}

fn rows(ids: &[(&str, Option<&str>)]) -> Vec<ThreadView> {
    ids.iter()
        .map(|(id, thread)| {
            let mut row = json!({ "id": id, "workspaceId": "ws-1", "harness": "claude", "startedBy": "person", "status": "completed", "title": "t", "sessionId": "s-1", "turns": 1, "ran": false });
            if let Some(thread) = thread {
                row["threadId"] = json!(thread);
            }
            serde_json::from_value(row).expect("a thread row reads")
        })
        .collect()
}

/// `pickThread` (workspaces-help.ts:138), a private function the recorder cannot call: the exact id wins whole, a
/// prefix naming one is that one, the empty word is a prefix of all, and the two refusals are told apart by kind.
#[test]
fn pick_thread_exact_prefix_and_the_two_refusals() {
    let all = rows(&[("t-1", None), ("t-12", None), ("t-123", None), ("u-9", None)]);
    assert_eq!(w::pick_thread(&all, "t-1").unwrap().id, "t-1");
    assert_eq!(w::pick_thread(&all, "t-12").unwrap().id, "t-12");
    assert_eq!(w::pick_thread(&all, "u").unwrap().id, "u-9");
    let two = w::pick_thread(&all, "t-").unwrap_err();
    assert_eq!(two.message, "3 threads start with t-; give more of the id");
    assert!(two.kind.is_none(), "an ambiguous prefix is a plain error");
    let none = w::pick_thread(&all, "zzz").unwrap_err();
    assert_eq!(none.message, "no thread zzz");
    assert_eq!(none.kind, Some(Kind::NotFound));
    let empty = w::pick_thread(&all, "").unwrap_err();
    assert_eq!(empty.message, "4 threads start with ; give more of the id");
    assert_eq!(w::pick_thread(&rows(&[("only", None)]), "").unwrap().id, "only");
    assert_eq!(w::pick_thread(&[], "x").unwrap_err().message, "no thread x");
}

/// The id a ref is matched against is the thread's key, the thread id where a row has one, never the row id.
#[test]
fn pick_thread_matches_the_thread_key_not_the_session_row() {
    let all = rows(&[("thread-aaaa", Some("thread-aaaa")), ("row-1", None)]);
    assert_eq!(w::pick_thread(&all, "row-").unwrap().id, "row-1");
    assert_eq!(w::pick_thread(&all, "thread-a").unwrap().id, "thread-aaaa");
}

#[derive(Clone, Default)]
struct Kept {
    bytes: Arc<Mutex<Vec<u8>>>,
    shut: Arc<AtomicBool>,
}

impl AsyncWrite for Kept {
    fn poll_write(self: Pin<&mut Self>, _cx: &mut Context<'_>, buf: &[u8]) -> Poll<std::io::Result<usize>> {
        self.bytes.lock().unwrap().extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.shut.store(true, Ordering::SeqCst);
        Poll::Ready(Ok(()))
    }
}

/// `pipeBytes` (workspaces-help.ts:296): the input goes to the socket until it ends, the socket's bytes go to the
/// output until the socket closes, the output is never ended, and the call settles when the socket closes. Verified
/// against node's `net` with the same listener: `echo:hello`, output not ended.
#[tokio::test]
async fn pipe_bytes_both_ways_until_the_socket_closes_and_never_ends_the_output() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback listener");
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("a client");
        let mut got = Vec::new();
        socket.read_to_end(&mut got).await.expect("the client's bytes");
        socket.write_all(b"echo:").await.unwrap();
        socket.write_all(&got).await.unwrap();
        socket.shutdown().await.unwrap();
    });
    let kept = Kept::default();
    let bytes = IoBytes { input: Box::new(std::io::Cursor::new(b"hello".to_vec())), output: Box::new(kept.clone()) };
    w::pipe_bytes(port, bytes).await.expect("it settles when the socket closes");
    server.await.unwrap();
    assert_eq!(kept.bytes.lock().unwrap().as_slice(), b"echo:hello");
    assert!(!kept.shut.load(Ordering::SeqCst), "the output must not be ended");
}

/// `pipeBytes` with nothing listening: the connect error fails the call (node: ECONNREFUSED; the words are the OS's).
#[tokio::test]
async fn pipe_bytes_fails_where_nothing_listens() {
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap().port()
    };
    let kept = Kept::default();
    let bytes = IoBytes { input: Box::new(std::io::Cursor::new(Vec::new())), output: Box::new(kept.clone()) };
    assert!(w::pipe_bytes(port, bytes).await.is_err());
    assert!(kept.bytes.lock().unwrap().is_empty());
}

/// The recorded cases the unit holds are all present: a guard that a re-record that drops a case is seen.
#[test]
fn the_recording_holds_every_helper_of_the_unit() {
    let all = cases(UNIT);
    for func in [
        "workspaces",
        "workspaceStatuses",
        "threads",
        "threadOf",
        "threadsOf",
        "workspaceOf",
        "threadRows",
        "stateLine",
        "rebuiltLine",
        "wokeLine",
        "nap",
        "resize",
        "rebuild",
        "imageView",
        "imageLines",
        "buildImageAt",
        "const_host_side_vault",
        "const_ssh_pipes_here_line",
        "sshWorkspaceOf",
    ] {
        assert!(all.iter().any(|c| c.func == func), "no recorded case of {func}");
    }
}

/// The folded key order of a thread row: `foldThreads` writes `asking` and the fields after it before `turns` and
/// `ran`, which the wire struct's field order does not. Recorded in `ws_reads` as `threads / the key order of a folded
/// row with asking and a cost`; named here so it is read as a rule and not a quirk of one case.
#[test]
fn fold_keeps_the_ts_key_order_of_a_row_with_asking_and_cost() {
    let all = cases(UNIT);
    assert!(all.iter().any(|c| c.func == "threads" && c.case == "the key order of a folded row with asking and a cost"));
    holds("ws_reads", "threads", "the key order of a folded row with asking and a cost", d::threads);
}
