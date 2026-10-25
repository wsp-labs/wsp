// SPDX-License-Identifier: AGPL-3.0-only
//! Unit lazy_slates: spec/lazy_slates.md, TS test/lazy-slates.test.ts. The fake area below records each call's name
//! in order, as the TS `vi.mock` of `createSlates` does, and the factory closure pushes each fake it makes into a
//! shared list.
mod support;

use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};
use support::{local, temp_root, until, FakeHost, MemStore};
use wsp_slates::lazy_slates::{lazy_slates, SlatesFactory, SLATES};
use wsp_slates::pending::wire::{SlateEventAnswer, SlateEventOutcome, SlateReadAnswer, SlateStateAnswer, SlateView, SlateWriteAnswer, SlatesImageAnswer};
use wsp_slates::slates::{ApprovalScope, ApproveParams, CatalogParams, EventParams, ReadParams, SlateTarget, Slates, SlatesDeps, StateParams, WriteParams};
use wsp_store::store::Store;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

#[derive(Default)]
struct FakeArea {
    calls: RefCell<Vec<String>>,
    args: RefCell<Vec<String>>,
    subscribed: RefCell<Vec<Vec<String>>>,
}

impl FakeArea {
    fn note(&self, name: &str) {
        self.calls.borrow_mut().push(name.to_string());
    }
    fn noted(&self, name: &str, arg: String) {
        self.note(name);
        self.args.borrow_mut().push(format!("{name}:{arg}"));
    }
    fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
    fn args(&self) -> Vec<String> {
        self.args.borrow().clone()
    }
    fn subscribed(&self) -> Vec<Vec<String>> {
        self.subscribed.borrow().clone()
    }
}

struct Wrapped(Rc<FakeArea>);

impl std::ops::Deref for Wrapped {
    type Target = FakeArea;
    fn deref(&self) -> &FakeArea {
        &self.0
    }
}

