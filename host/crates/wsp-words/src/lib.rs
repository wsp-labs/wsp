// SPDX-License-Identifier: AGPL-3.0-only
//! Every sentence the host says to a person, the refusal and exit contract, and the JavaScript text and number rules
//! the TS host leans on. Port of packages/protocol/src/words/, format.ts, exit.ts and the sentence functions of
//! packages/protocol/src/ at a3b870a6e. Each unit's spec is spec/<unit>.md beside this folder.

pub mod conversations;
pub mod cost_history;
pub mod docker_shim;
pub mod exit;
pub mod general_prefs;
pub mod js;
pub mod oom;
pub mod outside_line;
pub mod place_name;
pub mod place_state;
pub mod place_word;
pub mod plan_alerts;
pub mod project_path;
pub mod ps_time;
pub mod quote;
pub mod regexp;
pub mod run_block;
pub mod semver;
pub mod thread_cgroup;
pub mod thread_shell;
pub mod thread_state;
pub mod tool_result;
pub mod usage;
pub mod words;
pub mod workspace_look;
pub mod workspace_state;
pub mod wsp_tools;
