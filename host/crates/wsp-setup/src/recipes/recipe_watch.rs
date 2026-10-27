// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `recipe-watch.ts`: noticing a change on this computer to something a followed recipe holds.
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use serde_json::Value;
use wsp_wire::types::recipe_file::RecipeFile;

/// `WatchFn` (`recipe-watch.ts:16`), fs.watch as the watcher uses it.
pub type WatchFn = Rc<dyn Fn(&str, WatchOptions, Rc<dyn Fn(&str, Option<&str>)>) -> Box<dyn WatchHandle>>;

/// The `{ recursive }` argument of `WatchFn` (`recipe-watch.ts:16`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WatchOptions {
    pub recursive: bool,
}

/// The handle `WatchFn` returns (`recipe-watch.ts:16`).
pub trait WatchHandle {
    /// `close` (`recipe-watch.ts:16`).
    fn close(&self);

    /// `on` (`recipe-watch.ts:16`); its TS return is unknown and is not read.
    fn on(&self, _event: WatchEvent, _listener: Rc<dyn Fn()>) -> Value;
}

/// The `"error" | "close"` of `WatchHandle.on` (`recipe-watch.ts:16`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchEvent {
    Error,
    Close,
}

/// `WatchTimers` (`recipe-watch.ts:19`), the timer the watcher arms, injected in tests.
pub trait WatchTimers {
    /// `set` (`recipe-watch.ts:20`).
    fn set(&self, _f: Rc<dyn Fn()>, _ms: u64) -> Box<dyn TimerHandle>;

    /// `clear` (`recipe-watch.ts:21`).
    fn clear(&self, _timer: &dyn TimerHandle);
}

/// The timer `WatchTimers.set` returns (`recipe-watch.ts:20`), whose `unref` is optional in the TS.
pub trait TimerHandle {
    fn unref(&self) -> Value;
}

/// The `{ slug, file }` of `RecipeWatchOptions.followed` (`recipe-watch.ts:27`).
#[derive(Clone, Debug, PartialEq)]
pub struct FollowedRecipe {
    pub slug: String,
    pub file: RecipeFile,
}

/// `RecipeWatchOptions` (`recipe-watch.ts:24`).
#[derive(Clone)]
pub struct RecipeWatchOptions {
    pub home: String,
    pub followed: Rc<dyn Fn() -> LocalBoxFuture<'static, Vec<FollowedRecipe>>>,
    pub changed: Rc<dyn Fn(&[String])>,
    pub versions: Option<Rc<dyn Fn() -> LocalBoxFuture<'static, Value>>>,
    pub watch: Option<WatchFn>,
    pub timers: Option<Rc<dyn WatchTimers>>,
    pub debounce_ms: Option<u64>,
    pub versions_every_ms: Option<u64>,
}

/// `WATCH_DEBOUNCE_MS` (`recipe-watch.ts:40`).
pub const WATCH_DEBOUNCE_MS: u64 = 2_000;

/// `VERSIONS_EVERY_MS` (`recipe-watch.ts:41`).
pub const VERSIONS_EVERY_MS: u64 = 24 * 60 * 60_000;

/// The `{ path, recursive }` of `recipeWatch`'s `watching` (`recipe-watch.ts:65`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchedPath {
    pub path: String,
    pub recursive: bool,
}

/// The object `recipeWatch` returns (`recipe-watch.ts:65`).
pub struct RecipeWatch;

impl RecipeWatch {
    /// `refresh` (`recipe-watch.ts:65`).
    pub async fn refresh(&self) {
        todo!()
    }

    /// `close` (`recipe-watch.ts:65`).
    pub fn close(&self) {
        todo!()
    }

    /// `watching` (`recipe-watch.ts:65`).
    pub fn watching(&self) -> Vec<WatchedPath> {
        todo!()
    }
}

/// `recipeWatch` (`recipe-watch.ts:65`).
pub fn recipe_watch(_o: RecipeWatchOptions) -> RecipeWatch {
    todo!()
}
