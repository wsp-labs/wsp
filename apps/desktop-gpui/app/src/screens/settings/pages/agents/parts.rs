// SPDX-License-Identifier: AGPL-3.0-only
//! The small pieces the Agents pages draw that `ui/` has no piece for yet: the switch, the segmented control, a state
//! as its word and dot, a list's head with when it was read and its refresh, a row's act as an xs button that may ask
//! first, a row's lead, the agents' marks after a name, and the refusal slot.

use std::rc::Rc;

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Entity, FontWeight, SharedString, Window};

use super::marks::harness_mark;
use super::model::{agent_name, Tone};
use crate::screens::settings::Acts;
use crate::ui::{framed, glyph, spinner, text, tip, ActivePalette, AlertDialog, Button, Glyph, Refusal};

pub type Run = Rc<dyn Fn(&mut Window, &mut App)>;

/// A row's or a detail's act (`RowAct`): no `run` is held.
#[derive(Clone)]
pub struct Act {
    pub id: &'static str,
    pub label: String,
    pub glyph: Option<Glyph>,
    pub add: bool,
    pub hover: Option<String>,
    pub destructive: bool,
    pub run: Option<Run>,
    pub busy: bool,
    pub confirm: Option<(String, String)>,
}

impl Act {
    pub fn new(id: &'static str, label: impl Into<String>, glyph: Option<Glyph>) -> Act {
        Act { id, label: label.into(), glyph, add: false, hover: None, destructive: false, run: None, busy: false, confirm: None }
    }

    /// `notYet`: drawn where it will stand, held with the reason.
    pub fn not_yet(id: &'static str, label: impl Into<String>, glyph: Glyph) -> Act {
        Act::new(id, label, Some(glyph)).hover(super::model::w::NOT_YET)
    }

    pub fn hover(mut self, hover: impl Into<String>) -> Act {
        self.hover = Some(hover.into());
        self
    }

    pub fn run(mut self, run: impl Fn(&mut Window, &mut App) + 'static) -> Act {
        self.run = Some(Rc::new(run));
        self
    }

    pub fn busy(mut self) -> Act {
        self.busy = true;
        self.run = None;
        self
    }

    pub fn destructive(mut self) -> Act {
        self.destructive = true;
        self
    }

    pub fn add(mut self) -> Act {
        self.add = true;
        self
    }

    pub fn confirm(mut self, title: String, body: String) -> Act {
        self.confirm = Some((title, body));
        self
    }

    /// `wordOnly`: the act drawn without its glyph.
    pub fn word_only(mut self) -> Act {
        self.glyph = None;
        self
    }

    /// `holdAll`'s one act: held with the reason, its road and its spinner gone.
    pub fn held(mut self, why: &str) -> Act {
        self.run = None;
        self.busy = false;
        self.hover = Some(why.to_owned());
        self
    }
}

/// `ActButton`: an xs outline button with its glyph, held where it has no road; one that asks first opens its
/// confirmation.
pub fn act_button(key: impl Into<SharedString>, act: &Act, acts: &Acts) -> AnyElement {
    let key: SharedString = key.into();
    let held = act.run.is_none();
    let mut button = if act.add {
        Button::add(key.clone(), act.label.clone())
    } else if act.destructive {
        Button::danger(key.clone(), act.label.clone())
    } else {
        Button::outline(key.clone(), act.label.clone())
    }
    .xs()
    .held(held);
    if !act.add {
        if let Some(g) = act.glyph {
            button = button.glyph(g);
        }
    }
    if act.busy {
        button = button.glyph(Glyph::LoaderCircle);
    }
    if let Some(run) = act.run.clone() {
        match act.confirm.clone() {
            None => button = button.on_click(move |_, window, cx| run(window, cx)),
            Some((title, body)) => {
                let (acts, label, glyph) = (acts.clone(), act.label.clone(), act.glyph);
                button =
                    button.on_click(move |_, _, cx| confirm(&acts, title.clone(), body.clone(), label.clone(), glyph, run.clone(), cx));
            }
        }
    }
    match &act.hover {
        Some(hover) => {
            div().id(SharedString::from(format!("{key}-hover"))).flex().tooltip(tip(hover.clone())).child(button).into_any_element()
        }
        None => button.into_any_element(),
    }
}

