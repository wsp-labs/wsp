// SPDX-License-Identifier: AGPL-3.0-only
//! The window's content drawn by GPUI's real offscreen renderer, WGPU on Linux (Mesa lavapipe without a GPU) and
//! Metal on macOS, and read back as pixels. It runs without libtest's harness because AppKit panics unless it starts
//! on the main thread, and libtest runs each test on another.

use std::sync::Arc;

use desktop_gpui::screens::app_shell::assets::AppAssets;
use desktop_gpui::screens::app_shell::frame::{clamp_panel, SIDEBAR_DEFAULT, TOPBAR};
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::sidebar::ThreadItem;
use gpui_kit::component::Root;
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
    let dir = std::env::temp_dir().join("desktop-gpui-pixels-no-host");
    let (state, home) = (dir.join("state.json"), [("HOME".to_owned(), dir.to_string_lossy().into_owned())].into_iter().collect());
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
                let view = cx.new(|cx| Shell::new(state, home, runtime, Launch::default(), window, cx));
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
        let p = desktop_gpui::ui::ActivePalette::palette(cx);
        Colors { sidebar: rgb(p.sidebar), background: rgb(p.background), bubble: rgb(p.accent) }
    });
    let (width, height) = image.dimensions();
    (Capture { width, height, scale, raw: image.into_raw() }, colors)
}

fn main() {
    the_window_draws_the_sidebar_beside_the_transcript();
    println!("test the_window_draws_the_sidebar_beside_the_transcript ... ok");
    the_launchers_lede_centres_each_line();
    println!("test the_launchers_lede_centres_each_line ... ok");
    each_columns_top_row_stands_on_the_columns_own_fill();
    println!("test each_columns_top_row_stands_on_the_columns_own_fill ... ok");
    the_browsers_address_bar_stands_on_the_panels_own_fill();
    println!("test the_browsers_address_bar_stands_on_the_panels_own_fill ... ok");
}

/// The address bar under it.
struct BarOnGround {
    field: gpui_kit::Entity<desktop_gpui::screens::browser::field::AddressField>,
}

impl gpui_kit::Render for BarOnGround {
    fn render(&mut self, _: &mut gpui_kit::Window, cx: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        use gpui_kit::{IntoElement as _, ParentElement as _, Styled as _};
        let act = || -> desktop_gpui::screens::browser::chrome::Act { std::rc::Rc::new(|_, _| {}) };
        let ground = desktop_gpui::ui::ActivePalette::palette(&**cx).sidebar;
        gpui_kit::div().size_full().flex().flex_col().bg(ground).child(desktop_gpui::screens::browser::chrome::ChromeRow {
            field: self.field.clone(),
            loading: false,
            can_back: false,
            can_forward: false,
            refresh_held: false,
            has_page: false,
            inline: true,
            forwarded: None,
            copy: None,
            more: gpui_kit::div().into_any_element(),
            on_back: act(),
            on_forward: act(),
            on_refresh: act(),
            on_open_outside: act(),
            on_open_on_laptop: act(),
            on_copy: act(),
        })
    }
}

/// `PreviewChromeRow`'s inline row draws on the panel's own surface: the bar shows whatever its pane stands on, here a
/// ground apart from the theme's background.
fn the_browsers_address_bar_stands_on_the_panels_own_fill() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(AppAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    let handle: AnyWindowHandle = cx
        .update(|cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Default::default(), size: size(px(540.), px(120.)) })),
                focus: false,
                show: false,
                ..Default::default()
            };
            cx.open_window(options, |window, cx| {
                let field = cx.new(|cx| desktop_gpui::screens::browser::field::AddressField::new(window, cx));
                let view = cx.new(|_| BarOnGround { field });
                cx.new(|cx| Root::new(view, window, cx))
            })
        })
        .unwrap()
        .into();
    for _ in 0..2 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
    }
    let scale = cx.update_window(handle, |_, window, _| window.scale_factor()).unwrap();
    let image = cx.capture_screenshot(handle).expect("offscreen rendering must be available");
    let (width, height) = image.dimensions();
    let shot = Capture { width, height, scale, raw: image.into_raw() };
    let (sidebar, background) = cx.update(|cx| {
        let p = desktop_gpui::ui::ActivePalette::palette(cx);
        (rgb(p.sidebar), rgb(p.background))
    });
    assert_ne!(sidebar, background, "the ground stands apart from the background");
    // In the row's 8 px left padding, then under the row in the 12 px it keeps below it.
    assert_eq!(shot.at(3., 14.), sidebar, "the address bar's row at (3, 14)");
    assert_eq!(shot.at(3., 34.), sidebar, "the ground under the row at (3, 34)");
}

