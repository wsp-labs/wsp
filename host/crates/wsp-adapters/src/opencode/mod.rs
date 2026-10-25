// SPDX-License-Identifier: AGPL-3.0-only
//! packages/adapter-opencode at a3b870a6e: one OpenCode turn as `opencode run --format json`. Spec: spec/opencode.md.

pub mod adapter;
pub mod catalog;
pub mod command;

pub use adapter::{create_opencode_adapter, OpenCodeAdapter, OpenCodeAdapterDeps, OpenCodeSession};
