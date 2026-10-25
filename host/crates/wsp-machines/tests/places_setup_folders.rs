// SPDX-License-Identifier: AGPL-3.0-only
//! Ports test/places-setup.test.ts :470-497 (the servers step's inputs), :1030, :1067-1126, :1983, :2069-2259 and :2333
//! (the folders, their servers and a sync's removals) at the setup area's seam. Spec: spec/places_setup_folders.md.
mod common;

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use async_trait::async_trait;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};
use wsp_core::daemon_channel::DaemonChannel;
use wsp_core::types::pending::DaemonReach;
use wsp_engine::machine::{ExecResult, MachineBackend};
use wsp_engine::provision::{EngineStep, ProvisionOn, ProvisionStage};
use wsp_store::keys::Seal;
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::types::recipe_file::{RecipeFile, RecipeFileFoldersValue};
use wsp_wire::views::place::{PendingComputer, PlaceAddJob, PlaceProvisionRow, PlaceSetAlso, PlaceSetup, PlaceSetupStep, PlaceWait};
use wsp_wire::views::place_more::{PlaceDial, PlaceHolds, PlaceRemoved, PlaceSettingWord, PlaceSettings, PlaceSettingsAsk, PlaceUpdateReply, PlaceView};
use wsp_wire::views::place_setup_words::PlaceEstimate;
use wsp_wire::wire::machine_link::SysSample;
use wsp_wire::wire::place_link::{PlaceAuthRequest, PlaceJoinRequest, PlaceProveRequest, PlaceReport};
use wsp_words::views::place_setup_words::{edited_there_line, was_there_line, FOLDER_SERVERS_WAIT_LINE, GITHUB_SKIPPED_LINE, NEEDS_GITHUB_LINE, NO_FOLDER_ROAD, SIGNED_IN_THERE, UNLAND_FAILED_LINE, WAITS_ON_GITHUB_LINE};

use common::places::*;
use wsp_machines::places::helpers::LinkSocket;
use wsp_machines::places::link::LinkDoor;
use wsp_machines::places::setup::GivenPicks;
use wsp_machines::places::types::{
    AddAsk, AuthAnswer, FolderComputer, FolderMove, ForwardAsk, HeldRow, JoinAnswer, PlaceAdded, PlaceChallenge, PlaceDoor, PlaceEvent, PlaceExecOpts, PlaceFor, PlaceRecord, PlaceResult, PlaceSetUp, PlaceUndo, ProjectOn,
    Proved, RemoveAsk, RemovedRow, Resolved, SetUpAsk,
};

const P: &str = "p_spoo";

enum Say {
    Run { exit: i64, stdout: String, stderr: String },
    Fail(String),
}

fn run(exit: i64, stdout: &str) -> Say {
    Say::Run { exit, stdout: stdout.into(), stderr: String::new() }
}

fn run_err(exit: i64, stdout: &str, stderr: &str) -> Say {
    Say::Run { exit, stdout: stdout.into(), stderr: stderr.into() }
}

/// A computer answering every command as `answer` says, else as one with no gh login and nothing to print.
fn script(answer: impl Fn(&str) -> Option<Say> + 'static) -> Script {
    Rc::new(move |op, params| {
        if op != "exec" {
            return Reply::Ok(obj(json!({ "ok": true })));
        }
        let cmd = params.get("cmd").and_then(Value::as_str).unwrap_or("");
        let said = answer(cmd).unwrap_or_else(|| {
            if cmd.contains("gh auth status") {
                run(1, "You are not logged into any GitHub hosts. To log in, run: gh auth login\n")
            } else {
                run(0, "")
            }
        });
        match said {
            Say::Run { exit, stdout, stderr } => Reply::Ok(obj(json!({ "ok": true, "exitCode": exit, "stdout": stdout, "stderr": stderr, "truncated": false }))),
            Say::Fail(e) => Reply::Err(e),
        }
    })
}

/// A root daemon asked who its lines run as, and a home `owner` owns.
fn login_probe(cmd: &str, owner: &str, home: &str) -> Option<Say> {
    cmd.starts_with("uname -s;").then(|| run(0, &["Linux", "0", "root", owner, "1", home, "/usr/bin", ""].join("\n")))
}

fn private_read_refused(cmd: &str) -> Option<Say> {
    cmd.contains("ls-remote").then(|| run(128, ""))
}

fn spoo_report() -> PlaceReport {
    report("spoo", json!({ "daemonVersion": 200, "login": { "HOME": "/root", "PATH": "/usr/bin" } }))
}

