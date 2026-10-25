// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-runs.ts at 65f6b89c7: the commands a slate starts and the secrets a person
//! types into one. Four units: `parts` (the shapes and the pure record helpers, spec/runs_parts.md), `secrets`
//! (spec/runs_secrets.md), `road` (this computer's process road, spec/runs_road.md) and `table` (the process table,
//! spec/runs_table.md).

pub mod parts;
pub mod road;
pub mod secrets;
pub mod table;

pub use parts::*;
pub use road::{kill_group, local_road, reshape_answer, reshape_result, ReshapeStart, RoadEnd, RoadEvents, RoadStart, RoadStarted, RunRoad, Stream};
pub use secrets::{create_secrets, encode_uri_component, secret_forms, DefaultApprovals, RunApprovals, SlateSecrets};
pub use table::{create_slate_runs, ApproveScope, SlateRuns, SlateRunsDeps, TimerDecl};
