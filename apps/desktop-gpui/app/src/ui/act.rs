// SPDX-License-Identifier: AGPL-3.0-only
//! `ActButton`: a row's or a detail's act (`RowAct`) as an xs button with its glyph, held with its reason where it has
//! no road, asking once first where what it does does not come back.

use std::rc::Rc;

use gpui_kit::{div, prelude::*, AnyElement, App, SharedString, Window};

use super::dialog::tip;
use super::{Button, Glyph};

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

    /// Asks first, with this title and sentence, before it runs.
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

/// The confirmation an act asks before it runs: its title and sentence, then its own label and glyph on the red
/// button that runs `run`.
pub struct Ask {
    pub title: String,
    pub body: String,
    pub label: String,
    pub glyph: Option<Glyph>,
    pub run: Run,
}

/// Where a screen stands an act's confirmation: the screen holds the dialog slot.
pub trait Asks: Clone + 'static {
    fn ask(&self, ask: Ask, cx: &mut App);
}

/// `ActButton`: an xs outline button with its glyph (Add's plus, the danger door for one that destroys), held where it
/// has no road, its reason on the hover; one that asks first opens its confirmation through `asks`.
pub fn act_button(key: impl Into<SharedString>, act: &Act, asks: &impl Asks) -> AnyElement {
    let key: SharedString = key.into();
    let mut button = if act.add {
        Button::add(key.clone(), act.label.clone())
    } else if act.destructive {
        Button::danger(key.clone(), act.label.clone())
    } else {
        Button::outline(key.clone(), act.label.clone())
    }
    .xs()
    .held(act.run.is_none());
    if !act.add {
        if let Some(g) = act.glyph {
            button = button.glyph(g);
        }
    }
    button = button.busy(act.busy);
    if let Some(run) = act.run.clone() {
        match act.confirm.clone() {
            None => button = button.on_click(move |_, window, cx| run(window, cx)),
            Some((title, body)) => {
                let (asks, label, glyph) = (asks.clone(), act.label.clone(), act.glyph);
                button = button.on_click(move |_, _, cx| {
                    let ask = Ask { title: title.clone(), body: body.clone(), label: label.clone(), glyph, run: run.clone() };
                    asks.ask(ask, cx)
                });
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

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui_kit::component::Root;
    use gpui_kit::{div, prelude::*, px, size, App, Context, Modifiers, TestAppContext, Window};

    use super::{act_button, Act, Ask, Asks};
    use crate::ui::{self, Glyph, Side};

    #[derive(Clone, Default)]
    struct Asked(Rc<Cell<u32>>);

    impl Asks for Asked {
        fn ask(&self, _: Ask, _: &mut App) {
            self.0.set(self.0.get() + 1);
        }
    }

    struct Lone(Act, Asked);

    impl Render for Lone {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().p(px(24.)).child(act_button("act", &self.0, &self.1))
        }
    }

    /// Clicks the act drawn alone and says how often it ran and how often it asked first.
    fn click(act: impl FnOnce(Act) -> Act, cx: &mut TestAppContext) -> (u32, u32) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
        });
        let ran = Rc::new(Cell::new(0));
        let counted = ran.clone();
        let act = act(Act::new("check", "Check", Some(Glyph::Activity)).run(move |_, _| counted.set(counted.get() + 1)));
        let asked = Asked::default();
        let view = cx.new(|_| Lone(act, asked.clone()));
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(view, window, cx));
        cx.simulate_resize(size(px(320.), px(120.)));
        cx.run_until_parked();
        let button = cx.debug_bounds("button").expect("the act is drawn");
        cx.simulate_click(button.center(), Modifiers::none());
        cx.run_until_parked();
        (ran.get(), asked.0.get())
    }

    #[gpui_kit::test]
    fn an_act_with_a_road_runs_on_a_click(cx: &mut TestAppContext) {
        assert_eq!(click(|a| a, cx), (1, 0));
    }

    #[gpui_kit::test]
    fn a_held_act_takes_no_click(cx: &mut TestAppContext) {
        assert_eq!(click(|a| a.held("not in this wsp yet"), cx), (0, 0));
    }

    #[gpui_kit::test]
    fn a_busy_act_takes_no_click(cx: &mut TestAppContext) {
        assert_eq!(click(Act::busy, cx), (0, 0));
    }

    #[gpui_kit::test]
    fn an_act_that_asks_first_asks_and_does_not_run(cx: &mut TestAppContext) {
        assert_eq!(click(|a| a.destructive().confirm("Remove?".into(), "It goes.".into()), cx), (0, 1));
    }
}
