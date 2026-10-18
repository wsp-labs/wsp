// SPDX-License-Identifier: AGPL-3.0-only
//! The dialogs' layer as the shell draws it, and the sidebar's asks reaching each flow: the app's own window, the shell
//! inside gpui-kit's Root on a throwaway host.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::dialogs::{self, layer, layer::Closes};
use desktop_gpui::screens::{self, entry, Opened, Place, Route, Screen, THREAD};
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::sidebar::Asked;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::Root;
use gpui_kit::{
    div, point, prelude::*, px, size, AppContext, Context, Entity, Focusable as _, Modifiers, TestAppContext, VisualTestContext, Window,
};
use wsp_host_client::Env;

fn runtime() -> Arc<tokio::runtime::Runtime> {
    Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap())
}

fn open(cx: &mut TestAppContext, state: std::path::PathBuf, env: Env) -> (Entity<Shell>, &mut VisualTestContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
    });
    let runtime = runtime();
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
        shell = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    (shell.unwrap(), cx)
}

/// The shell on a home of its own that no host serves; the folder goes when the returned guard drops.
fn hostless(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let home = [("HOME".to_owned(), dir.path().to_string_lossy().into_owned())].into_iter().collect();
    let (shell, cx) = open(cx, dir.path().join("state.json"), home);
    (shell, cx, dir)
}

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A 200 by 120 surface with nothing in it.
struct Probe;

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().debug_selector(|| "probe".into()).w(px(200.)).h(px(120.))
    }
}

fn stand(key: &'static str, closes: Closes, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        let view = cx.new(|_| Probe).into();
        layer::open(key, view, closes, None, window, cx);
    });
    cx.run_until_parked();
}

fn top(cx: &mut VisualTestContext) -> Option<&'static str> {
    cx.update(|_, cx| layer::top(cx))
}

#[gpui_kit::test]
fn a_step_dialog_stays_on_a_press_inside_and_closes_on_the_scrim_or_escape(cx: &mut TestAppContext) {
    let (_, cx, _home) = hostless(cx);
    stand("probe", Closes::Outside, cx);
    let probe = cx.debug_bounds("probe").expect("the layer draws the dialog");
    assert_eq!(probe.center(), point(px(640.), px(400.)), "centred in the window");
    cx.simulate_click(probe.center(), Modifiers::none());
    assert_eq!(top(cx), Some("probe"), "a press inside keeps it open");
    cx.simulate_click(point(px(40.), px(400.)), Modifiers::none());
    assert_eq!(top(cx), None, "a press on the scrim closes it");

    stand("probe", Closes::Outside, cx);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(top(cx), None, "Escape closes it");
}

#[gpui_kit::test]
fn an_alert_ignores_the_scrim_and_closes_on_escape(cx: &mut TestAppContext) {
    let (_, cx, _home) = hostless(cx);
    stand("alert", Closes::Escape, cx);
    cx.simulate_click(point(px(40.), px(400.)), Modifiers::none());
    assert_eq!(top(cx), Some("alert"), "a press outside is no answer");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(top(cx), None);
}

#[gpui_kit::test]
fn the_layer_keeps_the_pointer_from_the_sidebar_under_it(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    stand("probe", Closes::Escape, cx);
    // The sidebar's trigger sits at the frame row's left; under the layer a press there must not reach it.
    let trigger = cx.debug_bounds("sidebar-column").expect("the sidebar is drawn").origin + point(px(30.), px(26.));
    cx.simulate_click(trigger, Modifiers::none());
    assert!(shell.read_with(cx, |s, _| s.sidebar_open()), "the press stopped at the layer");
    assert_eq!(top(cx), Some("probe"));
}

#[gpui_kit::test]
fn each_sidebar_ask_opens_its_flow(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    for (asked, key) in [
        (Asked::AddComputer, dialogs::ADD_COMPUTER),
        (Asked::Setup("setup-1".into()), dialogs::ADD_COMPUTER),
        (Asked::AddCloud, dialogs::ADD_CLOUD),
        (Asked::AddProject, dialogs::ADD_PROJECT),
        (Asked::ExportProject("workspace-1".into()), dialogs::EXPORT_PROJECT),
    ] {
        let said = format!("{asked:?}");
        sidebar.update(cx, |_, cx| cx.emit(asked));
        until(cx, &said, |cx| top(cx) == Some(key));
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert_eq!(top(cx), None, "Escape closes what {said} opened");
    }
}

