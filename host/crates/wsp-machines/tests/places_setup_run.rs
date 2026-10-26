// SPDX-License-Identifier: AGPL-3.0-only
//! Ports test/places-setup.test.ts "a computer added with its picks" (:646-1180), "a host that stops in the middle of
//! a setup" (:1265-1396), "one row of a setup that ended, run again" (:1535-1631) and the sync group's ownership cases
//! (:2124-2217, :2419, :2465) at the run's seam. Spec: spec/places_setup_run.md.
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use serde_json::{json, Value};
use wsp_engine::provision::EngineStep;
use wsp_machines::places::helpers::LandedRow;
use wsp_machines::places::setup::GivenPicks;
use wsp_machines::places::types::{Resolved, SetupEvent};
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::views::place::PlaceSetup;

use common::places::*;

fn laptop() -> RecipeFile {
    recipe(json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" }, "codex": { "signin": "machine" } }, "clis": { "gh": { "via": "brew" } } }))
}

fn claude_only() -> RecipeFile {
    recipe(json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" } }, "clis": { "gh": { "via": "brew" } } }))
}

fn spoo(scene: &Scene, over: Value) -> Rc<FakeSocket> {
    let mut base = obj(json!({ "daemonVersion": 200, "login": { "HOME": "/root", "PATH": "/usr/bin" } }));
    for (k, v) in obj(over) {
        base.insert(k, v);
    }
    scene.linked("p_spoo", "spoo", report("spoo", Value::Object(base)), answers_ok())
}

fn with_sign_ins(scene: &Scene) -> Rc<SignIns> {
    let s = SignIns::new();
    *scene.recording.sign_ins.borrow_mut() = Some(Rc::clone(&s));
    s
}

async fn start(scene: &Scene, picks: RecipeFile) {
    scene.setup.start_setup("p_spoo", "a1", Some(GivenPicks { picks, recipe: None }), None).await.expect("started");
}

fn setup_of(scene: &Scene) -> PlaceSetup {
    scene.records.get("p_spoo").setup.expect("a setup")
}

fn state(scene: &Scene) -> String {
    serde_json::to_value(&setup_of(scene).state).unwrap().as_str().unwrap().to_string()
}

fn rows(scene: &Scene) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = scene
        .records
        .get("p_spoo")
        .applied
        .map(|a| a.rows)
        .unwrap_or_default()
        .iter()
        .map(|r| {
            let v = serde_json::to_value(&r.row).unwrap();
            (r.row.id.clone(), v["step"].as_str().unwrap_or("").to_string(), v["outcome"].as_str().unwrap().to_string())
        })
        .collect();
    out.sort();
    out
}

fn row_json(scene: &Scene, id: &str) -> Value {
    let r = scene.records.get("p_spoo").applied.expect("applied").rows.into_iter().find(|r| r.row.id == id).unwrap_or_else(|| panic!("no row {id}"));
    serde_json::to_value(r).unwrap()
}

fn lines(scene: &Scene) -> Vec<Value> {
    setup_of(scene).steps.iter().map(|l| serde_json::to_value(l).unwrap()).collect()
}

fn step_names(scene: &Scene) -> Vec<String> {
    lines(scene).iter().map(|l| l["step"].as_str().unwrap().to_string()).collect()
}

fn running_widths(scene: &Scene) -> Vec<usize> {
    scene.setup_frames().iter().filter_map(|f| f.running.as_ref().map(|r| r.iter().filter(|s| serde_json::to_value(s).unwrap() != json!("signins")).count())).collect()
}

async fn ended(scene: &Scene) {
    until("the run let the computer go", || !scene.setup.setting.borrow().contains("p_spoo")).await;
    settle().await;
}

#[test]
fn runs_the_floor_and_agents_first_and_writes_every_step() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({}));
        let s = with_sign_ins(&scene);
        start(&scene, laptop()).await;
        until("the codex sign-in waits with its page", || setup_of(&scene).waiting.iter().any(|w| w.url.is_some())).await;
        let waiting: Vec<Value> = setup_of(&scene).waiting.iter().map(|w| serde_json::to_value(w).unwrap()).collect();
        assert_eq!(waiting.len(), 1);
        assert_eq!((&waiting[0]["row"], &waiting[0]["label"], &waiting[0]["url"], &waiting[0]["code"], &waiting[0]["state"]), (&json!("signins/codex"), &json!("Codex"), &json!("https://auth.example/codex/1"), &json!("CODE-1"), &json!("waiting")));
        ended(&scene).await;
        assert_eq!(*scene.provisioner.picked.borrow(), vec![laptop()]);
        assert_eq!(scene.provisioner.ran_words(), ["floor", "agents", "skills", "clis", "mcp", "configs", "plugins", "context"]);
        assert_eq!(state(&scene), "done");
        let names = step_names(&scene);
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(sorted, ["agents", "clis", "configs", "context", "floor", "folderServers", "folders", "github", "mcp", "plugins", "signins", "skills"]);
        assert!(lines(&scene).iter().all(|l| l["state"] == json!("done") && l["ms"].is_number()));
        let before = |a: &str, b: &str| names.iter().position(|x| x == a) < names.iter().position(|x| x == b);
        assert!(before("floor", "agents") && before("agents", "clis") && before("clis", "mcp") && before("mcp", "plugins") && before("github", "folders") && before("folders", "context"));
        assert_eq!(rows(&scene), vec![
            ("agents/claude".to_string(), "agents".to_string(), "installed".to_string()),
            ("signins/claude".to_string(), "signins".to_string(), "present".to_string()),
            ("tools/brew/gh".to_string(), "clis".to_string(), "installed".to_string()),
        ]);
        assert_eq!(scene.ends(), ["needs-you"]);
        s.end("codex", sign_in_event(json!({ "agent": "codex", "state": "signed-in" })));
        until("the wait went", || setup_of(&scene).waiting.is_empty()).await;
        until("the end said again", || scene.ends().len() == 2).await;
        assert_eq!(scene.ends(), ["needs-you", "ready"]);
        let codex = row_json(&scene, "signins/codex");
        assert_eq!((&codex["outcome"], &codex["step"]), (&json!("installed"), &json!("signins")));
    });
}

