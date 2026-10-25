// SPDX-License-Identifier: AGPL-3.0-only
//! agents_launch (spec/agents_launch.md): config folders, files a turn carries, environments, servers, the adapter.
//! Expected values read off agents.ts at 65f6b89c7 and the TS tests named above each test.
mod support;

use std::rc::Rc;
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::{json, Value};
use support::agents_ctx::{entry, factory, landing_entry, plain_factory, table, FakeAdapter, FakeAgentsCtx, FakeSetup};
use support::{json_of, local, record, settle, FakeMachine};
use wsp_adapters::port::{AgentLaunch, AttachmentRoad, Env};
use wsp_catalog::mcp_launch::LaunchServerRoad;
use wsp_core::console;
use wsp_core::types::harness::{Brokered, BrokeredKind};
use wsp_frames::WorkspaceKind;
use wsp_threads::threads::agents::launch::{AdapterAsk, INLINE_EXEC_MS};
use wsp_threads::threads::agents::{AgentsArea, BrokeredLaunch, ProjectSeen, ServerValuesRoad, SetupLaunch, TurnLaunch};
use wsp_threads::threads::threads::Entry;
use wsp_wire::types::attachments::Attachment;
use wsp_wire::types::thread_defaults::{AccessChoice, ProjectOverrides};
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::preferences::default_preferences;
use wsp_words::attachments::{drop_files_line, files_not_landed_line, land_files_line};
use wsp_words::exit::{HostError, Kind};
use wsp_words::workspace_state::sign_in_refusal_line;

const HOST_TOKEN_ENV: &str = "WSP_HOST_TOKEN";
const CLAUDE_CONFIG: &str = "/root/.claude-cfg/.claude.json";

// ---- fixtures ----------------------------------------------------------------------------------------------------

fn env(pairs: &[(&str, &str)]) -> Env {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn pairs(e: &Env) -> Vec<(String, String)> {
    e.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

fn pair(k: &str, v: &str) -> (String, String) {
    (k.to_string(), v.to_string())
}

fn refused<T>(r: Result<T, HostError>) -> HostError {
    match r {
        Ok(_) => panic!("expected a refusal"),
        Err(e) => e,
    }
}

fn b64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }
    out
}

fn att(media: &str, bytes: &[u8], name: Option<&str>) -> Attachment {
    Attachment { media_type: media.to_string(), bytes: b64(bytes), name: name.map(str::to_string) }
}

fn runs(machine: &FakeMachine, needle: &str) -> Vec<Option<u64>> {
    machine.execs.borrow().iter().filter(|(c, _)| c.contains(needle)).map(|(_, t)| *t).collect()
}

/// The words a refused config folder launches with.
fn launch_refusal(agent: &str, id: &str, kept: &str, why: &str) -> String {
    format!("{agent} does not start with its config folder {kept}: {why}. Set another with wsp agents setup {id} --config, or put its own back with --reset config.")
}

/// A scratch computer: a real home and a real folder outside it, both canonical.
struct Scratch {
    _dir: tempfile::TempDir,
    root: String,
    home: String,
    outside: String,
}

fn scratch() -> Scratch {
    let dir = tempfile::tempdir().expect("a folder");
    let root = std::fs::canonicalize(dir.path()).expect("a real path");
    let home = root.join("home");
    let outside = root.join("outside");
    std::fs::create_dir_all(&home).expect("home");
    std::fs::create_dir_all(&outside).expect("outside");
    let s = |p: &std::path::Path| p.to_str().expect("utf8").to_string();
    Scratch { root: s(&root), home: s(&home), outside: s(&outside), _dir: dir }
}

fn link(from: &str, to: &str) {
    std::os::unix::fs::symlink(from, to).expect("a link");
}

/// A context for this computer with `home` as its home, and a live local workspace in it.
fn here(s: &Scratch) -> (Rc<FakeAgentsCtx>, Entry, Arc<FakeMachine>) {
    let ctx = FakeAgentsCtx::new();
    *ctx.local_home_dir.borrow_mut() = Some(s.home.clone());
    let (e, machine) = entry(json!({}));
    ctx.add_entry(e.clone());
    (ctx, e, machine)
}

/// A context for a computer the person joined, and a live workspace on it.
fn joined() -> (Rc<FakeAgentsCtx>, Entry, Arc<FakeMachine>) {
    let ctx = FakeAgentsCtx::new();
    ctx.place_names.borrow_mut().insert("srv".to_string(), "spoo".to_string());
    let (e, machine) = entry(json!({ "kind": "place", "place": "srv" }));
    ctx.add_entry(e.clone());
    (ctx, e, machine)
}

/// A context whose kind answers what a launch on a box does: HOME and the agents' store variables.
fn serving() -> (Rc<FakeAgentsCtx>, Entry, Arc<FakeMachine>) {
    let ctx = FakeAgentsCtx::new();
    *ctx.kind_env_base.borrow_mut() = env(&[("HOME", "/root"), ("CLAUDE_CONFIG_DIR", "/root/.claude-cfg"), ("CODEX_HOME", "/root/.codex")]);
    ctx.kind_env_homes.borrow_mut().clear();
    ctx.kind_server_values.set(ServerValuesRoad::Launch);
    let (e, machine) = entry(json!({}));
    ctx.add_entry(e.clone());
    (ctx, e, machine)
}

/// What the config read prints with `text` as the first scope's first file, and no other file there.
fn config_read(text: &str) -> String {
    format!("wsp-mcp 0 0 {}\nwsp-mcp end\n", b64(text.as_bytes()))
}

fn reads(machine: &FakeMachine, text: &str) {
    machine.answer("wsp_mcp_read", 0, &config_read(text));
}

const TRACKER: &str = r#"{"primaryApiKey":"sk-ant-api03-PERSONS-OWN-TESTONLY","mcpServers":{"tracker":{"type":"http","url":"https://mcp.linear.app/mcp","headers":{"Authorization":"Bearer ${LINEAR_TOKEN}"}}}}"#;

// ---- A1: where an agent's setup lives -------------------------------------------------------------------------------

#[test]
fn setup_place_is_here_the_record_place_or_none() {
    let (local_entry, _) = entry(json!({ "kind": "local" }));
    let (placed, _) = entry(json!({ "kind": "place", "place": "srv" }));
    let (cloud, _) = entry(json!({ "kind": "cloud" }));
    assert_eq!(AgentsArea::setup_place(&local_entry), Some("here".to_string()));
    assert_eq!(AgentsArea::setup_place(&placed), Some("srv".to_string()));
    assert_eq!(AgentsArea::setup_place(&cloud), None, "a fork at a provider has no setup of the person's");

    let ctx = FakeAgentsCtx::new();
    ctx.turn_off("srv", "codex");
    ctx.turn_off("here", "claude");
    let area = ctx.area();
    assert!(area.agent_off(&placed, "codex"));
    assert!(!area.agent_off(&placed, "claude"), "off for one agent on one computer only");
    assert!(area.agent_off(&local_entry, "claude"));
    assert!(!area.agent_off(&cloud, "codex"), "never at a provider");
    assert_eq!(AgentsArea::agent_label("claude"), "Claude Code");
    assert_eq!(AgentsArea::agent_label("codex"), "Codex");
    assert_eq!(AgentsArea::agent_label("aider"), "aider", "an id with no row is its own label");
}

// ---- A2: a config folder as the computer it names resolves it -----------------------------------------------------

// TS: thread-defaults.test.ts:275
#[test]
fn config_folder_on_here_follows_links_and_names_the_folders_wsp_keeps() {
    local(async {
        let s = scratch();
        std::fs::create_dir_all(format!("{}/state", s.root)).expect("state");
        std::fs::create_dir_all(format!("{}/real", s.home)).expect("real");
        link(&format!("{}/real", s.home), &format!("{}/in", s.home));
        let ctx = FakeAgentsCtx::new();
        *ctx.local_home_dir.borrow_mut() = Some(s.home.clone());
        let area = ctx.area();

        // The part that exists is resolved, the rest kept as named: an agent's first run makes its config folder.
        let read = area.config_folder_on("here", &format!("{}/in/claude-wsp", s.home)).await.expect("read");
        assert_eq!(read.folder, format!("{}/real/claude-wsp", s.home));
        assert_eq!(read.home, s.home);
        assert_eq!(read.kept, [format!("{}/.wsp", s.home)]);

        // With the host's state file, its folder is one wsp keeps too.
        *ctx.state_path.borrow_mut() = Some(format!("{}/state/state.json", s.root));
        let read = area.config_folder_on("here", &format!("{}/in", s.home)).await.expect("read");
        assert_eq!(read.folder, format!("{}/real", s.home));
        assert_eq!(read.kept, [format!("{}/.wsp", s.home), format!("{}/state", s.root)]);

        // With no local wiring the process's home is the home.
        let bare = FakeAgentsCtx::new();
        *bare.process_home.borrow_mut() = s.home.clone();
        let read = bare.area().config_folder_on("here", &format!("{}/x", s.home)).await.expect("read");
        assert_eq!(read.home, s.home);
    });
}

