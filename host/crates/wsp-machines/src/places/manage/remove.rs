// SPDX-License-Identifier: AGPL-3.0-only
//! manage.ts:90-195,494-720 at 65f6b89c7: holds, remove, update and their helpers. Spec:
//! `spec/places_manage_remove.md` (rules H1-H6, HL1, RM1-RM15, U1-U6).

use std::rc::Rc;
use std::sync::Arc;

use futures::FutureExt;
use indexmap::IndexSet;
use serde_json::{json, Map, Value};
use wsp_core::project_landing::{read_unsaved, RunHere};
use wsp_wire::types::daemon_contract::{RUNTIME_PROJECTS, RUNTIME_ROOT};
use wsp_wire::types::recipe_file::NO_RECIPE;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::views::place_more::{
    PlaceHolds, PlaceHoldsImagesItem, PlaceRemoved, PlaceRemovedEvent, PlaceUpdateReply, PlaceUpdateReplyDaemon, PlaceUpdateReplyDaemonRoad,
};
use wsp_wire::wire::daemon_version::DAEMON_VERSION;
use wsp_wire::wire::machine_link::{no_such_place_refusal, place_forget_answers_refusal, place_away_refusal, place_unsaved_refusal};
use wsp_wire::wire::place_link::plugins_kept_line;
use wsp_words::exit::{refusal_said, usage_refusal, HostError, Kind};
use wsp_words::place_word::{place_login_elsewhere, place_login_elsewhere_removed_line, place_login_other_refusal, place_login_unchecked_refusal, PlaceRef};
use wsp_words::thread_cgroup::thread_cgroups_end_script;
use wsp_words::views::place_setup_words::place_syncing_line;
use wsp_words::words::base::last_line;
use wsp_words::words::places::PLACE_LEAVE_LINE;
use wsp_words::wire::place_link::{place_forgotten_line, place_still_installed_line};
use wsp_words::workspace_state::{absent_computer, unpushed_unread_line};
use wsp_wire::views::place::place_forgets_only;

use super::add::exec_over;
use super::ManageDoor;
use crate::places::helpers::{PendingRecord, UPDATE_WAIT_MS};
use crate::places::provider_remove::{remove_provider, ProviderImages, ProviderRemove};
use crate::places::records::{PlaceRecordsArea, Reached};
use crate::places::types::{
    is_place_host_key_changed, is_place_login_refused, place_elsewhere_swept_over_link_line, place_login_refused, place_login_road_line,
    place_swept_over_link_line, place_swept_over_ssh_line, place_update_slow_line, PlaceEvent, PlaceExecOpts, PlaceLeaveRequest, PlaceLogin, PlaceRecord,
    PlaceResult, PlaceUpdateRequest, RemoveAsk, SudoVerb, UpdateRoad, NO_PLACE_UPDATER,
};

/// How long a leave over the link gives the threads' cgroups there to end (manage.ts:91).
pub const THREADS_END_MS: u64 = 60_000;

/// Where an older wsp kept a project's checkout on a computer you joined (manage.ts:94): `/wsp/projects`.
pub fn runtime_checkouts() -> String {
    format!("{RUNTIME_ROOT}/{RUNTIME_PROJECTS}")
}

/// H1 (:97).
pub fn runtime_checkouts_script() -> String {
    let root = shell_quote(&runtime_checkouts());
    format!("for d in {root}/*/checkout; do [ -d \"$d\" ] && printf '%s\\n' \"$d\"; done; true")
}

/// H2 (:101-104).
pub fn runtime_project_of(checkout: Option<&str>) -> Option<String> {
    let prefix = format!("{}/", runtime_checkouts());
    let checkout = checkout?;
    let name = checkout.strip_prefix(&prefix)?.strip_suffix("/checkout")?;
    (!name.is_empty() && !name.contains('/')).then(|| name.to_string())
}

/// What `reaches_itself` answers (H3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReachesItself {
    Stands,
    Refused { refused: String },
    Elsewhere { other: Option<String> },
}

