// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/conversations.test.ts at 65f6b89c7, case for case. Conversations a harness kept on this computer
//! outside wsp: the one list a project answers, read through each harness's own store on the project's computer, and a thread
//! opened on one of them, in the folder it ran in, under its own name, with its earlier messages ahead of the first prompt. The
//! stores are fakes with the adapters' own shape, except where the daemon's op is the point; the daemon is a fake; git is real.
//! Each test names the TS test above it. The TS `memoryStore()` is the sqlite store here.
mod common;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use common::adapter::{event, result};
use common::daemon::git;
use common::local::{git_copier, local_wiring_with, write_stub, LocalOpts};
use common::*;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use indexmap::IndexSet;
use serde_json::{json, Value};
use wsp_adapters::claude::conversations::{claude_conversations, ConversationDeps};
use wsp_adapters::codex::adapter::{create_codex_adapter, CodexAdapterDeps};
use wsp_adapters::port::{
    AdapterError, AgentLaunch, ConversationEarlier, ConversationLine, ConversationOrigin, ConversationRoad, ConversationStore, EarlierAsk,
    EarlierRead, ExecFailure, ExecOptions, ExecStream, ExecStreamFactory, HarnessAdapter, HarnessExec, HarnessSession, HarnessStartOptions,
    SessionRenameWrite, SharedFuture, StoreError, StoredConversation, TurnResult,
};
use wsp_core::clock::FakeClock;
use wsp_core::console::capture;
use wsp_core::types::api::{ConversationAsk, PlacedStart, StartOptions};
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_wire::types::conversations::{
    ResumeAsk, CONVERSATIONS_AGAIN_FIX, CONVERSATION_OPEN_KIND, EARLIER_MESSAGES, PREVIEW_MESSAGES, PREVIEW_TAIL_BYTES,
};
use wsp_words::conversations::{conversation_open_fix, conversations_unread_line, conversations_update_fix, earlier_line};
use wsp_words::daemon_contract::unknown_op_line;
use wsp_words::exit::{refusal_parts, HostError};
use wsp_words::project_path::claude_project_key;
use wsp_words::views::session::fold_threads;

const ID: &str = "7414323d-e71b-4957-8b56-eefdf6bfa350";
const OTHER: &str = "e615a6ab-a2f2-4b10-8fd6-0dc38bf99c28";
const KEPT_LIST_MS: i64 = wsp_threads::threads::conversations::KEPT_LIST_MS;

/// Folders of the test's own, removed when it ends.
#[derive(Default)]
struct Scratch(RefCell<Vec<tempfile::TempDir>>);

impl Scratch {
    fn dir(&self) -> PathBuf {
        let dir = tempfile::Builder::new().prefix("wsp-conversations-").tempdir().expect("a folder");
        let path = std::fs::canonicalize(dir.path()).expect("a real path");
        self.0.borrow_mut().push(dir);
        path
    }
    /// A repo on main with one commit, and a worktree of it on a branch of its own beside it.
    fn repo_with_tree(&self) -> (PathBuf, PathBuf) {
        let folder = self.dir().join("lab");
        std::fs::create_dir(&folder).expect("a folder");
        git(&folder, &["init", "-q", "-b", "main"]);
        git(&folder, &["commit", "-q", "--allow-empty", "-m", "first"]);
        let tree = PathBuf::from(format!("{}-wt", folder.display()));
        git(&folder, &["worktree", "add", "-q", "-b", "feat/x", &tree.display().to_string()]);
        (folder, tree)
    }
}

/// What a harness was handed and asked.
#[derive(Default)]
struct Seen {
    starts: RefCell<Vec<Started>>,
    seeds: RefCell<Vec<String>>,
    renames: RefCell<Vec<String>>,
    n: Cell<u32>,
}

#[derive(Clone, Debug, Default)]
struct Started {
    resume: Option<String>,
    copy: bool,
    cwd: Option<String>,
    title: Option<String>,
}

/// A harness that answers each turn at once, a resume under its own session and a copy under a new one, and records what it was
/// started with, the seed it was handed where it reads one, and every name written into its store.
fn harness(seen: &Rc<Seen>, store: Option<Arc<dyn ConversationStore>>) -> HarnessAdapterFactory {
    let seen = seen.clone();
    Arc::new(move |_ctx| Ok(Arc::new(Recording { seen: seen.clone(), store: store.clone() }) as Arc<dyn HarnessAdapter>))
}

struct Recording {
    seen: Rc<Seen>,
    store: Option<Arc<dyn ConversationStore>>,
}

impl HarnessAdapter for Recording {
    fn steers(&self) -> bool {
        false
    }
    fn attachments(&self) -> Option<wsp_adapters::port::AttachmentRoad> {
        Some(wsp_adapters::port::AttachmentRoad::Inline)
    }
    fn conversations(&self) -> Option<Arc<dyn ConversationStore>> {
        self.store.clone()
    }
    fn renames(&self) -> bool {
        true
    }
    fn rename_session(
        &self,
        id: String,
        title: String,
        _exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<SessionRenameWrite, ExecFailure>>> {
        self.seen.renames.borrow_mut().push(format!("{id}:{title}"));
        Some(async { Ok(SessionRenameWrite::Written) }.boxed_local())
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let seen = &self.seen;
        seen.n.set(seen.n.get() + 1);
        let n = seen.n.get();
        seen.starts.borrow_mut().push(Started { resume: o.resume.clone(), copy: o.copy, cwd: o.cwd.clone(), title: o.title.clone() });
        let session =
            if o.copy || o.resume.is_none() { format!("55555555-5555-4555-8555-{n:012}") } else { o.resume.clone().unwrap_or_default() };
        let on_event = o.on_event.clone().expect("an event sink");
        let seed = o.seed.clone();
        let kept = seen.clone();
        let (tx, rx) = futures::channel::oneshot::channel::<TurnResult>();
        let id = session.clone();
        tokio::task::spawn_local(async move {
            tokio::task::yield_now().await;
            if let Some(seed) = seed {
                let words = seed().await.unwrap_or_default();
                kept.seeds.borrow_mut().push(words);
            }
            let done = result(json!({ "status": "completed", "text": "ok" }));
            on_event(event(json!({ "type": "session.start", "sessionId": id })));
            on_event(event(json!({ "type": "turn.done", "sessionId": id, "result": serde_json::to_value(&done).expect("a result") })));
            on_event(event(json!({ "type": "session.end", "sessionId": id, "exitCode": 0, "sawResult": true })));
            let _ = tx.send(done);
        });
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn's end") }.boxed_local();
        Ok(Arc::new(Quick { local_id: session, finished: finished.shared() }))
    }
}

struct Quick {
    local_id: String,
    finished: SharedFuture<TurnResult>,
}

impl HarnessSession for Quick {
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

/// What each store read was given.
#[derive(Clone, Debug, PartialEq)]
struct AskedOf {
    cwds: Vec<String>,
    probe: Option<bool>,
    last: Option<u32>,
    tail: Option<u64>,
    count_off: bool,
}

#[derive(Clone)]
enum Live {
    /// No `live` member at all.
    Absent,
    Ids(Rc<RefCell<Vec<String>>>),
}

type EarlierFn = Rc<dyn Fn(bool) -> EarlierRead>;

/// A store with the adapters' shape over rows, open ids and one conversation's earlier messages, recording the folders each read
/// was given.
struct FakeStore {
    rows: Rc<RefCell<Vec<StoredConversation>>>,
    live: Live,
    earlier: EarlierFn,
    lets_go: Option<String>,
    asked: Rc<RefCell<Vec<AskedOf>>>,
    list_fails: Option<String>,
}

struct Spec {
    rows: Rc<RefCell<Vec<StoredConversation>>>,
    live: Live,
    earlier: EarlierFn,
    lets_go: Option<String>,
    list_fails: Option<String>,
}

fn spec() -> Spec {
    Spec { rows: Rc::default(), live: Live::Absent, earlier: Rc::new(|_| EarlierRead::Gone), lets_go: None, list_fails: None }
}

fn fake_store(o: Spec) -> (Arc<dyn ConversationStore>, Rc<RefCell<Vec<AskedOf>>>) {
    let asked: Rc<RefCell<Vec<AskedOf>>> = Rc::default();
    let store =
        FakeStore { rows: o.rows, live: o.live, earlier: o.earlier, lets_go: o.lets_go, asked: asked.clone(), list_fails: o.list_fails };
    (Arc::new(store), asked)
}

impl ConversationStore for FakeStore {
    fn list(&self, cwds: Vec<String>, _road: ConversationRoad) -> LocalBoxFuture<'static, Result<Vec<StoredConversation>, StoreError>> {
        self.asked.borrow_mut().push(AskedOf { cwds, probe: None, last: None, tail: None, count_off: false });
        let rows = self.rows.borrow().clone();
        let fails = self.list_fails.clone();
        async move {
            match fails {
                Some(message) => Err(StoreError { message, lasting: false }),
                None => Ok(rows),
            }
        }
        .boxed_local()
    }
    fn live(&self, _road: ConversationRoad) -> Option<LocalBoxFuture<'static, Option<IndexSet<String>>>> {
        match &self.live {
            Live::Absent => None,
            Live::Ids(ids) => {
                let ids: IndexSet<String> = ids.borrow().iter().cloned().collect();
                Some(async move { Some(ids) }.boxed_local())
            }
        }
    }
    fn earlier(&self, _id: String, ask: EarlierAsk, _road: ConversationRoad) -> LocalBoxFuture<'static, Result<EarlierRead, StoreError>> {
        self.asked.borrow_mut().push(AskedOf {
            cwds: ask.cwds.clone(),
            probe: Some(ask.probe),
            last: Some(ask.last),
            tail: ask.tail,
            count_off: !ask.count,
        });
        let read = (self.earlier)(ask.probe);
        async move { Ok(read) }.boxed_local()
    }
    fn lets_go(&self) -> Option<String> {
        self.lets_go.clone()
    }
}

