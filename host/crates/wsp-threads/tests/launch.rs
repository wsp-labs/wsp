// SPDX-License-Identifier: AGPL-3.0-only
//! spec/launch.md. TS cases: thread-warm-agent.test.ts, lost-session-line.test.ts, cloud-notice.test.ts,
//! thread-defaults.test.ts, thread-keeps-picks.test.ts. Each TS case is ported at this unit's seam: the warm area's
//! answers and what it hands `hold_warm`, `reap_kept`, `rewarm` and the adapter; the picks a launch reads; the line a
//! lost session logs.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("launch/world.rs");

// L1: launch.ts:212
#[test]
fn a_root_thread_keeps_its_server() {
    let wsp = wsp_server(&["/opt/wsp/bin.js", "mcp", SCOPED_MCP_ARG]);
    assert_eq!(served_to(Some(&wsp), "t1", "t1"), Some(wsp.clone()));
    assert_eq!(served_to(None, "t1", "t1"), None);
    assert_eq!(served_to(None, "root", "child"), None, "no server to hand a child either");
}

// L2: launch.ts:212
#[test]
fn a_child_gets_no_slate_and_the_scoped_arg_list_gains_the_flag() {
    let wsp = wsp_server(&["/opt/wsp/bin.js", "mcp", "--state", "/Users/me/.wsp/state.json", SCOPED_MCP_ARG]);
    let got = served_to(Some(&wsp), "root", "child").expect("a server for the child");
    assert_eq!(
        serde_json::to_string(&got).unwrap(),
        format!(
            r#"{{"command":"/usr/local/bin/node","args":["/opt/wsp/bin.js","mcp","--state","/Users/me/.wsp/state.json","{SCOPED_MCP_ARG}","{NO_SLATE_MCP_ARG}"],"noSlate":true}}"#
        ),
        "the server's own keys, then noSlate, with args overwritten in place"
    );
    assert_eq!(wsp.args.last().map(String::as_str), Some(SCOPED_MCP_ARG), "the host's own server is left as it was");
}

// L2: launch.ts:212
#[test]
fn a_child_server_without_the_scoped_arg_keeps_its_args() {
    let wsp = wsp_server(&["/opt/wsp/bin.js", "mcp"]);
    let got = served_to(Some(&wsp), "root", "child").expect("a server for the child");
    assert_eq!(
        serde_json::to_string(&got).unwrap(),
        r#"{"command":"/usr/local/bin/node","args":["/opt/wsp/bin.js","mcp"],"noSlate":true}"#
    );
}

// L4, L6: cloud-notice.test.ts:20
#[test]
fn a_thread_with_no_record_is_told_nothing() {
    let host = FakeHost::new();
    let mut prompt = "hello".to_string();
    let held = clouds_told(Some(vec![provider("box")]), &host.records, "t1", &mut prompt);
    assert_eq!(prompt, "hello");
    assert_eq!(held, vec![said_of("box")], "the clouds held are answered whatever the record says");
    assert_eq!(host.records.borrow().len(), 0, "the record is not written here");
}

// L4: cloud-notice.test.ts:20
#[test]
fn a_record_from_before_the_clouds_were_kept_is_told_nothing_and_starts_keeping_them() {
    let host = FakeHost::new();
    with_clouds(&host, None);
    let before = host.records.borrow().clone();
    let mut prompt = "hello".to_string();
    let held = clouds_told(Some(vec![provider("box")]), &host.records, "t1", &mut prompt);
    assert_eq!(prompt, "hello");
    assert_eq!(held, vec![said_of("box")], "the caller writes these into the record once the turn launched");
    assert_eq!(*host.records.borrow(), before, "this call writes nothing");
}

// L5: cloud-notice.test.ts:34
#[test]
fn added_then_removed_clouds_lead_the_prompt() {
    let host = FakeHost::new();
    with_clouds(&host, Some(vec![]));
    let mut prompt = "can you spawn a cloud thread to run the test suite?".to_string();
    let held = clouds_told(Some(vec![computer("mac"), provider("box")]), &host.records, "t1", &mut prompt);
    assert_eq!(prompt, format!("{}\n\ncan you spawn a cloud thread to run the test suite?", cloud_added_line(&said_of("box"))));
    assert_eq!(held, vec![said_of("box")]);
    // The record took the held cloud once its turn launched; the cloud is then taken away (cloud-notice.test.ts:38).
    with_clouds(&host, Some(vec![&said_of("box")]));
    let mut prompt = "is it still there?".to_string();
    let held = clouds_told(Some(vec![computer("mac")]), &host.records, "t1", &mut prompt);
    assert_eq!(prompt, format!("{}\n\nis it still there?", cloud_removed_line(&said_of("box"))));
    assert_eq!(held, Vec::<String>::new());
}

