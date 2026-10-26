// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/sqlite.ts. Spec: spec/project_state_files.md.
//!
//! The agents that index sessions in sqlite get their rows updated here (rusqlite where the TS has node:sqlite). A
//! missing database is nothing found, never created.
use std::path::Path;
use std::time::Duration;

use base64::Engine;
use indexmap::IndexMap;
use rusqlite::types::{Value as RValue, ValueRef};
use rusqlite::{Connection, OpenFlags, ToSql};
use serde_json::{json, Value};

use wsp_words::js::number_string;

use crate::errors::{EngineError, Result};
use crate::project_state::py::py_data;
use crate::project_state::resolver::moved_path;

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
    format!("({column} = $from or substr({column}, 1, length($from) + 1) = $from || '/')")
}

/// sqlite.ts:17. `iif(<under_path(column)>, $to || substr(<column>, length($from) + 1), <column>)`.
pub fn moved_column(column: &str) -> String {
    format!("iif({}, $to || substr({column}, length($from) + 1), {column})", under_path(column))
}

/// sqlite.ts:18. `$from` then `$to`.
pub fn path_params(from: &str, to: &str) -> Vec<(String, SqlValue)> {
    vec![("$from".into(), SqlValue::Text(from.into())), ("$to".into(), SqlValue::Text(to.into()))]
}

