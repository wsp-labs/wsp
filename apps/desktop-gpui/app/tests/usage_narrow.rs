// SPDX-License-Identifier: AGPL-3.0-only
//! Settings > Usage at today's app's narrow widths, against a host started for this test alone (host-client's
//! tests/common) and seeded by `tests/usage/seed.mjs`: at 600, 760 and 1280 px, the totals strip and its figures on one
//! line, the split table's columns, the segments' sides and a pool's window rows, against the `max-sm:` rules
//! `usage.tsx` writes.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::screens::usage::model::{Range, Split};
use desktop_gpui::screens::usage::{frame, Tab, UsagePage};
use desktop_gpui::screens::Host;
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{prelude::*, px, size, AppContext, Bounds, Context, Entity, Pixels, TestAppContext, VisualTestContext, Window};
use wsp_host_client::HostClient;

const WIDTHS: [f32; 3] = [600., 760., 1280.];

fn seeded() -> Throwaway {
    let host = Throwaway::start();
    let folder = host.repo("usage");
    host.wsp(&["add", &folder.to_string_lossy(), "--name", "usage"]);
    let state = host.state().parent().unwrap().to_string_lossy().into_owned();
    let seed = common::root().join("apps/desktop-gpui/app/tests/usage/seed.mjs");
    let out = Command::new("node").arg(seed).arg(&state).env_clear().envs(host.env()).current_dir(host.dir.path()).output().unwrap();
    assert!(out.status.success(), "seed.mjs: {}", String::from_utf8_lossy(&out.stderr));
    host
}

struct Framed {
    page: Entity<UsagePage>,
}

impl Render for Framed {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = self.page.update(cx, |page, cx| page.tabs(cx).into_any_element());
        frame(Some(tabs), Some(self.page.clone().into_any_element()), window, cx)
    }
}

fn open<'a>(cx: &'a mut TestAppContext, host: &Throwaway) -> (Entity<UsagePage>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    cx.executor().allow_parking();
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    let handle = Host::new(client, runtime);
    let mut page = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let usage = cx.new(|cx| UsagePage::with_hold(handle.clone(), Duration::from_secs(60), window, cx));
        page = Some(usage.clone());
        let framed = cx.new(|_| Framed { page: usage });
        Root::new(framed, window, cx)
    });
    (page.unwrap(), cx)
}

fn until(cx: &mut VisualTestContext, page: &Entity<UsagePage>, what: &str, done: impl Fn(&UsagePage) -> bool) {
    let limit = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if cx.read(|cx| done(page.read(cx))) {
            return;
        }
        assert!(Instant::now() < limit, "the page never got to: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn landed(page: &UsagePage) -> bool {
    let used = page.used.value.as_ref().is_some_and(|u| Range::from(u.range) == Range::Week && Split::from(u.split) == Split::Agent);
    !page.reading() && used && page.accounts.value.is_some()
}

fn at(cx: &mut VisualTestContext, width: f32) {
    cx.simulate_resize(size(px(width), px(2400.)));
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str, width: f32) -> Bounds<Pixels> {
    cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} was not drawn at {width} px"))
}

#[track_caller]
fn near(what: &str, got: Pixels, want: f32) {
    let got = f32::from(got);
    assert!((got - want).abs() < 0.5, "{what}: {got} px, the web writes {want} px");
}

#[gpui_kit::test]
fn usage_folds_its_totals_table_and_windows_under_640(cx: &mut TestAppContext) {
    let host = seeded();
    let (page, cx) = open(cx, &host);
    until(cx, &page, "a week by agent and the accounts", landed);
    let mut range_width = Vec::new();
    for width in WIDTHS {
        at(cx, width);
        let (tokens, cache) = (bounds(cx, "stat-stat-tokens", width), bounds(cx, "stat-stat-cache", width));
        let estimate = bounds(cx, "stat-stat-estimate", width);
        near(&format!("the estimate's figure on one line at {width} px"), bounds(cx, "stat-estimate-figure", width).size.height, 32.);
        if width < 640. {
            near("the strip's second figure beside the first", estimate.left(), f32::from(tokens.right()));
            assert!(cache.top() >= tokens.bottom(), "the strip's later figures start a second line at {width} px: {cache:?}");
            assert!(cx.debug_bounds("used-head-cache-hit").is_none(), "Cache hit stands down at {width} px");
            assert!(cx.debug_bounds("used-head-turns").is_none(), "Turns stands down at {width} px");
            // Each head word is set right in its column: `minmax(0,1fr) 72px 88px` with `gap-x-4`.
            let (head_tokens, head_estimate) = (bounds(cx, "used-head-tokens", width), bounds(cx, "used-head-estimate", width));
            near("the estimate's 88 px and the gap before it", head_estimate.right() - head_tokens.right(), 16. + 88.);
        } else {
            near("one line of figures", cache.top(), f32::from(tokens.top()));
            let (head_tokens, head_cache) = (bounds(cx, "used-head-tokens", width), bounds(cx, "used-head-cache-hit", width));
            near("cache hit's 96 px and the gap-x-6 before it", head_cache.right() - head_tokens.right(), 24. + 96.);
        }
        range_width.push(bounds(cx, "usage-range", width).size.width);
    }
    // `px-3.5 max-sm:px-2`: each of the three segments 12 px narrower.
    near("the range's segments under 640 px", range_width[1] - range_width[0], 12. * Range::ALL.len() as f32);
    near("the range's segments from 640 px", range_width[2] - range_width[1], 0.);

    cx.update(|_, cx| page.update(cx, |p, cx| p.pick_tab(Tab::Limits, cx)));
    cx.run_until_parked();
    for width in WIDTHS {
        at(cx, width);
        let (figure, meter) = (bounds(cx, "usage-window-figure", width), bounds(cx, "usage-meter", width));
        if width < 640. {
            near("the meter under the figure, gap-y-3", meter.top() - figure.bottom(), 12.);
            near("one column", meter.size.width, f32::from(figure.size.width));
        } else {
            near("the figure's 14rem", figure.size.width, 224.);
            near("gap-x-10 before the meter", meter.left() - figure.right(), 40.);
        }
    }
}
