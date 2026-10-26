// SPDX-License-Identifier: AGPL-3.0-only
//! kinds.ts:26-316, 506-563: the roads every kind shares. Spec: spec/kinds_roads.md.
#![allow(clippy::result_large_err)]
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use futures::future::{select, Either, LocalBoxFuture};
use indexmap::IndexMap;
use serde_json::{json, Value};

use wsp_adapters::port::ExecStreamFactory;
use wsp_catalog::catalog::{catalog_agents, guest_env, shared_on};
use wsp_catalog::signin::{login_home_in, shared_login_of};
use wsp_core::adapters::secret_env_of;
use wsp_core::daemon_roots::write_daemon_roots_script;
use wsp_core::project_landing::under_login_home;
use wsp_core::project_sources::{project_remote, project_source, ProjectSourceModule};
use wsp_core::types::harness::{login_env_on, place_login_env, Brokered, BrokeredKind};
use wsp_core::types::internal::{ImportLanded, ImportReport};
use wsp_core::types::pending::{MachineExecOptions, TurnWaiting};
use wsp_core::types::wiring::{
    folder_named, last_line_of, LiveWorkspace, ProjectImportOptions, StageReport, WorkspaceRecord,
    CLONE_MS,
};
use wsp_engine::exec_detached::put::INLINE_EXEC_MS;
use wsp_engine::machine::{ExecOptions, ExecResult, Lifecycle, MachineBackend};
use wsp_engine::project_install::{install_script, project_installs, INSTALL_MS};
use wsp_engine::project_state::{guest_agent_homes, project_state_key};
use wsp_engine::run_env::GITHUB_TOKEN_ENV;
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::projects::REGISTERING_LINE;
use wsp_wire::types::shell_quote::{shell_line, shell_quote};
use wsp_wire::views::project_bundle::{ProjectImportResult, ProjectImportStage};
use wsp_wire::views::workspace::{
    DaemonReachView, ProjectSource, ProjectView, WorkspacePhase, WorkspaceProject,
};
use wsp_wire::views::workspace_events::WorkspaceCreateStage;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::wire::capabilities::PauseMode;
use wsp_wire::wire::runtime::BrokerStatusCodex;
use wsp_words::exit::{HostError, Kind};
use wsp_words::js::js_trim;
use wsp_words::js::utf16::tail;
use wsp_words::js_date::iso_string;
use wsp_words::project_path::folder_name;
use wsp_words::projects::{
    clone_failed_line, clone_into_file_line, clone_into_taken_line, clone_on_machine_failed_line,
    clone_url_refusal, home_shortened, kind_for_computer, project_name_of, registered_line,
    CloneFailed,
};
use wsp_words::start::owner_repo_of;
use wsp_words::words::computer::no_kind_line;

use super::{kind_word, BrokerSaid, BrokeredLaunch, KindsArea};
use crate::machines::area_error::{AreaError, AreaResult};
use crate::machines::brokered::{ClaudePlaceholderKind, Placeholders};
use crate::places::helpers::place_home_refusal;
use crate::places::types::FolderComputer;

/// kinds.ts:27. How long a launch waits on the broker's answer.
pub const BROKER_READ_MS: f64 = 5_000.0;

/// ENOTDIR on Linux and macOS.
const ENOTDIR: i32 = 20;

/// What a folder on this computer is (kinds.ts `standing`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Standing {
    Missing,
    Empty,
    Full,
    File,
}

