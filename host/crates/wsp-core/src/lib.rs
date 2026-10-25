// SPDX-License-Identifier: AGPL-3.0-only
//! The host's core: the wsp-core row of the spec's crate table (chapter 01, Proposal 1), ported from
//! packages/runtime/src at 65f6b89c7 (the dead-code cut, #2441) less the files the area crates take.
//! One module per TS file; each names its TS file and its spec under spec/.
pub mod adapters;
pub mod agent_setup;
pub mod clock;
pub mod console;
pub mod context;
pub mod daemon_channel;
pub mod daemon_roots;
pub mod daemon_token;
pub mod devices;
pub mod forwards;
pub mod github_cache;
pub mod harness_catalog;
pub mod host_id;
pub mod ids;
pub mod idle;
pub mod local_exec;
pub mod machine_exec;
pub mod pipe;
pub mod place_forward;
pub mod plan_alerts;
pub mod project_landing;
pub mod project_sources;
pub mod reach;
pub mod recipe_sync;
pub mod setup_graph;
pub mod sign_in_socket;
pub mod status;
pub mod store;
pub mod sys_socket;
pub mod tunnel;
pub mod types;