#[async_trait(?Send)]
impl Slates for Wrapped {
    async fn get(&self, thread_id: &str) -> Result<Option<SlateView>, HostError> {
        self.noted("get", thread_id.to_string());
        Ok(None)
    }
    async fn write(&self, p: WriteParams, _caller: Option<&Caller>) -> Result<SlateWriteAnswer, HostError> {
        self.noted("write", p.text.clone().unwrap_or_default());
        Ok(SlateWriteAnswer { version: 1, text: String::new(), warnings: vec![], problems: vec![], waiting: None })
    }
    async fn state(&self, _p: StateParams, _caller: Option<&Caller>) -> Result<SlateStateAnswer, HostError> {
        self.note("state");
        Ok(SlateStateAnswer { version: 2, text: "state text".into(), problems: vec![], waiting: None, not_started: None })
    }
    async fn read(&self, _p: ReadParams, _caller: Option<&Caller>) -> Result<SlateReadAnswer, HostError> {
        self.note("read");
        Ok(SlateReadAnswer {
            version: 3,
            text: "read text".into(),
            document: None,
            values: IndexMap::new(),
            state: IndexMap::new(),
            derived: IndexMap::new(),
            runs: IndexMap::new(),
            problems: vec![],
            waiting: vec![],
            comments: vec![],
            approvals: IndexMap::new(),
        })
    }
    async fn catalog(&self, _p: CatalogParams, _caller: Option<&Caller>) -> Result<String, HostError> {
        self.note("catalog");
        Ok("catalog text".into())
    }
    async fn shown(&self, thread_id: &str) -> Result<(), HostError> {
        self.noted("shown", thread_id.to_string());
        Ok(())
    }
    async fn revoke(&self, thread_id: &str, key: &str) -> Result<(), HostError> {
        self.noted("revoke", format!("{thread_id},{key}"));
        Ok(())
    }
    async fn event(&self, p: EventParams) -> Result<SlateEventAnswer, HostError> {
        self.noted("event", p.piece);
        Ok(SlateEventAnswer { outcome: SlateEventOutcome::Done, said: "event said".into(), turn_id: None, ask: None })
    }
    async fn approve(&self, p: ApproveParams) -> Result<(), HostError> {
        self.noted("approve", p.key);
        Ok(())
    }
    async fn cancel(&self, thread_id: &str, run: &str) -> Result<(), HostError> {
        self.noted("cancel", format!("{thread_id},{run}"));
        Ok(())
    }
    fn subscribe(&self, _thread_id: &str, sources: &[String]) -> Rc<dyn Fn()> {
        self.note("subscribe");
        self.subscribed.borrow_mut().push(sources.to_vec());
        let me = self.0.clone();
        let mine = sources.to_vec();
        Rc::new(move || {
            let mut held = me.subscribed.borrow_mut();
            if let Some(i) = held.iter().position(|s| *s == mine) {
                held.remove(i);
            }
        })
    }
    async fn resolve(&self, thread_id: &str, paths: &[String]) -> Result<IndexMap<String, Value>, HostError> {
        self.noted("resolve", format!("{thread_id},{}", paths.join(",")));
        Ok(IndexMap::from([("a".to_string(), json!(1))]))
    }
    async fn image(&self, thread_id: &str, src: &str, _have: Option<&str>) -> Result<SlatesImageAnswer, HostError> {
        self.noted("image", format!("{thread_id},{src}"));
        Ok(json!({"kind": "image"}))
    }
    async fn turn_ended(&self, thread_id: &str, turn_id: &str) -> Result<(), HostError> {
        self.noted("turn_ended", format!("{thread_id},{turn_id}"));
        Ok(())
    }
    async fn rewound(&self, thread_id: &str, turn_id: &str, cut: &[String]) -> Result<(), HostError> {
        self.noted("rewound", format!("{thread_id},{turn_id},{}", cut.join(",")));
        Ok(())
    }
    async fn forget(&self, thread_id: &str, keep: bool) -> Result<(), HostError> {
        self.noted("forget", format!("{thread_id},{keep}"));
        Ok(())
    }
    fn close_servers(&self, thread_id: &str) {
        self.noted("close_servers", thread_id.to_string());
    }
    fn watches_pr(&self, workspace_id: &str) -> bool {
        self.noted("watches_pr", workspace_id.to_string());
        true
    }
    fn napped(&self, workspace_id: &str) {
        self.noted("napped", workspace_id.to_string());
    }
    async fn ready(&self) -> Result<(), HostError> {
        self.note("ready");
        Ok(())
    }
    async fn settled(&self) {
        self.note("settled");
    }
    fn close(&self) {
        self.note("close");
    }
}

type Made = Rc<RefCell<Vec<Rc<FakeArea>>>>;

struct Rig {
    _root: tempfile::TempDir,
    host: Rc<FakeHost>,
    made: Made,
    slates: Rc<dyn Slates>,
    deps_seen: Rc<RefCell<Vec<usize>>>,
}

impl Rig {
    fn new() -> Rig {
        Rig::over(MemStore::new())
    }

    fn over(store: Rc<MemStore>) -> Rig {
        let root = temp_root("lazy-slates-");
        let host = FakeHost::new(root.path(), store);
        let made: Made = Rc::new(RefCell::new(Vec::new()));
        let deps_seen = Rc::new(RefCell::new(Vec::new()));
        let (list, seen) = (made.clone(), deps_seen.clone());
        let factory: SlatesFactory = Box::new(move |deps: Rc<dyn SlatesDeps>| {
            seen.borrow_mut().push(Rc::as_ptr(&deps) as *const () as usize);
            let fake = Rc::new(FakeArea::default());
            list.borrow_mut().push(fake.clone());
            Rc::new(Wrapped(fake)) as Rc<dyn Slates>
        });
        let slates = lazy_slates(host.clone(), factory);
        Rig { _root: root, host, made, slates, deps_seen }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.made.borrow().iter().map(|a| a.calls()).collect()
    }

