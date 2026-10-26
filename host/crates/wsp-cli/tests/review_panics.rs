// SPDX-License-Identifier: AGPL-3.0-only
// The #2449 review, check 5: panics an argument, a host answer, a file or the process can reach. Each test names the
// node behaviour it is held to and fails today. Anything that could wait for ever runs on its own thread and is given
// ten seconds; what ends the process (a stack overflow, a lowered descriptor limit, a latin-1 variable or argument) runs
// in a child process of this very binary, started with PANICS_CHILD set.
#![allow(dead_code)]

#[path = "cli_b/common.rs"]
mod common;

use std::cell::RefCell;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use common::{Case, Held, ScriptedHost};
use futures::{SinkExt, StreamExt};
use indexmap::IndexMap;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use wsp_cli::hosts::HostAim;
use wsp_cli::verbs::client::{dial_host, has_tool, DialOpts, Frame, VerbDeps};
use wsp_cli::verbs::verbs_for;

/// A request for more than 4 GiB panics where it is made, so a test that asks for it fails and the run goes on; without
/// this the allocator aborts the whole test binary.
struct Guard;

unsafe impl std::alloc::GlobalAlloc for Guard {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        if l.size() > (1usize << 32) {
            panic!("huge allocation of {} bytes", l.size());
        }
        std::alloc::System.alloc(l)
    }
    unsafe fn alloc_zeroed(&self, l: std::alloc::Layout) -> *mut u8 {
        if l.size() > (1usize << 32) {
            panic!("huge allocation of {} bytes", l.size());
        }
        std::alloc::System.alloc_zeroed(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        std::alloc::System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, n: usize) -> *mut u8 {
        if n > (1usize << 32) {
            panic!("huge allocation of {} bytes", n);
        }
        std::alloc::System.realloc(p, l, n)
    }
}

#[global_allocator]
static GUARD: Guard = Guard;

thread_local! {
    static LAST: RefCell<Option<(String, String)>> = const { RefCell::new(None) };
}

fn install_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let at = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
            let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = info.payload().downcast_ref::<String>() {
                s.clone()
            } else {
                String::new()
            };
            let at = if msg.starts_with("huge allocation") {
                let bt = std::backtrace::Backtrace::force_capture().to_string();
                let frame = bt.lines().find(|l| l.contains("wsp_") && !l.contains("review_panics")).unwrap_or("?").trim().to_string();
                format!("alloc in {frame}")
            } else {
                at
            };
            LAST.with(|l| *l.borrow_mut() = Some((at, msg)));
            previous(info);
        }));
    });
}

const BOUND: Duration = Duration::from_secs(10);

/// Runs `f` on its own thread; a panic comes back as its message and a hang as a failure in ten seconds.
fn bounded<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    install_hook();
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            LAST.with(|l| *l.borrow_mut() = None);
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
            let _ = tx.send(r.map_err(|_| LAST.with(|l| l.borrow().clone()).map(|(a, m)| format!("{a}: {m}")).unwrap_or_else(|| "a panic".into())));
        })
        .unwrap();
    rx.recv_timeout(BOUND).unwrap_or_else(|_| Err("no answer in ten seconds".into()))
}

/// One tool call through the verb table against a scripted host.
fn call_tool(tool: &'static str, args: Value, replies: Vec<(&'static str, String)>) -> Result<Result<wsp_cli::verbs::client::CallToolResult, wsp_words::exit::HostError>, String> {
    bounded(move || {
        let table = verbs_for(false);
        let entry = table.iter().find(|v| v.name().replace(' ', "_") == tool).expect("a verb for the tool");
        let t = has_tool(entry).expect("a tool");
        let host = ScriptedHost::new(replies.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), IndexMap::new());
        let deps = VerbDeps {
            state_path: "/state/state.json".into(),
            cwd: None,
            env: IndexMap::new(),
            dialer: Arc::new(Held(host)),
            host_wait_ms: None,
            start: None,
            signals: None,
            dial: None,
            elsewhere: false,
            terminal: None,
            open: None,
        };
        let a = args.as_object().cloned().unwrap_or_default();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let call = t.call;
        rt.block_on(call(a, &deps))
    })
}

