// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/workspaces-help.ts` at 65f6b89c7, in three units: `reads` (listings, picking a thread or
//! a workspace, waking, napping, the image, ssh), `life` (commits, fixes, merges, starts, reviews, stops, forgets,
//! settles, renames) and `rows` (dropping and deleting a workspace, the thread rows and tree, projects, creating a
//! workspace, sizes). Specs: `spec/ws-reads.md`, `spec/ws-life.md`, `spec/ws-rows.md`.

pub mod life;
pub mod reads;
pub mod rows;

pub use life::*;
pub use reads::*;
pub use rows::*;