fn items(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn setup(v: Value) -> PlaceSetup {
    serde_json::from_value(v).expect("a setup")
}

fn folder(v: Value) -> RecipeFileFoldersValue {
    serde_json::from_value(v).expect("a folder pick")
}

fn picks(folders: Value, github: Option<&str>) -> RecipeFile {
    let mut v = json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" } }, "folders": folders });
    if let Some(word) = github {
        v["configs"] = json!({ "github": { "signin": word } });
    }
    recipe(v)
}

fn given(p: RecipeFile) -> Option<GivenPicks> {
    Some(GivenPicks { picks: p, recipe: None })
}

fn folders_of(keys: &[&str]) -> Value {
    Value::Object(keys.iter().map(|k| (k.to_string(), json!({ "from": format!("/repos/{k}"), "keep": [] }))).collect())
}

/// The road an add takes: a project `proj_<key>` made from the pick.
fn lands(scene: &Scene) {
    let road = |key: &str, f: &RecipeFileFoldersValue, _mv: Option<&FolderMove>| -> PlaceResult<HeldRow> {
        Ok(held(json!({
            "id": format!("folders/{key}"), "label": f.name.clone().unwrap_or_else(|| key.to_string()), "outcome": "installed",
            "project": { "id": format!("proj_{key}") }, "pick": f, "createdAt": iso(T0),
        })))
    };
    *scene.recording.add_folder.borrow_mut() = Some(Rc::new(road));
}

fn on_github(scene: &Scene, keys: &[&str]) {
    for k in keys {
        scene.recording.remotes.borrow_mut().insert(format!("/repos/{k}"), format!("git@github.com:acme/{k}.git"));
    }
}

fn project(id: &str, path: &str) -> ProjectOn {
    ProjectOn { id: id.into(), name: "app".into(), path: path.into(), checkout: None }
}

fn rows(scene: &Scene) -> Vec<HeldRow> {
    scene.records.get(P).applied.map(|a| a.rows).unwrap_or_default()
}

fn row_of(scene: &Scene, id: &str) -> Option<HeldRow> {
    rows(scene).into_iter().find(|r| r.row.id == id)
}

fn pinned(r: &HeldRow) -> Value {
    let v = serde_json::to_value(r).expect("a row as json");
    let mut out = Map::new();
    for k in ["id", "label", "outcome", "note", "step", "project", "earlier"] {
        if let Some(x) = v.get(k) {
            out.insert(k.to_string(), x.clone());
        }
    }
    Value::Object(out)
}

fn pin(scene: &Scene, id: &str) -> Value {
    pinned(&row_of(scene, id).unwrap_or_else(|| panic!("no row {id}")))
}

fn pins(scene: &Scene) -> Vec<Value> {
    rows(scene).iter().map(pinned).collect()
}

fn setup_state(scene: &Scene) -> String {
    serde_json::to_value(scene.records.get(P).setup.expect("a setup").state).expect("a state").as_str().expect("a word").to_string()
}

fn waiting(scene: &Scene) -> Vec<PlaceWait> {
    scene.records.get(P).setup.expect("a setup").waiting
}

fn ls_remotes(socket: &FakeSocket) -> Vec<String> {
    socket.reach.cmds().into_iter().filter(|c| c.contains("ls-remote")).collect()
}

fn added_keys(scene: &Scene) -> Vec<(String, Option<String>)> {
    scene.recording.added.borrow().clone()
}

fn gh_event(state: &str, said: Option<&str>) -> wsp_wire::types::agents_report::AgentsSignInEvent {
    let mut v = json!({ "agent": "gh", "state": state });
    if let Some(s) = said {
        v["said"] = json!(s);
    }
    sign_in_event(v)
}

/// A scene whose GitHub sign-in is the person's to finish and whose private folders `keys` are on GitHub.
fn github_scene(keys: &[&str]) -> (Scene, Rc<SignIns>, Rc<FakeSocket>) {
    let scene = Scene::new(SceneOpts::default());
    let signs = SignIns::new();
    *scene.recording.sign_ins.borrow_mut() = Some(Rc::clone(&signs));
    lands(&scene);
    on_github(&scene, keys);
    let socket = scene.linked(P, "spoo", spoo_report(), script(private_read_refused));
    (scene, signs, socket)
}

async fn settled_setup(scene: &Scene) {
    until("the setup ended", || setup_state(scene) == "done").await;
    settle().await;
}

// ---------------------------------------------------------------------------------------------------------------
// A door whose only answer is the folder computer, which the stores read goes through.

struct DoorOver {
    link: Rc<LinkDoor>,
}

fn with_door(scene: &Scene) -> Rc<DoorOver> {
    let door = Rc::new(DoorOver { link: scene.link() });
    scene.ctx.set_door(Rc::downgrade(&door) as Weak<dyn PlaceDoor>);
    door
}

#[allow(unused_variables)]
#[async_trait(?Send)]
impl PlaceDoor for DoorOver {
    fn answer_challenge(&self, subject: &str, nonce: &str, ephemeral: Option<&str>) -> Option<PlaceChallenge> {
        unreachable!("not read by the folders")
    }
    async fn join(&self, req: PlaceJoinRequest, from: &str, now: u64) -> PlaceResult<Option<JoinAnswer>> {
        unreachable!("not read by the folders")
    }
    async fn auth(&self, req: PlaceAuthRequest, now: u64) -> PlaceResult<AuthAnswer> {
        unreachable!("not read by the folders")
    }
    fn host_key(&self) -> String {
        unreachable!("not read by the folders")
    }
    async fn prove(&self, place_id: &str, req: PlaceProveRequest, expect: &[u8], from: &str, now: u64) -> PlaceResult<Proved> {
        unreachable!("not read by the folders")
    }
    async fn attach(&self, place_id: &str, socket: Rc<dyn LinkSocket>, report: PlaceReport, from: &str, now: u64, seal: Option<Seal>) -> PlaceResult<()> {
        unreachable!("not read by the folders")
    }
    fn link(&self, place_id: &str) -> Option<Rc<dyn DaemonReach>> {
        unreachable!("not read by the folders")
    }
    fn channel(&self, place_id: &str, on_event: Rc<dyn Fn(serde_json::Map<String, serde_json::Value>)>) -> Option<Rc<dyn DaemonChannel>> {
        unreachable!("not read by the folders")
    }
    async fn watch_sys(&self, place_id: &str, f: Rc<dyn Fn(SysSample)>) -> PlaceResult<Box<dyn FnOnce()>> {
        unreachable!("not read by the folders")
    }
    async fn load(&self) -> PlaceResult<()> {
        unreachable!("not read by the folders")
    }
    fn backend_of(&self, place_id: &str) -> Option<std::sync::Arc<dyn MachineBackend>> {
        unreachable!("not read by the folders")
    }
    fn folder_computer(&self, place_id: &str) -> Option<FolderComputer> {
        self.link.folder_computer(place_id)
    }
    fn joined(&self, place_id: &str) -> bool {
        unreachable!("not read by the folders")
    }
    async fn forking_backend(&self, place_id: &str) -> PlaceResult<std::sync::Arc<dyn MachineBackend>> {
        unreachable!("not read by the folders")
    }
    fn name_of(&self, place_id: &str) -> String {
        unreachable!("not read by the folders")
    }
    fn settings_at(&self, place_id: &str) -> PlaceSettings {
        unreachable!("not read by the folders")
    }
    fn turn_limit_at(&self, place_id: &str) -> Option<Option<u64>> {
        unreachable!("not read by the folders")
    }
    fn threads_at(&self, place_id: &str) -> Option<u64> {
        unreachable!("not read by the folders")
    }
    fn machines_at(&self, place_id: &str) -> Option<u64> {
        unreachable!("not read by the folders")
    }
    fn sign_ins_at(&self, place_id: &str, agents: Option<&[String]>) -> Option<IndexMap<String, AgentSignInState>> {
        unreachable!("not read by the folders")
    }
    fn github_from_vault(&self, place_id: &str) -> bool {
        unreachable!("not read by the folders")
    }
    fn vault_at(&self, place_id: &str, vault: &IndexMap<String, String>) -> IndexMap<String, String> {
        unreachable!("not read by the folders")
    }
    async fn login_landed(&self, place_id: &str, agent: &str) -> PlaceResult<()> {
        unreachable!("not read by the folders")
    }
    async fn logins_again(&self, place_id: &str) -> PlaceResult<()> {
        unreachable!("not read by the folders")
    }
    async fn key_landed(&self, agent: &str) -> PlaceResult<()> {
        unreachable!("not read by the folders")
    }
    async fn gh_there(&self, place_id: &str, stage: ProvisionStage) -> PlaceResult<Vec<PlaceProvisionRow>> {
        unreachable!("not read by the folders")
    }
    fn offer_of(&self, place_id: &str) -> Option<String> {
        unreachable!("not read by the folders")
    }
    async fn forward(&self, place_id: &str, place_port: u16, o: ForwardAsk) -> PlaceResult<u16> {
        unreachable!("not read by the folders")
    }
    fn pane_forwards(&self) -> Rc<wsp_machines::places::pane_ports::PanePorts> {
        unreachable!("not read by the folders")
    }
    async fn place_for(&self, word: &str) -> PlaceResult<PlaceFor> {
        unreachable!("not read by the folders")
    }
    async fn default_place(&self) -> PlaceResult<PlaceFor> {
        unreachable!("not read by the folders")
    }
    async fn mark_used(&self, place_id: Option<&str>) -> PlaceResult<()> {
        unreachable!("not read by the folders")
    }
    async fn mark_default_if_none(&self, place_id: &str) -> PlaceResult<()> {
        unreachable!("not read by the folders")
    }
    async fn add(&self, req: AddAsk, now: u64) -> PlaceResult<PlaceAdded> {
        unreachable!("not read by the folders")
    }
    async fn set_up(&self, reference: &str, o: SetUpAsk) -> PlaceResult<PlaceSetUp> {
        unreachable!("not read by the folders")
    }
    async fn pending(&self) -> PlaceResult<Vec<PendingComputer>> {
        unreachable!("not read by the folders")
    }
    async fn choose(&self, reference: &str, choices: RecipeFile, recipe: Option<String>) -> PlaceResult<PendingComputer> {
        unreachable!("not read by the folders")
    }
    async fn dial(&self, place_id: &str, now: u64) -> PlaceResult<PlaceDial> {
        unreachable!("not read by the folders")
    }
    async fn set(&self, place_id: &str, set: PlaceSettingsAsk, reset: Vec<PlaceSettingWord>, also: PlaceSetAlso) -> PlaceResult<PlaceView> {
        unreachable!("not read by the folders")
    }
    async fn road(&self, place_id: &str) -> PlaceResult<u16> {
        unreachable!("not read by the folders")
    }
    async fn exec(&self, place_id: &str, cmd: &str, opts: PlaceExecOpts) -> PlaceResult<ExecResult> {
        unreachable!("not read by the folders")
    }
    async fn report_of(&self, place_id: &str) -> PlaceResult<Option<PlaceReport>> {
        unreachable!("not read by the folders")
    }
    async fn home_of(&self, place_id: &str) -> PlaceResult<Option<String>> {
        unreachable!("not read by the folders")
    }
    async fn list(&self, now: u64) -> PlaceResult<Vec<PlaceView>> {
        unreachable!("not read by the folders")
    }
    async fn rows(&self) -> PlaceResult<Vec<PlaceView>> {
        unreachable!("not read by the folders")
    }
    fn adds(&self) -> Vec<PlaceAddJob> {
        unreachable!("not read by the folders")
    }
    async fn update(&self, place_id: &str, ask: RemoveAsk) -> PlaceResult<PlaceUpdateReply> {
        unreachable!("not read by the folders")
    }
    async fn holds(&self, place_id: &str) -> PlaceResult<PlaceHolds> {
        unreachable!("not read by the folders")
    }
    async fn remove(&self, place_id: &str, ask: RemoveAsk) -> PlaceResult<PlaceRemoved> {
        unreachable!("not read by the folders")
    }
    async fn find(&self, reference: &str) -> PlaceResult<Vec<PlaceRecord>> {
        unreachable!("not read by the folders")
    }
    async fn follow(&self, place_id: &str, recipe: &str) -> PlaceResult<PlaceView> {
        unreachable!("not read by the folders")
    }
    async fn recipe_changed(&self, slug: &str, after_ms: u64) -> PlaceResult<Vec<String>> {
        unreachable!("not read by the folders")
    }
    async fn followers(&self) -> PlaceResult<IndexMap<String, Vec<String>>> {
        unreachable!("not read by the folders")
    }
    async fn skip(&self, place_id: &str, row: &str) -> PlaceResult<PlaceView> {
        unreachable!("not read by the folders")
    }
    async fn setup_log(&self, place_id: &str, step: Option<PlaceSetupStep>) -> PlaceResult<Vec<String>> {
        unreachable!("not read by the folders")
    }
    async fn estimate(&self, reference: &str, choices: RecipeFile) -> PlaceResult<PlaceEstimate> {
        unreachable!("not read by the folders")
    }
    async fn unfollow(&self, slug: &str) -> PlaceResult<Vec<String>> {
        unreachable!("not read by the folders")
    }
    async fn picks_of(&self, place_id: &str) -> PlaceResult<Option<RecipeFile>> {
        unreachable!("not read by the folders")
    }
    fn on(&self, f: Rc<dyn Fn(&PlaceEvent)>) -> u64 {
        unreachable!("not read by the folders")
    }
    fn off(&self, id: u64) {
        unreachable!("not read by the folders")
    }
    async fn close(&self) -> PlaceResult<()> {
        unreachable!("not read by the folders")
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The servers step's inputs.

/// What the servers step (mcp) was handed on a setup of a computer, by the vault, the stores and who its lines run as.
async fn mcp_on(vault: &[(&str, &str)], rep: PlaceReport, probe: Option<(&'static str, &'static str)>, stores: Option<&[(&str, &str)]>) -> ProvisionOn {
    let scene = Scene::new(SceneOpts { vault: items(vault), ..SceneOpts::default() });
    if let Some(s) = stores {
        *scene.recording.stores.borrow_mut() = Some(items(s));
    }
    let _door = stores.map(|_| with_door(&scene));
    scene.linked(P, "spoo", rep, script(move |cmd| probe.and_then(|(owner, home)| login_probe(cmd, owner, home))));
    scene.setup.start_setup(P, "a1", given(picks(json!({}), None)), None).await.expect("started");
    until("the servers step ran", || scene.provisioner.ons.borrow().contains_key(&EngineStep::Mcp)).await;
    settled_setup(&scene).await;
    let on = scene.provisioner.ons.borrow().get(&EngineStep::Mcp).cloned().expect("the servers step's input");
    on
}

#[test]
fn the_servers_step_is_handed_held_names_and_store_folders() {
    block(async {
        let held_names = mcp_on(&[("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat01-x"), ("LINEAR_TOKEN", "lin_TESTONLY")], spoo_report(), None, None).await;
        assert_eq!(held_names.held, Some(["LINEAR_TOKEN".to_string()].into_iter().collect::<IndexSet<_>>()));
        assert_eq!(held_names.stores, None);

        let stores = [("claude", "/root/.claude-cfg"), ("codex", "/var/lib/wsp/logins/codex")];
        let root = report("spoo", json!({ "daemonVersion": 200, "login": { "HOME": "/root", "USER": "root", "PATH": "/usr/bin" } }));
        let as_root = mcp_on(&[("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat01-x")], root, Some(("root", "/root")), Some(&stores[..])).await;
        assert_eq!(as_root.stores, Some(items(&stores)));

        let maya_stores = [("claude", "/home/maya/.claude-cfg"), ("codex", "/var/lib/wsp/logins/codex")];
        let maya = mcp_on(&[("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat01-x")], report("spoo", json!({ "daemonVersion": 200 })), Some(("maya", "/home/maya")), Some(&maya_stores[..])).await;
        assert_eq!(maya.stores, Some(items(&[("claude", "/home/maya/.claude-cfg")])));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The folders and GitHub.

#[test]
fn a_private_folder_waits_for_github_and_clones_once_through() {
    block(async {
        let (scene, signs, socket) = github_scene(&["app"]);
        scene.setup.start_setup(P, "a1", given(picks(folders_of(&["app"]), Some("machine"))), None).await.expect("started");
        until("the folder waits and the sign-in shows its page", || {
            row_of(&scene, "folders/app").is_some() && setup_state(&scene) == "done" && waiting(&scene).iter().any(|w| w.code.is_some())
        })
        .await;
        settle().await;

        assert_eq!(signs.started.borrow().clone(), vec!["gh".to_string()]);
        assert_eq!(scene.provisioner.ran.borrow().iter().filter(|s| **s == EngineStep::Github).count(), 1);
        let w = waiting(&scene);
        assert_eq!(w.len(), 1);
        assert_eq!((w[0].row.as_str(), w[0].label.as_str(), w[0].url.as_deref(), w[0].code.as_deref()), ("github", "GitHub", Some("https://auth.example/gh/1"), Some("CODE-1")));
        assert_eq!(pin(&scene, "folders/app"), json!({ "id": "folders/app", "label": "app", "outcome": "skipped", "note": WAITS_ON_GITHUB_LINE, "step": "folders" }));
        let read = ls_remotes(&socket);
        assert_eq!(read.len(), 1);
        assert!(read[0].contains("'https://github.com/acme/app.git'") && read[0].contains("credential.helper="), "{}", read[0]);
        assert!(added_keys(&scene).is_empty(), "nothing cloned while GitHub is open");
        let heard = scene.ends().len();

        signs.end("gh", gh_event("signed-in", None));
        until("the folder landed", || row_of(&scene, "folders/app").is_some_and(|r| r.row.note.as_deref() != Some(WAITS_ON_GITHUB_LINE))).await;
        settle().await;

        assert_eq!(scene.ends()[heard..].to_vec(), vec!["ready".to_string()]);
        assert_eq!(pin(&scene, "github"), json!({ "id": "github", "label": "GitHub", "outcome": "installed", "note": SIGNED_IN_THERE, "step": "github" }));
        assert_eq!(pin(&scene, "folders/app"), json!({ "id": "folders/app", "label": "app", "outcome": "installed", "step": "folders", "project": { "id": "proj_app" } }));
        assert_eq!(added_keys(&scene), vec![("app".to_string(), None)]);
        assert_eq!(ls_remotes(&socket).len(), 1, "the anonymous read is asked once");
        assert!(!socket.reach.cmds().iter().any(|c| c.contains("clone")));
        assert!(waiting(&scene).is_empty());
    });
}

#[test]
fn a_private_folder_reads_needs_github_where_github_is_skipped() {
    block(async {
        let (scene, _signs, socket) = github_scene(&["app"]);
        scene.setup.start_setup(P, "a1", given(picks(folders_of(&["app"]), Some("skip"))), None).await.expect("started");
        settled_setup(&scene).await;

        assert_eq!(pin(&scene, "github"), json!({ "id": "github", "label": "GitHub", "outcome": "skipped", "note": GITHUB_SKIPPED_LINE, "step": "github" }));
        assert_eq!(pin(&scene, "folders/app"), json!({ "id": "folders/app", "label": "app", "outcome": "failed", "note": NEEDS_GITHUB_LINE, "step": "folders" }));
        let read = ls_remotes(&socket);
        assert_eq!(read.len(), 1);
        assert!(read[0].contains("'https://github.com/acme/app.git'") && read[0].contains("credential.helper="), "{}", read[0]);
        assert!(added_keys(&scene).is_empty());
        assert!(!socket.reach.cmds().iter().any(|c| c.contains("clone")));
        assert_eq!(scene.ends(), vec!["needs-you".to_string()]);
    });
}

#[test]
fn a_private_folder_reads_needs_github_where_github_is_failed() {
    block(async {
        let (scene, signs, socket) = github_scene(&["app"]);
        scene.setup.start_setup(P, "a1", given(picks(folders_of(&["app"]), Some("machine"))), None).await.expect("started");
        until("the sign-in shows its page", || waiting(&scene).iter().any(|w| w.code.is_some())).await;

        signs.end("gh", gh_event("failed", Some("gh refused")));
        until("the folder failed", || row_of(&scene, "folders/app").is_some_and(|r| r.row.outcome == wsp_wire::views::place::PlaceProvisionRowOutcome::Failed)).await;
        settle().await;

        assert_eq!(pin(&scene, "folders/app"), json!({ "id": "folders/app", "label": "app", "outcome": "failed", "note": NEEDS_GITHUB_LINE, "step": "folders" }));
        assert_eq!(pin(&scene, "github"), json!({ "id": "github", "label": "GitHub", "outcome": "failed", "note": "gh refused", "step": "github" }));
        assert!(added_keys(&scene).is_empty());
        assert_eq!(ls_remotes(&socket).len(), 1);
        assert_eq!(scene.ends(), vec!["needs-you".to_string()]);
    });
}

#[test]
fn a_folder_github_lets_anyone_read_clones_at_once() {
    block(async {
        let signs = SignIns::new();
        let scene = Scene::new(SceneOpts::default());
        *scene.recording.sign_ins.borrow_mut() = Some(Rc::clone(&signs));
        lands(&scene);
        on_github(&scene, &["app"]);
        scene.recording.remotes.borrow_mut().insert("/repos/lib".into(), "git@gitlab.com:acme/lib.git".into());
        let socket = scene.linked(P, "spoo", spoo_report(), script(|_| None));
        scene.setup.start_setup(P, "a1", given(picks(folders_of(&["app", "lib"]), Some("machine"))), None).await.expect("started");
        until("both folders landed", || row_of(&scene, "folders/app").is_some() && row_of(&scene, "folders/lib").is_some() && setup_state(&scene) == "done").await;
        settle().await;

        assert_eq!(pin(&scene, "folders/app"), json!({ "id": "folders/app", "label": "app", "outcome": "installed", "step": "folders", "project": { "id": "proj_app" } }));
        assert_eq!(pin(&scene, "folders/lib"), json!({ "id": "folders/lib", "label": "lib", "outcome": "installed", "step": "folders", "project": { "id": "proj_lib" } }));
        assert_eq!(added_keys(&scene), vec![("app".to_string(), None), ("lib".to_string(), None)]);
        let read = ls_remotes(&socket);
        assert_eq!(read.len(), 1, "only the GitHub folder is read, and a folder with another host is not");
        assert!(read[0].contains("'https://github.com/acme/app.git'"), "{}", read[0]);
        assert_eq!(waiting(&scene).len(), 1, "GitHub's own sign-in still waits on the person");
    });
}

#[test]
fn a_failed_anonymous_read_counts_as_needing_github() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        lands(&scene);
        on_github(&scene, &["app"]);
        scene.linked(P, "spoo", spoo_report(), script(|cmd| cmd.contains("ls-remote").then(|| Say::Fail("connection lost".into()))));
        scene.setup.start_setup(P, "a1", given(picks(folders_of(&["app"]), Some("skip"))), None).await.expect("started");
        settled_setup(&scene).await;

        assert_eq!(pin(&scene, "folders/app"), json!({ "id": "folders/app", "label": "app", "outcome": "failed", "note": NEEDS_GITHUB_LINE, "step": "folders" }));
        assert!(added_keys(&scene).is_empty());
    });
}

#[test]
fn the_last_waiting_folder_says_the_end_once() {
    block(async {
        let (scene, signs, _socket) = github_scene(&["a", "b"]);
        scene.setup.start_setup(P, "a1", given(picks(folders_of(&["a", "b"]), Some("machine"))), None).await.expect("started");
        until("both folders wait on GitHub", || {
            row_of(&scene, "folders/a").is_some() && row_of(&scene, "folders/b").is_some() && setup_state(&scene) == "done" && waiting(&scene).iter().any(|w| w.code.is_some())
        })
        .await;
        settle().await;
        assert_eq!(scene.ends(), Vec::<String>::new(), "a run with folders waiting says no end yet");
        let heard = scene.setup_frames().len();

        signs.end("gh", gh_event("signed-in", None));
        until("both folders landed", || {
            ["folders/a", "folders/b"].iter().all(|id| row_of(&scene, id).is_some_and(|r| r.row.outcome == wsp_wire::views::place::PlaceProvisionRowOutcome::Installed))
        })
        .await;
        settle().await;

        let after = scene.setup_frames()[heard..].to_vec();
        let ending: Vec<_> = after.iter().filter(|f| f.end.is_some()).collect();
        assert_eq!(ending.len(), 1);
        assert_eq!(scene.ends(), vec!["ready".to_string()]);
        assert!(matches!(ending[0].landed.as_deref(), Some("folders/a") | Some("folders/b")), "the end rides a folder's landing");
        assert_eq!(ending[0].said, None);
        let mut keys: Vec<String> = added_keys(&scene).into_iter().map(|(k, _)| k).collect();
        keys.sort();
        assert_eq!(keys, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(pin(&scene, "folders/a"), json!({ "id": "folders/a", "label": "a", "outcome": "installed", "step": "folders", "project": { "id": "proj_a" } }));
        assert_eq!(pin(&scene, "folders/b"), json!({ "id": "folders/b", "label": "b", "outcome": "installed", "step": "folders", "project": { "id": "proj_b" } }));
    });
}

#[test]
fn a_late_folders_servers_go_after_every_step() {
    block(async {
        let (scene, signs, _socket) = github_scene(&["app"]);
        scene.provisioner.project_servers.set(true);
        scene.recording.projects.borrow_mut().push(project("proj_app", "/root/app"));
        scene.setup.start_setup(P, "a1", given(picks(folders_of(&["app"]), Some("machine"))), None).await.expect("started");
        until("the folder waits on GitHub", || row_of(&scene, "folders/app").is_some() && setup_state(&scene) == "done" && waiting(&scene).iter().any(|w| w.code.is_some())).await;
        settle().await;
        assert!(scene.provisioner.carried.borrow().is_empty(), "nothing is carried before the folder lands");

        signs.end("gh", gh_event("signed-in", None));
        until("the servers were carried", || scene.provisioner.carried.borrow().len() == 1).await;
        settle().await;

        let carried = scene.provisioner.carried.borrow().clone();
        assert_eq!(carried.len(), 1);
        assert_eq!((carried[0].0.as_str(), carried[0].1.as_str()), ("app", "/root/app"));
        let mut ended: Vec<&str> = carried[0].2.iter().map(|s| s.word()).collect();
        ended.sort();
        assert_eq!(ended, vec!["agents", "clis", "configs", "context", "github", "mcp", "plugins", "skills"]);
        assert_eq!(
            pin(&scene, "agents/mcp/claude/@/root/app/kept"),
            json!({ "id": "agents/mcp/claude/@/root/app/kept", "label": "Claude Code kept", "outcome": "installed", "step": "folderServers" })
        );
        assert_eq!(pin(&scene, "folders/app"), json!({ "id": "folders/app", "label": "app", "outcome": "installed", "step": "folders", "project": { "id": "proj_app" } }));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// A project an earlier run made.

fn made_before(scene: &Scene, pick: Value) {
    let mut r = scene.records.get(P);
    r.applied = Some(
        serde_json::from_value(json!({
            "hash": "h0", "at": iso(T0),
            "rows": [{ "id": "folders/app", "label": "app", "outcome": "installed", "step": "folders", "project": { "id": "proj_old" }, "pick": pick, "createdAt": iso(T0) }],
        }))
        .expect("applied rows"),
    );
    scene.records.keep(r);
    scene.recording.projects.borrow_mut().push(project("proj_old", "/root/app"));
}

fn app_pick(extra: Value) -> Value {
    let mut v = json!({ "from": "/repos/app", "keep": [] });
    for (k, x) in extra.as_object().expect("an object") {
        v[k] = x.clone();
    }
    v
}

#[test]
fn a_folder_made_before_from_the_same_pick_stands_as_wsps() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        lands(&scene);
        let socket = scene.linked(P, "spoo", spoo_report(), script(private_read_refused));
        on_github(&scene, &["app"]);
        made_before(&scene, app_pick(json!({})));
        scene.setup.start_setup(P, "a2", given(picks(json!({ "app": app_pick(json!({})) }), None)), None).await.expect("started");
        settled_setup(&scene).await;

        assert_eq!(pin(&scene, "folders/app"), json!({ "id": "folders/app", "label": "app", "outcome": "installed", "step": "folders", "project": { "id": "proj_old" }, "earlier": true }));
        assert_eq!(rows(&scene).iter().filter(|r| r.row.id == "folders/app").count(), 1);
        assert!(added_keys(&scene).is_empty(), "a second add of its source would be refused as already one");
        assert!(scene.recording.looks.borrow().is_empty());
        assert!(ls_remotes(&socket).is_empty(), "nothing is asked of the remote");
    });
}

#[test]
fn a_folder_whose_look_moved_gets_the_look() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        lands(&scene);
        scene.linked(P, "spoo", spoo_report(), script(|_| None));
        made_before(&scene, app_pick(json!({})));
        scene.setup.start_setup(P, "a2", given(picks(json!({ "app": app_pick(json!({ "hue": "red" })) }), None)), None).await.expect("started");
        settled_setup(&scene).await;

        assert_eq!(scene.recording.looks.borrow().clone(), vec!["proj_old".to_string()]);
        assert_eq!(pin(&scene, "folders/app"), json!({ "id": "folders/app", "label": "app", "outcome": "installed", "step": "folders", "project": { "id": "proj_old" } }));
        assert_eq!(row_of(&scene, "folders/app").and_then(|r| r.pick), Some(folder(app_pick(json!({ "hue": "red" })))));
        assert!(added_keys(&scene).is_empty());
    });
}

#[test]
fn a_folder_that_did_not_land_keeps_its_claim() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        let road = |_key: &str, _f: &RecipeFileFoldersValue, _mv: Option<&FolderMove>| -> PlaceResult<HeldRow> { Err(err("clone refused\nsecond line")) };
        *scene.recording.add_folder.borrow_mut() = Some(Rc::new(road));
        scene.linked(P, "spoo", spoo_report(), script(|_| None));
        made_before(&scene, app_pick(json!({})));
        let moved = json!({ "app": { "from": "/repos/app2", "keep": [] } });
        scene.setup.start_setup(P, "a2", given(picks(moved, None)), None).await.expect("started");
        settled_setup(&scene).await;

        assert_eq!(added_keys(&scene), vec![("app".to_string(), Some("proj_old".to_string()))]);
        assert_eq!(pin(&scene, "folders/app"), json!({ "id": "folders/app", "label": "app", "outcome": "failed", "note": "clone refused", "step": "folders", "project": { "id": "proj_old" } }));
        let kept = row_of(&scene, "folders/app").expect("the row");
        assert_eq!(kept.pick, Some(folder(app_pick(json!({})))));
        assert_eq!(kept.created_at, Some(iso(T0)));
    });
}

