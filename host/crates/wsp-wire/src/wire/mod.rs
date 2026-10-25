// SPDX-License-Identifier: AGPL-3.0-only
//! The host's wire: packages/protocol/src/wire, one module per TS file.

pub mod capabilities;
pub mod daemon;
pub mod daemon_replies;
pub mod daemon_events;
pub mod daemon_version;
pub mod devices;
pub mod events_union;
pub mod helpers;
pub mod json_text;
pub mod limits;
pub mod machine_link;
pub mod machine_replies;
pub mod place_link;
pub mod runtime;
pub mod runtime_ops;

pub use crate::types::check::{parse, Check, SchemaError};
