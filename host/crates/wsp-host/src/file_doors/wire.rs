// SPDX-License-Identifier: AGPL-3.0-only
//! Packet H6's slots: the server's own doors of `serveRuntime` and the icons port, which `wsp-host` leaves empty today.
use std::rc::Rc;

use wsp_core::types::wiring::{HostEditor, HostFolders, HostSsh, HostTerminalConfig, ProjectLander};
use wsp_machines::agents_read::ServerIcons;
use wsp_serve::serve::{BundlerFn, SshHostsFn};

use crate::wiring::Wiring;

/// `bundlerFor` (`project-bundle.ts`): `ServeOptions.projects`.
pub fn projects(_wiring: &Wiring) -> Option<BundlerFn> {
    None
}

/// `lander` (`server.ts:573-582`): `ServeOptions.landing`.
pub fn landing(_wiring: &Wiring) -> Option<Rc<dyn ProjectLander>> {
    None
}

/// `hostFolders` (`host-folders.ts`): `ServeOptions.folders`.
pub fn folders(_wiring: &Wiring) -> Option<Rc<dyn HostFolders>> {
    None
}

/// `readGhosttyConfig` (`wsp-collect/src/ghostty_config.rs`): `ServeOptions.terminal_config`.
pub fn terminal_config(_wiring: &Wiring) -> Option<Rc<dyn HostTerminalConfig>> {
    None
}

/// `editorHost` (`editor.ts`): `ServeOptions.editor`.
pub fn editor(_wiring: &Wiring) -> Option<Rc<dyn HostEditor>> {
    None
}

/// `sshDoor(relay, sshFiles(...), ...)` (`server.ts:581`): `ServeOptions.ssh`.
pub fn ssh(_wiring: &Wiring) -> Option<Rc<dyn HostSsh>> {
    None
}

/// `sshHostsIn` (`ssh-hosts.ts`): `ServeOptions.ssh_hosts`.
pub fn ssh_hosts(_wiring: &Wiring) -> Option<SshHostsFn> {
    None
}

/// `serverIcons({ dir: <state>/icons })` (`server-icons.ts:52-190`, `cli/wiring.ts:365`): `RuntimeOptions.server_icons`.
pub fn server_icons(_wiring: &Wiring) -> Option<Rc<dyn ServerIcons>> {
    None
}