// L5: launch.ts:221
#[test]
fn added_lines_come_in_held_order_then_removed_lines_in_the_records_order() {
    let host = FakeHost::new();
    let box_said = said_of("box");
    with_clouds(&host, Some(vec!["zed", "gone", &box_said]));
    let mut prompt = "go".to_string();
    let held = clouds_told(Some(vec![provider("solari"), provider("box")]), &host.records, "t1", &mut prompt);
    let lines = [cloud_added_line(&said_of("solari")), cloud_removed_line("zed"), cloud_removed_line("gone")];
    assert_eq!(prompt, format!("{}\n\ngo", lines.join("\n")));
    assert_eq!(held, vec![said_of("box"), said_of("solari")]);
}

// L5: cloud-notice.test.ts:35
#[test]
fn nothing_changed_leaves_the_prompt() {
    let host = FakeHost::new();
    let box_said = said_of("box");
    with_clouds(&host, Some(vec![&box_said]));
    let mut prompt = "and again".to_string();
    let held = clouds_told(Some(vec![provider("box")]), &host.records, "t1", &mut prompt);
    assert_eq!(prompt, "and again", "no blank lines added where there is nothing to say");
    assert_eq!(held, vec![box_said]);
}

// L3: launch.ts:221
#[test]
fn held_clouds_are_sorted() {
    let host = FakeHost::new();
    let mut prompt = String::new();
    let rows = vec![provider("solari"), computer("mac"), provider("alpha"), provider("box")];
    let held = clouds_told(Some(rows), &host.records, "t1", &mut prompt);
    assert_eq!(
        held,
        vec![said_of("box"), said_of("solari"), said_of("alpha")],
        "by code unit, so an upper case name leads a lower case one, and a computer is no cloud"
    );
}

// L3: launch.ts:221
#[test]
fn a_host_with_no_door_holds_no_clouds_and_says_every_recorded_one_gone() {
    let host = FakeHost::new();
    with_clouds(&host, Some(vec!["Boat"]));
    let mut prompt = "hi".to_string();
    let held = clouds_told(None, &host.records, "t1", &mut prompt);
    assert_eq!(held, Vec::<String>::new());
    assert_eq!(prompt, format!("{}\n\nhi", cloud_removed_line("Boat")));
}

// L8, L9: thread-defaults.test.ts:111
#[test]
fn picks_named_win_over_the_threads_own() {
    run_local(async {
        let (host, area, e) = picks_rig();
        set_ran(&host, Rc::new(RefCell::new(ran(Some("claude-opus-5"), Some("low"), None))));
        host.hooks.set::<dyn Fn(&str, &str, Option<&str>) -> Option<String>>(
            "LaunchCtx::access_of",
            Rc::new(|_w: &str, _t: &str, _s: Option<&str>| Some("default".to_string())),
        );
        host.hooks.set::<dyn Fn(Option<&HarnessCatalog>, &str, &AccessChoice) -> Result<Option<String>, HostError>>(
            "LaunchCtx::named_mode",
            Rc::new(|_c: Option<&HarnessCatalog>, _h: &str, a: &AccessChoice| {
                Ok((*a == AccessChoice::Full).then(|| "bypassPermissions".to_string()))
            }),
        );
        // A permission mode named is the access, and the access word is never read.
        let read = read_picks(
            &area,
            &e,
            &LaunchAsked { permission_mode: Some("acceptEdits".into()), access: Some(AccessChoice::Full), ..Default::default() },
        )
        .await
        .unwrap();
        assert_eq!(read.catalog, Some(catalog()));
        assert_eq!(
            (read.picks_for)(Some("sess-1")).unwrap(),
            launch_picks_of(picks(Some("claude-opus-5"), Some("low"), Some("acceptEdits")), None)
        );
        assert_eq!(host.hooks.called("named_mode"), Vec::<String>::new());
        assert_eq!(host.hooks.called("access_of"), Vec::<String>::new(), "a named access leaves the thread's own unread");
        // An access word is named through the catalog.
        let read = read_picks(&area, &e, &LaunchAsked { access: Some(AccessChoice::Full), ..Default::default() }).await.unwrap();
        assert_eq!(
            (read.picks_for)(Some("sess-1")).unwrap(),
            launch_picks_of(picks(Some("claude-opus-5"), Some("low"), Some("bypassPermissions")), None)
        );
        assert_eq!(host.hooks.called("named_mode"), vec!["named_mode(claude)".to_string()]);
        // A model named wins over the thread's, and the thread's effort rides where the new model takes it.
        let read = read_picks(&area, &e, &LaunchAsked { model: Some("claude-sonnet-5".into()), ..Default::default() }).await.unwrap();
        assert_eq!(
            (read.picks_for)(Some("sess-1")).unwrap(),
            launch_picks_of(picks(Some("claude-sonnet-5"), Some("low"), Some("default")), None)
        );
    });
}

