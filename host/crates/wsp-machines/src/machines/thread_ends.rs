// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/machines/thread-ends.ts` at 65f6b89c7: the end of a thread's cgroup on a computer the person
//! joined, and the debt a computer that did not answer owes. Spec: spec/thread_ends.md.

use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use futures::channel::oneshot;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::json;
use wsp_engine::exec_detached::put::INLINE_EXEC_MS;
use wsp_engine::machine::ExecResult;
use wsp_wire::types::js_number::JsInt;
use wsp_wire::wire::limits::STOP_REACH_MS;
use wsp_words::exit::HostError;
use wsp_words::thread_cgroup::{cgroup_end_script, thread_cgroup};
use wsp_words::words::computer::{
    thread_end_away_line, thread_end_late_line, thread_end_owed_line, thread_left_line,
};

/// The store collection of the threads each computer is owed an end of, by place id (thread-ends.ts:14).
pub const OWED: &str = "thread-ends";

/// One computer's debt (thread-ends.ts:16).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owed {
    pub threads: Vec<String>,
}

/// What the area reads off the rest of the runtime (thread-ends.ts:33-51).
#[async_trait(?Send)]
pub trait ThreadEndsRuntime {
    async fn store_get(
        &self,
        collection: &str,
        id: &str,
    ) -> Result<Option<serde_json::Value>, HostError>;
    async fn store_put(
        &self,
        collection: &str,
        id: &str,
        value: serde_json::Value,
    ) -> Result<(), HostError>;
    async fn store_delete(&self, collection: &str, id: &str) -> Result<(), HostError>;
    async fn ready(&self) -> Result<(), HostError>;
    /// `ctx.runningOn(thread) !== undefined`.
    fn running_on(&self, thread_id: &str) -> bool;
    /// `ctx.launchingOn(thread) !== undefined`.
    fn launching_on(&self, thread_id: &str) -> bool;
    fn stop_holds(&self, thread_id: &str) -> bool;
    fn warn(&self, line: &str);
}

/// The place door as this area reads it (thread-ends.ts:46, 101, 111).
#[async_trait(?Send)]
pub trait ThreadEndsDoor {
    async fn exec(
        &self,
        place_id: &str,
        script: &str,
        timeout_ms: u64,
    ) -> Result<ExecResult, HostError>;
    /// `door.link(place) !== undefined`.
    fn link(&self, place_id: &str) -> bool;
    fn name_of(&self, place_id: &str) -> String;
}

/// The door's events this area hears (thread-ends.ts:87).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaceEvent {
    Present { place_id: String },
    Removed { place_id: String },
    Other,
}

/// What a stop or a delete asks of `end` (thread-ends.ts:24): wsp-core's `EndThread`, the same two flags.
pub use wsp_core::types::internal::EndThread as EndAsk;

/// A debt still owed: `paid` resolves once that computer ran the end or was removed (thread-ends.ts:27).
pub struct OwedEnd {
    pub paid: futures::future::LocalBoxFuture<'static, ()>,
}

/// `threadEnds(ctx, door)` (thread-ends.ts:33).
pub struct ThreadEnds {
    pub(crate) rt: Rc<dyn ThreadEndsRuntime>,
    pub(crate) door: Rc<dyn ThreadEndsDoor>,
    /// `waiting`: what waits on a thread's owed end, by `<place>\n<thread>`.
    pub(crate) waiting: RefCell<IndexMap<String, Vec<oneshot::Sender<()>>>>,
}

/// The answer of an end that ran: every process ended, or these pids still stand.
enum Ended {
    All,
    Left(Vec<i64>),
}

/// `survivorsOf` (thread-ends.ts:31): the pids the end script names as still standing.
pub fn survivors_of(stderr: &str) -> Vec<i64> {
    for (at, _) in stderr.match_indices("processes") {
        let mut tail = &stderr[at + "processes".len()..];
        let mut pids = Vec::new();
        while let Some(after_space) = tail.strip_prefix(' ') {
            let digits = after_space.bytes().take_while(u8::is_ascii_digit).count();
            if digits == 0 {
                break;
            }
            pids.push(&after_space[..digits]);
            tail = &after_space[digits..];
        }
        if !pids.is_empty() && tail.starts_with(" of ") {
            return pids.iter().filter_map(|pid| pid.parse().ok()).collect();
        }
    }
    Vec::new()
}

