// SPDX-License-Identifier: AGPL-3.0-only
//! The window's content drawn by GPUI's real offscreen renderer, WGPU on Linux (Mesa lavapipe without a GPU) and
//! Metal on macOS, and read back as pixels. It runs without libtest's harness because AppKit panics unless it starts
//! on the main thread, and libtest runs each test on another.

use std::sync::Arc;

use desktop_gpui::shell::layout;
use desktop_gpui::sidebar::{Sidebar, ThreadItem, WIDTH};
use desktop_gpui::thread_view::ThreadView;
use desktop_gpui::ui::{ActivePalette, UiAssets};
use gpui_kit::component::{ActiveTheme, Root};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    px, size, AnyWindowHandle, AppContext, Bounds, Context, Entity, HeadlessAppContext, IntoElement, Render, Rgba, Window, WindowBounds,
    WindowOptions,
};
use wsp_host_client::protocol::SessionEvent;

const W: f32 = 1280.;
const H: f32 = 800.;

struct Content {
    sidebar: Entity<Sidebar>,
    thread_view: Entity<ThreadView>,
}

impl Render for Content {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        layout(&self.sidebar, &self.thread_view, false, cx)
    }
}

/// One running turn: the person's message, then a reply with prose and a fenced code block. No turn settles, so
/// no fold hides the reply.
fn transcript() -> Vec<SessionEvent> {
    let reply = "The build fails because `host-client` pins an older `tokio`.\n\n\
                 Bump it in the workspace manifest:\n\n\
                 ```toml\n[workspace.dependencies]\ntokio = { version = \"1.40\", features = [\"rt-multi-thread\"] }\n```\n\n\
                 Then run the tests again.";
    [
        serde_json::json!({"type": "session.start", "pos": 1, "prompt": "Why does the desktop build fail on Linux?"}),
        serde_json::json!({"type": "session.delta", "pos": 2, "kind": "text", "text": reply}),
    ]
    .into_iter()
    .map(|mut event| {
        let scope = serde_json::json!({"workspaceId": "w", "sessionId": "s", "threadId": "t1", "turnId": "u1"});
        event.as_object_mut().unwrap().extend(scope.as_object().unwrap().clone());
        serde_json::from_value(event).unwrap()
    })
    .collect()
}

fn items() -> Vec<ThreadItem> {
    let item = |thread_id: &str, title: &str| ThreadItem {
        workspace_id: "w".to_owned(),
        thread_id: thread_id.to_owned(),
        project: "wsp".to_owned(),
        title: title.to_owned(),
    };
    vec![item("t1", "Why does the desktop build fail on Linux?"), item("t2", "Move the app onto gpui-kit")]
}

struct Capture {
    width: u32,
    height: u32,
    scale: f32,
    raw: Vec<u8>,
}

impl Capture {
    fn device(&self, logical: f32) -> u32 {
        (logical * self.scale).round() as u32
    }

    /// The pixel at a logical point.
    fn at(&self, x: f32, y: f32) -> [u8; 3] {
        self.device_at(self.device(x), self.device(y))
    }

    fn device_at(&self, x: u32, y: u32) -> [u8; 3] {
        let start = ((y * self.width + x) * 4) as usize;
        [self.raw[start], self.raw[start + 1], self.raw[start + 2]]
    }
}

fn rgb(color: impl Into<Rgba>) -> [u8; 3] {
    let c: Rgba = color.into();
    assert!(c.a > 0.99, "{c:?} is translucent, so its pixel depends on what is under it");
    [c.r, c.g, c.b].map(|v| (v * 255.).round() as u8)
}

fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(a, b)| a.abs_diff(b) <= 3)
}

struct Colors {
    sidebar: [u8; 3],
    background: [u8; 3],
    bubble: [u8; 3],
}

