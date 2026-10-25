// SPDX-License-Identifier: AGPL-3.0-only
//! agents_lists (spec/agents_lists.md): what each agent offers on a machine, and thread titles. Expected values read
//! off agents.ts at 65f6b89c7 and the TS tests named above each test.
#[path = "support_b/mod.rs"]
mod support;

use std::rc::Rc;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use serde_json::{json, Value};
use support::agents_ctx::{entry, plain_factory, table, Ability, FakeAdapter, FakeAgentsCtx, Gate};
use support::{json_of, local, row, sessions_of, settle, silent, view, FakeMachine};
use wsp_adapters::port::{AttachmentRoad, ExecFailure, ExecFailureKind, HarnessCatalogAnswer, HarnessCatalogModelProbe, HarnessCatalogProbe, SessionRenameWrite};
use wsp_core::console;
use wsp_threads::threads::agents::lists::{
    known_catalog, source_of, title_rows, CarriedTitle, AGENT_LISTS, AGENT_VERSION_READ_MS, CATALOG_PROBE_TIMEOUT_MS, CATALOG_TTL_MS, FIRST_RUN_READ_MS,
    SESSION_TITLE_REFRESH_MAX, SESSION_TITLE_TIMEOUT_MS, SESSION_TITLE_TTL_MS, TITLE_MAKE_TIMEOUT_MS,
};
use wsp_wire::views::harness::{HarnessCatalog, ScreenCommand, ScreenControl};
use wsp_wire::views::session::TitleSource;

const OPENING: &str = "make a server, and its tests";
const LOST_HEAD: &str = "Your earlier session for this thread was lost, so it continues here in a new one.";

// ---- fixtures ----------------------------------------------------------------------------------------------------

fn claude_table() -> HarnessCatalog {
    table(
        "claude",
        "Claude Code",
        json!({
            "version": "--help 2.1.257, 2026-10-10",
            "models": [
                { "value": "claude-opus-5-5", "label": "Opus 5.5", "isDefault": true },
                { "value": "claude-sonnet-5-5", "label": "Sonnet 5.5" },
            ],
            "efforts": [{ "value": "low", "label": "Low" }, { "value": "high", "label": "High", "isDefault": true }],
            "permissionModes": [
                { "value": "default", "label": "Default" },
                { "value": "bypassPermissions", "label": "Bypass", "isDefault": true },
            ],
            "smallModel": "claude-haiku-5-5",
        }),
    )
}

fn codex_table() -> HarnessCatalog {
    table(
        "codex",
        "Codex",
        json!({
            "version": "app-server 0.153.0, 2026-09-07",
            "models": [
                { "value": "gpt-5.6-sol", "label": "GPT-5.6-Sol", "isDefault": true },
                { "value": "gpt-5.6-terra", "label": "GPT-5.6-Terra" },
            ],
            "efforts": [{ "value": "low", "label": "Low", "isDefault": true }, { "value": "high", "label": "High" }],
            "permissionModes": [{ "value": "read-only", "label": "Read only", "description": "Reads only; edits no files and runs no command that writes" }],
        }),
    )
}

fn model(slug: &str, label: &str, is_default: bool, efforts: &[&str]) -> HarnessCatalogModelProbe {
    HarnessCatalogModelProbe {
        slug: slug.to_string(),
        label: label.to_string(),
        description: None,
        efforts: Some(efforts.iter().map(|e| e.to_string()).collect()),
        default_effort: None,
        context_windows: vec![],
        is_default,
        fast: None,
    }
}

fn probe(version: &str) -> HarnessCatalogProbe {
    HarnessCatalogProbe {
        version: Some(version.to_string()),
        models: vec![model("claude-opus-5-5", "Opus", true, &["low", "high"]), model("claude-haiku-4-5", "Haiku 4.5", false, &[])],
        efforts: vec!["low".into(), "high".into()],
        permission_modes: vec!["default".into(), "bypassPermissions".into()],
    }
}

fn answered(version: &str) -> Ability<Option<HarnessCatalogAnswer>> {
    Ability::answering(Some(HarnessCatalogAnswer::Probe(probe(version))))
}

fn values(c: &Value, list: &str) -> Vec<String> {
    c[list].as_array().expect("a list").iter().map(|m| m["value"].as_str().expect("a value").to_string()).collect()
}

fn other(message: &str) -> ExecFailure {
    ExecFailure { kind: ExecFailureKind::Other, message: message.to_string() }
}

/// The machine's commands that contain `needle`, each with the timeout it was run under.
fn runs(machine: &FakeMachine, needle: &str) -> Vec<Option<u64>> {
    machine.execs.borrow().iter().filter(|(c, _)| c.contains(needle)).map(|(_, t)| *t).collect()
}

/// A machine whose `claude` command is a wrapper whose first run installs the agent.
fn installing(machine: &FakeMachine) {
    machine.rules.borrow_mut().clear();
    machine.answer("head -c 4096", 0, "\u{1e}#!/bin/sh\n# ascii-harness-shim\nexec /usr/local/lib/ascii-harnesses/claude \"$@\"\n\u{1e}");
    machine.answer("ascii-lazy-harness", 0, "1\n");
}

/// A live running workspace in the context, answering for `harness` with this adapter.
fn running(ctx: &Rc<FakeAgentsCtx>, harness: &str, adapter: &Arc<FakeAdapter>) -> (wsp_threads::threads::threads::Entry, Arc<FakeMachine>) {
    let (e, machine) = entry(json!({}));
    ctx.add_entry(e.clone());
    ctx.add_adapter(harness, plain_factory(adapter.clone()));
    (e, machine)
}

fn seed(ctx: &Rc<FakeAgentsCtx>, rows: Vec<Value>) {
    *ctx.sessions.borrow_mut() = sessions_of(rows.into_iter().enumerate().map(|(i, v)| row(view(v), &format!("u{i}"))).collect()).borrow().clone();
}

fn row_json(ctx: &Rc<FakeAgentsCtx>, id: &str) -> Value {
    json_of(&ctx.sessions.borrow().get(id).expect("a row").borrow().view)
}

/// The calls that keep, push or write a title, in order.
fn effects(ctx: &Rc<FakeAgentsCtx>) -> Vec<String> {
    ctx.called().into_iter().filter(|c| ["persist", "pushHead", "writeSession"].iter().any(|p| c.starts_with(p))).collect()
}

async fn ready_within<T>(mut f: LocalBoxFuture<'static, T>) -> Option<T> {
    for _ in 0..50 {
        if let std::task::Poll::Ready(v) = futures::poll!(&mut f) {
            return Some(v);
        }
        tokio::task::yield_now().await;
    }
    None
}

// ---- L1: the catalog the adapter's abilities make ------------------------------------------------------------------

#[test]
fn the_constants_are_the_ts_values() {
    assert_eq!(CATALOG_TTL_MS, 300_000);
    assert_eq!(CATALOG_PROBE_TIMEOUT_MS, 25_000);
    assert_eq!(FIRST_RUN_READ_MS, 3_000);
    assert_eq!(AGENT_VERSION_READ_MS, 3_000);
    assert_eq!(SESSION_TITLE_TTL_MS, 10_000);
    assert_eq!(SESSION_TITLE_TIMEOUT_MS, 15_000);
    assert_eq!(SESSION_TITLE_REFRESH_MAX, 20);
    assert_eq!(TITLE_MAKE_TIMEOUT_MS, 25_000);
    assert_eq!(AGENT_LISTS, "agent-lists");
}

#[test]
fn known_carries_the_adapters_abilities_in_order() {
    let mut adapter = FakeAdapter::new().steering();
    adapter.rename_session = Some(Ability::answering(SessionRenameWrite::Written));
    adapter.attachments = Some(AttachmentRoad::File);
    adapter.mcp_servers = true;
    adapter.moves_access = true;
    adapter.steers_images = true;
    adapter.aside = true;
    adapter.compacts = Some("/compact".to_string());
    adapter.terminal_resume = Some("claude --resume".to_string());
    adapter.forks = true;
    adapter.reverts = true;
    adapter.forks_by_count = true;
    adapter.screen_commands = vec![ScreenCommand { name: "model".into(), control: ScreenControl::Model }];
    let known = json_of(&known_catalog(&claude_table(), &adapter));
    // The table's own keys stand, and what the adapter says is laid over them.
    assert_eq!(known["harness"], "claude");
    assert_eq!(known["source"], "table");
    assert_eq!(known["version"], "--help 2.1.257, 2026-10-10");
    assert_eq!(known["steers"], true);
    assert_eq!(known["renames"], true);
    assert_eq!(known["images"], true);
    assert_eq!(known["mcpServers"], true);
    assert_eq!(known["movesAccess"], true);
    assert_eq!(known["steersImages"], true);
    assert_eq!(known["asides"], true);
    assert_eq!(known["compacts"], "/compact");
    assert_eq!(known["terminalResume"], "claude --resume");
    assert_eq!(known["rewindsConversation"], true);
    assert_eq!(known["rewindsByCount"], true);
    assert_eq!(known["forks"], true);
    assert_eq!(known["forksByCount"], true);
    assert_eq!(known["screenCommands"], json!([{ "name": "model", "control": "model" }]));

    // An adapter that does none of it says no, and sets none of the optional keys.
    let bare = json_of(&known_catalog(&claude_table(), &FakeAdapter::new()));
    assert_eq!(bare["steers"], false);
    assert_eq!(bare["renames"], false);
    assert_eq!(bare["images"], false);
    assert_eq!(bare["asides"], false);
    for key in ["mcpServers", "movesAccess", "steersImages", "compacts", "terminalResume", "rewindsConversation", "rewindsByCount", "forks", "forksByCount", "screenCommands"] {
        assert!(bare.get(key).is_none(), "{key} is absent");
    }
}

