// SPDX-License-Identifier: AGPL-3.0-only
//! The host's state in one SQLite database beside the state file, the TS host's own: a `state.db` that host made is
//! opened as it is, its `migrations` table and rows left alone, and a file made here is one that host opens (owner's
//! ruling on Proposal 3, #2442; rules 2.19, 2.29). No import, no shape upgrade. Port of
//! packages/runtime/src/sqlite-store.ts at 65f6b89c7 less what Proposal 3 drops; the spec is spec/sqlite_store.md. The connection lives on the state owner's thread and runs synchronously (rule
//! 2.20); the checkpointer keeps a connection of its own on a thread of its own.
use std::cell::{Cell, RefCell};
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde_json::Value;
use wsp_wire::text::json_stringify;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::wire::daemon_version::StateShape;
use wsp_words::exit::HostError;

use crate::own_file::{own_folder, OWN_FILE_MODE};
use crate::sqlite_transcripts::{SqliteTranscripts, TranscriptRows};
use crate::store::{refuse_other_shape, shape_now, state_shape_unreadable_line, state_unreadable_line, BlobMark, StateWriter, Store};

/// The whole schema, run once in one `begin immediate` on a database with no `shape` table: the TS host's own
/// statements, `migrations` first as its `migrate` makes it (sqlite-store.ts:137,30-32, sqlite-transcripts.ts:15-17).
/// A file this host makes holds `migrations` with the rows of `MIGRATED` only so that the TS host opens it: that host
/// runs a module's statements again where its row is missing and fails on `table collections already exists`. This
/// host never reads the table (rule 2.29).
pub const SCHEMA: &[&str] = &[
    "create table migrations (module text primary key, version integer not null)",
    "create table collections (collection text not null, id text not null, json text not null, primary key (collection, id))",
    "create table blobs (collection text not null, id text not null, bytes blob not null, at real not null, primary key (collection, id))",
    "create table shape (one integer primary key check (one = 1), json text not null)",
    "create table events (workspace text not null, pos integer not null, thread text not null, type text not null, size integer not null, json text not null, primary key (workspace, pos))",
    "create index events_thread on events (workspace, thread, pos)",
    "create table transcript_index (workspace text primary key, json text not null)",
];

/// The `migrations` rows the TS host holds for `SCHEMA`: each module's count of statements run (STORE_MODULES, one
/// migration each).
pub const MIGRATED: &[(&str, i64)] = &[("store", 1), ("transcripts", 1)];

/// How long a statement waits on another connection's write before it fails (sqlite-store.ts:53).
pub const BUSY_MS: u64 = 10_000;
/// How often the checkpointer looks at the WAL (sqlite-store.ts:59).
pub const CHECKPOINT_EVERY_MS: u64 = 2_000;
/// The WAL past which a commit checkpoints itself (sqlite-store.ts:63): `wal_autocheckpoint` is this in pages.
pub const WAL_BOUND_BYTES: u64 = 64 * 1024 * 1024;
/// The WAL file size a checkpoint cuts back to (sqlite-store.ts:66): `journal_size_limit`.
pub const WAL_KEPT_BYTES: u64 = 16 * 1024 * 1024;
/// Checkpointer threads in a row that may end early before the checkpoints are left to the bound (sqlite-store.ts:70).
pub const CHECKPOINTER_TRIES: u32 = 3;
/// A checkpointer thread that lived this long counts as no failure (sqlite-store.ts:71).
pub const CHECKPOINTER_LIVED_MS: u64 = 10 * CHECKPOINT_EVERY_MS;

/// `join(dirname(statePath), "state.db")` (sqlite-store.ts:39): the state file itself is only an anchor.
pub fn state_db_path(state_path: &Path) -> PathBuf {
    state_path.parent().unwrap_or(Path::new("")).join("state.db")
}

/// `the checkpoints of <db> stopped (<why>), so a commit takes one past 64 MB of WAL` (sqlite-store.ts:114).
pub fn checkpoints_stopped_line(db_path: &str, why: &str) -> String {
    format!("the checkpoints of {db_path} stopped ({why}), so a commit takes one past {} MB of WAL", WAL_BOUND_BYTES / 1_048_576)
}

