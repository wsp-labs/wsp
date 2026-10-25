// SPDX-License-Identifier: AGPL-3.0-only
//! What the machines slice's tests share: a fake Boat API behind the fetch seam, a clock whose sleeps cost nothing,
//! and a recorder for the warn lines. Each test file takes it with `mod common;`.
#![allow(dead_code)]
pub mod fake_box;
pub mod fake_clock;
pub mod warn;
