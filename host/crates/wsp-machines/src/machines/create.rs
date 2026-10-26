// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/machines/create.ts` at 65f6b89c7: the create of one workspace, the name rules, and the
//! creates a window holds before their record lands. Spec: spec/create.md.

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Value};

use crate::places::types::PlaceDoor;
use wsp_core::types::internal::{BuilderLife, KindModule};
use wsp_core::types::pending::Clock;
use wsp_core::types::wiring::{
    until, CreateWorkspaceOptions, CreatedWorkspace, IdleWindow, RecordSpec, StageReport,
    StageSaid, WorkspaceRecord,
};
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::views::place::{MachineBind, WorkspaceCopy};
use wsp_wire::views::workspace::WorkspaceAgentsPatch;
use wsp_wire::views::workspace::{agents_from, BranchOfRecord, ProjectView};
use wsp_wire::views::workspace_events::{WorkspaceCreateStage, WorkspaceCreatingEvent};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ProjectRef, ThreadScope, WorkspaceAgents, WorkspaceView};
use wsp_wire::wire::capabilities::{names_size, WorkspaceSize};
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::{HostError, Kind};
use wsp_words::js::js_trim;
use wsp_words::js_date::iso_string;
use wsp_words::projects::runs_in_folder;
use wsp_words::words::thread::{
    copy_first_line, name_deleting_refusal, name_taken_refusal, starting_line, BLANK_NAME_REFUSAL,
    CREATE_READY,
};
use wsp_words::words::units::{
    machine_cap_refusal, offered_size, size_got_line, size_refusal, size_word, SIZE_PICK_FIX,
};

use super::machines::{Entry, Ids};
use crate::unwound::unwound_or_failed;

/// A create as a window holds it before its record lands (context.ts:542 `ShownCreate`).
pub type ShownCreate = WorkspaceView;

/// The road that landed a project on its computer, as a create reads it (project-landing.ts `ProjectLanding`; a
/// wsp-core type once its slice lands).
pub trait LandingRoad {
    /// `"provider"`, `"box"`, ...
    fn kind(&self) -> String;
    fn refusal(&self, project: &ProjectView, deps: &serde_json::Value) -> Option<String>;
    /// `places({project, memoryKey, deps}).memoryDir`.
    fn memory_dir(
        &self,
        project: &ProjectView,
        memory_key: &str,
        deps: &serde_json::Value,
    ) -> String;
    fn workspace_binds(&self, project: &ProjectView) -> Vec<MachineBind>;
}

/// `namedOfRecord`'s question (branch-of-record.ts:139).
#[derive(Clone, Debug, PartialEq)]
pub struct RecordBranchAsk {
    pub remote: String,
    pub name: String,
    pub project: String,
    pub on: String,
    pub base: Option<String>,
    pub carried: bool,
}

/// A builder as the cap road reads it (create.ts:131): its id, name, life, the version it sealed and its place.
#[derive(Clone, Debug, PartialEq)]
pub struct KeptBuilder {
    pub id: String,
    pub name: String,
    pub life: BuilderLife,
    pub sealed_version: Option<u32>,
    pub place: Option<String>,
}

