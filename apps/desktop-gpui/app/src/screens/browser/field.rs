// SPDX-License-Identifier: AGPL-3.0-only
//! The browser pane's address bar (`PreviewChromeRow.tsx`): the page's address while it rests, everything selected
//! on focus, Enter opening what is typed and Escape putting the address back. With no page it is the servers list's
//! filter too, so what was typed stays after focus leaves.

use gpui_kit::component::input::{Escape, Input, InputEvent, InputState};
use gpui_kit::{div, prelude::*, px, App, Context, Entity, EventEmitter, Focusable as _, MouseButton, SharedString, Subscription, Window};

use super::chrome::Act;
use crate::ui::{motion, size, ActivePalette, Button, Glyph};

pub const PLACEHOLDER: &str = "Search or enter URL";

#[derive(Clone, Debug, PartialEq)]
pub enum FieldEvent {
    /// Enter on what is typed, trimmed and never empty.
    Submit(String),
    /// What is typed, each time it changes; the address itself once it moves.
    Draft(String),
}

pub struct AddressField {
    input: Entity<InputState>,
    /// The page's address, empty with no page.
    url: String,
    /// What is typed, which the list filters on.
    draft: String,
    /// The pane's page opens in the system browser from the button at the field's end; the chrome row sets it.
    outside: Option<Act>,
    /// The address moved and the input shows it once the field draws.
    moved: bool,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<FieldEvent> for AddressField {}

impl AddressField {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(PLACEHOLDER));
        let typed = cx.subscribe_in(&input, window, Self::on_input);
        AddressField { input, url: String::new(), draft: String::new(), outside: None, moved: false, _subscriptions: vec![typed] }
    }

    /// The page's address moved: the field shows it, and what was typed gives way to it.
    pub fn set_url(&mut self, url: impl Into<String>, cx: &mut Context<Self>) {
        let url = url.into();
        if url == self.url {
            return;
        }
        self.url = url.clone();
        self.moved = true;
        self.redraft(url, cx);
        cx.notify();
    }

    /// What the system browser button does, none while the pane has no page; the chrome row sets it as it draws.
    pub(super) fn set_outside(&mut self, outside: Option<Act>) {
        self.outside = outside;
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn draft(&self) -> &str {
        &self.draft
    }

    /// What the field shows.
    pub fn text(&self, cx: &App) -> SharedString {
        self.input.read(cx).value()
    }

    pub fn focused(&self, window: &Window, cx: &App) -> bool {
        self.input.read(cx).focus_handle(cx).is_focused(window)
    }

    fn show(&self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| {
            if input.value() != text {
                input.set_value(text.to_owned(), window, cx);
            }
        });
    }

    fn redraft(&mut self, draft: String, cx: &mut Context<Self>) {
        if draft != self.draft {
            self.draft = draft.clone();
            cx.emit(FieldEvent::Draft(draft));
        }
    }

    fn on_input(&mut self, input: &Entity<InputState>, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Change => {
                let typed = input.read(cx).value().to_string();
                self.redraft(typed, cx);
            }
            InputEvent::Focus => {
                if !self.url.is_empty() {
                    let url = self.url.clone();
                    self.show(&url, window, cx);
                    self.redraft(url, cx);
                }
                input.update(cx, |input, cx| input.select_all(window, cx));
                cx.notify();
            }
            InputEvent::Blur => {
                if !self.url.is_empty() {
                    let url = self.url.clone();
                    self.show(&url, window, cx);
                }
                cx.notify();
            }
            InputEvent::PressEnter { .. } => {
                let next = input.read(cx).value().trim().to_owned();
                if next.is_empty() {
                    return;
                }
                cx.emit(FieldEvent::Submit(next));
                window.blur(cx);
            }
        }
    }

    fn on_escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        let url = self.url.clone();
        self.show(&url, window, cx);
        self.redraft(url, cx);
        window.blur(cx);
        cx.notify();
    }
}

