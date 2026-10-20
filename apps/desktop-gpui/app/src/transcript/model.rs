// SPDX-License-Identifier: AGPL-3.0-only
//! The fold of a thread's events into entries and turn summaries (createSessionFold).

use std::collections::{HashMap, HashSet};
use std::num::NonZeroU64;

use crate::screens::app_shell::title::{cut_line, title_line};
use crate::words::{compacted_line, server_tool, tool_activity_line, tool_done_line, tool_field, tool_input, waited_line};
use wsp_host_client::protocol::{SessionEvent, SessionEventAttachmentsItem, SessionEventKind, SessionEventResultStatus, SessionEventWho};

use super::{
    facts::{compact_lines, facts, preview, unified},
    groups::{display_label, work_of},
    Asked, Body, Build, ChangedFile, Changes, Entry, Far, FileTile, FoldLine, Kept, Kid, Mark, Model, Outcome, Prompt, PromptOption, Ran,
    Retry, Source, State, Status, Sub, SubState, Summary, Tone, Tool, Work, AFTER_CUT, INTERNAL_RESULT, RESUMED,
};

/// The fields every event carries: its position, its stamp, its turn and its session.
fn head(event: &SessionEvent) -> (Option<u64>, Option<f64>, Option<&str>, &str) {
    use SessionEvent as E;
    match event {
        E::SessionStart { pos, at, turn_id, session_id, .. }
        | E::SessionDelta { pos, at, turn_id, session_id, .. }
        | E::SessionDone { pos, at, turn_id, session_id, .. }
        | E::SessionEnd { pos, at, turn_id, session_id, .. }
        | E::SessionSteer { pos, at, turn_id, session_id, .. }
        | E::SessionNotify { pos, at, turn_id, session_id, .. }
        | E::SessionPermission { pos, at, turn_id, session_id, .. }
        | E::SessionPermissionClosed { pos, at, turn_id, session_id, .. }
        | E::SessionCheckpoint { pos, at, turn_id, session_id, .. }
        | E::SessionChanges { pos, at, turn_id, session_id, .. }
        | E::SessionPlan { pos, at, turn_id, session_id, .. }
        | E::SessionCompacted { pos, at, turn_id, session_id, .. }
        | E::SessionRetry { pos, at, turn_id, session_id, .. }
        | E::SessionContext { pos, at, turn_id, session_id, .. }
        | E::SessionRun { pos, at, turn_id, session_id, .. }
        | E::SessionMoved { pos, at, turn_id, session_id, .. }
        | E::SessionBehind { pos, at, turn_id, session_id, .. }
        | E::SessionNotice { pos, at, turn_id, session_id, .. }
        | E::SessionEarlier { pos, at, turn_id, session_id, .. }
        | E::SessionCapped { pos, at, turn_id, session_id, .. }
        | E::SessionUncapped { pos, at, turn_id, session_id, .. }
        | E::SessionSubagent { pos, at, turn_id, session_id, .. }
        | E::SessionSlate { pos, at, turn_id, session_id, .. }
        | E::ThreadWaited { pos, at, turn_id, session_id, .. } => (pos.map(NonZeroU64::get), *at, turn_id.as_deref(), session_id),
        E::Unknown => (None, None, None, ""),
    }
}

impl Model {
    fn checkpoint(&mut self, event: usize) {
        self.tangled = false;
        self.latest = Some(Mark {
            event,
            entries: self.entries.len(),
            turns: self.turns.len(),
            seen: self.order.len(),
            starts: self.starts.clone(),
            earlier: self.earlier,
            cwd: self.cwd.clone(),
            rows: 0,
            working: false,
            retry: self.retry.clone(),
        });
    }

    pub(super) fn working(&self) -> bool {
        self.turns.last().is_some_and(|t| t.state == State::Running)
    }

    /// An event that changes an entry or a turn summary the latest turn's opening left behind it.
    fn reach(&mut self, entry: Option<usize>, turn: Option<usize>) {
        let Some(mark) = self.latest.as_ref().filter(|_| self.folding >= self.fresh) else { return };
        if entry.is_some_and(|ix| ix < mark.entries) || turn.is_some_and(|ix| ix < mark.turns) {
            self.tangled = true;
        }
    }

    /// An event of a turn other than the open one.
    fn elsewhere(&mut self, turn_id: Option<&str>) {
        let other = turn_id.is_none() || turn_id != self.turn.as_ref().map(|t| t.id.as_str());
        if self.folding >= self.fresh && other {
            self.tangled = true;
        }
        if other {
            let at = turn_id.and_then(|id| self.turns.iter().rposition(|t| t.id == id));
            self.far(at.map_or(Far::Unheld, Far::Turn));
        }
    }

    /// Notes an event that reaches back past the open turn's opening, which a page laid under it has to fold again.
    fn far(&mut self, back: Far) {
        let before = match (&self.latest, back) {
            (Some(m), Far::Entry(ix)) => ix < m.entries,
            (Some(m), Far::Turn(ix)) => ix < m.turns,
            (Some(m), Far::Event(ix)) => ix < m.event,
            _ => true,
        };
        if before {
            self.far.push((self.folding, back));
        }
    }

    pub(super) fn restore(&mut self, mark: &Mark) {
        self.entries.truncate(mark.entries);
        self.turns.truncate(mark.turns);
        for id in self.order.drain(mark.seen..) {
            self.seen.remove(&id);
        }
        self.starts = mark.starts.clone();
        self.earlier = mark.earlier;
        self.cwd = mark.cwd.clone();
        self.retry = mark.retry.clone();
        self.prompts.retain(|_, ix| *ix < mark.entries);
        self.runs.retain(|_, run| run.event < mark.event);
        self.kids.retain(|_, kid| kid.event < mark.event);
        self.far.retain(|(event, _)| *event < mark.event);
        self.turn = None;
        self.tangled = false;
        self.latest = None;
    }

