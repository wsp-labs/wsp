// SPDX-License-Identifier: AGPL-3.0-only
//! The op switch's own ops: the prefix ops, sessions, images, usage, forwards, folders, agents, editors, the host's
//! doors and the catch. Each TS case is ported at this crate's seam: the area is scripted, and the test reads what the
//! socket sent and what the runtime was asked. Spec: spec/serve_ops.md.
#![allow(dead_code, unused_imports)]
mod support;

include!("serve_ops/world.rs");

// ---------------------------------------------------------------------------------------------------------------
// O1. The prefix ops.

// O1. serve.ts:869-878
#[test]
fn prefix_ops_spread_their_helpers_answer() {
    block(async {
        let r = rig(|o| o.broker = Some(Rc::new(Broker { github: Some("zingzy".into()) })));
        let rt = &r.rt;
        let (host, sink) = r.host().await;

        let view = SlateView {
            thread_id: "t1".into(),
            workspace_id: "ws_1".into(),
            version: 2,
            revision: 2,
            document: None,
            values: IndexMap::new(),
            empty: Some(SlateEmpty::None),
            comments: vec![],
            approvals: IndexMap::new(),
            asks: vec![],
            problems: vec![],
            shown_once: false,
            can_undo: false,
            updated_at: 0,
        };
        let shown = json!({ "id": 1, "ok": true, "slate": view.to_json() }).to_string();
        rt.script.once::<Result<Option<SlateView>, HostError>>("slates.get", Ok(Some(view)));
        request(&host, &sink, 1, "slates.get", json!({ "threadId": "t1" })).await;
        assert_eq!(sink.last_text(), shown);

        rt.script.once::<Result<IndexMap<String, wsp_wire::types::thread_defaults::ThreadDefaults>, HostError>>(
            "projects.defaults",
            Ok(IndexMap::new()),
        );
        request(&host, &sink, 2, "projects.defaults", json!({})).await;
        assert_eq!(sink.last_text(), r#"{"id":2,"ok":true,"defaults":{}}"#);

        request(&host, &sink, 3, "appProcs.watch", json!({})).await;
        assert_eq!(sink.last_text(), r#"{"id":3,"ok":true,"procs":[]}"#, "the hub's answer, from the socket's own road");

        rt.script.once::<Result<ActivityEnded, HostError>>(
            "activity.end",
            Ok(from(json!({ "pid": 4200, "ended": [], "forced": [], "left": [] }))),
        );
        request(&host, &sink, 4, "activity.end", json!({ "pid": 4200, "startedAt": 7 })).await;
        assert_eq!(sink.last_text(), r#"{"id":4,"ok":true,"pid":4200,"ended":[],"forced":[],"left":[]}"#);

        rt.script.once::<Result<wsp_wire::types::conversations::ConversationsAnswer, HostError>>(
            "conversations.list",
            Ok(from(json!({ "rows": [], "held": [] }))),
        );
        request(&host, &sink, 5, "conversations.list", json!({})).await;
        assert_eq!(sink.last_text(), r#"{"id":5,"ok":true,"rows":[],"held":[]}"#);

        request(&host, &sink, 6, "broker.status", json!({})).await;
        assert_eq!(sink.last_text(), r#"{"id":6,"ok":true,"github":"zingzy"}"#);
        assert!(r.failed.borrow().is_empty());
    })
}

// O1. serve.ts:877
#[test]
fn sign_in_ops_are_the_own_road() {
    block(async {
        let r = rig(|_| {});
        let (host, host_sink) = r.host().await;
        request(&host, &host_sink, 1, "agents.signIns", json!({})).await;
        assert_eq!(r.rt.script.calls_to("agents.signIns").len(), 1, "the own road reaches the agents' sign-in runs");

        let (ticketed, ticket_sink) = r.ticketed().await;
        let refused = request(&ticketed, &ticket_sink, 1, "agents.signIns", json!({})).await;
        assert_eq!(refused, ticket_reply(1));
        assert_eq!(r.rt.script.calls_to("agents.signIns").len(), 1, "a socket off the own road reaches no sign-in road");
        let stop = request(&ticketed, &ticket_sink, 2, "agents.signInStop", json!({ "signInId": "si_1" })).await;
        assert_eq!(stop, ticket_reply(2));
        assert!(r.rt.script.calls_to("agents.signInStop").is_empty() && r.rt.script.calls_to("agents.signIn").is_empty());
        assert!(r.failed.borrow().is_empty(), "a refusal the switch sends is not a failed op");
    })
}

// ---------------------------------------------------------------------------------------------------------------
// O2. The catch.

// O2. serve-refusal.test.ts:50
#[test]
fn a_failed_op_carries_kind_fix_and_left_and_tells_failed() {
    block(async {
        let r = rig(|_| {});
        let left: Vec<wsp_wire::views::activity::LeftThread> = from(json!([{
            "threadId": "t-1", "title": "fix login", "workspaceId": "ws-1", "memory": 100, "cpu": 0,
            "processes": [{ "pid": 4200, "ppid": 1, "depth": 0, "startedAt": 1, "kind": "dev", "name": "vite", "command": "node vite", "memory": 100, "cpu": 0, "ports": [5173] }],
        }]));
        let e = processes_left_refusal(LeftVerb::Settle, &left, None);
        r.rt.script.once::<Result<wsp_wire::wire::runtime::SessionSettleResult, HostError>>("sessions.settle", Err(e.clone()));
        let (host, sink) = r.host().await;
        let reply = request(&host, &sink, 1, "sessions.settle", json!({ "threadIds": ["t-1"], "processes": "ask" })).await;
        let expected = json!({
            "id": 1, "ok": false, "error": e.message, "kind": "processes-left", "fix": e.fix.clone().unwrap(), "left": serde_json::to_value(&left).unwrap(),
        });
        assert_eq!(reply, expected);
        assert_eq!(reply.to_string(), expected.to_string(), "error, kind, fix, then left");
        assert_eq!(*r.failed.borrow(), vec![("sessions.settle".to_string(), e.message.clone())]);

        let still = request(&host, &sink, 2, "sessions.list", json!({})).await;
        assert_eq!(still["ok"], json!(false), "the socket stays and answers the next frame (here: unscripted)");
    })
}

// O2. serve-refusal.test.ts:42
#[test]
fn a_refusal_carries_the_fix_beside_the_sentence() {
    block(async {
        let key = "sk-ant-x-fake-refusal-key";
        let e = refusal(&format!("Box refused {key}\nits answer was 401"), "Check it and paste it again.", Some(Kind::Auth));
        let door = Rc::new(RefusingInit(e.clone()));
        let r = rig(|o| o.init = Some(door));
        let (host, sink) = r.host().await;
        let reply = request(&host, &sink, 1, "init.keys", json!({ "provider": "box", "key": key })).await;
        let expected = json!({ "id": 1, "ok": false, "error": e.message, "kind": "auth", "fix": "Check it and paste it again." });
        assert_eq!(reply.to_string(), expected.to_string());
        assert_eq!(*r.failed.borrow(), vec![("init.keys".to_string(), e.message.clone())]);
        let logged = r.logs.borrow().clone();
        assert_eq!(logged.len(), 1, "one line for the refusal, after it was sent");
        assert!(logged[0].starts_with("refused init.keys kind=auth: Box refused "));
        assert!(!logged[0].contains(key), "the key the request carried is never logged");
    })
}

// O2. A failure with no kind and no fix carries its sentence alone; a slate write's lists ride beside it.
#[test]
fn a_failure_carries_only_what_it_has() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        r.rt.script.once::<Result<SlateWriteAnswer, HostError>>("slates.write", Err(HostError::new("boom")));
        let plain = request(&host, &sink, 1, "slates.write", json!({ "threadId": "t1", "text": "x" })).await;
        assert_eq!(plain.to_string(), json!({ "id": 1, "ok": false, "error": "boom" }).to_string());

        let bad = HostError {
            errors: Some(vec![json!({ "path": "rows", "said": "a row needs a label" })]),
            warnings: Some(vec![json!({ "path": "title", "said": "long" })]),
            ..HostError::new("the slate is not valid")
        };
        r.rt.script.once::<Result<SlateWriteAnswer, HostError>>("slates.write", Err(bad));
        let lists = request(&host, &sink, 2, "slates.write", json!({ "threadId": "t1", "text": "x" })).await;
        let expected = json!({
            "id": 2, "ok": false, "error": "the slate is not valid",
            "errors": [{ "path": "rows", "said": "a row needs a label" }], "warnings": [{ "path": "title", "said": "long" }],
        });
        assert_eq!(lists.to_string(), expected.to_string());
        assert_eq!(r.failed_ops(), vec!["slates.write", "slates.write"]);

        let not_left = HostError { left: Some(vec![json!({ "threadId": "t" })]), ..HostError::with_kind("no", Kind::Usage) };
        r.rt.script.once::<Result<SlateWriteAnswer, HostError>>("slates.write", Err(not_left));
        let no_left = request(&host, &sink, 3, "slates.write", json!({ "threadId": "t1", "text": "x" })).await;
        assert!(no_left.get("left").is_none(), "left rides only on a processes-left refusal");
        assert_eq!(no_left["kind"], json!("usage"));
    })
}

// O2, R5. A refusal the switch sends itself is not told to failed.
#[test]
fn a_refusal_the_switch_sends_is_not_told_to_failed() {
    block(async {
        let r = rig(|_| {});
        let (ticketed, sink) = r.ticketed().await;
        request(&ticketed, &sink, 1, "usage.accounts", json!({})).await;
        let (host, host_sink) = r.host().await;
        request(&host, &host_sink, 2, "places.readings", json!({ "range": "day" })).await;
        assert!(r.failed.borrow().is_empty());
        request(&host, &host_sink, 3, "sessions.list", json!({})).await;
        assert_eq!(r.failed_ops(), vec!["sessions.list"], "only the catch tells it");
    })
}

// ---------------------------------------------------------------------------------------------------------------
// O3. A door the host did not wire.

// O3. serve.test.ts:1186
#[test]
fn each_unwired_door_refuses_in_its_own_sentence() {
    block(async {
        let r = rig(|_| {});
        r.rt.script
            .on::<Result<WorkspaceView, HostError>>("workspaces.get", |_| Ok(workspace("ws_1", "app", "local", "running", json!({}))));
        let (host, sink) = r.host().await;
        let cases: Vec<(&str, Value, &str)> = vec![
            ("host.folders", json!({}), NO_FOLDERS),
            ("host.terminalConfig", json!({}), NO_TERMINAL_CONFIG),
            ("editor.list", json!({}), NO_EDITOR),
            ("editor.open", json!({ "workspaceId": "ws_1", "path": "a.rs", "editor": "zed" }), NO_EDITOR),
            ("ssh.include", json!({}), NO_SSH),
            ("ssh.port", json!({ "workspaceId": "ws_1" }), NO_SSH),
            ("release.get", json!({}), NO_RELEASE),
            ("release.check", json!({}), NO_RELEASE),
            ("init.get", json!({}), NO_INIT),
            ("init.keys", json!({ "provider": "box" }), NO_INIT),
            ("project.plan", json!({ "source": "/src" }), NO_BUNDLER),
            ("project.import", json!({ "workspaceId": "ws_1", "source": "/src", "dest": "/d" }), NO_BUNDLER),
            ("project.export", json!({ "workspaceId": "ws_1", "source": "/src", "dest": "/d" }), NO_LANDER),
        ];
        for (i, (op, fields, sentence)) in cases.iter().enumerate() {
            let id = (i + 1) as u64;
            let reply = request(&host, &sink, id, op, fields.clone()).await;
            assert_eq!(reply.to_string(), json!({ "id": id, "ok": false, "error": sentence }).to_string(), "{op}");
        }
        let ops: Vec<String> = cases.iter().map(|(op, _, _)| op.to_string()).collect();
        assert_eq!(r.failed_ops(), ops, "each is the catch's, told in turn");
        assert!(
            r.rt.script.calls_to("projects.import").is_empty() && r.rt.script.calls_to("projects.export").is_empty(),
            "the door is read before the area is asked"
        );
        assert_eq!(
            r.rt.script.calls_to("workspaces.get").len(),
            1,
            "ssh.port reads its door before the workspace; only editor.open read one"
        );
        let (_, again) = r.host().await;
        assert!(again.frames().len() == 1, "the host started and served without any of them");
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Sessions.

fn started(outcome: &str) -> SessionStartResult {
    from(json!({
        "session": { "id": "s1", "workspaceId": "ws_1", "harness": "claude", "status": "running" },
        "outcome": outcome, "turnId": "tu_1",
    }))
}

/// A started turn that answers the view, outcome and turn id of a canned start answer.
struct Handle(SessionStartResult);

impl SessionHandle for Handle {
    fn id(&self) -> String {
        self.0.session.id.clone()
    }
    fn workspace_id(&self) -> String {
        self.0.session.workspace_id.clone()
    }
    fn finished(&self) -> Shared<LocalBoxFuture<'static, TurnResult>> {
        futures::future::pending::<TurnResult>().boxed_local().shared()
    }
    fn turn_id(&self) -> String {
        self.0.turn_id.clone()
    }
    fn outcome(&self) -> SessionStartOutcome {
        self.0.outcome
    }
    fn notice(&self) -> Option<String> {
        self.0.notice.clone()
    }
    fn view(&self) -> SessionView {
        self.0.session.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
    fn steer(&self, _prompt: String, _id: Option<String>, _images: Vec<TurnImage>) -> Option<Local<'static, SteerTaken>> {
        None
    }
    fn tells_unread(&self) -> Option<bool> {
        None
    }
    fn answer(&self, _ask_id: String, _option_id: String, _reason: Option<String>) -> Option<Local<'static, SessionAnswerResult>> {
        None
    }
    fn set_access(&self, _mode: String) -> Option<Local<'static, AccessMove>> {
        None
    }
    fn stop_task(&self, _task: String) -> Option<Local<'static, TaskStop>> {
        None
    }
}

fn folder(workspace_id: &str, cwd: Option<&str>) -> FolderFor {
    FolderFor { workspace: workspace(workspace_id, "app", "local", "running", json!({})), cwd: cwd.map(String::from), setup: None }
}

// O4. serve.test.ts:634
#[test]
fn sessions_start_sends_every_answer_under_one_id() {
    block(async {
        let r = rig(|_| {});
        let rt = &r.rt;
        rt.script.once::<Result<FolderFor, HostError>>("workspaces.folderFor", Ok(folder("ws_1", None)));
        rt.script.once::<Result<Rc<dyn SessionHandle>, HostError>>(
            "sessions.start",
            Ok(Rc::new(Handle(started("started"))) as Rc<dyn SessionHandle>),
        );
        let (host, sink) = r.host().await;
        send(&host, json!({ "id": 7, "op": "sessions.start", "prompt": "go", "project": "app", "answerHeld": true })).await;
        let replies = sink.replies(7);
        assert_eq!(replies.len(), 1, "the start's one answer");
        for reply in &replies {
            assert_eq!(&keys(reply)[..2], ["id", "ok"]);
            assert_eq!(reply["ok"], json!(true));
            assert_eq!(reply["outcome"], json!("started"));
            assert_eq!(reply["turnId"], json!("tu_1"));
        }
        let start = args_of(rt, "sessions.start", 0);
        assert_eq!(start["workspaceId"], json!("ws_1"), "the folder found is the one started in");
        assert_eq!(start["opts"]["onHeld"], json!(true), "answerHeld rides to the start road");
    })
}

// O5. thread-warm-agent.test.ts
#[test]
fn sessions_warm_strips_the_folder_fields() {
    block(async {
        let r = rig(|_| {});
        let rt = &r.rt;
        rt.script.once::<Result<FolderFor, HostError>>("workspaces.folderFor", Ok(folder("ws_1", Some("/at"))));
        rt.script
            .once::<Result<wsp_wire::wire::runtime::SessionWarmResult, HostError>>("sessions.warm", Ok(from(json!({ "warm": "started" }))));
        let (host, sink) = r.host().await;
        let frame = json!({ "project": "app", "cwd": "/ignored", "harness": "claude", "model": "opus", "fast": true });
        let reply = request(&host, &sink, 1, "sessions.warm", frame.clone()).await;
        assert_eq!(reply.to_string(), r#"{"id":1,"ok":true,"warm":"started"}"#);
        let ask = WarmAsk {
            harness: Some("claude".into()),
            cwd: Some("/at".into()),
            model: Some("opus".into()),
            fast: Some(true),
            ..Default::default()
        };
        let warm = args_of(rt, "sessions.warm", 0);
        assert_eq!(warm["workspaceId"], json!("ws_1"));
        assert_eq!(warm["ask"], json!(format!("{ask:?}")), "the folder's cwd, not the frame's; no project");
        let at = args_of(rt, "workspaces.folderFor", 0)["ask"].clone();
        let asked = FolderForAsk {
            project: Some("app".into()),
            cwd: Some("/ignored".into()),
            picks: Some(StartPicksAsked {
                harness: Some("claude".into()),
                model: Some("opus".into()),
                fast: Some(true),
                ..Default::default()
            }),
            opens: false,
            ..Default::default()
        };
        assert_eq!(at, json!(format!("{asked:?}")), "the folder is found from the frame as sent, and a warm opens no setup");

        rt.script.once::<Result<FolderFor, HostError>>("workspaces.folderFor", Ok(folder("ws_2", None)));
        rt.script
            .once::<Result<wsp_wire::wire::runtime::SessionWarmResult, HostError>>("sessions.warm", Ok(from(json!({ "warm": "none" }))));
        request(&host, &sink, 2, "sessions.warm", frame).await;
        let bare = WarmAsk { harness: Some("claude".into()), model: Some("opus".into()), fast: Some(true), ..Default::default() };
        assert_eq!(args_of(rt, "sessions.warm", 1)["ask"], json!(format!("{bare:?}")), "no cwd from the folder, none from the frame");
    })
}

// O6. serve.test.ts:508
#[test]
fn harnesses_list_passes_the_catalog_place_only_when_named() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        r.rt.script.once::<Result<Vec<wsp_wire::views::harness::HarnessCatalog>, HostError>>("harnesses.list", Ok(vec![]));
        r.rt.script.once::<Result<Vec<wsp_wire::views::harness::HarnessCatalog>, HostError>>("harnesses.list", Ok(vec![]));
        let plain = request(&host, &sink, 1, "harnesses.list", json!({ "workspaceId": "ws_1" })).await;
        assert_eq!(plain.to_string(), r#"{"id":1,"ok":true,"harnesses":[]}"#);
        request(&host, &sink, 2, "harnesses.list", json!({ "project": "app", "computer": "pl_1" })).await;
        let first = args_of(&r.rt, "harnesses.list", 0);
        assert_eq!(first["workspaceId"], json!("ws_1"));
        assert_eq!(first["at"], json!(format!("{:?}", CatalogAt::default())), "nothing named, nothing passed");
        let second = args_of(&r.rt, "harnesses.list", 1);
        assert_eq!(second["workspaceId"], Value::Null);
        assert_eq!(second["at"], json!(format!("{:?}", CatalogAt { project: Some("app".into()), computer: Some("pl_1".into()) })));
    })
}

