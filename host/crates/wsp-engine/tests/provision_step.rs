// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/provision.test.ts at a4033dc34, the half that runs provision_step: the run on the
//! computer itself, the person's own files and their servers, the PATH every script of the job exports, a CLI's hook,
//! the CLIs a step beside the loop waits on and a plugin that asks; and the one provision_step test of
//! provision-floor.test.ts. What a computer already satisfies is tests/provision_plan.rs.
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
        rows.extend(provision_step(&common::handle(machine), plan, step, run.clone(), stage.clone(), on).await?);
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

use std::sync::atomic::{AtomicUsize, Ordering};

use indexmap::IndexMap;

use wsp_catalog::mcp::MCP_SERVERS_JSON;
use wsp_catalog::skills::PluginRoad;
use wsp_engine::golden_seam::{path_line, GoldenImport, McpAgentPlan, McpPlan, McpScope, PackFiles, PackedFiles, SkippedPath};
use wsp_engine::provision::{provision_counts_of, provision_plan_of, PlanFiles, PlanPlugin, PlanSkipped};
use wsp_engine::provision_files::{ProvisionLanding, SERVER_MARK};
use wsp_engine::vault::{tar_of, TarEntry};
use wsp_wire::types::daemon_contract::TOOL_PREFIX;
use wsp_wire::views::golden_image::MCP_ID_PREFIX;
use wsp_wire::views::place::{PlaceApplied, PlaceProvisionKind, PlaceSetup, PlaceSetupState, PlaceSetupStep};
use wsp_words::project_path::{place_provision_paths, probe_path};
use wsp_words::views::place::provision_count_word;
use wsp_words::views::place_setup_words::{
    provision_landed_line, provision_list_read_line, provision_packed_line, provision_servers_line, provision_shipped_line, setup_word,
    Packed, Shipped,
};

const PROFILE_PATH_FILE: &str = "/etc/profile.d/wsp-golden.sh";
/// exec-detached.ts:159 `OLD_APPEND_MARKS`.
// Adapted: no Rust counterpart in exec_detached.rs yet, which is empty; the TS value.
const OLD_APPEND_MARKS: &str = ".a[0-9a-f]*";

