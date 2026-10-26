// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/project-landing.ts at 65f6b89c7: the landing roads as modules, one per kind of
//! computer a project can be added on, and the scripts every road runs there. Specs: spec/landing_scripts.md (the
//! scripts, :119-352 and :483-504) and spec/landing_roads.md (the roads, :354-481 and :506-639).
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use indexmap::{IndexMap, IndexSet};
use serde::Serialize;
use wsp_catalog::roads::{CLAUDE_CONFIG_DIR, GUEST_HOME};
use wsp_engine::errors::EngineError;
use wsp_engine::exec_detached::put::INLINE_EXEC_MS;
use wsp_engine::machine::{ExecOptions, Machine};
use wsp_engine::project_install::{install_script, project_installs, INSTALL_MS, INSTALL_STORES};
use wsp_engine::run_env::{env_input, with_env_from_input};
use wsp_wire::types::daemon_contract::{BROKER_PORT, SEEDED_REFS};
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::project_seed::{SEED_DIR, SEED_MEMORY_DIR, SEED_PATCH};
use wsp_wire::types::shell_quote::{shell_line, shell_quote};
use wsp_wire::views::place::MachineBind;
use wsp_wire::views::workspace::{
    BranchOfRecord, ProjectAddStage, ProjectSource, ProjectView, ProjectViewGit, ProjectViewInstalled, ProjectViewSeeded,
    ProjectViewSeededMemory, SeedChoice, SeedPlan,
};
use wsp_wire::views::workspace_more::ProjectBranch;
use wsp_wire::wire::machine_link::project_folder_named;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;
use wsp_words::js::{is_js_whitespace, js_round, js_trim, number_string, to_number};
use wsp_words::js_date::iso_string;
use wsp_words::project_path::{claude_memory_dir, claude_project_key, place_daemon_paths, place_install_log};
use wsp_words::project_seed::{seed_bytes, seed_commits_landed_line, seed_commits_lost_line, seeding_line};
use wsp_words::projects::{
    project_left_on_computer_line, project_path_on, project_removed_at_provider_line, project_removed_here_line, seed_memory_kept_line,
};
use wsp_words::workspace_state::unpushed_unread_line;

use crate::console;
use crate::project_sources::ProjectSourceModule;

/// Every git line of the patch step runs as wsp's own identity, since git refuses a commit without one. project-landing.ts:60.
const AS_WSP: [&str; 4] = ["-c", "user.name=wsp", "-c", "user.email=wsp@localhost"];
/// Every remote form git may be handed for GitHub, each rewritten to plain http. project-landing.ts:186.
const GITHUB_REMOTES: [&str; 4] = ["https://github.com/", "git@github.com:", "ssh://git@github.com/", "ssh://git@ssh.github.com:443/"];

/// The folder a fork of an image holds its checkout under. project-landing.ts:107.
pub const FORK_CHECKOUT_HOME: &str = "/root";
/// How long the clone gets on the computer. project-landing.ts:115.
pub const CLONE_MS: u64 = 600_000;
/// How long the small steps get. project-landing.ts:117.
pub const STEP_MS: u64 = 120_000;
/// What the memory step prints where the computer already keeps memory at the agent's path. project-landing.ts:251.
pub const MEMORY_KEPT_MARK: &str = "wsp-memory-kept";
/// What the unsaved read prints for a folder that holds no git checkout. project-landing.ts:254.
pub const NO_CHECKOUT_MARK: &str = "wsp-no-checkout";
/// How many names the add tries in a home. project-landing.ts:495.
pub const NAMES_TRIED: u32 = 99;

/// How far the add has got. project-landing.ts:14.
pub type LandingReport = Rc<dyn Fn(ProjectAddStage, &str)>;

/// Where one computer keeps a project. project-landing.ts:18.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPlaces {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
    pub memory_dir: String,
}

/// The seed archive a folder on this computer packed. project-landing.ts:29.
#[derive(Clone, Debug, PartialEq)]
pub struct LandSeed {
    pub tar: Vec<u8>,
    pub choice: SeedChoice,
    pub plan: SeedPlan,
}

/// What the add hands a landing road. project-landing.ts:26.
#[derive(Clone)]
pub struct LandRequest {
    pub project: ProjectView,
    pub source: Rc<dyn ProjectSourceModule>,
    pub seed: Option<LandSeed>,
    pub report: LandingReport,
}

/// What one landing left behind, which the record keeps. Serialized in the TS's interface order for tests; the
/// record's own keys are the projects area's. project-landing.ts:34.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Landed {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<ProjectViewGit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seeded: Option<ProjectViewSeeded>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed: Option<ProjectViewInstalled>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

/// A computer the person joined, as its login runs every command there, and that login's home.
#[derive(Clone)]
pub struct LandingComputer {
    pub machine: Arc<dyn Machine>,
    pub home: String,
}

/// What a landing road may ask of the runtime. project-landing.ts:52.
#[derive(Clone)]
pub struct LandingDeps {
    pub land: Rc<dyn Fn(Arc<dyn Machine>, String, Vec<u8>) -> LocalBoxFuture<'static, Result<(), HostError>>>,
    /// Where wsp writes its own working files on that machine.
    pub scratch: Rc<dyn Fn(&dyn Machine) -> String>,
    /// Absent on a provider and on this computer.
    pub computer: Option<LandingComputer>,
    /// Where Claude Code keeps its projects on this Mac.
    pub mac_state_home: String,
    /// The branch a folder on this computer has checked out; `None` on a detached head or where git answers nothing.
    pub branch_here: Rc<dyn Fn(&str) -> LocalBoxFuture<'static, Option<String>>>,
    pub computer_name: String,
    /// What a clone on a computer the person owns reads on its own input.
    pub clone_env: Option<Rc<dyn Fn() -> IndexMap<String, String>>>,
    pub now: Rc<dyn Fn() -> i64>,
}

