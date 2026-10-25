// SPDX-License-Identifier: AGPL-3.0-only
//! A pty on a builder or a box shown in this terminal the way ssh would, the same pty watched with nobody at the
//! terminal, and one command run quietly. Only a printed URL is read from the stream. Port of
//! `packages/host/src/signin-relay.ts` at 65f6b89c7; the spec is `spec/signin_relay.md`.
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::BoxFuture;
use serde_json::{Map, Value};
use wsp_catalog::signin::Question;
use wsp_words::exit::HostError;

/// A frame or a reply as the daemon carries it.
pub type Frame = Map<String, Value>;

/// A daemon event listener; detaching is the `Detach` the link hands back.
pub type EventSink = Arc<dyn Fn(&Frame) + Send + Sync>;

/// Takes a listener back off; called at most once.
pub struct Detach(pub Box<dyn FnOnce() + Send>);

impl Detach {
    pub fn detach(self) {
        (self.0)()
    }
}

/// `PtyLink` (runtime agents-read.ts:82-90): ops answered with the daemon's reply, events as they arrive, a close,
/// and whether the pty is on a computer somebody owns.
#[async_trait]
pub trait PtyLink: Send + Sync {
    /// The daemon's reply to `{...extra, op}`; an `Err` is the link failing the op (a dead socket).
    async fn op(&self, op: &str, extra: Frame) -> Result<Frame, HostError>;
    fn on_event(&self, sink: EventSink) -> Detach;
    /// Settles when the link is gone; `None` for a link that never says. Each call hands a fresh future.
    fn closed(&self) -> Option<BoxFuture<'static, ()>>;
    /// `bare`: `Some(true)` on a computer somebody owns.
    fn bare(&self) -> Option<bool>;
}

/// signin-relay.ts:18.
pub const BARE_BASH: &str = "exec bash --noprofile --norc";

/// Raw mode on the local terminal's input, where it is a terminal (signin-relay.ts:24).
pub trait RawMode: Send {
    fn is_raw(&self) -> bool;
    fn set_raw_mode(&mut self, on: bool);
}

/// What the local terminal's output knows of itself (signin-relay.ts:25).
pub trait RelayOutput: Send + Sync {
    fn write(&self, text: &str);
    fn columns(&self) -> Option<u16>;
    fn rows(&self) -> Option<u16>;
}

/// signin-relay.ts:23-26: the keystrokes as they arrive (each chunk decoded as UTF-8), raw mode where the input is a
/// terminal (`None` off a tty), the output, and one `()` per resize the output saw.
pub struct RelayTerminal {
    pub input: tokio::sync::mpsc::UnboundedReceiver<String>,
    pub raw: Option<Box<dyn RawMode>>,
    pub output: Arc<dyn RelayOutput>,
    pub resized: tokio::sync::mpsc::UnboundedReceiver<()>,
}

/// signin-relay.ts:28-48.
pub struct RelayOptions {
    pub link: Arc<dyn PtyLink>,
    pub command: Option<String>,
    pub open: Arc<dyn Fn(String) -> BoxFuture<'static, bool> + Send + Sync>,
    pub callback_url: Option<Arc<dyn Fn() -> Option<String> + Send + Sync>>,
    pub on_consent: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    pub env: Option<indexmap::IndexMap<String, String>>,
    pub timeout_ms: u64,
    /// `None` is `FLUSH_MS`.
    pub flush_ms: Option<u64>,
    /// `Date.now()` in milliseconds; `None` is the clock.
    pub now: Option<Arc<dyn Fn() -> f64 + Send + Sync>>,
}

/// signin-relay.ts:50-60. Serialized `exitCode, timedOut, dropped, urls, opened`.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayOutcome {
    pub exit_code: i64,
    pub timed_out: bool,
    pub dropped: bool,
    pub urls: u32,
    pub opened: u32,
}

/// signin-relay.ts:64.
pub const OFFER_MS: f64 = 60_000.0;
/// signin-relay.ts:65.
pub const OPENED_PRINTED: &str = "opened on this computer; if the page shows a code, paste it into the terminal above";
/// signin-relay.ts:66.
pub const OPENED_PAGE: &str = "opened the sign-in page; it returns to the machine on its own";
/// signin-relay.ts:258.
pub const OPEN_FAILED: &str = "could not open a browser here; use the link above";
/// signin-relay.ts:67.
pub const FLUSH_MS: u64 = 300;
/// signin-relay.ts:71: the tail a scanner keeps, in UTF-16 units.
pub const TAIL_CHARS: usize = 2048;

/// signin-relay.ts:75-77.
pub fn strip_osc8(text: &str) -> String {
    todo!()
}