fn pairs(rows: &[(&str, &str)]) -> Vec<(String, String)> {
    rows.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

fn snapshot(calls: &Calls) -> Vec<String> {
    calls.lock().unwrap().clone()
}

/// A stage that keeps every line the job says.
fn said_into(seen: Arc<Mutex<Vec<String>>>) -> ProvisionStage {
    Arc::new(move |detail, _, _| seen.lock().unwrap().push(detail.to_string()))
}

/// A stage that keeps every row the job hands it as its outcome lands.
fn rows_into(rows: Arc<Mutex<Vec<PlaceProvisionRow>>>) -> ProvisionStage {
    Arc::new(move |_, _, row| {
        if let Some(row) = row {
            rows.lock().unwrap().push(row.clone());
        }
    })
}

/// A stage that keeps `index/of: label` of every row under way.
fn at_into(at: Arc<Mutex<Vec<Option<String>>>>) -> ProvisionStage {
    Arc::new(move |_, under, _| at.lock().unwrap().push(under.map(|u| format!("{}/{}: {}", u.index, u.of, u.label))))
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

fn why_tar() -> Vec<u8> {
    tar_of(&[TarEntry::File { path: ".claude-cfg/skills/why/SKILL.md".to_string(), mode: 0o644, content: b"why\n".to_vec() }]).unwrap()
}

fn pack_of(packed: PackedFiles) -> PackFiles {
    Arc::new(move |_| {
        let packed = packed.clone();
        Box::pin(async move { Ok(packed) })
    })
}

fn skills_land() -> ProvisionLanding {
    ProvisionLanding {
        id: "agents/claude".to_string(),
        label: "Claude Code".to_string(),
        dest: ".claude-cfg/skills".to_string(),
        once: false,
    }
}

/// A plan whose files and servers are named, with a pack this computer answers without reading a disk.
fn with_files_and_servers(steps: Vec<ToolInstall>) -> ProvisionPlan {
    let agents = steps.len();
    let pack: PackFiles = Arc::new(|_| Box::pin(async { Ok(PackedFiles { tar: why_tar(), bytes: 1, unpacked: 1, ..Default::default() }) }));
    ProvisionPlan {
        agents,
        files: Some(PlanFiles { lands: vec![skills_land()], pack }),
        mcp: Some(McpPlan {
            agents: vec![McpAgentPlan {
                id: "claude".to_string(),
                label: "Claude Code".to_string(),
                scopes: vec![McpScope {
                    files: vec!["/root/.claude-cfg/.claude.json".to_string()],
                    format: MCP_SERVERS_JSON,
                    project: None,
                    keep: vec!["github".to_string()],
                    drop: vec![],
                    travelled: None,
                    own: None,
                    folder: None,
                    trust: None,
                    edit: None,
                }],
                aside: vec![],
            }],
            guest_home: "/root".to_string(),
            rewrites: vec![],
            bin_dirs: vec![],
            tools: vec![],
        }),
        ..plan_of(steps)
    }
}

fn codex() -> ToolInstall {
    ToolInstall { bin: Some("codex".to_string()), ..step("agents/codex", "Codex") }
}

/// Every PATH one script runs under; the line that writes the login shell's PATH into a file is not one of them.
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

fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// `/^jq \(\d+\/\d+\)$/`.
fn is_jq_line(line: &str) -> bool {
    line.strip_prefix("jq (")
        .and_then(|rest| rest.strip_suffix(')'))
        .is_some_and(|inner| inner.split_once('/').is_some_and(|(a, b)| digits(a) && digits(b)))
}

/// `/^\d+ installed.*free$/`.
fn is_tally_line(line: &str) -> bool {
    let rest = line.trim_start_matches(|c: char| c.is_ascii_digit());
    rest.len() < line.len() && rest.len() >= " installed".len() + "free".len() && rest.starts_with(" installed") && rest.ends_with("free")
}

/// `/^install -D -m 0755 \S+ (\S+)\/claude'?$/m`: the folder a harness install lands its command in.
fn harness_dir(cmd: &str) -> Option<String> {
    cmd.split('\n').find_map(|line| {
        let mut tokens = line.strip_prefix("install -D -m 0755 ")?.split(' ');
        let (_from, to) = (tokens.next()?, tokens.next()?);
        if tokens.next().is_some() {
            return None;
        }
        to.strip_suffix('\'').unwrap_or(to).strip_suffix("/claude").map(str::to_string)
    })
}

/// The floor's read with one row's version left out, so that row's own step runs and its script is in what the job
/// sent: a computer that answers every floor row installs none of them.
fn floor_without(bin: &str) -> String {
    floor_read().split('\n').filter(|l| !l.starts_with(&format!("VERSION {bin}:"))).collect::<Vec<_>>().join("\n")
}

/// A computer with nothing of the floor on it, which answers every run and records it.
fn bare() -> (FakeMachine, Calls) {
    let runs: Calls = Arc::new(Mutex::new(Vec::new()));
    let exec: ExecFn = Arc::new(|cmd, _| Box::pin(async move { Ok(if cmd == free_kb_cmd() { res(0, "9000000\n", "") } else { ok() }) }));
    let seen = runs.clone();
    let run: RunFn = Arc::new(move |script, _| {
        seen.lock().unwrap().push(script);
        Box::pin(async { Ok(ok()) })
    });
    (FakeMachine { run: Some(run), ..FakeMachine::new(exec) }, runs)
}

/// `calls.findIndex(c => c.includes(needle))`: -1 where nothing matches.
fn at(calls: &[String], needle: &str) -> i64 {
    calls.iter().position(|c| c.contains(needle)).map_or(-1, |i| i as i64)
}

// TS: packages/engine/test/provision.test.ts:349
// "reads the floor first, then what is already there, then the steps, and writes the context last"
#[tokio::test(flavor = "multi_thread")]
async fn reads_the_floor_first_then_what_is_already_there_then_the_steps_and_writes_the_context_last() {
    let steps = vec![
        ToolInstall { manager: "npm".to_string(), bin: Some("codex".to_string()), ..step("agents/codex", "Codex") },
        ToolInstall { manager: "release".to_string(), bin: Some("gh".to_string()), ..step("tools/release/gh", "GitHub CLI") },
    ];
    let (machine, calls) = box_machine(no_answer, TOOLS_PATH);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let rows = provision_box(&machine, &plan_of(steps), said_into(seen.clone()), &on()).await.unwrap();
    let calls = snapshot(&calls);
    assert_eq!(at(&calls, &base_versions_cmd(TOOLS_PATH)), 0);
    assert!(at(&calls, "wsp-present") > 0);
    assert!(at(&calls, "install agents/codex") > at(&calls, "wsp-present"));
    assert!(at(&calls, "install tools/release/gh") > at(&calls, "install agents/codex"));
    assert!(at(&calls, "echo WSP_CTX") > at(&calls, "install tools/release/gh"));
    assert_eq!(outcomes(&rows), pairs(&[("agents/codex", "installed"), ("tools/release/gh", "installed")]));
    assert!(seen.lock().unwrap().last().unwrap().contains("machine context:"));
    // The floor's caches are the person's own on a computer they keep: nothing sweeps them.
    assert!(!calls.iter().any(|c| c.contains("rm -rf /root/.npm") || c.contains("apt-get clean")));
}

// TS: packages/engine/test/provision.test.ts:366
// "opens no shell of the computer's own on the context probe, since its root home is the one every workspace there writes"
#[tokio::test(flavor = "multi_thread")]
async fn opens_no_shell_of_the_computers_own_on_the_context_probe() {
    let (machine, calls) = box_machine(no_answer, TOOLS_PATH);
    let steps = vec![ToolInstall { manager: "npm".to_string(), bin: Some("codex".to_string()), ..step("agents/codex", "Codex") }];
    provision_box(&machine, &plan_of(steps), quiet(), &on()).await.unwrap();
    let probe = snapshot(&calls).into_iter().find(|c| c.contains("echo WSP_CTX")).expect("the context probe ran");
    // The alias section is what opened one: three login and interactive shells, one per shell the computer may run.
    assert!(!probe.contains("-lic"));
    assert!(!probe.contains("bash -l"));
    assert!(probe.contains(r#"echo "ALIASES unread""#));
    // What no shell costs is the aliases alone: the kernel, the disk and the shell name are read as they were.
    assert!(probe.contains("uname -r"));
    assert!(probe.contains(r#"echo "SHELL $shell""#));
}

// TS: packages/engine/test/provision.test.ts:380
// "says the floor's own lines as a person reads them, with no stage id of the image build's in front"
#[tokio::test(flavor = "multi_thread")]
async fn says_the_floors_own_lines_as_a_person_reads_them_with_no_stage_id_in_front() {
    // A computer with none of the floor on it: every floor row runs, and what the terminal and the log on that
    // computer read is the row and the tally, not the stage the image build files them under.
    let (machine, _) = box_machine(|cmd| (cmd == base_versions_cmd(TOOLS_PATH)).then(|| res(0, "", "")), TOOLS_PATH);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let steps = vec![ToolInstall { bin: Some("codex".to_string()), ..step("agents/codex", "Codex") }];
    provision_box(&machine, &plan_of(steps), said_into(seen.clone()), &on()).await.unwrap();
    let seen = seen.lock().unwrap();
    assert!(!seen.iter().any(|l| l.starts_with("deploying-daemon")));
    assert!(seen.iter().any(|l| is_jq_line(l)));
    assert!(seen.iter().any(|l| is_tally_line(l)));
}

// TS: packages/engine/test/provision.test.ts:391
// "answers one row per step and per row the plan set aside, with the outcome the computer gave each"
#[tokio::test(flavor = "multi_thread")]
async fn answers_one_row_per_step_and_per_row_the_plan_set_aside() {
    let steps = vec![
        step("agents/node", "Node 22.23.2"),
        ToolInstall {
            bin: Some("claude".to_string()),
            check: Some("claude --version".to_string()),
            ..step("agents/claude", "Claude Code")
        },
        ToolInstall {
            manager: "npm".to_string(),
            bin: Some("codex".to_string()),
            after: Some("agents/node".to_string()),
            ..step("agents/codex", "Codex")
        },
        ToolInstall { manager: "uv".to_string(), bin: Some("ruff".to_string()), ..step("tools/uv/ruff", "ruff") },
    ];
    let mut plan = plan_of(steps);
    plan.skipped = vec![PlanSkipped {
        id: "tools/brew-cask/raycast".to_string(),
        label: "Raycast".to_string(),
        note: "macOS app, no Linux build".to_string(),
    }];
    // Claude answers already; the node step fails, so what waits on it is skipped with its name.
    let (machine, _) = box_machine(
        |cmd| {
            // The read asks only about the steps that carry a check or a command, so the claude step is the first of them.
            if cmd.contains("wsp-present") {
                return Some(res(0, "wsp-present 0\n", ""));
            }
            if cmd.contains("install agents/node") {
                return Some(res(1, "", "Error: nodejs.org answered 404"));
            }
            None
        },
        TOOLS_PATH,
    );
    let rows = Arc::new(Mutex::new(Vec::new()));
    let answered = provision_box(&machine, &plan, rows_into(rows.clone()), &on()).await.unwrap();
    assert_eq!(
        outcomes(&answered),
        pairs(&[
            ("tools/brew-cask/raycast", "skipped"),
            ("agents/node", "failed"),
            ("agents/claude", "present"),
            ("agents/codex", "skipped"),
            ("tools/uv/ruff", "installed"),
        ])
    );
    // Every row reached the stage as its outcome landed, in the same order, and the reasons are the plan's and the
    // loop's own rather than a second wording here.
    assert_eq!(*rows.lock().unwrap(), answered);
    assert_eq!(answered[0].note.as_deref(), Some("macOS app, no Linux build"));
    assert!(answered[1].note.as_deref().unwrap().contains("nodejs.org answered 404"));
    assert_eq!(answered[2].note, None);
    assert_eq!(answered[3].note.as_deref(), Some("Node 22.23.2 did not install"));
}

// TS: packages/engine/test/provision.test.ts:426
// "answers one row per step as the checks left it, so a row whose check failed after its install is failed in the rows and said again"
#[tokio::test(flavor = "multi_thread")]
async fn answers_one_row_per_step_as_the_checks_left_it_so_a_row_whose_check_failed_after_its_install_is_failed() {
    let brew = |name: &str| ToolInstall {
        manager: "brew".to_string(),
        check: Some(format!("brew list --versions {name}")),
        ..step(&format!("tools/brew/{name}"), name)
    };
    let steps = vec![brew("bat"), brew("eza")];
    // Homebrew answers that bat is already installed and up to date, and its check then fails: the row is what the
    // check left it, never an installed row in the answer beside a failed one in the tally.
    let (machine, _) = box_machine(
        |cmd| {
            cmd.contains("wsp-check")
                .then(|| res(0, "wsp-check 0 Error: The current working directory must be readable to linuxbrew to run brew.\n", ""))
        },
        TOOLS_PATH,
    );
    let seen = Arc::new(Mutex::new(Vec::new()));
    let rows = provision_box(&machine, &plan_of(steps), said_into(seen.clone()), &on()).await.unwrap();
    assert_eq!(outcomes(&rows), pairs(&[("tools/brew/bat", "failed"), ("tools/brew/eza", "installed")]));
    assert!(rows[0].note.as_deref().unwrap().contains("The current working directory must be readable"));
    // What the log on that computer reads says the row twice: as the loop said it, then as the check left it, so the
    // log and the rows the record keeps cannot disagree. The check's command is the log's and never the row's.
    let seen = seen.lock().unwrap();
    let of = |prefix: &str| seen.iter().filter(|l| l.starts_with(prefix)).cloned().collect::<Vec<_>>();
    assert_eq!(
        of("bat: "),
        [
            "bat: installed",
            "bat: the check did not pass: brew list --versions bat",
            "bat: failed (Error: The current working directory must be readable to linuxbrew to run brew.)"
        ]
    );
    assert_eq!(of("eza: "), ["eza: installed"]);
}

// TS: packages/engine/test/provision.test.ts:450
// "says which row it was on while it runs, and how many of how many"
#[tokio::test(flavor = "multi_thread")]
async fn says_which_row_it_was_on_while_it_runs_and_how_many_of_how_many() {
    let steps = vec![step("agents/codex", "Codex"), step("tools/release/gh", "GitHub CLI")];
    let (machine, _) = box_machine(no_answer, TOOLS_PATH);
    let at = Arc::new(Mutex::new(Vec::new()));
    provision_box(&machine, &plan_of(steps), at_into(at.clone()), &on()).await.unwrap();
    let at = at.lock().unwrap();
    let said: Vec<&String> = at.iter().flatten().collect();
    assert!(said.iter().any(|a| a.as_str() == "1/2: Codex"));
    assert!(said.iter().any(|a| a.as_str() == "2/2: GitHub CLI"));
    // Nothing is under way while the floor runs and nothing once the last row has landed.
    assert_eq!(at[0], None);
    assert_eq!(*at.last().unwrap(), None);
}

// TS: packages/engine/test/provision.test.ts:462
// "throws the computer's own sentence when it stops answering, with the rows that landed before it already said"
#[tokio::test(flavor = "multi_thread")]
async fn throws_the_computers_own_sentence_when_it_stops_answering() {
    let steps = vec![step("a/one", "One"), step("a/two", "Two"), step("a/three", "Three")];
    let ran = Arc::new(AtomicUsize::new(0));
    let exec_ran = ran.clone();
    let exec: ExecFn = Arc::new(move |cmd, _| {
        let ran = exec_ran.clone();
        Box::pin(async move {
            if cmd == base_versions_cmd(TOOLS_PATH) {
                return Ok(res(0, &format!("{}\n", floor_read()), ""));
            }
            // The floor's own step is one run, so the link goes while the third of the recipe's rows is going on.
            if ran.load(Ordering::SeqCst) >= 4 {
                return Err(common::other("spoo is not connected"));
            }
            if cmd == free_kb_cmd() {
                return Ok(res(0, "9000000\n", ""));
            }
            Ok(ok())
        })
    });
    let run: RunFn = Arc::new(move |_, _| {
        ran.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(ok()) })
    });
    let machine = FakeMachine { run: Some(run), ..FakeMachine::new(exec) };
    let rows = Arc::new(Mutex::new(Vec::new()));
    let err = provision_box(&machine, &plan_of(steps), rows_into(rows.clone()), &on()).await.unwrap_err();
    assert!(err.to_string().contains("spoo is not connected"), "{err}");
    assert_eq!(rows.lock().unwrap().iter().map(|r| r.id.clone()).collect::<Vec<_>>(), ["a/one", "a/two"]);
}

// TS: packages/engine/test/provision.test.ts:508
// "installs the agents alone, lands their own files with the servers after the CLIs, and the machine context after all of it"
#[tokio::test(flavor = "multi_thread")]
async fn installs_the_agents_alone_lands_their_own_files_with_the_servers_after_the_clis_and_the_context_after_all_of_it() {
    let (machine, calls) = box_machine(no_answer, TOOLS_PATH);
    let jq = ToolInstall { bin: Some("jq".to_string()), ..step("tools/apt/jq", "jq") };
    let mut plan = with_files_and_servers(vec![codex(), jq]);
    plan.agents = 1;
    let agents = provision_step(&common::handle(&machine), &plan, EngineStep::Agents, new_setup_run(), quiet(), &on()).await.unwrap();
    assert_eq!(agents.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), ["agents/codex"]);
    assert!(!snapshot(&calls).iter().any(|c| c.contains("wsp-land")));
    calls.lock().unwrap().clear();
    let rows = provision_box(&machine, &plan, quiet(), &on()).await.unwrap();
    let calls = snapshot(&calls);
    assert!(at(&calls, "install agents/codex") > -1);
    assert!(at(&calls, "wsp-land") > at(&calls, "install tools/apt/jq"));
    assert!(at(&calls, "wsp_mcp_read") > at(&calls, "wsp-land"));
    assert!(at(&calls, "echo WSP_CTX") > at(&calls, "wsp_mcp_read"));
    // Every row says what it puts there, so the word on the computers row counts them by kind.
    let kinds: Vec<(String, Option<PlaceProvisionKind>, Option<PlaceSetupStep>)> =
        rows.iter().map(|r| (r.id.clone(), r.kind, r.step)).collect();
    assert_eq!(
        kinds,
        vec![
            ("agents/codex".to_string(), None, Some(PlaceSetupStep::Agents)),
            ("tools/apt/jq".to_string(), None, Some(PlaceSetupStep::Clis)),
            ("files/.claude-cfg/skills".to_string(), Some(PlaceProvisionKind::File), Some(PlaceSetupStep::Mcp)),
            (format!("{MCP_ID_PREFIX}claude/github"), Some(PlaceProvisionKind::Server), Some(PlaceSetupStep::Mcp)),
        ]
    );
    assert_eq!(setup_word(Some(&ended()), Some(&applied(rows))), "2 tools, 1 file, 1 MCP server ready");
}

// TS: packages/engine/test/provision.test.ts:531
// "counts the files and the servers in what the job says it is putting there, and says the row it is on in each loop of installs"
#[tokio::test(flavor = "multi_thread")]
async fn counts_the_files_and_the_servers_in_what_the_job_says_it_is_putting_there() {
    let plan = with_files_and_servers(vec![codex()]);
    let counts = provision_counts_of(&plan);
    let by_kind: IndexMap<PlaceProvisionKind, f64> = [
        (PlaceProvisionKind::Tool, counts.tool as f64),
        (PlaceProvisionKind::File, counts.file as f64),
        (PlaceProvisionKind::Server, counts.server as f64),
    ]
    .into_iter()
    .collect();
    assert_eq!(provision_count_word(&by_kind), "1 tool, 1 file, 1 MCP server");
    let (machine, _) = box_machine(no_answer, TOOLS_PATH);
    let at = Arc::new(Mutex::new(Vec::new()));
    provision_box(&machine, &plan, at_into(at.clone()), &on()).await.unwrap();
    assert!(at.lock().unwrap().iter().flatten().any(|a| a == "1/1: Codex"));
}

// TS: packages/engine/test/provision.test.ts:542
// "says each stage of the two rounds as a line of its own, in the order they happen"
#[tokio::test(flavor = "multi_thread")]
async fn says_each_stage_of_the_two_rounds_as_a_line_of_its_own_in_the_order_they_happen() {
    let tar = why_tar();
    let plan = with_files_and_servers(vec![codex()]);
    let (machine, _) = box_machine(no_answer, TOOLS_PATH);
    let said = Arc::new(Mutex::new(Vec::new()));
    let packed = PackedFiles {
        tar: tar.clone(),
        bytes: tar.len() as u64,
        unpacked: 4,
        files: Some(1),
        stood: Some(vec![".claude-cfg/skills/old/SKILL.md".to_string()]),
        skipped: vec![],
        ..Default::default()
    };
    let with_pack =
        ProvisionPlan { files: Some(PlanFiles { lands: plan.files.as_ref().unwrap().lands.clone(), pack: pack_of(packed) }), ..plan };
    let rows = provision_box(&machine, &with_pack, said_into(said.clone()), &on()).await.unwrap();
    // The lines the job reads its own minutes off, each written when its stage is done: the log on that computer
    // carries the time of every one of them.
    let lines: Vec<String> = said
        .lock()
        .unwrap()
        .iter()
        .filter(|l| ["packed", "shipped", "landed", "list read", "servers merged"].iter().any(|p| l.starts_with(p)))
        .cloned()
        .collect();
    let written = rows.iter().filter(|r| r.kind == Some(PlaceProvisionKind::Server) && word(r.outcome) == "installed").count();
    assert_eq!(
        lines,
        [
            // What the pack left home because that computer already holds it, and what it put in the archive, are both
            // on the line: this pack was told one file of the plan stands there.
            provision_packed_line(1.0, &Packed { bytes: tar.len() as f64, files: Some(1.0), stood: Some(1.0) }),
            provision_shipped_line(&Shipped { part: 1.0, parts: 1.0, bytes: tar.len() as f64, total: tar.len() as f64, pieces: None }),
            // This machine's canned run prints no landing line, so the walk reads none; the file counts are the
            // landing's own test, over a real shell.
            provision_landed_line(0.0),
            provision_list_read_line(0.0),
            provision_servers_line(written as f64, 1.0),
        ]
    );
}

// TS: packages/engine/test/provision.test.ts:568
// "puts each thing the pack left out of the copy in the job's rows with its reason, where the app and the log read it"
#[tokio::test(flavor = "multi_thread")]
async fn puts_each_thing_the_pack_left_out_of_the_copy_in_the_jobs_rows_with_its_reason() {
    let tar = why_tar();
    let plan = with_files_and_servers(vec![]);
    let gem = "gem left out of the copy: GEMINI_API_KEY belongs to the Gemini CLI key, so set it there or give the variable another name";
    let pem = "pem left out of the copy: sets KEY to a value on more than one line, which cannot travel by name";
    let skipped: Vec<SkippedPath> = [gem, pem]
        .iter()
        .map(|note| SkippedPath { id: "agents/claude".to_string(), path: "~/.claude.json".to_string(), note: note.to_string() })
        .collect();
    let (machine, _) = box_machine(no_answer, TOOLS_PATH);
    let said = Arc::new(Mutex::new(Vec::new()));
    let packed =
        PackedFiles { tar: tar.clone(), bytes: tar.len() as u64, unpacked: 4, files: Some(1), skipped, stood: None, ..Default::default() };
    let with_pack =
        ProvisionPlan { files: Some(PlanFiles { lands: plan.files.as_ref().unwrap().lands.clone(), pack: pack_of(packed) }), ..plan };
    let rows = provision_box(&machine, &with_pack, said_into(said.clone()), &on()).await.unwrap();
    let left: Vec<PlaceProvisionRow> = rows.into_iter().filter(|r| r.id.starts_with("left-out/")).collect();
    let expected: Vec<PlaceProvisionRow> = [gem, pem]
        .iter()
        .enumerate()
        .map(|(i, note)| PlaceProvisionRow {
            id: format!("left-out/{i}"),
            label: "~/.claude.json".to_string(),
            outcome: PlaceProvisionRowOutcome::Skipped,
            kind: Some(PlaceProvisionKind::File),
            step: Some(PlaceSetupStep::Mcp),
            note: Some(note.to_string()),
            fix: None,
            keys: None,
            ms: None,
            earlier: None,
            project: None,
        })
        .collect();
    assert_eq!(left, expected);
    assert!(said.lock().unwrap().contains(&format!("~/.claude.json: skipped ({gem})")));
}

// TS: packages/engine/test/provision.test.ts:587
// "says every path the archive carried failed, with the reason, when the files could not be packed or landed, and goes on with the rest"
#[tokio::test(flavor = "multi_thread")]
async fn says_every_path_the_archive_carried_failed_with_the_reason_and_goes_on_with_the_rest() {
    let plan = with_files_and_servers(vec![codex()]);
    let (machine, calls) = box_machine(no_answer, TOOLS_PATH);
    let pack: PackFiles = Arc::new(|_| Box::pin(async { Err(common::other("Keychain: user cancelled")) }));
    let with_pack = ProvisionPlan { files: Some(PlanFiles { lands: plan.files.as_ref().unwrap().lands.clone(), pack }), ..plan };
    let rows = provision_box(&machine, &with_pack, quiet(), &on()).await.unwrap();
    let got: Vec<(String, String, Option<String>)> =
        rows.iter().map(|r| (r.id.clone(), word(r.outcome).to_string(), r.note.clone())).collect();
    assert_eq!(
        got,
        vec![
            ("agents/codex".to_string(), "installed".to_string(), None),
            ("files/.claude-cfg/skills".to_string(), "failed".to_string(), Some("Keychain: user cancelled".to_string())),
            (format!("{MCP_ID_PREFIX}claude/github"), "skipped".to_string(), Some("the config edit did not run (exit 0)".to_string())),
        ]
    );
    let calls = snapshot(&calls);
    // The machine context still lands: a computer with its tools on and no word of why is worse than the failure.
    assert!(calls.iter().any(|c| c.contains("echo WSP_CTX")));
    // Whose a server in an agent's own file is comes off the list beside the job, which is read even where nothing
    // was packed, so a key wsp wrote there before is still wsp's.
    assert!(calls.iter().any(|c| c.contains(SERVER_MARK)));
}

// TS: packages/engine/test/provision.test.ts:608
// "reads the list's keys and both copies of the agents' own files between the landing and the close, and writes down the keys it merged"
#[tokio::test(flavor = "multi_thread")]
async fn reads_the_lists_keys_and_both_copies_of_the_agents_own_files_between_the_landing_and_the_close() {
    let (machine, calls) = box_machine(no_answer, TOOLS_PATH);
    provision_box(&machine, &with_files_and_servers(vec![codex()]), quiet(), &on()).await.unwrap();
    let calls = snapshot(&calls);
    // The copy that travelled is read out of the job's own folder beside the agent's own file, in one read.
    let read = calls.iter().find(|c| c.contains("wsp_mcp_read")).expect("the configs were read");
    assert!(read.contains("/root/.claude-cfg/.claude.json"));
    assert!(read.contains(&format!("{}/.claude-cfg/.claude.json", place_provision_paths("/root").staging)));
    assert!(at(&calls, "wsp-land") < at(&calls, "wsp_mcp_read"));
    assert!(at(&calls, SERVER_MARK) > at(&calls, "wsp-land"));
    // The close is last of the three, so the keys the servers step wrote are in the list it folds together.
    assert!(at(&calls, OLD_APPEND_MARKS) > at(&calls, SERVER_MARK));
}

// TS: packages/engine/test/provision.test.ts:622
// "asks the computer nothing about files or servers when the recipe names none, and closes its own folder there all the same"
#[tokio::test(flavor = "multi_thread")]
async fn asks_the_computer_nothing_about_files_or_servers_when_the_recipe_names_none() {
    let (machine, calls) = box_machine(no_answer, TOOLS_PATH);
    provision_box(&machine, &plan_of(vec![codex()]), quiet(), &on()).await.unwrap();
    let calls = snapshot(&calls);
    assert!(!calls.iter().any(|c| c.contains("wsp-land") || c.contains("wsp_mcp_read")));
    // The close is where the job's own folder on that computer is swept, so it runs whether or not this recipe
    // carries a file of the person's; with no landing it leaves the list exactly as it was.
    assert!(calls.iter().any(|c| c.contains(OLD_APPEND_MARKS)));
}

// TS: packages/engine/test/provision.test.ts:645
// "is the plan's own on every script the job sends, and no line of one names a directory under the home the workspaces there write"
#[tokio::test(flavor = "multi_thread")]
async fn is_the_plans_own_on_every_script_the_job_sends() {
    let probe = probe_path("/root");
    let import = GoldenImport {
        agents: agent_installs_for(&["agents/claude", "agents/codex"]).installs,
        node: agent_installs_for(&["agents/codex"]).node,
        tools: tool_installs_for(&["tools/brew/gh", "tools/npm/turbo"], &probe, None),
        ..Default::default()
    };
    let plan = provision_plan_of(&import, RECIPE_AT, &probe, None, None);
    let (machine, calls) = box_machine(no_answer, &probe);
    provision_box(&machine, &plan, quiet(), &on()).await.unwrap();
    let calls = snapshot(&calls);
    let scripts: Vec<String> = calls.iter().cloned().chain(plan.steps.iter().map(|s| s.cmd.clone())).collect();
    let exported: Vec<String> = scripts.iter().flat_map(|s| exported_paths(s)).collect();
    // The presence read, the checks, the floor's versions read, the cache sweep, the context probe and every step's
    // own line are all in here; the node line a harness install carries is the one other PATH they set.
    assert!(exported.len() > 8);
    for value in &exported {
        assert!(*value == probe || *value == format!("{probe}:$PATH") || value == r#""/usr/local/bin:$PATH""#, "{value}");
    }
    // No line names /root/.local/bin, a folder every workspace there writes, but the login shell's PATH written into
    // a file on that computer, which the daemon and this job both stopped resolving a command through.
    let naming: Vec<&str> = scripts.iter().flat_map(|c| c.split('\n')).filter(|l| l.contains("/root/.local/bin")).collect();
    assert_eq!(naming.len(), 1);
    assert!(naming[0].contains(PROFILE_PATH_FILE));
    // The harness lands in a folder of the job's own PATH, and its check runs under that PATH, so the check finds it.
    let harness = plan.steps.iter().find(|s| s.id == "agents/claude").unwrap();
    let dir = harness_dir(&harness.cmd).expect("the harness install line");
    assert!(probe.split(':').any(|d| d == dir), "{dir}");
    let checks: Vec<&String> = calls.iter().filter(|c| c.contains("claude --version")).collect();
    assert!(!checks.is_empty());
    for check in checks {
        assert!(check.starts_with(&path_line(&probe, None)), "{check}");
    }
}

// TS: packages/engine/test/provision.test.ts:711
// "carries every manager's knob under the prefix on a joined computer's job, the floor's own rows among them"
#[tokio::test(flavor = "multi_thread")]
async fn carries_every_managers_knob_under_the_prefix_on_a_joined_computers_job() {
    let probe = probe_path("/root");
    let tools = tool_installs_for(&["tools/uv/ruff", "tools/cargo/tokei"], &probe, Some(TOOL_PREFIX));
    let import = GoldenImport { tools, ..Default::default() };
    let plan = provision_plan_of(&import, RECIPE_AT, &probe, Some(TOOL_PREFIX), None);
    assert_eq!(plan.prefix.as_deref(), Some(TOOL_PREFIX));
    let ids: Vec<&str> = plan.steps.iter().map(|t| t.id.as_str()).collect();
    assert!(ids.contains(&"tools/uv/ruff") && ids.contains(&"tools/cargo/tokei"));
    let without_python = format!("{}\n", floor_without("python3"));
    let versions = base_versions_cmd(&probe);
    let (machine, calls) = box_machine(move |cmd| (cmd == versions).then(|| res(0, &without_python, "")), &probe);
    provision_box(&machine, &plan, quiet(), &on()).await.unwrap();
    let scripts: Vec<String> = snapshot(&calls).into_iter().chain(plan.steps.iter().map(|s| s.cmd.clone())).collect();
    // Every line that puts this job's own PATH on a script carries the managers' knobs with it: the presence read,
    // the checks, the version reads and every step's own line, the floor's among them.
    let exported: Vec<&str> =
        scripts.iter().flat_map(|s| s.split('\n')).filter(|l| l.starts_with("export PATH=") && l.contains(&probe)).collect();
    assert!(exported.len() > 8);
    // The floor's version read puts the list ahead of the machine's own PATH and asks each floor command for its
    // version; every other script runs a row's own lines and carries the managers' knobs beside the list.
    for value in &exported {
        assert!(*value == path_line(&probe, Some(TOOL_PREFIX)) || *value == format!("export PATH={probe}:$PATH"), "{value}");
    }
    assert!(exported.iter().filter(|l| **l == path_line(&probe, Some(TOOL_PREFIX))).count() > 5);
    // The floor's python row is in that read, and the interpreter python3 is linked to is uv's under the prefix.
    let python: Vec<&String> = scripts.iter().filter(|s| s.contains("uv python install 3.12")).collect();
    assert_eq!(python.len(), 1);
    assert!(python[0].contains(&format!("UV_PYTHON_INSTALL_DIR={TOOL_PREFIX}/uv/python")));
    assert!(python[0].contains(r#"ln -sfn "$(uv python find --managed-python 3.12)" /usr/local/bin/python3"#));
}

// TS: packages/engine/test/provision.test.ts:786
// "runs git-lfs's install system-wide once wsp put the row on, never on one the box had, and a hook that fails fails its row"
#[tokio::test(flavor = "multi_thread")]
async fn runs_git_lfss_install_system_wide_once_wsp_put_the_row_on_never_on_one_the_box_had() {
    let lfs = ToolInstall { bin: Some("git-lfs".to_string()), ..step("tools/brew/git-lfs", "Git LFS") };
    let jq = ToolInstall { bin: Some("jq".to_string()), ..step("tools/brew/jq", "jq") };
    let (machine, calls) = box_machine(no_answer, TOOLS_PATH);
    let rows =
        provision_step(&common::handle(&machine), &plan_of(vec![lfs.clone(), jq]), EngineStep::Clis, new_setup_run(), quiet(), &on())
            .await
            .unwrap();
    assert_eq!(outcomes(&rows), pairs(&[("tools/brew/git-lfs", "installed"), ("tools/brew/jq", "installed")]));
    let hooks: Vec<String> = snapshot(&calls).into_iter().filter(|c| c.contains("git lfs install")).collect();
    assert_eq!(hooks.len(), 1);
    assert!(hooks[0].contains("git lfs install --system"));
    // The person's own ~/.gitconfig is the git row's file, so the filters go in the system's and never there.
    assert!(!hooks[0].contains("--global"));

    // A git-lfs the box had before wsp is the person's own: its system gitconfig is not wsp's to write.
    let (theirs, their_calls) = box_machine(
        |cmd| (cmd.contains("wsp-present") || cmd.contains("command -v 'git-lfs'")).then(|| res(0, "wsp-present 0 /usr/bin/git-lfs\n", "")),
        TOOLS_PATH,
    );
    let kept = provision_step(&common::handle(&theirs), &plan_of(vec![lfs.clone()]), EngineStep::Clis, new_setup_run(), quiet(), &on())
        .await
        .unwrap();
    assert_eq!(outcomes(&kept), pairs(&[("tools/brew/git-lfs", "present")]));
    assert!(!snapshot(&their_calls).iter().any(|c| c.contains("git lfs install")));

    let (refused, _) =
        box_machine(|cmd| cmd.contains("git lfs install").then(|| res(2, "", "git: 'lfs' is not a git command")), TOOLS_PATH);
    let failed =
        provision_step(&common::handle(&refused), &plan_of(vec![lfs]), EngineStep::Clis, new_setup_run(), quiet(), &on()).await.unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].id, "tools/brew/git-lfs");
    assert_eq!(word(failed[0].outcome), "failed");
    assert!(failed[0].note.as_deref().unwrap().contains("git: 'lfs' is not a git command"));
}

// TS: packages/engine/test/provision.test.ts:812
// "runs a CLI's hook as its row lands, so the row is said after its hook and before the next row installs"
#[tokio::test(flavor = "multi_thread")]
async fn runs_a_clis_hook_as_its_row_lands_so_the_row_is_said_after_its_hook_and_before_the_next_row_installs() {
    let lfs = ToolInstall { bin: Some("git-lfs".to_string()), ..step("tools/brew/git-lfs", "Git LFS") };
    let jq = ToolInstall { bin: Some("jq".to_string()), ..step("tools/brew/jq", "jq") };
    let order = Arc::new(Mutex::new(Vec::<String>::new()));
    let answered = order.clone();
    let (machine, _) = box_machine(
        move |cmd| {
            if cmd.contains("git lfs install") {
                answered.lock().unwrap().push("hook git-lfs".to_string());
            }
            if cmd.contains("install tools/brew/jq") {
                answered.lock().unwrap().push("install jq".to_string());
            }
            None
        },
        TOOLS_PATH,
    );
    let said = order.clone();
    let stage: ProvisionStage = Arc::new(move |_, _, row| {
        if let Some(row) = row {
            said.lock().unwrap().push(format!("said {}", row.id));
        }
    });
    provision_step(&common::handle(&machine), &plan_of(vec![lfs, jq]), EngineStep::Clis, new_setup_run(), stage, &on()).await.unwrap();
    assert_eq!(*order.lock().unwrap(), ["hook git-lfs", "said tools/brew/git-lfs", "install jq", "said tools/brew/jq"]);
}

// TS: packages/engine/test/provision.test.ts:827
// "are on the run the moment each lands, and on it once when the loop ends"
#[tokio::test(flavor = "multi_thread")]
async fn are_on_the_run_the_moment_each_lands_and_on_it_once_when_the_loop_ends() {
    let jq = ToolInstall { bin: Some("jq".to_string()), ..step("tools/brew/jq", "jq") };
    let yq = ToolInstall { bin: Some("yq".to_string()), ..step("tools/brew/yq", "yq") };
    let (machine, _) = box_machine(no_answer, TOOLS_PATH);
    let run = new_setup_run();
    let seen = Arc::new(Mutex::new(IndexMap::<String, Vec<String>>::new()));
    let (on_run, kept) = (run.clone(), seen.clone());
    let stage: ProvisionStage = Arc::new(move |_, _, row| {
        if let Some(row) = row {
            let mut seen = kept.lock().unwrap();
            if !seen.contains_key(&row.id) {
                let tools = on_run.lock().unwrap().tools.iter().map(|t| t.id.clone()).collect();
                seen.insert(row.id.clone(), tools);
            }
        }
    });
    provision_step(&common::handle(&machine), &plan_of(vec![jq, yq]), EngineStep::Clis, run.clone(), stage, &on()).await.unwrap();
    assert_eq!(seen.lock().unwrap().get("tools/brew/jq"), Some(&vec!["tools/brew/jq".to_string()]));
    let tools: Vec<(String, String)> = run.lock().unwrap().tools.iter().map(|t| (t.id.clone(), t.outcome.clone())).collect();
    assert_eq!(tools, pairs(&[("tools/brew/jq", "installed"), ("tools/brew/yq", "installed")]));
}

// TS: packages/engine/test/provision.test.ts:845
// "is set aside at once with the sha256 claude showed, and the one that installs reads installed"
#[tokio::test(flavor = "multi_thread")]
async fn is_set_aside_at_once_with_the_sha256_claude_showed_and_the_one_that_installs_reads_installed() {
    let sha = "a".repeat(64);
    let install = |name: &str| PluginRoad::Claude.install(name, "acme/plugins");
    assert!(install("hooks@acme").contains("--json"));
    let shown = format!("{}\n", serde_json::json!({ "ok": false, "shownCommand": { "command": "curl x | sh", "sha256": sha } }));
    let (machine, _) = box_machine(move |cmd| cmd.contains("hooks@acme").then(|| res(1, &shown, "")), TOOLS_PATH);
    let plugin = |name: &'static str| PlanPlugin {
        id: format!("plugins/{name}"),
        label: name.to_string(),
        cmd: PluginRoad::Claude.install(name, "acme/plugins"),
        asked: Some(Arc::new(move |out: &str| PluginRoad::Claude.asked(out, name))),
    };
    let plan = ProvisionPlan { plugins: Some(vec![plugin("hooks@acme"), plugin("lint@acme")]), ..plan_of(vec![]) };
    let rows = provision_step(&common::handle(&machine), &plan, EngineStep::Plugins, new_setup_run(), quiet(), &on()).await.unwrap();
    assert_eq!(outcomes(&rows), pairs(&[("plugins/hooks@acme", "skipped"), ("plugins/lint@acme", "installed")]));
    assert!(rows[0].note.as_deref().unwrap().contains(&format!("--accept-command {sha}")));
}

// TS: packages/engine/test/provision-floor.test.ts:40
// "installs build-essential as a setup's floor step only for a plan that needs a compiler"
#[tokio::test(flavor = "multi_thread")]
async fn installs_build_essential_as_a_setups_floor_step_only_for_a_plan_that_needs_a_compiler() {
    let plan = |compiler: bool| ProvisionPlan { recipe_at: "x".to_string(), path: TOOLS_PATH.to_string(), compiler, ..Default::default() };
    let (off, off_runs) = bare();
    provision_step(&common::handle(&off), &plan(false), EngineStep::Floor, new_setup_run(), quiet(), &on()).await.unwrap();
    let off_runs = snapshot(&off_runs);
    assert!(!off_runs.iter().any(|r| r.contains("build-essential")));
    assert!(off_runs.iter().any(|r| r.contains("ripgrep")));
    let (with, with_runs) = bare();
    provision_step(&common::handle(&with), &plan(true), EngineStep::Floor, new_setup_run(), quiet(), &on()).await.unwrap();
    assert!(snapshot(&with_runs).iter().any(|r| r.contains("build-essential")));
}
