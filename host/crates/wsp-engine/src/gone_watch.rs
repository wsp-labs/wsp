// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `GoneWatch` (packages/engine/src/golden.ts:200-260), over `gone::read_back_gone`: stops for the sweeps that
//! never wait on a machine. A stop resolves once the provider takes the first ask, so a failure to ask still reaches the
//! sweep; the read-back runs behind that on the host's local task set, as the TS's promise runs after its caller, and
//! asks again every `retry_ms` for as long as the machine is still there, until the watch closes or the provider refuses
//! the account. Outside a `LocalSet` there is nothing to run it on, so `settled` drives it.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use futures::future::{select, LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::IndexMap;
use tokio::sync::watch;

use crate::box_backend::Warn;
use crate::errors::{is_account_refusal, is_missing, RealRetryClock, Result, RetryClock};
use crate::gone::{read_back_gone, KillConfirm, WatchRound};
use crate::local_task::try_spawn_local;
use crate::machine::{Machine, MachineBackend};

/// golden.ts:193. How long a background stop whose read-back ended with the machine still there waits before asking again.
pub const GONE_RETRY_MS: u64 = 60_000;

/// golden.ts:195 `GoneWatchOptions`. `warn` is `console.warn` (stderr) and `clock` the wall clock when `None`.
#[derive(Default, Clone)]
pub struct GoneWatchOptions {
    pub confirm: KillConfirm,
    pub retry_ms: Option<u64>,
    pub warn: Option<Warn>,
    pub clock: Option<Arc<dyn RetryClock>>,
}

type Watched = Shared<LocalBoxFuture<'static, ()>>;

struct Inner {
    confirm: KillConfirm,
    retry_ms: u64,
    warn: Warn,
    clock: Arc<dyn RetryClock>,
    watching: RefCell<IndexMap<String, Watched>>,
    closed: Cell<bool>,
    closing: watch::Sender<bool>,
}

/// golden.ts:208. One per host; shared by `Rc` among the sweeps that hand it machines.
pub struct GoneWatch {
    inner: Rc<Inner>,
}

impl GoneWatch {
    pub fn new(opts: GoneWatchOptions) -> Self {
        let warn: Warn = opts.warn.unwrap_or_else(|| Arc::new(|line: &str| eprintln!("{line}")));
        let clock: Arc<dyn RetryClock> = opts.clock.unwrap_or_else(|| Arc::new(RealRetryClock));
        let (closing, _) = watch::channel(false);
        GoneWatch {
            inner: Rc::new(Inner {
                confirm: opts.confirm,
                retry_ms: opts.retry_ms.unwrap_or(GONE_RETRY_MS),
                warn,
                clock,
                watching: RefCell::new(IndexMap::new()),
                closed: Cell::new(false),
                closing,
            }),
        }
    }

    /// golden.ts:215. The first kill is asked here and its failure is the caller's, a 404 aside; the read-back after it is
    /// the watch's, unless the watch is closed or already holds the machine.
    pub async fn stop(&self, backend: Arc<dyn MachineBackend>, machine: Arc<dyn Machine>) -> Result<()> {
        if let Err(e) = machine.kill().await {
            if !is_missing(&e) {
                return Err(e);
            }
        }
        let id = machine.id().to_string();
        if self.inner.closed.get() || self.inner.watching.borrow().contains_key(&id) {
            return Ok(());
        }
        let inner = self.inner.clone();
        let key = id.clone();
        let task: Watched = async move {
            inner.until_gone(&*backend, &*machine).await;
            inner.watching.borrow_mut().shift_remove(&key);
        }
        .boxed_local()
        .shared();
        self.inner.watching.borrow_mut().insert(id, task.clone());
        let _ = try_spawn_local(task);
        Ok(())
    }

    /// golden.ts:228. Machines asked to stop that have not yet read gone.
    pub fn ids(&self) -> Vec<String> {
        self.inner.watching.borrow().keys().cloned().collect()
    }

    /// golden.ts:233. Resolves once every machine watched now has read gone, been given up on, or had its round cut by
    /// `close`; a request already sent when `close` ran still lands.
    pub async fn settled(&self) {
        let held: Vec<Watched> = self.inner.watching.borrow().values().cloned().collect();
        futures::future::join_all(held).await;
    }

    /// golden.ts:237.
    pub fn close(&self) {
        self.inner.closed.set(true);
        self.inner.closing.send_replace(true);
    }
}

impl Inner {
    /// golden.ts:244.
    async fn until_gone(&self, backend: &dyn MachineBackend, machine: &dyn Machine) {
        let id = machine.id();
        let live = || !self.closed.get();
        let mut asked = 0u32;
        while live() {
            let round = WatchRound { asked: asked == 0, live: &live };
            match read_back_gone(backend, machine, self.confirm, &*self.clock, Some(&round)).await {
                Ok(gone) => {
                    if gone && asked > 0 {
                        (self.warn)(&format!("machine {id} read gone after asking again"));
                    }
                    return;
                }
                Err(e) if is_account_refusal(&e) => {
                    (self.warn)(&format!("{e}; not asked again, the next start's sweep stops machine {id}"));
                    return;
                }
                Err(e) => (self.warn)(&format!("{e}; asking again in {} s", (self.retry_ms as f64 / 1000.0).round())),
            }
            if !live() {
                return;
            }
            let mut closing = self.closing.subscribe();
            let woken = closing.wait_for(|closed| *closed);
            futures::pin_mut!(woken);
            select(self.clock.sleep(self.retry_ms), woken).await;
            asked += 1;
        }
    }
}
