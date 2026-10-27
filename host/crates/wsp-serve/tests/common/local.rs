// SPDX-License-Identifier: AGPL-3.0-only
//! The pieces fork.test.ts needs beyond the shared fixture: `gitCopier` (git-copier.ts, a copier whose worktree half
//! is real git), a recorder of the turns a fork asked an adapter to count, and the one-shot websocket request of
//! ws-client.ts `wsRequest`.
use std::cell::RefCell;
use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::{SinkExt, StreamExt};
use indexmap::IndexMap;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;
use wsp_adapters::port::{AdapterError, AttachmentRoad, ForkTurn, ForkedSession, HarnessAdapter, HarnessSession, HarnessStartOptions};
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_engine::copier::{
    fake_copier, Copier, CopyAsk, CopyReport, CopyRoad, FakeCopier, FakeCopierLog, WorktreeAsk, WorktreeAt, WorktreeProject,
    WorktreeRemoval, WorktreeRemove, WorktreeReport,
};
use wsp_engine::errors::{EngineError, Result};
use wsp_engine::local_backend::MemoryHere;
use wsp_wire::views::workspace_more::FoundModule;

const GIT_ENV: [(&str, &str); 4] = [
    ("GIT_AUTHOR_NAME", "t"),
    ("GIT_AUTHOR_EMAIL", "t@example.com"),
    ("GIT_COMMITTER_NAME", "t"),
    ("GIT_COMMITTER_EMAIL", "t@example.com"),
];

fn other(message: String) -> EngineError {
    EngineError::Other { message, code: None }
}

