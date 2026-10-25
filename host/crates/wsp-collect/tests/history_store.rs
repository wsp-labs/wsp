// SPDX-License-Identifier: AGPL-3.0-only
//! The reads over every agent's store, and the cache that keeps what each session file came to. Port of the
//! `readHistories` and `the session cache` blocks of packages/collect/test/history.test.ts.
mod common;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use common::fake_host::*;
use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use futures::StreamExt;
use indexmap::IndexMap;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use wsp_catalog::catalog::{catalog_agents, AgentEntry};
use wsp_collect::history::cache::file_history_cache;
use wsp_collect::history::reader::PARSE_VERSION;
use wsp_collect::history::tally::Count;
use wsp_collect::history::{read_histories, AgentHistory, HistoryOptions};
use wsp_collect::host::{Host, HostFs, Stat};
use wsp_wire::views::golden_image::RecipeHistoryState;

fn claude_line(session_id: &str, tools: &[(&str, Value)]) -> String {
    let content: Vec<Value> =
        tools.iter().map(|(name, input)| json!({ "type": "tool_use", "id": "toolu_1", "name": name, "input": input })).collect();
    json!({ "type": "assistant", "cwd": "/Users/dev/proj", "sessionId": session_id, "message": { "role": "assistant", "content": content } }).to_string()
}

fn state_word(s: RecipeHistoryState) -> &'static str {
    match s {
        RecipeHistoryState::Read => "read",
        RecipeHistoryState::Empty => "empty",
        RecipeHistoryState::Unreadable => "unreadable",
        RecipeHistoryState::NoReader => "no-reader",
    }
}

fn entries(m: &IndexMap<String, Count>) -> Vec<(String, Count)> {
    m.iter().map(|(k, v)| (k.clone(), *v)).collect()
}

fn rows(r: &[(&str, u64, u64)]) -> Vec<(String, Count)> {
    r.iter().map(|(k, sessions, calls)| (k.to_string(), Count { sessions: *sessions, calls: *calls })).collect()
}

/// `CATALOG_AGENTS.filter(a => a.id === "claude")`.
fn claude_agents() -> Vec<&'static AgentEntry> {
    catalog_agents().into_iter().filter(|a| a.id == "claude").collect()
}

/// What `fs.lines` does for a path ending in `/s3.jsonl`: the iterator throws, as a disk that fails part way would.
struct FailingFs {
    inner: Rc<dyn HostFs>,
}

impl HostFs for FailingFs {
    fn stat<'a>(&'a self, path: &'a str) -> LocalBoxFuture<'a, Option<Stat>> {
        self.inner.stat(path)
    }

    fn list<'a>(&'a self, dir: &'a str) -> LocalBoxFuture<'a, Vec<String>> {
        self.inner.list(dir)
    }

    fn read_text<'a>(&'a self, path: &'a str) -> LocalBoxFuture<'a, Option<String>> {
        self.inner.read_text(path)
    }

    fn walk<'a>(&'a self, dir: &'a str) -> LocalBoxFuture<'a, Vec<String>> {
        self.inner.walk(dir)
    }

    fn lines<'a>(&'a self, path: &'a str, holding: Option<&'a str>) -> LocalBoxStream<'a, std::io::Result<String>> {
        if path.ends_with("/s3.jsonl") {
            futures::stream::iter(vec![Err(std::io::Error::other("EIO"))]).boxed_local()
        } else {
            self.inner.lines(path, holding)
        }
    }
}

mod read_histories {
    use super::*;

    #[test]
    fn ends_a_store_at_a_file_it_cannot_read_no_file_of_that_store_is_read_or_told_afte() {
        let mut laptop = FakeLaptop::new();
        for i in 0..12 {
            laptop = laptop.file(
                &format!("~/.claude/projects/-Users-dev-proj/s{i}.jsonl"),
                claude_line(&format!("s{i}"), &[("Read", json!({ "file_path": "/x" }))]),
            );
        }
        let host = fake_host(laptop);
        let mut failing: Host = host.host.clone();
        failing.fs = Rc::new(FailingFs { inner: host.host.fs.clone() });
        let heard: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let on_agent = |h: &AgentHistory| heard.borrow_mut().push(format!("{} {}", h.agent, state_word(h.state)));
        let on_progress = |p: wsp_collect::history::HistoryProgress| heard.borrow_mut().push(format!("{} {}/{}", p.agent, p.read, p.files));
        let agents = claude_agents();
        block_on(read_histories(
            &failing,
            &agents,
            HistoryOptions { on_agent: Some(&on_agent), on_progress: Some(&on_progress), ..Default::default() },
        ))
        .unwrap();
        assert_eq!(heard.borrow().last().map(String::as_str), Some("claude unreadable"));
    }

