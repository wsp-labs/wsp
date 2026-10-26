// SPDX-License-Identifier: AGPL-3.0-only
//! The small pieces the Agents pages draw that `ui/` has no piece for yet: a state as its word and dot, a list's head
//! with when it was read and its refresh, how an act asks first on these pages, a row's lead, the agents'
//! marks after a name, and the refusal slot.

use gpui_kit::component::h_flex;
use gpui_kit::component::input::{Input, InputState};
use std::sync::Arc;

use gpui_kit::{div, img, prelude::*, px, AnyElement, App, Entity, Focusable as _, FontWeight, Image, ObjectFit, SharedString, Window};

use super::model::Tone;
use crate::screens::settings::page::DialogBuild;
use crate::screens::settings::Acts;
use crate::ui::catalog;
use crate::ui::{framed, glyph, text, tip, Act, ActivePalette, AlertDialog, Ask, Asks, Button, Glyph, Refusal, Run};

/// `notYet`: drawn where it will stand, held with the reason.
pub fn not_yet(id: &'static str, label: impl Into<String>, glyph: Glyph) -> Act {
    Act::new(id, label, Some(glyph)).hover(super::model::w::NOT_YET)
}

/// The confirmation an act after which something does not come back asks once, its own button the one red at rest.
impl Asks for Acts {
    fn ask(&self, ask: Ask, cx: &mut App) {
        let close = self.clone();
        let Ask { title, body, label, glyph, run } = ask;
        self.open_dialog(
            DialogBuild::alert(move |_, _, _| {
                let (go_close, cancel_close, run) = (close.clone(), close.clone(), run.clone());
                let mut go = Button::destructive("confirm-go", label.clone()).on_click(move |_, window, cx| {
                    go_close.close_dialog(cx);
                    run(window, cx);
                });
                if let Some(g) = glyph {
                    go = go.glyph(g);
                }
                AlertDialog::new(title.clone())
                    .layered()
                    .sentence(body.clone())
                    .act(
                        Button::outline("confirm-cancel", super::model::w::CANCEL)
                            .neutral_ring()
                            .on_click(move |_, _, cx| cancel_close.close_dialog(cx)),
                    )
                    .act(go)
                    .into_any_element()
            }),
            cx,
        );
    }
}

/// `StatusWord`: the state's word in the page's muted ink, then its 8 px dot.
pub fn status_word(key: impl Into<SharedString>, word: &str, tone: Tone, hover: Option<&str>, cx: &App) -> AnyElement {
    let p = cx.palette();
    let dot = match tone {
        Tone::Good => p.success,
        Tone::Waiting => p.warning,
        Tone::Bad => p.destructive,
        Tone::Quiet => p.foreground.opacity(0.3),
    };
    let key: SharedString = key.into();
    h_flex()
        .id(key)
        .debug_selector(|| "agent-status".into())
        .flex_none()
        .gap_2()
        .text_size(text::NOTE.0)
        .line_height(text::NOTE.1)
        .text_color(p.muted_foreground)
        .child(word.to_owned())
        .child(div().flex_none().size(px(8.)).rounded_full().bg(dot))
        .when_some(hover.map(str::to_owned), |el, hover| el.tooltip(tip(hover)))
        .into_any_element()
}

/// A row's act as its glyph alone, its name on the hover (`StepMark`).
pub fn step_mark(key: impl Into<SharedString>, act: &Act) -> AnyElement {
    let mut button = Button::icon_xs(key.into(), act.glyph.unwrap_or(Glyph::RefreshCw)).tooltip(act.label.clone()).held(act.run.is_none());
    if act.busy {
        button = Button::icon_xs(format!("{}-busy", act.id), Glyph::LoaderCircle).busy(true);
    }
    if let Some(run) = act.run.clone() {
        button = button.on_click(move |_, window, cx| run(window, cx));
    }
    button.into_any_element()
}

/// `OnHead`: a list's head with when it was last read and the refresh, which turns while a read runs.
pub fn on_head(
    key: &str,
    head: AnyElement,
    read_when: Option<String>,
    reading: bool,
    refresh: Option<Run>,
    again: &str,
    cx: &App,
) -> AnyElement {
    let p = cx.palette();
    let refresh = refresh.map(|run| {
        let button = Button::icon_xs(SharedString::from(format!("{key}-refresh")), Glyph::RotateCw).tooltip(again.to_owned()).busy(reading);
        if reading {
            button
        } else {
            button.on_click(move |_, window, cx| run(window, cx))
        }
    });
    h_flex()
        .debug_selector(|| "section-head".into())
        .w_full()
        .min_h(crate::ui::size::SECTION_HEAD_FLOOR)
        .gap_3()
        .text_size(text::SECTION_HEAD.0)
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.foreground.opacity(0.7))
        .child(div().flex_1().min_w_0().truncate().child(head))
        .children(
            read_when.map(|when| {
                div().flex_none().text_size(px(12.5)).font_weight(FontWeight::NORMAL).text_color(p.muted_foreground).child(when)
            }),
        )
        .children(refresh)
        .into_any_element()
}

/// The quiet head over a card, as `section_head` draws it, from an element: a group's name with its folder after it.
pub fn group_head(name: String, path: Option<String>, cx: &App) -> AnyElement {
    let p = cx.palette();
    h_flex()
        .min_w_0()
        .items_baseline()
        .gap_2()
        .child(div().min_w_0().truncate().child(name))
        .children(path.map(|path| {
            div()
                .flex_none()
                .font_weight(FontWeight::NORMAL)
                .text_size(text::FACT.0)
                .text_color(p.muted_foreground)
                .font_features(text::tabular())
                .child(path)
        }))
        .into_any_element()
}