    fn note_id(&mut self, id: &str) {
        if self.seen.insert(id.to_owned()) {
            self.order.push(id.to_owned());
        } else {
            self.tangled = true;
        }
    }

    fn open(&mut self, id: String, at: Option<f64>, copied: bool) {
        self.note_id(&id);
        self.turns.push(Summary {
            id: id.clone(),
            state: State::Running,
            started: at,
            completed: None,
            duration_ms: None,
            waited_ms: None,
            error: None,
            limit: false,
            unreached: false,
            copied,
            kept: None,
            changes: None,
            tokens: None,
            held: None,
        });
        self.turn = Some(Build {
            id,
            index: self.turns.len() - 1,
            ordinal: 0,
            open: None,
            open_id: None,
            saw_text: false,
            tools: HashMap::new(),
            anon: None,
            reply: None,
            held: HashSet::new(),
            subagents: HashMap::new(),
            child_calls: HashMap::new(),
            plan: None,
            keys: HashMap::new(),
        });
    }

    /// turnFor: the turn an event lands in, opened here when history starts in the middle of one.
    fn turn_for(&mut self, ix: usize, turn_id: Option<&str>, session: &str, at: Option<f64>) {
        if let Some(t) = &self.turn {
            if turn_id.is_none_or(|id| id == t.id) {
                return;
            }
        }
        self.end_running(at);
        self.checkpoint(ix);
        self.open(turn_id.map_or_else(|| format!("{session}#0"), str::to_owned), at, false);
    }

    fn running(&self) -> bool {
        self.turn.as_ref().is_some_and(|t| self.turns[t.index].state == State::Running)
    }

    /// endRunningTurn: a running turn a new one supersedes keeps its reply's status, or failed if it never replied.
    fn end_running(&mut self, at: Option<f64>) {
        if !self.running() {
            return;
        }
        match self.turn.as_ref().and_then(|t| t.reply) {
            Some(status) => self.set_state(status.into()),
            None => self.finish(&Outcome::failed("session restarted before it finished".to_owned()), at),
        }
    }

    fn set_state(&mut self, state: State) {
        if let Some(t) = &self.turn {
            self.turns[t.index].state = state;
        }
    }

    fn finish(&mut self, outcome: &Outcome, at: Option<f64>) {
        self.apply_result(outcome, at);
        self.set_state(outcome.status.into());
    }

    fn apply_result(&mut self, outcome: &Outcome, at: Option<f64>) {
        self.close();
        let t = self.turn.as_ref().expect("a result lands in a turn");
        for tool in t.tools.values() {
            if let Body::Work(w) = &mut self.entries[tool.entry].body {
                if w.status == Some(Status::Running) {
                    w.status = Some(Status::Stopped);
                }
            }
        }
        let folds: Vec<usize> = t.subagents.values().copied().collect();
        for ix in folds {
            if let Body::Subagent(s) = &mut self.entries[ix].body {
                if s.state == SubState::Running {
                    s.state = SubState::Stopped;
                    s.ended = at;
                }
            }
        }
        let t = self.turn.as_ref().expect("a result lands in a turn");
        let (saw_text, index) = (t.saw_text, t.index);
        if outcome.status == SessionEventResultStatus::Completed && !saw_text {
            if let Some(text) = outcome.text.as_ref().filter(|t| !t.is_empty()) {
                self.add_message(Body::Agent { text: text.clone(), streaming: false, updated: at }, at);
            }
        }
        if outcome.status == SessionEventResultStatus::Failed && !outcome.limit {
            if let Some(error) = outcome.error.as_ref().filter(|e| !killed_for_memory(e)) {
                self.add_work(Work::new(error.clone(), Tone::Error, Source::Error), at);
            }
        }
        let summary = &mut self.turns[index];
        summary.duration_ms = outcome.duration_ms;
        summary.waited_ms = outcome.waited_ms;
        summary.error = outcome.error.clone();
        summary.limit = outcome.limit;
        summary.unreached = outcome.unreached;
        summary.tokens = outcome.tokens;
        summary.completed = at;
    }

    fn close(&mut self) {
        let Some(t) = &mut self.turn else { return };
        if let Some(ix) = t.open.take() {
            if let Body::Agent { streaming, .. } = &mut self.entries[ix].body {
                *streaming = false;
            }
        }
        t.open_id = None;
    }

    /// rowId: a row is named by its turn and the event that opened it, or by its call.
    fn key(&mut self, kind: &str, call: Option<&str>) -> String {
        let t = self.turn.as_mut().expect("a keyed entry lands in a turn");
        t.ordinal += 1;
        let Some(place) = self.place else { return format!("{}:{kind}{}", t.id, t.ordinal) };
        let base = match call {
            Some(call) => format!("{}:call:{call}", t.id),
            None => format!("{}:{kind}@{place}", t.id),
        };
        let n = t.keys.entry(base.clone()).or_insert(0);
        let key = if *n == 0 { base } else { format!("{base}.{n}") };
        *n += 1;
        key
    }

    fn add_message(&mut self, body: Body, at: Option<f64>) -> usize {
        let key = self.key("m", None);
        let turn = matches!(body, Body::Agent { .. }).then(|| self.turn.as_ref().map(|t| t.id.clone())).flatten();
        self.entries.push(Entry { key, turn, at, earlier: false, body });
        self.entries.len() - 1
    }

    fn add_work(&mut self, work: Work, at: Option<f64>) -> usize {
        let key = self.key("w", work.call.as_deref());
        let turn = self.turn.as_ref().map(|t| t.id.clone());
        self.entries.push(Entry { key, turn, at, earlier: false, body: Body::Work(Box::new(work)) });
        self.entries.len() - 1
    }

