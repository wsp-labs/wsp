// SPDX-License-Identifier: AGPL-3.0-only
//! box-backend.ts:788-1044 at a4033dc34. Spec: spec/box_fleet.md.
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Value};

use super::machine::BoxMachine;
use super::plan::{
    box_budgets, box_class_for, box_labels, box_name, box_root_disk_gb, is_uuid, state_map, state_of, BoxLimits, BoxPricing, SizeAsk,
    BACKSTOP_RETRY_MAX_MS, BACKSTOP_RETRY_MS, BACKSTOP_SLACK_MS, BOX_BASE_TEMPLATE, BOX_CLASSES, BOX_KEY_KEPT_MS, IN_PROGRESS_WAIT_MS,
    LOST_ANSWER_EVERY_MS, LOST_ANSWER_MS,
};
use super::request::{
    encode_uri_component, other, road_words, template_row_of, BackstopFailure, BoxBackend, BoxView, NamedSnapshotView, RequestOpts,
    SnapshotView, Timer, BOX_DISK_KEPT_MS,
};
use crate::errors::{far_end_answered, is_missing, EngineError, Result};
use crate::labels::NAME_LABEL;
use crate::machine::{
    BackendOffers, BackendPricing, BaseTemplates, Capabilities, CloudLimits, Lifecycle, LifecycleBudgets, Machine, MachineBackend,
    MachineKind, MachineListRow, MachineSpec, MachineState, NamedMachine, ProviderBoxState, RootDisk, Seen, Size, SnapshotRow, TemplateRow,
    Waiting,
};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::wire::capabilities::{MachineSizeOffer, PauseMode, WorkspaceSize};
use wsp_words::place_word::provider_key_name;
use wsp_words::words::thread::{create_answer_lost_line, refusal_line};
use wsp_words::words::units::{boat_box_state_line, size_refusal, size_word, SIZE_PICK_FIX};

/// What `postCreate` answers: the create's answer and, where one was lost on the way, what was said then.
#[derive(Debug, Clone, PartialEq)]
pub struct PostedCreate {
    /// The answer's `status`, where it carried one.
    pub status: Option<String>,
    pub sandbox: BoxView,
    pub lost: Option<String>,
}

/// A page reader's stop test: it has what it wanted once this answers true.
type Enough<'a> = Option<&'a dyn Fn(&[Value]) -> bool>;

impl BoxBackend {
    /// box-backend.ts:543. The classes the plan sells off the last read of /limits; every class before the first.
    pub fn box_capabilities(&self) -> Capabilities {
        let sold = self.shared.state.lock().unwrap().limits_kept.as_ref().map(|(_, limits)| limits.sizes.clone());
        let sizes = BOX_CLASSES
            .iter()
            .filter(|c| sold.as_ref().is_none_or(|sold| sold.contains(&c.kind)))
            .map(|c| MachineSizeOffer {
                cpu: JsNumber(c.cpu),
                mem_mb: JsNumber(c.mem_mb),
                rate_usd_per_hour: JsNumber(c.rate_usd_per_hour),
                name: Some(c.name.to_string()),
            })
            .collect();
        Capabilities {
            live_clone_forks: false,
            pause_mode: Some(PauseMode::Disk),
            replaces_machine: true,
            preview_urls: true,
            signed_urls: false,
            callback_relay: true,
            disk_snapshots: true,
            images: true,
            snapshots_any_life: true,
            snapshot_listing: true,
            templates: true,
            kept: false,
            copies: true,
            own_network: true,
            sizes,
        }
    }

    /// box-backend.ts:790. The exact bytes a create of this spec sends.
    pub async fn box_create_request(&self, spec: &MachineSpec) -> Result<String> {
        let named = spec.from_snapshot.as_deref().or(spec.template.as_deref());
        let from = named.filter(|n| *n != BOX_BASE_TEMPLATE);
        let timer_until = spec.idle_timeout_ms.map(|ms| self.shared.clock.now() + ms.0 as i64);
        let ttl = self.ttl_seconds_for(timer_until).await?;
        let labels = spec.labels.clone().unwrap_or_default();
        let mut body = serde_json::Map::new();
        body.insert("type".to_string(), json!(box_class_for(size_ask(spec)).kind.as_str()));
        body.insert("ttlSeconds".to_string(), json!(ttl));
        body.insert("noEnv".to_string(), json!(true));
        body.insert("name".to_string(), json!(box_name(&labels)));
        if let Some(from) = from {
            body.insert("from".to_string(), json!(from));
        }
        Ok(Value::Object(body).to_string())
    }