/// One landing road. The TS's optional members answer `None` where the road has none. project-landing.ts:71.
#[async_trait(?Send)]
pub trait ProjectLanding {
    /// `"box"`, `"provider"` or `"mac"`.
    fn kind(&self) -> &'static str;
    fn path(&self, name: &str, source: &ProjectSource, deps: &LandingDeps) -> Result<String, HostError>;
    fn places(&self, project: &ProjectView, memory_key: &str, deps: &LandingDeps) -> Result<ProjectPlaces, HostError>;
    fn lands_at_add(&self, seeding: bool) -> bool;
    async fn land(&self, o: LandRequest, deps: &LandingDeps) -> Result<Landed, HostError>;
    fn refusal(&self, project: &ProjectView, deps: &LandingDeps) -> Option<String>;
    async fn remove(&self, project: &ProjectView, deps: &LandingDeps) -> Result<String, HostError>;
    async fn folder_stands(&self, _project: &ProjectView, _deps: &LandingDeps) -> Option<Result<bool, HostError>> {
        None
    }
    async fn seed_into(&self, _project: &ProjectView, _tar: Vec<u8>, _deps: &LandingDeps) -> Option<Result<(), HostError>> {
        None
    }
    async fn unsaved(&self, _project: &ProjectView, _deps: &LandingDeps) -> Option<Option<String>> {
        None
    }
    fn workspace_binds(&self, project: &ProjectView) -> Vec<MachineBind>;
    async fn branch(&self, project: &ProjectView, deps: &LandingDeps) -> ProjectBranch;
}

fn engine_err(e: EngineError) -> HostError {
    HostError::new(e.to_string())
}

async fn run_on(machine: &dyn Machine, cmd: &str, timeout_ms: u64) -> Result<ExecResult, HostError> {
    machine.exec(cmd, ExecOptions { timeout_ms: Some(timeout_ms), ..ExecOptions::default() }).await.map_err(engine_err)
}

fn computer_of(deps: &LandingDeps) -> Result<&LandingComputer, HostError> {
    deps.computer.as_ref().ok_or_else(|| {
        HostError::new(format!("{} answered no login to run the add as, so nothing can be cloned there", deps.computer_name))
    })
}

fn failed_with(out: &ExecResult, fallback: String) -> String {
    last_line(&out.stderr).or_else(|| last_line(&out.stdout)).unwrap_or(fallback)
}

fn iso_now(deps: &LandingDeps) -> String {
    iso_string((deps.now)() as f64).unwrap_or_default()
}

/// The places a clone lands in: where the work runs, where the cloning line says it holds, and its memory and log.
struct Places {
    checkout: String,
    holds: String,
    memory_dir: String,
    log: String,
}

struct SeedPatched {
    commits: i64,
    notice: Option<String>,
}

