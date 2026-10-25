// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-mcp.ts at 65f6b89c7, in two units: `client` (the types, the refusals of a write
//! from away, the approval key, the two clients and the schema folding; spec/slate_mcp_client.md) and `runs`
//! (`createSlateMcp`; spec/slate_mcp_runs.md).

pub mod client;
pub mod runs;

pub use client::*;
pub use runs::*;
