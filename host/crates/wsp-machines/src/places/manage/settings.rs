// SPDX-License-Identifier: AGPL-3.0-only
//! manage.ts:122-145,425-492,722-871 at 65f6b89c7: list, rows, set, find, pending, choose, setUp, the recipe roads,
//! skip, estimate, setupLog, picksOf, on and close. Spec: `spec/places_manage_settings.md` (rules L1-L3, S1-S10,
//! F1-F6, P1-P3, SK1-SK4, ES1, LG1, C1-C2).

use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};
use serde_json::{to_value, Value};
use wsp_core::ids::{Ids, OsIds};
use wsp_engine::golden_seam::owned_floor_bytes;
use wsp_engine::machine::{ExecOptions, Machine};
use wsp_engine::place_machine::PlaceMachine;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::recipe_file::{RecipeFile, NO_RECIPE};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::place::{
    PlaceProvisionRow, PlaceProvisionRowOutcome, PlaceSetAlso, PlaceSetupEvent, PlaceSetupState, PlaceSetupStep, PlaceSync, PlaceSyncEvent, PlaceSyncState,
};
use wsp_wire::views::place::{PendingComputer, PlaceSetup};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::place_more::{PlaceChangedEvent, PlaceSettingWord, PlaceSettings, PlaceSettingsAsk, PlaceSettingsSpawn, PlaceView};
use wsp_wire::views::place_setup_words::PlaceEstimate;
use wsp_wire::wire::machine_link::no_such_place_refusal;
use wsp_wire::wire::place_link::two_places_refusal;
use wsp_words::exit::{usage_refusal, HostError, Kind};
use wsp_words::place_state::{place_set_refusal, place_setting_dropped};
use wsp_words::place_word::{place_rename_refusal, place_ssh_other_refusal, place_ssh_refusal, place_ssh_unchecked_refusal, PlaceRef};
use wsp_words::project_path::place_provision_paths;
use wsp_words::recipe_file::recipe_counts;
use wsp_words::views::place_setup_words::{
    no_pending_refusal, nothing_to_skip_line, pending_not_joined_fix, pending_not_joined_line, place_no_picks_line, CHOOSE_FIX, SETUP_LOG_TAIL_BYTES,
    SKIPPED_FOR_NOW,
};

use super::{ManageDoor, SetupPicks, SetupStarted};
use crate::places::helpers::{pending_view, setup_outcome, LOG_READ_MS, NO_ESTIMATE_LINE};
use crate::places::records::{PlaceRecordsArea, Reached};
use crate::places::setup::{host_of, iso_at};
use crate::places::types::{
    applied_view, HeldApplied, HeldRow, PlaceEvent, PlaceLogin, PlaceRecord, PlaceRecordRoad, PlaceResult, PlaceSetUp, Resolved, SetUpAsk,
};

/// The usage fix a name that matches no computer carries (manage.ts:458,500 and others).
const NAMED_FIX: &str = "Run wsp computers to read the ones this host holds.";

/// The setting words a set names that are not the switch (`spawn`): one given key each (manage.ts:446).
fn gives_any(set: &PlaceSettingsAsk) -> bool {
    set.threads.is_some()
        || set.machines.is_some()
        || set.spend_per_day_usd.is_some()
        || set.nap_ms.is_some()
        || set.turn_limit_ms.is_some()
        || set.size.is_some()
        || set.delete_after_ms.is_some()
}

/// Whether a place holds no setting at all (`Object.keys(next).length === 0`, manage.ts:462).
fn holds_none(s: &PlaceSettings) -> bool {
    s.threads.is_none()
        && s.machines.is_none()
        && s.spend_per_day_usd.is_none()
        && s.nap_ms.is_none()
        && s.turn_limit_ms.is_none()
        && s.spawn.is_none()
        && s.size.is_none()
        && s.delete_after_ms.is_none()
}

