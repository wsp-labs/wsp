// SPDX-License-Identifier: AGPL-3.0-only
//! Add a computer: what it reads and writes against a host started for the test alone (host-client's tests/common),
//! the refusal an address nobody can reach gets, a place frame the host pushes, the picks kept 400 ms after they move,
//! the pick list's ranges and filter, and how the dialog leaves through the layer as the shell draws it. Nothing past
//! the checks reaches a real computer: those steps are staged from the fixtures `tests/dialogs/computer.rs` holds.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "dialogs/computer.rs"]
mod computer;
mod dialogs {
    #[allow(unused_imports)]
    pub use super::{page, State};
}

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::dialogs::add_computer::model::{self, AddStep};
use desktop_gpui::screens::dialogs::add_computer::{self, AddComputer, KEEP_AFTER, OUTPUT_EVERY};
use desktop_gpui::screens::dialogs::picks::{PickItem, PickListState};
use desktop_gpui::screens::dialogs::sign_in;
use desktop_gpui::screens::dialogs::{layer, ADD_COMPUTER};
use desktop_gpui::screens::Host;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::ui::{self, ActivePalette, Side};
use gpui_kit::component::Root;
use gpui_kit::{
    div, point, prelude::*, px, size, AnyView, App, AppContext, Context, Entity, Modifiers, TestAppContext, VisualTestContext, Window,
};
use serde_json::json;
use wsp_host_client::protocol::PlacesSetRequest;
use wsp_host_client::{Env, HostClient};

/// What `computer.rs` stages states with.
pub struct State {
    pub id: &'static str,
    pub width: f32,
    pub height: f32,
    pub stage: fn(&mut Window, &mut App) -> AnyView,
}

impl State {
    pub fn new(id: &'static str, stage: fn(&mut Window, &mut App) -> AnyView) -> Self {
        State { id, width: 1280., height: 800., stage }
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }
}

pub fn page(_: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_| Page).into()
}

struct Page;

impl Render for Page {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(cx.palette().background)
    }
}

/// The page with the dialogs' layer drawn last over it, as the shell draws it.
struct Stage;

impl Render for Stage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(page(window, cx)).children(layer::draw(window, cx))
    }
}

