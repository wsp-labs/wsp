// SPDX-License-Identifier: AGPL-3.0-only
//! spec/landing_roads.md: the three landing roads against the TS oracle (tests/fixtures/core_rest/landing_roads.json,
//! oracle/core_rest.oracle.ts "project landing roads"), each command a road ran read back off a machine that answers
//! by needle, as the oracle's did. Also ports packages/runtime/test/box-folder.test.ts's "the road a project on a
//! joined computer lands by" (:578, :590). project-landing.test.ts drives `createRuntime` and belongs to the projects
//! area.
mod rest;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use futures::FutureExt;
use indexmap::IndexMap;
use rest::*;
use serde_json::{json, Value};
use wsp_core::console;
use wsp_core::project_landing::{project_landing, project_landings, LandRequest, LandSeed, LandingComputer, LandingDeps, LandingReport, ProjectLanding};
use wsp_core::project_sources::{project_source, ProjectSourceModule};
use wsp_engine::Machine;
use wsp_wire::views::workspace::{ProjectSource, ProjectView};
use wsp_words::exit::HostError;
use wsp_words::project_path::place_owned_paths;

fn fx() -> Value {
    fixture("landing_roads")
}

/// The oracle's `project(over)`.
fn project(over: Value) -> ProjectView {
    let mut p = json!({ "id": "pr_1", "name": "spoo-ts", "computer": "p", "source": { "kind": "git", "url": "https://github.com/acme/lab.git" }, "path": "", "remote": "https://github.com/acme/lab.git", "defaultBranch": "main", "memoryKey": "", "memoryDir": "", "createdAt": "" });
    for (k, v) in over.as_object().unwrap() {
        p[k] = v.clone();
    }
    wire(&p)
}

/// What a road's `deps.land` was handed: the path and the bytes as text.
type Lands = Rc<RefCell<Vec<Value>>>;

/// The oracle's `deps(m, over)`: a computer with home /root named hetzner, the scratch /scratch, a Mac state home, a
/// folder here whose branch is main, and a clock at 2026-10-25T10:00:00.000Z.
fn deps(m: Option<Rc<RecMachine>>, home: &str, lands: &Lands) -> LandingDeps {
    let l = lands.clone();
    LandingDeps {
        land: Rc::new(move |_m: Arc<dyn Machine>, path: String, bytes: Vec<u8>| {
            l.borrow_mut().push(json!({ "path": path, "bytes": String::from_utf8(bytes).unwrap() }));
            async { Ok::<(), HostError>(()) }.boxed_local()
        }),
        scratch: Rc::new(|_m: &dyn Machine| "/scratch".to_string()),
        computer: m.map(|m| LandingComputer { machine: Arc::new(Shared(m)), home: home.into() }),
        mac_state_home: "/Users/dev/.claude".into(),
        branch_here: Rc::new(|p: &str| {
            let b = (p == "/Users/dev/web").then(|| "main".to_string());
            async move { b }.boxed_local()
        }),
        computer_name: "hetzner".into(),
        clone_env: None,
        now: Rc::new(|| 1_792_922_400_000),
    }
}

/// A `RecMachine` behind an `Arc`, as a computer's machine is held, sharing the test's handle.
struct Shared(Rc<RecMachine>);

#[async_trait::async_trait(?Send)]
impl Machine for Shared {
    fn id(&self) -> &str {
        self.0.id()
    }
    fn kind(&self) -> wsp_engine::machine::MachineKind {
        self.0.kind()
    }
    fn takes_stdin(&self) -> bool {
        true
    }
    async fn exec(&self, cmd: &str, opts: wsp_engine::machine::ExecOptions) -> wsp_engine::errors::Result<wsp_engine::machine::ExecResult> {
        self.0.exec(cmd, opts).await
    }
    async fn run(&self, s: &str, o: wsp_engine::machine::RunOptions) -> wsp_engine::errors::Result<wsp_engine::machine::ExecResult> {
        self.0.run(s, o).await
    }
    async fn snapshot(&self, n: &str, l: wsp_engine::machine::MachineLife, o: wsp_engine::machine::SnapshotOptions) -> wsp_engine::errors::Result<String> {
        self.0.snapshot(n, l, o).await
    }
    async fn pause(&self, w: Option<u64>) -> wsp_engine::errors::Result<()> {
        self.0.pause(w).await
    }
    async fn resume(&self, s: Option<&wsp_engine::Cancel>) -> wsp_engine::errors::Result<()> {
        self.0.resume(s).await
    }
    async fn kill(&self) -> wsp_engine::errors::Result<()> {
        self.0.kill().await
    }
    async fn state(&self) -> wsp_engine::errors::Result<wsp_engine::machine::MachineState> {
        self.0.state().await
    }
    async fn download_url(&self, p: &str) -> wsp_engine::errors::Result<String> {
        self.0.download_url(p).await
    }
    async fn upload_url(&self, p: &str) -> wsp_engine::errors::Result<String> {
        self.0.upload_url(p).await
    }
}

