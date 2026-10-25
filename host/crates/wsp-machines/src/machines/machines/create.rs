// SPDX-License-Identifier: AGPL-3.0-only
//! machines.ts:15-362 at 65f6b89c7: the road to a new machine. Spec: spec/machines_create.md.

use std::rc::Rc;
use std::sync::Arc;

use wsp_core::types::internal::PendingCreate;
use wsp_core::types::wiring::{WorkspaceRecord, WorkspaceSpec};
use wsp_engine::machine::{Machine, MachineBackend, Waiting};
use wsp_engine::orphans::{ReapFailure, ReapedMachine};
use wsp_wire::wire::machine_link::MachineSpec;
use wsp_words::exit::HostError;

use super::{Entry, MachinesArea, Report};

/// What a fork's spec is built from beside the record: the machine kind the image was sealed as, the image's own spec
/// (`goldenImage(...).spec`, absent on a computer that keeps no image), the override a rebuild names, and the image's
/// `npmBin` (machines.ts:23).
pub struct ForkSpecIn<'a> {
    pub kind: wsp_wire::views::golden_image::MachineKind,
    pub image: Option<&'a MachineSpec>,
    pub over: Option<&'a WorkspaceSpec>,
    pub npm_bin: Option<&'a str>,
}

/// A create handle `claiming` gives its run: the backend's own calls, and a `create` that keys, holds and claims.
pub struct Claimed {
    pub(crate) area: Rc<MachinesArea>,
    pub(crate) at: Arc<dyn MachineBackend>,
    pub(crate) purpose: String,
    pub(crate) place: Option<String>,
    pub(crate) mine: std::cell::RefCell<Vec<String>>,
}

impl Claimed {
    /// The backend this handle stands in front of, for every call but `create` (machines.ts:187-200).
    pub fn backend(&self) -> Arc<dyn MachineBackend> {
        todo!()
    }

    /// machines.ts:201: holds the purpose, runs `keyed_create`, claims every box a waiting line names and the machine
    /// answered into `inflight`, each once.
    pub async fn create(&self, spec: MachineSpec, waiting: Option<Waiting>) -> Result<Arc<dyn Machine>, HostError> {
        todo!()
    }
}

impl MachinesArea {
    /// `forkSpec` (machines.ts:23): keys and precedence by spec rule 1; `createdAt` is the clock's now as ISO.
    pub fn fork_spec(&self, record: &WorkspaceRecord, o: ForkSpecIn<'_>) -> MachineSpec {
        todo!()
    }

    /// `imageMark` (machines.ts:41).
    pub fn image_mark(&self) -> String {
        todo!()
    }

    /// `hold` (machines.ts:45): waits while the purpose is held, re-checking after each wake (not FIFO), then holds it.
    /// The answer releases it.
    pub async fn hold(self: &Rc<Self>, purpose: &str) -> Box<dyn FnOnce()> {
        todo!()
    }

    /// `keyedCreate` (machines.ts:61), spec rule 4.
    pub async fn keyed_create(
        self: &Rc<Self>,
        at: &Arc<dyn MachineBackend>,
        purpose: &str,
        spec: MachineSpec,
        waiting: Option<Waiting>,
        after_corpse: bool,
        place: Option<&str>,
    ) -> Result<Arc<dyn Machine>, HostError> {
        todo!()
    }

    /// `replayCreates` (machines.ts:106), spec rule 5. `known` is read once per machine found.
    pub async fn replay_creates(
        self: &Rc<Self>,
        known: &dyn Fn() -> Vec<String>,
        failed: &mut Vec<ReapFailure>,
    ) -> Result<Vec<ReapedMachine>, HostError> {
        todo!()
    }

    /// `sweepStrays` (machines.ts:159), spec rule 6.
    pub async fn sweep_strays(
        self: &Rc<Self>,
        known: &dyn Fn() -> Vec<String>,
        failed: &mut Vec<ReapFailure>,
        at: &[Arc<dyn MachineBackend>],
    ) -> Result<Vec<ReapedMachine>, HostError> {
        todo!()
    }

    /// `claiming` (machines.ts:185): `at` is `backend()` when `None`. Every id the handle claimed leaves `inflight`
    /// once `run` settles, either way.
    pub async fn claiming<T, F>(self: &Rc<Self>, purpose: &str, run: F, at: Option<Arc<dyn MachineBackend>>, place: Option<&str>) -> Result<T, HostError>
    where
        F: for<'c> FnOnce(&'c Claimed) -> futures::future::LocalBoxFuture<'c, Result<T, HostError>>,
    {
        todo!()
    }

    /// `observed` (machines.ts:232): the machine with `exec` and `run` reported to the recipe's listener.
    pub fn observed(&self, machine: Arc<dyn Machine>) -> Arc<dyn Machine> {
        todo!()
    }

    /// `observing` (machines.ts:302): `create` and `get` answer observed machines.
    pub fn observing(self: &Rc<Self>, backend: Arc<dyn MachineBackend>) -> Arc<dyn MachineBackend> {
        todo!()
    }

    /// `fork` (machines.ts:307), spec rule 10. `bind` runs before the claim is released.
    pub async fn fork(
        self: &Rc<Self>,
        record: &mut WorkspaceRecord,
        bind: &dyn Fn(&Arc<dyn Machine>, &WorkspaceRecord),
        over: Option<&WorkspaceSpec>,
        report: Option<Report>,
        reads_memory: bool,
    ) -> Result<Arc<dyn Machine>, HostError> {
        todo!()
    }

    /// `unfork` (machines.ts:356), spec rule 11.
    pub async fn unfork(self: &Rc<Self>, entry: &Entry) -> Result<(), HostError> {
        todo!()
    }
}

/// The attempt a create puts under its purpose (wsp-core's `PendingCreate`), re-exported for the tests.
pub type Attempt = PendingCreate;
