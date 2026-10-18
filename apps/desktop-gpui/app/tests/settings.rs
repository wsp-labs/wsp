// SPDX-License-Identifier: AGPL-3.0-only
//! Settings against a throwaway host of its own (the shared one in `host-client/tests/common`): what it reads as it
//! opens, what it writes, and what it takes from the events the host pushes when another client changes something.
//! It needs node and the packages built (pnpm build).

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[allow(dead_code)]
#[path = "pr_pane/pr_page_host.rs"]
mod pr_page_host;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::dialogs::layer;
use desktop_gpui::screens::settings::model::{req, At, Data, GroupId};
use desktop_gpui::screens::settings::{Opening, SettingsScreen, DIALOG};
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::ui::{self, Side, UiAssets};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, Entity, SharedString, TestAppContext, VisualTestContext};
use serde_json::json;
use wsp_host_client::ops::settings::PreferencesReply;
use wsp_host_client::protocol::{PreferencesGetRequest, PreferencesSetRequest};
use wsp_host_client::{Env, HostClient};

use common::Throwaway;
use pr_page_host::Door;

/// A host carrying the wsp-daemon the shared helper finds, whose claude is the script `claude`, for a test that runs a
/// turn on it.
fn carrying(claude: &str) -> Throwaway {
    let host = Throwaway::start_with(claude, true);
    if host.daemon.is_none() {
        panic!("{}: run apps/desktop-gpui/scripts/stage-daemon.sh", common::wsp_daemon().unwrap_err());
    }
    host
}

fn runtime() -> Arc<tokio::runtime::Runtime> {
    Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap())
}

/// Settings opened on `at` against `host`, in a window as the app draws it.
fn open<'a>(host: &Throwaway, at: At, cx: &'a mut TestAppContext) -> (Entity<SettingsScreen>, &'a mut VisualTestContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let _ = UiAssets;
    let opening = Opening { state: host.state(), env: host.env(), runtime: runtime(), at, scroll: 0. };
    let mut screen = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| SettingsScreen::open(opening, window, cx));
        screen = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    (screen.unwrap(), cx)
}

/// Runs the window until `done` holds, or fails after a while saying `what`.
fn until(cx: &mut VisualTestContext, screen: &Entity<SettingsScreen>, what: &str, done: impl Fn(&SettingsScreen) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        if screen.read_with(cx, |s, _| done(s)) {
            return;
        }
        assert!(Instant::now() < deadline, "Settings never came to: {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The release the throwaway host runs: the version its package names, read beside the files it runs from.
fn host_release() -> String {
    let package = std::fs::read_to_string(common::root().join("packages/host/package.json")).unwrap();
    let package: serde_json::Value = serde_json::from_str(&package).unwrap();
    package["version"].as_str().expect("the host's package names its version").to_owned()
}

/// One op on the host from a client of its own, as another window would send it.
fn other_client<T>(host: &Throwaway, f: impl AsyncFnOnce(HostClient) -> T) -> T {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let client = HostClient::connect(&host.state(), &host.env()).await.unwrap_or_else(|f| panic!("{}", f.message));
        f(client).await
    })
}

#[gpui_kit::test]
fn settings_reads_the_host_as_it_opens(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let folder = host.repo("alpha");
    host.wsp(&["add", &folder.to_string_lossy(), "--name", "alpha"]);
    let (screen, cx) = open(&host, At::Group(GroupId::Projects), cx);

    until(cx, &screen, "the project, the computer here, the preferences, the account and the release read", |s| {
        let d = s.data();
        d.preferences.is_some()
            && d.projects.iter().any(|p| p.name == "alpha")
            && d.places.iter().any(|p| p.id == "here")
            && d.account.is_some()
            && d.release.is_some()
            && d.host_version.is_some()
    });
    screen.read_with(cx, |s, _| {
        let d = s.data();
        assert!(d.projects_refused.is_none() && d.places_refused.is_none());
        assert!(!d.host_elsewhere, "a host on this computer's own state is not on another computer, so it offers Restart host");
    });
}

