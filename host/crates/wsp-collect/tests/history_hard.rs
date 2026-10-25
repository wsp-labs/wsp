// SPDX-License-Identifier: AGPL-3.0-only
//! The history readers, the parser and the cache against what the TS answered for the same inputs
//! (tests/fixtures/oracle/{commands,reader,tally,history-cache}.json, written by oracle/rest.oracle.ts), and the
//! cache's restart and rewrite rules. spec/history_commands.md, history_readers.md and history_store.md.
#![allow(clippy::result_large_err, clippy::type_complexity)] // HostError is 192 bytes; the gate traces return tuples
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

/// Which error a store answers when more than one of its reads fails, and what the reads still open do after the
/// first failure (spec history_store rule 7). The TS traces were printed by a gated reader in the TS, 2026-10-25:
/// file 3 failing before file 1 throws file 1's error; file 2 failing before file 4 (which worker 0 took after file 0)
/// throws file 4's error.
mod workers {
    use super::*;
    use futures::channel::oneshot;
    use futures::future::{BoxFuture, LocalBoxFuture};
    use futures::stream::LocalBoxStream;
    use futures::FutureExt;
    use std::cell::Cell;
    use wsp_collect::history::reader::{Call, HistoryFile};
    use wsp_collect::host::Host;
    use wsp_words::exit::HostError;

    fn error(message: &str) -> HostError {
        HostError {
            message: message.to_string(),
            fix: None,
            kind: None,
            errors: None,
            warnings: None,
            left: None,
            host_key: None,
            code: None,
            status: None,
            marks: vec![],
        }
    }

    /// Each file's read waits on its own gate; the gate's message, when there is one, is the read's error.
    struct Gated {
        gates: RefCell<Vec<Option<oneshot::Receiver<Option<String>>>>>,
        stamps: Vec<String>,
        started: RefCell<Vec<usize>>,
    }

