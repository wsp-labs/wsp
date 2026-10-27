// SPDX-License-Identifier: AGPL-3.0-only
//! Packet C2b's doors and command lines: the account and the admitted devices, the relay address of the door a computer
//! you own dials, and `wsp host link`, `wsp host unlink` and `wsp relay`.
use std::rc::Rc;

use wsp_cli::verbs::client::CliIo;
use wsp_serve::serve::{AccountDoor, AdmittedDevices};

use crate::context::{not_in_build, CommandRun, HostWiring};

/// `accountHere` (`relay-link.ts:114`): `ServeOptions.account` (`server.ts:585`).
pub fn account_door(_wiring: &HostWiring) -> Option<Rc<dyn AccountDoor>> {
    None
}

/// `admittedDevices(statePath)` (`relay-link.ts:592`, made at `cli/serve.ts:237`): `ServeOptions.admitted`.
pub fn admitted_devices(_wiring: &HostWiring) -> Option<Rc<dyn AdmittedDevices>> {
    None
}

/// `relayUrlOf(publicHostname(statePath))` (`server.ts:484-488`): the relay's own name beside the door's addresses, where
/// a connector is carrying this host.
pub fn door_relay(_state_path: &str) -> Option<String> {
    None
}

/// `wsp host link` (`cli/commands.ts`, `relayCommand(["link", ...])`).
pub async fn host_link(_line: Vec<String>, json: bool, io: &dyn CliIo, _run: &CommandRun) -> i32 {
    not_in_build(io, json, "host")
}

/// `wsp host unlink` (`cli/commands.ts`, `relayCommand(["unlink", ...])`).
pub async fn host_unlink(_line: Vec<String>, json: bool, io: &dyn CliIo, _run: &CommandRun) -> i32 {
    not_in_build(io, json, "host")
}