/// Every call the create area makes on the rest of the runtime.
#[async_trait(?Send)]
pub trait CreateRuntime {
    fn clock(&self) -> Rc<dyn Clock>;
    fn ids(&self) -> Rc<dyn Ids>;
    fn emit(&self, event: EventUnion) -> Result<(), HostError>;
    fn warn(&self, line: &str);
    fn live(&self) -> Rc<RefCell<IndexMap<String, Entry>>>;
    /// `places.wired`.
    fn wired(&self) -> String;
    fn place_door(&self) -> Option<Rc<dyn PlaceDoor>>;
    async fn landing_place(&self, computer: &str) -> Result<Option<String>, HostError>;
    async fn place_refuses(&self, place_id: Option<&str>) -> Result<(), HostError>;
    async fn landing_backend(
        &self,
        place_id: Option<&str>,
    ) -> Result<Arc<dyn MachineBackend>, HostError>;
    fn landing_kind(&self, computer: &str, at: &Arc<dyn MachineBackend>) -> String;
    async fn landing_deps(&self, computer: &str) -> Result<serde_json::Value, HostError>;
    fn project_landing(&self, kind: &str) -> Result<Rc<dyn LandingRoad>, HostError>;
    async fn remember_project(&self, project: ProjectView) -> Result<(), HostError>;
    fn project_held(&self, project_id: &str) -> Result<ProjectView, HostError>;
    fn keeps_images(&self, at: &Arc<dyn MachineBackend>) -> bool;
    async fn copy_for_fork(
        &self,
        golden: &str,
        place_id: Option<&str>,
        building: &dyn Fn(&str, f64),
    ) -> Result<String, HostError>;
    async fn image_head(&self) -> Result<String, HostError>;
    /// `imageOf(golden).version?.size`.
    async fn image_size(
        &self,
        golden: &str,
    ) -> Result<Option<wsp_wire::wire::capabilities::WorkspaceSize>, HostError>;
    fn brokered_start(&self, provider: bool) -> bool;
    async fn named_of_record(&self, ask: RecordBranchAsk) -> Result<BranchOfRecord, HostError>;
    fn place_name(&self, place: &str) -> String;
    /// `ctx.placeDoorOf().nameOf(place)`.
    fn place_door_name(&self, place: &str) -> String;
    fn spawn_at(&self, place: &str) -> WorkspaceAgents;
    /// `ctx.treeOf(spawned)`: `{rootThreadId, threadId}` written on the record, or nothing.
    fn tree_of(&self, spawned: Option<&ThreadScope>) -> serde_json::Map<String, serde_json::Value>;
    /// The machines area's `attach`.
    fn attach(
        &self,
        record: &Rc<RefCell<WorkspaceRecord>>,
        machine: Arc<dyn Machine>,
    ) -> Result<Entry, HostError>;
    /// The machines area's `fork` with no override and `readsMemory` false; `bind` runs before the claim is let go.
    async fn fork(
        &self,
        record: &Rc<RefCell<WorkspaceRecord>>,
        bind: &dyn for<'m, 'r> Fn(
            &'m Arc<dyn Machine>,
            &'r WorkspaceRecord,
        ) -> Result<(), HostError>,
        report: StageReport,
    ) -> Result<Arc<dyn Machine>, HostError>;
    async fn refresh_builders(&self) -> Result<(), HostError>;
    /// The builders in map order, as the cap road reads them.
    fn builders(&self) -> Vec<KeptBuilder>;
    /// `ctx.graceTimers.get(id)?.(); ctx.graceTimers.delete(id)`.
    fn cancel_grace(&self, builder_id: &str);
    /// `killUntilGone(ctx.backendAt(place ?? wired), machine, opts.killConfirm)`.
    async fn kill_builder(&self, builder_id: &str) -> Result<(), HostError>;
    async fn forget_builder(&self, builder_id: &str) -> Result<(), HostError>;
    fn holds_slot(&self, record: &WorkspaceRecord) -> bool;
    fn served_by_its_computer(&self, entry: &Entry) -> Option<String>;
    /// `ctx.lifecycleOf(entry).budgets.daemonAnswersMs`.
    fn daemon_answers_ms(&self, entry: &Entry) -> Result<u64, HostError>;
    /// `entry.ws.daemonReach()`.
    async fn daemon_reach(&self, entry: &Entry) -> Result<(), HostError>;
    async fn ping_daemon(&self, entry: &Entry) -> Option<String>;
    fn sync_daemon(&self, entry: &Entry);
    async fn read_memory(
        &self,
        record: &Rc<RefCell<WorkspaceRecord>>,
        machine: &Arc<dyn Machine>,
        sizes: &[wsp_wire::wire::capabilities::WorkspaceSize],
    ) -> bool;
    fn module_of(&self, kind: &str) -> Result<Rc<dyn KindModule>, HostError>;
    async fn write_daemon_roots(&self, entry: &Entry) -> Result<(), HostError>;
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError>;
    async fn mark_used(&self, place_id: Option<&str>) -> Result<(), HostError>;
    fn view(&self, record: &WorkspaceRecord) -> WorkspaceView;
    fn ref_of(&self, project: &ProjectView) -> ProjectRef;
}

/// The create area: the name holds and the creates create.ts keeps in its closure.
pub struct CreateArea {
    pub(crate) rt: Rc<dyn CreateRuntime>,
    pub(crate) me: Weak<CreateArea>,
    /// `forking`: names whose fork is between its check and its first machine.
    pub forking: RefCell<IndexSet<String>>,
    /// `failedCreates`: creates that failed before any machine was recorded.
    pub failed_creates: Rc<RefCell<IndexMap<String, ShownCreate>>>,
    /// `shownCreates`: forks a thread asked for, shown under it before their record lands.
    pub shown_creates: Rc<RefCell<IndexMap<String, ShownCreate>>>,
    /// `createStages`: each create in flight's last stage, without its `seq`.
    pub create_stages: RefCell<IndexMap<String, WorkspaceCreatingEvent>>,
    pub(crate) folder_making: RefCell<IndexMap<String, FolderJoin>>,
}

