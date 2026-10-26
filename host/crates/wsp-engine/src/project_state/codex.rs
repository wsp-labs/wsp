// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/codex.ts. Spec: spec/project_state_agents.md.
use std::path::Path;

use async_trait::async_trait;
use serde_json::{json, Map, Value};

use wsp_wire::views::project_bundle::ProjectCarry;

use crate::errors::Result;
use crate::project_state::merge::{jsonl_cwd_step, merge_script};
use crate::project_state::py::py_data;
use crate::project_state::resolver::{join_posix, rewrite_cwd, under_home, CwdHolder, MovedState, ProjectStateResolver, SharedStore};
use crate::project_state::sqlite::{
    count_rows, from_param, js_string, moved_column, moved_sql, path_params, project_rows, select_rows, select_then_update, sqlite_filter,
    sqlite_merge_step, under_path, MergeMode, MergeTable, ProjectTable, Row, RowUpdate, SqlValue,
};

/// codex.ts:9.
pub const SESSIONS_DIR: &str = "sessions";
/// codex.ts:11. The index every project's threads sit in.
pub const INDEX: &str = "state_5.sqlite";

const SESSIONS: &str = "/sessions/";

/// codex.ts:18. The rollout under this home: the tail after the last `/sessions/` of the recorded path, under
/// `<home>/sessions` (`under_home`); `None` without that segment or where the tail climbs out.
pub fn rollout_under(home: &str, recorded: &str) -> Option<String> {
    let at = recorded.rfind(SESSIONS)?;
    under_home(home, &[SESSIONS_DIR, &recorded[at + SESSIONS.len()..]])
}

/// codex.ts:36. carry transcript-only; states thread index, rollout transcript; roots state_5.sqlite, sessions; shared store the index filtered to threads under cwd.
pub struct CodexResolver;