#[gpui_kit::test]
fn settings_writes_preferences_through_the_host(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let (screen, cx) = open(&host, At::Group(GroupId::Appearance), cx);
    until(cx, &screen, "the preferences read", |s| s.data().preferences.is_some());

    screen.update(cx, |s, cx| s.set_preferences(json!({ "theme": "dark", "darkTheme": "pitch" }), cx));
    // The screen lays the patch on its own records at once, so only the host's copy says the set arrived.
    let deadline = Instant::now() + Duration::from_secs(10);
    let preferences = loop {
        let PreferencesReply { preferences } =
            other_client(&host, async |client| client.call(req::<PreferencesGetRequest>(json!({}))).await.unwrap());
        if preferences.dark_theme == "pitch" {
            break preferences;
        }
        assert!(Instant::now() < deadline, "the host never kept the set");
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(preferences.theme.to_string(), "dark", "the host kept the set");
    assert_eq!(preferences.dark_theme, "pitch");
    assert_eq!(cx.update(|_, cx| ui::theme::current(cx).id), "pitch", "the window took the theme it picked");

    // A reset is a null in the patch, which has to reach the host as a null: here a project's model put back.
    let folder = host.repo("gamma");
    host.wsp(&["add", &folder.to_string_lossy(), "--name", "gamma"]);
    until(cx, &screen, "the project", |s| s.data().projects.iter().any(|p| p.name == "gamma"));
    let id = screen.read_with(cx, |s, _| s.data().projects.iter().find(|p| p.name == "gamma").unwrap().id.clone());
    let pointer = format!("/projectDefaults/{id}/model");
    let stored_model = |host: &Throwaway| {
        let stored = other_client(host, async |client| client.call(req::<PreferencesGetRequest>(json!({}))).await.unwrap());
        serde_json::to_value(stored.preferences).unwrap().pointer(&pointer).cloned()
    };
    screen.update(cx, |s, cx| s.set_preferences(json!({ "projectDefaults": { id.clone(): { "model": "claude-opus-5-5" } } }), cx));
    let deadline = Instant::now() + Duration::from_secs(10);
    while stored_model(&host).is_none() {
        assert!(Instant::now() < deadline, "the host never kept the project's model");
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(100));
    }
    screen.update(cx, |s, cx| s.set_preferences(json!({ "projectDefaults": { id.clone(): { "model": null } } }), cx));
    let deadline = Instant::now() + Duration::from_secs(10);
    while stored_model(&host).is_some() {
        assert!(Instant::now() < deadline, "the host still holds the model the reset put back");
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[gpui_kit::test]
fn settings_follows_what_another_client_changes(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    // Appearance in a window with focus is the one place a draw does not lay the theme on again, so only the push can.
    let (screen, cx) = open(&host, At::Group(GroupId::Appearance), cx);
    cx.update(|window, _| window.activate_window());
    until(cx, &screen, "the first reads", |s| s.data().preferences.is_some() && s.data().places.iter().any(|p| p.id == "here"));

    other_client(&host, async |client| {
        client.call(req::<PreferencesSetRequest>(json!({ "patch": { "lightTheme": "linen", "theme": "light" } }))).await.unwrap();
    });
    until(cx, &screen, "the pushed preferences", |s| s.data().preferences.as_ref().is_some_and(|p| p.light_theme == "linen"));
    assert_eq!(cx.update(|_, cx| ui::theme::current(cx).id), "linen", "the window took the theme another client set");

    let folder = host.repo("beta");
    host.wsp(&["add", &folder.to_string_lossy(), "--name", "beta"]);
    until(cx, &screen, "the pushed project", |s| s.data().projects.iter().any(|p| p.name == "beta"));
}

#[gpui_kit::test]
fn the_recipes_are_read_again_as_their_page_opens(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let (screen, cx) = open(&host, At::Group(GroupId::Appearance), cx);
    until(cx, &screen, "the first reads", |s| s.data().recipes.is_some() && s.data().preferences.is_some());
    // A recipe written by hand beside the state file: the host pushes nothing for it.
    let dir = host.state().parent().unwrap().join("recipes");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("by-hand.toml"), "name = \"By hand\"\n").unwrap();
    let row = cx.debug_bounds("settings-recipes").expect("the Recipes row is drawn");
    cx.simulate_click(row.center(), gpui_kit::Modifiers::none());
    until(cx, &screen, "the recipe written by hand", |s| s.data().recipes.as_ref().is_some_and(|r| r.iter().any(|r| r.name == "By hand")));
}

#[gpui_kit::test]
fn a_tick_on_a_recipe_keeps_the_picks_it_does_not_touch(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let file = json!({ "name": "Web app", "agents": {}, "configs": { "git": {} } });
    other_client(&host, async |client| {
        client.call(req::<wsp_host_client::protocol::RecipesSaveRequest>(json!({ "name": "Web app", "file": file }))).await.unwrap()
    });
    let (screen, cx) = open(&host, At::Group(GroupId::Recipes), cx);
    until(cx, &screen, "the recipe", |s| s.data().recipes.as_ref().is_some_and(|r| !r.is_empty()));
    let row = cx.debug_bounds("grid-row").expect("the recipe's row is drawn");
    cx.simulate_click(row.center(), gpui_kit::Modifiers::none());
    let deadline = Instant::now() + Duration::from_secs(20);
    let tick = loop {
        cx.run_until_parked();
        if let Some(tick) = cx.debug_bounds("checkbox") {
            break tick;
        }
        assert!(Instant::now() < deadline, "the agents offered were never drawn");
        std::thread::sleep(Duration::from_millis(50));
    };
    cx.simulate_click(tick.center(), gpui_kit::Modifiers::none());
    let saved = host.state().parent().unwrap().join("recipes/web-app.toml");
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        let text = std::fs::read_to_string(&saved).unwrap();
        if text.contains("[agents.") {
            assert!(text.contains("[configs.git]"), "the tick kept Git ticked:\n{text}");
            break;
        }
        assert!(Instant::now() < deadline, "the tick never reached the recipe:\n{text}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Whether a daemon binary is staged where the host's asset table reads it (`node packages/wspx/scripts/daemon-binary.mjs`).
/// A checkout without one runs no daemon, and a test that needs one says so and stops.

#[gpui_kit::test]
fn a_projects_counts_follow_threads_and_workspaces_from_the_command_line(cx: &mut TestAppContext) {
    use desktop_gpui::screens::settings::pages::projects::standing::Count;
    let host = carrying(common::STAND_IN);
    let folder = host.repo("alpha");
    host.wsp(&["add", &folder.to_string_lossy(), "--name", "alpha"]);
    let (screen, cx) = open(&host, At::Group(GroupId::Projects), cx);
    until(cx, &screen, "the project and its counts read", |s| {
        s.data().projects.iter().any(|p| p.name == "alpha") && s.data().standing.threads(&s.data().projects[0].id) == Count::Threads(0, 0)
    });
    let id = screen.read_with(cx, |s, _| s.data().projects[0].id.clone());
    screen.update_in(cx, |s, window, cx| s.go(At::Project(id.clone()), window, cx));

    let said = host.wsp(&["run", "alpha", "--agent", "claude", "hello"]);
    let thread = said.split_whitespace().skip_while(|w| *w != "thread").nth(1).expect("run names its thread").to_owned();
    until(cx, &screen, "the pushed thread, holding the remove", |s| {
        s.data().standing.threads(&id) == Count::Threads(1, 0) && s.data().standing.on(&id) == Some(1)
    });
    assert_eq!(screen.read_with(cx, |s, _| s.resolved()), At::Project(id.clone()), "the project's page stands");

    host.wsp(&["delete", &thread, "--yes"]);
    until(cx, &screen, "the deleted thread gone, and the remove no longer held", |s| {
        s.data().standing.threads(&id) == Count::Threads(0, 0) && s.data().standing.on(&id) == Some(0)
    });
}

#[gpui_kit::test]
fn a_previewed_theme_goes_with_the_windows_focus_and_with_settings(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let bare = json!({ "theme": "system", "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {}, "access": {},
        "projectLook": {}, "projectOrder": [], "labs": false });
    let data = desktop_gpui::screens::settings::model::Data { preferences: serde_json::from_value(bare).ok(), ..Default::default() };
    let mut screen = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| SettingsScreen::with_data(data, At::Group(GroupId::Appearance), runtime(), window, cx));
        screen = Some(view.clone());
        Root::new(view, window, cx)
    });
    let screen = screen.unwrap();
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let theme = |cx: &mut VisualTestContext| cx.update(|_, cx| ui::theme::current(cx).id);
    let linen = cx.debug_bounds("theme-option-linen").expect("the light themes are drawn");
    let off = linen.origin - gpui_kit::point(px(8.), px(8.));
    cx.simulate_mouse_move(linen.center(), None, gpui_kit::Modifiers::none());
    assert_eq!(theme(cx), "linen", "the pointer shows the window in the theme under it");
    cx.deactivate_window();
    assert_eq!(theme(cx), "paper", "the window losing focus puts the pick back");
    cx.update(|window, _| window.activate_window());
    cx.simulate_mouse_move(off, None, gpui_kit::Modifiers::none());
    cx.simulate_mouse_move(linen.center(), None, gpui_kit::Modifiers::none());
    assert_eq!(theme(cx), "linen");
    cx.update(|window, cx| screen.update(cx, |s, cx| s.escape(window, cx)));
    assert_eq!(theme(cx), "paper", "Escape closing Settings puts the pick back");
}

#[gpui_kit::test]
fn the_sidebar_moves_between_pages_and_the_field_searches(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let (screen, cx) = open(&host, At::Group(GroupId::Appearance), cx);
    until(cx, &screen, "the first reads", |s| s.data().preferences.is_some());

    // Nothing pressed yet: Settings holds the keys from the moment it opens.
    let closed = std::rc::Rc::new(std::cell::Cell::new(false));
    let heard = closed.clone();
    cx.update(|_, cx| cx.subscribe(&screen, move |_, _: &desktop_gpui::screens::settings::Closed, _| heard.set(true)).detach());
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(closed.get(), "Escape with nothing of Settings' own open asks whoever holds it to close it");

    let row = cx.debug_bounds("settings-keybindings").expect("the Keybindings row is drawn");
    cx.simulate_click(row.center(), gpui_kit::Modifiers::none());
    until(cx, &screen, "the Keybindings page", |s| *s.at() == At::Group(GroupId::Keybindings));

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(
        screen.read_with(cx, |s, _| s.at().clone()),
        At::Group(GroupId::Keybindings),
        "Escape with nothing open leaves the page to whoever holds Settings"
    );
}

