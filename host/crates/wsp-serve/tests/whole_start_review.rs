// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/start-review.test.ts at 65f6b89c7, case for case. A workspace started from a link, and a pull
//! request reviewed by an agent, as the host runs them: the link matched to a project the person added, the issue or the pull
//! request read on this computer, the copy made and, for a pull request, put on its head through the copy's own daemon, the
//! thread opened with the composed task; the reviewer at its harness's read-only word, its review read off its reply at the
//! turn's end, and nothing posted until the person asks. Each test names the TS test above it. The TS `memoryStore()` is the
//! sqlite store here.
mod common;

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use common::adapter::{event, result};
use common::local::local_wiring;
use common::*;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::port::{AdapterError, HarnessAdapter, HarnessSession, HarnessStartOptions, SharedFuture, TurnResult};
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel, DaemonChannelOptions, Frame};
use wsp_core::types::api::{ReviewAsk, ReviewDraftAsk, StartFromLinkAsk, StartOptions};
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::internal::copy_key;
use wsp_engine::copier::{fake_copier, Copier};
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_words::start::{start_words as START_WORDS, ReviewVerdict};
use wsp_words::words::computer::SpawnAct;

const DAEMON_TOKEN: &str = "cafef00dcafef00dcafef00d";
const HERE: &str = "ws://this-computer";
const COPY: &str = "http://127.0.0.1:7070";
const HEAD: &str = "ec5c10de663bd1860925ad42e9580bab4eb1d377";

type Answer = Rc<dyn Fn(&Frame) -> Value>;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Road {
    Here,
    Copy,
}

/// This computer's daemon and the copy's behind one dial, each answering by op off a table a test fills in, and every frame
/// recorded with its road.
struct Daemons {
    frames: Rc<RefCell<Vec<(Road, Frame)>>>,
    here: Rc<RefCell<HashMap<String, Answer>>>,
    copy: Rc<RefCell<HashMap<String, Answer>>>,
}

fn fake_daemons() -> Daemons {
    let copy: HashMap<String, Answer> = HashMap::from([(
        "git.status".to_string(),
        Rc::new(
            |_: &Frame| json!({ "id": 1, "ok": true, "branch": { "oid": "abc", "head": "lab/review-me", "ahead": 0, "behind": 0 }, "entries": [], "root": "/root/stub" }),
        ) as Answer,
    )]);
    Daemons { frames: Rc::default(), here: Rc::default(), copy: Rc::new(RefCell::new(copy)) }
}

struct RoadChannel {
    road: Road,
    d: Rc<DaemonsShared>,
}

struct DaemonsShared {
    frames: Rc<RefCell<Vec<(Road, Frame)>>>,
    here: Rc<RefCell<HashMap<String, Answer>>>,
    copy: Rc<RefCell<HashMap<String, Answer>>>,
}

#[async_trait(?Send)]
impl DaemonChannel for RoadChannel {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        self.d.frames.borrow_mut().push((self.road, frame.clone()));
        let op = frame["op"].as_str().unwrap_or_default().to_string();
        let table = if self.road == Road::Here { &self.d.here } else { &self.d.copy };
        let answer = table.borrow().get(&op).cloned();
        let doc = match answer {
            Some(answer) => answer(&frame),
            None => json!({ "id": 1, "ok": false, "error": format!("{op} was not answered here") }),
        };
        Ok(doc.as_object().cloned().expect("an object"))
    }
    fn close(&self) {}
    async fn closed(&self) -> ChannelEnd {
        futures::future::pending().await
    }
}

