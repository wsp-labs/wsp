// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/rewind.test.ts at 65f6b89c7, case for case. A checkpoint at every turn's end and a rewind to
//! one: the frames the runtime sends the workspace's own daemon, what it keeps of each turn, what a rewind cuts and in
//! what order, and every refusal made before anything is written. The daemon and the harnesses are fakes, so the
//! runtime's own reading is what is under test. Each test names the TS test above it. The TS `memoryStore()` is the
//! sqlite store here.
mod common;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use common::adapter::{event, result};
use common::*;
use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::port::{
    AdapterError, CountedTurn, ForkTurn, ForkedSession, HarnessAdapter, HarnessSession, HarnessStartOptions, RevertAsk, RevertAt, Reverted,
    SharedFuture, TurnResult,
};
use wsp_core::daemon_channel::{ChannelError, Frame};
use wsp_core::types::api::{RewindAsk, StartOptions};
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_slates::slates::WriteParams;
use wsp_store::store::Store;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_words::exit::{HostError, Kind};
use wsp_words::views::session::fold_threads;
use wsp_words::words::thread::{
    rewind_beside_line, rewind_children_line, rewind_kept_line, CODEX_LEGACY_HISTORY, REWIND_LATEST_LINE, REWIND_NO_CHECKPOINT_LINE,
    REWIND_NO_UNDO_LINE, REWIND_SHARED_LINE, REWIND_WORKING_LINE,
};

const FIRST_SESSION: &str = "44444444-4444-4444-8444-444444444444";
const DAEMON_TOKEN: &str = "cafef00dcafef00dcafef00d";

/// What `fakeDaemon` takes: refuse the checkpoint, answer it with no `based` mark, or be unreachable while `unreached` holds.
#[derive(Default, Clone)]
struct DaemonOpts {
    refuse_checkpoint: bool,
    unbased: bool,
    unreached: Option<Rc<dyn Fn() -> bool>>,
}

/// A daemon that answers each checkpoint with a ref of the turn it names and records every frame.
struct Daemon {
    rec: Rc<RecDaemon>,
    seen: Rc<RefCell<Option<String>>>,
}

fn fake_daemon(o: DaemonOpts) -> Daemon {
    let seen: Rc<RefCell<Option<String>>> = Rc::default();
    let first = seen.clone();
    let text = |f: &Frame, key: &str| f.get(key).and_then(Value::as_str).unwrap_or("undefined").to_string();
    let rec = RecDaemon::new(move |f: &Frame| match f["op"].as_str().unwrap_or_default() {
        "git.checkpoint" => {
            first.borrow_mut().get_or_insert_with(|| text(f, "cwd"));
            if o.refuse_checkpoint {
                return json!({ "id": 1, "ok": false, "code": "not-a-git-repo", "error": "not inside a git repository" });
            }
            json!({
                "id": 1, "ok": true, "ref": format!("refs/wsp/checkpoints/stub-1/{}/{}", text(f, "thread"), text(f, "turn")),
                "commit": format!("c-{}", text(f, "turn")), "changed": true, "based": !o.unbased,
            })
        }
        "git.restore" => json!({ "id": 1, "ok": true, "before": format!("{}-before-1", text(f, "checkpoint")), "files": 2 }),
        other => json!({ "id": 1, "ok": false, "error": format!("no {other} here") }),
    });
    if let Some(unreached) = o.unreached {
        rec.failing_sends(move || unreached().then(|| ChannelError::Refused { message: "Sandbox machine is not running.".into() }));
    }
    Daemon { rec, seen }
}

impl Daemon {
    /// The folder the first checkpoint was asked at: the stub names each checkout it makes by a count of its own.
    fn checkout(&self) -> String {
        self.seen.borrow().clone().unwrap_or_default()
    }
    fn frames(&self) -> Vec<Value> {
        self.rec.frames.borrow().iter().map(|f| Value::Object(f.clone())).collect()
    }
    fn clear(&self) {
        self.rec.frames.borrow_mut().clear();
    }
    fn restores(&self) -> Vec<Value> {
        self.frames().into_iter().filter(|f| f["op"] == "git.restore").collect()
    }
    fn ops(&self) -> Vec<String> {
        self.rec.ops()
    }
}

#[derive(Clone, Copy, Default, PartialEq)]
enum Cuts {
    #[default]
    Copy,
    Revert,
    NoCut,
}

/// `harness(o)`: every turn announces one session, names its end `a<n>`, says `reply <n>` and replies at once.
/// `Copy` rewinds onto a copy of its session through the kept turn's anchor (Claude Code), `Revert` cuts at once (Codex),
/// `NoCut` keeps its own.
#[derive(Clone, Default)]
struct HarnessOpts {
    cuts: Cuts,
    copied: Rc<RefCell<Vec<(String, ForkTurn)>>>,
    reverted: Rc<RefCell<Vec<RevertAsk>>>,
    revert_fails: bool,
    fork_fails: bool,
    revert_keeps: Option<String>,
    anchorless: bool,
    anchors_from: Option<u32>,
    keeps: Option<String>,
    ends: HashMap<u32, Value>,
    starts: Rc<RefCell<Vec<(String, Option<String>)>>>,
    hang_from: Option<u32>,
}

fn harness(o: HarnessOpts) -> HarnessAdapterFactory {
    let turns: Rc<Cell<u32>> = Rc::default();
    Arc::new(move |_ctx| Ok(Arc::new(Cutting { o: o.clone(), turns: turns.clone() }) as Arc<dyn HarnessAdapter>))
}

struct Cutting {
    o: HarnessOpts,
    turns: Rc<Cell<u32>>,
}

