// SPDX-License-Identifier: AGPL-3.0-only
//! The first eight cases of packages/runtime/test/steer-unread.test.ts at 65f6b89c7: a message steered into a running turn that
//! its agent never read. The harness says which at the turn's end, and the runtime sends each into the thread again as its next
//! message, keeps it for the wake of a paused workspace, or tells the person where the person stopped the turn. Each test names
//! the TS test above it. The TS `memoryStore()` is the sqlite store here.
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use common::adapter::{event, result};
use common::*;
use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::port::{
    AdapterError, AttachmentRoad, ExecFailure, HarnessAdapter, HarnessSession, HarnessStartOptions, OnEvent, SharedFuture, SteerOutcome,
    TurnImage, TurnResult,
};
use wsp_core::types::api::{SteerAsk, StartOptions};
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_wire::types::attachments::Attachment;
use wsp_wire::views::session::SessionOrigin;
use wsp_wire::views::session_events::NOTIFY_ME;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_words::words::turn::unread_line;

const LINE: &str = "thread 1234abcd finished (completed): reply with the single word BANANA";
/// A one-pixel PNG.
const DOT_BYTES: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

fn dot() -> Attachment {
    Attachment { media_type: "image/png".into(), bytes: DOT_BYTES.into(), name: Some("dot.png".into()) }
}

fn uuid() -> String {
    let n: u128 = rand::random();
    format!("{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}", (n >> 96) as u32, (n >> 80) as u16, (n >> 68) as u16 & 0xfff, (n >> 56) as u16 & 0xfff, n as u64 & 0xffff_ffff_ffff)
}

/// What a start was handed.
#[derive(Clone, Debug, PartialEq)]
struct Start {
    prompt: String,
    resume: Option<String>,
    images: Vec<(String, String)>,
}

struct DrivenTurn {
    session: String,
    on_event: OnEvent,
    steered: RefCell<Vec<(String, String)>>,
    over: Cell<bool>,
    tx: RefCell<Option<oneshot::Sender<TurnResult>>>,
}

impl DrivenTurn {
    /// A turn ends: the messages it never read are said unread first.
    fn end(&self, doc: Value, unread: &[&str]) {
        if self.over.replace(true) {
            return;
        }
        let done = result(doc);
        let ids: Vec<String> = unread
            .iter()
            .map(|prompt| self.steered.borrow().iter().find(|(_, p)| p == prompt).map_or_else(uuid, |(id, _)| id.clone()))
            .collect();
        if !ids.is_empty() {
            (self.on_event)(event(json!({ "type": "turn.unread", "sessionId": self.session, "ids": ids })));
        }
        let completed = j(&done)["status"] == "completed";
        (self.on_event)(event(json!({ "type": "turn.done", "sessionId": self.session, "result": serde_json::to_value(&done).expect("a result") })));
        (self.on_event)(event(json!({ "type": "session.end", "sessionId": self.session, "exitCode": if completed { 0 } else { 1 }, "sawResult": true })));
        if let Some(tx) = self.tx.borrow_mut().take() {
            let _ = tx.send(done);
        }
    }
}

/// A harness whose turns end when the case says, telling the messages steered into them unread as they end; a stop, the person's
/// or a cut's, ends the turn with every message it was steered unread. With `images` it reads them inline, on a start and on a
/// steer.
struct Driven {
    images: bool,
    starts: RefCell<Vec<Start>>,
    turns: RefCell<Vec<Rc<DrivenTurn>>>,
}

fn driven(images: bool) -> Rc<Driven> {
    Rc::new(Driven { images, starts: RefCell::default(), turns: RefCell::default() })
}

impl Driven {
    fn factory(self: &Rc<Self>) -> HarnessAdapterFactory {
        let h = self.clone();
        Arc::new(move |_ctx| Ok(Arc::new(DrivenAdapter { h: h.clone() }) as Arc<dyn HarnessAdapter>))
    }
    fn starts(&self) -> Vec<Start> {
        self.starts.borrow().clone()
    }
    fn turn(&self, n: usize) -> Rc<DrivenTurn> {
        self.turns.borrow()[n].clone()
    }
}

struct DrivenAdapter {
    h: Rc<Driven>,
}