/// A folder record being made, shared by every ask for its key.
pub(crate) type FolderJoin = Shared<LocalBoxFuture<'static, Result<Entry, HostError>>>;

/// `size`'s cpu and memory as the offers name them, for the read of the machine's memory.
fn sizes_of(offers: &[wsp_wire::wire::capabilities::MachineSizeOffer]) -> Vec<WorkspaceSize> {
    offers
        .iter()
        .map(|o| WorkspaceSize {
            cpu: o.cpu,
            mem_mb: o.mem_mb,
        })
        .collect()
}

/// `s.charAt(0).toLowerCase() + s.slice(1, -1)`: the first letter lowered and the last character dropped.
fn lowered_without_last(s: &str) -> String {
    let mut chars: Vec<char> = s.chars().collect();
    if let Some(first) = chars.first_mut() {
        *first = first.to_lowercase().next().unwrap_or(*first);
    }
    chars.pop();
    chars.into_iter().collect()
}

fn entry_name(entry: &Entry) -> String {
    entry.borrow().record.borrow().name.clone()
}

/// The first item of `start`'s run and its join: runs the synchronous part now, then keeps the rest on the local tasks.
fn start_join<T: Clone + 'static>(
    join: Shared<LocalBoxFuture<'static, T>>,
) -> Shared<LocalBoxFuture<'static, T>> {
    let mut first = join.clone();
    let _ = (&mut first).now_or_never();
    let keep = join.clone();
    wsp_engine::local_task::try_spawn_local(async move {
        let _ = keep.await;
    });
    join
}

impl CreateArea {
    /// `createArea(ctx)` (create.ts:11).
    pub fn new(rt: Rc<dyn CreateRuntime>) -> Rc<Self> {
        Rc::new_cyclic(|me: &Weak<CreateArea>| CreateArea {
            rt,
            me: me.clone(),
            forking: RefCell::default(),
            failed_creates: Rc::default(),
            shown_creates: Rc::default(),
            create_stages: RefCell::default(),
            folder_making: RefCell::default(),
        })
    }