/// git-copier.ts `git`: `git -C cwd args` under the test identity, the trimmed stdout; a failure is an error.
fn run_git(cwd: &str, args: &[&str]) -> Result<String> {
    let out = Command::new("git").arg("-C").arg(cwd).args(args).envs(GIT_ENV).output().map_err(|e| other(e.to_string()))?;
    if !out.status.success() {
        return Err(other(format!("Command failed: git -C {cwd} {}\n{}", args.join(" "), String::from_utf8_lossy(&out.stderr))));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// What a `GitCopier` was asked, as `fakeCopier()` keeps it.
pub type CopierLog = Arc<Mutex<FakeCopierLog>>;

/// git-copier.ts:16 `gitCopier()`: the engine's fake with git doing the worktree half for real. The branch checked out
/// anywhere answers that worktree, an existing branch is checked out as it stands, a new one starts at HEAD; a removal
/// is refused over files no commit holds and for a worktree outside the host's folder.
pub struct GitCopier {
    inner: FakeCopier,
}

pub fn git_copier() -> (Rc<GitCopier>, CopierLog) {
    let inner = fake_copier(None, None);
    let log = inner.log.clone();
    (Rc::new(GitCopier { inner }), log)
}

fn dirname(file: &str) -> String {
    match file.rfind('/') {
        Some(at) => file[..at].to_string(),
        None => ".".to_string(),
    }
}

fn basename(file: &str) -> &str {
    file.rsplit('/').next().unwrap_or(file)
}

#[async_trait(?Send)]
impl Copier for GitCopier {
    async fn make(&self, ask: &CopyAsk) -> Result<CopyReport> {
        self.inner.make(ask).await
    }
    async fn remove(&self, from: &str, to: &str, road: CopyRoad) -> Result<()> {
        self.inner.remove(from, to, road).await
    }
    async fn worktree(&self, ask: &WorktreeAsk) -> Result<WorktreeReport> {
        self.inner.log.lock().unwrap().worktrees.push(ask.clone());
        let ours = Path::new(&ask.home).join("worktrees");
        let ours_prefix = format!("{}/", ours.display());
        let listed = run_git(&ask.from, &["worktree", "list", "--porcelain"])?;
        let wanted = format!("branch refs/heads/{}", ask.branch);
        let held = listed.split("\n\n").find(|block| block.contains(&wanted));
        let held_at =
            held.and_then(|block| block.split('\n').next()).map(|first| first.strip_prefix("worktree ").unwrap_or(first).to_string());
        match held_at {
            // One deleted by hand is dropped and made again, as the verb does.
            Some(at) if !Path::new(&at).exists() => {
                run_git(&ask.from, &["worktree", "prune"])?;
            }
            Some(at) => {
                return Ok(WorktreeReport {
                    made: at.starts_with(&ours_prefix),
                    path: at,
                    branch: ask.branch.clone(),
                    carried: vec![],
                    plain: None,
                    overlaid: None,
                    fresh: false,
                    modules: vec![],
                    ms: 1.into(),
                });
            }
            None => {}
        }
        let safe: String =
            ask.branch.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '-' }).collect();
        let path = ours.join(&ask.project).join(safe).display().to_string();
        std::fs::create_dir_all(ours.join(&ask.project)).map_err(|e| other(e.to_string()))?;
        let exists = !Command::new("git")
            .args(["-C", &ask.from, "branch", "--list", &ask.branch])
            .output()
            .map_err(|e| other(e.to_string()))?
            .stdout
            .iter()
            .all(u8::is_ascii_whitespace);
        if exists {
            run_git(&ask.from, &["worktree", "add", "-q", &path, &ask.branch])?;
        } else {
            run_git(&ask.from, &["worktree", "add", "-q", "-b", &ask.branch, &path, "HEAD"])?;
        }
        let tracked: Vec<String> = run_git(&path, &["ls-files"])?
            .split('\n')
            .filter(|f| !f.is_empty() && std::fs::symlink_metadata(Path::new(&path).join(f)).is_ok_and(|m| m.is_file()))
            .map(str::to_string)
            .collect();
        let mut folders: Vec<String> = tracked.iter().map(|f| dirname(f)).collect();
        folders.sort();
        folders.dedup();
        let mut modules = vec![];
        for folder in &folders {
            for m in &ask.modules {
                if tracked.iter().any(|f| dirname(f) == *folder && m.lockfiles.iter().any(|l| l == basename(f))) {
                    modules.push(FoundModule { id: m.id.clone(), folder: folder.clone(), rebuild: true });
                }
            }
        }
        Ok(WorktreeReport {
            path,
            branch: ask.branch.clone(),
            made: true,
            carried: vec![],
            plain: None,
            overlaid: None,
            fresh: true,
            modules,
            ms: 1.into(),
        })
    }
    async fn worktree_mount(&self, o: &WorktreeAt) -> Result<()> {
        self.inner.worktree_mount(o).await
    }
    async fn worktree_forget(&self, o: &WorktreeProject) -> Result<()> {
        self.inner.worktree_forget(o).await
    }
    async fn worktree_remove(&self, o: &WorktreeRemove) -> Result<WorktreeRemoval> {
        self.inner.log.lock().unwrap().worktrees_removed.push(o.clone());
        let ours = format!("{}/", Path::new(&o.home).join("worktrees").display());
        if !o.path.starts_with(&ours) {
            return Err(other(format!("{} is not a worktree wsp made for this project, so nothing was removed", o.path)));
        }
        let changed = run_git(&o.path, &["status", "--porcelain"])?.split('\n').filter(|l| !l.is_empty()).count();
        if changed > 0 && !o.force {
            return Err(other(format!(
                "{} has {changed} files not committed, so it was not removed; commit them, or remove it with --force",
                o.path
            )));
        }
        run_git(&o.from, &["worktree", "remove", "--force", &o.path])?;
        Ok(WorktreeRemoval { path: o.path.clone(), rescued: None })
    }
    fn has_room(&self) -> bool {
        self.inner.has_room()
    }
    async fn room(&self) -> Option<MemoryHere> {
        self.inner.room().await
    }
}

/// An adapter that is `inner` in everything and keeps each turn a fork was asked to ready, whole: the fixture's
/// `Asked.forks` keeps only how many turns a count carried, and one case reads which they were.
pub fn recording_forks(inner: HarnessAdapterFactory) -> (HarnessAdapterFactory, Rc<RefCell<Vec<ForkTurn>>>) {
    let seen: Rc<RefCell<Vec<ForkTurn>>> = Rc::default();
    let kept = seen.clone();
    let factory: HarnessAdapterFactory = Arc::new(move |ctx| {
        let inner = inner(ctx)?;
        Ok(Arc::new(Recording { inner, seen: kept.clone() }) as Arc<dyn HarnessAdapter>)
    });
    (factory, seen)
}

