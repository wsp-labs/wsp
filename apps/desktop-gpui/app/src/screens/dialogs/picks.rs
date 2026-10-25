// SPDX-License-Identifier: AGPL-3.0-only
//! The pick lists of the add flows (`settings/add/PickList.tsx`, `PickRow.tsx`): a filter, the order the rows stand
//! in, All, None and Invert over the rows the filter leaves, then the rows in one card. A shift-press ticks or clears
//! every row from the one pressed before to this one, as this one went; a locked row says why once pressed.

use std::collections::HashSet;
use std::rc::Rc;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, px, AnyElement, App, ClickEvent, Context, Entity, SharedString, Subscription, Window};

use crate::ui::card::CardItem;
use crate::ui::{size, text, ActivePalette, Button, Card, Field, Select, SelectItem, SelectState};

/// The words on a row's hover: what a shift-press does.
pub const RANGE_HOVER: &str = "Shift-click to tick or clear every row from the last one you clicked.";
/// The order a list's rows come in where it is neither by name nor by use: the catalog's for agents, the projects'.
pub const AS_FOUND: &str = "As found";
/// The CLIs' own order, the ones the agents ran most first.
pub const MOST_USED: &str = "Most used";

/// The order a list stands in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// The order the rows come in: As found, Most used.
    Own,
    Name,
    /// The ticked rows first, as they stood when it was picked.
    Ticked,
}

/// One row as the list orders, filters and ticks it: its key in the picks, the name it is filtered and sorted by,
/// whether the picks hold it, and whether the host fixed its tick.
#[derive(Clone, Debug)]
pub struct PickItem {
    key: SharedString,
    name: SharedString,
    on: bool,
    locked: bool,
}

impl PickItem {
    pub fn new(key: impl Into<SharedString>, name: impl Into<SharedString>, on: bool) -> Self {
        PickItem { key: key.into(), name: name.into(), on, locked: false }
    }

    /// A tick the host fixed: a press never changes it, and the row says why.
    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }

    pub fn key(&self) -> &SharedString {
        &self.key
    }

    pub fn name(&self) -> &SharedString {
        &self.name
    }

    pub fn is_on(&self) -> bool {
        self.on
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }
}

/// Each row's new tick, by key.
pub type Changes = Vec<(SharedString, bool)>;

type OnSet = Rc<dyn Fn(Changes, &mut Window, &mut App)>;
type RowOf = Box<dyn Fn(&PickItem, Tick) -> Vec<CardItem>>;

/// What a pick list keeps between draws: the filter, the order, the rows as they stood when Ticked first was picked,
/// the row pressed last, and the locked rows that were pressed. The view that draws the list holds it.
pub struct PickListState {
    filter: Entity<InputState>,
    order: Entity<SelectState<Order>>,
    held: Vec<SharedString>,
    anchor: Option<SharedString>,
    told: HashSet<SharedString>,
    _filtering: Subscription,
}

