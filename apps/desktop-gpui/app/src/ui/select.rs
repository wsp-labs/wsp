// SPDX-License-Identifier: AGPL-3.0-only
//! The select in a row, after `select.tsx`: the 176 px trigger at the row's height, and the list that opens over it
//! with the picked item laid on the trigger's own words (`alignItemWithTrigger`). Items carry a typed value and an
//! optional glyph; the picked one stands on a faint fill, the one under the pointer or the keys on the accent. The
//! list is modal, as Base UI's: a press outside it closes it and goes no further. Typing finds an item by its first
//! letters, open or closed, and the list keeps the item under the keys in view.

use std::rc::Rc;
use std::time::Duration;

use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::{
    anchored, canvas, deferred, div, point, prelude::*, px, AnyElement, App, Context, Entity, EventEmitter, KeyDownEvent, Keystroke,
    MouseButton, Pixels, ScrollHandle, SharedString, Task, Window,
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
/// `SelectSeparator`'s `my-1 h-px`.
const RULE: Pixels = px(9.);
/// The trigger's `px-[9px]` against the item's `px-2` inside the list's pad and edge: how far left the list stands so
/// the two sets of words start on one line.
const TEXT_SHIFT: Pixels = px(3.);
/// `useTypeahead`'s `resetMs`, which Base UI's Select leaves at its default: the typed letters are forgotten this long
/// after the last one.
const TYPED_RESET: Duration = Duration::from_millis(750);

type Mark = Rc<dyn Fn(&App) -> AnyElement>;
type Change<T> = Rc<dyn Fn(&T, &mut Window, &mut App) + 'static>;
/// An item as the open list draws it: its index, words, mark, rule above and face.
type ListRow = (usize, SharedString, Option<Mark>, bool, Option<SharedString>);

/// One item of a select: its value, its words and an optional mark before them.
pub struct SelectItem<T> {
    value: T,
    label: SharedString,
    /// The trigger's words once this item is picked, where the list's are too long for its 176 px.
    short: Option<SharedString>,
    mark: Option<Mark>,
    /// The family its words are drawn in in the list, as a font picker draws each family in itself.
    face: Option<SharedString>,
    /// Shown on the trigger while picked, never in the list: a value the list does not offer.
    hidden: bool,
    /// `SelectSeparator` before it in the list.
    rule_above: bool,
    /// The value Base UI reads as none picked, `""`: picked, its words stand on the trigger in the placeholder ink.
    blank: bool,
}

impl<T> SelectItem<T> {
    pub fn new(value: T, label: impl Into<SharedString>) -> Self {
        SelectItem { value, label: label.into(), short: None, mark: None, face: None, hidden: false, rule_above: false, blank: false }
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    /// Its words in the list drawn in `family`; the trigger keeps the select's own face.
    pub fn face(mut self, family: impl Into<SharedString>) -> Self {
        self.face = Some(family.into());
        self
    }

    /// The trigger says it while it is picked, and the list leaves it out (`SelectValue` over a value no item holds).
    pub fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }

    /// Picked, the trigger says it in the placeholder ink, as Base UI marks an empty value `data-placeholder`.
    pub fn blank(mut self) -> Self {
        self.blank = true;
        self
    }

    /// A hairline before it in the list, as `SelectSeparator` sets an act apart from the picks.
    pub fn rule_above(mut self) -> Self {
        self.rule_above = true;
        self
    }

    /// Words the trigger shows in place of the label once this item is picked.
    pub fn short(mut self, words: impl Into<SharedString>) -> Self {
        self.short = Some(words.into());
        self
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

/// The one select whose list stands open, so a screen's Escape closes it before anything of the screen's own: the
/// screen's Escape is bound above the list's key listener and reaches the screen first.
#[derive(Default, Clone)]
struct OpenList(Option<(gpui_kit::EntityId, CloseList)>);

/// Closes one select's list, saying whether it stood open.
type CloseList = Rc<dyn Fn(&mut App) -> bool>;

impl gpui_kit::Global for OpenList {}

/// Closes the list that stands open, if one does, and says whether it did.
pub fn close_open_list(cx: &mut App) -> bool {
    let Some((_, close)) = cx.try_global::<OpenList>().and_then(|open| open.0.clone()) else { return false };
    cx.set_global(OpenList(None));
    close(cx)
}

/// What a select holds: its items, the picked one, and whether its list is open.
pub struct SelectState<T> {
    items: Vec<SelectItem<T>>,
    picked: Option<usize>,
    open: bool,
    cursor: Option<usize>,
    scroll: ScrollHandle,
    typed: Typed,
    /// The trigger's width as last laid out, for a select that fills what holds it.
    anchor: Pixels,
}

/// The letters typed so far toward an item, after `useTypeahead`.
#[derive(Default)]
struct Typed {
    letters: String,
    /// Where a search starts after: the item the session began on, or the last found once its first letter comes again.
    from: Option<usize>,
    found: Option<usize>,
    _reset: Option<Task<()>>,
}

impl<T: Clone + PartialEq + 'static> SelectState<T> {
    /// Make it with `cx.new(|_| SelectState::new(items, Some(&value)))`.
    pub fn new(items: Vec<SelectItem<T>>, value: Option<&T>) -> Self {
        let picked = value.and_then(|v| items.iter().position(|i| &i.value == v));
        SelectState { items, picked, open: false, cursor: None, scroll: ScrollHandle::new(), typed: Typed::default(), anchor: px(0.) }
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
        let listed = |ix: &usize| !self.items[*ix].hidden;
        self.cursor = if open { self.picked.filter(listed).or_else(|| (0..self.items.len()).find(listed)) } else { None };
        self.typed = Typed::default();
        if let Some(at) = self.cursor {
            self.scroll.set_offset(point(px(0.), px(0.)));
            self.scroll.scroll_to_item(at);
        }
        self.mark_open(cx);
        cx.notify();
    }

    /// Keeps [`OpenList`] naming this select while its list is open, and no longer once it closes.
    fn mark_open(&self, cx: &mut Context<Self>) {
        let id = cx.entity_id();
        if self.open {
            let weak = cx.entity().downgrade();
            let close = move |cx: &mut App| {
                weak.update(cx, |s, cx| {
                    let was = s.open;
                    s.set_open(false, cx);
                    was
                })
                .unwrap_or(false)
            };
            cx.set_global(OpenList(Some((id, Rc::new(close)))));
        } else if cx.try_global::<OpenList>().and_then(|open| open.0.as_ref().map(|(at, _)| *at)) == Some(id) {
            cx.set_global(OpenList(None));
        }
    }

    /// Lays the cursor on `ix` and scrolls the list to it.
    fn point_at(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.cursor = Some(ix);
        self.scroll.scroll_to_item(ix);
        cx.notify();
    }

    /// Whether letters are being typed toward an item, so a Space is one more of them and not a pick.
    fn typing(&self) -> bool {
        !self.typed.letters.is_empty()
    }

    /// One letter typed, as `useTypeahead` takes it: the first item from the one after where the session stands whose
    /// words start with the letters so far, case aside. The same first letter again steps to the next item it starts.
    fn type_letter(&mut self, letter: char, cx: &mut Context<Self>) -> Option<usize> {
        let labels: Vec<String> = self.items.iter().map(|i| i.label.to_lowercase()).collect();
        if self.typed.letters.is_empty() {
            self.typed.from = self.picked.or(self.cursor);
        }
        let doubled = |l: &String| {
            let mut c = l.chars();
            matches!((c.next(), c.next()), (Some(a), Some(b)) if a == b)
        };
        let letter = letter.to_lowercase().collect::<String>();
        if !labels.iter().any(doubled) && self.typed.letters == letter {
            self.typed.letters.clear();
            self.typed.from = self.typed.found;
        }
        self.typed.letters.push_str(&letter);
        self.typed._reset = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TYPED_RESET).await;
            let _ = this.update(cx, |s, _| s.typed.letters.clear());
        }));
        let start = self.typed.from.map_or(0, |f| f + 1);
        let n = labels.len();
        let found = (0..n).map(|k| (start + k) % n).find(|&ix| !self.items[ix].hidden && labels[ix].starts_with(&self.typed.letters));
        match found {
            Some(ix) => self.typed.found = Some(ix),
            None if letter != " " => self.typed.letters.clear(),
            None => {}
        }
        found
    }

    fn step(&mut self, by: isize, cx: &mut Context<Self>) {
        let listed: Vec<usize> = (0..self.items.len()).filter(|ix| !self.items[*ix].hidden).collect();
        if listed.is_empty() {
            return;
        }
        let last = listed.len() as isize - 1;
        let at = self.cursor.and_then(|c| listed.iter().position(|ix| *ix == c)).map_or(0, |c| (c as isize + by).clamp(0, last));
        self.point_at(listed[at as usize], cx);
    }

    /// Picks the item at `ix` as a person does: the list closes and the select says so.
    pub fn confirm(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.open = false;
        self.cursor = None;
        self.mark_open(cx);
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

/// A select in a row at `SELECT_WIDTH`, 144 px under 640. Held, it keeps its look at 64% and neither the pointer nor the
/// keys open it.
#[derive(IntoElement)]
pub struct Select<T: Clone + PartialEq + 'static> {
    state: Entity<SelectState<T>>,
    held: bool,
    width: Option<Pixels>,
    fill: bool,
    height: Pixels,
    placeholder: Option<SharedString>,
    on_change: Option<Change<T>>,
}

