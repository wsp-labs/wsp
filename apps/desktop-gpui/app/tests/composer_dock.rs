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
use desktop_gpui::screens::{self, Mounted, Opened, Route, THREAD};
use desktop_gpui::shell::{Launch, Shell};
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
/// said as the reply. One holding LATE starts seven seconds late. One holding MANY makes 450 calls with no words, more
/// than two history pages, then waits, or with DONE also ends. Every other prompt is answered at once.
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
      if (prompt.includes("MANY")) {
        for (let i = 0; i < 450; i++) {
          const id = ++n;
          say({ type: "assistant", message: { id: `msg_${id}`, type: "message", role: "assistant", model: "claude-haiku-5-5", content: [{ type: "tool_use", id: `toolu_${id}`, name: "Bash", input: { command: "true" } }], stop_reason: null, usage }, parent_tool_use_id: null, session_id: sid, uuid: `a_${id}` });
        }
        if (prompt.includes("DONE")) result("done");
      } else if (prompt.includes("ASK")) {
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
    composer.read_with(cx, |c, _| !matches!(c.hold(), Some(Hold::Connecting | Hold::HostDown)))
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
    cx.run_until_parked();
    // Deny's field takes the keys as it opens, so the words go to it and not to a message.
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
    let folded = |cx: &mut VisualTestContext| composer.read_with(cx, |c, _| c.dock().map(|d| d.folded));
    assert_eq!(picked(cx), Some("Allow"), "the first row is picked from the start");
    cx.simulate_keystrokes("down");
    assert_eq!(picked(cx), Some("Deny"));
    cx.simulate_input("not now");
    assert_eq!(folded(cx), Some(false), "Deny's field takes the words as it opens");
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "", "and the box none");
    cx.simulate_keystrokes("escape");
    assert_eq!(picked(cx), Some("Allow"), "Esc in the field goes back to the first row");
    cx.simulate_keystrokes("up");
    assert_eq!(picked(cx), Some("Deny"), "the arrows go round");
    cx.simulate_keystrokes("escape 1");
    assert_eq!(picked(cx), Some("Allow"));
    composer.update_in(cx, |c, window, cx| c.type_key("2", window, cx));
    assert_eq!((picked(cx), folded(cx)), (Some("Deny"), Some(false)), "a digit typed in the transcript is the dock's");
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "");
    cx.simulate_keystrokes("escape escape");
    cx.run_until_parked();
    assert_eq!(folded(cx), Some(true), "Esc folds the dock");
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
fn a_folded_prompt_stays_folded_when_its_thread_shows_again(cx: &mut TestAppContext) {
    let host = Host::start(&["first", "second"]);
    let (first, second) = (host.thread("first"), host.thread("second"));
    let (mounted, screen, cx) = mount(&host, &first, cx);
    ask(&screen, cx);
    let (view, composer) = parts(&screen, cx);
    cx.simulate_keystrokes("escape");
    let shown = |thread: &str, cx: &mut VisualTestContext| {
        linked(&composer, cx) && composer.read_with(cx, |c, _| c.thread_key() == Some(thread)) && view.read_with(cx, |v, _| v.loaded())
    };
    show(&mounted, &host, &second, cx);
    until(cx, "the second thread", |cx| shown(&second, cx));
    show(&mounted, &host, &first, cx);
    until(cx, "the first thread's prompt again", |cx| shown(&first, cx) && composer.read_with(cx, |c, _| c.dock().is_some()));
    assert_eq!(composer.read_with(cx, |c, _| c.dock().map(|d| d.folded)), Some(true), "the person's fold stands");
    assert!(cx.debug_bounds("drawer-question").is_some(), "under the drawer's question row");
}

#[gpui_kit::test]
fn a_quote_under_the_dock_joins_the_draft_and_the_dock_keeps_its_keys(cx: &mut TestAppContext) {
    let host = Host::start(&["first"]);
    let thread = host.thread("first");
    let (_mounted, screen, cx) = mount(&host, &thread, cx);
    ask(&screen, cx);
    let (view, composer) = parts(&screen, cx);
    view.update(cx, |_, cx| cx.emit(ThreadViewEvent::Quote("> a line".into())));
    cx.run_until_parked();
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "> a line\n", "the quote joins the draft");
    assert!(cx.update(|window, cx| composer.read(cx).dock_focused(window, cx)), "the dock keeps the keys");
    cx.simulate_keystrokes("escape");
    assert_eq!(composer.read_with(cx, |c, _| c.dock().map(|d| d.folded)), Some(true), "so Esc still folds it");
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

