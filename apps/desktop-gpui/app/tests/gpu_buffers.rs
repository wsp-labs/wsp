// SPDX-License-Identifier: AGPL-3.0-only
//! What a window's renderer holds beside its atlas, drawn by GPUI's offscreen renderer: the textures paths are drawn
//! through, as large as the window, and the buffer each frame's instances are written to. A window that draws no path
//! holds no path texture, and one heavy frame does not hold a heavy buffer for good once the frames after it are light.
//! Without libtest's harness, as `pixels.rs` explains.

use std::sync::Arc;

use desktop_gpui::ui::{self, Side, UiAssets};
use gpui_kit::{
    black, canvas, div, fill, point, prelude::*, px, size, AnyWindowHandle, Bounds, Context, GpuUsage, HeadlessAppContext, PathBuilder,
    Window, WindowBounds, WindowOptions,
};

/// Quads in the heavy frame: some 8 MB of instances, four times the buffer a window starts with.
const QUADS: usize = 60_000;
/// Light frames drawn after the heavy one, past what the renderer waits before it shrinks.
const LIGHT_FRAMES: usize = 90;
const INITIAL_INSTANCE_DATA: u64 = 2 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq)]
enum Look {
    Plain,
    Path,
    Heavy,
}

struct Page {
    look: Look,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let look = self.look;
        div().size_full().child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let at = bounds.origin;
                    match look {
                        Look::Plain => window.paint_quad(fill(Bounds::new(at, size(px(40.), px(20.))), black())),
                        Look::Path => {
                            let mut line = PathBuilder::stroke(px(1.5));
                            line.move_to(at + point(px(10.), px(10.)));
                            line.line_to(at + point(px(200.), px(120.)));
                            if let Ok(path) = line.build() {
                                window.paint_path(path, black());
                            }
                        }
                        Look::Heavy => {
                            for i in 0..QUADS {
                                let cell = point(px((i % 300) as f32), px((i / 300) as f32));
                                window.paint_quad(fill(Bounds::new(at + cell, size(px(1.), px(1.))), black()));
                            }
                        }
                    }
                },
            )
            .size_full(),
        )
    }
}

fn main() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let (handle, page): (AnyWindowHandle, _) = cx
        .update(|cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Default::default(), size: size(px(400.), px(300.)) })),
                focus: false,
                show: false,
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| Page { look: Look::Plain }))
        })
        .unwrap();
    // A frame as a window shows one: drawn, then rendered by the window's renderer.
    let frame = |look: Look, cx: &mut HeadlessAppContext| -> GpuUsage {
        cx.update(|cx| {
            page.update(cx, |p, cx| {
                p.look = look;
                cx.notify();
            })
        });
        cx.update_window(handle, |_, window, cx| window.draw(cx).clear(cx)).unwrap();
        cx.run_until_parked();
        cx.capture_screenshot(handle).expect("the offscreen renderer draws");
        cx.update_window(handle, |_, window, _| window.gpu_usage()).unwrap().expect("the offscreen renderer reads what it holds")
    };

    let plain = frame(Look::Plain, &mut cx);
    let path = frame(Look::Path, &mut cx);
    let after_path = frame(Look::Plain, &mut cx);
    println!(
        "path textures: {} bytes with no path drawn, {} with one, {} on the next frame without",
        plain.path_textures, path.path_textures, after_path.path_textures
    );
    assert!(path.path_textures > 0, "the frame with a path drew it through no texture");
    assert_eq!(plain.path_textures, 0, "a window that draws no path holds {} bytes of path textures", plain.path_textures);
    assert_eq!(after_path.path_textures, 0, "the path textures stayed once no frame drew a path");

    let heavy = frame(Look::Heavy, &mut cx);
    let mut light = heavy;
    for _ in 0..LIGHT_FRAMES {
        light = frame(Look::Plain, &mut cx);
    }
    println!(
        "instance data: {} bytes at first, {} after a frame of {QUADS} quads, {} after {LIGHT_FRAMES} light frames",
        plain.instance_data, heavy.instance_data, light.instance_data
    );
    assert!(heavy.instance_data > INITIAL_INSTANCE_DATA, "the heavy frame fit the buffer a window starts with");
    assert!(
        light.instance_data <= INITIAL_INSTANCE_DATA,
        "{} bytes of instance data stayed after {LIGHT_FRAMES} light frames: one heavy frame holds it for good",
        light.instance_data
    );
    println!("gpu_buffers: ok");
}
