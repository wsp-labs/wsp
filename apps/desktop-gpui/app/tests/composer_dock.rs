// SPDX-License-Identifier: AGPL-3.0-only
//! The thread's own prompt in the composer's place, through the thread screen as the frame mounts it, against a host
//! started for each test alone whose stand-in claude asks before it runs a command and says what it was answered.
//! Also Fork from here, handed from the screen to the composer, and a subagent's prompt, which keeps its card.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::screens::composer::dock::Prompt;
use desktop_gpui::screens::composer::model::Hold;
use desktop_gpui::screens::composer::{self, Composer};
use desktop_gpui::screens::thread_view::ThreadScreen;
use desktop_gpui::screens::{self, Mounted, Opened, THREAD};
use desktop_gpui::thread_view::{ThreadView, ThreadViewEvent};
use desktop_gpui::transcript::{RowKind, Transcript};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext as _, BorrowAppContext as _, Entity, Focusable as _, TestAppContext, VisualTestContext};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::protocol::SessionEvent;
use wsp_host_client::HostClient;

const WAIT: Duration = Duration::from_secs(40);

/// A prompt holding ASK raises a permission prompt to run `rm -rf build` and waits; the answer the host sends back is
/// said as the reply. One holding LATE starts seven seconds late. Every other prompt is answered at once.
const STAND_IN: &str = r#"#!/usr/bin/env node
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
const init = () => say({ type: "system", subtype: "init", cwd: process.cwd(), session_id: sid, tools: [], mcp_servers: [], model: "claude-haiku-5-5", permissionMode: "default", slash_commands: [], apiKeySource: "none", uuid: `init_${++n}` });
const text = words => {
  const id = ++n;
  say({ type: "assistant", message: { id: `msg_${id}`, type: "message", role: "assistant", model: "claude-haiku-5-5", content: [{ type: "text", text: words }], stop_reason: null, usage }, parent_tool_use_id: null, session_id: sid, uuid: `a_${id}` });
};
const result = words => say({ type: "result", subtype: "success", is_error: false, duration_ms: 1, num_turns: 1, session_id: sid, total_cost_usd: 0, usage, result: words, uuid: `r_${++n}` });
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
    } else if (m.type === "control_response" && m.response?.request_id === "perm-1") {
      const words = `answered: ${JSON.stringify(m.response.response ?? {})}`;
      text(words);
      result(words);
    } else if (m.type === "user") {
      const prompt = (m.message?.content ?? []).filter(c => c.type === "text").map(c => c.text).join("");
      if (prompt.includes("LATE")) {
        setTimeout(() => { init(); text(`ok: ${prompt}`); result(`ok: ${prompt}`); }, 7000);
        continue;
      }
      init();
      if (prompt.includes("ASK")) {
        const id = ++n;
        say({ type: "assistant", message: { id: `msg_${id}`, type: "message", role: "assistant", model: "claude-haiku-5-5", content: [{ type: "tool_use", id: "toolu_ask", name: "Bash", input: { command: "rm -rf build" } }], stop_reason: null, usage }, parent_tool_use_id: null, session_id: sid, uuid: `a_${id}` });
        say({ type: "control_request", request_id: "perm-1", request: { subtype: "can_use_tool", tool_name: "Bash", tool_use_id: "toolu_ask", input: { command: "rm -rf build" } } });
      } else {
        text(`ok: ${prompt}`);
        result(`ok: ${prompt}`);
      }
    }
  }
});
process.stdin.on("end", () => process.exit(0));
"#;

struct Host {
    server: Throwaway,
    client: Arc<HostClient>,
    runtime: Arc<Runtime>,
    workspace: String,
}

