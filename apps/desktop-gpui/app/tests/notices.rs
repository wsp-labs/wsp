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
use desktop_gpui::screens::dialogs;
use desktop_gpui::screens::dialogs::add_computer::{self, model::AddStep};
use desktop_gpui::screens::notices::host::Heard;
use desktop_gpui::screens::palette::overlays;
use desktop_gpui::screens::sidebar::host::Change;
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::ui::toast::{self, store, Action, Kind, Notice, Toasts};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    div, point, prelude::*, px, size, AppContext, Context, ElementId, Entity, Modifiers, TestAppContext, TestSupportExt as _,
    VisualTestContext, Window,
};
use serde_json::{json, Value};
use wsp_host_client::protocol::PlaceView;

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

/// The app's window on a host of its own, with a project `repo` added, once the shell is connected: the notices read
/// the computers and workspaces its sidebar follows.
fn live(cx: &mut TestAppContext) -> (Throwaway, &mut VisualTestContext) {
    let server = Throwaway::start_with(STAND_IN, false);
    let folder = server.repo("repo");
    server.wsp(&["add", &folder.to_string_lossy()]);
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        desktop_gpui::screens::palette::init(cx);
    });
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let (state, env) = (server.state(), server.env());
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
        shell = Some(view.clone());
        Root::new(view, window, cx)
    });
    let shell = shell.unwrap();
    cx.simulate_resize(size(px(1280.), px(800.)));
    let until = Instant::now() + Duration::from_secs(30);
    while !shell.read_with(cx, |s, _| s.connected()) {
        assert!(Instant::now() < until, "the shell never connected\nhost log: {}", server.log());
        std::thread::sleep(Duration::from_millis(100));
        cx.run_until_parked();
    }
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
    let toast = heard(cx, &server, Kind::Error, |t| t.contains("stopped before it replied"));
    assert_eq!(toast.text.as_ref(), "CRASH stopped before it replied: claude exited with code 1 before emitting a result", "by its title");
    assert!(toast.action.is_some_and(|a| a.word == "Open"), "it opens the thread");
}

/// Sonner hands the focus back to what held it once the toast a press focused leaves, so the shell's chords and the
/// field's keys come back with it.
#[gpui_kit::test]
fn a_press_on_a_toasts_card_gives_the_focus_back_when_it_closes(cx: &mut TestAppContext) {
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
    cx.run_until_parked();
    let focused = |cx: &mut VisualTestContext| cx.update(|window, cx| window.focused(cx));
    // The right panel, open, not the shell's root, which takes the keys back on its own once a focus is lost.
    let alt_b = if cfg!(target_os = "macos") { "cmd-alt-b" } else { "ctrl-alt-b" };
    cx.simulate_keystrokes(alt_b);
    let panel = shell.read_with(cx, |s, cx| gpui_kit::Focusable::focus_handle(s.panel().read(cx), cx));
    cx.update(|window, cx| window.focus(&panel, cx));
    cx.run_until_parked();
    assert_eq!(focused(cx), Some(panel.clone()), "the panel holds the keys");
    push(cx, Notice::note("first"));
    cx.update(|window, cx| window.click(id("notice", 1), cx));
    assert_ne!(focused(cx), Some(panel.clone()), "the press focuses the toast");
    cx.update(|window, cx| window.press("escape", cx));
    assert!(standing(cx).is_empty(), "the focused toast takes Escape");
    settle(cx);
    assert_eq!(focused(cx), Some(panel), "the focus is back where the press found it");
    let open = shell.read_with(cx, |s, _| s.sidebar_open());
    cx.simulate_keystrokes(if cfg!(target_os = "macos") { "cmd-b" } else { "ctrl-b" });
    cx.run_until_parked();
    assert_ne!(shell.read_with(cx, |s, _| s.sidebar_open()), open, "and the shell's chords with it");
}