// O7. serve.test.ts:388
#[test]
fn history_with_a_thread_reads_a_page() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        let page = json!({ "events": [], "pos": 3, "total": 3 });
        r.rt.script.once::<Result<wsp_wire::views::preferences::HistoryPage, HostError>>("sessions.page", Ok(from(page.clone())));
        r.rt.script.once::<Result<wsp_wire::views::preferences::HistoryPage, HostError>>("sessions.page", Ok(from(page)));
        let reply =
            request(&host, &sink, 1, "sessions.history", json!({ "workspaceId": "ws_1", "threadId": "t1", "before": 3, "limit": 2 })).await;
        assert_eq!(reply.to_string(), r#"{"id":1,"ok":true,"events":[],"pos":3,"total":3}"#, "the page is spread beside id and ok");
        request(&host, &sink, 2, "sessions.history", json!({ "workspaceId": "ws_1", "threadId": "t1" })).await;
        let window = |w: PageWindow| json!(format!("{w:?}"));
        assert_eq!(
            args_of(&r.rt, "sessions.page", 0)["window"],
            window(PageWindow { thread_id: "t1".into(), before: Some(3), limit: Some(2) })
        );
        assert_eq!(args_of(&r.rt, "sessions.page", 1)["window"], window(PageWindow { thread_id: "t1".into(), before: None, limit: None }));
        assert!(r.rt.script.calls_to("sessions.history").is_empty() && r.rt.script.calls_to("sessions.threadHistory").is_empty());
    })
}