// TS: aside.test.ts:209, aside.test.ts:218, aside.test.ts:227
#[test]
fn known_says_asides_compacts_and_the_terminal_command_off_the_adapter_alone() {
    let mut adapter = FakeAdapter::new();
    adapter.aside = true;
    adapter.compacts = Some("/compact".to_string());
    adapter.terminal_resume = Some("claude --resume".to_string());
    let with = json_of(&known_catalog(&claude_table(), &adapter));
    assert_eq!(with["asides"], true);
    assert_eq!(with["compacts"], "/compact");
    assert_eq!(with["terminalResume"], "claude --resume");
    let without = json_of(&known_catalog(&claude_table(), &FakeAdapter::new()));
    assert_eq!(without["asides"], false);
    assert!(without.get("compacts").is_none());
    assert!(without.get("terminalResume").is_none());
}

// TS: rewind.test.ts:386, rewind.test.ts:393, rewind.test.ts:401, fork.test.ts:453
#[test]
fn known_says_what_a_rewind_and_a_fork_can_do_by_what_the_adapter_has() {
    let mut reverting = FakeAdapter::new();
    reverting.reverts = true;
    let c = json_of(&known_catalog(&claude_table(), &reverting));
    assert_eq!((c["rewindsConversation"].clone(), c["rewindsByCount"].clone()), (json!(true), json!(true)));
    assert!(c.get("forks").is_none());

    let mut copying = FakeAdapter::new();
    copying.forks = true;
    let c = json_of(&known_catalog(&claude_table(), &copying));
    assert_eq!(c["rewindsConversation"], true);
    assert!(c.get("rewindsByCount").is_none(), "a copy is not a rewind by count");
    assert_eq!(c["forks"], true);
    assert!(c.get("forksByCount").is_none());

    let mut counting = FakeAdapter::new();
    counting.forks = true;
    counting.forks_by_count = true;
    let c = json_of(&known_catalog(&claude_table(), &counting));
    assert_eq!((c["forks"].clone(), c["forksByCount"].clone()), (json!(true), json!(true)));

    let none = json_of(&known_catalog(&claude_table(), &FakeAdapter::new()));
    assert!(none.get("rewindsConversation").is_none(), "a harness that keeps its own history cuts no conversation");
}

// TS: runtime-steer.test.ts:457, session-title.test.ts:814, session-title.test.ts:853
#[test]
fn known_says_steers_and_renames_off_the_adapter_and_catalog_on_carries_them() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (e, _) = entry(json!({}));
        let steering = FakeAdapter::new().steering().arc();
        let plain = FakeAdapter::new().arc();
        let area = ctx.area();
        let c = json_of(&area.catalog_on(&claude_table(), &e, steering).await);
        assert_eq!((c["steers"].clone(), c["renames"].clone()), (json!(true), json!(false)));
        let c = json_of(&area.catalog_on(&codex_table(), &e, plain).await);
        assert_eq!((c["steers"].clone(), c["renames"].clone()), (json!(false), json!(false)));
        let mut naming = FakeAdapter::new();
        naming.rename_session = Some(Ability::answering(SessionRenameWrite::Written));
        let c = json_of(&area.catalog_on(&claude_table(), &entry(json!({ "id": "ws2" })).0, Arc::new(naming)).await);
        assert_eq!(c["renames"], true);
    });
}

#[test]
fn an_adapter_with_no_probe_answers_known() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (e, machine) = entry(json!({}));
        let adapter = FakeAdapter::new().steering().arc();
        let area = ctx.area();
        let got = area.catalog_on(&claude_table(), &e, adapter.clone()).await;
        assert_eq!(json_of(&got), json_of(&known_catalog(&claude_table(), &*adapter)));
        assert_eq!(json_of(&got)["source"], "table");
        assert!(machine.commands().is_empty(), "the machine is not asked");
        assert!(ctx.store.logged().is_empty());
    });
}

// ---- L2: the probe, once per machine and TTL -----------------------------------------------------------------------

// TS: runtime-history.test.ts:445
#[test]
fn catalog_on_probes_once_per_machine_and_ttl() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (a, machine_a) = entry(json!({ "id": "ws1" }));
        let (b, _machine_b) = entry(json!({ "id": "ws2" }));
        let adapter = FakeAdapter::new().probing(answered("2.1.257").execing("claude --help")).arc();
        let area = ctx.area();
        let table = claude_table();

        let first = json_of(&area.catalog_on(&table, &a, adapter.clone()).await);
        assert_eq!(first["source"], "harness");
        assert_eq!(first["version"], "2.1.257");
        assert_eq!(values(&first, "models"), ["claude-opus-5-5", "claude-haiku-4-5"]);
        assert_eq!(first["models"][0], json!({ "value": "claude-opus-5-5", "label": "Opus 5.5", "isDefault": true, "efforts": ["low", "high"], "contextWindows": [] }));
        assert_eq!(first["models"][1], json!({ "value": "claude-haiku-4-5", "label": "Haiku 4.5", "efforts": [], "contextWindows": [] }));
        assert_eq!(values(&first, "efforts"), ["low", "high"]);
        assert_eq!(values(&first, "permissionModes"), ["default", "bypassPermissions"]);
        assert_eq!(runs(&machine_a, "claude --help"), [Some(25_000)], "one probe, under its 25 s");

        let again = json_of(&area.catalog_on(&table, &a, adapter.clone()).await);
        assert_eq!(again, first);
        assert_eq!(adapter.called(), ["probeCatalog"]);

        area.catalog_on(&table, &b, adapter.clone()).await;
        assert_eq!(adapter.called().len(), 2, "another machine is probed on its own");

        ctx.clock.advance(CATALOG_TTL_MS as i64 - 1);
        area.catalog_on(&table, &a, adapter.clone()).await;
        assert_eq!(adapter.called().len(), 2, "inside the TTL nothing is asked");
        ctx.clock.advance(1);
        area.catalog_on(&table, &a, adapter.clone()).await;
        assert_eq!(adapter.called().len(), 3, "at the TTL the binary is asked again");
    });
}

#[test]
fn catalog_on_keeps_the_answer_for_the_next_host() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (a, _) = entry(json!({ "id": "ws1" }));
        let adapter = FakeAdapter::new().probing(answered("2.1.257")).arc();
        let area = ctx.area();
        area.catalog_on(&claude_table(), &a, adapter.clone()).await;
        settle().await;
        // The answer is kept, not the catalog: what wsp itself says of the agent is this build's.
        assert_eq!(ctx.store.get_now(AGENT_LISTS, "m-ws1:claude"), Some(json_of(&probe("2.1.257"))));
        assert_eq!(ctx.store.logged(), ["put agent-lists/m-ws1:claude"]);

        // A put that fails is said, once, and costs the answer nothing.
        let warned = console::capture();
        ctx.store.fail_put.set(true);
        let (b, _) = entry(json!({ "id": "ws2" }));
        let got = area.catalog_on(&claude_table(), &b, adapter.clone()).await;
        settle().await;
        assert_eq!(json_of(&got)["version"], "2.1.257");
        assert_eq!(
            warned.lines(),
            ["claude on m-ws2: its lists were not kept for the next host (the disk is full); its first send there waits on the probe"]
        );
    });
}

// TS: runtime-history.test.ts:612
#[test]
fn catalog_on_answers_kept_lists_while_the_binary_answers_their_version() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (a, machine) = entry(json!({ "id": "ws1" }));
        let area = ctx.area();

        // A host before this one heard 2.1.257 there; the binary says it is still 2.1.257: those lists answer at
        // once, while the probe (held here) is still out.
        ctx.store.seed(AGENT_LISTS, "m-ws1:claude", json_of(&probe("2.1.257")));
        let gate = Gate::new();
        let held = FakeAdapter::new()
            .probing(answered("2.1.300").gated(&gate))
            .versioned(Ability::answering(Some("2.1.257".to_string())).execing("claude --version"))
            .arc();
        let got = ready_within(area.catalog_on(&claude_table(), &a, held.clone())).await.expect("answered before the probe");
        let got = json_of(&got);
        assert_eq!((got["source"].clone(), got["version"].clone()), (json!("harness"), json!("2.1.257")));
        assert_eq!(runs(&machine, "claude --version"), [Some(3_000)], "the version is read under its 3 s");
        gate.open();
        settle().await;
        // The probe settles, and its catalog is what the entry holds from then on.
        let later = json_of(&area.catalog_on(&claude_table(), &a, held.clone()).await);
        assert_eq!(later["version"], "2.1.300");

        // A binary that says another version has no kept lists standing: the probe answers.
        let ctx = FakeAgentsCtx::new();
        let (b, _) = entry(json!({ "id": "ws1" }));
        ctx.store.seed(AGENT_LISTS, "m-ws1:claude", json_of(&probe("2.1.257")));
        let updated = FakeAdapter::new().probing(answered("2.1.300")).versioned(Ability::answering(Some("2.1.300".to_string()))).arc();
        let got = json_of(&ctx.area().catalog_on(&claude_table(), &b, updated).await);
        assert_eq!(got["version"], "2.1.300");

        // Lists from the table are the lists whatever the binary says.
        let ctx = FakeAgentsCtx::new();
        let (c, _) = entry(json!({ "id": "ws1" }));
        let fresh = FakeAdapter::new().probing(answered("2.1.300")).arc();
        ctx.area().catalog_on(&claude_table(), &c, fresh.clone()).await;
        assert!(!fresh.called().contains(&"probeVersion".to_string()));
    });
}

