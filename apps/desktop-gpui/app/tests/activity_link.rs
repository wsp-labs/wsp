// SPDX-License-Identifier: AGPL-3.0-only
//! Settings > Activity against a throwaway host of its own (the shared one in `host-client/tests/common`) and the
//! daemon it starts: a link whose daemon is gone dials again, and the page fills once the daemon is back. Needs node,
//! the packages built (pnpm build) and the daemon the host stages.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::settings::model::{req, At, GroupId};
use desktop_gpui::screens::settings::{Opening, SettingsScreen};
use desktop_gpui::ui::{self, Side, UiAssets};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, TestAppContext, VisualTestContext};
use serde_json::{json, Value};
use wsp_host_client::ops::activity::AppProcs;
use wsp_host_client::ops::sidebar::Workspaces;
use wsp_host_client::protocol::{AppProcsWatchRequest, WorkspacesListRequest, WorkspacesRestartDaemonRequest};
use wsp_host_client::{Done, Event, HostClient};

use common::Throwaway;

/// A host carrying the wsp-daemon the shared helper finds, for a test that runs a turn on it.
fn carrying() -> Throwaway {
    let host = Throwaway::start_with(common::STAND_IN, true);
    if host.daemon.is_none() {
        panic!("{}: run apps/desktop-gpui/scripts/stage-daemon.sh", common::wsp_daemon().unwrap_err());
    }
    host
}

/// A project on this host and one turn in it, so it holds a local workspace to restart the daemon through.
fn project(host: &Throwaway) {
    let folder = host.repo("alpha");
    host.wsp(&["add", &folder.to_string_lossy(), "--name", "alpha"]);
    host.wsp(&["run", "alpha", "--agent", "claude", "hi"]);
}

/// What `ps` says, on the system's own PATH; macOS has no /proc to read.
fn ps(args: &[&str]) -> String {
    let out = Command::new("ps").args(args).env_clear().env("PATH", "/usr/bin:/bin").output();
    out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

/// The daemons this host started: the processes named as the daemon whose parent is the host the lock names.
fn daemons(host: &Throwaway) -> Vec<i32> {
    let lock = std::fs::read_to_string(host.state().with_file_name("host.lock")).unwrap_or_default();
    let Some(pid) = serde_json::from_str::<Value>(&lock).ok().and_then(|l| l["pid"].as_i64()) else { return Vec::new() };
    ps(&["-A", "-o", "pid=,ppid=,comm="])
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let (child, parent) = (words.next()?.parse::<i32>().ok()?, words.next()?.parse::<i64>().ok()?);
            // macOS names the whole path, Linux the bare name.
            let named = words.collect::<Vec<_>>().join(" ").rsplit('/').next().is_some_and(|name| name == "wsp-daemon");
            (named && parent == pid).then_some(child)
        })
        .collect()
}

/// Whether a process stands, a zombie not counted.
fn alive(pid: i32) -> bool {
    let state = ps(&["-o", "stat=", "-p", &pid.to_string()]);
    let state = state.trim();
    !state.is_empty() && !state.starts_with('Z')
}

/// Runs the window until `done` holds, or fails after `within` saying `what`.
fn until(cx: &mut VisualTestContext, within: Duration, what: &str, done: impl Fn(&mut VisualTestContext) -> bool) {
    let deadline = Instant::now() + within;
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "Activity never came to: {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// `workspaces.restartDaemon` on the host's local workspace, as the web's Restart daemon button asks.
fn restart_daemon(host: &Throwaway) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let other = HostClient::connect(&host.state(), &host.env()).await.unwrap_or_else(|f| panic!("{}", f.message));
        let Workspaces { workspaces } = other.call(req::<WorkspacesListRequest>(json!({}))).await.unwrap();
        let local =
            workspaces.iter().filter_map(|w| serde_json::to_value(w).ok()).find(|w| w["kind"] == "local").expect("a local workspace");
        let _: Done = other.call(req::<WorkspacesRestartDaemonRequest>(json!({ "workspaceId": local["id"] }))).await.unwrap();
    });
}

/// Whether a daemon binary is staged where the host's asset table reads it (`node packages/wspx/scripts/daemon-binary.mjs`).
/// A checkout without one runs no daemon, and a test that needs one says so and stops.

#[gpui_kit::test]
fn activity_dials_again_and_fills_once_the_daemon_is_back(cx: &mut TestAppContext) {
    let host = carrying();
    project(&host);
    restart_daemon(&host);
    let up = Instant::now() + Duration::from_secs(30);
    while daemons(&host).is_empty() {
        assert!(Instant::now() < up, "the host started no daemon: {}", host.log());
        std::thread::sleep(Duration::from_millis(100));
    }
    for pid in daemons(&host) {
        unsafe { libc::kill(pid, libc::SIGTERM) };
    }
    let gone = Instant::now() + Duration::from_secs(10);
    while !daemons(&host).is_empty() {
        assert!(Instant::now() < gone, "the daemon did not stop");
        std::thread::sleep(Duration::from_millis(100));
    }

    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let _ = UiAssets;
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let opening = Opening { state: host.state(), env: host.env(), runtime, at: At::Group(GroupId::Activity), scroll: 0. };
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| SettingsScreen::open(opening, window, cx));
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    until(cx, Duration::from_secs(20), "its skeleton", |cx| cx.debug_bounds("activity-loading").is_some());
    let still = Instant::now() + Duration::from_secs(3);
    while Instant::now() < still {
        cx.run_until_parked();
        assert!(cx.debug_bounds("activity-totals").is_none(), "totals drawn with no daemon");
        std::thread::sleep(Duration::from_millis(100));
    }

    restart_daemon(&host);
    assert!(!daemons(&host).is_empty(), "the restart started no daemon");
    until(cx, Duration::from_secs(30), "its totals once the daemon is back", |cx| cx.debug_bounds("activity-totals").is_some());

    let daemons = daemons(&host);
    assert!(!daemons.is_empty());
    drop(host);
    let gone = Instant::now() + Duration::from_secs(10);
    while daemons.iter().any(|pid| alive(*pid)) {
        assert!(Instant::now() < gone, "daemons left running after the host went: {daemons:?}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Whether a list the host sends a page names this process as the app's main one.
fn names_this_app(procs: &wsp_host_client::protocol::AppProcList) -> bool {
    let rows = serde_json::to_value(procs).unwrap();
    rows.as_array().unwrap().iter().any(|p| p["pid"] == json!(std::process::id()) && p["kind"] == "main")
}

#[test]
fn the_app_names_its_own_process_to_a_page_that_watches() {
    let host = Throwaway::start();
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let launch = host.env();
    rt.block_on(async {
        let app = HostClient::connect(&host.state(), &launch).await.unwrap_or_else(|f| panic!("{}", f.message));
        let offering = desktop_gpui::desktop::app_procs::offer(Arc::new(app), &rt);
        let page = HostClient::connect(&host.state(), &launch).await.unwrap_or_else(|f| panic!("{}", f.message));
        let mut events = page.events().await.unwrap();
        let AppProcs { procs } = page.call(req::<AppProcsWatchRequest>(json!({}))).await.unwrap();
        let named = names_this_app(&procs)
            || tokio::time::timeout(Duration::from_secs(10), async {
                while let Some(next) = events.next().await {
                    if let Ok(Event::AppProcs(e)) = next {
                        if names_this_app(&e.procs) {
                            return true;
                        }
                    }
                }
                false
            })
            .await
            .unwrap_or(false);
        offering.abort();
        assert!(named, "a page watching the app's processes never heard this app name itself: {}", host.log());
    });
}
