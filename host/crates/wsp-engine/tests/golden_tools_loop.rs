// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/tools-floor.test.ts and the describe "the tools loop on a machine that is not a fresh
//! builder" of packages/engine/test/golden.test.ts (lines 2953-3082), at a4033dc34: the disk the tools loop keeps free,
//! the volume it reads on a computer somebody owns, the presence checks paged, the shared Homebrew step, a step the
//! caller found on the machine, the caches and the watcher's hand-off.
mod common;

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use common::{ok, res, ExecFn, FakeMachine, RunFn};
use wsp_catalog::roads::MIB;
use wsp_engine::golden_seam::{
    install_tools, owned_floor_bytes, InstallToolsOptions, OnTool, ToolInstall, ToolResult, ToolStage, ALREADY_ON_MACHINE,
    TOOLS_DISK_FLOOR, TOOLS_PATH,
};
use wsp_engine::machine::ExecResult;
use wsp_engine::provision::{new_setup_run, provision_step, EngineStep, ProvisionOn, ProvisionPlan, ProvisionStage};
use wsp_wire::types::daemon_contract::TOOL_PREFIX;
use wsp_wire::views::place::PlaceProvisionRowOutcome;

/// golden-tools.ts's own df line for /root's free column, as golden.test.ts spells it.
const FREE: &str = "df -Pk /root | awk 'NR==2{print $4}'";
/// exec-detached.ts: how many presence checks one exec carries.
const READS_PER_EXEC: usize = 8;

fn quiet_stage() -> ToolStage {
    Arc::new(|_, _| {})
}

fn quiet_provision() -> ProvisionStage {
    Arc::new(|_, _, _| {})
}

/// writeStub: a script a test runs by name on a PATH, its mode set here because the box's umask is 077.
fn write_stub(path: &Path, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, script).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn script_step(id: &str, label: &str, cmd: &str) -> ToolInstall {
    ToolInstall { id: id.to_string(), label: label.to_string(), manager: "script".to_string(), cmd: cmd.to_string(), ..Default::default() }
}

/// A machine that answers every exec and every run through `answer`, which sees the text each time.
fn answering(answer: impl Fn(&str) -> ExecResult + 'static) -> FakeMachine {
    let answer = Arc::new(answer);
    let exec_answer = answer.clone();
    let exec: ExecFn = Arc::new(move |cmd, _| {
        let said = exec_answer(&cmd);
        Box::pin(async move { Ok(said) })
    });
    let run: RunFn = Arc::new(move |script, _| {
        let said = answer(&script);
        Box::pin(async move { Ok(said) })
    });
    FakeMachine { id: "m1".to_string(), exec, run: Some(run), download_url: None, upload_url: None, put_bytes: None }
}

// --- tools-floor.test.ts ------------------------------------------------------------------------------------

