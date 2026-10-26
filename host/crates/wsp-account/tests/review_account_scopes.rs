// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the process scopes over generated worlds. `oracle/zz-review-account-scopes-fuzz.oracle.ts` ran
//! `procScopesArea` (proc-scopes.ts at 65f6b89c7) over 400 seeded worlds (duplicate and negative start times, sessions
//! with no thread or no pid, threads of workspaces the host does not hold, 70 pids for one thread, 300 threads) and wrote
//! each world beside what `scopesHere()` and `scopesOn()` answered, as tests/fixtures/oracle/zz_review_account_scopes_fuzz.json.
//! The context below is the one tests/proc_scopes.rs builds.
#![allow(dead_code, unused_imports)]
mod common;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use serde_json::{json, Value};

use common::{plain, run, settle};
use wsp_account::account::core_types::{AccountResult, Frame};
use wsp_account::account::proc_scopes::{KeptRow, LatestRow, ProcScopesArea, ProcScopesCtx, ScopeRecord, ScopeRow, ThreadTerminal};
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::wire::daemon::ProcKind;

/// A channel that records what it was sent and when it was closed (the number of frames sent by then), and answers
/// every frame ok. `fail_first` makes that many sends fail, as a daemon from before proc.scopes refuses it.
struct Chan {
    sent: RefCell<Vec<Value>>,
    closes: RefCell<Vec<usize>>,
    fail_first: Cell<usize>,
    ended: Cell<bool>,
}

impl Chan {
    fn new() -> Rc<Chan> {
        Rc::new(Chan { sent: RefCell::default(), closes: RefCell::default(), fail_first: Cell::new(0), ended: Cell::new(false) })
    }
    fn sent(&self) -> Vec<Value> {
        self.sent.borrow().clone()
    }
    fn ops(&self) -> Vec<String> {
        self.sent().iter().map(|f| f["op"].as_str().unwrap().to_string()).collect()
    }
}

#[async_trait(?Send)]
impl DaemonChannel for Chan {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        self.sent.borrow_mut().push(Value::Object(frame.clone()));
        if self.fail_first.get() > 0 {
            self.fail_first.set(self.fail_first.get() - 1);
            return Err(ChannelError::Unanswered { message: "connection lost".into(), cause: None });
        }
        Ok(json!({ "id": frame.get("id").cloned().unwrap_or(Value::Null), "ok": true }).as_object().unwrap().clone())
    }
    fn close(&self) {
        let at = self.sent.borrow().len();
        self.closes.borrow_mut().push(at);
    }
    async fn closed(&self) -> ChannelEnd {
        while !self.ended.get() {
            tokio::task::yield_now().await;
        }
        ChannelEnd { code: 1006, reason: String::new() }
    }
}

/// The context the oracle's `build` made: a world as plain JSON (the fixture holds each one beside its answer).
struct Scopes {
    turn_groups: Vec<(String, Vec<(i64, String)>)>,
    groups: HashSet<i64>,
    sessions: Vec<ScopeRow>,
    kept: RefCell<Vec<(String, KeptRow)>>,
    latest: HashMap<String, LatestRow>,
    thread_records: Vec<(String, String)>,
    terminals: Vec<ThreadTerminal>,
    records: HashMap<String, ScopeRecord>,
    owned: Vec<(String, i64, String)>,
    /// What each call of `channel_here` answers, in turn; none left fails (no daemon wired here).
    dials: RefCell<VecDeque<AccountResult<Rc<Chan>>>>,
    dialled: Cell<usize>,
}

fn kind_of(word: &str) -> WorkspaceKind {
    serde_json::from_value(json!(word)).unwrap()
}

fn proc_kind(word: &str) -> ProcKind {
    serde_json::from_value(json!(word)).unwrap()
}


