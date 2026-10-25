// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/account/slates.ts at 65f6b89c7. Spec: spec/account_slates.md.
//!
//! The slate area's wiring: what `createSlates` (slates-a's) is handed, each answer read off the runtime per call.
//! Everything it reads goes through `SlatesAreaCtx` (ruling 6401022701 rule 4), implemented on wsp-core's context
//! handle at the join and by a fake in the tests.
#![allow(unused_variables)]

use std::rc::Rc;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::Value;

use wsp_engine::machine::Machine;
use wsp_engine::target_line::TargetLogin;

use crate::slate_files::{HashOn, HashRead};
use crate::slate_images::ImageOn;
use crate::slate_mcp::{Away, McpServerSpec, MissingOn, ToolAsks};
use crate::slate_mcp_on::StdioRoad;
use crate::slate_sources::context::{AccountRow, Checkout, PullRequestSeen, SessionView, TurnResult};
use crate::slate_sources::SlateSourceContext;

/// What a thread's record says, as the area reads it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ThreadFacts {
    pub workspace_id: Option<String>,
    pub harness: Option<String>,
    pub permission_mode: Option<String>,
}

/// Where a box thread's agent reads its servers (slates.ts:18).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServersTarget {
    Place { place_id: String, project: Option<String> },
    Workspace { workspace_id: String },
}

/// The projects a server read on this computer is asked about (slates.ts:100).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HereProject {
    pub id: String,
    pub name: String,
    pub path: String,
}

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

/// A session start a press delivers (slates.ts:154).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliverStart {
    pub workspace_id: String,
    pub prompt: String,
    pub thread: String,
    pub request_id: String,
}

/// What a start answered: `outcome` and the turn where one started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    pub outcome: String,
    pub turn_id: Option<String>,
}

/// What the area reads off the runtime. `Entry` is a live workspace.
pub trait SlatesAreaCtx {
    type Entry: Clone + 'static;

    fn latest_on(&self, thread: &str) -> Option<SessionView>;
    fn thread_record(&self, thread: &str) -> Option<ThreadFacts>;
    /// The `turnId` of a running session of this thread, the first in the sessions map.
    fn running_turn(&self, thread: &str) -> Option<String>;
    fn live(&self, workspace: &str) -> Option<Self::Entry>;
    fn box_of(&self, thread: &str) -> Option<Self::Entry>;
    fn folder_of(&self, workspace: &str, session: &str) -> Option<String>;
    fn runs_in(&self, entry: &Self::Entry, ran_in: Option<&str>, fallback: &str) -> String;
    /// The kind module's `folder(record)`.
    fn kind_folder(&self, entry: &Self::Entry) -> Option<String>;
    fn checkout_of(&self, entry: &Self::Entry) -> String;
    fn is_local(&self, entry: &Self::Entry) -> bool;
    fn path_exists(&self, path: &str) -> bool;
    fn home_dir(&self) -> String;
    fn computer_of(&self, entry: &Self::Entry) -> String;
    fn root_of(&self, thread: &str) -> String;
    fn tool_asks(&self, harness: Option<&str>, mode: Option<&str>) -> ToolAsks;

    // The entry's record.
    fn entry_id(&self, entry: &Self::Entry) -> String;
    fn entry_name(&self, entry: &Self::Entry) -> String;
    fn entry_place(&self, entry: &Self::Entry) -> Option<String>;
    fn entry_project(&self, entry: &Self::Entry) -> Option<String>;
    /// `folder_on_joined(record.kind)`.
    fn entry_folder_on_joined(&self, entry: &Self::Entry) -> bool;
    fn entry_running(&self, entry: &Self::Entry) -> bool;
    fn entry_machine(&self, entry: &Self::Entry) -> Option<Arc<dyn Machine>>;
    fn entry_checkout(&self, entry: &Self::Entry) -> Option<Checkout>;
    fn entry_pr(&self, entry: &Self::Entry) -> Option<PullRequestSeen>;
    /// The backend's hourly rate for the record's size; none without a size.
    fn entry_rate_usd_per_hour(&self, entry: &Self::Entry) -> Option<f64>;
    fn at_provider(&self, place: &str) -> bool;