/// What one checkpointer thread is handed (sqlite-store.ts:120).
#[derive(Clone, Debug, PartialEq)]
pub struct CheckpointJob {
    pub db_path: PathBuf,
    pub every: Duration,
    pub busy_ms: u64,
}

/// The real checkpointer thread's body (sqlite-store.ts:78-105): every `every`, ends cleanly once the database file
/// is gone; opens the file read-write without creating it, sets `busy_timeout`, and runs
/// `pragma wal_checkpoint(passive)` when `pragma data_version` moved since the last pass that copied every frame.
/// An error while the file is there ends the thread with that error's text.
pub fn checkpoint_loop(job: CheckpointJob) -> Result<(), String> {
    let mut held: Option<Connection> = None;
    let mut done: Option<i64> = None;
    loop {
        std::thread::sleep(job.every);
        if !job.db_path.exists() {
            return Ok(());
        }
        let tick = (|| -> rusqlite::Result<()> {
            if held.is_none() {
                let conn = Connection::open_with_flags(&job.db_path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
                conn.busy_timeout(Duration::from_millis(job.busy_ms))?;
                held = Some(conn);
            }
            let conn = held.as_ref().expect("opened above");
            let now: i64 = conn.query_row("pragma data_version", [], |r| r.get(0))?;
            if done == Some(now) {
                return Ok(());
            }
            let (log, checkpointed): (i64, i64) = conn.query_row("pragma wal_checkpoint(passive)", [], |r| Ok((r.get(1)?, r.get(2)?)))?;
            if log == checkpointed {
                done = Some(now);
            }
            Ok(())
        })();
        if let Err(e) = tick {
            return if job.db_path.exists() { Err(sql_message(&e)) } else { Ok(()) };
        }
    }
}

/// How a checkpointer thread is made, the seam the TS tests mock `node:worker_threads` through: the real one spawns
/// `checkpoint_loop`; a test's fake stalls, ends at once with an error, or cannot be made.
pub trait CheckpointerSpawn: Send + Sync {
    fn spawn(&self, job: CheckpointJob) -> std::io::Result<JoinHandle<Result<(), String>>>;
}

/// Spawns `checkpoint_loop` on a named std thread.
pub struct RealCheckpointer;

impl CheckpointerSpawn for RealCheckpointer {
    fn spawn(&self, job: CheckpointJob) -> std::io::Result<JoinHandle<Result<(), String>>> {
        std::thread::Builder::new().name("wsp-checkpointer".into()).spawn(move || checkpoint_loop(job))
    }
}

/// What a store is built with besides its path and writer.
#[derive(Clone)]
pub struct SqliteStoreOptions {
    pub checkpointer: Arc<dyn CheckpointerSpawn>,
    /// `console.warn`: the give-up line of the checkpointer.
    pub warn: Arc<dyn Fn(&str) + Send + Sync>,
}

impl Default for SqliteStoreOptions {
    fn default() -> Self {
        SqliteStoreOptions { checkpointer: Arc::new(RealCheckpointer), warn: Arc::new(|line: &str| eprintln!("{line}")) }
    }
}

/// Watches checkpointer threads for one database (sqlite-store.ts:109-131): one that cannot be made, or ends while
/// the database file is there, is made again `CHECKPOINT_EVERY_MS` later, at most `CHECKPOINTER_TRIES` in a row; one
/// that lived `CHECKPOINTER_LIVED_MS` resets the count. Giving up warns `checkpoints_stopped_line` once. Nothing here
/// holds the store's thread or keeps the process alive.
pub fn checkpointer(db_path: &Path, options: &SqliteStoreOptions) {
    let job = CheckpointJob { db_path: db_path.to_path_buf(), every: Duration::from_millis(CHECKPOINT_EVERY_MS), busy_ms: BUSY_MS };
    let options = options.clone();
    let supervise = move || {
        let mut tries = CHECKPOINTER_TRIES;
        loop {
            let born = Instant::now();
            let why = match options.checkpointer.spawn(job.clone()) {
                Err(e) => e.to_string(),
                Ok(thread) => {
                    let ended = thread.join();
                    if !job.db_path.exists() {
                        return;
                    }
                    match ended {
                        Ok(Err(e)) => e,
                        _ => "the thread ended".to_string(),
                    }
                }
            };
            let lived = born.elapsed() >= Duration::from_millis(CHECKPOINTER_LIVED_MS);
            let left = if lived { CHECKPOINTER_TRIES } else { tries - 1 };
            if left == 0 {
                (options.warn)(&checkpoints_stopped_line(&job.db_path.display().to_string(), &why));
                return;
            }
            std::thread::sleep(Duration::from_millis(CHECKPOINT_EVERY_MS));
            tries = left;
        }
    };
    let _ = std::thread::Builder::new().name("wsp-checkpointer-watch".into()).spawn(supervise);
}

/// The state in SQLite (sqlite-store.ts:278-384), opened at its first call (rule 2.25). Not `Send`: it lives on the
/// state owner's thread.
pub fn sqlite_store(state_path: &Path, writer: StateWriter) -> SqliteStore {
    sqlite_store_with(state_path, writer, SqliteStoreOptions::default())
}

pub fn sqlite_store_with(state_path: &Path, writer: StateWriter, options: SqliteStoreOptions) -> SqliteStore {
    let inner =
        Rc::new(SqliteInner { db_path: state_db_path(state_path), writer, options, held: RefCell::new(None), last: Cell::new(0.0) });
    let reach = inner.clone();
    let transcripts = SqliteTranscripts::new(Rc::new(move || reach.ready()));
    SqliteStore { inner, transcripts }
}

/// SQLite's own message for an error, unwrapped: what node's binding puts in `Error.message`.
fn sql_message(e: &rusqlite::Error) -> String {
    match e {
        rusqlite::Error::SqliteFailure(_, Some(message)) => message.clone(),
        rusqlite::Error::SqlInputError { msg, .. } => msg.clone(),
        other => other.to_string(),
    }
}

pub(crate) fn db_err(e: rusqlite::Error) -> HostError {
    HostError::new(sql_message(&e))
}

fn io_err(e: std::io::Error) -> HostError {
    HostError::new(e.to_string())
}

/// A statement run to its end, whether or not it answers rows (`pragma journal_mode = wal` does).
fn exec(conn: &Connection, sql: &str) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query([])?;
    while rows.next()?.is_some() {}
    Ok(())
}

