// SPDX-License-Identifier: AGPL-3.0-only
//! Paths that climb out of the folders the daemon serves, sent the ways the Files pane, the file tab, the finder and the
//! Changes edit send them: the daemon refuses each, nothing outside is read or written, and the window says the
//! daemon's sentence. Without a daemon every test says so and passes nothing.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "pr_pane/stand_in.rs"]
mod stand_in;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::screens::pr_pane::daemon::DaemonLink;
use desktop_gpui::screens::pr_pane::files::file_tab::Read;
use desktop_gpui::screens::pr_pane::files::FilesScreen;
use desktop_gpui::screens::{Opened, Screen};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, Entity, TestAppContext, VisualTestContext};
use serde_json::{json, Value};
use stand_in::STAND_IN;
use tokio::runtime::Runtime;
use wsp_host_client::HostClient;

const WAIT: Duration = Duration::from_secs(30);

struct Host {
    client: Arc<HostClient>,
    runtime: Arc<Runtime>,
    workspace_id: String,
    thread_id: String,
    folder: PathBuf,
    outside: tempfile::TempDir,
    server: Throwaway,
}

impl Host {
    fn start() -> Option<Host> {
        let server = Throwaway::start_with(STAND_IN, true);
        if server.daemon.is_none() {
            println!("skipped: the host serves files through a wsp-daemon: {}", common::wsp_daemon().unwrap_err());
            return None;
        }
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), "outside\n").unwrap();
        let folder = server.repo_with("repo", &[("a.txt", "a\n"), ("src/b.txt", "b\n")]);
        std::os::unix::fs::symlink(outside.path(), folder.join("escape")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret.txt"), folder.join("leak.txt")).unwrap();
        server.wsp(&["add", &folder.to_string_lossy()]);
        server.wsp(&["run", "repo", "first", "--agent", "claude"]);
        let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
        let client = runtime.block_on(server.client());
        let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
        let row = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread wsp run opened");
        let (workspace_id, thread_id) = (row.workspace_id.clone(), row.thread_id.clone().expect("a thread id"));
        let folder = folder.canonicalize().unwrap();
        Some(Host { client: Arc::new(client), runtime, workspace_id, thread_id, folder, outside, server })
    }

    fn shell(&self) -> desktop_gpui::screens::Host {
        desktop_gpui::screens::Host::new(self.client.clone(), self.runtime.clone())
    }

    fn at(&self, rel: &str) -> String {
        self.folder.join(rel).to_string_lossy().into_owned()
    }

    fn turn_in(&self, cwd: &str) {
        std::fs::write(self.server.dir.path().join("next-cwd"), cwd).unwrap();
        self.server.wsp(&["send", &self.thread_id, "move"]);
    }

    fn ask(&self, link: &Entity<DaemonLink>, op: &str, params: Value, cx: &mut TestAppContext) -> Result<String, String> {
        let sent = link.read_with(cx, |link, _| link.request(op, params));
        self.runtime
            .block_on(async { tokio::time::timeout(WAIT, sent).await.expect("an answer") })
            .map(|reply| serde_json::to_string(&reply).unwrap().chars().take(160).collect())
            .map_err(|r| format!("{} ({:?})", r.message, r.code))
    }
}

fn init(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
}

