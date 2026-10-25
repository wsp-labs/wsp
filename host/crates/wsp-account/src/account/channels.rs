// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/account/channels.ts at 65f6b89c7: one area in three files (README A3). This file holds
//! the context the area reads, the area's state and its constructor; `daemon.rs` the facts of a record and the roads
//! to a daemon (channels.ts:1-280), `ptys.rs` the pty owners, tags, kill guard and port roots (280-580), `copy.rs` the
//! served and copy channels, shell tokens and where a child starts (580-934). Specs: spec/channels_*.md.
use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::{IndexMap, IndexSet};
use serde_json::Value;

use wsp_core::daemon_channel::DaemonChannel;
use wsp_core::devices::{DeviceDoor, PairedDevice, ScopedRoad};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::views::place_more::{PlaceSettings, PlaceView};
use wsp_wire::views::workspace::{DaemonReachView, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::wire::place_link::PlaceReport;

use crate::account::box_panels::{BoxPanelsCtx, BoxPanelsOpener, Watchers};
use crate::account::core_types::{AccountResult, Cancel, Live};

pub mod copy;
pub mod daemon;
pub mod ptys;

/// channels.ts:21: how long the origin read may hold the host's one thread.
pub const ORIGIN_READ_MS: i64 = 2_000;
/// channels.ts:24: how long an op that carries a token waits for the daemon's hello.
pub const HELLO_WAIT_MS: i64 = 2_000;
/// channels.ts:27: the ops whose git or gh reach the git host, in the TS set's order.
pub const OP_TOKEN_OPS: [&str; 6] = ["git.push", "git.startOn", "git.update", "git.mergeIn", "git.pr", "git.prCheckout"];
/// channels.ts:282.
pub const GUEST_EVENTS: [&str; 5] = ["guest.opened", "guest.message", "guest.closed", "tunnel.data", "tunnel.end"];
/// channels.ts:285.
pub const PTY_EVENTS: [&str; 3] = ["pty.data", "pty.exit", "pty.mode"];
/// channels.ts:288.
pub const COMPUTER_WATCHES: [&str; 2] = ["ports.watch", "sys.watch"];
/// channels.ts:291.
pub const COMPUTER_PROCS: [&str; 5] = ["proc.watch", "proc.unwatch", "proc.inspect", "proc.kill", "container.stop"];
/// channels.ts:295.
pub const WORKSPACE_FRAMES: [&str; 22] = [
    "pty.create", "pty.attach", "pty.detach", "pty.write", "pty.resize", "pty.kill", "pty.tab", "pty.list", "fs.list", "fs.files", "fs.read", "fs.write", "fs.search", "git.status", "git.diff", "git.snapshot", "git.range", "git.turn", "git.push", "git.pr", "git.prList", "ping",
];
/// channels.ts:299: `WORKSPACE_FRAMES`, then these.
pub const GUEST_ROAD_EXTRA: [&str; 7] = ["guest.watch", "guest.reply", "guest.close", "ssh.start", "tunnel.open", "tunnel.write", "tunnel.close"];
/// channels.ts:378: one document per workspace, pty id → thread.
pub const PTY_THREADS: &str = "pty-threads";
/// channels.ts:494.
pub const PORT_ROOTS_RECHECK_MS: i64 = 5_000;
/// channels.ts:717: one document per workspace, pty id → device id.
pub const PTY_TOKENS: &str = "pty-tokens";
/// channels.ts:790.
pub const BLOCKED_READS: [&str; 2] = ["git.status", "ping"];

/// One session row as the channels read it: its workspace, the harness session it resumes, the facts it recorded
/// (`cwd`, `permissionMode`, ... as `SESSION_FACTS` names them), and what the kill guard reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelRow {
    pub workspace_id: String,
    pub claude_session_id: Option<String>,
    pub facts: IndexMap<String, String>,
    pub status: String,
    pub pid: Option<i64>,
    /// The run the turn's launch recorded (`s.run`), read by `agentPidOf`.
    pub run: Option<String>,
}

/// A thread record as the channels read it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelThread {
    pub workspace_id: String,
    pub permission_mode: Option<String>,
    pub harness: Option<String>,
}

/// What `ctx.threadLaunch(entry, threadId, root)` answers: the turn's scoped device and its environment.
pub struct ThreadLaunch {
    pub scoped: Option<PairedDevice>,
    pub env: IndexMap<String, String>,
}

/// `terminalSignIn(...)`'s answer: the program a shell signs in with and its environment.
#[derive(Clone, Debug, PartialEq)]
pub struct TerminalSignIn {
    pub program: String,
    pub env: IndexMap<String, String>,
}