/// Settings as the app opens it: through the shell's route, in the app's frame, on a throwaway host.
fn in_frame<'a>(host: &Throwaway, route: &str, cx: &'a mut TestAppContext) -> (Entity<Shell>, &'a mut VisualTestContext) {
    in_frame_at(host.state(), host.env(), route, cx)
}

/// `in_frame` dialling the host through `state`, which may be a door in front of it.
fn in_frame_at<'a>(state: PathBuf, env: Env, route: &str, cx: &'a mut TestAppContext) -> (Entity<Shell>, &'a mut VisualTestContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
    });
    let launch = Launch { screen: Some(route.to_owned()), ..Launch::default() };
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime(), launch, window, cx));
        shell = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    (shell.unwrap(), cx)
}

/// The Settings page the shell has mounted, once it has.
fn mounted(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<Entity<SettingsScreen>> {
    shell.read_with(cx, |s, _| s.screen_view()).and_then(|view| view.downcast::<SettingsScreen>().ok())
}

fn settle(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "Settings never came to: {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[gpui_kit::test]
fn settings_mounts_in_the_frame_with_its_parts_in_their_places(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let (shell, cx) = in_frame(&host, "settings:appearance", cx);
    settle(cx, "Settings mounted on Appearance with its reads", |cx| {
        mounted(&shell, cx)
            .is_some_and(|s| s.read_with(cx, |s, _| *s.at() == At::Group(GroupId::Appearance) && s.data().preferences.is_some()))
    });
    let column = cx.debug_bounds("sidebar-column").expect("the frame's sidebar column");
    let first = cx.debug_bounds("settings-general").expect("the page list stands in the frame's sidebar column");
    assert!(first.origin.x >= column.origin.x && first.right() <= column.right(), "{first:?} in {column:?}");
    assert!(first.origin.y >= px(52.), "under the frame's own row: {first:?}");
    let crumbs = cx.debug_bounds("settings-crumbs").expect("the crumbs are drawn");
    assert!(crumbs.origin.x >= column.right() && crumbs.bottom() <= px(52.), "the crumbs stand in the frame's header row: {crumbs:?}");
    assert!(cx.debug_bounds("settings-top-bar").is_none(), "no top bar of Settings' own under the frame's header");

    let row = cx.debug_bounds("settings-privacy").expect("the Privacy row");
    cx.simulate_click(row.center(), gpui_kit::Modifiers::none());
    settle(cx, "the Privacy page", |cx| {
        mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| *s.at() == At::Group(GroupId::Privacy)))
    });

    let route = shell.read_with(cx, |s, _| s.route().clone());
    assert_eq!(route, Route::Screen("settings:appearance".into()), "a page move inside Settings is Settings' own");
}

#[gpui_kit::test]
fn escape_in_the_frame_takes_a_dialog_then_the_search_then_settings(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let (shell, cx) = in_frame(&host, "settings:devices", cx);
    settle(cx, "Settings mounted with its reads", |cx| {
        mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| s.data().preferences.is_some()))
    });
    let screen = mounted(&shell, cx).unwrap();

    // A keyed state belongs to the view that asks for it, so a dialog built by two views holds two of each.
    let states = std::rc::Rc::new(std::cell::Cell::new(0));
    let made = states.clone();
    screen.update(cx, |s, cx| {
        s.open_dialog(
            desktop_gpui::screens::settings::page::DialogBuild::alert(move |_, window, cx| {
                let made = made.clone();
                window.use_keyed_state("dialog-probe", cx, move |_, _| made.set(made.get() + 1));
                gpui_kit::IntoElement::into_any_element(gpui_kit::div())
            }),
            cx,
        )
    });
    cx.run_until_parked();
    for _ in 0..3 {
        screen.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
    assert_eq!(states.get(), 1, "the dialog is built by the one view that draws it");
    assert_eq!(cx.update(|_, cx| layer::top(cx)), Some(DIALOG), "the dialog stands on the window's dialogs layer");
    let layer = cx.debug_bounds("dialogs-layer").expect("the dialog's backdrop is drawn");
    assert!(
        layer.origin == gpui_kit::point(px(0.), px(0.)) && layer.size == size(px(1280.), px(800.)),
        "the backdrop stands over the whole window, the frame's sidebar and header too: {layer:?}"
    );
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(mounted(&shell, cx).is_some(), "Escape with a dialog open leaves Settings open");
    assert!(!cx.update(|_, cx| layer::is_open(DIALOG, cx)), "and closes the dialog");

    let field = cx.debug_bounds("settings-search").expect("the search field");
    cx.simulate_click(field.center(), gpui_kit::Modifiers::none());
    cx.simulate_input("theme");
    settle(cx, "the search", |cx| screen.read_with(cx, |s, _| !s.search().is_empty()));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(mounted(&shell, cx).is_some(), "Escape while searching leaves Settings open");
    assert!(screen.read_with(cx, |s, _| s.search().is_empty()), "and clears the search");

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(mounted(&shell, cx).is_none(), "Escape with nothing of Settings' own open closes it");
    assert!(!matches!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen(_)));
}

#[gpui_kit::test]
fn usage_draws_inside_settings_with_its_tabs_in_the_header(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let (shell, cx) = in_frame(&host, "settings:usage", cx);
    settle(cx, "Settings on Usage with the page's refresh in the header", |cx| {
        mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| *s.at() == At::Group(GroupId::Usage)))
            && cx.debug_bounds("usage-refresh").is_some()
    });
    let refresh = cx.debug_bounds("usage-refresh").unwrap();
    let column = cx.debug_bounds("sidebar-column").unwrap();
    assert!(refresh.origin.x > column.right() && refresh.bottom() <= px(52.), "the page's tabs stand in the frame's header: {refresh:?}");
    let page = cx.debug_bounds("settings-page").expect("the page column");
    assert!(page.origin.y >= px(52.) && page.origin.x >= column.right(), "{page:?}");

    let row = cx.debug_bounds("settings-general").expect("the General row");
    cx.simulate_click(row.center(), gpui_kit::Modifiers::none());
    settle(cx, "General, the Usage page let go", |cx| {
        mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| *s.at() == At::Group(GroupId::General)))
            && cx.debug_bounds("usage-refresh").is_none()
    });
}

#[gpui_kit::test]
fn a_route_to_another_page_moves_the_open_settings(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let (shell, cx) = in_frame(&host, "settings:privacy", cx);
    settle(cx, "Settings on Privacy", |cx| {
        mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| *s.at() == At::Group(GroupId::Privacy)))
    });
    let first = mounted(&shell, cx).unwrap();
    cx.update(|_, cx| desktop_gpui::screens::navigate(Route::Screen("settings:keybindings".into()), cx));
    settle(cx, "the same Settings on Keybindings", |cx| {
        mounted(&shell, cx).is_some_and(|s| s == first && s.read_with(cx, |s, _| *s.at() == At::Group(GroupId::Keybindings)))
    });
}

/// Settings in the frame learns this computer's reach through the socket's one status watch, the one the sidebar reads
/// too: the app asks `status.subscribe` once on its socket.
#[gpui_kit::test]
fn settings_knows_this_computers_reach_from_the_frames_one_status_watch(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    // A project and one turn on it, so the host holds this computer's own workspace and its status.
    let folder = host.repo("alpha");
    host.wsp(&["add", &folder.to_string_lossy(), "--name", "alpha"]);
    host.wsp(&["run", "alpha", "--agent", "claude", "hello"]);
    let door = Door::open(&host);
    let (shell, cx) = in_frame_at(door.state.clone(), host.env(), "settings:computers", cx);
    // The sidebar asks for the forwards after its statuses, so both readers have asked by then.
    settle(cx, "this computer's reach in Settings, and the sidebar's read", |cx| {
        door.sent("forwards.list") >= 1 && mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| s.data().here_status.is_some()))
    });
    assert_eq!(door.sent("status.subscribe"), 1, "Settings and the sidebar read the socket's one status watch");
}

