// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/readings.ts` at 65f6b89c7: a computer's live readings over the one link it holds to
//! this host, one watch per link however many readers. Spec: `spec/places_readings.md`.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use futures::future::{ready, LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{Map, Value};
use wsp_core::types::pending::DaemonReach;
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_wire::wire::daemon_version::SYS_UNWATCH_DAEMON_VERSION;
use wsp_wire::wire::machine_link::{no_such_place_refusal, SysSample};
use wsp_words::exit::HostError;

use super::context::PlaceDoorContext;

/// One reader of a computer's samples.
pub type Reader = Rc<dyn Fn(&SysSample)>;

/// Lets go of one reader's hold; a second call does nothing.
pub type Release = Rc<dyn Fn()>;

/// The readings of every linked computer (readings.ts:97). State is memory only.
pub struct PlaceReadings {
    ctx: Rc<PlaceDoorContext>,
    state: RefCell<ReadingsState>,
}

type Answer = Shared<LocalBoxFuture<'static, Map<String, Value>>>;

/// The `sys.watch` asked on one link, and the answer it gets; `id` tells this ask from a later one.
#[derive(Clone)]
struct Asked {
    id: u64,
    on: Rc<dyn DaemonReach>,
    answer: Answer,
}

/// One place's watch: its readers by id in the order they came, and the ask standing on the link.
#[derive(Default)]
struct Watched {
    readers: RefCell<IndexMap<u64, Reader>>,
    asked: RefCell<Option<Asked>>,
}

#[derive(Default)]
struct ReadingsState {
    watched: IndexMap<String, Rc<Watched>>,
    /// The links whose daemon could not be told to stop, held weakly so a dropped link is not kept alive.
    still_sending: Vec<Weak<dyn DaemonReach>>,
    next_id: u64,
}

fn ok_frame() -> Map<String, Value> {
    let mut frame = Map::new();
    frame.insert("ok".to_string(), Value::Bool(true));
    frame
}

fn refused_frame(error: String) -> Map<String, Value> {
    let mut frame = Map::new();
    frame.insert("ok".to_string(), Value::Bool(false));
    frame.insert("error".to_string(), Value::String(error));
    frame
}

fn is_ok(said: &Map<String, Value>) -> bool {
    said.get("ok") == Some(&Value::Bool(true))
}

impl PlaceReadings {
    pub fn new(ctx: Rc<PlaceDoorContext>) -> Rc<PlaceReadings> {
        Rc::new(PlaceReadings { ctx, state: RefCell::new(ReadingsState::default()) })
    }

    /// Hands `f` every sample the computer pushes until the release runs; held while the computer is away. Fails with
    /// the daemon's refusal of the watch, or for a place this host holds no computer for (readings.ts:133).
    pub async fn watch(self: &Rc<Self>, place_id: &str, f: Reader) -> Result<Release, HostError> {
        if !self.ctx.kept.borrow().contains_key(place_id) {
            let names: Vec<String> = self.ctx.kept.borrow().values().map(|r| r.name.clone()).collect();
            return Err(HostError::new(no_such_place_refusal(place_id, &names)));
        }
        let w = self.watched_of(place_id);
        let id = self.next_id();
        w.readers.borrow_mut().insert(id, f);
        let release = self.release_of(place_id, &w, id);
        let Some(on) = self.live_reach(place_id) else {
            return Ok(release);
        };
        let standing = w.asked.borrow().clone().filter(|a| Rc::ptr_eq(&a.on, &on)).map(|a| a.answer);
        let answer = match standing {
            Some(answer) => answer,
            None => self.ask(&w, &on),
        };
        let said = answer.await;
        if !is_ok(&said) && self.live_reach(place_id).is_some_and(|live| Rc::ptr_eq(&live, &on)) {
            release();
            let error = said.get("error").and_then(Value::as_str).unwrap_or("the computer would not send its readings");
            return Err(HostError::new(error));
        }
        Ok(release)
    }

    /// A link that just attached: asked for the readings where somebody is still reading them (readings.ts:157).
    pub fn linked(self: &Rc<Self>, place_id: &str) {
        let Some(w) = self.state.borrow().watched.get(place_id).cloned() else {
            return;
        };
        let Some(on) = self.live_reach(place_id) else {
            return;
        };
        if w.asked.borrow().as_ref().is_some_and(|a| Rc::ptr_eq(&a.on, &on)) {
            return;
        }
        drop(self.ask(&w, &on));
    }