    impl HistoryReader for Gated {
        fn files<'a>(&'a self, _host: &'a Host, _root: &'a str) -> LocalBoxFuture<'a, Vec<HistoryFile>> {
            async move {
                self.stamps.iter().enumerate().map(|(i, stamp)| HistoryFile { path: format!("/f{i}"), stamp: stamp.clone() }).collect()
            }
            .boxed_local()
        }

        fn read<'a>(&'a self, _host: &'a Host, _root: &'a str, file: &'a str) -> LocalBoxStream<'a, Result<Call, HostError>> {
            let i: usize = file[2..].parse().unwrap();
            self.started.borrow_mut().push(i);
            let gate = self.gates.borrow_mut()[i].take().unwrap();
            futures::stream::once(async move {
                match gate.await.unwrap() {
                    Some(message) => Err(error(&message)),
                    None => Ok(Call { session: format!("s{i}"), folder: None, at: None, kind: CallKind::Other { name: "Read".into() } }),
                }
            })
            .boxed_local()
        }

        fn holder(&self, _root: &str, file: &str) -> String {
            file.to_string()
        }
    }

    /// Lets every ready future run before the next gate opens.
    fn settle() -> BoxFuture<'static, ()> {
        async {
            for _ in 0..50 {
                let mut once = false;
                futures::future::poll_fn(|cx| {
                    if once {
                        std::task::Poll::Ready(())
                    } else {
                        once = true;
                        cx.waker().wake_by_ref();
                        std::task::Poll::Pending
                    }
                })
                .await;
            }
        }
        .boxed()
    }

    /// Opens the gates in `order`, each with its outcome, and answers the store's result, the files started in order,
    /// and every `on_file` count told.
    fn run(files: usize, order: &[(usize, Option<&str>)]) -> (Result<u64, String>, Vec<usize>, Vec<(usize, usize)>) {
        run_with(&vec!["1:1".to_string(); files], None, order)
    }

    /// `run` with each file's stamp given and a cache passed to the store.
    fn run_with(
        stamps: &[String],
        cache: Option<&dyn HistoryCache>,
        order: &[(usize, Option<&str>)],
    ) -> (Result<u64, String>, Vec<usize>, Vec<(usize, usize)>) {
        let files = stamps.len();
        let mut senders = Vec::new();
        let mut gates = Vec::new();
        for _ in 0..files {
            let (tx, rx) = oneshot::channel();
            senders.push(Some(tx));
            gates.push(Some(rx));
        }
        let reader = Gated { gates: RefCell::new(gates), stamps: stamps.to_vec(), started: RefCell::new(vec![]) };
        let told = RefCell::new(Vec::new());
        let on_file = |read: usize, total: usize| told.borrow_mut().push((read, total));
        let host = empty_host();
        let done = Cell::new(false);
        let store = async {
            let r = read_store(&host, &reader, "/root", StoreOptions { on_file: Some(&on_file), cache, ..Default::default() }).await;
            done.set(true);
            r
        };
        let driver = async {
            settle().await;
            for (i, outcome) in order {
                let _ = senders[*i].take().unwrap().send(outcome.map(str::to_string));
                settle().await;
            }
        };
        let (result, ()) = block_on(futures::future::join(store, driver));
        assert!(done.get());
        (result.map(|u| u.calls).map_err(|e| e.message), reader.started.into_inner(), told.into_inner())
    }

    #[test]
    fn the_lowest_numbered_worker_s_failure_is_the_error_not_the_first_in_time() {
        let (result, started, told) = run(9, &[(3, Some("E3")), (1, Some("E1")), (0, None), (2, None)]);
        assert_eq!(result, Err("E1".to_string()));
        // Four reads start at once; after file 3 fails no file is started, and the two still open finish and are told.
        assert_eq!(started, vec![0, 1, 2, 3]);
        assert_eq!(told, vec![(1, 9), (2, 9)]);
    }

    #[test]
    fn a_later_file_on_worker_0_beats_an_earlier_file_on_worker_2() {
        let (result, started, told) = run(9, &[(0, None), (2, Some("E2")), (4, Some("E4")), (1, None), (3, None)]);
        assert_eq!(result, Err("E4".to_string()));
        assert_eq!(started, vec![0, 1, 2, 3, 4]);
        assert_eq!(told, vec![(1, 9), (2, 9), (3, 9)]);
    }

    #[test]
    fn with_no_failure_every_file_is_read_four_at_a_time_in_file_order() {
        let order: Vec<(usize, Option<&str>)> = vec![(1, None), (0, None), (3, None), (2, None), (4, None), (5, None)];
        let (result, started, told) = run(6, &order);
        assert_eq!(result, Ok(6));
        // Worker 1 finishes first and takes file 4, worker 0 next takes file 5.
        assert_eq!(started, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(told, (1..=6).map(|n| (n, 6)).collect::<Vec<_>>());
    }

    fn cache_keys(path: &std::path::Path) -> Vec<String> {
        let file: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        file["files"].as_object().unwrap().keys().cloned().collect()
    }

    /// The cache's `files` map is in the order entries were kept: a hit when it is asked, a read when it lands
    /// (spec history_store rule 12). TS trace: releasing 3, 1, 0, 2, 4, 5 keeps f3, f1, f0, f2, f4, f5; a second run
    /// with f3's stamp moved keeps the hits f0, f1, f2, f4, f5 as asked, then f3 when its read lands.
    #[test]
    fn the_cache_keeps_files_in_the_order_they_were_kept_across_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history-cache.json");
        let stamps: Vec<String> = (0..6).map(|i| format!("s{i}")).collect();
        let cache = file_history_cache(&path);
        let order: Vec<(usize, Option<&str>)> = [3, 1, 0, 2, 4, 5].iter().map(|i| (*i, None)).collect();
        let (result, _, _) = run_with(&stamps, Some(&cache), &order);
        assert_eq!(result, Ok(6));
        cache.save().unwrap();
        assert_eq!(cache_keys(&path), vec!["/f3", "/f1", "/f0", "/f2", "/f4", "/f5"]);

        let mut moved = stamps.clone();
        moved[3] = "changed".to_string();
        let again = file_history_cache(&path);
        let (result, started, told) = run_with(&moved, Some(&again), &[(3, None)]);
        assert_eq!(result, Ok(6));
        assert_eq!(started, vec![3]);
        assert_eq!(told, (1..=6).map(|n| (n, 6)).collect::<Vec<_>>());
        again.save().unwrap();
        assert_eq!(cache_keys(&path), vec!["/f0", "/f1", "/f2", "/f4", "/f5", "/f3"]);
    }

    /// A store that fails still leaves what its open reads set in the cache, and the save drops every file the run
    /// never reached. TS trace: release 2 (file 4 starts), fail 1, release 4, 0, 3: the store throws E1 and the saved
    /// keys are f2, f4, f0, f3.
    #[test]
    fn a_failed_store_keeps_the_reads_that_landed_and_drops_the_files_never_started() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history-cache.json");
        let stamps: Vec<String> = (0..9).map(|i| format!("s{i}")).collect();
        let cache = file_history_cache(&path);
        let (result, started, _) = run_with(&stamps, Some(&cache), &[(2, None), (1, Some("E1")), (4, None), (0, None), (3, None)]);
        assert_eq!(result, Err("E1".to_string()));
        assert_eq!(started, vec![0, 1, 2, 3, 4]);
        cache.save().unwrap();
        assert_eq!(cache_keys(&path), vec!["/f2", "/f4", "/f0", "/f3"]);
    }
}

