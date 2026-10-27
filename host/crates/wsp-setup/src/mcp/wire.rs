// SPDX-License-Identifier: AGPL-3.0-only
//! Packet 2's command line: `wsp mcp install`, which `mcp.rs` of `wsp-host` answers with the refusal below until it lands.
use wsp_cli::verbs::client::{failed, CliIo};
use wsp_words::exit::HostError;

use crate::context::CommandRun;

/// `wsp mcp install` (`cli/mcp.ts`): `line` is what follows `mcp`, the shared flags first.
pub fn mcp_install(io: &dyn CliIo, json: bool, _line: &[String], _run: &CommandRun) -> i32 {
    failed(io, json, &HostError::new("wsp mcp install is not in this build of the command line yet"), "")
}
