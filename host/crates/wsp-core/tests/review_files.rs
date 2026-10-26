// SPDX-License-Identifier: AGPL-3.0-only
//! Review probes for the file and record readers and writers of wsp-core, against the TS oracle. Only the tests that
//! fail on 11db4a4e6. Paste into host/crates/wsp-core/tests/review_files.rs.
mod rest;
mod support;

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use futures::FutureExt;
use indexmap::IndexMap;
use rest::*;
use serde_json::{json, Value};
use support::{run_local, settle, MemStore};
use wsp_core::devices::{make_devices, DEVICES, PAIRINGS};
use wsp_core::host_id::{host_identity, local_config_dir};
use wsp_core::ids::OsIds;
use wsp_core::project_landing::*;
use wsp_core::project_sources::{project_source, ProjectSourceModule};
use wsp_core::types::internal::*;
use wsp_core::types::pending::Store;
use wsp_engine::Machine;
use wsp_wire::views::session_events::SessionEvent;
use wsp_wire::views::workspace::ProjectView;
use wsp_words::exit::HostError;

static ENV: Mutex<()> = Mutex::new(());

fn src(kind: &str) -> Rc<dyn ProjectSourceModule> {
    project_source(kind).unwrap()
}

// ---- landing: a machine that answers by needle, and a road over it ----

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
    async fn snapshot(
        &self,
        n: &str,
        l: wsp_engine::machine::MachineLife,
        o: wsp_engine::machine::SnapshotOptions,
    ) -> wsp_engine::errors::Result<String> {
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

fn deps(m: Rc<RecMachine>, now: Rc<dyn Fn() -> i64>) -> LandingDeps {
    LandingDeps {
        land: Rc::new(|_m: Arc<dyn Machine>, _p: String, _b: Vec<u8>| async { Ok::<(), HostError>(()) }.boxed_local()),
        scratch: Rc::new(|_m: &dyn Machine| "/scratch".to_string()),
        computer: Some(LandingComputer { machine: Arc::new(Shared(m)), home: "/root".into() }),
        mac_state_home: "/Users/dev/.claude".into(),
        branch_here: Rc::new(|_p: &str| async { None }.boxed_local()),
        computer_name: "hetzner".into(),
        clone_env: None,
        now,
    }
}

fn project(name: &str) -> ProjectView {
    wire(&json!({ "id": "pr_1", "name": name, "computer": "p", "source": { "kind": "git", "url": "https://github.com/acme/lab.git" }, "path": "", "remote": "https://github.com/acme/lab.git", "defaultBranch": "main", "memoryKey": "", "memoryDir": "", "createdAt": "" }))
}

fn request(name: &str) -> LandRequest {
    LandRequest { project: project(name), source: src("git"), seed: None, report: Rc::new(|_, _| {}) }
}

// F1 BLOCKER: TS project-landing.ts:636
#[test]
fn last_line_keeps_the_words_as_the_machine_said_them() {
    assert_eq!(last_line("fatal:  a   b\tc\n"), Some("fatal:  a   b\tc".to_string()));
}

// F1 BLOCKER: TS project-landing.ts:636
#[test]
fn a_project_named_with_two_spaces_is_swept_where_it_was_made() {
    run_local(async {
        let m = RecMachine::new(vec![("for n in", result(0, "/root/my  app\n", "")), ("git clone", result(1, "", "boom"))]);
        let d = deps(m.clone(), Rc::new(|| 0));
        let road = project_landing("box").unwrap();
        let err = road.land(request("my  app"), &d).await.unwrap_err();
        assert_eq!(err.message, "boom");
        let swept: Vec<String> = m.cmds().into_iter().filter(|c| c.starts_with("rm -rf -- ")).collect();
        assert_eq!(swept, vec!["rm -rf -- '/root/my  app'".to_string()]);
    });
}

// F1 BLOCKER: TS project-landing.ts:636
#[test]
fn unsaved_of_keeps_the_inner_whitespace_of_what_the_machine_said() {
    let got = unsaved_of("my  proj", &result(1, "", "fatal:  a   b\tc\n"));
    assert_eq!(got.as_deref(), Some("my  proj: could not read what is not pushed: fatal:  a   b\tc"));
}

// F2 MAJOR: TS status.ts:808
#[test]
fn stored_cost_points_carry_the_type_key_the_ts_wrote() {
    use support::status::*;
    run_local(async {
        let roads = Rc::new(Roads::default());
        let r = rig();
        let v = view("ws_1", "m_1", "running");
        let mut rec = record(v.clone(), &roads);
        rec.rate_usd_per_hour = 0.5;
        r.put(rec);
        let _t = r.tracker(Default::default());
        r.bus.fire(&created(&v));
        settle().await;
        let doc: Value = serde_json::from_str(&r.store.text(wsp_core::status::COST_HISTORIES, "ws_1").expect("a stored document")).unwrap();
        let first = doc["points"][0].as_object().unwrap();
        assert_eq!(first.keys().next().map(String::as_str), Some("type"), "TS points are {{type, workspaceId, phase, ...}}: {first:?}");
        assert_eq!(first["type"], "workspace.cost");
    });
}

// F3 MAJOR: TS host-id.ts:14
#[test]
fn local_config_dir_without_home_asks_the_passwd_file() {
    let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let (home, xdg) = (std::env::var_os("HOME"), std::env::var_os("XDG_CONFIG_HOME"));
    std::env::remove_var("HOME");
    std::env::remove_var("XDG_CONFIG_HOME");
    let got = local_config_dir();
    if let Some(h) = home {
        std::env::set_var("HOME", h);
    }
    if let Some(x) = xdg {
        std::env::set_var("XDG_CONFIG_HOME", x);
    }
    assert!(got.is_absolute(), "node's homedir() falls back to the passwd entry (/root here); got {got:?}");
}

// F4 MINOR: TS types/internal.ts:512
#[test]
fn index_read_takes_a_lone_surrogate_the_ts_wrote() {
    // What indexBytes writes for a first line of 199 x then an emoji: firstOf cuts at 200 units, inside the pair.
    let text = r#"{"words":[["t2",{"lines":["xx😀tail"],"last":1,"size":8,"first":"xx\ud83d"}]],"starts":[["t2","s2"]],"cut":[],"facts":[["s2",{}]],"taken":[],"children":[],"trimmed":[],"pos":0}"#;
    assert!(index_read(text.as_bytes()).is_some(), "index_read refused what the TS's indexBytes wrote");
}

// ---- the transcript index as bytes ----

fn events() -> Vec<SessionEvent> {
    let usage = json!({"tokens": {"input": 10, "output": 2, "cached": 0, "cacheWrite": 0, "reasoning": 0}, "toolUses": 3, "durationMs": 50});
    let base = |v: Value| {
        let mut o = json!({"workspaceId": "w1"});
        for (k, x) in v.as_object().unwrap() {
            o[k] = x.clone();
        }
        serde_json::from_value::<SessionEvent>(o).unwrap()
    };
    vec![
        base(json!({"type": "session.start", "sessionId": "s1", "threadId": "t1", "turnId": "u1", "model": "m", "requestId": "r1", "at": 1, "prompt": "hi"})),
        base(json!({"type": "session.start", "sessionId": "s1", "threadId": "t1", "turnId": "u2", "cwd": "/x", "at": 2})),
        base(json!({"type": "session.subagent", "sessionId": "s1", "threadId": "t1", "turnId": "u2", "task": "k", "state": "running", "title": "T", "at": 5, "usage": usage})),
        base(json!({"type": "session.subagent", "sessionId": "s1", "threadId": "t1", "turnId": "u2", "task": "k", "state": "done", "summary": "all fine\nok", "at": 9})),
        base(json!({"type": "session.subagent", "sessionId": "s1", "threadId": "t1", "turnId": "u2", "task": "j", "state": "running", "title": "J", "at": 6})),
        base(json!({"type": "session.subagent", "sessionId": "s1", "threadId": "t1", "turnId": "u2", "task": "j", "state": "failed", "summary": "bad thing\nmore", "at": 10})),
        base(json!({"type": "session.subagent", "sessionId": "s1", "threadId": "t1", "turnId": "u2", "task": "r", "state": "running", "title": "R", "at": 7, "depth": 1, "model": "haiku", "asked": "look", "usage": usage})),
        base(json!({"type": "session.end", "sessionId": "s1", "threadId": "t1", "turnId": "u2", "exitCode": 0, "sawResult": true, "at": 11})),
    ]
}

// F5 MINOR: TS types/internal.ts:396
#[test]
fn index_bytes_match_ts() {
    let want = r#"{"words":[["t1",{"lines":["hi"],"last":11,"size":2,"first":"hi"}]],"starts":[["t1","s1"]],"cut":[["t1",false]],"facts":[["s1",{"model":"m","cwd":"/x"}]],"taken":[["r1",{"sessionId":"s1","threadId":"t1","turnId":"u1","outcome":"started"}]],"children":[["t1",[["k",{"id":"k","title":"T","state":"done","usage":{"tokens":{"input":10,"output":2,"cached":0,"cacheWrite":0,"reasoning":0},"toolUses":3,"durationMs":50},"startedAt":5,"turnId":"u2","startRow":5,"endedAt":9,"lastLine":"ok"}],["j",{"id":"j","title":"J","state":"failed","startedAt":6,"turnId":"u2","startRow":6,"endedAt":10,"failure":"bad thing"}],["r",{"id":"r","title":"R","state":"stopped","depth":1,"model":"haiku","asked":"look","usage":{"tokens":{"input":10,"output":2,"cached":0,"cacheWrite":0,"reasoning":0},"toolUses":3},"startedAt":7,"turnId":"u2","startRow":7,"endedAt":11}]]]],"trimmed":[],"pos":0}"#;
    let index = index_of(&events());
    let got = String::from_utf8(index_bytes(&index, None, true)).unwrap();
    assert_eq!(got, want);
}

// F6 MINOR: TS devices.ts:259
#[test]
fn a_seen_keeps_every_key_the_row_held() {
    run_local(async {
        let store = MemStore::new();
        let door = make_devices(store.clone(), Rc::new(OsIds));
        store.seed(DEVICES, "d_1", json!({"id": "d_1", "tokenHash": "h".repeat(64), "future": {"a": 1}, "lastSeenAt": "x", "name": "n", "createdAt": "2026-01-01T00:00:00.000Z", "extra": 7}));
        door.seen("d_1", 5000).await.unwrap();
        let kept: Value = serde_json::from_str(&store.text(DEVICES, "d_1").unwrap()).unwrap();
        assert_eq!(
            kept,
            json!({"id": "d_1", "tokenHash": "h".repeat(64), "future": {"a": 1}, "lastSeenAt": "1970-01-01T00:00:05.000Z", "name": "n", "createdAt": "2026-01-01T00:00:00.000Z", "extra": 7}),
            "seen rewrites the row with every key it held"
        );
    });
}

// F7 MINOR: TS devices.ts:85
#[test]
fn a_pairing_whose_expiry_is_fractional_is_a_code() {
    run_local(async {
        let store = MemStore::new();
        let door = make_devices(store.clone(), Rc::new(OsIds));
        store.seed(PAIRINGS, "FRAC", json!({"code": "FRAC", "expiresAt": 1500.5}));
        assert!(door.redeem("FRAC", "n", 1000).await.unwrap().is_some(), "isPairing takes any number");
    });
}

// ---- host id ----

fn id_of(tag: &str, bytes: &[u8]) -> String {
    let dir = std::env::temp_dir().join(format!("rev-hid-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("host-id"), bytes).unwrap();
    let id = host_identity(Some(&dir));
    std::fs::remove_dir_all(&dir).ok();
    id.find(':').map_or(id.clone(), |i| id[i + 1..].to_string())
}

// F8 MINOR: TS host-id.ts:24
#[test]
fn host_identity_trims_a_byte_order_mark() {
    assert_eq!(id_of("bom", "\u{feff}abcd1234\n".as_bytes()), "abcd1234", "JS trim() takes U+FEFF");
}

// F8 MINOR: TS host-id.ts:24
#[test]
fn host_identity_does_not_trim_a_next_line_mark() {
    assert_eq!(id_of("nel", "\u{85}abcd1234\n".as_bytes()), "\u{85}abcd1234", "JS trim() leaves U+0085");
}

// F8 MINOR: TS host-id.ts:24
#[test]
fn host_identity_reads_a_file_that_is_not_utf8_with_replacements() {
    assert_eq!(id_of("bad", &[0x61, 0x62, 0xff, 0xfe, 0x63, 0x64, 0x0a]), "ab\u{fffd}\u{fffd}cd", "readFileSync(path, \"utf8\") replaces what does not decode");
}

// F9 MINOR: TS host-id.ts:12
#[test]
fn local_config_dir_joins_as_node_does() {
    let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let xdg = std::env::var_os("XDG_CONFIG_HOME");
    std::env::set_var("XDG_CONFIG_HOME", "/a/b/../c/");
    let got = local_config_dir();
    match xdg {
        Some(x) => std::env::set_var("XDG_CONFIG_HOME", x),
        None => std::env::remove_var("XDG_CONFIG_HOME"),
    }
    assert_eq!(got, PathBuf::from("/a/c/wsp"));
}

// F10 MINOR: TS project-landing.ts:435
#[test]
fn install_seconds_round_as_math_round_does_when_the_clock_stepped_back() {
    run_local(async {
        let m = RecMachine::new(vec![("for n in", result(0, "/root/lab\n", "")), ("ls -A", result(0, "package-lock.json\n", ""))]);
        let calls = Rc::new(Cell::new(0usize));
        let c = calls.clone();
        let now: Rc<dyn Fn() -> i64> = Rc::new(move || {
            let i = c.get();
            c.set(i + 1);
            [10_000, 8_500, 8_500, 8_500][i.min(3)]
        });
        let d = deps(m, now);
        let landed = project_landing("box").unwrap().land(request("lab"), &d).await.unwrap();
        let seconds = landed.installed.expect("an install ran").seconds.0;
        assert_eq!(seconds, -1.0, "Math.round(-1.5) is -1");
    });
}

// ---- find_session_file ----

fn jsonl_dir(tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!("rev-fsf-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for n in ["m", "z", "a", "q", "b", "k", "c", "y", "d", "x"] {
        std::fs::write(dir.join(format!("{n}.jsonl")), "x").unwrap();
    }
    dir.to_string_lossy().into_owned()
}

// F11 MINOR: TS types/internal.ts:700
#[test]
fn find_session_file_answers_the_first_name_in_sorted_order() {
    let d = jsonl_dir("sorted");
    let found = find_session_file(&d, ".jsonl", 0);
    std::fs::remove_dir_all(&d).ok();
    assert_eq!(found, Some(format!("{d}/a.jsonl")), "TS readdirSync lists names in sorted order, so a.jsonl wins");
}

// F11 MINOR: TS types/internal.ts:700
#[test]
fn find_session_file_joins_as_node_does() {
    let dir = std::env::temp_dir().join(format!("rev-fsf-join-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.jsonl"), "x").unwrap();
    let d = dir.to_string_lossy().into_owned();
    let found = find_session_file(&format!("{d}//"), ".jsonl", 0);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(found, Some(format!("{d}/a.jsonl")), "TS join normalises the doubled slash");
}

// F12 MINOR: TS recipe-sync.ts:20
#[test]
fn recipe_changes_list_rows_in_js_key_order() {
    use wsp_core::recipe_sync::recipe_changes;
    use wsp_wire::types::recipe_file::RecipeFile;
    let text = r#"{"name":"r","skills":{"b":{"from":"x"},"1":{"from":"y"}},"folders":{"zed":{"from":"a"},"7":{"from":"b"}}}"#;
    let after: RecipeFile = serde_json::from_str(text).unwrap();
    let keys: Vec<String> = recipe_changes(None, None, &after, &IndexMap::new()).into_iter().map(|c| c.key).collect();
    assert_eq!(keys, ["skills/1", "skills/b", "folders/7", "folders/zed"], "JSON.parse and Object.entries list integer-like names first");
}

// F13 NIT: TS project-landing.ts:301
#[test]
fn unsaved_of_splits_counts_on_js_whitespace_only() {
    let got = unsaved_of("p", &result(0, "1\u{85} 2 3\n", ""));
    assert_eq!(got.as_deref(), Some("p: could not read what is not pushed: the read printed \"1\u{85} 2 3\""), "U+0085 is not \\s in JS");
}

// F13 NIT: TS project-landing.ts:301
#[test]
fn unsaved_of_writes_a_count_past_i64_as_js_does() {
    let got = unsaved_of("p", &result(0, "1e30 2 3\n", ""));
    assert_eq!(got.as_deref(), Some("p holds 1e+30 commits not pushed, 2 uncommitted files and 3 stashes"));
}

// ---- the stub ----

struct SetupStore(MemStore);

#[async_trait::async_trait(?Send)]
impl wsp_store::store::Store for SetupStore {
    async fn get(&self, c: &str, id: &str) -> Result<Option<Value>, HostError> {
        self.0.get(c, id).await
    }
    async fn put(&self, c: &str, id: &str, v: Value) -> Result<(), HostError> {
        self.0.put(c, id, v).await
    }
    async fn list(&self, c: &str) -> Result<Vec<Value>, HostError> {
        self.0.list(c).await
    }
    async fn keys(&self, c: &str) -> Result<Vec<String>, HostError> {
        self.0.keys(c).await
    }
    async fn delete(&self, c: &str, id: &str) -> Result<(), HostError> {
        self.0.delete(c, id).await
    }
    async fn get_blob(&self, _c: &str, _id: &str) -> Result<Option<Vec<u8>>, HostError> {
        unreachable!()
    }
    async fn put_blob(&self, _c: &str, _id: &str, _b: Vec<u8>) -> Result<(), HostError> {
        unreachable!()
    }
    async fn delete_blob(&self, _c: &str, _id: &str) -> Result<(), HostError> {
        unreachable!()
    }
    async fn stat_blob(&self, _c: &str, _id: &str) -> Result<Option<wsp_store::store::BlobMark>, HostError> {
        unreachable!()
    }
}

// F14 STUB: TS agent-setup.ts:87 (wsp-adapters landmines.rs:17 is todo!())
#[test]
fn a_claude_setup_with_an_env_variable_reads_into_a_launch() {
    run_local(async {
        let store = MemStore::new();
        store.seed("agentSetups", "p:claude", json!({"env": {"FOO": "bar"}}));
        let setups = wsp_core::agent_setup::agent_setups(Rc::new(SetupStore(Rc::try_unwrap(store).ok().unwrap())));
        setups.load().await.unwrap();
        let launch = setups.launch_of("p", "claude");
        assert_eq!(launch.env.get("FOO").map(String::as_str), Some("bar"));
    });
}