#[test]
fn a_sign_in_landing_after_its_step_is_said_with_its_row() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({}));
        let s = with_sign_ins(&scene);
        scene.provisioner.hold(EngineStep::Clis);
        start(&scene, laptop()).await;
        until("signins done and the code waits", || {
            let st = setup_of(&scene);
            st.steps.iter().any(|l| serde_json::to_value(l).unwrap() == json!({ "step": "signins", "state": "done", "ms": l.ms })) && st.waiting.iter().any(|w| w.code.is_some())
        })
        .await;
        s.end("codex", sign_in_event(json!({ "agent": "codex", "state": "signed-in" })));
        until("the landing was said", || scene.setup_frames().iter().any(|f| f.landed.is_some())).await;
        let landed: Vec<(String, String, Option<String>, bool)> = scene.setup_frames().iter().filter(|f| f.landed.is_some()).map(|f| (f.add_id.clone(), f.place_id.clone(), f.landed.clone(), f.end.is_some())).collect();
        assert_eq!(landed, vec![("a1".to_string(), "p_spoo".to_string(), Some("signins/codex".to_string()), false)]);
        assert_eq!(state(&scene), "running");
        scene.provisioner.let_go(EngineStep::Clis);
        ended(&scene).await;
        assert_eq!(scene.ends(), ["ready"]);
    });
}

#[test]
fn a_frame_is_said_only_once_the_record_holds_it() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({}));
        let s = with_sign_ins(&scene);
        let seen: Rc<RefCell<Vec<(wsp_wire::views::place::PlaceSetupEvent, Option<PlaceSetup>)>>> = Rc::new(RefCell::new(Vec::new()));
        let (seen2, records) = (Rc::clone(&seen), Rc::clone(&scene.records));
        let frames = Rc::clone(&scene.frames);
        let watched = Rc::new(Cell::new(0usize));
        let w2 = Rc::clone(&watched);
        scene.provisioner.hold(EngineStep::Clis);
        start(&scene, laptop()).await;
        until("the code waits", || setup_of(&scene).waiting.iter().any(|w| w.code.is_some())).await;
        s.end("codex", sign_in_event(json!({ "agent": "codex", "state": "signed-in" })));
        scene.provisioner.let_go(EngineStep::Clis);
        let check = move || {
            let all = frames.borrow();
            for f in all.iter().skip(w2.get()) {
                if let SetupEvent::Setup(e) = f {
                    seen2.borrow_mut().push((e.clone(), records.map.borrow().get("p_spoo").and_then(|r| r.setup.clone())));
                }
            }
            w2.set(all.len());
        };
        for _ in 0..40 {
            check();
            settle().await;
        }
        ended(&scene).await;
        check();
        let all = seen.borrow();
        assert!(all.iter().any(|(f, _)| f.landed.as_deref() == Some("signins/codex")));
        for (frame, held) in all.iter() {
            let held = held.as_ref().expect("a setup on the record");
            if let Some(landed) = &frame.landed {
                assert!(!held.waiting.iter().any(|w| &w.row == landed), "a landing is said once the wait is off the record");
            }
            if let Some(line) = &frame.line {
                if serde_json::to_value(&line.state).unwrap() != json!("running") {
                    let on_record = held.steps.iter().find(|l| l.step == line.step).map(|l| serde_json::to_value(&l.state).unwrap());
                    assert_eq!(on_record, Some(serde_json::to_value(&line.state).unwrap()));
                }
            }
        }
    });
}

