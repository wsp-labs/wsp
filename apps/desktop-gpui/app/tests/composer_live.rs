// SPDX-License-Identifier: AGPL-3.0-only
//! The composer tied by `link::Link` to a thread on a host started for each test alone (host-client's tests/common),
//! with a stand-in claude first on its PATH that answers a prompt at once, or holds a SLOW one open until it is
//! interrupted and echoes each message steered into it. Every test checks what the host's own transcript says
//! happened, not only what the composer believes.
//!
//! The project's file list and the conversations kept outside wsp come from a wsp-daemon, which a checkout does not
//! build: the host runs from a copy of its bundle carrying the one `common::wsp_daemon` finds, and without one those
//! checks are skipped with a line saying so.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::screens::composer::link::{self, Link, Target};
use desktop_gpui::screens::composer::model::ItemKind;
use desktop_gpui::screens::composer::{self, Banner, Composer};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{AppContext, Entity, Focusable, TestAppContext, VisualTestContext};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::{protocol, Failure, HostClient};

const WAIT: Duration = Duration::from_secs(30);

/// `claude seed <prompt>` writes a conversation Claude Code kept in the folder it runs in, as a person's terminal
/// session leaves one, and prints its session id.
const STAND_IN: &str = r#"#!/usr/bin/env node
const argv = process.argv.slice(2);
const at = flag => (argv.indexOf(flag) >= 0 ? argv[argv.indexOf(flag) + 1] : undefined);
const sid = at("--session-id") ?? at("--resume");
const say = o => process.stdout.write(`${JSON.stringify(o)}\n`);
if (argv[0] === "seed") {
  const fs = require("node:fs");
  const id = require("node:crypto").randomUUID();
  const cwd = process.cwd();
  const dir = `${process.env.HOME}/.claude/projects/${cwd.replace(/[^A-Za-z0-9]/g, "-")}`;
  const line = o => JSON.stringify({ parentUuid: null, isSidechain: false, cwd, sessionId: id, version: "2.1.295", entrypoint: "cli", timestamp: new Date().toISOString(), ...o });
  const reply = { id: "msg_1", type: "message", role: "assistant", model: "claude-haiku-5-5", content: [{ type: "text", text: `ok: ${argv[1]}` }] };
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(`${dir}/${id}.jsonl`, `${line({ type: "user", uuid: "u1", message: { role: "user", content: argv[1] } })}\n${line({ type: "assistant", uuid: "a1", parentUuid: "u1", message: reply })}\n`);
  console.log(id);
  process.exit(0);
}
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
        if (prompt.includes("PLAN")) {
          const id = ++n;
          const todos = [{ content: "Write the test", status: "in_progress", activeForm: "Writing the test" }, { content: "Run it", status: "pending", activeForm: "Running it" }];
          say({ type: "assistant", message: { id: `msg_${id}`, type: "message", role: "assistant", model: "claude-haiku-5-5", content: [{ type: "tool_use", id: `toolu_${id}`, name: "TodoWrite", input: { todos } }], stop_reason: null, usage }, parent_tool_use_id: null, session_id: sid, uuid: `a_${id}` });
          waiting = true;
        } else if (prompt.includes("SLOW")) waiting = true;
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

struct Host {
    client: Arc<HostClient>,
    runtime: Arc<Runtime>,
    target: Target,
    /// The project's folder.
    folder: PathBuf,
    server: Throwaway,
}

impl Host {
    /// A host on a fresh state file with a project `repo` holding notes/plan.md, and a thread whose first turn ran.
    fn start() -> Host {
        let server = Throwaway::start_with(STAND_IN, true);
        let folder = server.repo_with("repo", &[("notes/plan.md", "# plan\n")]);
        server.wsp(&["add", &folder.to_string_lossy()]);
        server.wsp(&["run", "repo", "first", "--agent", "claude"]);

        let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
        let client = runtime.block_on(server.client());
        let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
        let row = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread wsp run opened");
        let target = Target { workspace_id: row.workspace_id.clone(), thread_id: row.thread_id.clone() };
        Host { client: Arc::new(client), runtime, target, folder, server }
    }

