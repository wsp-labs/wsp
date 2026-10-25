// SPDX-License-Identifier: AGPL-3.0-only
//! The runtime's op switch and the assembly of its areas: the wsp-serve row of the spec's crate table
//! (chapter 01, Proposal 1), ported from packages/runtime/src at 65f6b89c7 (the dead-code cut, #2441).
//! One module per TS file; `index.ts`, the package's barrel, is this file's re-exports.
pub mod runtime;
pub mod serve;
pub mod serve_app_procs;
pub mod serve_broker;
pub mod serve_conversations;
pub mod serve_processes;
pub mod serve_projects;
pub mod serve_slates;
pub mod serve_start;