#[test]
fn a_running_step_is_stamped_with_when_it_started() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({}));
        scene.provisioner.hold(EngineStep::Clis);
        start(&scene, claude_only()).await;
        until("clis started", || scene.provisioner.ran_words().contains(&"clis")).await;
        let began = iso(T0);
        scene.clock.pass(300_000).await;
        let line = lines(&scene).into_iter().find(|l| l["step"] == json!("clis")).expect("the clis line");
        assert_eq!(line, json!({ "step": "clis", "state": "running", "startedAt": began }));
        let frame = scene.setup_frames().into_iter().find_map(|f| f.line.filter(|l| serde_json::to_value(&l.step).unwrap() == json!("clis"))).expect("its frame");
        assert_eq!(serde_json::to_value(frame).unwrap(), json!({ "step": "clis", "state": "running", "startedAt": began }));
        scene.provisioner.let_go(EngineStep::Clis);
        ended(&scene).await;
    });
}

#[test]
fn a_failed_floor_stops_the_run_and_says_why() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({}));
        scene.provisioner.rows.borrow_mut().insert(EngineStep::Floor, vec![row(json!({ "id": "base/curl", "label": "curl", "outcome": "failed", "note": "apt exited 100" }))]);
        start(&scene, laptop()).await;
        ended(&scene).await;
        assert_eq!(scene.provisioner.ran_words(), ["floor"]);
        assert_eq!(state(&scene), "failed");
        let said = setup_of(&scene).said.expect("a reason");
        let ends: Vec<_> = scene.setup_frames().into_iter().filter(|f| f.end.is_some()).collect();
        assert_eq!(ends.len(), 1);
        assert_eq!(ends[0].said.as_deref(), Some(said.as_str()));
        assert_eq!(said, wsp_words::views::place_setup_words::floor_failed_line(&[row(json!({ "id": "base/curl", "label": "curl", "outcome": "failed" }))]));
    });
}

#[test]
fn every_agent_failed_stops_and_one_installed_goes_on() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({}));
        scene.provisioner.rows.borrow_mut().insert(EngineStep::Agents, vec![row(json!({ "id": "agents/claude", "label": "Claude Code", "outcome": "failed", "note": "exit 1" }))]);
        start(&scene, laptop()).await;
        ended(&scene).await;
        assert_eq!(scene.provisioner.ran_words(), ["floor", "agents", "skills"]);
        assert_eq!(state(&scene), "failed");
        let said = setup_of(&scene).said.unwrap();
        assert_eq!(said, wsp_words::views::place_setup_words::no_agent_line(&[row(json!({ "id": "agents/claude", "label": "Claude Code", "outcome": "failed", "note": "exit 1" }))]));

        let one = Scene::new(SceneOpts::default());
        spoo(&one, json!({}));
        one.provisioner.plan.borrow_mut().steps.insert(1, tool("agents/codex", "Codex", "script", None));
        one.provisioner.plan.borrow_mut().agents = 2;
        one.provisioner.rows.borrow_mut().insert(EngineStep::Agents, vec![
            row(json!({ "id": "agents/claude", "label": "Claude Code", "outcome": "failed", "note": "exit 1" })),
            row(json!({ "id": "agents/codex", "label": "Codex", "outcome": "installed" })),
        ]);
        start(&one, claude_only()).await;
        ended(&one).await;
        assert_eq!(state(&one), "done");
        assert!(one.provisioner.ran_words().contains(&"clis"));
    });
}