// TS: thread-defaults.test.ts:275
#[test]
fn config_folder_on_here_refuses_a_loop_of_links() {
    local(async {
        let s = scratch();
        link("loop", &format!("{}/loop", s.home));
        let (ctx, _, _) = here(&s);
        let path = format!("{}/loop/x", s.home);
        let e = refused(ctx.area().config_folder_on("here", &path).await);
        assert_eq!(e.message, format!("{path} goes round a loop of links"));
        assert_eq!(e.kind, Some(Kind::Usage));
    });
}

// TS: thread-defaults.test.ts:333, places-agents.test.ts:190
#[test]
fn config_folder_on_a_joined_computer_reads_its_answer() {
    local(async {
        let (ctx, _, _) = joined();
        let area = ctx.area();
        // The folder and the home, one a line, read on that computer; the path is normalised and the kept folder is its home's.
        ctx.place_answers("srv", "pwd", 0, "/home/maya/x/../y\n/home/maya\n", "");
        let read = area.config_folder_on("srv", "/home/maya/x/../y").await.expect("read");
        assert_eq!((read.folder.as_str(), read.home.as_str()), ("/home/maya/y", "/home/maya"));
        assert_eq!(read.kept, ["/home/maya/.wsp"]);
        let asked = ctx.called_with("placeExec");
        assert_eq!(asked.len(), 1);
        assert!(asked[0].starts_with(&format!("placeExec srv {INLINE_EXEC_MS} p='/home/maya/y'; rest=; n=0; while ")), "{}", asked[0]);

        // A loop of links is the computer's own refusal, in its own words, a usage error.
        ctx.place_rules.borrow_mut().clear();
        ctx.place_answers("srv", "pwd", 3, "", "/a/loop/x goes round a loop of links\n");
        let e = refused(area.config_folder_on("srv", "/a/loop/x").await);
        assert_eq!((e.message.as_str(), e.kind), ("/a/loop/x goes round a loop of links", Some(Kind::Usage)));
    });
}

// TS: places-agents.test.ts:190
#[test]
fn config_folder_on_refuses_an_answer_with_no_absolute_home() {
    local(async {
        let (ctx, _, _) = joined();
        let area = ctx.area();
        // A home that is not absolute.
        ctx.place_answers("srv", "pwd", 0, "/a/b\nrelative\n", "");
        let e = refused(area.config_folder_on("srv", "/a/b").await);
        assert_eq!((e.message.as_str(), e.kind), ("spoo did not say where /a/b is: exit 0", Some(Kind::Invalid)));
        // A missing second line.
        ctx.place_rules.borrow_mut().clear();
        ctx.place_answers("srv", "pwd", 0, "/a/b\n", "");
        assert_eq!(refused(area.config_folder_on("srv", "/a/b").await).message, "spoo did not say where /a/b is: exit 0");
        // A non-zero exit says its own words, trimmed.
        ctx.place_rules.borrow_mut().clear();
        ctx.place_answers("srv", "pwd", 1, "", "realpath: no such folder\n");
        let e = refused(area.config_folder_on("srv", "/a/b").await);
        assert_eq!((e.message.as_str(), e.kind), ("spoo did not say where /a/b is: realpath: no such folder", Some(Kind::Invalid)));
        // Nothing at all.
        ctx.place_rules.borrow_mut().clear();
        assert_eq!(refused(area.config_folder_on("srv", "/a/b").await).message, "spoo did not say where /a/b is: exit 0");
    });
}

// ---- A3: the one check on a kept config folder ------------------------------------------------------------------------

#[test]
fn setup_refusal_reads_nothing_without_a_kept_folder() {
    local(async {
        let (ctx, e, machine) = joined();
        let area = ctx.area();
        assert_eq!(area.setup_refusal(&e, None).await.expect("no refusal"), None);
        // Another agent's kept folder is not the default agent's.
        ctx.keep_config_dir("srv", "codex", "/home/maya/codex-wsp");
        assert_eq!(area.setup_refusal(&e, None).await.expect("no refusal"), None);
        assert!(ctx.called().is_empty(), "no read, and no word moved");
        assert!(machine.commands().is_empty());
        // A fork at a provider keeps no folder of the person's.
        let (cloud, _) = entry(json!({ "kind": "cloud" }));
        assert_eq!(area.setup_refusal(&cloud, Some("codex")).await.expect("no refusal"), None);
        assert!(ctx.called().is_empty());
    });
}

// TS: thread-defaults.test.ts:288, places-agents.test.ts:190
#[test]
fn setup_refusal_remembers_and_says_when_it_moved() {
    local(async {
        let (ctx, e, _) = joined();
        ctx.keep_config_dir("srv", "claude", "/home/maya/claude-wsp");
        let area = ctx.area();
        let words = launch_refusal(
            "Claude Code",
            "claude",
            "/home/maya/claude-wsp",
            "Claude Code's config folder has to be under the home folder /home/maya, and /home/maya/claude-wsp is not",
        );

        // Its links now lead out of the home: refused, remembered, and said once.
        ctx.place_answers("srv", "pwd", 0, "/etc/x\n/home/maya\n", "");
        assert_eq!(area.setup_refusal(&e, None).await.expect("read"), Some(words.clone()));
        assert_eq!(area.setup_refusals.borrow().get("srv:claude"), Some(&words));
        assert_eq!(ctx.called_with("setupRefusalMoved"), ["setupRefusalMoved srv claude"]);
        assert_eq!(area.setup_refusal(&e, None).await.expect("read"), Some(words.clone()));
        assert_eq!(ctx.called_with("setupRefusalMoved").len(), 1, "the same word moves nothing");

        // It stands again: the word is dropped and that is said.
        ctx.place_rules.borrow_mut().clear();
        ctx.place_answers("srv", "pwd", 0, "/home/maya/claude-wsp\n/home/maya\n", "");
        assert_eq!(area.setup_refusal(&e, None).await.expect("read"), None);
        assert!(area.setup_refusals.borrow().get("srv:claude").is_none());
        assert_eq!(ctx.called_with("setupRefusalMoved").len(), 2);
        assert_eq!(area.setup_refusal(&e, None).await.expect("read"), None);
        assert_eq!(ctx.called_with("setupRefusalMoved").len(), 2, "no refusal before and after moves nothing");

        // The folder is the home itself, or inside wsp's own.
        ctx.place_rules.borrow_mut().clear();
        ctx.place_answers("srv", "pwd", 0, "/home/maya\n/home/maya\n", "");
        assert_eq!(
            area.setup_refusal(&e, None).await.expect("read"),
            Some(launch_refusal("Claude Code", "claude", "/home/maya/claude-wsp", "Claude Code's config folder cannot be the home folder itself; name a folder under it"))
        );
        ctx.place_rules.borrow_mut().clear();
        ctx.place_answers("srv", "pwd", 0, "/home/maya/.wsp/claude\n/home/maya\n", "");
        assert_eq!(
            area.setup_refusal(&e, None).await.expect("read"),
            Some(launch_refusal("Claude Code", "claude", "/home/maya/claude-wsp", "/home/maya/claude-wsp is inside wsp's own folder, so it is no agent's config folder"))
        );

        // A read that fails is the failure, never a word.
        ctx.place_rules.borrow_mut().clear();
        ctx.place_answers("srv", "pwd", 1, "", "realpath: no such folder");
        assert_eq!(refused(area.setup_refusal(&e, None).await).message, "spoo did not say where /home/maya/claude-wsp is: realpath: no such folder");
    });
}

// ---- A4: the homes a local launch reads ---------------------------------------------------------------------------------

// TS: thread-defaults.test.ts:225, thread-defaults.test.ts:288
#[test]
fn homes_here_refuses_a_kept_folder_that_leads_out_and_stores_here_lists_moved_homes() {
    local(async {
        let s = scratch();
        let (ctx, _, _) = here(&s);
        *ctx.process_home.borrow_mut() = s.home.clone();
        ctx.local_homes.borrow_mut().insert("codex".to_string(), format!("{}/codex-elsewhere", s.home));
        let area = ctx.area();

        // Each agent's home: the folder the person kept, else the local wiring's, else its own under the home.
        let homes = area.homes_here().await.expect("homes");
        assert_eq!(ctx.called_with("ready"), ["ready"], "after the host is ready");
        assert_eq!(homes.get("claude"), Some(&format!("{}/.claude", s.home)));
        assert_eq!(homes.get("codex"), Some(&format!("{}/codex-elsewhere", s.home)));

        // Only what is not <home>/<state home> is a store.
        let stores = area.stores_here().await.expect("stores");
        assert_eq!(stores.get("codex"), Some(&format!("{}/codex-elsewhere", s.home)));
        assert!(stores.get("claude").is_none());
        let kept = format!("{}/claude-wsp", s.home);
        ctx.keep_config_dir("here", "claude", &kept);
        let stores = area.stores_here().await.expect("stores");
        assert_eq!(stores.get("claude"), Some(&kept));
        assert_eq!(area.homes_here().await.expect("homes").get("claude"), Some(&kept));

        // A kept folder that now leads out of the home fails every launch road, in the one sentence.
        link(&s.outside, &format!("{}/in", s.home));
        let leading = format!("{}/in/claude-wsp", s.home);
        ctx.keep_config_dir("here", "claude", &leading);
        let e = refused(area.homes_here().await);
        assert_eq!(e.kind, Some(Kind::Usage));
        assert_eq!(
            e.message,
            launch_refusal("Claude Code", "claude", &leading, &format!("Claude Code's config folder has to be under the home folder {}, and {leading} is not", s.home))
        );
        assert_eq!(refused(area.stores_here().await).message, e.message);
    });
}

