// SPDX-License-Identifier: AGPL-3.0-only
//! The dialog shell as Add a computer draws its steps (`StepDialog.tsx`): the title with "n of N" at its right and
//! one line under it, cards, and the foot. The copyable command row (`CopyRow`), and the tooltip a shortcut lives in.

use std::time::Duration;

use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{
    div, linear_color_stop, linear_gradient, prelude::*, px, AnyElement, AnyView, App, ClipboardItem, FocusHandle, FontWeight,
    HighlightStyle, Hsla, MouseButton, Pixels, ScrollHandle, SharedString, StyledText, Window,
};

use super::card::{keycap, word, CardItem, WordClass};
use super::controls::Button;
use super::icons::Glyph;
use super::motion::open_fade;
use super::theme::ActivePalette;
use super::{size, text};

/// How long the copy glyph stands as a check before it is a copy glyph again.
const COPIED: Duration = Duration::from_millis(1_400);
/// `w-14`: the copy row's label column, so the values under each other line up.
const LABEL_WIDTH: gpui_kit::Pixels = px(56.);
/// `max-w-110`: an alert dialog's and a sheet's popup.
const ALERT_WIDTH: gpui_kit::Pixels = px(440.);
/// The fade at the right of a copy row's value while more of it waits past the box.
const FADE: gpui_kit::Pixels = px(24.);

/// `STEP_FRAME`'s `sm:h-[640px]`: one height across every step.
const FRAME_HEIGHT: Pixels = px(640.);
/// The sheet's `pt-12`: the room above it that still shows the page.
pub const SHEET_TOP: Pixels = px(48.);

type Outside = std::rc::Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// Whether the window is narrow enough that a step dialog is a sheet along its foot.
pub fn narrow(window: &Window) -> bool {
    super::narrow::width(window).sm
}

/// A dialog of steps at `STEP_WIDTH`: its surface is drawn where it is put, so a caller lays it over a scrim (gpui-kit's
/// dialog layer) or shows it in place. With [`Dialog::frame`] it is the step frame Add a computer walks: 640 px high
/// from 640 px up, and under that a sheet along the window's foot with its acts stacked, the primary on top.
#[derive(IntoElement)]
pub struct Dialog {
    title: AnyElement,
    count: Option<(SharedString, SharedString)>,
    line: Option<SharedString>,
    body: Vec<AnyElement>,
    state: Option<AnyElement>,
    acts: Vec<AnyElement>,
    sheet: bool,
    frame: bool,
    page: Option<AnyElement>,
    focus: Option<FocusHandle>,
    scroll: Option<ScrollHandle>,
    outside: Option<Outside>,
    layered: bool,
}