#[test]
fn a_failed_cli_row_stands_and_the_computer_reads_ready() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({}));
        scene.provisioner.rows.borrow_mut().insert(EngineStep::Clis, vec![row(json!({ "id": "tools/brew/gh", "label": "GitHub CLI", "outcome": "failed", "note": "no bottle" }))]);
        start(&scene, claude_only()).await;
        ended(&scene).await;
        assert_eq!(state(&scene), "done");
        let gh = row_json(&scene, "tools/brew/gh");
        assert_eq!((&gh["outcome"], &gh["step"]), (&json!("failed"), &json!("clis")));
        let line = lines(&scene).into_iter().find(|l| l["step"] == json!("clis")).unwrap();
        assert_eq!((&line["state"], &line["note"]), (&json!("failed"), &json!("1 of 1 failed")));
        let ends: Vec<_> = scene.setup_frames().into_iter().filter(|f| f.end.is_some()).collect();
        assert_eq!(ends.len(), 1);
        assert_eq!(serde_json::to_value(&ends[0].end).unwrap(), json!("ready"));
        assert_eq!(ends[0].said.as_deref(), Some("1 row did not install"));
        assert!(gh["fix"].is_string(), "a failed row carries the fix the setup stamps");
    });
}

#[test]
fn runs_as_wide_as_the_memory_allows() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({}));
        scene.provisioner.hold(EngineStep::Agents);
        scene.provisioner.hold(EngineStep::Skills);
        start(&scene, laptop()).await;
        until("skills started", || scene.provisioner.ran_words().contains(&"skills")).await;
        assert_eq!(scene.provisioner.ran_words(), ["floor", "agents", "skills"]);
        let last = scene.setup_frames().into_iter().rev().find_map(|f| f.running).expect("a frame naming what runs");
        assert_eq!(serde_json::to_value(last).unwrap(), json!(["agents", "skills"]));
        scene.provisioner.let_go(EngineStep::Agents);
        scene.provisioner.let_go(EngineStep::Skills);
        ended(&scene).await;
        assert_eq!(running_widths(&scene).into_iter().max(), Some(2));
    });
}

#[test]
fn runs_one_step_at_a_time_on_a_small_box() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({ "shape": { "cpu": 1, "memMb": 2048 } }));
        scene.provisioner.hold(EngineStep::Agents);
        start(&scene, laptop()).await;
        until("agents started", || scene.provisioner.ran_words().contains(&"agents")).await;
        settle().await;
        assert_eq!(scene.provisioner.ran_words(), ["floor", "agents"]);
        scene.provisioner.let_go(EngineStep::Agents);
        ended(&scene).await;
        assert_eq!(running_widths(&scene).into_iter().max(), Some(1));
    });
}

fn marked_plan(scene: &Scene) {
    let mut p = scene.provisioner.plan.borrow_mut();
    p.steps = vec![
        tool("agents/claude", "Claude Code", "script", None),
        tool("tools/manager/npm", "npm", "script", None),
        tool("tools/homebrew", "Homebrew", "script", None),
        tool("tools/brew/jq", "jq", "brew", Some("tools/homebrew")),
        tool("tools/brew/gh", "GitHub CLI", "brew", Some("tools/homebrew")),
    ];
    p.server_tools = Some(vec!["tools/brew/jq".into()]);
}

