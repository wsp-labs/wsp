// SPDX-License-Identifier: AGPL-3.0-only
//! The Processes pane against a host that answers only what each test says (`tests/settings/door.mjs`), so the
//! daemon's process frames, its refusals and its answers to a kill come on cue: the registry builds the pane under its
//! pane's name in a window whose layers hold a sidebar fed the workspace records, places and statuses by hand. Needs
//! node and the packages installed.

#[path = "../src/screens/settings/door.rs"]
mod door;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::processes::table::{Act, LitShown, Shown};
use desktop_gpui::screens::processes::tree::{Sort, ARM, ESCALATE};
use desktop_gpui::screens::processes::{Processes, ThreadState};
use desktop_gpui::screens::sidebar::host::Change;
use desktop_gpui::screens::{self, palette, Host, Opened, Pane, Place};
use desktop_gpui::sidebar::Sidebar;
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{div, prelude::*, px, size, AnyView, Context, Entity, TestAppContext, VisualTestContext, Window};
use serde_json::{json, Value};
use wsp_host_client::HostClient;

use door::Door;

const WAIT: Duration = Duration::from_secs(20);
const HERE_NAME: &str = "mac";

struct Holder(Option<AnyView>);

impl Render for Holder {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // The right panel's default width in a wide window, so what reads the pane's width cannot read the window's.
        div().h_full().w(px(540.)).children(self.0.clone())
    }
}

fn row(pid: i64, ppid: i64, cmd: &str, cpu: f64, rss: f64) -> Value {
    let comm = cmd.split(' ').next().unwrap_or_default().rsplit('/').next().unwrap_or_default();
    json!({ "pid": pid, "ppid": ppid, "user": "bench", "state": "S", "comm": comm, "cmdline": cmd, "cpu": cpu, "rss": rss,
        "startedAt": 1_700_000_000_000i64 })
}

fn snapshot(procs: Vec<Value>, total: usize) -> Value {
    json!({ "type": "daemon.event", "channel": "c1",
        "event": { "type": "proc.snapshot", "at": 1000, "daemon": 1, "total": total, "procs": procs, "seq": 1 } })
}

/// A small tree: the daemon at 1, a shell at 10 over a busy server at 11 and an idle one at 12, a heavy loner at 20.
fn tree() -> Vec<Value> {
    vec![
        row(1, 0, "wsp-daemon", 0.5, 30e6),
        row(10, 1, "bash", 0.0, 950e6),
        row(11, 10, "python3 -m http.server needle", 9.0, 20e6),
        row(12, 10, "sleep 60", 0.1, 1e6),
        row(20, 1, "node build.js", 1.0, 900e6),
    ]
}

fn rules(push: Vec<Value>, more: Value) -> Value {
    let mut rules = json!({
        "daemon.open": { "reply": { "channel": "c1" } },
        "daemon.close": { "reply": {} },
        "daemon.send": { "reply": { "reply": { "ok": true } }, "push": push },
        "workspaces.list": { "reply": { "workspaces": [] } },
        "sessions.list": { "reply": { "sessions": [] } },
        "appProcs.watch": { "reply": { "procs": [] } },
        "appProcs.unwatch": { "reply": {} },
    });
    for (k, v) in more.as_object().cloned().unwrap_or_default() {
        rules[k] = v;
    }
    rules
}

fn place(id: &str, name: &str, kind: &str, present: bool) -> Value {
    json!({ "id": id, "kind": kind, "name": name, "default": id == "here", "present": present })
}

fn workspace(id: &str, kind: &str, place: Option<&str>) -> Value {
    let mut w = json!({ "id": id, "name": id, "kind": kind, "createdAt": "2026-10-01T00:00:00Z", "golden": "", "machineId": "",
        "phase": "running", "project": { "id": "p1", "name": "bench", "computer": "here", "path": "/src/bench" } });
    if let Some(place) = place {
        w["place"] = json!(place);
    }
    if kind == "cloud" {
        w["provider"] = json!("cloudco");
    }
    w
}