/// The window as the app opens it, on `host`, once connected.
fn framed<'a>(host: &Host, cx: &'a mut TestAppContext) -> (Entity<Shell>, &'a mut VisualTestContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        composer::init(cx);
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
    (shell, cx)
}

fn thread_route(host: &Host, thread: &str) -> Route {
    Route::Thread { workspace_id: host.workspace.clone(), thread_id: thread.to_owned() }
}

/// Opens `thread` in the window and waits for its composer's link.
fn open_thread(shell: &Entity<Shell>, host: &Host, thread: &str, cx: &mut VisualTestContext) -> Entity<ThreadScreen> {
    shell.update_in(cx, |s, window, cx| s.go(thread_route(host, thread), window, cx));
    until(cx, "the thread linked", |cx| {
        shell.read_with(cx, |s, cx| {
            s.thread_screen().is_some_and(|t| {
                let c = t.read(cx).composer().read(cx);
                c.holds(thread) && c.hold().is_none() && t.read(cx).view().read(cx).loaded()
            })
        })
    });
    shell.read_with(cx, |s, _| s.thread_screen()).unwrap()
}

/// The New thread page's composer on the host's workspace, once Fork from here opened it there and its link stands.
fn fork_page(shell: &Entity<Shell>, host: &Host, cx: &mut VisualTestContext) -> Entity<Composer> {
    let page = Route::Screen(format!("composer:{}", host.workspace).into());
    let mut found = None;
    until(cx, "the fork's draft on the New thread page", |cx| {
        let here = shell.read_with(cx, |s, _| s.route().clone()) == page;
        found = cx.update(|_, cx| composer::open_on(&host.workspace, cx));
        here && found.as_ref().is_some_and(|c| c.read_with(cx, |c, _| c.forking().is_some() && c.hold().is_none()))
    });
    found.unwrap()
}

/// The first person's message of the thread on screen, as Fork from here carries it.
fn persons_message(view: &Entity<ThreadView>, cx: &mut VisualTestContext) -> (String, Vec<desktop_gpui::transcript::FileTile>) {
    view.read_with(cx, |v, _| {
        v.rows()
            .iter()
            .find_map(|r| match &r.kind {
                RowKind::Person { files, .. } => Some((r.text.clone(), files.clone())),
                _ => None,
            })
            .expect("a person's message")
    })
}

