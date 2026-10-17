// SPDX-License-Identifier: AGPL-3.0-only
//! The select in a row, after `select.tsx`: the 176 px trigger at the row's height, and the list that opens over it
//! with the picked item laid on the trigger's own words (`alignItemWithTrigger`). Items carry a typed value and an
//! optional glyph; the picked one stands on a faint fill, the one under the pointer or the keys on the accent. The
//! list is modal, as Base UI's: a press outside it closes it and goes no further.

use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, App, Context, Entity, EventEmitter, KeyDownEvent, MouseButton, Pixels,
    SharedString, Window,
};

use super::controls::Dim;
use super::icons::Glyph;
use super::theme::ActivePalette;
use super::{size, text};

/// `SelectItem`'s `min-h-7` at 640 px and up.
const ITEM_HEIGHT: Pixels = px(28.);
/// The list's `p-1`.
const LIST_PAD: Pixels = px(4.);
/// An item's `min-w-[calc(var(--anchor-width)+1.25rem)]` while it lies over the trigger.
const ITEM_WIDER: Pixels = px(20.);
/// The trigger's `px-[9px]` against the item's `px-2` inside the list's pad and edge: how far left the list stands so
/// the two sets of words start on one line.
const TEXT_SHIFT: Pixels = px(3.);

type Mark = Rc<dyn Fn(&App) -> AnyElement>;
type Change<T> = Rc<dyn Fn(&T, &mut Window, &mut App) + 'static>;

/// One item of a select: its value, its words and an optional mark before them.
pub struct SelectItem<T> {
    value: T,
    label: SharedString,
    mark: Option<Mark>,
}

impl<T> SelectItem<T> {
    pub fn new(value: T, label: impl Into<SharedString>) -> Self {
        SelectItem { value, label: label.into(), mark: None }
    }

    /// A lucide glyph before the words, at 16 px in the muted ink.
    pub fn glyph(mut self, glyph: Glyph) -> Self {
        self.mark =
            Some(Rc::new(move |cx| Icon::new(glyph).with_size(px(16.)).text_color(cx.palette().muted_foreground).into_any_element()));
        self
    }

    /// Any mark before the words, drawn in the current theme: a project's glyph in its hue, a swatch.
    pub fn mark(mut self, mark: impl Fn(&App) -> AnyElement + 'static) -> Self {
        self.mark = Some(Rc::new(mark));
        self
    }
}

/// What a select says when a person picks.
pub enum SelectEvent<T> {
    Confirm(T),
}

/// What a select holds: its items, the picked one, and whether its list is open.
pub struct SelectState<T> {
    items: Vec<SelectItem<T>>,
    picked: Option<usize>,
    open: bool,
    cursor: Option<usize>,
}

impl<T: Clone + PartialEq + 'static> SelectState<T> {
    /// Make it with `cx.new(|_| SelectState::new(items, Some(&value)))`.
    pub fn new(items: Vec<SelectItem<T>>, value: Option<&T>) -> Self {
        let picked = value.and_then(|v| items.iter().position(|i| &i.value == v));
        SelectState { items, picked, open: false, cursor: None }
    }

    pub fn value(&self) -> Option<&T> {
        self.picked.map(|ix| &self.items[ix].value)
    }

    /// Picks `value` from outside, as when the host's answer lands; says nothing.
    pub fn set_value(&mut self, value: Option<&T>, cx: &mut Context<Self>) {
        self.picked = value.and_then(|v| self.items.iter().position(|i| &i.value == v));
        cx.notify();
    }

    /// New items, keeping the picked value where they still hold it.
    pub fn set_items(&mut self, items: Vec<SelectItem<T>>, cx: &mut Context<Self>) {
        let value = self.value().cloned();
        self.items = items;
        self.picked = value.and_then(|v| self.items.iter().position(|i| i.value == v));
        self.cursor = None;
        cx.notify();
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens or closes the list, the cursor on the picked item.
    pub fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.open = open;
        self.cursor = if open { self.picked.or((!self.items.is_empty()).then_some(0)) } else { None };
        cx.notify();
    }

    fn step(&mut self, by: isize, cx: &mut Context<Self>) {
        if self.items.is_empty() {
            return;
        }
        let last = self.items.len() as isize - 1;
        let at = self.cursor.map_or(0, |c| (c as isize + by).clamp(0, last));
        self.cursor = Some(at as usize);
        cx.notify();
    }

    /// Picks the item at `ix` as a person does: the list closes and the select says so.
    pub fn confirm(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.open = false;
        self.cursor = None;
        let Some(item) = self.items.get(ix) else { return };
        let changed = self.picked != Some(ix);
        self.picked = Some(ix);
        if changed {
            cx.emit(SelectEvent::Confirm(item.value.clone()));
        }
        cx.notify();
    }
}