impl Dialog {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Dialog::titled(title.into())
    }

    /// A title drawn as `title` is, for one with a mono run in it.
    pub fn titled(title: impl IntoElement) -> Self {
        Dialog {
            title: title.into_any_element(),
            count: None,
            line: None,
            body: Vec::new(),
            state: None,
            acts: Vec::new(),
            sheet: false,
            frame: false,
            page: None,
            focus: None,
            scroll: None,
            outside: None,
            layered: false,
        }
    }

    /// The web's plain `DialogPopup` at `max-w-110`: the header, one `DialogPanel` at `pt-1` (`pb-0` over a foot,
    /// `pb-3` where there is none) and the foot only where it has an act.
    pub fn sheet(mut self) -> Self {
        self.sheet = true;
        self
    }

    /// Drawn on the dialogs' layer, which times its own fades and moves, so the popup does not fade in a second time.
    pub fn layered(mut self) -> Self {
        self.layered = true;
        self
    }

    /// The step's place as a quiet fact at the head's right: "2 of 5", "Step 2 of 5" on its hover.
    pub fn count(mut self, step: usize, of: usize) -> Self {
        self.count = Some((format!("{step} of {of}").into(), format!("Step {step} of {of}").into()));
        self
    }

    /// The head's fact in words of the caller's own and its hover: "3 of 9 done", "3 of 9 steps done".
    pub fn count_words(mut self, words: impl Into<SharedString>, why: impl Into<SharedString>) -> Self {
        self.count = Some((words.into(), why.into()));
        self
    }

    /// The one line under the title saying what this step is for.
    pub fn line(mut self, line: impl Into<SharedString>) -> Self {
        self.line = Some(line.into());
        self
    }

    /// One card of the body; its rows keep the dialog's 16 px inset.
    pub fn card(mut self, card: super::Card) -> Self {
        self.body.push(card.inset(size::DIALOG_INSET).into_any_element());
        self
    }

    pub fn child(mut self, el: impl IntoElement) -> Self {
        self.body.push(el.into_any_element());
        self
    }

    pub fn children(mut self, els: impl IntoIterator<Item = impl IntoElement>) -> Self {
        self.body.extend(els.into_iter().map(IntoElement::into_any_element));
        self
    }

    /// The quiet state at the foot's left: "Saved, you can finish later".
    pub fn state(mut self, state: impl IntoElement) -> Self {
        self.state = Some(state.into_any_element());
        self
    }

    /// An act at the foot's right, in order: the outline secondary, then the one primary.
    pub fn act(mut self, button: impl IntoElement) -> Self {
        self.acts.push(button.into_any_element());
        self
    }

    /// The step frame (`STEP_FRAME`): one height of 640 px across the steps, and under 640 px a sheet along the foot.
    pub fn frame(mut self) -> Self {
        self.frame = true;
        self
    }

    /// A whole page in place of the head, body and foot: a ready page.
    pub fn page(mut self, page: impl IntoElement) -> Self {
        self.page = Some(page.into_any_element());
        self
    }

    /// The handle the dialog holds focus with while no field of it does, so Escape still reaches the layer once the
    /// field that had focus is gone.
    pub fn focus(mut self, handle: &FocusHandle) -> Self {
        self.focus = Some(handle.clone());
        self
    }

    /// The body's scroll, for a caller that brings a row into view.
    pub fn scroll(mut self, handle: &ScrollHandle) -> Self {
        self.scroll = Some(handle.clone());
        self
    }

    /// What a press on the page above a sheet does, as a press on the scrim does: the layer's dismiss.
    pub fn on_outside(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.outside = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for Dialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let plain = self.sheet;
        let along_foot = self.frame && narrow(window);
        let footless = plain && self.acts.is_empty() && self.state.is_none();
        let viewport = window.viewport_size();
        let surface = v_flex()
            .debug_selector(|| "dialog".into())
            .when_some(self.focus.as_ref(), |el, f| el.track_focus(f))
            .relative()
            .min_h_0()
            .bg(p.popover)
            .shadow(p.dialog_shadow.clone())
            .text_color(p.foreground)
            .border_color(p.popover_edge)
            .map(|el| {
                if along_foot {
                    el.w(viewport.width).max_h(viewport.height - SHEET_TOP).border_t_1()
                } else {
                    el.w(if plain { ALERT_WIDTH } else { size::DIALOG_WIDTH })
                        .when(self.frame, |el| el.h(FRAME_HEIGHT))
                        .max_h(popup_max(window))
                        .rounded(size::DIALOG_RADIUS)
                        .border_1()
                }
            });
        let surface = match self.page {
            Some(page) => surface.child(page),
            None => {
                let head = h_flex()
                    .items_start()
                    .justify_between()
                    .gap_6()
                    .px_5()
                    .pt_4()
                    .pb(if plain { px(4.) } else { px(12.) })
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap_1()
                            .child(
                                div()
                                    .debug_selector(|| "dialog-title".into())
                                    .text_size(text::DIALOG_TITLE.0)
                                    .line_height(text::DIALOG_TITLE.1)
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(p.foreground)
                                    .child(self.title),
                            )
                            .children(self.line.map(|line| {
                                div()
                                    .debug_selector(|| "dialog-line".into())
                                    .text_size(text::NOTE.0)
                                    .line_height(text::NOTE.1)
                                    .text_color(p.muted_foreground)
                                    .child(line)
                            })),
                    )
                    .children(self.count.map(|(words, why)| {
                        div()
                            .id("dialog-count")
                            .debug_selector(|| "dialog-count".into())
                            .flex_none()
                            .pt(px(2.))
                            .tooltip(tip(why))
                            .child(word(words, WordClass::Fact, cx))
                    }));
                let (top, bottom) = match (plain, footless) {
                    (false, _) => (px(8.), px(20.)),
                    (true, false) => (px(4.), px(0.)),
                    (true, true) => (px(4.), px(12.)),
                };
                let scroll = self.scroll.unwrap_or_default();
                let body = v_flex()
                    .id("dialog-body")
                    .debug_selector(|| "dialog-body".into())
                    .when(self.frame, |el| el.flex_1())
                    .flex_shrink(1.)
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&scroll)
                    .child(v_flex().gap_5().px_5().pt(top).pb(bottom).children(self.body));
                let left = h_flex().min_w_0().min_h(px(20.)).children(self.state);
                let foot = (!footless).then(|| {
                    let foot = if along_foot {
                        let stacked: Vec<AnyElement> = acts(self.acts).collect();
                        v_flex().gap_2().px_5().pt_3().pb_4().children(stacked.into_iter().rev()).child(left.justify_center())
                    } else {
                        h_flex().justify_end().gap_2().px_5().pt_3().pb_4().child(left.flex_1()).children(acts(self.acts))
                    };
                    foot.debug_selector(|| "dialog-foot".into())
                });
                surface.child(head).child(body).children(foot)
            }
        };
        let surface = if self.layered { surface.into_any_element() } else { open_fade("dialog-open", surface, window, cx) };
        if !along_foot {
            return surface;
        }
        as_sheet(surface, self.outside, window)
    }
}

