// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/fork.test.ts at 65f6b89c7, case for case. A thread forked from one of its finished turns:
//! the source read and every refusal made before anything is written, the session the fork's turns run on, the
//! history it opens on, what it carries of its source, and the source left as it was. The harnesses and the daemon are
//! fakes; git is real where a fork lands on a new branch. Each test names the TS test above it.
mod common;

use std::cell::Cell;
use std::collections::HashSet;
use std::future::Future;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use common::adapter::session_of;
use common::daemon::{fake_daemon, git};
use common::local::{git_copier, recording_forks, ws_request, CopierLog};
use common::*;
use serde_json::{json, Value};
use wsp_adapters::port::ForkTurn;
use wsp_core::types::api::{
    DeleteThreadAsk, FolderForAsk, ForkOptions, OutsideOpening, PageWindow, ProjectAddAsk, RewindAsk, StartOptions,
};
use wsp_core::types::internal::SESSIONS;
use wsp_core::types::wiring::{LocalWiring, Platform};
use wsp_engine::local_backend::{LocalBackend, LocalBackendOptions};
use wsp_serve::serve::{serve_runtime, ServeOptions};
use wsp_serve::AssembledRuntime;
use wsp_store::store::Store;
use wsp_wire::types::attachments::Attachment;
use wsp_wire::types::js_number::JsInt;
use wsp_wire::views::session::{ForkSource, ThreadView};
use wsp_wire::views::session_events::{copied_from_of, SessionEvent};
use wsp_words::exit::{HostError, Kind};
use wsp_words::thread_read::thread_messages;
use wsp_words::views::session::fold_threads;
use wsp_words::words::thread::{
    codex_no_turn_line, fork_agent_line, fork_no_anchor_line, fork_refused_line, fork_turns_line, FORK_BESIDE_FIX, FORK_NO_CHECKPOINT_FIX,
    FORK_NO_CHECKPOINT_LINE, FORK_REFUSED_FIX, FORK_RESUME_FIX, FORK_RESUME_LINE, FORK_RUNNING_FIX, FORK_RUNNING_LINE, REWIND_COPIED_LINE,
};

const DAEMON_TOKEN: &str = "cafef00dcafef00dcafef00d";
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
const COPY: &str = "c0c0c0c0-0000-4000-8000-000000000001";

fn png() -> Attachment {
    Attachment { media_type: "image/png".into(), bytes: PNG.into(), name: Some("dot.png".into()) }
}

/// A box workspace with the claude id on the copy road and the codex id on the start road.
#[derive(Default)]
struct BoxOpts {
    claude: AgentOptions,
    codex: AgentOptions,
    /// Keeps each turn the codex agent was asked to ready a fork through.
    record_codex_forks: bool,
}

struct Boxed {
    fx: Fx,
    state: State,
    ws: String,
    claude: Rc<Asked>,
    codex: Rc<Asked>,
    backend: Arc<StubBackend>,
    claude_opts: AgentOptions,
    codex_forks: Rc<std::cell::RefCell<Vec<ForkTurn>>>,
}

fn wired(backend: &Arc<StubBackend>, store: &Rc<dyn Store>) -> Wiring {
    runtime(backend).store(store).with(|o| {
        o.daemon_token = Some(DAEMON_TOKEN.into());
        o.daemon_channel = Some(fake_daemon(None).0);
    })
}

async fn boxed(o: BoxOpts) -> Boxed {
    let backend = StubBackend::new(None);
    backend.set_exec_impl(token_guest());
    let claude = Rc::new(Asked::default());
    let codex = Rc::new(Asked::default());
    let counter: Rc<Cell<u32>> = Rc::default();
    let state = sqlite_state();
    let claude_opts = AgentOptions { counter: Some(counter.clone()), ..o.claude };
    let codex_adapter = agent(Road::Start, &codex, AgentOptions { counter: Some(counter.clone()), ..o.codex });
    let (codex_adapter, codex_forks) = if o.record_codex_forks { recording_forks(codex_adapter) } else { (codex_adapter, Rc::default()) };
    let fx = wired(&backend, &state.store)
        .adapter("claude", agent(Road::Copy, &claude, claude_opts.clone()))
        .adapter("codex", codex_adapter)
        .build();
    let ws = create_on(&fx.rt, CreateOn::golden("pricing page"), None).await.expect("a workspace").view.id;
    backend.machine(0).set_preview_url("http://127.0.0.1:7070");
    Boxed { fx, state, ws, claude, codex, backend, claude_opts, codex_forks }
}

impl Boxed {
    /// `again(store)`: a second runtime over the same backend and the same state, with the claude agent alone.
    fn again(&self) -> Fx {
        wired(&self.backend, &self.state.store).adapter("claude", agent(Road::Copy, &self.claude, self.claude_opts.clone())).build()
    }
}