/// The place reference a row names, for the rename and login refusals (words' `PlaceRef`).
fn place_ref(view: &PlaceView) -> PlaceRef {
    let kind = match view.kind {
        PlaceKind::Computer => "computer",
        PlaceKind::Provider => "provider",
    };
    PlaceRef { id: view.id.clone(), name: view.name.clone(), kind: Some(kind.to_string()) }
}

/// The step a row or a wait is named by, as its wire word (`step` in ` [<step>] `, manage.ts:833).
fn step_word(step: PlaceSetupStep) -> String {
    match to_value(step) {
        Ok(Value::String(word)) => word,
        _ => String::new(),
    }
}

impl ManageDoor {
    /// F1 (:124-135).
    pub async fn follow_to(&self, place_id: &str, recipe: &str) -> PlaceResult<Option<PlaceRecord>> {
        let before: Rc<std::cell::RefCell<Option<PlaceRecord>>> = Rc::new(std::cell::RefCell::new(None));
        let (seen, given) = (Rc::clone(&before), recipe.to_string());
        let moved = self
            .records
            .change(place_id, move |now: PlaceRecord| {
                *seen.borrow_mut() = Some(now.clone());
                if given == NO_RECIPE {
                    Some(PlaceRecord { recipe: Some(given), sync: None, ..now })
                } else {
                    Some(PlaceRecord { recipe: Some(given), ..now })
                }
            })
            .await?;
        let before = before.borrow().clone();
        if recipe == NO_RECIPE && before.as_ref().is_some_and(|b| b.sync.is_some()) {
            self.setup.sync_frame(PlaceSyncEvent { place_id: place_id.to_string(), sync: None, line: None, applied: None });
        }
        if recipe != NO_RECIPE {
            self.setup.sync_soon(place_id, 0);
        }
        let mut slugs: IndexSet<String> = IndexSet::new();
        if let Some(old) = before.and_then(|b| b.recipe) {
            slugs.insert(old);
        }
        slugs.insert(recipe.to_string());
        for slug in slugs.iter().filter(|slug| slug.as_str() != NO_RECIPE) {
            self.setup.recipes_moved(slug);
        }
        Ok(moved)
    }

    /// S5 (:140-145).
    pub async fn same_computer_over(&self, record: &PlaceRecord, ssh: &str) -> PlaceResult<()> {
        let login = PlaceLogin { ssh: ssh.to_string(), key_path: record.road.as_ref().and_then(|r| r.key_path.clone()), host_key: None };
        match self.records.reached_over(record, &login, None).await? {
            Reached::Same => Ok(()),
            Reached::Unread { unread, .. } => {
                let said = place_ssh_unchecked_refusal(ssh, &record.name, &unread);
                Err(usage_refusal(&said.happened, &said.fix))
            }
            Reached::Other { .. } => {
                let said = place_ssh_other_refusal(ssh, &record.name);
                Err(usage_refusal(&said.happened, &said.fix))
            }
        }
    }

    /// L1 (:425-435).
    pub async fn list(self: &Rc<Self>, now: u64) -> PlaceResult<Vec<PlaceView>> {
        let _ = now;
        let held = self.records.records().await?;
        let mut room: IndexMap<String, _> = IndexMap::new();
        for record in &held {
            room.insert(record.id.clone(), self.views.forks_of(record).await);
        }
        let rows = self.views.rows_of(&held).await?;
        let providers: Vec<PlaceView> = rows.iter().filter(|r| r.kind == PlaceKind::Provider).cloned().collect();
        let spent = match self.ctx.recording.spent_today(&providers).await {
            Some(Ok(spent)) => spent,
            _ => Vec::new(),
        };
        let mut out = Vec::with_capacity(rows.len());
        for mut row in rows {
            if let Some(Some(forks)) = room.get(&row.id).cloned() {
                row.forks = Some(forks);
            }
            if let Some(found) = spent.iter().find(|s| s.place == row.id) {
                row.spent_today_usd = Some(JsNumber(found.today_usd));
            }
            out.push(self.records.with_limits(row));
        }
        Ok(out)
    }

