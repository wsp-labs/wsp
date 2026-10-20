// SPDX-License-Identifier: AGPL-3.0-only
//! The New thread page as today's app draws it: the centre with nothing open is the first project's page
//! (`WorkspaceCenter`, `useHomeProject`), its headline and field (EmptyHero.tsx, `EmptyThread`), and the row under every
//! composer (ComposerCheckoutRow.tsx): the computer by its own mark, the branch, and the placeholder's slash hint.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::composer::screen::ComposerScreen;
use desktop_gpui::screens::composer::{self, Composer};
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::sidebar::host::{Change, Snapshot};
use desktop_gpui::sidebar::Sidebar;
use desktop_gpui::ui::computer_glyph::{provider_mark, ComputerIcon};
use desktop_gpui::ui::status::Still;
use desktop_gpui::ui::{self, Glyph, Side};
use gpui_kit::component::Root;
use gpui_kit::{prelude::*, px, size, AppContext, Context, Entity, TestAppContext, VisualTestContext, Window};
use serde_json::json;

const WAIT: Duration = Duration::from_secs(30);

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + WAIT;
    while !done(cx) {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
        cx.run_until_parked();
    }
}

fn git(host: &common::Throwaway, folder: &std::path::Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git").args(args).current_dir(folder).env_clear().envs(host.env()).output().unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

/// A project of its own named `name`, its folder on a new branch `branch`, with the workspace its first thread makes.
fn project_on(host: &common::Throwaway, name: &str, branch: &str) {
    let folder = host.repo(name);
    git(host, &folder, &["checkout", "-q", "-b", branch]);
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", name, "first", "--agent", "claude"]);
}

/// The shell in a 1280 by 800 window, opened on no thread, on a host holding one project, `repo`.
fn launched(cx: &mut TestAppContext, still: bool) -> (common::Throwaway, String, Entity<Shell>, &mut VisualTestContext) {
    launched_with(cx, still, |_| {})
}

/// As [`launched`], with what `more` adds to the host after `repo`.
fn launched_with(
    cx: &mut TestAppContext,
    still: bool,
    more: impl FnOnce(&common::Throwaway),
) -> (common::Throwaway, String, Entity<Shell>, &mut VisualTestContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    // A project's New thread page here is its folder's workspace's, which its first thread makes.
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let branch = git(&host, &folder, &["rev-parse", "--abbrev-ref", "HEAD"]);
    more(&host);
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
        desktop_gpui::screens::palette::init(cx);
        composer::init(cx);
        if still {
            cx.set_global(Still(true));
        }
    });
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap());
    let (state, env) = (host.state(), host.env());
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
        shell = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    let shell = shell.unwrap();
    until(cx, "the project's New thread page", |cx| {
        shell.read_with(cx, |s, _| matches!(s.route(), Route::Screen(r) if r.starts_with("composer:")))
    });
    (host, branch, shell, cx)
}