    /// The host as a screen holds it.
    fn shell(&self) -> desktop_gpui::screens::Host {
        desktop_gpui::screens::Host::new(self.client.clone(), self.runtime.clone())
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
        let rows = self.runtime.block_on(link::rows_of(&self.shell(), &self.target.workspace_id, self.thread())).unwrap();
        let session = link::session_id(&rows, None).expect("a row to interrupt");
        self.interrupt_session(&session).unwrap_or_else(|f| panic!("sessions.interrupt: {}", f.message));
    }

    fn interrupt_session(&self, session: &str) -> Result<(), Failure> {
        let op: protocol::SessionsInterruptRequest = serde_json::from_value(json!({ "sessionId": session })).unwrap();
        let client = self.client.clone();
        self.runtime.block_on(async move { client.call(op).await }).map(|_| ())
    }

    /// A conversation Claude Code kept in the project's folder outside wsp, written by the stand-in: its session id.
    fn seed_conversation(&self, title_prompt: &str) -> String {
        let out = Command::new(self.server.dir.path().join("bin/claude"))
            .args(["seed", title_prompt])
            .env_clear()
            .envs(self.server.env())
            .current_dir(&self.folder)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(out.status.success(), "claude seed: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
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
    let opened = host.runtime.block_on(Link::read(&host.shell(), &target)).unwrap_or_else(|f| panic!("{}", f.message));
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let composer = cx.new(|cx| Composer::new(window, cx));
        made = Some(composer.clone());
        Root::new(composer, window, cx)
    });
    let composer = made.unwrap();
    let link = cx.update(|window, cx| {
        let composer = composer.clone();
        cx.new(|cx| Link::new(host.shell(), target, opened, composer, window, cx))
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
    let opened = host.runtime.block_on(Link::read(&host.shell(), &host.target)).unwrap_or_else(|f| panic!("{}", f.message));
    let harnesses: Vec<&str> = opened.agents.iter().map(|a| a.harness.as_str()).collect();
    assert!(harnesses.contains(&"claude"), "agents {harnesses:?}");
    assert!(opened.agents.iter().any(|a| a.harness == "claude" && a.steers), "claude reads as an agent that steers");
    assert!(!opened.rows.is_empty(), "no rows for the thread");
    assert!(opened.rows.iter().all(|r| r.thread_id.as_deref() == Some(host.thread()) && r.harness == "claude"), "{:?}", opened.rows);
    let history: Vec<Value> = opened.history.iter().map(|e| serde_json::to_value(e).unwrap()).collect();
    assert!(started(&history, "first") && said(&history, "ok: first") && ended(&history, "first"), "history {:?}", kinds(&history));
    let rows = host.runtime.block_on(link::rows_of(&host.shell(), &host.target.workspace_id, host.thread())).unwrap();
    assert_eq!(rows.iter().map(|r| &r.id).collect::<Vec<_>>(), opened.rows.iter().map(|r| &r.id).collect::<Vec<_>>());
    println!("agents {harnesses:?}, {} rows, history {:?}", opened.rows.len(), kinds(&history));

    if host.server.daemon.is_none() {
        println!("skipped the file list, which the host serves through a wsp-daemon: {}", common::wsp_daemon().unwrap_err());
        return;
    }
    let cwd = opened.rows.last().and_then(|r| r.cwd.clone());
    let files = host
        .runtime
        .block_on(link::list_files(&host.shell(), &host.target.workspace_id, cwd))
        .unwrap_or_else(|f| panic!("fs.files: {}", f.message));
    assert!(files.iter().any(|f| f == "notes/plan.md"), "files {files:?}");

    let (composer, _link, cx) = tie(&host, cx);
    composer.update_in(cx, |c, window, cx| c.focus_handle(cx).focus(window, cx));
    cx.run_until_parked();
    cx.simulate_input("@pla");
    until(cx, "the mention menu listing notes/plan.md off the host", |cx| {
        composer.read_with(cx, |c, _| {
            c.menu_items().is_some_and(|items| items.iter().any(|i| i.kind == ItemKind::File && i.insert == "@notes/plan.md"))
        })
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

#[gpui_kit::test]
fn the_screen_on_a_workspace_route_opens_a_new_thread_there(cx: &mut TestAppContext) {
    use desktop_gpui::screens::composer::screen::ComposerScreen;
    use desktop_gpui::screens::{self, Opened};
    let host = Host::start();
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        composer::init(cx);
    });
    let entry = screens::screen("composer").expect("the composer screen is registered");
    let opened = Opened { arg: Some(host.target.workspace_id.clone().into()), ..Opened::default() };
    let mut mounted = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = (entry.mount)(&host.shell(), opened, window, cx);
        let view = screen.view.clone();
        mounted = Some(screen);
        Root::new(view, window, cx)
    });
    let mounted = mounted.unwrap();
    let Ok(screen) = mounted.view.clone().downcast::<ComposerScreen>() else { panic!("the view is not the composer screen") };
    let composer = cx.read(|cx| screen.read(cx).composer().clone());
    until(cx, "the screen tied to the host", |cx| composer.read_with(cx, |c, _| c.hold().is_none()));
    assert!(composer.read_with(cx, |c, _| c.fresh()), "the first thread's turn is not this composer's");
    send(&composer, "a thread from New thread", cx);
    until(cx, "the host listing a new thread whose first turn is the send", |_| {
        let threads = host.runtime.block_on(host.client.threads()).unwrap();
        threads.iter().any(|t| t.prompt.as_deref() == Some("a thread from New thread") && t.thread_id != host.target.thread_id)
    });

    // New thread again while the page is open: the frame shows the page again rather than building another.
    let again = Opened { arg: Some(host.target.workspace_id.clone().into()), ..Opened::default() };
    cx.update(|window, cx| (mounted.show)(again, window, cx));
    let composer = cx.read(|cx| screen.read(cx).composer().clone());
    until(cx, "the screen tied again", |cx| composer.read_with(cx, |c, _| c.hold().is_none()));
    assert!(composer.read_with(cx, |c, _| c.fresh()), "the second new thread starts empty");
    send(&composer, "a second new thread", cx);
    until(cx, "a thread of its own for the second send", |_| {
        let threads = host.runtime.block_on(host.client.threads()).unwrap();
        threads.iter().any(|t| t.prompt.as_deref() == Some("a second new thread"))
    });
}

#[test]
fn a_conversation_kept_outside_wsp_is_listed() {
    let host = Host::start();
    if host.server.daemon.is_none() {
        println!("skipped the conversation list, which the host reads through a wsp-daemon: {}", common::wsp_daemon().unwrap_err());
        return;
    }
    let id = host.seed_conversation("pick this up later");
    let list: protocol::ConversationsListRequest = common::ask(json!({ "project": "repo", "agent": "claude" }));
    let client = host.client.clone();
    let answer =
        host.runtime.block_on(async move { client.call(list).await }).unwrap_or_else(|f| panic!("conversations.list: {}", f.message));
    let rows: Vec<Value> = answer.rows.iter().map(|r| serde_json::to_value(r).unwrap()).collect();
    let row = rows.iter().find(|r| r["id"] == id.as_str()).unwrap_or_else(|| panic!("{id} not among {rows:?}, held {:?}", answer.held));
    assert_eq!(row["title"], "pick this up later");
    assert_eq!(row["origin"], "terminal");
    assert_eq!(row["cwd"], host.folder.to_string_lossy().as_ref());
    println!("conversations.list: {row}");
}

/// The project's skill, in the folder Claude Code reads a person's skills from.
fn seed_skill(host: &Host) {
    let home = host.server.env().into_iter().find(|(k, _)| k == "HOME").map(|(_, v)| PathBuf::from(v)).expect("the host's HOME");
    let dir = home.join(".claude/skills/tidy-imports");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), "---\nname: tidy-imports\ndescription: Sort the imports\n---\nSort them.\n").unwrap();
}