impl HarnessAdapter for Cutting {
    fn steers(&self) -> bool {
        false
    }
    fn forks(&self) -> bool {
        self.o.cuts == Cuts::Copy
    }
    fn reverts(&self) -> bool {
        self.o.cuts == Cuts::Revert
    }
    fn fork_session(&self, session: String, turn: ForkTurn) -> Option<LocalBoxFuture<'static, Result<ForkedSession, String>>> {
        if self.o.cuts != Cuts::Copy {
            return None;
        }
        self.o.copied.borrow_mut().push((session, turn));
        let fails = self.o.fork_fails;
        let n = self.o.copied.borrow().len();
        Some(
            async move {
                if fails {
                    return Err("claude would not copy the session".to_string());
                }
                Ok(ForkedSession::Resume {
                    resume: format!("c0c0c0c0-0000-4000-8000-{n:012}"),
                    drop: Box::new(|| async { Ok(()) }.boxed_local()),
                })
            }
            .boxed_local(),
        )
    }
    fn revert(&self, ask: RevertAsk) -> Option<LocalBoxFuture<'static, Result<Reverted, String>>> {
        if self.o.cuts != Cuts::Revert {
            return None;
        }
        self.o.reverted.borrow_mut().push(ask);
        let (fails, keeps) = (self.o.revert_fails, self.o.revert_keeps.clone());
        Some(
            async move {
                if fails {
                    return Err(format!("codex would not cut the thread: timed out shutting down thread {FIRST_SESSION} before revert"));
                }
                Ok(keeps.map_or(Reverted::Cut, Reverted::Kept))
            }
            .boxed_local(),
        )
    }
    fn start(&self, s: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        self.turns.set(self.turns.get() + 1);
        let n = self.turns.get();
        // A fresh session per thread, as a harness mints one, and the same one again on its resume.
        let session = s.resume.clone().unwrap_or_else(|| if n == 1 { FIRST_SESSION.to_string() } else { format!("44444444-4444-4444-8444-{n:012}") });
        self.o.starts.borrow_mut().push((s.prompt.clone(), s.resume.clone()));
        let on_event = s.on_event.clone().expect("an event sink");
        let (tx, rx) = oneshot::channel::<TurnResult>();
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn's end") }.boxed_local();
        let stop: Rc<RefCell<Option<oneshot::Sender<TurnResult>>>> = Rc::new(RefCell::new(None));
        if self.o.hang_from.is_some_and(|from| n >= from) {
            *stop.borrow_mut() = Some(tx);
            let (announced, emit) = (session.clone(), on_event.clone());
            tokio::task::spawn_local(async move {
                tokio::task::yield_now().await;
                emit(event(json!({ "type": "session.start", "sessionId": announced })));
            });
            return Ok(Arc::new(Hanging { local_id: session, finished: finished.shared(), stop, on_event }));
        }
        let done = result(self.o.ends.get(&n).cloned().unwrap_or_else(|| json!({ "status": "completed", "text": format!("reply {n}") })));
        let (anchorless, anchors_from, keeps) = (self.o.anchorless, self.o.anchors_from, self.o.keeps.clone());
        let id = session.clone();
        tokio::task::spawn_local(async move {
            tokio::task::yield_now().await;
            on_event(event(json!({ "type": "session.start", "sessionId": id })));
            on_event(event(json!({ "type": "turn.delta", "sessionId": id, "kind": "text", "text": format!("reply {n}") })));
            if !anchorless || anchors_from.is_some_and(|from| n >= from) {
                let mut anchor = json!({ "type": "turn.anchor", "sessionId": id, "anchor": format!("a{n}") });
                if let Some(kept) = keeps {
                    anchor["kept"] = json!(kept);
                }
                on_event(event(anchor));
            }
            on_event(event(json!({ "type": "turn.done", "sessionId": id, "result": serde_json::to_value(&done).expect("a result") })));
            on_event(event(json!({ "type": "session.end", "sessionId": id, "exitCode": 0, "sawResult": true })));
            let _ = tx.send(done);
        });
        Ok(Arc::new(Hanging { local_id: session, finished: finished.shared(), stop: Rc::default(), on_event: Arc::new(|_| {}) }))
    }
}

/// A turn whose process runs on: only an interrupt ends it. Also the shape a quick turn takes, whose `stop` is empty.
struct Hanging {
    local_id: String,
    finished: SharedFuture<TurnResult>,
    stop: Rc<RefCell<Option<oneshot::Sender<TurnResult>>>>,
    on_event: wsp_adapters::port::OnEvent,
}

impl HarnessSession for Hanging {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        let (session, on_event, stop) = (self.local_id.clone(), self.on_event.clone(), self.stop.clone());
        async move {
            let tx = stop.borrow_mut().take();
            let Some(tx) = tx else { return };
            let interrupted = result(json!({ "status": "interrupted" }));
            on_event(event(json!({ "type": "turn.done", "sessionId": session, "result": serde_json::to_value(&interrupted).expect("a result") })));
            on_event(event(json!({ "type": "session.end", "sessionId": session, "exitCode": null, "sawResult": false })));
            let _ = tx.send(interrupted);
        }
        .boxed_local()
    }
}

struct Ws {
    fx: Fx,
    ws: String,
    daemon: Daemon,
    events: Events,
    backend: Arc<StubBackend>,
}

/// A cloud workspace whose daemon answers, as a fork on a box does, with one harness under the claude id.
async fn workspace(adapter: HarnessAdapterFactory, daemon: Daemon, store: Option<Rc<dyn Store>>) -> Ws {
    let backend = StubBackend::new(None);
    backend.set_exec_impl(token_guest());
    let open = daemon.rec.open();
    let mut wiring = runtime(&backend).adapter("claude", adapter).with(move |o| {
        o.daemon_token = Some(DAEMON_TOKEN.into());
        o.daemon_channel = Some(open);
    });
    let state = sqlite_state();
    wiring = wiring.store(&store.unwrap_or_else(|| state.store.clone()));
    let fx = wiring.build();
    let ws = create_on(&fx.rt, CreateOn::golden("pricing page").agents(2, 2), None).await.expect("a workspace").view.id;
    backend.machine(0).set_preview_url("http://127.0.0.1:7070");
    let events = fx.events();
    std::mem::forget(state);
    Ws { fx, ws, daemon, events, backend }
}

