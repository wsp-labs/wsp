// SPDX-License-Identifier: AGPL-3.0-only
//! machines.ts:15-362 at 65f6b89c7: the road to a new machine. Spec: spec/machines_create.md.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashSet};
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::channel::oneshot;
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

use wsp_core::clock::{RetryClockOver, Timer};
use wsp_core::host_id::template_host;
use wsp_core::idle::IDLE_OFF_BACKSTOP_MS;
use wsp_core::types::harness::login_env_on;
use wsp_core::types::internal::{
    fingerprint, pid_alive, DroppedMachine, NamedWorkspace, PendingCreate, CREATES, DROPPED,
    KEY_PURPOSE_MAX, LOST_CREATE_MS, WORKSPACE_NAMES,
};
use wsp_core::types::pending::Clock;
use wsp_core::types::wiring::{
    set_hostname, GoldenExec, StageSaid, WorkspaceRecord, WorkspaceSpec,
};
use wsp_engine::errors::EngineError;
use wsp_engine::gone::{kill_until_gone, KillConfirm};
use wsp_engine::machine::{
    Capabilities, ExecOptions, ExecResult, Machine, MachineBackend, MachineState, ProviderBoxState,
    RunOptions, Waiting,
};
use wsp_engine::machine_context::render::{
    apply_machine_context, ApplyContextOptions, GoldenPick, WorkspaceName,
};
use wsp_engine::orphans::{date_parse, strays, ReapFailure, ReapReason, ReapedMachine, StrayRead};
use wsp_engine::snapshot_names::name_owner;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::golden_image::{golden_image, GoldenVersion, ImageSpec, MachineKind};
use wsp_wire::views::workspace_events::WorkspaceCreateStage;
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::machine_link::{place_holds_no_image_line, MachineSpec, MachineSpecOnIdle};
use wsp_words::exit::{HostError, Kind};
use wsp_words::js_date::iso_string;
use wsp_words::words::thread::{hostname_set_line, HOSTNAME_KEPT};

use super::super::area_error::{AreaError, AreaResult};
use super::{Entry, MachinesArea, MachinesRuntime, Report};

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
    pub(crate) mine: Rc<RefCell<Vec<String>>>,
}

impl Claimed {
    /// The backend this handle stands in front of, for every call but `create` (machines.ts:187-200).
    pub fn backend(&self) -> Arc<dyn MachineBackend> {
        Arc::new(Relay {
            inner: self.at.clone(),
            hook: Rc::new(ClaimHook {
                area: self.area.clone(),
                purpose: self.purpose.clone(),
                place: self.place.clone(),
                mine: self.mine.clone(),
            }),
        })
    }

    /// machines.ts:201: holds the purpose, runs `keyed_create`, claims every box a waiting line names and the machine
    /// answered into `inflight`, each once.
    pub async fn create(
        &self,
        spec: MachineSpec,
        waiting: Option<Waiting>,
    ) -> AreaResult<Arc<dyn Machine>> {
        claimed_create(
            &self.area,
            &self.at,
            &self.purpose,
            self.place.as_deref(),
            &self.mine,
            spec,
            waiting,
        )
        .await
    }
}

/// `claiming`'s create: holds the purpose, keys the create, and claims the boxes and machine into `inflight` (machines.ts:201).
async fn claimed_create(
    area: &Rc<MachinesArea>,
    at: &Arc<dyn MachineBackend>,
    purpose: &str,
    place: Option<&str>,
    mine: &Rc<RefCell<Vec<String>>>,
    spec: MachineSpec,
    waiting: Option<Waiting>,
) -> AreaResult<Arc<dyn Machine>> {
    let release = area.hold(purpose).await;
    let (claimer, claimed) = (area.clone(), mine.clone());
    let claim: Waiting = Arc::new(move |line: &str, box_: Option<&ProviderBoxState>| {
        if let Some(b) = box_ {
            claimer.claim(&claimed, &b.id);
        }
        if let Some(w) = &waiting {
            w(line, box_);
        }
    });
    let made = area
        .keyed_create(at, purpose, spec, Some(claim), false, place)
        .await;
    let out = match made {
        Ok(machine) => {
            area.claim(mine, machine.id());
            Ok(machine)
        }
        Err(e) => Err(e),
    };
    release();
    out
}

/// The backend a claim's create goes through, for `Relay`.
#[async_trait(?Send)]
pub(crate) trait BackendHook {
    async fn create(
        &self,
        inner: &Arc<dyn MachineBackend>,
        spec: MachineSpec,
        waiting: Option<Waiting>,
        sent: Option<String>,
    ) -> Result<Arc<dyn Machine>, EngineError>;
    async fn get(
        &self,
        inner: &Arc<dyn MachineBackend>,
        id: &str,
    ) -> Result<Arc<dyn Machine>, EngineError>;
}

struct ClaimHook {
    area: Rc<MachinesArea>,
    purpose: String,
    place: Option<String>,
    mine: Rc<RefCell<Vec<String>>>,
}

#[async_trait(?Send)]
impl BackendHook for ClaimHook {
    async fn create(
        &self,
        inner: &Arc<dyn MachineBackend>,
        spec: MachineSpec,
        waiting: Option<Waiting>,
        _sent: Option<String>,
    ) -> Result<Arc<dyn Machine>, EngineError> {
        claimed_create(
            &self.area,
            inner,
            &self.purpose,
            self.place.as_deref(),
            &self.mine,
            spec,
            waiting,
        )
        .await
        .map_err(AreaError::into_engine)
    }
    async fn get(
        &self,
        inner: &Arc<dyn MachineBackend>,
        id: &str,
    ) -> Result<Arc<dyn Machine>, EngineError> {
        inner.get(id).await
    }
}

