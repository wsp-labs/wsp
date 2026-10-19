// SPDX-License-Identifier: AGPL-3.0-only
//! The Changes pane's reads and writes against a host started for each test alone (host-client's tests/common),
//! carrying a wsp-daemon that serves git.* and fs.*: the workspace's daemon link, git.diff for each scope, the viewed
//! mark and its push, discard asked first, and commit only in Uncommitted. The project holds the five changes of
//! states.mjs's `fiveChanges` (data B). Without a daemon on this computer every test says so and passes nothing.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "pr_pane/stand_in.rs"]
mod stand_in;

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::local_store::LocalStore;
use desktop_gpui::screens::composer::store as drafts;
use desktop_gpui::screens::computer::daemon_down::Restart;
use desktop_gpui::screens::dialogs::layer;
use desktop_gpui::screens::pr_pane::changes;
use desktop_gpui::screens::pr_pane::changes::model::{self, Scope, TurnRange};
use desktop_gpui::screens::pr_pane::changes::not_running::Form as NotRunning;
use desktop_gpui::screens::pr_pane::changes::{words, ChangesScreen, Load};
use desktop_gpui::screens::pr_pane::daemon::DaemonLink;
use desktop_gpui::screens::pr_pane::diff::{LineRange, Side as DiffSide};
use desktop_gpui::screens::terminal::state::restart_daemon;
use desktop_gpui::screens::Opened;
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext as _, BorrowAppContext as _, Entity, Focusable as _, Modifiers, TestAppContext, VisualTestContext};
use serde_json::{json, Value};
use stand_in::STAND_IN;
use tokio::runtime::Runtime;
use wsp_host_client::HostClient;

/// A host with a project `repo` holding the committed fixtures of states.mjs's `seed`, and a thread whose first turn ran.
struct Host {
    client: Arc<HostClient>,
    runtime: Arc<Runtime>,
    workspace_id: String,
    thread_id: String,
    folder: PathBuf,
    server: Throwaway,
}

impl Host {
    fn start() -> Option<Host> {
        let server = Throwaway::start_with(STAND_IN, true);
        if server.daemon.is_none() {
            println!("skipped: the host serves git through a wsp-daemon: {}", common::wsp_daemon().unwrap_err());
            return None;
        }
        Some(Host::on(server))
    }

    /// The project and its first thread on `server`, with or without a daemon.
    fn on(server: Throwaway) -> Host {
        let folder = server.repo_with(
            "repo",
            &[
                ("src/keep.ts", "export const a = 1;\nexport const b = 2;\n"),
                ("src/gone.txt", "old\n"),
                ("src/moved.ts", "line one\nline two\nline three\n"),
                ("docs/NOTES.md", "# Notes\n"),
            ],
        );
        let mut logo = vec![0x89, 0x50, 0x4e, 0x47, 0, 0, 0];
        logo.extend(std::iter::repeat_n(3u8, 2048));
        std::fs::write(folder.join("logo.bin"), &logo).unwrap();
        git(&server, &folder, &["config", "user.name", "t"]);
        git(&server, &folder, &["config", "user.email", "t@t"]);
        git(&server, &folder, &["add", "logo.bin"]);
        git(&server, &folder, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "logo"]);
        server.wsp(&["add", &folder.to_string_lossy()]);
        server.wsp(&["run", "repo", "first", "--agent", "claude"]);
        let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
        let client = runtime.block_on(server.client());
        let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
        let row = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread wsp run opened");
        let (workspace_id, thread_id) = (row.workspace_id.clone(), row.thread_id.clone().expect("a thread id"));
        Host { client: Arc::new(client), runtime, workspace_id, thread_id, folder, server }
    }

    fn shell(&self) -> desktop_gpui::screens::Host {
        desktop_gpui::screens::Host::new(self.client.clone(), self.runtime.clone())
    }

    /// states.mjs's `fiveChanges`: an edit, a delete, a rename with an edit, a binary edit and a new file.
    fn five_changes(&self) {
        let f = &self.folder;
        std::fs::write(f.join("src/keep.ts"), "export const a = 1;\nexport const b = 3;\nexport const c = 4;\n").unwrap();
        std::fs::remove_file(f.join("src/gone.txt")).unwrap();
        git(&self.server, f, &["mv", "src/moved.ts", "src/renamed.ts"]);
        std::fs::write(f.join("src/renamed.ts"), "line one\nline two\nline three\nline four\n").unwrap();
        let mut logo = std::fs::read(f.join("logo.bin")).unwrap();
        logo.extend(std::iter::repeat_n(7u8, 2048));
        std::fs::write(f.join("logo.bin"), logo).unwrap();
        std::fs::write(f.join("src/added.ts"), "export const added = true;\n").unwrap();
    }
}

fn git(server: &Throwaway, folder: &Path, args: &[&str]) {
    let ok = Command::new("git").args(args).current_dir(folder).env_clear().envs(server.env()).status().unwrap().success();
    assert!(ok, "git {args:?}");
}

/// Runs one request over the link to its answer, the window's tasks carrying the computer's channel up meanwhile.
fn ask(
    link: &Entity<DaemonLink>,
    op: &str,
    params: Value,
    cx: &mut TestAppContext,
) -> Result<serde_json::Map<String, Value>, desktop_gpui::screens::pr_pane::daemon::Refused> {
    use futures::FutureExt as _;
    let sent = link.read_with(cx, |link, _| link.request(op, params));
    let mut sent = std::pin::pin!(sent);
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if let Some(answer) = sent.as_mut().now_or_never() {
            return answer;
        }
        assert!(Instant::now() < until, "the daemon answered {op} within 30 s");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn paths(reply: &serde_json::Map<String, Value>) -> Vec<String> {
    let mut paths: Vec<String> = reply["files"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap().to_owned()).collect();
    paths.sort();
    paths
}

#[gpui_kit::test]
fn the_link_opens_and_git_diff_answers_with_the_patch(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    cx.executor().allow_parking();
    host.five_changes();
    let link = cx.update(|cx| DaemonLink::of(&host.shell(), &host.workspace_id, cx));
    let folder = host.folder.canonicalize().unwrap().to_string_lossy().into_owned();
    let reply = ask(&link, "git.diff", json!({ "cwd": folder, "scope": "head" }), cx).unwrap_or_else(|r| panic!("git.diff: {r:?}"));
    assert_eq!(paths(&reply), ["logo.bin", "src/added.ts", "src/gone.txt", "src/keep.ts", "src/renamed.ts"], "{reply:?}");
    let keep = reply["files"].as_array().unwrap().iter().find(|f| f["path"] == "src/keep.ts").unwrap();
    assert!(keep["patch"].as_str().unwrap().contains("+export const c = 4;"), "{keep}");
    let until = Instant::now() + Duration::from_secs(10);
    let root = loop {
        cx.run_until_parked();
        if let Some(root) = link.read_with(cx, |link, _| link.root()) {
            break root;
        }
        assert!(Instant::now() < until, "no hello named the link's folder within 10 s");
        std::thread::sleep(Duration::from_millis(50));
    };
    let home = host.server.dir.path().canonicalize().unwrap().to_string_lossy().into_owned();
    assert_eq!(root, home, "the hello names the daemon's home, which holds the project");
    assert!(folder.starts_with(&root), "{folder} inside {root}");
    let again = cx.update(|cx| DaemonLink::of(&host.shell(), &host.workspace_id, cx));
    assert_eq!(again.entity_id(), link.entity_id(), "a second pane shares the workspace's link");
    println!("git.diff over the link: {:?}, root {root}", paths(&reply));
}

/// The pane in a window wrapped in gpui-kit's Root, as the shell mounts it, on a local store of the test's own.
fn mount<'a>(host: &Host, cx: &'a mut TestAppContext) -> (Entity<ChangesScreen>, &'a mut VisualTestContext) {
    mount_on(host, host.shell(), cx)
}