impl HarnessAdapter for DrivenAdapter {
    fn steers(&self) -> bool {
        true
    }
    fn steers_images(&self) -> bool {
        self.h.images
    }
    fn attachments(&self) -> Option<AttachmentRoad> {
        self.h.images.then_some(AttachmentRoad::Inline)
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        self.h.starts.borrow_mut().push(Start {
            prompt: o.prompt.clone(),
            resume: o.resume.clone(),
            images: o.images.iter().map(|i| (i.media_type.clone(), i.bytes.clone())).collect(),
        });
        let session = o.resume.clone().unwrap_or_else(uuid);
        let on_event = o.on_event.clone().expect("an event sink");
        let (tx, rx) = oneshot::channel::<TurnResult>();
        let turn = Rc::new(DrivenTurn {
            session: session.clone(),
            on_event: on_event.clone(),
            steered: RefCell::default(),
            over: Cell::new(false),
            tx: RefCell::new(Some(tx)),
        });
        self.h.turns.borrow_mut().push(turn.clone());
        on_event(event(json!({ "type": "session.start", "sessionId": session, "model": "claude-sonnet-4-5" })));
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn's end") }.boxed_local();
        Ok(Arc::new(DrivenSession { local_id: session, finished: finished.shared(), turn }))
    }
}

struct DrivenSession {
    local_id: String,
    finished: SharedFuture<TurnResult>,
    turn: Rc<DrivenTurn>,
}

impl HarnessSession for DrivenSession {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn steer(
        &self,
        prompt: String,
        id: Option<String>,
        _images: Vec<TurnImage>,
    ) -> Option<LocalBoxFuture<'static, Result<SteerOutcome, ExecFailure>>> {
        self.turn.steered.borrow_mut().push((id.unwrap_or_else(uuid), prompt));
        Some(async { Ok(SteerOutcome::Accepted) }.boxed_local())
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        let turn = self.turn.clone();
        async move {
            let prompts: Vec<String> = turn.steered.borrow().iter().map(|(_, p)| p.clone()).collect();
            let prompts: Vec<&str> = prompts.iter().map(String::as_str).collect();
            turn.end(json!({ "status": "interrupted" }), &prompts);
        }
        .boxed_local()
    }
}

fn start(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

fn steer(prompt: &str, request_id: Option<&str>, attachments: Option<Vec<Attachment>>) -> SteerAsk {
    SteerAsk { prompt: prompt.into(), request_id: request_id.map(str::to_string), attachments }
}

fn failed(error: &str) -> Value {
    json!({ "status": "failed", "error": error })
}

fn plain(prompt: &str, resume: &Option<String>) -> Start {
    Start { prompt: prompt.into(), resume: resume.clone(), images: vec![] }
}

async fn history(fx: &Fx, ws: &str) -> Vec<Value> {
    fx.rt.sessions().history(ws, None).await.expect("a history").iter().map(j).collect()
}

fn starts_are(h: &Rc<Driven>, n: usize) -> impl std::future::Future<Output = ()> + '_ {
    until(move || async move { h.starts().len() == n })
}

struct Rt {
    fx: Fx,
    ws: String,
}

async fn one(h: &Rc<Driven>) -> Rt {
    let backend = StubBackend::new(None);
    let fx = runtime(&backend).adapter("claude", h.factory()).build();
    let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
    Rt { fx, ws }
}

// "a steered message its agent never read" > "goes into the thread again as its next message once the turn is over, when its process went without reading it"
#[test]
fn goes_into_the_thread_again_as_its_next_message_once_the_turn_is_over_when_its_process_went_without_reading_it() {
    drive(|| async {
        let h = driven(false);
        let r = one(&h).await;
        let lead = r.fx.rt.sessions().start(&r.ws, start("orchestrate"), None).await.expect("a start");
        let taken = r.fx.rt.sessions().steer(&lead.id(), steer(LINE, None, None), None).await.expect("a steer");
        assert_eq!(j(&taken), json!({ "outcome": "accepted" }));
        h.turn(0).end(failed("claude exited with code 137"), &[LINE]);
        starts_are(&h, 2).await;
        assert_eq!(h.starts()[1], plain(LINE, &Some(h.turn(0).session.clone())));
        h.turn(1).end(json!({ "status": "completed", "text": "BANANA" }), &[]);
        r.fx.rt.close().await.expect("a close");
    });
}