/// The clone, the seed and the install on one machine, the half both roads that clone share. project-landing.ts:354.
async fn clone_seed_install(
    o: &LandRequest,
    deps: &LandingDeps,
    machine: &Arc<dyn Machine>,
    places: &Places,
    owned_env: Option<IndexMap<String, String>>,
) -> Result<Landed, HostError> {
    let project = &o.project;
    let seed_tar = o.seed.as_ref().map(|_| format!("{}/seed-{}.tgz", (deps.scratch)(machine.as_ref()), project.id));
    if let (Some(seed), Some(tar)) = (&o.seed, &seed_tar) {
        (deps.land)(machine.clone(), tar.clone(), seed.tar.clone()).await?;
    }
    (o.report)(ProjectAddStage::Cloning, &format!("Cloning {} into {}.", project.remote, places.holds));
    let ask = CloneAsk {
        source: o.source.clone(),
        remote: project.remote.clone(),
        checkout: places.checkout.clone(),
        computer: deps.computer_name.clone(),
        branch: project.base.clone(),
        setup_git: owned_env.as_ref().map(|_| false),
        brokered: None,
    };
    let script = clone_script(&ask, seed_tar.as_deref());
    if let Some(seed) = &o.seed {
        (o.report)(ProjectAddStage::Seeding, &seeding_line(&seed.plan, &seed.choice));
    }
    let env = owned_env.unwrap_or_default();
    let (cmd, opts) = if env.is_empty() {
        (script, ExecOptions { timeout_ms: Some(CLONE_MS), ..ExecOptions::default() })
    } else {
        (with_env_from_input(&script), ExecOptions { timeout_ms: Some(CLONE_MS), stdin: Some(env_input(&env)), ..ExecOptions::default() })
    };
    let ran = machine.exec(&cmd, opts).await.map_err(engine_err)?;
    if ran.exit_code.get() != 0 {
        return Err(HostError::new(failed_with(&ran, format!("the clone exited {}", ran.exit_code.get()))));
    }
    let patched = match &o.seed {
        Some(seed) => Some(patch_seed(seed, o, deps, machine.as_ref(), &places.checkout).await?),
        None => None,
    };
    let mut memory_kept = false;
    if let (Some(seed), Some(tar)) = (&o.seed, &seed_tar) {
        let memory = seed.choice.memory && seed.plan.memory.is_some();
        let rest = run_on(machine.as_ref(), &seed_rest_script(&places.checkout, &places.memory_dir, tar, memory), STEP_MS).await?;
        if rest.exit_code.get() != 0 {
            return Err(HostError::new(failed_with(&rest, format!("landing the seed exited {}", rest.exit_code.get()))));
        }
        memory_kept = rest.stdout.contains(MEMORY_KEPT_MARK);
        if memory_kept {
            (o.report)(ProjectAddStage::Seeding, &seed_memory_kept_line(&deps.computer_name));
        }
    }
    let seeded = o.seed.as_ref().map(|seed| {
        let memory = seed.plan.memory.as_ref().filter(|_| seed.choice.memory);
        ProjectViewSeeded {
            files: JsInt::from(seed.choice.files.len() as i64),
            bytes: JsInt::from(seed_bytes(&seed.plan, &seed.choice) as i64),
            memory: match memory {
                None => ProjectViewSeededMemory::None,
                Some(_) if memory_kept => ProjectViewSeededMemory::Kept,
                Some(_) => ProjectViewSeededMemory::Landed,
            },
            memory_files: memory.map(|m| JsInt::from(m.files.0 .0 as i64)),
            commits: JsInt::from(patched.as_ref().map_or(0, |p| p.commits)),
            at: iso_now(deps),
        }
    });
    let notice = patched.and_then(|p| p.notice);
    let root = run_on(machine.as_ref(), &format!("ls -A {}", shell_quote(&places.checkout)), STEP_MS).await?;
    let names: Vec<String> = root.stdout.split('\n').map(|line| line.trim().to_string()).collect();
    let installs = project_installs(&names, &places.checkout);
    let landed = Landed { seeded, notice, ..Landed::default() };
    if installs.is_empty() {
        return Ok(landed);
    }
    let began = (deps.now)();
    for install in &installs {
        (o.report)(ProjectAddStage::Installing, &format!("{} in {}.", install.command, places.checkout));
        let script = install_script(install, &places.checkout, &places.log);
        let ok = machine.exec(&script, ExecOptions { timeout_ms: Some(INSTALL_MS), ..ExecOptions::default() }).await.map_err(engine_err)?;
        if ok.exit_code.get() != 0 {
            let said = failed_with(&ok, format!("exit {}", ok.exit_code.get()));
            return Err(HostError::new(format!(
                "{} in {}: {said}; its whole output is {} on {}",
                install.command, places.checkout, places.log, project.computer
            )));
        }
    }
    let seconds = js_round((((deps.now)() - began) as f64) / 1000.0);
    Ok(Landed {
        installed: Some(ProjectViewInstalled {
            row: installs.iter().map(|i| i.row.as_str()).collect::<Vec<_>>().join(", "),
            command: installs.iter().map(|i| i.command.as_str()).collect::<Vec<_>>().join("; "),
            at: iso_now(deps),
            seconds: JsNumber(seconds),
        }),
        ..landed
    })
}

/// The seed's commits put on the checkout, as its own step. A refusal is not the add's failure. project-landing.ts:444.
async fn patch_seed(
    seed: &LandSeed,
    o: &LandRequest,
    deps: &LandingDeps,
    machine: &dyn Machine,
    checkout: &str,
) -> Result<SeedPatched, HostError> {
    let unpushed = match &seed.plan.unpushed {
        Some(unpushed) if seed.choice.commits => unpushed,
        _ => return Ok(SeedPatched { commits: 0, notice: None }),
    };
    let back = clone_branch(seed, o.project.base.as_deref(), checkout, machine).await?;
    let step = PatchStep { branch: seed.plan.branch.clone(), base: unpushed.base.clone(), back };
    let ran = run_on(machine, &patch_script(&step, checkout), STEP_MS).await?;
    if ran.exit_code.get() == 0 {
        let counted = last_line(&ran.stdout).map_or(f64::NAN, |line| to_number(&line));
        if counted.is_finite() && counted.fract() == 0.0 {
            (o.report)(ProjectAddStage::Seeding, &seed_commits_landed_line(counted));
            return Ok(SeedPatched { commits: counted as i64, notice: None });
        }
    }
    run_on(machine, &patch_cleanup_script(&step, checkout), STEP_MS).await?;
    let said = failed_with(&ran, format!("git exited {}", ran.exit_code.get()));
    let notice = seed_commits_lost_line(unpushed.commits.0 .0, &step.branch, &deps.computer_name, &said, &step.back);
    (o.report)(ProjectAddStage::Seeding, &notice);
    Ok(SeedPatched { commits: 0, notice: Some(notice) })
}

/// The branch the clone came up on, read off the checkout itself. project-landing.ts:455.
async fn clone_branch(seed: &LandSeed, base: Option<&str>, checkout: &str, machine: &dyn Machine) -> Result<String, HostError> {
    let read = run_on(machine, &shell_line(&["git", "-C", checkout, "symbolic-ref", "--short", "HEAD"]), STEP_MS).await?;
    let on = if read.exit_code.get() == 0 { last_line(&read.stdout) } else { None };
    Ok(on.or_else(|| base.map(str::to_string)).or_else(|| seed.plan.default_branch.clone()).unwrap_or_else(|| seed.plan.branch.clone()))
}

/// A computer the person joined: the project is a folder in the home of the login the computer was joined with.
struct BoxLanding;

