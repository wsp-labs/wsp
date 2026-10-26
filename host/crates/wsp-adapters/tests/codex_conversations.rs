// SPDX-License-Identifier: AGPL-3.0-only
// Codex's threads read for a person picking one up: the store's scripts run under bash against a fake app server that
// answers with what codex-cli 0.162.1 answered in a throwaway home (a TUI's thread, a wsp thread, their turns and items,
// a fork, and the refusal of a second writer). Ports packages/adapter-codex/test/conversations.test.ts and one case of
// placeholder-login.test.ts.
#![allow(dead_code)]

#[path = "common/codex_shell.rs"]
mod codex_shell;

use std::collections::HashMap;
use std::sync::Arc;

use futures::future::{FutureExt, LocalBoxFuture};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_adapters::codex::catalog::{catalog_probe_command, version_probe_command};
use wsp_adapters::codex::command::{build_env, codex_launch, codex_word, BrokerMode, CodexBroker, CodexEnvOptions};
use wsp_adapters::codex::conversations::{codex_conversations, items_command, list_command, turns_command, CodexConversations, CodexRoad};
use wsp_adapters::codex::session_title::{draft_for_command, rename_command, title_for_command};
use wsp_adapters::pending::CodexClaims;
use wsp_adapters::port::{
    ConversationEarlier, ConversationLine, ConversationOrigin, ConversationRoad, ConversationStore, EarlierAsk, EarlierRead, Env,
    ExecFailure, ExecFailureKind, StoredConversation,
};

use codex_shell::{fake_app_server, nul_env, run_shell, FakeAppServer, TempDir};

const TUI: &str = "01a12813-cd12-7a12-9b13-e76892906ff0";
const WSP: &str = "01a12813-ce2c-7600-87d3-bfff156be264";
const LAB: &str = "/work/acme/lab";

fn recorded() -> Map<String, Value> {
    let path = format!("{}/tests/fixtures/codex/app-server-threads-0.162.1.json", env!("CARGO_MANIFEST_DIR"));
    match serde_json::from_str::<Value>(&std::fs::read_to_string(&path).unwrap()).unwrap() {
        Value::Object(m) => m,
        _ => panic!("not an object"),
    }
}

/// Each exec runs under bash with PATH=/usr/bin:/bin and a HOME, the variables on its input as a machine's exec runs them.
fn shell_exec(command: &str, env: Option<&Env>) -> LocalBoxFuture<'static, Result<String, ExecFailure>> {
    let command = command.to_string();
    let input = env.map(|e| nul_env(e.iter())).unwrap_or_default();
    async move {
        let ran = tokio::task::spawn_blocking(move || {
            let shell_env = vec![
                ("PATH".to_string(), "/usr/bin:/bin".to_string()),
                ("HOME".to_string(), std::env::temp_dir().to_string_lossy().into_owned()),
            ];
            run_shell("bash", &command, &shell_env, &input)
        })
        .await
        .unwrap();
        Ok(ran.stdout)
    }
    .boxed_local()
}

fn plain_road() -> ConversationRoad {
    ConversationRoad {
        exec: Arc::new(shell_exec),
        ask: Arc::new(|_frame| {
            async { Err(ExecFailure { kind: ExecFailureKind::Other, message: "codex asks no daemon".into() }) }.boxed_local()
        }),
    }
}

struct Over {
    f: FakeAppServer,
    store: Arc<CodexConversations>,
    road: ConversationRoad,
}

/// The store, its every line run under bash against a fake app server answering each method as given.
fn store_over(answers: Value) -> Over {
    let f = fake_app_server(answers);
    let mut base = IndexMap::new();
    base.insert("PATH".to_string(), Some(f.path.clone()));
    let env = build_env(&CodexEnvOptions { base: Some(base), home: f.home.clone(), ..Default::default() }).unwrap();
    let store = codex_conversations(&f.home, CodexRoad::default(), env);
    Over { f, store, road: plain_road() }
}

impl Over {
    /// What the fake was asked, but for the initialize.
    fn requests(&self) -> Vec<Map<String, Value>> {
        self.f.requests().into_iter().filter(|r| r.get("id").is_some() && r.get("method") != Some(&json!("initialize"))).collect()
    }
    fn methods(&self) -> Vec<String> {
        self.requests().iter().map(|r| r["method"].as_str().unwrap().to_string()).collect()
    }
}