thread_local! {
    static FIELD: std::cell::RefCell<Option<Entity<InputState>>> = const { std::cell::RefCell::new(None) };
}

/// A page holding one text field, where the composer would stand under a dialog.
struct Page;

impl Screen for Page {
    const NAME: &'static str = "field-page";
    const PLACE: Place = Place::Main;

    fn new(_: screens::Host, _: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        FIELD.with(|f| *f.borrow_mut() = Some(cx.new(|cx| InputState::new(window, cx))));
        Page
    }
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_8().child(div().w(px(300.)).child(Input::new(&field())))
    }
}

fn field() -> Entity<InputState> {
    FIELD.with(|f| f.borrow().clone()).expect("the page is mounted")
}

fn page_text(cx: &mut VisualTestContext) -> String {
    let f = field();
    cx.update(|_, cx| f.read(cx).value().to_string())
}

fn page_focused(cx: &mut VisualTestContext) -> bool {
    let f = field();
    cx.update(|window, cx| f.read(cx).focus_handle(cx).is_focused(window))
}

/// The shell on `host` with the field page on `route`, "ab" typed in its field, and Add a project opened over it.
fn under_add_project<'a>(cx: &'a mut TestAppContext, host: &common::Throwaway, route: Route) -> (Entity<Shell>, &'a mut VisualTestContext) {
    screens::add(THREAD, entry::<Page>());
    screens::add(Page::NAME, entry::<Page>());
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    shell.update_in(cx, |s, window, cx| s.go(route, window, cx));
    until(cx, "the page", |_| FIELD.with(|f| f.borrow().is_some()));
    field().update_in(cx, |f, window, cx| f.focus(window, cx));
    cx.simulate_keystrokes("a b");
    assert_eq!(page_text(cx), "ab");
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    sidebar.update(cx, |_, cx| cx.emit(Asked::AddProject));
    until(cx, "Add a project", |cx| top(cx) == Some(dialogs::ADD_PROJECT));
    (shell, cx)
}

fn a_thread() -> Route {
    Route::Thread { workspace_id: "w1".into(), thread_id: "t1".into() }
}

/// Catches: Tab walking out of the dialog to a field on the page under it, which then takes what is typed.
#[gpui_kit::test]
fn tab_stays_inside_the_dialog(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let (_, cx) = under_add_project(cx, &host, a_thread());
    for press in 1..=12 {
        cx.simulate_keystrokes("tab");
        assert!(!page_focused(cx), "Tab press {press} reached the page under the dialog");
    }
    cx.simulate_keystrokes("z");
    assert_eq!(page_text(cx), "ab");
}

/// Catches: focus left on nothing once a dialog closes, so the field it was opened over takes no more keys.
#[gpui_kit::test]
fn escape_gives_focus_back_to_where_it_stood(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let (_, cx) = under_add_project(cx, &host, a_thread());
    cx.simulate_keystrokes("x escape");
    assert_eq!(top(cx), None);
    cx.simulate_keystrokes("q");
    assert!(page_focused(cx), "focus is back on the page's field");
    assert_eq!(page_text(cx), "abq", "what was typed in the dialog stayed there");
}

/// Catches: Escape over a page route closing the page under the dialog and leaving the dialog open.
#[gpui_kit::test]
fn escape_over_a_page_closes_the_dialog_and_not_the_page(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let (shell, cx) = under_add_project(cx, &host, Route::Screen(Page::NAME.into()));
    cx.simulate_keystrokes("escape");
    assert_eq!(top(cx), None, "Escape closes the dialog");
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen(Page::NAME.into()), "and the page stays");
}

/// A dialog whose one field goes away while it holds focus, as a step that moves on drops its field.
struct Fading {
    field: Entity<InputState>,
    shown: bool,
}

impl Render for Fading {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(200.)).h(px(120.)).when(self.shown, |el| el.child(Input::new(&self.field)))
    }
}

