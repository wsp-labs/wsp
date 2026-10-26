// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/projects/branch-of-record.ts` at 65f6b89c7: the one branch a thread on a machine whose
//! credentials this host keeps may push, and the repo it is on by GitHub's id, read with gh on this computer. Spec:
//! `spec/branch_of_record.md`.
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::channel::oneshot;
use futures::future::{join_all, select, LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::IndexMap;

use wsp_catalog::git_hosts::{git_host_of, remote_host, GitHostId};
use wsp_core::devices::{MintMore, PairedDevice};
use wsp_core::types::pending::Clock;
use wsp_core::types::wiring::{WorkspaceRecord, CLONE_MS};
use wsp_engine::exec_detached::put::INLINE_EXEC_MS;
use wsp_engine::machine::Machine;
use wsp_engine::run_env::with_env_from_input;
use wsp_frames::True;
use wsp_wire::text::json_stringify;
use wsp_wire::types::js_number::JsInt;
use wsp_wire::types::shell_quote::shell_line;
use wsp_wire::views::workspace::{BranchOfRecord, ProjectView, ThreadLit};
use wsp_wire::views::workspace_more::ThreadScope;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;
use wsp_words::js::to_number;
use wsp_words::projects::{clone_on_machine_failed_line, CloneFailed};
use wsp_words::start::owner_repo_of;
use wsp_words::sub_thread::{
    branch_of_record_unread_refusal, brokered_git_hub_only_refusal, clone_no_thread_refusal,
    fork_head_gone_refusal, fork_not_pushable_refusal, sub_branch_of,
};
use wsp_words::words::base::last_line;

/// branch-of-record.ts:57.
pub const PUBLIC_REREAD_MS: i64 = 5 * 60_000;
/// branch-of-record.ts:74.
pub const MARKETPLACES_REREAD_MS: i64 = 10 * 60_000;
/// branch-of-record.ts:76.
pub const MARKETPLACES_WAIT_MS: i64 = 3_000;

const REPO_JQ: &str = "[.id, .owner.login, .name, .private] | @tsv";
const PULL_JQ: &str = "[.head.repo.id, .head.repo.owner.login, .head.repo.name, .head.ref, .maintainer_can_modify, .base.repo.id, .head.sha, .head.repo.private] | @tsv";
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

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
    async fn mint(
        &self,
        name: &str,
        scope: ThreadScope,
        now: i64,
        more: MintMore,
    ) -> Result<PairedDevice, HostError>;
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
    fn brokered_git_env(
        &self,
        token: &str,
        of_record: Option<&BranchOfRecord>,
    ) -> IndexMap<String, String>;
    /// project-landing.ts `onRecordLines(o)`.
    fn on_record_lines(
        &self,
        checkout: &str,
        of_record: &BranchOfRecord,
        origin: &str,
        held: bool,
    ) -> Vec<String>;
    /// engine `envExec(machine, script, env, { timeoutMs })`.
    async fn env_exec(
        &self,
        machine: Arc<dyn Machine>,
        script: &str,
        env: IndexMap<String, String>,
        timeout_ms: u64,
    ) -> Result<ExecResult, HostError>;
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
    provider && broker_ready
}