    #[test]
    fn names_every_catalog_agent_read_with_counts_empty_unreadable_or_no_reader_for_its() {
        let host = fake_host(
            FakeLaptop::new()
                .file("~/.claude/projects/-Users-dev-proj/s1.jsonl", claude_line("s1", &[("Bash", json!({ "command": "gh auth status" }))]))
                .file("~/.hermes/state.db", 4096),
        );
        let agents = catalog_agents();
        let out = block_on(read_histories(&host, &agents, HistoryOptions::default())).unwrap();
        assert_eq!(out.iter().map(|h| h.agent.clone()).collect::<Vec<_>>(), agents.iter().map(|a| a.id.clone()).collect::<Vec<_>>());
        for h in &out {
            let stood = match h.agent.as_str() {
                "claude" => ("read", 1, 1),
                "codex" => ("empty", 0, 0),
                "hermes" => ("unreadable", 0, 0),
                _ => ("no-reader", 0, 0),
            };
            assert_eq!((state_word(h.state), h.sessions, h.calls), stood, "{}", h.agent);
        }
    }
}

mod the_session_cache {
    use super::*;

    const S1: &str = "~/.claude/projects/-Users-dev-proj/s1.jsonl";
    const S2: &str = "~/.claude/projects/-Users-dev-proj/s2.jsonl";

    fn first() -> String {
        [claude_line("s1", &[("Bash", json!({ "command": "gh pr view" }))]), claude_line("s1", &[("Read", json!({ "file_path": "/x" }))])]
            .join("\n")
    }

    fn second() -> String {
        claude_line("s2", &[("Bash", json!({ "command": "pnpm test && npm install -g agent-browser" }))])
    }

    fn laptop() -> FakeLaptop {
        FakeLaptop::new().file(S1, first()).file(S2, second())
    }

    fn grown() -> FakeLaptop {
        FakeLaptop::new()
            .file(S1, first())
            .file(S2, format!("{}\n{}", second(), claude_line("s2", &[("Bash", json!({ "command": "cargo build" }))])))
    }

    fn session_path(name: &str) -> String {
        format!("/Users/dev/.claude/projects/-Users-dev-proj/{name}.jsonl")
    }

    /// The session files a run opened, in order; the fake host records every read of a transcript.
    fn opened(host: &FakeHost) -> Vec<String> {
        host.calls().iter().filter_map(|c| c.strip_prefix("lines ").map(str::to_string)).collect()
    }

    /// A folder that lives as long as the test, and the cache path inside it.
    fn cache_path() -> (tempfile::TempDir, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("history-cache.json");
        (d, p)
    }

    fn run(host: &FakeHost, cache: &Path) -> Vec<AgentHistory> {
        let c = file_history_cache(cache);
        let agents = claude_agents();
        block_on(read_histories(host, &agents, HistoryOptions { cache: Some(&c), ..Default::default() })).unwrap()
    }

    #[test]
    fn a_second_run_over_an_unchanged_history_opens_no_session_file_and_comes_to_the_sa() {
        let (_dir, cache) = cache_path();
        let first = fake_host(laptop());
        let one = run(&first, &cache);
        assert_eq!(opened(&first), vec![session_path("s1"), session_path("s2")]);
        assert_eq!(one[0].state, RecipeHistoryState::Read);
        assert_eq!(one[0].sessions, 2);
        assert_eq!(one[0].calls, 3);

        let again = fake_host(laptop());
        let two = run(&again, &cache);
        assert_eq!(opened(&again), Vec::<String>::new());
        assert_eq!(two, one);
    }

    #[test]
    fn re_reads_the_one_file_whose_stamp_moved_whether_its_time_or_its_size_changed_and() {
        let (_dir, cache) = cache_path();
        run(&fake_host(laptop()), &cache);

        // Same bytes, later mtime: the file is read again and the counts stand.
        let touched = fake_host(laptop().mtime(S2, MTIME + 1.0));
        let after = run(&touched, &cache);
        assert_eq!(opened(&touched), vec![session_path("s2")]);
        assert_eq!(after[0].sessions, 2);
        assert_eq!(after[0].calls, 3);

        // Same mtime, more lines: the size says it moved, and the new command is counted.
        let grown = fake_host(grown());
        let last = run(&grown, &cache);
        assert_eq!(opened(&grown), vec![session_path("s2")]);
        assert_eq!(last[0].sessions, 2);
        assert_eq!(last[0].calls, 4);
        assert_eq!(entries(&last[0].usage.commands), rows(&[("cargo", 1, 1), ("gh", 1, 1), ("npm", 1, 1), ("pnpm", 1, 1)]));
    }

