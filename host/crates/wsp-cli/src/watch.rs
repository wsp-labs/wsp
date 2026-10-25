// SPDX-License-Identifier: AGPL-3.0-only
//! A list that refreshes where it stands, for every line that takes --watch. Port of `packages/host/src/watch.ts` at
//! 65f6b89c7; the spec is `spec/layout.md`.
use std::sync::Arc;

use futures::future::BoxFuture;
use wsp_words::exit::HostError;

/// watch.ts:14.
pub const WATCH_EVERY_MS: u64 = 1_000;

/// watch.ts:18-19.
pub const HIDE_CURSOR: &str = "\x1b[?25l";
pub const SHOW_CURSOR: &str = "\x1b[?25h";

/// watch.ts:23, in order.
pub const WATCH_STOP_SIGNALS: [&str; 3] = ["SIGINT", "SIGTERM", "SIGHUP"];

/// A listener a watch hangs on a signal: the same one for all three.
pub type StopListener = Arc<dyn Fn() + Send + Sync>;

/// watch.ts:27-30: where a watch's stop arrives. The process by default (`ProcessSignals`); a test hands in its own.
/// `on` and `off` are called once per signal in `WATCH_STOP_SIGNALS`, with the same listener each time.
pub trait WatchSignals: Send + Sync {
    fn on(&self, signal: &'static str, listener: StopListener);
    fn off(&self, signal: &'static str, listener: &StopListener);
}

/// The process's own SIGINT, SIGTERM and SIGHUP.
pub struct ProcessSignals;

impl WatchSignals for ProcessSignals {
    fn on(&self, signal: &'static str, listener: StopListener) {
        todo!()
    }
    fn off(&self, signal: &'static str, listener: &StopListener) {
        todo!()
    }
}

/// watch.ts:34-39: raw writes and the width to count wrapped rows at, off one stream.
pub trait Redraw: Send + Sync {
    /// Raw text, no newline added.
    fn write(&self, text: &str);
    /// The stream's width as it is at this moment.
    fn columns(&self) -> u16;
}

/// watch.ts:41-48.
pub struct WatchDeps {
    pub redraw: Arc<dyn Redraw>,
    /// `None` is `WATCH_EVERY_MS`.
    pub every_ms: Option<u64>,
    /// `None` is `ProcessSignals`.
    pub signals: Option<Arc<dyn WatchSignals>>,
    /// The next reading arriving, raced with the tick; called once per wait.
    pub until: Option<Arc<dyn Fn() -> BoxFuture<'static, ()> + Send + Sync>>,
}

/// The rows of one frame, or the refusal that ends the watch.
pub type Lines = Arc<dyn Fn() -> BoxFuture<'static, Result<Vec<String>, HostError>> + Send + Sync>;

/// watch.ts:53-94.
pub async fn watch_block(lines: Lines, deps: WatchDeps) -> Result<(), HostError> {
    todo!()
}

/// watch.ts:99-103: where a --watch may redraw, or the refusal that says why not.
pub enum WatchOn {
    Redraw(Arc<dyn Redraw>),
    Refusal(HostError),
}

/// watch.ts:99-103. `--json` is refused first, then no terminal.
pub fn watch_on(words: &str, json: bool, redraw: Option<Arc<dyn Redraw>>) -> WatchOn {
    todo!()
}