// O7. serve.test.ts:388
#[test]
fn history_with_a_window_and_no_thread_is_usage() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        let reply = request(&host, &sink, 1, "sessions.history", json!({ "workspaceId": "ws_1", "limit": 3 })).await;
        let expected = json!({
            "id": 1, "ok": false, "error": "before and limit page one thread's events. Name the thread with threadId.",
            "kind": "usage", "fix": "Name the thread with threadId.",
        });
        assert_eq!(reply.to_string(), expected.to_string());
        let before = request(&host, &sink, 2, "sessions.history", json!({ "workspaceId": "ws_1", "before": 3 })).await;
        assert_eq!(before["kind"], json!("usage"));
        assert!(r.rt.script.names().iter().all(|n| !n.starts_with("sessions.")), "nothing is read for the refused window");
        assert_eq!(r.failed_ops(), vec!["sessions.history", "sessions.history"]);
    })
}

// O7. serve.test.ts:426
#[test]
fn history_whole_reads_thread_history() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        let events = || Ok(Vec::<wsp_wire::views::session_events_more::SessionEvent>::new());
        r.rt.script.once::<Result<Vec<wsp_wire::views::session_events_more::SessionEvent>, HostError>>("sessions.threadHistory", events());
        r.rt.script.once::<Result<Vec<wsp_wire::views::session_events_more::SessionEvent>, HostError>>("sessions.threadHistory", events());
        let one = request(&host, &sink, 1, "sessions.history", json!({ "workspaceId": "ws_1", "threadId": "t1", "whole": true })).await;
        assert_eq!(one.to_string(), r#"{"id":1,"ok":true,"events":[]}"#);
        request(&host, &sink, 2, "sessions.history", json!({ "workspaceId": "ws_1", "whole": true, "before": 4 })).await;
        let asked = |a: HistoryAsk| json!(format!("{a:?}"));
        assert_eq!(
            args_of(&r.rt, "sessions.threadHistory", 0)["asked"],
            asked(HistoryAsk { thread_id: Some("t1".into()), before: None, limit: None })
        );
        assert_eq!(
            args_of(&r.rt, "sessions.threadHistory", 1)["asked"],
            asked(HistoryAsk { thread_id: None, before: Some(4), limit: None }),
            "the area, not the switch, refuses whole with a window and no thread"
        );
        assert!(r.rt.script.calls_to("sessions.page").is_empty() && r.rt.script.calls_to("sessions.history").is_empty());
    })
}