/// The foot's acts, each findable in a test as `dialog-act-<n>` in order from the left.
fn acts(acts: Vec<AnyElement>) -> impl Iterator<Item = AnyElement> {
    acts.into_iter().enumerate().map(|(n, act)| div().debug_selector(move || format!("dialog-act-{n}")).child(act).into_any_element())
}

/// A popup stood as a sheet along the window's foot (`max-sm`). The layer centres what it holds inside the viewport's
/// 16 px; a sheet takes the whole window, the room over it showing the page and, where `outside` says, closing the
/// dialog as the scrim does.
fn as_sheet(surface: AnyElement, outside: Option<Outside>, window: &Window) -> AnyElement {
    let viewport = window.viewport_size();
    // The sheet runs into the layer's 16 px, past the surface that keeps presses off the scrim, so it keeps them itself.
    v_flex()
        .w(viewport.width)
        .h(viewport.height)
        .m(px(-16.))
        .occlude()
        .child(
            div()
                .id("sheet-outside")
                .flex_1()
                .min_h(SHEET_TOP)
                .when_some(outside, |el, f| el.on_mouse_down(MouseButton::Left, move |_, window, cx| f(window, cx))),
        )
        .child(surface)
        .into_any_element()
}

/// A popup's `max-h-full` inside the viewport's `p-4`: past it the body scrolls and the head and foot stay.
fn popup_max(window: &Window) -> Pixels {
    window.viewport_size().height - px(32.)
}

/// `AlertDialog`: the one confirmation before something that does not come back, and the sheet that edits one value.
/// The title and the sentence computed from what goes, a body (a field, the refusal), and the foot: Cancel with the
/// neutral ring, then the destructive confirm or Save. A surface, laid over a scrim by the caller as `Dialog` is; under
/// 640 px a sheet along the window's foot, its acts stacked with the last on top (`bottomStickOnMobile`).
#[derive(IntoElement)]
pub struct AlertDialog {
    title: AnyElement,
    sentence: Option<AnyElement>,
    lead: Option<AnyElement>,
    body: Vec<AnyElement>,
    acts: Vec<AnyElement>,
    outside: Option<Outside>,
    layered: bool,
}

impl AlertDialog {
    pub fn new(title: impl Into<SharedString>) -> Self {
        AlertDialog::titled(title.into())
    }

    /// A title drawn as `title` is, for one with a mono run in it.
    pub fn titled(title: impl IntoElement) -> Self {
        AlertDialog {
            title: title.into_any_element(),
            sentence: None,
            lead: None,
            body: Vec::new(),
            acts: Vec::new(),
            outside: None,
            layered: false,
        }
    }

    /// Drawn on the dialogs' layer, which times its own fades and moves, so the popup does not fade in a second time.
    pub fn layered(mut self) -> Self {
        self.layered = true;
        self
    }

