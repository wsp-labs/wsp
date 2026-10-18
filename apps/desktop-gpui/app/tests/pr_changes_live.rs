// SPDX-License-Identifier: AGPL-3.0-only
//! The Changes pane's reads and writes against a host started for each test alone (host-client's tests/common),
//! carrying a wsp-daemon that serves git.* and fs.*: the workspace's daemon link, git.diff for each scope, the viewed
//! mark and its push, discard asked first, and commit only in Uncommitted. The project holds the five changes of
//! states.mjs's `fiveChanges` (data B). Without a daemon on this computer every test says so and passes nothing.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "pr_pane/stand_in.rs"]
mod stand_in;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::local_store::LocalStore;
use desktop_gpui::screens::composer::store as drafts;
use desktop_gpui::screens::pr_pane::changes::model::{self, Scope, TurnRange};
use desktop_gpui::screens::pr_pane::changes::{words, ChangesScreen, Load};
use desktop_gpui::screens::pr_pane::daemon::DaemonLink;
use desktop_gpui::screens::pr_pane::diff::{LineRange, Side as DiffSide};
use desktop_gpui::screens::Opened;
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext as _, BorrowAppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext};
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
        Some(Host { client: Arc::new(client), runtime, workspace_id, thread_id, folder, server })
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

/// Runs one request over the link to its answer, the host's runtime doing the work.
fn ask(
    host: &Host,
    link: &Entity<DaemonLink>,
    op: &str,
    params: Value,
    cx: &mut TestAppContext,
) -> Result<serde_json::Map<String, Value>, desktop_gpui::screens::pr_pane::daemon::Refused> {
    let sent = link.read_with(cx, |link, _| link.request(op, params));
    host.runtime.block_on(async { tokio::time::timeout(Duration::from_secs(30), sent).await.expect("the daemon answered within 30 s") })
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
    let reply = ask(&host, &link, "git.diff", json!({ "cwd": folder, "scope": "head" }), cx).unwrap_or_else(|r| panic!("git.diff: {r:?}"));
    assert_eq!(paths(&reply), ["logo.bin", "src/added.ts", "src/gone.txt", "src/keep.ts", "src/renamed.ts"], "{reply:?}");
    let keep = reply["files"].as_array().unwrap().iter().find(|f| f["path"] == "src/keep.ts").unwrap();
    assert!(keep["patch"].as_str().unwrap().contains("+export const c = 4;"), "{keep}");
    let until = Instant::now() + Duration::from_secs(10);
    let root = loop {
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
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::local_store::init(host.server.dir.path().join("local-store.json"), cx);
    });
    cx.executor().allow_parking();
    let opened = Opened { workspace_id: Some(host.workspace_id.clone()), thread_id: Some(host.thread_id.clone()), tab: None, arg: None };
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| ChangesScreen::open(host.shell(), opened, window, cx));
        made = Some(screen.clone());
        Root::new(screen, window, cx)
    });
    cx.simulate_resize(size(px(1200.), px(800.)));
    (made.unwrap(), cx)
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
                "load {:?}, not running {}, folder {:?}, files {:?}, commit {commit:?}",
                s.load(),
                s.not_running(),
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

    // Past the sheet's width the panel stands inline with a 28 px head; under it the head is 40 px; under 640 px the
    // folder, branch, pin, fold all and the view leave it.
    let head = |cx: &mut VisualTestContext| cx.debug_bounds("changes-head").expect("the head").size.height;
    assert_eq!(head(cx), px(28.), "the inline panel's head");
    cx.simulate_resize(size(px(900.), px(800.)));
    cx.run_until_parked();
    assert_eq!(head(cx), px(40.), "the sheet's head");
    assert!(cx.debug_bounds("changes-branch").is_some() && cx.debug_bounds("changes-crumbs").is_some());
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

    cx.update(|window, cx| screen.update(cx, |s, cx| s.ask_discard("src/added.ts", window, cx)));
    cx.run_until_parked();
    let deletes = cx.read(|cx| screen.read(cx).discard_state().map(|d| d.deletes));
    assert_eq!(deletes, Some(true), "a new file's discard says it deletes it");
    assert!(host.folder.join("src/added.ts").exists(), "asking discards nothing");
    let title = cx.debug_bounds("dialog-title").expect("the dialog draws its title");
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
            if let Some(root) = link.read_with(cx, |l, _| l.root()) {
                return root;
            }
            assert!(Instant::now() < until, "no hello named the link's folder within 10 s");
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    let a = cx.update(|cx| DaemonLink::of(&host.shell(), &host.workspace_id, cx));
    ask(&host, &a, "ping", json!({}), cx).unwrap();
    let root_a = rooted(&a, cx);
    let b = cx.update(|cx| DaemonLink::of(&host.shell(), &second, cx));
    ask(&host, &b, "ping", json!({}), cx).unwrap();
    rooted(&b, cx);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(a.read_with(cx, |l, _| l.root()), Some(root_a), "the first link's root outlives the second link's hello");
}