impl Host {
    /// A host with a project `repo` and one thread per prompt, each run to its end.
    fn start(prompts: &[&str]) -> Host {
        let server = Throwaway::start_with(STAND_IN, false);
        let folder = server.repo("repo");
        server.wsp(&["add", &folder.to_string_lossy()]);
        for prompt in prompts {
            server.wsp(&["run", "repo", prompt, "--agent", "claude"]);
        }
        let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
        let client = runtime.block_on(server.client());
        let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
        let workspace = threads.first().expect("a thread").workspace_id.clone();
        Host { server, client: Arc::new(client), runtime, workspace }
    }

    fn thread(&self, prompt: &str) -> String {
        let threads = self.runtime.block_on(self.client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
        let row = threads.iter().find(|t| t.prompt.as_deref() == Some(prompt)).unwrap_or_else(|| panic!("no thread {prompt}"));
        row.thread_id.clone().expect("a thread id")
    }

    fn history(&self, thread: &str) -> Vec<Value> {
        let opened = self.runtime.block_on(self.client.open(&self.workspace, thread)).unwrap_or_else(|f| panic!("{}", f.message));
        opened.history.iter().map(|e| serde_json::to_value(e).unwrap()).collect()
    }

    fn shell(&self) -> screens::Host {
        screens::Host::new(self.client.clone(), self.runtime.clone())
    }
}

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + WAIT;
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The thread screen as the frame mounts it, on `thread`.
fn mount<'a>(host: &Host, thread: &str, cx: &'a mut TestAppContext) -> (Mounted, Entity<ThreadScreen>, &'a mut VisualTestContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        composer::init(cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
    });
    let entry = screens::screen(THREAD).expect("the thread screen is registered");
    let opened = Opened { workspace_id: Some(host.workspace.clone()), thread_id: Some(thread.to_owned()), ..Opened::default() };
    let mut mounted = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = (entry.mount)(&host.shell(), opened, window, cx);
        let view = screen.view.clone();
        mounted = Some(screen);
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(1100.), px(800.)));
    let mounted = mounted.unwrap();
    let Ok(screen) = mounted.view.clone().downcast::<ThreadScreen>() else { panic!("the view is not the thread screen") };
    (mounted, screen, cx)
}

fn parts(screen: &Entity<ThreadScreen>, cx: &mut VisualTestContext) -> (Entity<ThreadView>, Entity<Composer>) {
    screen.read_with(cx, |s, _| (s.view().clone(), s.composer().clone()))
}

fn show(mounted: &Mounted, host: &Host, thread: &str, cx: &mut VisualTestContext) {
    let opened = Opened { workspace_id: Some(host.workspace.clone()), thread_id: Some(thread.to_owned()), ..Opened::default() };
    cx.update(|window, cx| (mounted.show)(opened, window, cx));
}

fn linked(composer: &Entity<Composer>, cx: &mut VisualTestContext) -> bool {
    composer.read_with(cx, |c, _| c.hold() != Some(&Hold::Connecting))
}

/// Sends ASK on the screen's thread and waits for its prompt to stand in the composer's place.
fn ask(screen: &Entity<ThreadScreen>, cx: &mut VisualTestContext) {
    let (_, composer) = parts(screen, cx);
    until(cx, "the composer's link", |cx| linked(&composer, cx));
    composer.update_in(cx, |c, window, cx| {
        c.set_text("ASK to clean", window, cx);
        c.submit(window, cx);
    });
    until(cx, "the prompt docked in the composer's place", |cx| {
        composer.read_with(cx, |c, _| c.dock().is_some_and(|d| !d.folded)) && cx.debug_bounds("prompt-dock").is_some()
    });
}