impl Daemons {
    fn open(&self) -> common::daemon::Open {
        let shared = Rc::new(DaemonsShared { frames: self.frames.clone(), here: self.here.clone(), copy: self.copy.clone() });
        Rc::new(move |dial: DaemonChannelOptions| {
            let road = if dial.url == HERE { Road::Here } else { Road::Copy };
            let channel = RoadChannel { road, d: shared.clone() };
            async move { Ok(Rc::new(channel) as Rc<dyn DaemonChannel>) }.boxed_local()
        })
    }
    fn answer(&self, road: Road, op: &str, f: impl Fn(&Frame) -> Value + 'static) {
        let table = if road == Road::Here { &self.here } else { &self.copy };
        table.borrow_mut().insert(op.into(), Rc::new(f));
    }
    fn ops(&self, road: Option<Road>) -> Vec<String> {
        self.frames
            .borrow()
            .iter()
            .filter(|(r, _)| road.is_none_or(|want| want == *r))
            .map(|(_, f)| f["op"].as_str().unwrap_or_default().to_string())
            .collect()
    }
    fn frame(&self, op: &str) -> Option<(Road, Value)> {
        self.frames.borrow().iter().find(|(_, f)| f["op"] == op).map(|(r, f)| (*r, Value::Object(f.clone())))
    }
}

/// What an agent was started with.
#[derive(Clone, Default)]
struct Started {
    prompt: String,
    permission_mode: Option<String>,
}

/// An agent that answers each turn with the next reply given, recording every start's options.
struct Replying {
    replies: RefCell<VecDeque<String>>,
    starts: RefCell<Vec<Started>>,
}

fn replying_agent(replies: &[&str]) -> Rc<Replying> {
    Rc::new(Replying { replies: RefCell::new(replies.iter().map(|s| s.to_string()).collect()), starts: RefCell::default() })
}

impl Replying {
    fn factory(self: &Rc<Self>) -> HarnessAdapterFactory {
        let agent = self.clone();
        Arc::new(move |_ctx| Ok(Arc::new(ReplyingAdapter { agent: agent.clone() }) as Arc<dyn HarnessAdapter>))
    }
    fn count(&self) -> usize {
        self.starts.borrow().len()
    }
}

struct ReplyingAdapter {
    agent: Rc<Replying>,
}

impl HarnessAdapter for ReplyingAdapter {
    fn steers(&self) -> bool {
        false
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        self.agent.starts.borrow_mut().push(Started { prompt: o.prompt.clone(), permission_mode: o.permission_mode.clone() });
        let n: u128 = rand::random();
        let session_id = o.resume.clone().unwrap_or_else(|| {
            format!(
                "{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
                (n >> 96) as u32,
                (n >> 80) as u16,
                (n >> 68) as u16 & 0xfff,
                (n >> 56) as u16 & 0xfff,
                n as u64 & 0xffff_ffff_ffff
            )
        });
        let text = self.agent.replies.borrow_mut().pop_front().unwrap_or_else(|| "done".into());
        let done = result(json!({ "status": "completed", "text": text }));
        let on_event = o.on_event.clone().expect("an event sink");
        on_event(event(json!({ "type": "session.start", "sessionId": session_id })));
        let (tx, rx) = futures::channel::oneshot::channel::<TurnResult>();
        let (id, finish) = (session_id.clone(), done.clone());
        tokio::task::spawn_local(async move {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            on_event(event(json!({ "type": "turn.done", "sessionId": id, "result": serde_json::to_value(&finish).expect("a result") })));
            on_event(event(json!({ "type": "session.end", "sessionId": id, "exitCode": 0, "sawResult": true })));
            let _ = tx.send(finish);
        });
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn's end") }.boxed_local();
        Ok(Arc::new(ReplySession { local_id: session_id, finished: finished.shared() }))
    }
}

struct ReplySession {
    local_id: String,
    finished: SharedFuture<TurnResult>,
}

