// SPDX-License-Identifier: AGPL-3.0-only
//! `wsp-own.ts`: wsp's own command line among a computer's global packages, found by the command each one installs.
use indexmap::IndexSet;
use wsp_collect::host::Host;
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_words::exit::HostError;

/// The `{ name, via }` rows `wspPackages` reads (`wsp-own.ts:12`).
#[derive(Clone, Debug, PartialEq)]
pub struct WspPackageRow {
    pub name: String,
    pub via: String,
}

/// `wspPackages` (`wsp-own.ts:12`).
pub async fn wsp_packages(_host: &Host, _rows: &[WspPackageRow]) -> Result<IndexSet<String>, HostError> {
    todo!()
}

/// `withoutWspHere` (`wsp-own.ts:21`).
pub async fn without_wsp_here(_host: &Host, _file: &RecipeFile) -> Result<RecipeFile, HostError> {
    todo!()
}