/// Settings on a client of its own, with no sidebar on the socket, asks that socket's status watch as it opens, so it
/// hears this computer's status all the same.
#[gpui_kit::test]
fn settings_alone_asks_for_this_computers_reach_as_it_opens(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let folder = host.repo("alpha");
    host.wsp(&["add", &folder.to_string_lossy(), "--name", "alpha"]);
    host.wsp(&["run", "alpha", "--agent", "claude", "hello"]);
    let (screen, cx) = open(&host, At::Group(GroupId::Computers), cx);
    until(cx, &screen, "this computer's status, off the socket's watch", |s| s.data().here_status.is_some());
}

#[gpui_kit::test]
fn the_switchers_cogs_open_a_project_and_a_computer_in_settings(cx: &mut TestAppContext) {
    use desktop_gpui::screens::sidebar::Asked;
    let host = Throwaway::start();
    let folder = host.repo("alpha");
    host.wsp(&["add", &folder.to_string_lossy(), "--name", "alpha"]);
    let (shell, cx) = in_frame(&host, "settings:general", cx);
    settle(cx, "Settings with its projects read", |cx| {
        mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| s.data().projects.iter().any(|p| p.name == "alpha")))
    });
    let id = mounted(&shell, cx).unwrap().read_with(cx, |s, _| s.data().projects[0].id.clone());
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    sidebar.update(cx, |_, cx| cx.emit(Asked::ProjectSettings(id.clone())));
    settle(cx, "the project's page", |cx| mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| *s.at() == At::Project(id.clone()))));
    sidebar.update(cx, |_, cx| cx.emit(Asked::ComputerSettings("here".into())));
    settle(cx, "this computer's page", |cx| {
        mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| *s.at() == At::Computer("here".into())))
    });
}

#[gpui_kit::test]
fn back_closes_settings_in_the_frame(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let (shell, cx) = in_frame(&host, "settings:privacy", cx);
    settle(cx, "Settings mounted", |cx| mounted(&shell, cx).is_some());
    let back = cx.debug_bounds("settings-back").expect("the Back row");
    cx.simulate_click(back.center(), gpui_kit::Modifiers::none());
    settle(cx, "Settings closed", |cx| mounted(&shell, cx).is_none());
}

#[gpui_kit::test]
fn escape_closes_an_open_select_before_anything_of_settings(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let bare = json!({ "theme": "system", "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {}, "access": {},
        "projectLook": {}, "projectOrder": [], "labs": false });
    let data = Data { preferences: serde_json::from_value(bare).ok(), ..Data::default() };
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| SettingsScreen::with_data(data, At::Group(GroupId::General), runtime(), window, cx));
        Root::new(view, window, cx)
    });
    // Tall enough that the whole page is in view, whichever of its selects the selector names last.
    cx.simulate_resize(size(px(1280.), px(2000.)));
    let trigger = cx.debug_bounds("select").expect("General draws a select");
    cx.simulate_click(trigger.center(), gpui_kit::Modifiers::none());
    assert!(cx.debug_bounds("select-list").is_some(), "the press opens the list");
    cx.simulate_keystrokes("escape");
    assert!(cx.debug_bounds("select-list").is_none(), "Escape closes the list, as the select does on its own");
}

#[gpui_kit::test]
fn the_agents_page_keeps_its_tab_and_its_report_while_settings_moves(cx: &mut TestAppContext) {
    use desktop_gpui::screens::settings::pages::agents::Tab;
    let host = Throwaway::start();
    let (screen, cx) = open(&host, At::Group(GroupId::Agents), cx);
    // The store as the screen holds it at each look, so a store made afresh is the one asked.
    let agents = |cx: &mut VisualTestContext| screen.read_with(cx, |s, _| s.agents().clone());
    let read_at = |cx: &mut VisualTestContext| agents(cx).read_with(cx, |a, _| a.reads.get("here").and_then(|r| r.read_at));
    until(cx, &screen, "this computer's agents read", |s| s.data().places_read);
    let deadline = Instant::now() + Duration::from_secs(60);
    while read_at(cx).is_none() {
        assert!(Instant::now() < deadline, "the agents report never came");
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(100));
    }
    let first = read_at(cx);
    agents(cx).update(cx, |a, cx| {
        a.tab = Tab::Skills;
        cx.notify();
    });
    cx.run_until_parked();

    for (row, group) in [("settings-computers", GroupId::Computers), ("settings-agents", GroupId::Agents)] {
        let at = cx.debug_bounds(row).unwrap_or_else(|| panic!("the {row} row is drawn"));
        cx.simulate_click(at.center(), gpui_kit::Modifiers::none());
        until(cx, &screen, row, |s| *s.at() == At::Group(group));
    }
    cx.run_until_parked();

    assert_eq!(agents(cx).read_with(cx, |a, _| a.tab), Tab::Skills, "coming back to Agents finds the tab picked before");
    assert_eq!(read_at(cx), first, "coming back within five minutes reads no report again");
    assert!(cx.debug_bounds("kind-toolbar").is_some(), "the Skills tab is what the page draws");
}

#[gpui_kit::test]
fn the_toggle_closing_settings_puts_back_a_previewed_theme(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let (shell, cx) = in_frame(&host, "settings:appearance", cx);
    // Settings lays the pick on again while the window has no focus, which would hide what the shell puts back.
    cx.update(|window, _| window.activate_window());
    settle(cx, "Appearance with its reads", |cx| {
        mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| s.data().preferences.is_some()))
            && cx.debug_bounds("theme-option-linen").is_some()
    });
    let theme = |cx: &mut VisualTestContext| cx.update(|_, cx| ui::theme::current(cx).id);
    assert_eq!(theme(cx), "paper");
    let linen = cx.debug_bounds("theme-option-linen").unwrap();
    cx.simulate_mouse_move(linen.center(), None, gpui_kit::Modifiers::none());
    assert_eq!(theme(cx), "linen", "the pointer shows the window in the theme under it");
    // The action and not its chord: a modifier's release moves the pointer, and the card it leaves puts the pick back.
    cx.dispatch_action(desktop_gpui::screens::app_shell::keys::ToggleSettings);
    cx.run_until_parked();
    assert!(mounted(&shell, cx).is_none(), "the toggle closes Settings");
    assert_eq!(theme(cx), "paper", "closing Settings puts the pick back");
}

#[gpui_kit::test]
fn a_light_theme_picked_for_the_dark_side_draws_the_dark_default_everywhere(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    other_client(&host, async |client| {
        client.call(req::<PreferencesSetRequest>(json!({ "patch": { "theme": "dark", "darkTheme": "paper" } }))).await.unwrap();
    });
    let (shell, cx) = in_frame(&host, "settings:general", cx);
    cx.update(|window, _| window.activate_window());
    settle(cx, "Settings with its reads", |cx| {
        mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| s.data().preferences.as_ref().is_some_and(|p| p.dark_theme == "paper")))
    });
    let theme = |cx: &mut VisualTestContext| cx.update(|_, cx| ui::theme::current(cx).id);
    assert_eq!(theme(cx), "graphite", "Settings draws the dark side's default");
    cx.dispatch_action(desktop_gpui::screens::app_shell::keys::ToggleSettings);
    cx.run_until_parked();
    assert!(mounted(&shell, cx).is_none(), "the toggle closes Settings");
    assert_eq!(theme(cx), "graphite", "and the frame draws the same theme Settings did");
}

