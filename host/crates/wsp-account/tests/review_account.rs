// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the account half of the crate that holds the channels' pty wrappers, the box panels and the process scopes.
//! Every expected value is the TS's: node probes `oracle/zz-review-account-ptys.oracle.ts`,
//! `zz-review-account-scopes.oracle.ts` and `zz-review-account-panels.oracle.ts` (text and output are in the review
//! report), or the TS line named at the test. Over the same world as tests/channels_ptys.rs.
#![allow(dead_code, unused_imports)]
mod common;
include!("channels_ptys/world.rs");

use wsp_account::account::box_panels::{BoxPanels, BoxPanelsOpener, Placements, Watchers};
use wsp_account::account::box_panels::proc_watch_scope;
use wsp_account::account::core_types::Served;

/// What a daemon heard as `{ op, roots? }`, the shape the node probes print.
fn ops_roots(d: &Daemon) -> String {
    let rows: Vec<Value> = d
        .heard()
        .iter()
        .map(|f| {
            let mut row = serde_json::Map::new();
            row.insert("op".into(), f["op"].clone());
            if let Some(roots) = f.get("roots") {
                row.insert("roots".into(), roots.clone());
            }
            Value::Object(row)
        })
        .collect();
    serde_json::to_string(&rows).unwrap()
}

/// `{ op, scopes: [thread...] }` for each frame, the shape the node probe prints.
fn ops_scopes(d: &Daemon) -> String {
    let rows: Vec<Value> = d
        .heard()
        .iter()
        .map(|f| {
            let mut row = serde_json::Map::new();
            row.insert("op".into(), f["op"].clone());
            if let Some(scopes) = f.get("scopes").and_then(Value::as_array) {
                row.insert("scopes".into(), Value::Array(scopes.iter().map(|s| s["thread"].clone()).collect()));
            }
            Value::Object(row)
        })
        .collect();
    serde_json::to_string(&rows).unwrap()
}

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-A1. proc-scopes.ts:178-188 (`tell` and `push`). The TS tells a watching channel the new scopes inside the move:
// `push` runs `tell()` to its first await, which sends the frame at once, and a `proc.unwatch` that comes after finds the
// frame gone and sets `told` back to undefined. proc_scopes.rs `scoped_procs` queues the tell with `ctx.spawn`, so an
// unwatch that comes before the queue runs sets `told` to None, then the queued tell sets it to `Some` again and sends
// proc.scopes after the proc.unwatch: from then on every move is told to a channel that no longer watches.
// Node (zz-review-account-scopes.oracle.ts): proc.scopes[t1], proc.watch, proc.scopes[t1,t2], proc.unwatch, and nothing for
// the move after the unwatch.

