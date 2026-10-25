// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/vault-cipher.ts. Spec: spec/small.md.
//!
//! One sealed container for the image vault, keyed to a place's X25519 public key: an ephemeral X25519 key,
//! HKDF-SHA256, AES-256-GCM, with the header line as the cipher's additional data.
use wsp_wire::views::golden_image::{SealedImage, SealedVaultHeader};

use crate::errors::Result;

/// vault-cipher.ts:10.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenedVault {
    pub image: SealedImage,
    pub plain: Vec<u8>,
}

/// vault-cipher.ts:75. Which side of the place's keypair a call holds, as raw 32-byte X25519 keys. node's KeyObject
/// spelling of a public key is SPKI DER: the 12 bytes `302a300506032b656e032100` then these 32.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultKey {
    Public([u8; 32]),
    Private([u8; 32]),
}

pub const KEY_BYTES: usize = 32;
pub const NONCE_BYTES: usize = 12;
pub const TAG_BYTES: usize = 16;
pub const SALT_BYTES: usize = 16;

/// vault-cipher.ts:85. `Private` is refused with `sealing to a place needs its public key`. The output is the header's
/// JSON (keys format, cipher, to, salt, nonce, ephemeral, image, in that order) and a newline, then the ciphertext and
/// its 16-byte tag. salt 16 and nonce 12 random bytes; ephemeral the SPKI DER of a fresh key; all three standard base64.
pub fn seal(plain: &[u8], image: &SealedImage, to: &VaultKey) -> Result<Vec<u8>> {
    todo!()
}

/// vault-cipher.ts:97. Refusals, in order: `Public` key; no newline; a header line that is not JSON or not a
/// SealedVaultHeader; `to` not `key`; no ephemeral; a body shorter than its tag; a failed open (wrong key, edited header
/// or body), each with the words in spec/small.md.
pub fn open(sealed: &[u8], with: &VaultKey) -> Result<OpenedVault> {
    todo!()
}

/// vault-cipher.ts:109. The record a sealed file names, read without any key; the header refusals as `open`'s.
pub fn sealed_image_of(sealed: &[u8]) -> Result<SealedImage> {
    todo!()
}

/// The header line `open` reads (vault-cipher.ts:40 `unpacked`), for a reader that wants the whole of it.
pub fn header_of(sealed: &[u8]) -> Result<SealedVaultHeader> {
    todo!()
}