/// `WorldJson` of the oracle with every key a scenario leaves out empty.
fn world(w: &Value) -> Rc<Scopes> {
    let list = |k: &str| w[k].as_array().cloned().unwrap_or_default();
    Rc::new(Scopes {
        turn_groups: list("turnGroups")
            .iter()
            .map(|g| {
                (
                    g[0].as_str().unwrap().to_string(),
                    g[1].as_array().unwrap().iter().map(|p| (p[0].as_i64().unwrap(), p[1].as_str().unwrap().to_string())).collect(),
                )
            })
            .collect(),
        groups: list("groupsAlive").iter().map(|g| g.as_i64().unwrap()).collect(),
        sessions: list("sessions")
            .iter()
            .map(|s| ScopeRow {
                workspace_id: s["workspace"].as_str().unwrap().into(),
                thread_id: s["thread"].as_str().map(String::from),
                status: s["status"].as_str().unwrap().into(),
                started_at: s["startedAt"].as_f64(),
                pid: s["pid"].as_i64(),
            })
            .collect(),
        kept: RefCell::new(
            list("kept")
                .iter()
                .map(|k| {
                    (
                        k[0].as_str().unwrap().to_string(),
                        KeptRow {
                            workspace_id: k[1]["workspace"].as_str().unwrap().into(),
                            pid: k[1]["pid"].as_i64(),
                            warm_unclaimed: k[1]["warmUnclaimed"] == json!(true),
                        },
                    )
                })
                .collect(),
        ),
        latest: w["latest"]
            .as_object()
            .map(|m| {
                m.iter()
                    .map(|(t, l)| {
                        (
                            t.clone(),
                            LatestRow { workspace_id: l["workspace"].as_str().unwrap().into(), cwd: l["cwd"].as_str().map(String::from) },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        thread_records: list("threadRecords")
            .iter()
            .map(|r| (r[0].as_str().unwrap().to_string(), r[1].as_str().unwrap().to_string()))
            .collect(),
        terminals: list("terminals")
            .iter()
            .map(|t| ThreadTerminal {
                thread: t["thread"].as_str().unwrap().into(),
                workspace: t["workspace"].as_str().unwrap().into(),
                pid: t["pid"].as_i64().unwrap(),
            })
            .collect(),
        records: w["live"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| {
                (
                    l["id"].as_str().unwrap().to_string(),
                    ScopeRecord {
                        kind: kind_of(l["kind"].as_str().unwrap()),
                        place: l["place"].as_str().map(String::from),
                        checkout: l["path"].as_str().unwrap().into(),
                    },
                )
            })
            .collect(),
        owned: list("owned")
            .iter()
            .map(|o| (o[0].as_str().unwrap().to_string(), o[1].as_i64().unwrap(), o[2].as_str().unwrap().to_string()))
            .collect(),
        dials: RefCell::default(),
        dialled: Cell::new(0),
    })
}


#[async_trait(?Send)]
impl ProcScopesCtx for Scopes {
    fn turn_groups(&self) -> Vec<(String, Vec<(i64, String)>)> {
        self.turn_groups.clone()
    }
    fn group_exists(&self, pgid: i64) -> bool {
        self.groups.contains(&pgid)
    }
    fn sessions(&self) -> Vec<ScopeRow> {
        self.sessions.clone()
    }
    fn kept_agents(&self) -> Vec<(String, KeptRow)> {
        self.kept.borrow().clone()
    }
    fn latest_on(&self, thread_id: &str) -> Option<LatestRow> {
        self.latest.get(thread_id).cloned()
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        self.thread_records.clone()
    }
    fn thread_terminals(&self) -> Vec<ThreadTerminal> {
        self.terminals.clone()
    }
    fn live_record(&self, workspace_id: &str) -> Option<ScopeRecord> {
        self.records.get(workspace_id).cloned()
    }
    fn shared_daemon(&self, kind: WorkspaceKind) -> bool {
        kind == WorkspaceKind::Local
    }
    async fn channel_here(&self) -> AccountResult<Rc<dyn DaemonChannel>> {
        self.dialled.set(self.dialled.get() + 1);
        match self.dials.borrow_mut().pop_front() {
            Some(Ok(c)) => Ok(c),
            Some(Err(e)) => Err(e),
            None => Err(plain("this host wired no daemon here").into()),
        }
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        tokio::task::spawn_local(f);
    }
}

/// The area over a world, with the world's `ownProcess` calls made in order.
fn area_of(w: &Rc<Scopes>) -> Rc<ProcScopesArea> {
    let area = ProcScopesArea::new(w.clone());
    for (thread, pid, kind) in &w.owned {
        area.own_process(thread, *pid, proc_kind(kind));
    }
    area
}

fn canon(v: &Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 9.0e15 => json!(f as i64),
            _ => v.clone(),
        },
        Value::Array(a) => Value::Array(a.iter().map(canon).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), canon(x))).collect()),
        _ => v.clone(),
    }
}

// proc_scopes.rs:358 (and box_panels.rs:331) `sort_by(partial_cmp ... unwrap_or(Equal))`: with a start time that is NaN the
// comparison is no total order, and Rust's sort panics once a list passes twenty rows ("user-provided comparison function
// does not correctly implement a total order"), where JavaScript's `sort((a, b) => b.startedAt - a.startedAt)` at
// proc-scopes.ts:166 and box-panels.ts:75 sorts on. No stored file reaches it (serde_json reads no NaN and refuses 1e999),
// so it is latent: the host would have to compute a start time that is not a number.
#[test]
fn a_session_with_no_number_for_a_start_time_does_not_stop_the_scopes() {
    run(async {
        let times = [f64::NAN, 1.0, f64::NAN, 5.0, f64::INFINITY, 0.0, f64::NEG_INFINITY, 3.0, f64::NAN, 2.0];
        let sessions: Vec<Value> = (0..60)
            .map(|i| json!({ "thread": format!("t{i}"), "workspace": "ws_box", "status": "running" }))
            .collect();
        let w = json!({ "live": [{ "id": "ws_box", "kind": "place", "place": "pl_1", "path": "/box/ws_box" }], "sessions": sessions });
        let ctx = {
            let base = world(&w);
            let mut rows = base.sessions.clone();
            for (i, row) in rows.iter_mut().enumerate() {
                row.started_at = Some(times[(i * 7 + i / 3) % times.len()]);
            }
            Rc::new(Scopes { sessions: rows, ..Rc::try_unwrap(base).ok().expect("one owner") })
        };
        let area = area_of(&ctx);
        let on = area.scopes_on("pl_1");
        assert_eq!(on.len(), 60);
        let _ = area.scopes_here();
    });
}