fn turn(id: &str, thread: &str, workspace: &str, pid: Option<i64>, running: bool, parent: Option<&str>) -> Value {
    let mut t = json!({ "id": id, "threadId": thread, "workspaceId": workspace, "harness": "claude", "prompt": format!("Thread {thread}."),
        "status": if running { "running" } else { "completed" }, "startedAt": 1_700_000_000_000i64 });
    if let Some(pid) = pid {
        t["pid"] = json!(pid);
    }
    if let Some(parent) = parent {
        t["parentThreadId"] = json!(parent);
    }
    t
}

struct Fixture<'a> {
    door: Door,
    runtime: Arc<tokio::runtime::Runtime>,
    /// The rules past the base ones, kept when a push changes the daemon's.
    more: Value,
    pane: Entity<Processes>,
    host: Host,
    holder: Entity<Holder>,
    _sidebar: Entity<Sidebar>,
    cx: &'a mut VisualTestContext,
}

/// The pane on `opened`, mounted by the registry under the Processes pane's name, its window's sidebar holding
/// `changes` before it opens.
fn open(cx: &mut TestAppContext, more: Value, opened: Opened, changes: Vec<Change>) -> Fixture<'_> {
    let entry = screens::screen(Pane::Processes.name()).expect("the Processes pane registers under its pane's name");
    assert_eq!(entry.place, Place::Panel(Pane::Processes));
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        palette::init(cx);
    });
    let door = Door::start(rules(vec![], more.clone()));
    let runtime = door::runtime();
    let host = Host::new(door.client(&runtime), runtime.clone());
    let sidebar = cx.new(Sidebar::new);
    sidebar.update(cx, |s, cx| {
        let mut all = vec![Change::Places(vec![place("here", HERE_NAME, "computer", true), place("cloudco", "Cloudco", "provider", true)])];
        all.extend(changes);
        for c in all {
            s.apply(c, cx);
        }
    });
    let holder = cx.new(|_| Holder(None));
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(holder.clone(), window, cx));
    cx.simulate_resize(size(px(1280.), px(800.)));
    let pane = cx.update(|window, cx| {
        let layers = palette::overlays::of(window, cx).expect("the layers stand on the window's root");
        layers.update(cx, |o, _| o.sidebar = Some(sidebar.downgrade()));
        let mounted = (entry.mount)(&host, opened, window, cx);
        holder.update(cx, |h, cx| {
            h.0 = Some(mounted.view.clone());
            cx.notify();
        });
        mounted.view.downcast::<Processes>().expect("the registry built the Processes screen")
    });
    cx.run_until_parked();
    Fixture { door, runtime, more, pane, host, holder, _sidebar: sidebar, cx }
}