    /// L2 (:437).
    pub async fn rows(&self) -> PlaceResult<Vec<PlaceView>> {
        let held = self.records.records().await?;
        self.views.rows_of(&held).await
    }

    /// S1-S10 (:439-492).
    pub async fn set(self: &Rc<Self>, place_id: &str, set: PlaceSettingsAsk, reset: Vec<PlaceSettingWord>, also: PlaceSetAlso) -> PlaceResult<PlaceView> {
        let marked = self.records.mark_held().await?.unwrap_or_else(|| HERE_PLACE_ID.to_string());
        let record = if place_id == HERE_PLACE_ID { None } else { self.records.record_of(place_id).await? };
        let row = if place_id == HERE_PLACE_ID {
            self.views.here_row(&marked)
        } else if let Some(record) = &record {
            self.views.joined_row(record, &marked)
        } else if self.records.provider_ids().iter().any(|id| id == place_id) {
            self.views.provider_row(place_id, &marked)
        } else {
            let mut held = vec![(self.ctx.wiring.here)().name];
            held.extend(self.records.records().await?.into_iter().map(|r| r.name));
            held.extend(self.records.provider_ids().iter().map(|id| wsp_words::place_word::provider_key_name(id)));
            return Err(HostError::with_kind(no_such_place_refusal(place_id, &held), Kind::Usage));
        };
        let renamed = also.name.as_deref().map(|name| name.trim().to_string());
        let ssh = also.ssh.as_deref().map(|ssh| ssh.trim().to_string());
        let spawn = set.spawn.clone();
        let settles = (renamed.is_none() && ssh.is_none() && also.recipe.is_none()) || spawn.is_some() || gives_any(&set) || !reset.is_empty();
        if settles {
            if let Some(refused) = place_set_refusal(&row, &set, &reset) {
                return Err(HostError::with_kind(refused, Kind::Usage));
            }
        }
        if let Some(renamed) = &renamed {
            let mut others: Vec<PlaceRef> = vec![place_ref(&self.views.here_row(&marked))];
            for r in self.records.records().await? {
                others.push(place_ref(&self.views.joined_row(&r, &marked)));
            }
            for id in self.records.provider_ids() {
                others.push(place_ref(&self.views.provider_row(&id, &marked)));
            }
            if let Some(misnamed) = place_rename_refusal(&place_ref(&row), renamed, &others) {
                return Err(usage_refusal(&misnamed.happened, &misnamed.fix));
            }
        }
        if let Some(ssh) = &ssh {
            if let Some(wrong) = place_ssh_refusal(&place_ref(&row), ssh) {
                return Err(usage_refusal(&wrong.happened, &wrong.fix));
            }
        }
        if also.recipe.is_some() && record.is_none() {
            let names: Vec<String> = self.records.records().await?.into_iter().map(|r| r.name).collect();
            return Err(HostError::with_kind(no_such_place_refusal(place_id, &names), Kind::Usage));
        }
        if let (Some(ssh), Some(record)) = (&ssh, &record) {
            if PlaceRecordsArea::login_of(record).map(|l| l.ssh) != Some(ssh.clone()) {
                self.same_computer_over(record, ssh).await?;
            }
        }
        if settles {
            let (records, store, place) = (Rc::clone(&self.records), Rc::clone(&self.ctx.store), place_id.to_string());
            let switch = spawn.clone();
            let given = set.clone();
            let reset_words = reset.clone();
            records
                .in_turn(Box::pin(async move {
                    let held = self.records.settings_of(&place).await?;
                    let mut next = held.clone();
                    if let Some(v) = given.threads {
                        next.threads = Some(v);
                    }
                    if let Some(v) = given.machines {
                        next.machines = Some(v);
                    }
                    if let Some(v) = given.spend_per_day_usd {
                        next.spend_per_day_usd = Some(v);
                    }
                    if let Some(v) = given.nap_ms {
                        next.nap_ms = Some(v);
                    }
                    if let Some(v) = given.turn_limit_ms {
                        next.turn_limit_ms = Some(v);
                    }
                    if let Some(v) = given.size {
                        next.size = Some(v);
                    }
                    if let Some(v) = given.delete_after_ms {
                        next.delete_after_ms = Some(v);
                    }
                    if let Some(sp) = switch {
                        let old = held.spawn.clone();
                        next.spawn = Some(PlaceSettingsSpawn {
                            max_machines: sp.max_machines.or(old.as_ref().and_then(|o| o.max_machines)),
                            max_depth: sp.max_depth.or(old.as_ref().and_then(|o| o.max_depth)),
                        });
                    }
                    for word in &reset_words {
                        next = place_setting_dropped(&next, word);
                    }
                    if holds_none(&next) {
                        store.delete(crate::places::types::CAPS, &place).await?;
                    } else {
                        store.put(crate::places::types::CAPS, &place, to_value(&next).map_err(|e| HostError::new(e.to_string()))?).await?;
                    }
                    self.records.settings_held.borrow_mut().insert(place.clone(), next);
                    Ok::<(), HostError>(())
                }))
                .await?;
        }
        if set.nap_ms.is_some() || reset.contains(&PlaceSettingWord::Nap) {
            if let Some(nap_changed) = &self.ctx.opts.nap_changed {
                nap_changed(place_id);
            }
        }
        let was = record.as_ref().and_then(PlaceRecordsArea::login_of);
        let mut moved = if renamed.is_none() && ssh.is_none() {
            None
        } else {
            let (new_name, new_ssh) = (renamed.clone(), ssh.clone());
            self.records
                .change(place_id, move |now: PlaceRecord| {
                    let mut next = now.clone();
                    if let Some(name) = new_name {
                        next.name = name;
                    }
                    if let Some(ssh) = new_ssh {
                        next.road = Some(PlaceRecordRoad { ssh: Some(ssh), ..now.road.clone().unwrap_or_default() });
                    }
                    Some(next)
                })
                .await?
        };
        if let (Some(moved_now), Some(name)) = (&moved, &renamed) {
            let _ = name;
            for pending in self.views.pending_records().await? {
                if pending.view.place_id.as_deref() == Some(place_id) {
                    let mut next = pending.clone();
                    next.view.name = Some(moved_now.name.clone());
                    self.views.put_pending(&next).await?;
                }
            }
        }
        if let (Some(moved_now), Some(was), Some(new_ssh)) = (&moved, &was, &ssh) {
            if was.ssh != *new_ssh && moved_now.road.as_ref().and_then(|r| r.back.clone()).is_some() {
                let others = self.records.records().await?;
                let shared = others.iter().any(|r| r.id != place_id && PlaceRecordsArea::login_of(r).is_some_and(|l| l.ssh == was.ssh));
                if !shared {
                    if let Some(holder) = self.ctx.wiring.back.clone() {
                        holder.release(was);
                    }
                }
                self.records.hold_back(moved_now);
            }
        }
        if let Some(recipe) = &also.recipe {
            if let Some(followed) = self.follow_to(place_id, recipe).await? {
                moved = Some(followed);
            }
        }
        let view = match &moved {
            Some(moved) => self.views.joined_row(moved, &marked),
            None => row,
        };
        let ids = self.records.row_ids().await?;
        let place = self.records.with_limits(self.records.with_cap(view, &ids).await?);
        self.ctx.emit(&PlaceEvent::Changed(PlaceChangedEvent { place: place.clone() }));
        Ok(place)
    }

