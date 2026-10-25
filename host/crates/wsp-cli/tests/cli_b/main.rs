// SPDX-License-Identifier: AGPL-3.0-only
// cli-b's verbs held to the TS command line (#2449 wave 5): workspaces, agents, computers, projects, slate,
// activity-table, activity and conversations. One test binary so the eight units link once.

mod common;

mod activity;
mod activity_rules;
mod agents_rules;
mod agents_servers;
mod agents_skills;
mod computers;
mod computers_rules;
mod projects;
mod projects_rules;
mod shared_rules;
mod slate;
mod slate_rules;
mod workspaces_git;
mod workspaces_life;
mod workspaces_rules;
