// SPDX-License-Identifier: AGPL-3.0-only
//! The fakes of the thread_ends oracle: a runtime and a place door that log every call in the oracle's shape, over a
//! memory store, and the world a scene builds them from. Included with `#[path = "support/thread_ends.rs"] mod support;`.
#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;

use async_trait::async_trait;
use futures::channel::oneshot;
use serde_json::{json, Value};

use wsp_core::types::pending::Store;
use wsp_engine::machine::ExecResult;
use wsp_machines::machines::thread_ends::{ThreadEnds, ThreadEndsDoor, ThreadEndsRuntime, OWED};
use wsp_wire::types::js_number::JsInt;
use wsp_words::exit::HostError;

use crate::common::{self, Log, MemStore, StoreLogs};

/// What the oracle's scripted door answers to one exec: `{exitCode, stderr}`, `"throws"` or `"hangs"`.
pub enum Answer {
    Exit(i64, &'static str),
    Throws,
    Hangs,
}

/// The oracle's `World` plus its log and store.
pub struct World {
    pub log: Log,
    pub store: Rc<MemStore>,
    pub linked: RefCell<HashSet<String>>,
    pub running: RefCell<HashSet<String>>,
    pub launching: RefCell<HashSet<String>>,
    pub holds: RefCell<HashSet<String>>,
    pub answers: RefCell<VecDeque<Answer>>,
    gate: RefCell<Option<oneshot::Receiver<()>>>,
}

fn names(v: &Value) -> HashSet<String> {
    v.as_array().map(|a| a.iter().map(|x| x.as_str().expect("a name").to_string()).collect()).unwrap_or_default()
}

impl World {
    /// The world of a fixture scene: its seed rows (keyed by place id, as the oracle seeds them) and its `world`.
    pub fn of_scene(scene: &Value, answers: Vec<Answer>) -> Rc<World> {
        World::new(&scene["seed"], &scene["world"], answers)
    }

    pub fn new(seed: &Value, world: &Value, answers: Vec<Answer>) -> Rc<World> {
        let log = Log::default();
        let logs = StoreLogs { get: true, put: true, delete: true, ..Default::default() };
        let store = MemStore::seeded(&json!({ OWED: seed }), log.clone(), logs);
        Rc::new(World {
            log,
            store,
            linked: RefCell::new(names(&world["linked"])),
            running: RefCell::new(names(&world["running"])),
            launching: RefCell::new(names(&world["launching"])),
            holds: RefCell::new(names(&world["holds"])),
            answers: RefCell::new(answers.into()),
            gate: RefCell::new(None),
        })
    }

    /// `ThreadEnds` over the fakes of this world, as `threadEnds(ctx, door)` is built in the oracle.
    pub fn ends(self: &Rc<Self>) -> Rc<ThreadEnds> {
        ThreadEnds::new(Rc::new(FakeRuntime { world: Rc::clone(self) }), Rc::new(FakeDoor { world: Rc::clone(self) }))
    }

    /// The next `store_get` reads its row at once and answers only once the returned sender is used (or dropped).
    pub fn hold_next_get(&self) -> oneshot::Sender<()> {
        let (release, held) = oneshot::channel();
        *self.gate.borrow_mut() = Some(held);
        release
    }

    /// The log entries whose first word is one of `kinds`, in order.
    pub fn log_of(&self, kinds: &[&str]) -> Vec<Value> {
        self.log.take().into_iter().filter(|e| kinds.iter().any(|k| e[0] == *k)).collect()
    }
}

pub struct FakeRuntime {
    world: Rc<World>,
}

#[async_trait(?Send)]
impl ThreadEndsRuntime for FakeRuntime {
    async fn store_get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        let row = Store::get(&*self.world.store, collection, id).await?;
        let held = self.world.gate.borrow_mut().take();
        if let Some(held) = held {
            let _ = held.await;
        }
        Ok(row)
    }

    async fn store_put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        Store::put(&*self.world.store, collection, id, value).await
    }

    async fn store_delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        Store::delete(&*self.world.store, collection, id).await
    }

    async fn ready(&self) -> Result<(), HostError> {
        self.world.log.push(json!(["ready"]));
        Ok(())
    }

    fn running_on(&self, thread_id: &str) -> bool {
        self.world.log.push(json!(["runningOn", thread_id]));
        self.world.running.borrow().contains(thread_id)
    }

    fn launching_on(&self, thread_id: &str) -> bool {
        self.world.log.push(json!(["launchingOn", thread_id]));
        self.world.launching.borrow().contains(thread_id)
    }

    fn stop_holds(&self, thread_id: &str) -> bool {
        self.world.log.push(json!(["stopHolds", thread_id]));
        self.world.holds.borrow().contains(thread_id)
    }

    fn warn(&self, line: &str) {
        self.world.log.push(json!(["warn", line]));
    }
}

pub struct FakeDoor {
    world: Rc<World>,
}

#[async_trait(?Send)]
impl ThreadEndsDoor for FakeDoor {
    async fn exec(&self, place_id: &str, script: &str, timeout_ms: u64) -> Result<ExecResult, HostError> {
        self.world.log.push(json!(["door.exec", place_id, script, { "timeoutMs": timeout_ms }]));
        let answer = self.world.answers.borrow_mut().pop_front().unwrap_or(Answer::Exit(0, ""));
        match answer {
            Answer::Exit(code, stderr) => Ok(ExecResult { exit_code: JsInt::from(code), stdout: String::new(), stderr: stderr.to_string() }),
            Answer::Throws => Err(HostError { message: "no answer".into(), fix: None, kind: None, errors: None, warnings: None, left: None, marks: Vec::new() }),
            Answer::Hangs => std::future::pending::<Result<ExecResult, HostError>>().await,
        }
    }

    fn link(&self, place_id: &str) -> bool {
        self.world.log.push(json!(["door.link", place_id]));
        self.world.linked.borrow().contains(place_id)
    }

    fn name_of(&self, place_id: &str) -> String {
        if place_id == "p1" { "hetzner".to_string() } else { place_id.to_string() }
    }
}

/// Lets what the scene started run, then asserts the log and the store rows equal the fixture's.
pub async fn finish(world: &World, scene: &Value) {
    common::flush().await;
    common::assert_log(scene, &world.log.take());
    assert_eq!(world.store.table(OWED), scene["rows"], "scene {:?}: store rows", scene["name"]);
}

/// The scenes whose act is one `end` on `p1`/`t1` and its answer.
pub async fn end_scene(name: &str, answers: Vec<Answer>, ask: wsp_machines::machines::thread_ends::EndAsk) {
    let fixture = common::fixture("thread_ends");
    let scene = common::scene(&fixture, name);
    let world = World::of_scene(scene, answers);
    let ends = world.ends();
    let said = ends.end("p1", "t1", ask).await.expect("end answers");
    world.log.push(json!(["answer", said]));
    finish(&world, scene).await;
}

/// What the oracle's `log.push(["answer", ...])` writes for an `owed` answer.
pub fn owed_answer(owed: &Option<wsp_machines::machines::thread_ends::OwedEnd>) -> Value {
    if owed.is_none() { Value::Null } else { json!("paid-promise") }
}

/// `void o?.paid.then(() => log.push(["paid", label]))`.
pub fn note_paid(world: &World, owed: Option<wsp_machines::machines::thread_ends::OwedEnd>, label: &'static str) {
    if let Some(owed) = owed {
        let log = world.log.clone();
        tokio::task::spawn_local(async move {
            owed.paid.await;
            log.push(json!(["paid", label]));
        });
    }
}
