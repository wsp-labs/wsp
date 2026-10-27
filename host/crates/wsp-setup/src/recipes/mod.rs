// SPDX-License-Identifier: AGPL-3.0-only
//! Packet 4: recipes.
pub mod cloud_configs;
pub mod init_brew;
pub mod init_recipe;
pub mod pick_sizes;
pub mod recipe_configs;
pub mod recipe_options;
pub mod recipe_watch;
#[allow(clippy::module_inception)]
pub mod recipes;
pub mod wire;