/// A pressed toast that waits, a sign-in for one, may stand for minutes: the shell's chords go on past it while it holds
/// the focus, as Sonner's keys go on to the document, and Escape still closes it first.
#[gpui_kit::test]
fn a_pressed_toast_lets_the_shells_chords_through(cx: &mut TestAppContext) {
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
    cx.run_until_parked();
    let m = if cfg!(target_os = "macos") { "cmd" } else { "ctrl" };
    let focused = |cx: &mut VisualTestContext| cx.update(|window, cx| window.focused(cx));
    let shells = focused(cx);
    push(cx, Notice::waiting("Waiting for the sign-in").key("sign-in"));
    cx.update(|window, cx| window.click(id("notice", 1), cx));
    let toasts = focused(cx);
    assert_ne!(toasts, shells, "the press focuses the toast");
    let open = shell.read_with(cx, |s, _| s.sidebar_open());
    cx.simulate_keystrokes(&format!("{m}-b"));
    cx.run_until_parked();
    assert_ne!(shell.read_with(cx, |s, _| s.sidebar_open()), open, "mod+b reaches the shell past the focused toast");
    assert_eq!(focused(cx), toasts, "and the toast keeps the focus");
    cx.simulate_keystrokes("escape");
    assert!(standing(cx).is_empty(), "Escape closes the focused toast first");
    assert_ne!(shell.read_with(cx, |s, _| s.sidebar_open()), open, "and nothing else");
    settle(cx);
    push(cx, Notice::waiting("Waiting for the sign-in").key("sign-in"));
    cx.update(|window, cx| window.click(id("notice", 2), cx));
    assert_ne!(focused(cx), shells, "the press focuses the second toast");
    cx.simulate_keystrokes(&format!("{m}-k"));
    cx.run_until_parked();
    assert!(cx.update(|window, cx| desktop_gpui::screens::palette::palette::is_open(window, cx)), "mod+k opens the palette");
    assert_eq!(standing(cx), ["Waiting for the sign-in"], "the waiting toast still stands");
}

#[gpui_kit::test]
fn a_press_in_the_gap_between_spread_toasts_stays_with_the_stack(cx: &mut TestAppContext) {
    let (presses, cx) = open(cx);
    push(cx, Notice::note("first"));
    push(cx, Notice::note("second"));
    cx.update(|window, cx| window.hover(id("notice", 2), cx));
    settle(cx);
    let (front, behind) = cx.update(|window, _| (window.find(id("notice", 2)).bounds(), window.find(id("notice", 1)).bounds()));
    assert!(behind.top() - front.bottom() > px(10.), "spread out under the pointer: {front:?} {behind:?}");
    let gap = point(front.center().x, front.bottom() + (behind.top() - front.bottom()) / 2.);
    cx.simulate_click(gap, Modifiers::default());
    assert_eq!(presses.get(), 0, "the gap took the press");
}

/// A layer drawn over the toasts, a scrim, takes the pointer: the stack under it neither spreads nor holds its timers.
#[gpui_kit::test]
fn the_pointer_on_a_scrim_over_the_stack_leaves_its_timers_running(cx: &mut TestAppContext) {
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
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, home, runtime, Launch::default(), window, cx));
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    push(cx, Notice::note("under"));
    let at = cx.update(|window, _| window.find(id("notice", 1)).bounds().center());
    cx.update(desktop_gpui::screens::palette::palette::open);
    settle(cx);
    cx.simulate_mouse_move(at, None, Modifiers::default());
    cx.executor().advance_clock(store::NOTICE_MS);
    cx.run_until_parked();
    assert!(standing(cx).is_empty(), "the toast's time ran out under the scrim");
}

/// lab-box as the sidebar holds it once the host lists it: its setup `run` at `state`, with `rows` applied.
fn lab_box(run: &str, state: &str, rows: Value) -> Value {
    let place = json!({
        "id": "p_box", "kind": "computer", "name": "lab-box", "default": false, "agents": [], "agentVersions": {}, "signIns": {},
        "sizes": [], "recipe": "none", "applied": { "hash": "h", "at": "2026-10-18T10:00:00Z", "rows": rows },
        "setup": { "state": state, "addId": run, "startedAt": "2026-10-18T10:00:00Z", "steps": [], "waiting": [] }
    });
    serde_json::from_value::<PlaceView>(place.clone()).unwrap_or_else(|e| panic!("{place}: {e}"));
    place
}

/// The host's computers moved: the sidebar takes the list, as its road from the host hands it over.
fn listed(cx: &mut VisualTestContext, places: Vec<Value>) {
    cx.update(|window, cx| {
        let sidebar = overlays::of(window, cx).and_then(|o| o.read(cx).sidebar.clone()).and_then(|s| s.upgrade());
        sidebar.expect("the shell hands the overlays its sidebar").update(cx, |s, cx| {
            s.apply(Change::Places(places), cx);
            cx.notify();
        });
    });
    cx.run_until_parked();
}

