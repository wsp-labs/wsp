// SPDX-License-Identifier: AGPL-3.0-only
//! What the transcript lays over its rows from outside its own events.

use super::{
    prompt::prompt_row, waiting_asker_line, Glyph, Listing, Prompt, Row, RowKind, Splice, StepState, Tone, Transcript, PAUSED_KEY,
};

impl Transcript {
    /// Takes what the thread listing says; the rows read off it are folded again.
    pub fn listing(&mut self, listing: Listing) -> Option<Splice> {
        if self.around.listing.as_ref() == Some(&listing) {
            return None;
        }
        self.around.listing = Some(listing);
        self.refold_all()
    }

    /// The listing the transcript reads, once one has arrived.
    pub fn listed(&self) -> Option<&Listing> {
        self.around.listing.as_ref()
    }
}

/// What the transcript keeps from outside its own events.
#[derive(Clone, Debug, Default)]
pub(super) struct Around {
    pub(super) listing: Option<Listing>,
    /// The subagent whose page the rows are, by the call that launched it.
    pub(super) page: Option<String>,
}

/// The rule line's own words: `forkedFromLine` in packages/protocol/src/words/thread.ts.
pub fn forked_from_line(title: &str) -> String {
    format!("Forked from {title}")
}

impl Around {
    /// Lays what the listing and the window know over the rows the events folded into: the fork rule's title and Open,
    /// no Rewind on a capped turn, each child's state, the borrowed question, the start that failed, and the foot.
    /// `split` counts the rows before the latest turn and moves with any row taken out before it.
    pub(super) fn dress(&self, mut rows: Vec<Row>, mut split: Option<usize>, whole: bool, shut: bool) -> (Vec<Row>, Option<usize>) {
        let listing = self.listing.as_ref();
        let mut ix = 0;
        while ix < rows.len() {
            match &mut rows[ix].kind {
                RowKind::Fork { source, open } => match listing.map(|l| l.source.as_ref()) {
                    Some(Some(from)) => {
                        *source = from.title.clone();
                        *open = from.at.clone();
                        rows[ix].text = forked_from_line(&from.title);
                    }
                    // As today, no rule stands until the listing names the thread the fork came from.
                    _ => {
                        rows.remove(ix);
                        split = split.map(|s| if s > ix { s - 1 } else { s });
                        continue;
                    }
                },
                RowKind::Agent { rewind, .. } if listing.is_some_and(|l| l.capped) => *rewind = None,
                RowKind::Spawn { children } => {
                    for child in children.iter_mut() {
                        child.listed = child.thread.as_deref().and_then(|t| listing?.child(t)).cloned();
                        child.workspace = listing.and_then(|l| l.workspace.clone());
                    }
                }
                _ => {}
            }
            ix += 1;
        }
        let Some(listing) = listing else { return self.foot(rows, split, shut) };
        if let Some(start) = &listing.start_failed {
            // `machineStartAt`: after the person's first message at or after the start began, with some slack; a start
            // still under way or failed with no such message stands last, a finished one is not drawn.
            let person =
                rows.iter().position(|r| matches!(r.kind, RowKind::Person { at, .. } if at.is_none_or(|at| at >= start.since - 10_000.)));
            let answered = person.and_then(|p| {
                rows[p + 1..].iter().find_map(|r| match r.kind {
                    RowKind::Agent { at, .. } => at,
                    _ => None,
                })
            });
            let mut start = start.clone();
            if let (Some(ready), Some(answered)) = (start.ready, answered) {
                start.took = Some((answered - start.since).max(0.));
                if let Some(agent) = start.steps.iter_mut().find(|s| s.id == "agent") {
                    (agent.state, agent.ms, agent.since) = (StepState::Done, Some((answered - ready).max(0.)), None);
                }
            }
            let at = person.map_or(rows.len(), |p| p + 1);
            let stands = person.is_some() || start.failed || start.ready.is_none();
            if stands && (whole || at >= rows.len()) {
                let text = match start.took {
                    _ if start.failed => format!("Could not start a machine on {}", start.on),
                    None => format!("Starting a machine on {}", start.on),
                    Some(took) => format!("Started on {} in {}", start.on, crate::words::fmt_duration(took)),
                };
                rows.insert(
                    at.min(rows.len()),
                    Row { key: "start-failed".to_owned(), kind: RowKind::StartFailed { failed: start }, text, turn: None },
                );
                split = split.map(|s| if s >= at { s + 1 } else { s });
            }
        }
        if let Some((title, prompt)) = &listing.waiting_on {
            let working = rows.iter().any(|r| matches!(r.kind, RowKind::Working { .. }));
            if working {
                rows.retain(|r| !matches!(r.kind, RowKind::Thinking));
                for row in rows.iter_mut() {
                    if let RowKind::Working { waiting, .. } = &mut row.kind {
                        *waiting = true;
                        row.text = "Waiting for you".to_owned();
                    }
                }
                let text = |k: &str| prompt.pointer(k).and_then(|x| x.as_str()).map(str::to_owned);
                let options = prompt
                    .pointer("/prompt/options")
                    .and_then(|o| o.as_array())
                    .map(|o| {
                        o.iter()
                            .filter_map(|o| {
                                let s = |k: &str| o.get(k).and_then(|x| x.as_str()).map(str::to_owned);
                                Some((s("id")?, s("label")?, s("effect")?, s("mode")))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if let (Some(ask), Some(session), Some(tool), Some(thread)) =
                    (text("/prompt/askId"), text("/sessionId"), text("/prompt/toolName"), text("/threadId"))
                {
                    let borrowed = Prompt {
                        ask: ask.clone(),
                        session,
                        tool,
                        input: text("/prompt/input").unwrap_or_default(),
                        detail: text("/prompt/detail"),
                        options,
                        outcome: None,
                        picked: None,
                    };
                    rows.push(prompt_row(&format!("waiting-on:{thread}:{ask}"), None, &borrowed, Some(waiting_asker_line(title))));
                }
            }
        }
        if let Some(wait) = &listing.wait {
            for row in rows.iter_mut() {
                match &mut row.kind {
                    RowKind::Working { wait: at, .. } => *at = Some(wait.clone()),
                    // Nothing thinks on a machine that is not running; the row keeps its room.
                    RowKind::Thinking => row.text.clear(),
                    _ => {}
                }
            }
        }
        self.foot(rows, split, shut)
    }

    /// The rows under the transcript: the send's starting notice, the Threads section and the machine line.
    fn foot(&self, mut rows: Vec<Row>, split: Option<usize>, shut: bool) -> (Vec<Row>, Option<usize>) {
        // The settled turn's word stands under the Threads section, as ChatView's foot orders them.
        let word = rows.last().filter(|r| matches!(r.kind, RowKind::Footer { .. })).is_some().then(|| rows.pop()).flatten();
        if let Some(text) = self.listing.as_ref().and_then(|l| l.sent.as_ref()) {
            let kind = RowKind::Person { at: None, files: Vec::new(), fork: None, full: false, steered: false };
            rows.push(Row { key: "sent".to_owned(), kind, text: text.clone(), turn: None });
        }
        if let Some(line) = self.listing.as_ref().and_then(|l| l.starting.as_ref()) {
            rows.push(Row {
                key: "starting".to_owned(),
                kind: RowKind::Work { tone: Tone::Notice, glyph: Glyph::Info, body: None, open: false },
                text: line.clone(),
                turn: None,
            });
        }
        let children = self.listing.as_ref().map(|l| l.children.clone()).unwrap_or_default();
        // The section stands for the threads this one opened; its agent's subagents stand among them, never alone.
        if children.iter().any(|c| c.subagent.is_none()) {
            rows.push(Row {
                key: "threads".to_owned(),
                kind: RowKind::Threads { open: !shut, children },
                text: "Threads".to_owned(),
                turn: None,
            });
        }
        rows.extend(word);
        let working = rows.iter().any(|r| matches!(r.kind, RowKind::Working { .. }));
        if let Some(word) = self.listing.as_ref().and_then(|l| l.paused.as_ref()).filter(|_| !working) {
            rows.push(Row { key: PAUSED_KEY.to_owned(), kind: RowKind::Rule, text: word.clone(), turn: None });
        }
        if let Some(line) = self.listing.as_ref().and_then(|l| l.machine.as_ref()) {
            rows.push(Row { key: "machine".to_owned(), kind: RowKind::Rule, text: line.clone(), turn: None });
        }
        (rows, split)
    }
}
