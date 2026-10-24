// SPDX-License-Identifier: AGPL-3.0-only
//! The selection toolbar (`AssistantSelectionToolbar.tsx`) and the scroll-to-end pill over the transcript.

use std::cell::Cell;
use std::rc::Rc;

use std::time::Duration;

use gpui_kit::component::{h_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    anchored, canvas, deferred, div, point, prelude::*, px, Animation, AnimationExt as _, AnyElement, Bounds, BoxShadow, Context,
    DispatchPhase, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point, SharedString, Window,
};

use crate::thread_view::{ThreadView, ThreadViewEvent};
use crate::transcript::RowKind;
use crate::ui::{ActivePalette, Button, Glyph};

/// How many characters of the prompt a quote's header names its reply by, `REPLY_TO_CHARS`.
const REPLY_TO_CHARS: usize = 60;
/// The least room the button keeps from the window's edges.
const MARGIN: Pixels = px(8.);

/// The button on screen: what it quotes and where it stands.
pub struct Quote {
    pub text: String,
    pub reply_to: Option<String>,
    pub at: Point<Pixels>,
    key: String,
    scroll: Pixels,
    drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `quoteText`: a header naming the prompt the reply answered, then the selection with every line quoted.
pub fn quote_text(text: &str, reply_to: Option<&str>) -> String {
    let named = reply_to.map(|r| one_line(r).replace('"', "'")).unwrap_or_default();
    let named = if named.chars().count() > REPLY_TO_CHARS {
        format!("{}\u{2026}", named.chars().take(REPLY_TO_CHARS - 1).collect::<String>())
    } else {
        named
    };
    let header = if named.is_empty() { "Quoting your reply:".to_owned() } else { format!("Quoting your reply to \"{named}\":") };
    let body = text
        .trim()
        .replace("\r\n", "\n")
        .split('\n')
        .map(|l| if l.is_empty() { ">".to_owned() } else { format!("> {l}") })
        .collect::<Vec<_>>()
        .join("\n");
    format!("{header}\n{body}")
}

/// `resolveSelectionActionPosition`: the pointer, held inside the transcript's box and the window's 8 px margin.
pub fn place(pointer: Point<Pixels>, bounds: Bounds<Pixels>, window: gpui_kit::Size<Pixels>) -> Point<Pixels> {
    let x = pointer.x.max(bounds.left()).min(bounds.right()).min(window.width - MARGIN).max(MARGIN);
    let y = pointer.y.max(bounds.top()).min(bounds.bottom()).min(window.height - MARGIN).max(MARGIN);
    point(x, y)
}

/// The mouse came up: once the text views have taken the release, a selection inside one reply shows the button.
pub fn released(view: &mut ThreadView, pointer: Point<Pixels>, window: &mut Window, cx: &mut Context<ThreadView>) {
    let selected: Vec<(String, String)> = view
        .bodies()
        .iter()
        .map(|(key, body)| (key.clone(), body.read(cx).selected_text()))
        .filter(|(_, text)| !text.trim().is_empty())
        .collect();
    let quote = match selected.as_slice() {
        [(key, text)] => {
            let rows = view.rows();
            let at = rows.iter().position(|r| &r.key == key);
            let reply = at.filter(|&ix| matches!(rows[ix].kind, RowKind::Agent { .. }));
            reply.map(|ix| {
                let reply_to = rows[..ix].iter().rev().find(|r| matches!(r.kind, RowKind::Person { .. })).map(|r| r.text.clone());
                let at = place(pointer, view.list().viewport_bounds(), window.viewport_size());
                Quote {
                    text: text.trim().to_owned(),
                    reply_to: reply_to.filter(|r| !r.is_empty()),
                    at,
                    key: key.clone(),
                    scroll: view.list().scroll_px_offset_for_scrollbar().y,
                    drawn: Rc::default(),
                }
            })
        }
        _ => None,
    };
    *view.quote() = quote;
    cx.notify();
}

/// Quote: hands the composer the quote, lets go of the selection and puts the button away.
pub fn quote(view: &mut ThreadView, cx: &mut Context<ThreadView>) {
    let Some(q) = view.quote().take() else { return };
    if let Some(body) = view.bodies().get(&q.key).cloned() {
        body.update(cx, |b, cx| b.clear_selection(cx));
    }
    cx.emit(ThreadViewEvent::Quote(quote_text(&q.text, q.reply_to.as_deref())));
    cx.notify();
}

/// The window's own mouse listeners, set while the view paints: a release anywhere may finish a selection.
pub fn listen(view: gpui_kit::WeakEntity<ThreadView>, window: &mut Window) {
    let up = view.clone();
    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
            return;
        }
        let (view, pointer) = (up.clone(), event.position);
        window.defer(cx, move |window, cx| {
            _ = view.update(cx, |view, cx| released(view, pointer, window, cx));
        });
    });
    window.on_mouse_event(move |event: &MouseDownEvent, phase, _, cx| {
        if phase != DispatchPhase::Capture {
            return;
        }
        _ = view.update(cx, |view, cx| {
            let off = view.quote().as_ref().is_some_and(|q| !q.drawn.get().is_some_and(|b| b.contains(&event.position)));
            if off {
                *view.quote() = None;
                cx.notify();
            }
        });
    });
}

