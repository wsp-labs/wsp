// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/thread-keeps-picks.test.ts at 65f6b89c7, case for case. A thread keeps the model, effort and
//! context window its turns ran on: a start into it that names none of them runs on the thread's own latest picks, and
//! anything the start names still wins. Each test names the TS test above it.
mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use common::adapter::{event, result};
use common::*;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::port::{AdapterError, HarnessAdapter, HarnessSession, HarnessStartOptions, SharedFuture, TurnResult};
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_store::store::Store;
use wsp_wire::types::place_word::HERE_PLACE_ID;

/// What a start was handed of the three picks, and its prompt.
#[derive(Clone, Debug, PartialEq)]
struct Start {
    prompt: String,
    model: Option<String>,
    effort: Option<String>,
    context_window: Option<String>,
}

type Starts = Rc<RefCell<Vec<Start>>>;

/// A harness that answers at once and keeps every start it was handed. Each turn announces the model it was asked for,
/// with the window as a suffix where it rode one, the way claude's init names "claude-opus-5-5[1m]".
fn recording(mark: &str) -> (HarnessAdapterFactory, Starts) {
    let starts: Starts = Rc::default();
    let (kept, mark) = (starts.clone(), mark.to_string());
    let factory: HarnessAdapterFactory =
        Arc::new(move |_ctx| Ok(Arc::new(Recording { starts: kept.clone(), mark: mark.clone() }) as Arc<dyn HarnessAdapter>));
    (factory, starts)
}

struct Recording {
    starts: Starts,
    mark: String,
}

impl HarnessAdapter for Recording {
    fn steers(&self) -> bool {
        false
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        self.starts.borrow_mut().push(Start {
            prompt: o.prompt.clone(),
            model: o.model.clone(),
            effort: o.effort.clone(),
            context_window: o.context_window.clone(),
        });
        let session = format!("{:08}-{}-4111-8111-111111111111", self.starts.borrow().len(), self.mark);
        let done: TurnResult = result(json!({ "status": "completed", "text": "ok" }));
        let announced = o.model.as_ref().map(|m| if o.context_window.as_deref() == Some("1m") { format!("{m}[1m]") } else { m.clone() });
        let on_event = o.on_event.clone().expect("an event sink");
        let (session_id, end) = (session.clone(), done.clone());
        let finished: LocalBoxFuture<'static, TurnResult> = async move {
            tokio::task::yield_now().await;
            let mut start = json!({ "type": "session.start", "sessionId": session_id });
            if let Some(model) = announced {
                start["model"] = json!(model);
            }
            on_event(event(start));
            on_event(event(
                json!({ "type": "turn.done", "sessionId": session_id, "result": serde_json::to_value(&end).expect("a result") }),
            ));
            on_event(event(json!({ "type": "session.end", "sessionId": session_id, "exitCode": 0, "sawResult": true })));
            end
        }
        .boxed_local();
        Ok(Arc::new(Instant { local_id: session, finished: finished.shared() }))
    }
}

struct Instant {
    local_id: String,
    finished: SharedFuture<TurnResult>,
}

impl HarnessSession for Instant {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        // The TS promise runs its body once the microtask comes; the first poll here is the runtime's.
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
}

struct Host {
    root: tempfile::TempDir,
    claude: Starts,
    codex: Starts,
    claude_f: HarnessAdapterFactory,
    codex_f: HarnessAdapterFactory,
    state: State,
    fx: Fx,
}

fn build(root: &std::path::Path, state: &State, claude: &HarnessAdapterFactory, codex: &HarnessAdapterFactory) -> Fx {
    let local = fake_local(root);
    runtime(&StubBackend::new(None))
        .store(&state.store)
        .adapter("claude", claude.clone())
        .adapter("codex", codex.clone())
        .with(move |o| o.local = Some(local))
        .build()
}

fn host() -> Host {
    let root = tempfile::tempdir().expect("a folder");
    let (claude_f, claude) = recording("1111");
    let (codex_f, codex) = recording("2222");
    let state = sqlite_state();
    let fx = build(root.path(), &state, &claude_f, &codex_f);
    Host { root, claude, codex, claude_f, codex_f, state, fx }
}

impl Host {
    fn again(&mut self) {
        self.fx = build(self.root.path(), &self.state, &self.claude_f, &self.codex_f);
    }
}

