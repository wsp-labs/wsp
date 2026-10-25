// SPDX-License-Identifier: AGPL-3.0-only
//! The machines area of the runtime, ported from `packages/runtime/src/`:
//! `machines/`, `places/`, `images/`, `places.ts`, `agents-read.ts`,
//! `usage.ts` and `projects/branch-of-record.ts`. One module per TS file.

pub mod agents_read;
pub mod images;
pub mod machines;
pub mod places;
pub mod projects;
pub mod usage;