// TS: launch.ts:53 `ctx.namedMode(catalog, harness, o.access)` throws the usage refusal naming the words the agent takes, and the pick read
// rejects with it.
#[test]
fn an_access_word_the_agent_has_no_mode_for_fails_the_pick_read_with_its_refusal() {
    run_local(async {
        let (host, area, e) = picks_rig();
        host.hooks.set::<dyn Fn(Option<&HarnessCatalog>, &str, &AccessChoice) -> Result<Option<String>, HostError>>(
            "LaunchCtx::named_mode",
            Rc::new(|_c: Option<&HarnessCatalog>, _h: &str, _a: &AccessChoice| {
                Err(HostError::with_kind("Claude Code takes no plan access; it takes ask, auto-edit, full", Kind::Usage))
            }),
        );
        let got = read_picks(&area, &e, &LaunchAsked { access: Some(AccessChoice::Plan), ..Default::default() }).await;
        let refused = got.err().expect("the read throws");
        assert_eq!(
            (refused.message.as_str(), refused.kind),
            ("Claude Code takes no plan access; it takes ask, auto-edit, full", Some(Kind::Usage))
        );
    });
}

// L9: thread-defaults.test.ts:111
#[test]
fn a_threads_own_access_applies_only_where_the_harness_lists_it() {
    run_local(async {
        let (host, area, e) = picks_rig();
        let own = Rc::new(RefCell::new(Some("acceptEdits".to_string())));
        let o = own.clone();
        host.hooks.set::<dyn Fn(&str, &str, Option<&str>) -> Option<String>>(
            "LaunchCtx::access_of",
            Rc::new(move |_w: &str, _t: &str, _s: Option<&str>| o.borrow().clone()),
        );
        let read = read_picks(&area, &e, &LaunchAsked::default()).await.unwrap();
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(picks(None, None, Some("acceptEdits")), None));
        assert_eq!(host.hooks.called("access_of"), vec![r#"access_of(w1, t1, Some("sess-1"))"#.to_string()]);
        // A mode this harness does not take is a pick that does not apply here, not a refusal.
        *own.borrow_mut() = Some("plan".to_string());
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(StartPicks::default(), None));
    });
}

// L7, L9: thread-defaults.test.ts:111
#[test]
fn a_thread_with_none_opens_on_the_defaults() {
    run_local(async {
        let (host, area, e) = picks_rig();
        // The defaults put the agent's own model, effort and access on the catalog: sonnet at low, access ask.
        let marked = catalog_over(json!({
            "permissionModes": [
                { "value": "default", "label": "default", "isDefault": true },
                { "value": "acceptEdits", "label": "acceptEdits" },
                { "value": "bypassPermissions", "label": "bypassPermissions" },
            ],
        }));
        let resolved = DefaultsOn { catalog: marked.clone(), open_model: Some("claude-sonnet-5".into()), open_effort: Some("low".into()) };
        host.hooks.set::<dyn Fn(HarnessCatalog, &Preferences, &str) -> DefaultsOn>(
            "LaunchCtx::defaults_on",
            Rc::new(move |_c: HarnessCatalog, _p: &Preferences, _project: &str| resolved.clone()),
        );
        let read = read_picks(&area, &e, &LaunchAsked::default()).await.unwrap();
        assert_eq!(read.catalog, Some(marked), "the catalog the defaults marked, not the one the harness answered");
        assert_eq!((read.picks_for)(None).unwrap(), launch_picks_of(picks(Some("claude-sonnet-5"), Some("low"), Some("default")), None));
        assert_eq!(host.hooks.called("defaults_on"), vec!["defaults_on(p1)".to_string()], "read against the workspace's project");
        // A pick the start names is over all of them.
        let read = read_picks(
            &area,
            &e,
            &LaunchAsked { model: Some("claude-opus-5".into()), permission_mode: Some("bypassPermissions".into()), ..Default::default() },
        )
        .await
        .unwrap();
        assert_eq!(
            (read.picks_for)(None).unwrap(),
            launch_picks_of(picks(Some("claude-opus-5"), Some("low"), Some("bypassPermissions")), None)
        );
    });
}

