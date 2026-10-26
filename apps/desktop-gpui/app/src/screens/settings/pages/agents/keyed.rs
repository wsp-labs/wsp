// SPDX-License-Identifier: AGPL-3.0-only
//! A select's and a field's state kept across frames by name, so a page drawn afresh every frame keeps what was
//! picked and typed, and its pick or its Enter reaches the act the frame last gave it.

use std::rc::Rc;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{prelude::*, App, Entity, SharedString, Subscription, Window};

use crate::ui::{SelectItem, SelectState};

type OnEnter = Rc<dyn Fn(&Entity<InputState>, &mut Window, &mut App)>;

pub struct Pick<T> {
    state: Entity<SelectState<T>>,
    /// The values and words the items were last made of, so a frame that brings the same ones leaves the state be.
    seen: Vec<(T, SharedString)>,
    _drawn: Subscription,
}

/// A select's state kept by name, its items and its value laid on as the page now has them.
pub fn pick<T: Clone + PartialEq + 'static>(
    key: impl Into<SharedString>,
    items: Vec<(T, SharedString, Option<Mark>)>,
    value: Option<T>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<SelectState<T>> {
    let seen: Vec<(T, SharedString)> = items.iter().map(|(v, l, _)| (v.clone(), l.clone())).collect();
    let made = |items: Vec<(T, SharedString, Option<Mark>)>| {
        items
            .into_iter()
            .map(|(v, l, mark)| match mark {
                Some(mark) => SelectItem::new(v, l).mark(move |cx| mark(cx)),
                None => SelectItem::new(v, l),
            })
            .collect::<Vec<_>>()
    };
    let first = (made(items.clone()), value.clone(), seen.clone());
    let holder = window.use_keyed_state(key.into(), cx, move |_, cx| {
        let (items, value, seen) = first;
        let state = cx.new(|_| SelectState::new(items, value.as_ref()));
        let drawn = cx.observe(&state, |_, _, cx| cx.notify());
        Pick { state, seen, _drawn: drawn }
    });
    holder.update(cx, |h, cx| {
        if h.seen != seen {
            h.seen = seen;
            h.state.update(cx, |s, cx| s.set_items(made(items), cx));
        }
        if h.state.read(cx).value() != value.as_ref() {
            h.state.update(cx, |s, cx| s.set_value(value.as_ref(), cx));
        }
        h.state.clone()
    })
}

/// A mark drawn before an item's words, in the theme it is drawn in.
pub type Mark = Rc<dyn Fn(&App) -> gpui_kit::AnyElement>;

pub struct Typed {
    state: Entity<InputState>,
    on_enter: Option<OnEnter>,
    _typed: Subscription,
}

/// A field kept by name, holding `initial` when first drawn, its words read back each frame; Enter hands off to `on_enter`.
pub fn typed(
    key: impl Into<SharedString>,
    placeholder: &str,
    initial: &str,
    masked: bool,
    on_enter: impl Fn(&Entity<InputState>, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Entity<InputState> {
    let (placeholder, initial) = (placeholder.to_owned(), initial.to_owned());
    let holder = window.use_keyed_state(key.into(), cx, move |window, cx| {
        let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(initial).masked(masked));
        let typed = cx.subscribe_in(&state, window, |this: &mut Typed, _, event: &InputEvent, window, cx| match event {
            InputEvent::PressEnter { .. } => {
                if let Some(on_enter) = this.on_enter.clone() {
                    let state = this.state.clone();
                    on_enter(&state, window, cx);
                }
            }
            InputEvent::Change => cx.notify(),
            _ => {}
        });
        Typed { state, on_enter: None, _typed: typed }
    });
    holder.update(cx, |h, _| {
        h.on_enter = Some(Rc::new(on_enter));
        h.state.clone()
    })
}

/// A field's words as they stand.
pub fn words(field: &Entity<InputState>, cx: &App) -> String {
    field.read(cx).value().to_string()
}

/// Empties a field, as a press that took its words does.
pub fn clear(field: &Entity<InputState>, window: &mut Window, cx: &mut App) {
    field.update(cx, |f, cx| f.set_value("", window, cx));
}

/// Puts the caret in a field the first frame it is drawn, as `autoFocus` does.
pub fn focus_once(key: impl Into<SharedString>, field: &Entity<InputState>, window: &mut Window, cx: &mut App) {
    let focused = window.use_keyed_state(key.into(), cx, |_, _| false);
    if !*focused.read(cx) {
        focused.update(cx, |f, _| *f = true);
        field.update(cx, |f, cx| f.focus(window, cx));
    }
}
