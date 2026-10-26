// SPDX-License-Identifier: AGPL-3.0-only
//! Part 2 of `launch.rs`: the same world, the tests after it.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("launch/world.rs");

// L19: launch.ts:296
#[test]
fn the_warm_launch_carries_the_token_thread_and_servers_in_order() {
    run_local(async {
        let r = rig_with(FakeAdapter { mcp: true, ..FakeAdapter::warming() });
        set_catalog(&r.host, Some(catalog()));
        let wsp = McpServerSpec { command: "/bin/wsp".into(), args: vec!["mcp".into()], no_slate: None };
        let minted = ThreadLaunch {
            scoped_device_id: Some("dev-1".into()),
            env: IndexMap::from([("WSP_HOST".to_string(), "h1".to_string()), ("WSP_X".to_string(), "x".to_string())]),
            wsp: Some(wsp),
        };
        r.host.hooks.set::<dyn Fn(&Entry, &str, &str, bool) -> LocalBoxFuture<'static, Result<ThreadLaunch, HostError>>>(
            "LaunchCtx::thread_launch",
            Rc::new(move |_e: &Entry, _t: &str, _r: &str, _a: bool| {
                let m = minted.clone();
                async move { Ok(m) }.boxed_local()
            }),
        );
        let vault: Env = IndexMap::from([("VAULT_URL".to_string(), "http://v".to_string())]);
        let v = vault.clone();
        r.host.hooks.set::<dyn Fn() -> Env>("LaunchCtx::vault", Rc::new(move || v.clone()));
        let launch = ServerValues {
            entries: Some(IndexMap::from([("launch".to_string(), IndexMap::from([("url".to_string(), json!("http://l"))]))])),
            config: Some(IndexMap::from([("K".to_string(), "v".to_string())])),
        };
        let l = LaunchServers { server_values: Some(launch.clone()), off_servers: Some(vec!["gone".into()]) };
        r.host.hooks.set::<dyn Fn(&Entry, &str, &dyn HarnessAdapter, &str) -> LocalBoxFuture<'static, Result<LaunchServers, HostError>>>(
            "LaunchCtx::launch_servers_for",
            Rc::new(move |_e: &Entry, _h: &str, _a: &dyn HarnessAdapter, _c: &str| {
                let l = l.clone();
                async move { Ok(l) }.boxed_local()
            }),
        );
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
        // The token is 16 bytes drawn from Ids, in hex; the first draw is the first byte string.
        let token = "01".repeat(16);
        let asks = r.asks.borrow();
        assert_eq!(asks.len(), 2);
        assert_eq!(asks[0].1.turn_env.as_ref().map_or(0, |e| e.len()), 0, "the probe has no env");
        assert_eq!(asks[0].1.thread.as_deref(), Some("thr-1"));
        let env: Vec<(String, String)> =
            asks[1].1.turn_env.as_ref().expect("the warm ask carries the launch env").iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        assert_eq!(
            env,
            vec![
                (TURN_TOKEN_ENV.to_string(), token.clone()),
                (THREAD_ENV.to_string(), "thr-1".to_string()),
                ("WSP_HOST".to_string(), "h1".to_string()),
                ("WSP_X".to_string(), "x".to_string()),
            ]
        );
        assert_eq!(asks[1].1.servers, wsp_catalog::signin_rows::server_values_of(&vault));
        assert_eq!(asks[1].1.thread.as_deref(), Some("thr-1"));
        assert!(!asks[1].1.asks_until_stopped);
        let waiting = asks[1].1.waiting.clone().expect("the adapter reads the waiting box");
        let w = r.adapter.warmed();
        assert_eq!(
            w,
            vec![Warmed {
                cwd: Some("/work/w1".into()),
                model: Some("claude-opus-5".into()),
                effort: Some("high".into()),
                permission_mode: Some("bypassPermissions".into()),
                context_window: None,
                fast: None,
                mcp_servers: Some(json!({ "wsp": { "command": "/bin/wsp", "args": ["mcp"] } })),
                server_values: Some(launch),
                version: Some("2.1.0".into()),
            }]
        );
        let held = r.held.borrow();
        assert_eq!(held[0].0, "thr-1");
        let h = &held[0].1;
        assert_eq!(
            (h.workspace_id.as_str(), harness_of(h).as_str(), h.turn_token.as_str(), h.scope_device_id.as_deref()),
            ("w1", "claude", token.as_str(), Some("dev-1"))
        );
        assert!(!waiting(), "the box is off");
        h.waiting.set(true);
        assert!(waiting(), "the held agent shares the box the adapter reads");
        h.waiting.set(false);
        assert!(!h.waiting.get());
        assert_eq!(r.host.hooks.called("thread_launch"), vec!["thread_launch(thr-1, thr-1, false)".to_string()]);
        assert_eq!(r.host.hooks.called("launch_servers_for"), vec!["launch_servers_for(claude, /work/w1)".to_string()]);
        in_order(
            &r.host.hooks.calls(),
            &[
                "ready",
                "entry_of(w1)",
                "adapter_for(claude)",
                "confine_setup(claude)",
                "catalog_on(claude)",
                "thread_launch(thr-1, thr-1, false)",
                "adapter_for(claude)",
                "launch_servers_for(claude, /work/w1)",
                "hold_warm(thr-1)",
            ],
        );
    });
}

