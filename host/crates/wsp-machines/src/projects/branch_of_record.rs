// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/projects/branch-of-record.ts` at 65f6b89c7: the one branch a thread on a machine whose
//! credentials this host keeps may push, and the repo it is on by GitHub's id, read with gh on this computer. Spec:
//! `spec/branch_of_record.md`.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use indexmap::IndexMap;

use wsp_core::devices::{MintMore, PairedDevice};
use wsp_core::types::pending::Clock;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_engine::machine::Machine;
use wsp_wire::views::workspace::{BranchOfRecord, ProjectView};
use wsp_wire::views::workspace_more::ThreadScope;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

/// branch-of-record.ts:57.
pub const PUBLIC_REREAD_MS: i64 = 5 * 60_000;
/// branch-of-record.ts:74.
pub const MARKETPLACES_REREAD_MS: i64 = 10 * 60_000;
/// branch-of-record.ts:76.
pub const MARKETPLACES_WAIT_MS: i64 = 3_000;

/// branch-of-record.ts:17 `Repo`: the branch of record less its branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoOfRecord {
    pub repo_id: i64,
    pub owner: String,
    pub name: String,
    pub public: bool,
}

/// A live workspace as these roads read it (TS `LiveWorkspace`): its shared record, the thread its first start
/// opens, whether its image already holds the checkout, and its machine.
#[derive(Clone)]
pub struct OnRecordEntry {
    pub record: Rc<RefCell<WorkspaceRecord>>,
    pub opens_thread: Option<String>,
    pub of_record_held: bool,
    pub machine: Arc<dyn Machine>,
}

/// One marketplace file a catalog agent reads: its path under the home (`~/` cut) and the repos its text names.
#[derive(Clone)]
pub struct MarketplaceFile {
    pub path: String,
    pub github_repos: Rc<dyn Fn(&str) -> Vec<String>>,
}

/// Every call the branch-of-record roads make on the rest of the runtime.
#[async_trait(?Send)]
pub trait BranchCtx {
    fn clock(&self) -> Rc<dyn Clock>;
    fn warn(&self, line: &str);
    /// `ctx.opts.broker?.ready() === true`.
    fn broker_ready(&self) -> bool;
    /// This computer's machine's `exec` (`ctx.moduleOf("local").backend(..).get(LOCAL_MACHINE_ID).exec`).
    async fn exec_here(&self, cmd: &str, timeout_ms: u64) -> Result<ExecResult, HostError>;
    /// `ctx.persist(record)`.
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError>;
    /// The live records, in map order (`ctx.live`).
    fn live_records(&self) -> Vec<Rc<RefCell<WorkspaceRecord>>>;
    /// `ctx.deviceDoor.mint`.
    async fn mint(&self, name: &str, scope: ThreadScope, now: i64, more: MintMore) -> Result<PairedDevice, HostError>;
    /// `ctx.deviceDoor.revoke`.
    async fn revoke(&self, id: &str) -> Result<bool, HostError>;
    /// `ctx.moduleOf(kind).turnRoad`, as the mint's road.
    fn turn_road_of(&self, record: &WorkspaceRecord) -> MintMore;
    /// `ctx.placeName(place)`.
    fn place_name(&self, place: &str) -> String;
    /// `places.wired`.
    fn wired(&self) -> String;
    /// `ctx.opts.local?.homeDir ?? homedir()`.
    fn home_dir(&self) -> String;
    /// Reads a file on this computer; a failure is the TS `readFile` rejection.
    async fn read_file(&self, path: &str) -> Result<String, HostError>;
    /// Every catalog agent's marketplace file, where it names GitHub repos.
    fn marketplace_files(&self) -> Vec<MarketplaceFile>;
    /// project-landing.ts `brokeredGitEnv(token, ofRecord)`.
    fn brokered_git_env(&self, token: &str, of_record: Option<&BranchOfRecord>) -> IndexMap<String, String>;
    /// project-landing.ts `onRecordLines(o)`.
    fn on_record_lines(&self, checkout: &str, of_record: &BranchOfRecord, origin: &str, held: bool) -> Vec<String>;
    /// engine `envExec(machine, script, env, { timeoutMs })`.
    async fn env_exec(&self, machine: Arc<dyn Machine>, script: &str, env: IndexMap<String, String>, timeout_ms: u64) -> Result<ExecResult, HostError>;
}