fn window(cx: &mut TestAppContext) -> &mut VisualTestContext {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let stage = cx.new(|_| Stage);
        Root::new(stage, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    cx
}

fn host_of(host: &common::Throwaway) -> Host {
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    Host::new(client, runtime)
}

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn opened(cx: &mut VisualTestContext) -> Entity<AddComputer> {
    let mut view = None;
    cx.update(|window, cx| view = Some(add_computer::staged(window, cx, |_, _, _| {})));
    cx.run_until_parked();
    view.unwrap()
}

fn read<R>(view: &Entity<AddComputer>, cx: &mut VisualTestContext, f: impl FnOnce(&AddComputer, &App) -> R) -> R {
    view.read_with(cx, |v, cx| f(v, cx))
}

#[gpui_kit::test]
fn it_reads_the_host_and_an_address_nobody_can_reach_is_refused_on_its_checks(cx: &mut TestAppContext) {
    let throwaway = common::Throwaway::start();
    let host = host_of(&throwaway);
    let cx = window(cx);
    let mut dialog = None;
    cx.update(|window, cx| dialog = Some(add_computer::open(&host, None, window, cx)));
    let dialog = dialog.unwrap();
    cx.update(|_, cx| assert_eq!(layer::top(cx), Some(ADD_COMPUTER)));
    until(cx, "places.list, recipes.options and places.sshHosts", |cx| {
        read(&dialog, cx, |d, _| {
            !d.here().is_empty() && d.options().is_some() && (!d.hosts().0.is_empty() || d.hosts().1.is_some() || d.hosts_read())
        })
    });
    let here = read(&dialog, cx, |d, _| d.here());
    println!("places.list named this computer {here:?}; recipes.options answered");

    dialog.update_in(cx, |d, window, cx| d.type_address("nobody@wsp-parity.invalid", window, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    until(cx, "the add's refusal", |cx| read(&dialog, cx, |d, _| d.job().and_then(|j| j["state"].as_str()) == Some("failed")));
    let (step, said) = read(&dialog, cx, |d, _| (d.step(), d.job().unwrap()["said"].as_str().unwrap_or_default().to_owned()));
    assert_eq!(step, AddStep::Checks, "a refused add stands on its checks");
    assert!(!said.is_empty(), "the refusal carries the host's sentence");
    assert!(cx.debug_bounds("step-refusal").is_some(), "the failed row opens on what happened");
    println!("places.add refused: {said}");
}

/// A window whose layers hold the host and a sidebar following it, as the shell leaves them once connected.
fn with_sidebar<'a>(cx: &'a mut TestAppContext, host: &Host, throwaway: &common::Throwaway) -> &'a mut VisualTestContext {
    cx.update(desktop_gpui::screens::palette::overlays::init);
    let cx = window(cx);
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap());
    let client = runtime.block_on(HostClient::connect(&throwaway.state(), &throwaway.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let link = desktop_gpui::sidebar::Link { host: Arc::new(client), runtime, page: None };
    cx.update(|window, cx| {
        let sidebar = cx.new(desktop_gpui::sidebar::Sidebar::new);
        sidebar.update(cx, |s, cx| s.attach(link, cx));
        let overlays = desktop_gpui::screens::palette::overlays::of(window, cx).expect("the layers are registered");
        overlays.update(cx, |o, _| {
            o.host = Some(host.clone());
            o.sidebar = Some(sidebar.downgrade());
        });
        // The sidebar lives as long as the window's layers point at it.
        std::mem::forget(sidebar);
    });
    cx
}

#[gpui_kit::test]
fn the_host_pushes_reach_the_dialog(cx: &mut TestAppContext) {
    let throwaway = common::Throwaway::start();
    let host = host_of(&throwaway);
    let cx = with_sidebar(cx, &host, &throwaway);
    let mut dialog = None;
    cx.update(|window, cx| dialog = Some(add_computer::open(&host, None, window, cx)));
    let dialog = dialog.unwrap();
    until(cx, "this computer listed", |cx| read(&dialog, cx, |d, _| !d.here().is_empty()));

    // place.changed: a setting of this computer moved elsewhere lands on the sidebar's record of it, which the dialog reads.
    let set: PlacesSetRequest = serde_json::from_value(json!({ "placeId": "here", "turnLimitMs": 7_200_000 })).unwrap();
    let answer = futures::executor::block_on(host.run(move |c| async move { c.call(set).await }));
    answer.unwrap_or_else(|f| panic!("places.set: {}", f.message));
    let here = |d: &AddComputer| d.places().iter().find(|p| p["id"] == "here").map(|p| p["turnLimitMs"].clone());
    until(cx, "place.changed", |cx| read(&dialog, cx, |d, _| here(d) == Some(json!(7_200_000))));

    // place.stage: the add's own steps arrive only as pushes; its answer carries none.
    dialog.update_in(cx, |d, window, cx| {
        d.type_address("nobody@wsp-parity.invalid", window, cx);
        d.connect(None, None, window, cx);
    });
    until(cx, "a place.stage frame", |cx| {
        read(&dialog, cx, |d, _| d.job().is_some_and(|j| j["steps"].as_array().is_some_and(|s| !s.is_empty())))
    });
    let steps = read(&dialog, cx, |d, _| d.job().unwrap()["steps"].clone());
    assert_eq!(steps[0]["step"], "connect", "the first stage the host pushed is the connection: {steps}");
}

#[gpui_kit::test]
fn picks_are_kept_400_ms_after_the_last_change_and_once_for_a_burst(cx: &mut TestAppContext) {
    let cx = window(cx);
    let dialog = opened(cx);
    dialog.update(cx, |d, cx| {
        d.take_places(vec![computer::here(), computer::joined(None, json!([]))], vec![], vec![], cx);
        d.stage(AddStep::Clis, computer::BOX, Some(computer::job("done", json!([]))), Some("p_box"), Some(computer::picks()), cx);
        for cli in ["ffmpeg", "duckdb", "rg"] {
            let mut p = d.picks().unwrap().clone();
            p["clis"][cli] = json!({ "via": "apt" });
            d.set_picks(p, None, cx);
        }
    });
    cx.executor().advance_clock(KEEP_AFTER - Duration::from_millis(50));
    cx.run_until_parked();
    assert_eq!(read(&dialog, cx, |d, _| d.writes().len()), 0, "nothing is written while the picks still move");
    cx.executor().advance_clock(Duration::from_millis(60));
    cx.run_until_parked();
    let writes = read(&dialog, cx, |d, _| d.writes().to_vec());
    assert_eq!(writes.len(), 1, "a burst of ticks is one write");
    assert_eq!(writes[0].0, "p_box");
    assert!(writes[0].1["clis"]["duckdb"].is_object(), "the write holds the picks as they last stood");
}

#[gpui_kit::test]
fn closing_keeps_the_picks_that_wait(cx: &mut TestAppContext) {
    let (_home, cx) = shell(cx);
    let dialog = opened(cx);
    dialog.update(cx, |d, cx| {
        d.stage(AddStep::Clis, computer::BOX, None, Some("p_box"), Some(computer::picks()), cx);
        d.set_picks(computer::picks(), None, cx);
    });
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| layer::top(cx)), None);
    assert_eq!(read(&dialog, cx, |d, _| d.writes().len()), 1, "Escape writes the picks at once");
}

