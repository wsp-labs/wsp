// SPDX-License-Identifier: AGPL-3.0-only
//! The history readers, the parser and the cache against what the TS answered for the same inputs
//! (tests/fixtures/oracle/{commands,reader,tally,history-cache}.json, written by oracle/rest.oracle.ts), and the
//! cache's restart and rewrite rules. spec/history_commands.md, history_readers.md and history_store.md.
mod common;

use std::cell::RefCell;

use common::fake_host::*;
use futures::StreamExt;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_catalog::catalog::catalog_agents;
use wsp_collect::history::cache::{file_history_cache, HistoryCache};
use wsp_collect::history::claude::claude_session;
use wsp_collect::history::commands::{command_names, install_names, split_commands, without_heredocs, Install};
use wsp_collect::history::hermes::{HermesReader, QUERY, QUERY_WITH_CWD};
use wsp_collect::history::reader::{stamp_of, stem, time_of, CallKind, HistoryReader};
use wsp_collect::history::tally::{one_holder, reduce_calls, usage_of, Count, Held, HistoryBucket};
use wsp_collect::history::{read_histories, read_store, HistoryOptions, StoreOptions};

fn oracle(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/oracle/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect()
}

fn claude() -> Vec<&'static wsp_catalog::catalog::AgentEntry> {
    catalog_agents().into_iter().filter(|a| a.id == "claude").collect()
}

mod parser {
    use super::*;

    #[test]
    fn every_line_of_the_oracle_splits_names_and_installs_as_the_ts_did() {
        for case in oracle("commands.json").as_array().unwrap() {
            let line = case["line"].as_str().unwrap();
            assert_eq!(without_heredocs(line), case["heredocless"].as_str().unwrap(), "heredocs of {line:?}");
            assert_eq!(split_commands(&without_heredocs(line)), strings(&case["split"]), "split of {line:?}");
            assert_eq!(command_names(line), strings(&case["names"]), "names of {line:?}");
            let installs: Vec<Install> = case["installs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| Install { via: i["via"].as_str().unwrap().to_string(), name: i["name"].as_str().unwrap().to_string() })
                .collect();
            assert_eq!(install_names(line), installs, "installs of {line:?}");
        }
    }

    #[test]
    fn an_empty_or_blank_line_runs_nothing_and_installs_nothing() {
        for line in ["", "   ", "\n\n", "# only a comment"] {
            assert_eq!(command_names(line), Vec::<String>::new(), "{line:?}");
            assert_eq!(install_names(line), Vec::<Install>::new(), "{line:?}");
        }
    }
}

mod reader_helpers {
    use super::*;

    #[test]
    fn time_of_reads_what_date_parse_reads_with_an_offset_or_a_bare_date_and_nothing_else() {
        for case in oracle("reader.json")["timeOf"].as_array().unwrap() {
            let value = &case["value"];
            // A date-time with no offset is local time in JavaScript; the oracle ran at UTC, so it is left to the spec.
            if value.as_str().is_some_and(|s| s.len() == 19) {
                continue;
            }
            let arg = if value == "<undefined>" { None } else { Some(value) };
            assert_eq!(time_of(arg), case["at"].as_f64(), "{value}");
        }
    }

    #[test]
    fn stem_cuts_the_folder_and_the_last_extension_but_keeps_a_leading_dot() {
        for case in oracle("reader.json")["stem"].as_array().unwrap() {
            assert_eq!(stem(case["path"].as_str().unwrap()), case["stem"].as_str().unwrap(), "{}", case["path"]);
        }
    }

    #[test]
    fn claude_session_keys_every_shape_of_path_as_the_ts_did() {
        for case in oracle("reader.json")["claudeSession"].as_array().unwrap() {
            let rel = case["rel"].as_str().unwrap();
            assert_eq!(claude_session(rel).as_deref(), case["session"].as_str(), "{rel:?}");
        }
    }

