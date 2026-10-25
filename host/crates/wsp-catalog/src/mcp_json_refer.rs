// SPDX-License-Identifier: AGPL-3.0-only
//! The JSON formats' reference writer: each server's header, command variable, OAuth secret and argument default
//! written by name in place. Port of packages/catalog/src/mcp.ts:921-1087 at a3b870a6e; spec/mcp_json_refer.md.
use indexmap::IndexSet;

use crate::mcp::{KnownValue, McpFormat, McpReferred};
use crate::pending::HostError;

/// `format.refer` for a JSON format (mcp.ts:957).
pub fn refer(format: McpFormat, text: &str, only: Option<&str>, known: &[KnownValue], outside: &IndexSet<String>) -> Result<McpReferred, HostError> {
    todo!()
}
