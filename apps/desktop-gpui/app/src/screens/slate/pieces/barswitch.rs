// SPDX-License-Identifier: AGPL-3.0-only
//! Bar lists written one after another or held in a grid or row of nothing else, after `barswitch.tsx`: one card
//! holding every list under a segmented control naming them, one list shown at a time. Every list stands in the card's
//! one grid cell, the others invisible, so the card is the tallest list's height whatever is picked.

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, px, AnyElement, App, SharedString, Window};

use super::{draw, layout, look, PieceCx};
use crate::ui::{ActivePalette, Segmented};

/// Each list's name, the label or title it was given, else its place in the switch; and the window every name ends in,
/// such as (15m), said once beside the control rather than in every segment.
pub fn list_names(given: &[Option<String>]) -> (Vec<String>, Option<String>) {
    let names: Vec<String> = given
        .iter()
        .enumerate()
        .map(|(at, name)| match name.as_deref().map(str::trim) {
            Some(n) if !n.is_empty() => n.to_owned(),
            _ => format!("List {}", at + 1),
        })
        .collect();
    let tails: Vec<Option<(&str, &str)>> = names.iter().map(|n| window_of(n)).collect();
    let tail = tails.first().copied().flatten().map(|(_, t)| t);
    match tail {
        Some(tail) if names.len() >= 2 && tails.iter().all(|t| t.is_some_and(|(_, w)| w == tail)) => {
            let heads = tails.iter().map(|t| t.map(|(h, _)| h.to_owned()).unwrap_or_default()).collect();
            (heads, Some(tail.to_owned()))
        }
        _ => (names, None),
    }
}

/// A name's head and the window in brackets it ends in, as `^(.*\S)\s+\(([^()]+)\)$` splits it.
fn window_of(name: &str) -> Option<(&str, &str)> {
    let body = name.strip_suffix(')')?;
    let open = body.rfind('(')?;
    let (before, inner) = (&body[..open], &body[open + 1..]);
    if inner.is_empty() || inner.contains(')') || !before.ends_with(char::is_whitespace) {
        return None;
    }
    let head = before.trim_end();
    (!head.is_empty()).then_some((head, inner))
}

/// The switch over the bar lists `ids` of one run, `p` the piece the run stands in.
pub fn bar_switch(p: &PieceCx, ids: &[String], window: &mut Window, cx: &mut App) -> AnyElement {
    let slate = p.slate;
    let shown: Vec<String> = ids.iter().filter(|id| slate.shown(id)).cloned().collect();
    let Some(first) = shown.first().cloned() else { return div().into_any_element() };
    // Keyed by the first list, so a value push or a redraw keeps the pick.
    let kept = window.use_keyed_state(SharedString::from(format!("slate-bar-switch-{first}")), cx, |_, _| first.clone());
    let picked = shown.iter().position(|id| id == kept.read(cx)).unwrap_or(0);
    let given: Vec<Option<String>> = shown
        .iter()
        .map(|id| {
            let props = slate.doc().and_then(|d| d.piece(id)).map(|piece| slate.props(piece)).unwrap_or_default();
            look::str(props.get("label").filter(|v| !v.is_null())).or_else(|| look::str(props.get("title").filter(|v| !v.is_null())))
        })
        .collect();
    let (names, shared) = list_names(&given);
    let lists = shown.clone();
    let control = names
        .into_iter()
        .fold(Segmented::new(format!("slate-bar-switch-{first}-lists"), picked).keycap(), |s, name| s.segment(name))
        .on_pick(move |ix, _, cx| {
            if let Some(id) = lists.get(ix) {
                kept.update(cx, |k, cx| {
                    *k = id.clone();
                    cx.notify();
                });
            }
        });
    let muted = cx.palette().muted_foreground;
    let cells = shown.iter().enumerate().map(|(ix, id)| {
        div()
            .col_start(1)
            .row_start(1)
            .min_w_0()
            .debug_selector({
                let id = id.clone();
                move || format!("slate-bar-list-{id}")
            })
            .when(ix != picked, |el| el.invisible())
            .children(draw(slate, id, &p.screen, window, cx))
    });
    let body = div().grid().grid_cols(1).min_w_0().children(cells.collect::<Vec<_>>());
    v_flex()
        .debug_selector(|| "slate-bar-switch".into())
        .min_w_0()
        .gap(px(10.))
        .child(
            h_flex()
                .min_w_0()
                .gap_3()
                .child(
                    div().id(SharedString::from(format!("slate-bar-switch-{first}-scroll"))).min_w_0().overflow_x_scroll().child(control),
                )
                .children(shared.map(|w| {
                    div()
                        .debug_selector(|| "slate-bar-switch-note".into())
                        .ml_auto()
                        .flex_none()
                        .whitespace_nowrap()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(muted)
                        .child(w)
                })),
        )
        .child(layout::surface(SharedString::from(format!("slate-bar-switch-{first}-card")), body))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::list_names;

    fn names(given: &[Option<&str>]) -> (Vec<String>, Option<String>) {
        list_names(&given.iter().map(|g| g.map(str::to_owned)).collect::<Vec<_>>())
    }

    #[test]
    fn a_window_every_name_ends_in_is_said_once() {
        assert_eq!(names(&[Some("CPU (15m)"), Some("Memory (15m)")]), (vec!["CPU".into(), "Memory".into()], Some("15m".into())));
        assert_eq!(names(&[Some("CPU (15m)"), Some("Memory (1h)")]), (vec!["CPU (15m)".into(), "Memory (1h)".into()], None));
        assert_eq!(names(&[Some("CPU (15m)")]), (vec!["CPU (15m)".into()], None));
        assert_eq!(names(&[Some("CPU(15m)"), Some("Memory(15m)")]).1, None);
        assert_eq!(names(&[Some("(15m)"), Some("Memory (15m)")]).1, None);
    }

    #[test]
    fn a_list_with_no_label_reads_its_place() {
        assert_eq!(names(&[None, Some("  "), Some(" Disk ")]).0, vec!["List 1", "List 2", "Disk"]);
        assert_eq!(names(&[Some("CPU (15m)"), None]), (vec!["CPU (15m)".into(), "List 2".into()], None));
    }
}
