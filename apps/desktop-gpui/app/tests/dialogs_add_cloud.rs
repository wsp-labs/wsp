// SPDX-License-Identifier: AGPL-3.0-only
//! Add a cloud provider: its reads, a write and a push against a host started for these tests alone, and the flow's
//! own moves on fixture answers. No key ever reaches a provider: `providers.checkKey` and `init.build` call a
//! provider's API, so the check's answer is fed in as a fixture, and the one write sent names a provider the host keys
//! nothing for, which `init.keys` refuses before any check.

#[path = "dialogs/cloud.rs"]
mod cloud;
#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::dialogs::add_cloud::calls::Pushed;
use desktop_gpui::screens::dialogs::add_cloud::flow::{self, Check, Facts, Flow, RowState, Step};
use desktop_gpui::screens::dialogs::add_cloud::{self, AddCloud};
use desktop_gpui::screens::dialogs::{layer, ADD_CLOUD};
use desktop_gpui::screens::Host;
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{
    div, point, prelude::*, px, size, AnyView, App, ClipboardItem, Context, Entity, Modifiers, TestAppContext, VisualTestContext, Window,
};
use serde_json::json;
use wsp_host_client::HostClient;

/// `dialogs_shots`' state, which the fixtures file names.
pub struct State {
    pub id: &'static str,
    pub stage: fn(&mut Window, &mut App) -> AnyView,
}

impl State {
    pub fn new(id: &'static str, stage: fn(&mut Window, &mut App) -> AnyView) -> Self {
        State { id, stage }
    }
}

pub fn page(_: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_| Page).into()
}

struct Page;

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

/// An empty page with the dialogs' layer over it, as the shell draws it.
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
    let stage = cx.new(|_| Stage);
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(stage, window, cx));
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    cx
}

fn stage(flow: Flow, cx: &mut VisualTestContext) -> Entity<AddCloud> {
    let view = cx.update(|window, cx| add_cloud::open_on(None, flow, window, cx));
    cx.run_until_parked();
    view
}