    /// L3 (:722).
    pub async fn find(&self, reference: &str) -> PlaceResult<Vec<PlaceRecord>> {
        Ok(self.records.records().await?.into_iter().filter(|r| r.id == reference || r.name == reference).collect())
    }

    /// P1 (:724).
    pub async fn pending(&self) -> PlaceResult<Vec<PendingComputer>> {
        Ok(self.views.pending_records().await?.iter().map(pending_view).collect())
    }

    /// P2 (:726-733).
    pub async fn choose(&self, reference: &str, choices: RecipeFile, recipe: Option<String>) -> PlaceResult<PendingComputer> {
        let found = self
            .views
            .pending_records()
            .await?
            .into_iter()
            .find(|p| p.view.id == reference || p.view.address == reference || p.view.place_id.as_deref() == Some(reference));
        let Some(mut next) = found else {
            return Err(usage_refusal(&no_pending_refusal(reference), "Run wsp computers to read the adds still pending."));
        };
        next.view.choices = choices;
        next.view.recipe = recipe;
        self.views.put_pending(&next).await?;
        Ok(pending_view(&next))
    }

    /// P3 (:735-755).
    pub async fn set_up(&self, reference: &str, o: SetUpAsk) -> PlaceResult<PlaceSetUp> {
        let add_id = o.add_id.clone().unwrap_or_else(|| format!("a_{}", hex::encode(OsIds.bytes(6))));
        let held = self.records.records().await?;
        let pend = self.views.pending_records().await?.into_iter().find(|p| {
            p.view.id == reference
                || p.view.address == reference
                || p.view.name.as_deref() == Some(reference)
                || p.view.place_id.as_deref().is_some_and(|pid| pid == reference || self.ctx.kept.borrow().get(pid).is_some_and(|r| r.name == reference))
        });
        if let Some(pend) = &pend {
            if pend.view.place_id.is_none() {
                let name = pend.view.name.clone().unwrap_or_else(|| pend.view.address.clone());
                return Err(usage_refusal(&pending_not_joined_line(&name), &pending_not_joined_fix(&pend.view.address)));
            }
        }
        let found: Vec<PlaceRecord> = match pend.as_ref().and_then(|p| p.view.place_id.clone()) {
            Some(pid) => held.iter().filter(|r| r.id == pid).cloned().collect(),
            None => held.iter().filter(|r| r.id == reference || r.name == reference).cloned().collect(),
        };
        if found.len() > 1 {
            let ids: Vec<String> = found.iter().map(|r| r.id.clone()).collect();
            return Err(usage_refusal(&two_places_refusal(reference, &ids), "Name it by its id."));
        }
        let Some(record) = found.first().cloned() else {
            let names: Vec<String> = held.iter().map(|r| r.name.clone()).collect();
            return Err(usage_refusal(&no_such_place_refusal(reference, &names), NAMED_FIX));
        };
        let default = self.records.default_id().await?;
        if self.setup.is_setting(&record.id) && o.choices.is_none() && o.recipe.is_none() {
            if let Some(setup) = record.setup.clone() {
                return Ok(PlaceSetUp { add_id: setup.add_id.clone(), place: self.views.view_of(&record, default.as_deref()), setup: Some(setup), said: None });
            }
        }
        let choices = match &o.choices {
            Some(choices) => Some(choices.clone()),
            None => pend.as_ref().filter(|p| recipe_counts(&p.view.choices).values().any(|n| *n > 0)).map(|p| p.view.choices.clone()),
        };
        if choices.is_none() && record.picks.is_none() {
            return Err(usage_refusal(&place_no_picks_line(&record.name), CHOOSE_FIX));
        }
        if let Some(pend) = &pend {
            self.views.drop_pending(&pend.view.id).await?;
        }
        let recipe = match &o.recipe {
            Some(recipe) => Some(recipe.clone()),
            None if o.choices.is_none() => pend.as_ref().and_then(|p| p.view.recipe.clone()),
            None => None,
        };
        let given = choices.map(|picks| SetupPicks { picks, recipe });
        let started: SetupStarted = self.setup.start_setup(&record.id, &add_id, given, o.row.clone()).await?;
        let now = self.records.record_of(&record.id).await?.unwrap_or_else(|| record.clone());
        let default = self.records.default_id().await?;
        Ok(PlaceSetUp { add_id, place: self.views.view_of(&now, default.as_deref()), setup: started.setup, said: started.said })
    }

