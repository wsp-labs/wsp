// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/copier.ts at a4033dc34. Spec: spec/copier.md.
//!
//! How a workspace on the computer somebody sits at gets its copy of the project folder: the daemon binary's own
//! copy verb, run as a child of this host with argv and answering one JSON line. One trait, the real road and the
//! stand-in tests drive, so nothing above here knows there is a child process.
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::future::BoxFuture;

pub use wsp_wire::views::workspace_more::{CopyAsk, CopyReport, CopyRoad, WorktreeAsk, WorktreeRemoval, WorktreeReport};
pub use wsp_wire::wire::machine_link::ExecResult;

use crate::errors::Result;
use crate::local_backend::MemoryHere;

/// copier.ts:26 `worktreeMount`'s argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeAt {
    pub from: String,
    pub home: String,
    pub path: String,
}

/// copier.ts:28 `worktreeForget`'s argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeProject {
    pub from: String,
    pub home: String,
    pub project: String,
}

/// copier.ts:31 `worktreeRemove`'s argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRemove {
    pub from: String,
    pub home: String,
    pub path: String,
    pub force: bool,
}

/// copier.ts:15. Every error is `EngineError::Other { message, code: None }` whose message is the sentence the TS
/// throws.
#[async_trait]
pub trait Copier: Send + Sync {
    /// Makes the copy and answers what the verb reported; the verb's own sentence when it refused.
    async fn make(&self, ask: &CopyAsk) -> Result<CopyReport>;
    /// Takes a copy away by the road that made it, which is the road the record carries.
    async fn remove(&self, from: &str, to: &str, road: CopyRoad) -> Result<()>;
    /// The worktree holding the branch.
    async fn worktree(&self, ask: &WorktreeAsk) -> Result<WorktreeReport>;
    /// Mounts again the dependency overlays of a worktree wsp made.
    async fn worktree_mount(&self, o: &WorktreeAt) -> Result<()>;
    /// Takes away the frozen copies a project's worktrees sat on.
    async fn worktree_forget(&self, o: &WorktreeProject) -> Result<()>;
    /// Takes a worktree wsp made away with git.
    async fn worktree_remove(&self, o: &WorktreeRemove) -> Result<WorktreeRemoval>;
    /// Whether this copier has the TS's optional `room` (copier.ts:34). The stand-in has it only when handed one.
    fn has_room(&self) -> bool;
    /// The memory of the computer the copy lands on. `None` where `has_room` is false, and where the reading failed.
    async fn room(&self) -> Option<MemoryHere>;
}

/// copier.ts:39. How long one copy has: ten minutes.
pub const COPY_TIMEOUT_MS: u64 = 10 * 60_000;
/// copier.ts:41. A mount again is a few mount calls.
pub const MOUNT_TIMEOUT_MS: u64 = 30_000;

/// What the runner is asked beside argv: the deadline alone, as copier.ts passes `{ timeoutMs }`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChildRunOptions {
    pub timeout_ms: Option<u64>,
}

/// copier.ts:62 `run: typeof runChild`. A runner never fails: a child that could not start is exit 127, one killed at
/// its deadline answers exit -1 with what it printed (child-exec.ts).
pub type ChildRun = Arc<dyn Fn(String, Vec<String>, ChildRunOptions) -> BoxFuture<'static, ExecResult> + Send + Sync>;

/// copier.ts:43 `argvFor`: the argv of `copy make` for one ask, in this order: `copy make --from <from> --to <to>`,
/// `--base <base>` where named, `--exclude <dir>` per exclusion in order, `--size-line-bytes <n>` (decimal), `--road
/// <road>` where named.
pub fn argv_for(ask: &CopyAsk) -> Vec<String> {
    todo!()
}

/// copier.ts:63 `line`: the sentence a failed verb run throws. Exit 1 is the verb's own refusal: its last line of
/// `stderr.trim()`, else of `stdout.trim()`; with both empty it is `the copy failed and said nothing`. Any other exit
/// is `copy_verb_failed_line(exit)`, and what the child said (when not empty) goes to the host's log on stderr as
/// `<binary> copy: <said>`, never to the person.
pub fn refusal_line(binary: &str, res: &ExecResult) -> String {
    todo!()
}