    fn notice(&mut self, label: String, detail: Option<String>, source: Source, at: Option<f64>) {
        let mut work = Work::new(label, Tone::Notice, source);
        work.detail = detail;
        self.add_work(work, at);
    }

    pub(super) fn add(&mut self, ix: usize, event: &SessionEvent) {
        let (pos, at, turn_id, session) = head(event);
        self.place = pos;
        self.folding = ix;
        if let SessionEvent::SessionRetry { attempt, of, why, .. } = event {
            let line = retry_line(&attempt.to_string(), of.as_ref().map(ToString::to_string).as_deref(), why.as_deref());
            self.retry = Some(Retry { turn: turn_id.map(str::to_owned), line });
        } else if turn_id.is_some() && self.retry.as_ref().is_some_and(|r| r.turn.as_deref() == turn_id) && agent_wrote(event) {
            self.retry = None;
        }
        match event {
            SessionEvent::SessionStart { prompt, after_cut, after_limit, attachments, cwd, copied_from, request_id, agent, .. } => {
                if let Some(cwd) = cwd {
                    self.cwd = Some(cwd.clone());
                }
                if agent.is_some() {
                    self.agent.clone_from(agent);
                }
                self.from_start |= ix == 0;
                self.end_running(at);
                self.checkpoint(ix);
                let count = self.starts.get(session).map_or(1, |n| n + 1);
                self.starts.insert(session.to_owned(), count);
                let id = turn_id.map_or_else(|| format!("{session}#{count}"), str::to_owned);
                self.open(id.clone(), at, copied_from.is_some());
                if self.source.is_none() {
                    self.source.clone_from(copied_from);
                }
                if let (Some(prompt), None) = (prompt, after_limit) {
                    let files = tiles(attachments, request_id.as_deref());
                    self.add_message(Body::Person { text: prompt.clone(), files, turn: Some(id), steered: false }, at);
                }
                if *after_cut == Some(true) {
                    self.notice(AFTER_CUT.to_owned(), None, Source::Other, at);
                }
                if after_limit.is_some() {
                    self.notice(RESUMED.to_owned(), None, Source::Other, at);
                }
            }
            SessionEvent::SessionDelta { .. } => {
                self.turn_for(ix, turn_id, session, at);
                self.delta(event, at);
            }
            SessionEvent::SessionSteer { prompt, attachments, request_id, .. } => {
                self.turn_for(ix, turn_id, session, at);
                self.close();
                let (files, turn) = (tiles(attachments, request_id.as_deref()), self.turn.as_ref().map(|t| t.id.clone()));
                self.add_message(Body::Person { text: prompt.clone(), files, turn, steered: true }, at);
            }
            SessionEvent::SessionNotify { notify, text, .. } => {
                self.turn_for(ix, turn_id, session, at);
                self.close();
                let label = if notify == "me" {
                    "told you".to_owned()
                } else {
                    format!("told thread {}", notify.chars().take(8).collect::<String>())
                };
                self.notice(label, Some(text.clone()), Source::Other, at);
            }
            SessionEvent::SessionPermission { ask_id, tool_name, tool_use_id, parent_tool_use_id, input, detail, options, .. } => {
                self.turn_for(ix, turn_id, session, at);
                self.close();
                let t = self.turn.as_mut().expect("turn_for opened a turn");
                match (tool_use_id, parent_tool_use_id) {
                    (Some(call), None) => _ = t.held.insert(call.clone()),
                    (Some(_), Some(_)) => {}
                    (None, _) => t.held.extend(t.tools.keys().cloned()),
                }
                let turn = Some(t.id.clone());
                let prompt = Prompt {
                    ask: ask_id.clone(),
                    session: session.to_owned(),
                    tool: tool_name.clone(),
                    input: input.clone(),
                    detail: detail.clone(),
                    options: options.iter().map(|o| (o.id.clone(), o.label.clone(), o.effect.to_string(), o.mode.clone())).collect(),
                    outcome: None,
                    picked: None,
                };
                if let Some(parent) = parent_tool_use_id {
                    let ix = self.fold_for(parent, at);
                    if let Body::Subagent(s) = &mut self.entries[ix].body {
                        s.prompts.push(prompt);
                    }
                    self.prompts.insert(ask_id.clone(), ix);
                    return;
                }
                self.prompts.insert(ask_id.clone(), self.entries.len());
                let body = Body::Prompt(Box::new(prompt));
                self.entries.push(Entry { key: format!("permission:{ask_id}"), turn, at, earlier: false, body });
            }
            SessionEvent::SessionPermissionClosed { ask_id, outcome, option_id, .. } => {
                let Some(&ix) = self.prompts.get(ask_id) else { return self.far(Far::Unheld) };
                self.reach(Some(ix), None);
                self.far(Far::Entry(ix));
                let close = |p: &mut Prompt| {
                    p.outcome = Some(outcome.to_string());
                    p.picked = option_id.clone();
                };
                match self.entries.get_mut(ix).map(|e| &mut e.body) {
                    Some(Body::Prompt(p)) => close(p),
                    Some(Body::Subagent(s)) => s.prompts.iter_mut().filter(|p| p.ask == *ask_id).for_each(close),
                    _ => {}
                }
            }
            SessionEvent::SessionChanges { files, others, shared, from, to, .. } => {
                let Some(at) = self.turns.iter().position(|t| Some(t.id.as_str()) == turn_id) else { return self.far(Far::Unheld) };
                self.reach(None, Some(at));
                self.far(Far::Turn(at));
                let files = files.iter().map(|f| changed_file(&f.path, f.additions, f.deletions)).collect();
                let others = (!others.is_empty()).then(|| others.iter().map(|f| changed_file(&f.path, f.additions, f.deletions)).collect());
                self.turns[at].changes = Some(Changes { files, others, shared: *shared == Some(true), from: from.clone(), to: to.clone() });
            }
            SessionEvent::SessionCheckpoint { ref_, anchor, kept, .. } => {
                let Some(at) = self.turns.iter().position(|t| Some(t.id.as_str()) == turn_id) else { return self.far(Far::Unheld) };
                self.reach(None, Some(at));
                self.far(Far::Turn(at));
                self.turns[at].kept = Some(Kept { files: ref_.is_some(), anchor: anchor.is_some(), kept: kept.is_some() });
                if kept.is_some() && self.folding >= self.fresh {
                    self.tangled = true;
                }
            }
            SessionEvent::SessionRun { block, run_id, command, state, exit_code, signal, output, pty_id, .. } => {
                self.elsewhere(turn_id);
                let ran = Ran {
                    run_id: run_id.clone(),
                    command: command.clone(),
                    state: state.to_string(),
                    output: output.clone(),
                    exit_code: *exit_code,
                    signal: signal.map(|s| s.to_string()),
                    pty_id: pty_id.clone(),
                    event: ix,
                };
                self.runs.insert(block.clone(), ran);
            }
            SessionEvent::SessionSubagent { task, state, parent_tool_use_id, title, summary, .. } => {
                let Some(parent) = parent_tool_use_id else { return };
                self.elsewhere(turn_id);
                let state = sub_state(&state.to_string());
                match self.kids.get(parent) {
                    Some(kid) => self.far(Far::Event(kid.event)),
                    // An update to a subagent no row named yet may name one an older page holds, unless this turn launched it.
                    None if state != SubState::Running
                        && !self.turn.as_ref().is_some_and(|t| t.tools.contains_key(parent) || t.subagents.contains_key(parent)) =>
                    {
                        self.far(Far::Unheld)
                    }
                    None => {}
                }
                if state == SubState::Running {
                    let held = self.kids.get(parent);
                    let kid = Kid {
                        title: title.clone().or_else(|| held.map(|k| k.title.clone())).unwrap_or_else(|| task.clone()),
                        state,
                        summary: None,
                        turn: turn_id.map(str::to_owned).or_else(|| held.and_then(|k| k.turn.clone())),
                        event: held.map_or(ix, |k| k.event),
                    };
                    self.kids.insert(parent.clone(), kid);
                } else if let Some(kid) = self.kids.get_mut(parent) {
                    kid.state = state;
                    if summary.is_some() {
                        kid.summary = summary.clone();
                    }
                }
            }
            SessionEvent::SessionContext { context, window, .. } => {
                self.turn_for(ix, turn_id, session, at);
                let t = self.turn.as_ref().expect("turn_for opened a turn");
                self.turns[t.index].held = Some((*context, *window));
            }
            SessionEvent::SessionCompacted { before, after, .. } => {
                self.turn_for(ix, turn_id, session, at);
                self.close();
                self.add_work(Work::new(compacted_line(*before, *after), Tone::Compaction, Source::Other), at);
            }
            SessionEvent::SessionPlan { text, .. } => {
                self.turn_for(ix, turn_id, session, at);
                let Some(text) = text else { return };
                let t = self.turn.as_ref().expect("turn_for opened a turn");
                if let Some(ix) = t.plan {
                    self.entries[ix].body = Body::Plan(text.clone());
                    return;
                }
                let (key, turn) = (format!("plan:{}", t.id), Some(t.id.clone()));
                self.close();
                self.entries.push(Entry { key, turn, at, earlier: false, body: Body::Plan(text.clone()) });
                let ix = self.entries.len() - 1;
                if let Some(t) = &mut self.turn {
                    t.plan = Some(ix);
                }
            }
            SessionEvent::SessionDone { result, .. } => {
                self.turn_for(ix, turn_id, session, at);
                let outcome = Outcome::of(result);
                self.apply_result(&outcome, at);
                if let Some(t) = &mut self.turn {
                    t.reply = Some(outcome.status);
                }
            }
            SessionEvent::SessionEnd { reason, saw_result, exit_code, .. } => {
                let current = self.turn.as_ref().map(|t| t.id.as_str());
                if current.is_none() || turn_id.is_some_and(|id| Some(id) != current) {
                    self.end_running(at);
                    if let Some(reason) = reason {
                        if let Some(id) = turn_id {
                            self.note_id(id);
                        }
                        let key = format!("refusal:{}", turn_id.unwrap_or(session));
                        let body = Body::Work(Box::new(Work::new(reason.clone(), Tone::Error, Source::Error)));
                        self.entries.push(Entry { key, turn: turn_id.map(str::to_owned), at, earlier: false, body });
                    }
                    return;
                }
                if !self.running() {
                    return;
                }
                match self.turn.as_ref().and_then(|t| t.reply) {
                    Some(status) => self.set_state(status.into()),
                    None => {
                        let error = reason.clone().unwrap_or_else(|| match (saw_result, exit_code) {
                            (true, _) => "session exited without a result".to_owned(),
                            (false, Some(code)) => format!("session exited without a result (exit code {code})"),
                            (false, None) => "session exited without a result (exit code unknown)".to_owned(),
                        });
                        self.finish(&Outcome::failed(error), at);
                    }
                }
            }
            SessionEvent::SessionNotice { text, .. } => {
                self.turn_for(ix, turn_id, session, at);
                self.close();
                self.notice(text.clone(), None, Source::Notice, at);
            }
            SessionEvent::ThreadWaited { ms, said, .. } => {
                self.turn_for(ix, turn_id, session, at);
                self.close();
                self.notice(said.clone().unwrap_or_else(|| waited_line(*ms as f64)), None, Source::Notice, at);
            }
            SessionEvent::SessionEarlier { who, text, .. } => {
                if !self.turns.is_empty() && self.folding >= self.fresh {
                    self.tangled = true;
                }
                self.earlier += 1;
                let key = format!("earlier:{session}:{}", pos.map_or(format!("n{}", self.earlier), |p| p.to_string()));
                let body = match who {
                    SessionEventWho::Person => Body::Person { text: text.clone(), files: Vec::new(), turn: None, steered: false },
                    SessionEventWho::Agent => Body::Agent { text: text.clone(), streaming: false, updated: at },
                    SessionEventWho::Note | SessionEventWho::Unknown => {
                        Body::Work(Box::new(Work::new(text.clone(), Tone::Notice, Source::Other)))
                    }
                    SessionEventWho::Tool => Body::Work(Box::new(Work::new(text.clone(), Tone::Tool, Source::Other))),
                };
                self.entries.push(Entry { key, turn: None, at, earlier: true, body });
            }
            _ => {}
        }
    }