#[gpui_kit::test]
fn fork_from_here_opens_its_draft_on_the_new_thread_page_and_its_send_forks_the_thread(cx: &mut TestAppContext) {
    let host = Host::start(&["first", "second"]);
    let (first, second) = (host.thread("first"), host.thread("second"));
    let (shell, cx) = framed(&host, cx);
    let screen = open_thread(&shell, &host, &second, cx);
    let (view, own) = parts(&screen, cx);
    own.update_in(cx, |c, window, cx| c.set_text("my reply to second", window, cx));

    // A press drawn for the first thread lands after the switch to the second: nothing forks.
    view.update(cx, |_, cx| cx.emit(ThreadViewEvent::Fork { thread: first.clone(), turn: None, message: None }));
    cx.run_until_parked();
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), thread_route(&host, &second), "a stale press forks nothing");

    view.update(cx, |_, cx| cx.emit(ThreadViewEvent::Fork { thread: second.clone(), turn: None, message: None }));
    let page = fork_page(&shell, &host, cx);
    let banner = page.read_with(cx, |c, _| c.banner().cloned()).expect("the fork's banner");
    assert!(banner.fork);
    assert_eq!((banner.line.as_str(), banner.note.as_str(), banner.dismiss()), ("Fork of second", "in this folder", "Cancel the fork"));
    assert!(cx.debug_bounds("composer-banner").is_some() && cx.debug_bounds("banner-dismiss").is_some());
    assert_eq!(page.read_with(cx, |c, _| c.picks().agent.clone()).as_deref(), Some("claude"), "the draft carries the source's agent");
    assert_eq!(
        own.read_with(cx, |c, cx| (c.text(cx), c.banner().is_some())),
        ("my reply to second".to_owned(), false),
        "the fork took the source thread's own box"
    );

    page.update_in(cx, |c, window, cx| {
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
    let fork_id = forked.thread_id.clone().unwrap_or_else(|| forked.id.clone());
    assert_ne!(fork_id, second, "the fork is a thread of its own");
    let from = serde_json::to_value(&forked.forked_from).unwrap();
    assert_eq!(from["threadId"], json!(second), "{from}");
    until(cx, "the window on the fork", |cx| shell.read_with(cx, |s, _| s.route().clone()) == thread_route(&host, &fork_id));
    assert!(!host.history(&second).iter().any(|e| e["prompt"] == "a fork of the second"), "the send was no turn of the second");
    let page = Route::Screen(format!("composer:{}", host.workspace).into());
    let drawer = cx.update(|_, cx| desktop_gpui::screens::terminal::drawer_key(&page, cx));
    assert_eq!(
        drawer,
        Some(desktop_gpui::screens::terminal::hub::Key::Thread { workspace_id: host.workspace.clone(), thread_id: fork_id.clone() }),
        "the New thread page's drawer did not go to the fork its send opened"
    );
}

#[gpui_kit::test]
fn fork_from_here_decides_at_the_press_and_never_pulls_back_a_person_who_moved_on(cx: &mut TestAppContext) {
    use desktop_gpui::screens::composer::fork::{self, Asked, Kept};
    let host = Host::start(&["first", "second"]);
    let (first, second) = (host.thread("first"), host.thread("second"));
    let (shell, cx) = framed(&host, cx);
    open_thread(&shell, &host, &first, cx);
    let asked = Asked {
        workspace: host.workspace.clone(),
        thread: first.clone(),
        title: "first".into(),
        turn: Some("u1".into()),
        picks: Default::default(),
        message: None,
    };
    let reading = host.shell();
    cx.update(|window, cx| fork::ask(&reading, asked, Some(Kept { based: false, later_changed: false }), None, window, cx));
    shell.update_in(cx, |s, window, cx| s.go(thread_route(&host, &second), window, cx));
    let end = Instant::now() + Duration::from_millis(1500);
    while Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        shell.read_with(cx, |s, _| s.route().clone()),
        thread_route(&host, &second),
        "the fork pulled the window off the thread the person chose"
    );
    assert!(cx.update(|_, cx| fork::shown(cx)).is_none(), "the branch question opened over the thread the person chose");
}

#[gpui_kit::test]
fn a_computer_not_answering_holds_the_send_through_the_windows_own_sidebar(cx: &mut TestAppContext) {
    use desktop_gpui::sidebar::host::Change;
    let said = include_str!("../../../../packages/protocol/src/workspace-state.ts");
    assert!(said.contains("composerHeldLine = (computer: string): string => `held until ${computer} answers`"));
    let host = Host::start(&["first"]);
    let first = host.thread("first");
    let (shell, cx) = framed(&host, cx);
    let screen = open_thread(&shell, &host, &first, cx);
    let (_, composer) = parts(&screen, cx);
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    let ws = host.workspace.clone();
    let workspace = |kind: &str| json!({ "id": ws, "name": "repo", "kind": kind, "place": "mini", "project": { "id": "p1" } });
    let place = |present: bool| json!({ "id": "mini", "name": "mac-mini", "label": "Mac mini", "kind": "computer", "present": present });
    sidebar.update(cx, |s, cx| {
        s.apply(Change::Workspaces(vec![workspace("place")]), cx);
        s.apply(Change::Places(vec![place(false)]), cx);
    });
    let held = |cx: &mut VisualTestContext| composer.read_with(cx, |c, cx| c.send_held(cx));
    until(cx, "the send held for the computer", |cx| held(cx).as_deref() == Some("held until Mac mini answers"));
    composer.update_in(cx, |c, window, cx| {
        c.set_text("fix the scroll", window, cx);
        c.submit(window, cx);
    });
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "fix the scroll", "the send went, or the box let go of the words");
    assert!(cx.debug_bounds("composer-editor").is_some(), "the box stays open to write in");

    sidebar.update(cx, |s, cx| s.apply(Change::Places(vec![place(true)]), cx));
    until(cx, "the computer answering again", |cx| held(cx).is_none());

    // This computer's own daemon stopping is not the road a turn takes: the box is not held (`daemonOnly`).
    sidebar.update(cx, |s, cx| {
        s.apply(Change::Workspaces(vec![workspace("local")]), cx);
        s.apply(Change::Status(json!({ "id": ws, "reach": { "state": "no-daemon" } })), cx);
    });
    cx.run_until_parked();
    assert_eq!(held(cx), None, "this computer's stopped daemon held the send");
}

