// SPDX-License-Identifier: AGPL-3.0-only
//! The composer tied by `link::Link` to a thread on a host started for each test alone: a fresh state file in a folder
//! of its own, nothing of this machine's environment, and a stand-in claude first on its PATH that answers a prompt at
//! once, or holds a SLOW one open until it is interrupted and echoes each message steered into it. Every test checks
//! what the host's own transcript says happened, not only what the composer believes.
//!
//! The project's file list comes from a wsp-daemon, which a checkout does not build: the host runs from a copy of
//! packages/host whose assets folder carries the daemon the installed wsp keeps, and without one the file list is
//! skipped with a line saying so.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::composer::link::{self, Link, Target};
use desktop_gpui::screens::composer::{self, Composer};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{AppContext, Entity, Focusable, TestAppContext, VisualTestContext};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::{protocol, Env, Failure, HostClient};

const DAEMON_ASSETS: &str = "/root/wsp-daemon/wsp/assets/daemon";
const WAIT: Duration = Duration::from_secs(30);

const STAND_IN: &str = r#"
const argv = process.argv.slice(2);
const at = flag => (argv.indexOf(flag) >= 0 ? argv[argv.indexOf(flag) + 1] : undefined);
const sid = at("--session-id") ?? at("--resume");
const say = o => process.stdout.write(`${JSON.stringify(o)}\n`);
if (sid === undefined) {
  if (argv.includes("--help")) console.log('--permission-mode <mode>  (choices: "default", "bypassPermissions")');
  else if (argv.includes("--bare")) say({ type: "control_response", response: { subtype: "success", request_id: "init", response: { models: [{ value: "default", resolvedModel: "claude-haiku-5-5" }] } } });
  else console.log("2.1.295 (Claude Code)");
  process.exit(0);
}
const usage = { input_tokens: 1, output_tokens: 1 };
let n = 0;
let waiting = false;
const init = () => say({ type: "system", subtype: "init", cwd: process.cwd(), session_id: sid, tools: [], mcp_servers: [], model: "claude-haiku-5-5", permissionMode: "bypassPermissions", slash_commands: [], apiKeySource: "none", uuid: `init_${++n}` });
const text = words => {
  const id = ++n;
  say({ type: "assistant", message: { id: `msg_${id}`, type: "message", role: "assistant", model: "claude-haiku-5-5", content: [{ type: "text", text: words }], stop_reason: null, usage }, parent_tool_use_id: null, session_id: sid, uuid: `a_${id}` });
};
const result = (subtype, more) => say({ type: "result", subtype, is_error: false, duration_ms: 1, num_turns: 1, session_id: sid, total_cost_usd: 0, usage, uuid: `r_${++n}`, ...more });
let buf = "";
process.stdin.on("data", d => {
  buf += d;
  for (let i = buf.indexOf("\n"); i >= 0; i = buf.indexOf("\n")) {
    const line = buf.slice(0, i);
    buf = buf.slice(i + 1);
    if (line.trim() === "") continue;
    const m = JSON.parse(line);
    if (m.type === "control_request") {
      say({ type: "control_response", response: { subtype: "success", request_id: m.request_id } });
      if (m.request?.subtype === "interrupt" && waiting) {
        waiting = false;
        result("error_during_execution", { errors: ["interrupted by the person"] });
      }
    } else if (m.type === "user") {
      const prompt = (m.message?.content ?? []).filter(c => c.type === "text").map(c => c.text).join("");
      if (waiting) text(`steered: ${prompt}`);
      else {
        init();
        if (prompt.includes("SLOW")) waiting = true;
        else {
          text(`ok: ${prompt}`);
          result("success", { result: `ok: ${prompt}` });
        }
      }
    }
  }
});
process.stdin.on("end", () => process.exit(0));
"#;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

/// node off this process's PATH, named by its full path to every child, whose PATH holds only what the test gives it.
fn node() -> PathBuf {
    std::env::var_os("PATH")
        .and_then(|path| std::env::split_paths(&path).map(|dir| dir.join("node")).find(|p| p.is_file()))
        .unwrap_or_else(|| PathBuf::from("/usr/local/bin/node"))
}