fn start(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

fn on_thread(prompt: &str, thread: &str) -> StartOptions {
    StartOptions { thread: Some(thread.into()), ..start(prompt) }
}

/// Turns teaching ALPHA, BETA and GAMMA on one thread of the agent; answers the thread and each turn's id.
async fn three_turns(
    rt: &AssembledRuntime,
    ws: &str,
    harness: &str,
    model: Option<&str>,
    effort: Option<&str>,
    attachments: Option<Vec<Attachment>>,
) -> (String, Vec<String>) {
    let opts = StartOptions {
        harness: Some(harness.into()),
        request_id: Some("req_alpha".into()),
        model: model.map(str::to_string),
        effort: effort.map(str::to_string),
        attachments,
        ..start("Remember ALPHA.")
    };
    let first = rt.sessions().start(ws, opts, None).await.expect("a start");
    first.finished().await;
    let thread_id = first.view().thread_id.expect("a thread");
    let mut turns = vec![first.turn_id()];
    for word in ["BETA", "GAMMA"] {
        let next = rt.sessions().start(ws, on_thread(&format!("Remember {word}."), &thread_id), None).await.expect("a start");
        next.finished().await;
        turns.push(next.turn_id());
    }
    settle().await;
    (thread_id, turns)
}

async fn thread_of(rt: &AssembledRuntime, ws: &str, thread: &str) -> Option<ThreadView> {
    let rows = rt.sessions().list(Some(ws.to_string()), None).await.expect("a list");
    fold_threads(&rows).into_iter().find(|t| t.thread_id.as_deref() == Some(thread))
}

fn prompts_of(events: &[SessionEvent], thread: &str) -> Vec<String> {
    events
        .iter()
        .map(j)
        .filter(|e| e["threadId"] == thread && e["type"] == "session.start")
        .filter_map(|e| e["prompt"].as_str().map(str::to_string))
        .collect()
}

async fn refusal_of<T>(r: impl Future<Output = Result<T, HostError>>) -> HostError {
    match r.await {
        Ok(_) => panic!("the fork was not refused"),
        Err(e) => e,
    }
}

fn source_of(thread: &str, turn: Option<&str>, at: Option<i64>) -> ForkSource {
    ForkSource { thread_id: thread.into(), turn_id: turn.map(str::to_string), at: at.map(JsInt::from) }
}

fn forking(prompt: &str, thread: &str, turn: Option<&str>, at: Option<i64>) -> ForkOptions {
    ForkOptions { start: start(prompt), fork: source_of(thread, turn, at), branch: None }
}

fn anchor(a: &str) -> ForkAt {
    ForkAt::Anchor(a.into())
}

// "a fork of a Claude Code thread at a finished turn" > "resumes a copy of the source's session through that turn's anchor, on the source's agent, model and effort, and its next send resumes the copy"
#[test]
fn resumes_a_copy_of_the_sources_session_through_that_turns_anchor_on_the_sources_agent_model_and_effort_and_its_next_send_resumes_the_copy(
) {
    drive(|| async {
        let b = boxed(BoxOpts::default()).await;
        let rt = &*b.fx.rt;
        let (thread_id, turns) = three_turns(rt, &b.ws, "claude", Some("claude-opus-5"), Some("high"), None).await;
        let source = thread_of(rt, &b.ws, &thread_id).await.expect("the source");
        let session = source.claude_session_id.clone().expect("the source's session");
        let run = rt
            .sessions()
            .fork(forking("list every code word you remember", &thread_id, Some(&turns[1]), None), None)
            .await
            .expect("a fork");
        run.finished().await;
        assert_eq!(*b.claude.forks.borrow(), [(session.clone(), anchor("a2"))]);
        let first = b.claude.starts.borrow().last().cloned().expect("a start");
        assert_eq!(first.prompt, "list every code word you remember");
        assert_eq!(first.resume.as_deref(), Some(COPY));
        assert_eq!(first.model.as_deref(), Some("claude-opus-5"));
        assert_eq!(first.effort.as_deref(), Some("high"));
        assert!(first.fork.is_none());
        let fork_id = run.view().thread_id.expect("the fork's thread");
        assert_ne!(fork_id, thread_id);
        // The fork's second send resumes its own session, never the source's.
        rt.sessions().start(&b.ws, on_thread("and now?", &fork_id), None).await.expect("a start").finished().await;
        assert_eq!(b.claude.starts.borrow().last().and_then(|s| s.resume.clone()).as_deref(), Some(COPY));
        assert_eq!(b.claude.starts.borrow().iter().filter(|s| s.resume.as_deref() == Some(session.as_str())).count(), 2);
        // A start that names another model or effort runs at it.
        let mut asked = forking("x", &thread_id, Some(&turns[0]), None);
        asked.start.model = Some("claude-sonnet-5".into());
        asked.start.effort = Some("low".into());
        rt.sessions().fork(asked, None).await.expect("a fork").finished().await;
        let last = b.claude.starts.borrow().last().cloned().expect("a start");
        assert_eq!(last.model.as_deref(), Some("claude-sonnet-5"));
        assert_eq!(last.effort.as_deref(), Some("low"));
        rt.close().await.expect("a close");
    });
}

// "opens on copies of the source's events through that turn under its own thread, and says where it came from on its record and every view"
#[test]
fn opens_on_copies_of_the_sources_events_through_that_turn_under_its_own_thread_and_says_where_it_came_from_on_its_record_and_every_view() {
    drive(|| async {
        let b = boxed(BoxOpts::default()).await;
        let rt = &*b.fx.rt;
        let (thread_id, turns) = three_turns(rt, &b.ws, "claude", None, None, None).await;
        let title = thread_of(rt, &b.ws, &thread_id).await.expect("the source").title;
        let run = rt
            .sessions()
            .fork(forking("list every code word you remember", &thread_id, Some(&turns[1]), None), None)
            .await
            .expect("a fork");
        run.finished().await;
        let fork_id = run.view().thread_id.expect("the fork's thread");
        let page = rt.sessions().page(&b.ws, PageWindow { thread_id: fork_id.clone(), ..Default::default() }, None).await.expect("a page");
        assert_eq!(prompts_of(&page.events, &fork_id), ["Remember ALPHA.", "Remember BETA.", "list every code word you remember"]);
        let copied: Vec<&SessionEvent> = page.events.iter().filter(|e| copied_from_of(e).is_some()).collect();
        let turn_of = |e: &SessionEvent| j(e)["turnId"].as_str().map(str::to_string);
        let seen: HashSet<Option<String>> = copied.iter().map(|e| turn_of(e)).collect();
        let want: HashSet<Option<String>> = turns[..2].iter().cloned().map(Some).collect();
        assert_eq!(seen, want);
        assert!(copied.iter().all(|e| j(e)["threadId"] == fork_id.as_str() && copied_from_of(e).as_deref() == Some(thread_id.as_str())));
        assert!(page.events.iter().filter(|e| copied_from_of(e).is_none()).all(|e| turn_of(e).as_deref() == Some(run.turn_id().as_str())));
        assert_eq!(page.events.iter().position(|e| copied_from_of(e).is_none()), Some(copied.len()));
        let from = json!({ "threadId": thread_id, "turnId": turns[1], "title": title });
        assert_matches(&j(&run.view()), json!({ "threadId": fork_id }));
        let rows = rt.sessions().list(Some(b.ws.clone()), None).await.expect("a list");
        let row = rows.iter().find(|r| r.thread_id.as_deref() == Some(fork_id.as_str())).expect("the fork's row");
        assert_matches(&j(row), json!({ "forkedFrom": from }));
        assert_matches(&j(&thread_of(rt, &b.ws, &fork_id).await.expect("the fork")), json!({ "forkedFrom": from }));
        assert!(thread_of(rt, &b.ws, &thread_id).await.expect("the source").forked_from.is_none());
        // The fork's own turns rewind; the turns it was handed do not.
        let rewind = rt
            .sessions()
            .rewind(&fork_id, RewindAsk { turn_id: Some(turns[0].clone()), ..Default::default() }, None)
            .await
            .expect_err("a refusal");
        assert!(rewind.message.contains(REWIND_COPIED_LINE), "{}", rewind.message);
        rt.close().await.expect("a close");
    });
}

// "carries the outside conversation its source opened on ahead of the turns, and wsp thread read shows it"
#[test]
fn carries_the_outside_conversation_its_source_opened_on_ahead_of_the_turns_and_wsp_thread_read_shows_it() {
    drive(|| async {
        let b = boxed(BoxOpts::default()).await;
        let rt = &*b.fx.rt;
        let line = |who: &str, text: &str| {
            serde_json::from_value(json!({ "workspaceId": b.ws, "sessionId": "outside", "who": who, "text": text }))
                .expect("an earlier line")
        };
        let outside_id = session_of("0d0d0d0d", 1);
        let outside = OutsideOpening {
            harness: "claude".into(),
            id: outside_id.clone(),
            copy: false,
            project: "lab".into(),
            cwd: None,
            title: "lab codewords".into(),
            earlier: vec![line("person", "Remember ALPHA."), line("agent", "ALPHA kept.")],
        };
        let first = rt
            .sessions()
            .start(&b.ws, StartOptions { harness: Some("claude".into()), outside: Some(outside), ..start("Remember BETA.") }, None)
            .await
            .expect("a start");
        first.finished().await;
        let thread_id = first.view().thread_id.expect("a thread");
        assert_eq!(b.claude.starts.borrow().last().and_then(|s| s.resume.clone()), Some(outside_id));
        settle().await;
        let run = rt.sessions().fork(forking("list every code word you remember", &thread_id, None, None), None).await.expect("a fork");
        run.finished().await;
        let fork_id = run.view().thread_id.expect("the fork's thread");
        let events =
            rt.sessions().page(&b.ws, PageWindow { thread_id: fork_id.clone(), ..Default::default() }, None).await.expect("a page").events;
        let earlier: Vec<&SessionEvent> = events.iter().filter(|e| j(e)["type"] == "session.earlier").collect();
        let rows: Vec<Value> = earlier.iter().map(|e| json!([j(e)["text"], j(e)["threadId"], copied_from_of(e)])).collect();
        assert_eq!(rows, [json!(["Remember ALPHA.", fork_id, thread_id]), json!(["ALPHA kept.", fork_id, thread_id])]);
        assert_eq!(events[..2].iter().map(j).collect::<Vec<_>>(), earlier.iter().copied().map(j).collect::<Vec<_>>());
        let said: Vec<(Value, Value)> = thread_messages(&events, &fork_id)
            .iter()
            .map(j)
            .filter(|m| m["who"] != "turn")
            .map(|m| (m["who"].clone(), m["text"].clone()))
            .collect();
        let want: Vec<(Value, Value)> = [
            ("person", "Remember ALPHA."),
            ("agent", "ALPHA kept."),
            ("person", "Remember BETA."),
            ("agent", "reply 1"),
            ("person", "list every code word you remember"),
            ("agent", "reply 2"),
        ]
        .iter()
        .map(|(who, text)| (json!(who), json!(text)))
        .collect();
        assert_eq!(said, want);
        // A fork that carries no turn carries none of it either, as its agent starts with nothing; the app offers no such
        // fork on a thread opened on an outside conversation, and only wsp run --fork --at 0 still makes one.
        let bare = rt.sessions().fork(forking("x", &thread_id, None, Some(0)), None).await.expect("a fork");
        bare.finished().await;
        let bare_id = bare.view().thread_id.expect("the fork's thread");
        let page = rt.sessions().page(&b.ws, PageWindow { thread_id: bare_id, ..Default::default() }, None).await.expect("a page");
        assert!(!page.events.iter().any(|e| j(e)["type"] == "session.earlier"));
        rt.close().await.expect("a close");
    });
}

// "leaves the source's transcript, rows, read stamp and status as they were"
#[test]
fn leaves_the_sources_transcript_rows_read_stamp_and_status_as_they_were() {
    drive(|| async {
        let b = boxed(BoxOpts::default()).await;
        let rt = &*b.fx.rt;
        let (thread_id, turns) = three_turns(rt, &b.ws, "claude", None, None, None).await;
        rt.sessions().read(&thread_id, None).await.expect("a read");
        let events_of = || async {
            rt.sessions()
                .page(&b.ws, PageWindow { thread_id: thread_id.clone(), ..Default::default() }, None)
                .await
                .expect("a page")
                .events
                .iter()
                .map(j)
                .collect::<Vec<_>>()
        };
        let rows_of = || async {
            rt.sessions()
                .list(Some(b.ws.clone()), None)
                .await
                .expect("a list")
                .iter()
                .filter(|r| r.thread_id.as_deref() == Some(thread_id.as_str()))
                .map(j)
                .collect::<Vec<_>>()
        };
        let before = (events_of().await, rows_of().await, j(&thread_of(rt, &b.ws, &thread_id).await));
        rt.sessions().fork(forking("x", &thread_id, Some(&turns[0]), None), None).await.expect("a fork").finished().await;
        settle().await;
        assert_eq!(events_of().await, before.0);
        assert_eq!(rows_of().await, before.1);
        assert_eq!(j(&thread_of(rt, &b.ws, &thread_id).await), before.2);
        rt.close().await.expect("a close");
    });
}

// "forks the latest finished turn without one named, counts --at from 1, and with 0 carries no turn at all"
#[test]
fn forks_the_latest_finished_turn_without_one_named_counts_at_from_1_and_with_0_carries_no_turn_at_all() {
    drive(|| async {
        let b = boxed(BoxOpts::default()).await;
        let rt = &*b.fx.rt;
        let (thread_id, turns) = three_turns(rt, &b.ws, "claude", None, None, None).await;
        let latest = rt.sessions().fork(forking("x", &thread_id, None, None), None).await.expect("a fork");
        latest.finished().await;
        assert_eq!(b.claude.forks.borrow().last().map(|f| f.1.clone()), Some(anchor("a3")));
        rt.sessions().fork(forking("x", &thread_id, None, Some(2)), None).await.expect("a fork").finished().await;
        assert_eq!(b.claude.forks.borrow().last().map(|f| f.1.clone()), Some(anchor("a2")));
        let none = rt.sessions().fork(forking("Remember ALPHA.", &thread_id, None, Some(0)), None).await.expect("a fork");
        none.finished().await;
        assert_eq!(b.claude.forks.borrow().len(), 2);
        assert!(b.claude.starts.borrow().last().expect("a start").resume.is_none());
        let none_id = none.view().thread_id.expect("the fork's thread");
        let from = j(&thread_of(rt, &b.ws, &none_id).await.expect("the fork").forked_from);
        assert!(from["title"].is_string(), "{from}");
        assert_eq!(from, json!({ "threadId": thread_id, "title": from["title"] }));
        let refused = refusal_of(rt.sessions().fork(forking("x", &thread_id, None, Some(4)), None)).await;
        assert_eq!(refused.kind, Some(Kind::Usage));
        assert!(refused.message.contains(&fork_turns_line(3.0)), "{}", refused.message);
        assert_eq!(turns.len(), 3);
        rt.close().await.expect("a close");
    });
}

// "refuses in two halves where the session holds no such message, and leaves no thread, row, event, copy or image behind"
#[test]
fn refuses_in_two_halves_where_the_session_holds_no_such_message_and_leaves_no_thread_row_event_copy_or_image_behind() {
    drive(|| async {
        let b = boxed(BoxOpts {
            claude: AgentOptions { refuse: Some("the session holds no message a2".into()), ..Default::default() },
            ..Default::default()
        })
        .await;
        let rt = &*b.fx.rt;
        let (thread_id, turns) = three_turns(rt, &b.ws, "claude", None, None, Some(vec![png()])).await;
        let ids = || async {
            rt.sessions().list(Some(b.ws.clone()), None).await.expect("a list").iter().map(|r| r.id.clone()).collect::<Vec<_>>()
        };
        let rows = ids().await;
        let events = rt.sessions().history(&b.ws, None).await.expect("a history").len();
        let refused = refusal_of(rt.sessions().fork(forking("x", &thread_id, Some(&turns[1]), None), None)).await;
        assert_eq!(refused.fix.as_deref(), Some(FORK_REFUSED_FIX));
        assert_eq!(refused.kind, Some(Kind::Conflict));
        assert_eq!(refused.message, format!("{}. {FORK_REFUSED_FIX}", fork_refused_line("Claude Code", "the session holds no message a2")));
        assert_eq!(b.claude.forks.borrow().len(), 1);
        assert_eq!(ids().await, rows);
        assert_eq!(rt.sessions().history(&b.ws, None).await.expect("a history").len(), events);
        assert_eq!(b.claude.starts.borrow().len(), 3);
        rt.close().await.expect("a close");
    });
}

// "refuses a turn that named no anchor in the agent's words, and an agent other than the source's"
#[test]
fn refuses_a_turn_that_named_no_anchor_in_the_agents_words_and_an_agent_other_than_the_sources() {
    drive(|| async {
        let b = boxed(BoxOpts { claude: AgentOptions { anchorless: vec![2], ..Default::default() }, ..Default::default() }).await;
        let rt = &*b.fx.rt;
        let (thread_id, turns) = three_turns(rt, &b.ws, "claude", None, None, None).await;
        let refused = refusal_of(rt.sessions().fork(forking("x", &thread_id, Some(&turns[1]), None), None)).await;
        assert!(refused.message.contains(&fork_no_anchor_line("Claude Code")), "{}", refused.message);
        assert!(b.claude.forks.borrow().is_empty());
        let mut other = forking("x", &thread_id, None, None);
        other.start.harness = Some("codex".into());
        let agent_other = refusal_of(rt.sessions().fork(other, None)).await;
        assert_eq!(agent_other.kind, Some(Kind::Usage));
        assert!(agent_other.message.contains(&fork_agent_line("Claude Code", "Codex")), "{}", agent_other.message);
        assert!(b.claude.forks.borrow().is_empty());
        rt.close().await.expect("a close");
    });
}

/// What a host from before wrote for a thread rewound to its first turn: the anchor under resumeAt.
async fn owe_a_cut(store: &Rc<dyn Store>, ws: &str, thread: &str) {
    let mut doc = store.get(SESSIONS, ws).await.expect("a read").expect("the sessions record");
    doc["threads"][thread]["resumeAt"] = json!("a1");
    store.put(SESSIONS, ws, doc).await.expect("a write");
}

/// `expect(kept.threads[thread]).not.toHaveProperty("resumeAt")` and the same for `cutOwed`.
async fn owes_no_cut(store: &Rc<dyn Store>, ws: &str, thread: &str) {
    let kept = store.get(SESSIONS, ws).await.expect("a read").expect("the sessions record");
    assert!(kept["threads"][thread].get("resumeAt").is_none(), "{}", kept["threads"][thread]);
    assert!(kept["threads"][thread].get("cutOwed").is_none(), "{}", kept["threads"][thread]);
}

// "a Claude Code thread a host before copied rewinds left owing a cut" > "resumes a copy through that cut on its next send, every row of it moved onto the copy, and owes it no more"
#[test]
fn resumes_a_copy_through_that_cut_on_its_next_send_every_row_of_it_moved_onto_the_copy_and_owes_it_no_more() {
    drive(|| async {
        let b = boxed(BoxOpts::default()).await;
        let store = b.state.store.clone();
        let (thread_id, _) = three_turns(&b.fx.rt, &b.ws, "claude", None, None, None).await;
        let session = thread_of(&b.fx.rt, &b.ws, &thread_id).await.expect("the thread").claude_session_id.expect("its session");
        b.fx.rt.close().await.expect("a close");
        owe_a_cut(&store, &b.ws, &thread_id).await;
        let again = b.again();
        let rt = &*again.rt;
        rt.sessions().start(&b.ws, on_thread("what do you remember?", &thread_id), None).await.expect("a start").finished().await;
        assert_eq!(*b.claude.forks.borrow(), [(session, anchor("a1"))]);
        assert_eq!(b.claude.starts.borrow().last().and_then(|s| s.resume.clone()).as_deref(), Some(COPY));
        let rows = rt.sessions().list(Some(b.ws.clone()), None).await.expect("a list");
        assert!(rows
            .iter()
            .filter(|r| r.thread_id.as_deref() == Some(thread_id.as_str()))
            .all(|r| r.claude_session_id.as_deref() == Some(COPY)));
        rt.sessions().start(&b.ws, on_thread("and now?", &thread_id), None).await.expect("a start").finished().await;
        assert_eq!(b.claude.forks.borrow().len(), 1);
        assert_eq!(b.claude.starts.borrow().last().and_then(|s| s.resume.clone()).as_deref(), Some(COPY));
        rt.close().await.expect("a close");
        owes_no_cut(&store, &b.ws, &thread_id).await;
    });
}

// "a Claude Code thread owing a cut whose session file is gone" > "drops the cut on the first send and resumes as before it, so the lost-session road runs, and is never refused again"
#[test]
fn drops_the_cut_on_the_first_send_and_resumes_as_before_it_so_the_lost_session_road_runs_and_is_never_refused_again() {
    drive(|| async {
        let claude = AgentOptions { refuse: Some("No conversation found with session ID: gone".into()), ..Default::default() };
        let b = boxed(BoxOpts { claude, ..Default::default() }).await;
        let store = b.state.store.clone();
        let (thread_id, _) = three_turns(&b.fx.rt, &b.ws, "claude", None, None, None).await;
        let session = thread_of(&b.fx.rt, &b.ws, &thread_id).await.expect("the thread").claude_session_id.expect("its session");
        b.fx.rt.close().await.expect("a close");
        owe_a_cut(&store, &b.ws, &thread_id).await;
        let again = b.again();
        let rt = &*again.rt;
        for prompt in ["one", "two", "three"] {
            let result = rt.sessions().start(&b.ws, on_thread(prompt, &thread_id), None).await.expect("a start").finished().await;
            assert_eq!(j(&result)["status"], "completed");
            assert_eq!(b.claude.starts.borrow().last().and_then(|s| s.resume.clone()), Some(session.clone()));
        }
        assert_eq!(*b.claude.forks.borrow(), [(session, anchor("a1"))]);
        rt.close().await.expect("a close");
        owes_no_cut(&store, &b.ws, &thread_id).await;
    });
}

// "a fork of a Codex thread at a finished turn" > "hands the start the turn to fork through, resuming nothing, and its next send resumes the thread the fork answered"
#[test]
fn hands_the_start_the_turn_to_fork_through_resuming_nothing_and_its_next_send_resumes_the_thread_the_fork_answered() {
    drive(|| async {
        let b = boxed(BoxOpts::default()).await;
        let rt = &*b.fx.rt;
        let (thread_id, turns) = three_turns(rt, &b.ws, "codex", Some("gpt-5.5"), None, None).await;
        let source = thread_of(rt, &b.ws, &thread_id).await.expect("the source").claude_session_id.expect("its session");
        let run = rt
            .sessions()
            .fork(forking("list every code word you remember", &thread_id, Some(&turns[1]), None), None)
            .await
            .expect("a fork");
        run.finished().await;
        assert_eq!(*b.codex.forks.borrow(), [(source.clone(), anchor("a2"))]);
        let start = b.codex.starts.borrow().last().cloned().expect("a start");
        assert_eq!(start.fork, Some((source, "a2".to_string())));
        assert_eq!(start.model.as_deref(), Some("gpt-5.5"));
        assert!(start.resume.is_none());
        let fork_id = run.view().thread_id.expect("the fork's thread");
        let announced = thread_of(rt, &b.ws, &fork_id).await.expect("the fork").claude_session_id.expect("its session");
        assert!(announced.starts_with("f0f0f0f0"), "{announced}");
        rt.sessions().start(&b.ws, on_thread("and now?", &fork_id), None).await.expect("a start").finished().await;
        let last = b.codex.starts.borrow().last().cloned().expect("a start");
        assert_eq!(last.resume, Some(announced));
        assert!(last.fork.is_none());
        rt.close().await.expect("a close");
    });
}

// "finds a turn that named no anchor by the source's turns after it, a compaction's turn among them"
#[test]
fn finds_a_turn_that_named_no_anchor_by_the_sources_turns_after_it_a_compactions_turn_among_them() {
    drive(|| async {
        let b = boxed(BoxOpts {
            codex: AgentOptions { anchorless: vec![1], ..Default::default() },
            record_codex_forks: true,
            ..Default::default()
        })
        .await;
        let rt = &*b.fx.rt;
        let (thread_id, turns) = three_turns(rt, &b.ws, "codex", None, None, None).await;
        rt.sessions().start(&b.ws, on_thread("/compact", &thread_id), None).await.expect("a start").finished().await;
        settle().await;
        rt.sessions().fork(forking("x", &thread_id, Some(&turns[0]), None), None).await.expect("a fork").finished().await;
        let turn = b.codex_forks.borrow().last().cloned().expect("a fork was readied");
        let ForkTurn::After(after) = turn else { panic!("the turn was named by an anchor: {turn:?}") };
        let anchors: Vec<Option<String>> = after.iter().map(|t| t.anchor.clone()).collect();
        assert_eq!(anchors, [Some("a2".to_string()), Some("a3".to_string()), Some("a4".to_string())]);
        rt.close().await.expect("a close");
    });
}

// "refuses in two halves a turn Codex does not list, leaving nothing behind"
#[test]
fn refuses_in_two_halves_a_turn_codex_does_not_list_leaving_nothing_behind() {
    drive(|| async {
        let b =
            boxed(BoxOpts { codex: AgentOptions { refuse: Some(codex_no_turn_line("a2")), ..Default::default() }, ..Default::default() })
                .await;
        let rt = &*b.fx.rt;
        let (thread_id, turns) = three_turns(rt, &b.ws, "codex", None, None, None).await;
        let rows = rt.sessions().list(Some(b.ws.clone()), None).await.expect("a list").len();
        let refused = refusal_of(rt.sessions().fork(forking("x", &thread_id, Some(&turns[1]), None), None)).await;
        assert_eq!(refused.message, format!("{}. {FORK_REFUSED_FIX}", fork_refused_line("Codex", &codex_no_turn_line("a2"))));
        assert_eq!(refused.kind, Some(Kind::Conflict));
        assert_eq!(rt.sessions().list(Some(b.ws.clone()), None).await.expect("a list").len(), rows);
        assert_eq!(b.codex.starts.borrow().len(), 3);
        rt.close().await.expect("a close");
    });
}

/// "refuses a fork through the source's running turn before anything is written, and forks an earlier turn while it
/// runs, on %s".
async fn a_fork_while_its_source_runs(harness: &str) {
    let hang = AgentOptions { hang_from: Some(3), ..Default::default() };
    let b = if harness == "claude" {
        boxed(BoxOpts { claude: hang, ..Default::default() }).await
    } else {
        boxed(BoxOpts { codex: hang, ..Default::default() }).await
    };
    let rt = &*b.fx.rt;
    let asked = if harness == "claude" { &b.claude } else { &b.codex };
    let first = rt
        .sessions()
        .start(&b.ws, StartOptions { harness: Some(harness.into()), ..start("Remember ALPHA.") }, None)
        .await
        .expect("a start");
    first.finished().await;
    let thread_id = first.view().thread_id.expect("a thread");
    rt.sessions().start(&b.ws, on_thread("Remember BETA.", &thread_id), None).await.expect("a start").finished().await;
    let running = rt.sessions().start(&b.ws, on_thread("Remember GAMMA.", &thread_id), None).await.expect("a start");
    settle().await;
    let rows = rt.sessions().list(Some(b.ws.clone()), None).await.expect("a list").len();
    let refused = refusal_of(rt.sessions().fork(forking("x", &thread_id, Some(&running.turn_id()), None), None)).await;
    assert_eq!(refused.message, format!("{FORK_RUNNING_LINE}. {FORK_RUNNING_FIX}"));
    assert_eq!(refused.fix.as_deref(), Some(FORK_RUNNING_FIX));
    assert_eq!(refused.kind, Some(Kind::Conflict));
    assert!(asked.forks.borrow().is_empty());
    assert_eq!(rt.sessions().list(Some(b.ws.clone()), None).await.expect("a list").len(), rows);
    let earlier = rt.sessions().fork(forking("x", &thread_id, Some(&first.turn_id()), None), None).await.expect("a fork");
    let forks = asked.forks.borrow().clone();
    assert_eq!(forks.len(), 1);
    assert!(!forks[0].0.is_empty());
    assert_eq!(forks[0].1, anchor("a1"));
    assert_ne!(earlier.view().thread_id.as_deref(), Some(thread_id.as_str()));
    rt.sessions().interrupt(&running.id(), None, None).await.expect("an interrupt");
    rt.close().await.expect("a close");
}

// "a fork while its source runs" > "refuses a fork through the source's running turn before anything is written, and forks an earlier turn while it runs, on %s" (claude)
#[test]
fn refuses_a_fork_through_the_sources_running_turn_before_anything_is_written_and_forks_an_earlier_turn_while_it_runs_on_claude() {
    drive(|| a_fork_while_its_source_runs("claude"));
}

// "refuses a fork through the source's running turn before anything is written, and forks an earlier turn while it runs, on %s" (codex)
#[test]
fn refuses_a_fork_through_the_sources_running_turn_before_anything_is_written_and_forks_an_earlier_turn_while_it_runs_on_codex() {
    drive(|| a_fork_while_its_source_runs("codex"));
}

// "what a fork's agent offers" > "the catalog says forks off the adapter, and by count where it counts"
#[test]
fn the_catalog_says_forks_off_the_adapter_and_by_count_where_it_counts() {
    drive(|| async {
        let b = boxed(BoxOpts::default()).await;
        let rt = &*b.fx.rt;
        let listed: Vec<Value> = rt.harnesses().list(Some(b.ws.clone()), None, None).await.expect("a catalog").iter().map(j).collect();
        let of = |id: &str| listed.iter().find(|c| c["harness"] == id).cloned().expect("the agent's catalog");
        assert_matches(&of("claude"), json!({ "forks": true }));
        assert!(of("claude").get("forksByCount").is_none());
        assert_matches(&of("codex"), json!({ "forks": true, "forksByCount": true }));
        rt.close().await.expect("a close");
    });
}

/// A host on this computer over a git project, its agent on the copy road, its daemon checkpointing the repo for real.
struct Here {
    fx: Fx,
    _state: State,
    repo: PathBuf,
    copier: CopierLog,
    asked: Rc<Asked>,
    ws: String,
    _root: tempfile::TempDir,
}

async fn here(o: AgentOptions) -> Here {
    let dir = tempfile::tempdir().expect("a folder");
    let root = std::fs::canonicalize(dir.path()).expect("a real path");
    let repo = root.join("acme");
    std::fs::create_dir(&repo).expect("the project's folder");
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("a.txt"), "a\n").expect("a file");
    std::fs::write(repo.join("b.txt"), "b\n").expect("a file");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "first"]);
    let state = root.join("state");
    std::fs::create_dir_all(&state).expect("the state folder");
    let (copier, log) = git_copier();
    let asked = Rc::new(Asked::default());
    let folder = root.display().to_string();
    let home = folder.clone();
    let runs = format!("{folder}/runs");
    let exec_root = folder.clone();
    let local = LocalWiring {
        backend: Arc::new(LocalBackend::new(LocalBackendOptions { root: root.clone(), env: None })),
        folder: folder.clone(),
        exec_stream: Rc::new(move |opts, waiting| {
            let mut local = wsp_core::local_exec::LocalExecOptions::new(exec_root.clone(), runs.clone());
            if let Some(opts) = opts {
                local.idle_ms = opts.idle_ms;
                local.deadline_ms = opts.deadline_ms;
                local.now = opts.now;
                local.poll_ms = opts.poll_ms;
                local.reading = opts.reading;
            }
            wsp_core::local_exec::local_exec_stream(local, waiting)
        }),
        home: Rc::new(move |_| format!("{home}/.claude")),
        home_dir: folder.clone(),
        roots_path: format!("{folder}/roots"),
        env: Rc::new(|| indexmap::IndexMap::from([("PATH".to_string(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()))])),
        daemon_road: Some(Rc::new(|| {
            Box::pin(async {
                Ok(wsp_wire::views::workspace::DaemonReachView {
                    url: "http://127.0.0.1:1".into(),
                    expires_at: wsp_wire::types::JsNumber(9_007_199_254_740_991.0),
                    daemon_token: Some("t".into()),
                })
            })
        })),
        here_daemon: None,
        restart_daemon: None,
        sys_samples: None,
        copier: Some(copier),
        platform: if cfg!(target_os = "macos") { Platform::Darwin } else { Platform::Linux },
        close: None,
    };
    let backend = StubBackend::new(None);
    let state_path = state.join("state.json").display().to_string();
    let channel = fake_daemon(Some(repo.clone())).0;
    let state = sqlite_state();
    let fx = runtime(&backend)
        .store(&state.store)
        .adapter("claude", agent(Road::Copy, &asked, o))
        .with(move |opts| {
            opts.local = Some(local);
            opts.state_path = Some(state_path);
            opts.daemon_channel = Some(channel);
        })
        .build();
    let project =
        fx.rt.projects().add(ProjectAddAsk { source: repo.display().to_string(), ..Default::default() }, None).await.expect("a project");
    let at = fx
        .rt
        .workspaces()
        .folder_for(FolderForAsk { project: Some(project.project.id), ..Default::default() }, None)
        .await
        .expect("a folder");
    Here { fx, _state: state, repo, copier: log, asked, ws: at.workspace.id, _root: dir }
}

