// SPDX-License-Identifier: AGPL-3.0-only
//! `machine-host.ts`: the collector's Host over any machine this host reaches (a joined computer, a workspace or a fork).
use std::rc::Rc;

use wsp_collect::host::Host;
use wsp_engine::machine::Machine;
use wsp_engine::target_line::TargetLogin;

/// `MachineHost` (`machine-host.ts:13`), a Host whose reads may have been refused: one line per read that could not answer whole.
pub struct MachineHost {
    pub host: Host,
    pub refused: Vec<String>,
}

/// `MACHINE_READ_CAP` (`machine-host.ts:18`).
pub const MACHINE_READ_CAP: u64 = 1024 * 1024;

/// `EnvRoad` (`machine-host.ts:101`), how a run's variables reach it: on the command's stdin, or in a file landed on the machine.
pub enum EnvRoad {
    Stdin,
    Land(Rc<dyn Machine>),
}

/// `machineHost` (`machine-host.ts:103`).
pub fn machine_host(_machine: Rc<dyn Machine>, _login: &TargetLogin, _env_road: EnvRoad) -> MachineHost {
    todo!()
}
