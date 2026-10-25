// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/lifecycle.ts. Spec: spec/lifecycle.md.
//!
//! One workspace's machine through naps, wakes, rebuilds and upgrades. Each TS hook is an optional closure; absent
//! is the TS's `undefined`, and the fallback the TS takes for a missing hook is the spec's.
use std::cell::{Cell, RefCell};
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;

use wsp_wire::wire::machine_link::MachineLife;
use wsp_words::words::computer::wake_failed_line;

use crate::errors::{is_missing, EngineError, RealRetryClock, Result, RetryClock};
use crate::machine::{Machine, MachineSpec, PreviewReach, SnapshotOptions};
use crate::preview::{refresh_preview_token, DAEMON_PORT};

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

pub type ResurrectHook = Arc<dyn Fn(SpecOverride) -> LocalBoxFuture<'static, Result<MachineRef>>>;
/// `Ok(None)` is an export over the cap: the replacement goes on with no backup.
pub type VaultExportHook = Arc<dyn Fn(MachineRef) -> LocalBoxFuture<'static, Result<Option<Vec<u8>>>>>;
pub type VaultImportHook = Arc<dyn Fn(MachineRef, Vec<u8>) -> LocalBoxFuture<'static, Result<()>>>;
pub type MachineHook = Arc<dyn Fn(MachineRef) -> LocalBoxFuture<'static, Result<()>>>;
/// `Ok(None)` is healthy; `Ok(Some(fault))` names the fault.
pub type WakeCheckHook = Arc<dyn Fn(MachineRef) -> LocalBoxFuture<'static, Result<Option<String>>>>;
pub type MoveHook = Arc<dyn Fn(MachineRef, ProviderMove) -> LocalBoxFuture<'static, Result<()>>>;

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

/// lifecycle.ts:42. One object shared by everything that holds the workspace (wsp-core keeps it as `Rc<Workspace>`),
/// as the TS shares one. Every method takes `&self` and the state sits in cells borrowed only between awaits (rule
/// 3.3), so a hook can read the workspace while a wake runs.
pub struct Workspace {
    machine: RefCell<MachineRef>,
    hooks: WorkspaceHooks,
    /// Shared so a reader outside the workspace (a hook, the runtime's status) sees the phase while a wake runs.
    phase: Arc<std::sync::Mutex<WorkspacePhase>>,
    first_life: Cell<bool>,
    /// The routes minted per port, and the machine id they were minted under.
    preview_machine_id: RefCell<String>,
    preview_by_port: RefCell<IndexMap<u16, PreviewReach>>,
}

impl Workspace {
    /// lifecycle.ts:51.
    pub fn new(machine: MachineRef, hooks: WorkspaceHooks, initial: WorkspaceInitial) -> Self {
        Workspace {
            machine: RefCell::new(machine),
            hooks,
            phase: Arc::new(std::sync::Mutex::new(initial.phase.unwrap_or(WorkspacePhase::Running))),
            first_life: Cell::new(initial.first_life.unwrap_or(true)),
            preview_machine_id: RefCell::new(String::new()),
            preview_by_port: RefCell::new(IndexMap::new()),
        }
    }

    /// The machine now held; a rebuild or an upgrade replaces it.
    pub fn machine(&self) -> MachineRef {
        self.machine.borrow().clone()
    }

    /// lifecycle.ts:61.
    pub fn machine_id(&self) -> String {
        self.machine().id().to_string()
    }

    /// lifecycle.ts:65.
    pub fn current_phase(&self) -> WorkspacePhase {
        *self.phase.lock().unwrap()
    }

    /// The phase as a cell other readers hold: it reads waking while a wake's check runs.
    pub fn phase_cell(&self) -> Arc<std::sync::Mutex<WorkspacePhase>> {
        self.phase.clone()
    }

    /// lifecycle.ts:69.
    pub fn is_first_life(&self) -> bool {
        self.first_life.get()
    }

    /// lifecycle.ts:73.
    pub fn golden_snapshot(&self) -> &str {
        &self.hooks.golden_snapshot
    }

    /// lifecycle.ts:78. `port_reach(DAEMON_PORT)`.
    pub async fn daemon_reach(&self) -> Result<PreviewReach> {
        self.port_reach(DAEMON_PORT).await
    }

    /// lifecycle.ts:84. Through preview.rs's `refresh_preview_token(machine, port, cached)`.
    pub async fn port_reach(&self, port: u16) -> Result<PreviewReach> {
        let machine = self.machine();
        if *self.preview_machine_id.borrow() != machine.id() {
            *self.preview_machine_id.borrow_mut() = machine.id().to_string();
            self.preview_by_port.borrow_mut().clear();
        }
        let cached = self.preview_by_port.borrow().get(&port).cloned();
        let reach = refresh_preview_token(&*machine, port, cached.as_ref(), RealRetryClock.now()).await?;
        self.preview_by_port.borrow_mut().insert(port, reach.clone());
        Ok(reach)
    }