// L9: thread-keeps-picks.test.ts:140
#[test]
fn a_new_thread_with_no_defaults_opens_on_the_catalogs_marks() {
    run_local(async {
        let (host, area, e) = picks_rig();
        let read = read_picks(&area, &e, &LaunchAsked::default()).await.unwrap();
        assert_eq!(
            (read.picks_for)(None).unwrap(),
            launch_picks_of(picks(Some("claude-opus-5"), Some("high"), Some("bypassPermissions")), None)
        );
        assert_eq!(host.hooks.called("resumed_fact"), Vec::<String>::new(), "a new session resumes nothing");
    });
}

// L9, L11: thread-keeps-picks.test.ts:82
#[test]
fn a_thread_keeps_the_model_effort_and_window_its_turns_ran_on() {
    run_local(async {
        let (host, area, e) = picks_rig();
        set_ran(&host, Rc::new(RefCell::new(ran(Some("claude-opus-5"), Some("low"), Some("1m")))));
        let read = read_picks(&area, &e, &LaunchAsked::default()).await.unwrap();
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(picks(Some("claude-opus-5"), Some("low"), None), Some("1m")));
        assert_eq!(host.hooks.called("ran_on"), vec![r#"ran_on(w1, t1, Some("sess-1"))"#.to_string()]);
        assert_eq!(host.hooks.called("resumed_fact"), vec!["resumed_fact(w1, sess-1, model)".to_string()]);
    });
}

// L9, L11: thread-keeps-picks.test.ts:92
#[test]
fn what_a_send_names_wins_and_the_effort_it_leaves_out_stays_where_the_new_model_takes_it() {
    run_local(async {
        let (host, area, e) = picks_rig();
        set_ran(&host, Rc::new(RefCell::new(ran(Some("claude-opus-5"), Some("low"), Some("1m")))));
        let read = read_picks(&area, &e, &LaunchAsked { effort: Some("max".into()), ..Default::default() }).await.unwrap();
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(picks(Some("claude-opus-5"), Some("max"), None), Some("1m")));
        // Sonnet 5 takes the effort but offers no window, so the window does not ride with it.
        set_ran(&host, Rc::new(RefCell::new(ran(Some("claude-opus-5"), Some("max"), Some("1m")))));
        let read = read_picks(&area, &e, &LaunchAsked { model: Some("claude-sonnet-5".into()), ..Default::default() }).await.unwrap();
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(picks(Some("claude-sonnet-5"), Some("max"), None), None));
        // A window the send names is the window kept.
        let read = read_picks(&area, &e, &LaunchAsked { context_window: Some("1m".into()), ..Default::default() }).await.unwrap();
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(picks(Some("claude-opus-5"), Some("max"), None), Some("1m")));
    });
}

// L9: thread-keeps-picks.test.ts:132
#[test]
fn a_model_the_catalog_dropped_is_left_out_rather_than_refused_and_the_effort_still_rides() {
    run_local(async {
        let (host, area, e) = picks_rig();
        set_ran(&host, Rc::new(RefCell::new(ran(Some("claude-opus-6-preview"), Some("low"), None))));
        let read = read_picks(&area, &e, &LaunchAsked::default()).await.unwrap();
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(picks(None, Some("low"), None), None));
    });
}

// L9: thread-defaults.test.ts:111
#[test]
fn a_resume_never_takes_the_defaults_open_picks() {
    run_local(async {
        let (host, area, e) = picks_rig();
        let resolved = DefaultsOn { catalog: catalog(), open_model: Some("claude-sonnet-5".into()), open_effort: Some("low".into()) };
        host.hooks.set::<dyn Fn(HarnessCatalog, &Preferences, &str) -> DefaultsOn>(
            "LaunchCtx::defaults_on",
            Rc::new(move |_c: HarnessCatalog, _p: &Preferences, _project: &str| resolved.clone()),
        );
        let on = Rc::new(RefCell::new(RanPicks::default()));
        set_ran(&host, on.clone());
        let read = read_picks(&area, &e, &LaunchAsked::default()).await.unwrap();
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(StartPicks::default(), None), "a resume opens on nothing");
        // A new session on a thread that already ran a model opens on that model, and the catalog's mark for the effort.
        *on.borrow_mut() = ran(Some("claude-opus-5"), None, None);
        assert_eq!(
            (read.picks_for)(None).unwrap(),
            launch_picks_of(picks(Some("claude-opus-5"), Some("high"), Some("bypassPermissions")), None)
        );
        // One that ran only an effort keeps it, and the model is the catalog's mark, not the defaults' open one.
        *on.borrow_mut() = ran(None, Some("max"), None);
        assert_eq!(
            (read.picks_for)(None).unwrap(),
            launch_picks_of(picks(Some("claude-opus-5"), Some("max"), Some("bypassPermissions")), None)
        );
    });
}

