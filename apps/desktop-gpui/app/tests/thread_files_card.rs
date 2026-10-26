// SPDX-License-Identifier: AGPL-3.0-only
//! The presses a reply's files reach the panes by, in the frame against a host started for this test alone: a file chip
//! opens Files at its line, the changed-files card's Open diff and its file rows open Changes on the turn's range, and
//! the finder chords open the finder over the thread.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::local_store::LocalStore;
use desktop_gpui::screens::dialogs::layer;
use desktop_gpui::screens::pr_pane::changes::model::{Kept, TurnRange};
use desktop_gpui::screens::pr_pane::files::finder::{Finder, Mode, FINDER};
use desktop_gpui::screens::pr_pane::files::Reveal;
use desktop_gpui::screens::{entry, Host, Opened, Pane, Place, Route, Screen};
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::thread_view::ThreadView;
use desktop_gpui::transcript::RowKind;
use gpui_kit::component::Root;
use gpui_kit::{prelude::*, px, size, Entity, Modifiers, TestAppContext, VisualTestContext, Window};
use serde_json::json;
use wsp_host_client::protocol::SessionEvent;
use wsp_host_client::HostClient;

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

/// Panes the frame can open, standing in for the Changes and Files screens, which read through a daemon this host has not.
macro_rules! pane {
    ($ty:ident, $pane:expr) => {
        struct $ty;
        impl Render for $ty {
            fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
                gpui_kit::div()
            }
        }
        impl Screen for $ty {
            const NAME: &'static str = "pane";
            const PLACE: Place = Place::Panel($pane);
            fn new(_: Host, _: Opened, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> Self {
                $ty
            }
        }
    };
}

pane!(ChangesPane, Pane::Changes);
pane!(FilesPane, Pane::Files);
pane!(BrowserPane, Pane::Browser);

/// The shell against `host` on its one thread, the panes stood in and the app's store under the host's own folder.
struct Opened1<'a> {
    shell: Entity<Shell>,
    view: Entity<ThreadView>,
    workspace: String,
    thread: String,
    cx: &'a mut VisualTestContext,
}

fn open<'a>(host: &common::Throwaway, cx: &'a mut TestAppContext) -> Opened1<'a> {
    desktop_gpui::screens::add(Pane::Changes.name(), entry::<ChangesPane>());
    desktop_gpui::screens::add(Pane::Files.name(), entry::<FilesPane>());
    desktop_gpui::screens::add(Pane::Browser.name(), entry::<BrowserPane>());
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let t = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread");
    let (workspace, thread) = (t.workspace_id.clone(), t.thread_id.clone().expect("a thread id"));
    cx.executor().allow_parking();
    let store = host.state().join("gpui-store.json");
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
        desktop_gpui::screens::palette::init(cx);
        desktop_gpui::local_store::init(store, cx);
    });
    let (state, vars) = (host.state(), host.env());
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, vars, runtime.clone(), Launch::default(), window, cx));
        shell = Some(view.clone());
        Root::new(view, window, cx)
    });
    let shell = shell.unwrap();
    cx.simulate_resize(size(px(1280.), px(800.)));
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: workspace.clone(), thread_id: thread.clone() }, window, cx));
    until(cx, "the thread", |cx| shell.read_with(cx, |s, cx| s.thread_screen().is_some_and(|t| t.read(cx).view().read(cx).loaded())));
    let view = shell.read_with(cx, |s, cx| s.thread_screen().expect("the thread screen").read(cx).view().clone());
    Opened1 { shell, view, workspace, thread, cx }
}

fn event(thread: &str, workspace: &str, kind: &str, pos: u64, extra: serde_json::Value) -> SessionEvent {
    let mut v = json!({"type": kind, "workspaceId": workspace, "sessionId": "s", "threadId": thread, "turnId": "u1", "pos": pos});
    v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}

/// A settled turn in `folder` whose reply names `src/a.rs:12` and that changed two files, and the folder's other file.
fn turn(thread: &str, workspace: &str, folder: &str) -> Vec<SessionEvent> {
    let e = |kind: &str, pos: u64, extra: serde_json::Value| event(thread, workspace, kind, pos, extra);
    vec![
        e("session.start", 1, json!({"prompt": "first", "at": 1_000.0, "cwd": folder})),
        e("session.delta", 2, json!({"kind": "text", "text": "Fixed in `src/a.rs:12`.", "messageId": "m1"})),
        e("session.done", 3, json!({"result": {"status": "completed", "durationMs": 2_000}})),
        e("session.end", 4, json!({"exitCode": 0, "sawResult": true})),
        e(
            "session.changes",
            5,
            json!({"from": "a1", "to": "b2", "moved": [], "files": [
                {"path": "src/a.rs", "kind": "modified", "additions": 3, "deletions": 1},
                {"path": "b.rs", "kind": "added", "additions": 9, "deletions": 0}
            ], "others": [{"path": "docs/NOTES.md", "kind": "modified", "additions": 1, "deletions": 1}]}),
        ),
    ]
}