/// A machine whose df reads what is free, and each install takes the bytes its step says it will. The second field is
/// what ran, by the command that matched.
fn disk(free_bytes: u64, takes: Vec<(&'static str, u64)>) -> (FakeMachine, Arc<Mutex<Vec<String>>>) {
    let free = Arc::new(Mutex::new(free_bytes));
    let ran: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let seen = ran.clone();
    let machine = answering(move |text| {
        if text.starts_with("df ") {
            let free_kb = *free.lock().unwrap() / 1024;
            let stdout = if text.contains("$3") { format!("{} {free_kb}\n", (50_000 * MIB) / 1024) } else { format!("{free_kb}\n") };
            return res(0, &stdout, "");
        }
        for (cmd, bytes) in &takes {
            if text.contains(cmd) {
                seen.lock().unwrap().push(cmd.to_string());
                let mut free = free.lock().unwrap();
                *free = free.saturating_sub(*bytes);
            }
        }
        ok()
    });
    (machine, ran)
}

// TS: packages/engine/test/tools-floor.test.ts:44
// "skips an install too big for the room above the floor, alone, and installs what fits after it"
#[tokio::test(flavor = "multi_thread")]
async fn skips_an_install_too_big_for_the_room_above_the_floor_alone_and_installs_what_fits_after_it() {
    let zig = ToolInstall {
        id: "tools/brew/zig".to_string(),
        label: "zig".to_string(),
        manager: "brew".to_string(),
        cmd: "install-zig".to_string(),
        bytes: Some(2400 * MIB),
        ..Default::default()
    };
    let jq = ToolInstall {
        id: "tools/brew/jq".to_string(),
        label: "jq".to_string(),
        manager: "brew".to_string(),
        cmd: "install-jq".to_string(),
        bytes: Some(2 * MIB),
        ..Default::default()
    };
    let (machine, ran) = disk(TOOLS_DISK_FLOOR + 1000 * MIB, vec![("install-zig", 2400 * MIB), ("install-jq", 2 * MIB)]);
    let tools =
        install_tools(&machine, &[zig, jq], quiet_stage(), InstallToolsOptions { keep_caches: true, ..Default::default() }).await.unwrap();
    let got: Vec<(&str, &str)> = tools.iter().map(|t| (t.label.as_str(), t.outcome.as_str())).collect();
    assert_eq!(got, vec![("zig", "skipped"), ("jq", "installed")]);
    let note = tools[0].note.as_deref().unwrap();
    let allowed: Vec<String> = ["3", "3.0"]
        .iter()
        .flat_map(|free| ["2", "2.0"].iter().map(move |keep| format!("needs about 2.3 GB, {free} GB free, keeping {keep} GB free")))
        .collect();
    assert!(allowed.iter().any(|a| a == note), "note was {note:?}");
    assert_eq!(*ran.lock().unwrap(), vec!["install-jq".to_string()]);
}

// TS: packages/engine/test/tools-floor.test.ts:57
// "holds back a tenth of the disk on a computer somebody owns, where that is more than the floor"
#[test]
fn holds_back_a_tenth_of_the_disk_on_a_computer_somebody_owns_where_that_is_more_than_the_floor() {
    assert_eq!(owned_floor_bytes(Some(75 * 1024 * MIB)), (75 * 1024 * MIB) / 10);
    assert_eq!(owned_floor_bytes(Some(10 * 1024 * MIB)), TOOLS_DISK_FLOOR);
    assert_eq!(owned_floor_bytes(None), TOOLS_DISK_FLOOR);
}

/// A box whose /root sits on a 100 GB volume with 50 GB free and whose every other folder, wsp's install folder among
/// them, sits on a 20 GB one with 3 GB free. Every df the run sends goes to a real bash, whose PATH finds a df
/// answering by the folder it is handed; anything else answers nothing. The second field reads the folders df was
/// handed.
fn two_volumes(dir: &Path) -> (FakeMachine, impl Fn() -> Vec<String>) {
    let handed = dir.join("handed");
    write_stub(
        &dir.join("df"),
        &[
            "#!/bin/sh".to_string(),
            format!("echo \"$2\" >> {}", handed.display()),
            "case \"$2\" in /root|/root/*) size=104857600 free=52428800 ;; *) size=20971520 free=3145728 ;; esac".to_string(),
            "echo \"Filesystem 1024-blocks Used Available Capacity Mounted on\"".to_string(),
            "echo \"fake $size $((size - free)) $free 50% /\"".to_string(),
            String::new(),
        ]
        .join("\n"),
    );
    let stub_dir = dir.to_path_buf();
    let machine = answering(move |cmd| {
        if !cmd.contains("df -Pk") {
            return ok();
        }
        let path = format!("{}:{}", stub_dir.display(), std::env::var("PATH").unwrap_or_default());
        let out = std::process::Command::new("bash").arg("-c").arg(cmd).env("PATH", path).output().expect("bash runs");
        ExecResult {
            exit_code: out.status.code().map(i64::from).unwrap_or(-1).into(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    });
    (machine, move || std::fs::read_to_string(&handed).unwrap().trim().split('\n').map(str::to_string).collect())
}

// TS: packages/engine/test/tools-floor.test.ts:91
// "keeps its floor and reads what is free on the volume wsp installs onto, not the one /root sits on"
#[tokio::test(flavor = "multi_thread")]
async fn keeps_its_floor_and_reads_what_is_free_on_the_volume_wsp_installs_onto_not_the_one_root_sits_on() {
    let dir = tempfile::Builder::new().prefix("wsp-volumes-").tempdir().unwrap();
    let big = ToolInstall {
        id: "tools/custom/big".to_string(),
        label: "big".to_string(),
        manager: "script".to_string(),
        cmd: "install-big".to_string(),
        bytes: Some(2048 * MIB),
        ..Default::default()
    };
    let (machine, handed) = two_volumes(dir.path());
    let plan = ProvisionPlan {
        recipe_at: "2026-10-05T10:00:00.000Z".to_string(),
        path: "/usr/bin:/bin".to_string(),
        prefix: Some(TOOL_PREFIX.to_string()),
        steps: vec![big],
        skipped: vec![],
        agents: 0,
        compiler: false,
        ..Default::default()
    };
    let on = ProvisionOn { home: "/root".to_string(), held: None, stores: None };
    let rows = provision_step(&machine, &plan, EngineStep::Clis, new_setup_run(), quiet_provision(), &on).await.unwrap();
    // 3 GB free less the 2 GB it takes leaves less than the 2 GB a 20 GB disk keeps; /root's volume would have let it in.
    let got: Vec<(&str, PlaceProvisionRowOutcome, Option<&str>)> =
        rows.iter().map(|r| (r.id.as_str(), r.outcome, r.note.as_deref())).collect();
    assert_eq!(got, vec![("tools/custom/big", PlaceProvisionRowOutcome::Skipped, Some("needs about 2 GB, 3 GB free, keeping 2 GB free"))]);
    // Every df was handed wsp's install folder, or the nearest folder above it that is there.
    fn above(dir: &str) -> Vec<String> {
        if dir == "/" {
            return vec!["/".to_string()];
        }
        let parent = Path::new(dir).parent().map(|p| p.to_str().unwrap().to_string()).unwrap_or_else(|| "/".to_string());
        let mut out = vec![dir.to_string()];
        out.extend(above(&parent));
        out
    }
    for folder in handed() {
        assert!(above(TOOL_PREFIX).contains(&folder), "df was handed {folder}");
        assert!(Path::new(&folder).exists(), "{folder} is not there");
    }
}

// --- golden.test.ts: the tools loop on a machine that is not a fresh builder -----------------------------------

/// A machine that answers df from plenty and everything else with exit 0, recording what it was sent.
fn loop_machine() -> (FakeMachine, Arc<Mutex<Vec<String>>>) {
    let calls: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    let machine = answering(move |cmd| {
        seen.lock().unwrap().push(cmd.to_string());
        if cmd == FREE {
            res(0, &format!("{}\n", 9_000_000), "")
        } else {
            ok()
        }
    });
    (machine, calls)
}

fn node_and_codex() -> Vec<ToolInstall> {
    vec![
        script_step("agents/node", "Node 22.23.2", "install node"),
        ToolInstall {
            id: "agents/codex".to_string(),
            label: "Codex".to_string(),
            manager: "npm".to_string(),
            cmd: "install codex".to_string(),
            after: Some("agents/node".to_string()),
            bin: Some("codex".to_string()),
            ..Default::default()
        },
    ]
}

fn twenty_checked() -> Vec<ToolInstall> {
    (0..20)
        .map(|i| ToolInstall {
            id: format!("tools/brew/f{i}"),
            label: format!("f{i}"),
            manager: "brew".to_string(),
            cmd: format!("install f{i}"),
            check: Some(format!("brew list --versions f{i}")),
            ..Default::default()
        })
        .collect()
}

fn checks_in(text: &str) -> usize {
    text.split('\n').filter(|l| l.contains("wsp-check")).count()
}

// TS: packages/engine/test/golden.test.ts:2980
// "reads the checks a page at a time, so a recipe with a dozen of them never asks one exec to run them all"
#[tokio::test(flavor = "multi_thread")]
async fn reads_the_checks_a_page_at_a_time_so_a_recipe_with_a_dozen_never_asks_one_exec_to_run_them_all() {
    // A check can be a brew list of about a second, which is why the presence read is paged: one read of twenty
    // of them reaches the inline bound, and a bound reached there used to fail every checked row on the machine.
    let many = twenty_checked();
    let (machine, calls) = loop_machine();
    let tools = install_tools(&machine, &many, quiet_stage(), InstallToolsOptions::default()).await.unwrap();
    let calls = calls.lock().unwrap().clone();
    let reads: Vec<&String> = calls.iter().filter(|c| c.contains("wsp-check")).collect();
    // Twenty checks, eight to a page: three pages, and every page opens with the tools PATH.
    assert_eq!(reads.len(), many.len().div_ceil(READS_PER_EXEC));
    for read in &reads {
        assert!(checks_in(read) <= READS_PER_EXEC, "a page carried {} checks", checks_in(read));
    }
    // No exec carries the lot, which is what reached the bound and failed every checked row at once.
    assert!(!reads.iter().any(|r| checks_in(r) == many.len()));
    assert!(reads.iter().all(|r| r.starts_with(&format!("export PATH={TOOLS_PATH}"))));
    assert!(tools.iter().all(|t| t.outcome == "installed"));
}

// TS: packages/engine/test/golden.test.ts:2996
// "fails only the rows of a page it could not run, not every checked row on the machine"
#[tokio::test(flavor = "multi_thread")]
async fn fails_only_the_rows_of_a_page_it_could_not_run_not_every_checked_row_on_the_machine() {
    let many = twenty_checked();
    let read = Arc::new(AtomicUsize::new(0));
    let calls: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    // The second page of checks runs its deadline out, as a page of brew lists on a slow box would.
    let exec: ExecFn = Arc::new(move |cmd, _| {
        seen.lock().unwrap().push(cmd.clone());
        let said = if cmd == FREE {
            res(0, &format!("{}\n", 9_000_000), "")
        } else if !cmd.contains("wsp-check") {
            ok()
        } else if read.fetch_add(1, Ordering::SeqCst) + 1 == 2 {
            res(124, "", "")
        } else {
            ok()
        };
        Box::pin(async move { Ok(said) })
    });
    let run: RunFn = Arc::new(|_, _| Box::pin(async { Ok(ok()) }));
    let machine = FakeMachine { id: "spoo".to_string(), exec, run: Some(run), download_url: None, upload_url: None, put_bytes: None };
    let tools = install_tools(&machine, &many, quiet_stage(), InstallToolsOptions::default()).await.unwrap();
    let calls = calls.lock().unwrap().clone();
    assert_eq!(calls.iter().filter(|c| c.contains("wsp-check")).count(), many.len().div_ceil(READS_PER_EXEC));
    let failed: Vec<&ToolResult> = tools.iter().filter(|t| t.outcome == "failed").collect();
    // The eight rows that page carried, and no others: the rows on either side of it kept what they landed as.
    assert_eq!(failed.len(), READS_PER_EXEC);
    let failed_labels: Vec<&str> = failed.iter().map(|t| t.label.as_str()).collect();
    let page_labels: Vec<&str> = many[8..16].iter().map(|t| t.label.as_str()).collect();
    assert_eq!(failed_labels, page_labels);
    for t in &failed {
        assert!(t.note.as_deref().unwrap_or_default().contains("the check could not be run"), "note was {:?}", t.note);
    }
    assert_eq!(tools.iter().filter(|t| t.outcome == "installed").count(), many.len() - READS_PER_EXEC);
}

// TS: packages/engine/test/golden.test.ts:3022
// "keeps the shared Homebrew step as it landed when a formula of its own failed, since its check reads what it installs"
#[tokio::test(flavor = "multi_thread")]
async fn keeps_the_shared_homebrew_step_as_it_landed_when_a_formula_of_its_own_failed() {
    // The plan a recipe with two formulae gets: Homebrew, its toolchain, the shared step, then each formula. One
    // formula's install fails; the dependencies the shared step put on are there, and that row did its work.
    // Adapted: tool_installs_for labels each row with its id where the TS rows say "gh" and "yq"; no assertion reads a label.
    let plan = wsp_engine::golden_seam::tool_installs_for(&["tools/brew/gh", "tools/brew/yq"], TOOLS_PATH, None);
    let shared = plan.iter().find(|t| t.id == "tools/brew-shared").expect("the plan has the shared Homebrew step");
    // The check reads the dependencies, derived the way the install derives them, not the formulae themselves.
    let check = shared.check.as_deref().expect("the shared step has a check");
    assert!(check.contains("deps --for-each"));
    assert!(check.contains("list --versions $shared"));
    let machine = answering_split(
        |cmd| {
            if cmd == FREE {
                return res(0, &format!("{}\n", 9_000_000), "");
            }
            if !cmd.contains("wsp-check") {
                return ok();
            }
            // gh is not installed, so its own check fails; every other check on the page passes.
            match cmd.split('\n').position(|l| l.contains("list --versions gh")) {
                None => ok(),
                Some(at) => res(0, &format!("wsp-check {} Error: No available formula gh\n", at as i64 - 1), ""),
            }
        },
        |script| if script.contains("install gh") { res(1, "", "Error: gh did not build") } else { ok() },
    );
    let tools = install_tools(&machine, &plan, quiet_stage(), InstallToolsOptions::default()).await.unwrap();
    let row = |id: &str| tools.iter().find(|t| t.id == id).unwrap_or_else(|| panic!("no row {id}"));
    assert_eq!(row("tools/brew/gh").outcome, "failed");
    assert_eq!(row("tools/brew-shared").outcome, "installed");
    assert_eq!(row("tools/brew/yq").outcome, "installed");
}

/// A machine whose execs and whose runs answer from their own closures.
fn answering_split(on_exec: impl Fn(&str) -> ExecResult + 'static, on_run: impl Fn(&str) -> ExecResult + 'static) -> FakeMachine {
    let exec: ExecFn = Arc::new(move |cmd, _| {
        let said = on_exec(&cmd);
        Box::pin(async move { Ok(said) })
    });
    let run: RunFn = Arc::new(move |script, _| {
        let said = on_run(&script);
        Box::pin(async move { Ok(said) })
    });
    FakeMachine { id: "spoo".to_string(), exec, run: Some(run), download_url: None, upload_url: None, put_bytes: None }
}

// TS: packages/engine/test/golden.test.ts:3054
// "pushes a step the caller already found on the machine as installed, runs nothing for it, and lets what waits on it run"
#[tokio::test(flavor = "multi_thread")]
async fn pushes_a_step_the_caller_already_found_on_the_machine_as_installed_and_runs_nothing_for_it() {
    let (machine, calls) = loop_machine();
    let opts = InstallToolsOptions { present: BTreeSet::from(["agents/node".to_string()]), ..Default::default() };
    let tools = install_tools(&machine, &node_and_codex(), quiet_stage(), opts).await.unwrap();
    let got: Vec<(&str, &str, Option<&str>)> = tools.iter().map(|t| (t.id.as_str(), t.outcome.as_str(), t.note.as_deref())).collect();
    assert_eq!(got, vec![("agents/node", "installed", Some(ALREADY_ON_MACHINE)), ("agents/codex", "installed", None)]);
    let calls = calls.lock().unwrap().clone();
    assert!(!calls.iter().any(|c| c.contains("install node")));
    assert!(calls.iter().any(|c| c.contains("install codex")));
}

// TS: packages/engine/test/golden.test.ts:3065
// "keeps the caches where the machine is somebody's own computer, and sweeps them where it becomes an image"
#[tokio::test(flavor = "multi_thread")]
async fn keeps_the_caches_where_the_machine_is_somebodys_own_computer_and_sweeps_them_where_it_becomes_an_image() {
    let (kept_machine, kept) = loop_machine();
    install_tools(&kept_machine, &node_and_codex(), quiet_stage(), InstallToolsOptions { keep_caches: true, ..Default::default() })
        .await
        .unwrap();
    let kept = kept.lock().unwrap().clone();
    for needle in ["/root/.npm", "apt-get clean", "go clean -cache"] {
        assert!(!kept.iter().any(|c| c.contains(needle)), "{needle} ran on a computer that keeps its caches");
    }
    let (swept_machine, swept) = loop_machine();
    install_tools(&swept_machine, &node_and_codex(), quiet_stage(), InstallToolsOptions::default()).await.unwrap();
    let swept = swept.lock().unwrap().clone();
    for needle in ["/root/.npm", "apt-get clean"] {
        assert!(swept.iter().any(|c| c.contains(needle)), "{needle} did not run on an image");
    }
}

// TS: packages/engine/test/golden.test.ts:3074
// "hands each row to the caller watching the moment its outcome exists, in the order the loop reached them"
#[tokio::test(flavor = "multi_thread")]
async fn hands_each_row_to_the_caller_watching_the_moment_its_outcome_exists_in_the_order_the_loop_reached_them() {
    let (machine, _) = loop_machine();
    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let watcher = seen.clone();
    let on_tool: OnTool = Arc::new(move |t: ToolResult| {
        watcher.lock().unwrap().push(t.id);
        Box::pin(async {})
    });
    let opts = InstallToolsOptions { on_tool: Some(on_tool), ..Default::default() };
    let tools = install_tools(&machine, &node_and_codex(), quiet_stage(), opts).await.unwrap();
    let seen = seen.lock().unwrap().clone();
    assert_eq!(seen, vec!["agents/node".to_string(), "agents/codex".to_string()]);
    let ids: Vec<String> = tools.iter().map(|t| t.id.clone()).collect();
    assert_eq!(ids, seen);
}
