// SPDX-License-Identifier: AGPL-3.0-only
//! The toasts through the road the app takes: a view inside gpui-kit's Root after `gpui_kit::init`, wsp's themes and
//! the layers' registration, a notice pushed as any screen pushes one. The pointer, the keys and the timers are
//! checked on a window with no host; the host's pushes on a host started for these tests alone.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::screens::palette::overlays;
use desktop_gpui::screens::{Host, Route};
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::ui::toast::{self, store, Action, Kind, Notice, Toasts};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    div, point, prelude::*, px, size, AppContext, Context, ElementId, Entity, Modifiers, TestAppContext, TestSupportExt as _,
    VisualTestContext, Window,
};
use wsp_host_client::HostClient;

/// The page under the toasts: it counts the presses that reach it, and holds a field.
struct Page {
    presses: Rc<Cell<usize>>,
    field: Entity<InputState>,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let presses = self.presses.clone();
        div()
            .id("page")
            .size_full()
            .on_click(move |_, _, _| presses.set(presses.get() + 1))
            .child(Input::new(&self.field).id("field").w(px(200.)))
            .test_support()
    }
}

fn open(cx: &mut TestAppContext) -> (Rc<Cell<usize>>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        desktop_gpui::screens::palette::init(cx);
    });
    let presses = Rc::new(Cell::new(0));
    let counted = presses.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let field = cx.new(|cx| InputState::new(window, cx));
        let page = cx.new(|_| Page { presses: counted, field });
        Root::new(page, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    (presses, cx)
}

fn notices_of(cx: &mut VisualTestContext) -> Entity<Toasts> {
    cx.update(|window, cx| toast::of(window, cx).expect("ui::init registers the stack"))
}

/// Draws until every move has run out.
fn settle(cx: &mut VisualTestContext) {
    for _ in 0..8 {
        cx.executor().advance_clock(Duration::from_millis(150));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

fn push(cx: &mut VisualTestContext, notice: Notice) {
    cx.update(|window, cx| desktop_gpui::ui::notify_in(window, cx, notice));
    settle(cx);
}

/// The sentences whose toast stands, oldest first.
fn standing(cx: &mut VisualTestContext) -> Vec<String> {
    let notices = notices_of(cx);
    notices.read_with(cx, |n, _| n.standing().iter().map(|n| n.text.to_string()).collect())
}

fn id(name: &'static str, n: u64) -> ElementId {
    ElementId::NamedInteger(name.into(), n)
}

/// Whether toast `n` is drawn at all.
fn drawn(cx: &mut VisualTestContext, n: u64) -> bool {
    cx.update(|window, _| window.try_find(id("notice", n)).is_some())
}

#[gpui_kit::test]
fn a_push_shows_a_toast_and_its_x_closes_it(cx: &mut TestAppContext) {
    let (_, cx) = open(cx);
    push(cx, Notice::done("Settled 1 thread").place("Boat"));
    assert_eq!(standing(cx), ["Settled 1 thread"]);
    let toast = cx.update(|window, _| window.find(id("notice", 1)));
    assert!(toast.visible());
    let b = toast.bounds();
    assert_eq!((b.size.width, b.origin.x, b.origin.y), (px(356.), px(1280. - 20. - 356.), px(52. + 8.)), "{b:?}");
    assert!(!cx.update(|window, _| window.find(id("notice-close", 1)).visible()), "the X shows only under the pointer");
    cx.update(|window, cx| window.hover(id("notice", 1), cx));
    settle(cx);
    cx.update(|window, cx| window.click(id("notice-close", 1), cx));
    assert!(standing(cx).is_empty());
    settle(cx);
    assert!(!drawn(cx, 1), "the toast is gone once its exit ran");
}

/// The one call a screen with no window at hand makes reaches the window's stack.
#[gpui_kit::test]
fn notify_says_it_in_the_windows_stack(cx: &mut TestAppContext) {
    let (_, cx) = open(cx);
    cx.update(|_, cx| desktop_gpui::ui::notify(cx, Notice::error("Not opened: no editor")));
    cx.run_until_parked();
    assert_eq!(standing(cx), ["Not opened: no editor"]);
}

#[gpui_kit::test]
fn escape_closes_the_newest_unless_a_field_holds_the_focus(cx: &mut TestAppContext) {
    let (_, cx) = open(cx);
    push(cx, Notice::note("first"));
    push(cx, Notice::note("second"));
    cx.update(|window, cx| window.press("escape", cx));
    assert_eq!(standing(cx), ["first"]);
    cx.update(|window, cx| window.click("field", cx));
    cx.update(|window, cx| window.press("escape", cx));
    assert_eq!(standing(cx), ["first"], "the field takes Escape");
}

/// Escape on a page goes back, and a toast takes it first, as the web's document listener runs ahead of the shell's.
#[gpui_kit::test]
fn on_a_page_escape_closes_the_toast_before_the_page(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        desktop_gpui::screens::palette::init(cx);
    });
    let dir = tempfile::tempdir().unwrap();
    let home = [("HOME".to_owned(), dir.path().to_string_lossy().into_owned())].into_iter().collect();
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap());
    let state = dir.path().join("state.json");
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, home, runtime, Launch::default(), window, cx));
        shell = Some(view.clone());
        Root::new(view, window, cx)
    });
    let shell = shell.unwrap();
    cx.simulate_resize(size(px(1280.), px(800.)));
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen("settings".into()), window, cx));
    cx.run_until_parked();
    push(cx, Notice::note("first"));
    cx.update(|window, cx| window.press("escape", cx));
    assert!(standing(cx).is_empty());
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen("settings".into()), "the page stays");
    cx.update(|window, cx| window.press("escape", cx));
    assert_ne!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen("settings".into()), "the next Escape goes back");
}

