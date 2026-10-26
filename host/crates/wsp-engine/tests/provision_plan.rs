// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/provision.test.ts at a4033dc34, the half that reads what a computer already satisfies
//! and what the plan's scripts say: the presence read run under a real bash, the planner's rows and the PATH lines of
//! the job's scripts. The run on the computer itself is tests/provision_step.rs.
mod common;

use wsp_engine::golden_seam::{agent_installs_for, tool_installs_for};

use std::sync::{Arc, Mutex};

use common::{ok, res, ExecFn, FakeMachine, PutFn, RunFn};
use wsp_engine::errors::Result;
use wsp_engine::golden_seam::{free_kb_cmd, ToolInstall, BASE_VERSION_LINES, TOOLS_PATH};
use wsp_engine::machine::ExecResult;
use wsp_engine::provision::{new_setup_run, provision_step, EngineStep, ProvisionOn, ProvisionPlan, ProvisionStage};
use wsp_wire::views::place::{PlaceProvisionRow, PlaceProvisionRowOutcome};

const RECIPE_AT: &str = "2026-09-17T10:00:00.000Z";

type Calls = Arc<Mutex<Vec<String>>>;

/// golden-base.ts:111 `baseVersionsCmd`.
fn base_versions_cmd(path: &str) -> String {
    format!("export PATH={path}:$PATH\n{BASE_VERSION_LINES}")
}

/// Every floor row answering at the version its own row pins, so the floor installs nothing and the run under test is
/// the recipe's rows alone, and an ssh server of its own as a box added over ssh has.
fn floor_read() -> String {
    let mut lines: Vec<String> = Vec::new();
    for e in wsp_catalog::catalog::base_floor() {
        let version = match &e.major {
            None => "9.9.9".to_string(),
            Some(m) => format!("{}.0", m.version),
        };
        lines.push(format!("VERSION {}: {} {version}", e.bin, e.bin));
        for b in e.brings.iter().flatten() {
            lines.push(format!("VERSION {}: 9.9.9", b.bin));
        }
    }
    lines.push("VERSION sshd: OpenSSH_9.6p1".to_string());
    lines.join("\n")
}

/// A computer that answers every exec and every run from `answer` and records the order it was asked in.
fn box_machine(answer: impl Fn(&str) -> Option<ExecResult> + 'static, path: &str) -> (FakeMachine, Calls) {
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let answer = Arc::new(answer);
    let path = path.to_string();
    let exec_calls = calls.clone();
    let exec_answer = answer.clone();
    let exec: ExecFn = Arc::new(move |cmd, _| {
        exec_calls.lock().unwrap().push(cmd.clone());
        let said = exec_answer(&cmd);
        let path = path.clone();
        Box::pin(async move {
            if let Some(said) = said {
                return Ok(said);
            }
            if cmd == free_kb_cmd() {
                return Ok(res(0, "9000000\n", ""));
            }
            if cmd == base_versions_cmd(&path) {
                return Ok(res(0, &format!("{}\n", floor_read()), ""));
            }
            if cmd.contains("echo WSP_CTX") {
                return Ok(res(0, "WSP_CTX\nAGENT claude\nWSP_CTX_END\n", ""));
            }
            Ok(ok())
        })
    });
    let run_calls = calls.clone();
    let run: RunFn = Arc::new(move |script, _| {
        run_calls.lock().unwrap().push(script.clone());
        let said = answer(&script);
        Box::pin(async move { Ok(said.unwrap_or_else(ok)) })
    });
    let put: PutFn = Arc::new(|_, _| Box::pin(async { Ok(None) }));
    (FakeMachine { id: "spoo".to_string(), exec, run: Some(run), download_url: None, upload_url: None, put_bytes: Some(put) }, calls)
}

fn no_answer(_: &str) -> Option<ExecResult> {
    None
}

fn step(id: &str, label: &str) -> ToolInstall {
    ToolInstall {
        id: id.to_string(),
        label: label.to_string(),
        manager: "script".to_string(),
        cmd: format!("install {id}"),
        ..Default::default()
    }
}

fn plan_of(steps: Vec<ToolInstall>) -> ProvisionPlan {
    ProvisionPlan { recipe_at: RECIPE_AT.to_string(), path: TOOLS_PATH.to_string(), steps, ..Default::default() }
}

const ENGINE_STEPS: [EngineStep; 8] = [
    EngineStep::Floor,
    EngineStep::Agents,
    EngineStep::Clis,
    EngineStep::Mcp,
    EngineStep::Skills,
    EngineStep::Plugins,
    EngineStep::Configs,
    EngineStep::Context,
];

/// The engine's steps of a setup in the order the job runs them, one run carried through all of them.
async fn provision_box(
    machine: &FakeMachine,
    plan: &ProvisionPlan,
    stage: ProvisionStage,
    on: &ProvisionOn,
) -> Result<Vec<PlaceProvisionRow>> {
    let run = new_setup_run();
    let mut rows = Vec::new();
    for step in ENGINE_STEPS {
        rows.extend(provision_step(machine, plan, step, run.clone(), stage.clone(), on).await?);
    }
    Ok(rows)
}

