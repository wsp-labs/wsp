// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/machine-exec.ts at 65f6b89c7: the `ExecStreamFactory` over a machine's one-shot
//! exec. Three units, each with its spec under spec/: `rules` (machine_exec_rules.md), `factory`
//! (machine_exec_factory.md) and `stream` (machine_exec_stream.md).
pub mod factory;
pub mod rules;
pub mod stream;

pub use factory::{machine_exec_stream, KilledWhy, MachineExecFactory, MachineExecOptions, NowFn, SleepFn};
pub use rules::{
    reads_work, turn_activity, turn_cut, LineCut, ReadingSet, TurnActivity, TurnLimits, TurnWaiting, LEFTOVERS_AWK, TREE_WORK_AWK,
};
pub use stream::MachineStream;
