// SPDX-License-Identifier: AGPL-3.0-only
//! `images/records.ts` at 65f6b89c7: a workspace's view and persist, its size read off the machine, and the image
//! copies and records filed per place. Spec: `spec/images_records.md`. `migrateCopies`, the bare-name fallback and
//! the record backfilled from a copy are dropped (Proposal 3).
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexSet;
use serde::Serialize;

use wsp_core::types::internal::{
    copy_key, recipe_key, NamedWorkspace, GOLDENS, GOLDEN_RECIPES, IMAGES, WORKSPACES,
    WORKSPACE_NAMES,
};
use wsp_core::types::pending::{Clock, Store};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_engine::exec_detached::put::INLINE_EXEC_MS;
use wsp_engine::machine::{ExecOptions, Machine, MachineBackend};
use wsp_engine::machine_facts::{mem_mb_of, read_values, MEM_READ};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::golden_image::{GoldenManifest, GoldenVersion, RecipeDigest, SealedImage};
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_more::{ProjectRef, WorkspaceView};
use wsp_wire::wire::capabilities::{MachineSizeOffer, WorkspaceSize};
use wsp_wire::wire::machine_link::MachineShape;
use wsp_words::exit::HostError;
use wsp_words::js;
use wsp_words::words::units::size_word;

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

fn json_of<T: Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).expect("a record serializes as JSON")
}

fn set_look<T: serde::de::DeserializeOwned>(
    slot: &mut Option<T>,
    value: Option<serde_json::Value>,
) {
    match value {
        None => *slot = None,
        Some(v) => {
            if let Ok(v) = serde_json::from_value(v) {
                *slot = Some(v);
            }
        }
    }
}

impl RecordsArea {
    pub fn new(ctx: Rc<dyn RecordsCtx>) -> RecordsArea {
        RecordsArea { ctx }
    }

    /// records.ts:88-125 (R1, R2).
    pub fn view(&self, r: &WorkspaceRecord) -> WorkspaceView {
        let project = self.ctx.project_ref(&r.project);
        WorkspaceView {
            id: r.id.clone(),
            name: r.name.clone(),
            machine_id: r.machine_id.clone(),
            phase: r.phase,
            kind: Some(r.kind),
            golden: r.golden.clone(),
            created_at: r.created_at.clone(),
            project: ProjectRef {
                id: project.id,
                name: project.name,
                path: project.path,
                computer: project.computer,
            },
            folder: self.ctx.module_folder(r),
            home: self.ctx.home_of(r),
            claude_session_id: r.claude_session_id.clone(),
            queue: r.queue.clone(),
            screen: r.screen.clone(),
            gone: r.gone.clone(),
            theme: r.theme.clone(),
            glyph: r.glyph,
            daemon_note: self.ctx.daemon_note(&r.id),
            vaulted_at: r.vaulted_at.clone(),
            vault_refused: r.vault_refused.clone(),
            daemon_refused_at: r.daemon_refused_at.clone(),
            wake_refused: r.wake_refused.clone(),
            disk_gone: r.disk_gone.clone(),
            agents: self
                .ctx
                .agents_of(r)
                .and_then(|v| serde_json::from_value(v).ok()),
            parent_thread_id: r.parent_thread_id.clone(),
            root_thread_id: r.root_thread_id.clone(),
            parent_workspace_id: r.parent_workspace_id.clone(),
            place: r.place.clone(),
            worktree: r.worktree.clone(),
            provider: self.ctx.provider_of(r),
            pause_mode: self
                .ctx
                .backend_for(r)
                .ok()
                .and_then(|b| b.capabilities().pause_mode),
            from: r.from.clone(),
            review: r.review.clone(),
            branch_of_record: r.branch_of_record.clone(),
        }
    }

    /// records.ts:129-133 (R3): `None` leaves, `Some(None)` clears, `Some(Some(v))` sets the look fact, as its JSON.
    pub fn put_look(
        &self,
        r: &mut WorkspaceRecord,
        key: LookKey,
        value: Option<Option<serde_json::Value>>,
    ) {
        let Some(value) = value else { return };
        match key {
            LookKey::Theme => set_look(&mut r.theme, value),
            LookKey::Glyph => set_look(&mut r.glyph, value),
        }
    }

    /// records.ts:135-142 (R4).
    pub async fn persist(&self, r: &WorkspaceRecord) -> Result<(), HostError> {
        self.ctx.bump_generation(&r.id);
        let store = self.ctx.store();
        store.put(WORKSPACES, &r.id, json_of(r)).await?;
        let name = NamedWorkspace {
            workspace_id: r.id.clone(),
            name: r.name.clone(),
            project: Some(r.project.clone()),
        };
        store.put(WORKSPACE_NAMES, &r.id, json_of(&name)).await
    }

    /// records.ts:144-147 (R5).
    pub async fn shape_of(&self, m: Arc<dyn Machine>) -> Option<MachineShape> {
        m.describe().await.ok()
    }

    /// records.ts:150-153 (R6).
    pub fn size_built(&self, shape: Option<&MachineShape>, asked: &WorkspaceSize) -> WorkspaceSize {
        WorkspaceSize {
            cpu: shape.and_then(|s| s.cpu).unwrap_or(asked.cpu),
            mem_mb: shape.and_then(|s| s.mem_mb).unwrap_or(asked.mem_mb),
        }
    }

