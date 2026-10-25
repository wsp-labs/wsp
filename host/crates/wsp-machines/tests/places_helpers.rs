// SPDX-License-Identifier: AGPL-3.0-only
//! Ports `helpers.ts`'s readers at `packages/runtime/src/places/helpers.ts` (65f6b89c7): every pure reader against `oracle/helpers.json`, plus `bounded`, `login_here` and `box_said`.

mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use async_trait::async_trait;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Value};
use wsp_engine::provision::ProvisionPlan;
use wsp_machines::places::helpers as h;
use wsp_machines::places::types::{HerePlace, PlaceInstalled, PlaceLogReader, PlaceLogin, PlaceResult, PlaceWiring};
use wsp_store::keys::new_place_key_pair;
use wsp_wire::types::recipe_file::{RecipeFile, RecipeSignIn};
use wsp_wire::views::place::{PendingComputer, PlaceApplied, PlaceProvisionRow, PlaceSetup};
use wsp_words::exit::HostError;
use wsp_words::recipe_file::{AgentOption, RecipeOptions};

use common::places::{block, boxed, recipe, report, row, tool, TestClock};

fn fixture() -> &'static Value {
    static FIXTURE: OnceLock<Value> = OnceLock::new();
    FIXTURE.get_or_init(|| serde_json::from_str(include_str!("fixtures/oracle/helpers.json")).expect("the helpers fixture"))
}

fn section(name: &str) -> &'static Vec<Value> {
    fixture()[name].as_array().unwrap_or_else(|| panic!("fixture section {name} is a list"))
}

fn err(message: &str) -> HostError {
    HostError { message: message.to_string(), fix: None, kind: None, errors: None, warnings: None, left: None, marks: Vec::new() }
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_else(|| panic!("{v} is a string")).to_string()
}

fn texts(v: &Value) -> Vec<String> {
    v.as_array().unwrap_or_else(|| panic!("{v} is a list")).iter().map(text).collect()
}

fn pairs(v: &Value) -> Vec<(String, Value)> {
    v.as_object().unwrap_or_else(|| panic!("{v} is an object")).iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

fn vault_of(v: &Value) -> IndexMap<String, String> {
    pairs(v).into_iter().map(|(k, v)| (k, text(&v))).collect()
}

fn rows_of(v: &Value) -> Vec<PlaceProvisionRow> {
    v.as_array().unwrap_or_else(|| panic!("{v} is a list")).iter().map(|r| row(r.clone())).collect()
}

fn word<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("a word")
}

#[test]
fn constants_match_the_ts() {
    let want = fixture()["consts"].as_object().expect("the constants");
    let have: Vec<(&str, Value)> = vec![
        ("JOIN_PROVE_MS", json!(h::JOIN_PROVE_MS)),
        ("LOGINS_READ_MS", json!(h::LOGINS_READ_MS)),
        ("SEEN_EVERY_MS", json!(h::SEEN_EVERY_MS)),
        ("REPLACED", json!(h::REPLACED)),
        ("LINK_FRAME_MS", json!(h::LINK_FRAME_MS)),
        ("RELINK_WAIT_MS", json!(h::RELINK_WAIT_MS)),
        ("BACKEND_FACTS_MS", json!(h::BACKEND_FACTS_MS)),
        ("ADD_FACTS_MS", json!(h::ADD_FACTS_MS)),
        ("CAPACITY_MS", json!(h::CAPACITY_MS)),
        ("DIAL_MS", json!(h::DIAL_MS)),
        ("JOIN_WAIT_MS", json!(h::JOIN_WAIT_MS)),
        ("ADDS_KEPT", json!(h::ADDS_KEPT)),
        ("ADD_RUNNING_LINE", json!(h::ADD_RUNNING_LINE)),
        ("ADD_RUNNING_FIX", json!(h::ADD_RUNNING_FIX)),
        ("PENDING", json!(h::PENDING)),
        ("ADD_STOPPED_LINE", json!(h::ADD_STOPPED_LINE)),
        ("ADD_STOPPED_FIX", json!(h::ADD_STOPPED_FIX)),
        ("ADD_NOT_TAKEN_BACK_LINE", json!(h::ADD_NOT_TAKEN_BACK_LINE)),
        ("NO_ESTIMATE_LINE", json!(h::NO_ESTIMATE_LINE)),
        ("LOG_READ_MS", json!(h::LOG_READ_MS)),
        ("UNDO_MS", json!(h::UNDO_MS)),
        ("SIGN_IN_SLACK_MS", json!(h::SIGN_IN_SLACK_MS)),
        ("GITHUB_MS", json!(h::GITHUB_MS)),
        ("INSTALLS", json!(h::INSTALLS)),
        ("FILES", json!(h::FILES)),
        ("PROBE_MS", json!(h::PROBE_MS)),
        ("SIGNIN_STATUS_MS", json!(h::SIGNIN_STATUS_MS)),
        ("PROVISION_LOG_EVERY_MS", json!(h::PROVISION_LOG_EVERY_MS)),
        ("PROVISION_LOG_LINES", json!(h::PROVISION_LOG_LINES)),
        ("UPDATE_WAIT_MS", json!(h::UPDATE_WAIT_MS)),
        ("UPDATE_POLL_MS", json!(h::UPDATE_POLL_MS)),
        ("UNMERGE_MS", json!(h::UNMERGE_MS)),
        ("PLACE_BAD_KEY_REFUSAL", json!(h::PLACE_BAD_KEY_REFUSAL)),
    ];
    assert_eq!(have.len(), want.len(), "every constant the fixture holds is compared");
    for (name, value) in &have {
        assert_eq!(want.get(*name), Some(value), "{name}");
    }
}

