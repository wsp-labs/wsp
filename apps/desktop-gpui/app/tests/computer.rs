// SPDX-License-Identifier: AGPL-3.0-only
//! The Computer pane in the app's own frame: the shell connected to a host started for these tests alone, its right
//! panel open on the pane, which the registry builds under the Computer pane's name and which reads the sidebar the
//! shell holds; the readings the panes share; and DaemonDown's Start again against that host's own answer.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::cell::RefCell;
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::computer::daemon_down::{DaemonDown, Restart};
use desktop_gpui::screens::computer::live::{self, Target};
use desktop_gpui::screens::computer::model::{Sample, Usage, LIVE_WINDOW};
use desktop_gpui::screens::computer::{Computer, Shown};
use desktop_gpui::screens::{self, entry, Entry, Host, Mounted, Opened, Pane, Place, Route};
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{div, prelude::*, px, size, App, Entity, Modifiers, TestAppContext, VisualTestContext, Window};
use serde_json::json;
use wsp_host_client::protocol::WorkspacesRestartDaemonRequest;
use wsp_host_client::HostClient;

const WAIT: Duration = Duration::from_secs(20);

fn host_of(throwaway: &common::Throwaway) -> Host {
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(runtime.block_on(throwaway.client()));
    Host::new(client, runtime)
}

fn init(cx: &mut TestAppContext) {
    // The host's answers wake the window's tasks from the tokio runtime's thread.
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        screens::palette::init(cx);
    });
}

thread_local! {
    static MOUNTED: RefCell<Option<Entity<Computer>>> = const { RefCell::new(None) };
}

/// The registry's own mount for the pane, noting the screen it built so the test can read it.
fn mount_and_note(host: &Host, opened: Opened, window: &mut Window, cx: &mut App) -> Mounted {
    let mounted = (entry::<Computer>().mount)(host, opened, window, cx);
    MOUNTED.with(|m| *m.borrow_mut() = mounted.view.clone().downcast::<Computer>().ok());
    mounted
}

/// The shell on `throwaway` in a 1280 by 800 window, connected, its panel open on the Computer pane of `thread`, or
/// of this computer with none.
fn open<'a>(
    cx: &'a mut TestAppContext,
    throwaway: &common::Throwaway,
    thread: Option<(String, String)>,
) -> (Entity<Shell>, Entity<Computer>, &'a mut VisualTestContext) {
    let registered = screens::screen(Pane::Computer.name()).expect("the Computer pane registers under its pane's name");
    assert_eq!(registered.place, Place::Panel(Pane::Computer));
    screens::add("computer", Entry { place: Place::Panel(Pane::Computer), mount: mount_and_note, closed: registered.closed });
    init(cx);
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap());
    let (state, env) = (throwaway.state(), throwaway.env());
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
        shell = Some(view.clone());
        Root::new(view, window, cx)
    });
    let shell = shell.unwrap();
    cx.simulate_resize(size(px(1280.), px(800.)));
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    shell.update_in(cx, |s, window, cx| {
        if let Some((workspace_id, thread_id)) = thread {
            s.go(Route::Thread { workspace_id, thread_id }, window, cx);
        }
        s.panel().update(cx, |p, cx| p.add(Pane::Computer, None, window, cx));
    });
    cx.run_until_parked();
    let pane = MOUNTED.with(|m| m.borrow().clone()).expect("the panel built the Computer pane");
    (shell, pane, cx)
}

fn shown(pane: &Entity<Computer>, cx: &mut VisualTestContext) -> Shown {
    cx.update(|_, cx| pane.read(cx).shown(cx))
}

/// Waits, drawing, until `done` holds or `WAIT` passes.
fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let start = Instant::now();
    while !done(cx) {
        assert!(start.elapsed() < WAIT, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
        cx.run_until_parked();
    }
}

/// The workspace and thread of the host's first thread, which runs in this computer's folder: a local workspace.
fn first(host: &Host) -> (String, String) {
    let client = host.client().clone();
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let first = runtime.block_on(common::first(&client));
    (first.workspace_id, first.thread_id)
}