impl Render for AddressField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.moved) {
            let url = self.url.clone();
            self.show(&url, window, cx);
        }
        let p = cx.palette().clone();
        let id = SharedString::from(format!("browser-address-{}", cx.entity_id()));
        let focused = self.input.read(cx).focus_handle(cx).is_focused(window);
        let over = motion::hovered(id.clone(), window, cx);
        let hovered = *over.read(cx);
        let outside = self.outside.clone().filter(|_| !focused);
        let clear = p.ring.opacity(0.);
        let edge = motion::ease_ink(id.clone(), "edge", if focused { p.ring } else { clear }, motion::COLORS, window, cx);
        let ground = match (focused, hovered) {
            (true, _) => p.background,
            (false, true) => p.muted.opacity(0.4),
            (false, false) => p.background.opacity(0.),
        };
        let shown = f32::from(motion::ease_px(
            id.clone(),
            "outside",
            px(if outside.is_some() && hovered { 1. } else { 0. }),
            motion::COLORS,
            window,
            cx,
        ));
        // The group's `pe-1.5` on its input while the system browser button stands at its end.
        let end = if outside.is_some() { px(6.) } else { px(9.) };
        let input = Styled::h(Input::new(&self.input).appearance(false), px(26.))
            .w_full()
            .pl(px(9.))
            .pr(end)
            .py_0()
            .text_size(px(14.))
            .text_color(p.foreground);
        div()
            .id(id.clone())
            .debug_selector(|| "browser-address".into())
            .relative()
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .h(px(28.))
            .rounded(size::BUTTON_RADIUS)
            .border_1()
            .border_color(edge)
            .bg(ground)
            .on_hover(motion::track_hover(over))
            .on_action(cx.listener(Self::on_escape))
            .child(input)
            .when_some(outside, |el, outside| {
                el.child(
                    div()
                        .debug_selector(|| "browser-outside".into())
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right(px(1.))
                        .flex()
                        .items_center()
                        .opacity(shown)
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            Button::icon_xs(SharedString::from(format!("{id}-outside")), Glyph::ExternalLink)
                                .tooltip("Open in system browser")
                                .on_click(move |_, window, cx| outside(window, cx)),
                        ),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui_kit::component::Root;
    use gpui_kit::{div, point, px, size, Context, Modifiers, TestAppContext, VisualTestContext, Window};

    use super::*;
    use crate::ui::{self, Side};

    struct Host {
        field: Entity<AddressField>,
        _events: Subscription,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .p(px(20.))
                .flex()
                .flex_col()
                .child(div().flex().w(px(360.)).child(self.field.clone()))
                .child(div().debug_selector(|| "elsewhere".into()).h(px(100.)).w_full())
        }
    }

    type Events = Rc<RefCell<Vec<FieldEvent>>>;

    fn open<'a>(cx: &'a mut TestAppContext, url: &str, outside: Option<Act>) -> (&'a mut VisualTestContext, Entity<AddressField>, Events) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
            cx.set_global(ui::Still(true));
        });
        let events: Events = Default::default();
        let log = events.clone();
        let url = url.to_owned();
        let made: Rc<RefCell<Option<Entity<AddressField>>>> = Default::default();
        let keep = made.clone();
        let (_, cx) = cx.add_window_view(move |window, cx| {
            let field = cx.new(|cx| AddressField::new(window, cx));
            *keep.borrow_mut() = Some(field.clone());
            field.update(cx, |f, cx| {
                f.set_url(url, cx);
                f.set_outside(outside);
            });
            let events = cx.subscribe(&field, move |_, _, e: &FieldEvent, _| log.borrow_mut().push(e.clone()));
            let host = cx.new(|_| Host { field, _events: events });
            Root::new(host, window, cx)
        });
        cx.simulate_resize(size(px(480.), px(240.)));
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        let field = made.borrow().clone().expect("the field is made");
        events.borrow_mut().clear();
        (cx, field, events)
    }

    fn click_field(cx: &mut VisualTestContext) {
        let at = cx.debug_bounds("browser-address").expect("the field is drawn");
        cx.simulate_click(at.center(), Modifiers::none());
        cx.run_until_parked();
    }

    fn state(cx: &mut VisualTestContext, field: &Entity<AddressField>) -> (String, bool, std::ops::Range<usize>) {
        cx.update(|window, cx| {
            let f = field.read(cx);
            let selected = f.input.read(cx).selected_range();
            (f.text(cx).to_string(), f.focused(window, cx), selected)
        })
    }

    #[gpui_kit::test]
    fn the_field_is_28_px_and_focus_selects_the_whole_address(cx: &mut TestAppContext) {
        let (cx, field, _) = open(cx, "localhost:3000/a?x=1", None);
        let at = cx.debug_bounds("browser-address").unwrap();
        assert_eq!((at.size.width, at.size.height), (px(360.), px(28.)));
        click_field(cx);
        let (text, focused, selected) = state(cx, &field);
        assert_eq!((text.as_str(), focused), ("localhost:3000/a?x=1", true));
        assert_eq!(selected, 0..text.len(), "a press in the field selects all of it");
    }

    #[gpui_kit::test]
    fn enter_submits_the_trimmed_text_and_blurs(cx: &mut TestAppContext) {
        let (cx, field, events) = open(cx, "", None);
        click_field(cx);
        cx.simulate_input("  3000/a  ");
        cx.run_until_parked();
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        let (text, focused, _) = state(cx, &field);
        assert_eq!(events.borrow().last(), Some(&FieldEvent::Submit("3000/a".into())));
        assert!(events.borrow().contains(&FieldEvent::Draft("  3000/a  ".into())), "each change of the typed text is a draft");
        assert!(!focused, "Enter blurs the field");
        assert_eq!(text, "  3000/a  ", "with no page the text typed stays after blur");
    }

    #[gpui_kit::test]
    fn enter_on_nothing_submits_nothing(cx: &mut TestAppContext) {
        let (cx, field, events) = open(cx, "", None);
        click_field(cx);
        cx.simulate_input("   ");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert!(!events.borrow().iter().any(|e| matches!(e, FieldEvent::Submit(_))));
        assert!(state(cx, &field).1, "nothing opened, so the field keeps focus");
    }

    #[gpui_kit::test]
    fn escape_puts_the_address_back_and_blurs(cx: &mut TestAppContext) {
        let (cx, field, events) = open(cx, "localhost:3000/", None);
        click_field(cx);
        cx.simulate_input("zzz");
        cx.run_until_parked();
        assert_eq!(state(cx, &field).0, "zzz", "typing replaces the selected address");
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        let (text, focused, _) = state(cx, &field);
        assert_eq!((text.as_str(), focused), ("localhost:3000/", false));
        assert_eq!(events.borrow().last(), Some(&FieldEvent::Draft("localhost:3000/".into())), "the filter follows the address back");
    }

    #[gpui_kit::test]
    fn with_no_page_escape_empties_the_field(cx: &mut TestAppContext) {
        let (cx, field, events) = open(cx, "", None);
        click_field(cx);
        cx.simulate_input("zzz");
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        let (text, focused, _) = state(cx, &field);
        assert_eq!((text.as_str(), focused), ("", false));
        assert_eq!(events.borrow().last(), Some(&FieldEvent::Draft(String::new())), "so the filter clears");
    }

    #[gpui_kit::test]
    fn with_a_page_blur_shows_the_address_again(cx: &mut TestAppContext) {
        let (cx, field, _) = open(cx, "localhost:3000/", None);
        click_field(cx);
        cx.simulate_input("zzz");
        let away = cx.debug_bounds("elsewhere").unwrap();
        cx.simulate_click(away.center(), Modifiers::none());
        cx.update(|window, cx| window.blur(cx));
        cx.run_until_parked();
        assert_eq!(state(cx, &field).0, "localhost:3000/");
        assert_eq!(field.read_with(cx, |f, _| f.draft().to_owned()), "zzz", "the draft stays what was typed");
    }

    #[gpui_kit::test]
    fn the_system_browser_button_shows_on_hover_with_a_page_and_goes_on_focus(cx: &mut TestAppContext) {
        let pressed = Rc::new(std::cell::Cell::new(0));
        let count = pressed.clone();
        let (cx, field, events) = open(cx, "localhost:3000/", Some(Rc::new(move |_, _| count.set(count.get() + 1))));
        let at = cx.debug_bounds("browser-address").unwrap();
        cx.simulate_mouse_move(at.center(), None, Modifiers::none());
        cx.run_until_parked();
        let button = cx.debug_bounds("browser-outside").expect("hovered with a page, the button stands at the end");
        assert_eq!(at.right() - button.right(), px(2.), "1 px in from the group's edge");
        assert_eq!(button.size.height, px(26.));
        let icon = cx.debug_bounds("button").unwrap();
        assert_eq!((icon.size.width, icon.size.height), (px(24.), px(24.)));
        cx.simulate_click(icon.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!((pressed.get(), events.borrow().len()), (1, 0), "the button opens the page outside and the field says nothing");
        assert!(!state(cx, &field).1, "the button's press is not the field's");
        click_field(cx);
        assert!(cx.debug_bounds("browser-outside").is_none(), "a focused field has no button");
        field.update_in(cx, |f, window, cx| {
            window.blur(cx);
            f.set_outside(None);
            cx.notify();
        });
        cx.simulate_mouse_move(point(at.left() + px(4.), at.center().y), None, Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("browser-outside").is_none(), "no page, no button");
    }
}
