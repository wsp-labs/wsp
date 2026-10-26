// SPDX-License-Identifier: AGPL-3.0-only
//! agents_lists (spec/agents_lists.md): what each agent offers on a machine, and thread titles. Expected values read
//! off agents.ts at 65f6b89c7 and the TS tests named above each test.
#![allow(dead_code, unused_imports)]

#[path = "support_b/mod.rs"]
mod support;
include!("agents_lists/world.rs");

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
    for key in [
        "mcpServers",
        "movesAccess",
        "steersImages",
        "compacts",
        "terminalResume",
        "rewindsConversation",
        "rewindsByCount",
        "forks",
        "forksByCount",
        "screenCommands",
    ] {
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
        assert_eq!(
            first["models"][0],
            json!({ "value": "claude-opus-5-5", "label": "Opus 5.5", "isDefault": true, "efforts": ["low", "high"], "contextWindows": [] })
        );
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
        let adapter = FakeAdapter::new()
            .probing(answered("2.1.257").execing("claude --help"))
            .versioned(Ability::answering(Some("2.1.257".to_string())).execing("claude --version"))
            .arc();
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
        let adapter = FakeAdapter::new()
            .probing(answered("2.1.257").execing("claude --help"))
            .versioned(Ability::answering(Some("2.1.257".to_string())).execing("claude --version"))
            .arc();
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
        assert_eq!(warned.lines(), std::slice::from_ref(&line));

        // The cache holds the failure too: one line and one exec however often the composer opens.
        area.catalog_on(&table, &a, adapter.clone()).await;
        assert_eq!(warned.lines(), std::slice::from_ref(&line));
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
        let codex = FakeAdapter::new()
            .probing(Ability::answering(Some(HarnessCatalogAnswer::Probe(codex_probe))).execing("codex --describe"))
            .arc();
        let area = ctx.area();
        let c = json_of(&area.catalog_on(&claude_table(), &a, claude.clone()).await);
        let x = json_of(&area.catalog_on(&codex_table(), &a, codex.clone()).await);
        assert_eq!((c["source"].clone(), c["version"].clone()), (json!("harness"), json!("2.1.257")));
        assert_eq!((x["source"].clone(), x["version"].clone(), x["label"].clone()), (json!("harness"), json!("0.9.0"), json!("Codex")));
        assert!(x.get("isDefault").is_none());
        assert_eq!(
            x["models"],
            json!([{ "value": "gpt-5-codex", "label": "Codex", "isDefault": true, "efforts": ["high"], "contextWindows": [] }])
        );
        // The table lends the words for values the binary only names.
        assert_eq!(
            x["permissionModes"],
            json!([{ "value": "read-only", "label": "Read only", "description": "Reads only; edits no files and runs no command that writes" }])
        );
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
