// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/account/slates.ts at 65f6b89c7. Spec: spec/account_slates.md.
//!
//! The slate area's wiring: what `createSlates` (slates-a's) is handed, each answer read off the runtime per call.
//! Everything it reads goes through `SlatesAreaCtx` (ruling 6401022701 rule 4), implemented on wsp-core's context
//! handle at the join and by a fake in the tests.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{join_all, select, Either, LocalBoxFuture};
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::Value;

use wsp_core::clock::Clock;
use wsp_core::ids::OsIds;
use wsp_core::types::internal::ThreadRecord;
use wsp_core::types::wiring::{Detach, WorkspaceRecord};
use wsp_engine::machine::Machine;
use wsp_engine::place_machine::PlaceFolderMachine;
use wsp_engine::target_line::TargetLogin;
use wsp_frames::ProcKind;
use wsp_store::store::Store;
use wsp_wire::types::agents_report::{AgentsProject, AgentsTarget, AgentsTargetV0, AgentsTargetV1};
use wsp_wire::types::env::THREAD_ENV;
use wsp_wire::types::slate_wire::SessionSlateEvent;
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_wire::wire::daemon::{FsHashReply, FsImageReply};
use wsp_wire::wire::daemon_version::HTTP_POST_DAEMON_VERSION;
use wsp_words::exit::HostError;
use wsp_words::projects::folder_on_joined;
use wsp_words::words::computer::thread_word;
use wsp_words::workspace_state::is_local_workspace;

pub use wsp_machines::machines::machines::Entry;

use crate::slate_files::{HashOn, HashRead};
use crate::slate_images::{ImageOn, ImageRefusal};
use crate::slate_mcp::{Away, McpPost, McpPostRoad, McpServerSpec, MissingOn, MissingOnFn, ToolAsks};
use crate::slate_mcp_on::{box_slate_dir, pipe_on, post_here, post_on, StdioRoad, StdioRoadDial};
use crate::slate_sources::context::{AccountRow, SessionView, SlateWorkspaceFacts, TurnResult};
use crate::slate_sources::SlateSourceContext;
use crate::slates::{Delivered, Delivery, DeliveryOutcome, SlateEmit, SlateThreadFacts, SlatesDeps};

/// A server as an agent's config there gives it, with the login its lines run as where it was read on a box.
#[derive(Clone)]
pub struct ReadServer {
    pub spec: McpServerSpec,
    pub login: Option<TargetLogin>,
}

/// Why a server read failed: not set up there, off there, or anything else (which refuses no write).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerReadError {
    NotSetUp(String),
    Off(String),
    Other(String),
}

/// What the area reads off the runtime: the members of `RuntimeContext` that `slatesArea` touches.
pub trait SlatesAreaCtx {
    fn store(&self) -> Rc<dyn Store>;
    fn clock(&self) -> Rc<dyn Clock>;
    /// `ctx.record`: puts a slate's event in its thread's transcript.
    fn record(&self, event: SessionSlateEvent);
    /// `bus.emit` of a slate's live event.
    fn emit(&self, event: SlateEmit);

    fn latest_on(&self, thread: &str) -> Option<SessionView>;
    fn thread_record(&self, thread: &str) -> Option<ThreadRecord>;
    /// The `turnId` of a running session of this thread, the first in the sessions map.
    fn running_turn(&self, thread: &str) -> Option<String>;
    fn live(&self, workspace: &str) -> Option<Entry>;
    fn box_of(&self, thread: &str) -> Option<Entry>;
    /// The joined computer's folder machine the thread's box is, where it is one (`machine instanceof PlaceFolderMachine`).
    fn place_of(&self, thread: &str) -> Option<Arc<PlaceFolderMachine>>;
    fn folder_of(&self, workspace: &str, session: &str) -> Option<String>;
    fn runs_in(&self, entry: &Entry, ran_in: Option<&str>, fallback: &str) -> String;
    /// The kind module's `folder(record)`.
    fn kind_folder(&self, entry: &Entry) -> Option<String>;
    /// `ctx.checkoutOf(record)`, which throws where the record's project is gone.
    fn checkout_of(&self, entry: &Entry) -> Result<String, HostError>;
    fn path_exists(&self, path: &str) -> bool;
    fn home_dir(&self) -> String;
    fn computer_of(&self, entry: &Entry) -> String;
    fn root_of(&self, thread: &str) -> String;
    fn tool_asks(&self, harness: Option<&str>, mode: Option<&str>) -> ToolAsks;

