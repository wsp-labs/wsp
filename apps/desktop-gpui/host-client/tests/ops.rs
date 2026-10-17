// SPDX-License-Identifier: AGPL-3.0-only
//! Every op the web app sends, called typed against one throwaway host the whole binary shares (tests/common), grouped
//! by the screen that sends it. A call passes when the host answers with a reply that reads as its type, or refuses it
//! in its own words where the throwaway host cannot do what was asked (no GitHub, no provider, no other computer).
//! A frame the host's schema refuses, or a reply that does not read, fails whichever the test expected.

mod common;

#[path = "ops/activity.rs"]
mod activity;
#[path = "ops/dialogs.rs"]
mod dialogs;
#[path = "ops/pull_request.rs"]
mod pull_request;
#[path = "ops/settings.rs"]
mod settings;
#[path = "ops/settings_agents.rs"]
mod settings_agents;
#[path = "ops/sidebar.rs"]
mod sidebar;
#[path = "ops/slates.rs"]
mod slates;
#[path = "ops/terminal.rs"]
mod terminal;
#[path = "ops/thread.rs"]
mod thread;
#[path = "ops/usage.rs"]
mod usage;
