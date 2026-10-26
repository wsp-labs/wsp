// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/turns-help.ts` at 65f6b89c7, in four units: `picks` (what a start carries), `target`
//! (where a run goes and the threads a line names), `follow` (starting a turn and following or waiting on it) and
//! `stream` (what a watched turn prints, and the answers to its prompts). The types `io.rs` names in its signatures
//! stay here. Specs: `spec/turns-picks.md`, `spec/turns-target.md`, `spec/turns-follow.md`, `spec/turns-stream.md`.

pub mod follow;
pub mod picks;
pub mod stream;
pub mod target;

pub use follow::*;
pub use picks::*;
pub use stream::*;
pub use target::*;

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
