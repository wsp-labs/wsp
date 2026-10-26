// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/provision-outside.test.ts at a4033dc34. What a setup on a computer somebody owns writes
//! outside the agents' homes: the record and the leave are run by a real shell over a scratch root standing in for that
//! computer's file system, since what they are for is telling a file wsp wrote from one the computer already had.
mod common;

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use common::{ok, res, FakeMachine};
use wsp_engine::golden_seam::{free_kb_cmd, ToolInstall};
use wsp_engine::provision::ProvisionPlan;
use wsp_engine::provision::{new_setup_run, outside_roots, provision_step, EngineStep, ProvisionOn};
use wsp_engine::provision_files::{outside_after_script, outside_before_script, outside_marks, outside_sweep_script};
use wsp_wire::types::daemon_contract::{TOOLS_PATH, TOOL_PREFIX};

// Adapted: the TS puts a node-written sha256sum first on PATH because a Mac may carry none; this box's own
// coreutils answers the flags the scripts use, so no shim is made.

/// A computer's file system under /tmp, read by its real path: the leave reads each folder on the way physically.
fn scratch() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::Builder::new().prefix("wsp-outside-").tempdir().unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    fs::create_dir_all(root.join("usr/local/bin")).unwrap();
    fs::create_dir_all(root.join("opt")).unwrap();
    (dir, root)
}

fn root_str(root: &Path) -> String {
    root.to_str().unwrap().to_string()
}

/// `join(root, TOOL_PREFIX, ...)` as node joins it: the prefix's own leading slash does not reset the path.
fn wsp_dir(root: &Path) -> PathBuf {
    root.join(TOOL_PREFIX.trim_start_matches('/'))
}

fn sh_status(script: &str, first: Option<&Path>) -> std::process::Output {
    let path = match first {
        Some(dir) => format!("{}:{}", dir.display(), std::env::var("PATH").unwrap_or_default()),
        None => std::env::var("PATH").unwrap_or_default(),
    };
    Command::new("/bin/sh").arg("-c").arg(script).env("PATH", path).output().unwrap()
}