#[gpui_kit::test]
fn the_action_runs_and_closes_its_toast(cx: &mut TestAppContext) {
    let (_, cx) = open(cx);
    let ran = Rc::new(Cell::new(0));
    let counted = ran.clone();
    push(cx, Notice::note("Settled 2 threads").action(Action::new("Undo", move |_, _| counted.set(counted.get() + 1))));
    cx.update(|window, cx| window.click(id("notice-action", 1), cx));
    assert_eq!(ran.get(), 1);
    assert!(standing(cx).is_empty());
}

#[gpui_kit::test]
fn a_press_on_a_toast_does_not_reach_the_page_under_it(cx: &mut TestAppContext) {
    let (presses, cx) = open(cx);
    push(cx, Notice::error("Not copied: no clipboard"));
    cx.update(|window, cx| window.click(id("notice", 1), cx));
    assert_eq!(presses.get(), 0, "the toast took the press");
    assert_eq!(standing(cx).len(), 1, "a press is not a dismiss");
    cx.update(|window, cx| window.click_at("page", point(px(640.), px(600.)), cx));
    assert_eq!(presses.get(), 1, "the page still takes a press beside it");
}

#[gpui_kit::test]
fn three_stand_and_a_fourth_drops_the_oldest_plain_one(cx: &mut TestAppContext) {
    let (_, cx) = open(cx);
    push(cx, Notice::waiting("wsp needs you: sign in").key("needs-you"));
    for text in ["one", "two", "three"] {
        push(cx, Notice::note(text));
    }
    assert_eq!(standing(cx), ["wsp needs you: sign in", "two", "three"]);
    settle(cx);
    assert!(!drawn(cx, 2), "the dropped toast left");
    // The front toast whole, the two behind it lifted a gap each and 5% narrower each.
    let widths: Vec<f32> =
        [4, 3, 1].iter().map(|n| cx.update(|window, _| f32::from(window.find(id("notice", *n)).bounds().size.width))).collect();
    // Laid out on whole pixels: each edge rounds, so a width can be half a pixel off.
    for (width, want) in widths.iter().zip([356., 356. * 0.95, 356. * 0.9]) {
        assert!((width - want).abs() <= 0.5, "{widths:?}");
    }
}

