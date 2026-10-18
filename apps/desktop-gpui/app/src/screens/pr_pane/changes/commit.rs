// SPDX-License-Identifier: AGPL-3.0-only
//! The box a commit is written in, under the head (`diffs/CommitBox.tsx`): the message, filled from the workspace's
//! own agent's draft or left empty with the line saying why, the one line said while that agent still works in the
//! copy, and Commit, held until there is a message and a file ticked. The files are ticked in the tree above it.

use std::collections::HashSet;

use gpui_kit::component::input::TextareaState;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, px, App, Entity, Window};

use super::words;
use crate::ui::{spinner, ActivePalette as _, Button, TextArea};

pub struct CommitState {
    pub message: Entity<TextareaState>,
    pub drafting: bool,
    pub note: Option<String>,
    pub ticked: HashSet<String>,
    pub busy: bool,
    pub error: Option<String>,
}

impl CommitState {
    pub fn new(paths: &[String], window: &mut Window, cx: &mut App) -> Self {
        CommitState {
            message: TextArea::state(words::COMMIT_FIELD, window, cx),
            drafting: true,
            note: None,
            ticked: paths.iter().cloned().collect(),
            busy: false,
            error: None,
        }
    }

    pub fn message(&self, cx: &App) -> String {
        self.message.read(cx).value().to_string()
    }

    /// Commit waits on the draft, a message, a ticked file, or the commit already on its way.
    pub fn held(&self, cx: &App) -> bool {
        self.busy || self.drafting || self.message(cx).trim().is_empty() || self.ticked.is_empty()
    }
}

/// The box as drawn; `on_commit` and `on_cancel` are the foot's two acts.
pub fn commit_box(
    state: &CommitState,
    working: bool,
    on_commit: impl Fn(&mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let p = cx.palette();
    let said = state.drafting || state.error.is_some() || state.note.is_some() || working;
    let line = said.then(|| {
        let first = if state.drafting {
            Some(h_flex().gap(px(6.)).child(spinner(cx)).child(words::DRAFTING).into_any_element())
        } else if let Some(error) = &state.error {
            Some(div().text_color(p.error_foreground).child(error.clone()).into_any_element())
        } else {
            state.note.clone().map(|note| div().child(note).into_any_element())
        };
        h_flex()
            .debug_selector(|| "commit-line".into())
            .min_h(px(20.))
            .gap(px(12.))
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(p.muted_foreground)
            .children(first)
            .when(working, |el| el.child(div().debug_selector(|| "commit-working".into()).child(words::AGENT_WORKING)))
    });
    v_flex()
        .debug_selector(|| "commit-box".into())
        .flex_none()
        .gap(px(8.))
        .px(px(12.))
        .py(px(10.))
        .border_b_1()
        .border_color(p.border.opacity(0.6))
        .child(TextArea::new("commit-message", &state.message).mono().text_size(px(12.), px(18.)).min_height(px(96.)).disabled(state.busy))
        .children(line)
        .child(
            h_flex()
                .justify_end()
                .gap(px(8.))
                .child(
                    Button::outline("commit-cancel", words::CANCEL)
                        .xs()
                        .held(state.busy)
                        .on_click(move |_, window, cx| on_cancel(window, cx)),
                )
                .child(
                    Button::primary("commit-go", words::COMMIT)
                        .xs()
                        .held(state.held(cx))
                        .busy(state.busy)
                        .on_click(move |_, window, cx| on_commit(window, cx)),
                ),
        )
}