#[test]
fn landed_on_matches_the_ts() {
    for (i, case) in section("landed").iter().enumerate() {
        let rows = rows_of(&case["rows"]);
        let landing = &case["row"];
        let label = landing.get("label").map(text);
        let mut own = landing.clone();
        own["label"] = json!(label.clone().unwrap_or_else(|| "unused".to_string()));
        let landed = h::LandedRow { row: row(own), label };
        let got = h::landed_on(&rows, &landed);
        match case["out"].as_array() {
            None => assert!(got.is_none(), "landed[{i}] lands nothing, got {:?}", got.map(|r| word(&r))),
            Some(_) => assert_eq!(word(&got.unwrap_or_else(|| panic!("landed[{i}] lands rows"))), case["out"], "landed[{i}]"),
        }
    }
}

async fn waiting_on_nothing(clock: &TestClock, ms: u64, what: &str) -> Result<u32, HostError> {
    let (_keep, rx) = tokio::sync::oneshot::channel::<u32>();
    let mut call = Box::pin(h::bounded(clock, async { rx.await.map_err(|_| err("the work was dropped")) }, ms, what));
    assert!(futures::poll!(call.as_mut()).is_pending(), "the work has not answered");
    clock.pass(ms as i64).await;
    call.await
}

#[test]
fn bounded_says_what_was_not_answered() {
    block(async {
        let clock = TestClock::new();
        let (_keep, rx) = tokio::sync::oneshot::channel::<u32>();
        let mut call = Box::pin(h::bounded(&*clock, async { rx.await.map_err(|_| err("the work was dropped")) }, 10_000, "the daemon"));
        assert!(futures::poll!(call.as_mut()).is_pending());
        clock.pass(9_999).await;
        assert!(futures::poll!(call.as_mut()).is_pending(), "nothing is said before the bound");
        clock.pass(1).await;
        let said = call.await.expect_err("the bound ran out");
        assert_eq!(said.message, "the daemon was not answered in 10s");
        assert_eq!(clock.pending(), 0);
    });
}

#[test]
fn bounded_rounds_half_up() {
    block(async {
        for (ms, said) in [(500, "1s"), (499, "0s"), (2_500, "3s")] {
            let clock = TestClock::new();
            let got = waiting_on_nothing(&clock, ms, "that computer").await.expect_err("the bound ran out");
            assert_eq!(got.message, format!("that computer was not answered in {said}"), "{ms} ms");
        }
    });
}

#[test]
fn bounded_passes_the_works_own_error() {
    block(async {
        let clock = TestClock::new();
        let got = h::bounded(&*clock, async { Err::<u32, HostError>(err("the daemon said no")) }, 10_000, "that computer").await;
        assert_eq!(got.expect_err("the work's error").message, "the daemon said no");
        assert_eq!(clock.pending(), 0);
    });
}

#[test]
fn bounded_leaves_no_timer_once_settled() {
    block(async {
        let clock = TestClock::new();
        let (tx, rx) = tokio::sync::oneshot::channel::<u32>();
        let mut call = Box::pin(h::bounded(&*clock, async { rx.await.map_err(|_| err("the work was dropped")) }, 10_000, "that computer"));
        assert!(futures::poll!(call.as_mut()).is_pending());
        assert_eq!(clock.pending(), 1, "the bound is armed while the work runs");
        tx.send(7).expect("the work is still waiting");
        assert_eq!(call.await.expect("the work's answer"), 7);
        assert_eq!(clock.pending(), 0);
        clock.pass(20_000).await;
    });
}