fn earlier_ask(last: u32, probe: bool, count: bool) -> EarlierAsk {
    EarlierAsk { cwds: vec![LAB.to_string()], last, probe, tail: None, count }
}

fn line(who: &str, text: &str, at: f64) -> ConversationLine {
    ConversationLine { who: who.to_string(), text: text.to_string(), tool: None, at: Some(at) }
}

/// The recorded thread/list answer with each rollout moved to a file of a known size in a folder of the test's.
fn listed_with_files() -> (TempDir, Value, HashMap<String, f64>) {
    let dir = TempDir::new("wsp-codex-rollouts-");
    let mut answer = recorded()["thread-list"].clone();
    let mut sizes = HashMap::new();
    for (i, thread) in answer["result"]["data"].as_array_mut().unwrap().iter_mut().enumerate() {
        let path = dir.join(&format!("rollout-{i}.jsonl"));
        std::fs::write(&path, "x".repeat(1000 * (i + 1))).unwrap();
        thread["path"] = json!(path.to_string_lossy());
        sizes.insert(thread["id"].as_str().unwrap().to_string(), (1000 * (i + 1)) as f64);
    }
    (dir, answer, sizes)
}

mod the_threads_codex_kept_in_a_folder {
    use super::*;

    #[tokio::test]
    async fn come_off_thread_list_on_the_computers_own_app_server_newest_first_with_a_name_a() {
        tokio::task::LocalSet::new().run_until(come_off_thread_list_on_the_computers_own_app_server_newest_first_with_a_name_a_body()).await
    }

    async fn come_off_thread_list_on_the_computers_own_app_server_newest_first_with_a_name_a_body() {
        let (_dir, answer, sizes) = listed_with_files();
        let o = store_over(json!({ "thread/list": answer }));
        let rows = o.store.list(vec![LAB.to_string(), format!("{LAB}-wt")], o.road.clone()).await.unwrap();
        let asked: Vec<(Value, Value)> = o.requests().iter().map(|r| (r["method"].clone(), r["params"].clone())).collect();
        assert_eq!(
            asked,
            vec![(json!("thread/list"), json!({ "cwd": [LAB, format!("{LAB}-wt")], "sortKey": "recency_at", "limit": 100 }))]
        );
        assert_eq!(
            rows,
            vec![
                StoredConversation {
                    id: WSP.into(),
                    title: "first prompt from wsp".into(),
                    first_prompt: Some("first prompt from wsp".into()),
                    branch: Some("feat/x".into()),
                    cwd: LAB.into(),
                    last_at: 1_791_673_749_000.0,
                    bytes: Some(sizes[WSP]),
                    origin: ConversationOrigin::Wsp
                },
                StoredConversation {
                    id: TUI.into(),
                    title: "lab terminal thread".into(),
                    first_prompt: Some("first prompt from codex-tui".into()),
                    branch: Some("feat/x".into()),
                    cwd: LAB.into(),
                    last_at: 1_791_673_748_000.0,
                    bytes: Some(sizes[TUI]),
                    origin: ConversationOrigin::Terminal
                },
            ]
        );
    }

    #[tokio::test]
    async fn pages_thread_list_by_its_cursor_and_leaves_out_the_size_of_a_rollout_that_is_not() {
        tokio::task::LocalSet::new()
            .run_until(pages_thread_list_by_its_cursor_and_leaves_out_the_size_of_a_rollout_that_is_not_body())
            .await
    }

    async fn pages_thread_list_by_its_cursor_and_leaves_out_the_size_of_a_rollout_that_is_not_body() {
        let rec = recorded();
        let data = rec["thread-list"]["result"]["data"].as_array().unwrap().clone();
        let first = json!({ "result": { "data": [data[0]], "nextCursor": "page-2", "backwardsCursor": null } });
        let second = json!({ "result": { "data": [data[1]], "nextCursor": null, "backwardsCursor": null } });
        let o = store_over(json!({ "thread/list": [first, second] }));
        let rows = o.store.list(vec![LAB.to_string()], o.road.clone()).await.unwrap();
        assert_eq!(rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), vec![WSP.to_string(), TUI.to_string()]);
        assert!(rows.iter().all(|r| r.bytes.is_none()));
        let cursors: Vec<Option<Value>> = o.requests().iter().map(|r| r["params"].get("cursor").cloned()).collect();
        assert_eq!(cursors, vec![None, Some(json!("page-2"))]);
    }

    #[tokio::test]
    async fn asks_nothing_for_no_folder_at_all() {
        tokio::task::LocalSet::new().run_until(asks_nothing_for_no_folder_at_all_body()).await
    }

    async fn asks_nothing_for_no_folder_at_all_body() {
        let o = store_over(json!({}));
        assert_eq!(o.store.list(vec![], o.road.clone()).await.unwrap(), vec![]);
        assert!(o.requests().is_empty());
    }
}