struct Recording {
    inner: Arc<dyn HarnessAdapter>,
    seen: Rc<RefCell<Vec<ForkTurn>>>,
}

impl HarnessAdapter for Recording {
    fn start(&self, options: HarnessStartOptions) -> std::result::Result<Arc<dyn HarnessSession>, AdapterError> {
        self.inner.start(options)
    }
    fn steers(&self) -> bool {
        self.inner.steers()
    }
    fn attachments(&self) -> Option<AttachmentRoad> {
        self.inner.attachments()
    }
    fn forks(&self) -> bool {
        self.inner.forks()
    }
    fn forks_by_count(&self) -> bool {
        self.inner.forks_by_count()
    }
    fn fork_session(&self, session: String, turn: ForkTurn) -> Option<LocalBoxFuture<'static, std::result::Result<ForkedSession, String>>> {
        self.seen.borrow_mut().push(turn.clone());
        self.inner.fork_session(session, turn)
    }
}

/// ws-client.ts `wsRequest`: connect, auth, send one request, return the reply.
pub async fn ws_request(port: u16, token: &str, op: &str, params: Value) -> Value {
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/")).await.expect("a socket");
    let send = |id: u64, op: &str, params: &Value| {
        let mut frame = params.as_object().cloned().unwrap_or_default();
        frame.insert("id".into(), json!(id));
        frame.insert("op".into(), json!(op));
        Message::Text(Value::Object(frame).to_string().into())
    };
    ws.send(send(1, "auth", &json!({ "token": token }))).await.expect("auth sent");
    let authed = reply_to(&mut ws, 1).await;
    assert_eq!(authed["ok"], json!(true), "auth failed: {authed}");
    ws.send(send(2, op, &params)).await.expect("the request is sent");
    let reply = reply_to(&mut ws, 2).await;
    let _ = ws.close(None).await;
    reply
}

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// The reply that carries `id`: a frame with an `op` is one the host pushed, never the answer to a request.
async fn reply_to(ws: &mut Ws, id: u64) -> Value {
    loop {
        match ws.next().await.expect("the host answers").expect("a frame") {
            Message::Text(text) => {
                let frame: Value = serde_json::from_str(&text).expect("a frame is JSON");
                if frame.get("op").is_none() && frame["id"] == json!(id) {
                    return frame;
                }
            }
            Message::Close(_) => panic!("the host closed the socket before it answered"),
            _ => {}
        }
    }
}

/// What a test sets beside the wiring: the folder runs live in, the poll, a wall of its own, the environment a turn is launched
/// with, and the set that holds every run the factory is reading (`localExecStream({ root, runDir, pollMs, ...o, deadlineMs })`).
#[derive(Default)]
pub struct LocalOpts {
    pub run_dir: Option<String>,
    pub poll_ms: Option<u64>,
    pub deadline_ms: Option<f64>,
    pub vars: Option<IndexMap<String, String>>,
    /// The environment every exec of the host's own backend runs under (this process's where none).
    pub backend_vars: Option<IndexMap<String, String>>,
    pub reading: Option<Arc<wsp_core::machine_exec::ReadingSet>>,
}

/// `fakeLocal(root)` and the `local` the TS tests that stand a host on this computer write beside it: a `LocalBackend` rooted in a
/// folder of the test's own, an exec that runs in that folder, and the copier the test hands in.
pub fn local_wiring(root: &Path, copier: Rc<dyn Copier>) -> wsp_core::types::wiring::LocalWiring {
    local_wiring_with(root, copier, LocalOpts::default())
}

