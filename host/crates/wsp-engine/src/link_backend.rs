// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/link-backend.ts at a4033dc34. Spec: spec/link_backend.md.
//!
//! One computer's machines, driven from another: each call is one frame on a link somebody else holds. What the far
//! side answers is what its own backend answered, kind and status included.
use std::pin::pin;
use std::sync::Arc;

use crate::cancel::Cancel;
use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use url::Url;
use wsp_catalog::signin_rows::shares_in;
use wsp_wire::types::check::{parse, Check};
use wsp_wire::views::place::MachineShare;
use wsp_wire::wire::daemon::DaemonExecReply;
use wsp_wire::wire::machine_link::{
    BackendFacts, MachineAnswersReply, MachineExecReply, MachineFactsReply, MachineHandle, MachineHandleReply, MachineListReply,
    MachineReachReply, MachineReadingReply, MachineShapeReply, MachineStateReply, MachineUrlReply, MACHINE_PUT_PART_BYTES, NO_IMAGES_HERE,
};

use crate::errors::{aborted, EngineError, Result};
use crate::exec_detached::{exec_detached, INLINE_EXEC_MS};
use crate::labels::BUILDER_LABEL;
use crate::machine::{
    BackendOffers, BackendPricing, BytesLanded, Capabilities, DaemonSupervisor, ExecOptions, ExecResult, Lifecycle, Machine,
    MachineBackend, MachineFacts, MachineKind, MachineLife, MachineListRow, MachineOffers, MachineShape, MachineSpec, MachineState,
    PlaceCapacity, PreviewReach, RunOptions, Seen, Size, SnapshotOptions, Waiting,
};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::wire::machine_link::{BackendFactsBaseTemplates, LifecycleBudgets, SnapshotStoragePricing as WireSnapshotStoragePricing};

/// link-backend.ts:55. What one ask on a link may say about itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkAsk {
    pub timeout_ms: Option<u64>,
    pub idempotency_key: Option<String>,
}

/// link-backend.ts:61. The road to one place, as whoever holds the socket hands it over.
#[async_trait(?Send)]
pub trait MachineLink {
    /// One frame and its answer: the far side's refusal with its kind and status, `PlaceAbsent` when the place is not
    /// connected, a timeout when nothing answered inside the ask's bound.
    async fn request(&self, op: &str, params: Map<String, Value>, ask: LinkAsk) -> Result<Map<String, Value>>;
    /// Whether this link redials at all (the TS's `dialsBack` present).
    fn has_dials_back(&self) -> bool {
        false
    }
    /// Whether the road went and came back inside the bounded wait.
    async fn dials_back(&self) -> bool {
        false
    }
    /// A port on this computer's loopback carried to one port on the place's own loopback.
    async fn forward(&self, place_port: u16) -> Result<u16>;
}

/// link-backend.ts:82.
pub const LINK_MARGIN_MS: u64 = 5_000;
/// link-backend.ts:87.
pub const LINK_PROMPT_MS: u64 = 6_000;
/// How long a cancelled resume goes on driving its frame.
const CANCELLED_RESUME_MS: std::time::Duration = std::time::Duration::from_millis(80);
/// link-backend.ts:399.
pub const CREATE_MS: u64 = 600_000 + LINK_MARGIN_MS;
/// link-backend.ts:403.
pub const UPLOAD_ATTEMPTS: u32 = 3;

/// link-backend.ts:91. What every call on a place that is not connected fails with.
pub fn place_absent(message: &str) -> EngineError {
    EngineError::PlaceAbsent { message: message.to_string() }
}

/// link-backend.ts:95.
pub fn is_place_absent(e: &EngineError) -> bool {
    matches!(e, EngineError::PlaceAbsent { .. })
}

/// link-backend.ts:105. One machine on a place.
pub struct LinkMachine {
    link: Arc<dyn MachineLink>,
    handle: MachineHandle,
    seen: Option<Seen>,
}

/// The loopback route's port the TS hands back as `expiresAt`: `Number.MAX_SAFE_INTEGER`, which never comes.
const NEVER: i64 = 9_007_199_254_740_991;

pub(crate) fn other(message: impl ToString) -> EngineError {
    EngineError::Other { message: message.to_string(), code: None }
}