    /// The description under the title: what leaves, read off what the thing holds now.
    pub fn sentence(mut self, sentence: impl IntoElement) -> Self {
        self.sentence = Some(sentence.into_any_element());
        self
    }

    /// The block straight under the head at the body's sides alone, as a radio group sits under the title
    /// (`RewindDialog.tsx`'s `RadioGroup className="px-5"`); the body's `pt-2` then stands between it and the body.
    pub fn lead(mut self, lead: impl IntoElement) -> Self {
        self.lead = Some(lead.into_any_element());
        self
    }

    /// One block of the body, `px-5 pt-2`: a copy row, a field, the `Refusal`.
    pub fn child(mut self, el: impl IntoElement) -> Self {
        self.body.push(el.into_any_element());
        self
    }

    /// What a press on the page above the sheet does, for a dialog the scrim closes: the layer's dismiss.
    pub fn on_outside(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.outside = Some(std::rc::Rc::new(f));
        self
    }

    /// An act at the foot's right, in order: `Button::outline(..).neutral_ring()` for Cancel, then
    /// `Button::destructive(..)`.
    pub fn act(mut self, button: Button) -> Self {
        self.acts.push(button.into_any_element());
        self
    }
}

impl RenderOnce for AlertDialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let sheet = narrow(window);
        let head = v_flex()
            .gap_1()
            .px_5()
            .pt_4()
            .pb_1()
            .child(
                div()
                    .debug_selector(|| "dialog-title".into())
                    .text_size(text::DIALOG_TITLE.0)
                    .line_height(text::DIALOG_TITLE.1)
                    .font_weight(FontWeight::MEDIUM)
                    .child(self.title),
            )
            .children(self.sentence.map(|sentence| {
                div()
                    .debug_selector(|| "dialog-sentence".into())
                    .text_size(text::NOTE.0)
                    .line_height(text::NOTE.1)
                    .text_color(p.muted_foreground)
                    .child(sentence)
            }));
        let body = (!self.body.is_empty()).then(|| {
            v_flex()
                .id("alert-body")
                .flex_shrink(1.)
                .min_h_0()
                .overflow_y_scroll()
                .child(v_flex().gap_2().px_5().pt_2().children(self.body))
        });
        let foot = if sheet {
            let stacked: Vec<AnyElement> = acts(self.acts).collect();
            v_flex().gap_2().px_5().pt_3().pb_4().children(stacked.into_iter().rev())
        } else {
            h_flex().justify_end().gap_2().px_5().pt_3().pb_4().children(acts(self.acts))
        };
        let surface = v_flex()
            .debug_selector(|| "alert-dialog".into())
            .border_color(p.popover_edge)
            .bg(p.popover)
            .shadow(p.dialog_shadow.clone())
            .text_color(p.foreground)
            .map(|el| {
                if sheet {
                    el.w(window.viewport_size().width).max_h(window.viewport_size().height - SHEET_TOP).border_t_1()
                } else {
                    el.w(ALERT_WIDTH).max_h(popup_max(window)).rounded(size::DIALOG_RADIUS).border_1()
                }
            })
            .child(head)
            .children(self.lead.map(|lead| div().px_5().child(lead)))
            .children(body)
            .child(foot);
        let surface = if self.layered { surface.into_any_element() } else { open_fade("alert-dialog-open", surface, window, cx) };
        if sheet {
            as_sheet(surface, self.outside, window)
        } else {
            surface
        }
    }
}

/// `dialog-backdrop`: the page at 60%, 64% in a dark theme, blurred 4 px, the size of the window, fading in with the
/// popup it holds.
pub fn scrim(id: impl Into<gpui_kit::ElementId>, window: &mut Window, cx: &mut App) -> AnyElement {
    let size = window.viewport_size();
    open_fade(id, backdrop(cx).w(size.width).h(size.height), window, cx)
}

/// `dialog-backdrop`'s look alone, for a layer that sizes and fades it itself.
pub fn backdrop(cx: &App) -> gpui_kit::Div {
    let dark = cx.theme().is_dark();
    div().bg(cx.palette().background.opacity(if dark { 0.64 } else { 0.6 })).backdrop_blur(px(4.))
}