fn activity_with_depth(depth: &str) -> Result<Result<wsp_cli::verbs::client::CallToolResult, wsp_words::exit::HostError>, String> {
    install_hook();
    let proc_ = format!(
        r#"{{"pid":5,"ppid":1,"depth":{depth},"startedAt":1700000000000,"kind":"tool","name":"sh","command":"sh -c x","memory":1024,"cpu":1.5,"ports":[]}}"#
    );
    let view = format!(r#"{{"computer":"here","at":1760000010000,"threads":[],"wsp":[],"unowned":[{proc_}]}}"#);
    let reply = format!(r#"{{"id":1,"ok":true,"activity":{view}}}"#);
    call_tool("activity", serde_json::json!({}), vec![("activity.read", reply)])
}

/// activity.ts:29 reads the reply through `ActivityView.parse`, whose `depth` is `z.number().int().nonnegative()`
/// (protocol views/activity.ts:27): node throws a ZodError (`too_small`, minimum 0) for -1. The Rust tool reads it with
/// serde alone, takes the -1 and panics on `p.depth.get() as usize + 1` (activity.rs:68, attempt to add with overflow).
#[test]
fn activity_tool_refuses_a_process_whose_depth_is_below_zero_instead_of_panicking() {
    let got = activity_with_depth("-1");
    let answered = got.unwrap_or_else(|p| panic!("the activity tool panicked: {p}"));
    assert!(answered.is_err(), "node refuses the reply (ZodError); got {:?}", answered.map(|r| r.is_error));
}

/// activity.ts:39 `"  ".repeat(p.depth + 1)` with a depth of 4294967296 (a valid integer): node throws RangeError
/// "Invalid string length" and the call fails. The Rust `"  ".repeat(depth as usize + 1)` asks the allocator for 8 GiB
/// (a depth of 2^53 asks for 2^54 bytes and aborts the process with no message).
#[test]
fn activity_tool_fails_a_process_whose_depth_would_make_a_line_of_gigabytes() {
    let got = activity_with_depth("4294967296");
    let answered = got.unwrap_or_else(|p| panic!("the activity tool panicked: {p}"));
    assert!(answered.is_err(), "node throws RangeError: Invalid string length; got an answer");
}

// ---------------------------------------------------------------- the socket

/// A loopback host: every frame answered by `answer(frame) -> texts to send back`.
async fn ws_host(answer: Arc<dyn Fn(&Value) -> Vec<String> + Send + Sync>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((tcp, _)) = listener.accept().await {
            let answer = answer.clone();
            tokio::spawn(async move {
                let Ok(mut ws) = tokio_tungstenite::accept_async(tcp).await else { return };
                while let Some(Ok(message)) = ws.next().await {
                    if let Message::Text(text) = message {
                        let frame: Value = serde_json::from_str(&text).unwrap();
                        for out in answer(&frame) {
                            let _ = ws.send(Message::text(out)).await;
                        }
                    }
                }
            });
        }
    });
    port
}

fn url_aim(port: u16) -> HostAim {
    HostAim::Url { url: format!("http://127.0.0.1:{port}"), token: Some("tok".into()), host_key: None }
}

fn dial_opts(aim: HostAim) -> DialOpts {
    DialOpts { aim: Some(aim), deadline_ms: Some(2_000), ..DialOpts::default() }
}