fn reporter() -> (LandingReport, Rc<RefCell<Vec<Value>>>) {
    let out: Rc<RefCell<Vec<Value>>> = Rc::default();
    let o = out.clone();
    (Rc::new(move |stage, message: &str| o.borrow_mut().push(json!([json(&stage), message]))), out)
}

fn road(kind: &str) -> Rc<dyn ProjectLanding> {
    project_landing(kind).unwrap()
}

fn src(kind: &str) -> Rc<dyn ProjectSourceModule> {
    project_source(kind).unwrap()
}

fn out_of<T: serde::Serialize>(r: Result<T, HostError>) -> Value {
    match r {
        Ok(v) => json!({ "ok": json(&v) }),
        Err(e) => json!({ "err": e.message }),
    }
}

fn source(v: Value) -> ProjectSource {
    wire(&v)
}

// fixture: landing_roads.json kinds, unknown
#[test]
fn the_roads_are_box_provider_and_mac_and_any_other_kind_is_a_wiring_fault() {
    let f = fx();
    assert_eq!(json(&project_landings().keys().collect::<Vec<_>>()), f["kinds"]);
    for kind in ["box", "provider", "mac"] {
        assert_eq!(road(kind).kind(), kind);
    }
    assert_eq!(json!({ "err": project_landing("moon").err().unwrap().message }), f["unknown"]);
}

// fixture: landing_roads.json path
#[test]
fn each_road_names_the_folder_a_project_takes_on_its_computer() {
    let f = fx();
    let lands: Lands = Rc::default();
    let m = RecMachine::new(vec![]);
    let folder = source(json!({ "kind": "folder", "path": "/srv/legacy" }));
    let git = source(json!({ "kind": "git", "url": "https://github.com/acme/lab.git" }));
    let gh = source(json!({ "kind": "github", "repo": "acme/lab" }));
    let d = deps(Some(m.clone()), "/root", &lands);
    let none = LandingDeps { computer: None, ..d.clone() };
    let got = vec![
        json!(["box folder", road("box").path("legacy", &folder, &d).unwrap()]),
        json!(["box git", road("box").path("lab", &git, &deps(Some(m.clone()), "/home/dev/", &lands)).unwrap()]),
        json!(["box no computer", out_of(road("box").path("lab", &git, &none))["err"].clone().as_str().map(|e| json!({ "err": e })).unwrap()]),
        json!(["provider git", road("provider").path("lab", &git, &d).unwrap()]),
        json!(["provider folder", road("provider").path("legacy", &folder, &d).unwrap()]),
        json!(["mac folder", road("mac").path("legacy", &folder, &d).unwrap()]),
        json!(["mac github", json!({ "err": road("mac").path("lab", &gh, &d).unwrap_err().message })]),
    ];
    assert_eq!(Value::Array(got), f["path"]);
}

// fixture: landing_roads.json places
#[test]
fn each_road_keeps_the_memory_where_its_computer_reads_it() {
    let f = fx();
    let lands: Lands = Rc::default();
    let m = RecMachine::new(vec![]);
    let d = deps(Some(m.clone()), "/root", &lands);
    let dev = deps(Some(m), "/home/dev", &lands);
    let got = vec![
        json!(["box", json(&road("box").places(&project(json!({})), "-root-lab", &dev).unwrap())]),
        json!(["provider", json(&road("provider").places(&project(json!({})), "-root-lab", &d).unwrap())]),
        json!(["mac folder", json(&road("mac").places(&project(json!({ "source": { "kind": "folder", "path": "/srv/legacy" }, "path": "/elsewhere" })), "-srv-legacy", &d).unwrap())]),
        json!(["mac git", json(&road("mac").places(&project(json!({ "path": "/Users/dev/lab" })), "-Users-dev-lab", &d).unwrap())]),
    ];
    assert_eq!(Value::Array(got), f["places"]);
}

