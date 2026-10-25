// SPDX-License-Identifier: AGPL-3.0-only
//! `opencode run --format json` read into the runtime's events. TS adapter-opencode/src/adapter.ts. Spec:
//! spec/opencode.md rules 5-14.

use std::sync::{Arc, Mutex};

use futures::future::BoxFuture;
use indexmap::IndexMap;

use crate::port::{AdapterAttachOptions, AdapterError, AgentLaunch, Env, ExecFailure, ExecStreamFactory, HarnessCatalogAnswer, HarnessExec, HarnessStartOptions, Now, SharedFuture, TurnResult};

/// TS adapter.ts:41.
#[derive(Clone)]
pub struct OpenCodeAdapterDeps {
    pub exec: Arc<dyn ExecStreamFactory>,
    pub base_env: Option<IndexMap<String, Option<String>>>,
    /// wsp's half of a turn OpenCode refuses for want of a sign-in.
    pub sign_in_refusal: Option<String>,
    pub api_key: Option<String>,
    pub key_env: Option<String>,
    /// INTERRUPT_GRACE_MS (5000) where `None`.
    pub interrupt_grace_ms: Option<u64>,
    /// RUN_EXIT_MS (10000) where `None`.
    pub result_exit_ms: Option<u64>,
    pub launch: Option<AgentLaunch>,
    /// The system clock where `None`.
    pub now: Option<Now>,
}

impl OpenCodeAdapterDeps {
    /// Deps with every optional field `None`.
    pub fn new(exec: Arc<dyn ExecStreamFactory>) -> Self {
        OpenCodeAdapterDeps { exec, base_env: None, sign_in_refusal: None, api_key: None, key_env: None, interrupt_grace_ms: None, result_exit_ms: None, launch: None, now: None }
    }
}

/// What a session does. Not part of the port.
pub trait OpenCodeCore: Send + Sync {
    fn local_id(&self) -> String;
    fn session_id(&self) -> String;
    fn command(&self) -> Option<String>;
    fn run(&self) -> Option<String>;
    fn pid(&self) -> Option<u32>;
    fn finished(&self) -> SharedFuture<TurnResult>;
    fn interrupt(&self) -> BoxFuture<'static, ()>;
}

/// TS adapter.ts:30.
#[derive(Clone)]
pub struct OpenCodeSession {
    pub core: Arc<dyn OpenCodeCore>,
}

impl OpenCodeSession {
    /// The resume id, or one minted at start.
    pub fn local_id(&self) -> String {
        todo!()
    }
    /// The session id the first event named, the local id until then.
    pub fn session_id(&self) -> String {
        todo!()
    }
    pub fn command(&self) -> Option<String> {
        todo!()
    }
    pub fn run(&self) -> Option<String> {
        todo!()
    }
    pub fn pid(&self) -> Option<u32> {
        todo!()
    }
    pub fn finished(&self) -> SharedFuture<TurnResult> {
        todo!()
    }
    /// TS adapter.ts:238.
    pub fn interrupt(&self) -> BoxFuture<'static, ()> {
        todo!()
    }
}

/// What attach found.
pub enum AttachedSession {
    Session(OpenCodeSession),
    Gone,
}

/// TS adapter.ts:54. Fields are the implementer's to extend.
#[derive(Clone)]
pub struct OpenCodeAdapter {
    pub deps: OpenCodeAdapterDeps,
    pub env: Env,
    pub sessions: Arc<Mutex<IndexMap<String, OpenCodeSession>>>,
}

/// TS adapter.ts:124.
pub fn create_opencode_adapter(deps: OpenCodeAdapterDeps) -> OpenCodeAdapter {
    todo!()
}

impl OpenCodeAdapter {
    /// `run` reads its message and closes stdin: no steer. TS adapter.ts:298.
    pub fn steers(&self) -> bool {
        todo!()
    }
    /// `-f` reads each image off the machine. TS adapter.ts:299.
    pub fn attachments(&self) -> Option<crate::port::AttachmentRoad> {
        todo!()
    }
    /// Takes no MCP servers with a launch.
    pub fn mcp_servers(&self) -> bool {
        todo!()
    }
    /// TS adapter.ts:252. `Err` where the TS throws before anything runs.
    pub fn start(&self, options: HarnessStartOptions) -> Result<OpenCodeSession, AdapterError> {
        todo!()
    }
    /// TS adapter.ts:282. `None` where the exec factory has no attach.
    pub fn attach(&self, options: AdapterAttachOptions) -> Option<BoxFuture<'static, Result<AttachedSession, ExecFailure>>> {
        todo!()
    }
    pub fn sessions(&self) -> IndexMap<String, OpenCodeSession> {
        todo!()
    }
    pub fn env(&self) -> Env {
        todo!()
    }
    /// TS adapter.ts:300.
    pub async fn probe_catalog(&self, exec: Arc<dyn HarnessExec>) -> Result<Option<HarnessCatalogAnswer>, ExecFailure> {
        todo!()
    }
    /// TS adapter.ts:301.
    pub async fn probe_version(&self, exec: Arc<dyn HarnessExec>) -> Result<Option<String>, ExecFailure> {
        todo!()
    }
}