/// lab-box's setup `run` ended as `end`: the frame the host pushes, taken as the notices' road takes it.
fn setup_ended(cx: &mut VisualTestContext, run: &str, end: &str, said: Option<&str>) {
    let mut frame = json!({ "type": "place.setup", "addId": run, "placeId": "p_box", "end": end });
    if let Some(said) = said {
        frame["said"] = json!(said);
    }
    cx.update(|window, cx| {
        let notices = overlays::of(window, cx).expect("the window's overlays").read(cx).notices.clone();
        notices.update(cx, |n, cx| n.hear(Heard { kind: "place.setup".into(), frame, preferences: None }, window, cx));
    });
    cx.run_until_parked();
}

/// lab-box's setup `run` ends as `end`, the list moving from running to done with `rows` as the host moves it.
fn setup_runs_out(cx: &mut VisualTestContext, run: &str, end: &str, said: Option<&str>, rows: Value) {
    listed(cx, vec![lab_box(run, "running", json!([]))]);
    setup_ended(cx, run, end, said);
    listed(cx, vec![lab_box(run, "done", rows)]);
}

/// The notices said of lab-box, oldest first.
fn of_lab_box(cx: &mut VisualTestContext) -> Vec<Notice> {
    let notices = notices_of(cx);
    notices.read_with(cx, |n, _| n.standing().into_iter().filter(|n| n.place.as_deref() == Some("lab-box")).collect())
}

/// Add a computer standing on lab-box, as the person left it on the running view.
fn add_a_computer_on_lab_box(cx: &mut VisualTestContext, place: Value) {
    cx.update(|window, cx| {
        add_computer::staged(window, cx, |d, _, cx| {
            d.take_places(vec![place], vec![], vec![], cx);
            d.stage(AddStep::Running, "nobody@192.0.2.1", None, Some("p_box"), None, cx);
        })
    });
    cx.run_until_parked();
}

/// Catches: the host's road saying a ready setup a second time, after the dialog or a frame said it already, or its
/// Open going anywhere but Add a computer's running view.
#[gpui_kit::test]
fn a_setup_that_ends_ready_is_said_once_and_opens_add_a_computer(cx: &mut TestAppContext) {
    let (_server, cx) = live(cx);
    setup_runs_out(cx, "s_ready", "ready", None, json!([]));
    let said = of_lab_box(cx);
    let texts: Vec<&str> = said.iter().map(|n| n.text.as_ref()).collect();
    assert_eq!(texts, ["lab-box is ready: threads you start on it run there"], "the host's road says it");
    setup_ended(cx, "s_ready", "ready", None);
    listed(cx, vec![lab_box("s_ready", "done", json!([]))]);
    add_a_computer_on_lab_box(cx, lab_box("s_ready", "done", json!([])));
    cx.update(dialogs::layer::dismiss);
    cx.run_until_parked();
    assert_eq!(of_lab_box(cx).len(), 1, "said once a run, whichever road said it first");
    let open = said[0].action.clone().expect("the notice opens something");
    assert_eq!(open.word.as_ref(), "Open");
    cx.update(|window, cx| (open.run)(window, cx));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| dialogs::layer::top(cx)), Some(dialogs::ADD_COMPUTER), "its Open opens Add a computer");
}

/// Catches: a ready end heard after the sidebar's own read already listed the run done, so no later list says it.
#[gpui_kit::test]
fn a_setup_listed_done_before_its_end_is_heard_is_still_said(cx: &mut TestAppContext) {
    let (_server, cx) = live(cx);
    listed(cx, vec![lab_box("s_late", "running", json!([]))]);
    listed(cx, vec![lab_box("s_late", "done", json!([]))]);
    setup_ended(cx, "s_late", "ready", None);
    let texts: Vec<String> = of_lab_box(cx).iter().map(|n| n.text.to_string()).collect();
    assert_eq!(texts, ["lab-box is ready: threads you start on it run there"]);
}

/// Catches: the host's road saying a setup that left rows, or one whose folder did not land, as plainly ready.
#[gpui_kit::test]
fn a_setup_that_ended_says_what_it_left_or_that_it_needs_you(cx: &mut TestAppContext) {
    let (_server, cx) = live(cx);
    let left = json!([{ "id": "clis/rg", "label": "rg", "outcome": "failed", "step": "clis", "note": "apt refused" }]);
    setup_runs_out(cx, "s_left", "ready", Some("rg failed"), left);
    let said = of_lab_box(cx);
    assert_eq!(said.len(), 1, "{:?}", said.iter().map(|n| n.text.to_string()).collect::<Vec<_>>());
    assert_eq!(said[0].text.as_ref(), "lab-box is ready: threads you start on it run there");
    let detail = said[0].detail.as_ref().map(|d| d.to_string()).unwrap_or_default();
    assert_eq!(detail, "rg failed. Finish it from lab-box's page in Settings.", "it names what the setup left");
    let folder = json!([{ "id": "folders/wsp", "label": "wsp", "outcome": "failed", "step": "folders", "note": "git clone was refused" }]);
    setup_runs_out(cx, "s_folder", "needs-you", Some("wsp: git clone was refused"), folder);
    let texts: Vec<String> = of_lab_box(cx).iter().map(|n| n.text.to_string()).collect();
    assert_eq!(texts.last().map(String::as_str), Some("lab-box needs you: wsp: git clone was refused"), "a needs-you end says so");
}

