// SPDX-License-Identifier: AGPL-3.0-only
//! packages/adapter-codex/test/adapter.test.ts: the side question, the rewind and fork cases, the thread picked up from
//! outside wsp and the lost-session case; catalog.test.ts's brokered probe; placeholder-login.test.ts's three adapter
//! cases; session-title.test.ts's and session-held.test.ts's adapter cases.

#[path = "common/codex_shell.rs"]
mod codex_shell;
#[path = "common/codex_wire.rs"]
mod codex_wire;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::{LocalBoxFuture, FutureExt};
use indexmap::IndexMap;
use regex::Regex;
use serde_json::{json, Map, Value};
use wsp_adapters::codex::command::{build_env, AccountModels, BrokerMode, CodexBroker, CodexEnvOptions, DOOR, KEY_FILE_ENV};
use wsp_adapters::codex::conversations::LETS_GO;
use wsp_adapters::codex::catalog::parse_catalog_probe;
use wsp_adapters::codex::{create_codex_adapter, CodexAdapter, CodexAdapterDeps};
use wsp_adapters::pending::{
    aside_wall_line, codex_no_turn_line, codex_not_signed_in_line, conversation_written_elsewhere_line, lost_session_prompt, CodexClaims, CODEX_FEWER_TURNS,
    CODEX_LEGACY_HISTORY, CODEX_PINNED_VERSION, LOST_SESSION_NOTE,
};
use wsp_adapters::port::{
    agent_word, AdapterEvent, AgentLaunch, AsideAnswer, AsideQuestion, CountedTurn, DeltaKind, DraftAsk, Env, ExecFailure, ForkTurn, HarnessCatalogAnswer, HarnessCatalogProbe,
    HarnessExec, HarnessStartOptions, KeptTurn, RevertAsk, RevertAt, Reverted, TitleTurn, TurnResult, TurnStatus,
};
use wsp_wire::types::shell_quote::ENV_FROM_INPUT;
use wsp_wire::views::session::TurnRefusal;

use codex_shell::{plain_env, run_shell, sqlite, write_stub, TempDir};
use codex_wire::{launcher, no_events, scripted_server, server, until, wire, Events, Launch, OnWrite, Wire, WireOpts};

const THREAD_ID: &str = "01a0e2c1-5d10-7b42-9a6e-3f1c2d4b5a60";
const TURN_ID: &str = "01a0e2c1-5e02-7c11-8d3f-9b2a1c0d4e71";
const LOGIN: &str = "codex login --device-auth";

fn method(m: &Value) -> &str {
    m.get("method").and_then(Value::as_str).unwrap_or("")
}

/// The lines a live server prints up to a running turn, for the cases that drive the rest by hand.
fn opened(thread_id: &str) -> Vec<String> {
    vec![
        json!({"id":"wsp-initialize","result":{"userAgent":"wsp/0.155.1","codexHome":"/root/.codex","platformFamily":"unix","platformOs":"linux"}}).to_string(),
        json!({"id":"wsp-thread","result":{"thread":{"id":thread_id,"model":"gpt-5.6-sol","cwd":"/root/app","turns":[]},"model":"gpt-5.6-sol","cwd":"/root/app","approvalPolicy":"on-request","sandbox":{"type":"workspaceWrite"}}}).to_string(),
    ]
}

fn turn_started() -> String {
    json!({"method":"turn/started","params":{"threadId":THREAD_ID,"turn":{"id":TURN_ID,"items":[],"status":"inProgress"}}}).to_string()
}

fn completed(status: &str) -> String {
    json!({"method":"turn/completed","params":{"threadId":THREAD_ID,"turn":{"id":TURN_ID,"items":[],"status":status}}}).to_string()
}

fn agent_message(id: &str, text: &str) -> String {
    json!({"method":"item/completed","params":{"item":{"type":"agentMessage","id":id,"text":text},"threadId":THREAD_ID,"turnId":TURN_ID}}).to_string()
}

/// A server that opens the thread and starts the turn, then prints what the case hands it.
fn scripted(rest: Vec<String>, hang: bool) -> impl Fn(&[String]) -> Wire + 'static {
    let mut lines = opened(THREAD_ID);
    lines.push(turn_started());
    lines.extend(rest);
    server(lines, WireOpts { hang, ..Default::default() })
}

/// A launcher whose server answers what wsp writes through `on_write`.
fn answering(on_write: impl Fn(&Value, &Wire) + 'static) -> Launch {
    let on_write: OnWrite = Arc::new(on_write);
    launcher(scripted_server(WireOpts { on_write: Some(on_write), ..Default::default() }))
}

fn adapter_over(launch: &Launch) -> CodexAdapter {
    create_codex_adapter(CodexAdapterDeps { exec: launch.factory(), home: "/root/.codex".into(), login: LOGIN.into(), ..CodexAdapterDeps::new(launch.factory(), "/root/.codex", LOGIN) }).unwrap()
}

fn result_of(status: TurnStatus) -> TurnResult {
    TurnResult {
        status,
        duration_ms: None,
        waited_ms: None,
        cost_usd: None,
        tokens: None,
        model: None,
        models: None,
        text: None,
        error: None,
        unreached: None,
        refusal: None,
        limit: None,
    }
}

fn done(text: &str) -> CountedTurn {
    CountedTurn { anchor: None, result: Some(TurnResult { text: Some(text.into()), ..result_of(TurnStatus::Completed) }) }
}

fn refused_for_sign_in() -> TurnResult {
    TurnResult { error: Some("Codex is not signed in".into()), refusal: Some(TurnRefusal::SignIn), ..result_of(TurnStatus::Failed) }
}

fn first_session_start(events: &Events) -> (usize, String) {
    events
        .all()
        .iter()
        .enumerate()
        .find_map(|(i, e)| match e {
            AdapterEvent::SessionStart { session_id, .. } => Some((i, session_id.clone())),
            _ => None,
        })
        .expect("no session.start")
}

fn find_written(wire: &Wire, id: &str) -> Option<Value> {
    wire.written().into_iter().find(|m| m.get("id").and_then(Value::as_str) == Some(id))
}

fn find_method(wire: &Wire, name: &str) -> Value {
    wire.written_method(name).into_iter().next().unwrap_or_else(|| panic!("nothing wrote {name}"))
}

fn aside_command(cwd: &str) -> String {
    format!("cd '{cwd}' && {} app-server -c tools.update_plan.enabled='true'", agent_word("codex", None))
}

mod a_side_question_on_a_codex_thread {
    use super::*;

    const FORK: &str = "01a0e2d0-0000-7000-8000-000000000001";

    fn forking(on_turn: impl Fn(&Wire) + 'static, hang: bool) -> Launch {
        let on_write: OnWrite = Arc::new(move |message, w| {
            let name = method(message);
            // 0.155.1 refuses an ephemeral fork that would hand back the thread's whole history, in these words.
            if name == "thread/fork" && message["params"]["excludeTurns"] != json!(true) {
                w.push(&[
                    json!({"id":"wsp-initialize","result":{}}).to_string(),
                    json!({"error":{"code":-32600,"message":"ephemeral paginated thread/fork requires `excludeTurns: true`"},"id":"wsp-thread"}).to_string(),
                ]);
            } else if name == "thread/fork" {
                w.push(&[
                    json!({"id":"wsp-initialize","result":{}}).to_string(),
                    json!({"id":"wsp-thread","result":{"thread":{"id":FORK,"ephemeral":true,"forkedFromId":THREAD_ID},"model":"gpt-5.6-sol"}}).to_string(),
                ]);
            }
            if name == "turn/start" {
                on_turn(w);
            }
        });
        launcher(scripted_server(WireOpts { hang, on_write: Some(on_write), ..Default::default() }))
    }

    fn fork_completed() -> String {
        json!({"method":"turn/completed","params":{"threadId":FORK,"turn":{"id":"t1","items":[],"status":"completed"}}}).to_string()
    }

    fn numbers(usage: &IndexMap<String, Value>) -> BTreeMap<String, f64> {
        usage.iter().map(|(k, v)| (k.clone(), v.as_f64().unwrap_or_else(|| panic!("{k} is not a number: {v}")))).collect()
    }

    #[tokio::test]
    async fn forks_the_thread_ephemeral_and_read_only_on_its_own_server_asks_the_question_on() {
        tokio::task::LocalSet::new().run_until(forks_the_thread_ephemeral_and_read_only_on_its_own_server_asks_the_question_on_body()).await
    }

    async fn forks_the_thread_ephemeral_and_read_only_on_its_own_server_asks_the_question_on_body() {
        let launch = forking(
            |w| {
                w.push(&[
                    json!({"method":"item/completed","params":{"item":{"type":"agentMessage","id":"m1","text":"You are in /root/app; you last asked me to count."},"threadId":FORK,"turnId":"t1"}}).to_string(),
                    json!({"method":"thread/tokenUsage/updated","params":{"threadId":FORK,"turnId":"t1","tokenUsage":{"last":{"inputTokens":900,"cachedInputTokens":880,"outputTokens":12,"reasoningOutputTokens":0,"totalTokens":912},"total":{}}}}).to_string(),
                    fork_completed(),
                ]);
            },
            false,
        );
        let adapter = adapter_over(&launch);
        let answer: AsideAnswer = adapter
            .aside(AsideQuestion { session: THREAD_ID.into(), question: "which folder are you in?".into(), cwd: Some("/root/app".into()), model: Some("gpt-5.5".into()), ..Default::default() })
            .await
            .unwrap();
        assert_eq!(answer.text, "You are in /root/app; you last asked me to count.");
        let usage = answer.usage.as_ref().expect("no usage");
        let expected: BTreeMap<String, f64> = [("input", 900.0), ("cached", 880.0), ("output", 12.0), ("reasoning", 0.0), ("context", 912.0)].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        assert_eq!(numbers(usage), expected);
        let call = launch.call(0);
        assert_eq!(call.command, aside_command("/root/app"));
        // A side question is no turn of the account's: it asks neither the account nor its limits.
        let input = call.input.clone().expect("no input");
        let methods: Vec<String> = input.iter().map(|l| method(&codex_wire::parse(l)).to_string()).collect();
        assert_eq!(methods, ["initialize", "initialized", "thread/fork"]);
        let fork = codex_wire::parse(input.last().unwrap());
        assert_eq!(method(&fork), "thread/fork");
        let params = &fork["params"];
        assert_eq!(params["threadId"], json!(THREAD_ID));
        assert_eq!(params["ephemeral"], json!(true));
        assert_eq!(params["sandbox"], json!("read-only"));
        assert_eq!(params["approvalPolicy"], json!("never"));
        assert_eq!(params["model"], json!("gpt-5.5"));
        assert!(params["developerInstructions"].as_str().unwrap_or("").contains("Run nothing"), "{params}");
        let turn = find_method(&launch.wire(0), "turn/start");
        assert_eq!(turn["params"], json!({"threadId": FORK, "input": [{"type": "text", "text": "which folder are you in?"}]}));
        assert!(launch.wire(0).closed());
    }