/// sqlite.ts:24. The query's `n` as a number, the database opened read-only; 0 where the file is absent.
pub fn count_rows(db: &str, sql: &str, params: &[(String, SqlValue)]) -> Result<u64> {
    if !Path::new(db).exists() {
        return Ok(0);
    }
    let conn = open(db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    Ok(match query(&conn, sql, params)?.first().and_then(|row| row.get("n")) {
        Some(SqlValue::Integer(n)) => u64::try_from(*n).unwrap_or(0),
        Some(SqlValue::Real(f)) => *f as u64,
        _ => 0,
    })
}

/// sqlite.ts:55. The rows the select finds, read-only; `None` where the file is absent.
pub fn select_rows(db: &str, sql: &str, params: &[(String, SqlValue)]) -> Result<Option<Vec<Row>>> {
    if !Path::new(db).exists() {
        return Ok(None);
    }
    let conn = open(db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    query(&conn, sql, params).map(Some)
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
    let sql = format!("select * from {} where {} order by 1, 2", t.table, t.r#where);
    Ok(select_rows(db, &sql, &from_param(from))?.unwrap_or_default())
}

/// sqlite.ts:82. Each table's project rows moved; `None` where every table has none.
pub fn shared_tables(db: &str, tables: &[SharedTable], from: &str, to: &str) -> Result<Option<Vec<MergeTable>>> {
    let mut out = Vec::with_capacity(tables.len());
    for t in tables {
        let owned = ProjectTable { table: t.table.clone(), r#where: t.r#where.clone() };
        let rows = project_rows(db, &owned, from)?.iter().map(|row| (t.r#move)(row, from, to)).collect();
        out.push(MergeTable { table: t.table.clone(), mode: t.mode.clone(), rows });
    }
    Ok(out.iter().any(|t| !t.rows.is_empty()).then_some(out))
}

/// sqlite.ts:91. Rows the filter fetches at a time.
pub const FILTER_BATCH: usize = 500;

/// sqlite.ts:99. The listing lines that write the project's rows of each table into a copy of the store.
pub fn sqlite_filter(tables: &[ProjectTable]) -> Vec<String> {
    let table_list: Vec<Value> = tables.iter().map(|t| json!({ "table": t.table, "where": t.r#where })).collect();
    let lines: Vec<String> = vec![
        format!("TABLES = {}", py_data(&Value::Array(table_list))),
        "src = sqlite3.connect(\"file:\" + urllib.parse.quote(STORE) + \"?mode=ro\", uri=True)".into(),
        "try:".into(),
        "    dst = sqlite3.connect(COPY)".into(),
        "    try:".into(),
        "        with dst:".into(),
        format!(
            "            for (sql,) in src.execute({}).fetchall():",
            py_data(&json!("select sql from sqlite_master where sql is not null and name not like 'sqlite@_%' escape '@' order by rowid"))
        ),
        "                try:".into(),
        "                    dst.execute(sql)".into(),
        "                except sqlite3.OperationalError:".into(),
        "                    pass".into(),
        format!("            held = {{r[0] for r in src.execute({})}}", py_data(&json!("select name from sqlite_master where type = 'table'"))),
        "            for t in TABLES:".into(),
        "                if t[\"table\"] not in held:".into(),
        "                    continue".into(),
        "                cur = src.execute(\"select * from \" + q(t[\"table\"]) + \" where \" + t[\"where\"], {\"from\": PATH})".into(),
        "                cols = [d[0] for d in cur.description]".into(),
        "                ins = \"insert into \" + q(t[\"table\"]) + \" (\" + \", \".join(q(c) for c in cols) + \") values (\" + \", \".join(\"?\" for c in cols) + \")\"".into(),
        "                while True:".into(),
        format!("                    got = cur.fetchmany({FILTER_BATCH})"),
        "                    if not got:".into(),
        "                        break".into(),
        "                    dst.executemany(ins, got)".into(),
        "                    FILTERED += len(got)".into(),
        "    finally:".into(),
        "        dst.close()".into(),
        "finally:".into(),
        "    src.close()".into(),
    ];
    lines
}

/// sqlite.ts:135. A row as the merge script carries it: a blob as `{"$blob": <base64>}`, every other value as JSON.
pub fn portable(row: &Row) -> Value {
    Value::Object(
        row.iter()
            .map(|(column, value)| {
                let carried = match value {
                    SqlValue::Null => Value::Null,
                    SqlValue::Integer(i) => json!(i),
                    SqlValue::Real(f) => json!(f),
                    SqlValue::Text(s) => Value::String(s.clone()),
                    SqlValue::Blob(b) => json!({ "$blob": base64::engine::general_purpose::STANDARD.encode(b) }),
                };
                (column.clone(), carried)
            })
            .collect(),
    )
}

/// sqlite.ts:143. The merge step over the machine's copy of the store.
pub fn sqlite_merge_step(db: &str, tables: &[MergeTable]) -> String {
    let mut lines: Vec<String> = vec![
        format!("DB = {}", py_data(&json!(db))),
        "if not os.path.exists(DB):".into(),
        "    out({\"waiting\": \"no \" + DB + \" on the machine\"})".into(),
        format!("TABLES = {}", py_data(&Value::Array(tables.iter().map(merge_table_json).collect()))),
    ];
    lines.extend(MERGE_TAIL.iter().map(|line| line.to_string()));
    lines.join("\n")
}

/// sqlite.ts:196. Each update in one transaction, answering the rows each changed; `None` where the file is absent.
/// A failing update rolls the whole transaction back.
pub fn update_rows(db: &str, updates: &[RowUpdate]) -> Result<Option<Vec<u64>>> {
    if !Path::new(db).exists() {
        return Ok(None);
    }
    let mut conn = open(db, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    let tx = conn.transaction().map_err(sql_error)?;
    let changed = updates.iter().map(|u| run_update(&tx, u)).collect::<Result<Vec<u64>>>()?;
    tx.commit().map_err(sql_error)?;
    Ok(Some(changed))
}

/// sqlite.ts:40 used by codex: runs `select` (its first column as text) then `update` in one transaction; `None` where
/// the file is absent. Answers the selected values and the rows the update changed.
pub fn select_then_update(
    db: &str,
    select: &str,
    select_params: &[(String, SqlValue)],
    update: &RowUpdate,
) -> Result<Option<(Vec<String>, u64)>> {
    if !Path::new(db).exists() {
        return Ok(None);
    }
    let mut conn = open(db, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    let tx = conn.transaction().map_err(sql_error)?;
    let values: Vec<String> =
        query(&tx, select, select_params)?.iter().map(|row| row.values().next().map_or_else(String::new, js_string)).collect();
    let changed = run_update(&tx, update)?;
    tx.commit().map_err(sql_error)?;
    Ok(Some((values, changed)))
}

pub(crate) fn from_param(from: &str) -> Vec<(String, SqlValue)> {
    vec![("$from".into(), SqlValue::Text(from.into()))]
}

/// `String(value)` for a value node:sqlite returned: null as "null", a blob as its bytes joined with commas.
pub(crate) fn js_string(value: &SqlValue) -> String {
    match value {
        SqlValue::Null => "null".into(),
        SqlValue::Integer(i) => i.to_string(),
        SqlValue::Real(f) => number_string(*f),
        SqlValue::Text(s) => s.clone(),
        SqlValue::Blob(b) => b.iter().map(u8::to_string).collect::<Vec<_>>().join(","),
    }
}

/// resolver.ts:100 for a stored value: a text value moved when it is a path at or under `from`, any other as it was,
/// and an absent column as null.
pub(crate) fn moved_sql(value: Option<&SqlValue>, from: &str, to: &str) -> SqlValue {
    match value {
        Some(SqlValue::Text(path)) => SqlValue::Text(moved_path(path, from, to).unwrap_or_else(|| path.clone())),
        Some(other) => other.clone(),
        None => SqlValue::Null,
    }
}

pub(crate) fn shared_filter(tables: &[SharedTable]) -> Vec<String> {
    let owned: Vec<ProjectTable> = tables.iter().map(|t| ProjectTable { table: t.table.clone(), r#where: t.r#where.clone() }).collect();
    sqlite_filter(&owned)
}

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

const MERGE_TAIL: &[&str] = &[
    "con = sqlite3.connect(DB)",
    "names = {r[0] for r in con.execute(\"select name from sqlite_master where type = 'table'\")}",
    "for t in TABLES:",
    "    if t[\"table\"] not in names:",
    "        out({\"waiting\": \"no table \" + t[\"table\"] + \" in \" + DB})",
    "def value(v):",
    "    return base64.b64decode(v[\"$blob\"]) if isinstance(v, dict) else v",
    "def insert(t, row, cols, on_conflict):",
    "    sql = \"insert into \" + q(t[\"table\"]) + \" (\" + \", \".join(q(c) for c in cols) + \") values (\" + \", \".join(\"?\" for c in cols) + \")\" + on_conflict",
    "    return con.execute(sql, [value(row[c]) for c in cols]).rowcount",
    "def merge_table(t):",
    "    global merged, kept",
    "    if \"by\" in t:",
    "        groups = {}",
    "        for row in t[\"rows\"]:",
    "            groups.setdefault(row[t[\"by\"]], []).append(row)",
    "        for owner, rows in groups.items():",
    "            held = con.execute(\"select count(*) from \" + q(t[\"table\"]) + \" where \" + q(t[\"by\"]) + \" = ?\", (owner,)).fetchone()[0]",
    "            if held:",
    "                kept += len(rows)",
    "                continue",
    "            for row in rows:",
    "                merged += insert(t, row, [c for c in row if c not in t[\"drop\"]], \"\")",
    "        return",
    "    if t[\"set\"]:",
    "        action = \"update set \" + \", \".join(q(c) + \" = excluded.\" + q(c) for c in t[\"set\"]) + \" where \" + \" or \".join(q(t[\"table\"]) + \".\" + q(c) + \" is not excluded.\" + q(c) for c in t[\"set\"])",
    "    else:",
    "        action = \"nothing\"",
    "    for row in t[\"rows\"]:",
    "        n = insert(t, row, list(row), \" on conflict (\" + \", \".join(q(c) for c in t[\"key\"]) + \") do \" + action)",
    "        merged += n",
    "        kept += 1 - n",
    "try:",
    "    with con:",
    "        for t in TABLES:",
    "            merge_table(t)",
    "except sqlite3.Error as e:",
    "    fail(\"merge into \" + DB + \" failed: \" + str(e))",
    "finally:",
    "    con.close()",
];

fn merge_table_json(t: &MergeTable) -> Value {
    let rows = Value::Array(t.rows.iter().map(portable).collect());
    match &t.mode {
        MergeMode::Keyed { key, set } => json!({ "table": t.table, "key": key, "set": set, "rows": rows }),
        MergeMode::By { by, drop } => json!({ "table": t.table, "by": by, "drop": drop, "rows": rows }),
    }
}

fn open(db: &str, flags: OpenFlags) -> Result<Connection> {
    // rusqlite adds the path to sqlite's own words for a file that will not open, which node's sqlite does not.
    let conn = Connection::open_with_flags(db, flags).map_err(|e| match sql_error(e) {
        EngineError::Other { message, code } => {
            let message = message.strip_suffix(&format!(": {db}")).map_or(message.clone(), str::to_string);
            EngineError::Other { message, code }
        }
        other => other,
    })?;
    conn.busy_timeout(Duration::ZERO).map_err(sql_error)?;
    Ok(conn)
}

fn sql_error(e: rusqlite::Error) -> EngineError {
    let message = match &e {
        rusqlite::Error::SqliteFailure(_, Some(text)) => text.clone(),
        other => other.to_string(),
    };
    EngineError::Other { message, code: None }
}

fn bind(value: &SqlValue) -> RValue {
    match value {
        SqlValue::Null => RValue::Null,
        SqlValue::Integer(i) => RValue::Integer(*i),
        SqlValue::Real(f) => RValue::Real(*f),
        SqlValue::Text(s) => RValue::Text(s.clone()),
        SqlValue::Blob(b) => RValue::Blob(b.clone()),
    }
}

fn bind_all(params: &[(String, SqlValue)]) -> Vec<(String, RValue)> {
    params.iter().map(|(name, value)| (name.clone(), bind(value))).collect()
}

fn named(bound: &[(String, RValue)]) -> Vec<(&str, &dyn ToSql)> {
    bound.iter().map(|(name, value)| (name.as_str(), value as &dyn ToSql)).collect()
}

fn read_value(value: ValueRef<'_>) -> Result<SqlValue> {
    Ok(match value {
        ValueRef::Null => SqlValue::Null,
        // node:sqlite's own range check takes `abs` of the integer, which overflows for the smallest one and lets it through,
        // as the double it reads as.
        ValueRef::Integer(i64::MIN) => SqlValue::Real(i64::MIN as f64),
        ValueRef::Integer(i) if i.unsigned_abs() > MAX_SAFE_INTEGER => {
            return Err(EngineError::Other {
                message: format!("Value is too large to be represented as a JavaScript number: {i}"),
                code: None,
            });
        }
        ValueRef::Integer(i) => SqlValue::Integer(i),
        ValueRef::Real(f) => SqlValue::Real(f),
        ValueRef::Text(text) => SqlValue::Text(String::from_utf8_lossy(text).into_owned()),
        ValueRef::Blob(bytes) => SqlValue::Blob(bytes.to_vec()),
    })
}

fn query(conn: &Connection, sql: &str, params: &[(String, SqlValue)]) -> Result<Vec<Row>> {
    let mut stmt = conn.prepare(sql).map_err(sql_error)?;
    let names: Vec<String> = stmt.column_names().into_iter().map(str::to_string).collect();
    let bound = bind_all(params);
    let mut rows = stmt.query(named(&bound).as_slice()).map_err(sql_error)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().map_err(sql_error)? {
        let mut picked = Row::new();
        for (i, name) in names.iter().enumerate() {
            picked.insert(name.clone(), read_value(row.get_ref(i).map_err(sql_error)?)?);
        }
        out.push(picked);
    }
    Ok(out)
}

fn run_update(conn: &Connection, update: &RowUpdate) -> Result<u64> {
    let bound = bind_all(&update.params);
    conn.execute(&update.sql, named(&bound).as_slice()).map(|n| n as u64).map_err(sql_error)
}