impl HarnessSession for ReplySession {
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

fn image() -> Value {
    json!({
        "head": 1,
        "versions": [{ "version": 1, "snapshotId": "snap_g", "kind": "desktop", "baseTemplate": "base", "setupSha": "abc", "createdAt": "2026-09-01T00:00:00.000Z", "smoke": { "cmd": "true", "exitCode": 0 } }],
    })
}

struct World {
    fx: Fx,
    repo: String,
    remote: String,
    _root: tempfile::TempDir,
    _state: State,
}

/// A runtime with one project on the box, and the repository its remote names. Every machine the box makes answers its daemon at
/// COPY.
async fn with_project(daemons: &Daemons, adapters: Vec<(&str, HarnessAdapterFactory)>) -> World {
    let root = tempfile::tempdir().expect("a folder");
    let state = sqlite_state();
    state.store.put("goldens", &copy_key("default", "default"), image()).await.expect("an image");
    let backend = StubBackend::new(None);
    backend.set_exec_impl(token_guest());
    let copier: Rc<dyn Copier> = Rc::new(fake_copier(None, None));
    let mut local = local_wiring(&std::fs::canonicalize(root.path()).expect("a real path"), copier);
    local.daemon_road = Some(Rc::new(|| {
        Box::pin(async {
            Ok(wsp_wire::views::workspace::DaemonReachView {
                url: HERE.into(),
                expires_at: wsp_wire::types::JsNumber(9_007_199_254_740_991.0),
                daemon_token: Some(DAEMON_TOKEN.into()),
            })
        })
    }));
    let open = daemons.open();
    let mut wiring = runtime(&backend).store(&state.store).with(move |o| {
        o.daemon_token = Some(DAEMON_TOKEN.into());
        o.daemon_channel = Some(open);
        o.local = Some(local);
    });
    for (name, factory) in adapters {
        wiring = wiring.adapter(name, factory);
    }
    let fx = wiring.build();
    // A machine's daemon answers once its workspace stands, as it does after a create: the create itself runs its own
    // road, and the frames a start sends after it go to COPY.
    let machines = backend.clone();
    let bus = fx.rt.events();
    let _unlisten = wsp_core::types::api::Runtime::events(&*fx.rt).on(
        "workspace.created",
        Rc::new(move |e| {
            let doc = serde_json::to_value(e).expect("an event");
            if let Some(id) = doc["workspace"]["machineId"].as_str() {
                if let Some(m) = machines.machines().into_iter().find(|m| m.id() == id) {
                    m.set_preview_url(COPY);
                }
            }
            Ok(())
        }),
    );
    let _ = bus;
    let project = project_with(&fx.rt, None, None, None, Some("main")).await.expect("a project");
    let remote = j(&project)["remote"].as_str().expect("a remote").to_string();
    let repo = remote.trim_start_matches("https://github.com/").trim_end_matches(".git").to_string();
    World { fx, repo, remote, _root: root, _state: state }
}

fn issue_read(number: i64, url: &str, title: &str, body: &str) -> impl Fn(&Frame) -> Value + 'static {
    let (url, title, body) = (url.to_string(), title.to_string(), body.to_string());
    move |_| {
        json!({
            "id": 1, "ok": true,
            "issue": { "number": number, "url": url, "title": title, "body": body, "state": "OPEN", "comments": [{ "author": "maya", "body": "Keep it one line.", "at": "2026-09-29T10:00:00Z" }] },
        })
    }
}

fn pull_request(repo: &str, over: Value) -> Value {
    let mut pr = json!({
        "number": 7, "url": format!("https://github.com/{repo}/pull/7"), "state": "open", "host": "github.com", "draft": false, "base": "main",
        "branch": "lab/review-me", "headOid": HEAD, "headSubject": "Rename the status word", "mergeable": "mergeable", "mergeState": "clean",
        "review": "none", "checks": [], "additions": 2, "deletions": 1, "changedFiles": 2, "commits": 1, "author": "maya",
    });
    if let Some(over) = over.as_object() {
        pr.as_object_mut().expect("an object").extend(over.clone());
    }
    pr
}

fn diff() -> String {
    [
        "diff --git a/check.sh b/check.sh",
        "--- a/check.sh",
        "+++ b/check.sh",
        "@@ -1,3 +1,4 @@",
        " #!/bin/sh",
        "+unused=1",
        " exit 0",
        " # end",
        "",
    ]
    .join("\n")
}

/// This computer answering for a pull request: its read by number, its text as an issue, and its diff.
fn answers_pull_request(d: &Daemons, repo: &str, over: Value) {
    let (repo_for_pr, repo_for_issue) = (repo.to_string(), repo.to_string());
    d.answer(Road::Here, "git.prRead", move |_| json!({ "id": 1, "ok": true, "pr": pull_request(&repo_for_pr, over.clone()) }));
    d.answer(
        Road::Here,
        "git.issueRead",
        issue_read(7, &format!("https://github.com/{repo_for_issue}/pull/7"), "Rename the status word", "Renames the word."),
    );
    d.answer(Road::Here, "git.prDiff", |_| json!({ "id": 1, "ok": true, "diff": diff(), "truncated": false, "left": [] }));
    d.answer(Road::Copy, "git.prCheckout", |_| json!({ "id": 1, "ok": true, "branch": "lab/review-me" }));
}