    fn made(&self) -> usize {
        self.made.borrow().len()
    }
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn write_of(text: &str) -> WriteParams {
    WriteParams { target: SlateTarget { thread_id: Some("t1".into()), turn_token: None }, text: Some(text.into()), ..Default::default() }
}

fn event_of() -> EventParams {
    EventParams { thread_id: "t1".into(), version: 1, piece: "button-1".into(), event: "press".into(), request_id: "r1".into(), scope: None, row_action: None }
}

// lazy_slates Edge case: lazy-slates.test.ts:30 (is not loaded by a host with no slate, whatever its threads do)
#[test]
fn is_not_loaded_by_a_host_with_no_slate_whatever_its_threads_do() {
    local(async {
        let rig = Rig::new();
        rig.slates.ready().await.unwrap();
        assert!(rig.slates.get("t1").await.unwrap().is_none());
        rig.slates.turn_ended("t1", "u1").await.unwrap();
        rig.slates.rewound("t1", "u1", &[]).await.unwrap();
        rig.slates.forget("t1", false).await.unwrap();
        assert!(!rig.slates.watches_pr("w1"));
        rig.slates.settled().await;
        rig.slates.close();
        assert_eq!(rig.made(), 0);
    });
}

// lazy_slates Behaviour 1 / Edge case: lazy-slates.test.ts:43 (loads at start where a slate is stored)
#[test]
fn loads_at_start_where_a_slate_is_stored_so_its_timed_runs_recover_then() {
    local(async {
        assert_eq!(SLATES, "slates");
        let store = MemStore::new();
        store.put(SLATES, "t1", json!({"schema": 2, "threadId": "t1"})).await.unwrap();
        let rig = Rig::over(store);
        rig.slates.ready().await.unwrap();
        assert_eq!(rig.calls(), vec![names(&["ready"])]);
    });
}

// lazy_slates Behaviour 4 / Edge case: lazy-slates.test.ts:50 (loads on a thread's first write, once)
#[test]
fn loads_on_a_threads_first_write_once_and_then_follows_the_threads_turns() {
    local(async {
        let rig = Rig::new();
        rig.slates.ready().await.unwrap();
        let (a, b) = futures::future::join(rig.slates.write(write_of("<slate/>"), None), rig.slates.write(write_of("<slate/>"), None)).await;
        a.unwrap();
        b.unwrap();
        rig.slates.turn_ended("t1", "u1").await.unwrap();
        assert_eq!(rig.calls(), vec![names(&["write", "write", "turn_ended"])]);
    });
}

// lazy_slates Behaviour 8 / Edge case: lazy-slates.test.ts:58 (holds a window's sources once it loads, unless the window let go first)
#[test]
fn holds_a_windows_sources_once_it_loads_unless_the_window_let_go_first() {
    local(async {
        let rig = Rig::new();
        let kept = rig.slates.subscribe("t1", &names(&["usage"]));
        rig.slates.subscribe("t1", &names(&["pr"]))();
        assert!(until(2_000, || rig.made() == 1 && rig.made.borrow()[0].subscribed() == vec![names(&["usage"])]).await);
        kept();
        assert_eq!(rig.made.borrow()[0].subscribed(), Vec::<Vec<String>>::new());
        assert_eq!(rig.made(), 1);
    });
}

// lazy_slates Behaviour 2: making the area calls create once, with the deps the wrapper was given
#[test]
fn making_the_area_gives_the_factory_the_wrappers_deps_once() {
    local(async {
        let rig = Rig::new();
        rig.slates.write(write_of("a"), None).await.unwrap();
        rig.slates.write(write_of("b"), None).await.unwrap();
        rig.slates.shown("t1").await.unwrap();
        let mine = Rc::as_ptr(&rig.host) as *const () as usize;
        assert_eq!(*rig.deps_seen.borrow(), vec![mine]);
        assert_eq!(rig.calls(), vec![names(&["write", "write", "shown"])]);
    });
}

// lazy_slates Behaviour 3 / 5: each call of `used` makes the area and reaches it, answering what it answered
#[test]
fn every_used_call_makes_the_area_and_passes_through_its_answer() {
    local(async {
        let rig = Rig::new();
        let s = &rig.slates;
        assert_eq!(s.write(write_of("<slate/>"), None).await.unwrap().version, 1);
        assert_eq!(s.state(StateParams::default(), None).await.unwrap().text, "state text");
        assert_eq!(s.read(ReadParams::default(), None).await.unwrap().text, "read text");
        assert_eq!(s.catalog(CatalogParams::default(), None).await.unwrap(), "catalog text");
        s.shown("t1").await.unwrap();
        s.revoke("t1", "k1").await.unwrap();
        assert_eq!(s.event(event_of()).await.unwrap().said, "event said");
        s.approve(ApproveParams { thread_id: "t1".into(), key: "k2".into(), scope: ApprovalScope::Once }).await.unwrap();
        s.cancel("t1", "build").await.unwrap();
        assert_eq!(s.resolve("t1", &names(&["a", "b"])).await.unwrap(), IndexMap::from([("a".to_string(), json!(1))]));
        assert_eq!(s.image("t1", "pic.png", None).await.unwrap(), json!({"kind": "image"}));
        assert_eq!(rig.calls(), vec![names(&["write", "state", "read", "catalog", "shown", "revoke", "event", "approve", "cancel", "resolve", "image"])]);
        assert_eq!(
            rig.made.borrow()[0].args(),
            names(&["write:<slate/>", "shown:t1", "revoke:t1,k1", "event:button-1", "approve:k2", "cancel:t1,build", "resolve:t1,a,b", "image:t1,pic.png"])
        );
    });
}

// lazy_slates Behaviour 3: calls made before the area exists run in call order
#[test]
fn calls_made_before_the_area_exists_reach_it_in_call_order() {
    local(async {
        let rig = Rig::new();
        let s = &rig.slates;
        let (w, st, r, c) = futures::join!(s.write(write_of("x"), None), s.state(StateParams::default(), None), s.read(ReadParams::default(), None), s.shown("t1"));
        w.unwrap();
        st.unwrap();
        r.unwrap();
        c.unwrap();
        assert_eq!(rig.calls(), vec![names(&["write", "state", "read", "shown"])]);
    });
}

// lazy_slates Behaviour 3 / Rule 3.4: once the area is made a call goes straight through with no extra wait
#[test]
fn a_call_after_the_area_is_made_runs_straight_through() {
    local(async {
        let rig = Rig::new();
        rig.slates.write(write_of("x"), None).await.unwrap();
        let at_once = rig.slates.shown("t1").now_or_never();
        assert!(matches!(at_once, Some(Ok(()))));
        let state = rig.slates.state(StateParams::default(), None).now_or_never();
        assert_eq!(state.unwrap().unwrap().version, 2);
        assert_eq!(rig.calls(), vec![names(&["write", "shown", "state"])]);
    });
}

// lazy_slates Behaviour 4: a slate stored of any shape is found by `get`, and the area's answer is passed on
#[test]
fn get_where_a_slate_is_stored_reaches_the_area() {
    local(async {
        let store = MemStore::new();
        store.put(SLATES, "other", json!({"anything": [1, 2]})).await.unwrap();
        let rig = Rig::over(store);
        assert!(rig.slates.get("t1").await.unwrap().is_none());
        assert_eq!(rig.calls(), vec![names(&["get"])]);
        assert_eq!(rig.made.borrow()[0].args(), names(&["get:t1"]));
    });
}

// lazy_slates Behaviour 5: where a slate is stored the turn-following calls reach the area with their arguments
#[test]
fn turn_following_calls_reach_the_area_where_a_slate_is_stored() {
    local(async {
        let store = MemStore::new();
        store.put(SLATES, "t1", json!({"schema": 2, "threadId": "t1"})).await.unwrap();
        let rig = Rig::over(store);
        rig.slates.turn_ended("t1", "u1").await.unwrap();
        rig.slates.rewound("t1", "u1", &names(&["u2", "u3"])).await.unwrap();
        rig.slates.forget("t1", true).await.unwrap();
        assert_eq!(rig.calls(), vec![names(&["turn_ended", "rewound", "forget"])]);
        assert_eq!(rig.made.borrow()[0].args(), names(&["turn_ended:t1,u1", "rewound:t1,u1,u2,u3", "forget:t1,true"]));
    });
}

// lazy_slates Behaviour 4: with nothing stored the store is read once and the answer kept
#[test]
fn a_host_with_nothing_stored_reads_the_store_once() {
    local(async {
        let rig = Rig::new();
        rig.slates.ready().await.unwrap();
        rig.slates.get("t1").await.unwrap();
        rig.slates.turn_ended("t1", "u1").await.unwrap();
        rig.slates.rewound("t1", "u1", &[]).await.unwrap();
        rig.slates.forget("t1", false).await.unwrap();
        assert_eq!(rig.host.store.lists.get(), 1);
        assert_eq!(rig.made(), 0);
        rig.slates.write(write_of("x"), None).await.unwrap();
        rig.slates.ready().await.unwrap();
        assert_eq!(rig.calls(), vec![names(&["write", "ready"])]);
        assert_eq!(rig.host.store.lists.get(), 1);
    });
}

// lazy_slates Behaviour 4: once loading has started the store is not read at all
#[test]
fn once_loading_has_started_an_if_any_call_goes_to_the_area_without_reading_the_store() {
    local(async {
        let rig = Rig::new();
        rig.slates.write(write_of("x"), None).await.unwrap();
        rig.slates.ready().await.unwrap();
        assert!(rig.slates.get("t1").await.unwrap().is_none());
        rig.slates.turn_ended("t1", "u1").await.unwrap();
        assert_eq!(rig.host.store.lists.get(), 0);
        assert_eq!(rig.calls(), vec![names(&["write", "ready", "get", "turn_ended"])]);
    });
}

// lazy_slates Edge case: the remembered failed list (lazy-slates.ts:28, `stored ??=` keeps the rejected promise)
#[test]
fn a_failed_list_is_remembered_and_fails_every_later_call_until_the_area_is_made() {
    local(async {
        let rig = Rig::new();
        *rig.host.store.list_fails.borrow_mut() = Some("disk gone".into());
        assert_eq!(rig.slates.ready().await.unwrap_err().message, "disk gone");
        *rig.host.store.list_fails.borrow_mut() = None;
        assert_eq!(rig.slates.ready().await.unwrap_err().message, "disk gone");
        assert_eq!(rig.slates.get("t1").await.unwrap_err().message, "disk gone");
        assert_eq!(rig.slates.turn_ended("t1", "u1").await.unwrap_err().message, "disk gone");
        assert_eq!(rig.slates.rewound("t1", "u1", &[]).await.unwrap_err().message, "disk gone");
        assert_eq!(rig.slates.forget("t1", false).await.unwrap_err().message, "disk gone");
        assert_eq!(rig.host.store.lists.get(), 1);
        assert_eq!(rig.made(), 0);
        rig.slates.write(write_of("x"), None).await.unwrap();
        assert!(rig.slates.get("t1").await.unwrap().is_none());
        rig.slates.ready().await.unwrap();
        assert_eq!(rig.calls(), vec![names(&["write", "get", "ready"])]);
        assert_eq!(rig.host.store.lists.get(), 1);
    });
}

// lazy_slates Edge case: close, close_servers, napped and watches_pr make nothing (lazy-slates.ts:33, 59-61)
#[test]
fn close_close_servers_napped_and_watches_pr_make_no_area() {
    local(async {
        let rig = Rig::new();
        rig.slates.close_servers("t1");
        rig.slates.napped("w1");
        assert!(!rig.slates.watches_pr("w1"));
        rig.slates.close();
        assert_eq!(rig.made(), 0);
        assert_eq!(rig.host.store.lists.get(), 0);
    });
}

// lazy_slates Behaviour 7: once the area is made, close, close_servers, napped and watches_pr reach it
#[test]
fn close_close_servers_napped_and_watches_pr_reach_a_made_area() {
    local(async {
        let rig = Rig::new();
        rig.slates.write(write_of("x"), None).await.unwrap();
        rig.slates.close_servers("t1");
        rig.slates.napped("w1");
        assert!(rig.slates.watches_pr("w1"));
        rig.slates.close();
        assert_eq!(rig.calls(), vec![names(&["write", "close_servers", "napped", "watches_pr", "close"])]);
        assert_eq!(rig.made.borrow()[0].args(), names(&["write:x", "close_servers:t1", "napped:w1", "watches_pr:w1"]));
    });
}

// lazy_slates Edge case: settled with nothing made answers at once and makes nothing
#[test]
fn settled_with_nothing_made_answers_at_once_and_makes_nothing() {
    local(async {
        let rig = Rig::new();
        assert!(rig.slates.settled().now_or_never().is_some());
        assert_eq!(rig.made(), 0);
        assert_eq!(rig.host.store.lists.get(), 0);
    });
}

// lazy_slates Behaviour 6: settled reaches a made area
#[test]
fn settled_reaches_a_made_area() {
    local(async {
        let rig = Rig::new();
        rig.slates.write(write_of("x"), None).await.unwrap();
        rig.slates.settled().await;
        assert_eq!(rig.calls(), vec![names(&["write", "settled"])]);
    });
}

// lazy_slates Behaviour 6: settled waits for an area that is loading, then asks it
#[test]
fn settled_waits_for_the_loading_area_and_then_asks_it() {
    local(async {
        let rig = Rig::new();
        let release = rig.slates.subscribe("t1", &names(&["usage"]));
        rig.slates.settled().await;
        assert_eq!(rig.made(), 1);
        assert!(until(2_000, || rig.made.borrow()[0].subscribed() == vec![names(&["usage"])]).await);
        let mut calls = rig.made.borrow()[0].calls();
        calls.sort();
        assert_eq!(calls, names(&["settled", "subscribe"]));
        release();
    });
}

// lazy_slates Behaviour 8: with the area made, subscribe is the area's own and so is its release
#[test]
fn subscribe_with_the_area_made_is_the_areas_own_at_once() {
    local(async {
        let rig = Rig::new();
        rig.slates.write(write_of("x"), None).await.unwrap();
        let release = rig.slates.subscribe("t1", &names(&["usage", "pr"]));
        assert_eq!(rig.made.borrow()[0].subscribed(), vec![names(&["usage", "pr"])]);
        release();
        assert_eq!(rig.made.borrow()[0].subscribed(), Vec::<Vec<String>>::new());
        assert_eq!(rig.calls(), vec![names(&["write", "subscribe"])]);
    });
}

// lazy_slates Behaviour 8: a subscribe starts making the area and answers a release at once
#[test]
fn subscribe_starts_the_area_and_answers_a_release_at_once() {
    local(async {
        let rig = Rig::new();
        let release = rig.slates.subscribe("t1", &names(&["usage"]));
        assert_eq!(rig.host.store.lists.get(), 0);
        assert!(until(2_000, || rig.made() == 1).await);
        release();
        release();
        assert!(until(2_000, || rig.made.borrow()[0].subscribed().is_empty()).await);
        assert_eq!(rig.made(), 1);
    });
}