// O7. serve.test.ts:374
#[test]
fn history_plain_reads_the_folder() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        r.rt.script.once::<Result<Vec<wsp_wire::views::session_events_more::SessionEvent>, HostError>>("sessions.history", Ok(vec![]));
        let reply = request(&host, &sink, 1, "sessions.history", json!({ "workspaceId": "ws_1" })).await;
        assert_eq!(reply.to_string(), r#"{"id":1,"ok":true,"events":[]}"#);
        assert_eq!(args_of(&r.rt, "sessions.history", 0)["workspaceId"], json!("ws_1"));
        let missing = request(&host, &sink, 2, "sessions.history", json!({ "workspaceId": "ws_nope" })).await;
        assert_eq!(missing["ok"], json!(false), "an unknown workspace is the area's refusal");
    })
}

// O8. serve.test.ts:661
#[test]
fn session_ops_pass_only_the_fields_sent() {
    block(async {
        let r = rig(|_| {});
        let rt = &r.rt;
        let (host, sink) = r.host().await;
        let stop = || HostError::new("stop");

        rt.script.once::<Result<Vec<wsp_wire::views::session::SessionView>, HostError>>("sessions.list", Ok(vec![]));
        let listed = request(&host, &sink, 1, "sessions.list", json!({})).await;
        assert_eq!(listed.to_string(), r#"{"id":1,"ok":true,"sessions":[]}"#);
        assert_eq!(args_of(rt, "sessions.list", 0)["workspaceId"], Value::Null);

        rt.script.once::<Result<wsp_wire::wire::runtime::SessionInterruptResult, HostError>>(
            "sessions.interrupt",
            Ok(from(json!({ "outcome": "accepted" }))),
        );
        let interrupted = request(&host, &sink, 2, "sessions.interrupt", json!({ "sessionId": "s1", "task": "build" })).await;
        assert_eq!(interrupted, json!({ "id": 2, "ok": true, "outcome": "accepted" }));
        let a = args_of(rt, "sessions.interrupt", 0);
        assert_eq!((a["sessionId"].clone(), a["task"].clone()), (json!("s1"), json!("build")));

        rt.script.once::<Result<wsp_wire::wire::runtime::SessionAnswerResult, HostError>>(
            "sessions.answer",
            Ok(from(json!({ "outcome": "answered" }))),
        );
        let answered = request(&host, &sink, 3, "sessions.answer", json!({ "sessionId": "s1", "askId": "a1", "optionId": "o1" })).await;
        assert_eq!(answered, json!({ "id": 3, "ok": true, "outcome": "answered" }));
        let plain = AnswerAsk { ask_id: "a1".into(), option_id: "o1".into(), reason: None };
        assert_eq!(args_of(rt, "sessions.answer", 0)["ask"], json!(format!("{plain:?}")), "no reason, none passed");

        rt.script.once::<Result<wsp_wire::wire::runtime::SessionSteerResult, HostError>>("sessions.steer", Err(stop()));
        request(&host, &sink, 4, "sessions.steer", json!({ "sessionId": "s1", "prompt": "and this" })).await;
        let steer = SteerAsk { prompt: "and this".into(), request_id: None, attachments: None };
        assert_eq!(args_of(rt, "sessions.steer", 0)["ask"], json!(format!("{steer:?}")));

        rt.script.once::<Result<wsp_wire::wire::runtime::SessionRewindResult, HostError>>("sessions.rewind", Err(stop()));
        request(&host, &sink, 5, "sessions.rewind", json!({ "threadId": "t1", "turnId": "tu_1", "files": false })).await;
        let rewind = RewindAsk { turn_id: Some("tu_1".into()), files: Some(false), undo: None };
        assert_eq!(args_of(rt, "sessions.rewind", 0)["ask"], json!(format!("{rewind:?}")), "a false files is sent, an absent undo is not");

        rt.script.once::<Result<(), HostError>>("sessions.forget", Ok(()));
        rt.script.once::<Result<(), HostError>>("sessions.forget", Ok(()));
        let forgot = request(&host, &sink, 6, "sessions.forget", json!({ "threadId": "t1", "check": false })).await;
        assert_eq!(forgot, json!({ "id": 6, "ok": true }));
        request(&host, &sink, 7, "sessions.forget", json!({ "threadId": "t1", "check": true })).await;
        assert_eq!(args_of(rt, "sessions.forget", 0)["check"], Value::Null, "check rides only when true");
        assert_eq!(args_of(rt, "sessions.forget", 1)["check"], json!(true));

        rt.script.once::<Result<(), HostError>>("sessions.read", Ok(()));
        rt.script.once::<Result<(), HostError>>("sessions.replaceable", Ok(()));
        rt.script.once::<Result<(), HostError>>("sessions.mark", Ok(()));
        for (id, op, fields) in [
            (8, "sessions.read", json!({ "threadId": "t1" })),
            (9, "sessions.replaceable", json!({ "threadId": "t1" })),
            (10, "sessions.mark", json!({ "threadIds": ["t1", "t2"], "marks": { "resumeAtReset": true } })),
        ] {
            assert_eq!(request(&host, &sink, id, op, fields).await, json!({ "id": id, "ok": true }), "{op}");
        }
        let mark = args_of(rt, "sessions.mark", 0);
        assert_eq!((mark["threadIds"].clone(), mark["marks"].clone()), (json!(["t1", "t2"]), json!({ "resumeAtReset": true })));

        rt.script.once::<Result<wsp_wire::views::preferences::ThreadHead, HostError>>("sessions.head", Err(stop()));
        rt.script.once::<Result<wsp_wire::views::preferences::ThreadHead, HostError>>("sessions.head", Err(stop()));
        request(&host, &sink, 11, "sessions.head", json!({ "threadId": "t1" })).await;
        request(&host, &sink, 12, "sessions.head", json!({ "threadId": "t1", "afterRead": true })).await;
        assert_eq!(args_of(rt, "sessions.head", 0)["afterRead"], Value::Null);
        assert_eq!(args_of(rt, "sessions.head", 1)["afterRead"], json!(true));

        let ran = json!({ "workspaceId": "ws_1", "sessionId": "s1", "runId": "r1", "block": "b1", "command": "ls", "state": "running" });
        rt.script.once::<Result<wsp_wire::views::session_events_more::SessionRunEvent, HostError>>("sessions.run", Ok(from(ran.clone())));
        let run = request(
            &host,
            &sink,
            13,
            "sessions.run",
            json!({ "runId": "r1", "block": "b1", "command": "ls", "state": "running", "threadId": "t1", "turnId": "tu_1" }),
        )
        .await;
        assert_eq!(keys(&run), vec!["id", "ok", "run"]);
        assert_eq!(run["run"]["runId"], json!("r1"));
        let step = args_of(rt, "sessions.run", 0)["step"].clone();
        assert_eq!((step["runId"].clone(), step["threadId"].clone(), step["turnId"].clone()), (json!("r1"), json!("t1"), json!("tu_1")));
        assert!(step.get("id").is_none() && step.get("op").is_none(), "the step is the frame less id and op");
    })
}

// O8. serve.test.ts:661
#[test]
fn interrupting_twice_reaches_the_runtime_twice() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        for outcome in ["accepted", "not-running"] {
            r.rt.script.once::<Result<wsp_wire::wire::runtime::SessionInterruptResult, HostError>>(
                "sessions.interrupt",
                Ok(from(json!({ "outcome": outcome }))),
            );
        }
        let first = request(&host, &sink, 1, "sessions.interrupt", json!({ "sessionId": "s1" })).await;
        let second = request(&host, &sink, 2, "sessions.interrupt", json!({ "sessionId": "s1" })).await;
        assert_eq!(first, json!({ "id": 1, "ok": true, "outcome": "accepted" }));
        assert_eq!(second, json!({ "id": 2, "ok": true, "outcome": "not-running" }));
        assert_eq!(r.rt.script.calls_to("sessions.interrupt").len(), 2, "the switch keeps no memory of the first");
        assert_eq!(args_of(&r.rt, "sessions.interrupt", 0), args_of(&r.rt, "sessions.interrupt", 1));
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Images, usage and forwards.

// O9. serve.test.ts:362
#[test]
fn capabilities_are_the_backends_flags() {
    block(async {
        let r = rig(|_| {});
        *r.rt.capabilities.borrow_mut() = json!({ "images": true, "previewUrls": true, "copies": true });
        let (host, sink) = r.host().await;
        let reply = request(&host, &sink, 1, "capabilities.get", json!({})).await;
        assert_eq!(reply["ok"], json!(true));
        let c = &reply["capabilities"];
        assert_eq!(
            (c["images"].clone(), c["previewUrls"].clone(), c["copies"].clone(), c["liveCloneForks"].clone()),
            (json!(true), json!(true), json!(true), json!(false))
        );
        assert_eq!(c["sizes"], json!([]));
        assert!(r.failed.borrow().is_empty());
    })
}

fn version(n: u32) -> Value {
    json!({
        "version": n, "snapshotId": format!("snap_golden-v{n}"), "baseTemplate": "base", "setupSha": format!("sha{n}"),
        "createdAt": format!("2026-08-{}T00:00:00.000Z", 10 + n), "smoke": { "cmd": "true", "exitCode": 0 },
    })
}

fn manifest(head: u32) -> wsp_wire::views::golden_image::GoldenManifest {
    from(json!({ "head": head, "versions": [version(1), version(2)] }))
}

// O10. serve.test.ts:834
#[test]
fn snapshots_list_names_default_and_null_head() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        let m = manifest(2);
        let shown = serde_json::to_value(&m).unwrap();
        r.rt.script.once::<Result<Option<wsp_wire::views::golden_image::GoldenManifest>, HostError>>("golden.get", Ok(Some(m)));
        let listed = request(&host, &sink, 1, "snapshots.list", json!({})).await;
        assert_eq!(listed["lineage"], json!({ "name": "default", "head": 2, "versions": shown["versions"] }));
        assert_eq!(keys(&listed["lineage"]), vec!["name", "head", "versions"]);
        assert_eq!(args_of(&r.rt, "golden.get", 0)["name"], json!("default"));

        r.rt.script.once::<Result<Option<wsp_wire::views::golden_image::GoldenManifest>, HostError>>("golden.get", Ok(None));
        let empty = request(&host, &sink, 2, "snapshots.list", json!({ "name": "other" })).await;
        assert_eq!(empty.to_string(), r#"{"id":2,"ok":true,"lineage":{"name":"other","head":null,"versions":[]}}"#);
        assert_eq!(args_of(&r.rt, "golden.get", 1)["name"], json!("other"));
    })
}

