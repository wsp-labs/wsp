// SPDX-License-Identifier: AGPL-3.0-only
//! Settings against a throwaway host of its own: what it reads as it opens, what it writes, and what it takes from the
//! events the host pushes when another client changes something. The host runs on a fresh state file in a fresh
//! folder with nothing of this machine's environment but PATH; it needs node and the packages built (pnpm build).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::settings::model::{req, At, GroupId};
use desktop_gpui::screens::settings::{Opening, SettingsScreen};
use desktop_gpui::ui::{self, Side, UiAssets};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, Entity, TestAppContext, VisualTestContext};
use serde_json::json;
use wsp_host_client::ops::settings::PreferencesReply;
use wsp_host_client::protocol::{PreferencesGetRequest, PreferencesSetRequest};
use wsp_host_client::{Env, HostClient};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

/// A host on a state file in a folder of its own, stopped and removed with this.
struct Throwaway {
    dir: PathBuf,
    host: Child,
}

impl Throwaway {
    fn start(name: &str) -> Throwaway {
        let built = root().join("packages/host/dist/bin.js");
        assert!(built.exists(), "{} is not built: run pnpm install && pnpm build", built.display());
        let dir = std::env::temp_dir().join(format!("wsp-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("state")).unwrap();
        let log = std::fs::File::create(dir.join("up.log")).unwrap();
        let state = dir.join("state/state.json").to_string_lossy().into_owned();
        let host = Throwaway::node(&dir, &["up", "--port", "0", "--no-relay", "--state", &state])
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .unwrap();
        let throwaway = Throwaway { dir, host };
        let until = Instant::now() + Duration::from_secs(30);
        let listening = || {
            let lock = std::fs::read_to_string(throwaway.state().with_file_name("host.lock")).unwrap_or_default();
            serde_json::from_str::<serde_json::Value>(&lock).ok().and_then(|l| l["port"].as_u64()).is_some_and(|p| p != 0)
        };
        while !throwaway.state().with_file_name("host-token").exists() || !listening() {
            assert!(Instant::now() < until, "no host came up: {}", throwaway.log());
            std::thread::sleep(Duration::from_millis(100));
        }
        throwaway
    }

    fn node(dir: &Path, args: &[&str]) -> Command {
        let mut command = Command::new("node");
        command.arg(root().join("packages/host/dist/bin.js")).args(args).env_clear().envs(env_in(dir)).current_dir(dir);
        command
    }

    fn state(&self) -> PathBuf {
        self.dir.join("state/state.json")
    }

    fn env(&self) -> Env {
        env_in(&self.dir)
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.dir.join("up.log")).unwrap_or_default()
    }

    /// One wsp line against this host, which has to succeed.
    fn wsp(&self, args: &[&str]) -> String {
        let state = self.state().to_string_lossy().into_owned();
        let out = Throwaway::node(&self.dir, args).args(["--state", &state]).stdin(Stdio::null()).output().unwrap();
        let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        assert!(out.status.success(), "wsp {args:?}: {said}\nhost log: {}", self.log());
        said
    }

    /// A git repository with one empty commit, in a folder of that name under the host's own.
    fn repo(&self, name: &str) -> PathBuf {
        let folder = self.dir.join(name);
        std::fs::create_dir_all(&folder).unwrap();
        let git = |args: &[&str]| {
            assert!(Command::new("git").args(args).current_dir(&folder).env_clear().envs(self.env()).status().unwrap().success())
        };
        git(&["init", "-q"]);
        git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "init"]);
        folder
    }
}

