// SPDX-License-Identifier: AGPL-3.0-only
//! Every sentence the host says to a person, the refusal and exit contract, and the JavaScript text and number rules
//! the TS host leans on. Port of packages/protocol/src/words/, format.ts, exit.ts and the sentence functions of
//! packages/protocol/src/ at a3b870a6e. Each unit's spec is spec/<unit>.md beside this folder.

pub mod exit;
pub mod js;
pub mod pending_wire;
pub mod semver;
pub mod words;
