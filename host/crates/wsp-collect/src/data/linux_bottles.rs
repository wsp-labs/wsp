// SPDX-License-Identifier: AGPL-3.0-only
//! The checked-in bottle snapshot, embedded from data/linux-bottles.json, which data/linux_bottles.py writes from
//! packages/collect/src/data/linux-bottles.ts. Refresh both by running that script; tests/brew_bottles.rs pins the
//! file's hash and its agreement with the TS table.
use std::sync::LazyLock;

use crate::brew_bottles::BottleSnapshot;

/// The embedded file's text, byte for byte.
pub const LINUX_BOTTLES_JSON: &str = include_str!("../../data/linux-bottles.json");

/// LINUX_BOTTLES_JSON parsed once; a file that does not parse is a build mistake and panics on first use.
pub static LINUX_BOTTLES: LazyLock<BottleSnapshot> = LazyLock::new(|| todo!());