fn mount_on<'a>(
    host: &Host,
    shell: desktop_gpui::screens::Host,
    cx: &'a mut TestAppContext,
) -> (Entity<ChangesScreen>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::local_store::init(host.server.dir.path().join("local-store.json"), cx);
    });
    cx.executor().allow_parking();
    let opened = Opened { workspace_id: Some(host.workspace_id.clone()), thread_id: Some(host.thread_id.clone()), tab: None, arg: None };
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| ChangesScreen::open(shell, opened, window, cx));
        made = Some(screen.clone());
        Root::new(cx.new(|_| Stage(screen)), window, cx)
    });
    cx.simulate_resize(size(px(1200.), px(800.)));
    (made.unwrap(), cx)
}

/// The pane with the dialogs' layer drawn last over it, as the shell's root draws it.
struct Stage(Entity<ChangesScreen>);

impl gpui_kit::Render for Stage {
    fn render(&mut self, window: &mut gpui_kit::Window, cx: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        use gpui_kit::{ParentElement as _, Styled as _};
        gpui_kit::div().size_full().child(self.0.clone()).children(layer::draw(window, cx))
    }
}

/// Lets real time pass until `done` holds of the pane's last Start again.
fn until_started(cx: &mut VisualTestContext, screen: &Entity<ChangesScreen>, what: &str, done: impl Fn(&Restart) -> bool) {
    let restart = cx.read(|cx| screen.read(cx).restart().clone());
    let limit = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if cx.read(|cx| done(restart.read(cx))) {
            return;
        }
        assert!(Instant::now() < limit, "the start never got to: {what}");
        std::thread::sleep(Duration::from_millis(30));
    }
}

/// Lets real time pass until `done` holds of the pane.
fn until(cx: &mut VisualTestContext, screen: &Entity<ChangesScreen>, what: &str, done: impl Fn(&ChangesScreen) -> bool) {
    let limit = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if cx.read(|cx| done(screen.read(cx))) {
            return;
        }
        let state = cx.read(|cx| {
            let s = screen.read(cx);
            let commit = s.commit_state().map(|c| (c.busy, c.drafting, c.error.clone(), c.ticked.len()));
            format!(
                "load {:?}, not running {:?}, folder {:?}, files {:?}, commit {commit:?}",
                s.load(),
                s.not_running_form(),
                s.folder(),
                shown(s)
            )
        });
        assert!(Instant::now() < limit, "the pane never got to: {what}; {state}");
        std::thread::sleep(Duration::from_millis(30));
    }
}

fn settle(cx: &mut VisualTestContext) {
    let end = Instant::now() + Duration::from_millis(800);
    while Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn shown(screen: &ChangesScreen) -> Vec<String> {
    let mut paths: Vec<String> = screen.diff().map(|d| d.files.iter().map(|f| f.path.to_string()).collect()).unwrap_or_default();
    paths.sort();
    paths
}

fn ready(s: &ChangesScreen) -> bool {
    *s.load() == Load::Ready
}

fn leak(selector: String) -> &'static str {
    Box::leak(selector.into_boxed_str())
}

#[gpui_kit::test]
fn the_pane_reads_git_diff_for_each_scope(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    host.five_changes();
    let (screen, cx) = mount(&host, cx);
    until(cx, &screen, "the first read, Branch changes", ready);
    assert_eq!(cx.read(|cx| screen.read(cx).scope()), Scope::Branch);
    let branch = cx.read(|cx| shown(screen.read(cx)));
    let mut seen = vec![(Scope::Branch.label(), branch)];
    for scope in [Scope::Head, Scope::Unstaged, Scope::Staged] {
        cx.update(|_, cx| screen.update(cx, |s, cx| s.set_scope(scope, cx)));
        until(cx, &screen, scope.label(), |s| ready(s) && s.scope() == scope);
        seen.push((scope.label(), cx.read(|cx| shown(screen.read(cx)))));
    }
    println!("{seen:?}");
    let of = |scope: &str| seen.iter().find(|(s, _)| *s == scope).unwrap().1.clone();
    assert_eq!(
        of("Uncommitted"),
        ["logo.bin", "src/added.ts", "src/gone.txt", "src/keep.ts", "src/renamed.ts"],
        "Uncommitted holds every change"
    );
    assert_eq!(of("Staged"), ["src/renamed.ts"], "Staged holds the git mv alone");
    assert!(!of("Working tree").contains(&"src/added.ts".to_owned()), "the working tree leaves the untracked file out");
    let folder = host.folder.canonicalize().unwrap().to_string_lossy().into_owned();
    assert_eq!(cx.read(|cx| screen.read(cx).folder().map(str::to_owned)), Some(folder.clone()), "git ran in the thread's folder");
    cx.update(|window, cx| screen.update(cx, |s, cx| s.fetch(window, cx)));
    assert!(cx.read(|cx| *screen.read(cx).load() == Load::Pending && screen.read(cx).diff().is_some()), "a refresh keeps the last diff");
    until(cx, &screen, "the refresh", ready);
    assert!(cx.read(|cx| screen.read(cx).repo().is_some_and(|r| r.root == folder && !r.branch.is_empty())), "git.status named the branch");

    cx.update(|_, cx| screen.update(cx, |s, cx| s.set_scope(Scope::Head, cx)));
    until(cx, &screen, "Uncommitted again", |s| ready(s) && s.scope() == Scope::Head);
    cx.run_until_parked();
    for selector in ["changes-head", "changes-scope", "changed-files", "changed-files-tree", "viewed-count", "changes-branch"] {
        assert!(cx.debug_bounds(selector).is_some(), "the pane draws {selector}");
    }
    for path in ["src/keep.ts", "logo.bin"] {
        assert!(cx.debug_bounds(leak(format!("tree-row:{path}"))).is_some(), "the tree lists {path}");
    }
    assert!(cx.debug_bounds("head-mark:logo.bin").is_some(), "a file head draws its kind's mark");
    let kept =
        cx.read(|cx| desktop_gpui::screens::pr_pane::changes::model::Kept::read(cx.global::<desktop_gpui::local_store::LocalStore>()));
    assert_eq!(kept.scope(&host.workspace_id), Scope::Head, "the scope is kept on the local store for the next open");

    // Stacked and split are two 28 px ghost toggles, the picked one filled with the accent.
    let accent = cx.read(|cx| changes::hex(desktop_gpui::ui::ActivePalette::palette(cx).accent));
    let filled = |id: &str| leak(format!("{id}:{accent}"));
    let bare = |id: &str| leak(format!("{id}:none"));
    let stacked = cx.debug_bounds(filled("changes-stacked")).expect("stacked, picked, in the accent");
    let split = cx.debug_bounds(bare("changes-split")).expect("split, not picked, unfilled");
    assert_eq!((stacked.size, split.size), (size(px(28.), px(28.)), size(px(28.), px(28.))));
    assert_eq!(split.left() - stacked.right(), px(4.), "4 px apart");
    cx.simulate_click(split.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(filled("changes-split")).is_some() && cx.debug_bounds(bare("changes-stacked")).is_some(),
        "the press picks split"
    );
    assert_eq!(cx.read(|cx| screen.read(cx).code_view().read(cx).layout()), desktop_gpui::screens::pr_pane::diff::Layout::Split);
    let stacked = cx.debug_bounds(bare("changes-stacked")).unwrap();
    cx.simulate_click(stacked.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds(filled("changes-stacked")).is_some(), "and back");

    // A file head's title takes 16% of the primary under the pointer, as today's `[data-title]:hover`.
    cx.update(|_, cx| cx.set_reduce_motion(true));
    cx.run_until_parked();
    let (rest, over) = cx.read(|cx| {
        let p = desktop_gpui::ui::ActivePalette::palette(cx);
        let (fg, primary) = (p.foreground.to_rgb(), p.primary.to_rgb());
        let mix = |a: f32, b: f32| a * 0.84 + b * 0.16;
        let mixed = gpui_kit::Rgba { r: mix(fg.r, primary.r), g: mix(fg.g, primary.g), b: mix(fg.b, primary.b), a: 1. };
        (changes::hex(p.foreground), changes::hex(mixed.into()))
    });
    assert_ne!(rest, over);
    let title = cx.debug_bounds(leak(format!("file-title:logo.bin:{rest}"))).expect("logo.bin's title in the code ink");
    cx.simulate_mouse_move(title.center(), None, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds(leak(format!("file-title:logo.bin:{over}"))).is_some(), "the hovered title in {over}");
    cx.update(|_, cx| cx.set_reduce_motion(false));

    // Past the sheet's width the panel stands inline with a 28 px head; under it the head is 40 px; under 640 px the
    // folder, branch, pin, fold all and the view leave it.
    let head = |cx: &mut VisualTestContext| cx.debug_bounds("changes-head").expect("the head").size.height;
    assert_eq!(head(cx), px(28.), "the inline panel's head");
    cx.simulate_resize(size(px(900.), px(800.)));
    cx.run_until_parked();
    assert_eq!(head(cx), px(40.), "the sheet's head");
    assert!(cx.debug_bounds("changes-branch").is_some() && cx.debug_bounds("changes-crumbs").is_some());
    git(&host.server, &host.folder, &["checkout", "-qb", "feature/a-branch-name-long-enough-to-need-cutting"]);
    cx.update(|window, cx| screen.update(cx, |s, cx| s.fetch(window, cx)));
    until(cx, &screen, "the long branch named", |s| ready(s) && s.repo().is_some_and(|r| r.branch.starts_with("feature/")));
    // `shrink-[999]`: a head short of room takes it from the path before the branch.
    let widths = |cx: &mut VisualTestContext| {
        (cx.debug_bounds("changes-crumbs").unwrap().size.width, cx.debug_bounds("changes-branch").unwrap().size.width)
    };
    let (path, branch) = widths(cx);
    cx.simulate_resize(size(px(660.), px(800.)));
    cx.run_until_parked();
    let (narrow_path, narrow_branch) = widths(cx);
    assert!(
        narrow_path < path && narrow_branch == branch,
        "the path gives first: {path:?} to {narrow_path:?}, branch {branch:?} to {narrow_branch:?}"
    );
    cx.simulate_resize(size(px(600.), px(800.)));
    cx.run_until_parked();
    for selector in ["changes-branch", "changes-crumbs"] {
        assert!(cx.debug_bounds(selector).is_none(), "{selector} leaves the narrow head");
    }
    assert!(cx.debug_bounds("changes-scope").is_some() && cx.debug_bounds("changes-commit").is_some(), "the scope and Commit stay");
}