fn start(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

fn on_thread(prompt: &str, thread: &str) -> StartOptions {
    StartOptions { thread: Some(thread.into()), ..start(prompt) }
}

/// The checkpoint is asked after the turn is over and off its road, so a case waits for it to land.
async fn wait() {
    sleep_ms(20).await;
}

/// Three finished turns on one thread; answers the thread id and each turn's id in order.
async fn three_turns(w: &Ws) -> (String, Vec<String>) {
    let first = w.fx.rt.sessions().start(&w.ws, start("one"), None).await.expect("a start");
    first.finished().await;
    let thread_id = first.view().thread_id.expect("a thread");
    let mut turns = vec![first.turn_id()];
    for prompt in ["two", "three"] {
        let next = w.fx.rt.sessions().start(&w.ws, on_thread(prompt, &thread_id), None).await.expect("a start");
        next.finished().await;
        turns.push(next.turn_id());
    }
    wait().await;
    (thread_id, turns)
}

async fn history(w: &Ws) -> Vec<Value> {
    w.fx.rt.sessions().history(&w.ws, None).await.expect("a history").iter().map(j).collect()
}

async fn texts(w: &Ws) -> Vec<String> {
    history(w).await.iter().filter(|e| e["type"] == "session.delta").map(|e| e["text"].as_str().unwrap_or_default().to_string()).collect()
}

async fn checkpoints(w: &Ws) -> Vec<Value> {
    history(w).await.into_iter().filter(|e| e["type"] == "session.checkpoint").collect()
}

async fn thread_of(w: &Ws, thread: &str) -> Value {
    let rows = w.fx.rt.sessions().list(Some(w.ws.clone()), None).await.expect("a list");
    j(&fold_threads(&rows).into_iter().find(|t| t.thread_id.as_deref() == Some(thread)).expect("the thread"))
}

fn rewind_to(turn: &str, files: bool) -> RewindAsk {
    RewindAsk { turn_id: Some(turn.into()), files: Some(files), undo: None }
}

fn undo() -> RewindAsk {
    RewindAsk { turn_id: None, files: None, undo: Some(true) }
}

async fn rewind(w: &Ws, thread: &str, ask: RewindAsk) -> Result<Value, HostError> {
    w.fx.rt.sessions().rewind(thread, ask, None).await.map(|r| j(&r))
}

async fn refusal(w: &Ws, thread: &str, ask: RewindAsk) -> String {
    rewind(w, thread, ask).await.err().expect("a refusal").message
}

fn restore_of(w: &Ws, thread: &str, turn: &str, suffix: &str) -> Value {
    json!({ "op": "git.restore", "cwd": w.daemon.checkout(), "checkpoint": format!("refs/wsp/checkpoints/stub-1/{thread}/{turn}{suffix}"), "scope": w.ws })
}

// "a checkpoint at every turn's end" > "asks the workspace's daemon for one at the checkout, named by the record, the thread and the turn, and keeps it with the anchor"
#[test]
fn asks_the_workspaces_daemon_for_one_at_the_checkout_named_by_the_record_the_thread_and_the_turn_and_keeps_it_with_the_anchor() {
    drive(|| async {
        let w = workspace(harness(HarnessOpts::default()), fake_daemon(DaemonOpts::default()), None).await;
        let run = w.fx.rt.sessions().start(&w.ws, start("one"), None).await.expect("a start");
        run.finished().await;
        wait().await;
        let thread_id = run.view().thread_id.expect("a thread");
        let asked: Vec<Value> = w.daemon.frames().into_iter().filter(|f| f["op"] == "git.checkpoint").collect();
        assert_eq!(asked, [json!({ "op": "git.checkpoint", "cwd": w.daemon.checkout(), "thread": thread_id, "turn": run.turn_id(), "scope": w.ws })]);
        let kept = checkpoints(&w).await;
        assert_eq!(kept.len(), 1);
        assert_matches(
            &kept[0],
            json!({
                "type": "session.checkpoint", "turnId": run.turn_id(), "threadId": thread_id,
                "ref": format!("refs/wsp/checkpoints/stub-1/{thread_id}/{}", run.turn_id()), "based": true, "anchor": "a1",
            }),
        );
        w.fx.rt.close().await.expect("a close");
    });
}

// "a checkpoint at every turn's end" > "keeps no based mark where the checkpoint stands on no commit, so a branch forked from it starts at the folder's HEAD"
#[test]
fn keeps_no_based_mark_where_the_checkpoint_stands_on_no_commit_so_a_branch_forked_from_it_starts_at_the_folders_head() {
    drive(|| async {
        let w = workspace(harness(HarnessOpts::default()), fake_daemon(DaemonOpts { unbased: true, ..Default::default() }), None).await;
        let run = w.fx.rt.sessions().start(&w.ws, start("one"), None).await.expect("a start");
        run.finished().await;
        wait().await;
        let kept = checkpoints(&w).await;
        assert_eq!(kept.len(), 1);
        assert!(kept[0]["ref"].is_string());
        assert_eq!(kept[0]["anchor"], "a1");
        assert!(kept[0].get("based").is_none());
        w.fx.rt.close().await.expect("a close");
    });
}

// "a checkpoint at every turn's end" > "a checkout that takes none leaves the turn as it ended and keeps the anchor alone"
#[test]
fn a_checkout_that_takes_none_leaves_the_turn_as_it_ended_and_keeps_the_anchor_alone() {
    drive(|| async {
        let w = workspace(harness(HarnessOpts::default()), fake_daemon(DaemonOpts { refuse_checkpoint: true, ..Default::default() }), None).await;
        let run = w.fx.rt.sessions().start(&w.ws, start("one"), None).await.expect("a start");
        assert_eq!(j(&run.finished().await)["status"], "completed");
        wait().await;
        assert_eq!(j(&run.view())["status"], "completed");
        let kept = checkpoints(&w).await;
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0]["anchor"], "a1");
        assert!(kept[0].get("ref").is_none());
        w.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "puts the files back, cuts the turns after it from the transcript, says so, and moves the thread onto a copy of its session through the kept turn"
#[test]
fn puts_the_files_back_cuts_the_turns_after_it_from_the_transcript_says_so_and_moves_the_thread_onto_a_copy_of_its_session_through_the_kept_turn(
) {
    drive(|| async {
        let o = HarnessOpts::default();
        let w = workspace(harness(o.clone()), fake_daemon(DaemonOpts::default()), None).await;
        let (thread_id, turns) = three_turns(&w).await;
        w.daemon.clear();
        let done = rewind(&w, &thread_id, rewind_to(&turns[0], true)).await.expect("a rewind");
        assert_eq!(done, json!({ "turns": 2, "files": 2 }));
        assert_eq!(w.daemon.frames(), [restore_of(&w, &thread_id, &turns[0], "")]);
        assert_eq!(texts(&w).await, ["reply 1"]);
        assert!(history(&w).await.iter().all(|e| e["turnId"] == turns[0].as_str()));
        let rewound = w.events.of("thread.rewound");
        assert_eq!(rewound.len(), 1);
        assert_matches(&rewound[0], json!({ "type": "thread.rewound", "workspaceId": w.ws, "threadId": thread_id }));
        assert!(thread_of(&w, &thread_id).await["rewoundAt"].is_number());

        // The copy ends at the kept turn's own end, and every later turn resumes it, never the session the cut turns ran on.
        assert_eq!(*o.copied.borrow(), [(FIRST_SESSION.to_string(), ForkTurn::Anchor("a1".into()))]);
        let copy = "c0c0c0c0-0000-4000-8000-000000000001";
        assert_eq!(thread_of(&w, &thread_id).await["claudeSessionId"], copy);
        w.fx.rt.sessions().start(&w.ws, on_thread("four", &thread_id), None).await.expect("a start").finished().await;
        w.fx.rt.sessions().start(&w.ws, on_thread("five", &thread_id), None).await.expect("a start").finished().await;
        let resumes: Vec<Option<String>> = o.starts.borrow().iter().map(|s| s.1.clone()).collect();
        let first = Some(FIRST_SESSION.to_string());
        assert_eq!(resumes, [None, first.clone(), first, Some(copy.to_string()), Some(copy.to_string())]);
        let prompts: Vec<String> = o.starts.borrow().iter().map(|s| s.0.clone()).collect();
        assert_eq!(prompts, ["one", "two", "three", "four", "five"]);
        wait().await;
        // Undo lasts until the turn after the rewind ends.
        assert!(thread_of(&w, &thread_id).await.get("rewoundAt").is_none());
        w.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "cuts one thread's turns off that thread's rows alone, and another thread in the folder keeps its events and its words"
#[test]
fn cuts_one_threads_turns_off_that_threads_rows_alone_and_another_thread_in_the_folder_keeps_its_events_and_its_words() {
    drive(|| async {
        let state = sqlite_state();
        let watched = Watched::over(&state.store);
        let w = workspace(harness(HarnessOpts::default()), fake_daemon(DaemonOpts::default()), Some(watched.store())).await;
        let (thread_id, turns) = three_turns(&w).await;
        let other = w.fx.rt.sessions().start(&w.ws, start("other"), None).await.expect("a start");
        other.finished().await;
        wait().await;
        let other_id = other.view().thread_id.expect("a thread");
        let thread_history = |thread: String| {
            let (rt, ws) = (w.fx.rt.clone(), w.ws.clone());
            async move {
                rt.sessions()
                    .thread_history(&ws, wsp_core::types::api::HistoryAsk { thread_id: Some(thread), ..Default::default() }, None)
                    .await
                    .expect("a history")
                    .iter()
                    .map(j)
                    .collect::<Vec<Value>>()
            }
        };
        let kept = thread_history(other_id.clone()).await;
        let said = |words: &'static str| {
            let rt = w.fx.rt.clone();
            async move {
                let found = j(&rt.sessions().search(words, None).await.expect("a search"));
                found["hits"].as_array().expect("hits").iter().map(|h| h["threadId"].as_str().unwrap_or_default().to_string()).collect::<Vec<_>>()
            }
        };
        assert_eq!(said("reply 3").await, [thread_id.clone()]);
        watched.rows.folder_reads.set(0);
        rewind(&w, &thread_id, rewind_to(&turns[0], false)).await.expect("a rewind");
        assert_eq!(watched.rows.folder_reads.get(), 0);
        assert!(thread_history(thread_id.clone()).await.iter().all(|e| e["turnId"] == turns[0].as_str()));
        assert_eq!(thread_history(other_id.clone()).await, kept);
        assert_eq!(said("reply 3").await, Vec::<String>::new());
        assert_eq!(said("reply 4").await, [other_id]);
        assert_eq!(said("reply 1").await, [thread_id]);
        w.fx.rt.close().await.expect("a close");
        drop(state);
    });
}

// "rewinding a thread to one of its replies" > "with the conversation alone leaves the files where they are"
#[test]
fn with_the_conversation_alone_leaves_the_files_where_they_are() {
    drive(|| async {
        let w = workspace(harness(HarnessOpts::default()), fake_daemon(DaemonOpts::default()), None).await;
        let (thread_id, turns) = three_turns(&w).await;
        w.daemon.clear();
        assert_eq!(rewind(&w, &thread_id, rewind_to(&turns[1], false)).await.expect("a rewind"), json!({ "turns": 1 }));
        assert!(w.daemon.frames().is_empty());
        assert_eq!(texts(&w).await, ["reply 1", "reply 2"]);
        assert!(thread_of(&w, &thread_id).await.get("rewoundAt").is_none());
        w.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "lists the last line and the failure of the turn that is now the thread's last"
#[test]
fn lists_the_last_line_and_the_failure_of_the_turn_that_is_now_the_threads_last() {
    drive(|| async {
        let ends = HashMap::from([(1, json!({ "status": "failed", "error": "API Error: 529 overloaded\nretry" }))]);
        let w = workspace(harness(HarnessOpts { ends, ..Default::default() }), fake_daemon(DaemonOpts::default()), None).await;
        let (thread_id, turns) = three_turns(&w).await;
        let listed = thread_of(&w, &thread_id).await;
        assert_matches(&listed, json!({ "lastLine": "reply 3" }));
        rewind(&w, &thread_id, rewind_to(&turns[1], false)).await.expect("a rewind");
        let listed = thread_of(&w, &thread_id).await;
        assert_matches(&listed, json!({ "lastLine": "reply 2" }));
        assert!(listed.get("failure").is_none());
        rewind(&w, &thread_id, rewind_to(&turns[0], false)).await.expect("a rewind");
        let listed = thread_of(&w, &thread_id).await;
        assert_matches(&listed, json!({ "failure": "API Error: 529 overloaded" }));
        assert!(listed.get("lastLine").is_none());
        w.fx.rt.close().await.expect("a close");
    });
}

fn revert_ask(cwd: &str, at: RevertAt) -> RevertAsk {
    RevertAsk { session: FIRST_SESSION.into(), cwd: Some(cwd.into()), at }
}

fn counted(anchor: Option<&str>, doc: Value) -> CountedTurn {
    CountedTurn { anchor: anchor.map(str::to_string), result: Some(result(doc)) }
}

fn completed(text: &str) -> Value {
    json!({ "status": "completed", "text": text })
}

// "rewinding a thread to one of its replies" > "cuts a harness that reverts at once before the first turn it drops, after the files"
#[test]
fn cuts_a_harness_that_reverts_at_once_before_the_first_turn_it_drops_after_the_files() {
    drive(|| async {
        let o = HarnessOpts { cuts: Cuts::Revert, ..Default::default() };
        let w = workspace(harness(o.clone()), fake_daemon(DaemonOpts::default()), None).await;
        let (thread_id, turns) = three_turns(&w).await;
        w.daemon.clear();
        rewind(&w, &thread_id, rewind_to(&turns[0], true)).await.expect("a rewind");
        assert_eq!(*o.reverted.borrow(), [revert_ask(&w.daemon.checkout(), RevertAt::BeforeTurn("a2".into()))]);
        assert_eq!(w.daemon.ops(), ["git.restore"]);
        w.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "refused whole when the harness will not cut: the files go back to how they stood and the transcript keeps every turn"
#[test]
fn refused_whole_when_the_harness_will_not_cut_the_files_go_back_to_how_they_stood_and_the_transcript_keeps_every_turn() {
    drive(|| async {
        let o = HarnessOpts { cuts: Cuts::Revert, revert_fails: true, ..Default::default() };
        let w = workspace(harness(o), fake_daemon(DaemonOpts::default()), None).await;
        let (thread_id, turns) = three_turns(&w).await;
        w.daemon.clear();
        let said = refusal(&w, &thread_id, rewind_to(&turns[0], true)).await;
        assert!(said.contains(&format!("codex would not cut the thread: timed out shutting down thread {FIRST_SESSION} before revert")), "{said}");
        assert_eq!(w.daemon.frames(), [restore_of(&w, &thread_id, &turns[0], ""), restore_of(&w, &thread_id, &turns[0], "-before-1")]);
        assert_eq!(texts(&w).await, ["reply 1", "reply 2", "reply 3"]);
        w.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "refused whole when the harness will not copy the session: the files go back and the transcript keeps every turn"
#[test]
fn refused_whole_when_the_harness_will_not_copy_the_session_the_files_go_back_and_the_transcript_keeps_every_turn() {
    drive(|| async {
        let o = HarnessOpts { fork_fails: true, ..Default::default() };
        let w = workspace(harness(o), fake_daemon(DaemonOpts::default()), None).await;
        let (thread_id, turns) = three_turns(&w).await;
        w.daemon.clear();
        let said = refusal(&w, &thread_id, rewind_to(&turns[0], true)).await;
        assert!(said.contains("claude would not copy the session"), "{said}");
        assert_eq!(w.daemon.frames(), [restore_of(&w, &thread_id, &turns[0], ""), restore_of(&w, &thread_id, &turns[0], "-before-1")]);
        assert_eq!(texts(&w).await, ["reply 1", "reply 2", "reply 3"]);
        w.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "a harness whose turns named no anchor is handed how each cut turn ended, and the transcript is cut"
#[test]
fn a_harness_whose_turns_named_no_anchor_is_handed_how_each_cut_turn_ended_and_the_transcript_is_cut() {
    drive(|| async {
        let refused = json!({ "status": "failed", "error": "Codex is not signed in where this thread runs", "refusal": "sign-in" });
        let o = HarnessOpts { cuts: Cuts::Revert, anchorless: true, ends: HashMap::from([(2, refused.clone())]), ..Default::default() };
        let w = workspace(harness(o.clone()), fake_daemon(DaemonOpts::default()), None).await;
        let (thread_id, turns) = three_turns(&w).await;
        assert_eq!(rewind(&w, &thread_id, rewind_to(&turns[0], true)).await.expect("a rewind"), json!({ "turns": 2, "files": 2 }));
        let want = revert_ask(&w.daemon.checkout(), RevertAt::Turns(vec![counted(None, refused), counted(None, completed("reply 3"))]));
        assert_eq!(*o.reverted.borrow(), [want]);
        assert_eq!(texts(&w).await, ["reply 1"]);
        w.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "hands the harness each cut turn's anchor beside its end where the turn kept one"
#[test]
fn hands_the_harness_each_cut_turns_anchor_beside_its_end_where_the_turn_kept_one() {
    drive(|| async {
        let o = HarnessOpts { cuts: Cuts::Revert, anchorless: true, anchors_from: Some(3), ..Default::default() };
        let w = workspace(harness(o.clone()), fake_daemon(DaemonOpts::default()), None).await;
        let (thread_id, turns) = three_turns(&w).await;
        rewind(&w, &thread_id, rewind_to(&turns[0], false)).await.expect("a rewind");
        let want = revert_ask(&w.daemon.checkout(), RevertAt::Turns(vec![counted(None, completed("reply 2")), counted(Some("a3"), completed("reply 3"))]));
        assert_eq!(*o.reverted.borrow(), [want]);
        w.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "a harness that keeps a thread's conversation whole leaves the files back with Undo, keeps every turn, and says why"
#[test]
fn a_harness_that_keeps_a_threads_conversation_whole_leaves_the_files_back_with_undo_keeps_every_turn_and_says_why() {
    drive(|| async {
        let o = HarnessOpts { cuts: Cuts::Revert, revert_keeps: Some(CODEX_LEGACY_HISTORY.into()), ..Default::default() };
        let w = workspace(harness(o), fake_daemon(DaemonOpts::default()), None).await;
        let (thread_id, turns) = three_turns(&w).await;
        w.daemon.clear();
        let said = refusal(&w, &thread_id, rewind_to(&turns[0], true)).await;
        assert!(said.contains(&rewind_kept_line(CODEX_LEGACY_HISTORY, true)), "{said}");
        assert_eq!(
            rewind_kept_line(CODEX_LEGACY_HISTORY, true),
            "a Codex older than 0.151.0 made this thread and keeps its history in a form thread/revert cannot cut; the files went back and every turn stays"
        );
        assert_eq!(w.daemon.ops(), ["git.restore"]);
        assert_eq!(texts(&w).await, ["reply 1", "reply 2", "reply 3"]);
        assert!(thread_of(&w, &thread_id).await["rewoundAt"].is_number());
        assert_eq!(rewind(&w, &thread_id, undo()).await.expect("an undo"), json!({ "turns": 0, "files": 2 }));

        let said = refusal(&w, &thread_id, rewind_to(&turns[0], false)).await;
        assert!(said.contains(&rewind_kept_line(CODEX_LEGACY_HISTORY, false)), "{said}");
        assert_eq!(texts(&w).await, ["reply 1", "reply 2", "reply 3"]);
        w.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "a thread the harness said it cannot cut is rewound in its files alone without asking the harness, and the conversation alone is refused in its words"
#[test]
fn a_thread_the_harness_said_it_cannot_cut_is_rewound_in_its_files_alone_without_asking_the_harness_and_the_conversation_alone_is_refused_in_its_words(
) {
    drive(|| async {
        let o = HarnessOpts { cuts: Cuts::Revert, keeps: Some(CODEX_LEGACY_HISTORY.into()), ..Default::default() };
        let w = workspace(harness(o.clone()), fake_daemon(DaemonOpts::default()), None).await;
        let (thread_id, turns) = three_turns(&w).await;
        let kept: Vec<Value> = checkpoints(&w).await.iter().map(|e| e["kept"].clone()).collect();
        assert_eq!(kept, [json!(CODEX_LEGACY_HISTORY), json!(CODEX_LEGACY_HISTORY), json!(CODEX_LEGACY_HISTORY)]);
        w.daemon.clear();
        let said = refusal(&w, &thread_id, rewind_to(&turns[0], false)).await;
        assert!(said.contains(&rewind_kept_line(CODEX_LEGACY_HISTORY, false)), "{said}");
        assert!(w.daemon.frames().is_empty());
        assert_eq!(rewind(&w, &thread_id, rewind_to(&turns[0], true)).await.expect("a rewind"), json!({ "turns": 0, "files": 2 }));
        assert!(o.reverted.borrow().is_empty());
        assert_eq!(texts(&w).await, ["reply 1", "reply 2", "reply 3"]);
        w.fx.rt.close().await.expect("a close");
    });
}

async fn claude_catalog(w: &Ws) -> Value {
    let list = w.fx.rt.harnesses().list(Some(w.ws.clone()), None, None).await.expect("a list");
    list.iter().map(j).find(|c| c["harness"] == "claude").expect("claude's catalog")
}

// "rewinding a thread to one of its replies" > "tells a window which harness the host rewinds by count, so a reply with no anchor is offered too"
#[test]
fn tells_a_window_which_harness_the_host_rewinds_by_count_so_a_reply_with_no_anchor_is_offered_too() {
    drive(|| async {
        let w = workspace(harness(HarnessOpts { cuts: Cuts::Revert, ..Default::default() }), fake_daemon(DaemonOpts::default()), None).await;
        assert_matches(&claude_catalog(&w).await, json!({ "rewindsConversation": true, "rewindsByCount": true }));
        w.fx.rt.close().await.expect("a close");
        let other = workspace(harness(HarnessOpts::default()), fake_daemon(DaemonOpts::default()), None).await;
        assert!(claude_catalog(&other).await.get("rewindsByCount").is_none());
        other.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "on a harness that keeps its own history moves the files and keeps every turn, and says it cuts no conversation"
#[test]
fn on_a_harness_that_keeps_its_own_history_moves_the_files_and_keeps_every_turn_and_says_it_cuts_no_conversation() {
    drive(|| async {
        let w = workspace(harness(HarnessOpts { cuts: Cuts::NoCut, ..Default::default() }), fake_daemon(DaemonOpts::default()), None).await;
        assert!(!claude_catalog(&w).await["rewindsConversation"].as_bool().unwrap_or(false));
        let (thread_id, turns) = three_turns(&w).await;
        assert_eq!(rewind(&w, &thread_id, rewind_to(&turns[0], true)).await.expect("a rewind"), json!({ "turns": 0, "files": 2 }));
        assert_eq!(texts(&w).await, ["reply 1", "reply 2", "reply 3"]);
        w.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "tells a window whether a harness cuts its conversation"
#[test]
fn tells_a_window_whether_a_harness_cuts_its_conversation() {
    drive(|| async {
        let w = workspace(harness(HarnessOpts::default()), fake_daemon(DaemonOpts::default()), None).await;
        assert_eq!(claude_catalog(&w).await["rewindsConversation"], true);
        w.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "undoes by putting back the files the rewind replaced, once, and says when there is nothing to undo"
#[test]
fn undoes_by_putting_back_the_files_the_rewind_replaced_once_and_says_when_there_is_nothing_to_undo() {
    drive(|| async {
        let w = workspace(harness(HarnessOpts::default()), fake_daemon(DaemonOpts::default()), None).await;
        let (thread_id, turns) = three_turns(&w).await;
        assert!(refusal(&w, &thread_id, undo()).await.contains(REWIND_NO_UNDO_LINE));
        rewind(&w, &thread_id, rewind_to(&turns[0], true)).await.expect("a rewind");
        w.daemon.clear();
        assert_eq!(rewind(&w, &thread_id, undo()).await.expect("an undo"), json!({ "turns": 0, "files": 2 }));
        assert_eq!(w.daemon.frames(), [restore_of(&w, &thread_id, &turns[0], "-before-1")]);
        // The transcript is as the rewind left it: undo brings the files back, not the conversation.
        assert_eq!(texts(&w).await, ["reply 1"]);
        assert!(refusal(&w, &thread_id, undo()).await.contains(REWIND_NO_UNDO_LINE));
        w.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "wakes a napped machine before its files go back, on a rewind and on its undo"
#[test]
fn wakes_a_napped_machine_before_its_files_go_back_on_a_rewind_and_on_its_undo() {
    drive(|| async {
        let machine: Rc<RefCell<Option<Arc<StubMachine>>>> = Rc::default();
        let looking = machine.clone();
        let unreached: Rc<dyn Fn() -> bool> = Rc::new(move || looking.borrow().as_ref().is_some_and(|m| m.paused.get()));
        let w = workspace(harness(HarnessOpts::default()), fake_daemon(DaemonOpts { unreached: Some(unreached), ..Default::default() }), None).await;
        let m = w.backend.machine(0);
        *machine.borrow_mut() = Some(m.clone());
        // The wake's check asks the daemon here rather than at a preview address nothing serves.
        m.set_daemon_answer(true);
        let (thread_id, turns) = three_turns(&w).await;
        w.fx.rt.workspaces().nap(&w.ws, None).await.expect("a nap");
        assert!(m.paused.get());
        assert!(refusal(&w, &thread_id, rewind_to(&turns[2], true)).await.contains(REWIND_LATEST_LINE));
        assert_eq!(m.calls.resume.get(), 0);
        assert_eq!(rewind(&w, &thread_id, rewind_to(&turns[0], true)).await.expect("a rewind"), json!({ "turns": 2, "files": 2 }));
        assert_eq!(m.calls.resume.get(), 1);
        w.fx.rt.workspaces().nap(&w.ws, None).await.expect("a nap");
        assert_eq!(rewind(&w, &thread_id, undo()).await.expect("an undo"), json!({ "turns": 0, "files": 2 }));
        assert_eq!(m.calls.resume.get(), 2);
        w.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "reads the provider when the daemon's road refuses a record that reads running, so a machine stopped behind it wakes at the next rewind"
#[test]
fn reads_the_provider_when_the_daemons_road_refuses_a_record_that_reads_running_so_a_machine_stopped_behind_it_wakes_at_the_next_rewind() {
    drive(|| async {
        let machine: Rc<RefCell<Option<Arc<StubMachine>>>> = Rc::default();
        let daemon = fake_daemon(DaemonOpts::default());
        // A stopped box's route answers the upgrade with a redirect to its archived page, and the box stays stopped.
        let looking = machine.clone();
        daemon.rec.refusing_dials(move || {
            looking.borrow().as_ref().is_some_and(|m| m.paused.get()).then(|| {
                let mut door = HostError::with_kind("the connection was refused with 302", Kind::Refused);
                door.status = Some(302.0);
                door
            })
        });
        let w = workspace(harness(HarnessOpts::default()), daemon, None).await;
        let m = w.backend.machine(0);
        *machine.borrow_mut() = Some(m.clone());
        m.set_daemon_answer(true);
        let (thread_id, turns) = three_turns(&w).await;
        m.paused.set(true);
        let said = refusal(&w, &thread_id, rewind_to(&turns[0], true)).await;
        assert!(said.contains("the connection was refused with 302"), "{said}");
        assert_eq!(m.calls.resume.get(), 0);
        assert_eq!(rewind(&w, &thread_id, rewind_to(&turns[0], true)).await.expect("a rewind"), json!({ "turns": 2, "files": 2 }));
        assert_eq!(m.calls.resume.get(), 1);
        w.fx.rt.close().await.expect("a close");
    });
}

fn as_thread(thread: &str, workspace: &str) -> Caller {
    let by = serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": workspace, "rootThreadId": thread })).expect("a scope");
    Caller::Scoped { origin: WorkspaceOrigin::Here, by }
}

// "rewinding a thread to one of its replies" > "leaves the slate as the rewind left it on undo, since undo never puts the conversation back"
#[test]
fn leaves_the_slate_as_the_rewind_left_it_on_undo_since_undo_never_puts_the_conversation_back() {
    drive(|| async {
        let w = workspace(harness(HarnessOpts::default()), fake_daemon(DaemonOpts::default()), None).await;
        let (thread_id, turns) = three_turns(&w).await;
        let caller = as_thread(&thread_id, &w.ws);
        let slates = w.fx.rt.slates();
        let written = WriteParams {
            text: Some(r#"<slate><value name="pick" start="C" /><text id="t">Version {$pick}</text></slate>"#.into()),
            ..Default::default()
        };
        slates.write(written, Some(&caller)).await.expect("a slate");
        let fourth = w.fx.rt.sessions().start(&w.ws, on_thread("four", &thread_id), None).await.expect("a start");
        fourth.finished().await;
        wait().await;
        rewind(&w, &thread_id, rewind_to(&turns[0], true)).await.expect("a rewind");
        let after_rewind = slates.get(&thread_id).await.expect("a read").expect("a slate");
        assert_eq!(after_rewind.empty, Some(wsp_slates::pending::wire::SlateEmpty::RewoundBefore));
        assert_eq!(rewind(&w, &thread_id, undo()).await.expect("an undo"), json!({ "turns": 0, "files": 2 }));
        let after = slates.get(&thread_id).await.expect("a read").expect("a slate");
        assert!(after.document.is_none());
        assert_eq!(after.empty, Some(wsp_slates::pending::wire::SlateEmpty::RewoundBefore));
        w.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "refuses before anything is written: a working thread, the latest reply, and files a turn kept no checkpoint of"
#[test]
fn refuses_before_anything_is_written_a_working_thread_the_latest_reply_and_files_a_turn_kept_no_checkpoint_of() {
    drive(|| async {
        let w = workspace(harness(HarnessOpts { hang_from: Some(3), ..Default::default() }), fake_daemon(DaemonOpts::default()), None).await;
        let first = w.fx.rt.sessions().start(&w.ws, start("one"), None).await.expect("a start");
        first.finished().await;
        let thread_id = first.view().thread_id.expect("a thread");
        let second = w.fx.rt.sessions().start(&w.ws, on_thread("two", &thread_id), None).await.expect("a start");
        second.finished().await;
        wait().await;
        assert!(refusal(&w, &thread_id, rewind_to(&second.turn_id(), false)).await.contains(REWIND_LATEST_LINE));
        let third = w.fx.rt.sessions().start(&w.ws, on_thread("three", &thread_id), None).await.expect("a start");
        wait().await;
        w.daemon.clear();
        assert!(refusal(&w, &thread_id, rewind_to(&first.turn_id(), true)).await.contains(REWIND_WORKING_LINE));
        assert!(w.daemon.frames().is_empty());
        third.interrupt().await;
        wait().await;
        assert_eq!(texts(&w).await, ["reply 1", "reply 2"]);

        let refusing = workspace(harness(HarnessOpts::default()), fake_daemon(DaemonOpts { refuse_checkpoint: true, ..Default::default() }), None).await;
        let (other, turns) = three_turns(&refusing).await;
        assert!(refusal(&refusing, &other, rewind_to(&turns[0], true)).await.contains(REWIND_NO_CHECKPOINT_LINE));
        // The conversation alone still goes, off the anchor the turn kept.
        assert_eq!(rewind(&refusing, &other, rewind_to(&turns[0], false)).await.expect("a rewind"), json!({ "turns": 2 }));
        w.fx.rt.close().await.expect("a close");
        refusing.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "refuses a thread whose threads under it still run, naming them"
#[test]
fn refuses_a_thread_whose_threads_under_it_still_run_naming_them() {
    drive(|| async {
        let w = workspace(harness(HarnessOpts { hang_from: Some(4), ..Default::default() }), fake_daemon(DaemonOpts::default()), None).await;
        let (thread_id, turns) = three_turns(&w).await;
        // A thread this one's agent opened, still working; the lead itself is idle.
        let by = as_thread(&thread_id, &w.ws);
        let opts = StartOptions { started_by: Some(wsp_wire::views::session::SessionOrigin::Agent), ..start("fix the port list") };
        let child = w.fx.rt.sessions().start(&w.ws, opts, Some(by)).await.expect("a start");
        wait().await;
        let said = refusal(&w, &thread_id, rewind_to(&turns[0], false)).await;
        assert!(said.contains(&rewind_children_line(&["fix the port list"])), "{said}");
        child.interrupt().await;
        wait().await;
        // Stopped, it stands in the way of nothing.
        assert_eq!(rewind(&w, &thread_id, rewind_to(&turns[0], false)).await.expect("a rewind"), json!({ "turns": 2 }));
        w.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "moves no files once another thread worked in the folder after the checkpoint, running or done, and the conversation still goes"
#[test]
fn moves_no_files_once_another_thread_worked_in_the_folder_after_the_checkpoint_running_or_done_and_the_conversation_still_goes() {
    drive(|| async {
        let w = workspace(harness(HarnessOpts { hang_from: Some(4), ..Default::default() }), fake_daemon(DaemonOpts::default()), None).await;
        let (thread_id, turns) = three_turns(&w).await;
        // A second thread of the person's on the same workspace, so in the same folder, mid-turn.
        let beside = w.fx.rt.sessions().start(&w.ws, start("move the pricing table"), None).await.expect("a start");
        wait().await;
        w.daemon.clear();
        assert_eq!(
            rewind(&w, &thread_id, rewind_to(&turns[1], true)).await.expect("a rewind"),
            json!({ "turns": 1, "kept": REWIND_SHARED_LINE })
        );
        beside.interrupt().await;
        wait().await;
        // Its turn is over, and it still ran after every checkpoint this thread holds.
        assert_eq!(
            rewind(&w, &thread_id, rewind_to(&turns[0], true)).await.expect("a rewind"),
            json!({ "turns": 1, "kept": REWIND_SHARED_LINE })
        );
        assert!(w.daemon.restores().is_empty());
        w.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "will not undo while another thread in the folder works, and any turn that ends in the folder closes every undo there"
#[test]
fn will_not_undo_while_another_thread_in_the_folder_works_and_any_turn_that_ends_in_the_folder_closes_every_undo_there() {
    drive(|| async {
        let w = workspace(harness(HarnessOpts { hang_from: Some(4), ..Default::default() }), fake_daemon(DaemonOpts::default()), None).await;
        let (thread_id, turns) = three_turns(&w).await;
        rewind(&w, &thread_id, rewind_to(&turns[0], true)).await.expect("a rewind");
        let beside = w.fx.rt.sessions().start(&w.ws, start("move the pricing table"), None).await.expect("a start");
        wait().await;
        w.daemon.clear();
        let said = refusal(&w, &thread_id, undo()).await;
        assert!(said.contains(&rewind_beside_line("move the pricing table")), "{said}");
        assert!(w.daemon.restores().is_empty());
        // The other thread's turn ends with work the undo's checkpoint predates, so the undo is gone, and a window hears it.
        beside.interrupt().await;
        wait().await;
        assert!(thread_of(&w, &thread_id).await.get("rewoundAt").is_none());
        let marked = w.events.of("thread.marked");
        assert!(
            marked.iter().any(|e| e["workspaceId"] == w.ws.as_str() && e["threadIds"] == json!([thread_id])),
            "{marked:?}"
        );
        assert!(refusal(&w, &thread_id, undo()).await.contains(REWIND_NO_UNDO_LINE));
        assert!(w.daemon.restores().is_empty());
        w.fx.rt.close().await.expect("a close");
    });
}

// "rewinding a thread to one of its replies" > "is a person's act alone: no thread's token and no paired computer may ask it"
#[test]
fn is_a_persons_act_alone_no_threads_token_and_no_paired_computer_may_ask_it() {
    use wsp_wire::wire::runtime::{DEVICE_OPS, THREAD_OPS};
    assert!(!THREAD_OPS.contains(&"sessions.rewind"));
    assert!(!DEVICE_OPS.contains(&"sessions.rewind"));
}