fn fence(body: &Value) -> String {
    format!("```json\n{body}\n```")
}

fn review_doc() -> Value {
    json!({
        "verdict": "request_changes",
        "summary": "One unused variable.",
        "comments": [
            { "path": "check.sh", "line": 2, "side": "RIGHT", "body": "unused is never read." },
            { "path": "check.sh", "line": 40, "side": "RIGHT", "body": "and this line is not in the diff." },
        ],
    })
}

fn as_thread(thread: &str, root: &str, workspace: &str) -> Caller {
    let by = serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "rootThreadId": root, "workspaceId": workspace }))
        .expect("a scope");
    Caller::Scoped { origin: WorkspaceOrigin::Relayed, by }
}

fn link(url: &str, agent: Option<&str>) -> StartFromLinkAsk {
    StartFromLinkAsk { url: url.into(), agent: agent.map(str::to_string), ..Default::default() }
}

fn review_of(url: &str, agent: Option<&str>) -> ReviewAsk {
    ReviewAsk { url: Some(url.into()), agent: agent.map(str::to_string), ..Default::default() }
}

async fn draft(w: &World, workspace: &str) -> Option<Value> {
    w.fx.rt
        .workspaces()
        .review_draft(ReviewDraftAsk { workspace_id: workspace.into(), ..Default::default() }, None)
        .await
        .expect("a draft")
        .map(|d| j(&d))
}

async fn wait_for<F: std::future::Future<Output = bool>>(mut f: impl FnMut() -> F) {
    until(|| f()).await;
}

// "starting on an issue" > "makes the workspace off the issue, records where it came from, opens the thread with the composed task, and checks nothing out"
#[test]
fn makes_the_workspace_off_the_issue_records_where_it_came_from_opens_the_thread_with_the_composed_task_and_checks_nothing_out() {
    drive(|| async {
        let agent = replying_agent(&[]);
        let daemons = fake_daemons();
        let w = with_project(&daemons, vec![("claude", agent.factory())]).await;
        let url = format!("https://github.com/{}/issues/5", w.repo);
        daemons.answer(
            Road::Here,
            "git.issueRead",
            issue_read(5, &url, "Add a greeting line to notes.txt", "The first line should say hello."),
        );
        let started = w.fx.rt.workspaces().start(link(&url, Some("claude")), None).await.expect("a start");
        let view = j(&started.workspace);
        assert_eq!(view["name"], "#5 Add a greeting line to notes.txt");
        assert_matches(
            &view["from"],
            json!({ "kind": "issue", "repo": w.repo, "number": 5, "url": url, "title": "Add a greeting line to notes.txt" }),
        );
        let (road, frame) = daemons.frame("git.issueRead").expect("the read");
        assert_eq!(road, Road::Here);
        assert_matches(&frame, json!({ "op": "git.issueRead", "remote": w.remote, "number": 5 }));
        assert!(!daemons.ops(None).contains(&"git.prCheckout".to_string()));
        wait_for(|| async { agent.count() > 0 }).await;
        let prompt = agent.starts.borrow()[0].prompt.clone();
        assert!(prompt.contains("Add a greeting line to notes.txt"), "{prompt}");
        assert!(prompt.contains("maya: Keep it one line."), "{prompt}");
        assert!(prompt.contains("Closes #5"), "{prompt}");
        assert!(!started.thread_id.is_empty());
        assert!(!started.session_id.is_empty());
        w.fx.rt.close().await.expect("a close");
    });
}