// TS: runtime-history.test.ts:612
#[test]
fn catalog_on_past_the_ttl_answers_the_held_lists_while_the_binary_is_asked_again() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (a, _) = entry(json!({ "id": "ws1" }));
        let gate = Gate::new();
        let adapter = FakeAdapter::new()
            .probing(Ability::steps(vec![(Ok(Some(HarnessCatalogAnswer::Probe(probe("2.1.257")))), None), (Ok(None), Some(gate.clone()))]))
            .versioned(Ability::answering(Some("2.1.257".to_string())))
            .arc();
        let area = ctx.area();
        let table = claude_table();
        let first = json_of(&area.catalog_on(&table, &a, adapter.clone()).await);
        assert_eq!(first["source"], "harness");
        ctx.clock.advance(CATALOG_TTL_MS as i64);
        // The binary still says 2.1.257, so the held lists answer; the probe it is asked again with is not waited for.
        let held = ready_within(area.catalog_on(&table, &a, adapter.clone())).await.expect("the held lists answer");
        assert_eq!(json_of(&held), first);
        assert_eq!(adapter.called(), ["probeCatalog", "probeCatalog", "probeVersion"]);
        // The binary then gives nothing: the table is the next answer, and the one after reads it without asking.
        gate.open();
        settle().await;
        let next = json_of(&area.catalog_on(&table, &a, adapter.clone()).await);
        assert_eq!(next["source"], "table");
        assert_eq!(adapter.called().len(), 3);
    });
}

// TS: runtime-history.test.ts:640
#[test]
fn catalog_on_skips_the_probe_when_the_first_run_installs() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (a, machine) = entry(json!({ "id": "ws1" }));
        installing(&machine);
        let adapter = FakeAdapter::new().probing(answered("2.1.257").execing("claude --help")).versioned(Ability::answering(Some("2.1.257".to_string())).execing("claude --version")).arc();
        let area = ctx.area();
        let table = claude_table();

        // A command whose first run installs its agent is not asked: the table answers as no binary.
        let got = json_of(&area.catalog_on(&table, &a, adapter.clone()).await);
        settle().await;
        assert_eq!(got["source"], "table");
        assert!(runs(&machine, "claude --help").is_empty());
        assert!(runs(&machine, "claude --version").is_empty());
        assert!(ctx.store.logged().is_empty(), "nothing is kept");

        // The skipped probe dropped its entry once it settled, so the next ask looks again; the agent is there now.
        machine.rules.borrow_mut().clear();
        let next = json_of(&area.catalog_on(&table, &a, adapter.clone()).await);
        assert_eq!(next["source"], "harness");
        assert_eq!(runs(&machine, "claude --help"), [Some(25_000)]);
    });
}

// TS: runtime-history.test.ts:640
#[test]
fn catalog_on_reads_no_version_once_the_command_reads_as_installing_again() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (a, machine) = entry(json!({ "id": "ws1" }));
        let adapter = FakeAdapter::new().probing(answered("2.1.257").execing("claude --help")).versioned(Ability::answering(Some("2.1.257".to_string())).execing("claude --version")).arc();
        let area = ctx.area();
        let first = json_of(&area.catalog_on(&claude_table(), &a, adapter.clone()).await);
        assert_eq!(first["source"], "harness");
        installing(&machine);
        ctx.clock.advance(CATALOG_TTL_MS as i64);
        area.catalog_on(&claude_table(), &a, adapter.clone()).await;
        settle().await;
        assert!(runs(&machine, "claude --version").is_empty(), "no binary is run to read its version");
        assert_eq!(runs(&machine, "claude --help").len(), 1, "nor to probe it");
    });
}

// TS: runtime-history.test.ts:534
#[test]
fn catalog_on_answers_the_refusal_with_known_lists() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (a, _) = entry(json!({ "id": "ws1" }));
        let said = "Codex is not signed in where this thread runs; run codex login there";
        let adapter = FakeAdapter::new().probing(Ability::answering(Some(HarnessCatalogAnswer::Refused(said.to_string())))).arc();
        let got = json_of(&ctx.area().catalog_on(&codex_table(), &a, adapter.clone()).await);
        settle().await;
        let mut expected = json_of(&known_catalog(&codex_table(), &*adapter));
        expected["refusal"] = json!(said);
        assert_eq!(got, expected);
        assert_eq!(got["source"], "table");
        assert_eq!(values(&got, "models"), ["gpt-5.6-sol", "gpt-5.6-terra"], "the table's models stand");
        assert!(ctx.store.logged().is_empty(), "a refusal is not kept for the next host");
    });
}

// TS: runtime-history.test.ts:547, runtime-history.test.ts:666
#[test]
fn catalog_on_warns_on_a_failed_probe_and_answers_the_table() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (a, machine) = entry(json!({ "id": "ws1" }));
        let words = "timeoutMs must be at most 26000 for a dedicated sandbox";
        let adapter = FakeAdapter::new().probing(Ability::sequence(vec![Err(other(words))]).execing("claude --help")).arc();
        let area = ctx.area();
        let warned = console::capture();
        let table = claude_table();
        let got = json_of(&area.catalog_on(&table, &a, adapter.clone()).await);
        assert_eq!(got["source"], "table");
        assert_eq!(got["version"], "--help 2.1.257, 2026-10-10");
        assert_eq!(values(&got, "models"), ["claude-opus-5-5", "claude-sonnet-5-5"]);
        let line = format!("claude on m-ws1: the probe of the agent failed ({words}); wsp's built-in list answers until the next probe");
        assert_eq!(warned.lines(), [line.clone()]);

        // The cache holds the failure too: one line and one exec however often the composer opens.
        area.catalog_on(&table, &a, adapter.clone()).await;
        assert_eq!(warned.lines(), [line.clone()]);
        assert_eq!(adapter.called(), ["probeCatalog"]);
        assert_eq!(runs(&machine, "claude --help").len(), 1);

        // Past the TTL it asks again, and says it again.
        ctx.clock.advance(CATALOG_TTL_MS as i64);
        let later = json_of(&area.catalog_on(&table, &a, adapter.clone()).await);
        assert_eq!(later["source"], "table");
        assert_eq!(adapter.called().len(), 2);
        assert_eq!(warned.lines(), [line.clone(), line]);
    });
}

// TS: runtime-history.test.ts:666
#[test]
fn catalog_on_answers_the_table_when_the_binary_gives_nothing_and_does_not_ask_again_within_the_ttl() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (a, _) = entry(json!({ "id": "ws1" }));
        let adapter = FakeAdapter::new().probing(Ability::answering(None)).arc();
        let area = ctx.area();
        let first = json_of(&area.catalog_on(&claude_table(), &a, adapter.clone()).await);
        assert_eq!((first["source"].clone(), first["version"].clone()), (json!("table"), json!("--help 2.1.257, 2026-10-10")));
        assert_eq!(values(&first, "models"), ["claude-opus-5-5", "claude-sonnet-5-5"]);
        area.catalog_on(&claude_table(), &a, adapter.clone()).await;
        assert_eq!(adapter.called(), ["probeCatalog"]);
        ctx.clock.advance(CATALOG_TTL_MS as i64);
        let later = json_of(&area.catalog_on(&claude_table(), &a, adapter.clone()).await);
        assert_eq!(adapter.called().len(), 2);
        assert_eq!(later["source"], "table");
    });
}

// TS: runtime-history.test.ts:505
#[test]
fn each_adapter_decides_whether_its_binary_is_asked_and_each_harness_keeps_its_answer() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (a, machine) = entry(json!({ "id": "ws1" }));
        let claude = FakeAdapter::new().probing(answered("2.1.257").execing("claude --help")).arc();
        let codex_probe = HarnessCatalogProbe {
            version: Some("0.9.0".to_string()),
            models: vec![model("gpt-5-codex", "Codex", true, &["high"])],
            efforts: vec!["low".into(), "high".into()],
            permission_modes: vec!["read-only".into()],
        };
        let codex = FakeAdapter::new().probing(Ability::answering(Some(HarnessCatalogAnswer::Probe(codex_probe))).execing("codex --describe")).arc();
        let area = ctx.area();
        let c = json_of(&area.catalog_on(&claude_table(), &a, claude.clone()).await);
        let x = json_of(&area.catalog_on(&codex_table(), &a, codex.clone()).await);
        assert_eq!((c["source"].clone(), c["version"].clone()), (json!("harness"), json!("2.1.257")));
        assert_eq!((x["source"].clone(), x["version"].clone(), x["label"].clone()), (json!("harness"), json!("0.9.0"), json!("Codex")));
        assert!(x.get("isDefault").is_none());
        assert_eq!(x["models"], json!([{ "value": "gpt-5-codex", "label": "Codex", "isDefault": true, "efforts": ["high"], "contextWindows": [] }]));
        // The table lends the words for values the binary only names.
        assert_eq!(x["permissionModes"], json!([{ "value": "read-only", "label": "Read only", "description": "Reads only; edits no files and runs no command that writes" }]));
        assert_eq!(runs(&machine, "claude --help").len(), 1);
        assert_eq!(runs(&machine, "codex --describe").len(), 1);

        // An adapter with no probe line leaves the machine alone, whatever its harness id.
        let quiet = FakeAdapter::new().arc();
        let q = json_of(&area.catalog_on(&claude_table(), &entry(json!({ "id": "ws2" })).0, quiet.clone()).await);
        assert_eq!(q["source"], "table");
        assert!(quiet.called().is_empty());
    });
}

