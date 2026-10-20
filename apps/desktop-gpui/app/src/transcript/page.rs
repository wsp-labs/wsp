// SPDX-License-Identifier: AGPL-3.0-only
//! A subagent's page, and the slate lines under the replies.

use std::collections::{HashMap, HashSet};

use crate::words::{tool_field, tool_input};
use wsp_host_client::protocol::SessionEvent;

use super::{
    derive::{derive, Opened},
    model::sub_word,
    prompt::prompt_row,
    Body, Can, Model, Row, RowKind, Splice, Sub, SubState, Transcript,
};

/// One subagent's page off the lead's transcript (`subagentEntries` in adapt/session.ts).
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub call: String,
    pub title: String,
    /// running, done, failed or stopped: the listing's word where it holds one, else the fold's.
    pub state: String,
    pub started: Option<f64>,
    pub ended: Option<f64>,
    pub failure: Option<String>,
    pub model: Option<String>,
    pub harness: Option<String>,
}

impl Transcript {
    /// The turns the agent wrote the slate in: a person's write and the host's draw no line (adapt/session.ts).
    pub(super) fn slated(&self) -> HashSet<String> {
        self.events
            .iter()
            .filter_map(|e| match e {
                SessionEvent::SessionSlate { turn_id: Some(turn), by, .. } => {
                    (serde_json::to_value(by).ok()?.as_str() == Some("agent")).then(|| turn.clone())
                }
                _ => None,
            })
            .collect()
    }

    /// Shows one subagent's page in place of the thread's own rows, by the call that launched it; None shows the thread.
    pub fn page(&mut self, call: Option<String>) -> Option<Splice> {
        if self.around.page == call {
            return None;
        }
        self.around.page = call;
        self.refold_all()
    }

    /// What the page's bar and crumbs say: the subagent's title and state, its model and its agent; None where neither
    /// the transcript nor the listing knows the call (ChatView then opens the lead). The fold's own state and times
    /// come first, as they follow the run event by event where the listing is read only now and then.
    pub fn page_of(&self, call: &str) -> Option<Page> {
        let listed = self.around.listing.as_ref().and_then(|l| l.subagent(call));
        let sub = self.sub_of(call);
        if listed.is_none() && sub.is_none() {
            return None;
        }
        let fold_word = sub.map(|s| sub_word(s.state).to_owned());
        Some(Page {
            call: call.to_owned(),
            title: listed.map(|l| l.title.clone()).or_else(|| sub.map(|s| s.title.clone())).unwrap_or_default(),
            state: fold_word.or_else(|| listed.map(|l| l.state.clone())).unwrap_or_else(|| "running".to_owned()),
            started: sub.and_then(|s| s.started).or_else(|| listed.map(|l| l.started)),
            ended: sub.and_then(|s| s.ended).or_else(|| listed.and_then(|l| l.ended)),
            failure: listed.and_then(|l| l.failure.clone()),
            model: listed.and_then(|l| l.model.clone()),
            harness: self.around.listing.as_ref().and_then(|l| l.harness.clone()).or_else(|| self.harness()),
        })
    }

    fn sub_of(&self, call: &str) -> Option<&Sub> {
        self.model.entries.iter().find_map(|e| match &e.body {
            Body::Subagent(s) if s.parent == call => Some(&**s),
            _ => None,
        })
    }