#[gpui_kit::test]
fn this_computers_pane_draws_its_facts_and_waits_for_its_readings(cx: &mut TestAppContext) {
    let shared = common::shared();
    let (_shell, pane, cx) = open(cx, &shared.host, None);
    assert_eq!(cx.update(|_, cx| pane.read(cx).target().clone()), Target::Here);
    until(cx, "this computer's facts", |cx| !shown(&pane, cx).chips.is_empty());
    let drawn = shown(&pane, cx);
    let words: Vec<&str> = drawn.chips.iter().map(|(text, _)| text.as_str()).collect();
    assert!(words.iter().any(|w| w.ends_with(" cores") || w.ends_with(" core")), "its cores: {words:?}");
    assert!(words.iter().any(|w| w.ends_with(" GB")), "its memory: {words:?}");
    assert!(drawn.footer.is_empty() && cx.debug_bounds("computer-footer").is_none(), "nothing to act on");
    assert_eq!(drawn.path, None);
    // A host with no daemon staged gives this computer no readings, so each slot waits on its first.
    for plot in &drawn.plots {
        assert_eq!((plot.figure.as_str(), plot.dimmed), ("pending", false), "{:?}", plot.plot);
    }
    for key in ["chart-load", "chart-mem", "chart-disk"] {
        let chart = cx.debug_bounds(key).unwrap_or_else(|| panic!("{key}"));
        assert_eq!(chart.size.height, px(96.));
    }
    // `py-1`, the 19.5 px figure row, `mt-3`, the 96 px plot, `mt-1.5` and the 16.5 px ends, inside `px-5`.
    let (plot, chart, pane_box) =
        (cx.debug_bounds("plot-load").unwrap(), cx.debug_bounds("chart-load").unwrap(), cx.debug_bounds("computer-pane").unwrap());
    assert_eq!(plot.size.height, px(4. + 19.5 + 12. + 96. + 6. + 16.5 + 4.));
    assert_eq!((chart.origin.x - pane_box.origin.x, chart.size.width), (px(20.), pane_box.size.width - px(40.)));
    assert_eq!(cx.debug_bounds("plot-mem").unwrap().origin.y - (plot.origin.y + plot.size.height), px(32.), "gap-8 between plots");
    let facts = cx.debug_bounds("facts").unwrap();
    let last = cx.debug_bounds("plot-disk").unwrap();
    assert_eq!(facts.origin.y - (last.origin.y + last.size.height), px(20.), "gap-5 before the facts");
}

/// A claude that answers what the host asks of one before a turn, and ends each turn at once.
const STAND_IN: &str = r#"#!/bin/sh
case " $* " in
  *" --version "*) echo "2.1.295 (Claude Code)"; exit 0 ;;
  *" --help "*) echo '--permission-mode <mode>  (choices: "default", "bypassPermissions")'; exit 0 ;;
  *" --bare "*) read -r _; echo '{"type":"control_response","response":{"subtype":"success","request_id":"init","response":{"models":[{"value":"default","resolvedModel":"claude-haiku-5-5"}]}}}'; exit 0 ;;
  *" --input-format stream-json "*) ;;
  *) exit 1 ;;
esac
session=""; prev=""
for a in "$@"; do [ "$prev" = "--session-id" ] && session=$a; prev=$a; done
while IFS= read -r _; do
  printf '{"type":"system","subtype":"init","session_id":"%s","model":"claude-haiku-5-5","cwd":"/","tools":[]}\n' "$session"
  printf '{"type":"result","subtype":"success","is_error":false,"result":"ok","session_id":"%s"}\n' "$session"
done
"#;

/// A host carrying a wsp-daemon, with a project on this computer whose first thread ran; none, said, where this
/// checkout has no daemon to carry.
fn with_daemon() -> Option<(common::Throwaway, std::path::PathBuf)> {
    if let Err(why) = common::wsp_daemon() {
        eprintln!("skipped: {why}");
        return None;
    }
    let host = common::Throwaway::start_with(STAND_IN, true);
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    Some((host, folder))
}