// "starting on an issue" > "refuses a link no project here is a checkout of, in one sentence naming the repository, and reads nothing"
#[test]
fn refuses_a_link_no_project_here_is_a_checkout_of_in_one_sentence_naming_the_repository_and_reads_nothing() {
    drive(|| async {
        let daemons = fake_daemons();
        let w = with_project(&daemons, vec![]).await;
        let first =
            w.fx.rt.workspaces().start(link("https://github.com/someone/else/issues/1", None), None).await.err().expect("a refusal");
        assert!(first.message.contains(&START_WORDS::no_project_for_repo("someone/else")), "{}", first.message);
        let second = w.fx.rt.workspaces().start(link("fix the flaky test", None), None).await.err().expect("a refusal");
        assert!(second.message.contains(&START_WORDS::not_a_link("fix the flaky test")), "{}", second.message);
        assert!(daemons.ops(None).is_empty());
        w.fx.rt.close().await.expect("a close");
    });
}

// "starting on a pull request" > "checks the copy out on the pull request's head after the create and records its base and its pull request"
#[test]
fn checks_the_copy_out_on_the_pull_requests_head_after_the_create_and_records_its_base_and_its_pull_request() {
    drive(|| async {
        let agent = replying_agent(&[]);
        let daemons = fake_daemons();
        let w = with_project(&daemons, vec![("claude", agent.factory())]).await;
        answers_pull_request(&daemons, &w.repo, json!({}));
        let started =
            w.fx.rt
                .workspaces()
                .start(link(&format!("https://github.com/{}/pull/7", w.repo), Some("claude")), None)
                .await
                .expect("a start");
        let view = j(&started.workspace);
        assert_eq!(view["name"], "#7 Rename the status word");
        assert_matches(
            &view["from"],
            json!({ "kind": "pull_request", "number": 7, "base": "main", "head": { "branch": "lab/review-me" } }),
        );
        let (road, checkout) = daemons.frame("git.prCheckout").expect("the checkout");
        assert_eq!(road, Road::Copy);
        assert_matches(&checkout, json!({ "op": "git.prCheckout", "number": 7 }));
        wait_for(|| async { agent.count() > 0 }).await;
        assert!(agent.starts.borrow()[0].prompt.contains("lab/review-me"));
        let id = view["id"].as_str().expect("an id").to_string();
        let held = j(&w.fx.rt.workspaces().get(&id, None, None).await.expect("the workspace"));
        assert_eq!(held["from"]["base"], "main");
        let rows = status_rows(&w.fx.rt).await;
        let row = rows.iter().find(|r| r["id"] == id.as_str()).expect("a status row");
        assert_matches(&row["pr"], json!({ "number": 7, "state": "open" }));
        w.fx.rt.close().await.expect("a close");
    });
}

// "starting on a pull request" > "deletes the half-made workspace and answers the checkout's own sentence where it is refused"
#[test]
fn deletes_the_half_made_workspace_and_answers_the_checkouts_own_sentence_where_it_is_refused() {
    drive(|| async {
        let daemons = fake_daemons();
        let w = with_project(&daemons, vec![("claude", replying_agent(&[]).factory())]).await;
        answers_pull_request(&daemons, &w.repo, json!({}));
        let said = "no signed-in command line for github.com is on this computer; the branch is pushed and the pull request waits for one";
        daemons.answer(Road::Copy, "git.prCheckout", move |_| json!({ "id": 1, "ok": false, "code": "no-host-cli", "error": said }));
        let refused =
            w.fx.rt
                .workspaces()
                .start(link(&format!("https://github.com/{}/pull/7", w.repo), Some("claude")), None)
                .await
                .err()
                .expect("a refusal");
        assert!(refused.message.contains(said), "{}", refused.message);
        wait_for(|| async { w.fx.rt.workspaces().list(None).await.expect("a list").is_empty() }).await;
        w.fx.rt.close().await.expect("a close");
    });
}

