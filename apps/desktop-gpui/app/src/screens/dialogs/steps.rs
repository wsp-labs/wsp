// SPDX-License-Identifier: AGPL-3.0-only
//! What the add flows draw inside the step frame (`ui::Dialog::frame`) and its rows (`ui::StepRow`), from
//! `settings/add/StepRow.tsx` and `AddComputerDialog.tsx`: the foot's saved line, a step's log, the acts on a row that
//! did not land, the sign-in that waits on the person, and the ready page.

use std::rc::Rc;
use std::time::{Duration, SystemTime};

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, relative, Animation, AnimationExt as _, AnyElement, App, ClickEvent, ClipboardItem, FontWeight, Hsla,
    ScrollHandle, SharedString, Window,
};

pub use crate::ui::{fmt_duration, fmt_elapsed, narrow, step_refusal};
use crate::ui::{size, text, ActivePalette, Button, Glyph, Still};

type Click = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// `max-h-48` on a step's log.
const LOG_HEIGHT: f32 = 192.;
/// `SAVED_EVERY_MS`: the saved line's check draws in again at most this often.
const SAVED_EVERY: Duration = Duration::from_secs(4);

/// The foot's saved line: the check and "Saved, you can finish later". `saves` counts the saves landed; the check
/// fades in again on a new one, at most every 4 s, and nothing moves at rest.
pub fn saved_line(saves: usize, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let p = cx.palette().clone();
    let drawn = window.use_keyed_state("saved-line", cx, |_, _| (saves, SystemTime::now()));
    let (n, at) = *drawn.read(cx);
    if n != saves && at.elapsed().unwrap_or_default() >= SAVED_EVERY {
        drawn.update(cx, |d, _| *d = (saves, SystemTime::now()));
    }
    let key = drawn.read(cx).0;
    let check = Icon::new(Glyph::Check).with_size(px(14.)).text_color(p.muted_foreground);
    let still = cx.try_global::<Still>().is_some_and(|s| s.0) || cx.reduce_motion();
    h_flex()
        .debug_selector(|| "saved-line".into())
        .min_w_0()
        .gap(px(6.))
        .text_size(text::XS.0)
        .line_height(text::XS.1)
        .text_color(p.muted_foreground)
        .child(if still {
            div().flex_none().child(check).into_any_element()
        } else {
            div()
                .flex_none()
                .child(check)
                .with_animation(
                    gpui_kit::ElementId::NamedInteger("saved-check".into(), key as u64),
                    Animation::new(Duration::from_millis(600)),
                    |el, t| el.opacity(t),
                )
                .into_any_element()
        })
        .child("Saved, you can finish later")
}

/// A step's last lines of output under its open row (`StepLog`): the mono, wrapping, 192 px at most and kept at its
/// end; "No output yet." while it has none.
#[derive(IntoElement)]
pub struct StepLog {
    id: SharedString,
    lines: Vec<SharedString>,
}

impl StepLog {
    pub fn new(id: impl Into<SharedString>, lines: Vec<SharedString>) -> Self {
        StepLog { id: id.into(), lines }
    }
}

impl RenderOnce for StepLog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        if self.lines.is_empty() {
            return div()
                .debug_selector(|| "step-log-empty".into())
                .text_size(text::NOTE.0)
                .line_height(text::NOTE.1)
                .text_color(p.muted_foreground)
                .child("No output yet.")
                .into_any_element();
        }
        let text: SharedString = self.lines.join("\n").into();
        let key = SharedString::from(format!("{}-log", self.id));
        let held = window.use_keyed_state(key, cx, |_, _| (ScrollHandle::new(), SharedString::default()));
        let (scroll, was) = held.read(cx).clone();
        if was != text {
            scroll.scroll_to_bottom();
            held.update(cx, |h, _| h.1 = text.clone());
        }
        div()
            .id(SharedString::from(format!("{}-log", self.id)))
            .debug_selector(|| "step-log".into())
            .max_h(px(LOG_HEIGHT))
            .overflow_y_scroll()
            .track_scroll(&scroll)
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(px(11.))
            .line_height(px(16.))
            .text_color(p.muted_foreground)
            .child(text)
            .into_any_element()
    }
}

