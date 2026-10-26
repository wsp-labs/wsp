// SPDX-License-Identifier: AGPL-3.0-only
//! sub-spawn.ts:25-30, 76-281, 343-412: the record and every check before it. Spec: spec/sub_spawn_checks.md.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{json, Map, Value};

use indexmap::{IndexMap, IndexSet};
use wsp_catalog::git_hosts::{git_host_of, remote_host, GitHost, GitHostId};
use wsp_engine::box_backend::{BoxLimits, BOX_CLASSES};
use wsp_engine::machine::{BackendPricing, ExecOptions, ExecResult, MachineBackend, Size};
use wsp_engine::exec_detached::put::INLINE_EXEC_MS;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::init_job::LoginState;
use wsp_wire::types::shell_quote::shell_line;
use wsp_wire::types::sub_thread::{SubOut, SubOutBase, SubOutNotSent, SubOutSent, SubOutSize, SUB_SEED_MAX_BYTES};
use wsp_wire::views::golden_image::{golden_head, GoldenManifest, GoldenVersion};
use wsp_wire::views::workspace::{BranchOfRecord, Caller, ProjectView, scope_of};
use wsp_wire::wire::daemon::{GitStatusReply, DETACHED_HEAD};
use wsp_words::exit::{HostError, Kind};
use wsp_words::js::js_round;
use wsp_words::place_word::{names_place, provider_key_name, PlaceRef};
use wsp_words::projects::{copies_folder, kind_for_computer, runs_in_folder};
use wsp_words::start::owner_repo_of;
use wsp_words::words::computer::branch_unread_refusal;
use wsp_words::sub_thread::*;
use wsp_words::sub_thread::CpuMem;
use wsp_words::words::base::last_line;
use wsp_words::words::units::{boat_plan_words, fmt_bytes, starts_used_line, StartWindow};

use super::{ImageRecord, ProviderRow, Seed, SubSpawnArea};
use crate::machines::area_error::{AreaError, AreaResult};
use crate::machines::machines::Entry;
use wsp_wire::types::place_state::DELETE_AFTER_MS;
use wsp_core::types::wiring::WorkspaceRecord;

/// sub-spawn.ts:26. The image every sub-machine forks.
pub const SETUP_IMAGE: &str = "default";
/// sub-spawn.ts:30. The states a sealed login stands in on a fork.
pub const LOGIN_HELD: [&str; 2] = ["copied", "signed-in"];

/// sub-spawn.ts:28. The words a git host's sign-in goes by.
pub fn host_words(host: &GitHost) -> &'static str {
    match host.id {
        GitHostId::Github => "GitHub",
        GitHostId::Gitlab => "GitLab",
    }
}

/// What a start with `sub` asks (types/wiring.ts:98 `SubAsk`), beside the create's name and project word.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpawnAsk {
    pub name: String,
    pub project: String,
    pub on: String,
    pub size: Option<String>,
    pub from: Option<String>,
    pub branch: Option<String>,
    pub harness: Option<String>,
}

/// The sub half of the answer (types/wiring.ts `SubStarted`).
#[derive(Clone, Debug, PartialEq)]
pub struct SubSpawned {
    pub out: SubOut,
    pub folder: String,
    pub preamble: String,
    pub harness: String,
}

/// `CreatedWorkspace` as a spawn answers it: the record's view, then `notice?`, then `sub`.
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnAnswer {
    pub view: Value,
    pub notice: Option<String>,
    pub sub: SubSpawned,
}

/// The image head a spawn forks and the image record beside it (sub-spawn.ts:104).
#[derive(Clone, Debug)]
pub struct ImageAt {
    pub head: GoldenVersion,
    pub record: Option<ImageRecord>,
}

/// Releases the place a spawn's guard took, on every exit (sub-spawn.ts `finally { freePlace() }`).
struct FreePlace(Option<Box<dyn FnOnce()>>);

impl Drop for FreePlace {
    fn drop(&mut self) {
        if let Some(release) = self.0.take() {
            release();
        }
    }
}

fn login_state_word(state: &LoginState) -> String {
    serde_json::to_value(state).ok().and_then(|word| word.as_str().map(str::to_string)).unwrap_or_default()
}

