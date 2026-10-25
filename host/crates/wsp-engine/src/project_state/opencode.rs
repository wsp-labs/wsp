// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/opencode.ts. Spec: spec/project_state_agents.md.
use async_trait::async_trait;

use wsp_wire::views::project_bundle::ProjectCarry;

use crate::errors::Result;
use crate::project_state::merge::merge_script;
use crate::project_state::resolver::{join_posix, MovedState, ProjectStateResolver, SharedStore};
use crate::project_state::sqlite::{
    count_rows, from_param, moved_column, moved_sql, path_params, shared_filter, shared_tables, sqlite_merge_step, under_path, update_rows,
    MergeMode, Row, RowUpdate, SharedTable, SqlValue,
};

/// opencode.ts:12.
pub const DB: &str = "opencode.db";

/// opencode.ts:26. carry transcript-only; states project, project directories, sessions; roots opencode.db; shared store project, project_directory, session, message, part; entries always empty.
pub struct OpencodeResolver;

fn updates() -> [(&'static str, String); 3] {
    [
        ("project", format!("update project set worktree = {}, sandboxes = '[]' where {}", moved_column("worktree"), under_path("worktree"))),
        ("project directories", format!("update project_directory set directory = {} where {}", moved_column("directory"), under_path("directory"))),
        ("sessions", format!("update session set directory = {} where {}", moved_column("directory"), under_path("directory"))),
    ]
}

fn session_ids() -> String {
    format!("select id from session where {}", under_path("directory"))
}

fn tables() -> Vec<SharedTable> {
    vec![
        SharedTable {
            table: "project".into(),
            r#where: under_path("worktree"),
            mode: MergeMode::Keyed { key: vec!["id".into()], set: vec!["worktree".into(), "sandboxes".into()] },
            r#move: move_project,
        },
        SharedTable {
            table: "project_directory".into(),
            r#where: under_path("directory"),
            mode: MergeMode::Keyed { key: vec!["project_id".into(), "directory".into()], set: vec![] },
            r#move: move_directory,
        },
        SharedTable {
            table: "session".into(),
            r#where: under_path("directory"),
            mode: MergeMode::Keyed { key: vec!["id".into()], set: vec!["directory".into()] },
            r#move: move_directory,
        },
        SharedTable {
            table: "message".into(),
            r#where: format!("session_id in ({})", session_ids()),
            mode: MergeMode::Keyed { key: vec!["id".into()], set: vec![] },
            r#move: same_row,
        },
        SharedTable {
            table: "part".into(),
            r#where: format!("message_id in (select id from message where session_id in ({}))", session_ids()),
            mode: MergeMode::Keyed { key: vec!["id".into()], set: vec![] },
            r#move: same_row,
        },
    ]
}

fn move_project(row: &Row, from: &str, to: &str) -> Row {
    let mut out = row.clone();
    out.insert("worktree".into(), moved_sql(row.get("worktree"), from, to));
    out.insert("sandboxes".into(), SqlValue::Text("[]".into()));
    out
}

fn move_directory(row: &Row, from: &str, to: &str) -> Row {
    let mut out = row.clone();
    out.insert("directory".into(), moved_sql(row.get("directory"), from, to));
    out
}

fn same_row(row: &Row, _from: &str, _to: &str) -> Row {
    row.clone()
}

#[async_trait(?Send)]
impl ProjectStateResolver for OpencodeResolver {
    fn agent(&self) -> &'static str {
        "opencode"
    }
    fn states(&self) -> Vec<&'static str> {
        vec!["project", "project directories", "sessions"]
    }
    fn roots(&self) -> Vec<&'static str> {
        vec![DB]
    }
    fn carry(&self) -> ProjectCarry {
        ProjectCarry::TranscriptOnly
    }
    async fn move_state(&self, home: &str, from: &str, to: &str) -> Result<Vec<MovedState>> {
        let db = join_posix(&[home, DB]);
        let updates = updates();
        let statements: Vec<RowUpdate> = updates.iter().map(|(_, sql)| RowUpdate { sql: sql.clone(), params: path_params(from, to) }).collect();
        let Some(rows) = update_rows(&db, &statements)? else {
            return Ok(vec![]);
        };
        Ok(updates
            .iter()
            .zip(rows)
            .filter(|(_, changed)| *changed > 0)
            .map(|(entry, changed)| MovedState { state: entry.0.to_string(), files: vec![db.clone()], changed, skipped: None })
            .collect())
    }
    async fn sessions(&self, home: &str, path: &str) -> Result<u64> {
        count_rows(&join_posix(&[home, DB]), &format!("select count(*) as n from session where {}", under_path("directory")), &from_param(path))
    }
    async fn entries(&self, _home: &str, _path: &str) -> Result<Vec<String>> {
        Ok(vec![])
    }
    fn shared(&self) -> Option<SharedStore> {
        Some(SharedStore { store: DB.into(), filter: shared_filter(&tables()) })
    }
    fn has_merge(&self) -> bool {
        true
    }
    async fn merge(&self, home: &str, from: &str, to: &str, guest_home: &str) -> Result<Option<String>> {
        let Some(merged) = shared_tables(&join_posix(&[home, DB]), &tables(), from, to)? else {
            return Ok(None);
        };
        Ok(Some(merge_script(from, to, &[sqlite_merge_step(&join_posix(&[guest_home, DB]), &merged)])))
    }
}