fn has_table(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    Ok(conn.query_row("select 1 from sqlite_master where type = 'table' and name = ?", [name], |_| Ok(())).optional()?.is_some())
}

/// The shape document the database holds, nothing where it holds none, and a refusal where it holds one that does not
/// parse (sqlite-store.ts:155-170).
fn shape_in(conn: &Connection, db_path: &Path) -> Result<Option<StateShape>, HostError> {
    if !has_table(conn, "shape").map_err(db_err)? {
        return Ok(None);
    }
    let row: Option<String> = conn.query_row("select json from shape where one = 1", [], |r| r.get(0)).optional().map_err(db_err)?;
    let Some(row) = row else { return Ok(None) };
    let document = serde_json::from_str::<Value>(&row).unwrap_or(Value::String(row));
    match serde_json::from_value::<StateShape>(document.clone()) {
        Ok(wrote) => Ok(Some(wrote)),
        Err(_) => Err(HostError::new(state_shape_unreadable_line(&db_path.display().to_string(), &document))),
    }
}

/// A database holding no shape table yet is a fresh one; one that holds the table is read in no other shape.
fn refuse_shape(conn: &Connection, db_path: &Path) -> Result<(), HostError> {
    if has_table(conn, "shape").map_err(db_err)? {
        refuse_other_shape(&db_path.display().to_string(), shape_in(conn, db_path)?.as_ref())?;
    }
    Ok(())
}

fn data_version(conn: &Connection) -> Result<i64, HostError> {
    conn.query_row("pragma data_version", [], |r| r.get(0)).map_err(db_err)
}