    /// The backend's hourly rate for the record's size; none without a size.
    fn entry_rate_usd_per_hour(&self, entry: &Entry) -> Option<f64>;
    /// `atProvider(place, ctx.places)`.
    fn at_provider(&self, place: &str) -> bool;

    fn boot_work(&self, workspace: &str) -> LocalBoxFuture<'static, ()>;
    fn wake(&self, workspace: &str) -> LocalBoxFuture<'static, Result<(), HostError>>;
    fn ready(&self) -> LocalBoxFuture<'static, Result<(), HostError>>;
    /// `ctx.settledNow`: whether the thread is settled by the sidebar's rule at this window (`None` is never).
    fn settled_now(&self, thread: &str, ms: Option<f64>) -> bool;
    /// The settle preference's key: `15m`, `1h`, `2h`, `1d` or `never`.
    fn settle_after(&self) -> LocalBoxFuture<'static, String>;
    fn tree_under(&self, lead: &str) -> Vec<String>;
    fn thread_of_token(&self, token: &str) -> Result<String, HostError>;
    fn own_process(&self, thread: &str, pid: u32, kind: ProcKind) -> Detach;
    fn daemon_image(&self, entry: &Entry, path: String) -> LocalBoxFuture<'static, Result<Value, ImageRefusal>>;
    fn daemon_hash(&self, entry: &Entry, root: String, paths: Vec<String>) -> LocalBoxFuture<'static, Result<Value, String>>;
    fn agents_server(&self, target: AgentsTarget, agent: &str, name: &str) -> LocalBoxFuture<'static, Result<ReadServer, ServerReadError>>;
    /// This computer's agents reader; `None` when the host reads no agent's MCP config.
    fn agents_reader_server(
        &self,
        projects: Vec<AgentsProject>,
        agent: &str,
        name: &str,
    ) -> Option<LocalBoxFuture<'static, Result<McpServerSpec, HostError>>>;
    fn stdio_road(&self, entry: &Entry, min_version: Option<i64>) -> LocalBoxFuture<'static, Result<Rc<dyn StdioRoad>, String>>;
    fn place_name_here(&self) -> String;
    fn agent_label(&self, agent: &str) -> String;
    fn rows_on(&self, thread: &str) -> Vec<SessionView>;
    /// The workspace's transcript events, as JSON.
    fn transcript(&self, workspace: &str) -> Vec<Value>;
    fn usage_account_key(&self, entry: &Entry, harness: &str) -> String;
    fn usage_accounts(&self) -> LocalBoxFuture<'static, Vec<AccountRow>>;
    /// `ctx.projectHeld(project).name`, which throws where the project is gone.
    fn project_name(&self, project: &str) -> Result<String, HostError>;
    fn accrued(&self, workspace: &str) -> Option<f64>;
    fn start_session(&self, start: Delivery) -> LocalBoxFuture<'static, Result<Delivered, HostError>>;
    /// Calls `f` on the `session.queued` event with this request id; the answer stops listening.
    fn on_queued(&self, request_id: &str, f: Box<dyn Fn()>) -> Box<dyn FnOnce()>;
    fn warn(&self, line: &str);
    fn poll_pull_request(&self, entry: &Entry);
    fn local_env(&self) -> Option<IndexMap<String, String>>;
    fn process_env(&self) -> IndexMap<String, String>;
    /// None without a state path.
    fn state_folder(&self) -> Option<String>;
    /// Calls `f` with the workspace id of every `workspace.napped`.
    fn on_napped(&self, f: Box<dyn Fn(&str)>);
}