#[test]
fn reads_as_ed25519_matches_the_ts() {
    for case in section("ed") {
        assert_eq!(h::reads_as_ed25519(case["key"].as_str().unwrap()), case["out"].as_bool().unwrap(), "{}", case["note"]);
    }
}

#[test]
fn place_home_refusal_matches_the_ts() {
    for case in section("homeRefusal") {
        let said = case["said"].as_str();
        assert_eq!(h::place_home_refusal(said), text(&case["out"]), "{}", case["said"]);
    }
}

#[test]
fn taken_report_matches_the_ts() {
    for (i, case) in section("taken").iter().enumerate() {
        let given = report("maya", json!({ "login": case["login"], "dialed": "https://relay.example/link" }));
        let got = h::taken_report(&given);
        match (case["out"].get("ok"), case["out"].get("err")) {
            (Some(ok), _) => {
                let taken = got.unwrap_or_else(|e| panic!("taken[{i}] is taken, refused with {}", e.message));
                let login: Vec<(String, Value)> = taken.login.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
                assert_eq!(login, pairs(&ok["login"]), "taken[{i}] login, in its order");
                let (mut before, mut after) = (word(&given), word(&taken));
                before.as_object_mut().unwrap().remove("login");
                after.as_object_mut().unwrap().remove("login");
                assert_eq!(after, before, "taken[{i}] changes nothing but the login");
            }
            (None, Some(refusal)) => assert_eq!(got.expect_err("refused").message, text(refusal), "taken[{i}]"),
            _ => panic!("taken[{i}] has neither an answer nor a refusal"),
        }
    }
}

#[test]
fn agents_on_matches_the_ts() {
    for (i, case) in section("agentsOn").iter().enumerate() {
        let got = h::agents_on(&texts(&case["report"]["agents"]), &rows_of(&case["rows"]));
        assert_eq!(got, texts(&case["out"]), "agentsOn[{i}]");
    }
}

fn sign_ins(case: &Value, vault: &IndexMap<String, String>) -> Option<IndexMap<String, wsp_wire::types::agents_report::AgentSignInState>> {
    let logins = case["report"].get("logins").map(texts);
    let agents = case.get("agents").map(texts);
    h::sign_ins_of(&texts(&case["report"]["agents"]), logins.as_deref(), vault, &rows_of(&case["rows"]), agents.as_deref())
}

#[test]
fn sign_ins_of_matches_the_ts() {
    for (i, case) in section("signInsCases").iter().enumerate() {
        let got = sign_ins(case, &vault_of(&case["vault"]));
        match case["out"].as_object() {
            None => assert!(got.is_none(), "signInsCases[{i}] is unknown"),
            Some(_) => {
                let got = got.unwrap_or_else(|| panic!("signInsCases[{i}] is known"));
                let words: Vec<(String, Value)> = got.iter().map(|(k, v)| (k.clone(), word(v))).collect();
                assert_eq!(words, pairs(&case["out"]), "signInsCases[{i}]");
            }
        }
    }
}

#[test]
fn sign_ins_of_is_none_where_the_daemon_lists_no_logins() {
    let unknown = &section("signInsCases")[0];
    assert!(unknown["report"].get("logins").is_none() && unknown["out"].is_null());
    let held = vault_of(&json!({ "CLAUDE_CODE_OAUTH_TOKEN": "t", "ANTHROPIC_API_KEY": "k", "OPENAI_API_KEY": "o" }));
    assert!(sign_ins(unknown, &IndexMap::new()).is_none());
    assert!(sign_ins(unknown, &held).is_none(), "a held vault does not make unknown known");
    let mut with_rows = unknown.clone();
    with_rows["rows"] = json!([{ "id": "signins/claude", "label": "Claude Code", "outcome": "installed", "note": "signed in on that computer" }]);
    assert!(sign_ins(&with_rows, &held).is_none(), "signed-in rows do not make unknown known");
}

#[test]
fn shared_login_file_matches_the_ts() {
    let want = fixture()["facts"]["sharedLoginFile"].as_object().expect("the shared files by agent");
    assert!(!want.is_empty());
    for (agent, file) in want {
        assert_eq!(h::shared_login_file(agent), file.as_str().map(str::to_string), "{agent}");
    }
}

