// SPDX-License-Identifier: AGPL-3.0-only
//! The dotenv a host keeps beside its state file and what a turn's vault reads out of it: `env-keys.ts` and `cli/keys.ts`,
//! kept in `wsp-setup`.
pub use wsp_setup::files::env_keys::{env_file_for, parse_env_file, server_env_file_for};
pub use wsp_setup::providers::keys::vault_now;