// ---- A5: confining a launch to its setup ----------------------------------------------------------------------------------

// TS: thread-defaults.test.ts:288, places-agents.test.ts:190
#[test]
fn launch_adapter_for_checks_the_folder_then_the_broker() {
    local(async {
        let s = scratch();
        let (ctx, e, _) = here(&s);
        let adapter = FakeAdapter::new().arc();
        let (f, seen) = factory(adapter);
        ctx.add_adapter("claude", f);
        let area = ctx.area();

        // Nothing kept: the broker is told which agent, then the adapter is built.
        let (harness, _) = area.launch_adapter_for(&e, AdapterAsk::default()).await.expect("built");
        assert_eq!(harness, "claude", "unnamed means the default agent");
        assert_eq!(ctx.called_with("knowBroker"), ["knowBroker ws1 claude"]);
        assert_eq!(seen.borrow().len(), 1);

        // A turn that carries the host's own token needs no broker read.
        let turn = env(&[(HOST_TOKEN_ENV, "tok")]);
        area.launch_adapter_for(&e, AdapterAsk { turn_env: Some(turn), ..Default::default() }).await.expect("built");
        assert_eq!(ctx.called_with("knowBroker").len(), 1);
        assert_eq!(seen.borrow().len(), 2);

        // A broker that cannot be read fails the launch before an adapter is made.
        ctx.broker_failures.borrow_mut().insert("claude".to_string(), "no login is held for Claude Code".to_string());
        assert_eq!(refused(area.launch_adapter_for(&e, AdapterAsk::default()).await).message, "no login is held for Claude Code");
        assert_eq!(seen.borrow().len(), 2);
        ctx.broker_failures.borrow_mut().clear();

        // A kept folder that leads out is refused first: no broker read, no adapter.
        link(&s.outside, &format!("{}/in", s.home));
        let leading = format!("{}/in/claude-wsp", s.home);
        ctx.keep_config_dir("here", "claude", &leading);
        let known = ctx.called_with("knowBroker").len();
        let e2 = refused(area.launch_adapter_for(&e, AdapterAsk::default()).await);
        assert_eq!(e2.kind, Some(Kind::Usage));
        assert!(e2.message.starts_with(&format!("Claude Code does not start with its config folder {leading}: ")), "{}", e2.message);
        assert_eq!(ctx.called_with("knowBroker").len(), known);
        assert_eq!(seen.borrow().len(), 2);
        // confine_setup alone says the same, and settles at once where nothing is kept for the agent named.
        assert_eq!(refused(area.confine_setup(&e, None).await).message, e2.message);
        area.confine_setup(&e, Some("codex")).await.expect("codex keeps no folder");
    });
}

// ---- A6, A7, A8: defaults and access -------------------------------------------------------------------------------------

// TS: thread-defaults.test.ts:92, thread-defaults.test.ts:104, thread-defaults.test.ts:180
#[test]
fn default_agent_of_skips_an_agent_turned_off() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        ctx.add_adapter("claude", plain_factory(FakeAdapter::new().arc()));
        ctx.add_adapter("codex", plain_factory(FakeAdapter::new().arc()));
        let (e, _) = entry(json!({}));
        let area = ctx.area();
        let mut prefs = default_preferences();

        // The catalog's first agent that runs; then the person's default; then the project's over both.
        assert_eq!(area.default_agent_of(&prefs, Some(&e)), "claude");
        prefs.default_agent = Some("codex".to_string());
        assert_eq!(area.default_agent_of(&prefs, Some(&e)), "codex");
        let project: ProjectOverrides = serde_json::from_value(json!({ "agent": "claude" })).expect("overrides");
        prefs.project_defaults.insert("p1".to_string(), project);
        assert_eq!(area.default_agent_of(&prefs, Some(&e)), "claude");
        let other: ProjectOverrides = serde_json::from_value(json!({ "agent": "codex" })).expect("overrides");
        prefs.default_agent = Some("claude".to_string());
        prefs.project_defaults.insert("p1".to_string(), other);
        assert_eq!(area.default_agent_of(&prefs, Some(&e)), "codex", "a project whose default agent is Codex runs Codex");

        // An agent turned off on that computer is skipped, wherever the default names it, and the default moves on.
        ctx.turn_off("here", "codex");
        assert_eq!(area.default_agent_of(&prefs, Some(&e)), "claude");
        ctx.turn_off("here", "claude");
        prefs.project_defaults.clear();
        assert_eq!(area.default_agent_of(&prefs, Some(&e)), "claude", "with nothing running the catalog's first is the answer");
        // With no workspace named, nothing is off.
        prefs.default_agent = Some("codex".to_string());
        assert_eq!(area.default_agent_of(&prefs, None), "codex");

        // An agent with no adapter runs nothing.
        let bare = FakeAgentsCtx::new();
        bare.add_adapter("codex", plain_factory(FakeAdapter::new().arc()));
        assert_eq!(bare.area().default_agent_of(&default_preferences(), None), "codex");
    });
}

fn claude_table() -> HarnessCatalog {
    table(
        "claude",
        "Claude Code",
        json!({
            "models": [{ "value": "claude-opus-5-5", "label": "Opus 5.5", "isDefault": true }, { "value": "claude-sonnet-5-5", "label": "Sonnet 5.5" }],
            "efforts": [{ "value": "low", "label": "Low" }, { "value": "high", "label": "High", "isDefault": true }],
            "permissionModes": [
                { "value": "default", "label": "Default" },
                { "value": "acceptEdits", "label": "Accept edits" },
                { "value": "bypassPermissions", "label": "Bypass", "isDefault": true },
            ],
            "access": { "ask": "default", "auto-edit": "acceptEdits", "full": "bypassPermissions" },
            "bypassMode": "bypassPermissions",
        }),
    )
}

fn agent_defaults(v: Value) -> wsp_wire::types::thread_defaults::AgentDefaults {
    serde_json::from_value(v).expect("agent defaults")
}

// TS: thread-defaults.test.ts:111, thread-defaults.test.ts:133, thread-defaults.test.ts:145, thread-defaults.test.ts:156
#[test]
fn defaults_on_marks_the_lists_for_what_a_new_thread_starts_on() {
    // The catalog's own marks stand where the person set nothing.
    let plain = AgentsArea::defaults_on(&claude_table(), &default_preferences(), None);
    let c = json_of(&plain.catalog);
    assert_eq!(marked(&c["models"]), Some("claude-opus-5-5".to_string()));
    assert_eq!(marked(&c["permissionModes"]), Some("bypassPermissions".to_string()));
    assert_eq!((plain.open_model, plain.open_effort), (None, None), "a closed list has nothing open to carry");
    assert!(c.get("unshaped").is_none(), "nothing shaped, so no unshaped lists beside");

    // The agent's defaults mark what a new thread starts on.
    let mut prefs = default_preferences();
    prefs.agent_defaults.insert("claude".to_string(), agent_defaults(json!({ "model": "claude-sonnet-5-5", "effort": "low", "access": "ask" })));
    let shaped = json_of(&AgentsArea::defaults_on(&claude_table(), &prefs, None).catalog);
    assert_eq!(marked(&shaped["models"]), Some("claude-sonnet-5-5".to_string()));
    assert_eq!(marked(&shaped["efforts"]), Some("low".to_string()));
    assert_eq!(marked(&shaped["permissionModes"]), Some("default".to_string()));

    // The project's model and access over the agent's.
    let project: ProjectOverrides = serde_json::from_value(json!({ "model": "claude-opus-5-5", "access": "auto-edit" })).expect("overrides");
    let over = json_of(&AgentsArea::defaults_on(&claude_table(), &prefs, Some(&project)).catalog);
    assert_eq!(marked(&over["models"]), Some("claude-opus-5-5".to_string()));
    assert_eq!(marked(&over["efforts"]), Some("low".to_string()));
    assert_eq!(marked(&over["permissionModes"]), Some("acceptEdits".to_string()));
}

fn marked(list: &Value) -> Option<String> {
    list.as_array()?.iter().find(|o| o["isDefault"] == true).map(|o| o["value"].as_str().expect("a value").to_string())
}

// TS: thread-defaults.test.ts:133, thread-defaults.test.ts:145
#[test]
fn defaults_on_hides_and_adds_the_persons_models_and_hands_the_agents_own_lists_beside() {
    let mut prefs = default_preferences();
    prefs.agent_defaults.insert("claude".to_string(), agent_defaults(json!({ "models": { "hide": ["claude-opus-5-5"], "custom": ["claude-opus-6-preview"] } })));
    let c = json_of(&AgentsArea::defaults_on(&claude_table(), &prefs, None).catalog);
    let models: Vec<String> = c["models"].as_array().expect("models").iter().map(|m| m["value"].as_str().expect("a value").to_string()).collect();
    assert!(!models.contains(&"claude-opus-5-5".to_string()), "a hidden model leaves the picker");
    assert!(models.contains(&"claude-opus-6-preview".to_string()), "a custom id is listed");
    assert_eq!(marked(&c["models"]), Some("claude-sonnet-5-5".to_string()), "the mark goes to the first shown");
    // The agent's own order and set stay in `unshaped`, so a client shapes a change off them.
    let own: Vec<String> = c["unshaped"]["models"].as_array().expect("unshaped models").iter().map(|m| m["value"].as_str().expect("a value").to_string()).collect();
    assert_eq!(own, ["claude-opus-5-5", "claude-sonnet-5-5"]);
}

