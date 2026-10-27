// SPDX-License-Identifier: AGPL-3.0-only
//! Packet C5's command lines: `wsp add`, `wsp remove`, `wsp login`, `wsp logout` and `wsp hosts`.
use wsp_cli::verbs::client::CliIo;

use crate::context::{not_in_build, CommandRun};

/// `wsp add` (`cli/commands.ts`, `places/commands.ts`).
pub async fn add(_line: Vec<String>, json: bool, io: &dyn CliIo, _run: &CommandRun) -> i32 {
    not_in_build(io, json, "add")
}

/// `wsp remove` (`cli/commands.ts`, `places/commands.ts`).
pub async fn remove(_line: Vec<String>, json: bool, io: &dyn CliIo, _run: &CommandRun) -> i32 {
    not_in_build(io, json, "remove")
}

/// `wsp login` (`loginCommand`, `places/commands.ts`).
pub async fn login(_line: Vec<String>, json: bool, io: &dyn CliIo, _run: &CommandRun) -> i32 {
    not_in_build(io, json, "login")
}

/// `wsp logout` (`logoutCommand`, `places/commands.ts`).
pub async fn logout(_line: Vec<String>, json: bool, io: &dyn CliIo, _run: &CommandRun) -> i32 {
    not_in_build(io, json, "logout")
}

/// `wsp hosts` (`cli/commands.ts`, `places/commands.ts`).
pub async fn hosts(_line: Vec<String>, json: bool, io: &dyn CliIo, _run: &CommandRun) -> i32 {
    not_in_build(io, json, "hosts")
}