fn sh_in(script: &str, first: Option<&Path>) -> String {
    let out = sh_status(script, first);
    assert!(out.status.success(), "sh failed with {:?}: {}", out.status, String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

fn sh(script: &str) -> String {
    sh_in(script, None)
}

/// A folder holding one stand-in command, put first on PATH for one run: how a run killed part way is made.
fn stubbed(name: &str, body: &str) -> tempfile::TempDir {
    let dir = tempfile::Builder::new().prefix("wsp-outside-stub-").tempdir().unwrap();
    let path = dir.path().join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    dir
}

const EVERYWHERE: [&str; 2] = ["/usr/local", "/opt"];

fn put(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn walked_of(walked: &[&str]) -> Vec<String> {
    walked.iter().map(|w| w.to_string()).collect()
}

/// One install step as the setup brackets it: the listing before, what the step writes, the record after.
fn step_in(root: &Path, name: &str, writes: impl FnOnce(), walked: &[&str]) {
    let walked = walked_of(walked);
    sh(&outside_before_script(name, &walked, &root_str(root)));
    writes();
    sh(&outside_after_script(name, &walked, &root_str(root)));
}

fn step(root: &Path, name: &str, writes: impl FnOnce()) {
    step_in(root, name, writes, &EVERYWHERE);
}

fn ledger(root: &Path) -> String {
    fs::read_to_string(wsp_dir(root).join("landed")).unwrap()
}

fn paths_of(ledger: &str) -> Vec<String> {
    ledger.trim().split('\n').map(|line| line.split('\t').nth(1).unwrap().to_string()).collect()
}

fn listed(root: &Path) -> Vec<String> {
    if wsp_dir(root).join("landed").exists() {
        ledger(root).trim().split('\n').filter(|l| !l.is_empty()).map(|line| line.split('\t').nth(1).unwrap().to_string()).collect()
    } else {
        vec![]
    }
}

fn sweep(root: &Path) -> Vec<String> {
    outside_marks(&sh(&outside_sweep_script(&root_str(root))))
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

fn j(root: &Path, rel: &str) -> String {
    root.join(rel).to_str().unwrap().to_string()
}

// TS: packages/engine/test/provision-outside.test.ts:70
// writes down every path an install step made under /usr/local and /opt, and nothing the computer had before
#[test]
fn writes_down_every_path_an_install_step_made_under_usr_local_and_opt_and_nothing_the_computer_had_before() {
    let (_keep, root) = scratch();
    put(&root.join("usr/local/bin/jq"), "the box's own jq\n");
    put(&root.join("usr/local/bin/claude"), "the box's own claude\n");
    step(&root, "agents", || {
        put(&root.join("usr/local/bin/claude"), "claude 2.1.280\n");
        put(&root.join("usr/local/bin/gopls"), "gopls from go install\n");
        put(&root.join("opt/cursor-agent/cursor-agent"), "cursor\n");
        symlink(root.join("opt/cursor-agent/cursor-agent"), root.join("usr/local/bin/cursor-agent")).unwrap();
        put(&wsp_dir(&root).join("go/bin/x"), "x\n");
    });
    let paths = sorted(paths_of(&ledger(&root)));
    assert_eq!(
        paths,
        sorted(vec![
            j(&root, "opt/cursor-agent"),
            j(&root, "opt/cursor-agent/cursor-agent"),
            j(&root, "usr/local/bin/cursor-agent"),
            j(&root, "usr/local/bin/gopls")
        ])
    );
    assert!(!ledger(&root).contains(".landing"));
    assert!(!wsp_dir(&root).join(".landing-agents.before").exists());
}

// TS: packages/engine/test/provision-outside.test.ts:92
// leaves off a path a package put there, which is dpkg's to take
#[test]
fn leaves_off_a_path_a_package_put_there_which_is_dpkgs_to_take() {
    let (_keep, root) = scratch();
    put(&root.join("var/lib/dpkg/info/vendor.list"), "/.\n/opt\n/opt/vendor\n/opt/vendor/bin\n/opt/vendor/bin/v\n");
    step(&root, "clis", || {
        put(&root.join("opt/vendor/bin/v"), "a deb's file\n");
        put(&root.join("usr/local/bin/gopls"), "gopls\n");
    });
    assert_eq!(paths_of(&ledger(&root)), vec![j(&root, "usr/local/bin/gopls")]);
}

// TS: packages/engine/test/provision-outside.test.ts:102
// takes off exactly what the list names as wsp left it, and every folder it made once nothing is left in it
#[test]
fn takes_off_exactly_what_the_list_names_as_wsp_left_it_and_every_folder_it_made_once_nothing_is_left_in_it() {
    let (_keep, root) = scratch();
    put(&root.join("usr/local/bin/jq"), "the box's own jq\n");
    put(&root.join("usr/local/bin/claude"), "the box's own claude\n");
    step(&root, "clis", || {
        put(&root.join("usr/local/bin/claude"), "claude 2.1.280\n");
        put(&root.join("usr/local/bin/gopls"), "gopls\n");
        put(&root.join("opt/gcloud/bin/gcloud"), "gcloud\n");
        put(&root.join("opt/gcloud/lib/core.py"), "core\n");
        put(&root.join("opt/cursor-agent/cursor-agent"), "cursor\n");
        symlink(root.join("opt/gcloud/bin/gcloud"), root.join("usr/local/bin/gcloud")).unwrap();
    });
    fs::write(root.join("opt/cursor-agent/cursor-agent"), "the person's own build\n").unwrap();
    put(&root.join("opt/gcloud/lib/mine.txt"), "theirs\n");
    let swept = sweep(&root);
    assert_eq!(
        sorted(swept),
        sorted(vec![
            j(&root, "usr/local/bin/gopls"),
            j(&root, "usr/local/bin/gcloud"),
            j(&root, "opt/gcloud/bin/gcloud"),
            j(&root, "opt/gcloud/bin"),
            j(&root, "opt/gcloud/lib/core.py"),
        ])
    );
    assert_eq!(fs::read_to_string(root.join("usr/local/bin/claude")).unwrap(), "claude 2.1.280\n");
    assert_eq!(fs::read_to_string(root.join("usr/local/bin/jq")).unwrap(), "the box's own jq\n");
    assert_eq!(fs::read_to_string(root.join("opt/cursor-agent/cursor-agent")).unwrap(), "the person's own build\n");
    assert_eq!(fs::read_to_string(root.join("opt/gcloud/lib/mine.txt")).unwrap(), "theirs\n");
    assert!(root.join("usr/local/bin").exists());
}

// TS: packages/engine/test/provision-outside.test.ts:128
// writes a file down again at the bytes a later run left in it, and leaves the old line's bytes behind
#[test]
fn writes_a_file_down_again_at_the_bytes_a_later_run_left_in_it_and_leaves_the_old_lines_bytes_behind() {
    let (_keep, root) = scratch();
    step(&root, "agents", || put(&root.join("usr/local/bin/claude"), "claude 2.1.270\n"));
    step(&root, "agents", || put(&root.join("usr/local/bin/claude"), "claude 2.1.280\n"));
    assert_eq!(sweep(&root), vec![j(&root, "usr/local/bin/claude")]);
    assert!(!root.join("usr/local/bin/claude").exists());
}

// TS: packages/engine/test/provision-outside.test.ts:136
// takes nothing a line names outside /usr/local and /opt, under wsp's own folder, or past a folder that is a link
#[test]
fn takes_nothing_a_line_names_outside_usr_local_and_opt_under_wsps_own_folder_or_past_a_folder_that_is_a_link() {
    let (_keep, root) = scratch();
    let digest = |path: &str| sh(&format!("sha256sum < '{path}' | cut -c1-64")).trim().to_string();
    put(&root.join("etc/passwd"), "root:x:0:0\n");
    put(&root.join("usr/bin/env"), "env\n");
    put(&wsp_dir(&root).join("keep"), "wsp's own\n");
    put(&root.join("opt/real/tool"), "theirs\n");
    symlink(root.join("opt/real"), root.join("opt/via")).unwrap();
    let forged = [
        j(&root, "etc/passwd"),
        j(&root, "usr/bin/env"),
        j(&wsp_dir(&root), "keep"),
        j(&root, "opt/via/tool"),
        format!("{}/../../bin/env", j(&root, "usr/local/bin")),
    ];
    let ledger_path = wsp_dir(&root).join("landed");
    let text: String = forged
        .iter()
        .map(|p| format!("{}\t{p}\n", digest(&if Path::new(p).exists() { p.clone() } else { j(&root, "usr/bin/env") })))
        .collect();
    fs::write(&ledger_path, text).unwrap();
    assert_eq!(sweep(&root), Vec::<String>::new());
    for p in [j(&root, "etc/passwd"), j(&root, "usr/bin/env"), j(&wsp_dir(&root), "keep"), j(&root, "opt/real/tool")] {
        assert!(Path::new(&p).exists(), "{p}");
    }
}

// TS: packages/engine/test/provision-outside.test.ts:151
// reads nothing off a list that sits in a prefix that is a link
#[test]
fn reads_nothing_off_a_list_that_sits_in_a_prefix_that_is_a_link() {
    let (_keep, root) = scratch();
    let elsewhere = root.join("elsewhere");
    put(&root.join("usr/local/bin/gopls"), "gopls\n");
    let gopls = j(&root, "usr/local/bin/gopls");
    put(&elsewhere.join("landed"), &format!("{}\t{gopls}\n", sh(&format!("sha256sum < '{gopls}' | cut -c1-64")).trim()));
    fs::create_dir_all(wsp_dir(&root).parent().unwrap()).unwrap();
    symlink(&elsewhere, wsp_dir(&root)).unwrap();
    assert_eq!(sweep(&root), Vec::<String>::new());
    assert!(root.join("usr/local/bin/gopls").exists());
}

// TS: packages/engine/test/provision-outside.test.ts:161
// records nothing when the listing before the step was cut short, so the box's own files are never claimed
#[test]
fn records_nothing_when_the_listing_before_the_step_was_cut_short_so_the_boxs_own_files_are_never_claimed() {
    let (_keep, root) = scratch();
    put(&root.join("usr/local/bin/jq"), "the box's own jq\n");
    put(&root.join("opt/vendor/tool"), "the box's own tool\n");
    let everywhere = walked_of(&EVERYWHERE);
    let killed = stubbed("find", "echo \"$2\"; kill -KILL $$");
    sh_in(&outside_before_script("clis", &everywhere, &root_str(&root)), Some(killed.path()));
    put(&root.join("usr/local/bin/claude"), "claude\n");
    sh(&outside_after_script("clis", &everywhere, &root_str(&root)));
    assert_eq!(listed(&root), Vec::<String>::new());
    sh(&outside_before_script("clis", &everywhere, &root_str(&root)));
    let before = wsp_dir(&root).join(".landing-clis.before");
    let head: Vec<String> = fs::read_to_string(&before).unwrap().split('\n').take(2).map(str::to_string).collect();
    fs::write(&before, format!("{}\n", head.join("\n"))).unwrap();
    put(&root.join("usr/local/bin/gopls"), "gopls\n");
    sh(&outside_after_script("clis", &everywhere, &root_str(&root)));
    assert_eq!(listed(&root), Vec::<String>::new());
    assert_eq!(sweep(&root), Vec::<String>::new());
    assert!(root.join("usr/local/bin/jq").exists() && root.join("opt/vendor/tool").exists());
    let left: Vec<String> = fs::read_dir(wsp_dir(&root))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.starts_with(".landing"))
        .collect();
    assert_eq!(left, Vec::<String>::new());
}

// TS: packages/engine/test/provision-outside.test.ts:183
// records and takes four thousand files well inside the leave's sixty seconds
#[test]
fn records_and_takes_four_thousand_files_well_inside_the_leaves_sixty_seconds() {
    let (_keep, root) = scratch();
    let started = Instant::now();
    step(&root, "clis", || {
        for i in 0..4000 {
            put(&root.join("opt/gcloud").join(format!("d{}", i % 40)).join(format!("f{i}")), &format!("file {i}\n"));
        }
    });
    let recorded = started.elapsed().as_millis();
    assert_eq!(listed(&root).len(), 4041);
    let sweeping = Instant::now();
    let swept = sweep(&root);
    let took = sweeping.elapsed().as_millis();
    assert_eq!(swept.len(), 4041);
    assert!(!root.join("opt/gcloud").exists());
    assert!(recorded < 20_000, "the record took {recorded} ms");
    assert!(took < 20_000, "the sweep took {took} ms");
}

// TS: packages/engine/test/provision-outside.test.ts:200
// keeps the list when the leave is cut short, and a second leave finishes it
#[test]
fn keeps_the_list_when_the_leave_is_cut_short_and_a_second_leave_finishes_it() {
    let (_keep, root) = scratch();
    step(&root, "agents", || {
        put(&root.join("usr/local/bin/claude"), "claude\n");
        put(&root.join("opt/cursor/cursor"), "cursor\n");
    });
    let cut = stubbed("xargs", "kill -KILL $PPID");
    // The shell died, which is the point: its exit is not read.
    let _ = sh_status(&outside_sweep_script(&root_str(&root)), Some(cut.path()));
    assert!(wsp_dir(&root).join("landed").exists());
    assert_eq!(sorted(sweep(&root)), vec![j(&root, "opt/cursor"), j(&root, "opt/cursor/cursor"), j(&root, "usr/local/bin/claude")]);
    assert!(!wsp_dir(&root).join("landed").exists());
}

// TS: packages/engine/test/provision-outside.test.ts:218
// walks only the folders the step's roads write, so a file somebody else puts elsewhere meanwhile stays theirs
#[test]
fn walks_only_the_folders_the_steps_roads_write_so_a_file_somebody_else_puts_elsewhere_meanwhile_stays_theirs() {
    let (_keep, root) = scratch();
    step_in(
        &root,
        "clis",
        || {
            put(&root.join("usr/local/bin/rg"), "rg from its release\n");
            put(&root.join("opt/theirs/tool"), "installed by somebody else meanwhile\n");
            put(&root.join("usr/local/lib/theirs.so"), "theirs too\n");
        },
        &["/usr/local/bin"],
    );
    assert_eq!(listed(&root), vec![j(&root, "usr/local/bin/rg")]);
}

// TS: packages/engine/test/provision-outside.test.ts:233
// never writes down the person's bytes when a later step only touches a file of wsp's they wrote over
#[test]
fn never_writes_down_the_persons_bytes_when_a_later_step_only_touches_a_file_of_wsps_they_wrote_over() {
    let (_keep, root) = scratch();
    step(&root, "agents", || put(&root.join("usr/local/bin/claude"), "claude 2.1.280\n"));
    fs::write(root.join("usr/local/bin/claude"), "the person's own build\n").unwrap();
    step(&root, "clis", || fs::set_permissions(root.join("usr/local/bin/claude"), fs::Permissions::from_mode(0o755)).unwrap());
    assert_eq!(sweep(&root), Vec::<String>::new());
    assert_eq!(fs::read_to_string(root.join("usr/local/bin/claude")).unwrap(), "the person's own build\n");
}

// TS: packages/engine/test/provision-outside.test.ts:242
// answers only the mark's own lines, each an absolute path
#[test]
fn answers_only_the_marks_own_lines_each_an_absolute_path() {
    assert_eq!(
        outside_marks("wsp-outside\t/usr/local/bin/claude\nsomething else\nwsp-outside\trelative\nwsp-outside\t\n"),
        vec!["/usr/local/bin/claude".to_string()]
    );
}

fn recording() -> (FakeMachine, Arc<Mutex<Vec<String>>>) {
    let ran = Arc::new(Mutex::new(Vec::<String>::new()));
    let exec_ran = ran.clone();
    let run_ran = ran.clone();
    let mut machine = FakeMachine::new(Arc::new(move |cmd: String, _| {
        exec_ran.lock().unwrap().push(cmd.clone());
        let answer = if cmd == free_kb_cmd() { res(0, "9000000\n", "") } else { ok() };
        Box::pin(async move { Ok(answer) })
    }));
    machine.run = Some(Arc::new(move |script: String, _| {
        run_ran.lock().unwrap().push(script);
        Box::pin(async { Ok(ok()) })
    }));
    (machine, ran)
}

fn plan(prefix: Option<&str>) -> ProvisionPlan {
    ProvisionPlan {
        recipe_at: "x".to_string(),
        path: TOOLS_PATH.to_string(),
        steps: vec![],
        skipped: vec![],
        agents: 0,
        compiler: false,
        prefix: prefix.map(str::to_string),
        ..Default::default()
    }
}

fn on_root() -> ProvisionOn {
    ProvisionOn { home: "/root".to_string(), held: None, stores: None }
}

// TS: packages/engine/test/provision-outside.test.ts:267
// lists the folders before the floor and writes down what it made after, under wsp's prefix only
#[tokio::test]
async fn lists_the_folders_before_the_floor_and_writes_down_what_it_made_after_under_wsps_prefix_only() {
    let (machine, ran) = recording();
    provision_step(&machine, &plan(Some(TOOL_PREFIX)), EngineStep::Floor, new_setup_run(), Arc::new(|_, _, _| {}), &on_root())
        .await
        .unwrap();
    let walked = outside_roots(&plan(Some(TOOL_PREFIX)), EngineStep::Floor);
    let ran_owned = ran.lock().unwrap().clone();
    assert_eq!(ran_owned.first().unwrap(), &outside_before_script("floor", &walked, ""));
    assert_eq!(ran_owned.last().unwrap(), &outside_after_script("floor", &walked, ""));
    let (image, image_ran) = recording();
    provision_step(&image, &plan(None), EngineStep::Floor, new_setup_run(), Arc::new(|_, _, _| {}), &on_root()).await.unwrap();
    assert!(!image_ran.lock().unwrap().contains(&outside_before_script("floor", &walked, "")));
}

// TS: packages/engine/test/provision-outside.test.ts:278
// walks the folders each road of the step writes under /usr/local and /opt, wsp's own folder left out
#[test]
fn walks_the_folders_each_road_of_the_step_writes_under_usr_local_and_opt_wsps_own_folder_left_out() {
    let tool = |manager: &str, bins: Option<Vec<&str>>| ToolInstall {
        id: format!("tools/{manager}"),
        label: manager.to_string(),
        manager: manager.to_string(),
        cmd: "true".to_string(),
        bins: bins.map(|b| b.into_iter().map(str::to_string).collect()),
        ..Default::default()
    };
    let with = |steps: Vec<ToolInstall>| ProvisionPlan { steps, agents: 0, ..plan(Some(TOOL_PREFIX)) };
    let roots = |steps: Vec<ToolInstall>| outside_roots(&with(steps), EngineStep::Clis);
    assert_eq!(roots(vec![tool("release", None)]), vec!["/usr/local/bin".to_string()]);
    assert_eq!(roots(vec![tool("go", Some(vec!["/usr/local/bin"]))]), vec!["/usr/local/bin".to_string()]);
    assert_eq!(roots(vec![tool("npm", None)]), vec!["/usr/local/bin".to_string(), "/usr/local/lib/node_modules".to_string()]);
    assert_eq!(roots(vec![tool("vendor", None)]), vec!["/opt".to_string(), "/usr/local/bin".to_string()]);
    assert_eq!(roots(vec![tool("script", None)]), vec!["/opt".to_string(), "/usr/local".to_string()]);
    assert_eq!(roots(vec![]), Vec::<String>::new());
}

// TS: packages/engine/test/provision-outside.test.ts:290
// leaves a step that installs nothing outside the home alone
#[tokio::test]
async fn leaves_a_step_that_installs_nothing_outside_the_home_alone() {
    let (machine, ran) = recording();
    provision_step(&machine, &plan(Some(TOOL_PREFIX)), EngineStep::Skills, new_setup_run(), Arc::new(|_, _, _| {}), &on_root())
        .await
        .unwrap();
    assert_eq!(*ran.lock().unwrap(), Vec::<String>::new());
}

// TS: packages/engine/test/provision-outside.test.ts:301
// landed-outside.sh equals its regeneration, so the twin the daemon carries is this script
#[test]
fn landed_outside_sh_equals_its_regeneration_so_the_twin_the_daemon_carries_is_this_script() {
    let text = format!("{}\n", outside_sweep_script("{root}"));
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../daemon/fixtures/contract/landed-outside.sh");
    let on_disk = fs::read_to_string(path).unwrap_or_else(|e| {
        panic!("daemon/fixtures/contract/landed-outside.sh is missing ({e}); regenerate it from the engine and commit it")
    });
    assert_eq!(on_disk, text, "daemon/fixtures/contract/landed-outside.sh is behind the engine");
}