/// The act on a step that did not land: Retry, held while it asks.
pub fn retry_act(id: impl Into<SharedString>, busy: bool) -> Button {
    Button::outline(id, "Retry").xs().held(busy)
}

/// Skip on a row that waits on the person or did not land, held while it asks (`SkipAct`): "Skip" or "Skip for now".
pub fn skip_act(id: impl Into<SharedString>, word: impl Into<SharedString>, busy: bool) -> Button {
    Button::ghost(id, word).xs().held(busy)
}

/// A sign-in that waits on the person (`WaitBlock`): the device code with Copy, Open the tab again, Skip for now, and
/// where it can be finished later; once its page ran out, that it did, Retry and Skip for now. Put it under its
/// [`StepRow`].
#[derive(IntoElement)]
pub struct SignInWait {
    id: SharedString,
    code: Option<SharedString>,
    url: Option<SharedString>,
    expired: bool,
    busy: bool,
    on_retry: Option<Click>,
    on_skip: Option<Click>,
}

impl SignInWait {
    pub fn new(id: impl Into<SharedString>) -> Self {
        SignInWait { id: id.into(), code: None, url: None, expired: false, busy: false, on_retry: None, on_skip: None }
    }

    /// The code the person types on the sign-in page.
    pub fn code(mut self, code: Option<impl Into<SharedString>>) -> Self {
        self.code = code.map(Into::into);
        self
    }

    /// The sign-in page, which Open the tab again opens in the browser.
    pub fn url(mut self, url: Option<impl Into<SharedString>>) -> Self {
        self.url = url.map(Into::into);
        self
    }

    /// The page ran out: Retry opens a fresh one.
    pub fn expired(mut self, expired: bool) -> Self {
        self.expired = expired;
        self
    }

    /// An act is on its way: Retry and Skip are held.
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    pub fn on_retry(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_retry = Some(Rc::new(f));
        self
    }

    pub fn on_skip(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_skip = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for SignInWait {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let id = self.id.clone();
        let skip = |bare: bool| {
            let button = if bare {
                Button::text(format!("{id}-skip"), "Skip for now")
            } else {
                skip_act(format!("{id}-skip"), "Skip for now", self.busy)
            };
            let on = self.on_skip.clone();
            button.held(self.busy).when_some(on, |b, f| b.on_click(move |e, w, cx| f(e, w, cx)))
        };
        if self.expired {
            let on = self.on_retry.clone();
            return v_flex()
                .debug_selector(|| "sign-in-wait".into())
                .gap_3()
                .child(step_refusal("The sign-in page ran out.", Some("Retry opens a fresh one.".into()), cx))
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_2()
                        .child(retry_act(format!("{id}-retry"), self.busy).when_some(on, |b, f| b.on_click(move |e, w, cx| f(e, w, cx))))
                        .child(skip(false)),
                )
                .into_any_element();
        }
        let copied = window.use_keyed_state(SharedString::from(format!("{id}-copied")), cx, |_, _| false);
        let code = self.code.map(|code| {
            let value = code.clone();
            let is_copied = *copied.read(cx);
            h_flex()
                .gap_x_3()
                .child(
                    div()
                        .debug_selector(|| "sign-in-code".into())
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(px(20.))
                        .line_height(px(28.))
                        .text_color(p.foreground)
                        .child(code),
                )
                .child(
                    Button::outline(format!("{id}-copy-code"), "Copy")
                        .xs()
                        .glyph(if is_copied { Glyph::Check } else { Glyph::Copy })
                        .on_click(move |_, window, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(value.to_string()));
                            copied.update(cx, |c, cx| {
                                *c = true;
                                cx.notify();
                            });
                            let copied = copied.clone();
                            window
                                .spawn(cx, async move |cx| {
                                    cx.background_executor().timer(Duration::from_millis(1_400)).await;
                                    copied.update(cx, |c, cx| {
                                        *c = false;
                                        cx.notify();
                                    });
                                })
                                .detach();
                        }),
                )
        });
        let url = self.url.map(|url| {
            Button::outline(format!("{id}-open-tab"), "Open the tab again")
                .xs()
                .glyph(Glyph::ExternalLink)
                .on_click(move |_, _, cx| cx.open_url(&url))
        });
        v_flex()
            .debug_selector(|| "sign-in-wait".into())
            .gap_3()
            .child(h_flex().min_h(px(40.)).flex_wrap().gap_x_3().gap_y_2().children(code).children(url).child(skip(true)))
            .child(
                div()
                    .text_size(text::NOTE.0)
                    .line_height(text::NOTE.1)
                    .text_color(p.muted_foreground)
                    .child("You can sign in later in Settings."),
            )
            .into_any_element()
    }
}

