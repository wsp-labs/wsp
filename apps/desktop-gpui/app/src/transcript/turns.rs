// SPDX-License-Identifier: AGPL-3.0-only
//! What the transcript says of its turns: rewind facts, context figures, the docked prompt.

use wsp_host_client::protocol::SessionEvent;

use super::{prompt::prompt_row, Body, Entry, Prompt, Row, State, Transcript};

/// What Rewind to here's dialog says about one earlier turn (`RewindRequest` in shellRequests.ts).
#[derive(Clone, Debug, PartialEq)]
pub struct RewindFacts {
    pub turns_after: usize,
    pub files: bool,
    pub cuts_conversation: bool,
    pub kept: Option<String>,
    pub harness: Option<String>,
}

impl Transcript {
    /// The facts of the turn `turn`, while the transcript holds it.
    pub(crate) fn rewind_facts(&self, turn: &str) -> Option<RewindFacts> {
        let turns = &self.model.turns;
        let at = turns.iter().position(|t| t.id == turn)?;
        let checkpoint = |id: &str| {
            self.events.iter().rev().find_map(|e| match e {
                SessionEvent::SessionCheckpoint { turn_id, kept, .. } if turn_id.as_deref() == Some(id) => Some(kept.clone()),
                _ => None,
            })
        };
        let kept = turns.iter().filter(|t| t.kept.as_ref().is_some_and(|k| k.kept)).find_map(|t| checkpoint(&t.id).flatten());
        Some(RewindFacts {
            turns_after: turns.len() - 1 - at,
            files: turns[at].kept.as_ref().is_some_and(|k| k.files),
            cuts_conversation: self.can.rewinds_conversation && kept.is_none(),
            kept,
            harness: self.harness(),
        })
    }
}

/// One turn's context figures as TurnSummary's `tokens` and `held` carry them; `counted` where its reply carried tokens.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TurnContext {
    pub running: bool,
    pub failed: bool,
    pub counted: bool,
    pub tokens_context: Option<f64>,
    pub tokens_window: Option<f64>,
    pub held_context: Option<f64>,
    pub held_window: Option<f64>,
}

impl Transcript {
    /// Each turn's context figures, oldest first, for the header's context ring.
    pub fn turn_contexts(&self) -> Vec<TurnContext> {
        self.model
            .turns
            .iter()
            .map(|t| TurnContext {
                running: t.state == State::Running,
                failed: t.state == State::Failed,
                counted: t.tokens.is_some(),
                tokens_context: t.tokens.and_then(|(context, _)| context),
                tokens_window: t.tokens.and_then(|(_, window)| window),
                held_context: t.held.map(|(context, _)| context),
                held_window: t.held.and_then(|(_, window)| window),
            })
            .collect()
    }

    /// The harness the thread's latest turn ran on, where its start named one.
    pub fn harness(&self) -> Option<String> {
        self.model.agent.clone()
    }

    pub fn event_list(&self) -> &[SessionEvent] {
        &self.events
    }
    /// The prompt the latest turn waits on where the thread's own agent raised it and nobody has answered
    /// (`openPrompt` in WorkspaceThread.tsx): the dock's, whose row keeps only the waiting record.
    pub fn docked(&self) -> Option<Docked<'_>> {
        let (e, p) = self.waiting()?;
        Some(Docked {
            row: prompt_row(&e.key, e.turn.clone(), p, None),
            tool: &p.tool,
            input: &p.input,
            detail: p.detail.as_deref(),
            cwd: self.model.cwd.as_deref(),
        })
    }

    /// The key of the row [`Transcript::docked`] answers, without drawing it.
    pub fn docked_key(&self) -> Option<&str> {
        self.waiting().map(|(e, _)| e.key.as_str())
    }

    /// The latest turn's first unanswered prompt, read off the entries from that turn's opening on where the fold
    /// marked it.
    fn waiting(&self) -> Option<(&Entry, &Prompt)> {
        let latest = self.model.turns.last()?.id.as_str();
        let from = self.mark.as_ref().map_or(0, |m| m.entries.min(self.model.entries.len()));
        #[cfg(test)]
        self.looked.set(self.looked.get() + self.model.entries.len() - from);
        self.model.entries[from..].iter().find_map(|e| match &e.body {
            Body::Prompt(p) if p.outcome.is_none() && e.turn.as_deref() == Some(latest) => Some((e, &**p)),
            _ => None,
        })
    }
}

/// The prompt the composer's dock answers: its row, the call it asks about, and the thread's folder.
pub struct Docked<'a> {
    pub row: Row,
    pub tool: &'a str,
    pub input: &'a str,
    pub detail: Option<&'a str>,
    pub cwd: Option<&'a str>,
}