#[gpui_kit::test]
fn a_page_opened_scrolled_lights_the_sections_in_view(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    // As a capture of a page's second window opens it.
    let (shell, cx) = in_frame(&host, "settings:general@600", cx);
    // A test window has no frame loop of its own, and the sidebar part hears what the page's frame lit on the next.
    let lit = |cx: &mut VisualTestContext, ix: usize| {
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
        cx.debug_bounds(Box::leak(format!("settings-section-lit-{ix}").into_boxed_str())).is_some()
    };
    let page = |cx: &mut VisualTestContext| cx.debug_bounds("settings-page").unwrap();
    settle(cx, "General opened 600 px down", |cx| {
        mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| s.data().preferences.is_some()))
            && cx.debug_bounds("settings-section-0").is_some()
            && (1..12).any(|ix| lit(cx, ix))
            && !lit(cx, 0)
    });
    let row = cx.debug_bounds("settings-section-0").unwrap();
    assert!(row.origin.x < page(cx).origin.x, "the trail stands in the sidebar");
    let bar = cx.debug_bounds("scrollbar").expect("the long page draws the shared bar");
    assert_eq!((page(cx).right() - bar.right(), bar.size.width), (px(1.), px(6.)), "w-1.5 mr-px at the page's right edge");
    let shown: Vec<usize> = (0..12).filter(|ix| lit(cx, *ix)).collect();
    assert!(!shown.contains(&0) && !shown.contains(&1), "the top sections are scrolled out of view: {shown:?}");

    cx.simulate_click(row.center(), gpui_kit::Modifiers::none());
    settle(cx, "the press on the first section scrolls back to it", |cx| lit(cx, 0));
}

#[gpui_kit::test]
fn the_trail_lights_the_cards_on_screen_as_the_page_scrolls(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let (shell, cx) = in_frame(&host, "settings:general", cx);
    let lit = |cx: &mut VisualTestContext, ix: usize| {
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
        cx.debug_bounds(Box::leak(format!("settings-section-lit-{ix}").into_boxed_str())).is_some()
    };
    settle(cx, "General opened at its top", |cx| {
        mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| s.data().preferences.is_some() && s.data().release.is_some()))
            && cx.debug_bounds("settings-section-0").is_some()
            && lit(cx, 0)
    });
    let heads = (0..20).take_while(|ix| cx.debug_bounds(Box::leak(format!("settings-section-{ix}").into_boxed_str())).is_some()).count();
    assert!(heads > 4, "General's trail lists its cards: {heads}");
    let page = cx.debug_bounds("settings-page").unwrap();
    let last = cx.debug_bounds(Box::leak(format!("settings-section-{}", heads - 1).into_boxed_str())).unwrap();
    // The trail's open motion, and each glide after a press, run to its end.
    let ahead = |cx: &mut VisualTestContext| {
        cx.executor().advance_clock(Duration::from_millis(500));
        cx.update(|window, cx| window.simulate_next_frame(cx));
    };
    ahead(cx);
    cx.simulate_click(last.center(), gpui_kit::Modifiers::none());
    settle(cx, "a press on the last head scrolls its card into view", |cx| {
        ahead(cx);
        lit(cx, heads - 1) && !lit(cx, 0)
    });
    let first = cx.debug_bounds("settings-section-0").unwrap();
    cx.simulate_click(first.center(), gpui_kit::Modifiers::none());
    settle(cx, "and the first head scrolls back up", |cx| {
        ahead(cx);
        lit(cx, 0)
    });
    // A person's wheel, as far down as the page goes.
    for _ in 0..10 {
        cx.simulate_event(gpui_kit::ScrollWheelEvent {
            position: page.center(),
            delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-400.))),
            ..Default::default()
        });
    }
    settle(cx, "the foot's cards lit", |cx| lit(cx, heads - 1));
    let shown: Vec<usize> = (0..heads).filter(|ix| lit(cx, *ix)).collect();
    assert!(!shown.contains(&0), "the first card is scrolled off: {shown:?}");
    assert!(shown.contains(&(heads - 1)) && shown.contains(&(heads - 2)), "the cards at the foot are on screen: {shown:?}");
}

#[gpui_kit::test]
fn the_agents_page_hears_the_host_again_after_it_restarts(cx: &mut TestAppContext) {
    use desktop_gpui::screens::settings::model::req;
    use wsp_host_client::protocol::ServersRemoveRequest;
    let mut host = Throwaway::start();
    let (screen, cx) = open(&host, At::Group(GroupId::Agents), cx);
    let agents = |cx: &mut VisualTestContext| screen.read_with(cx, |s, _| s.agents().clone());
    let read_at =
        |cx: &mut VisualTestContext| agents(cx).read_with(cx, |a, _| a.reads.get("here").filter(|r| !r.reading).and_then(|r| r.read_at));
    let read_after = |cx: &mut VisualTestContext, after: Option<std::time::SystemTime>, what: &str| {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            cx.run_until_parked();
            let at = read_at(cx);
            if at.is_some() && at > after {
                return at;
            }
            assert!(Instant::now() < deadline, "{what}");
            std::thread::sleep(Duration::from_millis(100));
        }
    };
    let first = read_after(cx, None, "the agents report never came");

    host.restart();
    let back = read_after(cx, first, "the store never heard the host again after it restarted");

    // A write there tells every listener the agents changed, whether or not it held.
    other_client(&host, async |client| {
        let ask = json!({ "target": { "placeId": "here" }, "agent": "claude", "name": "nothing-here", "scope": "user" });
        let _ = client.call(req::<ServersRemoveRequest>(ask)).await;
    });
    read_after(cx, back, "agents.changed from the restarted host never reached the store");
}

#[gpui_kit::test]
fn the_mode_tiles_and_theme_cards_are_radio_groups_the_arrows_pick_in(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let (screen, cx) = open(&host, At::Group(GroupId::Appearance), cx);
    until(cx, &screen, "the first reads", |s| s.data().preferences.is_some());
    let picked = |s: &SettingsScreen| s.data().preferences.as_ref().map(|p| (p.theme.to_string(), p.light_theme.clone()));
    let click = |cx: &mut VisualTestContext, selector: &'static str| {
        let at = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn")).center();
        cx.simulate_click(at, gpui_kit::Modifiers::none());
    };
    click(cx, "theme-mode-light");
    until(cx, &screen, "Light, pressed", |s| picked(s).is_some_and(|(mode, _)| mode == "light"));
    cx.simulate_keystrokes("right");
    until(cx, &screen, "Dark, the arrow's pick", |s| picked(s).is_some_and(|(mode, _)| mode == "dark"));
    cx.simulate_keystrokes("right");
    until(cx, &screen, "System, round the end", |s| picked(s).is_some_and(|(mode, _)| mode == "system"));

    click(cx, "theme-option-linen");
    until(cx, &screen, "Linen, pressed", |s| picked(s).is_some_and(|(_, light)| light == "linen"));
    cx.simulate_keystrokes("down");
    until(cx, &screen, "Frost, the next card", |s| picked(s).is_some_and(|(_, light)| light == "frost"));
    cx.simulate_keystrokes("left left");
    until(cx, &screen, "Paper, two back", |s| picked(s).is_some_and(|(_, light)| light == "paper"));
}

