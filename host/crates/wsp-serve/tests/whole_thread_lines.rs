// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/thread-lines.test.ts at 65f6b89c7, case for case. What a thread's listing says of how its turns
//! ended, all the way through the runtime. The harness is a fake the test ends one way or another. Each test names the
//! TS test above it.
mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use common::adapter::{event, result};
use common::*;
use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::port::{AdapterError, HarnessAdapter, HarnessSession, HarnessStartOptions, SharedFuture, TurnResult};
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::internal::RESTARTED_REASON;
use wsp_serve::AssembledRuntime;
use wsp_store::store::Store;
use wsp_wire::views::session::SessionView;
use wsp_words::views::session::fold_threads;
use wsp_words::words::turn::{LISTED_LINE_CHARS, NO_RESULT_LINE};

struct Turn {
    on_event: wsp_adapters::port::OnEvent,
    session: String,
    settle: RefCell<Option<oneshot::Sender<TurnResult>>>,
}

/// A harness whose turns end however the test says, and whose next start throws where `refuse` names a reason.
#[derive(Default)]
struct Hand {
    turns: RefCell<Vec<Rc<Turn>>>,
    refuse: RefCell<Option<String>>,
    minted: std::cell::Cell<u32>,
}

impl Hand {
    fn new() -> Rc<Hand> {
        Rc::default()
    }

    fn factory(self: &Rc<Self>) -> HarnessAdapterFactory {
        let hand = self.clone();
        Arc::new(move |_ctx| Ok(Arc::new(Ended(hand.clone())) as Arc<dyn HarnessAdapter>))
    }

    fn count(&self) -> usize {
        self.turns.borrow().len()
    }

    /// `turns[n].end(result)`.
    fn end(&self, n: usize, doc: Value) {
        let turn = self.turns.borrow()[n].clone();
        let r = result(doc.clone());
        (turn.on_event)(event(json!({ "type": "turn.done", "sessionId": turn.session, "result": doc })));
        (turn.on_event)(event(json!({ "type": "session.end", "sessionId": turn.session, "exitCode": 0, "sawResult": true })));
        let tx = turn.settle.borrow_mut().take();
        if let Some(tx) = tx {
            let _ = tx.send(r);
        }
    }

    /// `turns[n].die()`.
    fn die(&self, n: usize) {
        let turn = self.turns.borrow()[n].clone();
        (turn.on_event)(event(json!({ "type": "session.end", "sessionId": turn.session, "exitCode": 1, "sawResult": false })));
        let tx = turn.settle.borrow_mut().take();
        if let Some(tx) = tx {
            let _ = tx.send(result(json!({ "status": "failed" })));
        }
    }

    fn refuse_with(&self, why: Option<&str>) {
        *self.refuse.borrow_mut() = why.map(str::to_string);
    }
}

struct Ended(Rc<Hand>);

impl HarnessAdapter for Ended {
    fn steers(&self) -> bool {
        false
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        if let Some(why) = self.0.refuse.borrow().clone() {
            return Err(AdapterError(why));
        }
        self.0.minted.set(self.0.minted.get() + 1);
        let session = o.resume.clone().unwrap_or_else(|| format!("sess-{}", self.0.minted.get()));
        let (tx, rx) = oneshot::channel::<TurnResult>();
        let on_event = o.on_event.clone().expect("an event sink");
        let announce = on_event.clone();
        let announced = session.clone();
        tokio::task::spawn_local(async move {
            announce(event(json!({ "type": "session.start", "sessionId": announced, "cwd": "/root/work" })));
        });
        self.0.turns.borrow_mut().push(Rc::new(Turn { on_event, session: session.clone(), settle: RefCell::new(Some(tx)) }));
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn was never ended") }.boxed_local();
        Ok(Arc::new(Quiet { local_id: session, finished: finished.shared() }))
    }
}

struct Quiet {
    local_id: String,
    finished: SharedFuture<TurnResult>,
}