fn head_scope(cx: &mut VisualTestContext, screen: &Entity<ChangesScreen>) {
    until(cx, screen, "the first read", ready);
    cx.update(|_, cx| screen.update(cx, |s, cx| s.set_scope(Scope::Head, cx)));
    until(cx, screen, "Uncommitted", |s| ready(s) && s.scope() == Scope::Head);
}

/// The marks the host holds for the workspace, read by a client of the test's own.
fn host_marks(host: &Host) -> std::collections::HashMap<String, String> {
    host.runtime
        .block_on(desktop_gpui::screens::pr_pane::changes::host::marks(&host.client, &host.workspace_id))
        .unwrap_or_else(|f| panic!("workspaces.viewed: {}", f.message))
}

#[gpui_kit::test]
fn a_viewed_tick_marks_the_blob_folds_the_file_and_the_hosts_push_folds_another(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    host.five_changes();
    let (screen, cx) = mount(&host, cx);
    head_scope(cx, &screen);
    assert!(!cx.read(|cx| screen.read(cx).is_folded("src/keep.ts")), "a file shows open before it is viewed");

    cx.update(|_, cx| screen.update(cx, |s, cx| s.toggle_viewed("src/keep.ts", cx)));
    until(cx, &screen, "keep.ts marked", |s| s.marks().contains_key("src/keep.ts"));
    assert!(host_marks(&host).contains_key("src/keep.ts"), "the mark is the host's");
    assert!(cx.read(|cx| screen.read(cx).is_folded("src/keep.ts")), "a viewed file folds");
    cx.run_until_parked();
    assert!(cx.read(|cx| screen.read(cx).code_view().read(cx).is_folded("src/keep.ts")), "the code view draws it folded");

    // Another window marks a file: the host's workspace.viewed push folds it here.
    let blob = cx
        .read(|cx| screen.read(cx).diff().unwrap().files.iter().find(|f| f.path.as_ref() == "src/added.ts").unwrap().blob.clone().unwrap());
    host.runtime
        .block_on(desktop_gpui::screens::pr_pane::changes::host::mark(&host.client, &host.workspace_id, "src/added.ts", Some(&blob)))
        .unwrap();
    until(cx, &screen, "the push naming added.ts", |s| s.marks().contains_key("src/added.ts"));
    assert!(cx.read(|cx| screen.read(cx).is_folded("src/added.ts")), "the pushed mark folds the file");
    assert_eq!(cx.read(|cx| screen.read(cx).viewed_count()), 2);
    cx.run_until_parked();
    assert!(cx.debug_bounds("viewed-count").is_some());

    // An edit gives the file new contents, which its mark no longer names.
    std::fs::write(host.folder.join("src/keep.ts"), "export const a = 9;\n").unwrap();
    cx.update(|window, cx| screen.update(cx, |s, cx| s.fetch(window, cx)));
    until(cx, &screen, "keep.ts read again", |s| ready(s) && !s.is_folded("src/keep.ts"));
    assert_eq!(cx.read(|cx| screen.read(cx).viewed_count()), 1, "the edited file reads as not viewed");
}

#[gpui_kit::test]
fn discard_asks_first_and_keeps_the_hosts_refusal_in_the_dialog(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    host.five_changes();
    let (screen, cx) = mount(&host, cx);
    until(cx, &screen, "the first read", ready);
    cx.run_until_parked();
    assert!(cx.debug_bounds("tree-discard:src/keep.ts").is_none(), "Branch changes offers no discard");
    head_scope(cx, &screen);
    cx.run_until_parked();
    assert!(cx.debug_bounds("tree-discard:src/keep.ts").is_some(), "Uncommitted offers discard");
    let under = cx.debug_bounds("changes-commit").expect("Commit in the head, under where the scrim will stand");
    // The dialog slides down from the window's top as it opens; still, it is drawn where it rests.
    cx.update(|_, cx| cx.set_reduce_motion(true));

    cx.update(|window, cx| screen.update(cx, |s, cx| s.ask_discard("src/added.ts", window, cx)));
    cx.run_until_parked();
    let deletes = cx.read(|cx| screen.read(cx).discard_state().map(|d| d.deletes));
    assert_eq!(deletes, Some(true), "a new file's discard says it deletes it");
    assert!(host.folder.join("src/added.ts").exists(), "asking discards nothing");
    let title = cx.debug_bounds("dialog-title").expect("the dialog draws its title");
    let popup = cx.debug_bounds("discard-dialog").expect("the dialog's popup");
    let off = popup.center().y - px(400.);
    assert!(off > px(-1.) && off < px(1.), "the dialog sits in the middle of the 800 px window, as today's: {popup:?}");
    cx.simulate_click(title.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.read(|cx| screen.read(cx).discard_state().is_some()), "a press inside the dialog keeps it open");
    cx.simulate_click(under.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.read(|cx| screen.read(cx).commit_state().is_none()), "the scrim takes the press: Commit under it never opened");
    assert!(cx.read(|cx| screen.read(cx).discard_state().is_some()), "a press on the scrim keeps an alert open");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(cx.read(|cx| screen.read(cx).discard_state().is_none()), "Escape closes it");
    assert!(host.folder.join("src/added.ts").exists(), "a closed dialog discards nothing");

    // The file goes back on its own before the confirm: the host refuses, and its sentence stays in the dialog.
    cx.update(|window, cx| screen.update(cx, |s, cx| s.ask_discard("src/keep.ts", window, cx)));
    git(&host.server, &host.folder, &["checkout", "--", "src/keep.ts"]);
    cx.update(|window, cx| {
        screen.update(cx, |s, cx| {
            s.discard(window, cx);
            s.close_discard(window, cx);
        })
    });
    assert!(cx.read(|cx| screen.read(cx).discard_state().is_some()), "Cancel is held while the discard is on its way");
    // Dispatched at once, before the executor can land the host's answer.
    cx.update(|window, cx| window.dispatch_keystroke(gpui_kit::Keystroke::parse("escape").unwrap(), cx));
    assert!(cx.read(|cx| screen.read(cx).discard_state().is_some_and(|d| d.busy)), "the discard is still on its way");
    assert!(cx.read(|cx| layer::is_open(desktop_gpui::screens::pr_pane::changes::discard::KEY, cx)), "and Escape is held with it");
    until(cx, &screen, "the refusal", |s| s.discard_state().is_some_and(|d| d.refusal.is_some()));
    let said = cx.read(|cx| screen.read(cx).discard_state().unwrap().refusal.clone().unwrap());
    let direct =
        host.runtime.block_on(desktop_gpui::screens::pr_pane::changes::host::discard(&host.client, &host.workspace_id, "src/keep.ts"));
    assert_eq!(Some(said.clone()), direct.err().map(|f| f.message), "the dialog says the host's own sentence");
    cx.run_until_parked();
    assert!(cx.debug_bounds("refusal").is_some(), "the refusal is drawn in the dialog");

    cx.update(|window, cx| screen.update(cx, |s, cx| s.close_discard(window, cx)));
    cx.update(|window, cx| screen.update(cx, |s, cx| s.ask_discard("src/added.ts", window, cx)));
    cx.update(|window, cx| screen.update(cx, |s, cx| s.discard(window, cx)));
    until(cx, &screen, "the discard done and read again", |s| {
        s.discard_state().is_none() && ready(s) && !shown(s).contains(&"src/added.ts".to_owned())
    });
    assert!(!host.folder.join("src/added.ts").exists(), "the confirmed discard deleted the new file");
    println!("refused with: {said}");
}