/// `{ ...store, live: async () => null }`: a real store whose open ones nothing can say.
struct NoLive(Arc<dyn ConversationStore>);

impl ConversationStore for NoLive {
    fn list(&self, cwds: Vec<String>, road: ConversationRoad) -> LocalBoxFuture<'static, Result<Vec<StoredConversation>, StoreError>> {
        self.0.list(cwds, road)
    }
    fn live(&self, _road: ConversationRoad) -> Option<LocalBoxFuture<'static, Option<IndexSet<String>>>> {
        Some(async { None }.boxed_local())
    }
    fn stamp(&self, cwds: Vec<String>, road: ConversationRoad) -> Option<LocalBoxFuture<'static, Result<String, StoreError>>> {
        self.0.stamp(cwds, road)
    }
    fn earlier(&self, id: String, ask: EarlierAsk, road: ConversationRoad) -> LocalBoxFuture<'static, Result<EarlierRead, StoreError>> {
        self.0.earlier(id, ask, road)
    }
    fn lets_go(&self) -> Option<String> {
        self.0.lets_go()
    }
}

/// A store whose list is the test's own function and whose earlier answers gone.
struct Counting {
    inner: Option<Arc<dyn ConversationStore>>,
    list: Rc<dyn Fn(Vec<String>, ConversationRoad) -> LocalBoxFuture<'static, Result<Vec<StoredConversation>, StoreError>>>,
}

impl ConversationStore for Counting {
    fn list(&self, cwds: Vec<String>, road: ConversationRoad) -> LocalBoxFuture<'static, Result<Vec<StoredConversation>, StoreError>> {
        (self.list)(cwds, road)
    }
    fn live(&self, road: ConversationRoad) -> Option<LocalBoxFuture<'static, Option<IndexSet<String>>>> {
        self.inner.as_ref().and_then(|s| s.live(road))
    }
    fn earlier(&self, id: String, ask: EarlierAsk, road: ConversationRoad) -> LocalBoxFuture<'static, Result<EarlierRead, StoreError>> {
        match &self.inner {
            Some(inner) => inner.earlier(id, ask, road),
            None => async { Ok(EarlierRead::Gone) }.boxed_local(),
        }
    }
}

fn row(id: &str, cwd: &Path, last_at: f64, branch: Option<&str>, bytes: Option<f64>, origin: ConversationOrigin) -> StoredConversation {
    StoredConversation {
        id: id.into(),
        title: format!("t-{}", &id[..4]),
        first_prompt: None,
        branch: branch.map(str::to_string),
        cwd: cwd.display().to_string(),
        last_at,
        bytes,
        origin,
    }
}

fn terminal(id: &str, cwd: &Path, last_at: f64) -> StoredConversation {
    row(id, cwd, last_at, None, None, ConversationOrigin::Terminal)
}

struct Stores {
    claude: Option<Arc<dyn ConversationStore>>,
    codex: Option<Arc<dyn ConversationStore>>,
}

type Answers = Vec<(&'static str, Box<dyn Fn(&wsp_core::daemon_channel::Frame) -> Value>)>;

struct Here {
    fx: Fx,
    seen: Rc<Seen>,
    _scratch: Rc<Scratch>,
    _state: State,
}

fn here(scratch: &Rc<Scratch>, stores: Stores, answers: Answers) -> Here {
    here_with(scratch, stores, answers, None, None)
}

fn here_with(
    scratch: &Rc<Scratch>,
    stores: Stores,
    answers: Answers,
    clock: Option<&FakeClock>,
    vars: Option<indexmap::IndexMap<String, String>>,
) -> Here {
    let root = scratch.dir();
    let seen: Rc<Seen> = Rc::default();
    let daemon = RecDaemon::new(|f| {
        if f["op"] == "git.checkpoint" {
            json!({ "id": 1, "ok": true, "ref": "refs/wsp/checkpoints/x", "commit": "c", "changed": false })
        } else {
            json!({ "id": 1, "ok": false, "error": format!("unknown op: {}", f["op"].as_str().unwrap_or("undefined")) })
        }
    });
    for (op, answer) in answers {
        daemon.answer(op, move |f| answer(f));
    }
    let (copier, _log) = git_copier();
    let local = local_wiring_with(&root, copier, LocalOpts { backend_vars: vars.clone(), vars, ..Default::default() });
    let state = sqlite_state();
    let open = daemon.open();
    let backend = StubBackend::new(None);
    let state_path = root.join("state");
    std::fs::create_dir_all(&state_path).expect("a folder");
    let state_file = state_path.join("state.json").display().to_string();
    let mut wiring = runtime(&backend)
        .store(&state.store)
        .adapter("claude", harness(&seen, stores.claude))
        .adapter("codex", harness(&seen, stores.codex))
        .with(move |o| {
            o.local = Some(local);
            o.state_path = Some(state_file);
            o.daemon_channel = Some(open);
        });
    if let Some(clock) = clock {
        wiring = wiring.clock(clock);
    }
    Here { fx: wiring.build(), seen, _scratch: scratch.clone(), _state: state }
}

/// The serve road of a start on a conversation: placed reads it on the project's computer and names the record of the folder it
/// ran in, which the start then runs on.
async fn start_on(
    h: &Here,
    project: &str,
    resume: ResumeAsk,
    prompt: &str,
    harness: Option<&str>,
) -> Result<(Value, wsp_core::types::api::OutsideOpening), HostError> {
    let ask = PlacedStart { project: Some(project.into()), harness: harness.map(str::to_string), ..Default::default() };
    let at = h.fx.rt.conversations().placed(ask, resume, None).await?;
    let opts = StartOptions { prompt: prompt.into(), cwd: at.cwd.clone(), outside: Some(at.outside.clone()), ..Default::default() };
    let handle = h.fx.rt.sessions().start(&at.workspace_id, opts, None).await?;
    handle.finished().await;
    let workspace = h.fx.rt.workspaces().list(None).await?.iter().map(j).find(|w| w["id"] == at.workspace_id.as_str()).expect("the record");
    Ok((workspace, at.outside))
}

fn resume(id: &str) -> ResumeAsk {
    ResumeAsk { id: id.into(), copy: None }
}

async fn add(h: &Here, folder: &Path) -> String {
    let ask = wsp_core::types::api::ProjectAddAsk { source: folder.display().to_string(), ..Default::default() };
    h.fx.rt.projects().add(ask, None).await.expect("a project").project.id
}

async fn list(h: &Here, project: &str, agent: Option<&str>) -> Value {
    j(&h.fx.rt.conversations().list(Some(project.into()), agent.map(str::to_string), None).await.expect("a list"))
}

fn ids(rows: &Value) -> Vec<(Value, Value)> {
    rows["rows"].as_array().expect("rows").iter().map(|r| (r["agent"].clone(), r["id"].clone())).collect()
}

fn start(prompt: &str, harness: Option<&str>) -> StartOptions {
    StartOptions { prompt: prompt.into(), harness: harness.map(str::to_string), ..Default::default() }
}

async fn run_in_folder(h: &Here, project: &str) -> String {
    let at =
        h.fx.rt
            .workspaces()
            .folder_for(wsp_core::types::api::FolderForAsk { project: Some(project.into()), ..Default::default() }, None)
            .await
            .expect("a folder");
    at.workspace.id
}

// ===== a project's conversations =====

// "a project's conversations" > "are every agent's in the folder and its worktrees, in one list newest first, a script's left out"
#[test]
fn are_every_agents_in_the_folder_and_its_worktrees_in_one_list_newest_first_a_scripts_left_out() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, tree) = scratch.repo_with_tree();
        let (claude, claude_asked) = fake_store(Spec {
            rows: Rc::new(RefCell::new(vec![
                row(ID, &folder, 100.0, Some("main"), Some(2048.0), ConversationOrigin::Terminal),
                row(OTHER, &tree, 300.0, None, None, ConversationOrigin::Script),
            ])),
            live: Live::Ids(Rc::default()),
            ..spec()
        });
        let codex_id = "01a12813-cd12-7a12-9b13-e76892906ff0";
        let mut codex_row = terminal(codex_id, &tree, 200.0);
        codex_row.title = "lab terminal thread".into();
        let (codex, codex_asked) =
            fake_store(Spec { rows: Rc::new(RefCell::new(vec![codex_row])), lets_go: Some("It lets go a minute later.".into()), ..spec() });
        let h = here(&scratch, Stores { claude: Some(claude), codex: Some(codex) }, vec![]);
        let project = add(&h, &folder).await;
        let answer = list(&h, &project, None).await;
        let want = [folder.display().to_string(), tree.display().to_string()];
        assert_eq!(claude_asked.borrow()[0].cwds, want);
        assert_eq!(codex_asked.borrow()[0].cwds, want);
        assert_eq!(answer["held"], json!([]));
        assert_eq!(
            answer["rows"],
            json!([
                { "agent": "codex", "id": codex_id, "title": "lab terminal thread", "cwd": tree.display().to_string(), "lastAt": 200, "origin": "terminal", "live": false, "letsGo": "It lets go a minute later." },
                { "agent": "claude", "id": ID, "title": "t-7414", "cwd": folder.display().to_string(), "lastAt": 100, "origin": "terminal", "live": false, "branch": "main", "bytes": 2048 },
            ])
        );
        let only = list(&h, &project, Some("claude")).await;
        assert_eq!(only["rows"].as_array().expect("rows").iter().map(|r| r["agent"].clone()).collect::<Vec<_>>(), [json!("claude")]);
        h.fx.rt.close().await.expect("a close");
    });
}

