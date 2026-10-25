// SPDX-License-Identifier: AGPL-3.0-only
// Rules take `&T` for the key's type T so they fit `keys::Rule<T>`; the enums mirror the TS unions variant for variant.
#![allow(clippy::ptr_arg, clippy::large_enum_variant)]

pub mod text;
pub mod types;
pub mod views;
pub mod wire;
