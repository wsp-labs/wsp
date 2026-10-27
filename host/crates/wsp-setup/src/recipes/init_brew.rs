// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `init-brew.ts`: this Mac's Homebrew, read once for the Tools screen.
use wsp_collect::host::Host;
use wsp_engine::golden_seam::BrewTable;
use wsp_words::exit::HostError;

/// `readBrewTable` (`init-brew.ts:8`).
pub async fn read_brew_table(_host: &Host) -> Result<BrewTable, HostError> {
    todo!()
}
