// SPDX-License-Identifier: AGPL-3.0-only
//! Node's `Buffer.from(text, "base64")`, which the tunnel's data frames and the exec road's reap chunks are read with.
use base64::engine::{general_purpose::NO_PAD, GeneralPurpose};
use base64::{alphabet, Engine as _};

const LENIENT: GeneralPurpose = GeneralPurpose::new(&alphabet::STANDARD, NO_PAD.with_decode_allow_trailing_bits(true));

/// Reads `text` as node does: both alphabets (`-` and `_` are `+` and `/`), characters outside them skipped, the first
/// `=` the end, a missing padding fine and a lone last character dropped.
pub fn node_base64(text: &str) -> Vec<u8> {
    let mut clean: String = text
        .chars()
        .take_while(|&c| c != '=')
        .filter_map(|c| match c {
            '-' => Some('+'),
            '_' => Some('/'),
            c if c.is_ascii_alphanumeric() || c == '+' || c == '/' => Some(c),
            _ => None,
        })
        .collect();
    if clean.len() % 4 == 1 {
        clean.pop();
    }
    LENIENT.decode(clean).unwrap_or_default()
}