// TS: runtime-history.test.ts:725
#[test]
fn catalog_on_keeps_one_answer_per_machine() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (a, _) = entry(json!({ "id": "a" }));
        let (b, _) = entry(json!({ "id": "b" }));
        let adapter = FakeAdapter::new().probing(answered("2.1.257")).arc();
        let area = ctx.area();
        for e in [&a, &a, &b] {
            area.catalog_on(&claude_table(), e, adapter.clone()).await;
        }
        assert_eq!(adapter.called().len(), 2);
        assert_eq!(ctx.store.logged(), ["put agent-lists/m-a:claude", "put agent-lists/m-b:claude"]);
    });
}

#[test]
fn catalog_on_asks_nothing_while_closing() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (a, machine) = entry(json!({ "id": "ws1" }));
        let adapter = FakeAdapter::new().probing(answered("2.1.257").execing("claude --help")).arc();
        let area = ctx.area();
        ctx.closing.set(true);
        // No entry: wsp's own lists answer and nothing is asked of the machine or the store.
        let got = json_of(&area.catalog_on(&claude_table(), &a, adapter.clone()).await);
        assert_eq!(got["source"], "table");
        assert!(adapter.called().is_empty());
        assert!(machine.commands().is_empty());

        // A stale entry's lists answer, and past the TTL no new probe goes out.
        ctx.closing.set(false);
        let first = json_of(&area.catalog_on(&claude_table(), &a, adapter.clone()).await);
        assert_eq!(first["source"], "harness");
        ctx.clock.advance(CATALOG_TTL_MS as i64 + 1);
        ctx.closing.set(true);
        let held = json_of(&area.catalog_on(&claude_table(), &a, adapter.clone()).await);
        assert_eq!(held, first);
        assert_eq!(adapter.called(), ["probeCatalog"]);
    });
}

// TS: title-after-close.test.ts:124, title-after-close.test.ts:146
#[test]
fn a_probe_in_flight_is_an_ask_a_close_waits_for_until_it_settles() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (a, _) = entry(json!({ "id": "ws1" }));
        let gate = Gate::new();
        let adapter = FakeAdapter::new().probing(answered("2.1.257").gated(&gate)).arc();
        let area = ctx.area();
        assert!(area.agent_asks().is_empty());
        let asking = tokio::task::spawn_local({
            let area = area.clone();
            let table = claude_table();
            let adapter = adapter.clone();
            async move { area.catalog_on(&table, &a, adapter).await }
        });
        settle().await;
        assert_eq!(area.agent_asks().len(), 1, "the probe is held");
        gate.open();
        let got = asking.await.expect("the ask ran");
        assert_eq!(json_of(&got)["source"], "harness");
        settle().await;
        assert!(area.agent_asks().is_empty(), "a settled ask is dropped");
    });
}

// ---- L3: the account's models --------------------------------------------------------------------------------------

// TS: sub-codex-models.test.ts:38, sub-codex-models.test.ts:67, sub-codex-models.test.ts:73, sub-codex-models.test.ts:89, vault.test.ts:510
#[test]
fn account_models_answer_where_the_binary_said_nothing_and_keep_the_tables_default() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (a, _) = entry(json!({ "id": "ws1" }));
        let (b, _) = entry(json!({ "id": "ws2" }));
        let astra = model("gpt-6-astra", "GPT-6-Astra", true, &["low"]);
        let sol = model("gpt-5.6-sol", "GPT-5.6-Sol", false, &["low"]);
        // The binary on the first machine described nothing; the second's names a version first.
        let adapter = FakeAdapter::new()
            .probing(Ability::sequence(vec![Ok(None), Ok(None)]))
            .accounting(Ability::answering(Some(vec![astra, sol])))
            .arc();
        let area = ctx.area();
        let got = json_of(&area.catalog_on(&codex_table(), &a, adapter.clone()).await);
        // The account's list, its first row not the default where the table's default is listed there.
        assert_eq!(got["source"], "harness");
        assert_eq!(got["version"], Value::Null);
        assert_eq!(values(&got, "models"), ["gpt-6-astra", "gpt-5.6-sol"]);
        assert_eq!(got["models"][0].get("isDefault"), None);
        assert_eq!(got["models"][1]["isDefault"], true);
        assert_eq!(values(&got, "efforts"), ["low", "high"], "the table's efforts stand");
        assert_eq!(values(&got, "permissionModes"), ["read-only"]);
        assert_eq!(adapter.called().iter().filter(|c| c.starts_with("accountModels")).count(), 1);

        // One read per TTL serves every machine, a failed one included.
        area.catalog_on(&codex_table(), &b, adapter.clone()).await;
        assert_eq!(adapter.called().iter().filter(|c| c.starts_with("accountModels")).count(), 1);
        ctx.clock.advance(CATALOG_TTL_MS as i64);
        let (c, _) = entry(json!({ "id": "ws3" }));
        area.catalog_on(&codex_table(), &c, adapter.clone()).await;
        assert_eq!(adapter.called().iter().filter(|c| c.starts_with("accountModels")).count(), 2);
    });
}

// TS: sub-codex-models.test.ts:45, sub-codex-models.test.ts:89
#[test]
fn account_models_that_cannot_be_read_answer_the_table_and_the_newest_heard_version_keys_the_read() {
    local(async {
        // A read that failed is none: the table's models answer, and the failure is kept for the TTL.
        let ctx = FakeAgentsCtx::new();
        let (a, _) = entry(json!({ "id": "ws1" }));
        let failing = FakeAdapter::new().probing(Ability::answering(None)).accounting(Ability::failing("chatgpt.com unreachable")).arc();
        let area = ctx.area();
        let got = json_of(&area.catalog_on(&codex_table(), &a, failing.clone()).await);
        assert_eq!(got["source"], "table");
        assert_eq!(values(&got, "models"), ["gpt-5.6-sol", "gpt-5.6-terra"]);
        let (b, _) = entry(json!({ "id": "ws2" }));
        area.catalog_on(&codex_table(), &b, failing.clone()).await;
        assert_eq!(failing.called().iter().filter(|c| c.starts_with("accountModels")).count(), 1);

        // The newest version a binary named is the one the account is read for, before a new machine's binary is asked.
        let ctx = FakeAgentsCtx::new();
        let (a, _) = entry(json!({ "id": "ws1" }));
        let (b, _) = entry(json!({ "id": "ws2" }));
        let probing = FakeAdapter::new()
            .probing(Ability::sequence(vec![Ok(Some(HarnessCatalogAnswer::Probe(probe("0.163.0")))), Ok(None)]))
            .accounting(Ability::answering(Some(vec![model("gpt-6.1-sol", "GPT-6.1-Sol", true, &["low"])])))
            .arc();
        let area = ctx.area();
        area.catalog_on(&codex_table(), &a, probing.clone()).await;
        let got = json_of(&area.catalog_on(&codex_table(), &b, probing.clone()).await);
        assert_eq!(values(&got, "models"), ["gpt-6.1-sol"]);
        assert!(probing.called().contains(&"accountModels 0.163.0".to_string()), "{:?}", probing.called());
    });
}

// ---- L4: a first run that installs ---------------------------------------------------------------------------------

#[test]
fn first_run_here_reads_the_installer_without_running_the_agent() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let (e, machine) = entry(json!({}));
        let area = ctx.area();
        assert!(!area.first_run_here(&e, "claude").await, "a binary that is there does not install");
        let reads = machine.execs.borrow().clone();
        assert!(!reads.is_empty());
        assert!(reads.iter().all(|(_, timeout)| *timeout == Some(3_000)), "each read runs under 3 s: {reads:?}");
        assert!(reads.iter().all(|(cmd, _)| !cmd.contains("claude --")), "the agent itself is never run: {reads:?}");

        installing(&machine);
        assert!(area.first_run_here(&e, "claude").await);

        // An exec that fails is no answer.
        machine.rules.borrow_mut().clear();
        machine.fail("head -c 4096", "the machine did not answer");
        assert!(!area.first_run_here(&e, "claude").await);

        // An agent with no command in the catalog reads nothing.
        let (other_machine_entry, other) = entry(json!({ "id": "ws2" }));
        assert!(!area.first_run_here(&other_machine_entry, "aider").await);
        assert!(other.commands().is_empty());
    });
}

// ---- L5: reading a row's title off the harness's store ---------------------------------------------------------------

fn titled(title: &str) -> Ability<Option<String>> {
    Ability::answering(Some(title.to_string())).execing("wsp-title-read S1")
}

