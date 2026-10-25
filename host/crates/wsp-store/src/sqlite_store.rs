// SPDX-License-Identifier: AGPL-3.0-only
//! The host's state in one SQLite database beside the state file, in chapter 02's fresh schema: no migrations table,
//! no import, no shape upgrade. Port of packages/runtime/src/sqlite-store.ts at 65f6b89c7 less what Proposal 3 drops;
//! the spec is spec/sqlite_store.md. The connection lives on the state owner's thread and runs synchronously (rule
//! 2.20); the checkpointer keeps a connection of its own on a thread of its own.
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::Value;
use wsp_words::exit::HostError;

use crate::sqlite_transcripts::{SqliteTranscripts, TranscriptRows};
use crate::store::{BlobMark, StateWriter, Store};

/// The whole schema, run once in one `begin immediate` on a database with no `shape` table (02, Proposal 3): the TS
/// host's seven statements less `migrations`, verbatim (sqlite-store.ts:30-32, sqlite-transcripts.ts:15-17).
pub const SCHEMA: &[&str] = &[
    "create table collections (collection text not null, id text not null, json text not null, primary key (collection, id))",
    "create table blobs (collection text not null, id text not null, bytes blob not null, at real not null, primary key (collection, id))",
    "create table shape (one integer primary key check (one = 1), json text not null)",
    "create table events (workspace text not null, pos integer not null, thread text not null, type text not null, size integer not null, json text not null, primary key (workspace, pos))",
    "create index events_thread on events (workspace, thread, pos)",
    "create table transcript_index (workspace text primary key, json text not null)",
];

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
    todo!()
}

/// `the checkpoints of <db> stopped (<why>), so a commit takes one past 64 MB of WAL` (sqlite-store.ts:114).
pub fn checkpoints_stopped_line(db_path: &str, why: &str) -> String {
    todo!()
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
    todo!()
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
        todo!()
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
        todo!()
    }
}

/// Watches checkpointer threads for one database (sqlite-store.ts:109-131): one that cannot be made, or ends while
/// the database file is there, is made again `CHECKPOINT_EVERY_MS` later, at most `CHECKPOINTER_TRIES` in a row; one
/// that lived `CHECKPOINTER_LIVED_MS` resets the count. Giving up warns `checkpoints_stopped_line` once. Nothing here
/// holds the store's thread or keeps the process alive.
pub fn checkpointer(db_path: &Path, options: &SqliteStoreOptions) {
    todo!()
}

/// The state in SQLite (sqlite-store.ts:278-384), opened at its first call (rule 2.25). Not `Send`: it lives on the
/// state owner's thread.
pub fn sqlite_store(state_path: &Path, writer: StateWriter) -> SqliteStore {
    todo!()
}

pub fn sqlite_store_with(state_path: &Path, writer: StateWriter, options: SqliteStoreOptions) -> SqliteStore {
    todo!()
}

/// Builds `state.db` when it is not there (sqlite-store.ts:232-273 less the import): clears dead builds, makes
/// `state.db.importing-<pid>` empty at 0600, switches it to WAL, closes it and links it to `state.db`; a link that
/// finds `state.db` made returns quietly; the build file is removed on every road out. Public for the race test.
pub fn build_database(db_path: &Path) -> Result<(), HostError> {
    todo!()
}

pub struct SqliteStore {
    inner: Rc<SqliteInner>,
    transcripts: SqliteTranscripts,
}

/// What the store and its transcript rows share: the lazily opened connection (sqlite-store.ts:280-337).
pub(crate) struct SqliteInner {
    pub(crate) state_path: PathBuf,
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
        todo!()
    }
}

impl SqliteInner {
    /// `ready()` (sqlite-store.ts:289-337): the connection, opened and checked at the first call, and the shape checked
    /// again whenever another connection wrote since.
    pub(crate) fn ready(&self) -> Result<Rc<rusqlite::Connection>, HostError> {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl Store for SqliteStore {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        todo!()
    }
    async fn put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        todo!()
    }
    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError> {
        todo!()
    }
    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError> {
        todo!()
    }
    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        todo!()
    }
    async fn get_blob(&self, collection: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        todo!()
    }
    async fn put_blob(&self, collection: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError> {
        todo!()
    }
    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), HostError> {
        todo!()
    }
    async fn stat_blob(&self, collection: &str, id: &str) -> Result<Option<BlobMark>, HostError> {
        todo!()
    }
    fn transcripts(&self) -> Option<&dyn TranscriptRows> {
        Some(&self.transcripts)
    }
}