    /// box-backend.ts:810.
    pub async fn post_create(&self, key: &str, bytes: &str) -> Result<PostedCreate> {
        let started = self.shared.clock.now();
        let mut lost: Option<String> = None;
        let mut ask: u32 = 0;
        loop {
            let opts = RequestOpts {
                bytes: Some(bytes.to_string()),
                headers: vec![("Idempotency-Key".to_string(), key.to_string())],
                cap_ms: Some(self.shared.budgets.read_ms),
                ..RequestOpts::default()
            };
            match self.request("POST", "/sandboxes", opts).await {
                Ok(answer) => {
                    let status = answer.get("status").and_then(Value::as_str).map(str::to_string);
                    let sandbox: BoxView =
                        serde_json::from_value(answer.get("sandbox").cloned().unwrap_or(Value::Null)).map_err(|e| other(e.to_string()))?;
                    return Ok(PostedCreate { status, sandbox, lost });
                }
                Err(e) => {
                    let in_progress = e.code() == Some("idempotency_in_progress");
                    let lost_here = !far_end_answered(&e) || e.status().is_some_and(|s| s >= 500);
                    if (!in_progress && !lost_here) || self.shared.clock.now() - started >= LOST_ANSWER_MS as i64 {
                        return Err(e);
                    }
                    if !in_progress && lost.is_none() {
                        lost = Some(road_words(&e));
                    }
                    let wait = if in_progress {
                        IN_PROGRESS_WAIT_MS
                    } else if ask == 0 {
                        0
                    } else {
                        LOST_ANSWER_EVERY_MS
                    };
                    self.shared.clock.sleep(wait).await;
                }
            }
            ask += 1;
        }
    }

    /// box-backend.ts:831.
    pub async fn box_create(&self, spec: MachineSpec, waiting: Option<Waiting>, sent: Option<String>) -> Result<BoxMachine> {
        let what: Option<String> = spec.labels.as_ref().and_then(|labels| labels.get(NAME_LABEL)).cloned();
        let limits = self.before_start().await?;
        let cls = box_class_for(size_ask(&spec));
        if !limits.sizes.contains(&cls.kind) {
            let word = size_word(&WorkspaceSize { cpu: JsNumber(cls.cpu), mem_mb: JsNumber(cls.mem_mb) });
            let happened = size_refusal(&word, &self.box_capabilities().sizes);
            return Err(EngineError::Invalid { message: refusal_line(&happened, SIZE_PICK_FIX) });
        }
        let timer_until = spec.idle_timeout_ms.map(|ms| self.shared.clock.now() + ms.0 as i64);
        let bytes = match sent {
            Some(bytes) => bytes,
            None => self.box_create_request(&spec).await?,
        };
        let name = name_of(&bytes)?;
        let key = spec.idempotency_key.clone().unwrap_or_else(uuid_v4);
        let posted = match self.post_create(&key, &bytes).await {
            Ok(posted) => posted,
            Err(e) => return Err(self.start_refused(e, what.as_deref()).await),
        };
        let replayed = posted.lost.is_none() && posted.status.as_deref().is_some_and(|s| s != "provisioning");
        if !replayed {
            self.counted();
        }
        let id = posted.sandbox.id.clone();
        if let Some(said) = &posted.lost {
            self.warn(&create_answer_lost_line(&provider_key_name("box"), said, &id));
        }
        let machine = BoxMachine::new(self.clone(), &id, spec.kind, spec.labels.clone(), None, replayed);
        if let Some(until) = timer_until {
            let span_ms = spec.idle_timeout_ms.map_or(0, |ms| ms.0 as i64);
            self.shared.state.lock().unwrap().backstops.insert(id.clone(), Timer { until, span_ms });
        }
        let said: RefCell<Option<String>> = RefCell::new(None);
        let tell = |state: &str| {
            if said.borrow().as_deref() == Some(state) {
                return;
            }
            *said.borrow_mut() = Some(state.to_string());
            if let Some(waiting) = &waiting {
                let line = boat_box_state_line(&id, state);
                waiting(&line, Some(&ProviderBoxState { id: id.clone(), state: state.to_string() }));
            }
        };
        tell(&posted.sandbox.state);
        let seen = |view: &BoxView| tell(&view.state);
        let seen: &dyn Fn(&BoxView) = &seen;
        let done = |view: &BoxView| state_map(&view.state) == Some(MachineState::Running);
        let done: &dyn Fn(&BoxView) -> bool = &done;
        let finished: Result<()> = async {
            let view = self.settle(&id, done, self.shared.budgets.create_ms, "create", None, what.as_deref(), Some(seen)).await?;
            machine.prove_road(None).await?;
            if view.name != name {
                let path = format!("/sandboxes/{}", encode_uri_component(&id));
                self.request("PATCH", &path, RequestOpts { body: Some(json!({ "name": name })), ..RequestOpts::default() }).await?;
            }
            if let Some(envs) = spec.envs.as_ref().filter(|envs| !envs.is_empty()) {
                machine.land_envs(envs).await?;
            }
            Ok(())
        }
        .await;
        if let Err(e) = finished {
            if !replayed {
                let _ = machine.kill().await;
            }
            return Err(e);
        }
        Ok(machine)
    }

