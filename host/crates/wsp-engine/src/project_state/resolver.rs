// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/resolver.ts. Spec: spec/project_state_files.md.
//!
//! Moving a project's agent state to a new absolute path: the trait every agent module implements, the report it
//! returns, and the file operations the modules share.
use async_trait::async_trait;
use serde_json::{Map, Value};

use wsp_wire::views::project_bundle::ProjectCarry;

use crate::errors::Result;

/// resolver.ts:15. One catalog projectState row the module moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovedState {
    pub state: String,
    pub files: Vec<String>,
    pub changed: u64,
    /// Sessions the row named whose transcript was not under the home; `None`, never `Some(0)`.
    pub skipped: Option<u64>,
}

/// resolver.ts:26. A store one agent keeps for every project: its path under the home and the python3 lines that write
/// PATH's rows of STORE into COPY, counting them in FILTERED.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedStore {
    pub store: String,
    pub filter: Vec<String>,
}

/// A key rule (`keyOf`): the name a project's directory takes for a path.
pub type KeyOf = fn(&str) -> String;

/// resolver.ts:32. Moves one agent's project state inside that agent's home. A TS optional member is a method with a
/// default here: `key` `None`, `listing` `None`, `shared` `None`, and `merge` absent where `has_merge` is false.
#[async_trait]
pub trait ProjectStateResolver: Send + Sync {
    /// The catalog id of the agent whose state this moves.
    fn agent(&self) -> &'static str;
    /// The catalog projectState rows this module moves, in order.
    fn states(&self) -> Vec<&'static str>;
    /// The paths under home this module reads, relative to it, in order.
    fn roots(&self) -> Vec<&'static str>;
    fn carry(&self) -> ProjectCarry;
    /// The name this agent keys a project's state under, where its store takes such a word.
    fn key(&self, _path: &str) -> Option<String> {
        None
    }
    /// Applies the move; empty where nothing in this home was keyed to `from`.
    async fn move_state(&self, home: &str, from: &str, to: &str) -> Result<Vec<MovedState>>;
    /// How many sessions in this home ran at `path` or in a folder under it.
    async fn sessions(&self, home: &str, path: &str) -> Result<u64>;
    /// The files under home holding state for `path` alone, absolute.
    async fn entries(&self, home: &str, path: &str) -> Result<Vec<String>>;
    /// The listing steps for the machine; `None` where the module's whole roots are the answer.
    fn listing(&self, _home: &str, _path: &str) -> Option<Vec<String>> {
        None
    }
    fn shared(&self) -> Option<SharedStore> {
        None
    }
    fn has_merge(&self) -> bool {
        false
    }
    /// The merge script for the machine; `None` where this home holds no row for `from`.
    async fn merge(&self, _home: &str, _from: &str, _to: &str, _guest_home: &str) -> Result<Option<String>> {
        Ok(None)
    }
}

/// resolver.ts:68. node's `path.resolve` against the process's cwd, then the real path where it is a directory that
/// exists; the resolved spelling where it does not or is not a directory. Never fails.
pub fn resolve_project_path(path: &str) -> String {
    todo!()
}

/// resolver.ts:79. `to` plus the tail after `from` where `under_project(path, from)`, else `None`.
pub fn moved_path(path: &str, from: &str, to: &str) -> Option<String> {
    todo!()
}

/// resolver.ts:86. Not empty, not absolute, not `..`, and not opening with `../`; `..cache` is inside.
pub fn inside_folder(rel: &str) -> bool {
    todo!()
}

/// resolver.ts:94. `join(home, parts...)` where its path relative to home stays inside it, else `None`.
pub fn under_home(home: &str, parts: &[&str]) -> Option<String> {
    todo!()
}

/// resolver.ts:100. A string moved when it is a path at or under `from`; any other value as it was.
pub fn moved_or(value: &Value, from: &str, to: &str) -> Value {
    todo!()
}

/// resolver.ts:103. The `cwd` string of the first JSON line carrying one, across the `.jsonl` files under dir in path
/// order; a line that is not JSON, or whose cwd is not a string, is passed over.
pub async fn recorded_cwd(dir: &str) -> Result<Option<String>> {
    todo!()
}

/// resolver.ts:135 answer row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyedDirectory {
    pub dir: String,
    pub cwd: String,
}

/// resolver.ts:135. Rules in spec/project_state_files.md; nothing where `parent` does not exist.
pub async fn keyed_directories(parent: &str, key_of: KeyOf, from: &str) -> Result<Vec<KeyedDirectory>> {
    todo!()
}

/// resolver.ts:152 answer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyedMove {
    pub files: Vec<String>,
    pub changed: u64,
}

/// resolver.ts:152. A destination that exists fails with `<moved> already exists`, directories renamed before it
/// staying renamed.
pub async fn move_keyed_directories(parent: &str, key_of: KeyOf, from: &str, to: &str) -> Result<KeyedMove> {
    todo!()
}

/// resolver.ts:176. The `.jsonl` regular files directly in each keyed directory.
pub async fn keyed_sessions(parent: &str, key_of: KeyOf, path: &str) -> Result<u64> {
    todo!()
}

/// resolver.ts:183. Every regular file under each keyed directory, directory by directory.
pub async fn keyed_entries(parent: &str, key_of: KeyOf, path: &str) -> Result<Vec<String>> {
    todo!()
}

/// resolver.ts:195. Every regular file (not a link) under dir, at any depth, whose name ends with `ext` (empty: all),
/// sorted by path in UTF-16 code unit order; nothing where dir does not exist.
pub fn files_under(dir: &str, ext: &str) -> Result<Vec<String>> {
    todo!()
}

/// resolver.ts:208. Rewrites the JSON lines `edit` returns true for, leaving every other byte alone; answers how many.
/// Rules (temp file, mode, a file with none untouched, a failure leaving no temp file) in spec/project_state_files.md.
pub async fn rewrite_jsonl(file: &str, edit: &mut (dyn FnMut(&mut Map<String, Value>) -> bool + Send)) -> Result<u64> {
    todo!()
}

/// resolver.ts:258 `at`: picks the object holding `cwd` on a line.
pub type CwdHolder = fn(&mut Map<String, Value>) -> Option<&mut Map<String, Value>>;

/// resolver.ts:258. Rewrites `cwd` where it is `from` or under it, in the object `at` picks (the line itself when
/// `None`); a holder that is not an object, or a cwd that is not a string, is no change.
pub async fn rewrite_cwd(file: &str, from: &str, to: &str, at: Option<CwdHolder>) -> Result<u64> {
    todo!()
}
