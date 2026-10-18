// SPDX-License-Identifier: AGPL-3.0-only
//! A launch: the lock per home, the starting window's 800 ms, the routing to onboarding or the app, onboarding's
//! reads and its one write, and first run, each against a host started for the test alone (host-client's
//! tests/common) whose claude is a stand-in first on its PATH.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::screens::dialogs::first_run::{FirstRun, Holds};
use desktop_gpui::screens::dialogs::onboarding::{self, Onboarding, Road, Step};
use desktop_gpui::screens::dialogs::starting;
use desktop_gpui::screens::dialogs::startup::{self, Claim, Next, STARTING_SHOWN};
use desktop_gpui::screens::Host;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::ui::{self, Side};
use futures::channel::mpsc::TryRecvError;
use gpui_kit::component::Root;
use gpui_kit::{div, prelude::*, px, size, AppContext, Context, Entity, TestAppContext, VisualTestContext, Window};
use serde_json::json;
use wsp_host_client::protocol::AgentsReadRequest;
use wsp_host_client::HostClient;

const HOLD: Duration = Duration::from_secs(60);

fn runtime() -> Arc<tokio::runtime::Runtime> {
    Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap())
}

fn client(runtime: &tokio::runtime::Runtime, host: &Throwaway) -> HostClient {
    runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message))
}

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
}

/// Runs the window's tasks until `done` says so, the host's answers arriving on the tokio runtime's threads.
fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let limit = Instant::now() + HOLD;
    while !done(cx) {
        assert!(Instant::now() < limit, "{what} did not happen in {} s", HOLD.as_secs());
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn one_app_holds_a_home_until_it_ends() {
    let (one, two) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let Ok(Claim::Held(mut first)) = startup::claim(one.path()) else { panic!("the first launch on a home takes its lock") };
    let mut raised = first.raised().expect("the holder listens for later launches");
    assert!(matches!(startup::claim(one.path()), Ok(Claim::Taken)), "a second launch on the same home is refused");
    let limit = Instant::now() + Duration::from_secs(5);
    while !matches!(raised.try_recv(), Ok(())) {
        assert!(Instant::now() < limit, "the refused launch asks the holder to come forward");
        std::thread::sleep(Duration::from_millis(20));
    }
    let Ok(Claim::Held(other)) = startup::claim(two.path()) else { panic!("a launch on another home is not refused") };
    drop(first);
    let limit = Instant::now() + Duration::from_secs(5);
    loop {
        match raised.try_recv() {
            Err(TryRecvError::Closed) => break,
            Ok(()) => panic!("the lock going raised the window"),
            Err(TryRecvError::Empty) => assert!(Instant::now() < limit, "the listener ends with the lock"),
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!one.path().join("desktop-gpui.sock").exists(), "and its socket goes, so a later knock reaches nobody");
    assert!(matches!(startup::claim(one.path()), Ok(Claim::Held(_))), "the lock goes when its holder ends");
    drop(other);
}

#[test]
fn a_home_the_lock_makes_is_the_persons_alone() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home").join(".wsp");
    // SAFETY: umask only swaps the process mask; the usual 022 is set so the mode below is the lock's own, then put back.
    let was = unsafe { libc::umask(0o022) };
    let claimed = startup::claim(&home);
    // SAFETY: as above, putting the mask back.
    unsafe { libc::umask(was) };
    let Ok(Claim::Held(_lock)) = claimed else { panic!("a fresh home takes its lock") };
    for made in [home.as_path(), home.parent().unwrap()] {
        let mode = std::fs::metadata(made).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "{} is made at 0700, as wsp makes its home", made.display());
    }
}

#[gpui_kit::test]
fn no_starting_window_when_the_next_window_stands_in_time(cx: &mut TestAppContext) {
    init(cx);
    let starting = cx.update(|cx| startup::starting_after(STARTING_SHOWN, starting::open, cx));
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    cx.update(|cx| starting.end(cx));
    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    assert_eq!(cx.update(|cx| cx.windows().len()), 0, "an open done in 500 ms shows no starting window");
}