// fixture: landing_roads.json landsAtAdd, refusal, remove, binds, optional
#[test]
fn what_each_road_does_at_the_add_at_a_remove_and_for_every_workspace() {
    run_local(async {
        let f = fx();
        let lands: Lands = Rc::default();
        let m = RecMachine::new(vec![]);
        let d = deps(Some(m.clone()), "/root", &lands);
        let (b, p, mac) = (road("box"), road("provider"), road("mac"));
        assert_eq!(json!([b.lands_at_add(false), p.lands_at_add(true), mac.lands_at_add(true)]), f["landsAtAdd"]);
        let pr = project(json!({}));
        assert_eq!(json!([b.refusal(&pr, &d), p.refusal(&pr, &d), mac.refusal(&pr, &d)]), f["refusal"]);
        let removed = json!([
            b.remove(&project(json!({ "path": "/root/spoo-ts" })), &d).await.unwrap(),
            p.remove(&pr, &d).await.unwrap(),
            mac.remove(&pr, &d).await.unwrap()
        ]);
        assert_eq!(removed, f["remove"]);
        assert_eq!(json!([json(&b.workspace_binds(&pr)), json(&p.workspace_binds(&pr)), json(&mac.workspace_binds(&pr))]), f["binds"]);
        let mut offers = vec![];
        for (name, r) in [("box", &b), ("provider", &p), ("mac", &mac)] {
            let stands = r.folder_stands(&pr, &d).await.is_some();
            let into = r.seed_into(&pr, b"t".to_vec(), &d).await.is_some();
            let unsaved = r.unsaved(&pr, &d).await.is_some();
            offers.push(json!([name, stands, into, unsaved]));
        }
        assert_eq!(Value::Array(offers), f["optional"]);
        assert_eq!(m.cmds().iter().filter(|c| c.starts_with("test -e")).count(), 1, "only the box road read the folder");
    });
}

// fixture: landing_roads.json branch
#[test]
fn each_road_reads_the_branch_a_new_thread_starts_on() {
    run_local(async {
        let f = fx();
        let lands: Lands = Rc::default();
        let none = RecMachine::new(vec![]);
        let d = deps(Some(none.clone()), "/root", &lands);
        let read = |r: ExecResultLike| -> LandingDeps {
            let m = match r {
                ExecResultLike::Answer(code, out) => RecMachine::new(vec![("symbolic-ref", result(code, out, ""))]),
                ExecResultLike::Throws => Rc::new(RecMachine::plain(vec![]).failing("symbolic-ref")),
            };
            deps(Some(m), "/root", &lands)
        };
        let got = vec![
            json!(["provider base", json(&road("provider").branch(&project(json!({ "base": "dev" })), &d).await)]),
            json!(["provider default", json(&road("provider").branch(&project(json!({})), &d).await)]),
            json!(["mac here", json(&road("mac").branch(&project(json!({ "path": "/Users/dev/web" })), &d).await)]),
            json!(["mac detached", json(&road("mac").branch(&project(json!({ "path": "/Users/dev/other" })), &d).await)]),
            json!(["box main", json(&road("box").branch(&project(json!({ "path": "/root/x" })), &read(ExecResultLike::Answer(0, " main \n"))).await)]),
            json!(["box detached", json(&road("box").branch(&project(json!({ "path": "/root/x" })), &read(ExecResultLike::Answer(1, ""))).await)]),
            json!(["box empty", json(&road("box").branch(&project(json!({ "path": "/root/x" })), &read(ExecResultLike::Answer(0, "\n"))).await)]),
            json!(["box throws", json(&road("box").branch(&project(json!({ "path": "/root/x" })), &read(ExecResultLike::Throws)).await)]),
            json!(["box no computer", json(&road("box").branch(&project(json!({ "path": "/root/x" })), &LandingDeps { computer: None, ..d.clone() }).await)]),
        ];
        assert_eq!(Value::Array(got), f["branch"]);
    });
}

