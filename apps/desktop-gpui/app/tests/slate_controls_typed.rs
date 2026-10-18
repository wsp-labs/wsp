// SPDX-License-Identifier: AGPL-3.0-only
//! Words typed into a slate field and dropped with the panel inside the field's 300 ms wait, as a thread switch drops
//! it: the panel keeps them for the thread and sends them to the host as it goes, against a host of this binary alone.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "slate/fixture.rs"]
mod fixture;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use common::{shared, Throwaway};
use desktop_gpui::screens::slate::{self, SlateScreen};
use desktop_gpui::screens::{Host, Opened, Screen};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{div, prelude::*, px, size, Context, Entity, Modifiers, TestAppContext, VisualTestContext, Window};
use serde_json::{json, Value};
use wsp_host_client::{protocol, HostClient};

const FIELD: &str = r#"<slate><value name="note" start="" /><column>
  <input id="note" label="Note" value={$note} />
</column></slate>"#;

fn bench() -> (&'static Throwaway, &'static PathBuf) {
    static FOLDER: OnceLock<PathBuf> = OnceLock::new();
    let host = &shared().host;
    (host, FOLDER.get_or_init(|| fixture::project(host)))
}

fn client() -> Host {
    let host = bench().0;
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    Host::new(client, runtime)
}

/// What draws the panel: a thread switch drops the screen as `RightPanel` drops its tab's.
struct Holder {
    screen: Option<Entity<SlateScreen>>,
}

impl Render for Holder {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().children(self.screen.clone())
    }
}

fn on_host(handle: &Host, thread: &str, path: &str) -> Option<Value> {
    let (thread, path) = (thread.to_owned(), path.to_owned());
    let read = futures::executor::block_on(handle.run(move |host| async move {
        let ask: protocol::SlatesReadRequest =
            serde_json::from_value(json!({ "threadId": thread, "sketch": false, "text": false })).unwrap();
        host.call(ask).await
    }))
    .unwrap_or_else(|f| panic!("slates.read: {}", f.message));
    read.state.get(&path).cloned()
}

fn until(cx: &mut VisualTestContext, what: &str, done: impl Fn(&mut VisualTestContext) -> bool) {
    let limit = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < limit, "never got to: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[gpui_kit::test]
fn words_typed_inside_the_wait_outlive_the_panel_and_reach_the_host(cx: &mut TestAppContext) {
    let (host, folder) = bench();
    let thread = fixture::staged(host, folder, &fixture::State { name: "typed", file: None });
    fixture::write(host, &thread, FIELD);
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    cx.executor().allow_parking();
    let handle = client();
    let opened = Opened { thread_id: Some(thread.clone()), workspace_id: None, tab: Some("slate".into()), arg: None };
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| SlateScreen::new(handle.clone(), opened, window, cx));
        let holder = cx.new(|_| Holder { screen: Some(screen) });
        made = Some(holder.clone());
        Root::new(holder, window, cx)
    });
    cx.simulate_resize(size(px(540.), px(748.)));
    let holder = made.unwrap();
    until(cx, "the field drawn", |cx| cx.debug_bounds("slate-field").is_some());
    let field = cx.debug_bounds("slate-field").unwrap();
    cx.simulate_click(field.center(), Modifiers::none());
    cx.simulate_input("Ada");
    cx.run_until_parked();
    holder.update(cx, |h, cx| {
        h.screen = None;
        cx.notify();
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(cx.read(slate::kept_threads), std::slice::from_ref(&thread), "the thread keeps the typed words");
    let limit = Instant::now() + Duration::from_secs(20);
    while on_host(&handle, &thread, "$note") != Some(json!("Ada")) {
        assert!(Instant::now() < limit, "the words typed inside the field's wait reached the host as the panel went");
        std::thread::sleep(Duration::from_millis(50));
    }
}