// O10. serve.test.ts:834, 855
#[test]
fn snapshots_rollback_says_untouched() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        let m = manifest(1);
        let shown = serde_json::to_value(&m).unwrap();
        r.rt.script.once::<Result<wsp_wire::views::golden_image::GoldenManifest, HostError>>("golden.rollback", Ok(m));
        let rolled = request(&host, &sink, 1, "snapshots.rollback", json!({ "version": 1 })).await;
        assert_eq!(keys(&rolled), vec!["id", "ok", "lineage", "existingWorkspaces"]);
        assert_eq!(rolled["lineage"], json!({ "name": "default", "head": 1, "versions": shown["versions"] }));
        assert_eq!(rolled["existingWorkspaces"], json!("untouched"));
        let a = args_of(&r.rt, "golden.rollback", 0);
        assert_eq!((a["version"].clone(), a["name"].clone()), (json!(1), json!("default")));

        let e = HostError::with_kind("snapshot v9 is not in the manifest", Kind::Missing);
        r.rt.script.once::<Result<wsp_wire::views::golden_image::GoldenManifest, HostError>>("golden.rollback", Err(e.clone()));
        let refused = request(&host, &sink, 2, "snapshots.rollback", json!({ "version": 9 })).await;
        assert_eq!(refused.to_string(), json!({ "id": 2, "ok": false, "error": e.message, "kind": "missing" }).to_string());
        assert!(r.rt.script.calls_to("golden.get").is_empty(), "nothing else is read for a refused rollback");
    })
}