// L9: thread-defaults.test.ts:123
#[test]
fn an_open_catalog_takes_any_model_named_and_starts_on_the_one_set_for_it() {
    run_local(async {
        let (host, area, e) = picks_rig();
        let open = catalog_over(json!({
            "harness": "opencode", "models": [], "efforts": [], "contextWindows": [],
            "permissionModes": [{ "value": "auto", "label": "auto", "isDefault": true }],
        }));
        set_catalog(&host, Some(open.clone()));
        let resolved = DefaultsOn { catalog: open, open_model: Some("anthropic/claude-sonnet-5".into()), open_effort: None };
        host.hooks.set::<dyn Fn(HarnessCatalog, &Preferences, &str) -> DefaultsOn>(
            "LaunchCtx::defaults_on",
            Rc::new(move |_c: HarnessCatalog, _p: &Preferences, _project: &str| resolved.clone()),
        );
        let read = read_picks(&area, &e, &LaunchAsked::default()).await.unwrap();
        assert_eq!((read.picks_for)(None).unwrap(), launch_picks_of(picks(Some("anthropic/claude-sonnet-5"), None, Some("auto")), None));
        let read = read_picks(&area, &e, &LaunchAsked { model: Some("openai/gpt-x".into()), ..Default::default() }).await.unwrap();
        assert_eq!((read.picks_for)(None).unwrap(), launch_picks_of(picks(Some("openai/gpt-x"), None, Some("auto")), None));
    });
}