mod a_codex_threads_earlier_messages {
    use super::*;

    fn answered() -> Map<String, Value> {
        let rec = recorded();
        let mut m = Map::new();
        m.insert("thread/resume".into(), json!({ "result": rec["thread-read"]["result"] }));
        m.insert("thread/read".into(), rec["thread-read"].clone());
        m.insert("thread/turns/list".into(), rec["turns-list"].clone());
        m.insert("thread/items/list".into(), rec["items-list"].clone());
        m
    }

    fn with(key: &str, value: Value) -> Value {
        let mut m = answered();
        m.insert(key.into(), value);
        Value::Object(m)
    }

    #[tokio::test]
    async fn come_off_thread_turns_list_and_thread_items_list_each_at_the_time_codex_stamped() {
        tokio::task::LocalSet::new().run_until(come_off_thread_turns_list_and_thread_items_list_each_at_the_time_codex_stamped_body()).await
    }

    async fn come_off_thread_turns_list_and_thread_items_list_each_at_the_time_codex_stamped_body() {
        let o = store_over(Value::Object(answered()));
        let read = o.store.earlier(TUI.into(), earlier_ask(200, true, true), o.road.clone()).await.unwrap();
        assert_eq!(
            read,
            EarlierRead::Read(ConversationEarlier {
                cwd: Some(LAB.into()),
                title: Some("lab terminal thread".into()),
                lines: vec![
                    line("person", "first prompt from codex-tui", 1791673748846.0),
                    line("agent", "stub reply to: first prompt from codex-tui", 1791673748862.0),
                    line("person", "second turn", 1791673748894.0),
                    line("agent", "stub reply to: second turn", 1791673748904.0),
                ],
                earlier: 0,
            })
        );
        assert_eq!(o.methods(), vec!["thread/resume", "thread/turns/list", "thread/items/list"]);
        assert_eq!(o.requests()[1]["params"]["sortDirection"], json!("desc"));
    }

    #[tokio::test]
    async fn for_a_preview_read_one_page_of_turns_and_count_nothing_the_thread_never_taken() {
        tokio::task::LocalSet::new().run_until(for_a_preview_read_one_page_of_turns_and_count_nothing_the_thread_never_taken_body()).await
    }

    async fn for_a_preview_read_one_page_of_turns_and_count_nothing_the_thread_never_taken_body() {
        let rec = recorded();
        let turns = rec["turns-list"]["result"].as_object().unwrap().clone();
        let mut first = turns.clone();
        first.insert("nextCursor".into(), json!("page-2"));
        let mut second = turns.clone();
        second.insert("nextCursor".into(), Value::Null);
        let paged = json!([{ "result": first }, { "result": second }]);
        let o = store_over(with("thread/turns/list", paged));
        let read = o.store.earlier(TUI.into(), earlier_ask(1, false, false), o.road.clone()).await.unwrap();
        assert_eq!(
            read,
            EarlierRead::Read(ConversationEarlier {
                cwd: Some(LAB.into()),
                title: Some("lab terminal thread".into()),
                lines: vec![line("agent", "stub reply to: second turn", 1791673748904.0)],
                earlier: 0
            })
        );
        assert_eq!(o.methods(), vec!["thread/read", "thread/turns/list", "thread/items/list"]);
    }

    #[tokio::test]
    async fn keep_the_newest_messages_and_count_the_rest_and_read_the_thread_without_taking_i() {
        tokio::task::LocalSet::new()
            .run_until(keep_the_newest_messages_and_count_the_rest_and_read_the_thread_without_taking_i_body())
            .await
    }

