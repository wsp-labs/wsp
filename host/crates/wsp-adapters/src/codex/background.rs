// SPDX-License-Identifier: AGPL-3.0-only
//! A kept server's commands still going, by item id, across its turns. TS adapter-codex/src/background.ts. Spec:
//! spec/codex_turn.md rule 22.

use indexmap::IndexSet;

use crate::port::ExecStream;

/// TS background.ts:8.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Background {
    pub commands: IndexSet<String>,
    /// The thread a turn's end kept subscribed for the commands, let go once the last one ends.
    pub held: Option<String>,
}

/// A line the server printed between turns, where a command a turn left going may end. TS background.ts:15.
pub fn ended_between(run: &dyn ExecStream, background: &mut Background, raw: &str) {
    todo!()
}
