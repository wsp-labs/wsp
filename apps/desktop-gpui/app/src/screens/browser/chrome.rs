// SPDX-License-Identifier: AGPL-3.0-only
//! The browser pane's chrome row (`PreviewChromeRow.tsx`, `BrowserSurface.tsx`): Back, Forward and Refresh, the
//! address bar, then Open on laptop, Copy URL and the More menu, with the loading bar along its bottom edge.

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::base::Easing;
use gpui_kit::component::{h_flex, Icon, Sizable as _};
use gpui_kit::{div, point, prelude::*, px, relative, AnyElement, App, BoxShadow, ElementId, Entity, Hsla, SharedString, Window};

use super::field::AddressField;
use crate::ui::controls::{bevel, Dim};
use crate::ui::focus::ring;
use crate::ui::status::{still, turning};
use crate::ui::{motion, size, tip, ActivePalette, Button, Glyph};

pub type Act = Rc<dyn Fn(&mut Window, &mut App)>;

/// The row's height, and its height inline over the page, which keeps 12 px under it.
pub const ROW_HEIGHT: gpui_kit::Pixels = px(40.);
pub const INLINE_HEIGHT: gpui_kit::Pixels = px(28.);

/// `preview-loading-progress`: 0.04 to 0.9 over 5.3 s while loading; once done, to 1 over 150 ms and faded over
/// 150 ms after 220 ms.
const GROW: Duration = Duration::from_millis(5300);
const FILL: Duration = Duration::from_millis(150);
const FADE_DELAY: Duration = Duration::from_millis(220);
const FADE: Duration = Duration::from_millis(150);
/// Where the bar stands while loading under reduced motion.
const HELD_AT: f32 = 0.9;

/// The 24 px icon button in the looks `ui::Button` does not draw: a glyph that turns while the button stays live, a
/// glyph in its own ink, the outline keycap at the icon size, and a ghost whose hover is a fill of its own.
#[derive(IntoElement)]
pub(super) struct IconKey {
    id: SharedString,
    selector: &'static str,
    glyph: Glyph,
    outline: bool,
    ink: Option<Hsla>,
    hover: Option<Hsla>,
    turning: bool,
    held: bool,
    tooltip: Option<SharedString>,
    on_click: Option<Act>,
}

impl IconKey {
    pub(super) fn new(id: impl Into<SharedString>, selector: &'static str, glyph: Glyph) -> Self {
        IconKey {
            id: id.into(),
            selector,
            glyph,
            outline: false,
            ink: None,
            hover: None,
            turning: false,
            held: false,
            tooltip: None,
            on_click: None,
        }
    }

    pub(super) fn outline(mut self) -> Self {
        self.outline = true;
        self
    }

    pub(super) fn ink(mut self, ink: Hsla) -> Self {
        self.ink = Some(ink);
        self
    }

    pub(super) fn hover(mut self, fill: Hsla) -> Self {
        self.hover = Some(fill);
        self
    }

    pub(super) fn turning(mut self) -> Self {
        self.turning = true;
        self
    }

    pub(super) fn held(mut self, held: bool) -> Self {
        self.held = held;
        self
    }

    pub(super) fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    pub(super) fn on_click(mut self, act: Act) -> Self {
        self.on_click = Some(act);
        self
    }
}