/// `RefusalSlot`: the host's refusal under what it refused, in the destructive ink, its fix in the foreground; or what
/// the dialog waits on, quiet. It keeps two lines of room so nothing moves when one lands.
#[derive(IntoElement)]
pub struct Refusal {
    said: Option<SharedString>,
    fix: Option<SharedString>,
    quiet: Option<SharedString>,
    mono: bool,
}

impl Refusal {
    pub fn new() -> Self {
        Refusal { said: None, fix: None, quiet: None, mono: false }
    }

    /// In the mono face at 12 px, as onboarding's refusal slot draws it.
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    pub fn said(mut self, said: impl Into<SharedString>) -> Self {
        self.said = Some(said.into());
        self
    }

    pub fn fix(mut self, fix: impl Into<SharedString>) -> Self {
        self.fix = Some(fix.into());
        self
    }

    /// What the dialog waits on, or a note, while nothing is refused.
    pub fn quiet(mut self, quiet: impl Into<SharedString>) -> Self {
        self.quiet = Some(quiet.into());
        self
    }
}

impl Default for Refusal {
    fn default() -> Self {
        Refusal::new()
    }
}

impl Refusal {
    /// The slot's text in its runs, each with its ink: the sentence in `refused`, its fix after it in `ink`, or the quiet
    /// line in `quiet` while nothing is refused.
    pub fn runs(&self, refused: Hsla, ink: Hsla, quiet: Hsla) -> Vec<(SharedString, Hsla)> {
        let still = if self.said.is_none() { self.quiet.clone() } else { None };
        [self.said.clone().map(|s| (s, refused)), self.fix.as_ref().map(|f| (format!(" {f}").into(), ink)), still.map(|q| (q, quiet))]
            .into_iter()
            .flatten()
            .collect()
    }
}

impl RenderOnce for Refusal {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let runs = self.runs(p.error_foreground, p.foreground, p.muted_foreground);
        let text: SharedString = runs.iter().map(|(t, _)| t.as_ref()).collect::<String>().into();
        let mut at = 0;
        let highlights = runs
            .iter()
            .map(|(t, color)| {
                let range = at..at + t.len();
                at += t.len();
                (range, HighlightStyle { color: Some(*color), ..Default::default() })
            })
            .collect::<Vec<_>>();
        div()
            .debug_selector(|| "refusal".into())
            .min_h(px(36.))
            .text_size(if self.mono { px(12.) } else { text::FACT.0 })
            .when(self.mono, |el| el.font_family(cx.theme().mono_font_family.clone()))
            .line_height(px(18.))
            .child(StyledText::new(text).with_highlights(highlights))
    }
}

/// The quiet state a dialog's foot says at its left.
pub fn quiet(text: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div().text_size(text::NOTE.0).line_height(text::NOTE.1).text_color(cx.palette().muted_foreground).child(text.into())
}

/// A fact somebody types on another computer: its label in a fixed column, the value in mono on one line, and the
/// button that copies it. A value longer than the box fades out at its right edge.
#[derive(IntoElement)]
pub struct CopyRow {
    id: SharedString,
    label: Option<SharedString>,
    value: SharedString,
    ground: Option<Hsla>,
}