#[gpui_kit::test]
fn the_threads_own_prompt_stands_in_the_composers_place_and_its_row_waits(cx: &mut TestAppContext) {
    let host = Host::start(&["first"]);
    let thread = host.thread("first");
    let (_mounted, screen, cx) = mount(&host, &thread, cx);
    ask(&screen, cx);
    let (view, composer) = parts(&screen, cx);
    let dock = composer.read_with(cx, |c, _| c.dock().cloned()).unwrap();
    assert_eq!(dock.prompt.title, "Run a command");
    assert_eq!(dock.prompt.code.as_deref(), Some("rm -rf build"));
    assert_eq!(dock.prompt.options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>(), ["Allow", "Deny"]);
    assert!(cx.debug_bounds("copy-row").is_some(), "the command is a copy row");
    assert!(cx.debug_bounds("composer-editor").is_none(), "the dock stands in the box's place");
    assert!(cx.update(|window, cx| composer.read(cx).dock_focused(window, cx)), "a new ask takes the keys");
    let row = view.read_with(cx, |v, _| v.rows().iter().find(|r| matches!(r.kind, RowKind::Permission { .. })).cloned()).unwrap();
    assert!(cx.debug_bounds("record").is_some(), "the transcript keeps the waiting record of {:?}", row.text);
    assert!(cx.debug_bounds("options").is_none(), "and no buttons of its own");
}

#[gpui_kit::test]
fn deny_with_its_words_reaches_the_agent_through_sessions_answer(cx: &mut TestAppContext) {
    let host = Host::start(&["first"]);
    let thread = host.thread("first");
    let (_mounted, screen, cx) = mount(&host, &thread, cx);
    ask(&screen, cx);
    let (_, composer) = parts(&screen, cx);
    cx.simulate_keystrokes("2");
    assert_eq!(composer.read_with(cx, |c, _| c.dock().map(|d| d.primary_word())), Some("Deny"), "2 picks Deny");
    cx.simulate_keystrokes("2");
    cx.run_until_parked();
    cx.simulate_input("build it with make instead");
    cx.simulate_keystrokes("enter");
    let mut said = String::new();
    until(cx, "the stand-in saying the deny it was handed", |_| {
        let history = host.history(&thread);
        said = history
            .iter()
            .filter(|e| e["type"] == "session.delta")
            .filter_map(|e| e["text"].as_str())
            .find(|t| t.starts_with("answered:"))
            .unwrap_or_default()
            .to_owned();
        !said.is_empty()
    });
    assert!(said.contains("\"behavior\":\"deny\""), "{said}");
    assert!(said.contains("build it with make instead"), "the reason reached the agent: {said}");
    until(cx, "the dock gone once the host closed the prompt", |cx| composer.read_with(cx, |c, _| c.dock().is_none()));
    assert!(cx.debug_bounds("composer-editor").is_some(), "the box is back");
}

#[gpui_kit::test]
fn the_dock_takes_digits_and_arrows_folds_on_escape_and_opens_from_the_drawer(cx: &mut TestAppContext) {
    let host = Host::start(&["first"]);
    let thread = host.thread("first");
    let (_mounted, screen, cx) = mount(&host, &thread, cx);
    ask(&screen, cx);
    let (_, composer) = parts(&screen, cx);
    let picked = |cx: &mut VisualTestContext| composer.read_with(cx, |c, _| c.dock().map(|d| d.primary_word()));
    assert_eq!(picked(cx), Some("Allow"), "the first row is picked from the start");
    cx.simulate_keystrokes("down");
    assert_eq!(picked(cx), Some("Deny"));
    cx.simulate_keystrokes("down");
    assert_eq!(picked(cx), Some("Allow"), "the arrows go round");
    cx.simulate_keystrokes("2 1");
    assert_eq!(picked(cx), Some("Allow"));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(composer.read_with(cx, |c, _| c.dock().is_some_and(|d| d.folded)), "Esc folds the dock");
    assert!(cx.debug_bounds("composer-editor").is_some(), "the box is back under the folded prompt");
    let row = cx.debug_bounds("drawer-question").expect("the drawer's question row");
    cx.simulate_click(row.center(), gpui_kit::Modifiers::none());
    cx.run_until_parked();
    assert!(composer.read_with(cx, |c, _| c.dock().is_some_and(|d| !d.folded)), "the drawer's row opens the dock again");
    assert!(cx.debug_bounds("prompt-dock").is_some());
    cx.simulate_keystrokes("enter");
    until(cx, "the allowed prompt closed", |cx| composer.read_with(cx, |c, _| c.dock().is_none()));
    let history = host.history(&thread);
    assert!(history.iter().any(|e| e["type"] == "session.permission.closed" && e["outcome"] == "allowed"));
}

