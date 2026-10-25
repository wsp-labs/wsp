// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/lifecycle.ts. Spec: spec/lifecycle.md.
//!
//! One workspace's machine through naps, wakes, rebuilds and upgrades. Each TS hook is an optional closure; absent
//! is the TS's `undefined`, and the fallback the TS takes for a missing hook is the spec's.
use std::sync::Arc;

use futures::future::BoxFuture;
use indexmap::IndexMap;

use crate::errors::Result;
use crate::machine::{Machine, MachineSpec, PreviewReach};

/// A machine handle as the workspace holds it.
pub type MachineRef = Arc<dyn Machine>;

/// lifecycle.ts:33. The two provider calls a backend settles on its own budgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderMove {
    Pause,
    Resume,
}

/// lifecycle.ts:35.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkspacePhase {
    Running,
    Napping,
    Waking,
}

/// A partial `MachineSpec` (`Partial<MachineSpec>`): the fields a rebuild or upgrade overrides; `None` keeps the
/// resurrect hook's own.
pub type SpecOverride = Option<MachineSpec>;

pub type ResurrectHook = Arc<dyn Fn(SpecOverride) -> BoxFuture<'static, Result<MachineRef>> + Send + Sync>;
/// `Ok(None)` is an export over the cap: the replacement goes on with no backup.
pub type VaultExportHook = Arc<dyn Fn(MachineRef) -> BoxFuture<'static, Result<Option<Vec<u8>>>> + Send + Sync>;
pub type VaultImportHook = Arc<dyn Fn(MachineRef, Vec<u8>) -> BoxFuture<'static, Result<()>> + Send + Sync>;
pub type MachineHook = Arc<dyn Fn(MachineRef) -> BoxFuture<'static, Result<()>> + Send + Sync>;
/// `Ok(None)` is healthy; `Ok(Some(fault))` names the fault.
pub type WakeCheckHook = Arc<dyn Fn(MachineRef) -> BoxFuture<'static, Result<Option<String>>> + Send + Sync>;
pub type MoveHook = Arc<dyn Fn(MachineRef, ProviderMove) -> BoxFuture<'static, Result<()>> + Send + Sync>;

/// lifecycle.ts:6.
#[derive(Clone)]
pub struct WorkspaceHooks {
    pub golden_snapshot: String,
    /// How many times a wake may resume and check before it fails: the backend's number.
    pub wake_attempts: u32,
    pub resurrect: Option<ResurrectHook>,
    pub vault_export: Option<VaultExportHook>,
    pub vault_import: Option<VaultImportHook>,
    /// Runs before every pause.
    pub stash_vault: Option<MachineHook>,
    /// Puts the last stashed vault onto a replacement.
    pub restore_vault: Option<MachineHook>,
    pub wake_check: Option<WakeCheckHook>,
    /// Absent: the machine's own `pause(None)` / `resume(None)` is awaited.
    pub move_hook: Option<MoveHook>,
    /// Absent: the machine's own `kill()` is awaited.
    pub retire: Option<MachineHook>,
}

/// lifecycle.ts:37.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WakeResult {
    /// Present when the wake did not go straight through: every fault met, in order, joined by "; ".
    pub reason: Option<String>,
}

/// lifecycle.ts:52 `initial`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkspaceInitial {
    /// `None` reads running.
    pub phase: Option<WorkspacePhase>,
    /// `None` reads true.
    pub first_life: Option<bool>,
}

/// lifecycle.ts:42. Every method takes `&mut self`: the TS object is driven by one caller at a time.
pub struct Workspace {
    machine: MachineRef,
    hooks: WorkspaceHooks,
    /// Shared so a reader outside the workspace (a hook, the runtime's status) sees the phase while a wake runs.
    phase: Arc<std::sync::Mutex<WorkspacePhase>>,
    first_life: bool,
    /// The routes minted per port, and the machine id they were minted under.
    preview_machine_id: String,
    preview_by_port: IndexMap<u16, PreviewReach>,
}

impl Workspace {
    /// lifecycle.ts:51.
    pub fn new(machine: MachineRef, hooks: WorkspaceHooks, initial: WorkspaceInitial) -> Self {
        todo!()
    }

    pub fn machine(&self) -> &MachineRef {
        &self.machine
    }

    /// lifecycle.ts:61.
    pub fn machine_id(&self) -> String {
        todo!()
    }

    /// lifecycle.ts:65.
    pub fn current_phase(&self) -> WorkspacePhase {
        todo!()
    }

    /// The phase as a cell other readers hold: it reads waking while a wake's check runs.
    pub fn phase_cell(&self) -> Arc<std::sync::Mutex<WorkspacePhase>> {
        self.phase.clone()
    }

    /// lifecycle.ts:69.
    pub fn is_first_life(&self) -> bool {
        todo!()
    }

    /// lifecycle.ts:73.
    pub fn golden_snapshot(&self) -> &str {
        todo!()
    }

    /// lifecycle.ts:78. `port_reach(DAEMON_PORT)`.
    pub async fn daemon_reach(&mut self) -> Result<PreviewReach> {
        todo!()
    }

    /// lifecycle.ts:84. Through preview.rs's `refresh_preview_token(machine, port, cached)`.
    pub async fn port_reach(&mut self, port: u16) -> Result<PreviewReach> {
        todo!()
    }

    /// lifecycle.ts:93.
    pub async fn remint_port_reach(&mut self, port: u16) -> Result<PreviewReach> {
        todo!()
    }

    /// lifecycle.ts:98.
    pub async fn nap(&mut self) -> Result<()> {
        todo!()
    }

    /// lifecycle.ts:111.
    pub fn note_paused(&mut self) {
        todo!()
    }

    /// lifecycle.ts:117.
    pub fn note_running(&mut self) {
        todo!()
    }

    /// lifecycle.ts:130. `landed`: a resume already sent that the provider took without its call answering.
    pub async fn wake(&mut self, landed: bool) -> Result<WakeResult> {
        todo!()
    }

    /// lifecycle.ts:167.
    pub async fn rebuild(&mut self) -> Result<()> {
        todo!()
    }

    /// lifecycle.ts:191.
    pub async fn checkpoint(&mut self, name: &str) -> Result<String> {
        todo!()
    }

    /// lifecycle.ts:196.
    pub async fn upgrade(&mut self, spec: SpecOverride) -> Result<()> {
        todo!()
    }
}
