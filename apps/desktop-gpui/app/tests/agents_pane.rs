// SPDX-License-Identifier: AGPL-3.0-only
//! The Agents pane in the app's own frame: the shell connected to a host started for these tests alone, its right
//! panel open on the pane on the host's first thread, which the registry builds under the Agents pane's name; the
//! computer's agents once `agents.read` answers; and an act's word said as a toast where no Settings screen stands.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::cell::RefCell;
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::agents_pane::AgentsPane;
use desktop_gpui::screens::app_shell::vibrancy;
use desktop_gpui::screens::settings::pages::agents::{model::w, Tab};
use desktop_gpui::screens::{self, entry, Entry, Host, Mounted, Opened, Pane, Place, Route};
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::ui::{self, NoticeKind, Side};
use gpui_kit::{prelude::*, px, size, App, Entity, Modifiers, TestAppContext, VisualTestContext, Window};

const WAIT: Duration = Duration::from_secs(20);

thread_local! {
    static MOUNTED: RefCell<Option<Entity<AgentsPane>>> = const { RefCell::new(None) };
}

fn mount_and_note(host: &Host, opened: Opened, window: &mut Window, cx: &mut App) -> Mounted {
    let mounted = (entry::<AgentsPane>().mount)(host, opened, window, cx);
    MOUNTED.with(|m| *m.borrow_mut() = mounted.view.clone().downcast::<AgentsPane>().ok());
    mounted
}

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let start = Instant::now();
    while !done(cx) {
        assert!(start.elapsed() < WAIT, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
        cx.run_until_parked();
    }
}

/// The shell on the shared host in a 1280 by 800 window, on its first thread, the panel open on the Agents pane.
fn open(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
    let registered = screens::screen(Pane::Agents.name()).expect("the Agents pane registers under its pane's name");
    assert_eq!(registered.place, Place::Panel(Pane::Agents));
    screens::add("agents", Entry { place: Place::Panel(Pane::Agents), mount: mount_and_note, closed: registered.closed });
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        screens::palette::init(cx);
    });
    let shared = common::shared();
    let first = {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async { common::first(&shared.host.client().await).await })
    };
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap());
    let (state, env) = (shared.host.state(), shared.host.env());
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
        shell = Some(view.clone());
        vibrancy::clear_root(view, window, cx)
    });
    let shell = shell.unwrap();
    cx.simulate_resize(size(px(1280.), px(800.)));
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    shell.update_in(cx, |s, window, cx| {
        s.go(Route::Thread { workspace_id: first.workspace_id, thread_id: first.thread_id }, window, cx);
        s.panel().update(cx, |p, cx| p.add(Pane::Agents, None, window, cx));
    });
    cx.run_until_parked();
    (shell, cx)
}

fn pane() -> Entity<AgentsPane> {
    MOUNTED.with(|m| m.borrow().clone()).expect("the panel built the Agents pane")
}

/// Catches: Agents in the panel's picker drawing "Nothing open here" because no screen answers its name, or the pane
/// mounting without its tabs bar and body.
#[gpui_kit::test]
fn opening_agents_on_a_thread_mounts_the_pane_with_its_tabs(cx: &mut TestAppContext) {
    let (_shell, cx) = open(cx);
    assert!(cx.debug_bounds("pane-empty").is_none(), "the panel draws a screen, not its empty state");
    let pane_box = cx.debug_bounds("agents-pane").expect("the Agents pane draws");
    let tabs = cx.debug_bounds("segmented").expect("the tabs bar");
    assert_eq!((tabs.left() - pane_box.left(), pane_box.right() - tabs.right()), (px(16.), px(16.)), "p-4 and w-full");
    assert_eq!(tabs.top() - pane_box.top(), px(16.));
    assert_eq!(cx.update(|_, cx| pane().read(cx).tab()), Tab::Agents);
}

/// Catches: the pane reading no computer, or reading it and not drawing Settings' agent rows once the host answers.
#[gpui_kit::test]
fn the_computers_agents_draw_once_the_host_answers_its_read(cx: &mut TestAppContext) {
    let (_shell, cx) = open(cx);
    let pane = pane();
    let place = |cx: &mut VisualTestContext| cx.update(|_, cx| pane.read(cx).place(cx).map(|p| p.id));
    until(cx, "the sidebar to read the thread's workspace", |cx| place(cx).is_some());
    let place = place(cx);
    assert_eq!(place.as_deref(), Some("here"), "a thread in this computer's folder reads this computer");
    let report = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let store = pane.read(cx).store().read(cx);
            store
                .reads
                .get("here")
                .and_then(|r| r.report.as_ref())
                .map(|r| r.agents.iter().map(|a| (a.id.clone(), a.installed)).collect::<Vec<_>>())
        })
    };
    until(cx, "this computer's agents", |cx| report(cx).is_some());
    let agents = report(cx).unwrap();
    assert!(agents.contains(&("claude".to_owned(), true)), "the host's stand-in claude, installed: {agents:?}");
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(cx.debug_bounds("row-skeleton").is_none(), "the read is over");
    let body = cx.debug_bounds("agents-pane-body").expect("the pane's body");
    let row = cx.debug_bounds("row").expect("an agent's row in the On card");
    assert!(body.contains(&row.origin), "{row:?} in {body:?}");
    let head = cx.debug_bounds("section-head").expect("a card's head");
    assert!(body.contains(&head.origin) && head.bottom() <= row.top(), "{head:?} over {row:?}");
    assert_eq!(row.left() - body.left(), px(17.), "px-4, then the card's 1 px edge");
}