// L19: launch.ts:300
#[test]
fn an_agent_that_takes_no_servers_is_handed_none_and_asked_for_none() {
    run_local(async {
        let r = rig();
        let wsp = McpServerSpec { command: "/bin/wsp".into(), args: vec!["mcp".into()], no_slate: None };
        r.host.hooks.set::<dyn Fn(&Entry, &str, &str, bool) -> LocalBoxFuture<'static, Result<ThreadLaunch, HostError>>>(
            "LaunchCtx::thread_launch",
            Rc::new(move |_e: &Entry, _t: &str, _r: &str, _a: bool| {
                let m = ThreadLaunch { wsp: Some(wsp.clone()), ..Default::default() };
                async move { Ok(m) }.boxed_local()
            }),
        );
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
        let w = r.adapter.warmed();
        assert_eq!((w[0].mcp_servers.clone(), w[0].server_values.clone()), (None, None));
        assert_eq!(r.host.hooks.called("launch_servers_for"), Vec::<String>::new());
    });
}

// L19: launch.ts:301
#[test]
fn an_agent_that_denies_servers_is_asked_for_the_launch_servers_but_handed_no_wsp_server() {
    run_local(async {
        let r = rig_with(FakeAdapter { denies: true, ..FakeAdapter::warming() });
        let wsp = McpServerSpec { command: "/bin/wsp".into(), args: vec!["mcp".into()], no_slate: None };
        r.host.hooks.set::<dyn Fn(&Entry, &str, &str, bool) -> LocalBoxFuture<'static, Result<ThreadLaunch, HostError>>>(
            "LaunchCtx::thread_launch",
            Rc::new(move |_e: &Entry, _t: &str, _r: &str, _a: bool| {
                let m = ThreadLaunch { wsp: Some(wsp.clone()), ..Default::default() };
                async move { Ok(m) }.boxed_local()
            }),
        );
        let launch = ServerValues { entries: None, config: Some(IndexMap::from([("K".to_string(), "v".to_string())])) };
        let l = LaunchServers { server_values: Some(launch.clone()), off_servers: None };
        r.host.hooks.set::<dyn Fn(&Entry, &str, &dyn HarnessAdapter, &str) -> LocalBoxFuture<'static, Result<LaunchServers, HostError>>>(
            "LaunchCtx::launch_servers_for",
            Rc::new(move |_e: &Entry, _h: &str, _a: &dyn HarnessAdapter, _c: &str| {
                let l = l.clone();
                async move { Ok(l) }.boxed_local()
            }),
        );
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
        let w = r.adapter.warmed();
        assert_eq!((w[0].mcp_servers.clone(), w[0].server_values.clone()), (None, Some(launch)));
        assert_eq!(r.host.hooks.called("launch_servers_for"), vec!["launch_servers_for(claude, /work/w1)".to_string()]);
    });
}

