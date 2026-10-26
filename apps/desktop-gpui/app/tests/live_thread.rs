// SPDX-License-Identifier: AGPL-3.0-only
//! A thread whose agent keeps working, open in the shell against a host started for this test alone: each step of a
//! piece that moves (a spinner's turn, a crab's pose, the Working clock's tick, the shimmer's frame) draws that piece
//! alone, and nothing around it renders: not the shell, the sidebar, the right panel, the thread screen or its rows.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::frames;
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::thread_view::ThreadView;
use desktop_gpui::transcript::RowKind;
use desktop_gpui::ui::{self, FrozenNow, Side};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, Entity, TestAppContext, VisualTestContext};
use wsp_host_client::protocol::SessionEvent;
use wsp_host_client::HostClient;

/// A claude stand-in that never ends its turn: on a prompt saying spawn it starts a child thread through wsp, which
/// works on as well, and runs a command it never hears back from; every other prompt just thinks.
const WORKS: &str = r#"#!/bin/sh
case " $* " in
  *" --version "*) echo "2.1.295 (Claude Code)"; exit 0 ;;
  *" --help "*) echo '--permission-mode <mode>  (choices: "default", "bypassPermissions")'; exit 0 ;;
  *" --bare "*) read -r _; echo '{"type":"control_response","response":{"subtype":"success","request_id":"init","response":{"models":[{"value":"default","resolvedModel":"claude-haiku-5-5"}]}}}'; exit 0 ;;
  *" --input-format stream-json "*) ;;
  *) exit 1 ;;
esac
session=""; prev=""
for a in "$@"; do [ "$prev" = "--session-id" ] && session=$a; prev=$a; done
while IFS= read -r line; do
  printf '{"type":"system","subtype":"init","session_id":"%s","model":"claude-haiku-5-5","cwd":"/","tools":[]}\n' "$session"
  case "$line" in
    *spawn*)
      node "$HOME/hostpkg/dist/bin.js" run repo --agent claude --notify me --detach --title Kid "kid works" --state "$HOME/state/state.json" >/dev/null 2>&1
      printf '{"type":"assistant","session_id":"%s","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"sleep 300"}}]}}\n' "$session" ;;
  esac
  exec sleep 300
done
"#;

/// The views a step of a piece must not render.
const AROUND: [&str; 5] = ["Shell", "Sidebar", "RightPanel", "ThreadScreen", "Rows"];

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + Duration::from_secs(60);
    while !done(cx) {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
        cx.run_until_parked();
    }
}

fn view_of(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<Entity<ThreadView>> {
    shell.read_with(cx, |s, cx| s.thread_screen().map(|screen| screen.read(cx).view().clone()))
}

fn rows_hold(view: &Entity<ThreadView>, cx: &mut VisualTestContext, kind: impl Fn(&RowKind) -> bool) -> bool {
    view.read_with(cx, |v, _| v.rows().iter().any(|r| kind(&r.kind)))
}

/// The shell in a 1280 by 800 window drawn as the app draws it, open on the lead thread while it and its child work,
/// the right panel open; the counts start from nothing.
fn opened(cx: &mut TestAppContext) -> (common::Throwaway, Entity<ThreadView>, &mut VisualTestContext) {
    let host = common::Throwaway::start_with(WORKS, false);
    host.wsp(&["add", &host.repo("repo").to_string_lossy()]);
    host.wsp(&["run", "repo", "spawn a kid", "--agent", "claude", "--detach"]);
    let ran = Instant::now();
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap());
    let client = runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let lead = threads.iter().find(|t| t.prompt.as_deref() == Some("spawn a kid")).expect("the lead thread");
    let (workspace_id, thread_id) = (lead.workspace_id.clone(), lead.thread_id.clone().expect("a thread id"));
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
        desktop_gpui::screens::palette::init(cx);
        desktop_gpui::screens::composer::init(cx);
    });
    // The sidebar's times past 10 s take no other width for the next 50 s, which no step of these tests reaches.
    std::thread::sleep(Duration::from_secs(12).saturating_sub(ran.elapsed()));
    let (state, env) = (host.state(), host.env());
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
        shell = Some(view.clone());
        frames::watch(window, cx);
        Root::new(view, window, cx).cache_view()
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    let shell = shell.unwrap();
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id, thread_id }, window, cx));
    until(cx, "the lead's running command and its child at work", |cx| {
        view_of(&shell, cx).is_some_and(|v| {
            rows_hold(&v, cx, |k| matches!(k, RowKind::Live { active: true, .. }))
                && rows_hold(&v, cx, |k| matches!(k, RowKind::Working { .. }))
                && v.read_with(cx, |v, _| v.listing().is_some_and(|l| l.children.iter().any(|c| c.running)))
        })
    });
    let view = view_of(&shell, cx).unwrap();
    cx.update(|window, cx| {
        let panel = shell.read(cx).panel().clone();
        panel.update(cx, |p, cx| p.toggle(window, cx));
    });
    cx.run_until_parked();
    assert!(frames::take().contains_key("RightPanel"), "the right panel is open");
    // The panel slides open over frames of its own on the wall clock and what it shows lands: frames go on until
    // none has rendered the shell for 600 ms, or for 5 s where every one does (the shimmer's, drawn not in place).
    let (mut quiet, end) = (Instant::now(), Instant::now() + Duration::from_secs(5));
    while quiet.elapsed() < Duration::from_millis(600) && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(16));
        next_frame(cx);
        cx.run_until_parked();
        if frames::take().contains_key("Shell") {
            quiet = Instant::now();
        }
    }
    frames::take();
    (host, view, cx)
}