#[test]
fn a_folder_with_no_road_to_add_it_says_so() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        scene.linked(P, "spoo", spoo_report(), script(|_| None));
        scene.setup.start_setup(P, "a1", given(picks(folders_of(&["app"]), None)), None).await.expect("started");
        settled_setup(&scene).await;

        assert_eq!(pin(&scene, "folders/app"), json!({ "id": "folders/app", "label": "app", "outcome": "failed", "note": NO_FOLDER_ROAD, "step": "folders" }));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// A folder's servers.

#[test]
fn servers_wait_where_the_agents_files_did_not_land() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        lands(&scene);
        scene.provisioner.project_servers.set(true);
        scene.provisioner.rows.borrow_mut().insert(EngineStep::Mcp, vec![row(json!({ "id": "files/.claude/settings.json", "label": "settings", "outcome": "failed", "kind": "file" }))]);
        scene.recording.projects.borrow_mut().push(project("proj_app", "/root/app"));
        scene.linked(P, "spoo", spoo_report(), script(|_| None));
        scene.setup.start_setup(P, "a1", given(picks(folders_of(&["app"]), None)), None).await.expect("started");
        settled_setup(&scene).await;

        assert_eq!(
            pin(&scene, "folders/app/servers"),
            json!({ "id": "folders/app/servers", "label": "app servers", "outcome": "skipped", "note": FOLDER_SERVERS_WAIT_LINE, "step": "folderServers" })
        );
        assert!(scene.provisioner.carried.borrow().is_empty(), "no server is carried into a project whose agent files are not there");
        assert_eq!(pin(&scene, "folders/app"), json!({ "id": "folders/app", "label": "app", "outcome": "installed", "step": "folders", "project": { "id": "proj_app" } }));
    });
}

