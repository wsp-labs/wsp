// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/workspace-state.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

/// TS: packages/protocol/src/workspace-state.ts:97 at a3b870a6e.
pub const COMPUTER_LEFT: &str = "computer is left as it is";

/// TS: packages/protocol/src/workspace-state.ts:219 at a3b870a6e.
pub const UNNAMED_COMPUTER: &str = "that computer";

/// TS: packages/protocol/src/workspace-state.ts:370 at a3b870a6e.
pub const START_DAEMON_WORD: &str = "Start again";

/// TS: packages/protocol/src/workspace-state.ts:391 at a3b870a6e.
pub const WAITED_ROW_MS: u64 = 5_000;

/// TS: packages/protocol/src/workspace-state.ts:419 at a3b870a6e.
pub const REPORTED_WORD: &str = "reported";

/// TS: packages/protocol/src/workspace-state.ts:465 at a3b870a6e.
pub const BACK_OVER_SSH: &str = "back over ssh";

/// TS: packages/protocol/src/workspace-state.ts:562 at a3b870a6e.
/// A Record in the TS: its keys in the TS's order.
pub const SEND_BLOCK_WORDS: &[(&str, &str)] = &[
    ("connecting", "Connecting to wsp"),
    ("reconnecting", "wsp is not running, reconnecting"),
    ("closed", "wsp is not running"),
    ("not-found", "Task not found"),
    ("loading", "Loading transcript"),
    ("no-agents", "the agents here have not answered yet"),
];

/// TS: packages/protocol/src/workspace-state.ts:596 at a3b870a6e.
pub const CLOUD_SIGN_IN_ROAD: &str = "sign this workspace in from the Workspace panel";

/// TS: packages/protocol/src/workspace-state.ts:661 at a3b870a6e.
pub const FORGET_NEEDS_GONE: &str = "Only a workspace whose computer is gone can be forgotten";