/// `createSlates`'s deps as `slatesArea` builds them: `SlatesDeps` over the area's context. Every method reads the
/// context per call.
pub struct SlatesWiring<C: SlatesAreaCtx> {
    ctx: Rc<C>,
}

/// Registers the nap listener (spec rule 14) with `napped`, the slate area's `napped`.
pub fn slates_area<C: SlatesAreaCtx + 'static>(ctx: Rc<C>, napped: Rc<dyn Fn(&str)>) -> SlatesWiring<C> {
    ctx.on_napped(Box::new(move |workspace: &str| napped(workspace)));
    SlatesWiring { ctx }
}

impl<C: SlatesAreaCtx + 'static> SlatesWiring<C> {
    /// slates.ts:16.
    pub fn harness_of(&self, thread: &str) -> Option<String> {
        harness_of(&*self.ctx, thread)
    }

    /// slates.ts:18.
    pub fn servers_target(&self, entry: &Entry) -> AgentsTarget {
        servers_target(entry)
    }

    /// slates.ts:22.
    pub fn machine_on_box(&self, entry: &Entry) -> bool {
        machine_on_box(&*self.ctx, entry)
    }

    /// spec rule 7: the thread's server, read on its box or from this computer.
    async fn mcp_spec(&self, thread_id: &str, name: &str) -> Result<McpServerSpec, HostError> {
        let ctx = &*self.ctx;
        let workspace_id =
            ctx.latest_on(thread_id).map(|row| row.workspace_id).or_else(|| ctx.thread_record(thread_id).map(|record| record.workspace_id));
        let harness = harness_of(ctx, thread_id);
        let entry = workspace_id.as_deref().and_then(|workspace| ctx.live(workspace));
        let (Some(entry), Some(harness), Some(workspace_id)) = (entry, harness, workspace_id) else {
            return Err(HostError::new(format!("this host holds no folder for thread {thread_id}")));
        };

        if let Some(box_entry) = ctx.box_of(thread_id) {
            let computer = ctx.computer_of(&box_entry);
            if machine_on_box(ctx, &box_entry) {
                let machine = box_entry.borrow().record.borrow().name.clone();
                return Err(HostError::new(machine_on_box_line(name, &machine, &computer)));
            }
            let read = ctx.agents_server(servers_target(&box_entry), &harness, name).await.map_err(|e| HostError::new(read_message(&e)))?;
            let ReadServer { spec, login } = read;
            let Some(login) = login else {
                return Err(HostError::new(format!("{computer} did not say who its lines run as")));
            };
            let mark: IndexMap<String, String> =
                IndexMap::from([(THREAD_ENV.to_string(), thread_id.to_string()), ("SLATE_DIR".to_string(), box_slate_dir(thread_id))]);
            let open = pipe_on(&computer, login, stdio_dial(self.ctx.clone(), box_entry.clone(), None), mark, Rc::new(OsIds));
            let make = post_on(&computer, stdio_dial(self.ctx.clone(), box_entry, Some(HTTP_POST_DAEMON_VERSION)), spec.sign_in.clone());
            let post: McpPostRoad = Rc::new(move |_own: Rc<dyn McpPost>| make());
            return Ok(McpServerSpec { open: Some(open), post: Some(post), ..spec });
        }

        let path = ctx.checkout_of(&entry)?;
        let Some(read) = ctx.agents_reader_server(
            vec![AgentsProject { id: workspace_id, name: "thread".into(), path }],
            &harness,
            name,
        ) else {
            return Err(HostError::new("this host reads no agent's MCP config"));
        };
        let spec = read.await?;
        let computer = ctx.computer_of(&entry);
        match spec.sign_in.clone() {
            Some(sign_in) if sign_in.read.is_some() => Ok(McpServerSpec { post: Some(post_here(&computer, sign_in)), ..spec }),
            _ => Ok(spec),
        }
    }
}

/// `harness_of` in slates.ts:16: the latest row's harness, else the thread record's.
fn harness_of<C: SlatesAreaCtx + ?Sized>(ctx: &C, thread: &str) -> Option<String> {
    ctx.latest_on(thread).map(|row| row.harness).or_else(|| ctx.thread_record(thread).map(|record| record.harness))
}