// L10: thread-defaults.test.ts:133
#[test]
fn a_refused_model_reads_the_not_run_words() {
    run_local(async {
        let (host, area, e) = picks_rig();
        let asked = LaunchAsked { model: Some("claude-nope".into()), ..Default::default() };
        let refusal = |cat: &HarnessCatalog| start_picks(Some(cat), &picks(Some("claude-nope"), None, None), true, None).unwrap_err();
        // Where nothing says the model once ran, the start's own refusal stands.
        let read = read_picks(&area, &e, &asked).await.unwrap();
        let got = (read.picks_for)(None).unwrap_err();
        let own = refusal(&catalog());
        assert_eq!(said(&got), (own.message, None, Some(Kind::Usage)));
        assert_eq!(host.hooks.called("not_run_refused"), vec![r#"not_run_refused(Some("claude-nope"))"#.to_string()]);
        // Where the runtime has words for a model that is not run, they replace it.
        let words = err("claude-nope is not run on this computer", Some(Kind::Usage));
        let w = words.clone();
        host.hooks.set::<dyn Fn(Option<&HarnessCatalog>, Option<&str>) -> Option<HostError>>(
            "LaunchCtx::not_run_refused",
            Rc::new(move |_c: Option<&HarnessCatalog>, _m: Option<&str>| Some(w.clone())),
        );
        let got = (read.picks_for)(None).unwrap_err();
        assert_eq!(said(&got), said(&words));
    });
}

// L7, L8: launch.ts:237
#[test]
fn no_table_no_catalog() {
    run_local(async {
        let host = FakeHost::new();
        let e = host.add_entry(entry("w1", json!({})));
        let area = LaunchArea::new(host.clone());
        set_ran(&host, Rc::new(RefCell::new(ran(Some("ran-model"), Some("ran-effort"), None))));
        let read = read_picks(&area, &e, &LaunchAsked::default()).await.unwrap();
        assert_eq!(read.catalog, None);
        // The thread's own latest picks stand for whatever the send leaves out, with nothing to check them against.
        assert_eq!((read.picks_for)(Some("sess-1")).unwrap(), launch_picks_of(picks(Some("ran-model"), Some("ran-effort"), None), None));
        let asked = LaunchAsked {
            model: Some("m1".into()),
            effort: Some("e1".into()),
            permission_mode: Some("p1".into()),
            context_window: Some("cw".into()),
            fast: Some(true),
            ..Default::default()
        };
        let read = read_picks(&area, &e, &asked).await.unwrap();
        let got = (read.picks_for)(None).unwrap();
        assert_eq!(
            got,
            LaunchPicks {
                picks: StartPicks {
                    model: Some("m1".into()),
                    effort: Some("e1".into()),
                    permission_mode: Some("p1".into()),
                    fast: Some(true)
                },
                context_window: Some("cw".into())
            }
        );
        assert_eq!(host.hooks.called("defaults_on"), Vec::<String>::new());
        assert_eq!(host.hooks.called("access_of"), Vec::<String>::new());
    });
}

// L12: launch.ts:268
#[test]
fn the_fixed_key_writes_only_present_keys_in_order() {
    let (host, area, e) = fixed_rig();
    assert_eq!(area.launch_fixed(&e, "claude", "/work/w1", None, None), r#"{"harness":"claude","cwd":"/work/w1"}"#);
    assert_eq!(area.launch_fixed(&e, "claude", "/work/w1", None, Some(None)), r#"{"harness":"claude","cwd":"/work/w1","version":null}"#);
    assert_eq!(
        area.launch_fixed(&e, "claude", "/work/w1", None, Some(Some("2.1.0"))),
        r#"{"harness":"claude","cwd":"/work/w1","version":"2.1.0"}"#
    );
    // A place whose setup says nothing for the agent writes no setup key.
    host.hooks.set::<dyn Fn(&Entry) -> Option<String>>("LaunchCtx::setup_place", Rc::new(|_e: &Entry| Some("here".to_string())));
    assert_eq!(area.launch_fixed(&e, "claude", "/work/w1", None, None), r#"{"harness":"claude","cwd":"/work/w1"}"#);
    assert_eq!(host.hooks.called("launch_of"), vec!["launch_of(here, claude)".to_string()]);
}

// L12: launch.ts:268
#[test]
fn the_fixed_key_carries_the_servers_the_version_and_the_setup_whole() {
    let (host, area, e) = fixed_rig();
    host.hooks.set::<dyn Fn(&Entry) -> Option<String>>("LaunchCtx::setup_place", Rc::new(|_e: &Entry| Some("here".to_string())));
    host.hooks.set::<dyn Fn(&str, &str) -> Option<AgentLaunch>>(
        "LaunchCtx::launch_of",
        Rc::new(|_p: &str, _h: &str| Some(AgentLaunch { program: Some("/opt/claude".into()), args: Some(vec!["--debug".into()]) })),
    );
    let servers = json!({ "wsp": { "command": "/bin/wsp", "args": ["mcp"] } });
    assert_eq!(
        area.launch_fixed(&e, "claude", "/work/w1", Some(&servers), Some(Some("2.1.0"))),
        r#"{"harness":"claude","cwd":"/work/w1","mcpServers":{"wsp":{"command":"/bin/wsp","args":["mcp"]}},"version":"2.1.0","setup":{"program":"/opt/claude","args":["--debug"]}}"#
    );
    host.hooks.set::<dyn Fn(&str, &str) -> Option<AgentLaunch>>(
        "LaunchCtx::launch_of",
        Rc::new(|_p: &str, _h: &str| Some(AgentLaunch { program: None, args: Some(vec!["--debug".into()]) })),
    );
    assert_eq!(
        area.launch_fixed(&e, "codex", "/c", None, Some(None)),
        r#"{"harness":"codex","cwd":"/c","version":null,"setup":{"args":["--debug"]}}"#
    );
}

// L13: thread-warm-agent.test.ts:197
#[test]
fn a_threads_token_warms_nothing() {
    run_local(async {
        let r = rig();
        assert_eq!(r.area.warm("w1", ask(), Some(thread_scope())).await.unwrap(), none());
        assert_eq!(r.host.hooks.calls(), vec!["ready".to_string()], "nothing is read for a thread's token");
        // The person's own road, with no thread behind it, is asked as usual.
        assert_eq!(r.area.warm("w1", ask(), Some(Caller::Road(WorkspaceOrigin::Here))).await.unwrap(), started());
        assert_eq!(r.held.borrow().len(), 1);
    });
}

// L14: launch.ts:316
#[test]
fn an_unknown_workspace_is_the_callers_refusal() {
    run_local(async {
        let r = rig();
        let got = r.area.warm("nope", ask(), None).await.unwrap_err();
        assert_eq!(said(&got), ("no workspace nope".to_string(), None, Some(Kind::NotFound)));
        assert_eq!(r.host.hooks.called("adapter_for"), Vec::<String>::new());
    });
}

// L14: launch.ts:320
#[test]
fn a_kind_that_keeps_no_agents_warms_nothing() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&Entry) -> bool>("LaunchCtx::keeps_agents", Rc::new(|_e: &Entry| false));
        assert_eq!(warm(&r, ask()).await.unwrap(), none());
        assert_eq!(r.host.hooks.called("preferences_held"), Vec::<String>::new(), "nothing more is read");
        assert_eq!(r.host.hooks.called("adapter_for"), Vec::<String>::new());
        assert_eq!(r.held.borrow().len(), 0);
    });
}

// L14: launch.ts:320
#[test]
fn a_kept_box_too_small_to_keep_one_warms_nothing_and_one_with_room_warms() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&str) -> Option<u32>>("LaunchCtx::kept_box_cap", Rc::new(|_w: &str| Some(0)));
        assert_eq!(warm(&r, ask()).await.unwrap(), none());
        assert_eq!(r.host.hooks.called("adapter_for"), Vec::<String>::new());
        assert_eq!(r.host.hooks.called("kept_box_cap"), vec!["kept_box_cap(w1)".to_string()]);
        r.host.hooks.set::<dyn Fn(&str) -> Option<u32>>("LaunchCtx::kept_box_cap", Rc::new(|_w: &str| Some(2)));
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
    });
}

