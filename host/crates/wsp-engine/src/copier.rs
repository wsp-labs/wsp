// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/copier.ts at a4033dc34. Spec: spec/copier.md.
//!
//! How a workspace on the computer somebody sits at gets its copy of the project folder: the daemon binary's own
//! copy verb, run as a child of this host with argv and answering one JSON line. One trait, the real road and the
//! stand-in tests drive, so nothing above here knows there is a child process.
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use serde::de::DeserializeOwned;

use wsp_wire::views::workspace_more::Carried;
pub use wsp_wire::views::workspace_more::{CopyAsk, CopyReport, CopyRoad, WorktreeAsk, WorktreeRemoval, WorktreeReport};
pub use wsp_wire::wire::machine_link::ExecResult;

use wsp_wire::types::check::{parse, Check};
use wsp_words::js::{js_trim, number_string, utf16};
use wsp_words::words::units::copy_verb_failed_line;

use crate::child_exec::{run_child, ChildOptions};
use crate::errors::{EngineError, Result};
use crate::local_backend::{memory_here, MemoryHere};

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
#[async_trait(?Send)]
pub trait Copier {
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
pub type ChildRun = Arc<dyn Fn(String, Vec<String>, ChildRunOptions) -> LocalBoxFuture<'static, ExecResult>>;

fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

fn road_word(road: CopyRoad) -> &'static str {
    match road {
        CopyRoad::Clonefile => "clonefile",
        CopyRoad::Worktree => "worktree",
    }
}

/// copier.ts:43 `argvFor`: the argv of `copy make` for one ask, in this order: `copy make --from <from> --to <to>`,
/// `--base <base>` where named, `--exclude <dir>` per exclusion in order, `--size-line-bytes <n>` (decimal), `--road
/// <road>` where named.
pub fn argv_for(ask: &CopyAsk) -> Vec<String> {
    let mut words = argv(&["copy", "make", "--from", &ask.from, "--to", &ask.to]);
    if let Some(base) = &ask.base {
        words.extend(argv(&["--base", base]));
    }
    for dir in &ask.exclude {
        words.extend(argv(&["--exclude", dir]));
    }
    words.push("--size-line-bytes".into());
    words.push(number_string(ask.size_line_bytes.as_f64()));
    if let Some(road) = ask.road {
        words.extend(argv(&["--road", road_word(road)]));
    }
    words
}

/// copier.ts:63 `line`: the sentence a failed verb run throws. Exit 1 is the verb's own refusal: its last line of
/// `stderr.trim()`, else of `stdout.trim()`; with both empty it is `""` (the TS fallback never fires). Any other exit
/// is `copy_verb_failed_line(exit)`, and what the child said (when not empty) goes to the host's log on stderr as
/// `<binary> copy: <said>`, never to the person.
pub fn refusal_line(binary: &str, res: &ExecResult) -> String {
    let stderr = js_trim(&res.stderr);
    let said = if stderr.is_empty() { js_trim(&res.stdout) } else { stderr };
    if res.exit_code.get() != 1 {
        if !said.is_empty() {
            eprintln!("{binary} copy: {said}");
        }
        return copy_verb_failed_line(res.exit_code.as_f64());
    }
    said.split('\n').next_back().unwrap_or("").to_string()
}

fn refusal(binary: &str, res: &ExecResult) -> EngineError {
    EngineError::Other { message: refusal_line(binary, res), code: None }
}

/// copier.ts:73-106: a refusal is its line; an answer is read through its wire type, or refused as the not-read
/// sentence with the trimmed stdout (cut to 200 UTF-16 units) in place of the answer.
fn answer<T: DeserializeOwned + Check>(what: &str, binary: &str, res: &ExecResult) -> Result<T> {
    if res.exit_code.get() != 0 {
        return Err(refusal(binary, res));
    }
    let said = js_trim(&res.stdout);
    let read = serde_json::from_str::<serde_json::Value>(said).ok().and_then(|v| parse::<T>(&v).ok());
    read.ok_or_else(|| {
        let shown = if said.is_empty() { "nothing at all".to_string() } else { utf16::cut(said, 200) };
        EngineError::Other { message: format!("the {what} verb answered something this host does not read: {shown}"), code: None }
    })
}