/// The checkpoint the daemon kept for a turn, off the workspace's history.
async fn checkpoint_ref(rt: &AssembledRuntime, ws: &str, turn: &str) -> String {
    let history = rt.sessions().history(ws, None).await.expect("a history");
    let found = history.iter().map(j).find(|e| e["type"] == "session.checkpoint" && e["turnId"] == turn).expect("the turn's checkpoint");
    found["ref"].as_str().expect("a ref").to_string()
}

// "a fork onto a new branch" > "makes a worktree at the turn's checkpoint, a child record of the source's folder, and launches the fork there"
#[test]
fn makes_a_worktree_at_the_turns_checkpoint_a_child_record_of_the_sources_folder_and_launches_the_fork_there() {
    drive(|| async {
        let h = here(AgentOptions::default()).await;
        let rt = &*h.fx.rt;
        let first = rt.sessions().start(&h.ws, start("edit A"), None).await.expect("a start");
        std::fs::write(h.repo.join("a.txt"), "A edited\n").expect("a file");
        first.finished().await;
        settle().await;
        let thread_id = first.view().thread_id.expect("a thread");
        let turn_one = checkpoint_ref(rt, &h.ws, &first.turn_id()).await;
        let mut ask = forking("x", &thread_id, Some(&first.turn_id()), None);
        ask.branch = Some("main-fork".into());
        let run = rt.sessions().fork(ask, None).await.expect("a fork");
        run.finished().await;
        assert_matches(&j(&h.copier.lock().unwrap().worktrees), json!([{ "branch": "main-fork", "checkpoint": turn_one }]));
        let listed: Vec<Value> = rt.workspaces().list(None).await.expect("a list").iter().map(j).collect();
        let made = listed.iter().find(|w| w["worktree"]["branch"] == "main-fork").expect("the worktree's record");
        assert_eq!(made["parentWorkspaceId"], h.ws.as_str());
        assert_eq!(j(&run.view())["workspaceId"], made["id"]);
        let last = h.asked.starts.borrow().last().cloned().expect("a start");
        assert_eq!(last.cwd.as_deref(), made["worktree"]["path"].as_str());
        assert_eq!(last.resume.as_deref(), Some(COPY));
        assert_eq!(std::fs::read_to_string(h.repo.join("a.txt")).expect("the file"), "A edited\n");
        rt.close().await.expect("a close");
    });
}