impl ManageDoor {
    /// H3 (:151-157).
    pub async fn reaches_itself(&self, record: &PlaceRecord, login: &PlaceLogin, sudo_password: Option<&str>) -> PlaceResult<ReachesItself> {
        match self.records.reached_over(record, login, sudo_password).await? {
            Reached::Same => Ok(ReachesItself::Stands),
            Reached::Other { other } => Ok(ReachesItself::Elsewhere { other }),
            Reached::Unread { unread, refused: true } => Ok(ReachesItself::Refused { refused: unread }),
            Reached::Unread { unread, refused: false } => {
                let said = place_login_unchecked_refusal(&login.ssh, &record.name, &unread);
                Err(usage_refusal(&said.happened, &said.fix))
            }
        }
    }

    /// H4 (:162-168).
    pub async fn holds_of(&self, place_id: &str, record: &PlaceRecord) -> PlaceResult<PlaceHolds> {
        let answers = self.ctx.live.borrow().get(place_id).is_some();
        let mut holds = self.ctx.recording.holds_on(place_id, answers).await?;
        if !answers {
            holds.away = Some(Default::default());
            return Ok(holds);
        }
        if record.report.takes_runtime == Some(true) {
            let unrecorded = self.unrecorded_unsaved(place_id).await?;
            holds.unsaved.extend(unrecorded);
        }
        Ok(holds)
    }

    /// H6 (:172-182).
    pub async fn images_sized(&self, place_id: &str) -> PlaceResult<Vec<PlaceHoldsImagesItem>> {
        let images = if self.ctx.recording.has_images() { self.ctx.recording.images_on(place_id).await? } else { Vec::new() };
        let at = self.records.provider_backend(place_id);
        let listable = at.as_ref().is_some_and(|at| at.capabilities().snapshot_listing && at.offers().list_snapshots);
        let rows: Vec<(String, Option<f64>)> = match (&at, listable, images.is_empty()) {
            (Some(at), true, false) => {
                let ids: Vec<String> = images.iter().map(|i| i.id.clone()).collect();
                at.list_snapshots(Some(&ids)).await.map(|rows| rows.into_iter().map(|r| (r.id, Some(r.size_bytes.0))).collect()).unwrap_or_default()
            }
            _ => Vec::new(),
        };
        Ok(images
            .into_iter()
            .map(|image| {
                let size = rows.iter().find(|(id, _)| *id == image.id).and_then(|(_, size)| *size).filter(|size| *size != 0.0);
                PlaceHoldsImagesItem { id: image.id, size_bytes: size.map(JsNumber) }
            })
            .collect())
    }

    /// H5 (:184-195).
    pub async fn unrecorded_unsaved(&self, place_id: &str) -> PlaceResult<Vec<String>> {
        let recorded: IndexSet<String> = self.ctx.recording.projects_on(place_id).await?.into_iter().filter_map(|p| p.checkout).collect();
        let run: RunHere = {
            let (ctx, records, place) = (Rc::clone(&self.ctx), Rc::clone(&self.records), place_id.to_string());
            Rc::new(move |cmd: String, timeout: u64| {
                let (ctx, records, place) = (Rc::clone(&ctx), Rc::clone(&records), place.clone());
                async move { exec_over(&ctx, &records, &place, &cmd, PlaceExecOpts { timeout_ms: Some(timeout), stdin: None }).await }.boxed_local()
            })
        };
        let listed = match run(runtime_checkouts_script(), THREADS_END_MS).await {
            Ok(listed) => listed,
            Err(e) => serde_json::from_value(json!({ "exitCode": -1, "stdout": "", "stderr": e.message })).map_err(|e| HostError::new(e.to_string()))?,
        };
        let code = listed.exit_code.as_f64() as i64;
        if code != 0 {
            let why = last_line(&listed.stderr).unwrap_or_else(|| format!("the listing exited {code}"));
            return Ok(vec![unpushed_unread_line(&runtime_checkouts(), &why)]);
        }
        let mut unsaved = Vec::new();
        for checkout in listed.stdout.split('\n').filter(|line| !line.is_empty() && !recorded.contains(*line)) {
            if let Some(line) = read_unsaved(checkout, checkout, Rc::clone(&run)).await {
                unsaved.push(line);
            }
        }
        Ok(unsaved)
    }