impl Fixture<'_> {
    fn until(&mut self, what: &str, mut done: impl FnMut(&Processes, &gpui_kit::App) -> bool) {
        let start = Instant::now();
        loop {
            self.cx.run_until_parked();
            let pane = self.pane.clone();
            if self.cx.update(|_, cx| done(pane.read(cx), cx)) {
                return;
            }
            assert!(start.elapsed() < WAIT, "timed out waiting for {what}; the table: {:?}", self.table());
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    fn table(&mut self) -> Option<Shown> {
        let pane = self.pane.clone();
        self.cx.update(|_, cx| pane.read(cx).table(cx))
    }

    fn shown(&mut self) -> Shown {
        self.table().expect("the pane draws the computer table")
    }

    fn rows(&mut self) -> Vec<i64> {
        self.shown().rows.iter().map(|r| r.pid).collect()
    }

    fn thread_state(&mut self) -> Option<ThreadState> {
        let pane = self.pane.clone();
        self.cx.update(|_, cx| pane.read(cx).thread_state(cx))
    }

    /// Answers from now on by `more` over the base rules.
    fn answer(&mut self, more: Value) {
        for (k, v) in more.as_object().cloned().unwrap_or_default() {
            self.more[k] = v;
        }
        self.door.rules(rules(vec![], self.more.clone()));
    }

    /// The daemon pushes `event`: a frame from a client of the test's own makes the door say it to every socket.
    fn push(&mut self, event: Value) {
        self.door.rules(rules(vec![event], self.more.clone()));
        let client: Arc<HostClient> = self.door.client(&self.runtime);
        self.runtime.block_on(async move {
            let ask: wsp_host_client::protocol::DaemonSendRequest =
                serde_json::from_value(json!({ "channel": "elsewhere", "frame": { "op": "nothing" } })).unwrap();
            _ = client.call(ask).await;
        });
        self.door.rules(rules(vec![], self.more.clone()));
    }

    /// The tree's frame, and the pane drawing it.
    fn framed(&mut self, procs: Vec<Value>, total: usize) {
        let pids: Vec<i64> = procs.iter().filter_map(|p| p["pid"].as_i64()).collect();
        self.until("the link opens", |p, cx| p.live(cx));
        self.push(snapshot(procs, total));
        self.until("the frame", |p, cx| p.listed(cx) == Some(pids.clone()));
    }

    fn act(&mut self, f: impl FnOnce(&mut Processes, &mut Window, &mut Context<Processes>)) {
        let pane = self.pane.clone();
        self.cx.update(|window, cx| pane.update(cx, |p, cx| f(p, window, cx)));
        self.cx.run_until_parked();
    }

    fn lit(&mut self) -> LitShown {
        self.shown().lit.expect("a row is lit")
    }

    fn said(&mut self, what: &str, want: &str) {
        let start = Instant::now();
        while self.lit().said.as_deref() != Some(want) {
            assert!(start.elapsed() < WAIT, "{what}: the row said {:?}, not {want:?}", self.lit().said);
            std::thread::sleep(Duration::from_millis(30));
            self.cx.run_until_parked();
        }
    }

    fn advance(&mut self, by: Duration) {
        self.cx.executor().advance_clock(by);
        self.cx.run_until_parked();
    }

    fn kill_word(&mut self) -> Option<(String, bool)> {
        match self.lit().act {
            Act::Kill(offer) => offer,
            other => panic!("the lit row offers {other:?}, not kill"),
        }
    }

    /// The last `proc.kill` the pane sent its daemon.
    fn last_kill(&self) -> Value {
        self.door
            .asked_of("daemon.send")
            .into_iter()
            .rev()
            .find(|f| f["frame"]["op"] == "proc.kill")
            .map(|f| f["frame"].clone())
            .unwrap_or_default()
    }
}

#[gpui_kit::test]
fn the_count_says_pending_then_the_rows_and_n_of_m_under_a_filter_that_keeps_a_matchs_parents(cx: &mut TestAppContext) {
    let mut f = open(cx, json!({}), Opened::default(), vec![]);
    f.until("the link opens", |p, cx| p.live(cx));
    assert_eq!(f.shown().count, "pending", "no frame yet");
    f.framed(tree(), 5);
    assert_eq!(f.shown().count, "5 processes");
    assert_eq!(f.shown().rows[0].label.as_deref(), Some("wsp"), "the daemon's own row");
    let drawn = f.cx.debug_bounds("proc-row-11").expect("the row is drawn");
    assert_eq!(drawn.size.height, px(22.), "every row 22 px");
    assert!(f.cx.debug_bounds("procs-count").is_some() && f.cx.debug_bounds("procs-sort-cpu").is_some(), "the count and the head drawn");
    f.act(|p, window, cx| p.set_filter("needle", window, cx));
    assert_eq!(f.rows(), [1, 10, 11], "the match under its parents, and not its sibling");
    assert_eq!(f.shown().count, "3 of 5");
    assert_eq!(f.shown().rows.iter().map(|r| r.depth).collect::<Vec<_>>(), [0, 1, 2], "the tree's shape kept");
    f.act(|p, window, cx| p.set_filter("", window, cx));
    // The same pids as the frame before, so the new total is what tells the new frame has come.
    f.push(snapshot(tree(), 9));
    f.until("a machine with more than the frame lists", |p, cx| p.table(cx).is_some_and(|t| t.count == "5 of 9"));
}

#[gpui_kit::test]
fn a_long_command_ends_at_the_rows_edge(cx: &mut TestAppContext) {
    let mut f = open(cx, json!({}), Opened::default(), vec![]);
    let long = format!("node /srv/bench/server.js {}", "--flag-with-a-long-name ".repeat(20));
    f.framed(vec![row(1, 0, "wsp-daemon", 0.5, 30e6), row(7, 1, &long, 0.1, 1e6)], 2);
    let (command, row) = (f.cx.debug_bounds("proc-command-7").expect("the command is drawn"), f.cx.debug_bounds("proc-row-7").unwrap());
    assert!(command.right() <= row.right(), "the command's box shrinks to the row, where its text truncates: {command:?} in {row:?}");
    assert!(command.size.width > px(100.), "and still takes the room left: {command:?}");
}

#[gpui_kit::test]
fn a_refused_watch_reads_unavailable_with_the_daemons_reason(cx: &mut TestAppContext) {
    let refused =
        json!({ "daemon.send:proc.watch": { "reply": { "reply": { "ok": false, "error": "proc.watch is off on this daemon" } } } });
    let mut f = open(cx, refused, Opened::default(), vec![]);
    f.until("the refusal", |p, cx| p.table(cx).is_some_and(|t| t.unavailable.is_some()));
    let shown = f.shown();
    assert_eq!(shown.count, "unavailable");
    assert_eq!(shown.unavailable.as_deref(), Some("proc.watch is off on this daemon"));
}

#[gpui_kit::test]
fn siblings_sort_by_cpu_or_by_memory(cx: &mut TestAppContext) {
    let mut f = open(cx, json!({}), Opened::default(), vec![]);
    f.framed(tree(), 5);
    assert_eq!(f.shown().sort, Sort::Cpu, "cpu first, as the web opens");
    assert_eq!(f.rows(), [1, 20, 10, 11, 12], "the busiest first under each parent");
    f.act(|p, _, cx| p.set_sort(Sort::Mem, cx));
    assert_eq!(f.rows(), [1, 10, 11, 12, 20], "the heaviest first under each parent");
}

#[gpui_kit::test]
fn a_pane_built_again_on_its_tab_finds_its_session_until_the_tab_closes(cx: &mut TestAppContext) {
    let opened = Opened { tab: Some("processes".into()), ..Opened::default() };
    let mut f = open(cx, json!({}), opened.clone(), vec![]);
    f.framed(tree(), 5);
    f.act(|p, window, cx| {
        p.set_sort(Sort::Mem, cx);
        p.set_filter("needle", window, cx);
        p.toggle(11, cx);
    });
    let entry = screens::screen(Pane::Processes.name()).unwrap();
    let host = f.host.clone();
    let again = |f: &mut Fixture| {
        let (host, opened) = (host.clone(), opened.clone());
        f.cx.update(move |window, cx| {
            let view = (entry.mount)(&host, opened, window, cx).view.downcast::<Processes>().unwrap();
            view.read(cx).session().clone()
        })
    };
    let kept = again(&mut f);
    assert_eq!((kept.sort, kept.filter.as_str(), kept.lit), (Sort::Mem, "needle", Some(11)), "found again by its tab");
    f.cx.update(|_, cx| (entry.closed)(&host, opened.clone(), cx));
    assert_eq!(again(&mut f), desktop_gpui::screens::processes::Session::default(), "the closed tab's session went with it");
}

#[gpui_kit::test]
fn a_lit_row_restored_from_its_tab_reads_its_inspect_once_a_frame_lists_it(cx: &mut TestAppContext) {
    let opened = Opened { tab: Some("processes".into()), ..Opened::default() };
    let inspect = json!({ "daemon.send:proc.inspect": { "reply": { "reply": { "ok": true, "pid": 11, "cwd": "/srv/bench", "ports": [8000],
        "children": [], "threads": 3 } } } });
    let mut f = open(cx, inspect, opened.clone(), vec![]);
    f.framed(tree(), 5);
    f.act(|p, _, cx| p.toggle(11, cx));
    let asked = |f: &Fixture| f.door.asked_of("daemon.send").iter().filter(|a| a["frame"]["op"] == "proc.inspect").count();
    // The first pane's own ask lands before the count is taken.
    f.until("the first pane's inspect", |p, cx| {
        p.table(cx).and_then(|t| t.lit).is_some_and(|l| l.fields.iter().any(|(k, v)| *k == "cwd" && v == "/srv/bench"))
    });
    let before = asked(&f);
    // The pane built again after a thread switch, on a link that has no channel yet.
    let (entry, host, holder) = (screens::screen(Pane::Processes.name()).unwrap(), f.host.clone(), f.holder.clone());
    let rebuilt = f.cx.update(move |window, cx| {
        let view = (entry.mount)(&host, opened, window, cx).view;
        holder.update(cx, |h, cx| {
            h.0 = Some(view.clone());
            cx.notify();
        });
        view.downcast::<Processes>().unwrap()
    });
    f.pane = rebuilt;
    assert_eq!(f.cx.update(|_, cx| f.pane.read(cx).session().lit), Some(11), "the lit row came back with the tab");
    f.framed(tree(), 5);
    f.until("the inspect fields", |p, cx| {
        p.table(cx).and_then(|t| t.lit).is_some_and(|l| l.fields.iter().any(|(k, v)| *k == "cwd" && v == "/srv/bench"))
    });
    let fields = f.lit().fields;
    assert!(!fields.iter().any(|(k, _)| *k == "inspect"), "no refusal stands in the fields: {fields:?}");
    assert!(fields.contains(&("ports", "8000".to_owned())) && fields.contains(&("threads", "3".to_owned())));
    assert_eq!(asked(&f), before + 1, "asked once, on the first frame that listed it");
    assert_eq!(f.cx.debug_bounds("procs-field-cwd").map(|_| ()), Some(()), "and drawn");
}

#[gpui_kit::test]
fn closing_the_pane_lets_its_watch_and_its_channel_go(cx: &mut TestAppContext) {
    let mut f = open(cx, json!({}), Opened::default(), vec![]);
    f.framed(tree(), 5);
    assert!(f.door.asked_of("daemon.close").is_empty(), "the channel stands while the pane does");
    let Fixture { door, pane, holder, cx, .. } = f;
    holder.update(cx, |h, cx| {
        h.0 = None;
        cx.notify();
    });
    drop(pane);
    let start = Instant::now();
    loop {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let closed = door.asked_of("daemon.close").iter().any(|f| f["channel"] == "c1");
        let unwatched = door.asked_of("daemon.send").iter().any(|f| f["frame"]["op"] == "proc.unwatch");
        if closed && unwatched {
            break;
        }
        assert!(start.elapsed() < WAIT, "the pane went and its watch stayed: closed {closed}, unwatched {unwatched}");
        std::thread::sleep(Duration::from_millis(30));
    }
}

#[gpui_kit::test]
fn a_row_lights_on_a_click_and_on_enter_or_space(cx: &mut TestAppContext) {
    let mut f = open(cx, json!({}), Opened::default(), vec![]);
    f.framed(tree(), 5);
    let at = f.cx.debug_bounds("proc-row-11").expect("the row is drawn").center();
    f.cx.simulate_click(at, gpui_kit::Modifiers::none());
    f.cx.run_until_parked();
    assert_eq!(f.lit().pid, 11, "a click lights the row");
    f.cx.simulate_keystrokes("space");
    assert_eq!(f.shown().lit, None, "Space on the lit row puts it out");
    f.cx.simulate_keystrokes("enter");
    assert_eq!(f.lit().pid, 11, "Enter lights it again");
}

#[gpui_kit::test]
fn the_lit_row_goes_out_when_its_process_goes(cx: &mut TestAppContext) {
    let mut f = open(cx, json!({}), Opened::default(), vec![]);
    f.framed(tree(), 5);
    f.act(|p, _, cx| p.toggle(12, cx));
    assert_eq!(f.lit().pid, 12);
    let mut fewer = tree();
    fewer.retain(|p| p["pid"] != 12);
    f.framed(fewer, 4);
    assert_eq!(f.shown().lit, None, "nothing lit once 12 left the frame");
    assert_eq!(f.cx.update(|_, cx| f.pane.read(cx).session().lit), None, "and the tab's session forgot it");
}

#[gpui_kit::test]
fn kill_arms_for_two_seconds_then_ends_the_tree_and_says_what_the_daemon_did(cx: &mut TestAppContext) {
    let mut f = open(cx, json!({}), Opened::default(), vec![]);
    f.framed(tree(), 5);
    f.act(|p, _, cx| p.toggle(20, cx));
    assert_eq!(f.kill_word(), Some(("kill".into(), false)));
    assert!(f.cx.debug_bounds("proc-details-20").is_some(), "the lit row opens onto its fields");
    f.act(|p, _, cx| p.press_kill(cx));
    assert_eq!(f.kill_word(), Some(("confirm kill".into(), true)), "the first press arms");
    f.advance(ARM);
    assert_eq!(f.kill_word(), Some(("kill".into(), false)), "an arm left two seconds lapses");
    f.answer(json!({ "daemon.send:proc.kill": { "reply": { "reply": { "ok": true, "signalled": [20], "forced": [], "left": [] } } } }));
    f.act(|p, _, cx| p.press_kill(cx));
    f.act(|p, _, cx| p.press_kill(cx));
    f.said("a tree kill the daemon carried out", "ended");
    assert_eq!(f.kill_word(), None, "nothing more to press once it was sent");
    let sent = f.last_kill();
    assert_eq!((sent["pid"].clone(), sent["signal"].clone(), sent["mode"].clone()), (json!(20), json!("TERM"), json!("tree")));
    assert_eq!(sent["startedAt"], json!(1_700_000_000_000i64), "checked against the row's start");

    f.answer(json!({ "daemon.send:proc.kill": { "reply": { "reply": { "ok": true, "signalled": [11], "forced": [11], "left": [] } } } }));
    f.act(|p, _, cx| p.toggle(11, cx));
    f.act(|p, _, cx| p.press_kill(cx));
    f.act(|p, _, cx| p.press_kill(cx));
    f.said("a tree TERM did not end", "ended by KILL after TERM's 2 s");

    f.answer(json!({ "daemon.send:proc.kill": { "reply": { "reply": { "ok": true, "signalled": [12], "forced": [12], "left": [12] } } } }));
    f.act(|p, _, cx| p.toggle(12, cx));
    f.act(|p, _, cx| p.press_kill(cx));
    f.act(|p, _, cx| p.press_kill(cx));
    f.said("a process KILL did not end", "still running after a forced end");
}

#[gpui_kit::test]
fn an_old_daemons_term_says_so_and_offers_kill_nine_after_five_seconds(cx: &mut TestAppContext) {
    let mut f = open(cx, json!({}), Opened::default(), vec![]);
    f.framed(tree(), 5);
    f.answer(json!({ "daemon.send:proc.kill": { "reply": { "reply": { "ok": true } } } }));
    f.act(|p, _, cx| p.toggle(20, cx));
    f.act(|p, _, cx| p.press_kill(cx));
    f.act(|p, _, cx| p.press_kill(cx));
    f.said(
        "TERM to the pid alone",
        "TERM went to this process alone: mac runs a daemon too old to end what it started; wsp add mac --update updates it",
    );
    assert_eq!(f.kill_word(), None, "nothing offered while TERM has its time");
    f.advance(ESCALATE);
    assert_eq!(f.kill_word(), Some(("kill -9".into(), false)));
    f.act(|p, _, cx| p.press_kill(cx));
    assert_eq!(f.kill_word(), Some(("confirm kill -9".into(), true)));
    f.act(|p, _, cx| p.press_kill(cx));
    f.said("KILL to the pid", "KILL sent");
    let sent = f.last_kill();
    assert_eq!((sent["signal"].clone(), sent.get("mode").cloned()), (json!("KILL"), None), "KILL to the pid alone");
}

/// A turn's run on this computer: its script and the fork of it, the agent, and a tool server under the agent.
fn run() -> Vec<Value> {
    vec![
        row(1, 0, "wsp-daemon", 0.5, 30e6),
        row(30, 1, "bash /tmp/runs/0123456789ab.sh", 0.0, 1e6),
        row(31, 30, "claude -p", 2.0, 300e6),
        row(32, 31, "node mcp.js", 0.1, 40e6),
    ]
}

#[gpui_kit::test]
fn a_kept_agent_ends_through_its_thread_and_a_tool_under_it_is_killed(cx: &mut TestAppContext) {
    let ended = json!({ "activity.end": { "reply": { "pid": 31, "ended": [], "forced": [], "left": [], "freed": "t1" } } });
    let mut f = open(cx, ended, Opened::default(), vec![]);
    f.framed(run(), 4);
    f.act(|p, _, cx| p.toggle(32, cx));
    assert!(matches!(f.lit().act, Act::Kill(_)), "a tool server under the agent is killable");
    f.act(|p, _, cx| p.toggle(31, cx));
    assert_eq!(f.lit().act, Act::End, "no turn runs on it, so End frees it");
    f.act(|p, _, cx| p.toggle(30, cx));
    assert_eq!(f.lit().act, Act::End, "the run script is the agent's too");
    f.act(|p, _, cx| p.end_kept(cx));
    f.said("a kept agent freed", "freed; the thread's next send starts a new one");
    let asked = f.door.asked_of("activity.end");
    assert_eq!((asked[0]["pid"].clone(), asked[0]["startedAt"].clone()), (json!(30), json!(1_700_000_000_000i64)));
}

#[gpui_kit::test]
fn a_running_turns_agent_offers_its_threads_stop_and_says_how_it_went(cx: &mut TestAppContext) {
    let listed = json!({
        "workspaces.list": { "reply": { "workspaces": [workspace("w1", "local", None)] } },
        "sessions.list": { "reply": { "sessions": [turn("s1", "t1", "w1", Some(30), true, None)] } },
        "sessions.interrupt": { "reply": { "outcome": "accepted" } },
    });
    let changes = vec![Change::Workspaces(vec![workspace("w1", "local", None)])];
    let mut f = open(cx, listed, Opened::default(), changes);
    f.framed(run(), 4);
    f.act(|p, _, cx| p.toggle(31, cx));
    f.until("the thread named", |p, cx| p.table(cx).and_then(|t| t.lit).is_some_and(|l| matches!(l.act, Act::Stop(_))));
    let Act::Stop(agent) = f.lit().act else { unreachable!() };
    assert_eq!((agent.title.as_str(), agent.session.as_deref()), ("Thread t1.", Some("s1")));
    f.act(|p, window, cx| p.ask_stop(agent.clone(), window, cx));
    assert!(f.cx.update(|_, cx| screens::dialogs::layer::is_open("processes-stop", cx)), "Stop asks first");
    f.act(|p, _, cx| p.stop_turn("s1".into(), cx));
    f.said("an accepted stop", "stopped");
    assert_eq!(f.door.asked_of("sessions.interrupt")[0]["sessionId"], "s1");
    f.answer(json!({ "sessions.interrupt": { "reply": { "outcome": "not-found" } } }));
    f.act(|p, _, cx| p.stop_turn("s1".into(), cx));
    f.said("a stop with no turn", "no turn is running");
    f.answer(json!({ "sessions.interrupt": { "reply": { "outcome": "refused", "error": "the turn is past stopping" } } }));
    f.act(|p, _, cx| p.stop_turn("s1".into(), cx));
    f.said("the host's own words", "the turn is past stopping");
}

#[gpui_kit::test]
fn an_agent_on_a_cloud_with_no_thread_named_says_to_stop_its_thread(cx: &mut TestAppContext) {
    let changes = vec![Change::Workspaces(vec![workspace("wc", "cloud", None)])];
    let opened = Opened { workspace_id: Some("wc".into()), thread_id: Some("tc".into()), ..Opened::default() };
    let mut f = open(cx, json!({}), opened, changes);
    f.framed(run(), 4);
    f.act(|p, _, cx| p.toggle(31, cx));
    assert_eq!(f.lit().act, Act::Unnamed, "a cloud's agent cannot be freed from here");
    assert_eq!(f.door.asked_of("daemon.open")[0]["workspaceId"], "wc", "the cloud's own daemon");
}

#[gpui_kit::test]
fn a_threads_pane_draws_that_thread_and_its_children_and_never_a_sibling(cx: &mut TestAppContext) {
    let sessions = json!([
        turn("s1", "t1", "w1", Some(100), false, None),
        turn("s2", "t2", "w1", Some(200), false, Some("t1")),
        turn("s3", "t3", "w1", Some(300), false, None),
        turn("s4", "t4", "w1", Some(400), false, Some("t2")),
    ]);
    let listed = json!({
        "workspaces.list": { "reply": { "workspaces": [workspace("w1", "local", None)] } },
        "sessions.list": { "reply": { "sessions": sessions } },
    });
    let changes = vec![Change::Workspaces(vec![workspace("w1", "local", None)])];
    let opened = Opened { workspace_id: Some("w1".into()), thread_id: Some("t1".into()), ..Opened::default() };
    let mut f = open(cx, listed, opened, changes);
    f.until("the thread form", |p, _| p.form().is_some());
    assert_eq!(f.table(), None, "a thread here draws Activity's table, not the computer's");
    assert_eq!(f.thread_state(), Some(ThreadState::Pending));
    let procs = vec![
        row(1, 0, "wsp-daemon", 0.5, 30e6),
        row(100, 1, "claude", 1.0, 300e6),
        row(101, 100, "python3 -m http.server 8000", 1.0, 80e6),
        row(200, 1, "claude", 1.0, 200e6),
        row(300, 1, "claude", 1.0, 100e6),
        row(400, 1, "claude", 1.0, 50e6),
    ];
    f.framed(procs, 6);
    f.until("the groups", |p, cx| {
        matches!(p.thread_state(cx), Some(ThreadState::Groups(g)) if g.iter().map(|g| g.id.as_str()).collect::<Vec<_>>() == ["t1", "t2", "t4"])
    });
    assert!(f.cx.debug_bounds("procs-open-activity").is_some(), "Open in Activity at the foot");
    assert!(f.cx.debug_bounds("activity-sort-CPU").is_none(), "the pane's own width drops CPU, as its container query does");
    assert!(f.cx.debug_bounds("activity-head-menu").is_some(), "and a thread head carries its menu");
    assert!(f.cx.debug_bounds("activity-tag").is_some(), "a kind tag stands while the window is 640 px or more, as `max-sm` reads it");
    let sub = f.cx.debug_bounds("activity-sub-row").expect("a process row under the thread");
    assert_eq!(sub.size.height, px(40.), "`py-2.5` round a 20 px line, with no rule inside the thread's block");
}

#[gpui_kit::test]
fn a_thread_whose_daemon_stopped_says_so_and_offers_its_start(cx: &mut TestAppContext) {
    let changes = vec![
        Change::Workspaces(vec![workspace("w1", "local", None)]),
        Change::Status(json!({ "id": "w1", "phase": "running", "reach": { "state": "unreachable" } })),
    ];
    let opened = Opened { workspace_id: Some("w1".into()), thread_id: Some("t1".into()), ..Opened::default() };
    let mut f = open(cx, json!({}), opened, changes);
    f.until("the thread form", |p, _| p.form().is_some());
    let Some(ThreadState::Absent(absent)) = f.thread_state() else { panic!("{:?}", f.thread_state()) };
    assert_eq!(absent.said, "mac's terminals and files stopped");
    assert_eq!(absent.start.as_deref(), Some("Start again"));
    assert!(f.cx.debug_bounds("daemon-down").is_some(), "the start button stands under the line");
}

#[gpui_kit::test]
fn a_computer_not_answering_says_its_said_and_will_lines(cx: &mut TestAppContext) {
    let changes = vec![
        Change::Places(vec![place("here", HERE_NAME, "computer", true), place("cloudco", "Cloudco", "provider", false)]),
        Change::Workspaces(vec![workspace("wc", "cloud", None)]),
    ];
    let opened = Opened { workspace_id: Some("wc".into()), thread_id: Some("tc".into()), ..Opened::default() };
    let mut f = open(cx, json!({}), opened, changes);
    f.until("the computer table", |p, cx| p.table(cx).is_some());
    let shown = f.shown();
    let absent = shown.absent.expect("its computer is away");
    assert_eq!((absent.said.as_str(), absent.will.as_deref()), ("Cloudco is not answering", Some("it connects on its own when it is on")));
    assert_eq!(shown.count, "no answer", "the count says the away word");
}
