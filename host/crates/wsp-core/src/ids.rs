// SPDX-License-Identifier: AGPL-3.0-only
//! The ids seam of rule 3.16: every UUID, random byte and `Math.random()` draw of the host, so a replay can seed them.
//! No TS file of its own: the TS reads `node:crypto` and `Math.random` directly.

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
        todo!()
    }
    fn bytes(&self, n: usize) -> Vec<u8> {
        todo!()
    }
    fn random(&self) -> f64 {
        todo!()
    }
}
