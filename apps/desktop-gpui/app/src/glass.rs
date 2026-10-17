// SPDX-License-Identifier: AGPL-3.0-only
//! The composer's frosted glass, the web app's glass-backdrop (apps/web/src/index.css and ComposerSurface.tsx): the
//! page under it blurred 12 px and saturated 1.14 in light mode, 16 px and 1.08 in dark, under a tint of the ground
//! at 55% in light mode and of the ink at 3% in dark, inside a hairline of the ink at 10% and 9%.

use gpui_kit::component::ActiveTheme;
use gpui_kit::{div, prelude::*, px, App, Div, Pixels};

/// The composer's column, max-w-3xl, and the height of an empty composer with its tray.
pub const WIDTH: Pixels = px(768.);
pub const HEIGHT: Pixels = px(120.);
const RADIUS: Pixels = px(22.);

pub fn composer_glass(cx: &App) -> Div {
    let theme = cx.theme();
    let glass = div().w(WIDTH).h(HEIGHT).rounded(RADIUS).border_1();
    if theme.is_dark() {
        glass
            .bg(theme.foreground.opacity(0.03))
            .border_color(theme.foreground.opacity(0.09))
            .backdrop_blur(px(16.))
            .backdrop_saturation(1.08)
    } else {
        glass
            .bg(theme.background.opacity(0.55))
            .border_color(theme.foreground.opacity(0.1))
            .backdrop_blur(px(12.))
            .backdrop_saturation(1.14)
    }
}