// L14: launch.ts:320
#[test]
fn a_closing_host_warms_nothing() {
    run_local(async {
        let r = rig();
        r.host.closing.set(true);
        assert_eq!(warm(&r, ask()).await.unwrap(), none());
        assert_eq!(r.host.hooks.called("adapter_for"), Vec::<String>::new());
        assert_eq!(r.held.borrow().len(), 0);
    });
}

// L15: launch.ts:323
#[test]
fn an_agent_turned_off_warms_nothing() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&Entry, &str) -> bool>("LaunchCtx::agent_off", Rc::new(|_e: &Entry, h: &str| h == "codex"));
        let asked = WarmAsk { harness: Some("codex".into()), ..Default::default() };
        assert_eq!(warm(&r, asked).await.unwrap(), none());
        assert_eq!(r.host.hooks.called("agent_off"), vec!["agent_off(codex)".to_string()]);
        assert_eq!(r.host.hooks.called("adapter_for"), Vec::<String>::new());
        assert_eq!(r.host.hooks.called("default_agent_of"), Vec::<String>::new(), "an agent named is not looked up");
        assert_eq!(warm(&r, ask()).await.unwrap(), started(), "another agent on the same computer warms");
    });
}

// L15: launch.ts:322
#[test]
fn the_default_agent_is_warmed_where_none_is_named_and_held_preferences_are_read_before_the_store() {
    run_local(async {
        let r = rig();
        let mut held = default_preferences();
        held.default_agent = Some("held-agent".into());
        r.host.hooks.set::<dyn Fn() -> Option<Preferences>>("LaunchCtx::preferences_held", Rc::new(move || Some(held.clone())));
        r.host.hooks.set::<dyn Fn(&Preferences, &Entry) -> String>(
            "LaunchCtx::default_agent_of",
            Rc::new(|p: &Preferences, _e: &Entry| p.default_agent.clone().expect("a default agent")),
        );
        assert_eq!(warm(&r, WarmAsk::default()).await.unwrap(), started());
        assert_eq!(
            r.asks.borrow().iter().map(|a| a.0.clone()).collect::<Vec<_>>(),
            vec!["held-agent".to_string(), "held-agent".to_string()]
        );
        assert_eq!(r.held.borrow()[0].1.warm.as_ref().expect("a warm hold").harness, "held-agent");
        assert_eq!(r.host.hooks.called("preferences"), Vec::<String>::new(), "the store is not read where the host holds them");
        // Where it holds none, the stored ones are read.
        let r = rig();
        let mut stored = default_preferences();
        stored.default_agent = Some("stored-agent".into());
        r.host.hooks.set::<dyn Fn() -> LocalBoxFuture<'static, Result<Preferences, HostError>>>(
            "LaunchCtx::preferences",
            Rc::new(move || {
                let p = stored.clone();
                async move { Ok(p) }.boxed_local()
            }),
        );
        r.host.hooks.set::<dyn Fn(&Preferences, &Entry) -> String>(
            "LaunchCtx::default_agent_of",
            Rc::new(|p: &Preferences, _e: &Entry| p.default_agent.clone().expect("a default agent")),
        );
        assert_eq!(warm(&r, WarmAsk::default()).await.unwrap(), started());
        assert_eq!(r.host.hooks.called("preferences"), vec!["preferences".to_string()]);
        assert_eq!(r.held.borrow()[0].1.warm.as_ref().expect("a warm hold").harness, "stored-agent");
    });
}

