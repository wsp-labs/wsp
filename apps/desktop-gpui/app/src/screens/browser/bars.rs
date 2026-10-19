// SPDX-License-Identifier: AGPL-3.0-only
//! The browser pane's quiet lines (`BrowserSurface.tsx`, `ui/empty.tsx`, `ZoomIndicator.tsx`): the bars over a page,
//! the empty sentence that stands where there is nothing to show, and the zoom pill over the page's top right.

use std::time::{Duration, Instant};

use gpui_kit::base::{Easing, Transition};
use gpui_kit::component::{v_flex, ActiveTheme as _};
use gpui_kit::{div, prelude::*, px, App, BoxShadow, Div, FontWeight, HighlightStyle, Pixels, SharedString, StyledText, Window};

use crate::ui::{motion, tip, ActivePalette};

/// `max-w-sm`: the empty sentence's measure.
pub const SENTENCE_WIDTH: Pixels = px(384.);
/// Tailwind's `md`, from which the empty sentence stands 48 px in rather than 24.
const WIDE: Pixels = px(768.);
/// `duration-200 ease-out`: the zoom pill coming and going.
const PILL: Duration = Duration::from_millis(200);
/// `HIDE_AFTER_MS`: how long the pill stands after a change.
pub const HOLD: Duration = Duration::from_millis(1500);

/// Draws the window again once `wait` has passed, when the pill's hold runs out.
fn wake_after(wait: Duration, window: &mut Window, cx: &mut App) {
    let pending = window.use_keyed_state("browser-zoom-pill-wake", cx, |_, _| false);
    if *pending.read(cx) {
        return;
    }
    pending.update(cx, |p, _| *p = true);
    window
        .spawn(cx, async move |cx| {
            cx.background_executor().timer(wait).await;
            _ = cx.update(|window, cx| {
                pending.update(cx, |p, _| *p = false);
                window.refresh();
            });
        })
        .detach();
}

/// One bar's frame: `px-3 py-1.5 text-xs text-muted-foreground` on `bg-muted/40`, a 60% line under.
fn bar(cx: &App) -> Div {
    let p = cx.palette();
    div()
        .flex_none()
        .w_full()
        .border_b_1()
        .border_color(p.border.opacity(0.6))
        .bg(p.muted.opacity(0.4))
        .px_3()
        .py(px(6.))
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(p.muted_foreground)
}

/// The down bar: why the port does not answer, on one line cut at its edge, the whole of it on hover.
pub fn down_bar(sentence: impl Into<SharedString>) -> impl IntoElement {
    Bar::Down(sentence.into())
}

/// The hint bar, said once a site the pane cannot frame went to a new browser tab.
pub fn hint_bar(hint: impl Into<SharedString>) -> impl IntoElement {
    Bar::Hint(hint.into())
}

/// The refusal over a kept page: its title in the medium foreground, its detail after it in the bar's ink.
pub fn refusal_bar(title: impl Into<SharedString>, detail: impl Into<SharedString>) -> impl IntoElement {
    Bar::Refusal(title.into(), detail.into())
}

#[derive(IntoElement)]
enum Bar {
    Down(SharedString),
    Hint(SharedString),
    Refusal(SharedString, SharedString),
}

impl RenderOnce for Bar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        match self {
            Bar::Down(sentence) => bar(cx)
                .id("browser-down-bar")
                .debug_selector(|| "browser-down-bar".into())
                .truncate()
                .tooltip(tip(sentence.clone()))
                .child(sentence)
                .into_any_element(),
            Bar::Hint(hint) => bar(cx).debug_selector(|| "browser-hint-bar".into()).child(hint).into_any_element(),
            Bar::Refusal(title, detail) => {
                let whole: SharedString = format!("{title} {detail}").into();
                let strong =
                    HighlightStyle { color: Some(cx.palette().foreground), font_weight: Some(FontWeight::MEDIUM), ..Default::default() };
                bar(cx)
                    .debug_selector(|| "browser-refusal-bar".into())
                    .child(StyledText::new(whole).with_highlights(vec![(0..title.len(), strong)]))
                    .into_any_element()
            }
        }
    }
}

/// The empty pane, the no-route line and the refusal card: `Empty` with its title and an optional detail.
pub fn empty_sentence(title: impl Into<SharedString>, detail: Option<SharedString>) -> EmptySentence {
    EmptySentence { title: title.into(), detail }
}

/// Centred, 13 px muted at most 384 px wide, the detail 12 px muted 4 px under it.
#[derive(IntoElement)]
pub struct EmptySentence {
    title: SharedString,
    detail: Option<SharedString>,
}

