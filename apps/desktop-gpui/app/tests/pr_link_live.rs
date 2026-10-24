// SPDX-License-Identifier: AGPL-3.0-only
//! The daemon link's channels as the host holds them, counted by the host's sockets to the throwaway's wsp-daemon: one
//! per computer however many panes and terminals hold it, none once the last holder's release time passes, none left by
//! a dial dropped part-way, and a link that dials again by itself after the app's host socket drops, its folder and the
//! listings that failed coming back with it. The counts and the socket drop read Linux's `ss`. Without a daemon every
//! test says so and passes nothing.
#![cfg(target_os = "linux")]

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "pr_pane/stand_in.rs"]
mod stand_in;

use std::future::Future;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::screens::pr_pane::changes::{ChangesScreen, Load};
use desktop_gpui::screens::pr_pane::daemon::DaemonLink;
use desktop_gpui::screens::pr_pane::files::FilesScreen;
use desktop_gpui::screens::terminal::link::{Link, Status, Target};
use desktop_gpui::screens::{Opened, Screen};
use desktop_gpui::ui::{self, Side};
use futures::FutureExt as _;
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext as _, Entity, TestAppContext, VisualTestContext};
use serde_json::json;
use stand_in::STAND_IN;
use tokio::runtime::Runtime;
use wsp_host_client::HostClient;

struct Host {
    client: Arc<HostClient>,
    runtime: Arc<Runtime>,
    workspace_id: String,
    thread_id: String,
    folder: std::path::PathBuf,
    server: Throwaway,
}

impl Host {
    fn start() -> Option<Host> {
        let server = Throwaway::start_with(STAND_IN, true);
        if server.daemon.is_none() {
            println!("skipped: the host serves files through a wsp-daemon: {}", common::wsp_daemon().unwrap_err());
            return None;
        }
        let folder = server.repo_with("repo", &[("a.txt", "a\n"), ("src/b.txt", "b\n")]);
        server.wsp(&["add", &folder.to_string_lossy()]);
        server.wsp(&["run", "repo", "first", "--agent", "claude"]);
        let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
        let client = runtime.block_on(server.client());
        let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
        let row = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread");
        let (workspace_id, thread_id) = (row.workspace_id.clone(), row.thread_id.clone().expect("a thread id"));
        let folder = folder.canonicalize().unwrap();
        Some(Host { client: Arc::new(client), runtime, workspace_id, thread_id, folder, server })
    }

    fn shell(&self) -> desktop_gpui::screens::Host {
        desktop_gpui::screens::Host::new(self.client.clone(), self.runtime.clone())
    }

    fn ping(&self, link: &Entity<DaemonLink>, cx: &mut TestAppContext) {
        let sent = link.read_with(cx, |link, _| link.request("ping", json!({})));
        answered(cx, "the ping", sent).unwrap_or_else(|r| panic!("ping: {r:?}"));
    }

    fn target(&self) -> Target {
        Target::Workspace(self.workspace_id.clone())
    }

    /// The port the throwaway's own wsp-daemon listens on: the one whose parent is this host, or whose folder is its home.
    fn daemon_port(&self) -> u16 {
        let dir = self.server.dir.path().to_string_lossy().into_owned();
        let mut pids = Vec::new();
        for entry in std::fs::read_dir("/proc").unwrap().flatten() {
            let pid = entry.file_name().to_string_lossy().into_owned();
            if !pid.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            if std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default().trim() != "wsp-daemon" {
                continue;
            }
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
            let ppid = stat.rsplit(')').next().unwrap_or_default().split_whitespace().nth(1).unwrap_or_default().to_owned();
            let parent = std::fs::read(format!("/proc/{ppid}/cmdline")).unwrap_or_default();
            let cwd = std::fs::read_link(format!("/proc/{pid}/cwd")).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
            if String::from_utf8_lossy(&parent).contains(&dir) || cwd.starts_with(&dir) {
                pids.push(pid);
            }
        }
        assert_eq!(pids.len(), 1, "one wsp-daemon of this host: {pids:?}");
        let out = Command::new("ss").args(["-ltnpH"]).output().unwrap();
        let listed = String::from_utf8_lossy(&out.stdout).into_owned();
        let line =
            listed.lines().find(|l| l.contains(&format!("pid={},", pids[0]))).unwrap_or_else(|| panic!("the daemon listens: {listed}"));
        line.split_whitespace().nth(3).unwrap().rsplit(':').next().unwrap().parse().unwrap()
    }

    /// The host's sockets to its daemon now: one per channel it holds open.
    fn channels(&self, port: u16) -> usize {
        let out = Command::new("ss").args(["-tnH", "state", "established", &format!("( dport = :{port} )")]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).lines().filter(|l| !l.trim().is_empty()).count()
    }