#[gpui_kit::test]
fn the_choose_and_estimate_writes_reach_the_host_as_it_reads_them(cx: &mut TestAppContext) {
    let throwaway = common::Throwaway::start();
    let host = host_of(&throwaway);
    let cx = window(cx);
    let mut dialog = None;
    cx.update(|window, cx| dialog = Some(add_computer::open(&host, None, window, cx)));
    let dialog = dialog.unwrap();
    until(cx, "options", |cx| read(&dialog, cx, |d, _| d.options().is_some()));
    dialog.update(cx, |d, cx| {
        d.stage(AddStep::Other, "here", None, Some("here"), Some(computer::picks()), cx);
        d.set_picks(computer::picks(), None, cx);
        d.go(AddStep::Summary, cx);
    });
    cx.executor().advance_clock(KEEP_AFTER);
    until(cx, "places.choose answered", |cx| read(&dialog, cx, |d, _| d.chose().is_some()));
    let chose = read(&dialog, cx, |d, _| d.chose().cloned().unwrap());
    // This computer has no pending add, so the host reads the picks and refuses them for that, not for their shape.
    assert!(matches!(&chose, Err(said) if said.contains("no add of here is pending")), "{chose:?}");
    println!("places.choose: {chose:?}");
    let deadline = Instant::now() + Duration::from_secs(20);
    while read(&dialog, cx, |d, _| d.estimate().is_none()) && Instant::now() < deadline {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    let estimate = read(&dialog, cx, |d, _| d.estimate().cloned()).expect("the summary weighed the picks");
    assert!(estimate["keptBytes"].as_u64().is_some_and(|n| n > 0), "{estimate}");
}

fn shell(cx: &mut TestAppContext) -> (tempfile::TempDir, &mut VisualTestContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
    });
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap());
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().to_owned();
    let env: Env = [("HOME".to_owned(), dir.to_string_lossy().into_owned())].into_iter().collect();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(dir.join("state.json"), env, runtime, Launch::default(), window, cx));
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    (home, cx)
}

fn top(cx: &mut VisualTestContext) -> Option<&'static str> {
    cx.update(|_, cx| layer::top(cx))
}

#[gpui_kit::test]
fn escape_and_the_scrim_close_it_and_a_press_inside_does_not(cx: &mut TestAppContext) {
    let (_home, cx) = shell(cx);
    opened(cx);
    let dialog = cx.debug_bounds("dialog").expect("the dialog stands");
    cx.simulate_click(dialog.center(), Modifiers::none());
    assert_eq!(top(cx), Some(ADD_COMPUTER), "a press inside keeps it");
    cx.simulate_click(point(px(40.), px(400.)), Modifiers::none());
    assert_eq!(top(cx), None, "a press on the scrim closes it");

    opened(cx);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(top(cx), None, "Escape closes it, the address field holding focus");
}

/// Catches: under 640 px, a press on the sheet's own edge reaching the scrim, and a press above it closing two dialogs.
#[gpui_kit::test]
fn the_narrow_sheet_keeps_its_own_presses_and_the_room_above_closes_one(cx: &mut TestAppContext) {
    let cx = window(cx);
    cx.simulate_resize(size(px(600.), px(800.)));
    cx.update(|window, cx| layer::open("under", page(window, cx), layer::Closes::Outside, None, window, cx));
    opened(cx);
    let sheet = cx.debug_bounds("dialog").expect("the sheet stands");
    cx.simulate_click(point(px(8.), sheet.origin.y + px(80.)), Modifiers::none());
    assert_eq!(top(cx), Some(ADD_COMPUTER), "a press on the sheet's own edge is a press inside it");
    cx.simulate_click(point(px(300.), px(8.)), Modifiers::none());
    assert_eq!(top(cx), Some("under"), "a press above the sheet closes it and only it");
}

#[gpui_kit::test]
fn escape_closes_an_open_select_first(cx: &mut TestAppContext) {
    let (_home, cx) = shell(cx);
    let dialog = opened(cx);
    dialog.update(cx, |d, cx| {
        d.take_options(computer::options(), cx);
        d.stage(AddStep::Clis, computer::BOX, None, Some("p_box"), Some(computer::picks()), cx);
    });
    cx.run_until_parked();
    let order = cx.debug_bounds("select").expect("the order select stands in the tools");
    cx.simulate_click(order.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("select-list").is_some(), "the list opened");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(cx.debug_bounds("select-list").is_none(), "Escape closed the list");
    assert_eq!(top(cx), Some(ADD_COMPUTER), "and not the dialog");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(top(cx), None);
}