// "a project's conversations" > "mark a wsp thread's own session as wsp's, with its thread, never open elsewhere, and another app's open one as live"
#[test]
fn mark_a_wsp_threads_own_session_as_wsps_with_its_thread_never_open_elsewhere_and_another_apps_open_one_as_live() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let rows: Rc<RefCell<Vec<StoredConversation>>> = Rc::default();
        let live: Rc<RefCell<Vec<String>>> = Rc::default();
        let (claude, _) = fake_store(Spec { rows: rows.clone(), live: Live::Ids(live.clone()), ..spec() });
        let h = here(&scratch, Stores { claude: Some(claude), codex: None }, vec![]);
        let project = add(&h, &folder).await;
        let ws = run_in_folder(&h, &project).await;
        h.fx.rt.sessions().start(&ws, start("hello", Some("claude")), None).await.expect("a start").finished().await;
        let mine = j(&h.fx.rt.sessions().list(Some(ws.clone()), None).await.expect("a list")[0]);
        let session = mine["claudeSessionId"].as_str().expect("a session").to_string();
        rows.borrow_mut().push(row(&session, &folder, 50.0, None, None, ConversationOrigin::Script));
        rows.borrow_mut().push(terminal(OTHER, &folder, 40.0));
        live.borrow_mut().extend([session.clone(), OTHER.to_string()]);
        let answer = list(&h, &project, None).await;
        let got: Vec<Vec<Value>> = answer["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .map(|r| vec![r["id"].clone(), r["origin"].clone(), r["live"].clone(), r.get("thread").cloned().unwrap_or(Value::Null)])
            .collect();
        assert_eq!(
            got,
            [
                vec![json!(session), json!("wsp"), json!(false), mine["threadId"].clone()],
                vec![json!(OTHER), json!("terminal"), json!(true), Value::Null]
            ]
        );
        assert_eq!(h.seen.starts.borrow().len(), 1);
        h.fx.rt.close().await.expect("a close");
    });
}

fn claude_real(config_dir: &str) -> Arc<dyn ConversationStore> {
    claude_conversations(ConversationDeps { config_dir: config_dir.into(), project_dir_name: None, launch: None, base_env: None })
}

// "a project's conversations" > "from a daemon without the op are the other agents' rows and one line in two halves naming the update"
#[test]
fn from_a_daemon_without_the_op_are_the_other_agents_rows_and_one_line_in_two_halves_naming_the_update() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let claude: Arc<dyn ConversationStore> = Arc::new(NoLive(claude_real("/home/dev/.claude")));
        let (codex, _) = fake_store(Spec {
            rows: Rc::new(RefCell::new(vec![terminal("01a12813-cd12-7a12-9b13-e76892906ff0", &folder, 200.0)])),
            ..spec()
        });
        let h = here(&scratch, Stores { claude: Some(claude), codex: Some(codex) }, vec![]);
        let project = add(&h, &folder).await;
        let answer = list(&h, &project, None).await;
        assert_eq!(answer["rows"].as_array().expect("rows").iter().map(|r| r["agent"].clone()).collect::<Vec<_>>(), [json!("codex")]);
        let held = answer["held"].as_array().expect("held");
        assert_eq!(held.len(), 1);
        assert_eq!(held[0]["agent"], "claude");
        assert_eq!(held[0]["said"], conversations_unread_line("Claude Code", "this computer"));
        assert_eq!(held[0]["fix"], conversations_update_fix("this computer"));
        h.fx.rt.close().await.expect("a close");
    });
}

// "a project's conversations" > "say in one line which agent's store could not be read and why, the others' rows standing"
#[test]
fn say_in_one_line_which_agents_store_could_not_be_read_and_why_the_others_rows_standing() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let (codex, _) = fake_store(Spec { list_fails: Some("codex: command not found".into()), ..spec() });
        let (claude, _) =
            fake_store(Spec { rows: Rc::new(RefCell::new(vec![terminal(ID, &folder, 100.0)])), live: Live::Ids(Rc::default()), ..spec() });
        let h = here(&scratch, Stores { claude: Some(claude), codex: Some(codex) }, vec![]);
        let project = add(&h, &folder).await;
        let warned = capture();
        let answer = list(&h, &project, None).await;
        let rows = answer["rows"].as_array().expect("rows");
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0]["agent"].clone(), rows[0]["id"].clone()), (json!("claude"), json!(ID)));
        assert_eq!(
            answer["held"],
            json!([{ "agent": "codex", "said": "Codex's conversations could not be read: codex: command not found", "fix": CONVERSATIONS_AGAIN_FIX, "why": "codex: command not found" }])
        );
        assert!(warned.lines().join("\n").contains("codex: command not found"));
        h.fx.rt.close().await.expect("a close");
    });
}