    #[tokio::test]
    async fn refuses_every_approval_the_fork_asks_for_so_a_side_question_never_runs_anything() {
        tokio::task::LocalSet::new().run_until(refuses_every_approval_the_fork_asks_for_so_a_side_question_never_runs_anything_body()).await
    }

    async fn refuses_every_approval_the_fork_asks_for_so_a_side_question_never_runs_anything_body() {
        let launch = forking(
            |w| {
                w.push(&[
                    json!({"id":3,"method":"item/commandExecution/requestApproval","params":{"threadId":FORK,"turnId":"t1","itemId":"c1","command":"ls","startedAtMs":1}}).to_string(),
                    json!({"method":"item/completed","params":{"item":{"type":"agentMessage","id":"m1","text":"I would rather not run that."},"threadId":FORK,"turnId":"t1"}}).to_string(),
                    fork_completed(),
                ]);
            },
            false,
        );
        let answer = adapter_over(&launch).aside(AsideQuestion { session: THREAD_ID.into(), question: "list the files".into(), ..Default::default() }).await.unwrap();
        assert_eq!(answer.text, "I would rather not run that.");
        let reply = launch.wire(0).written().into_iter().find(|m| m.get("id") == Some(&json!(3))).expect("no answer to approval 3");
        assert_eq!(reply, json!({"id": 3, "result": {"decision": "decline"}}));
    }

    #[tokio::test]
    async fn ends_a_fork_that_answers_nothing_for_the_whole_wall_with_its_tree_and_says_so() {
        tokio::task::LocalSet::new().run_until(ends_a_fork_that_answers_nothing_for_the_whole_wall_with_its_tree_and_says_so_body()).await
    }

    async fn ends_a_fork_that_answers_nothing_for_the_whole_wall_with_its_tree_and_says_so_body() {
        let launch = forking(|_| {}, true);
        let adapter = create_codex_adapter(CodexAdapterDeps { aside_wall_ms: Some(20), interrupt_grace_ms: Some(10), ..CodexAdapterDeps::new(launch.factory(), "/root/.codex", LOGIN) }).unwrap();
        let err = adapter.aside(AsideQuestion { session: THREAD_ID.into(), question: "still there?".into(), ..Default::default() }).await.unwrap_err();
        assert!(err.contains(&aside_wall_line(20)), "{err}");
        assert_eq!(launch.wire(0).order(), ["teardown", "kill"]);
    }

    #[tokio::test]
    async fn a_turn_is_held_to_no_wall_of_the_side_questions() {
        tokio::task::LocalSet::new().run_until(a_turn_is_held_to_no_wall_of_the_side_questions_body()).await
    }

    async fn a_turn_is_held_to_no_wall_of_the_side_questions_body() {
        let launch = launcher(scripted(vec![], true));
        let adapter = create_codex_adapter(CodexAdapterDeps { aside_wall_ms: Some(20), interrupt_grace_ms: Some(10), ..CodexAdapterDeps::new(launch.factory(), "/root/.codex", LOGIN) }).unwrap();
        let session = adapter.start(HarnessStartOptions { prompt: "x".into(), on_event: Some(no_events()), ..Default::default() }).unwrap();
        codex_wire::sleep_ms(60).await;
        assert!(launch.wire(0).order().is_empty());
        session.interrupt().await;
    }

    #[tokio::test]
    async fn rejects_in_the_servers_words_when_the_fork_fails_and_in_the_sign_in_line_when_it() {
        tokio::task::LocalSet::new().run_until(rejects_in_the_servers_words_when_the_fork_fails_and_in_the_sign_in_line_when_it_body()).await
    }

    async fn rejects_in_the_servers_words_when_the_fork_fails_and_in_the_sign_in_line_when_it_body() {
        let refused = answering(|m, w| {
            if method(m) == "thread/fork" {
                w.push1(json!({"error":{"code":-32600,"message":"no rollout found for thread id"},"id":"wsp-thread"}).to_string());
            }
        });
        let err = adapter_over(&refused).aside(AsideQuestion { session: THREAD_ID.into(), question: "x".into(), ..Default::default() }).await.unwrap_err();
        assert!(err.contains("codex could not open the thread: no rollout found for thread id"), "{err}");
        let unsigned = forking(
            |w| {
                w.push1(json!({"method":"turn/completed","params":{"threadId":FORK,"turn":{"id":"t1","items":[],"status":"failed","error":{"message":"unexpected status 401 Unauthorized: Missing bearer"}}}}).to_string());
            },
            false,
        );
        let err = adapter_over(&unsigned).aside(AsideQuestion { session: THREAD_ID.into(), question: "x".into(), ..Default::default() }).await.unwrap_err();
        assert!(err.contains(&codex_not_signed_in_line(LOGIN)), "{err}");
    }
}

mod what_a_rewind_needs_of_a_codex_turn {
    use super::*;

    #[tokio::test]
    async fn cuts_the_threads_history_before_a_turn_on_its_own_short_server_run_resuming_the() {
        tokio::task::LocalSet::new().run_until(cuts_the_threads_history_before_a_turn_on_its_own_short_server_run_resuming_the_body()).await
    }

    async fn cuts_the_threads_history_before_a_turn_on_its_own_short_server_run_resuming_the_body() {
        let launch = answering(|m, w| {
            if method(m) == "thread/resume" {
                w.push(&[json!({"id":"wsp-initialize","result":{}}).to_string(), json!({"id":"wsp-thread","result":{"thread":{"id":THREAD_ID},"model":"gpt-5.6-sol"}}).to_string()]);
            }
            if method(m) == "thread/revert" {
                w.push1(json!({"id":"wsp-revert","result":{"thread":{"id":THREAD_ID,"turns":[]}}}).to_string());
            }
        });
        let reverted = adapter_over(&launch).revert(RevertAsk { session: THREAD_ID.into(), cwd: Some("/root/app".into()), at: RevertAt::BeforeTurn(TURN_ID.into()) }).await.unwrap();
        assert_eq!(reverted, Reverted::Cut);
        let call = launch.call(0);
        assert_eq!(call.command, aside_command("/root/app"));
        let last = codex_wire::parse(call.input.as_ref().unwrap().last().unwrap());
        assert_eq!(method(&last), "thread/resume");
        assert_eq!(last["params"]["threadId"], json!(THREAD_ID));
        let w = launch.wire(0);
        assert_eq!(find_method(&w, "thread/revert"), json!({"id": "wsp-revert", "method": "thread/revert", "params": {"threadId": THREAD_ID, "beforeTurnId": TURN_ID}}));
        assert!(!w.wrote("turn/start"));
        assert!(w.closed());
    }

    /// A server whose thread answers in the history mode given, with its turns newest first in pages of the size
    /// given, and the answer to thread/revert given.
    #[derive(Default, Clone)]
    struct History {
        mode: Option<&'static str>,
        turns: Vec<&'static str>,
        page_size: Option<usize>,
        repeat_cursor: bool,
        revert: Option<String>,
    }

    fn history(o: History) -> Launch {
        answering(move |message, w| {
            let params = &message["params"];
            match method(message) {
                "thread/resume" => {
                    let mut thread = json!({"id": THREAD_ID, "turns": []});
                    if let Some(mode) = o.mode {
                        thread["historyMode"] = json!(mode);
                    }
                    w.push1(json!({"id":"wsp-thread","result":{"thread":thread,"model":"gpt-5.6-sol"}}).to_string());
                }
                "thread/turns/list" => {
                    let from = params["cursor"].as_str().map(|s| s.parse::<usize>().unwrap()).unwrap_or(0);
                    let size = (params["limit"].as_f64().unwrap() as usize).min(o.page_size.unwrap_or(100));
                    let data: Vec<Value> = o.turns.iter().skip(from).take(size).map(|id| json!({"id": id, "items": [], "itemsView": "summary", "status": "completed"})).collect();
                    let next = if o.repeat_cursor {
                        json!("0")
                    } else if from + size < o.turns.len() {
                        json!((from + size).to_string())
                    } else {
                        Value::Null
                    };
                    w.push1(json!({"id": message["id"], "result": {"data": data, "nextCursor": next, "backwardsCursor": null}}).to_string());
                }
                "thread/revert" => w.push1(o.revert.clone().unwrap_or_else(|| json!({"id":"wsp-revert","result":{"thread":{"id":THREAD_ID,"turns":[]}}}).to_string())),
                _ => {}
            }
        })
    }

    fn listed(launch: &Launch) -> Vec<Value> {
        launch.wire(0).written_method("thread/turns/list").into_iter().map(|m| m["params"].clone()).collect()
    }

    fn limits(launch: &Launch) -> Vec<Value> {
        listed(launch).into_iter().map(|p| p["limit"].clone()).collect()
    }

    fn revert_turns(session_turns: Vec<CountedTurn>) -> RevertAsk {
        RevertAsk { session: THREAD_ID.into(), cwd: None, at: RevertAt::Turns(session_turns) }
    }

    fn revert_before(turn: &str) -> RevertAsk {
        RevertAsk { session: THREAD_ID.into(), cwd: None, at: RevertAt::BeforeTurn(turn.into()) }
    }

    const NEWEST_FIRST: [&str; 5] = ["t9", "t8", "t7", "t6", "t5"];

    #[tokio::test]
    async fn rejects_in_the_servers_words_when_it_will_not_cut_so_the_rewind_is_refused_whole() {
        tokio::task::LocalSet::new().run_until(rejects_in_the_servers_words_when_it_will_not_cut_so_the_rewind_is_refused_whole_body()).await
    }

    async fn rejects_in_the_servers_words_when_it_will_not_cut_so_the_rewind_is_refused_whole_body() {
        let launch = history(History {
            mode: Some("paginated"),
            revert: Some(json!({"error":{"code":-32603,"message":format!("timed out shutting down thread {THREAD_ID} before revert")},"id":"wsp-revert"}).to_string()),
            ..Default::default()
        });
        let err = adapter_over(&launch).revert(revert_before(TURN_ID)).await.unwrap_err();
        assert!(err.contains(&format!("codex would not cut the thread: timed out shutting down thread {THREAD_ID} before revert")), "{err}");
    }

