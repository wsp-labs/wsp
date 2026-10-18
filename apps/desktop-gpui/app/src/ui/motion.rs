// SPDX-License-Identifier: AGPL-3.0-only
//! The web's short transitions: `transition-colors duration-150` on a hover's fill and ink, and a switch's thumb.
//! Each value eases toward its target from wherever it stood, in Tailwind's default curve.

use std::time::Duration;

use gpui_kit::base::animation::Lerp;
use gpui_kit::base::{transition, Easing, Transition};
use gpui_kit::{
    prelude::*, px, Animation, AnimationExt as _, AnyElement, App, ElementId, Entity, Hsla, Pixels, Rgba, SharedString, Window,
};

use super::status::still;
use super::theme;

/// `duration-150`, every hover's colors.
pub const COLORS: Duration = Duration::from_millis(150);
/// `duration-200`, a switch's track.
pub const TRACK: Duration = Duration::from_millis(200);
/// `duration-200 ease-in-out`, a dialog's popup and backdrop as they open.
pub const DIALOG: Duration = Duration::from_millis(200);

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
    Transition::new(duration).easing(tailwind_ease())
}

/// Tailwind's `--ease-in-out`, which is its `--default-transition-timing-function` too; not CSS's own `ease-in-out`.
fn tailwind_ease() -> Easing {
    Easing::cubic_bezier(0.4, 0., 0.2, 1.).expect("a valid curve")
}

/// The channel keyed by the theme too, so a theme switch lands at once as the web's `.no-transitions` makes it.
fn channel(name: &str, cx: &App) -> SharedString {
    format!("{name}-{}", theme::current(cx).id).into()
}

/// `target`, eased over `duration` from the value the piece `id` showed for `name` last frame.
pub fn ease_ink(id: impl Into<ElementId>, name: &str, target: Hsla, duration: Duration, window: &mut Window, cx: &mut App) -> Hsla {
    ease_ink_by(id, name, target, policy(duration), window, cx)
}

pub fn ease_px(id: impl Into<ElementId>, name: &str, target: Pixels, duration: Duration, window: &mut Window, cx: &mut App) -> Pixels {
    ease_px_by(id, name, target, policy(duration), window, cx)
}

/// `ease-[cubic-bezier(0.23,1,0.32,1)]`: a figure's digits rolling and a meter's fill.
pub fn snappy(duration: Duration) -> Transition {
    Transition::new(duration).easing(Easing::cubic_bezier(0.23, 1., 0.32, 1.).expect("a valid curve"))
}

/// `ease_ink` in a timing of the caller's own.
pub fn ease_ink_by(id: impl Into<ElementId>, name: &str, target: Hsla, timing: Transition, window: &mut Window, cx: &mut App) -> Hsla {
    if still(cx) {
        return target;
    }
    let channel = channel(name, cx);
    transition((id.into(), channel), Ink(target), timing, window, cx).0
}

/// `ease_px` in a timing of the caller's own.
pub fn ease_px_by(id: impl Into<ElementId>, name: &str, target: Pixels, timing: Transition, window: &mut Window, cx: &mut App) -> Pixels {
    if still(cx) {
        return target;
    }
    let channel = channel(name, cx);
    px(transition((id.into(), channel), Length(target.into()), timing, window, cx).0)
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

/// `el` fading in over `DIALOG` from the first frame it is drawn, as `data-starting-style` opens a dialog's popup and
/// backdrop; at once under reduced motion. GPUI has no scale for a box and its text, so the popup's `scale-98` is not
/// drawn.
pub fn open_fade<E: Styled + IntoElement + 'static>(id: impl Into<ElementId>, el: E, cx: &App) -> AnyElement {
    if still(cx) {
        return el.into_any_element();
    }
    let curve = tailwind_ease();
    el.with_animation(id, Animation::new(DIALOG).with_easing(move |t| curve.sample(t)), |el, t| el.opacity(t)).into_any_element()
}

/// `animation: slate-piece-in 150ms ease-out`, a slate piece fading in once as it first draws.
pub const PIECE_IN: Duration = Duration::from_millis(150);
/// `transition-[height] duration-200`, a fold's panel.
pub const FOLD: Duration = Duration::from_millis(200);
/// `data-hovering:duration-100`, a scrollbar coming in.
pub const BAR_IN: Duration = Duration::from_millis(100);
/// `delay-300` and Tailwind's default 150 ms, a scrollbar going out.
pub const BAR_OUT: (Duration, Duration) = (Duration::from_millis(300), Duration::from_millis(150));

/// `el` fading in over `PIECE_IN` from the first frame it is drawn under `id`; at once under reduced motion.
pub fn fade_in<E: Styled + IntoElement + 'static>(id: impl Into<ElementId>, el: E, cx: &App) -> AnyElement {
    if still(cx) {
        return el.into_any_element();
    }
    let curve = Easing::EaseOut;
    el.with_animation(id, Animation::new(PIECE_IN).with_easing(move |t| curve.sample(t)), |el, t| el.opacity(t)).into_any_element()
}

/// A fold's panel: its height eased between 0 and the body's own over `FOLD`, the body drawn through the close and
/// at its own height once open, so a body that grows is never clipped. None once shut, and the body is not built.
pub fn fold(
    id: impl Into<ElementId>,
    open: bool,
    body: impl FnOnce(&mut Window, &mut App) -> AnyElement,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let id: ElementId = id.into();
    let t = f32::from(ease_px_by(id.clone(), "fold", px(if open { 1. } else { 0. }), policy(FOLD), window, cx));
    if !open && t <= 0. {
        return None;
    }
    let body = body(window, cx);
    let height = window.use_keyed_state(ElementId::from((id, "fold-height")), cx, |_, _| None::<Pixels>);
    let natural = *height.read(cx);
    let probe = gpui_kit::canvas(
        move |bounds, _, cx| {
            if *height.read(cx) != Some(bounds.size.height) {
                height.update(cx, |h, cx| {
                    *h = Some(bounds.size.height);
                    cx.notify();
                });
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full();
    let inner = gpui_kit::div().relative().w_full().flex_none().child(body).child(probe);
    if open && t >= 1. {
        return Some(inner.into_any_element());
    }
    Some(gpui_kit::div().w_full().overflow_hidden().h(natural.unwrap_or_default() * t).child(inner).into_any_element())
}

/// A scrollbar's opacity: in over `BAR_IN` while `shown`, out over `BAR_OUT` after its delay; at once under reduced
/// motion.
pub fn bar_opacity(id: impl Into<ElementId>, shown: bool, window: &mut Window, cx: &mut App) -> f32 {
    let curve = || Easing::cubic_bezier(0.4, 0., 0.2, 1.).expect("a valid curve");
    let timing = if shown { Transition::new(BAR_IN).easing(curve()) } else { Transition::new(BAR_OUT.1).delay(BAR_OUT.0).easing(curve()) };
    f32::from(ease_px_by(id, "bar", px(if shown { 1. } else { 0. }), timing, window, cx))
}

#[cfg(test)]
mod tests {
    /// `duration-200 ease-in-out` in Tailwind 4 is `cubic-bezier(0.4, 0, 0.2, 1)`: three quarters in by half time,
    /// where CSS's own `ease-in-out` is at half.
    #[test]
    fn a_dialog_fades_on_tailwinds_ease_in_out() {
        let half = super::tailwind_ease().sample(0.5);
        assert!((half - 0.776).abs() < 0.01, "the dialog's fade at half time: {half}");
    }
}