/// The login the computer's own agent runs as, which is where the agents' folders are there.
fn on() -> ProvisionOn {
    ProvisionOn { home: "/root".to_string(), held: None, stores: None }
}

fn quiet() -> ProvisionStage {
    Arc::new(|_, _, _| {})
}

fn word(outcome: PlaceProvisionRowOutcome) -> &'static str {
    match outcome {
        PlaceProvisionRowOutcome::Installed => "installed",
        PlaceProvisionRowOutcome::Present => "present",
        PlaceProvisionRowOutcome::Failed => "failed",
        PlaceProvisionRowOutcome::Skipped => "skipped",
    }
}

/// The rows a run answered, as the assertions read them.
fn outcomes(rows: &[PlaceProvisionRow]) -> Vec<(String, String)> {
    rows.iter().map(|r| (r.id.clone(), word(r.outcome).to_string())).collect()
}

use std::collections::BTreeSet;
use std::path::Path;

use indexmap::IndexMap;

use wsp_engine::golden_seam::{base_installs, AgentsPlan, GoldenImport, PinRead};
use wsp_engine::provision::{present_by_what_waits, present_elsewhere, present_steps, provision_plan_of, PresentRead, Versions};
use wsp_wire::types::daemon_contract::{HOMEBREW_PREFIX, PNPM_HOME, TOOL_PREFIX};
use wsp_wire::views::place::{PlaceApplied, PlaceSetup, PlaceSetupState};
use wsp_words::views::place_setup_words::{setup_lines, setup_word};

const PROFILE_PATH_FILE: &str = "/etc/profile.d/wsp-golden.sh";
const READS_PER_EXEC: usize = 8;

fn import_of(agents: AgentsPlan, tools: Vec<ToolInstall>) -> GoldenImport {
    GoldenImport { agents: agents.installs, node: agents.node, tools, ..Default::default() }
}

fn pin(read: &str, fixed: bool, words: &str) -> Option<PinRead> {
    Some(PinRead { read: Some(read.to_string()), fixed, words: words.to_string() })
}

fn stub(path: &Path, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, script).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A folder on this computer with one script per command a test wants to answer, so a presence read run under bash
/// finds the commands it looks for.
fn scratch(commands: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::Builder::new().prefix("wsp-present-").tempdir().unwrap();
    for (name, body) in commands {
        stub(&dir.path().join(name), &format!("#!/bin/sh\n{body}\n"));
    }
    dir
}

/// A computer whose execs a real bash answers, with the scratch folder ahead of the tools PATH the read exports: the
/// read is the thing under test, so a shell runs it.
fn shell_machine(dir: &Path) -> FakeMachine {
    let dir = dir.display().to_string();
    FakeMachine::new(Arc::new(move |cmd, _| {
        let cmd = cmd.replace(TOOLS_PATH, &format!("{dir}:{TOOLS_PATH}"));
        Box::pin(async move { Ok(common::bash(&cmd, None, None).await) })
    }))
}

async fn present_of(machine: &FakeMachine, steps: &[ToolInstall]) -> IndexMap<String, PresentRead> {
    present_steps(machine, steps, TOOLS_PATH, None, Versions::Asked).await.unwrap()
}

fn sorted_keys(present: &IndexMap<String, PresentRead>) -> Vec<String> {
    let mut keys: Vec<String> = present.keys().cloned().collect();
    keys.sort();
    keys
}

/// The place a presence marker line gives a step in its read: `/wsp-present[^0-9]*([0-9]+)/`.
fn mark_at(line: &str) -> Option<String> {
    let after = &line[line.find("wsp-present")? + "wsp-present".len()..];
    let digits: String = after.trim_start_matches(|c: char| !c.is_ascii_digit()).chars().take_while(|c| c.is_ascii_digit()).collect();
    (!digits.is_empty()).then_some(digits)
}

/// Every marker a presence read asks for, as a computer that answers every one of them prints them.
fn every_mark(cmd: &str) -> String {
    cmd.split('\n').filter_map(mark_at).map(|at| format!("wsp-present {at}")).collect::<Vec<_>>().join("\n")
}