fn marked_picks() -> RecipeFile {
    recipe(json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" } }, "clis": { "jq": { "via": "brew" }, "gh": { "via": "brew" } }, "configs": { "github": { "signin": "vault" } } }))
}

fn started_steps(scene: &Scene) -> Vec<String> {
    scene.setup_frames().iter().filter_map(|f| f.line.as_ref()).filter(|l| serde_json::to_value(&l.state).unwrap() == json!("running")).map(|l| serde_json::to_value(&l.step).unwrap().as_str().unwrap().to_string()).collect()
}

fn ended_steps(scene: &Scene) -> Vec<String> {
    scene.setup_frames().iter().filter_map(|f| f.line.as_ref()).filter(|l| serde_json::to_value(&l.state).unwrap() != json!("running")).map(|l| serde_json::to_value(&l.step).unwrap().as_str().unwrap().to_string()).collect()
}

#[test]
fn never_starts_a_step_before_what_it_needs() {
    block(async {
        let mut vault = SceneOpts::default().vault;
        vault.insert("GH_TOKEN".into(), "ghp_vaulted".into());
        let scene = Scene::new(SceneOpts { vault, ..SceneOpts::default() });
        spoo(&scene, json!({}));
        marked_plan(&scene);
        scene.provisioner.hold(EngineStep::Clis);
        start(&scene, marked_picks()).await;
        until("the rest beside the clis ended", || ["skills", "configs", "signins"].iter().all(|s| ended_steps(&scene).contains(&s.to_string())) && scene.provisioner.ran_words().contains(&"clis")).await;
        assert!(!started_steps(&scene).contains(&"github".to_string()));
        assert!(!scene.provisioner.ran_words().contains(&"mcp"));
        scene.provisioner.say(EngineStep::Clis, &row(json!({ "id": "tools/brew/gh", "label": "GitHub CLI", "outcome": "installed" })));
        until("github and folders ended", || ["github", "folders"].iter().all(|s| ended_steps(&scene).contains(&s.to_string()))).await;
        assert!(!scene.provisioner.ran_words().contains(&"mcp"));
        scene.provisioner.say(EngineStep::Clis, &row(json!({ "id": "tools/brew/jq", "label": "jq", "outcome": "installed" })));
        until("mcp and plugins ended", || ["mcp", "plugins"].iter().all(|s| ended_steps(&scene).contains(&s.to_string()))).await;
        assert!(!ended_steps(&scene).contains(&"clis".to_string()));
        let github = row_json(&scene, "github");
        assert_eq!((&github["outcome"], &github["step"]), (&json!("present"), &json!("github")));
        let order: Vec<String> = scene.provisioner.plans.borrow().get(&EngineStep::Clis).unwrap().steps[1..].iter().map(|s| s.id.clone()).collect();
        assert_eq!(order, ["tools/homebrew", "tools/brew/gh", "tools/brew/jq", "tools/manager/npm"]);
        scene.provisioner.let_go(EngineStep::Clis);
        ended(&scene).await;
        assert_eq!(state(&scene), "done");
    });
}

#[test]
fn holds_github_until_gh_is_said() {
    block(async {
        let mut vault = SceneOpts::default().vault;
        vault.insert("GH_TOKEN".into(), "ghp_vaulted".into());
        let scene = Scene::new(SceneOpts { vault, ..SceneOpts::default() });
        spoo(&scene, json!({}));
        marked_plan(&scene);
        scene.provisioner.hold(EngineStep::Clis);
        start(&scene, marked_picks()).await;
        until("clis running", || scene.provisioner.ran_words().contains(&"clis")).await;
        for _ in 0..5 {
            settle().await;
        }
        assert!(!scene.provisioner.ran_words().contains(&"github"));
        assert!(!started_steps(&scene).contains(&"github".to_string()));
        scene.provisioner.let_go(EngineStep::Clis);
        ended(&scene).await;
        assert!(scene.provisioner.ran_words().contains(&"github"));
    });
}

#[test]
fn holds_the_folders_until_the_hook_row_is_said() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({}));
        {
            let mut p = scene.provisioner.plan.borrow_mut();
            p.steps = vec![tool("agents/claude", "Claude Code", "script", None), tool("tools/apt/git-lfs", "git-lfs", "apt", None), tool("tools/apt/jq", "jq", "apt", None)];
        }
        *scene.recording.add_folder.borrow_mut() = Some(Rc::new(|key, _f, _m| Ok(held(json!({ "id": format!("folders/{key}"), "label": key, "outcome": "installed", "project": { "id": "pr_1" } })))));
        scene.provisioner.hold(EngineStep::Clis);
        let picks = recipe(json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" } }, "clis": { "git-lfs": { "via": "apt" }, "jq": { "via": "apt" } }, "folders": { "app": { "from": "/work/app", "keep": [] } } }));
        start(&scene, picks).await;
        until("clis running", || scene.provisioner.ran_words().contains(&"clis")).await;
        settle().await;
        assert!(!started_steps(&scene).contains(&"folders".to_string()), "the folders wait on git-lfs's hook");
        scene.provisioner.say(EngineStep::Clis, &row(json!({ "id": "tools/apt/git-lfs", "label": "git-lfs", "outcome": "installed" })));
        until("folders ended", || ended_steps(&scene).contains(&"folders".to_string())).await;
        assert!(!ended_steps(&scene).contains(&"clis".to_string()), "and not on the rest of the CLIs");
        scene.provisioner.let_go(EngineStep::Clis);
        ended(&scene).await;
    });
}

#[test]
fn holds_the_servers_until_their_tool_is_on() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({}));
        marked_plan(&scene);
        scene.provisioner.hold(EngineStep::Clis);
        start(&scene, recipe(json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" } }, "clis": { "jq": { "via": "brew" }, "gh": { "via": "brew" } } }))).await;
        until("clis running", || scene.provisioner.ran_words().contains(&"clis")).await;
        settle().await;
        assert!(!scene.provisioner.ran_words().contains(&"mcp"));
        assert!(!scene.provisioner.ran_words().contains(&"plugins"));
        scene.provisioner.say(EngineStep::Clis, &row(json!({ "id": "tools/brew/jq", "label": "jq", "outcome": "installed" })));
        until("mcp and plugins ran", || ["mcp", "plugins"].iter().all(|s| scene.provisioner.ran_words().contains(s))).await;
        scene.provisioner.let_go(EngineStep::Clis);
        ended(&scene).await;
    });
}