// L19: launch.ts:307
#[test]
fn the_version_rides_only_from_a_harness_catalog_that_has_one_and_always_marks_the_fixed_key() {
    run_local(async {
        let cases: [(Option<HarnessCatalog>, Option<&str>, &str); 4] = [
            (None, None, r#"{"harness":"claude","cwd":"/work/w1"}"#),
            (Some(catalog_over(json!({ "version": null }))), None, r#"{"harness":"claude","cwd":"/work/w1","version":null}"#),
            (
                Some(catalog_over(json!({ "source": "table", "version": "9.9.9" }))),
                None,
                r#"{"harness":"claude","cwd":"/work/w1","version":"9.9.9"}"#,
            ),
            (
                Some(catalog_over(json!({ "version": "2.1.0" }))),
                Some("2.1.0"),
                r#"{"harness":"claude","cwd":"/work/w1","version":"2.1.0"}"#,
            ),
        ];
        for (cat, rides, fixed) in cases {
            let r = rig();
            set_catalog(&r.host, cat);
            assert_eq!(warm(&r, ask()).await.unwrap(), started());
            assert_eq!(r.adapter.warmed()[0].version.as_deref(), rides, "{fixed}");
            assert_eq!(r.held.borrow()[0].1.launch.fixed, fixed);
        }
    });
}

// L11, L19: launch.ts:249
#[test]
fn the_window_kept_rides_with_the_warm_picks() {
    run_local(async {
        let r = rig();
        set_catalog(&r.host, Some(catalog()));
        let asked = WarmAsk { context_window: Some("1m".into()), model: Some("claude-opus-5".into()), ..ask() };
        assert_eq!(warm(&r, asked).await.unwrap(), started());
        assert_eq!(r.adapter.warmed()[0].context_window.as_deref(), Some("1m"));
        assert_eq!(held_picks(&r, 0)["model"], "claude-opus-5");
    });
}

// L20: launch.ts:310
#[test]
fn a_failed_warm_revokes_its_device() {
    run_local(async {
        let r = rig_with(FakeAdapter { fail: Some("the agent would not start".into()), ..FakeAdapter::warming() });
        r.host.hooks.set::<dyn Fn(&Entry, &str, &str, bool) -> LocalBoxFuture<'static, Result<ThreadLaunch, HostError>>>(
            "LaunchCtx::thread_launch",
            Rc::new(|_e: &Entry, _t: &str, _r: &str, _a: bool| {
                async move { Ok(ThreadLaunch { scoped_device_id: Some("dev-1".into()), ..Default::default() }) }.boxed_local()
            }),
        );
        let got = warm(&r, ask()).await.unwrap_err();
        assert_eq!(got.message, "the agent would not start", "the failure is the caller's");
        assert_eq!(r.host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)".to_string()]);
        assert_eq!(r.held.borrow().len(), 0);
        in_order(&r.host.hooks.calls(), &["thread_launch(thr-1, thr-1, false)", "revoke_device(dev-1)"]);
    });
}

// L20: launch.ts:310
#[test]
fn a_failure_building_the_launch_adapter_revokes_too_and_the_revokes_own_failure_is_swallowed() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&Entry, &str, &str, bool) -> LocalBoxFuture<'static, Result<ThreadLaunch, HostError>>>(
            "LaunchCtx::thread_launch",
            Rc::new(|_e: &Entry, _t: &str, _r: &str, _a: bool| {
                async move { Ok(ThreadLaunch { scoped_device_id: Some("dev-1".into()), ..Default::default() }) }.boxed_local()
            }),
        );
        let calls = Rc::new(Cell::new(0));
        let (c, a) = (calls.clone(), r.adapter.clone());
        let refused = err("no such agent here", Some(Kind::Usage));
        let rf = refused.clone();
        r.host.hooks.set::<dyn Fn(&Entry, AdapterAsk) -> Result<BuiltAdapter, HostError>>(
            "LaunchCtx::adapter_for",
            Rc::new(move |_e: &Entry, ask: AdapterAsk| {
                let h = ask.named.clone().unwrap_or_default();
                c.set(c.get() + 1);
                if c.get() == 2 {
                    return Err(rf.clone());
                }
                Ok(BuiltAdapter { harness: h, adapter: a.clone() })
            }),
        );
        r.host.hooks.set::<dyn Fn(&str) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "LaunchCtx::revoke_device",
            Rc::new(|_d: &str| async move { Err(err("the device is gone already", None)) }.boxed_local()),
        );
        let got = warm(&r, ask()).await.unwrap_err();
        assert_eq!(said(&got), said(&refused));
        assert_eq!(r.host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)".to_string()]);
    });
}

