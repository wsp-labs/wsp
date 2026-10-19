// SPDX-License-Identifier: AGPL-3.0-only
//! Road 1: gpui-kit's webview at our pin, which builds on GPUI Fast and its window composition.

use gpui_kit::*;
use gpui_webview::WebView;

use crate::{log, rect};

pub const NAME: &str = "gpui-kit webview (GPUI Fast)";

pub struct Page {
    webview: Entity<WebView>,
    last: Option<Bounds<Pixels>>,
}

impl Page {
    pub fn new(url: &str, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let builder = lb_wry::WebViewBuilder::new()
            .with_url(url)
            .with_ipc_handler(|request| log(format!("page: {}", request.body())))
            .with_on_page_load_handler(|event, url| {
                if let lb_wry::PageLoadEvent::Finished = event {
                    log(format!("page: load finished {url}"));
                }
            });
        let webview = cx.new(|cx| WebView::build(builder, window, cx).expect("gpui-kit builds the webview"));
        log(format!("page: created for {url}"));
        cx.new(|_| Self { webview, last: None })
    }

    pub fn load(&mut self, url: &str, cx: &mut Context<Self>) {
        self.webview.update(cx, |webview, _| webview.load_url(url));
    }

    /// GPUI Fast's composition draws overlays above the page by itself; nothing to cut.
    pub fn set_overlays(&mut self, _: Vec<Bounds<Pixels>>) {}
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        div().relative().size_full().child(self.webview.clone()).child(
            canvas(
                move |bounds, _, cx| {
                    this.update(cx, |page, _| {
                        if page.last.replace(bounds) != Some(bounds) {
                            log(format!("pane bounds: {}", rect(bounds)));
                        }
                    })
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
    }
}