#[gpui_kit::test]
fn a_shift_press_ranges_over_the_rows_shown_and_a_locked_row_keeps_its_tick(cx: &mut TestAppContext) {
    let cx = window(cx);
    let state = cx.update(|window, cx| cx.new(|cx| PickListState::new(None, window, cx)));
    let items = vec![
        PickItem::new("a", "alpha", false),
        PickItem::new("b", "bravo", false).locked(true),
        PickItem::new("c", "charlie", false),
        PickItem::new("d", "delta", false),
    ];
    let first = state.update(cx, |s, cx| s.press(&"a".into(), true, false, &items, cx));
    assert_eq!(first, vec![("a".into(), true)]);
    let range = state.update(cx, |s, cx| s.press(&"d".into(), true, true, &items, cx));
    assert_eq!(range, vec![("a".into(), true), ("c".into(), true), ("d".into(), true)], "the locked row is never in a range");
    state.update_in(cx, |s, window, cx| s.set_filter("CHAR", window, cx));
    let shown = state.read_with(cx, |s, cx| s.shown(&items, cx).into_iter().map(|i| i.key().to_string()).collect::<Vec<_>>());
    assert_eq!(shown, vec!["c"], "the filter reads names and keys, case aside");
}

#[gpui_kit::test]
fn a_filter_that_matches_nothing_says_so(cx: &mut TestAppContext) {
    let cx = window(cx);
    let dialog = opened(cx);
    dialog.update_in(cx, |d, window, cx| {
        d.take_options(computer::options(), cx);
        d.stage(AddStep::Skills, computer::BOX, None, Some("p_box"), Some(computer::picks()), cx);
        d.filter(add_computer::List::Skills, "nothing-like-it", window, cx);
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("pick-none-match").is_some(), "No row matches nothing-like-it.");
}

#[gpui_kit::test]
fn the_ready_page_stands_once_every_row_is_done(cx: &mut TestAppContext) {
    let cx = window(cx);
    let dialog = opened(cx);
    dialog.update(cx, |d, cx| {
        let place = computer::joined(Some(computer::setup("done", json!([]), json!([]))), json!([]));
        d.take_places(vec![computer::here(), place], vec![], vec![], cx);
        d.stage(AddStep::Running, computer::BOX, None, Some("p_box"), None, cx);
    });
    cx.run_until_parked();
    for act in ["act-Close", "act-Next"] {
        assert!(cx.debug_bounds(act).is_some(), "{act} stands in the foot once every row is done");
    }
    assert!(cx.debug_bounds("act-Run in background").is_none());
    dialog.update(cx, |d, cx| d.go(AddStep::Ready, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("ready-title").is_some(), "lab-box is ready");
}

/// The box's setup ended with rows that did not land: Claude Code's sign-in failed, Codex's was skipped, and, with
/// `folder`, a project did not clone, far down the list.
fn ended_with_misses(cx: &mut VisualTestContext, folder: bool) -> Entity<AddComputer> {
    let dialog = opened(cx);
    dialog.update(cx, |d, cx| {
        let mut rows = computer::landed_rows();
        let list = rows.as_array_mut().unwrap();
        list.retain(|r| r["id"] != "signins/claude" && r["id"] != "folders/wsp");
        list.push(json!({ "id": "signins/claude", "label": "Claude Code", "outcome": "failed", "step": "signins", "note": "the sign-in ran out" }));
        list.push(json!({ "id": "signins/codex", "label": "Codex", "outcome": "skipped", "step": "signins" }));
        if folder {
            list.push(json!({ "id": "folders/wsp", "label": "wsp", "outcome": "failed", "step": "folders", "note": "git clone was refused" }));
        }
        let place = computer::joined(Some(computer::setup("done", computer::done_steps(), json!([]))), rows);
        d.take_places(vec![computer::here(), place], vec![], vec![], cx);
        d.stage(AddStep::Running, computer::BOX, None, Some("p_box"), None, cx);
    });
    settle(cx);
    dialog
}

/// Draws until what the last frame moved stands: the running view scrolls to the row that needs it as it paints.
fn settle(cx: &mut VisualTestContext) {
    for _ in 0..2 {
        cx.run_until_parked();
        cx.update(|window, cx| drop(window.draw(cx)));
    }
    cx.run_until_parked();
}

#[gpui_kit::test]
fn a_sign_in_that_did_not_land_offers_each_way_and_not_retry(cx: &mut TestAppContext) {
    let cx = window(cx);
    ended_with_misses(cx, false);
    for way in ["act-token", "act-key", "act-machine"] {
        assert!(cx.debug_bounds(way).is_some(), "Claude Code's failed sign-in offers {way}");
    }
    assert!(cx.debug_bounds("act-retry").is_none(), "a sign-in row offers its ways, not Retry");
}

#[gpui_kit::test]
fn the_running_view_opens_on_the_first_row_that_needs_you(cx: &mut TestAppContext) {
    let cx = window(cx);
    let dialog = ended_with_misses(cx, true);
    dialog.update(cx, |d, cx| {
        let mut place = d.place().cloned().unwrap();
        for cli in ["jq", "fd", "bat", "ffmpeg", "duckdb", "pandoc"] {
            let row = json!({ "id": format!("clis/{cli}"), "label": cli, "outcome": "failed", "step": "clis", "note": "apt-get could not find it" });
            place["applied"]["rows"].as_array_mut().unwrap().push(row);
        }
        d.take_places(vec![computer::here(), place], vec![], vec![], cx);
    });
    settle(cx);
    cx.run_until_parked();
    let scrolled = read(&dialog, cx, |d, _| d.scrolled());
    assert!(scrolled > px(0.), "the body scrolled down to the failed sign-in: {scrolled:?}");
    let (mark, body) = (cx.debug_bounds("asks-mark").expect("the row that needs you"), cx.debug_bounds("dialog-body").expect("the body"));
    let off = (mark.center().y - body.center().y).abs();
    assert!(off < px(2.), "the row that needs you stands in the body's middle, {off:?} off it");
}

#[gpui_kit::test]
fn each_ssh_host_names_its_login(cx: &mut TestAppContext) {
    let cx = window(cx);
    let dialog = opened(cx);
    dialog.update(cx, |d, cx| {
        let host = computer::shaped::<wsp_host_client::protocol::SshHostSuggestion>(
            json!({ "alias": "lab", "hostName": "10.0.0.9", "user": "root", "from": "config" }),
        );
        d.take_hosts(Ok(vec![host]), cx);
    });
    cx.run_until_parked();
    let fact = cx.debug_bounds("host-lab-fact").expect("a fact at the row's right");
    let wide = cx.update(|window, cx| {
        let family = gpui_kit::component::ActiveTheme::theme(cx).font_family.clone();
        ui::text::width("root@10.0.0.9", 13., family, window)
    });
    assert!((fact.size.width - wide).abs() < px(1.), "the fact is root@10.0.0.9: {:?} wide, the words {wide:?}", fact.size.width);
}

#[gpui_kit::test]
fn the_log_is_read_once_on_open_and_every_3_s_only_while_an_open_row_runs(cx: &mut TestAppContext) {
    let throwaway = common::Throwaway::start();
    let host = host_of(&throwaway);
    let cx = window(cx);
    let mut dialog = None;
    cx.update(|window, cx| dialog = Some(add_computer::open(&host, None, window, cx)));
    let dialog = dialog.unwrap();
    until(cx, "the host's first reads", |cx| read(&dialog, cx, |d, _| !d.here().is_empty() && d.recipes_read()));
    let running = computer::setup(
        "running",
        json!([{ "step": "floor", "state": "done", "ms": 4_000 }, { "step": "agents", "state": "running" }]),
        json!([]),
    );
    dialog.update(cx, |d, cx| {
        d.take_places(vec![computer::here(), computer::joined(Some(running), json!([]))], vec![], vec![], cx);
        d.stage(AddStep::Running, computer::BOX, None, Some("p_box"), None, cx);
    });
    let reads = |cx: &mut VisualTestContext| {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(200));
        cx.run_until_parked();
        read(&dialog, cx, |d, _| d.log_reads())
    };
    for _ in 0..5 {
        cx.update(|window, _| window.refresh());
    }
    assert_eq!(reads(cx), 0, "no row stands open, so nothing is read however often it draws");
    dialog.update(cx, |d, cx| d.open_row("floor", true, cx));
    assert_eq!(reads(cx), 1, "opening a row reads once");
    cx.executor().advance_clock(OUTPUT_EVERY * 3);
    assert_eq!(reads(cx), 1, "a step that ended is not read again");
    dialog.update(cx, |d, cx| d.open_row("agents", true, cx));
    assert_eq!(reads(cx), 2, "another row opening reads once more");
    cx.executor().advance_clock(OUTPUT_EVERY);
    assert_eq!(reads(cx), 3, "a running row open is read every 3 s");
    cx.executor().advance_clock(OUTPUT_EVERY);
    assert_eq!(reads(cx), 4);
    dialog.update(cx, |d, cx| d.open_row("agents", false, cx));
    cx.executor().advance_clock(OUTPUT_EVERY * 3);
    assert_eq!(reads(cx), 5, "the open rows no longer running read once more, as the web's effect on `live` does, then nothing");
    dialog.update(cx, |d, cx| d.open_row("agents", true, cx));
    assert_eq!(reads(cx), 6);
    let ended = json!({ "type": "place.setup", "placeId": "p_box", "addId": "s_fixture", "line": { "step": "agents", "state": "failed" } });
    dialog.update(cx, |d, cx| d.apply(&ended, cx));
    assert_eq!(reads(cx), 7, "the open row ending reads its last lines once");
    cx.executor().advance_clock(OUTPUT_EVERY * 3);
    assert_eq!(reads(cx), 7, "and nothing after");
}

#[gpui_kit::test]
fn an_add_the_host_no_longer_lists_ends(cx: &mut TestAppContext) {
    let cx = window(cx);
    let dialog = opened(cx);
    dialog.update(cx, |d, cx| {
        d.stage(
            AddStep::Checks,
            computer::BOX,
            Some(computer::job("running", json!([{ "step": "connect", "state": "running" }]))),
            None,
            None,
            cx,
        );
        d.take_places(vec![computer::here()], vec![], vec![], cx);
    });
    let job = read(&dialog, cx, |d, _| d.job().cloned().unwrap());
    assert_eq!((job["state"].as_str(), job["said"].as_str()), (Some("failed"), Some(add_computer::words::HOST_LOST)));
}

#[gpui_kit::test]
fn a_press_with_nothing_sent_holds_nothing(cx: &mut TestAppContext) {
    let cx = window(cx);
    let dialog = opened(cx);
    dialog.update(cx, |d, cx| {
        let mut rows = computer::landed_rows();
        let row = json!({ "id": "agents/mcp/sentry", "label": "sentry", "outcome": "skipped", "step": "mcp", "keys": ["SENTRY_TOKEN"] });
        rows.as_array_mut().unwrap().push(row);
        let mut place = computer::joined(Some(computer::setup("done", computer::done_steps(), json!([]))), rows);
        // A computer that carries no picks: Copy the keys has nothing to send them with.
        place.as_object_mut().unwrap().remove("picks");
        d.take_places(vec![computer::here(), place], vec![], vec![], cx);
        d.stage(AddStep::Running, computer::BOX, None, Some("p_box"), None, cx);
    });
    settle(cx);
    let copy = cx.debug_bounds("act-copy-keys").expect("sentry's Copy the keys");
    cx.simulate_click(copy.center(), Modifiers::none());
    settle(cx);
    assert!(cx.debug_bounds("refusal").is_some(), "the press reached the row: it says the computer has nothing picked");
    assert!(!read(&dialog, cx, |d, _| d.busy()), "with no host behind it nothing was sent, so no act is held");
}

#[gpui_kit::test]
fn a_sign_in_runs_under_its_row_and_cancel_closes_it(cx: &mut TestAppContext) {
    let cx = window(cx);
    let dialog = ended_with_misses(cx, false);
    let machine = cx.debug_bounds("act-machine").expect("Sign in on lab-box");
    cx.simulate_click(machine.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(read(&dialog, cx, |d, _| d.sign_in_of("signins/claude").is_some()), "the run stands under its row");
    assert!(cx.debug_bounds("sign-in-view").is_some(), "its page, code or spinner is drawn under the row");
    assert!(cx.debug_bounds("act-machine").is_none(), "while it stands its act is Cancel");
    let cancel = cx.debug_bounds("act-cancel").expect("Cancel");
    cx.simulate_click(cancel.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("sign-in-view").is_none(), "Cancel takes it down");
}

#[gpui_kit::test]
fn a_key_already_picked_opens_the_vault_field(cx: &mut TestAppContext) {
    let cx = window(cx);
    let dialog = ended_with_misses(cx, false);
    dialog.update(cx, |d, cx| {
        let mut place = d.place().cloned().unwrap();
        place["picks"]["agents"]["claude"] = json!({ "signin": "key" });
        d.take_places(vec![computer::here(), place], vec![], vec![], cx);
    });
    cx.run_until_parked();
    let key = cx.debug_bounds("act-key").expect("Use an API key");
    cx.simulate_click(key.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        matches!(read(&dialog, cx, |d, _| d.sign_in_of("signins/claude").cloned()), Some(sign_in::Flow::Vault { token: false, .. })),
        "the key is pasted under the row into this host's vault"
    );
    assert!(cx.debug_bounds("control").is_some(), "the key's field stands under the row");
}

#[gpui_kit::test]
fn a_refusal_draws_its_sentence_and_its_fix(cx: &mut TestAppContext) {
    let throwaway = common::Throwaway::start();
    let host = host_of(&throwaway);
    let cx = window(cx);
    let mut dialog = None;
    cx.update(|window, cx| dialog = Some(add_computer::open(&host, None, window, cx)));
    let dialog = dialog.unwrap();
    until(cx, "the host's first reads", |cx| read(&dialog, cx, |d, _| !d.here().is_empty() && d.recipes_read()));
    dialog.update(cx, |d, cx| {
        let mut rows = computer::landed_rows();
        let row = json!({ "id": "clis/jq", "label": "jq", "outcome": "failed", "step": "clis", "note": "apt-get could not find it" });
        rows.as_array_mut().unwrap().push(row);
        let place = computer::joined(Some(computer::setup("done", computer::done_steps(), json!([]))), rows);
        d.take_places(vec![computer::here(), place], vec![], vec![], cx);
        d.stage(AddStep::Running, computer::BOX, None, Some("p_box"), None, cx);
    });
    settle(cx);
    // The host holds no computer p_box, so Retry there is refused with what to do about it.
    let retry = cx.debug_bounds("act-retry").expect("jq's Retry");
    cx.simulate_click(retry.center(), Modifiers::none());
    until(cx, "the refusal", |cx| read(&dialog, cx, |d, _| d.row_refused().is_some()));
    let (row, said) = read(&dialog, cx, |d, _| d.row_refused().cloned().unwrap());
    println!("Retry refused: {said:?}");
    assert_eq!(row, "clis/jq", "the refusal stands under the row that asked");
    let fix = said.fix.clone().expect("the host said what to do");
    assert!(!said.said.is_empty() && !said.said.ends_with(fix.as_str()), "the sentence alone, its fix apart: {said:?}");
    settle(cx);
    assert!(cx.debug_bounds("refusal").is_some(), "the refusal stands under the row");
}

#[gpui_kit::test]
fn a_refusal_with_a_fix_draws_both_parts(cx: &mut TestAppContext) {
    let cx = window(cx);
    let dialog = opened(cx);
    let fix = "Check that the file is yours and readable, then open Add a computer again; until then type the address of the computer you mean to add, as user@host.";
    dialog.update(cx, |d, cx| {
        d.take_hosts(Err(model::Said { said: "permission denied reading ~/.ssh/config".into(), fix: Some(fix.into()) }), cx)
    });
    settle(cx);
    let drawn = cx.debug_bounds("refusal").expect("the refusal under the address");
    // The sentence alone fits a line; with its fix after it the slot runs to a third.
    assert!(drawn.size.height > px(36.), "the fix is drawn after the sentence: {:?} high", drawn.size.height);
}

/// The window's own store, in a folder of the test's, where a pending add's step is kept.
fn keeping(cx: &mut VisualTestContext) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let store = desktop_gpui::local_store::LocalStore::open(dir.path().join("local-store.json"));
    cx.update(|_, cx| cx.set_global(store));
    dir
}

/// `p_box` joined and pending under the add `a_fixture`, with its picks, as `places.list` lists it.
fn pending_box() -> serde_json::Value {
    computer::shaped::<wsp_host_client::protocol::PendingComputer>(json!({
        "id": "a_fixture", "address": computer::BOX, "placeId": "p_box", "step": "choosing", "choices": computer::picks(),
        "startedAt": "2026-10-18T10:00:00Z"
    }))
}

/// The dialog opened on the pending add's id, as its sidebar row opens it, the host's list landing behind it.
fn reopened(cx: &mut VisualTestContext, adds: Vec<serde_json::Value>) -> Entity<AddComputer> {
    cx.update(|window, cx| layer::close(ADD_COMPUTER, window, cx));
    let dialog = opened(cx);
    dialog.update(cx, |d, cx| {
        d.stage(AddStep::Running, "", None, Some("a_fixture"), None, cx);
        d.take_places(vec![computer::here(), computer::joined(None, json!([]))], adds, vec![pending_box()], cx);
    });
    cx.run_until_parked();
    dialog
}

/// Catches: a pending add opened again on its first pick whatever step it was left at, or the step not kept per add.
#[gpui_kit::test]
fn a_pending_add_opens_again_where_it_was_left(cx: &mut TestAppContext) {
    let cx = window(cx);
    let _store = keeping(cx);
    let dialog = reopened(cx, vec![]);
    assert_eq!(read(&dialog, cx, |d, _| d.step()), AddStep::Agents, "never left anywhere, it opens on its first pick");
    dialog.update(cx, |d, cx| d.go(AddStep::Skills, cx));
    let dialog = reopened(cx, vec![]);
    assert_eq!(read(&dialog, cx, |d, _| d.step()), AddStep::Skills, "it opens on the step it was left at");
}

/// Catches: an add closed on its checks before a pick opening past them, or the checks kept after Continue.
#[gpui_kit::test]
fn a_pending_add_closed_on_its_checks_opens_on_them(cx: &mut TestAppContext) {
    let cx = window(cx);
    let _store = keeping(cx);
    let mut add = computer::job("done", json!([{ "step": "join", "state": "done" }]));
    add["placeId"] = json!("p_box");
    let dialog = reopened(cx, vec![add.clone()]);
    assert_eq!(read(&dialog, cx, |d, _| d.step()), AddStep::Checks, "closed on its checks, it opens on them");
    dialog.update(cx, |d, cx| d.go(AddStep::Agents, cx));
    let dialog = reopened(cx, vec![add]);
    assert_eq!(read(&dialog, cx, |d, _| d.step()), AddStep::Agents, "once Continue went on, it opens on the picks");
}

/// The sentences whose toast stands in the window.
fn toasts(cx: &mut VisualTestContext) -> Vec<String> {
    let toasts = cx.update(|window, cx| ui::toast::of(window, cx).expect("ui::init registers the stack"));
    toasts.read_with(cx, |t, _| t.standing().iter().map(|n| n.text.to_string()).collect())
}

/// Catches: a recipe the host would not save after Set up dropped with nothing said.
#[gpui_kit::test]
fn a_recipe_save_refused_after_set_up_is_said_as_a_notice(cx: &mut TestAppContext) {
    let cx = window(cx);
    let dialog = opened(cx);
    let refusal = wsp_host_client::Failure::new("A recipe named lab-boxes is already saved.");
    dialog.update(cx, |d, cx| d.take_saved(Err(refusal), "rocket", cx));
    cx.run_until_parked();
    assert_eq!(toasts(cx), ["A recipe named lab-boxes is already saved."]);
}

/// The notices standing in the window, oldest first.
fn notices(cx: &mut VisualTestContext) -> Vec<ui::Notice> {
    let toasts = cx.update(|window, cx| ui::toast::of(window, cx).expect("ui::init registers the stack"));
    toasts.read_with(cx, |t, _| t.standing())
}

/// Catches: a setup closed while it runs saying nothing, saying it twice for one run, or a close by Escape skipping it.
#[gpui_kit::test]
fn closing_on_a_setup_that_runs_says_it_goes_on_once(cx: &mut TestAppContext) {
    let cx = window(cx);
    let running =
        || computer::joined(Some(computer::setup("running", json!([{ "step": "floor", "state": "running" }]), json!([]))), json!([]));
    for _ in 0..2 {
        let dialog = opened(cx);
        dialog.update(cx, |d, cx| {
            d.take_places(vec![computer::here(), running()], vec![], vec![], cx);
            d.stage(AddStep::Running, computer::BOX, None, Some("p_box"), None, cx);
        });
        cx.update(layer::dismiss);
        cx.run_until_parked();
    }
    let said: Vec<(String, Option<String>)> =
        notices(cx).iter().map(|n| (n.text.to_string(), n.detail.as_ref().map(|d| d.to_string()))).collect();
    assert_eq!(said, [("lab-box is being set up; you can close this".to_owned(), Some("wsp says when it is ready.".to_owned()))]);
    assert!(notices(cx)[0].action.as_ref().is_some_and(|a| a.word == "Open"), "its Open goes back to the running view");
}

/// Catches: a setup that ended said as still running, or its misses and its Needs you left out.
#[gpui_kit::test]
fn closing_on_a_setup_that_ended_says_how_it_ended(cx: &mut TestAppContext) {
    let cx = window(cx);
    let dialog = ended_with_misses(cx, false);
    dialog.update_in(cx, |d, window, cx| d.close(window, cx));
    cx.run_until_parked();
    let said = notices(cx);
    assert_eq!(said.len(), 1);
    assert_eq!(said[0].text.as_ref(), "lab-box is ready: threads you start on it run there");
    let detail = said[0].detail.as_ref().map(|d| d.to_string()).unwrap_or_default();
    assert!(detail.ends_with(". Finish them from lab-box's page in Settings."), "it names what the setup left: {detail}");
}

/// Catches: a row's refusal standing after the setup or the recipe's sync it spoke of has moved on.
#[gpui_kit::test]
fn a_rows_refusal_goes_once_the_sync_moves_on(cx: &mut TestAppContext) {
    let cx = window(cx);
    let dialog = opened(cx);
    dialog.update(cx, |d, cx| {
        let mut rows = computer::landed_rows();
        let row = json!({ "id": "agents/mcp/sentry", "label": "sentry", "outcome": "skipped", "step": "mcp", "keys": ["SENTRY_TOKEN"] });
        rows.as_array_mut().unwrap().push(row);
        let mut place = computer::joined(Some(computer::setup("done", computer::done_steps(), json!([]))), rows);
        place.as_object_mut().unwrap().remove("picks");
        d.take_places(vec![computer::here(), place], vec![], vec![], cx);
        d.stage(AddStep::Running, computer::BOX, None, Some("p_box"), None, cx);
    });
    settle(cx);
    let copy = cx.debug_bounds("act-copy-keys").expect("sentry's Copy the keys");
    cx.simulate_click(copy.center(), Modifiers::none());
    settle(cx);
    assert!(read(&dialog, cx, |d, _| d.row_refused().is_some()), "the press is refused under its row");
    dialog.update(cx, |d, cx| d.apply(&json!({ "type": "place.sync", "placeId": "p_box", "sync": { "state": "running" } }), cx));
    assert!(read(&dialog, cx, |d, _| d.row_refused().is_none()), "a sync that moved on takes the refusal away");
}