#[gpui_kit::test]
fn a_fork_off_a_persons_message_holds_its_words_and_keeps_the_pages_own_aside(cx: &mut TestAppContext) {
    use desktop_gpui::local_store::{self, LocalStore};
    use desktop_gpui::screens::composer::store;
    let host = Host::start(&["first"]);
    let first = host.thread("first");
    let kept = tempfile::tempdir().unwrap();
    cx.update(|cx| local_store::init(kept.path().join("local-store.json"), cx));
    let ws = host.workspace.clone();
    cx.update(|cx| cx.update_global(|s: &mut LocalStore, _| store::set_draft(s, &ws, "words for a new thread", 4)).unwrap());
    let (shell, cx) = framed(&host, cx);
    let screen = open_thread(&shell, &host, &first, cx);
    let (view, _) = parts(&screen, cx);
    let page_draft = |cx: &mut VisualTestContext| cx.update(|_, cx| store::draft(cx.global::<LocalStore>(), &ws).map(|(d, _)| d));
    let fork = |cx: &mut VisualTestContext| {
        let message = persons_message(&view, cx);
        assert_eq!(message.0, "first");
        view.update(cx, |_, cx| cx.emit(ThreadViewEvent::Fork { thread: first.clone(), turn: None, message: Some(message) }));
        fork_page(&shell, &host, cx)
    };
    let page = fork(cx);
    assert_eq!(page.read_with(cx, |c, cx| c.text(cx)), "first", "the fork's draft holds the message's words");
    // The draft's save waits 300 ms after an edit.
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(page_draft(cx).as_deref(), Some("words for a new thread"), "the message's words were kept as the page's own draft");
    drop(page);
    let frames = |cx: &mut VisualTestContext| {
        for _ in 0..3 {
            cx.update(|window, cx| window.simulate_next_frame(cx));
            cx.run_until_parked();
        }
    };

    let at = cx.debug_bounds("banner-dismiss").expect("the banner's dismiss").center();
    cx.simulate_click(at, gpui_kit::Modifiers::none());
    until(cx, "Cancel the fork back on the thread", |cx| shell.read_with(cx, |s, _| s.route().clone()) == thread_route(&host, &first));
    frames(cx);
    assert!(cx.update(|_, cx| composer::open_on(&ws, cx)).is_none(), "the page outlived Cancel the fork");
    assert_eq!(page_draft(cx).as_deref(), Some("words for a new thread"), "Cancel the fork lost the page's own words");

    drop(fork(cx));
    shell.update_in(cx, |s, window, cx| s.go(thread_route(&host, &first), window, cx));
    frames(cx);
    assert!(cx.update(|_, cx| composer::open_on(&ws, cx)).is_none(), "the page outlived the person leaving it");
    assert_eq!(page_draft(cx).as_deref(), Some("words for a new thread"), "leaving the draft lost the page's own words");
}

#[gpui_kit::test]
fn a_fork_off_a_message_with_an_image_carries_the_image(cx: &mut TestAppContext) {
    use desktop_gpui::screens::composer::model::File;
    const PNG: [u8; 70] = [
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13,
        73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96,
        130,
    ];
    let host = Host::start(&["first"]);
    let first = host.thread("first");
    let (shell, cx) = framed(&host, cx);
    let screen = open_thread(&shell, &host, &first, cx);
    let (view, own) = parts(&screen, cx);
    own.update_in(cx, |c, window, cx| {
        c.attach(vec![File { name: "dot.png".into(), media_type: "image/png".into(), bytes: Arc::new(PNG.to_vec()) }], cx);
        c.set_text("look at this", window, cx);
        c.submit(window, cx);
    });
    let mut message = None;
    until(cx, "the message with its image, its turn ended", |cx| {
        message = view.read_with(cx, |v, _| {
            v.rows().iter().find_map(|r| match &r.kind {
                RowKind::Person { files, .. } if r.text == "look at this" && files.iter().any(|f| f.image && f.id.is_some()) => {
                    Some((r.text.clone(), files.clone()))
                }
                _ => None,
            })
        });
        message.is_some() && own.read_with(cx, |c, _| !c.running() && c.hold().is_none())
    });
    view.update(cx, |_, cx| cx.emit(ThreadViewEvent::Fork { thread: first.clone(), turn: None, message: message.clone() }));
    let page = fork_page(&shell, &host, cx);
    until(cx, "the image in the fork's draft", |cx| page.read_with(cx, |c, _| !c.files().is_empty()));
    let file = page.read_with(cx, |c, _| c.files()[0].clone());
    assert_eq!((file.name.as_str(), file.media_type.as_str(), file.bytes.as_slice()), ("dot.png", "image/png", PNG.as_slice()));
    assert_eq!(page.read_with(cx, |c, cx| c.text(cx)), "look at this");
}