// L20: launch.ts:297
#[test]
fn a_failure_before_a_device_was_minted_revokes_nothing() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&Entry, &str, &str, bool) -> LocalBoxFuture<'static, Result<ThreadLaunch, HostError>>>(
            "LaunchCtx::thread_launch",
            Rc::new(|_e: &Entry, _t: &str, _r: &str, _a: bool| async move { Err(err("could not mint a device", None)) }.boxed_local()),
        );
        let got = warm(&r, ask()).await.unwrap_err();
        assert_eq!(got.message, "could not mint a device");
        assert_eq!(r.host.hooks.called("revoke_device"), Vec::<String>::new());
        // A launch that minted no scoped device has none to revoke when the warm road fails.
        let r = rig_with(FakeAdapter { fail: Some("boom".into()), ..FakeAdapter::warming() });
        assert_eq!(warm(&r, ask()).await.unwrap_err().message, "boom");
        assert_eq!(r.host.hooks.called("revoke_device"), Vec::<String>::new());
    });
}

// L18: thread-warm-agent.test.ts:124
#[test]
fn a_standing_agent_at_the_same_launch_is_rewarmed() {
    run_local(async {
        let r = rig();
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
        // Asked a minute apart for five minutes, as the composer does while the person types.
        for _ in 0..5 {
            r.host.clock.advance(60_000);
            assert_eq!(warm(&r, ask()).await.unwrap(), standing());
        }
        assert_eq!(r.host.hooks.called("rewarm"), vec!["rewarm(thr-1)".to_string(); 5], "each ask starts the window over");
        assert_eq!(r.held.borrow().len(), 1, "one process was held, the one the first ask started");
        assert_eq!(r.adapter.warmed().len(), 1);
        assert_eq!(r.host.hooks.called("reap_kept"), Vec::<String>::new());
        assert_eq!(r.host.hooks.called("thread_launch").len(), 1, "no device minted for a standing one");
        assert_eq!(r.host.ids.drawn.get(), 1, "the only bytes drawn were the first ask's token");
        in_order(&r.host.hooks.calls(), &["warm_on(w1, claude)", "rewarm(thr-1)"]);
    });
}

// L18: thread-warm-agent.test.ts:100
#[test]
fn an_agent_that_ended_in_its_window_is_not_standing_and_the_next_ask_starts_one() {
    run_local(async {
        let r = rig();
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
        // The keeper's three minutes went by: it ended the agent and no longer stands one.
        r.standing.borrow_mut().clear();
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
        let held = r.held.borrow();
        assert_eq!(held.iter().map(|h| h.0.clone()).collect::<Vec<_>>(), vec!["thr-1".to_string(), "thr-2".to_string()]);
        assert_eq!(held.iter().map(|h| kept_tag(&h.1)).collect::<Vec<_>>(), vec!["kept-1".to_string(), "kept-2".to_string()]);
        assert_eq!(r.host.hooks.called("rewarm"), Vec::<String>::new());
        assert_eq!(r.host.hooks.called("reap_kept"), Vec::<String>::new(), "nothing stood to reap");
    });
}