// "refuses in two halves a turn whose checkpoint is gone by the send, and makes no worktree, thread or row"
#[test]
fn refuses_in_two_halves_a_turn_whose_checkpoint_is_gone_by_the_send_and_makes_no_worktree_thread_or_row() {
    drive(|| async {
        let h = here(AgentOptions::default()).await;
        let rt = &*h.fx.rt;
        let first = rt.sessions().start(&h.ws, start("edit A"), None).await.expect("a start");
        first.finished().await;
        settle().await;
        let thread_id = first.view().thread_id.expect("a thread");
        let turn_one = checkpoint_ref(rt, &h.ws, &first.turn_id()).await;
        git(&h.repo, &["update-ref", "-d", &turn_one]);
        let rows = rt.sessions().list(Some(h.ws.clone()), None).await.expect("a list").len();
        let mut ask = forking("x", &thread_id, Some(&first.turn_id()), None);
        ask.branch = Some("main-fork".into());
        let refused = refusal_of(rt.sessions().fork(ask, None)).await;
        assert_eq!(refused.message, format!("{FORK_NO_CHECKPOINT_LINE}. {FORK_NO_CHECKPOINT_FIX}"));
        assert_eq!(refused.fix.as_deref(), Some(FORK_NO_CHECKPOINT_FIX));
        assert_eq!(refused.kind, Some(Kind::Conflict));
        assert!(h.copier.lock().unwrap().worktrees.is_empty());
        assert!(h.asked.forks.borrow().is_empty());
        assert_eq!(rt.sessions().list(Some(h.ws.clone()), None).await.expect("a list").len(), rows);
        assert_eq!(git(&h.repo, &["branch", "--list", "main-fork"]), "");
        rt.close().await.expect("a close");
    });
}

