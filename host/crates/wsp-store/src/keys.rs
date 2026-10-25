// SPDX-License-Identifier: AGPL-3.0-only
//! What a wsp link is built out of: the ed25519 pair each end proves itself with, a key's fingerprint, and the
//! AES-256-GCM seal every frame after a handshake travels inside. Port of packages/keys/src/index.ts at a3b870a6e;
//! the spec is spec/keys.md. The daemon's twin is daemon/crates/wsp-seal, held to the same bytes by
//! daemon/fixtures/place-link-seal.json and place-link-signing.json.

/// One info word per direction, so the two keys of a link can never be swapped (index.ts:108).
pub const HOST_TO_PLACE_INFO: &str = "wsp place link host to place";
pub const PLACE_TO_HOST_INFO: &str = "wsp place link place to host";

/// The one sentence every frame that will not open is refused with (index.ts:116).
pub const SEAL_REFUSAL: &str = "a frame on this link did not open under the key both ends agreed";

/// A key, a PEM or a peer's public value that does not parse; the TS throws node's own error there.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct KeyError(pub String);

/// A frame refused on a sealed link: always `SEAL_REFUSAL`, whichever check caught it (index.ts:114).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a frame on this link did not open under the key both ends agreed")]
pub struct SealRefusal;

/// Base64 read as node's `Buffer.from(s, "base64")` reads it: both alphabets, characters outside them skipped,
/// padding optional, and nothing read past the first `=`.
pub fn node_base64(text: &str) -> Vec<u8> {
    todo!()
}

/// `SHA256:` and the unpadded base64 sha256 of the key's bytes, as ssh-keygen prints it (index.ts:64).
pub fn key_fingerprint(key_base64: &str) -> String {
    todo!()
}

/// An ed25519 pair: the public key as SPKI DER base64, the private key as PKCS#8 PEM, node's spellings (index.ts:70).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceKeyPair {
    pub public_key: String,
    pub private_key_pem: String,
}

/// A fresh ed25519 pair in those two spellings (index.ts:77).
pub fn new_place_key_pair() -> PlaceKeyPair {
    todo!()
}

/// The base64 ed25519 signature over `bytes` by the PKCS#8 PEM key; a PEM that does not parse is an error
/// (index.ts:92).
pub fn sign_place_bytes(private_key_pem: &str, bytes: &[u8]) -> Result<String, KeyError> {
    todo!()
}

/// Whether the SPKI DER key made the signature; a key or a signature that does not parse is false, never an error
/// (index.ts:98).
pub fn verify_place_bytes(public_key_base64: &str, bytes: &[u8], signature_base64: &str) -> bool {
    todo!()
}

/// The two keys of one link, one per direction (index.ts:119).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealKeys {
    pub host_to_place: [u8; 32],
    pub place_to_host: [u8; 32],
}

/// HKDF-SHA256 of the agreed secret, the place id's UTF-8 as the salt, one info word per direction (index.ts:126).
pub fn seal_keys(secret: &[u8], place_id: &str) -> SealKeys {
    todo!()
}

/// One end's X25519 private key for one attempt, never held past its socket (index.ts:140).
pub struct EphemeralKey {
    _private: (),
}

impl EphemeralKey {
    /// The key from a JWK's `d`, base64url: how the TS tests rebuild the fixture's ephemerals
    /// (`createPrivateKey({ format: "jwk" })`).
    pub fn from_jwk_d(d: &str) -> Result<EphemeralKey, KeyError> {
        todo!()
    }
}

/// The key's X25519 public value as the raw 32 bytes the wire carries, base64 with padding (index.ts:132).
pub fn raw_public_key(key: &EphemeralKey) -> String {
    todo!()
}

/// A peer's X25519 public value read off the wire; anything that is not 32 bytes is an error (index.ts:135).
pub fn public_key_of(raw: &str) -> Result<[u8; 32], KeyError> {
    todo!()
}

/// A fresh pair for one attempt: the private key and its raw public value, base64 (index.ts:140).
pub fn fresh_ephemeral() -> (EphemeralKey, String) {
    todo!()
}

/// The 32 bytes the two ephemerals agree on; a peer value that is not 32 bytes, or a low-order point whose agreement
/// is all zero, is an error as node's derivation is (index.ts:146).
pub fn shared_secret(private_key: &EphemeralKey, peer: &str) -> Result<[u8; 32], KeyError> {
    todo!()
}

/// Four zero bytes, then the counter big endian (index.ts:150).
pub fn seal_nonce(counter: u64) -> [u8; 12] {
    todo!()
}

/// The text's UTF-8 sealed at the counter: ciphertext, then the 16-byte tag (index.ts:157).
pub fn seal_with(key: &[u8; 32], counter: u64, text: &str) -> Vec<u8> {
    todo!()
}

/// The text back, decoded as UTF-8 with a replacement character for a bad sequence as node's toString does; a frame
/// under 16 bytes, a bad tag and a wrong counter all refuse alike (index.ts:164).
pub fn unseal_with(key: &[u8; 32], counter: u64, bytes: &[u8]) -> Result<String, SealRefusal> {
    todo!()
}

/// Which end of the link a seal is: the host seals with host-to-place and opens place-to-host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Host,
    Place,
}

/// One end's view of a sealed link, each direction counting on its own from 0 (index.ts:178).
pub struct Seal {
    _private: (),
}

impl Seal {
    /// Seals at the outward counter, then counts it.
    pub fn seal(&mut self, text: &str) -> Vec<u8> {
        todo!()
    }

    /// Opens at the inward counter; the counter moves only when the frame opened.
    pub fn unseal(&mut self, bytes: &[u8]) -> Result<String, SealRefusal> {
        todo!()
    }
}

/// The seal for one side of a link (index.ts:193).
pub fn make_seal(keys: &SealKeys, side: Side) -> Seal {
    todo!()
}

/// One frame off a socket as the websocket hands it.
#[derive(Debug, Clone, Copy)]
pub enum Frame<'a> {
    Text(&'a str),
    Binary(&'a [u8]),
}

/// The text of one frame: as it came before a seal (a binary frame decoded as UTF-8, lossily); after it, a text frame
/// is refused and a binary one is opened (index.ts:187).
pub fn open_frame(seal: Option<&mut Seal>, raw: Frame<'_>) -> Result<String, SealRefusal> {
    todo!()
}
