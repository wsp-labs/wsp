// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/key-fingerprint.ts, which re-exports packages/keys/src/index.ts:23. Spec: spec/small.md.

/// keys/src/index.ts:23. `SHA256:` then the unpadded standard base64 of the sha256 of the key's bytes, decoded from
/// `key_base64` the way node's `Buffer.from(s, "base64")` decodes: url-safe letters accepted, characters outside the
/// alphabet skipped, padding optional, decoding stopping at the first `=`.
pub fn key_fingerprint(key_base64: &str) -> String {
    use base64::engine::general_purpose::STANDARD_NO_PAD;
    use base64::Engine;
    use sha2::{Digest, Sha256};
    format!("SHA256:{}", STANDARD_NO_PAD.encode(Sha256::digest(crate::node_base64::node_base64_decode(key_base64))))
}