/// The button, or nothing; a scroll since it showed puts it away.
pub fn toolbar(view: &mut ThreadView, cx: &mut Context<ThreadView>) -> Option<AnyElement> {
    let scroll = view.list().scroll_px_offset_for_scrollbar().y;
    if view.quote().as_ref().is_some_and(|q| q.scroll != scroll) {
        *view.quote() = None;
    }
    let q = view.quote().as_ref()?;
    let drawn = q.drawn.clone();
    Some(
        deferred(
            anchored().position(q.at).snap_to_window_with_margin(MARGIN).child(
                div()
                    .debug_selector(|| "quote".into())
                    .child(
                        Button::outline("quote-selection", "Quote")
                            .xs()
                            .glyph(Glyph::Quote)
                            .on_click(cx.listener(|view, _, _, cx| quote(view, cx))),
                    )
                    .child(canvas(move |bounds, _, _| drawn.set(Some(bounds)), |_, _, _, _| {}).absolute().inset_0()),
            ),
        )
        .with_priority(2)
        .into_any_element(),
    )
}

/// Tailwind's `ease-out`, `cubic-bezier(0, 0, 0.2, 1)`.
pub fn ease_out(t: f32) -> f32 {
    gpui_kit::base::Easing::cubic_bezier(0., 0., 0.2, 1.).expect("a valid curve").sample(t)
}

/// The ease of Chromium's smooth scroll, CSS's `ease-in-out`.
pub fn ease_in_out(t: f32) -> f32 {
    gpui_kit::base::Easing::EaseInOut.sample(t)
}

/// How long the pill takes to fade and slide, `duration-200`, and how far it slides, `translate-y-1`.
pub const PILL_FADE: Duration = Duration::from_millis(200);
const PILL_SLIDE: f32 = 4.;
/// The pill's foot over the composer's top edge, `pb-2`.
pub const PILL_GAP: Pixels = px(8.);

/// `ScrollToEnd` (ChatView.tsx); once shown it stays drawn, clear and inert, so it can fade out.
pub fn scroll_to_end(view: &mut ThreadView, shown: bool, cx: &mut Context<ThreadView>) -> Option<AnyElement> {
    let inset = view.composer_inset();
    // A page read in leaves rows of unknown height, over which the list cannot say it stands short of its end.
    let away = shown && !view.list().is_following_tail() && !view.near_end();
    let was = *view.pill();
    if !away && was.is_none() {
        return None;
    }
    *view.pill() = Some(away);
    let p = cx.palette().clone();
    let dark = cx.theme().is_dark();
    let shadow = vec![
        BoxShadow {
            color: gpui_kit::black().opacity(0.45),
            offset: point(px(0.), px(8.)),
            blur_radius: px(20.),
            spread_radius: px(-8.),
            inset: false,
        },
        BoxShadow {
            color: gpui_kit::black().opacity(0.3),
            offset: point(px(0.), px(2.)),
            blur_radius: px(4.),
            spread_radius: px(-2.),
            inset: false,
        },
    ];
    let pill = h_flex()
        .id("scroll-to-end")
        .debug_selector(|| "scroll-to-end".into())
        .h(px(28.))
        .gap(px(6.))
        .px(px(12.))
        .rounded_full()
        .border_1()
        .border_color(p.border)
        .bg(p.popover.opacity(0.95))
        .shadow(shadow)
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(p.muted_foreground)
        .when(!cfg!(target_os = "macos"), |el| {
            if dark {
                el.backdrop_blur(px(16.)).backdrop_saturation(1.08)
            } else {
                el.backdrop_blur(px(12.)).backdrop_saturation(1.14)
            }
        })
        .child(Icon::new(Glyph::ArrowDown).with_size(px(14.)))
        .child("Scroll to end")
        .when(away, |el| {
            el.cursor_pointer().hover(|s| s.text_color(p.foreground)).on_click(cx.listener(|view, _, _, cx| view.glide_to_end(cx)))
        });
    let (from, to) = if away { (0., 1.) } else { (1., 0.) };
    let id = SharedString::from(if away { "scroll-to-end-in" } else { "scroll-to-end-out" });
    let pill = pill.with_animation(id, Animation::new(PILL_FADE).with_easing(ease_out), move |el, t| {
        let shown = from + (to - from) * t;
        el.opacity(shown).top(px(PILL_SLIDE * (1. - shown)))
    });
    Some(
        h_flex()
            .absolute()
            .left_0()
            .right_0()
            .bottom(inset + PILL_GAP)
            .justify_center()
            .child(div().relative().child(pill))
            .into_any_element(),
    )
}

/// Escape puts the button away.
pub fn dismiss(view: &mut ThreadView, cx: &mut Context<ThreadView>) -> bool {
    if view.quote().take().is_some() {
        cx.notify();
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::size;

    #[test]
    fn a_quote_names_the_prompt_and_quotes_every_line() {
        assert_eq!(
            quote_text("  one\n\ntwo ", Some("Fix the \"bug\"\nnow")),
            "Quoting your reply to \"Fix the 'bug' now\":\n> one\n>\n> two"
        );
        assert_eq!(quote_text("x", None), "Quoting your reply:\n> x");
        let long = "a".repeat(70);
        assert_eq!(quote_text("x", Some(&long)), format!("Quoting your reply to \"{}\u{2026}\":\n> x", "a".repeat(59)));
    }

    #[test]
    fn ease_out_starts_fast_and_ends_at_one() {
        assert!(ease_out(0.) < 1e-4);
        assert!((ease_out(1.) - 1.).abs() < 1e-4);
        assert!(ease_out(0.5) > 0.6);
    }

    #[test]
    fn the_button_stands_at_the_pointer_inside_the_transcript_and_the_window() {
        let bounds = Bounds::new(point(px(256.), px(48.)), size(px(1024.), px(752.)));
        let window = size(px(1280.), px(800.));
        assert_eq!(place(point(px(400.), px(300.)), bounds, window), point(px(400.), px(300.)));
        assert_eq!(place(point(px(100.), px(10.)), bounds, window), point(px(256.), px(48.)));
        assert_eq!(place(point(px(1279.), px(799.)), bounds, window), point(px(1272.), px(792.)));
    }
}