#[gpui_kit::test]
fn the_starting_window_stands_after_800_ms_until_ended(cx: &mut TestAppContext) {
    init(cx);
    let starting = cx.update(|cx| startup::starting_after(STARTING_SHOWN, starting::open, cx));
    cx.executor().advance_clock(Duration::from_millis(790));
    cx.run_until_parked();
    assert_eq!(cx.update(|cx| cx.windows().len()), 0, "nothing stands before 800 ms");
    cx.executor().advance_clock(Duration::from_millis(20));
    cx.run_until_parked();
    assert_eq!(cx.update(|cx| cx.windows().len()), 1, "the starting window stands at 800 ms");
    assert!(starting.shown().is_some());
    cx.update(|cx| starting.end(cx));
    cx.run_until_parked();
    assert_eq!(cx.update(|cx| cx.windows().len()), 0, "ending the launch takes it away");
}

#[test]
fn an_empty_host_opens_onboarding_and_one_holding_a_project_the_app() {
    let runtime = runtime();
    let host = Throwaway::start();
    let empty = client(&runtime, &host);
    assert_eq!(runtime.block_on(startup::next(&empty)).unwrap(), Next::Onboarding, "a host holding nothing opens onboarding");
    let shared = &common::shared().host;
    let holding = client(&runtime, shared);
    assert_eq!(runtime.block_on(startup::next(&holding)).unwrap(), Next::App, "a host holding a project opens the app");
}

#[test]
fn onboarding_reads_the_agents_here_and_writes_the_tools_into_the_ticked_one() {
    let runtime = runtime();
    let host = Throwaway::start();
    let c = client(&runtime, &host);
    let agents = runtime.block_on(startup::agents(&c)).unwrap();
    assert_eq!(agents.first().map(|a| a.id.as_str()), Some("claude"), "the catalog's order: {agents:?}");
    assert!(agents.len() >= 8, "every catalog agent, found or not: {agents:?}");
    let claude = agents.iter().find(|a| a.id == "claude").unwrap();
    assert!(claude.found && !claude.installs, "the stand-in claude on the host's PATH is found: {claude:?}");
    assert_eq!(onboarding::state_word(claude), "installed");

    let read = || -> bool {
        let ask: AgentsReadRequest = serde_json::from_value(json!({ "target": { "placeId": "here" } })).unwrap();
        let report = runtime.block_on(c.call(ask)).unwrap().report;
        report.agents.iter().find(|a| a.id == "claude").unwrap().wsp_tools
    };
    assert!(!read(), "a fresh home's claude has no wsp tools yet");
    let refused = runtime.block_on(startup::add_tools(&c, vec!["claude".into()]));
    assert!(refused.is_empty(), "claude's config takes the tools: {refused:?}");
    assert!(read(), "claude's config in the test's home names the wsp server");
    let refused = runtime.block_on(startup::add_tools(&c, vec!["nobody".into()]));
    assert_eq!(refused.len(), 1, "an agent the catalog does not know is refused, by name: {refused:?}");
    assert_eq!(refused[0].0, "nobody");
}