enum ExecResultLike {
    Answer(i64, &'static str),
    Throws,
}

// fixture: landing_roads.json folderStands, folderStandsCmd
#[test]
fn the_box_road_reads_whether_the_folder_stands_and_fails_on_a_read_that_broke() {
    run_local(async {
        let f = fx();
        let lands: Lands = Rc::default();
        let mut got = vec![];
        for code in [0, 1, 2] {
            let m = RecMachine::new(vec![("test -e", result(code, "", if code > 1 { "test: broke\n" } else { "" }))]);
            let r = road("box").folder_stands(&project(json!({ "path": "/srv/it's" })), &deps(Some(m.clone()), "/root", &lands)).await.expect("the box road reads it");
            got.push(json!([code, out_of(r)]));
            if code == 0 {
                assert_eq!(m.ran_json(), f["folderStandsCmd"]);
            }
        }
        assert_eq!(Value::Array(got), f["folderStands"]);
    });
}

// fixture: landing_roads.json seedInto
#[test]
fn the_box_road_puts_a_seeds_files_into_a_standing_folder_and_says_why_it_could_not() {
    run_local(async {
        let f = fx();
        let lands: Lands = Rc::default();
        let m = RecMachine::new(vec![]);
        road("box").seed_into(&project(json!({ "path": "/srv/legacy" })), b"tar".to_vec(), &deps(Some(m.clone()), "/home/dev", &lands)).await.unwrap().unwrap();
        assert_eq!(m.ran_json(), f["seedInto"]["ran"]);
        assert_eq!(Value::Array(lands.borrow().clone()), f["seedInto"]["land"]);
        let failing = RecMachine::new(vec![("tar -xzf", result(2, "", "tar: broke\n"))]);
        let r = road("box").seed_into(&project(json!({ "path": "/srv/legacy" })), b"t".to_vec(), &deps(Some(failing), "/root", &lands)).await.unwrap();
        assert_eq!(out_of(r.map(|()| Value::Null)), f["seedInto"]["fails"]);
    });
}

// fixture: landing_roads.json unsaved
#[test]
fn the_box_road_reads_what_a_checkout_holds_unpushed_and_nothing_where_it_holds_none() {
    run_local(async {
        let f = fx();
        let lands: Lands = Rc::default();
        let none = RecMachine::new(vec![]);
        assert_eq!(road("box").unsaved(&project(json!({ "path": "/root/x" })), &deps(Some(none.clone()), "/root", &lands)).await, Some(None));
        assert_eq!(none.cmds(), Vec::<String>::new());
        let m = RecMachine::new(vec![("rev-parse --git-dir", result(0, "1 0 0\n", ""))]);
        let said = road("box").unsaved(&project(json!({ "name": "web", "path": "/root/x", "checkout": "/root/web-2" })), &deps(Some(m.clone()), "/root", &lands)).await;
        assert_eq!(json!([["no checkout", null], ["checkout", said.flatten(), m.ran_json()]]), f["unsaved"]);
    });
}

/// One land through a road, as the oracle ran it: what it answered, every command, every report, every land.
async fn land(kind: &str, req: impl FnOnce(LandingReport) -> LandRequest, m: Option<Rc<RecMachine>>, clone_env: Option<IndexMap<String, String>>) -> (Value, Value, Value, Value) {
    let lands: Lands = Rc::default();
    let (report, reports) = reporter();
    let mut d = deps(m.clone(), "/root", &lands);
    if let Some(env) = clone_env {
        d.clone_env = Some(Rc::new(move || env.clone()));
    }
    let out = out_of(road(kind).land(req(report), &d).await);
    let ran = m.map(|m| m.ran_json()).unwrap_or(json!([]));
    let r = Value::Array(reports.borrow().clone());
    let l = Value::Array(lands.borrow().clone());
    (out, ran, r, l)
}

fn request(p: ProjectView, source: Rc<dyn ProjectSourceModule>, seed: Option<LandSeed>) -> impl FnOnce(LandingReport) -> LandRequest {
    move |report| LandRequest { project: p, source, seed, report }
}

fn seed_plan() -> Value {
    json!({ "source": "/Users/dev/web", "remote": "https://github.com/acme/lab.git", "branch": "refactor/x", "defaultBranch": "main", "unpushed": { "commits": 2, "base": "abc123" }, "uncommitted": 0, "memory": { "key": "-Users-dev-web", "files": 3, "bytes": 20 }, "files": [{ "path": ".env.local", "dir": false, "bytes": 12, "kind": "config", "row": { "id": "next", "name": "Next" }, "ticked": true }], "remembered": false })
}

// fixture: landing_roads.json land.plain (box-folder.test.ts:590 reads the same land for its log path)
#[test]
fn the_box_road_claims_a_folder_clones_and_installs_and_records_what_ran() {
    run_local(async {
        let f = fx();
        let m = RecMachine::new(vec![("mkdir '/root/spoo-ts'", result(0, "/root/spoo-ts\n", "")), ("ls -A", result(0, "package-lock.json\nREADME.md\n", ""))]);
        let (out, ran, reports, _) = land("box", request(project(json!({})), src("git"), None), Some(m), Some(IndexMap::new())).await;
        assert_eq!(out, f["land"]["plain"]["out"]);
        assert_eq!(ran, f["land"]["plain"]["ran"]);
        assert_eq!(reports, f["land"]["plain"]["reports"]);
    });
}

// TS: box-folder.test.ts:590
#[test]
fn writes_the_installs_log_under_a_path_a_leave_takes_so_a_leave_leaves_nothing_of_the_landings_in_wsps_folder() {
    run_local(async {
        let m = RecMachine::new(vec![("mkdir '/root/spoo-ts'", result(0, "/root/spoo-ts\n", "")), ("ls -A", result(0, "package-lock.json\n", ""))]);
        let (out, _, _, _) = land("box", request(project(json!({})), src("git"), None), Some(m.clone()), None).await;
        assert!(out.get("ok").is_some(), "{out}");
        let all = m.cmds().join("\n");
        let log = regex::Regex::new(r"npm ci >> '([^']+)' 2>&1").unwrap().captures(&all).map(|c| c[1].to_string()).unwrap_or_else(|| panic!("{all}"));
        assert!(place_owned_paths("/root").iter().any(|row| log.starts_with(&format!("{row}/"))), "{log}");
    });
}

// fixture: landing_roads.json land.vault
#[test]
fn the_box_road_clones_with_the_vaults_env_on_the_clones_input_and_installs_each_lockfile_in_order() {
    run_local(async {
        let f = fx();
        let m = RecMachine::new(vec![("mkdir '/root/spoo-ts'", result(0, "/root/spoo-ts-2\n", "")), ("ls -A", result(0, "Cargo.lock\npnpm-lock.yaml\n", ""))]);
        let env: IndexMap<String, String> = [("GH_TOKEN".to_string(), "gho_x".to_string())].into_iter().collect();
        let (out, ran, reports, _) = land("box", request(project(json!({ "base": "dev" })), src("github"), None), Some(m), Some(env)).await;
        assert_eq!(out, f["land"]["vault"]["out"]);
        assert_eq!(ran, f["land"]["vault"]["ran"]);
        assert_eq!(reports, f["land"]["vault"]["reports"]);
        assert!(!ran.to_string().contains("gho_x\\n") && !ran[1]["cmd"].as_str().unwrap().contains("gho_x"), "the token rides the input, never the command");
    });
}

// fixture: landing_roads.json land.seeded
#[test]
fn the_box_road_lands_a_seed_its_commits_and_keeps_the_memory_already_there() {
    run_local(async {
        let f = fx();
        let m = RecMachine::new(vec![
            ("mkdir '/root/spoo-ts'", result(0, "/root/spoo-ts\n", "")),
            ("symbolic-ref", result(0, "main\n", "")),
            ("am --3way", result(0, "x\n2\n", "")),
            ("rmdir", result(0, "wsp-memory-kept\n", "")),
        ]);
        let seed = LandSeed { tar: b"seed archive".to_vec(), choice: wire(&json!({ "files": [".env.local"], "memory": true, "commits": true })), plan: wire(&seed_plan()) };
        let p = project(json!({ "source": { "kind": "folder", "path": "/Users/dev/web" } }));
        let (out, ran, reports, lands) = land("box", request(p, src("folder"), Some(seed)), Some(m), None).await;
        assert_eq!(out, f["land"]["seeded"]["out"]);
        assert_eq!(ran, f["land"]["seeded"]["ran"]);
        assert_eq!(reports, f["land"]["seeded"]["reports"]);
        assert_eq!(lands, f["land"]["seeded"]["land"]);
    });
}

// fixture: landing_roads.json land.refused
#[test]
fn a_patch_git_refuses_is_put_back_and_said_once_and_the_add_still_lands() {
    run_local(async {
        let f = fx();
        let m = RecMachine::new(vec![
            ("mkdir '/root/spoo-ts'", result(0, "/root/spoo-ts\n", "")),
            ("symbolic-ref", result(128, "", "fatal: not a symbolic ref\n")),
            ("am --3way", result(1, "", "error: patch failed\n")),
        ]);
        let seed = LandSeed { tar: b"t".to_vec(), choice: wire(&json!({ "files": [], "memory": false, "commits": true })), plan: wire(&seed_plan()) };
        let p = project(json!({ "source": { "kind": "folder", "path": "/Users/dev/web" }, "base": "dev" }));
        let (out, ran, reports, _) = land("box", request(p, src("folder"), Some(seed)), Some(m), None).await;
        assert_eq!(out, f["land"]["refused"]["out"]);
        assert_eq!(ran, f["land"]["refused"]["ran"]);
        assert_eq!(reports, f["land"]["refused"]["reports"]);
    });
}

// fixture: landing_roads.json land.cloneFailed, land.installFailed, land.noFolder
#[test]
fn a_clone_or_an_install_that_fails_sweeps_the_folder_it_claimed_and_a_home_with_no_free_name_claims_none() {
    run_local(async {
        let f = fx();
        let clone = RecMachine::new(vec![("mkdir '/root/spoo-ts'", result(0, "/root/spoo-ts\n", "")), ("git clone", result(128, "", "Cloning...\nfatal: repository not found\n"))]);
        let (out, ran, _, _) = land("box", request(project(json!({})), src("git"), None), Some(clone), None).await;
        assert_eq!(json!({ "out": out, "ran": ran }), f["land"]["cloneFailed"]);
        let install = RecMachine::new(vec![("mkdir '/root/spoo-ts'", result(0, "/root/spoo-ts\n", "")), ("ls -A", result(0, "package-lock.json\n", "")), ("npm ci", result(1, "", "npm ERR! nope\n"))]);
        let (out, ran, _, _) = land("box", request(project(json!({})), src("git"), None), Some(install), None).await;
        assert_eq!(json!({ "out": out, "ran": ran }), f["land"]["installFailed"]);
        let full = RecMachine::new(vec![("mkdir '/root/spoo-ts'", result(1, "", "every name from spoo-ts to spoo-ts-99 is taken in /root\n"))]);
        let (out, ran, _, _) = land("box", request(project(json!({})), src("git"), None), Some(full), None).await;
        assert_eq!(json!({ "out": out, "ran": ran }), f["land"]["noFolder"]);
    });
}

// fixture: landing_roads.json land.sweepFailed
#[test]
fn a_sweep_that_fails_is_warned_and_the_adds_own_failure_is_the_answer() {
    run_local(async {
        let f = fx();
        let caught = console::capture();
        let m = Rc::new(RecMachine::plain(vec![("mkdir '/root/spoo-ts'", result(0, "/root/spoo-ts\n", "")), ("git clone", result(1, "", ""))]).failing("rm -rf"));
        let (out, ran, _, _) = land("box", request(project(json!({})), src("git"), None), Some(m), None).await;
        assert_eq!(json!({ "out": out, "ran": ran }), f["land"]["sweepFailed"]);
        assert_eq!(caught.lines(), ["/root/spoo-ts on hetzner was not swept after the add of spoo-ts failed: the link went"]);
    });
}

// fixture: landing_roads.json land.folderNoSeed, providerLand, macLand, noComputer (box-folder.test.ts:578)
#[test]
fn a_folder_already_on_the_computer_is_kept_where_it_stands_and_the_other_roads_land_nothing() {
    run_local(async {
        let f = fx();
        let m = RecMachine::new(vec![]);
        let p = project(json!({ "source": { "kind": "folder", "path": "/srv/legacy" }, "path": "/srv/legacy" }));
        let (out, ran, _, _) = land("box", request(p, src("folder"), None), Some(m), None).await;
        assert_eq!(json!({ "out": out, "ran": ran }), f["land"]["folderNoSeed"]);
        let (out, _, _, _) = land("provider", request(project(json!({})), src("git"), None), Some(RecMachine::new(vec![])), None).await;
        assert_eq!(out["ok"], f["land"]["providerLand"]);
        let (out, _, _, _) = land("mac", request(project(json!({})), src("git"), None), Some(RecMachine::new(vec![])), None).await;
        assert_eq!(out["ok"], f["land"]["macLand"]);
        let (out, _, _, _) = land("box", request(project(json!({})), src("git"), None), None, None).await;
        assert_eq!(out, f["land"]["noComputer"]);
    });
}