fn host(throwaway: &common::Throwaway) -> Host {
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client =
        Arc::new(runtime.block_on(HostClient::connect(&throwaway.state(), &throwaway.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    Host::new(client, runtime)
}

fn until(cx: &mut VisualTestContext, what: &str, secs: u64, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn flow_of(view: &Entity<AddCloud>, cx: &mut VisualTestContext) -> Flow {
    cx.update(|_, cx| view.read(cx).flow.clone())
}

fn click(selector: &'static str, cx: &mut VisualTestContext) {
    let at = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn"));
    cx.simulate_click(at.center(), Modifiers::none());
    cx.run_until_parked();
}

fn drawn(selector: &'static str, cx: &mut VisualTestContext) -> bool {
    cx.debug_bounds(selector).is_some()
}

/// Catches: the setup never read, a fresh host's Boat read as held, the flow placed on another provider or step.
#[gpui_kit::test]
fn a_fresh_host_opens_on_boats_key_with_none_held(cx: &mut TestAppContext) {
    let throwaway = common::Throwaway::start();
    let host = host(&throwaway);
    let cx = window(cx);
    let view = cx.update(|window, cx| add_cloud::open_on(Some(&host), Flow::new(None), window, cx));
    until(cx, "init.get", 30, |cx| flow_of(&view, cx).setup.is_some());
    let flow = flow_of(&view, cx);
    assert_eq!(flow.setup.as_ref().unwrap().keys.get("box"), Some(&false), "a fresh host keys Boat with no key held");
    assert_eq!((flow.provider.as_str(), flow.step, flow.held()), ("box", Step::Key, false));
    assert_eq!(flow.head().0, "Add Boat");
    assert!(!flow.key_ready(), "Continue is held with no key typed and none held");
    until(cx, "the computer's name", 30, |cx| !flow_of(&view, cx).here.is_empty());
}

/// Catches: Continue never sending `init.keys`, the typed key left out of it, and the host's refusal not drawn.
#[gpui_kit::test]
fn continue_sends_the_key_and_draws_the_hosts_refusal(cx: &mut TestAppContext) {
    let throwaway = common::Throwaway::start();
    let host = host(&throwaway);
    let cx = window(cx);
    let mut flow = Flow::new(Some("nowhere"));
    flow.on_setup(serde_json::from_value(json!({ "keys": { "nowhere": false } })).unwrap());
    let view = cx.update(|window, cx| add_cloud::open_on(Some(&host), flow, window, cx));
    until(cx, "init.get", 30, |cx| flow_of(&view, cx).setup.is_some());
    cx.update(|window, cx| {
        view.update(cx, |v, cx| {
            v.flow.provider = "nowhere".into();
            v.flow.go(Step::Key);
            cx.notify();
        });
        let _ = window;
    });
    cx.simulate_input("fake-key");
    cx.update(|_, cx| {
        view.update(cx, |v, cx| {
            v.flow.on_check("fake-key", Ok(Facts::default()));
            cx.notify();
        })
    });
    cx.run_until_parked();
    assert!(flow_of(&view, cx).key_ready(), "a key the provider took lets Continue through");
    click("cloud-continue", cx);
    until(cx, "the host's refusal", 30, |cx| flow_of(&view, cx).refused.is_some());
    let said = flow_of(&view, cx).refused.unwrap().said;
    assert!(said.contains("not a provider this wsp takes a key for"), "{said}");
    assert!(drawn("key-refused", cx), "the refusal stands under the key");
}

/// Catches: an `init.job` push not followed, or read into the flow without moving it off its spinner.
#[gpui_kit::test]
fn a_pushed_job_moves_the_picks_off_their_spinner(cx: &mut TestAppContext) {
    let throwaway = common::Throwaway::start();
    let host = host(&throwaway);
    let cx = window(cx);
    let mut flow = Flow::new(Some("here"));
    flow.go(Step::Pick(0));
    let view = cx.update(|window, cx| add_cloud::open_on(Some(&host), flow, window, cx));
    until(cx, "init.get", 30, |cx| flow_of(&view, cx).setup.is_some());
    assert!(drawn("options-loading", cx), "the picks wait on the reading");
    let state = throwaway.state();
    let env = throwaway.env();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let client = HostClient::connect(&state, &env).await.unwrap();
            let start: wsp_host_client::protocol::InitStartRequest =
                serde_json::from_value(json!({ "road": "manual", "on": "here" })).unwrap();
            client.call(start).await.unwrap_or_else(|f| panic!("{}", f.message));
        })
    })
    .join()
    .unwrap();
    until(cx, "the pushed job", 90, |cx| flow_of(&view, cx).mine().is_some_and(|j| j.phase == "answering"));
    cx.run_until_parked();
    assert!(!drawn("options-loading", cx), "the picks stand once the host has read");
}

/// Catches: a pushed job not moving the build's rows or its count.
#[gpui_kit::test]
fn a_pushed_job_moves_the_build_rows(cx: &mut TestAppContext) {
    let cx = window(cx);
    let view = stage(cloud::job_flow(Step::Building, "building", cloud::build_rows(["done", "running", "waiting", "waiting"])), cx);
    assert_eq!(flow_of(&view, cx).figure(), "1 of 4 done");
    let next = cloud::job("building", cloud::build_rows(["done", "done", "running", "waiting"]));
    cx.update(|_, cx| view.update(cx, |v, cx| v.on_pushed(Pushed::Job(Box::new(next)), cx)));
    cx.run_until_parked();
    let flow = flow_of(&view, cx);
    assert_eq!(flow.figure(), "2 of 4 done");
    let states: Vec<RowState> = flow::build_rows(flow.mine().unwrap()).iter().map(|r| r.state).collect();
    assert_eq!(states, [RowState::Done, RowState::Done, RowState::Working, RowState::Waiting]);
}

/// Catches: a plain keystroke checking the key, and a paste or Enter not checking it.
#[gpui_kit::test]
fn the_key_is_checked_on_a_paste_or_enter_only(cx: &mut TestAppContext) {
    let cx = window(cx);
    let view = stage(cloud::key_flow(false), cx);
    cx.simulate_input("abc");
    assert_eq!(flow_of(&view, cx).check, Check::Idle, "typing alone asks nothing");
    cx.simulate_keystrokes("enter");
    assert_eq!(flow_of(&view, cx).check, Check::Checking, "Enter checks the key");
    let view = stage(cloud::key_flow(false), cx);
    cx.write_to_clipboard(ClipboardItem::new_string("pasted-key".into()));
    cx.simulate_keystrokes(if cfg!(target_os = "macos") { "cmd-v" } else { "ctrl-v" });
    cx.run_until_parked();
    let flow = flow_of(&view, cx);
    assert_eq!((flow.key.as_str(), &flow.check), ("pasted-key", &Check::Checking), "a paste checks the key");
}