/// A frame's params as a JSON object's keys, in the order written.
pub(crate) fn obj(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

/// `{ ...params, machineId }`: a `machineId` the params already carry keeps its place and takes the machine's id.
pub(crate) fn named(id: &str, mut params: Map<String, Value>) -> Map<String, Value> {
    params.insert("machineId".to_string(), Value::String(id.to_string()));
    params
}

/// A far side's answer read by the reply's own schema.
pub(crate) fn read<T: Check>(answer: Map<String, Value>) -> Result<T> {
    parse::<T>(&Value::Object(answer)).map_err(|e| other(e.message))
}

/// A daemon frame's op and the rest of its params, in the order written.
pub(crate) fn split_op(mut frame: Map<String, Value>) -> (String, Map<String, Value>) {
    let op = frame.shift_remove("op").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    (op, frame)
}

/// One daemon frame's answer as the TS reply object: the reply's own keys after `ok: true`, or the error's words and
/// its code where it is a string, after `ok: false`. Never an error.
pub(crate) async fn daemon_reply(link: &dyn MachineLink, op: &str, params: Map<String, Value>) -> Map<String, Value> {
    match link.request(op, params, LinkAsk::default()).await {
        Ok(reply) => {
            let mut out = obj(json!({ "ok": true }));
            for (k, v) in reply {
                out.insert(k, v);
            }
            out
        }
        Err(e) => {
            let mut out = obj(json!({ "ok": false, "error": e.to_string() }));
            if let Some(code) = e.code() {
                out.insert("code".to_string(), Value::String(code.to_string()));
            }
            out
        }
    }
}

/// What a cancelled resume rejects with: the caller's reason as an error, or the engine's abort where it gave none.
fn cancel_error(stop: &Cancel) -> EngineError {
    match stop.given_reason() {
        None => aborted(),
        Some(reason) => other(reason),
    }
}

impl LinkMachine {
    /// link-backend.ts:123.
    pub fn new(link: Arc<dyn MachineLink>, handle: MachineHandle) -> Self {
        let seen = handle.seen.as_ref().map(|s| Seen { state: s.state, created_at: s.created_at.clone(), paused_at: None });
        Self { link, handle, seen }
    }

    /// One frame to this machine: its params with `machineId` last.
    async fn ask(&self, op: &str, params: Map<String, Value>, ask: LinkAsk) -> Result<Map<String, Value>> {
        self.link.request(op, named(&self.handle.id, params), ask).await
    }

    /// The file in parts under one upload id, in order, the last one marked; no part carries a key of its own.
    async fn land_parts(&self, path: &str, bytes: &[u8], timeout: u64, upload_id: &str, parts: usize) -> Result<()> {
        let part_bytes = MACHINE_PUT_PART_BYTES as usize;
        for seq in 0..parts {
            let start = seq * part_bytes;
            let end = ((seq + 1) * part_bytes).min(bytes.len());
            self.ask(
                "machine.putBytes",
                obj(json!({
                    "path": path,
                    "uploadId": upload_id,
                    "seq": seq,
                    "last": seq + 1 == parts,
                    "data": STANDARD.encode(&bytes[start..end]),
                    "timeoutMs": timeout,
                })),
                LinkAsk { timeout_ms: Some(timeout.saturating_add(LINK_MARGIN_MS)), idempotency_key: None },
            )
            .await?;
        }
        Ok(())
    }
}

#[async_trait(?Send)]
impl Machine for LinkMachine {
    fn id(&self) -> &str {
        &self.handle.id
    }
    fn kind(&self) -> MachineKind {
        self.handle.kind
    }
    fn stream_url(&self) -> Option<&str> {
        self.handle.stream_url.as_deref()
    }
    fn labels(&self) -> Option<&IndexMap<String, String>> {
        self.handle.labels.as_ref()
    }
    fn seen(&self) -> Option<&Seen> {
        self.seen.as_ref()
    }
    fn replayed(&self) -> bool {
        self.handle.replayed.unwrap_or(false)
    }
    fn notice(&self) -> Option<&str> {
        self.handle.notice.as_deref()
    }
    fn takes_stdin(&self) -> bool {
        true
    }
    fn daemon_answers_prompt_ms(&self) -> Option<u64> {
        self.handle.roads.daemon_answers.then_some(LINK_PROMPT_MS)
    }
    fn daemon_supervisor(&self) -> Option<DaemonSupervisor> {
        self.handle.daemon_supervisor
    }
    fn offers(&self) -> MachineOffers {
        let roads = &self.handle.roads;
        MachineOffers {
            preview_url: roads.preview_url,
            daemon_answers: roads.daemon_answers,
            put_bytes: roads.put_bytes,
            describe: roads.describe,
            facts: roads.facts,
            metrics: roads.metrics,
            daemon_frame: true,
            ..Default::default()
        }
    }

    /// link-backend.ts:152.
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        let timeout = opts.timeout_ms.unwrap_or(INLINE_EXEC_MS);
        let mut params = obj(json!({ "cmd": cmd, "timeoutMs": timeout }));
        if let Some(stdin) = &opts.stdin {
            params.insert("stdin".to_string(), Value::String(STANDARD.encode(stdin)));
        }
        let ask = LinkAsk { timeout_ms: Some(timeout.saturating_add(LINK_MARGIN_MS)), idempotency_key: opts.idempotency_key };
        Ok(read::<MachineExecReply>(self.ask("machine.exec", params, ask).await?)?.result)
    }

    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        exec_detached(self, script, opts).await
    }

    /// link-backend.ts:171.
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        Err(other(NO_IMAGES_HERE))
    }

    /// link-backend.ts:175. The wait is the TS's to ignore.
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        self.ask("machine.pause", Map::new(), LinkAsk::default()).await.map(|_| ())
    }

    /// link-backend.ts:181. The frame is sent once whatever the caller does with it: a cancel ends the wait, and the frame
    /// goes on being driven for `CANCELLED_RESUME_MS` so the link can finish its round trip, where the TS lets it run on
    /// without end.
    async fn resume(&self, signal: Option<&Cancel>) -> Result<()> {
        let mut sent = pin!(self.ask("machine.resume", Map::new(), LinkAsk::default()));
        let Some(stop) = signal else {
            return sent.await.map(|_| ());
        };
        if !stop.is_cancelled() {
            tokio::select! {
                biased;
                () = stop.cancelled() => {}
                r = sent.as_mut() => return r.map(|_| ()),
            }
        }
        let _ = tokio::time::timeout(CANCELLED_RESUME_MS, sent).await;
        Err(cancel_error(stop))
    }

    async fn kill(&self) -> Result<()> {
        self.ask("machine.kill", Map::new(), LinkAsk::default()).await.map(|_| ())
    }

    async fn state(&self) -> Result<MachineState> {
        Ok(read::<MachineStateReply>(self.ask("machine.state", Map::new(), LinkAsk::default()).await?)?.state)
    }

    async fn download_url(&self, path: &str) -> Result<String> {
        let answer = self.ask("machine.downloadUrl", obj(json!({ "path": path })), LinkAsk::default()).await?;
        Ok(read::<MachineUrlReply>(answer)?.url)
    }

    async fn upload_url(&self, path: &str) -> Result<String> {
        let answer = self.ask("machine.uploadUrl", obj(json!({ "path": path })), LinkAsk::default()).await?;
        Ok(read::<MachineUrlReply>(answer)?.url)
    }

    /// link-backend.ts:214. The place's own loopback route, carried to a port on this computer.
    async fn preview_url(&self, port: u16) -> Result<PreviewReach> {
        let reach =
            read::<MachineReachReply>(self.ask("machine.previewUrl", obj(json!({ "port": port })), LinkAsk::default()).await?)?.reach;
        let place_port =
            Url::parse(&reach.url).map_err(|_| other("Invalid URL"))?.port().filter(|p| *p >= 1).ok_or_else(|| {
                other(format!("{} answered {}, which names no port this host can be carried to", self.handle.id, reach.url))
            })?;
        let local = self.link.forward(place_port).await?;
        Ok(PreviewReach { url: format!("http://127.0.0.1:{local}"), token: String::new(), expires_at: JsNumber::from(NEVER) })
    }

    /// link-backend.ts:235.
    async fn daemon_answers(&self, timeout_ms: Option<u64>) -> Result<bool> {
        let timeout = timeout_ms.unwrap_or(INLINE_EXEC_MS);
        let ask = LinkAsk { timeout_ms: Some(timeout.saturating_add(LINK_MARGIN_MS)), idempotency_key: None };
        Ok(read::<MachineAnswersReply>(self.ask("machine.daemonAnswers", obj(json!({ "timeoutMs": timeout })), ask).await?)?.answers)
    }

    /// link-backend.ts:247. Each attempt is the file in parts under a fresh id; a flapping link that dials back is
    /// tried again, up to three attempts, and every other failure is the part's own.
    async fn put_bytes(&self, path: &str, bytes: &[u8], timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        let timeout = timeout_ms.unwrap_or(INLINE_EXEC_MS);
        let parts = bytes.len().div_ceil(MACHINE_PUT_PART_BYTES as usize).max(1);
        for attempt in 1.. {
            let upload_id = hex::encode(rand::random::<[u8; 8]>());
            match self.land_parts(path, bytes, timeout, &upload_id, parts).await {
                Ok(()) => return Ok(None),
                Err(e) => {
                    if attempt >= UPLOAD_ATTEMPTS || !self.link.has_dials_back() || !self.link.dials_back().await {
                        return Err(e);
                    }
                }
            }
        }
        unreachable!("the attempt counter never ends")
    }

    /// link-backend.ts:225. The frame names this machine.
    async fn daemon_frame(&self, frame: Map<String, Value>) -> Result<Map<String, Value>> {
        let (op, params) = split_op(frame);
        Ok(daemon_reply(&*self.link, &op, named(&self.handle.id, params)).await)
    }

    async fn describe(&self) -> Result<MachineShape> {
        Ok(read::<MachineShapeReply>(self.ask("machine.describe", Map::new(), LinkAsk::default()).await?)?.shape)
    }

    async fn facts(&self) -> Result<MachineFacts> {
        Ok(read::<MachineFactsReply>(self.ask("machine.facts", Map::new(), LinkAsk::default()).await?)?.facts)
    }

    async fn metrics(&self) -> Result<()> {
        self.ask("machine.metrics", Map::new(), LinkAsk::default()).await.map(|_| ())
    }
}

