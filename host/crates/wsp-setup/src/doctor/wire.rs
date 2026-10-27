// SPDX-License-Identifier: AGPL-3.0-only
//! Packet 5b's door and command line: `places.doctor` and `wsp doctor`.
use std::rc::Rc;

use wsp_cli::verbs::client::CliIo;
use wsp_serve::serve::PlaceDoctor;

use crate::context::{not_in_build, CommandRun, HostWiring};

/// `computerDoctor` (`doctor.ts:1589`) as `server.ts:532-560` hands it to `serveRuntime`: the `doctor` slot.
pub fn place_doctor(_wiring: &HostWiring) -> Option<Rc<dyn PlaceDoctor>> {
    None
}

/// `wsp doctor` (`cli/commands.ts`): `line` is the whole shared line.
pub async fn doctor(_line: Vec<String>, json: bool, io: &dyn CliIo, _run: &CommandRun) -> i32 {
    not_in_build(io, json, "doctor")
}
