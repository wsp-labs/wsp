// SPDX-License-Identifier: AGPL-3.0-only
//! `cursor-agent -p --output-format stream-json` read into the runtime's events. TS adapter-cursor/src/adapter.ts. Spec:
//! spec/cursor.md rules 4-13.

use std::sync::{Arc, Mutex};

use futures::future::BoxFuture;
use indexmap::IndexMap;

use crate::port::{AdapterAttachOptions, AdapterError, AgentLaunch, Env, ExecFailure, ExecStreamFactory, HarnessStartOptions, SharedFuture, TurnResult};

/// TS adapter.ts:36.
#[derive(Clone)]
pub struct CursorAdapterDeps {
    pub exec: Arc<dyn ExecStreamFactory>,
    pub base_env: Option<IndexMap<String, Option<String>>>,
    /// wsp's half of a turn the CLI refuses for want of a credential.
    pub sign_in_refusal: Option<String>,
    pub api_key: Option<String>,
    pub key_env: Option<String>,
    /// INTERRUPT_GRACE_MS (5000) where `None`.
    pub interrupt_grace_ms: Option<u64>,
    /// RUN_EXIT_MS (10000) where `None`.
    pub result_exit_ms: Option<u64>,
    pub launch: Option<AgentLaunch>,
}

impl CursorAdapterDeps {
    /// Deps with every optional field `None`.
    pub fn new(exec: Arc<dyn ExecStreamFactory>) -> Self {
        CursorAdapterDeps { exec, base_env: None, sign_in_refusal: None, api_key: None, key_env: None, interrupt_grace_ms: None, result_exit_ms: None, launch: None }
    }
}

/// What a session does. Not part of the port.
pub trait CursorCore: Send + Sync {
    fn local_id(&self) -> String;
    fn session_id(&self) -> String;
    fn command(&self) -> Option<String>;
    fn run(&self) -> Option<String>;
    fn pid(&self) -> Option<u32>;
    fn finished(&self) -> SharedFuture<TurnResult>;
    fn interrupt(&self) -> BoxFuture<'static, ()>;
}

/// TS adapter.ts:25.
#[derive(Clone)]
pub struct CursorSession {
    pub core: Arc<dyn CursorCore>,
}

impl CursorSession {
    /// The resume id, or one minted at start.
    pub fn local_id(&self) -> String {
        todo!()
    }
    /// The chat id init named, the local id until then.
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
    /// TS adapter.ts:210.
    pub fn interrupt(&self) -> BoxFuture<'static, ()> {
        todo!()
    }
}

/// What attach found.
pub enum AttachedSession {
    Session(CursorSession),
    Gone,
}

/// TS adapter.ts:51. Fields are the implementer's to extend.
#[derive(Clone)]
pub struct CursorAdapter {
    pub deps: CursorAdapterDeps,
    pub env: Env,
    pub sessions: Arc<Mutex<IndexMap<String, CursorSession>>>,
}

/// TS adapter.ts:111.
pub fn create_cursor_adapter(deps: CursorAdapterDeps) -> CursorAdapter {
    todo!()
}

impl CursorAdapter {
    /// No control channel: no steer. TS adapter.ts:261.
    pub fn steers(&self) -> bool {
        todo!()
    }
    /// No image flag is documented, so none. TS adapter.ts:49.
    pub fn attachments(&self) -> Option<crate::port::AttachmentRoad> {
        todo!()
    }
    /// Takes no MCP servers with a launch.
    pub fn mcp_servers(&self) -> bool {
        todo!()
    }
    /// TS adapter.ts:219. `Err` where the TS throws before anything runs.
    pub fn start(&self, options: HarnessStartOptions) -> Result<CursorSession, AdapterError> {
        todo!()
    }
    /// TS adapter.ts:246. `None` where the exec factory has no attach.
    pub fn attach(&self, options: AdapterAttachOptions) -> Option<BoxFuture<'static, Result<AttachedSession, ExecFailure>>> {
        todo!()
    }
    pub fn sessions(&self) -> IndexMap<String, CursorSession> {
        todo!()
    }
    pub fn env(&self) -> Env {
        todo!()
    }
}
