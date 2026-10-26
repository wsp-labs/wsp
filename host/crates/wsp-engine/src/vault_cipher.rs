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
use crate::node_base64::node_base64_decode;

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

/// The SPKI DER prefix of an Ed25519 public key.
const ED25519_SPKI_PREFIX: [u8; 12] = [0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];

/// `createPublicKey({ key, type: "spki", format: "der" })` for the keys a vault could name: the 32 bytes after an X25519
/// prefix (bytes past them are ignored, as node's reader ignores them), refused in node's words where the DER is no key
/// at all, or a key of a kind the exchange cannot use.
fn public_of_spki(der: &[u8]) -> Result<[u8; 32]> {
    let key_at = |prefix: &[u8; 12]| der.starts_with(prefix).then(|| der.get(12..44)).flatten();
    if let Some(key) = key_at(&X25519_SPKI_PREFIX) {
        return key.try_into().map_err(|_| refuse("Failed to read asymmetric key"));
    }
    if key_at(&ED25519_SPKI_PREFIX).is_some() {
        return Err(refuse("Incompatible key types for Diffie-Hellman: x25519 and ed25519"));
    }
    Err(refuse("Failed to read asymmetric key"))
}

/// The error node's `diffieHellman` throws for a shared secret that is all zeros (a low-order public key).
fn derivation_failed() -> EngineError {
    refuse("error:1C8000A4:Provider routines::failed during derivation")
}

/// AES-256-GCM open with a nonce of any length from 1 to 32 bytes, which is what `createDecipheriv` takes (its length
/// is part of the hash that starts the counter). `None` where it does not open.
fn open_gcm(key: &[u8; KEY_BYTES], nonce: &[u8], body: &[u8], aad: &[u8]) -> Option<Vec<u8>> {
    use aes_gcm::aead::consts::*;
    use aes_gcm::aead::generic_array::GenericArray;
    macro_rules! by_length {
        ($($n:ident),*) => {
            match nonce.len() {
                $(l if l == <$n as aes_gcm::aead::generic_array::typenum::Unsigned>::USIZE => aes_gcm::AesGcm::<aes_gcm::aes::Aes256, $n>::new_from_slice(key)
                    .ok()?
                    .decrypt(GenericArray::from_slice(nonce), Payload { msg: body, aad })
                    .ok(),)*
                _ => None,
            }
        };
    }
    by_length!(
        U1, U2, U3, U4, U5, U6, U7, U8, U9, U10, U11, U12, U13, U14, U15, U16, U17, U18, U19, U20, U21, U22, U23, U24, U25, U26, U27, U28,
        U29, U30, U31, U32
    )
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
    if !shared.was_contributory() {
        return Err(derivation_failed());
    }
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

/// The deepest nesting `read_json` follows: far past anything wsp writes, short of what would overflow the stack.
const JSON_DEPTH: usize = 2_000;

/// `JSON.parse` of the header's text, `None` where it is not JSON. node's parser has no depth limit of its own below
/// the stack's, where serde_json stops at 128 levels, so the limit is lifted here and a bound of ours put in its place.
fn read_json(text: &str) -> Option<serde_json::Value> {
    use serde::Deserialize;
    let (mut depth, mut deepest, mut in_string, mut escaped) = (0usize, 0usize, false, false);
    for b in text.bytes() {
        match (in_string, b) {
            (true, _) if escaped => escaped = false,
            (true, b'\\') => escaped = true,
            (true, b'"') => in_string = false,
            (true, _) => {}
            (false, b'"') => in_string = true,
            (false, b'[' | b'{') => {
                depth += 1;
                deepest = deepest.max(depth);
            }
            (false, b']' | b'}') => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    if deepest > JSON_DEPTH {
        return None;
    }
    let mut de = serde_json::Deserializer::from_str(text);
    de.disable_recursion_limit();
    let value = serde_json::Value::deserialize(&mut de).ok()?;
    de.end().ok()?;
    Some(value)
}

/// The header line and the ciphertext behind it, the header parsed as `SealedVaultHeader`. Refusals, in the TS's order:
/// no newline; a line that is not JSON, or not a header wsp wrote.
fn unpacked(sealed: &[u8]) -> Result<(SealedVaultHeader, &[u8], &[u8])> {
    let Some(at) = sealed.iter().position(|&b| b == b'\n') else {
        return Err(refuse("this file is not a sealed wsp image: it has no header line"));
    };
    let header = read_json(&String::from_utf8_lossy(&sealed[..at]))
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
    let ephemeral = node_base64_decode(ephemeral_b64);
    let peer = public_of_spki(&ephemeral)?;
    let shared = StaticSecret::from(*my_secret).diffie_hellman(&PublicKey::from(peer));
    if !shared.was_contributory() {
        return Err(derivation_failed());
    }
    let key = derive(shared.as_bytes(), &node_base64_decode(&header.salt), &ephemeral);
    let nonce = node_base64_decode(&header.nonce);
    if body.len() < TAG_BYTES {
        return Err(refuse("the sealed vault is shorter than its own tag"));
    }
    if nonce.is_empty() {
        return Err(refuse("Invalid initialization vector"));
    }
    let plain = open_gcm(&key, &nonce, body, aad).ok_or_else(|| {
        refuse("the sealed vault would not open: the passphrase or the key is not the one it was sealed to, or the file was changed")
    })?;
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
