// SPDX-License-Identifier: AGPL-3.0-only
//! What a wsp link is built out of: the ed25519 pair each end proves itself with, a key's fingerprint, and the
//! AES-256-GCM seal every frame after a handshake travels inside. Port of packages/keys/src/index.ts at a3b870a6e;
//! the spec is spec/keys.md. The daemon's twin is daemon/crates/wsp-seal, held to the same bytes by
//! daemon/fixtures/place-link-seal.json and place-link-signing.json.

use std::fmt;

use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use base64::Engine;
use ed25519_dalek::pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePublicKey};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand_core::{OsRng, RngCore};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::digest::{digest, SHA256};
use ring::hkdf::{self, Salt, HKDF_SHA256};
use x25519_dalek::{PublicKey, StaticSecret};

/// One info word per direction, so the two keys of a link can never be swapped (index.ts:67).
pub const HOST_TO_PLACE_INFO: &str = "wsp place link host to place";
pub const PLACE_TO_HOST_INFO: &str = "wsp place link place to host";

/// The one sentence every frame that will not open is refused with (index.ts:75).
pub const SEAL_REFUSAL: &str = "a frame on this link did not open under the key both ends agreed";

const KEY_BYTES: usize = 32;

/// A key, a PEM or a peer's public value that does not parse; the TS throws node's own error there.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct KeyError(pub String);

/// A frame refused on a sealed link: always `SEAL_REFUSAL`, whichever check caught it (index.ts:75).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a frame on this link did not open under the key both ends agreed")]
pub struct SealRefusal;

/// Base64 read as node's `Buffer.from(s, "base64")` reads it: both alphabets, characters outside them skipped,
/// padding optional, and nothing read past the first `=`.
pub fn node_base64(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut bits: u32 = 0;
    let mut width = 0;
    for c in text.encode_utf16().map(|unit| unit as u8) {
        if c == b'=' {
            break;
        }
        let value = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => continue,
        };
        bits = (bits << 6) | u32::from(value);
        width += 6;
        if width >= 8 {
            width -= 8;
            out.push((bits >> width) as u8);
        }
    }
    out
}

/// `SHA256:` and the unpadded base64 sha256 of the key's bytes, as ssh-keygen prints it (index.ts:23).
pub fn key_fingerprint(key_base64: &str) -> String {
    format!("SHA256:{}", STANDARD_NO_PAD.encode(digest(&SHA256, &node_base64(key_base64))))
}

/// An ed25519 pair: the public key as SPKI DER base64, the private key as PKCS#8 PEM, node's spellings (index.ts:29).
#[derive(Clone, PartialEq, Eq)]
pub struct PlaceKeyPair {
    pub public_key: String,
    pub private_key_pem: String,
}

impl fmt::Debug for PlaceKeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlaceKeyPair").field("public_key", &self.public_key).finish_non_exhaustive()
    }
}

/// A fresh ed25519 pair in those two spellings (index.ts:36).
pub fn new_place_key_pair() -> PlaceKeyPair {
    let seed = random_32();
    let signing = SigningKey::from_bytes(&seed);
    let mut pkcs8 = PKCS8_V1_ED25519_PREFIX.to_vec();
    pkcs8.extend_from_slice(&seed);
    PlaceKeyPair {
        public_key: STANDARD.encode(
            signing.verifying_key().to_public_key_der().expect("an ed25519 public key encodes as SPKI").as_bytes(),
        ),
        private_key_pem: format!("-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n", STANDARD.encode(pkcs8)),
    }
}

/// PKCS#8 v1 for an ed25519 seed, the form node writes: no public key attached.
const PKCS8_V1_ED25519_PREFIX: [u8; 16] = [0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20];

/// The base64 ed25519 signature over `bytes` by the PKCS#8 PEM key; a PEM that does not parse is an error
/// (index.ts:51).
pub fn sign_place_bytes(private_key_pem: &str, bytes: &[u8]) -> Result<String, KeyError> {
    let key = SigningKey::from_pkcs8_der(&pkcs8_der_of(private_key_pem)?).map_err(|e| KeyError(e.to_string()))?;
    Ok(STANDARD.encode(key.sign(bytes).to_bytes()))
}