fn record_of(entry: &Entry) -> WorkspaceRecord {
    entry.borrow().record.borrow().clone()
}

fn servers_target(entry: &Entry) -> AgentsTarget {
    let record = record_of(entry);
    match &record.place {
        Some(place) if folder_on_joined(&record.kind) => {
            AgentsTarget::V0(AgentsTargetV0 { place_id: place.clone(), project: Some(record.project.clone()) })
        }
        _ => AgentsTarget::V1(AgentsTargetV1 { workspace_id: record.id.clone() }),
    }
}

fn machine_on_box<C: SlatesAreaCtx + ?Sized>(ctx: &C, entry: &Entry) -> bool {
    let record = record_of(entry);
    record.place.as_deref().is_some_and(|place| !folder_on_joined(&record.kind) && !ctx.at_provider(place))
}

fn machine_on_box_line(name: &str, machine: &str, computer: &str) -> String {
    format!(
        "${name} calls an MCP server, and this thread runs on {machine}, a machine wsp made on {computer}, whose server would start on {computer} outside it: a slate's tool run goes only to a thread in a folder on {computer} or on a cloud machine, so start the thread in the project's folder on {computer}"
    )
}

fn read_message(error: &ServerReadError) -> String {
    match error {
        ServerReadError::NotSetUp(message) | ServerReadError::Off(message) | ServerReadError::Other(message) => message.clone(),
    }
}

fn stdio_dial<C: SlatesAreaCtx + 'static>(ctx: Rc<C>, entry: Entry, min_version: Option<i64>) -> StdioRoadDial {
    Rc::new(move || ctx.stdio_road(&entry, min_version))
}

fn started_at(row: &SessionView) -> f64 {
    row.started_at.as_ref().map_or(0.0, |n| n.0)
}

/// spec rule 9: which of `servers` the thread's agent on its box does not name, and which it names but has off.
async fn missing_on_read<C: SlatesAreaCtx>(ctx: &C, thread: &str, servers: Vec<String>) -> MissingOn {
    let agent = harness_of(ctx, thread);
    match (ctx.box_of(thread), agent) {
        (Some(entry), Some(agent)) => {
            let target = servers_target(&entry);
            let asked: Vec<_> = servers.iter().map(|name| ctx.agents_server(target.clone(), &agent, name)).collect();
            let said = join_all(asked).await;
            let off: Vec<String> = said
                .iter()
                .filter_map(|answer| match answer {
                    Err(ServerReadError::Off(message)) => Some(message.clone()),
                    _ => None,
                })
                .collect();
            let missing: Vec<String> = servers
                .iter()
                .zip(&said)
                .filter(|(_, answer)| matches!(answer, Err(ServerReadError::NotSetUp(_))))
                .map(|(name, _)| name.clone())
                .collect();
            MissingOn { agent_id: agent.clone(), agent_name: ctx.agent_label(&agent), thread: thread.to_string(), missing, off }
        }
        (_, agent) => {
            let agent = agent.unwrap_or_default();
            MissingOn { agent_id: agent.clone(), agent_name: agent, thread: thread.to_string(), missing: vec![], off: vec![] }
        }
    }
}

/// `sources(...)` in slates.ts:117: the source reads of one thread in one workspace, the clock read once at build.
struct Sources<C: SlatesAreaCtx> {
    ctx: Rc<C>,
    thread: String,
    workspace: String,
    now: i64,
}