#[test]
fn a_resume_whose_folders_step_ended_carries_that_folders_servers() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        lands(&scene);
        scene.provisioner.project_servers.set(true);
        scene.recording.projects.borrow_mut().push(project("proj_old", "/root/app"));
        scene.linked(P, "spoo", spoo_report(), script(|_| None));
        let ended: Vec<Value> = ["floor", "agents", "signins", "skills", "github", "clis", "mcp", "configs", "plugins", "folders"].iter().map(|s| json!({ "step": s, "state": "done" })).collect();
        let mut r = scene.records.get(P);
        r.picks = Some(picks(json!({ "app": app_pick(json!({})) }), None));
        r.setup = Some(setup(json!({ "state": "running", "addId": "a1", "startedAt": iso(T0), "steps": ended, "waiting": [] })));
        r.applied = Some(
            serde_json::from_value(json!({
                "hash": "h0", "at": iso(T0),
                "rows": [
                    { "id": "agents/claude", "label": "Claude Code", "outcome": "installed", "step": "agents" },
                    { "id": "folders/app", "label": "app", "outcome": "installed", "step": "folders", "project": { "id": "proj_old" }, "pick": app_pick(json!({})), "createdAt": iso(T0) },
                ],
            }))
            .expect("applied rows"),
        );
        scene.records.keep(r);
        scene.setup.start_setup(P, "a1", None, None).await.expect("resumed");
        settled_setup(&scene).await;

        assert_eq!(scene.provisioner.ran_words(), vec!["context"]);
        assert_eq!(scene.provisioner.carried.borrow().clone(), vec![("app".to_string(), "/root/app".to_string(), Vec::new())]);
        assert_eq!(
            pin(&scene, "agents/mcp/claude/@/root/app/kept"),
            json!({ "id": "agents/mcp/claude/@/root/app/kept", "label": "Claude Code kept", "outcome": "installed", "step": "folderServers" })
        );
        assert_eq!(pin(&scene, "folders/app"), json!({ "id": "folders/app", "label": "app", "outcome": "installed", "step": "folders", "project": { "id": "proj_old" } }));
        assert!(added_keys(&scene).is_empty());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// A sync.