/// The DER of a PKCS#8 PEM, read line by line as OpenSSL reads it: each marker must stand alone on its line, a blank
/// line in the body is refused, and ASCII whitespace inside the body is dropped. Text after END is never read.
fn pkcs8_der_of(pem: &str) -> Result<Vec<u8>, KeyError> {
    const BEGIN: &str = "-----BEGIN PRIVATE KEY-----";
    const END: &str = "-----END PRIVATE KEY-----";
    let mut lines = pem.split('\n');
    if !lines.any(|line| line == BEGIN) {
        return Err(KeyError("no PKCS#8 PEM begins in this text".to_string()));
    }
    let mut body = String::new();
    for line in lines {
        if line == END {
            return STANDARD.decode(body).map_err(|e| KeyError(e.to_string()));
        }
        if line.is_empty() {
            return Err(KeyError("a PKCS#8 PEM has a blank line in its body".to_string()));
        }
        body.extend(line.chars().filter(|c| !c.is_ascii_whitespace()));
    }
    Err(KeyError("a PKCS#8 PEM has no END line".to_string()))
}

/// Whether the SPKI DER key made the signature; a key or a signature that does not parse is false, never an error
/// (index.ts:57).
pub fn verify_place_bytes(public_key_base64: &str, bytes: &[u8], signature_base64: &str) -> bool {
    let Ok(key) = VerifyingKey::from_public_key_der(&node_base64(public_key_base64)) else {
        return false;
    };
    let Ok(signature) = Signature::from_slice(&node_base64(signature_base64)) else {
        return false;
    };
    key.verify(bytes, &signature).is_ok()
}

/// The two keys of one link, one per direction (index.ts:78).
#[derive(Clone, PartialEq, Eq)]
pub struct SealKeys {
    pub host_to_place: [u8; 32],
    pub place_to_host: [u8; 32],
}

impl fmt::Debug for SealKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SealKeys").finish_non_exhaustive()
    }
}

/// HKDF-SHA256 of the agreed secret, the place id's UTF-8 as the salt, one info word per direction (index.ts:85).
pub fn seal_keys(secret: &[u8], place_id: &str) -> SealKeys {
    SealKeys {
        host_to_place: derive(secret, place_id, HOST_TO_PLACE_INFO),
        place_to_host: derive(secret, place_id, PLACE_TO_HOST_INFO),
    }
}

struct KeyLen;

impl hkdf::KeyType for KeyLen {
    fn len(&self) -> usize {
        KEY_BYTES
    }
}

fn derive(secret: &[u8], place_id: &str, info: &str) -> [u8; KEY_BYTES] {
    let mut key = [0u8; KEY_BYTES];
    Salt::new(HKDF_SHA256, place_id.as_bytes())
        .extract(secret)
        .expand(&[info.as_bytes()], KeyLen)
        .expect("32 bytes is within HKDF-SHA256's limit")
        .fill(&mut key)
        .expect("the output is 32 bytes");
    key
}

/// One end's X25519 private key for one attempt, never held past its socket (index.ts:99).
pub struct EphemeralKey {
    secret: StaticSecret,
}

impl EphemeralKey {
    /// The key from a JWK's `d`, base64url: how the TS tests rebuild the fixture's ephemerals
    /// (`createPrivateKey({ format: "jwk" })`).
    pub fn from_jwk_d(d: &str) -> Result<EphemeralKey, KeyError> {
        let bytes: [u8; KEY_BYTES] = node_base64(d).try_into().map_err(|_| not_32_bytes())?;
        Ok(EphemeralKey { secret: StaticSecret::from(bytes) })
    }
}

/// The key's X25519 public value as the raw 32 bytes the wire carries, base64 with padding (index.ts:91).
pub fn raw_public_key(key: &EphemeralKey) -> String {
    STANDARD.encode(PublicKey::from(&key.secret).as_bytes())
}

/// A peer's X25519 public value read off the wire; anything that is not 32 bytes is an error (index.ts:94).
pub fn public_key_of(raw: &str) -> Result<[u8; 32], KeyError> {
    node_base64(raw).try_into().map_err(|_| not_32_bytes())
}

/// A fresh pair for one attempt: the private key and its raw public value, base64 (index.ts:99).
pub fn fresh_ephemeral() -> (EphemeralKey, String) {
    let key = EphemeralKey { secret: StaticSecret::from(random_32()) };
    let public = raw_public_key(&key);
    (key, public)
}