    /// records.ts:157-170 (R7, R8).
    pub async fn read_memory(
        &self,
        record: &mut WorkspaceRecord,
        machine: Arc<dyn Machine>,
        sizes: &[MachineSizeOffer],
    ) -> bool {
        let outcome: Result<Option<u64>, String> = match machine
            .exec(
                MEM_READ,
                ExecOptions {
                    timeout_ms: Some(INLINE_EXEC_MS),
                    ..Default::default()
                },
            )
            .await
        {
            Ok(res) if res.exit_code.get() == 0 => Ok(mem_mb_of(&read_values(&res.stdout))),
            Ok(res) => {
                let stderr = js::js_trim(&res.stderr);
                let tail = if stderr.is_empty() {
                    String::new()
                } else {
                    format!(": {}", js::utf16::tail(stderr, 200))
                };
                Err(format!("exit {}{tail}", res.exit_code.get()))
            }
            Err(e) => Err(e.to_string()),
        };
        let count = match outcome {
            Ok(Some(count)) => count as f64,
            Ok(None) => return false,
            Err(reason) => {
                self.ctx.warn(&format!(
                    "workspace {}: memory not read on {} ({reason}); the row keeps {}",
                    record.id,
                    machine.id(),
                    size_word(&record.size)
                ));
                return false;
            }
        };
        let offered = sizes
            .iter()
            .find(|s| (s.mem_mb.0 - count).abs() <= s.mem_mb.0 / 16.0);
        let largest = sizes.iter().map(|s| s.mem_mb.0).fold(0.0, f64::max);
        let mem = match offered {
            Some(s) => s.mem_mb.0,
            None if largest > 0.0 => count.min(largest),
            None => count,
        };
        record.size.mem_mb = JsNumber(mem);
        true
    }

    /// records.ts:173 (R9).
    pub fn forked_from(&self, snapshot_id: &str) -> Vec<String> {
        self.ctx
            .live_records()
            .iter()
            .filter(|r| r.borrow().golden == snapshot_id)
            .map(|r| r.borrow().name.clone())
            .collect()
    }

    /// records.ts:179 (R10).
    pub async fn copy_of(
        &self,
        place: &str,
        name: &str,
    ) -> Result<Option<GoldenManifest>, HostError> {
        let stored = self
            .ctx
            .store()
            .get(GOLDENS, &copy_key(place, name))
            .await?;
        Ok(stored.and_then(|v| serde_json::from_value(v).ok()))
    }

    /// records.ts:181 (R10).
    pub async fn put_copy(
        &self,
        place: &str,
        name: &str,
        manifest: &GoldenManifest,
    ) -> Result<(), HostError> {
        self.ctx
            .store()
            .put(GOLDENS, &copy_key(place, name), json_of(manifest))
            .await
    }

    /// records.ts:182-184 (R10).
    pub async fn copy_recipe_of(
        &self,
        place: &str,
        name: &str,
        version: u32,
    ) -> Result<Option<RecipeDigest>, HostError> {
        let stored = self
            .ctx
            .store()
            .get(GOLDEN_RECIPES, &copy_key(place, &recipe_key(name, version)))
            .await?;
        Ok(stored.and_then(|v| serde_json::from_value(v).ok()))
    }

    /// records.ts:185-186 (R10).
    pub async fn put_copy_recipe(
        &self,
        place: &str,
        name: &str,
        version: u32,
        digest: &RecipeDigest,
    ) -> Result<(), HostError> {
        self.ctx
            .store()
            .put(
                GOLDEN_RECIPES,
                &copy_key(place, &recipe_key(name, version)),
                json_of(digest),
            )
            .await
    }

    /// records.ts:187-190 (R10).
    pub async fn drop_copy_recipe(
        &self,
        place: &str,
        name: &str,
        version: u32,
    ) -> Result<(), HostError> {
        self.ctx
            .store()
            .delete(GOLDEN_RECIPES, &copy_key(place, &recipe_key(name, version)))
            .await
    }

    /// records.ts:226-228 (R11).
    pub async fn record_of(&self, name: &str) -> Result<Option<SealedImage>, HostError> {
        let stored = self.ctx.store().get(IMAGES, name).await?;
        Ok(stored.and_then(|v| serde_json::from_value(v).ok()))
    }

    /// records.ts:248-258 (R12).
    pub async fn recorded_images(&self) -> Result<IndexSet<String>, HostError> {
        let store = self.ctx.store();
        let mut ids = IndexSet::new();
        for raw in store.list(GOLDENS).await? {
            let Ok(manifest) = serde_json::from_value::<GoldenManifest>(raw) else {
                continue;
            };
            for v in manifest.versions {
                ids.insert(v.snapshot_id);
                if let Some(template) = v.template_id {
                    ids.insert(template);
                }
            }
        }
        for r in self.ctx.live_records() {
            ids.insert(r.borrow().golden.clone());
        }
        Ok(ids)
    }

    /// records.ts:261-277 (R13).
    pub async fn image_of(&self, snapshot_id: &str) -> Result<ImageOf, HostError> {
        let store = self.ctx.store();
        for raw in store.list(GOLDENS).await? {
            let Ok(manifest) = serde_json::from_value::<GoldenManifest>(raw) else {
                continue;
            };
            if let Some(version) = manifest
                .versions
                .into_iter()
                .find(|v| v.snapshot_id == snapshot_id)
            {
                return Ok(ImageOf {
                    golden: snapshot_id.to_string(),
                    version: Some(version),
                });
            }
        }
        Ok(ImageOf {
            golden: snapshot_id.to_string(),
            version: None,
        })
    }
}