    /// HL1 (:560-568).
    pub async fn holds(&self, place_id: &str) -> PlaceResult<PlaceHolds> {
        if let Some(held) = self.records.record_of(place_id).await? {
            return self.holds_of(place_id, &held).await;
        }
        if !self.records.provider_ids().iter().any(|id| id == place_id) {
            return Ok(PlaceHolds { forks: Vec::new(), projects: Vec::new(), unsaved: Vec::new(), away: None, images: None });
        }
        let images = self.images_sized(place_id).await?;
        let unsaved = match self.ctx.recording.sub_holds_on(place_id).await {
            Some(read) => read?,
            None => Vec::new(),
        };
        Ok(PlaceHolds { forks: Vec::new(), projects: Vec::new(), unsaved, away: None, images: (!images.is_empty()).then_some(images) })
    }

    /// RM1-RM15 (:570-720).
    pub async fn remove(&self, place_id: &str, ask: RemoveAsk) -> PlaceResult<PlaceRemoved> {
        let held = self.records.record_of(place_id).await?;
        let provider = match (&held, self.records.provider_ids().iter().any(|id| id == place_id)) {
            (None, true) => self.records.provider_backend(place_id),
            _ => None,
        };
        if let (Some(_), Some(true)) = (&provider, ask.forget) {
            if let Some(forget) = self.ctx.recording.forget_records_on(place_id).await {
                forget?;
            }
            let still = match self.ctx.wiring.forget_key.clone() {
                Some(forgetter) => forgetter.forget_key(place_id).await?,
                None => None,
            };
            self.ctx.emit(&PlaceEvent::Removed(PlaceRemovedEvent { place_id: place_id.to_string() }));
            return Ok(PlaceRemoved { removed: true, took: None, swept: Vec::new(), note: still, deleted: None, snapshots: None });
        }
        if let Some(backend) = provider {
            return self.remove_cloud(place_id, backend).await;
        }
        let Some(held) = held else {
            return Ok(PlaceRemoved { removed: false, took: None, swept: Vec::new(), note: None, deleted: None, snapshots: None });
        };
        self.remove_computer(place_id, held, ask).await
    }

    /// RM3 (:582-595): a provider's remove, as places-a's provider remove answers it.
    async fn remove_cloud(&self, place_id: &str, backend: Arc<dyn wsp_engine::machine::MachineBackend>) -> PlaceResult<PlaceRemoved> {
        let recording = Rc::clone(&self.ctx.recording);
        let images = if recording.has_images() {
            let (held_rec, drop_rec, place) = (Rc::clone(&recording), Rc::clone(&recording), place_id.to_string());
            let drop_place = place.clone();
            Some(ProviderImages {
                held: Rc::new(move || {
                    let (recording, place) = (Rc::clone(&held_rec), place.clone());
                    async move { recording.images_on(&place).await }.boxed_local()
                }),
                drop: Rc::new(move |id: String| {
                    let (recording, place) = (Rc::clone(&drop_rec), drop_place.clone());
                    async move { recording.drop_image_on(&place, &id).await }.boxed_local()
                }),
            })
        } else {
            None
        };
        let owner = match &self.ctx.opts.owner {
            Some(owner) => owner().await,
            None => String::new(),
        };
        let known = match self.ctx.recording.machines_on(place_id).await {
            Some(read) => read?,
            None => Vec::new(),
        };
        let answer = remove_provider(ProviderRemove {
            place_id: place_id.to_string(),
            backend,
            owner,
            known,
            forget_key: self.ctx.wiring.forget_key.clone(),
            confirm: self.ctx.opts.kill_confirm,
            images,
        })
        .await?;
        if answer.removed {
            if let Some(release) = self.ctx.recording.release_records_on(place_id).await {
                release?;
            }
            self.ctx.emit(&PlaceEvent::Removed(PlaceRemovedEvent { place_id: place_id.to_string() }));
        }
        Ok(answer)
    }