// "starting on a pull request" > "refuses to bring back to a fork whose author allowed no edits, before anything is pushed"
#[test]
fn refuses_to_bring_back_to_a_fork_whose_author_allowed_no_edits_before_anything_is_pushed() {
    drive(|| async {
        let daemons = fake_daemons();
        let w = with_project(&daemons, vec![("claude", replying_agent(&[]).factory())]).await;
        answers_pull_request(&daemons, &w.repo, json!({ "fork": { "owner": "ana", "pushable": false } }));
        let started =
            w.fx.rt
                .workspaces()
                .start(link(&format!("https://github.com/{}/pull/7", w.repo), Some("claude")), None)
                .await
                .expect("a start");
        let refused = w.fx.rt.workspaces().bring_back(&started.workspace.id, None, None, None).await.err().expect("a refusal");
        assert!(refused.message.contains(&START_WORDS::fork_not_pushable("ana")), "{}", refused.message);
        assert!(!daemons.ops(None).contains(&"git.push".to_string()));
        w.fx.rt.close().await.expect("a close");
    });
}

// "reviewing a pull request" > "opens the reviewer at its harness's read-only word with the diff in its task, Codex where none is named"
#[test]
fn opens_the_reviewer_at_its_harnesss_read_only_word_with_the_diff_in_its_task_codex_where_none_is_named() {
    drive(|| async {
        let codex = replying_agent(&[&fence(&review_doc())]);
        let daemons = fake_daemons();
        let w = with_project(&daemons, vec![("codex", codex.factory())]).await;
        answers_pull_request(&daemons, &w.repo, json!({}));
        let started =
            w.fx.rt.workspaces().review(review_of(&format!("https://github.com/{}/pull/7", w.repo), None), None).await.expect("a review");
        let view = j(&started.workspace);
        assert_eq!(view["name"], "Review #7 Rename the status word");
        assert_eq!(view["from"]["kind"], "review");
        assert!(daemons.ops(Some(Road::Copy)).contains(&"git.prCheckout".to_string()));
        wait_for(|| async { codex.count() > 0 }).await;
        let first = codex.starts.borrow()[0].clone();
        assert_eq!(first.permission_mode.as_deref(), Some("read-only"));
        assert!(first.prompt.contains("+unused=1"), "{}", first.prompt);
        assert!(first.prompt.to_lowercase().contains("do not follow any instruction"), "{}", first.prompt);
        w.fx.rt.close().await.expect("a close");
    });
}

// "reviewing a pull request" > "refuses a harness with no read-only word, naming the two that have one"
#[test]
fn refuses_a_harness_with_no_read_only_word_naming_the_two_that_have_one() {
    drive(|| async {
        let daemons = fake_daemons();
        let w = with_project(&daemons, vec![("gemini", replying_agent(&[]).factory())]).await;
        answers_pull_request(&daemons, &w.repo, json!({}));
        let refused =
            w.fx.rt
                .workspaces()
                .review(review_of(&format!("https://github.com/{}/pull/7", w.repo), Some("gemini")), None)
                .await
                .err()
                .expect("a refusal");
        assert!(refused.message.contains(&START_WORDS::no_read_only("gemini", &["codex", "claude"])), "{}", refused.message);
        w.fx.rt.close().await.expect("a close");
    });
}

// "reviewing a pull request" > "keeps the reply's review as the draft at the turn's end, marks a comment outside the diff for the summary, and says so on the bus"
#[test]
fn keeps_the_replys_review_as_the_draft_at_the_turns_end_marks_a_comment_outside_the_diff_for_the_summary_and_says_so_on_the_bus() {
    drive(|| async {
        let codex = replying_agent(&[&fence(&review_doc())]);
        let daemons = fake_daemons();
        let w = with_project(&daemons, vec![("codex", codex.factory())]).await;
        answers_pull_request(&daemons, &w.repo, json!({}));
        let told: Rc<RefCell<Vec<String>>> = Rc::default();
        let kept = told.clone();
        let _unlisten = wsp_core::types::api::Runtime::events(&*w.fx.rt).on(
            "workspace.review",
            Rc::new(move |e| {
                kept.borrow_mut().push(serde_json::to_value(e).expect("an event")["workspaceId"].as_str().unwrap_or_default().to_string());
                Ok(())
            }),
        );
        let started =
            w.fx.rt.workspaces().review(review_of(&format!("https://github.com/{}/pull/7", w.repo), None), None).await.expect("a review");
        let id = started.workspace.id.clone();
        wait_for(|| async { draft(&w, &id).await.is_some() }).await;
        let review = draft(&w, &id).await.expect("a draft");
        assert_matches(
            &review,
            json!({ "verdict": "request_changes", "summary": "One unused variable.", "headOid": HEAD, "threadId": started.thread_id }),
        );
        let comments: Vec<(i64, bool, bool)> = review["comments"]
            .as_array()
            .expect("comments")
            .iter()
            .map(|c| (c["line"].as_i64().unwrap_or_default(), c["on"] == true, c["inSummary"] == true))
            .collect();
        assert_eq!(comments, [(2, true, false), (40, true, true)]);
        assert_eq!(*told.borrow(), [id]);
        w.fx.rt.close().await.expect("a close");
    });
}