#[gpui_kit::test]
fn a_toast_stands_8_s_one_with_an_action_12_s_and_a_keyed_one_until_ended(cx: &mut TestAppContext) {
    let (_, cx) = open(cx);
    cx.update(|window, cx| {
        desktop_gpui::ui::notify_in(window, cx, Notice::note("plain"));
        desktop_gpui::ui::notify_in(window, cx, Notice::note("acting").action(Action::new("Open", |_, _| {})));
        desktop_gpui::ui::notify_in(window, cx, Notice::waiting("waiting").key("k"));
    });
    cx.run_until_parked();
    cx.executor().advance_clock(store::NOTICE_MS - Duration::from_millis(1));
    cx.run_until_parked();
    assert_eq!(standing(cx), ["plain", "acting", "waiting"]);
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    assert_eq!(standing(cx), ["acting", "waiting"]);
    cx.executor().advance_clock(store::NOTICE_WITH_ACTION_MS - store::NOTICE_MS);
    cx.run_until_parked();
    assert_eq!(standing(cx), ["waiting"]);
    cx.executor().advance_clock(Duration::from_secs(600));
    cx.run_until_parked();
    assert_eq!(standing(cx), ["waiting"]);
    cx.update(|_, cx| desktop_gpui::ui::end_notice(cx, "k"));
    assert!(standing(cx).is_empty());
}

#[gpui_kit::test]
fn the_pointer_on_the_stack_holds_its_timers(cx: &mut TestAppContext) {
    let (_, cx) = open(cx);
    push(cx, Notice::note("held"));
    cx.update(|window, cx| window.hover(id("notice", 1), cx));
    cx.executor().advance_clock(Duration::from_secs(30));
    cx.run_until_parked();
    assert_eq!(standing(cx), ["held"], "the timer holds under the pointer");
    cx.simulate_mouse_move(point(px(100.), px(700.)), None, Modifiers::default());
    cx.executor().advance_clock(store::NOTICE_MS - Duration::from_millis(1200) - Duration::from_millis(10));
    cx.run_until_parked();
    assert_eq!(standing(cx), ["held"], "the time it had left runs again from where it stopped");
    cx.executor().advance_clock(Duration::from_millis(20));
    cx.run_until_parked();
    assert!(standing(cx).is_empty());
}

