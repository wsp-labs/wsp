// SPDX-License-Identifier: AGPL-3.0-only
//! What `wsp start <url>` and `wsp review` are made of: the project a repository names, the issue and pull request
//! read here, the folder or workspace the thread runs in, the picks checked first, the thread opened, and a
//! reviewer's reply read into a draft. Port of runtime/src/projects/start-from.ts at 65f6b89c7; spec/start_from.md.
use std::rc::Rc;

use async_trait::async_trait;
use futures::FutureExt;
use indexmap::IndexSet;
use serde_json::{json, Value};
use wsp_core::console::warn;
use wsp_core::types::api::StartAs;
use wsp_core::types::harness::StartPicksAsked;
use wsp_core::types::wiring::{CreateWorkspaceOptions, CreatedWorkspace, CLONE_MS};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::pull_request::{GitPrReadReply, GitUpdateReply, PullRequest, PullRequestRecord, PullRequestSeen};
use wsp_wire::types::start::{
    GitIssueReadReply, GitPrDiffReply, IssueRead, ReviewDraft, ReviewDraftV1, WorkspaceFrom, WorkspaceFromHead, WorkspaceFromHeadFork,
    WorkspaceFromKind,
};
use wsp_wire::types::thread_defaults::AccessChoice;
use wsp_wire::types::tree::GitStartOnReply;
use wsp_wire::views::harness::{HarnessCatalog, StartResult};
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{BranchOfRecord, Caller, ProjectView};
use wsp_wire::views::workspace_more::WorkspaceOut;
use wsp_words::exit::{HostError, Kind};
use wsp_words::js::js_trim;
use wsp_words::projects::{copies_folder, kind_for_computer, no_branches_line, place_branch_line, pr_behind_words, runs_in_folder};
use wsp_words::start::start_words::no_project_for_repo;
use wsp_words::start::{
    holds_repo, is_review_read, line_in_diff, project_for_repo, review_from_reply, review_reask_prompt, start_name, taken_name_after,
    RepoProject,
};

use crate::projects::folders::tree::{git_said, ListedWorktree};
use crate::projects::projects::add::LandingKind;
use crate::projects::workspaces::seam::{record_of, Base, DaemonAsk, DaemonWork, Entry, GitAsk, HereWork};

/// `"start" | "review"`: what a made workspace is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartKind {
    Start,
    Review,
}

/// Where a start or a review runs. context.ts `Placed`.
#[derive(Clone)]
pub struct Placed {
    pub entry: Entry,
    pub behind: Option<String>,
    pub setup: Option<String>,
    pub r#as: Option<StartAs>,
}

/// The agent's lists for a start's picks, defaults applied: `ctx.defaultsOn(ctx.catalogOn(table, home, adapter), ...)`.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedPicks {
    pub catalog: HarnessCatalog,
    pub open_model: Option<String>,
    pub open_effort: Option<String>,
}

/// What `openWith` is told. start-from.ts:193.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpenAsk {
    pub prompt: String,
    pub harness: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: Option<String>,
    pub access: Option<AccessChoice>,
}

/// The session `sessionsApi.start` opened, as `handle.view()` reads it.
pub type OpenedSession = SessionView;

/// What the start area reads off the runtime context, past `Base`.
#[async_trait(?Send)]
pub trait StartFromCtx: Base {
    /// A TS `void f()`: `f` runs now, nothing awaits it.
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>);
    /// `ctx.places.wired`.
    fn places_wired(&self) -> String;
    /// `ctx.landingDeps(computer)`'s `placeId`, and whether a backend could be read there.
    async fn landing_place(&self, computer: &str) -> Result<Option<String>, HostError>;
    /// `ctx.landingKind(computer)`.
    fn landing_kind(&self, computer: &str) -> LandingKind;
    /// `brokeredStart(ctx, provider)` (branch-of-record.ts, wsp-machines').
    fn brokered_start(&self, provider: bool) -> bool;
    /// `pullRequestOfRecord(ctx, remote, number, project, computer, start)` (branch-of-record.ts).
    async fn pull_request_of_record(
        &self,
        remote: &str,
        number: i64,
        project: &str,
        computer: &str,
        start: bool,
    ) -> Result<Option<BranchOfRecord>, HostError>;
    /// `ctx.workspaces.create(opts, origin)`.
    async fn create_workspace(&self, opts: CreateWorkspaceOptions, origin: Option<Caller>) -> Result<CreatedWorkspace, HostError>;
    /// `ctx.workspaces.delete(id, origin)`.
    async fn delete_workspace(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError>;
    /// `ctx.projectFolder(project)`.
    async fn project_folder(&self, project: &ProjectView) -> Result<Entry, HostError>;
    /// `ctx.worktreeFolder(project, top, branch)`.
    async fn worktree_folder(&self, project: &ProjectView, top: &str, branch: &str) -> Result<Entry, HostError>;
    /// `ctx.worktreesOf(top)`.
    async fn worktrees_of(&self, top: &str) -> Result<Vec<ListedWorktree>, HostError>;
    /// `ctx.takeSetupLine(workspaceId)`.
    fn take_setup_line(&self, workspace_id: &str) -> Option<String>;
    /// `ctx.state.preferencesHeld`.
    fn preferences_held(&self) -> Option<Preferences>;
    /// `ctx.defaultAgentOf(prefs, entry)`.
    fn default_agent_of(&self, prefs: &Preferences, entry: &Entry) -> String;
    /// `harnessCatalog(harness)`, then `launchAdapterFor(home, harness)`, `catalogOn` and `defaultsOn`; none where the
    /// catalog has no table for the harness.
    async fn resolved_picks(
        &self,
        home: &Entry,
        harness: &str,
        prefs: &Preferences,
        project_id: &str,
    ) -> Result<Option<ResolvedPicks>, HostError>;
    /// `accessWordRefusal(catalog, access)`.
    fn access_word_refusal(&self, catalog: &HarnessCatalog, access: &AccessChoice) -> Option<HostError>;
    /// `ctx.namedMode(catalog, harness, access)`.
    fn named_mode(&self, catalog: &HarnessCatalog, harness: &str, access: &AccessChoice) -> Result<Option<String>, HostError>;
    /// `startPicks(catalog, picks, true)` throwing, as `notRunRefused(catalog, model) ?? pickRefusal(e, catalog)`.
    fn start_picks_refusal(&self, catalog: &HarnessCatalog, picks: &StartPicksAsked) -> Option<HostError>;
    /// `ctx.sessionsApi.start(workspaceId, opts, origin)`, answering `handle.view()`.
    async fn start_session(&self, workspace_id: &str, opts: serde_json::Value, origin: Option<Caller>) -> Result<OpenedSession, HostError>;
    /// `HARNESS_CATALOGS` as `(harness, readOnlyMode)`, in catalog order.
    fn read_only_modes(&self) -> Vec<(String, Option<String>)>;
    /// `ctx.sendDetached(workspaceId, {prompt, thread}, undefined)`.
    async fn send_detached(&self, workspace_id: &str, prompt: &str, thread: &str) -> Result<(), HostError>;
}

