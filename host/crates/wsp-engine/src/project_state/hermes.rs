// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/hermes.ts. Spec: spec/project_state_agents.md.
use async_trait::async_trait;

use wsp_wire::views::project_bundle::ProjectCarry;

use crate::errors::Result;
use crate::project_state::merge::merge_script;
use crate::project_state::resolver::{join_posix, MovedState, ProjectStateResolver, SharedStore};
use crate::project_state::sqlite::{
    count_rows, from_param, moved_column, moved_sql, path_params, shared_filter, shared_tables, sqlite_merge_step, under_path, update_rows,
    MergeMode, Row, RowUpdate, SharedTable,
};

/// hermes.ts:7.
pub const DB: &str = "state.db";

/// hermes.ts:17. carry transcript-only; states sessions; roots state.db; shared store sessions and their messages; entries always empty.
pub struct HermesResolver;

fn at_path() -> String {
    format!("{} or {}", under_path("cwd"), under_path("git_repo_root"))
}

fn tables() -> Vec<SharedTable> {
    vec![
        SharedTable {
            table: "sessions".into(),
            r#where: at_path(),
            mode: MergeMode::Keyed { key: vec!["id".into()], set: vec!["cwd".into(), "git_repo_root".into()] },
            r#move: move_session,
        },
        SharedTable {
            table: "messages".into(),
            r#where: format!("session_id in (select id from sessions where {})", at_path()),
            mode: MergeMode::By { by: "session_id".into(), drop: vec!["id".into()] },
            r#move: same_row,
        },
    ]
}

fn move_session(row: &Row, from: &str, to: &str) -> Row {
    let mut out = row.clone();
    out.insert("cwd".into(), moved_sql(row.get("cwd"), from, to));
    out.insert("git_repo_root".into(), moved_sql(row.get("git_repo_root"), from, to));
    out
}

fn same_row(row: &Row, _from: &str, _to: &str) -> Row {
    row.clone()
}

#[async_trait(?Send)]
impl ProjectStateResolver for HermesResolver {
    fn agent(&self) -> &'static str {
        "hermes"
    }
    fn states(&self) -> Vec<&'static str> {
        vec!["sessions"]
    }
    fn roots(&self) -> Vec<&'static str> {
        vec![DB]
    }
    fn carry(&self) -> ProjectCarry {
        ProjectCarry::TranscriptOnly
    }
    async fn move_state(&self, home: &str, from: &str, to: &str) -> Result<Vec<MovedState>> {
        let db = join_posix(&[home, DB]);
        let update = RowUpdate {
            sql: format!(
                "update sessions set cwd = {}, git_repo_root = {} where {}",
                moved_column("cwd"),
                moved_column("git_repo_root"),
                at_path()
            ),
            params: path_params(from, to),
        };
        let changed = update_rows(&db, &[update])?.and_then(|rows| rows.first().copied()).unwrap_or(0);
        if changed == 0 {
            return Ok(vec![]);
        }
        Ok(vec![MovedState { state: "sessions".into(), files: vec![db], changed, skipped: None }])
    }
    async fn sessions(&self, home: &str, path: &str) -> Result<u64> {
        count_rows(&join_posix(&[home, DB]), &format!("select count(*) as n from sessions where {}", at_path()), &from_param(path))
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