    /// RM5-RM15 (:600-720): a computer's remove, read first and then taken.
    async fn remove_computer(&self, place_id: &str, held: PlaceRecord, ask: RemoveAsk) -> PlaceResult<PlaceRemoved> {
        let read = self.ctx.live.borrow().get(place_id).is_some();
        let holds = self.holds_of(place_id, &held).await?;
        let forget = ask.forget == Some(true);
        if holds.away != Some(Default::default()) && forget {
            let refused = place_forget_answers_refusal(&held.name);
            return Err(usage_refusal(&refused.said, &refused.fix));
        }
        if place_forgets_only(&holds) && !forget {
            let refused = place_away_refusal(&held.name, &absent_computer(&held.name, None).said);
            return Err(usage_refusal(&refused.said, &refused.fix));
        }
        if !holds.unsaved.is_empty() && ask.force != Some(true) {
            let refused = place_unsaved_refusal(&held.name, &holds.unsaved);
            return Err(usage_refusal(&refused.said, &refused.fix));
        }
        let reach = self.ctx.live.borrow().get(place_id).map(|live| Rc::clone(&live.reach));
        let leaver = self.ctx.wiring.leave.clone();
        let login = PlaceRecordsArea::login_of(&held);
        let answers = match (&leaver, &login) {
            (Some(_), Some(login)) => self.records.login_answers(login).await,
            _ => false,
        };
        let mut key_changed = false;
        let sudo_password = match (answers, &login) {
            (true, Some(login)) => match self.records.root_over(login, ask.sudo_password.as_deref(), SudoVerb::Remove, &held.name).await {
                Ok(password) => password,
                Err(e) if is_place_host_key_changed(&e) => {
                    key_changed = true;
                    None
                }
                Err(e) => return Err(e),
            },
            _ => None,
        };
        let reached: Option<ReachesItself> = if key_changed {
            Some(ReachesItself::Elsewhere { other: None })
        } else {
            match (answers, &login) {
                (true, Some(login)) => match self.reaches_itself(&held, login, sudo_password.as_deref()).await {
                    Ok(reached) => Some(reached),
                    Err(e) if !forget => return Err(e),
                    Err(e) => Some(ReachesItself::Refused { refused: refusal_said(&e.message, e.fix.as_deref()) }),
                },
                _ => None,
            }
        };
        let stands = matches!(reached, Some(ReachesItself::Stands));
        let elsewhere = match &reached {
            Some(ReachesItself::Elsewhere { other }) => Some(other.clone()),
            _ => None,
        };
        let away = match (&elsewhere, &login) {
            (Some(other), Some(login)) => Some(place_login_elsewhere(&login.ssh, &held.name, other.as_deref())),
            _ => None,
        };
        let mut projects: Vec<String> = Vec::new();
        for project in self.ctx.recording.projects_on(place_id).await? {
            if let Some(name) = runtime_project_of(project.checkout.as_deref()) {
                projects.push(name);
            }
        }
        let force = ask.force == Some(true) || (read && held.report.takes_runtime.is_some());
        let went = if holds.away == Some(Default::default()) {
            self.ctx.recording.forget_on(place_id).await?
        } else {
            self.ctx.recording.drop_on(place_id).await?
        };
        let plugins = self
            .views
            .plugins_off(place_id, &held, reach.is_some(), if stands { login.as_ref() } else { None }, sudo_password.as_deref())
            .await;
        let stored = if reach.is_some() { self.views.store_files_off(place_id, &held).await } else { Vec::new() };
        let mut swept: Vec<String> = Vec::new();
        let mut note: Option<String> = None;
        let mut login_road: Option<(String, Option<String>)> = None;
        if let (Some(leaver), Some(login)) = (&leaver, &login) {
            if !stands {
                login_road = Some((login.ssh.clone(), None));
            } else {
                let request = PlaceLeaveRequest {
                    place_id: place_id.to_string(),
                    name: held.name.clone(),
                    report: held.report.clone(),
                    ssh: login.clone(),
                    sudo_password: sudo_password.clone(),
                    force: force.then_some(true),
                    projects: Some(projects.clone()),
                };
                match leaver.leave(request).await {
                    Ok(lines) => {
                        swept = lines;
                        note = Some(place_swept_over_ssh_line(&held.name));
                    }
                    Err(e) => {
                        let said = if is_place_login_refused(&e) { None } else { Some(e.message.clone()) };
                        login_road = Some((login.ssh.clone(), said));
                    }
                }
            }
        }
        if note.is_none() {
            if let (Some(away), Some(login)) = (&away, &login) {
                note = Some(match reach {
                    None => place_login_elsewhere_removed_line(&login.ssh, &held.name, elsewhere.clone().flatten().as_deref()),
                    Some(_) => place_elsewhere_swept_over_link_line(&held.name, away),
                });
            }
            match reach {
                None => {
                    let left = if forget { place_forgotten_line(&held.name, !went.forks.is_empty()) } else { place_still_installed_line(&held.name) };
                    if note.is_none() {
                        note = Some(match &login_road {
                            Some((at, Some(said))) => format!("{}; {left}", place_login_road_line(&held.name, at, Some(said))),
                            _ => left,
                        });
                    }
                }
                Some(ref link) => {
                    let sweep = self.link_sweep(place_id, &held, link, force, &projects).await;
                    match sweep {
                        Ok((lines, servers_kept)) => {
                            swept.extend(lines);
                            if let (Some((at, said)), None) = (&login_road, &away) {
                                note = Some(place_swept_over_link_line(&held.name, at, said.as_deref()));
                            }
                            if !servers_kept.is_empty() {
                                let mut parts: Vec<String> = note.take().into_iter().collect();
                                parts.extend(servers_kept);
                                note = Some(parts.join("; "));
                            }
                        }
                        Err(e) => {
                            let failed = format!("{} was connected but did not finish the sweep: {}; run {PLACE_LEAVE_LINE} on that computer", held.name, e.message);
                            let before = match (&away, &login_road) {
                                (Some(away), _) => Some(format!("{away}, so nothing was changed on the machine it reaches")),
                                (None, Some((at, said))) => Some(place_login_road_line(&held.name, at, said.as_deref())),
                                (None, None) => None,
                            };
                            note = Some(match before {
                                None => failed,
                                Some(before) => format!("{before}, and {failed}"),
                            });
                        }
                    }
                }
            }
        }
        let mut all = plugins.off.clone();
        all.extend(stored);
        all.extend(swept);
        if !plugins.kept.is_empty() {
            let kept = plugins_kept_line(&held.name, &plugins.kept);
            note = Some(match note {
                None => kept,
                Some(note) => format!("{note}; {kept}"),
            });
        }
        if reach.is_some() {
            self.views.cut(place_id, "removed from this host");
        }
        if let (Some(login), Some(_)) = (&login, &held.road.as_ref().and_then(|r| r.back.clone())) {
            let all_records = self.records.records().await?;
            let shared = all_records.iter().any(|r| r.id != place_id && PlaceRecordsArea::login_of(r).is_some_and(|other| other.ssh == login.ssh));
            if !shared {
                if let Some(holder) = self.ctx.wiring.back.clone() {
                    holder.release(login);
                }
            }
        }
        let pending: Vec<PendingRecord> = self.views.pending_records().await?;
        for p in pending.iter().filter(|p| p.view.place_id.as_deref() == Some(place_id)) {
            self.views.drop_pending(&p.view.id).await?;
        }
        self.views.forget(place_id).await?;
        Ok(PlaceRemoved { removed: true, took: Some(went), swept: all, note, deleted: None, snapshots: None })
    }