#[test]
fn shared_logins_script_matches_the_ts() {
    for case in section("scripts") {
        assert_eq!(h::shared_logins_script(case["logins"].as_str().unwrap()), text(&case["out"]), "{}", case["logins"]);
    }
}

#[test]
fn relisted_logins_matches_the_ts() {
    for (i, case) in section("relisted").iter().enumerate() {
        let got = h::relisted_logins(&texts(&case["listed"]), &texts(&case["found"]));
        let want = case["out"].as_array().map(|_| texts(&case["out"]));
        assert_eq!(got, want, "relisted[{i}]");
    }
}

#[test]
fn vault_sign_in_matches_the_ts() {
    let cases = section("vaultSignIn");
    assert!(!cases.is_empty());
    for case in cases {
        let got = h::vault_sign_in(case["agent"].as_str().unwrap(), &vault_of(&case["vault"]));
        assert_eq!(word(&got), case["out"], "{} with {}", case["agent"], case["vault"]);
    }
}

#[test]
fn vault_held_line_matches_the_ts() {
    let cases = section("vaultHeld");
    assert!(!cases.is_empty());
    for case in cases {
        let way: Option<RecipeSignIn> = case["way"].as_str().map(|w| serde_json::from_value(json!(w)).expect("a sign-in way"));
        let got = h::vault_held_line(case["agent"].as_str().unwrap(), &vault_of(&case["vault"]), way, "mac");
        assert_eq!(got, text(&case["out"]), "{} with {} picked {}", case["agent"], case["vault"], case["way"]);
    }
}

#[test]
fn picked_vault_matches_the_ts() {
    for (i, case) in section("picked").iter().enumerate() {
        let vault = vault_of(&case["vault"]);
        let picks: Option<RecipeFile> = case["picks"].as_object().map(|p| recipe(json!({ "name": "x", "agents": p["agents"] })));
        let got = h::picked_vault(&vault, picks.as_ref());
        let got: Vec<(String, Value)> = got.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        assert_eq!(got, pairs(&case["out"]), "picked[{i}]");
    }
}

fn options_of(v: &Value) -> RecipeOptions {
    let agents = v["agents"]
        .as_array()
        .expect("agent options")
        .iter()
        .map(|a| AgentOption {
            id: text(&a["id"]),
            name: text(&a["id"]),
            signins: a["signins"].as_array().unwrap().iter().map(|w| serde_json::from_value(w.clone()).expect("a sign-in way")).collect(),
            kind: None,
            bytes: None,
        })
        .collect();
    RecipeOptions { agents, mcp: Vec::new(), clis: Vec::new(), skills: Vec::new(), plugins: Vec::new(), configs: Vec::new(), folders: None }
}

#[test]
fn servable_options_matches_the_ts() {
    for (i, case) in section("servable").iter().enumerate() {
        let held: IndexSet<String> = texts(&case["held"]).into_iter().collect();
        let got = h::servable_options(&options_of(&case["options"]), &vault_of(&case["vault"]), &held);
        assert_eq!(got, options_of(&case["out"]), "servable[{i}]");
    }
}

fn homes_of(homes: Result<IndexMap<String, String>, String>) -> h::HomesHere {
    Rc::new(move || {
        let homes = homes.clone();
        boxed(async move { homes.map_err(|m| err(&m)) })
    })
}

#[test]
fn login_here_reads_the_shared_file_and_nothing_else() {
    block(async {
        let shared = text(&fixture()["facts"]["sharedLoginFile"]["codex"]);
        let file = shared.rsplit('/').next().expect("the file's name").to_string();
        let home = tempfile::tempdir().expect("a home");
        let at = |agent: &str| IndexMap::from([(agent.to_string(), home.path().to_str().unwrap().to_string())]);

        assert_eq!(h::login_here(homes_of(Ok(at("codex"))), "codex").await, None, "no file yet");

        std::fs::write(home.path().join(&file), b"").unwrap();
        assert_eq!(h::login_here(homes_of(Ok(at("codex"))), "codex").await, None, "an empty file");

        std::fs::write(home.path().join(&file), b"{\"tokens\":\"abc\"}\n").unwrap();
        assert_eq!(h::login_here(homes_of(Ok(at("codex"))), "codex").await, Some(b"{\"tokens\":\"abc\"}\n".to_vec()));

        assert_eq!(h::login_here(homes_of(Ok(at("gemini"))), "gemini").await, None, "an agent whose login no workspace shares");
        assert_eq!(h::login_here(homes_of(Ok(at("gemini"))), "codex").await, None, "no home for the agent asked about");
        assert_eq!(h::login_here(homes_of(Err("no homes".to_string())), "codex").await, None, "a failed homes read");
    });
}