    /// A sample to each reader of a snapshot taken before the first call; any other event is dropped
    /// (readings.ts:164).
    pub fn heard(&self, place_id: &str, event: &DaemonEvent) {
        let DaemonEvent::SysSample(sample) = event else {
            return;
        };
        let Some(w) = self.state.borrow().watched.get(place_id).cloned() else {
            return;
        };
        let readers: Vec<Reader> = w.readers.borrow().values().cloned().collect();
        for reader in readers {
            reader(sample);
        }
    }

    fn watched_of(&self, place_id: &str) -> Rc<Watched> {
        let mut state = self.state.borrow_mut();
        Rc::clone(state.watched.entry(place_id.to_string()).or_insert_with(|| Rc::new(Watched::default())))
    }

    fn next_id(&self) -> u64 {
        let mut state = self.state.borrow_mut();
        state.next_id += 1;
        state.next_id
    }

    fn live_reach(&self, place_id: &str) -> Option<Rc<dyn DaemonReach>> {
        self.ctx.live.borrow().get(place_id).map(|live| Rc::clone(&live.reach))
    }

    fn unwatches(&self, place_id: &str) -> bool {
        self.ctx.kept.borrow().get(place_id).is_some_and(|record| record.report.daemon_version.get() >= SYS_UNWATCH_DAEMON_VERSION)
    }

    fn still_sends(&self, on: &Rc<dyn DaemonReach>) -> bool {
        self.state.borrow().still_sending.iter().any(|link| link.upgrade().is_some_and(|live| Rc::ptr_eq(&live, on)))
    }

    /// One `sys.watch` on `on` for this place, shared by every road that asks while it stands (readings.ts:115).
    fn ask(&self, w: &Rc<Watched>, on: &Rc<dyn DaemonReach>) -> Answer {
        let id = self.next_id();
        let answer: Answer = if self.still_sends(on) {
            ready(ok_frame()).boxed_local().shared()
        } else {
            let (reach, weak) = (Rc::clone(on), Rc::downgrade(w));
            let asking = async move {
                let said = match reach.request("sys.watch", Map::new()).await {
                    Ok(frame) => frame,
                    Err(e) => refused_frame(e.message),
                };
                if !is_ok(&said) {
                    if let Some(w) = weak.upgrade() {
                        let mut asked = w.asked.borrow_mut();
                        if asked.as_ref().is_some_and(|a| a.id == id) {
                            *asked = None;
                        }
                    }
                }
                said
            };
            let answer = asking.boxed_local().shared();
            tokio::task::spawn_local(answer.clone());
            answer
        };
        *w.asked.borrow_mut() = Some(Asked { id, on: Rc::clone(on), answer: answer.clone() });
        answer
    }

    /// The release of one reader's hold on a place's watch (readings.ts:138).
    fn release_of(self: &Rc<Self>, place_id: &str, w: &Rc<Watched>, id: u64) -> Release {
        let (weak, place, w) = (Rc::downgrade(self), place_id.to_string(), Rc::clone(w));
        Rc::new(move || {
            if let Some(this) = weak.upgrade() {
                this.release(&place, &w, id);
            }
        })
    }

    fn release(&self, place_id: &str, w: &Rc<Watched>, id: u64) {
        let (removed, empty) = {
            let mut readers = w.readers.borrow_mut();
            (readers.shift_remove(&id).is_some(), readers.is_empty())
        };
        let current = self.state.borrow().watched.get(place_id).is_some_and(|cur| Rc::ptr_eq(cur, w));
        if !removed || !empty || !current {
            return;
        }
        self.state.borrow_mut().watched.shift_remove(place_id);
        let Some(asked) = w.asked.borrow().clone() else {
            return;
        };
        if !self.live_reach(place_id).is_some_and(|live| Rc::ptr_eq(&live, &asked.on)) {
            return;
        }
        if self.unwatches(place_id) {
            let on = asked.on;
            tokio::task::spawn_local(async move {
                let _ = on.request("sys.unwatch", Map::new()).await;
            });
        } else {
            self.state.borrow_mut().still_sending.push(Rc::downgrade(&asked.on));
        }
    }
}