#[gpui_kit::test]
fn leaving_denys_field_with_escape_lets_its_words_go(cx: &mut TestAppContext) {
    let host = Host::start(&["first"]);
    let thread = host.thread("first");
    let (_mounted, screen, cx) = mount(&host, &thread, cx);
    ask(&screen, cx);
    let (_, composer) = parts(&screen, cx);
    cx.simulate_keystrokes("2");
    cx.run_until_parked();
    cx.simulate_input("use make");
    cx.simulate_keystrokes("escape");
    cx.simulate_keystrokes("2");
    cx.run_until_parked();
    assert_eq!(composer.read_with(cx, |c, _| c.dock().map(|d| d.primary_word())), Some("Deny"));
    cx.simulate_keystrokes("enter");
    let mut said = String::new();
    until(cx, "the stand-in saying the deny it was handed", |_| {
        said = host
            .history(&thread)
            .iter()
            .filter(|e| e["type"] == "session.delta")
            .filter_map(|e| e["text"].as_str())
            .find(|t| t.starts_with("answered:"))
            .unwrap_or_default()
            .to_owned();
        !said.is_empty()
    });
    assert!(said.contains("\"behavior\":\"deny\""), "{said}");
    assert!(!said.contains("use make"), "Deny's field opened again on the words Escape left behind: {said}");
}

/// The thread screen and the slate panel side by side, as the frame draws the panel beside the thread.
struct Beside {
    thread: gpui_kit::AnyView,
    slate: gpui_kit::AnyView,
}

impl gpui_kit::Render for Beside {
    fn render(&mut self, _: &mut gpui_kit::Window, _: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        use gpui_kit::{div, ParentElement as _, Styled as _};
        div()
            .size_full()
            .flex()
            .child(div().w(px(760.)).h_full().child(self.thread.clone()))
            .child(div().w(px(420.)).h_full().child(self.slate.clone()))
    }
}

#[gpui_kit::test]
fn the_slates_ask_for_one_waits_while_the_threads_prompt_stands_in_the_composer(cx: &mut TestAppContext) {
    use desktop_gpui::screens::slate::{words, Load, SlateScreen};
    let host = Host::start(&["first"]);
    let thread = host.thread("first");
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        composer::init(cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
    });
    let opened = Opened { workspace_id: Some(host.workspace.clone()), thread_id: Some(thread.clone()), ..Opened::default() };
    let slate_opened = Opened { thread_id: Some(thread.clone()), tab: Some("slate".into()), ..Opened::default() };
    let (mut mounted, mut slate) = (None, None);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = (screens::screen(THREAD).unwrap().mount)(&host.shell(), opened, window, cx);
        let panel = cx.new(|cx| <SlateScreen as screens::Screen>::new(host.shell(), slate_opened, window, cx));
        let beside = cx.new(|_| Beside { thread: screen.view.clone(), slate: panel.clone().into() });
        mounted = Some(screen);
        slate = Some(panel);
        Root::new(beside, window, cx)
    });
    cx.simulate_resize(size(px(1180.), px(800.)));
    let (mounted, slate) = (mounted.unwrap(), slate.unwrap());
    let screen = mounted.view.clone().downcast::<ThreadScreen>().ok().unwrap();
    let (_, composer) = parts(&screen, cx);
    until(cx, "the slate's empty state", |cx| slate.read_with(cx, |s, _| matches!(s.load, Load::Empty(_))));
    assert!(cx.debug_bounds("slate-held").is_none(), "no prompt stands yet");

    ask(&screen, cx);
    until(cx, "the slate's held note", |cx| cx.debug_bounds("slate-held").is_some());
    let ask_for_one = cx.debug_bounds("slate-ask").expect("Ask for one").center();
    cx.simulate_click(ask_for_one, gpui_kit::Modifiers::none());
    cx.run_until_parked();
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "", "Ask for one wrote into a box the prompt stands over");

    composer.update_in(cx, |c, window, cx| c.fold_prompt(window, cx));
    until(cx, "the held note gone once the prompt folds", |cx| cx.debug_bounds("slate-held").is_none());
    let ask_for_one = cx.debug_bounds("slate-ask").expect("Ask for one").center();
    cx.simulate_click(ask_for_one, gpui_kit::Modifiers::none());
    cx.run_until_parked();
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), words::ASK_PROMPT, "Ask for one fills the box once it stands");
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

