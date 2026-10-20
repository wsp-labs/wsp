// SPDX-License-Identifier: AGPL-3.0-only
//! The header as the thread screen mounts it, in the app's shell against a host started for each test: the panes' Open
//! on the app's one road, its spinner while the host has not answered, a pick that becomes the default; the opener crumb
//! that selects the thread that opened this one; a subagent page's Back to lead.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::pr_pane::files::open_split::{self, Editor, OpenSplit};
use desktop_gpui::screens::sidebar::model::Status;
use desktop_gpui::screens::sidebar::Sidebar;
use desktop_gpui::screens::thread_view::header;
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::transcript::RowKind;
use gpui_kit::component::Root;
use gpui_kit::{
    prelude::*, px, size, Entity, Focusable as _, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, PlatformInput, Point,
    TestAppContext, VisualTestContext,
};
use wsp_host_client::{protocol, HostClient};

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

/// Each thread the host lists: its first prompt, its workspace and its id.
type Listed = Vec<(String, String, String)>;

/// The app's shell in a window against `host`, connected, and the threads the host lists by their first prompt.
fn shell_on<'a>(host: &common::Throwaway, cx: &'a mut TestAppContext) -> (Entity<Shell>, Listed, &'a mut VisualTestContext) {
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = threads
        .iter()
        .map(|t| (t.prompt.clone().unwrap_or_default(), t.workspace_id.clone(), t.thread_id.clone().expect("a thread id")))
        .collect();
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
        desktop_gpui::screens::palette::init(cx);
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
    (shell, threads, cx)
}

/// Opens `prompt`'s thread and waits until its first message is drawn and the header holds its workspace's status.
fn open(shell: &Entity<Shell>, threads: &Listed, prompt: &str, cx: &mut VisualTestContext) -> (String, String) {
    let (_, workspace_id, thread_id) = threads.iter().find(|(p, _, _)| p == prompt).unwrap_or_else(|| panic!("no thread {prompt}")).clone();
    let route = Route::Thread { workspace_id: workspace_id.clone(), thread_id: thread_id.clone() };
    shell.update_in(cx, |s, window, cx| s.go(route, window, cx));
    until(cx, prompt, |cx| {
        shell.read_with(cx, |s, cx| {
            let Some(screen) = s.thread_screen() else { return false };
            let view = screen.read(cx).view().read(cx);
            view.thread() == Some(thread_id.as_str())
                && view.rows().iter().any(|r| matches!(r.kind, RowKind::Person { .. }) && r.text == prompt)
        })
    });
    let bar = cx.update(|_, cx| header::bar(&thread_id, cx));
    until(cx, "the status row", |cx| bar.read_with(cx, |b, _| b.status.is_some()));
    (workspace_id, thread_id)
}