/// What the channels area reads off the runtime's context and the modules it imports.
#[async_trait(?Send)]
pub trait ChannelsCtx {
    // The transcript index and the rows (daemon D1, D2).
    /// `transcriptIndex.get(ws)?.starts.get(thread)`.
    fn index_start(&self, workspace_id: &str, thread_id: &str) -> Option<String>;
    /// `transcriptIndex.get(ws)?.cut.get(thread)`.
    fn index_cut(&self, workspace_id: &str, thread_id: &str) -> Option<bool>;
    /// `transcriptIndex.get(ws)?.facts.get(resume)?.[fact]`.
    fn index_fact(&self, workspace_id: &str, resume: &str, fact: &str) -> Option<String>;
    /// `sessions.values()`, in map order.
    fn session_rows(&self) -> Vec<ChannelRow>;
    /// `threadRecords.get(id)`.
    fn thread_record(&self, thread_id: &str) -> Option<ChannelThread>;
    /// `threadRecords`' ids, in map order.
    fn thread_ids(&self) -> Vec<String>;
    /// `ctx.latestOn(thread)?.harness`.
    fn latest_harness(&self, thread_id: &str) -> Option<String>;

    // The kind's module.
    /// `moduleOf(kind).homeDir(record)`.
    fn home_dir(&self, record: &WorkspaceRecord) -> Option<String>;
    /// `moduleOf(kind).daemonRoad(entry)`.
    async fn daemon_road(&self, entry: &Live) -> AccountResult<DaemonReachView>;
    /// `moduleOf(kind).turnRoad`.
    fn turn_road(&self, kind: WorkspaceKind) -> Option<ScopedRoad>;

    // The place door (none on a host without one).
    fn has_door(&self) -> bool;
    fn offer_of(&self, place_id: &str) -> Option<String>;
    /// `places.wired`.
    fn wired(&self) -> String;
    fn settings_at(&self, place_id: &str) -> Option<PlaceSettings>;
    /// `placeDoor?.turnLimitAt(placeId)`: None where the door lists no row, `Some(None)` for no limit.
    fn turn_limit_at(&self, place_id: &str) -> Option<Option<i64>>;
    fn name_of(&self, place_id: &str) -> String;
    fn joined(&self, place_id: &str) -> bool;
    async fn report_of(&self, place_id: &str) -> Option<PlaceReport>;
    async fn rows(&self) -> Vec<PlaceView>;
    /// `placeDoor.channel(placeId, onEvent)`: the link as a channel, None where none stands.
    fn link_channel(&self, place_id: &str, on_event: Rc<dyn Fn(Value)>) -> Option<Rc<dyn DaemonChannel>>;
    /// `ctx.status.spend(rows, now)`: each place's spend today.
    async fn spend_today(&self, rows: &[PlaceView], now: i64) -> Vec<(String, Option<f64>)>;

    // The projects.
    fn projects_held(&self) -> IndexMap<String, ProjectView>;

    // Dials.
    /// `ctx.openChannel({ url, token, onEvent })`.
    async fn open_channel(&self, url: &str, token: &str, on_event: Rc<dyn Fn(Value)>) -> AccountResult<Rc<dyn DaemonChannel>>;
    /// `local?.daemonRoad !== undefined`.
    fn local_has_daemon(&self) -> bool;
    /// `local.homeDir`.
    fn local_home_dir(&self) -> String;
    /// `ctx.localRoad()`.
    async fn local_road(&self) -> AccountResult<DaemonReachView>;
    async fn adopt_pause(&self, entry: &Live);
    /// `ctx.opts.broker?.ready() === true`.
    fn broker_ready(&self) -> bool;
    fn root_of(&self, thread_id: &str) -> String;
    fn device_door(&self) -> Rc<dyn DeviceDoor>;

    // This process (ptys Y3, Y4).
    fn host_pid(&self) -> i64;
    fn host_parent_pid(&self) -> i64;
    /// `ctx.opts.statePath !== undefined`.
    fn has_state_path(&self) -> bool;
    fn state_folder(&self) -> String;
    fn read_link(&self, path: &str) -> Option<String>;
    fn hostname(&self) -> String;
    /// `process.kill(pid, 0)` succeeded.
    fn pid_alive(&self, pid: i64) -> bool;
    /// Every catalog agent's `bin`, catalog order.
    fn agent_bins(&self) -> Vec<String>;
    /// `agentPidOf(run)`.
    fn agent_pid_of(&self, run: &str) -> Option<i64>;
    /// `groupExists(pid)`.
    fn group_exists(&self, pgid: i64) -> bool;
    /// `ctx.procScopesMoved()`.
    fn proc_scopes_moved(&self);
    /// `ctx.state.rootsRecheck`: the one interval that stands.
    fn roots_recheck(&self) -> Rc<RefCell<Option<Cancel>>>;
    /// The box panels' context (`boxPanelsOpener(ctx, ...)`).
    fn box_panels_ctx(&self) -> Rc<dyn BoxPanelsCtx>;