/// Catches: an act raised in the pane whose word goes to a Settings screen that is not there, said nowhere.
#[gpui_kit::test]
fn an_acts_word_is_a_toast_where_no_settings_screen_stands(cx: &mut TestAppContext) {
    let (_shell, cx) = open(cx);
    let pane = pane();
    cx.update(|_, cx| {
        let acts = pane.read(cx).store().read(cx).acts().clone();
        acts.failed("The host said no.", cx);
        acts.done("Copied.", cx);
    });
    cx.run_until_parked();
    let said: Vec<(NoticeKind, String)> = cx.update(|window, cx| {
        ui::toast::of(window, cx).expect("the window's toasts").read(cx).standing().iter().map(|n| (n.kind, n.text.to_string())).collect()
    });
    assert!(said.contains(&(NoticeKind::Error, "The host said no.".to_owned())), "{said:?}");
    assert!(said.contains(&(NoticeKind::Done, "Copied.".to_owned())), "{said:?}");
}

/// Catches: the tabs bar not switching the body, or the other tabs not drawing Settings' own list for the computer.
#[gpui_kit::test]
fn picking_plugins_draws_settings_plugins_list_in_the_pane(cx: &mut TestAppContext) {
    let (_shell, cx) = open(cx);
    let pane = pane();
    assert!(cx.debug_bounds("kind-toolbar").is_none(), "the Agents tab draws no kind's search");
    pick_tab(&pane, 3, cx);
    let (toolbar, body) = (cx.debug_bounds("kind-toolbar").expect("the plugins' search"), cx.debug_bounds("agents-pane-body").unwrap());
    assert!(body.contains(&toolbar.origin), "{toolbar:?} in {body:?}");
}

/// Waits for this computer's report, then picks the tab at `ix` in the bar, whose segments share its width alike.
fn pick_tab(pane: &Entity<AgentsPane>, ix: usize, cx: &mut VisualTestContext) {
    let read = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| pane.read(cx).store().read(cx).reads.get("here").is_some_and(|r| r.report.is_some()))
    };
    until(cx, "this computer's report", read);
    let bar = cx.debug_bounds("segmented").expect("the tabs bar");
    let x = bar.left() + bar.size.width * ((ix as f32 + 0.5) / Tab::ALL.len() as f32);
    cx.simulate_click(gpui_kit::point(x, bar.center().y), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| pane.read(cx).tab()), Tab::ALL[ix]);
}

/// Opens the Skills tab's add page with a press on its Add, at the toolbar's right end.
fn open_add(pane: &Entity<AgentsPane>, cx: &mut VisualTestContext) {
    pick_tab(pane, 2, cx);
    let toolbar = cx.debug_bounds("kind-toolbar").expect("the skills' search and Add");
    cx.simulate_click(gpui_kit::point(toolbar.right() - px(12.), toolbar.center().y), Modifiers::none());
    cx.run_until_parked();
}

/// Catches: a row's or an Add's page move going to a Settings screen that is not there, so the press does nothing; the
/// page opening without the back and its name in the tabs' place; or the focus left where the press was.
#[gpui_kit::test]
fn a_kinds_add_opens_its_page_in_place_with_the_focus_on_its_back(cx: &mut TestAppContext) {
    let (_shell, cx) = open(cx);
    let pane = pane();
    open_add(&pane, cx);
    let name = cx.update(|_, cx| pane.read(cx).level().map(|l| l.name.to_string()));
    assert_eq!(name.as_deref(), Some(w::ADD_SKILL), "the add page stands in place of the list");
    let (top, back, page) = (
        cx.debug_bounds("agents-pane-top").unwrap(),
        cx.debug_bounds("agents-back").expect("the back in the tabs' place"),
        cx.debug_bounds("agents-page").expect("the page's name after the slash"),
    );
    assert!(
        top.contains(&back.center()) && top.contains(&page.center()) && back.right() < page.left(),
        "{back:?} then {page:?} in {top:?}"
    );
    assert!(cx.update(|window, cx| pane.read(cx).back().is_focused(window)), "the opened page's focus is on its back");
}

/// Catches: Escape on an opened page doing nothing, or leaving the page without the tab's list drawn again.
#[gpui_kit::test]
fn escape_on_an_opened_page_steps_back_to_the_list(cx: &mut TestAppContext) {
    let (_shell, cx) = open(cx);
    let pane = pane();
    open_add(&pane, cx);
    assert!(cx.update(|_, cx| pane.read(cx).level().is_some()));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(cx.update(|_, cx| pane.read(cx).level().is_none()), "Escape steps back");
    assert!(cx.debug_bounds("agents-back").is_none(), "the tabs stand again");
    assert_eq!(cx.update(|_, cx| pane.read(cx).tab()), Tab::Skills);
    assert!(cx.debug_bounds("kind-toolbar").is_some(), "the skills' list");
}

/// Catches: an agent's row in the pane pressed and nothing opening; it opens that agent's page in Settings.
#[gpui_kit::test]
fn an_agents_row_opens_its_page_in_settings(cx: &mut TestAppContext) {
    let (shell, cx) = open(cx);
    let pane = pane();
    let read = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| pane.read(cx).store().read(cx).reads.get("here").is_some_and(|r| r.report.is_some()))
    };
    until(cx, "this computer's report", read);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let body = cx.debug_bounds("agents-pane-body").expect("the pane's body");
    // The On card's first row, under its 28 px head and `gap-3`: the stand-in claude, the one agent installed there.
    cx.simulate_click(gpui_kit::point(body.left() + px(120.), body.top() + px(28. + 12. + 33.)), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| shell.read(cx).route().clone()), Route::Screen("settings:agent:claude".into()));
}