impl<T: 'static> EventEmitter<SelectEvent<T>> for SelectState<T> {}

/// A select in a row at `SELECT_WIDTH`. Held, it keeps its look at 64% and neither the pointer nor the keys open it.
#[derive(IntoElement)]
pub struct Select<T: Clone + PartialEq + 'static> {
    state: Entity<SelectState<T>>,
    held: bool,
    width: Pixels,
    height: Pixels,
    placeholder: Option<SharedString>,
    on_change: Option<Change<T>>,
}

impl<T: Clone + PartialEq + 'static> Select<T> {
    pub fn new(state: &Entity<SelectState<T>>) -> Self {
        Select {
            state: state.clone(),
            held: false,
            width: size::SELECT_WIDTH,
            height: size::CONTROL_HEIGHT,
            placeholder: None,
            on_change: None,
        }
    }

    pub fn held(mut self, held: bool) -> Self {
        self.held = held;
        self
    }

    /// A width other than the row's 176 px.
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    /// A height other than the row's 30 px: Activity's bar selects stand at 32 (`BAR_SELECT`).
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = height;
        self
    }

    /// The words the trigger shows while nothing is picked, in the placeholder ink.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Called with the value a person picks, after the state holds it.
    pub fn on_change(mut self, f: impl Fn(&T, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

/// The keys an open or a focused select answers.
fn keys<T: Clone + PartialEq + 'static>(
    state: Entity<SelectState<T>>,
    on_change: Option<Change<T>>,
) -> impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static {
    move |event, window, cx| {
        let key = event.keystroke.key.as_str();
        if event.keystroke.modifiers.modified() {
            return;
        }
        let open = state.read(cx).open;
        if !open {
            if matches!(key, "enter" | "space" | "up" | "down") {
                state.update(cx, |s, cx| s.set_open(true, cx));
                cx.stop_propagation();
            }
            return;
        }
        match key {
            "up" => state.update(cx, |s, cx| s.step(-1, cx)),
            "down" => state.update(cx, |s, cx| s.step(1, cx)),
            "home" => state.update(cx, |s, cx| s.step(isize::MIN / 2, cx)),
            "end" => state.update(cx, |s, cx| s.step(isize::MAX / 2, cx)),
            "enter" | "space" => {
                let picked = state.update(cx, |s, cx| {
                    let ix = s.cursor?;
                    s.confirm(ix, cx);
                    s.value().cloned()
                });
                if let (Some(value), Some(f)) = (picked, on_change.as_ref()) {
                    f(&value, window, cx);
                }
            }
            "escape" => state.update(cx, |s, cx| s.set_open(false, cx)),
            "tab" => {
                state.update(cx, |s, cx| s.set_open(false, cx));
                return;
            }
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl<T: Clone + PartialEq + 'static> RenderOnce for Select<T> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let id = SharedString::from(format!("select-{}", self.state.entity_id()));
        let state = self.state.read(cx);
        let (open, picked, cursor) = (state.open && !self.held, state.picked, state.cursor);
        let shown = picked.map(|ix| (state.items[ix].label.clone(), state.items[ix].mark.clone()));
        let rows: Vec<(SharedString, Option<Mark>)> =
            if open { state.items.iter().map(|i| (i.label.clone(), i.mark.clone())).collect() } else { Vec::new() };
        let dim = Dim::new(p.background, p.input_fill);
        let (edge, fill, ink, placeholder) = if self.held {
            (dim.over(p.input), dim.fill(), dim.over(p.foreground), dim.over(p.placeholder))
        } else {
            (p.input, p.input_fill, p.foreground, p.placeholder)
        };
        let words = match shown {
            Some((label, mark)) => h_flex().gap_2().min_w_0().children(mark.map(|m| m(cx))).child(div().min_w_0().truncate().child(label)),
            None => h_flex().min_w_0().text_color(placeholder).child(div().truncate().child(self.placeholder.unwrap_or_default())),
        };
        let chevron = Icon::new(Glyph::ChevronDown).with_size(px(12.)).flex_none().mr(px(-4.)).text_color(if self.held {
            dim.over(p.muted_foreground.opacity(0.5))
        } else {
            p.muted_foreground.opacity(0.5)
        });
        let ring_ink = p.ring;
        let trigger = h_flex()
            .id(id.clone())
            .debug_selector(|| "select".into())
            .relative()
            .flex_none()
            .justify_between()
            .gap_2()
            .w(self.width)
            .h(self.height)
            .px(px(9.))
            .rounded(size::CONTROL_RADIUS)
            .border_1()
            .border_color(edge)
            .bg(fill)
            .text_size(text::FACT)
            .text_color(ink)
            .child(words.flex_1())
            .child(chevron)
            .when(!self.held, |el| {
                let (toggle, keyed) = (self.state.clone(), self.state.clone());
                el.cursor_pointer()
                    .tab_index(0)
                    .focus_visible(move |s| s.border_color(ring_ink))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        toggle.update(cx, |s, cx| {
                            let open = !s.open;
                            s.set_open(open, cx)
                        })
                    })
                    .on_key_down(keys(keyed, self.on_change.clone()))
            });
        let popup = open.then(|| {
            let at = picked.unwrap_or(0);
            let top = -(px(1.) + LIST_PAD + ITEM_HEIGHT * at as f32) + (self.height - ITEM_HEIGHT) / 2. - px(1.);
            let items = rows.into_iter().enumerate().map(|(ix, (label, mark))| {
                let (hover, confirm) = (self.state.clone(), self.state.clone());
                let on_change = self.on_change.clone();
                let ground = if cursor == Some(ix) {
                    p.accent
                } else if picked == Some(ix) {
                    p.foreground.opacity(0.08)
                } else {
                    gpui_kit::transparent_black()
                };
                h_flex()
                    .id(ix)
                    .debug_selector(|| "select-item".into())
                    .min_h(ITEM_HEIGHT)
                    .min_w(self.width + ITEM_WIDER)
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded(size::BUTTON_RADIUS)
                    .bg(ground)
                    .cursor_pointer()
                    .text_size(text::VALUE)
                    .line_height(px(20.))
                    .text_color(p.foreground)
                    .children(mark.map(|m| m(cx)))
                    .child(div().min_w_0().flex_1().child(label))
                    .on_hover(move |over, _, cx| {
                        if *over {
                            hover.update(cx, |s, cx| {
                                if s.cursor != Some(ix) {
                                    s.cursor = Some(ix);
                                    cx.notify();
                                }
                            })
                        }
                    })
                    // A click, not a release: the press that opened the list lands its release on the picked item, which
                    // stands over the trigger.
                    .on_click(move |_, window, cx| {
                        let value = confirm.update(cx, |s, cx| {
                            s.confirm(ix, cx);
                            s.value().cloned()
                        });
                        if let (Some(value), Some(f)) = (value, on_change.as_ref()) {
                            f(&value, window, cx);
                        }
                    })
            });
            let list = v_flex()
                .id(SharedString::from(format!("{id}-list")))
                .debug_selector(|| "select-list".into())
                .occlude()
                .max_h(window.viewport_size().height - px(16.))
                .overflow_y_scroll()
                .p(LIST_PAD)
                .rounded(px(12.))
                .border_1()
                .border_color(p.popover_edge)
                .bg(p.popover)
                .shadow(p.dropdown_shadow.clone())
                .children(items);
            let close = self.state.clone();
            let viewport = window.viewport_size();
            // A press anywhere off the list closes it and reaches nothing under it.
            let backdrop = deferred(anchored().position(point(px(0.), px(0.))).child(
                div().id("select-backdrop").w(viewport.width).h(viewport.height).occlude().on_mouse_down(
                    MouseButton::Left,
                    move |_, _, cx| {
                        close.update(cx, |s, cx| s.set_open(false, cx));
                        cx.stop_propagation();
                    },
                ),
            ))
            .with_priority(1);
            let list = deferred(anchored().offset(point(-TEXT_SHIFT, top)).snap_to_window_with_margin(px(8.)).child(list)).with_priority(2);
            div().absolute().top_0().left_0().child(backdrop).child(list)
        });
        div().debug_selector(|| "control".into()).relative().flex_none().child(trigger).children(popup)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui_kit::component::Root;
    use gpui_kit::{div, prelude::*, px, size, Context, Entity, TestAppContext, VisualTestContext, Window};

    use super::{Select, SelectEvent, SelectItem, SelectState};
    use crate::ui::{self, Side};

    struct Lone {
        state: Entity<SelectState<u8>>,
        held: bool,
    }

    impl Render for Lone {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().p(px(24.)).child(Select::new(&self.state).held(self.held))
        }
    }

    fn draw(cx: &mut TestAppContext, held: bool) -> (Entity<SelectState<u8>>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
        });
        let items = vec![SelectItem::new(1, "One"), SelectItem::new(2, "Two"), SelectItem::new(3, "Three")];
        let state = cx.new(|_| SelectState::new(items, Some(&1)));
        let view = cx.new(|_| Lone { state: state.clone(), held });
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(view, window, cx));
        cx.simulate_resize(size(px(400.), px(400.)));
        cx.run_until_parked();
        (state, cx)
    }

    #[gpui_kit::test]
    fn the_keys_open_the_list_move_and_pick(cx: &mut TestAppContext) {
        let (state, cx) = draw(cx, false);
        let said = Rc::new(RefCell::new(Vec::new()));
        let heard = said.clone();
        cx.update(|_, cx| {
            cx.subscribe(&state, move |_, event: &SelectEvent<u8>, _| {
                let SelectEvent::Confirm(v) = event;
                heard.borrow_mut().push(*v);
            })
            .detach()
        });
        // Tab with nothing focused reaches no key context in a test window; Root's Tab runs `focus_next`.
        cx.update(|window, cx| window.focus_next(cx));
        let focused = cx.update(|window, cx| window.focused(cx).is_some());
        assert!(focused, "Tab reaches the select");
        cx.simulate_keystrokes("down");
        assert!(cx.read(|cx| state.read(cx).is_open()), "Tab then Down opens the focused select");
        cx.simulate_keystrokes("down down enter");
        assert!(!cx.read(|cx| state.read(cx).is_open()));
        assert_eq!(cx.read(|cx| state.read(cx).value().copied()), Some(3));
        assert_eq!(*said.borrow(), vec![3]);
        cx.simulate_keystrokes("space escape");
        assert!(!cx.read(|cx| state.read(cx).is_open()), "Escape closes it again");
    }

    #[gpui_kit::test]
    fn a_held_select_takes_neither_the_keys_nor_focus(cx: &mut TestAppContext) {
        let (state, cx) = draw(cx, true);
        cx.update(|window, cx| window.focus_next(cx));
        assert!(cx.update(|window, cx| window.focused(cx).is_none()), "a held select is no tab stop");
        cx.simulate_keystrokes("down enter space");
        assert!(!cx.read(|cx| state.read(cx).is_open()));
        assert_eq!(cx.read(|cx| state.read(cx).value().copied()), Some(1));
    }
}