    /// F2 (:757-761).
    pub async fn follow(&self, place_id: &str, recipe: &str) -> PlaceResult<PlaceView> {
        let Some(held) = self.records.record_of(place_id).await? else {
            let names: Vec<String> = self.records.records().await?.into_iter().map(|r| r.name).collect();
            return Err(HostError::with_kind(no_such_place_refusal(place_id, &names), Kind::Usage));
        };
        let written = self.follow_to(place_id, recipe).await?;
        let default = self.records.default_id().await?;
        Ok(self.views.view_of(&written.unwrap_or(held), default.as_deref()))
    }

    /// F3 (:763-781).
    pub async fn recipe_changed(&self, slug: &str, after_ms: u64) -> PlaceResult<Vec<String>> {
        self.setup.recipes_moved(slug);
        let mut names: Vec<String> = Vec::new();
        let mut resolved: Option<Resolved> = None;
        for record in self.records.records().await? {
            if record.recipe.as_deref() != Some(slug) {
                continue;
            }
            if resolved.is_none() {
                if let Some(resolver) = self.ctx.opts.recipes.as_ref().and_then(|f| f()) {
                    resolved = resolver.resolve(slug).await.ok();
                }
            }
            let Some(read) = self.setup.sync_of(&record, resolved.as_ref()).await? else {
                continue;
            };
            if read.change_keys.is_empty() {
                self.setup.sync_soon(&record.id, after_ms);
                continue;
            }
            names.push(record.name.clone());
            if !self.setup.is_setting(&record.id) && !self.setup.is_syncing(&record.id) {
                let since = record.sync.as_ref().map(|s| s.since.clone()).unwrap_or_else(|| iso_at(self.ctx.clock.now() as u64));
                self.setup.mark_sync(&record.id, PlaceSync { state: PlaceSyncState::Behind, changes: read.change_keys.clone(), since }).await?;
            }
            self.setup.sync_soon(&record.id, after_ms);
        }
        Ok(names)
    }

