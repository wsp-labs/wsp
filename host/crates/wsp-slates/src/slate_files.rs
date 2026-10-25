// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-files.ts at 65f6b89c7. Spec: spec/slate_files.md.
//!
//! The files a slate run reads: the slate's own files its command names, and the scripts in the thread's folder an
//! approval binds by hash, read on this disk or, for a command on the thread's own computer, by that computer's daemon.
#![allow(unused_variables)]

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;

pub use wsp_wire::types::daemon_contract::{FS_HASH_CAP_BYTES, FS_HASH_FILES_MAX, FS_HASH_PATHS_MAX};
pub use wsp_wire::wire::daemon::FsHashReply;

use crate::pending::{SlateDoc, SlateRunDecl};

/// What a read of the files a run names found, on this computer or by the daemon of the thread's own (slate-files.ts:51).
pub type HashRead = FsHashReply;

/// The thread's own computer as its daemon reads the files a run names there, under `root`; fails where that daemon
/// cannot say (slate-files.ts:55).
pub type HashOn = std::rc::Rc<dyn Fn(String, Vec<String>) -> LocalBoxFuture<'static, Result<HashRead, String>>>;

/// Why an Always cannot hold to a file a run names (slate-files.ts:61).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnpinnedWhy {
    Unread,
    Outside,
    Link,
    Past,
    Special,
    Built,
    Paths,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Unpinned {
    pub path: String,
    pub why: UnpinnedWhy,
}

/// The hashes an approval of a run binds, and the files it names that no hash can bind (slate-files.ts:64). `scripts`
/// is absent when no file was hashed; `unpinned` keeps first-keyed order.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Pin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scripts: Option<IndexMap<String, String>>,
    pub unpinned: Vec<Unpinned>,
}

// slate-files.ts:11 filesRead(doc, decl)
/// The slate's files a run's cmd or then reads, in `doc.files` order; `None` when it reads none (spec rule 1).
pub fn files_read(doc: Option<&SlateDoc>, decl: &SlateRunDecl) -> Option<IndexMap<String, String>> {
    todo!()
}

// slate-files.ts:20 writeSlateFiles(dir, files)
/// The slate's folder made to hold its document's files and nothing else (spec rule 3). Answers `dir`.
pub fn write_slate_files(dir: &Path, files: &IndexMap<String, String>) -> std::io::Result<std::path::PathBuf> {
    todo!()
}

/// A declaration with the files it reads beside it, as its approval key and its sheet take it (slate-files.ts:32).
#[derive(Debug, Clone, PartialEq)]
pub struct WithFiles {
    pub decl: SlateRunDecl,
    pub files: Option<IndexMap<String, String>>,
}

impl WithFiles {
    /// The declaration as JSON with `files` last when there are any: what the key and the sheet read.
    pub fn to_json(&self) -> serde_json::Value {
        todo!()
    }
}

// slate-files.ts:32 withFiles(doc, decl)
pub fn with_files(doc: Option<&SlateDoc>, decl: &SlateRunDecl) -> WithFiles {
    todo!()
}

// slate-files.ts:38 pathsNamed(decl)
/// The words of a cmd run's cmd or then that read as a path (spec rule 4). A non-cmd declaration names none.
pub fn paths_named(decl: &SlateRunDecl) -> Vec<String> {
    todo!()
}

// slate-files.ts:44 wordsAt(folder, decl, there)
/// Each word `paths_named` gives, a relative one joined to the folder the command runs in (spec rule 5).
pub fn words_at(folder: &str, decl: &SlateRunDecl, there: bool) -> Vec<String> {
    todo!()
}

// slate-files.ts:68 keyed(pin)
/// The hashes a pin puts in an approval key, each unpinned file standing there by its why (spec rule 10).
pub fn keyed(pin: &Pin) -> Option<IndexMap<String, String>> {
    todo!()
}

// slate-files.ts:87 cannotPin(pin)
pub fn cannot_pin(pin: &Pin) -> Option<String> {
    todo!()
}

// slate-files.ts:93 alwaysRefused(run, pin)
pub fn always_refused(run: &str, pin: &Pin) -> Option<String> {
    todo!()
}

// slate-files.ts:217 pinHere(folder, decl)
/// What an approval of a run on this computer binds; no folder, a missing folder or a non-cmd run pins nothing.
pub fn pin_here(folder: Option<&str>, decl: &SlateRunDecl) -> Pin {
    todo!()
}

/// How long a read of the hashes waits on the thread's computer (slate-files.ts:223).
pub const HASH_WAIT_MS: u64 = 10_000;

/// One cmd a reading covers.
#[derive(Debug, Clone)]
pub struct PinAsk {
    pub folder: Option<String>,
    pub decl: SlateRunDecl,
}

struct Reading {
    at: u64,
    pins: HashMap<String, HashRead>,
}

/// slate-files.ts:231 boxPins(hashOn, waitMs): the last reading per thread of what each command on a thread's own
/// computer names there (spec rule 15). State is borrowed only between awaits (rule 3.3).
pub struct BoxPins {
    hash_on: Box<dyn Fn(&str) -> Option<HashOn>>,
    wait_ms: u64,
    read: RefCell<HashMap<String, Reading>>,
    reads: Cell<u64>,
}

pub fn box_pins(hash_on: Box<dyn Fn(&str) -> Option<HashOn>>, wait_ms: Option<u64>) -> BoxPins {
    todo!()
}

impl BoxPins {
    /// Reads the hashes again for these commands; `asleep` asks nothing, so a napping computer is not woken.
    pub async fn read(&self, thread_id: &str, cmds: &[PinAsk], asleep: bool) {
        todo!()
    }

    /// What an approval of this command binds, from the last reading.
    pub fn pin(&self, thread_id: &str, folder: Option<&str>, decl: &SlateRunDecl) -> Pin {
        todo!()
    }

    pub fn drop_thread(&self, thread_id: &str) {
        todo!()
    }
}
