// SPDX-License-Identifier: AGPL-3.0-only
//! `SpawnTiles`: the children a call started, each in the row's box, and where a press on one moves the window.

use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, SharedString};

use super::subagent_fold::{lead_of, subagent_route};
use super::SM_TIGHT;
use crate::screens::{navigate, Route};
use crate::thread_view::ThreadView;
use crate::transcript::{Child, ChildPart, Listed, ThreadAt};
use crate::ui::{catalog, Glyph, Palette, Status, StatusMark};

/// `SpawnTiles`: one column, each child a `ThreadRow` or `SubagentRow` in the row's box.
pub(super) fn spawn(view: &ThreadView, key: &str, children: &[Child], p: &Palette, cx: &App) -> AnyElement {
    let lead = lead_of(view);
    v_flex()
        .debug_selector(|| "spawn".into())
        .min_w_0()
        .children(children.iter().enumerate().map(|(at_ix, child)| spawn_row(key, at_ix, child, lead.as_ref(), p, cx)))
        .into_any_element()
}

#[cfg(test)]
thread_local! {
    /// Every move the cards asked the frame for in this test thread.
    pub static ROUTED: std::cell::RefCell<Vec<Route>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Moves the window to a thread or a page.
pub(super) fn go(route: Route, cx: &mut App) {
    #[cfg(test)]
    ROUTED.with(|r| r.borrow_mut().push(route.clone()));
    navigate(route, cx);
}

pub(super) fn open_thread(at: &ThreadAt, cx: &mut App) {
    go(Route::Thread { workspace_id: at.workspace.clone(), thread_id: at.thread.clone() }, cx);
}

/// The listing's kind as the status slot draws it.
pub(super) fn listed_status(listed: &Listed) -> Status {
    crate::screens::sidebar::tile::status_of(listed.kind, listed.slot.clone())
}

/// One child in the row's box (`rowBox`, `ThreadRow`, `SubagentRow`): 36 px, or at least 48 with a second line; the
/// mark, the title over the computer and the note, then the status slot. A press opens the thread, or a subagent's page.
fn spawn_row(key: &str, ix: usize, child: &Child, lead: Option<&ThreadAt>, p: &Palette, cx: &App) -> AnyElement {
    let listed = child.listed.as_ref();
    let status = match (listed, child.settled, child.status.as_deref()) {
        (Some(listed), ..) => Some(listed_status(listed)),
        (None, true, _) | (None, _, None) => None,
        (None, false, Some("running")) => Some(Status::Working(SharedString::default())),
        (None, false, Some("done")) => Some(Status::Done),
        (None, false, Some("failed")) => Some(Status::Failed),
        _ => None,
    };
    let settled = listed.map_or(child.settled, |l| l.part == ChildPart::Settled);
    let note = listed.map_or(child.note.clone(), |l| l.note.clone());
    let mark = if child.subagent {
        Icon::new(Glyph::Bot).with_size(px(13.)).text_color(p.muted_foreground).into_any_element()
    } else {
        match listed.map(|l| l.harness.as_str()).filter(|h| !h.is_empty()) {
            Some(harness) => catalog::mark(harness, &catalog::name(harness), px(13.), p.muted_foreground, cx),
            None => div().into_any_element(),
        }
    };
    let press: Option<Route> = match (listed, &child.call, lead) {
        (Some(listed), ..) => Some(Route::Thread { workspace_id: listed.at.workspace.clone(), thread_id: listed.at.thread.clone() }),
        (None, Some(call), Some(lead)) if child.subagent => Some(Route::Screen(subagent_route(lead, call).into())),
        _ => None,
    };
    child_row(
        SharedString::from(format!("spawn-{key}-{ix}")),
        "spawn-row",
        mark,
        &child.title,
        settled,
        listed.and_then(|l| l.computer.clone()),
        note,
        status,
        press,
        p,
    )
}

/// The row's box every child takes, in the Threads section and at the calls that started it.
#[allow(clippy::too_many_arguments)]
fn child_row(
    id: SharedString,
    selector: &'static str,
    mark: AnyElement,
    title: &str,
    settled: bool,
    computer: Option<String>,
    note: Option<String>,
    status: Option<Status>,
    press: Option<Route>,
    p: &Palette,
) -> AnyElement {
    let two = note.is_some() || computer.is_some();
    let second = two.then(|| {
        h_flex()
            .min_w_0()
            .gap(px(12.))
            .text_size(px(11.))
            .line_height(px(14.))
            .text_color(p.muted_foreground)
            .children(computer.map(|c| div().flex_none().child(c)))
            .children(note.map(|note| div().min_w_0().truncate().child(note)))
    });
    h_flex()
        .id(id)
        .debug_selector(move || selector.into())
        .min_w_0()
        .gap(px(10.))
        .px(px(8.))
        .rounded(px(8.))
        .text_size(SM_TIGHT.0)
        .line_height(SM_TIGHT.1)
        .hover(|s| s.bg(p.accent))
        .map(|el| if two { el.min_h(px(48.)).py(px(6.)) } else { el.h(px(36.)) })
        .child(div().flex().flex_none().items_center().justify_center().size(px(13.)).child(mark))
        .child(
            v_flex()
                .flex_1()
                .min_w(px(112.))
                .child(
                    div().min_w_0().truncate().text_color(if settled { p.muted_foreground } else { p.foreground }).child(title.to_owned()),
                )
                .children(second),
        )
        .children(status.map(StatusMark::new))
        .when_some(press, |el, route| el.cursor_pointer().on_click(move |_, _, cx| go(route.clone(), cx)))
        .into_any_element()
}
