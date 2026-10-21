// SPDX-License-Identifier: AGPL-3.0-only
//! The Processes pane of a thread on this computer or on a box the person joined (`ThreadProcesses.tsx`): Activity's
//! Threads table over that thread's own processes and, folded under it, each of its child threads', read by Activity's
//! one rule over the whole computer so a sibling in the same folder is never drawn here; Open in Activity at its foot.

use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, SharedString, Window};

use super::tree::words as w;
use super::{measure, width_of, Form, Processes, ThreadState};
use crate::screens::computer::daemon_down::DaemonDown;
use crate::screens::settings::pages::activity::model::{words as activity_words, View};
use crate::screens::settings::pages::activity::tables::{threads_table, Hand, ThreadLooks};
use crate::ui::{scroll_area, text, ActivePalette, Button};

fn note(word: impl Into<SharedString>, cx: &App) -> gpui_kit::Div {
    div().text_size(text::NOTE.0).line_height(text::NOTE.1).text_color(cx.palette().muted_foreground).child(word.into())
}

pub fn view(pane: &mut Processes, window: &mut Window, cx: &mut Context<Processes>) -> impl IntoElement {
    let Some(Form::Thread(scoped)) = pane.form.as_ref() else { return div().into_any_element() };
    let scoped = scoped.clone();
    let state = pane.thread_state(cx).unwrap_or(ThreadState::Pending);
    let p = cx.palette().clone();
    let body: AnyElement = match state {
        ThreadState::Absent(absent) => {
            let id = pane.opened.workspace_id.clone().unwrap_or_default();
            let start = absent.start.clone().map(SharedString::from);
            v_flex()
                .debug_selector(|| "procs-absent".into())
                .items_start()
                .gap_2()
                .py_1()
                .child(note(absent.said.clone(), cx))
                .children(absent.will.clone().map(|will| note(will, cx)))
                .child(DaemonDown::new(pane.host.clone(), id, absent.said, start).button_only().state(pane.restart.clone()))
                .into_any_element()
        }
        ThreadState::Refused(said) => note(said, cx).debug_selector(|| "procs-unavailable".into()).into_any_element(),
        ThreadState::Pending => note(activity_words::PENDING, cx).debug_selector(|| "procs-pending".into()).into_any_element(),
        ThreadState::Nothing => note(w::NOTHING, cx).debug_selector(|| "procs-nothing".into()).m_auto().text_center().into_any_element(),
        ThreadState::Groups(groups) => {
            let looks = pane.looks(cx);
            let width = width_of(pane, window);
            scoped.update(cx, |s, cx| s.drawing(View::Threads, width, cx));
            let s = scoped.read(cx);
            let hand = Hand {
                scoped: scoped.downgrade(),
                ending: Rc::new(s.ending.clone()),
                stopping: Rc::new(s.stopping.clone()),
                open: Rc::new(s.open.clone()),
                sort: s.sort,
                energy: s.scope.as_ref().is_some_and(|sc| sc.energy),
                filtering: false,
                width,
                workspace: pane.opened.workspace_id.clone(),
                pieces: s.pieces(),
            };
            let looks = ThreadLooks {
                status: Rc::new(move |id: &str| looks.get(id).cloned()),
                model: Rc::new(|_: &str, m: &str| Some(m.to_owned())),
            };
            let name = s.computer.name();
            let boxed = !s.computer.here();
            threads_table(name, Rc::new(groups), looks, boxed, None, &hand).whole(cx)
        }
    };
    let menu = scoped.read(cx).menu.overlay().map(|open| div().absolute().debug_selector(|| "procs-row-menu".into()).child(open));
    let content = v_flex().min_h_full().p_3().child(body);
    v_flex()
        .debug_selector(|| "procs-thread".into())
        .relative()
        .size_full()
        .min_h_0()
        .child(scroll_area("procs-thread-scroll", &pane.scroll, content, window, cx).flex_1().min_h_0())
        .child(
            h_flex().h(px(36.)).flex_none().justify_end().px_3().border_t_1().border_color(p.border.opacity(0.5)).child(
                div().debug_selector(|| "procs-open-activity".into()).child(
                    Button::text("procs-open-activity", w::OPEN_IN_ACTIVITY)
                        .on_click(cx.listener(|this, _, window, cx| this.open_activity(window, cx))),
                ),
            ),
        )
        .children(menu)
        .child(measure(pane.width.clone()))
        .into_any_element()
}