#[gpui_kit::test]
fn enter_moves_on_then_opens_wsp_once_the_tools_are_in(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    cx.executor().allow_parking();
    init(cx);
    let runtime = runtime();
    let handle = Host::new(Arc::new(client(&runtime, &host)), runtime);
    let finished = Rc::new(Cell::new(false));
    let mut screen = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let done = finished.clone();
        let road = Road { host: handle, finish: Rc::new(move |_, _| done.set(true)) };
        let view = cx.new(|cx| Onboarding::new(road, window, cx));
        screen = Some(view.clone());
        Root::new(view, window, cx)
    });
    let screen: Entity<Onboarding> = screen.unwrap();
    cx.simulate_resize(size(px(1280.), px(788.)));
    until(cx, "the scan", |cx| screen.read_with(cx, |s, _| !s.agents().is_empty()));
    screen.read_with(cx, |s, _| {
        let found: Vec<bool> = s.agents().iter().map(|a| a.found).collect();
        assert!(found.windows(2).all(|w| w[0] >= w[1]), "found agents first: {found:?}");
        assert!(s.here().is_some_and(|h| !h.is_empty()), "the computer's name lands");
        assert!(s.ticked().contains(&"claude".to_owned()), "a found agent starts ticked");
        assert!(s.agents().iter().filter(|a| !a.found).all(|a| !s.ticked().contains(&a.id)), "one not found is never ticked");
        assert_eq!(s.line(), onboarding::SOME);
    });
    let unfound = screen.read_with(cx, |s, _| s.agents().iter().find(|a| !a.found).map(|a| a.id.clone()));
    if let Some(id) = unfound {
        screen.update(cx, |s, cx| s.toggle(&id, cx));
        assert!(!screen.read_with(cx, |s, _| s.ticked().contains(&id)), "a row not found takes no tick");
    }
    assert_eq!(screen.read_with(cx, |s, _| s.step()), Step::Welcome);
    cx.simulate_keystrokes("enter");
    assert_eq!(screen.read_with(cx, |s, _| s.step()), Step::Agents, "Enter moves the welcome on");
    let ticked = screen.read_with(cx, |s, _| s.ticked());
    screen.update(cx, |s, cx| ticked.iter().for_each(|id| s.toggle(id, cx)));
    assert_eq!(screen.read_with(cx, |s, _| (s.held(), s.line())), (true, onboarding::NONE_TICKED.to_owned()), "no tick holds the press");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(!finished.get(), "a held press opens nothing");
    screen.update(cx, |s, cx| s.toggle("claude", cx));
    cx.simulate_keystrokes("enter");
    until(cx, "Open wsp", |_| finished.get());
    assert!(screen.read_with(cx, |s, _| s.refused().is_none()), "the tools went in");
}

/// The first run over a blank view, as the shell's centre holds it.
struct Centre {
    first_run: Entity<FirstRun>,
}

impl Render for Centre {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.first_run.clone())
    }
}

#[gpui_kit::test]
fn first_run_shows_on_a_host_with_no_project_and_not_on_one_with(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    cx.executor().allow_parking();
    init(cx);
    let runtime = runtime();
    let (state, env) = (host.state(), host.env());
    let (_, vcx) = cx.add_window_view(|window, cx| {
        let shell = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
        Root::new(shell, window, cx)
    });
    vcx.simulate_resize(size(px(1280.), px(800.)));
    until(vcx, "first run in the shell's centre", |cx| cx.debug_bounds("first-run").is_some());

    let shared = &common::shared().host;
    let (first_run, vcx) = first_run_on(cx, shared, desktop_gpui::sidebar::host_page(&shared.state()));
    until(vcx, "the shared host's answer", |cx| first_run.read_with(cx, |f, _| f.holds().is_some()));
    assert!(!first_run.read_with(vcx, |f, _| f.showing()), "a host holding a project shows no first run");
    assert!(vcx.debug_bounds("first-run").is_none());
}

/// First run on `host` over a blank view, its usage line read off the page at `page`.
fn first_run_on<'a>(cx: &'a mut TestAppContext, host: &Throwaway, page: Option<String>) -> (Entity<FirstRun>, &'a mut VisualTestContext) {
    let runtime = self::runtime();
    let handle = Host::new(Arc::new(client(&runtime, host)), runtime);
    let mut first_run = None;
    let (_, vcx) = cx.add_window_view(|window, cx| {
        let behind = cx.new(|_| Blank).into();
        let view = cx.new(|cx| FirstRun::new(handle, page, behind, cx));
        first_run = Some(view.clone());
        let centre = cx.new(|_| Centre { first_run: view });
        Root::new(centre, window, cx)
    });
    vcx.simulate_resize(size(px(1280.), px(800.)));
    (first_run.unwrap(), vcx)
}

/// A staged first run whose host's page did or did not keep the counts off.
fn staged_first_run(cx: &mut TestAppContext, usage: bool) -> &mut VisualTestContext {
    let (_, vcx) = cx.add_window_view(|window, cx| {
        let behind = cx.new(|_| Blank).into();
        let holds = Holds { projects: 0, workspaces: 0, here: "mini".into() };
        let view = cx.new(|_| FirstRun::staged(holds, usage, behind));
        let centre = cx.new(|_| Centre { first_run: view });
        Root::new(centre, window, cx)
    });
    vcx.simulate_resize(size(px(1280.), px(800.)));
    vcx.run_until_parked();
    vcx
}