#[gpui_kit::test]
fn settings_opened_with_no_page_named_opens_where_it_last_was(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let (shell, cx) = in_frame(&host, "settings:privacy", cx);
    settle(cx, "Settings on Privacy", |cx| {
        mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| *s.at() == At::Group(GroupId::Privacy)))
    });
    let row = cx.debug_bounds("settings-keybindings").expect("the Keybindings row");
    cx.simulate_click(row.center(), gpui_kit::Modifiers::none());
    settle(cx, "Keybindings", |cx| mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| *s.at() == At::Group(GroupId::Keybindings))));
    let back = cx.debug_bounds("settings-back").expect("the Back row");
    cx.simulate_click(back.center(), gpui_kit::Modifiers::none());
    settle(cx, "Settings closed", |cx| mounted(&shell, cx).is_none());
    cx.update(|_, cx| desktop_gpui::screens::navigate(Route::Screen("settings".into()), cx));
    settle(cx, "Settings again, on Keybindings", |cx| {
        mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| *s.at() == At::Group(GroupId::Keybindings)))
    });
}

#[gpui_kit::test]
fn a_right_press_on_a_computer_row_picks_the_icon_it_shows_as(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let (screen, cx) = open(&host, At::Group(GroupId::Computers), cx);
    until(cx, &screen, "the computer here and the preferences", |s| {
        s.data().preferences.is_some() && s.data().places.iter().any(|p| p.id == "here")
    });
    let row = cx.debug_bounds("grid-row").expect("the computer here's row").center();
    assert!(cx.debug_bounds("computer-icon-menu").is_none());
    cx.simulate_mouse_down(row, gpui_kit::MouseButton::Right, gpui_kit::Modifiers::none());
    cx.simulate_mouse_up(row, gpui_kit::MouseButton::Right, gpui_kit::Modifiers::none());
    cx.run_until_parked();
    let (asked, offered) = screen.read_with(cx, |s, cx| {
        let here = s.data().places.iter().find(|p| p.id == "here").unwrap();
        let offered: Vec<SharedString> = desktop_gpui::screens::settings::pages::computers::icon_items(here, s.data().preferences.as_ref())
            .into_iter()
            .map(|(_, label)| label.into())
            .collect();
        (s.computers_menu_asked(cx), offered)
    });
    assert_eq!(asked, offered, "a right press asks for every icon but the one the row wears");
    // On the Mac AppKit draws the menu, which the test platform cannot drive.
    if cfg!(target_os = "macos") {
        return;
    }
    assert!(cx.debug_bounds("computer-icon-menu").is_some(), "the row's menu of icons is open");
    // The computer here wears the desktop, so the menu's first row is the laptop.
    cx.simulate_keystrokes("down enter");
    cx.run_until_parked();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let PreferencesReply { preferences } =
            other_client(&host, async |client| client.call(req::<PreferencesGetRequest>(json!({}))).await.unwrap());
        let look = serde_json::to_value(preferences).unwrap().pointer("/computerLook/here/icon").cloned();
        if look == Some(json!("laptop")) {
            break;
        }
        assert!(Instant::now() < deadline, "the host never kept the icon: {look:?}");
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(cx.debug_bounds("computer-icon-menu").is_none(), "the pick closed the menu");
}

/// The words of the hover standing over `at` once the pointer has rested there, if the one asked for.
fn hover_says(cx: &mut VisualTestContext, at: gpui_kit::Point<gpui_kit::Pixels>, words: &str) -> bool {
    cx.simulate_mouse_move(at, None, gpui_kit::Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(800));
    cx.run_until_parked();
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    cx.debug_bounds(Box::leak(format!("tip:{words}").into_boxed_str())).is_some()
}

fn bounds(cx: &mut VisualTestContext, selector: String) -> Option<gpui_kit::Bounds<gpui_kit::Pixels>> {
    cx.debug_bounds(Box::leak(selector.into_boxed_str()))
}

#[gpui_kit::test]
fn the_models_grip_moves_a_model_by_its_keys_and_by_a_drag(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let (shell, cx) = in_frame(&host, "settings:agent:claude", cx);
    cx.update(|window, _| window.activate_window());
    cx.simulate_resize(size(px(1280.), px(4000.)));
    let models = |cx: &mut VisualTestContext| {
        mounted(&shell, cx)
            .and_then(|s| s.read_with(cx, |s, _| s.data().harnesses.iter().find(|h| h.harness == "claude").cloned()))
            .map(|c| c.models.iter().map(|m| (m.value.clone(), m.label.clone())).collect::<Vec<_>>())
            .unwrap_or_default()
    };
    settle(cx, "the Models card with three models", |cx| {
        let all = models(cx);
        all.len() >= 3 && bounds(cx, format!("model-grip-{}", all[0].0)).is_some()
    });
    let all = models(cx);
    let values: Vec<String> = all.iter().map(|(v, _)| v.clone()).collect();
    let (a, b, c) = (values[0].clone(), values[1].clone(), values[2].clone());
    let order = |cx: &mut VisualTestContext| {
        let _ = cx;
        other_client(&host, async |client| {
            let PreferencesReply { preferences } = client.call(req::<PreferencesGetRequest>(json!({}))).await.unwrap();
            preferences.agent_defaults.get("claude").and_then(|d| d.models.as_ref()).map(|m| m.order.clone()).unwrap_or_default()
        })
    };
    let lands = |cx: &mut VisualTestContext, want: Vec<String>, what: &str| {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            cx.run_until_parked();
            let have = order(cx);
            if have.iter().take(3).eq(want.iter()) {
                return;
            }
            assert!(Instant::now() < deadline, "{what}: the order is {have:?}");
            std::thread::sleep(Duration::from_millis(100));
        }
    };

    let grip = bounds(cx, format!("model-grip-{a}")).unwrap();
    cx.simulate_click(grip.center(), gpui_kit::Modifiers::none());
    cx.simulate_keystrokes("down");
    lands(cx, vec![b.clone(), a.clone(), c.clone()], "ArrowDown on the first grip moves it one down");
    settle(cx, "the moved row drawn second", |cx| {
        bounds(cx, format!("model-row-{a}")).zip(bounds(cx, format!("model-row-{b}"))).is_some_and(|(a, b)| a.origin.y > b.origin.y)
    });
    cx.simulate_keystrokes("up");
    lands(cx, vec![a.clone(), b.clone(), c.clone()], "the grip keeps its focus as its row moves, and ArrowUp moves it back");

    let (from, onto) = (bounds(cx, format!("model-row-{c}")).unwrap().center(), bounds(cx, format!("model-row-{a}")).unwrap().center());
    cx.simulate_mouse_down(from, gpui_kit::MouseButton::Left, gpui_kit::Modifiers::none());
    cx.simulate_mouse_move(from + gpui_kit::point(px(0.), px(-10.)), gpui_kit::MouseButton::Left, gpui_kit::Modifiers::none());
    cx.simulate_mouse_move(onto, gpui_kit::MouseButton::Left, gpui_kit::Modifiers::none());
    assert!(cx.debug_bounds("model-ghost").is_some(), "the row is drawn under the pointer as it is dragged");
    cx.simulate_mouse_up(onto, gpui_kit::MouseButton::Left, gpui_kit::Modifiers::none());
    lands(cx, vec![c.clone(), a.clone(), b.clone()], "the third row dropped on the first stands before it");

    let field = cx.debug_bounds("agent-models-add-row").unwrap();
    cx.simulate_click(field.center() - gpui_kit::point(px(60.), px(0.)), gpui_kit::Modifiers::none());
    cx.simulate_input("a b");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let refusal = cx.debug_bounds("agent-models-refusal").expect("Enter on an id with a space in it is refused");
    let add_row = cx.debug_bounds("agent-models-add-row").unwrap();
    assert_eq!(refusal.origin.y, add_row.bottom() - px(12.), "the refusal stands inside the card, 12 px up into the add row");

    // Last: under the test clock a grip whose hover has shown misses later presses, which the app under Xvfb takes.
    let grip = bounds(cx, format!("model-grip-{c}")).unwrap();
    assert!(hover_says(cx, grip.center(), &format!("Drag to reorder {}, or use the arrow keys", all[2].1)), "the grip says what it does");
}