fn thread_rows() -> Vec<Value> {
    vec![
        json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING, "startedAt": 10 }),
        json!({ "id": "r2", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING, "startedAt": 20 }),
        json!({ "id": "r3", "threadId": "t2", "claudeSessionId": "S9", "prompt": "another thread", "startedAt": 30 }),
    ]
}

// TS: session-title.test.ts:137, session-title.test.ts:147, session-title.test.ts:168, session-row.test.ts:227
#[test]
fn refresh_title_reads_once_per_window_and_keeps_it_on_every_row_of_the_session() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new().titled(titled("Building the server")).arc();
        let (_, machine) = running(&ctx, "claude", &adapter);
        seed(&ctx, thread_rows());
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r2").expect("a row").borrow().view.clone();

        area.refresh_title(&v, false).await;
        assert_eq!(adapter.called(), ["sessionTitle S1"]);
        assert_eq!(runs(&machine, "wsp-title-read S1"), [Some(15_000)], "the store read runs under 15 s");
        // The title is not the opening words, so it is the person's or the harness's own, and stands on both rows.
        for id in ["r1", "r2"] {
            let r = row_json(&ctx, id);
            assert_eq!((r["harnessTitle"].clone(), r["titleSource"].clone()), (json!("Building the server"), json!("person")), "{id}");
        }
        assert!(row_json(&ctx, "r3").get("harnessTitle").is_none(), "another session's row is left alone");
        // Kept, then the thread is pushed once.
        assert_eq!(effects(&ctx), ["persist ws1", "pushHead t1"]);

        // However often a client refreshes inside the window, nothing more is read.
        area.refresh_title(&v, false).await;
        area.refresh_title(&v, false).await;
        assert_eq!(adapter.called().len(), 1);
        // Past the window the read goes out again; a title that did not move pushes no row.
        ctx.clock.advance(SESSION_TITLE_TTL_MS as i64 + 1);
        area.refresh_title(&v, false).await;
        assert_eq!(adapter.called().len(), 2);
        assert_eq!(effects(&ctx), ["persist ws1", "pushHead t1", "persist ws1"]);
    });
}

// TS: session-title.test.ts:147
#[test]
fn refresh_title_follows_a_rename_made_inside_the_harness() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new()
            .titled(Ability::sequence(vec![Ok(Some("Building the server".to_string())), Ok(Some("the sidebar's own name".to_string()))]))
            .arc();
        running(&ctx, "claude", &adapter);
        seed(&ctx, thread_rows());
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        area.refresh_title(&v, false).await;
        ctx.clock.advance(SESSION_TITLE_TTL_MS as i64 + 1);
        area.refresh_title(&v, false).await;
        assert_eq!(row_json(&ctx, "r1")["harnessTitle"], "the sidebar's own name");
        assert_eq!(row_json(&ctx, "r2")["harnessTitle"], "the sidebar's own name");
        assert_eq!(effects(&ctx), ["persist ws1", "pushHead t1", "persist ws1", "pushHead t1"]);
    });
}

// TS: session-title.test.ts:288
#[test]
fn refresh_title_keeps_the_rows_when_the_store_has_no_title_and_asks_again_after_the_window() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new().titled(Ability::sequence(vec![Ok(None), Ok(Some("Building the server".to_string()))])).arc();
        running(&ctx, "claude", &adapter);
        seed(&ctx, thread_rows());
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        area.refresh_title(&v, false).await;
        assert!(row_json(&ctx, "r1").get("harnessTitle").is_none());
        assert!(effects(&ctx).is_empty(), "no title changes nothing, so nothing is kept or pushed");
        // A miss is not cached forever.
        ctx.clock.advance(SESSION_TITLE_TTL_MS as i64 + 1);
        area.refresh_title(&v, false).await;
        assert_eq!(row_json(&ctx, "r1")["harnessTitle"], "Building the server");
    });
}

// TS: session-title.test.ts:278
#[test]
fn refresh_title_does_nothing_for_an_adapter_with_no_store_title() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new().arc();
        let (_, machine) = running(&ctx, "claude", &adapter);
        seed(&ctx, thread_rows());
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        ctx.area().refresh_title(&v, true).await;
        assert!(adapter.called().is_empty());
        assert!(machine.commands().is_empty(), "its machine is asked nothing");
        assert!(row_json(&ctx, "r1").get("harnessTitle").is_none());
    });
}

// TS: session-title.test.ts:213
#[test]
fn refresh_title_fails_one_rows_title_never_the_call_when_the_adapter_refuses_the_id() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new().titled(Ability::failing("the session id is not a plain slug")).arc();
        running(&ctx, "claude", &adapter);
        seed(&ctx, thread_rows());
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        let warned = console::capture();
        area.refresh_title(&v, false).await;
        assert!(row_json(&ctx, "r1").get("harnessTitle").is_none());
        assert_eq!(warned.lines(), ["no title for session S1 on ws1: the session id is not a plain slug"]);
    });
}

// TS: session-title.test.ts:362, session-title.test.ts:320, session-title.test.ts:341
#[test]
fn refresh_title_warns_once_until_a_read_answers() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let slow = || Err(other("timed out after 15000 ms"));
        let adapter = FakeAdapter::new()
            .titled(Ability::sequence(vec![slow(), slow(), slow(), slow(), Ok(Some("Building the server".to_string())), slow()]))
            .arc();
        running(&ctx, "claude", &adapter);
        seed(&ctx, thread_rows());
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        let warned = console::capture();
        let line = "no title for session S1 on ws1: timed out after 15000 ms".to_string();
        for _ in 0..4 {
            area.refresh_title(&v, false).await;
            ctx.clock.advance(SESSION_TITLE_TTL_MS as i64 + 1);
        }
        assert_eq!(adapter.called().len(), 4, "a failed read is a read: the window holds");
        assert_eq!(warned.lines(), [line.clone()], "said once while it keeps failing");
        // A read that answers clears the mark, so the next failure is said again.
        area.refresh_title(&v, false).await;
        assert_eq!(row_json(&ctx, "r1")["harnessTitle"], "Building the server");
        ctx.clock.advance(SESSION_TITLE_TTL_MS as i64 + 1);
        area.refresh_title(&v, false).await;
        assert_eq!(warned.lines(), [line.clone(), line]);
        assert_eq!(adapter.called().len(), 6);
    });
}

// TS: session-title.test.ts:182, session-title.test.ts:866
#[test]
fn refresh_title_does_nothing_on_an_unreached_machine() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new().titled(titled("Building the server")).arc();
        running(&ctx, "claude", &adapter);
        seed(&ctx, thread_rows());
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();

        ctx.unreached.borrow_mut().insert("ws1".to_string(), "the machine did not answer".to_string());
        area.refresh_title(&v, true).await;
        ctx.unreached.borrow_mut().clear();

        // A workspace that is not running keeps the title it last read and is asked nothing.
        let (napping, machine) = entry(json!({ "phase": "napping" }));
        ctx.add_entry(napping);
        area.refresh_title(&v, true).await;
        assert!(machine.commands().is_empty());

        // A workspace that is not live, a row with no agent session, a harness with no adapter.
        let mut elsewhere = v.clone();
        elsewhere.workspace_id = "ws-gone".to_string();
        area.refresh_title(&elsewhere, true).await;
        let mut sessionless = v.clone();
        sessionless.claude_session_id = None;
        area.refresh_title(&sessionless, true).await;
        let mut unknown = v.clone();
        unknown.harness = "aider".into();
        area.refresh_title(&unknown, true).await;

        assert!(adapter.called().is_empty());
        assert!(row_json(&ctx, "r1").get("harnessTitle").is_none());
        assert!(effects(&ctx).is_empty());
    });
}

// TS: session-title.test.ts:458, session-title.test.ts:617
#[test]
fn refresh_title_keeps_a_persons_name_over_the_seed() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        // The store answers the opening words, which codex writes there the moment a thread starts: the seed again.
        let adapter = FakeAdapter::new()
            .titled(titled(OPENING))
            .renaming(Ability::answering(SessionRenameWrite::Written).execing("wsp-title-write S1"))
            .arc();
        let (_, machine) = running(&ctx, "claude", &adapter);
        seed(
            &ctx,
            vec![json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING, "startedAt": 10, "harnessTitle": "Ticket 411 review", "titleSource": "person" })],
        );
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();

        area.refresh_title(&v, false).await;
        settle().await;
        let r = row_json(&ctx, "r1");
        assert_eq!((r["harnessTitle"].clone(), r["titleSource"].clone()), (json!("Ticket 411 review"), json!("person")));
        assert_eq!(effects(&ctx), ["persist ws1"], "nothing moved, so no row is pushed and no name is written");
        assert!(adapter.called().iter().all(|c| !c.starts_with("renameSession")));

        // Forced, at a turn's end: the row's own name is written into the store again, since the index row is there now.
        ctx.clock.advance(SESSION_TITLE_TTL_MS as i64 + 1);
        area.refresh_title(&v, true).await;
        settle().await;
        assert_eq!(adapter.called().iter().filter(|c| c.starts_with("renameSession")).cloned().collect::<Vec<_>>(), ["renameSession S1 Ticket 411 review"]);
        assert_eq!(runs(&machine, "wsp-title-write S1"), [Some(15_000)]);
        assert!(ctx.called().contains(&"writeSession S1".to_string()));
        assert_eq!(row_json(&ctx, "r1")["harnessTitle"], "Ticket 411 review");
    });
}

