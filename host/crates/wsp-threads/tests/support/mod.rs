// SPDX-License-Identifier: AGPL-3.0-only
//! The tests' one fake runtime. `FakeHost` implements every context trait of slice a: each call is logged by name in
//! order (string, bool and number arguments in parentheses), and answered by the hook a test set for
//! `"<Trait>::<method>"`, else by the host's own state, else by a neutral default (no, none, empty, ok). A method with
//! no neutral answer panics naming the hook to set. `generated.rs` is written by `gen_fake.py` from the traits:
//! `python3 tests/support/gen_fake.py src tests/support/fake_hand.rs.in > tests/support/generated.rs`.
#![allow(dead_code)]

pub mod fake;
pub mod generated;

pub use fake::*;