#[gpui_kit::test]
fn a_letter_typed_on_the_dock_folds_it_and_starts_a_message(cx: &mut TestAppContext) {
    let host = Host::start(&["first"]);
    let thread = host.thread("first");
    let (_mounted, screen, cx) = mount(&host, &thread, cx);
    ask(&screen, cx);
    let (_, composer) = parts(&screen, cx);
    cx.simulate_keystrokes("w");
    cx.run_until_parked();
    assert!(composer.read_with(cx, |c, _| c.dock().is_some_and(|d| d.folded)));
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "w");
}

#[gpui_kit::test]
fn fork_from_here_opens_the_fork_draft_and_its_send_forks_the_thread(cx: &mut TestAppContext) {
    let host = Host::start(&["first", "second"]);
    let (first, second) = (host.thread("first"), host.thread("second"));
    let (mounted, screen, cx) = mount(&host, &first, cx);
    let (view, composer) = parts(&screen, cx);
    until(cx, "the first thread linked", |cx| linked(&composer, cx) && view.read_with(cx, |v, _| v.loaded()));

    // A press drawn for the first thread lands after a switch to the second: nothing forks.
    show(&mounted, &host, &second, cx);
    until(cx, "the second thread linked", |cx| {
        linked(&composer, cx) && composer.read_with(cx, |c, _| c.thread_key() == Some(second.as_str()))
    });
    view.update(cx, |_, cx| cx.emit(ThreadViewEvent::Fork { thread: first.clone(), turn: None }));
    cx.run_until_parked();
    assert_eq!(composer.read_with(cx, |c, _| c.forking().cloned()), None, "a stale press forks nothing");
    assert!(composer.read_with(cx, |c, _| c.banner().is_none()));

    view.update(cx, |_, cx| cx.emit(ThreadViewEvent::Fork { thread: second.clone(), turn: None }));
    cx.run_until_parked();
    let banner = composer.read_with(cx, |c, _| c.banner().cloned()).expect("the fork's banner");
    assert!(banner.fork);
    assert_eq!((banner.line.as_str(), banner.note.as_str()), ("Fork of second", "in this folder"));
    assert!(cx.debug_bounds("composer-banner").is_some());
    composer.update_in(cx, |c, window, cx| {
        c.set_text("a fork of the second", window, cx);
        c.submit(window, cx);
    });
    let mut forked = None;
    until(cx, "a thread forked from the second", |_| {
        let threads = host.runtime.block_on(host.client.threads()).unwrap();
        forked = threads.into_iter().find(|t| t.prompt.as_deref() == Some("a fork of the second"));
        forked.is_some()
    });
    let forked = forked.unwrap();
    assert_ne!(forked.thread_id.as_deref(), Some(second.as_str()), "the fork is a thread of its own");
    let from = serde_json::to_value(&forked.forked_from).unwrap();
    assert_eq!(from["threadId"], json!(second), "{from}");
    until(cx, "the draft spent", |cx| composer.read_with(cx, |c, _| c.forking().is_none() && c.banner().is_none()));
    assert!(!host.history(&second).iter().any(|e| e["prompt"] == "a fork of the second"), "the send was no turn of the second");
}

fn event(value: Value) -> SessionEvent {
    serde_json::from_value(value).unwrap()
}