fn press(cx: &mut VisualTestContext, name: &'static str) {
    let at = cx.debug_bounds(name).unwrap_or_else(|| panic!("{name} was not drawn")).center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

fn active(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<Pane> {
    shell.read_with(cx, |s, cx| s.panel().read(cx).active())
}

fn kept_turn(o: &mut Opened1<'_>) -> Option<TurnRange> {
    let (workspace, thread) = (o.workspace.clone(), o.thread.clone());
    o.cx.update(|_, cx| Kept::read(cx.global::<LocalStore>()).turn(&workspace, &thread).cloned())
}

#[gpui_kit::test]
fn a_chip_opens_files_at_its_line_and_the_cards_presses_open_changes_on_the_turn(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let mut o = open(&host, cx);
    let root = folder.to_string_lossy().into_owned();
    let events = turn(&o.thread, &o.workspace, &root);
    o.view.update(o.cx, |v, cx| v.load(events, cx));
    until(o.cx, "the card", |cx| cx.debug_bounds("changed-files").is_some() && cx.debug_bounds("file-chip").is_some());
    assert!(o.view.read_with(o.cx, |v, _| v.rows().iter().any(|r| matches!(r.kind, RowKind::ChangedFiles { .. }))));

    press(o.cx, "file-chip");
    let reveal = o.cx.update(|_, cx| cx.try_global::<Reveal>().cloned());
    let (path, line) = reveal.map(|r| (r.path, r.line)).expect("the chip asked Files for nothing");
    assert_eq!((path.as_str(), line), (format!("{root}/src/a.rs").as_str(), Some(12)));
    assert_eq!(active(&o.shell, o.cx), Some(Pane::Files), "the chip did not open Files");

    press(o.cx, "open-diff");
    let range = kept_turn(&mut o).expect("Open diff kept no turn for the thread");
    let want = TurnRange {
        thread_id: o.thread.clone(),
        turn_id: "u1".into(),
        cwd: root.clone(),
        from: "a1".into(),
        to: "b2".into(),
        path: Some("src/a.rs".into()),
        only: Some(vec!["src/a.rs".into(), "b.rs".into()]),
    };
    assert_eq!(range, want);
    assert_eq!(active(&o.shell, o.cx), Some(Pane::Changes), "Open diff did not open Changes");

    press(o.cx, "tree-row:docs");
    press(o.cx, "tree-row:docs/NOTES.md");
    let range = kept_turn(&mut o).expect("a file row kept no turn");
    assert_eq!((range.path.as_deref(), range.only), (Some("docs/NOTES.md"), Some(vec!["docs/NOTES.md".to_owned()])));
}

/// The workspace's own folder as the host lists it (`threadFolderOf`: the copy's folder, else the project's).
fn thread_folder(host: &common::Throwaway, workspace: &str) -> String {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        let list: wsp_host_client::protocol::WorkspacesListRequest = serde_json::from_value(json!({})).unwrap();
        let listed = host.client().await.call(list).await.unwrap_or_else(|f| panic!("workspaces.list: {}", f.message));
        let all = serde_json::to_value(listed.workspaces).unwrap();
        let record = all.as_array().unwrap().iter().find(|w| w["id"] == workspace).cloned().expect("the workspace's record");
        record["folder"].as_str().or(record["project"]["path"].as_str()).expect("a folder").to_owned()
    })
}

/// A settled turn whose start names no folder, that edited `src/a.rs` and `c.rs` one call each, and whose changes hold
/// `src/a.rs` and `b.rs` only.
fn edited(thread: &str, workspace: &str) -> Vec<SessionEvent> {
    let e = |kind: &str, pos: u64, extra: serde_json::Value| event(thread, workspace, kind, pos, extra);
    let edit = |id: &str, path: &str, pos: u64| {
        let patch = json!([{"path": path, "hunks": [{"oldStart": 7, "oldLines": 1, "newStart": 7, "newLines": 1, "lines": ["-a", "+b"]}]}]);
        [
            e(
                "session.delta",
                pos,
                json!({"kind": "tool_use", "toolName": "Edit", "toolUseId": id, "text": format!("{{\"file_path\":\"{path}\"}}")}),
            ),
            e("session.delta", pos + 1, json!({"kind": "tool_result", "toolUseId": id, "text": "ok", "patch": patch})),
        ]
    };
    let mut events = vec![e("session.start", 1, json!({"prompt": "first", "at": 1_000.0}))];
    events.extend(edit("a", "src/a.rs", 2));
    events.extend(edit("c", "c.rs", 4));
    events.extend([
        e("session.delta", 6, json!({"kind": "text", "text": "Done.", "messageId": "m1"})),
        e("session.done", 7, json!({"result": {"status": "completed", "durationMs": 2_000}})),
        e("session.end", 8, json!({"exitCode": 0, "sawResult": true})),
        e(
            "session.changes",
            9,
            json!({"from": "a1", "to": "b2", "moved": [], "files": [
                {"path": "src/a.rs", "kind": "modified", "additions": 1, "deletions": 1},
                {"path": "b.rs", "kind": "added", "additions": 9, "deletions": 0}
            ]}),
        ),
    ]);
    events
}