fn mount<'a>(host: &Host, tab: &str, cx: &'a mut TestAppContext) -> (Entity<FilesScreen>, &'a mut VisualTestContext) {
    init(cx);
    let opened = Opened {
        workspace_id: Some(host.workspace_id.clone()),
        thread_id: Some(host.thread_id.clone()),
        tab: Some(gpui_kit::SharedString::from(tab.to_owned())),
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
    (made.unwrap(), cx)
}

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + WAIT;
    loop {
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < end, "{what} within {} s", WAIT.as_secs());
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[gpui_kit::test]
fn every_road_out_of_the_roots_is_refused_by_the_daemon(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    cx.executor().allow_parking();
    let link = cx.update(|cx| DaemonLink::of(&host.shell(), &host.workspace_id, cx));
    let secret = host.outside.path().join("secret.txt").to_string_lossy().into_owned();
    let outside = host.outside.path().to_string_lossy().into_owned();
    let asks: Vec<(&str, Value)> = vec![
        ("fs.list", json!({ "path": host.at("escape"), "gitignore": true })),
        ("fs.list", json!({ "path": host.at("../../../.."), "gitignore": true })),
        ("fs.list", json!({ "path": "/etc", "gitignore": true })),
        ("fs.list", json!({ "path": outside, "gitignore": true })),
        ("fs.read", json!({ "path": host.at("leak.txt") })),
        ("fs.read", json!({ "path": host.at("escape/secret.txt") })),
        ("fs.read", json!({ "path": format!("{}/../../../..{}", host.at(""), secret) })),
        ("fs.read", json!({ "path": "/etc/hostname" })),
        ("fs.read", json!({ "path": "../../../../../../etc/hostname" })),
        ("fs.search", json!({ "path": host.at("escape"), "query": "outside", "mode": "text" })),
        ("fs.search", json!({ "path": outside, "query": "outside", "mode": "text" })),
        ("fs.write", json!({ "path": host.at("leak.txt"), "contents": "written\n" })),
        ("fs.write", json!({ "path": host.at("escape/secret.txt"), "contents": "written\n" })),
        ("fs.write", json!({ "path": format!("{}/../../../..{}", host.at(""), secret), "contents": "written\n" })),
        ("git.diff", json!({ "cwd": outside, "scope": "head" })),
        ("git.status", json!({ "cwd": host.at("escape") })),
    ];
    let mut leaked = Vec::new();
    for (op, params) in asks {
        let said = host.ask(&link, op, params.clone(), cx);
        println!("{op} {params} -> {said:?}");
        if said.is_ok() {
            leaked.push(format!("{op} {params}"));
        }
    }
    let searched = host.ask(&link, "fs.search", json!({ "path": host.at(""), "query": "outside", "mode": "text" }), cx);
    println!("fs.search over the project for the outside file's word -> {searched:?}");
    assert!(searched.as_deref().is_ok_and(|s| !s.contains("leak.txt")), "a link to a file outside is not searched: {searched:?}");
    assert_eq!(std::fs::read_to_string(&secret).unwrap(), "outside\n", "nothing outside was written");
    assert!(leaked.is_empty(), "answered outside the roots: {leaked:?}");
}

#[gpui_kit::test]
fn the_tree_says_the_daemons_refusal_for_an_agent_folder_outside_the_roots(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let (screen, cx) = mount(&host, "files", cx);
    let tree = |cx: &mut VisualTestContext| screen.read_with(cx, |s, _| s.tree().unwrap().clone());
    until(cx, "the tree listing the project", |cx| {
        let t = tree(cx);
        t.read_with(cx, |t, _| !t.rows().is_empty())
    });
    for cwd in ["/etc".to_owned(), host.at("../../../../etc"), host.outside.path().to_string_lossy().into_owned()] {
        host.turn_in(&cwd);
        until(cx, "the tree following the agent's folder", |cx| {
            let t = tree(cx);
            t.read_with(cx, |t, _| {
                t.folder() == Some(cwd.as_str())
                    && t.levels().get(&cwd).is_some_and(|l| !l.pending && (l.error.is_some() || l.entries.is_some()))
            })
        });
        let t = tree(cx);
        let (error, rows) = t.read_with(cx, |t, _| (t.levels()[&cwd].error.clone(), t.rows().len()));
        let drawn = cx.debug_bounds("files-root-error").is_some();
        println!("agent in {cwd}: error {error:?}, {rows} rows, files-root-error drawn {drawn}");
        assert!(error.is_some() && rows == 0, "{cwd} is refused and lists nothing");
    }
}

#[gpui_kit::test]
fn a_file_tab_out_of_the_roots_says_the_daemons_refusal(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let secret = host.outside.path().join("secret.txt").to_string_lossy().into_owned();
    let tabs = [
        format!("file:{secret}"),
        "file:/etc/hostname".to_owned(),
        "file:../../../../../../etc/hostname".to_owned(),
        "file:leak.txt".to_owned(),
        "file:escape/secret.txt".to_owned(),
    ];
    init(cx);
    for tab in tabs {
        let opened = Opened {
            workspace_id: Some(host.workspace_id.clone()),
            thread_id: Some(host.thread_id.clone()),
            tab: Some(gpui_kit::SharedString::from(tab.clone())),
            arg: None,
        };
        let shell = host.shell();
        let mut made = None;
        let (_, vcx) = cx.add_window_view(|window, cx| {
            let screen = cx.new(|cx| FilesScreen::new(shell, opened, window, cx));
            made = Some(screen.clone());
            Root::new(screen, window, cx)
        });
        let file = made.unwrap().read_with(vcx, |s, _| s.file().cloned().unwrap());
        until(vcx, "the file tab's read settling", |cx| file.read_with(cx, |f, _| !matches!(f.read(), Read::Pending { .. })));
        let (path, read) = file.read_with(vcx, |f, _| (f.path().to_owned(), format!("{:?}", f.read())));
        let drawn = vcx.debug_bounds("file-error").is_some();
        println!("{tab}: read {path} -> {read}, file-error drawn {drawn}");
        assert!(read.starts_with("Error"), "{tab} is refused");
    }
}