#[test]
fn a_move_just_before_an_unwatch_is_told_before_it_and_no_move_after_it_is() {
    run(async {
        let (w, area, scopes) = both_areas();
        w.groups.borrow_mut().insert(5000);
        set_groups(&area, "ws_a", &[(5000, "t1")]);
        let wire = Daemon::ok();
        let of = scopes.clone();
        let channel = scopes.scoped_procs(Rc::new(move || of.scopes_here()), wire.clone());
        send(&channel, json!({ "id": 1, "op": "proc.watch" })).await;
        area.turn_groups.borrow_mut().get_mut("ws_a").unwrap().insert(5001, "t2".into());
        w.groups.borrow_mut().insert(5001);
        scopes.proc_scopes_moved();
        send(&channel, json!({ "id": 2, "op": "proc.unwatch" })).await;
        settle().await;
        area.turn_groups.borrow_mut().get_mut("ws_a").unwrap().insert(5002, "t3".into());
        w.groups.borrow_mut().insert(5002);
        scopes.proc_scopes_moved();
        settle().await;
        assert_eq!(
            ops_scopes(&wire),
            r#"[{"op":"proc.scopes","scopes":["t1"]},{"op":"proc.watch"},{"op":"proc.scopes","scopes":["t1","t2"]},{"op":"proc.unwatch"}]"#
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-A2. channels.ts:538-542 (`push`). A moved root is named to the daemon with `void channel.send(...)`, so the frame
// leaves inside the move. ptys.rs `rooted_ports` computes the roots inside the move and sends them from a task it queues
// with `ctx.spawn`. Node (zz-review-account-ptys.oracle.ts, "heard right after the create answered"): the daemon has
// heard ports.watch [], pty.create, ports.watch [4242] by the time the pty.create answer is back.

#[test]
fn a_new_shells_pid_is_named_to_the_ports_watch_before_its_create_is_answered() {
    run(async {
        let w = World::new();
        let area = w.area();
        let inner = Daemon::new(|fr| match fr["op"].as_str().unwrap() {
            "pty.create" => frame(json!({ "id": null, "ok": true, "ptyId": "p1", "pid": 4242 })),
            _ => frame(json!({ "id": null, "ok": true, "ports": [] })),
        });
        set_ptys(&area, "ws", &[]);
        let c = area.rooted_ports("ws", "/work/ws", inner.clone());
        send(&c, json!({ "id": 1, "op": "ports.watch" })).await;
        send(&c, json!({ "id": 2, "op": "pty.create" })).await;
        assert_eq!(ops_roots(&inner), r#"[{"op":"ports.watch","roots":[]},{"op":"pty.create"},{"op":"ports.watch","roots":[4242]}]"#);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-A3. The same queueing, with a client frame that comes next. A turn's group (5000) is alive when a shell opens,
// and gone when the client names its roots again. The TS sent the push at the create, so the last frame the daemon holds is
// the client's, [4242]. The Rust sends the client's frame first and the queued push [4242,5000] after it, and `told` says
// [4242]: the daemon keeps watching a group that no longer exists, and nothing sends it again (the recheck compares with
// `told`), which is the stranger's-group case the 5 s recheck exists to prevent. Node ("frames in order"):
// ports.watch [5000], pty.create, ports.watch [4242,5000], ports.watch [4242].

#[test]
fn the_last_ports_watch_the_daemon_holds_is_the_clients_latest_roots() {
    run(async {
        let w = World::new();
        let area = w.area();
        w.groups.borrow_mut().insert(5000);
        let inner = Daemon::new(|fr| match fr["op"].as_str().unwrap() {
            "pty.create" => frame(json!({ "id": null, "ok": true, "ptyId": "p1", "pid": 4242 })),
            _ => frame(json!({ "id": null, "ok": true, "ports": [] })),
        });
        set_ptys(&area, "ws", &[]);
        set_groups(&area, "ws", &[(5000, "t1")]);
        let c = area.rooted_ports("ws", "/work/ws", inner.clone());
        send(&c, json!({ "id": 1, "op": "ports.watch" })).await;
        send(&c, json!({ "id": 2, "op": "pty.create" })).await;
        w.groups.borrow_mut().remove(&5000);
        send(&c, json!({ "id": 3, "op": "ports.watch" })).await;
        settle().await;
        assert_eq!(
            ops_roots(&inner),
            r#"[{"op":"ports.watch","roots":[5000]},{"op":"pty.create"},{"op":"ports.watch","roots":[4242,5000]},{"op":"ports.watch","roots":[4242]}]"#
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-A4. channels.ts:382-394 (`tagsOf`). The TS registers `void read.then(tags => tagsRead.set(...))` before it hands
// the read to its caller, so by the time the first `await tagsOf(ws)` of a workspace resumes, `threadTerminals()` reads
// that workspace's tags. ptys.rs `tags_of` sets `tags_read` from a task it queues with `ctx.spawn`, and the caller resumes
// first. A `pty.thread` that is the first frame a workspace's channel sees (a host that restarted over shells a daemon
// still holds) moves the roots at once, the scopes are read at that moment, and the shell is named to no thread.
// Node ("frames the standing channel heard"): one proc.scopes frame naming th_a with pid 4242 as a terminal.

struct Standing {
    w: Rc<World>,
    area: RefCell<Option<Rc<ChannelsArea>>>,
    daemon: Rc<Daemon>,
}

#[async_trait(?Send)]
impl ProcScopesCtx for Standing {
    fn turn_groups(&self) -> Vec<(String, Vec<(i64, String)>)> {
        let area = self.area.borrow().clone().unwrap();
        let groups =
            area.turn_groups.borrow().iter().map(|(ws, g)| (ws.clone(), g.iter().map(|(p, t)| (*p, t.clone())).collect())).collect();
        groups
    }
    fn group_exists(&self, pgid: i64) -> bool {
        self.w.groups.borrow().contains(&pgid)
    }
    fn sessions(&self) -> Vec<ScopeRow> {
        vec![]
    }
    fn kept_agents(&self) -> Vec<(String, KeptRow)> {
        vec![]
    }
    fn latest_on(&self, _thread_id: &str) -> Option<LatestRow> {
        None
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        vec![]
    }
    fn thread_terminals(&self) -> Vec<ThreadTerminal> {
        self.area.borrow().clone().unwrap().thread_terminals()
    }
    fn live_record(&self, workspace_id: &str) -> Option<ScopeRecord> {
        (workspace_id == "ws_a").then(|| ScopeRecord { kind: WorkspaceKind::Local, place: None, checkout: "/work/ws_a".into() })
    }
    fn shared_daemon(&self, _kind: WorkspaceKind) -> bool {
        true
    }
    async fn channel_here(&self) -> AccountResult<Rc<dyn DaemonChannel>> {
        Ok(self.daemon.clone())
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        tokio::task::spawn_local(f);
    }
}

#[test]
fn a_shell_handed_over_as_a_workspaces_first_frame_is_named_to_its_thread() {
    run(async {
        let w = World::new();
        let area = w.area();
        let standing = Daemon::ok();
        let ctx = Rc::new(Standing { w: w.clone(), area: RefCell::new(Some(area.clone())), daemon: standing.clone() });
        let scopes = ProcScopesArea::new(ctx);
        *w.forward_moved.borrow_mut() = Some(scopes.clone());
        set_ptys(&area, "ws_a", &[("pty_1", 4242)]);
        let pane = area.rooted_ports("ws_a", "/work/ws_a", area.thread_tagged("ws_a", Daemon::ok()));
        send(&pane, json!({ "id": 1, "op": "pty.thread", "ptyId": "pty_1", "thread": "th_a" })).await;
        settle().await;
        let told: Vec<Value> = standing.heard().into_iter().filter(|f| f["op"] == json!("proc.scopes")).collect();
        assert_eq!(told.len(), 1, "node: the standing channel heard one proc.scopes frame; the Rust heard {}", told.len());
        let named: Vec<ProcScope> = serde_json::from_value(told[0]["scopes"].clone()).unwrap();
        assert_eq!(terminals_of(&named, "th_a"), vec![4242]);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-A5. box-panels.ts:62 `procAsks = new WeakMap<object, ...>`: what the panes of a link asked is held by the link, so
// a link that is gone takes it along. box_panels.rs `with_asks` keeps a `Weak` of every link a pane watched processes over
// in `proc_asks` and never removes the entry, and a `Weak` keeps the link's allocation alive: one entry per link the host
// ever had, for as long as the host runs. The handle below is the link; nothing of it may outlive its panes.

struct Link(Rc<()>);

#[async_trait(?Send)]
impl BoxPanelsCtx for Link {
    fn on_workspace_deleted(&self, _f: Rc<dyn Fn(&str)>) {}
    fn session_rows(&self) -> Vec<StartedRow> {
        vec![]
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        vec![]
    }
    fn link(&self, _place_id: &str) -> Option<Rc<()>> {
        Some(self.0.clone())
    }
    fn name_of(&self, place_id: &str) -> String {
        place_id.to_string()
    }
    async fn daemon_version(&self, _place_id: &str) -> Option<i64> {
        Some(200)
    }
    async fn watch_sys(&self, _place_id: &str) -> Result<Release, String> {
        Err("no".into())
    }
    fn scopes_on(&self, _place_id: &str) -> Vec<ProcScope> {
        vec![]
    }
    fn thread_env(&self, _entry: &Entry) -> IndexMap<String, String> {
        IndexMap::new()
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        tokio::task::spawn_local(f);
    }
}

fn served_ok() -> Served {
    use futures::FutureExt;
    Rc::new(|_f: Frame| async { AccountResult::Ok(frame(json!({ "ok": true }))) }.boxed_local())
}

fn a_folder_entry() -> Entry {
    common::live(common::record(
        &json!({ "id": "w1", "name": "alpha-studio", "kind": "place", "project": "pA2", "machineId": "", "phase": "running", "place": "box1" }),
    ))
}

#[test]
fn a_link_that_every_pane_has_left_is_held_by_nothing_the_panes_left_behind() {
    run(async {
        let link = Rc::new(());
        let opener = BoxPanelsOpener::new(Rc::new(Link(link.clone())), Watchers::default(), Placements::default());
        let pane = opener.open(&a_folder_entry(), "box1", served_ok(), "/srv/alpha");
        pane.send(frame(json!({ "op": "proc.watch" }))).await.unwrap();
        pane.close();
        drop(pane);
        settle().await;
        assert_eq!(Rc::weak_count(&link), 0, "the TS holds a link's asks in a WeakMap; the Rust holds a Weak of the link in `proc_asks`");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-A6. box-panels.ts:32 `[...named].filter(...).sort()`: JavaScript's default sort compares UTF-16 code units, so a
// cgroup name with a character outside the BMP sorts before one with U+E000..U+FFFF. box_panels.rs:44 `kept.sort()` compares
// UTF-8 bytes. A page names these cgroups in its `proc.watch`. Node (zz-review-account-panels.oracle.ts "scope sort"):
// "/a", "/a\u{1F600}", "/a\u{E000}", "/a\u{FF5E}".

#[test]
fn cgroup_names_sort_as_the_ts_sorts_them_by_utf16_code_units() {
    let names: Vec<String> = ["/a\u{1F600}", "/a\u{FF5E}", "/a\u{E000}", "/a"].iter().map(|s| s.to_string()).collect();
    let got = proc_watch_scope(&[Some(names)]).unwrap();
    assert_eq!(got, vec!["/a", "/a\u{1F600}", "/a\u{E000}", "/a\u{FF5E}"]);
}

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-A7. box-panels.ts:203 `frame["roots"].filter(r => Number.isInteger(r))`: every number with no fraction stays,
// 1e300 and 2^64 included. box_panels.rs:273 `integer_of` keeps only those under 9e15 (and the ones serde read as i64), so a
// huge integer is dropped where the TS sends it on. Node (zz-review-account-panels.oracle.ts "roots sent"): [7, 1e+300,
// 18446744073709552000, 9007199254740992, -3, 1, 9007199254740992] for [7, 1e300, 2**64, 2**53, -3, 1.0, "5", true, null,
// [4], 9007199254740993].

#[test]
fn a_ports_watch_names_every_integer_root_the_page_sent() {
    run(async {
        let served = Rc::new(RefCell::new(Vec::<Value>::new()));
        let seen = served.clone();
        let sink: Served = Rc::new(move |f: Frame| {
            use futures::FutureExt;
            seen.borrow_mut().push(Value::Object(f));
            async { AccountResult::Ok(frame(json!({ "ok": true, "ports": [] }))) }.boxed_local()
        });
        let opener = BoxPanelsOpener::new(Rc::new(Link(Rc::new(()))), Watchers::default(), Placements::default());
        let pane = opener.open(&a_folder_entry(), "box1", sink, "/srv/alpha");
        let roots: Value = serde_json::from_str(r#"[7, 1e300, 1.8446744073709552e19, 9007199254740992.0, -3, 1.0, "5", true, null, [4]]"#).unwrap();
        pane.send(frame(json!({ "op": "ports.watch", "roots": roots }))).await.unwrap();
        let sent = served.borrow()[0]["roots"].clone();
        assert_eq!(sent.as_array().unwrap().len(), 6, "node sent 6 roots (7, 1e300, 2^64, 2^53, -3, 1); the Rust sent {sent}");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-A8. channels.ts:461-466 `rows.map(row => String(row["id"]))`: a list row that is null throws a TypeError, so the
// frame is answered as an error ("Cannot read properties of null (reading 'id')", node: zz-review-account-ptys.oracle.ts
// "tagged null row"). ptys.rs reads `row.get("id")` of a null as nothing and answers the list. A daemon names no null row.

#[test]
fn a_null_row_in_a_pty_list_fails_the_list_as_it_does_in_the_ts() {
    run(async {
        let w = World::new();
        let area = w.area();
        let d = Daemon::new(|_| frame(json!({ "id": null, "ok": true, "ptys": [null] })));
        let c = area.thread_tagged("wnull", d);
        let answered = c.send(frame(json!({ "id": 9, "op": "pty.list" }))).await;
        assert!(answered.is_err(), "node threw a TypeError; the Rust answered {answered:?}");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// channels.ts:358-373 `killGuarded`, over frames a page could send: its own protect, apps, agents and agentBins are
// dropped or replaced in place, and an op that is not exactly "proc.kill" passes as it came. Expected: node
// (zz-review-account-ptys.oracle.ts "kill heard"). Found right.

#[test]
fn the_kill_guard_answers_odd_frames_as_the_ts_does() {
    run(async {
        let w = World::new();
        *w.agent_bins.borrow_mut() =
            ["claude", "codex", "gemini", "opencode", "pi", "hermes", "crush", "qwen", "goose", "amp", "cursor-agent"].iter().map(|b| b.to_string()).collect();
        let area = w.area();
        let inner = Daemon::ok();
        let guard = area.kill_guarded(inner.clone(), false);
        for f in [
            json!({ "id": 1, "op": "proc.kill", "pid": 5, "protect": "x", "apps": null, "agents": 5, "agentBins": "evil" }),
            json!({ "op": "proc.kill" }),
            json!({ "id": 2, "op": "proc.kill ", "pid": 7 }),
            json!({ "id": 3, "op": ["proc.kill"] }),
        ] {
            send(&guard, f).await;
        }
        let bins = r#"["claude","codex","gemini","opencode","pi","hermes","crush","qwen","goose","amp","cursor-agent"]"#;
        let want = format!(
            r#"[{{"id":1,"op":"proc.kill","pid":5,"agentBins":{bins}}},{{"op":"proc.kill","agentBins":{bins}}},{{"id":2,"op":"proc.kill ","pid":7}},{{"id":3,"op":["proc.kill"]}}]"#
        );
        assert_eq!(Value::Array(inner.heard()).to_string(), want);
    });
}
