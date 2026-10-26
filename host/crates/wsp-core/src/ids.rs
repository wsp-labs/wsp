// SPDX-License-Identifier: AGPL-3.0-only
//! The ids seam of rule 3.16: every UUID, random byte and `Math.random()` draw of the host, so a replay can seed them.
//! No TS file of its own: the TS reads `node:crypto` and `Math.random` directly.

use rand::rngs::OsRng;
use rand::{Rng, RngCore};

/// What the host draws its randomness from.
pub trait Ids {
    /// `randomUUID()`.
    fn uuid(&self) -> String;
    /// `randomBytes(n)`.
    fn bytes(&self, n: usize) -> Vec<u8>;
    /// `Math.random()`: a draw in [0, 1).
    fn random(&self) -> f64;
}

/// The OS's randomness, as `node:crypto` reads it.
#[derive(Debug, Clone, Copy, Default)]
pub struct OsIds;

impl Ids for OsIds {
    fn uuid(&self) -> String {
        let mut b = self.bytes(16);
        b[6] = (b[6] & 0x0f) | 0x40;
        b[8] = (b[8] & 0x3f) | 0x80;
        let h = hex::encode(b);
        format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
    }
    fn bytes(&self, n: usize) -> Vec<u8> {
        let mut out = vec![0u8; n];
        OsRng.fill_bytes(&mut out);
        out
    }
    fn random(&self) -> f64 {
        OsRng.gen::<f64>()
    }
}
