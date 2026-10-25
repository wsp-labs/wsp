// SPDX-License-Identifier: AGPL-3.0-only
//! The release road's scripts: a catalog row's recorded asset by arch, or a person's own row read off the release's
//! listing, with the go fall-through. Port of packages/catalog/src/road-modules.ts:436-563 at a3b870a6e; the spec is
//! spec/road_modules.md.
use wsp_wire::views::golden_image::ToolPin;
use crate::roads::ReleaseAssets;

/// road-modules.ts:472.
pub const ASSET_ARCH_X86_64: &str = "amd64|x86_64|x64";
pub const ASSET_ARCH_AARCH64: &str = "arm64|aarch64";
/// road-modules.ts:475.
pub const ASSET_SKIPPED: &str = "\\.(sha256|sha256sum|sha512|sig|asc|txt|md5|pem|deb|rpm|apk|zst|tar\\.zst|json)$";

/// The script for a row the catalog recorded assets for (road-modules.ts:481).
pub fn asset_install(name: &str, repo: &str, tag: &str, assets: &ReleaseAssets, go: Option<&str>) -> String {
    todo!()
}

/// The script for a person's own row with no assets (road-modules.ts:518).
pub fn release_install(name: &str, repo: &str, tag: &str, pin: Option<&ToolPin>, go: Option<&str>) -> String {
    todo!()
}