/// The start area.
pub struct StartFrom {
    pub(crate) ctx: Rc<dyn StartFromCtx>,
}

// export function startFromArea(ctx: RuntimeContext): StartFromArea {
pub fn start_from_area(ctx: Rc<dyn StartFromCtx>) -> Rc<StartFrom> {
    Rc::new(StartFrom { ctx })
}

impl wsp_core::context::StartFromArea for StartFrom {}

fn json_of<T: serde::Serialize>(value: &T) -> Result<Value, HostError> {
    serde_json::to_value(value).map_err(|e| HostError::new(e.to_string()))
}

fn read_json<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, HostError> {
    serde_json::from_value(value).map_err(|e| HostError::new(e.to_string()))
}

/// A project as `projectForRepo` reads it: its remote and the computer it is on.
struct RepoOf<'a>(&'a ProjectView);

impl RepoProject for RepoOf<'_> {
    fn remote(&self) -> &str {
        &self.0.remote
    }
    fn computer(&self) -> &str {
        &self.0.computer
    }
}

/// `{...fact, readAt}`, the pull request fact a held entry keeps.
fn seen_of(fact: &PullRequest, now: i64) -> Result<PullRequestSeen, HostError> {
    let mut value = json_of(fact)?;
    value["readAt"] = json!(now);
    read_json(value)
}

/// The pull request a record keeps: its number, url, state and base.
fn record_pr_of(fact: &PullRequest) -> Result<PullRequestRecord, HostError> {
    read_json(json!({ "number": fact.number, "url": fact.url, "state": fact.state, "base": fact.base }))
}

/// `isPullRequestFact(entry.pr) ? entry.pr.headOid`.
fn head_oid_of(pr: Option<&PullRequestSeen>) -> Option<String> {
    let value = json_of(pr?).ok()?;
    value.get("headOid")?.as_str().map(str::to_owned)
}

fn git_ask(cwd: &str, args: &[&str], timeout_ms: Option<u64>) -> GitAsk {
    GitAsk { cwd: cwd.to_owned(), args: args.iter().map(|arg| arg.to_string()).collect(), timeout_ms, input: None }
}

fn ref_of(branch: &str) -> String {
    format!("refs/heads/{branch}")
}

// ts: start-from.ts:39-48
//|   const fromOf = (kind: WorkspaceFrom["kind"], repo: string, read: IssueRead, fact: PullRequest | undefined): WorkspaceFrom => ({
//|     kind,
//|     repo,
//|     number: read.number,
//|     url: read.url,
//|     title: read.title,
//|     ...(fact !== undefined
//|       ? { base: fact.base, head: { branch: fact.branch, oid: fact.headOid, ...(fact.fork !== undefined ? { fork: fact.fork } : {}) } }
//|       : {}),
//|   });
/// Where the work came from, off the link's kind and what was read (spec rule 3).
pub fn from_of(kind: WorkspaceFromKind, repo: &str, read: &IssueRead, fact: Option<&PullRequest>) -> WorkspaceFrom {
    let head = fact.map(|fact| WorkspaceFromHead {
        branch: fact.branch.clone(),
        oid: Some(fact.head_oid.clone()),
        fork: fact.fork.as_ref().map(|fork| WorkspaceFromHeadFork { owner: fork.owner.clone(), pushable: fork.pushable }),
    });
    WorkspaceFrom {
        kind,
        repo: repo.to_owned(),
        number: read.number,
        url: read.url.clone(),
        title: read.title.clone(),
        base: fact.map(|fact| fact.base.clone()),
        head,
    }
}

impl StartFrom {
    // ts: start-from.ts:17-25
    //|   /** The project a GitHub repository names: the one whose remote is that repository, this computer's where two
    //|    * computers hold it, or the one named by name or id among them. A link is only ever matched to a project the
    //|    * person added, and every later call names the repository off that project's record. */
    //|   const projectByRepo = (repo: string, named: string | undefined): ProjectView => {
    //|     const held = [...projectsHeld.values()];
    //|     const picked = named === undefined ? projectForRepo(held, repo, HERE_PLACE_ID) : held.find(p => holdsRepo(p, repo) && (p.id === named || p.name === named));
    //|     if (picked === undefined) throw Object.assign(new Error(START_WORDS.noProjectForRepo(repo)), { kind: "invalid" });
    //|     return picked;
    //|   };
    /// The project a GitHub repository names (spec rule 1).
    pub fn project_by_repo(&self, repo: &str, named: Option<&str>) -> Result<ProjectView, HostError> {
        let held: Vec<ProjectView> = self.ctx.projects_held().borrow().values().cloned().collect();
        let picked = match named {
            None => {
                let repos: Vec<RepoOf<'_>> = held.iter().map(RepoOf).collect();
                project_for_repo(&repos, repo, HERE_PLACE_ID).map(|picked| picked.0.clone())
            }
            Some(named) => held.iter().find(|p| holds_repo(&p.remote, repo) && (p.id == named || p.name == named)).cloned(),
        };
        picked.ok_or_else(|| HostError::with_kind(no_project_for_repo(repo), Kind::Invalid))
    }

