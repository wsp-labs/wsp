// SPDX-License-Identifier: AGPL-3.0-only
//! A row's menu, after the web's action registry and `menu.tsx`: an ellipsis on the ghost icon button, its acts on a
//! press in gpui-kit's popup menu, a rule between groups, the acts that destroy in the danger ink at rest.

use std::rc::Rc;

use gpui_kit::component::menu::{DropdownMenu, PopupMenu, PopupMenuItem};
use gpui_kit::component::{h_flex, ActiveTheme as _, Icon, Selectable};
use gpui_kit::{div, prelude::*, px, Corners, Div, Hsla, Interactivity, SharedString, Stateful, StyleRefinement};

use super::act::Run;
use super::dialog::tip;
use super::theme::ActivePalette;
use super::{Button, Glyph};

/// One act of a row's menu: its group (a rule stands between groups), glyph, title, whether it destroys, and whether
/// it is held while it cannot run, with why on its hover; and its chord, as a right press's menu shows one.
#[derive(Clone)]
pub struct MenuAct {
    pub group: u8,
    pub glyph: Option<Glyph>,
    pub title: SharedString,
    pub destructive: bool,
    pub held: bool,
    pub why: Option<SharedString>,
    pub keys: Option<SharedString>,
    pub run: Run,
}

/// The acts as a popup menu, a rule between their groups: the row menu's list, and a row's right press.
pub fn menu_of(mut menu: PopupMenu, acts: &[MenuAct], danger: Hsla, muted: Hsla) -> PopupMenu {
    let mut last: Option<u8> = None;
    for (ix, act) in acts.iter().enumerate() {
        if last.is_some_and(|g| g != act.group) {
            menu = menu.separator();
        }
        last = Some(act.group);
        let run = act.run.clone();
        let item = if act.destructive || act.why.is_some() || act.keys.is_some() {
            // A held act takes the menu's held ink, the danger ink only where it can run.
            let (title, why, keys, destructive) = (act.title.clone(), act.why.clone(), act.keys.clone(), act.destructive && !act.held);
            PopupMenuItem::element(move |_, cx| {
                let row = h_flex()
                    .id(("menu-act", ix))
                    .w_full()
                    .gap(px(12.))
                    .child(div().flex_1().min_w_0().when(destructive, |el| el.text_color(danger)).child(title.clone()))
                    .children(
                        keys.clone().map(|k| div().flex_none().font_family(cx.theme().mono_font_family.clone()).text_color(muted).child(k)),
                    );
                match why.clone() {
                    Some(why) => row.tooltip(tip(why)).into_any_element(),
                    None => row.into_any_element(),
                }
            })
        } else {
            PopupMenuItem::new(act.title.clone())
        };
        let item = match act.glyph {
            Some(glyph) => item.icon(Icon::new(glyph).when(act.destructive && !act.held, |icon| icon.text_color(danger))),
            None => item,
        };
        // gpui-kit's first Down lands on row 0 and Enter runs whatever handler it holds, so a held act carries none.
        let item = item.disabled(act.held);
        menu = menu.item(if act.held { item } else { item.on_click(move |_, window, cx| run(window, cx)) });
    }
    menu
}

/// The button a popup opens from: gpui-kit's popover takes the press on the element round it.
struct Trigger(Stateful<Div>);

impl Styled for Trigger {
    fn style(&mut self) -> &mut StyleRefinement {
        self.0.style()
    }
}

impl InteractiveElement for Trigger {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.0.interactivity()
    }
}

impl Selectable for Trigger {
    fn selected(self, _: bool) -> Self {
        self
    }

    fn is_selected(&self) -> bool {
        false
    }
}

impl IntoElement for Trigger {
    type Element = Stateful<Div>;

    fn into_element(self) -> Self::Element {
        self.0
    }
}

impl DropdownMenu for Trigger {}

/// The row's menu: the ellipsis on the 20 px ghost icon button (`MENU_BUTTON`), its acts on a press.
pub fn row_menu(id: impl Into<SharedString>, acts: Rc<Vec<MenuAct>>) -> impl IntoElement {
    let id: SharedString = id.into();
    let button = Button::icon_xs(id.clone(), Glyph::Ellipsis).side(px(20.)).corners(Corners::all(px(4.)));
    Trigger(div().id(SharedString::from(format!("{id}-menu"))).flex().child(button))
        .dropdown_menu(move |menu, _, cx| menu_of(menu, &acts, cx.palette().error_foreground, cx.palette().muted_foreground))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui_kit::component::Root;
    use gpui_kit::{div, prelude::*, px, size, Context, TestAppContext, Window};

    use super::{row_menu, MenuAct};
    use crate::ui::{self, Glyph, Side};

    struct Lone(Rc<Vec<MenuAct>>);

    impl Render for Lone {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().p(px(24.)).child(row_menu("row", self.0.clone()))
        }
    }

    #[gpui_kit::test]
    fn a_press_opens_the_menu_and_an_item_runs_its_act(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
        });
        let (copied, ended) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
        let act = |glyph, title: &'static str, destructive, ran: &Rc<Cell<u32>>| {
            let ran = ran.clone();
            let run: crate::ui::act::Run = Rc::new(move |_, _| ran.set(ran.get() + 1));
            MenuAct { group: 0, glyph: Some(glyph), title: title.into(), destructive, held: false, why: None, keys: None, run }
        };
        let acts = Rc::new(vec![act(Glyph::ClipboardCopy, "Copy", false, &copied), act(Glyph::SquareX, "End", true, &ended)]);
        let view = cx.new(|_| Lone(acts));
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(view, window, cx));
        cx.simulate_resize(size(px(320.), px(240.)));
        cx.run_until_parked();

        let trigger = cx.debug_bounds("button").expect("the trigger is drawn");
        assert_eq!((trigger.size.width, trigger.size.height), (px(20.), px(20.)), "the trigger is `MENU_BUTTON`'s size-5");
        cx.simulate_keystrokes("down down enter");
        cx.run_until_parked();
        assert_eq!((copied.get(), ended.get()), (0, 0), "with the menu shut the keys run nothing");
        cx.simulate_click(trigger.center(), gpui_kit::Modifiers::none());
        cx.run_until_parked();
        cx.simulate_keystrokes("down down enter");
        cx.run_until_parked();
        assert_eq!((copied.get(), ended.get()), (0, 1), "the second item, End, ran once and Copy did not");
    }
}