#[test]
fn each_log_line_names_its_step() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        let socket = spoo(&scene, json!({}));
        start(&scene, claude_only()).await;
        ended(&scene).await;
        scene.clock.pass(2_000).await;
        let text: String = socket.reach.cmds().iter().filter(|c| c.contains("/.wsp/provision/log") && c.contains("printf")).map(|c| common::places::written(c)).collect();
        assert!(text.contains("[clis] clis under way"), "a step's own lines carry its step: {text}");
        assert!(text.contains("[agents] "), "{text}");
    });
}

#[test]
fn a_resume_redoes_no_ended_step() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({}));
        let mut r = scene.records.get("p_spoo");
        r.picks = Some(claude_only());
        r.setup = Some(serde_json::from_value(json!({
            "state": "running", "addId": "a1", "startedAt": iso(T0),
            "steps": [{ "step": "floor", "state": "done", "ms": 1 }, { "step": "agents", "state": "done", "ms": 1 }, { "step": "skills", "state": "done", "ms": 1 }, { "step": "clis", "state": "running" }],
            "waiting": [],
        })).unwrap());
        r.applied = Some(serde_json::from_value(json!({ "hash": "h", "at": iso(T0), "rows": [{ "id": "agents/claude", "label": "Claude Code", "outcome": "installed", "step": "agents" }] })).unwrap());
        scene.records.keep(r);
        scene.setup.start_setup("p_spoo", "a1", None, None).await.unwrap();
        ended(&scene).await;
        assert_eq!(scene.provisioner.ran_words(), ["clis", "mcp", "configs", "plugins", "context"]);
        assert!(rows(&scene).contains(&("agents/claude".to_string(), "agents".to_string(), "installed".to_string())));
        assert_eq!(state(&scene), "done");
    });
}

#[test]
fn a_step_that_throws_is_one_failed_row_and_keeps_what_earlier_runs_put_on() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({}));
        let mut r = scene.records.get("p_spoo");
        r.picks = Some(claude_only());
        r.applied = Some(serde_json::from_value(json!({ "hash": "h", "at": iso(T0), "rows": [{ "id": "tools/brew/gh", "label": "GitHub CLI", "outcome": "installed", "step": "clis" }] })).unwrap());
        scene.records.keep(r);
        *scene.provisioner.throws.borrow_mut() = Some((EngineStep::Clis, err("brew went away\nsecond line")));
        start(&scene, claude_only()).await;
        ended(&scene).await;
        let stopped = row_json(&scene, "clis/stopped");
        assert_eq!((&stopped["outcome"], &stopped["note"], &stopped["step"]), (&json!("failed"), &json!("brew went away"), &json!("clis")));
        let kept = row_json(&scene, "tools/brew/gh");
        assert_eq!((&kept["outcome"], &kept["earlier"]), (&json!("installed"), &json!(true)), "what an earlier run put on stays wsp's through a step that threw");
    });
}

#[test]
fn a_computer_that_went_away_leaves_the_setup_running() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({}));
        scene.provisioner.hold(EngineStep::Clis);
        start(&scene, claude_only()).await;
        until("clis running", || scene.provisioner.ran_words().contains(&"clis")).await;
        *scene.provisioner.throws.borrow_mut() = Some((EngineStep::Clis, absent("spoo is not connected")));
        scene.provisioner.let_go(EngineStep::Clis);
        ended(&scene).await;
        let s = setup_of(&scene);
        assert_eq!(serde_json::to_value(&s.state).unwrap(), json!("running"), "it resumes at the next link");
        assert_eq!(s.finished_at, None);
        assert!(scene.ends().is_empty(), "no end is said");
        let clis = lines(&scene).into_iter().find(|l| l["step"] == json!("clis")).unwrap();
        assert_eq!(clis["state"], json!("running"), "the step that was cut stays running, to be run again");
        assert!(scene.records.get("p_spoo").applied.map(|a| a.rows.iter().all(|r| r.row.id != "clis/stopped")).unwrap_or(true));
        assert!(scene.setup.landers.borrow().get("p_spoo").is_none(), "a run cut off writes nothing more");
    });
}

