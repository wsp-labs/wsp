// SPDX-License-Identifier: AGPL-3.0-only
//! packages/protocol/src/words/thread.ts, ported as two units split at its line 445: `lines` and `forks`.

mod forks;
mod lines;

pub use forks::*;
pub use lines::*;
