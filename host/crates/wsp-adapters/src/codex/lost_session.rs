// SPDX-License-Identifier: AGPL-3.0-only
//! A start that resumes a thread whose rollout the server does not hold: a second launch on a new thread handed the
//! thread so far, through the port's `start_after_loss`. TS adapter-codex/src/lost-session.ts:59-96. Spec:
//! spec/codex_adapter.md rule 8.

use super::turn::{CodexSession, Sessions};
use crate::port::{AdapterError, HarnessStartOptions};

/// What a server answers a resume of a thread its home holds no rollout for. TS lost-session.ts:77.
pub fn no_rollout_line(thread_id: &str) -> String {
    todo!()
}

/// A launch, and where it resumes with a seed (not a copy, not a fork), the second launch behind it; the answer reads
/// whichever runs now. Registered under the first's local id. TS lost-session.ts:81.
pub fn start_seeded(
    launch_turn: std::sync::Arc<dyn Fn(HarnessStartOptions) -> Result<CodexSession, AdapterError>>,
    sessions: &Sessions,
    options: HarnessStartOptions,
) -> Result<CodexSession, AdapterError> {
    todo!()
}