impl RenderOnce for IconKey {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let clear = gpui_kit::transparent_black();
        let (rest, over) = if self.outline {
            ((p.input_fill, p.input), (p.input_fill_hover, p.input))
        } else {
            ((clear, clear), (self.hover.unwrap_or(p.accent), clear))
        };
        let hovered = motion::hovered(self.id.clone(), window, cx);
        let (fill, edge) = if *hovered.read(cx) && !self.held { over } else { rest };
        let mut keycap = bevel(&p);
        let glyph_ink = self.ink.unwrap_or(p.muted_foreground);
        let (fill, edge, glyph_ink) = if self.held {
            let dim = Dim::new(p.background, fill);
            keycap[0].color = dim.over(p.keycap_top);
            let edge = if edge.a > 0. { dim.over(edge) } else { edge };
            let fill = if fill.a > 0. { dim.fill() } else { fill };
            (fill, edge, dim.over(glyph_ink))
        } else {
            let fill = motion::ease_ink(self.id.clone(), "fill", fill, motion::COLORS, window, cx);
            (fill, motion::ease_ink(self.id.clone(), "edge", edge, motion::COLORS, window, cx), glyph_ink)
        };
        let icon = Icon::new(self.glyph).with_size(px(14.)).text_color(glyph_ink);
        let icon = if self.turning { turning(icon, window, cx) } else { icon };
        let selector = self.selector;
        h_flex()
            .id(self.id)
            .debug_selector(move || selector.into())
            .relative()
            .flex_none()
            .justify_center()
            .size(px(24.))
            .rounded(size::BUTTON_RADIUS)
            .border_1()
            .border_color(edge)
            .bg(fill)
            // GPUI paints an inset shadow under the border, where CSS keeps it inside; a child filling the padding box holds it there.
            .when(self.outline, |el| el.child(div().absolute().inset_0().rounded(size::BUTTON_RADIUS - px(1.)).shadow(keycap)))
            .child(icon)
            .map(|el| {
                if self.held {
                    return el;
                }
                let ring_ink = p.ring;
                el.cursor_pointer()
                    .tab_index(0)
                    .focus_visible(move |s| s.shadow(vec![ring(ring_ink)]))
                    .on_hover(motion::track_hover(hovered))
                    .when_some(self.tooltip, |el, t| el.tooltip(tip(t)))
                    .when_some(self.on_click, |el, act| el.on_click(move |_, window, cx| act(window, cx)))
            })
    }
}

/// The chrome row over a browser tab. The field is the tab's own address bar; `more` the More menu the screen
/// builds. `forwarded` names the address Open on laptop opens while the host forwards the port; `copy` is there while
/// the tab has a real url, true for the 1.5 s after a copy, which the screen times.
#[derive(IntoElement)]
pub struct ChromeRow {
    pub field: Entity<AddressField>,
    pub loading: bool,
    pub can_back: bool,
    pub can_forward: bool,
    pub refresh_held: bool,
    pub has_page: bool,
    pub inline: bool,
    pub forwarded: Option<SharedString>,
    pub copy: Option<bool>,
    pub more: AnyElement,
    pub on_back: Act,
    pub on_forward: Act,
    pub on_refresh: Act,
    pub on_open_outside: Act,
    pub on_open_on_laptop: Act,
    pub on_copy: Act,
}

fn slot(selector: &'static str, el: impl IntoElement) -> impl IntoElement {
    div().debug_selector(move || selector.into()).flex().flex_none().child(el)
}

fn run(act: &Act) -> impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut App) + 'static {
    let act = act.clone();
    move |_, window, cx| act(window, cx)
}

/// Where the loading bar stood when `loading` last changed, and since when.
struct Bar {
    loading: bool,
    since: Option<Instant>,
    from: f32,
}

/// The bar's width as a share of the row and its opacity, `None` once it is gone.
fn bar_at(bar: &Bar, now: Instant) -> Option<(f32, f32)> {
    let since = bar.since?;
    let elapsed = now.saturating_duration_since(since);
    if bar.loading {
        let t = (elapsed.as_secs_f32() / GROW.as_secs_f32()).min(1.);
        let curve = Easing::cubic_bezier(0.1, 0.5, 0.2, 1.).expect("a valid curve");
        return Some((0.04 + (0.9 - 0.04) * curve.sample(t), 1.));
    }
    if elapsed >= FADE_DELAY + FADE {
        return None;
    }
    let grown = Easing::EaseOut.sample((elapsed.as_secs_f32() / FILL.as_secs_f32()).min(1.));
    let faded = Easing::EaseOut.sample((elapsed.saturating_sub(FADE_DELAY).as_secs_f32() / FADE.as_secs_f32()).min(1.));
    Some((bar.from + (1. - bar.from) * grown, 1. - faded))
}

/// Whether the bar still moves: growing while the page loads, filling and fading once it is done, then still.
fn moving(bar: &Bar, now: Instant) -> bool {
    let span = if bar.loading { GROW } else { FADE_DELAY + FADE };
    bar.since.is_some_and(|since| now.saturating_duration_since(since) < span)
}