    /// foldFor: the fold one subagent's lines gather under, which takes the place of the call that launched it.
    fn fold_for(&mut self, parent: &str, at: Option<f64>) -> usize {
        let t = self.turn.as_ref().expect("a subagent's line lands in a turn");
        if let Some(&ix) = t.subagents.get(parent) {
            return ix;
        }
        let call = t.tools.get(parent);
        let launched = call.and_then(|c| work_of(&self.entries[c.entry]).map(|w| (c.entry, w)));
        let title = call
            .and_then(|c| task_line(&c.input))
            .or_else(|| launched.and_then(|(_, w)| w.detail.clone()))
            .or_else(|| launched.map(|(_, w)| w.label.clone()))
            .unwrap_or_else(|| "Subagent".to_owned());
        let answered = call.and_then(|c| c.answered);
        let state = match answered {
            None => SubState::Running,
            Some((_, true)) => SubState::Failed,
            Some(_) => SubState::Done,
        };
        let opened = launched.map_or(at, |(ix, _)| self.entries[ix].at);
        let sub = Sub {
            parent: parent.to_owned(),
            title,
            lines: Vec::new(),
            prompts: Vec::new(),
            state,
            started: opened,
            ended: answered.and_then(|(at, _)| at),
        };
        let entry = Entry {
            key: format!("subagent:{parent}"),
            turn: Some(t.id.clone()),
            at: opened,
            earlier: false,
            body: Body::Subagent(Box::new(sub)),
        };
        let ix = match launched {
            Some((ix, _)) => {
                self.entries[ix] = entry;
                ix
            }
            None => {
                self.entries.push(entry);
                self.entries.len() - 1
            }
        };
        if let Some(t) = &mut self.turn {
            t.subagents.insert(parent.to_owned(), ix);
        }
        ix
    }