impl CopyRow {
    pub fn new(id: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        CopyRow { id: id.into(), label: None, value: value.into(), ground: None }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// What the row stands on, which its fade ends in: a dialog's popover unless said otherwise.
    pub fn on(mut self, ground: Hsla) -> Self {
        self.ground = Some(ground);
        self
    }
}

impl From<CopyRow> for CardItem {
    fn from(row: CopyRow) -> Self {
        CardItem::any(row)
    }
}

impl RenderOnce for CopyRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let copied = window.use_keyed_state(SharedString::from(format!("{}-copied", self.id)), cx, |_, _| false);
        let is_copied = *copied.read(cx);
        // No mask on this base, so the fade is a gradient into the row's own fill, laid over the value's end.
        let ground = self.ground.unwrap_or(p.popover).blend(p.input_fill);
        let fade = linear_gradient(90., linear_color_stop(ground.opacity(0.), 0.), linear_color_stop(ground, 1.));
        let (value, whole) = (self.value.clone(), self.value.clone());
        h_flex()
            .id(self.id.clone())
            .debug_selector(|| "copy-row".into())
            .w_full()
            .h(px(40.))
            .gap_3()
            .px_3()
            .rounded(size::BUTTON_RADIUS)
            .border_1()
            .border_color(p.border)
            .bg(p.input_fill)
            .children(self.label.map(|label| {
                div().flex_none().w(LABEL_WIDTH).text_size(text::XS.0).line_height(text::XS.1).text_color(p.muted_foreground).child(label)
            }))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .id(SharedString::from(format!("{}-value", self.id)))
                            .overflow_x_scroll()
                            .whitespace_nowrap()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_size(text::XS.0)
                            .line_height(text::XS.1)
                            .text_color(p.foreground)
                            .tooltip(move |window, cx| Tooltip::new(whole.clone()).build(window, cx))
                            .child(self.value),
                    )
                    .child(div().absolute().top_0().bottom_0().right_0().w(FADE).bg(fade)),
            )
            .child(
                Button::icon_xs(SharedString::from(format!("{}-copy", self.id)), if is_copied { Glyph::Check } else { Glyph::Copy })
                    .on_click(move |_, window, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(value.to_string()));
                        copied.update(cx, |c, cx| {
                            *c = true;
                            cx.notify();
                        });
                        let copied = copied.clone();
                        window
                            .spawn(cx, async move |cx| {
                                cx.background_executor().timer(COPIED).await;
                                copied.update(cx, |c, cx| {
                                    *c = false;
                                    cx.notify();
                                });
                            })
                            .detach();
                    }),
            )
    }
}

/// A hover word or sentence in wsp's tooltip: `.tooltip(tip("Back to the default"))` on any element with an id.
pub fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text: SharedString = text.into();
    move |window, cx| shortcut_tooltip_view(text.clone(), Vec::new(), window, cx)
}

/// The tooltip a shortcut lives in: the word, then the chord's keycaps. Never a hint strip.
pub fn shortcut_tooltip(label: impl Into<SharedString>, keys: &[&str]) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let label: SharedString = label.into();
    let keys: Vec<SharedString> = keys.iter().map(|k| SharedString::from(k.to_string())).collect();
    move |window, cx| shortcut_tooltip_view(label.clone(), keys.clone(), window, cx)
}

/// The same tooltip as a view, for a caller that draws it in place; with no keys it is the plain hover word.
pub fn shortcut_tooltip_view(label: SharedString, keys: Vec<SharedString>, window: &mut Window, cx: &mut App) -> AnyView {
    tooltip_view(
        move |cx| {
            let named = label.clone();
            div().debug_selector(|| "tooltip".into()).child(
                h_flex()
                    .debug_selector(move || format!("tip:{named}"))
                    .gap_2()
                    .child(label.clone())
                    .when(!keys.is_empty(), |el| el.child(h_flex().gap_1().children(keys.iter().cloned().map(|k| keycap(k, cx))))),
            )
        },
        window,
        cx,
    )
}

