// SPDX-License-Identifier: AGPL-3.0-only
//! spec/agent_setups.md: agent-setup.ts and the protocol stand-ins it reads, against the TS oracle
//! (tests/fixtures/core_rest/agent_setups.json and adapters_setups.json, oracle/core_rest.oracle.ts), plus the real
//! folder reads on this computer: packages/runtime/test/thread-defaults.test.ts:333 ("reads a folder on another
//! computer the same way") and the same tree read here.
mod rest;

use std::os::unix::fs::symlink;
use std::rc::Rc;

use futures::FutureExt;
use rest::{fixture, json, run_local, wire};
use serde_json::{json, Value};
use wsp_adapters::claude::landmines::is_landmine;
use wsp_core::agent_setup::{
    agent_setups, key_of, loop_line, real_folder_here, real_folder_script, SetupSetOptions, AGENT_SETUPS, LINK_HOPS,
};
use wsp_core::protocol_pending::{agent_env_refusal, config_dir_refusal, no_config_dir_line, not_run_refusal, setup_patched};
use wsp_store::store::{BlobMark, Store};
use wsp_wire::types::thread_defaults::{AgentSetup, AgentSetupSet, ResolvedFolder};
use wsp_wire::views::harness::HarnessCatalog;
use wsp_words::exit::{HostError, Kind};

/// The store as text per document, so every read parses afresh and a test reads the bytes a write left.
#[derive(Default)]
struct MemStore {
    docs: std::cell::RefCell<indexmap::IndexMap<String, indexmap::IndexMap<String, String>>>,
}

impl MemStore {
    fn new() -> Rc<MemStore> {
        Rc::new(MemStore::default())
    }
    fn text(&self, collection: &str, id: &str) -> Option<String> {
        self.docs.borrow().get(collection).and_then(|c| c.get(id).cloned())
    }
    fn ids(&self, collection: &str) -> Vec<String> {
        self.docs.borrow().get(collection).map(|c| c.keys().cloned().collect()).unwrap_or_default()
    }
    fn seed(&self, collection: &str, id: &str, value: Value) {
        self.docs.borrow_mut().entry(collection.into()).or_default().insert(id.into(), value.to_string());
    }
}

#[async_trait::async_trait(?Send)]
impl Store for MemStore {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        Ok(self.text(collection, id).map(|t| serde_json::from_str(&t).unwrap()))
    }
    async fn put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        self.docs.borrow_mut().entry(collection.into()).or_default().insert(id.into(), serde_json::to_string(&value).unwrap());
        Ok(())
    }
    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError> {
        Ok(self.ids(collection).iter().map(|id| serde_json::from_str(&self.text(collection, id).unwrap()).unwrap()).collect())
    }
    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError> {
        Ok(self.ids(collection))
    }
    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        if let Some(c) = self.docs.borrow_mut().get_mut(collection) {
            c.shift_remove(id);
        }
        Ok(())
    }
    async fn get_blob(&self, _c: &str, _id: &str) -> Result<Option<Vec<u8>>, HostError> {
        panic!("setups keep no blobs")
    }
    async fn put_blob(&self, _c: &str, _id: &str, _bytes: Vec<u8>) -> Result<(), HostError> {
        panic!("setups keep no blobs")
    }
    async fn delete_blob(&self, _c: &str, _id: &str) -> Result<(), HostError> {
        panic!("setups keep no blobs")
    }
    async fn stat_blob(&self, _c: &str, _id: &str) -> Result<Option<BlobMark>, HostError> {
        panic!("setups keep no blobs")
    }
}

fn fx() -> Value {
    fixture("agent_setups")
}

fn opt<T: serde::de::DeserializeOwned>(v: &Value) -> Option<T> {
    (!v.is_null()).then(|| wire(v))
}