// TS: thread-defaults.test.ts:123
#[test]
fn defaults_on_carries_what_an_open_list_cannot_mark() {
    let open = table("opencode", "OpenCode", json!({ "permissionModes": [{ "value": "auto", "label": "Auto" }] }));
    let mut prefs = default_preferences();
    prefs.agent_defaults.insert("opencode".to_string(), agent_defaults(json!({ "model": "anthropic/claude-sonnet-5", "effort": "high" })));
    let got = AgentsArea::defaults_on(&open, &prefs, None);
    assert_eq!(got.open_model.as_deref(), Some("anthropic/claude-sonnet-5"));
    assert_eq!(got.open_effort.as_deref(), Some("high"));
    // Another agent's model kept on the project does not carry over to this one.
    let project: ProjectOverrides = serde_json::from_value(json!({ "agent": "claude", "model": "claude-fable-5-1" })).expect("overrides");
    let dropped = AgentsArea::defaults_on(&open, &default_preferences(), Some(&project));
    assert_eq!(dropped.open_model, None);
}

// TS: permission-relay.test.ts:540
#[test]
fn named_mode_refuses_a_word_the_agent_has_no_mode_for() {
    let claude = claude_table();
    assert_eq!(AgentsArea::named_mode(Some(&claude), "claude", AccessChoice::Ask).expect("a mode"), "default");
    assert_eq!(AgentsArea::named_mode(Some(&claude), "claude", AccessChoice::AutoEdit).expect("a mode"), "acceptEdits");
    assert_eq!(AgentsArea::named_mode(Some(&claude), "claude", AccessChoice::Full).expect("a mode"), "bypassPermissions");

    // A word the agent maps to none of its modes: refused, naming the words it takes.
    let e = refused(AgentsArea::named_mode(Some(&claude), "claude", AccessChoice::Plan));
    assert_eq!(e.message, "Claude Code takes no plan access; it takes ask, auto-edit, full");
    assert_eq!(e.kind, Some(Kind::Usage));
    // A word mapped to a mode the agent does not list is no mode.
    let unlisted = table_without_mode();
    assert_eq!(refused(AgentsArea::named_mode(Some(&unlisted), "claude", AccessChoice::Full)).message, "Claude Code takes no full access; it takes ask, auto-edit");
    // An agent that maps none of wsp's words.
    let bare = table("codex", "Codex", json!({ "permissionModes": [{ "value": "read-only", "label": "Read only" }] }));
    assert_eq!(refused(AgentsArea::named_mode(Some(&bare), "codex", AccessChoice::Ask)).message, "Codex takes no access of wsp's words");
    // No catalog at all: the agent's label and no modes.
    let e = refused(AgentsArea::named_mode(None, "claude", AccessChoice::Ask));
    assert_eq!((e.message.as_str(), e.kind), ("Claude Code takes no access of wsp's words", Some(Kind::Usage)));
}

fn table_without_mode() -> HarnessCatalog {
    let mut v = json_of(&claude_table());
    let modes = v["permissionModes"].as_array().expect("modes").iter().filter(|m| m["value"] != "bypassPermissions").cloned().collect::<Vec<_>>();
    v["permissionModes"] = json!(modes);
    serde_json::from_value(v).expect("a catalog")
}

// ---- A9, A10: the files a turn carries --------------------------------------------------------------------------------------

const FOLDER: &str = "/work/app";
const DIR: &str = "/work/app/.wsp-files/t1/req_a.images";

// TS: session-images.test.ts:261, session-images.test.ts:272, session-images.test.ts:280, session-images.test.ts:373
#[test]
fn land_images_inline_lands_nothing() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (e, machine) = landing_entry(json!({}));
        let area = ctx.area();
        let images = [att("image/png", b"one", None), att("image/webp", b"two", None)];
        // Every image of the message in order, as bytes with its type, and no folder to answer with.
        let landed = area.land_images(&e, Some(AttachmentRoad::Inline), FOLDER, DIR, &images).await.expect("landed");
        assert_eq!(landed.dir, None);
        assert_eq!(json_of(&landed.images), json!([{ "mediaType": "image/png", "bytes": b64(b"one") }, { "mediaType": "image/webp", "bytes": b64(b"two") }]));
        // No image, or a harness that reads none: nothing is landed and nothing asked.
        let none = area.land_images(&e, Some(AttachmentRoad::File), FOLDER, DIR, &[]).await.expect("landed");
        assert!(none.images.is_empty() && none.dir.is_none());
        let no_road = area.land_images(&e, None, FOLDER, DIR, &images).await.expect("landed");
        assert!(no_road.images.is_empty() && no_road.dir.is_none());
        assert!(machine.commands().is_empty());
        assert!(machine.put.borrow().is_empty());
    });
}

// TS: session-images.test.ts:289, session-images.test.ts:329
#[test]
fn land_images_on_files_lands_each_in_the_sends_folder() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (e, machine) = landing_entry(json!({}));
        let area = ctx.area();
        let images = [att("image/png", b"one", None), att("image/jpeg", b"two", None), att("image/webp", b"three", None)];
        let landed = area.land_images(&e, Some(AttachmentRoad::File), FOLDER, DIR, &images).await.expect("landed");
        // The folder is readied as the thread's files are, in one command, under the inline exec's 20 s.
        assert_eq!(machine.commands(), [land_files_line(FOLDER, DIR)]);
        assert_eq!(runs(&machine, ".wsp-files"), [Some(INLINE_EXEC_MS)]);
        // Each image by its place in the message and its own type, one after another.
        let paths: Vec<String> = ["1.png", "2.jpg", "3.webp"].iter().map(|n| format!("{DIR}/{n}")).collect();
        assert_eq!(machine.put.borrow().clone(), vec![(paths[0].clone(), b"one".to_vec()), (paths[1].clone(), b"two".to_vec()), (paths[2].clone(), b"three".to_vec())]);
        assert_eq!(landed.dir.as_deref(), Some(DIR));
        assert_eq!(
            json_of(&landed.images),
            json!([
                { "mediaType": "image/png", "bytes": b64(b"one"), "path": paths[0] },
                { "mediaType": "image/jpeg", "bytes": b64(b"two"), "path": paths[1] },
                { "mediaType": "image/webp", "bytes": b64(b"three"), "path": paths[2] },
            ])
        );
    });
}

#[test]
fn land_images_fails_when_the_folder_is_not_made() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (e, machine) = landing_entry(json!({}));
        machine.answer(".wsp-files", 1, "");
        let e1 = refused(ctx.area().land_images(&e, Some(AttachmentRoad::File), FOLDER, DIR, &[att("image/png", b"one", None)]).await);
        assert_eq!(e1.message, files_not_landed_line(FOLDER));
        assert!(machine.put.borrow().is_empty(), "nothing is put where the folder could not be readied");
    });
}

// TS: session-images.test.ts:88, session-images.test.ts:105
#[test]
fn land_files_names_each_once_and_lands_in_order() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (e, machine) = landing_entry(json!({}));
        let area = ctx.area();
        let dir = "/work/app/.wsp-files/t1/req_a";
        // No file: nothing runs.
        assert!(area.land_files(&e, FOLDER, dir, &[]).await.expect("landed").is_empty());
        assert!(machine.commands().is_empty());

        // A name the client sent is one plain name inside the send's folder; a repeat is numbered, in order.
        let files = [
            att("application/pdf", b"first", Some("../../.ssh/authorized_keys")),
            att("application/pdf", b"second", Some("../../.ssh/authorized_keys")),
            att("application/pdf", b"third", Some("report.pdf")),
        ];
        let paths = area.land_files(&e, FOLDER, dir, &files).await.expect("landed");
        assert_eq!(paths, [format!("{dir}/authorized_keys"), format!("{dir}/2-authorized_keys"), format!("{dir}/report.pdf")]);
        assert_eq!(machine.commands(), [land_files_line(FOLDER, dir)], "one command readies the send's folder");
        assert_eq!(runs(&machine, ".wsp-files"), [Some(INLINE_EXEC_MS)]);
        assert_eq!(
            machine.put.borrow().clone(),
            vec![(paths[0].clone(), b"first".to_vec()), (paths[1].clone(), b"second".to_vec()), (paths[2].clone(), b"third".to_vec())]
        );

        // A folder that could not be readied: one line, and nothing put.
        let (blocked, blocked_machine) = landing_entry(json!({ "id": "ws2" }));
        blocked_machine.answer(".wsp-files", 1, "");
        let e2 = refused(area.land_files(&blocked, "/root/shots", "/root/shots/.wsp-files/t1/req_a", &files).await);
        assert_eq!(e2.message, files_not_landed_line("/root/shots"));
        assert!(blocked_machine.put.borrow().is_empty());
    });
}

// ---- A11, A12: taking them off again -----------------------------------------------------------------------------------------

fn files_in(ctx: &Rc<FakeAgentsCtx>, rows: &[(&str, &[&str])]) {
    ctx.thread_records.with(|r| {
        for (thread, folders) in rows {
            r.insert(thread.to_string(), record("ws1", json!({ "filesIn": folders })));
        }
    });
}