#[gpui_kit::test]
fn commit_stands_only_in_uncommitted_and_commits_the_ticked_files(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    host.five_changes();
    let (screen, cx) = mount(&host, cx);
    until(cx, &screen, "the first read", ready);
    cx.run_until_parked();
    assert!(cx.debug_bounds("changes-commit").is_none(), "Branch changes has no Commit");
    for scope in [Scope::Unstaged, Scope::Staged] {
        cx.update(|_, cx| screen.update(cx, |s, cx| s.set_scope(scope, cx)));
        until(cx, &screen, scope.label(), |s| ready(s) && s.scope() == scope);
        cx.run_until_parked();
        assert!(cx.debug_bounds("changes-commit").is_none(), "{} has no Commit", scope.label());
    }
    head_scope(cx, &screen);
    cx.run_until_parked();
    assert!(cx.debug_bounds("changes-commit").is_some(), "Uncommitted has Commit");

    cx.update(|window, cx| screen.update(cx, |s, cx| s.open_commit(window, cx)));
    until(cx, &screen, "the draft answered", |s| s.commit_state().is_some_and(|c| !c.drafting));
    cx.run_until_parked();
    assert!(cx.debug_bounds("commit-box").is_some() && cx.debug_bounds("commit-tick:src/keep.ts").is_some(), "the box and its ticks");
    let note = cx.read(|cx| screen.read(cx).commit_state().unwrap().note.clone());
    assert_eq!(note.as_deref(), Some(words::NO_DRAFT), "the stand-in drafts nothing, which the box says");
    cx.update(|window, cx| {
        screen.update(cx, |s, cx| {
            s.tick("logo.bin", cx);
            let field = s.commit_state().unwrap().message.clone();
            field.update(cx, |f, cx| f.set_value("Keep the fixtures", window, cx));
        })
    });
    cx.update(|window, cx| screen.update(cx, |s, cx| s.make_commit(window, cx)));
    until(cx, &screen, "the commit made and the diff read again", |s| s.commit_state().is_none() && ready(s) && shown(s) == ["logo.bin"]);
    let out = Command::new("git")
        .args(["log", "-1", "--format=%s"])
        .current_dir(&host.folder)
        .env_clear()
        .envs(host.server.env())
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "Keep the fixtures");
}

/// A door in front of the host that writes down the op of every request a client sends through it, read off the
/// client's WebSocket frames; a client dials it through `state`, whose lock names the door's port.
struct Door {
    state: PathBuf,
    ops: Arc<Mutex<Vec<String>>>,
    _dir: tempfile::TempDir,
}

impl Door {
    fn open(host: &Host) -> Door {
        let dir = tempfile::tempdir().unwrap();
        let served = host.server.state();
        let mut lock: Value = serde_json::from_str(&std::fs::read_to_string(served.with_file_name("host.lock")).unwrap()).unwrap();
        let upstream = format!("127.0.0.1:{}", lock["port"]);
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        lock["port"] = json!(listener.local_addr().unwrap().port());
        std::fs::write(dir.path().join("host.lock"), lock.to_string()).unwrap();
        std::fs::copy(served.with_file_name("host-token"), dir.path().join("host-token")).unwrap();
        let ops = Arc::new(Mutex::new(Vec::new()));
        let heard = ops.clone();
        std::thread::spawn(move || {
            for client in listener.incoming() {
                let (Ok(mut client), Ok(mut upstream)) = (client, std::net::TcpStream::connect(&upstream)) else { return };
                let (mut back, mut to) = (upstream.try_clone().unwrap(), client.try_clone().unwrap());
                std::thread::spawn(move || _ = std::io::copy(&mut back, &mut to));
                let heard = heard.clone();
                std::thread::spawn(move || {
                    let (mut seen, mut upgraded, mut chunk) = (Vec::new(), false, vec![0u8; 1 << 16]);
                    while let Ok(n @ 1..) = client.read(&mut chunk) {
                        if upstream.write_all(&chunk[..n]).is_err() {
                            return;
                        }
                        seen.extend_from_slice(&chunk[..n]);
                        if !upgraded {
                            let Some(end) = seen.windows(4).position(|w| w == b"\r\n\r\n") else { continue };
                            seen.drain(..end + 4);
                            upgraded = true;
                        }
                        while let Some((payload, used)) = client_frame(&seen) {
                            let op = serde_json::from_slice::<Value>(&payload).ok().and_then(|v| v["op"].as_str().map(str::to_owned));
                            heard.lock().unwrap().extend(op);
                            seen.drain(..used);
                        }
                    }
                });
            }
        });
        Door { state: dir.path().join("state.json"), ops, _dir: dir }
    }

    fn sent(&self, op: &str) -> usize {
        self.ops.lock().unwrap().iter().filter(|o| *o == op).count()
    }
}

/// The first whole frame of a client's WebSocket bytes: its payload unmasked and how many bytes it took.
fn client_frame(bytes: &[u8]) -> Option<(Vec<u8>, usize)> {
    let second = *bytes.get(1)?;
    let (len, mut at) = match second & 0x7f {
        126 => (u16::from_be_bytes(bytes.get(2..4)?.try_into().ok()?) as usize, 4),
        127 => (u64::from_be_bytes(bytes.get(2..10)?.try_into().ok()?) as usize, 10),
        n => (n as usize, 2),
    };
    let mut mask = [0u8; 4];
    if second & 0x80 != 0 {
        mask = bytes.get(at..at + 4)?.try_into().ok()?;
        at += 4;
    }
    let payload = bytes.get(at..at + len)?;
    Some((payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]).collect(), at + len))
}

