// SPDX-License-Identifier: AGPL-3.0-only
//! Add a cloud provider: its reads, a write and a push against a host started for these tests alone, and the flow's
//! own moves on fixture answers. No key ever reaches a provider: `providers.checkKey` calls a provider's API, so the
//! check's answer is fed in as a fixture, and the one write sent names a provider the host keys nothing for, which
//! `init.keys` refuses before any check.

#[path = "dialogs/cloud.rs"]
mod cloud;
#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::dialogs::add_cloud::flow::{self, Check, Facts, Flow, Step};
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

/// Catches: a refusal's fix kept inside its sentence, which would draw the fix in the refusal's ink.
#[test]
fn a_refusal_keeps_its_fix_apart() {
    let mut failure = wsp_host_client::Failure::new("Boat refused this key. Paste a new one.");
    failure.fix = Some("Paste a new one.".into());
    let refused = flow::Refused::from(failure);
    assert_eq!((refused.said.as_str(), refused.fix.as_deref()), ("Boat refused this key.", Some("Paste a new one.")));
}

/// Catches: Continue from the key not saving what was typed, a held key sent as an empty one, and the dialog left up
/// once the cloud was added.
#[test]
fn continuing_from_the_key_saves_it_and_the_dialog_closes_on_the_cloud_added() {
    let mut flow = cloud::key_flow(false);
    assert_eq!(flow.key_on(), None, "no key typed and none held: Continue sends nothing");
    flow.type_key("fake-key", true);
    flow.on_check("fake-key", Ok(Facts::default()));
    assert_eq!(flow.key_on(), Some(flow::Ask::Save { provider: "box".into(), key: Some("fake-key".into()) }));
    assert_eq!(flow.key_on(), None, "a second press while the first is with the host sends nothing");
    assert!(flow.on_saved(Ok(Some(cloud::setup(true)))), "the key saved: the dialog closes");
    assert!(flow.held() && !flow.busy && flow.key.is_empty());
    let mut held = cloud::key_flow(true);
    assert_eq!(held.key_on(), Some(flow::Ask::Save { provider: "box".into(), key: None }), "a held key is used as it stands");
    assert!(!held.on_saved(Err(wsp_host_client::Failure::new("the host said no"))), "a refusal keeps the dialog up");
    assert_eq!(held.refused.as_ref().map(|r| r.said.as_str()), Some("the host said no"));
}

/// Catches: the provider step counted where the host offers one cloud, or missing where it offers two.
#[test]
fn the_provider_step_is_counted_only_where_more_than_one_cloud_is_offered() {
    assert_eq!(cloud::key_flow(false).steps(), [Step::Key]);
    assert_eq!(cloud::key_flow(false).figure(), "1 of 1");
    let two = cloud::two_clouds();
    assert_eq!(two.steps(), [Step::Provider, Step::Key]);
    assert_eq!(two.figure(), "1 of 2");
}