    /// box-backend.ts:880. `made_at` is an ISO date as `Date.parse` reads it.
    pub async fn box_replay_create(&self, key: &str, bytes: &str, made_at: &str) -> Result<Option<BoxMachine>> {
        let window = BOX_KEY_KEPT_MS as i64 - LOST_ANSWER_MS as i64 - LOST_ANSWER_EVERY_MS as i64;
        if self.shared.clock.now() as f64 - crate::orphans::date_parse(made_at) >= window as f64 {
            return Ok(None);
        }
        let posted = self.post_create(key, bytes).await?;
        let name = name_of(bytes)?;
        let state = state_of(&posted.sandbox.state);
        if state != MachineState::Gone && posted.sandbox.name != name {
            let path = format!("/sandboxes/{}", encode_uri_component(&posted.sandbox.id));
            self.request("PATCH", &path, RequestOpts { body: Some(json!({ "name": name })), ..RequestOpts::default() }).await?;
        }
        let seen = Seen { state, created_at: posted.sandbox.created_at.clone(), paused_at: None };
        Ok(Some(BoxMachine::new(self.clone(), &posted.sandbox.id, MachineKind::Sandbox, box_labels(name.as_deref()), Some(seen), true)))
    }

    /// box-backend.ts:894.
    pub async fn box_named_after(&self, names: &BTreeSet<String>) -> Result<Vec<NamedMachine>> {
        let mut out: IndexMap<String, NamedMachine> = IndexMap::new();
        for b in self.every_box().await? {
            let Some(name) = b.name.as_ref() else { continue };
            if !names.contains(name) || out.contains_key(&b.id) {
                continue;
            }
            let machine =
                NamedMachine { id: b.id.clone(), name: name.clone(), state: state_of(&b.state), created_at: b.created_at.clone() };
            out.insert(b.id.clone(), machine);
        }
        Ok(out.into_values().collect())
    }

    /// box-backend.ts:903.
    pub async fn box_get(&self, id: &str) -> Result<BoxMachine> {
        let view = self.view(id).await?;
        let state = state_of(&view.state);
        let paused_at = if state == MachineState::Paused { view.snapshot_completed_at.clone() } else { None };
        let seen = Seen { state, created_at: view.created_at.clone(), paused_at };
        Ok(BoxMachine::new(self.clone(), &view.id, MachineKind::Sandbox, box_labels(view.name.as_deref()), Some(seen), false))
    }