fn triple() -> String {
    let os = if cfg!(target_os = "macos") { "apple-darwin" } else { "unknown-linux-musl" };
    format!("{}-{os}", std::env::consts::ARCH)
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn kind(event: &Value) -> &str {
    event["type"].as_str().unwrap_or_default()
}

fn started(history: &[Value], prompt: &str) -> bool {
    history.iter().any(|e| kind(e) == "session.start" && e["prompt"] == prompt)
}

fn said(history: &[Value], words: &str) -> bool {
    history.iter().any(|e| kind(e) == "session.delta" && e["text"].as_str().is_some_and(|t| t.contains(words)))
}

/// The turn `prompt` started has a `session.end` after its start.
fn ended(history: &[Value], prompt: &str) -> bool {
    let Some(start) = history.iter().position(|e| kind(e) == "session.start" && e["prompt"] == prompt) else { return false };
    history[start..].iter().any(|e| kind(e) == "session.end")
}

fn kinds(history: &[Value]) -> Vec<&str> {
    history.iter().map(kind).collect()
}

/// The host process and its folder, stopped and removed with this.
struct Server {
    dir: PathBuf,
    node: PathBuf,
    bin: PathBuf,
    child: Child,
}

struct Host {
    client: Arc<HostClient>,
    runtime: Arc<Runtime>,
    target: Target,
    daemon: bool,
    server: Server,
}

impl Server {
    fn state(&self) -> PathBuf {
        self.dir.join("state/state.json")
    }

    fn env(dir: &Path, node: &Path) -> Env {
        let path = format!("{}:{}:/usr/bin:/bin", dir.join("bin").display(), node.parent().unwrap().display());
        [("HOME", dir.to_string_lossy().into_owned()), ("PATH", path), ("WSP_UPDATE_CHECK", "0".to_owned())]
            .map(|(k, v)| (k.to_owned(), v))
            .into_iter()
            .collect()
    }

    fn command(&self, program: &Path) -> Command {
        let mut command = Command::new(program);
        command.env_clear().envs(Self::env(&self.dir, &self.node)).current_dir(&self.dir).stdin(Stdio::null());
        command
    }

    /// One wsp line against this host, which has to succeed.
    fn wsp(&self, args: &[&str]) -> String {
        let out = self.command(&self.node).arg(&self.bin).args(args).arg("--state").arg(self.state()).output().unwrap();
        let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        assert!(out.status.success(), "wsp {args:?}: {said}\nhost log: {}", self.log());
        said
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.dir.join("up.log")).unwrap_or_default()
    }
}

