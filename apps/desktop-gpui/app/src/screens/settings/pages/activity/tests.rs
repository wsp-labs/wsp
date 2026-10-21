// SPDX-License-Identifier: AGPL-3.0-only

use super::wire::Kind;
use super::*;
use crate::screens::settings::SettingsScreen;
use gpui_kit::{size, Bounds, Pixels, TestAppContext, VisualTestContext, WeakEntity};

const WIDTHS: [f32; 3] = [600., 760., 1280.];

fn proc(pid: Pid, kind: Kind) -> Proc {
    Proc {
        pid,
        name: format!("p{pid}"),
        kind,
        mem: 2e8,
        cpu: 3.,
        energy: None,
        ports: vec![],
        command: "node dev".into(),
        started_at: None,
        id: None,
        harness: None,
        needed: false,
        endable: true,
        heading: false,
        note: None,
        children: vec![],
    }
}

fn thread_group() -> Group {
    let thread = Thread {
        id: "t1".into(),
        title: "Fix the login".into(),
        harness: "claude".into(),
        project: "bench".into(),
        workspace_id: "w1".into(),
        session_id: "s1".into(),
        working: false,
        pid: None,
        agent_pid: None,
        cgroup: None,
        parent: None,
        slate: None,
        subagents: vec![],
    };
    Group {
        id: "t1".into(),
        kind: model::GroupKind::Thread,
        thread: Some(thread),
        procs: vec![proc(11, Kind::Dev)],
        subagents: vec![],
        containers: vec![],
        memory: None,
    }
}

/// The page's tiles, bar and Threads list as its body lays them, at the window's width, off one thread with no host.
struct Drawn {
    page: Page,
    computers: Vec<Computer>,
    scoped: Entity<Scoped>,
}

impl Render for Drawn {
    fn render(&mut self, window: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        let width = narrow::width(window);
        let computer = self.computers[0].clone();
        let selects = selects(&self.computers, &computer, &self.scoped, window, cx);
        let s = self.scoped.read(cx);
        let scope = Scope { groups: vec![thread_group()], rest: vec![], energy: false };
        let hand = Hand {
            scoped: self.scoped.downgrade(),
            ending: Rc::default(),
            stopping: Rc::default(),
            open: Rc::new(["t1".to_owned()].into_iter().collect()),
            sort: Sort::Mem,
            energy: false,
            filtering: false,
            width,
            workspace: None,
            pieces: Rc::default(),
        };
        let looks = ThreadLooks { status: Rc::new(|_| None), model: Rc::new(|_, _| None) };
        v_flex()
            .size_full()
            .p_4()
            .gap(px(40.))
            .child(tiles(&scope, &[], None, width.sm, &hand, cx))
            .child(controls(&self.page, selects, View::Threads, &self.scoped, s, width.sm, cx))
            .child(tables::threads_table("This computer".into(), Rc::new(scope.groups.clone()), looks, false, None, &hand).whole(cx))
    }
}

fn draw(cx: &mut TestAppContext) -> &mut VisualTestContext {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
    let place: PlaceView = serde_json::from_value(json!({ "id": HERE, "kind": "computer", "name": "here", "default": true })).unwrap();
    let computer = Computer { glyph: ComputerIcon::of(&place, None), place: place.clone(), key: HERE.into(), workspace_id: None };
    let acts = Acts::new(WeakEntity::<SettingsScreen>::new_invalid(), None, runtime);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let pick = cx.new(|_| Pick::default());
        let page = Page { pick, places: vec![(place, computer.glyph)], models: vec![], acts: acts.clone() };
        let scoped = cx.new(|cx| Scoped::new(computer.clone(), acts, View::Threads, window, cx));
        let drawn = cx.new(|_| Drawn { page, computers: vec![computer], scoped });
        gpui_kit::component::Root::new(drawn, window, cx)
    });
    cx
}

fn at(cx: &mut VisualTestContext, width: f32) {
    cx.simulate_resize(size(px(width), px(1200.)));
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str, width: f32) -> Bounds<Pixels> {
    cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} was not drawn at {width} px"))
}

#[track_caller]
fn near(what: &str, got: Pixels, want: f32) {
    let got = f32::from(got);
    assert!((got - want).abs() < 0.5, "{what}: {got} px, the web writes {want} px");
}