fn page(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Entity<ComposerScreen> {
    cx.read(|cx| shell.read(cx).screen_view().and_then(|v| v.downcast::<ComposerScreen>().ok()).expect("the composer screen"))
}

/// With no thread open the window is the project's New thread page, as its home, never a page that asks to pick one:
/// its crumbs, the question naming the project, and the row under the box with the computer and the folder's branch.
#[gpui_kit::test]
fn with_no_thread_open_the_window_is_the_projects_new_thread_page(cx: &mut TestAppContext) {
    let (_host, branch, shell, cx) = launched(cx, false);
    assert_eq!(shell.read_with(cx, |s, _| s.selected()), None, "a project's home selects no workspace");
    until(cx, "the headline", |cx| cx.debug_bounds("hero-question:What should we build in repo?").is_some());
    assert!(cx.debug_bounds("crumbs-project").is_some() && cx.debug_bounds("crumbs-title").is_some(), "repo / New thread");
    assert!(cx.debug_bounds("hero-field").is_some(), "the field behind the page");
    until(cx, "the computer in the row under the box", |cx| cx.debug_bounds("tray-computer").is_some());
    let chip: &'static str = Box::leak(format!("tray-branch:{branch}").into_boxed_str());
    until(cx, "the project's branch in the row", |cx| cx.debug_bounds(chip).is_some());
}

/// `ProjectHome`: the headline's foot 40 px over the composer, the two one stack centred in the column under the
/// 52 px header, over 8vh of room below it.
#[gpui_kit::test]
fn the_headline_and_the_composer_stand_as_one_centred_stack(cx: &mut TestAppContext) {
    let (_host, _, _shell, cx) = launched(cx, false);
    until(cx, "the headline", |cx| cx.debug_bounds("hero-heading").is_some());
    cx.run_until_parked();
    let heading = cx.debug_bounds("hero-heading").unwrap();
    let composer = cx.debug_bounds("composer").unwrap();
    assert_eq!(composer.top() - heading.bottom(), px(40.), "gap-10");
    let stack = composer.bottom() - heading.top();
    let top = px(52.) + (px(800. - 52. - 64.) - stack) / 2.;
    assert!((heading.top() - top).abs() < px(1.), "the stack at {:?}, centred at {top:?}", heading.top());
    let mark = cx.debug_bounds("hero-question:What should we build in repo?").unwrap();
    assert!(mark.top() - heading.top() >= px(40. + 24.), "the 40 px mark and gap-6 over the question");
}

/// The field draws a frame every 120 ms in a view of its own, which the frame draws behind the page: neither the page
/// nor its composer is built again for it.
#[gpui_kit::test]
fn the_field_moves_on_its_own_without_building_the_composer(cx: &mut TestAppContext) {
    let (_host, branch, shell, cx) = launched(cx, false);
    until(cx, "the field", |cx| cx.debug_bounds("hero-field").is_some());
    let page = page(&shell, cx);
    let (hero, composer) = cx.read(|cx| (page.read(cx).hero().clone(), page.read(cx).composer().clone()));
    // The host's answers to the page land before the count starts: the agents it lists and the folder's branch.
    let chip: &'static str = Box::leak(format!("tray-branch:{branch}").into_boxed_str());
    until(cx, "the page settled", |cx| composer.read_with(cx, |c, _| c.hold().is_none()) && cx.debug_bounds(chip).is_some());
    std::thread::sleep(Duration::from_millis(300));
    cx.run_until_parked();
    let (frames, built) = cx.read(|cx| (hero.read(cx).frames(), composer.read(cx).built()));
    for _ in 0..5 {
        cx.executor().advance_clock(Duration::from_millis(120));
        cx.run_until_parked();
    }
    let (frames_after, built_after) = cx.read(|cx| (hero.read(cx).frames(), composer.read(cx).built()));
    assert!(frames_after >= frames + 4, "a frame each 120 ms: {frames} then {frames_after}");
    assert_eq!(built_after, built, "the composer is not built again while the field moves");
}

/// Under reduced motion the field draws one still frame and asks for no other.
#[gpui_kit::test]
fn under_reduced_motion_the_field_draws_once(cx: &mut TestAppContext) {
    let (_host, _, shell, cx) = launched(cx, true);
    until(cx, "the field", |cx| cx.debug_bounds("hero-field").is_some());
    let page = page(&shell, cx);
    let hero = cx.read(|cx| page.read(cx).hero().clone());
    for _ in 0..5 {
        cx.executor().advance_clock(Duration::from_millis(120));
        cx.run_until_parked();
    }
    assert_eq!(cx.read(|cx| hero.read(cx).frames()), 0, "no frame asked for");
}

/// Once the page is left the field asks for no more frames: the one it had asked for is its last.
#[gpui_kit::test]
fn the_field_stops_once_the_page_is_left(cx: &mut TestAppContext) {
    let (_host, _, shell, cx) = launched(cx, false);
    until(cx, "the field", |cx| cx.debug_bounds("hero-field").is_some());
    let page = page(&shell, cx);
    let hero = cx.read(|cx| page.read(cx).hero().clone());
    let moving = cx.read(|cx| hero.read(cx).frames());
    cx.executor().advance_clock(Duration::from_millis(240));
    cx.run_until_parked();
    assert!(cx.read(|cx| hero.read(cx).frames()) > moving, "the field moves while the page stands");
    cx.update(|window, cx| shell.update(cx, |s, cx| s.go(Route::Screen("settings".into()), window, cx)));
    cx.executor().advance_clock(Duration::from_millis(120));
    cx.run_until_parked();
    let left = cx.read(|cx| hero.read(cx).frames());
    for _ in 0..5 {
        cx.executor().advance_clock(Duration::from_millis(120));
        cx.run_until_parked();
    }
    assert_eq!(cx.read(|cx| hero.read(cx).frames()), left, "no frame asked for off the page");
}

/// The workspace of the project other than the one whose home the window opened on.
fn other_workspace(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> String {
    let home = shell.read_with(cx, |s, _| match s.route() {
        Route::Screen(r) => r.trim_start_matches("composer:").to_owned(),
        other => panic!("not a New thread page: {other:?}"),
    });
    let mut other = None;
    until(cx, "the other project's workspace", |cx| {
        other = shell.read_with(cx, |s, cx| s.sidebar().read(cx).items().iter().map(|i| i.workspace_id.clone()).find(|w| *w != home));
        other.is_some()
    });
    other.unwrap()
}

/// Another workspace's New thread page after a project's home is a composer of its own: the home's branch, which the
/// row names over the workspace's, does not come along.
#[gpui_kit::test]
fn a_workspaces_page_after_a_project_home_drops_the_homes_branch(cx: &mut TestAppContext) {
    let (_host, branch, shell, cx) = launched_with(cx, false, |host| project_on(host, "other", "elsewhere"));
    let other = other_workspace(&shell, cx);
    let home_is_repo =
        !shell.read_with(cx, |s, cx| s.sidebar().read(cx).items().iter().any(|i| i.workspace_id == other && i.project == "repo"));
    let home_chip: &'static str = if home_is_repo { format!("tray-branch:{branch}") } else { "tray-branch:elsewhere".to_owned() }.leak();
    until(cx, "the home's branch", |cx| cx.debug_bounds(home_chip).is_some());
    cx.update(|window, cx| shell.update(cx, |s, cx| s.go(Route::Screen(format!("composer:{other}").into()), window, cx)));
    let page = page(&shell, cx);
    until(cx, "the workspace's page", |cx| page.read_with(cx, |p, cx| p.composer().read(cx).hold().is_none()));
    std::thread::sleep(Duration::from_millis(300));
    cx.run_until_parked();
    assert!(cx.debug_bounds(home_chip).is_none(), "the home's branch stayed behind");
}

/// The headline follows a rename of its project, which reaches the page only through the sidebar.
#[gpui_kit::test]
fn the_headline_follows_its_projects_new_name(cx: &mut TestAppContext) {
    let (_host, _, shell, cx) = launched(cx, false);
    until(cx, "the headline", |cx| cx.debug_bounds("hero-question:What should we build in repo?").is_some());
    let workspace = match shell.read_with(cx, |s, _| s.route().clone()) {
        Route::Screen(r) => r.trim_start_matches("composer:").to_owned(),
        other => panic!("not a New thread page: {other:?}"),
    };
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    let mut record = sidebar.read_with(cx, |s, _| s.workspace(&workspace).0.cloned()).expect("the workspace's record");
    record["project"]["name"] = json!("renamed");
    sidebar.update(cx, |s, cx| s.apply(Change::Workspaces(vec![record]), cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("hero-question:What should we build in renamed?").is_some(), "the headline names the new name");
}

/// `projectHome`: Escape out of Settings opened from a project's home lands on that project's home, not the first's.
#[gpui_kit::test]
fn escape_out_of_settings_goes_back_to_the_project_home_it_was_opened_from(cx: &mut TestAppContext) {
    let (_host, _, shell, cx) = launched_with(cx, false, |host| project_on(host, "other", "elsewhere"));
    let other = other_workspace(&shell, cx);
    cx.update(|window, cx| shell.update(cx, |s, cx| s.open_home(&other, window, cx)));
    let home: Route = Route::Screen(format!("composer:{other}").into());
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), home);
    cx.update(|window, cx| shell.update(cx, |s, cx| s.go(Route::Screen("settings".into()), window, cx)));
    cx.run_until_parked();
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), home, "back on the project home Settings was opened from");
}