/// Opens the turn's fold, its tool group and the call `id`.
fn open_call(view: &Entity<ThreadView>, id: &str, cx: &mut VisualTestContext) {
    for key in ["u1", "group:tool:u1:a", &format!("u1:call:{id}")] {
        view.update(cx, |v, cx| v.toggle(key, cx));
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn an_edits_open_diff_opens_changes_on_its_turn_and_a_file_the_turn_kept_no_change_of_in_files(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let mut o = open(&host, cx);
    let root = thread_folder(&host, &o.workspace);
    let events = edited(&o.thread, &o.workspace);
    o.view.update(o.cx, |v, cx| v.load(events, cx));
    until(o.cx, "the card", |cx| cx.debug_bounds("changed-files").is_some());

    open_call(&o.view, "a", o.cx);
    press(o.cx, "edit-open-diff");
    let range = kept_turn(&mut o).expect("the edit's Open diff kept no turn");
    assert_eq!(range.cwd, root, "a thread with no folder of its own read the turn against no folder");
    // A turn with no other files in its folder opens its whole range (`onlyOf`).
    assert_eq!((range.path.as_deref(), range.only), (Some("src/a.rs"), None));
    assert_eq!(active(&o.shell, o.cx), Some(Pane::Changes));

    for key in ["u1:call:a", "u1:call:c"] {
        o.view.update(o.cx, |v, cx| v.toggle(key, cx));
        o.cx.run_until_parked();
    }
    press(o.cx, "edit-open-diff");
    let reveal =
        o.cx.update(|_, cx| cx.try_global::<Reveal>().cloned()).expect("Open diff on a file the turn did not change opened no file");
    assert_eq!((reveal.path, reveal.line), (format!("{root}/c.rs"), Some(7)));
    assert_eq!(active(&o.shell, o.cx), Some(Pane::Files));
}

#[gpui_kit::test]
fn a_changed_file_opens_by_its_name_from_the_top_of_the_checkout_whatever_folder_the_thread_worked_in(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let mut o = open(&host, cx);
    let sub = format!("{}/sub", folder.to_string_lossy());
    let (thread, workspace) = (o.thread.clone(), o.workspace.clone());
    let e = |kind: &str, pos: u64, extra: serde_json::Value| event(&thread, &workspace, kind, pos, extra);
    let events = vec![
        e("session.start", 1, json!({"prompt": "first", "at": 1_000.0, "cwd": sub})),
        e("session.delta", 2, json!({"kind": "text", "text": "Done.", "messageId": "m1"})),
        e("session.done", 3, json!({"result": {"status": "completed", "durationMs": 2_000}})),
        e("session.end", 4, json!({"exitCode": 0, "sawResult": true})),
        e(
            "session.changes",
            5,
            json!({"from": "a1", "to": "b2", "moved": [], "files": [
                {"path": "a.rs", "kind": "modified", "additions": 1, "deletions": 1},
                {"path": "sub/a.rs", "kind": "modified", "additions": 1, "deletions": 1}
            ]}),
        ),
    ];
    o.view.update(o.cx, |v, cx| v.load(events, cx));
    until(o.cx, "the card", |cx| cx.debug_bounds("tree-row:a.rs").is_some());
    press(o.cx, "tree-row:a.rs");
    let range = kept_turn(&mut o).expect("the file row kept no turn");
    assert_eq!(range.path.as_deref(), Some("a.rs"), "the card's a.rs opened another file of that name");
}

/// A reply linking `href` in a settled turn.
fn linked(thread: &str, workspace: &str, href: &str) -> Vec<SessionEvent> {
    let e = |kind: &str, pos: u64, extra: serde_json::Value| event(thread, workspace, kind, pos, extra);
    vec![
        e("session.start", 1, json!({"prompt": "first", "at": 1_000.0})),
        e("session.delta", 2, json!({"kind": "text", "text": format!("[app]({href})"), "messageId": "m1"})),
        e("session.done", 3, json!({"result": {"status": "completed", "durationMs": 2_000}})),
        e("session.end", 4, json!({"exitCode": 0, "sawResult": true})),
    ]
}

/// Says the workspace is a folder on a computer the person joined, as the host's record for one reads.
fn joined(o: &mut Opened1<'_>) {
    let workspace = o.workspace.clone();
    let sidebar = o.shell.read_with(o.cx, |s, _| s.sidebar().clone());
    until(o.cx, "the sidebar's read of the workspace", |cx| sidebar.read_with(cx, |s, _| s.workspace(&workspace).0.is_some()));
    o.shell.update(o.cx, |s, cx| {
        s.sidebar().update(cx, |sidebar, cx| {
            let mut record = sidebar.workspace(&workspace).0.cloned().expect("the workspace's record");
            record["kind"] = json!("place");
            sidebar.apply(desktop_gpui::screens::sidebar::host::Change::Workspaces(vec![record]), cx);
        })
    });
    o.cx.run_until_parked();
}

/// The link's first glyph, the reply being the transcript's first agent row.
fn press_link(o: &mut Opened1<'_>) {
    let reply = o.view.read_with(o.cx, |v, _| v.rows().iter().position(|r| matches!(r.kind, RowKind::Agent { .. })).expect("the reply"));
    let row: &'static str = Box::leak(format!("row-{reply}").into_boxed_str());
    until(o.cx, "the reply", |cx| cx.debug_bounds(row).is_some());
    // Past the globe the link draws before its text, inside the reply's `px-1`.
    let at = o.cx.debug_bounds(row).expect("the reply's row").origin + gpui_kit::point(px(30.), px(12.));
    o.cx.simulate_click(at, Modifiers::none());
    o.cx.run_until_parked();
}

fn browser_tab(o: &mut Opened1<'_>) -> Option<desktop_gpui::screens::browser::url::Place> {
    let key = desktop_gpui::screens::browser::store::Key { thread: o.thread.clone().into(), tab: "browser:p1".into() };
    o.cx.update(|_, cx| {
        let store = desktop_gpui::screens::browser::store::Store::get(cx);
        let place = store.read(cx).entry(&key).and_then(|e| e.tab.current().cloned());
        place
    })
}

#[gpui_kit::test]
fn a_link_to_a_port_on_a_joined_computer_opens_in_a_browser_tab(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let mut o = open(&host, cx);
    joined(&mut o);
    let events = linked(&o.thread, &o.workspace, "http://localhost:5173/about?x=1");
    o.view.update(o.cx, |v, cx| v.load(events, cx));
    press_link(&mut o);
    let place = browser_tab(&mut o).expect("the port link opened no Browser tab");
    assert_eq!(desktop_gpui::screens::browser::url::place_url(&place), "http://localhost:5173/about?x=1");
    assert_eq!(active(&o.shell, o.cx), Some(Pane::Browser), "the Browser pane did not open on the tab");
}

#[gpui_kit::test]
fn a_port_link_in_a_thread_on_this_computer_opens_no_browser_tab(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let mut o = open(&host, cx);
    let events = linked(&o.thread, &o.workspace, "http://localhost:5173/");
    o.view.update(o.cx, |v, cx| v.load(events, cx));
    press_link(&mut o);
    assert_eq!(browser_tab(&mut o), None, "a link in a thread on this computer opened a Browser tab");
}

fn finder_mode(cx: &mut VisualTestContext) -> Option<Mode> {
    cx.update(|_, cx| layer::view(FINDER, cx).and_then(|v| v.downcast::<Finder>().ok()).map(|f| f.read(cx).mode()))
}

#[gpui_kit::test]
fn the_finder_chords_open_it_over_the_thread_and_never_from_under_settings(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let o = open(&host, cx);
    o.cx.simulate_keystrokes("secondary-p");
    o.cx.run_until_parked();
    assert_eq!(finder_mode(o.cx), Some(Mode::Files), "mod+p opened no finder over the thread");
    o.cx.simulate_keystrokes("escape");
    o.cx.run_until_parked();
    assert_eq!(finder_mode(o.cx), None, "Escape left the finder open");
    o.cx.simulate_keystrokes("secondary-shift-f");
    o.cx.run_until_parked();
    assert_eq!(finder_mode(o.cx), Some(Mode::Text), "mod+shift+f opened no search in files");
    o.cx.simulate_keystrokes("escape");
    o.cx.run_until_parked();
    o.shell.update_in(o.cx, |s, window, cx| s.go(Route::Screen(desktop_gpui::shell::SETTINGS.into()), window, cx));
    o.cx.run_until_parked();
    o.cx.simulate_keystrokes("secondary-p");
    o.cx.run_until_parked();
    assert_eq!(finder_mode(o.cx), None, "mod+p on Settings opened the finder over the thread behind it");
}