    /// addFoldLine: one line inside a fold, or the change a call's result makes to the line its input opened.
    fn fold_line(&mut self, parent: &str, at: Option<f64>, mono: bool, label: Option<String>, detail: Option<String>, key: Option<String>) {
        let ix = self.fold_for(parent, at);
        let Body::Subagent(s) = &mut self.entries[ix].body else { return };
        if let Some(line) = key.as_ref().and_then(|key| s.lines.iter_mut().find(|l| l.id == *key)) {
            line.mono = mono;
            if let Some(label) = label {
                line.label = label;
            }
            if detail.is_some() {
                line.detail = detail;
            }
            return;
        }
        let id = key.unwrap_or_else(|| format!("{}:l{}", s.parent, s.lines.len()));
        s.lines.push(FoldLine { id, mono, label: label.unwrap_or_default(), detail });
    }

    /// applyChildDelta: every line a subagent wrote, inside its own fold.
    #[allow(clippy::too_many_arguments)]
    fn child_delta(
        &mut self,
        parent: &str,
        kind: SessionEventKind,
        text: &str,
        tool: Option<&str>,
        call: Option<&str>,
        failed: bool,
        at: Option<f64>,
    ) {
        let key = call.map(|id| format!("{parent}:t{id}"));
        match kind {
            SessionEventKind::Text | SessionEventKind::Note => self.fold_line(parent, at, false, Some(text.to_owned()), None, None),
            SessionEventKind::Thinking => self.fold_line(parent, at, true, Some(text.to_owned()), None, None),
            SessionEventKind::ToolUse => {
                let t = self.turn.as_mut().expect("turn_for opened a turn");
                let open = key.as_ref().and_then(|key| t.child_calls.get(key));
                let name = tool.map(str::to_owned).or_else(|| open.map(|(name, _)| name.clone())).unwrap_or_else(|| "tool".to_owned());
                let input = open.map_or(String::new(), |(_, input)| input.clone()) + text;
                let line = tool_activity_line(Some(&name), &input);
                if let Some(key) = &key {
                    t.child_calls.insert(key.clone(), (name, input));
                }
                self.fold_line(parent, at, true, Some(line), None, key);
            }
            SessionEventKind::ToolResult => {
                let t = self.turn.as_mut().expect("turn_for opened a turn");
                let open = key.as_ref().and_then(|key| t.child_calls.remove(key));
                let did = open.filter(|_| !failed).and_then(|(name, input)| tool_done_line(Some(&name), &input));
                self.fold_line(parent, at, true, did, result_line(text, failed), key);
            }
            SessionEventKind::Unknown => {}
        }
    }

