// SPDX-License-Identifier: AGPL-3.0-only
use std::cell::RefCell;
use std::path::Path;
use std::rc::{Rc, Weak};

use wsp_account::projects::projects::add::AddAsk;
use wsp_account::projects::projects::door::RemoveWords;
use wsp_core::types::internal::DELETED_REASON;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_machines::agents_read as reads;
use wsp_machines::images::held::HeldImages;
use wsp_machines::machines::machines::Entry;
use wsp_machines::places::helpers::{login_here as shared_login_here, HomesHere};
use wsp_machines::places::types::{FolderMove, FolderStage, HeldRow, PlaceResult, ProjectOn, RowId, SignInEmit, SignInHandle, SpentToday};
use wsp_wire::types::agents_report::{AgentsTarget, AgentsTargetV0};
use wsp_wire::types::js_number::JsInt;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::recipe_file::RecipeFileFoldersValue;
use wsp_wire::views::place::{PlaceProvisionRow, PlaceProvisionRowOutcome, PlaceProvisionRowProject};
use wsp_wire::views::place_more::{PlaceHolds, PlaceHoldsForksItem, PlaceRemovedTook, PlaceView};
use wsp_wire::views::preferences::ProjectLook;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_more::{ProjectRef, WorkspaceView};
use wsp_words::exit::HostError;
use wsp_words::place_state::running_on;
use wsp_words::project_seed::{no_remote_line, seed_choice_from, SeedFlags};
use wsp_words::sub_thread::sub_held_line;
use wsp_words::views::place_setup_words::{folder_move_held_line, folder_move_stands_line, recipe_folder_gone_line};
use wsp_words::views::session::fold_threads;

use super::Ctx;
use crate::runtime::core::{CoreCtx, NamedCount, StandingRows};

/// What the place door reads of this runtime (runtime.ts:600-790).
pub(super) struct Recording {
    ctx: Weak<Ctx>,
}

impl Recording {
    pub(super) fn new(ctx: Weak<Ctx>) -> Recording {
        Recording { ctx }
    }

    fn ctx(&self) -> Rc<Ctx> {
        self.ctx.upgrade().expect("the context is alive while the door asks")
    }

    async fn forget_records(&self, place_id: &str) -> PlaceResult<()> {
        let ctx = self.ctx();
        ctx.booted().await?;
        for entry in live_entries(&ctx) {
            let record = record_of(&entry);
            if record.sub.is_some() || ctx.channels.get().place_id_of(&record).as_deref() != Some(place_id) {
                continue;
            }
            CoreCtx::end_sessions(&*ctx, &record.id, DELETED_REASON);
            CoreCtx::drop_record(&*ctx, &record.id).await?;
        }
        let sub_life = ctx.sub_life.get();
        sub_life.release_subs_on(place_id, true).await.map_err(|e| e.into_host())
    }

    async fn release_records(&self, place_id: &str) -> PlaceResult<()> {
        let ctx = self.ctx();
        ctx.booted().await?;
        let sub_life = ctx.sub_life.get();
        sub_life.release_subs_on(place_id, false).await.map_err(|e| e.into_host())
    }

    async fn sub_holds(&self, place_id: &str) -> PlaceResult<Vec<String>> {
        let ctx = self.ctx();
        ctx.booted().await?;
        let mut lines = Vec::new();
        for entry in live_entries(&ctx) {
            let record = record_of(&entry);
            let Some(sub) = record.sub.as_ref() else { continue };
            if ctx.channels.get().place_id_of(&record).as_deref() != Some(place_id) {
                continue;
            }
            let Some(held) = ctx.sub_life.get().sub_unpushed(&entry) else { continue };
            let thread = ctx.thread_records.borrow().iter().find(|(_, r)| r.workspace_id == record.id).map(|(id, _)| id.clone());
            if let Some(thread) = thread {
                lines.push(sub_held_line(&thread, &held, &sub.branch));
            }
        }
        Ok(lines)
    }

