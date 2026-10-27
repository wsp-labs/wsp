// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/session-images.test.ts at 65f6b89c7, case for case. What a message's images come to on each of the
//! two roads an adapter can declare, and what a message with an image to an agent that declares neither comes to. Nothing
//! here is a cloud: the backend is the in-process stub, whose upload URLs point at a loopback server that records every
//! body, so the file road can be read byte for byte.
#[macro_use]
mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use base64::Engine as _;
use common::*;
use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::port::{
    AdapterError, AttachmentRoad, ExecFailure, HarnessAdapter, HarnessSession, HarnessStartOptions, OnEvent, SharedFuture, SteerOutcome,
    TurnImage, TurnResult,
};
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::{HarnessAdapterContext, HarnessAdapterFactory};
use wsp_store::store::Store;
use wsp_wire::types::attachments::Attachment;
use wsp_words::attachments::{
    attached_files_prompt, drop_files_line, files_not_landed_line, image_path_in, land_files_line, no_images_line, turn_images_dir,
};
use wsp_words::exit::Kind;

const SESSION_ID: &str = "11111111-1111-4111-8111-111111111111";

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// `recording`: one completed turn per start, printed at once.
    Record,
    /// `failing` and `neverRan`: a failed done and an end, with no `session.start`.
    Failing(&'static str),
    /// `writer`: a turn that writes a large file, as tool calls do.
    Writer,
    /// `heldAdapter` and `lingering`: a turn that ends only when the case says so.
    Manual,
}

struct Begun {
    prompt: String,
    resume: Option<String>,
    images: Vec<TurnImage>,
}

struct Turn {
    on_event: OnEvent,
    finish: RefCell<Option<oneshot::Sender<TurnResult>>>,
}

/// One adapter that records what it was started with, on whichever road the case is about; no road at all when the case is
/// an agent that reads no image.
struct Hand {
    mode: Mode,
    road: Option<AttachmentRoad>,
    steers: bool,
    starts: RefCell<Vec<Begun>>,
    turns: RefCell<Vec<Rc<Turn>>>,
    steered: RefCell<Vec<String>>,
}

fn hand(mode: Mode, road: Option<AttachmentRoad>, steers: bool) -> Rc<Hand> {
    Rc::new(Hand { mode, road, steers, starts: RefCell::default(), turns: RefCell::default(), steered: RefCell::default() })
}

fn recording(road: Option<AttachmentRoad>) -> Rc<Hand> {
    hand(Mode::Record, road, false)
}

fn done() -> TurnResult {
    serde_json::from_value(json!({ "status": "completed", "text": "done" })).expect("a result")
}

impl Hand {
    fn factory(self: &Rc<Self>) -> HarnessAdapterFactory {
        let me = self.clone();
        Arc::new(move |_ctx: HarnessAdapterContext| Ok(Arc::new(HandAdapter { me: me.clone() }) as Arc<dyn HarnessAdapter>))
    }

    fn prompts(&self) -> Vec<String> {
        self.starts.borrow().iter().map(|s| s.prompt.clone()).collect()
    }

    /// `end(nth)` of `heldAdapter`: the nth turn replies and exits.
    fn end(&self, nth: usize) {
        self.reply(nth);
        self.exit(nth);
    }

    /// `reply()` of `lingering`: turn.done lands while the process keeps running.
    fn reply(&self, nth: usize) {
        let t = self.turns.borrow()[nth].clone();
        (t.on_event)(ev(json!({ "type": "turn.done", "sessionId": SESSION_ID, "result": { "status": "completed", "text": "done" } })));
    }

    fn exit(&self, nth: usize) {
        let t = self.turns.borrow()[nth].clone();
        (t.on_event)(ev(json!({ "type": "session.end", "sessionId": SESSION_ID, "exitCode": 0, "sawResult": true })));
        let tx = t.finish.borrow_mut().take();
        if let Some(tx) = tx {
            let _ = tx.send(done());
        }
    }

    fn last(&self) -> usize {
        self.turns.borrow().len() - 1
    }
}

fn ev(doc: Value) -> wsp_adapters::port::AdapterEvent {
    serde_json::from_value(doc).expect("an adapter event")
}

struct HandAdapter {
    me: Rc<Hand>,
}

impl HarnessAdapter for HandAdapter {
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let me = &self.me;
        me.starts.borrow_mut().push(Begun { prompt: o.prompt.clone(), resume: o.resume.clone(), images: o.images.clone() });
        let on_event = o.on_event.clone().expect("an event sink");
        let (tx, rx) = oneshot::channel();
        let turn = Rc::new(Turn { on_event: on_event.clone(), finish: RefCell::new(None) });
        match me.mode {
            Mode::Record => {
                for line in [
                    json!({ "type": "session.start", "sessionId": SESSION_ID }),
                    json!({ "type": "turn.done", "sessionId": SESSION_ID, "result": { "status": "completed", "text": "done" } }),
                    json!({ "type": "session.end", "sessionId": SESSION_ID, "exitCode": 0, "sawResult": true }),
                ] {
                    on_event(ev(line));
                }
                let _ = tx.send(done());
            }
            Mode::Failing(error) => {
                on_event(ev(json!({ "type": "turn.done", "sessionId": SESSION_ID, "result": { "status": "failed", "error": error } })));
                on_event(ev(json!({ "type": "session.end", "sessionId": SESSION_ID, "exitCode": 1, "sawResult": false })));
                let _ = tx.send(serde_json::from_value(json!({ "status": "failed", "error": error })).expect("a result"));
            }
            Mode::Writer => {
                on_event(ev(json!({ "type": "session.start", "sessionId": SESSION_ID })));
                if o.prompt != "look" {
                    on_event(ev(
                        json!({ "type": "turn.delta", "sessionId": SESSION_ID, "kind": "tool_use", "text": "w".repeat(256 * 1024), "toolName": "Write", "toolUseId": "tu_1" }),
                    ));
                }
                on_event(ev(json!({ "type": "turn.done", "sessionId": SESSION_ID, "result": { "status": "completed", "text": "done" } })));
                on_event(ev(json!({ "type": "session.end", "sessionId": SESSION_ID, "exitCode": 0, "sawResult": true })));
                let _ = tx.send(done());
            }
            Mode::Manual => {
                on_event(ev(json!({ "type": "session.start", "sessionId": SESSION_ID })));
                *turn.finish.borrow_mut() = Some(tx);
                me.turns.borrow_mut().push(turn.clone());
            }
        }
        let finished: LocalBoxFuture<'static, TurnResult> = async move {
            match rx.await {
                Ok(r) => r,
                Err(_) => futures::future::pending().await,
            }
        }
        .boxed_local();
        Ok(Arc::new(HandSession { me: me.clone(), finished: finished.shared() }))
    }

    fn steers(&self) -> bool {
        self.me.steers
    }

    fn attachments(&self) -> Option<AttachmentRoad> {
        self.me.road
    }
}