/// A computer that answers the presence read for the commands named in it and for no others, so a test says which
/// rows the box has rather than which place they fell in a page.
fn marks_for(cmd: &str, wanted: &[&str]) -> String {
    cmd.split('\n')
        .filter(|line| wanted.iter().any(|w| line.contains(w)))
        .filter_map(mark_at)
        .map(|at| format!("wsp-present {at}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn reads_present(cmd: &str, stdout: String) -> Option<ExecResult> {
    cmd.contains("wsp-present").then(|| res(0, &stdout, ""))
}

fn ended() -> PlaceSetup {
    PlaceSetup {
        state: PlaceSetupState::Done,
        add_id: "a_1".to_string(),
        started_at: "x".to_string(),
        finished_at: None,
        steps: vec![],
        waiting: vec![],
        said: None,
    }
}

fn applied(rows: Vec<PlaceProvisionRow>) -> PlaceApplied {
    PlaceApplied { hash: "h".to_string(), at: "x".to_string(), rows, items: None }
}

/// Every PATH one script runs under. The line that writes the login shell's PATH into a file on that computer is not
/// one of them: what a person's own shell there looks along is not what this job resolves a command through, and it
/// still names where the recipe installs.
fn exported_paths(script: &str) -> Vec<String> {
    script
        .split('\n')
        .filter(|line| !line.contains(PROFILE_PATH_FILE))
        .flat_map(|line| {
            line.match_indices("export PATH=")
                .map(|(at, mark)| line[at + mark.len()..].split_whitespace().next().unwrap_or("").to_string())
                .filter(|v| !v.is_empty())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn tool(steps: &[ToolInstall], id: &str) -> ToolInstall {
    steps.iter().find(|t| t.id == id).unwrap_or_else(|| panic!("no step {id}")).clone()
}

fn s(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

// TS: packages/engine/test/provision.test.ts:145
// "is the step whose check passes, the step whose command answers with no version asked, and the step reading the version it asks for"
#[tokio::test(flavor = "multi_thread")]
async fn is_the_step_whose_check_passes_the_step_whose_command_answers_and_the_step_reading_the_version() {
    let dir = scratch(&[("ace", "exit 0"), ("bee", "exit 0"), ("cee", "echo 1.2.3"), ("dee", "echo 1.0.9")]);
    let npm = |id: &str, label: &str, bin: &str, asks: &str| ToolInstall {
        manager: "npm".to_string(),
        bin: Some(bin.to_string()),
        asks: Some(asks.to_string()),
        pin: pin(&format!("{bin} --version"), true, "as an npm global"),
        ..step(id, label)
    };
    let steps = vec![
        ToolInstall { check: Some("ace --version".to_string()), ..step("tools/custom/ace", "Ace") },
        ToolInstall { manager: "apt".to_string(), bin: Some("bee".to_string()), ..step("tools/apt/bee", "Bee") },
        npm("agents/cee", "Cee", "cee", "1.2.3"),
        npm("agents/dee", "Dee", "dee", "1.2.3"),
        npm("tools/npm/eff", "Eff", "eff", "1.0.0"),
        step("agents/node", "Node 22.23.2"),
    ];
    let present = present_of(&shell_machine(dir.path()), &steps).await;
    // Dee answers, but at an older version than the recipe asks for, which is the drift the recipe is there to fix;
    // Eff is not on the computer at all; the node step names nothing that can be read and is never present.
    assert_eq!(sorted_keys(&present), s(&["agents/cee", "tools/apt/bee", "tools/custom/ace"]));
    assert_eq!(present.get("tools/apt/bee"), Some(&PresentRead { path: Some(dir.path().join("bee").display().to_string()) }));
    assert_eq!(present.get("tools/custom/ace"), Some(&PresentRead::default()));
}

// TS: packages/engine/test/provision.test.ts:165
// "puts bubblewrap on by apt in the agents step once Codex is on, since Codex's sandbox runs under it on Linux"
#[test]
fn puts_bubblewrap_on_by_apt_in_the_agents_step_once_codex_is_on() {
    let plan_for = |ids: &[&str]| {
        let installs = agent_installs_for(ids);
        provision_plan_of(&import_of(installs, vec![]), RECIPE_AT, TOOLS_PATH, None, None)
    };
    let plan = plan_for(&["agents/claude", "agents/codex"]);
    let agents = &plan.steps[..plan.agents];
    assert_eq!(
        agents.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["agents/node", "agents/claude", "agents/codex", "agents/codex/bubblewrap", "agents/codex/bwrap-apparmor"]
    );
    let bubblewrap = agents.iter().find(|s| s.id == "agents/codex/bubblewrap").unwrap();
    assert_eq!(bubblewrap.manager, "apt");
    assert_eq!(bubblewrap.bin.as_deref(), Some("bwrap"));
    assert_eq!(bubblewrap.after.as_deref(), Some("agents/codex"));
    assert_eq!(bubblewrap.label, "bubblewrap");
    assert!(bubblewrap.cmd.contains("apt-get install -y -qq bubblewrap"));
    assert!(!plan_for(&["agents/claude"]).steps.iter().any(|s| s.id == "agents/codex/bubblewrap"));
}

// TS: packages/engine/test/provision.test.ts:176
// "loads Ubuntu's bwrap profile once bubblewrap is on, so a turn as a user other than root gets Codex's sandbox"
#[test]
fn loads_ubuntus_bwrap_profile_once_bubblewrap_is_on() {
    let plan = provision_plan_of(&import_of(agent_installs_for(&["agents/codex"]), vec![]), RECIPE_AT, TOOLS_PATH, None, None);
    let agents = &plan.steps[..plan.agents];
    assert_eq!(
        agents.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["agents/node", "agents/codex", "agents/codex/bubblewrap", "agents/codex/bwrap-apparmor"]
    );
    let profile = agents.last().unwrap();
    assert_eq!(profile.label, "bwrap AppArmor profile");
    assert_eq!(profile.after.as_deref(), Some("agents/codex/bubblewrap"));
    let check = profile.check.as_deref().unwrap();
    // The one file out of apparmor-profiles, never the package, whose install loads twenty unrelated enforce profiles.
    assert!(profile.cmd.contains("apt-get download -qq -o APT::Sandbox::User=root apparmor-profiles"));
    assert!(!profile.cmd.contains("apt-get install"));
    assert!(profile.cmd.contains(r#"apparmor_parser -r -W "$f""#));
    assert!(check.contains("bwrap --ro-bind / / --unshare-net true"));
    // A failed row is never undone, so a load that leaves bwrap failing takes the profile off itself.
    assert_eq!(
        profile.cmd.split('\n').next_back().unwrap(),
        format!(r#"({check}) >&2 || {{ apparmor_parser -R "$f" 2>/dev/null; rm -f "$f"; exit 1; }}"#)
    );
}

// TS: packages/engine/test/provision.test.ts:192
// "reads the bwrap profile row present where a user other than root can run bwrap, and says bwrap's own line where not"
#[tokio::test(flavor = "multi_thread")]
async fn reads_the_bwrap_profile_row_present_where_a_user_other_than_root_can_run_bwrap() {
    let plan = provision_plan_of(&import_of(agent_installs_for(&["agents/codex"]), vec![]), RECIPE_AT, TOOLS_PATH, None, None);
    let step = tool(&plan.steps, "agents/codex/bwrap-apparmor");
    let works = scratch(&[("setpriv", "exit 0")]);
    assert_eq!(sorted_keys(&present_of(&shell_machine(works.path()), std::slice::from_ref(&step)).await), ["agents/codex/bwrap-apparmor"]);
    let stopped = scratch(&[("setpriv", "echo 'bwrap: loopback: Failed RTM_NEWADDR: Operation not permitted' >&2; exit 1")]);
    assert_eq!(present_of(&shell_machine(stopped.path()), std::slice::from_ref(&step)).await.len(), 0);
    let said = common::bash(&format!("PATH={}:$PATH\n{}", stopped.path().display(), step.check.as_deref().unwrap()), None, None).await;
    assert_eq!(said.exit_code, 1.into());
    assert_eq!(said.stdout.trim(), "bwrap fails for users other than root: bwrap: loopback: Failed RTM_NEWADDR: Operation not permitted");
}

// TS: packages/engine/test/provision.test.ts:204
// "is every step of a plan the computer already answers, so a second run of that recipe installs nothing"
#[tokio::test(flavor = "multi_thread")]
async fn is_every_step_of_a_plan_the_computer_already_answers_so_a_second_run_installs_nothing() {
    // The recipe spoo gets: the node step, Claude Code by its own installer, Codex by npm at the version the
    // catalog pins, Homebrew and its toolchain, the shared step and two formulae.
    let import = GoldenImport {
        agents: agent_installs_for(&["agents/claude", "agents/codex"]).installs,
        node: agent_installs_for(&["agents/codex"]).node,
        tools: tool_installs_for(&["tools/brew/gh", "tools/brew/yq"], TOOLS_PATH, None),
        ..Default::default()
    };
    let plan = provision_plan_of(&import, RECIPE_AT, TOOLS_PATH, None, None);
    assert!(plan.steps.len() > 6);
    for s in &plan.steps {
        assert!(s.check.is_some() || s.bin.is_some(), "{}", s.id);
    }
    let (machine, _) = box_machine(|cmd| reads_present(cmd, every_mark(cmd)), TOOLS_PATH);
    let rows = provision_box(&machine, &plan, quiet(), &on()).await.unwrap();
    assert_eq!(rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), plan.steps.iter().map(|s| s.id.clone()).collect::<Vec<_>>());
    assert!(rows.iter().all(|r| word(r.outcome) == "present"));
    assert_eq!(setup_word(Some(&ended()), Some(&applied(rows.clone()))), format!("{} tools ready", rows.len()));
    assert_eq!(
        setup_lines("spoo", &ended(), Some(&applied(rows.clone())), None)[0],
        format!("spoo: nothing installed, {} already there", rows.len())
    );
}

// TS: packages/engine/test/provision.test.ts:229
// "is a step whose only read is its version: the version it asks for, or any version at all where it asks for none"
#[tokio::test(flavor = "multi_thread")]
async fn is_a_step_whose_only_read_is_its_version() {
    let dir = scratch(&[
        ("bun", "echo 1.4.0"),
        ("wrangler", "echo 4.105.0"),
        ("vite", "echo 7.1.0"),
        ("quiet", "exit 0"),
        ("cloudflared", "echo 2026.9.1"),
    ]);
    // The three npm rows of spoo's recipe carry no command of their own and no check: what a person asked for is a
    // package at a version, and the road's own version read is the whole of what the computer can be asked.
    let npm = |name: &str, label: &str, asks: &str| ToolInstall {
        manager: "npm".to_string(),
        asks: Some(asks.to_string()),
        pin: pin(name, true, "as an npm global"),
        ..step(&format!("tools/npm/{name}"), label)
    };
    let steps = vec![
        npm("bun", "bun@1.4.0", "1.4.0"),
        npm("wrangler", "wrangler@4.106.0", "4.106.0"),
        npm("vite", "vite@7.0.0", "7.0.0"),
        npm("agent-browser", "agent-browser@0.31.1", "0.31.1"),
        ToolInstall { manager: "npm".to_string(), pin: pin("quiet", false, "as an npm global"), ..step("tools/npm/quiet", "quiet") },
        ToolInstall {
            manager: "brew".to_string(),
            pin: pin("cloudflared", false, "with Homebrew"),
            ..step("tools/brew/cloudflared", "cloudflared")
        },
    ];
    let present = present_of(&shell_machine(dir.path()), &steps).await;
    // bun answers at the version the recipe pins; wrangler answers at an older one and vite at a newer one, which is
    // the drift the recipe is there to fix, since only an agent ahead of its pin is kept; agent-browser is not on the
    // computer; quiet prints nothing, so nothing says it is there; cloudflared's road installs whatever it serves that
    // day, so any version it prints is that row on the box.
    assert_eq!(sorted_keys(&present), s(&["tools/brew/cloudflared", "tools/npm/bun"]));
}

// TS: packages/engine/test/provision.test.ts:248
// "asks a formula's row by the prefix's own link, which runs no brew at all, and leaves brew's list as the check after an install"
#[tokio::test(flavor = "multi_thread")]
async fn asks_a_formulas_row_by_the_prefixs_own_link_which_runs_no_brew_at_all() {
    let names: Vec<String> =
        ["bat", "beads", "btop", "dust", "eza", "fzf", "gum", "yq"].iter().map(|n| format!("tools/brew/{n}")).collect();
    let ids: Vec<&str> = names.iter().map(String::as_str).collect();
    let brew: Vec<ToolInstall> =
        tool_installs_for(&ids, TOOLS_PATH, None).into_iter().filter(|s| s.id.starts_with("tools/brew/")).collect();
    assert_eq!(brew.len(), READS_PER_EXEC);
    // The check and the version read are one command, the brew list under su, and neither is the presence question:
    // the prefix keeps a link per formula it installed and that link is the answer.
    for s in &brew {
        let check = s.check.as_deref().unwrap();
        assert!(s.pin.as_ref().unwrap().read.as_deref().unwrap().starts_with(check), "{}", s.id);
    }
    let (machine, calls) = box_machine(|cmd| reads_present(cmd, every_mark(cmd)), TOOLS_PATH);
    let present = present_of(&machine, &brew).await;
    assert_eq!(present.len(), brew.len());
    let reads: Vec<String> = calls.lock().unwrap().iter().filter(|c| c.contains("wsp-present")).cloned().collect();
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0].matches("list --versions").count(), 0);
    assert_eq!(reads[0].matches(&format!("test -e {HOMEBREW_PREFIX}/opt/")).count(), brew.len());
    assert!(!reads[0].contains("linuxbrew -c"));
}

// TS: packages/engine/test/provision.test.ts:265
// "says where a command answered from when it is outside the directories its own road links into, and says nothing when it is inside them"
#[tokio::test(flavor = "multi_thread")]
async fn says_where_a_command_answered_from_when_it_is_outside_the_directories_its_own_road_links_into() {
    // The row this rule was filed on: on spoo the recipe's node tick is planned on the catalog's own installer,
    // which links into /usr/local/bin, and node answers from /usr/bin, where apt put it. Both readings are true of
    // what they read, so the row says which path answered rather than reading as the row its own road installed.
    let step = tool(&tool_installs_for(&["tools/brew/node"], TOOLS_PATH, None), "tools/brew/node");
    let dir = scratch(&[("node", "echo v22.0.0")]);
    let answered = dir.path().join("node").display().to_string();
    let read = present_of(&shell_machine(dir.path()), std::slice::from_ref(&step)).await;
    assert_eq!(read.get(&step.id), Some(&PresentRead { path: Some(answered.clone()) }));
    assert_eq!(
        present_elsewhere(&step, read.get(&step.id)),
        Some(format!("node answers from {answered}, outside where its own installer puts it (/usr/local/bin)"))
    );
    // The same command answering from inside its road's own directories says nothing at all.
    let inside = ToolInstall { bins: Some(vec![dir.path().display().to_string()]), ..step.clone() };
    assert_eq!(present_elsewhere(&inside, read.get(&step.id)), None);
    // And a step whose road names no directories says nothing either, rather than a note about every path.
    let none = ToolInstall { bins: None, ..step.clone() };
    assert_eq!(present_elsewhere(&none, read.get(&step.id)), None);
}

// TS: packages/engine/test/provision.test.ts:281
// "keeps that note on a present row and drops the landing's own, which says what the outcome already says"
#[tokio::test(flavor = "multi_thread")]
async fn keeps_that_note_on_a_present_row_and_drops_the_landings_own() {
    let step = tool(&tool_installs_for(&["tools/brew/node"], TOOLS_PATH, None), "tools/brew/node");
    let (machine, _) = box_machine(|cmd| reads_present(cmd, "wsp-present 0 /usr/bin/node\n".to_string()), TOOLS_PATH);
    let rows = provision_box(&machine, &plan_of(vec![step.clone()]), quiet(), &on()).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (word(rows[0].outcome), rows[0].note.clone()),
        ("present", Some("node answers from /usr/bin/node, outside where its own installer puts it (/usr/local/bin)".to_string()))
    );
    // A present row whose command answered from its own road's directory carries no note: the landing lands every
    // present step with "already on the machine", which is what the outcome word says.
    let (inside, _) = box_machine(|cmd| reads_present(cmd, "wsp-present 0 /usr/local/bin/node\n".to_string()), TOOLS_PATH);
    let quiet_rows = provision_box(&inside, &plan_of(vec![step.clone()]), quiet(), &on()).await.unwrap();
    assert_eq!((word(quiet_rows[0].outcome), quiet_rows[0].note.clone()), ("present", None));
    // A marker with no path still reads present, which is every row whose read asked for no command.
    let (bare, _) = box_machine(|cmd| reads_present(cmd, "wsp-present 0\n".to_string()), TOOLS_PATH);
    let said = provision_box(&bare, &plan_of(vec![step]), quiet(), &on()).await.unwrap();
    assert_eq!((word(said[0].outcome), said[0].note.clone()), ("present", None));
}

// TS: packages/engine/test/provision.test.ts:298
// "is a step nothing can be asked about when every step waiting on it is already there, and not when one of them is missing"
#[tokio::test(flavor = "multi_thread")]
async fn is_a_step_nothing_can_be_asked_about_when_every_step_waiting_on_it_is_already_there() {
    let apt = |id: &str, label: &str, bin: &str| ToolInstall {
        manager: "apt".to_string(),
        bin: Some(bin.to_string()),
        after: Some("tools/apt-index".to_string()),
        ..step(id, label)
    };
    let steps = vec![
        ToolInstall { manager: "apt".to_string(), ..step("tools/apt-index", "apt index") },
        apt("tools/apt/ffmpeg", "ffmpeg", "ffmpeg"),
        apt("tools/apt/ruby", "Ruby 3.1 with bundler", "ruby"),
    ];
    // An index refresh answers no read of its own: what it was for is the rows behind it, and a box that has all of
    // them has nothing for it to do.
    let (machine, calls) = box_machine(|cmd| reads_present(cmd, marks_for(cmd, &["ffmpeg", "ruby"])), TOOLS_PATH);
    let rows = provision_box(&machine, &plan_of(steps.clone()), quiet(), &on()).await.unwrap();
    assert_eq!(
        outcomes(&rows),
        [("tools/apt-index", "present"), ("tools/apt/ffmpeg", "present"), ("tools/apt/ruby", "present")]
            .map(|(a, b)| (a.to_string(), b.to_string()))
    );
    assert!(!calls.lock().unwrap().iter().any(|c| c.contains("install tools/apt-index")));

    // One of them is not on the box: the index runs, since the row behind it needs it.
    let (one, one_calls) = box_machine(|cmd| reads_present(cmd, marks_for(cmd, &["ffmpeg"])), TOOLS_PATH);
    let again = provision_box(&one, &plan_of(steps), quiet(), &on()).await.unwrap();
    assert_eq!(
        outcomes(&again),
        [("tools/apt-index", "installed"), ("tools/apt/ffmpeg", "present"), ("tools/apt/ruby", "installed")]
            .map(|(a, b)| (a.to_string(), b.to_string()))
    );
    assert!(one_calls.lock().unwrap().iter().any(|c| c.contains("install tools/apt-index")));
}

// TS: packages/engine/test/provision.test.ts:326
// "is the apt index of a real plan and no other step of it, since every other step says a command, a check or a version"
#[tokio::test(flavor = "multi_thread")]
async fn is_the_apt_index_of_a_real_plan_and_no_other_step_of_it() {
    // Two apt rows of the catalog: the plan reads the index once, before the first row that waits on it.
    let import =
        GoldenImport { tools: tool_installs_for(&["tools/catalog/ffmpeg", "tools/catalog/ruby"], TOOLS_PATH, None), ..Default::default() };
    let plan = provision_plan_of(&import, RECIPE_AT, TOOLS_PATH, None, None);
    // Through the rule itself rather than a second copy of it here: on a computer where every step's own read answers,
    // the apt index is the one step the dependents rule has anything to say about.
    let by_what_waits = present_by_what_waits(&plan.steps, &plan.steps.iter().map(|s| s.id.clone()).collect::<BTreeSet<_>>());
    let labels: Vec<String> = by_what_waits.iter().map(|id| plan.steps.iter().find(|s| &s.id == id).unwrap().label.clone()).collect();
    assert_eq!(labels, ["apt index"]);
    let (machine, calls) = box_machine(|cmd| reads_present(cmd, marks_for(cmd, &["ffmpeg", "ruby", "bundle", "gem"])), TOOLS_PATH);
    let rows = provision_box(&machine, &plan, quiet(), &on()).await.unwrap();
    assert!(rows.iter().all(|r| word(r.outcome) == "present"));
    assert!(!calls.lock().unwrap().iter().any(|c| c.contains("apt-get update")));
}

// TS: packages/engine/test/provision.test.ts:339
// "is nothing at all when the computer answers nothing, and asks for nothing when a step says nothing that can be read"
#[tokio::test(flavor = "multi_thread")]
async fn is_nothing_at_all_when_the_computer_answers_nothing() {
    let dir = scratch(&[]);
    let bee = ToolInstall { bin: Some("bee".to_string()), ..step("tools/apt/bee", "Bee") };
    assert!(present_of(&shell_machine(dir.path()), &[bee]).await.is_empty());
    let (machine, calls) = box_machine(no_answer, TOOLS_PATH);
    assert!(present_of(&machine, &[step("agents/node", "Node")]).await.is_empty());
    assert_eq!(*calls.lock().unwrap(), Vec::<String>::new());
}

// TS: packages/engine/test/provision.test.ts:677
// "is the tools PATH for the image build, which is root's own machine and shares its home with nobody"
#[test]
fn is_the_tools_path_for_the_image_build_which_is_roots_own_machine() {
    let import = GoldenImport { tools: tool_installs_for(&["tools/npm/turbo"], TOOLS_PATH, None), ..Default::default() };
    let plan = provision_plan_of(&import, RECIPE_AT, TOOLS_PATH, None, None);
    assert_eq!(plan.path, TOOLS_PATH);
    assert!(plan.steps.iter().flat_map(|s| exported_paths(&s.cmd)).any(|p| p == TOOLS_PATH));
}

// TS: packages/engine/test/provision.test.ts:683
// "keeps a binary planted under that home out of what a row runs, under a real shell"
#[tokio::test(flavor = "multi_thread")]
async fn keeps_a_binary_planted_under_that_home_out_of_what_a_row_runs_under_a_real_shell() {
    let home = tempfile::Builder::new().prefix("wsp-planted-").tempdir().unwrap();
    let bin = home.path().join(".local/bin");
    std::fs::create_dir_all(&bin).unwrap();
    let marker = home.path().join("uv-ran");
    stub(&bin.join("uv"), &format!("#!/bin/sh\ntouch {}\n", marker.display()));
    // The tools PATH as it reads on a computer somebody owns: that home's own directory first, the system's behind it;
    // the probe list is the same with every directory under the home taken out.
    let tools = format!("{}:/usr/bin:/bin", bin.display());
    let under = format!("{}/", home.path().display());
    let probe = tools.split(':').filter(|dir| !dir.starts_with(&under)).collect::<Vec<_>>().join(":");
    let uv_row =
        |path: &str| tool_installs_for(&["tools/uv/ruff"], path, None).into_iter().find(|t| t.cmd.contains("uv tool install")).unwrap().cmd;
    common::bash(&uv_row(&probe), None, None).await;
    assert!(!marker.exists(), "a row of the job ran the binary planted under the home");
    // The same row on a list that does name that home runs it, which is what the probe list is here to stop.
    common::bash(&uv_row(&tools), None, None).await;
    assert!(marker.exists());
}

// TS: packages/engine/test/provision.test.ts:734
// "carries no knob but pnpm's own on the image build's plan, whose scripts read as they did"
#[test]
fn carries_no_knob_but_pnpms_own_on_the_image_builds_plan() {
    let import = GoldenImport {
        agents: agent_installs_for(&["agents/claude"]).installs,
        tools: tool_installs_for(&["tools/uv/ruff", "tools/cargo/tokei"], TOOLS_PATH, None),
        ..Default::default()
    };
    let plan = provision_plan_of(&import, RECIPE_AT, TOOLS_PATH, None, None);
    assert_eq!(plan.prefix, None);
    let ids: Vec<&str> = plan.steps.iter().map(|t| t.id.as_str()).collect();
    assert!(ids.contains(&"tools/uv/ruff") && ids.contains(&"tools/cargo/tokei"));
    // An image takes no prefix, so every script of its job exports the one line it always did and no step of it names
    // the prefix at all.
    for step in plan.steps.iter().cloned().chain(base_installs(&BTreeSet::new(), TOOLS_PATH, None, &BTreeSet::new())) {
        let lines: Vec<&str> = step.cmd.split('\n').filter(|l| l.starts_with("export PATH=") && l.contains(TOOLS_PATH)).collect();
        assert_eq!(lines, [format!("export PATH={TOOLS_PATH} PNPM_HOME={PNPM_HOME}")], "{}", step.id);
        assert!(!step.cmd.contains(TOOL_PREFIX), "{}", step.id);
    }
}

// TS: packages/engine/test/provision.test.ts:747
// "links what a manager with no folder knob left in its own folder, under a real shell"
#[tokio::test(flavor = "multi_thread")]
async fn links_what_a_manager_with_no_folder_knob_left_in_its_own_folder_under_a_real_shell() {
    let dir = tempfile::Builder::new().prefix("wsp-link-").tempdir().unwrap();
    // The folder the daemon's fixed PATH holds, which is /usr/local/bin on a box and this one under a test.
    let local = dir.path().join("local-bin");
    std::fs::create_dir(&local).unwrap();
    // A cargo that writes its command where cargo does, into the bin folder of the home the job told it to keep.
    let system = dir.path().join("system");
    std::fs::create_dir(&system).unwrap();
    stub(
        &system.join("cargo"),
        "#!/bin/sh\n[ -n \"$CARGO_HOME\" ] || { echo \"the job told cargo no home\" >&2; exit 1; }\nmkdir -p \"$CARGO_HOME/bin\"\nprintf '#!/bin/sh\\necho tokei\\n' > \"$CARGO_HOME/bin/tokei\"\nchmod +x \"$CARGO_HOME/bin/tokei\"\n",
    );
    let prefix = dir.path().display().to_string();
    let step = tool_installs_for(&["tools/cargo/tokei"], &format!("{}:/usr/bin:/bin", system.display()), Some(&prefix))
        .into_iter()
        .find(|t| t.cmd.contains("cargo install"))
        .unwrap();
    let res = common::bash(&step.cmd.replace("/usr/local/bin", &local.display().to_string()), None, None).await;
    assert_eq!(res.exit_code, 0.into(), "{}", res.stderr);
    // cargo takes no knob for where its commands go, so what it left under the prefix answers from that folder.
    assert!(std::fs::symlink_metadata(local.join("tokei")).unwrap().file_type().is_symlink());
    assert_eq!(std::fs::canonicalize(local.join("tokei")).unwrap(), std::fs::canonicalize(dir.path().join("cargo/bin/tokei")).unwrap());
}

// TS: packages/engine/test/provision.test.ts:765
// "reaches uv itself with the prefix under a real shell, and never the uv planted under that home"
#[tokio::test(flavor = "multi_thread")]
async fn reaches_uv_itself_with_the_prefix_under_a_real_shell_and_never_the_uv_planted_under_that_home() {
    let home = tempfile::Builder::new().prefix("wsp-knobs-").tempdir().unwrap();
    let planted = home.path().join(".local/bin");
    std::fs::create_dir_all(&planted).unwrap();
    let ran = home.path().join("planted-ran");
    stub(&planted.join("uv"), &format!("#!/bin/sh\ntouch {}\n", ran.display()));
    // A uv where the job's own list looks, which writes down what the job told it.
    let system = home.path().join("system");
    std::fs::create_dir(&system).unwrap();
    let said = home.path().join("said");
    stub(
        &system.join("uv"),
        &format!("#!/bin/sh\nprintf '%s %s %s\\n' \"$UV_TOOL_DIR\" \"$UV_TOOL_BIN_DIR\" \"$UV_PYTHON_INSTALL_DIR\" > {}\n", said.display()),
    );
    let step = tool_installs_for(&["tools/uv/ruff"], &format!("{}:/usr/bin:/bin", system.display()), Some(TOOL_PREFIX))
        .into_iter()
        .find(|t| t.cmd.contains("uv tool install"))
        .unwrap();
    let res = common::bash(&step.cmd, None, None).await;
    assert_eq!(res.exit_code, 0.into(), "{}", res.stderr);
    assert_eq!(std::fs::read_to_string(&said).unwrap().trim(), format!("{TOOL_PREFIX}/uv/tools /usr/local/bin {TOOL_PREFIX}/uv/python"));
    assert!(!ran.exists(), "a row of the job ran the binary planted under the home");
}