/// A project's home opened from a thread leaves the thread: Settings opened over the home stands over no thread.
#[gpui_kit::test]
fn settings_over_a_project_home_stands_over_no_thread(cx: &mut TestAppContext) {
    let (_host, _, shell, cx) = launched_with(cx, false, |host| project_on(host, "other", "elsewhere"));
    let other = other_workspace(&shell, cx);
    let thread = shell.read_with(cx, |s, cx| s.sidebar().read(cx).items().iter().find(|i| i.workspace_id != other).cloned()).unwrap();
    let thread = Route::Thread { workspace_id: thread.workspace_id, thread_id: thread.thread_id };
    cx.update(|window, cx| shell.update(cx, |s, cx| s.go(thread, window, cx)));
    cx.update(|window, cx| shell.update(cx, |s, cx| s.open_home(&other, window, cx)));
    cx.update(|window, cx| shell.update(cx, |s, cx| s.go(Route::Screen("settings".into()), window, cx)));
    cx.run_until_parked();
    assert_eq!(shell.read_with(cx, |s, _| s.selected()), None, "no thread selected under Settings");
}

/// The composer alone in a window, its workspace's records in the sidebar the app reads them from.
struct Column {
    composer: Entity<Composer>,
}

impl Render for Column {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        gpui_kit::div().size_full().child(self.composer.clone())
    }
}