fn loading_bar(key: ElementId, loading: bool, window: &mut Window, cx: &mut App) -> Option<impl IntoElement> {
    let state = window.use_keyed_state(key, cx, |_, _| Bar { loading: false, since: None, from: 1. });
    let now = Instant::now();
    if state.read(cx).loading != loading {
        let from = bar_at(state.read(cx), now).map_or(1., |(scale, _)| scale);
        state.update(cx, |bar, _| *bar = Bar { loading, since: Some(now), from });
    }
    let at = if still(cx) {
        loading.then_some((HELD_AT, 1.))
    } else {
        let bar = state.read(cx);
        if moving(bar, now) {
            window.request_animation_frame();
        }
        bar_at(bar, now)
    };
    let (scale, opacity) = at?;
    let p = cx.palette();
    Some(
        div()
            .debug_selector(|| "browser-loading-bar".into())
            .absolute()
            .bottom_0()
            .left_0()
            .h(px(2.))
            .w(relative(scale))
            .rounded_tr(px(1.))
            .rounded_br(px(1.))
            .bg(p.primary)
            .opacity(opacity)
            .shadow(vec![BoxShadow {
                color: p.ring,
                offset: point(px(0.), px(0.)),
                blur_radius: px(3.),
                spread_radius: px(1.),
                inset: false,
            }]),
    )
}

impl RenderOnce for ChromeRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let key = self.field.entity_id();
        let id = |name: &str| SharedString::from(format!("browser-{name}-{key}"));
        let outside = self.has_page.then(|| self.on_open_outside.clone());
        self.field.update(cx, |field, _| field.set_outside(outside));

        let back = Button::icon_xs(id("back"), Glyph::ArrowLeft).tooltip("Back").held(!self.can_back).on_click(run(&self.on_back));
        let forward =
            Button::icon_xs(id("forward"), Glyph::ArrowRight).tooltip("Forward").held(!self.can_forward).on_click(run(&self.on_forward));
        let refresh = if self.loading && !self.refresh_held {
            IconKey::new(id("refresh"), "button", Glyph::RotateCw)
                .turning()
                .tooltip("Loading…")
                .on_click(self.on_refresh.clone())
                .into_any_element()
        } else {
            Button::icon_xs(id("refresh"), Glyph::RotateCw)
                .tooltip("Refresh")
                .held(self.refresh_held)
                .on_click(run(&self.on_refresh))
                .into_any_element()
        };
        let laptop = self.forwarded.map(|address| {
            slot(
                "browser-laptop",
                Button::icon_xs(id("laptop"), Glyph::Laptop)
                    .tooltip(format!("Open {address} on this computer"))
                    .on_click(run(&self.on_open_on_laptop)),
            )
        });
        let copy = self.copy.map(|copied| {
            if copied {
                slot(
                    "browser-copied",
                    IconKey::new(id("copy"), "button", Glyph::Check).ink(p.success).tooltip("Copied").on_click(self.on_copy.clone()),
                )
                .into_any_element()
            } else {
                slot("browser-copy", Button::icon_xs(id("copy"), Glyph::Copy).tooltip("Copy URL").on_click(run(&self.on_copy)))
                    .into_any_element()
            }
        });
        let row = h_flex()
            .debug_selector(|| "browser-chrome-row".into())
            .flex_none()
            .w_full()
            .items_center()
            .gap_1()
            .px_2()
            .border_b_1()
            .map(|el| {
                if self.inline {
                    el.h(INLINE_HEIGHT).mb_3().border_color(gpui_kit::transparent_black())
                } else {
                    el.h(ROW_HEIGHT).border_color(p.border.opacity(0.6))
                }
            })
            .child(
                h_flex()
                    .flex_none()
                    .gap(px(2.))
                    .child(slot("browser-back", back))
                    .child(slot("browser-forward", forward))
                    .child(slot("browser-refresh", refresh)),
            )
            .child(self.field.clone())
            .children(laptop)
            .children(copy)
            .child(self.more);
        let bar = loading_bar(ElementId::Name(id("loading-bar")), self.loading, window, cx);
        div().debug_selector(|| "browser-chrome".into()).relative().flex().flex_col().flex_none().w_full().child(row).children(bar)
    }
}

#[cfg(test)]
mod tests {
    use super::{moving, Bar, FADE, FADE_DELAY, GROW};

    #[test]
    fn the_bar_asks_for_frames_only_while_it_moves() {
        let start = std::time::Instant::now();
        let done = Bar { loading: false, since: Some(start), from: 0.5 };
        assert!(moving(&done, start + FADE_DELAY), "it fades after the load");
        assert!(!moving(&done, start + FADE_DELAY + FADE), "and is still once faded");
        let loading = Bar { loading: true, since: Some(start), from: 1. };
        assert!(moving(&loading, start + GROW / 2));
        assert!(!moving(&loading, start + GROW), "held at its last width once grown");
        assert!(!moving(&Bar { loading: false, since: None, from: 1. }, start), "never loaded, never moved");
    }