fn alive(pid: &str) -> bool {
    let Some(pid) = pid.parse::<i64>().ok().and_then(|p| i32::try_from(p).ok()) else { return false };
    unsafe { libc::kill(pid, 0) == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) }
}

/// The databases a build whose process is gone left half made, and this process's own from a pid used before
/// (sqlite-store.ts:216-225).
fn clear_dead_builds(db_path: &Path) -> std::io::Result<()> {
    let folder = db_path.parent().unwrap_or(Path::new(""));
    let prefix = format!("{}.importing-", db_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
    for entry in std::fs::read_dir(folder)? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        let Some(rest) = name.strip_prefix(&prefix) else { continue };
        let pid = rest.split('-').next().unwrap_or("");
        if pid.parse::<u32>().ok() == Some(std::process::id()) || !alive(pid) {
            match std::fs::remove_file(folder.join(&name)) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
        }
    }
    Ok(())
}

/// A file of the owner's at 0600, made empty when it is not there and never truncated when it is.
fn own_empty(path: &Path) -> std::io::Result<()> {
    match OpenOptions::new().write(true).create_new(true).mode(OWN_FILE_MODE).open(path) {
        Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => Err(e),
        _ => Ok(()),
    }
}

/// Builds `state.db` when it is not there (sqlite-store.ts:232-273 less the import): clears dead builds, makes
/// `state.db.importing-<pid>` empty at 0600, switches it to WAL, closes it and links it to `state.db`; a link that
/// finds `state.db` made returns quietly; the build file is removed on every road out. Public for the race test.
pub fn build_database(db_path: &Path) -> Result<(), HostError> {
    let mut name = db_path.as_os_str().to_owned();
    name.push(format!(".importing-{}", std::process::id()));
    let building = PathBuf::from(name);
    own_folder(db_path.parent().unwrap_or(Path::new(""))).map_err(io_err)?;
    clear_dead_builds(db_path).map_err(io_err)?;
    own_empty(&building).map_err(io_err)?;
    let built = (|| -> Result<(), HostError> {
        {
            let conn = Connection::open(&building).map_err(db_err)?;
            conn.busy_timeout(Duration::from_millis(BUSY_MS)).map_err(db_err)?;
            exec(&conn, "pragma journal_mode = wal").map_err(db_err)?;
        }
        match std::fs::hard_link(&building, db_path) {
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            other => other.map_err(io_err),
        }
    })();
    let _ = std::fs::remove_file(&building);
    built
}

pub struct SqliteStore {
    inner: Rc<SqliteInner>,
    transcripts: SqliteTranscripts,
}

/// What the store and its transcript rows share: the lazily opened connection (sqlite-store.ts:280-337).
pub(crate) struct SqliteInner {
    pub(crate) db_path: PathBuf,
    pub(crate) writer: StateWriter,
    pub(crate) options: SqliteStoreOptions,
    pub(crate) held: RefCell<Option<Held>>,
    /// The last blob stamp, so two writes inside a millisecond are still two marks (sqlite-store.ts:281-283).
    pub(crate) last: Cell<f64>,
}

pub(crate) struct Held {
    pub(crate) conn: Rc<rusqlite::Connection>,
    /// `pragma data_version` as this connection last saw it.
    pub(crate) seen: i64,
    /// Whether this process has written the shape row yet (sqlite-store.ts:339-346).
    pub(crate) stamped: bool,
}

impl SqliteStore {
    /// `pragma <name>` on the store's own connection, opening it as any call does: what the tests read the
    /// per-connection pragmas of rule 2.25 through (`busy_timeout`, `synchronous`, `wal_autocheckpoint`,
    /// `journal_size_limit`, `journal_mode`). A number answers as a JSON number, text as a string.
    pub fn pragma(&self, name: &str) -> Result<Value, HostError> {
        let conn = self.inner.ready()?;
        conn.query_row(&format!("pragma {name}"), [], |r| {
            Ok(match r.get_ref(0)? {
                rusqlite::types::ValueRef::Integer(n) => Value::from(n),
                rusqlite::types::ValueRef::Real(n) => Value::from(n),
                rusqlite::types::ValueRef::Text(t) => Value::from(String::from_utf8_lossy(t).into_owned()),
                _ => Value::Null,
            })
        })
        .map_err(db_err)
    }
}

