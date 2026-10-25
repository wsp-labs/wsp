// SPDX-License-Identifier: AGPL-3.0-only
//! What a computer holds, read for a recipe. Port of packages/collect at a3b870a6e; each module names its TS file
//! and its spec under spec/. recipe.ts and live.ts are not ported: the dead-code cut (#2441) archives them.
#![allow(clippy::result_large_err)] // HostError is wsp-words' error type, 192 bytes, and every refusal carries it

pub mod brew_bottles;
pub mod cache_names;
pub mod collect;
pub mod credential_signals;
pub mod data;
pub mod detect;
pub mod ghostty_config;
pub mod history;
pub mod host;
pub mod live_host;
pub mod manifest;
pub mod project;
pub mod seed;
pub mod ssh_config;