/// `WASH_LAYERS` stadiums stand in for the web's `radial-gradient(70% 100% at 50% 0%, hero 32%, transparent)`, which
/// GPUI cannot draw: each a step smaller, their alphas stacking to 32% at the top edge's middle and none at the rim.
const WASH_LAYERS: usize = 48;

/// One static wash of the theme's hero ink falling from the page's top edge over its top half, behind its words.
fn wash(hero: Hsla) -> impl IntoElement {
    let alpha = 1. - (1f32 - 0.32).powf(1. / WASH_LAYERS as f32);
    div()
        .debug_selector(|| "ready-wash".into())
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(relative(0.5))
        .rounded_t(size::DIALOG_RADIUS - px(1.))
        .overflow_hidden()
        .children((1..=WASH_LAYERS).map(|k| {
            let f = k as f32 / WASH_LAYERS as f32;
            div()
                .absolute()
                .left(relative(0.5 - 0.7 * f))
                .top(relative(-f))
                .w(relative(1.4 * f))
                .h(relative(2. * f))
                .rounded(px(10_000.))
                .bg(hero.opacity(alpha))
        }))
}

/// The page after every row is done (`ReadyPage`): the place's glyph in its tile, "X is ready", what landed, the one
/// act and Close. Hand it to [`StepDialog::page`].
#[derive(IntoElement)]
pub struct ReadyPage {
    glyph: AnyElement,
    name: SharedString,
    line: Option<SharedString>,
    act: Button,
    on_close: Option<Click>,
}

impl ReadyPage {
    /// `glyph` is the place's own mark at 24 px in the foreground at 80% (`ui::computer_glyph`); `act` the primary,
    /// as `Button::primary("start-task", "Start a task here")`.
    pub fn new(glyph: impl IntoElement, name: impl Into<SharedString>, act: Button) -> Self {
        ReadyPage { glyph: glyph.into_any_element(), name: name.into(), line: None, act, on_close: None }
    }

    /// What landed: "2 agents, 3 CLIs.".
    pub fn line(mut self, line: Option<impl Into<SharedString>>) -> Self {
        self.line = line.map(Into::into);
        self
    }

    pub fn on_close(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ReadyPage {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let close = self.on_close;
        v_flex()
            .debug_selector(|| "ready-page".into())
            .relative()
            .flex_1()
            .min_h_0()
            .items_center()
            .justify_center()
            .px_6()
            .py_12()
            .child(wash(p.hero_field))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(48.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(p.border)
                    .bg(p.foreground.opacity(0.04))
                    .child(self.glyph),
            )
            .child(
                div()
                    .debug_selector(|| "ready-title".into())
                    .mt_5()
                    .text_size(px(24.))
                    .line_height(px(32.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.foreground)
                    .child(format!("{} is ready", self.name)),
            )
            .children(self.line.map(|line| {
                div().mt(px(6.)).text_size(text::NOTE.0).line_height(text::NOTE.1).text_color(p.muted_foreground).text_center().child(line)
            }))
            .child(
                h_flex()
                    .mt(px(28.))
                    .gap_2()
                    .child(self.act)
                    .child(Button::ghost("ready-close", "Close").when_some(close, |b, f| b.on_click(move |e, w, cx| f(e, w, cx)))),
            )
    }
}