// TS: session-images.test.ts:121, session-images.test.ts:133
#[test]
fn drop_thread_files_runs_one_line_per_folder_on_a_running_machine() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (e, machine) = entry(json!({}));
        files_in(&ctx, &[("t1", &["/work/app"]), ("t2", &["/work/app", "/work/other"]), ("t3", &[]), ("t4", &["/work/app"])]);
        let area = ctx.area();
        // One line per folder, folders in first-seen order, each naming the threads that landed there.
        area.drop_thread_files(&e, &["t1".to_string(), "t2".to_string(), "t3".to_string(), "t9".to_string()]).await;
        assert_eq!(machine.commands(), [drop_files_line("/work/app", &["t1", "t2"]), drop_files_line("/work/other", &["t2"])]);
        assert_eq!(runs(&machine, "rm -rf"), [Some(INLINE_EXEC_MS), Some(INLINE_EXEC_MS)]);

        // A machine that is not running keeps them.
        let (napping, nap_machine) = entry(json!({ "id": "ws2", "phase": "napping" }));
        area.drop_thread_files(&napping, &["t1".to_string()]).await;
        assert!(nap_machine.commands().is_empty());
        // No thread with a folder: nothing asked.
        let (idle, idle_machine) = entry(json!({ "id": "ws3" }));
        area.drop_thread_files(&idle, &["t3".to_string()]).await;
        assert!(idle_machine.commands().is_empty());
    });
}

// TS: session-images.test.ts:121
#[test]
fn drop_thread_files_warns_and_goes_on() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (e, machine) = entry(json!({}));
        files_in(&ctx, &[("t1", &["/work/app"]), ("t2", &["/work/app", "/work/other"])]);
        machine.fail("/work/app", "the machine did not answer");
        // An exit code is not read: a line that exits 1 is no warning.
        machine.answer("/work/other", 1, "");
        let warned = console::capture();
        ctx.area().drop_thread_files(&e, &["t1".to_string(), "t2".to_string()]).await;
        assert_eq!(warned.lines(), ["attached files of t1, t2 not removed from /work/app: the machine did not answer"]);
        assert_eq!(machine.commands().len(), 2, "the next folder is still tried");
    });
}

// TS: session-images.test.ts:344, session-images.test.ts:357, session-images.test.ts:501
#[test]
fn drop_images_removes_the_dir_detached() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (e, machine) = entry(json!({}));
        let area = ctx.area();
        area.drop_images(&e, DIR);
        settle().await;
        assert_eq!(machine.commands(), [format!("rm -rf '{DIR}'")]);
        assert_eq!(runs(&machine, "rm -rf"), [Some(INLINE_EXEC_MS)]);

        // A machine that cannot be asked keeps the folder and says so, never failing the turn's end.
        machine.fail("rm -rf", "the machine is gone");
        let warned = console::capture();
        area.drop_images(&e, "/work/app/.wsp-files/t1/req_b.images");
        settle().await;
        assert_eq!(warned.lines(), ["images for a finished turn not removed from ws1: the machine is gone"]);
    });
}

// ---- A13: environments and stores ---------------------------------------------------------------------------------------------

// TS: thread-defaults.test.ts:225, box-panels.test.ts:638, usage.test.ts:494, box-folder.test.ts:250
#[test]
fn thread_env_puts_the_persons_variables_over_the_kinds() {
    let ctx = FakeAgentsCtx::new();
    *ctx.kind_env_base.borrow_mut() = env(&[("HOME", "/root"), ("CLAUDE_CONFIG_DIR", "/kind/claude"), ("FOO", "kind")]);
    ctx.set_setup(
        "here",
        "claude",
        FakeSetup { launch: SetupLaunch { env: env(&[("FOO", "bar"), ("CLAUDE_CONFIG_DIR", "/kept")]), launch: None }, ..Default::default() },
    );
    ctx.set_setup("here", "codex", FakeSetup { launch: SetupLaunch { env: env(&[("FOO", "codex"), ("CODEX_HOME", "/c")]), launch: None }, ..Default::default() });
    let (e, _) = entry(json!({}));
    let area = ctx.area();

    // A turn of one agent: its own variables on top of the kind's; the home the kind reads is the area's own.
    assert_eq!(
        pairs(&area.thread_env(&e, Some("claude"))),
        [pair("HOME", "/root"), pair("CLAUDE_CONFIG_DIR", "/kept"), pair("FOO", "bar"), pair("HOME_OF_CLAUDE", "/kind/ws1/claude")]
    );
    assert_eq!(ctx.called_with("kindEnv"), ["kindEnv ws1 claude"]);
    // Another agent's variables on the same computer are not carried.
    let codex = area.thread_env(&e, Some("codex"));
    assert_eq!(codex.get("FOO").map(String::as_str), Some("codex"));
    assert!(codex.get("CODEX_HOME").is_some());
    assert_eq!(codex.get("CLAUDE_CONFIG_DIR").map(String::as_str), Some("/kind/claude"));

    // A process that is no agent's, the terminal in a thread: every agent's, in catalog order, a later one's value winning.
    let terminal = area.thread_env(&e, None);
    assert_eq!(terminal.get("FOO").map(String::as_str), Some("codex"));
    assert_eq!(terminal.get("CLAUDE_CONFIG_DIR").map(String::as_str), Some("/kept"));
    assert_eq!(terminal.get("CODEX_HOME").map(String::as_str), Some("/c"));
    assert_eq!(ctx.called_with("kindEnv").last().map(String::as_str), Some("kindEnv ws1 -"));

    // A fork at a provider has no setup of the person's: the kind's environment alone.
    let (cloud, _) = entry(json!({ "id": "ws9", "kind": "cloud" }));
    assert_eq!(pairs(&area.thread_env(&cloud, Some("claude"))), [pair("HOME", "/root"), pair("CLAUDE_CONFIG_DIR", "/kind/claude"), pair("FOO", "kind"), pair("HOME_OF_CLAUDE", "/kind/ws9/claude")]);
}

#[test]
fn agent_home_is_the_kept_config_folder_else_the_kinds() {
    let ctx = FakeAgentsCtx::new();
    ctx.keep_config_dir("here", "claude", "/kept");
    ctx.kind_homes.borrow_mut().insert("codex".to_string(), "/kind/codex".to_string());
    let (e, _) = entry(json!({}));
    let (cloud, _) = entry(json!({ "id": "ws9", "kind": "cloud" }));
    let area = ctx.area();
    assert_eq!(area.agent_home(&e, "claude"), "/kept");
    assert_eq!(area.agent_home(&e, "codex"), "/kind/codex");
    assert_eq!(area.agent_home(&cloud, "claude"), "/kind/ws9/claude", "nothing kept at a provider");
}

// TS: places-setup.test.ts:481
#[test]
fn place_stores_reads_variable_then_folder_then_default() {
    let ctx = FakeAgentsCtx::new();
    let area = ctx.area();
    // What the person set for the variable wins; then the folder they kept; then the kind's own for that computer.
    ctx.set_setup("srv", "claude", FakeSetup { launch: SetupLaunch { env: env(&[("CLAUDE_CONFIG_DIR", "/set/by/variable")]), launch: None }, config_dir: Some("/kept/claude".to_string()), ..Default::default() });
    ctx.keep_config_dir("srv", "codex", "/kept/codex");
    let stores = area.place_stores("srv", "/home/maya");
    assert_eq!(stores.get("claude").map(String::as_str), Some("/set/by/variable"));
    assert_eq!(stores.get("codex").map(String::as_str), Some("/kept/codex"));
    assert_eq!(stores.len(), 2, "only the agents with a store variable");

    ctx.setups.borrow_mut().clear();
    ctx.place_agent_homes.borrow_mut().insert(("srv".to_string(), "claude".to_string()), "/root/.claude-cfg".to_string());
    ctx.place_agent_homes.borrow_mut().insert(("srv".to_string(), "codex".to_string()), "/var/lib/wsp/logins/codex".to_string());
    let stores = area.place_stores("srv", "/root");
    assert_eq!(stores.get("claude").map(String::as_str), Some("/root/.claude-cfg"));
    assert_eq!(stores.get("codex").map(String::as_str), Some("/var/lib/wsp/logins/codex"));
}

// ---- A14: reading the agent's config files ---------------------------------------------------------------------------------

async fn configs(area: &AgentsArea, e: &Entry, agent: &str) -> Result<Option<(LaunchServerRoad, Value)>, HostError> {
    area.turn_configs_on(e, agent, "/work/app").await.map(|got| {
        got.map(|c| {
            let user = c.configs.user.as_ref().map(|f| json!({ "path": f.path, "text": f.text }));
            let projects = c.configs.projects.as_ref().map_or(0, |p| p.len());
            (c.road, json!({ "user": user, "projects": projects, "folder": c.configs.folder }))
        })
    })
}

