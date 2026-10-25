// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/place-state.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

/// TS: packages/protocol/src/place-state.ts:40 at a3b870a6e.
pub const THREAD_MEM_MB: u64 = 2_048;

/// TS: packages/protocol/src/place-state.ts:46 at a3b870a6e.
/// A Record in the TS: its keys in the TS's order.
pub const CLOUD_CAP_DEFAULT: &[(&str, f64)] = &[("machines", 3.0), ("spendPerDayUsd", 10.0)];

/// TS: packages/protocol/src/place-state.ts:54 at a3b870a6e.
pub const NAP_AFTER_MS: u64 = 1_200_000;

/// TS: packages/protocol/src/place-state.ts:57 at a3b870a6e.
pub const NAP_AFTER_MAX_MS: u64 = 10_800_000;

/// TS: packages/protocol/src/place-state.ts:63 at a3b870a6e.
pub const DELETE_AFTER_MS: u64 = 1_800_000;

/// TS: packages/protocol/src/place-state.ts:65 at a3b870a6e.
pub const DELETE_AFTER_MAX_MS: u64 = 86_400_000;

/// TS: packages/protocol/src/place-state.ts:69 at a3b870a6e.
pub const TURN_WALL_MS: u64 = 21_600_000;

/// TS: packages/protocol/src/place-state.ts:73 at a3b870a6e.
pub const TURN_LIMIT_MAX_MS: u64 = 86_400_000;

/// TS: packages/protocol/src/place-state.ts:361 at a3b870a6e.
pub const CAP_RAISE_ACT: &str = "Raise threads at once";

/// TS: packages/protocol/src/place-state.ts:368 at a3b870a6e.
pub const WAKE_ACT: &str = "Wake";

// ---- Hand-written below: the functions of the unit.

// place-state.ts:51 at a3b870a6e
// export const AGENTS_ON: WorkspaceAgents = { spawn: true, maxMachines: 3, maxDepth: 2 };
/// What a workspace's agents may do before a person sets it: spawn, at most 3 machines, 2 levels deep.
pub const AGENTS_ON: crate::views::workspace::WorkspaceAgents =
    crate::views::workspace::WorkspaceAgents { spawn: true, max_machines: 3, max_depth: 2 };