fn v1_json() -> Value {
    json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" } }, "clis": { "jq": { "via": "brew" } }, "skills": { "unslop": { "from": "~/.claude/skills" } } })
}

fn v1() -> RecipeFile {
    recipe(v1_json())
}

const ITEMS: [(&str, &str); 2] = [("clis/jq", "1.7"), ("skills/unslop", "d1")];

struct Follow {
    before: RecipeFile,
    rows: Value,
    after: RecipeFile,
    after_items: Vec<(&'static str, &'static str)>,
    hash: &'static str,
    report: PlaceReport,
    vault: Option<IndexMap<String, String>>,
    script: Script,
}

/// A computer set up from the recipe `before` and in step with it, the saved recipe now `after`.
fn following(f: Follow) -> (Scene, Rc<FakeSocket>) {
    let shelf = Rc::new(Shelf { now: RefCell::new(Resolved { file: f.after, items: items(&f.after_items), hash: f.hash.into() }), resolves: Cell::new(0) });
    let mut opts = SceneOpts { shelf: Some(shelf), ..SceneOpts::default() };
    if let Some(v) = f.vault {
        opts.vault = v;
    }
    let scene = Scene::new(opts);
    let socket = scene.linked(P, "spoo", f.report, f.script);
    let mut r = scene.records.get(P);
    r.setup = Some(setup(json!({ "state": "done", "addId": "a1", "startedAt": iso(T0), "finishedAt": iso(T0), "steps": [], "waiting": [] })));
    r.applied = Some(serde_json::from_value(json!({ "hash": "h1", "at": iso(T0), "rows": f.rows, "items": items(&ITEMS) })).expect("applied rows"));
    r.picks = Some(f.before);
    r.recipe = Some("laptop".into());
    scene.records.keep(r);
    (scene, socket)
}

