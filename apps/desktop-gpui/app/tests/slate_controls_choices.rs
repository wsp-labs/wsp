// SPDX-License-Identifier: AGPL-3.0-only
//! A choices piece to the keyboard, against a host started for this binary alone, as the web's radio rows answer: one
//! tab stop wearing the ring before any pick, Enter and Space picking the row with the stop, and the first arrow
//! stepping from that stop, the first row.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "slate/fixture.rs"]
mod fixture;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use common::{shared, Throwaway};
use desktop_gpui::screens::slate::SlateScreen;
use desktop_gpui::screens::{Host, Opened, Screen};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{prelude::*, px, size, Entity, TestAppContext, VisualTestContext};
use serde_json::{json, Value};
use wsp_host_client::HostClient;

const CHOICES: &str = r#"<slate title="Choices">
  <value name="answer" start={null} />
  <column>
    <choices id="layer" label="Which layer routes packets?" value={$answer} options={["Link", "Network", "Transport"]} />
  </column>
</slate>"#;

fn bench() -> (&'static Throwaway, &'static PathBuf) {
    static FOLDER: OnceLock<PathBuf> = OnceLock::new();
    let host = &shared().host;
    (host, FOLDER.get_or_init(|| fixture::project(host)))
}

fn thread() -> String {
    let (host, folder) = bench();
    let thread = fixture::staged(host, folder, &fixture::State { name: "choices", file: None });
    fixture::write(host, &thread, CHOICES);
    thread
}

fn client() -> Host {
    let host = bench().0;
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    Host::new(client, runtime)
}

fn open<'a>(cx: &'a mut TestAppContext, thread: &str) -> (Entity<SlateScreen>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    cx.executor().allow_parking();
    let handle = client();
    let opened = Opened { thread_id: Some(thread.to_owned()), workspace_id: None, tab: Some("slate".into()), arg: None };
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| SlateScreen::new(handle.clone(), opened, window, cx));
        made = Some(screen.clone());
        Root::new(screen, window, cx)
    });
    cx.simulate_resize(size(px(540.), px(748.)));
    let screen = made.unwrap();
    until(cx, &screen, "the slate drawn and answered", |s| {
        s.slate().is_some_and(|slate| slate.formulas().iter().all(|f| slate.resolved.contains_key(f)))
    });
    (screen, cx)
}

fn until(cx: &mut VisualTestContext, screen: &Entity<SlateScreen>, what: &str, done: impl Fn(&SlateScreen) -> bool) {
    let limit = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        if cx.read(|cx| done(screen.read(cx))) {
            return;
        }
        assert!(Instant::now() < limit, "the panel never got to: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn answer(s: &SlateScreen) -> Option<Value> {
    s.slate()?.record.values.get("answer").cloned()
}

/// Tab onto the group, past whatever the panel's head holds, as the person reaches it.
fn reach(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.focus_next(cx));
    if cx.debug_bounds("slate-choice-open-ring").is_some() {
        return;
    }
    for _ in 0..8 {
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        if cx.debug_bounds("slate-choice-open-ring").is_some() {
            return;
        }
    }
}

#[gpui_kit::test]
fn before_any_pick_the_first_row_wears_the_ring(cx: &mut TestAppContext) {
    let (_, cx) = open(cx, &thread());
    assert!(cx.debug_bounds("slate-choice-open-ring").is_none(), "no ring before the group is reached");
    reach(cx);
    let ring = cx.debug_bounds("slate-choice-open-ring").expect("a row wears the ring once the group is reached");
    let first = cx.debug_bounds("slate-choice-open").expect("the other rows are drawn");
    assert!(ring.top() < first.top(), "the ring stands on the first row, above the rows without it");
}

#[gpui_kit::test]
fn space_picks_the_row_with_the_stop(cx: &mut TestAppContext) {
    let (screen, cx) = open(cx, &thread());
    reach(cx);
    cx.simulate_keystrokes("space");
    until(cx, &screen, "Space picked Link", |s| answer(s) == Some(json!("Link")));
}

#[gpui_kit::test]
fn enter_picks_the_row_with_the_stop(cx: &mut TestAppContext) {
    let (screen, cx) = open(cx, &thread());
    reach(cx);
    cx.simulate_keystrokes("enter");
    until(cx, &screen, "Enter picked Link", |s| answer(s) == Some(json!("Link")));
}

#[gpui_kit::test]
fn the_first_down_steps_from_the_first_row_to_the_second(cx: &mut TestAppContext) {
    let (screen, cx) = open(cx, &thread());
    reach(cx);
    cx.simulate_keystrokes("down");
    until(cx, &screen, "Down picked Network", |s| answer(s) == Some(json!("Network")));
    cx.simulate_keystrokes("down");
    until(cx, &screen, "a second Down picked Transport", |s| answer(s) == Some(json!("Transport")));
}

#[gpui_kit::test]
fn the_first_up_steps_from_the_first_row_round_to_the_last(cx: &mut TestAppContext) {
    let (screen, cx) = open(cx, &thread());
    reach(cx);
    cx.simulate_keystrokes("up");
    until(cx, &screen, "Up picked Transport", |s| answer(s) == Some(json!("Transport")));
}