    #[tokio::test]
    async fn without_the_turns_id_finds_the_boundary_by_count_in_the_threads_turns_newest_fir() {
        tokio::task::LocalSet::new().run_until(without_the_turns_id_finds_the_boundary_by_count_in_the_threads_turns_newest_fir_body()).await
    }

    async fn without_the_turns_id_finds_the_boundary_by_count_in_the_threads_turns_newest_fir_body() {
        let launch = history(History { mode: Some("paginated"), turns: NEWEST_FIRST.to_vec(), page_size: Some(2), ..Default::default() });
        let reverted = adapter_over(&launch).revert(revert_turns(vec![done("a"), done("b"), done("c")])).await.unwrap();
        assert_eq!(reverted, Reverted::Cut);
        assert_eq!(
            listed(&launch),
            vec![
                json!({"threadId": THREAD_ID, "cursor": null, "limit": 3, "sortDirection": "desc", "itemsView": "summary"}),
                json!({"threadId": THREAD_ID, "cursor": "2", "limit": 1, "sortDirection": "desc", "itemsView": "summary"}),
            ]
        );
        assert_eq!(find_method(&launch.wire(0), "thread/revert")["params"], json!({"threadId": THREAD_ID, "beforeTurnId": "t7"}));
    }

    #[tokio::test]
    async fn counts_only_the_cut_turns_the_server_opened_not_one_refused_for_a_sign_in_nor_on() {
        tokio::task::LocalSet::new().run_until(counts_only_the_cut_turns_the_server_opened_not_one_refused_for_a_sign_in_nor_on_body()).await
    }

    async fn counts_only_the_cut_turns_the_server_opened_not_one_refused_for_a_sign_in_nor_on_body() {
        let launch = history(History { mode: Some("paginated"), turns: NEWEST_FIRST.to_vec(), ..Default::default() });
        let refused = refused_for_sign_in();
        let unopened = TurnResult { error: Some("codex could not open the thread: no rollout found for thread id 01a0".into()), ..result_of(TurnStatus::Failed) };
        adapter_over(&launch)
            .revert(revert_turns(vec![
                CountedTurn { anchor: None, result: Some(refused) },
                done("a"),
                CountedTurn { anchor: None, result: Some(unopened) },
                CountedTurn::default(),
                CountedTurn { anchor: None, result: Some(result_of(TurnStatus::Interrupted)) },
            ]))
            .await
            .unwrap();
        assert_eq!(limits(&launch), vec![json!(3)]);
        assert_eq!(find_method(&launch.wire(0), "thread/revert")["params"], json!({"threadId": THREAD_ID, "beforeTurnId": "t7"}));
    }

    #[tokio::test]
    async fn counts_a_cut_turn_that_kept_the_servers_own_id_as_opened_whatever_its_result_say() {
        tokio::task::LocalSet::new().run_until(counts_a_cut_turn_that_kept_the_servers_own_id_as_opened_whatever_its_result_say_body()).await
    }

    async fn counts_a_cut_turn_that_kept_the_servers_own_id_as_opened_whatever_its_result_say_body() {
        let launch = history(History { mode: Some("paginated"), turns: NEWEST_FIRST.to_vec(), ..Default::default() });
        let refused = refused_for_sign_in();
        adapter_over(&launch)
            .revert(revert_turns(vec![
                CountedTurn { anchor: None, result: Some(refused.clone()) },
                CountedTurn { anchor: Some("t8".into()), result: Some(refused) },
                done("a"),
            ]))
            .await
            .unwrap();
        assert_eq!(limits(&launch), vec![json!(2)]);
        assert_eq!(find_method(&launch.wire(0), "thread/revert")["params"], json!({"threadId": THREAD_ID, "beforeTurnId": "t8"}));
    }

    #[tokio::test]
    async fn refuses_when_the_server_lists_fewer_turns_than_the_rewind_would_cut_and_cuts_not() {
        tokio::task::LocalSet::new().run_until(refuses_when_the_server_lists_fewer_turns_than_the_rewind_would_cut_and_cuts_not_body()).await
    }

    async fn refuses_when_the_server_lists_fewer_turns_than_the_rewind_would_cut_and_cuts_not_body() {
        let launch = history(History { mode: Some("paginated"), turns: vec!["t2", "t1"], page_size: Some(1), ..Default::default() });
        let err = adapter_over(&launch).revert(revert_turns(vec![done("a"), done("b"), done("c")])).await.unwrap_err();
        assert!(err.contains(CODEX_FEWER_TURNS), "{err}");
        assert_eq!(limits(&launch), vec![json!(3), json!(2)]);
        assert!(!launch.wire(0).wrote("thread/revert"));
    }

    #[tokio::test]
    async fn refuses_a_server_that_hands_back_a_page_it_already_gave_before_asking_any_cut() {
        tokio::task::LocalSet::new().run_until(refuses_a_server_that_hands_back_a_page_it_already_gave_before_asking_any_cut_body()).await
    }

    async fn refuses_a_server_that_hands_back_a_page_it_already_gave_before_asking_any_cut_body() {
        let launch = history(History { mode: Some("paginated"), turns: NEWEST_FIRST.to_vec(), page_size: Some(1), repeat_cursor: true, ..Default::default() });
        let err = adapter_over(&launch).revert(revert_turns(vec![done("a"), done("b"), done("c")])).await.unwrap_err();
        assert!(err.contains("codex could not list the thread's turns: it handed back a page it already gave"), "{err}");
        assert!(!launch.wire(0).wrote("thread/revert"));
    }

    #[tokio::test]
    async fn a_thread_an_older_codex_made_answers_kept_by_its_history_mode_before_any_request() {
        tokio::task::LocalSet::new().run_until(a_thread_an_older_codex_made_answers_kept_by_its_history_mode_before_any_request_body()).await
    }

    async fn a_thread_an_older_codex_made_answers_kept_by_its_history_mode_before_any_request_body() {
        let legacy = history(History { mode: Some("legacy"), turns: NEWEST_FIRST.to_vec(), ..Default::default() });
        let reverted = adapter_over(&legacy).revert(revert_before(TURN_ID)).await.unwrap();
        assert_eq!(reverted, Reverted::Kept(CODEX_LEGACY_HISTORY.to_string()));
        let w = legacy.wire(0);
        assert!(!w.wrote("thread/revert") && !w.wrote("thread/turns/list"));
        assert!(w.closed());

        let unsaid = history(History {
            turns: NEWEST_FIRST.to_vec(),
            revert: Some(json!({"error":{"code":-32600,"message":"thread/revert only supports paginated threads"},"id":"wsp-revert"}).to_string()),
            ..Default::default()
        });
        let reverted = adapter_over(&unsaid).revert(revert_turns(vec![done("a")])).await.unwrap();
        assert_eq!(reverted, Reverted::Kept(CODEX_LEGACY_HISTORY.to_string()));
    }

    mod a_fork_of_a_codex_thread {
        use super::*;

        const FORK_ID: &str = "01a12812-adb3-7c12-9acc-a86fcbe74924";

        fn on_fork(line: String) -> String {
            line.replace(THREAD_ID, FORK_ID)
        }

        #[tokio::test]
        async fn forks_the_thread_through_the_turn_on_the_starts_own_server_run_in_the_forks_fold() {
            tokio::task::LocalSet::new().run_until(forks_the_thread_through_the_turn_on_the_starts_own_server_run_in_the_forks_fold_body()).await
        }

        async fn forks_the_thread_through_the_turn_on_the_starts_own_server_run_in_the_forks_fold_body() {
            let mut lines = opened(FORK_ID);
            lines.push(on_fork(turn_started()));
            lines.push(on_fork(agent_message("m1", "ALPHA, BETA")));
            lines.push(on_fork(completed("completed")));
            let launch = launcher(server(lines, WireOpts::default()));
            let events = Events::default();
            let result = adapter_over(&launch)
                .start(HarnessStartOptions {
                    prompt: "list every code word you remember".into(),
                    fork: Some((THREAD_ID.into(), "t8".into())),
                    cwd: Some("/root/app-fork".into()),
                    model: Some("gpt-5.5".into()),
                    permission_mode: Some("workspace-write".into()),
                    on_event: Some(events.on_event()),
                    ..Default::default()
                })
                .unwrap()
                .finished()
                .await;
            assert_eq!(result.status, TurnStatus::Completed);
            assert_eq!(result.text.as_deref(), Some("ALPHA, BETA"));
            assert_eq!(launch.calls().len(), 1);
            let call = launch.call(0);
            assert_eq!(call.command, aside_command("/root/app-fork"));
            let fork = codex_wire::parse(call.input.as_ref().unwrap().last().unwrap());
            assert_eq!(
                fork,
                json!({"id": "wsp-thread", "method": "thread/fork", "params": {"threadId": THREAD_ID, "lastTurnId": "t8", "ephemeral": false, "excludeTurns": true, "cwd": "/root/app-fork", "model": "gpt-5.5", "sandbox": "workspace-write", "approvalPolicy": "on-request"}})
            );
            let w = launch.wire(0);
            assert!(!w.wrote("thread/start") && !w.wrote("thread/resume"));
            let turn = find_method(&w, "turn/start");
            assert_eq!(turn["params"]["threadId"], json!(FORK_ID));
            assert_eq!(turn["params"]["input"], json!([{"type": "text", "text": "list every code word you remember"}]));
            // The thread every later turn resumes is the fork's own, as the server answered it.
            assert_eq!(first_session_start(&events).1, FORK_ID);
            let both = adapter_over(&launch).start(HarnessStartOptions {
                prompt: "x".into(),
                fork: Some((THREAD_ID.into(), "t8".into())),
                resume: Some(FORK_ID.into()),
                on_event: Some(events.on_event()),
                ..Default::default()
            });
            let err = both.err().expect("a fork with a resume started");
            assert!(err.0.contains("never both"), "{err}");
        }

        #[tokio::test]
        async fn fails_the_turn_in_the_servers_words_where_it_will_not_fork_naming_nothing_it_nev() {
            tokio::task::LocalSet::new().run_until(fails_the_turn_in_the_servers_words_where_it_will_not_fork_naming_nothing_it_nev_body()).await
        }

        async fn fails_the_turn_in_the_servers_words_where_it_will_not_fork_naming_nothing_it_nev_body() {
            let refused = answering(|m, w| {
                if method(m) == "thread/fork" {
                    w.push1(json!({"id":"wsp-thread","error":{"code":-32600,"message":"turn not found: t8"}}).to_string());
                }
            });
            let result = adapter_over(&refused)
                .start(HarnessStartOptions { prompt: "x".into(), fork: Some((THREAD_ID.into(), "t8".into())), on_event: Some(no_events()), ..Default::default() })
                .unwrap()
                .finished()
                .await;
            assert_eq!(result.status, TurnStatus::Failed);
            assert_eq!(result.error.as_deref(), Some("codex could not open the thread: turn not found: t8"));
            assert!(!refused.wire(0).wrote("turn/start"));
        }

