// SPDX-License-Identifier: AGPL-3.0-only
//! The web's short transitions: `transition-colors duration-150` on a hover's fill and ink, and a switch's thumb.
//! Each value eases toward its target from wherever it stood, in Tailwind's default curve.

use std::time::Duration;

use gpui_kit::base::animation::Lerp;
use gpui_kit::base::{transition, Easing, Transition};
use gpui_kit::{px, App, ElementId, Entity, Hsla, Pixels, Rgba, SharedString, Window};

use super::status::still;
use super::theme;

/// `duration-150`, every hover's colors.
pub const COLORS: Duration = Duration::from_millis(150);
/// `duration-200`, a switch's track.
pub const TRACK: Duration = Duration::from_millis(200);

/// A color eased as CSS eases it: in sRGB with premultiplied alpha, so a fade from transparent keeps its hue.
#[derive(Clone, Copy, PartialEq)]
struct Ink(Hsla);

impl Lerp for Ink {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        let (a, b): (Rgba, Rgba) = (self.0.to_rgb(), target.0.to_rgb());
        let alpha = a.a + (b.a - a.a) * t;
        if alpha <= 0. {
            return Ink(gpui_kit::transparent_black());
        }
        let mix = |x: f32, y: f32| (x * a.a * (1. - t) + y * b.a * t) / alpha;
        Ink(Rgba { r: mix(a.r, b.r), g: mix(a.g, b.g), b: mix(a.b, b.b), a: alpha }.into())
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Length(f32);

impl Lerp for Length {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        Length(self.0 + (target.0 - self.0) * t)
    }
}

fn policy(duration: Duration) -> Transition {
    // Tailwind's `--default-transition-timing-function`.
    Transition::new(duration).easing(Easing::cubic_bezier(0.4, 0., 0.2, 1.).expect("a valid curve"))
}

/// The channel keyed by the theme too, so a theme switch lands at once as the web's `.no-transitions` makes it.
fn channel(name: &str, cx: &App) -> SharedString {
    format!("{name}-{}", theme::current(cx).id).into()
}

/// `target`, eased over `duration` from the value the piece `id` showed for `name` last frame.
pub fn ease_ink(id: impl Into<ElementId>, name: &str, target: Hsla, duration: Duration, window: &mut Window, cx: &mut App) -> Hsla {
    if still(cx) {
        return target;
    }
    let channel = channel(name, cx);
    transition((id.into(), channel), Ink(target), policy(duration), window, cx).0
}

pub fn ease_px(id: impl Into<ElementId>, name: &str, target: Pixels, duration: Duration, window: &mut Window, cx: &mut App) -> Pixels {
    if still(cx) {
        return target;
    }
    let channel = channel(name, cx);
    px(transition((id.into(), channel), Length(target.into()), policy(duration), window, cx).0)
}

/// Whether the pointer is over the piece `id`, kept between frames: set it from the element's `on_hover`.
pub fn hovered(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> Entity<bool> {
    let id: ElementId = id.into();
    window.use_keyed_state(ElementId::from((id, "hovered")), cx, |_, _| false)
}

/// The `on_hover` listener that keeps `hovered` true while the pointer is over its element.
pub fn track_hover(state: Entity<bool>) -> impl Fn(&bool, &mut Window, &mut App) + 'static {
    move |over, _, cx| {
        let over = *over;
        state.update(cx, |s, cx| {
            if *s != over {
                *s = over;
                cx.notify();
            }
        });
    }
}