impl ThreadEnds {
    pub fn new(rt: Rc<dyn ThreadEndsRuntime>, door: Rc<dyn ThreadEndsDoor>) -> Rc<Self> {
        Rc::new(ThreadEnds {
            rt,
            door,
            waiting: RefCell::default(),
        })
    }

    fn wait_key(place_id: &str, thread_id: &str) -> String {
        format!("{place_id}\n{thread_id}")
    }

    /// Resolves every waiter of `key` and forgets it.
    fn settle(&self, key: &str) {
        let waiters = self
            .waiting
            .borrow_mut()
            .shift_remove(key)
            .unwrap_or_default();
        for resolve in waiters {
            let _ = resolve.send(());
        }
    }

    async fn owed_on(&self, place_id: &str) -> Result<Vec<String>, HostError> {
        let row = self.rt.store_get(OWED, place_id).await?;
        Ok(row
            .and_then(|row| serde_json::from_value::<Owed>(row).ok())
            .map(|owed| owed.threads)
            .unwrap_or_default())
    }

    async fn keep(&self, place_id: &str, threads: Vec<String>) -> Result<(), HostError> {
        if threads.is_empty() {
            return self.rt.store_delete(OWED, place_id).await;
        }
        self.rt
            .store_put(OWED, place_id, json!({ "threads": threads }))
            .await
    }

    async fn owe(&self, place_id: &str, thread_id: &str) -> Result<(), HostError> {
        let mut threads = self.owed_on(place_id).await?;
        if !threads.iter().any(|owed| owed == thread_id) {
            threads.push(thread_id.to_string());
            self.keep(place_id, threads).await?;
        }
        Ok(())
    }

    /// Runs the end there: nothing when it ended all, else the pids still standing, none where it said no pids.
    async fn run(
        &self,
        place_id: &str,
        thread_id: &str,
        remove: bool,
    ) -> Result<Option<Vec<i64>>, HostError> {
        let script = cgroup_end_script(&thread_cgroup(thread_id), remove);
        let ended = self.door.exec(place_id, &script, INLINE_EXEC_MS).await?;
        Ok(if ended.exit_code == JsInt::from(0) {
            None
        } else {
            Some(survivors_of(&ended.stderr))
        })
    }

    /// A turn of the thread running or on its way stands in the same cgroup, and an end there would take it; one a stop
    /// holds back waits for this very end.
    fn busy(&self, thread_id: &str) -> bool {
        self.rt.running_on(thread_id)
            || (self.rt.launching_on(thread_id) && !self.rt.stop_holds(thread_id))
    }

    /// The end once run there: what survived it, and nothing where that computer did not answer.
    async fn ran(&self, place_id: &str, thread_id: &str, remove: bool) -> Option<Ended> {
        match self.run(place_id, thread_id, remove).await {
            Ok(None) => Some(Ended::All),
            Ok(Some(left)) => Some(Ended::Left(left)),
            Err(_) => None,
        }
    }

    /// An end that ran there pays the thread's debt, whatever survived it, so no send waits on a computer that is
    /// connected: off the state file before its waiters go, so a send reading it after this end finds nothing owed.
    async fn clear(&self, place_id: &str, thread_id: &str) -> Result<(), HostError> {
        let owed = self.owed_on(place_id).await?;
        if owed.iter().any(|owing| owing == thread_id) {
            self.keep(
                place_id,
                owed.into_iter()
                    .filter(|owing| owing != thread_id)
                    .collect(),
            )
            .await?;
        }
        self.settle(&Self::wait_key(place_id, thread_id));
        Ok(())
    }

