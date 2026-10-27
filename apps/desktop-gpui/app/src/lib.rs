// SPDX-License-Identifier: AGPL-3.0-only
#[cfg(feature = "agent")]
pub mod agent;
pub mod bench;
pub mod desktop;
pub mod frames;
pub mod gallery;
pub mod glass;
pub mod heap;
pub mod in_front;
pub mod local_store;
pub mod prefs;
pub mod screens;
pub mod scroll_trace;
pub mod service;
pub mod shell;
pub mod sidebar;
pub mod thread_view;
pub mod transcript;
pub mod ui;
pub mod update;
pub mod words;

pub const TITLE: &str = "wsp";