#[gpui_kit::test]
fn the_usage_line_draws_only_where_the_hosts_page_lets_its_switch_decide(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    cx.executor().allow_parking();
    init(cx);
    let (first_run, vcx) = first_run_on(cx, &host, desktop_gpui::sidebar::host_page(&host.state()));
    until(vcx, "the page's answer", |cx| first_run.read_with(cx, |f, _| f.holds().is_some() && f.usage_off().is_some()));
    assert!(vcx.debug_bounds("first-run").is_some(), "an empty host shows first run");
    // A checkout's host carries no analytics key, so its page says why its counts are off.
    assert_eq!(first_run.read_with(vcx, |f, _| f.usage_off().cloned()), Some(Ok(Some("build".to_owned()))));
    assert!(vcx.debug_bounds("product-usage").is_none(), "a page that keeps the counts off draws no usage line");

    let (first_run, vcx) = first_run_on(cx, &host, Some("http://127.0.0.1:9/".to_owned()));
    until(vcx, "the failed read", |cx| first_run.read_with(cx, |f, _| f.holds().is_some() && f.usage_off().is_some()));
    assert!(matches!(first_run.read_with(vcx, |f, _| f.usage_off().cloned()), Some(Err(_))), "a page nobody serves is a failed read");
    assert!(vcx.debug_bounds("product-usage").is_none(), "and draws no usage line either");

    let vcx = staged_first_run(cx, true);
    assert!(vcx.debug_bounds("product-usage").is_some(), "a page that lets the switch decide draws the line");
    let vcx = staged_first_run(cx, false);
    assert!(vcx.debug_bounds("product-usage").is_none());
}

struct Blank;

impl Render for Blank {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

#[test]
fn a_failed_start_says_its_first_line_and_opens_the_hosts_log() {
    assert_eq!(startup::first_line("\n  no host answered at 127.0.0.1:7777\nstack"), "no host answered at 127.0.0.1:7777");
    assert_eq!(startup::first_line(" \n"), startup::START_FAILED);
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("state.json");
    assert_eq!(startup::log_of(&state), dir.path(), "no host log yet: its folder");
    std::fs::write(dir.path().join("host.log"), "").unwrap();
    assert_eq!(startup::log_of(&state), dir.path().join("host.log"));
}

#[test]
fn a_refusal_names_three_whole_and_past_that_two_and_how_many_more() {
    let names = |n: &[&str]| n.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(onboarding::tools_refused(&names(&["Codex"])), "Codex's config would not take the wsp tools.");
    assert_eq!(onboarding::capped(&names(&["A", "B", "C"])), "A, B and C");
    assert_eq!(onboarding::capped(&names(&["A", "B", "C", "D", "E"])), "A, B and 3 more");
    assert_eq!(onboarding::sentence(None), "Each ticked agent gets the wsp tools and skill, so it can open threads and tasks.");
    assert!(onboarding::sentence(Some("mini")).ends_with("threads and tasks on mini."));
}

#[test]
fn the_usage_line_follows_the_boot_line_of_the_hosts_page() {
    let host = Throwaway::start();
    let page = desktop_gpui::sidebar::host_page(&host.state()).expect("the host's lock names its port");
    // A checkout's host carries no analytics key, so its page says why its counts are off and the line stays hidden.
    assert_eq!(desktop_gpui::screens::dialogs::first_run::usage_off(&page), Ok(Some("build".to_owned())));
    let read = desktop_gpui::screens::dialogs::first_run::usage_off_in;
    assert_eq!(
        read(r#"<head><script>window.__WSP__ = {"token":"t","paired":true};</script>"#),
        Some(None),
        "a switch that decides shows the line"
    );
    assert_eq!(read(r#"<script>window.__WSP__ = {"productUsageOff":"env"};</script>"#), Some(Some("env".to_owned())));
    assert_eq!(read("<html></html>"), None, "a page with no boot line says nothing");
}