    async fn keep_the_newest_messages_and_count_the_rest_and_read_the_thread_without_taking_i_body() {
        let o = store_over(Value::Object(answered()));
        let read = o.store.earlier(TUI.into(), earlier_ask(1, false, true), o.road.clone()).await.unwrap();
        assert_eq!(
            read,
            EarlierRead::Read(ConversationEarlier {
                cwd: Some(LAB.into()),
                title: Some("lab terminal thread".into()),
                lines: vec![line("agent", "stub reply to: second turn", 1791673748904.0)],
                earlier: 3
            })
        );
        assert_eq!(o.requests()[0]["method"], json!("thread/read"));
    }

    #[tokio::test]
    async fn are_open_where_codex_refuses_the_resume_a_second_writer_and_gone_where_it_holds() {
        tokio::task::LocalSet::new().run_until(are_open_where_codex_refuses_the_resume_a_second_writer_and_gone_where_it_holds_body()).await
    }

    async fn are_open_where_codex_refuses_the_resume_a_second_writer_and_gone_where_it_holds_body() {
        let rec = recorded();
        let held = store_over(with("thread/resume", rec["resume-held"].clone()));
        assert_eq!(held.store.earlier(TUI.into(), earlier_ask(200, true, true), held.road.clone()).await.unwrap(), EarlierRead::Open);
        let gone = store_over(with(
            "thread/resume",
            json!({ "error": { "code": -32600, "message": format!("no rollout found for thread id {TUI}") } }),
        ));
        assert_eq!(gone.store.earlier(TUI.into(), earlier_ask(200, true, true), gone.road.clone()).await.unwrap(), EarlierRead::Gone);
        let unread =
            store_over(with("thread/read", json!({ "error": { "code": -32600, "message": format!("thread not loaded: {TUI}") } })));
        assert_eq!(unread.store.earlier(TUI.into(), earlier_ask(200, false, true), unread.road.clone()).await.unwrap(), EarlierRead::Gone);
    }

    #[tokio::test]
    async fn refuse_an_id_that_is_not_a_plain_slug_before_anything_runs() {
        tokio::task::LocalSet::new().run_until(refuse_an_id_that_is_not_a_plain_slug_before_anything_runs_body()).await
    }

    async fn refuse_an_id_that_is_not_a_plain_slug_before_anything_runs_body() {
        let o = store_over(Value::Object(answered()));
        let err = o.store.earlier("x' or '1'='1".into(), earlier_ask(1, false, true), o.road.clone()).await.unwrap_err();
        assert!(err.message.contains("plain slug"), "{}", err.message);
        assert!(o.requests().is_empty());
    }

    #[test]
    fn say_a_person_may_try_again_once_codex_lets_go_and_how_soon() {
        let o = store_over(json!({}));
        let says = o.store.lets_go().expect("letsGo");
        assert!(says.contains("about a minute after its window closes"), "{says}");
    }
}

mod every_other_codex_launch_on_a_brokered_machine {
    use super::*;

    fn chatgpt() -> CodexBroker {
        CodexBroker {
            mode: BrokerMode::Chatgpt {
                claims: CodexClaims {
                    account_id: "11111111-2222-4333-8444-555555555555".into(),
                    plan_type: Some("plus".into()),
                    user_id: Some("user-acme0001".into()),
                    fedramp: Some(false),
                },
                models: None,
            },
            providers: None,
        }
    }

    #[test]
    fn titles_drafts_renames_probes_and_conversation_reads_each_run_codex_through_it() {
        let now = 1_791_900_000_000_i64;
        let broker = chatgpt();
        let b = Some(&broker);
        let question = codex_launch(None, b, now).unwrap();
        let expected = {
            let mut words = vec![question.word.clone(), "exec".to_string()];
            words.extend(question.flags.iter().cloned());
            words.join(" ")
        };
        for l in
            [title_for_command("name this", None, None, b, now).unwrap(), draft_for_command("/tmp/acme-draft", None, None, b, now).unwrap()]
        {
            assert!(l.contains(&expected), "{l}");
        }
        let word = codex_word(None, b).unwrap();
        let road = CodexRoad { launch: None, broker: Some(broker.clone()) };
        let lines = [
            rename_command("t1", "acme", None, b).unwrap(),
            catalog_probe_command(None, b).unwrap(),
            version_probe_command(None, b).unwrap(),
            list_command(&["/root/acme".to_string()], None, &road).unwrap(),
            turns_command("t1", true, None, &road).unwrap(),
            items_command("t1", None, &road).unwrap(),
        ];
        for l in &lines {
            assert!(l.contains(&word), "{l}");
        }
        assert!(!title_for_command("name this", None, None, None, now).unwrap().contains("cli_auth_credentials_store"));
    }
}