#[async_trait(?Send)]
impl ProjectLanding for BoxLanding {
    fn kind(&self) -> &'static str {
        "box"
    }

    fn path(&self, name: &str, source: &ProjectSource, deps: &LandingDeps) -> Result<String, HostError> {
        project_path_on(source, name, Some(&computer_of(deps)?.home))
    }

    fn places(&self, _project: &ProjectView, memory_key: &str, deps: &LandingDeps) -> Result<ProjectPlaces, HostError> {
        let home = &computer_of(deps)?.home;
        Ok(ProjectPlaces { checkout: None, memory_dir: claude_memory_dir(&under_login_home(home, CLAUDE_CONFIG_DIR), memory_key) })
    }

    fn lands_at_add(&self, _seeding: bool) -> bool {
        true
    }

    async fn land(&self, o: LandRequest, deps: &LandingDeps) -> Result<Landed, HostError> {
        let computer = computer_of(deps)?;
        let machine = computer.machine.clone();
        let home = computer.home.clone();
        if matches!(o.project.source, ProjectSource::Folder(_)) && o.seed.is_none() {
            return Ok(Landed { git: Some(ProjectViewGit { top: o.project.path.clone() }), ..Landed::default() });
        }
        let picked = run_on(machine.as_ref(), &free_folder_script(&home, &o.project.name), STEP_MS).await?;
        let path = match last_line(&picked.stdout) {
            Some(path) if picked.exit_code.get() == 0 => path,
            _ => {
                let fallback = format!("no folder for {} in {home}: exit {}", o.project.name, picked.exit_code.get());
                return Err(HostError::new(last_line(&picked.stderr).unwrap_or(fallback)));
            }
        };
        let memory_key = claude_project_key(&path);
        let memory_dir = claude_memory_dir(&under_login_home(&home, CLAUDE_CONFIG_DIR), &memory_key);
        let put_dir = place_daemon_paths(&home).put_dir;
        let places = Places {
            checkout: path.clone(),
            holds: path.clone(),
            memory_dir: memory_dir.clone(),
            log: place_install_log(&home, &o.project.id),
        };
        let owned_env = Some(deps.clone_env.as_ref().map(|env| env()).unwrap_or_default());
        let mut req = o.clone();
        req.project.path = path.clone();
        req.project.memory_key = memory_key.clone();
        req.project.memory_dir = memory_dir.clone();
        let mut scratch_deps = deps.clone();
        scratch_deps.scratch = Rc::new(move |_| put_dir.clone());
        match clone_seed_install(&req, &scratch_deps, &machine, &places, owned_env).await {
            Ok(landed) => Ok(Landed {
                path: Some(path.clone()),
                memory_key: Some(memory_key),
                memory_dir: Some(memory_dir),
                git: Some(ProjectViewGit { top: path }),
                ..landed
            }),
            Err(e) => {
                if let Err(why) = run_on(machine.as_ref(), &format!("rm -rf -- {}", shell_quote(&path)), STEP_MS).await {
                    console::warn(&format!(
                        "{path} on {} was not swept after the add of {} failed: {}",
                        deps.computer_name, o.project.name, why.message
                    ));
                }
                Err(e)
            }
        }
    }

    fn refusal(&self, _project: &ProjectView, _deps: &LandingDeps) -> Option<String> {
        None
    }

    async fn remove(&self, project: &ProjectView, deps: &LandingDeps) -> Result<String, HostError> {
        Ok(project_left_on_computer_line(&project.name, &deps.computer_name, &project.path))
    }

    async fn folder_stands(&self, project: &ProjectView, deps: &LandingDeps) -> Option<Result<bool, HostError>> {
        let computer = match computer_of(deps) {
            Ok(computer) => computer,
            Err(e) => return Some(Err(e)),
        };
        let read = match run_on(computer.machine.as_ref(), &format!("test -e {}", shell_quote(&project.path)), STEP_MS).await {
            Ok(read) => read,
            Err(e) => return Some(Err(e)),
        };
        if read.exit_code.get() > 1 {
            let fallback = format!("reading {} on {} exited {}", project.path, deps.computer_name, read.exit_code.get());
            return Some(Err(HostError::new(last_line(&read.stderr).unwrap_or(fallback))));
        }
        Some(Ok(read.exit_code.get() == 0))
    }

    async fn seed_into(&self, project: &ProjectView, tar: Vec<u8>, deps: &LandingDeps) -> Option<Result<(), HostError>> {
        Some(seed_into_on_box(project, tar, deps).await)
    }

    async fn unsaved(&self, project: &ProjectView, deps: &LandingDeps) -> Option<Option<String>> {
        let Some(checkout) = project.checkout.as_ref() else {
            return Some(None);
        };
        let machine = match computer_of(deps) {
            Ok(computer) => computer.machine.clone(),
            Err(e) => return Some(Some(unpushed_unread_line(&project.name, &e.message))),
        };
        let run: RunHere = Rc::new(move |cmd: String, timeout_ms: u64| {
            let machine = machine.clone();
            async move { run_on(machine.as_ref(), &cmd, timeout_ms).await }.boxed_local()
        });
        Some(read_unsaved(&project_folder_named(&project.name, checkout), checkout, run).await)
    }

    fn workspace_binds(&self, _project: &ProjectView) -> Vec<MachineBind> {
        Vec::new()
    }

    async fn branch(&self, project: &ProjectView, deps: &LandingDeps) -> ProjectBranch {
        let read = match &deps.computer {
            Some(computer) => {
                let cmd = shell_line(&["git", "-C", &project.path, "symbolic-ref", "--quiet", "--short", "HEAD"]);
                computer.machine.exec(&cmd, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..ExecOptions::default() }).await.ok()
            }
            None => None,
        };
        let branch = read.filter(|r| r.exit_code.get() == 0).map(|r| r.stdout.trim().to_string()).unwrap_or_default();
        ProjectBranch { branch: if branch.is_empty() { None } else { Some(branch) }, folder: true }
    }
}