// L17: launch.ts:276
#[test]
fn an_adapter_with_no_warm_road_answers_none() {
    run_local(async {
        let r = rig_with(FakeAdapter { warms: false, ..FakeAdapter::warming() });
        assert_eq!(warm(&r, ask()).await.unwrap(), none());
        let asks = r.asks.borrow();
        assert_eq!(asks.len(), 1, "only the probe is built");
        assert_eq!(asks[0].0, "claude");
        assert_eq!(asks[0].1.turn_env.as_ref().map_or(0, |e| e.len()), 0, "a probe has no env");
        assert_eq!(asks[0].1.thread.as_deref(), Some("thr-1"));
        for later in ["confine_setup", "catalog_on", "thread_launch", "hold_warm"] {
            assert_eq!(r.host.hooks.called(later), Vec::<String>::new(), "{later} comes after the warm road is found");
        }
    });
}

// L17: launch.ts:283
#[test]
fn a_missing_local_folder_answers_none() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&str) -> bool>("LaunchCtx::folder_exists", Rc::new(|p: &str| p != "/work/w1"));
        assert_eq!(warm(&r, ask()).await.unwrap(), none());
        assert_eq!(r.host.hooks.called("folder_exists"), vec!["folder_exists(/work/w1)".to_string()]);
        assert_eq!(r.host.hooks.called("thread_launch"), Vec::<String>::new());
        assert_eq!(r.held.borrow().len(), 0);
        // The folder the start names is the one looked for.
        let asked = WarmAsk { cwd: Some("/proj/sub".into()), ..ask() };
        assert_eq!(warm(&r, asked).await.unwrap(), started());
        assert_eq!(
            r.host.hooks.called("thread_folder"),
            vec!["thread_folder(None)".to_string(), r#"thread_folder(Some("/proj/sub"))"#.to_string()]
        );
        assert_eq!(r.adapter.warmed()[0].cwd.as_deref(), Some("/proj/sub"));
    });
}

// L17: launch.ts:283
#[test]
fn a_cloud_workspace_never_checks_this_computers_disk() {
    run_local(async {
        let r = rig();
        r.host.add_entry(entry("w2", json!({ "kind": "cloud" })));
        r.host.hooks.set::<dyn Fn(&str) -> bool>("LaunchCtx::folder_exists", Rc::new(|_p: &str| false));
        assert_eq!(r.area.warm("w2", ask(), None).await.unwrap(), started());
        assert_eq!(r.host.hooks.called("folder_exists"), Vec::<String>::new());
        assert_eq!(r.adapter.warmed()[0].cwd.as_deref(), Some("/work/w2"));
    });
}

// L17, L19: thread-warm-agent.test.ts:82
#[test]
fn the_warm_launch_runs_on_the_catalogs_marks_and_is_held_for_the_threads_first_send() {
    run_local(async {
        let r = rig();
        set_catalog(&r.host, Some(catalog()));
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
        // The process comes up on the model the catalog marks, as the composer's first send would run on.
        let w = r.adapter.warmed();
        assert_eq!(w.len(), 1);
        assert_eq!(
            (w[0].model.as_deref(), w[0].effort.as_deref(), w[0].permission_mode.as_deref()),
            (Some("claude-opus-5"), Some("high"), Some("bypassPermissions"))
        );
        let held = r.held.borrow();
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].0, "thr-1");
        assert_eq!(held[0].1.workspace_id, "w1");
        assert_eq!(held[0].1.warm.as_ref().expect("a warm hold").harness, "claude");
        assert_eq!(kept_tag(&held[0].1), "kept-1");
        assert_eq!(
            serde_json::to_value(&held[0].1.launch.picks).unwrap(),
            json!({ "model": "claude-opus-5", "effort": "high", "permissionMode": "bypassPermissions" })
        );
        assert_eq!(held[0].1.launch.fixed, r#"{"harness":"claude","cwd":"/work/w1","version":"2.1.0"}"#);
    });
}