impl<C: SlatesAreaCtx + 'static> SlateSourceContext for Sources<C> {
    fn thread_id(&self) -> &str {
        &self.thread
    }
    fn workspace_id(&self) -> &str {
        &self.workspace
    }
    fn now(&self) -> i64 {
        self.now
    }
    fn rows(&self) -> Vec<SessionView> {
        let mut rows = self.ctx.rows_on(&self.thread);
        rows.sort_by(|a, b| started_at(a).partial_cmp(&started_at(b)).unwrap_or(Ordering::Equal));
        rows
    }
    fn results(&self) -> Vec<TurnResult> {
        thread_results(&self.ctx.transcript(&self.workspace), &self.thread)
    }
    fn account(&self) -> LocalBoxFuture<'_, Option<AccountRow>> {
        let key = self.ctx.live(&self.workspace).and_then(|entry| {
            let harness = harness_of(&*self.ctx, &self.thread)?;
            Some(self.ctx.usage_account_key(&entry, &harness))
        });
        let ctx = self.ctx.clone();
        async move {
            let key = key?;
            ctx.usage_accounts().await.into_iter().find(|account| account.key == key)
        }
        .boxed_local()
    }
    fn workspace(&self) -> Option<SlateWorkspaceFacts> {
        let entry = self.ctx.live(&self.workspace)?;
        let project = record_of(&entry).project;
        let (checkout, pr) = {
            let live = entry.borrow();
            (live.checkout.clone(), live.pr.clone())
        };
        Some(SlateWorkspaceFacts {
            computer: Some(self.ctx.computer_of(&entry)),
            project: Some(self.ctx.project_name(&project).ok()?),
            folder: Some(self.ctx.checkout_of(&entry).ok()?),
            checkout,
            pr,
            rate_usd_per_hour: Some(self.ctx.entry_rate_usd_per_hour(&entry).unwrap_or(0.0)),
            accrued_usd: Some(self.ctx.accrued(&self.workspace).unwrap_or(0.0)),
        })
    }
}