/// link-backend.ts:289. A backend on a computer this host reaches over a link.
pub struct LinkBackend {
    link: Arc<dyn MachineLink>,
    facts: BackendFacts,
}

/// The pricing the place's own offers say, rebuilt from its facts (link-backend.ts:321).
struct LinkPricing {
    sizes: Vec<wsp_wire::wire::capabilities::MachineSizeOffer>,
    default_size: wsp_wire::wire::capabilities::WorkspaceSize,
    snapshot_storage: WireSnapshotStoragePricing,
    builder_disk_gb: Option<JsNumber>,
}

impl BackendPricing for LinkPricing {
    fn rate_usd_per_hour(&self, size: Size) -> f64 {
        self.sizes.iter().find(|o| o.cpu.0 == size.cpu && o.mem_mb.0 == size.mem_mb).map_or(0.0, |o| o.rate_usd_per_hour.0)
    }
    fn default_size(&self) -> Size {
        Size { cpu: self.default_size.cpu.0, mem_mb: self.default_size.mem_mb.0 }
    }
    fn snapshot_storage(&self) -> crate::machine::SnapshotStoragePricing {
        crate::machine::SnapshotStoragePricing { wire: self.snapshot_storage.clone(), free_named: None, usd_per_named_month: None }
    }
    fn builder_disk_gb(&self) -> Option<f64> {
        self.builder_disk_gb.map(|g| g.0)
    }
}