/// Catches: the host's road saying a setup's end while Add a computer stands on that computer, whose rows say it.
#[gpui_kit::test]
fn nothing_is_said_of_a_setup_while_add_a_computer_stands_on_its_computer(cx: &mut TestAppContext) {
    let (_server, cx) = live(cx);
    add_a_computer_on_lab_box(cx, lab_box("s_shown", "running", json!([])));
    setup_runs_out(cx, "s_shown", "ready", None, json!([]));
    let texts: Vec<String> = of_lab_box(cx).iter().map(|n| n.text.to_string()).collect();
    assert!(texts.is_empty(), "the dialog's rows say it: {texts:?}");
}

/// Catches: the host's road saying a cloud's sealed build while Add a cloud provider stands on that build, whose heading
/// says it, so the dialog's close says it a second time.
#[gpui_kit::test]
fn a_seal_the_cloud_dialog_stands_on_is_said_once_by_its_close(cx: &mut TestAppContext) {
    use desktop_gpui::screens::dialogs::add_cloud::{self, flow};
    let (_server, cx) = live(cx);
    let job = json!({ "id": "j_seal", "phase": "done", "place": { "id": "box", "name": "Boat" }, "golden": { "version": 3 } });
    cx.update(|window, cx| {
        let mut on_build = flow::Flow::new(Some("box"));
        on_build.step = flow::Step::Building;
        on_build.job = Some(serde_json::from_value(job.clone()).unwrap());
        add_cloud::open_on(None, on_build, window, cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let notices = overlays::of(window, cx).expect("the window's overlays").read(cx).notices.clone();
        let frame = json!({ "type": "init.job", "job": job });
        notices.update(cx, |n, cx| n.hear(Heard { kind: "init.job".into(), frame, preferences: None }, window, cx));
    });
    cx.run_until_parked();
    let sealed = |cx: &mut VisualTestContext| {
        notices_of(cx).read_with(cx, |n, _| n.standing().iter().filter(|n| n.text.as_ref() == "Boat is ready").count())
    };
    assert_eq!(sealed(cx), 0, "the host's road holds it while the dialog stands on the build");
    cx.update(dialogs::layer::dismiss);
    cx.run_until_parked();
    assert_eq!(sealed(cx), 1, "the dialog's close says it once");
}

/// A change to the build job as the sidebar's road hands it over.
fn job_moved(cx: &mut VisualTestContext, change: Change) {
    cx.update(|window, cx| {
        let sidebar = overlays::of(window, cx).and_then(|o| o.read(cx).sidebar.clone()).and_then(|s| s.upgrade());
        sidebar.expect("the shell hands the overlays its sidebar").update(cx, |s, cx| {
            s.apply(change, cx);
            cx.notify();
        });
    });
    cx.run_until_parked();
}

fn job_read(cx: &mut VisualTestContext, job: Value) {
    job_moved(cx, Change::Job(Some(job)));
}

fn needs(cx: &mut VisualTestContext) -> Vec<String> {
    let notices = notices_of(cx);
    notices.read_with(cx, |n, _| {
        n.standing().into_iter().filter(|n| n.key.as_deref() == Some("needs-you")).map(|n| n.text.to_string()).collect()
    })
}

/// Catches: the notices keeping a build job of their own, so the need stands on after the sidebar's read of the job
/// no longer carries it, or is ended while the sidebar's job still asks for it; or a need the job's road pushed going
/// unsaid.
#[gpui_kit::test]
fn a_builds_need_goes_once_the_sidebars_job_no_longer_carries_it(cx: &mut TestAppContext) {
    let (_server, cx) = live(cx);
    let need = json!({ "what": "Sign in to GitHub" });
    let job = json!({ "id": "j_need", "phase": "building", "place": { "id": "box", "name": "Boat" }, "needsYou": need });
    job_read(cx, job.clone());
    assert_eq!(needs(cx).len(), 0, "a read alone says nothing, as the web says a need only on its push");
    job_moved(cx, Change::JobNeed(need));
    assert_eq!(needs(cx).len(), 1, "the push says the need");
    job_read(cx, job);
    assert_eq!(needs(cx).len(), 1, "a read that still carries the need keeps it");
    job_read(cx, json!({ "id": "j_need", "phase": "building", "place": { "id": "box", "name": "Boat" } }));
    assert_eq!(needs(cx), Vec::<String>::new(), "the sidebar's job no longer carries the need, so it goes");
}

/// A push the window's rules take, as the notices' road hands it over.
fn hear(cx: &mut VisualTestContext, frame: Value) {
    cx.update(|window, cx| {
        let notices = overlays::of(window, cx).expect("the window's overlays").read(cx).notices.clone();
        let kind = frame["type"].as_str().unwrap_or_default().to_owned();
        notices.update(cx, |n, cx| n.hear(Heard { kind, frame, preferences: None }, window, cx));
    });
    cx.run_until_parked();
}

/// Catches: one of the moments today's page says over the system (`sayOutside` in `hostNotices.ts`) going unsaid by
/// the window's rules while the window is up with no focus, a sign-in a setup waits on said once per frame rather
/// than once while it stands, or the sign-in's click opening anything but Add a computer. Seven lines: a setup's
/// sign-in, its needs-you, ready and failed ends, a build's seal, a build's need, and a workspace waking.
#[gpui_kit::test]
fn the_window_says_the_pages_moments_over_the_system_while_it_has_no_focus(cx: &mut TestAppContext) {
    use desktop_gpui::desktop::notify;
    use gpui_kit::SystemNotificationResponse;
    let (_server, cx) = live(cx);
    cx.update(|_, cx| {
        notify::init(Rc::new(|_| {}), cx);
        notify::take_as_bundled(true, cx);
    });
    cx.deactivate_window();
    let mut waiting = lab_box("s_out", "running", json!([]));
    waiting["setup"]["waiting"] = json!([{ "row": "gh", "state": "waiting", "label": "GitHub" }]);
    let wait = json!({ "type": "place.setup", "placeId": "p_box", "addId": "s_out", "wait": { "row": "gh", "state": "waiting", "label": "GitHub" } });
    // Listed again before each push: the sidebar's own read of the host's computers, which lists no lab-box, can land late.
    listed(cx, vec![waiting.clone()]);
    hear(cx, wait.clone());
    listed(cx, vec![waiting.clone()]);
    hear(cx, wait);
    for (end, said) in [("needs-you", "a code"), ("ready", "rg failed"), ("failed", "no\nroute")] {
        listed(cx, vec![waiting.clone()]);
        setup_ended(cx, "s_out", end, Some(said));
    }
    let job = json!({ "id": "j_out", "phase": "done", "place": { "id": "box", "name": "Boat" }, "golden": { "version": 3 } });
    hear(cx, json!({ "type": "init.job", "job": job }));
    job_read(cx, json!({ "id": "j_need", "phase": "building", "place": { "id": "box", "name": "Boat" } }));
    job_moved(cx, Change::JobNeed(json!({ "what": "Sign in to GitHub" })));
    job_moved(cx, Change::Workspaces(vec![json!({ "id": "ws_api", "kind": "cloud", "name": "api" })]));
    hear(cx, json!({ "type": "workspace.woken", "workspaceId": "ws_api" }));
    let shown = cx.shown_system_notifications();
    let said: Vec<(String, String)> = shown.iter().map(|n| (n.title.to_string(), n.body.to_string())).collect();
    let want = [
        ("wsp needs you", "lab-box: GitHub waits on you to sign in"),
        ("wsp needs you", "lab-box needs you: a code"),
        ("lab-box is ready; rg failed", ""),
        ("lab-box: setup failed", "no route"),
        ("Boat is ready", "Image v3 sealed"),
        ("wsp needs you", "Sign in to GitHub"),
        ("wsp needs you", "api is awake"),
    ];
    assert_eq!(said, want.map(|(t, b)| (t.to_owned(), b.to_owned())));
    cx.simulate_system_notification_response(SystemNotificationResponse { tag: shown[0].tag.clone(), action_id: None });
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| dialogs::layer::top(cx)), Some(dialogs::ADD_COMPUTER), "the sign-in's click opens Add a computer");
}
