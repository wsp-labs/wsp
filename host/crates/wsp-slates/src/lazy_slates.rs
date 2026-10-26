// SPDX-License-Identifier: AGPL-3.0-only
//! Unit lazy_slates: lazy-slates.ts at 65f6b89c7. Spec: spec/lazy_slates.md.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::Value;
use tokio::task::spawn_local;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

use crate::pending::wire::{SlateEventAnswer, SlateReadAnswer, SlateStateAnswer, SlateView, SlateWriteAnswer, SlatesImageAnswer};
use crate::slates::{ApproveParams, CatalogParams, EventParams, ReadParams, Slates, SlatesDeps, StateParams, WriteParams};

pub const SLATES: &str = "slates";

/// What builds the real area: `create_slates` in the host, a fake in tests.
pub type SlatesFactory = Box<dyn FnOnce(Rc<dyn SlatesDeps>) -> Rc<dyn Slates>>;

/// The wrapper's state (lazy-slates.ts:10-12, 24).
pub struct LazySlates {
    pub(crate) deps: Rc<dyn SlatesDeps>,
    pub(crate) create: RefCell<Option<SlatesFactory>>,
    pub(crate) real: RefCell<Option<Rc<dyn Slates>>>,
    pub(crate) loading: RefCell<Option<Shared<LocalBoxFuture<'static, ()>>>>,
    pub(crate) stored: RefCell<Option<Shared<LocalBoxFuture<'static, Result<bool, HostError>>>>>,
}

pub fn lazy_slates(deps: Rc<dyn SlatesDeps>, create: SlatesFactory) -> Rc<dyn Slates> {
    Rc::new(LazyArea {
        inner: Rc::new(LazySlates {
            deps,
            create: RefCell::new(Some(create)),
            real: RefCell::new(None),
            loading: RefCell::new(None),
            stored: RefCell::new(None),
        }),
    })
}

/// The wrapper as the runtime holds it.
pub struct LazyArea {
    pub(crate) inner: Rc<LazySlates>,
}

fn real(this: &LazySlates) -> Option<Rc<dyn Slates>> {
    this.real.borrow().clone()
}

/// The making runs on the local set whether or not anyone waits for it.
fn load(this: &Rc<LazySlates>) -> Shared<LocalBoxFuture<'static, ()>> {
    let existing = this.loading.borrow().clone();
    if let Some(loading) = existing {
        return loading;
    }
    let me = this.clone();
    let making: LocalBoxFuture<'static, ()> = async move {
        let at = Instant::now();
        let create = me.create.borrow_mut().take().expect("the area is made once");
        let area = create(me.deps.clone());
        *me.real.borrow_mut() = Some(area);
        println!("the slate's code loaded in {} ms", at.elapsed().as_millis());
    }
    .boxed_local();
    let loading = making.shared();
    *this.loading.borrow_mut() = Some(loading.clone());
    spawn_local(loading.clone());
    loading
}

async fn area(this: &Rc<LazySlates>) -> Rc<dyn Slates> {
    if let Some(s) = real(this) {
        return s;
    }
    load(this).await;
    real(this).expect("the area is made once loading ends")
}

fn loading_started(this: &LazySlates) -> bool {
    this.loading.borrow().is_some()
}

async fn stored(this: &Rc<LazySlates>) -> Result<bool, HostError> {
    let read = {
        let mut slot = this.stored.borrow_mut();
        slot.get_or_insert_with(|| {
            let store = this.deps.store();
            async move { store.list(SLATES).await.map(|list| !list.is_empty()) }.boxed_local().shared()
        })
        .clone()
    };
    read.await
}

async fn if_any(this: &Rc<LazySlates>) -> Result<Option<Rc<dyn Slates>>, HostError> {
    if loading_started(this) {
        return Ok(Some(area(this).await));
    }
    let has = stored(this).await?;
    if has || loading_started(this) {
        Ok(Some(area(this).await))
    } else {
        Ok(None)
    }
}