    /// The leave the computer's own sweep over its link runs (RM13 :617-639): the servers wsp merged, the threads'
    /// cgroups and the daemon's `place.leave`. Answers the lines it took and the servers' unread lines.
    async fn link_sweep(&self, place_id: &str, held: &PlaceRecord, reach: &Rc<dyn wsp_core::types::pending::DaemonReach>, force: bool, projects: &[String]) -> PlaceResult<(Vec<String>, Vec<String>)> {
        let servers = self.views.unmerged_over(place_id, held).await;
        let ended = exec_over(&self.ctx, &self.records, place_id, &thread_cgroups_end_script(None), PlaceExecOpts { timeout_ms: Some(THREADS_END_MS), stdin: None }).await.ok();
        let mut frame = Map::new();
        if force {
            frame.insert("force".to_string(), Value::Bool(true));
        }
        frame.insert("projects".to_string(), json!(projects));
        let answer = reach.request("place.leave", frame).await.map_err(|e| HostError::new(e.message))?;
        let mut lines = servers.took.clone();
        if let Some(ended) = ended {
            lines.extend(ended.stdout.split('\n').filter(|line| !line.is_empty()).map(str::to_string));
        }
        if let Some(Value::Array(swept)) = answer.get("swept") {
            lines.extend(swept.iter().map(|v| match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            }));
        }
        Ok((lines, servers.kept))
    }

    /// U1-U6 (:494-558).
    pub async fn update(&self, place_id: &str, ask: RemoveAsk) -> PlaceResult<PlaceUpdateReply> {
        let Some(held) = self.records.record_of(place_id).await? else {
            let names: Vec<String> = self.records.records().await?.into_iter().map(|r| r.name).collect();
            return Err(HostError::new(no_such_place_refusal(place_id, &names)));
        };
        let busy = if self.setup.is_setting(&held.id) { self.setup.setting_now(&held) } else { None };
        if let Some(busy) = busy {
            return Err(HostError::with_kind(busy, Kind::Conflict));
        }
        if self.setup.is_syncing(&held.id) {
            return Err(HostError::with_kind(place_syncing_line(&held.name), Kind::Conflict));
        }
        let from = held.report.daemon_version.as_f64() as u32;
        let moving = i64::from(from) < DAEMON_VERSION;
        let mut daemon: Option<PlaceUpdateReplyDaemon> = None;
        match self.ctx.wiring.update.clone() {
            None => {
                if moving {
                    return Err(HostError::new(NO_PLACE_UPDATER));
                }
            }
            Some(updater) => {
                let link = self.ctx.live.borrow().get(place_id).map(|live| Rc::clone(&live.reach));
                let ssh = PlaceRecordsArea::login_of(&held);
                let sudo_password = match (&link, &ssh) {
                    (None, Some(login)) => self.records.root_over(login, ask.sudo_password.as_deref(), SudoVerb::Update, &held.name).await?,
                    _ => None,
                };
                if let (None, Some(login)) = (&link, &ssh) {
                    match self.reaches_itself(&held, login, sudo_password.as_deref()).await? {
                        ReachesItself::Stands => {}
                        ReachesItself::Refused { refused } => return Err(place_login_refused(&refused)),
                        ReachesItself::Elsewhere { other } => {
                            let place = PlaceRef { id: held.id.clone(), name: held.name.clone(), kind: None };
                            let said = place_login_other_refusal(&login.ssh, &place, other.as_deref());
                            return Err(usage_refusal(&said.happened, &said.fix));
                        }
                    }
                }
                let landed = updater
                    .update(PlaceUpdateRequest {
                        place_id: place_id.to_string(),
                        name: held.name.clone(),
                        report: held.report.clone(),
                        daemon: moving,
                        link,
                        ssh: ssh.clone(),
                        sudo_password,
                    })
                    .await?;
                if let Some(landed) = landed {
                    let wait = self.ctx.opts.update_wait_ms.unwrap_or(UPDATE_WAIT_MS);
                    let to = self.records.until_daemon_version(place_id, from, wait).await?;
                    if to != from {
                        self.views.facts_on(place_id, &held).await;
                    }
                    let note = (i64::from(to) < DAEMON_VERSION).then(|| place_update_slow_line(&held.name, (wait + 500) / 1000));
                    daemon = Some(PlaceUpdateReplyDaemon {
                        from: JsInt(JsNumber(f64::from(from))),
                        to: JsInt(JsNumber(f64::from(to))),
                        road: match landed.road {
                            UpdateRoad::Link => PlaceUpdateReplyDaemonRoad::Link,
                            UpdateRoad::Ssh => PlaceUpdateReplyDaemonRoad::Ssh,
                        },
                        at: landed.at.clone(),
                        kept: landed.kept.clone(),
                        note,
                    });
                }
            }
        }
        if let Some(recipe) = held.recipe.as_deref().filter(|r| *r != NO_RECIPE) {
            let _ = recipe;
            self.setup.sync_soon(place_id, 0);
        }
        Ok(PlaceUpdateReply { name: held.name.clone(), daemon })
    }
}