#[test]
fn a_tool_wsp_put_on_stays_wsps_through_a_run_again() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({}));
        start(&scene, claude_only()).await;
        ended(&scene).await;
        scene.provisioner.rows.borrow_mut().insert(EngineStep::Clis, vec![row(json!({ "id": "tools/brew/gh", "label": "GitHub CLI", "outcome": "present" }))]);
        scene.setup.start_setup("p_spoo", "a2", None, None).await.unwrap();
        ended(&scene).await;
        let gh = row_json(&scene, "tools/brew/gh");
        assert_eq!((&gh["outcome"], &gh["earlier"]), (&json!("installed"), &json!(true)));
    });
}

#[test]
fn a_sync_that_failed_before_any_step_reads_behind() {
    block(async {
        let moved = recipe(json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" } }, "clis": { "gh": { "via": "brew" } } }));
        let shelf = Rc::new(Shelf { now: RefCell::new(Resolved { file: moved, items: Default::default(), hash: "h2".into() }), resolves: Cell::new(0) });
        let scene = Scene::new(SceneOpts { shelf: Some(shelf), ..SceneOpts::default() });
        spoo(&scene, json!({}));
        let mut r = scene.records.get("p_spoo");
        r.recipe = Some("laptop".into());
        r.picks = Some(recipe(json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" } }, "clis": { "gh": { "via": "brew" }, "jq": { "via": "brew" } } })));
        r.setup = Some(serde_json::from_value(json!({ "state": "done", "addId": "a1", "startedAt": iso(T0), "steps": [], "waiting": [] })).unwrap());
        r.applied = Some(serde_json::from_value(json!({ "hash": "h1", "at": iso(T0), "rows": [{ "id": "tools/brew/jq", "label": "jq", "outcome": "installed", "step": "clis" }] })).unwrap());
        scene.records.keep(r);
        *scene.provisioner.undo_fails.borrow_mut() = Some(err("the ledger could not be read"));
        scene.setup.sync_place("p_spoo").await.unwrap();
        let now = scene.records.get("p_spoo");
        assert_eq!(serde_json::to_value(now.sync.as_ref().unwrap()).unwrap(), json!({ "state": "behind", "changes": ["clis/jq"], "since": iso(T0) }));
        assert_eq!(now.applied.as_ref().unwrap().hash, "h1", "nothing put on, the old hash holds");
        assert!(scene.provisioner.ran.borrow().is_empty());
        assert_eq!(serde_json::to_value(scene.sync_frames().last().unwrap().sync.as_ref().unwrap()).unwrap()["state"], json!("behind"));
        assert!(!scene.setup.syncing.borrow().contains("p_spoo"));
    });
}

#[test]
fn a_sync_cut_partway_holds_the_hash_it_started_from() {
    block(async {
        let moved = recipe(json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" } }, "clis": { "gh": { "via": "brew" } }, "skills": { "review": { "from": "github.com/acme/skills" } } }));
        let shelf = Rc::new(Shelf { now: RefCell::new(Resolved { file: moved, items: Default::default(), hash: "h2".into() }), resolves: Cell::new(0) });
        let scene = Scene::new(SceneOpts { shelf: Some(shelf), ..SceneOpts::default() });
        let socket = spoo(&scene, json!({}));
        let mut r = scene.records.get("p_spoo");
        r.recipe = Some("laptop".into());
        r.picks = Some(claude_only());
        r.setup = Some(serde_json::from_value(json!({ "state": "done", "addId": "a1", "startedAt": iso(T0), "steps": [], "waiting": [] })).unwrap());
        r.applied = Some(serde_json::from_value(json!({ "hash": "h1", "at": iso(T0), "rows": [] })).unwrap());
        scene.records.keep(r);
        scene.provisioner.hold(EngineStep::Skills);
        let s = Rc::clone(&scene.setup);
        let syncing = tokio::task::spawn_local(async move { s.sync_place("p_spoo").await });
        until("skills running", || scene.provisioner.ran_words().contains(&"skills")).await;
        until("a row write landed", || scene.records.writes.borrow().len() >= 2).await;
        assert_eq!(scene.records.get("p_spoo").applied.unwrap().hash, "h1", "a sync holds the recipe it started from until its last step ended");
        scene.away("p_spoo", &socket);
        *scene.provisioner.throws.borrow_mut() = Some((EngineStep::Skills, absent("spoo is not connected")));
        scene.provisioner.let_go(EngineStep::Skills);
        let _ = syncing.await;
        settle().await;
        assert_eq!(scene.records.get("p_spoo").applied.unwrap().hash, "h1");
    });
}

#[test]
fn a_sign_in_landed_while_the_run_writes_survives_its_next_write() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({}));
        let picks = recipe(json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" } }, "clis": { "gh": { "via": "brew" } } }));
        scene.vault.borrow_mut().clear();
        scene.provisioner.hold(EngineStep::Clis);
        start(&scene, picks).await;
        until("the claude row failed for nothing to copy", || scene.records.get("p_spoo").applied.is_some_and(|a| a.rows.iter().any(|r| r.row.id == "signins/claude"))).await;
        let lander = scene.setup.landers.borrow().get("p_spoo").cloned().expect("the run holds a lander");
        let landed = lander(LandedRow { row: row(json!({ "id": "signins/claude", "label": "x", "outcome": "present", "note": "the token made on zingzys-mac" })), label: None }).await;
        assert!(landed);
        let now = row_json(&scene, "signins/claude");
        assert_eq!((&now["outcome"], &now["label"], &now["step"]), (&json!("present"), &json!("Claude Code"), &json!("signins")));
        scene.provisioner.let_go(EngineStep::Clis);
        ended(&scene).await;
        let after = row_json(&scene, "signins/claude");
        assert_eq!(after["outcome"], json!("present"), "the run's next write keeps it");
        assert!(scene.setup.landers.borrow().get("p_spoo").is_none(), "the run let go once nothing waits");
    });
}

#[test]
fn applied_keeps_one_row_per_id_and_stamps_a_fix() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({}));
        let mut r = scene.records.get("p_spoo");
        r.applied = Some(serde_json::from_value(json!({ "hash": "h", "at": iso(T0), "rows": [{ "id": "tools/brew/gh", "label": "GitHub CLI", "outcome": "installed", "step": "clis" }] })).unwrap());
        scene.records.keep(r);
        scene.provisioner.rows.borrow_mut().insert(EngineStep::Clis, vec![row(json!({ "id": "tools/brew/gh", "label": "GitHub CLI", "outcome": "failed", "note": "no bottle" }))]);
        start(&scene, claude_only()).await;
        ended(&scene).await;
        let all = scene.records.get("p_spoo").applied.unwrap().rows;
        assert_eq!(all.iter().filter(|r| r.row.id == "tools/brew/gh").count(), 1);
        let gh = row_json(&scene, "tools/brew/gh");
        assert_eq!(gh["outcome"], json!("failed"));
        assert_eq!(gh["fix"], json!(wsp_words::views::place_setup_words::setup_row_fix(&row(json!({ "id": "tools/brew/gh", "label": "GitHub CLI", "outcome": "failed", "step": "clis", "note": "no bottle" })), "spoo")));
    });
}