impl SqliteInner {
    /// `ready()` (sqlite-store.ts:289-337): the connection, opened and checked at the first call, and the shape checked
    /// again whenever another connection wrote since.
    pub(crate) fn ready(&self) -> Result<Rc<rusqlite::Connection>, HostError> {
        if let Some(held) = self.held.borrow_mut().as_mut() {
            let now = data_version(&held.conn)?;
            if now != held.seen {
                refuse_shape(&held.conn, &self.db_path)?;
                held.seen = now;
            }
            return Ok(held.conn.clone());
        }
        if !self.db_path.exists() {
            build_database(&self.db_path)?;
        }
        let db = self.db_path.display().to_string();
        let conn = Connection::open(&self.db_path)
            .and_then(|c| {
                c.busy_timeout(Duration::from_millis(BUSY_MS))?;
                has_table(&c, "shape")?;
                Ok(c)
            })
            .map_err(|e| HostError::new(state_unreadable_line(&db, &sql_message(&e))))?;
        self.prepare(&conn)?;
        let held = Held { seen: data_version(&conn)?, conn: Rc::new(conn), stamped: false };
        let conn = held.conn.clone();
        *self.held.borrow_mut() = Some(held);
        checkpointer(&self.db_path, &self.options);
        Ok(conn)
    }

    /// The open's checks and switches (sqlite-store.ts:308-322): the shape refused before anything writes, WAL and its
    /// pragmas, then the schema and the shape row in one transaction on a database that has none.
    fn prepare(&self, conn: &Connection) -> Result<(), HostError> {
        refuse_shape(conn, &self.db_path)?;
        exec(conn, "pragma journal_mode = wal").map_err(db_err)?;
        exec(conn, "pragma synchronous = normal").map_err(db_err)?;
        let page: u64 = conn.query_row("pragma page_size", [], |r| r.get(0)).map_err(db_err)?;
        exec(conn, &format!("pragma wal_autocheckpoint = {}", WAL_BOUND_BYTES.div_ceil(page))).map_err(db_err)?;
        exec(conn, &format!("pragma journal_size_limit = {WAL_KEPT_BYTES}")).map_err(db_err)?;
        exec(conn, "begin immediate").map_err(db_err)?;
        let made = (|| -> Result<(), HostError> {
            if has_table(conn, "shape").map_err(db_err)? {
                refuse_shape(conn, &self.db_path)?;
            } else {
                for statement in SCHEMA {
                    exec(conn, statement).map_err(db_err)?;
                }
                for (module, version) in MIGRATED {
                    conn.execute("insert into migrations (module, version) values (?, ?)", params![module, version]).map_err(db_err)?;
                }
                let shape = json_stringify(&serde_json::to_value(shape_now(&self.writer)).expect("a shape writes as JSON"));
                conn.execute("insert into shape (one, json) values (1, ?)", [shape]).map_err(db_err)?;
            }
            exec(conn, "commit").map_err(db_err)
        })();
        if made.is_err() {
            let _ = exec(conn, "rollback");
        }
        made
    }

    /// The first write of this process says which build wrote the database last, as every save of the JSON document did.
    fn writing(&self) -> Result<Rc<Connection>, HostError> {
        let conn = self.ready()?;
        let mut held = self.held.borrow_mut();
        let held = held.as_mut().expect("ready() holds the connection");
        if !held.stamped {
            let shape = json_stringify(&serde_json::to_value(shape_now(&self.writer)).expect("a shape writes as JSON"));
            conn.prepare_cached("insert into shape (one, json) values (1, ?) on conflict (one) do update set json = excluded.json")
                .and_then(|mut s| s.execute([shape]))
                .map_err(db_err)?;
            held.stamped = true;
        }
        Ok(conn)
    }

