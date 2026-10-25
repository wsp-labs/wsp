// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/local-backend.ts.

/// local-backend.ts: the memory this computer has, free and in all. Owned by the roads writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryHere {
    pub free_mb: u64,
    pub total_mb: u64,
}