fn start(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

struct Open {
    harness: &'static str,
    model: &'static str,
    effort: &'static str,
    context_window: Option<&'static str>,
}

async fn open_thread(h: &Host, o: Open) -> (String, String) {
    let rt = &*h.fx.rt;
    let ws = create_on(rt, CreateOn::named("mac").on(HERE_PLACE_ID), None).await.expect("a workspace").view.id;
    let first = rt
        .sessions()
        .start(
            &ws,
            StartOptions {
                harness: Some(o.harness.into()),
                model: Some(o.model.into()),
                effort: Some(o.effort.into()),
                context_window: o.context_window.map(str::to_string),
                ..start("one")
            },
            None,
        )
        .await
        .expect("a start");
    first.finished().await;
    (ws, first.view().thread_id.expect("a thread"))
}

fn picks(starts: &Starts) -> (Option<String>, Option<String>, Option<String>) {
    let last = starts.borrow().last().cloned().expect("a start");
    (last.model, last.effort, last.context_window)
}

fn want(model: Option<&str>, effort: &str, window: Option<&str>) -> (Option<String>, Option<String>, Option<String>) {
    (model.map(str::to_string), Some(effort.to_string()), window.map(str::to_string))
}

async fn send(h: &Host, ws: &str, thread: &str, prompt: &str, more: impl FnOnce(&mut StartOptions)) {
    let mut o = StartOptions { thread: Some(thread.into()), ..start(prompt) };
    more(&mut o);
    h.fx.rt.sessions().start(ws, o, None).await.expect("a start").finished().await;
}

const OPUS_1M: Open = Open { harness: "claude", model: "claude-opus-5", effort: "low", context_window: Some("1m") };

// "the model, effort and window a start into a thread runs on" > "a send naming none runs on the thread's model, effort and window, the window read back off the announced model"
#[test]
fn a_send_naming_none_runs_on_the_threads_model_effort_and_window_the_window_read_back_off_the_announced_model() {
    drive(|| async {
        let h = host();
        let (ws, thread) = open_thread(&h, OPUS_1M).await;
        let rows = h.fx.rt.sessions().list(Some(ws.clone()), None).await.expect("a list");
        assert_eq!(rows.last().and_then(|r| r.model.clone()).as_deref(), Some("claude-opus-5[1m]"));
        send(&h, &ws, &thread, "two", |_| {}).await;
        assert_eq!(picks(&h.claude), want(Some("claude-opus-5"), "low", Some("1m")));
        // The third start reads the second's row, which is where the thread's latest picks now are.
        send(&h, &ws, &thread, "three", |_| {}).await;
        assert_eq!(picks(&h.claude), want(Some("claude-opus-5"), "low", Some("1m")));
        h.fx.rt.close().await.expect("a close");
    });
}

// "what a send names wins, and the effort it leaves out stays where the new model takes it"
#[test]
fn what_a_send_names_wins_and_the_effort_it_leaves_out_stays_where_the_new_model_takes_it() {
    drive(|| async {
        let h = host();
        let (ws, thread) = open_thread(&h, OPUS_1M).await;
        send(&h, &ws, &thread, "two", |o| o.effort = Some("max".into())).await;
        assert_eq!(picks(&h.claude), want(Some("claude-opus-5"), "max", Some("1m")));
        // Sonnet 5 takes the effort but offers no window, so the window does not ride with it.
        send(&h, &ws, &thread, "three", |o| o.model = Some("claude-sonnet-5".into())).await;
        assert_eq!(picks(&h.claude), want(Some("claude-sonnet-5"), "max", None));
        send(&h, &ws, &thread, "four", |_| {}).await;
        assert_eq!(picks(&h.claude), want(Some("claude-sonnet-5"), "max", None));
        h.fx.rt.close().await.expect("a close");
    });
}

// "a codex thread keeps its model and effort the same way"
#[test]
fn a_codex_thread_keeps_its_model_and_effort_the_same_way() {
    drive(|| async {
        let h = host();
        let (ws, thread) = open_thread(&h, Open { harness: "codex", model: "gpt-5.5", effort: "high", context_window: None }).await;
        send(&h, &ws, &thread, "two", |_| {}).await;
        assert_eq!(picks(&h.codex), want(Some("gpt-5.5"), "high", None));
        h.fx.rt.close().await.expect("a close");
    });
}

// "a child's report back runs its parent's turn on the parent's own picks"
#[test]
fn a_childs_report_back_runs_its_parents_turn_on_the_parents_own_picks() {
    drive(|| async {
        let h = host();
        let (ws, thread) = open_thread(&h, Open { harness: "codex", model: "gpt-5.5", effort: "high", context_window: None }).await;
        let child =
            h.fx.rt
                .sessions()
                .start(&ws, StartOptions { harness: Some("claude".into()), notify: Some(vec![thread.clone()]), ..start("build it") }, None)
                .await
                .expect("a start");
        child.finished().await;
        until(|| async { h.codex.borrow().len() == 2 }).await;
        assert!(h.codex.borrow().last().expect("a start").prompt.contains("finished"));
        assert_eq!(picks(&h.codex), want(Some("gpt-5.5"), "high", None));
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread whose rows fell off the index still sends on its model, effort and window, read off its transcript"
#[test]
fn a_thread_whose_rows_fell_off_the_index_still_sends_on_its_model_effort_and_window_read_off_its_transcript() {
    drive(|| async {
        let mut h = host();
        let (ws, thread) = open_thread(&h, OPUS_1M).await;
        // Other threads in the same folder on other picks, so a value read off the wrong turn shows.
        for i in 0..3 {
            let o = StartOptions {
                harness: Some("claude".into()),
                model: Some("claude-sonnet-5".into()),
                effort: Some("max".into()),
                ..start(&format!("other {i}"))
            };
            h.fx.rt.sessions().start(&ws, o, None).await.expect("a start").finished().await;
        }
        // The state a host before each thread kept its newest row left: the thread's rows gone, its record standing.
        h.fx.rt.close().await.expect("a close");
        let mut doc = h.state.store.get("sessions", &ws).await.expect("a read").expect("a sessions doc");
        let kept: Vec<Value> =
            doc["sessions"].as_array().expect("rows").iter().filter(|s| s["threadId"] != thread.as_str()).cloned().collect();
        doc["sessions"] = json!(kept);
        h.state.store.put("sessions", &ws, doc).await.expect("a write");
        h.again();
        let rows = h.fx.rt.sessions().list(Some(ws.clone()), None).await.expect("a list");
        let row = rows.iter().find(|s| s.thread_id.as_deref() == Some(thread.as_str())).expect("the thread's row");
        assert_matches(&j(row), json!({ "id": thread, "prompt": "one" }));
        send(&h, &ws, &thread, "two", |_| {}).await;
        assert_eq!(picks(&h.claude), want(Some("claude-opus-5"), "low", Some("1m")));
        h.fx.rt.close().await.expect("a close");
    });
}

fn defaults(models: Value) -> wsp_wire::views::preferences::PreferencesPatch {
    serde_json::from_value(json!({ "agentDefaults": { "claude": { "models": { "custom": models } } } })).expect("a patch")
}

// "a model the catalog dropped since the thread's last turn is left out rather than refused, and the thread's effort still rides"
#[test]
fn a_model_the_catalog_dropped_since_the_threads_last_turn_is_left_out_rather_than_refused_and_the_threads_effort_still_rides() {
    drive(|| async {
        let h = host();
        h.fx.rt.preferences().set(defaults(json!(["claude-opus-6-preview"]))).await.expect("a set");
        let (ws, thread) =
            open_thread(&h, Open { harness: "claude", model: "claude-opus-6-preview", effort: "low", context_window: None }).await;
        h.fx.rt.preferences().set(defaults(json!([]))).await.expect("a set");
        send(&h, &ws, &thread, "two", |_| {}).await;
        assert_eq!(picks(&h.claude), want(None, "low", None));
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread's first turn still opens on the catalog's marks"
#[test]
fn a_threads_first_turn_still_opens_on_the_catalogs_marks() {
    drive(|| async {
        let h = host();
        let rt = &*h.fx.rt;
        let ws = create_on(rt, CreateOn::named("mac").on(HERE_PLACE_ID), None).await.expect("a workspace").view.id;
        rt.sessions()
            .start(&ws, StartOptions { harness: Some("claude".into()), ..start("one") }, None)
            .await
            .expect("a start")
            .finished()
            .await;
        assert_eq!(picks(&h.claude), want(Some("claude-opus-5-5"), "high", None));
        rt.close().await.expect("a close");
    });
}
