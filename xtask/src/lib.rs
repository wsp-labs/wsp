// SPDX-License-Identifier: AGPL-3.0-only
//! The parity harness: tapes of what a host was sent, the TS host's answers frozen as goldens, and a replay of every
//! tape against a host behind one interface, diffed after normalizing.

pub mod accepted;
pub mod check;
pub mod diff;
pub mod golden;
pub mod held;
pub mod host;
pub mod normalize;
pub mod sandbox;
pub mod seed;
pub mod tape;
pub mod unit;