        #[tokio::test]
        async fn finds_a_turn_by_its_id_among_the_threads_turns_on_a_run_that_loads_no_thread_acr() {
            tokio::task::LocalSet::new().run_until(finds_a_turn_by_its_id_among_the_threads_turns_on_a_run_that_loads_no_thread_acr_body()).await
        }

        async fn finds_a_turn_by_its_id_among_the_threads_turns_on_a_run_that_loads_no_thread_acr_body() {
            let launch = history(History { turns: NEWEST_FIRST.to_vec(), page_size: Some(2), ..Default::default() });
            let found = adapter_over(&launch).fork_session(THREAD_ID, ForkTurn::Anchor("t6".into())).await.unwrap();
            assert_eq!(found, (THREAD_ID.to_string(), "t6".to_string()));
            let cursors: Vec<Value> = listed(&launch).into_iter().map(|p| p["cursor"].clone()).collect();
            assert_eq!(cursors, vec![json!(null), json!("2")]);
            let w = launch.wire(0);
            for name in ["thread/resume", "thread/start", "thread/fork", "turn/start"] {
                assert!(!w.wrote(name), "{name} was written");
            }
            assert!(w.closed());
        }

        #[tokio::test]
        async fn finds_a_turn_that_named_no_anchor_by_counting_back_past_the_turns_after_it_that() {
            tokio::task::LocalSet::new().run_until(finds_a_turn_that_named_no_anchor_by_counting_back_past_the_turns_after_it_that_body()).await
        }

        async fn finds_a_turn_that_named_no_anchor_by_counting_back_past_the_turns_after_it_that_body() {
            let launch = history(History { turns: NEWEST_FIRST.to_vec(), ..Default::default() });
            let refused = refused_for_sign_in();
            let found = adapter_over(&launch)
                .fork_session(THREAD_ID, ForkTurn::After(vec![done("a"), CountedTurn { anchor: None, result: Some(refused) }, done("b")]))
                .await
                .unwrap();
            assert_eq!(found, (THREAD_ID.to_string(), "t7".to_string()));
            let again = history(History { turns: NEWEST_FIRST.to_vec(), ..Default::default() });
            let found = adapter_over(&again).fork_session(THREAD_ID, ForkTurn::After(vec![])).await.unwrap();
            assert_eq!(found, (THREAD_ID.to_string(), "t9".to_string()));
        }

        #[tokio::test]
        async fn refuses_a_turn_the_server_does_not_list_and_a_count_past_the_turns_it_lists_befo() {
            tokio::task::LocalSet::new().run_until(refuses_a_turn_the_server_does_not_list_and_a_count_past_the_turns_it_lists_befo_body()).await
        }

        async fn refuses_a_turn_the_server_does_not_list_and_a_count_past_the_turns_it_lists_befo_body() {
            let listing = history(History { turns: NEWEST_FIRST.to_vec(), page_size: Some(2), ..Default::default() });
            let err = adapter_over(&listing).fork_session(THREAD_ID, ForkTurn::Anchor("t1".into())).await.unwrap_err();
            assert!(err.contains(&codex_no_turn_line("t1")), "{err}");
            let short = history(History { turns: vec!["t2", "t1"], ..Default::default() });
            let err = adapter_over(&short).fork_session(THREAD_ID, ForkTurn::After(vec![done("a"), done("b")])).await.unwrap_err();
            assert!(err.contains(CODEX_FEWER_TURNS), "{err}");
        }

        #[tokio::test]
        async fn forks_a_thread_an_older_codex_made_at_its_anchor_as_given_where_the_server_will() {
            tokio::task::LocalSet::new().run_until(forks_a_thread_an_older_codex_made_at_its_anchor_as_given_where_the_server_will_body()).await
        }

        async fn forks_a_thread_an_older_codex_made_at_its_anchor_as_given_where_the_server_will_body() {
            let legacy = answering(|m, w| {
                if method(m) == "thread/turns/list" {
                    w.push1(json!({"id": m["id"], "error": {"code": -32600, "message": "thread/turns/list only supports paginated threads"}}).to_string());
                }
            });
            let found = adapter_over(&legacy).fork_session(THREAD_ID, ForkTurn::Anchor(TURN_ID.into())).await.unwrap();
            assert_eq!(found, (THREAD_ID.to_string(), TURN_ID.to_string()));
        }
    }
}

mod a_codex_thread_picked_up_from_outside_wsp {
    use super::*;

    const COPY_ID: &str = "01a12814-3d6c-7751-813b-3b1c4e6f33da";
    const FORK_ID: &str = "01a12812-adb3-7c12-9acc-a86fcbe74924";

    fn unsubscribed() -> String {
        json!({"id":"wsp-unsubscribe","result":{"status":"unsubscribed"}}).to_string()
    }

    fn thread_ids(w: &Wire, from: usize) -> Vec<(String, Value)> {
        w.written().into_iter().skip(from).map(|m| (method(&m).to_string(), m["params"]["threadId"].clone())).collect()
    }

    #[tokio::test]
    async fn runs_a_copy_on_thread_fork_written_to_disk_as_a_thread_of_its_own_and_turns_on_t() {
        tokio::task::LocalSet::new().run_until(runs_a_copy_on_thread_fork_written_to_disk_as_a_thread_of_its_own_and_turns_on_t_body()).await
    }

    async fn runs_a_copy_on_thread_fork_written_to_disk_as_a_thread_of_its_own_and_turns_on_t_body() {
        let launch = answering(|m, w| {
            if method(m) == "thread/fork" {
                w.push(&opened(COPY_ID));
            }
            if method(m) == "turn/start" {
                w.push(&[turn_started().replace(THREAD_ID, COPY_ID), completed("completed").replace(THREAD_ID, COPY_ID)]);
            }
        });
        let events = Events::default();
        let result = adapter_over(&launch)
            .start(HarnessStartOptions {
                prompt: "on the copy".into(),
                resume: Some(THREAD_ID.into()),
                copy: true,
                cwd: Some("/work/acme/lab".into()),
                on_event: Some(events.on_event()),
                ..Default::default()
            })
            .unwrap()
            .finished()
            .await;
        assert_eq!(result.status, TurnStatus::Completed);
        let w = launch.wire(0);
        let fork = find_written(&w, "wsp-thread").expect("no thread request");
        assert_eq!(method(&fork), "thread/fork");
        assert_eq!(fork["params"]["threadId"], json!(THREAD_ID));
        assert_eq!(fork["params"]["ephemeral"], json!(false));
        assert_eq!(fork["params"]["excludeTurns"], json!(true));
        assert_eq!(fork["params"]["cwd"], json!("/work/acme/lab"));
        assert!(!w.wrote("thread/resume"));
        assert_eq!(find_method(&w, "turn/start")["params"]["threadId"], json!(COPY_ID));
        assert_eq!(first_session_start(&events).1, COPY_ID);
    }

    #[tokio::test]
    async fn fails_a_turn_whose_thread_another_process_writes_in_two_halves_and_sends_no_turn() {
        tokio::task::LocalSet::new().run_until(fails_a_turn_whose_thread_another_process_writes_in_two_halves_and_sends_no_turn_body()).await
    }

    async fn fails_a_turn_whose_thread_another_process_writes_in_two_halves_and_sends_no_turn_body() {
        let launch = answering(|m, w| {
            if method(m) == "thread/resume" {
                w.push1(json!({"error":{"code":-32600,"message":format!("thread {THREAD_ID} already has an active writer")},"id":"wsp-thread"}).to_string());
            }
        });
        let result = adapter_over(&launch)
            .start(HarnessStartOptions { prompt: "x".into(), resume: Some(THREAD_ID.into()), on_event: Some(no_events()), ..Default::default() })
            .unwrap()
            .finished()
            .await;
        assert_eq!(result, TurnResult { error: Some(conversation_written_elsewhere_line("Codex", Some(LETS_GO))), ..result_of(TurnStatus::Failed) });
        assert_eq!(
            result.error.as_deref(),
            Some("Codex is writing this conversation in another app, so this turn did not run. Close it there and send again. Codex lets go about a minute after its window closes.")
        );
        assert!(!launch.wire(0).wrote("turn/start"));
    }

    #[tokio::test]
    async fn lets_go_of_the_thread_when_a_kept_servers_turn_ends_and_opens_it_again_on_that_s() {
        tokio::task::LocalSet::new().run_until(lets_go_of_the_thread_when_a_kept_servers_turn_ends_and_opens_it_again_on_that_s_body()).await
    }

    async fn lets_go_of_the_thread_when_a_kept_servers_turn_ends_and_opens_it_again_on_that_s_body() {
        let launch = answering(|m, w| {
            if method(m) == "thread/start" {
                w.push(&opened(THREAD_ID));
            }
            if method(m) == "thread/resume" {
                w.push1(opened(THREAD_ID)[1].clone());
            }
            if method(m) == "turn/start" {
                w.push(&[turn_started(), completed("completed")]);
            }
            if method(m) == "thread/unsubscribe" {
                w.push1(unsubscribed());
            }
        });
        let first = adapter_over(&launch).start(HarnessStartOptions { prompt: "one".into(), keep: true, on_event: Some(no_events()), ..Default::default() }).unwrap();
        assert_eq!(first.finished().await.status, TurnStatus::Completed);
        let w = launch.wire(0);
        until(|| w.wrote("thread/unsubscribe")).await;
        assert_eq!(find_method(&w, "thread/unsubscribe"), json!({"id": "wsp-unsubscribe", "method": "thread/unsubscribe", "params": {"threadId": THREAD_ID}}));
        assert!(!w.closed());
        let kept = first.kept().expect("no kept server");
        let before = w.written().len();
        let second = (kept.next)(KeptTurn { prompt: "two".into(), images: vec![], after: None, on_event: no_events() }).unwrap();
        assert_eq!(second.finished().await.status, TurnStatus::Completed);
        let after = thread_ids(&w, before);
        assert_eq!(after[..2], [("thread/resume".to_string(), json!(THREAD_ID)), ("turn/start".to_string(), json!(THREAD_ID))]);
        assert_eq!(launch.calls().len(), 1);
    }

