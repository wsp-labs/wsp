// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/turns-help.ts` at 65f6b89c7. Only the types `io.rs` names in its signatures are declared
//! here so far; the module's own writer adds the rest.

use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events::TurnResult;
use wsp_wire::views::sub_row::SubResult;
use wsp_wire::wire::runtime::SessionStartOutcome;

/// `Turn` (turns-help.ts:124-136).
#[derive(Clone, Debug, PartialEq)]
pub struct Turn {
    pub session: SessionView,
    pub thread_id: String,
    pub outcome: SessionStartOutcome,
    pub result: Option<TurnResult>,
    pub reason: Option<String>,
    /// `afterCut?: true`.
    pub after_cut: bool,
    pub notice: Option<String>,
}

/// `Ended` (turns-help.ts:731-735).
#[derive(Clone, Debug, PartialEq)]
pub struct Ended {
    pub thread_id: String,
    pub result: TurnResult,
    pub sub: Option<SubResult>,
}

/// `Waited` (turns-help.ts:747).
#[derive(Clone, Debug, PartialEq)]
pub enum Waited {
    Ended(Ended),
    TimedOutMs(f64),
}
