// SPDX-License-Identifier: AGPL-3.0-only
//! Packet 5a: the daemon's deploy (`doctor.ts` lines 1-1270, `daemon-flags`, `login-files`).
pub mod places;
pub mod bundle;
pub mod scripts;
#[allow(clippy::module_inception)]
pub mod deploy;
pub mod socket;
pub mod claude_envs;
pub mod daemon_flags;
pub mod login_files;
pub mod wire;