struct FakeLog {
    saw: RefCell<Vec<PlaceLogin>>,
    says: Result<Vec<String>, String>,
}

#[async_trait(?Send)]
impl PlaceLogReader for FakeLog {
    async fn log(&self, login: &PlaceLogin) -> PlaceResult<Vec<String>> {
        self.saw.borrow_mut().push(login.clone());
        self.says.clone().map_err(|m| err(&m))
    }
}

fn wiring(log: Option<Rc<dyn PlaceLogReader>>) -> PlaceWiring {
    let here = HerePlace { name: "zingzys-mac".into(), ..Default::default() };
    PlaceWiring {
        host_key: new_place_key_pair(),
        provider: None,
        here: Rc::new(move || here.clone()),
        host_name: Rc::new(|| "zingzys-mac".to_string()),
        install: None,
        dial: None,
        log,
        update: None,
        leave: None,
        forget_key: None,
        sudo_over: None,
        provision: None,
        github_token: None,
        undo: None,
        run_over: None,
        back: None,
    }
}

fn installed(ssh: Option<&str>, key_path: Option<&str>) -> PlaceInstalled {
    PlaceInstalled { name: "box".into(), ssh: ssh.map(Into::into), ssh_key_path: key_path.map(Into::into), ..Default::default() }
}

#[test]
fn box_said_is_empty_without_a_log_road_or_an_ssh_login() {
    block(async {
        assert_eq!(h::box_said(&wiring(None), &installed(Some("root@10.0.0.4"), None)).await, Vec::<String>::new(), "no log road");
        let log = Rc::new(FakeLog { saw: RefCell::new(Vec::new()), says: Ok(vec!["could not dial".to_string()]) });
        let with_log = wiring(Some(Rc::clone(&log) as Rc<dyn PlaceLogReader>));
        assert_eq!(h::box_said(&with_log, &installed(None, Some("/k"))).await, Vec::<String>::new(), "no ssh login");
        assert!(log.saw.borrow().is_empty(), "the log is not read without a login");
    });
}

#[test]
fn box_said_swallows_a_failed_read() {
    block(async {
        let log = Rc::new(FakeLog { saw: RefCell::new(Vec::new()), says: Err("ssh: connection refused".to_string()) });
        let said = h::box_said(&wiring(Some(Rc::clone(&log) as Rc<dyn PlaceLogReader>)), &installed(Some("root@10.0.0.4"), None)).await;
        assert_eq!(said, Vec::<String>::new());
        assert_eq!(log.saw.borrow().len(), 1, "the read was tried");
    });
}

#[test]
fn box_said_reads_the_log_with_the_key_path_only_where_there_is_one() {
    block(async {
        let log = Rc::new(FakeLog { saw: RefCell::new(Vec::new()), says: Ok(vec!["dial tcp 10.0.0.4:22: i/o timeout".to_string(), "retrying".to_string()]) });
        let with_log = wiring(Some(Rc::clone(&log) as Rc<dyn PlaceLogReader>));
        let lines = h::box_said(&with_log, &installed(Some("root@10.0.0.4"), Some("/keys/box"))).await;
        assert_eq!(lines, vec!["dial tcp 10.0.0.4:22: i/o timeout".to_string(), "retrying".to_string()]);
        h::box_said(&with_log, &installed(Some("root@10.0.0.4"), None)).await;
        assert_eq!(
            *log.saw.borrow(),
            vec![
                PlaceLogin { ssh: "root@10.0.0.4".into(), key_path: Some("/keys/box".into()), host_key: None },
                PlaceLogin { ssh: "root@10.0.0.4".into(), key_path: None, host_key: None },
            ]
        );
    });
}

#[test]
fn pending_view_drops_the_door_only_fields() {
    let view: PendingComputer = serde_json::from_value({
        let mut v = fixture()["pending"].clone();
        v["choices"] = word(&recipe(json!({ "name": "x" })));
        v
    })
    .expect("a pending computer");
    let record = h::PendingRecord { view: view.clone(), key_path: Some("/k".into()), host_key: Some("hk".into()), undo: Some("u".into()), login: Some("root@h".into()) };
    let got = h::pending_view(&record);
    assert_eq!(got, view);
    let wire = word(&got);
    for door_only in ["keyPath", "hostKey", "undo", "login"] {
        assert!(wire.get(door_only).is_none(), "{door_only} stays in the door");
    }
    assert_eq!(wire["id"], fixture()["pending"]["id"]);
    assert_eq!(wire["address"], fixture()["pending"]["address"]);
}