impl HarnessSession for Quiet {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
}

struct Begun {
    fx: Fx,
    ws: String,
}

fn start(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

async fn begin(h: &Rc<Hand>, store: &Rc<dyn Store>, backend: &Arc<StubBackend>) -> Begun {
    let fx = runtime(backend).store(store).adapter("claude", h.factory()).build();
    let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
    fx.rt.sessions().start(&ws, start("build it"), None).await.expect("a start");
    until(|| async { h.count() == 1 }).await;
    until(|| async { fx.rt.sessions().history(&ws, None).await.expect("a history").iter().any(|e| j(e)["type"] == "session.start") }).await;
    Begun { fx, ws }
}

async fn rows(rt: &AssembledRuntime, ws: &str) -> Vec<SessionView> {
    rt.sessions().list(Some(ws.to_string()), None).await.expect("a list")
}

async fn latest(rt: &AssembledRuntime, ws: &str) -> Value {
    j(rows(rt, ws).await.last().expect("a row"))
}

async fn settled_as(rt: &AssembledRuntime, ws: &str, status: &str) {
    until(|| async { latest(rt, ws).await["status"] == status }).await;
}

async fn first_thread(rt: &AssembledRuntime, ws: &str) -> Value {
    j(&fold_threads(&rows(rt, ws).await).into_iter().next().expect("a thread"))
}

fn is_number(v: &Value) -> bool {
    v.is_number()
}

// "how a thread's turns ended, on its listing" > "a turn that completes leaves its reply's last line on its row and its thread, cut at 200 characters"
#[test]
fn a_turn_that_completes_leaves_its_replys_last_line_on_its_row_and_its_thread_cut_at_200_characters() {
    drive(|| async {
        let h = Hand::new();
        let state = sqlite_state();
        let b = begin(&h, &state.store, &StubBackend::new(None)).await;
        let rt = &*b.fx.rt;
        let last = format!("{}and pushed.", "done ".repeat(60));
        h.end(0, json!({ "status": "completed", "text": format!("Read the ticket.\n\n{last}\n") }));
        settled_as(rt, &b.ws, "completed").await;
        let row = latest(rt, &b.ws).await;
        let line = row["lastLine"].as_str().expect("a last line");
        assert!(line.encode_utf16().count() <= LISTED_LINE_CHARS);
        let cut: String = line.chars().take(line.chars().count() - 1).collect();
        assert!(last.starts_with(&cut));
        assert!(row.get("failure").is_none());
        assert_matches(&first_thread(rt, &b.ws).await, json!({ "lastLine": line }));
        rt.close().await.expect("a close");
    });
}

// "a turn that ends failed leaves its reason, and the next turn that completes clears it"
#[test]
fn a_turn_that_ends_failed_leaves_its_reason_and_the_next_turn_that_completes_clears_it() {
    drive(|| async {
        let h = Hand::new();
        let state = sqlite_state();
        let b = begin(&h, &state.store, &StubBackend::new(None)).await;
        let rt = &*b.fx.rt;
        h.end(0, json!({ "status": "failed", "error": "API Error: 529 overloaded\nretry in a minute" }));
        settled_as(rt, &b.ws, "failed").await;
        assert_matches(&first_thread(rt, &b.ws).await, json!({ "status": "failed", "failure": "API Error: 529 overloaded" }));
        assert!(latest(rt, &b.ws).await.get("lastLine").is_none());
        let thread = first_thread(rt, &b.ws).await;
        rt.sessions()
            .start(&b.ws, StartOptions { thread: thread["id"].as_str().map(str::to_string), ..start("again") }, None)
            .await
            .expect("a start");
        until(|| async { h.count() == 2 }).await;
        h.end(1, json!({ "status": "completed", "text": "Fixed it." }));
        settled_as(rt, &b.ws, "completed").await;
        let after = first_thread(rt, &b.ws).await;
        assert_matches(&after, json!({ "lastLine": "Fixed it." }));
        assert!(after.get("failure").is_none());
        rt.close().await.expect("a close");
    });
}

// "a turn whose agent exited with no result says so as its failure"
#[test]
fn a_turn_whose_agent_exited_with_no_result_says_so_as_its_failure() {
    drive(|| async {
        let h = Hand::new();
        let state = sqlite_state();
        let b = begin(&h, &state.store, &StubBackend::new(None)).await;
        let rt = &*b.fx.rt;
        h.die(0);
        settled_as(rt, &b.ws, "failed").await;
        assert_matches(&latest(rt, &b.ws).await, json!({ "failure": NO_RESULT_LINE }));
        rt.close().await.expect("a close");
    });
}

// "a turn cut by a host restart carries the cut's reason as its failure"
#[test]
fn a_turn_cut_by_a_host_restart_carries_the_cuts_reason_as_its_failure() {
    drive(|| async {
        let h = Hand::new();
        let state = sqlite_state();
        let backend = StubBackend::new(None);
        let b = begin(&h, &state.store, &backend).await;
        b.fx.rt.close().await.expect("a close");
        // The fake's turn names no run on its machine, so the next host has nothing to re-open and cuts it.
        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        settled_as(&rt2.rt, &b.ws, "failed").await;
        assert_matches(&first_thread(&rt2.rt, &b.ws).await, json!({ "failure": RESTARTED_REASON }));
        rt2.rt.close().await.expect("a close");
    });
}

// "a send its sender hears refused, or whose agent fails to start while the sender waits, writes no failure on the thread"
#[test]
fn a_send_its_sender_hears_refused_or_whose_agent_fails_to_start_while_the_sender_waits_writes_no_failure_on_the_thread() {
    drive(|| async {
        let h = Hand::new();
        let state = sqlite_state();
        let b = begin(&h, &state.store, &StubBackend::new(None)).await;
        let rt = &*b.fx.rt;
        h.end(0, json!({ "status": "completed", "text": "Built it." }));
        settled_as(rt, &b.ws, "completed").await;
        let thread = first_thread(rt, &b.ws).await;
        let id = thread["id"].as_str().map(str::to_string);
        let refused = |r: Result<_, wsp_words::exit::HostError>| match r {
            Ok(_) => panic!("the send was not refused"),
            Err(e) => e.message,
        };
        let said = refused(
            rt.sessions().start(&b.ws, StartOptions { thread: id.clone(), effort: Some("galactic".into()), ..start("again") }, None).await,
        );
        assert!(said.contains("galactic"), "{said}");
        h.refuse_with(Some("claude: command not found\nexit 127"));
        let said = refused(rt.sessions().start(&b.ws, StartOptions { thread: id.clone(), ..start("again") }, None).await);
        assert!(said.contains("command not found"), "{said}");
        let after = first_thread(rt, &b.ws).await;
        assert_matches(&after, json!({ "status": "completed", "lastLine": "Built it." }));
        assert!(after.get("failure").is_none());
        rt.close().await.expect("a close");
    });
}

// "a folded thread lists its fold and keeps it across a host restart, and opening it clears the mark"
#[test]
fn a_folded_thread_lists_its_fold_and_keeps_it_across_a_host_restart_and_opening_it_clears_the_mark() {
    drive(|| async {
        let h = Hand::new();
        let state = sqlite_state();
        let backend = StubBackend::new(None);
        let b = begin(&h, &state.store, &backend).await;
        let rt = &*b.fx.rt;
        h.end(0, json!({ "status": "completed", "text": "ok" }));
        settled_as(rt, &b.ws, "completed").await;
        let thread = first_thread(rt, &b.ws).await;
        let id = thread["id"].as_str().expect("an id").to_string();
        rt.sessions()
            .mark(vec![id.clone()], serde_json::from_value(json!({ "folded": true })).expect("marks"), None)
            .await
            .expect("a mark");
        let folded = first_thread(rt, &b.ws).await;
        assert!(is_number(&folded["foldedAt"]));
        assert_eq!(latest(rt, &b.ws).await["foldedAt"], folded["foldedAt"]);
        rt.close().await.expect("a close");

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        assert_eq!(first_thread(&rt2.rt, &b.ws).await["foldedAt"], folded["foldedAt"]);
        rt2.rt.sessions().mark(vec![id], serde_json::from_value(json!({ "folded": false })).expect("marks"), None).await.expect("a mark");
        assert!(first_thread(&rt2.rt, &b.ws).await.get("foldedAt").is_none());
        rt2.rt.close().await.expect("a close");
    });
}

fn shut(patch: Value) -> wsp_wire::views::preferences::PreferencesPatch {
    serde_json::from_value(json!({ "threadsShut": patch })).expect("a patch")
}

// "keeps a lead's shut Threads section on the preferences record apart from its fold, and a write drops a lead the host does not hold"
#[test]
fn keeps_a_leads_shut_threads_section_on_the_preferences_record_apart_from_its_fold_and_a_write_drops_a_lead_the_host_does_not_hold() {
    drive(|| async {
        let h = Hand::new();
        let state = sqlite_state();
        let b = begin(&h, &state.store, &StubBackend::new(None)).await;
        let rt = &*b.fx.rt;
        h.end(0, json!({ "status": "completed", "text": "ok" }));
        settled_as(rt, &b.ws, "completed").await;
        let lead = first_thread(rt, &b.ws).await["id"].as_str().expect("an id").to_string();
        let set = rt.preferences().set(shut(json!({ lead.clone(): true, "thr_gone": true }))).await.expect("a set");
        assert_eq!(j(&set.preferences)["threadsShut"], json!({ lead.clone(): true }));
        assert!(first_thread(rt, &b.ws).await.get("foldedAt").is_none());
        let set = rt.preferences().set(shut(json!({ lead.clone(): null }))).await.expect("a set");
        assert_eq!(j(&set.preferences)["threadsShut"], json!({}));
        rt.close().await.expect("a close");
    });
}

// "keeps a shut Threads section for a lead whose rows fell off the index cap, while its thread record stands"
#[test]
fn keeps_a_shut_threads_section_for_a_lead_whose_rows_fell_off_the_index_cap_while_its_thread_record_stands() {
    drive(|| async {
        let h = Hand::new();
        let state = sqlite_state();
        let backend = StubBackend::new(None);
        let b = begin(&h, &state.store, &backend).await;
        let rt = &*b.fx.rt;
        h.end(0, json!({ "status": "completed", "text": "ok" }));
        settled_as(rt, &b.ws, "completed").await;
        let lead = first_thread(rt, &b.ws).await;
        rt.close().await.expect("a close");
        // The state the cap leaves: no row of the thread in the index, its record still kept.
        let mut doc = state.store.get("sessions", &b.ws).await.expect("a read").expect("a sessions doc");
        doc["sessions"] = json!([]);
        state.store.put("sessions", &b.ws, doc).await.expect("a write");
        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        // Listed off its record, its row gone.
        let listed: Vec<(String, Option<String>)> =
            rows(&rt2.rt, &b.ws).await.iter().map(|s| (s.id.clone(), s.thread_id.clone())).collect();
        let want = lead["threadId"].as_str().expect("a thread id").to_string();
        assert_eq!(listed, [(want.clone(), Some(want))]);
        let id = lead["id"].as_str().expect("an id").to_string();
        let set = rt2.rt.preferences().set(shut(json!({ id.clone(): true, "thr_gone": true }))).await.expect("a set");
        assert_eq!(j(&set.preferences)["threadsShut"], json!({ id: true }));
        rt2.rt.close().await.expect("a close");
    });
}