#[gpui_kit::test]
fn back_from_a_found_skill_finds_the_search_as_it_was_left(cx: &mut TestAppContext) {
    use desktop_gpui::screens::settings::page::Level;
    use desktop_gpui::screens::settings::pages::agents::Tab;
    let host = Throwaway::start();
    let (shell, cx) = in_frame(&host, "settings:agents", cx);
    cx.update(|window, _| window.activate_window());
    let agents = |cx: &mut VisualTestContext| mounted(&shell, cx).map(|s| s.read_with(cx, |s, _| s.agents().clone()));
    settle(cx, "this computer's agents read", |cx| {
        agents(cx).is_some_and(|a| a.read_with(cx, |a, _| a.reads.get("here").is_some_and(|r| r.report.is_some())))
    });
    let store = agents(cx).unwrap();
    let hit = serde_json::from_value(
        json!({ "id": "acme/skills/pdf", "source": "acme/skills", "skillId": "pdf", "name": "pdf", "installs": 1200 }),
    );
    store.update(cx, |a, cx| {
        a.tab = Tab::Skills;
        a.search_asked("pdf".into());
        a.search_answered("pdf", Ok(vec![hit.unwrap()]));
        a.acts().open_level(Some(Level { key: "add".into(), name: "Add a skill".into(), up: None }), cx);
    });
    settle(cx, "the add page", |cx| cx.debug_bounds("add-search").is_some());
    let field = cx.debug_bounds("add-search").unwrap().center();
    cx.simulate_click(field, gpui_kit::Modifiers::none());
    cx.simulate_input("pdf");
    let found = |cx: &mut VisualTestContext| {
        cx.executor().advance_clock(Duration::from_millis(300));
        cx.run_until_parked();
        cx.debug_bounds("found-acme/skills/pdf")
    };
    settle(cx, "the search's row", |cx| found(cx).is_some());
    let row = found(cx).unwrap().center();
    cx.simulate_click(row, gpui_kit::Modifiers::none());
    settle(cx, "the found skill's page", |cx| found(cx).is_none() && cx.debug_bounds("settings-up").is_some());
    let up = cx.debug_bounds("settings-up").unwrap();
    assert!(hover_says(cx, up.center(), "Back to Add a skill"), "the chevron says where it goes");
    cx.simulate_click(up.center(), gpui_kit::Modifiers::none());
    settle(cx, "back on the add page", |cx| cx.debug_bounds("add-search").is_some());
    assert!(found(cx).is_some(), "the add page holds the search it was left with, and its row");
    assert_eq!(store.read_with(cx, |a, _| a.add_asked.clone()), "pdf", "nothing asked for an empty search on the way back");
}

#[gpui_kit::test]
fn the_agents_report_is_held_across_settings_closing_and_opening(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let (shell, cx) = in_frame(&host, "settings:agents", cx);
    cx.update(|window, _| window.activate_window());
    let read_at = |cx: &mut VisualTestContext| {
        mounted(&shell, cx)
            .map(|s| s.read_with(cx, |s, _| s.agents().clone()))
            .and_then(|a| a.read_with(cx, |a, _| a.reads.get("here").filter(|r| !r.reading).and_then(|r| r.read_at)))
    };
    settle(cx, "the agents report", |cx| read_at(cx).is_some());
    let first = read_at(cx);
    for _ in 0..2 {
        cx.dispatch_action(desktop_gpui::screens::app_shell::keys::ToggleSettings);
        cx.run_until_parked();
    }
    settle(cx, "Settings open again", |cx| mounted(&shell, cx).is_some());
    let reopened = mounted(&shell, cx).unwrap();
    if reopened.read_with(cx, |s, _| *s.at() != At::Group(GroupId::Agents)) {
        let row = cx.debug_bounds("settings-agents").expect("the Agents row is drawn");
        cx.simulate_click(row.center(), gpui_kit::Modifiers::none());
    }
    settle(cx, "the Agents page again", |cx| cx.debug_bounds("agents-controls").is_some());
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    assert_eq!(read_at(cx), first, "reopened inside five minutes, the page asks for no report");
}

/// Settings opened again inside a minute of its reads draws the ones `settingsReads.ts` holds and asks the host for none
/// of them, though Settings itself went with its closing.
#[gpui_kit::test]
fn settings_opened_again_inside_a_minute_asks_the_host_for_none_of_its_held_reads(cx: &mut TestAppContext) {
    use wsp_host_client::protocol::SshIncludeRequest;
    let host = Throwaway::start();
    let (shell, cx) = in_frame(&host, "settings:general", cx);
    let ssh = |cx: &mut VisualTestContext| mounted(&shell, cx).and_then(|s| s.read_with(cx, |s, _| s.data().ssh_include));
    settle(cx, "the ssh config's word", |cx| ssh(cx).is_some());
    assert_eq!(ssh(cx), Some(false), "a fresh home's ssh config does not read wsp's");
    cx.dispatch_action(desktop_gpui::screens::app_shell::keys::ToggleSettings);
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    assert!(mounted(&shell, cx).is_none(), "the toggle closes Settings");
    // Nothing tells a closed Settings, so only a read made as it opens again would see this.
    other_client(&host, async |client| {
        client.call(req::<SshIncludeRequest>(json!({ "on": true }))).await.unwrap_or_else(|f| panic!("{}", f.message));
    });
    cx.dispatch_action(desktop_gpui::screens::app_shell::keys::ToggleSettings);
    settle(cx, "Settings open again with its reads", |cx| {
        mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| s.data().preferences.is_some() && s.data().host_version.is_some()))
    });
    let version = mounted(&shell, cx).and_then(|s| s.read_with(cx, |s, _| s.data().host_version.clone()));
    assert_eq!(version.as_deref(), Some(host_release().as_str()), "the release the host named as it let the socket in");
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    assert_eq!(ssh(cx), Some(false), "inside the hold the ssh config's word is the one read a moment ago, asked of nobody");
}