fn is_commit_word(name: &str) -> bool {
    (7..=40).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_hexdigit())
}

fn cpu_mem(size: &Size) -> CpuMem {
    CpuMem { cpu: size.cpu, mem_mb: size.mem_mb }
}

impl SubSpawnArea {
    /// sub-spawn.ts:137 `spawnSub` (rules 2-5).
    pub async fn spawn_sub(&self, ask: SpawnAsk, origin: Option<&Caller>) -> Result<SpawnAnswer, HostError> {
        self.spawn_sub_checked(ask, origin).await.map_err(AreaError::into_host)
    }

    async fn spawn_sub_checked(&self, ask: SpawnAsk, origin: Option<&Caller>) -> AreaResult<SpawnAnswer> {
        let began = self.ctx.now();
        let name = self.ctx.name_given(&ask.name);
        let scope = scope_of(origin).ok_or_else(|| AreaError::Host(sub_no_thread_refusal(&ask.on)))?;
        let free = self.ctx.spawn_guard(origin).map_err(|mut e| {
            e.kind.get_or_insert(Kind::Usage);
            AreaError::Host(e)
        })?;
        let _free = FreePlace(Some(free));
        let providers: Vec<ProviderRow> = self.ctx.computer_rows().await?.into_iter().filter(|r| r.kind == "provider").collect();
        let row = providers.iter().find(|r| names_place(&PlaceRef { id: r.id.clone(), name: r.name.clone(), kind: None }, &ask.on)).cloned();
        let at = row.as_ref().and_then(|row| self.provider_at(&row.id));
        let (row, at) = match (row, at) {
            (Some(row), Some(at)) => (row, at),
            _ => {
                let held: Vec<String> = providers.iter().filter(|r| self.provider_at(&r.id).is_some()).map(|r| provider_key_name(&r.name)).collect();
                return Err(AreaError::Host(sub_no_cloud_refusal(&ask.on, &held)));
            }
        };
        let on = provider_key_name(&row.name);
        let place_id = self.ctx.landing_place(&row.id).await?;
        let place = place_id.clone().unwrap_or_else(|| row.id.clone());
        let lead = self.ctx.live(&scope.workspace_id).expect("a scope's workspace is a live thread");
        let (lead_kind, lead_name, lead_project) = {
            let lead_record = lead.borrow().record.borrow().clone();
            (lead_record.kind, lead_record.name.clone(), lead_record.project.clone())
        };
        if !runs_in_folder(&lead_kind) {
            return Err(AreaError::Host(sub_lead_on_machine_refusal(&on, &scope.thread_id, &lead_name)));
        }
        let project = self.ctx.project_held(&lead_project);
        if !ask.project.is_empty() && ask.project != project.id && ask.project != project.name {
            return Err(AreaError::Host(sub_other_project_refusal(&ask.on, &ask.project, &project.name)));
        }
        if project.remote.is_empty() {
            return Err(AreaError::Host(sub_no_remote_refusal(&project.name, &on)));
        }
        let Some(host) = git_host_of(&project.remote) else {
            let host_word = remote_host(&project.remote).unwrap_or_else(|| project.remote.clone());
            return Err(AreaError::Host(sub_other_host_refusal(&project.name, &host_word, &on)));
        };
        let owner_repo = owner_repo_of(&project.remote).unwrap_or_default();
        let repo = host.https_url(&owner_repo);
        self.ctx.copy_blocked(&lead).await?;
        let frame = json!({ "op": "git.status", "cwd": self.ctx.checkout_of(&lead.borrow().record.borrow()) });
        let said = self.ctx.daemon_frame(&lead, frame).await.map_err(|e| AreaError::Host(HostError::new(branch_unread_refusal(&lead_name, &e.message()))))?;
        let read: GitStatusReply = serde_json::from_value(said).map_err(|e| AreaError::Host(HostError::new(e.to_string())))?;
        let head = read.branch.head.clone();
        if ask.from.is_none() {
            let pushed = format!("origin/{}", read.default_branch.as_deref().unwrap_or("main"));
            if head.is_empty() || head == DETACHED_HEAD {
                return Err(AreaError::Host(sub_detached_refusal(&project.name, &pushed)));
            }
            if read.branch.upstream.is_none() {
                return Err(AreaError::Host(sub_no_upstream_refusal(&head, &pushed)));
            }
            let ahead = read.branch.ahead.0;
            if ahead > 0.0 {
                return Err(AreaError::Host(sub_ahead_refusal(&head, ahead, &pushed)));
            }
            if read.counts_unknown == Some(true) {
                return Err(AreaError::Host(sub_ahead_unknown_refusal(&head, &pushed)));
            }
        }
        let base = match &ask.from {
            None => BaseAt { branch: head.clone(), sha: read.branch.oid.clone() },
            Some(from) => {
                let (branch, sha) = self.from_ref(from, &lead, read.default_branch.as_deref()).await?;
                BaseAt { branch, sha }
            }
        };
        let uncommitted = if ask.from.is_none() { read.entries.iter().filter(|e| e.xy != "!!").count() as u64 } else { 0 };
        let brokered = self.ctx.brokered("cloud", place_id.as_deref());
        let image = self.image_at(&place, &on, &host, &project.remote, brokered).await?;
        let limits = if at.offers().limits { Some(at.limits().await?) } else { None };
        let sold = self.sold_by(&*at, limits.as_ref());
        let kept = self.kept_size(&place).and_then(|size| {
            Some(CpuMem { cpu: size.get("cpu")?.as_f64()?, mem_mb: size.get("memMb")?.as_f64()? })
        });
        let word = match ask.size.clone() {
            Some(word) => word,
            None => sub_size_word(&kept.unwrap_or_else(|| cpu_mem(&at.pricing().default_size()))),
        };
        let size = sub_size_of(&word).and_then(|asked| sold.iter().find(|s| s.cpu == asked.cpu && s.mem_mb == asked.mem_mb).cloned());
        let Some(size) = size else {
            return Err(AreaError::Host(sub_size_refusal(&on, &word, &sold)));
        };
        if let Some(branch) = &ask.branch {
            if !sub_branch_name_ok(branch) {
                return Err(AreaError::Host(sub_branch_name_refusal(branch)));
            }
            if *branch == head {
                return Err(AreaError::Host(sub_lead_branch_refusal(branch)));
            }
        }
        let result = match ask.branch.clone() {
            Some(branch) => branch,
            None => self.free_branch(&sub_branch_of(&name), &lead).await,
        };
        let branch_of_record = if self.ctx.brokered_start() {
            let mut of_record = self.ctx.repo_of_record(&project.remote, &project.name, &on).await?;
            of_record.insert("branch".to_string(), json!(result));
            of_record.insert("startSha".to_string(), json!(base.sha));
            Some(serde_json::from_value::<BranchOfRecord>(Value::Object(of_record)).map_err(|e| AreaError::Host(HostError::new(e.to_string())))?)
        } else {
            None
        };
        self.ctx.place_guard(&place).await?;
        let harness = match ask.harness.clone() {
            Some(harness) => harness,
            None => self.ctx.default_agent_of(&lead).await?,
        };
        self.agent_on(image.record.as_ref(), &on, &harness, brokered)?;
        let seed = self.seed_of(&project).await?;
        self.starts_left(limits.as_ref(), &name)?;

        if let Some(taken) = self.ctx.name_refusal(&name) {
            return Err(AreaError::Host(HostError::with_kind(taken, Kind::Conflict)));
        }
        let folder = sub_folder_of(&project.remote);
        let delete_after_minutes = js_round(self.ctx.delete_after_ms(&place).unwrap_or(DELETE_AFTER_MS as f64) / 60_000.0);
        let id = format!("ws_{}", self.ctx.random_hex(4));
        let mut record = Map::new();
        record.insert("id".to_string(), json!(id));
        record.insert("name".to_string(), json!(name));
        record.insert("kind".to_string(), json!("cloud"));
        record.insert("machineId".to_string(), json!(""));
        record.insert("phase".to_string(), json!("running"));
        record.insert("golden".to_string(), json!(image.head.snapshot_id));
        record.insert("createdAt".to_string(), json!(wsp_words::js_date::iso_string(self.ctx.now()).unwrap_or_default()));
        record.insert("project".to_string(), json!(project.id));
        record.insert("spec".to_string(), json!({}));
        if let Some(place_id) = &place_id {
            record.insert("place".to_string(), json!(place_id));
        }
        record.insert("size".to_string(), json!({ "cpu": size.cpu, "memMb": size.mem_mb }));
        record.insert("firstLife".to_string(), json!(true));
        for (key, value) in self.ctx.tree_of(&scope) {
            record.insert(key, value);
        }
        record.insert("lead".to_string(), json!({ "threadId": scope.thread_id, "workspaceId": lead.borrow().record.borrow().id }));
        if let Some(of_record) = &branch_of_record {
            record.insert("branchOfRecord".to_string(), serde_json::to_value(of_record).unwrap_or_default());
        }
        let mut spawn = Map::new();
        spawn.insert("startedAt".to_string(), json!(began));
        spawn.insert("branch".to_string(), json!({ "branch": base.branch, "sha": base.sha, "ms": self.ctx.now() - began }));
        if uncommitted > 0 {
            spawn.insert("uncommitted".to_string(), json!(uncommitted));
        }
        if let Some(why) = &seed.why {
            spawn.insert("notSent".to_string(), json!(why));
        }
        spawn.insert("log".to_string(), json!([]));
        let mut sub = Map::new();
        sub.insert("on".to_string(), json!(on.to_lowercase()));
        sub.insert("repo".to_string(), json!(repo));
        sub.insert("base".to_string(), json!({ "branch": base.branch, "sha": base.sha }));
        sub.insert("branch".to_string(), json!(result));
        sub.insert("folder".to_string(), json!(folder));
        sub.insert("rateUsdPerHour".to_string(), json!(size.rate_usd_per_hour));
        sub.insert("sizeName".to_string(), json!(size.name));
        sub.insert("spawn".to_string(), Value::Object(spawn));
        record.insert("sub".to_string(), Value::Object(sub));
        let machine_record: WorkspaceRecord = serde_json::from_value(Value::Object(record)).map_err(|e| AreaError::Host(HostError::new(e.to_string())))?;
        let machine_record = Rc::new(RefCell::new(machine_record));

        self.ctx.forking_add(&name);
        self.ctx.supersede_failed(&name);
        let wait = self.ctx.queue_wait(&place, limits.as_ref());
        let queued = self.ctx.enqueue(&machine_record, wait).await;
        self.ctx.forking_remove(&name);
        queued?;

        let mut notices: Vec<String> = Vec::new();
        if uncommitted > 0 {
            notices.push(sub_uncommitted_line(uncommitted as f64, &project.name, &base.branch, &base.sha));
        }
        if let Some(why) = &seed.why {
            notices.push(sub_not_sent_line(why, &project.name));
        }
        if !seed.stayed.is_empty() {
            notices.push(sub_stayed_line(&seed.stayed));
        }
        let out = SubOut {
            on: on.to_lowercase(),
            machine: name.clone(),
            size: SubOutSize { cpu: JsInt(JsNumber(size.cpu)), mem_mb: JsInt(JsNumber(size.mem_mb)) },
            rate_usd_per_hour: JsNumber(size.rate_usd_per_hour),
            repo: repo.clone(),
            base: SubOutBase { branch: base.branch.clone(), sha: base.sha.clone() },
            branch: result.clone(),
            sent: SubOutSent { paths: seed.paths.clone(), bytes: JsInt(JsNumber(seed.bytes as f64)), missing: seed.missing.clone() },
            not_sent: SubOutNotSent { uncommitted: JsInt(JsNumber(uncommitted as f64)), why: seed.why.clone() },
            delete_after_minutes: JsInt(JsNumber(delete_after_minutes)),
        };
        let preamble = sub_preamble(&SubPreamble {
            on: on.clone(),
            size: size.clone(),
            lead: scope.thread_id.clone(),
            folder: folder.clone(),
            sent: seed.paths.clone(),
            setup_git: host.setup_git.to_string(),
            repo,
            base,
            branch: result.clone(),
            delete_after_minutes,
            brokered: branch_of_record.is_some().then_some(true),
        });
        let view = self.ctx.view(&machine_record.borrow());
        let notice = (!notices.is_empty()).then(|| notices.join("\n"));
        Ok(SpawnAnswer { view, notice, sub: SubSpawned { out, folder, preamble, harness } })
    }