// "a project's conversations" > "come off the daemon's transcripts.list where it has the op, under the store the turns read"
#[test]
fn come_off_the_daemons_transcripts_list_where_it_has_the_op_under_the_store_the_turns_read() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, tree) = scratch.repo_with_tree();
        let frames: Rc<RefCell<Vec<Value>>> = Rc::default();
        let kept = frames.clone();
        let at = folder.display().to_string();
        let answer: Answers = vec![(
            "transcripts.list",
            Box::new(move |f| {
                kept.borrow_mut().push(Value::Object(f.clone()));
                json!({ "id": 1, "ok": true, "rows": [{ "id": ID, "cwd": at, "entrypoint": "cli", "title": "lab codewords", "lastAt": 9, "bytes": 10 }] })
            }),
        )];
        let claude: Arc<dyn ConversationStore> = Arc::new(NoLive(claude_real("/home/dev/.claude")));
        let h = here(&scratch, Stores { claude: Some(claude), codex: None }, answer);
        let project = add(&h, &folder).await;
        let rows = list(&h, &project, None).await;
        assert_eq!(
            rows["rows"],
            json!([{ "agent": "claude", "id": ID, "title": "lab codewords", "cwd": folder.display().to_string(), "lastAt": 9, "bytes": 10, "origin": "terminal", "live": false }])
        );
        assert_matches(
            &frames.borrow()[0],
            json!({ "op": "transcripts.list", "root": "/home/dev/.claude/projects", "cwds": [folder.display().to_string(), tree.display().to_string()] }),
        );
        h.fx.rt.close().await.expect("a close");
    });
}

// "a project's conversations" > "of 74 folders are all listed and opened, in frames of at most 64 the daemon takes, asking only for folders Claude Code keeps"
#[test]
fn of_74_folders_are_all_listed_and_opened_in_frames_of_at_most_64_the_daemon_takes_asking_only_for_folders_claude_code_keeps() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, tree) = scratch.repo_with_tree();
        let trees: Vec<PathBuf> = (0..72).map(|i| PathBuf::from(format!("{}-t{i}", folder.display()))).collect();
        for (i, t) in trees.iter().enumerate() {
            git(&folder, &["worktree", "add", "-q", "-b", &format!("t{i}"), &t.display().to_string()]);
        }
        let home = scratch.dir();
        let root = home.join(".claude").join("projects");
        let held = [(ID, folder.clone()), (OTHER, trees[70].clone())];
        for (id, cwd) in &held {
            let dir = root.join(claude_project_key(&cwd.display().to_string()));
            std::fs::create_dir_all(&dir).expect("a folder");
            std::fs::write(dir.join(format!("{id}.jsonl")), format!("{}\n", json!({ "cwd": cwd.display().to_string() }))).expect("a file");
        }
        let frames: Rc<RefCell<Vec<Value>>> = Rc::default();
        // The daemon's own frame check, which refuses the whole frame before any folder is read.
        let capped = {
            let frames = frames.clone();
            move |f: &wsp_core::daemon_channel::Frame| -> Option<String> {
                frames.borrow_mut().push(Value::Object(f.clone()));
                for key in ["dirs", "cwds"] {
                    if let Some(a) = f.get(key).and_then(Value::as_array) {
                        if a.len() > 64 {
                            return Some(format!("invalid frame: at most 64 entries, got {}", a.len()));
                        }
                    }
                }
                None
            }
        };
        let root_for_kept = root.clone();
        let kept = move |f: &wsp_core::daemon_channel::Frame| -> Vec<(String, String)> {
            let mut out = vec![];
            for dir in f["dirs"].as_array().expect("dirs") {
                let at = root_for_kept.join(dir.as_str().unwrap_or_default());
                let names = std::fs::read_dir(&at)
                    .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect::<Vec<_>>())
                    .unwrap_or_default();
                for name in names {
                    let text = std::fs::read_to_string(at.join(&name)).unwrap_or_default();
                    let cwd =
                        serde_json::from_str::<Value>(&text).map(|v| v["cwd"].as_str().unwrap_or_default().to_string()).unwrap_or_default();
                    out.push((name[..name.len() - 6].to_string(), cwd));
                }
            }
            out
        };
        let (capped_list, kept_list) = (capped.clone(), kept.clone());
        let answers: Answers = vec![
            (
                "transcripts.list",
                Box::new(move |f| {
                    if let Some(error) = capped_list(f) {
                        return json!({ "id": 1, "ok": false, "error": error });
                    }
                    let cwds: Vec<&str> = f["cwds"].as_array().expect("cwds").iter().filter_map(Value::as_str).collect();
                    let rows: Vec<Value> = kept_list(f)
                        .into_iter()
                        .filter(|(_, cwd)| cwds.contains(&cwd.as_str()))
                        .map(|(id, cwd)| json!({ "id": id, "cwd": cwd, "entrypoint": "cli", "lastAt": if id == ID { 2 } else { 1 }, "bytes": 1 }))
                        .collect();
                    json!({ "id": 1, "ok": true, "rows": rows })
                }),
            ),
            (
                "transcripts.read",
                Box::new(move |f| {
                    if let Some(error) = capped(f) {
                        return json!({ "id": 1, "ok": false, "error": error });
                    }
                    match kept(f).into_iter().find(|(id, _)| f["session"] == id.as_str()) {
                        Some((_, cwd)) => json!({ "id": 1, "ok": true, "found": true, "cwd": cwd, "messages": [], "earlier": 0 }),
                        None => json!({ "id": 1, "ok": true, "found": false, "messages": [], "earlier": 0 }),
                    }
                }),
            ),
        ];
        let claude: Arc<dyn ConversationStore> = Arc::new(NoLive(claude_real(&home.join(".claude").display().to_string())));
        let h = here(&scratch, Stores { claude: Some(claude), codex: None }, answers);
        let project = add(&h, &folder).await;
        let answer = list(&h, &project, None).await;
        assert_eq!(answer["held"], json!([]));
        let got: Vec<(Value, Value)> =
            answer["rows"].as_array().expect("rows").iter().map(|r| (r["id"].clone(), r["cwd"].clone())).collect();
        assert_eq!(got, [(json!(ID), json!(folder.display().to_string())), (json!(OTHER), json!(trees[70].display().to_string()))]);
        let listed: Vec<Value> = frames.borrow().iter().filter(|f| f["op"] == "transcripts.list").cloned().collect();
        assert!(listed
            .iter()
            .all(|f| f["dirs"].as_array().is_some_and(|a| a.len() <= 64) && f["cwds"].as_array().is_some_and(|a| a.len() <= 64)));
        let dirs: std::collections::HashSet<String> = listed
            .iter()
            .flat_map(|f| f["dirs"].as_array().cloned().unwrap_or_default())
            .filter_map(|d| d.as_str().map(str::to_string))
            .collect();
        let want: std::collections::HashSet<String> = held.iter().map(|(_, c)| claude_project_key(&c.display().to_string())).collect();
        assert_eq!(dirs, want);
        let cwds: std::collections::HashSet<String> = listed
            .iter()
            .flat_map(|f| f["cwds"].as_array().cloned().unwrap_or_default())
            .filter_map(|d| d.as_str().map(str::to_string))
            .collect();
        assert_eq!(cwds.len(), 74);
        let _ = tree;
        let (_, outside) = start_on(&h, &project, resume(OTHER), "go on", Some("claude")).await.expect("a start");
        assert_eq!(outside.cwd.as_deref(), Some(trees[70].display().to_string().as_str()));
        let reads: Vec<Value> = frames.borrow().iter().filter(|f| f["op"] == "transcripts.read").map(|f| f["dirs"].clone()).collect();
        assert_eq!(reads, [json!([claude_project_key(&trees[70].display().to_string())])]);
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== a project's kept conversation list =====

/// Claude Code's store and Codex's as the adapters read them, over folders in a scratch home, with `claude` and `codex` stubs on PATH
/// that note each start and list the conversations each store's folder holds, and the ones `open` names as open in another app.
struct Real {
    claude: Arc<dyn ConversationStore>,
    codex: Arc<dyn ConversationStore>,
    claude_dir: PathBuf,
    day: PathBuf,
    opened: PathBuf,
    starts_file: PathBuf,
    vars: indexmap::IndexMap<String, String>,
    folder: String,
}

struct NoTurns;

impl ExecStreamFactory for NoTurns {
    fn launch(&self, _command: &str, _options: ExecOptions) -> Arc<dyn ExecStream> {
        panic!("no turn runs here")
    }
}