    #[test]
    fn a_stamp_is_the_mtime_as_javascript_writes_it_then_the_size() {
        let host = fake_host(FakeLaptop::new().file("~/a", 4).mtime("~/a", 1500.25).file("~/b", "xy").file("~/d/x", 1));
        block_on(async {
            assert_eq!(stamp_of(&host, &format!("{HOME}/a")).await.unwrap().stamp, "1500.25:4");
            assert_eq!(stamp_of(&host, &format!("{HOME}/b")).await.unwrap().stamp, "1000:2");
            // A folder or a missing path has no stamp.
            assert_eq!(stamp_of(&host, &format!("{HOME}/d")).await, None);
            assert_eq!(stamp_of(&host, &format!("{HOME}/none")).await, None);
        });
    }
}

mod hermes {
    use super::*;

    const DB: &str = "/Users/dev/.hermes/state.db";

    fn read_all(host: &FakeHost) -> Vec<Result<CallKind, String>> {
        block_on(async { HermesReader.read(host, DB, DB).map(|r| r.map(|c| c.kind).map_err(|e| e.message)).collect::<Vec<_>>().await })
    }

    #[test]
    fn an_empty_answer_is_no_rows_and_a_json_answer_that_is_not_a_list_is_an_error() {
        let empty = fake_host(
            FakeLaptop::new().file("~/.hermes/state.db", 4096).exec(&format!("sqlite3 -readonly -json {DB} {QUERY_WITH_CWD}"), "  \n"),
        );
        assert_eq!(read_all(&empty), vec![]);
        let object = fake_host(
            FakeLaptop::new().file("~/.hermes/state.db", 4096).exec(&format!("sqlite3 -readonly -json {DB} {QUERY}"), "{\"x\":1}"),
        );
        assert_eq!(read_all(&object), vec![Err(format!("{DB}: sqlite3 returned no JSON"))]);
        let none = fake_host(FakeLaptop::new().file("~/.hermes/state.db", 4096));
        assert_eq!(read_all(&none), vec![Err(format!("{DB} could not be read with sqlite3"))]);
    }

    #[test]
    fn a_row_without_a_string_session_or_with_unreadable_tool_calls_gives_nothing() {
        let rows = json!([
            {"session_id": 7, "tool_calls": "[]"},
            {"session_id": "h1", "tool_calls": "not json"},
            {"session_id": "h1", "tool_calls": json!([{"function": {"name": "terminal", "arguments": {"command": "ls"}}}, {"function": {"name": 3}}, "x"]).to_string()},
        ]);
        let host = fake_host(
            FakeLaptop::new()
                .file("~/.hermes/state.db", 4096)
                .exec(&format!("sqlite3 -readonly -json {DB} {QUERY_WITH_CWD}"), &rows.to_string()),
        );
        assert_eq!(read_all(&host), vec![Ok(CallKind::Shell { line: "ls".to_string() })]);
    }
}

mod tally {
    use super::*;

    fn bucket(v: &Value) -> HistoryBucket {
        serde_json::from_value(v.clone()).unwrap()
    }