// L18: thread-warm-agent.test.ts:141
#[test]
fn a_changed_pick_reaps_the_standing_one_and_starts_another() {
    run_local(async {
        let r = rig();
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
        assert_eq!(warm(&r, ask_with(Some("claude-sonnet-5"), None, None)).await.unwrap(), started());
        assert_eq!(warm(&r, ask_with(Some("claude-sonnet-5"), Some("low"), None)).await.unwrap(), started());
        assert_eq!(warm(&r, ask_with(Some("claude-sonnet-5"), Some("low"), Some("acceptEdits"))).await.unwrap(), started());
        assert_eq!(
            r.host.hooks.called("reap_kept"),
            vec!["reap_kept(thr-1)".to_string(), "reap_kept(thr-2)".to_string(), "reap_kept(thr-3)".to_string()]
        );
        assert_eq!(held_picks(&r, 0), json!({}));
        assert_eq!(held_picks(&r, 1), json!({ "model": "claude-sonnet-5" }));
        assert_eq!(held_picks(&r, 2), json!({ "model": "claude-sonnet-5", "effort": "low" }));
        assert_eq!(held_picks(&r, 3), json!({ "model": "claude-sonnet-5", "effort": "low", "permissionMode": "acceptEdits" }));
        let w = r.adapter.warmed();
        assert_eq!(
            (w[1].model.as_deref(), w[2].effort.as_deref(), w[3].permission_mode.as_deref()),
            (Some("claude-sonnet-5"), Some("low"), Some("acceptEdits"))
        );
        assert_eq!(r.host.hooks.called("rewarm"), Vec::<String>::new());
        // The one standing is the last, and each ask reaped the one before it before it started its own.
        in_order(
            &r.host.hooks.calls(),
            &[
                "reap_kept(thr-1)",
                "thread_launch(thr-2, thr-2, false)",
                "hold_warm(thr-2)",
                "reap_kept(thr-2)",
                "thread_launch(thr-3, thr-3, false)",
            ],
        );
        assert_eq!(r.standing.borrow().values().map(|v| v.0.clone()).collect::<Vec<_>>(), vec!["thr-4".to_string()]);
    });
}

// L18: thread-warm-agent.test.ts:174
#[test]
fn a_standing_agent_with_a_pick_the_ask_leaves_out_is_reaped_and_so_is_one_lacking_a_pick_the_ask_names() {
    run_local(async {
        let r = rig();
        // Fast never rides unasked: the process launched with it is not the launch of an ask that leaves it out.
        assert_eq!(warm(&r, WarmAsk { fast: Some(true), ..ask() }).await.unwrap(), started());
        assert_eq!(r.adapter.warmed()[0].fast, Some(true));
        assert_eq!(held_picks(&r, 0), json!({ "fast": true }));
        assert_eq!(warm(&r, ask()).await.unwrap(), started());
        assert_eq!(r.host.hooks.called("reap_kept"), vec!["reap_kept(thr-1)".to_string()]);
        assert_eq!(r.adapter.warmed()[1].fast, None);
        // And the other way: an ask naming a pick the standing one lacks.
        assert_eq!(warm(&r, WarmAsk { fast: Some(true), ..ask() }).await.unwrap(), started());
        assert_eq!(r.host.hooks.called("reap_kept"), vec!["reap_kept(thr-1)".to_string(), "reap_kept(thr-2)".to_string()]);
        assert_eq!(r.host.hooks.called("rewarm"), Vec::<String>::new());
    });
}