/// `observing`'s hook: every machine a create or a get answers is observed (machines.ts:302).
struct ObserveBackend {
    area: Rc<MachinesArea>,
}

#[async_trait(?Send)]
impl BackendHook for ObserveBackend {
    async fn create(
        &self,
        inner: &Arc<dyn MachineBackend>,
        spec: MachineSpec,
        _waiting: Option<Waiting>,
        _sent: Option<String>,
    ) -> Result<Arc<dyn Machine>, EngineError> {
        Ok(self.area.observed(inner.create(spec, None, None).await?))
    }
    async fn get(
        &self,
        inner: &Arc<dyn MachineBackend>,
        id: &str,
    ) -> Result<Arc<dyn Machine>, EngineError> {
        Ok(self.area.observed(inner.get(id).await?))
    }
}

/// A backend whose `create` and `get` go through a hook and whose every other call is its inner backend's own.
pub(crate) struct Relay {
    pub(crate) inner: Arc<dyn MachineBackend>,
    pub(crate) hook: Rc<dyn BackendHook>,
}

#[async_trait(?Send)]
impl MachineBackend for Relay {
    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }
    fn pricing(&self) -> Arc<dyn wsp_engine::machine::BackendPricing> {
        self.inner.pricing()
    }
    fn lifecycle(&self) -> Option<Arc<dyn wsp_engine::machine::Lifecycle>> {
        self.inner.lifecycle()
    }
    fn base_templates(&self) -> Option<wsp_engine::machine::BaseTemplates> {
        self.inner.base_templates()
    }
    fn names_workspace(&self) -> bool {
        self.inner.names_workspace()
    }
    fn logins(&self) -> Option<String> {
        self.inner.logins()
    }
    fn projects(&self) -> Option<String> {
        self.inner.projects()
    }
    fn offers(&self) -> wsp_engine::machine::BackendOffers {
        self.inner.offers()
    }
    fn root_disk_of(
        &self,
        size: wsp_engine::machine::Size,
    ) -> Option<wsp_engine::machine::RootDisk> {
        self.inner.root_disk_of(size)
    }

    async fn create(
        &self,
        spec: MachineSpec,
        waiting: Option<Waiting>,
        sent: Option<String>,
    ) -> Result<Arc<dyn Machine>, EngineError> {
        self.hook.create(&self.inner, spec, waiting, sent).await
    }
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>, EngineError> {
        self.hook.get(&self.inner, id).await
    }
    async fn list(
        &self,
        labels: Option<&IndexMap<String, String>>,
    ) -> Result<Vec<wsp_engine::machine::MachineListRow>, EngineError> {
        self.inner.list(labels).await
    }
    async fn delete_snapshot(&self, id: &str) -> Result<(), EngineError> {
        self.inner.delete_snapshot(id).await
    }
    async fn on_computer(
        &self,
        cmd: &str,
        timeout_ms: Option<u64>,
    ) -> Result<ExecResult, EngineError> {
        self.inner.on_computer(cmd, timeout_ms).await
    }
    async fn create_request(&self, spec: &MachineSpec) -> Result<String, EngineError> {
        self.inner.create_request(spec).await
    }
    async fn replay_create(
        &self,
        key: &str,
        bytes: &str,
        made_at: &str,
    ) -> Result<Option<Arc<dyn Machine>>, EngineError> {
        self.inner.replay_create(key, bytes, made_at).await
    }
    async fn named_after(
        &self,
        names: &BTreeSet<String>,
    ) -> Result<Vec<wsp_engine::machine::NamedMachine>, EngineError> {
        self.inner.named_after(names).await
    }
    async fn check_key(&self) -> Result<serde_json::Value, EngineError> {
        self.inner.check_key().await
    }
    async fn place_limits(&self) -> Result<Option<wsp_engine::machine::CloudLimits>, EngineError> {
        self.inner.place_limits().await
    }
    async fn capacity(&self) -> Result<wsp_engine::machine::PlaceCapacity, EngineError> {
        self.inner.capacity().await
    }
    async fn limits(&self) -> Result<wsp_engine::box_backend::BoxLimits, EngineError> {
        self.inner.limits().await
    }
    async fn list_snapshots(
        &self,
        ids: Option<&[String]>,
    ) -> Result<Vec<wsp_engine::machine::SnapshotRow>, EngineError> {
        self.inner.list_snapshots(ids).await
    }
    async fn promote_snapshot(&self, snapshot_id: &str, name: &str) -> Result<String, EngineError> {
        self.inner.promote_snapshot(snapshot_id, name).await
    }
    async fn get_template(
        &self,
        id: &str,
    ) -> Result<wsp_engine::machine::TemplateRow, EngineError> {
        self.inner.get_template(id).await
    }
    async fn list_templates(&self) -> Result<Vec<wsp_engine::machine::TemplateRow>, EngineError> {
        self.inner.list_templates().await
    }
    async fn delete_template(&self, id: &str) -> Result<(), EngineError> {
        self.inner.delete_template(id).await
    }
}

/// What a machine's `exec` and `run` do around the provider's own call (`observed` and `watched`).
#[async_trait(?Send)]
pub(crate) trait ExecHook {
    async fn exec(
        &self,
        inner: &Arc<dyn Machine>,
        cmd: &str,
        opts: ExecOptions,
    ) -> Result<ExecResult, EngineError>;
    async fn run(
        &self,
        inner: &Arc<dyn Machine>,
        script: &str,
        opts: RunOptions,
    ) -> Result<ExecResult, EngineError>;
}

/// A machine whose exec and run go through a hook; every other member is the provider's own, bound to it.
pub(crate) struct Around {
    pub(crate) inner: Arc<dyn Machine>,
    pub(crate) hook: Rc<dyn ExecHook>,
}