/// Puts a seed archive's files into the folder a joined computer's project stands in. project-landing.ts:530.
async fn seed_into_on_box(project: &ProjectView, tar: Vec<u8>, deps: &LandingDeps) -> Result<(), HostError> {
    let computer = computer_of(deps)?;
    let seed_tar = format!("{}/seed-{}.tgz", place_daemon_paths(&computer.home).put_dir, project.id);
    (deps.land)(computer.machine.clone(), seed_tar.clone(), tar).await?;
    let ran = run_on(computer.machine.as_ref(), &seed_into_script(&project.path, &seed_tar), STEP_MS).await?;
    if ran.exit_code.get() != 0 {
        return Err(HostError::new(failed_with(
            &ran,
            format!("putting the kept files into {} exited {}", project.path, ran.exit_code.get()),
        )));
    }
    Ok(())
}

/// A provider: nothing of the project sits on a disk of this person's there, so the record stands alone.
struct ProviderLanding;

#[async_trait(?Send)]
impl ProjectLanding for ProviderLanding {
    fn kind(&self) -> &'static str {
        "provider"
    }

    fn path(&self, name: &str, source: &ProjectSource, _deps: &LandingDeps) -> Result<String, HostError> {
        project_path_on(source, name, Some(FORK_CHECKOUT_HOME))
    }

    fn places(&self, _project: &ProjectView, memory_key: &str, _deps: &LandingDeps) -> Result<ProjectPlaces, HostError> {
        Ok(ProjectPlaces { checkout: None, memory_dir: guest_memory_dir(memory_key) })
    }

    fn lands_at_add(&self, _seeding: bool) -> bool {
        false
    }

    async fn land(&self, _o: LandRequest, _deps: &LandingDeps) -> Result<Landed, HostError> {
        Ok(Landed::default())
    }

    fn refusal(&self, _project: &ProjectView, _deps: &LandingDeps) -> Option<String> {
        None
    }

    async fn remove(&self, project: &ProjectView, deps: &LandingDeps) -> Result<String, HostError> {
        Ok(project_removed_at_provider_line(&project.name, &deps.computer_name))
    }

    fn workspace_binds(&self, _project: &ProjectView) -> Vec<MachineBind> {
        Vec::new()
    }

    async fn branch(&self, project: &ProjectView, _deps: &LandingDeps) -> ProjectBranch {
        ProjectBranch { branch: Some(project.base.clone().unwrap_or_else(|| project.default_branch.clone())), folder: false }
    }
}

/// The computer the app runs on: the person's own folder is the project, so nothing is cloned, seeded or installed.
struct MacLanding;

#[async_trait(?Send)]
impl ProjectLanding for MacLanding {
    fn kind(&self) -> &'static str {
        "mac"
    }

    fn path(&self, name: &str, source: &ProjectSource, _deps: &LandingDeps) -> Result<String, HostError> {
        project_path_on(source, name, None)
    }

    fn places(&self, project: &ProjectView, memory_key: &str, deps: &LandingDeps) -> Result<ProjectPlaces, HostError> {
        let checkout = match &project.source {
            ProjectSource::Folder(folder) => folder.path.clone(),
            _ => project.path.clone(),
        };
        Ok(ProjectPlaces { checkout: Some(checkout), memory_dir: claude_memory_dir(&deps.mac_state_home, memory_key) })
    }

    fn lands_at_add(&self, _seeding: bool) -> bool {
        false
    }

    async fn land(&self, _o: LandRequest, _deps: &LandingDeps) -> Result<Landed, HostError> {
        Ok(Landed::default())
    }

    fn refusal(&self, _project: &ProjectView, _deps: &LandingDeps) -> Option<String> {
        None
    }

    async fn remove(&self, project: &ProjectView, _deps: &LandingDeps) -> Result<String, HostError> {
        Ok(project_removed_here_line(&project.name))
    }

    fn workspace_binds(&self, _project: &ProjectView) -> Vec<MachineBind> {
        Vec::new()
    }

    async fn branch(&self, project: &ProjectView, deps: &LandingDeps) -> ProjectBranch {
        ProjectBranch { branch: (deps.branch_here)(&project.path).await, folder: true }
    }
}

/// box, provider, mac, in that order. project-landing.ts:613.
pub fn project_landings() -> IndexMap<&'static str, Rc<dyn ProjectLanding>> {
    let mut roads: IndexMap<&'static str, Rc<dyn ProjectLanding>> = IndexMap::new();
    roads.insert("box", Rc::new(BoxLanding));
    roads.insert("provider", Rc::new(ProviderLanding));
    roads.insert("mac", Rc::new(MacLanding));
    roads
}

/// The road for one kind, or `no project landing road for <kind>`. project-landing.ts:620.
pub fn project_landing(kind: &str) -> Result<Rc<dyn ProjectLanding>, HostError> {
    project_landings().get(kind).cloned().ok_or_else(|| HostError::new(format!("no project landing road for {kind}")))
}

/// `claudeMemoryDir(CLAUDE_CONFIG_DIR, memory_key)`. project-landing.ts:628.
pub fn guest_memory_dir(memory_key: &str) -> String {
    claude_memory_dir(CLAUDE_CONFIG_DIR, memory_key)
}

