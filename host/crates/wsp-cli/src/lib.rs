// SPDX-License-Identifier: AGPL-3.0-only
//! The command line's verb table and what it reads (`packages/host/src/verbs.ts`, `verbs/`,
//! and the files of chapter 01's `wsp-cli` row). A socket client of the host: it depends on no
//! runtime crate (rule 1.6).

// HostError is 192 bytes and the signatures the tests call are fixed: boxing or regrouping them is a change of those calls.
#![allow(clippy::result_large_err, clippy::type_complexity, clippy::too_many_arguments, clippy::large_enum_variant)]

pub mod parse_args;
pub mod verbs;

pub mod account;
pub mod cloud;
pub mod cloud_text;
pub mod daemon_fix;
pub mod host_lock;
pub mod hosts;
pub mod init_layout;
pub mod place_signin;
pub mod repo_root;
pub mod serving_home;
pub mod setup_follow;
pub mod signin_relay;
pub mod skill;
pub mod version;
pub mod watch;
