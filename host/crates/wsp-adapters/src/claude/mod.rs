// SPDX-License-Identifier: AGPL-3.0-only
//! packages/adapter-claude.

pub mod adapter;
pub mod aside;
pub mod catalog;
pub mod chain;
pub mod conversations;
pub mod fields;
pub mod handback;
pub mod landmines;
pub mod limits;
pub mod normalize;
pub mod permissions;
pub mod plans;
pub mod session_title;
pub mod shell_cwd;
pub mod steers;
pub mod subagent_spend;
pub mod tokens;
pub mod tool_results;
pub mod turn;

pub use adapter::{create_claude_adapter, AdapterDeps, AttachedSession, ClaudeAdapter};
pub use turn::{ClaudeKept, ClaudeSession};
