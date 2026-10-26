// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/sources.ts at 65f6b89c7.
/// The keys of SLATE_SOURCES (sources.ts), in the order the registry lists them.
pub const SLATE_SOURCE_NAMES: &[&str] =
    &["thread", "tree", "usage", "cost", "time", "git", "pr", "machine", "processes", "ports", "notifications", "mcp"];

pub fn is_slate_source(name: &str) -> bool {
    SLATE_SOURCE_NAMES.contains(&name)
}