/// A clone where the host keeps the machine's credentials. project-landing.ts:125 `brokered`.
#[derive(Clone, Debug, PartialEq)]
pub struct BrokeredClone {
    pub of_record: BranchOfRecord,
    /// The project's own owner/name.
    pub origin: String,
    pub held: bool,
}

/// What every clone line reads. project-landing.ts:125.
#[derive(Clone)]
pub struct CloneAsk {
    pub source: Rc<dyn ProjectSourceModule>,
    pub remote: String,
    pub checkout: String,
    pub computer: String,
    pub branch: Option<String>,
    /// `Some(false)` drops the setup-git line; `None` and `Some(true)` keep it.
    pub setup_git: Option<bool>,
    pub brokered: Option<BrokeredClone>,
}

/// project-landing.ts:125.
pub fn clone_lines(o: &CloneAsk) -> Vec<String> {
    if let Some(brokered) = &o.brokered {
        let mut branch: Vec<&str> = vec!["git", "clone", "--depth", "1"];
        if let Some(b) = o.branch.as_deref() {
            branch.extend(["--branch", b]);
        }
        branch.extend(["--", o.remote.as_str(), o.checkout.as_str()]);
        let mut lines = vec!["set -e".to_string(), format!("mkdir -p {}", shell_quote(parent_of(&o.checkout))), shell_line(&branch)];
        lines.extend(on_record_lines(&o.checkout, &brokered.of_record, &brokered.origin, brokered.held));
        return lines;
    }
    plain_clone_lines(o)
}

fn plain_clone_lines(o: &CloneAsk) -> Vec<String> {
    let mut lines = vec!["set -e".to_string()];
    if let Some(cli) = o.source.cli() {
        lines.push(format!("command -v {} >/dev/null 2>&1 || {{ echo {} >&2; exit 1; }}", cli.bin, shell_quote(&cli.missing(&o.computer))));
        if o.setup_git != Some(false) {
            lines.push(cli.setup_git.to_string());
        }
    }
    lines.push(format!("mkdir -p {}", shell_quote(parent_of(&o.checkout))));
    lines.push(o.source.clone_command(&o.remote, &o.checkout, o.branch.as_deref()));
    lines
}

/// The folder above a path, as the TS's `replace(/\/[^/]+$/, "")` cuts it; a path ending in a slash is left whole.
fn parent_of(path: &str) -> &str {
    match path.rfind('/') {
        Some(i) if i + 1 < path.len() => &path[..i],
        _ => path,
    }
}

/// project-landing.ts:155.
pub fn on_record_lines(checkout: &str, of_record: &BranchOfRecord, origin: &str, held: bool) -> Vec<String> {
    let branch = of_record.branch.as_str();
    let (owner, name) = (of_record.owner.as_str(), of_record.name.as_str());
    let git = |args: &[&str]| {
        let mut words = vec!["git", "-C", checkout];
        words.extend_from_slice(args);
        shell_line(&words)
    };
    let fork = format!("{owner}/{name}").to_lowercase() != origin.to_lowercase();
    let remote = if fork { owner } else { "origin" };
    let fetch_refspec = format!("+refs/heads/{branch}*:refs/remotes/origin/{branch}*");
    let fork_url = format!("https://github.com/{owner}/{name}.git");
    let mut lines = vec![if fork {
        git(&["remote", "add", "-t", &format!("{branch}*"), remote, &fork_url])
    } else {
        git(&["config", "--add", "remote.origin.fetch", &fetch_refspec])
    }];
    match &of_record.start_sha {
        None => lines.push(git(&["checkout", "-q", "-B", branch])),
        Some(start) => {
            let has = git(&["cat-file", "-e", &format!("{start}^{{commit}}")]);
            let fetch = git(&["fetch", "-q", "--depth", "1", remote, start]);
            lines.push(format!("{has} 2>/dev/null || {fetch}"));
            lines.push(git(&["checkout", "-q", "-B", branch, start]));
        }
    }
    if let (true, Some(start)) = (held, &of_record.start_sha) {
        lines.push(git(&["update-ref", &format!("refs/remotes/{remote}/{branch}"), start]));
    }
    if fork || held {
        lines.push(git(&["config", &format!("branch.{branch}.remote"), remote]));
        lines.push(git(&["config", &format!("branch.{branch}.merge"), &format!("refs/heads/{branch}")]));
    }
    lines.push(git(&["rev-parse", "HEAD"]));
    lines
}

/// The repo of record as the git environment reads it. project-landing.ts:181 `repo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitEnvRepo {
    pub owner: String,
    pub name: String,
    pub public: bool,
}

/// GIT_CONFIG_COUNT, the numbered pairs and GIT_TERMINAL_PROMPT, in that key order. project-landing.ts:181.
pub fn brokered_git_env(token: &str, repo: Option<&GitEnvRepo>, direct: &[String]) -> IndexMap<String, String> {
    let mut repos: IndexSet<String> = IndexSet::new();
    if let Some(repo) = repo.filter(|r| r.public) {
        repos.insert(format!("{}/{}", repo.owner, repo.name));
    }
    repos.extend(direct.iter().cloned());
    let repos: Vec<String> = repos.into_iter().map(|r| format!("{r}.git")).collect();
    let mut pairs: Vec<(String, String)> =
        GITHUB_REMOTES.iter().map(|from| ("url.http://github.com/.insteadOf".to_string(), from.to_string())).collect();
    for own in &repos {
        for from in GITHUB_REMOTES {
            pairs.push((format!("url.https://github.com/{own}.insteadOf"), format!("{from}{own}")));
            pairs.push((format!("url.http://github.com/{own}.pushInsteadOf"), format!("{from}{own}")));
        }
    }
    pairs.push(("http.http://github.com/.proxy".to_string(), format!("http://wsp:{token}@127.0.0.1:{BROKER_PORT}")));
    pairs.push(("http.proxyAuthMethod".to_string(), "basic".to_string()));
    pairs.push(("credential.helper".to_string(), String::new()));
    let mut env = IndexMap::new();
    env.insert("GIT_CONFIG_COUNT".to_string(), pairs.len().to_string());
    for (i, (key, value)) in pairs.into_iter().enumerate() {
        env.insert(format!("GIT_CONFIG_KEY_{i}"), key);
        env.insert(format!("GIT_CONFIG_VALUE_{i}"), value);
    }
    env.insert("GIT_TERMINAL_PROMPT".to_string(), "0".to_string());
    env
}