    #[test]
    fn one_holder_keeps_each_session_from_the_holder_whose_newest_call_is_latest_the_first_on_a_tie() {
        let t = oracle("tally.json");
        let held: Vec<Held> = t["held"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| Held {
                holder: h["holder"].as_str().unwrap().to_string(),
                buckets: h["buckets"].as_array().unwrap().iter().map(bucket).collect(),
            })
            .collect();
        let kept = one_holder(&held);
        let want: Vec<HistoryBucket> = t["kept"].as_array().unwrap().iter().map(bucket).collect();
        assert_eq!(kept, want);
        let usage = usage_of(&kept, None);
        assert_eq!((usage.sessions, usage.calls), (3, 4));
        let commands: Vec<(String, Count)> = usage.commands.into_iter().collect();
        let want: Vec<(String, Count)> = t["usage"]["commands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p[0].as_str().unwrap().to_string(),
                    Count { sessions: p[1]["sessions"].as_u64().unwrap(), calls: p[1]["calls"].as_u64().unwrap() },
                )
            })
            .collect();
        assert_eq!(commands, want);
        assert_eq!(usage.tools.into_iter().collect::<Vec<_>>(), vec![("jq".to_string(), Count { sessions: 1, calls: 1 })]);
    }

    #[test]
    fn reduce_calls_makes_one_bucket_per_session_and_folder_in_first_call_order_keeping_the_newest_time() {
        use wsp_collect::history::reader::Call;
        let call = |session: &str, folder: Option<&str>, at: Option<f64>, kind: CallKind| Call {
            session: session.into(),
            folder: folder.map(Into::into),
            at,
            kind,
        };
        let shell = |l: &str| CallKind::Shell { line: l.into() };
        let calls = vec![
            Ok(call("s1", Some("/a"), Some(5.0), shell("gh pr view && brew install jq gh"))),
            Ok(call("s2", None, None, CallKind::Other { name: "Read".into() })),
            Ok(call("s1", Some("/b"), None, shell("ls"))),
            Ok(call("s1", Some("/a"), Some(3.0), shell("gh api"))),
            Ok(call("s1", Some("/a"), Some(9.0), CallKind::Other { name: "Edit".into() })),
        ];
        let buckets = block_on(reduce_calls(futures::stream::iter(calls).boxed_local())).unwrap();
        let text: Vec<String> = buckets.iter().map(|b| serde_json::to_string(b).unwrap()).collect();
        assert_eq!(
            text,
            vec![
                r#"{"session":"s1","folder":"/a","calls":3,"commands":{"gh":2,"brew":1},"installs":{"brew jq":1,"brew gh":1},"at":9}"#,
                r#"{"session":"s2","calls":1,"commands":{},"installs":{}}"#,
                r#"{"session":"s1","folder":"/b","calls":1,"commands":{"ls":1},"installs":{}}"#,
            ]
        );
    }

    #[test]
    fn reduce_calls_hands_back_the_first_error_and_reads_nothing_after_it() {
        use wsp_collect::history::reader::Call;
        let read = RefCell::new(0);
        let calls = futures::stream::iter(0..3).map(|i| {
            *read.borrow_mut() += 1;
            if i == 1 {
                Err(wsp_words::exit::HostError::new("EIO"))
            } else {
                Ok(Call { session: "s".into(), folder: None, at: None, kind: CallKind::Other { name: "x".into() } })
            }
        });
        let out = block_on(reduce_calls(calls.boxed_local()));
        assert_eq!(out.unwrap_err().message, "EIO");
        assert_eq!(*read.borrow(), 2);
    }
}

mod store {
    use super::*;

    fn two_sessions() -> FakeLaptop {
        let files = oracle("history-cache.json")["files"].clone();
        let mut laptop = FakeLaptop::new().mtime("~/.claude/projects/-Users-dev-proj/s2.jsonl", 1500.25);
        for (path, text) in files.as_object().unwrap() {
            laptop = laptop.file(path, text.as_str().unwrap());
        }
        laptop
    }

    #[test]
    fn the_cache_file_holds_what_the_ts_wrote_for_the_same_history() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history-cache.json");
        let cache = file_history_cache(&path);
        block_on(read_histories(&fake_host(two_sessions()), &claude(), HistoryOptions { cache: Some(&cache), ..Default::default() }))
            .unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        // One line of JSON and a newline; the top keys in this order. Which file's entry comes first follows which read
        // finished first, so the entries are compared as a map, and each bucket's key order as text.
        assert!(text.ends_with("}\n") && text.matches('\n').count() == 1, "{text:?}");
        assert!(text.starts_with(r#"{"version":1,"parse":2,"files":{"#), "{text}");
        let want: Value = serde_json::from_str(oracle("history-cache.json")["text"].as_str().unwrap()).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), want);
        assert!(text.contains(r#"{"session":"s1","folder":"/Users/dev/proj","calls":2,"commands":{"gh":2,"brew":1},"installs":{"brew jq":1},"at":1788688860000}"#), "{text}");
        assert!(text.contains(r#""stamp":"1500.25:185""#), "{text}");
    }

    #[test]
    fn a_new_cache_over_the_same_file_trusts_what_the_last_run_kept_as_after_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history-cache.json");
        block_on(read_histories(
            &fake_host(two_sessions()),
            &claude(),
            HistoryOptions { cache: Some(&file_history_cache(&path)), ..Default::default() },
        ))
        .unwrap();
        let again = file_history_cache(&path);
        let s1 = format!("{HOME}/.claude/projects/-Users-dev-proj/s1.jsonl");
        let kept = again.get(&s1, "1000:431").unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].commands, IndexMap::from([("gh".to_string(), 2), ("brew".to_string(), 1)]));
        assert_eq!(kept[0].at, Some(1788688860000.0));
        // Another stamp is a miss, and a miss keeps nothing for the next save.
        assert_eq!(again.get(&s1, "1001:431"), None);
    }