// fixture: agent_setups.json patched
#[test]
fn a_change_over_a_kept_setup_is_the_tss_patch_key_for_key_and_nothing_where_all_is_back_to_the_agents_own() {
    for row in fx()["patched"].as_array().unwrap() {
        let kept: Option<AgentSetup> = opt(&row["kept"]);
        let set: AgentSetupSet = wire(&row["set"]);
        let out = setup_patched(kept.as_ref(), &set);
        assert_eq!(out.as_ref().map(json).unwrap_or(Value::Null), row["out"], "{}", row["name"]);
        if let Some(o) = &out {
            assert_eq!(serde_json::to_string(o).unwrap(), serde_json::to_string(&row["out"]).unwrap(), "{}: key order", row["name"]);
        }
    }
}

// fixture: agent_setups.json envRefusal
#[test]
fn a_variable_is_refused_where_it_is_wsps_own_decides_how_the_process_starts_or_the_agents_launch_drops_it() {
    let drops: &dyn Fn(&str) -> bool = &is_landmine;
    for row in fx()["envRefusal"].as_array().unwrap() {
        let (name, agent, claude) = (row[0].as_str().unwrap(), row[1].as_str().unwrap(), row[2].as_bool().unwrap());
        let label = if agent == "claude" { "Claude Code" } else { "Codex" };
        let got = agent_env_refusal(name, label, claude.then_some(drops));
        assert_eq!(got.map(Value::String).unwrap_or(Value::Null), row[3], "{name} for {agent}");
    }
}

// fixture: agent_setups.json configDirRefusal, noConfigDirLine
#[test]
fn a_config_folder_is_refused_as_the_home_outside_it_or_inside_wsps_own_folder() {
    let f = fx();
    for row in f["configDirRefusal"].as_array().unwrap() {
        let real: ResolvedFolder = ResolvedFolder {
            folder: row[2]["folder"].as_str().unwrap().into(),
            home: row[2]["home"].as_str().unwrap().into(),
            kept: row[2]["kept"].as_array().unwrap().iter().map(|k| k.as_str().unwrap().to_string()).collect(),
        };
        let got = config_dir_refusal("Claude Code", row[1].as_str().unwrap(), &real);
        assert_eq!(got.map(Value::String).unwrap_or(Value::Null), row[3], "{}", row[0]);
    }
    assert_eq!(no_config_dir_line("Cursor"), f["noConfigDirLine"]);
}

/// The oracle's `lists(version, models)` over the codex table.
fn lists(table: &HarnessCatalog, version: Option<&str>, models: &[&str]) -> HarnessCatalog {
    let mut v = json(table);
    v["source"] = json!("harness");
    v["version"] = json!(version);
    v["models"] = Value::Array(models.iter().map(|m| json!({ "value": m, "label": m.to_uppercase() })).collect());
    v["legacyModels"] = json!([]);
    wire(&v)
}

