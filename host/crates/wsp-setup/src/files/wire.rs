// SPDX-License-Identifier: AGPL-3.0-only
//! Packet 1's roads, answered as `wsp-host` answers them today: the release door, the local daemon's three roads and the
//! prices read are not wired, so each answers `None`.
use std::rc::Rc;

use wsp_core::types::wiring::{HereDaemon, Local};
use wsp_serve::serve::ReleaseDoor;
use wsp_wire::views::workspace::DaemonReachView;

use crate::context::HostWiring;

/// `releaseWatch` made at `cli/serve.ts:293-301`, the `release` slot of `serveRuntime` (`server.ts:597`).
pub fn release_door(_wiring: &HostWiring) -> Option<Rc<dyn ReleaseDoor>> {
    None
}

/// `LocalWiring.daemonRoad` (`cli/wiring.ts`, `local-daemon.ts`).
pub fn daemon_road(_wiring: &HostWiring) -> Option<Rc<dyn Fn() -> Local<'static, DaemonReachView>>> {
    None
}

/// `LocalWiring.hereDaemon`.
pub fn here_daemon(_wiring: &HostWiring) -> Option<Rc<dyn HereDaemon>> {
    None
}

/// `LocalWiring.restartDaemon`.
pub fn restart_daemon(_wiring: &HostWiring) -> Option<Rc<dyn Fn() -> Local<'static, ()>>> {
    None
}

/// `RuntimeOptions.pricesFetch` (`cli/wiring.ts:345`, `PRICES_URL`).
pub fn prices_fetch(_wiring: &HostWiring) -> Option<Rc<dyn Fn() -> Local<'static, serde_json::Value>>> {
    None
}