#[test]
fn a_subagents_prompt_keeps_its_card_and_the_threads_own_docks() {
    let base = |kind: &str, pos: u64, more: Value| {
        let mut e = json!({ "type": kind, "workspaceId": "w", "sessionId": "s", "turnId": "t1", "threadId": "th", "pos": pos, "at": 1.0 });
        e.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
        event(e)
    };
    let options = json!([{ "id": "allow", "label": "Allow", "effect": "allow" }, { "id": "deny", "label": "Deny", "effect": "deny" }]);
    let mut transcript = Transcript::new();
    transcript.load(vec![
        base("session.start", 1, json!({ "prompt": "go", "agent": "claude" })),
        base(
            "session.permission",
            3,
            json!({ "askId": "sub", "toolName": "Bash", "toolUseId": "b1", "parentToolUseId": "task-1", "input": "{\"command\":\"ls\"}", "options": options }),
        ),
    ]);
    assert!(transcript.docked().is_none(), "a subagent's prompt is answered from its card");
    transcript.push(base(
        "session.permission",
        4,
        json!({ "askId": "own", "toolName": "Bash", "toolUseId": "b2", "input": "{\"command\":\"rm -rf build\",\"cwd\":\"/elsewhere\"}", "options": options }),
    ));
    let docked = transcript.docked().expect("the thread's own prompt docks");
    let prompt = Prompt::of(&docked, Some("claude".into())).unwrap();
    assert_eq!((prompt.ask.as_str(), prompt.code.as_deref(), prompt.runs_in.as_deref()), ("own", Some("rm -rf build"), Some("/elsewhere")));
    assert_eq!(prompt.options.iter().map(|o| o.effect.as_str()).collect::<Vec<_>>(), ["allow", "deny"]);
}

#[gpui_kit::test]
fn a_send_whose_agent_is_slow_to_start_says_so_until_its_turn_starts(cx: &mut TestAppContext) {
    let host = Host::start(&["first"]);
    let thread = host.thread("first");
    let (_mounted, screen, cx) = mount(&host, &thread, cx);
    let (view, composer) = parts(&screen, cx);
    until(cx, "the composer's link", |cx| linked(&composer, cx) && view.read_with(cx, |v, _| v.loaded()));
    composer.update_in(cx, |c, window, cx| {
        c.set_text("LATE to start", window, cx);
        c.submit(window, cx);
    });
    let starting =
        |cx: &mut VisualTestContext| view.read_with(cx, |v, _| v.rows().iter().find(|r| r.key == "starting").map(|r| r.text.clone()));
    until(cx, "the starting line under the send", |cx| starting(cx).is_some());
    let line = starting(cx).unwrap();
    assert_eq!(line, "Starting Claude Code");
    until(cx, "the line gone once the turn started", |cx| starting(cx).is_none());
    assert!(host.history(&thread).iter().any(|e| e["type"] == "session.start" && e["prompt"] == "LATE to start"));
}