#[async_trait(?Send)]
impl Machine for Around {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn kind(&self) -> MachineKind {
        self.inner.kind()
    }
    fn stream_url(&self) -> Option<&str> {
        self.inner.stream_url()
    }
    fn labels(&self) -> Option<&IndexMap<String, String>> {
        self.inner.labels()
    }
    fn seen(&self) -> Option<&wsp_engine::machine::Seen> {
        self.inner.seen()
    }
    fn replayed(&self) -> bool {
        self.inner.replayed()
    }
    fn notice(&self) -> Option<&str> {
        self.inner.notice()
    }
    fn takes_stdin(&self) -> bool {
        self.inner.takes_stdin()
    }
    fn daemon_token_path(&self) -> Option<&str> {
        self.inner.daemon_token_path()
    }
    fn daemon_answers_prompt_ms(&self) -> Option<u64> {
        self.inner.daemon_answers_prompt_ms()
    }
    fn daemon_supervisor(&self) -> Option<wsp_engine::machine::DaemonSupervisor> {
        self.inner.daemon_supervisor()
    }
    fn offers(&self) -> wsp_engine::machine::MachineOffers {
        self.inner.offers()
    }

    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult, EngineError> {
        self.hook.exec(&self.inner, cmd, opts).await
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult, EngineError> {
        self.hook.run(&self.inner, script, opts).await
    }
    async fn snapshot(
        &self,
        name: &str,
        life: wsp_engine::machine::MachineLife,
        opts: wsp_engine::machine::SnapshotOptions,
    ) -> Result<String, EngineError> {
        self.inner.snapshot(name, life, opts).await
    }
    async fn pause(&self, wait_ms: Option<u64>) -> Result<(), EngineError> {
        self.inner.pause(wait_ms).await
    }
    async fn resume(&self, signal: Option<&wsp_engine::cancel::Cancel>) -> Result<(), EngineError> {
        self.inner.resume(signal).await
    }
    async fn kill(&self) -> Result<(), EngineError> {
        self.inner.kill().await
    }
    async fn state(&self) -> Result<MachineState, EngineError> {
        self.inner.state().await
    }
    async fn download_url(&self, path: &str) -> Result<String, EngineError> {
        self.inner.download_url(path).await
    }
    async fn upload_url(&self, path: &str) -> Result<String, EngineError> {
        self.inner.upload_url(path).await
    }
    async fn framed(&self) -> Result<Arc<dyn wsp_engine::machine::Framing>, EngineError> {
        self.inner.framed().await
    }
    async fn resume_at(
        &self,
        cpu: f64,
        mem_mb: u64,
        signal: Option<&wsp_engine::cancel::Cancel>,
    ) -> Result<(), EngineError> {
        self.inner.resume_at(cpu, mem_mb, signal).await
    }
    async fn preview_url(
        &self,
        port: u16,
    ) -> Result<wsp_engine::machine::PreviewReach, EngineError> {
        self.inner.preview_url(port).await
    }
    async fn daemon_answers(&self, timeout_ms: Option<u64>) -> Result<bool, EngineError> {
        self.inner.daemon_answers(timeout_ms).await
    }
    async fn prove_road(
        &self,
        signal: Option<&wsp_engine::cancel::Cancel>,
    ) -> Result<(), EngineError> {
        self.inner.prove_road(signal).await
    }
    async fn start_daemon(&self) -> Result<ExecResult, EngineError> {
        self.inner.start_daemon().await
    }
    async fn daemon_exits(&self) -> Result<Option<String>, EngineError> {
        self.inner.daemon_exits().await
    }
    async fn put_bytes(
        &self,
        path: &str,
        bytes: &[u8],
        timeout_ms: Option<u64>,
    ) -> Result<Option<wsp_engine::machine::BytesLanded>, EngineError> {
        self.inner.put_bytes(path, bytes, timeout_ms).await
    }
    async fn read_bytes(
        &self,
        path: &str,
        size: Option<u64>,
        timeout_ms: Option<u64>,
    ) -> Result<Vec<u8>, EngineError> {
        self.inner.read_bytes(path, size, timeout_ms).await
    }
    async fn daemon_frame(
        &self,
        frame: serde_json::Map<String, serde_json::Value>,
    ) -> Result<serde_json::Map<String, serde_json::Value>, EngineError> {
        self.inner.daemon_frame(frame).await
    }
    async fn describe(&self) -> Result<wsp_wire::wire::machine_link::MachineShape, EngineError> {
        self.inner.describe().await
    }
    async fn facts(&self) -> Result<wsp_engine::machine::MachineFacts, EngineError> {
        self.inner.facts().await
    }
    async fn metrics(&self) -> Result<(), EngineError> {
        self.inner.metrics().await
    }
}

/// `observed`'s exec hook: every exec and run is reported to the recipe's listener, and a listener that fails is
/// warned about once for its machine and never changes a result (machines.ts:232-267).
struct ObserveExec {
    rt: Rc<dyn MachinesRuntime>,
    on_exec: Rc<dyn Fn(&GoldenExec) -> Result<(), String>>,
    machine_id: String,
    unheard: Cell<bool>,
}

impl ObserveExec {
    fn report(&self, exec: GoldenExec) {
        if let Err(msg) = (self.on_exec)(&exec) {
            if self.unheard.replace(true) {
                return;
            }
            self.rt.warn(&format!(
                "exec log for {} failed, its execs go on unlogged: {}",
                self.machine_id, msg
            ));
        }
    }

