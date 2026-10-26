// SPDX-License-Identifier: AGPL-3.0-only
//! Every workspace's transcript as rows of the state database, one per event, so a flush appends what a turn wrote
//! and one thread's window is read off the `(workspace, thread, pos)` index. Port of
//! packages/runtime/src/sqlite-transcripts.ts at 65f6b89c7 less the blob move (`moveBlobs`, `indexOfBlob`), which
//! Proposal 3 drops; the spec is spec/sqlite_transcripts.md. The tables are in `sqlite_store::SCHEMA`.
use std::cell::Cell;
use std::ops::ControlFlow;
use std::rc::Rc;

use rusqlite::{params, Connection, OptionalExtension, Params, Row};
use serde_json::Value;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_words::exit::HostError;

use crate::sqlite_store::db_err;
use crate::transcript_reader::{event_json, value_of};

/// An event as a trim weighs it: its place, its thread (`None` for `''`), its type and its size
/// (sqlite-transcripts.ts:26-31).
#[derive(Clone, Debug, PartialEq)]
pub struct EventSize {
    pub pos: i64,
    pub thread_id: Option<String>,
    /// The event's `type`, as `session.delta`.
    pub kind: String,
    pub size: u64,
}

/// Events oldest first and their sizes summed.
#[derive(Clone, Debug, PartialEq)]
pub struct Summed {
    pub events: Vec<SessionEvent>,
    pub size: u64,
}

/// How many events and their sizes summed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Total {
    pub count: u64,
    pub size: u64,
}

/// The transcripts module of a state database (sqlite-transcripts.ts:35-64). Every call is one statement or one
/// transaction, so a reader on another connection sees a flush whole or not at all. Every call reaches the database
/// through the store's `ready()`, so a call can refuse as any store call can.
pub trait TranscriptRows {
    /// Every event the workspace's transcript keeps, oldest first, and their sizes summed.
    fn all(&self, workspace_id: &str) -> Result<Summed, HostError>;
    /// One thread's events newest first, below `before` where it is given, handed to `each` one at a time; `each`
    /// answering `Break` stops the read. `each` runs while the connection is in use: it must not call the store.
    fn newest(
        &self,
        workspace_id: &str,
        thread_id: &str,
        before: Option<i64>,
        each: &mut dyn FnMut(SessionEvent) -> ControlFlow<()>,
    ) -> Result<(), HostError>;
    /// How many events of the thread the transcript keeps.
    fn count(&self, workspace_id: &str, thread_id: &str) -> Result<u64, HostError>;
    /// How many events the transcript keeps and their sizes summed, of one thread where it is named (`""` for none).
    fn total(&self, workspace_id: &str, thread_id: Option<&str>) -> Result<Total, HostError>;
    /// The newest position the transcript keeps, 0 where it keeps none.
    fn newest_pos(&self, workspace_id: &str) -> Result<i64, HostError>;
    /// Every event's size, oldest first, of one thread where it is named.
    fn sizes(&self, workspace_id: &str, thread_id: Option<&str>) -> Result<Vec<EventSize>, HostError>;
    /// One thread's events that can carry its words, oldest first: `session.start`, `session.steer`,
    /// `session.earlier`, and a `session.delta` whose `kind` is `text`.
    fn word_events(&self, workspace_id: &str, thread_id: &str) -> Result<Vec<SessionEvent>, HostError>;
    /// One thread's events, oldest first, and their sizes summed.
    fn thread(&self, workspace_id: &str, thread_id: &str) -> Result<Summed, HostError>;
    /// The events at these positions, in the order asked, a position kept by no row giving nothing.
    fn at(&self, workspace_id: &str, positions: &[i64]) -> Result<Vec<SessionEvent>, HostError>;
    /// Events after every one the transcript keeps, each at its own `pos`, in one transaction.
    fn append(&self, workspace_id: &str, events: &[SessionEvent]) -> Result<(), HostError>;
    fn remove(&self, workspace_id: &str, positions: &[i64]) -> Result<(), HostError>;
    /// The transcript and its index, gone, in one transaction.
    fn clear(&self, workspace_id: &str) -> Result<(), HostError>;
    fn index(&self, workspace_id: &str) -> Result<Option<String>, HostError>;
    fn put_index(&self, workspace_id: &str, json: &str) -> Result<(), HostError>;
    /// `f` in one `begin immediate` transaction, undone whole if it fails; a call inside one joins it.
    fn atomically(&self, f: &mut dyn FnMut() -> Result<(), HostError>) -> Result<(), HostError>;
}

/// The connection a transcript call runs on: the store's `ready()`, which may open the database at the first call.
pub type DbHandle = Rc<dyn Fn() -> Result<Rc<rusqlite::Connection>, HostError>>;