#[gpui_kit::test]
fn add_to_chat_writes_into_the_open_threads_box_and_after_another_threads_draft(cx: &mut TestAppContext) {
    use desktop_gpui::local_store::{self, LocalStore};
    use desktop_gpui::screens::composer::store;
    use desktop_gpui::screens::Route;
    use desktop_gpui::shell::{Launch, Shell};
    let host = Host::start(&["first", "second"]);
    let (first, second) = (host.thread("first"), host.thread("second"));
    let kept = tempfile::tempdir().unwrap();
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
        local_store::init(kept.path().join("local-store.json"), cx);
        cx.update_global(|s: &mut LocalStore, _| store::set_draft(s, &second, "half a thought", 14)).unwrap();
    });
    let (state, vars, runtime) = (host.server.state(), host.server.env(), host.runtime.clone());
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, vars, runtime, Launch::default(), window, cx));
        made = Some(view.clone());
        Root::new(view, window, cx)
    });
    let shell = made.unwrap();
    cx.simulate_resize(size(px(1280.), px(800.)));
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    let route = Route::Thread { workspace_id: host.workspace.clone(), thread_id: first.clone() };
    shell.update_in(cx, |s, window, cx| s.go(route, window, cx));
    until(cx, "the first thread's composer", |cx| {
        shell.read_with(cx, |s, cx| s.thread_screen().is_some_and(|t| t.read(cx).composer().read(cx).holds(&first)))
    });
    let composer = shell.read_with(cx, |s, cx| s.thread_screen().unwrap().read(cx).composer().clone());
    composer.update_in(cx, |c, window, cx| c.set_text("look at this", window, cx));

    cx.update(|_, cx| composer::add_to_chat(&first, "$ cargo test\nok", cx));
    cx.run_until_parked();
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "look at this\n$ cargo test\nok\n", "on lines of its own at the caret");
    assert!(cx.update(|window, cx| composer.read(cx).editor().read(cx).focus_handle(cx).is_focused(window)), "the box has the keys");

    cx.update(|_, cx| composer::add_to_chat(&second, "$ ls\nsrc", cx));
    cx.run_until_parked();
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "look at this\n$ cargo test\nok\n", "the open box is left as it was");
    let draft = cx.update(|_, cx| store::draft(cx.global::<LocalStore>(), &second).map(|(d, _)| d));
    assert_eq!(draft.as_deref(), Some("half a thought\n$ ls\nsrc\n"), "the other thread's draft takes it");
    let route = Route::Thread { workspace_id: host.workspace.clone(), thread_id: second.clone() };
    shell.update_in(cx, |s, window, cx| s.go(route, window, cx));
    until(cx, "the second thread's box with the block", |cx| composer.read_with(cx, |c, cx| c.text(cx) == "half a thought\n$ ls\nsrc\n"));
}

#[gpui_kit::test]
fn a_pick_the_host_will_not_take_is_said_under_the_options(cx: &mut TestAppContext) {
    use desktop_gpui::screens::composer::dock::Answer;
    use desktop_gpui::screens::composer::ComposerEvent;
    let host = Host::start(&["first"]);
    let thread = host.thread("first");
    let (_mounted, screen, cx) = mount(&host, &thread, cx);
    ask(&screen, cx);
    let (_, composer) = parts(&screen, cx);
    let (session, ask) = composer.read_with(cx, |c, _| c.dock().map(|d| (d.prompt.session.clone(), d.prompt.ask.clone()))).unwrap();
    composer.update(cx, |_, cx| cx.emit(ComposerEvent::Answer(Answer { session, ask, option: "nope".into(), reason: None })));
    until(cx, "the host's refusal under the options", |cx| {
        composer.read_with(cx, |c, _| c.dock().and_then(|d| d.refused.clone()).is_some())
    });
    let said = composer.read_with(cx, |c, _| c.dock().and_then(|d| d.refused.clone())).unwrap();
    assert_eq!(said, "The prompt carries no option by that id.");
    assert!(cx.debug_bounds("refusal").is_some(), "the refusal is drawn");
}

#[test]
fn an_edit_prompt_docks_its_file_and_the_lines_that_change() {
    let options = json!([{ "id": "allow", "label": "Allow", "effect": "allow" }, { "id": "deny", "label": "Deny", "effect": "deny" }]);
    let edit = json!({ "file_path": "/repo/src/lib.rs", "old_string": "a\nb", "new_string": "c" }).to_string();
    let event = |kind: &str, pos: u64, more: Value| {
        let mut e = json!({ "type": kind, "workspaceId": "w", "sessionId": "s", "turnId": "t1", "threadId": "th", "pos": pos });
        e.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
        serde_json::from_value::<SessionEvent>(e).unwrap()
    };
    let mut transcript = Transcript::new();
    transcript.load(vec![
        event("session.start", 1, json!({ "prompt": "go", "cwd": "/repo" })),
        event("session.permission", 2, json!({ "askId": "e", "toolName": "Edit", "input": edit, "options": options })),
    ]);
    let prompt = Prompt::of(&transcript.docked().unwrap(), None).unwrap();
    assert_eq!(prompt.title, "Edit lib.rs in src");
    let file = prompt.file.expect("the file card");
    assert_eq!((file.path.as_str(), file.edit), ("/repo/src/lib.rs", Some(("a\nb".to_owned(), "c".to_owned()))));
    assert!(prompt.fields.is_empty(), "the edit's strings are drawn as lines, never as fields");
}