    #[test]
    fn a_run_that_changed_nothing_leaves_the_file_as_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history-cache.json");
        block_on(read_histories(
            &fake_host(two_sessions()),
            &claude(),
            HistoryOptions { cache: Some(&file_history_cache(&path)), ..Default::default() },
        ))
        .unwrap();
        // The same content written another way: a save would put back the one-line form.
        let pretty =
            serde_json::to_string_pretty(&serde_json::from_str::<Value>(&std::fs::read_to_string(&path).unwrap()).unwrap()).unwrap();
        std::fs::write(&path, &pretty).unwrap();
        let host = fake_host(two_sessions());
        block_on(read_histories(&host, &claude(), HistoryOptions { cache: Some(&file_history_cache(&path)), ..Default::default() }))
            .unwrap();
        assert!(host.calls().iter().all(|c| !c.starts_with("lines ")), "{:?}", host.calls());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), pretty);
    }

    #[test]
    fn a_save_with_nothing_asked_of_a_filled_cache_empties_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history-cache.json");
        block_on(read_histories(
            &fake_host(two_sessions()),
            &claude(),
            HistoryOptions { cache: Some(&file_history_cache(&path)), ..Default::default() },
        ))
        .unwrap();
        file_history_cache(&path).save().unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"version\":1,\"parse\":2,\"files\":{}}\n");
    }

    #[test]
    fn a_cache_file_of_another_version_or_shape_is_read_as_empty_and_replaced_whole() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deep/er/history-cache.json");
        for bad in [
            "",
            "null",
            "[]",
            r#"{"version":2,"parse":2,"files":{}}"#,
            r#"{"version":1,"parse":2,"files":{"/x":{"stamp":"1:1","buckets":[{"session":"s","calls":-1,"commands":{},"installs":{}}]}}}"#,
        ] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, bad).unwrap();
            let c = file_history_cache(&path);
            assert_eq!(c.get("/x", "1:1"), None, "{bad:?}");
        }
        // A missing folder is made on save.
        let fresh = dir.path().join("new/folder/cache.json");
        let c = file_history_cache(&fresh);
        c.set("/f", "1:2", &[]);
        c.save().unwrap();
        assert_eq!(
            std::fs::read_to_string(&fresh).unwrap(),
            "{\"version\":1,\"parse\":2,\"files\":{\"/f\":{\"stamp\":\"1:2\",\"buckets\":[]}}}\n"
        );
    }

    #[test]
    fn a_store_with_no_files_is_empty_and_tells_no_progress() {
        let told = RefCell::new(Vec::new());
        let on_file = |read: usize, files: usize| told.borrow_mut().push((read, files));
        let usage = block_on(read_store(
            &empty_host(),
            wsp_collect::history::history_reader(wsp_catalog::catalog::HistoryFormat::ClaudeJsonl),
            "/Users/dev/.claude/projects",
            StoreOptions { on_file: Some(&on_file), ..Default::default() },
        ))
        .unwrap();
        assert_eq!((usage.sessions, usage.calls, usage.commands.len(), usage.installs.len(), usage.tools.len()), (0, 0, 0, 0, 0));
        assert!(told.borrow().is_empty());
    }

    #[test]
    fn a_failed_save_is_the_error_read_histories_answers() {
        let dir = tempfile::tempdir().unwrap();
        // The cache's folder is a file, so the save cannot make it.
        std::fs::write(dir.path().join("blocked"), "").unwrap();
        let cache = file_history_cache(dir.path().join("blocked/cache.json"));
        let out =
            block_on(read_histories(&fake_host(two_sessions()), &claude(), HistoryOptions { cache: Some(&cache), ..Default::default() }));
        assert!(out.is_err());
    }
}