    /// sub-spawn.ts:77 `providerAt`.
    pub(crate) fn provider_at(&self, id: &str) -> Option<Arc<dyn MachineBackend>> {
        self.ctx.provider_backend(id)
    }

    /// sub-spawn.ts:80 (rule 6).
    pub(crate) fn sold_by(&self, at: &dyn MachineBackend, limits: Option<&BoxLimits>) -> Vec<SubSize> {
        match limits {
            Some(limits) => BOX_CLASSES
                .iter()
                .filter(|class| limits.sizes.contains(&class.kind))
                .map(|class| SubSize { name: class.name.to_string(), cpu: class.cpu, mem_mb: class.mem_mb, rate_usd_per_hour: class.rate_usd_per_hour })
                .collect(),
            None => at
                .capabilities()
                .sizes
                .iter()
                .map(|offer| {
                    let cpu = offer.cpu.0;
                    let mem_mb = offer.mem_mb.0;
                    let name = offer.name.clone().unwrap_or_else(|| sub_size_word(&CpuMem { cpu, mem_mb }));
                    SubSize { name, cpu, mem_mb, rate_usd_per_hour: offer.rate_usd_per_hour.0 }
                })
                .collect(),
        }
    }

    /// sub-spawn.ts:86 `settingOf(place, "size")`.
    pub(crate) fn kept_size(&self, place: &str) -> Option<Value> {
        self.ctx.place_settings(place).and_then(|settings| settings.get("size").cloned())
    }

