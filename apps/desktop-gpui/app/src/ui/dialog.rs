// SPDX-License-Identifier: AGPL-3.0-only
//! The dialog shell as Add a computer draws its steps (`StepDialog.tsx`): the title with "n of N" at its right and
//! one line under it, cards, and the foot. The copyable command row (`CopyRow`), and the tooltip a shortcut lives in.

use std::time::Duration;

use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{
    div, linear_color_stop, linear_gradient, prelude::*, px, AnyElement, AnyView, App, ClipboardItem, FontWeight, HighlightStyle, Hsla,
    Pixels, SharedString, StyledText, Window,
};

use super::card::{keycap, word, CardItem, WordClass};
use super::controls::Button;
use super::icons::Glyph;
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

/// A dialog of steps at `STEP_WIDTH`: its surface is drawn where it is put, so a caller lays it over a scrim (gpui-kit's
/// dialog layer) or shows it in place.
#[derive(IntoElement)]
pub struct Dialog {
    title: SharedString,
    count: Option<SharedString>,
    line: Option<SharedString>,
    body: Vec<AnyElement>,
    state: Option<AnyElement>,
    acts: Vec<AnyElement>,
}

impl Dialog {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Dialog { title: title.into(), count: None, line: None, body: Vec::new(), state: None, acts: Vec::new() }
    }

    /// The step's place as a quiet fact at the head's right: "2 of 5".
    pub fn count(mut self, step: usize, of: usize) -> Self {
        self.count = Some(format!("{step} of {of}").into());
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

    /// The quiet state at the foot's left: "Saved, you can finish later".
    pub fn state(mut self, state: impl IntoElement) -> Self {
        self.state = Some(state.into_any_element());
        self
    }

    /// An act at the foot's right, in order: the outline secondary, then the one primary.
    pub fn act(mut self, button: Button) -> Self {
        self.acts.push(button.into_any_element());
        self
    }
}

impl RenderOnce for Dialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let head = h_flex()
            .items_start()
            .justify_between()
            .gap_6()
            .px_5()
            .pt_4()
            .pb_3()
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
                    .children(
                        self.line
                            .map(|line| div().text_size(text::NOTE.0).line_height(text::NOTE.1).text_color(p.muted_foreground).child(line)),
                    ),
            )
            .children(self.count.map(|count| div().flex_none().pt(px(2.)).child(word(count, WordClass::Fact, cx))));
        let body = v_flex()
            .id("dialog-body")
            .flex_shrink(1.)
            .min_h_0()
            .overflow_y_scroll()
            .child(v_flex().gap_5().px_5().pt_2().pb_5().children(self.body));
        let foot = h_flex()
            .justify_end()
            .gap_2()
            .px_5()
            .pt_3()
            .pb_4()
            .child(h_flex().flex_1().min_w_0().min_h(px(20.)).children(self.state))
            .children(self.acts);
        v_flex()
            .debug_selector(|| "dialog".into())
            .w(size::DIALOG_WIDTH)
            .max_h(popup_max(window))
            .rounded(size::DIALOG_RADIUS)
            .border_1()
            .border_color(p.popover_edge)
            .bg(p.popover)
            .shadow(p.dialog_shadow.clone())
            .text_color(p.foreground)
            .child(head)
            .child(body)
            .child(foot)
    }
}

/// A popup's `max-h-full` inside the viewport's `p-4`: past it the body scrolls and the head and foot stay.
fn popup_max(window: &Window) -> Pixels {
    window.viewport_size().height - px(32.)
}

/// `AlertDialog`: the one confirmation before something that does not come back, and the sheet that edits one value.
/// The title and the sentence computed from what goes, a body (a field, the refusal), and the foot: Cancel with the
/// neutral ring, then the destructive confirm or Save. A surface, laid over a scrim by the caller as `Dialog` is.
#[derive(IntoElement)]
pub struct AlertDialog {
    title: SharedString,
    sentence: Option<AnyElement>,
    body: Vec<AnyElement>,
    acts: Vec<AnyElement>,
}

impl AlertDialog {
    pub fn new(title: impl Into<SharedString>) -> Self {
        AlertDialog { title: title.into(), sentence: None, body: Vec::new(), acts: Vec::new() }
    }

    /// The description under the title: what leaves, read off what the thing holds now.
    pub fn sentence(mut self, sentence: impl IntoElement) -> Self {
        self.sentence = Some(sentence.into_any_element());
        self
    }

    /// One block of the body, `px-5 pt-2`: a copy row, a field, the `Refusal`.
    pub fn child(mut self, el: impl IntoElement) -> Self {
        self.body.push(el.into_any_element());
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
        v_flex()
            .debug_selector(|| "alert-dialog".into())
            .w(ALERT_WIDTH)
            .max_h(popup_max(window))
            .rounded(size::DIALOG_RADIUS)
            .border_1()
            .border_color(p.popover_edge)
            .bg(p.popover)
            .shadow(p.dialog_shadow.clone())
            .text_color(p.foreground)
            .child(head)
            .children(body)
            .child(h_flex().justify_end().gap_2().px_5().pt_3().pb_4().children(self.acts))
    }
}

/// `RefusalSlot`: the host's refusal under what it refused, in the destructive ink, its fix in the foreground; or what
/// the dialog waits on, quiet. It keeps two lines of room so nothing moves when one lands.
#[derive(IntoElement)]
pub struct Refusal {
    said: Option<SharedString>,
    fix: Option<SharedString>,
    quiet: Option<SharedString>,
}

impl Refusal {
    pub fn new() -> Self {
        Refusal { said: None, fix: None, quiet: None }
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

impl RenderOnce for Refusal {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let quiet = if self.said.is_none() { self.quiet } else { None };
        let runs: Vec<(SharedString, Hsla)> = [
            self.said.map(|s| (s, p.error_foreground)),
            self.fix.map(|f| (format!(" {f}").into(), p.foreground)),
            quiet.map(|q| (q, p.muted_foreground)),
        ]
        .into_iter()
        .flatten()
        .collect();
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
            .text_size(text::FACT)
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
    let p = cx.palette().clone();
    Tooltip::element(move |_, cx| {
        h_flex()
            .gap_2()
            .child(label.clone())
            .when(!keys.is_empty(), |el| el.child(h_flex().gap_1().children(keys.iter().cloned().map(|k| keycap(k, cx)))))
    })
    .m_0()
    .rounded(size::BUTTON_RADIUS)
    .py_1()
    .px_2()
    .border_color(p.border)
    .bg(p.popover)
    .text_size(text::FACT)
    .line_height(px(19.5))
    .text_color(p.foreground)
    .shadow(p.tooltip_shadow.clone())
    .build(window, cx)
}
