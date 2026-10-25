// SPDX-License-Identifier: AGPL-3.0-only
//! The hard cases of unit port (spec/port.md): launch words, ending a run, kept runs and lost sessions. Expected
//! values follow packages/protocol/src/{adapter-port,kept-run,lost-session}.ts at a3b870a6e.

#[path = "common/claude_exec.rs"]
mod claude_exec;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use claude_exec::{FakeStream, StreamOpts};
use futures::future::{BoxFuture, FutureExt};
use futures::stream::{BoxStream, StreamExt};
use wsp_adapters::pending::{lost_session_prompt, LOST_SESSION_NOTE};
use wsp_adapters::port::*;
use wsp_wire::types::js_number::JsNumber;

fn manual() -> (Arc<FakeStream>, Arc<Mutex<Vec<String>>>) {
    let order: Arc<Mutex<Vec<String>>> = Default::default();
    (FakeStream::manual(StreamOpts { run: Some("run-1".into()), ..Default::default() }, order.clone(), Default::default()), order)
}

/// A manual stream that counts its turns as a multi-turn road does: next_turn answers 100 bytes more each time and
/// rest is noted in the order list.
struct Turned {
    inner: Arc<FakeStream>,
    bytes: AtomicU64,
}

impl ExecStream for Turned {
    fn lines(&self) -> BoxStream<'static, Result<String, ExecFailure>> {
        self.inner.lines()
    }
    fn run(&self) -> Option<String> {
        self.inner.run()
    }
    fn teardown(&self) {
        self.inner.teardown()
    }
    fn kill(&self) {
        self.inner.kill()
    }
    fn write(&self, line: String) -> BoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        self.inner.write(line)
    }
    fn close_input(&self) {
        self.inner.close_input()
    }
    fn rest(&self) -> Option<()> {
        self.inner.order.lock().unwrap().push("rest".into());
        Some(())
    }
    fn next_turn(&self) -> Option<u64> {
        Some(self.bytes.fetch_add(100, Ordering::SeqCst) + 100)
    }
    fn exited(&self) -> SharedFuture<Option<i32>> {
        self.inner.exited()
    }
    fn signalled(&self) -> Option<String> {
        self.inner.signalled()
    }
}

async fn next_line(lines: &mut BoxStream<'static, Result<String, ExecFailure>>) -> Option<String> {
    tokio::time::timeout(Duration::from_secs(2), lines.next()).await.expect("a line or the end within 2 s").map(|l| l.expect("no failure"))
}

