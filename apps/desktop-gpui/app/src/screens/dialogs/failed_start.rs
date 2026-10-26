// SPDX-License-Identifier: AGPL-3.0-only
//! A failed start (`shell/FailedStart.tsx`): the main column for a workspace whose create failed before any machine
//! stood, a fork a thread asked for. No history and nothing to send to, so the page is the card the refused create ends
//! on: the step it stopped on with the reason, and the thread that asked for it, where it is started again.

use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, px, App, SharedString, Window};
use serde_json::Value;

use crate::ui::{ActivePalette, Button, Card};
use crate::ui::{Step, StepRow};

/// `FAILED_START_WORDS.next`.
pub const NEXT: &str = "Start it again from the thread that asked for it.";
/// `FAILED_START_WORDS.openLead`.
pub const OPEN_LEAD: &str = "Open that thread";
/// `CREATE_ASKED`: a create that reported no step before it was refused is the one step, the asking.
pub const ASKED: &str = "Asking wsp to start it";
/// `max-w-3xl`.
const COLUMN: f32 = 768.;

/// The thread a fork was asked for by.
#[derive(Clone, Debug, PartialEq)]
pub struct Lead {
    pub workspace_id: String,
    pub thread_id: String,
}

/// What the page says of one workspace.
#[derive(Clone, Debug, PartialEq)]
pub struct Failed {
    pub title: String,
    /// Why, as the host kept it; empty where it kept none.
    pub said: String,
    pub lead: Option<Lead>,
}

impl Failed {
    /// `startFailed`: gone with no machine ever named. Nothing for any other workspace.
    pub fn of(workspace: &Value) -> Option<Failed> {
        if !crate::screens::sidebar::model::start_failed(workspace) {
            return None;
        }
        let word = |key: &str| workspace.get(key).and_then(Value::as_str);
        let lead = word("parentWorkspaceId")
            .zip(word("parentThreadId"))
            .map(|(w, t)| Lead { workspace_id: w.to_owned(), thread_id: t.to_owned() });
        Some(Failed {
            title: format!("Could not start {}", word("name").unwrap_or_default()),
            said: word("gone").unwrap_or_default().to_owned(),
            lead,
        })
    }
}

type OnOpen = Rc<dyn Fn(&Lead, &mut Window, &mut App)>;

/// The page: the card in the middle of the column, its one failed row opened on the reason and the act.
#[derive(IntoElement)]
pub struct FailedStart {
    failed: Failed,
    on_open: OnOpen,
}

impl FailedStart {
    pub fn new(failed: Failed, on_open: impl Fn(&Lead, &mut Window, &mut App) + 'static) -> Self {
        FailedStart { failed, on_open: Rc::new(on_open) }
    }
}

impl RenderOnce for FailedStart {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let Failed { title, said, lead } = self.failed;
        let mut row = StepRow::new("asked", Step::Failed, ASKED).said(said, None);
        if let Some(lead) = lead.clone() {
            let open = self.on_open.clone();
            row = row.act(
                div()
                    .debug_selector(|| "failed-start-lead".into())
                    .child(Button::ghost("failed-start-lead", OPEN_LEAD).xs().on_click(move |_, window, cx| open(&lead, window, cx))),
            );
        }
        let card = Card::new("setting-up").head(SharedString::from(title)).when(lead.is_some(), |card| card.lede(NEXT)).item(row);
        div().debug_selector(|| "failed-start".into()).relative().size_full().text_color(p.foreground).child(
            v_flex()
                .id("setting-up-room")
                .absolute()
                .inset_0()
                .gap_4()
                .overflow_y_scroll()
                .px_5()
                .pt_4()
                .child(h_flex().flex_1().items_center().justify_center().pb_6().child(div().w_full().max_w(px(COLUMN)).child(card))),
        )
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn only_a_fork_gone_before_its_machine_stood_is_a_failed_start() {
        let failed = json!({ "name": "fix-login", "phase": "gone", "machineId": "", "gone": "the provider said no",
            "parentWorkspaceId": "w1", "parentThreadId": "t1" });
        assert_eq!(
            Failed::of(&failed),
            Some(Failed {
                title: "Could not start fix-login".into(),
                said: "the provider said no".into(),
                lead: Some(Lead { workspace_id: "w1".into(), thread_id: "t1".into() }),
            })
        );
        assert_eq!(Failed::of(&json!({ "name": "a", "phase": "gone", "machineId": "m1" })), None);
        assert_eq!(Failed::of(&json!({ "name": "a", "phase": "running", "machineId": "" })), None);
        assert_eq!(Failed::of(&json!({ "name": "a", "phase": "gone", "machineId": "", "parentWorkspaceId": "w1" })).unwrap().lead, None);
    }
}
