// SPDX-License-Identifier: AGPL-3.0-only
//! A shell's view drawn by GPUI's real offscreen renderer in Paper and Graphite, fed a fixed stream: a prompt, the
//! sixteen colours, bold and underline, an address and a wide character. It checks the ground, a coloured cell and the
//! block cursor by their pixels, and `TERMINAL_PNG=<folder>` writes each capture there as `render-<theme>.png`.
//! Without libtest's harness, as `pixels.rs` explains.

use std::sync::Arc;

use desktop_gpui::screens::terminal::look::{hsla_of, Look};
use desktop_gpui::screens::terminal::session::Session;
use desktop_gpui::screens::terminal::view::TerminalView;
use desktop_gpui::ui::{self, Side, UiAssets};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    px, size, AnyWindowHandle, AppContext, Bounds, Entity, EntityInputHandler, Focusable, HeadlessAppContext, Rgba, WindowBounds,
    WindowOptions,
};

const W: f32 = 900.;
const H: f32 = 280.;

const STREAM: &[u8] = b"\x1b[1;32mana@wsp\x1b[0m:\x1b[34m~/wsp\x1b[0m$ ls --color\r\n\
\x1b[30m 0 \x1b[31m 1 \x1b[32m 2 \x1b[33m 3 \x1b[34m 4 \x1b[35m 5 \x1b[36m 6 \x1b[37m 7 \x1b[0m\r\n\
\x1b[90m 8 \x1b[91m 9 \x1b[92m10 \x1b[93m11 \x1b[94m12 \x1b[95m13 \x1b[96m14 \x1b[97m15 \x1b[0m\r\n\
\x1b[41m red ground \x1b[0m \x1b[1mbold\x1b[0m \x1b[4munderline\x1b[0m \x1b[7minverse\x1b[0m\r\n\
docs at https://wsp.dev/docs and \xe4\xb8\x96\xe7\x95\x8c wide\r\n\
\x1b[1;32mana@wsp\x1b[0m:\x1b[34m~/wsp\x1b[0m$ ";

struct Shot {
    width: u32,
    scale: f32,
    raw: Vec<u8>,
}

impl Shot {
    fn at(&self, x: f32, y: f32) -> [u8; 3] {
        let (x, y) = ((x * self.scale).round() as u32, (y * self.scale).round() as u32);
        let i = ((y * self.width + x) * 4) as usize;
        [self.raw[i], self.raw[i + 1], self.raw[i + 2]]
    }
}

fn rgb(c: impl Into<Rgba>) -> [u8; 3] {
    let c: Rgba = c.into();
    [c.r, c.g, c.b].map(|v| (v * 255.).round() as u8)
}

fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(a, b)| a.abs_diff(b) <= 3)
}

fn shoot(theme: &str) {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    let dark = theme != "paper";
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(if dark { Side::Dark } else { Side::Light }, cx);
        ui::theme::set_theme(theme, cx);
    });
    let mut made: Option<Entity<TerminalView>> = None;
    let handle: AnyWindowHandle = cx
        .update(|cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Default::default(), size: size(px(W), px(H)) })),
                focus: true,
                show: false,
                ..Default::default()
            };
            cx.open_window(options, |window, cx| {
                let session = cx.new(|_| Session::new("pty_1", "shell 1", 80, 24));
                let look = Look::new(None, None, cx);
                let view = cx.new(|cx| TerminalView::new(session, look, window, cx));
                made = Some(view.clone());
                cx.new(|cx| Root::new(view, window, cx))
            })
        })
        .unwrap()
        .into();
    let view = made.unwrap();
    for _ in 0..2 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
    }
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |v, cx| {
            v.focus_handle(cx).focus(window, cx);
            v.session().clone().update(cx, |s, cx| s.feed(STREAM, cx));
        })
    })
    .unwrap();
    for _ in 0..3 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
    }
    let image = cx.capture_screenshot(handle).expect("offscreen rendering must be available");
    let scale = cx.update_window(handle, |_, window, _| window.scale_factor()).unwrap();
    if let Some(dir) = std::env::var_os("TERMINAL_PNG") {
        std::fs::create_dir_all(&dir).unwrap();
        image.save(std::path::Path::new(&dir).join(format!("render-{theme}.png"))).unwrap();
    }
    let (frame, look, cursor_cell, red_ground) = cx
        .update_window(handle, |_, _, cx| {
            let v = view.read(cx);
            let cursor = v.frame().cursor.expect("the prompt's cursor shows");
            (v.frame().clone(), v.look().clone(), v.cell_bounds(cursor.x, cursor.y).unwrap(), v.cell_bounds(0, 3).unwrap())
        })
        .unwrap();
    let shot = Shot { width: image.width(), scale, raw: image.into_raw() };
    let ground = rgb(hsla_of(frame.background));
    assert!(
        near(shot.at(W - 20., H - 10.), ground),
        "{theme}: the ground under the last row: {:?} against {ground:?}",
        shot.at(W - 20., H - 10.)
    );
    let red = rgb(hsla_of(look.colors.palette[1].1));
    let at = red_ground.center();
    assert!(
        near(shot.at(f32::from(at.x), f32::from(at.y)), red),
        "{theme}: SGR 41 fills its cell with slot 1: {:?}",
        shot.at(f32::from(at.x), f32::from(at.y))
    );
    let cursor = rgb(hsla_of(frame.cursor.unwrap().color));
    let c = cursor_cell.center();
    assert!(
        near(shot.at(f32::from(c.x), f32::from(c.y)), cursor),
        "{theme}: the block cursor fills its cell: {:?}",
        shot.at(f32::from(c.x), f32::from(c.y))
    );
    let before = point_left(&cursor_cell);
    assert!(!near(shot.at(before.0, before.1), cursor), "{theme}: and only its cell");
    let next = (f32::from(c.x) + f32::from(cursor_cell.size.width), f32::from(c.y));
    assert!(near(shot.at(next.0, next.1), ground), "{theme}: the cell past the cursor is bare ground");

    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |v, cx| v.replace_and_mark_text_in_range(None, "\u{2588}\u{2588}", None, window, cx))
    })
    .unwrap();
    for _ in 0..2 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
    }
    let image = cx.capture_screenshot(handle).expect("offscreen rendering must be available");
    let shot = Shot { width: image.width(), scale, raw: image.into_raw() };
    let ink = rgb(hsla_of(frame.foreground));
    assert!(
        near(shot.at(next.0, next.1), ink),
        "{theme}: a composition draws at the cursor before it commits: {:?} against {ink:?}",
        shot.at(next.0, next.1)
    );
    println!("test {theme} ... ok");
}

fn point_left(b: &Bounds<gpui_kit::Pixels>) -> (f32, f32) {
    (f32::from(b.left()) - f32::from(b.size.width) / 2., f32::from(b.center().y))
}

fn main() {
    shoot("paper");
    shoot("graphite");
}
