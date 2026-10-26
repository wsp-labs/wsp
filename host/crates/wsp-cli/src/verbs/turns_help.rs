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

use serde::{Serialize, Serializer};
use serde_json::Value;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events::TurnResult;
use wsp_wire::views::sub_row::SubResult;
use wsp_wire::wire::runtime::SessionStartOutcome;

/// A turn's result as the host wrote it: the TS keeps the event's object and prints it as it came, so this keeps the
/// raw JSON for printing and reads the fields the verbs look at through `TurnResult`.
#[derive(Clone, Debug, PartialEq)]
pub struct HostResult {
    typed: TurnResult,
    raw: Value,
}

impl HostResult {
    /// The host's result object, or nothing where it does not read as a `TurnResult`.
    pub fn read(raw: Value) -> Option<Self> {
        let typed = serde_json::from_value(raw.clone()).ok()?;
        Some(Self { typed, raw })
    }

    /// A result the verb made itself, written in the struct's order.
    pub fn made(typed: TurnResult) -> Self {
        let raw = serde_json::to_value(&typed).expect("a result writes as JSON");
        Self { typed, raw }
    }
}

impl std::ops::Deref for HostResult {
    type Target = TurnResult;

    fn deref(&self) -> &TurnResult {
        &self.typed
    }
}

impl Serialize for HostResult {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.raw.serialize(serializer)
    }
}

/// `Turn` (turns-help.ts:124-136).
#[derive(Clone, Debug, PartialEq)]
pub struct Turn {
    pub session: SessionView,
    pub thread_id: String,
    pub outcome: SessionStartOutcome,
    pub result: Option<HostResult>,
    pub reason: Option<String>,
    /// `afterCut?: true`.
    pub after_cut: bool,
    pub notice: Option<String>,
}

/// `Ended` (turns-help.ts:731-735).
#[derive(Clone, Debug, PartialEq)]
pub struct Ended {
    pub thread_id: String,
    pub result: HostResult,
    pub sub: Option<SubResult>,
}

/// `Waited` (turns-help.ts:747).
#[derive(Clone, Debug, PartialEq)]
pub enum Waited {
    Ended(Ended),
    TimedOutMs(f64),
}