    /// F4 (:783-787).
    pub async fn followers(&self) -> PlaceResult<IndexMap<String, Vec<String>>> {
        let mut out: IndexMap<String, Vec<String>> = IndexMap::new();
        for record in self.records.records().await? {
            if let Some(recipe) = record.recipe.as_deref().filter(|r| *r != NO_RECIPE) {
                out.entry(recipe.to_string()).or_default().push(record.name.clone());
            }
        }
        Ok(out)
    }

    /// SK1-SK4 (:789-813).
    pub async fn skip(&self, place_id: &str, row: &str) -> PlaceResult<PlaceView> {
        let Some(held) = self.records.record_of(place_id).await? else {
            let names: Vec<String> = self.records.records().await?.into_iter().map(|r| r.name).collect();
            return Err(usage_refusal(&no_such_place_refusal(place_id, &names), NAMED_FIX));
        };
        if let Some(live) = self.setup.skipper(&format!("{place_id}/{row}")) {
            live().await?;
        } else {
            let waits = held.setup.as_ref().is_some_and(|s| s.waiting.iter().any(|w| w.row == row));
            let failed = held.applied.as_ref().is_some_and(|a| a.rows.iter().any(|r| r.row.id == row && r.row.outcome == PlaceProvisionRowOutcome::Failed));
            if !waits && !failed {
                return Err(usage_refusal(&nothing_to_skip_line(row, &held.name), "Skip a row the computer's page shows waiting or failed."));
            }
            let stamp = iso_at(self.ctx.clock.now() as u64);
            let row_id = row.to_string();
            self.records
                .change(place_id, move |now: PlaceRecord| {
                    let failed_now = now.applied.as_ref().and_then(|a| a.rows.iter().find(|r| r.row.id == row_id && r.row.outcome == PlaceProvisionRowOutcome::Failed).cloned());
                    let step = failed_now.as_ref().and_then(|r| r.row.step).or_else(|| now.applied.as_ref().and_then(|a| a.rows.iter().find(|r| r.row.id == row_id)).and_then(|r| r.row.step));
                    let label = failed_now
                        .as_ref()
                        .map(|r| r.row.label.clone())
                        .or_else(|| now.setup.as_ref().and_then(|s| s.waiting.iter().find(|w| w.row == row_id)).map(|w| w.label.clone()))
                        .unwrap_or_else(|| row_id.clone());
                    let skipped = HeldRow {
                        row: PlaceProvisionRow {
                            id: row_id.clone(),
                            label,
                            outcome: PlaceProvisionRowOutcome::Skipped,
                            kind: None,
                            step,
                            note: Some(SKIPPED_FOR_NOW.to_string()),
                            fix: None,
                            keys: None,
                            ms: None,
                            earlier: None,
                            project: None,
                        },
                        pick: None,
                        created_at: None,
                    };
                    let mut rows: Vec<HeldRow> = now.applied.as_ref().map(|a| a.rows.iter().filter(|r| r.row.id != row_id).cloned().collect()).unwrap_or_default();
                    rows.push(skipped);
                    let applied = match now.applied.clone() {
                        Some(applied) => HeldApplied { rows, ..applied },
                        None => HeldApplied { hash: String::new(), at: stamp.clone(), rows, items: None },
                    };
                    let setup = now.setup.clone().map(|s| PlaceSetup {
                        waiting: s.waiting.iter().filter(|w| w.row != row_id).cloned().collect(),
                        ..s
                    });
                    Some(PlaceRecord { setup: setup.or(now.setup.clone()), applied: Some(applied), ..now })
                })
                .await?;
            let now = self.records.record_of(place_id).await?;
            if let Some(setup) = now.as_ref().and_then(|n| n.setup.clone()) {
                if setup.state != PlaceSetupState::Running {
                    let applied = now.as_ref().and_then(|n| n.applied.as_ref()).map(applied_view);
                    let (end, said) = setup_outcome(&setup, applied.as_ref());
                    self.setup.setup_frame(PlaceSetupEvent {
                        add_id: setup.add_id.clone(),
                        place_id: place_id.to_string(),
                        line: None,
                        wait: None,
                        landed: None,
                        running: None,
                        end: Some(end),
                        said,
                    });
                }
            }
        }
        let now = self.records.record_of(place_id).await?.unwrap_or(held);
        let default = self.records.default_id().await?;
        Ok(self.views.view_of(&now, default.as_deref()))
    }