#[test]
fn the_launch_words_are_the_ts_ones() {
    assert_eq!(program_word("claude", None), "claude");
    assert_eq!(program_word("claude", Some(&AgentLaunch { program: None, args: Some(vec!["--x".into()]) })), "claude");
    assert_eq!(program_word("claude", Some(&AgentLaunch { program: Some("/opt/it's".into()), args: None })), r#"'/opt/it'\''s'"#);
    assert_eq!(agent_word("codex", None), r#"sh -c '[ -z "${WSP_AGENT_PID_FILE:-}" ] || echo $$ > "$WSP_AGENT_PID_FILE"; unset WSP_AGENT_PID_FILE; exec "$0" "$@"' codex"#);
    assert_eq!(launch_words(Some(&AgentLaunch { program: None, args: Some(vec!["--a b".into(), "c".into()]) })), vec!["'--a b'".to_string(), "'c'".to_string()]);
    assert_eq!(launch_words(None), Vec::<String>::new());
    let end = AdapterEvent::SessionEnd { session_id: "s-9".into(), exit_code: None, saw_result: false, failure: None };
    assert_eq!(end.session_id(), "s-9");
    let before = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    let now = system_now()();
    assert!(now >= before && now - before < 5_000, "{now} against {before}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn end_run_kills_only_what_outlives_the_grace() {
    let order: Arc<Mutex<Vec<String>>> = Default::default();
    let held = FakeStream::manual(StreamOpts { slow_teardown: true, ..Default::default() }, order.clone(), Default::default());
    end_run(held.as_ref(), 20).await;
    assert_eq!(*order.lock().unwrap(), vec!["teardown", "kill"]);
    assert_eq!(held.exited().await, None, "the kill's exit has no code");
    let (quick, order) = manual();
    end_run(quick.as_ref(), 2_000).await;
    assert_eq!(*order.lock().unwrap(), vec!["teardown"], "a run that goes on the teardown is never killed");
    assert_eq!(quick.exited().await, Some(143));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn end_after_result_leaves_a_run_that_goes_on_its_own() {
    let (going, order) = manual();
    let ender = going.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        ender.end(Some(0));
    });
    end_after_result(going.as_ref(), 2_000, 2_000).await;
    assert_eq!(*order.lock().unwrap(), Vec::<String>::new());
    let order2: Arc<Mutex<Vec<String>>> = Default::default();
    let lingering = FakeStream::manual(StreamOpts { slow_teardown: true, ..Default::default() }, order2.clone(), Default::default());
    end_after_result(lingering.as_ref(), 10, 10).await;
    assert_eq!(*order2.lock().unwrap(), vec!["teardown", "kill"]);
}

#[tokio::test]
async fn settles_within_reads_a_rejection_as_settled() {
    assert!(settles_within(async { Err::<(), &str>("no") }.boxed(), 50).await);
    assert!(settles_within(async { 1 }.boxed(), 50).await);
    let never = futures::future::pending::<()>().boxed();
    let started = std::time::Instant::now();
    assert!(!settles_within(never, 30).await);
    assert!(started.elapsed() >= Duration::from_millis(30));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_kept_run_hands_each_turn_its_own_lines() {
    let (inner, order) = manual();
    let stream: Arc<dyn ExecStream> = Arc::new(Turned { inner: inner.clone(), bytes: AtomicU64::new(0) });
    let between: Arc<Mutex<Vec<String>>> = Default::default();
    let sink = between.clone();
    let kept = keep_run(stream, Some(Arc::new(move |line| sink.lock().unwrap().push(line))));
    assert!(kept.up());
    let (first, from) = kept.turn();
    assert_eq!(from, None, "the first turn reads the log from its start");
    assert_eq!(first.run().as_deref(), Some("run-1"));
    let mut lines = first.lines();
    inner.push("a");
    assert_eq!(next_line(&mut lines).await.as_deref(), Some("a"));
    inner.push("b");
    tokio::time::sleep(Duration::from_millis(20)).await;
    first.close_input();
    assert_eq!(first.exited().await, Some(0), "the view ends, the process does not");
    assert!(order.lock().unwrap().contains(&"rest".to_string()));
    assert!(!order.lock().unwrap().contains(&"closeInput".to_string()), "the process's own input stays open");
    inner.push("hook line");
    tokio::time::sleep(Duration::from_millis(20)).await;
    let (second, from) = kept.turn();
    assert_eq!(from, Some(100));
    let mut lines = second.lines();
    inner.push("c");
    assert_eq!(next_line(&mut lines).await.as_deref(), Some("c"));
    assert_eq!(*between.lock().unwrap(), vec!["b", "hook line"], "what no turn read went to between, in order");
    assert!(kept.up());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_closed_view_rests_the_process_and_takes_no_write() {
    let (inner, order) = manual();
    let kept = keep_run(inner.clone(), None);
    let (view, _) = kept.turn();
    assert_eq!(view.write("one".into()).await, Ok(WriteOutcome::Written));
    view.close_input();
    view.close_input();
    assert_eq!(view.write("two".into()).await, Ok(WriteOutcome::Gone));
    assert_eq!(*inner.writes.lock().unwrap(), vec!["one"]);
    assert_eq!(order.lock().unwrap().iter().filter(|o| *o == "write").count(), 1, "the closed view never reached the process");
    assert_eq!(view.signalled(), None);
    kept.close(0, 10).await;
    assert!(!kept.up());
    assert_eq!(order.lock().unwrap().iter().filter(|o| *o == "closeInput").count(), 1, "close shuts the process's input once");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_released_view_ends_with_the_process() {
    let (inner, order) = manual();
    let kept = keep_run(inner.clone(), None);
    let (view, _) = kept.turn();
    let mut lines = view.lines();
    kept.release();
    assert!(!kept.up());
    view.close_input();
    assert_eq!(*order.lock().unwrap(), vec!["closeInput"], "released, the view's close reaches the process");
    inner.push("last words");
    inner.end(Some(3));
    assert_eq!(next_line(&mut lines).await.as_deref(), Some("last words"));
    assert_eq!(next_line(&mut lines).await, None);
    assert_eq!(view.exited().await, Some(3));
    assert_eq!(kept.exited().await, Some(3));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_view_after_the_end_ends_at_once() {
    let (inner, _) = manual();
    let kept = keep_run(inner.clone(), None);
    inner.fail(ExecFailure { kind: ExecFailureKind::NoAnswer, message: "the machine went away".into() });
    inner.end(Some(1));
    let _ = kept.exited().await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!kept.up());
    let (late, _) = kept.turn();
    let mut lines = late.lines();
    let first = tokio::time::timeout(Duration::from_secs(2), lines.next()).await.expect("ends at once");
    assert_eq!(first, Some(Err(ExecFailure { kind: ExecFailureKind::NoAnswer, message: "the machine went away".into() })), "the stored failure is what the late view reads");
    assert_eq!(late.exited().await, Some(1));
}

/// A launch start_after_loss holds: it emits `events` when told to and finishes with `result`.
struct FakeTurn {
    finished: SharedFuture<TurnResult>,
}

impl Finishes for FakeTurn {
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
}

fn result(error: Option<&str>, text: Option<&str>) -> TurnResult {
    TurnResult { status: if error.is_some() { TurnStatus::Failed } else { TurnStatus::Completed }, duration_ms: Some(JsNumber(5.0)), waited_ms: None, cost_usd: None, tokens: None, model: None, models: None, text: text.map(String::from), error: error.map(String::from), unreached: None, refusal: None, limit: None }
}

fn start(id: &str) -> AdapterEvent {
    AdapterEvent::SessionStart { session_id: id.into(), model: None, cwd: None, tools: None, harness: None, prompted: None }
}

fn done(id: &str, r: TurnResult) -> AdapterEvent {
    AdapterEvent::TurnDone { session_id: id.into(), result: r, held: None }
}

fn end(id: &str) -> AdapterEvent {
    AdapterEvent::SessionEnd { session_id: id.into(), exit_code: Some(0), saw_result: true, failure: None }
}

const LOST: &str = "No conversation found with session ID: old-1";

/// Each launch plays one script of events synchronously, then finishes with that script's result.
fn launcher(scripts: Vec<(Vec<AdapterEvent>, TurnResult)>, calls: Arc<Mutex<Vec<(String, Option<String>)>>>) -> impl Fn(String, Option<String>, OnEvent) -> Result<Arc<FakeTurn>, AdapterError> + Send + Sync + 'static {
    let scripts = Arc::new(Mutex::new(scripts.into_iter()));
    move |prompt, resume, on_event| {
        calls.lock().unwrap().push((prompt, resume));
        let (events, result) = scripts.lock().unwrap().next().expect("no more launches scripted");
        for e in events {
            on_event(e);
        }
        Ok(Arc::new(FakeTurn { finished: async move { result }.boxed().shared() }))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_lost_session_starts_again_seeded_and_says_so() {
    let (events, on_event) = claude_exec::collect();
    let calls: Arc<Mutex<Vec<(String, Option<String>)>>> = Default::default();
    let lost = result(Some(LOST), None);
    let answer = result(None, Some("done"));
    let launch = launcher(
        vec![(vec![start("old-1"), done("old-1", lost.clone()), end("old-1")], lost), (vec![start("new-2"), done("new-2", answer.clone()), end("new-2")], answer.clone())],
        calls.clone(),
    );
    let options = LostSessionStart { prompt: "go on".into(), resume: "old-1".into(), seed: Arc::new(|| async { Ok("the thread so far".to_string()) }.boxed()), on_event };
    let after = start_after_loss(options, LOST.into(), launch).expect("the first launch starts");
    assert_eq!(after.finished.clone().await, answer);
    assert_eq!(*calls.lock().unwrap(), vec![("go on".to_string(), Some("old-1".to_string())), (lost_session_prompt("the thread so far", "go on"), None)]);
    let note = AdapterEvent::TurnDelta(TurnDelta::new("new-2", DeltaKind::Note, LOST_SESSION_NOTE));
    assert_eq!(*events.lock().unwrap(), vec![start("old-1"), start("new-2"), note, done("new-2", answer), end("new-2")], "the lost turn's end and its session.end reach nobody");
    assert!(!Arc::ptr_eq(&after.first, &after.now.lock().unwrap().clone()), "now is the second launch");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_session_still_held_runs_as_one_launch() {
    let (events, on_event) = claude_exec::collect();
    let calls: Arc<Mutex<Vec<(String, Option<String>)>>> = Default::default();
    let other_failure = result(Some("No conversation found with session ID: someone-else"), None);
    let launch = launcher(vec![(vec![start("old-1"), done("old-1", other_failure.clone()), end("old-1")], other_failure.clone())], calls.clone());
    let options = LostSessionStart { prompt: "go on".into(), resume: "old-1".into(), seed: Arc::new(|| async { panic!("never asked for a seed") }.boxed()), on_event };
    let after = start_after_loss(options, LOST.into(), launch).expect("starts");
    assert_eq!(after.finished.clone().await, other_failure, "only the exact sentence for this session is a loss");
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(*events.lock().unwrap(), vec![start("old-1"), done("old-1", other_failure), end("old-1")]);
    assert!(Arc::ptr_eq(&after.first, &after.now.lock().unwrap().clone()));
}

// Rule P1: the runtime retries a message whose computer said nothing (threads.ts:67, `computerSilent`), which is
// every class the transport throws but a plain error.
#[test]
fn every_transport_kind_but_other_is_a_silent_computer() {
    let kinds = [ExecFailureKind::Unreached, ExecFailureKind::Unreachable, ExecFailureKind::PlaceAbsent, ExecFailureKind::LaunchUnanswered, ExecFailureKind::Deadline, ExecFailureKind::NoAnswer, ExecFailureKind::Other];
    assert_eq!(kinds.map(ExecFailureKind::silent), [true, true, true, true, true, true, false]);
}

fn failed(error: &str) -> TurnResult {
    TurnResult { status: TurnStatus::Failed, duration_ms: None, waited_ms: None, cost_usd: None, tokens: None, model: None, models: None, text: None, error: Some(error.into()), unreached: None, refusal: None, limit: None }
}

// Rule P2: release is one flag for good. A view a later turn() opens, and an older view, both close the process's
// input from then on, and the run is never up again.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_release_holds_for_every_view_after_it() {
    let (inner, order) = manual();
    let stream: Arc<dyn ExecStream> = Arc::new(Turned { inner: inner.clone(), bytes: AtomicU64::new(0) });
    let kept = keep_run(stream, None);
    let (first, _) = kept.turn();
    kept.release();
    let (second, _) = kept.turn();
    second.close_input();
    first.close_input();
    assert_eq!(*order.lock().unwrap(), vec!["closeInput", "closeInput"], "no rest: each close reached the process");
    assert!(!kept.up());
}

// Rule P3: a view a later turn() cut still rests the process when its input closes, and its exited settles 0, while the
// newer view stays open and takes writes.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cut_view_closing_rests_the_process_and_leaves_the_open_one() {
    let (inner, order) = manual();
    let stream: Arc<dyn ExecStream> = Arc::new(Turned { inner: inner.clone(), bytes: AtomicU64::new(0) });
    let kept = keep_run(stream, None);
    let (first, _) = kept.turn();
    let (second, from) = kept.turn();
    assert_eq!(from, Some(100), "next_turn is asked before the cut");
    first.close_input();
    assert_eq!(first.exited().await, Some(0));
    assert_eq!(*order.lock().unwrap(), vec!["rest"]);
    assert_eq!(second.write("x".into()).await, Ok(WriteOutcome::Written));
    assert!(kept.up());
}

// Rule P4: close on a process that lingers past the wait is closeInput, teardown, then kill past the grace; a second
// close closes the input again.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn closing_a_lingering_kept_process_kills_it_past_the_grace() {
    let order: Arc<Mutex<Vec<String>>> = Default::default();
    let inner = FakeStream::manual(StreamOpts { slow_teardown: true, ..Default::default() }, order.clone(), Default::default());
    let kept = keep_run(inner.clone(), None);
    let _view = kept.turn();
    kept.close(20, 10).await;
    assert_eq!(*order.lock().unwrap(), vec!["closeInput", "teardown", "kill"]);
    assert_eq!(kept.exited().await, None, "killed: no code");
    kept.close(0, 10).await;
    assert_eq!(order.lock().unwrap().iter().filter(|o| *o == "closeInput").count(), 2);
}

// Rule P5 (the coordinator's ruling of 2026-10-25 on a refused seed): a seed that fails settles the turn failed with the
// seed's words; no second launch is made and `now` stays the first.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_seed_fails_the_turn_with_its_words() {
    let (events, on_event) = claude_exec::collect();
    let calls: Arc<Mutex<Vec<(String, Option<String>)>>> = Default::default();
    let lost = result(Some(LOST), None);
    let launch = launcher(vec![(vec![start("old-1"), done("old-1", lost.clone()), end("old-1")], lost)], calls.clone());
    let options = LostSessionStart { prompt: "go on".into(), resume: "old-1".into(), seed: Arc::new(|| async { Err("seed broke".to_string()) }.boxed()), on_event };
    let after = start_after_loss(options, LOST.into(), launch).expect("the first launch starts");
    assert_eq!(after.finished.clone().await, failed("seed broke"));
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(*events.lock().unwrap(), vec![start("old-1")]);
    assert!(Arc::ptr_eq(&after.first, &after.now.lock().unwrap().clone()));
}

// Rule P6: the second launch refusing (the TS rejects `finished` with its error) settles the turn failed with the
// launch's words, by the same ruling; nothing more is emitted.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_second_launch_fails_the_turn_with_its_words() {
    let (events, on_event) = claude_exec::collect();
    let lost = result(Some(LOST), None);
    let first = lost.clone();
    let made = Arc::new(AtomicU64::new(0));
    let count = made.clone();
    let launch = move |_prompt: String, _resume: Option<String>, on_event: OnEvent| -> Result<Arc<FakeTurn>, AdapterError> {
        if count.fetch_add(1, Ordering::SeqCst) > 0 {
            return Err(AdapterError("no machine".into()));
        }
        for e in [start("old-1"), done("old-1", first.clone()), end("old-1")] {
            on_event(e);
        }
        let r = first.clone();
        Ok(Arc::new(FakeTurn { finished: async move { r }.boxed().shared() }))
    };
    let options = LostSessionStart { prompt: "go on".into(), resume: "old-1".into(), seed: Arc::new(|| async { Ok("so far".to_string()) }.boxed()), on_event };
    let after = start_after_loss(options, LOST.into(), launch).expect("the first launch starts");
    assert_eq!(after.finished.clone().await, failed("no machine"));
    assert_eq!(made.load(Ordering::SeqCst), 2);
    assert_eq!(*events.lock().unwrap(), vec![start("old-1")]);
    assert!(Arc::ptr_eq(&after.first, &after.now.lock().unwrap().clone()));
}

// Rule P7: only a turn.done event marks the session lost: a result carrying the words with no such event is the turn's
// own, and the seed is never asked.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_loss_is_read_off_the_event_and_never_off_the_result() {
    let (events, on_event) = claude_exec::collect();
    let calls: Arc<Mutex<Vec<(String, Option<String>)>>> = Default::default();
    let lost = result(Some(LOST), None);
    let launch = launcher(vec![(vec![start("old-1"), end("old-1")], lost.clone())], calls.clone());
    let options = LostSessionStart { prompt: "go on".into(), resume: "old-1".into(), seed: Arc::new(|| async { panic!("never asked for a seed") }.boxed()), on_event };
    let after = start_after_loss(options, LOST.into(), launch).expect("starts");
    assert_eq!(after.finished.clone().await, lost);
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(*events.lock().unwrap(), vec![start("old-1"), end("old-1")]);
}

// Rule P8: a second launch that never says session.start gets no note.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_note_without_the_new_sessions_start() {
    let (events, on_event) = claude_exec::collect();
    let calls: Arc<Mutex<Vec<(String, Option<String>)>>> = Default::default();
    let lost = result(Some(LOST), None);
    let cut = result(Some("cut"), None);
    let launch = launcher(vec![(vec![start("old-1"), done("old-1", lost.clone()), end("old-1")], lost), (vec![done("new-2", cut.clone()), end("new-2")], cut.clone())], calls.clone());
    let options = LostSessionStart { prompt: "go on".into(), resume: "old-1".into(), seed: Arc::new(|| async { Ok("so far".to_string()) }.boxed()), on_event };
    let after = start_after_loss(options, LOST.into(), launch).expect("starts");
    assert_eq!(after.finished.clone().await, cut);
    assert_eq!(*events.lock().unwrap(), vec![start("old-1"), done("new-2", cut), end("new-2")]);
}
