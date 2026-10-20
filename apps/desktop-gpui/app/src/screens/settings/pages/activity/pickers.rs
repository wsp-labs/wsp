// SPDX-License-Identifier: AGPL-3.0-only
//! The bar over the open view: the computer, kind and thread selects, the every-process switch and the filter.

use gpui_kit::component::input::Input;
use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, App, Entity, SharedString, Subscription, Window};

use super::model::{pick_choices, words as w, View};
use super::wire::Kind;
use super::{absent, Computer, Page, Scoped};
use crate::ui::select::{Select, SelectItem, SelectState};
use crate::ui::{text, ActivePalette, ComputerIcon, Glyph, Switch};

/// A select's state kept in step with what the page holds: its items and its value laid on again only when they move.
struct Synced<T: Clone + PartialEq + 'static> {
    state: Entity<SelectState<T>>,
    sig: Vec<(T, SharedString)>,
    value: T,
    _watch: Subscription,
}

pub(super) type Items<T> = Vec<(T, SharedString, Option<ComputerIcon>)>;

fn select_items<T: Clone + PartialEq + 'static>(items: &Items<T>) -> Vec<SelectItem<T>> {
    items
        .iter()
        .map(|(v, label, glyph)| {
            let item = SelectItem::new(v.clone(), label.clone());
            match *glyph {
                Some(icon) => item.mark(move |cx| icon.draw(cx.palette().foreground.opacity(0.8), px(14.))),
                None => item,
            }
        })
        .collect()
}

pub(super) fn synced<T: Clone + PartialEq + 'static>(
    key: String,
    items: Items<T>,
    value: T,
    window: &mut Window,
    cx: &mut App,
) -> Entity<SelectState<T>> {
    let sig: Vec<(T, SharedString)> = items.iter().map(|(v, l, _)| (v.clone(), l.clone())).collect();
    let (first, v0, sig0) = (select_items(&items), value.clone(), sig.clone());
    let held = window.use_keyed_state(SharedString::from(key), cx, move |_, cx| {
        let state = cx.new(|_| SelectState::new(first, Some(&v0)));
        let watch = cx.observe(&state, |_, _, cx| cx.notify());
        Synced { state, sig: sig0, value: v0, _watch: watch }
    });
    let (state, stale_items, stale_value) = {
        let h = held.read(cx);
        (h.state.clone(), h.sig != sig, h.value != value)
    };
    if stale_items || stale_value {
        held.update(cx, |h, _| {
            h.sig = sig;
            h.value = value.clone();
        });
        state.update(cx, |st, cx| {
            if stale_items {
                st.set_items(select_items(&items), cx);
            }
            st.set_value(Some(&value), cx);
        });
    }
    state
}

/// The bar's three selects: the computer, the kind and the thread.
pub(super) struct Selects {
    computer: Entity<SelectState<String>>,
    kind: Entity<SelectState<Option<Kind>>>,
    thread: Entity<SelectState<Option<String>>>,
}

pub(super) fn selects(computers: &[Computer], computer: &Computer, scoped: &Entity<Scoped>, window: &mut Window, cx: &mut App) -> Selects {
    let (kinds, threads, picks) = {
        let s = scoped.read(cx);
        let (kinds, threads) = match (&s.scope, absent(computer)) {
            (Some(sc), None) => pick_choices(&sc.groups, &s.picks),
            _ => (Vec::new(), Vec::new()),
        };
        (kinds, threads, s.picks.clone())
    };
    let places: Items<String> = computers.iter().map(|c| (c.place.id.clone(), SharedString::from(c.name()), Some(c.glyph))).collect();
    let mut kind_items: Items<Option<Kind>> = vec![(None, w::ALL_KINDS.into(), None)];
    kind_items.extend(kinds.into_iter().map(|k| (Some(k), SharedString::from(k.word()), None)));
    let mut thread_items: Items<Option<String>> = vec![(None, w::ALL_THREADS.into(), None)];
    thread_items.extend(threads.into_iter().map(|(id, title)| (Some(id), SharedString::from(title), None)));
    Selects {
        computer: synced("activity-computer".into(), places, computer.place.id.clone(), window, cx),
        kind: synced(format!("activity-kind-{}", computer.key), kind_items, picks.kind, window, cx),
        thread: synced(format!("activity-thread-{}", computer.key), thread_items, picks.thread, window, cx),
    }
}