/// Catches: Build image taken while a sign-in on this computer is missing.
#[gpui_kit::test]
fn build_image_is_held_until_every_sign_in_is_ready(cx: &mut TestAppContext) {
    let cx = window(cx);
    let view = stage(cloud::summary(false), cx);
    assert!(flow_of(&view, cx).build_held());
    click("cloud-continue", cx);
    assert!(!flow_of(&view, cx).recipe_busy, "a held Build image sends nothing");
    let mut ready = cloud::summary(true);
    ready.github = Some("zingzy".into());
    let view = stage(ready, cx);
    assert!(!flow_of(&view, cx).build_held());
    click("cloud-continue", cx);
    assert!(flow_of(&view, cx).recipe_busy, "Build image goes once all are signed in");
}

/// Catches: the check's answer for an older key standing over the newer one, and a refusal read as unreachable.
#[test]
fn a_late_check_is_dropped_and_its_kind_read() {
    let mut flow = cloud::key_flow(false);
    let ask = flow.type_key("first", true);
    assert_eq!(ask, Some(flow::Ask::Check { provider: "box".into(), key: "first".into() }));
    flow.type_key("second", true);
    flow.on_check("first", Ok(Facts::default()));
    assert_eq!(flow.check, Check::Checking, "an answer for a key no longer typed is dropped");
    flow.on_check("second", Err(wsp_host_client::Failure::of_kind("Boat refused this key", "keyRefused")));
    assert!(matches!(flow.check, Check::Refused(_)));
    flow.type_key("third", true);
    flow.on_check("third", Err(wsp_host_client::Failure::new("connect ETIMEDOUT")));
    assert!(matches!(flow.check, Check::Unreachable(_)));
    assert_eq!(
        flow::key_works_line(&Facts { account: "zingzy".into(), plan: "Pro".into(), running: 1, limit: 8 }),
        "Works. Account zingzy, Pro, 1 of 8 running at once."
    );
}

/// Catches: Escape or a press on the scrim leaving the dialog up, and a press inside taking it down.
#[gpui_kit::test]
fn escape_and_the_scrim_close_and_a_press_inside_does_not(cx: &mut TestAppContext) {
    let cx = window(cx);
    stage(cloud::key_flow(false), cx);
    let title = cx.debug_bounds("dialog-title").expect("the title is drawn");
    cx.simulate_click(title.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| layer::top(cx)), Some(ADD_CLOUD), "a press inside keeps it open");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| layer::top(cx)), None, "Escape closes it, the key field holding the keys");
    stage(cloud::key_flow(false), cx);
    cx.simulate_click(point(px(20.), px(20.)), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| layer::top(cx)), None, "a press on the scrim closes it");
}

/// Catches: the provider radios without Boat's cheapest rate at its row's right, or a rate drawn on Solari's row.
#[gpui_kit::test]
fn the_provider_radios_carry_each_clouds_rate(cx: &mut TestAppContext) {
    let cx = window(cx);
    let view = stage(cloud::two_clouds(), cx);
    let flow = flow_of(&view, cx);
    assert_eq!((flow.step, flow.offered()), (Step::Provider, vec!["box", "solari"]));
    let fact = cx.debug_bounds("box-fact").expect("Boat's radio carries its rate");
    assert!(cx.debug_bounds("solari-fact").is_none(), "Solari's radio carries no rate");
    let dialog = cx.debug_bounds("dialog").expect("the dialog is drawn");
    assert!(fact.center().x > dialog.center().x, "the rate stands at the row's right");
}