    fn delta(&mut self, event: &SessionEvent, at: Option<f64>) {
        let SessionEvent::SessionDelta {
            kind,
            text,
            message_id,
            tool_name,
            tool_use_id,
            is_error,
            parent_tool_use_id,
            exit_code,
            duration_ms,
            patch,
            patch_cut,
            bytes,
            ..
        } = event
        else {
            return;
        };
        if let Some(parent) = parent_tool_use_id {
            self.close();
            self.child_delta(parent, *kind, text, tool_name.as_deref(), tool_use_id.as_deref(), *is_error == Some(true), at);
            return;
        }
        let launched = tool_use_id.as_ref().filter(|id| self.turn.as_ref().is_some_and(|t| t.subagents.contains_key(*id)));
        if let (SessionEventKind::ToolResult, Some(id)) = (kind, launched) {
            if text.contains(INTERNAL_RESULT) {
                return;
            }
            let failed = *is_error == Some(true);
            if let Some(answer) = result_line(text, failed) {
                self.fold_line(id, at, false, Some(answer), None, Some(format!("answer:{id}")));
            }
            let ix = self.fold_for(id, at);
            if let Body::Subagent(s) = &mut self.entries[ix].body {
                s.state = if failed { SubState::Failed } else { SubState::Done };
                s.ended = at;
            }
            if let Some(t) = &mut self.turn {
                t.tools.remove(id);
            }
            return;
        }
        match kind {
            SessionEventKind::Unknown => {}
            SessionEventKind::Text => {
                let t = self.turn.as_ref().expect("turn_for opened a turn");
                if t.open_id.is_some() && message_id.is_some() && *message_id != t.open_id {
                    self.close();
                }
                let t = self.turn.as_mut().expect("turn_for opened a turn");
                t.saw_text = true;
                if let Some(Body::Agent { text: said, updated, .. }) = t.open.map(|ix| &mut self.entries[ix].body) {
                    said.push_str(text);
                    *updated = at.or(*updated);
                    return;
                }
                let ix = self.add_message(Body::Agent { text: text.clone(), streaming: true, updated: at }, at);
                let t = self.turn.as_mut().expect("turn_for opened a turn");
                t.open = Some(ix);
                t.open_id = message_id.clone();
            }
            SessionEventKind::Note => self.notice(text.clone(), None, Source::Other, at),
            SessionEventKind::Thinking => {
                self.close();
                let turn = self.turn.as_ref().map(|t| t.id.as_str());
                if let Some(last) = self.entries.last_mut().filter(|e| e.turn.as_deref() == turn) {
                    if let Body::Work(w) = &mut last.body {
                        if w.tone == Tone::Thinking {
                            let detail = w.detail.take().unwrap_or_default() + text;
                            w.preview = preview(&detail).or(w.preview.take());
                            w.detail = Some(detail);
                            return;
                        }
                    }
                }
                let mut work = Work::new("Thinking", Tone::Thinking, Source::Other);
                work.preview = preview(text);
                work.detail = Some(text.clone());
                self.add_work(work, at);
            }
            SessionEventKind::ToolUse => {
                self.close();
                let cwd = self.cwd.clone();
                let t = self.turn.as_mut().expect("turn_for opened a turn");
                let key = tool_use_id.clone().or_else(|| t.anon.map(|ix| format!("anon:{ix}")));
                if let Some(tool) = key.and_then(|key| t.tools.get_mut(&key)) {
                    if let Body::Work(w) = &mut self.entries[tool.entry].body {
                        if w.status == Some(Status::Running) {
                            tool.input.push_str(text);
                            let arriving = tool_name.clone().unwrap_or_else(|| w.label.clone());
                            let grown = facts(&arriving, &tool.input, cwd.as_deref());
                            let named = grown.title.clone().unwrap_or(arriving);
                            w.label = named.clone();
                            w.tool = Some(named);
                            w.facts(grown);
                            return;
                        }
                    }
                }
                let name = tool_name.as_deref().unwrap_or("tool");
                let found = facts(name, text, cwd.as_deref());
                let named = found.title.clone().unwrap_or_else(|| name.to_owned());
                let mut work = Work::new(named.clone(), Tone::Tool, Source::Other);
                work.tool = Some(named);
                work.status = Some(Status::Running);
                work.call = tool_use_id.clone();
                work.facts(found);
                let ix = self.add_work(work, at);
                let t = self.turn.as_mut().expect("turn_for opened a turn");
                let key = tool_use_id.clone().unwrap_or_else(|| format!("anon:{ix}"));
                t.tools.insert(key, Tool { entry: ix, name: tool_name.clone(), input: text.clone(), answered: None });
                if tool_use_id.is_none() {
                    t.anon = Some(ix);
                }
            }
            SessionEventKind::ToolResult => {
                self.close();
                let cwd = self.cwd.clone();
                let t = self.turn.as_mut().expect("turn_for opened a turn");
                let key = tool_use_id.clone().or_else(|| t.anon.map(|ix| format!("anon:{ix}")));
                let failed_now = *is_error == Some(true);
                let note = text.contains(INTERNAL_RESULT);
                let output = if note { None } else { preview(text) };
                let Some(call) = key.as_ref().and_then(|key| t.tools.get_mut(key)) else {
                    let name = tool_name.clone().unwrap_or_else(|| "tool".to_owned());
                    let mut work = Work::new(name.clone(), Tone::Tool, Source::Other);
                    work.tool = Some(name);
                    work.call = tool_use_id.clone();
                    work.status = Some(if failed_now { Status::Failed } else { Status::Completed });
                    work.detail = output;
                    self.add_work(work, at);
                    return;
                };
                let (entry_ix, name) = (call.entry, call.name.clone());
                let failed = failed_now || matches!(&self.entries[entry_ix].body, Body::Work(w) if w.status == Some(Status::Failed));
                if !note {
                    call.answered = Some((at, failed));
                }
                let held = key.as_ref().is_some_and(|key| t.held.contains(key));
                let refused = failed_now && name.as_deref().is_some_and(spawns_thread);
                let entry = &mut self.entries[entry_ix];
                let Body::Work(w) = &mut entry.body else { return };
                if tool_use_id.is_none() {
                    t.anon = None;
                }
                if let Some(line) = refused.then(|| compact_lines(text).into_iter().next()).flatten() {
                    let mut row = Work::new(line, Tone::Error, Source::Other);
                    row.status = Some(Status::Failed);
                    row.call = w.call.take();
                    **w = row;
                    return;
                }
                w.status = Some(if failed { Status::Failed } else { Status::Completed });
                if !failed {
                    let title = display_label(w).text;
                    w.spawned = spawned_thread(name.as_deref(), w.command.as_deref(), text).map(|thread| (thread, title));
                }
                if let Some(output) = output {
                    w.detail = Some(output);
                }
                if note || key.is_none() {
                    return;
                }
                let command = w.command.is_some();
                let took = entry.at.zip(at).map(|(from, to)| to - from).filter(|ms| *ms >= 0.0);
                if let Some(ms) = duration_ms.or(took.filter(|_| command && !held)) {
                    w.duration_ms = Some(ms);
                }
                let output = match exit_code {
                    Some(code) => strip_exit(text, *code),
                    None => text.as_str(),
                };
                if command && !output.is_empty() {
                    w.output = Some(output.to_owned());
                }
                if exit_code.is_some() {
                    w.exit_code = *exit_code;
                }
                if let Some(patch) = unified(patch, cwd.as_deref()) {
                    w.patch = Some(patch);
                }
                w.cut = *patch_cut == Some(true);
                w.bytes = *bytes;
            }
        }
    }
}