impl PickListState {
    /// `own` names the order the rows come in where it is not by name (`AS_FOUND`, `MOST_USED`); a list whose rows
    /// arrive by name leaves it out and opens on Name.
    pub fn new(own: Option<&'static str>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter"));
        let mut items = Vec::new();
        if let Some(own) = own {
            items.push(SelectItem::new(Order::Own, own));
        }
        items.push(SelectItem::new(Order::Name, "Name"));
        items.push(SelectItem::new(Order::Ticked, "Ticked first"));
        let first = if own.is_some() { Order::Own } else { Order::Name };
        let order = cx.new(|_| SelectState::new(items, Some(&first)));
        let filtering = cx.subscribe(&filter, |_, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                cx.notify();
            }
        });
        PickListState { filter, order, held: Vec::new(), anchor: None, told: HashSet::new(), _filtering: filtering }
    }

    /// The words typed in the filter.
    pub fn filter(&self, cx: &App) -> String {
        self.filter.read(cx).value().to_string()
    }

    /// Types `words` into the filter, as a person would.
    pub fn set_filter(&mut self, words: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.filter.update(cx, |f, cx| f.set_value(words.to_owned(), window, cx));
        cx.notify();
    }

    pub fn order(&self, cx: &App) -> Order {
        self.order.read(cx).value().copied().unwrap_or(Order::Name)
    }

    /// Picks the order, as the select does: Ticked first takes the rows as they stand now.
    pub fn set_order(&mut self, order: Order, items: &[PickItem], cx: &mut Context<Self>) {
        let was = self.order(cx);
        self.held = if order == Order::Ticked {
            let mut now = ordered(items, was, &self.held);
            now.sort_by_key(|i| !i.on);
            now.into_iter().map(|i| i.key).collect()
        } else {
            Vec::new()
        };
        self.order.update(cx, |s, cx| s.set_value(Some(&order), cx));
        cx.notify();
    }

    /// The rows the filter leaves, in the order picked.
    pub fn shown(&self, items: &[PickItem], cx: &App) -> Vec<PickItem> {
        let words = self.filter(cx).trim().to_lowercase();
        let left: Vec<PickItem> = items
            .iter()
            .filter(|i| words.is_empty() || i.name.to_lowercase().contains(&words) || i.key.to_lowercase().contains(&words))
            .cloned()
            .collect();
        ordered(&left, self.order(cx), &self.held)
    }

    /// The changes a press on `key` makes, turning it to `next`: with shift, every row shown from the one pressed last
    /// to this one; a locked row in the range keeps its tick.
    pub fn press(&mut self, key: &SharedString, next: bool, shift: bool, items: &[PickItem], cx: &App) -> Changes {
        let shown = self.shown(items, cx);
        let to = shown.iter().position(|i| &i.key == key);
        let from = if shift { self.anchor.as_ref().and_then(|a| shown.iter().position(|i| &i.key == a)) } else { None };
        self.anchor = Some(key.clone());
        let range: Vec<&PickItem> = match (from, to) {
            (Some(a), Some(b)) => shown[a.min(b)..=a.max(b)].iter().collect(),
            _ => shown.iter().filter(|i| &i.key == key).collect(),
        };
        range.into_iter().filter(|i| !i.locked).map(|i| (i.key.clone(), next)).collect()
    }

    /// Whether the locked row `key` was pressed, so it says why in its note's place.
    pub fn told(&self, key: &str) -> bool {
        self.told.contains(key)
    }
}

/// The rows in the order picked. Ticked first stands as the rows were ticked when it was picked, so a tick moves no
/// row and a shift-press ranges over the rows as shown.
fn ordered(items: &[PickItem], order: Order, held: &[SharedString]) -> Vec<PickItem> {
    let mut out = items.to_vec();
    match order {
        Order::Own => {}
        Order::Name => out.sort_by_key(|i| i.name.to_lowercase()),
        Order::Ticked => out.sort_by_key(|i| held.iter().position(|k| k == &i.key).unwrap_or(held.len())),
    }
    out
}

/// What a row hands its press to: the list's tick, which reaches the row's range on a shift-press, or on a locked row
/// says why.
#[derive(Clone)]
pub struct Tick {
    key: SharedString,
    on: bool,
    locked: bool,
    told: bool,
    state: Entity<PickListState>,
    items: Rc<Vec<PickItem>>,
    on_set: OnSet,
}

impl Tick {
    /// Whether the row is locked and was pressed: its note's place says why.
    pub fn told(&self) -> bool {
        self.told
    }

    /// The press for the row, `ui::PickRow::on_click(tick.press())`: the tick turns, or on a locked row the why shows.
    pub fn press(&self) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        let tick = self.clone();
        move |e, window, cx| {
            let key = tick.key.clone();
            if tick.locked {
                tick.state.update(cx, |s, cx| {
                    s.told.insert(key);
                    cx.notify();
                });
                return;
            }
            let shift = e.modifiers().shift;
            let changes = tick.state.update(cx, |s, cx| s.press(&key, !tick.on, shift, &tick.items, cx));
            (tick.on_set)(changes, window, cx);
        }
    }
}

/// One pick list (`PickList`): the tools over the rows, then the rows in one card at the dialog's inset. `row` draws
/// one row (a `ui::PickRow` or `ui::Choice` turned into a card item) with the [`Tick`] its press goes through; rows
/// under a ticked row (a [`pick_line`]) go in the same `Vec`.
#[derive(IntoElement)]
pub struct PickList {
    id: SharedString,
    state: Entity<PickListState>,
    items: Vec<PickItem>,
    row: RowOf,
    on_set: OnSet,
}