#[gpui_kit::test]
fn the_tiles_stand_two_by_two_under_640(cx: &mut TestAppContext) {
    let cx = draw(cx);
    for width in WIDTHS {
        at(cx, width);
        let (mem, cpu, procs) = (bounds(cx, "activity-mem", width), bounds(cx, "activity-cpu", width), bounds(cx, "activity-procs", width));
        near("the second tile beside the first", cpu.top(), f32::from(mem.top()));
        if width < 640. {
            let grid = bounds(cx, "activity-totals", width);
            near("two columns, each half the strip", mem.size.width, f32::from(grid.size.width) / 2.);
            near("two columns, the second at the first's edge", cpu.left(), f32::from(mem.right()));
            assert!(procs.top() >= mem.bottom(), "the third tile starts the second line at {width} px: {procs:?}");
            near("the second line's first column under the first", procs.left(), f32::from(mem.left()));
        } else {
            near("one line of three", procs.top(), f32::from(mem.top()));
        }
    }
}

#[gpui_kit::test]
fn the_bar_becomes_a_grid_of_two_under_640(cx: &mut TestAppContext) {
    let cx = draw(cx);
    for width in WIDTHS {
        at(cx, width);
        let (computer, kind, thread, filter) = (
            bounds(cx, "activity-computer", width),
            bounds(cx, "activity-kind", width),
            bounds(cx, "activity-thread", width),
            bounds(cx, "activity-filter", width),
        );
        near(&format!("BAR_SELECT's h-8 at {width} px"), computer.size.height, 32.);
        if width < 640. {
            near("the computer across both columns", computer.size.width, f32::from(kind.size.width + thread.size.width) + 8.);
            near("kind and thread side by side", kind.top(), f32::from(thread.top()));
            near("gap-2 down the grid", kind.top() - computer.bottom(), 8.);
            near("the filter on the line under, gap-y-2", filter.top() - kind.bottom(), 8.);
            near("the filter across the bar", filter.size.width, f32::from(computer.size.width));
        } else {
            near("the computer's w-56", computer.size.width, 224.);
            near("the kind's w-40", kind.size.width, 160.);
            near("the thread's w-48", thread.size.width, 192.);
            near("the filter's w-64", filter.size.width, 256.);
        }
    }
}

#[gpui_kit::test]
fn the_lists_drop_cpu_and_energy_and_a_thread_head_carries_its_menu_under_768(cx: &mut TestAppContext) {
    let cx = draw(cx);
    for width in WIDTHS {
        at(cx, width);
        let narrow = width < 768.;
        assert_eq!(cx.debug_bounds("activity-sort-CPU").is_none(), narrow, "CPU's head at {width} px");
        assert_eq!(cx.debug_bounds("activity-head-menu").is_some(), narrow, "the thread head's menu button at {width} px");
        let mem = bounds(cx, "activity-sort-Memory", width);
        let row = bounds(cx, "activity-sub-row", width);
        // `narrow:grid-cols-[minmax(0,1fr)_72px_20px]` against the full `76px 60px 64px 20px`.
        let past = if narrow { 16. + 20. + 20. } else { 16. + 60. + 16. + 64. + 16. + 20. + 20. };
        near(&format!("memory's column against the row's end at {width} px"), row.right() - mem.right(), past);
        near(&format!("memory's column at {width} px"), mem.size.width, if narrow { 72. } else { 76. });
    }
}

#[test]
fn activity_reads_this_computer_and_each_box_a_folder_stands_on() {
    let row = |v: Value| -> (PlaceView, ComputerIcon) {
        let place: PlaceView = serde_json::from_value(v).unwrap();
        let icon = ComputerIcon::of(&place, None);
        (place, icon)
    };
    let places = vec![
        row(json!({ "id": "here", "kind": "computer", "name": "mac", "default": true })),
        row(json!({ "id": "pi", "kind": "computer", "name": "pi", "default": false, "present": false })),
        row(json!({ "id": "nas", "kind": "computer", "name": "nas", "default": false })),
        row(json!({ "id": "box", "kind": "provider", "name": "box", "default": false })),
    ];
    let workspaces =
        [json!({ "id": "wpi", "kind": "place", "place": "pi" }), json!({ "id": "wcloud", "kind": "cloud", "provider": "box" })];
    let read = computers(&places, &workspaces);
    let keys: Vec<(&str, &str)> = read.iter().map(|c| (c.place.id.as_str(), c.key.as_str())).collect();
    assert_eq!(keys, [("here", HERE), ("pi", "wpi")], "no box without a folder there, and no cloud");
    assert_eq!(absent(&read[0]), None, "this computer is never absent here");
    assert_eq!(absent(&read[1]), Some(("pi is not answering".to_owned(), "it connects on its own when it is on".to_owned())));
}