/// The lifecycle the place's facts carry; its quiet figure is the reading's, asked of the computer running the machine.
struct LinkLifecycle {
    link: Arc<dyn MachineLink>,
    budgets: LifecycleBudgets,
}

#[async_trait(?Send)]
impl Lifecycle for LinkLifecycle {
    fn budgets(&self) -> LifecycleBudgets {
        self.budgets.clone()
    }
    fn has_quiet_for_ms(&self) -> bool {
        true
    }
    /// link-backend.ts:344. Undefined where the reading has no figure, never zero.
    async fn quiet_for_ms(&self, machine: &dyn Machine) -> Result<Option<u64>> {
        let answer = self.link.request("machine.metrics", obj(json!({ "machineId": machine.id() })), LinkAsk::default()).await?;
        let reading = read::<MachineReadingReply>(answer)?.reading;
        Ok(reading.quiet_for_ms.and_then(|q| u64::try_from(q.get()).ok()))
    }
}

impl LinkBackend {
    /// link-backend.ts:303. Asks the place what its backend is and builds it from the answer.
    pub async fn open(link: Arc<dyn MachineLink>) -> Result<Self> {
        let facts = read::<BackendFacts>(link.request("machine.backend", Map::new(), LinkAsk::default()).await?)?;
        Ok(Self { link, facts })
    }