pub fn local_wiring_with(root: &Path, copier: Rc<dyn Copier>, o: LocalOpts) -> wsp_core::types::wiring::LocalWiring {
    use wsp_core::types::wiring::{LocalWiring, Platform};
    use wsp_engine::local_backend::{LocalBackend, LocalBackendOptions};
    let folder = root.display().to_string();
    let home = folder.clone();
    let runs = o.run_dir.clone().unwrap_or_else(|| format!("{folder}/runs"));
    let exec_root = folder.clone();
    let (poll_ms, deadline_ms, reading) = (o.poll_ms, o.deadline_ms, o.reading.clone());
    let launch_vars = o
        .vars
        .clone()
        .unwrap_or_else(|| IndexMap::from([("PATH".to_string(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()))]));
    LocalWiring {
        backend: Arc::new(LocalBackend::new(LocalBackendOptions { root: root.to_path_buf(), env: o.backend_vars.clone() })),
        folder: folder.clone(),
        exec_stream: Rc::new(move |opts, waiting| {
            let mut local = wsp_core::local_exec::LocalExecOptions::new(exec_root.clone(), runs.clone());
            local.poll_ms = poll_ms;
            if let Some(opts) = opts {
                local.idle_ms = opts.idle_ms;
                local.deadline_ms = opts.deadline_ms;
                local.now = opts.now;
                local.poll_ms = opts.poll_ms.or(poll_ms);
                local.reading = opts.reading;
            }
            if deadline_ms.is_some() {
                local.deadline_ms = deadline_ms;
            }
            if reading.is_some() {
                local.reading = reading.clone();
            }
            wsp_core::local_exec::local_exec_stream(local, waiting)
        }),
        home: Rc::new(move |_| format!("{home}/.claude")),
        home_dir: folder.clone(),
        roots_path: format!("{folder}/roots"),
        env: Rc::new(move || launch_vars.clone()),
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
    }
}

/// stub-script.ts `writeStub`: a script at `path` the shell can run, made executable.
pub fn write_stub(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, body).expect("a stub");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("a mode");
}

/// ws-client.ts `WsClient`: a socket that authenticates with a token, answers each request by its id and keeps every frame the host
/// pushed it, a frame with an `op` or a `type` that no request is waiting on.
pub struct WsClient {
    sink: futures::stream::SplitSink<Ws, Message>,
    pending: Rc<RefCell<std::collections::HashMap<u64, futures::channel::oneshot::Sender<Value>>>>,
    pub events: Rc<RefCell<Vec<Value>>>,
    next: std::cell::Cell<u64>,
    reader: tokio::task::JoinHandle<()>,
}

impl WsClient {
    pub async fn connect(port: u16, token: &str) -> WsClient {
        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/")).await.expect("a socket");
        let (sink, mut stream) = ws.split();
        let pending: Rc<RefCell<std::collections::HashMap<u64, futures::channel::oneshot::Sender<Value>>>> = Rc::default();
        let events: Rc<RefCell<Vec<Value>>> = Rc::default();
        let (waiting, kept) = (pending.clone(), events.clone());
        let reader = tokio::task::spawn_local(async move {
            while let Some(Ok(message)) = stream.next().await {
                let Message::Text(text) = message else { continue };
                let frame: Value = serde_json::from_str(&text).expect("a frame is JSON");
                if frame["op"].is_string() {
                    kept.borrow_mut().push(frame);
                    continue;
                }
                let answered = frame["id"].as_u64().and_then(|id| waiting.borrow_mut().remove(&id));
                match answered {
                    Some(reply) => {
                        let _ = reply.send(frame);
                    }
                    None if frame["type"].is_string() => kept.borrow_mut().push(frame),
                    None => {}
                }
            }
        });
        let mut client = WsClient { sink, pending, events, next: std::cell::Cell::new(1), reader };
        let authed = client.request("auth", json!({ "token": token })).await;
        assert_eq!(authed["ok"], json!(true), "auth failed: {authed}");
        client
    }

    pub async fn request(&mut self, op: &str, params: Value) -> Value {
        let id = self.next.get();
        self.next.set(id + 1);
        let (tx, rx) = futures::channel::oneshot::channel();
        self.pending.borrow_mut().insert(id, tx);
        let mut frame = params.as_object().cloned().unwrap_or_default();
        frame.insert("id".into(), json!(id));
        frame.insert("op".into(), json!(op));
        self.sink.send(Message::Text(Value::Object(frame).to_string().into())).await.expect("the request is sent");
        rx.await.expect("the host answers")
    }

    pub fn close(self) {
        self.reader.abort();
    }
}

/// What a test does to a command before this computer's machine runs it: `Ok(cmd)` runs that command, `Err(result)` answers in its place.
pub type Around = Rc<dyn Fn(String) -> LocalBoxFuture<'static, std::result::Result<String, wsp_engine::machine::ExecResult>>>;