    #[tokio::test]
    async fn lets_go_of_a_forks_own_thread_when_a_kept_servers_turn_ends_and_opens_that_threa() {
        tokio::task::LocalSet::new().run_until(lets_go_of_a_forks_own_thread_when_a_kept_servers_turn_ends_and_opens_that_threa_body()).await
    }

    async fn lets_go_of_a_forks_own_thread_when_a_kept_servers_turn_ends_and_opens_that_threa_body() {
        let launch = answering(|m, w| {
            if method(m) == "thread/fork" {
                w.push(&opened(FORK_ID));
            }
            if method(m) == "thread/resume" {
                w.push1(opened(FORK_ID)[1].clone());
            }
            if method(m) == "turn/start" {
                w.push(&[turn_started().replace(THREAD_ID, FORK_ID), completed("completed").replace(THREAD_ID, FORK_ID)]);
            }
            if method(m) == "thread/unsubscribe" {
                w.push1(unsubscribed());
            }
        });
        let first = adapter_over(&launch)
            .start(HarnessStartOptions { prompt: "one".into(), fork: Some((THREAD_ID.into(), "t8".into())), keep: true, on_event: Some(no_events()), ..Default::default() })
            .unwrap();
        assert_eq!(first.finished().await.status, TurnStatus::Completed);
        let w = launch.wire(0);
        until(|| w.wrote("thread/unsubscribe")).await;
        assert_eq!(find_method(&w, "thread/unsubscribe"), json!({"id": "wsp-unsubscribe", "method": "thread/unsubscribe", "params": {"threadId": FORK_ID}}));
        let before = w.written().len();
        let second = (first.kept().expect("no kept server").next)(KeptTurn { prompt: "two".into(), images: vec![], after: None, on_event: no_events() }).unwrap();
        assert_eq!(second.finished().await.status, TurnStatus::Completed);
        let after = thread_ids(&w, before);
        assert_eq!(after[..2], [("thread/resume".to_string(), json!(FORK_ID)), ("turn/start".to_string(), json!(FORK_ID))]);
        assert_eq!(launch.calls().len(), 1);
    }
}

mod a_codex_turn_that_does_not_complete {
    use super::*;

    #[tokio::test]
    async fn a_resume_whose_rollout_is_gone_starts_a_new_thread_handed_the_thread_so_far_as_c() {
        tokio::task::LocalSet::new().run_until(a_resume_whose_rollout_is_gone_starts_a_new_thread_handed_the_thread_so_far_as_c_body()).await
    }

    async fn a_resume_whose_rollout_is_gone_starts_a_new_thread_handed_the_thread_so_far_as_c_body() {
        const FRESH: &str = "01a0e2c1-5d10-7b42-9a6e-0000000000f5";
        // The first server has no rollout for the thread, as 0.162.1 answers one; the second opens a new thread and answers.
        let launch = answering(|m, w| {
            if method(m) == "thread/resume" {
                w.push(&[opened(THREAD_ID)[0].clone(), json!({"error":{"code":-32600,"message":format!("no rollout found for thread id {THREAD_ID}")},"id":"wsp-thread"}).to_string()]);
            }
            if method(m) == "thread/start" {
                w.push(&opened(FRESH));
            }
            if method(m) == "turn/start" {
                w.push(&[
                    json!({"method":"turn/started","params":{"threadId":FRESH,"turn":{"id":TURN_ID,"items":[],"status":"inProgress"}}}).to_string(),
                    json!({"method":"item/completed","params":{"item":{"type":"agentMessage","id":"m1","text":"lantern"},"threadId":FRESH,"turnId":TURN_ID}}).to_string(),
                    json!({"method":"turn/completed","params":{"threadId":FRESH,"turn":{"id":TURN_ID,"items":[],"status":"completed"}}}).to_string(),
                ]);
            }
        });
        let events = Events::default();
        let seed = "person: remember the word lantern\nagent: remembered";
        let handed = seed.to_string();
        let session = adapter_over(&launch)
            .start(HarnessStartOptions {
                prompt: "what was the word?".into(),
                resume: Some(THREAD_ID.into()),
                seed: Some(Arc::new(move || {
                    let seed = handed.clone();
                    async move { Ok(seed) }.boxed_local()
                })),
                on_event: Some(events.on_event()),
                ..Default::default()
            })
            .unwrap();
        let result = session.finished().await;
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(result.text.as_deref(), Some("lantern"));
        assert_eq!(launch.wire_count(), 2);
        let second = launch.wire(1);
        assert!(!second.wrote("thread/resume"));
        let turn = find_method(&second, "turn/start");
        let input = turn["params"]["input"].as_array().unwrap();
        assert_eq!(input.last().unwrap()["text"], json!(lost_session_prompt(seed, "what was the word?")));
        // The failed resume is no turn of the thread's: one turn ends, one session ends, the new thread's.
        assert_eq!(events.of("turn.done").len(), 1);
        assert_eq!(events.of("session.end").len(), 1);
        let (started, id) = first_session_start(&events);
        assert_eq!(id, FRESH);
        assert_eq!(session.thread_id(), FRESH);
        let note = events.all().into_iter().skip(started).find_map(|e| match e {
            AdapterEvent::TurnDelta(d) if d.kind == DeltaKind::Note => Some(d),
            _ => None,
        });
        assert_eq!(note.expect("no note delta").text, LOST_SESSION_NOTE);
    }
}

mod a_probe_on_a_machine_whose_chatgpt_sign_in_the_host_holds {
    use super::*;

    const SEP: &str = "__WSP_CATALOG_SEP__";

    fn real() -> String {
        codex_wire::fixture_text("codex", "catalog-probe.txt")
    }

    fn claims() -> CodexClaims {
        CodexClaims { account_id: "11111111-2222-4333-8444-555555555555".into(), plan_type: Some("plus".into()), user_id: None, fedramp: None }
    }

    fn levels(efforts: &[&str]) -> Value {
        Value::Array(efforts.iter().map(|e| json!({"effort": e, "description": format!("{e} effort")})).collect())
    }

    // ChatGPT's /backend-api/codex/models answer for codex-cli 0.155.1 on 2026-10-16, cut to the fields the picker reads,
    // out of codex's order: it sorts by priority, shows only the rows marked list, and makes the first shown the default.
    fn account() -> Value {
        json!({"models": [
            {"slug": "gpt-6-luna", "display_name": "GPT-6-Luna", "visibility": "list", "priority": 4, "default_reasoning_level": "medium", "supported_reasoning_levels": levels(&["low", "medium", "high"]), "additional_speed_tiers": ["fast"]},
            {"slug": "gpt-reserve", "display_name": "GPT-Reserve", "visibility": "hide", "priority": 4, "supported_reasoning_levels": levels(&["low"])},
            {"slug": "gpt-6-astra", "display_name": "GPT-6-Astra", "description": "Frontier model", "visibility": "list", "priority": 2, "default_reasoning_level": "low", "supported_reasoning_levels": levels(&["low", "medium", "high", "xhigh", "max", "ultra"]), "additional_speed_tiers": ["fast"]},
            {"slug": "gpt-6-sol", "display_name": "GPT-6-Sol", "visibility": "list", "priority": 3, "default_reasoning_level": "medium", "supported_reasoning_levels": levels(&["low", "medium", "high", "xhigh"]), "additional_speed_tiers": []},
        ]})
    }

    fn chatgpt(models: Option<AccountModels>) -> CodexBroker {
        CodexBroker { mode: BrokerMode::Chatgpt { claims: claims(), models }, providers: None }
    }

    fn reading(answer: Result<Option<Value>, String>) -> AccountModels {
        Arc::new(move |_version: String| {
            let answer = answer.clone();
            async move { answer }.boxed_local()
        })
    }

    fn adapter_with(broker: Option<CodexBroker>) -> CodexAdapter {
        let launch = launcher(scripted_server(WireOpts::default()));
        create_codex_adapter(CodexAdapterDeps { broker, ..CodexAdapterDeps::new(launch.factory(), "/root/.codex", LOGIN) }).unwrap()
    }

    fn printing(stdout: String) -> Arc<dyn HarnessExec> {
        Arc::new(move |_command: &str, _env: Option<&Env>| {
            let stdout = stdout.clone();
            async move { Ok::<String, ExecFailure>(stdout) }.boxed_local()
        })
    }

    async fn probe_on(broker: Option<CodexBroker>) -> Option<HarnessCatalogAnswer> {
        adapter_with(broker).probe_catalog(printing(real())).await.unwrap()
    }

    fn lists_of(answer: Option<HarnessCatalogAnswer>) -> HarnessCatalogProbe {
        match answer {
            Some(HarnessCatalogAnswer::Probe(p)) => p,
            other => panic!("expected lists, got {other:?}"),
        }
    }

    fn slugs(probe: &HarnessCatalogProbe) -> Vec<String> {
        probe.models.iter().map(|m| m.slug.clone()).collect()
    }

    fn own_slugs() -> Vec<String> {
        slugs(&lists_of(parse_catalog_probe(&real(), LOGIN, None)))
    }

    #[tokio::test]
    async fn lists_the_accounts_own_models_read_through_the_host_for_the_binarys_version_in_c() {
        tokio::task::LocalSet::new().run_until(lists_the_accounts_own_models_read_through_the_host_for_the_binarys_version_in_c_body()).await
    }

    async fn lists_the_accounts_own_models_read_through_the_host_for_the_binarys_version_in_c_body() {
        let asked = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen = asked.clone();
        let models: AccountModels = Arc::new(move |version: String| {
            seen.lock().unwrap().push(version);
            async { Ok(Some(account())) }.boxed_local()
        });
        let probe = lists_of(probe_on(Some(chatgpt(Some(models)))).await);
        assert_eq!(*asked.lock().unwrap(), vec!["0.153.0".to_string()]);
        assert_eq!(slugs(&probe), ["gpt-6-astra", "gpt-6-sol", "gpt-6-luna"]);
        let defaults: Vec<&str> = probe.models.iter().filter(|m| m.is_default).map(|m| m.slug.as_str()).collect();
        assert_eq!(defaults, ["gpt-6-astra"]);
        assert_eq!(probe.models[0].label, "GPT-6-Astra");
        assert_eq!(probe.models[0].description.as_deref(), Some("Frontier model"));
        assert_eq!(probe.models[0].default_effort.as_deref(), Some("low"));
        assert_eq!(probe.models[0].fast, Some(true));
        assert_eq!(probe.models[1].slug, "gpt-6-sol");
        assert_eq!(probe.models[1].efforts, Some(["low", "medium", "high", "xhigh"].map(String::from).to_vec()));
        assert_eq!(probe.models[1].default_effort.as_deref(), Some("medium"));
        assert_eq!(probe.models[1].fast, None);
        assert_eq!(probe.efforts, ["low", "medium", "high", "xhigh", "max", "ultra"]);
        assert_eq!(probe.version.as_deref(), Some("0.153.0"));
    }