#[gpui_kit::test]
fn a_local_threads_pane_is_pending_then_draws_the_hosts_readings(cx: &mut TestAppContext) {
    let Some((throwaway, folder)) = with_daemon() else { return };
    let host = host_of(&throwaway);
    let (workspace, thread) = first(&host);
    let (shell, pane, cx) = open(cx, &throwaway, Some((workspace.clone(), thread)));
    until(cx, "the workspace's facts", |cx| shown(&pane, cx).path.is_some());
    let first = shown(&pane, cx);
    if first.plots[0].points.is_empty() {
        for plot in &first.plots {
            assert!(plot.figure == "pending" || plot.figure == "unreachable", "{plot:?}");
        }
    }
    assert_eq!(first.path.as_deref(), Some(folder.to_string_lossy().as_ref()));
    let ids: Vec<&str> = first.footer.iter().map(|b| b.id()).collect();
    assert!(ids.contains(&"delete") && !ids.contains(&"phase"), "this computer is not paused or woken: {ids:?}");
    assert!(cx.debug_bounds("computer-footer").is_some());

    until(cx, "a sample from the host", |cx| !shown(&pane, cx).plots[0].points.is_empty());
    let drawn = shown(&pane, cx);
    let load = &drawn.plots[0];
    assert!(!load.dimmed, "a live reading draws its dot");
    assert_eq!(load.points.last().unwrap().0, 1., "the newest reading takes the last slot");
    assert!(load.figure.split('.').nth(1).is_some_and(|d| d.len() == 2), "load to two places: {}", load.figure);
    for plot in &drawn.plots[1..] {
        assert!(plot.figure.contains(" of ") && plot.figure.ends_with('B'), "used of total: {}", plot.figure);
    }

    // Its tab closed, the pane lets go of the host's ask, and the readings stop.
    let live = cx.update(|_, cx| live::live(cx));
    shell.update_in(cx, |s, window, cx| s.panel().update(cx, |p, cx| p.close(0, window, cx)));
    drop(pane);
    MOUNTED.with(|m| m.borrow_mut().take());
    // The frame drawn before the close still holds the pane's view until the next one is drawn.
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(live.read_with(cx, |l, _| l.holders(&workspace)), 0);
    let at = |cx: &mut VisualTestContext| live.read_with(cx, |l, _| l.readings(&workspace).ring.last().map(|s| s.at));
    let before = at(cx);
    std::thread::sleep(Duration::from_secs(5));
    cx.run_until_parked();
    assert_eq!(at(cx), before, "no reading arrives for five seconds once the pane is gone");
}

#[gpui_kit::test]
fn panes_on_one_workspace_share_the_hosts_ask_until_the_last_lets_go(cx: &mut TestAppContext) {
    let Some((throwaway, _)) = with_daemon() else { return };
    let host = host_of(&throwaway);
    let (workspace, _) = first(&host);
    init(cx);
    let (_, cx) = cx.add_window_view(|_, _| gpui_kit::Empty);
    let live = cx.update(|_, cx| live::live(cx));
    let count = |cx: &mut VisualTestContext| live.read_with(cx, |l, _| l.readings(&workspace).ring.len());
    // The pane a thread switch builds holds before the one it replaces lets go.
    live.update(cx, |l, cx| l.hold_host(&host, &workspace, cx));
    live.update(cx, |l, cx| l.hold_host(&host, &workspace, cx));
    until(cx, "a sample", |cx| count(cx) > 0);
    live.update(cx, |l, cx| l.release_host(&host, &workspace, cx));
    let held = count(cx);
    until(cx, "a sample after one holder let go", |cx| count(cx) > held);
    live.update(cx, |l, cx| l.release_host(&host, &workspace, cx));
    assert_eq!(live.read_with(cx, |l, _| l.holders(&workspace)), 0);
}

#[gpui_kit::test]
fn this_computers_pane_reads_its_own_daemon(cx: &mut TestAppContext) {
    let Some((throwaway, _)) = with_daemon() else { return };
    let (_shell, pane, cx) = open(cx, &throwaway, None);
    until(cx, "a sample down this computer's daemon", |cx| !shown(&pane, cx).plots[0].points.is_empty());
    let drawn = shown(&pane, cx);
    assert!(drawn.plots.iter().all(|p| !p.dimmed && p.figure != "pending"), "{:?}", drawn.plots);
    assert!(drawn.plots[2].figure.contains(" of "), "{}", drawn.plots[2].figure);
}

#[gpui_kit::test]
fn a_local_thread_whose_host_refuses_its_readings_says_unreachable_and_runs_its_acts_through_the_sidebar(cx: &mut TestAppContext) {
    let shared = common::shared();
    let host = host_of(&shared.host);
    let (shell, pane, cx) = open(cx, &shared.host, Some(first(&host)));
    until(cx, "the refusal", |cx| shown(&pane, cx).plots[0].figure == "unreachable");
    let drawn = shown(&pane, cx);
    assert!(drawn.plots.iter().all(|p| p.dimmed && p.points.is_empty()), "{:?}", drawn.plots);
    // With no daemon staged this computer's own daemon is down: the state slot says so and Start again leads.
    assert_eq!(drawn.chips[0].0, "Stopped");
    let ids: Vec<&str> = drawn.footer.iter().map(|b| b.id()).collect();
    assert_eq!(ids, ["start-daemon", "bring-back", "export-project", "delete"]);
    assert_eq!(drawn.footer[0].word, "Start again");
    assert_eq!(drawn.footer[3].tooltip(), Some("Its computer is left as it is"));

    // Delete opens the sidebar's own end-workspace dialog, as its menu's Delete task does.
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    assert_eq!(sidebar.read_with(cx, |s, _| s.dialog_words()), None);
    let delete = cx.debug_bounds("footer-delete").expect("the Delete button");
    cx.simulate_click(delete.center(), Modifiers::none());
    cx.run_until_parked();
    let words = sidebar.read_with(cx, |s, _| s.dialog_words()).expect("the delete dialog");
    assert!(words[0].starts_with("Delete ") && words[0].ends_with('?'), "{words:?}");
}