impl RenderOnce for EmptySentence {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let pad = if window.viewport_size().width >= WIDE { px(48.) } else { px(24.) };
        v_flex().debug_selector(|| "browser-empty".into()).flex_1().min_w_0().size_full().items_center().justify_center().p(pad).child(
            v_flex()
                .debug_selector(|| "browser-empty-header".into())
                .max_w(SENTENCE_WIDTH)
                .items_center()
                .text_center()
                .child(
                    div()
                        .debug_selector(|| "browser-empty-title".into())
                        .text_size(px(13.))
                        .line_height(px(20.))
                        .text_color(p.muted_foreground)
                        .child(self.title),
                )
                .children(self.detail.map(|detail| {
                    div()
                        .debug_selector(|| "browser-empty-detail".into())
                        .mt_1()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(p.muted_foreground)
                        .child(detail)
                })),
        )
    }
}

/// The zoom pill: the tab's zoom in the mono over the page's top right, for 1.5 s after `changed_at` while `shown`.
/// It fades and drops 4 px into place.
pub fn zoom_pill(zoom: f32, shown: bool, changed_at: Option<Instant>) -> impl IntoElement {
    ZoomPill { zoom, shown, changed_at }
}

#[derive(IntoElement)]
struct ZoomPill {
    zoom: f32,
    shown: bool,
    changed_at: Option<Instant>,
}

/// What the pill and the zoom row read: "120%".
pub fn percent(zoom: f32) -> SharedString {
    format!("{}%", (zoom * 100.).round() as i32).into()
}

impl RenderOnce for ZoomPill {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let left = self.changed_at.map(|at| HOLD.saturating_sub(at.elapsed())).unwrap_or_default();
        let shown = self.shown && !left.is_zero();
        if shown {
            wake_after(left, window, cx);
        }
        let curve = Transition::new(PILL).easing(Easing::cubic_bezier(0., 0., 0.2, 1.).expect("a valid curve"));
        let t = f32::from(motion::ease_px_by("browser-zoom-pill", "shown", px(if shown { 1. } else { 0. }), curve, window, cx));
        // Faded out it is not drawn at all, so it cuts no hole in the page under it.
        if t <= 0. {
            return div().into_any_element();
        }
        let p = cx.palette();
        let dark = cx.theme().is_dark();
        // `shadow-md/10`: the tooltip's `shadow-md/5` at twice its ink.
        let shadow = p.tooltip_shadow.iter().map(|s| BoxShadow { color: s.color.alpha(s.color.a * 2.), ..s.clone() }).collect();
        div()
            .id("browser-zoom-pill")
            .debug_selector(|| "browser-zoom-pill".into())
            .occlude()
            .absolute()
            .top(px(12.) - px(4.) * (1. - t))
            .right(px(12.))
            .opacity(t)
            .rounded(px(8.))
            .border_1()
            .border_color(p.border.opacity(0.7))
            .bg(p.popover.opacity(0.95))
            .when(!cfg!(target_os = "macos"), |el| {
                if dark {
                    el.backdrop_blur(px(16.)).backdrop_saturation(1.08)
                } else {
                    el.backdrop_blur(px(12.)).backdrop_saturation(1.14)
                }
            })
            .shadow(shadow)
            .px(px(10.))
            .py_1()
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(p.foreground)
            .child(percent(self.zoom))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui_kit::component::Root;
    use gpui_kit::{div, point, px, size, Context, Modifiers, MouseButton, TestAppContext, VisualTestContext, Window};

    use super::*;
    use crate::ui::{self, Side};

    const LONG: &str = ":3000 on spoo-landing is not answering; this tab reconnects when it does, and it says so here in a line long enough to run past the pane's edge";

    struct Bars {
        pill: Option<Instant>,
        reached: Rc<Cell<u32>>,
    }