    /// sub-spawn.ts:93 (rule 7).
    pub(crate) async fn host_signed_in(&self) -> bool {
        let Ok(machine) = self.ctx.local_machine().await else {
            return false;
        };
        let options = ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..ExecOptions::default() };
        machine.exec("gh auth token --hostname github.com > /dev/null 2>&1", options).await.map(|read| read.exit_code == JsInt::from(0)).unwrap_or(false)
    }

    /// sub-spawn.ts:104 (rule 8).
    pub(crate) async fn image_at(&self, place: &str, on: &str, host: &GitHost, remote: &str, brokered: bool) -> AreaResult<ImageAt> {
        let manifest: Option<GoldenManifest> = self.ctx.copy_of(place, SETUP_IMAGE).await?;
        let Some(head) = golden_head(manifest.as_ref()).cloned() else {
            return Err(AreaError::Host(sub_no_image_refusal(on)));
        };
        let record = self.ctx.record_of(SETUP_IMAGE).await?;
        let label = host_words(host);
        let owner_repo = owner_repo_of(remote).unwrap_or_default();
        if brokered {
            if !matches!(host.id, GitHostId::Github) {
                return Err(AreaError::Host(sub_host_other_git_refusal(on, label, &owner_repo)));
            }
            if !self.host_signed_in().await {
                return Err(AreaError::Host(sub_host_no_sign_in_refusal(on, &owner_repo)));
            }
            return Ok(ImageAt { head, record });
        }
        let logins: Vec<(String, String)> = match (&head.logins, record.as_ref().and_then(|r| r.logins.clone())) {
            (Some(logins), _) => logins.iter().map(|l| (l.name.clone(), login_state_word(&l.state))).collect(),
            (None, Some(logins)) => logins,
            (None, None) => Vec::new(),
        };
        let held = logins.iter().any(|(login, state)| (*login == host.cli || login.starts_with(&format!("{label} CLI"))) && LOGIN_HELD.contains(&state.as_str()));
        if !held {
            let words = GitHostWords { label: label.to_string(), cli: host.cli.to_string(), login: format!("{label} CLI login") };
            return Err(AreaError::Host(sub_no_sign_in_refusal(on, &words, &owner_repo)));
        }
        Ok(ImageAt { head, record })
    }

    /// sub-spawn.ts:123 (rule 9).
    pub(crate) fn agent_on(&self, record: Option<&ImageRecord>, on: &str, harness: &str, brokered: bool) -> Result<(), HostError> {
        let refused = if brokered { self.ctx.unbrokered_refusal(harness) } else { None };
        if let Some(refused) = refused {
            return Err(refused);
        }
        let carried: Vec<String> = record.map(|r| r.agents.iter().filter(|(_, enabled)| *enabled).map(|(id, _)| id.clone()).collect()).unwrap_or_default();
        if !carried.is_empty() && !carried.iter().any(|id| id == harness) {
            return Err(sub_no_agent_refusal(on, harness, &carried));
        }
        Ok(())
    }

    /// sub-spawn.ts:131 (rule 10).
    pub(crate) fn starts_left(&self, limits: Option<&BoxLimits>, name: &str) -> Result<(), HostError> {
        let Some(limits) = limits else {
            return Ok(());
        };
        let Some(starts) = limits.starts.as_ref() else {
            return Ok(());
        };
        let windows = [(StartWindow::Day, &starts.day), (StartWindow::Hour, &starts.hour)];
        match windows.into_iter().find(|(_, count)| count.used >= count.limit) {
            Some((window, count)) => Err(HostError::new(starts_used_line(
                window,
                count.used,
                count.limit,
                boat_plan_words(limits.trial, limits.plan_usd).as_deref(),
                Some(&format!("sub-thread {name}")),
            ))),
            None => Ok(()),
        }
    }

    /// sub-spawn.ts:347 (rule 11).
    pub(crate) async fn free_branch(&self, wanted: &str, lead: &Entry) -> String {
        let heads = self.git_at_lead(lead, &["ls-remote", "--heads", "origin"]).await.ok();
        let listing = heads.filter(|read| read.exit_code == JsInt::from(0)).map(|read| read.stdout);
        self.ctx.free_branch_name(wanted, listing.as_deref())
    }

    /// sub-spawn.ts:354 (rule 12).
    pub(crate) async fn git_at_lead(&self, lead: &Entry, args: &[&str]) -> AreaResult<ExecResult> {
        let (project_id, folder) = {
            let record = lead.borrow().record.borrow().clone();
            (record.project.clone(), self.ctx.checkout_of(&record))
        };
        let computer = self.ctx.project_held(&project_id).computer;
        let machine = if copies_folder(&kind_for_computer(&computer, None)) {
            self.ctx.local_machine().await?
        } else {
            lead.borrow().machine.clone()
        };
        let mut words = vec!["git", "-C", folder.as_str()];
        words.extend_from_slice(args);
        let command = format!("GIT_TERMINAL_PROMPT=0 {}", shell_line(&words));
        Ok(machine.exec(&command, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..ExecOptions::default() }).await?)
    }

    /// sub-spawn.ts:360 (rule 13).
    pub(crate) async fn remote_read(&self, lead: &Entry, from: &str, args: &[&str]) -> Result<String, HostError> {
        let mut words = vec!["ls-remote"];
        words.extend_from_slice(args);
        let read = match self.git_at_lead(lead, &words).await {
            Ok(read) => read,
            Err(e) => ExecResult { exit_code: JsInt::from(-1), stdout: String::new(), stderr: e.message() },
        };
        if read.exit_code != JsInt::from(0) {
            let said = last_line(&read.stderr).unwrap_or_else(|| format!("git ls-remote exited {}", read.exit_code));
            return Err(sub_from_unread_refusal(from, &said));
        }
        Ok(read.stdout)
    }

    /// sub-spawn.ts:369 (rule 14). Answers `(branch, sha)`.
    pub(crate) async fn from_ref(&self, from: &str, lead: &Entry, preferred: Option<&str>) -> Result<(String, String), HostError> {
        let name = from.strip_prefix("origin/").unwrap_or(from);
        let listed = self.remote_read(lead, from, &["origin", name, &format!("{name}^{{}}")]).await?;
        let mut held: IndexMap<String, String> = IndexMap::new();
        for line in listed.split('\n') {
            let mut parts = line.split('\t');
            if let (Some(sha), Some(reference)) = (parts.next(), parts.next()) {
                held.insert(reference.to_string(), sha.to_string());
            }
        }
        let found = [name.to_string(), format!("refs/heads/{name}"), format!("refs/tags/{name}^{{}}"), format!("refs/tags/{name}")]
            .into_iter()
            .find(|reference| held.contains_key(reference));
        if let Some(reference) = found {
            return Ok((name.to_string(), held.get(&reference).cloned().unwrap_or_default()));
        }
        if !is_commit_word(name) {
            return Err(sub_from_not_on_remote_refusal(from, Some(name)));
        }
        let commit = self.git_at_lead(lead, &["rev-parse", "--verify", "--quiet", &format!("{name}^{{commit}}")]).await.map_err(AreaError::into_host)?;
        let sha = commit.stdout.trim().to_string();
        let holding = if commit.exit_code == JsInt::from(0) {
            Some(self.git_at_lead(lead, &["for-each-ref", "--contains", &sha, "--format=%(refname:lstrip=3)", "refs/remotes/origin"]).await.map_err(AreaError::into_host)?)
        } else {
            None
        };
        let tracked: Vec<String> = match holding {
            Some(read) if read.exit_code == JsInt::from(0) => read.stdout.split('\n').filter(|b| !b.is_empty() && *b != "HEAD").map(str::to_string).collect(),
            _ => Vec::new(),
        };
        if tracked.is_empty() {
            return Err(sub_from_not_on_remote_refusal(from, None));
        }
        let live: IndexSet<String> = self
            .remote_read(lead, from, &["--heads", "origin"])
            .await?
            .split('\n')
            .filter_map(|line| line.split('\t').nth(1).map(|reference| reference.get(11..).unwrap_or_default().to_string()))
            .collect();
        let branches: Vec<String> = tracked.into_iter().filter(|b| live.contains(b)).collect();
        let Some(first) = branches.first().cloned() else {
            return Err(sub_from_not_on_remote_refusal(from, None));
        };
        let branch = match preferred {
            Some(preferred) if branches.iter().any(|b| b == preferred) => preferred.to_string(),
            _ => first,
        };
        Ok((branch, sha))
    }

    /// sub-spawn.ts:392 (rule 15).
    pub(crate) async fn seed_of(&self, project: &ProjectView) -> AreaResult<Seed> {
        let none = Seed::default();
        if !copies_folder(&kind_for_computer(&project.computer, None)) {
            return Ok(Seed { why: Some(sub_picked_elsewhere_why(&project.name, &self.ctx.place_name(&project.computer))), ..none });
        }
        let folder = self.ctx.folder_named(&project.path);
        let choice = self.ctx.get_seed_choice(&folder).await?;
        let Some(choice) = choice.filter(|_| self.ctx.seeds()) else {
            return Ok(Seed { why: Some(sub_nothing_picked_why(&project.name)), ..none });
        };
        let homes = self.ctx.homes_here().await?;
        let plan = self.ctx.seed_plan(&folder, &homes).await?;
        let travelling: IndexMap<String, u64> = plan.files.iter().filter(|f| wsp_words::project_seed::seed_travels(&plan, f)).map(|f| (f.path.clone(), f.bytes.0 .0 as u64)).collect();
        let paths: Vec<String> = choice.files.iter().filter(|p| travelling.contains_key(*p)).cloned().collect();
        let missing: Vec<String> = choice.files.iter().filter(|p| !plan.files.iter().any(|f| f.path == **p)).cloned().collect();
        let stayed: Vec<String> = choice.files.iter().filter(|p| !travelling.contains_key(*p) && !missing.iter().any(|m| m == *p)).cloned().collect();
        if paths.is_empty() {
            return Ok(Seed { missing, stayed, ..none });
        }
        let packed = self.ctx.seed_pack(&plan, &paths, &homes).await?;
        if packed.tar.len() as u64 > SUB_SEED_MAX_BYTES {
            let mut largest: Vec<(String, u64)> = paths.iter().filter_map(|p| travelling.get(p).map(|bytes| (p.clone(), *bytes))).collect();
            largest.sort_by(|a, b| b.1.cmp(&a.1));
            let largest: Vec<String> = largest.iter().take(2).map(|(path, bytes)| format!("{path} ({})", fmt_bytes(*bytes as f64))).collect();
            return Err(AreaError::Host(sub_seed_too_big_refusal(
                &project.name,
                &fmt_bytes(packed.tar.len() as f64),
                &fmt_bytes(SUB_SEED_MAX_BYTES as f64),
                &largest,
            )));
        }
        let mut stayed = stayed;
        stayed.extend(packed.left.iter().cloned());
        Ok(Seed { tar: Some(packed.tar), files: packed.files, paths, bytes: packed.bytes, missing, stayed, why: None })
    }
}