#[test]
fn a_thread_titles_as_the_sidebar_does() {
    let row = |key: &str, text: &str| Row::of(&serde_json::json!({ "id": "s1", "workspaceId": "w1", key: text })).unwrap();
    let turn = |prompt: &str| row("prompt", prompt);
    assert_eq!(title_of(&turn("Fix the login bug. Then add tests."), &turn("")), "Fix the login bug.");
    // 30 faces are 60 UTF-16 units, which JavaScript's length counts: past 48, so cut.
    let faces = "\u{1f600}".repeat(30);
    assert_eq!(title_of(&turn(&faces), &turn("")), format!("{}\u{2026}", "\u{1f600}".repeat(23)));
    assert_eq!(title_of(&row("cwd", "/w"), &row("harnessTitle", "Named\nmore")), "Named");
}

#[test]
fn a_refused_watch_holds_the_skeletons_still() {
    assert!(skeletons_move(false, false));
    assert!(!skeletons_move(true, false));
    assert!(!skeletons_move(false, true));
}

fn ticked(pid: Pid, command: &str, id: Option<&str>) -> Proc {
    Proc {
        pid,
        name: "node".into(),
        kind: Kind::Dev,
        mem: 0.,
        cpu: 0.,
        energy: None,
        ports: vec![],
        command: command.into(),
        started_at: Some(1_700_000_000_000.4),
        id: id.map(str::to_owned),
        harness: None,
        needed: false,
        endable: true,
        heading: false,
        note: None,
        children: vec![],
    }
}

fn entry(pid: Pid, id: Option<&str>) -> wire::ProcEntry {
    wire::ProcEntry {
        pid,
        ppid: 1,
        user: String::new(),
        state: "S".into(),
        comm: "node".into(),
        cmdline: String::new(),
        cpu: 0.,
        rss: 0.,
        started_at: 0.,
        pty: None,
        cgroup: None,
        energy_mw: None,
        pgid: None,
        owner: None,
        kind: None,
        id: id.map(str::to_owned),
    }
}

fn snapshot(procs: Vec<wire::ProcEntry>) -> Snapshot {
    Snapshot { at: 0., daemon: 1, procs, total: None, seq: None, cost: None, containers: None, cgroups: None }
}

#[test]
fn end_is_a_tree_term_and_force_end_a_lone_kill_both_checked_against_the_row() {
    let p = ticked(41, "npm run dev", Some("tick-1"));
    assert_eq!(
        kill_frame(&p, "TERM", true),
        json!({ "op": "proc.kill", "pid": 41, "signal": "TERM", "mode": "tree", "startedAt": 1_700_000_000_000i64, "identity": "tick-1" })
    );
    assert_eq!(
        kill_frame(&p, "KILL", false),
        json!({ "op": "proc.kill", "pid": 41, "signal": "KILL", "startedAt": 1_700_000_000_000i64, "identity": "tick-1" })
    );
}

#[test]
fn only_a_reply_with_no_signalled_list_is_a_too_old_daemons() {
    assert!(!reached_alone(&json!({ "id": 3, "ok": true, "signalled": [41, 42], "forced": [], "left": [] })));
    assert!(reached_alone(&json!({ "id": 3, "ok": true })));
}

#[test]
fn force_end_offers_kill_only_to_what_the_frame_still_lists_as_itself() {
    let said = [ticked(41, "a", Some("tick-1")), ticked(42, "b", Some("tick-2")), ticked(43, "c", None)];
    let frame = snapshot(vec![entry(41, Some("tick-1")), entry(42, Some("tick-9")), entry(43, Some("tick-3"))]);
    assert_eq!(still_listed(&said, Some(&frame)).iter().map(|p| p.pid).collect::<Vec<_>>(), [41, 43], "42 is another process now");
    assert!(still_listed(&said, None).is_empty());
}