    fn boot_work(&self, workspace: &str) -> LocalBoxFuture<'static, ()>;
    fn wake(&self, workspace: &str) -> LocalBoxFuture<'static, Result<(), String>>;
    fn ready(&self) -> LocalBoxFuture<'static, ()>;
    fn settled_now(&self, thread: &str, ms: Option<u64>) -> LocalBoxFuture<'static, ()>;
    /// The settle preference's key: `15m`, `1h`, `2h`, `1d` or `never`.
    fn settle_after(&self) -> LocalBoxFuture<'static, String>;
    fn tree_under(&self, lead: &str) -> Vec<String>;
    fn thread_of_token(&self, token: &str) -> Option<String>;
    fn own_process(&self, thread: &str, pid: u32, kind: &str) -> Box<dyn FnOnce()>;
    fn daemon_image(&self, entry: &Self::Entry, path: String) -> LocalBoxFuture<'static, Result<Value, String>>;
    fn daemon_hash(&self, entry: &Self::Entry, root: String, paths: Vec<String>) -> LocalBoxFuture<'static, Result<Value, String>>;
    fn agents_server(&self, target: ServersTarget, agent: &str, name: &str) -> LocalBoxFuture<'static, Result<ReadServer, ServerReadError>>;
    /// This computer's agents reader; `None` when the host reads no agent's MCP config.
    fn agents_reader_server(&self, projects: Vec<HereProject>, agent: &str, name: &str) -> Option<LocalBoxFuture<'static, Result<McpServerSpec, String>>>;
    fn stdio_road(&self, entry: &Self::Entry, min_version: Option<i64>) -> LocalBoxFuture<'static, Result<Rc<dyn StdioRoad>, String>>;
    fn place_name_here(&self) -> String;
    fn agent_label(&self, agent: &str) -> String;
    fn rows_on(&self, thread: &str) -> Vec<SessionView>;
    /// The workspace's transcript events, as JSON.
    fn transcript(&self, workspace: &str) -> Vec<Value>;
    fn usage_account_key(&self, entry: &Self::Entry, harness: &str) -> String;
    fn usage_accounts(&self) -> LocalBoxFuture<'static, Vec<AccountRow>>;
    fn project_name(&self, project: &str) -> String;
    fn accrued(&self, workspace: &str) -> Option<f64>;
    fn now(&self) -> i64;
    fn start_session(&self, start: DeliverStart) -> LocalBoxFuture<'static, Result<Started, String>>;
    /// Calls `f` on the `session.queued` event with this request id; the answer stops listening.
    fn on_queued(&self, request_id: &str, f: Box<dyn Fn()>) -> Box<dyn FnOnce()>;
    fn warn(&self, line: &str);
    fn poll_pull_request(&self, entry: &Self::Entry);
    fn local_env(&self) -> Option<IndexMap<String, String>>;
    fn process_env(&self) -> IndexMap<String, String>;
    /// None without a state path.
    fn state_folder(&self) -> Option<String>;
    /// Calls `f` with the workspace id of every `workspace.napped`.
    fn on_napped(&self, f: Box<dyn Fn(&str)>);
}

/// What a thread's slate runs in (slates.ts:28-53), in the TS's key order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlateThread {
    pub workspace_id: String,
    pub root_thread_id: String,
    pub session_id: String,
    pub turn_id: Option<String>,
    pub folder: Option<String>,
    pub host_folder: Option<String>,
    pub computer: Option<String>,
    pub tool_asks: ToolAsks,
}

/// What a press answered (slates.ts:145).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delivered {
    Queued,
    Started { outcome: String, turn_id: Option<String> },
}

