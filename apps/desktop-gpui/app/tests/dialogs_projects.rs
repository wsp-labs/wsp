// SPDX-License-Identifier: AGPL-3.0-only
//! Add a project and Export project on the dialogs' layer, against a host started for these tests alone: a repo found
//! under a typed path and added, a folder that is no repo refused in the foot, an export into a folder that is there
//! refused with the one follow-up, and Escape and the scrim on both.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{shared, Throwaway};
use desktop_gpui::screens::dialogs::add_project::{self, AddProject};
use desktop_gpui::screens::dialogs::export_project::{self, Phase};
use desktop_gpui::screens::dialogs::layer::{self, Closes};
use desktop_gpui::screens::dialogs::{ADD_PROJECT, EXPORT_PROJECT};
use desktop_gpui::screens::Host;
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{div, point, prelude::*, px, size, App, Context, Entity, Modifiers, TestAppContext, VisualTestContext, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol::ProjectsListRequest;
use wsp_host_client::HostClient;

/// The window as the shell draws it: a page with the layer over it.
struct Stage;

impl Render for Stage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().children(layer::draw(window, cx))
    }
}

fn window(cx: &mut TestAppContext) -> &mut VisualTestContext {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(cx.new(|_| Stage), window, cx));
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    cx
}

fn connect(host: &Throwaway) -> Host {
    let rt = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = rt.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    Host::new(Arc::new(client), rt)
}

fn wait(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < until, "never: {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn top(cx: &mut VisualTestContext) -> Option<&'static str> {
    cx.update(|_, cx| layer::top(cx))
}

/// What `open` stood on the layer.
fn opened<V: 'static>(cx: &mut VisualTestContext, open: impl FnOnce(&mut Window, &mut App) -> Entity<V>) -> Entity<V> {
    cx.update(open)
}

fn projects(host: &Host) -> Vec<Value> {
    let listed = host.client().clone();
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async move {
        let listed = listed.call(common::ask::<ProjectsListRequest>(json!({}))).await.unwrap().projects;
        listed.iter().map(|p| serde_json::to_value(p).unwrap()).collect()
    })
}

/// Catches: a typed path not listing its folder, the lit repo not added on Enter, the dialog not closing once added.
#[gpui_kit::test]
fn a_repo_under_a_typed_path_is_added_on_enter(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let repo = host.repo("alpha");
    let handle = connect(&host);
    let cx = window(cx);
    let view = opened(cx, |window, cx| add_project::open(&handle, window, cx));
    let stem = format!("{}/alp", host.dir.path().display());
    view.update_in(cx, |v, window, cx| v.set_field(&stem, window, cx));
    let path = repo.to_string_lossy().into_owned();
    wait(cx, "the repo listed under the typed folder", |cx| view.read_with(cx, |v, cx| v.rows(cx) == [path.clone()]));
    cx.simulate_keystrokes("enter");
    wait(cx, "the dialog closing once added", |cx| top(cx).is_none());
    // The host records a folder through its links (`folderNamed`), as a Mac's /var is /private/var.
    let recorded = std::fs::canonicalize(&repo).unwrap().to_string_lossy().into_owned();
    assert!(projects(&handle).iter().any(|p| p["path"] == recorded.as_str()), "the host records {recorded}");
}

/// Catches: Tab moving focus out of the field instead of completing the lit row into it, as the web's Tab does.
#[gpui_kit::test]
fn tab_completes_the_lit_folder_into_the_field(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let repo = host.repo("alpha");
    let handle = connect(&host);
    let cx = window(cx);
    let view = opened(cx, |window, cx| add_project::open(&handle, window, cx));
    let stem = format!("{}/alp", host.dir.path().display());
    view.update_in(cx, |v, window, cx| v.set_field(&stem, window, cx));
    let path = repo.to_string_lossy().into_owned();
    wait(cx, "the repo listed under the typed folder", |cx| view.read_with(cx, |v, cx| v.rows(cx) == [path.clone()]));
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    let typed = view.read_with(cx, |v, cx| v.typed_value(cx));
    assert!(typed.ends_with("/alpha/"), "Tab walks into the lit folder, the field reads {typed:?}");
    assert_eq!(top(cx), Some(ADD_PROJECT));
}