#[async_trait(?Send)]
impl Slates for LazyArea {
    async fn get(&self, thread_id: &str) -> Result<Option<SlateView>, HostError> {
        match if_any(&self.inner).await? {
            Some(s) => s.get(thread_id).await,
            None => Ok(None),
        }
    }
    async fn write(&self, p: WriteParams, caller: Option<&Caller>) -> Result<SlateWriteAnswer, HostError> {
        area(&self.inner).await.write(p, caller).await
    }
    async fn state(&self, p: StateParams, caller: Option<&Caller>) -> Result<SlateStateAnswer, HostError> {
        area(&self.inner).await.state(p, caller).await
    }
    async fn read(&self, p: ReadParams, caller: Option<&Caller>) -> Result<SlateReadAnswer, HostError> {
        area(&self.inner).await.read(p, caller).await
    }
    async fn catalog(&self, p: CatalogParams, caller: Option<&Caller>) -> Result<String, HostError> {
        area(&self.inner).await.catalog(p, caller).await
    }
    async fn shown(&self, thread_id: &str) -> Result<(), HostError> {
        area(&self.inner).await.shown(thread_id).await
    }
    async fn revoke(&self, thread_id: &str, key: &str) -> Result<(), HostError> {
        area(&self.inner).await.revoke(thread_id, key).await
    }
    async fn event(&self, p: EventParams) -> Result<SlateEventAnswer, HostError> {
        area(&self.inner).await.event(p).await
    }
    async fn approve(&self, p: ApproveParams) -> Result<(), HostError> {
        area(&self.inner).await.approve(p).await
    }
    async fn cancel(&self, thread_id: &str, run: &str) -> Result<(), HostError> {
        area(&self.inner).await.cancel(thread_id, run).await
    }
    fn subscribe(&self, thread_id: &str, sources: &[String]) -> Rc<dyn Fn()> {
        if let Some(s) = real(&self.inner) {
            return s.subscribe(thread_id, sources);
        }
        let released = Rc::new(Cell::new(false));
        let release: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let (inner, thread, sources) = (self.inner.clone(), thread_id.to_string(), sources.to_vec());
        let loading = load(&self.inner);
        {
            let (released, release) = (released.clone(), release.clone());
            spawn_local(async move {
                loading.await;
                if !released.get() {
                    if let Some(s) = real(&inner) {
                        let held = s.subscribe(&thread, &sources);
                        *release.borrow_mut() = Some(held);
                    }
                }
            });
        }
        Rc::new(move || {
            released.set(true);
            let held = release.borrow_mut().take();
            if let Some(f) = held {
                f();
            }
        })
    }
    async fn resolve(&self, thread_id: &str, paths: &[String]) -> Result<IndexMap<String, Value>, HostError> {
        area(&self.inner).await.resolve(thread_id, paths).await
    }
    async fn image(&self, thread_id: &str, src: &str, have: Option<&str>) -> Result<SlatesImageAnswer, HostError> {
        area(&self.inner).await.image(thread_id, src, have).await
    }
    async fn turn_ended(&self, thread_id: &str, turn_id: &str) -> Result<(), HostError> {
        match if_any(&self.inner).await? {
            Some(s) => s.turn_ended(thread_id, turn_id).await,
            None => Ok(()),
        }
    }
    async fn rewound(&self, thread_id: &str, turn_id: &str, cut: &[String]) -> Result<(), HostError> {
        match if_any(&self.inner).await? {
            Some(s) => s.rewound(thread_id, turn_id, cut).await,
            None => Ok(()),
        }
    }
    async fn forget(&self, thread_id: &str, keep: bool) -> Result<(), HostError> {
        match if_any(&self.inner).await? {
            Some(s) => s.forget(thread_id, keep).await,
            None => Ok(()),
        }
    }
    fn close_servers(&self, thread_id: &str) {
        if let Some(s) = real(&self.inner) {
            s.close_servers(thread_id);
        }
    }
    fn watches_pr(&self, workspace_id: &str) -> bool {
        real(&self.inner).is_some_and(|s| s.watches_pr(workspace_id))
    }
    fn napped(&self, workspace_id: &str) {
        if let Some(s) = real(&self.inner) {
            s.napped(workspace_id);
        }
    }
    async fn ready(&self) -> Result<(), HostError> {
        match if_any(&self.inner).await? {
            Some(s) => s.ready().await,
            None => Ok(()),
        }
    }
    async fn settled(&self) {
        let loading = self.inner.loading.borrow().clone();
        if let Some(loading) = loading {
            loading.await;
        }
        if let Some(s) = real(&self.inner) {
            s.settled().await;
        }
    }
    fn close(&self) {
        if let Some(s) = real(&self.inner) {
            s.close();
        }
    }
}