/// The module's rows on the database `db` answers (sqlite-transcripts.ts:74).
pub struct SqliteTranscripts {
    db: DbHandle,
    /// How deep `atomically` is, so an inner call joins the outer transaction (sqlite-transcripts.ts:82-99).
    depth: Cell<u32>,
}

impl SqliteTranscripts {
    pub fn new(db: DbHandle) -> SqliteTranscripts {
        SqliteTranscripts { db, depth: Cell::new(0) }
    }
}

/// The events a thread's words come from (sqlite-transcripts.ts:246), as SQL.
pub const WORDS: &str =
    "(type in ('session.start', 'session.steer', 'session.earlier') or (type = 'session.delta' and json_extract(json, '$.kind') = 'text'))";

/// `Number.MAX_SAFE_INTEGER`: the `before` of a read that names none (sqlite-transcripts.ts:123).
const NO_BEFORE: i64 = 9_007_199_254_740_991;

fn parse_event(json: &str) -> Result<SessionEvent, HostError> {
    serde_json::from_str(json).map_err(|e| HostError::new(e.to_string()))
}

fn run(conn: &Connection, sql: &str, p: impl Params) -> Result<usize, HostError> {
    let mut stmt = conn.prepare_cached(sql).map_err(db_err)?;
    stmt.execute(p).map_err(db_err)
}

