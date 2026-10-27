// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `pick-sizes.ts`: what the picks weigh on a box, read on this computer before anything is sent.
use wsp_wire::types::recipe_file::RecipeFile;

/// The `{ bytes, unmeasured }` of `estimatePicks` (`pick-sizes.ts:50`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PicksEstimate {
    pub bytes: u64,
    pub unmeasured: usize,
}

/// `cliBytes` (`pick-sizes.ts:23`).
pub fn cli_bytes(_via: &str, _name: &str) -> Option<u64> {
    todo!()
}

/// `agentBytes` (`pick-sizes.ts:26`).
pub fn agent_bytes(_id: &str) -> Option<u64> {
    todo!()
}

/// `folderBytes` (`pick-sizes.ts:44`).
pub fn folder_bytes(_path: &str, _keep: &[String]) -> u64 {
    todo!()
}

/// `estimatePicks` (`pick-sizes.ts:50`).
pub fn estimate_picks(_picks: &RecipeFile, _home: &str) -> PicksEstimate {
    todo!()
}
