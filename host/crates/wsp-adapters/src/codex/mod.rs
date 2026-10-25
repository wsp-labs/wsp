// SPDX-License-Identifier: AGPL-3.0-only
//! packages/adapter-codex at a3b870a6e: one Codex turn through `codex app-server`, and everything read from codex
//! outside a turn. Each module's spec is spec/codex_<unit>.md.

pub mod adapter;
pub mod background;
pub mod catalog;
pub mod children;
pub mod command;
pub mod conversations;
pub mod fields;
pub mod items;
pub mod limits;
pub mod lost_session;
pub mod reset;
pub mod rpc;
pub mod session_held;
pub mod session_title;
pub mod shell_script;
pub mod turn;
pub mod turn_pages;

pub use adapter::{create_codex_adapter, CodexAdapter, CodexAdapterDeps};
pub use turn::{CodexKept, CodexSession};