/// Rules CV1 to CV3 of spec/codex_conversations.md, "Rules added for the port", each value printed by the TS at 65f6b89c7.
mod rules_added {
    use wsp_adapters::codex::conversations::{parse_list, parse_opened, parse_sizes, Opened};

    #[test]
    fn a_blank_preview_trims_to_an_empty_title_and_first_prompt_and_a_blank_name_is_kept_as_it_is() {
        // CV1; conversations.ts:63-82 (`str(preview)?.trim()`, `title ?? preview`).
        let page = parse_list("{\"id\":1,\"result\":{}}\n{\"id\":2,\"result\":{\"data\":[{\"id\":\"T1\",\"cwd\":\"/w\",\"preview\":\"   \"},{\"id\":\"T2\",\"cwd\":\"/w\",\"name\":\"  \",\"preview\":\"P\"}]}}").unwrap();
        let rows: Vec<(String, String, Option<String>)> =
            page.rows.into_iter().map(|r| (r.row.id, r.row.title, r.row.first_prompt)).collect();
        assert_eq!(rows, [("T1".into(), String::new(), Some(String::new())), ("T2".into(), "  ".into(), Some("P".into()))]);
    }

    #[test]
    fn a_size_is_js_number_of_the_trimmed_line_where_that_is_a_safe_integer() {
        // CV2; conversations.ts:112-118.
        let got = parse_sizes(
            &["1e3", "0x10", "+5", "12.0", "-5", "Infinity", "NaN", "1_0", "5 6", "9007199254740992", "", " 7 "].join("\n"),
            13,
        );
        assert_eq!(got, [Some(1000.0), Some(16.0), Some(5.0), Some(12.0), Some(-5.0), None, None, None, None, None, None, Some(7.0), None]);
    }

    #[test]
    fn opening_reads_a_blank_preview_as_an_empty_title_and_an_empty_name_falls_to_the_preview() {
        // CV3; conversations.ts:136-149.
        let opened =
            |thread: &str| parse_opened(&format!("{{\"id\":1,\"result\":{{}}}}\n{{\"id\":3,\"result\":{{\"thread\":{thread}}}}}")).unwrap();
        assert_eq!(opened(r#"{"cwd":"/w","preview":"   "}"#), Opened::Read { cwd: Some("/w".into()), title: Some(String::new()) });
        assert_eq!(opened(r#"{"cwd":"/w","name":"","preview":"P"}"#), Opened::Read { cwd: Some("/w".into()), title: Some("P".into()) });
    }
}

/// Rule CV4 of spec/codex_conversations.md, printed by the TS at 65f6b89c7.
mod rules_added_newest {
    use wsp_adapters::codex::conversations::newest_lines;
    use wsp_adapters::port::ConversationLine;

    #[test]
    fn the_newest_lines_keep_every_tool_line_back_to_the_last_message_kept() {
        // CV4; conversations.ts:200-213, over [t0, p1, t1, a1, t2, p2, a2, t3] with a total of 10.
        let line = |who: &str, text: &str| ConversationLine { who: who.into(), text: text.into(), tool: None, at: None };
        let items = [
            line("tool", "t0"),
            line("person", "p1"),
            line("tool", "t1"),
            line("agent", "a1"),
            line("tool", "t2"),
            line("person", "p2"),
            line("agent", "a2"),
            line("tool", "t3"),
        ];
        let kept = |last: u32| {
            let r = newest_lines(&items, 10, last);
            (r.lines.into_iter().map(|l| l.text).collect::<Vec<_>>(), r.earlier)
        };
        assert_eq!(kept(0), (vec![], 10));
        assert_eq!(kept(1), (vec!["a2".to_string(), "t3".into()], 9));
        assert_eq!(kept(3), (["a1", "t2", "p2", "a2", "t3"].map(String::from).to_vec(), 7));
        let all = ["p1", "t1", "a1", "t2", "p2", "a2", "t3"].map(String::from).to_vec();
        assert_eq!(kept(4), (all.clone(), 6));
        assert_eq!(kept(9), (all, 6));
    }
}
