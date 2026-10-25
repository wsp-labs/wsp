// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/local-exec.ts at 65f6b89c7: the `ExecStreamFactory` of a turn on this computer. Two
//! units, each with its spec under spec/: `reap` (local_exec_reap.md) and `stream` (local_exec_stream.md).
pub mod reap;
pub mod stream;

pub use reap::{agent_pid_of, group_exists, tree_work, TreeWorkReader};
pub use stream::{local_exec_stream, LocalExecFactory, LocalExecOptions, LocalStream};