    /// link-backend.ts:310. The same from facts already in hand, with no round trip.
    pub fn of(link: Arc<dyn MachineLink>, facts: BackendFacts) -> Self {
        Self { link, facts }
    }
}

#[async_trait(?Send)]
impl MachineBackend for LinkBackend {
    fn capabilities(&self) -> Capabilities {
        self.facts.capabilities.clone()
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(LinkPricing {
            sizes: self.facts.capabilities.sizes.clone(),
            default_size: self.facts.pricing.default_size.clone(),
            snapshot_storage: self.facts.pricing.snapshot_storage.clone(),
            builder_disk_gb: self.facts.pricing.builder_disk_gb,
        })
    }
    fn lifecycle(&self) -> Option<Arc<dyn Lifecycle>> {
        let lifecycle = self.facts.lifecycle.as_ref()?;
        Some(Arc::new(LinkLifecycle { link: self.link.clone(), budgets: lifecycle.budgets.clone() }))
    }
    fn base_templates(&self) -> Option<BackendFactsBaseTemplates> {
        self.facts.base_templates.clone()
    }
    fn names_workspace(&self) -> bool {
        true
    }
    fn logins(&self) -> Option<String> {
        self.facts.logins.clone()
    }
    fn projects(&self) -> Option<String> {
        self.facts.projects.clone()
    }
    fn offers(&self) -> BackendOffers {
        BackendOffers { on_computer: true, check_key: true, capacity: true, ..Default::default() }
    }

    /// link-backend.ts:355. `waiting` and `sent` are the TS's to ignore.
    async fn create(&self, mut spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> Result<Arc<dyn Machine>> {
        let ask = LinkAsk { timeout_ms: Some(CREATE_MS), idempotency_key: spec.idempotency_key.clone() };
        if let Some(logins) = &self.facts.logins {
            if spec.labels.as_ref().and_then(|l| l.get(BUILDER_LABEL)).is_none() {
                spec.shares = Some(shares_in(logins).into_iter().map(|s| MachineShare { source: s.source, target: s.target }).collect());
            }
        }
        let spec = serde_json::to_value(&spec).map_err(other)?;
        let answer = self.link.request("machine.create", obj(json!({ "spec": spec })), ask).await?;
        Ok(Arc::new(LinkMachine::new(self.link.clone(), read::<MachineHandleReply>(answer)?.machine)))
    }

    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>> {
        let answer = self.link.request("machine.get", obj(json!({ "machineId": id })), LinkAsk::default()).await?;
        Ok(Arc::new(LinkMachine::new(self.link.clone(), read::<MachineHandleReply>(answer)?.machine)))
    }

    async fn list(&self, labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        let params = match labels {
            None => Map::new(),
            Some(labels) => obj(json!({ "labels": labels })),
        };
        Ok(read::<MachineListReply>(self.link.request("machine.list", params, LinkAsk::default()).await?)?.machines)
    }

    /// link-backend.ts:376. Nothing here ever made a snapshot, so there is nothing to delete and no frame to send.
    async fn delete_snapshot(&self, _id: &str) -> Result<()> {
        Err(other(NO_IMAGES_HERE))
    }

    /// link-backend.ts:381.
    async fn check_key(&self) -> Result<Value> {
        self.link.request("machine.checkKey", Map::new(), LinkAsk::default()).await?;
        Ok(Value::Null)
    }

    async fn capacity(&self) -> Result<PlaceCapacity> {
        read::<PlaceCapacity>(self.link.request("machine.capacity", Map::new(), LinkAsk::default()).await?)
    }

    /// link-backend.ts:391. The frame names no machine: the computer's own daemon runs the command on itself.
    async fn on_computer(&self, cmd: &str, timeout_ms: Option<u64>) -> Result<ExecResult> {
        let timeout = timeout_ms.unwrap_or(INLINE_EXEC_MS);
        let ask = LinkAsk { timeout_ms: Some(timeout.saturating_add(LINK_MARGIN_MS)), idempotency_key: None };
        let answer = self.link.request("exec", obj(json!({ "cmd": cmd, "timeoutMs": timeout })), ask).await?;
        let reply = read::<DaemonExecReply>(answer)?;
        Ok(ExecResult { exit_code: reply.exit_code, stdout: reply.stdout, stderr: reply.stderr })
    }
}