/// The clone lines with the seed unpacked on top, joined by newlines. `o.brokered` is not read. project-landing.ts:200.
pub fn clone_script(o: &CloneAsk, seed_tar: Option<&str>) -> String {
    let plain = CloneAsk { brokered: None, ..o.clone() };
    let mut lines = plain_clone_lines(&plain);
    if let Some(seed) = seed_tar {
        lines.push(format!("tar -xzf {} -C {}", shell_quote(seed), shell_quote(&o.checkout)));
    }
    lines.join("\n")
}

/// project-landing.ts:209.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchStep {
    pub branch: String,
    pub base: String,
    pub back: String,
}

/// project-landing.ts:221.
pub fn patch_script(step: &PatchStep, checkout: &str) -> String {
    let mut at: Vec<&str> = vec!["git", "-C", checkout];
    at.extend(AS_WSP);
    let with = |tail: &[&str]| {
        let mut words = at.clone();
        words.extend_from_slice(tail);
        shell_line(&words)
    };
    let patch = format!("{checkout}/{SEED_PATCH}");
    let landed_ref = format!("{SEEDED_REFS}/{}", step.branch);
    let range = format!("{}..{}", step.base, step.branch);
    [
        "set -e".to_string(),
        with(&["checkout", "-B", &step.branch, &step.base]),
        with(&["am", "--3way", &patch]),
        with(&["update-ref", &landed_ref, "HEAD"]),
        with(&["checkout", &step.back]),
        with(&["rev-list", "--count", &range]),
    ]
    .join("\n")
}

/// project-landing.ts:239.
pub fn patch_cleanup_script(step: &PatchStep, checkout: &str) -> String {
    let mut at: Vec<&str> = vec!["git", "-C", checkout];
    at.extend(AS_WSP);
    let with = |tail: &[&str]| {
        let mut words = at.clone();
        words.extend_from_slice(tail);
        shell_line(&words)
    };
    let origin_back = format!("origin/{}", step.back);
    let mut lines = vec![with(&["am", "--abort"]), with(&["checkout", "-f", &step.back]), with(&["reset", "--hard", &origin_back])];
    if step.branch != step.back {
        lines.push(with(&["branch", "-D", &step.branch]));
    }
    lines.join("\n")
}

/// project-landing.ts:262.
pub fn unsaved_script(checkout: &str) -> String {
    let git = "git -c safe.directory='*' -c core.fsmonitor=false --no-optional-locks -C";
    let at = shell_quote(checkout);
    let excluded: Vec<String> = INSTALL_STORES.iter().map(|dir| shell_quote(&format!(":(top,exclude){dir}"))).collect();
    let glob = shell_quote(&format!("--glob={SEEDED_REFS}/*"));
    [
        format!("[ -d {at} ] || {{ echo {NO_CHECKOUT_MARK}; exit 0; }}"),
        "command -v git >/dev/null 2>&1 || exit 1".to_string(),
        format!("[ -L {at}/.git ] && [ ! -e {at}/.git ] && exit 1"),
        format!("if [ -e {at}/.git ] || [ -L {at}/.git ]; then {git} {at} rev-parse --git-dir >/dev/null 2>&1 || exit 1;"),
        format!("else {git} {at} rev-parse --git-dir >/dev/null 2>&1 || {{ echo {NO_CHECKOUT_MARK}; exit 0; }}; fi"),
        format!("trees=$({git} {at} worktree list --porcelain) || exit 1"),
        "heads=$(printf '%s\\n' \"$trees\" | sed -n 's/^HEAD //p' | grep -v '^0*$')".to_string(),
        format!("ahead=$({git} {at} rev-list --count --branches $heads --not --remotes {glob}) || exit 1"),
        "changed=0".to_string(),
        "tree=".to_string(),
        "while IFS= read -r line; do".to_string(),
        "  case $line in".to_string(),
        "    \"worktree \"*) tree=${line#worktree } ;;".to_string(),
        "    bare) tree= ;;".to_string(),
        "    \"\")".to_string(),
        "      if [ -n \"$tree\" ] && [ -d \"$tree\" ]; then".to_string(),
        format!("        said=$({git} \"$tree\" status --porcelain -- {}) || exit 1", excluded.join(" ")),
        "        [ -z \"$said\" ] || changed=$((changed + $(printf '%s\\n' \"$said\" | wc -l)))".to_string(),
        "      fi".to_string(),
        "      tree= ;;".to_string(),
        "  esac".to_string(),
        "done <<WSP_TREES".to_string(),
        "$trees".to_string(),
        String::new(),
        "WSP_TREES".to_string(),
        format!("stashed=$({git} {at} stash list) || exit 1"),
        "stashes=0".to_string(),
        "[ -z \"$stashed\" ] || stashes=$(($(printf '%s\\n' \"$stashed\" | wc -l)))".to_string(),
        "echo \"$ahead $changed $stashes\"".to_string(),
    ]
    .join("\n")
}

