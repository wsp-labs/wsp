// SPDX-License-Identifier: AGPL-3.0-only
//! The tab column beside the shells once a drawer holds two (`ThreadTerminalDrawer.tsx`): 144 px, the toolbar as its
//! head, then each group's row and its tabs.

use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, SharedString, Window};

use super::groups::{Direction, Ui};
use crate::ui::{tip, ActivePalette, Glyph};

pub const WIDTH: f32 = 144.;

/// What a press in the column asks of the drawer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    Shell(String),
    Close(String),
    Group(String),
}

pub type OnPick = Rc<dyn Fn(Pick, &mut Window, &mut App)>;

/// A group's word and glyph: "Single", "Side by side" or "Stacked".
fn kind(len: usize, direction: Direction) -> (&'static str, Glyph) {
    match (len, direction) {
        (0 | 1, _) => ("Single", Glyph::Square),
        (_, Direction::Vertical) => ("Stacked", Glyph::SquareSplitVertical),
        (_, Direction::Horizontal) => ("Side by side", Glyph::SquareSplitHorizontal),
    }
}

/// The column under its head; `label` names a shell as its tab reads.
pub fn column(head: AnyElement, ui: &Ui, label: &dyn Fn(&str) -> SharedString, on: OnPick, cx: &App) -> AnyElement {
    let p = cx.palette();
    let headers = ui.terminal_groups.len() > 1 || ui.terminal_groups.iter().any(|g| g.terminal_ids.len() > 1);
    let groups = ui.terminal_groups.iter().map(|group| {
        let active_group = group.terminal_ids.contains(&ui.active_terminal_id);
        let (word, glyph) = kind(group.terminal_ids.len(), group.split_direction);
        let pick_group = on.clone();
        let gid = group.id.clone();
        let head = headers.then(|| {
            h_flex()
                .id(SharedString::from(format!("terminal-group-{}", group.id)))
                .debug_selector(|| "terminal-group".into())
                .h(px(22.))
                .w_full()
                .items_center()
                .gap_1()
                .rounded(px(8.))
                .px(px(6.))
                .text_size(px(11.))
                .cursor_pointer()
                .map(|el| {
                    if active_group {
                        el.bg(p.accent.opacity(0.5)).text_color(p.foreground)
                    } else {
                        el.text_color(p.muted_foreground).hover(|s| s.bg(p.accent.opacity(0.4)).text_color(p.foreground))
                    }
                })
                .on_click(move |_, window, cx| pick_group(Pick::Group(gid.clone()), window, cx))
                .child(Icon::new(glyph).with_size(px(12.)).flex_none())
                .child(div().min_w_0().flex_1().truncate().child(word))
                .when(group.terminal_ids.len() > 1, |el| {
                    el.child(
                        div()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_color(p.muted_foreground)
                            .child(group.terminal_ids.len().to_string()),
                    )
                })
        });
        let tabs = group.terminal_ids.iter().map(|id| {
            let active = *id == ui.active_terminal_id;
            let name = label(id);
            let close_tip: SharedString = format!("Close {name}").into();
            let (pick, close) = (on.clone(), on.clone());
            let (pick_id, close_id) = (id.clone(), id.clone());
            let row = SharedString::from(format!("terminal-tab-{id}"));
            h_flex()
                .id(row.clone())
                .debug_selector(|| "terminal-tab".into())
                .group(row.clone())
                .h(px(24.))
                .w_full()
                .items_center()
                .gap(px(2.))
                .rounded(px(8.))
                .pl(px(6.))
                .pr_2()
                .text_size(px(12.))
                .line_height(px(16.))
                .map(|el| {
                    if active {
                        el.bg(p.accent).text_color(p.foreground)
                    } else {
                        el.text_color(p.muted_foreground).hover(|s| s.bg(p.accent.opacity(0.6)).text_color(p.foreground))
                    }
                })
                .child(
                    div()
                        .id(SharedString::from(format!("terminal-tab-close-{id}")))
                        .debug_selector(|| "terminal-tab-close".into())
                        .relative()
                        .flex()
                        .flex_none()
                        .size(px(16.))
                        .items_center()
                        .justify_center()
                        .rounded(px(6.))
                        .cursor_pointer()
                        .hover(|s| s.bg(p.muted))
                        .tooltip(tip(close_tip))
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            close(Pick::Close(close_id.clone()), window, cx)
                        })
                        .child(div().group_hover(row.clone(), |s| s.invisible()).child(Icon::new(Glyph::SquareTerminal).with_size(px(12.))))
                        .child(
                            div()
                                .absolute()
                                .invisible()
                                .group_hover(row.clone(), |s| s.visible())
                                .child(Icon::new(Glyph::X).with_size(px(12.))),
                        ),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("terminal-tab-name-{id}")))
                        .flex()
                        .min_w_0()
                        .flex_1()
                        .cursor_pointer()
                        .on_click(move |_, window, cx| pick(Pick::Shell(pick_id.clone()), window, cx))
                        .child(div().truncate().child(name)),
                )
        });
        v_flex().pb(px(2.)).children(head).child(v_flex().gap(px(2.)).children(tabs))
    });
    v_flex()
        .id("terminal-tabs")
        .debug_selector(|| "terminal-tabs".into())
        .relative()
        .flex_none()
        .w(px(WIDTH))
        .min_w(px(WIDTH))
        .h_full()
        .border_l_1()
        .border_color(p.border)
        .bg(p.muted.opacity(0.1))
        .child(head)
        .child(v_flex().id("terminal-tab-list").min_h_0().flex_1().overflow_y_scroll().p_1().children(groups))
        .into_any_element()
}