    async fn reported<F>(
        &self,
        cmd: &str,
        unlogged: bool,
        call: F,
    ) -> Result<ExecResult, EngineError>
    where
        F: Future<Output = Result<ExecResult, EngineError>>,
    {
        let t0 = self.rt.clock().now();
        let result = call.await;
        let ms = JsNumber((self.rt.clock().now() - t0) as f64);
        match result {
            Ok(res) => {
                let (stdout, stderr) = if unlogged {
                    (None, None)
                } else {
                    (Some(res.stdout.clone()), Some(res.stderr.clone()))
                };
                self.report(GoldenExec {
                    machine_id: self.machine_id.clone(),
                    cmd: cmd.to_string(),
                    ms,
                    exit_code: Some(res.exit_code.0 .0 as i32),
                    stdout,
                    stderr,
                    error: None,
                });
                Ok(res)
            }
            Err(e) => {
                self.report(GoldenExec {
                    machine_id: self.machine_id.clone(),
                    cmd: cmd.to_string(),
                    ms,
                    error: Some(e.to_string()),
                    ..Default::default()
                });
                Err(e)
            }
        }
    }
}

#[async_trait(?Send)]
impl ExecHook for ObserveExec {
    async fn exec(
        &self,
        inner: &Arc<dyn Machine>,
        cmd: &str,
        opts: ExecOptions,
    ) -> Result<ExecResult, EngineError> {
        self.reported(cmd, false, inner.exec(cmd, opts)).await
    }
    async fn run(
        &self,
        inner: &Arc<dyn Machine>,
        script: &str,
        opts: RunOptions,
    ) -> Result<ExecResult, EngineError> {
        let unlogged = opts.unlogged;
        self.reported(script, unlogged, inner.run(script, opts))
            .await
    }
}

/// machines.ts:23-39 `forkSpec`'s blank spec: every key unset, with the kind the machine boots as.
fn blank_spec(kind: MachineKind) -> MachineSpec {
    MachineSpec {
        kind,
        template: None,
        from_snapshot: None,
        cpu: None,
        mem_mb: None,
        disk_gb: None,
        envs: None,
        labels: None,
        on_idle: None,
        idle_timeout_ms: None,
        idempotency_key: None,
        engine: None,
        copy: None,
        shares: None,
        binds: None,
    }
}

/// `goldenImage(golden ?? { snapshotId }).spec` as the spec keys it spreads (machines.ts:33).
fn image_spec_of(version: Option<&GoldenVersion>, snapshot: &str) -> MachineSpec {
    let spec = match version {
        Some(v) => golden_image(v).spec,
        None => ImageSpec::FromSnapshot(snapshot.to_string()),
    };
    let mut out = blank_spec(MachineKind::Sandbox);
    match spec {
        ImageSpec::Template(t) => out.template = Some(t),
        ImageSpec::FromSnapshot(s) => out.from_snapshot = Some(s),
    }
    out
}

/// `new Date(ms).toISOString()` of the clock's now, as the TS writes its ISO stamps.
pub(crate) fn iso(ms: i64) -> String {
    iso_string(ms as f64).unwrap_or_default()
}

/// `backstopMs` (idle.ts:22) in the TS's floats: the window the person set is any number, so doubling it is not integer
/// arithmetic.
pub(crate) fn backstop_of(window_ms: Option<f64>) -> f64 {
    match window_ms {
        None => IDLE_OFF_BACKSTOP_MS as f64,
        Some(window) => window * 2.0,
    }
}

/// A provider's answer as a host error, with its kind, status and code kept (`AreaError::into_host`).
pub(crate) fn host_of(e: EngineError) -> HostError {
    AreaError::from(e).into_host()
}

/// A host error handed back to a backend call, which speaks engine errors; its words are kept.
pub(crate) fn host_to_engine(e: HostError) -> EngineError {
    EngineError::Other {
        message: e.message,
        code: None,
    }
}

pub(crate) fn is_missing_host(e: &HostError) -> bool {
    matches!(e.kind, Some(Kind::Missing))
}

fn parse_row<T: DeserializeOwned>(v: serde_json::Value) -> Result<T, HostError> {
    serde_json::from_value(v).map_err(|e| HostError::new(e.to_string()))
}

pub(crate) async fn sleep_ms(clock: &Rc<dyn Clock>, ms: i64) {
    let (tx, rx) = oneshot::channel::<()>();
    let run: Timer = Box::new(move || {
        let _ = tx.send(());
    });
    let _cancel = clock.schedule(run, ms);
    let _ = rx.await;
}

/// A purpose's hold: dropping it, whether or not its release was called, frees the purpose and wakes its waiters.
struct Hold {
    area: Rc<MachinesArea>,
    purpose: String,
}

impl Drop for Hold {
    fn drop(&mut self) {
        let waiters = self.area.holding.borrow_mut().shift_remove(&self.purpose);
        drop(waiters);
    }
}

/// A replay's outcome: the attempt stays for the next sweep, or its row is gone and the machine it named is reaped.
enum Replayed {
    Keep,
    Done(Option<ReapedMachine>),
}

impl MachinesArea {
    /// The claim's id added to `inflight`, once per handle (machines.ts:203-206).
    pub(crate) fn claim(&self, mine: &RefCell<Vec<String>>, id: &str) {
        if mine.borrow().iter().any(|m| m == id) {
            return;
        }
        self.inflight.borrow_mut().insert(id.to_string());
        mine.borrow_mut().push(id.to_string());
    }

    /// `claiming` without the closure: the handle a fork holds across its own awaits (see `end_claim`).
    pub fn begin_claim(
        self: &Rc<Self>,
        purpose: &str,
        at: Arc<dyn MachineBackend>,
        place: Option<&str>,
    ) -> Claimed {
        Claimed {
            area: self.clone(),
            at,
            purpose: purpose.to_string(),
            place: place.map(str::to_string),
            mine: Rc::default(),
        }
    }

