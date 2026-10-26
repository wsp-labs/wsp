// SPDX-License-Identifier: AGPL-3.0-only
//! cli-c's tests: the helpers of `turns-help.ts`, `workspaces-help.ts` and `agents-help.ts`, each case recorded
//! from the TS at 65f6b89c7 (`tests/fixtures/cli_c/record/`). One binary so the units link once.
#![allow(
    clippy::result_large_err,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::large_enum_variant,
    clippy::regex_creation_in_loops,
    non_snake_case,
    dead_code
)]
mod common;

mod agents_read;
mod agents_read_rules;
mod agents_set;
mod agents_set_rules;
mod drivers_agents_read;
mod drivers_agents_set;
mod drivers_turns_follow;
mod drivers_turns_picks;
mod drivers_turns_stream;
mod drivers_turns_target;
mod drivers_ws_life;
mod drivers_ws_reads;
mod drivers_ws_rows;
mod turns_follow;
mod turns_follow_rules;
mod turns_picks;
mod turns_picks_rules;
mod turns_stream;
mod turns_stream_rules;
mod turns_target;
mod turns_target_rules;
mod ws_life;
mod ws_life_rules;
mod ws_reads;
mod ws_reads_rules;
mod ws_rows;
mod ws_rows_rules;