    use std::cell::Cell;

    use gpui_kit::component::Root;
    use gpui_kit::{px, size, Context, Modifiers, TestAppContext, VisualTestContext};

    use super::*;
    use crate::ui::{self, Side};

    #[derive(Default)]
    struct Counts {
        back: Cell<u32>,
        forward: Cell<u32>,
        refresh: Cell<u32>,
        laptop: Cell<u32>,
        copy: Cell<u32>,
    }

    #[derive(Clone, Copy, Default)]
    struct Shape {
        loading: bool,
        can_back: bool,
        refresh_held: bool,
        inline: bool,
        forwarded: bool,
        copied: Option<bool>,
    }

    struct Host {
        field: Entity<AddressField>,
        shape: Shape,
        counts: Rc<Counts>,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let c = self.counts.clone();
            let act = |f: fn(&Counts) -> &Cell<u32>| -> Act {
                let c = c.clone();
                Rc::new(move |_, _| f(&c).set(f(&c).get() + 1))
            };
            let s = self.shape;
            div().size_full().flex().flex_col().child(ChromeRow {
                field: self.field.clone(),
                loading: s.loading,
                can_back: s.can_back,
                can_forward: false,
                refresh_held: s.refresh_held,
                has_page: true,
                inline: s.inline,
                forwarded: s.forwarded.then(|| "localhost:3000/".into()),
                copy: s.copied,
                more: div().debug_selector(|| "more".into()).size(px(24.)).into_any_element(),
                on_back: act(|c| &c.back),
                on_forward: act(|c| &c.forward),
                on_refresh: act(|c| &c.refresh),
                on_open_outside: act(|c| &c.laptop),
                on_open_on_laptop: act(|c| &c.laptop),
                on_copy: act(|c| &c.copy),
            })
        }
    }

    fn open(cx: &mut TestAppContext, shape: Shape) -> (&mut VisualTestContext, Rc<Counts>) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
            cx.set_global(ui::Still(true));
        });
        let counts = Rc::new(Counts::default());
        let kept = counts.clone();
        let (_, cx) = cx.add_window_view(move |window, cx| {
            let field = cx.new(|cx| AddressField::new(window, cx));
            let host = cx.new(|_| Host { field, shape, counts: kept });
            Root::new(host, window, cx)
        });
        cx.simulate_resize(size(px(540.), px(300.)));
        cx.run_until_parked();
        (cx, counts)
    }

    fn press(cx: &mut VisualTestContext, selector: &'static str) {
        let at = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn"));
        cx.simulate_click(at.center(), Modifiers::none());
        cx.run_until_parked();
    }

    #[gpui_kit::test]
    fn the_row_is_40_px_with_24_px_icon_buttons(cx: &mut TestAppContext) {
        let (cx, _) = open(cx, Shape { forwarded: true, copied: Some(false), ..Shape::default() });
        let row = cx.debug_bounds("browser-chrome-row").unwrap();
        assert_eq!((row.size.width, row.size.height), (px(540.), ROW_HEIGHT));
        assert_eq!(cx.debug_bounds("browser-chrome").unwrap().size.height, ROW_HEIGHT, "nothing under the row but its line");
        for selector in ["browser-back", "browser-forward", "browser-refresh", "browser-laptop", "browser-copy"] {
            let at = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn"));
            assert_eq!((at.size.width, at.size.height), (px(24.), px(24.)), "{selector}");
        }
        let (back, forward) = (cx.debug_bounds("browser-back").unwrap(), cx.debug_bounds("browser-forward").unwrap());
        assert_eq!((back.left() - row.left(), forward.left() - back.right()), (px(8.), px(2.)), "8 px in, the navigation 2 px apart");
        let (field, more) = (cx.debug_bounds("browser-address").unwrap(), cx.debug_bounds("more").unwrap());
        assert_eq!(field.left() - cx.debug_bounds("browser-refresh").unwrap().right(), px(4.));
        assert_eq!(row.right() - more.right(), px(8.), "the More menu ends 8 px in");
        assert_eq!(field.size.height, px(28.));
    }

    #[gpui_kit::test]
    fn inline_the_row_is_28_px_with_12_px_under_it(cx: &mut TestAppContext) {
        let (cx, _) = open(cx, Shape { inline: true, loading: true, ..Shape::default() });
        let (row, whole) = (cx.debug_bounds("browser-chrome-row").unwrap(), cx.debug_bounds("browser-chrome").unwrap());
        assert_eq!((row.size.height, whole.size.height), (INLINE_HEIGHT, INLINE_HEIGHT + px(12.)));
        let bar = cx.debug_bounds("browser-loading-bar").expect("loading, the bar is drawn");
        assert_eq!(bar.bottom(), whole.bottom(), "the bar runs on the page's top edge, under the 12 px gap");
    }

    #[gpui_kit::test]
    fn held_buttons_take_no_press(cx: &mut TestAppContext) {
        let (cx, counts) = open(cx, Shape { refresh_held: true, ..Shape::default() });
        press(cx, "browser-back");
        press(cx, "browser-forward");
        press(cx, "browser-refresh");
        assert_eq!(
            (counts.back.get(), counts.forward.get(), counts.refresh.get()),
            (0, 0, 0),
            "with no history and no place all three are held"
        );
    }

    #[gpui_kit::test]
    fn live_buttons_take_their_press(cx: &mut TestAppContext) {
        let (cx, counts) = open(cx, Shape { can_back: true, forwarded: true, copied: Some(false), ..Shape::default() });
        press(cx, "browser-back");
        press(cx, "browser-refresh");
        press(cx, "browser-laptop");
        press(cx, "browser-copy");
        assert_eq!((counts.back.get(), counts.refresh.get(), counts.laptop.get(), counts.copy.get()), (1, 1, 1, 1));
    }

    #[gpui_kit::test]
    fn while_loading_refresh_turns_and_still_takes_a_press(cx: &mut TestAppContext) {
        let (cx, counts) = open(cx, Shape { loading: true, ..Shape::default() });
        press(cx, "browser-refresh");
        assert_eq!(counts.refresh.get(), 1, "Stop, while the page loads, goes to the screen, which stops it");
        let (bar, row) = (cx.debug_bounds("browser-loading-bar").unwrap(), cx.debug_bounds("browser-chrome").unwrap());
        assert_eq!((bar.size.width, bar.size.height), (row.size.width * HELD_AT, px(2.)), "under reduced motion the bar stands at 0.9");
        assert_eq!(bar.bottom(), row.bottom(), "on the row's bottom edge");
    }

    #[gpui_kit::test]
    fn no_bar_while_nothing_loads(cx: &mut TestAppContext) {
        let (cx, _) = open(cx, Shape::default());
        assert!(cx.debug_bounds("browser-loading-bar").is_none());
        assert!(cx.debug_bounds("browser-laptop").is_none(), "no forwarded port, no Open on laptop");
        assert!(cx.debug_bounds("browser-copy").is_none() && cx.debug_bounds("browser-copied").is_none(), "no url, no Copy URL");
    }

    #[gpui_kit::test]
    fn a_copy_reads_as_the_check(cx: &mut TestAppContext) {
        let (cx, counts) = open(cx, Shape { copied: Some(true), ..Shape::default() });
        assert!(cx.debug_bounds("browser-copy").is_none(), "the copy glyph gives way");
        let check = cx.debug_bounds("browser-copied").expect("the check stands in its slot");
        assert_eq!((check.size.width, check.size.height), (px(24.), px(24.)));
        press(cx, "browser-copied");
        assert_eq!(counts.copy.get(), 1, "a press while it reads Copied copies again");
    }

    #[test]
    fn the_bar_grows_on_its_curve_then_fills_and_fades() {
        let start = Instant::now();
        let loading = Bar { loading: true, since: Some(start), from: 1. };
        assert_eq!(bar_at(&loading, start), Some((0.04, 1.)));
        let (late, _) = bar_at(&loading, start + GROW).unwrap();
        assert!((late - 0.9).abs() < 1e-4, "5.3 s in it stands at 0.9: {late}");
        let (half, _) = bar_at(&loading, start + GROW / 2).unwrap();
        assert!(half > 0.8, "the curve front-loads the growth: {half}");
        let done = Bar { loading: false, since: Some(start), from: 0.5 };
        let (full, still_shown) = bar_at(&done, start + FILL).unwrap();
        assert_eq!((full, still_shown), (1., 1.), "full width at 150 ms, not yet fading");
        let (_, fading) = bar_at(&done, start + FADE_DELAY + FADE / 2).unwrap();
        assert!(fading > 0. && fading < 1.);
        assert_eq!(bar_at(&done, start + FADE_DELAY + FADE), None);
    }
}