impl Host {
    /// A host on a fresh state file with a project `repo` holding notes/plan.md, and a thread whose first turn ran.
    fn start() -> Host {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = repo_root();
        for built in ["packages/host/dist/bin.js", "apps/web/dist/index.html"].map(|p| root.join(p)) {
            assert!(built.exists(), "{} is not built: run pnpm install && pnpm build", built.display());
        }
        let dir = std::env::temp_dir().join(format!("composer-live-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::create_dir_all(dir.join("state")).unwrap();
        let node = node();
        let claude = dir.join("bin/claude");
        std::fs::write(&claude, format!("#!{}\n{STAND_IN}", node.display())).unwrap();
        std::fs::set_permissions(&claude, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();

        // The host reads its daemon out of the assets folder beside the bundle it runs from.
        let pkg = dir.join("hostpkg");
        copy_dir(&root.join("packages/host/dist"), &pkg.join("dist"));
        std::fs::copy(root.join("packages/host/package.json"), pkg.join("package.json")).unwrap();
        std::fs::create_dir_all(pkg.join("assets")).unwrap();
        let link = |from: PathBuf, to: &str| std::os::unix::fs::symlink(from, pkg.join(to)).unwrap();
        link(root.join("packages/host/node_modules"), "node_modules");
        link(root.join("apps/web/dist"), "assets/web");
        if root.join("packages/wspx/assets/cli").exists() {
            link(root.join("packages/wspx/assets/cli"), "assets/cli");
        }
        let daemon = Path::new(DAEMON_ASSETS).join(triple()).join("wsp-daemon").is_file();
        if daemon {
            link(PathBuf::from(DAEMON_ASSETS), "assets/daemon");
        }
        let bin = pkg.join("dist/bin.js");

        let state = dir.join("state/state.json");
        let log = std::fs::File::create(dir.join("up.log")).unwrap();
        let child = Command::new(&node)
            .arg(&bin)
            .args(["up", "--port", "0", "--no-relay", "--state"])
            .arg(&state)
            .env_clear()
            .envs(Server::env(&dir, &node))
            .current_dir(&dir)
            .stdin(Stdio::null())
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .unwrap();
        let server = Server { dir, node, bin, child };
        let until = Instant::now() + WAIT;
        // The lock is written before the host listens, with port 0 until it does.
        let listening = |server: &Server| {
            let lock = std::fs::read_to_string(server.state().with_file_name("host.lock")).unwrap_or_default();
            serde_json::from_str::<Value>(&lock).ok().and_then(|lock| lock["port"].as_u64()).is_some_and(|port| port != 0)
        };
        while !server.state().with_file_name("host-token").exists() || !listening(&server) {
            assert!(Instant::now() < until, "no host came up: {}", server.log());
            std::thread::sleep(Duration::from_millis(100));
        }

        let folder = server.dir.join("repo");
        std::fs::create_dir_all(folder.join("notes")).unwrap();
        std::fs::write(folder.join("notes/plan.md"), "# plan\n").unwrap();
        let git = |args: &[&str]| {
            let mut command = server.command(Path::new("git"));
            let ok = command
                .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
                .args(args)
                .current_dir(&folder)
                .status();
            assert!(ok.unwrap().success(), "git {args:?}");
        };
        git(&["init", "-q"]);
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "plan"]);
        server.wsp(&["add", &folder.to_string_lossy()]);
        server.wsp(&["run", "repo", "first", "--agent", "claude"]);

        let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
        let vars = Server::env(&server.dir, &server.node);
        let client = runtime.block_on(HostClient::connect(&server.state(), &vars)).unwrap_or_else(|f| panic!("{}", f.message));
        let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
        let row = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread wsp run opened");
        let target = Target { workspace_id: row.workspace_id.clone(), thread_id: row.thread_id.clone() };
        Host { client: Arc::new(client), runtime, target, daemon, server }
    }

    fn thread(&self) -> &str {
        self.target.thread_id.as_deref().unwrap()
    }

    /// The thread's transcript as the host gives it now.
    fn history(&self) -> Vec<Value> {
        let opened =
            self.runtime.block_on(self.client.open(&self.target.workspace_id, self.thread())).unwrap_or_else(|f| panic!("{}", f.message));
        opened.history.iter().map(|e| serde_json::to_value(e).unwrap()).collect()
    }

    /// Stops the thread's running turn from outside the composer, as another client would.
    fn interrupt(&self) {
        let rows = self.runtime.block_on(link::rows_of(&self.client, &self.runtime, &self.target.workspace_id, self.thread())).unwrap();
        let session = link::session_id(&rows, None).expect("a row to interrupt");
        self.interrupt_session(&session).unwrap_or_else(|f| panic!("sessions.interrupt: {}", f.message));
    }

    fn interrupt_session(&self, session: &str) -> Result<(), Failure> {
        let op: protocol::SessionsInterruptRequest = serde_json::from_value(json!({ "sessionId": session })).unwrap();
        let client = self.client.clone();
        self.runtime.block_on(async move { client.call(op).await }).map(|_| ())
    }

    /// The ids of the workspace's rows whose turn runs now, on any thread.
    fn running_rows(&self) -> Vec<String> {
        let op: protocol::SessionsListRequest = serde_json::from_value(json!({ "workspaceId": self.target.workspace_id })).unwrap();
        let client = self.client.clone();
        let listed = self.runtime.block_on(async move { client.call(op).await }).map(|l| l.sessions).unwrap_or_default();
        listed.into_iter().filter(|r| serde_json::to_value(r.status).ok() == Some(json!("running"))).map(|r| r.id).collect()
    }
}

impl Drop for Host {
    /// A turn's process runs apart from the host so it outlives a restart, so a turn a failed test left running is
    /// stopped here, before the host and its folder go.
    fn drop(&mut self) {
        let until = Instant::now() + Duration::from_secs(20);
        loop {
            let running = self.running_rows();
            if running.is_empty() || Instant::now() > until {
                break;
            }
            for session in running {
                let _ = self.interrupt_session(&session);
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }
}

impl Drop for Server {
    /// A terminated host stops its daemon on its way out; a killed one leaves it running.
    fn drop(&mut self) {
        let pid = self.child.id().to_string();
        let _ = self.command(Path::new("/bin/kill")).args(["-TERM", &pid]).status();
        let until = Instant::now() + Duration::from_secs(20);
        while matches!(self.child.try_wait(), Ok(None)) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A composer in a window, tied by Link to the host's thread as the composer screen ties it.
fn tie<'a>(host: &Host, cx: &'a mut TestAppContext) -> (Entity<Composer>, Entity<Link>, &'a mut VisualTestContext) {
    tie_to(host, host.target.clone(), cx)
}

fn tie_to<'a>(host: &Host, target: Target, cx: &'a mut TestAppContext) -> (Entity<Composer>, Entity<Link>, &'a mut VisualTestContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        composer::init(cx);
    });
    let opened = host.runtime.block_on(Link::read(&host.client, &host.runtime, &target)).unwrap_or_else(|f| panic!("{}", f.message));
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let composer = cx.new(|cx| Composer::new(window, cx));
        made = Some(composer.clone());
        Root::new(composer, window, cx)
    });
    let composer = made.unwrap();
    let link = cx.update(|window, cx| {
        let composer = composer.clone();
        cx.new(|cx| Link::new(host.client.clone(), host.runtime.clone(), target, opened, composer, window, cx))
    });
    cx.run_until_parked();
    (composer, link, cx)
}