/// A second press on a confirm before the host answers sends nothing more: one discard, one commit.
#[gpui_kit::test]
fn a_second_press_before_the_host_answers_sends_no_second_discard_or_commit(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    host.five_changes();
    let door = Door::open(&host);
    let client = host.runtime.block_on(HostClient::connect(&door.state, &host.server.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let (screen, cx) = mount_on(&host, desktop_gpui::screens::Host::new(Arc::new(client), host.runtime.clone()), cx);
    head_scope(cx, &screen);

    cx.update(|window, cx| screen.update(cx, |s, cx| s.ask_discard("src/keep.ts", window, cx)));
    cx.run_until_parked();
    cx.update(|window, cx| {
        screen.update(cx, |s, cx| {
            s.discard(window, cx);
            s.discard(window, cx);
        })
    });
    until(cx, &screen, "the discard done and read again", |s| {
        s.discard_state().is_none() && ready(s) && !shown(s).contains(&"src/keep.ts".to_owned())
    });
    settle(cx);
    assert_eq!(door.sent("workspaces.discard"), 1, "two presses on Discard sent one discard");

    cx.update(|window, cx| screen.update(cx, |s, cx| s.open_commit(window, cx)));
    until(cx, &screen, "the draft answered", |s| s.commit_state().is_some_and(|c| !c.drafting));
    cx.update(|window, cx| {
        screen.update(cx, |s, cx| {
            let field = s.commit_state().unwrap().message.clone();
            field.update(cx, |f, cx| f.set_value("Once", window, cx));
        })
    });
    cx.update(|window, cx| {
        screen.update(cx, |s, cx| {
            s.make_commit(window, cx);
            s.make_commit(window, cx);
        })
    });
    until(cx, &screen, "the commit made and the diff read again", |s| s.commit_state().is_none() && ready(s));
    settle(cx);
    assert_eq!(door.sent("workspaces.commit"), 1, "two presses on Commit sent one commit");
}

#[gpui_kit::test]
fn an_edit_saves_the_whole_file_over_the_link(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    host.five_changes();
    let (screen, cx) = mount(&host, cx);
    head_scope(cx, &screen);
    cx.update(|window, cx| screen.update(cx, |s, cx| s.start_edit("src/keep.ts", window, cx)));
    until(cx, &screen, "the editor open on keep.ts", |s| s.editing() == Some("src/keep.ts"));
    let text = cx.read(|cx| screen.read(cx).editor_text(cx));
    assert_eq!(
        text.as_deref(),
        Some("export const a = 1;\nexport const b = 3;\nexport const c = 4;\n"),
        "the editor holds the whole new side"
    );
    cx.run_until_parked();
    let editor = cx.debug_bounds("changes-editor").expect("the editor drawn in the file's place");
    let lines = cx.read(|cx| screen.read(cx).editor_text(cx)).unwrap().split('\n').count();
    assert_eq!(lines, 4, "three lines and the empty one after the last newline");
    assert_eq!(editor.size.height, px(20.) * lines as f32, "one 20 px row per line and nothing more, as today's rows: no box, no cap");
    assert_eq!(cx.read(|cx| screen.read(cx).editor_row(cx)), Some(px(20.)), "and the editor lays its lines out at that row");
    cx.update(|window, cx| {
        screen.update(cx, |s, cx| s.set_editor_text("export const a = 1;\nexport const edited = true;\n", window, cx));
        screen.update(cx, |s, cx| s.save_edit(window, cx));
    });
    until(cx, &screen, "the save done and read again", |s| s.editing().is_none() && ready(s));
    assert_eq!(std::fs::read_to_string(host.folder.join("src/keep.ts")).unwrap(), "export const a = 1;\nexport const edited = true;\n");

    // A save that changed nothing writes nothing, so a change made meanwhile on disk stays.
    cx.update(|window, cx| screen.update(cx, |s, cx| s.start_edit("src/keep.ts", window, cx)));
    until(cx, &screen, "the editor open again", |s| s.editing() == Some("src/keep.ts"));
    std::fs::write(host.folder.join("src/keep.ts"), "changed outside\n").unwrap();
    cx.update(|window, cx| screen.update(cx, |s, cx| s.save_edit(window, cx)));
    until(cx, &screen, "the editor closed", |s| s.editing().is_none());
    assert_eq!(std::fs::read_to_string(host.folder.join("src/keep.ts")).unwrap(), "changed outside\n", "an untouched save wrote nothing");

    // A mode change alone has no text: the editor does not open on an empty file.
    git(&host.server, &host.folder, &["update-index", "--chmod=+x", "docs/NOTES.md"]);
    std::fs::set_permissions(host.folder.join("docs/NOTES.md"), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    cx.update(|window, cx| screen.update(cx, |s, cx| s.fetch(window, cx)));
    until(cx, &screen, "the mode change listed", |s| ready(s) && shown(s).contains(&"docs/NOTES.md".to_owned()));
    cx.update(|window, cx| screen.update(cx, |s, cx| s.start_edit("docs/NOTES.md", window, cx)));
    until(cx, &screen, "the no-text note", |s| s.write_note() == Some(words::NO_TEXT) || s.editing().is_some());
    assert_eq!(cx.read(|cx| screen.read(cx).editing().map(str::to_owned)), None, "no editor on a file with no text");

    // A file with carriage returns is not edited here.
    std::fs::write(host.folder.join("docs/crlf.txt"), "one\r\ntwo\r\n").unwrap();
    git(&host.server, &host.folder, &["add", "docs/crlf.txt"]);
    git(&host.server, &host.folder, &["-c", "core.autocrlf=false", "commit", "-qm", "crlf", "--", "docs/crlf.txt"]);
    std::fs::write(host.folder.join("docs/crlf.txt"), "one\r\ntwo\r\nthree\r\n").unwrap();
    cx.update(|window, cx| screen.update(cx, |s, cx| s.fetch(window, cx)));
    until(cx, &screen, "the CRLF file listed", |s| ready(s) && shown(s).contains(&"docs/crlf.txt".to_owned()));
    cx.update(|window, cx| screen.update(cx, |s, cx| s.start_edit("docs/crlf.txt", window, cx)));
    until(cx, &screen, "the line endings note", |s| s.write_note() == Some(words::LINE_ENDINGS) || s.editing().is_some());
    assert_eq!(cx.read(|cx| screen.read(cx).editing().map(str::to_owned)), None, "no editor on a file with carriage returns");

    // A whole-file read asked before a move to Staged does not open the editor under Staged.
    cx.update(|window, cx| {
        screen.update(cx, |s, cx| {
            s.start_edit("src/renamed.ts", window, cx);
            s.set_scope(Scope::Staged, cx);
        })
    });
    until(cx, &screen, "Staged, after the edit's read", |s| ready(s) && s.scope() == Scope::Staged);
    settle(cx);
    assert_eq!(cx.read(|cx| screen.read(cx).editing().map(str::to_owned)), None, "a late whole-file read opens nothing");
    cx.update(|_, cx| screen.update(cx, |s, cx| s.set_scope(Scope::Staged, cx)));
    until(cx, &screen, "Staged", |s| ready(s) && s.scope() == Scope::Staged);
    cx.run_until_parked();
    assert!(cx.debug_bounds("edit:src/renamed.ts").is_none(), "Staged offers no edit");
    cx.update(|_, cx| screen.update(cx, |s, cx| s.set_scope(Scope::Head, cx)));
    until(cx, &screen, "Uncommitted", |s| ready(s) && s.scope() == Scope::Head);
    cx.run_until_parked();
    assert!(cx.debug_bounds("edit:src/renamed.ts").is_some(), "Uncommitted offers the edit");
    assert!(
        cx.debug_bounds("edit:src/gone.txt").is_none() && cx.debug_bounds("edit:logo.bin").is_none(),
        "no edit for a deleted or binary file"
    );
}

#[gpui_kit::test]
fn the_states_empty_not_a_repository_a_turn_narrowed_and_its_snapshot_gone(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    let (screen, cx) = mount(&host, cx);
    head_scope(cx, &screen);
    assert!(cx.read(|cx| screen.read(cx).diff().is_some_and(|d| d.files.is_empty())), "a clean checkout lists nothing");
    cx.run_until_parked();
    assert!(cx.debug_bounds("changes-line").is_some(), "the empty line stands in the body");

    // A turn: snapshots before and after an edit, narrowed to one file of the two it changed.
    let link = cx.update(|_, cx| DaemonLink::of(&host.shell(), &host.workspace_id, cx));
    let folder = host.folder.canonicalize().unwrap().to_string_lossy().into_owned();
    let snap = |cx: &mut VisualTestContext| {
        let sent = link.read_with(cx, |l, _| l.request("git.snapshot", json!({ "cwd": folder })));
        host.runtime.block_on(sent).unwrap()["commit"].as_str().unwrap().to_owned()
    };
    let from = snap(cx);
    std::fs::write(host.folder.join("src/keep.ts"), "export const turned = 1;\n").unwrap();
    std::fs::write(host.folder.join("docs/NOTES.md"), "# Notes, turned\n").unwrap();
    let to = snap(cx);
    let range = TurnRange {
        thread_id: host.thread_id.clone(),
        turn_id: "turn-1".into(),
        cwd: folder.clone(),
        from: from.clone(),
        to,
        path: Some("src/elsewhere.ts".into()),
        only: Some(vec!["src/keep.ts".into()]),
    };
    cx.update(|_, cx| cx.update_global(|store: &mut LocalStore, _| model::open_turn(store, &host.workspace_id, range.clone()).unwrap()));
    until(cx, &screen, "the turn read", |s| ready(s) && s.turn().is_some() && shown(s) == ["src/keep.ts"]);
    assert_eq!(cx.read(|cx| screen.read(cx).reveal_note().map(str::to_owned)), Some("elsewhere.ts has no changes in this turn".to_owned()));
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("changes-commit").is_none() && cx.debug_bounds("discard:src/keep.ts").is_none(),
        "a turn is not committed or discarded"
    );
    cx.update(|_, cx| screen.update(cx, |s, cx| s.show_all(cx)));
    until(cx, &screen, "Show all", |s| ready(s) && shown(s) == ["docs/NOTES.md", "src/keep.ts"]);

    let gone = TurnRange { to: "0".repeat(40), only: None, path: None, ..range };
    cx.update(|_, cx| cx.update_global(|store: &mut LocalStore, _| model::open_turn(store, &host.workspace_id, gone).unwrap()));
    until(cx, &screen, "the snapshot gone", |s| matches!(s.load(), Load::Error { line: Some(l), .. } if *l == words::SNAPSHOT_GONE));

    // A folder outside any repository, which the thread's panes are pinned to.
    // Pinned while the Uncommitted read is still on its way: the pin is read too.
    let plain = host.server.dir.path().join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let plain = plain.canonicalize().unwrap().to_string_lossy().into_owned();
    cx.update(|_, cx| screen.update(cx, |s, cx| s.set_scope(Scope::Head, cx)));
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| screen.read(cx).load().clone()), Load::Pending, "the Uncommitted read is on its way");
    cx.update(|_, cx| link.update(cx, |l, cx| l.pin(&host.thread_id, &plain, cx)));
    until(cx, &screen, "the not-a-repository line", |s| matches!(s.load(), Load::Error { line: Some(l), .. } if *l == words::NOT_A_REPO));
    assert_eq!(cx.read(|cx| screen.read(cx).folder().map(str::to_owned)), Some(plain));
}