    /// `createStaged` (create.ts:13), spec rule 1.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_staged(
        self: &Rc<Self>,
        o: &CreateWorkspaceOptions,
        project: &ProjectView,
        id: &str,
        report: StageReport,
        spawned: Option<&ThreadScope>,
        landed: Option<Box<dyn Fn()>>,
        child_base: Option<&str>,
    ) -> Result<CreatedWorkspace, HostError> {
        let rt = self.rt.clone();
        let place_id = rt.landing_place(&project.computer).await?;
        rt.place_refuses(place_id.as_deref()).await?;
        let at = rt.landing_backend(place_id.as_deref()).await?;
        let road = rt.project_landing(&rt.landing_kind(&project.computer, &at))?;
        let landing_on = rt.landing_deps(&project.computer).await?;
        if let Some(refused) = road.refusal(project, &landing_on) {
            return Err(HostError::with_kind(refused, Kind::Invalid));
        }
        let memory_dir = road.memory_dir(project, &project.memory_key, &landing_on);
        if memory_dir != project.memory_dir {
            rt.remember_project(ProjectView {
                memory_dir,
                ..project.clone()
            })
            .await?;
        }
        let binds = road.workspace_binds(&rt.project_held(&project.id)?);
        let copy = project.checkout.as_ref().map(|from| WorkspaceCopy {
            from: from.clone(),
            at: project.path.clone(),
        });
        let from_image = rt.keeps_images(&at);
        let golden = if from_image {
            let head = match &o.golden {
                Some(golden) => golden.clone(),
                None => rt.image_head().await?,
            };
            let building = |where_: &str, rate: f64| {
                report(
                    WorkspaceCreateStage::ForkRequested,
                    &copy_first_line(where_, &o.name, rate),
                    None,
                )
            };
            rt.copy_for_fork(&head, place_id.as_deref(), &building)
                .await?
        } else {
            String::new()
        };
        let inherited = if from_image {
            rt.image_size(&golden).await?
        } else {
            None
        };
        let brokered = rt.brokered_start(road.kind() == "provider");
        let carried = child_base.is_none() && project.checkout.is_some();
        let on_place = place_id.clone().unwrap_or_else(|| rt.wired());
        let branch_of_record = if brokered {
            match &o.branch_of_record {
                Some(branch) => Some(branch.clone()),
                None => Some(
                    rt.named_of_record(RecordBranchAsk {
                        remote: project.remote.clone(),
                        name: o.name.clone(),
                        project: project.name.clone(),
                        on: rt.place_name(&on_place),
                        base: child_base
                            .map(str::to_string)
                            .or_else(|| project.base.clone()),
                        carried,
                    })
                    .await?,
                ),
            }
        } else {
            None
        };
        let opens_thread = o.opens_thread.clone().or_else(|| {
            if brokered && spawned.is_none() {
                Some(rt.ids().uuid())
            } else {
                None
            }
        });
        let default = at.pricing().default_size();
        let size = WorkspaceSize {
            cpu: o
                .spec
                .cpu
                .or_else(|| inherited.as_ref().map(|s| s.cpu))
                .unwrap_or(JsNumber(default.cpu)),
            mem_mb: o
                .spec
                .mem_mb
                .or_else(|| inherited.as_ref().map(|s| s.mem_mb))
                .unwrap_or(JsNumber(default.mem_mb)),
        };
        let mut record: WorkspaceRecord = serde_json::from_value(json!({
            "id": id,
            "name": o.name,
            "kind": "cloud",
            "machineId": "",
            "phase": "running",
            "golden": golden,
            "createdAt": iso_string(rt.clock().now() as f64).unwrap_or_default(),
            "project": project.id,
            "spec": {},
            "size": {"cpu": size.cpu.0, "memMb": size.mem_mb.0},
            "firstLife": true,
        }))
        .map_err(|e| HostError::new(e.to_string()))?;
        record.spec = RecordSpec {
            envs: o.spec.envs.clone(),
            labels: o.spec.labels.clone(),
            binds: if binds.is_empty() { None } else { Some(binds) },
            copy,
        };
        record.idle_window_ms = match o.idle_window_ms {
            IdleWindow::Default => None,
            IdleWindow::Off => Some(None),
            IdleWindow::Ms(ms) => Some(Some(ms)),
        };
        record.place = place_id.clone();
        for (key, value) in rt.tree_of(spawned) {
            match key.as_str() {
                "parentThreadId" => record.parent_thread_id = value.as_str().map(str::to_string),
                "rootThreadId" => record.root_thread_id = value.as_str().map(str::to_string),
                _ => {}
            }
        }
        record.parent_workspace_id = o.parent.clone();
        record.base = child_base
            .map(str::to_string)
            .or_else(|| project.base.clone());
        record.branch_of_record = branch_of_record;
        if spawned.is_none() {
            if let Some(patch) = &o.agents {
                let count = |key: &str| {
                    patch
                        .get(key)
                        .and_then(Value::as_f64)
                        .map(|n| JsInt(JsNumber(n)))
                };
                let patch = WorkspaceAgentsPatch {
                    max_machines: count("maxMachines"),
                    max_depth: count("maxDepth"),
                };
                record.agents = Some(agents_from(Some(&rt.spawn_at(&on_place)), &patch));
            }
        }
        let offers = at.capabilities().sizes;
        if names_size(o.spec.cpu.map(|c| c.0), o.spec.mem_mb.map(|m| m.0))
            && !offered_size(&offers, &record.size)
        {
            let line = wsp_words::words::thread::refusal_line(
                &size_refusal(&size_word(&record.size), &offers),
                SIZE_PICK_FIX,
            );
            return Err(HostError::with_kind(line, Kind::Invalid));
        }
        let asked = record.size.clone();
        let where_ = match &place_id {
            None => rt.wired(),
            Some(place) => rt.place_door_name(place),
        };
        report(
            WorkspaceCreateStage::ForkRequested,
            &starting_line(&record.name, &where_),
            None,
        );
        let cell = Rc::new(RefCell::new(record));
        let opens = opens_thread.clone();
        let of_record = o.branch_of_record.is_some();
        let bind = |m: &Arc<dyn Machine>, _r: &WorkspaceRecord| -> Result<(), HostError> {
            cell.borrow_mut().machine_id = m.id().to_string();
            let entry = rt.attach(&cell, m.clone())?;
            {
                let mut live = entry.borrow_mut();
                live.creating = true;
                if let Some(thread) = &opens {
                    live.opens_thread = Some(thread.clone());
                }
                if of_record {
                    live.of_record_held = true;
                }
            }
            if let Some(landed) = &landed {
                landed();
            }
            Ok(())
        };
        let mut notices: Vec<String> = Vec::new();
        let first = rt.fork(&cell, &bind, report.clone()).await;
        let machine = match first {
            Ok(machine) => machine,
            Err(e) => {
                if e.kind != Some(Kind::Concurrency) {
                    return Err(e);
                }
                let mut refusal = e;
                let mut made: Option<Arc<dyn Machine>> = None;
                rt.refresh_builders().await?;
                let candidates: Vec<KeptBuilder> = rt
                    .builders()
                    .into_iter()
                    .filter(|x| {
                        matches!(x.life, BuilderLife::Own | BuilderLife::Reusable)
                            && x.sealed_version.is_some()
                    })
                    .collect();
                for x in candidates {
                    let version = x.sealed_version.unwrap_or_default();
                    let stopped = format!("Stopped the builder kept from image v{version} to make room at the machine cap.");
                    rt.cancel_grace(&x.id);
                    rt.kill_builder(&x.id).await?;
                    rt.forget_builder(&x.id).await?;
                    notices.push(stopped.clone());
                    rt.warn(&format!(
                        "workspace {id}: {} ({})",
                        lowered_without_last(&stopped),
                        x.id
                    ));
                    let again = format!("{} again", starting_line(&o.name, &where_));
                    report(
                        WorkspaceCreateStage::ForkRequested,
                        &again,
                        Some(StageSaid {
                            notice: Some(stopped),
                            ..StageSaid::default()
                        }),
                    );
                    match rt.fork(&cell, &bind, report.clone()).await {
                        Ok(machine) => {
                            made = Some(machine);
                            break;
                        }
                        Err(again) => {
                            if again.kind != Some(Kind::Concurrency) {
                                return Err(again);
                            }
                            refusal = again;
                        }
                    }
                }
                match made {
                    Some(machine) => machine,
                    None => {
                        let holding: Vec<String> = rt
                            .live()
                            .borrow()
                            .values()
                            .filter(|w| rt.holds_slot(&w.borrow().record.borrow()))
                            .map(entry_name)
                            .collect();
                        let names: Vec<String> =
                            rt.builders().into_iter().map(|x| x.name).collect();
                        let line = machine_cap_refusal(&holding, &names, Some(&refusal.message));
                        return Err(HostError {
                            status: refusal.status,
                            ..HostError::with_kind(line, Kind::Concurrency)
                        });
                    }
                }
            }
        };
        let entry = rt
            .live()
            .borrow()
            .get(id)
            .cloned()
            .ok_or_else(|| HostError::new(format!("{id} is not live")))?;
        let record_cell = entry.borrow().record.clone();
        record_cell.borrow_mut().machine_id = machine.id().to_string();
        if let Some(notice) = entry.borrow().machine.notice() {
            notices.push(notice.to_string());
        }
        let machine_obj = entry.borrow().machine.clone();
        let mut fault: Option<String> = None;
        let offers_now = machine_obj.offers();
        if rt.served_by_its_computer(&entry).is_none()
            && (offers_now.preview_url || offers_now.daemon_answers)
        {
            if offers_now.preview_url {
                // create.ts:169: the lifecycle read sits in the try whose catch reports the route as not minted.
                let reached = match rt.daemon_answers_ms(&entry) {
                    Ok(answers_ms) => {
                        let deadline = rt.clock().now() + answers_ms as i64;
                        until(
                            rt.daemon_reach(&entry),
                            deadline,
                            "preview route",
                            rt.clock(),
                        )
                        .await
                        .map_err(|e| match e {
                            wsp_core::types::wiring::UntilError::Deadline(d) => d.0,
                            wsp_core::types::wiring::UntilError::Failed(f) => f.message,
                        })
                    }
                    Err(e) => Err(e.message),
                };
                match reached {
                    Ok(()) => report(
                        WorkspaceCreateStage::PreviewRoute,
                        "Preview route to the daemon minted.",
                        None,
                    ),
                    Err(why) => {
                        let notice =
                            format!("preview route for {} not minted ({why})", machine_obj.id());
                        report(
                            WorkspaceCreateStage::PreviewRoute,
                            "No preview route to the daemon.",
                            Some(StageSaid {
                                notice: Some(notice),
                                ..StageSaid::default()
                            }),
                        );
                    }
                }
            }
            fault = rt.ping_daemon(&entry).await;
            let message = if fault.is_none() {
                "Daemon answered."
            } else {
                "Daemon did not answer."
            };
            report(
                WorkspaceCreateStage::DaemonAnswering,
                message,
                Some(StageSaid {
                    notice: fault.clone(),
                    ..StageSaid::default()
                }),
            );
            rt.sync_daemon(&entry);
        }
        if place_id.is_none() {
            if fault.is_some() {
                rt.warn(&format!(
                    "workspace {id}: memory not read: the daemon did not answer"
                ));
            } else if rt
                .read_memory(&record_cell, &machine_obj, &sizes_of(&offers))
                .await
            {
                let got = record_cell.borrow().size.clone();
                if got.cpu.0 != asked.cpu.0 || got.mem_mb.0 != asked.mem_mb.0 {
                    let line = size_got_line(
                        &asked,
                        &got,
                        names_size(o.spec.cpu.map(|c| c.0), o.spec.mem_mb.map(|m| m.0)),
                    );
                    rt.warn(&format!("workspace {id}: {line}"));
                    notices.push(line);
                }
            }
        }
        let kind = record_cell.borrow().kind;
        let kind_name = serde_json::to_value(kind)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        let landing = rt.module_of(&kind_name)?.land_project(
            &entry.borrow(),
            project.clone(),
            report.clone(),
        );
        landing.await?;
        rt.write_daemon_roots(&entry).await?;
        let snapshot = record_cell.borrow().clone();
        rt.persist(&snapshot).await?;
        rt.mark_used(place_id.as_deref()).await?;
        entry.borrow_mut().creating = false;
        report(WorkspaceCreateStage::Ready, CREATE_READY, None);
        let view = rt.view(&snapshot);
        let created = serde_json::to_value(&view).map_err(|e| HostError::new(e.to_string()))?;
        let event: EventUnion =
            serde_json::from_value(json!({"type": "workspace.created", "workspace": created}))
                .map_err(|e| HostError::new(e.to_string()))?;
        rt.emit(event)?;
        Ok(CreatedWorkspace {
            view,
            notice: if notices.is_empty() {
                None
            } else {
                Some(notices.join(" "))
            },
            sub: None,
        })
    }

    /// `nameGiven` (create.ts:207): JavaScript's `trim()`.
    pub fn name_given(name: &str) -> String {
        js_trim(name).to_string()
    }

    /// `nameRefusal` (create.ts:271), spec rule 3.
    pub fn name_refusal(&self, name: &str) -> Option<String> {
        if Self::name_given(name).is_empty() {
            return Some(BLANK_NAME_REFUSAL.to_string());
        }
        let held = self
            .rt
            .live()
            .borrow()
            .values()
            .find(|entry| entry_name(entry) == name)
            .cloned();
        if let Some(entry) = held {
            let deleting = entry.borrow().deleting.is_some();
            return Some(if deleting {
                name_deleting_refusal(name)
            } else {
                name_taken_refusal(name)
            });
        }
        if self.forking.borrow().contains(name) {
            return Some(name_taken_refusal(name));
        }
        None
    }

    /// `failedView` (create.ts:229), spec rule 4. `began` is epoch ms.
    #[allow(clippy::too_many_arguments)]
    pub fn failed_view(
        &self,
        id: &str,
        name: &str,
        kind: WorkspaceKind,
        golden: &str,
        began: i64,
        project: &ProjectView,
        said: &str,
    ) -> ShownCreate {
        let view = json!({
            "id": id,
            "name": name,
            "machineId": "",
            "phase": "gone",
            "kind": kind,
            "golden": golden,
            "createdAt": iso_string(began as f64).unwrap_or_default(),
            "project": self.rt.ref_of(project),
            "gone": said,
        });
        serde_json::from_value(view).expect("a failed create reads as a workspace view")
    }

    /// `supersedeFailed` (create.ts:241), spec rule 5.
    pub fn supersede_failed(&self, name: &str) {
        let superseded: Vec<String> = self
            .failed_creates
            .borrow()
            .iter()
            .filter(|(_, failed)| failed.name == name)
            .map(|(id, _)| id.clone())
            .collect();
        for failed_id in superseded {
            self.failed_creates.borrow_mut().shift_remove(&failed_id);
            let _ = self.rt.emit(EventUnion::WorkspaceDeleted(
                serde_json::from_value(
                    json!({"type": "workspace.deleted", "workspaceId": failed_id}),
                )
                .expect("a deleted event reads"),
            ));
        }
    }

    /// `stageReporter` (create.ts:251), spec rule 6.
    pub fn stage_reporter(
        self: &Rc<Self>,
        id: &str,
        name: &str,
        began: i64,
        spawned: Option<&ThreadScope>,
    ) -> StageReport {
        let asked_by = spawned.map(
            |scope| json!({"threadId": scope.thread_id, "rootThreadId": scope.root_thread_id}),
        );
        let rt = self.rt.clone();
        let (id, name) = (id.to_string(), name.to_string());
        Rc::new(
            move |stage: WorkspaceCreateStage, message: &str, said: Option<StageSaid>| {
                let mut event = json!({
                    "type": "workspace.creating",
                    "workspaceId": id,
                    "name": name,
                    "stage": stage,
                    "message": message,
                    "elapsedMs": rt.clock().now() - began,
                });
                if let Some(said) = said {
                    if let Some(notice) = said.notice {
                        event["notice"] = json!(notice);
                    }
                    if let Some(detail) = said.detail {
                        event["detail"] = json!(detail);
                    }
                    if said.waiting {
                        event["waiting"] = json!(true);
                    }
                }
                if let Some(asked_by) = &asked_by {
                    event["askedBy"] = asked_by.clone();
                }
                if let Ok(event) = serde_json::from_value::<EventUnion>(event) {
                    let _ = rt.emit(event);
                }
            },
        )
    }

    /// The `*` listener (create.ts:221), spec rule 7: the wiring calls it for every event in emit order.
    pub fn on_event(&self, event: &EventUnion) {
        match event {
            EventUnion::WorkspaceCreating(stage) => {
                self.create_stages
                    .borrow_mut()
                    .insert(stage.event.workspace_id.clone(), stage.event.clone());
            }
            EventUnion::WorkspaceCreated(created) => {
                self.create_stages
                    .borrow_mut()
                    .shift_remove(&created.event.workspace.id);
            }
            EventUnion::WorkspaceDeleted(deleted) => {
                self.create_stages
                    .borrow_mut()
                    .shift_remove(&deleted.event.workspace_id);
            }
            _ => {}
        }
    }

    /// `showCreate` (create.ts:216), spec rule 8.
    pub fn show_create(&self, view: ShownCreate) {
        self.shown_creates
            .borrow_mut()
            .insert(view.id.clone(), view.clone());
        if let Ok(view_json) = serde_json::to_value(&view) {
            if let Ok(event) = serde_json::from_value::<EventUnion>(
                json!({"type": "workspace.created", "workspace": view_json}),
            ) {
                let _ = self.rt.emit(event);
            }
        }
    }

    /// `foldersOf` (create.ts:279), spec rule 9.
    pub fn folders_of(&self, project_id: &str) -> Vec<Entry> {
        self.rt
            .live()
            .borrow()
            .values()
            .filter(|entry| {
                let record = entry.borrow().record.clone();
                let record = record.borrow();
                runs_in_folder(&record.kind) && record.project == project_id
            })
            .cloned()
            .collect()
    }

    /// `oneFolder` (create.ts:283), spec rule 10: a join per key.
    pub fn one_folder(
        &self,
        key: &str,
        make: Box<
            dyn FnOnce() -> futures::future::LocalBoxFuture<'static, Result<Entry, HostError>>,
        >,
    ) -> futures::future::LocalBoxFuture<'static, Result<Entry, HostError>> {
        if let Some(held) = self.folder_making.borrow().get(key).cloned() {
            return held.boxed_local();
        }
        let me = self.me.clone();
        let owned_key = key.to_string();
        let making: LocalBoxFuture<'static, Result<Entry, HostError>> = async move {
            let made = unwound_or_failed(make()).await;
            if let Some(area) = me.upgrade() {
                area.folder_making.borrow_mut().shift_remove(&owned_key);
            }
            made
        }
        .boxed_local();
        let join = making.shared();
        self.folder_making
            .borrow_mut()
            .insert(key.to_string(), join.clone());
        start_join(join).boxed_local()
    }
}