/// signin-relay.ts:80-89.
pub fn urls_in(text: &str) -> Vec<String> {
    todo!()
}

/// signin-relay.ts:92-109.
#[derive(Debug, Default)]
pub struct UrlScanner {
    tail: String,
    seen: std::collections::HashSet<String>,
}

impl UrlScanner {
    pub fn new() -> UrlScanner {
        UrlScanner::default()
    }
    /// signin-relay.ts:95-99.
    pub fn feed(&mut self, chunk: &str) -> Vec<String> {
        todo!()
    }
    /// signin-relay.ts:101-103.
    pub fn flush(&mut self) -> Vec<String> {
        todo!()
    }
}

/// signin-relay.ts:112-116.
#[derive(Clone, Debug, PartialEq)]
pub struct Asked {
    pub question: Question,
    pub matched: String,
}

/// signin-relay.ts:120-138. Questions are told apart by their place in the list.
#[derive(Debug)]
pub struct QuestionScanner {
    questions: Vec<Question>,
    tail: String,
    seen: Vec<bool>,
}

impl QuestionScanner {
    pub fn new(questions: Vec<Question>) -> QuestionScanner {
        let seen = vec![false; questions.len()];
        QuestionScanner { questions, tail: String::new(), seen }
    }
    /// signin-relay.ts:124-137.
    pub fn feed(&mut self, chunk: &str) -> Vec<Asked> {
        todo!()
    }
}

/// signin-relay.ts:140-142.
pub fn hyperlink(url: &str) -> String {
    todo!()
}

/// signin-relay.ts:160.
pub const TOOL_STARTS: &str = "\x1e";

/// signin-relay.ts:163, byte for byte.
pub const STAGE: &str = r#"umask 077 && d=$(mktemp -d "${TMPDIR:-/tmp}/wsp-line.XXXXXX") && cat > "$d/line" && printf '%s' "$d""#;

/// signin-relay.ts:167-172.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Staged {
    pub dir: String,
    /// `shellQuote("<dir>/line")`.
    pub file: String,
}

/// signin-relay.ts:174-181: the command staged in a folder only the daemon's login reads.
pub async fn stage(link: &dyn PtyLink, command: &str) -> Result<Staged, HostError> {
    todo!()
}

/// signin-relay.ts:180: `rm -rf -- <dir>` on the link, any failure swallowed.
pub async fn clear_staged(link: &dyn PtyLink, staged: &Staged) {
    todo!()
}

/// signin-relay.ts:185-187.
pub fn shell_line(staged: &Staged) -> String {
    todo!()
}

/// signin-relay.ts:202-300.
pub async fn relay_pty(o: RelayOptions, terminal: &mut RelayTerminal) -> Result<RelayOutcome, HostError> {
    todo!()
}

/// signin-relay.ts:302-310. Serialized `exitCode, timedOut, dropped, stopped`.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchOutcome {
    pub exit_code: i64,
    pub timed_out: bool,
    pub dropped: bool,
    pub stopped: bool,
}

/// What `on_typing` is handed: the pty's input while it is attached.
pub type PtyTyping = Arc<dyn Fn(String) -> BoxFuture<'static, Result<(), HostError>> + Send + Sync>;

/// signin-relay.ts:312-340.
pub struct WatchOptions {
    pub link: Arc<dyn PtyLink>,
    pub command: String,
    pub on_data: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    pub on_url: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    pub on_scanned: Option<Arc<dyn Fn() + Send + Sync>>,
    pub env: Option<indexmap::IndexMap<String, String>>,
    pub questions: Vec<Question>,
    pub on_question: Option<Arc<dyn Fn(&Asked) + Send + Sync>>,
    pub on_typing: Option<Arc<dyn Fn(Option<PtyTyping>) + Send + Sync>>,
    pub timeout_ms: u64,
    pub stop: Option<BoxFuture<'static, ()>>,
    pub flush_ms: Option<u64>,
}

/// signin-relay.ts:347-409.
pub async fn watch_pty(o: WatchOptions) -> Result<WatchOutcome, HostError> {
    todo!()
}

/// signin-relay.ts:411-417. Serialized `output, exitCode, timedOut, dropped`.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuietRun {
    pub output: String,
    pub exit_code: i64,
    pub timed_out: bool,
    pub dropped: bool,
}

/// signin-relay.ts:419.
pub const STATUS_MARK: &str = "WSP_STATUS";

/// signin-relay.ts:424-470.
pub async fn run_quiet(
    link: &dyn PtyLink,
    command: &str,
    timeout_ms: u64,
    env: &indexmap::IndexMap<String, String>,
) -> Result<QuietRun, HostError> {
    todo!()
}
