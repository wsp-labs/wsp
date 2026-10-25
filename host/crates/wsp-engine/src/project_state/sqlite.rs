// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/sqlite.ts. Spec: spec/project_state_files.md.
//!
//! The agents that index sessions in sqlite get their rows updated here (rusqlite where the TS has node:sqlite). A
//! missing database is nothing found, never created.
use indexmap::IndexMap;
use serde_json::Value;

use crate::errors::Result;

/// One sqlite value as node:sqlite reads it: null, an integer, a real, text or a blob.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

/// sqlite.ts:64. A row by column name, in the select's column order.
pub type Row = IndexMap<String, SqlValue>;

/// sqlite.ts:9. One update and its named parameters (`$from`, `$to`).
#[derive(Debug, Clone, PartialEq)]
pub struct RowUpdate {
    pub sql: String,
    pub params: Vec<(String, SqlValue)>,
}

/// sqlite.ts:15. `(<column> = $from or substr(<column>, 1, length($from) + 1) = $from || '/')`.
pub fn under_path(column: &str) -> String {
    todo!()
}

/// sqlite.ts:17. `iif(<under_path(column)>, $to || substr(<column>, length($from) + 1), <column>)`.
pub fn moved_column(column: &str) -> String {
    todo!()
}

/// sqlite.ts:18. `$from` then `$to`.
pub fn path_params(from: &str, to: &str) -> Vec<(String, SqlValue)> {
    todo!()
}

/// sqlite.ts:24. The query's `n` as a number, the database opened read-only; 0 where the file is absent.
pub fn count_rows(db: &str, sql: &str, params: &[(String, SqlValue)]) -> Result<u64> {
    todo!()
}

/// sqlite.ts:55. The rows the select finds, read-only; `None` where the file is absent.
pub fn select_rows(db: &str, sql: &str, params: &[(String, SqlValue)]) -> Result<Option<Vec<Row>>> {
    todo!()
}

/// sqlite.ts:62. How one table's rows meet the machine's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeMode {
    /// Inserted when the key is absent; when present only the `set` columns change.
    Keyed { key: Vec<String>, set: Vec<String> },
    /// Inserted less the `drop` columns when the machine holds no row whose `by` column matches; else kept as a group.
    By { by: String, drop: Vec<String> },
}

/// sqlite.ts:63. On the wire (in the merge step's TABLES): `table`, then `key`, `set` or `by`, `drop`, then `rows`.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeTable {
    pub table: String,
    pub mode: MergeMode,
    pub rows: Vec<Row>,
}

/// sqlite.ts:67.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectTable {
    pub table: String,
    /// SQL naming the rows keyed to `$from`.
    pub r#where: String,
}

/// sqlite.ts:75 `move`: a row moved to the new path.
pub type MoveRow = fn(&Row, &str, &str) -> Row;

/// sqlite.ts:75.
#[derive(Debug, Clone)]
pub struct SharedTable {
    pub table: String,
    pub r#where: String,
    pub mode: MergeMode,
    pub r#move: MoveRow,
}

/// sqlite.ts:78. `select * from <table> where <where> order by 1, 2` with `$from`; empty where the file is absent.
pub fn project_rows(db: &str, t: &ProjectTable, from: &str) -> Result<Vec<Row>> {
    todo!()
}

/// sqlite.ts:82. Each table's project rows moved; `None` where every table has none.
pub fn shared_tables(db: &str, tables: &[SharedTable], from: &str, to: &str) -> Result<Option<Vec<MergeTable>>> {
    todo!()
}

/// sqlite.ts:91. Rows the filter fetches at a time.
pub const FILTER_BATCH: usize = 500;

/// sqlite.ts:99. The listing lines that write the project's rows of each table into a copy of the store.
pub fn sqlite_filter(tables: &[ProjectTable]) -> Vec<String> {
    todo!()
}

/// sqlite.ts:135. A row as the merge script carries it: a blob as `{"$blob": <base64>}`, every other value as JSON.
pub fn portable(row: &Row) -> Value {
    todo!()
}

/// sqlite.ts:143. The merge step over the machine's copy of the store.
pub fn sqlite_merge_step(db: &str, tables: &[MergeTable]) -> String {
    todo!()
}

/// sqlite.ts:196. Each update in one transaction, answering the rows each changed; `None` where the file is absent.
/// A failing update rolls the whole transaction back.
pub fn update_rows(db: &str, updates: &[RowUpdate]) -> Result<Option<Vec<u64>>> {
    todo!()
}

/// sqlite.ts:40 used by codex: runs `select` (its first column as text) then `update` in one transaction; `None` where
/// the file is absent. Answers the selected values and the rows the update changed.
pub fn select_then_update(db: &str, select: &str, select_params: &[(String, SqlValue)], update: &RowUpdate) -> Result<Option<(Vec<String>, u64)>> {
    todo!()
}