fn real_stores(scratch: &Scratch, folder: &Path) -> Real {
    let home = scratch.dir();
    let bin = home.join("bin");
    let starts = home.join("starts");
    let claude_dir = home.join(".claude").join("projects").join(claude_project_key(&folder.display().to_string()));
    let day = home.join(".codex").join("sessions").join("2026").join("10").join("11");
    for dir in [&bin, &claude_dir, &day] {
        std::fs::create_dir_all(dir).expect("a folder");
    }
    std::fs::write(&starts, "").expect("a file");
    let opened = home.join("open");
    write_stub(
        &bin.join("claude"),
        &format!("#!/bin/sh\necho \"claude $*\" >> '{}'\ncat '{}' 2>/dev/null || echo '[]'\n", starts.display(), opened.display()),
    );
    let codex_script = [
        "#!/bin/sh".to_string(),
        format!("echo \"codex $*\" >> '{}'", starts.display()),
        "[ \"$1\" = app-server ] || exit 0".to_string(),
        "while IFS= read -r line; do".to_string(),
        "  case $line in".to_string(),
        r#"    *'"id":1,'*) echo '{"id":1,"result":{}}' ;;"#.to_string(),
        format!(
            r#"    *'"id":2,'*) rows=""; for f in '{day}'/*.jsonl; do [ -e "$f" ] || continue; rows="$rows${{rows:+,}}{{\"id\":\"$(basename "$f" .jsonl)\",\"cwd\":\"{folder}\",\"recencyAt\":1,\"source\":\"cli\"}}"; done; printf '{{"id":2,"result":{{"data":[%s]}}}}\n' "$rows" ;;"#,
            day = day.display(),
            folder = folder.display()
        ),
        "  esac".to_string(),
        "done".to_string(),
        String::new(),
    ]
    .join("\n");
    write_stub(&bin.join("codex"), &codex_script);
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()));
    let vars = indexmap::IndexMap::from([("PATH".to_string(), path)]);
    let codex_adapter =
        create_codex_adapter(CodexAdapterDeps::new(Arc::new(NoTurns), home.join(".codex").display().to_string(), "codex login"))
            .expect("a codex adapter");
    Real {
        claude: claude_real(&home.join(".claude").display().to_string()),
        codex: codex_adapter.conversations(),
        claude_dir,
        day,
        opened,
        starts_file: starts,
        vars,
        folder: folder.display().to_string(),
    }
}

impl Real {
    fn open(&self, ids: &[&str]) {
        if ids.is_empty() {
            let _ = std::fs::remove_file(&self.opened);
        } else {
            let doc: Vec<Value> = ids.iter().map(|id| json!({ "sessionId": id })).collect();
            std::fs::write(&self.opened, Value::Array(doc).to_string()).expect("a file");
        }
    }
    fn starts(&self) -> (usize, usize) {
        let said = std::fs::read_to_string(&self.starts_file).unwrap_or_default();
        (said.matches("claude agents --json").count(), said.matches("codex app-server").count())
    }
}

// "a project's kept conversation list" > "answers ten visits within a minute with one claude agents --json and one Codex app server"
#[test]
fn answers_ten_visits_within_a_minute_with_one_claude_agents_json_and_one_codex_app_server() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let real = real_stores(&scratch, &folder);
        std::fs::write(real.claude_dir.join(format!("{ID}.jsonl")), "{}\n").expect("a file");
        std::fs::write(real.day.join(format!("rollout-2026-10-11T09-00-00-{OTHER}.jsonl")), "{}\n").expect("a file");
        let (claude, codex, vars) = (real.claude.clone(), real.codex.clone(), real.vars.clone());
        let h = here_with(&scratch, Stores { claude: Some(claude), codex: Some(codex) }, real.daemon(), None, Some(vars));
        let project = add(&h, &folder).await;
        let mut visits: Vec<Vec<Value>> = vec![];
        for _ in 0..5 {
            let a = list(&h, &project, None).await;
            visits.push(a["rows"].as_array().expect("rows").iter().map(|r| r["id"].clone()).collect());
        }
        let together = futures::future::join_all((0..5).map(|_| list(&h, &project, None))).await;
        for a in together {
            visits.push(a["rows"].as_array().expect("rows").iter().map(|r| r["id"].clone()).collect());
        }
        assert_eq!(visits.len(), 10);
        let mut want = vec![json!(format!("rollout-2026-10-11T09-00-00-{OTHER}")), json!(ID)];
        want.sort_by_key(Value::to_string);
        for mut got in visits {
            got.sort_by_key(Value::to_string);
            assert_eq!(got, want);
        }
        assert_eq!(real.starts(), (1, 1));
        h.fx.rt.close().await.expect("a close");
    });
}

impl Real {
    /// The daemon's `transcripts.list`: the sessions the Claude folder holds, each in the project's folder.
    fn daemon(&self) -> Answers {
        let (dir, folder) = (self.claude_dir.clone(), self.folder.clone());
        vec![(
            "transcripts.list",
            Box::new(move |_| {
                let rows: Vec<Value> = std::fs::read_dir(&dir)
                    .map(|d| {
                        d.flatten()
                            .filter_map(|e| e.file_name().to_str().map(str::to_string))
                            .filter(|f| f.ends_with(".jsonl"))
                            .map(|f| json!({ "id": &f[..f.len() - 6], "cwd": folder, "entrypoint": "cli", "lastAt": 2, "bytes": 1 }))
                            .collect()
                    })
                    .unwrap_or_default();
                json!({ "id": 1, "ok": true, "rows": rows })
            }),
        )]
    }
}

// "a project's kept conversation list" > "shows a conversation started in a terminal on the next visit after its store moved"
#[test]
fn shows_a_conversation_started_in_a_terminal_on_the_next_visit_after_its_store_moved() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let real = real_stores(&scratch, &folder);
        let (claude, codex, vars) = (real.claude.clone(), real.codex.clone(), real.vars.clone());
        let h = here_with(&scratch, Stores { claude: Some(claude), codex: Some(codex) }, real.daemon(), None, Some(vars));
        let project = add(&h, &folder).await;
        assert_eq!(list(&h, &project, None).await["rows"], json!([]));
        std::fs::write(real.claude_dir.join(format!("{ID}.jsonl")), "{}\n").expect("a file");
        assert_eq!(ids(&list(&h, &project, None).await), [(json!("claude"), json!(ID))]);
        std::fs::write(real.day.join(format!("rollout-{OTHER}.jsonl")), "{}\n").expect("a file");
        let mut agents: Vec<Value> =
            list(&h, &project, None).await["rows"].as_array().expect("rows").iter().map(|r| r["agent"].clone()).collect();
        agents.sort_by_key(Value::to_string);
        assert_eq!(agents, [json!("claude"), json!("codex")]);
        list(&h, &project, None).await;
        assert_eq!(real.starts(), (2, 2));
        h.fx.rt.close().await.expect("a close");
    });
}

// "a project's kept conversation list" > "asks the open ones again while a kept row is open elsewhere, so closing that app clears its mark"
#[test]
fn asks_the_open_ones_again_while_a_kept_row_is_open_elsewhere_so_closing_that_app_clears_its_mark() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let real = real_stores(&scratch, &folder);
        std::fs::write(real.claude_dir.join(format!("{ID}.jsonl")), "{}\n").expect("a file");
        real.open(&[ID]);
        let (claude, vars) = (real.claude.clone(), real.vars.clone());
        let h = here_with(&scratch, Stores { claude: Some(claude), codex: None }, real.daemon(), None, Some(vars));
        let project = add(&h, &folder).await;
        let marks = || async {
            list(&h, &project, None).await["rows"].as_array().expect("rows").iter().map(|r| r["live"].clone()).collect::<Vec<_>>()
        };
        assert_eq!(marks().await, [json!(true)]);
        real.open(&[]);
        assert_eq!(marks().await, [json!(false)]);
        assert_eq!(marks().await, [json!(false)]);
        assert_eq!(real.starts().0, 2);
        h.fx.rt.close().await.expect("a close");
    });
}