    #[test]
    fn counts_every_session_file_off_as_it_lands_so_a_first_run_can_be_counted_out_loud() {
        let progress: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let on_progress =
            |p: wsp_collect::history::HistoryProgress| progress.borrow_mut().push(format!("{} {}/{}", p.agent, p.read, p.files));
        let agents = claude_agents();
        block_on(read_histories(&fake_host(laptop()), &agents, HistoryOptions { on_progress: Some(&on_progress), ..Default::default() }))
            .unwrap();
        assert_eq!(*progress.borrow(), vec!["claude 1/2".to_string(), "claude 2/2".to_string()]);
    }

    #[test]
    fn drops_a_session_file_that_is_gone_from_the_cache_and_reads_everything_again_when() {
        let (_dir, cache) = cache_path();
        run(&fake_host(laptop()), &cache);
        let cleared = FakeLaptop::new().file(S1, first());
        run(&fake_host(cleared), &cache);
        let stored: Value = serde_json::from_str(&std::fs::read_to_string(&cache).unwrap()).unwrap();
        let files: Vec<String> = stored["files"].as_object().unwrap().keys().cloned().collect();
        assert_eq!(files, vec![session_path("s1")]);

        std::fs::write(&cache, "{ half a file").unwrap();
        let cold = fake_host(laptop());
        let out = run(&cold, &cache);
        assert_eq!(opened(&cold).len(), 2);
        assert_eq!(out[0].sessions, 2);
        assert_eq!(out[0].calls, 3);
    }

    #[test]
    fn reads_everything_again_when_another_parse_version_filled_the_file_so_no_run_hand() {
        let (_dir, cache) = cache_path();
        run(&fake_host(laptop()), &cache);
        let mut written: Value = serde_json::from_str(&std::fs::read_to_string(&cache).unwrap()).unwrap();
        assert_eq!(written["parse"], json!(PARSE_VERSION));

        // The same stamps, the same shape, one parse version on: the stamps say nothing moved and the file is still refused whole.
        written["parse"] = json!(PARSE_VERSION + 1);
        std::fs::write(&cache, written.to_string()).unwrap();
        let cold = fake_host(laptop());
        let out = run(&cold, &cache);
        assert_eq!(opened(&cold), vec![session_path("s1"), session_path("s2")]);
        assert_eq!(out[0].sessions, 2);
        assert_eq!(out[0].calls, 3);
    }

    #[test]
    fn puts_the_new_cache_in_place_with_a_rename_so_a_second_wsp_over_one_state_folder() {
        let (_dir, cache) = cache_path();
        run(&fake_host(laptop()), &cache);
        let before = std::fs::read_to_string(&cache).unwrap();
        // What another process holding the old file sees: an in-place write moves under it, a rename leaves it whole.
        let held = PathBuf::from(format!("{}.held", cache.display()));
        std::fs::hard_link(&cache, &held).unwrap();

        run(&fake_host(grown()), &cache);
        assert_ne!(std::fs::read_to_string(&cache).unwrap(), before);
        assert_eq!(std::fs::read_to_string(&held).unwrap(), before);
    }

    /// PARSE_VERSION and what the files that fill a bucket hashed to when it was last weighed against them.
    #[test]
    fn bumps_parse_version_when_the_code_that_fills_a_bucket_changes() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/history");
        let mut h = Sha256::new();
        for f in ["claude.rs", "codex.rs", "commands.rs", "hermes.rs", "reader.rs", "tally.rs"] {
            h.update(std::fs::read(src.join(f)).unwrap());
        }
        let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
        // Bump PARSE_VERSION in reader.rs and put the digest this prints in tests/fixtures/parse-fingerprint.txt: every cache filled by the old code is then re-read.
        let fingerprint =
            std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parse-fingerprint.txt")).unwrap();
        assert_eq!(format!("{PARSE_VERSION}:{}", &hex[..16]), fingerprint.trim());
    }
}