    async fn add_folder_here(
        &self,
        place_id: &str,
        key: &str,
        folder: &RecipeFileFoldersValue,
        mv: Option<&FolderMove>,
        stage: Option<FolderStage>,
    ) -> PlaceResult<HeldRow> {
        let ctx = self.ctx();
        let source = expand_home(&folder.from);
        if !Path::new(&source).exists() {
            return Err(HostError::new(recipe_folder_gone_line(&source)));
        }
        let projects = ctx.projects.get();
        let plan = projects.seed_plan(&source, Some(place_id)).await?;
        if plan.remote.is_none() {
            return Err(HostError::new(no_remote_line(&source)));
        }
        let none: &[&str] = &[];
        let seed = seed_choice_from(&plan, folder.keep.as_slice(), none, &SeedFlags::default())?;
        let old: Option<ProjectView> = mv.and_then(|m| ctx.projects_held.borrow().get(&m.id).cloned());
        if let (Some(m), Some(old)) = (mv, old.as_ref()) {
            if m.pick.from == folder.from && m.pick.name == folder.name {
                let kept: Vec<String> = folder.keep.iter().filter(|path| !m.pick.keep.contains(path)).cloned().collect();
                if !kept.is_empty() {
                    projects.seed_into(&old.id, plan, kept).await?;
                }
                let core = ctx.core.get();
                core.folder_look(&old.id, &m.pick, folder).await?;
                return Ok(installed_row(key, old, None, folder));
            }
        }
        if let Some(old) = old.as_ref() {
            if projects.folder_stands(&old.id).await? {
                let computer = ctx.place_door.get().map(|door| door.name_of(place_id)).unwrap_or_default();
                return Err(HostError::new(folder_move_stands_line(&old.path, &computer)));
            }
            let keep_look = RemoveWords { force: false, check: false, keep_look: true };
            projects
                .remove_with(&old.id, None, keep_look)
                .await
                .map_err(|e| HostError::new(folder_move_held_line(e.message.split('\n').next().unwrap_or_default())))?;
        }
        let created_at = old.as_ref().map(|o| o.created_at.clone()).or_else(|| mv.and_then(|m| m.created_at.clone()));
        let ask = AddAsk {
            source,
            on: Some(place_id.to_string()),
            name: folder.name.clone(),
            base: None,
            into: None,
            seed: Some(seed),
            id: mv.map(|m| m.id.clone()),
            created_at,
            report: stage,
        };
        let added = projects.add(ask, None).await?;
        let project = added.project;
        match mv {
            Some(m) => {
                let core = ctx.core.get();
                core.folder_look(&project.id, &m.pick, folder).await?;
            }
            None => {
                if folder.icon.is_some() || folder.hue.is_some() {
                    let look = ProjectLook { icon: folder.icon, hue: folder.hue };
                    CoreCtx::set_project_look(&*ctx, &project.id, Some(look)).await?;
                }
                let icons = ctx.project_icons.get();
                icons.wear(&project.id, folder.image.as_ref().map(|image| image.0.clone())).await?;
            }
        }
        Ok(installed_row(key, &project, added.notice, folder))
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::places::types::PlaceRecording for Recording {
    async fn projects_on(&self, place_id: &str) -> wsp_machines::places::types::PlaceResult<Vec<wsp_machines::places::types::ProjectOn>> {
        let ctx = self.ctx();
        ctx.booted().await?;
        let held = ctx.projects_held.borrow();
        let projects: Vec<ProjectOn> = held
            .values()
            .filter(|p| p.computer == place_id)
            .map(|p| ProjectOn { id: p.id.clone(), name: p.name.clone(), path: p.path.clone(), checkout: p.checkout.clone() })
            .collect();
        Ok(projects)
    }
    async fn holds_on(
        &self,
        place_id: &str,
        read: bool,
    ) -> wsp_machines::places::types::PlaceResult<wsp_wire::views::place_more::PlaceHolds> {
        let ctx = self.ctx();
        let core = ctx.core.get();
        let (rows, unsaved) = core.holds_on(place_id, read).await?;
        Ok(PlaceHolds { forks: items(rows.forks), projects: items(rows.projects), unsaved, away: None, images: None })
    }
    async fn drop_on(&self, place_id: &str) -> wsp_machines::places::types::PlaceResult<wsp_wire::views::place_more::PlaceRemovedTook> {
        let ctx = self.ctx();
        let core = ctx.core.get();
        let rows = core.drop_on(place_id).await?;
        Ok(removed_took(rows))
    }
    async fn forget_on(&self, place_id: &str) -> wsp_machines::places::types::PlaceResult<wsp_wire::views::place_more::PlaceRemovedTook> {
        let ctx = self.ctx();
        let core = ctx.core.get();
        let rows = core.forget_on(place_id).await?;
        Ok(removed_took(rows))
    }
    async fn running_on(
        &self,
        place_id: &str,
        places: &[wsp_machines::places::types::RowId],
    ) -> wsp_machines::places::types::PlaceResult<u64> {
        let ctx = self.ctx();
        ctx.booted().await?;
        let places = places.iter().map(place_row).collect::<PlaceResult<Vec<PlaceView>>>()?;
        let mut workspaces = Vec::new();
        for entry in live_entries(&ctx) {
            let record = record_of(&entry);
            let provider = ctx.channels.get().provider_of(&record);
            workspaces.push(placed(&record, provider));
        }
        let rows: Vec<_> = ctx.sessions.borrow().values().cloned().collect();
        let mut views: Vec<SessionView> = Vec::new();
        for row in rows {
            let (turn_id, view) = {
                let session = row.borrow();
                (session.turn_id.clone(), session.view.clone())
            };
            if !CoreCtx::cap_held(&*ctx, &turn_id) {
                views.push(view);
            }
        }
        let threads = fold_threads(&views);
        Ok(running_on(place_id, &places, &workspaces, &threads) as u64)
    }
    async fn machines_on(&self, place_id: &str) -> Option<wsp_machines::places::types::PlaceResult<Vec<String>>> {
        let ctx = self.ctx();
        let core = ctx.core.get();
        Some(core.machines_on(place_id).await)
    }
    async fn forget_records_on(&self, place_id: &str) -> Option<wsp_machines::places::types::PlaceResult<()>> {
        Some(self.forget_records(place_id).await)
    }
    async fn sub_holds_on(&self, place_id: &str) -> Option<wsp_machines::places::types::PlaceResult<Vec<String>>> {
        Some(self.sub_holds(place_id).await)
    }
    async fn release_records_on(&self, place_id: &str) -> Option<wsp_machines::places::types::PlaceResult<()>> {
        Some(self.release_records(place_id).await)
    }
    fn has_images(&self) -> bool {
        true
    }
    async fn images_on(&self, place_id: &str) -> wsp_machines::places::types::PlaceResult<Vec<wsp_machines::places::types::HeldImage>> {
        let held = self.ctx().held_images.get();
        held.on(place_id).await
    }
    async fn drop_image_on(&self, place_id: &str, id: &str) -> wsp_machines::places::types::PlaceResult<()> {
        let held = self.ctx().held_images.get();
        HeldImages::drop(&held, place_id, id).await
    }
    async fn spent_today(
        &self,
        places: &[wsp_wire::views::place_more::PlaceView],
    ) -> Option<wsp_machines::places::types::PlaceResult<Vec<wsp_machines::places::types::SpentToday>>> {
        let ctx = self.ctx();
        let now = ctx.clock.now();
        let tracker = ctx.status_tracker.get();
        let spend = tracker.spend(places, Some(now)).await;
        let rows = spend.map_err(HostError::new).map(|rows| {
            rows.into_iter().map(|row| SpentToday { place: row.place, today_usd: row.today_usd.0 }).collect::<Vec<SpentToday>>()
        });
        Some(rows)
    }
    async fn sign_in(
        &self,
        place_id: &str,
        agent: &str,
        emit: wsp_machines::places::types::SignInEmit,
    ) -> Option<wsp_machines::places::types::PlaceResult<wsp_machines::places::types::SignInHandle>> {
        Some(follow_sign_in(self.ctx(), place_target(place_id), sign_in_ask(agent, Some(true)), emit).await)
    }
    async fn mint_here(
        &self,
        agent: &str,
        emit: wsp_machines::places::types::SignInEmit,
    ) -> Option<wsp_machines::places::types::PlaceResult<wsp_machines::places::types::SignInHandle>> {
        Some(follow_sign_in(self.ctx(), place_target(HERE_PLACE_ID), sign_in_ask(agent, None), emit).await)
    }
    async fn sign_in_line(
        &self,
        place_id: &str,
        agent: &str,
    ) -> Option<wsp_machines::places::types::PlaceResult<wsp_wire::types::agents_report::SignInLine>> {
        let ctx = self.ctx();
        let reads = ctx.agents_read();
        Some(reads.sign_in_line(&place_target(place_id), &sign_in_ask(agent, None), None).await)
    }
    fn stores_on(&self, place_id: &str, home: &str) -> Option<indexmap::IndexMap<String, String>> {
        let ctx = self.ctx();
        let agents = ctx.agents.get();
        agents.place_stores(place_id, home).ok()
    }
    async fn login_here(&self, agent: &str) -> Option<wsp_machines::places::types::PlaceResult<Option<Vec<u8>>>> {
        let ctx = self.ctx();
        let agents = ctx.agents.get();
        let homes: HomesHere = Rc::new(move || {
            let agents = agents.clone();
            Box::pin(async move { agents.homes_here().await })
        });
        Some(Ok(shared_login_here(homes, agent).await))
    }
    async fn add_folder(
        &self,
        place_id: &str,
        key: &str,
        folder: &wsp_wire::types::recipe_file::RecipeFileFoldersValue,
        mv: Option<&wsp_machines::places::types::FolderMove>,
        stage: Option<wsp_machines::places::types::FolderStage>,
    ) -> Option<wsp_machines::places::types::PlaceResult<wsp_machines::places::types::HeldRow>> {
        Some(self.add_folder_here(place_id, key, folder, mv, stage).await)
    }
    async fn folder_remote(
        &self,
        folder: &wsp_wire::types::recipe_file::RecipeFileFoldersValue,
    ) -> Option<wsp_machines::places::types::PlaceResult<Option<String>>> {
        let ctx = self.ctx();
        let path = expand_home(&folder.from);
        let kinds = ctx.kinds.get();
        let remote = kinds.remote_here(&path).await.map_err(|e| e.into_host());
        Some(remote.map(|(remote, _)| Some(remote).filter(|remote| !remote.is_empty())))
    }
    async fn remove_folder(&self, place_id: &str, project_id: &str) -> wsp_machines::places::types::PlaceResult<()> {
        let ctx = self.ctx();
        ctx.booted().await?;
        let core = ctx.core.get();
        core.remove_folder(place_id, project_id).await
    }
    async fn folder_look(
        &self,
        project_id: &str,
        was: &wsp_wire::types::recipe_file::RecipeFileFoldersValue,
        now: &wsp_wire::types::recipe_file::RecipeFileFoldersValue,
    ) -> wsp_machines::places::types::PlaceResult<()> {
        let ctx = self.ctx();
        let core = ctx.core.get();
        core.folder_look(project_id, was, now).await
    }
}

fn live_entries(ctx: &Ctx) -> Vec<Entry> {
    ctx.live.borrow().values().cloned().collect()
}

fn record_of(entry: &Entry) -> WorkspaceRecord {
    entry.borrow().record.borrow().clone()
}

fn items(rows: Vec<NamedCount>) -> Vec<PlaceHoldsForksItem> {
    rows.into_iter().map(|row| PlaceHoldsForksItem { name: row.name, threads: JsInt::from(row.threads as i64) }).collect()
}

fn removed_took(rows: StandingRows) -> PlaceRemovedTook {
    PlaceRemovedTook { forks: items(rows.forks), projects: items(rows.projects), away: None, images: None }
}

fn place_row(row: &RowId) -> PlaceResult<PlaceView> {
    from_json(serde_json::json!({ "id": row.id, "kind": row.kind, "name": row.id, "default": false }))
}

fn from_json<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> PlaceResult<T> {
    serde_json::from_value(value).map_err(|e| HostError::new(e.to_string()))
}

/// The fields `running_on` reads of a workspace; the rest of its view stays empty.
fn placed(record: &WorkspaceRecord, provider: Option<String>) -> WorkspaceView {
    WorkspaceView {
        folder: None,
        home: None,
        id: record.id.clone(),
        name: record.name.clone(),
        machine_id: record.machine_id.clone(),
        phase: record.phase,
        kind: Some(record.kind),
        golden: record.golden.clone(),
        created_at: record.created_at.clone(),
        project: ProjectRef { id: record.project.clone(), name: String::new(), path: String::new(), computer: String::new() },
        claude_session_id: None,
        queue: record.queue.clone(),
        screen: None,
        gone: None,
        theme: None,
        glyph: None,
        daemon_note: None,
        vaulted_at: None,
        vault_refused: None,
        daemon_refused_at: None,
        wake_refused: None,
        disk_gone: None,
        agents: None,
        parent_thread_id: None,
        root_thread_id: None,
        parent_workspace_id: None,
        place: record.place.clone(),
        worktree: None,
        provider,
        pause_mode: None,
        from: None,
        review: None,
        branch_of_record: None,
    }
}

fn sign_in_ask(agent: &str, tool_there: Option<bool>) -> reads::SignInAsk {
    reads::SignInAsk { agent: agent.to_string(), server: None, terminal: None, tool_there, login: None, scope: None, project: None }
}

fn place_target(place_id: &str) -> AgentsTarget {
    AgentsTarget::V0(AgentsTargetV0 { place_id: place_id.to_string(), project: None })
}

async fn follow_sign_in(ctx: Rc<Ctx>, target: AgentsTarget, ask: reads::SignInAsk, emit: SignInEmit) -> PlaceResult<SignInHandle> {
    let reads = ctx.agents_read();
    let following = reads.sign_in(target, ask, emit, None).await?;
    let id = following.sign_in_id.clone();
    let leave = RefCell::new(Some(following.leave));
    let stops = reads.clone();
    Ok(SignInHandle {
        leave: Box::new(move || {
            let leave = leave.borrow_mut().take();
            if let Some(leave) = leave {
                leave();
            }
        }),
        stop: Some(Box::new(move || {
            let _ = stops.sign_in_stop(&id);
        })),
    })
}

fn expand_home(path: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    match path.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("{home}{rest}"),
        _ => path.to_string(),
    }
}

fn installed_row(key: &str, project: &ProjectView, note: Option<String>, pick: &RecipeFileFoldersValue) -> HeldRow {
    HeldRow {
        row: PlaceProvisionRow {
            id: format!("folders/{key}"),
            label: project.name.clone(),
            outcome: PlaceProvisionRowOutcome::Installed,
            kind: None,
            step: None,
            note,
            fix: None,
            keys: None,
            ms: None,
            earlier: None,
            project: Some(PlaceProvisionRowProject { id: project.id.clone() }),
        },
        pick: Some(pick.clone()),
        created_at: Some(project.created_at.clone()),
    }
}