// TS: box-folder.test.ts:340
#[test]
fn turn_configs_on_reads_the_checkout_and_the_agents_files_as_the_turns_login() {
    local(async {
        let (ctx, e, machine) = serving();
        reads(&machine, TRACKER);
        let area = ctx.area();
        let (road, read) = configs(&area, &e, "claude").await.expect("read").expect("a road");
        assert_eq!(road, LaunchServerRoad::Claude);
        assert_eq!(read, json!({ "user": { "path": CLAUDE_CONFIG, "text": TRACKER }, "projects": 0, "folder": "/work/app" }));
        // The checkout is asked first, the config files after, each under the inline exec's 20 s.
        let execs = machine.execs.borrow().clone();
        assert_eq!(execs.len(), 2);
        assert!(execs[0].0.starts_with("git ") && execs[0].0.ends_with(" 2>/dev/null"), "{}", execs[0].0);
        assert!(execs[0].0.contains("/work/app"));
        assert!(execs[1].0.contains("wsp_mcp_read"));
        assert_eq!((execs[0].1, execs[1].1), (Some(INLINE_EXEC_MS), Some(INLINE_EXEC_MS)));

        // A checkout read that fails reads as none, and the files are still read.
        let (ctx, e, machine) = serving();
        machine.fail("rev-parse", "the machine did not answer");
        reads(&machine, TRACKER);
        assert!(configs(&ctx.area(), &e, "claude").await.expect("read").is_some());

        // An agent with no launch road, or that is no MCP agent, reads nothing.
        let (ctx, e, machine) = serving();
        assert!(configs(&ctx.area(), &e, "opencode").await.expect("none").is_none());
        assert!(machine.commands().is_empty());
    });
}

// TS: box-folder.test.ts:402, box-folder.test.ts:415, box-folder.test.ts:428
#[test]
fn turn_configs_on_refuses_a_cut_read_an_unread_read_and_a_scope_left() {
    local(async {
        // A read that exits non-zero.
        let (ctx, e, machine) = serving();
        machine.answer("wsp_mcp_read", 124, &config_read(TRACKER));
        let said = refused(configs(&ctx.area(), &e, "claude").await).message;
        assert!(said.starts_with("Claude Code's MCP servers on that computer could not be read for the values its launch hands them ("), "{said}");
        assert!(said.ends_with("), so the turn did not start; try again once it answers"), "{said}");
        assert!(!said.contains("PERSONS-OWN") && !said.contains("mcp.linear.app"), "no line of that config: {said}");

        // A read that exits 0 and prints no non-zero exit but no end mark: cut, the files named, nothing of them said.
        let (ctx, e, machine) = serving();
        machine.answer("wsp_mcp_read", 0, &config_read(TRACKER)[..60]);
        let said = refused(configs(&ctx.area(), &e, "claude").await).message;
        assert!(said.starts_with("Claude Code's MCP servers on that computer could not be read whole for the values its launch hands them: "), "{said}");
        assert!(said.contains(CLAUDE_CONFIG) && said.contains(" together, once encoded, pass the "), "{said}");
        assert!(said.ends_with(", so the turn did not start; make them smaller and send again"), "{said}");
        assert!(!said.contains("PERSONS-OWN"));

        // Output that does not parse: no scope printed the mark.
        let (ctx, e, machine) = serving();
        machine.answer("wsp_mcp_read", 0, "garbage\nwsp-mcp end\n");
        assert!(refused(configs(&ctx.area(), &e, "claude").await).message.starts_with("Claude Code's MCP servers on that computer could not be read for the values its launch hands them ("));

        // A file left unread, the word in place of its bytes: the file named, and what the person does.
        let (ctx, e, machine) = serving();
        machine.answer("wsp_mcp_read", 0, "wsp-mcp 0 0 -plain\nwsp-mcp end\n");
        let said = refused(configs(&ctx.area(), &e, "claude").await).message;
        assert!(
            said.contains(&wsp_words::agents_report::config_unread_line(CLAUDE_CONFIG, "it is not a plain file, or it is a link out of the folder it belongs to, and wsp reads neither")),
            "{said}"
        );
        assert!(said.ends_with("so the turn did not start; make it a plain file under 32 MB, or a link to one inside its folder, and send again"), "{said}");
    });
}

// ---- A15, A17: the values a launch hands the agent's servers ---------------------------------------------------------------

// TS: box-folder.test.ts:340, box-folder.test.ts:372, box-folder.test.ts:442
#[test]
fn server_values_for_fills_only_on_a_launch_kind_with_vault_values() {
    local(async {
        let vault = env(&[("LINEAR_TOKEN", "lin_TESTONLY"), ("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat-fake")]);

        // Another kind hands servers its own way: nothing, and nothing is read there.
        for road in [ServerValuesRoad::None, ServerValuesRoad::Environment] {
            let (ctx, e, machine) = serving();
            ctx.kind_server_values.set(road);
            *ctx.vault.borrow_mut() = vault.clone();
            reads(&machine, TRACKER);
            assert!(ctx.area().server_values_for(&e, "claude", "/work/app").await.expect("none").is_none());
            assert!(machine.commands().is_empty());
        }

        // A vault with no server's value: nothing, and nothing is read.
        let (ctx, e, machine) = serving();
        *ctx.vault.borrow_mut() = env(&[("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat-fake")]);
        assert!(ctx.area().server_values_for(&e, "claude", "/work/app").await.expect("none").is_none());
        assert!(machine.commands().is_empty());

        // Claude Code: each server that reads a value the vault holds, its whole entry filled.
        let user = r#"{"mcpServers":{"tracker":{"type":"http","url":"https://mcp.linear.app/mcp","headers":{"Authorization":"Bearer ${LINEAR_TOKEN:-}"},"oauth":{"clientId":"abc","callbackPort":8080}},"mine":{"command":"mine-mcp","env":{"X":"${NOT_HELD}"}}}}"#;
        let (ctx, e, machine) = serving();
        *ctx.vault.borrow_mut() = vault.clone();
        reads(&machine, user);
        let got = ctx.area().server_values_for(&e, "claude", "/work/app").await.expect("read");
        assert_eq!(
            got,
            Some(wsp_adapters::port::ServerValues {
                entries: Some(serde_json::from_value(json!({ "tracker": { "type": "http", "url": "https://mcp.linear.app/mcp", "headers": { "Authorization": "Bearer lin_TESTONLY" }, "oauth": { "clientId": "abc", "callbackPort": 8080 } } })).expect("entries")),
                config: None,
            })
        );

        // Codex: the keys of its thread's start.
        let notion = "[mcp_servers.notion]\ncommand = \"npx\"\nenv_vars = [\"NOTION_TOKEN\"]\ndisabled_tools = [\"delete_page\"]\n";
        let (ctx, e, machine) = serving();
        *ctx.vault.borrow_mut() = env(&[("NOTION_TOKEN", "ntn_TESTONLY"), ("OPENAI_API_KEY", "sk-openai-fake")]);
        reads(&machine, notion);
        let got = ctx.area().server_values_for(&e, "codex", "/work/app").await.expect("read");
        assert_eq!(got, Some(wsp_adapters::port::ServerValues { entries: None, config: Some(env(&[("mcp_servers.notion.env.NOTION_TOKEN", "ntn_TESTONLY")])) }));

        // Every value empty reads as none.
        let (ctx, e, machine) = serving();
        *ctx.vault.borrow_mut() = env(&[("OTHER_TOKEN", "x")]);
        reads(&machine, TRACKER);
        assert!(ctx.area().server_values_for(&e, "claude", "/work/app").await.expect("none").is_none());
        assert_eq!(runs(&machine, "wsp_mcp_read").len(), 1, "the config was read to find that out");
    });
}