/// The 32 bytes the two ephemerals agree on; a peer value that is not 32 bytes, or a low-order point whose agreement
/// is all zero, is an error as node's derivation is (index.ts:105).
pub fn shared_secret(private_key: &EphemeralKey, peer: &str) -> Result<[u8; 32], KeyError> {
    let peer_key = PublicKey::from(public_key_of(peer)?);
    let shared = private_key.secret.diffie_hellman(&peer_key);
    if !shared.was_contributory() {
        return Err(KeyError("the peer's key is of low order and agrees on nothing".to_string()));
    }
    Ok(shared.to_bytes())
}

/// Four zero bytes then the counter, big endian: one nonce per frame per direction, so no key ever seals two
/// frames under the same nonce and a frame moved or repeated does not open.
pub fn seal_nonce(counter: u64) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[4..].copy_from_slice(&counter.to_be_bytes());
    nonce
}

/// The text's UTF-8 sealed at the counter: ciphertext, then the 16-byte tag (index.ts:116).
pub fn seal_with(key: &[u8; 32], counter: u64, text: &str) -> Vec<u8> {
    let mut in_out = text.as_bytes().to_vec();
    aead_key(key)
        .seal_in_place_append_tag(Nonce::assume_unique_for_key(seal_nonce(counter)), Aad::empty(), &mut in_out)
        .expect("AES-256-GCM seals any text");
    in_out
}

/// The text back, decoded as UTF-8 with a replacement character for a bad sequence as node's toString does; a frame
/// under 16 bytes, a bad tag and a wrong counter all refuse alike (index.ts:123).
pub fn unseal_with(key: &[u8; 32], counter: u64, bytes: &[u8]) -> Result<String, SealRefusal> {
    let mut in_out = bytes.to_vec();
    let plain = aead_key(key)
        .open_in_place(Nonce::assume_unique_for_key(seal_nonce(counter)), Aad::empty(), &mut in_out)
        .map_err(|_| SealRefusal)?;
    Ok(String::from_utf8_lossy(plain).into_owned())
}

/// Which end of the link a seal is: the host seals with host-to-place and opens place-to-host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Host,
    Place,
}

/// One end's view of a sealed link, each direction counting on its own from 0 (index.ts:137).
pub struct Seal {
    outward: [u8; 32],
    inward: [u8; 32],
    sent: u64,
    taken: u64,
}

impl Seal {
    /// Seals at the outward counter, then counts it.
    pub fn seal(&mut self, text: &str) -> Vec<u8> {
        let bytes = seal_with(&self.outward, self.sent, text);
        self.sent += 1;
        bytes
    }

    /// Opens at the inward counter; the counter moves only when the frame opened.
    pub fn unseal(&mut self, bytes: &[u8]) -> Result<String, SealRefusal> {
        let text = unseal_with(&self.inward, self.taken, bytes)?;
        self.taken += 1;
        Ok(text)
    }
}

/// The seal for one side of a link (index.ts:152).
pub fn make_seal(keys: &SealKeys, side: Side) -> Seal {
    let (outward, inward) = match side {
        Side::Host => (keys.host_to_place, keys.place_to_host),
        Side::Place => (keys.place_to_host, keys.host_to_place),
    };
    Seal { outward, inward, sent: 0, taken: 0 }
}

/// One frame off a socket as the websocket hands it.
#[derive(Debug, Clone, Copy)]
pub enum Frame<'a> {
    Text(&'a str),
    Binary(&'a [u8]),
}

/// The text of one frame: as it came before a seal (a binary frame decoded as UTF-8, lossily); after it, a text frame
/// is refused and a binary one is opened (index.ts:146).
pub fn open_frame(seal: Option<&mut Seal>, raw: Frame<'_>) -> Result<String, SealRefusal> {
    match (seal, raw) {
        (None, Frame::Text(text)) => Ok(text.to_string()),
        (None, Frame::Binary(bytes)) => Ok(String::from_utf8_lossy(bytes).into_owned()),
        (Some(_), Frame::Text(_)) => Err(SealRefusal),
        (Some(seal), Frame::Binary(bytes)) => seal.unseal(bytes),
    }
}

fn aead_key(key: &[u8; 32]) -> LessSafeKey {
    LessSafeKey::new(UnboundKey::new(&AES_256_GCM, key).expect("a 32-byte key is an AES-256 key"))
}

fn random_32() -> [u8; 32] {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

fn not_32_bytes() -> KeyError {
    KeyError("an X25519 public value is 32 bytes".to_string())
}