#[gpui_kit::test]
fn a_line_comment_goes_to_the_threads_composer_by_send_to_thread(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    host.five_changes();
    let (screen, cx) = mount(&host, cx);
    head_scope(cx, &screen);
    let comments = cx.read(|cx| screen.read(cx).comments().clone());
    cx.update(|window, cx| {
        let patch = screen.read(cx).diff().unwrap().files.iter().find(|f| f.path.as_ref() == "src/keep.ts").unwrap().patch.clone().unwrap();
        comments.update(cx, |c, cx| {
            assert!(c.begin(&LineRange::new("src/keep.ts", DiffSide::Modified, 2, 3), &patch, window, cx), "the field opens on the lines");
            let field = c.draft_field().unwrap().clone();
            field.update(cx, |f, cx| f.set_value("Why three?", window, cx));
            assert!(c.submit(cx));
        });
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("changes-send").is_some(), "Send to thread stands in the head once a comment is kept");
    cx.update(|_, cx| screen.update(cx, |s, cx| s.send_comments(cx)));
    cx.run_until_parked();
    let draft = cx.read(|cx| drafts::draft(cx.global::<LocalStore>(), &host.thread_id)).map(|(text, _)| text).unwrap_or_default();
    assert!(draft.contains("Why three?") && draft.contains("src/keep.ts"), "the thread's draft holds the quote: {draft:?}");
    assert!(cx.read(|cx| comments.read(cx).kept().is_empty()), "the pane keeps none after");
    assert!(cx.debug_bounds("changes-send").is_none());
}

#[gpui_kit::test]
fn show_in_changes_names_the_file_from_the_checkouts_top_in_a_pinned_subfolder(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    host.five_changes();
    let (screen, cx) = mount(&host, cx);
    head_scope(cx, &screen);
    let src = host.folder.join("src").canonicalize().unwrap().to_string_lossy().into_owned();
    let link = cx.update(|_, cx| DaemonLink::of(&host.shell(), &host.workspace_id, cx));
    cx.update(|_, cx| link.update(cx, |l, cx| l.pin(&host.thread_id, &src, cx)));
    until(cx, &screen, "git run in src", |s| ready(s) && s.folder() == Some(src.as_str()));
    cx.update(|_, cx| screen.update(cx, |s, cx| s.toggle_file("src/keep.ts", cx)));
    assert!(cx.read(|cx| screen.read(cx).is_folded("src/keep.ts")));
    cx.update(|_, cx| screen.update(cx, |s, cx| s.show_file(&format!("{src}/keep.ts"), cx)));
    assert_eq!(cx.read(|cx| screen.read(cx).reveal_note().map(str::to_owned)), None, "the file is found, not named as missing");
    assert!(!cx.read(|cx| screen.read(cx).is_folded("src/keep.ts")), "and opened");

    // The head's crumbs name each folder down to the one shown, and a crumb pins the panes to its folder.
    cx.run_until_parked();
    let repo = host.folder.canonicalize().unwrap().to_string_lossy().into_owned();
    assert!(cx.debug_bounds(leak(format!("crumb-{src}"))).is_some(), "a crumb names the folder shown");
    let up = cx.debug_bounds(leak(format!("crumb-{repo}"))).expect("a crumb for the folder above");
    cx.simulate_click(up.center(), Modifiers::none());
    until(cx, &screen, "git run in the repository's top", |s| ready(s) && s.folder() == Some(repo.as_str()));
    // Backspace on the pane goes up a folder, as on the Files pane.
    let home = host.server.dir.path().canonicalize().unwrap().to_string_lossy().into_owned();
    cx.update(|window, cx| screen.read(cx).focus_handle(cx).focus(window, cx));
    cx.simulate_keystrokes("backspace");
    until(cx, &screen, "the folder above", |s| s.folder() == Some(home.as_str()));
    cx.update(|_, cx| link.update(cx, |l, cx| l.pin(&host.thread_id, &src, cx)));
    until(cx, &screen, "git run in src again", |s| ready(s) && s.folder() == Some(src.as_str()));
    cx.update(|window, cx| screen.read(cx).focus_handle(cx).focus(window, cx));
    cx.simulate_keystrokes("alt-up");
    until(cx, &screen, "the folder above src on Alt+Up", |s| s.folder() == Some(repo.as_str()));
}

/// The Files pane's Show in Changes reaches the open Changes pane, which shows the file once and takes the ask.
#[gpui_kit::test]
fn the_files_panes_show_in_changes_opens_the_file_here(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    host.five_changes();
    let (screen, cx) = mount(&host, cx);
    head_scope(cx, &screen);
    cx.update(|_, cx| screen.update(cx, |s, cx| s.toggle_file("src/keep.ts", cx)));
    assert!(cx.read(|cx| screen.read(cx).is_folded("src/keep.ts")));
    let keep = host.folder.join("src/keep.ts").canonicalize().unwrap().to_string_lossy().into_owned();
    cx.update(|_, cx| desktop_gpui::screens::pr_pane::files::show_in_changes(&host.workspace_id, &keep, cx));
    settle(cx);
    assert!(!cx.read(|cx| screen.read(cx).is_folded("src/keep.ts")), "the asked file opens");
    assert!(cx.read(|cx| cx.try_global::<desktop_gpui::screens::pr_pane::files::ShowInChanges>().is_none()), "and the ask is taken");
}

#[gpui_kit::test]
fn two_workspaces_links_each_keep_their_own_root(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    cx.executor().allow_parking();
    let other = host.server.repo_with("other", &[("a.txt", "a\n")]);
    host.server.wsp(&["add", &other.to_string_lossy()]);
    host.server.wsp(&["run", "other", "second", "--agent", "claude"]);
    let threads = host.runtime.block_on(host.client.threads()).unwrap();
    let second = threads.iter().find(|t| t.prompt.as_deref() == Some("second")).expect("the second thread").workspace_id.clone();
    assert_ne!(second, host.workspace_id, "a second project is a second workspace");
    let rooted = |link: &Entity<DaemonLink>, cx: &mut TestAppContext| {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            cx.run_until_parked();
            if let Some(root) = link.read_with(cx, |l, _| l.root()) {
                return root;
            }
            assert!(Instant::now() < until, "no hello named the link's folder within 10 s");
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    let a = cx.update(|cx| DaemonLink::of(&host.shell(), &host.workspace_id, cx));
    ask(&a, "ping", json!({}), cx).unwrap();
    let root_a = rooted(&a, cx);
    let b = cx.update(|cx| DaemonLink::of(&host.shell(), &second, cx));
    ask(&b, "ping", json!({}), cx).unwrap();
    rooted(&b, cx);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(a.read_with(cx, |l, _| l.root()), Some(root_a), "the first link's root outlives the second link's hello");
}

/// A dial the host turns away by its kind reads Refused over the host's sentence, as `NotRunning.tsx` says it through
/// `useLinkDown`, and not starting, then Unreachable.
#[gpui_kit::test]
fn a_dial_the_host_turns_away_reads_refused_in_its_sentence(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    cx.executor().allow_parking();
    let target = desktop_gpui::screens::terminal::link::Target::Workspace(host.workspace_id.clone());
    let link = cx.update(|cx| desktop_gpui::screens::terminal::link::Link::get(&host.shell(), target, cx));
    let mut turned = wsp_host_client::Failure::new("the host's sentence");
    turned.kind = Some("usage".into());
    link.update(cx, |l, cx| l.refuse_for_test(turned, cx));
    let (screen, cx) = mount(&host, cx);
    let refused = NotRunning::LinkDown { word: words::REFUSED, line: "the host's sentence".into() };
    until(cx, &screen, "the link refused", |s| s.not_running_form().as_ref() == Some(&refused));
    cx.run_until_parked();
    assert!(cx.debug_bounds("not-running-title:Refused").is_some(), "the word drawn");
    assert!(cx.debug_bounds("not-running-line:the host's sentence").is_some(), "over the host's sentence");

    // The workspace runs again: the link is dialled then, not at its refused backoff, which this clock never reaches.
    host.five_changes();
    host.runtime.block_on(restart_daemon(&host.shell(), &host.workspace_id)).unwrap_or_else(|refused| panic!("{refused}"));
    until(cx, &screen, "the diff read once the workspace runs again", |s| ready(s) && !s.not_running());
}

#[gpui_kit::test]
fn a_refused_read_says_gits_sentence_and_keeps_the_last_diff(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    host.five_changes();
    let (screen, cx) = mount(&host, cx);
    head_scope(cx, &screen);
    let before = cx.read(|cx| shown(screen.read(cx)));
    std::fs::write(host.folder.join(".git/index"), b"not an index").unwrap();
    cx.update(|window, cx| screen.update(cx, |s, cx| s.fetch(window, cx)));
    until(cx, &screen, "git's refusal", |s| matches!(s.load(), Load::Error { line: None, .. }));
    assert_eq!(cx.read(|cx| shown(screen.read(cx))), before, "the last diff stays through a refused read");
    cx.run_until_parked();
    assert!(cx.debug_bounds("changes-error").is_some(), "git's sentence stands under the head");
}

/// What this computer's sentences call it: the places list's own row, as the host names it.
fn here_name(host: &Host) -> String {
    let list: wsp_host_client::protocol::PlacesListRequest = serde_json::from_value(json!({})).unwrap();
    let places = host.runtime.block_on(host.client.call(list)).unwrap_or_else(|f| panic!("{}", f.message));
    let places = serde_json::to_value(&places.places).unwrap();
    let here = places.as_array().unwrap().iter().find(|p| p["id"] == "here").expect("this computer's row").clone();
    here["label"].as_str().or(here["name"].as_str()).unwrap().to_owned()
}

fn daemon_down(s: &ChangesScreen) -> bool {
    matches!(s.not_running_form(), Some(NotRunning::DaemonDown { .. }))
}

fn press(cx: &mut VisualTestContext, selector: &'static str) {
    cx.run_until_parked();
    let at = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn"));
    cx.simulate_click(at.center(), Modifiers::none());
}

/// The daemon this host runs for its own computer did not start: the pane says the computer's sentence over the
/// button that starts it again, as `DaemonDown` does, and not the thread's line.
#[gpui_kit::test]
fn with_no_daemon_the_pane_says_the_computer_stopped_over_start_again(cx: &mut TestAppContext) {
    let host = Host::on(Throwaway::start_with(STAND_IN, false));
    let said = format!("{}'s terminals and files stopped", here_name(&host));
    let (screen, cx) = mount(&host, cx);
    until(cx, &screen, "not running", ChangesScreen::not_running);
    until(cx, &screen, "the computer's sentence", daemon_down);
    assert_eq!(
        cx.read(|cx| screen.read(cx).not_running_form()),
        Some(NotRunning::DaemonDown { said: said.clone(), start: "Start again".into() }),
        "the reading's sentence and its button's word"
    );
    cx.run_until_parked();
    let drawn = cx.debug_bounds(leak(format!("daemon-down-said:{said}"))).expect("the computer's sentence handed to DaemonDown");
    let down = cx.debug_bounds("daemon-down").expect("the computer's DaemonDown is drawn");
    let button = cx.debug_bounds("button").expect("with its Start again");
    assert!(drawn.contains(&down.center()) && down.contains(&button.center()), "the button is DaemonDown's, under that sentence");
    assert!(cx.debug_bounds("not-running-head").is_none(), "and no title and line of any words");
}

/// Start again with nothing to start says the host's own sentence under the button, and with a daemon to start reads
/// over it; a daemon that goes while the pane is open is said, and one started again from anywhere (the sidebar's
/// Reconnect here) is read over with no reopen.
#[gpui_kit::test]
fn a_daemon_that_comes_back_is_read_again_with_no_reopen(cx: &mut TestAppContext) {
    let server = Throwaway::start_with(STAND_IN, true);
    let Some(daemon) = server.daemon.clone() else {
        println!("skipped: the host serves git through a wsp-daemon: {}", common::wsp_daemon().unwrap_err());
        return;
    };
    let asset = server.dir.path().join("hostpkg/assets/daemon").join(common::triple()).join("wsp-daemon");
    std::fs::remove_file(&asset).unwrap();
    let host = Host::on(server);
    host.five_changes();
    let five = ["logo.bin", "src/added.ts", "src/gone.txt", "src/keep.ts", "src/renamed.ts"];
    let (screen, cx) = mount(&host, cx);
    until(cx, &screen, "the computer's sentence", daemon_down);

    press(cx, "button");
    until_started(cx, &screen, "the start refused", |r| r.refusal.is_some() && !r.busy);
    let refusal = cx.read(|cx| screen.read(cx).restart().read(cx).refusal.clone()).unwrap();
    let direct = host.runtime.block_on(restart_daemon(&host.shell(), &host.workspace_id)).err().map(|line| line.to_string());
    assert_eq!(Some(refusal.clone()), direct, "the host's own refusal, as start_refused_line words it");
    assert!(refusal.starts_with("the daemon for this computer did not start") && refusal.ends_with(". Start again."), "{refusal}");
    cx.run_until_parked();
    assert!(cx.debug_bounds("start-daemon-refused").is_some(), "the host's sentence stands under the button");

    std::os::unix::fs::symlink(&daemon, &asset).unwrap();
    press(cx, "button");
    until(cx, &screen, "the diff read after Start again", |s| ready(s) && !s.not_running() && shown(s) == five);

    // The host starts nothing and says no status: the link that went is found out, and the computer still reads as
    // there, so the link is the one reconnecting.
    std::fs::remove_file(&asset).unwrap();
    _ = host.runtime.block_on(restart_daemon(&host.shell(), &host.workspace_id));
    let reconnecting = NotRunning::LinkDown { word: words::RECONNECTING, line: format!("Reconnecting to {}", here_name(&host)) };
    until(cx, &screen, "the link reconnecting", |s| s.not_running_form().as_ref() == Some(&reconnecting));
    cx.run_until_parked();
    let (word, line) = ("not-running-title:Reconnecting", leak(format!("not-running-line:Reconnecting to {}", here_name(&host))));
    assert!(cx.debug_bounds(word).is_some() && cx.debug_bounds(line).is_some(), "the link's word and line");
    assert!(cx.debug_bounds("daemon-down").is_none(), "and no Start");

    // Started again from elsewhere while the phase stays running all along.
    std::os::unix::fs::symlink(&daemon, &asset).unwrap();
    host.runtime.block_on(restart_daemon(&host.shell(), &host.workspace_id)).unwrap_or_else(|refused| panic!("{refused}"));
    until(cx, &screen, "the diff read over the daemon started elsewhere", |s| ready(s) && !s.not_running() && shown(s) == five);
    cx.run_until_parked();
    assert!(cx.debug_bounds("changes-not-running").is_none());
}

/// The panes pinned to another repository in the daemon's home list its changes, but the host's discard and commit
/// act in the project's own checkout: a discard pressed there must not put back the project's file of the same name.
#[gpui_kit::test]
fn the_panes_in_another_repository_offer_no_discard_or_commit_the_host_would_run_in_the_project(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    host.five_changes();
    let other = host.server.repo_with("other", &[("src/keep.ts", "theirs\n")]);
    std::fs::write(other.join("src/keep.ts"), "theirs, edited\n").unwrap();
    let ours = std::fs::read_to_string(host.folder.join("src/keep.ts")).unwrap();
    let (screen, cx) = mount(&host, cx);
    head_scope(cx, &screen);
    let other_at = other.canonicalize().unwrap().to_string_lossy().into_owned();
    let link = cx.update(|_, cx| DaemonLink::of(&host.shell(), &host.workspace_id, cx));
    cx.update(|_, cx| link.update(cx, |l, cx| l.pin(&host.thread_id, &other_at, cx)));
    until(cx, &screen, "the other repository's change listed", |s| {
        ready(s) && s.folder() == Some(other_at.as_str()) && shown(s) == ["src/keep.ts"]
    });
    cx.update(|window, cx| {
        screen.update(cx, |s, cx| {
            s.ask_discard("src/keep.ts", window, cx);
            s.discard(window, cx);
        })
    });
    until(cx, &screen, "no discard on its way", |s| s.discard_state().is_none_or(|d| !d.busy));
    settle(cx);
    assert_eq!(std::fs::read_to_string(host.folder.join("src/keep.ts")).unwrap(), ours, "the project's own edit to keep.ts is still there");
    assert!(
        cx.debug_bounds("tree-discard:src/keep.ts").is_none() && cx.debug_bounds("changes-commit").is_none(),
        "no discard or commit for a repository the host does not act in"
    );
}

#[gpui_kit::test]
fn the_head_commit_box_discard_dialog_and_cut_notice_stand_as_todays(cx: &mut TestAppContext) {
    let Some(host) = Host::start() else { return };
    host.five_changes();
    let (screen, cx) = mount(&host, cx);
    head_scope(cx, &screen);
    cx.run_until_parked();
    let at = |cx: &mut VisualTestContext, selector: &'static str| cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector}"));

    // `size-full shrink-[999]`: the crumbs take the head's slack, so the branch and the pin sit against the counts.
    let folder = cx.read(|cx| screen.read(cx).folder().map(|f| f.to_string())).expect("the pane's folder");
    let last = at(cx, leak(format!("crumb-{folder}")));
    let (crumbs, branch, pin, stat) = (at(cx, "changes-crumbs"), at(cx, "changes-branch"), at(cx, "changes-pin"), at(cx, "changes-stat"));
    assert!(last.right() < crumbs.right() - px(8.), "the path ends inside its box, the slack after it: {last:?} in {crumbs:?}");
    assert_eq!(branch.left() - crumbs.right(), px(8.), "the branch right after the crumbs' box");
    let slack = stat.left() - px(8.) - pin.right();
    assert!(slack.abs() < px(0.01), "no room between the pin and the counts: {slack:?}");

    // `min-h-24` on the edged span: 96 px with the edge.
    cx.update(|window, cx| screen.update(cx, |s, cx| s.open_commit(window, cx)));
    cx.run_until_parked();
    assert_eq!(at(cx, "text-area").size.height, px(96.), "the commit message's box");
    cx.update(|_, cx| screen.update(cx, |s, cx| s.cancel_commit(cx)));

    // 16 + 20 + 4 above the body's `pt-1`, two 18 px lines, the foot's 12 + 32 + 16, and the 1 px edge twice.
    cx.update(|_, cx| cx.set_reduce_motion(true));
    cx.update(|window, cx| screen.update(cx, |s, cx| s.ask_discard("src/added.ts", window, cx)));
    cx.run_until_parked();
    let (dialog, title, body) = (at(cx, "alert-dialog"), at(cx, "dialog-title"), at(cx, "refusal"));
    assert_eq!(body.top() - title.bottom(), px(8.), "the title's `pb-1` and the body's `pt-1`");
    assert_eq!(dialog.size.height, px(142.), "the discard dialog's height, edge included");
    cx.update(|window, cx| screen.update(cx, |s, cx| s.close_discard(window, cx)));
    cx.run_until_parked();

    // Past the daemon's 2 MB: `py-1.5` around one 11 px line at the page's 1.5, and the edge.
    let line = "x".repeat(99) + "\n";
    std::fs::write(host.folder.join("big.txt"), line.repeat(25_000)).unwrap();
    cx.update(|window, cx| screen.update(cx, |s, cx| s.fetch(window, cx)));
    until(cx, &screen, "the cut read", |s| ready(s) && s.diff().is_some_and(|d| d.cut));
    cx.run_until_parked();
    assert_eq!(at(cx, "changes-cut").size.height, px(29.5), "the cut notice's row");

    // A head short of room cuts the path, and the cut edge fades as the crumb scroller's does.
    let src = format!("{folder}/src");
    cx.update(|_, cx| screen.update(cx, |s, cx| s.go(&src, cx)));
    until(cx, &screen, "src read", |s| ready(s) && s.folder() == Some(src.as_str()));
    cx.simulate_resize(size(px(650.), px(800.)));
    cx.run_until_parked();
    cx.update(|window, cx| _ = window.simulate_next_frame(cx));
    cx.run_until_parked();
    let (crumbs, last) = (at(cx, "changes-crumbs"), at(cx, leak(format!("crumb-{src}"))));
    assert!(last.right() > crumbs.right(), "the path runs past its box: {last:?} in {crumbs:?}");
    assert!(cx.debug_bounds("crumbs-fade-end").is_some(), "the cut end fades");
}