/// The marketplaces read held per host (branch-of-record.ts:77).
#[derive(Clone)]
pub struct MarketplacesRead {
    pub at: i64,
    pub repos: Rc<RefCell<Vec<String>>>,
    pub read: Shared<LocalBoxFuture<'static, ()>>,
    pub first: bool,
}

/// The roads of branch-of-record.ts over one context; its two memos (`publicReadAt`, `marketplacesRead`) are this
/// area's memory.
pub struct BranchOfRecordArea {
    pub ctx: Rc<dyn BranchCtx>,
    /// `publicReadAt`, by workspace id.
    pub public_read_at: RefCell<IndexMap<String, i64>>,
    pub marketplaces: RefCell<Option<MarketplacesRead>>,
}

/// branch-of-record.ts:21 (B1).
pub fn brokered_start(broker_ready: bool, provider: bool) -> bool {
    todo!()
}

/// branch-of-record.ts:31-34 (B3).
pub fn sha_of(repo: &str, word: Option<&str>) -> Result<String, HostError> {
    todo!()
}

/// branch-of-record.ts:36-40 (B3): JavaScript `Number(word)`, a safe integer above 0.
pub fn id_of(repo: &str, word: Option<&str>) -> Result<i64, HostError> {
    todo!()
}

/// branch-of-record.ts:43-47 (B4).
pub fn github_repo_of(remote: &str, project: &str, on: &str) -> Result<String, HostError> {
    todo!()
}

/// branch-of-record.ts:124-133 (B9).
pub fn free_branch_name(live: &[Rc<RefCell<WorkspaceRecord>>], wanted: &str, heads: Option<&str>) -> String {
    todo!()
}

/// `namedOfRecord`'s ask (branch-of-record.ts:139).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedAsk {
    pub remote: String,
    pub name: String,
    pub project: String,
    pub on: String,
    pub base: Option<String>,
    pub carried: bool,
}

impl BranchOfRecordArea {
    pub fn new(ctx: Rc<dyn BranchCtx>) -> Rc<BranchOfRecordArea> {
        todo!()
    }

    /// branch-of-record.ts:24-29 (B2).
    pub async fn gh_here(&self, repo: &str, args: &[&str]) -> Result<Vec<String>, HostError> {
        todo!()
    }

    /// branch-of-record.ts:51-55 (B5).
    pub async fn repo_of_record(&self, remote: &str, project: &str, on: &str) -> Result<RepoOfRecord, HostError> {
        todo!()
    }

    /// branch-of-record.ts:62-72 (B6).
    pub async fn reread_public(&self, workspace_id: &str, entry: &OnRecordEntry) -> Result<(), HostError> {
        todo!()
    }

    /// branch-of-record.ts:80 (B7).
    pub fn public_marketplaces(&self) -> Vec<String> {
        todo!()
    }

    /// branch-of-record.ts:86-101 (B7).
    pub async fn read_public_marketplaces(self: &Rc<Self>) {
        todo!()
    }

    /// branch-of-record.ts:103-107 (B7).
    pub async fn marketplace_repos(&self) -> Vec<String> {
        todo!()
    }

    /// branch-of-record.ts:113-119 (B8).
    pub async fn pull_request_of_record(&self, remote: &str, number: u64, project: &str, on: &str, pushes: bool) -> Result<BranchOfRecord, HostError> {
        todo!()
    }

    /// branch-of-record.ts:139-147 (B10).
    pub async fn named_of_record(&self, o: &NamedAsk) -> Result<BranchOfRecord, HostError> {
        todo!()
    }

    /// branch-of-record.ts:152-162 (B11): `run` gets the git environment carrying the token.
    pub async fn with_op_token<T>(&self, entry: &OnRecordEntry, op: &str, run: impl FnOnce(IndexMap<String, String>) -> LocalBoxFuture<'static, Result<T, HostError>>) -> Result<T, HostError> {
        todo!()
    }

    /// branch-of-record.ts:167-179 (B12).
    pub async fn land_on_record(&self, entry: &OnRecordEntry, project: &ProjectView, clone: Option<&[String]>) -> Result<(), HostError> {
        todo!()
    }
}