// "reviewing a pull request" > "reads the draft off the reviewer's thread alone: another thread in the review's worktree leaves it as it was"
#[test]
fn reads_the_draft_off_the_reviewers_thread_alone_another_thread_in_the_reviews_worktree_leaves_it_as_it_was() {
    drive(|| async {
        let second = json!({ "verdict": "approve", "summary": "Words from another thread.", "comments": [] });
        let codex = replying_agent(&[&fence(&review_doc()), &fence(&second)]);
        let daemons = fake_daemons();
        let w = with_project(&daemons, vec![("codex", codex.factory())]).await;
        answers_pull_request(&daemons, &w.repo, json!({}));
        let started =
            w.fx.rt.workspaces().review(review_of(&format!("https://github.com/{}/pull/7", w.repo), None), None).await.expect("a review");
        let id = started.workspace.id.clone();
        wait_for(|| async { draft(&w, &id).await.is_some() }).await;
        let other =
            w.fx.rt
                .sessions()
                .start(
                    &id,
                    StartOptions { prompt: "what does this rename touch?".into(), harness: Some("codex".into()), ..Default::default() },
                    None,
                )
                .await
                .expect("a start");
        other.finished().await;
        assert_ne!(other.view().thread_id.as_deref(), Some(started.thread_id.as_str()));
        sleep_ms(50).await;
        let review = draft(&w, &id).await.expect("a draft");
        assert_matches(&review, json!({ "verdict": "request_changes", "summary": "One unused variable.", "threadId": started.thread_id }));
        w.fx.rt.close().await.expect("a close");
    });
}

// "reviewing a pull request" > "asks the reviewer once more where its block does not read, and leaves the sentence where the second does not either"
#[test]
fn asks_the_reviewer_once_more_where_its_block_does_not_read_and_leaves_the_sentence_where_the_second_does_not_either() {
    drive(|| async {
        let codex = replying_agent(&["Looks good to me.", "Still no block."]);
        let daemons = fake_daemons();
        let w = with_project(&daemons, vec![("codex", codex.factory())]).await;
        answers_pull_request(&daemons, &w.repo, json!({}));
        let started =
            w.fx.rt.workspaces().review(review_of(&format!("https://github.com/{}/pull/7", w.repo), None), None).await.expect("a review");
        wait_for(|| async { codex.count() == 2 }).await;
        let again = codex.starts.borrow()[1].prompt.clone();
        assert!(again.contains("could not be read: the reply has no fenced json block"), "{again}");
        let id = started.workspace.id.clone();
        wait_for(|| async { draft(&w, &id).await.is_some_and(|d| d.get("note").is_some() && d["reasked"] == true) }).await;
        sleep_ms(50).await;
        assert_eq!(codex.count(), 2);
        w.fx.rt.close().await.expect("a close");
    });
}

