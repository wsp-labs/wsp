// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/key-fingerprint.ts, which re-exports packages/keys/src/index.ts:23. Spec: spec/small.md.

/// keys/src/index.ts:23. `SHA256:` then the unpadded standard base64 of the sha256 of the key's bytes, decoded from
/// `key_base64` the way node's `Buffer.from(s, "base64")` decodes: url-safe letters accepted, characters outside the
/// alphabet skipped, padding optional, decoding stopping at the first `=`.
pub fn key_fingerprint(key_base64: &str) -> String {
    use base64::engine::general_purpose::STANDARD_NO_PAD;
    use base64::Engine;
    use sha2::{Digest, Sha256};
    format!("SHA256:{}", STANDARD_NO_PAD.encode(Sha256::digest(node_base64_decode(key_base64))))
}

/// `Buffer.from(s, "base64")`: the sextets of every letter in the alphabet, url-safe letters too, in order; any other
/// character is skipped; decoding stops at the first `=`. Whole bytes come out, a dangling sextet pair is dropped. Each
/// UTF-16 unit is read by its low byte, as node reads a string it holds in two bytes.
pub(crate) fn node_base64_decode(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut bits, mut nbits) = (0u32, 0u32);
    for unit in s.encode_utf16().map(|u| u & 0xff).take_while(|&u| u != u16::from(b'=')) {
        let c = char::from(unit as u8);
        let v = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' | '-' => 62,
            '/' | '_' => 63,
            _ => continue,
        };
        bits = (bits << 6) | v;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((bits >> nbits) as u8);
            bits &= (1 << nbits) - 1;
        }
    }
    out
}