#[test]
fn the_too_old_notice_says_the_webs_words() {
    assert_eq!(
        w::kill_too_old_of("mac", &[(41, "npm run dev")]),
        "TERM went to 41 (npm run dev) alone: mac runs a daemon too old to end what it started; wsp add mac --update updates it"
    );
    assert_eq!(
        w::kill_too_old_of("box", &[(41, "a"), (42, "b")]),
        "TERM went to 41 (a) and 42 (b) alone: box runs a daemon too old to end what they started; wsp add box --update updates it"
    );
}

#[test]
fn a_slates_heading_takes_the_title_slates_get_answers() {
    assert_eq!(
        slate_title(&json!({ "slate": { "threadId": "t", "document": { "title": "Deploy board" } } })).as_deref(),
        Some("Deploy board")
    );
    assert_eq!(slate_title(&json!({ "slate": { "threadId": "t", "document": null } })), None);
    assert_eq!(slate_title(&json!({ "slate": null })), None);
}

struct Blank;

impl Render for Blank {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[gpui_kit::test]
fn what_a_too_old_daemon_signalled_alone_is_said_once_after_the_wait(cx: &mut gpui_kit::TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let (_, cx) = cx.add_window_view(|window, cx| gpui_kit::component::Root::new(cx.new(|_| Blank), window, cx));
    let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap());
    let place: PlaceView = serde_json::from_value(json!({ "id": "here", "kind": "computer", "name": "mac", "default": true })).unwrap();
    let computer = Computer { glyph: ComputerIcon::of(&place, None), place, key: HERE.into(), workspace_id: None };
    let acts = Acts::new(gpui_kit::WeakEntity::new_invalid(), None, runtime);
    let scoped = cx.update(|window, cx| cx.new(|cx| Scoped::new(computer, acts, View::Threads, window, cx)));
    scoped.update(cx, |s, cx| {
        s.snapshot = Some(snapshot(vec![entry(41, Some("tick-1"))]));
        for p in [ticked(41, "npm run dev", Some("tick-1")), ticked(42, "vite", None)] {
            s.ending.insert(p.pid);
            s.wait_too_old(p, cx);
        }
    });
    let said = |cx: &mut gpui_kit::VisualTestContext| {
        let toasts = cx.update(|window, cx| crate::ui::toast::of(window, cx).expect("ui::init registers the stack"));
        toasts.read_with(cx, |t, _| t.standing())
    };
    cx.executor().advance_clock(ESCALATE / 2);
    cx.run_until_parked();
    assert_eq!(said(cx).len(), 0, "nothing is said before TERM has had its wait");
    cx.executor().advance_clock(ESCALATE);
    cx.run_until_parked();
    let notices = said(cx);
    assert_eq!(notices.len(), 1, "one notice names both");
    let n = &notices[0];
    assert_eq!(n.text.as_ref(), w::kill_too_old_of("mac", &[(41, "npm run dev"), (42, "vite")]));
    assert_eq!(n.place.as_deref(), Some("mac"), "where it happened, as the web's notice says it");
    assert_eq!(n.action.as_ref().map(|a| a.word.as_ref()), Some(w::FORCE_END), "41 is still listed as itself");
    assert!(scoped.read_with(cx, |s, _| s.ending.is_empty() && s.too_old.is_empty() && s.too_old_wait.is_none()));
}