// O10. serve.test.ts:855
#[test]
fn snapshots_storage_null_when_none() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        r.rt.script.once::<Result<Option<wsp_wire::views::snapshot_lineage::SnapshotStorage>, HostError>>("golden.storage", Ok(None));
        let reply = request(&host, &sink, 1, "snapshots.storage", json!({})).await;
        assert_eq!(reply.to_string(), r#"{"id":1,"ok":true,"storage":null}"#);
    })
}

// O9. serve.test.ts:735
#[test]
fn golden_and_image_ops_pass_only_the_fields_sent() {
    block(async {
        let r = rig(|_| {});
        let rt = &r.rt;
        let (host, sink) = r.host().await;
        rt.script.once::<Result<Option<wsp_wire::views::golden_image::GoldenManifest>, HostError>>("golden.get", Ok(None));
        let got = request(&host, &sink, 1, "golden.get", json!({ "name": "default" })).await;
        assert_eq!(got.to_string(), r#"{"id":1,"ok":true}"#);

        let stop = || HostError::new("stop");
        rt.script.once::<Result<wsp_wire::views::golden_image::GoldenBuilderView, HostError>>("golden.prepare", Err(stop()));
        rt.script.once::<Result<wsp_wire::views::golden_image::GoldenBuilderView, HostError>>("golden.prepare", Err(stop()));
        request(&host, &sink, 2, "golden.prepare", json!({ "name": "default" })).await;
        request(&host, &sink, 3, "golden.prepare", json!({ "name": "default", "kind": "desktop" })).await;
        assert_eq!(args_of(rt, "golden.prepare", 0)["ask"]["kind"], Value::Null, "no kind named, none passed");
        assert_eq!(args_of(rt, "golden.prepare", 1)["ask"]["kind"], json!("desktop"));
        assert_eq!(args_of(rt, "golden.prepare", 1)["ask"]["name"], json!("default"));

        rt.script.once::<Result<wsp_core::types::api::Sealed, HostError>>("golden.seal", Err(stop()));
        request(&host, &sink, 4, "golden.seal", json!({ "builderId": "b_1" })).await;
        assert_eq!(args_of(rt, "golden.seal", 0)["builderId"], json!("b_1"));
        assert_eq!(args_of(rt, "golden.seal", 0)["logins"], Value::Null);

        rt.script.once::<Result<wsp_wire::views::snapshot_lineage::SealedImageView, HostError>>("image.get", Err(stop()));
        request(&host, &sink, 5, "image.get", json!({})).await;
        assert_eq!(args_of(rt, "image.get", 0)["name"], Value::Null);

        rt.script.once::<Result<wsp_wire::views::golden_image::SealedImageBuilt, HostError>>("image.build", Err(stop()));
        rt.script.once::<Result<wsp_wire::views::golden_image::SealedImageBuilt, HostError>>("image.build", Err(stop()));
        request(&host, &sink, 6, "image.build", json!({ "place": "pl_1" })).await;
        request(&host, &sink, 7, "image.build", json!({ "place": "pl_1", "name": "mine", "force": false })).await;
        assert_eq!(args_of(rt, "image.build", 0)["ask"], json!({ "place": "pl_1", "name": null, "force": null }));
        assert_eq!(
            args_of(rt, "image.build", 1)["ask"],
            json!({ "place": "pl_1", "name": "mine", "force": false }),
            "a false force is sent"
        );
    })
}

fn spend_row() -> PlaceSpend {
    from(json!({ "place": "box", "todayUsd": 1.5, "monthUsd": 10, "rateUsdPerHour": 0.1 }))
}

// O11. serve-refusal.test.ts:164
#[test]
fn cost_spend_for_a_thread_reads_its_places() {
    block(async {
        let rt = FakeRuntime::with_places();
        let r = rig_over(rt.clone(), |o| o.now = Some(Rc::new(|| 4242)));
        rt.script.once::<Result<WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_1", "box1", "cloud", "running", json!({}))));
        rt.script.once::<Result<Vec<PlaceView>, HostError>>(
            "places.list",
            Ok(vec![place("pl_1", "laptop", "computer"), place("box", "box", "provider")]),
        );
        rt.script.once::<Result<Vec<PlaceSpend>, String>>("status.spend", Ok(vec![spend_row()]));
        let (thread, sink) = r.thread("ws_1").await;
        let reply = request(&thread, &sink, 1, "cost.spend", json!({})).await;
        assert_eq!(keys(&reply), vec!["id", "ok", "places"]);
        assert_eq!(reply["places"], json!([serde_json::to_value(spend_row()).unwrap()]));
        let asked = args_of(&rt, "status.spend", 0)["places"].clone();
        assert_eq!(
            asked,
            json!([{ "id": "box", "kind": "provider", "name": "box", "label": said("box", "provider"), "default": false }]),
            "a machine's thread reads the clouds alone"
        );
        assert_eq!(args_of(&rt, "places.list", 0)["now"], json!(4242));
        assert_eq!(args_of(&rt, "workspaces.get", 0)["id"], json!("ws_1"));
        assert!(r.failed.borrow().is_empty());
    })
}

// O11. serve-refusal.test.ts:164
#[test]
fn cost_spend_is_the_own_road_and_reads_the_places_it_holds() {
    block(async {
        let rt = FakeRuntime::with_places();
        let r = rig_over(rt.clone(), |_| {});
        let (ticketed, ticket_sink) = r.ticketed().await;
        let refused = request(&ticketed, &ticket_sink, 1, "cost.spend", json!({})).await;
        assert_eq!(refused, ticket_reply(1));
        assert!(rt.script.calls_to("status.spend").is_empty() && rt.script.calls_to("places.list").is_empty());

        let rows = vec![place("pl_1", "laptop", "computer")];
        rt.script.once::<Result<Vec<PlaceView>, HostError>>("places.list", Ok(rows.clone()));
        rt.script.once::<Result<Vec<PlaceSpend>, String>>("status.spend", Ok(vec![]));
        let (host, sink) = r.host().await;
        let reply = request(&host, &sink, 2, "cost.spend", json!({})).await;
        assert_eq!(reply.to_string(), r#"{"id":2,"ok":true,"places":[]}"#);
        assert_eq!(
            args_of(&rt, "status.spend", 0)["places"],
            json!([serde_json::to_value(&rows[0]).unwrap()]),
            "the rows as the list holds them, road and all"
        );

        let bare = rig(|_| {});
        bare.rt.script.once::<Result<Vec<PlaceSpend>, String>>("status.spend", Ok(vec![]));
        let (bare_host, bare_sink) = bare.host().await;
        request(&bare_host, &bare_sink, 1, "cost.spend", json!({})).await;
        assert_eq!(args_of(&bare.rt, "status.spend", 0)["places"], json!([]), "a runtime with no places spends over none");
    })
}

// O11. serve.test.ts
#[test]
fn cost_history_passes_the_origin() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        r.rt.script.once::<Result<Vec<wsp_core::status::CostPoint>, HostError>>("status.history", Ok(vec![]));
        let reply = request(&host, &sink, 1, "cost.history", json!({ "workspaceId": "ws_1", "origin": "here" })).await;
        assert_eq!(reply.to_string(), r#"{"id":1,"ok":true,"points":[]}"#);
        let a = args_of(&r.rt, "status.history", 0);
        assert_eq!((a["workspaceId"].clone(), a["origin"].clone()), (json!("ws_1"), json!("here")));
    })
}