    /// ES1 (:815-824).
    pub async fn estimate(&self, reference: &str, choices: RecipeFile) -> PlaceResult<PlaceEstimate> {
        let Some(provision) = self.ctx.wiring.provision.clone() else {
            return Err(HostError::new(NO_ESTIMATE_LINE));
        };
        if !provision.has_estimate() {
            return Err(HostError::new(NO_ESTIMATE_LINE));
        }
        let pending = self
            .views
            .pending_records()
            .await?
            .into_iter()
            .find(|p| p.view.id == reference || p.view.address == reference || p.view.name.as_deref() == Some(reference) || p.view.place_id.as_deref() == Some(reference));
        let place_id = match pending.and_then(|p| p.view.place_id) {
            Some(place_id) => Some(place_id),
            None => self.records.records().await?.into_iter().find(|r| r.id == reference || r.name == reference).map(|r| r.id),
        };
        let disk = match &place_id {
            Some(place_id) => self.records.record_of(place_id).await?.map(|r| r.report),
            None => None,
        };
        let sized = provision.estimate(&choices).await?;
        let disk_size = disk.as_ref().and_then(|d| d.disk_size_bytes.map(|j| j.as_f64() as u64));
        Ok(PlaceEstimate {
            needed_bytes: JsInt(JsNumber(sized.bytes as f64)),
            kept_bytes: JsInt(JsNumber(owned_floor_bytes(disk_size) as f64)),
            free_bytes: disk.as_ref().and_then(|d| d.disk_free_bytes),
            unmeasured: JsInt(JsNumber(sized.unmeasured as f64)),
        })
    }

