// SPDX-License-Identifier: AGPL-3.0-only
//! The account area of the runtime, ported from `packages/runtime/src/`:
//! `account/` less `slates.ts` (which is `wsp-slates`'), `projects/` less
//! `branch-of-record.ts` (which is `wsp-machines`'), and `usage-reset.ts`.
//! One module per TS file.

pub mod account;
pub mod projects;
pub mod usage_reset;