// TS: cloud-server-off.test.ts:88
#[test]
fn launch_servers_for_asks_each_only_where_the_adapter_takes_it() {
    local(async {
        let (ctx, e, machine) = serving();
        *ctx.vault.borrow_mut() = env(&[("LINEAR_TOKEN", "lin_TESTONLY")]);
        reads(&machine, TRACKER);
        let area = ctx.area();

        // An adapter that takes neither: nothing is read.
        let none = area.launch_servers_for(&e, "claude", &FakeAdapter::new(), "/work/app").await.expect("read");
        assert!(none.server_values.is_none() && none.off_servers.is_none());
        assert!(machine.commands().is_empty());

        // One that takes servers gets the vault's values, and offServers stays unset off a provider.
        let mut takes = FakeAdapter::new();
        takes.mcp_servers = true;
        let got = area.launch_servers_for(&e, "claude", &takes, "/work/app").await.expect("read");
        assert_eq!(
            got.server_values,
            Some(wsp_adapters::port::ServerValues {
                entries: Some(serde_json::from_value(json!({ "tracker": { "type": "http", "url": "https://mcp.linear.app/mcp", "headers": { "Authorization": "Bearer lin_TESTONLY" } } })).expect("entries")),
                config: None,
            })
        );
        assert!(got.off_servers.is_none());

        // One that denies servers where the machine is at a provider gets the list, `[]` where none is off.
        let (ctx, e, machine) = provider("boat");
        reads(&machine, r#"{"mcpServers":{"stub":{"command":"node","args":["s.js"]}}}"#);
        let mut denies = FakeAdapter::new();
        denies.denies_servers = true;
        let got = ctx.area().launch_servers_for(&e, "claude", &denies, "/work/app").await.expect("read");
        assert!(got.server_values.is_none());
        assert_eq!(got.off_servers, Some(vec![]));
    });
}

// ---- A16: the servers a cloud launch leaves out -----------------------------------------------------------------------------

const CLOUD_CONFIG: &str = r#"{"mcpServers":{"cloudflare":{"type":"http","url":"https://mcp.cloudflare.com/mcp"},"context7":{"type":"http","url":"https://mcp.context7.com/mcp"},"linear":{"type":"http","url":"https://mcp.linear.app/mcp","headers":{"X-Client":"acme"}},"stub":{"command":"node","args":["s.js"]}}}"#;

/// A live workspace at a provider's place, and the context that knows it.
fn provider(place: &str) -> (Rc<FakeAgentsCtx>, Entry, Arc<FakeMachine>) {
    let (ctx, e, machine) = serving();
    ctx.provider_places.borrow_mut().insert(place.to_string());
    ctx.place_ids.borrow_mut().insert("ws1".to_string(), place.to_string());
    (ctx, e, machine)
}

// TS: cloud-server-off.test.ts:88
#[test]
fn off_servers_for_reads_once_and_drops_on_an_agents_change() {
    local(async {
        let (ctx, e, machine) = provider("boat");
        reads(&machine, CLOUD_CONFIG);
        // The host's probe as it stands: cloudflare answered 401, context7 answered, nothing else has been asked.
        *ctx.signs.borrow_mut() = Some(IndexMap::from([("https://mcp.cloudflare.com/mcp".to_string(), true), ("https://mcp.context7.com/mcp".to_string(), false)]));
        let area = ctx.area();
        let read = |m: &FakeMachine| runs(m, "wsp_mcp_read").len();

        // Each server the config names that needs the person's sign-in; one that sends a fixed header or runs a command stays.
        assert_eq!(area.off_servers_for(&e, "claude", "/work/app").await, Some(vec!["cloudflare".to_string()]));
        let asked = ctx.called_with("signsIn");
        assert_eq!(asked, ["signsIn https://mcp.cloudflare.com/mcp,https://mcp.context7.com/mcp"], "only the addresses with no header of their own are asked");
        assert_eq!(read(&machine), 1);
        // Read once per machine, folder and agent.
        assert_eq!(area.off_servers_for(&e, "claude", "/work/app").await, Some(vec!["cloudflare".to_string()]));
        assert_eq!(read(&machine), 1);
        area.off_servers_for(&e, "claude", "/work/other").await;
        assert_eq!(read(&machine), 2, "another folder is its own read");

        // A change of another workspace's servers drops nothing; this one's drops its reads.
        ctx.emit(json!({ "type": "agents.changed", "target": { "workspaceId": "ws-other" } }));
        ctx.emit(json!({ "type": "agents.changed", "target": { "placeId": "srv" } }));
        area.off_servers_for(&e, "claude", "/work/app").await;
        assert_eq!(read(&machine), 2);
        ctx.emit(json!({ "type": "agents.changed", "target": { "workspaceId": "ws1" } }));
        area.off_servers_for(&e, "claude", "/work/app").await;
        area.off_servers_for(&e, "claude", "/work/other").await;
        assert_eq!(read(&machine), 4, "every folder of the workspace reads again");

        // A change that names no target drops every read; so do a delete and an upgrade of the workspace.
        ctx.emit(json!({ "type": "agents.changed" }));
        area.off_servers_for(&e, "claude", "/work/app").await;
        assert_eq!(read(&machine), 5);
        ctx.emit(json!({ "type": "workspace.deleted", "workspaceId": "ws1" }));
        area.off_servers_for(&e, "claude", "/work/app").await;
        assert_eq!(read(&machine), 6);
        ctx.emit(json!({ "type": "workspace.upgraded", "workspaceId": "ws1", "machineId": "m-new" }));
        area.off_servers_for(&e, "claude", "/work/app").await;
        assert_eq!(read(&machine), 7);
        ctx.emit(json!({ "type": "workspace.deleted", "workspaceId": "ws-other" }));
        area.off_servers_for(&e, "claude", "/work/app").await;
        assert_eq!(read(&machine), 7);

        // A probe that no longer asks for a sign-in leaves nothing out, and says so with an empty list.
        *ctx.signs.borrow_mut() = Some(IndexMap::from([("https://mcp.cloudflare.com/mcp".to_string(), false)]));
        assert_eq!(area.off_servers_for(&e, "claude", "/work/app").await, Some(vec![]));
        // A host with no reader asks nobody: nothing is off.
        *ctx.signs.borrow_mut() = None;
        assert_eq!(area.off_servers_for(&e, "claude", "/work/app").await, Some(vec![]));
    });
}

// TS: cloud-server-off.test.ts:195
#[test]
fn off_servers_for_answers_nothing_off_a_provider() {
    local(async {
        // A computer the person joined reads no config for it and leaves nothing out.
        let (ctx, e, machine) = serving();
        ctx.place_ids.borrow_mut().insert("ws1".to_string(), "srv".to_string());
        reads(&machine, CLOUD_CONFIG);
        assert_eq!(ctx.area().off_servers_for(&e, "claude", "/work/app").await, None);
        assert!(machine.commands().is_empty());
    });
}

// TS: cloud-server-off.test.ts:88
#[test]
fn off_servers_for_warns_and_reads_again_after_a_failure() {
    local(async {
        let (ctx, e, machine) = provider("boat");
        machine.answer("wsp_mcp_read", 1, "");
        let area = ctx.area();
        let warned = console::capture();
        // A config that cannot be read costs the launch its list, never the turn.
        assert_eq!(area.off_servers_for(&e, "claude", "/work/app").await, None);
        let lines = warned.lines();
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(
            lines[0].starts_with("claude on m-ws1: its servers that need a sign-in were not read (Claude Code's MCP servers on that computer could not be read for the values its launch hands them ("),
            "{}",
            lines[0]
        );
        assert!(lines[0].ends_with("); the launch leaves none out"), "{}", lines[0]);

        // The failure is not kept: the next ask reads again, and can answer.
        machine.rules.borrow_mut().clear();
        reads(&machine, CLOUD_CONFIG);
        *ctx.signs.borrow_mut() = Some(IndexMap::from([("https://mcp.cloudflare.com/mcp".to_string(), true)]));
        assert_eq!(area.off_servers_for(&e, "claude", "/work/app").await, Some(vec!["cloudflare".to_string()]));
        assert_eq!(runs(&machine, "wsp_mcp_read").len(), 2);
        assert_eq!(warned.lines().len(), 1);
    });
}

// TS: cloud-server-off.test.ts:141
#[test]
fn off_servers_for_leaves_out_aws_mcp_whose_login_stayed_on_the_persons_computer() {
    local(async {
        let (ctx, e, machine) = provider("boat");
        let image = json!({ "mcpServers": {
            "aws-mcp": { "command": "uvx", "args": ["mcp-proxy-for-aws@latest", "https://aws-mcp.us-east-1.api.aws/mcp", "--metadata", "INSTALL_SOURCE=aws-cli"] },
            "aws-installed": { "command": "/root/.local/bin/mcp-proxy-for-aws", "args": ["https://aws-mcp.us-east-1.api.aws/mcp"] },
            "aws-readme": { "command": "uvx", "args": ["mcp-proxy-for-aws-cli@latest", "https://aws-mcp.us-east-1.api.aws/mcp"] },
            "aws-wrapped": { "command": "sh", "args": ["-c", "uvx mcp-proxy-for-aws@latest https://aws-mcp.us-east-1.api.aws/mcp"] },
            "aws-unsigned": { "command": "uvx", "args": ["mcp-proxy-for-aws==1.1.0", "https://aws-mcp.us-east-1.api.aws/mcp", "--skip-auth"] },
            "aws-wrapped-unsigned": { "command": "sh", "args": ["-c", "uvx mcp-proxy-for-aws@latest https://aws-mcp.us-east-1.api.aws/mcp --skip-auth"] },
            "stub": { "command": "node", "args": ["s.js"] },
        } });
        reads(&machine, &image.to_string());
        *ctx.signs.borrow_mut() = Some(IndexMap::new());
        assert_eq!(
            ctx.area().off_servers_for(&e, "claude", "/work/app").await,
            Some(vec!["aws-mcp".to_string(), "aws-installed".to_string(), "aws-readme".to_string(), "aws-wrapped".to_string()]),
            "kept where it sends unsigned"
        );
    });
}

// ---- A18: the adapter ------------------------------------------------------------------------------------------------------------

// TS: thread-defaults.test.ts:92 (the no adapter line), runtime.test.ts's registry refusals
#[test]
fn adapter_for_refuses_an_agent_with_no_adapter() {
    let ctx = FakeAgentsCtx::new();
    ctx.add_adapter("codex", plain_factory(FakeAdapter::new().arc()));
    ctx.add_adapter("cursor", plain_factory(FakeAdapter::new().arc()));
    let (e, _) = entry(json!({}));
    let area = ctx.area();
    let e1 = refused(area.adapter_for(&e, AdapterAsk { named: Some("aider".to_string()), ..Default::default() }));
    assert_eq!(e1.message, "no adapter registered for harness \"aider\"; agents on this host: codex, cursor");
    assert_eq!(e1.kind, None, "a plain error");
    // Unnamed means the default agent, which has no adapter here either.
    assert_eq!(refused(area.adapter_for(&e, AdapterAsk::default())).message, "no adapter registered for harness \"claude\"; agents on this host: codex, cursor");
    let bare = FakeAgentsCtx::new();
    assert_eq!(refused(bare.area().adapter_for(&e, AdapterAsk::default())).message, "no adapter registered for harness \"claude\"; agents on this host: none");
}

// TS: thread-defaults.test.ts:225, cloud-server-off.test.ts:88
#[test]
fn adapter_for_layers_env_servers_broker_then_turn() {
    let ctx = FakeAgentsCtx::new();
    *ctx.kind_env_base.borrow_mut() = env(&[("HOME", "/root"), ("A", "kind")]);
    ctx.set_setup(
        "here",
        "claude",
        FakeSetup { launch: SetupLaunch { env: env(&[("A", "person"), ("B", "person")]), launch: Some(AgentLaunch { program: Some("/opt/claude".to_string()), args: Some(vec!["--debug".to_string()]) }) }, ..Default::default() },
    );
    ctx.kind_server_values.set(ServerValuesRoad::Environment);
    ctx.kind_memory_keys.borrow_mut().insert("claude".to_string(), "memory-key".to_string());
    ctx.kind_login_stands.borrow_mut().insert("claude".to_string());
    *ctx.vault_on.borrow_mut() = env(&[("V", "vault")]);
    let adapter = FakeAdapter::new().arc();
    let (f, seen) = factory(adapter);
    ctx.add_adapter("claude", f);
    let (e, machine) = entry(json!({}));
    let area = ctx.area();

    // The person's variables over the computer's own and under the turn's, which only wsp sets; the servers' values
    // first, since a kind that hands them by environment gets them no other way.
    let ask = AdapterAsk { turn_env: Some(env(&[("B", "turn")])), servers: env(&[("S", "server")]), ..Default::default() };
    let (harness, _) = area.adapter_for(&e, ask).expect("built");
    assert_eq!(harness, "claude");
    {
        let seen = seen.borrow();
        let c = &seen[0];
        assert_eq!(c.machine.id(), machine.id);
        assert_eq!(c.workspace_id, "ws1");
        assert_eq!(pairs(&c.env), [pair("S", "server"), pair("HOME", "/root"), pair("A", "person"), pair("HOME_OF_CLAUDE", "/kind/ws1/claude"), pair("B", "turn")]);
        assert_eq!(c.vault, env(&[("V", "vault")]));
        assert_eq!(c.launch, Some(AgentLaunch { program: Some("/opt/claude".to_string()), args: Some(vec!["--debug".to_string()]) }));
        assert_eq!(c.project_key.as_deref(), Some("memory-key"));
        assert_eq!(c.sign_in_refusal, sign_in_refusal_line(Some(&WorkspaceKind::Local)));
        assert!(c.brokered.is_none() && c.codex_broker.is_none());
        assert_eq!((c.home)("claude"), "/kind/ws1/claude");
        assert!((c.login_stands)("claude") && !(c.login_stands)("codex"));
        assert_eq!(Arc::as_ptr(&c.exec_stream) as *const (), Arc::as_ptr(&ctx.exec_factory) as *const ());
    }
    assert_eq!(ctx.called_with("brokeredLaunch"), ["brokeredLaunch ws1 claude -"]);
    assert_eq!(ctx.exec_factories.borrow().clone(), vec![("ws1".to_string(), TurnLaunch::default(), false)]);

    // A kind that does not hand servers by environment carries none of their values.
    ctx.kind_server_values.set(ServerValuesRoad::Launch);
    area.adapter_for(&e, AdapterAsk { servers: env(&[("S", "server")]), ..Default::default() }).expect("built");
    assert_eq!(pairs(&seen.borrow()[1].env), [pair("HOME", "/root"), pair("A", "person"), pair("HOME_OF_CLAUDE", "/kind/ws1/claude"), pair("B", "person")]);

    // A workspace whose credentials the host keeps: the turn token in place of the vault and no server value; the
    // broker's own variables between the person's and the turn's.
    ctx.kind_server_values.set(ServerValuesRoad::Environment);
    *ctx.brokered.borrow_mut() = Some(BrokeredLaunch {
        vault: env(&[("BV", "brokered-vault")]),
        env: env(&[("A", "broker"), ("BR", "broker")]),
        claude: Some(Brokered { token: "tok".to_string(), kind: BrokeredKind::Token }),
        codex: None,
    });
    let ask = AdapterAsk { turn_env: Some(env(&[(HOST_TOKEN_ENV, "host-tok"), ("B", "turn")])), servers: env(&[("S", "server")]), ..Default::default() };
    area.adapter_for(&e, ask).expect("built");
    let seen = seen.borrow();
    let c = &seen[2];
    assert_eq!(pairs(&c.env), [pair("HOME", "/root"), pair("A", "broker"), pair("HOME_OF_CLAUDE", "/kind/ws1/claude"), pair("B", "turn"), pair("BR", "broker"), pair(HOST_TOKEN_ENV, "host-tok")]);
    assert_eq!(c.vault, env(&[("BV", "brokered-vault")]));
    assert_eq!(c.brokered, Some(Brokered { token: "tok".to_string(), kind: BrokeredKind::Token }));
    assert_eq!(ctx.called_with("brokeredLaunch").last().map(String::as_str), Some("brokeredLaunch ws1 claude host-tok"));
}

#[test]
fn adapter_for_groups_a_thread_only_where_the_kind_ends_threads_and_hands_the_waiting_reader() {
    let ctx = FakeAgentsCtx::new();
    ctx.add_adapter("claude", plain_factory(FakeAdapter::new().arc()));
    let (e, _) = entry(json!({}));
    let area = ctx.area();
    let thread = |asks: bool| AdapterAsk { thread: Some("t1".to_string()), asks_until_stopped: asks, ..Default::default() };

    area.adapter_for(&e, thread(false)).expect("built");
    // A kind that ends threads groups the turn under its thread; a line's try says it asks until stopped.
    ctx.kind_ends_threads.set(true);
    area.adapter_for(&e, thread(false)).expect("built");
    area.adapter_for(&e, thread(true)).expect("built");
    ctx.kind_ends_threads.set(false);
    area.adapter_for(&e, thread(true)).expect("built");
    let launches: Vec<TurnLaunch> = ctx.exec_factories.borrow().iter().map(|(_, t, _)| t.clone()).collect();
    assert_eq!(
        launches,
        [
            TurnLaunch::default(),
            TurnLaunch { thread: Some("t1".to_string()), asks_until_stopped: false, killed_why: false },
            TurnLaunch { thread: Some("t1".to_string()), asks_until_stopped: true, killed_why: false },
            TurnLaunch { thread: None, asks_until_stopped: true, killed_why: false },
        ]
    );
}

// ---- A19: where a new thread's lists are read -------------------------------------------------------------------------------

// TS: older-agent-lists.test.ts:100, older-agent-lists.test.ts:126
#[test]
fn project_home_is_none_off_a_folder_kind_or_a_folder_not_held() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        ctx.folder_computers.borrow_mut().insert("mac".to_string());
        ctx.projects.borrow_mut().extend([
            ProjectSeen { id: "p1".to_string(), computer: "mac".to_string() },
            ProjectSeen { id: "p-box".to_string(), computer: "cloud".to_string() },
            ProjectSeen { id: "p-unheld".to_string(), computer: "mac".to_string() },
        ]);
        let (folder, _) = entry(json!({ "id": "ws-folder" }));
        ctx.folders.borrow_mut().insert("p1".to_string(), folder);
        let (box_folder, _) = entry(json!({ "id": "ws-box" }));
        ctx.folders.borrow_mut().insert("p-box".to_string(), box_folder);
        let area = ctx.area();

        // The record a new thread runs on where it runs in the project's folder.
        assert_eq!(area.project_home("p1", None).await.expect("resolved").map(|e| e.record.id), Some("ws-folder".to_string()));
        // A project whose threads run on machines of their own has no folder to ask.
        assert!(area.project_home("p-box", None).await.expect("resolved").is_none());
        // A folder that cannot be held now is no folder, and the failure is dropped.
        assert!(area.project_home("p-unheld", None).await.expect("resolved").is_none());
        // A project nobody lists is the door's own refusal.
        assert_eq!(refused(area.project_home("p-none", None).await).message, "no such project p-none");
    });
}