/// `retryLine` in packages/protocol/src/words/turn.ts.
fn retry_line(attempt: &str, of: Option<&str>, why: Option<&str>) -> String {
    let which = of.map_or_else(|| attempt.to_owned(), |of| format!("{attempt} of {of}"));
    let Some(why) = why else { return format!("Trying again ({which})") };
    let mut chars = why.chars();
    let first = chars.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default();
    format!("{first}{}, trying again ({which})", chars.as_str())
}

/// AGENT_WROTE in adapt/session.ts: what only an agent whose model call got through writes into its turn.
fn agent_wrote(event: &SessionEvent) -> bool {
    use SessionEvent as E;
    matches!(
        event,
        E::SessionDelta { .. }
            | E::SessionContext { .. }
            | E::SessionCompacted { .. }
            | E::SessionPlan { .. }
            | E::SessionSubagent { .. }
            | E::SessionPermission { .. }
            | E::SessionDone { .. }
            | E::SessionEnd { .. }
    )
}

/// Claude Code opens a failed command's words with its code for the model, which the row says on its own.
fn strip_exit(text: &str, code: i64) -> &str {
    let lead = format!("Exit code {code}");
    match text.strip_prefix(&lead) {
        Some("") => "",
        Some(rest) => rest.strip_prefix('\n').unwrap_or(text),
        None => text,
    }
}

fn killed_for_memory(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.strip_prefix("the agent on ").and_then(|rest| rest.get(1..)).is_some_and(|rest| rest.contains(" was killed for memory: "))
}

/// spawnsThread in packages/protocol/src/wsp-tools.ts: the wsp tools a cap can refuse.
fn spawns_thread(tool: &str) -> bool {
    matches!(server_tool(tool), Some(("wsp", "run" | "fork")))
}

fn tiles(attachments: &[SessionEventAttachmentsItem], request: Option<&str>) -> Vec<FileTile> {
    attachments
        .iter()
        .enumerate()
        .map(|(index, a)| {
            let image = matches!(a.media_type.as_str(), "image/png" | "image/jpeg" | "image/gif" | "image/webp");
            let name = a.name.clone().unwrap_or_else(|| if image { "image" } else { "file" }.to_owned());
            let (id, mime) = (request.map(str::to_owned), Some(a.media_type.clone()));
            FileTile { name, bytes: Some(a.bytes), image, id, index, mime }
        })
        .collect()
}

fn changed_file(path: &str, added: f64, removed: f64) -> ChangedFile {
    ChangedFile { path: path.to_owned(), added: added.max(0.0) as u64, removed: removed.max(0.0) as u64 }
}

fn sub_state(state: &str) -> SubState {
    match state {
        "done" => SubState::Done,
        "failed" => SubState::Failed,
        "stopped" => SubState::Stopped,
        _ => SubState::Running,
    }
}

pub(super) fn sub_word(state: SubState) -> &'static str {
    match state {
        SubState::Running => "running",
        SubState::Done => "done",
        SubState::Failed => "failed",
        SubState::Stopped => "stopped",
    }
}

/// lastLine in packages/protocol/src/words/base.ts.
fn last_line(text: &str) -> Option<String> {
    text.lines().rev().find(|l| !l.trim().is_empty()).map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
}

const LISTED_LINE_CHARS: usize = 200;