/// copier.ts:62. The daemon binary's verb as a child of this host.
pub struct VerbCopier {
    pub binary: String,
    pub run: ChildRun,
}

/// copier.ts:62 `verbCopier(binary, run = runChild)`. `None` runs `crate::child_exec::run_child`.
pub fn verb_copier(binary: &str, run: Option<ChildRun>) -> VerbCopier {
    todo!()
}

#[async_trait]
impl Copier for VerbCopier {
    async fn make(&self, ask: &CopyAsk) -> Result<CopyReport> {
        todo!()
    }
    async fn remove(&self, from: &str, to: &str, road: CopyRoad) -> Result<()> {
        todo!()
    }
    async fn worktree(&self, ask: &WorktreeAsk) -> Result<WorktreeReport> {
        todo!()
    }
    async fn worktree_mount(&self, o: &WorktreeAt) -> Result<()> {
        todo!()
    }
    async fn worktree_forget(&self, o: &WorktreeProject) -> Result<()> {
        todo!()
    }
    async fn worktree_remove(&self, o: &WorktreeRemove) -> Result<WorktreeRemoval> {
        todo!()
    }
    fn has_room(&self) -> bool {
        todo!()
    }
    /// copier.ts:128: `crate::local_backend::memory_here()`.
    async fn room(&self) -> Option<MemoryHere> {
        todo!()
    }
}

/// The stand-in's `script` (copier.ts:136).
pub type CopyScript = Arc<dyn Fn(&CopyAsk) -> CopyReport + Send + Sync>;
/// The stand-in's `room` (copier.ts:137).
pub type RoomReading = Arc<dyn Fn() -> BoxFuture<'static, Option<MemoryHere>> + Send + Sync>;

/// copier.ts:138: what the stand-in was asked, in the order it was asked.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FakeCopierLog {
    pub asks: Vec<CopyAsk>,
    pub removed: Vec<(String, String, CopyRoad)>,
    pub worktrees: Vec<WorktreeAsk>,
    pub worktrees_mounted: Vec<String>,
    pub worktrees_forgotten: Vec<String>,
    pub worktrees_removed: Vec<WorktreeRemove>,
}

/// copier.ts:135. The stand-in every test above the daemon drives.
pub struct FakeCopier {
    pub log: Arc<Mutex<FakeCopierLog>>,
    pub script: Option<CopyScript>,
    pub room: Option<RoomReading>,
}

/// copier.ts:135 `fakeCopier(script?, room?)`.
pub fn fake_copier(script: Option<CopyScript>, room: Option<RoomReading>) -> FakeCopier {
    todo!()
}

#[async_trait]
impl Copier for FakeCopier {
    /// copier.ts:169. Records the ask; answers `script(ask)` where one was handed, else road `ask.road` or
    /// clonefile, path `ask.to`, base forty `0`s, branch `main`, fetched true, carried deps-and-config, excluded a copy
    /// of `ask.exclude`, bytes 1024, ms 1.
    async fn make(&self, ask: &CopyAsk) -> Result<CopyReport> {
        todo!()
    }
    async fn remove(&self, from: &str, to: &str, road: CopyRoad) -> Result<()> {
        todo!()
    }
    /// copier.ts:153. Records the ask; makes `<home>/worktrees/<project>/<branch with every char outside
    /// [A-Za-z0-9._-] as ->` (mkdir -p) and answers it with made true, carried [], fresh true, modules [], ms 1.
    async fn worktree(&self, ask: &WorktreeAsk) -> Result<WorktreeReport> {
        todo!()
    }
    async fn worktree_mount(&self, o: &WorktreeAt) -> Result<()> {
        todo!()
    }
    async fn worktree_forget(&self, o: &WorktreeProject) -> Result<()> {
        todo!()
    }
    /// copier.ts:165: answers `{ path: o.path }`.
    async fn worktree_remove(&self, o: &WorktreeRemove) -> Result<WorktreeRemoval> {
        todo!()
    }
    fn has_room(&self) -> bool {
        todo!()
    }
    async fn room(&self) -> Option<MemoryHere> {
        todo!()
    }
}