// "takes the worktree and its branch away again where the agent then refuses, leaving no thread"
#[test]
fn takes_the_worktree_and_its_branch_away_again_where_the_agent_then_refuses_leaving_no_thread() {
    drive(|| async {
        let h = here(AgentOptions { refuse: Some("the session holds no message a1".into()), ..Default::default() }).await;
        let rt = &*h.fx.rt;
        let first = rt.sessions().start(&h.ws, start("edit A"), None).await.expect("a start");
        first.finished().await;
        settle().await;
        let thread_id = first.view().thread_id.expect("a thread");
        let records = rt.workspaces().list(None).await.expect("a list").len();
        let mut ask = forking("x", &thread_id, Some(&first.turn_id()), None);
        ask.branch = Some("main-fork".into());
        let refused = refusal_of(rt.sessions().fork(ask, None)).await;
        assert_eq!(refused.fix.as_deref(), Some(FORK_REFUSED_FIX));
        assert_eq!(refused.kind, Some(Kind::Conflict));
        assert_eq!(h.asked.forks.borrow().len(), 1);
        assert_eq!(h.copier.lock().unwrap().worktrees.len(), 1);
        assert_eq!(h.copier.lock().unwrap().worktrees_removed.len(), 1);
        assert_eq!(git(&h.repo, &["branch", "--list", "main-fork"]), "");
        assert_eq!(git(&h.repo, &["worktree", "list"]).split('\n').count(), 1);
        assert_eq!(rt.workspaces().list(None).await.expect("a list").len(), records);
        let all = rt.sessions().list(None, None).await.expect("a list");
        assert_eq!(fold_threads(&all).iter().map(|t| t.thread_id.clone()).collect::<Vec<_>>(), [Some(thread_id)]);
        rt.close().await.expect("a close");
    });
}

