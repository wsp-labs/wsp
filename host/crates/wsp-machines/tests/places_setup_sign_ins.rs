// SPDX-License-Identifier: AGPL-3.0-only
//! Ports test/places-setup.test.ts :647-904, :1084, :1290-1396, :1439-1508, :1571, :1633-1997, :2188 and :2349 at the sign-ins step and the GitHub step of a whole run. Spec: spec/places_setup_sign_ins.md.
mod common;

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine as _;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use common::places::{block, iso, obj, recipe, row, settle, sign_in_event, until, FakeSocket, Reply, Scene, SceneOpts, Script, Shelf, SignIns, T0};
use wsp_core::daemon_channel::DaemonChannel;
use wsp_core::reach::DaemonReach;
use wsp_core::types::pending::Clock;
use wsp_engine::machine::{ExecResult, MachineBackend};
use wsp_engine::provision::{EngineStep, ProvisionStage};
use wsp_machines::places::helpers::LinkSocket;
use wsp_machines::places::link::LinkDoor;
use wsp_machines::places::setup::GivenPicks;
use wsp_machines::places::types::{
    FolderComputer, ForwardAsk, JoinAnswer, PlaceChallenge, PlaceDoor, PlaceEvent, PlaceExecOpts, PlaceFor, PlaceRecord, PlaceResult, Proved, Resolved,
};
use wsp_store::keys::Seal;
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::views::place::{PendingComputer, PlaceAddJob, PlaceProvisionRow, PlaceSetAlso, PlaceSetupState, PlaceSetupStep};
use wsp_wire::views::place_more::{PlaceDial, PlaceHolds, PlaceRemoved, PlaceSettingWord, PlaceSettings, PlaceSettingsAsk, PlaceUpdateReply, PlaceView};
use wsp_wire::views::place_setup_words::PlaceEstimate;
use wsp_wire::wire::daemon::EXEC_DEADLINE_EXIT;
use wsp_wire::wire::machine_link::SysSample;
use wsp_wire::wire::place_link::{PlaceAuthRequest, PlaceJoinRequest, PlaceProveRequest, PlaceReport};
use wsp_words::views::place_setup_words::{
    copied_from_line, key_held_line, no_copy_line, no_key_line, setup_row_fix, sign_in_there_fix, sign_in_ways_fix, token_held_line, waits_for_install_line,
    SignInWay, SignInWayFacts, AT_ITS_TERMINAL, CODE_FROM_ROW, GITHUB_SKIPPED_LINE, NO_SIGN_IN_ROAD, SIGNED_IN_THERE, SIGN_IN_WAIT_MS, SKIPPED_FOR_NOW,
};

const HERE: &str = "zingzys-mac";
const STORE: &str = "/var/lib/wsp/logins/codex";
const CODEX_AUTH: &str = "{\"auth_mode\":\"chatgpt\",\"tokens\":{\"refresh_token\":\"rt_TESTONLY\"}}\n";
const SLACK_MS: i64 = wsp_machines::places::helpers::SIGN_IN_SLACK_MS as i64;
const WAIT_MS: i64 = SIGN_IN_WAIT_MS as i64;

// ---------------------------------------------------------------------------------------------------------------
// The door as the context holds it: only the folder computer, which the copy's store read goes through.

struct StoresDoor {
    link: Weak<LinkDoor>,
}