    #[tokio::test]
    async fn keeps_the_binarys_own_list_where_the_hosts_read_fails_answers_nothing_or_holds_n() {
        tokio::task::LocalSet::new().run_until(keeps_the_binarys_own_list_where_the_hosts_read_fails_answers_nothing_or_holds_n_body()).await
    }

    async fn keeps_the_binarys_own_list_where_the_hosts_read_fails_answers_nothing_or_holds_n_body() {
        let reads: Vec<Result<Option<Value>, String>> = vec![
            Err("chatgpt.com unreachable".into()),
            Ok(None),
            Ok(Some(json!({"models": []}))),
            Ok(Some(json!({"models": [{"slug": "gpt-reserve", "visibility": "hide", "priority": 1}]}))),
            Ok(Some(json!({"detail": "Unauthorized"}))),
        ];
        let mut got = Vec::new();
        for read in reads {
            got.push(slugs(&lists_of(probe_on(Some(chatgpt(Some(reading(read))))).await)));
        }
        let own = own_slugs();
        for slugs in got {
            assert_eq!(slugs, own);
        }
    }

    #[tokio::test]
    async fn asks_the_host_nothing_on_an_api_key_with_no_read_handed_or_where_the_binary_prin() {
        tokio::task::LocalSet::new().run_until(asks_the_host_nothing_on_an_api_key_with_no_read_handed_or_where_the_binary_prin_body()).await
    }