impl PickList {
    /// `on_set` takes each row's new tick by key; a locked row is never in it.
    pub fn new(
        id: impl Into<SharedString>,
        state: &Entity<PickListState>,
        items: Vec<PickItem>,
        on_set: impl Fn(Changes, &mut Window, &mut App) + 'static,
        row: impl Fn(&PickItem, Tick) -> Vec<CardItem> + 'static,
    ) -> Self {
        PickList { id: id.into(), state: state.clone(), items, row: Box::new(row), on_set: Rc::new(on_set) }
    }
}

/// One line under a ticked row (`PickLine`): its label on the name's edge and its control at the right, 48 px tall.
pub fn pick_line(id: impl Into<SharedString>, label: impl Into<SharedString>, control: impl IntoElement) -> CardItem {
    let (id, label, control) = (id.into(), label.into(), control.into_any_element());
    CardItem::placed(move |corners| {
        corners
            .apply(h_flex().id(id).debug_selector(|| "pick-line".into()))
            .min_h(size::LINE_FLOOR)
            .justify_between()
            .gap_5()
            .py_2()
            .pl(corners.inset + px(72.))
            .pr(corners.inset)
            .child(div().text_size(text::VALUE.0).line_height(text::VALUE.1).child(label))
            .child(h_flex().flex_none().child(control))
            .into_any_element()
    })
}

impl RenderOnce for PickList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let state = self.state.read(cx);
        let shown = state.shown(&self.items, cx);
        let filter_words = state.filter(cx).trim().to_owned();
        let (filter, order) = (state.filter.clone(), state.order.clone());
        let items = Rc::new(self.items);
        let tools = {
            let set = |pick: fn(&PickItem) -> Option<bool>| {
                let (shown, on_set) = (shown.clone(), self.on_set.clone());
                move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                    let changes: Changes =
                        shown.iter().filter(|i| !i.locked).filter_map(|i| pick(i).map(|on| (i.key.clone(), on))).collect();
                    on_set(changes, window, cx);
                }
            };
            let (list, all_items) = (self.state.clone(), items.clone());
            let order = Select::new(&order).on_change(move |next: &Order, _, cx| {
                let next = *next;
                let all = all_items.clone();
                list.update(cx, |s, cx| s.set_order(next, &all, cx));
            });
            let acts = h_flex()
                .gap_2()
                .child(order)
                .child(Button::outline(format!("{}-all", self.id), "All").xs().on_click(set(|i| (!i.on).then_some(true))))
                .child(Button::outline(format!("{}-none", self.id), "None").xs().on_click(set(|i| i.on.then_some(false))))
                .child(Button::outline(format!("{}-invert", self.id), "Invert").xs().on_click(set(|i| Some(!i.on))));
            let field = h_flex().flex_1().min_w_0().child(Field::new(&filter).fill());
            if crate::ui::narrow(window) {
                v_flex().debug_selector(|| "pick-tools".into()).gap_2().child(acts).child(field)
            } else {
                h_flex().debug_selector(|| "pick-tools".into()).gap_2().child(field).child(acts)
            }
        };
        let mut rows: Vec<CardItem> = Vec::new();
        for item in &shown {
            let tick = Tick {
                key: item.key.clone(),
                on: item.on,
                locked: item.locked,
                told: state.told(&item.key),
                state: self.state.clone(),
                items: items.clone(),
                on_set: self.on_set.clone(),
            };
            rows.extend((self.row)(item, tick));
        }
        if shown.is_empty() && !items.is_empty() {
            let none: AnyElement = div()
                .debug_selector(|| "pick-none-match".into())
                .text_size(text::NOTE.0)
                .line_height(text::NOTE.1)
                .text_color(p.muted_foreground)
                .child(format!("No row matches {filter_words}."))
                .into_any_element();
            rows.push(CardItem::placed(move |corners| {
                corners.apply(h_flex()).min_h(size::LINE_FLOOR).py_3().px(corners.inset).child(none).into_any_element()
            }));
        }
        v_flex()
            .id(self.id.clone())
            .debug_selector(|| "pick-list".into())
            .gap_3()
            .child(tools)
            .child(Card::new(format!("{}-rows", self.id)).inset(size::DIALOG_INSET).items(rows))
    }
}