// TS: older-agent-lists.test.ts:126
#[test]
fn computer_home_is_the_folder_of_the_first_project_there_that_runs_in_a_folder() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        ctx.folder_computers.borrow_mut().insert("mac".to_string());
        ctx.projects.borrow_mut().extend([
            ProjectSeen { id: "p0".to_string(), computer: "other".to_string() },
            ProjectSeen { id: "p1".to_string(), computer: "mac".to_string() },
            ProjectSeen { id: "p2".to_string(), computer: "mac".to_string() },
            ProjectSeen { id: "p3".to_string(), computer: "cloud".to_string() },
        ]);
        for (project, workspace) in [("p0", "ws0"), ("p1", "ws1"), ("p2", "ws2"), ("p3", "ws3")] {
            let (e, _) = entry(json!({ "id": workspace }));
            ctx.folders.borrow_mut().insert(project.to_string(), e);
        }
        let area = ctx.area();
        assert_eq!(area.computer_home("mac", None).await.expect("listed").map(|e| e.record.id), Some("ws1".to_string()), "the first listed on that computer");
        assert!(area.computer_home("nowhere", None).await.expect("listed").is_none(), "a computer holding no such project");
        assert!(area.computer_home("cloud", None).await.expect("listed").is_none(), "a computer whose threads run on machines of their own");
        assert!(area.computer_home("other", None).await.expect("listed").is_none());
    });
}