/// Every case of tests/fixtures/oracle/w2/history_{commands,readers,store}.json (oracle/w2-history_*.oracle.ts): the
/// TS's own answers on the edges the three specs' wave-2 rules name.
mod w2_oracle {
    use super::*;
    use common::oracle::{cases, host, strs, w2};
    use wsp_catalog::catalog::HistoryFormat;
    use wsp_collect::history::cache::FileHistoryCache;
    use wsp_collect::history::claude::claude_call;
    use wsp_collect::history::codex::codex_call;
    use wsp_collect::history::commands::command_words;
    use wsp_collect::history::hermes::hermes_calls;
    use wsp_collect::history::history_reader;
    use wsp_collect::history::reader::Call;
    use wsp_collect::history::tally::{in_folders, install_key, installed_name, is_heavy, meets_used_floor, Usage};

    fn s(v: &Value) -> &str {
        v.as_str().unwrap()
    }

    fn opt_s(v: Option<&Value>) -> Option<&str> {
        v.and_then(Value::as_str)
    }

    fn arg_opt(case: &Value, i: usize) -> Option<&Value> {
        case["args"].get(i).filter(|v| !v.is_null())
    }

    fn call_json(c: &Call) -> Value {
        let mut o = serde_json::Map::new();
        o.insert("session".into(), json!(c.session));
        if let Some(f) = &c.folder {
            o.insert("folder".into(), json!(f));
        }
        if let Some(at) = c.at {
            o.insert("at".into(), json!(at));
        }
        match &c.kind {
            CallKind::Shell { line } => {
                o.insert("kind".into(), json!("shell"));
                o.insert("line".into(), json!(line));
            }
            CallKind::Other { name } => {
                o.insert("kind".into(), json!("other"));
                o.insert("name".into(), json!(name));
            }
        }
        Value::Object(o)
    }

    fn call_of(v: &Value) -> Call {
        let kind = if v["kind"] == "shell" {
            CallKind::Shell { line: s(&v["line"]).into() }
        } else {
            CallKind::Other { name: s(&v["name"]).into() }
        };
        Call { session: s(&v["session"]).into(), folder: v["folder"].as_str().map(str::to_string), at: v["at"].as_f64(), kind }
    }