#[gpui_kit::test]
fn the_skills_agents_read_reports_reach_the_dollar_menu(cx: &mut TestAppContext) {
    let host = Host::start();
    seed_skill(&host);
    let (composer, _link, cx) = tie(&host, cx);
    composer.update_in(cx, |c, window, cx| c.set_text("$tid", window, cx));
    until(cx, "the $ menu listing the skill agents.read reports", |cx| {
        composer.update_in(cx, |c, window, cx| c.set_text("$tid", window, cx));
        composer.read_with(cx, |c, _| c.menu_items().is_some_and(|items| items.iter().any(|i| i.label == "$tidy-imports")))
    });
}

#[gpui_kit::test]
fn the_hash_menu_says_why_a_github_repository_lists_nothing(cx: &mut TestAppContext) {
    let host = Host::start();
    if host.server.daemon.is_none() {
        println!("skipped the # menu, which the host serves through a wsp-daemon: {}", common::wsp_daemon().unwrap_err());
        return;
    }
    // A GitHub remote with no signed-in `gh` on the host's PATH: the daemon names the host it has no command line for.
    let added = Command::new("git")
        .args(["remote", "add", "origin", "https://github.com/example/repo.git"])
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", host.server.dir.path())
        .current_dir(&host.folder)
        .status()
        .unwrap();
    assert!(added.success());
    let (composer, _link, cx) = tie(&host, cx);
    composer.update_in(cx, |c, window, cx| c.set_text("#", window, cx));
    assert_eq!(composer.read_with(cx, |c, _| c.items_note().map(str::to_owned)).as_deref(), Some("Loading pull requests and issues"));
    until(cx, "the # menu's note from git.prList in place of the loading line", |cx| {
        composer.read_with(cx, |c, _| c.items_note().is_some_and(|n| n != "Loading pull requests and issues"))
    });
    let note = composer.read_with(cx, |c, _| c.items_note().map(str::to_owned)).unwrap();
    assert!(note.starts_with("No signed-in command line for github.com is on "), "{note:?}");
    assert!(note.ends_with(", so its pull requests and issues are not listed"), "{note:?}");
}