/// The confirmation an act after which something does not come back asks once, its own button the one red at rest.
fn confirm(acts: &Acts, title: String, body: String, label: String, glyph: Option<Glyph>, run: Run, cx: &mut App) {
    let close = acts.clone();
    acts.open_dialog(
        Rc::new(move |_, _, _| {
            let (go_close, cancel_close, run) = (close.clone(), close.clone(), run.clone());
            let mut go = Button::destructive("confirm-go", label.clone()).on_click(move |_, window, cx| {
                go_close.close_dialog(cx);
                run(window, cx);
            });
            if let Some(g) = glyph {
                go = go.glyph(g);
            }
            AlertDialog::new(title.clone())
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
        button = Button::icon_xs(format!("{}-busy", act.id), Glyph::LoaderCircle).held(true);
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
        let button =
            Button::icon_xs(SharedString::from(format!("{key}-refresh")), if reading { Glyph::LoaderCircle } else { Glyph::RotateCw })
                .tooltip(again.to_owned())
                .held(reading);
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
        .text_size(text::SECTION_HEAD)
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
        .children(
            path.map(|path| {
                div().flex_none().font_weight(FontWeight::NORMAL).text_size(text::FACT).text_color(p.muted_foreground).child(path)
            }),
        )
        .into_any_element()
}

/// A card's head as an element, standing where `section_head` stands.
pub fn plain_head(head: AnyElement, cx: &App) -> AnyElement {
    h_flex()
        .debug_selector(|| "section-head".into())
        .min_h(crate::ui::size::SECTION_HEAD_FLOOR)
        .text_size(text::SECTION_HEAD)
        .font_weight(FontWeight::MEDIUM)
        .text_color(cx.palette().foreground.opacity(0.7))
        .child(head)
        .into_any_element()
}

/// What leads a row or a detail's head (`Lead`).
#[derive(Clone)]
pub enum Lead {
    Agent(String),
    Box(Glyph),
    Glyph(Glyph),
}

/// `LeadMark` bare, in the page's glyph frame: an agent's mark, a server's glyph, the kind's glyph.
pub fn lead(lead: &Lead, label: &str, cx: &App) -> AnyElement {
    match lead {
        Lead::Agent(id) => framed(harness_mark(id, label, px(16.), cx), cx).into_any_element(),
        Lead::Box(g) | Lead::Glyph(g) => framed(glyph(*g, cx), cx).into_any_element(),
    }
}

/// `AgentMarks`: the marks of the agents a thing is set up for, four, then how many more; every name on the hover.
pub fn agent_marks(key: impl Into<SharedString>, agents: &[String], cx: &App) -> Option<AnyElement> {
    if agents.is_empty() {
        return None;
    }
    let names = agents.iter().map(|a| agent_name(a)).collect::<Vec<_>>().join(", ");
    let more = agents.len().saturating_sub(4);
    Some(
        h_flex()
            .id(key.into())
            .flex_none()
            .gap_1()
            .tooltip(tip(names))
            .children(agents.iter().take(4).map(|a| harness_mark(a, &agent_name(a), px(14.), cx)))
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
    let p = cx.palette();
    let el = Styled::h(Input::new(input), px(30.))
        .rounded(px(7.))
        .px(px(11.))
        .py_0()
        .border_color(p.input)
        .bg(p.input_fill)
        .text_size(text::FACT);
    match width {
        Some(w) => div().flex_none().w(w).child(el.w(w)).into_any_element(),
        None => div().flex_1().min_w_0().child(el.w_full()).into_any_element(),
    }
}

/// A line of text in the `XS` muted class, a sign-in's label.
pub fn xs_label(words: impl Into<SharedString>, cx: &App) -> AnyElement {
    div().text_size(text::XS.0).line_height(text::XS.1).text_color(cx.palette().muted_foreground).child(words.into()).into_any_element()
}

/// A column of parts at the given gap.
pub fn stack(gap: gpui_kit::Pixels, parts: Vec<AnyElement>) -> AnyElement {
    v_flex().gap(gap).children(parts).into_any_element()
}

/// The plain spinner as an element.
pub fn spin(cx: &App) -> AnyElement {
    spinner(cx).into_any_element()
}