fn open_of(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Entity<OpenSplit> {
    shell.read_with(cx, |s, cx| s.thread_screen().map(|t| t.read(cx).open_split().clone())).expect("the thread screen")
}

/// The refusals standing in the window's toasts, oldest first.
fn refusals(cx: &mut VisualTestContext) -> Vec<String> {
    let toasts = cx.update(|window, cx| desktop_gpui::ui::toast::of(window, cx).expect("the window's toasts"));
    toasts.read_with(cx, |t, _| {
        t.standing()
            .into_iter()
            .filter(|n| n.kind == desktop_gpui::ui::NoticeKind::Error && !n.text.starts_with(READINGS))
            .map(|n| n.text.to_string())
            .collect()
    })
}

/// The toast a host with no daemon raises once the thread's readings are refused, in the stack over the header's right.
const READINGS: &str = "Live readings for ";

/// Takes the readings toast off the header's cluster, once it has stood up.
fn clear_readings(cx: &mut VisualTestContext) {
    let toasts = cx.update(|window, cx| desktop_gpui::ui::toast::of(window, cx).expect("the window's toasts"));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        cx.run_until_parked();
        let ids: Vec<u64> = toasts.read_with(cx, |t, _| {
            let store = t.store();
            store.toasts().iter().copied().filter(|id| store.get(*id).is_some_and(|s| s.notice.text.starts_with(READINGS))).collect()
        });
        if !ids.is_empty() {
            toasts.update(cx, |t, cx| ids.into_iter().for_each(|id| t.dismiss(id, cx)));
            let settle = Instant::now() + Duration::from_millis(600);
            while Instant::now() < settle {
                cx.run_until_parked();
                std::thread::sleep(Duration::from_millis(50));
            }
            return;
        }
        if Instant::now() > deadline {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A press and its release at `at`, with nothing run after it: the host's answer is not let in.
fn press_held(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.update(|window, cx| {
        let (modifiers, button) = (Modifiers::none(), MouseButton::Left);
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent { position: at, modifiers, button, click_count: 1, first_mouse: false }),
            cx,
        );
        window.dispatch_event(PlatformInput::MouseUp(MouseUpEvent { position: at, modifiers, button, click_count: 1 }), cx);
    });
}

/// Editors of the test's own, in place of the host's, once the host has listed its own.
fn editors(cx: &mut VisualTestContext, picked: Option<&str>) {
    until(cx, "the host's editors", |cx| cx.update(|_, cx| cx.try_global::<open_split::Editors>().is_some_and(|e| e.list.is_some())));
    let editor = |id: &str, name: &str| Editor { id: id.to_owned(), name: name.to_owned(), remote: false };
    let list = vec![editor("vscode", "VS Code"), editor("zed", "Zed")];
    cx.update(|_, cx| open_split::set_editors(Some(list), picked.map(str::to_owned), cx));
    clear_readings(cx);
}

#[gpui_kit::test]
fn open_wears_the_spinner_while_the_host_has_not_answered_and_a_second_press_waits(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let (shell, threads, cx) = shell_on(&host, cx);
    open(&shell, &threads, "first", cx);
    editors(cx, Some("zed"));
    assert!(cx.debug_bounds("editor-mark-zed").is_some(), "Open wears no mark of the default before a press");
    // The host refuses an open of a folder that moved away before it looks for an editor to start.
    std::fs::rename(&folder, folder.with_extension("moved")).expect("the repo's folder moved away");
    // Open's own act, with no pointer over the window to redraw it and no frame forced: the open's start redraws Open.
    let open = open_of(&shell, cx);
    open.update_in(cx, |o, window, cx| o.open_default(window, cx));
    assert!(cx.update(|window, cx| open.read(cx).is_opening(window, cx)), "the press started no open");
    let mark = cx.debug_bounds("open-mark").expect("Open's lead drawn");
    assert_eq!(mark.size, size(px(16.), px(16.)));
    assert!(cx.debug_bounds("editor-mark-zed").is_none(), "Open kept its mark while the open waits on the host, not the spinner");
    let main = cx.debug_bounds("open-main").expect("Open drawn").center();
    press_held(cx, main);
    until(cx, "the refusal", |cx| !refusals(cx).is_empty());
    let settle = Instant::now() + Duration::from_millis(800);
    while Instant::now() < settle {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(refusals(cx).len(), 1, "the second press opened again: {:?}", refusals(cx));
    assert!(refusals(cx)[0].contains("is not in this workspace's folder"), "the toast said {:?}", refusals(cx));
    assert!(cx.debug_bounds("editor-mark-zed").is_some(), "the spinner stayed once the host answered");
}

#[gpui_kit::test]
fn a_pick_in_opens_menu_becomes_the_default_on_the_host_and_opens_there(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let (shell, threads, cx) = shell_on(&host, cx);
    open(&shell, &threads, "first", cx);
    editors(cx, Some("zed"));
    std::fs::rename(&folder, folder.with_extension("moved")).expect("the repo's folder moved away");
    let choose = cx.debug_bounds("open-choose").expect("Open's chevron drawn").center();
    cx.simulate_click(choose, Modifiers::none());
    cx.run_until_parked();
    let vscode = cx.debug_bounds("open-item-vscode").expect("VS Code in the menu").center();
    press_held(cx, vscode);
    // `inFlight` holds the preference write as well as the open: a press while the default changes waits.
    let workspace = ids(&threads, "first").0;
    let waiting = cx.update(|window, cx| desktop_gpui::screens::notices::opening(&workspace, window, cx));
    assert!(waiting, "the pick's preference write is not part of the open's wait");
    until(cx, "the open's refusal", |cx| !refusals(cx).is_empty());
    assert!(refusals(cx)[0].contains("is not in this workspace's folder"), "the toast said {:?}", refusals(cx));
    let picked = cx.update(|_, cx| cx.global::<open_split::Editors>().picked.clone());
    assert_eq!(picked.as_deref(), Some("vscode"), "the pick did not become the default");
    assert!(cx.debug_bounds("editor-mark-vscode").is_some(), "Open does not wear the new default's mark");
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let kept = runtime.block_on(async {
        let get: protocol::PreferencesGetRequest = serde_json::from_value(serde_json::json!({})).unwrap();
        let got = host.client().await.call(get).await.unwrap_or_else(|f| panic!("preferences.get: {}", f.message));
        serde_json::to_value(got.preferences).unwrap()["editor"].as_str().map(str::to_owned)
    });
    assert_eq!(kept.as_deref(), Some("vscode"), "the host does not keep the pick");
}

/// `thread`'s row once the host's listing has landed in the sidebar and its turn has ended, so no push replaces it.
fn listed_row(shell: &Entity<Shell>, thread: &str, cx: &mut VisualTestContext) -> (Entity<Sidebar>, String, serde_json::Value) {
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    let row = |cx: &mut VisualTestContext| {
        sidebar.read_with(cx, |s, _| {
            s.rows()
                .iter()
                .find(|r| r.thread_id.as_deref() == Some(thread) && r.status != Status::Running)
                .map(|r| (r.id.clone(), r.as_sent()))
        })
    };
    until(cx, "the thread's row in the sidebar", |cx| row(cx).is_some());
    let (id, sent) = row(cx).unwrap();
    (sidebar, id, sent)
}

/// The thread `prompt` names, by its workspace and id.
fn ids(threads: &Listed, prompt: &str) -> (String, String) {
    let (_, workspace, thread) = threads.iter().find(|(p, _, _)| p == prompt).unwrap_or_else(|| panic!("no thread {prompt}"));
    (workspace.clone(), thread.clone())
}

#[gpui_kit::test]
fn a_press_on_the_opener_crumb_selects_the_thread_that_opened_this_one(cx: &mut TestAppContext) {
    use desktop_gpui::screens::sidebar::host::Change;
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    host.wsp(&["run", "repo", "second", "--agent", "claude"]);
    let (shell, threads, cx) = shell_on(&host, cx);
    let (workspace, first) = ids(&threads, "first");
    let (_, second) = open(&shell, &threads, "second", cx);
    // The second thread as an agent on the first would have opened it.
    let (sidebar, id, mut opened) = listed_row(&shell, &second, cx);
    opened["parentThreadId"] = first.clone().into();
    sidebar.update(cx, |s, cx| s.apply(Change::Row { id, row: Some(opened) }, cx));
    until(cx, "the opener crumb", |cx| cx.debug_bounds("crumbs-opener").is_some());
    let opener = cx.debug_bounds("crumbs-opener").unwrap().center();
    cx.simulate_click(opener, Modifiers::none());
    let lead = Route::Thread { workspace_id: workspace, thread_id: first };
    until(cx, "the opener selected", |cx| shell.read_with(cx, |s, _| s.route() == &lead));
}

#[gpui_kit::test]
fn back_to_lead_selects_the_lead_and_puts_the_caret_in_its_composer(cx: &mut TestAppContext) {
    use desktop_gpui::screens::sidebar::host::Change;
    use desktop_gpui::screens::thread_view::cards::SUBAGENT;
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let (shell, threads, cx) = shell_on(&host, cx);
    let (workspace, thread) = open(&shell, &threads, "first", cx);
    let screen = shell.read_with(cx, |s, _| s.thread_screen()).expect("the thread screen");
    let composer = screen.read_with(cx, |s, _| s.composer().clone());
    // On the row, as the host sends it: each move of the sidebar reads the thread's listing again off its rows.
    let (sidebar, id, mut row) = listed_row(&shell, &thread, cx);
    row["subagents"] = serde_json::json!([
        { "id": "survey", "title": "Survey", "state": "done", "startedAt": 1_000., "endedAt": 9_000., "parentToolUseId": "toolu-survey" }
    ]);
    sidebar.update(cx, |s, cx| s.apply(Change::Row { id, row: Some(row) }, cx));
    let page = Route::Screen(format!("{SUBAGENT}:{workspace}/{thread}/toolu-survey").into());
    shell.update_in(cx, |s, window, cx| s.go(page.clone(), window, cx));
    until(cx, "the subagent's bar", |cx| cx.debug_bounds("subagent-bar").is_some());
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), page, "the page did not stay open");
    // `useSubagentModel`: a subagent that says no model is named by its agent.
    let named = cx.debug_bounds("subagent-model-name").expect("the bar names the subagent's model");
    let agent = cx.update(|window, cx| {
        let family = gpui_kit::component::ActiveTheme::theme(cx).font_family.clone();
        desktop_gpui::ui::text::width("Claude Code", 13., family, window)
    });
    assert!(
        (named.size.width - agent).abs() < px(1.),
        "the bar names no agent where the subagent says no model: {:?} wide",
        named.size.width
    );
    let bar = cx.debug_bounds("subagent-bar").unwrap();
    // Back to lead ends the bar, past its 8 px pad.
    cx.simulate_click(gpui_kit::point(bar.right() - px(40.), bar.center().y), Modifiers::none());
    let lead = Route::Thread { workspace_id: workspace, thread_id: thread };
    until(cx, "the lead selected", |cx| shell.read_with(cx, |s, _| s.route() == &lead));
    cx.run_until_parked();
    let caret = cx.update(|window, cx| composer.read(cx).focus_handle(cx).contains_focused(window, cx));
    assert!(caret, "Back to lead left the caret out of the lead's composer");
}
