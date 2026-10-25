// SPDX-License-Identifier: AGPL-3.0-only
//! Unit lazy_slates: lazy-slates.ts at 65f6b89c7. Spec: spec/lazy_slates.md.
use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use indexmap::IndexMap;
use serde_json::Value;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

use crate::pending::wire::{SlateEventAnswer, SlateReadAnswer, SlateStateAnswer, SlateView, SlateWriteAnswer, SlatesImageAnswer};
use crate::slates::{ApproveParams, CatalogParams, EventParams, ReadParams, Slates, SlatesDeps, StateParams, WriteParams};

// lazy-slates.ts:6
// /** The collection each thread's slate record is kept in, read here at start without loading the slate code. */
// export const SLATES = "slates";
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

// lazy-slates.ts:9
// export function lazySlates(deps: SlatesDeps): Slates {
//   let real: Slates | undefined;
//   let loading: Promise<Slates> | undefined;
//   const load = (): Promise<Slates> =>
//     (loading ??= (async () => {
//       const at = performance.now();
//       const m = await import("./slates.js");
//       real = m.createSlates(deps);
//       console.log(`the slate's code loaded in ${Math.round(performance.now() - at)} ms`);
//       return real;
//     })());
//   /** Once loaded, a call goes straight through, and while loading it waits its turn behind the calls before it, so a
//    * turn's snapshot is taken before the next change lands as it was when nothing was lazy. */
//   const used = <T>(f: (s: Slates) => T | Promise<T>): Promise<T> => (real !== undefined ? Promise.resolve(f(real)) : load().then(f));
//   let stored: Promise<boolean> | undefined;
//   /** As used, but while no slate was stored at start and none was used since, it answers none and loads nothing. */
//   const ifAny = async <T>(f: (s: Slates) => T | Promise<T>, none: T): Promise<T> => {
//     if (loading !== undefined) return used(f);
//     return (await (stored ??= deps.store.list(SLATES).then(list => list.length > 0))) || loading !== undefined ? used(f) : none;
//   };
//   return {
//     ready: () => ifAny(s => s.ready(), undefined),
//     settled: async () => void (await (await loading)?.settled()),
//     close: () => real?.close(),
//     get: threadId => ifAny(s => s.get(threadId), null),
//     write: (p, caller) => used(s => s.write(p, caller)),
//     state: (p, caller) => used(s => s.state(p, caller)),
//     read: (p, caller) => used(s => s.read(p, caller)),
//     catalog: (p, caller) => used(s => s.catalog(p, caller)),
//     shown: threadId => used(s => s.shown(threadId)),
//     revoke: p => used(s => s.revoke(p)),
//     event: p => used(s => s.event(p)),
//     approve: p => used(s => s.approve(p)),
//     cancel: p => used(s => s.cancel(p)),
//     resolve: p => used(s => s.resolve(p)),
//     image: p => used(s => s.image(p)),
//     subscribe: p => {
//       if (real !== undefined) return real.subscribe(p);
//       let release: (() => void) | undefined;
//       let released = false;
//       void load().then(s => {
//         if (!released) release = s.subscribe(p);
//       });
//       return () => {
//         released = true;
//         release?.();
//       };
//     },
//     turnEnded: o => ifAny(s => s.turnEnded(o), undefined),
//     rewound: o => ifAny(s => s.rewound(o), undefined),
//     forget: (threadId, o) => ifAny(s => s.forget(threadId, o), undefined),
//     closeServers: threadId => real?.closeServers(threadId),
//     watchesPr: workspaceId => real?.watchesPr(workspaceId) ?? false,
//     napped: workspaceId => real?.napped(workspaceId),
//   };
// }
pub fn lazy_slates(deps: Rc<dyn SlatesDeps>, create: SlatesFactory) -> Rc<dyn Slates> {
    todo!()
}

/// The wrapper as the runtime holds it.
pub struct LazyArea {
    pub(crate) inner: Rc<LazySlates>,
}