/// Delivers the next frame 16 ms on, as a display would: an animation that eases on the clock moves on with it.
fn next_frame(cx: &mut VisualTestContext) {
    cx.executor().advance_clock(Duration::from_millis(16));
    cx.update(|window, cx| _ = window.simulate_next_frame(cx));
}

/// How many of `piece` the window draws: each renders once when the whole window draws again.
fn drawn(piece: &str, cx: &mut VisualTestContext) -> u64 {
    frames::take();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let n = frames::take().get(piece).copied().unwrap_or(0);
    assert!(n > 0, "no {piece} on screen");
    n
}

/// Runs `step` until one draws `piece`, and returns what that step rendered.
fn step_of(piece: &str, cx: &mut VisualTestContext, mut step: impl FnMut(&mut VisualTestContext)) -> BTreeMap<&'static str, u64> {
    for _ in 0..200 {
        step(cx);
        cx.run_until_parked();
        let counts = frames::take();
        if counts.contains_key(piece) {
            return counts;
        }
    }
    panic!("no step drew {piece}");
}

fn assert_around_untouched(piece: &str, counts: &BTreeMap<&'static str, u64>) {
    for view in AROUND {
        assert!(!counts.contains_key(view), "{view} rendered for a step of the {piece}: {counts:?}");
    }
}

fn assert_alone(piece: &str, times: u64, counts: &BTreeMap<&'static str, u64>) {
    assert_around_untouched(piece, counts);
    assert_eq!(counts.get(piece), Some(&times), "each {piece} rendered once: {counts:?}");
}

#[gpui_kit::test]
fn one_crab_pose_renders_the_crabs_alone(cx: &mut TestAppContext) {
    let (_host, view, cx) = opened(cx);
    let kid = view.read_with(cx, |v, _| v.listing().and_then(|l| l.children.first()).map(|c| c.at.thread.clone())).unwrap();
    let row: &'static str = Box::leak(format!("tree-row-{kid}-title").into_boxed_str());
    assert!(cx.debug_bounds(row).is_some(), "the working child's row, its crab in the transcript's rows, is drawn");
    let crabs = drawn("Crab", cx);
    for _ in 0..3 {
        let counts = step_of("Crab", cx, |cx| cx.executor().advance_clock(Duration::from_millis(10)));
        assert_around_untouched("Crab", &counts);
        // A crab drawn anew while the host's news lands walks out of step with the rest until its next pose.
        assert!(counts["Crab"] <= crabs, "a crab rendered twice for a pose, of {crabs} drawn: {counts:?}");
    }
}

#[gpui_kit::test]
fn one_spinner_step_renders_the_spinner_alone(cx: &mut TestAppContext) {
    let (_host, view, cx) = opened(cx);
    let group = view.read_with(cx, |v, _| {
        v.rows().iter().find_map(|r| match &r.kind {
            RowKind::Live { group, .. } => Some(group.clone()),
            _ => None,
        })
    });
    view.update(cx, |v, cx| v.toggle(&group.expect("the live line"), cx));
    until(cx, "the running command's spinner", |cx| {
        cx.executor().advance_clock(Duration::from_millis(34));
        cx.run_until_parked();
        frames::take().contains_key("Spinner")
    });
    let spinners = drawn("Spinner", cx);
    for _ in 0..3 {
        let counts = step_of("Spinner", cx, |cx| cx.executor().advance_clock(Duration::from_millis(34)));
        assert_alone("Spinner", spinners, &counts);
    }
}

