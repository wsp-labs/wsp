// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/agents-help.ts` at 65f6b89c7, in two units: `read` (the target a line names, the report
//! and the rows of its four lists, the plugin and server writes, sign-in) and `set` (skills, agent and project
//! defaults, an agent's setup). Specs: `spec/agents-read.md`, `spec/agents-set.md`.

pub mod read;
pub mod set;

pub use read::*;
pub use set::*;