/// A send in flight: the person's words stand at the transcript's foot as their message at once, before the host
/// answers, with the starting line under them, and give way to the turn's own message once it starts
/// (useChatThread.ts `pendingPrompt`, `sentEntry`).
#[gpui_kit::test]
fn a_send_shows_the_persons_message_at_once_then_the_starting_line(cx: &mut TestAppContext) {
    let host = Host::start(&["first"]);
    let thread = host.thread("first");
    let (_mounted, screen, cx) = mount(&host, &thread, cx);
    let (view, composer) = parts(&screen, cx);
    until(cx, "the composer's link", |cx| linked(&composer, cx) && view.read_with(cx, |v, _| v.loaded()));
    composer.update_in(cx, |c, window, cx| {
        c.set_text("LATE to start", window, cx);
        c.submit(window, cx);
    });
    let mine = |cx: &mut VisualTestContext| {
        view.read_with(cx, |v, _| {
            v.rows()
                .iter()
                .filter(|r| matches!(r.kind, RowKind::Person { .. }) && r.text == "LATE to start")
                .map(|r| r.key.clone())
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(mine(cx), ["sent"], "the person's message stands before the host has answered");
    until(cx, "the starting line under the send", |cx| view.read_with(cx, |v, _| v.rows().iter().any(|r| r.key == "starting")));
    let keys: Vec<String> = view.read_with(cx, |v, _| v.rows().iter().map(|r| r.key.clone()).collect());
    let at = |key: &str| keys.iter().position(|k| k == key);
    assert!(at("sent") < at("starting"), "the message, then the starting line under it: {keys:?}");
    until(cx, "the turn's own message in its place", |cx| {
        let shown = mine(cx);
        shown.len() == 1 && shown[0] != "sent"
    });
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
    assert_eq!(said.said, "The prompt carries no option by that id.");
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
fn a_frame_the_composer_did_not_change_in_reuses_its_drawing_at_its_size(cx: &mut TestAppContext) {
    let host = Host::start(&["first"]);
    let thread = host.thread("first");
    let (_mounted, screen, cx) = mount(&host, &thread, cx);
    let (view, composer) = parts(&screen, cx);
    until(cx, "the composer's link", |cx| linked(&composer, cx) && view.read_with(cx, |v, _| v.loaded()));
    let frame = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    };
    let dock = |cx: &mut VisualTestContext| cx.debug_bounds("thread-dock").map(|b| (b.origin.y, b.size.height));
    composer.update_in(cx, |c, window, cx| c.set_text("one\ntwo\nthree", window, cx));
    for _ in 0..3 {
        frame(cx);
    }
    // The first frame drawn cached fills the cache; the next reuses it.
    screen.update(cx, |_, cx| cx.notify());
    frame(cx);
    let (built, tall) = (composer.read_with(cx, |c, _| c.built()), dock(cx));
    screen.update(cx, |_, cx| cx.notify());
    frame(cx);
    assert_eq!(composer.read_with(cx, |c, _| c.built()), built, "a frame of the thread screen alone built the composer again");
    assert_eq!(dock(cx), tall, "the cached composer moved the dock");
    composer.update_in(cx, |c, window, cx| c.set_text("", window, cx));
    frame(cx);
    let short = dock(cx).expect("the dock");
    assert!(short.1 < tall.expect("the dock").1, "the composer drawn cached kept its old height: {short:?} against {tall:?}");

    // A change of height draws the composer whole in its next frame, never cached at the height it had before.
    for text in ["one\ntwo\nthree", "x", "one\ntwo", ""] {
        composer.update_in(cx, |c, window, cx| c.set_text(text, window, cx));
        frame(cx);
        screen.update(cx, |_, cx| cx.notify());
        frame(cx);
    }
    assert_eq!(composer.read_with(cx, |c, _| c.stale_layouts()), 0, "a frame laid the composer out at a height it no longer had");
}

#[gpui_kit::test]
fn a_fork_answered_after_the_person_left_moves_nothing(cx: &mut TestAppContext) {
    let host = Host::start(&["first", "second"]);
    let (first, second) = (host.thread("first"), host.thread("second"));
    let (shell, cx) = framed(&host, cx);
    let screen = open_thread(&shell, &host, &second, cx);
    let (view, _) = parts(&screen, cx);
    view.update(cx, |_, cx| cx.emit(ThreadViewEvent::Fork { thread: second.clone(), turn: None, message: None }));
    let page = fork_page(&shell, &host, cx);
    page.update_in(cx, |c, window, cx| {
        c.set_text("a fork nobody waits for", window, cx);
        c.submit(window, cx);
    });
    drop(page);
    shell.update_in(cx, |s, window, cx| s.go(thread_route(&host, &first), window, cx));
    for _ in 0..3 {
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }
    assert!(cx.update(|_, cx| composer::open_on(&host.workspace, cx)).is_none(), "the page's box outlived the page");
    until(cx, "the fork made on the host", |_| {
        let threads = host.runtime.block_on(host.client.threads()).unwrap();
        threads.iter().any(|t| t.prompt.as_deref() == Some("a fork nobody waits for"))
    });
    let end = Instant::now() + Duration::from_millis(1500);
    while Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        shell.read_with(cx, |s, _| s.route().clone()),
        thread_route(&host, &first),
        "the window stays on the thread the person chose"
    );
}

/// Runs `prompt` on the host's project without waiting, until the host holds `calls` of its turn's events and, with
/// `ended`, the turn's end.
fn many(host: &Host, prompt: &str, ended: bool) -> String {
    host.server.wsp(&["run", "repo", prompt, "--agent", "claude", "--detach"]);
    let thread = host.thread(prompt);
    let end = Instant::now() + WAIT;
    loop {
        let history = host.history(&thread);
        let deltas = history.iter().filter(|e| e["type"] == "session.delta").count();
        if deltas >= 450 && (!ended || history.iter().any(|e| e["type"] == "session.end")) {
            return thread;
        }
        assert!(Instant::now() < end, "the host held {deltas} of {prompt}'s calls");
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// The newest page of `thread`'s history, as the thread view first reads it, and how many events the thread holds.
fn newest(host: &Host, thread: &str) -> (Vec<Value>, u64) {
    let page = host.runtime.block_on(host.client.open_page(&host.workspace, thread)).unwrap_or_else(|_| panic!("no page of {thread}"));
    (page.history.iter().map(|e| serde_json::to_value(e).unwrap()).collect(), page.total)
}

/// Catches: the composer reading its compact rule (ChatComposer.tsx: a running turn or any message) off the one page of
/// history the view first holds, whose newest 200 events on a long thread are calls with no message and no turn's start.
#[gpui_kit::test]
fn a_long_threads_newest_page_of_calls_still_reads_as_running_and_messaged(cx: &mut TestAppContext) {
    let host = Host::start(&["first"]);
    let running = many(&host, "MANY calls, then wait", false);
    let done = many(&host, "MANY calls DONE", true);
    for thread in [&running, &done] {
        let (page, total) = newest(&host, thread);
        let said = page.iter().any(|e| e["type"] == "session.start" || e["kind"] == "text");
        assert!(total > page.len() as u64 && !said, "{thread}'s newest page holds a start or words, or the whole thread");
    }
    let (mounted, screen, cx) = mount(&host, &running, cx);
    let (view, composer) = parts(&screen, cx);
    until(cx, "the newest page and the link", |cx| view.read_with(cx, |v, _| v.loaded()) && linked(&composer, cx));
    assert!(composer.read_with(cx, |c, _| !c.fresh() && c.running()), "a running turn on a long thread leaves the composer tall and idle");
    show(&mounted, &host, &done, cx);
    until(cx, "the finished thread's newest page", |cx| {
        view.read_with(cx, |v, _| v.loaded() && v.thread() == Some(done.as_str())) && composer.read_with(cx, |c, _| !c.running())
    });
    assert!(composer.read_with(cx, |c, _| !c.fresh()), "a thread with messages above its newest page reads as fresh");
}