impl Drop for Throwaway {
    fn drop(&mut self) {
        let pid = self.host.id().to_string();
        // A terminated host stops what it started on its way out; a killed one would leave it running.
        let _ = Command::new("kill").args(["-TERM", &pid]).env_clear().status();
        let until = Instant::now() + Duration::from_secs(20);
        while matches!(self.host.try_wait(), Ok(None)) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = self.host.kill();
        let _ = self.host.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// What every process of the test runs in: the folder as its home and PATH, and nothing that aims a host elsewhere.
fn env_in(dir: &Path) -> Env {
    let home = dir.to_string_lossy().into_owned();
    [("HOME", home), ("PATH", std::env::var("PATH").unwrap_or_default()), ("WSP_UPDATE_CHECK", "0".to_owned())]
        .map(|(k, v)| (k.to_owned(), v))
        .into_iter()
        .collect()
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
        let view = cx.new(|cx| SettingsScreen::new(opening, window, cx));
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
    let host = Throwaway::start("reads");
    let folder = host.repo("alpha");
    host.wsp(&["add", &folder.to_string_lossy(), "--name", "alpha"]);
    let (screen, cx) = open(&host, At::Group(GroupId::Projects), cx);

    until(cx, &screen, "the project, the computer here and the preferences read", |s| {
        let d = s.data();
        d.preferences.is_some() && d.projects.iter().any(|p| p.name == "alpha") && d.places.iter().any(|p| p.id == "here")
    });
    screen.read_with(cx, |s, _| {
        let d = s.data();
        assert!(d.account.is_some(), "the account was read");
        assert!(d.release.is_some(), "the release was read");
        assert!(d.projects_refused.is_none() && d.places_refused.is_none());
    });
}

#[gpui_kit::test]
fn settings_writes_preferences_through_the_host(cx: &mut TestAppContext) {
    let host = Throwaway::start("writes");
    let (screen, cx) = open(&host, At::Group(GroupId::Appearance), cx);
    until(cx, &screen, "the preferences read", |s| s.data().preferences.is_some());

    screen.update(cx, |s, cx| s.set_preferences(json!({ "theme": "dark", "darkTheme": "pitch" }), cx));
    until(cx, &screen, "the host's answer to the set", |s| s.data().preferences.as_ref().is_some_and(|p| p.dark_theme == "pitch"));

    let stored = other_client(&host, async |client| client.call(req::<PreferencesGetRequest>(json!({}))).await.unwrap());
    let PreferencesReply { preferences } = stored;
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
    let host = Throwaway::start("live");
    let (screen, cx) = open(&host, At::Group(GroupId::Projects), cx);
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
fn the_sidebar_moves_between_pages_and_the_field_searches(cx: &mut TestAppContext) {
    let host = Throwaway::start("moves");
    let (screen, cx) = open(&host, At::Group(GroupId::Appearance), cx);
    until(cx, &screen, "the first reads", |s| s.data().preferences.is_some());

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

/// The app's frame as it lays a framed Settings out: the page list in its sidebar column beside the page.
struct Framed {
    sidebar: Entity<desktop_gpui::screens::settings::SettingsPart>,
    header: Entity<desktop_gpui::screens::settings::SettingsPart>,
    screen: Entity<SettingsScreen>,
}

impl gpui_kit::Render for Framed {
    fn render(&mut self, _: &mut gpui_kit::Window, _: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        use gpui_kit::{div, prelude::*};
        div()
            .flex()
            .size_full()
            .child(div().w(px(256.)).h_full().child(self.sidebar.clone()))
            .child(div().flex().flex_col().flex_1().child(div().h(px(52.)).flex().child(self.header.clone())).child(self.screen.clone()))
    }
}

#[gpui_kit::test]
fn a_framed_settings_draws_its_parts_in_the_frame(cx: &mut TestAppContext) {
    use desktop_gpui::screens::settings::{Part, SettingsPart};
    let host = Throwaway::start("framed");
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let rt = runtime();
    let client = Arc::new(rt.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    let mut screen = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let s = cx.new(|cx| SettingsScreen::with_client(client, rt.clone(), At::Group(GroupId::Appearance), window, cx));
        let sidebar = cx.new(|cx| SettingsPart::new(&s, Part::Sidebar, cx));
        let header = cx.new(|cx| SettingsPart::new(&s, Part::Header, cx));
        screen = Some(s.clone());
        let framed = cx.new(|_| Framed { sidebar, header, screen: s });
        Root::new(framed, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    let screen = screen.unwrap();
    until(cx, &screen, "the first reads", |s| s.data().preferences.is_some());

    let first = cx.debug_bounds("settings-general").expect("the page list stands in the frame's sidebar column");
    assert_eq!(first.origin.y, px(48.), "the search field and then the list, with no frame row of Settings' own above them");
    let row = cx.debug_bounds("settings-privacy").expect("the Privacy row");
    assert!(cx.debug_bounds("settings-top-bar").is_some(), "the crumbs stand in the frame's header row");
    cx.simulate_click(row.center(), gpui_kit::Modifiers::none());
    until(cx, &screen, "the Privacy page", |s| *s.at() == At::Group(GroupId::Privacy));
}
