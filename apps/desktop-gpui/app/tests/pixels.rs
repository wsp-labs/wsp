// SPDX-License-Identifier: AGPL-3.0-only
//! The window's content drawn by GPUI's real offscreen renderer, WGPU on Linux (Mesa lavapipe without a GPU) and
//! Metal on macOS, and read back as pixels. It runs without libtest's harness because AppKit panics unless it starts
//! on the main thread, and libtest runs each test on another.

use std::sync::Arc;

use desktop_gpui::screens::app_shell::assets::AppAssets;
use desktop_gpui::screens::app_shell::frame::{SIDEBAR_DEFAULT, TOPBAR};
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::sidebar::ThreadItem;
use gpui_kit::component::{ActiveTheme, Root};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{px, size, AnyWindowHandle, AppContext, Bounds, HeadlessAppContext, Rgba, WindowBounds, WindowOptions};
use wsp_host_client::protocol::SessionEvent;

const W: f32 = 1280.;
const H: f32 = 800.;

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
        Arc::new(AppAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap());
    let state = std::env::temp_dir().join("desktop-gpui-pixels-no-host").join("state.json");
    let (handle, shell) = cx.update(|cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Default::default(), size: size(px(W), px(H)) })),
            focus: false,
            show: false,
            ..Default::default()
        };
        let mut shell = None;
        let handle = cx
            .open_window(options, |window, cx| {
                let view = cx.new(|cx| Shell::new(state, Default::default(), runtime, Launch::default(), window, cx));
                shell = Some(view.clone());
                cx.new(|cx| Root::new(view, window, cx))
            })
            .unwrap();
        let handle: AnyWindowHandle = handle.into();
        (handle, shell.unwrap())
    });
    cx.update_window(handle, |_, window, cx| {
        shell.update(cx, |shell, cx| {
            shell.sidebar().update(cx, |s, cx| s.set_items(items(), cx));
            shell.go(Route::Thread { workspace_id: "w".to_owned(), thread_id: "t1".to_owned() }, window, cx);
            shell.thread_view().update(cx, |v, cx| v.load(transcript(), cx));
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
        let p = desktop_gpui::ui::ActivePalette::palette(cx);
        Colors { sidebar: rgb(p.sidebar), background: rgb(p.background), bubble: rgb(theme.secondary) }
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
    let width = f32::from(SIDEBAR_DEFAULT);
    let top = f32::from(TOPBAR);
    assert_ne!(colors.sidebar, colors.background, "the theme draws the sidebar apart from the thread view");

    // Below the last row, each side shows only its own background.
    assert!(near(shot.at(width / 2., H - 10.), colors.sidebar), "sidebar at the bottom: {:?}", shot.at(width / 2., H - 10.));
    assert!(near(shot.at(W - 10., H - 10.), colors.background), "thread view at the bottom: {:?}", shot.at(W - 10., H - 10.));

    // The person's bubble is the first row, flush with the 16 px right padding of the thread view's centred 768 px
    // column. Across the middle of its one line the fill stands either side of the text.
    let column_right = width + (W - width + 768.) / 2.;
    let y = shot.device(top + 29.);
    let bubble: Vec<u32> = (shot.device(width)..shot.device(W)).filter(|&x| shot.device_at(x, y) == colors.bubble).collect();
    let (first, last) = (bubble.first().copied().unwrap_or(0), bubble.last().copied().unwrap_or(0));
    assert!(!bubble.is_empty(), "no bubble fill at y = {y}");
    assert!(last.abs_diff(shot.device(column_right - 16.)) <= 2, "the bubble must end at the right padding, ends at {last}");
    assert!(first > shot.device(width + 200.), "the bubble must fit its text, starts at {first}");

    // The reply's prose and code put dark glyph pixels on the thread view's background.
    let ink = (shot.device(top + 80.)..shot.device(top + 400.))
        .flat_map(|y| (shot.device(width + 16.)..shot.device(W - 16.)).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let p = shot.device_at(x, y);
            p.iter().zip(colors.background).map(|(a, b)| a.abs_diff(b) as u32).sum::<u32>() > 300
        })
        .count();
    assert!(ink > 2_000, "only {ink} glyph pixels in the reply");
}