fn threads() -> ProjectTable {
    ProjectTable { table: "threads".into(), r#where: under_path("cwd") }
}

fn indexed_rollouts_sql() -> String {
    format!("select rollout_path from threads where {} order by rollout_path", under_path("cwd"))
}

fn rollout_value(row: &Row) -> String {
    js_string(row.get("rollout_path").unwrap_or(&SqlValue::Null))
}

struct Rollouts {
    files: Vec<String>,
    skipped: u64,
}

/// The rollouts present under this home among the ones the index recorded, and how many it recorded that are not:
/// archived outside sessions/, or gone.
fn rollouts_under(home: &str, recorded: &[String]) -> Rollouts {
    let mut files: Vec<String> = Vec::new();
    let mut skipped = 0;
    for r in recorded {
        match rollout_under(home, r) {
            Some(f) if Path::new(&f).exists() => {
                if !files.contains(&f) {
                    files.push(f);
                }
            }
            _ => skipped += 1,
        }
    }
    Rollouts { files, skipped }
}

fn payload(line: &mut Map<String, Value>) -> Option<&mut Map<String, Value>> {
    line.get_mut("payload")?.as_object_mut()
}

#[async_trait(?Send)]
impl ProjectStateResolver for CodexResolver {
    fn agent(&self) -> &'static str {
        "codex"
    }
    fn states(&self) -> Vec<&'static str> {
        vec!["thread index", "rollout transcript"]
    }
    fn roots(&self) -> Vec<&'static str> {
        vec![INDEX, SESSIONS_DIR]
    }
    fn carry(&self) -> ProjectCarry {
        ProjectCarry::TranscriptOnly
    }
    async fn move_state(&self, home: &str, from: &str, to: &str) -> Result<Vec<MovedState>> {
        let index = join_posix(&[home, INDEX]);
        let update = RowUpdate {
            sql: format!("update threads set cwd = {} where {}", moved_column("cwd"), under_path("cwd")),
            params: path_params(from, to),
        };
        let Some((recorded, threads)) = select_then_update(&index, &indexed_rollouts_sql(), &from_param(from), &update)? else {
            return Ok(vec![]);
        };
        let rollouts = rollouts_under(home, &recorded);
        let mut moved = Vec::new();
        if threads > 0 {
            moved.push(MovedState {
                state: "thread index".into(),
                files: vec![index.clone()],
                changed: threads,
                skipped: (rollouts.skipped > 0).then_some(rollouts.skipped),
            });
        }
        let mut files = Vec::new();
        let mut changed = 0;
        for f in rollouts.files {
            let n = rewrite_cwd(&f, from, to, Some(payload as CwdHolder)).await?;
            if n > 0 {
                files.push(f);
                changed += n;
            }
        }
        if changed > 0 {
            moved.push(MovedState { state: "rollout transcript".into(), files, changed, skipped: None });
        }
        Ok(moved)
    }
    async fn sessions(&self, home: &str, path: &str) -> Result<u64> {
        count_rows(
            &join_posix(&[home, INDEX]),
            &format!("select count(*) as n from threads where {}", under_path("cwd")),
            &from_param(path),
        )
    }
    async fn entries(&self, home: &str, path: &str) -> Result<Vec<String>> {
        let recorded: Vec<String> = select_rows(&join_posix(&[home, INDEX]), &indexed_rollouts_sql(), &from_param(path))?
            .unwrap_or_default()
            .iter()
            .map(rollout_value)
            .collect();
        Ok(rollouts_under(home, &recorded).files)
    }
    fn listing(&self, home: &str, _path: &str) -> Option<Vec<String>> {
        Some(vec![
            "con = sqlite3.connect(\"file:\" + urllib.parse.quote(COPY) + \"?mode=ro\", uri=True)".into(),
            "try:".into(),
            format!("    indexed = con.execute({}).fetchall()", py_data(&json!("select rollout_path from threads order by rollout_path"))),
            "finally:".into(),
            "    con.close()".into(),
            "for (rollout,) in indexed:".into(),
            format!("    cut = rollout.rfind({}) if isinstance(rollout, str) else -1", py_data(&json!(SESSIONS))),
            "    if cut >= 0:".into(),
            format!(
                "        say(os.path.join({}, rollout[cut + {}:]))",
                py_data(&json!(join_posix(&[home, SESSIONS_DIR]))),
                SESSIONS.len()
            ),
        ])
    }
    fn shared(&self) -> Option<SharedStore> {
        Some(SharedStore { store: INDEX.into(), filter: sqlite_filter(&[threads()]) })
    }
    fn has_merge(&self) -> bool {
        true
    }
    async fn merge(&self, home: &str, from: &str, to: &str, guest_home: &str) -> Result<Option<String>> {
        // A row whose rollout did not travel would open nothing on the machine, so only travelled rollouts are kept.
        let threads: Vec<Row> = project_rows(&join_posix(&[home, INDEX]), &threads(), from)?
            .into_iter()
            .filter(|r| !rollouts_under(home, &[rollout_value(r)]).files.is_empty())
            .collect();
        if threads.is_empty() {
            return Ok(None);
        }
        let rows: Vec<Row> = threads
            .into_iter()
            .map(|mut r| {
                let rollout = rollout_under(guest_home, &rollout_value(&r)).map_or(SqlValue::Null, SqlValue::Text);
                let cwd = moved_sql(r.get("cwd"), from, to);
                r.insert("cwd".into(), cwd);
                r.insert("rollout_path".into(), rollout);
                r
            })
            .collect();
        let landed: Vec<String> = rows.iter().map(rollout_value).collect();
        let table = MergeTable {
            table: "threads".into(),
            mode: MergeMode::Keyed { key: vec!["id".into()], set: vec!["cwd".into(), "rollout_path".into()] },
            rows,
        };
        Ok(Some(merge_script(
            from,
            to,
            &[jsonl_cwd_step(&landed, Some("payload")), sqlite_merge_step(&join_posix(&[guest_home, INDEX]), &[table])],
        )))
    }
}
