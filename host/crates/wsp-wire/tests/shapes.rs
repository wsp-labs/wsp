// SPDX-License-Identifier: AGPL-3.0-only
//! Every exported zod shape of the protocol's views/ and top-level files, read and written as zod does: the exact JSON
//! text zod's parse wrote for each sample, and the code (and, for a check's rule, the path) of the first issue it raised
//! for each sample that breaks a rule. The fixtures come from running zod on the TS schemas at a3b870a6e.

#[path = "shapes/common.rs"]
mod common;

#[path = "shapes/types_agents_report.rs"]
mod types_agents_report;
#[path = "shapes/types_attachments.rs"]
mod types_attachments;
#[path = "shapes/types_bring_back.rs"]
mod types_bring_back;
#[path = "shapes/types_changes.rs"]
mod types_changes;
#[path = "shapes/types_conversations.rs"]
mod types_conversations;
#[path = "shapes/types_general_prefs.rs"]
mod types_general_prefs;
#[path = "shapes/types_init_job.rs"]
mod types_init_job;
#[path = "shapes/types_outside_line.rs"]
mod types_outside_line;
#[path = "shapes/types_plan_alerts.rs"]
mod types_plan_alerts;
#[path = "shapes/types_project_look.rs"]
mod types_project_look;
#[path = "shapes/types_pull_request.rs"]
mod types_pull_request;
#[path = "shapes/types_recipe_file.rs"]
mod types_recipe_file;
#[path = "shapes/types_release.rs"]
mod types_release;
#[path = "shapes/types_start.rs"]
mod types_start;
#[path = "shapes/types_sub_thread.rs"]
mod types_sub_thread;
#[path = "shapes/types_thread_defaults.rs"]
mod types_thread_defaults;
#[path = "shapes/types_thread_read.rs"]
mod types_thread_read;
#[path = "shapes/types_tree.rs"]
mod types_tree;
#[path = "shapes/types_usage.rs"]
mod types_usage;
#[path = "shapes/types_workspace_look.rs"]
mod types_workspace_look;
#[path = "shapes/views_activity.rs"]
mod views_activity;
#[path = "shapes/views_app_procs.rs"]
mod views_app_procs;
#[path = "shapes/views_golden_image.rs"]
mod views_golden_image;
#[path = "shapes/views_harness.rs"]
mod views_harness;
#[path = "shapes/views_place.rs"]
mod views_place;
#[path = "shapes/views_place_kind.rs"]
mod views_place_kind;
#[path = "shapes/views_place_setup_words.rs"]
mod views_place_setup_words;
#[path = "shapes/views_preferences.rs"]
mod views_preferences;
#[path = "shapes/views_project_bundle.rs"]
mod views_project_bundle;
#[path = "shapes/views_session.rs"]
mod views_session;
#[path = "shapes/views_session_events.rs"]
mod views_session_events;
#[path = "shapes/views_snapshot_lineage.rs"]
mod views_snapshot_lineage;
#[path = "shapes/views_sub_row.rs"]
mod views_sub_row;
#[path = "shapes/views_workspace.rs"]
mod views_workspace;
#[path = "shapes/views_workspace_events.rs"]
mod views_workspace_events;
#[path = "shapes/views_workspace_kind.rs"]
mod views_workspace_kind;