// TS: session-title.test.ts:475
#[test]
fn refresh_title_reads_the_opening_words_as_the_seed_whole_or_by_its_head() {
    local(async {
        for stored in [OPENING, "make a server"] {
            let ctx = FakeAgentsCtx::new();
            let adapter = FakeAdapter::new().titled(titled(stored)).arc();
            running(&ctx, "claude", &adapter);
            seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING, "startedAt": 10 })]);
            let area = ctx.area();
            let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
            area.refresh_title(&v, false).await;
            let r = row_json(&ctx, "r1");
            assert_eq!((r["harnessTitle"].clone(), r["titleSource"].clone()), (json!(stored), json!("seed")), "{stored}");
            assert_eq!(area.thread_source("t1"), TitleSource::Seed, "{stored}: the thread is still to be asked for a name");
        }
    });
}

// TS: session-title.test.ts:490
#[test]
fn refresh_title_reads_a_title_that_is_the_whole_of_a_longer_opening_as_the_seed() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let brief = format!("make a server, and its tests. {}", "Read the ticket and every comment on it before the code. ".repeat(40));
        // The row keeps only the head of the opening: 600 units.
        let kept: String = brief.chars().take(600).collect();
        let adapter = FakeAdapter::new().titled(titled(&brief)).arc();
        running(&ctx, "claude", &adapter);
        seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": kept, "startedAt": 10 })]);
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        ctx.area().refresh_title(&v, false).await;
        assert_eq!(row_json(&ctx, "r1")["titleSource"], "seed");
    });
}

// TS: session-title.test.ts:504
#[test]
fn refresh_title_keeps_the_name_it_made_when_the_store_still_answers_the_opening_words() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new().titled(titled(OPENING)).arc();
        running(&ctx, "claude", &adapter);
        seed(
            &ctx,
            vec![json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING, "startedAt": 10, "harnessTitle": "Seed thread titles here", "titleSource": "auto" })],
        );
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        area.refresh_title(&v, false).await;
        ctx.clock.advance(SESSION_TITLE_TTL_MS as i64 + 1);
        area.refresh_title(&v, false).await;
        let r = row_json(&ctx, "r1");
        assert_eq!((r["harnessTitle"].clone(), r["titleSource"].clone()), (json!("Seed thread titles here"), json!("auto")));
        assert!(!effects(&ctx).iter().any(|e| e.starts_with("pushHead")));
    });
}

// TS: session-title.test.ts:645, session-title.test.ts:662
#[test]
fn refresh_title_keeps_a_start_named_thread_whose_store_titles_it_with_the_whole_brief() {
    local(async {
        let brief = "You are the cold reviewer for wsp-map #1671: the desktop app\nupdating itself in place.\n\nRead the diff first.";
        let with_files = format!("{brief}\n\nAttached files:\n- /root/app/.wsp-files/t1/req_a/notes.txt");
        for stored in [brief.to_string(), with_files] {
            let ctx = FakeAgentsCtx::new();
            let adapter = FakeAdapter::new().titled(titled(&stored)).arc();
            running(&ctx, "claude", &adapter);
            seed(
                &ctx,
                vec![json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": brief, "startedAt": 10, "harnessTitle": "Review 1671 self update", "titleSource": "person" })],
            );
            let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
            ctx.area().refresh_title(&v, false).await;
            let r = row_json(&ctx, "r1");
            assert_eq!((r["harnessTitle"].clone(), r["titleSource"].clone()), (json!("Review 1671 self update"), json!("person")), "{stored}");
        }
    });
}

// TS: sub-resume.test.ts:446, sub-resume.test.ts:454
#[test]
fn refresh_title_keeps_a_name_when_the_new_machines_store_titles_it_with_the_threads_opening_or_the_lost_session_words() {
    local(async {
        let preamble = format!("You are a sub-thread of wsp. Remember the word lantern. {OPENING}");
        let lost = format!("{LOST_HEAD} Run git log --oneline -1.");
        for stored in [preamble.clone(), lost] {
            let ctx = FakeAgentsCtx::new();
            let adapter = FakeAdapter::new().titled(titled(&stored)).arc();
            running(&ctx, "claude", &adapter);
            seed(
                &ctx,
                vec![
                    json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S0", "prompt": preamble, "startedAt": 10, "harnessTitle": "Fix the upload test", "titleSource": "person" }),
                    json!({ "id": "r2", "threadId": "t1", "claudeSessionId": "S1", "prompt": "Run git log --oneline -1.", "startedAt": 20, "harnessTitle": "Fix the upload test", "titleSource": "person" }),
                ],
            );
            let v = ctx.sessions.borrow().get("r2").expect("a row").borrow().view.clone();
            ctx.area().refresh_title(&v, false).await;
            let r = row_json(&ctx, "r2");
            assert_eq!((r["harnessTitle"].clone(), r["titleSource"].clone()), (json!("Fix the upload test"), json!("person")), "{stored}");
        }
    });
}

// ---- L6: where a title came from -------------------------------------------------------------------------------------

#[test]
fn thread_source_is_person_over_auto_over_seed() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let area = ctx.area();
        seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1" }), json!({ "id": "r2", "threadId": "t1", "titleSource": "seed" })]);
        assert_eq!(area.thread_source("t1"), TitleSource::Seed);
        assert_eq!(area.thread_source("t-none"), TitleSource::Seed);
        seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1" }), json!({ "id": "r2", "threadId": "t1", "titleSource": "auto" })]);
        assert_eq!(area.thread_source("t1"), TitleSource::Auto);
        seed(
            &ctx,
            vec![json!({ "id": "r1", "threadId": "t1", "titleSource": "auto" }), json!({ "id": "r2", "threadId": "t1", "titleSource": "person" }), json!({ "id": "r3", "threadId": "t1" })],
        );
        assert_eq!(area.thread_source("t1"), TitleSource::Person, "a person's name on any turn is the thread's");
        let bare = view(json!({ "id": "x" }));
        assert_eq!(source_of(&bare), TitleSource::Seed, "a row with no provenance reads as seed");
        assert_eq!(source_of(&view(json!({ "id": "x", "titleSource": "auto" }))), TitleSource::Auto);
    });
}

#[test]
fn rows_on_lists_the_threads_rows_in_map_order_and_the_opening_is_the_oldest_rows_prompt() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let area = ctx.area();
        seed(
            &ctx,
            vec![
                json!({ "id": "r1", "threadId": "t1", "prompt": "second", "startedAt": 20 }),
                json!({ "id": "r2", "threadId": "t2", "prompt": "elsewhere", "startedAt": 1 }),
                json!({ "id": "r3", "threadId": "t1", "prompt": "first", "startedAt": 10 }),
                json!({ "id": "r4", "threadId": "t1", "prompt": "tied", "startedAt": 10 }),
            ],
        );
        assert_eq!(area.rows_on("t1").iter().map(|v| v.id.clone()).collect::<Vec<_>>(), ["r1", "r3", "r4"]);
        assert_eq!(area.opening_of("t1"), Some("first".to_string()), "the first on a tie");
        assert_eq!(area.opening_of("t9"), None);
        // An absent start reads as zero.
        seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1", "prompt": "later", "startedAt": 5 }), json!({ "id": "r2", "threadId": "t1", "prompt": "no start" })]);
        assert_eq!(area.opening_of("t1"), Some("no start".to_string()));
    });
}

#[test]
fn carried_title_is_the_newest_titled_row() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let area = ctx.area();
        seed(
            &ctx,
            vec![
                json!({ "id": "r1", "threadId": "t1", "startedAt": 10, "harnessTitle": "Old", "titleSource": "person" }),
                json!({ "id": "r2", "threadId": "t1", "startedAt": 30, "harnessTitle": "New", "titleSource": "auto" }),
                json!({ "id": "r3", "threadId": "t1", "startedAt": 50 }),
                json!({ "id": "r4", "threadId": "t2", "startedAt": 90, "harnessTitle": "Other thread" }),
            ],
        );
        assert_eq!(area.carried_title("t1"), CarriedTitle { harness_title: Some("New".to_string()), title_source: Some(TitleSource::Auto) });
        // A row with a title and no provenance carries seed.
        seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1", "startedAt": 10, "harnessTitle": "Written before provenance" })]);
        assert_eq!(area.carried_title("t1"), CarriedTitle { harness_title: Some("Written before provenance".to_string()), title_source: Some(TitleSource::Seed) });
        seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1", "startedAt": 10 })]);
        assert_eq!(area.carried_title("t1"), CarriedTitle::default(), "no titled row is empty");
    });
}

// ---- L7: naming a session in the harness -----------------------------------------------------------------------------