/// project-landing.ts:301.
pub fn unsaved_of(name: &str, ran: &ExecResult) -> Option<String> {
    let said = if ran.exit_code.get() == 0 { last_line(&ran.stdout) } else { None };
    if said.as_deref() == Some(NO_CHECKOUT_MARK) {
        return None;
    }
    let text = said.clone().unwrap_or_default();
    let counts: Vec<f64> = text.split(is_js_whitespace).filter(|n| !n.is_empty()).map(to_number).collect();
    let counted = |n: &f64| n.fract() == 0.0 && *n >= 0.0;
    if counts.len() != 3 || !counts.iter().all(counted) {
        let why = if ran.exit_code.get() != 0 {
            last_line(&ran.stderr).unwrap_or_else(|| format!("the read exited {}", ran.exit_code.get()))
        } else {
            format!("the read printed {}", serde_json::Value::String(text.clone()))
        };
        return Some(unpushed_unread_line(name, &why));
    }
    counts_line(name, counts[0], counts[1], counts[2])
}

/// `unpushedLine` for a read of three counts, which are numbers as JavaScript has them: past `i64` they print as it prints them.
fn counts_line(name: &str, ahead: f64, changed: f64, stashes: f64) -> Option<String> {
    let count = |n: f64, one: &str, many: &str| format!("{} {}", number_string(n), if n == 1.0 { one } else { many });
    let parts: Vec<String> = [
        (ahead > 0.0).then(|| format!("{} not pushed", count(ahead, "commit", "commits"))),
        (changed > 0.0).then(|| count(changed, "uncommitted file", "uncommitted files")),
        (stashes > 0.0).then(|| count(stashes, "stash", "stashes")),
    ]
    .into_iter()
    .flatten()
    .collect();
    let (last, init) = parts.split_last()?;
    let listed = if init.is_empty() { last.clone() } else { format!("{} and {last}", init.join(", ")) };
    Some(format!("{name} holds {listed}"))
}

/// One command run where the checkout is, with its timeout in ms.
pub type RunHere = Rc<dyn Fn(String, u64) -> LocalBoxFuture<'static, Result<ExecResult, HostError>>>;

/// project-landing.ts:314.
pub async fn read_unsaved(name: &str, checkout: &str, run: RunHere) -> Option<String> {
    match run(unsaved_script(checkout), STEP_MS).await {
        Ok(ran) => unsaved_of(name, &ran),
        Err(e) => Some(unpushed_unread_line(name, &e.message)),
    }
}

/// project-landing.ts:336.
pub fn seed_rest_script(checkout: &str, memory_dir: &str, seed_tar: &str, memory: bool) -> String {
    let mut lines = vec!["set -e".to_string()];
    if memory {
        let memory_path = shell_quote(memory_dir);
        lines.push(format!("rmdir {memory_path} 2>/dev/null || true"));
        lines.push(format!("if [ -e {memory_path} ]; then"));
        lines.push(format!("  echo {}", shell_quote(MEMORY_KEPT_MARK)));
        lines.push("else".to_string());
        lines.push(format!("  mkdir -p {}", shell_quote(parent_of(memory_dir))));
        lines.push(format!("  mv {} {memory_path}", shell_quote(&format!("{checkout}/{SEED_MEMORY_DIR}"))));
        lines.push("fi".to_string());
    }
    lines.push(format!("rm -rf {} {}", shell_quote(&format!("{checkout}/{SEED_DIR}")), shell_quote(seed_tar)));
    lines.join("\n")
}

/// project-landing.ts:485.
pub fn seed_into_script(checkout: &str, seed_tar: &str) -> String {
    let tar = shell_quote(seed_tar);
    [
        format!(
            "tar -xzf {tar} -C {} --skip-old-files --exclude={} || {{ rc=$?; rm -f {tar}; exit $rc; }}",
            shell_quote(checkout),
            shell_quote(SEED_DIR)
        ),
        format!("rm -f {tar}"),
    ]
    .join("\n")
}

/// project-landing.ts:492.
pub fn under_login_home(home: &str, path: &str) -> String {
    match path.strip_prefix(GUEST_HOME) {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("{}{rest}", home.trim_end_matches('/')),
        _ => path.to_string(),
    }
}

/// project-landing.ts:500.
pub fn free_folder_script(home: &str, name: &str) -> String {
    let at = shell_quote(&format!("{}/{name}", home.trim_end_matches('/')));
    let tails: Vec<String> = std::iter::once("''".to_string()).chain((0..NAMES_TRIED - 1).map(|i| format!("-{}", i + 2))).collect();
    let taken = shell_quote(&format!("every name from {name} to {name}-{NAMES_TRIED} is taken in {home}"));
    format!(
        "for n in {}; do mkdir {at}\"$n\" 2>/dev/null && {{ printf '%s\\n' {at}\"$n\"; exit 0; }}; done; echo {taken} >&2; exit 1",
        tails.join(" ")
    )
}

/// The last line of `out` trimmed, `None` where it is empty. project-landing.ts:636.
pub fn last_line(out: &str) -> Option<String> {
    let line = js_trim(js_trim(out).rsplit('\n').next().unwrap_or(""));
    (!line.is_empty()).then(|| line.to_string())
}