    /// lifecycle.ts:93.
    pub async fn remint_port_reach(&self, port: u16) -> Result<PreviewReach> {
        if *self.preview_machine_id.borrow() == self.machine().id() {
            self.preview_by_port.borrow_mut().shift_remove(&port);
        }
        self.port_reach(port).await
    }

    /// lifecycle.ts:98.
    pub async fn nap(&self) -> Result<()> {
        if self.current_phase() == WorkspacePhase::Napping {
            return Ok(());
        }
        if let Some(stash) = &self.hooks.stash_vault {
            stash(self.machine()).await?;
        }
        self.move_machine(ProviderMove::Pause).await?;
        self.set_phase(WorkspacePhase::Napping);
        Ok(())
    }

    /// lifecycle.ts:111.
    pub fn note_paused(&self) {
        self.set_phase(WorkspacePhase::Napping);
    }

    /// lifecycle.ts:117.
    pub fn note_running(&self) {
        self.set_phase(WorkspacePhase::Running);
    }

    /// lifecycle.ts:130. `landed`: a resume already sent that the provider took without its call answering.
    pub async fn wake(&self, landed: bool) -> Result<WakeResult> {
        if self.current_phase() == WorkspacePhase::Running {
            return Ok(WakeResult { reason: None });
        }
        self.set_phase(WorkspacePhase::Waking);
        let answer = self.wake_attempts(landed).await;
        if answer.is_err() {
            self.set_phase(WorkspacePhase::Napping);
        }
        answer
    }

    async fn wake_attempts(&self, landed: bool) -> Result<WakeResult> {
        let mut faults: Vec<String> = Vec::new();
        for attempt in 1..=self.hooks.wake_attempts {
            if attempt > 1 || !landed {
                self.move_machine(ProviderMove::Resume).await?;
            }
            self.preview_by_port.borrow_mut().clear();
            self.first_life.set(false);
            let fault = match &self.hooks.wake_check {
                Some(check) => check(self.machine()).await?,
                None => None,
            };
            let Some(fault) = fault else {
                self.set_phase(WorkspacePhase::Running);
                return Ok(WakeResult { reason: (!faults.is_empty()).then(|| faults.join("; ")) });
            };
            faults.push(format!("attempt {attempt}: {fault}"));
            if attempt < self.hooks.wake_attempts {
                if let Err(e) = self.move_machine(ProviderMove::Pause).await {
                    faults.push(format!("re-pause failed: {e}"));
                }
            }
        }
        Err(EngineError::Other { message: wake_failed_line(self.machine().id(), &faults.join("; ")), code: None })
    }

    /// lifecycle.ts:167.
    pub async fn rebuild(&self) -> Result<()> {
        let Some(resurrect) = self.hooks.resurrect.clone() else {
            return Err(EngineError::Other { message: "rebuild requires a resurrect hook".into(), code: None });
        };
        self.replace(resurrect).await?;
        self.set_phase(WorkspacePhase::Running);
        Ok(())
    }

    /// lifecycle.ts:191.
    pub async fn checkpoint(&self, name: &str) -> Result<String> {
        let life = MachineLife { first_life: self.first_life.get() };
        self.machine().snapshot(name, life, SnapshotOptions { on_progress: None }).await
    }

    /// lifecycle.ts:196.
    pub async fn upgrade(&self, spec: SpecOverride) -> Result<()> {
        let Some(resurrect) = self.hooks.resurrect.clone() else {
            return Err(EngineError::Other { message: "upgrade requires a resurrect hook".into(), code: None });
        };
        let payload = match &self.hooks.vault_export {
            Some(export) => export(self.machine()).await?,
            None => None,
        };
        self.retire(self.machine()).await?;
        let fresh = resurrect(spec).await?;
        *self.machine.borrow_mut() = fresh;
        if let (Some(payload), Some(import)) = (payload, &self.hooks.vault_import) {
            import(self.machine(), payload).await?;
        }
        self.set_phase(WorkspacePhase::Running);
        self.first_life.set(true);
        Ok(())
    }

    /// Fork first, vault second, kill last: the old machine dies only once its replacement holds the files.
    async fn replace(&self, resurrect: ResurrectHook) -> Result<()> {
        let old = self.machine();
        let fresh = resurrect(None).await?;
        *self.machine.borrow_mut() = fresh;
        self.first_life.set(true);
        if let Some(restore) = &self.hooks.restore_vault {
            restore(self.machine()).await?;
        }
        match self.retire(old).await {
            Err(e) if !is_missing(&e) => Err(e),
            _ => Ok(()),
        }
    }

    async fn move_machine(&self, move_: ProviderMove) -> Result<()> {
        let machine = self.machine();
        match &self.hooks.move_hook {
            Some(hook) => hook(machine, move_).await,
            None => match move_ {
                ProviderMove::Pause => machine.pause(None).await,
                ProviderMove::Resume => machine.resume(None).await,
            },
        }
    }

    async fn retire(&self, m: MachineRef) -> Result<()> {
        match &self.hooks.retire {
            Some(hook) => hook(m).await,
            None => m.kill().await,
        }
    }

    fn set_phase(&self, phase: WorkspacePhase) {
        *self.phase.lock().unwrap() = phase;
    }
}