    /// Numbers compare as numbers: the TS writes `5`, serde_json may hold `5.0`.
    fn same(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
            (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q)),
            (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w))),
            _ => a == b,
        }
    }

    fn assert_same(got: &Value, want: &Value, what: &str) {
        assert!(same(got, want), "{what}\n got: {got}\nwant: {want}");
    }

    fn bucket_of(v: &Value) -> HistoryBucket {
        serde_json::from_value(v.clone()).unwrap()
    }

    fn buckets_json(b: &[HistoryBucket]) -> Value {
        Value::Array(b.iter().map(|b| serde_json::to_value(b).unwrap()).collect())
    }

    fn usage_json(u: &Usage) -> Value {
        let m = |m: &IndexMap<String, Count>| {
            Value::Array(m.iter().map(|(k, c)| json!([k, { "sessions": c.sessions, "calls": c.calls }])).collect())
        };
        json!({ "sessions": u.sessions, "calls": u.calls, "commands": m(&u.commands), "installs": m(&u.installs), "tools": m(&u.tools) })
    }

    fn format_of(name: &str) -> HistoryFormat {
        match name {
            "claude" => HistoryFormat::ClaudeJsonl,
            "codex" => HistoryFormat::CodexRollout,
            _ => HistoryFormat::HermesSqlite,
        }
    }

    #[test]
    fn every_line_splits_names_and_installs_as_the_ts_did() {
        let f = w2("history_commands");
        for case in cases(&f, "without_heredocs") {
            assert_eq!(json!(without_heredocs(s(&case["args"][0]))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "split_commands") {
            assert_eq!(json!(split_commands(s(&case["args"][0]))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "command_words") {
            assert_eq!(json!(command_words(s(&case["args"][0]))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "command_names") {
            assert_eq!(json!(command_names(s(&case["args"][0]))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "install_names") {
            let got: Vec<Value> = install_names(s(&case["args"][0])).iter().map(|i| json!({ "via": i.via, "name": i.name })).collect();
            assert_eq!(Value::Array(got), case["out"], "{}", case["name"]);
        }
    }

    /// The forms V8's legacy `Date.parse` fallback reads and the ISO grammar does not; the coordinator ruled them an
    /// accepted difference (#2446, ruling on collect's spec pass, point 1): no agent writes one.
    const LEGACY_DATE_FORMS: [&str; 8] = [
        "2026-09-06 10:01:00Z",
        "Sun, 06 Sep 2026 10:01:00 GMT",
        "Sep 6 2026 10:01:00 UTC",
        "2026-09-06 10:01:00",
        "2026/09/06",
        "Sep 6, 2026",
        "6 Sep 2026",
        "12",
    ];

    /// Local-time cases (named `[local time, ...]`) ran at UTC in the oracle; they are checked only when this process
    /// runs at UTC too.
    #[test]
    fn readers_pure_functions_answer_what_the_ts_answered() {
        let f = w2("history_readers");
        let utc = std::env::var("TZ").map(|tz| tz == "UTC" || tz.is_empty()).unwrap_or(true);
        for case in cases(&f, "time_of") {
            if s(&case["name"]).starts_with("[local time") && !utc {
                continue;
            }
            if case["args"][0].as_str().is_some_and(|v| LEGACY_DATE_FORMS.contains(&v)) {
                continue;
            }
            let got = time_of(arg_opt(case, 0)).map(|n| json!(n)).unwrap_or(Value::Null);
            assert_same(&got, &case["out"], s(&case["name"]));
        }
        for case in cases(&f, "stem") {
            assert_eq!(json!(stem(s(&case["args"][0]))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "claude_session") {
            assert_eq!(json!(claude_session(s(&case["args"][0]))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "claude_call") {
            let got = claude_call(
                s(&case["args"][0]),
                s(&case["args"][1]),
                arg_opt(case, 2),
                opt_s(arg_opt(case, 3)),
                arg_opt(case, 4).and_then(Value::as_f64),
            );
            assert_same(&call_json(&got), &case["out"], s(&case["name"]));
        }
        for case in cases(&f, "codex_call") {
            let got = codex_call(
                s(&case["args"][0]),
                s(&case["args"][1]),
                arg_opt(case, 2),
                opt_s(arg_opt(case, 3)),
                arg_opt(case, 4).and_then(Value::as_f64),
            );
            assert_same(&call_json(&got), &case["out"], s(&case["name"]));
        }
        for case in cases(&f, "hermes_calls") {
            let got: Vec<Value> =
                hermes_calls(s(&case["args"][0]), arg_opt(case, 1), opt_s(arg_opt(case, 2))).iter().map(call_json).collect();
            assert_same(&Value::Array(got), &case["out"], s(&case["name"]));
        }
        for case in cases(&f, "holder") {
            let reader = history_reader(format_of(s(&case["args"][0])));
            assert_eq!(json!(reader.holder(s(&case["args"][1]), s(&case["args"][2]))), case["out"], "{}", case["name"]);
        }
    }

    #[test]
    fn readers_list_stamp_and_read_files_as_the_ts_did() {
        let f = w2("history_readers");
        for case in cases(&f, "stamp_of") {
            let h = host(&case["laptop"]);
            let got =
                block_on(stamp_of(&h, s(&case["args"][0]))).map(|f| json!({ "path": f.path, "stamp": f.stamp })).unwrap_or(Value::Null);
            assert_eq!(got, case["out"], "{}", case["name"]);
        }
        for (key, format) in [("claude_reader_files", "claude"), ("codex_reader_files", "codex"), ("hermes_reader_files", "hermes")] {
            for case in cases(&f, key) {
                let h = host(&case["laptop"]);
                let files = block_on(history_reader(format_of(format)).files(&h, s(&case["args"][0])));
                let got: Vec<Value> = files.iter().map(|f| json!({ "path": f.path, "stamp": f.stamp })).collect();
                assert_eq!(Value::Array(got), case["out"], "{key}: {}", case["name"]);
            }
        }
        for (key, format) in [("claude_reader_read", "claude"), ("codex_reader_read", "codex"), ("hermes_reader_read", "hermes")] {
            for case in cases(&f, key) {
                let h = host(&case["laptop"]);
                let reader = history_reader(format_of(format));
                let items: Vec<Result<Call, wsp_words::exit::HostError>> =
                    block_on(reader.read(&h, s(&case["args"][0]), s(&case["args"][1])).collect());
                let mut got = Vec::new();
                let mut error = None;
                for item in items {
                    match item {
                        Ok(c) => got.push(call_json(&c)),
                        Err(e) => {
                            error = Some(e.message);
                            break;
                        }
                    }
                }
                match case.get("error") {
                    Some(want) => assert_eq!(json!(error), *want, "{key}: {}", case["name"]),
                    None => {
                        assert_eq!(error, None, "{key}: {}", case["name"]);
                        assert_same(&Value::Array(got), &case["out"], &format!("{key}: {}", case["name"]));
                    }
                }
                assert_eq!(json!(h.calls()), case["calls"], "calls of {key}: {}", case["name"]);
            }
        }
    }

    #[test]
    fn tally_functions_answer_what_the_ts_answered() {
        let f = w2("history_store");
        for case in cases(&f, "reduce_calls") {
            let calls: Vec<Result<Call, wsp_words::exit::HostError>> =
                case["args"][0].as_array().unwrap().iter().map(|c| Ok(call_of(c))).collect();
            let got = block_on(reduce_calls(futures::stream::iter(calls).boxed_local())).map_err(|e| e.message).unwrap();
            assert_same(&buckets_json(&got), &case["out"], s(&case["name"]));
        }
        for case in cases(&f, "one_holder") {
            let held: Vec<Held> = case["args"][0]
                .as_array()
                .unwrap()
                .iter()
                .map(|h| Held { holder: s(&h["holder"]).into(), buckets: h["buckets"].as_array().unwrap().iter().map(bucket_of).collect() })
                .collect();
            assert_same(&buckets_json(&one_holder(&held)), &case["out"], s(&case["name"]));
        }
        for case in cases(&f, "usage_of") {
            let buckets: Vec<HistoryBucket> = case["args"][0].as_array().unwrap().iter().map(bucket_of).collect();
            let folders = arg_opt(case, 1).map(strs);
            assert_eq!(usage_json(&usage_of(&buckets, folders.as_deref())), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "in_folders") {
            assert_eq!(json!(in_folders(case["args"][0]["folder"].as_str(), &strs(&case["args"][1]))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "installed_name") {
            assert_eq!(json!(installed_name(s(&case["args"][0]))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "install_key") {
            let i = Install { via: s(&case["args"][0]["via"]).into(), name: s(&case["args"][0]["name"]).into() };
            assert_eq!(json!(install_key(&i)), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "meets_used_floor") {
            let c = Count { sessions: case["args"][0]["sessions"].as_u64().unwrap(), calls: case["args"][0]["calls"].as_u64().unwrap() };
            assert_eq!(json!(meets_used_floor(c, arg_opt(case, 1).and_then(Value::as_u64))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "is_heavy") {
            assert_eq!(json!(is_heavy(arg_opt(case, 0).and_then(Value::as_u64))), case["out"], "{}", case["name"]);
        }
    }

    /// The cache file before and after, with `<dir>` this test's own folder. Text is compared parsed: the key order of
    /// a bucket the TS kept from the cache is an accepted difference (spec history_store Decisions 1).
    struct CacheCase {
        _dir: tempfile::TempDir,
        root: String,
        path: std::path::PathBuf,
    }

    impl CacheCase {
        fn new(c: &Value) -> CacheCase {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().to_str().unwrap().to_string();
            let path = std::path::PathBuf::from(s(&c["path"]).replace("<dir>", &root));
            if let Some(before) = c["before"].as_str() {
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(&path, before).unwrap();
            }
            CacheCase { _dir: dir, root, path }
        }

        fn check(&self, c: &Value, what: &str) {
            let after = std::fs::read_to_string(&self.path).ok();
            match c["after"].as_str() {
                None => assert_eq!(after, None, "{what}: no cache file"),
                Some(want) => {
                    let got = after.unwrap_or_else(|| panic!("{what}: a cache file"));
                    assert!(got.ends_with('\n') == want.ends_with('\n'), "{what}: the trailing newline");
                    match (serde_json::from_str::<Value>(&got), serde_json::from_str::<Value>(want)) {
                        (Ok(g), Ok(w)) => assert_same(&g, &w, what),
                        _ => assert_eq!(got, want, "{what}"),
                    }
                }
            }
            for (dir, exists) in c["dirs_after"].as_object().into_iter().flatten() {
                let p = dir.replace("<dir>", &self.root);
                assert_eq!(std::path::Path::new(&p).is_dir(), exists.as_bool().unwrap(), "{what}: folder {p}");
            }
            let tmp: Vec<_> =
                std::fs::read_dir(&self.root).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".tmp")).collect();
            assert!(tmp.is_empty(), "{what}: a temp file left behind");
        }
    }

    #[test]
    fn the_cache_reads_answers_and_writes_what_the_ts_did() {
        for case in cases(&w2("history_store"), "file_history_cache") {
            let what = s(&case["name"]);
            let cc = CacheCase::new(&case["cache"]);
            let cache: FileHistoryCache = file_history_cache(&cc.path);
            for (op, want) in case["ops"].as_array().unwrap().iter().zip(case["outs"].as_array().unwrap()) {
                let a = &op["args"];
                match s(&op["op"]) {
                    "get" => {
                        let got = cache.get(s(&a[0]), s(&a[1])).map(|b| buckets_json(&b)).unwrap_or(Value::Null);
                        assert_same(&got, want, &format!("{what}: get {a}"));
                    }
                    "set" => cache.set(s(&a[0]), s(&a[1]), &a[2].as_array().unwrap().iter().map(bucket_of).collect::<Vec<_>>()),
                    _ => cache.save().unwrap(),
                }
            }
            cc.check(&case["cache"], what);
        }
    }

    #[test]
    fn read_histories_says_counts_tells_and_caches_what_the_ts_did() {
        let all = catalog_agents();
        for case in cases(&w2("history_store"), "read_histories") {
            let what = s(&case["name"]);
            let h = host(&case["laptop"]);
            let ids = strs(&case["args"][0]);
            let agents: Vec<&wsp_catalog::catalog::AgentEntry> = ids.iter().map(|id| *all.iter().find(|a| a.id == *id).unwrap()).collect();
            let folders = case["args"][1].get("folders").map(strs);
            let told = RefCell::new(Vec::new());
            let progress = RefCell::new(Vec::new());
            let on_agent = |a: &wsp_collect::history::AgentHistory| {
                told.borrow_mut().push(format!("{} {}", a.agent, serde_json::to_value(a.state).unwrap().as_str().unwrap()))
            };
            let on_progress =
                |p: wsp_collect::history::HistoryProgress| progress.borrow_mut().push(format!("{} {}/{}", p.agent, p.read, p.files));
            let cc = case.get("cache").map(CacheCase::new);
            let cache = cc.as_ref().map(|c| file_history_cache(&c.path));
            let opts = HistoryOptions {
                on_agent: Some(&on_agent),
                folders: folders.as_deref(),
                cache: cache.as_ref().map(|c| c as &dyn HistoryCache),
                on_progress: Some(&on_progress),
            };
            let got = block_on(read_histories(&h, &agents, opts)).unwrap();
            let got: Vec<Value> = got
                .iter()
                .map(|a| json!({ "agent": a.agent, "state": a.state, "sessions": a.sessions, "calls": a.calls, "usage": usage_json(&a.usage) }))
                .collect();
            assert_eq!(Value::Array(got), case["out"], "{what}");
            assert_eq!(json!(told.into_inner()), case["told"], "told of {what}");
            assert_eq!(json!(progress.into_inner()), case["progress"], "progress of {what}");
            if !case["calls"].is_null() {
                assert_eq!(json!(h.calls()), case["calls"], "calls of {what}");
            }
            if let Some(cc) = &cc {
                cc.check(&case["cache"], what);
            }
        }
    }
}