/// `BAR_SELECT`'s `h-8`, the filter's height too.
const BAR_HEIGHT: gpui_kit::Pixels = px(32.);

/// The page's own pickers and its filter, at the height of the Agents page's computer picker. Under 640 px the pickers
/// stand on a grid of two, the computer's across both, and the filter takes the line under them.
pub(super) fn controls(
    page: &Page,
    selects: Selects,
    view: View,
    scoped: &Entity<Scoped>,
    s: &Scoped,
    narrow: bool,
    cx: &App,
) -> impl IntoElement {
    let p = cx.palette();
    let pick = page.pick.clone();
    let (kinds, threads) = (scoped.downgrade(), scoped.downgrade());
    let computer = Select::new(&selects.computer).width(px(224.)).height(BAR_HEIGHT).fill(narrow).on_change(move |id, _, cx| {
        let id = id.clone();
        pick.update(cx, |p, cx| {
            p.place = Some(id);
            cx.notify();
        })
    });
    let kind = Select::new(&selects.kind).width(px(160.)).height(BAR_HEIGHT).fill(narrow).on_change(move |kind, _, cx| {
        let kind = *kind;
        _ = kinds.update(cx, |s, cx| {
            s.picks.kind = kind;
            cx.notify();
        });
    });
    let thread = Select::new(&selects.thread).width(px(192.)).height(BAR_HEIGHT).fill(narrow).on_change(move |thread, _, cx| {
        let thread = thread.clone();
        _ = threads.update(cx, |s, cx| {
            s.picks.thread = thread;
            cx.notify();
        });
    });
    let (computer, kind, thread) = (
        div().debug_selector(|| "activity-computer".into()).child(computer),
        div().debug_selector(|| "activity-kind".into()).child(kind),
        div().debug_selector(|| "activity-thread".into()).child(thread),
    );
    let left = if narrow {
        v_flex().w_full().gap_2().child(computer).child(h_flex().gap_2().child(kind.flex_1().min_w_0()).child(thread.flex_1().min_w_0()))
    } else {
        h_flex().min_w_0().flex_wrap().gap_2().child(computer).child(kind).child(thread)
    };
    let every = (view == View::Processes).then(|| {
        let scoped = scoped.downgrade();
        h_flex()
            .flex_none()
            .gap_2()
            .text_size(text::NOTE.0)
            .line_height(text::NOTE.1)
            .text_color(p.muted_foreground)
            .child(Switch::new("activity-every", s.every).on_change(move |on, _, cx| {
                let on = *on;
                _ = scoped.update(cx, |s, cx| {
                    s.every = on;
                    s.set_reads(View::Processes, cx);
                    cx.notify();
                });
            }))
            .child(w::EVERY)
    });
    let filter = Input::new(&s.filter)
        .prefix(Icon::new(Glyph::Search).with_size(px(14.)).text_color(p.muted_foreground))
        .h(BAR_HEIGHT)
        .when(!narrow, |el| el.w(px(256.)))
        .when(narrow, |el| el.flex_1().min_w_0())
        .rounded(crate::ui::size::CONTROL_RADIUS)
        .border_color(p.input)
        .bg(p.input_fill)
        .text_size(text::FACT.0);
    h_flex().debug_selector(|| "activity-controls".into()).flex_wrap().justify_between().gap_x_4().gap_y_2().child(left).child(
        h_flex()
            .when(narrow, |el| el.w_full())
            .gap_4()
            .children(every)
            .child(div().debug_selector(|| "activity-filter".into()).when(narrow, |el| el.flex_1().min_w_0()).child(filter)),
    )
}