/// The TS tests' `machine.exec = async (cmd, o) => ...` on the machine a `LocalBackend` hands out: every exec of the one machine goes
/// through `around` first, and nothing else about the backend changes.
pub struct AroundBackend {
    inner: Arc<wsp_engine::local_backend::LocalBackend>,
    around: Around,
}

impl AroundBackend {
    pub fn new(root: &Path, env: Option<IndexMap<String, String>>, around: Around) -> Arc<AroundBackend> {
        use wsp_engine::local_backend::{LocalBackend, LocalBackendOptions};
        Arc::new(AroundBackend { inner: Arc::new(LocalBackend::new(LocalBackendOptions { root: root.to_path_buf(), env })), around })
    }
}

struct AroundMachine {
    inner: Arc<dyn wsp_engine::machine::Machine>,
    around: Around,
}

#[async_trait(?Send)]
impl wsp_engine::machine::MachineBackend for AroundBackend {
    fn capabilities(&self) -> wsp_engine::machine::Capabilities {
        self.inner.capabilities()
    }
    fn pricing(&self) -> Arc<dyn wsp_engine::machine::BackendPricing> {
        self.inner.pricing()
    }
    async fn create(
        &self,
        spec: wsp_engine::machine::MachineSpec,
        waiting: Option<wsp_engine::machine::Waiting>,
        sent: Option<String>,
    ) -> Result<Arc<dyn wsp_engine::machine::Machine>> {
        self.inner.create(spec, waiting, sent).await
    }
    async fn get(&self, id: &str) -> Result<Arc<dyn wsp_engine::machine::Machine>> {
        let machine = self.inner.get(id).await?;
        Ok(Arc::new(AroundMachine { inner: machine, around: self.around.clone() }))
    }
    async fn list(&self, labels: Option<&IndexMap<String, String>>) -> Result<Vec<wsp_engine::machine::MachineListRow>> {
        self.inner.list(labels).await
    }
    async fn delete_snapshot(&self, id: &str) -> Result<()> {
        self.inner.delete_snapshot(id).await
    }
}

#[async_trait(?Send)]
impl wsp_engine::machine::Machine for AroundMachine {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn kind(&self) -> wsp_engine::machine::MachineKind {
        self.inner.kind()
    }
    fn takes_stdin(&self) -> bool {
        self.inner.takes_stdin()
    }
    fn offers(&self) -> wsp_engine::machine::MachineOffers {
        self.inner.offers()
    }
    async fn exec(&self, cmd: &str, opts: wsp_engine::machine::ExecOptions) -> Result<wsp_engine::machine::ExecResult> {
        match (self.around)(cmd.to_string()).await {
            Ok(cmd) => self.inner.exec(&cmd, opts).await,
            Err(answer) => Ok(answer),
        }
    }
    async fn run(&self, script: &str, opts: wsp_engine::machine::RunOptions) -> Result<wsp_engine::machine::ExecResult> {
        self.inner.run(script, opts).await
    }
    async fn snapshot(
        &self,
        name: &str,
        life: wsp_engine::machine::MachineLife,
        opts: wsp_engine::machine::SnapshotOptions,
    ) -> Result<String> {
        self.inner.snapshot(name, life, opts).await
    }
    async fn pause(&self, wait_ms: Option<u64>) -> Result<()> {
        self.inner.pause(wait_ms).await
    }
    async fn resume(&self, signal: Option<&wsp_engine::cancel::Cancel>) -> Result<()> {
        self.inner.resume(signal).await
    }
    async fn kill(&self) -> Result<()> {
        self.inner.kill().await
    }
    async fn state(&self) -> Result<wsp_engine::machine::MachineState> {
        self.inner.state().await
    }
    async fn download_url(&self, path: &str) -> Result<String> {
        self.inner.download_url(path).await
    }
    async fn upload_url(&self, path: &str) -> Result<String> {
        self.inner.upload_url(path).await
    }
    async fn describe(&self) -> Result<wsp_engine::machine::MachineShape> {
        self.inner.describe().await
    }
    async fn facts(&self) -> Result<wsp_engine::machine::MachineFacts> {
        self.inner.facts().await
    }
    async fn put_bytes(&self, path: &str, bytes: &[u8], timeout_ms: Option<u64>) -> Result<Option<wsp_engine::machine::BytesLanded>> {
        self.inner.put_bytes(path, bytes, timeout_ms).await
    }
}