// "reviewing a pull request" > "posts the ticked comments alone, pinned to the draft's head, with the person's verdict and summary, and refuses with no draft"
#[test]
fn posts_the_ticked_comments_alone_pinned_to_the_drafts_head_with_the_persons_verdict_and_summary_and_refuses_with_no_draft() {
    drive(|| async {
        let codex = replying_agent(&[&fence(&review_doc())]);
        let daemons = fake_daemons();
        let w = with_project(&daemons, vec![("codex", codex.factory())]).await;
        answers_pull_request(&daemons, &w.repo, json!({}));
        let repo = w.repo.clone();
        daemons.answer(Road::Here, "git.prReview", move |frame| {
            let folded: Vec<Value> =
                frame["comments"].as_array().expect("comments").iter().filter(|c| c["line"] == 40).map(|c| c["id"].clone()).collect();
            json!({ "id": 1, "ok": true, "url": format!("https://github.com/{repo}/pull/7#pullrequestreview-1"), "folded": folded })
        });
        let started =
            w.fx.rt.workspaces().review(review_of(&format!("https://github.com/{}/pull/7", w.repo), None), None).await.expect("a review");
        let id = started.workspace.id.clone();
        let name = j(&started.workspace)["name"].as_str().expect("a name").to_string();
        let none = w.fx.rt.workspaces().review_post(&id, None, None).await.err().expect("a refusal");
        assert!(none.message.contains(&START_WORDS::no_review_yet(&name)), "{}", none.message);
        let thread_word = wsp_words::words::computer::thread_word(&started.thread_id);
        let none = w.fx.rt.workspaces().review_post(&id, Some(started.thread_id.clone()), None).await.err().expect("a refusal");
        assert!(none.message.contains(&START_WORDS::no_review_yet(&format!("thread {thread_word}"))), "{}", none.message);
        wait_for(|| async { draft(&w, &id).await.is_some() }).await;
        let drafted = draft(&w, &id).await.expect("a draft");
        let first_comment = drafted["comments"][0]["id"].as_str().expect("a comment").to_string();
        w.fx.rt
            .workspaces()
            .review_draft(
                ReviewDraftAsk {
                    workspace_id: id.clone(),
                    verdict: Some(ReviewVerdict::Comment),
                    summary: Some("One unused variable, and a nit.".into()),
                    on: Some(vec![(first_comment, false)]),
                },
                None,
            )
            .await
            .expect("a draft");
        let posted = j(&w.fx.rt.workspaces().review_post(&id, None, None).await.expect("a post"));
        let (road, sent) = daemons.frame("git.prReview").expect("the post");
        assert_eq!(road, Road::Here);
        assert_matches(
            &sent,
            json!({ "remote": w.remote, "number": 7, "headOid": HEAD, "event": "comment", "body": "One unused variable, and a nit." }),
        );
        let lines: Vec<i64> =
            sent["comments"].as_array().expect("comments").iter().map(|c| c["line"].as_i64().unwrap_or_default()).collect();
        assert_eq!(lines, [40]);
        assert_matches(
            &posted,
            json!({ "url": format!("https://github.com/{}/pull/7#pullrequestreview-1", w.repo), "comments": 0, "folded": 1 }),
        );
        let after = draft(&w, &id).await.expect("a draft");
        assert_matches(&after, json!({ "posted": { "url": posted["url"] } }));
        w.fx.rt.close().await.expect("a close");
    });
}

// "a thread's token" > "is refused a start, a review and a post, which are the person's acts"
#[test]
fn is_refused_a_start_a_review_and_a_post_which_are_the_persons_acts() {
    drive(|| async {
        let daemons = fake_daemons();
        let w = with_project(&daemons, vec![]).await;
        let scope = as_thread("thr_1", "thr_1", "ws_x");
        let started =
            w.fx.rt
                .workspaces()
                .start(link(&format!("https://github.com/{}/issues/5", w.repo), None), Some(scope.clone()))
                .await
                .err()
                .expect("a refusal");
        assert!(started.message.contains(SpawnAct::Start.words()), "{}", started.message);
        let review =
            w.fx.rt
                .workspaces()
                .review(review_of(&format!("https://github.com/{}/pull/7", w.repo), None), Some(scope.clone()))
                .await
                .err()
                .expect("a refusal");
        assert!(review.message.contains(SpawnAct::Review.words()), "{}", review.message);
        let post = w.fx.rt.workspaces().review_post("ws_x", None, Some(scope)).await.err().expect("a refusal");
        assert!(post.message.contains(SpawnAct::ReviewPost.words()), "{}", post.message);
        assert!(daemons.ops(None).is_empty());
        w.fx.rt.close().await.expect("a close");
    });
}