// "keeps an image of the copied history openable in the fork after the source is deleted"
#[test]
fn keeps_an_image_of_the_copied_history_openable_in_the_fork_after_the_source_is_deleted() {
    drive(|| async {
        let h = here(AgentOptions::default()).await;
        let rt = &*h.fx.rt;
        let opts = StartOptions { request_id: Some("req_png".into()), attachments: Some(vec![png()]), ..start("what is this?") };
        let first = rt.sessions().start(&h.ws, opts, None).await.expect("a start");
        first.finished().await;
        settle().await;
        let thread_id = first.view().thread_id.expect("a thread");
        let run = rt.sessions().fork(forking("x", &thread_id, Some(&first.turn_id()), None), None).await.expect("a fork");
        run.finished().await;
        let fork_id = run.view().thread_id.expect("the fork's thread");
        rt.sessions().delete(&thread_id, None, DeleteThreadAsk::default()).await.expect("a delete");
        let kept = rt.sessions().attachment(&h.ws, &fork_id, "req_png", 0, None).await.expect("the image");
        assert_eq!(j(&kept), json!({ "mediaType": "image/png", "bytes": PNG }));
        let gone = rt.sessions().attachment(&h.ws, &thread_id, "req_png", 0, None).await.expect_err("a refusal");
        assert_eq!(gone.kind, Some(Kind::NotFound));
        // The source gone, the fork names the title it had at the fork.
        let from = j(&thread_of(rt, &h.ws, &fork_id).await.expect("the fork").forked_from);
        assert_eq!(from, json!({ "threadId": thread_id, "turnId": first.turn_id(), "title": "what is this?" }));
        rt.close().await.expect("a close");
    });
}