struct HandSession {
    me: Rc<Hand>,
    finished: SharedFuture<TurnResult>,
}

impl HarnessSession for HandSession {
    fn local_id(&self) -> String {
        SESSION_ID.to_string()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
    fn steer(
        &self,
        prompt: String,
        _id: Option<String>,
        _images: Vec<TurnImage>,
    ) -> Option<LocalBoxFuture<'static, Result<SteerOutcome, ExecFailure>>> {
        if !self.me.steers {
            return None;
        }
        self.me.steered.borrow_mut().push(prompt);
        Some(async { Ok(SteerOutcome::Accepted) }.boxed_local())
    }
}

fn bytes_of(fill: u8, size: usize) -> String {
    base64::engine::general_purpose::STANDARD.encode(vec![fill; size])
}

fn att(media: &str, bytes: String, name: Option<&str>) -> Attachment {
    Attachment { media_type: media.into(), bytes, name: name.map(str::to_string) }
}

fn png(fill: u8) -> Attachment {
    att("image/png", bytes_of(fill, 64), None)
}

fn pdf(name: &str) -> Attachment {
    att("application/pdf", base64::engine::general_purpose::STANDARD.encode("%PDF-1.7 heading"), Some(name))
}

fn quoted(path: &str) -> String {
    format!("'{path}'")
}

struct W {
    fx: Fx,
    ws: String,
    backend: Arc<StubBackend>,
    mark_puts: usize,
    mark_execs: usize,
}

impl W {
    fn puts(&self) -> Vec<common::stub_backend::Put> {
        self.backend.puts()[self.mark_puts..].to_vec()
    }
    fn execs(&self) -> Vec<String> {
        self.backend.machine(0).exec_log()[self.mark_execs..].to_vec()
    }
    fn all_execs(&self) -> Vec<String> {
        self.backend.machine(0).exec_log()
    }
}

/// A workspace on the stub, with what the machine was already sent and told at create marked off: a create lands the daemon,
/// so what an image costs is only ever what came after.
async fn workspace_on(adapters: &[(&str, &Rc<Hand>)], backend: Arc<StubBackend>, store: Option<Rc<dyn Store>>) -> W {
    let mut w = runtime(&backend);
    if let Some(store) = &store {
        w = w.store(store);
    }
    for (name, hand) in adapters {
        w = w.adapter(name, hand.factory());
    }
    let fx = w.build();
    let ws = create_on(&fx.rt, CreateOn::golden("shots"), None).await.expect("a workspace").view.id;
    let (mark_puts, mark_execs) = (backend.puts().len(), backend.machine(0).exec_log().len());
    W { fx, ws, backend, mark_puts, mark_execs }
}

async fn plain(adapters: &[(&str, &Rc<Hand>)]) -> W {
    workspace_on(adapters, StubBackend::new(None), None).await
}