/// slates.ts:14 slatesArea(ctx), less the `lazySlates` call, which the slates join makes from these methods.
pub struct SlatesWiring<C: SlatesAreaCtx> {
    ctx: Rc<C>,
}

// slates.ts:14 slatesArea(ctx)
/// Registers the nap listener (spec rule 14) with `napped`, slates-a's `slates.napped`.
pub fn slates_area<C: SlatesAreaCtx + 'static>(ctx: Rc<C>, napped: Rc<dyn Fn(&str)>) -> SlatesWiring<C> {
    todo!()
}

impl<C: SlatesAreaCtx + 'static> SlatesWiring<C> {
    /// slates.ts:16.
    pub fn harness_of(&self, thread: &str) -> Option<String> {
        todo!()
    }

    /// slates.ts:18.
    pub fn servers_target(&self, entry: &C::Entry) -> ServersTarget {
        todo!()
    }

    /// slates.ts:22.
    pub fn machine_on_box(&self, entry: &C::Entry) -> bool {
        todo!()
    }

    /// spec rule 4.
    pub fn thread(&self, thread: &str) -> Option<SlateThread> {
        todo!()
    }

    pub fn machine_of(&self, thread: &str) -> Option<Arc<dyn Machine>> {
        todo!()
    }

    pub fn image_on(&self, thread: &str) -> Option<ImageOn> {
        todo!()
    }

    pub fn hash_on(&self, thread: &str) -> Option<HashOn> {
        todo!()
    }

    pub fn asleep(&self, thread: &str) -> bool {
        todo!()
    }

    pub async fn wake(&self, thread: &str) -> Result<(), String> {
        todo!()
    }

    pub async fn loaded(&self) {
        todo!()
    }

    /// spec rule 6.
    pub async fn settled(&self, thread: &str) {
        todo!()
    }

    pub fn under(&self, lead: &str) -> Vec<String> {
        todo!()
    }

    pub fn thread_of_token(&self, token: &str) -> Option<String> {
        todo!()
    }

    pub fn own_process(&self, thread: &str, pid: u32, kind: &str) -> Box<dyn FnOnce()> {
        todo!()
    }

    /// spec rule 7.
    pub async fn mcp_server(&self, thread: &str, name: &str) -> Result<McpServerSpec, String> {
        todo!()
    }

    /// spec rule 8.
    pub fn away(&self, thread: &str) -> Option<Away> {
        todo!()
    }

    /// spec rule 9.
    pub async fn missing_on(&self, thread: &str, servers: Vec<String>) -> MissingOn {
        todo!()
    }

    /// spec rule 10.
    pub fn sources(&self, thread: &str, workspace: &str) -> Rc<dyn SlateSourceContext> {
        todo!()
    }

    /// spec rule 11.
    pub async fn deliver(&self, start: DeliverStart) -> Result<Delivered, String> {
        todo!()
    }

    pub fn watch_pr(&self, workspace: &str) {
        todo!()
    }

    pub fn run_env(&self) -> IndexMap<String, String> {
        todo!()
    }

    /// `<state folder>/slates.secrets.json`; none without a state path.
    pub fn secrets_file(&self) -> Option<String> {
        todo!()
    }

    /// `<state folder>/slates`; none without a state path.
    pub fn slates_dir(&self) -> Option<String> {
        todo!()
    }
}

/// The hash read a box thread's daemon gave, parsed (slates.ts:66).
pub fn parse_hash_reply(reply: Value) -> Result<HashRead, String> {
    todo!()
}

/// `SETTLE_MS` (protocol general-prefs.ts:38): none for `never` or an unknown key.
pub fn settle_ms(key: &str) -> Option<u64> {
    todo!()
}

/// The results of this thread's ended turns in a transcript (slates.ts:122).
pub fn thread_results(transcript: &[Value], thread: &str) -> Vec<TurnResult> {
    todo!()
}