/// Catches: a folder the picker gave not sent as it stands; one the host refuses closing the dialog, or its refusal not
/// said in the foot. The host takes a folder that is no repo as a project (`projects.add` answers it); it refuses the
/// same folder a second time, naming the project.
#[gpui_kit::test]
fn a_folder_the_host_refuses_is_said_in_the_foot(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let plain = host.dir.path().join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let handle = connect(&host);
    let cx = window(cx);
    let add = |cx: &mut VisualTestContext| {
        let view = opened(cx, |window, cx| add_project::open(&handle, window, cx));
        view.update_in(cx, |v, window, cx| v.add_folder(&plain.to_string_lossy(), window, cx));
        wait(cx, "the host's answer", |cx| view.read_with(cx, |v, _| !v.adding()));
        view.read_with(cx, |v, _| v.refusal().map(str::to_owned))
    };
    assert_eq!(add(cx), None, "the host takes a folder that is no repo");
    assert_eq!(top(cx), None, "an added folder closes the dialog");
    let said = add(cx).expect("the same folder a second time is refused");
    println!("projects.add {} again: {said}", plain.display());
    let refusal = cx.debug_bounds("add-project-refusal").expect("the refusal stands in the foot");
    cx.simulate_mouse_move(refusal.center(), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    assert!(cx.debug_bounds("tooltip").is_some(), "its hover holds the whole sentence, which one line may cut");
    assert_eq!(top(cx), Some(ADD_PROJECT), "a refusal leaves the dialog open");
    let recorded = std::fs::canonicalize(&plain).unwrap().to_string_lossy().into_owned();
    assert_eq!(projects(&handle).iter().filter(|p| p["path"] == recorded.as_str()).count(), 1);
}

/// Catches: the field swallowing Escape, a press on the scrim not closing Add a project, or a press inside it closing it.
#[gpui_kit::test]
fn escape_and_the_scrim_close_add_a_project(cx: &mut TestAppContext) {
    let cx = window(cx);
    let open = |cx: &mut VisualTestContext| {
        let view = opened(cx, |window, cx| {
            let view = cx.new(|cx| AddProject::staged(Vec::new(), None, &[], window, cx));
            layer::open(ADD_PROJECT, view.clone().into(), Closes::Outside, None, window, cx);
            view.update(cx, |v, cx| v.focus(window, cx));
            view
        });
        cx.run_until_parked();
        view
    };
    open(cx);
    cx.simulate_keystrokes("escape");
    assert_eq!(top(cx), None, "Escape closes it");
    open(cx);
    let dialog = cx.debug_bounds("add-project").expect("the dialog is drawn");
    cx.simulate_click(dialog.center(), Modifiers::none());
    assert_eq!(top(cx), Some(ADD_PROJECT), "a press inside it is its own");
    cx.simulate_click(point(px(8.), px(8.)), Modifiers::none());
    assert_eq!(top(cx), None, "a press on the scrim closes it");
}

/// Catches: a running export not holding its dialog against Escape and the scrim, an export into a folder that is there
/// not refused with Replace and export offered, the host's refusal of an
/// export into a fresh folder taken for the one that asks for a replace, or Escape not closing the dialog
/// once no export runs.
#[gpui_kit::test]
fn an_export_into_a_folder_that_is_there_offers_to_replace_it(cx: &mut TestAppContext) {
    let shared = shared();
    let handle = connect(&shared.host);
    let workspace = handle.client().clone();
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let first = rt.block_on(async move { common::first(&workspace).await });
    let into = tempfile::tempdir().unwrap();
    let source = shared.folder.to_string_lossy().into_owned();
    let there = into.path().join(export_project::folder_name(&source));
    std::fs::create_dir_all(&there).unwrap();
    std::fs::write(there.join("kept.txt"), "mine").unwrap();
    let cx = window(cx);
    let export = cx.update(|window, cx| export_project::open(&handle, first.workspace_id.clone(), window, cx));
    wait(cx, "the workspace's folder", |cx| export.read_with(cx, |v, cx| !v.source(cx).is_empty()));
    export.update_in(cx, |v, window, cx| {
        v.set_source(&source, window, cx);
        v.pick(&into.path().to_string_lossy(), cx);
        v.start(window, cx);
    });
    wait(cx, "the host's answer", |cx| export.read_with(cx, |v, _| v.phase() != Phase::Exporting));
    let refusal = export.read_with(cx, |v, _| v.refusal().cloned());
    println!("project.export into {}: {refusal:?}", there.display());
    let refusal = refusal.expect("a folder that is there is refused");
    assert!(refusal.exists, "the refusal is the one that asks for a replace: {}", refusal.message);
    assert!(refusal.message.contains(&there.to_string_lossy().into_owned()), "it names the folder: {}", refusal.message);
    assert_eq!(std::fs::read_to_string(there.join("kept.txt")).unwrap(), "mine", "nothing was written over it");

    let fresh = tempfile::tempdir().unwrap();
    let standing = export.update_in(cx, |v, window, cx| {
        v.pick(&fresh.path().to_string_lossy(), cx);
        v.start(window, cx);
        // What Escape and a press on the scrim do, while the export runs.
        layer::dismiss(window, cx);
        layer::top(cx)
    });
    assert_eq!(standing, Some(EXPORT_PROJECT), "a running export holds its dialog against Escape and the scrim");
    wait(cx, "the export's end", |cx| export.read_with(cx, |v, _| v.phase() != Phase::Exporting));
    let (phase, result, refusal) = export.read_with(cx, |v, _| (v.phase(), v.result().cloned(), v.refusal().cloned()));
    println!("project.export into {}: {phase:?} {result:?} {refusal:?}", fresh.path().display());
    // The shared host's workspace runs here, with no machine to download from: the host refuses it in its own words.
    let refusal = refusal.expect("the host answers the export of a workspace on this computer with a refusal");
    assert!((phase, result.is_none(), refusal.exists) == (Phase::Idle, true, false), "{refusal:?}");
    assert!(std::fs::read_dir(fresh.path()).unwrap().next().is_none(), "nothing landed in {}", fresh.path().display());
    cx.simulate_keystrokes("escape");
    assert_eq!(top(cx), None, "Escape closes it when no export runs");
}

/// Catches: Export standing past a window narrower than its 440 px popup (`max-sm` makes it a sheet along the foot),
/// its acts not stacked, or the room above the sheet not closing it as the scrim does.
#[gpui_kit::test]
fn export_in_a_400_px_window_is_a_sheet_inside_it(cx: &mut TestAppContext) {
    let cx = window(cx);
    cx.simulate_resize(size(px(400.), px(700.)));
    cx.update(|window, cx| {
        let view = cx.new(|cx| export_project::ExportProject::staged("fix-login", "Studio", "/root/wsp", &["claude"], window, cx));
        layer::open(EXPORT_PROJECT, view.into(), Closes::Outside, None, window, cx);
    });
    // Past the sheet's 200 ms rise.
    std::thread::sleep(Duration::from_millis(250));
    cx.update(|window, cx| drop(window.draw(cx)));
    cx.run_until_parked();
    let sheet = cx.debug_bounds("alert-dialog").expect("Export is drawn");
    assert!(sheet.origin.x >= px(0.) && sheet.right() <= px(400.), "no part stands outside the window: {sheet:?}");
    assert_eq!((sheet.origin.x, sheet.size.width, sheet.bottom()), (px(0.), px(400.), px(700.)), "a sheet along the foot");
    cx.simulate_click(point(px(200.), px(20.)), Modifiers::none());
    assert_eq!(top(cx), None, "a press on the page above the sheet closes it");
}
