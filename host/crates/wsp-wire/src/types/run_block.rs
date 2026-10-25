// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/run-block.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

/// TS: packages/protocol/src/run-block.ts:7 at a3b870a6e.
/// A Set in the TS, in insertion order.
pub const SHELL_FENCES: &[&str] = &["sh", "bash", "zsh", "shell", "console"];

/// TS: packages/protocol/src/run-block.ts:11 at a3b870a6e.
pub const RUN_BLOCK_WORDS: &str = "A command you mean the person to run goes in a fenced block labelled `sh`, which the app shows with Run: the person runs it with one click, in this thread's folder, and sees its output under the block";

/// TS: packages/protocol/src/run-block.ts:32 at a3b870a6e.
pub const PTY_RUN_DAEMON_VERSION: u64 = 98;

/// TS: packages/protocol/src/run-block.ts:35 at a3b870a6e.
pub const RUN_OUTPUT_MAX_CHARS: u64 = 65_536;

/// TS: packages/protocol/src/run-block.ts:107 at a3b870a6e.
pub const RUN_MOVE_AFTER_MS: u64 = 3_000;

/// TS: packages/protocol/src/run-block.ts:110 at a3b870a6e.
/// A Record in the TS: its keys in the TS's order.
pub const RUN_WORDS: &[(&str, &str)] = &[
    ("run", "Run"),
    ("runAgain", "Run again"),
    ("running", "Running"),
    ("move", "Move to terminal"),
    ("send", "Send output to the agent"),
    ("openTab", "Open the tab"),
];

/// TS: packages/protocol/src/run-block.ts:129 at a3b870a6e.
pub const RUN_SENT_LINES: u64 = 60;