#[async_trait(?Send)]
#[allow(unused_variables)]
impl PlaceDoor for StoresDoor {
    fn answer_challenge(&self, subject: &str, nonce: &str, ephemeral: Option<&str>) -> Option<PlaceChallenge> { unreachable!("sign_ins reads none of this") }
    async fn join(&self, req: PlaceJoinRequest, from: &str, now: u64) -> PlaceResult<Option<JoinAnswer>> { unreachable!("sign_ins reads none of this") }
    async fn auth(&self, req: PlaceAuthRequest, now: u64) -> PlaceResult<wsp_machines::places::types::AuthAnswer> { unreachable!("sign_ins reads none of this") }
    fn host_key(&self) -> String { unreachable!("sign_ins reads none of this") }
    async fn prove(&self, place_id: &str, req: PlaceProveRequest, expect: &[u8], from: &str, now: u64) -> PlaceResult<Proved> { unreachable!("sign_ins reads none of this") }
    async fn attach(&self, place_id: &str, socket: Rc<dyn LinkSocket>, report: PlaceReport, from: &str, now: u64, seal: Option<Seal>) -> PlaceResult<()> { unreachable!("sign_ins reads none of this") }
    fn link(&self, place_id: &str) -> Option<Rc<dyn DaemonReach>> { unreachable!("sign_ins reads none of this") }
    fn channel(&self, place_id: &str, on_event: Rc<dyn Fn(serde_json::Map<String, serde_json::Value>)>) -> Option<Rc<dyn DaemonChannel>> { unreachable!("sign_ins reads none of this") }
    async fn watch_sys(&self, place_id: &str, f: Rc<dyn Fn(SysSample)>) -> PlaceResult<Box<dyn FnOnce()>> { unreachable!("sign_ins reads none of this") }
    async fn load(&self) -> PlaceResult<()> { unreachable!("sign_ins reads none of this") }
    fn backend_of(&self, place_id: &str) -> Option<Arc<dyn MachineBackend>> { unreachable!("sign_ins reads none of this") }
    fn folder_computer(&self, place_id: &str) -> Option<FolderComputer> {
        self.link.upgrade().expect("the link door").folder_computer(place_id)
    }
    fn joined(&self, place_id: &str) -> bool { unreachable!("sign_ins reads none of this") }
    async fn forking_backend(&self, place_id: &str) -> PlaceResult<Arc<dyn MachineBackend>> { unreachable!("sign_ins reads none of this") }
    fn name_of(&self, place_id: &str) -> String { unreachable!("sign_ins reads none of this") }
    fn settings_at(&self, place_id: &str) -> PlaceSettings { unreachable!("sign_ins reads none of this") }
    fn turn_limit_at(&self, place_id: &str) -> Option<Option<u64>> { unreachable!("sign_ins reads none of this") }
    fn threads_at(&self, place_id: &str) -> Option<u64> { unreachable!("sign_ins reads none of this") }
    fn machines_at(&self, place_id: &str) -> Option<u64> { unreachable!("sign_ins reads none of this") }
    fn sign_ins_at(&self, place_id: &str, agents: Option<&[String]>) -> Option<IndexMap<String, wsp_wire::types::agents_report::AgentSignInState>> { unreachable!("sign_ins reads none of this") }
    fn github_from_vault(&self, place_id: &str) -> bool { unreachable!("sign_ins reads none of this") }
    fn vault_at(&self, place_id: &str, vault: &IndexMap<String, String>) -> IndexMap<String, String> { unreachable!("sign_ins reads none of this") }
    async fn login_landed(&self, place_id: &str, agent: &str) -> PlaceResult<()> { unreachable!("sign_ins reads none of this") }
    async fn logins_again(&self, place_id: &str) -> PlaceResult<()> { unreachable!("sign_ins reads none of this") }
    async fn key_landed(&self, agent: &str) -> PlaceResult<()> { unreachable!("sign_ins reads none of this") }
    async fn gh_there(&self, place_id: &str, stage: ProvisionStage) -> PlaceResult<Vec<PlaceProvisionRow>> { unreachable!("sign_ins reads none of this") }
    fn offer_of(&self, place_id: &str) -> Option<String> { unreachable!("sign_ins reads none of this") }
    async fn forward(&self, place_id: &str, place_port: u16, o: ForwardAsk) -> PlaceResult<u16> { unreachable!("sign_ins reads none of this") }
    fn pane_forwards(&self) -> Rc<wsp_machines::places::pane_ports::PanePorts> { unreachable!("sign_ins reads none of this") }
    async fn place_for(&self, word: &str) -> PlaceResult<PlaceFor> { unreachable!("sign_ins reads none of this") }
    async fn default_place(&self) -> PlaceResult<PlaceFor> { unreachable!("sign_ins reads none of this") }
    async fn mark_used(&self, place_id: Option<&str>) -> PlaceResult<()> { unreachable!("sign_ins reads none of this") }
    async fn mark_default_if_none(&self, place_id: &str) -> PlaceResult<()> { unreachable!("sign_ins reads none of this") }
    async fn add(&self, req: wsp_machines::places::types::AddAsk, now: u64) -> PlaceResult<wsp_machines::places::types::PlaceAdded> { unreachable!("sign_ins reads none of this") }
    async fn set_up(&self, reference: &str, o: wsp_machines::places::types::SetUpAsk) -> PlaceResult<wsp_machines::places::types::PlaceSetUp> { unreachable!("sign_ins reads none of this") }
    async fn pending(&self) -> PlaceResult<Vec<PendingComputer>> { unreachable!("sign_ins reads none of this") }
    async fn choose(&self, reference: &str, choices: RecipeFile, recipe: Option<String>) -> PlaceResult<PendingComputer> { unreachable!("sign_ins reads none of this") }
    async fn dial(&self, place_id: &str, now: u64) -> PlaceResult<PlaceDial> { unreachable!("sign_ins reads none of this") }
    async fn set(&self, place_id: &str, set: PlaceSettingsAsk, reset: Vec<PlaceSettingWord>, also: PlaceSetAlso) -> PlaceResult<PlaceView> { unreachable!("sign_ins reads none of this") }
    async fn road(&self, place_id: &str) -> PlaceResult<u16> { unreachable!("sign_ins reads none of this") }
    async fn exec(&self, place_id: &str, cmd: &str, opts: PlaceExecOpts) -> PlaceResult<ExecResult> { unreachable!("sign_ins reads none of this") }
    async fn report_of(&self, place_id: &str) -> PlaceResult<Option<PlaceReport>> { unreachable!("sign_ins reads none of this") }
    async fn home_of(&self, place_id: &str) -> PlaceResult<Option<String>> { unreachable!("sign_ins reads none of this") }
    async fn list(&self, now: u64) -> PlaceResult<Vec<PlaceView>> { unreachable!("sign_ins reads none of this") }
    async fn rows(&self) -> PlaceResult<Vec<PlaceView>> { unreachable!("sign_ins reads none of this") }
    fn adds(&self) -> Vec<PlaceAddJob> { unreachable!("sign_ins reads none of this") }
    async fn update(&self, place_id: &str, ask: wsp_machines::places::types::RemoveAsk) -> PlaceResult<PlaceUpdateReply> { unreachable!("sign_ins reads none of this") }
    async fn holds(&self, place_id: &str) -> PlaceResult<PlaceHolds> { unreachable!("sign_ins reads none of this") }
    async fn remove(&self, place_id: &str, ask: wsp_machines::places::types::RemoveAsk) -> PlaceResult<PlaceRemoved> { unreachable!("sign_ins reads none of this") }
    async fn find(&self, reference: &str) -> PlaceResult<Vec<PlaceRecord>> { unreachable!("sign_ins reads none of this") }
    async fn follow(&self, place_id: &str, recipe: &str) -> PlaceResult<PlaceView> { unreachable!("sign_ins reads none of this") }
    async fn recipe_changed(&self, slug: &str, after_ms: u64) -> PlaceResult<Vec<String>> { unreachable!("sign_ins reads none of this") }
    async fn followers(&self) -> PlaceResult<IndexMap<String, Vec<String>>> { unreachable!("sign_ins reads none of this") }
    async fn skip(&self, place_id: &str, row: &str) -> PlaceResult<PlaceView> { unreachable!("sign_ins reads none of this") }
    async fn setup_log(&self, place_id: &str, step: Option<PlaceSetupStep>) -> PlaceResult<Vec<String>> { unreachable!("sign_ins reads none of this") }
    async fn estimate(&self, reference: &str, choices: RecipeFile) -> PlaceResult<PlaceEstimate> { unreachable!("sign_ins reads none of this") }
    async fn unfollow(&self, slug: &str) -> PlaceResult<Vec<String>> { unreachable!("sign_ins reads none of this") }
    async fn picks_of(&self, place_id: &str) -> PlaceResult<Option<RecipeFile>> { unreachable!("sign_ins reads none of this") }
    fn on(&self, f: Rc<dyn Fn(&PlaceEvent)>) -> u64 { unreachable!("sign_ins reads none of this") }
    fn off(&self, id: u64) { unreachable!("sign_ins reads none of this") }
    async fn close(&self) -> PlaceResult<()> { unreachable!("sign_ins reads none of this") }
}


// ---------------------------------------------------------------------------------------------------------------
// The scene.

struct Rig {
    scene: Scene,
    signs: Rc<SignIns>,
    _link: Rc<LinkDoor>,
    _door: Rc<dyn PlaceDoor>,
}

impl std::ops::Deref for Rig {
    type Target = Scene;
    fn deref(&self) -> &Scene {
        &self.scene
    }
}

fn rig(o: SceneOpts) -> Rig {
    let scene = Scene::new(o);
    let link = scene.link();
    let door: Rc<dyn PlaceDoor> = Rc::new(StoresDoor { link: Rc::downgrade(&link) });
    scene.ctx.set_door(Rc::downgrade(&door));
    let signs = SignIns::new();
    *scene.recording.sign_ins.borrow_mut() = Some(Rc::clone(&signs));
    Rig { scene, signs, _link: link, _door: door }
}

fn opts_with_vault(pairs: &[(&str, &str)]) -> SceneOpts {
    SceneOpts { vault: pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), ..SceneOpts::default() }
}

fn computer(r: &Rig, agents: &[&str], logins: Option<&[&str]>, script: Script) -> Rc<FakeSocket> {
    let mut over = json!({ "daemonVersion": 200, "agents": agents, "login": { "HOME": "/root", "PATH": "/usr/bin" } });
    if let Some(l) = logins {
        over["logins"] = json!(l);
    }
    r.linked("p_spoo", "spoo", common::places::report("spoo", over), script)
}

fn stdin_of(frame: &Map<String, Value>) -> Vec<u8> {
    frame.get("stdin").and_then(Value::as_str).map(|s| base64::engine::general_purpose::STANDARD.decode(s).expect("base64 stdin")).unwrap_or_default()
}

fn default_answer(cmd: &str, input: &str) -> (i64, String) {
    if cmd.starts_with("uname -s;") {
        return (0, ["Linux", "0", "root", "root", "1", "/root", "/usr/bin", ""].join("\n"));
    }
    if cmd.contains("gh auth status") {
        return if input.contains("GH_TOKEN=") {
            (0, "github.com\n  - Logged in to github.com account dev (GH_TOKEN)\n  - Token scopes: 'gist', 'read:org', 'repo'\n".into())
        } else {
            (1, "You are not logged into any GitHub hosts. To log in, run: gh auth login\n".into())
        };
    }
    (0, String::new())
}