// lazy-slates.ts:31
//     settled: async () => void (await (await loading)?.settled()),
//     close: () => real?.close(),
//     get: threadId => ifAny(s => s.get(threadId), null),
//     write: (p, caller) => used(s => s.write(p, caller)),
//     state: (p, caller) => used(s => s.state(p, caller)),
//     read: (p, caller) => used(s => s.read(p, caller)),
//     catalog: (p, caller) => used(s => s.catalog(p, caller)),
//     shown: threadId => used(s => s.shown(threadId)),
//     revoke: p => used(s => s.revoke(p)),
//     event: p => used(s => s.event(p)),
//     approve: p => used(s => s.approve(p)),
//     cancel: p => used(s => s.cancel(p)),
//     resolve: p => used(s => s.resolve(p)),
//     image: p => used(s => s.image(p)),
//     subscribe: p => {
//       if (real !== undefined) return real.subscribe(p);
//       let release: (() => void) | undefined;
//       let released = false;
//       void load().then(s => {
//         if (!released) release = s.subscribe(p);
//       });
//       return () => {
//         released = true;
//         release?.();
//       };
//     },
//     turnEnded: o => ifAny(s => s.turnEnded(o), undefined),
//     rewound: o => ifAny(s => s.rewound(o), undefined),
//     forget: (threadId, o) => ifAny(s => s.forget(threadId, o), undefined),
//     closeServers: threadId => real?.closeServers(threadId),
//     watchesPr: workspaceId => real?.watchesPr(workspaceId) ?? false,
//     napped: workspaceId => real?.napped(workspaceId),
#[async_trait(?Send)]
impl Slates for LazyArea {
    async fn get(&self, thread_id: &str) -> Result<Option<SlateView>, HostError> {
        todo!()
    }
    async fn write(&self, p: WriteParams, caller: Option<&Caller>) -> Result<SlateWriteAnswer, HostError> {
        todo!()
    }
    async fn state(&self, p: StateParams, caller: Option<&Caller>) -> Result<SlateStateAnswer, HostError> {
        todo!()
    }
    async fn read(&self, p: ReadParams, caller: Option<&Caller>) -> Result<SlateReadAnswer, HostError> {
        todo!()
    }
    async fn catalog(&self, p: CatalogParams, caller: Option<&Caller>) -> Result<String, HostError> {
        todo!()
    }
    async fn shown(&self, thread_id: &str) -> Result<(), HostError> {
        todo!()
    }
    async fn revoke(&self, thread_id: &str, key: &str) -> Result<(), HostError> {
        todo!()
    }
    async fn event(&self, p: EventParams) -> Result<SlateEventAnswer, HostError> {
        todo!()
    }
    async fn approve(&self, p: ApproveParams) -> Result<(), HostError> {
        todo!()
    }
    async fn cancel(&self, thread_id: &str, run: &str) -> Result<(), HostError> {
        todo!()
    }
    fn subscribe(&self, thread_id: &str, sources: &[String]) -> Rc<dyn Fn()> {
        todo!()
    }
    async fn resolve(&self, thread_id: &str, paths: &[String]) -> Result<IndexMap<String, Value>, HostError> {
        todo!()
    }
    async fn image(&self, thread_id: &str, src: &str, have: Option<&str>) -> Result<SlatesImageAnswer, HostError> {
        todo!()
    }
    async fn turn_ended(&self, thread_id: &str, turn_id: &str) -> Result<(), HostError> {
        todo!()
    }
    async fn rewound(&self, thread_id: &str, turn_id: &str, cut: &[String]) -> Result<(), HostError> {
        todo!()
    }
    async fn forget(&self, thread_id: &str, keep: bool) -> Result<(), HostError> {
        todo!()
    }
    fn close_servers(&self, thread_id: &str) {
        todo!()
    }
    fn watches_pr(&self, workspace_id: &str) -> bool {
        todo!()
    }
    fn napped(&self, workspace_id: &str) {
        todo!()
    }
    async fn ready(&self) -> Result<(), HostError> {
        todo!()
    }
    async fn settled(&self) {
        todo!()
    }
    fn close(&self) {
        todo!()
    }
}
