// SPDX-License-Identifier: AGPL-3.0-only
//! The slates area of the runtime: `runtime/src/slates.ts`, `slate-*.ts`, `lazy-slates.ts`,
//! `slate-sources/` and `account/slates.ts`. `wsp-threads` and `wsp-slate` join the
//! dependencies when they land; until then `pending` holds what this crate names from `wsp-slate` and the gaps below
//! it (spec/join.md).

pub mod account;
pub mod lazy_slates;
pub mod pending;
pub mod slate_box;
pub mod slate_files;
pub mod slate_image_fetch;
pub mod slate_images;
pub mod slate_mcp;
pub mod slate_mcp_on;
pub mod slate_message;
pub mod slate_read;
pub mod slate_refused;
pub mod slate_runs;
pub mod slate_sources;
pub mod slate_writes;
pub mod slates;
