// SPDX-License-Identifier: AGPL-3.0-only
//! Packet 5a's roads: what a golden built by this host is given, its daemon deploy and its environment. A host answering
//! `None` builds a golden with neither, as `wsp-host` does today.
use wsp_core::types::wiring::DeployDaemon;

use crate::context::{Env, HostWiring};

/// `claudeEnvs` (`cli/wiring.ts:43-55`): `GoldenRecipe.envs`.
pub fn golden_envs(_wiring: &HostWiring) -> Option<Env> {
    None
}

/// `deployHostDaemon` (`cli/wiring.ts:43-55`): `GoldenRecipe.deploy_daemon`.
pub fn deploy_daemon(_wiring: &HostWiring) -> Option<DeployDaemon> {
    None
}