// fixture: agent_setups.json notRun
#[test]
fn a_model_the_binary_does_not_run_is_said_as_its_version_not_running_it() {
    let f = fx();
    let codex: HarnessCatalog = wire(&fixture("harness_catalog")["catalogs"][1]);
    assert_eq!(codex.harness, "codex");
    let mut pin_unread = json(&codex);
    pin_unread["version"] = json!("app-server x");
    let pin_unread: HarnessCatalog = wire(&pin_unread);
    let u = "npm i -g @openai/codex";
    let cases: Vec<(&str, HarnessCatalog, Option<&HarnessCatalog>, Option<&str>, Option<&str>)> = vec![
        ("table lists", codex.clone(), Some(&codex), Some("gpt-5.2"), Some(u)),
        ("older binary", lists(&codex, Some("0.150.0"), &["gpt-5.5"]), Some(&codex), Some("gpt-5.6-sol"), Some(u)),
        ("older, no update", lists(&codex, Some("0.150.0"), &["gpt-5.5"]), Some(&codex), Some("gpt-5.6-sol"), None),
        ("newer binary", lists(&codex, Some("0.160.0"), &["gpt-6", "gpt-6-mini"]), Some(&codex), Some("gpt-5.2"), Some(u)),
        ("listed", lists(&codex, Some("0.160.0"), &["gpt-5.2"]), Some(&codex), Some("gpt-5.2"), Some("u")),
        ("unknown model", lists(&codex, Some("0.160.0"), &["gpt-6"]), Some(&codex), Some("gpt-9"), Some("u")),
        ("no version", lists(&codex, None, &["gpt-6"]), Some(&codex), Some("gpt-5.2"), Some("u")),
        ("no model", lists(&codex, Some("0.160.0"), &["gpt-6"]), Some(&codex), None, Some("u")),
        ("no table", lists(&codex, Some("0.160.0"), &["gpt-6"]), None, Some("gpt-5.2"), Some("u")),
        ("same version", lists(&codex, Some("0.153.0"), &["gpt-6"]), Some(&codex), Some("gpt-5.2"), Some("u")),
        ("prerelease", lists(&codex, Some("0.153.0-alpha.1"), &["gpt-6"]), Some(&codex), Some("gpt-5.2"), Some("u")),
        ("unreadable pin", lists(&codex, Some("0.1.0"), &["gpt-6"]), Some(&pin_unread), Some("gpt-5.2"), Some("u")),
    ];
    let got: Vec<Value> = cases
        .into_iter()
        .map(|(name, l, t, m, up)| {
            let e = not_run_refusal(Some(&l), t, m, up);
            json!([name, e.map(|e| json!({ "message": e.message, "fix": e.fix, "kind": e.kind.map(|k| if k == Kind::Usage { "usage" } else { "other" }) }))])
        })
        .collect();
    assert_eq!(Value::Array(got), f["notRun"]);
}

fn folder_answer() -> Rc<dyn Fn(&str) -> futures::future::LocalBoxFuture<'static, Result<ResolvedFolder, HostError>>> {
    Rc::new(|p: &str| {
        let folder = if p == "/home/dev/link" { "/home/dev/real".to_string() } else { p.to_string() };
        async move { Ok(ResolvedFolder { folder, home: "/home/dev".into(), kept: vec!["/home/dev/.wsp".into()] }) }.boxed_local()
    })
}

fn o(agent_name: &str) -> SetupSetOptions {
    SetupSetOptions { folder: folder_answer(), agent_name: agent_name.into() }
}

fn out_of(r: Result<Option<AgentSetup>, HostError>) -> Value {
    match r {
        Ok(v) => json!({ "ok": v.as_ref().map(json) }),
        Err(e) => json!({ "err": e.message, "kind": if e.kind == Some(Kind::Usage) { "usage" } else { "other" } }),
    }
}