    /// box-backend.ts:917. Every row of a cursor-paged listing, 200 a page, until no cursor or `enough` says so.
    pub async fn pages(&self, path: &str, rows_key: &str, query: &[(&str, &str)], enough: Enough<'_>) -> Result<Vec<Value>> {
        let mut out: Vec<Value> = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut params: Vec<String> = query.iter().map(|(k, v)| format!("{}={}", form_encode(k), form_encode(v))).collect();
            params.push("limit=200".to_string());
            if let Some(cursor) = &cursor {
                params.push(format!("cursor={}", form_encode(cursor)));
            }
            let page = self.request("GET", &format!("{path}?{}", params.join("&")), RequestOpts::default()).await?;
            if let Some(rows) = page.get(rows_key).and_then(Value::as_array) {
                out.extend(rows.iter().cloned());
            }
            cursor = page.get("pageInfo").and_then(|info| info.get("nextCursor")).and_then(Value::as_str).map(str::to_string);
            if cursor.is_none() || enough.is_some_and(|enough| enough(&out)) {
                return Ok(out);
            }
        }
    }

    /// box-backend.ts:932. The plain listing then the archived one, duplicates kept.
    pub async fn every_box(&self) -> Result<Vec<BoxView>> {
        let mut rows = self.pages("/sandboxes", "sandboxes", &[], None).await?;
        rows.extend(self.pages("/sandboxes", "sandboxes", &[("state", "archived")], None).await?);
        rows.into_iter().map(|row| serde_json::from_value::<BoxView>(row).map_err(|e| other(e.to_string()))).collect()
    }

    /// box-backend.ts:940.
    pub async fn box_list(&self, labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        let mut boxes: IndexMap<String, BoxView> = IndexMap::new();
        for b in self.every_box().await? {
            boxes.entry(b.id.clone()).or_insert(b);
        }
        let mut out = Vec::new();
        for b in boxes.into_values() {
            let Some(carried) = box_labels(b.name.as_deref()) else { continue };
            if labels.is_some_and(|asked| asked.iter().any(|(k, v)| carried.get(k) != Some(v))) {
                continue;
            }
            let size = match (b.vcpu, b.memory_gb) {
                (Some(cpu), Some(gb)) => Some(WorkspaceSize { cpu: JsNumber(cpu), mem_mb: JsNumber(gb * 1024.0) }),
                _ => None,
            };
            out.push(MachineListRow { id: b.id.clone(), state: state_of(&b.state), labels: carried, size });
        }
        Ok(out)
    }

    /// box-backend.ts:965.
    pub async fn box_list_snapshots(&self, ids: Option<&[String]>) -> Result<Vec<SnapshotRow>> {
        let named = self.named_snapshots().await?;
        let wanted: Option<Vec<String>> = ids.map(|ids| ids.iter().filter(|id| is_uuid(id)).cloned().collect());
        let history: Vec<Value> = if wanted.as_ref().is_some_and(|w| w.is_empty()) {
            Vec::new()
        } else {
            let seen = |read: &[Value]| {
                wanted
                    .as_ref()
                    .is_some_and(|w| w.iter().all(|id| read.iter().any(|s| s.get("id").and_then(Value::as_str) == Some(id.as_str()))))
            };
            let enough: Enough<'_> = if wanted.is_some() { Some(&seen) } else { None };
            self.pages("/snapshots", "snapshots", &[], enough).await?
        };
        let mut rows: Vec<SnapshotRow> = named
            .into_iter()
            .map(|s| SnapshotRow {
                id: s.name.clone(),
                name: Some(s.name.clone()),
                size_bytes: JsNumber(s.size_bytes.unwrap_or(0.0)),
                restored_bytes: None,
                created_at: Some(s.created_at.clone()),
                parent: None,
            })
            .collect();
        for raw in history {
            let s: SnapshotView = serde_json::from_value(raw).map_err(|e| other(e.to_string()))?;
            let created_at = s.completed_at.clone().or(s.created_at.clone()).filter(|at| !at.is_empty());
            rows.push(SnapshotRow {
                id: s.id.clone(),
                name: None,
                size_bytes: JsNumber(s.size_bytes.unwrap_or(0.0)),
                restored_bytes: s.content_size_bytes.map(JsNumber),
                created_at,
                parent: s.chain_id.clone(),
            });
        }
        Ok(rows)
    }

    /// box-backend.ts:982.
    pub async fn box_delete_snapshot(&self, id: &str) -> Result<()> {
        let encoded = encode_uri_component(id);
        let deleted = if is_uuid(id) {
            let opts = RequestOpts { headers: vec![("X-Ascii-Confirm-Delete".to_string(), id.to_string())], ..RequestOpts::default() };
            self.request("DELETE", &format!("/snapshots/{encoded}"), opts).await
        } else {
            self.request("DELETE", &format!("/named-snapshots/{encoded}"), RequestOpts::default()).await
        };
        match deleted {
            Ok(_) => Ok(()),
            Err(e) if is_missing(&e) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// box-backend.ts:993.
    pub async fn named_snapshots(&self) -> Result<Vec<NamedSnapshotView>> {
        let page = self.request("GET", "/named-snapshots", RequestOpts::default()).await?;
        let Some(rows) = page.get("snapshots").and_then(Value::as_array) else {
            return Err(other("GET /named-snapshots answered without a snapshots array".to_string()));
        };
        rows.iter().cloned().map(|row| serde_json::from_value::<NamedSnapshotView>(row).map_err(|e| other(e.to_string()))).collect()
    }

    /// box-backend.ts:1004.
    pub async fn box_get_template(&self, id: &str) -> Result<TemplateRow> {
        let raw = self.request("GET", &format!("/named-snapshots/{}", encode_uri_component(id)), RequestOpts::default()).await?;
        let snapshot: NamedSnapshotView =
            serde_json::from_value(raw.get("snapshot").cloned().unwrap_or(Value::Null)).map_err(|e| other(e.to_string()))?;
        Ok(template_row_of(&snapshot))
    }

    /// box-backend.ts:1008.
    pub async fn box_list_templates(&self) -> Result<Vec<TemplateRow>> {
        Ok(self.named_snapshots().await?.iter().map(template_row_of).collect())
    }

    /// box-backend.ts:1019. `until` in ms since the epoch.
    pub async fn box_backstop(&self, machine_id: &str, until: i64, exact: bool) -> Result<()> {
        let failed = self.shared.state.lock().unwrap().backstop_failures.get(machine_id).copied();
        {
            let mut state = self.shared.state.lock().unwrap();
            let now = self.shared.clock.now();
            let in_backoff = failed.is_some_and(|f| now - f.at < f.wait_ms as i64);
            if !exact && (state.backstops_sending.contains(machine_id) || in_backoff) {
                return Ok(());
            }
            state.backstops_sending.insert(machine_id.to_string());
        }
        let sent = self.send_backstop(machine_id, until, exact).await;
        self.shared.state.lock().unwrap().backstops_sending.remove(machine_id);
        if sent.is_err() {
            let wait_ms = failed.map_or(BACKSTOP_RETRY_MS, |f| (f.wait_ms * 2).min(BACKSTOP_RETRY_MAX_MS));
            let at = self.shared.clock.now();
            self.shared.state.lock().unwrap().backstop_failures.insert(machine_id.to_string(), BackstopFailure { at, wait_ms });
        }
        sent
    }

    async fn send_backstop(&self, machine_id: &str, until: i64, exact: bool) -> Result<()> {
        let cap = self.ttl_cap().await?;
        let now = self.shared.clock.now();
        let instant = match cap {
            None => until,
            Some(cap) => until.min(now + cap as i64 * 1000),
        };
        let last = self.shared.state.lock().unwrap().backstops.get(machine_id).copied();
        if !exact && last.is_some_and(|last| instant - last.until < BACKSTOP_SLACK_MS as i64) {
            return Ok(());
        }
        let ttl_seconds = ((instant - now) as f64 / 1000.0).ceil().max(1.0) as u64;
        let opts = RequestOpts {
            body: Some(json!({ "ttlSeconds": ttl_seconds })),
            cap_ms: Some(self.shared.budgets.backstop_ms),
            once: true,
            ..RequestOpts::default()
        };
        self.request("PATCH", &format!("/sandboxes/{}", encode_uri_component(machine_id)), opts).await?;
        let mut state = self.shared.state.lock().unwrap();
        state.backstops.insert(machine_id.to_string(), Timer { until: instant, span_ms: instant - now });
        state.backstop_failures.remove(machine_id);
        Ok(())
    }

    /// box-backend.ts:1040.
    pub fn forget_backstop(&self, id: &str) {
        let mut state = self.shared.state.lock().unwrap();
        state.backstops.remove(id);
        state.backstop_failures.remove(id);
    }
}

/// box-backend.ts:557. Budgets [`super::plan::box_budgets`], the backstop through the backend, disk kept 14 days.
pub struct BoxLifecycle {
    pub backend: BoxBackend,
}

#[async_trait(?Send)]
impl Lifecycle for BoxLifecycle {
    fn budgets(&self) -> LifecycleBudgets {
        box_budgets()
    }
    fn has_backstop(&self) -> bool {
        true
    }
    async fn backstop(&self, machine: &dyn Machine, until_ms: i64, exact: bool) -> Result<()> {
        self.backend.box_backstop(machine.id(), until_ms, exact).await
    }
    fn disk_kept_ms(&self) -> Option<u64> {
        Some(BOX_DISK_KEPT_MS)
    }
}

#[async_trait(?Send)]
impl MachineBackend for BoxBackend {
    fn capabilities(&self) -> Capabilities {
        self.box_capabilities()
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(BoxPricing)
    }
    fn lifecycle(&self) -> Option<Arc<dyn Lifecycle>> {
        Some(Arc::new(BoxLifecycle { backend: self.clone() }))
    }
    /// sandbox and desktop both boot `base`.
    fn base_templates(&self) -> Option<crate::machine::BaseTemplates> {
        Some(BaseTemplates { sandbox: BOX_BASE_TEMPLATE.to_string(), desktop: BOX_BASE_TEMPLATE.to_string() })
    }
    /// keyed_create, check_key, place_limits, root_disk_of, limits, list_snapshots, templates.
    fn offers(&self) -> BackendOffers {
        BackendOffers {
            keyed_create: true,
            check_key: true,
            place_limits: true,
            root_disk_of: true,
            limits: true,
            list_snapshots: true,
            templates: true,
            ..BackendOffers::default()
        }
    }
    /// box-backend.ts:551.
    fn root_disk_of(&self, size: Size) -> Option<RootDisk> {
        let cls = BOX_CLASSES.iter().find(|c| c.cpu == size.cpu && c.mem_mb == size.mem_mb)?;
        let disk_gb = box_root_disk_gb(cls.kind)?;
        Some(RootDisk { name: cls.kind.as_str().to_string(), disk_gb })
    }
    async fn create(&self, spec: MachineSpec, waiting: Option<Waiting>, sent: Option<String>) -> Result<Arc<dyn Machine>> {
        Ok(Arc::new(self.box_create(spec, waiting, sent).await?))
    }
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>> {
        Ok(Arc::new(self.box_get(id).await?))
    }
    async fn list(&self, labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        self.box_list(labels).await
    }
    async fn delete_snapshot(&self, id: &str) -> Result<()> {
        self.box_delete_snapshot(id).await
    }
    async fn create_request(&self, spec: &MachineSpec) -> Result<String> {
        self.box_create_request(spec).await
    }
    async fn replay_create(&self, key: &str, bytes: &str, made_at: &str) -> Result<Option<Arc<dyn Machine>>> {
        Ok(self.box_replay_create(key, bytes, made_at).await?.map(|m| Arc::new(m) as Arc<dyn Machine>))
    }
    async fn named_after(&self, names: &BTreeSet<String>) -> Result<Vec<NamedMachine>> {
        self.box_named_after(names).await
    }
    /// box-backend.ts:747: the account's limits, as JSON.
    async fn check_key(&self) -> Result<Value> {
        serde_json::to_value(self.read_limits().await?).map_err(|e| other(e.to_string()))
    }
    async fn place_limits(&self) -> Result<Option<CloudLimits>> {
        Ok(self.place_limits().await)
    }
    async fn limits(&self) -> Result<BoxLimits> {
        self.limits().await
    }
    async fn list_snapshots(&self, ids: Option<&[String]>) -> Result<Vec<SnapshotRow>> {
        self.box_list_snapshots(ids).await
    }
    /// box-backend.ts:1000: the template is the name it was saved under.
    async fn promote_snapshot(&self, snapshot_id: &str, _name: &str) -> Result<String> {
        Ok(snapshot_id.to_string())
    }
    async fn get_template(&self, id: &str) -> Result<TemplateRow> {
        self.box_get_template(id).await
    }
    async fn list_templates(&self) -> Result<Vec<TemplateRow>> {
        self.box_list_templates().await
    }
    async fn delete_template(&self, id: &str) -> Result<()> {
        self.box_delete_snapshot(id).await
    }
}

/// box-backend.ts:786. The spec's size as `boxClassFor` reads it.
fn size_ask(spec: &MachineSpec) -> SizeAsk {
    SizeAsk { cpu: spec.cpu.map(|c| c.0), mem_mb: spec.mem_mb.map(|m| m.0) }
}

/// `JSON.parse(bytes).name`: the name a create's bytes carry, `None` where they carry none.
fn name_of(bytes: &str) -> Result<Option<String>> {
    let raw: Value = serde_json::from_str(bytes).map_err(|e| other(e.to_string()))?;
    Ok(raw.get("name").and_then(Value::as_str).map(str::to_string))
}

/// A fresh UUID v4, lowercase, as `crypto.randomUUID()` writes one.
fn uuid_v4() -> String {
    let mut b: [u8; 16] = rand::random();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = hex::encode(b);
    format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

/// `URLSearchParams`' encoding: letters, digits and `*-._` as they are, a space as `+`, any other byte as `%XX`.
fn form_encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b == b' ' {
            out.push('+');
        } else if b.is_ascii_alphanumeric() || b"*-._".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