/// The web app draws one surface per column, its 52 px top row on the same fill as the column under it: in the
/// sidebar, the centre (where the grain lies over the background) and the right panel, in Paper and in Graphite.
fn each_columns_top_row_stands_on_the_columns_own_fill() {
    for theme in ["paper", "graphite"] {
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
        let dir = std::env::temp_dir().join(format!("desktop-gpui-pixels-rows-{theme}-no-host"));
        let (state, home) = (dir.join("state.json"), [("HOME".to_owned(), dir.to_string_lossy().into_owned())].into_iter().collect());
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
                    let view = cx.new(|cx| Shell::new(state, home, runtime, Launch::default(), window, cx));
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
                shell.panel().update(cx, |p, cx| p.toggle(window, cx));
            });
            // With no host the shell draws the side's default; the theme goes on after it.
            desktop_gpui::ui::theme::set_theme(theme, cx);
            window.refresh();
        })
        .unwrap();
        for _ in 0..3 {
            cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
            cx.run_until_parked();
        }
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        let panel = cx.update(|cx| shell.read(cx).panel().read(cx).width());
        let panel_left = W - f32::from(clamp_panel(panel, px(W), px(W) - SIDEBAR_DEFAULT));
        // Each column's left edge, past its border and short of its row's first control.
        let columns = [("sidebar", 4.), ("centre", f32::from(SIDEBAR_DEFAULT) + 6.), ("panel", panel_left + 6.)];
        let scale = cx.update_window(handle, |_, window, _| window.scale_factor()).unwrap();
        let image = cx.capture_screenshot(handle).expect("offscreen rendering must be available");
        if let Some(dir) = std::env::var_os("PIXELS_ROWS_DIR") {
            image.save(std::path::Path::new(&dir).join(format!("rows-{theme}.png"))).unwrap();
        }
        let (width, height) = image.dimensions();
        let shot = Capture { width, height, scale, raw: image.into_raw() };
        let under = f32::from(TOPBAR) + 4.;
        for (column, x) in columns {
            assert_eq!(
                shot.at(x, 4.),
                shot.at(x, under),
                "{theme}: the {column}'s top row at x {x} against the column under it at y {under}"
            );
        }
    }
}

/// The launcher's line under "Open a panel" wraps to two lines in its 384 px column, each centred as `text-center`
/// centres it.
fn the_launchers_lede_centres_each_line() {
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
    let dir = std::env::temp_dir().join("desktop-gpui-pixels-launcher-no-host");
    let (state, home) = (dir.join("state.json"), [("HOME".to_owned(), dir.to_string_lossy().into_owned())].into_iter().collect());
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
                let view = cx.new(|cx| Shell::new(state, home, runtime, Launch::default(), window, cx));
                shell = Some(view.clone());
                cx.new(|cx| Root::new(view, window, cx))
            })
            .unwrap();
        let handle: AnyWindowHandle = handle.into();
        (handle, shell.unwrap())
    });
    cx.update_window(handle, |_, window, cx| shell.update(cx, |s, cx| s.panel().update(cx, |p, cx| p.toggle(window, cx)))).unwrap();
    for _ in 0..2 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
    }
    let lede = cx.update_window(handle, |_, window, _| window.find("panel-launcher-lede").bounds()).unwrap();
    let scale = cx.update_window(handle, |_, window, _| window.scale_factor()).unwrap();
    let image = cx.capture_screenshot(handle).expect("offscreen rendering must be available");
    if let Some(path) = std::env::var_os("PIXELS_LAUNCHER_PNG") {
        image.save(path).unwrap();
    }
    let (width, height) = image.dimensions();
    let shot = Capture { width, height, scale, raw: image.into_raw() };
    let (left, right) = (shot.device(f32::from(lede.left())), shot.device(f32::from(lede.right())));
    let ground = shot.device_at(left + 1, shot.device(f32::from(lede.top())));
    let ink = |x: u32, y: u32| shot.device_at(x, y).iter().zip(ground).map(|(a, b)| a.abs_diff(b) as u32).sum::<u32>() > 90;
    let rows: Vec<(u32, u32, u32)> = (shot.device(f32::from(lede.top()))..shot.device(f32::from(lede.bottom())))
        .filter_map(|y| {
            let xs: Vec<u32> = (left..right).filter(|&x| ink(x, y)).collect();
            Some((y, *xs.first()?, *xs.last()?))
        })
        .collect();
    let mut lines: Vec<(u32, u32)> = Vec::new();
    let mut last_y = None;
    for (y, first, last) in rows {
        match lines.last_mut() {
            Some(line) if last_y.is_some_and(|l: u32| y - l <= 2) => *line = (line.0.min(first), line.1.max(last)),
            _ => lines.push((first, last)),
        }
        last_y = Some(y);
    }
    assert_eq!(lines.len(), 2, "the lede wraps to two lines in {lede:?}: {lines:?}");
    let centre = (left + right) / 2;
    for (first, last) in &lines {
        let off = ((first + last) / 2).abs_diff(centre);
        // A wrapped line keeps its last space in GPUI, where Chromium hangs it, so it sits up to half a space left.
        assert!(off <= shot.device(3.), "a line runs {first} to {last} about {centre} device px, {off} off its centre");
    }
    assert!(lines[1].1 - lines[1].0 + shot.device(40.) < right - left, "the second line is short enough to show it: {lines:?}");
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

    // The person's bubble is the first row, flush with the right edge of the thread view's centred 768 px column, as
    // the web's `items-end` sets it. Across the middle of its one line the fill stands either side of the text.
    let column_right = width + (W - width + 768.) / 2.;
    let y = shot.device(top + 16. + 23.);
    let bubble: Vec<u32> = (shot.device(width)..shot.device(W)).filter(|&x| shot.device_at(x, y) == colors.bubble).collect();
    let (first, last) = (bubble.first().copied().unwrap_or(0), bubble.last().copied().unwrap_or(0));
    assert!(!bubble.is_empty(), "no bubble fill at y = {y}");
    assert!(last.abs_diff(shot.device(column_right)) <= 2, "the bubble must end at the column's edge, ends at {last}");
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