fn opts(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

fn with(prompt: &str, harness: Option<&str>, request: Option<&str>, attachments: Vec<Attachment>) -> StartOptions {
    StartOptions {
        prompt: prompt.into(),
        harness: harness.map(str::to_string),
        request_id: request.map(str::to_string),
        attachments: (!attachments.is_empty()).then_some(attachments),
        ..Default::default()
    }
}

type Handle = Rc<dyn wsp_core::types::wiring::SessionHandle>;

async fn start(w: &W, o: StartOptions) -> Handle {
    w.fx.rt.sessions().start(&w.ws, o, None).await.expect("a start")
}

fn view(h: &Handle) -> Value {
    j(&h.view())
}

fn thread_of(h: &Handle) -> String {
    view(h)["threadId"].as_str().expect("a thread").to_string()
}

fn cwd_of(h: &Handle) -> String {
    view(h)["cwd"].as_str().expect("a folder").to_string()
}

fn puts_as(puts: &[common::stub_backend::Put]) -> Vec<(String, String)> {
    puts.iter().map(|p| (p.path.clone(), String::from_utf8_lossy(&p.body).into_owned())).collect()
}

fn dir_of(folder: &str, thread: &str, request: &str) -> String {
    turn_images_dir(folder, thread, Some(request), "unused")
}

fn img_paths(h: &Hand, nth: usize) -> Vec<String> {
    h.starts.borrow()[nth].images.iter().map(|i| i.path.clone().unwrap_or_default()).collect()
}

// "a file that is not an image" > "lands in the thread's own folder, whatever road the agent reads images on, and the prompt names where"
#[test]
fn lands_in_the_threads_own_folder_whatever_road_the_agent_reads_images_on_and_the_prompt_names_where() {
    drive(|| async {
        for road in [Some(AttachmentRoad::Inline), Some(AttachmentRoad::File), None] {
            let agent = recording(road);
            let w = plain(&[("claude", &agent)]).await;
            let handle = start(&w, with("what is the first heading?", None, Some("req_a"), vec![pdf("report.pdf")])).await;
            handle.finished().await;
            let folder = cwd_of(&handle);
            let dir = format!("{folder}/.wsp-files/{}/req_a", thread_of(&handle));
            assert!(w.execs().contains(&land_files_line(&folder, &dir)));
            assert_eq!(puts_as(&w.puts()), [(format!("{dir}/report.pdf"), "%PDF-1.7 heading".to_string())]);
            assert_eq!(
                agent.starts.borrow()[0].prompt,
                attached_files_prompt("what is the first heading?", &[format!("{dir}/report.pdf")])
            );
            assert!(agent.starts.borrow()[0].images.is_empty());
            // The transcript keeps the words the person typed, not the paths the agent was handed.
            assert_eq!(view(&handle)["prompt"], "what is the first heading?");
        }
    });
}

// "a file that is not an image" > "lands under one plain name inside its send's folder, whatever name the client sent"
#[test]
fn lands_under_one_plain_name_inside_its_sends_folder_whatever_name_the_client_sent() {
    drive(|| async {
        let claude = recording(Some(AttachmentRoad::Inline));
        let w = plain(&[("claude", &claude)]).await;
        let handle =
            start(&w, with("read", None, Some("../../etc"), vec![pdf("../../.ssh/authorized_keys"), pdf("../../.ssh/authorized_keys")]))
                .await;
        handle.finished().await;
        let files = format!("{}/.wsp-files/{}/", cwd_of(&handle), thread_of(&handle));
        let paths: Vec<String> = w.puts().iter().map(|p| p.path.clone()).collect();
        assert_eq!(paths.len(), 2);
        for path in &paths {
            assert!(path.starts_with(&files), "{path}");
            assert!(!path.contains(".."));
            assert_eq!(path[files.len()..].split('/').count(), 2);
        }
        assert_eq!(
            paths.iter().map(|p| p.rsplit('/').next().unwrap_or_default().to_string()).collect::<Vec<_>>(),
            ["authorized_keys", "2-authorized_keys"]
        );
    });
}

// "a file that is not an image" > "go with the workspace when it is deleted, one removal per folder for every thread that landed there"
#[test]
fn go_with_the_workspace_when_it_is_deleted_one_removal_per_folder_for_every_thread_that_landed_there() {
    drive(|| async {
        let claude = recording(Some(AttachmentRoad::Inline));
        let w = plain(&[("claude", &claude)]).await;
        let one = start(&w, with("read", None, Some("req_a"), vec![pdf("report.pdf")])).await;
        one.finished().await;
        let two = start(&w, with("and this", None, Some("req_b"), vec![pdf("report.pdf")])).await;
        two.finished().await;
        let folder = cwd_of(&one);
        w.fx.rt.workspaces().delete(&w.ws, None).await.expect("a delete");
        let removals: Vec<String> = w.execs().into_iter().filter(|c| c.contains("rm -rf")).collect();
        assert_eq!(removals, [drop_files_line(&folder, &[thread_of(&one), thread_of(&two)])]);
    });
}

// "a file that is not an image" > "go with a thread that is forgotten, its launch having landed them before it failed"
#[test]
fn go_with_a_thread_that_is_forgotten_its_launch_having_landed_them_before_it_failed() {
    drive(|| async {
        let never_ran = hand(Mode::Failing("no sign-in"), Some(AttachmentRoad::Inline), false);
        let w = plain(&[("claude", &never_ran)]).await;
        let handle = start(&w, with("read", None, Some("req_a"), vec![pdf("report.pdf")])).await;
        handle.finished().await;
        assert!(never_ran.prompts()[0].contains("/.wsp-files/"));
        w.fx.rt.sessions().forget(&thread_of(&handle), None, None).await.expect("a forget");
        assert!(w.execs().contains(&drop_files_line(&cwd_of(&handle), &[thread_of(&handle)])));
    });
}

// "a file that is not an image" > "is refused in one line, with nothing put, where the folder could not be readied"
#[test]
fn is_refused_in_one_line_with_nothing_put_where_the_folder_could_not_be_readied() {
    drive(|| async {
        let claude = recording(Some(AttachmentRoad::Inline));
        let w = plain(&[("claude", &claude)]).await;
        let before = w.backend.exec_impl_fn();
        w.backend.set_exec_impl(Rc::new(move |m, cmd, stdin| {
            if cmd.contains(".wsp-files") {
                return Box::pin(async { Ok(exec_exit(1, "", "")) });
            }
            before(m, cmd, stdin)
        }));
        let mut o = with("read", None, Some("req_a"), vec![pdf("report.pdf")]);
        o.cwd = Some("/root/shots".into());
        let refused = w.fx.rt.sessions().start(&w.ws, o, None).await.err().expect("a refusal");
        assert!(refused.message.contains(&files_not_landed_line("/root/shots")), "{}", refused.message);
        assert!(w.puts().is_empty());
        assert!(claude.starts.borrow().is_empty());
    });
}

// "the images the host keeps" > "answer a client after a host restart, whoever sent them, and only on their own workspace and thread"
#[test]
fn answer_a_client_after_a_host_restart_whoever_sent_them_and_only_on_their_own_workspace_and_thread() {
    drive(|| async {
        let state = sqlite_state();
        let backend = StubBackend::new(None);
        let claude = recording(Some(AttachmentRoad::Inline));
        let w = workspace_on(&[("claude", &claude)], backend.clone(), Some(state.store.clone())).await;
        // A send from the command line: no client holds these bytes, and the transcript keeps the records alone.
        let handle = start(&w, with("what is this?", None, Some("req_a"), vec![png(7), pdf("report.pdf")])).await;
        handle.finished().await;
        let thread = thread_of(&handle);
        w.fx.rt.close().await.expect("a close");
        let again = runtime(&backend).store(&state.store).build();
        let kept = again.rt.sessions().attachment(&w.ws, &thread, "req_a", 0, None).await.expect("a kept image");
        assert_eq!(j(&kept), json!({ "mediaType": "image/png", "bytes": bytes_of(7, 64) }));
        // A file that is not an image draws from its record, so nothing of it is kept; nor is a send nobody named.
        for (who, req, index) in [(thread.as_str(), "req_a", 1), (thread.as_str(), "req_b", 0), ("another-thread", "req_a", 0)] {
            let refused = again.rt.sessions().attachment(&w.ws, who, req, index, None).await.expect_err("a refusal");
            assert_eq!(refused.kind, Some(Kind::NotFound), "{}", refused.message);
        }
        again.rt.close().await.expect("a close");
    });
}

// "the images the host keeps" > "keep nothing for a send that carried no request id, since no client could ask for it"
#[test]
fn keep_nothing_for_a_send_that_carried_no_request_id_since_no_client_could_ask_for_it() {
    drive(|| async {
        let state = sqlite_state();
        let claude = recording(Some(AttachmentRoad::Inline));
        let w = workspace_on(&[("claude", &claude)], StubBackend::new(None), Some(state.store.clone())).await;
        start(&w, with("unnamed", None, None, vec![png(3)])).await.finished().await;
        w.fx.rt.close().await.expect("a close");
        assert!(state.store.list("attachment-keys").await.expect("a list").is_empty());
    });
}

// "the images the host keeps" > "name an image before its bytes land, so a write cut short leaves an entry that reads as not found and is swept"
#[test]
fn name_an_image_before_its_bytes_land_so_a_write_cut_short_leaves_an_entry_that_reads_as_not_found_and_is_swept() {
    drive(|| async {
        let state = sqlite_state();
        // The host dies between the two writes: the bytes never land.
        let dying = WrappedStore::over(&state.store);
        dying.refuse_blob_puts("attachments", "the host went down");
        let claude = recording(Some(AttachmentRoad::Inline));
        let w = workspace_on(&[("claude", &claude)], StubBackend::new(None), Some(dying.store())).await;
        let handle = start(&w, with("look", None, Some("req_a"), vec![png(5)])).await;
        handle.finished().await;
        let thread = thread_of(&handle);
        let refused = w.fx.rt.sessions().attachment(&w.ws, &thread, "req_a", 0, None).await.expect_err("a refusal");
        assert_eq!(refused.kind, Some(Kind::NotFound), "{}", refused.message);
        assert_eq!(
            state.store.get("attachment-keys", &w.ws).await.expect("a read"),
            Some(json!({ "threads": { thread.clone(): [{ "key": format!("{thread}.req_a.0"), "mediaType": "image/png" }] } }))
        );
        w.fx.rt.workspaces().delete(&w.ws, None).await.expect("a delete");
        assert_eq!(state.store.get("attachment-keys", &w.ws).await.expect("a read"), None);
        w.fx.rt.close().await.expect("a close");
    });
}

// "the images the host keeps" > "go when the start that names them leaves the transcript, since no row can ask for them after"
#[test]
fn go_when_the_start_that_names_them_leaves_the_transcript_since_no_row_can_ask_for_them_after() {
    drive(|| async {
        let state = sqlite_state();
        // A turn that writes a large file, as tool calls do: twenty of them push the thread past the transcript's cap.
        let writer = hand(Mode::Writer, Some(AttachmentRoad::Inline), false);
        let w = workspace_on(&[("claude", &writer)], StubBackend::new(None), Some(state.store.clone())).await;
        let first = start(&w, with("look", None, Some("req_a"), vec![png(5)])).await;
        first.finished().await;
        let thread = thread_of(&first);
        let kept = w.fx.rt.sessions().attachment(&w.ws, &thread, "req_a", 0, None).await.expect("a kept image");
        assert_eq!(j(&kept), json!({ "mediaType": "image/png", "bytes": bytes_of(5, 64) }));
        for i in 0..20 {
            start(&w, StartOptions { thread: Some(thread.clone()), prompt: format!("write {i}"), ..Default::default() })
                .await
                .finished()
                .await;
        }
        let history = common::comeback::history(&w.fx.rt, &w.ws).await;
        assert!(!history.iter().any(|e| e["type"] == "session.start" && e["requestId"] == "req_a"));
        let refused = w.fx.rt.sessions().attachment(&w.ws, &thread, "req_a", 0, None).await.expect_err("a refusal");
        assert_eq!(refused.kind, Some(Kind::NotFound), "{}", refused.message);
        assert_eq!(state.store.get_blob("attachments", &format!("{thread}.req_a.0")).await.expect("a read"), None);
        w.fx.rt.close().await.expect("a close");
    });
}

// "the images the host keeps" > "go with the workspace when it is deleted"
#[test]
fn go_with_the_workspace_when_it_is_deleted() {
    drive(|| async {
        let state = sqlite_state();
        let claude = recording(Some(AttachmentRoad::Inline));
        let w = workspace_on(&[("claude", &claude)], StubBackend::new(None), Some(state.store.clone())).await;
        let handle = start(&w, with("look", None, Some("req_a"), vec![png(5)])).await;
        handle.finished().await;
        let key = format!("{}.req_a.0", thread_of(&handle));
        w.fx.rt.sessions().attachment(&w.ws, &thread_of(&handle), "req_a", 0, None).await.expect("a kept image");
        assert!(state.store.get_blob("attachments", &key).await.expect("a read").is_some());
        w.fx.rt.workspaces().delete(&w.ws, None).await.expect("a delete");
        assert_eq!(state.store.get_blob("attachments", &key).await.expect("a read"), None);
        assert_eq!(state.store.get("attachment-keys", &w.ws).await.expect("a read"), None);
        w.fx.rt.close().await.expect("a close");
    });
}

// "an image on the inline road" > "reaches the adapter as bytes with its type, and nothing lands on the machine"
#[test]
fn reaches_the_adapter_as_bytes_with_its_type_and_nothing_lands_on_the_machine() {
    drive(|| async {
        let claude = recording(Some(AttachmentRoad::Inline));
        let w = plain(&[("claude", &claude)]).await;
        start(&w, with("what does this show?", None, None, vec![png(7)])).await.finished().await;
        assert_eq!(claude.starts.borrow().len(), 1);
        let images: Vec<Value> = claude.starts.borrow()[0].images.iter().map(j).collect();
        assert_eq!(images, [json!({ "mediaType": "image/png", "bytes": bytes_of(7, 64) })]);
        // Nothing was written to the guest for it: an inline harness reads no file.
        assert!(w.puts().is_empty());
        assert!(!w.execs().iter().any(|c| c.contains(".images")));
    });
}

// "an image on the inline road" > "carries every image of the message, in the order the person added them"
#[test]
fn carries_every_image_of_the_message_in_the_order_the_person_added_them() {
    drive(|| async {
        let claude = recording(Some(AttachmentRoad::Inline));
        let w = plain(&[("claude", &claude)]).await;
        start(&w, with("these two", None, None, vec![png(1), att("image/webp", bytes_of(2, 64), None)])).await.finished().await;
        let starts = claude.starts.borrow();
        assert_eq!(starts[0].images.iter().map(|i| i.media_type.clone()).collect::<Vec<_>>(), ["image/png", "image/webp"]);
        assert_eq!(starts[0].images.iter().map(|i| i.bytes.clone()).collect::<Vec<_>>(), [bytes_of(1, 64), bytes_of(2, 64)]);
    });
}

// "an image on the inline road" > "a message with no image hands the adapter none, so a turn without one is the turn it always was"
#[test]
fn a_message_with_no_image_hands_the_adapter_none_so_a_turn_without_one_is_the_turn_it_always_was() {
    drive(|| async {
        let claude = recording(Some(AttachmentRoad::Inline));
        let w = plain(&[("claude", &claude)]).await;
        start(&w, opts("no image")).await.finished().await;
        assert!(claude.starts.borrow()[0].images.is_empty());
    });
}

// "an image on the file road" > "lands in this send's own folder among the thread's files, in the folder the thread works in, and the adapter is handed the path"
#[test]
fn lands_in_this_sends_own_folder_among_the_threads_files_in_the_folder_the_thread_works_in_and_the_adapter_is_handed_the_path() {
    drive(|| async {
        let codex = recording(Some(AttachmentRoad::File));
        let w = plain(&[("codex", &codex)]).await;
        let handle = start(&w, with("what is this?", Some("codex"), Some("req_a"), vec![png(9)])).await;
        handle.finished().await;
        let (thread, folder) = (thread_of(&handle), cwd_of(&handle));
        let dir = dir_of(&folder, &thread, "req_a");
        let path = image_path_in(&dir, 0.0, "image/png").expect("a path");
        // A computer the person owns has no /root to land under; the thread's folder is there on every kind of machine.
        assert_eq!(path, format!("{folder}/.wsp-files/{thread}/req_a.images/1.png"));
        // Readied as the thread's files are, so git's status stays clean and removing the thread removes it too.
        assert!(w.execs().contains(&land_files_line(&folder, &dir)));
        // The adapter reads a path, and the path it reads holds the bytes the person sent.
        assert_eq!(img_paths(&codex, 0), std::slice::from_ref(&path));
        let landed = w.puts().into_iter().find(|p| p.path == path).expect("the file landed");
        assert_eq!(landed.body, vec![9u8; 64]);
    });
}

// "an image on the file road" > "names its folder for itself when the send carried no request id, since two sends must never share a path"
#[test]
fn names_its_folder_for_itself_when_the_send_carried_no_request_id_since_two_sends_must_never_share_a_path() {
    drive(|| async {
        let codex = recording(Some(AttachmentRoad::File));
        let w = plain(&[("codex", &codex)]).await;
        let one = start(&w, with("first", Some("codex"), None, vec![png(1)])).await;
        one.finished().await;
        let thread = thread_of(&one);
        start(&w, StartOptions { thread: Some(thread.clone()), ..with("second", Some("codex"), None, vec![png(2)]) })
            .await
            .finished()
            .await;
        let dirs: Vec<String> =
            (0..2).map(|n| img_paths(&codex, n)[0].rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default()).collect();
        assert_ne!(dirs[0], dirs[1]);
        for dir in &dirs {
            assert!(dir.starts_with(&format!("{}/.wsp-files/{thread}/", cwd_of(&one))) && dir.ends_with(".images"), "{dir}");
        }
    });
}

// "an image on the file road" > "a request id shaped like a path of its own does not become one; the folder is named for this send instead"
#[test]
fn a_request_id_shaped_like_a_path_of_its_own_does_not_become_one_the_folder_is_named_for_this_send_instead() {
    drive(|| async {
        let codex = recording(Some(AttachmentRoad::File));
        let w = plain(&[("codex", &codex)]).await;
        let handle = start(&w, with("sneaky", Some("codex"), Some("../../../../etc/cron.d"), vec![png(1)])).await;
        handle.finished().await;
        let path = img_paths(&codex, 0).remove(0);
        assert!(path.starts_with(&format!("{}/.wsp-files/{}/", cwd_of(&handle), thread_of(&handle))), "{path}");
        assert!(!path.contains(".."));
    });
}

// "an image on the file road" > "names each image by its place in the message and its own type"
#[test]
fn names_each_image_by_its_place_in_the_message_and_its_own_type() {
    drive(|| async {
        let codex = recording(Some(AttachmentRoad::File));
        let w = plain(&[("codex", &codex)]).await;
        let handle = start(
            &w,
            with(
                "these three",
                Some("codex"),
                Some("req_a"),
                vec![png(1), att("image/jpeg", bytes_of(2, 64), None), att("image/webp", bytes_of(3, 64), None)],
            ),
        )
        .await;
        handle.finished().await;
        let dir = dir_of(&cwd_of(&handle), &thread_of(&handle), "req_a");
        assert_eq!(img_paths(&codex, 0), [format!("{dir}/1.png"), format!("{dir}/2.jpg"), format!("{dir}/3.webp")]);
        assert_eq!(w.puts().iter().filter(|p| p.path.contains(".images/")).count(), 3);
    });
}

// "an image on the file road" > "takes the send's folder off the machine when its turn ends, so a screenshot does not outlive the turn it was sent for"
#[test]
fn takes_the_sends_folder_off_the_machine_when_its_turn_ends_so_a_screenshot_does_not_outlive_the_turn_it_was_sent_for() {
    drive(|| async {
        let codex = recording(Some(AttachmentRoad::File));
        let w = plain(&[("codex", &codex)]).await;
        let handle = start(&w, with("first", Some("codex"), Some("req_a"), vec![png(1)])).await;
        handle.finished().await;
        let thread = thread_of(&handle);
        let dir = dir_of(&cwd_of(&handle), &thread, "req_a");
        let removal = format!("rm -rf {}", quoted(&dir));
        let (b, r) = (w.backend.clone(), removal.clone());
        wait!([b, r] b.machine(0).exec_log().contains(&r));
        // Twenty text turns after it, nothing of that send is still there and no other folder was touched.
        for nth in 0..3 {
            start(&w, StartOptions { thread: Some(thread.clone()), ..with(&format!("text {nth}"), Some("codex"), None, vec![]) })
                .await
                .finished()
                .await;
        }
        let removals: Vec<String> = w.execs().into_iter().filter(|c| c.starts_with("rm -rf ") && c.contains(".images")).collect();
        assert_eq!(removals, [removal]);
    });
}

// "an image on the file road" > "takes the folder off however the turn ended, a failed one as well as a completed one"
#[test]
fn takes_the_folder_off_however_the_turn_ended_a_failed_one_as_well_as_a_completed_one() {
    drive(|| async {
        let codex = hand(Mode::Failing("the harness died"), Some(AttachmentRoad::File), false);
        let w = plain(&[("codex", &codex)]).await;
        let handle = start(&w, with("boom", Some("codex"), Some("req_a"), vec![png(1)])).await;
        handle.finished().await;
        let dir = dir_of(&cwd_of(&handle), &thread_of(&handle), "req_a");
        let removal = format!("rm -rf {}", quoted(&dir));
        let b = w.backend.clone();
        wait!([b, removal] b.machine(0).exec_log().contains(&removal));
    });
}

// "an image on the file road" > "a turn that carried no image asks the machine to remove nothing"
#[test]
fn a_turn_that_carried_no_image_asks_the_machine_to_remove_nothing() {
    drive(|| async {
        let codex = recording(Some(AttachmentRoad::File));
        let w = plain(&[("codex", &codex)]).await;
        start(&w, with("no image", Some("codex"), None, vec![])).await.finished().await;
        assert!(w.execs().iter().all(|c| !c.starts_with("rm -rf ")));
    });
}

// "an image on the file road" > "a message with no image asks the machine for nothing"
#[test]
fn a_message_with_no_image_asks_the_machine_for_nothing() {
    drive(|| async {
        let codex = recording(Some(AttachmentRoad::File));
        let w = plain(&[("codex", &codex)]).await;
        start(&w, with("no image", Some("codex"), None, vec![])).await.finished().await;
        assert!(w.puts().is_empty());
        assert!(codex.starts.borrow()[0].images.is_empty());
    });
}

// "two sends with images on one thread" > "neither is handed the other's picture: two sends that wake together land in folders of their own"
#[test]
fn neither_is_handed_the_others_picture_two_sends_that_wake_together_land_in_folders_of_their_own() {
    drive(|| async {
        let codex = hand(Mode::Manual, Some(AttachmentRoad::File), false);
        let w = plain(&[("codex", &codex)]).await;
        let first = start(&w, with("A", Some("codex"), Some("req_a"), vec![png(0xaa)])).await;
        let thread = thread_of(&first);
        // Both wait behind the running turn and both wake in the one drain when it ends, so both land before either is
        // registered: the window the wait loop cannot close, and the one a shared folder loses a picture in.
        let (rt, ws, t) = (w.fx.rt.clone(), w.ws.clone(), thread.clone());
        let b = tokio::task::spawn_local({
            let (rt, ws, t) = (rt.clone(), ws.clone(), t.clone());
            async move {
                rt.sessions()
                    .start(&ws, StartOptions { thread: Some(t), ..with("B", Some("codex"), Some("req_b"), vec![png(0xbb)]) }, None)
                    .await
                    .map(|_| ())
            }
        });
        let c = tokio::task::spawn_local(async move {
            rt.sessions()
                .start(&ws, StartOptions { thread: Some(t), ..with("C", Some("codex"), Some("req_c"), vec![png(0xcc)]) }, None)
                .await
                .map(|_| ())
        });
        sleep_ms(20).await;
        assert_eq!(codex.starts.borrow().len(), 1);
        codex.end(0);
        let cx = codex.clone();
        wait!([cx] cx.starts.borrow().len() == 2);
        // The turn that is running holds its own bytes at its own path, whichever of the two won the race.
        let running_prompt = codex.starts.borrow()[1].prompt.clone();
        let running_path = img_paths(&codex, 1).remove(0);
        let fill = if running_prompt == "B" { 0xbbu8 } else { 0xcc };
        let landed = w.puts().into_iter().find(|p| p.path == running_path).expect("the running turn's file");
        assert_eq!(landed.body, vec![fill; 64]);
        codex.end(1);
        let cx = codex.clone();
        wait!([cx] cx.starts.borrow().len() == 3);
        let waited_prompt = codex.starts.borrow()[2].prompt.clone();
        let waited_path = img_paths(&codex, 2).remove(0);
        // And so does the one that waited: its folder was never written over while it sat in the queue.
        assert_ne!(waited_path, running_path);
        let landed = w.puts().into_iter().rfind(|p| p.path == waited_path).expect("the waiting turn's file");
        assert_eq!(landed.body, vec![if waited_prompt == "B" { 0xbbu8 } else { 0xcc }; 64]);
        codex.end(2);
        b.await.expect("B").expect("B started");
        c.await.expect("C").expect("C started");
    });
}

// "two sends with images on one thread" > "each takes only its own folder off the machine when its turn ends"
#[test]
fn each_takes_only_its_own_folder_off_the_machine_when_its_turn_ends() {
    drive(|| async {
        let codex = hand(Mode::Manual, Some(AttachmentRoad::File), false);
        let w = plain(&[("codex", &codex)]).await;
        let first = start(&w, with("A", Some("codex"), Some("req_a"), vec![png(1)])).await;
        let thread = thread_of(&first);
        let dir_for = |request: &str| dir_of(&cwd_of(&first), &thread, request);
        let (rt, ws, t) = (w.fx.rt.clone(), w.ws.clone(), thread.clone());
        let second = tokio::task::spawn_local(async move {
            rt.sessions()
                .start(&ws, StartOptions { thread: Some(t), ..with("B", Some("codex"), Some("req_b"), vec![png(2)]) }, None)
                .await
                .map(|_| ())
        });
        sleep_ms(20).await;
        codex.end(0);
        second.await.expect("B").expect("B started");
        let (a_gone, b_gone) = (format!("rm -rf {}", quoted(&dir_for("req_a"))), format!("rm -rf {}", quoted(&dir_for("req_b"))));
        let (b, ag) = (w.backend.clone(), a_gone.clone());
        wait!([b, ag] b.machine(0).exec_log().contains(&ag));
        // The turn still running keeps its folder; only the one that ended lost its.
        assert!(!w.all_execs().contains(&b_gone));
        codex.end(1);
        let b = w.backend.clone();
        wait!([b, b_gone] b.machine(0).exec_log().contains(&b_gone));
    });
}

/// `heldLanding`: a machine whose image landing can be held, so a send's images are on it while the turn that refuses the send
/// opens. Armed only after the workspace is created, since a create lands the daemon the same way.
struct HeldLanding {
    backend: Arc<StubBackend>,
    armed: Rc<std::cell::Cell<bool>>,
    gate: Rc<Gate>,
}

fn held_landing() -> HeldLanding {
    let backend = StubBackend::new(None);
    let armed = Rc::new(std::cell::Cell::new(false));
    let gate = Gate::new();
    let plain = backend.exec_impl_fn();
    let (a, g) = (armed.clone(), gate.clone());
    backend.set_exec_impl(Rc::new(move |m, cmd, stdin| {
        let (plain, a, g) = (plain.clone(), a.clone(), g.clone());
        Box::pin(async move {
            if a.get() && cmd.contains(".images") {
                g.wait().await;
            }
            plain(m, cmd, stdin).await
        })
    }));
    HeldLanding { backend, armed, gate }
}

// "a send that landed its images and is then refused" > "takes its folder off the machine when the workspace naps under it, since no turn will ever do it"
#[test]
fn takes_its_folder_off_the_machine_when_the_workspace_naps_under_it_since_no_turn_will_ever_do_it() {
    drive(|| async {
        let codex = hand(Mode::Manual, Some(AttachmentRoad::File), false);
        let held = held_landing();
        let w = workspace_on(&[("codex", &codex)], held.backend.clone(), None).await;
        held.armed.set(true);
        let first = start(&w, with("A", Some("codex"), None, vec![])).await;
        let thread = thread_of(&first);
        let dir_for = |request: &str| dir_of(&cwd_of(&first), &thread, request);
        // B queues behind A, wakes when A's process exits, and is landing its images when the workspace naps.
        let (rt, ws, t) = (w.fx.rt.clone(), w.ws.clone(), thread.clone());
        let second = tokio::task::spawn_local(async move {
            rt.sessions()
                .start(&ws, StartOptions { thread: Some(t), ..with("B", Some("codex"), Some("req_b"), vec![png(0xbb)]) }, None)
                .await
                .map(|_| ())
        });
        codex.reply(0);
        codex.exit(0);
        first.finished().await;
        let b = held.backend.clone();
        wait!([b] b.machine(0).exec_log().iter().any(|c| c.contains(".images")));
        // The nap leaves nothing on this machine for B to run as: B is refused once its own images have landed.
        w.fx.rt.workspaces().nap(&w.ws, None).await.expect("a nap");
        held.gate.open();
        let refused = second.await.expect("B").expect_err("a refusal");
        let words = wsp_words::workspace_state::send_refusal(
            &wsp_words::workspace_state::SendRefusalKind::State(wsp_words::workspace_state::WorkspaceState::Paused),
            None,
            None,
        )
        .expect("words");
        assert!(refused.message.contains(&words), "{}", refused.message);
        // B's images were on the machine before the refusal was known, and B has no turn to take them off, so B does.
        let removal = format!("rm -rf {}", quoted(&dir_for("req_b")));
        let b = held.backend.clone();
        wait!([b, removal] b.machine(0).exec_log().contains(&removal));
    });
}

// "a send that landed its images and is then refused" > "a refused send that carried no image asks the machine for nothing"
#[test]
fn a_refused_send_that_carried_no_image_asks_the_machine_for_nothing() {
    drive(|| async {
        let codex = hand(Mode::Manual, Some(AttachmentRoad::File), false);
        let w = plain(&[("codex", &codex)]).await;
        let first = start(&w, with("A", Some("codex"), None, vec![])).await;
        let thread = thread_of(&first);
        let (rt, ws, t) = (w.fx.rt.clone(), w.ws.clone(), thread.clone());
        let second = tokio::task::spawn_local(async move {
            rt.sessions().start(&ws, StartOptions { thread: Some(t), ..with("B", Some("codex"), None, vec![]) }, None).await.map(|_| ())
        });
        w.fx.rt.workspaces().nap(&w.ws, None).await.expect("a nap");
        codex.exit(0);
        let refused = second.await.expect("B").expect_err("a refusal");
        let words = wsp_words::workspace_state::send_refusal(
            &wsp_words::workspace_state::SendRefusalKind::State(wsp_words::workspace_state::WorkspaceState::Paused),
            None,
            None,
        )
        .expect("words");
        assert!(refused.message.contains(&words), "{}", refused.message);
        assert!(w.execs().iter().all(|c| !c.starts_with("rm -rf ")));
    });
}

// "a send that landed its images and is then refused" > "no turn opens under a send that is landing its images: a send that arrives then waits for it and runs after it, as the thread's next turn"
#[test]
fn no_turn_opens_under_a_send_that_is_landing_its_images_a_send_that_arrives_then_waits_for_it_and_runs_after_it_as_the_threads_next_turn()
{
    drive(|| async {
        let claude = hand(Mode::Manual, Some(AttachmentRoad::File), true);
        let held = held_landing();
        let w = workspace_on(&[("claude", &claude)], held.backend.clone(), None).await;
        held.armed.set(true);
        let first = start(&w, opts("A")).await;
        let thread = thread_of(&first);
        claude.reply(0);
        claude.exit(0);
        first.finished().await;
        // B lands into a thread with nothing running; the thread is B's from that moment, so C finds it taken.
        let (rt, ws, t) = (w.fx.rt.clone(), w.ws.clone(), thread.clone());
        let second = tokio::task::spawn_local(async move {
            rt.sessions().start(&ws, StartOptions { thread: Some(t), ..with("B", None, Some("req_b"), vec![png(0xbb)]) }, None).await
        });
        let b = held.backend.clone();
        wait!([b] b.machine(0).exec_log().iter().any(|c| c.contains(".images")));
        let opened = Rc::new(std::cell::Cell::new(false));
        let (rt, ws, t, flag) = (w.fx.rt.clone(), w.ws.clone(), thread.clone(), opened.clone());
        let third = tokio::task::spawn_local(async move {
            let h = rt.sessions().start(&ws, StartOptions { thread: Some(t), prompt: "C".into(), ..Default::default() }, None).await;
            flag.set(true);
            h
        });
        sleep_ms(50).await;
        assert!(!opened.get());
        assert_eq!(claude.prompts(), ["A"]);
        held.gate.open();
        let b = second.await.expect("B").expect("B started");
        assert_eq!(format!("{:?}", b.outcome()).to_lowercase(), "started");
        let starts: Vec<(String, Option<String>)> = claude.starts.borrow().iter().map(|s| (s.prompt.clone(), s.resume.clone())).collect();
        assert_eq!(starts, [("A".to_string(), None), ("B".to_string(), Some(SESSION_ID.to_string()))]);
        // C reached the running turn B opened and steered into it.
        let c = third.await.expect("C").expect("C started");
        assert_eq!(format!("{:?}", c.outcome()).to_lowercase(), "steered");
        assert_eq!(*claude.steered.borrow(), ["C"]);
        claude.reply(claude.last());
        claude.exit(claude.last());
        b.finished().await;
    });
}

// "an agent that reads no image" > "refuses in that agent's name, before the machine is asked for anything"
#[test]
fn refuses_in_that_agents_name_before_the_machine_is_asked_for_anything() {
    drive(|| async {
        let gemini = recording(None);
        let w = plain(&[("gemini", &gemini)]).await;
        let refused =
            w.fx.rt.sessions().start(&w.ws, with("look", Some("gemini"), None, vec![png(1)]), None).await.err().expect("a refusal");
        assert!(refused.message.contains(&no_images_line("gemini")), "{}", refused.message);
        assert!(gemini.starts.borrow().is_empty());
        assert!(w.puts().is_empty());
        assert!(w.execs().is_empty());
    });
}

// "an agent that reads no image" > "takes the same message without an image, since only the image was the trouble"
#[test]
fn takes_the_same_message_without_an_image_since_only_the_image_was_the_trouble() {
    drive(|| async {
        let gemini = recording(None);
        let w = plain(&[("gemini", &gemini)]).await;
        start(&w, with("look", Some("gemini"), None, vec![])).await.finished().await;
        assert_eq!(gemini.starts.borrow().len(), 1);
    });
}

// "the caps, checked again before the machine is asked" > "a 12 MB image is refused with the cap in the sentence and nothing is started"
#[test]
fn a_12_mb_image_is_refused_with_the_cap_in_the_sentence_and_nothing_is_started() {
    drive(|| async {
        let claude = recording(Some(AttachmentRoad::Inline));
        let w = plain(&[("claude", &claude)]).await;
        let refused =
            w.fx.rt
                .sessions()
                .start(&w.ws, with("big one", None, None, vec![att("image/png", bytes_of(1, 12 * 1024 * 1024), None)]), None)
                .await
                .err()
                .expect("a refusal");
        assert!(refused.message.contains("over the 10 MB an image may be"), "{}", refused.message);
        assert!(claude.starts.borrow().is_empty());
        assert!(w.puts().is_empty());
    });
}

// "the caps, checked again before the machine is asked" > "six files are refused with both counts"
#[test]
fn six_files_are_refused_with_both_counts() {
    drive(|| async {
        let claude = recording(Some(AttachmentRoad::Inline));
        let w = plain(&[("claude", &claude)]).await;
        let refused =
            w.fx.rt.sessions().start(&w.ws, with("six", None, None, (0..6).map(png).collect()), None).await.err().expect("a refusal");
        assert!(refused.message.contains("only 5 files fit one message; this one carries 6"), "{}", refused.message);
        assert!(claude.starts.borrow().is_empty());
    });
}

// "what the transcript keeps of a message's images" > "the records, never the pixels, so a transcript costs the same however large the image was"
#[test]
fn the_records_never_the_pixels_so_a_transcript_costs_the_same_however_large_the_image_was() {
    drive(|| async {
        let claude = recording(Some(AttachmentRoad::Inline));
        let w = plain(&[("claude", &claude)]).await;
        let big = bytes_of(4, 1_258_291);
        start(&w, with("what is this?", None, None, vec![att("image/png", big.clone(), Some("shot.png"))])).await.finished().await;
        let events = common::comeback::history(&w.fx.rt, &w.ws).await;
        let start_row = events.iter().find(|e| e["type"] == "session.start").expect("a start row");
        assert_eq!(start_row["attachments"], json!([{ "mediaType": "image/png", "bytes": 1_258_291, "name": "shot.png" }]));
        assert!(!serde_json::to_string(&events).expect("json").contains(&big));
    });
}

// "what the transcript keeps of a message's images" > "a turn without an image carries no records at all"
#[test]
fn a_turn_without_an_image_carries_no_records_at_all() {
    drive(|| async {
        let claude = recording(Some(AttachmentRoad::Inline));
        let w = plain(&[("claude", &claude)]).await;
        start(&w, opts("plain")).await.finished().await;
        let events = common::comeback::history(&w.fx.rt, &w.ws).await;
        assert!(events.iter().find(|e| e["type"] == "session.start").expect("a start row").get("attachments").is_none());
    });
}

// "what the catalog says about an agent's images" > "is the adapter's answer on the machine, one row per agent"
#[test]
fn is_the_adapters_answer_on_the_machine_one_row_per_agent() {
    drive(|| async {
        let (claude, codex, gemini) = (recording(Some(AttachmentRoad::Inline)), recording(Some(AttachmentRoad::File)), recording(None));
        let w = plain(&[("claude", &claude), ("codex", &codex), ("gemini", &gemini)]).await;
        let rows = j(&w.fx.rt.harnesses().list(Some(w.ws.clone()), None, None).await.expect("a catalog"));
        let rows = rows.as_array().cloned().unwrap_or_else(|| rows["harnesses"].as_array().cloned().unwrap_or_default());
        let images: Vec<Value> = ["claude", "codex", "gemini"]
            .iter()
            .map(|name| json!([name, rows.iter().find(|r| r["harness"] == *name).map(|r| r["images"].clone()).unwrap_or(Value::Null)]))
            .collect();
        assert_eq!(images, [json!(["claude", true]), json!(["codex", true]), json!(["gemini", false])]);
    });
}
