// SPDX-License-Identifier: AGPL-3.0-only
//! Packet C4's command lines: `wsp join` and `wsp leave`.
use wsp_cli::verbs::client::CliIo;

use crate::context::{not_in_build, CommandRun};

/// `wsp join` (`cli/commands.ts`, `places/join.ts`).
pub async fn join(_line: Vec<String>, json: bool, io: &dyn CliIo, _run: &CommandRun) -> i32 {
    not_in_build(io, json, "join")
}

/// `wsp leave` (`cli/commands.ts`, `places/join.ts`).
pub async fn leave(_line: Vec<String>, json: bool, io: &dyn CliIo, _run: &CommandRun) -> i32 {
    not_in_build(io, json, "leave")
}