// "a steered message its agent never read" > "goes again with the images it carried, read from what the host kept of its steer"
#[test]
fn goes_again_with_the_images_it_carried_read_from_what_the_host_kept_of_its_steer() {
    drive(|| async {
        let h = driven(true);
        let r = one(&h).await;
        let lead = r.fx.rt.sessions().start(&r.ws, start("orchestrate"), None).await.expect("a start");
        let taken = r
            .fx
            .rt
            .sessions()
            .steer(&lead.id(), steer("what colour is this?", Some("req_2"), Some(vec![dot()])), None)
            .await
            .expect("a steer");
        assert_eq!(j(&taken), json!({ "outcome": "accepted" }));
        h.turn(0).end(failed("claude exited with code 137"), &["what colour is this?"]);
        starts_are(&h, 2).await;
        assert_eq!(
            h.starts()[1],
            Start { prompt: "what colour is this?".into(), resume: Some(h.turn(0).session.clone()), images: vec![("image/png".into(), DOT_BYTES.into())] }
        );
        // The turn it opens names the image too, kept again under its own request, so the thread draws it on that row.
        let starts: Vec<Value> = history(&r.fx, &r.ws).await.into_iter().filter(|e| e["type"] == "session.start").collect();
        let last = starts.last().expect("a start").clone();
        assert_eq!(last["attachments"], json!([{ "mediaType": "image/png", "bytes": 70, "name": "dot.png" }]));
        let kept = r
            .fx
            .rt
            .sessions()
            .attachment(&r.ws, last["threadId"].as_str().expect("a thread"), last["requestId"].as_str().expect("a request"), 0, None)
            .await
            .expect("the image");
        assert_eq!(j(&kept), json!({ "mediaType": "image/png", "bytes": DOT_BYTES }));
        h.turn(1).end(json!({ "status": "completed", "text": "a dot" }), &[]);
        r.fx.rt.close().await.expect("a close");
    });
}

// "a steered message its agent never read" > "is told to the person, and not sent again, when the person stopped the turn"
#[test]
fn is_told_to_the_person_and_not_sent_again_when_the_person_stopped_the_turn() {
    drive(|| async {
        let h = driven(false);
        let r = one(&h).await;
        let lead = r.fx.rt.sessions().start(&r.ws, start("orchestrate"), None).await.expect("a start");
        r.fx.rt.sessions().steer(&lead.id(), steer(LINE, None, None), None).await.expect("a steer");
        let stopped = r.fx.rt.sessions().interrupt(&lead.id(), None, None).await.expect("an interrupt");
        assert_eq!(j(&stopped)["outcome"], "accepted");
        until(|| async { history(&r.fx, &r.ws).await.iter().any(|e| e["type"] == "session.notify") }).await;
        let told = history(&r.fx, &r.ws).await.into_iter().find(|e| e["type"] == "session.notify").expect("a notify");
        assert_matches(&told, json!({ "threadId": j(&lead.view().thread_id), "turnId": lead.turn_id(), "notify": NOTIFY_ME, "text": unread_line(LINE) }));
        sleep_ms(30).await;
        assert_eq!(h.starts().len(), 1);
        r.fx.rt.close().await.expect("a close");
    });
}

// "a steered message its agent never read" > "goes back as whoever steered it, and to the person where that road starts no turn of its own"
#[test]
fn goes_back_as_whoever_steered_it_and_to_the_person_where_that_road_starts_no_turn_of_its_own() {
    drive(|| async {
        let h = driven(false);
        let r = one(&h).await;
        let lead = r.fx.rt.sessions().start(&r.ws, start("orchestrate"), None).await.expect("a start");
        // A computer the person paired takes no start, by the list both doors read, so the line cannot go again from there.
        let paired = Some(Caller::Road(WorkspaceOrigin::Paired));
        let taken = r.fx.rt.sessions().steer(&lead.id(), steer(LINE, None, None), paired).await.expect("a steer");
        assert_eq!(j(&taken), json!({ "outcome": "accepted" }));
        h.turn(0).end(failed("claude exited with code 137"), &[LINE]);
        until(|| async { history(&r.fx, &r.ws).await.iter().any(|e| e["type"] == "session.notify") }).await;
        let told = history(&r.fx, &r.ws).await.into_iter().find(|e| e["type"] == "session.notify").expect("a notify");
        assert_matches(&told, json!({ "threadId": j(&lead.view().thread_id), "notify": NOTIFY_ME, "text": unread_line(LINE) }));
        assert_eq!(h.starts().len(), 1);
        r.fx.rt.close().await.expect("a close");
    });
}

