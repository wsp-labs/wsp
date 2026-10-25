// SPDX-License-Identifier: AGPL-3.0-only
//! A Claude Code session copied through one message to a new session beside it. TS:
//! packages/adapter-claude/src/chain.ts at a3b870a6e.

use std::sync::Arc;

use serde_json::Value;

use crate::port::{AdapterError, Env, ExecStreamFactory};

/// chain.ts:23.
pub const COPY_WALL_MS: u64 = 90_000;

/// chain.ts:30.
pub fn no_conversation_line(session: &str) -> String {
    todo!()
}

/// chain.ts:39. `Err("session identifier must be a UUID, got \"<id>\"")`.
pub fn session_file_line(config_dir: &str, session: &str) -> Result<String, AdapterError> {
    todo!()
}

/// chain.ts:46.
pub fn chain_read_command(session: &str, config_dir: &str) -> Result<String, AdapterError> {
    todo!()
}

/// One call an assistant line holds.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainCall {
    pub id: String,
    pub name: String,
    /// The call's input, `{}` where absent.
    pub input: Value,
}

/// One message line of the read. chain.ts:52.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainLine {
    pub at: u64,
    pub uuid: String,
    pub parent: Option<String>,
    pub r#type: String,
    pub boundary: bool,
    pub message_id: Option<String>,
    pub calls: Vec<ChainCall>,
    pub answers: Vec<String>,
}

/// chain.ts:70.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadChain {
    pub total: u64,
    pub lines: Vec<ChainLine>,
}

/// chain.ts:70. `Err` with the read's last line (or "the session's file could not be read") where its first line is
/// no whole number.
pub fn read_chain(read: &[String]) -> Result<ReadChain, AdapterError> {
    todo!()
}

/// chain.ts:104. Line numbers ascending.
pub fn chain_through(lines: &[ChainLine], anchor: &str) -> Result<Vec<u64>, AdapterError> {
    todo!()
}

/// Where a copy of a running session ends. chain.ts:124.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveEnd {
    pub anchor: String,
    pub running: Vec<String>,
}

/// chain.ts:124.
pub fn live_end(lines: &[ChainLine]) -> Option<LiveEnd> {
    todo!()
}

/// chain.ts:137.
pub fn line_ranges(numbers: &[u64]) -> String {
    todo!()
}

/// chain.ts:151.
pub fn chain_write_command(session: &str, fork: &str, config_dir: &str, count: u64) -> Result<String, AdapterError> {
    todo!()
}

/// chain.ts:161.
pub fn copy_cleanup_command(fork: &str, config_dir: &str) -> Result<String, AdapterError> {
    todo!()
}

/// chain.ts:168.
pub fn copy_wall_line(ms: u64) -> String {
    todo!()
}

/// chain.ts:171.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CopyThrough {
    Anchor(String),
    Live,
}

/// What copy_session takes. chain.ts:179.
#[derive(Clone)]
pub struct CopyOptions {
    pub exec: Arc<dyn ExecStreamFactory>,
    pub env: Env,
    pub config_dir: String,
    pub session: String,
    pub through: CopyThrough,
    pub grace_ms: u64,
    /// COPY_WALL_MS where `None`.
    pub wall_ms: Option<u64>,
}

/// A written copy and the calls a live copy left out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopiedSession {
    pub session: String,
    pub running: Vec<String>,
}

/// chain.ts:179. Two runs on `exec`: the read, then the write with the ranges as its one input line.
pub async fn copy_session(o: CopyOptions) -> Result<CopiedSession, AdapterError> {
    todo!()
}
