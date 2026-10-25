// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/vault-cipher.ts. Spec: spec/small.md.
//!
//! One sealed container for the image vault, keyed to a place's X25519 public key: an ephemeral X25519 key,
//! HKDF-SHA256, AES-256-GCM, with the header line as the cipher's additional data.
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use hkdf::Hkdf;
use sha2::Sha256;
use wsp_wire::types::check::parse;
use wsp_wire::views::golden_image::{SealedImage, SealedVaultHeader, SealedVaultHeaderTo};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::errors::{EngineError, Result};

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

/// The SPKI DER prefix of an X25519 public key.
const X25519_SPKI_PREFIX: [u8; 12] = [0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x6e, 0x03, 0x21, 0x00];

fn refuse(message: &str) -> EngineError {
    EngineError::Other { message: message.to_string(), code: None }
}

/// The shared secret stretched to a key: HKDF-SHA256 over `shared`, keyed by `salt`, with `wsp-vault-1` and the
/// ephemeral key's DER as the info.
fn derive(shared: &[u8; 32], salt: &[u8], ephemeral_der: &[u8]) -> [u8; KEY_BYTES] {
    let info = [b"wsp-vault-1".as_slice(), ephemeral_der].concat();
    let mut key = [0u8; KEY_BYTES];
    Hkdf::<Sha256>::new(Some(salt), shared).expand(&info, &mut key).expect("32 bytes is a valid HKDF length");
    key
}

/// The DER of an X25519 public key, which node writes as SPKI.
fn spki_der(public: &[u8; 32]) -> Vec<u8> {
    [X25519_SPKI_PREFIX.as_slice(), public].concat()
}

/// The public key inside an SPKI DER of an X25519 key: the last 32 bytes, once the prefix checks out.
fn public_of_spki(der: &[u8]) -> Option<[u8; 32]> {
    (der.len() == X25519_SPKI_PREFIX.len() + 32 && der.starts_with(&X25519_SPKI_PREFIX)).then(|| der[12..].try_into().ok()).flatten()
}

/// vault-cipher.ts:85. `Private` is refused with `sealing to a place needs its public key`. The output is the header's
/// JSON (keys format, cipher, to, salt, nonce, ephemeral, image, in that order) and a newline, then the ciphertext and
/// its 16-byte tag. salt 16 and nonce 12 random bytes; ephemeral the SPKI DER of a fresh key; all three standard base64.
pub fn seal(plain: &[u8], image: &SealedImage, to: &VaultKey) -> Result<Vec<u8>> {
    let VaultKey::Public(their_public) = to else {
        return Err(refuse("sealing to a place needs its public key"));
    };
    let secret = StaticSecret::random_from_rng(rand::thread_rng());
    let ephemeral = spki_der(PublicKey::from(&secret).as_bytes());
    let salt: [u8; SALT_BYTES] = rand::random();
    let nonce: [u8; NONCE_BYTES] = rand::random();
    let shared = secret.diffie_hellman(&PublicKey::from(*their_public));
    let key = derive(shared.as_bytes(), &salt, &ephemeral);
    let header = serde_json::json!({
        "format": "wsp-vault-1",
        "cipher": "aes-256-gcm",
        "to": "key",
        "salt": STANDARD.encode(salt),
        "nonce": STANDARD.encode(nonce),
        "ephemeral": STANDARD.encode(&ephemeral),
        "image": serde_json::to_value(image).expect("a sealed image serializes"),
    });
    let mut line = serde_json::to_vec(&header).expect("a header serializes");
    line.push(b'\n');
    let body = Aes256Gcm::new_from_slice(&key)
        .expect("a 32-byte key")
        .encrypt(Nonce::from_slice(&nonce), Payload { msg: plain, aad: &line })
        .map_err(|_| refuse("the sealed vault could not be encrypted"))?;
    Ok([line, body].concat())
}

/// The header line and the ciphertext behind it, the header parsed as `SealedVaultHeader`. Refusals, in the TS's order:
/// no newline; a line that is not JSON, or not a header wsp wrote.
fn unpacked(sealed: &[u8]) -> Result<(SealedVaultHeader, &[u8], &[u8])> {
    let Some(at) = sealed.iter().position(|&b| b == b'\n') else {
        return Err(refuse("this file is not a sealed wsp image: it has no header line"));
    };
    let header = serde_json::from_slice::<serde_json::Value>(&sealed[..at])
        .ok()
        .and_then(|value| parse::<SealedVaultHeader>(&value).ok())
        .ok_or_else(|| refuse("this file is not a sealed wsp image: its header is not one wsp wrote"))?;
    Ok((header, &sealed[..=at], &sealed[at + 1..]))
}

/// vault-cipher.ts:97. Refusals, in order: `Public` key; no newline; a header line that is not JSON or not a
/// SealedVaultHeader; `to` not `key`; no ephemeral; a body shorter than its tag; a failed open (wrong key, edited header
/// or body), each with the words in spec/small.md.
pub fn open(sealed: &[u8], with: &VaultKey) -> Result<OpenedVault> {
    let VaultKey::Private(my_secret) = with else {
        return Err(refuse("opening a vault sealed to a place needs that place's private key"));
    };
    let (header, aad, body) = unpacked(sealed)?;
    if header.to != SealedVaultHeaderTo::Key {
        return Err(refuse("this vault was sealed to a passphrase, not to a place's key"));
    }
    let Some(ephemeral_b64) = header.ephemeral.as_deref() else {
        return Err(refuse("this vault names no ephemeral key and cannot be opened"));
    };
    let ephemeral = STANDARD.decode(ephemeral_b64).map_err(|_| refuse("this vault names no ephemeral key and cannot be opened"))?;
    let peer = public_of_spki(&ephemeral).ok_or_else(|| refuse("this vault names no ephemeral key and cannot be opened"))?;
    let salt = STANDARD.decode(&header.salt).unwrap_or_default();
    let nonce = STANDARD.decode(&header.nonce).unwrap_or_default();
    let shared = StaticSecret::from(*my_secret).diffie_hellman(&PublicKey::from(peer));
    let key = derive(shared.as_bytes(), &salt, &ephemeral);
    if body.len() < TAG_BYTES {
        return Err(refuse("the sealed vault is shorter than its own tag"));
    }
    let would_not_open = || refuse("the sealed vault would not open: the passphrase or the key is not the one it was sealed to, or the file was changed");
    if nonce.len() != NONCE_BYTES {
        return Err(would_not_open());
    }
    let plain = Aes256Gcm::new_from_slice(&key)
        .expect("a 32-byte key")
        .decrypt(Nonce::from_slice(&nonce), Payload { msg: body, aad })
        .map_err(|_| would_not_open())?;
    Ok(OpenedVault { image: header.image, plain })
}

/// vault-cipher.ts:109. The record a sealed file names, read without any key; the header refusals as `open`'s.
pub fn sealed_image_of(sealed: &[u8]) -> Result<SealedImage> {
    Ok(unpacked(sealed)?.0.image)
}

/// The header line `open` reads (vault-cipher.ts:40 `unpacked`), for a reader that wants the whole of it.
pub fn header_of(sealed: &[u8]) -> Result<SealedVaultHeader> {
    Ok(unpacked(sealed)?.0)
}