// TS: session-title.test.ts:689, session-title.test.ts:748, session-title.test.ts:801
#[test]
fn name_in_harness_writes_through_write_session_and_warns_on_refusal() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new()
            .renaming(Ability::sequence(vec![
                Ok(SessionRenameWrite::Written),
                Ok(SessionRenameWrite::NoSession),
                Ok(SessionRenameWrite::Failed { error: "database is locked".to_string() }),
                Err(other("the machine hung up")),
            ]).execing("wsp-title-write S1"))
            .arc();
        let (_, machine) = running(&ctx, "claude", &adapter);
        let area = ctx.area();
        // The harness's own session is named, whatever the runtime calls the row.
        let v = view(json!({ "id": "row-not-the-session", "claudeSessionId": "S1", "threadId": "t1" }));
        let warned = console::capture();

        area.name_in_harness(&v, "the name he typed in wsp").await;
        assert_eq!(adapter.called(), ["renameSession S1 the name he typed in wsp"]);
        assert_eq!(runs(&machine, "wsp-title-write S1"), [Some(15_000)]);
        assert_eq!(ctx.called_with("writeSession"), ["writeSession S1"]);
        assert!(warned.lines().is_empty());

        // The store took nothing: no line, the name stands.
        area.name_in_harness(&v, "second").await;
        assert!(warned.lines().is_empty());
        // The store refused: one line with its own words. A write that failed on the way: the same line, the message.
        area.name_in_harness(&v, "third").await;
        area.name_in_harness(&v, "fourth").await;
        assert_eq!(
            warned.lines(),
            [
                "the harness did not take the name for session S1 on ws1: database is locked",
                "the harness did not take the name for session S1 on ws1: the machine hung up",
            ]
        );
    });
}

// TS: session-title.test.ts:726, session-title.test.ts:760, session-title.test.ts:790
#[test]
fn name_in_harness_does_nothing_where_nothing_can_be_written() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let plain = FakeAdapter::new().arc();
        let (_, machine) = running(&ctx, "claude", &plain);
        let area = ctx.area();
        // An adapter that renames nothing, and its machine is asked nothing.
        area.name_in_harness(&view(json!({ "claudeSessionId": "S1" })), "a name").await;
        assert!(machine.commands().is_empty());
        assert!(ctx.called_with("writeSession").is_empty());

        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new().renaming(Ability::answering(SessionRenameWrite::Written)).arc();
        running(&ctx, "claude", &adapter);
        let area = ctx.area();
        // A row whose harness never named a session; a harness with no adapter; a workspace that is not live.
        area.name_in_harness(&view(json!({})), "a name").await;
        area.name_in_harness(&view(json!({ "claudeSessionId": "S1", "harness": "aider" })), "a name").await;
        area.name_in_harness(&view(json!({ "claudeSessionId": "S1", "workspaceId": "ws-gone" })), "a name").await;
        // A workspace that is not running keeps the name only on its rows.
        let (napping, _) = entry(json!({ "id": "ws2", "phase": "napping" }));
        ctx.add_entry(napping);
        area.name_in_harness(&view(json!({ "claudeSessionId": "S1", "workspaceId": "ws2" })), "a name").await;
        assert!(adapter.called().is_empty());
        assert!(ctx.called_with("writeSession").is_empty());
    });
}

// ---- L8: the title the harness makes ---------------------------------------------------------------------------------

fn maker(title: &str) -> Ability<Option<String>> {
    Ability::answering(Some(title.to_string()))
}

// TS: session-title.test.ts:398, session-title.test.ts:449, session-title.test.ts:580
#[test]
fn make_title_asks_once_per_thread_and_names_every_row() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new()
            .titling(maker("Seed thread titles here").execing("wsp-title-make"))
            .renaming(Ability::answering(SessionRenameWrite::Written))
            .arc();
        let (_, machine) = running(&ctx, "claude", &adapter);
        seed(
            &ctx,
            vec![
                json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING, "startedAt": 10 }),
                json!({ "id": "r2", "threadId": "t1", "claudeSessionId": "S1", "prompt": "and now the tests", "startedAt": 20 }),
                json!({ "id": "r3", "threadId": "t2", "claudeSessionId": "S9", "prompt": "other", "startedAt": 30 }),
            ],
        );
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();

        area.make_title(&v).await.expect("asked");
        // From the opening turn alone and on the cheapest model its catalog lists.
        let turns = adapter.title_turns.lock().expect("turns").clone();
        assert_eq!(turns.len(), 1);
        assert_eq!((turns[0].opening.as_str(), turns[0].reply.as_deref(), turns[0].model.as_deref()), (OPENING, None, Some("claude-haiku-5-5")));
        assert_eq!(runs(&machine, "wsp-title-make"), [Some(25_000)]);
        for id in ["r1", "r2"] {
            let r = row_json(&ctx, id);
            assert_eq!((r["harnessTitle"].clone(), r["titleSource"].clone()), (json!("Seed thread titles here"), json!("auto")), "{id}");
        }
        assert!(row_json(&ctx, "r3").get("harnessTitle").is_none());
        // Kept, pushed, then written into the harness's own store.
        assert_eq!(effects(&ctx), ["persist ws1", "pushHead t1", "writeSession S1"]);
        assert_eq!(adapter.called().iter().filter(|c| c.starts_with("renameSession")).cloned().collect::<Vec<_>>(), ["renameSession S1 Seed thread titles here"]);

        // Once per thread: the second turn of it asks nothing.
        let second = ctx.sessions.borrow().get("r2").expect("a row").borrow().view.clone();
        area.make_title(&second).await.expect("nothing to ask");
        area.make_title(&v).await.expect("nothing to ask");
        assert_eq!(adapter.title_turns.lock().expect("turns").len(), 1);
    });
}

// TS: session-title.test.ts:522
#[test]
fn make_title_leaves_a_thread_a_person_named_meanwhile() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let gate = Gate::new();
        let adapter = FakeAdapter::new().titling(maker("Seed thread titles here").gated(&gate)).arc();
        running(&ctx, "claude", &adapter);
        seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING, "startedAt": 10 })]);
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        let asking = tokio::task::spawn_local({
            let area = area.clone();
            async move { area.make_title(&v).await }
        });
        settle().await;
        assert_eq!(adapter.called(), ["titleFor"], "the question is out");
        // The person renames the thread inside the harness while it thinks.
        {
            let rows = ctx.sessions.borrow();
            let mut r = rows.get("r1").expect("a row").borrow_mut();
            r.view.harness_title = Some("the sidebar's own name".to_string());
            r.view.title_source = Some(TitleSource::Person);
        }
        gate.open();
        asking.await.expect("ran").expect("not a failure");
        let r = row_json(&ctx, "r1");
        assert_eq!((r["harnessTitle"].clone(), r["titleSource"].clone()), (json!("the sidebar's own name"), json!("person")));
        assert!(effects(&ctx).is_empty(), "nothing is kept, pushed or written");
    });
}

// TS: session-title.test.ts:543
#[test]
fn make_title_keeps_the_opening_words_when_the_harness_names_nothing_and_asks_no_more() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new().titling(Ability::answering(None)).arc();
        running(&ctx, "claude", &adapter);
        seed(&ctx, vec![json!({ "id": "r1", "threadId": "thread-0001-abcdef", "claudeSessionId": "S1", "prompt": OPENING, "startedAt": 10 })]);
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        let warned = console::capture();
        area.make_title(&v).await.expect("not a failure");
        assert_eq!(warned.lines(), ["thread thread-0 on ws1 keeps its opening words: the harness answered with no title"]);
        assert!(row_json(&ctx, "r1").get("harnessTitle").is_none());
        area.make_title(&v).await.expect("not a failure");
        assert_eq!(adapter.called(), ["titleFor"], "nothing retries in a loop");
    });
}

// TS: title-after-close.test.ts:136
#[test]
fn make_title_asks_nothing_once_the_host_is_closing() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new().titling(maker("Seed thread titles here")).arc();
        running(&ctx, "claude", &adapter);
        seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING, "startedAt": 10 })]);
        let area = ctx.area();
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        ctx.closing.set(true);
        area.make_title(&v).await.expect("not a failure");
        assert!(adapter.called().is_empty());
        assert!(row_json(&ctx, "r1").get("harnessTitle").is_none());
    });
}

#[test]
fn make_title_does_nothing_for_a_row_that_is_not_to_be_asked() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let adapter = FakeAdapter::new().titling(maker("Seed thread titles here")).arc();
        running(&ctx, "claude", &adapter);
        let area = ctx.area();
        seed(
            &ctx,
            vec![
                json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING }),
                json!({ "id": "r5", "threadId": "t5", "claudeSessionId": "S5", "prompt": OPENING, "titleSource": "person", "harnessTitle": "Named" }),
            ],
        );
        let get = |id: &str| ctx.sessions.borrow().get(id).expect("a row").borrow().view.clone();
        // No thread, no prompt, a thread a person named, a harness with no adapter, a workspace not live.
        let mut no_thread = get("r1");
        no_thread.thread_id = None;
        area.make_title(&no_thread).await.expect("nothing");
        let mut no_prompt = get("r1");
        no_prompt.prompt = None;
        area.make_title(&no_prompt).await.expect("nothing");
        area.make_title(&get("r5")).await.expect("nothing");
        let mut unknown = get("r1");
        unknown.harness = "aider".into();
        area.make_title(&unknown).await.expect("nothing");
        let mut gone = get("r1");
        gone.workspace_id = "ws-gone".into();
        area.make_title(&gone).await.expect("nothing");
        assert!(adapter.called().is_empty());
        // An adapter that cannot be asked at all is never marked asked, and asks nothing.
        let ctx = FakeAgentsCtx::new();
        let silent_adapter = FakeAdapter::new().arc();
        running(&ctx, "claude", &silent_adapter);
        seed(&ctx, vec![json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING })]);
        let v = ctx.sessions.borrow().get("r1").expect("a row").borrow().view.clone();
        ctx.area().make_title(&v).await.expect("nothing");
        assert!(silent_adapter.called().is_empty());
    });
}

