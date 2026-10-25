// SPDX-License-Identifier: AGPL-3.0-only
//! `images/records.ts` at 65f6b89c7: a workspace's view and persist, its size read off the machine, and the image
//! copies and records filed per place. Spec: `spec/images_records.md`. `migrateCopies`, the bare-name fallback and
//! the record backfilled from a copy are dropped (Proposal 3).
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexSet;

use wsp_core::types::pending::{Clock, Store};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_wire::views::golden_image::{GoldenManifest, GoldenVersion, RecipeDigest, SealedImage};
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::capabilities::{MachineSizeOffer, WorkspaceSize};
use wsp_wire::wire::machine_link::MachineShape;
use wsp_words::exit::HostError;

/// Every call the records area makes on the rest of the runtime (records.ts reads them off `ctx`).
#[async_trait(?Send)]
pub trait RecordsCtx {
    fn store(&self) -> Rc<dyn Store>;
    fn clock(&self) -> Rc<dyn Clock>;
    /// `places.wired`.
    fn wired(&self) -> String;
    fn warn(&self, line: &str);
    /// `ctx.moduleOf(r.kind).folder(r)`.
    fn module_folder(&self, r: &WorkspaceRecord) -> Option<String>;
    /// `ctx.homeOf(r).home`.
    fn home_of(&self, r: &WorkspaceRecord) -> Option<String>;
    /// `ctx.refOf(ctx.projectHeld(r.project))`.
    fn project_ref(&self, project: &str) -> ProjectView;
    /// `ctx.daemonNotes.get(r.id)`.
    fn daemon_note(&self, workspace_id: &str) -> Option<String>;
    /// `ctx.agentsOf(r)`, as its JSON.
    fn agents_of(&self, r: &WorkspaceRecord) -> Option<serde_json::Value>;
    /// `ctx.providerOf(r)`.
    fn provider_of(&self, r: &WorkspaceRecord) -> Option<String>;
    /// `ctx.backendFor(r)`; a backend that cannot be read fails.
    fn backend_for(&self, r: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError>;
    /// The records of `ctx.live`, in map order.
    fn live_records(&self) -> Vec<Rc<RefCell<WorkspaceRecord>>>;
    /// `live.get(id).generation++`; false where no entry is live.
    fn bump_generation(&self, workspace_id: &str) -> bool;
}

/// `recordsArea(ctx)` (records.ts:76).
pub struct RecordsArea {
    pub ctx: Rc<dyn RecordsCtx>,
}

/// The look keys `putLook` takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookKey {
    Theme,
    Glyph,
}

/// `imageOf`'s answer (records.ts:274).
#[derive(Clone, Debug, PartialEq)]
pub struct ImageOf {
    pub golden: String,
    pub version: Option<GoldenVersion>,
}

impl RecordsArea {
    pub fn new(ctx: Rc<dyn RecordsCtx>) -> RecordsArea {
        todo!()
    }

    /// records.ts:88-125 (R1, R2).
    pub fn view(&self, r: &WorkspaceRecord) -> WorkspaceView {
        todo!()
    }

    /// records.ts:129-133 (R3): `None` leaves, `Some(None)` clears, `Some(Some(v))` sets the look fact, as its JSON.
    pub fn put_look(&self, r: &mut WorkspaceRecord, key: LookKey, value: Option<Option<serde_json::Value>>) {
        todo!()
    }

    /// records.ts:135-142 (R4).
    pub async fn persist(&self, r: &WorkspaceRecord) -> Result<(), HostError> {
        todo!()
    }

    /// records.ts:144-147 (R5).
    pub async fn shape_of(&self, m: Arc<dyn Machine>) -> Option<MachineShape> {
        todo!()
    }

    /// records.ts:150-153 (R6).
    pub fn size_built(&self, shape: Option<&MachineShape>, asked: &WorkspaceSize) -> WorkspaceSize {
        todo!()
    }

    /// records.ts:157-170 (R7, R8).
    pub async fn read_memory(&self, record: &mut WorkspaceRecord, machine: Arc<dyn Machine>, sizes: &[MachineSizeOffer]) -> bool {
        todo!()
    }

    /// records.ts:173 (R9).
    pub fn forked_from(&self, snapshot_id: &str) -> Vec<String> {
        todo!()
    }

    /// records.ts:179 (R10).
    pub async fn copy_of(&self, place: &str, name: &str) -> Result<Option<GoldenManifest>, HostError> {
        todo!()
    }

    /// records.ts:181 (R10).
    pub async fn put_copy(&self, place: &str, name: &str, manifest: &GoldenManifest) -> Result<(), HostError> {
        todo!()
    }

    /// records.ts:182-184 (R10).
    pub async fn copy_recipe_of(&self, place: &str, name: &str, version: u32) -> Result<Option<RecipeDigest>, HostError> {
        todo!()
    }

    /// records.ts:185-186 (R10).
    pub async fn put_copy_recipe(&self, place: &str, name: &str, version: u32, digest: &RecipeDigest) -> Result<(), HostError> {
        todo!()
    }

    /// records.ts:187-190 (R10).
    pub async fn drop_copy_recipe(&self, place: &str, name: &str, version: u32) -> Result<(), HostError> {
        todo!()
    }

    /// records.ts:226-228 (R11).
    pub async fn record_of(&self, name: &str) -> Result<Option<SealedImage>, HostError> {
        todo!()
    }

    /// records.ts:248-258 (R12).
    pub async fn recorded_images(&self) -> Result<IndexSet<String>, HostError> {
        todo!()
    }

    /// records.ts:261-277 (R13).
    pub async fn image_of(&self, snapshot_id: &str) -> Result<ImageOf, HostError> {
        todo!()
    }
}
