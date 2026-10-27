// SPDX-License-Identifier: AGPL-3.0-only
//! Packet C2a's command lines: `wsp down`, `wsp status`, `wsp host pair`, `wsp host devices` and `wsp up --service`.
use wsp_cli::verbs::client::CliIo;

use crate::context::{not_in_build, CommandRun};

/// `wsp down` (`downCommand`, `cli/service.ts:220`).
pub async fn down(_line: Vec<String>, json: bool, io: &dyn CliIo, _run: &CommandRun) -> i32 {
    not_in_build(io, json, "down")
}

/// `wsp status` (`statusCommand`, `cli/service.ts:282`).
pub async fn status(_line: Vec<String>, json: bool, io: &dyn CliIo, _run: &CommandRun) -> i32 {
    not_in_build(io, json, "status")
}

/// `wsp host pair` (`pairCommand`, `pairing.ts`).
pub async fn host_pair(_line: Vec<String>, json: bool, io: &dyn CliIo, _run: &CommandRun) -> i32 {
    not_in_build(io, json, "host")
}

/// `wsp host devices` (`devicesCommand`, `pairing.ts`).
pub async fn host_devices(_line: Vec<String>, json: bool, io: &dyn CliIo, _run: &CommandRun) -> i32 {
    not_in_build(io, json, "host")
}

/// `wsp up --service` (`upServiceCommand`, `cli/service.ts:165`): `None` is a line this build does not hand to the
/// service manager, which serves in the terminal as it does today with the flag read and unused.
pub async fn up_service(_line: &[String], _io: &dyn CliIo, _run: &CommandRun) -> Option<i32> {
    None
}