// "a project's kept conversation list" > "reads again on the next visit after a failure that passes"
#[test]
fn reads_again_on_the_next_visit_after_a_failure_that_passes() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let calls: Rc<Cell<u32>> = Rc::default();
        let (counted, at) = (calls.clone(), folder.clone());
        let claude: Arc<dyn ConversationStore> = Arc::new(Counting {
            inner: None,
            list: Rc::new(move |_, _| {
                counted.set(counted.get() + 1);
                let (n, at) = (counted.get(), at.clone());
                async move {
                    if n == 1 {
                        return Err(StoreError { message: "the daemon connection closed".into(), lasting: false });
                    }
                    Ok(vec![terminal(ID, &at, 100.0)])
                }
                .boxed_local()
            }),
        });
        let h = here(&scratch, Stores { claude: Some(claude), codex: None }, vec![]);
        let project = add(&h, &folder).await;
        let _warned = capture();
        let first = list(&h, &project, None).await;
        assert_eq!(first["rows"], json!([]));
        assert_matches(&first["held"], json!([{ "agent": "claude", "why": "the daemon connection closed" }]));
        let second = list(&h, &project, None).await;
        assert_matches(&second["rows"], json!([{ "id": ID }]));
        assert_eq!(second["held"], json!([]));
        assert_eq!(calls.get(), 2);
        h.fx.rt.close().await.expect("a close");
    });
}

// "a project's kept conversation list" > "keeps a failure that lasts the minute: an agent not installed, a daemon too old for the read"
#[test]
fn keeps_a_failure_that_lasts_the_minute_an_agent_not_installed_a_daemon_too_old_for_the_read() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let home = scratch.dir();
        let launch = AgentLaunch { program: Some(home.join("bin").join("codex").display().to_string()), args: None };
        let mut deps = CodexAdapterDeps::new(Arc::new(NoTurns), home.join(".codex").display().to_string(), "codex login");
        deps.launch = Some(launch);
        let missing = create_codex_adapter(deps).expect("a codex adapter").conversations();
        let lists: Rc<Cell<u32>> = Rc::default();
        let counted = lists.clone();
        let codex: Arc<dyn ConversationStore> = Arc::new(Counting {
            inner: Some(missing.clone()),
            list: Rc::new(move |cwds, road| {
                counted.set(counted.get() + 1);
                missing.list(cwds, road)
            }),
        });
        let frames: Rc<Cell<u32>> = Rc::default();
        let seen_frames = frames.clone();
        let claude: Arc<dyn ConversationStore> = Arc::new(NoLive(claude_real(&home.join(".claude").display().to_string())));
        let answers: Answers = vec![(
            "transcripts.list",
            Box::new(move |_| {
                seen_frames.set(seen_frames.get() + 1);
                json!({ "id": 1, "ok": false, "error": unknown_op_line("transcripts.list") })
            }),
        )];
        let h = here(&scratch, Stores { claude: Some(claude), codex: Some(codex) }, answers);
        let project = add(&h, &folder).await;
        let warned = capture();
        for _ in 0..3 {
            let a = list(&h, &project, None).await;
            assert_eq!(a["held"].as_array().expect("held").iter().map(|x| x["agent"].clone()).collect::<Vec<_>>(), [json!("claude")]);
        }
        assert_eq!((lists.get(), frames.get()), (1, 1));
        let said = warned.lines();
        assert!(said.join("\n").contains("codex is not installed"), "{said:?}");
        assert_eq!(said.len(), 1);
        h.fx.rt.close().await.expect("a close");
    });
}

// "a project's kept conversation list" > "is read again once a thread of the project ends, and once a minute has passed"
#[test]
fn is_read_again_once_a_thread_of_the_project_ends_and_once_a_minute_has_passed() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let (claude, asked) =
            fake_store(Spec { rows: Rc::new(RefCell::new(vec![terminal(ID, &folder, 100.0)])), live: Live::Ids(Rc::default()), ..spec() });
        let clock = FakeClock::at_now();
        let h = here_with(&scratch, Stores { claude: Some(claude), codex: None }, vec![], Some(&clock), None);
        let project = add(&h, &folder).await;
        list(&h, &project, None).await;
        list(&h, &project, None).await;
        assert_eq!(asked.borrow().len(), 1);
        let ws = run_in_folder(&h, &project).await;
        h.fx.rt.sessions().start(&ws, start("hello", Some("claude")), None).await.expect("a start").finished().await;
        list(&h, &project, None).await;
        assert_eq!(asked.borrow().len(), 2);
        clock.advance(KEPT_LIST_MS);
        list(&h, &project, None).await;
        assert_eq!(asked.borrow().len(), 3);
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== a conversation read for the picker =====

fn earlier_of(cwd: &str) -> ConversationEarlier {
    ConversationEarlier {
        cwd: Some(cwd.into()),
        title: Some("lab codewords".into()),
        lines: vec![
            ConversationLine { who: "person".into(), text: "Remember the codeword ALPHA.".into(), tool: None, at: None },
            ConversationLine { who: "tool".into(), text: r#"{"command":"echo hi-from-tool"}"#.into(), tool: Some("Bash".into()), at: None },
            ConversationLine { who: "agent".into(), text: "ALPHA".into(), tool: None, at: None },
        ],
        earlier: 3.0,
    }
}

fn reads(earlier: ConversationEarlier) -> EarlierFn {
    Rc::new(move |_| EarlierRead::Read(earlier.clone()))
}

fn read_ask(project: &str, agent: &str, id: &str, preview: Option<bool>) -> ConversationAsk {
    ConversationAsk { project: Some(project.into()), agent: agent.into(), id: id.into(), preview }
}

// "a conversation read for the picker" > "answers the rows a thread on it would open with, read without taking it and starting nothing"
#[test]
fn answers_the_rows_a_thread_on_it_would_open_with_read_without_taking_it_and_starting_nothing() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let (claude, asked) = fake_store(Spec {
            earlier: reads(earlier_of("/work/acme/lab-wt")),
            live: Live::Ids(Rc::new(RefCell::new(vec![ID.to_string()]))),
            ..spec()
        });
        let h = here(&scratch, Stores { claude: Some(claude), codex: None }, vec![]);
        let project = add(&h, &folder).await;
        let read = j(&h.fx.rt.conversations().read(read_ask(&project, "claude", ID, None), None).await.expect("a read"));
        assert_eq!(
            read,
            json!({
                "cwd": "/work/acme/lab-wt", "title": "lab codewords",
                "rows": [
                    { "who": "note", "text": earlier_line(3.0, "Claude Code") },
                    { "who": "person", "text": "Remember the codeword ALPHA." },
                    { "who": "tool", "text": "$ echo hi-from-tool" },
                    { "who": "agent", "text": "ALPHA" },
                ],
            })
        );
        let reads: Vec<AskedOf> = asked.borrow().iter().filter(|a| a.probe.is_some()).cloned().collect();
        let first_cwds = asked.borrow()[0].cwds.clone();
        assert_eq!(
            reads,
            [AskedOf { cwds: first_cwds, probe: Some(false), last: Some(EARLIER_MESSAGES as u32), tail: None, count_off: false }]
        );
        assert!(h.seen.starts.borrow().is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "a conversation read for the picker" > "previews the newest few off the end of the agent's file, with no count of the rest"
#[test]
fn previews_the_newest_few_off_the_end_of_the_agents_file_with_no_count_of_the_rest() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let (claude, asked) = fake_store(Spec { earlier: reads(earlier_of("/work/acme/lab-wt")), ..spec() });
        let h = here(&scratch, Stores { claude: Some(claude), codex: None }, vec![]);
        let project = add(&h, &folder).await;
        let preview = j(&h.fx.rt.conversations().read(read_ask(&project, "claude", ID, Some(true)), None).await.expect("a read"));
        let who: Vec<Value> = preview["rows"].as_array().expect("rows").iter().map(|r| r["who"].clone()).collect();
        assert_eq!(who, [json!("person"), json!("tool"), json!("agent")]);
        let last = asked.borrow().last().cloned().expect("a read");
        assert_eq!(
            (last.probe, last.last, last.tail, last.count_off),
            (Some(false), Some(PREVIEW_MESSAGES as u32), Some(PREVIEW_TAIL_BYTES), true)
        );
        h.fx.rt.close().await.expect("a close");
    });
}

