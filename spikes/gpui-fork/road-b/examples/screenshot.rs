// SPDX-License-Identifier: AGPL-3.0-only
use gpui::{AppContext, HeadlessAppContext, px, size};
use road_b::Spike;

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "screenshot.png".into());
    let platform = gpui_platform::current_platform(true);
    let mut cx =
        HeadlessAppContext::with_platform(platform.text_system(), std::sync::Arc::new(()), || {
            Some(
                Box::new(gpui_wgpu::WgpuHeadlessRenderer::new().expect("no headless wgpu device"))
                    as Box<dyn gpui::PlatformHeadlessRenderer>,
            )
        });
    let window = cx
        .open_window(size(px(1100.), px(720.)), |_, cx| cx.new(|_| Spike))
        .expect("failed to open headless window");
    cx.run_until_parked();
    let image = cx
        .capture_screenshot(window.into())
        .expect("capture failed");
    image.save(&out).expect("failed to write png");
    println!("wrote {out} ({}x{})", image.width(), image.height());
}