/// wsp's tooltip round `content`, for what a plain word cannot carry, such as a path in mono.
pub fn tooltip_view<E: IntoElement>(content: impl Fn(&mut App) -> E + 'static, window: &mut Window, cx: &mut App) -> AnyView {
    let p = cx.palette().clone();
    Tooltip::element(move |_, cx| content(cx))
        .m_0()
        .rounded(size::BUTTON_RADIUS)
        .py_1()
        .px_2()
        .border_color(p.border)
        .bg(p.popover)
        .text_size(text::FACT.0)
        .line_height(text::FACT.1)
        .text_color(p.foreground)
        .shadow(p.tooltip_shadow.clone())
        .build(window, cx)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use gpui_kit::{
        div, point, prelude::*, px, size, Bounds, Context, Modifiers, Pixels, TestAppContext, TooltipSide, VisualTestContext, Window,
    };

    use crate::ui::{self, Button, Glyph, Side};

    /// The usage head's refresh, where the test puts it, its tooltip on `side`.
    struct Refresh(gpui_kit::Point<Pixels>, TooltipSide);

    impl Render for Refresh {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let button = Button::icon_xs("refresh", Glyph::RefreshCw).tooltip("Read usage and limits again").tooltip_side(self.1);
            div().size_full().child(div().absolute().left(self.0.x).top(self.0.y).child(button))
        }
    }

    /// The button at `at` in a 400 by 300 window, hovered until its tooltip shows: the button's box and the tooltip's
    /// words' box.
    fn hovered(at: gpui_kit::Point<Pixels>, side: TooltipSide, cx: &mut TestAppContext) -> (Bounds<Pixels>, Bounds<Pixels>) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
        });
        let (_, cx): (_, &mut VisualTestContext) = cx.add_window_view(|_, _| Refresh(at, side));
        cx.simulate_resize(size(px(400.), px(300.)));
        cx.run_until_parked();
        let button = cx.debug_bounds("button").expect("the button is drawn");
        cx.simulate_mouse_move(button.center(), None, Modifiers::none());
        for _ in 0..2 {
            cx.run_until_parked();
            cx.executor().advance_clock(Duration::from_millis(600));
            cx.update(|window, cx| window.simulate_next_frame(cx));
        }
        cx.run_until_parked();
        (button, cx.debug_bounds("tooltip").expect("the tooltip shows"))
    }

    /// The words stand inside the tooltip's 1 px edge and its 4 px padding above and below, so 5 px in from its top and
    /// bottom, and inside its 8 px sides, 9 px in from its left and right.
    const INSET: Pixels = px(5.);
    const SIDE_INSET: Pixels = px(9.);

    fn centred_across(button: Bounds<Pixels>, tip: Bounds<Pixels>) {
        assert!((tip.center().x - button.center().x).abs() <= px(0.5), "centred on the button: {button:?} {tip:?}");
    }

    /// `TooltipPopup`'s defaults, `side="top"` and `sideOffset` 4: centred over the button, 4 px above it.
    #[gpui_kit::test]
    fn a_tooltip_opens_centred_above_its_button(cx: &mut TestAppContext) {
        let (button, tip) = hovered(point(px(188.), px(140.)), TooltipSide::Top, cx);
        centred_across(button, tip);
        assert_eq!(tip.bottom() + INSET, button.top() - px(4.), "4 px above the button: {button:?} {tip:?}");
    }

    /// The usage head's refresh says `side="bottom"`: centred under the button, 4 px below it.
    #[gpui_kit::test]
    fn the_refresh_tooltip_opens_centred_below_its_button(cx: &mut TestAppContext) {
        let (button, tip) = hovered(point(px(188.), px(40.)), TooltipSide::Bottom, cx);
        centred_across(button, tip);
        assert_eq!(tip.top() - INSET, button.bottom() + px(4.), "4 px below the button: {button:?} {tip:?}");
    }

    /// The right press menu's refusal says `side="right"`: centred down the button's side, 4 px right of it.
    #[gpui_kit::test]
    fn a_tooltip_on_the_right_opens_beside_its_button(cx: &mut TestAppContext) {
        let (button, tip) = hovered(point(px(40.), px(140.)), TooltipSide::Right, cx);
        assert!((tip.center().y - button.center().y).abs() <= px(0.5), "centred down the button: {button:?} {tip:?}");
        assert_eq!(tip.left() - SIDE_INSET, button.right() + px(4.), "4 px right of the button: {button:?} {tip:?}");
    }

    /// Against the window's right edge it shifts left to stand 5 px inside, still above the button.
    #[gpui_kit::test]
    fn the_tooltip_by_the_right_edge_stays_inside_the_window(cx: &mut TestAppContext) {
        let (button, tip) = hovered(point(px(376.), px(140.)), TooltipSide::Top, cx);
        assert_eq!(tip.right(), px(395.) - SIDE_INSET, "5 px inside the window, past the tooltip's 8 px side and edge: {tip:?}");
        assert_eq!(tip.bottom() + INSET, button.top() - px(4.), "still above the button: {button:?} {tip:?}");
    }

    /// With no room above, it opens below the button, as the positioner flips a side that collides.
    #[gpui_kit::test]
    fn the_tooltip_with_no_room_above_opens_below(cx: &mut TestAppContext) {
        let (button, tip) = hovered(point(px(188.), px(8.)), TooltipSide::Top, cx);
        assert_eq!(tip.top() - INSET, button.bottom() + px(4.), "below the button: {button:?} {tip:?}");
        centred_across(button, tip);
    }
}