#[gpui_kit::test]
fn up_recalls_what_the_thread_sent(cx: &mut TestAppContext) {
    let host = Host::start();
    let (composer, _link, cx) = tie(&host, cx);
    send(&composer, "second ask", cx);
    until(cx, "the second ask's turn in the transcript", |_| ended(&host.history(), "second ask"));
    composer.update_in(cx, |c, window, cx| c.editor().focus_handle(cx).focus(window, cx));
    until(cx, "Up recalling the newest prompt the host holds", |cx| {
        cx.simulate_keystrokes("up");
        let text = composer.read_with(cx, |c, cx| c.text(cx));
        if text != "second ask" {
            composer.update_in(cx, |c, window, cx| c.set_text("", window, cx));
        }
        text == "second ask"
    });
    cx.simulate_keystrokes("up");
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "first", "the thread's first turn, read from its history");
}

#[gpui_kit::test]
fn the_tasks_row_follows_the_running_turns_plan(cx: &mut TestAppContext) {
    let host = Host::start();
    let (composer, _link, cx) = tie(&host, cx);
    slow_turn(&host, &composer, "PLAN the work", cx);
    until(cx, "the drawer's tasks row on the step the turn is on", |cx| {
        composer.read_with(cx, |c, _| c.drawer().tasks == Some(("Write the test".to_owned(), "0/2".to_owned())))
    });
    host.interrupt();
    until(cx, "the tasks row gone with the turn", |cx| composer.read_with(cx, |c, _| c.drawer().tasks.is_none() && !c.running()));
}

#[gpui_kit::test]
fn a_send_under_the_resume_banner_continues_the_conversation(cx: &mut TestAppContext) {
    let host = Host::start();
    if host.server.daemon.is_none() {
        println!("skipped the resume, whose conversation the host lists through a wsp-daemon: {}", common::wsp_daemon().unwrap_err());
        return;
    }
    let id = host.seed_conversation("pick this up later");
    let target = Target { workspace_id: host.target.workspace_id.clone(), thread_id: None };
    let (composer, _link, cx) = tie_to(&host, target, cx);
    composer.update(cx, |c, cx| c.set_banner(Some(Banner::resume(id.clone(), "pick this up later", false, "Claude Code")), cx));
    send(&composer, "carry on", cx);
    until(cx, "a thread whose turn resumed the conversation's session", |_| {
        let threads = host.runtime.block_on(host.client.threads()).unwrap();
        threads.iter().any(|t| t.prompt.as_deref() == Some("carry on") && t.claude_session_id.as_deref() == Some(id.as_str()))
    });
    until(cx, "the banner gone once the host took the send", |cx| composer.read_with(cx, |c, _| c.banner().is_none()));
}
