// SPDX-License-Identifier: AGPL-3.0-only
//! packages/engine/src/image-vault.ts at 65f6b89c7: the image's vault, what the sign-in and secrets stages left on the
//! builder, archived off it before the snapshot so every other place builds its copy of the image from the record.
use std::sync::Arc;

use wsp_wire::wire::machine_link::VaultRoad;

use crate::errors::EngineError;
use crate::machine::Machine;
use crate::vault::VaultOptions;

/// image-vault.ts:14 `ImageVault`.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageVault {
    pub tar: Vec<u8>,
    pub sha256: String,
    /// How many of the paths asked for were on the machine and went into the archive.
    pub paths: u64,
    /// The paths that went in, absolute: what every member of the archive is judged against.
    pub held: Vec<String>,
}

/// image-vault.ts:28. The paths that exist on the machine, in the order asked.
pub async fn present_paths(_machine: &dyn Machine, _paths: &[String]) -> Result<Vec<String>, EngineError> {
    todo!("image-vault.ts:28")
}

/// image-vault.ts:42. The tar of the paths that are on the machine, from `/`, and its sha256.
pub async fn export_image_vault(_machine: &dyn Machine, _paths: &[String], _opts: &VaultOptions) -> Result<ImageVault, EngineError> {
    todo!("image-vault.ts:42")
}

/// image-vault.ts:52. Lands a vault over `/` on a fresh builder, as an overlay.
pub async fn import_image_vault(_machine: Arc<dyn Machine>, _tar: &[u8], _opts: &VaultOptions) -> Result<(), EngineError> {
    todo!("image-vault.ts:52")
}

/// image-vault.ts:58 `VaultMember["kind"]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberKind {
    File,
    Dir,
    Symlink,
    Hardlink,
}

/// image-vault.ts:58 `VaultMember`: one member of a vault archive as the guest's tar would extract it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaultMember {
    /// The name as the header carries it, before any rooting.
    pub name: String,
    pub kind: MemberKind,
    /// A symbolic link's target as written, a hard link's target as the archive names it.
    pub target: Option<String>,
}

/// image-vault.ts:137. Every member of a gzipped tar, read as the guest's tar would extract it. Every divergence is a
/// refusal of the whole archive.
pub fn vault_members(_road: VaultRoad, _tar: &[u8]) -> Result<Vec<VaultMember>, EngineError> {
    todo!("image-vault.ts:137")
}

/// image-vault.ts:224. Why a member may not land, or nothing when it may.
pub fn refused_member(_member: &VaultMember, _held: &[String]) -> Option<String> {
    todo!("image-vault.ts:224")
}

/// image-vault.ts:236. The archive against the paths the seal asked for, read whole before any of it is handed to a
/// machine: one member outside them refuses all of it.
pub fn refuse_foreign_members(_road: VaultRoad, _tar: &[u8], _held: &[String]) -> Result<(), EngineError> {
    todo!("image-vault.ts:236")
}

/// image-vault.ts:249. One rule for the image hash: the recipe the copy is built from and the vault it imports.
pub fn image_hash(_recipe_hash: &str, _vault_sha256: Option<&str>) -> String {
    todo!("image-vault.ts:249")
}