fn render() -> (Capture, Colors) {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    let (sidebar, thread_view) = cx.update(|cx| (cx.new(Sidebar::new), cx.new(ThreadView::new)));
    cx.update(|cx| {
        sidebar.update(cx, |s, cx| {
            s.set_items(items(), cx);
            s.mark("t1", cx);
        });
        thread_view.update(cx, |v, cx| {
            v.show("t1", cx);
            v.load(transcript(), cx);
        });
    });
    let (handle, _): (AnyWindowHandle, _) = cx
        .update(|cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Default::default(), size: size(px(W), px(H)) })),
                focus: false,
                show: false,
                ..Default::default()
            };
            let content = Content { sidebar: sidebar.clone(), thread_view: thread_view.clone() };
            gpui_kit::open_window(options, cx, |window, cx| {
                let content = cx.new(|_| content);
                cx.new(|cx| Root::new(content, window, cx))
            })
        })
        .unwrap();
    // The list measures its rows in one frame and the markdown highlights in background tasks, so draw until settled.
    for _ in 0..3 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
    }
    cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
    let scale = cx.update_window(handle, |_, window, _| window.scale_factor()).unwrap();
    let image = cx.capture_screenshot(handle).expect("offscreen rendering must be available");
    if let Some(path) = std::env::var_os("PIXELS_PNG") {
        image.save(path).unwrap();
    }
    let colors = cx.update(|cx| {
        let theme = cx.theme();
        Colors { sidebar: rgb(theme.sidebar), background: rgb(theme.background), bubble: rgb(cx.palette().accent) }
    });
    let (width, height) = image.dimensions();
    (Capture { width, height, scale, raw: image.into_raw() }, colors)
}

fn main() {
    the_window_draws_the_sidebar_beside_the_transcript();
    println!("test the_window_draws_the_sidebar_beside_the_transcript ... ok");
}

fn the_window_draws_the_sidebar_beside_the_transcript() {
    let (shot, colors) = render();
    assert_eq!((shot.width, shot.height), (shot.device(W), shot.device(H)));
    let width = f32::from(WIDTH);
    assert_ne!(colors.sidebar, colors.background, "the theme draws the sidebar apart from the thread view");

    // Below the last row, each side shows only its own background.
    assert!(near(shot.at(width / 2., H - 10.), colors.sidebar), "sidebar at the bottom: {:?}", shot.at(width / 2., H - 10.));
    assert!(near(shot.at(W - 10., H - 10.), colors.background), "thread view at the bottom: {:?}", shot.at(W - 10., H - 10.));

    // The person's bubble is the first row, flush with the right edge of the thread view's centred 768 px column. The
    // list's 16 px top space puts y = 22 inside the bubble's fill, above its 12 px padded text.
    let column_right = width + (W - width + 768.) / 2.;
    let y = shot.device(22.);
    let bubble: Vec<u32> = (shot.device(width)..shot.device(W)).filter(|&x| near(shot.device_at(x, y), colors.bubble)).collect();
    let (first, last) = (bubble.first().copied().unwrap_or(0), bubble.last().copied().unwrap_or(0));
    assert!(!bubble.is_empty() && bubble.len() as u32 == last - first + 1, "no unbroken bubble fill at y = {y}: {} columns", bubble.len());
    // Below the 18 px corner, the fill reaches the column's edge.
    let below = shot.device(16. + 18.);
    let edge = (shot.device(width)..shot.device(W)).rev().find(|&x| near(shot.device_at(x, below), colors.bubble)).unwrap_or(0);
    assert!(edge.abs_diff(shot.device(column_right)) <= 2, "the bubble must end at the column's edge, ends at {edge}");
    assert!(last < edge, "y = {y} is inside the bubble's rounded corner");
    assert!(first > shot.device(width + 200.), "the bubble must fit its text, starts at {first}");

    // The reply's prose and code put dark glyph pixels on the thread view's background.
    let ink = (shot.device(80.)..shot.device(400.))
        .flat_map(|y| (shot.device(width + 16.)..shot.device(W - 16.)).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let p = shot.device_at(x, y);
            p.iter().zip(colors.background).map(|(a, b)| a.abs_diff(b) as u32).sum::<u32>() > 300
        })
        .count();
    assert!(ink > 2_000, "only {ink} glyph pixels in the reply");
}