    // ts: start-from.ts:27-29
    //|   /** An issue, or a pull request read as the issue it also is, on this computer's own git host command line. */
    //|   const issueOf = async (remote: string, number: number): Promise<IssueRead> =>
    //|     GitIssueReadReply.parse(await ctx.onThisComputer((ask, home) => ask({ op: "git.issueRead", cwd: home, remote, number }))).issue;
    pub async fn issue_of(&self, remote: &str, number: i64) -> Result<IssueRead, HostError> {
        let remote = remote.to_owned();
        let work: HereWork = Box::new(move |ask: DaemonAsk, home: String| {
            ask(json!({ "op": "git.issueRead", "cwd": home, "remote": remote, "number": number }))
        });
        let reply = self.ctx.on_this_computer(work).await?;
        Ok(read_json::<GitIssueReadReply>(reply)?.issue)
    }

    // ts: start-from.ts:31-36
    //|   /** A pull request by number, read in full on this computer. */
    //|   const pullRequestOf = async (remote: string, number: number): Promise<PullRequest> => {
    //|     const read = GitPrReadReply.parse(await ctx.onThisComputer((ask, home) => ask({ op: "git.prRead", cwd: home, remote, number }))).pr;
    //|     if (read === undefined) throw new Error(`#${number} is not a pull request`);
    //|     return read;
    //|   };
    pub async fn pull_request_of(&self, remote: &str, number: i64) -> Result<PullRequest, HostError> {
        let remote = remote.to_owned();
        let work: HereWork = Box::new(move |ask: DaemonAsk, home: String| {
            ask(json!({ "op": "git.prRead", "cwd": home, "remote": remote, "number": number }))
        });
        let reply = self.ctx.on_this_computer(work).await?;
        read_json::<GitPrReadReply>(reply)?.pr.ok_or_else(|| HostError::new(format!("#{number} is not a pull request")))
    }