/// A reading moves the memory tile's piece only where its words or its tint move, and the page draws again for none.
#[gpui_kit::test]
fn a_sample_that_moves_no_word_draws_nothing(cx: &mut gpui_kit::TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let (_, cx) = cx.add_window_view(|window, cx| gpui_kit::component::Root::new(cx.new(|_| Blank), window, cx));
    let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap());
    let place: PlaceView = serde_json::from_value(json!({ "id": "here", "kind": "computer", "name": "mac", "default": true })).unwrap();
    let computer = Computer { glyph: ComputerIcon::of(&place, None), place, key: HERE.into(), workspace_id: None };
    let acts = Acts::new(gpui_kit::WeakEntity::new_invalid(), None, runtime);
    let scoped = cx.update(|window, cx| cx.new(|cx| Scoped::new(computer, acts, View::Threads, window, cx)));
    // The daemon at 6 GiB, its frame drawn as the page would draw it.
    let row = json!({ "pid": 1, "ppid": 0, "user": "u", "state": "S", "comm": "wsp-daemon", "cmdline": "wsp-daemon", "cpu": 0.,
        "rss": 6. * 1024. * 1024. * 1024., "startedAt": 1 });
    let frame = json!({ "type": "proc.snapshot", "at": 1000, "daemon": 1, "total": 1, "procs": [row], "seq": 1 });
    scoped.update(cx, |s, cx| {
        s.hear(frame, cx);
        s.drawing(View::Threads, Width::default(), cx);
    });
    let drawn = Rc::new(std::cell::Cell::new(0));
    let counted = drawn.clone();
    cx.update(|_, cx| cx.observe(&scoped, move |_, _| counted.set(counted.get() + 1)).detach());
    let sample = |used: f64, total: f64| json!({ "type": "sys.sample", "mem": { "used": used, "total": total } });
    let mut was = None;
    let mut take = |event: Value, cx: &mut gpui_kit::VisualTestContext| {
        scoped.update(cx, |s, cx| s.hear(event, cx));
        cx.run_until_parked();
        let look = scoped.read_with(cx, |s, cx| s.pieces().get("tile:mem").map(|piece| piece.read(cx).look().clone()));
        let moved = look != was;
        was = look;
        moved
    };
    assert!(take(sample(4.0e9, 16.0e9), cx), "the first sample names the memory in use");
    assert!(!take(sample(4.0e9 + 1024., 16.0e9), cx), "a few bytes more read the same words and draw nothing");
    assert!(take(sample(6.0e9, 16.0e9), cx), "new words draw");
    assert!(take(sample(6.0e9, 10.0e9), cx), "a total the daemon's 6 GiB is past half of moves the tint and draws");
    assert!(!take(sample(6.0e9, 11.0e9), cx), "a total that leaves the words and the tint draws nothing");
    assert_eq!(scoped.read_with(cx, |s, _| s.mem), Some((6.0e9, 11.0e9)), "the newest sample is held either way");
    assert_eq!(drawn.get(), 0, "the page draws again for none of them");
}

#[gpui_kit::test]
fn a_failed_resync_leaves_the_next_lost_frame_free_to_ask_again(cx: &mut gpui_kit::TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let (_, cx) = cx.add_window_view(|window, cx| gpui_kit::component::Root::new(cx.new(|_| Blank), window, cx));
    let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
    let place: PlaceView = serde_json::from_value(json!({ "id": "here", "kind": "computer", "name": "mac", "default": true })).unwrap();
    let computer = Computer { glyph: ComputerIcon::of(&place, None), place, key: HERE.into(), workspace_id: None };
    let acts = Acts::new(gpui_kit::WeakEntity::new_invalid(), None, runtime);
    let scoped = cx.update(|window, cx| cx.new(|cx| Scoped::new(computer, acts, View::Threads, window, cx)));
    // Changes on a frame the page never held: the daemon is asked for a whole list, and with no link it refuses.
    let lost = json!({ "type": "proc.changes", "at": 1.0, "daemon": 1, "seq": 9, "base": 7 });
    scoped.update(cx, |s, cx| {
        s.snapshot = Some(Snapshot { seq: Some(5), ..snapshot(vec![]) });
        s.take_frame(&lost, cx);
        assert!(s.unavailable.is_some(), "the resync was asked and refused");
        s.unavailable = None;
        s.take_frame(&lost, cx);
        assert!(s.unavailable.is_some(), "the next lost frame asks again, the refused resync no longer standing in its way");
    });
}

#[test]
fn a_box_reads_its_threads_cgroups_until_every_process_is_asked() {
    assert_eq!(reads(true, View::Threads, false), None);
    assert_eq!(reads(false, View::Threads, true), Some(vec![THREAD_CGROUPS.to_owned()]));
    assert_eq!(reads(false, View::Processes, false), Some(vec![THREAD_CGROUPS.to_owned()]));
    assert_eq!(reads(false, View::Processes, true), None);
}