    fn settles_at(&self, port: u16, want: usize, what: &str) {
        let until = Instant::now() + Duration::from_secs(10);
        let mut seen = self.channels(port);
        while seen != want && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(100));
            seen = self.channels(port);
        }
        assert_eq!(seen, want, "{what}");
    }

    /// The count once it has held still for three seconds: the host's own turn and checkpoint channels come and go
    /// as the first turn ends.
    fn steady(&self, port: u16) -> usize {
        let until = Instant::now() + Duration::from_secs(40);
        let (mut last, mut since) = (self.channels(port), Instant::now());
        while since.elapsed() < Duration::from_secs(3) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(100));
            let now = self.channels(port);
            if now != last {
                (last, since) = (now, Instant::now());
            }
        }
        last
    }

    /// Drops every socket to the host from this process, as a network blip or a sleeping laptop does.
    fn drop_host_socket(&self) {
        let lock = std::fs::read_to_string(self.server.state().with_file_name("host.lock")).unwrap();
        let port = serde_json::from_str::<serde_json::Value>(&lock).unwrap()["port"].as_u64().unwrap();
        let ok = Command::new("ss")
            .args(["-K", "-tnH", "state", "established", &format!("( dport = :{port} )")])
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(ok.success(), "ss -K");
    }
}