fn plain(rows: Value, after: RecipeFile, after_items: &[(&'static str, &'static str)], hash: &'static str, script: Script) -> Follow {
    Follow { before: v1(), rows, after, after_items: after_items.to_vec(), hash, report: spoo_report(), vault: None, script }
}

fn removing(scene: &Scene, plan: impl Fn(&[RemovedRow]) -> Vec<PlaceUndo> + 'static) {
    *scene.provisioner.undo_plan.borrow_mut() = Some(Rc::new(plan));
}

fn take_off_cmds(socket: &FakeSocket) -> Vec<String> {
    socket.reach.cmds().into_iter().filter(|c| c.starts_with("take-off-")).collect()
}

fn applied_hash(scene: &Scene) -> String {
    scene.records.get(P).applied.expect("applied").hash
}

#[test]
fn a_sync_lands_a_folder_with_no_wait_on_a_github_step_it_does_not_run() {
    block(async {
        let mut before = v1_json();
        before["configs"] = json!({ "github": { "signin": "vault" } });
        let mut after = before.clone();
        after["folders"] = json!({ "app": { "from": "/nowhere/app", "keep": [] } });
        let with_github = recipe(before);
        let mut vault = IndexMap::new();
        vault.insert("CLAUDE_CODE_OAUTH_TOKEN".to_string(), "sk-ant-oat01-x".to_string());
        vault.insert("GH_TOKEN".to_string(), "ghp_vaulted".to_string());
        let rows = json!([{ "id": "github", "label": "GitHub", "outcome": "present", "step": "github" }]);
        let (scene, socket) = following(Follow { before: with_github, vault: Some(vault), ..plain(rows, recipe(after), &ITEMS, "h9", script(|_| None)) });
        lands(&scene);
        scene.setup.sync_place(P).await.expect("synced");

        assert_eq!(applied_hash(&scene), "h9");
        assert_eq!(pin(&scene, "folders/app"), json!({ "id": "folders/app", "label": "app", "outcome": "installed", "step": "folders", "project": { "id": "proj_app" } }));
        assert_eq!(pin(&scene, "github")["outcome"], json!("present"));
        assert_eq!(added_keys(&scene), vec![("app".to_string(), None)]);
        assert!(ls_remotes(&socket).is_empty(), "the folder needs no read of its remote");
        assert!(scene.provisioner.ran.borrow().is_empty(), "no engine step runs for a folder alone");
    });
}