// ---- L9: which rows a refresh asks about ----------------------------------------------------------------------------

// TS: session-title.test.ts:235, session-title.test.ts:260
#[test]
fn title_rows_asks_the_newest_row_of_each_session_up_to_twenty() {
    let mut rows: Vec<_> = (0..22)
        .map(|i| view(json!({ "id": format!("row-{i:02}"), "claudeSessionId": format!("s-{i:02}"), "startedAt": 1000 + i * 10 })))
        .collect();
    // An older turn of the newest session, a turn that never named a session, and a tie on the oldest.
    rows.push(view(json!({ "id": "older-of-21", "claudeSessionId": "s-21", "startedAt": 5 })));
    rows.push(view(json!({ "id": "no-session", "startedAt": 99_999 })));
    let picked = title_rows(&rows);
    let ids: Vec<String> = picked.iter().map(|v| v.id.clone()).collect();
    let expected: Vec<String> = (2..22).rev().map(|i| format!("row-{i:02}")).collect();
    assert_eq!(ids, expected, "newest first, one row per session, rows with no session left out, at most 20");
    assert_eq!(picked.len(), SESSION_TITLE_REFRESH_MAX);

    // Two rows of one session at one start: the first in the list stands.
    let tied = vec![view(json!({ "id": "first", "claudeSessionId": "s", "startedAt": 7 })), view(json!({ "id": "second", "claudeSessionId": "s", "startedAt": 7 }))];
    assert_eq!(title_rows(&tied).iter().map(|v| v.id.clone()).collect::<Vec<_>>(), ["first"]);
    // An absent start reads as zero.
    let zero = vec![view(json!({ "id": "unstamped", "claudeSessionId": "a" })), view(json!({ "id": "stamped", "claudeSessionId": "b", "startedAt": 3 }))];
    assert_eq!(title_rows(&zero).iter().map(|v| v.id.clone()).collect::<Vec<_>>(), ["stamped", "unstamped"]);
}

// ---- L10: the lists a picker offers ----------------------------------------------------------------------------------

// TS: runtime-history.test.ts:429, runtime-history.test.ts:725, thread-access.test.ts:111, thread-defaults.test.ts:180
#[test]
fn agent_lists_answers_the_table_off_a_paused_machine() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        // Registered codex first: the lists come in the table's order whatever the adapters' own.
        let codex = FakeAdapter::new().probing(answered("0.9.0")).arc();
        let claude = FakeAdapter::new().probing(answered("2.1.257")).arc();
        ctx.add_adapter("codex", plain_factory(codex.clone()));
        ctx.add_adapter("claude", plain_factory(claude.clone()));
        let area = ctx.area();

        // No entry named: wsp's table, one row per harness with an adapter; the rest cannot run a turn and are not listed.
        let listed: Vec<Value> = area.agent_lists(None).await.iter().map(json_of).collect();
        assert_eq!(listed.iter().map(|c| c["harness"].as_str().expect("a harness").to_string()).collect::<Vec<_>>(), ["claude", "codex"]);
        assert!(listed.iter().all(|c| c["source"] == "table"));
        assert!(!listed[0]["efforts"].as_array().expect("efforts").is_empty());

        // A workspace whose machine is not up answers the table too, the access its next thread runs at included.
        let (napping, machine) = entry(json!({ "phase": "napping" }));
        let paused: Vec<Value> = area.agent_lists(Some(&napping)).await.iter().map(json_of).collect();
        assert_eq!(paused, listed);
        let modes = paused[0]["permissionModes"].as_array().expect("modes").clone();
        assert_eq!(modes.iter().find(|m| m["isDefault"] == true).map(|m| m["value"].clone()), Some(json!("bypassPermissions")));
        assert!(machine.commands().is_empty(), "a napping workspace's binary is left alone");
        assert!(ctx.called().is_empty());
        assert!(claude.called().is_empty() && codex.called().is_empty());

        // An agent turned off on the computer the workspace stands on leaves the lists.
        ctx.turn_off("here", "claude");
        let (off, _) = entry(json!({ "phase": "napping" }));
        let without: Vec<Value> = area.agent_lists(Some(&off)).await.iter().map(json_of).collect();
        assert_eq!(without.iter().map(|c| c["harness"].as_str().expect("a harness").to_string()).collect::<Vec<_>>(), ["codex"]);
    });
}

#[test]
fn agent_lists_asks_each_agent_of_a_running_machine_at_once_in_table_order() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        let codex = FakeAdapter::new().probing(answered("0.9.0")).arc();
        let claude = FakeAdapter::new().probing(answered("2.1.257")).arc();
        ctx.add_adapter("codex", plain_factory(codex.clone()));
        ctx.add_adapter("claude", plain_factory(claude.clone()));
        let (e, _) = entry(json!({}));
        let listed: Vec<Value> = ctx.area().agent_lists(Some(&e)).await.iter().map(json_of).collect();
        assert_eq!(listed.iter().map(|c| c["harness"].as_str().expect("a harness").to_string()).collect::<Vec<_>>(), ["claude", "codex"]);
        assert!(listed.iter().all(|c| c["source"] == "harness"));
        assert_eq!((listed[0]["version"].clone(), listed[1]["version"].clone()), (json!("2.1.257"), json!("0.9.0")));
        assert_eq!(ctx.called_with("knowBroker"), ["knowBroker ws1 claude", "knowBroker ws1 codex"]);
        assert_eq!(claude.called(), ["probeCatalog"]);
        assert_eq!(codex.called(), ["probeCatalog"]);
    });
}

// TS: places-agents.test.ts:190, thread-defaults.test.ts:288
#[test]
fn agent_lists_answers_a_refused_config_folder_as_its_refusal() {
    local(async {
        let root = std::fs::canonicalize(tempfile::tempdir().expect("a folder").keep()).expect("a real path");
        let home = root.join("home");
        let outside = root.join("outside");
        std::fs::create_dir_all(&home).expect("home");
        std::fs::create_dir_all(&outside).expect("outside");
        let kept = outside.join("claude-wsp");
        let kept = kept.to_str().expect("utf8").to_string();

        let ctx = FakeAgentsCtx::new();
        *ctx.local_home_dir.borrow_mut() = Some(home.to_str().expect("utf8").to_string());
        ctx.keep_config_dir("here", "claude", &kept);
        let claude = FakeAdapter::new().arc();
        let codex = FakeAdapter::new().arc();
        ctx.add_adapter("claude", plain_factory(claude));
        ctx.add_adapter("codex", plain_factory(codex));
        let (e, _) = entry(json!({}));
        let area = ctx.area();
        let listed: Vec<Value> = area.agent_lists(Some(&e)).await.iter().map(json_of).collect();

        let home = home.to_str().expect("utf8");
        let words = format!(
            "Claude Code does not start with its config folder {kept}: Claude Code's config folder has to be under the home folder {home}, and {kept} is not. Set another with wsp agents setup claude --config, or put its own back with --reset config."
        );
        assert_eq!(listed[0]["harness"], "claude");
        assert_eq!(listed[0]["refusal"], json!(words));
        assert_eq!(listed[1]["harness"], "codex");
        assert!(listed[1].get("refusal").is_none());
        // The refused agent is not asked for its broker; the word moved once.
        assert_eq!(ctx.called_with("knowBroker"), ["knowBroker ws1 codex"]);
        assert_eq!(ctx.called_with("setupRefusalMoved"), ["setupRefusalMoved here claude"]);
    });
}

#[test]
fn agent_lists_answers_a_failed_broker_read_as_a_refusal_and_a_dropped_link_as_the_table() {
    local(async {
        let ctx = FakeAgentsCtx::new();
        ctx.add_adapter("claude", plain_factory(FakeAdapter::new().arc()));
        ctx.add_adapter("codex", plain_factory(FakeAdapter::new().arc()));
        ctx.broker_failures.borrow_mut().insert("codex".to_string(), "no login is held for Codex".to_string());
        let (e, _) = entry(json!({}));
        let listed: Vec<Value> = ctx.area().agent_lists(Some(&e)).await.iter().map(json_of).collect();
        assert!(listed[0].get("refusal").is_none());
        assert_eq!(listed[1]["refusal"], "no login is held for Codex");
        assert_eq!(listed[1]["source"], "table", "the table's lists are the ones the refusal is said beside");

        // A joined computer whose link dropped under the check is its computer's state: the table answers, no refusal.
        let ctx = FakeAgentsCtx::new();
        ctx.add_adapter("claude", plain_factory(FakeAdapter::new().arc()));
        ctx.keep_config_dir("srv", "claude", "/root/claude-wsp");
        ctx.place_rules.borrow_mut().push(("srv".to_string(), "pwd".to_string(), Err(silent("the computer is not connected", "PlaceAbsent"))));
        let (placed, _) = entry(json!({ "kind": "place", "place": "srv" }));
        let listed: Vec<Value> = ctx.area().agent_lists(Some(&placed)).await.iter().map(json_of).collect();
        assert_eq!(listed.len(), 1);
        assert!(listed[0].get("refusal").is_none());
        assert_eq!(listed[0]["source"], "table");
    });
}