/// Runs the window's tasks, which carry the computer's channel up, until `sent` is answered.
fn answered<T>(cx: &mut TestAppContext, what: &str, sent: impl Future<Output = T>) -> T {
    let mut sent = std::pin::pin!(sent);
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if let Some(answer) = sent.as_mut().now_or_never() {
            return answer;
        }
        assert!(Instant::now() < until, "{what} within 30 s");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Moves the test clock past the heartbeats one at a time, letting each ping come back before the next beat.
fn pass_time(cx: &mut TestAppContext, seconds: u64) {
    for _ in 0..seconds / 10 {
        cx.executor().advance_clock(Duration::from_secs(10));
        let end = Instant::now() + Duration::from_secs(2);
        while Instant::now() < end {
            cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

fn root(link: &Entity<DaemonLink>, cx: &mut TestAppContext, what: &str) -> String {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        if let Some(root) = link.read_with(cx, |l, _| l.root()) {
            return root;
        }
        assert!(Instant::now() < until, "{what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[gpui_kit::test]
fn one_channel_per_workspace_closed_with_its_last_holder(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    cx.executor().allow_parking();
    let port = host.daemon_port();
    let before = host.steady(port);
    let a = cx.update(|cx| DaemonLink::of(&host.shell(), &host.workspace_id, cx));
    host.ping(&a, cx);
    let b = cx.update(|cx| DaemonLink::of(&host.shell(), &host.workspace_id, cx));
    host.ping(&b, cx);
    host.settles_at(port, before + 1, "two holders share one channel");
    drop(a);
    cx.update(|_| {});
    host.settles_at(port, before + 1, "the channel stays while one holder does");
    drop(b);
    cx.update(|_| {});
    pass_time(cx, 20);
    host.settles_at(port, before + 1, "the channel stays inside its release time, as a terminal's does");
    pass_time(cx, 30);
    host.settles_at(port, before, "the channel closes once its last holder's release time passed");
    println!("channels: {before} before, {} with two holders, {before} after", before + 1);
}

/// The terminal's drawer and the Files and Changes panes on one computer, each read through, hold one channel to it.
#[gpui_kit::test]
fn the_terminal_and_the_files_and_changes_panes_on_one_computer_open_one_channel(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let port = host.daemon_port();
    let before = host.steady(port);
    let shell = host.shell();
    let terminal = cx.update(|cx| Link::get(&shell, host.target(), cx));
    let _held = terminal.update(cx, |l, _| l.hold());
    let (_, cx) = cx.add_window_view(|_, _| gpui_kit::EmptyView);
    let opened = |tab: &str| Opened {
        workspace_id: Some(host.workspace_id.clone()),
        thread_id: Some(host.thread_id.clone()),
        tab: Some(tab.to_owned().into()),
        arg: None,
    };
    let (files, changes) = cx.update(|window, cx| {
        let files = cx.new(|cx| FilesScreen::new(shell.clone(), opened("files"), window, cx));
        let changes = cx.new(|cx| ChangesScreen::open(shell.clone(), opened("changes"), window, cx));
        (files, changes)
    });
    let tree = files.read_with(cx, |s, _| s.tree().unwrap().clone());
    let end = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        let live = terminal.read_with(cx, |l, _| *l.status() == Status::Live);
        let listed = tree.read_with(cx, |t, _| t.rows().iter().any(|r| r.name == "src"));
        let read = changes.read_with(cx, |c, _| *c.load() == Load::Ready && !c.not_running());
        if live && listed && read {
            break;
        }
        assert!(Instant::now() < end, "the terminal live {live}, the tree listed {listed}, the changes read {read}");
        std::thread::sleep(Duration::from_millis(50));
    }
    host.settles_at(port, before + 1, "the terminal and both panes ride one channel");
    println!("channels: {before} before, {} with the terminal and both panes", before + 1);
}

#[gpui_kit::test]
fn a_dial_dropped_part_way_leaves_no_channel_open(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    cx.executor().allow_parking();
    let port = host.daemon_port();
    let before = host.steady(port);
    for round in 0..60u64 {
        let link = cx.update(|cx| DaemonLink::of(&host.shell(), &host.workspace_id, cx));
        let asked = link.read_with(cx, |l, _| l.request("ping", json!({})));
        std::thread::sleep(Duration::from_micros(200 * (round % 30)));
        if round % 3 == 0 {
            cx.run_until_parked();
        }
        drop(asked);
        drop(link);
        // Let go at once rather than past its release time, so the dial is cut wherever it got to; the dropped tasks
        // go when the executor next runs, as the app's always does.
        cx.update(|cx| Link::find(&host.target(), cx).map(|l| l.update(cx, |l, cx| l.release_for_test(cx))));
        cx.run_until_parked();
    }
    let left = host.steady(port);
    println!("channels left after 60 dials dropped part-way: {}", left - before);
    assert_eq!(left, before, "every dial a pane let go of is closed on the host");
}

#[gpui_kit::test]
fn the_link_dials_again_by_itself_after_the_host_socket_drops(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    cx.executor().allow_parking();
    let port = host.daemon_port();
    let before = host.steady(port);
    let link = cx.update(|cx| DaemonLink::of(&host.shell(), &host.workspace_id, cx));
    host.ping(&link, cx);
    let first = root(&link, cx, "the hello names the root");
    host.drop_host_socket();
    let until = Instant::now() + Duration::from_secs(10);
    while link.read_with(cx, |l, _| l.up()) {
        assert!(Instant::now() < until, "the link hears its channel go with the socket");
        std::thread::sleep(Duration::from_millis(20));
        cx.run_until_parked();
    }
    let again = root(&link, cx, "the link dials again and its hello names the root, with no request asking");
    assert_eq!(again, first);
    host.settles_at(port, before + 1, "one channel after the redial, the dropped socket's closed by the host");
    println!("dialled again after the socket dropped; root {again}");
}

#[gpui_kit::test]
fn a_folder_that_failed_is_asked_again_and_the_roots_come_back_after_the_socket_drops(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let opened = Opened {
        workspace_id: Some(host.workspace_id.clone()),
        thread_id: Some(host.thread_id.clone()),
        tab: Some("files".into()),
        arg: None,
    };
    let shell = host.shell();
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| FilesScreen::new(shell, opened, window, cx));
        made = Some(screen.clone());
        Root::new(screen, window, cx)
    });
    cx.simulate_resize(size(px(900.), px(700.)));
    let screen = made.unwrap();
    let tree = screen.read_with(cx, |s, _| s.tree().unwrap().clone());
    let wait = |cx: &mut VisualTestContext, secs: u64, mut done: Box<dyn FnMut(&mut VisualTestContext) -> bool>| -> bool {
        let end = Instant::now() + Duration::from_secs(secs);
        loop {
            cx.executor().advance_clock(Duration::from_millis(100));
            cx.run_until_parked();
            if done(cx) {
                return true;
            }
            if Instant::now() > end {
                return false;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    };
    let t = tree.clone();
    assert!(wait(cx, 30, Box::new(move |cx| t.read_with(cx, |t, _| t.rows().iter().any(|r| r.name == "src")))), "the project listed");
    let roots_before = tree.read_with(cx, |t, cx| t.roots(cx));
    let src = host.folder.join("src");
    std::fs::remove_dir_all(&src).unwrap();
    let dir = src.to_string_lossy().into_owned();
    tree.update(cx, |t, cx| t.load(&dir, cx));
    let (t, d) = (tree.clone(), dir.clone());
    assert!(
        wait(cx, 30, Box::new(move |cx| t.read_with(cx, |t, _| t.levels().get(&d).is_some_and(|l| l.error.is_some())))),
        "src fails to list"
    );
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("b.txt"), "b\n").unwrap();
    host.drop_host_socket();
    let (t, d) = (tree.clone(), dir.clone());
    let asked = wait(
        cx,
        15,
        Box::new(move |cx| t.read_with(cx, |t, _| t.levels().get(&d).is_some_and(|l| l.error.is_none() && l.entries.is_some()))),
    );
    let t = tree.clone();
    let rooted = wait(cx, 5, Box::new(move |cx| t.read_with(cx, |t, cx| !t.roots(cx).is_empty())));
    let (folder, roots_after, level) =
        tree.read_with(cx, |t, cx| (t.folder().map(str::to_owned), t.roots(cx), t.levels().get(&dir).cloned()));
    println!("after the drop: src asked again {asked}, roots back {rooted} ({roots_before:?} -> {roots_after:?}), folder {folder:?}, src {level:?}");
    assert!(asked, "the folder that failed is asked again once the link is back (listing.ts retryFailed)");
    assert!(rooted, "the daemon's home is back among the roots");
}
