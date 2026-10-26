// SPDX-License-Identifier: AGPL-3.0-only
//! Where a revert cuts a thread and which turn a fork carries it through, on a server run that runs no turn. TS
//! adapter-codex/src/turn-pages.ts. Spec: spec/codex_adapter.md rules 9-11.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use serde_json::{Map, Value};

use super::command::access_params;
use super::fields::{rec, str};
use super::rpc::{request, thread_resume_line, thread_revert_line, thread_turns_list_line, RequestId, ThreadOptions};
use super::turn::{SideRun, SideStep};
use crate::pending::{codex_no_turn_line, CODEX_FEWER_TURNS, CODEX_LEGACY_HISTORY};
use crate::port::{CountedTurn, ForkTurn, RevertAsk, RevertAt, Reverted, TurnResult, TurnStatus};

/// A server run of its own that sends `input` after the handshake and hands each answer to `side_run`. TS
/// turn-pages.ts:13.
pub type SideRunner = Box<dyn Fn(Option<String>, Vec<String>, SideRun) -> LocalBoxFuture<'static, TurnResult>>;

/// The words a server that refuses to cut a legacy thread says it in (thread_processor.rs, 0.155.1). TS turn-pages.ts:16.
const PAGINATED_ONLY: &str = "only supports paginated threads";

/// The start of every failure the adapter words itself before the server opened the turn. TS turn-pages.ts:19.
const NEVER_OPENED: &str = "codex could not ";

/// TS turn-pages.ts:22.
pub const PAGE_AGAIN: &str = "codex could not list the thread's turns: it handed back a page it already gave";

/// Whether the server opened a cut turn, so the thread's history holds it. TS turn-pages.ts:27.
pub fn server_opened(turn: &CountedTurn) -> bool {
    let result = turn.result.as_ref();
    turn.anchor.is_some()
        || (result.map_or(true, |r| r.refusal.is_none())
            && !result.and_then(|r| r.error.as_deref()).is_some_and(|e| e.starts_with(NEVER_OPENED)))
}

fn finished(status: TurnStatus, error: Option<String>) -> TurnResult {
    TurnResult {
        status,
        duration_ms: None,
        waited_ms: None,
        cost_usd: None,
        tokens: None,
        model: None,
        models: None,
        text: None,
        error,
        unreached: None,
        refusal: None,
        limit: None,
    }
}

fn done() -> TurnResult {
    finished(TurnStatus::Completed, None)
}

fn failed(words: &str) -> TurnResult {
    finished(TurnStatus::Failed, Some(words.to_string()))
}

fn is(id: &RequestId, name: &str) -> bool {
    matches!(id, RequestId::Str(s) if s == name)
}

