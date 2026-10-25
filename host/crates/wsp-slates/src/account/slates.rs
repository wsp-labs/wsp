// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/account/slates.ts at 65f6b89c7. Spec: spec/account_slates.md.
//!
//! The slate area's wiring: what `createSlates` (slates-a's) is handed, each answer read off the runtime per call.
//! Everything it reads goes through `SlatesAreaCtx` (ruling 6401022701 rule 4), implemented on wsp-core's context
//! handle at the join and by a fake in the tests.
#![allow(unused_variables)]

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::Value;

use wsp_core::clock::Clock;
use wsp_core::types::internal::ThreadRecord;
use wsp_core::types::wiring::Detach;
use wsp_engine::machine::Machine;
use wsp_engine::target_line::TargetLogin;
use wsp_store::store::Store;
use wsp_wire::types::agents_report::{AgentsProject, AgentsTarget};
use wsp_frames::ProcKind;
use wsp_wire::types::slate_wire::SessionSlateEvent;
use wsp_words::exit::HostError;

pub use wsp_machines::machines::machines::Entry;

use crate::slate_files::{HashOn, HashRead};
use crate::slate_images::ImageOn;
use crate::slate_mcp::{Away, McpServerSpec, MissingOnFn, ToolAsks};
use crate::slate_mcp_on::StdioRoad;
use crate::slate_sources::context::{AccountRow, SessionView, TurnResult};
use crate::slate_sources::SlateSourceContext;
use crate::slates::{Delivered, Delivery, SlateEmit, SlateThreadFacts, SlatesDeps};

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
    fn folder_of(&self, workspace: &str, session: &str) -> Option<String>;
    fn runs_in(&self, entry: &Entry, ran_in: Option<&str>, fallback: &str) -> String;
    /// The kind module's `folder(record)`.
    fn kind_folder(&self, entry: &Entry) -> Option<String>;
    fn checkout_of(&self, entry: &Entry) -> String;
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
    fn daemon_image(&self, entry: &Entry, path: String) -> LocalBoxFuture<'static, Result<Value, String>>;
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
    fn project_name(&self, project: &str) -> String;
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

// slates.ts:14 slatesArea(ctx)
/// Registers the nap listener (spec rule 14) with `napped`, the slate area's `napped`.
pub fn slates_area<C: SlatesAreaCtx + 'static>(ctx: Rc<C>, napped: Rc<dyn Fn(&str)>) -> SlatesWiring<C> {
    todo!()
}

impl<C: SlatesAreaCtx + 'static> SlatesWiring<C> {
    /// slates.ts:16.
    pub fn harness_of(&self, thread: &str) -> Option<String> {
        todo!()
    }

    /// slates.ts:18.
    pub fn servers_target(&self, entry: &Entry) -> AgentsTarget {
        todo!()
    }

    /// slates.ts:22.
    pub fn machine_on_box(&self, entry: &Entry) -> bool {
        todo!()
    }
}

/// What the wiring answers `createSlates`: spec rules 4 to 14. `store`, `clock`, `record` and `emit` are the
/// context's own; `runs` is left to the area's default road.
#[async_trait(?Send)]
impl<C: SlatesAreaCtx + 'static> SlatesDeps for SlatesWiring<C> {
    fn store(&self) -> Rc<dyn Store> {
        todo!()
    }

    fn clock(&self) -> Rc<dyn Clock> {
        todo!()
    }

    fn record(&self, event: SessionSlateEvent) {
        todo!()
    }

    fn emit(&self, event: SlateEmit) {
        todo!()
    }

    /// spec rule 4.
    fn thread(&self, thread_id: &str) -> Option<SlateThreadFacts> {
        todo!()
    }

    async fn loaded(&self) -> Result<(), HostError> {
        todo!()
    }

    fn machine_of(&self, thread_id: &str) -> Option<Arc<dyn Machine>> {
        todo!()
    }

    fn image_on(&self, thread_id: &str) -> Option<ImageOn> {
        todo!()
    }

    fn hash_on(&self, thread_id: &str) -> Option<HashOn> {
        todo!()
    }

    fn asleep(&self, thread_id: &str) -> bool {
        todo!()
    }

    async fn wake(&self, thread_id: &str) -> Result<(), HostError> {
        todo!()
    }

    /// spec rule 6.
    async fn settled(&self, thread_id: &str) -> Option<bool> {
        todo!()
    }

    fn under(&self, lead: &str) -> Vec<String> {
        todo!()
    }

    fn thread_of_token(&self, token: &str) -> Result<String, HostError> {
        todo!()
    }

    /// spec rule 10.
    fn sources(&self, thread_id: &str, workspace_id: &str) -> Rc<dyn SlateSourceContext> {
        todo!()
    }

    /// spec rule 11.
    async fn deliver(&self, o: Delivery) -> Result<Delivered, HostError> {
        todo!()
    }

    fn watch_pr(&self, workspace_id: &str) {
        todo!()
    }

    fn run_env(&self) -> IndexMap<String, String> {
        todo!()
    }

    /// `<state folder>/slates.secrets.json`; none without a state path.
    fn secrets_file(&self) -> Option<PathBuf> {
        todo!()
    }

    /// `<state folder>/slates`; none without a state path.
    fn slates_dir(&self) -> Option<PathBuf> {
        todo!()
    }

    /// spec rule 7.
    async fn mcp_server(&self, thread_id: &str, name: &str) -> Option<Result<McpServerSpec, HostError>> {
        todo!()
    }

    fn own_process(&self, thread_id: &str, pid: u32, kind: &str) -> Option<Rc<dyn Fn()>> {
        todo!()
    }

    /// spec rule 8.
    fn away(&self, thread_id: &str) -> Option<Away> {
        todo!()
    }

    /// spec rule 9.
    fn missing_on(&self, thread_id: &str) -> Option<MissingOnFn> {
        todo!()
    }
}

/// The hash read a box thread's daemon gave, parsed (slates.ts:66).
pub fn parse_hash_reply(reply: Value) -> Result<HashRead, String> {
    todo!()
}

/// `SETTLE_MS` (protocol general-prefs.ts:38): none for `never` or an unknown key.
pub fn settle_ms(key: &str) -> Option<f64> {
    todo!()
}

/// The results of this thread's ended turns in a transcript (slates.ts:122).
pub fn thread_results(transcript: &[Value], thread: &str) -> Vec<TurnResult> {
    todo!()
}