/// listedLastLine in packages/protocol/src/words/turn.ts: a list marker leading the line goes.
pub(super) fn listed_last_line(text: &str) -> Option<String> {
    let last = last_line(text)?;
    let marker = ["- ", "* ", "+ "].iter().find_map(|m| last.strip_prefix(m)).or_else(|| {
        let digits = last.chars().take_while(char::is_ascii_digit).count();
        let rest = &last[digits..];
        (1..=9).contains(&digits).then(|| rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "))).flatten()
    });
    let line = marker.map(str::trim_start).filter(|m| !m.is_empty()).unwrap_or(&last);
    Some(cut_line(line, LISTED_LINE_CHARS))
}

/// listedFailure in packages/protocol/src/words/turn.ts.
pub(super) fn listed_failure(why: &str) -> String {
    let first = why.lines().find(|l| !l.trim().is_empty()).map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "));
    cut_line(first.as_deref().unwrap_or("turn ended without a result"), LISTED_LINE_CHARS)
}

/// toolResultLine in packages/protocol/src/words/tools.ts.
fn result_line(text: &str, failed: bool) -> Option<String> {
    if text.contains(INTERNAL_RESULT) {
        return None;
    }
    let first = title_line(text);
    match (failed, first.is_empty()) {
        (false, true) => None,
        (false, false) => Some(first),
        (true, true) => Some("failed".to_owned()),
        (true, false) => Some(format!("failed: {first}")),
    }
}

/// subagentTaskLine in packages/protocol/src/words/tools.ts.
fn task_line(input: &str) -> Option<String> {
    tool_input(input).and_then(|fields| tool_field(&fields, "description").map(title_line))
}

pub(super) fn change_paths(fields: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    let changes = fields.get("changes").and_then(serde_json::Value::as_array);
    changes.into_iter().flatten().filter_map(|c| c.get("path")?.as_str().filter(|p| !p.is_empty()).map(str::to_owned)).collect()
}

/// spawnedThreadOf in the web app's adapt/spawned.ts: the thread a finished call that starts one names in its answer.
fn spawned_thread(tool: Option<&str>, command: Option<&str>, answer: &str) -> Option<String> {
    (tool.is_some_and(spawns_thread) || command.is_some_and(starts_thread)).then(|| named_thread(answer)).flatten()
}

/// wsp's run or fork at any path, as a command of its own in a chain, inside a `sh -c` script where Codex wraps one.
pub(super) fn starts_thread(command: &str) -> bool {
    let trimmed = command.trim();
    let script = trimmed.split_once(char::is_whitespace).and_then(|(shell, rest)| {
        let rest = rest.trim_start();
        let rest = rest.strip_prefix("-lc").or_else(|| rest.strip_prefix("-c"))?.trim_start();
        let quote = rest.chars().next().filter(|q| *q == '\'' || *q == '"')?;
        (shell.ends_with("sh") && rest.len() >= 2 && rest.ends_with(quote)).then(|| &rest[1..rest.len() - 1])
    });
    let mut bare = String::new();
    let mut quote: Option<char> = None;
    for c in script.unwrap_or(trimmed).chars() {
        match quote {
            Some(q) if c == q => {
                quote = None;
                bare.push_str("''");
            }
            Some(_) => {}
            None if c == '\'' || c == '"' => quote = Some(c),
            None => bare.push(c),
        }
    }
    let words: Vec<&str> =
        bare.split(|c: char| c.is_whitespace() || matches!(c, ';' | '&' | '|' | '(' | ')')).filter(|w| !w.is_empty()).collect();
    words.windows(2).any(|pair| (pair[0] == "wsp" || pair[0].ends_with("/wsp")) && matches!(pair[1], "run" | "fork"))
}

pub(super) fn named_thread(answer: &str) -> Option<String> {
    let thread_in = |text: &str, parsed: Option<serde_json::Value>| {
        let parsed = parsed.filter(serde_json::Value::is_object);
        let id =
            parsed.as_ref().and_then(|v| v.get("threadId").and_then(|t| t.as_str()).or_else(|| v.get("turn")?.get("threadId")?.as_str()));
        id.map(str::to_owned).or_else(|| {
            text.lines().find_map(|l| l.strip_prefix("thread ")).and_then(|rest| rest.split_whitespace().next()).map(str::to_owned)
        })
    };
    let parse = |text: &str| serde_json::from_str::<serde_json::Value>(text).ok();
    match parse(answer) {
        Some(serde_json::Value::Array(blocks)) => blocks.iter().find_map(|b| b.get("text")?.as_str().and_then(|t| thread_in(t, parse(t)))),
        whole => {
            let parsed = whole.or_else(|| parse(answer.trim().lines().last().unwrap_or("")));
            thread_in(answer, parsed)
        }
    }
}

/// One question of a call to the question tool (questionsIn in packages/protocol/src/words/tools.ts).
pub(super) fn questions_in(fields: &serde_json::Map<String, serde_json::Value>) -> Option<Vec<Asked>> {
    let raw = fields.get("questions")?.as_array()?;
    let mut questions = Vec::new();
    for (at, entry) in raw.iter().enumerate() {
        let Some(q) = entry.as_object() else { continue };
        let Some(text) = tool_field(q, "question") else { continue };
        let choices = q.get("options").and_then(serde_json::Value::as_array).into_iter().flatten().filter_map(|o| o.as_object());
        let mut options: Vec<PromptOption> = Vec::new();
        for o in choices {
            let Some(label) = tool_field(o, "label") else { continue };
            let description = tool_field(o, "description").map(str::to_owned);
            let id = format!("q{at}:o{}", options.len());
            options.push(PromptOption { id, label: label.to_owned(), description, effect: "answer".to_owned(), mode: None });
        }
        let header = tool_field(q, "header").map(str::to_owned);
        let multiple = q.get("multiSelect").and_then(serde_json::Value::as_bool) == Some(true);
        questions.push(Asked { header, question: text.to_owned(), options, multiple });
    }
    (!questions.is_empty()).then_some(questions)
}

/// folderName and parentFolderName in packages/protocol/src/project-path.ts.
pub(super) fn file_in_folder(path: &str) -> String {
    let parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    let name = parts.last().copied().unwrap_or(path);
    match parts.len() {
        0 | 1 => name.to_owned(),
        n if parts[n - 2].is_empty() => name.to_owned(),
        n => format!("{name} in {}", parts[n - 2]),
    }
}