#[async_trait(?Send)]
impl<C: SlatesAreaCtx + 'static> SlatesDeps for SlatesWiring<C> {
    fn store(&self) -> Rc<dyn Store> {
        self.ctx.store()
    }

    fn clock(&self) -> Rc<dyn Clock> {
        self.ctx.clock()
    }

    fn record(&self, event: SessionSlateEvent) {
        self.ctx.record(event)
    }

    fn emit(&self, event: SlateEmit) {
        self.ctx.emit(event)
    }

    /// spec rule 4.
    fn thread(&self, thread_id: &str) -> Option<SlateThreadFacts> {
        let ctx = &*self.ctx;
        let latest = ctx.latest_on(thread_id);
        let record = ctx.thread_record(thread_id);
        let workspace_id =
            latest.as_ref().map(|row| row.workspace_id.clone()).or_else(|| record.as_ref().map(|record| record.workspace_id.clone()))?;
        let turn_id = ctx.running_turn(thread_id);
        let ran_in = latest.as_ref().and_then(|row| row.cwd.clone()).or_else(|| {
            let session = latest.as_ref()?.claude_session_id.as_ref()?;
            ctx.folder_of(&workspace_id, session)
        });
        let mode = record
            .as_ref()
            .and_then(|record| record.permission_mode.as_deref())
            .or_else(|| latest.as_ref().and_then(|row| row.permission_mode.as_deref()));
        let tool_asks = Some(ctx.tool_asks(harness_of(ctx, thread_id).as_deref(), mode));
        let session_id = latest
            .as_ref()
            .and_then(|row| row.claude_session_id.clone())
            .or_else(|| latest.as_ref().map(|row| row.id.clone()))
            .unwrap_or_else(|| thread_id.to_string());
        let root_thread_id = ctx.root_of(thread_id);

        let Some(entry) = ctx.live(&workspace_id) else {
            return Some(SlateThreadFacts {
                workspace_id,
                root_thread_id,
                session_id,
                turn_id,
                folder: None,
                host_folder: None,
                computer: None,
                tool_asks,
            });
        };
        // The facts answer none where the checkout read throws (slates.ts:40 would reject): the seam is infallible.
        let fallback = match ctx.kind_folder(&entry) {
            Some(folder) => folder,
            None => ctx.checkout_of(&entry).ok()?,
        };
        let folder = ctx.runs_in(&entry, ran_in.as_deref(), &fallback);
        let host_folder = if is_local_workspace(Some(&record_of(&entry).kind)) {
            folder.clone()
        } else {
            let checkout = ctx.checkout_of(&entry).ok()?;
            if ctx.path_exists(&checkout) {
                checkout
            } else {
                ctx.home_dir()
            }
        };
        Some(SlateThreadFacts {
            workspace_id,
            root_thread_id,
            session_id,
            turn_id,
            folder: Some(folder),
            host_folder: Some(host_folder),
            computer: Some(ctx.computer_of(&entry)),
            tool_asks,
        })
    }

    async fn loaded(&self) -> Result<(), HostError> {
        self.ctx.ready().await
    }

    fn machine_of(&self, thread_id: &str) -> Option<Arc<dyn Machine>> {
        let entry = self.ctx.box_of(thread_id)?;
        let machine = entry.borrow().machine.clone();
        Some(machine)
    }

    fn place_of(&self, thread_id: &str) -> Option<Arc<PlaceFolderMachine>> {
        self.ctx.place_of(thread_id)
    }

    fn image_on(&self, thread_id: &str) -> Option<ImageOn> {
        let entry = self.ctx.box_of(thread_id)?;
        let name = self.ctx.computer_of(&entry);
        let ctx = self.ctx.clone();
        let read: Rc<dyn Fn(String) -> LocalBoxFuture<'static, Result<FsImageReply, ImageRefusal>>> = Rc::new(move |path: String| {
            let reply = ctx.daemon_image(&entry, path);
            async move {
                let value = reply.await?;
                wsp_wire::types::check::parse::<FsImageReply>(&value).map_err(|e| ImageRefusal { code: None, message: e.message })
            }
            .boxed_local()
        });
        Some(ImageOn { name, read })
    }

    fn hash_on(&self, thread_id: &str) -> Option<HashOn> {
        let entry = self.ctx.box_of(thread_id)?;
        let ctx = self.ctx.clone();
        let hash: HashOn = Rc::new(move |root: String, paths: Vec<String>| {
            let reply = ctx.daemon_hash(&entry, root, paths);
            async move { parse_hash_reply(reply.await?) }.boxed_local()
        });
        Some(hash)
    }

    fn asleep(&self, thread_id: &str) -> bool {
        self.ctx.box_of(thread_id).is_some_and(|entry| record_of(&entry).phase != WorkspacePhase::Running)
    }

    /// spec rule 5.
    async fn wake(&self, thread_id: &str) -> Result<(), HostError> {
        let Some(entry) = self.ctx.box_of(thread_id) else {
            return Ok(());
        };
        let id = record_of(&entry).id;
        self.ctx.boot_work(&id).await;
        if record_of(&entry).phase == WorkspacePhase::Running {
            return Ok(());
        }
        self.ctx.wake(&id).await
    }

    /// spec rule 6.
    async fn settled(&self, thread_id: &str) -> Option<bool> {
        let key = self.ctx.settle_after().await;
        Some(self.ctx.settled_now(thread_id, settle_ms(&key)))
    }

    fn under(&self, lead: &str) -> Vec<String> {
        self.ctx.tree_under(lead)
    }

    fn thread_of_token(&self, token: &str) -> Result<String, HostError> {
        self.ctx.thread_of_token(token)
    }

    /// spec rule 10.
    fn sources(&self, thread_id: &str, workspace_id: &str) -> Rc<dyn SlateSourceContext> {
        Rc::new(Sources {
            ctx: self.ctx.clone(),
            thread: thread_id.to_string(),
            workspace: workspace_id.to_string(),
            now: self.ctx.clock().now(),
        })
    }

    /// spec rule 11.
    async fn deliver(&self, o: Delivery) -> Result<Delivered, HostError> {
        let (queued_tx, queued_rx) = futures::channel::oneshot::channel::<()>();
        let queued_tx = Rc::new(RefCell::new(Some(queued_tx)));
        let off = self.ctx.on_queued(
            &o.request_id,
            Box::new(move || {
                let sent = queued_tx.borrow_mut().take();
                if let Some(sent) = sent {
                    let _ = sent.send(());
                }
            }),
        );
        let thread = o.thread_id.clone();
        let started = self.ctx.start_session(o);
        let raced = select(started, queued_rx).await;
        off();
        match raced {
            Either::Left((first, _)) => first,
            Either::Right((Ok(()), started)) => {
                let ctx = self.ctx.clone();
                tokio::task::spawn_local(async move {
                    if let Err(e) = started.await {
                        ctx.warn(&format!("a press waiting in thread {} was not sent: {}", thread_word(&thread), e.message));
                    }
                });
                Ok(Delivered { outcome: DeliveryOutcome::Queued, turn_id: None })
            }
            Either::Right((Err(_), started)) => started.await,
        }
    }

    fn watch_pr(&self, workspace_id: &str) {
        if let Some(entry) = self.ctx.live(workspace_id) {
            self.ctx.poll_pull_request(&entry);
        }
    }

    fn run_env(&self) -> IndexMap<String, String> {
        self.ctx.local_env().unwrap_or_else(|| self.ctx.process_env())
    }

    /// `<state folder>/slates.secrets.json`; none without a state path.
    fn secrets_file(&self) -> Option<PathBuf> {
        self.ctx.state_folder().map(|folder| PathBuf::from(folder).join("slates.secrets.json"))
    }

    /// `<state folder>/slates`; none without a state path.
    fn slates_dir(&self) -> Option<PathBuf> {
        self.ctx.state_folder().map(|folder| PathBuf::from(folder).join("slates"))
    }

    /// spec rule 7.
    async fn mcp_server(&self, thread_id: &str, name: &str) -> Option<Result<McpServerSpec, HostError>> {
        Some(self.mcp_spec(thread_id, name).await)
    }

    fn own_process(&self, thread_id: &str, pid: u32, kind: &str) -> Option<Rc<dyn Fn()>> {
        let kind = match kind {
            "slate" => ProcKind::Slate,
            "tool" => ProcKind::Tool,
            _ => return None,
        };
        let detach: RefCell<Option<Detach>> = RefCell::new(Some(self.ctx.own_process(thread_id, pid, kind)));
        let own: Rc<dyn Fn()> = Rc::new(move || {
            let detach = detach.borrow_mut().take();
            if let Some(detach) = detach {
                detach();
            }
        });
        Some(own)
    }

    /// spec rule 8.
    fn away(&self, thread_id: &str) -> Option<Away> {
        let ctx = &*self.ctx;
        let entry = ctx.box_of(thread_id)?;
        let machine = machine_on_box(ctx, &entry).then(|| record_of(&entry).name);
        Some(Away { computer: ctx.computer_of(&entry), host: ctx.place_name_here(), machine })
    }

    /// spec rule 9.
    fn missing_on(&self, thread_id: &str) -> Option<MissingOnFn> {
        let ctx = self.ctx.clone();
        let thread = thread_id.to_string();
        let asked: MissingOnFn = Rc::new(move |servers: Vec<String>| {
            let ctx = ctx.clone();
            let thread = thread.clone();
            async move { missing_on_read(&*ctx, &thread, servers).await }.boxed_local()
        });
        Some(asked)
    }
}

/// The hash read a box thread's daemon gave, parsed (slates.ts:66).
pub fn parse_hash_reply(reply: Value) -> Result<HashRead, String> {
    wsp_wire::types::check::parse::<FsHashReply>(&reply).map_err(|e| e.message)
}

/// `SETTLE_MS` (protocol general-prefs.ts:38): none for `never` or an unknown key.
pub fn settle_ms(key: &str) -> Option<f64> {
    match key {
        "15m" => Some(15.0 * 60_000.0),
        "1h" => Some(60.0 * 60_000.0),
        "2h" => Some(2.0 * 60.0 * 60_000.0),
        "1d" => Some(24.0 * 60.0 * 60_000.0),
        _ => None,
    }
}

/// The results of this thread's ended turns in a transcript (slates.ts:122).
pub fn thread_results(transcript: &[Value], thread: &str) -> Vec<TurnResult> {
    transcript
        .iter()
        .filter(|event| event["type"] == "session.done" && event["threadId"].as_str() == Some(thread))
        .filter_map(|event| serde_json::from_value(event["result"].clone()).ok())
        .collect()
}