/// Catches: Escape going nowhere once the field that held focus inside a dialog is no longer drawn.
#[gpui_kit::test]
fn escape_still_closes_a_dialog_whose_focused_field_went(cx: &mut TestAppContext) {
    let (_, cx, _home) = hostless(cx);
    let view = cx.update(|window, cx| {
        let field = cx.new(|cx| InputState::new(window, cx));
        let view = cx.new(|_| Fading { field: field.clone(), shown: true });
        layer::open("fading", view.clone().into(), Closes::Outside, None, window, cx);
        field.update(cx, |f, cx| f.focus(window, cx));
        view
    });
    cx.run_until_parked();
    view.update(cx, |v, cx| {
        v.shown = false;
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("escape");
    assert_eq!(top(cx), None);
}

/// A 200 by 120 surface found under `name`.
struct Named(&'static str);

impl Render for Named {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let name = self.0;
        div().debug_selector(move || name.into()).w(px(200.)).h(px(120.))
    }
}

fn stand_named(name: &'static str, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        let view = cx.new(|_| Named(name)).into();
        layer::open(name, view, Closes::Outside, None, window, cx);
    });
    cx.run_until_parked();
}

/// Draws once the layer's 200 ms moves have run out.
fn moved(cx: &mut VisualTestContext) {
    std::thread::sleep(Duration::from_millis(250));
    cx.update(|window, cx| drop(window.draw(cx)));
    cx.run_until_parked();
}

/// Catches: a dialog under another not drawn, not stood up 1.25rem a level, a press on it reaching it rather than
/// closing the top, or it not coming back down once the top closes.
#[gpui_kit::test]
fn a_dialog_under_another_stands_up_a_level_and_comes_back_once_it_closes(cx: &mut TestAppContext) {
    let (_, cx, _home) = hostless(cx);
    stand_named("first", cx);
    stand_named("second", cx);
    moved(cx);
    let first = cx.debug_bounds("first").expect("the dialog under the top is drawn");
    let second = cx.debug_bounds("second").expect("the top is drawn");
    assert_eq!(second.center(), point(px(640.), px(400.)));
    assert_eq!(first.center(), point(px(640.), px(380.)), "one level under, it stands 20 px up");
    cx.simulate_click(point(px(640.), px(325.)), Modifiers::none());
    assert_eq!(top(cx), Some("first"), "a press on the dialog under closes the top, as a press outside it does");
    moved(cx);
    assert_eq!(cx.debug_bounds("first").map(|b| b.center()), Some(point(px(640.), px(400.))), "back where it stood");
}

/// Catches: reduced motion dropping the dialogs' moves, which the web keeps, or a narrow window's sheet not rising in
/// from 1rem below.
#[gpui_kit::test]
fn under_reduced_motion_a_sheet_still_rises_in(cx: &mut TestAppContext) {
    let (_, cx, _home) = hostless(cx);
    cx.simulate_resize(size(px(400.), px(700.)));
    cx.update(|_, cx| cx.set_reduce_motion(true));
    stand_named("first", cx);
    let rising = cx.debug_bounds("first").expect("drawn").center().y;
    assert!(rising > px(352.) && rising <= px(366.), "it comes in from up to 16 px below: {rising:?}");
    moved(cx);
    assert_eq!(cx.debug_bounds("first").map(|b| b.center().y), Some(px(350.)));
}

/// Catches: a dialog opened again under its own name dropping the one it replaces without its close hook, so what the
/// hook ends (a sign-in run, the picks still to send, the notice on close) never happens.
#[gpui_kit::test]
fn opening_a_dialog_again_closes_the_one_it_replaces(cx: &mut TestAppContext) {
    let (_, cx, _home) = hostless(cx);
    let closed = std::rc::Rc::new(std::cell::Cell::new(0));
    let hook = {
        let closed = closed.clone();
        std::rc::Rc::new(move |_: &mut Window, _: &mut gpui_kit::App| closed.set(closed.get() + 1)) as layer::OnClose
    };
    for _ in 0..2 {
        let hook = hook.clone();
        cx.update(|window, cx| layer::open("probe", cx.new(|_| Probe).into(), Closes::Outside, Some(hook), window, cx));
        cx.run_until_parked();
    }
    assert_eq!(closed.get(), 1, "the first one's hook ran as it went");
    assert_eq!(top(cx), Some("probe"));
}

/// Catches: opening a dialog again while its work runs taking the running one down.
#[gpui_kit::test]
fn a_held_dialog_stays_when_it_is_opened_again(cx: &mut TestAppContext) {
    let (_, cx, _home) = hostless(cx);
    stand("probe", Closes::Outside, cx);
    let first = cx.update(|_, cx| layer::view("probe", cx)).expect("it stands").entity_id();
    cx.update(|_, cx| layer::hold("probe", true, cx));
    stand("probe", Closes::Outside, cx);
    assert_eq!(cx.update(|_, cx| layer::view("probe", cx)).map(|v| v.entity_id()), Some(first), "the running one stands");
}
