// SPDX-License-Identifier: AGPL-3.0-only
//! packages/adapter-cursor at a3b870a6e: one Cursor turn as `cursor-agent -p --output-format stream-json`. Spec:
//! spec/cursor.md.

pub mod adapter;
pub mod command;

pub use adapter::{create_cursor_adapter, CursorAdapter, CursorAdapterDeps, CursorSession};