// fixture: agent_setups.json steps, launchClaude, offClaude
#[test]
fn a_setup_is_checked_kept_in_the_store_then_in_memory_and_read_by_every_launch() {
    run_local(async {
        let f = fx();
        let store = MemStore::new();
        let s = agent_setups(store.clone());
        s.load().await.unwrap();
        let set = |change: Value| -> AgentSetupSet { wire(&change) };
        let steps: Vec<(&str, &str, Value, &str)> = vec![
            ("set env", "claude", json!({ "env": { "FOO": "bar", "CLAUDE_CODE_X": null } }), "Claude Code"),
            ("refused env", "claude", json!({ "env": { "PATH": "/x" } }), "Claude Code"),
            ("refused dropped", "claude", json!({ "env": { "CLAUDE_CODE_USE_BEDROCK": "1" } }), "Claude Code"),
            ("relative dir", "claude", json!({ "configDir": "rel/c" }), "Claude Code"),
            ("dir through link", "claude", json!({ "configDir": "/home/dev/link" }), "Claude Code"),
            ("dir is home", "claude", json!({ "configDir": "/home/dev" }), "Claude Code"),
            ("program and args", "claude", json!({ "program": "/opt/claude", "args": ["--verbose"] }), "Claude Code"),
            ("off", "claude", json!({ "on": false }), "Claude Code"),
        ];
        let expected = f["steps"].as_array().unwrap();
        let mut at = 0;
        for (name, agent, change, label) in steps {
            let out = out_of(s.set("here", agent, set(change), o(label)).await);
            let row = &expected[at];
            assert_eq!(row["name"], name);
            assert_eq!(out, row["out"], "{name}");
            assert_eq!(
                store.text(AGENT_SETUPS, "here:claude").map(|t| serde_json::from_str::<Value>(&t).unwrap()).unwrap_or(Value::Null),
                row["doc"],
                "{name}: the stored document"
            );
            assert_eq!(json(&store.ids(AGENT_SETUPS)), row["keys"], "{name}");
            at += 1;
        }
        assert_eq!(
            store.text(AGENT_SETUPS, "here:claude").unwrap(),
            serde_json::to_string(&expected[7]["doc"]).unwrap(),
            "the stored document's key order is the TS's"
        );
        let launch = s.launch_of("here", "claude");
        assert_eq!(
            json!({ "configDir": launch.config_dir, "env": json(&launch.env), "launch": launch.launch.as_ref().map(json) }),
            f["launchClaude"]
        );
        assert_eq!(
            serde_json::to_string(&launch.env).unwrap(),
            serde_json::to_string(&f["launchClaude"]["env"]).unwrap(),
            "the config folder's own variable comes last"
        );
        assert_eq!(json!(s.off("here", "claude")), f["offClaude"]);
        for (name, agent, change, label) in [
            ("codex dir refused", "codex", json!({ "configDir": "/home/dev/cx" }), "Codex"),
            ("cursor no dir", "cursor", json!({ "configDir": "/home/dev/cu" }), "Cursor"),
            (
                "back to nothing",
                "claude",
                json!({ "on": true, "program": null, "args": null, "configDir": null, "env": { "FOO": null } }),
                "Claude Code",
            ),
        ] {
            let out = out_of(s.set("here", agent, set(change), o(label)).await);
            let row = &expected[at];
            assert_eq!(row["name"], name);
            assert_eq!(out, row["out"], "{name}");
            assert_eq!(json(&store.ids(AGENT_SETUPS)), row["keys"], "{name}");
            at += 1;
        }
        assert_eq!(s.get("here", "claude"), None, "a setup back to nothing is gone from memory too");
    });
}

// fixture: agent_setups.json afterLoad
#[test]
fn a_load_reads_every_kept_setup_and_a_launch_leaves_off_a_variable_kept_before_the_rule() {
    run_local(async {
        let f = fx();
        let store = MemStore::new();
        store.seed(
            AGENT_SETUPS,
            "box:claude",
            json!({ "configDir": "/root/cfg", "env": { "PATH": "/kept/before", "GOOD": "1", "CLAUDECODE": "1" } }),
        );
        let s = agent_setups(store.clone());
        assert_eq!(s.get("box", "claude"), None, "nothing is read before load");
        s.load().await.unwrap();
        let a = &f["afterLoad"];
        assert_eq!(s.get("box", "claude").as_ref().map(json).unwrap_or(Value::Null), a["get"]);
        let l = s.launch_of("box", "claude");
        assert_eq!(json!({ "configDir": l.config_dir, "env": json(&l.env) }), a["launch"]);
        let c = s.launch_of("box", "codex");
        assert_eq!(json!({ "env": json(&c.env) }), a["codexLaunch"]);
        assert_eq!(c.config_dir, None);
        assert_eq!(c.launch, None);
        assert_eq!(json!(s.off("box", "claude")), a["off"]);
        assert_eq!(s.get("nowhere", "claude"), None);
        assert_eq!(key_of("box", "claude"), "box:claude");
        assert_eq!(fixture("adapters_setups")["keyOf"], json!(key_of("here", "claude")));
        assert_eq!(fixture("adapters_setups")["agentSetups"], json!(AGENT_SETUPS));
    });
}

// fixture: adapters_setups.json realFolderScript
#[test]
fn the_folder_read_on_another_computer_is_the_tss_shell_line_word_for_word() {
    assert_eq!(LINK_HOPS, 40);
    for row in fixture("adapters_setups")["realFolderScript"].as_array().unwrap() {
        assert_eq!(real_folder_script(row[0].as_str().unwrap()), row[1].as_str().unwrap(), "{}", row[0]);
    }
}