// "a steered message its agent never read" > "goes back opened by whoever opened it: a person's message as the person's, a child's line as an agent's"
#[test]
fn goes_back_opened_by_whoever_opened_it_a_persons_message_as_the_persons_a_childs_line_as_an_agents() {
    drive(|| async {
        let h = driven(false);
        let r = one(&h).await;
        let lead = r.fx.rt.sessions().start(&r.ws, start("orchestrate"), None).await.expect("a start");
        let thread_id = lead.view().thread_id.expect("a thread");
        r.fx.rt.sessions().steer(&lead.id(), steer("also add a test", None, None), None).await.expect("a steer");
        h.turn(0).end(failed("claude exited with code 137"), &["also add a test"]);
        starts_are(&h, 2).await;
        let opts = StartOptions { thread: Some(thread_id.clone()), started_by: Some(SessionOrigin::Agent), ..start(LINE) };
        let joined = r.fx.rt.sessions().start(&r.ws, opts, None).await.expect("a start");
        assert_eq!(j(&joined.outcome()), "steered");
        h.turn(1).end(failed("claude exited with code 137"), &[LINE]);
        starts_are(&h, 3).await;
        let opened: Vec<(Value, Value)> = history(&r.fx, &r.ws)
            .await
            .iter()
            .filter(|e| e["type"] == "session.start" && e["threadId"] == thread_id.as_str())
            .map(|e| (e["prompt"].clone(), e["startedBy"].clone()))
            .collect();
        assert_eq!(opened[1..], [(json!("also add a test"), json!("person")), (json!(LINE), json!("agent"))]);
        h.turn(2).end(json!({ "status": "completed", "text": "BANANA" }), &[]);
        r.fx.rt.close().await.expect("a close");
    });
}

// "a steered message its agent never read" > "held for the wake of a napping workspace, outlives a host restart during the nap"
#[test]
fn held_for_the_wake_of_a_napping_workspace_outlives_a_host_restart_during_the_nap() {
    drive(|| async {
        let h = driven(false);
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let ws = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let lead = rt1.rt.sessions().start(&ws, start("orchestrate"), None).await.expect("a start");
        rt1.rt.sessions().steer(&lead.id(), steer(LINE, None, None), None).await.expect("a steer");
        rt1.rt.workspaces().nap(&ws, None).await.expect("a nap");
        sleep_ms(30).await;
        assert_eq!(h.starts().len(), 1);
        rt1.rt.close().await.expect("a close");

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        rt2.rt.sessions().list(Some(ws.clone()), None).await.expect("a list");
        rt2.rt.workspaces().wake(&ws, None).await.expect("a wake");
        starts_are(&h, 2).await;
        assert_eq!(h.starts()[1], plain(LINE, &Some(h.turn(0).session.clone())));
        h.turn(1).end(json!({ "status": "completed", "text": "BANANA" }), &[]);
        rt2.rt.close().await.expect("a close");
    });
}

// "a steered message its agent never read" > "held for the wake, keeps its images across a host restart"
#[test]
fn held_for_the_wake_keeps_its_images_across_a_host_restart() {
    drive(|| async {
        let h = driven(true);
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let ws = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let lead = rt1.rt.sessions().start(&ws, start("orchestrate"), None).await.expect("a start");
        rt1.rt.sessions().steer(&lead.id(), steer("what colour is this?", Some("req_2"), Some(vec![dot()])), None).await.expect("a steer");
        rt1.rt.workspaces().nap(&ws, None).await.expect("a nap");
        sleep_ms(30).await;
        assert_eq!(h.starts().len(), 1);
        rt1.rt.close().await.expect("a close");

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        rt2.rt.sessions().list(Some(ws.clone()), None).await.expect("a list");
        rt2.rt.workspaces().wake(&ws, None).await.expect("a wake");
        starts_are(&h, 2).await;
        assert_eq!(
            h.starts()[1],
            Start { prompt: "what colour is this?".into(), resume: Some(h.turn(0).session.clone()), images: vec![("image/png".into(), DOT_BYTES.into())] }
        );
        h.turn(1).end(json!({ "status": "completed", "text": "a dot" }), &[]);
        rt2.rt.close().await.expect("a close");
    });
}

// "a steered message its agent never read" > "waits for the wake of a workspace whose nap cut the turn, and goes then as the thread's next message"
#[test]
fn waits_for_the_wake_of_a_workspace_whose_nap_cut_the_turn_and_goes_then_as_the_threads_next_message() {
    drive(|| async {
        let h = driven(false);
        let r = one(&h).await;
        let lead = r.fx.rt.sessions().start(&r.ws, start("orchestrate"), None).await.expect("a start");
        r.fx.rt.sessions().steer(&lead.id(), steer(LINE, None, None), None).await.expect("a steer");
        r.fx.rt.workspaces().nap(&r.ws, None).await.expect("a nap");
        sleep_ms(30).await;
        assert_eq!(h.starts().len(), 1);
        r.fx.rt.workspaces().wake(&r.ws, None).await.expect("a wake");
        starts_are(&h, 2).await;
        assert_eq!(h.starts()[1], plain(LINE, &Some(h.turn(0).session.clone())));
        h.turn(1).end(json!({ "status": "completed", "text": "BANANA" }), &[]);
        r.fx.rt.close().await.expect("a close");
    });
}