    async fn asks_the_host_nothing_on_an_api_key_with_no_read_handed_or_where_the_binary_prin_body() {
        let on_key = slugs(&lists_of(probe_on(Some(CodexBroker { mode: BrokerMode::Apikey, providers: None })).await));
        let no_read = slugs(&lists_of(probe_on(Some(chatgpt(None))).await));
        let own = own_slugs();
        assert_eq!(on_key, own);
        assert_eq!(no_read, own);
        let asked = Arc::new(Mutex::new(0u32));
        let counted = asked.clone();
        let models: AccountModels = Arc::new(move |_version: String| {
            *counted.lock().unwrap() += 1;
            async { Ok(Some(account())) }.boxed_local()
        });
        let adapter = adapter_with(Some(chatgpt(Some(models))));
        let server_lines: Vec<String> = real().split(SEP).nth(2).unwrap().split('\n').filter(|l| l.starts_with('{')).map(String::from).collect();
        let no_version = format!("{SEP}{SEP}\n{}\n", server_lines.join("\n"));
        adapter.probe_catalog(printing(no_version)).await.unwrap();
        assert_eq!(*asked.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn offers_the_accounts_models_without_the_binary_read_for_wsps_npm_pin_only_where_t() {
        tokio::task::LocalSet::new().run_until(offers_the_accounts_models_without_the_binary_read_for_wsps_npm_pin_only_where_t_body()).await
    }

    async fn offers_the_accounts_models_without_the_binary_read_for_wsps_npm_pin_only_where_t_body() {
        let asked = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen = asked.clone();
        let models: AccountModels = Arc::new(move |version: String| {
            seen.lock().unwrap().push(version);
            async { Ok(Some(account())) }.boxed_local()
        });
        let held = adapter_with(Some(chatgpt(Some(models))));
        let offered = held.account_models(None).expect("no account models").await.expect("no models");
        assert_eq!(offered.iter().map(|m| m.slug.as_str()).collect::<Vec<_>>(), ["gpt-6-astra", "gpt-6-sol", "gpt-6-luna"]);
        assert_eq!(*asked.lock().unwrap(), vec![CODEX_PINNED_VERSION.to_string()]);
        let unreachable = adapter_with(Some(chatgpt(Some(reading(Err("chatgpt.com unreachable".into()))))));
        assert!(unreachable.account_models(None).expect("no account models").await.is_none());
        for broker in [None, Some(CodexBroker { mode: BrokerMode::Apikey, providers: None }), Some(chatgpt(None))] {
            assert!(adapter_with(broker).account_models(None).is_none());
        }
    }
}

mod a_brokered_codex_launch_adapter_cases {
    use super::*;

    const TOKEN: &str = "AbC-123_xyz0AbC-123_xyz0AbC-123_x";
    const HOST_TOKEN_ENV: &str = "WSP_HOST_TOKEN";

    fn claims() -> CodexClaims {
        CodexClaims { account_id: "11111111-2222-4333-8444-555555555555".into(), plan_type: Some("plus".into()), user_id: Some("user-acme0001".into()), fedramp: Some(false) }
    }

    fn chatgpt() -> CodexBroker {
        CodexBroker { mode: BrokerMode::Chatgpt { claims: claims(), models: None }, providers: None }
    }

    fn apikey() -> CodexBroker {
        CodexBroker { mode: BrokerMode::Apikey, providers: None }
    }

    fn base_of(env: &[(&str, &str)]) -> IndexMap<String, Option<String>> {
        env.iter().map(|(k, v)| (k.to_string(), Some(v.to_string()))).collect()
    }

    /// A folder holding the stand-in for codex, which keeps its words and its environment there.
    fn stand_in() -> (TempDir, String) {
        let dir = TempDir::new("codex-broker-");
        let d = dir.s();
        let program = write_stub(&dir.join("codex"), &format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > \"{d}/args\"\nenv > \"{d}/env\"\n"));
        (dir, program)
    }

    /// A line run in bash under build_env's environment, as the machine runs it. A key turn is handed the placeholder
    /// key as the runtime shapes it from the turn token.
    fn run(dir: &TempDir, line: &str, broker: &CodexBroker, env: &[(&str, &str)]) -> i32 {
        let keyed = matches!(broker.mode, BrokerMode::Apikey);
        let built = build_env(&CodexEnvOptions {
            base: Some(base_of(env)),
            home: dir.join("home").to_string_lossy().into_owned(),
            api_key: keyed.then(|| format!("sk-wsp-{TOKEN}")),
            broker: Some(broker.clone()),
        })
        .unwrap();
        let mut shell = vec![("PATH".to_string(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()))];
        shell.extend(built);
        run_shell("bash", line, &shell, "").status
    }

    fn args_of(dir: &TempDir) -> Vec<String> {
        std::fs::read_to_string(dir.join("args")).unwrap().trim().split('\n').map(String::from).collect()
    }

    fn env_of(dir: &TempDir) -> BTreeMap<String, String> {
        std::fs::read_to_string(dir.join("env"))
            .unwrap()
            .trim()
            .split('\n')
            .map(|line| {
                let at = line.find('=').unwrap();
                (line[..at].to_string(), line[at + 1..].to_string())
            })
            .collect()
    }

    /// A side run: the adapter's revert or its fork lookup, over a server that has already exited.
    async fn side_run(adapter: &CodexAdapter, which: usize) {
        let work = async {
            match which {
                0 => adapter.revert(RevertAsk { session: "t1".into(), cwd: None, at: RevertAt::BeforeTurn("turn-1".into()) }).await.map(|_| ()),
                _ => adapter.fork_session("t1", ForkTurn::Anchor("turn-1".into())).await.map(|_| ()),
            }
        };
        let _ = tokio::time::timeout(Duration::from_secs(5), work).await;
    }

    /// The exec of the TS case throws on launch; this one records the command and hands back a server that is gone.
    fn gone_launcher() -> Launch {
        launcher(|seed: &[String]| {
            let w = wire(WireOpts::default());
            w.seed(seed);
            w.exit(Some(0));
            w
        })
    }

    #[tokio::test]
    async fn a_revert_or_a_forks_turn_lookup_reads_no_login_not_even_the_one_the_last_turn_le() {
        tokio::task::LocalSet::new().run_until(a_revert_or_a_forks_turn_lookup_reads_no_login_not_even_the_one_the_last_turn_le_body()).await
    }

    async fn a_revert_or_a_forks_turn_lookup_reads_no_login_not_even_the_one_the_last_turn_le_body() {
        // Neither asks a model, and the runtime hands a revert no turn token, so on the last turn's dead login codex's
        // startup calls reached the door with no thread.
        let token_env = [(HOST_TOKEN_ENV, TOKEN)];
        let cases: Vec<(CodexBroker, Vec<(&str, &str)>)> = vec![(chatgpt(), vec![]), (chatgpt(), token_env.to_vec()), (apikey(), vec![])];
        for (broker, env) in cases {
            for which in 0..2 {
                let (dir, program) = stand_in();
                let launch = gone_launcher();
                let adapter = create_codex_adapter(CodexAdapterDeps {
                    home: dir.join("home").to_string_lossy().into_owned(),
                    login: "codex login".into(),
                    broker: Some(broker.clone()),
                    base_env: Some(base_of(&env)),
                    launch: Some(AgentLaunch { program: Some(program), args: None }),
                    ..CodexAdapterDeps::new(launch.factory(), "/unused", "codex login")
                })
                .unwrap();
                side_run(&adapter, which).await;
                let command = launch.call(0).command;
                std::fs::create_dir(dir.join("home")).unwrap();
                std::fs::write(dir.join("home").join("auth.json"), "last turn's").unwrap();
                assert_eq!(run(&dir, &command, &broker, &env), 0);
                let args = args_of(&dir);
                assert!(args.contains(&"app-server".to_string()), "{args:?}");
                assert!(args.contains(&r#"cli_auth_credentials_store="ephemeral""#.to_string()), "{args:?}");
                assert!(!args.contains(&r#"cli_auth_credentials_store="file""#.to_string()), "{args:?}");
                assert!(args.contains(&"features.plugins=false".to_string()), "{args:?}");
                assert_eq!(std::fs::read_to_string(dir.join("home").join("auth.json")).unwrap(), "last turn's");
                assert_eq!(env_of(&dir).get(KEY_FILE_ENV), None);
            }
        }
    }

    #[tokio::test]
    async fn a_revert_or_a_forks_turn_lookup_names_no_model_address_at_the_door_its_providers() {
        tokio::task::LocalSet::new().run_until(a_revert_or_a_forks_turn_lookup_names_no_model_address_at_the_door_its_providers_body()).await
    }

    async fn a_revert_or_a_forks_turn_lookup_names_no_model_address_at_the_door_its_providers_body() {
        // A thread/resume starts a session, and codex opens the provider's Responses websocket at every session start,
        // login or not: a revert's two reached the door with no token.
        let acme: Map<String, Value> = match json!({"base_url": "https://gw.acme.dev/v1", "supports_websockets": true, "experimental_bearer_token": "acme-real"}) {
            Value::Object(m) => m,
            _ => unreachable!(),
        };
        let providers: IndexMap<String, Map<String, Value>> = [("acme".to_string(), acme)].into_iter().collect();
        let brokers = vec![
            CodexBroker { providers: Some(providers.clone()), ..chatgpt() },
            CodexBroker { providers: Some(providers), ..apikey() },
        ];
        let address = Regex::new(r"^(openai_base_url|model_providers\.acme\.base_url)=").unwrap();
        for broker in brokers {
            for which in 0..2 {
                let (dir, program) = stand_in();
                let launch = gone_launcher();
                let adapter = create_codex_adapter(CodexAdapterDeps {
                    home: dir.join("home").to_string_lossy().into_owned(),
                    login: "codex login".into(),
                    broker: Some(broker.clone()),
                    launch: Some(AgentLaunch { program: Some(program), args: None }),
                    ..CodexAdapterDeps::new(launch.factory(), "/unused", "codex login")
                })
                .unwrap();
                side_run(&adapter, which).await;
                let command = launch.call(0).command;
                assert_eq!(run(&dir, &command, &broker, &[]), 0);
                let models: Vec<String> = args_of(&dir).into_iter().filter(|arg| address.is_match(arg)).collect();
                assert_eq!(models.len(), 2, "{models:?}");
                for arg in models {
                    let value = &arg[arg.find('=').unwrap() + 1..];
                    assert!(value.starts_with("\"http://127.0.0.1:0/"), "{arg}");
                }
            }
        }
    }

    #[tokio::test]
    async fn the_adapter_hands_a_title_its_brokered_word_and_the_turns_own_environment() {
        tokio::task::LocalSet::new().run_until(the_adapter_hands_a_title_its_brokered_word_and_the_turns_own_environment_body()).await
    }

    async fn the_adapter_hands_a_title_its_brokered_word_and_the_turns_own_environment_body() {
        let asked: Arc<Mutex<Vec<(String, Option<Env>)>>> = Arc::default();
        let launch = launcher(scripted_server(WireOpts::default()));
        let adapter = create_codex_adapter(CodexAdapterDeps {
            broker: Some(chatgpt()),
            base_env: Some(base_of(&[(HOST_TOKEN_ENV, TOKEN)])),
            ..CodexAdapterDeps::new(launch.factory(), "/root/.codex", "codex login")
        })
        .unwrap();
        let record = asked.clone();
        let exec: Arc<dyn HarnessExec> = Arc::new(move |command: &str, env: Option<&Env>| {
            record.lock().unwrap().push((command.to_string(), env.cloned()));
            async { Ok::<String, ExecFailure>(String::new()) }.boxed_local()
        });
        let _ = adapter.title_for(TitleTurn { opening: "hi".into(), reply: Some("hello".into()), model: None }, exec).await;
        let asked = asked.lock().unwrap();
        assert!(asked[0].0.contains("cli_auth_credentials_store="), "{}", asked[0].0);
        assert_eq!(asked[0].1.as_ref(), Some(&adapter.env()));
        let env = asked[0].1.as_ref().unwrap();
        assert_eq!(env.get("CODEX_REFRESH_TOKEN_URL_OVERRIDE"), Some(&format!("{DOOR}/oauth/token")));
    }
}

mod the_title_codex_makes_for_a_thread {
    use super::*;

    const THREAD: &str = "01a079b6-6f04-7f73-84d6-40e9e6885ffd";

    #[tokio::test]
    async fn runs_under_the_sessions_own_codex_home_which_a_guest_exec_would_not_carry_read_o() {
        tokio::task::LocalSet::new().run_until(runs_under_the_sessions_own_codex_home_which_a_guest_exec_would_not_carry_read_o_body()).await
    }

    async fn runs_under_the_sessions_own_codex_home_which_a_guest_exec_would_not_carry_read_o_body() {
        let asked: Arc<Mutex<Vec<(String, Option<Env>)>>> = Arc::default();
        let launch = launcher(scripted_server(WireOpts::default()));
        let base: IndexMap<String, Option<String>> = [("PATH", "/bin"), ("GATEWAY_TOKEN", "tok-x")].into_iter().map(|(k, v)| (k.to_string(), Some(v.to_string()))).collect();
        let codex = create_codex_adapter(CodexAdapterDeps { base_env: Some(base), ..CodexAdapterDeps::new(launch.factory(), "/root/it's here", "codex login") }).unwrap();
        let record = asked.clone();
        let exec: Arc<dyn HarnessExec> = Arc::new(move |command: &str, env: Option<&Env>| {
            record.lock().unwrap().push((command.to_string(), env.cloned()));
            async { Ok::<String, ExecFailure>(String::new()) }.boxed_local()
        });
        codex.probe_catalog(exec.clone()).await.unwrap();
        codex.title_for(TitleTurn { opening: "name it".into(), reply: None, model: None }, exec.clone()).await.unwrap();
        codex.draft_for(DraftAsk { prompt_file: "/tmp/asked".into(), model: None }, exec.clone()).await.unwrap();
        codex.rename_session(THREAD, "the name", exec).await.unwrap();
        let asked = asked.lock().unwrap();
        assert_eq!(asked.len(), 4);
        let secrets = Regex::new(r"it's here|tok-x|CODEX_HOME=").unwrap();
        for (cmd, env) in asked.iter() {
            let env = env.as_ref().expect("no env on the input");
            assert_eq!(env.get("CODEX_HOME").map(String::as_str), Some("/root/it's here"));
            assert_eq!(env.get("GATEWAY_TOKEN").map(String::as_str), Some("tok-x"));
            assert!(cmd.contains(ENV_FROM_INPUT), "{cmd}");
            assert!(!secrets.is_match(cmd), "{cmd}");
        }
    }
}

mod what_the_machine_holds_for_a_session_codex_would_not_resume {
    use super::*;

    const THREAD: &str = "01a1424a-7c2e-7d31-9f00-5b2a1c3d4e5f";
    const DAY: &str = "sessions/2026/10/16";
    const OPENING: &str = "Remember the word lantern.";

    #[tokio::test]
    async fn is_what_the_adapters_own_reading_answers_through_the_exec_it_is_handed() {
        tokio::task::LocalSet::new().run_until(is_what_the_adapters_own_reading_answers_through_the_exec_it_is_handed_body()).await
    }

    async fn is_what_the_adapters_own_reading_answers_through_the_exec_it_is_handed_body() {
        let named = format!("{DAY}/rollout-2026-10-16T01-19-52-{THREAD}.jsonl");
        let home = TempDir::new("wsp-codex-held-");
        let schema = "create table threads (id text primary key, rollout_path text not null, title text not null, history_mode text not null default 'legacy');";
        let row = format!("insert into threads (id, rollout_path, title, history_mode) values ('{THREAD}', '{}', '{OPENING}', 'paginated');", home.join(&named).display());
        sqlite(&home.join("state_5.sqlite"), &format!("{schema}\n{row}"));
        let launch = launcher(scripted_server(WireOpts::default()));
        let base: IndexMap<String, Option<String>> = [("PATH".to_string(), Some("/usr/bin:/bin".to_string()))].into_iter().collect();
        let codex = create_codex_adapter(CodexAdapterDeps { base_env: Some(base), ..CodexAdapterDeps::new(launch.factory(), home.s(), "codex login") }).unwrap();
        let exec: Arc<dyn HarnessExec> = Arc::new(|command: &str, _env: Option<&Env>| {
            let ran = run_shell("bash", command, &plain_env(), "");
            let out: LocalBoxFuture<'static, Result<String, ExecFailure>> = async move { Ok(ran.stdout) }.boxed_local();
            out
        });
        let line = codex.session_held(THREAD, exec).await.unwrap();
        let shape = Regex::new(r#"; rollout files: none; index row: state_5\.sqlite .*"mode":"paginated"\}, the file it names is missing$"#).unwrap();
        assert!(shape.is_match(&line), "{line}");
    }
}

/// Rules CA1 to CA6 of spec/codex_adapter.md, "Rules added for the port", each value printed by the TS at 65f6b89c7.
mod rules_added {
    use super::*;
    use wsp_adapters::codex::adapter::AttachedSession;
    use wsp_adapters::port::{AdapterAttachOptions, AnswerOutcome, PermissionAnswer, PermissionOutcome, SteerOutcome, TaskStop, TurnImage, PERMISSION_ALLOW};

    fn image(path: Option<&str>) -> TurnImage {
        TurnImage { media_type: "image/png".into(), bytes: String::new(), path: path.map(String::from) }
    }

    fn refusal(adapter: &CodexAdapter, o: HarnessStartOptions) -> String {
        match adapter.start(HarnessStartOptions { prompt: "go".into(), on_event: Some(no_events()), ..o }) {
            Ok(_) => panic!("the start was taken"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn a_start_refuses_in_one_order_before_anything_launches() {
        // CA1; adapter.ts:1262-1277: the context window, each image in order, the permission mode, then fork with resume.
        let launch = launcher(scripted(vec![], false));
        let a = adapter_over(&launch);
        let fork = Some(("s".to_string(), "t".to_string()));
        let all = HarnessStartOptions { context_window: Some(String::new()), images: vec![image(Some("rel")), image(None)], permission_mode: Some("bogus".into()), resume: Some(THREAD_ID.into()), fork: fork.clone(), ..Default::default() };
        assert_eq!(refusal(&a, all), "codex takes no context window");
        let bad_first = HarnessStartOptions { images: vec![image(Some("rel")), image(None)], permission_mode: Some("bogus".into()), resume: Some(THREAD_ID.into()), fork: fork.clone(), ..Default::default() };
        assert_eq!(refusal(&a, bad_first), r#"an image path must be one absolute path on the machine, got "rel""#);
        assert_eq!(refusal(&a, HarnessStartOptions { images: vec![image(None), image(Some("rel"))], ..Default::default() }), "codex reads images off the machine's disk; this one has no path on it");
        let mode = HarnessStartOptions { permission_mode: Some(String::new()), resume: Some(THREAD_ID.into()), fork: fork.clone(), ..Default::default() };
        assert_eq!(refusal(&a, mode), r#"permissionMode must be one of read-only, workspace-write, danger-full-access, got """#);
        assert_eq!(refusal(&a, HarnessStartOptions { resume: Some(THREAD_ID.into()), copy: true, fork, ..Default::default() }), "a codex start forks a thread or resumes one, never both");
        assert_eq!(launch.wire_count(), 0);
    }

    #[tokio::test]
    async fn an_empty_resume_is_a_resume_under_the_empty_id() {
        tokio::task::LocalSet::new().run_until(an_empty_resume_is_a_resume_under_the_empty_id_body()).await
    }

    async fn an_empty_resume_is_a_resume_under_the_empty_id_body() {
        // CA2; adapter.ts:1268 (`resume ?? randomUUID()`), no slug check.
        let launch = answering(|m, w| match method(m) {
            "thread/resume" => w.push(&opened(THREAD_ID)),
            "turn/start" => w.push(&[turn_started(), completed("completed")]),
            _ => {}
        });
        let a = adapter_over(&launch);
        let session = a.start(HarnessStartOptions { prompt: "go".into(), resume: Some(String::new()), on_event: Some(no_events()), ..Default::default() }).unwrap();
        let _ = session.finished().await;
        assert_eq!(session.local_id(), "");
        assert_eq!(find_method(&launch.wire(0), "thread/resume"), json!({"id":"wsp-thread","method":"thread/resume","params":{"threadId":"","excludeTurns":true,"sandbox":"danger-full-access","approvalPolicy":"never"}}));
        assert_eq!(a.sessions().keys().cloned().collect::<Vec<_>>(), vec![String::new()]);
    }

    #[tokio::test]
    async fn a_kept_server_s_next_turn_joins_the_registry_under_the_thread_id_beside_the_first() {
        tokio::task::LocalSet::new().run_until(a_kept_server_s_next_turn_joins_the_registry_under_the_thread_id_beside_the_first_body()).await
    }

    async fn a_kept_server_s_next_turn_joins_the_registry_under_the_thread_id_beside_the_first_body() {
        // CA3; adapter.ts:1229-1255 (`nextOn` follows with the thread id as its local id), 1225.
        let launch = answering(|m, w| match method(m) {
            "thread/start" | "thread/resume" => w.push(&opened(THREAD_ID)),
            "turn/start" => w.push(&[turn_started(), agent_message("m1", "ok"), completed("completed")]),
            _ => {}
        });
        let a = create_codex_adapter(CodexAdapterDeps { result_exit_ms: Some(10), interrupt_grace_ms: Some(10), ..CodexAdapterDeps::new(launch.factory(), "/root/.codex", LOGIN) }).unwrap();
        let first = a.start(HarnessStartOptions { prompt: "go".into(), keep: true, on_event: Some(no_events()), ..Default::default() }).unwrap();
        let _ = first.finished().await;
        let kept = first.kept().expect("the server is kept");
        let next = (kept.next)(KeptTurn { prompt: "again".into(), images: vec![], after: None, on_event: no_events() }).unwrap();
        let _ = next.finished().await;
        assert_eq!(a.sessions().keys().cloned().collect::<Vec<_>>(), vec![first.local_id(), THREAD_ID.to_string()]);
        assert_eq!(next.local_id(), THREAD_ID);
        assert_eq!(launch.wire_count(), 1);
    }

    #[tokio::test]
    async fn a_lost_session_whose_second_launch_is_refused_fails_in_its_words_under_the_new_launch_s_id() {
        tokio::task::LocalSet::new().run_until(a_lost_session_whose_second_launch_is_refused_fails_in_its_words_under_the_new_launch_s_id_body()).await
    }

    async fn a_lost_session_whose_second_launch_is_refused_fails_in_its_words_under_the_new_launch_s_id_body() {
        // CA4; lost-session.ts:66-96. Only the second launch's end reaches the caller; the registry holds the first
        // launch's local id (the resume) and the second launch's own fresh id.
        let launch = answering(|m, w| match method(m) {
            "thread/resume" => w.push(&[opened(THREAD_ID)[0].clone(), format!(r#"{{"error":{{"code":-32600,"message":"no rollout found for thread id {THREAD_ID}"}},"id":"wsp-thread"}}"#)]),
            "thread/start" => w.push(&[opened(THREAD_ID)[0].clone(), r#"{"error":{"code":-32600,"message":"boom on second"},"id":"wsp-thread"}"#.to_string()]),
            _ => {}
        });
        let a = adapter_over(&launch);
        let events = Events::default();
        let session = a
            .start(HarnessStartOptions {
                prompt: "what?".into(),
                resume: Some(THREAD_ID.into()),
                seed: Some(Arc::new(|| async { Ok("person: hi".to_string()) }.boxed_local())),
                on_event: Some(events.on_event()),
                ..Default::default()
            })
            .unwrap();
        let words = "codex could not open the thread: boom on second";
        assert_eq!(session.finished().await, TurnResult { error: Some(words.into()), ..result_of(TurnStatus::Failed) });
        assert_eq!(events.kinds(), ["turn.done", "session.end"]);
        let second = match &events.all()[1] {
            AdapterEvent::SessionEnd { session_id, exit_code: Some(0), saw_result: true, .. } => session_id.clone(),
            other => panic!("{other:?}"),
        };
        assert_ne!(second, THREAD_ID);
        assert_eq!(launch.wire_count(), 2);
        assert_eq!(session.local_id(), THREAD_ID);
        let keys: Vec<String> = a.sessions().keys().cloned().collect();
        assert_eq!(keys, vec![THREAD_ID.to_string(), second]);
    }

    #[tokio::test]
    async fn a_revert_by_count_pages_at_most_100_at_a_time_and_a_count_of_none_writes_nothing_past_the_resume() {
        tokio::task::LocalSet::new().run_until(a_revert_by_count_pages_at_most_100_at_a_time_and_a_count_of_none_writes_nothing_past_the_resume_body()).await
    }

    async fn a_revert_by_count_pages_at_most_100_at_a_time_and_a_count_of_none_writes_nothing_past_the_resume_body() {
        // CA5; turn-pages.ts:44-88.
        let ids: Vec<String> = (0..250).map(|i| format!("r{i}")).collect();
        let launch = answering(move |m, w| match method(m) {
            "thread/resume" => w.push1(json!({"id":"wsp-thread","result":{"thread":{"id":THREAD_ID,"turns":[]},"model":"gpt-5.6-sol"}}).to_string()),
            "thread/turns/list" => {
                let from = m["params"]["cursor"].as_str().map(|c| c.parse::<usize>().unwrap()).unwrap_or(0);
                let limit = m["params"]["limit"].as_u64().unwrap() as usize;
                let data: Vec<Value> = ids.iter().skip(from).take(limit).map(|id| json!({"id": id, "items": [], "status": "completed"})).collect();
                let next = if from + limit < ids.len() { json!((from + limit).to_string()) } else { Value::Null };
                w.push1(json!({"id": m["id"], "result": {"data": data, "nextCursor": next}}).to_string());
            }
            "thread/revert" => w.push1(json!({"id":"wsp-revert","result":{"thread":{"id":THREAD_ID,"turns":[]}}}).to_string()),
            _ => {}
        });
        let reverted = adapter_over(&launch).revert(RevertAsk { session: THREAD_ID.into(), cwd: None, at: RevertAt::Turns((0..250).map(|_| done("x")).collect()) }).await.unwrap();
        assert_eq!(reverted, Reverted::Cut);
        let w = launch.wire(0);
        let limits: Vec<Value> = w.written_method("thread/turns/list").into_iter().map(|m| m["params"]["limit"].clone()).collect();
        assert_eq!(limits, [json!(100), json!(100), json!(50)]);
        assert_eq!(find_method(&w, "thread/revert"), json!({"id":"wsp-revert","method":"thread/revert","params":{"threadId":THREAD_ID,"beforeTurnId":"r249"}}));

        let zero = answering(|m, w| {
            if method(m) == "thread/resume" {
                w.push1(json!({"id":"wsp-thread","result":{"thread":{"id":THREAD_ID,"turns":[]}}}).to_string());
            }
        });
        assert_eq!(adapter_over(&zero).revert(RevertAsk { session: THREAD_ID.into(), cwd: None, at: RevertAt::Turns(vec![]) }).await, Ok(Reverted::Cut));
        let methods: Vec<String> = zero.wire(0).written().iter().map(|m| method(m).to_string()).collect();
        assert_eq!(methods, ["initialize", "initialized", "thread/resume"]);
    }

    #[tokio::test]
    async fn an_attached_run_that_never_spoke_answers_every_method_as_not_running_and_a_stop_tears_it_down_alone() {
        tokio::task::LocalSet::new().run_until(an_attached_run_that_never_spoke_answers_every_method_as_not_running_and_a_stop_tears_it_down_alone_body()).await
    }

    async fn an_attached_run_that_never_spoke_answers_every_method_as_not_running_and_a_stop_tears_it_down_alone_body() {
        // CA6; adapter.ts:1397-1413 with follow's methods before any line.
        let launch = launcher(scripted(vec![], false));
        let quiet = wire(WireOpts::default());
        let stream = quiet.stream();
        launch.set_attach(move |_, _| {
            let stream = stream.clone();
            async move { Ok(wsp_adapters::port::Attached::Stream(stream)) }.boxed_local()
        });
        let a = create_codex_adapter(CodexAdapterDeps { steer_join_ms: Some(5), ..CodexAdapterDeps::new(launch.factory(), "/root/.codex", LOGIN) }).unwrap();
        let events = Events::default();
        let attached = a
            .attach(AdapterAttachOptions { run: "/tmp/wsp-run/cd34".into(), session_id: "SESS-1".into(), started_at: 77.0, model: None, cwd: None, prompt: None, effort: None, from: None, steered: None, on_event: events.on_event() })
            .expect("the factory attaches")
            .await
            .unwrap();
        let AttachedSession::Session(session) = attached else { panic!("gone") };
        assert_eq!(session.thread_id(), "SESS-1");
        assert_eq!(session.steer("x".into()).await, SteerOutcome::NotRunning);
        let allow = PermissionAnswer { option_id: PERMISSION_ALLOW.into(), outcome: PermissionOutcome::Allowed, deny_message: String::new(), reason: None };
        assert_eq!(session.answer("1".into(), allow).await, Ok(AnswerOutcome::Gone));
        assert_eq!(session.stop_task("k".into()).await, TaskStop::NotRunning);
        assert!(session.kept().is_none());
        session.interrupt().await;
        assert_eq!(session.finished().await, result_of(TurnStatus::Interrupted));
        assert_eq!(events.kinds(), ["turn.done", "session.end"]);
        assert!(matches!(&events.all()[1], AdapterEvent::SessionEnd { session_id, exit_code: Some(0), saw_result: false, .. } if session_id == "SESS-1"));
        assert!(quiet.written().is_empty());
        assert_eq!(quiet.order(), ["teardown"]);
    }
}