#[gpui_kit::test]
fn a_swipe_right_dismisses_and_a_short_one_left_does_not(cx: &mut TestAppContext) {
    let (_, cx) = open(cx);
    push(cx, Notice::note("swiped"));
    let at = cx.update(|window, _| window.find(id("notice", 1)).bounds().center());
    cx.simulate_mouse_down(at, gpui_kit::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(at - point(px(30.), px(0.)), Some(gpui_kit::MouseButton::Left), Modifiers::default());
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.simulate_mouse_up(at - point(px(30.), px(0.)), gpui_kit::MouseButton::Left, Modifiers::default());
    assert_eq!(standing(cx), ["swiped"]);
    settle(cx);
    cx.simulate_mouse_down(at, gpui_kit::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(at + point(px(60.), px(0.)), Some(gpui_kit::MouseButton::Left), Modifiers::default());
    // Slow enough that only the distance dismisses it.
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.simulate_mouse_up(at + point(px(60.), px(0.)), gpui_kit::MouseButton::Left, Modifiers::default());
    assert!(standing(cx).is_empty());
}

/// A claude that answers a prompt at once, and dies before it replies to one holding CRASH.
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
let buf = "";
process.stdin.on("data", d => {
  buf += d;
  for (let i = buf.indexOf("\n"); i >= 0; i = buf.indexOf("\n")) {
    const line = buf.slice(0, i);
    buf = buf.slice(i + 1);
    if (line.trim() === "") continue;
    const m = JSON.parse(line);
    if (m.type === "control_request") say({ type: "control_response", response: { subtype: "success", request_id: m.request_id } });
    else if (m.type === "user") {
      const prompt = (m.message?.content ?? []).filter(c => c.type === "text").map(c => c.text).join("");
      say({ type: "system", subtype: "init", cwd: process.cwd(), session_id: sid, tools: [], mcp_servers: [], model: "claude-haiku-5-5", permissionMode: "bypassPermissions", slash_commands: [], apiKeySource: "none", uuid: `init_${++n}` });
      if (prompt.includes("CRASH")) process.exit(1);
      say({ type: "assistant", message: { id: `msg_${++n}`, type: "message", role: "assistant", model: "claude-haiku-5-5", content: [{ type: "text", text: `ok: ${prompt}` }], stop_reason: null, usage }, parent_tool_use_id: null, session_id: sid, uuid: `a_${n}` });
      say({ type: "result", subtype: "success", is_error: false, duration_ms: 1, num_turns: 1, result: `ok: ${prompt}`, session_id: sid, total_cost_usd: 0, usage, uuid: `r_${++n}` });
    }
  }
});
process.stdin.on("end", () => process.exit(0));
"#;

/// A window whose toasts follow a host of their own, with a project `repo` added.
fn live(cx: &mut TestAppContext) -> (Throwaway, &mut VisualTestContext) {
    let server = Throwaway::start_with(STAND_IN, false);
    let folder = server.repo("repo");
    server.wsp(&["add", &folder.to_string_lossy()]);
    cx.executor().allow_parking();
    let (_, cx) = open(cx);
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client =
        Arc::new(runtime.block_on(HostClient::connect(&server.state(), &server.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    let host = Host::new(client, runtime);
    let notices = cx.update(|window, cx| overlays::of(window, cx).expect("the layers are registered").read(cx).notices.clone());
    notices.update_in(cx, |n, window, cx| n.attach(host, window, cx));
    cx.run_until_parked();
    // The road is up once the job's first read has landed.
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    (server, cx)
}

/// Waits up to 30 s for a toast that says `kind` and passes `says`.
fn heard(cx: &mut VisualTestContext, server: &Throwaway, kind: Kind, says: impl Fn(&str) -> bool) -> Notice {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        let notices = notices_of(cx);
        let found = notices.read_with(cx, |n, _| n.standing().into_iter().find(|n| n.kind == kind && says(&n.text)));
        if let Some(found) = found {
            return found;
        }
        let said: Vec<String> = notices.read_with(cx, |n, _| n.standing().iter().map(|n| n.text.to_string()).collect());
        assert!(Instant::now() < until, "no such toast; standing: {said:?}\nhost log: {}", server.log());
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[gpui_kit::test]
fn a_thread_that_tells_the_person_it_finished_raises_a_toast(cx: &mut TestAppContext) {
    let (server, cx) = live(cx);
    server.wsp(&["run", "repo", "hello", "--agent", "claude", "--notify", "me", "--detach"]);
    let toast = heard(cx, &server, Kind::Note, |t| t.starts_with("thread ") && t.contains(" finished (completed"));
    assert!(toast.action.is_some_and(|a| a.word == "Open"), "it opens the thread");
    assert!(toast.place.is_some(), "it names where the thread runs");
}

#[gpui_kit::test]
fn a_thread_whose_agent_dies_raises_an_error_toast(cx: &mut TestAppContext) {
    let (server, cx) = live(cx);
    server.wsp(&["run", "repo", "CRASH", "--agent", "claude", "--detach"]);
    let toast = heard(cx, &server, Kind::Error, |t| t.starts_with("A thread stopped before it replied"));
    assert_eq!(toast.text.as_ref(), "A thread stopped before it replied: claude exited with code 1 before emitting a result");
    assert!(toast.action.is_some_and(|a| a.word == "Open"), "it opens the thread");
}