fn take_off_by_command(removed: &[RemovedRow]) -> Vec<PlaceUndo> {
    removed
        .iter()
        .map(|c| {
            let owner = if c.kind == "clis" { "tools/brew/jq" } else { "agents/claude" };
            PlaceUndo { key: format!("{}/{}", c.kind, c.name), label: c.name.clone(), ids: vec![owner.into()], owner: Some(owner.into()), cmd: Some(format!("take-off-{}", c.name)), ..Default::default() }
        })
        .collect()
}

#[test]
fn a_sync_never_takes_off_what_the_box_had_before_wsp() {
    block(async {
        let rows = json!([
            { "id": "agents/claude", "label": "Claude Code", "outcome": "installed", "step": "agents" },
            { "id": "tools/brew/jq", "label": "jq", "outcome": "present", "step": "clis" },
        ]);
        let mut gone = v1_json();
        gone["agents"] = json!({});
        gone["clis"] = json!({});
        let (scene, socket) = following(plain(rows, recipe(gone), &[("skills/unslop", "d1")], "h4", script(|_| None)));
        removing(&scene, take_off_by_command);
        scene.setup.sync_place(P).await.expect("synced");

        assert_eq!(scene.provisioner.undone.borrow().clone(), vec![vec!["agents/claude".to_string(), "clis/jq".to_string()]]);
        assert_eq!(take_off_cmds(&socket), vec!["take-off-claude".to_string()]);
        assert_eq!(pins(&scene), vec![json!({ "id": "clis/jq", "label": "jq", "outcome": "skipped", "note": was_there_line("spoo") })]);
        assert_eq!(applied_hash(&scene), "h4");

        let rows = json!([
            { "id": "agents/claude", "label": "Claude Code", "outcome": "present", "step": "agents" },
            { "id": "files/.claude/settings.json", "label": "settings", "outcome": "installed", "step": "mcp", "kind": "file" },
        ]);
        let mut no_agents = v1_json();
        no_agents["agents"] = json!({});
        let unland = script(|cmd| cmd.contains("wsp-unland").then(|| run(0, "wsp-unland\tgone\t.claude/settings.json\n")));
        let (scene, socket) = following(plain(rows, recipe(no_agents), &[("clis/jq", "1.7"), ("skills/unslop", "d1")], "h10", unland));
        removing(&scene, |_| {
            vec![PlaceUndo {
                key: "agents/claude".into(),
                label: "Claude Code".into(),
                ids: vec!["agents/claude".into(), "signins/claude".into(), "files/.claude/settings.json".into()],
                owner: Some("agents/claude".into()),
                cmd: Some("take-off-claude".into()),
                dests: Some(vec![".claude/settings.json".into()]),
                ..Default::default()
            }]
        });
        scene.setup.sync_place(P).await.expect("synced");

        assert!(take_off_cmds(&socket).is_empty(), "the agent the box had stays");
        assert!(socket.reach.cmds().iter().any(|c| c.contains("wsp-unland")), "its own file comes off by the ledger all the same");
        assert_eq!(pins(&scene), vec![json!({ "id": "agents/claude", "label": "Claude Code", "outcome": "skipped", "note": was_there_line("spoo") })]);
        assert_eq!(applied_hash(&scene), "h10");
    });
}

fn skill_files_plan() -> impl Fn(&[RemovedRow]) -> Vec<PlaceUndo> {
    |_| {
        vec![PlaceUndo {
            key: "skills/unslop".into(),
            label: "unslop".into(),
            ids: vec!["skills/unslop".into(), "files/.claude/skills/unslop".into()],
            dests: Some(vec![".claude/skills/unslop".into()]),
            ..Default::default()
        }]
    }
}

fn skill_rows() -> Value {
    json!([
        { "id": "agents/claude", "label": "Claude Code", "outcome": "installed", "step": "agents" },
        { "id": "skills/unslop", "label": "unslop", "outcome": "installed", "step": "skills" },
        { "id": "files/.claude/skills/unslop", "label": "unslop files", "outcome": "installed", "step": "skills", "kind": "file" },
    ])
}

fn no_skills() -> RecipeFile {
    let mut v = v1_json();
    v["skills"] = json!({});
    recipe(v)
}

#[test]
fn a_file_edited_there_stays_and_its_row_says_so() {
    block(async {
        let ledger = script(|cmd| cmd.contains("wsp-unland").then(|| run(0, "wsp-unland\tgone\t.claude/skills/unslop/SKILL.md\nwsp-unland\tkept\t.claude/skills/unslop/notes.md\n")));
        let (scene, socket) = following(plain(skill_rows(), no_skills(), &[("clis/jq", "1.7")], "h3", ledger));
        removing(&scene, skill_files_plan());
        scene.setup.sync_place(P).await.expect("synced");

        assert!(socket.reach.cmds().iter().any(|c| c.contains("wsp-unland") && c.contains("'.claude/skills/unslop'")));
        assert_eq!(
            pins(&scene),
            vec![
                json!({ "id": "agents/claude", "label": "Claude Code", "outcome": "installed", "step": "agents" }),
                json!({ "id": "skills/unslop", "label": "unslop", "outcome": "skipped", "note": edited_there_line("spoo", &[".claude/skills/unslop/notes.md"]) }),
            ]
        );
        assert_eq!(applied_hash(&scene), "h3");
    });
}