fn one<T>(conn: &Connection, sql: &str, p: impl Params, f: impl FnOnce(&Row<'_>) -> rusqlite::Result<T>) -> Result<T, HostError> {
    let mut stmt = conn.prepare_cached(sql).map_err(db_err)?;
    stmt.query_row(p, f).map_err(db_err)
}

fn rows_of<T>(conn: &Connection, sql: &str, p: impl Params, f: impl FnMut(&Row<'_>) -> rusqlite::Result<T>) -> Result<Vec<T>, HostError> {
    let mut stmt = conn.prepare_cached(sql).map_err(db_err)?;
    let mapped = stmt.query_map(p, f).map_err(db_err)?;
    mapped.map(|r| r.map_err(db_err)).collect()
}

fn summed(conn: &Connection, sql: &str, p: impl Params) -> Result<Summed, HostError> {
    let mut out = Summed { events: Vec::new(), size: 0 };
    for (json, size) in rows_of(conn, sql, p, |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
        out.events.push(parse_event(&json)?);
        out.size += size as u64;
    }
    Ok(out)
}

fn size_row(r: &Row<'_>) -> rusqlite::Result<EventSize> {
    let thread: String = r.get(1)?;
    Ok(EventSize { pos: r.get(0)?, thread_id: (!thread.is_empty()).then_some(thread), kind: r.get(2)?, size: r.get::<_, i64>(3)? as u64 })
}

impl TranscriptRows for SqliteTranscripts {
    fn all(&self, workspace_id: &str) -> Result<Summed, HostError> {
        let conn = (self.db)()?;
        summed(&conn, "select json, size from events where workspace = ? order by pos", params![workspace_id])
    }
    fn newest(
        &self,
        workspace_id: &str,
        thread_id: &str,
        before: Option<i64>,
        each: &mut dyn FnMut(SessionEvent) -> ControlFlow<()>,
    ) -> Result<(), HostError> {
        let conn = (self.db)()?;
        let mut stmt = conn
            .prepare_cached("select json from events where workspace = ? and thread = ? and pos < ? order by pos desc")
            .map_err(db_err)?;
        let mut rows = stmt.query(params![workspace_id, thread_id, before.unwrap_or(NO_BEFORE)]).map_err(db_err)?;
        while let Some(row) = rows.next().map_err(db_err)? {
            let json: String = row.get(0).map_err(db_err)?;
            if each(parse_event(&json)?).is_break() {
                break;
            }
        }
        Ok(())
    }
    fn count(&self, workspace_id: &str, thread_id: &str) -> Result<u64, HostError> {
        let conn = (self.db)()?;
        let n = one(&conn, "select count(*) from events where workspace = ? and thread = ?", params![workspace_id, thread_id], |r| {
            r.get::<_, i64>(0)
        })?;
        Ok(n as u64)
    }
    fn total(&self, workspace_id: &str, thread_id: Option<&str>) -> Result<Total, HostError> {
        let conn = (self.db)()?;
        let (count, size) = match thread_id {
            None => one(&conn, "select count(*), sum(size) from events where workspace = ?", params![workspace_id], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?))
            })?,
            Some(thread) => one(
                &conn,
                "select count(*), sum(size) from events where workspace = ? and thread = ?",
                params![workspace_id, thread],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?)),
            )?,
        };
        Ok(Total { count: count as u64, size: size.unwrap_or(0) as u64 })
    }
    fn newest_pos(&self, workspace_id: &str) -> Result<i64, HostError> {
        let conn = (self.db)()?;
        let pos = one(&conn, "select max(pos) from events where workspace = ?", params![workspace_id], |r| r.get::<_, Option<i64>>(0))?;
        Ok(pos.unwrap_or(0))
    }
    fn sizes(&self, workspace_id: &str, thread_id: Option<&str>) -> Result<Vec<EventSize>, HostError> {
        let conn = (self.db)()?;
        match thread_id {
            None => rows_of(
                &conn,
                "select pos, thread, type, size from events where workspace = ? order by pos",
                params![workspace_id],
                size_row,
            ),
            Some(thread) => rows_of(
                &conn,
                "select pos, thread, type, size from events where workspace = ? and thread = ? order by pos",
                params![workspace_id, thread],
                size_row,
            ),
        }
    }
    fn word_events(&self, workspace_id: &str, thread_id: &str) -> Result<Vec<SessionEvent>, HostError> {
        let conn = (self.db)()?;
        let sql = format!("select json from events where workspace = ? and thread = ? and {WORDS} order by pos");
        let jsons = rows_of(&conn, &sql, params![workspace_id, thread_id], |r| r.get::<_, String>(0))?;
        jsons.iter().map(|json| parse_event(json)).collect()
    }
    fn thread(&self, workspace_id: &str, thread_id: &str) -> Result<Summed, HostError> {
        let conn = (self.db)()?;
        summed(&conn, "select json, size from events where workspace = ? and thread = ? order by pos", params![workspace_id, thread_id])
    }
    fn at(&self, workspace_id: &str, positions: &[i64]) -> Result<Vec<SessionEvent>, HostError> {
        let conn = (self.db)()?;
        let mut out = Vec::new();
        for &pos in positions {
            for json in rows_of(&conn, "select json from events where workspace = ? and pos = ?", params![workspace_id, pos], |r| {
                r.get::<_, String>(0)
            })? {
                out.push(parse_event(&json)?);
            }
        }
        Ok(out)
    }
    fn append(&self, workspace_id: &str, events: &[SessionEvent]) -> Result<(), HostError> {
        self.atomically(&mut || {
            let conn = (self.db)()?;
            for e in events {
                let value: Value = value_of(e);
                let Some(pos) = value.get("pos").and_then(Value::as_i64) else {
                    return Err(HostError::new("Provided value cannot be bound to SQLite parameter 2."));
                };
                let thread = value.get("threadId").and_then(Value::as_str).unwrap_or("");
                let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
                let ej = event_json(e);
                run(
                    &conn,
                    "insert into events (workspace, pos, thread, type, size, json) values (?, ?, ?, ?, ?, ?)",
                    params![workspace_id, pos, thread, kind, ej.bytes as i64, ej.json],
                )?;
            }
            Ok(())
        })
    }
    fn remove(&self, workspace_id: &str, positions: &[i64]) -> Result<(), HostError> {
        self.atomically(&mut || {
            let conn = (self.db)()?;
            for &pos in positions {
                run(&conn, "delete from events where workspace = ? and pos = ?", params![workspace_id, pos])?;
            }
            Ok(())
        })
    }
    fn clear(&self, workspace_id: &str) -> Result<(), HostError> {
        self.atomically(&mut || {
            let conn = (self.db)()?;
            run(&conn, "delete from events where workspace = ?", params![workspace_id])?;
            run(&conn, "delete from transcript_index where workspace = ?", params![workspace_id])?;
            Ok(())
        })
    }
    fn index(&self, workspace_id: &str) -> Result<Option<String>, HostError> {
        let conn = (self.db)()?;
        let mut stmt = conn.prepare_cached("select json from transcript_index where workspace = ?").map_err(db_err)?;
        let found = stmt.query_row(params![workspace_id], |r| r.get::<_, String>(0)).optional().map_err(db_err)?;
        Ok(found)
    }
    fn put_index(&self, workspace_id: &str, json: &str) -> Result<(), HostError> {
        let conn = (self.db)()?;
        run(
            &conn,
            "insert into transcript_index (workspace, json) values (?, ?) on conflict (workspace) do update set json = excluded.json",
            params![workspace_id, json],
        )?;
        Ok(())
    }
    fn atomically(&self, f: &mut dyn FnMut() -> Result<(), HostError>) -> Result<(), HostError> {
        if self.depth.get() > 0 {
            return f();
        }
        let conn = (self.db)()?;
        conn.execute_batch("begin immediate").map_err(db_err)?;
        self.depth.set(1);
        let out = f().and_then(|()| conn.execute_batch("commit").map_err(db_err));
        self.depth.set(0);
        if out.is_err() {
            let _ = conn.execute_batch("rollback");
        }
        out
    }
}
