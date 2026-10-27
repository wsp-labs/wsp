// SPDX-License-Identifier: AGPL-3.0-only
//! `assets.ts`: the table of shipped assets (the web app, the daemon binaries, the wsp command bundle), how each is staged and found.
use wsp_words::exit::HostError;

use crate::context::Env;

/// `ASSET_KINDS` (`assets.ts:15`).
pub const ASSET_KINDS: [&str; 3] = ["web", "daemon", "cli"];

/// `AssetKind` (`assets.ts:16`), the three names of `ASSET_KINDS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetKind {
    Web,
    Daemon,
    Cli,
}

/// `ASSETS_DIR` (`assets.ts:19`).
pub const ASSETS_DIR: &str = "assets";

/// `REQUIRE_DAEMON_ENV` (`assets.ts:79`).
pub const REQUIRE_DAEMON_ENV: &str = "WSP_REQUIRE_DAEMON";

/// `assetName` (`assets.ts:89`).
pub fn asset_name(_kind: AssetKind) -> String {
    todo!()
}

/// `assetProof` (`assets.ts:94`).
pub fn asset_proof(_kind: AssetKind) -> String {
    todo!()
}

/// `stagedAsset` (`assets.ts:99`).
pub fn staged_asset(_package_root: &str, _kind: AssetKind) -> String {
    todo!()
}

/// `stageAsset` (`assets.ts:104`).
pub fn stage_asset(_from: &str, _package_root: &str, _kind: AssetKind) -> Result<String, HostError> {
    todo!()
}

/// `stageAssetOrSkip` (`assets.ts:114`).
pub fn stage_asset_or_skip(_from: &str, _package_root: &str, _kind: AssetKind, _env: &Env) -> Result<Option<String>, HostError> {
    todo!()
}

/// `copyAsset` (`assets.ts:126`).
pub fn copy_asset(_kind: AssetKind, _from: &str, _to: &str) -> Result<(), HostError> {
    todo!()
}

/// `packedAsset` (`assets.ts:134`).
pub fn packed_asset(_from_dir: &str, _kind: AssetKind) -> Option<String> {
    todo!()
}

/// `workspaceAsset` (`assets.ts:140`).
pub fn workspace_asset(_kind: AssetKind) -> String {
    todo!()
}

/// `assetDir` (`assets.ts:149`), where `fromDir` is the folder the default `here()` names.
pub fn asset_dir(_kind: AssetKind, _from_dir: &str) -> String {
    todo!()
}

/// `daemonBinaryHere` (`assets.ts:155`).
pub fn daemon_binary_here(_from_dir: Option<&str>) -> Result<String, HostError> {
    todo!()
}

/// `webDirFor` (`assets.ts:166`).
pub fn web_dir_for(_env: &Env, _from_dir: Option<&str>) -> String {
    todo!()
}
