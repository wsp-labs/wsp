// SPDX-License-Identifier: AGPL-3.0-only
//! What the pane holds while there is no daemon to read a diff over (`diffs/NotRunning.tsx`): the computer's sentence
//! and the button that starts again a daemon this host runs, else the word and the line of a link that is down, else
//! the one line that says the thread is not running over the computer's sentence or the pane's own line.

use gpui_kit::component::v_flex;
use gpui_kit::{div, prelude::*, px, AnyElement, App, Pixels, SharedString, Window};

use std::time::Instant;

use super::words;
use crate::screens::terminal::link::Status;
use crate::screens::terminal::state::{Absent, PaneState};
use crate::screens::terminal::words as pane_words;
use crate::ui::ActivePalette as _;

/// Tailwind's `md`, past which an empty state's padding doubles.
const MD: Pixels = px(768.);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Form {
    /// `DaemonDown`: the computer's sentence and the Start again button.
    DaemonDown { said: String, start: String },
    /// `useLinkDown`: the word for how the link is down and the line that says why.
    LinkDown { word: &'static str, line: String },
    /// The thread is not running, over this line.
    Thread { line: String },
}

/// The form for the pane's state over the workspace's daemon (`paneOf` with the pane's channel), and the pane's own line,
/// in today's order: a computer the host can start, a link that is down, else the thread's line over the computer's
/// sentence or the pane's.
pub fn form(pane: &PaneState, line: &str) -> Form {
    match pane {
        PaneState::Absent(Absent { said, start: Some(start), .. }) => Form::DaemonDown { said: said.clone(), start: start.clone() },
        PaneState::Absent(absent) => Form::Thread { line: absent.sentence.clone() },
        pane => match link_down(pane) {
            Some((word, line)) => Form::LinkDown { word, line },
            None => Form::Thread { line: line.to_owned() },
        },
    }
}

/// `linkDownWord` and `linkDownLine`: what turned the link away in its own words, else the pane's title. The machine's
/// own states and a link inside its first-answer bound are not down.
pub fn link_down(pane: &PaneState) -> Option<(&'static str, String)> {
    let word = match pane {
        PaneState::Reconnecting { .. } => words::RECONNECTING,
        PaneState::Unanswered { .. } => words::UNREACHABLE,
        PaneState::Refused { .. } | PaneState::Reauth { .. } => words::REFUSED,
        _ => return None,
    };
    let line = match pane {
        PaneState::Refused { reason, .. } if !reason.is_empty() => reason.clone(),
        pane => pane_words::title(pane)?,
    };
    Some((word, line))
}

/// The pane's channel as a terminal's link words it: one that was open and went is reconnecting; one never open is
/// starting until the first-answer bound has passed, then has not answered. The port's link holds no refusal's kind,
/// so it never reads refused.
pub fn socket(live_once: bool, waited: bool) -> Status {
    match (live_once, waited) {
        (true, _) => Status::Connecting { since: Instant::now() },
        (false, true) => Status::Unanswered,
        (false, false) => Status::Opening,
    }
}

/// A selector naming what an element says, so a test reads the words drawn.
fn says(tag: &'static str, text: &str) -> impl Fn() -> String + 'static {
    let text = text.to_owned();
    move || format!("{tag}:{text}")
}

/// The form inside today's `Empty`; the computer-absent form is the computer's `DaemonDown`, which `daemon_down` draws
/// over the sentence.
pub fn view(form: &Form, daemon_down: impl FnOnce(&str, &str) -> AnyElement, window: &Window, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let inset = if window.viewport_size().width >= MD { px(48.) } else { px(24.) };
    let empty = v_flex()
        .debug_selector(|| "changes-not-running".into())
        .flex_1()
        .min_w_0()
        .items_center()
        .justify_center()
        .gap(px(24.))
        .p(inset)
        .text_center();
    let header = |title: SharedString, line: String| {
        v_flex()
            .debug_selector(|| "not-running-head".into())
            .max_w(px(384.))
            .items_center()
            .child(
                div()
                    .debug_selector(says("not-running-title", &title))
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .text_color(p.muted_foreground)
                    .child(title),
            )
            .child(
                div()
                    .debug_selector(says("not-running-line", &line))
                    .mt(px(4.))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(p.muted_foreground)
                    .child(line),
            )
    };
    match form {
        Form::DaemonDown { said, start } => empty.child(
            v_flex().debug_selector(says("daemon-down-said", said)).flex_1().w_full().items_center().child(daemon_down(said, start)),
        ),
        Form::LinkDown { word, line } => empty.child(header((*word).into(), line.clone())),
        Form::Thread { line } => empty.child(header(words::NOT_RUNNING.into(), line.clone())),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::screens::terminal::state::{input_of, pane_state};

    #[test]
    fn a_daemon_this_host_runs_draws_its_start_before_the_link_and_the_thread() {
        let stopped = pane_words::own_daemon_down("Mac");
        assert_eq!(
            form(&PaneState::Absent(stopped), words::READ_ON_COMPUTER),
            Form::DaemonDown { said: "Mac's terminals and files stopped".into(), start: "Start again".into() }
        );
        let away = pane_words::absent_computer("box");
        assert_eq!(
            form(&PaneState::Absent(away), words::READ_ON_COMPUTER),
            Form::Thread { line: "box is not answering; it connects on its own when it is on".into() },
            "the computer's sentence"
        );
        let refused = PaneState::Refused { reason: "the host's sentence".into(), computer: None };
        assert_eq!(form(&refused, words::READ_ON_COMPUTER), Form::LinkDown { word: words::REFUSED, line: "the host's sentence".into() });
        let starting = PaneState::Starting { local: true, where_: "Mac".into() };
        assert_eq!(form(&starting, words::READ_ON_COMPUTER), Form::Thread { line: words::READ_ON_COMPUTER.into() }, "else the pane's line");
    }

    #[test]
    fn the_link_reads_down_off_the_workspace_and_the_panes_channel() {
        let running = |live_once, waited| {
            let record = json!({ "kind": "local", "name": "repo", "phase": "running" });
            let status = json!({ "phase": "running", "reach": { "state": "reachable" } });
            pane_state(&input_of(&record, Some(&status), Some("Mac"), socket(live_once, waited), None))
        };
        assert_eq!(link_down(&running(true, false)), Some((words::RECONNECTING, "Reconnecting to Mac".into())));
        assert_eq!(link_down(&running(false, true)), Some((words::UNREACHABLE, "Nothing has answered on Mac".into())), "past the bound");
        assert_eq!(link_down(&running(false, false)), None, "inside the first-answer bound the link is starting, not down");
        let refused = PaneState::Refused { reason: String::new(), computer: None };
        assert_eq!(
            link_down(&refused),
            Some((words::REFUSED, "The connection to the thread's computer was refused; its threads keep running".into()))
        );
        let napping = json!({ "kind": "cloud", "name": "repo", "phase": "napping" });
        assert_eq!(link_down(&pane_state(&input_of(&napping, None, None, socket(true, true), None))), None, "the machine's own state");
    }

    #[test]
    fn a_blank_refused_start_asks_for_another_try() {
        use crate::screens::computer::daemon_down::start_refused_line;
        use wsp_host_client::Failure;
        assert_eq!(start_refused_line(&Failure::new(" ")), pane_words::NO_REASON, "a blank refusal is no reason, not \". Start again.\"");
    }
}