// O12. serve-refusal.test.ts:164
#[test]
fn usage_ops_are_the_own_road() {
    block(async {
        let r = rig(|_| {});
        let rt = &r.rt;
        let (ticketed, ticket_sink) = r.ticketed().await;
        let asks: Vec<(&str, Value)> = vec![
            ("usage.used", json!({ "range": "week", "split": "agent" })),
            ("usage.accounts", json!({})),
            ("usage.reset", json!({ "account": "a1" })),
            ("places.readings", json!({ "placeId": "pl_1", "range": "day" })),
        ];
        for (i, (op, fields)) in asks.iter().enumerate() {
            let id = (i + 1) as u64;
            assert_eq!(request(&ticketed, &ticket_sink, id, op, fields.clone()).await, ticket_reply(id), "{op}");
        }
        assert!(rt.script.names().iter().all(|n| !n.starts_with("usage.")), "a socket off the own road reads no usage");

        let (host, sink) = r.host().await;
        let used = json!({ "range": "week", "split": "agent", "rows": [], "series": [], "since": 1, "until": 2 });
        rt.script.once::<Result<wsp_wire::types::usage::UsedAnswer, HostError>>("usage.used", Ok(from(used)));
        rt.script.once::<Result<wsp_wire::types::usage::UsedAnswer, HostError>>(
            "usage.used",
            Ok(from(json!({ "range": "day", "split": "model", "rows": [], "series": [], "since": 1, "until": 2 }))),
        );
        let reply = request(&host, &sink, 1, "usage.used", json!({ "range": "week", "split": "agent" })).await;
        assert_eq!(keys(&reply), vec!["id", "ok", "used"]);
        assert_eq!(reply["used"]["range"], json!("week"));
        let a = args_of(rt, "usage.used", 0);
        assert_eq!((a["range"].clone(), a["split"].clone(), a["outside"].clone()), (json!("week"), json!("agent"), Value::Null));
        request(&host, &sink, 2, "usage.used", json!({ "range": "day", "split": "model", "outside": false })).await;
        assert_eq!(args_of(rt, "usage.used", 1)["outside"], json!(false), "a false outside is sent");

        rt.script.once::<Result<wsp_wire::types::usage::AccountsAnswer, HostError>>("usage.accounts", Ok(from(json!({ "accounts": [] }))));
        rt.script.once::<Result<wsp_wire::types::usage::AccountsAnswer, HostError>>("usage.accounts", Ok(from(json!({ "accounts": [] }))));
        let accounts = request(&host, &sink, 3, "usage.accounts", json!({ "fresh": false })).await;
        assert_eq!(accounts.to_string(), r#"{"id":3,"ok":true,"accounts":[]}"#);
        request(&host, &sink, 4, "usage.accounts", json!({ "fresh": true })).await;
        assert_eq!(args_of(rt, "usage.accounts", 0)["fresh"], Value::Null, "fresh rides only when true");
        assert_eq!(args_of(rt, "usage.accounts", 1)["fresh"], json!(true));

        rt.script.once::<Result<wsp_wire::types::usage::ResetAnswer, HostError>>(
            "usage.reset",
            Ok(from(json!({ "outcome": "reset", "said": "reset it" }))),
        );
        let reset = request(&host, &sink, 5, "usage.reset", json!({ "account": "a1", "creditId": "c1" })).await;
        assert_eq!(reset.to_string(), r#"{"id":5,"ok":true,"outcome":"reset","said":"reset it"}"#);
        let a = args_of(rt, "usage.reset", 0);
        assert_eq!((a["account"].clone(), a["creditId"].clone(), a["on"].clone()), (json!("a1"), json!("c1"), Value::Null));
    })
}

// O13. serve.ts:1649
#[test]
fn readings_name_a_place_or_a_workspace() {
    block(async {
        let r = rig(|_| {});
        let (host, sink) = r.host().await;
        let neither = request(&host, &sink, 1, "places.readings", json!({ "range": "day" })).await;
        assert_eq!(neither.to_string(), r#"{"id":1,"ok":false,"error":"places.readings names a placeId or a workspaceId","kind":"usage"}"#);
        assert!(r.rt.script.calls_to("usage.readings").is_empty());
        assert!(r.failed.borrow().is_empty(), "the switch sent it");

        let answer = json!({ "points": [], "stepMs": 1000, "from": 1, "to": 2 });
        r.rt.script.once::<Result<wsp_wire::wire::machine_link::ReadingsAnswer, HostError>>("usage.readings", Ok(from(answer.clone())));
        r.rt.script.once::<Result<wsp_wire::wire::machine_link::ReadingsAnswer, HostError>>("usage.readings", Ok(from(answer)));
        let both = request(&host, &sink, 2, "places.readings", json!({ "placeId": "pl_1", "workspaceId": "ws_1", "range": "day" })).await;
        assert_eq!(both.to_string(), r#"{"id":2,"ok":true,"points":[],"stepMs":1000,"from":1,"to":2}"#);
        assert_eq!(args_of(&r.rt, "usage.readings", 0)["target"], json!("Workspace(\"ws_1\")"), "a workspaceId wins over a placeId");
        request(&host, &sink, 3, "places.readings", json!({ "placeId": "pl_1", "range": "week" })).await;
        assert_eq!(args_of(&r.rt, "usage.readings", 1)["target"], json!("Place(\"pl_1\")"));
        assert_eq!(args_of(&r.rt, "usage.readings", 1)["range"], json!("week"));
    })
}