    /// Monotonic, so two writes of one blob inside a millisecond are still two marks.
    fn stamp(&self) -> f64 {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0);
        let at = now.max(self.last.get() + 0.001);
        self.last.set(at);
        at
    }
}

#[async_trait::async_trait(?Send)]
impl Store for SqliteStore {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        let conn = self.inner.ready()?;
        let json: Option<String> = conn
            .prepare_cached("select json from collections where collection = ? and id = ?")
            .and_then(|mut s| s.query_row([collection, id], |r| r.get(0)).optional())
            .map_err(db_err)?;
        json.map(|j| parse_row(&j)).transpose()
    }
    async fn put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        let json = json_stringify(&value);
        let_go(value);
        let conn = self.inner.writing()?;
        conn.prepare_cached("insert into collections (collection, id, json) values (?, ?, ?) on conflict (collection, id) do update set json = excluded.json")
            .and_then(|mut s| s.execute([collection, id, json.as_str()]))
            .map_err(db_err)?;
        Ok(())
    }
    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError> {
        let conn = self.inner.ready()?;
        let rows: Vec<String> = conn
            .prepare_cached("select json from collections where collection = ? order by rowid")
            .and_then(|mut s| s.query_map([collection], |r| r.get(0))?.collect())
            .map_err(db_err)?;
        rows.iter().map(|j| parse_row(j)).collect()
    }
    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError> {
        let conn = self.inner.ready()?;
        conn.prepare_cached("select id from collections where collection = ? order by rowid")
            .and_then(|mut s| s.query_map([collection], |r| r.get(0))?.collect())
            .map_err(db_err)
    }
    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        let conn = self.inner.writing()?;
        conn.prepare_cached("delete from collections where collection = ? and id = ?")
            .and_then(|mut s| s.execute([collection, id]))
            .map_err(db_err)?;
        Ok(())
    }
    async fn get_blob(&self, collection: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        let conn = self.inner.ready()?;
        conn.prepare_cached("select bytes from blobs where collection = ? and id = ?")
            .and_then(|mut s| s.query_row([collection, id], |r| r.get(0)).optional())
            .map_err(db_err)
    }
    async fn put_blob(&self, collection: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError> {
        let conn = self.inner.writing()?;
        let at = self.inner.stamp();
        conn.prepare_cached("insert into blobs (collection, id, bytes, at) values (?, ?, ?, ?) on conflict (collection, id) do update set bytes = excluded.bytes, at = excluded.at")
            .and_then(|mut s| s.execute(rusqlite::params![collection, id, bytes, at]))
            .map_err(db_err)?;
        Ok(())
    }
    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), HostError> {
        let conn = self.inner.writing()?;
        conn.prepare_cached("delete from blobs where collection = ? and id = ?")
            .and_then(|mut s| s.execute([collection, id]))
            .map_err(db_err)?;
        Ok(())
    }
    async fn stat_blob(&self, collection: &str, id: &str) -> Result<Option<BlobMark>, HostError> {
        let conn = self.inner.ready()?;
        conn.prepare_cached("select length(bytes), at from blobs where collection = ? and id = ?")
            .and_then(|mut s| {
                s.query_row([collection, id], |r| Ok(BlobMark { bytes: JsNumber(r.get::<_, i64>(0)? as f64), at: JsNumber(r.get(1)?) }))
                    .optional()
            })
            .map_err(db_err)
    }
    fn transcripts(&self) -> Option<&dyn TranscriptRows> {
        Some(&self.transcripts)
    }
}

/// A value dropped from a stack of its own: serde_json drops one level of the stack per level of the value.
fn let_go(value: Value) {
    let mut pending = vec![value];
    while let Some(next) = pending.pop() {
        match next {
            Value::Array(items) => pending.extend(items),
            Value::Object(fields) => pending.extend(fields.into_iter().map(|(_, v)| v)),
            _ => {}
        }
    }
}

/// A stored row parsed afresh; a row that is not JSON fails the call (the TS throws `SyntaxError`).
fn parse_row(json: &str) -> Result<Value, HostError> {
    serde_json::from_str(json).map_err(|e| HostError::new(e.to_string()))
}