// "a conversation read for the picker" > "is refused where the agent holds no such conversation, or reads none"
#[test]
fn is_refused_where_the_agent_holds_no_such_conversation_or_reads_none() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let (claude, _) = fake_store(Spec { earlier: Rc::new(|_| EarlierRead::Gone), ..spec() });
        let h = here(&scratch, Stores { claude: Some(claude), codex: None }, vec![]);
        let project = add(&h, &folder).await;
        let gone = h.fx.rt.conversations().read(read_ask(&project, "claude", OTHER, None), None).await.expect_err("a refusal");
        assert!(gone.message.contains("holds no conversation"), "{}", gone.message);
        let none = h.fx.rt.conversations().read(read_ask(&project, "codex", OTHER, None), None).await.expect_err("a refusal");
        assert!(none.message.contains("cannot pick up Codex conversations"), "{}", none.message);
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== a thread opened on a conversation from outside wsp =====

async fn events_of(h: &Here, ws: &str) -> Vec<Value> {
    h.fx.rt.sessions().history(ws, None).await.expect("a history").iter().map(j).collect()
}

// "a thread opened on a conversation from outside wsp" > "resumes that id under the start's agent, in the folder it ran in, on that folder's record, named as it is, writing nothing into the agent's store"
#[test]
fn resumes_that_id_under_the_starts_agent_in_the_folder_it_ran_in_on_that_folders_record_named_as_it_is_writing_nothing_into_the_agents_store(
) {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, tree) = scratch.repo_with_tree();
        let (claude, _) =
            fake_store(Spec { earlier: reads(earlier_of(&tree.display().to_string())), live: Live::Ids(Rc::default()), ..spec() });
        let h = here(&scratch, Stores { claude: Some(claude), codex: None }, vec![]);
        let project = add(&h, &folder).await;
        let (workspace, _) = start_on(&h, &project, resume(ID), "go on", None).await.expect("a start");
        let starts = h.seen.starts.borrow().clone();
        assert_eq!(starts.len(), 1);
        assert_eq!((starts[0].resume.as_deref(), starts[0].cwd.as_deref()), (Some(ID), Some(tree.display().to_string().as_str())));
        assert!(!starts[0].copy);
        assert!(starts[0].title.is_none());
        assert_eq!(workspace["worktree"]["path"], tree.display().to_string());
        let id = workspace["id"].as_str().expect("an id").to_string();
        let list = h.fx.rt.sessions().list(Some(id), None).await.expect("a list");
        assert_matches(
            &j(&list[0]),
            json!({ "harness": "claude", "claudeSessionId": ID, "harnessTitle": "lab codewords", "titleSource": "auto" }),
        );
        assert_eq!(j(&fold_threads(&list)[0])["title"], "lab codewords");
        sleep_ms(20).await;
        assert!(h.seen.renames.borrow().is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread opened on a conversation from outside wsp" > "opens with the conversation's earlier messages once, ahead of the prompt, a call as its line and the rest counted"
#[test]
fn opens_with_the_conversations_earlier_messages_once_ahead_of_the_prompt_a_call_as_its_line_and_the_rest_counted() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let (claude, _) =
            fake_store(Spec { earlier: reads(earlier_of(&folder.display().to_string())), live: Live::Ids(Rc::default()), ..spec() });
        let h = here(&scratch, Stores { claude: Some(claude), codex: None }, vec![]);
        let project = add(&h, &folder).await;
        let (workspace, _) = start_on(&h, &project, resume(ID), "go on", None).await.expect("a start");
        let ws = workspace["id"].as_str().expect("an id").to_string();
        let events = events_of(&h, &ws).await;
        let thread_id = j(&h.fx.rt.sessions().list(Some(ws.clone()), None).await.expect("a list")[0])["threadId"]
            .as_str()
            .expect("a thread")
            .to_string();
        let shown: Vec<String> = events
            .iter()
            .filter(|e| e["threadId"] == thread_id.as_str())
            .map(|e| match e["type"].as_str().unwrap_or_default() {
                "session.earlier" => format!("{}: {}", e["who"].as_str().unwrap_or_default(), e["text"].as_str().unwrap_or_default()),
                "session.start" => format!("start: {}", e["prompt"].as_str().unwrap_or_default()),
                other => other.to_string(),
            })
            .collect();
        assert_eq!(
            shown[..5],
            [
                format!("note: {}", earlier_line(3.0, "Claude Code")),
                "person: Remember the codeword ALPHA.".to_string(),
                "tool: $ echo hi-from-tool".to_string(),
                "agent: ALPHA".to_string(),
                "start: go on".to_string(),
            ]
        );
        let again = StartOptions { thread: Some(thread_id.clone()), ..start("and again", None) };
        h.fx.rt.sessions().start(&ws, again, None).await.expect("a start").finished().await;
        let after = events_of(&h, &ws).await.iter().filter(|e| e["type"] == "session.earlier").count();
        assert_eq!(after, 4);
        assert_eq!(h.seen.starts.borrow()[1].resume.as_deref(), Some(ID));
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread opened on a conversation from outside wsp" > "keeps on each earlier message the time it was written, not the time of the resume, the count at the first of them"
#[test]
fn keeps_on_each_earlier_message_the_time_it_was_written_not_the_time_of_the_resume_the_count_at_the_first_of_them() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let written: i64 = 1_791_848_679_454;
        let mut earlier = earlier_of(&folder.display().to_string());
        for (i, line) in earlier.lines.iter_mut().enumerate() {
            line.at = Some((written + i as i64 * 1000) as f64);
        }
        let (claude, _) = fake_store(Spec { earlier: reads(earlier), live: Live::Ids(Rc::default()), ..spec() });
        let h = here(&scratch, Stores { claude: Some(claude), codex: None }, vec![]);
        let project = add(&h, &folder).await;
        let before = common::now_ms() as f64;
        let (workspace, _) = start_on(&h, &project, resume(ID), "go on", None).await.expect("a start");
        let events = events_of(&h, workspace["id"].as_str().expect("an id")).await;
        let at: Vec<Value> = events.iter().filter(|e| e["type"] == "session.earlier").map(|e| e["at"].clone()).collect();
        assert_eq!(at, [json!(written), json!(written), json!(written + 1000), json!(written + 2000)]);
        let start_at = events.iter().find(|e| e["type"] == "session.start").expect("a start")["at"].as_f64().expect("a time");
        assert!(start_at >= before);
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread opened on a conversation from outside wsp" > "hands a lost session's seed the earlier messages it opened with"
#[test]
fn hands_a_lost_sessions_seed_the_earlier_messages_it_opened_with() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let (claude, _) =
            fake_store(Spec { earlier: reads(earlier_of(&folder.display().to_string())), live: Live::Ids(Rc::default()), ..spec() });
        let h = here(&scratch, Stores { claude: Some(claude), codex: None }, vec![]);
        let project = add(&h, &folder).await;
        start_on(&h, &project, resume(ID), "go on", None).await.expect("a start");
        let seeds = h.seen.seeds.borrow().clone();
        assert_eq!(seeds.len(), 1);
        assert!(seeds[0].contains("Remember the codeword ALPHA."));
        assert!(seeds[0].contains("ALPHA"));
        assert!(!seeds[0].contains("go on"));
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread opened on a conversation from outside wsp" > "is refused in two halves with its own kind where another app holds it open, and runs nothing"
#[test]
fn is_refused_in_two_halves_with_its_own_kind_where_another_app_holds_it_open_and_runs_nothing() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let (claude, _) = fake_store(Spec {
            earlier: reads(earlier_of(&folder.display().to_string())),
            live: Live::Ids(Rc::new(RefCell::new(vec![ID.to_string()]))),
            ..spec()
        });
        let h = here(&scratch, Stores { claude: Some(claude), codex: None }, vec![]);
        let project = add(&h, &folder).await;
        let refused = start_on(&h, &project, resume(ID), "go on", None).await.expect_err("a refusal");
        let parts = refusal_parts(&refused);
        assert_eq!(parts.kind.as_deref(), Some(CONVERSATION_OPEN_KIND));
        assert_eq!(parts.said, "lab codewords is open in another app on this computer, so continuing it here would split it.");
        assert_eq!(parts.fix.as_deref(), Some(conversation_open_fix(None).as_str()));
        assert!(h.seen.starts.borrow().is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread opened on a conversation from outside wsp" > "is refused the same where the agent itself refuses a second writer, and says when it lets go"
#[test]
fn is_refused_the_same_where_the_agent_itself_refuses_a_second_writer_and_says_when_it_lets_go() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let earlier = earlier_of(&folder.display().to_string());
        let (codex, _) = fake_store(Spec {
            earlier: Rc::new(move |probe| if probe { EarlierRead::Open } else { EarlierRead::Read(earlier.clone()) }),
            lets_go: Some("It lets go a minute later.".into()),
            ..spec()
        });
        let h = here(&scratch, Stores { claude: None, codex: Some(codex) }, vec![]);
        let project = add(&h, &folder).await;
        let refused = start_on(&h, &project, resume(ID), "x", Some("codex")).await.expect_err("a refusal");
        let parts = refusal_parts(&refused);
        assert_eq!(parts.kind.as_deref(), Some(CONVERSATION_OPEN_KIND));
        assert!(parts.said.starts_with("lab codewords is open in another app"), "{}", parts.said);
        assert_eq!(parts.fix.as_deref(), Some(conversation_open_fix(Some("It lets go a minute later.")).as_str()));
        assert!(h.seen.starts.borrow().is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread opened on a conversation from outside wsp" > "runs on a copy where the start asks one, read without taking it and never refused as open"
#[test]
fn runs_on_a_copy_where_the_start_asks_one_read_without_taking_it_and_never_refused_as_open() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let earlier = earlier_of(&folder.display().to_string());
        let (codex, asked) = fake_store(Spec {
            earlier: Rc::new(move |probe| if probe { EarlierRead::Open } else { EarlierRead::Read(earlier.clone()) }),
            lets_go: Some("It lets go a minute later.".into()),
            live: Live::Ids(Rc::new(RefCell::new(vec![ID.to_string()]))),
            ..spec()
        });
        let h = here(&scratch, Stores { claude: None, codex: Some(codex) }, vec![]);
        let project = add(&h, &folder).await;
        let (workspace, _) =
            start_on(&h, &project, ResumeAsk { id: ID.into(), copy: Some(true) }, "on a copy", Some("codex")).await.expect("a start");
        let probes: Vec<Option<bool>> = asked.borrow().iter().filter(|a| a.probe.is_some()).map(|a| a.probe).collect();
        assert_eq!(probes, [Some(false)]);
        let starts = h.seen.starts.borrow().clone();
        assert_eq!((starts[0].resume.as_deref(), starts[0].copy), (Some(ID), true));
        let mine = j(&h.fx.rt.sessions().list(Some(workspace["id"].as_str().expect("an id").to_string()), None).await.expect("a list")[0]);
        assert_ne!(mine["claudeSessionId"], ID);
        assert!(h.seen.seeds.borrow().is_empty());
        sleep_ms(20).await;
        assert!(h.seen.renames.borrow().is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread opened on a conversation from outside wsp" > "is refused beside a thread, a branch or a folder, since the conversation names where it runs"
#[test]
fn is_refused_beside_a_thread_a_branch_or_a_folder_since_the_conversation_names_where_it_runs() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let (claude, _) =
            fake_store(Spec { earlier: reads(earlier_of(&folder.display().to_string())), live: Live::Ids(Rc::default()), ..spec() });
        let h = here(&scratch, Stores { claude: Some(claude), codex: None }, vec![]);
        let project = add(&h, &folder).await;
        let besides = [
            PlacedStart { project: Some(project.clone()), branch: Some("feat/x".into()), ..Default::default() },
            PlacedStart { project: Some(project.clone()), cwd: Some(folder.display().to_string()), ..Default::default() },
            PlacedStart { project: Some(project.clone()), thread: Some("th_1".into()), ..Default::default() },
        ];
        for beside in besides {
            let refused = h.fx.rt.conversations().placed(beside, resume(ID), None).await.expect_err("a refusal");
            let parts = refusal_parts(&refused);
            assert_eq!(
                (parts.kind.as_deref(), parts.said.as_str()),
                (Some("usage"), "a resumed conversation runs where it ran, so --resume takes no --branch, --cwd or --beside.")
            );
        }
        assert!(h.seen.starts.borrow().is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread opened on a conversation from outside wsp" > "is refused before any turn where the agent holds no such conversation, never opening a new one"
#[test]
fn is_refused_before_any_turn_where_the_agent_holds_no_such_conversation_never_opening_a_new_one() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let (claude, _) = fake_store(Spec { earlier: Rc::new(|_| EarlierRead::Gone), live: Live::Ids(Rc::default()), ..spec() });
        let h = here(&scratch, Stores { claude: Some(claude), codex: None }, vec![]);
        let project = add(&h, &folder).await;
        let refused = start_on(&h, &project, resume(ID), "go on", None).await.expect_err("a refusal");
        let parts = refusal_parts(&refused);
        assert_eq!(parts.kind.as_deref(), Some("not-found"));
        assert_eq!(parts.said, format!("Claude Code holds no conversation {ID} on this computer any more."));
        assert_eq!(parts.fix.as_deref(), Some("Pick one that is still there from wsp conversations."));
        assert!(h.seen.starts.borrow().is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread opened on a conversation from outside wsp" > "is refused where the conversation is a wsp thread already, which is the thread to send into"
#[test]
fn is_refused_where_the_conversation_is_a_wsp_thread_already_which_is_the_thread_to_send_into() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let (claude, _) =
            fake_store(Spec { earlier: reads(earlier_of(&folder.display().to_string())), live: Live::Ids(Rc::default()), ..spec() });
        let h = here(&scratch, Stores { claude: Some(claude), codex: None }, vec![]);
        let project = add(&h, &folder).await;
        let ws = run_in_folder(&h, &project).await;
        h.fx.rt.sessions().start(&ws, start("hello", Some("claude")), None).await.expect("a start").finished().await;
        let mine = j(&h.fx.rt.sessions().list(Some(ws.clone()), None).await.expect("a list")[0]);
        let session = mine["claudeSessionId"].as_str().expect("a session").to_string();
        let refused = start_on(&h, &project, resume(&session), "go on", None).await.expect_err("a refusal");
        let parts = refusal_parts(&refused);
        assert_eq!(parts.kind.as_deref(), Some("usage"));
        let thread = mine["threadId"].as_str().expect("a thread");
        assert!(parts.said.contains(&format!("is thread {} already", &thread[..8])), "{}", parts.said);
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread opened on a conversation from outside wsp" > "refuses a conversation on a send into a thread that has run"
#[test]
fn refuses_a_conversation_on_a_send_into_a_thread_that_has_run() {
    drive(|| async {
        let scratch = Rc::new(Scratch::default());
        let (folder, _tree) = scratch.repo_with_tree();
        let (claude, _) =
            fake_store(Spec { earlier: reads(earlier_of(&folder.display().to_string())), live: Live::Ids(Rc::default()), ..spec() });
        let h = here(&scratch, Stores { claude: Some(claude), codex: None }, vec![]);
        let project = add(&h, &folder).await;
        let ws = run_in_folder(&h, &project).await;
        h.fx.rt.sessions().start(&ws, start("hello", Some("claude")), None).await.expect("a start").finished().await;
        let thread = j(&h.fx.rt.sessions().list(Some(ws.clone()), None).await.expect("a list")[0])["threadId"]
            .as_str()
            .expect("a thread")
            .to_string();
        let outside = match h.fx.rt.conversations().opening(&ws, None, resume(OTHER), None).await {
            Ok(opening) => opening,
            Err(_) => wsp_core::types::api::OutsideOpening {
                harness: "claude".into(),
                id: OTHER.into(),
                copy: false,
                project: project.clone(),
                cwd: None,
                title: "x".into(),
                earlier: vec![],
            },
        };
        let opts = StartOptions { thread: Some(thread), outside: Some(outside), ..start("x", None) };
        let refused = h.fx.rt.sessions().start(&ws, opts, None).await.err().expect("a refusal");
        let parts = refusal_parts(&refused);
        assert_eq!(parts.kind.as_deref(), Some("usage"));
        assert!(parts.said.contains("--resume opens a thread"), "{}", parts.said);
        h.fx.rt.close().await.expect("a close");
    });
}