#[gpui_kit::test]
fn a_streamed_line_leaves_the_composer_alone(cx: &mut TestAppContext) {
    let host = Host::start(&["first"]);
    let thread = host.thread("first");
    let (_mounted, screen, cx) = mount(&host, &thread, cx);
    let (view, composer) = parts(&screen, cx);
    until(cx, "the composer's link", |cx| linked(&composer, cx) && view.read_with(cx, |v, _| v.loaded()));
    std::thread::sleep(Duration::from_millis(300));
    cx.run_until_parked();
    let told = std::rc::Rc::new(std::cell::Cell::new(0));
    let count = told.clone();
    let _watch = cx.update(|_, cx| cx.observe(&composer, move |_, _| count.set(count.get() + 1)));
    let last = view.read_with(cx, |v, _| v.rows().len());
    let delta = json!({
        "type": "session.delta", "workspaceId": host.workspace, "threadId": thread, "sessionId": "s", "turnId": "t9",
        "pos": 100_000, "kind": "text", "text": "a streamed line",
    });
    view.update(cx, |v, cx| {
        v.push(serde_json::from_value(delta).unwrap(), cx);
        cx.emit(ThreadViewEvent::Arrived);
    });
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| v.rows().len()) >= last);
    assert_eq!(told.get(), 0, "a streamed line draws the composer nothing new");
}

#[gpui_kit::test]
fn a_fork_answered_after_the_person_left_moves_nothing(cx: &mut TestAppContext) {
    use desktop_gpui::screens::Route;
    use desktop_gpui::shell::{Launch, Shell};
    let host = Host::start(&["first", "second"]);
    let (first, second) = (host.thread("first"), host.thread("second"));
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
    });
    let (state, vars, runtime) = (host.server.state(), host.server.env(), host.runtime.clone());
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, vars, runtime, Launch::default(), window, cx));
        made = Some(view.clone());
        Root::new(view, window, cx)
    });
    let shell = made.unwrap();
    cx.simulate_resize(size(px(1280.), px(800.)));
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    let to = |thread: &str| Route::Thread { workspace_id: host.workspace.clone(), thread_id: thread.to_owned() };
    shell.update_in(cx, |s, window, cx| s.go(to(&second), window, cx));
    until(cx, "the second thread linked", |cx| {
        shell.read_with(cx, |s, cx| {
            s.thread_screen().is_some_and(|t| {
                let c = t.read(cx).composer().read(cx);
                c.holds(&second) && c.hold() != Some(&Hold::Connecting)
            })
        })
    });
    let screen = shell.read_with(cx, |s, _| s.thread_screen()).unwrap();
    let (view, composer) = parts(&screen, cx);
    view.update(cx, |_, cx| cx.emit(ThreadViewEvent::Fork { thread: second.clone(), turn: None }));
    cx.run_until_parked();
    composer.update_in(cx, |c, window, cx| {
        c.set_text("a fork nobody waits for", window, cx);
        c.submit(window, cx);
    });
    shell.update_in(cx, |s, window, cx| s.go(to(&first), window, cx));
    until(cx, "the fork made on the host", |_| {
        let threads = host.runtime.block_on(host.client.threads()).unwrap();
        threads.iter().any(|t| t.prompt.as_deref() == Some("a fork nobody waits for"))
    });
    let end = Instant::now() + Duration::from_millis(1500);
    while Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), to(&first), "the window stays on the thread the person chose");
}