#[test]
fn a_row_retry_runs_only_its_steps_and_keeps_the_rest() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({}));
        scene.provisioner.rows.borrow_mut().insert(EngineStep::Clis, vec![row(json!({ "id": "tools/brew/gh", "label": "GitHub CLI", "outcome": "failed", "note": "no bottle" }))]);
        start(&scene, claude_only()).await;
        ended(&scene).await;
        let before = lines(&scene);
        scene.provisioner.ran.borrow_mut().clear();
        scene.provisioner.rows.borrow_mut().insert(EngineStep::Clis, vec![row(json!({ "id": "tools/brew/gh", "label": "GitHub CLI", "outcome": "installed" }))]);
        scene.setup.start_setup("p_spoo", "a2", None, Some("tools/brew/gh")).await.unwrap();
        ended(&scene).await;
        assert_eq!(scene.provisioner.ran_words(), ["clis", "mcp", "context"]);
        let after = lines(&scene);
        for l in before.iter().filter(|l| !["clis", "mcp", "folderServers", "context"].contains(&l["step"].as_str().unwrap())) {
            assert!(after.contains(l), "the other steps' lines stand: {l}");
        }
        assert_eq!(row_json(&scene, "tools/brew/gh")["outcome"], json!("installed"));
    });
}

#[test]
fn a_retry_runs_the_steps_a_stopped_setup_never_reached() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        spoo(&scene, json!({}));
        scene.provisioner.rows.borrow_mut().insert(EngineStep::Floor, vec![row(json!({ "id": "base/curl", "label": "curl", "outcome": "failed", "note": "apt exited 100" }))]);
        start(&scene, claude_only()).await;
        ended(&scene).await;
        scene.provisioner.rows.borrow_mut().shift_remove(&EngineStep::Floor);
        scene.provisioner.ran.borrow_mut().clear();
        scene.setup.start_setup("p_spoo", "a2", None, Some("floor")).await.unwrap();
        ended(&scene).await;
        assert_eq!(scene.provisioner.ran_words(), ["floor", "agents", "skills", "clis", "mcp", "configs", "plugins", "context"]);
        assert_eq!(state(&scene), "done");
    });
}