#[test]
fn setup_outcome_matches_the_ts() {
    for (i, case) in section("outcome").iter().enumerate() {
        let setup: PlaceSetup = serde_json::from_value(json!({
            "state": "running", "addId": "a", "startedAt": "t", "steps": [], "waiting": case["setup"]["waiting"],
        }))
        .expect("a setup");
        let applied: Option<PlaceApplied> = (!case["applied"].is_null()).then(|| serde_json::from_value(case["applied"].clone()).expect("applied rows"));
        let (end, said) = h::setup_outcome(&setup, applied.as_ref());
        assert_eq!(word(&end), case["out"]["end"], "outcome[{i}] end");
        assert_eq!(said, case["out"].get("said").map(text), "outcome[{i}] said");
    }
}

fn plan_of(agents: usize, steps: &[(&str, Option<&str>)]) -> ProvisionPlan {
    let mut plan = ProvisionPlan::default();
    plan.agents = agents;
    plan.path = "/opt/bin:/usr/bin".into();
    plan.recipe_at = "laptop".into();
    plan.steps = steps.iter().map(|(id, after)| tool(id, id, "npm", *after)).collect();
    plan
}

fn ids(plan: &ProvisionPlan) -> Vec<String> {
    plan.steps.iter().map(|s| s.id.clone()).collect()
}

fn cli_plan() -> ProvisionPlan {
    plan_of(2, &[("a1", None), ("a2", None), ("c1", None), ("c2", Some("c1")), ("gh", None), ("lfs", Some("c2")), ("c5", None)])
}

fn only_reordered(got: &ProvisionPlan, plan: &ProvisionPlan) {
    assert_eq!((got.agents, &got.path, &got.recipe_at), (plan.agents, &plan.path, &plan.recipe_at));
    for step in &got.steps {
        assert_eq!(Some(step), plan.steps.iter().find(|s| s.id == step.id), "{} is carried as it was", step.id);
    }
}

#[test]
fn cli_first_matches_the_ts() {
    let plan = cli_plan();
    for case in section("cli") {
        let got = h::cli_first(&plan, &texts(&case["ids"]));
        assert_eq!(ids(&got), texts(&case["out"]), "{}", case["ids"]);
        only_reordered(&got, &plan);
    }
}

#[test]
fn cli_first_answers_the_same_plan_when_no_id_is_a_cli() {
    let plan = cli_plan();
    let same: Vec<&Value> = section("cli").iter().filter(|c| c["same"].as_bool().unwrap()).collect();
    assert_eq!(same.len(), 3, "no ids, an unknown id and an agent step");
    for case in same {
        let got = h::cli_first(&plan, &texts(&case["ids"]));
        assert_eq!(ids(&got), ids(&plan), "{}", case["ids"]);
        assert_eq!(got.steps, plan.steps, "{}", case["ids"]);
        only_reordered(&got, &plan);
    }
}

#[test]
fn cli_first_stops_at_a_cycle() {
    let plan = plan_of(0, &[("x", Some("y")), ("y", Some("x"))]);
    let got = h::cli_first(&plan, &["x".to_string()]);
    assert_eq!(ids(&got), texts(&fixture()["cliCycle"]));
}

#[test]
fn engine_row_matches_the_ts() {
    for case in section("engineRow") {
        assert_eq!(h::engine_row(&row(case["row"].clone())), case["out"].as_bool().unwrap(), "{}", case["row"]);
    }
}

#[test]
fn first_line_of_matches_the_ts() {
    let cases = section("firstLine");
    for (at, message) in [(0, "one\ntwo"), (1, "plain\nsecond"), (2, "")] {
        assert_eq!(h::first_line_of(&err(message)), text(&cases[at]["out"]), "firstLine[{at}]");
    }
}

#[test]
fn picks_hash_matches_the_ts() {
    for case in section("hashes") {
        let file: RecipeFile = serde_json::from_value(case["file"].clone()).expect("a recipe file");
        assert_eq!(h::picks_hash(&file), text(&case["out"]), "{}", case["file"]["name"]);
    }
}
