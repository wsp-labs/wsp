// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/link-backend.ts at a4033dc34. Spec: spec/link_backend.md.
//!
//! One computer's machines, driven from another: each call is one frame on a link somebody else holds. What the far
//! side answers is what its own backend answered, kind and status included.
use std::sync::Arc;

use crate::cancel::Cancel;
use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{Map, Value};

use crate::errors::{EngineError, Result};
use crate::machine::{
    BackendOffers, BackendPricing, BytesLanded, Capabilities, DaemonSupervisor, ExecOptions, ExecResult, Lifecycle, Machine,
    MachineBackend, MachineFacts, MachineKind, MachineLife, MachineListRow, MachineOffers, MachineShape, MachineSpec, MachineState,
    PlaceCapacity, PreviewReach, RunOptions, Seen, SnapshotOptions, Waiting,
};
use wsp_wire::wire::machine_link::{BackendFacts, MachineHandle};

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
/// link-backend.ts:399.
pub const CREATE_MS: u64 = 600_000 + LINK_MARGIN_MS;
/// link-backend.ts:403.
pub const UPLOAD_ATTEMPTS: u32 = 3;

/// link-backend.ts:91. What every call on a place that is not connected fails with.
pub fn place_absent(message: &str) -> EngineError {
    todo!()
}

/// link-backend.ts:95.
pub fn is_place_absent(e: &EngineError) -> bool {
    todo!()
}

/// link-backend.ts:105. One machine on a place.
pub struct LinkMachine {
    link: Arc<dyn MachineLink>,
    handle: MachineHandle,
    seen: Option<Seen>,
}

impl LinkMachine {
    /// link-backend.ts:123.
    pub fn new(link: Arc<dyn MachineLink>, handle: MachineHandle) -> Self {
        todo!()
    }
}

#[async_trait(?Send)]
impl Machine for LinkMachine {
    fn id(&self) -> &str {
        todo!()
    }
    fn kind(&self) -> MachineKind {
        todo!()
    }
    fn stream_url(&self) -> Option<&str> {
        todo!()
    }
    fn labels(&self) -> Option<&IndexMap<String, String>> {
        todo!()
    }
    fn seen(&self) -> Option<&Seen> {
        todo!()
    }
    fn replayed(&self) -> bool {
        todo!()
    }
    fn notice(&self) -> Option<&str> {
        todo!()
    }
    fn takes_stdin(&self) -> bool {
        todo!()
    }
    fn daemon_answers_prompt_ms(&self) -> Option<u64> {
        todo!()
    }
    fn daemon_supervisor(&self) -> Option<DaemonSupervisor> {
        todo!()
    }
    fn offers(&self) -> MachineOffers {
        todo!()
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        todo!()
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        todo!()
    }
    async fn snapshot(&self, name: &str, life: MachineLife, opts: SnapshotOptions) -> Result<String> {
        todo!()
    }
    async fn pause(&self, wait_ms: Option<u64>) -> Result<()> {
        todo!()
    }
    async fn resume(&self, signal: Option<&Cancel>) -> Result<()> {
        todo!()
    }
    async fn kill(&self) -> Result<()> {
        todo!()
    }
    async fn state(&self) -> Result<MachineState> {
        todo!()
    }
    async fn download_url(&self, path: &str) -> Result<String> {
        todo!()
    }
    async fn upload_url(&self, path: &str) -> Result<String> {
        todo!()
    }
    async fn preview_url(&self, port: u16) -> Result<PreviewReach> {
        todo!()
    }
    async fn daemon_answers(&self, timeout_ms: Option<u64>) -> Result<bool> {
        todo!()
    }
    async fn put_bytes(&self, path: &str, bytes: &[u8], timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        todo!()
    }
    async fn daemon_frame(&self, frame: Map<String, Value>) -> Result<Map<String, Value>> {
        todo!()
    }
    async fn describe(&self) -> Result<MachineShape> {
        todo!()
    }
    async fn facts(&self) -> Result<MachineFacts> {
        todo!()
    }
    async fn metrics(&self) -> Result<()> {
        todo!()
    }
}

/// link-backend.ts:289. A backend on a computer this host reaches over a link.
pub struct LinkBackend {
    link: Arc<dyn MachineLink>,
    facts: BackendFacts,
}

impl LinkBackend {
    /// link-backend.ts:303. Asks the place what its backend is and builds it from the answer.
    pub async fn open(link: Arc<dyn MachineLink>) -> Result<Self> {
        todo!()
    }

    /// link-backend.ts:310. The same from facts already in hand, with no round trip.
    pub fn of(link: Arc<dyn MachineLink>, facts: BackendFacts) -> Self {
        todo!()
    }
}

#[async_trait(?Send)]
impl MachineBackend for LinkBackend {
    fn capabilities(&self) -> Capabilities {
        todo!()
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        todo!()
    }
    fn lifecycle(&self) -> Option<Arc<dyn Lifecycle>> {
        todo!()
    }
    fn base_templates(&self) -> Option<crate::machine::BaseTemplates> {
        todo!()
    }
    fn names_workspace(&self) -> bool {
        todo!()
    }
    fn logins(&self) -> Option<String> {
        todo!()
    }
    fn projects(&self) -> Option<String> {
        todo!()
    }
    fn offers(&self) -> BackendOffers {
        todo!()
    }
    async fn create(&self, spec: MachineSpec, waiting: Option<Waiting>, sent: Option<String>) -> Result<Arc<dyn Machine>> {
        todo!()
    }
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>> {
        todo!()
    }
    async fn list(&self, labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        todo!()
    }
    async fn delete_snapshot(&self, id: &str) -> Result<()> {
        todo!()
    }
    async fn check_key(&self) -> Result<Value> {
        todo!()
    }
    async fn capacity(&self) -> Result<PlaceCapacity> {
        todo!()
    }
    async fn on_computer(&self, cmd: &str, timeout_ms: Option<u64>) -> Result<ExecResult> {
        todo!()
    }
}