fn tray(
    cx: &mut TestAppContext,
    workspace: serde_json::Value,
    places: Vec<serde_json::Value>,
    status: serde_json::Value,
) -> (Entity<Composer>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        composer::init(cx);
    });
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let sidebar = cx.new(Sidebar::new);
        let snapshot = Snapshot { workspaces: vec![workspace], places, statuses: vec![status], ..Default::default() };
        sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot)), cx));
        desktop_gpui::screens::terminal::read_workspaces_from(&sidebar, cx);
        let composer = cx.new(|cx| Composer::new(window, cx));
        composer.update(cx, |c, cx| c.set_workspace("w1".into(), cx));
        made = Some(composer.clone());
        Root::new(cx.new(|_| Column { composer }), window, cx)
    });
    cx.simulate_resize(size(px(1024.), px(800.)));
    cx.run_until_parked();
    (made.unwrap(), cx)
}

/// A thread on Boat: Boat's own mark, and the branch its checkout is on.
#[gpui_kit::test]
fn a_thread_on_a_cloud_names_its_providers_mark_and_its_branch(cx: &mut TestAppContext) {
    let workspace = json!({ "id": "w1", "name": "w1", "kind": "cloud", "provider": "box", "machineId": "m1" });
    let places = vec![json!({ "id": "box", "kind": "provider", "name": "box", "default": false })];
    let status = json!({ "id": "w1", "checkout": { "branch": "wsp/review-pr-pane-2397" } });
    let (composer, cx) = tray(cx, workspace, places, status);
    assert_eq!(composer.read_with(cx, |c, _| c.computer_icon()), Some(ComputerIcon::Mark(provider_mark("box").unwrap())));
    assert!(cx.debug_bounds("tray-branch:wsp/review-pr-pane-2397").is_some(), "the branch beside the computer");
    assert!(cx.debug_bounds("tray-computer:Boat").is_some(), "the computer by its name");
}

/// A thread on this computer, a MacBook: the laptop, and a detached head names no branch.
#[gpui_kit::test]
fn a_thread_here_draws_the_computers_own_glyph_and_no_branch_off_one(cx: &mut TestAppContext) {
    let workspace = json!({ "id": "w1", "name": "w1", "kind": "local", "machineId": "local" });
    let places = vec![json!({ "id": "here", "kind": "computer", "name": "mbp", "default": true, "mac": "macbook" })];
    let status = json!({ "id": "w1", "checkout": { "branch": "(detached)" } });
    let (composer, cx) = tray(cx, workspace, places, status);
    assert_eq!(composer.read_with(cx, |c, _| c.computer_icon()), Some(ComputerIcon::Lucide(Glyph::Laptop)));
    assert!(cx.debug_bounds("tray-computer").is_some());
    assert!(cx.debug_bounds("tray-branch:(detached)").is_none(), "no branch on a detached head");
}

/// A worktree's thread before its status arrives: the branch the worktree was made on.
#[gpui_kit::test]
fn a_worktrees_thread_names_its_branch_before_its_status_comes(cx: &mut TestAppContext) {
    let workspace = json!({ "id": "w1", "name": "w1", "kind": "local", "machineId": "local", "worktree": { "branch": "wsp/fix-it" } });
    let places = vec![json!({ "id": "here", "kind": "computer", "name": "box", "default": true })];
    let (_, cx) = tray(cx, workspace, places, json!({ "id": "w1" }));
    assert!(cx.debug_bounds("tray-branch:wsp/fix-it").is_some());
}

/// The placeholder names the slash menu where it has something to offer: on a thread whose agent asks side questions,
/// `/btw`; on a new thread, with no row to name a side question by, nothing.
#[gpui_kit::test]
fn the_placeholder_offers_the_slash_menu_where_btw_asks_a_side_question(cx: &mut TestAppContext) {
    use desktop_gpui::screens::composer::model::{Agent, Picks};
    let workspace = json!({ "id": "w1", "name": "w1", "kind": "local", "machineId": "local" });
    let (composer, cx) = tray(cx, workspace, Vec::new(), json!({ "id": "w1" }));
    let agent = Agent::read(&json!({ "harness": "claude", "label": "Claude Code", "asides": true })).unwrap();
    let ran = Picks { agent: Some("claude".into()), ..Picks::default() };
    composer.update(cx, |c, cx| c.set_catalog(vec![agent.clone()], ran, Vec::new(), cx));
    cx.run_until_parked();
    assert_eq!(composer.read_with(cx, |c, _| c.placeholder()), "Ask anything, or / for commands", "an open thread");
    composer.update(cx, |c, cx| c.set_catalog(vec![agent], Picks::default(), Vec::new(), cx));
    cx.run_until_parked();
    assert_eq!(composer.read_with(cx, |c, _| c.placeholder()), "Ask anything", "a new thread");
}