fn is_commit(word: &str) -> bool {
    word.len() == 40
        && word
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// branch-of-record.ts:31-34 (B3).
pub fn sha_of(repo: &str, word: Option<&str>) -> Result<String, HostError> {
    match word {
        Some(word) if is_commit(word) => Ok(word.to_string()),
        _ => {
            let said = json_stringify(&serde_json::Value::String(word.unwrap_or("").to_string()));
            Err(branch_of_record_unread_refusal(
                repo,
                &format!("no commit in {said}"),
            ))
        }
    }
}

/// branch-of-record.ts:36-40 (B3): JavaScript `Number(word)`, a safe integer above 0.
pub fn id_of(repo: &str, word: Option<&str>) -> Result<i64, HostError> {
    let id = word.map_or(f64::NAN, to_number);
    if id.fract() == 0.0 && id > 0.0 && id <= MAX_SAFE_INTEGER {
        return Ok(id as i64);
    }
    let said = json_stringify(&serde_json::Value::String(word.unwrap_or("").to_string()));
    Err(branch_of_record_unread_refusal(
        repo,
        &format!("no repository id in {said}"),
    ))
}

/// branch-of-record.ts:43-47 (B4).
pub fn github_repo_of(remote: &str, project: &str, on: &str) -> Result<String, HostError> {
    let github = git_host_of(remote).is_some_and(|host| host.id == GitHostId::Github);
    github
        .then(|| owner_repo_of(remote))
        .flatten()
        .ok_or_else(|| {
            brokered_git_hub_only_refusal(
                project,
                &remote_host(remote).unwrap_or_else(|| remote.to_string()),
                on,
            )
        })
}

/// branch-of-record.ts:124-133 (B9).
pub fn free_branch_name(
    live: &[Rc<RefCell<WorkspaceRecord>>],
    wanted: &str,
    heads: Option<&str>,
) -> String {
    let mut taken: HashSet<String> = HashSet::new();
    for record in live {
        let record = record.borrow();
        taken.extend(record.branch_of_record.as_ref().map(|b| b.branch.clone()));
        taken.extend(record.sub.as_ref().map(|s| s.branch.clone()));
    }
    for line in heads.unwrap_or("").split('\n') {
        if let Some(branch) = line
            .split('\t')
            .nth(1)
            .and_then(|r| r.strip_prefix("refs/heads/"))
        {
            taken.insert(branch.to_string());
        }
    }
    let mut branch = wanted.to_string();
    let mut n = 2;
    while taken.contains(&branch) {
        branch = format!("{wanted}-{n}");
        n += 1;
    }
    branch
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
        Rc::new(BranchOfRecordArea {
            ctx,
            public_read_at: RefCell::default(),
            marketplaces: RefCell::default(),
        })
    }

    /// branch-of-record.ts:24-29 (B2).
    pub async fn gh_here(&self, repo: &str, args: &[&str]) -> Result<Vec<String>, HostError> {
        let mut words = vec!["gh"];
        words.extend_from_slice(args);
        let cmd = format!("GH_PROMPT_DISABLED=1 {}", shell_line(&words));
        let read = match self.ctx.exec_here(&cmd, INLINE_EXEC_MS).await {
            Ok(read) => read,
            Err(e) => ExecResult {
                exit_code: JsInt::from(-1),
                stdout: String::new(),
                stderr: e.message,
            },
        };
        let code = read.exit_code.get();
        if code != 0 {
            let why = last_line(&read.stderr).unwrap_or_else(|| format!("gh exited {code}"));
            return Err(branch_of_record_unread_refusal(repo, &why));
        }
        let text = read
            .stdout
            .strip_suffix("\r\n")
            .or_else(|| read.stdout.strip_suffix('\n'))
            .unwrap_or(read.stdout.as_str());
        Ok(text.split('\t').map(str::to_string).collect())
    }

    /// branch-of-record.ts:51-55 (B5).
    pub async fn repo_of_record(
        &self,
        remote: &str,
        project: &str,
        on: &str,
    ) -> Result<RepoOfRecord, HostError> {
        let repo = github_repo_of(remote, project, on)?;
        let answer = self
            .gh_here(&repo, &["api", &format!("repos/{repo}"), "--jq", REPO_JQ])
            .await?;
        let repo_id = id_of(&repo, answer.first().map(String::as_str))?;
        Ok(RepoOfRecord {
            repo_id,
            owner: answer.get(1).cloned().unwrap_or_default(),
            name: answer.get(2).cloned().unwrap_or_default(),
            public: answer.get(3).map(String::as_str) == Some("false"),
        })
    }

    /// branch-of-record.ts:62-72 (B6).
    pub async fn reread_public(
        &self,
        workspace_id: &str,
        entry: &OnRecordEntry,
    ) -> Result<(), HostError> {
        let of_record = entry.record.borrow().branch_of_record.clone();
        let now = self.ctx.clock().now();
        let Some(of_record) = of_record.filter(|r| r.public.is_some()) else {
            return Ok(());
        };
        let last = self.public_read_at.borrow().get(workspace_id).copied();
        if last.is_some_and(|at| now - at < PUBLIC_REREAD_MS) {
            return Ok(());
        }
        self.public_read_at
            .borrow_mut()
            .insert(workspace_id.to_string(), now);
        let label = format!("{}/{}", of_record.owner, of_record.name);
        let hidden = self
            .gh_here(
                &label,
                &[
                    "api",
                    &format!("repositories/{}", of_record.repo_id.get()),
                    "--jq",
                    ".private",
                ],
            )
            .await
            .ok()
            .and_then(|answer| answer.into_iter().next());
        if hidden.as_deref() != Some("true") {
            return Ok(());
        }
        if entry.record.borrow().branch_of_record.as_ref() != Some(&of_record) {
            return Ok(());
        }
        let mut next = of_record;
        next.public = None;
        let snapshot = {
            let mut record = entry.record.borrow_mut();
            record.branch_of_record = Some(next);
            record.clone()
        };
        self.ctx.persist(&snapshot).await
    }

    /// branch-of-record.ts:80 (B7).
    pub fn public_marketplaces(&self) -> Vec<String> {
        self.marketplaces
            .borrow()
            .as_ref()
            .map(|held| held.repos.borrow().clone())
            .unwrap_or_default()
    }

    /// branch-of-record.ts:86-101 (B7).
    pub async fn read_public_marketplaces(self: &Rc<Self>) {
        let now = self.ctx.clock().now();
        let current = self.marketplaces.borrow().clone();
        let held = match current {
            Some(held) if now - held.at < MARKETPLACES_REREAD_MS => held,
            current => {
                let next = self.start_marketplaces_read(now, current.as_ref());
                *self.marketplaces.borrow_mut() = Some(next.clone());
                next
            }
        };
        if !held.first || held.read.clone().now_or_never().is_some() {
            return;
        }
        let (tx, rx) = oneshot::channel::<()>();
        let cancel = self.ctx.clock().schedule(
            Box::new(move || {
                let _ = tx.send(());
            }),
            MARKETPLACES_WAIT_MS,
        );
        let _ = select(held.read.clone(), rx).await;
        cancel();
    }

    fn start_marketplaces_read(
        self: &Rc<Self>,
        now: i64,
        held: Option<&MarketplacesRead>,
    ) -> MarketplacesRead {
        let repos = Rc::new(RefCell::new(
            held.map(|h| h.repos.borrow().clone()).unwrap_or_default(),
        ));
        let landing = repos.clone();
        let this = self.clone();
        let work: LocalBoxFuture<'static, ()> = async move {
            let found = this.marketplace_repos().await;
            let checks = found.into_iter().map(|repo| {
                let this = this.clone();
                async move {
                    let answer = this
                        .gh_here(
                            &repo,
                            &["api", &format!("repos/{repo}"), "--jq", ".private"],
                        )
                        .await;
                    match answer.ok().and_then(|a| a.into_iter().next()) {
                        Some(hidden) if hidden == "false" => Some(repo),
                        _ => None,
                    }
                }
            });
            let read: Vec<String> = join_all(checks).await.into_iter().flatten().collect();
            *landing.borrow_mut() = read;
        }
        .boxed_local();
        let read = work.shared();
        crate::local_task::spawn(read.clone());
        MarketplacesRead {
            at: now,
            repos,
            read,
            first: held.is_none(),
        }
    }

    /// branch-of-record.ts:103-107 (B7).
    pub async fn marketplace_repos(&self) -> Vec<String> {
        let home = self.ctx.home_dir();
        let reads = self.ctx.marketplace_files().into_iter().map(|file| {
            let path = format!(
                "{}/{}",
                home.trim_end_matches('/'),
                file.path.strip_prefix("~/").unwrap_or(&file.path)
            );
            let ctx = self.ctx.clone();
            async move {
                match ctx.read_file(&path).await {
                    Ok(text) => (file.github_repos)(&text),
                    Err(_) => Vec::new(),
                }
            }
        });
        let mut repos: Vec<String> = Vec::new();
        for repo in join_all(reads).await.into_iter().flatten() {
            if !repos.contains(&repo) {
                repos.push(repo);
            }
        }
        repos
    }

    /// branch-of-record.ts:113-119 (B8).
    pub async fn pull_request_of_record(
        &self,
        remote: &str,
        number: u64,
        project: &str,
        on: &str,
        pushes: bool,
    ) -> Result<BranchOfRecord, HostError> {
        let repo = github_repo_of(remote, project, on)?;
        let answer = self
            .gh_here(
                &repo,
                &[
                    "api",
                    &format!("repos/{repo}/pulls/{number}"),
                    "--jq",
                    PULL_JQ,
                ],
            )
            .await?;
        let field = |i: usize| answer.get(i).map(String::as_str);
        let id = field(0);
        if id.is_none_or(str::is_empty) {
            return Err(fork_head_gone_refusal(number as f64));
        }
        let owner = field(1);
        if pushes && field(5) != id && field(4) != Some("true") {
            return Err(fork_not_pushable_refusal(
                owner.unwrap_or(""),
                number as f64,
            ));
        }
        let repo_id = id_of(&repo, id)?;
        let start_sha = sha_of(&repo, field(6))?;
        Ok(BranchOfRecord {
            repo_id: JsInt::from(repo_id),
            owner: owner.unwrap_or("").to_string(),
            name: field(2).unwrap_or("").to_string(),
            branch: field(3).unwrap_or("").to_string(),
            start_sha: Some(start_sha),
            public: (field(7) == Some("false")).then_some(True),
        })
    }

    /// branch-of-record.ts:139-147 (B10).
    pub async fn named_of_record(&self, o: &NamedAsk) -> Result<BranchOfRecord, HostError> {
        let repo = self.repo_of_record(&o.remote, &o.project, &o.on).await?;
        let owner_repo = format!("{}/{}", repo.owner, repo.name);
        let tip = if o.carried {
            None
        } else {
            let base = o.base.as_deref().unwrap_or("HEAD");
            self.gh_here(
                &owner_repo,
                &[
                    "api",
                    &format!("repos/{owner_repo}/commits/{base}"),
                    "--jq",
                    ".sha",
                ],
            )
            .await?
            .into_iter()
            .next()
        };
        let heads_cmd = format!(
            "GIT_TERMINAL_PROMPT=0 {}",
            shell_line(&["git", "ls-remote", "--heads", o.remote.as_str()])
        );
        let heads = match self.ctx.exec_here(&heads_cmd, INLINE_EXEC_MS).await {
            Ok(read) if read.exit_code.get() == 0 => Some(read.stdout),
            _ => None,
        };
        let branch = free_branch_name(
            &self.ctx.live_records(),
            &sub_branch_of(&o.name),
            heads.as_deref(),
        );
        let start_sha = if o.carried {
            None
        } else {
            Some(sha_of(&owner_repo, tip.as_deref())?)
        };
        Ok(BranchOfRecord {
            repo_id: JsInt::from(repo.repo_id),
            owner: repo.owner,
            name: repo.name,
            public: repo.public.then_some(True),
            branch,
            start_sha,
        })
    }

    /// branch-of-record.ts:152-162 (B11): `run` gets the git environment carrying the token.
    pub async fn with_op_token<T>(
        &self,
        entry: &OnRecordEntry,
        op: &str,
        run: impl FnOnce(IndexMap<String, String>) -> LocalBoxFuture<'static, Result<T, HostError>>,
    ) -> Result<T, HostError> {
        let snapshot = entry.record.borrow().clone();
        let Some(thread_id) = snapshot
            .parent_thread_id
            .clone()
            .or_else(|| entry.opens_thread.clone())
        else {
            return Err(clone_no_thread_refusal(&snapshot.name));
        };
        let scope = ThreadScope {
            kind: ThreadLit,
            thread_id: thread_id.clone(),
            workspace_id: snapshot.id.clone(),
            root_thread_id: snapshot.root_thread_id.clone().unwrap_or(thread_id),
        };
        let more = self.ctx.turn_road_of(&snapshot);
        let minted = self
            .ctx
            .mint(&format!("op {op}"), scope, self.ctx.clock().now(), more)
            .await?;
        let of_record = entry.record.borrow().branch_of_record.clone();
        let result = run(self
            .ctx
            .brokered_git_env(&minted.device_token, of_record.as_ref()))
        .await;
        if let Err(e) = self.ctx.revoke(&minted.device_id).await {
            self.ctx.warn(&format!(
                "the {op} token of {} was not taken away: {}",
                snapshot.name, e.message
            ));
        }
        result
    }

    /// branch-of-record.ts:167-179 (B12).
    pub async fn land_on_record(
        &self,
        entry: &OnRecordEntry,
        project: &ProjectView,
        clone: Option<&[String]>,
    ) -> Result<(), HostError> {
        let of_record = entry
            .record
            .borrow()
            .branch_of_record
            .clone()
            .expect("a landing names its branch of record");
        let lines = match clone {
            Some(lines) => lines.to_vec(),
            None => {
                let origin = owner_repo_of(&project.remote).unwrap_or_default();
                let mut lines = vec!["set -e".to_string()];
                lines.extend(self.ctx.on_record_lines(
                    &project.path,
                    &of_record,
                    &origin,
                    entry.of_record_held,
                ));
                lines
            }
        };
        let script = with_env_from_input(&lines.join("\n"));
        let ctx = self.ctx.clone();
        let machine = entry.machine.clone();
        let ran = self
            .with_op_token(entry, "clone", move |env| {
                async move { ctx.env_exec(machine, &script, env, CLONE_MS).await }.boxed_local()
            })
            .await?;
        if ran.exit_code.get() != 0 {
            return Err(match clone {
                None => {
                    let said = last_line(&ran.stderr).or_else(|| last_line(&ran.stdout));
                    HostError::new(
                        said.unwrap_or_else(|| format!("git exited {}", ran.exit_code.get())),
                    )
                }
                Some(_) => {
                    let said = if ran.stderr.is_empty() {
                        ran.stdout.clone()
                    } else {
                        ran.stderr.clone()
                    };
                    let place = entry
                        .record
                        .borrow()
                        .place
                        .clone()
                        .unwrap_or_else(|| self.ctx.wired());
                    HostError::new(clone_on_machine_failed_line(&CloneFailed {
                        repo: owner_repo_of(&project.remote)
                            .unwrap_or_else(|| project.remote.clone()),
                        on: self.ctx.place_name(&place),
                        exit_code: ran.exit_code.get() as i32,
                        said,
                        limit_ms: CLONE_MS as f64,
                    }))
                }
            });
        }
        let at = match last_line(&ran.stdout) {
            Some(at) if is_commit(&at) && of_record.start_sha.is_none() => at,
            _ => return Ok(()),
        };
        let snapshot = {
            let mut record = entry.record.borrow_mut();
            record.branch_of_record = Some(BranchOfRecord {
                start_sha: Some(at),
                ..of_record
            });
            record.clone()
        };
        self.ctx.persist(&snapshot).await
    }
}