/// An agent's Reset sends a null for the field it puts back, which the host takes as the agent's own again; a null
/// dropped on the way leaves the host holding the program.
#[gpui_kit::test]
fn an_agents_reset_reaches_the_host_as_a_null(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    host.wsp(&["agents", "setup", "claude", "--program", "/bin/true", "--config", &host.dir.path().join("claude").to_string_lossy()]);
    let (screen, cx) = open(&host, At::Agent("claude".into()), cx);
    let agents = screen.read_with(cx, |s, _| s.agents().clone());
    let setup = |cx: &mut VisualTestContext| {
        agents.read_with(cx, |a, _| {
            let read = a.reads.get("here").filter(|r| !r.reading)?;
            let row = read.report.as_ref()?.agents.iter().find(|a| a.id == "claude")?.setup.clone()?;
            Some((row.program, row.config_dir))
        })
    };
    settle(cx, "the program and the folder set from the command line", |cx| {
        setup(cx).is_some_and(|(program, config)| program.as_deref() == Some("/bin/true") && config.is_some())
    });
    let answered = std::rc::Rc::new(std::cell::RefCell::new(None));
    for change in [json!({ "program": null }), json!({ "configDir": null })] {
        let answer_to = answered.clone();
        agents.update(cx, |s, cx| s.setup("here", "claude", change, cx, move |_, answer, _| *answer_to.borrow_mut() = Some(answer)));
        settle(cx, "the host's answer to the Reset", |_| answered.borrow().is_some());
        assert_eq!(answered.borrow_mut().take(), Some(Ok(())), "the host takes the Reset");
    }
    settle(cx, "the agent's own program and folder back on the host", |cx| setup(cx) == Some((None, None)));
}

/// Settings listens again once the socket went, so a preference another window sets after a restart still reaches it.
#[gpui_kit::test]
fn settings_hears_the_host_again_after_it_restarts(cx: &mut TestAppContext) {
    let mut host = Throwaway::start();
    let (screen, cx) = open(&host, At::Group(GroupId::General), cx);
    until(cx, &screen, "the first reads", |s| s.data().preferences.is_some());
    let light = |theme: &'static str| move |s: &SettingsScreen| s.data().preferences.as_ref().is_some_and(|p| p.light_theme == theme);
    let set = |host: &Throwaway, theme: &str| {
        other_client(host, async |client| {
            client.call(req::<PreferencesSetRequest>(json!({ "patch": { "lightTheme": theme } }))).await.unwrap();
        })
    };

    host.restart();
    set(&host, "linen");
    until(cx, &screen, "Settings back on the restarted host", light("linen"));
    // Only a listener on the new socket hears this one: the reads that came with the dial are done.
    set(&host, "frost");
    until(cx, &screen, "a preference set after the restart, pushed", light("frost"));
}

/// The Activity page's view and computer outlive Settings closing, as the web's store keeps them for the window.
#[gpui_kit::test]
fn the_activity_pick_is_kept_when_settings_closes_and_opens_again(cx: &mut TestAppContext) {
    use desktop_gpui::screens::settings::pages::activity::model::View;
    let host = Throwaway::start();
    let (shell, cx) = in_frame(&host, "settings:activity", cx);
    let pick = |cx: &mut VisualTestContext| {
        mounted(&shell, cx).map(|s| {
            s.read_with(cx, |s, cx| {
                let pick = s.activity().read(cx);
                (pick.view, pick.place.clone())
            })
        })
    };
    settle(cx, "Settings on Activity", |cx| mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| s.data().preferences.is_some())));
    assert_eq!(pick(cx), Some((View::Threads, None)), "a fresh window starts on Threads");
    mounted(&shell, cx).unwrap().update(cx, |s, cx| {
        s.activity().update(cx, |p, cx| {
            (p.view, p.place) = (View::Ports, Some("here".into()));
            cx.notify();
        })
    });

    cx.dispatch_action(desktop_gpui::screens::app_shell::keys::ToggleSettings);
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    assert!(mounted(&shell, cx).is_none(), "the toggle closes Settings");
    cx.dispatch_action(desktop_gpui::screens::app_shell::keys::ToggleSettings);
    settle(cx, "Settings open again", |cx| mounted(&shell, cx).is_some());
    assert_eq!(pick(cx), Some((View::Ports, Some("here".into()))), "the pick the window left");
}

#[gpui_kit::test]
fn activitys_views_in_the_frames_header_move_the_page(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let (shell, cx) = in_frame(&host, "settings:activity", cx);
    settle(cx, "Activity's bar, and its views in the header", |cx| {
        mounted(&shell, cx).is_some_and(|s| s.read_with(cx, |s, _| *s.at() == At::Group(GroupId::Activity)))
            && cx.debug_bounds("activity-controls").is_some()
            && cx.debug_bounds("segmented").is_some()
    });
    let (column, views) = (cx.debug_bounds("sidebar-column").unwrap(), cx.debug_bounds("segmented").unwrap());
    assert!(views.origin.x > column.right() && views.bottom() <= px(52.), "the views stand in the frame's header: {views:?}");
    assert!(cx.debug_bounds("switch").is_none(), "Threads, the view it opens on, has no every-process switch");
    // The last segment drawn is Ports; from there the arrows walk back through Kinds to Processes.
    let ports = cx.debug_bounds("segment").unwrap();
    cx.simulate_click(ports.center(), gpui_kit::Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("switch").is_none(), "Ports has none either");
    cx.simulate_keystrokes("left left");
    settle(cx, "Processes, with its every-process switch in the bar", |cx| cx.debug_bounds("switch").is_some());
}

/// A claude whose turn starts and never ends, so its thread stands running with its process on this computer.
const STILL_TURN: &str = r#"#!/bin/sh
case " $* " in
  *" --version "*) echo "2.1.295 (Claude Code)"; exit 0 ;;
  *" --help "*) echo '--permission-mode <mode>  (choices: "default", "bypassPermissions")'; exit 0 ;;
  *" --bare "*) read -r _; echo '{"type":"control_response","response":{"subtype":"success","request_id":"init","response":{"models":[{"value":"default","resolvedModel":"claude-haiku-5-5"}]}}}'; exit 0 ;;
  *" --input-format stream-json "*) ;;
  *) exit 1 ;;
esac
session=""; prev=""
for a in "$@"; do [ "$prev" = "--session-id" ] && session=$a; prev=$a; done
read -r _
printf '{"type":"system","subtype":"init","session_id":"%s","model":"claude-haiku-5-5","cwd":"/","tools":[]}\n' "$session"
exec sleep 120
"#;

#[gpui_kit::test]
fn settings_opened_straight_on_activity_lists_a_running_thread(cx: &mut TestAppContext) {
    // The shared host interrupts every running turn as it stops, so the turn's processes end with the test.
    let host = carrying(STILL_TURN);
    let folder = host.repo("alpha");
    host.wsp(&["add", &folder.to_string_lossy(), "--name", "alpha"]);
    let said = host.wsp(&["run", "alpha", "--agent", "claude", "--detach", "hold still"]);
    assert!(said.split_whitespace().any(|w| w == "thread"), "run names its thread: {said}");
    let (screen, cx) = open(&host, At::Group(GroupId::Activity), cx);
    let deadline = Instant::now() + Duration::from_secs(60);
    // A thread's head is the one row of the Threads view that wears a status mark.
    while cx.debug_bounds("status-mark").is_none() {
        assert!(Instant::now() < deadline, "no thread row came: {said}\nhost log: {}", host.log());
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(screen.read_with(cx, |s, _| s.at().clone()), At::Group(GroupId::Activity));
}