    /// LG1 (:826-837).
    pub async fn setup_log(&self, place_id: &str, step: Option<PlaceSetupStep>) -> PlaceResult<Vec<String>> {
        let Some(held) = self.records.record_of(place_id).await? else {
            let names: Vec<String> = self.records.records().await?.into_iter().map(|r| r.name).collect();
            return Err(usage_refusal(&no_such_place_refusal(place_id, &names), NAMED_FIX));
        };
        let Some(home) = held.report.login.get("HOME").cloned() else {
            return Ok(Vec::new());
        };
        let machine = PlaceMachine::new(self.setup.link_to(place_id), &held.name, &home);
        let log = place_provision_paths(&home).log;
        let cmd = format!("tail -c {SETUP_LOG_TAIL_BYTES} {} 2>/dev/null", shell_quote(&log));
        let read = machine.exec(&cmd, ExecOptions { timeout_ms: Some(LOG_READ_MS), idempotency_key: None, stdin: None }).await.map_err(|e| host_of(&e))?;
        let lines: Vec<String> = read.stdout.split('\n').filter(|line| !line.is_empty()).map(str::to_string).collect();
        let whole: Vec<String> = if read.stdout.len() >= SETUP_LOG_TAIL_BYTES { lines.into_iter().skip(1).collect() } else { lines };
        Ok(match step {
            None => whole,
            Some(step) => {
                let marker = format!(" [{}] ", step_word(step));
                whole.into_iter().filter(|line| line.contains(&marker)).collect()
            }
        })
    }

    /// F5 (:839-847).
    pub async fn unfollow(&self, slug: &str) -> PlaceResult<Vec<String>> {
        let mut names = Vec::new();
        for record in self.records.records().await? {
            if record.recipe.as_deref() != Some(slug) {
                continue;
            }
            let wanted = slug.to_string();
            let moved = self
                .records
                .change(&record.id, move |now: PlaceRecord| {
                    (now.recipe.as_deref() == Some(wanted.as_str())).then(|| PlaceRecord { recipe: Some(NO_RECIPE.to_string()), ..now })
                })
                .await?;
            if moved.is_some() {
                names.push(record.name.clone());
            }
        }
        self.setup.recipes_moved(slug);
        Ok(names)
    }

    /// F6 (:849).
    pub async fn picks_of(&self, place_id: &str) -> PlaceResult<Option<RecipeFile>> {
        Ok(self.records.record_of(place_id).await?.and_then(|r| r.picks))
    }

    /// C1 (:851-854): answers the id `off` takes.
    pub fn on(&self, f: Rc<dyn Fn(&PlaceEvent)>) -> u64 {
        let id = self.next_watch.get() + 1;
        self.next_watch.set(id);
        let remove = self.ctx.watch(f);
        self.watches.borrow_mut().insert(id, remove);
        id
    }

    /// C1.
    pub fn off(&self, id: u64) {
        let remove = self.watches.borrow_mut().shift_remove(&id);
        if let Some(remove) = remove {
            remove();
        }
    }

    /// C2 (:856-870).
    pub async fn close(&self) -> PlaceResult<()> {
        self.setup.clear_sync_timers();
        if let Some(back) = self.ctx.wiring.back.clone() {
            back.close();
        }
        let live: Vec<String> = self.ctx.live.borrow().keys().cloned().collect();
        for place in live {
            self.views.cut(&place, "this host is stopping");
        }
        for place in self.setup.waiting_ids() {
            self.setup.woken(&place, false);
        }
        let keys: Vec<String> = self.ctx.forwards.borrow().keys().cloned().collect();
        for key in keys {
            let forward = self.ctx.forwards.borrow_mut().shift_remove(&key);
            if let Some(forward) = forward {
                let conns: Vec<_> = forward.conns.borrow_mut().drain(..).map(|(_, conn)| conn).collect();
                for conn in conns {
                    conn.destroy();
                }
                forward.server.close();
                if let Some(stop) = &forward.stop {
                    stop();
                }
            }
        }
        self.records.awaiting.borrow_mut().clear();
        Ok(())
    }
}