/// The ids on one page of thread/turns/list, newest first. TS turn-pages.ts:37.
fn turn_ids(answer: Option<&Map<String, Value>>) -> Vec<String> {
    answer
        .and_then(|a| a.get("data"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|turn| str(turn.as_object()?.get("id")).map(String::from))
        .collect()
}

/// The revert's pages: the thread, whether it is a legacy history, the turns left to cut past, the turn to cut before,
/// the cursor last read and every cursor read. TS turn-pages.ts:44-50.
struct Cut {
    thread: String,
    legacy: bool,
    remaining: i64,
    before: Option<String>,
    cursor: Option<String>,
    seen: HashSet<Option<String>>,
}

impl Cut {
    /// TS turn-pages.ts:51.
    fn page(&mut self) -> String {
        self.seen.insert(self.cursor.clone());
        thread_turns_list_line(&self.thread, self.cursor.as_deref(), self.remaining.min(100) as u64)
    }

    /// TS turn-pages.ts:55.
    fn cut_at(&self) -> SideStep {
        match &self.before {
            None => SideStep::End(done()),
            Some(turn) => SideStep::Send(thread_revert_line(&self.thread, turn)),
        }
    }
}

/// TS turn-pages.ts:59.
fn cut_step(cut: &mut Cut, id: &RequestId, answer: Option<&Map<String, Value>>) -> SideStep {
    if is(id, request::THREAD) {
        let thread = rec(answer.and_then(|a| a.get("thread")));
        if let Some(thread_id) = thread.and_then(|t| str(t.get("id"))) {
            cut.thread = thread_id.to_string();
        }
        cut.legacy = thread.and_then(|t| str(t.get("historyMode"))) == Some("legacy");
        if cut.legacy {
            return SideStep::End(done());
        }
        return if cut.before.is_none() && cut.remaining > 0 { SideStep::Send(cut.page()) } else { cut.cut_at() };
    }
    if is(id, request::TURNS) {
        for turn in turn_ids(answer) {
            cut.before = Some(turn);
            cut.remaining -= 1;
            if cut.remaining == 0 {
                break;
            }
        }
        cut.cursor = str(answer.and_then(|a| a.get("nextCursor"))).map(String::from);
        if cut.remaining == 0 {
            return cut.cut_at();
        }
        // wsp's count is rebuilt from its own transcript, so a server that runs out first disagrees with it.
        if cut.cursor.is_none() {
            return SideStep::End(failed(CODEX_FEWER_TURNS));
        }
        if cut.seen.contains(&cut.cursor) {
            return SideStep::End(failed(PAGE_AGAIN));
        }
        return SideStep::Send(cut.page());
    }
    SideStep::End(done())
}

/// TS turn-pages.ts:44. `Err` carries the words the TS rejects with.
pub async fn revert_on(run: &SideRunner, ask: RevertAsk) -> Result<Reverted, String> {
    let (remaining, before) = match &ask.at {
        RevertAt::BeforeTurn(turn) => (0, Some(turn.clone())),
        RevertAt::Turns(turns) => (turns.iter().filter(|turn| server_opened(turn)).count() as i64, None),
    };
    let cut =
        Rc::new(RefCell::new(Cut { thread: ask.session.clone(), legacy: false, remaining, before, cursor: None, seen: HashSet::new() }));
    let handler = cut.clone();
    let side_run: SideRun =
        Box::new(move |id: &RequestId, answer: Option<&Map<String, Value>>| cut_step(&mut handler.borrow_mut(), id, answer));
    let options =
        ThreadOptions { cwd: ask.cwd.clone(), model: None, service_tier: None, access: access_params(Some("read-only"))?, config: None };
    let input = vec![thread_resume_line(&options, &ask.session)];
    let result = run(ask.cwd.clone(), input, side_run).await;
    let legacy = cut.borrow().legacy;
    if legacy || (matches!(result.status, TurnStatus::Failed) && result.error.as_deref().is_some_and(|e| e.contains(PAGINATED_ONLY))) {
        return Ok(Reverted::Kept(CODEX_LEGACY_HISTORY.to_string()));
    }
    if !matches!(result.status, TurnStatus::Completed) {
        return Err(result.error.unwrap_or_else(|| "codex did not cut the thread".to_string()));
    }
    Ok(Reverted::Cut)
}

/// The fork's pages: the session, the anchor it looks for or else the turns it skips, the turn found, the cursor last
/// read and every cursor read. TS turn-pages.ts:90-93.
struct Fork {
    session: String,
    anchor: Option<String>,
    skip: u64,
    found: Option<String>,
    cursor: Option<String>,
    seen: HashSet<Option<String>>,
}

impl Fork {
    /// TS turn-pages.ts:94.
    fn page(&mut self) -> String {
        self.seen.insert(self.cursor.clone());
        thread_turns_list_line(&self.session, self.cursor.as_deref(), 100)
    }
}

/// TS turn-pages.ts:100.
fn fork_step(fork: &mut Fork, id: &RequestId, answer: Option<&Map<String, Value>>) -> SideStep {
    if !is(id, request::TURNS) {
        return SideStep::End(done());
    }
    for turn in turn_ids(answer) {
        let hit = match &fork.anchor {
            Some(anchor) => turn == *anchor,
            None if fork.skip == 0 => true,
            None => {
                fork.skip -= 1;
                false
            }
        };
        if hit {
            fork.found = Some(turn);
            return SideStep::End(done());
        }
    }
    fork.cursor = str(answer.and_then(|a| a.get("nextCursor"))).map(String::from);
    if fork.cursor.is_none() {
        let words = match &fork.anchor {
            Some(anchor) => codex_no_turn_line(anchor),
            None => CODEX_FEWER_TURNS.to_string(),
        };
        return SideStep::End(failed(&words));
    }
    if fork.seen.contains(&fork.cursor) {
        return SideStep::End(failed(PAGE_AGAIN));
    }
    SideStep::Send(fork.page())
}

/// The session and turn a fork carries the thread through. TS turn-pages.ts:89.
pub async fn fork_session_on(run: &SideRunner, session: String, turn: ForkTurn) -> Result<(String, String), String> {
    let (anchor, skip) = match &turn {
        ForkTurn::Anchor(anchor) => (Some(anchor.clone()), 0),
        ForkTurn::After(turns) => (None, turns.iter().filter(|turn| server_opened(turn)).count() as u64),
    };
    let fork = Rc::new(RefCell::new(Fork {
        session: session.clone(),
        anchor: anchor.clone(),
        skip,
        found: None,
        cursor: None,
        seen: HashSet::new(),
    }));
    let first = fork.borrow_mut().page();
    let handler = fork.clone();
    let side_run: SideRun =
        Box::new(move |id: &RequestId, answer: Option<&Map<String, Value>>| fork_step(&mut handler.borrow_mut(), id, answer));
    let result = run(None, vec![first], side_run).await;
    let found = fork.borrow().found.clone();
    if let Some(turn) = found {
        return Ok((session, turn));
    }
    if let Some(anchor) = anchor {
        if result.error.as_deref().is_some_and(|e| e.contains(PAGINATED_ONLY)) {
            return Ok((session, anchor));
        }
    }
    Err(result.error.unwrap_or_else(|| "codex did not find the turn to fork the thread at".to_string()))
}
