// SPDX-License-Identifier: AGPL-3.0-only
//! Where a revert cuts a thread and which turn a fork carries it through, on a server run that runs no turn. TS
//! adapter-codex/src/turn-pages.ts. Spec: spec/codex_adapter.md rules 9-11.

use futures::future::LocalBoxFuture;

use super::turn::SideRun;
use crate::port::{CountedTurn, ForkTurn, RevertAsk, Reverted, TurnResult};

/// A server run of its own that sends `input` after the handshake and hands each answer to `side_run`. TS
/// turn-pages.ts:13.
pub type SideRunner = Box<dyn Fn(Option<String>, Vec<String>, SideRun) -> LocalBoxFuture<'static, TurnResult>>;

/// TS turn-pages.ts:22.
pub const PAGE_AGAIN: &str = "codex could not list the thread's turns: it handed back a page it already gave";

/// Whether the server opened a cut turn, so the thread's history holds it. TS turn-pages.ts:27.
pub fn server_opened(turn: &CountedTurn) -> bool {
    todo!()
}

/// TS turn-pages.ts:44. `Err` carries the words the TS rejects with.
pub async fn revert_on(run: &SideRunner, ask: RevertAsk) -> Result<Reverted, String> {
    todo!()
}

/// The session and turn a fork carries the thread through. TS turn-pages.ts:89.
pub async fn fork_session_on(run: &SideRunner, session: String, turn: ForkTurn) -> Result<(String, String), String> {
    todo!()
}