/// Lets real time pass, the host's replies landing on the window's thread, until `done` holds.
fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + WAIT;
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < end, "{what} within {} s", WAIT.as_secs());
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn settle(cx: &mut VisualTestContext) {
    let end = Instant::now() + Duration::from_millis(600);
    while Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn running(composer: &Entity<Composer>, cx: &mut VisualTestContext) -> bool {
    composer.read_with(cx, |c, _| c.running())
}

fn queued(composer: &Entity<Composer>, cx: &mut VisualTestContext) -> usize {
    composer.read_with(cx, |c, _| c.queue().len())
}

fn send(composer: &Entity<Composer>, text: &str, cx: &mut VisualTestContext) {
    composer.update_in(cx, |c, window, cx| {
        c.set_text(text, window, cx);
        c.submit(window, cx);
    });
}

/// Sends a SLOW prompt and waits for the host to say its turn runs, the composer to show it, and the rows to follow.
fn slow_turn(host: &Host, composer: &Entity<Composer>, prompt: &str, cx: &mut VisualTestContext) {
    send(composer, prompt, cx);
    until(cx, "the SLOW turn running in the composer and in the host's transcript", |cx| {
        running(composer, cx) && started(&host.history(), prompt)
    });
    settle(cx);
}

#[gpui_kit::test]
fn reads_the_agents_the_rows_the_history_and_the_project_files(cx: &mut TestAppContext) {
    let host = Host::start();
    let opened = host.runtime.block_on(Link::read(&host.client, &host.runtime, &host.target)).unwrap_or_else(|f| panic!("{}", f.message));
    let harnesses: Vec<&str> = opened.agents.iter().map(|a| a.harness.as_str()).collect();
    assert!(harnesses.contains(&"claude"), "agents {harnesses:?}");
    assert!(opened.agents.iter().any(|a| a.harness == "claude" && a.steers), "claude reads as an agent that steers");
    assert!(!opened.rows.is_empty(), "no rows for the thread");
    assert!(opened.rows.iter().all(|r| r.thread_id.as_deref() == Some(host.thread()) && r.harness == "claude"), "{:?}", opened.rows);
    let history: Vec<Value> = opened.history.iter().map(|e| serde_json::to_value(e).unwrap()).collect();
    assert!(started(&history, "first") && said(&history, "ok: first") && ended(&history, "first"), "history {:?}", kinds(&history));
    let rows = host.runtime.block_on(link::rows_of(&host.client, &host.runtime, &host.target.workspace_id, host.thread())).unwrap();
    assert_eq!(rows.iter().map(|r| &r.id).collect::<Vec<_>>(), opened.rows.iter().map(|r| &r.id).collect::<Vec<_>>());
    println!("agents {harnesses:?}, {} rows, history {:?}", opened.rows.len(), kinds(&history));

    if !host.daemon {
        println!("skipped the file list: no wsp-daemon at {DAEMON_ASSETS}/{}/wsp-daemon for the host to serve fs.files", triple());
        return;
    }
    let cwd = opened.rows.last().and_then(|r| r.cwd.clone());
    let files = host
        .runtime
        .block_on(link::list_files(&host.runtime, &host.client, &host.target.workspace_id, cwd))
        .unwrap_or_else(|f| panic!("fs.files: {}", f.message));
    assert!(files.iter().any(|f| f == "notes/plan.md"), "files {files:?}");

    let (composer, _link, cx) = tie(&host, cx);
    composer.update_in(cx, |c, window, cx| c.focus_handle(cx).focus(window, cx));
    cx.run_until_parked();
    cx.simulate_input("@pla");
    until(cx, "the mention menu listing notes/plan.md off the host", |cx| {
        composer.read_with(cx, |c, _| c.menu_items().is_some_and(|items| items.iter().any(|i| i.file && i.insert == "notes/plan.md")))
    });
    println!("files {files:?}, and the mention menu lists notes/plan.md");
}

#[gpui_kit::test]
fn a_send_starts_a_turn_on_the_thread(cx: &mut TestAppContext) {
    let host = Host::start();
    let (composer, _link, cx) = tie(&host, cx);
    send(&composer, "hello there", cx);
    let mut history = Vec::new();
    until(cx, "the thread's history gaining the send's session.start and its reply", |_| {
        history = host.history();
        started(&history, "hello there") && said(&history, "ok: hello there")
    });
    until(cx, "the composer showing the turn ended", |cx| !running(&composer, cx));
    println!("history {:?}", kinds(&history));
}

#[gpui_kit::test]
fn stop_ends_the_running_turn(cx: &mut TestAppContext) {
    let host = Host::start();
    let (composer, _link, cx) = tie(&host, cx);
    slow_turn(&host, &composer, "SLOW stop me", cx);
    composer.update(cx, |c, cx| c.stop(cx));
    let mut history = Vec::new();
    until(cx, "a session.end for the SLOW turn and the composer not running", |cx| {
        history = host.history();
        ended(&history, "SLOW stop me") && !running(&composer, cx)
    });
    let done = history.iter().rev().find(|e| kind(e) == "session.done").map(|e| e["result"]["status"].clone());
    println!("history {:?}, the turn's result {done:?}", kinds(&history));
}

#[gpui_kit::test]
fn a_steer_reaches_the_running_turn_and_leaves_the_queue(cx: &mut TestAppContext) {
    let host = Host::start();
    let (composer, _link, cx) = tie(&host, cx);
    composer.update(cx, |c, cx| c.set_steer_preferred(true, cx));
    slow_turn(&host, &composer, "SLOW steer me", cx);
    send(&composer, "turn left", cx);
    assert_eq!(queued(&composer, cx), 1, "a steer waits in the queue until the host takes it");
    let mut history = Vec::new();
    until(cx, "the host's history showing the steer and the composer's queue empty", |cx| {
        history = host.history();
        let steered =
            history.iter().any(|e| kind(e) == "session.steer" && e["prompt"] == "turn left") || said(&history, "steered: turn left");
        steered && queued(&composer, cx) == 0
    });
    assert!(running(&composer, cx), "a steer leaves the turn running");
    assert!(!started(&history, "turn left"), "the steer started a turn of its own");
    composer.update(cx, |c, cx| c.stop(cx));
    until(cx, "the steered turn ended", |cx| !running(&composer, cx));
    println!("history {:?}", kinds(&history));
}

#[gpui_kit::test]
fn a_turn_started_elsewhere_shows_in_the_composer(cx: &mut TestAppContext) {
    let host = Host::start();
    let (composer, _link, cx) = tie(&host, cx);
    host.server.wsp(&["send", host.thread(), "--detach", "SLOW from outside"]);
    until(cx, "the composer running on a turn the CLI started", |cx| running(&composer, cx));
    assert!(started(&host.history(), "SLOW from outside"));
    settle(cx);
    host.interrupt();
    until(cx, "the composer idle once another client stopped the turn", |cx| !running(&composer, cx));
    let history = host.history();
    assert!(ended(&history, "SLOW from outside"), "history {:?}", kinds(&history));
    println!("history {:?}", kinds(&history));
}

#[gpui_kit::test]
fn a_message_queued_during_a_turn_starts_the_next_one(cx: &mut TestAppContext) {
    let host = Host::start();
    let (composer, _link, cx) = tie(&host, cx);
    slow_turn(&host, &composer, "SLOW queue behind me", cx);
    send(&composer, "after the stop", cx);
    assert_eq!(queued(&composer, cx), 1, "with steering off a send during a turn waits in the queue");
    settle(cx);
    assert!(!started(&host.history(), "after the stop"), "the queued message went out while the turn ran");
    composer.update(cx, |c, cx| c.stop(cx));
    let mut history = Vec::new();
    until(cx, "the queued message's session.start and reply in the host's history", |_| {
        history = host.history();
        ended(&history, "SLOW queue behind me") && started(&history, "after the stop") && said(&history, "ok: after the stop")
    });
    until(cx, "the queue empty", |cx| queued(&composer, cx) == 0);
    println!("history {:?}", kinds(&history));
}

#[gpui_kit::test]
fn a_send_on_a_new_thread_opens_it_and_follows_it(cx: &mut TestAppContext) {
    let host = Host::start();
    let target = Target { workspace_id: host.target.workspace_id.clone(), thread_id: None };
    let (composer, _link, cx) = tie_to(&host, target, cx);
    send(&composer, "a fresh start", cx);
    let mut opened = None;
    until(cx, "the host listing a new thread whose first turn is the send", |_| {
        let threads = host.runtime.block_on(host.client.threads()).unwrap();
        opened = threads.into_iter().find(|t| t.prompt.as_deref() == Some("a fresh start")).and_then(|t| t.thread_id);
        opened.is_some()
    });
    let thread = opened.unwrap();
    until(cx, "the composer following the new thread to its turn's end", |cx| {
        composer.read_with(cx, |c, cx| !c.running() && c.send_held(cx).is_none())
    });
    // A second send goes out as a turn of the thread the first one opened, not onto a queue that never drains.
    send(&composer, "and a second", cx);
    until(cx, "the second send's turn in the new thread's history", |_| {
        let opened = host.runtime.block_on(host.client.open(&host.target.workspace_id, &thread)).unwrap();
        let history: Vec<Value> = opened.history.iter().map(|e| serde_json::to_value(e).unwrap()).collect();
        started(&history, "and a second") && said(&history, "ok: and a second")
    });
    assert_eq!(queued(&composer, cx), 0);
}
