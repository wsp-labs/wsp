// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/ssh-backend.ts at a4033dc34, in three units: [`dial`] (spec/ssh_dial.md), [`keys`]
//! (spec/ssh_keys.md) and [`machine`] (spec/ssh_machine.md).
//!
//! A computer reached over ssh: a machine that already exists, the person's own, dialled with their own key, which is
//! how a host joins it. Nothing here creates, forks, pauses or snapshots.
pub mod dial;
pub mod keys;
pub mod machine;

pub use dial::*;
pub use keys::*;
pub use machine::*;