// L18: launch.ts:289
#[test]
fn a_different_folder_is_a_different_launch_and_reaps_the_standing_one() {
    run_local(async {
        let r = rig();
        assert_eq!(warm(&r, WarmAsk { cwd: Some("/a".into()), ..ask() }).await.unwrap(), started());
        assert_eq!(warm(&r, WarmAsk { cwd: Some("/a".into()), ..ask() }).await.unwrap(), standing());
        assert_eq!(warm(&r, WarmAsk { cwd: Some("/b".into()), ..ask() }).await.unwrap(), started());
        assert_eq!(r.host.hooks.called("reap_kept"), vec!["reap_kept(thr-1)".to_string()]);
        assert_eq!(
            r.held.borrow().iter().map(|h| h.1.launch.fixed.clone()).collect::<Vec<_>>(),
            vec![r#"{"harness":"claude","cwd":"/a"}"#.to_string(), r#"{"harness":"claude","cwd":"/b"}"#.to_string()]
        );
    });
}

// L18: thread-warm-agent.test.ts:252
#[test]
fn each_workspace_and_agent_keeps_its_own_standing_one() {
    run_local(async {
        let r = rig();
        for id in ["w2", "w3", "w4"] {
            r.host.add_entry(entry(id, json!({})));
        }
        for id in ["w1", "w2", "w3"] {
            assert_eq!(r.area.warm(id, ask(), None).await.unwrap(), started());
        }
        assert_eq!(
            r.host.hooks.called("warm_on"),
            vec!["warm_on(w1, claude)".to_string(), "warm_on(w2, claude)".to_string(), "warm_on(w3, claude)".to_string()]
        );
        // Asked for again, the second is standing; asking in a fourth folder reaps none of the three: how many stand is
        // the keeper's to hold.
        assert_eq!(r.area.warm("w2", ask(), None).await.unwrap(), standing());
        assert_eq!(r.area.warm("w4", ask(), None).await.unwrap(), started());
        assert_eq!(r.host.hooks.called("rewarm"), vec!["rewarm(thr-2)".to_string()]);
        assert_eq!(r.host.hooks.called("reap_kept"), Vec::<String>::new());
        assert_eq!(
            r.held.borrow().iter().map(|h| h.1.workspace_id.clone()).collect::<Vec<_>>(),
            vec!["w1".to_string(), "w2".to_string(), "w3".to_string(), "w4".to_string()]
        );
        // Another agent in a folder with one standing is no standing one for it.
        assert_eq!(r.area.warm("w1", WarmAsk { harness: Some("codex".into()), ..Default::default() }, None).await.unwrap(), started());
        assert_eq!(r.host.hooks.called("reap_kept"), Vec::<String>::new());
    });
}

// L16: thread-warm-agent.test.ts:124
#[test]
fn a_second_ask_waits_for_the_first_and_one_stands() {
    run_local(async {
        let r = rig();
        let (open, gate) = futures::channel::oneshot::channel();
        gate_confine_setup(&r.host, vec![gate]);
        let first = ask_in_flight(&r, "w1", ask());
        settle().await;
        let second = ask_in_flight(&r, "w1", ask());
        settle().await;
        assert_eq!(r.host.hooks.called("confine_setup").len(), 1, "the second has not begun while the first runs");
        assert_eq!(r.asks.borrow().len(), 1, "no adapter built for the second yet");
        open.send(Ok(())).unwrap();
        assert_eq!(first.await.unwrap().unwrap(), started());
        assert_eq!(second.await.unwrap().unwrap(), standing(), "the second reads the one the first started");
        assert_eq!(r.held.borrow().len(), 1);
        assert_eq!(r.host.hooks.called("rewarm"), vec!["rewarm(thr-1)".to_string()]);
        assert_eq!(r.adapter.warmed().len(), 1);
    });
}

// L16: launch.ts:326
#[test]
fn a_third_ask_waits_for_the_second_not_only_the_first() {
    run_local(async {
        let r = rig();
        let (open_one, gate_one) = futures::channel::oneshot::channel();
        let (open_two, gate_two) = futures::channel::oneshot::channel();
        gate_confine_setup(&r.host, vec![gate_one, gate_two]);
        let first = ask_in_flight(&r, "w1", ask());
        settle().await;
        let second = ask_in_flight(&r, "w1", ask());
        let third = ask_in_flight(&r, "w1", ask());
        settle().await;
        open_one.send(Ok(())).unwrap();
        assert_eq!(first.await.unwrap().unwrap(), started());
        settle().await;
        assert_eq!(r.host.hooks.called("confine_setup").len(), 2, "the second runs, the third still waits on it");
        open_two.send(Ok(())).unwrap();
        assert_eq!(second.await.unwrap().unwrap(), standing());
        assert_eq!(third.await.unwrap().unwrap(), standing());
        assert_eq!(r.host.hooks.called("confine_setup").len(), 3);
        assert_eq!(r.held.borrow().len(), 1);
        assert_eq!(r.host.hooks.called("rewarm"), vec!["rewarm(thr-1)".to_string(), "rewarm(thr-1)".to_string()]);
    });
}

// L16: launch.ts:326
#[test]
fn a_second_ask_runs_its_own_after_the_first_failed() {
    run_local(async {
        let r = rig();
        let (open, gate) = futures::channel::oneshot::channel();
        gate_confine_setup(&r.host, vec![gate]);
        let first = ask_in_flight(&r, "w1", ask());
        settle().await;
        let second = ask_in_flight(&r, "w1", ask());
        settle().await;
        let refused = err("the setup folder is gone", Some(Kind::Usage));
        open.send(Err(refused.clone())).unwrap();
        assert_eq!(said(&first.await.unwrap().unwrap_err()), said(&refused));
        assert_eq!(second.await.unwrap().unwrap(), started(), "the first's failure is not the second's");
        assert_eq!(r.held.borrow().len(), 1);
        // The key was dropped once the newest settled: a later ask is not held back.
        assert_eq!(warm(&r, ask()).await.unwrap(), standing());
    });
}

// L16: launch.ts:325
#[test]
fn an_ask_for_another_agent_or_workspace_does_not_wait() {
    run_local(async {
        let r = rig();
        r.host.add_entry(entry("w2", json!({})));
        let (open, gate) = futures::channel::oneshot::channel();
        gate_confine_setup(&r.host, vec![gate]);
        let first = ask_in_flight(&r, "w1", ask());
        settle().await;
        let other_agent = ask_in_flight(&r, "w1", WarmAsk { harness: Some("codex".into()), ..Default::default() });
        let other_workspace = ask_in_flight(&r, "w2", ask());
        assert_eq!(other_agent.await.unwrap().unwrap(), started());
        assert_eq!(other_workspace.await.unwrap().unwrap(), started());
        assert_eq!(
            r.held.borrow().iter().map(|h| (h.1.workspace_id.clone(), harness_of(&h.1))).collect::<Vec<_>>(),
            vec![("w1".to_string(), "codex".to_string()), ("w2".to_string(), "claude".to_string())]
        );
        open.send(Ok(())).unwrap();
        assert_eq!(first.await.unwrap().unwrap(), started());
    });
}

// L21: lost-session-line.test.ts:102
#[test]
fn logs_one_line_naming_what_the_machine_holds() {
    run_local(async {
        let lines = console::capture();
        let (sessions, seeds) = (Rc::new(RefCell::new(Vec::new())), Rc::new(Cell::new(0)));
        let exec = Arc::new(HeldExec { commands: Mutex::new(Vec::new()) });
        let seed = seed_after_loss(Some(reading(sessions.clone())), exec.clone(), lost_at(), counting_seed(seeds.clone()));
        assert_eq!(seed().await.unwrap(), "the thread so far");
        settle().await;
        let words = "boot b1; rollout files: none; index row: state_5.sqlite";
        assert_eq!(lines.lines(), vec![lost_session_log_line(FIRST, SECOND, "ws_a", words)]);
        assert_eq!(*sessions.borrow(), vec![FIRST.to_string()], "the read names the lost session");
        assert_eq!(*exec.commands.lock().unwrap(), vec![format!("held {FIRST}")], "and runs on the machine's exec it was handed");
        assert!(!lines.lines()[0].contains("the thread so far"), "the seed is never logged");
    });
}

// L21, L22: lost-session-line.test.ts:114
#[test]
fn hands_the_seed_over_and_logs_why_when_the_reading_throws() {
    run_local(async {
        let lines = console::capture();
        let held: SessionHeld =
            Rc::new(|_s: String, _e: Arc<dyn HarnessExec>| async { Err("threadId must be a plain slug".to_string()) }.boxed_local());
        let seeds = Rc::new(Cell::new(0));
        let seed = seed_after_loss(Some(held), Arc::new(NoExec), lost_at(), counting_seed(seeds.clone()));
        assert_eq!(seed().await.unwrap(), "the thread so far");
        settle().await;
        assert_eq!(lines.lines(), vec![lost_session_log_line(FIRST, SECOND, "ws_a", "not read: threadId must be a plain slug")]);
    });
}

// L22: launch.ts:343
#[test]
fn the_seed_is_handed_over_while_the_read_is_still_pending_and_its_own_failure_is_its_own() {
    run_local(async {
        let lines = console::capture();
        let (open, gate) = futures::channel::oneshot::channel::<()>();
        let gate = Rc::new(RefCell::new(Some(gate)));
        let held: SessionHeld = Rc::new(move |_s: String, _e: Arc<dyn HarnessExec>| {
            let g = gate.borrow_mut().take();
            async move {
                if let Some(g) = g {
                    let _ = g.await;
                }
                Ok("boot b2".to_string())
            }
            .boxed_local()
        });
        let seeds = Rc::new(Cell::new(0));
        let seed = seed_after_loss(Some(held), Arc::new(NoExec), lost_at(), counting_seed(seeds.clone()));
        assert_eq!(seed().await.unwrap(), "the thread so far", "the seed does not wait for the read");
        settle().await;
        assert_eq!(lines.lines(), Vec::<String>::new(), "nothing is said while the machine has not answered");
        open.send(()).unwrap();
        settle().await;
        assert_eq!(lines.lines(), vec![lost_session_log_line(FIRST, SECOND, "ws_a", "boot b2")]);
        // A seed that fails fails its caller as it did, and still the read happened once.
        let lines = console::capture();
        let failing: Seed = Rc::new(|| async { Err(err("the thread could not be read", None)) }.boxed_local());
        let sessions = Rc::new(RefCell::new(Vec::new()));
        let seed =
            seed_after_loss(Some(reading(sessions.clone())), Arc::new(HeldExec { commands: Mutex::new(Vec::new()) }), lost_at(), failing);
        assert_eq!(seed().await.unwrap_err().message, "the thread could not be read");
        settle().await;
        assert_eq!(lines.lines().len(), 1);
        assert_eq!(sessions.borrow().len(), 1);
    });
}

// L23: lost-session-line.test.ts:125
#[test]
fn logs_nothing_and_reads_nothing_without_session_held() {
    run_local(async {
        let lines = console::capture();
        let exec = Arc::new(HeldExec { commands: Mutex::new(Vec::new()) });
        let seeds = Rc::new(Cell::new(0));
        let seed = seed_after_loss(None, exec.clone(), lost_at(), counting_seed(seeds.clone()));
        assert_eq!(seed().await.unwrap(), "the thread so far");
        assert_eq!(seed().await.unwrap(), "the thread so far");
        settle().await;
        assert_eq!(lines.lines(), Vec::<String>::new());
        assert_eq!(*exec.commands.lock().unwrap(), Vec::<String>::new());
        assert_eq!(seeds.get(), 2, "every ask is answered by the seed");
    });
}

// L21, L22: launch.ts:343
#[test]
fn a_second_ask_reads_nothing_more() {
    run_local(async {
        let lines = console::capture();
        let (sessions, seeds) = (Rc::new(RefCell::new(Vec::new())), Rc::new(Cell::new(0)));
        let exec = Arc::new(HeldExec { commands: Mutex::new(Vec::new()) });
        let seed = seed_after_loss(Some(reading(sessions.clone())), exec.clone(), lost_at(), counting_seed(seeds.clone()));
        for _ in 0..3 {
            assert_eq!(seed().await.unwrap(), "the thread so far");
            settle().await;
        }
        assert_eq!(*sessions.borrow(), vec![FIRST.to_string()], "one read for the first ask");
        assert_eq!(lines.lines().len(), 1);
        assert_eq!(exec.commands.lock().unwrap().len(), 1);
        assert_eq!(seeds.get(), 3);
    });
}