/// Waits for the app to settle while the Working clock reads a frozen now: the test clock a minute on, so the timers
/// that run on it have fired and every sidebar time has passed its seconds; then the preferences in the store and a
/// second and a half of the wall clock in which nothing around the thread rendered, so a read that failed has been
/// read again and has landed.
fn settle(cx: &mut VisualTestContext) {
    for _ in 0..61 {
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
    }
    let (mut quiet, end) = (Instant::now(), Instant::now() + Duration::from_secs(60));
    while quiet.elapsed() < Duration::from_millis(1_500) {
        assert!(Instant::now() < end, "something around the thread went on rendering for a minute");
        std::thread::sleep(Duration::from_millis(50));
        cx.run_until_parked();
        let counts = frames::take();
        if AROUND.iter().any(|view| counts.contains_key(view)) || cx.update(|_, cx| desktop_gpui::prefs::shown(cx).is_none()) {
            quiet = Instant::now();
        }
    }
}

#[gpui_kit::test]
fn one_tick_of_the_working_clock_renders_the_clock_alone(cx: &mut TestAppContext) {
    let (_host, _, cx) = opened(cx);
    // Nothing else moves, and the clock reads a now the test moves.
    let mut now = 2_000_000_000_000.;
    cx.update(|_, cx| {
        cx.set_reduce_motion(true);
        cx.set_global(FrozenNow(now));
    });
    settle(cx);
    let clocks = drawn("Clock", cx);
    for _ in 0..3 {
        frames::take();
        let counts = step_of("Clock", cx, |cx| {
            now += 1_000.;
            cx.update(|_, cx| cx.set_global(FrozenNow(now)));
            cx.executor().advance_clock(Duration::from_millis(1_100));
        });
        assert_alone("Clock", clocks, &counts);
    }
}

#[gpui_kit::test]
fn one_shimmer_frame_renders_the_shimmer_alone(cx: &mut TestAppContext) {
    let (_host, _, cx) = opened(cx);
    let shimmers = drawn("Shimmer", cx);
    for _ in 0..3 {
        let counts = step_of("Shimmer", cx, next_frame);
        assert_alone("Shimmer", shimmers, &counts);
    }
}

fn event(turn: &str, kind: &str, pos: u64, extra: serde_json::Value) -> SessionEvent {
    let mut v = serde_json::json!({"workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": turn, "type": kind, "pos": pos});
    v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}

/// Past an hour the Working clock reads `1h 4m`, which a second does not change: a second draws nothing at all.
#[gpui_kit::test]
fn a_tick_that_leaves_the_clock_reading_the_same_draws_nothing(cx: &mut TestAppContext) {
    let started = 1_000.;
    let mut now = started + 3_850_000.;
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        cx.set_reduce_motion(true);
        cx.set_global(FrozenNow(now));
    });
    let view = cx.new(ThreadView::new);
    let root = view.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        frames::watch(window, cx);
        Root::new(root, window, cx).cache_view()
    });
    cx.simulate_resize(size(px(900.), px(600.)));
    let events = vec![
        event("u0", "session.start", 1, serde_json::json!({"prompt": "Question"})),
        event("u0", "session.delta", 2, serde_json::json!({"kind": "text", "text": "Answer."})),
        event("u0", "session.done", 3, serde_json::json!({"result": {"status": "completed"}})),
        event("u0", "session.end", 4, serde_json::json!({"exitCode": 0, "sawResult": true})),
        event("live", "session.start", 10, serde_json::json!({"prompt": "Keep going", "at": started})),
    ];
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(events, cx);
        v.scroll_to_end(cx);
    });
    cx.run_until_parked();
    drawn("Clock", cx);
    for second in 0..5 {
        now += 1_000.;
        cx.update(|_, cx| cx.set_global(FrozenNow(now)));
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        let counts = frames::take();
        assert!(counts.is_empty(), "second {second} past 1h 4m 10s drew {counts:?}");
    }
}