    /// Every id the handle claimed leaves `inflight` once its run settles, either way (machines.ts:230).
    pub fn end_claim(&self, claim: &Claimed) {
        let mine = claim.mine.borrow().clone();
        let mut inflight = self.inflight.borrow_mut();
        for id in mine {
            inflight.shift_remove(&id);
        }
    }

    /// The host's id and this state's owner, each as the image name carries it (`imageMark`, machines.ts:41).
    fn image_mark_of(&self) -> String {
        format!(
            "{}{}",
            template_host(&self.config.host_id),
            template_host(&self.rt.owner())
        )
    }

    /// The store's row for one purpose, read as the attempt it is.
    async fn held_attempt(&self, purpose: &str) -> Result<Option<PendingCreate>, HostError> {
        let row = self.rt.store().get(CREATES, purpose).await?;
        row.map(parse_row::<PendingCreate>).transpose()
    }

    /// `forkSpec` (machines.ts:23): keys and precedence by spec rule 1; `createdAt` is the clock's now as ISO.
    pub fn fork_spec(&self, record: &WorkspaceRecord, o: ForkSpecIn<'_>) -> MachineSpec {
        let mut spec = o.image.cloned().unwrap_or_else(|| blank_spec(o.kind));
        spec.kind = o.kind;
        if let Some(binds) = record.spec.binds.as_ref().filter(|b| !b.is_empty()) {
            spec.binds = Some(binds.clone());
        }
        if let Some(copy) = record.spec.copy.as_ref() {
            spec.copy = Some(copy.clone());
        }
        let cpu = o
            .over
            .and_then(|x| x.cpu)
            .or_else(|| record.shape.as_ref().and_then(|s| s.cpu))
            .unwrap_or(record.size.cpu);
        let mem = o
            .over
            .and_then(|x| x.mem_mb)
            .or_else(|| record.shape.as_ref().and_then(|s| s.mem_mb))
            .unwrap_or(record.size.mem_mb);
        spec.cpu = Some(cpu);
        spec.mem_mb = Some(mem);

        let mut envs = login_env_on(record.place.as_deref(), o.npm_bin);
        if let Some(recorded) = &record.spec.envs {
            envs.extend(recorded.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        if let Some(over) = o.over.and_then(|x| x.envs.as_ref()) {
            envs.extend(over.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        spec.envs = Some(envs);

        let mut labels = record.spec.labels.clone().unwrap_or_default();
        labels.insert(wsp_engine::labels::WSP_LABEL.into(), "1".into());
        labels.insert(wsp_engine::labels::OWNER_LABEL.into(), self.rt.owner());
        labels.insert(
            wsp_engine::labels::WORKSPACE_LABEL.into(),
            record.id.clone(),
        );
        labels.insert(wsp_engine::labels::NAME_LABEL.into(), record.name.clone());
        labels.insert(
            wsp_engine::labels::GOLDEN_LABEL.into(),
            record.golden.clone(),
        );
        labels.insert(
            wsp_engine::labels::CREATED_AT_LABEL.into(),
            iso(self.rt.clock().now()),
        );
        spec.labels = Some(labels);

        spec.on_idle = Some(MachineSpecOnIdle::Pause);
        spec.idle_timeout_ms = Some(JsNumber(backstop_of(self.idle_window_of(record))));
        spec
    }

    /// `imageMark` (machines.ts:41).
    pub fn image_mark(&self) -> String {
        self.image_mark_of()
    }

    /// `hold` (machines.ts:45): waits while the purpose is held, re-checking after each wake (not FIFO), then holds it.
    /// The answer releases it.
    pub async fn hold(self: &Rc<Self>, purpose: &str) -> Box<dyn FnOnce()> {
        loop {
            let waiting = {
                let mut holding = self.holding.borrow_mut();
                match holding.get_mut(purpose) {
                    Some(waiters) => {
                        let (tx, rx) = oneshot::channel();
                        waiters.push(tx);
                        rx
                    }
                    None => {
                        holding.insert(purpose.to_string(), Vec::new());
                        break;
                    }
                }
            };
            let _ = waiting.await;
        }
        let hold = Hold {
            area: self.clone(),
            purpose: purpose.to_string(),
        };
        Box::new(move || drop(hold))
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
    ) -> AreaResult<Arc<dyn Machine>> {
        let store = self.rt.store();
        let body = fingerprint(&spec);
        let self_pid = std::process::id();
        let name = if purpose.encode_utf16().count() <= KEY_PURPOSE_MAX {
            purpose.to_string()
        } else {
            hex::encode(Sha256::digest(purpose.as_bytes()))
        };
        let mut after_corpse = after_corpse;
        loop {
            let held = self.held_attempt(purpose).await?;
            let theirs = held.as_ref().is_some_and(|h| {
                h.host != self.config.host_id || (h.pid != self_pid && pid_alive(h.pid as i32))
            });
            let offers = at.offers().keyed_create;
            let replay = held
                .as_ref()
                .is_some_and(|h| h.body == body && !theirs && (h.request.is_some() || !offers));
            let created_at = match (&held, replay) {
                (Some(h), true) => h.created_at.clone(),
                _ => iso(self.rt.clock().now()),
            };
            let mut stamped = spec.clone();
            if let Some(labels) = stamped
                .labels
                .as_mut()
                .filter(|l| l.contains_key(wsp_engine::labels::CREATED_AT_LABEL))
            {
                labels.insert(
                    wsp_engine::labels::CREATED_AT_LABEL.into(),
                    created_at.clone(),
                );
            }
            let image = spec.from_snapshot.clone().or_else(|| spec.template.clone());
            let attempt = match (&held, replay) {
                (Some(h), true) => PendingCreate {
                    host: self.config.host_id.clone(),
                    pid: self_pid,
                    ..h.clone()
                },
                _ => {
                    let key = format!("{name}:{}", hex::encode(self.rt.ids().bytes(8)));
                    let request = if offers {
                        Some(at.create_request(&stamped).await?)
                    } else {
                        None
                    };
                    PendingCreate {
                        key,
                        created_at: created_at.clone(),
                        body: body.clone(),
                        host: self.config.host_id.clone(),
                        pid: self_pid,
                        request,
                        image: image.clone(),
                        place: place.map(str::to_string),
                    }
                }
            };
            store
                .put(
                    CREATES,
                    purpose,
                    serde_json::to_value(&attempt).unwrap_or(serde_json::Value::Null),
                )
                .await?;
            let mut sent = stamped;
            sent.idempotency_key = Some(attempt.key.clone());
            let machine = match at
                .create(sent, waiting.clone(), attempt.request.clone())
                .await
            {
                Ok(machine) => machine,
                Err(e) => {
                    if e.status().is_some() {
                        store.delete(CREATES, purpose).await?;
                    }
                    return Err(e.into());
                }
            };
            store.delete(CREATES, purpose).await?;
            if machine.replayed() {
                if machine.state().await? == MachineState::Gone {
                    if after_corpse {
                        return Err(HostError::new(format!(
                            "create for {purpose}: the provider replayed {}, which is gone, under a key it had never seen ({})",
                            machine.id(),
                            attempt.key
                        ))
                        .into());
                    }
                    self.rt.warn(&format!("create for {purpose}: the replay under {} named {}, which is gone; creating anew", attempt.key, machine.id()));
                    after_corpse = true;
                    continue;
                }
                self.rt.warn(&format!(
                    "create for {purpose}: {} replayed from an earlier attempt under {}",
                    machine.id(),
                    attempt.key
                ));
            }
            return Ok(machine);
        }
    }

    /// `replayCreates` (machines.ts:106), spec rule 5. `known` is read once per machine found.
    pub async fn replay_creates(
        self: &Rc<Self>,
        known: &dyn Fn() -> Vec<String>,
        failed: &mut Vec<ReapFailure>,
    ) -> Result<Vec<ReapedMachine>, HostError> {
        let self_pid = std::process::id();
        let mut deleted = Vec::new();
        for purpose in self.rt.store().keys(CREATES).await? {
            let Some(row) = self.held_attempt(&purpose).await? else {
                continue;
            };
            let Some(request) = row.request.clone() else {
                continue;
            };
            if row.host != self.config.host_id {
                continue;
            }
            if self.holding.borrow().contains_key(&purpose)
                || (row.pid != self_pid && pid_alive(row.pid as i32))
            {
                continue;
            }
            let age = self.rt.clock().now() as f64 - date_parse(&row.created_at);
            if age < LOST_CREATE_MS as f64 {
                continue;
            }
            let places = self.rt.places();
            let at = match row.place.as_deref() {
                None => Some(self.rt.backend()),
                Some(p) if p == places.wired() => Some(self.rt.backend()),
                Some(p) => places.backend(p),
            };
            let Some(at) = at else { continue };
            if !at.offers().keyed_create {
                continue;
            }
            let release = self.hold(&purpose).await;
            let outcome = self
                .replay_row(&purpose, &row, &request, &at, known, failed)
                .await;
            release();
            match outcome? {
                Replayed::Keep => {}
                Replayed::Done(reaped) => deleted.extend(reaped),
            }
        }
        Ok(deleted)
    }

    /// One held attempt's replay, under its purpose's hold (machines.ts:124-150).
    async fn replay_row(
        &self,
        purpose: &str,
        row: &PendingCreate,
        request: &str,
        at: &Arc<dyn MachineBackend>,
        known: &dyn Fn() -> Vec<String>,
        failed: &mut Vec<ReapFailure>,
    ) -> Result<Replayed, HostError> {
        let store = self.rt.store();
        let machine = match at.replay_create(&row.key, request, &row.created_at).await {
            Ok(machine) => machine,
            Err(e) => {
                let e = AreaError::from(e);
                if e.is_missing() {
                    store.delete(CREATES, purpose).await?;
                    return Ok(Replayed::Done(None));
                }
                failed.push(ReapFailure {
                    id: None,
                    message: format!("the create under {} was asked again and not answered: {}; asked again next sweep", row.key, e.message()),
                });
                return Ok(Replayed::Keep);
            }
        };
        let Some(machine) = machine else {
            store.delete(CREATES, purpose).await?;
            return Ok(Replayed::Done(None));
        };
        let workspace = purpose.strip_prefix("workspace/");
        let live = match workspace {
            Some(w) => self.rt.live().borrow().contains_key(w),
            None => true,
        };
        let kept = match workspace {
            Some(w) if !live => store
                .get(WORKSPACE_NAMES, w)
                .await?
                .map(parse_row::<NamedWorkspace>)
                .transpose()?,
            _ => None,
        };
        let waits = kept
            .as_ref()
            .and_then(|k| k.project.clone())
            .is_some_and(|p| self.rt.project_held(&p));
        let seen_gone = machine.seen().map(|s| s.state) == Some(MachineState::Gone);
        if !seen_gone && !known().iter().any(|k| k == machine.id()) && !waits {
            if let Err(e) = self.rt.gone_stop(at, &machine).await {
                if !is_missing_host(&e) {
                    failed.push(ReapFailure {
                        id: Some(machine.id().to_string()),
                        message: format!("could not stop: {}", e.message),
                    });
                    return Ok(Replayed::Keep);
                }
            }
            let reaped = ReapedMachine {
                id: machine.id().to_string(),
                labels: machine.labels().cloned(),
                builder: false,
                reason: ReapReason::Lost,
                age_ms: Some((self.rt.clock().now() as f64 - date_parse(&row.created_at)) as i64),
                name: row.image.clone(),
            };
            store.delete(CREATES, purpose).await?;
            return Ok(Replayed::Done(Some(reaped)));
        }
        store.delete(CREATES, purpose).await?;
        Ok(Replayed::Done(None))
    }

    /// `sweepStrays` (machines.ts:159), spec rule 6.
    pub async fn sweep_strays(
        self: &Rc<Self>,
        known: &dyn Fn() -> Vec<String>,
        failed: &mut Vec<ReapFailure>,
        at: &[Arc<dyn MachineBackend>],
    ) -> Result<Vec<ReapedMachine>, HostError> {
        let asking: Vec<&Arc<dyn MachineBackend>> =
            at.iter().filter(|b| b.offers().keyed_create).collect();
        if asking.is_empty() {
            return Ok(Vec::new());
        }
        let mark = self.image_mark_of();
        let mut asked = HashSet::new();
        for row in self.rt.store().list(CREATES).await? {
            if let Some(image) = row.get("image").and_then(serde_json::Value::as_str) {
                asked.insert(image.to_string());
            }
        }
        let mut names = BTreeSet::new();
        for name in self
            .rt
            .recorded_images()
            .await?
            .into_iter()
            .chain(asked.iter().cloned())
        {
            if name_owner(Some(&name)).as_deref() == Some(mark.as_str()) {
                names.insert(name);
            }
        }
        if names.is_empty() {
            return Ok(Vec::new());
        }
        let mut deleted = Vec::new();
        for b in asking {
            let rows = b.named_after(&names).await.map_err(host_of)?;
            let read = StrayRead {
                known: known().into_iter().collect(),
                asked: asked.clone(),
                now: self.rt.clock().now(),
            };
            for (m, age) in strays(&rows, &read) {
                let stopped = match b.get(&m.id).await {
                    Ok(machine) => self.rt.gone_stop(b, &machine).await,
                    Err(e) => Err(host_of(e)),
                };
                if let Err(e) = stopped {
                    if !is_missing_host(&e) {
                        failed.push(ReapFailure {
                            id: Some(m.id.clone()),
                            message: format!("could not stop: {}", e.message),
                        });
                    }
                    continue;
                }
                deleted.push(ReapedMachine {
                    id: m.id.clone(),
                    labels: None,
                    builder: false,
                    reason: ReapReason::Stray,
                    age_ms: Some(age),
                    name: Some(m.name.clone()),
                });
            }
        }
        Ok(deleted)
    }

    /// `claiming` (machines.ts:185): `at` is `backend()` when `None`. Every id the handle claimed leaves `inflight`
    /// once `run` settles, either way.
    pub async fn claiming<T, F>(
        self: &Rc<Self>,
        purpose: &str,
        run: F,
        at: Option<Arc<dyn MachineBackend>>,
        place: Option<&str>,
    ) -> Result<T, HostError>
    where
        F: for<'c> FnOnce(&'c Claimed) -> futures::future::LocalBoxFuture<'c, Result<T, HostError>>,
    {
        let claim = self.begin_claim(purpose, at.unwrap_or_else(|| self.rt.backend()), place);
        let out = run(&claim).await;
        self.end_claim(&claim);
        out
    }

    /// `observed` (machines.ts:232): the machine with `exec` and `run` reported to the recipe's listener.
    pub fn observed(&self, machine: Arc<dyn Machine>) -> Arc<dyn Machine> {
        let Some(on_exec) = self.rt.on_exec() else {
            return machine;
        };
        let hook = ObserveExec {
            rt: self.rt.clone(),
            on_exec,
            machine_id: machine.id().to_string(),
            unheard: Cell::new(false),
        };
        Arc::new(Around {
            inner: machine,
            hook: Rc::new(hook),
        })
    }

    /// `observing` (machines.ts:302): `create` and `get` answer observed machines.
    pub fn observing(self: &Rc<Self>, backend: Arc<dyn MachineBackend>) -> Arc<dyn MachineBackend> {
        Arc::new(Relay {
            inner: backend,
            hook: Rc::new(ObserveBackend { area: self.clone() }),
        })
    }

    /// `fork` (machines.ts:307), spec rule 10. `bind` runs before the claim is released.
    pub async fn fork(
        self: &Rc<Self>,
        record: &Rc<RefCell<WorkspaceRecord>>,
        bind: &dyn for<'m, 'r> Fn(&'m Arc<dyn Machine>, &'r WorkspaceRecord),
        over: Option<&WorkspaceSpec>,
        report: Option<Report>,
        reads_memory: bool,
    ) -> AreaResult<Arc<dyn Machine>> {
        let snapshot = record.borrow().clone();
        let at = self.rt.backend_for(&snapshot);
        let claim = self.begin_claim(
            &format!("workspace/{}", snapshot.id),
            at,
            snapshot.place.as_deref(),
        );
        let out = self
            .fork_claimed(&claim, record, bind, over, report, reads_memory)
            .await;
        self.end_claim(&claim);
        out
    }

    /// The body of `fork` inside its claim (machines.ts:309-354).
    async fn fork_claimed(
        self: &Rc<Self>,
        claim: &Claimed,
        record: &Rc<RefCell<WorkspaceRecord>>,
        bind: &dyn for<'m, 'r> Fn(&'m Arc<dyn Machine>, &'r WorkspaceRecord),
        over: Option<&WorkspaceSpec>,
        report: Option<Report>,
        reads_memory: bool,
    ) -> AreaResult<Arc<dyn Machine>> {
        let b = claim.backend();
        let snapshot = record.borrow().clone();
        let image = if self.rt.keeps_images(&b) {
            Some(self.rt.image_of(&snapshot.golden).await?)
        } else {
            None
        };
        let golden = image.as_ref().and_then(|i| i.version.as_ref());
        let image_spec = image
            .as_ref()
            .map(|_| image_spec_of(golden, &snapshot.golden));
        let kind = golden.and_then(|g| g.kind).unwrap_or(MachineKind::Sandbox);
        let npm_bin = golden.and_then(|g| g.npm_bin.clone());
        let spec = self.fork_spec(
            &snapshot,
            ForkSpecIn {
                kind,
                image: image_spec.as_ref(),
                over,
                npm_bin: npm_bin.as_deref(),
            },
        );

        let waiting: Option<Waiting> = report.clone().map(|r| {
            Arc::new(move |line: &str, box_: Option<&ProviderBoxState>| {
                r(
                    WorkspaceCreateStage::ForkRequested,
                    line,
                    Some(StageSaid {
                        notice: None,
                        detail: None,
                        waiting: true,
                        r#box: box_.cloned(),
                    }),
                );
            }) as Waiting
        });
        let machine = match claim.create(spec.clone(), waiting).await {
            Ok(machine) => machine,
            Err(e) => {
                let place = snapshot.place.clone();
                if image.is_none() || place.is_none() || !e.is_missing() {
                    return Err(e);
                }
                let place = place.unwrap_or_default();
                let asked = spec
                    .from_snapshot
                    .clone()
                    .or_else(|| spec.template.clone())
                    .unwrap_or_else(|| snapshot.golden.clone());
                return Err(HostError::with_kind(
                    place_holds_no_image_line(&self.rt.place_name_of(&place), &asked),
                    Kind::Invalid,
                )
                .into());
            }
        };

        let bound = record.borrow().clone();
        bind(&machine, &bound);

        if !self.rt.names_workspace(&b) {
            let named = set_hostname(machine.as_ref(), &snapshot.name).await;
            if let Some(r) = &report {
                match named.refused {
                    None => r(
                        WorkspaceCreateStage::HostnameSet,
                        &hostname_set_line(&named.host),
                        None,
                    ),
                    Some(detail) => r(
                        WorkspaceCreateStage::HostnameSet,
                        HOSTNAME_KEPT,
                        Some(StageSaid {
                            notice: None,
                            detail: Some(detail),
                            waiting: false,
                            r#box: None,
                        }),
                    ),
                }
            }
        }

        let at_provider = self
            .rt
            .at_provider(self.rt.place_id_of(&snapshot).as_deref());
        let golden_pick = golden.map(|g| GoldenPick {
            version: g.version.0 as u32,
            created_at: g.created_at.clone(),
            setup_sha: g.setup_sha.clone(),
            npm_bin: g.npm_bin.clone(),
        });
        let context = apply_machine_context(
            &machine,
            ApplyContextOptions {
                workspace: Some(WorkspaceName {
                    name: snapshot.name.clone(),
                }),
                golden: golden_pick,
                result: None,
                roots: None,
                fetch: None,
                path: None,
                shells: None,
                at_provider,
            },
        )
        .await;
        if context.failure.is_some() {
            self.rt.warn(&format!(
                "machine context for {} on {} {}",
                snapshot.id,
                machine.id(),
                context.summary
            ));
        }
        record.borrow_mut().npm_bin = npm_bin;

        let shape = self.rt.shape_of(&machine).await;
        record.borrow_mut().shape = shape.clone();
        let asked = WorkspaceSize {
            cpu: spec.cpu.unwrap_or(JsNumber(0.0)),
            mem_mb: spec.mem_mb.unwrap_or(JsNumber(0.0)),
        };
        let built = self.rt.size_built(shape.as_ref(), asked);
        record.borrow_mut().size = built;

        if reads_memory && snapshot.place.is_none() {
            let sizes: Vec<WorkspaceSize> = b
                .capabilities()
                .sizes
                .iter()
                .map(|s| WorkspaceSize {
                    cpu: s.cpu,
                    mem_mb: s.mem_mb,
                })
                .collect();
            self.rt.read_memory(record, &machine, &sizes).await;
        }

        let screen =
            machine
                .stream_url()
                .map(|url| wsp_wire::views::workspace::WorkspaceViewScreen {
                    stream_url: url.to_string(),
                });
        record.borrow_mut().screen = screen;
        Ok(machine)
    }

    /// `unfork` (machines.ts:356), spec rule 11.
    pub async fn unfork(self: &Rc<Self>, entry: &Entry) -> Result<(), HostError> {
        let (id, machine, snapshot) = {
            let e = entry.borrow();
            let snapshot = e.record.borrow().clone();
            (e.machine.id().to_string(), e.machine.clone(), snapshot)
        };
        let at = self.rt.backend_for(&snapshot);
        let clock = self.rt.clock();
        kill_until_gone(
            at.as_ref(),
            machine.as_ref(),
            KillConfirm::default(),
            &RetryClockOver::new(clock.clone()),
        )
        .await
        .map_err(host_of)?;
        let mark = DroppedMachine {
            machine_id: id.clone(),
            at: iso(clock.now()),
        };
        if let Err(e) = self
            .rt
            .store()
            .put(
                DROPPED,
                &id,
                serde_json::to_value(&mark).unwrap_or(serde_json::Value::Null),
            )
            .await
        {
            self.rt.warn(&format!(
                "machine {id} is gone but its mark was not stored ({}); should the provider list it running again, the sweep reports it rather than killing it",
                e.message
            ));
        }
        Ok(())
    }
}

/// The attempt a create puts under its purpose (wsp-core's `PendingCreate`), re-exported for the tests.
pub type Attempt = PendingCreate;
