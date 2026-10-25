// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/exec-detached.ts at a4033dc34, in two units: [`put`] (spec/exec_put.md) and [`run`]
//! (spec/exec_run.md).
//!
//! One guest command that may outlive a single exec: the script goes to a file and starts in its own session with its
//! streams and exit code on disk, then short execs read the files until the exit code is there or the deadline kills
//! the session.
pub mod put;
pub mod run;

pub use put::*;
pub use run::*;