#[gpui_kit::test]
fn readings_kept_since_the_pane_closed_draw_at_once(cx: &mut TestAppContext) {
    let shared = common::shared();
    init(cx);
    let key = Target::Here.key();
    cx.update(|cx| {
        let live = live::live(cx);
        live.update(cx, |live, _| {
            for i in 0..LIVE_WINDOW + 5 {
                let sample = Sample { load1: i as f64 / 10., mem: Usage { used: 1., total: 2. }, ..Sample::default() };
                live.keep(&key, |r| r.ring.push(sample));
            }
            for other in 0..live::KEPT + 3 {
                live.keep(&format!("other-{other}"), |r| r.ring.push(Sample::default()));
            }
            assert_eq!(live.kept_count(), live::KEPT, "readings are kept for at most {} computers", live::KEPT);
            assert!(live.readings(&key).ring.is_empty(), "the computer read longest ago goes first");
            for _ in 0..3 {
                live.keep(&key, |r| r.ring.push(Sample { load1: 0.5, ..Sample::default() }));
            }
        });
    });
    let (_shell, pane, cx) = open(cx, &shared.host, None);
    assert_eq!(shown(&pane, cx).plots[0].points.len(), 3, "the kept readings draw before any new one arrives");
}

/// The piece as a pane draws it, holding the ask's state where the test reads it.
struct Down {
    host: Host,
    workspace: String,
    state: Entity<Restart>,
}

impl Render for Down {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        let down = DaemonDown::new(self.host.clone(), self.workspace.clone(), "mini's terminals and files stopped", Some("Start again"));
        div().size_full().flex().child(down.state(self.state.clone()))
    }
}

#[gpui_kit::test]
fn start_again_asks_the_host_and_says_its_refusal_under_the_button(cx: &mut TestAppContext) {
    let shared = common::shared();
    let host = host_of(&shared.host);
    init(cx);
    let workspace = first(&host).0;
    let expected = {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let client: HostClient = shared.host.client().await;
            let ask: WorkspacesRestartDaemonRequest = serde_json::from_value(json!({ "workspaceId": workspace })).unwrap();
            client.call(ask).await.err().map(|f| f.message)
        })
    };
    let refused = expected.expect("a host with no daemon staged refuses to start this computer's");
    let mut state = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let held = cx.new(|_| Restart::default());
        state = Some(held.clone());
        let down = cx.new(|_| Down { host: host.clone(), workspace: workspace.clone(), state: held });
        Root::new(down, window, cx)
    });
    let state = state.unwrap();
    cx.simulate_resize(size(px(540.), px(400.)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("daemon-down").is_some());
    assert!(cx.debug_bounds("start-daemon-refused").is_none());
    // Every state the ask passes through, since a host this near can answer inside the click.
    let held = std::rc::Rc::new(std::cell::Cell::new(false));
    let _seen = cx.update({
        let (state, held) = (state.clone(), held.clone());
        move |_, cx| cx.observe(&state, move |s, cx| held.set(held.get() || s.read(cx).busy))
    });
    let button = cx.debug_bounds("button").expect("Start again");
    cx.simulate_click(button.center(), Modifiers::none());
    assert!(held.get(), "the button holds while the ask is out");
    until(cx, "the host's answer", |cx| !state.read_with(cx, |s, _| s.busy));
    let line = state.read_with(cx, |s, _| s.refusal.clone()).expect("a refusal");
    assert!(line.starts_with(refused.trim_end_matches('.')), "{line} says the host's {refused}");
    assert!(line.ends_with("Start again."), "{line}");
    assert!(cx.debug_bounds("start-daemon-refused").is_some(), "drawn under the button");
}