fn standing(path: &str) -> Result<Standing, HostError> {
    match std::fs::read_dir(path) {
        Ok(mut entries) => Ok(if entries.next().is_none() {
            Standing::Empty
        } else {
            Standing::Full
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Standing::Missing),
        Err(e) if e.raw_os_error() == Some(ENOTDIR) => Ok(Standing::File),
        Err(e) => Err(HostError::new(e.to_string())),
    }
}

fn invalid(message: String) -> AreaError {
    AreaError::Host(HostError::with_kind(message, Kind::Invalid))
}

/// node's `os.homedir()` (kinds.ts:124): `$HOME` where it is set, empty included, else the effective user's passwd home.
fn here_home() -> String {
    if let Some(home) = std::env::var_os("HOME") {
        return home.to_string_lossy().into_owned();
    }
    let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
    let mut found: *mut libc::passwd = std::ptr::null_mut();
    let mut buf = vec![0u8; 4096];
    loop {
        // SAFETY: `entry` and `buf` outlive the call, which writes inside `buf` (its length passed) and points `found`
        // at `entry` or null.
        let rc = unsafe {
            libc::getpwuid_r(
                libc::geteuid(),
                &mut entry,
                buf.as_mut_ptr().cast(),
                buf.len(),
                &mut found,
            )
        };
        if rc == libc::ERANGE && buf.len() < 1 << 20 {
            buf.resize(buf.len() * 2, 0);
            continue;
        }
        if rc != 0 || found.is_null() || entry.pw_dir.is_null() {
            return String::new();
        }
        // SAFETY: `pw_dir` points to a NUL-terminated string inside `buf`, which is alive here.
        return unsafe { std::ffi::CStr::from_ptr(entry.pw_dir) }
            .to_string_lossy()
            .into_owned();
    }
}

fn here_cwd() -> String {
    std::env::current_dir()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn join(parent: &str, name: &str) -> String {
    Path::new(parent).join(name).to_string_lossy().into_owned()
}

fn parent_of(path: &str) -> String {
    Path::new(path).parent().map_or_else(
        || path.to_string(),
        |parent| parent.to_string_lossy().into_owned(),
    )
}

fn base_of(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A path as node's `path.resolve` answers it, against this process's folder.
fn resolved_path(path: &str) -> PathBuf {
    let joined = if path.starts_with('/') {
        PathBuf::from(path)
    } else {
        Path::new(&here_cwd()).join(path)
    };
    joined.components().fold(PathBuf::new(), |mut out, part| {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
        out
    })
}

fn source_kind(source: &ProjectSource) -> &'static str {
    match source {
        ProjectSource::Folder(_) => "folder",
        ProjectSource::Git(_) => "git",
        ProjectSource::Github(_) => "github",
        ProjectSource::Gitlab(_) => "gitlab",
    }
}

fn source_module(source: &ProjectSource) -> Result<Rc<dyn ProjectSourceModule>, HostError> {
    project_source(source_kind(source))
}

/// The object `cloneLines` takes for a project's clone (project-landing.ts `cloneLines`), the module named by its kind.
fn clone_object(project: &ProjectView, computer: &str, branch: Option<&str>) -> Value {
    let mut o = json!({ "source": source_kind(&project.source), "remote": project.remote, "checkout": project.path, "computer": computer });
    if let Some(branch) = branch {
        o["branch"] = json!(branch);
    }
    o
}

fn brokered_kind(kind: ClaudePlaceholderKind) -> BrokeredKind {
    match kind {
        ClaudePlaceholderKind::Token => BrokeredKind::Token,
        ClaudePlaceholderKind::Key => BrokeredKind::Key,
        ClaudePlaceholderKind::Gateway => BrokeredKind::Gateway,
    }
}

fn over_exec_options(base: MachineExecOptions, over: MachineExecOptions) -> MachineExecOptions {
    MachineExecOptions {
        poll_ms: over.poll_ms.or(base.poll_ms),
        idle_ms: over.idle_ms.or(base.idle_ms),
        deadline_ms: over.deadline_ms.or(base.deadline_ms),
        exec_timeout_ms: over.exec_timeout_ms.or(base.exec_timeout_ms),
        run_dir: over.run_dir.or(base.run_dir),
        now: over.now.or(base.now),
        sleep: over.sleep.or(base.sleep),
        reading: over.reading.or(base.reading),
        launch_on: over.launch_on.or(base.launch_on),
        thread: over.thread.or(base.thread),
        asks_until_stopped: over.asks_until_stopped || base.asks_until_stopped,
        killed_why: over.killed_why.or(base.killed_why),
        ids: over.ids.or(base.ids),
    }
}

impl KindsArea {
    fn said(&self) -> BrokerSaid {
        self.broker_said.borrow().clone()
    }

    /// kinds.ts:32 (rule 1).
    pub fn project_of(&self, dest: &str, size: f64) -> WorkspaceProject {
        WorkspaceProject {
            name: folder_name(dest),
            dest: dest.to_string(),
            imported_at: iso_string(self.ctx.now()).unwrap_or_default(),
            size: Some(JsInt(JsNumber(size))),
        }
    }
    /// kinds.ts:36 (rule 2).
    pub(crate) fn clone_on_machine(&self, project: &ProjectView, computer: &str) -> String {
        self.ctx
            .clone_lines(clone_object(project, computer, project.base.as_deref()))
            .join("\n")
    }
    /// kinds.ts:46 (rule 3). The answer owns what it read of the entry, so a kind module can hand it back as `'static`.
    pub(crate) fn clone_project(
        &self,
        entry: &LiveWorkspace,
        project: ProjectView,
        report: StageReport,
    ) -> LocalBoxFuture<'static, AreaResult<()>> {
        let ctx = Rc::clone(&self.ctx);
        let record = entry.record.borrow().clone();
        if project.checkout.is_some() {
            return match record.branch_of_record {
                Some(_) => ctx.land_on_record(entry, project, None),
                None => Box::pin(async { Ok(()) }),
            };
        }
        report(
            WorkspaceCreateStage::ProjectCloned,
            &format!("Cloning {} into {}.", project.remote, project.path),
            None,
        );
        let computer = ctx.place_name(&record.place.clone().unwrap_or_else(|| ctx.wired_place()));
        let (landing, clone_cmd) = match record.branch_of_record.as_ref() {
            Some(of_record) => {
                let base = record.base.as_deref().or(project.base.as_deref());
                let mut o = clone_object(&project, &computer, base);
                o["brokered"] = json!({
                    "ofRecord": serde_json::to_value(of_record).unwrap_or_default(),
                    "origin": owner_repo_of(&project.remote).unwrap_or_default(),
                    "held": entry.of_record_held,
                });
                (
                    Some(ctx.land_on_record(entry, project.clone(), Some(ctx.clone_lines(o)))),
                    None,
                )
            }
            None => (None, Some(self.clone_on_machine(&project, &computer))),
        };
        let machine = entry.machine.clone();
        let scratch = self.module_of(record.kind).map(|kind| kind.scratch(entry));
        let project_id = project.id.clone();
        let path = project.path.clone();
        let remote = project.remote.clone();
        Box::pin(async move {
            if let Some(landed) = landing {
                landed.await?;
            }
            if let Some(command) = clone_cmd {
                let cloned = machine
                    .exec(
                        &command,
                        ExecOptions {
                            timeout_ms: Some(CLONE_MS),
                            ..ExecOptions::default()
                        },
                    )
                    .await?;
                if cloned.exit_code != JsInt::from(0) {
                    let said = if cloned.stderr.is_empty() {
                        cloned.stdout.clone()
                    } else {
                        cloned.stderr.clone()
                    };
                    let failed = CloneFailed {
                        repo: owner_repo_of(&remote).unwrap_or_else(|| remote.clone()),
                        on: computer.clone(),
                        exit_code: cloned.exit_code.0 .0 as i32,
                        said,
                        limit_ms: CLONE_MS as f64,
                    };
                    return Err(HostError::new(clone_on_machine_failed_line(&failed)).into());
                }
            }
            let root = machine
                .exec(
                    &format!("ls -A {}", shell_quote(&path)),
                    ExecOptions {
                        timeout_ms: Some(INLINE_EXEC_MS),
                        ..ExecOptions::default()
                    },
                )
                .await?;
            let names: Vec<String> = root
                .stdout
                .split('\n')
                .map(|name| js_trim(name).to_string())
                .collect();
            let Some(install) = project_installs(&names, &path).into_iter().next() else {
                return Ok(());
            };
            report(
                WorkspaceCreateStage::ProjectCloned,
                &format!("{} in {}.", install.command, path),
                None,
            );
            let log = format!("{}/install-{}.log", scratch?, project_id);
            let ran = machine
                .exec(
                    &install_script(&install, &path, &log),
                    ExecOptions {
                        timeout_ms: Some(INSTALL_MS),
                        ..ExecOptions::default()
                    },
                )
                .await?;
            if ran.exit_code != JsInt::from(0) {
                let said = [last_line_of(&ran.stderr), last_line_of(&ran.stdout)]
                    .into_iter()
                    .find(|line| !line.is_empty())
                    .unwrap_or_else(|| format!("exit {}", ran.exit_code));
                return Err(HostError::new(format!(
                    "{} in {}: {}; its whole output is {} on the machine",
                    install.command, path, said, log
                ))
                .into());
            }
            Ok(())
        })
    }
    /// kinds.ts:80 (rule 4). `(remote, defaultBranch)`.
    pub async fn remote_here(&self, path: &str) -> AreaResult<(String, String)> {
        let machine = self.ctx.local_machine().await?;
        let command = format!(
            "{} || echo; {} || echo",
            shell_line(&["git", "-C", path, "remote", "get-url", "origin"]),
            shell_line(&[
                "git",
                "-C",
                path,
                "symbolic-ref",
                "--short",
                "refs/remotes/origin/HEAD"
            ])
        );
        let read = machine
            .exec(
                &command,
                ExecOptions {
                    timeout_ms: Some(INLINE_EXEC_MS),
                    ..ExecOptions::default()
                },
            )
            .await?;
        let mut lines = read.stdout.split('\n').map(js_trim);
        let remote = lines.next().unwrap_or_default().to_string();
        let head = lines.next().unwrap_or_default();
        Ok((
            remote,
            head.strip_prefix("origin/").unwrap_or(head).to_string(),
        ))
    }
    /// kinds.ts:93 (rule 5).
    pub async fn git_here(
        &self,
        cwd: &str,
        args: &[&str],
        timeout_ms: Option<u64>,
        stdin: Option<&str>,
    ) -> AreaResult<ExecResult> {
        let machine = self.ctx.local_machine().await?;
        let mut words = vec!["git", "-C", cwd];
        words.extend_from_slice(args);
        let options = ExecOptions {
            timeout_ms: Some(timeout_ms.unwrap_or(INLINE_EXEC_MS)),
            stdin: stdin.map(|text| text.as_bytes().to_vec()),
            ..ExecOptions::default()
        };
        Ok(machine.exec(&shell_line(&words), options).await?)
    }
    /// kinds.ts:99 (rule 5).
    pub async fn git_top_of(&self, path: &str) -> AreaResult<Option<String>> {
        let read = self
            .git_here(path, &["rev-parse", "--show-toplevel"], None, None)
            .await?;
        let top = js_trim(&read.stdout);
        Ok((read.exit_code == JsInt::from(0) && !top.is_empty()).then(|| top.to_string()))
    }
    /// kinds.ts:105 (rule 5).
    pub async fn branch_at(&self, path: &str) -> AreaResult<Option<String>> {
        let read = self
            .git_here(
                path,
                &["symbolic-ref", "--quiet", "--short", "HEAD"],
                None,
                None,
            )
            .await?;
        let branch = js_trim(&read.stdout);
        Ok((read.exit_code == JsInt::from(0) && !branch.is_empty()).then(|| branch.to_string()))
    }
    /// kinds.ts:112 (rule 6).
    pub fn state_folder(&self) -> Result<String, HostError> {
        let Some(state) = self.ctx.state_path() else {
            return Err(HostError::with_kind(
                "this runtime was given no state file, so it keeps no worktrees or copies",
                Kind::Invalid,
            ));
        };
        let resolved = resolved_path(&state);
        Ok(resolved
            .parent()
            .unwrap_or(resolved.as_path())
            .to_string_lossy()
            .into_owned())
    }
    /// kinds.ts:124 (rule 7). Answers the folder as it resolved.
    pub async fn clone_here(
        &self,
        word: &str,
        source: &ProjectSource,
        into: &str,
    ) -> Result<String, HostError> {
        self.clone_here_on(word, source, into)
            .await
            .map_err(AreaError::into_host)
    }
    async fn clone_here_on(
        &self,
        word: &str,
        source: &ProjectSource,
        into: &str,
    ) -> AreaResult<String> {
        if let Some(refused) = clone_url_refusal(word) {
            return Err(invalid(refused));
        }
        let home = here_home();
        let picked = folder_named(into, &home, &here_cwd());
        let name = project_name_of(source);
        let picked_base = base_of(&picked);
        let numbered = picked_base
            .strip_prefix(&format!("{name}-"))
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()));
        let own = picked_base == name || numbered;
        let dest = if own {
            picked.clone()
        } else {
            join(&picked, &name)
        };
        if !own && standing(&picked)? == Standing::File {
            return Err(invalid(clone_into_file_line(
                &home_shortened(&picked, Some(&home)),
                None,
            )));
        }
        let at_dest = standing(&dest)?;
        if matches!(at_dest, Standing::Full | Standing::File) {
            let beside = parent_of(&dest);
            let mut n = 2;
            while Path::new(&join(&beside, &format!("{name}-{n}"))).exists() {
                n += 1;
            }
            let shown = home_shortened(&dest, Some(&home));
            let free = home_shortened(&join(&beside, &format!("{name}-{n}")), Some(&home));
            let line = if at_dest == Standing::File {
                clone_into_file_line(&shown, Some(&free))
            } else {
                clone_into_taken_line(&shown, &free)
            };
            return Err(invalid(line));
        }
        let mut made: Vec<PathBuf> = Vec::new();
        let mut up = Path::new(&dest).parent();
        while let Some(folder) = up {
            if folder.exists() || folder.parent().is_none() {
                break;
            }
            made.push(folder.to_path_buf());
            up = folder.parent();
        }
        let machine = self.ctx.local_machine().await?;
        let line = format!(
            "{} {}",
            shell_line(&["env", "GIT_TERMINAL_PROMPT=0"]),
            source_module(source)?.clone_command(&project_remote(source), &dest, None)
        );
        let cloned = machine
            .exec(
                &line,
                ExecOptions {
                    timeout_ms: Some(CLONE_MS),
                    ..ExecOptions::default()
                },
            )
            .await?;
        if cloned.exit_code != JsInt::from(0) {
            for folder in &made {
                if std::fs::remove_dir(folder).is_err() {
                    break;
                }
            }
            let said = if cloned.stderr.is_empty() {
                cloned.stdout
            } else {
                cloned.stderr
            };
            return Err(invalid(clone_failed_line(&said)));
        }
        Ok(folder_named(&dest, &home, &here_cwd()))
    }
    /// kinds.ts:171 (rule 8).
    pub(crate) async fn register_import(
        &self,
        o: &ProjectImportOptions,
        report: &ImportReport,
    ) -> AreaResult<ImportLanded> {
        report(ProjectImportStage::Landing, REGISTERING_LINE, None);
        let plan = o.bundler.plan().await?;
        let project = self.project_of(&o.dest, plan.bytes.0);
        Ok(ImportLanded {
            result: ProjectImportResult {
                dest: o.dest.clone(),
                files: plan.files,
                bytes: plan.bytes,
                parts: JsNumber(0.0),
                cut: Vec::new(),
                rewritten: Vec::new(),
                agents: Vec::new(),
                project,
            },
            done: registered_line(&o.dest),
        })
    }
    /// kinds.ts:181 (rule 9).
    pub(crate) fn write_roots(
        &self,
        entry: &LiveWorkspace,
        dests: &[String],
        roots_path: Option<&str>,
    ) -> LocalBoxFuture<'static, AreaResult<()>> {
        let script = write_daemon_roots_script(dests, roots_path);
        let machine = entry.machine.clone();
        let last = dests
            .last()
            .cloned()
            .unwrap_or_else(|| "undefined".to_string());
        Box::pin(async move {
            let browsable = machine
                .exec(
                    &script,
                    ExecOptions {
                        timeout_ms: Some(INLINE_EXEC_MS),
                        ..ExecOptions::default()
                    },
                )
                .await?;
            if browsable.exit_code != JsInt::from(0) {
                return Err(HostError::new(format!(
                    "could not make {last} browsable on the machine: {}",
                    tail(&browsable.stderr, 200)
                ))
                .into());
            }
            Ok(())
        })
    }
    /// kinds.ts:185 (rule 10).
    pub fn cloud_home(&self, id: &str) -> Result<String, HostError> {
        guest_agent_homes()
            .get(id)
            .cloned()
            .ok_or_else(|| HostError::new(format!("the catalog has no home for {id}")))
    }
    /// kinds.ts:192 (rule 10).
    pub fn cloud_env(
        &self,
        record: &WorkspaceRecord,
        id: Option<&str>,
    ) -> IndexMap<String, String> {
        let mut env = login_env_on(record.place.as_deref(), record.npm_bin.as_deref());
        if let Some(agent) =
            id.and_then(|id| catalog_agents().into_iter().find(|agent| agent.id == id))
        {
            env.extend(guest_env(agent));
        }
        env
    }
    /// kinds.ts:197 (rule 11).
    pub(crate) fn cloud_road(
        &self,
        entry: &LiveWorkspace,
    ) -> LocalBoxFuture<'static, AreaResult<DaemonReachView>> {
        let ctx = Rc::clone(&self.ctx);
        let ws = entry.ws.clone();
        let machine = entry.machine.clone();
        Box::pin(async move {
            let reach = ws.daemon_reach().await?;
            let daemon_token = ctx.daemon_token_of(&machine).await?;
            Ok(DaemonReachView {
                url: reach.url,
                expires_at: reach.expires_at,
                daemon_token,
            })
        })
    }
    /// kinds.ts:202 (rule 11).
    pub async fn local_road(&self) -> AreaResult<DaemonReachView> {
        let wired = || {
            AreaError::Host(HostError::new(
                "this host wired no daemon for this computer, so nothing on it can be dialled",
            ))
        };
        if !self.ctx.has_local_daemon_road() {
            return Err(wired());
        }
        match self.ctx.local_daemon_road().await {
            Some(road) => road,
            None => Err(wired()),
        }
    }
    /// kinds.ts:209 (rule 12).
    pub(crate) fn cloud_deploy(
        &self,
        entry: &LiveWorkspace,
    ) -> LocalBoxFuture<'static, AreaResult<()>> {
        let ctx = Rc::clone(&self.ctx);
        let machine = entry.machine.clone();
        Box::pin(async move {
            let token = ctx.daemon_token_of(&machine).await?;
            ctx.deploy_daemon(&machine, token).await?;
            ctx.forget_daemon_token(machine.id());
            ctx.daemon_token_of(&machine).await?;
            Ok(())
        })
    }
    /// kinds.ts:217 (rule 13).
    pub(crate) fn shared_login_under(&self, home: &str, agent: &str) -> bool {
        shared_on(agent).is_some_and(|shared| Path::new(home).join(&shared.file).exists())
    }
    /// kinds.ts:227 (rule 13).
    pub fn place_of(&self, record: &WorkspaceRecord) -> Result<FolderComputer, HostError> {
        if !self.ctx.has_place_door() {
            return Err(HostError::new(no_kind_line("place")));
        }
        let at = record
            .place
            .as_deref()
            .and_then(|place| self.ctx.door_folder_computer(place));
        match (at, record.place.as_deref()) {
            (Some(at), _) => Ok(at),
            (None, Some(place)) if self.ctx.door_joined(place) => {
                Err(HostError::new(place_home_refusal(None)))
            }
            (None, _) => Err(HostError::new(format!(
                "{} stands on {}, which this host no longer holds",
                record.name,
                record.place.as_deref().unwrap_or("no computer")
            ))),
        }
    }
    /// kinds.ts:235 (rule 14).
    pub fn place_agent_home(
        &self,
        place: Option<&str>,
        home: &str,
        id: &str,
    ) -> Result<String, HostError> {
        let shared = catalog_agents()
            .into_iter()
            .find(|agent| agent.id == id)
            .and_then(|agent| shared_login_of(&agent.sign_in));
        let logins = place.and_then(|place| self.ctx.door_logins_of(place));
        match (shared, logins) {
            (Some(shared), Some(logins)) => Ok(login_home_in(&logins, shared)),
            _ => Ok(under_login_home(home, &self.cloud_home(id)?)),
        }
    }
    /// kinds.ts:242 (rule 15).
    pub fn vault_on(&self, entry: &LiveWorkspace) -> IndexMap<String, String> {
        let vault = self.ctx.vault();
        let place = entry.record.borrow().place.clone();
        match place {
            None => vault,
            Some(place) => self.ctx.door_vault_at(&place, &vault).unwrap_or(vault),
        }
    }
    /// kinds.ts:248 (rule 16).
    pub fn brokered(&self, record: &WorkspaceRecord) -> bool {
        self.ctx.broker_ready() && self.ctx.is_brokered(record)
    }
    /// kinds.ts:256 (rule 17).
    pub async fn read_broker(&self) {
        if !self.ctx.has_broker_status() {
            return;
        }
        let late = self.ctx.sleep(BROKER_READ_MS);
        let said = match select(Box::pin(self.read_said()), late).await {
            Either::Left((said, _)) => said,
            Either::Right(_) => BrokerSaid::default(),
        };
        *self.broker_said.borrow_mut() = said;
    }
    async fn read_said(&self) -> BrokerSaid {
        let status = self.ctx.broker_status().await.ok();
        let chatgpt = matches!(
            status.as_ref().and_then(|s| s.codex.as_ref()),
            Some(BrokerStatusCodex::Chatgpt | BrokerStatusCodex::AccessToken)
        );
        let claims = if chatgpt {
            self.ctx.broker_codex_claims().await.ok().flatten()
        } else {
            None
        };
        BrokerSaid { status, claims }
    }
    /// kinds.ts:278 (rule 18).
    pub fn brokered_launch(
        &self,
        entry: &LiveWorkspace,
        harness: &str,
        token: Option<&str>,
    ) -> Result<Option<BrokeredLaunch>, HostError> {
        let record = entry.record.borrow().clone();
        if !self.brokered(&record) {
            return Ok(None);
        }
        let said = self.said();
        let codex = if harness == "codex" {
            self.ctx.codex_broker_of(&said)
        } else {
            None
        };
        let Some(token) = token else {
            return Ok(Some(BrokeredLaunch {
                env: IndexMap::new(),
                vault: IndexMap::new(),
                claude: None,
                codex,
            }));
        };
        let refused = self.ctx.unbrokered_refusal(harness).or_else(|| {
            (harness == "codex" && codex.is_none()).then(|| self.ctx.codex_login_unread_refusal())
        });
        if let Some(refused) = refused {
            return Err(refused);
        }
        let Placeholders {
            claude_token,
            claude_kind,
            vault,
        } = self.ctx.placeholders_of(&said, token);
        let claude = Brokered {
            token: claude_token,
            kind: brokered_kind(claude_kind),
        };
        Ok(Some(BrokeredLaunch {
            env: self.ctx.brokered_env(token, &record),
            vault,
            claude: Some(claude),
            codex,
        }))
    }
    /// kinds.ts:291 (rule 19).
    pub async fn know_broker(&self, entry: &LiveWorkspace, harness: &str) -> Result<(), HostError> {
        let record = entry.record.borrow().clone();
        if harness != "codex" || !self.brokered(&record) {
            return Ok(());
        }
        if self.ctx.codex_broker_of(&self.said()).is_none() {
            self.read_broker().await;
        }
        if self.ctx.codex_broker_of(&self.said()).is_none() {
            return Err(self.ctx.codex_login_unread_refusal());
        }
        Ok(())
    }
    /// kinds.ts:297.
    pub(crate) fn place_login_stands(&self, entry: &LiveWorkspace, id: &str) -> bool {
        let place = entry.record.borrow().place.clone();
        place.is_some_and(|place| {
            self.ctx
                .door_sign_ins_at(&place)
                .and_then(|signs| signs.get(id).copied())
                == Some(AgentSignInState::SignedIn)
        })
    }
    /// kinds.ts:304 (rule 20).
    pub fn place_env(
        &self,
        entry: &LiveWorkspace,
        home_of: &dyn Fn(&str) -> String,
    ) -> Result<IndexMap<String, String>, HostError> {
        let record = entry.record.borrow().clone();
        let at = self.place_of(&record)?;
        let mut env = place_login_env();
        env.shift_remove("USER");
        env.shift_remove("PATH");
        env.insert("HOME".to_string(), at.home);
        let vault = self.vault_on(entry);
        for agent in catalog_agents() {
            let variable = agent
                .state_home_env
                .clone()
                .or_else(|| shared_login_of(&agent.sign_in).map(|shared| shared.home_env.clone()));
            if let Some(variable) = variable {
                env.insert(variable, home_of(&agent.id));
            }
            env.extend(secret_env_of(
                &vault,
                &agent.id,
                self.place_login_stands(entry, &agent.id),
            ));
        }
        if let Some(token) = vault.get(GITHUB_TOKEN_ENV) {
            if record
                .place
                .as_deref()
                .is_some_and(|place| self.ctx.door_github_from_vault(place))
            {
                env.insert(GITHUB_TOKEN_ENV.to_string(), token.clone());
            }
        }
        Ok(env)
    }
    /// kinds.ts:508 (rule 21).
    pub fn kind_of(&self, computer: &str) -> WorkspaceKind {
        kind_for_computer(computer, Some(self.ctx.door_joined(computer)))
    }
    /// kinds.ts:514 (rule 22).
    pub fn backend_for(
        &self,
        record: &WorkspaceRecord,
    ) -> Result<Arc<dyn MachineBackend>, HostError> {
        let released = record.phase == WorkspacePhase::Released
            || record.sub.as_ref().and_then(|sub| sub.cloud_gone) == Some(true);
        if released {
            return Ok(self.ctx.released_backend());
        }
        self.module_of(record.kind)?.backend(record)
    }
    /// kinds.ts:519 (rule 22).
    pub fn backend_of_kind(
        &self,
        kind: WorkspaceKind,
        place: Option<&str>,
    ) -> Result<Arc<dyn MachineBackend>, HostError> {
        let record: WorkspaceRecord = serde_json::from_value(json!({
            "id": "",
            "name": "",
            "kind": kind,
            "machineId": "",
            "phase": "running",
            "golden": "",
            "createdAt": "",
            "project": "",
            "spec": {},
            "size": { "cpu": 0, "memMb": 0 },
            "firstLife": false,
            "place": place,
        }))
        .map_err(|e| HostError::new(e.to_string()))?;
        self.module_of(kind)?.backend(&record)
    }
    /// kinds.ts:525-531 (rule 22).
    pub fn keeps_images(&self, at: &dyn MachineBackend) -> bool {
        at.capabilities().images
    }
    pub fn pause_keeps_disk(&self, at: &dyn MachineBackend) -> bool {
        at.capabilities().pause_mode == Some(PauseMode::Disk)
    }
    pub fn names_workspace(&self, at: &dyn MachineBackend) -> bool {
        at.names_workspace()
    }
    /// kinds.ts:534 (rule 22): the lifecycle's budgets' daemon answer time is what readers take.
    pub fn lifecycle_of(&self, entry: &LiveWorkspace) -> Result<Arc<dyn Lifecycle>, HostError> {
        let record = entry.record.borrow().clone();
        let backend = self.backend_for(&record)?;
        backend.lifecycle().ok_or_else(|| {
            HostError::new(format!(
                "{} machines declare no lifecycle",
                kind_word(record.kind)
            ))
        })
    }
    /// kinds.ts:539 (rule 23): the options the kind's exec stream is made with.
    pub fn exec_options_for(&self, o: Option<MachineExecOptions>) -> Option<MachineExecOptions> {
        match (self.ctx.machine_exec_opts(), o) {
            (None, o) => o,
            (Some(base), None) => Some(base),
            (Some(base), Some(over)) => Some(over_exec_options(base, over)),
        }
    }
    /// kinds.ts:539 (rule 23): the module's exec stream for the entry's kind, with `exec_options_for`'s options.
    pub fn exec_factory_for(
        &self,
        entry: &LiveWorkspace,
        o: Option<MachineExecOptions>,
        waiting: Option<TurnWaiting>,
    ) -> Result<Arc<dyn ExecStreamFactory>, HostError> {
        let kind = entry.record.borrow().kind;
        Ok(self
            .module_of(kind)?
            .exec_stream(entry, self.exec_options_for(o), waiting))
    }
    /// kinds.ts:546 (rule 24).
    pub fn project_memory_key(&self, agent: &str, project: &ProjectView) -> Option<String> {
        let keyed = catalog_agents()
            .into_iter()
            .any(|row| row.id == agent && row.project_key_env.is_some());
        if keyed {
            Some(project.memory_key.clone())
        } else {
            project_state_key(agent, &project.path)
        }
    }
    /// kinds.ts:556 (rule 25).
    pub async fn thread_folder(
        &self,
        entry: &LiveWorkspace,
        cwd: Option<&str>,
    ) -> Result<String, HostError> {
        if let Some(cwd) = cwd {
            return Ok(cwd.to_string());
        }
        let record = entry.record.borrow().clone();
        if let Some(folder) = self.module_of(record.kind)?.folder(&record) {
            return Ok(folder);
        }
        Ok(self.ctx.project_held(&record.project).path)
    }
}