    /// Runs every end owed to that computer, and keeps the ones it did not answer or a turn of the thread stood in.
    async fn pay(&self, place_id: &str) -> Result<(), HostError> {
        self.rt.ready().await?;
        for thread in self.owed_on(place_id).await? {
            let ended = if self.busy(&thread) {
                None
            } else {
                self.ran(place_id, &thread, true).await
            };
            let Some(ended) = ended else {
                continue;
            };
            if let Ended::Left(left) = &ended {
                self.rt
                    .warn(&thread_left_line(&self.door.name_of(place_id), left));
            }
            self.clear(place_id, &thread).await?;
        }
        Ok(())
    }

    /// `paying`: a payment that runs detached, and says what it could not end.
    fn paying(self: &Rc<Self>, place_id: &str) {
        let me = Rc::clone(self);
        let place_id = place_id.to_string();
        wsp_engine::local_task::try_spawn_local(async move {
            if let Err(err) = me.pay(&place_id).await {
                let name = me.door.name_of(&place_id);
                me.rt.warn(&format!(
                    "the threads stopped or deleted while {name} was away were not ended there: {}",
                    err.message
                ));
            }
        });
    }

    /// The door's listener (thread-ends.ts:87): the wiring calls it for every door event. A payment it starts runs
    /// detached.
    pub fn on_place_event(self: &Rc<Self>, event: &PlaceEvent) {
        match event {
            PlaceEvent::Removed { place_id } => {
                let rt = Rc::clone(&self.rt);
                let row_place = place_id.clone();
                wsp_engine::local_task::try_spawn_local(async move {
                    let _ = rt.store_delete(OWED, &row_place).await;
                });
                let prefix = format!("{place_id}\n");
                let keys: Vec<String> = self
                    .waiting
                    .borrow()
                    .keys()
                    .filter(|key| key.starts_with(&prefix))
                    .cloned()
                    .collect();
                for key in keys {
                    self.settle(&key);
                }
            }
            PlaceEvent::Present { place_id } => self.paying(place_id),
            PlaceEvent::Other => {}
        }
    }

    /// `end` (thread-ends.ts:97), spec rule 8: what a stop could not end there, in the person's words.
    pub async fn end(
        self: &Rc<Self>,
        place_id: &str,
        thread_id: &str,
        ask: EndAsk,
    ) -> Result<Option<String>, HostError> {
        if ask.away {
            self.owe(place_id, thread_id).await?;
            if !self.door.link(place_id) {
                return Ok(Some(thread_end_away_line(&self.door.name_of(place_id))));
            }
            self.paying(place_id);
            return Ok(Some(thread_end_late_line(
                &self.door.name_of(place_id),
                STOP_REACH_MS as f64 / 1000.0,
            )));
        }
        if !self.door.link(place_id) {
            if ask.remove {
                self.owe(place_id, thread_id).await?;
            }
            return Ok(None);
        }
        let ended = self.ran(place_id, thread_id, ask.remove).await;
        let name = self.door.name_of(place_id);
        if !ask.remove {
            let Some(ended) = ended else {
                return Ok(Some(thread_left_line(&name, &[])));
            };
            self.clear(place_id, thread_id).await?;
            return Ok(match ended {
                Ended::All => None,
                Ended::Left(left) => Some(thread_left_line(&name, &left)),
            });
        }
        if let Some(Ended::All) = ended {
            self.clear(place_id, thread_id).await?;
            return Ok(None);
        }
        self.owe(place_id, thread_id).await?;
        self.rt.warn(&thread_end_owed_line(&name));
        Ok(None)
    }

    /// `owed` (thread-ends.ts:125), spec rule 9.
    pub async fn owed(
        self: &Rc<Self>,
        place_id: &str,
        thread_id: &str,
    ) -> Result<Option<OwedEnd>, HostError> {
        let key = Self::wait_key(place_id, thread_id);
        let (resolve, paid) = oneshot::channel::<()>();
        self.waiting
            .borrow_mut()
            .entry(key.clone())
            .or_default()
            .push(resolve);
        let owed = self.owed_on(place_id).await?;
        if owed.iter().any(|owing| owing == thread_id) {
            return Ok(Some(OwedEnd {
                paid: Box::pin(async move {
                    let _ = paid.await;
                }),
            }));
        }
        self.settle(&key);
        Ok(None)
    }
}