#[test]
fn a_removed_skill_comes_off_by_the_ledger() {
    block(async {
        let ledger = script(|cmd| cmd.contains("wsp-unland").then(|| run(0, "wsp-unland\tgone\t.claude/skills/unslop/SKILL.md\nwsp-unland\tgone\t.claude/skills/unslop/notes.md\n")));
        let (scene, socket) = following(plain(skill_rows(), no_skills(), &[("clis/jq", "1.7")], "h3", ledger));
        removing(&scene, skill_files_plan());
        scene.setup.sync_place(P).await.expect("synced");

        assert_eq!(scene.provisioner.undone.borrow().clone(), vec![vec!["skills/unslop".to_string()]]);
        assert!(socket.reach.cmds().iter().any(|c| c.contains("wsp-unland") && c.contains("'.claude/skills/unslop'")));
        assert!(scene.provisioner.ran.borrow().is_empty(), "no step runs for a removal alone");
        assert_eq!(pins(&scene), vec![json!({ "id": "agents/claude", "label": "Claude Code", "outcome": "installed", "step": "agents" })]);
        assert_eq!(applied_hash(&scene), "h3");
    });
}

#[test]
fn a_files_round_the_computer_would_not_take_says_so() {
    block(async {
        let refused = script(|cmd| cmd.contains("wsp-unland").then(|| run(1, "")));
        let (scene, _socket) = following(plain(skill_rows(), no_skills(), &[("clis/jq", "1.7")], "h3", refused));
        removing(&scene, skill_files_plan());
        scene.setup.sync_place(P).await.expect("synced");

        assert_eq!(
            pin(&scene, "skills/unslop"),
            json!({ "id": "skills/unslop", "label": "unslop", "outcome": "failed", "note": UNLAND_FAILED_LINE })
        );
    });
}

fn jq_rows() -> Value {
    json!([{ "id": "tools/brew/jq", "label": "jq", "outcome": "installed", "step": "clis" }])
}

fn no_clis() -> RecipeFile {
    let mut v = v1_json();
    v["clis"] = json!({});
    recipe(v)
}

#[test]
fn an_undo_that_fails_says_what_was_not_taken_off() {
    block(async {
        let cases = [
            (run_err(1, "", "brew: no such keg\nuninstall failed\n"), "not taken off: uninstall failed"),
            (run_err(2, "", ""), "not taken off: exit 2"),
            (run_err(1, "partial\nstopped here\n", ""), "not taken off: stopped here"),
        ];
        for (said, note) in cases {
            let said = Rc::new(RefCell::new(Some(said)));
            let (scene, _socket) = following(plain(
                jq_rows(),
                no_clis(),
                &[("skills/unslop", "d1")],
                "h5",
                script(move |cmd| if cmd.starts_with("take-off-") { said.borrow_mut().take() } else { None }),
            ));
            removing(&scene, take_off_by_command);
            scene.setup.sync_place(P).await.expect("synced");

            assert_eq!(pins(&scene), vec![json!({ "id": "clis/jq", "label": "jq", "outcome": "failed", "note": note })]);
            assert_eq!(applied_hash(&scene), "h5", "a row that failed stands and the computer holds the recipe");
        }
    });
}

#[test]
fn a_road_with_no_way_off_says_so_on_its_row() {
    block(async {
        let (scene, socket) = following(plain(jq_rows(), no_clis(), &[("skills/unslop", "d1")], "h5", script(|_| None)));
        removing(&scene, |_| {
            vec![PlaceUndo {
                key: "clis/jq".into(),
                label: "jq".into(),
                ids: vec!["tools/brew/jq".into()],
                owner: Some("tools/brew/jq".into()),
                note: Some("brew leaves jq to take off by hand".into()),
                ..Default::default()
            }]
        });
        scene.setup.sync_place(P).await.expect("synced");

        assert_eq!(pins(&scene), vec![json!({ "id": "clis/jq", "label": "jq", "outcome": "skipped", "note": "brew leaves jq to take off by hand" })]);
        assert!(take_off_cmds(&socket).is_empty());
    });
}

#[test]
fn a_dropped_codexs_files_come_off_under_the_logins_folder() {
    block(async {
        let mut with_codex = v1_json();
        with_codex["agents"]["codex"] = json!({ "signin": "vault" });
        let ledger = script(|cmd| login_probe(cmd, "root", "/root").or_else(|| cmd.contains("wsp-unland").then(|| run(0, "wsp-unland\tgone\t.codex/AGENTS.md\n"))));
        let root = report("spoo", json!({ "daemonVersion": 200, "login": { "HOME": "/root", "USER": "root", "PATH": "/usr/bin" } }));
        let rows = json!([{ "id": "agents/codex", "label": "Codex", "outcome": "installed", "step": "agents" }]);
        let (scene, socket) = following(Follow { before: recipe(with_codex), report: root, ..plain(rows, v1(), &ITEMS, "h10", ledger) });
        *scene.recording.stores.borrow_mut() = Some(items(&[("claude", "/root/.claude-cfg"), ("codex", "/var/lib/wsp/logins/codex")]));
        let _door = with_door(&scene);
        removing(&scene, |_| {
            vec![PlaceUndo {
                key: "agents/codex".into(),
                label: "Codex".into(),
                ids: vec!["agents/codex".into(), "files/.codex/AGENTS.md".into()],
                owner: Some("agents/codex".into()),
                dests: Some(vec![".codex/AGENTS.md".into()]),
                ..Default::default()
            }]
        });
        scene.setup.sync_place(P).await.expect("synced");

        let cmds = socket.reach.cmds();
        let unland = cmds.iter().find(|c| c.contains("wsp-unland")).expect("the files came off");
        assert!(unland.contains("('.codex'/*) root='/var/lib/wsp/logins/codex'"), "{unland}");
        assert!(!unland.contains("'.claude'/*"), "{unland}");
        assert_eq!(applied_hash(&scene), "h10");
    });
}

#[test]
fn a_removed_folders_project_goes_with_it() {
    block(async {
        let mut before = v1_json();
        before["folders"] = json!({ "app": { "from": "/repos/app", "keep": [] } });
        let project_row = json!({ "id": "folders/app", "label": "app", "outcome": "installed", "step": "folders", "project": { "id": "proj_app" }, "pick": { "from": "/repos/app", "keep": [] } });
        let plan = |_: &[RemovedRow]| {
            vec![PlaceUndo { key: "folders/app".into(), label: "app".into(), ids: vec!["folders/app".into()], owner: Some("folders/app".into()), folder: Some("app".into()), ..Default::default() }]
        };
        let (scene, _socket) = following(Follow { before: recipe(before.clone()), ..plain(json!([project_row]), v1(), &ITEMS, "h6", script(|_| None)) });
        removing(&scene, plan);
        scene.setup.sync_place(P).await.expect("synced");

        assert_eq!(scene.recording.removed_folders.borrow().clone(), vec!["proj_app".to_string()]);
        assert!(pins(&scene).is_empty(), "the folder's row goes with its project");
        assert_eq!(applied_hash(&scene), "h6");

        let no_project = json!({ "id": "folders/app", "label": "app", "outcome": "installed", "step": "folders" });
        let (scene, _socket) = following(Follow { before: recipe(before), ..plain(json!([no_project]), v1(), &ITEMS, "h6", script(|_| None)) });
        removing(&scene, plan);
        scene.setup.sync_place(P).await.expect("synced");

        assert!(scene.recording.removed_folders.borrow().is_empty(), "a row with no project has none to remove");
    });
}
