// SPDX-License-Identifier: AGPL-3.0-only
//! The one line and the one button every pane that needs a daemon this host started shows while it is not running,
//! after `components/DaemonDown.tsx`: the reading's sentence, and Start again, which asks `workspaces.restartDaemon`,
//! reads Starting… while it is out, and leaves the host's refusal in the refusal ink under itself. A pane draws it
//! with its host, its workspace and the two halves of the reading the sidebar's `menu::absence` gives: what happened,
//! and the button's word where this host can start what is missing.

use gpui_kit::component::v_flex;
use gpui_kit::{div, prelude::*, px, App, ElementId, Entity, SharedString, Window};
use serde_json::json;
use wsp_host_client::protocol::WorkspacesRestartDaemonRequest;
use wsp_host_client::Failure;

use crate::screens::Host;
use crate::ui::{text, ActivePalette, Button};

pub const STARTING: &str = "Starting…";

/// What the last ask left: out, or refused with the line to show.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Restart {
    pub busy: bool,
    pub refusal: Option<String>,
}

/// `NO_REASON`: what a refusal with no sentence says; the host client words one as the op having failed.
pub const NO_REASON: &str = "The host answered with no reason. Try again.";

/// `startRefusedLine`: the host's sentence, then its fix, else another start; a refusal with no sentence asks for
/// another try already.
pub fn start_refused_line(refused: &Failure) -> String {
    let said = refused.said().trim_end();
    // `refusalLine`: what happened, closed with a full stop where it does not close itself, then what to do.
    let then = |fix: &str| format!("{said}{} {fix}", if said.ends_with(['.', '!', '?', ':']) { "" } else { "." });
    match refused.fix.as_deref() {
        Some(fix) => then(fix),
        None if said == NO_REASON || said == "workspaces.restartDaemon failed" => NO_REASON.to_owned(),
        None => then("Start again."),
    }
}

/// Asks the host to start `workspace_id`'s daemon again, holding `state` busy until it answers.
pub fn restart(host: &Host, workspace_id: &str, state: &Entity<Restart>, cx: &mut App) {
    state.update(cx, |s, cx| {
        *s = Restart { busy: true, refusal: None };
        cx.notify();
    });
    let id = workspace_id.to_owned();
    let asking = host.run(move |client| async move {
        let op: WorkspacesRestartDaemonRequest =
            serde_json::from_value(json!({ "workspaceId": id })).map_err(|e| Failure::new(e.to_string()))?;
        client.call(op).await.map(drop)
    });
    let state = state.downgrade();
    cx.spawn(async move |cx| {
        let answer = asking.await;
        _ = state.update(cx, |s, cx| {
            *s = Restart { busy: false, refusal: answer.err().map(|refused| start_refused_line(&refused)) };
            cx.notify();
        });
    })
    .detach();
}

/// `DaemonDown`, or `StartDaemonButton` alone for a pane whose title already says the sentence.
#[derive(IntoElement)]
pub struct DaemonDown {
    host: Host,
    workspace_id: SharedString,
    said: SharedString,
    start: Option<&'static str>,
    sentence: bool,
    state: Option<Entity<Restart>>,
}

impl DaemonDown {
    pub fn new(host: Host, workspace_id: impl Into<SharedString>, said: impl Into<SharedString>, start: Option<&'static str>) -> Self {
        DaemonDown { host, workspace_id: workspace_id.into(), said: said.into(), start, sentence: true, state: None }
    }

    /// The button and its refusal, with no sentence.
    pub fn button_only(mut self) -> Self {
        self.sentence = false;
        self
    }

    /// The ask's state held by the pane, which outlives a frame the piece is not drawn in; else it is kept for as long
    /// as the piece is drawn.
    pub fn state(mut self, state: Entity<Restart>) -> Self {
        self.state = Some(state);
        self
    }

    /// The button's word.
    pub fn word(busy: bool, start: &'static str) -> &'static str {
        if busy {
            STARTING
        } else {
            start
        }
    }
}

impl RenderOnce for DaemonDown {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let key = ElementId::from(SharedString::from(format!("daemon-down-{}", self.workspace_id)));
        let state = self.state.clone().unwrap_or_else(|| window.use_keyed_state(key, cx, |_, _| Restart::default()));
        let Restart { busy, refusal } = state.read(cx).clone();
        let (host, id) = (self.host.clone(), self.workspace_id.clone());
        let button = self.start.map(|start| {
            Button::outline(SharedString::from(format!("start-daemon-{id}")), DaemonDown::word(busy, start))
                .xs()
                .held(busy)
                .on_click(move |_, _, cx| restart(&host, &id, &state, cx))
        });
        let refused = refusal.filter(|_| button.is_some()).map(|line| {
            div()
                .debug_selector(|| "start-daemon-refused".into())
                .text_size(text::XS.0)
                .line_height(text::XS.1)
                .text_color(p.error_foreground)
                .child(line)
        });
        let body = v_flex().debug_selector(|| "daemon-down".into());
        if !self.sentence {
            return body.items_start().gap_2().children(button).children(refused);
        }
        body.flex_1()
            .items_center()
            .justify_center()
            .gap_2()
            .text_center()
            .child(
                div().text_size(text::SETTING_TITLE.0).line_height(text::SETTING_TITLE.1).text_color(p.foreground).child(self.said.clone()),
            )
            .children(button)
            .children(refused)
            .min_w(px(0.))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refused_start_says_the_hosts_sentence_then_to_start_again() {
        assert_eq!(start_refused_line(&Failure::new("the daemon did not start")), "the daemon did not start. Start again.");
        assert_eq!(start_refused_line(&Failure::new("no daemon for this computer.")), "no daemon for this computer. Start again.");
        assert_eq!(start_refused_line(&Failure::new(NO_REASON)), NO_REASON, "a refusal with no sentence already asks again");
        assert_eq!(start_refused_line(&Failure::new("workspaces.restartDaemon failed")), NO_REASON);
        let fixed = Failure {
            fix: Some("Run wsp add mini --update.".into()),
            ..Failure::new("mini's daemon is too old Run wsp add mini --update.")
        };
        assert_eq!(
            start_refused_line(&fixed),
            "mini's daemon is too old. Run wsp add mini --update.",
            "the host's fix takes the place of another start"
        );
    }

    #[test]
    fn the_button_reads_starting_while_the_ask_is_out() {
        assert_eq!(DaemonDown::word(false, "Start again"), "Start again");
        assert_eq!(DaemonDown::word(true, "Start again"), "Starting…");
    }
}