/// copier.ts:62. The daemon binary's verb as a child of this host.
pub struct VerbCopier {
    pub binary: String,
    pub run: ChildRun,
}

impl VerbCopier {
    async fn call(&self, words: Vec<String>, timeout_ms: u64) -> ExecResult {
        (self.run)(self.binary.clone(), words, ChildRunOptions { timeout_ms: Some(timeout_ms) }).await
    }

    async fn done(&self, words: Vec<String>, timeout_ms: u64) -> Result<()> {
        let res = self.call(words, timeout_ms).await;
        if res.exit_code.get() != 0 {
            return Err(refusal(&self.binary, &res));
        }
        Ok(())
    }
}

fn child_run(file: String, args: Vec<String>, opts: ChildRunOptions) -> LocalBoxFuture<'static, ExecResult> {
    Box::pin(async move { run_child(&file, &args, ChildOptions { timeout_ms: opts.timeout_ms, ..Default::default() }).await })
}

/// copier.ts:62 `verbCopier(binary, run = runChild)`. `None` runs `crate::child_exec::run_child`.
pub fn verb_copier(binary: &str, run: Option<ChildRun>) -> VerbCopier {
    let run: ChildRun = match run {
        Some(run) => run,
        None => Arc::new(child_run),
    };
    VerbCopier { binary: binary.to_string(), run }
}

#[async_trait(?Send)]
impl Copier for VerbCopier {
    async fn make(&self, ask: &CopyAsk) -> Result<CopyReport> {
        answer("copy", &self.binary, &self.call(argv_for(ask), COPY_TIMEOUT_MS).await)
    }
    async fn remove(&self, from: &str, to: &str, road: CopyRoad) -> Result<()> {
        self.done(argv(&["copy", "remove", "--from", from, "--to", to, "--road", road_word(road)]), COPY_TIMEOUT_MS).await
    }
    async fn worktree(&self, ask: &WorktreeAsk) -> Result<WorktreeReport> {
        let mut words =
            argv(&["copy", "worktree", "--from", &ask.from, "--home", &ask.home, "--project", &ask.project, "--branch", &ask.branch]);
        for module in &ask.modules {
            words.push("--module".into());
            words.push(serde_json::to_string(module).unwrap_or_default());
        }
        if let Some(checkpoint) = &ask.checkpoint {
            words.extend(argv(&["--checkpoint", checkpoint]));
        }
        answer("worktree", &self.binary, &self.call(words, COPY_TIMEOUT_MS).await)
    }
    async fn worktree_mount(&self, o: &WorktreeAt) -> Result<()> {
        self.done(argv(&["copy", "worktree-mount", "--from", &o.from, "--home", &o.home, "--path", &o.path]), MOUNT_TIMEOUT_MS).await
    }
    async fn worktree_forget(&self, o: &WorktreeProject) -> Result<()> {
        self.done(argv(&["copy", "worktree-forget", "--from", &o.from, "--home", &o.home, "--project", &o.project]), COPY_TIMEOUT_MS).await
    }
    async fn worktree_remove(&self, o: &WorktreeRemove) -> Result<WorktreeRemoval> {
        let mut words = argv(&["copy", "worktree-remove", "--from", &o.from, "--home", &o.home, "--path", &o.path]);
        if o.force {
            words.push("--force".into());
        }
        answer("worktree removal", &self.binary, &self.call(words, COPY_TIMEOUT_MS).await)
    }
    fn has_room(&self) -> bool {
        true
    }
    /// copier.ts:128: `crate::local_backend::memory_here()`.
    async fn room(&self) -> Option<MemoryHere> {
        memory_here().await
    }
}

/// The stand-in's `script` (copier.ts:136).
pub type CopyScript = Arc<dyn Fn(&CopyAsk) -> CopyReport>;
/// The stand-in's `room` (copier.ts:137).
pub type RoomReading = Arc<dyn Fn() -> LocalBoxFuture<'static, Option<MemoryHere>>>;

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
    FakeCopier { log: Arc::new(Mutex::new(FakeCopierLog::default())), script, room }
}