    impl Render for Bars {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let reached = self.reached.clone();
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(down_bar(LONG))
                .child(refusal_bar(":3000 answered 403 through the preview route", "The server refused it."))
                .child(
                    div()
                        .debug_selector(|| "page".into())
                        .relative()
                        .flex_1()
                        .on_mouse_down(MouseButton::Left, move |_, _, _| reached.set(reached.get() + 1))
                        .child(zoom_pill(1.2, true, self.pill)),
                )
        }
    }

    fn open<V: Render>(cx: &mut TestAppContext, width: f32, view: impl FnOnce(&mut Context<V>) -> V) -> &mut VisualTestContext {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
            cx.set_global(ui::Still(true));
        });
        let view = cx.new(view);
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(view, window, cx));
        cx.simulate_resize(size(px(width), px(400.)));
        cx.run_until_parked();
        cx
    }

    #[gpui_kit::test]
    fn the_down_bar_is_one_cut_line_on_its_padding(cx: &mut TestAppContext) {
        let cx = open(cx, 360., |_| Bars { pill: None, reached: Rc::new(Cell::new(0)) });
        let bar = cx.debug_bounds("browser-down-bar").expect("the down bar is drawn");
        assert_eq!(bar.size.height, px(29.), "a 16 px line, 6 px above and below, and the 1 px line under, however long");
        assert_eq!(bar.size.width, px(360.));
        let refusal = cx.debug_bounds("browser-refusal-bar").expect("the refusal bar is drawn");
        assert_eq!(refusal.origin.y, bar.bottom(), "the refusal bar stands under the down bar");
    }

    #[gpui_kit::test]
    fn the_zoom_pill_stands_12_px_in_and_covers_the_page(cx: &mut TestAppContext) {
        let reached = Rc::new(Cell::new(0));
        let shared = reached.clone();
        let cx = open(cx, 480., move |_| Bars { pill: Some(Instant::now()), reached: shared });
        let page = cx.debug_bounds("page").expect("the page slot is drawn");
        let pill = cx.debug_bounds("browser-zoom-pill").expect("the pill shows while shown");
        assert_eq!((page.right() - pill.right(), pill.top() - page.top()), (px(12.), px(12.)), "top right, 12 px in");
        assert_eq!(pill.size.height, px(26.), "a 16 px line, 4 px above and below, in its 1 px edge");
        cx.simulate_click(pill.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(reached.get(), 0, "the pill occludes, so the page under it does not take the press");
        cx.simulate_click(point(page.left() + px(20.), page.bottom() - px(20.)), Modifiers::none());
        assert_eq!(reached.get(), 1, "beside the pill the page takes it");
    }

    #[gpui_kit::test]
    fn the_zoom_pill_goes_1_5_s_after_the_change(cx: &mut TestAppContext) {
        let cx = open(cx, 480., |_| Bars { pill: Some(Instant::now()), reached: Rc::new(Cell::new(0)) });
        assert!(cx.debug_bounds("browser-zoom-pill").is_some());
        std::thread::sleep(Duration::from_millis(1600));
        cx.executor().advance_clock(Duration::from_millis(1600));
        cx.run_until_parked();
        assert!(cx.debug_bounds("browser-zoom-pill").is_none(), "past its hold the pill is gone");
    }

    #[gpui_kit::test]
    fn a_hidden_zoom_pill_is_not_drawn(cx: &mut TestAppContext) {
        let cx = open(cx, 480., |_| Bars { pill: None, reached: Rc::new(Cell::new(0)) });
        assert!(cx.debug_bounds("browser-zoom-pill").is_none(), "no pill, and no hole cut in the page");
        assert_eq!(percent(1.2), "120%");
        assert_eq!(percent(0.5), "50%");
    }

    struct Sentence;

    impl Render for Sentence {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().flex().child(
                div().debug_selector(|| "slot".into()).w(px(400.)).h_full().flex().flex_col().child(
                    empty_sentence(
                        ":3000 refused the preview host",
                        Some("Add the preview host to server.allowedHosts in vite.config.ts, then restart the dev server, and this pane loads the page.".into()),
                    ),
                ),
            )
        }
    }

    #[gpui_kit::test]
    fn the_empty_sentence_is_centred_with_its_detail_4_px_under(cx: &mut TestAppContext) {
        let cx = open(cx, 600., |_| Sentence);
        let empty = cx.debug_bounds("browser-empty").expect("the sentence is drawn");
        let header = cx.debug_bounds("browser-empty-header").expect("its header is drawn");
        let (title, detail) = (cx.debug_bounds("browser-empty-title").unwrap(), cx.debug_bounds("browser-empty-detail").unwrap());
        assert_eq!(header.center().x, empty.center().x, "centred");
        assert_eq!(header.center().y, empty.center().y, "centred");
        assert_eq!(detail.top() - title.bottom(), px(4.), "the detail stands 4 px under the title");
        assert_eq!(title.size.height, px(20.), "13 px on a 20 px line, the title short enough for one");
    }

    #[gpui_kit::test]
    fn the_empty_sentence_keeps_24_px_and_48_from_768_wide(cx: &mut TestAppContext) {
        let cx = open(cx, 600., |_| Sentence);
        let header = cx.debug_bounds("browser-empty-header").unwrap();
        assert_eq!(header.size.width, px(400. - 48.), "under 768 px wide, 24 px each side of a 400 px slot");
        cx.simulate_resize(size(px(800.), px(400.)));
        cx.run_until_parked();
        let header = cx.debug_bounds("browser-empty-header").unwrap();
        assert_eq!(header.size.width, px(400. - 96.), "from 768 px wide, 48 px each side");
    }

    #[gpui_kit::test]
    fn the_empty_sentence_is_at_most_384_wide(cx: &mut TestAppContext) {
        let cx = open(cx, 1200., |_| SentenceWide);
        let header = cx.debug_bounds("browser-empty-header").unwrap();
        assert_eq!(header.size.width, SENTENCE_WIDTH, "max-w-sm, the long detail filling it");
    }

    struct SentenceWide;

    impl Render for SentenceWide {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().flex().flex_col().child(empty_sentence(
                "No preview yet.",
                Some("Add the preview host to server.allowedHosts in vite.config.ts, then restart the dev server, and this pane loads the page.".into()),
            ))
        }
    }
}