    // The store.
    async fn store_get(&self, collection: &str, id: &str) -> AccountResult<Option<Value>>;
    async fn store_put(&self, collection: &str, id: &str, value: Value) -> AccountResult<()>;
    async fn store_delete(&self, collection: &str, id: &str) -> AccountResult<()>;
    async fn store_keys(&self, collection: &str) -> AccountResult<Vec<String>>;
    fn on_workspace_deleted(&self, f: Rc<dyn Fn(&str)>);
    fn live_has(&self, workspace_id: &str) -> bool;

    // The copy's acts (copy C6-C14).
    async fn thread_launch(&self, entry: &Live, thread_id: &str, root_thread_id: &str) -> AccountResult<ThreadLaunch>;
    /// `brokeredEnv(token, entry.record.branchOfRecord, publicMarketplaces(ctx))`.
    fn brokered_env(&self, token: &str, entry: &Live) -> IndexMap<String, String>;
    /// `terminalSignIn(harness, launchToken => ctx.brokeredLaunch(entry, harness, launchToken), token)`.
    fn terminal_sign_in(&self, harness: &str, entry: &Live, token: &str) -> Option<TerminalSignIn>;
    /// `terminalLine(harness, program)`.
    fn terminal_line(&self, harness: &str, program: &str) -> String;
    async fn thread_folder(&self, entry: &Live) -> String;
    fn thread_env(&self, entry: &Live) -> IndexMap<String, String>;
    async fn persist(&self, record: &WorkspaceRecord) -> AccountResult<()>;
    /// `void ctx.readCheckout(entry, true)`.
    fn read_checkout_soon(&self, entry: &Live);
    /// `ctx.workspaces.delete(id)`.
    async fn delete_workspace(&self, workspace_id: &str) -> AccountResult<()>;
    /// `spawnSync("git", ["-C", path, "remote", "get-url", "origin"], { timeout })`: exit status and stdout.
    fn git_origin(&self, path: &str, timeout_ms: i64) -> (Option<i32>, String);

    // The host's own.
    fn now(&self) -> i64;
    fn schedule(&self, ms: i64, f: Box<dyn FnOnce()>) -> Cancel;
    fn spawn(&self, f: LocalBoxFuture<'static, ()>);
    fn warn(&self, line: &str);
}

/// channels.ts:29 `channelsArea(ctx)`: every map the TS closure holds.
pub struct ChannelsArea {
    pub(crate) ctx: Rc<dyn ChannelsCtx>,
    /// channels.ts:67: the line a workspace's row shows while its daemon is updated.
    pub daemon_notes: RefCell<IndexMap<String, String>>,
    /// channels.ts:303: each local workspace's pty → leading pid.
    pub port_pids: Rc<RefCell<IndexMap<String, IndexMap<String, i64>>>>,
    /// channels.ts:308: each owner's ptys.
    pub(crate) pty_owners: RefCell<IndexMap<String, Rc<RefCell<IndexSet<String>>>>>,
    /// channels.ts:379: each workspace's tag read, joined by a second reader.
    pub(crate) tags_held: RefCell<IndexMap<String, futures::future::Shared<LocalBoxFuture<'static, Rc<RefCell<IndexMap<String, String>>>>>>>,
    /// channels.ts:381: the tags once read (`tagsRead`).
    pub(crate) tags_read: RefCell<IndexMap<String, Rc<RefCell<IndexMap<String, String>>>>>,
    /// channels.ts:406: each workspace's last queued tag write.
    pub(crate) tag_writes: RefCell<IndexMap<String, futures::future::Shared<LocalBoxFuture<'static, ()>>>>,
    /// channels.ts:409.
    pub(crate) tags_going: RefCell<IndexSet<String>>,
    /// channels.ts:487.
    pub(crate) port_watchers: Watchers,
    /// channels.ts:491: by workspace, each group a turn led and its thread.
    pub turn_groups: Rc<RefCell<IndexMap<String, IndexMap<i64, String>>>>,
    /// channels.ts:518.
    pub(crate) box_panels: Rc<BoxPanelsOpener>,
    /// channels.ts:698.
    pub(crate) terminals_written: RefCell<IndexMap<String, IndexSet<String>>>,
    /// channels.ts:718: each workspace's pty → device, once read.
    pub(crate) shell_tokens: RefCell<IndexMap<String, Rc<RefCell<IndexMap<String, String>>>>>,
}

impl ChannelsArea {
    /// channels.ts:29. Registers the two `workspace.deleted` listeners in the TS order (429, 778).
    pub fn new(ctx: Rc<dyn ChannelsCtx>) -> Rc<ChannelsArea> {
        todo!()
    }
}