/// A card's head as an element, standing where `section_head` stands.
pub fn plain_head(head: AnyElement, cx: &App) -> AnyElement {
    h_flex()
        .debug_selector(|| "section-head".into())
        .min_h(crate::ui::size::SECTION_HEAD_FLOOR)
        .text_size(text::SECTION_HEAD.0)
        .font_weight(FontWeight::MEDIUM)
        .text_color(cx.palette().foreground.opacity(0.7))
        .child(head)
        .into_any_element()
}

/// What leads a row or a detail's head (`Lead`).
#[derive(Clone)]
pub enum Lead {
    Agent(String),
    /// A tool server: its company's mark, else its own icon as the host fetched it, else the glyph.
    Server {
        mark: Option<&'static str>,
        icon: Option<Arc<Image>>,
    },
    Glyph(Glyph),
}

/// `LeadMark` bare, for a head that frames it itself.
pub fn lead_mark(lead: &Lead, label: &str, cx: &App) -> AnyElement {
    let ink = cx.palette().foreground;
    match lead {
        Lead::Agent(id) => catalog::mark(id, label, px(16.), ink, cx),
        Lead::Server { mark: Some(mark), .. } => {
            catalog::brand_mark(mark, px(18.), ink, cx).unwrap_or_else(|| glyph(Glyph::Server, cx).into_any_element())
        }
        Lead::Server { icon: Some(icon), .. } => img(icon.clone())
            .debug_selector(|| "server-icon".into())
            .size(px(20.))
            .rounded(px(4.))
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
        Lead::Server { .. } => glyph(Glyph::Server, cx).into_any_element(),
        Lead::Glyph(g) => glyph(*g, cx).into_any_element(),
    }
}

/// `LeadMark` in the page's glyph frame: an agent's mark, a server's mark, icon or glyph, the kind's glyph.
pub fn lead(lead: &Lead, label: &str, cx: &App) -> AnyElement {
    framed(lead_mark(lead, label, cx), cx).into_any_element()
}

/// `AgentMarks`: the marks of the agents a thing is set up for, four, then how many more; every name on the hover.
pub fn agent_marks(key: impl Into<SharedString>, agents: &[String], cx: &App) -> Option<AnyElement> {
    if agents.is_empty() {
        return None;
    }
    let names = agents.iter().map(|a| catalog::name(a)).collect::<Vec<_>>().join(", ");
    let more = agents.len().saturating_sub(4);
    Some(
        h_flex()
            .id(key.into())
            .flex_none()
            .gap_1()
            .tooltip(tip(names))
            .children(agents.iter().take(4).map(|a| catalog::mark(a, &catalog::name(a), px(14.), cx.palette().foreground, cx)))
            .when(more > 0, |el| el.child(format!("+{more}")))
            .into_any_element(),
    )
}

/// `RefusalSlot`: the host's refusal in the danger ink, keeping its room while empty.
pub fn refusal(said: Option<&str>) -> AnyElement {
    match said {
        Some(said) => Refusal::new().said(said.to_owned()).into_any_element(),
        None => Refusal::new().into_any_element(),
    }
}

/// A field the page types into at the row control's size, wider or narrower than a select.
pub fn field(input: &Entity<InputState>, width: Option<gpui_kit::Pixels>, cx: &App) -> AnyElement {
    sized(Input::new(input).border_color(cx.palette().input), width, cx)
}

/// A field across its row whose words were refused: `aria-invalid`, its edge the destructive ink, stronger in focus.
pub fn refused_field(input: &Entity<InputState>, refused: bool, window: &Window, cx: &App) -> AnyElement {
    let p = cx.palette();
    let edge = match (refused, input.read(cx).focus_handle(cx).is_focused(window)) {
        (true, true) => p.destructive.opacity(0.64),
        (true, false) => p.destructive.opacity(0.36),
        (false, _) => p.input,
    };
    sized(Input::new(input).focus_bordered(!refused).border_color(edge), None, cx)
}

fn sized(input: Input, width: Option<gpui_kit::Pixels>, cx: &App) -> AnyElement {
    let el = Styled::h(input, px(30.)).rounded(px(7.)).px(px(11.)).py_0().bg(cx.palette().input_fill).text_size(text::FACT.0);
    match width {
        Some(w) => div().flex_none().w(w).child(el.w(w)).into_any_element(),
        None => div().flex_1().min_w_0().child(el.w_full()).into_any_element(),
    }
}

/// `FIELD` of a recipe row, the sign-in's paste field: 32 px, the mono at 12 px, the page's ground.
pub fn code_field(input: &Entity<InputState>, cx: &App) -> AnyElement {
    let p = cx.palette();
    div()
        .debug_selector(|| "code-field".into())
        .flex_1()
        .min_w_0()
        .h(px(32.))
        .flex()
        .items_center()
        .px(px(9.))
        .rounded(crate::ui::size::BUTTON_RADIUS)
        .border_1()
        .border_color(p.input)
        .bg(p.background)
        .font_family(gpui_kit::component::ActiveTheme::theme(cx).mono_font_family.clone())
        .text_size(text::XS.0)
        .child(Input::new(input).appearance(false).w_full())
        .into_any_element()
}

/// A line of text in the `XS` muted class, a sign-in's label.
pub fn xs_label(words: impl Into<SharedString>, cx: &App) -> AnyElement {
    div().text_size(text::XS.0).line_height(text::XS.1).text_color(cx.palette().muted_foreground).child(words.into()).into_any_element()
}