/// thread-defaults.test.ts:333's tree: b links to a, a/later to /etc/wsp-not-made, a/near to ../a/later, loop to itself.
fn tree() -> (tempfile::TempDir, std::path::PathBuf, String) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::create_dir(root.join("a")).unwrap();
    symlink(root.join("a"), root.join("b")).unwrap();
    symlink("/etc/wsp-not-made", root.join("a").join("later")).unwrap();
    symlink("../a/later", root.join("a").join("near")).unwrap();
    symlink("loop", root.join("loop")).unwrap();
    let real = std::fs::canonicalize(&root).unwrap().display().to_string();
    (dir, root, real)
}

fn bash(script: &str, home: &std::path::Path) -> std::process::Output {
    std::process::Command::new("/bin/bash")
        .arg("-c")
        .arg(script)
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .output()
        .unwrap()
}

// TS: thread-defaults.test.ts:333
#[test]
fn reads_a_folder_on_another_computer_the_same_way_on_that_computer_links_followed_the_part_not_made_yet_kept() {
    let (_dir, root, real) = tree();
    let etc = std::fs::canonicalize("/etc").unwrap().display().to_string();
    let read = |p: std::path::PathBuf| -> Vec<String> {
        let out = bash(&real_folder_script(&p.display().to_string()), &root);
        String::from_utf8(out.stdout).unwrap().trim().lines().map(String::from).collect()
    };
    assert_eq!(read(root.join("b").join("not").join("yet")), [format!("{real}/a/not/yet"), real.clone()]);
    assert_eq!(read(root.join("b").join("later").join("x")), [format!("{etc}/wsp-not-made/x"), real.clone()]);
    assert_eq!(read(root.join("b").join("near")), [format!("{etc}/wsp-not-made"), real.clone()]);
    let looped = bash(&real_folder_script(&root.join("loop").join("x").display().to_string()), &root);
    assert_eq!(
        (looped.status.code(), String::from_utf8(looped.stderr).unwrap().trim().to_string()),
        (Some(3), loop_line(&root.join("loop").join("x").display().to_string()))
    );
}

// spec: R1, R2 (the same tree read on this computer: the part that exists resolved, a link not made yet read, the rest kept)
#[test]
fn the_same_folder_read_here_follows_every_link_and_keeps_the_part_not_made_yet() {
    run_local(async {
        let (_dir, root, real) = tree();
        let etc = std::fs::canonicalize("/etc").unwrap().display().to_string();
        let here = |p: std::path::PathBuf| async move { real_folder_here(&p.display().to_string()).await };
        assert_eq!(here(root.join("b").join("not").join("yet")).await.unwrap(), format!("{real}/a/not/yet"));
        assert_eq!(here(root.join("b").join("later").join("x")).await.unwrap(), format!("{etc}/wsp-not-made/x"));
        assert_eq!(here(root.join("b").join("near")).await.unwrap(), format!("{etc}/wsp-not-made"));
        assert_eq!(here(root.join("b")).await.unwrap(), format!("{real}/a"));
        assert_eq!(
            real_folder_here(&format!("{}/./b/../b//", root.display())).await.unwrap(),
            format!("{real}/a"),
            "the path is normalized before it is read"
        );
        let e = here(root.join("loop").join("x")).await.unwrap_err();
        assert_eq!(e.message, loop_line(&root.join("loop").join("x").display().to_string()));
        assert_eq!(e.kind, Some(Kind::Usage));
    });
}

// spec: R1 (a path whose every part is missing up to the root is answered as named, normalized)
#[test]
fn a_path_nothing_of_which_exists_is_answered_as_named() {
    run_local(async {
        assert_eq!(real_folder_here("/wsp-nothing-here/a/../b").await.unwrap(), "/wsp-nothing-here/b");
        assert_eq!(real_folder_here("/").await.unwrap(), "/");
    });
}