/// node's `path.join` of the parts, normalised lexically: empty and `.` segments go, `..` climbs and, on an absolute
/// path, stops at the root.
fn node_join(parts: &[&str]) -> String {
    let joined = parts.iter().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join("/");
    if joined.is_empty() {
        return ".".into();
    }
    let absolute = joined.starts_with('/');
    let mut out: Vec<&str> = Vec::new();
    for segment in joined.split('/') {
        match segment {
            "" | "." => {}
            ".." => match out.last() {
                Some(last) if *last != ".." => {
                    out.pop();
                }
                _ if !absolute => out.push(".."),
                _ => {}
            },
            kept => out.push(kept),
        }
    }
    let body = out.join("/");
    if absolute {
        format!("/{body}")
    } else if body.is_empty() {
        ".".into()
    } else {
        body
    }
}

/// copier.ts:153: each char outside `[A-Za-z0-9._-]` becomes `-`, one per UTF-16 unit.
fn safe_branch(branch: &str) -> String {
    let mut out = String::with_capacity(branch.len());
    for c in branch.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
            out.push(c);
        } else {
            out.push_str(&"-".repeat(c.len_utf16()));
        }
    }
    out
}

#[async_trait(?Send)]
impl Copier for FakeCopier {
    /// copier.ts:169. Records the ask; answers `script(ask)` where one was handed, else road `ask.road` or
    /// clonefile, path `ask.to`, base forty `0`s, branch `main`, fetched true, carried deps-and-config, excluded a copy
    /// of `ask.exclude`, bytes 1024, ms 1.
    async fn make(&self, ask: &CopyAsk) -> Result<CopyReport> {
        self.log.lock().unwrap().asks.push(ask.clone());
        if let Some(script) = &self.script {
            return Ok(script(ask));
        }
        Ok(CopyReport {
            road: ask.road.unwrap_or(CopyRoad::Clonefile),
            path: ask.to.clone(),
            base: "0".repeat(40),
            branch: "main".into(),
            fetched: true,
            carried: Carried::DepsAndConfig,
            excluded: ask.exclude.clone(),
            skipped: None,
            bytes: 1024.into(),
            ms: 1.into(),
            fell_back: None,
        })
    }
    async fn remove(&self, from: &str, to: &str, road: CopyRoad) -> Result<()> {
        self.log.lock().unwrap().removed.push((from.into(), to.into(), road));
        Ok(())
    }
    /// copier.ts:153. Records the ask; makes `<home>/worktrees/<project>/<branch with every char outside
    /// [A-Za-z0-9._-] as ->` (mkdir -p) and answers it with made true, carried [], fresh true, modules [], ms 1.
    async fn worktree(&self, ask: &WorktreeAsk) -> Result<WorktreeReport> {
        self.log.lock().unwrap().worktrees.push(ask.clone());
        let path = node_join(&[&ask.home, "worktrees", &ask.project, &safe_branch(&ask.branch)]);
        std::fs::create_dir_all(&path).map_err(|e| EngineError::Other { message: e.to_string(), code: None })?;
        Ok(WorktreeReport {
            path,
            branch: ask.branch.clone(),
            made: true,
            carried: vec![],
            plain: None,
            overlaid: None,
            fresh: true,
            modules: vec![],
            ms: 1.into(),
        })
    }
    async fn worktree_mount(&self, o: &WorktreeAt) -> Result<()> {
        self.log.lock().unwrap().worktrees_mounted.push(o.path.clone());
        Ok(())
    }
    async fn worktree_forget(&self, o: &WorktreeProject) -> Result<()> {
        self.log.lock().unwrap().worktrees_forgotten.push(o.project.clone());
        Ok(())
    }
    /// copier.ts:165: answers `{ path: o.path }`.
    async fn worktree_remove(&self, o: &WorktreeRemove) -> Result<WorktreeRemoval> {
        self.log.lock().unwrap().worktrees_removed.push(o.clone());
        Ok(WorktreeRemoval { path: o.path.clone(), rescued: None })
    }
    fn has_room(&self) -> bool {
        self.room.is_some()
    }
    async fn room(&self) -> Option<MemoryHere> {
        match &self.room {
            Some(reading) => reading().await,
            None => None,
        }
    }
}