fn daemon(answer: impl Fn(&str, &str) -> Option<(i64, String)> + 'static) -> Script {
    Rc::new(move |op, params| {
        if op != "exec" {
            return Reply::Ok(obj(json!({ "ok": true })));
        }
        let cmd = params.get("cmd").and_then(Value::as_str).unwrap_or("");
        let input = String::from_utf8_lossy(&stdin_of(params)).into_owned();
        let (code, out) = answer(cmd, &input).unwrap_or_else(|| default_answer(cmd, &input));
        Reply::Ok(obj(json!({ "ok": true, "exitCode": code, "stdout": out, "stderr": "", "truncated": false })))
    })
}

fn codex_in(cmd: &str) -> Option<(i64, String)> {
    cmd.contains("codex login status").then(|| (0, "Logged in using ChatGPT\n".to_string()))
}

fn exec_frames(s: &FakeSocket, needle: &str) -> Vec<Map<String, Value>> {
    s.reach.frames.borrow().iter().filter(|(op, p)| op == "exec" && p.get("cmd").and_then(Value::as_str).is_some_and(|c| c.contains(needle))).map(|(_, p)| p.clone()).collect()
}

fn timeout_of(frame: &Map<String, Value>) -> Option<f64> {
    frame.get("timeoutMs").and_then(Value::as_f64)
}

