// SPDX-License-Identifier: AGPL-3.0-only
//! The roads to computers, agents and sign-ins (chapter 01's `wsp-setup` row, #2477). One folder a packet; each folder's
//! `wire.rs` answers, until its packet lands, what `wsp-host` answers today for the ports, doors, slots, roads and command
//! lines the packet wires, so the host calls it from the start and the packet fills it in.
pub mod acts;
pub mod commands;
pub mod context;
pub mod deploy;
pub mod doctor;
pub mod files;
pub mod import;
pub mod install;
pub mod join;
pub mod mcp;
pub mod paths;
pub mod projects;
pub mod providers;
pub mod recipes;
pub mod relay;
pub mod relay_link;
pub mod report;
pub mod service;
pub mod sign_ins;
