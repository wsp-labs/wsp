// SPDX-License-Identifier: AGPL-3.0-only
//! The host's store leaves: the one writer under the owner's state folder (`packages/own-file`) and the keys and seal
//! of a wsp link (`packages/keys`). Chapter 01 of the Rust host spec puts both here.
pub mod keys;
pub mod own_file;