    // ts: start-from.ts:50-91
    //|   /** A workspace made for a start or a review: the copy made by the create road, where the work came from on its
    //|    * record, and for a pull request its base, its pull request and its head, the copy put on the head branch through
    //|    * its own daemon. A checkout that is refused takes the half-made workspace with it and answers its own sentence. */
    //|   const workspaceFrom = async (project: ProjectView, from: WorkspaceFrom, fact: PullRequest | undefined, kind: "start" | "review", origin: Caller | undefined): Promise<Placed> => {
    //|     const at = ctx.kindOf(project.computer);
    //|     if (copiesFolder(at)) return folderFrom(project, from, fact);
    //|     // A folder on a computer the person joined: an issue runs in the project folder, and a pull request's head waits
    //|     // on a worktree there, which wsp does not make yet.
    //|     if (runsInFolder(at)) {
    //|       if (fact !== undefined) throw Object.assign(new Error(placeBranchLine(ctx.placeName(project.computer))), { kind: "usage" });
    //|       return { entry: await ctx.projectFolder(project) };
    //|     }
    //|     const taken = new Set([...live.values()].map(e => e.record.name));
    //|     const name = takenNameAfter(startName(kind, from.number, from.title), taken);
    //|     // Where the host keeps the machine's credentials, a pull request's head is the one branch its pushes may land on,
    //|     // read before anything is made, so a fork that takes no maintainer's push is refused with nothing to take back.
    //|     const { at: landing, placeId } = await ctx.landingDeps(project.computer);
    //|     const brokered = brokeredStart(ctx, ctx.landingKind(project.computer, landing) === "provider");
    //|     const head = fact !== undefined && brokered ? await pullRequestOfRecord(ctx, project.remote, fact.number, project.name, ctx.placeName(placeId ?? ctx.places.wired), kind === "start") : undefined;
    //|     // A brokered clone runs under a token scoped to the thread this start opens, so that thread's id is picked first.
    //|     const as = brokered ? { threadId: randomUUID(), turnId: randomUUID() } : undefined;
    //|     const made = await ctx.workspaces.create({ project: project.id, name, ...(head !== undefined ? { branchOfRecord: head } : {}), ...(as !== undefined ? { opensThread: as.threadId } : {}) }, origin);
    //|     const entry = live.get(made.id);
    //|     if (entry === undefined) throw new Error(`${name} was made and then not found`);
    //|     entry.record.from = from;
    //|     if (fact !== undefined) {
    //|       entry.record.base = fact.base;
    //|       entry.record.pr = { number: fact.number, url: fact.url, state: fact.state, base: fact.base };
    //|       entry.pr = { ...fact, readAt: clock.now() };
    //|     }
    //|     await ctx.persist(entry.record);
    //|     // A brokered clone already stands on the pull request's head, fetched at depth 1 from the repo that holds it.
    //|     if (fact !== undefined && head === undefined) {
    //|       try {
    //|         await ctx.withDaemon(entry, ask => ask({ op: "git.prCheckout", cwd: ctx.checkoutOf(entry.record), number: fact.number }));
    //|       } catch (e) {
    //|         await ctx.workspaces.delete(entry.record.id, origin).catch((d: unknown) => console.warn(`${name} was not taken back after its checkout failed: ${d instanceof Error ? d.message : String(d)}`));
    //|         throw e;
    //|       }
    //|     }
    //|     return { entry, ...(as !== undefined ? { as } : {}) };
    //|   };
    /// A workspace made for a start or a review (spec rule 4).
    pub async fn workspace_from(
        &self,
        project: &ProjectView,
        from: WorkspaceFrom,
        fact: Option<PullRequest>,
        kind: StartKind,
        origin: Option<Caller>,
    ) -> Result<Placed, HostError> {
        let at = self.ctx.kind_of(&project.computer);
        if copies_folder(&at) {
            return self.folder_from(project, from, fact).await;
        }
        if runs_in_folder(&at) {
            if fact.is_some() {
                return Err(HostError::with_kind(place_branch_line(&self.ctx.place_name(&project.computer)), Kind::Usage));
            }
            let entry = self.ctx.project_folder(project).await?;
            return Ok(Placed { entry, behind: None, setup: None, r#as: None });
        }
        let live = self.ctx.live();
        let taken: IndexSet<String> = live.borrow().values().map(|entry| record_of(entry).name).collect();
        let kind_word = match kind {
            StartKind::Start => "start",
            StartKind::Review => "review",
        };
        let name = taken_name_after(&start_name(kind_word, from.number.get() as f64, &from.title), &taken);
        let landing_place = self.ctx.landing_place(&project.computer).await?;
        let brokered = self.ctx.brokered_start(self.ctx.landing_kind(&project.computer) == LandingKind::Provider);
        let head = match &fact {
            Some(fact) if brokered => {
                let place = landing_place.unwrap_or_else(|| self.ctx.places_wired());
                self.ctx
                    .pull_request_of_record(
                        &project.remote,
                        fact.number.get(),
                        &project.name,
                        &self.ctx.place_name(&place),
                        kind == StartKind::Start,
                    )
                    .await?
            }
            _ => None,
        };
        let has_head = head.is_some();
        let as_ = if brokered {
            let thread_id = self.ctx.ids().uuid();
            let turn_id = self.ctx.ids().uuid();
            Some(StartAs { thread_id, turn_id })
        } else {
            None
        };
        let opts = CreateWorkspaceOptions {
            project: project.id.clone(),
            name: name.clone(),
            branch_of_record: head,
            opens_thread: as_.as_ref().map(|started| started.thread_id.clone()),
            ..Default::default()
        };
        let made = self.ctx.create_workspace(opts, origin.clone()).await?;
        let found = self.ctx.live().borrow().get(&made.view.id).cloned();
        let Some(entry) = found else {
            return Err(HostError::new(format!("{name} was made and then not found")));
        };
        let pr_record = match &fact {
            Some(fact) => Some(record_pr_of(fact)?),
            None => None,
        };
        {
            let held = entry.borrow();
            let mut record = held.record.borrow_mut();
            record.from = Some(from);
            if let (Some(fact), Some(pr)) = (&fact, pr_record) {
                record.base = Some(fact.base.clone());
                record.pr = Some(pr);
            }
        }
        if let Some(fact) = &fact {
            entry.borrow_mut().pr = Some(seen_of(fact, self.ctx.clock().now())?);
        }
        self.ctx.persist(record_of(&entry)).await?;
        if let Some(fact) = &fact {
            if !has_head {
                let number = fact.number.get();
                let ctx = self.ctx.clone();
                let record = record_of(&entry);
                let work: DaemonWork = Box::new(move |ask: DaemonAsk| {
                    async move {
                        let cwd = ctx.checkout_of(&record)?;
                        ask(json!({ "op": "git.prCheckout", "cwd": cwd, "number": number })).await
                    }
                    .boxed_local()
                });
                if let Err(e) = self.ctx.with_daemon(&entry, work).await {
                    let id = record_of(&entry).id;
                    if let Err(d) = self.ctx.delete_workspace(&id, origin.clone()).await {
                        warn(&format!("{name} was not taken back after its checkout failed: {}", d.message));
                    }
                    return Err(e);
                }
            }
        }
        Ok(Placed { entry, behind: None, setup: None, r#as: as_ })
    }

    // ts: start-from.ts:93-116
    //|   /** Where a start or a review on this computer runs: an issue in the project folder; a pull request in a worktree on
    //|    * its head, fetched into the project's repo first as a branch of its own where none of that name stands, so
    //|    * nothing moves the project folder's checkout. The pull request's facts go on the worktree's record alone. */
    //|   const folderFrom = async (project: ProjectView, from: WorkspaceFrom, fact: PullRequest | undefined): Promise<Placed> => {
    //|     if (fact === undefined) return { entry: await ctx.projectFolder(project) };
    //|     const top = project.git?.top;
    //|     if (top === undefined) throw Object.assign(new Error(noBranchesLine(project.name)), { kind: "usage" });
    //|     const branch = fact.fork === undefined ? fact.branch : `${fact.fork.owner}/${fact.branch}`;
    //|     // A branch some worktree already holds runs there and is caught up below; any other is fetched first, made where
    //|     // it is not here and only moved forward where it is, never forced.
    //|     const holds = (await ctx.worktreesOf(top)).some(w => w.branch === branch);
    //|     if (!holds) await fetchHead(project, top, fact, branch);
    //|     const entry = await ctx.worktreeFolder(project, top, branch);
    //|     const behind = holds ? await catchUp(entry, project, top, fact) : undefined;
    //|     const setup = ctx.takeSetupLine(entry.record.id);
    //|     const placed = { entry, ...(behind !== undefined ? { behind } : {}), ...(setup !== undefined ? { setup } : {}) };
    //|     if (entry.record.worktree === undefined) return placed;
    //|     entry.record.from = from;
    //|     entry.record.base = fact.base;
    //|     entry.record.pr = { number: fact.number, url: fact.url, state: fact.state, base: fact.base };
    //|     entry.pr = { ...fact, readAt: clock.now() };
    //|     await ctx.persist(entry.record);
    //|     return placed;
    //|   };
    /// Where a start or a review on this computer runs (spec rule 5).
    pub async fn folder_from(&self, project: &ProjectView, from: WorkspaceFrom, fact: Option<PullRequest>) -> Result<Placed, HostError> {
        let Some(fact) = fact else {
            let entry = self.ctx.project_folder(project).await?;
            return Ok(Placed { entry, behind: None, setup: None, r#as: None });
        };
        let Some(top) = project.git.as_ref().map(|git| git.top.clone()) else {
            return Err(HostError::with_kind(no_branches_line(&project.name), Kind::Usage));
        };
        let branch = match &fact.fork {
            None => fact.branch.clone(),
            Some(fork) => format!("{}/{}", fork.owner, fact.branch),
        };
        let holds = self.ctx.worktrees_of(&top).await?.iter().any(|tree| tree.branch.as_deref() == Some(branch.as_str()));
        if !holds {
            self.fetch_head(project, &top, &fact, &branch).await?;
        }
        let entry = self.ctx.worktree_folder(project, &top, &branch).await?;
        let behind = if holds { self.catch_up(&entry, project, &top, &fact).await? } else { None };
        let setup = self.ctx.take_setup_line(&record_of(&entry).id);
        let placed = Placed { entry: entry.clone(), behind, setup, r#as: None };
        if record_of(&entry).worktree.is_none() {
            return Ok(placed);
        }
        let pr_record = record_pr_of(&fact)?;
        {
            let held = entry.borrow();
            let mut record = held.record.borrow_mut();
            record.from = Some(from);
            record.base = Some(fact.base.clone());
            record.pr = Some(pr_record);
        }
        entry.borrow_mut().pr = Some(seen_of(&fact, self.ctx.clock().now())?);
        self.ctx.persist(record_of(&entry)).await?;
        Ok(placed)
    }

    // ts: start-from.ts:118-140
    //|   /** A start's picks read against the agent's lists before a folder is made or moved for it, by the rule the start
    //|    * itself refuses them with, so a pick the agent does not take costs no git work. */
    //|   const picksHold = async (project: ProjectView, o: StartPicksAsked): Promise<void> => {
    //|     if (!copiesFolder(kindForComputer(project.computer))) return;
    //|     const home = await ctx.projectFolder(project);
    //|     const prefs = ctx.state.preferencesHeld ?? (await ctx.preferences.get());
    //|     const harness = o.harness ?? ctx.defaultAgentOf(prefs, home);
    //|     const table = harnessCatalog(harness);
    //|     if (table === undefined) return;
    //|     const { adapter } = await ctx.launchAdapterFor(home, harness);
    //|     const resolved = ctx.defaultsOn(await ctx.catalogOn(table, home, adapter), prefs, prefs.projectDefaults[home.record.project]);
    //|     // This road is the command line's and the tools', so it names the flag to drop; namedMode speaks for the app too.
    //|     const refused = o.access === undefined ? null : accessWordRefusal(resolved.catalog, o.access);
    //|     if (refused !== null) throw refused;
    //|     const named = o.permissionMode ?? (o.access === undefined ? undefined : ctx.namedMode(resolved.catalog, harness, o.access));
    //|     const model = o.model ?? resolved.open.model;
    //|     const effort = o.effort ?? resolved.open.effort;
    //|     try {
    //|       startPicks(resolved.catalog, { ...(model !== undefined ? { model } : {}), ...(effort !== undefined ? { effort } : {}), ...(named !== undefined ? { permissionMode: named } : {}), ...(o.fast === true ? { fast: true } : {}) }, true);
    //|     } catch (e) {
    //|       throw notRunRefused(resolved.catalog, model) ?? pickRefusal(e, resolved.catalog);
    //|     }
    //|   };
    /// A start's picks read against the agent's lists before a folder is made or moved for it (spec rule 9).
    pub async fn picks_hold(&self, project: &ProjectView, o: &StartPicksAsked) -> Result<(), HostError> {
        if !copies_folder(&kind_for_computer(&project.computer, None)) {
            return Ok(());
        }
        let home = self.ctx.project_folder(project).await?;
        let prefs = match self.ctx.preferences_held() {
            Some(prefs) => prefs,
            None => self.ctx.preferences().await?,
        };
        let harness = match &o.harness {
            Some(harness) => harness.clone(),
            None => self.ctx.default_agent_of(&prefs, &home),
        };
        let project_id = record_of(&home).project;
        let Some(resolved) = self.ctx.resolved_picks(&home, &harness, &prefs, &project_id).await? else {
            return Ok(());
        };
        if let Some(access) = &o.access {
            if let Some(refused) = self.ctx.access_word_refusal(&resolved.catalog, access) {
                return Err(refused);
            }
        }
        let named = match (&o.permission_mode, &o.access) {
            (Some(mode), _) => Some(mode.clone()),
            (None, Some(access)) => self.ctx.named_mode(&resolved.catalog, &harness, access)?,
            (None, None) => None,
        };
        let picks = StartPicksAsked {
            model: o.model.clone().or(resolved.open_model.clone()),
            effort: o.effort.clone().or(resolved.open_effort.clone()),
            permission_mode: named,
            fast: if o.fast == Some(true) { Some(true) } else { None },
            ..Default::default()
        };
        match self.ctx.start_picks_refusal(&resolved.catalog, &picks) {
            Some(refused) => Err(refused),
            None => Ok(()),
        }
    }

    // ts: start-from.ts:142-147
    //|   /** The project folder's record with this computer's daemon told the folder is its to work in, so an ask naming a
    //|    * project anywhere a project may live resolves. */
    //|   const askingIn = async (entry: LiveWorkspace): Promise<LiveWorkspace> => {
    //|     await ctx.writeDaemonRoots(entry);
    //|     return entry;
    //|   };
    pub async fn asking_in(&self, entry: Entry) -> Result<Entry, HostError> {
        self.ctx.write_daemon_roots(&entry, false).await?;
        Ok(entry)
    }

    // ts: start-from.ts:148-163
    //|   /** A pull request's head into its local branch through the daemon: the branch by name where the remote still holds
    //|    * it, else the pull request's own head, which outlives a branch deleted at the merge. GitHub serves a fork's head
    //|    * on the base repository as pull/<n>/head, so no fork's URL is dialled. */
    //|   const fetchHead = async (project: ProjectView, top: string, fact: PullRequest, branch: string): Promise<void> => {
    //|     if (fact.fork === undefined) {
    //|       const home = await askingIn(await ctx.projectFolder(project));
    //|       try {
    //|         GitStartOnReply.parse(await ctx.withDaemon(home, ask => ask({ op: "git.fetchBranch", cwd: project.path, remote: project.remote, branch: fact.branch, into: branch })));
    //|         return;
    //|       } catch (e) {
    //|         if (await remoteHolds(top, project.remote, `refs/heads/${fact.branch}`)) throw e;
    //|       }
    //|     }
    //|     const fetched = await ctx.gitHere(top, ["fetch", "--quiet", "--no-tags", project.remote, `pull/${fact.number}/head:refs/heads/${branch}`], CLONE_MS);
    //|     if (fetched.exitCode !== 0) throw new Error(ctx.gitSaid(fetched));
    //|   };
    /// A pull request's head into its local branch (spec rule 6).
    pub async fn fetch_head(&self, project: &ProjectView, top: &str, fact: &PullRequest, branch: &str) -> Result<(), HostError> {
        if fact.fork.is_none() {
            let home = self.asking_in(self.ctx.project_folder(project).await?).await?;
            let path = project.path.clone();
            let remote = project.remote.clone();
            let from_branch = fact.branch.clone();
            let into = branch.to_owned();
            let work: DaemonWork = Box::new(move |ask: DaemonAsk| {
                ask(json!({ "op": "git.fetchBranch", "cwd": path, "remote": remote, "branch": from_branch, "into": into }))
            });
            let fetched = self.ctx.with_daemon(&home, work).await.and_then(|reply| read_json::<GitStartOnReply>(reply).map(|_| ()));
            if let Err(e) = fetched {
                if self.remote_holds(top, &project.remote, &ref_of(&fact.branch)).await? {
                    return Err(e);
                }
            } else {
                return Ok(());
            }
        }
        let refspec = format!("pull/{}/head:refs/heads/{branch}", fact.number.get());
        let fetched = self
            .ctx
            .git_here(git_ask(top, &["fetch", "--quiet", "--no-tags", project.remote.as_str(), refspec.as_str()], Some(CLONE_MS)))
            .await?;
        if fetched.exit_code.get() != 0 {
            return Err(HostError::new(git_said(&fetched)));
        }
        Ok(())
    }

    // ts: start-from.ts:164-166
    //|   /** Whether the remote holds a ref; exit 2 alone is its answer that it does not. */
    //|   const remoteHolds = async (top: string, remote: string, ref: string): Promise<boolean> =>
    //|     (await ctx.gitHere(top, ["ls-remote", "--exit-code", remote, ref], CLONE_MS)).exitCode !== 2;
    /// Whether the remote holds a ref; exit 2 alone is its answer that it does not.
    pub async fn remote_holds(&self, top: &str, remote: &str, r#ref: &str) -> Result<bool, HostError> {
        let read = self.ctx.git_here(git_ask(top, &["ls-remote", "--exit-code", remote, r#ref], Some(CLONE_MS))).await?;
        Ok(read.exit_code.get() != 2)
    }

    // ts: start-from.ts:167-190
    //|   /** The folder holding a pull request's branch, caught up where it may be: a worktree wsp made that is clean and
    //|    * only behind moves forward through the daemon; the project folder never moves, and any folder left as it stands
    //|    * gets the line its thread is told. */
    //|   const catchUp = async (entry: LiveWorkspace, project: ProjectView, top: string, fact: PullRequest): Promise<string | undefined> => {
    //|     const path = entry.record.worktree?.path ?? top;
    //|     const byName = fact.fork === undefined && (await remoteHolds(top, project.remote, `refs/heads/${fact.branch}`));
    //|     const spec = byName ? `refs/heads/${fact.branch}` : `pull/${fact.number}/head`;
    //|     const fetched = await ctx.gitHere(top, ["fetch", "--quiet", "--no-tags", project.remote, spec], CLONE_MS);
    //|     if (fetched.exitCode !== 0) return PR_BEHIND_WORDS.unread(fact.number, ctx.gitSaid(fetched));
    //|     const head = (await ctx.gitHere(top, ["rev-parse", "FETCH_HEAD"])).stdout.trim();
    //|     const at = (await ctx.gitHere(path, ["rev-parse", "HEAD"])).stdout.trim();
    //|     if (head === at || (await ctx.gitHere(path, ["merge-base", "--is-ancestor", head, at])).exitCode === 0) return undefined;
    //|     if ((await ctx.gitHere(path, ["merge-base", "--is-ancestor", at, head])).exitCode !== 0) return PR_BEHIND_WORDS.diverged(fact.number, path);
    //|     if ((await ctx.gitHere(path, ["status", "--porcelain"])).stdout.trim() !== "") return PR_BEHIND_WORDS.changed(fact.number, path);
    //|     if (entry.record.worktree === undefined) return PR_BEHIND_WORDS.folder(fact.number, path);
    //|     if (entry.record.worktree.made !== true) return PR_BEHIND_WORDS.notMade(fact.number, path);
    //|     if (!byName) return PR_BEHIND_WORDS.byHead(fact.number, path, project.remote);
    //|     try {
    //|       GitUpdateReply.parse(await ctx.withDaemon(await askingIn(entry), ask => ask({ op: "git.update", cwd: ctx.checkoutOf(entry.record), base: fact.branch })));
    //|       return undefined;
    //|     } catch (e) {
    //|       return PR_BEHIND_WORDS.unread(fact.number, e instanceof Error ? e.message : String(e));
    //|     }
    //|   };
    /// The folder holding a pull request's branch, caught up where it may be; the line its thread is told (rule 8).
    pub async fn catch_up(&self, entry: &Entry, project: &ProjectView, top: &str, fact: &PullRequest) -> Result<Option<String>, HostError> {
        let record = record_of(entry);
        let path = record.worktree.as_ref().map(|tree| tree.path.clone()).unwrap_or_else(|| top.to_owned());
        let number = fact.number.get() as f64;
        let by_name = fact.fork.is_none() && self.remote_holds(top, &project.remote, &ref_of(&fact.branch)).await?;
        let spec = if by_name { ref_of(&fact.branch) } else { format!("pull/{}/head", fact.number.get()) };
        let fetched = self
            .ctx
            .git_here(git_ask(top, &["fetch", "--quiet", "--no-tags", project.remote.as_str(), spec.as_str()], Some(CLONE_MS)))
            .await?;
        if fetched.exit_code.get() != 0 {
            return Ok(Some(pr_behind_words::unread(number, &git_said(&fetched))));
        }
        let head_read = self.ctx.git_here(git_ask(top, &["rev-parse", "FETCH_HEAD"], None)).await?;
        let head = js_trim(&head_read.stdout).to_owned();
        let at_read = self.ctx.git_here(git_ask(&path, &["rev-parse", "HEAD"], None)).await?;
        let at = js_trim(&at_read.stdout).to_owned();
        if head == at {
            return Ok(None);
        }
        let ahead = self.ctx.git_here(git_ask(&path, &["merge-base", "--is-ancestor", head.as_str(), at.as_str()], None)).await?;
        if ahead.exit_code.get() == 0 {
            return Ok(None);
        }
        let behind = self.ctx.git_here(git_ask(&path, &["merge-base", "--is-ancestor", at.as_str(), head.as_str()], None)).await?;
        if behind.exit_code.get() != 0 {
            return Ok(Some(pr_behind_words::diverged(number, &path)));
        }
        let status = self.ctx.git_here(git_ask(&path, &["status", "--porcelain"], None)).await?;
        if !js_trim(&status.stdout).is_empty() {
            return Ok(Some(pr_behind_words::changed(number, &path)));
        }
        let Some(tree) = record.worktree.clone() else {
            return Ok(Some(pr_behind_words::folder(number, &path)));
        };
        if !tree.made {
            return Ok(Some(pr_behind_words::not_made(number, &path)));
        }
        if !by_name {
            return Ok(Some(pr_behind_words::by_head(number, &path, &project.remote)));
        }
        let branch = fact.branch.clone();
        let ctx = self.ctx.clone();
        let work: DaemonWork = Box::new(move |ask: DaemonAsk| {
            async move {
                let cwd = ctx.checkout_of(&record)?;
                ask(json!({ "op": "git.update", "cwd": cwd, "base": branch })).await
            }
            .boxed_local()
        });
        let updated = match self.asking_in(entry.clone()).await {
            Ok(home) => self.ctx.with_daemon(&home, work).await,
            Err(e) => Err(e),
        };
        match updated.and_then(|reply| read_json::<GitUpdateReply>(reply).map(|_| ())) {
            Ok(()) => Ok(None),
            Err(e) => Ok(Some(pr_behind_words::unread(number, &e.message))),
        }
    }

    // ts: start-from.ts:192-211
    //|   /** The thread a start or a review opens, detached: the turn goes on without the caller. */
    //|   const openWith = async ({ entry, behind, setup, as }: Placed, o: { prompt: string; harness?: string; model?: string; effort?: string; permissionMode?: string; access?: AccessChoice }, origin: Caller | undefined): Promise<StartResult> => {
    //|     const handle = await ctx.sessionsApi.start(
    //|       entry.record.id,
    //|       {
    //|         ...(behind !== undefined ? { behind } : {}),
    //|         ...(setup !== undefined ? { setup } : {}),
    //|         ...(as !== undefined ? { as } : {}),
    //|         prompt: o.prompt,
    //|         ...(o.harness !== undefined ? { harness: o.harness } : {}),
    //|         ...(o.model !== undefined ? { model: o.model } : {}),
    //|         ...(o.effort !== undefined ? { effort: o.effort } : {}),
    //|         ...(o.permissionMode !== undefined ? { permissionMode: o.permissionMode } : {}),
    //|         ...(o.access !== undefined ? { access: o.access } : {}),
    //|       },
    //|       origin,
    //|     );
    //|     const v = handle.view();
    //|     return { workspace: ctx.view(entry.record), threadId: v.threadId ?? v.id, sessionId: v.id };
    //|   };
    /// The thread a start or a review opens, detached (spec rule 10).
    pub async fn open_with(&self, placed: &Placed, o: OpenAsk, origin: Option<Caller>) -> Result<StartResult, HostError> {
        let mut opts = serde_json::Map::new();
        if let Some(behind) = &placed.behind {
            opts.insert("behind".to_owned(), json!(behind));
        }
        if let Some(setup) = &placed.setup {
            opts.insert("setup".to_owned(), json!(setup));
        }
        if let Some(started) = &placed.r#as {
            opts.insert("as".to_owned(), json!({ "threadId": started.thread_id, "turnId": started.turn_id }));
        }
        opts.insert("prompt".to_owned(), json!(o.prompt));
        if let Some(harness) = &o.harness {
            opts.insert("harness".to_owned(), json!(harness));
        }
        if let Some(model) = &o.model {
            opts.insert("model".to_owned(), json!(model));
        }
        if let Some(effort) = &o.effort {
            opts.insert("effort".to_owned(), json!(effort));
        }
        if let Some(mode) = &o.permission_mode {
            opts.insert("permissionMode".to_owned(), json!(mode));
        }
        if let Some(access) = &o.access {
            opts.insert("access".to_owned(), json_of(access)?);
        }
        let id = record_of(&placed.entry).id;
        let view = self.ctx.start_session(&id, Value::Object(opts), origin).await?;
        let thread_id = view.thread_id.clone().unwrap_or_else(|| view.id.clone());
        let workspace: WorkspaceOut = read_json(json_of(&self.ctx.view(&record_of(&placed.entry))?)?)?;
        Ok(StartResult { workspace, thread_id, session_id: view.id.clone() })
    }

    // ts: start-from.ts:213-214
    //|   /** The harnesses whose table names a mode that changes nothing, which are the ones that can review. */
    //|   const readOnlyOf = (agent: string): string | undefined => HARNESS_CATALOGS.find(c => c.harness === agent)?.readOnlyMode;
    pub fn read_only_of(&self, agent: &str) -> Option<String> {
        self.ctx.read_only_modes().into_iter().find(|(harness, _)| harness == agent).and_then(|(_, mode)| mode)
    }

    // ts: start-from.ts:215-219
    //|   const reviewers = (): string[] => {
    //|     const all = HARNESS_CATALOGS.filter(c => c.readOnlyMode !== undefined).map(c => c.harness);
    //|     // Codex first: the owner's rule since 2026-09-26 is that Codex runs reviews.
    //|     return [...all.filter(h => h === "codex"), ...all.filter(h => h !== "codex")];
    //|   };
    /// The harnesses that can review, Codex first.
    pub fn reviewers(&self) -> Vec<String> {
        let all: Vec<String> =
            self.ctx.read_only_modes().into_iter().filter(|(_, mode)| mode.is_some()).map(|(harness, _)| harness).collect();
        let codex = all.iter().filter(|harness| harness.as_str() == "codex").cloned();
        let rest = all.iter().filter(|harness| harness.as_str() != "codex").cloned();
        codex.chain(rest).collect()
    }

    // ts: start-from.ts:221-257
    //|   /** A reviewer's reply read at its turn's end: the review its last fenced json block writes becomes the draft, each
    //|    * comment on a line outside the diff marked for the summary; a reply with no block that reads is asked once more
    //|    * in the same thread, and a second that does not read leaves the sentence. A reply with no block after a draft that
    //|    * read leaves that draft as it was: the person may have asked the reviewer something else. */
    //|   const takeReview = async (entry: LiveWorkspace, threadId: string, text: string): Promise<void> => {
    //|     const from = entry.record.from;
    //|     if (from?.kind !== "review") return;
    //|     const reviewer = entry.record.reviewThread;
    //|     if (reviewer !== undefined && reviewer !== threadId) return;
    //|     const read = reviewFromReply(text);
    //|     const prior = entry.record.review;
    //|     const at = clock.now();
    //|     if (!read.ok) {
    //|       if (isReviewRead(prior)) return;
    //|       const reasked = prior !== undefined && "note" in prior && prior.reasked === true;
    //|       entry.record.review = { note: read.why, at, reasked: true };
    //|       await ctx.persist(entry.record);
    //|       bus.emit({ type: "workspace.review", workspaceId: entry.record.id });
    //|       if (!reasked) void ctx.sendDetached(entry.record.id, { prompt: reviewReaskPrompt(read.why), thread: threadId }, undefined).catch((e: unknown) => console.warn(`the reviewer of ${entry.record.name} was not asked again: ${e instanceof Error ? e.message : String(e)}`));
    //|       return;
    //|     }
    //|     const remote = ctx.projectHeld(entry.record.project).remote;
    //|     const diff = await ctx.onThisComputer((ask, home) => ask({ op: "git.prDiff", cwd: home, remote, number: from.number })).then(
    //|       r => GitPrDiffReply.parse(r).diff,
    //|       () => undefined,
    //|     );
    //|     const comments = read.review.comments.map((c, n) => ({
    //|       id: `c${n + 1}`,
    //|       ...c,
    //|       on: true,
    //|       ...(diff !== undefined && lineInDiff(diff, c.path, c.line, c.side) === false ? { inSummary: true } : {}),
    //|     }));
    //|     const headOid = (isPullRequestFact(entry.pr) ? entry.pr.headOid : undefined) ?? from.head?.oid ?? "";
    //|     entry.record.review = { verdict: read.review.verdict, summary: read.review.summary, comments, headOid, threadId, at };
    //|     await ctx.persist(entry.record);
    //|     bus.emit({ type: "workspace.review", workspaceId: entry.record.id });
    //|   };
    /// A reviewer's reply read at its turn's end (spec rule 12).
    pub async fn take_review(&self, entry: &Entry, thread_id: &str, text: &str) -> Result<(), HostError> {
        let record = record_of(entry);
        let Some(from) = record.from.clone() else {
            return Ok(());
        };
        if from.kind != WorkspaceFromKind::Review {
            return Ok(());
        }
        if record.review_thread.as_deref().is_some_and(|reviewer| reviewer != thread_id) {
            return Ok(());
        }
        let now = self.ctx.clock().now();
        let block = match review_from_reply(text) {
            Ok(block) => block,
            Err(why) => {
                let prior = record.review.clone();
                if is_review_read(prior.as_ref()) {
                    return Ok(());
                }
                let reasked = matches!(&prior, Some(ReviewDraft::V1(note)) if note.reasked == Some(true));
                let note = ReviewDraft::V1(ReviewDraftV1 { note: why.clone(), at: JsNumber::from(now), reasked: Some(true) });
                entry.borrow().record.borrow_mut().review = Some(note);
                self.ctx.persist(record_of(entry)).await?;
                self.ctx.emit(json!({ "type": "workspace.review", "workspaceId": record.id }))?;
                if !reasked {
                    let ctx = self.ctx.clone();
                    let id = record.id.clone();
                    let name = record.name.clone();
                    let prompt = review_reask_prompt(&why);
                    let thread = thread_id.to_owned();
                    self.ctx.spawn(Box::pin(async move {
                        if let Err(e) = ctx.send_detached(&id, &prompt, &thread).await {
                            warn(&format!("the reviewer of {name} was not asked again: {}", e.message));
                        }
                    }));
                }
                return Ok(());
            }
        };
        let remote = self.ctx.project_held(&record.project)?.remote;
        let number = from.number.get();
        let work: HereWork = Box::new(move |ask: DaemonAsk, home: String| {
            ask(json!({ "op": "git.prDiff", "cwd": home, "remote": remote, "number": number }))
        });
        let diff = match self.ctx.on_this_computer(work).await {
            Ok(reply) => Some(read_json::<GitPrDiffReply>(reply)?.diff),
            Err(_) => None,
        };
        let mut comments: Vec<Value> = Vec::new();
        for (n, comment) in block.comments.iter().enumerate() {
            let side = json_of(&comment.side)?;
            let side_word = side.as_str().unwrap_or_default();
            let mut item = json!({
                "id": format!("c{}", n + 1),
                "path": comment.path,
                "line": comment.line,
                "side": side,
                "body": comment.body,
                "on": true,
            });
            if let Some(diff) = &diff {
                if line_in_diff(diff, &comment.path, comment.line.get() as f64, side_word) == Some(false) {
                    item["inSummary"] = json!(true);
                }
            }
            comments.push(item);
        }
        let held_head = head_oid_of(entry.borrow().pr.as_ref());
        let head_oid = held_head.or_else(|| from.head.as_ref().and_then(|head| head.oid.clone())).unwrap_or_default();
        let review = read_json::<ReviewDraft>(json!({
            "verdict": json_of(&block.verdict)?,
            "summary": block.summary,
            "comments": comments,
            "headOid": head_oid,
            "threadId": thread_id,
            "at": now,
        }))?;
        entry.borrow().record.borrow_mut().review = Some(review);
        self.ctx.persist(record_of(entry)).await?;
        self.ctx.emit(json!({ "type": "workspace.review", "workspaceId": record.id }))?;
        Ok(())
    }
}
