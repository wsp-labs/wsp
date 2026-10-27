// SPDX-License-Identifier: AGPL-3.0-only
//! Packet 1: files and leaves (`env-keys`, `token-shapes`, `daemon-binary`, `assets`, `storage`, `connector`, `local-daemon`, `release`, the host's key).
pub mod assets;
pub mod state;
pub mod connector;
pub mod daemon_binary;
pub mod env_keys;
pub mod folder_files;
pub mod local_daemon;
pub mod release_fetch;
pub mod release;
pub mod storage;
pub mod token_shapes;
pub mod wsp_own;
pub mod this_computer_key;
pub mod wire;