impl<T: Clone + PartialEq + 'static> Select<T> {
    pub fn new(state: &Entity<SelectState<T>>) -> Self {
        Select {
            state: state.clone(),
            held: false,
            width: None,
            fill: false,
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
        self.width = Some(width);
        self
    }

    /// The whole width of what holds it, as `w-full`.
    pub fn fill(mut self, fill: bool) -> Self {
        self.fill = fill;
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

/// The one character a key types, Space among them, where it types one.
fn letter_of(stroke: &Keystroke) -> Option<char> {
    if stroke.key == "space" {
        return Some(' ');
    }
    let mut typed = stroke.key_char.as_deref().unwrap_or(&stroke.key).chars();
    match (typed.next(), typed.next()) {
        (Some(c), None) if !c.is_control() => Some(c),
        _ => None,
    }
}

/// The keys an open or a focused select answers.
fn keys<T: Clone + PartialEq + 'static>(
    state: Entity<SelectState<T>>,
    on_change: Option<Change<T>>,
) -> impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static {
    move |event, window, cx| {
        let stroke = &event.keystroke;
        let key = stroke.key.as_str();
        let m = stroke.modifiers;
        if m.control || m.alt || m.platform || m.function {
            return;
        }
        let pick = |ix: Option<usize>, window: &mut Window, cx: &mut App| {
            let picked = state.update(cx, |s, cx| {
                s.confirm(ix?, cx);
                s.value().cloned()
            });
            if let (Some(value), Some(f)) = (picked, on_change.as_ref()) {
                f(&value, window, cx);
            }
        };
        let open = state.read(cx).open;
        if let Some(letter) = letter_of(stroke).filter(|l| *l != ' ' || state.read(cx).typing()) {
            // Closed, a typed match is picked outright, as Base UI's Select does.
            match state.update(cx, |s, cx| s.type_letter(letter, cx)) {
                Some(ix) if open => state.update(cx, |s, cx| s.point_at(ix, cx)),
                Some(ix) => pick(Some(ix), window, cx),
                None => {}
            }
            cx.stop_propagation();
            return;
        }
        if m.shift {
            return;
        }
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
                let ix = state.read(cx).cursor;
                pick(ix, window, cx);
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
        let filling = self.fill;
        let state = self.state.read(cx);
        let width = match (filling, self.width) {
            (true, _) => state.anchor,
            (false, Some(width)) => width,
            (false, None) if super::narrow::width(window).sm => size::SELECT_WIDTH_NARROW,
            (false, None) => size::SELECT_WIDTH,
        };
        let (open, picked, cursor, scroll) = (state.open && !self.held, state.picked, state.cursor, state.scroll.clone());
        let shown = picked.map(|ix| {
            let item = &state.items[ix];
            (item.short.clone().unwrap_or_else(|| item.label.clone()), item.mark.clone(), item.blank)
        });
        let rows: Vec<ListRow> = if open {
            state
                .items
                .iter()
                .enumerate()
                .filter(|(_, i)| !i.hidden)
                .map(|(ix, i)| (ix, i.label.clone(), i.mark.clone(), i.rule_above, i.face.clone()))
                .collect()
        } else {
            Vec::new()
        };
        let dim = Dim::new(p.background, p.input_fill);
        let (edge, fill, ink, placeholder) = if self.held {
            (dim.over(p.input), dim.fill(), dim.over(p.foreground), dim.over(p.placeholder))
        } else {
            (p.input, p.input_fill, p.foreground, p.placeholder)
        };
        let words = match shown {
            Some((label, mark, blank)) => h_flex()
                .gap_2()
                .min_w_0()
                .when(blank, |el| el.debug_selector(|| "select-placeholder".into()).text_color(placeholder))
                .children(mark.map(|m| m(cx)))
                .child(div().min_w_0().truncate().child(label)),
            None => h_flex()
                .debug_selector(|| "select-placeholder".into())
                .min_w_0()
                .text_color(placeholder)
                .child(div().truncate().child(self.placeholder.unwrap_or_default())),
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
            .when(filling, |el| el.w_full())
            .when(!filling, |el| el.flex_none().w(width))
            .justify_between()
            .gap_2()
            .h(self.height)
            .px(px(9.))
            .rounded(size::CONTROL_RADIUS)
            .border_1()
            .border_color(edge)
            .bg(fill)
            .text_size(text::FACT.0)
            .line_height(text::FACT.1)
            .text_color(ink)
            .child(words.flex_1())
            .child(chevron)
            .when(filling, |el| {
                let anchor = self.state.clone();
                el.child(
                    canvas(
                        move |bounds, _, cx| {
                            anchor.update(cx, |s, _| s.anchor = bounds.size.width);
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                )
            })
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
            let at = picked.and_then(|picked| rows.iter().position(|(ix, ..)| *ix == picked)).unwrap_or(0);
            let ruled = rows.iter().take(at + 1).filter(|(_, _, _, rule, _)| *rule).count();
            let top = -(px(1.) + LIST_PAD + ITEM_HEIGHT * at as f32 + RULE * ruled as f32) + (self.height - ITEM_HEIGHT) / 2. - px(1.);
            let items = rows.into_iter().map(|(ix, label, mark, rule, face)| {
                let (hover, confirm) = (self.state.clone(), self.state.clone());
                let on_change = self.on_change.clone();
                let ground = if cursor == Some(ix) {
                    p.accent
                } else if picked == Some(ix) {
                    p.foreground.opacity(0.08)
                } else {
                    gpui_kit::transparent_black()
                };
                let item = h_flex()
                    .id(ix)
                    .debug_selector(|| "select-item".into())
                    .min_h(ITEM_HEIGHT)
                    .min_w(width + ITEM_WIDER)
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded(size::BUTTON_RADIUS)
                    .bg(ground)
                    .cursor_pointer()
                    .text_size(text::VALUE.0)
                    .line_height(px(20.))
                    .text_color(p.foreground)
                    .children(mark.map(|m| m(cx)))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .when_some(face, |el, face| {
                                let named = face.clone();
                                el.debug_selector(move || format!("select-face:{named}")).font_family(face)
                            })
                            .child(label),
                    )
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
                    });
                v_flex()
                    .when(rule, |el| el.child(div().debug_selector(|| "select-rule".into()).mx_2().my_1().h(px(1.)).bg(p.border)))
                    .child(item)
            });
            let list = v_flex()
                .id(SharedString::from(format!("{id}-list")))
                .debug_selector(|| "select-list".into())
                .occlude()
                .max_h(window.viewport_size().height - px(16.))
                .overflow_y_scroll()
                .track_scroll(&scroll)
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
        div()
            .debug_selector(|| "control".into())
            .relative()
            .when(filling, |el| el.w_full())
            .when(!filling, |el| el.flex_none())
            .child(trigger)
            .children(popup)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Duration;

    use gpui_kit::component::Root;
    use gpui_kit::{div, prelude::*, px, size, Context, Entity, TestAppContext, VisualTestContext, Window};

    use super::{Select, SelectEvent, SelectItem, SelectState, ITEM_HEIGHT};
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
        draw_items(cx, held, &["One", "Two", "Three"])
    }

    /// A select of `labels`, valued 1 up, the first picked.
    fn draw_items<'a>(cx: &'a mut TestAppContext, held: bool, labels: &[&str]) -> (Entity<SelectState<u8>>, &'a mut VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
        });
        let items = labels.iter().enumerate().map(|(ix, l)| SelectItem::new(ix as u8 + 1, l.to_string())).collect();
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
    fn a_hidden_item_stands_on_the_trigger_and_never_in_the_list(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
        });
        let items = vec![SelectItem::new(1, "One"), SelectItem::new(2, "Two").rule_above(), SelectItem::new(3, "Three").hidden()];
        let state = cx.new(|_| SelectState::new(items, Some(&3)));
        let view = cx.new(|_| Lone { state: state.clone(), held: false });
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(view, window, cx));
        cx.simulate_resize(size(px(400.), px(400.)));
        cx.run_until_parked();
        assert_eq!(cx.read(|cx| state.read(cx).value().copied()), Some(3), "the trigger holds the hidden value");
        cx.update(|window, cx| window.focus_next(cx));
        cx.simulate_keystrokes("down");
        assert!(cx.debug_bounds("select-rule").is_some(), "the separator stands before Two");
        let list = cx.debug_bounds("select-list").expect("the list is open").size.height;
        assert!(list < ITEM_HEIGHT * 3., "two items and the separator, not Three: {list:?}");
        cx.simulate_keystrokes("down down down enter");
        assert_eq!(cx.read(|cx| state.read(cx).value().copied()), Some(2), "the keys stop at the last item the list shows");
    }

    #[gpui_kit::test]
    fn an_item_with_a_face_draws_its_words_in_it_in_the_list_alone(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
        });
        let items = vec![SelectItem::new(1, "Default"), SelectItem::new(2, "Liberation Mono").face("Liberation Mono")];
        let state = cx.new(|_| SelectState::new(items, Some(&2)));
        let view = cx.new(|_| Lone { state: state.clone(), held: false });
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(view, window, cx));
        cx.simulate_resize(size(px(400.), px(400.)));
        cx.run_until_parked();
        assert!(cx.debug_bounds("select-face:Liberation Mono").is_none(), "the trigger keeps the select's face");
        cx.update(|window, cx| window.focus_next(cx));
        cx.simulate_keystrokes("down");
        assert!(cx.debug_bounds("select-face:Liberation Mono").is_some(), "the family's item is drawn in the family");
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

    fn cursor(cx: &mut VisualTestContext, state: &Entity<SelectState<u8>>) -> Option<usize> {
        cx.read(|cx| state.read(cx).cursor)
    }

    #[gpui_kit::test]
    fn typed_letters_find_an_item_until_they_are_forgotten(cx: &mut TestAppContext) {
        let (state, cx) = draw_items(cx, false, &["Apple", "Banana", "Blueberry", "Cherry"]);
        cx.update(|window, cx| window.focus_next(cx));
        cx.simulate_keystrokes("down");
        assert_eq!(cursor(cx, &state), Some(0));
        cx.simulate_keystrokes("b");
        assert_eq!(cursor(cx, &state), Some(1), "b finds Banana");
        cx.simulate_keystrokes("l");
        assert_eq!(cursor(cx, &state), Some(2), "b then l finds Blueberry");
        cx.executor().advance_clock(super::TYPED_RESET + Duration::from_millis(10));
        cx.simulate_keystrokes("b");
        assert_eq!(cursor(cx, &state), Some(1), "after the pause b starts afresh from the picked item and finds Banana");
        cx.simulate_keystrokes("x c");
        assert_eq!(cursor(cx, &state), Some(3), "a letter that finds nothing is dropped, so c finds Cherry");
        cx.executor().advance_clock(super::TYPED_RESET + Duration::from_millis(10));
        cx.simulate_keystrokes("b b");
        assert_eq!(cursor(cx, &state), Some(2), "the same first letter again steps from Banana to Blueberry");
        assert!(cx.read(|cx| state.read(cx).is_open()), "typing does not pick");
        cx.simulate_keystrokes("space");
        assert!(cx.read(|cx| state.read(cx).is_open()), "a Space while typing is one more letter");
        cx.executor().advance_clock(super::TYPED_RESET + Duration::from_millis(10));
        cx.simulate_keystrokes("space");
        assert_eq!(cx.read(|cx| state.read(cx).value().copied()), Some(3), "after the pause Space picks");

        cx.executor().advance_clock(super::TYPED_RESET + Duration::from_millis(10));
        cx.simulate_keystrokes("c");
        assert!(!cx.read(|cx| state.read(cx).is_open()), "a closed select stays closed");
        assert_eq!(cx.read(|cx| state.read(cx).value().copied()), Some(4), "closed, a typed letter picks Cherry outright");
    }

    #[gpui_kit::test]
    fn the_list_scrolls_to_the_item_under_the_keys(cx: &mut TestAppContext) {
        let labels: Vec<String> = (1..=40).map(|n| format!("Item {n:02}")).collect();
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        let (state, cx) = draw_items(cx, false, &labels);
        cx.update(|window, cx| window.focus_next(cx));
        cx.simulate_keystrokes("down");
        let scroll = cx.read(|cx| state.read(cx).scroll.clone());
        let shown = |cx: &mut VisualTestContext, ix: usize| {
            cx.run_until_parked();
            let list = scroll.bounds();
            let item = scroll.bounds_for_item(ix).expect("the item is laid out");
            let y = item.origin.y + scroll.offset().y;
            y >= list.top() && y + item.size.height <= list.bottom()
        };
        assert!(!shown(cx, 39), "the last item starts out of view");
        cx.simulate_keystrokes("end");
        assert!(shown(cx, 39), "End brings the last item into view");
        cx.simulate_keystrokes("home");
        assert!(shown(cx, 0), "Home brings the first back");
        cx.simulate_keystrokes("i t e m space 3 5");
        assert_eq!(cursor(cx, &state), Some(34));
        assert!(shown(cx, 34), "a typed match is scrolled to as well");
    }
}