fn laptop() -> RecipeFile {
    recipe(json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" }, "codex": { "signin": "machine" } }, "clis": { "gh": { "via": "brew" } } }))
}

fn picks_of(agents: Value, configs: Value) -> RecipeFile {
    recipe(json!({ "name": "laptop", "agents": agents, "configs": configs }))
}

async fn start(r: &Rig, picks: RecipeFile) {
    let started = r.setup.start_setup("p_spoo", "a1", Some(GivenPicks { picks, recipe: None }), None).await.expect("started");
    assert_eq!(started.said, None);
    assert_eq!(started.setup.expect("a setup").state, PlaceSetupState::Running);
}

async fn retry(r: &Rig, add_id: &str) {
    r.setup.start_setup("p_spoo", add_id, None, None).await.expect("started again");
}

fn rec(r: &Rig) -> PlaceRecord {
    r.records.get("p_spoo")
}

async fn finished(r: &Rig, add_id: &str) {
    until("the setup finished", || matches!(rec(r).setup, Some(ref s) if s.add_id == add_id && s.state == PlaceSetupState::Done)).await;
    until("its end was said", || !r.ends().is_empty()).await;
}

fn waiting(r: &Rig) -> Vec<Value> {
    rec(r).setup.map(|s| s.waiting.iter().map(|w| serde_json::to_value(w).unwrap()).collect()).unwrap_or_default()
}

fn has_code(r: &Rig) -> bool {
    waiting(r).iter().any(|w| w.get("code").is_some())
}

fn agent_wait(agent: &str, label: &str, n: u32, state: &str, begun: i64) -> Value {
    json!({ "row": format!("signins/{agent}"), "label": label, "url": format!("https://auth.example/{agent}/{n}"), "code": format!("CODE-{n}"), "expiresAt": iso(begun + WAIT_MS), "state": state })
}

fn codex_wait(n: u32, state: &str, begun: i64) -> Value {
    agent_wait("codex", "Codex", n, state, begun)
}

fn row_of(r: &Rig, id: &str) -> Option<Value> {
    rec(r).applied.and_then(|a| a.rows.into_iter().find(|h| h.row.id == id)).map(|h| serde_json::to_value(h).unwrap())
}

fn pick(v: Option<Value>, keys: &[&str]) -> Value {
    let v = v.expect("the row stands");
    Value::Object(keys.iter().filter_map(|k| v.get(*k).map(|x| ((*k).to_string(), x.clone()))).collect())
}

fn shape(v: Option<Value>) -> Value {
    pick(v, &["id", "label", "outcome", "note", "step", "fix"])
}

fn rows_of_step(r: &Rig, step: &str) -> Vec<(String, String, Option<String>)> {
    rec(r)
        .applied
        .map(|a| a.rows)
        .unwrap_or_default()
        .into_iter()
        .map(|h| serde_json::to_value(h).unwrap())
        .filter(|v| v["step"] == step)
        .map(|v| (v["id"].as_str().unwrap().to_string(), v["outcome"].as_str().unwrap().to_string(), v.get("note").and_then(Value::as_str).map(str::to_string)))
        .collect()
}

fn failed_shape(id: &str, label: &str, note: &str, step: &str) -> Value {
    let said: PlaceProvisionRow = row(json!({ "id": id, "label": label, "outcome": "failed", "note": note, "step": step }));
    let mut want = json!({ "id": id, "label": label, "outcome": "failed", "note": note, "step": step });
    if let Some(fix) = setup_row_fix(&said, "spoo") {
        want["fix"] = json!(fix);
    }
    want
}

fn landed(r: &Rig) -> Vec<String> {
    r.setup_frames().into_iter().filter_map(|f| f.landed).collect()
}

fn seed(r: &Rig, picks: RecipeFile, recipe: Option<&str>, setup: Value, rows: Value) {
    let mut rec = r.records.get("p_spoo");
    rec.picks = Some(picks);
    rec.recipe = recipe.map(str::to_string);
    rec.setup = Some(serde_json::from_value(setup).expect("a setup"));
    rec.applied = Some(serde_json::from_value(json!({ "hash": "h1", "at": iso(T0), "rows": rows })).expect("applied"));
    r.records.keep(rec);
}

fn done_setup() -> Value {
    json!({ "state": "done", "addId": "a1", "startedAt": iso(T0), "finishedAt": iso(T0), "steps": [], "waiting": [] })
}

fn claude_row() -> Value {
    json!({ "id": "agents/claude", "label": "Claude Code", "outcome": "installed", "step": "agents" })
}

fn claude_sign_in_row() -> Value {
    json!({ "id": "signins/claude", "label": "Claude Code", "outcome": "present", "step": "signins" })
}

fn claude_fix(ways: &[SignInWay]) -> String {
    sign_in_ways_fix(
        ways,
        &SignInWayFacts {
            name: "Claude Code".into(),
            here: HERE.into(),
            r#box: "spoo".into(),
            mint: Some("claude setup-token".into()),
            key_env: Some("ANTHROPIC_API_KEY".into()),
            field: None,
        },
    )
}

fn shelf(file: RecipeFile, hash: &str) -> Rc<Shelf> {
    Rc::new(Shelf { now: RefCell::new(Resolved { file, items: Default::default(), hash: hash.into() }), resolves: Cell::new(0) })
}

async fn codex_waiting() -> Rig {
    let r = rig(SceneOpts::default());
    computer(&r, &["claude", "codex"], Some(&[]), daemon(|_, _| None));
    start(&r, laptop()).await;
    until("the codex page and code", || has_code(&r)).await;
    finished(&r, "a1").await;
    r
}

fn end_codex(r: &Rig, e: Value) {
    r.signs.end("codex", sign_in_event(e));
}

// ---------------------------------------------------------------------------------------------------------------
// A wait on the person.

#[test]
fn a_machine_sign_in_waits_with_its_page_and_code() {
    block(async {
        let r = rig(SceneOpts::default());
        computer(&r, &["claude", "codex"], Some(&[]), daemon(|_, _| None));
        start(&r, laptop()).await;
        until("the page and code", || has_code(&r)).await;
        assert_eq!(*r.provisioner.picked.borrow(), vec![laptop()]);
        assert_eq!(waiting(&r), vec![codex_wait(1, "waiting", T0)]);
        finished(&r, "a1").await;

        let mut rows: Vec<[String; 3]> = rec(&r)
            .applied
            .unwrap()
            .rows
            .into_iter()
            .map(|h| serde_json::to_value(h).unwrap())
            .map(|v| [v["id"].as_str().unwrap().into(), v["step"].as_str().unwrap().into(), v["outcome"].as_str().unwrap().into()])
            .collect();
        rows.sort();
        let want = [["agents/claude", "agents", "installed"], ["signins/claude", "signins", "present"], ["tools/brew/gh", "clis", "installed"]];
        assert_eq!(rows, want.map(|t| t.map(String::from)));
        assert_eq!(
            pick(row_of(&r, "signins/claude"), &["id", "outcome", "note", "step"]),
            json!({ "id": "signins/claude", "outcome": "present", "note": token_held_line(HERE), "step": "signins" })
        );
        assert_eq!(r.ends(), ["needs-you"]);
        assert!(row_of(&r, "signins/codex").is_none());

        let waits: Vec<_> = r.setup_frames().into_iter().filter_map(|f| f.wait.map(|w| (f.add_id, f.place_id, w.url, w.code))).collect();
        assert_eq!(
            waits,
            vec![
                ("a1".to_string(), "p_spoo".to_string(), None, None),
                ("a1".to_string(), "p_spoo".to_string(), Some("https://auth.example/codex/1".to_string()), Some("CODE-1".to_string())),
            ]
        );

        end_codex(&r, json!({ "agent": "codex", "state": "signed-in" }));
        until("the wait went", || waiting(&r).is_empty()).await;
        until("the setup came out again", || r.ends().len() == 2).await;
        assert_eq!(r.ends(), ["needs-you", "ready"]);
        let landed_row = row_of(&r, "signins/codex");
        assert_eq!(landed_row.as_ref().and_then(|v| v["ms"].as_f64()), Some(0.0));
        assert_eq!(
            shape(landed_row),
            json!({ "id": "signins/codex", "label": "Codex", "outcome": "installed", "note": SIGNED_IN_THERE, "step": "signins" })
        );
        assert_eq!(landed(&r), ["signins/codex"]);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Whether an agent is signed in there.

#[test]
fn an_agent_signed_in_there_starts_no_sign_in() {
    block(async {
        let r = rig(SceneOpts::default());
        let sock = computer(&r, &["claude", "codex"], Some(&[]), daemon(|cmd, _| codex_in(cmd)));
        start(&r, laptop()).await;
        finished(&r, "a1").await;
        assert!(r.signs.started.borrow().is_empty());
        assert_eq!(exec_frames(&sock, "codex login status").len(), 1);
        assert!(waiting(&r).is_empty());
        assert_eq!(
            shape(row_of(&r, "signins/codex")),
            json!({ "id": "signins/codex", "label": "Codex", "outcome": "present", "note": SIGNED_IN_THERE, "step": "signins" })
        );
        assert_eq!(rec(&r).report.logins, Some(vec!["codex/auth.json".to_string()]));
        assert_eq!(r.ends(), ["ready"]);
    });
}

#[test]
fn a_signed_out_status_starts_the_sign_in() {
    block(async {
        let r = rig(SceneOpts::default());
        computer(&r, &["claude", "codex"], Some(&["codex/auth.json"]), daemon(|_, _| None));
        start(&r, laptop()).await;
        until("the page and code", || has_code(&r)).await;
        assert_eq!(*r.signs.started.borrow(), ["codex"]);
        assert_eq!(waiting(&r), vec![codex_wait(1, "waiting", T0)]);
    });
}

#[test]
fn with_no_status_the_listed_login_counts() {
    block(async {
        let r = rig(SceneOpts::default());
        r.signs.status.set(false);
        let sock = computer(&r, &["claude", "codex"], Some(&["codex/auth.json"]), daemon(|_, _| None));
        start(&r, laptop()).await;
        finished(&r, "a1").await;
        assert!(r.signs.started.borrow().is_empty());
        assert!(exec_frames(&sock, "codex login status").is_empty());
        assert_eq!(
            shape(row_of(&r, "signins/codex")),
            json!({ "id": "signins/codex", "label": "Codex", "outcome": "present", "note": SIGNED_IN_THERE, "step": "signins" })
        );
    });
}

#[test]
fn a_status_cut_at_its_deadline_is_unread() {
    block(async {
        let r = rig(SceneOpts::default());
        let sock = computer(&r, &["claude", "codex"], Some(&["codex/auth.json"]), daemon(|cmd, _| cmd.contains("codex login status").then(|| (EXEC_DEADLINE_EXIT, String::new()))));
        start(&r, laptop()).await;
        finished(&r, "a1").await;
        assert_eq!(exec_frames(&sock, "codex login status").len(), 1);
        assert!(r.signs.started.borrow().is_empty());
        assert_eq!(
            shape(row_of(&r, "signins/codex")),
            json!({ "id": "signins/codex", "label": "Codex", "outcome": "present", "note": SIGNED_IN_THERE, "step": "signins" })
        );
    });
}

#[test]
fn an_agent_with_no_shared_file_reads_its_status() {
    block(async {
        let r = rig(SceneOpts::default());
        computer(&r, &["claude", "gemini"], Some(&[]), daemon(|cmd, _| cmd.contains("oauth_creds.json").then(|| (0, "oauth_creds.json\n".to_string()))));
        start(&r, picks_of(json!({ "claude": { "signin": "vault" }, "gemini": { "signin": "machine" } }), json!({}))).await;
        finished(&r, "a1").await;
        assert!(r.signs.started.borrow().is_empty());
        assert_eq!(
            pick(row_of(&r, "signins/gemini"), &["id", "outcome", "note", "step"]),
            json!({ "id": "signins/gemini", "outcome": "present", "note": SIGNED_IN_THERE, "step": "signins" })
        );
    });
}

#[test]
fn a_run_again_starts_no_second_sign_in() {
    block(async {
        let r = rig(SceneOpts::default());
        let signed = Rc::new(Cell::new(false));
        let s2 = Rc::clone(&signed);
        computer(&r, &["claude", "codex"], Some(&[]), daemon(move |cmd, _| if s2.get() { codex_in(cmd) } else { None }));
        start(&r, laptop()).await;
        until("the page and code", || has_code(&r)).await;
        finished(&r, "a1").await;
        signed.set(true);
        end_codex(&r, json!({ "agent": "codex", "state": "signed-in" }));
        until("the wait went", || waiting(&r).is_empty()).await;
        retry(&r, "a2").await;
        finished(&r, "a2").await;
        assert_eq!(*r.signs.started.borrow(), ["codex"]);
        assert!(waiting(&r).is_empty());
        assert_eq!(
            shape(row_of(&r, "signins/codex")),
            json!({ "id": "signins/codex", "label": "Codex", "outcome": "present", "note": SIGNED_IN_THERE, "step": "signins" })
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// GitHub.

#[test]
fn gh_signed_in_there_starts_no_github_sign_in() {
    block(async {
        let r = rig(SceneOpts::default());
        let sock = computer(
            &r,
            &["claude"],
            Some(&[]),
            daemon(|cmd, _| cmd.contains("gh auth status").then(|| (0, "github.com\n  - Logged in to github.com account dev (keyring)\n".to_string()))),
        );
        start(&r, picks_of(json!({ "claude": { "signin": "vault" } }), json!({ "github": { "signin": "machine" } }))).await;
        finished(&r, "a1").await;
        assert!(r.signs.started.borrow().is_empty());
        assert!(waiting(&r).is_empty());
        assert_eq!(
            shape(row_of(&r, "github")),
            json!({ "id": "github", "label": "GitHub", "outcome": "present", "note": SIGNED_IN_THERE, "step": "github" })
        );
        let status = exec_frames(&sock, "gh auth status --hostname github.com 2>&1");
        assert_eq!(status.len(), 1);
        assert!(stdin_of(&status[0]).is_empty());
        assert_eq!(r.ends(), ["ready"]);
    });
}

#[test]
fn github_from_the_vault_names_the_token_scopes_and_never_sends_it_in_a_command() {
    block(async {
        let r = rig(opts_with_vault(&[("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat01-x"), ("GH_TOKEN", "ghp_vaulted")]));
        let sock = computer(&r, &["claude"], Some(&[]), daemon(|_, _| None));
        start(&r, picks_of(json!({ "claude": { "signin": "vault" } }), json!({ "github": { "signin": "vault" } }))).await;
        finished(&r, "a1").await;
        assert_eq!(
            shape(row_of(&r, "github")),
            json!({
                "id": "github", "label": "GitHub", "outcome": "present", "step": "github",
                "note": format!("{}; token scopes: gist, read:org, repo", copied_from_line(HERE)),
            })
        );
        assert!(!sock.reach.cmds().join("\n").contains("ghp_vaulted"));
        let status = exec_frames(&sock, "gh auth status --hostname github.com 2>&1");
        assert_eq!(status.len(), 1);
        assert_eq!(stdin_of(&status[0]), b"GH_TOKEN=ghp_vaulted\0");
        assert_eq!(timeout_of(&status[0]), Some(30_000.0));
    });
}

#[test]
fn github_signs_in_on_the_box_through_the_relay() {
    block(async {
        let r = rig(SceneOpts::default());
        computer(&r, &["claude"], Some(&[]), daemon(|_, _| None));
        start(&r, picks_of(json!({ "claude": { "signin": "vault" } }), json!({ "github": { "signin": "machine" } }))).await;
        until("the page and code", || has_code(&r)).await;
        finished(&r, "a1").await;
        assert_eq!(*r.signs.started.borrow(), ["gh"]);
        assert_eq!(r.provisioner.ran_words().iter().filter(|w| **w == "github").count(), 1);
        assert_eq!(
            waiting(&r),
            vec![json!({ "row": "github", "label": "GitHub", "url": "https://auth.example/gh/1", "code": "CODE-1", "expiresAt": iso(T0 + WAIT_MS), "state": "waiting" })]
        );
        assert!(r.setup.skippers.borrow().contains_key("p_spoo/github"));
        assert_eq!(r.ends(), ["needs-you"]);

        r.signs.end("gh", sign_in_event(json!({ "agent": "gh", "state": "signed-in" })));
        until("the setup came out again", || r.ends().len() == 2).await;
        assert_eq!(r.ends(), ["needs-you", "ready"]);
        assert!(waiting(&r).is_empty());
        assert_eq!(
            shape(row_of(&r, "github")),
            json!({ "id": "github", "label": "GitHub", "outcome": "installed", "note": SIGNED_IN_THERE, "step": "github" })
        );
        assert_eq!(landed(&r), ["github"]);
    });
}

#[test]
fn a_github_skip_keeps_what_an_earlier_run_put_on() {
    block(async {
        let r = rig(SceneOpts::default());
        r.provisioner.rows.borrow_mut().insert(EngineStep::Github, vec![row(json!({ "id": "github/gh", "label": "GitHub CLI", "outcome": "failed", "note": "E: Unable to locate package gh" }))]);
        computer(&r, &["claude"], Some(&[]), daemon(|_, _| None));
        start(&r, picks_of(json!({ "claude": { "signin": "vault" } }), json!({ "github": { "signin": "skip" } }))).await;
        finished(&r, "a1").await;
        assert!(!r.provisioner.ran_words().contains(&"github"));
        assert_eq!(rows_of_step(&r, "github"), vec![("github".to_string(), "skipped".to_string(), Some(GITHUB_SKIPPED_LINE.to_string()))]);
        assert_eq!(r.ends(), ["ready"]);

        let kept = rig(SceneOpts::default());
        computer(&kept, &["claude"], Some(&[]), daemon(|_, _| None));
        seed(
            &kept,
            picks_of(json!({ "claude": { "signin": "vault" } }), json!({ "github": { "signin": "skip" } })),
            None,
            done_setup(),
            json!([
                claude_row(),
                claude_sign_in_row(),
                { "id": "github/gh", "label": "GitHub CLI", "outcome": "installed", "step": "github" },
                { "id": "github", "label": "GitHub", "outcome": "present", "note": SIGNED_IN_THERE, "step": "github" },
            ]),
        );
        retry(&kept, "a2").await;
        finished(&kept, "a2").await;
        assert!(!kept.provisioner.ran_words().contains(&"github"));
        assert_eq!(
            rows_of_step(&kept, "github"),
            vec![
                ("github/gh".to_string(), "installed".to_string(), None),
                ("github".to_string(), "present".to_string(), Some(SIGNED_IN_THERE.to_string())),
            ]
        );
    });
}

#[test]
fn a_github_vault_word_with_no_token_fails_with_its_fix() {
    block(async {
        let r = rig(SceneOpts::default());
        let sock = computer(&r, &["claude"], Some(&[]), daemon(|_, _| None));
        start(&r, picks_of(json!({ "claude": { "signin": "vault" } }), json!({ "github": { "signin": "vault" } }))).await;
        finished(&r, "a1").await;
        assert_eq!(
            shape(row_of(&r, "github")),
            json!({ "id": "github", "label": "GitHub", "outcome": "failed", "note": no_copy_line("GitHub", HERE), "fix": sign_in_there_fix("spoo"), "step": "github" })
        );
        assert!(exec_frames(&sock, "gh auth status").is_empty());
        assert_eq!(r.ends(), ["needs-you"]);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// A resume, a retry and a page that ran out.

fn resumable(r: &Rig) {
    seed(
        r,
        laptop(),
        None,
        json!({
            "state": "running", "addId": "a_old", "startedAt": "2026-10-24T10:00:00.000Z",
            "steps": [{ "step": "floor", "state": "done", "ms": 5 }, { "step": "agents", "state": "done", "ms": 9 }, { "step": "signins", "state": "done", "ms": 1 }],
            "waiting": [{ "row": "signins/codex", "label": "Codex", "expiresAt": "2026-10-24T10:15:00.000Z", "state": "waiting", "code": "OLD" }],
        }),
        json!([claude_row(), claude_sign_in_row()]),
    );
}

#[test]
fn a_resume_asks_a_waiting_sign_in_afresh() {
    block(async {
        let r = rig(SceneOpts::default());
        computer(&r, &["claude", "codex"], Some(&[]), daemon(|_, _| None));
        resumable(&r);
        r.setup.start_setup("p_spoo", "a_old", None, None).await.expect("resumed");
        until("the page and code", || has_code(&r)).await;
        finished(&r, "a_old").await;
        assert_eq!(*r.signs.started.borrow(), ["codex"]);
        assert_eq!(waiting(&r), vec![codex_wait(1, "waiting", T0)]);
        assert_eq!(r.provisioner.ran_words(), ["skills", "clis", "mcp", "configs", "plugins", "context"]);
    });
}

#[test]
fn a_sign_in_finished_between_runs_reads_there() {
    block(async {
        let r = rig(SceneOpts::default());
        computer(&r, &["claude", "codex"], Some(&[]), daemon(|cmd, _| codex_in(cmd)));
        resumable(&r);
        r.setup.start_setup("p_spoo", "a_old", None, None).await.expect("resumed");
        finished(&r, "a_old").await;
        assert!(r.signs.started.borrow().is_empty());
        assert!(waiting(&r).is_empty());
        assert_eq!(
            shape(row_of(&r, "signins/codex")),
            json!({ "id": "signins/codex", "label": "Codex", "outcome": "present", "note": SIGNED_IN_THERE, "step": "signins" })
        );
    });
}

#[test]
fn a_retry_takes_a_waiting_sign_in_over() {
    block(async {
        let r = codex_waiting().await;
        retry(&r, "a2").await;
        until("the second login started", || r.signs.started.borrow().len() == 2).await;
        until("the first login was let go", || r.signs.left.borrow().len() == 1).await;
        finished(&r, "a2").await;
        assert_eq!(*r.signs.left.borrow(), ["codex"]);
        assert!(r.signs.stopped.borrow().is_empty());
        assert_eq!(waiting(&r), vec![codex_wait(2, "waiting", T0)]);

        let heard = r.setup_frames().len();
        end_codex(&r, json!({ "agent": "codex", "state": "signed-in" }));
        until("the wait went", || waiting(&r).is_empty()).await;
        until("the retry said its end", || r.setup_frames()[heard..].iter().any(|f| f.end.is_some())).await;
        let after = r.setup_frames().split_off(heard);
        assert!(after.iter().all(|f| f.add_id == "a2"));
        assert_eq!(
            shape(row_of(&r, "signins/codex")),
            json!({ "id": "signins/codex", "label": "Codex", "outcome": "installed", "note": SIGNED_IN_THERE, "step": "signins" })
        );
    });
}

#[test]
fn a_sign_in_whose_page_ran_out_reads_expired() {
    block(async {
        let r = codex_waiting().await;
        r.clock.pass(WAIT_MS).await;
        end_codex(&r, json!({ "agent": "codex", "state": "failed", "said": "the sign-in ran out" }));
        until("the wait read expired", || waiting(&r) == vec![codex_wait(1, "expired", T0)]).await;
        assert!(row_of(&r, "signins/codex").is_none());
        assert!(r.signs.left.borrow().is_empty());

        retry(&r, "a2").await;
        until("the second login started", || r.signs.started.borrow().len() == 2).await;
        until("a fresh page", || waiting(&r) == vec![codex_wait(2, "waiting", r.clock.now())]).await;
        assert!(r.signs.left.borrow().is_empty());
    });
}

#[test]
fn an_end_inside_the_slack_reads_expired() {
    block(async {
        let r = codex_waiting().await;
        r.clock.pass(WAIT_MS - SLACK_MS).await;
        end_codex(&r, json!({ "agent": "codex", "state": "failed", "said": "the sign-in ran out" }));
        until("the wait read expired", || waiting(&r) == vec![codex_wait(1, "expired", T0)]).await;
        assert!(row_of(&r, "signins/codex").is_none());

        let skip = r.setup.skippers.borrow().get("p_spoo/signins/codex").cloned().expect("an expired wait can still be skipped");
        skip().await;
        assert!(waiting(&r).is_empty());
        assert_eq!(
            shape(row_of(&r, "signins/codex")),
            json!({ "id": "signins/codex", "label": "Codex", "outcome": "skipped", "note": SKIPPED_FOR_NOW, "step": "signins" })
        );
        assert_eq!(*r.signs.stopped.borrow(), ["codex"]);
    });
}

#[test]
fn an_early_failure_lands_failed() {
    block(async {
        let r = codex_waiting().await;
        r.clock.pass(WAIT_MS - SLACK_MS - 1).await;
        end_codex(&r, json!({ "agent": "codex", "state": "failed", "said": "the code was wrong" }));
        until("the row failed", || row_of(&r, "signins/codex").is_some()).await;
        assert!(waiting(&r).is_empty());
        assert_eq!(shape(row_of(&r, "signins/codex")), failed_shape("signins/codex", "Codex", "the code was wrong", "signins"));
        assert!(!r.setup.skippers.borrow().contains_key("p_spoo/signins/codex"));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The copy of a login held here.

fn copying(r: &Rig) {
    *r.recording.login.borrow_mut() = Some(Some(CODEX_AUTH.as_bytes().to_vec()));
    *r.recording.stores.borrow_mut() = Some(IndexMap::from([("codex".to_string(), STORE.to_string())]));
}

fn both_vault() -> RecipeFile {
    picks_of(json!({ "claude": { "signin": "vault" }, "codex": { "signin": "vault" } }), json!({}))
}

#[test]
fn a_login_from_here_is_copied_on_stdin_alone() {
    block(async {
        let r = rig(SceneOpts::default());
        copying(&r);
        let landed_input: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let seen = Rc::clone(&landed_input);
        let sock = computer(
            &r,
            &["claude", "codex"],
            Some(&[]),
            daemon(move |cmd, input| {
                if cmd.contains("codex login status") {
                    return Some(if seen.borrow().is_some() { (0, "Logged in using ChatGPT\n".to_string()) } else { (1, "Not logged in\n".to_string()) });
                }
                if cmd.contains("/var/lib/wsp/logins/codex/auth.json") {
                    *seen.borrow_mut() = Some(input.to_string());
                }
                None
            }),
        );
        start(&r, both_vault()).await;
        finished(&r, "a1").await;
        assert_eq!(landed_input.borrow().as_deref(), Some(CODEX_AUTH));
        assert!(!sock.reach.cmds().iter().any(|c| c.contains("rt_TESTONLY")));
        let put = exec_frames(&sock, "auth.json");
        assert_eq!(put.len(), 1);
        assert_eq!(
            put[0]["cmd"],
            format!("umask 077 && mkdir -p '{STORE}' && cat > '{STORE}/auth.json'.wsp && mv -f '{STORE}/auth.json'.wsp '{STORE}/auth.json'")
        );
        assert_eq!(stdin_of(&put[0]), CODEX_AUTH.as_bytes());
        assert_eq!(timeout_of(&put[0]), Some(30_000.0));
        assert_eq!(exec_frames(&sock, "codex login status").len(), 2);
        assert!(r.signs.started.borrow().is_empty());
        assert!(waiting(&r).is_empty());
        assert_eq!(
            shape(row_of(&r, "signins/codex")),
            json!({ "id": "signins/codex", "label": "Codex", "outcome": "installed", "note": copied_from_line(HERE), "step": "signins" })
        );
        assert_eq!(rec(&r).report.logins, Some(vec!["codex/auth.json".to_string()]));
        assert_eq!(r.ends(), ["ready"]);
    });
}

#[test]
fn a_login_there_already_is_left_as_it_is() {
    block(async {
        let r = rig(SceneOpts::default());
        copying(&r);
        let sock = computer(&r, &["claude", "codex"], Some(&[]), daemon(|cmd, _| codex_in(cmd)));
        start(&r, both_vault()).await;
        finished(&r, "a1").await;
        assert!(exec_frames(&sock, "auth.json").is_empty());
        assert_eq!(
            shape(row_of(&r, "signins/codex")),
            json!({ "id": "signins/codex", "label": "Codex", "outcome": "present", "note": SIGNED_IN_THERE, "step": "signins" })
        );
    });
}

#[test]
fn no_copy_where_the_agent_did_not_install() {
    block(async {
        let r = rig(SceneOpts::default());
        copying(&r);
        r.provisioner.rows.borrow_mut().insert(EngineStep::Agents, vec![row(json!({ "id": "agents/codex", "label": "Codex", "outcome": "failed", "note": "npm ERR! 404" }))]);
        let sock = computer(
            &r,
            &["claude", "codex"],
            Some(&[]),
            daemon(|cmd, _| cmd.contains("codex login status").then(|| (127, "bash: line 2: codex: command not found\n".to_string()))),
        );
        start(&r, both_vault()).await;
        finished(&r, "a1").await;
        assert!(exec_frames(&sock, "auth.json").is_empty());
        assert_eq!(
            shape(row_of(&r, "signins/codex")),
            json!({ "id": "signins/codex", "label": "Codex", "outcome": "skipped", "note": waits_for_install_line("Codex"), "step": "signins" })
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// One sign-in run again.

#[test]
fn a_way_change_runs_one_sign_in() {
    block(async {
        let r = rig(opts_with_vault(&[]));
        computer(&r, &["claude", "codex"], Some(&[]), daemon(|_, _| None));
        let token_picks = picks_of(json!({ "claude": { "signin": "token" }, "codex": { "signin": "machine" } }), json!({}));
        start(&r, token_picks).await;
        until("the codex page and code", || has_code(&r)).await;
        finished(&r, "a1").await;
        assert_eq!(
            pick(row_of(&r, "signins/claude"), &["id", "outcome", "note", "fix", "step"]),
            json!({
                "id": "signins/claude", "outcome": "failed", "note": NO_SIGN_IN_ROAD, "step": "signins",
                "fix": claude_fix(&[SignInWay::Token, SignInWay::Key, SignInWay::Machine]),
            })
        );
        let skip = r.setup.skippers.borrow().get("p_spoo/signins/codex").cloned().expect("a skipper");
        skip().await;
        assert_eq!(*r.signs.started.borrow(), ["codex"]);

        r.provisioner.ran.borrow_mut().clear();
        r.vault.borrow_mut().insert("ANTHROPIC_API_KEY".into(), "sk-ant-api03-x".into());
        let key_picks = picks_of(json!({ "claude": { "signin": "key" }, "codex": { "signin": "machine" } }), json!({}));
        r.setup.start_setup("p_spoo", "a2", Some(GivenPicks { picks: key_picks, recipe: None }), Some("signins/claude")).await.expect("one row run again");
        finished(&r, "a2").await;
        assert_eq!(r.provisioner.ran_words(), ["context"]);
        assert_eq!(*r.signs.started.borrow(), ["codex"]);
        assert_eq!(
            shape(row_of(&r, "signins/claude")),
            json!({ "id": "signins/claude", "label": "Claude Code", "outcome": "present", "note": key_held_line(HERE), "step": "signins" })
        );
        assert_eq!(
            shape(row_of(&r, "signins/codex")),
            json!({ "id": "signins/codex", "label": "Codex", "outcome": "skipped", "note": SKIPPED_FOR_NOW, "step": "signins" })
        );
        assert!(waiting(&r).is_empty());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Skip for now.

#[test]
fn skip_for_now_lands_skipped_and_stops_the_login() {
    block(async {
        let r = codex_waiting().await;
        let skip = r.setup.skippers.borrow().get("p_spoo/signins/codex").cloned().expect("a skipper");
        skip().await;
        assert!(waiting(&r).is_empty());
        assert_eq!(
            shape(row_of(&r, "signins/codex")),
            json!({ "id": "signins/codex", "label": "Codex", "outcome": "skipped", "note": SKIPPED_FOR_NOW, "step": "signins" })
        );
        assert_eq!(*r.signs.stopped.borrow(), ["codex"]);
        assert_eq!(*r.signs.left.borrow(), ["codex"]);
        assert!(!r.setup.skippers.borrow().contains_key("p_spoo/signins/codex"));
        assert!(!r.setup.signing_in.borrow().contains_key("p_spoo/codex"));

        end_codex(&r, json!({ "agent": "codex", "state": "signed-in" }));
        settle().await;
        assert_eq!(row_of(&r, "signins/codex").unwrap()["outcome"], "skipped");
        until("the setup came out ready", || r.ends().last().map(String::as_str) == Some("ready")).await;
        assert_eq!(landed(&r), ["signins/codex"]);
    });
}

#[test]
fn a_second_skip_does_nothing() {
    block(async {
        let r = codex_waiting().await;
        let skip = r.setup.skippers.borrow().get("p_spoo/signins/codex").cloned().expect("a skipper");
        skip().await;
        let frames = r.setup_frames().len();
        let before = row_of(&r, "signins/codex");
        skip().await;
        settle().await;
        assert_eq!(*r.signs.stopped.borrow(), ["codex"]);
        assert_eq!(*r.signs.left.borrow(), ["codex"]);
        assert_eq!(row_of(&r, "signins/codex"), before);
        assert_eq!(r.setup_frames().len(), frames);
        assert_eq!(landed(&r), ["signins/codex"]);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Sign-ins no run can start.

#[test]
fn a_sign_in_that_asks_the_person_is_left_to_its_terminal() {
    block(async {
        let r = rig(SceneOpts::default());
        r.provisioner.rows.borrow_mut().insert(EngineStep::Agents, vec![row(json!({ "id": "agents/opencode", "label": "OpenCode", "outcome": "installed" }))]);
        computer(&r, &["opencode"], Some(&[]), daemon(|_, _| None));
        start(&r, picks_of(json!({ "opencode": { "signin": "machine" } }), json!({}))).await;
        finished(&r, "a1").await;
        assert!(r.signs.started.borrow().is_empty());
        assert!(waiting(&r).is_empty());
        assert_eq!(
            shape(row_of(&r, "signins/opencode")),
            json!({ "id": "signins/opencode", "label": "OpenCode", "outcome": "skipped", "note": AT_ITS_TERMINAL, "step": "signins" })
        );
        assert_eq!(r.ends(), ["ready"]);
    });
}

#[test]
fn a_page_that_hands_back_a_code_is_left_to_its_row() {
    block(async {
        let r = rig(opts_with_vault(&[]));
        computer(&r, &["claude"], Some(&[]), daemon(|_, _| None));
        start(&r, picks_of(json!({ "claude": { "signin": "machine" } }), json!({}))).await;
        finished(&r, "a1").await;
        assert_eq!(
            shape(row_of(&r, "signins/claude")),
            json!({
                "id": "signins/claude", "label": "Claude Code", "outcome": "skipped", "note": CODE_FROM_ROW, "step": "signins",
                "fix": claude_fix(&[SignInWay::Machine, SignInWay::Token, SignInWay::Key]),
            })
        );
        assert!(r.signs.started.borrow().is_empty());
        assert!(waiting(&r).is_empty());
    });
}

#[test]
fn a_sign_in_waits_on_its_agent_that_did_not_install() {
    block(async {
        let r = rig(SceneOpts::default());
        r.provisioner.rows.borrow_mut().insert(EngineStep::Agents, vec![row(json!({ "id": "agents/codex", "label": "Codex", "outcome": "failed", "note": "npm ERR! 404" }))]);
        computer(&r, &["codex"], Some(&[]), daemon(|_, _| None));
        start(&r, picks_of(json!({ "codex": { "signin": "machine" } }), json!({}))).await;
        finished(&r, "a1").await;
        assert!(r.signs.started.borrow().is_empty());
        assert_eq!(
            shape(row_of(&r, "signins/codex")),
            json!({ "id": "signins/codex", "label": "Codex", "outcome": "skipped", "note": waits_for_install_line("Codex"), "step": "signins" })
        );
    });
}

#[test]
fn no_sign_in_road_lands_failed_with_nothing_waiting() {
    block(async {
        let r = rig(SceneOpts::default());
        *r.recording.sign_ins.borrow_mut() = None;
        computer(&r, &["claude", "codex"], Some(&[]), daemon(|_, _| None));
        start(&r, laptop()).await;
        finished(&r, "a1").await;
        assert!(waiting(&r).is_empty());
        assert!(r.setup.skippers.borrow().is_empty());
        assert!(r.setup_frames().iter().all(|f| f.wait.is_none()));
        assert_eq!(shape(row_of(&r, "signins/codex")), failed_shape("signins/codex", "Codex", NO_SIGN_IN_ROAD, "signins"));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The key way and the token way.

#[test]
fn a_key_way_reads_the_vault_or_says_where_the_key_goes() {
    block(async {
        let r = rig(opts_with_vault(&[]));
        computer(&r, &["claude"], Some(&[]), daemon(|_, _| None));
        start(&r, picks_of(json!({ "claude": { "signin": "key" } }), json!({}))).await;
        finished(&r, "a1").await;
        assert_eq!(
            shape(row_of(&r, "signins/claude")),
            json!({
                "id": "signins/claude", "label": "Claude Code", "outcome": "failed", "step": "signins",
                "note": no_key_line("ANTHROPIC_API_KEY", HERE),
                "fix": claude_fix(&[SignInWay::Key, SignInWay::Token, SignInWay::Machine]),
            })
        );

        r.vault.borrow_mut().insert("ANTHROPIC_API_KEY".into(), "sk-ant-api03-x".into());
        r.vault.borrow_mut().insert("CLAUDE_CODE_OAUTH_TOKEN".into(), "sk-ant-oat01-x".into());
        retry(&r, "a2").await;
        finished(&r, "a2").await;
        assert_eq!(
            shape(row_of(&r, "signins/claude")),
            json!({ "id": "signins/claude", "label": "Claude Code", "outcome": "present", "note": key_held_line(HERE), "step": "signins" })
        );
    });
}

#[test]
fn a_token_way_is_minted_here_and_waits() {
    block(async {
        let r = rig(opts_with_vault(&[]));
        let mint = SignIns::new();
        *r.recording.mint.borrow_mut() = Some(Rc::clone(&mint));
        computer(&r, &["claude"], Some(&[]), daemon(|_, _| None));
        start(&r, picks_of(json!({ "claude": { "signin": "token" } }), json!({}))).await;
        until("the minting page and code", || has_code(&r)).await;
        finished(&r, "a1").await;
        assert_eq!(*mint.started.borrow(), ["claude"]);
        assert!(r.signs.started.borrow().is_empty());
        assert_eq!(waiting(&r), vec![agent_wait("claude", "Claude Code", 1, "waiting", T0)]);
        assert!(row_of(&r, "signins/claude").is_none());

        mint.end("claude", sign_in_event(json!({ "agent": "claude", "state": "signed-in" })));
        until("the row landed", || row_of(&r, "signins/claude").is_some()).await;
        assert_eq!(
            shape(row_of(&r, "signins/claude")),
            json!({ "id": "signins/claude", "label": "Claude Code", "outcome": "installed", "note": token_held_line(HERE), "step": "signins" })
        );
        assert!(waiting(&r).is_empty());

        retry(&r, "a2").await;
        until("the second minting", || mint.started.borrow().len() == 2).await;
        mint.end("claude", sign_in_event(json!({ "agent": "claude", "state": "failed", "said": "the browser was closed" })));
        until("the row failed", || row_of(&r, "signins/claude").is_some_and(|v| v["outcome"] == "failed")).await;
        assert_eq!(
            shape(row_of(&r, "signins/claude")),
            json!({
                "id": "signins/claude", "label": "Claude Code", "outcome": "failed", "step": "signins",
                "note": "the browser was closed",
                "fix": claude_fix(&[SignInWay::Token, SignInWay::Key, SignInWay::Machine]),
            })
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// A sync.

fn followed(shelf: Rc<Shelf>, vault: &[(&str, &str)]) -> Rig {
    let mut o = opts_with_vault(vault);
    o.shelf = Some(shelf);
    rig(o)
}

#[test]
fn a_sync_lands_an_added_agent_signed_in_there_as_there() {
    block(async {
        let before = picks_of(json!({ "claude": { "signin": "vault" } }), json!({}));
        let after = picks_of(json!({ "claude": { "signin": "vault" }, "codex": { "signin": "machine" } }), json!({}));
        let r = followed(shelf(after, "h11"), &[("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat01-x")]);
        computer(&r, &["claude", "codex"], Some(&[]), daemon(|cmd, _| codex_in(cmd)));
        seed(&r, before, Some("laptop"), done_setup(), json!([claude_row(), claude_sign_in_row()]));
        r.setup.sync_place("p_spoo").await.expect("synced");
        assert_eq!(rec(&r).applied.map(|a| a.hash), Some("h11".to_string()));
        assert!(r.signs.started.borrow().is_empty());
        assert_eq!(
            shape(row_of(&r, "signins/codex")),
            json!({ "id": "signins/codex", "label": "Codex", "outcome": "present", "note": SIGNED_IN_THERE, "step": "signins" })
        );
        assert_eq!(row_of(&r, "signins/claude").unwrap()["outcome"], "present");
    });
}

#[test]
fn a_sign_in_row_stands_as_its_own_step_reads_it() {
    block(async {
        let before = picks_of(json!({ "claude": { "signin": "vault" }, "codex": { "signin": "machine" } }), json!({}));
        let after = picks_of(json!({ "claude": { "signin": "vault" }, "codex": { "signin": "vault" } }), json!({}));
        let r = followed(shelf(after, "h11"), &[("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat01-x"), ("OPENAI_API_KEY", "sk-x")]);
        computer(&r, &["claude", "codex"], Some(&[]), daemon(|_, _| None));
        seed(
            &r,
            before,
            Some("laptop"),
            done_setup(),
            json!([
                claude_row(),
                claude_sign_in_row(),
                { "id": "signins/codex", "label": "Codex", "outcome": "installed", "note": SIGNED_IN_THERE, "step": "signins" },
            ]),
        );
        r.setup.sync_place("p_spoo").await.expect("synced");
        assert_eq!(rec(&r).applied.map(|a| a.hash), Some("h11".to_string()));
        assert!(r.signs.started.borrow().is_empty());
        assert_eq!(
            shape(row_of(&r, "signins/codex")),
            json!({ "id": "signins/codex", "label": "Codex", "outcome": "present", "note": copied_from_line(HERE), "step": "signins" })
        );
    });
}