/// Catches: a ticked agent the report does not list dropped from the sign-ins, so Build image goes without it.
#[test]
fn an_agent_missing_from_the_report_still_needs_its_sign_in() {
    let mut flow = cloud::summary(true);
    flow.github = Some("zingzy".into());
    flow.report.as_mut().unwrap().retain(|a| a.id != "codex");
    let codex = flow.sign_in_lines().into_iter().find(|l| l.id == "codex").expect("Codex keeps its line off the catalog's sign-in");
    assert!(!codex.ready);
    assert!(flow.build_held(), "Build image waits on Codex's sign-in");
}

/// Catches: a refusal's fix kept inside its sentence, which would draw the fix in the refusal's ink.
#[test]
fn a_refusal_keeps_its_fix_apart() {
    let mut failure = wsp_host_client::Failure::new("Boat refused this key. Paste a new one.");
    failure.fix = Some("Paste a new one.".into());
    let refused = flow::Refused::from(failure);
    assert_eq!((refused.said.as_str(), refused.fix.as_deref()), ("Boat refused this key.", Some("Paste a new one.")));
}

/// Catches: Back taken while a pick's answer is with the host, which the late answer would then undo.
#[test]
fn back_is_held_while_an_answer_is_with_the_host() {
    let mut flow = cloud::job_flow(Step::Pick(1), "answering", json!([]));
    assert!(matches!(flow.go_on(), Some(flow::Ask::Answer { then: Step::Pick(2), .. })));
    assert_eq!(flow.back(), None, "Back sends nothing while the answer is out");
    assert_eq!(flow.step, Step::Pick(1));
    flow.on_recipe(Some(Step::Pick(2)), Ok(()));
    assert_eq!(flow.step, Step::Pick(2));
    assert!(flow.back().is_some() && flow.step == Step::Pick(1), "Back goes once the answer is in");
}

/// The window as the shell leaves it once connected: the layers hold the host and a sidebar listing this computer and
/// Boat, and the window keeps a store of its own.
fn connected<'a>(cx: &'a mut TestAppContext, server: &common::Throwaway, dir: &std::path::Path) -> &'a mut VisualTestContext {
    cx.update(desktop_gpui::screens::palette::overlays::init);
    let cx = window(cx);
    let store = desktop_gpui::local_store::LocalStore::open(dir.join("local-store.json"));
    let host = host(server);
    cx.update(|window, cx| {
        cx.set_global(store);
        let sidebar = cx.new(desktop_gpui::sidebar::Sidebar::new);
        let places =
            vec![json!({ "id": "here", "kind": "computer", "name": "boat" }), json!({ "id": "box", "kind": "provider", "name": "Boat" })];
        sidebar.update(cx, |s, cx| s.apply(desktop_gpui::sidebar::host::Change::Places(places), cx));
        let overlays = desktop_gpui::screens::palette::overlays::of(window, cx).expect("the layers are registered");
        overlays.update(cx, |o, _| {
            o.host = Some(host);
            o.sidebar = Some(sidebar.downgrade());
        });
        // The sidebar lives as long as the window's layers point at it.
        std::mem::forget(sidebar);
    });
    cx
}

fn notices(cx: &mut VisualTestContext) -> Vec<ui::Notice> {
    let toasts = cx.update(|window, cx| ui::toast::of(window, cx).expect("ui::init registers the stack"));
    toasts.read_with(cx, |t, _| t.standing())
}

fn sealed_on_box() -> flow::Job {
    serde_json::from_value(json!({ "id": "j_1", "phase": "done", "place": { "id": "box", "name": "Boat" }, "golden": { "version": 3 } }))
        .unwrap()
}

/// Catches: Add a cloud closed on a build that sealed saying nothing, or a cloud's build not known to stand on screen.
#[gpui_kit::test]
fn closing_on_a_sealed_build_posts_the_sealed_notice(cx: &mut TestAppContext) {
    let server = common::Throwaway::start();
    let dir = tempfile::tempdir().unwrap();
    let cx = connected(cx, &server, dir.path());
    let mut flow = Flow::new(Some("box"));
    flow.step = Step::Building;
    flow.job = Some(sealed_on_box());
    stage(flow, cx);
    assert!(cx.update(|_, cx| add_cloud::build_on_screen("box", cx)), "the dialog's heading stands in for the host's notice");
    cx.update(layer::dismiss);
    cx.run_until_parked();
    let said: Vec<(String, Option<String>)> =
        notices(cx).iter().map(|n| (n.text.to_string(), n.detail.as_ref().map(|d| d.to_string()))).collect();
    assert_eq!(said, [("Boat is ready".to_owned(), Some("Image v3 sealed.".to_owned()))]);
    assert!(!cx.update(|_, cx| add_cloud::build_on_screen("box", cx)));
}