    /// A subagent's own lines folded as a thread's are, so its calls group and open as the thread's own do: its
    /// deltas, out from under the call that launched it, in one turn of its own that is settled and open, then cut to
    /// its replies, notes and calls, with no turn fold, working row or stop line of its own. A run that said nothing
    /// itself, or was stopped, keeps the launching call's answer as its last line (`subagentEntries`).
    fn page_lines(&self, call: &str, sub: &Sub) -> Vec<Row> {
        let turn = format!("page:{call}");
        let mut events: Vec<SessionEvent> = Vec::new();
        let ids = self.events.first().and_then(|e| serde_json::to_value(e).ok()).unwrap_or_default();
        let id = |k: &str| ids.get(k).cloned().unwrap_or_else(|| serde_json::json!(""));
        let frame = |kind: &str, extra: serde_json::Value| {
            let mut v = serde_json::json!({
                "type": kind, "workspaceId": id("workspaceId"), "sessionId": id("sessionId"), "threadId": id("threadId"), "turnId": turn, "pos": 1,
            });
            if let (Some(o), Some(extra)) = (v.as_object_mut(), extra.as_object()) {
                o.extend(extra.clone());
            }
            serde_json::from_value::<SessionEvent>(v).ok()
        };
        events.extend(frame("session.start", serde_json::json!({})));
        for event in &self.events {
            let SessionEvent::SessionDelta { parent_tool_use_id: Some(parent), .. } = event else { continue };
            if parent != call {
                continue;
            }
            let Ok(mut v) = serde_json::to_value(event) else { continue };
            if let Some(o) = v.as_object_mut() {
                o.remove("parentToolUseId");
                o.insert("turnId".to_owned(), serde_json::json!(turn));
            }
            events.extend(serde_json::from_value::<SessionEvent>(v).ok());
        }
        let spoke = sub.lines.iter().any(|l| !l.mono && !l.label.is_empty() && !l.id.starts_with("answer:"));
        let answer =
            (sub.state == SubState::Stopped || !spoke).then(|| sub.lines.iter().rev().find(|l| l.id.starts_with("answer:"))).flatten();
        if let Some(line) = answer {
            events.extend(frame("session.delta", serde_json::json!({ "kind": "text", "text": line.label })));
        }
        events.extend(frame("session.done", serde_json::json!({ "result": { "status": "completed" } })));
        events.extend(frame("session.end", serde_json::json!({ "exitCode": 0, "sawResult": true })));
        let mut model = Model::default();
        for (ix, event) in events.iter().enumerate() {
            model.add(ix, event);
        }
        let mut ids = self.opened.clone();
        ids.insert(turn.clone());
        let opened = Opened { ids: &ids, shut: &self.shut, all: false, can: Can::default(), source: None, listing: None };
        let rows = derive(&model, 0, 0, opened).map(|(rows, _)| rows).unwrap_or_default();
        rows.into_iter()
            .filter(|r| {
                matches!(
                    r.kind,
                    RowKind::Agent { .. } | RowKind::Group { .. } | RowKind::Calls { .. } | RowKind::Work { .. } | RowKind::Live { .. }
                )
            })
            .map(|mut r| {
                if let RowKind::Agent { rewind, fork, .. } = &mut r.kind {
                    (*rewind, *fork) = (None, None);
                }
                r.turn = None;
                r
            })
            .collect()
    }

    /// The page's rows: what it was asked as the person's message, its text as replies, its calls as rows, and its
    /// prompts last; no turn of its own to fold, count or close.
    pub(super) fn page_rows(&self, call: &str) -> Vec<Row> {
        let asked = self
            .events
            .iter()
            .find_map(|e| match e {
                SessionEvent::SessionDelta { tool_use_id: Some(id), text, .. } if id == call => {
                    tool_input(text).and_then(|f| tool_field(&f, "prompt").map(str::to_owned))
                }
                _ => None,
            })
            .or_else(|| self.around.listing.as_ref().and_then(|l| l.subagent(call)).and_then(|s| s.asked.clone()));
        let mut rows = Vec::new();
        if let Some(asked) = asked {
            let kind = RowKind::Person { at: None, files: Vec::new(), fork: None, full: false, steered: false };
            rows.push(Row { key: format!("{call}:asked"), kind, text: asked, turn: None });
        }
        let Some(sub) = self.sub_of(call) else { return rows };
        rows.extend(self.page_lines(call, sub));
        rows.extend(sub.prompts.iter().map(|p| prompt_row(&format!("permission:{}", p.ask), None, p, None)));
        rows
    }
}

/// "Updated the slate" under the last reply of each turn the agent wrote the slate in, once however many writes it
/// made, after the files that turn changed (`slatedMessageIds` in ChatView.tsx, `SlateUpdatedLine`).
pub(super) fn slate_lines(mut rows: Vec<Row>, mut split: Option<usize>, slated: &HashSet<String>) -> (Vec<Row>, Option<usize>) {
    if slated.is_empty() {
        return (rows, split);
    }
    let mut last: HashMap<&str, usize> = HashMap::new();
    for (ix, row) in rows.iter().enumerate() {
        if let (RowKind::Agent { .. }, Some(turn)) = (&row.kind, row.turn.as_deref()) {
            if slated.contains(turn) {
                last.insert(turn, ix);
            }
        }
    }
    let mut at: Vec<(usize, String)> = last
        .into_iter()
        .map(|(turn, ix)| {
            let after = match rows.get(ix + 1) {
                Some(Row { kind: RowKind::ChangedFiles { .. }, turn: t, .. }) if t.as_deref() == Some(turn) => ix + 2,
                _ => ix + 1,
            };
            (after, turn.to_owned())
        })
        .collect();
    at.sort_by_key(|a| std::cmp::Reverse(a.0));
    for (ix, turn) in at {
        let row = Row { key: format!("slate:{turn}"), kind: RowKind::Slate, text: SLATE_UPDATED.to_owned(), turn: Some(turn) };
        rows.insert(ix, row);
        split = split.map(|s| if s >= ix { s + 1 } else { s });
    }
    (rows, split)
}

/// `SlateUpdatedLine`'s words.
pub const SLATE_UPDATED: &str = "Updated the slate";