// "a fork's start beside what its source decides" > "is refused as usage beside a thread or a restart"
#[test]
fn is_refused_as_usage_beside_a_thread_or_a_restart() {
    drive(|| async {
        let b = boxed(BoxOpts::default()).await;
        let rt = &*b.fx.rt;
        let (thread_id, _) = three_turns(rt, &b.ws, "claude", None, None, None).await;
        let beside = StartOptions { fork: Some(source_of(&thread_id, None, None)), thread: Some(thread_id.clone()), ..start("x") };
        let refused = refusal_of(rt.sessions().start(&b.ws, beside, None)).await;
        assert_eq!(refused.fix.as_deref(), Some(FORK_BESIDE_FIX));
        assert_eq!(refused.kind, Some(Kind::Usage));
        rt.close().await.expect("a close");
    });
}

// "is refused as usage beside a conversation kept outside wsp, in process and over the wire, before anything starts"
#[test]
fn is_refused_as_usage_beside_a_conversation_kept_outside_wsp_in_process_and_over_the_wire_before_anything_starts() {
    drive(|| async {
        let b = boxed(BoxOpts::default()).await;
        let rt = &*b.fx.rt;
        let (thread_id, _) = three_turns(rt, &b.ws, "claude", None, None, None).await;
        let starts = b.claude.starts.borrow().len();
        let outside_id = session_of("0d0d0d0d", 1);
        let outside = OutsideOpening {
            harness: "claude".into(),
            id: outside_id.clone(),
            copy: true,
            project: "lab".into(),
            cwd: None,
            title: "lab".into(),
            earlier: vec![],
        };
        let beside = StartOptions { fork: Some(source_of(&thread_id, None, None)), outside: Some(outside), ..start("x") };
        let refused = refusal_of(rt.sessions().start(&b.ws, beside, None)).await;
        assert_eq!(refused.message, format!("{FORK_RESUME_LINE}. {FORK_RESUME_FIX}"));
        assert_eq!(refused.kind, Some(Kind::Usage));
        let served = serve_runtime(b.fx.rt.clone(), ServeOptions { port: Some(0), auth_token: "t".into(), ..Default::default() })
            .await
            .expect("the runtime serves");
        let res = ws_request(
            served.port(),
            "t",
            "sessions.start",
            json!({ "prompt": "x", "fork": { "threadId": thread_id }, "resume": { "id": outside_id, "copy": true } }),
        )
        .await;
        served.close().await.expect("a close");
        assert_matches(&res, json!({ "ok": false, "error": format!("{FORK_RESUME_LINE}. {FORK_RESUME_FIX}") }));
        assert_eq!(b.claude.starts.borrow().len(), starts);
        assert!(b.claude.forks.borrow().is_empty());
        rt.close().await.expect("a close");
    });
}