/// client.ts:238 reads every frame with `JSON.parse`, which has no depth limit: a reply nested 200 levels deep (a
/// slate value, a tool's input in a transcript) is read and answers its request. The Rust socket reads a frame with
/// serde_json, which refuses past 128 levels, and `receive` (socket.rs:406-415) drops a frame it cannot read when no seal
/// is open: the request is never answered and waits until the host closes.
#[test]
fn a_reply_nested_deeper_than_serde_json_reads_still_answers_its_request() {
    let deep = format!("{}1{}", "[".repeat(200), "]".repeat(200));
    let outcome = bounded(move || {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async move {
            let answer: Arc<dyn Fn(&Value) -> Vec<String> + Send + Sync> = Arc::new(move |frame| {
                let id = &frame["id"];
                if frame["op"] == "slates.read" {
                    vec![format!(r#"{{"id":{id},"ok":true,"values":{deep}}}"#)]
                } else {
                    vec![format!(r#"{{"id":{id},"ok":true}}"#)]
                }
            });
            let port = ws_host(answer).await;
            let client = dial_host("/nowhere/state.json", dial_opts(url_aim(port))).await.expect("dialled");
            tokio::time::timeout(Duration::from_secs(3), client.request("slates.read", Frame::new())).await
        })
    });
    let waited = outcome.unwrap_or_else(|p| panic!("{p}"));
    let answered = waited.unwrap_or_else(|_| panic!("the request was never answered: the frame nested 200 levels was dropped"));
    assert!(answered.is_ok(), "{:?}", answered.err());
}

// ---------------------------------------------------------------- the process

/// Runs this very binary again on one test with `PANICS_CHILD` set, so what it does to its environment and argv is its
/// own. A child that has not ended in twenty seconds is killed.
fn child(name: &str, configure: impl FnOnce(&mut std::process::Command)) -> (bool, String) {
    let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
    cmd.args([name, "--exact", "--nocapture", "--test-threads=1"]).env("PANICS_CHILD", "1");
    configure(&mut cmd);
    let mut spawned = cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();
    let started = std::time::Instant::now();
    loop {
        if let Some(status) = spawned.try_wait().unwrap() {
            let mut text = String::new();
            for mut pipe in [spawned.stdout.take().map(|p| Box::new(p) as Box<dyn std::io::Read>), spawned.stderr.take().map(|p| Box::new(p) as Box<dyn std::io::Read>)].into_iter().flatten() {
                let _ = std::io::Read::read_to_string(&mut pipe, &mut text);
            }
            return (status.success(), text);
        }
        if started.elapsed() > Duration::from_secs(20) {
            let _ = spawned.kill();
            return (false, "the child did not end in twenty seconds".into());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn panic_lines(text: &str) -> String {
    text.lines().filter(|l| l.contains("panicked") || l.contains("Err") || l.contains("called")).take(3).collect::<Vec<_>>().join(" | ")
}

/// node reads a variable that is not UTF-8 lossily into `process.env` (`caf�`) and every line goes on. `aimed_host`
/// with no environment handed in reads `std::env::vars()` (hosts.rs:86), which panics on the first such variable; so do
/// `process_env` of the dial (socket.rs:647) and the sign-in relay's `paint_on` (signin_relay.rs:797).
#[test]
fn a_variable_that_is_not_utf8_does_not_stop_a_host_from_being_aimed() {
    let home = tempfile::tempdir().unwrap();
    let (ok, text) = child("env_child", |cmd| {
        cmd.env("WSP_HOME", home.path()).env("WSP_HOST", "");
        cmd.env(OsStr::from_bytes(b"WSP_REVIEW_LATIN1"), OsStr::from_bytes(b"caf\xe9"));
    });
    assert!(ok, "aiming a host with a latin-1 variable in the environment panicked: {}", panic_lines(&text));
}

#[test]
fn env_child() {
    if std::env::var_os("PANICS_CHILD").is_none() {
        return;
    }
    let aim = wsp_cli::hosts::aimed_host("/nowhere/state.json", &wsp_cli::hosts::HostPick::default());
    assert!(aim.is_ok(), "{:?}", aim.err());
}

/// socket.rs:629 reads `std::env::args()` for every release mismatch, which panics when an argument is not UTF-8. node's
/// `process.argv` holds `a�b` and, for a host older than this line, is never read at all (client.ts:1038 reads it only
/// when the host is newer): the refusal is the same sentence either way.
#[test]
fn an_argument_that_is_not_utf8_does_not_hide_the_release_refusal() {
    let (ok, text) = child("argv_child", |_| {});
    assert!(ok, "a release refusal with a latin-1 argument panicked: {}", panic_lines(&text));
}

#[test]
fn argv_child() {
    if std::env::var_os("PANICS_CHILD").is_none() {
        return;
    }
    // The last argument stands at environ[-2]: argv, a null, then the environment.
    unsafe {
        let env = libc::environ;
        let last = *env.offset(-2);
        let seen = std::ffi::CStr::from_ptr(last).to_bytes().to_vec();
        assert_eq!(std::env::args_os().last().unwrap().as_bytes(), &seen[..], "the last argument was not where argv ends");
        *last.add(2) = 0xFFu8 as libc::c_char;
    }
    assert!(std::env::args_os().last().unwrap().to_str().is_none(), "the argument still reads as UTF-8");
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let answer: Arc<dyn Fn(&Value) -> Vec<String> + Send + Sync> =
            Arc::new(|frame| vec![format!(r#"{{"id":{},"ok":true,"version":"0.0.1","road":"up"}}"#, frame["id"])]);
        let port = ws_host(answer).await;
        let refused = dial_host("/nowhere/state.json", dial_opts(url_aim(port))).await.err().expect("an older host is refused");
        assert_eq!(
            refused.message,
            format!(
                "this line runs wsp {} and the host at http://127.0.0.1:{port} runs wsp 0.0.1: Ctrl-C the terminal running wsp up and run it again.",
                wsp_cli::version::VERSION
            )
        );
    });
}

// ---------------------------------------------------------------- a value nested past 128 levels

fn slate_line(argv: &[&str], files: &[(&str, String)], reply: &str, op: &'static str) -> Result<common::Run, String> {
    install_hook();
    let argv: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
    let files: IndexMap<String, String> = files.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    let reply = reply.to_string();
    bounded(move || {
        let c = Case {
            tool: None,
            case: "deep".into(),
            hand: true,
            argv,
            tty: None,
            cloud: false,
            env: IndexMap::new(),
            files,
            replies: [(op.to_string(), reply)].into_iter().collect(),
            pushed: IndexMap::new(),
            tool_arguments: None,
            tool_asked: None,
            tool_result: None,
            text: Default::default(),
            json: Default::default(),
        };
        common::run_line(&c, false)
    })
}

/// node's `JSON.parse` reads `$deep=[[[...1...]]]` 200 levels deep and sends it whole (slate.ts:72-77; probe: depthSent 200,
/// last 1). wsp-words `v8_json` keeps a container past `KEPT_DEPTH` (128) as an empty one, so the Rust line sends
/// `[]` at the 129th level and the 1 is gone: a value is changed on its way to the host, with no word said.
#[test]
fn slate_state_sends_a_value_nested_200_levels_deep_whole() {
    let deep = format!("{}1{}", "[".repeat(200), "]".repeat(200));
    let run = slate_line(
        &["slate", "state", &format!("$deep={deep}")],
        &[],
        r#"{"id":1,"ok":true,"version":8,"text":"slate v8"}"#,
        "slates.state",
    )
    .unwrap_or_else(|p| panic!("{p}"));
    assert_eq!(run.frames, vec![format!(r#"{{"values":{{"$deep":{deep}}},"id":2,"op":"slates.state"}}"#)]);
}

/// The same for a `.json` slate document (slate.ts:42-62): node sends the document whole.
#[test]
fn slate_write_sends_a_json_document_nested_200_levels_deep_whole() {
    let deep = format!("{}1{}", "[".repeat(200), "]".repeat(200));
    let doc = format!(r#"{{"schema":2,"root":"r","pieces":{{"r":{{"type":"text","x":{deep}}}}}}}"#);
    let run = slate_line(&["slate", "write", "d.json"], &[("d.json", doc.clone())], r#"{"id":1,"ok":true,"version":1,"text":"slate v1"}"#, "slates.write")
        .unwrap_or_else(|p| panic!("{p}"));
    assert_eq!(run.frames, vec![format!(r#"{{"document":{doc},"id":2,"op":"slates.write"}}"#)]);
}

// ---------------------------------------------------------------- the process table

/// Two agent processes that name each other as parent. node's `below` (activity-table.ts:52-57) recurses until the stack
/// is gone and the line fails: exit 1, `wsp activity: Maximum call stack size exceeded` on stderr, and under --json
/// `{"error":...,"class":"provider","exit":1}`. Rust's `agent_below` (activity_table.rs:119-132) recurses the same way and
/// the process dies of the overflow, with no line and no exit class. The line runs in a child process.
#[test]
fn a_process_table_where_two_agents_are_each_other_s_parent_fails_the_line_without_ending_the_process() {
    let (ok, text) = child("cycle_child", |_| {});
    assert!(ok, "the line ended the whole process: {}", text.lines().filter(|l| !l.trim().is_empty()).take(3).collect::<Vec<_>>().join(" | "));
}

#[test]
fn cycle_child() {
    if std::env::var_os("PANICS_CHILD").is_none() {
        return;
    }
    let proc_ = |pid: i64, ppid: i64| {
        format!(
            r#"{{"pid":{pid},"ppid":{ppid},"depth":0,"startedAt":1700000000000,"kind":"agent","name":"claude","command":"claude -p","memory":1024,"cpu":1.5,"ports":[]}}"#
        )
    };
    let view = format!(
        r#"{{"computer":"here","at":1760000010000,"wsp":[],"unowned":[],"threads":[{{"threadId":"thread-7f3a9c21e4b5","title":"t","workspaceId":"ws-1","memory":2048,"cpu":3,"processes":[{},{}]}}]}}"#,
        proc_(10, 11),
        proc_(11, 10)
    );
    let reply = format!(r#"{{"id":1,"ok":true,"activity":{view}}}"#);
    for json in [false, true] {
        let c = Case {
            tool: None,
            case: "cycle".into(),
            hand: true,
            argv: vec!["activity".into()],
            tty: None,
            cloud: false,
            env: IndexMap::new(),
            files: IndexMap::new(),
            replies: [("activity.read".to_string(), reply.clone())].into_iter().collect(),
            pushed: IndexMap::new(),
            tool_arguments: None,
            tool_asked: None,
            tool_result: None,
            text: Default::default(),
            json: Default::default(),
        };
        let run = common::run_line(&c, json);
        assert_eq!(run.exit, 1, "json={json}: the failure class is provider (exit 1)");
        assert_eq!(run.stderr.len(), 1, "json={json}: one line on stderr: {:?}", run.stderr);
    }
}

// ---------------------------------------------------------------- the stop signals

/// `ProcessSignals::on` (watch.rs:60-63) asserts that `pipe()` works; with no free descriptor it panics, and the `Once`
/// that held it stays poisoned for every later watch. node's `process.on("SIGINT", ...)` cannot fail.
#[test]
fn a_watch_hangs_its_stop_signals_even_with_no_descriptor_left_for_the_pipe() {
    let (ok, text) = child("watch_child", |_| {});
    assert!(ok, "hanging the stop signals panicked: {}", panic_lines(&text));
}

#[test]
fn watch_child() {
    if std::env::var_os("PANICS_CHILD").is_none() {
        return;
    }
    unsafe {
        let limit = libc::rlimit { rlim_cur: 3, rlim_max: 3 };
        assert_eq!(libc::setrlimit(libc::RLIMIT_NOFILE, &limit), 0);
    }
    use wsp_cli::watch::{ProcessSignals, StopListener, WatchSignals};
    let listener: StopListener = Arc::new(|| {});
    ProcessSignals.on("SIGINT", listener.clone());
    ProcessSignals.off("SIGINT", &listener);
}

/// The same drop for a number past the double range: node's `JSON.parse` reads `1e999` as Infinity and the frame answers
/// its request; serde_json refuses the text, `receive` drops the frame, and the request waits for ever.
#[test]
fn a_reply_holding_a_number_past_the_double_range_still_answers_its_request() {
    let outcome = bounded(move || {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async move {
            let answer: Arc<dyn Fn(&Value) -> Vec<String> + Send + Sync> = Arc::new(move |frame| {
                let id = &frame["id"];
                if frame["op"] == "usage.used" {
                    vec![format!(r#"{{"id":{id},"ok":true,"cost":1e999}}"#)]
                } else {
                    vec![format!(r#"{{"id":{id},"ok":true}}"#)]
                }
            });
            let port = ws_host(answer).await;
            let client = dial_host("/nowhere/state.json", dial_opts(url_aim(port))).await.expect("dialled");
            tokio::time::timeout(Duration::from_secs(3), client.request("usage.used", Frame::new())).await
        })
    });
    let waited = outcome.unwrap_or_else(|p| panic!("{p}"));
    let answered = waited.unwrap_or_else(|_| panic!("the request was never answered: the frame holding 1e999 was dropped"));
    assert!(answered.is_ok(), "{:?}", answered.err());
}