/// Catches: the seal's Open going to the computer's page though the build owes its ready page, or owing it again once
/// the page has shown.
#[gpui_kit::test]
fn the_sealed_notices_open_shows_the_ready_page_owed_to_that_build(cx: &mut TestAppContext) {
    let server = common::Throwaway::start();
    let dir = tempfile::tempdir().unwrap();
    let cx = connected(cx, &server, dir.path());
    let mut flow = Flow::new(Some("box"));
    flow.step = Step::Building;
    flow.job = Some(sealed_on_box());
    stage(flow, cx);
    cx.update(layer::dismiss);
    cx.run_until_parked();
    let open = notices(cx)[0].action.clone().expect("the notice opens what it is about");
    cx.update(|window, cx| (open.run)(window, cx));
    cx.run_until_parked();
    let shown =
        cx.update(|_, cx| layer::view(ADD_CLOUD, cx)).and_then(|v| v.downcast::<AddCloud>().ok()).expect("Open stood the dialog up");
    assert_eq!(flow_of(&shown, cx).step, Step::Ready, "on the ready page that build owed");
    // The host holds the sealed job until the next start; this one's has none, so it is handed over as it pushes it.
    shown.update(cx, |v, cx| {
        v.on_pushed(Pushed::Job(Box::new(sealed_on_box())), cx);
        cx.notify();
    });
    cx.update(|window, cx| drop(window.draw(cx)));
    cx.run_until_parked();
    cx.update(layer::dismiss);
    cx.update(|window, cx| (open.run)(window, cx));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| layer::top(cx)), None, "a page shown once is owed no more: Open goes to the cloud's page");
}

/// Catches: a sign-in run the dialog started outliving its close, so reopening starts a second.
#[gpui_kit::test]
fn closing_stops_the_sign_in_runs_the_dialog_started(cx: &mut TestAppContext) {
    // The test's PATH holds no real codex; a stand-in whose login waits keeps the run going until it is stopped.
    let server = common::Throwaway::start_with_tools(STAYS, true, &[("codex", CODEX_LOGIN_WAITS)]);
    if server.daemon.is_none() {
        println!("skipped the sign-in stop, whose run the host starts through a wsp-daemon: {}", common::wsp_daemon().unwrap_err());
        return;
    }
    let cx = window(cx);
    let host = host(&server);
    cx.update(|window, cx| add_cloud::open_on(Some(&host), cloud::summary(false), window, cx));
    until(cx, "Codex's Sign in", 60, |cx| drawn("sign-in-codex", cx));
    // Beside the binary's other tests a first press can start nothing; press again until a run starts.
    let mut pressed = Instant::now() - Duration::from_secs(2);
    until(cx, "the run to start", 60, |cx| {
        if pressed.elapsed() >= Duration::from_secs(2) && !server.log().contains("sign-in si_") {
            click("sign-in-codex", cx);
            pressed = Instant::now();
        }
        server.log().contains("sign-in si_") && server.log().contains("started: codex")
    });
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    cx.update(layer::dismiss);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !server.log().lines().any(|l| l.contains("ended: codex, on computer here: stopped")) {
        assert!(Instant::now() < deadline, "the run was never stopped: {}", server.log());
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A claude the host finds, which never answers; Codex's sign-in runs on its own road.
const CODEX_LOGIN_WAITS: &str = "#!/bin/sh\ncase \" $* \" in\n  *\" --version \"*) echo \"codex-cli 0.162.1\"; exit 0 ;;\n  *\" login status \"*) echo \"Not logged in\"; exit 1 ;;\nesac\nsleep 60\n";
const STAYS: &str = "#!/bin/sh\ncase \" $* \" in *\" --version \"*) echo \"2.1.295 (Claude Code)\"; exit 0 ;; esac\nsleep 60\n";
