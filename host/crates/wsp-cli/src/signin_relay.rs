// SPDX-License-Identifier: AGPL-3.0-only
//! A pty on a builder or a box shown in this terminal the way ssh would, the same pty watched with nobody at the
//! terminal, and one command run quietly. Only a printed URL is read from the stream. Port of
//! `packages/host/src/signin-relay.ts` at 65f6b89c7; the spec is `spec/signin_relay.md`.
use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex, Once, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use base64::Engine;
use futures::future::BoxFuture;
use indexmap::IndexMap;
use regex::Regex;
use serde_json::{json, Map, Value};
use tokio::sync::{mpsc::UnboundedReceiver, oneshot};
use tokio::task::JoinHandle;
use wsp_catalog::signin::{Question, Settle};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_words::agents_report::has_control_char;
use wsp_words::exit::HostError;
use wsp_words::js::{js_trim, utf16};
use wsp_words::words::base::last_line;

use crate::hosts::Env;
use crate::init_layout::{strip_vt, style_text, styles_on};

pub use crate::verbs::client::Frame;

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

/// This process's stdin and stdout as a `RelayTerminal`: keystrokes from a thread, raw mode where stdin is a terminal,
/// the window size where stdout is one, and one `()` per SIGWINCH only while stdout is a terminal.
pub fn process_terminal() -> RelayTerminal {
    let (keys, input) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || read_keys(keys));
    let raw: Option<Box<dyn RawMode>> =
        std::io::stdin().is_terminal().then(|| Box::new(TtyRaw { saved: None, on: false }) as Box<dyn RawMode>);
    let (resize_to, resized) = tokio::sync::mpsc::unbounded_channel();
    if std::io::stdout().is_terminal() {
        watch_resizes();
        *RESIZE_TO.lock().unwrap() = Some(resize_to);
    }
    RelayTerminal { input, raw, output: Arc::new(StdoutOutput), resized }
}

fn interrupted() -> bool {
    std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted
}

/// Polls stdin so that a relay which has gone drops its receiver stops the read without taking one more keystroke.
fn read_keys(keys: tokio::sync::mpsc::UnboundedSender<String>) {
    let mut buf = [0u8; 4096];
    while !keys.is_closed() {
        let mut fd = libc::pollfd { fd: 0, events: libc::POLLIN, revents: 0 };
        match unsafe { libc::poll(&mut fd, 1, 100) } {
            0 => continue,
            n if n < 0 && interrupted() => continue,
            n if n < 0 => return,
            _ => {}
        }
        let n = unsafe { libc::read(0, buf.as_mut_ptr().cast(), buf.len()) };
        if n < 0 && interrupted() {
            continue;
        }
        if n <= 0 {
            return;
        }
        let text = String::from_utf8_lossy(&buf[..n as usize]).into_owned();
        if keys.send(text).is_err() {
            return;
        }
    }
}

/// Raw mode on this process's stdin; turning it off restores the modes it had when turned on.
struct TtyRaw {
    saved: Option<libc::termios>,
    on: bool,
}

impl RawMode for TtyRaw {
    fn is_raw(&self) -> bool {
        self.on
    }

    fn set_raw_mode(&mut self, on: bool) {
        if on == self.on {
            return;
        }
        if !on {
            if let Some(saved) = self.saved.take() {
                unsafe { libc::tcsetattr(0, libc::TCSANOW, &saved) };
            }
            self.on = false;
            return;
        }
        let mut before: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(0, &mut before) } != 0 {
            return;
        }
        let mut raw = before;
        unsafe { libc::cfmakeraw(&mut raw) };
        // Output processing stays as it was: libuv's raw mode, which the TS relay runs on, leaves OPOST and ONLCR on.
        raw.c_oflag = before.c_oflag;
        if unsafe { libc::tcsetattr(0, libc::TCSANOW, &raw) } == 0 {
            self.saved = Some(before);
            self.on = true;
        }
    }
}

/// This process's stdout: each write is flushed, and the size is read off the terminal when stdout is one.
struct StdoutOutput;

impl RelayOutput for StdoutOutput {
    fn write(&self, text: &str) {
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(text.as_bytes());
        let _ = out.flush();
    }

    fn columns(&self) -> Option<u16> {
        window_size().map(|ws| ws.ws_col)
    }

    fn rows(&self) -> Option<u16> {
        window_size().map(|ws| ws.ws_row)
    }
}

fn window_size() -> Option<libc::winsize> {
    if !std::io::stdout().is_terminal() {
        return None;
    }
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    (unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) } == 0).then_some(ws)
}

/// The relay channel that SIGWINCH sends to; one relay watches the window at a time.
static RESIZE_TO: Mutex<Option<tokio::sync::mpsc::UnboundedSender<()>>> = Mutex::new(None);
/// The write end of the self-pipe the SIGWINCH handler writes one byte to, -1 until the watch is set up.
static RESIZE_PIPE: AtomicI32 = AtomicI32::new(-1);
static WATCH_RESIZES: Once = Once::new();

extern "C" fn on_sigwinch(_signal: libc::c_int) {
    let fd = RESIZE_PIPE.load(Ordering::Relaxed);
    if fd >= 0 {
        unsafe { libc::write(fd, [0u8].as_ptr().cast(), 1) };
    }
}

/// Installs the SIGWINCH handler and the thread that turns its bytes into `()`s, once per process.
fn watch_resizes() {
    WATCH_RESIZES.call_once(|| {
        let mut fds = [0; 2];
        if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
            return;
        }
        let (read_end, write_end) = (fds[0], fds[1]);
        unsafe {
            let flags = libc::fcntl(write_end, libc::F_GETFL);
            libc::fcntl(write_end, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
        RESIZE_PIPE.store(write_end, Ordering::Relaxed);
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = on_sigwinch as extern "C" fn(libc::c_int) as usize;
        action.sa_flags = libc::SA_RESTART;
        unsafe {
            libc::sigemptyset(&mut action.sa_mask);
            libc::sigaction(libc::SIGWINCH, &action, std::ptr::null_mut());
        }
        std::thread::spawn(move || relay_resizes(read_end));
    });
}

fn relay_resizes(read_end: libc::c_int) {
    let mut buf = [0u8; 64];
    loop {
        let n = unsafe { libc::read(read_end, buf.as_mut_ptr().cast(), buf.len()) };
        if n < 0 && interrupted() {
            continue;
        }
        if n <= 0 {
            return;
        }
        if let Some(to) = RESIZE_TO.lock().unwrap().as_ref() {
            for _ in 0..n {
                let _ = to.send(());
            }
        }
    }
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
    osc8_re().replace_all(text, "").into_owned()
}

/// signin-relay.ts:80-89.
pub fn urls_in(text: &str) -> Vec<String> {
    let clean = strip_osc8(text);
    let mut out: Vec<String> = Vec::new();
    for m in url_re().find_iter(&clean) {
        if m.end() == clean.len() {
            continue;
        }
        let url = m.as_str().trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']']);
        if !out.iter().any(|u| u == url) {
            out.push(url.to_string());
        }
    }
    out
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
        let text = format!("{}{chunk}", self.tail);
        self.tail = utf16::tail(&text, TAIL_CHARS);
        let urls = urls_in(&text);
        self.fresh(urls)
    }
    /// signin-relay.ts:101-103.
    pub fn flush(&mut self) -> Vec<String> {
        let urls = urls_in(&format!("{}\n", self.tail));
        self.fresh(urls)
    }
    fn fresh(&mut self, urls: Vec<String>) -> Vec<String> {
        let mut out = Vec::new();
        for u in urls {
            if self.seen.insert(u.clone()) {
                out.push(u);
            }
        }
        out
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
        if self.seen.iter().all(|s| *s) {
            return Vec::new();
        }
        let raw = format!("{}{chunk}", self.tail);
        self.tail = utf16::tail(&raw, TAIL_CHARS);
        let text = strip_vt(&strip_osc8(&raw));
        let mut out = Vec::new();
        for (i, question) in self.questions.iter().enumerate() {
            if self.seen[i] {
                continue;
            }
            if let Some(matched) = question.asks.find(&text) {
                self.seen[i] = true;
                out.push(Asked { question: question.clone(), matched });
            }
        }
        out
    }
}

/// signin-relay.ts:140-142.
pub fn hyperlink(url: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{url}\x1b]8;;\x1b\\")
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
    let stdin = base64::engine::general_purpose::STANDARD.encode(command);
    let reply = link.op("exec", frame_of(json!({"cmd": STAGE, "stdin": stdin}))).await?;
    let dir = reply.get("stdout").and_then(Value::as_str).unwrap_or("").to_string();
    let exited_zero = reply.get("exitCode").and_then(Value::as_i64) == Some(0);
    if !exited_zero || !dir.starts_with('/') || has_control_char(&dir) {
        let said = [reply.get("stderr"), reply.get("error")].into_iter().flatten().find(|v| !v.is_null()).map(text_of).unwrap_or_default();
        let why = last_line(&said).unwrap_or_else(|| "no reason given".into());
        return Err(HostError::new(format!("could not put the command on that computer: {why}")));
    }
    Ok(Staged { file: shell_quote(&format!("{dir}/line")), dir })
}

/// signin-relay.ts:180: `rm -rf -- <dir>` on the link, any failure swallowed.
pub async fn clear_staged(link: &dyn PtyLink, staged: &Staged) {
    let cmd = format!("rm -rf -- {}", shell_quote(&staged.dir));
    let _ = link.op("exec", frame_of(json!({"cmd": cmd}))).await;
}

/// signin-relay.ts:185-187.
pub fn shell_line(staged: &Staged) -> String {
    format!("c=$(< {}); rm -rf -- {}; printf '\\036'; exec bash -c \"$c\"\r", staged.file, shell_quote(&staged.dir))
}

/// signin-relay.ts:202-300.
pub async fn relay_pty(o: RelayOptions, terminal: &mut RelayTerminal) -> Result<RelayOutcome, HostError> {
    let link = o.link.clone();
    let (cols, rows) = (terminal.output.columns().unwrap_or(80), terminal.output.rows().unwrap_or(24));
    let reply = link.op("pty.create", create_frame(cols.into(), rows.into(), link.bare(), o.env.as_ref())).await?;
    let pty_id = pty_id_of(&reply)?;
    let now: Arc<dyn Fn() -> f64 + Send + Sync> = o.now.clone().unwrap_or_else(|| Arc::new(epoch_ms));
    let relay = Arc::new(Relay {
        output: terminal.output.clone(),
        now,
        state: Mutex::new(RelayState {
            outcome: RelayOutcome { exit_code: -1, timed_out: false, dropped: false, urls: 0, opened: 0 },
            offer: None,
            scanner: UrlScanner::new(),
            tool: ToolFilter::new(o.command.is_none()),
            flush: None,
            done: None,
        }),
    });
    let (done_tx, mut done_rx) = oneshot::channel();
    relay.state.lock().unwrap().done = Some(done_tx);
    let flush_ms = o.flush_ms.unwrap_or(FLUSH_MS);
    let sink_relay = relay.clone();
    let sink_pty = pty_id.clone();
    let detach = link.on_event(Arc::new(move |e: &Frame| {
        if e.get("ptyId").and_then(Value::as_str) != Some(sink_pty.as_str()) {
            return;
        }
        match e.get("type").and_then(Value::as_str) {
            Some("pty.data") => sink_relay.on_data(&e.get("data").map_or_else(|| "undefined".to_string(), text_of), flush_ms),
            Some("pty.exit") => sink_relay.on_exit(e.get("exitCode").and_then(Value::as_i64).unwrap_or(-1)),
            _ => {}
        }
    }));
    let closed_task = link.closed().map(|closed| {
        let r = relay.clone();
        tokio::spawn(async move {
            closed.await;
            {
                let mut st = r.state.lock().unwrap();
                if st.outcome.exit_code == -1 {
                    st.outcome.dropped = true;
                }
            }
            r.fire_done();
        })
    });
    let timer = {
        let r = relay.clone();
        let timeout = o.timeout_ms;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(timeout)).await;
            r.state.lock().unwrap().outcome.timed_out = true;
            r.fire_done();
        })
    };
    let ctx = RelayCtx {
        link: link.clone(),
        pty_id: pty_id.clone(),
        relay: relay.clone(),
        open: o.open.clone(),
        callback_url: o.callback_url.clone(),
        on_consent: o.on_consent.clone(),
        cols: cols.into(),
        rows: rows.into(),
    };
    let was_raw = terminal.raw.as_ref().is_some_and(|r| r.is_raw());
    let mut raw_set = false;
    let mut staged: Option<Staged> = None;
    let mut opens: Vec<JoinHandle<()>> = Vec::new();
    let result = relay_session(&ctx, o.command.as_deref(), terminal, &mut done_rx, &mut raw_set, &mut staged, &mut opens).await;
    timer.abort();
    if let Some(h) = closed_task {
        h.abort();
    }
    if let Some(h) = relay.state.lock().unwrap().flush.take() {
        h.abort();
    }
    if raw_set {
        if let Some(raw) = terminal.raw.as_mut() {
            raw.set_raw_mode(was_raw);
        }
    }
    detach.detach();
    let _ = link.op("pty.kill", frame_of(json!({"ptyId": pty_id}))).await;
    if let Some(s) = &staged {
        clear_staged(link.as_ref(), s).await;
    }
    for h in opens {
        let _ = h.await;
    }
    result?;
    let outcome = relay.state.lock().unwrap().outcome.clone();
    Ok(outcome)
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
    let link = o.link.clone();
    let reply = link.op("pty.create", create_frame(200, 50, link.bare(), o.env.as_ref())).await?;
    let pty_id = pty_id_of(&reply)?;
    let (done_tx, mut done_rx) = oneshot::channel();
    let run = Arc::new(WatchRun {
        link: link.clone(),
        pty_id: pty_id.clone(),
        on_data: o.on_data.clone(),
        on_url: o.on_url.clone(),
        on_scanned: o.on_scanned.clone(),
        on_question: o.on_question.clone(),
        flush_ms: o.flush_ms.unwrap_or(FLUSH_MS),
        state: Mutex::new(WatchState {
            ended: false,
            outcome: WatchOutcome { exit_code: -1, timed_out: false, dropped: false, stopped: false },
            scanner: UrlScanner::new(),
            questions: QuestionScanner::new(o.questions.clone()),
            tool: ToolFilter::new(false),
            flush: None,
            done: Some(done_tx),
            tasks: Vec::new(),
        }),
    });
    let sink_run = run.clone();
    let sink_pty = pty_id.clone();
    let detach = link.on_event(Arc::new(move |e: &Frame| {
        if e.get("ptyId").and_then(Value::as_str) != Some(sink_pty.as_str()) {
            return;
        }
        match e.get("type").and_then(Value::as_str) {
            Some("pty.data") => sink_run.on_data(&e.get("data").map_or_else(|| "undefined".to_string(), text_of)),
            Some("pty.exit") => {
                let code = e.get("exitCode").and_then(Value::as_i64).unwrap_or(-1);
                sink_run.end(|out| out.exit_code = code);
            }
            _ => {}
        }
    }));
    let mut tasks = Vec::new();
    if let Some(closed) = link.closed() {
        let r = run.clone();
        tasks.push(tokio::spawn(async move {
            closed.await;
            r.end(|out| out.dropped = true);
        }));
    }
    if let Some(stop) = o.stop {
        let r = run.clone();
        tasks.push(tokio::spawn(async move {
            stop.await;
            r.end(|out| out.stopped = true);
        }));
    }
    {
        let r = run.clone();
        let timeout = o.timeout_ms;
        tasks.push(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(timeout)).await;
            r.end(|out| out.timed_out = true);
        }));
    }
    run.state.lock().unwrap().tasks = tasks;
    let mut staged: Option<Staged> = None;
    let result = watch_session(&run, &o.command, o.on_typing.as_ref(), &mut done_rx, &mut staged).await;
    {
        let mut st = run.state.lock().unwrap();
        for h in st.tasks.drain(..) {
            h.abort();
        }
        if let Some(h) = st.flush.take() {
            h.abort();
        }
    }
    if let Some(f) = &o.on_typing {
        f(None);
    }
    detach.detach();
    let _ = link.op("pty.kill", frame_of(json!({"ptyId": pty_id}))).await;
    if let Some(s) = &staged {
        clear_staged(link.as_ref(), s).await;
    }
    result?;
    let outcome = run.state.lock().unwrap().outcome.clone();
    Ok(outcome)
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
    let mut quiet_env: IndexMap<String, String> = IndexMap::new();
    quiet_env.insert("PS1".into(), String::new());
    for (k, v) in env {
        quiet_env.insert(k.clone(), v.clone());
    }
    let reply = link.op("pty.create", frame_of(json!({"cols": 200, "rows": 50, "shell": "/bin/sh", "env": env_value(&quiet_env)}))).await?;
    let pty_id = pty_id_of(&reply)?;
    let (done_tx, mut done_rx) = oneshot::channel();
    let quiet = Arc::new(Mutex::new(QuietState { text: String::new(), done: Some(done_tx), timed_out: false, dropped: false }));
    let sink = quiet.clone();
    let sink_pty = pty_id.clone();
    let detach = link.on_event(Arc::new(move |e: &Frame| {
        if e.get("ptyId").and_then(Value::as_str) != Some(sink_pty.as_str()) {
            return;
        }
        match e.get("type").and_then(Value::as_str) {
            Some("pty.data") => {
                let data = e.get("data").map_or_else(|| "undefined".to_string(), text_of);
                sink.lock().unwrap().text.push_str(&data);
            }
            Some("pty.exit") => fire_quiet(&sink),
            _ => {}
        }
    }));
    let closed_task = link.closed().map(|closed| {
        let q = quiet.clone();
        tokio::spawn(async move {
            closed.await;
            let dropped = mark_at_end(&lines_of(&q.lock().unwrap().text)).is_none();
            q.lock().unwrap().dropped = dropped;
            fire_quiet(&q);
        })
    });
    let timer = {
        let q = quiet.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(timeout_ms)).await;
            q.lock().unwrap().timed_out = true;
            fire_quiet(&q);
        })
    };
    let mut staged: Option<Staged> = None;
    let result = quiet_session(link, &pty_id, command, &mut staged, &mut done_rx).await;
    timer.abort();
    if let Some(h) = closed_task {
        h.abort();
    }
    detach.detach();
    let _ = link.op("pty.kill", frame_of(json!({"ptyId": pty_id}))).await;
    if let Some(s) = &staged {
        clear_staged(link, s).await;
    }
    result?;
    let st = quiet.lock().unwrap();
    let lines = lines_of(&st.text);
    let mark = mark_at_end(&lines);
    let exit_code = mark.map_or(-1, |i| lines[i].rsplit(' ').next().and_then(|d| d.parse::<i64>().ok()).unwrap_or(-1));
    let end = mark.unwrap_or(lines.len());
    let body = if end > 1 { lines[1..end].join("\n") } else { String::new() };
    Ok(QuietRun { output: js_trim(&strip_vt(&body)).to_string(), exit_code, timed_out: st.timed_out, dropped: st.dropped })
}

fn osc8_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\x1b\]8;[^;\x07\x1b]*;[^\x07\x1b]*(?:\x07|\x1b\\)").expect("the osc 8 pattern compiles"))
}

fn url_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"https?://[^\s"'<>\x00-\x1f\x7f]+"#).expect("the url pattern compiles"))
}

fn csi_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1bO[A-Z]").expect("the csi pattern compiles"))
}

fn printable_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[\x{20}-\x{7e}\x{a0}-\x{10ffff}]").expect("the printable pattern compiles"))
}

fn status_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(&format!(r"^{STATUS_MARK} [0-9]+$")).expect("the status pattern compiles"))
}

fn frame_of(v: Value) -> Frame {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn env_value(env: &IndexMap<String, String>) -> Value {
    Value::Object(env.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect())
}

fn ok_or_throw(op: &str, reply: &Frame) -> Result<(), HostError> {
    if reply.get("ok") != Some(&Value::Bool(true)) {
        let why = reply.get("error").and_then(Value::as_str).unwrap_or("no reason given");
        return Err(HostError::new(format!("{op} refused: {why}")));
    }
    Ok(())
}

fn pty_id_of(reply: &Frame) -> Result<String, HostError> {
    ok_or_throw("pty.create", reply)?;
    match reply.get("ptyId").and_then(Value::as_str) {
        Some(id) => Ok(id.to_string()),
        None => Err(HostError::new("pty.create answered without a pty id")),
    }
}

/// `pty.create {cols, rows, shell: "bash", run?, env?}`, in that order (signin-relay.ts:208, :349).
fn create_frame(cols: i64, rows: i64, bare: Option<bool>, env: Option<&IndexMap<String, String>>) -> Frame {
    let mut m = Map::new();
    m.insert("cols".into(), json!(cols));
    m.insert("rows".into(), json!(rows));
    m.insert("shell".into(), json!("bash"));
    if bare == Some(true) {
        m.insert("run".into(), json!(BARE_BASH));
    }
    if let Some(env) = env {
        m.insert("env".into(), env_value(env));
    }
    m
}

fn epoch_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_millis() as f64)
}

fn paint_on() -> bool {
    let env: Env = std::env::vars().collect();
    styles_on(&env, std::io::stdout().is_terminal())
}

fn dim(s: &str) -> String {
    style_text(&["dim"], s, paint_on())
}

fn lines_of(text: &str) -> Vec<String> {
    text.replace('\r', "").split('\n').map(String::from).collect()
}

fn mark_at_end(lines: &[String]) -> Option<usize> {
    (0..lines.len()).rev().find(|&i| status_re().is_match(&lines[i]))
}

fn fire_quiet(q: &Mutex<QuietState>) {
    let done = q.lock().unwrap().done.take();
    if let Some(tx) = done {
        let _ = tx.send(());
    }
}

struct QuietState {
    text: String,
    done: Option<oneshot::Sender<()>>,
    timed_out: bool,
    dropped: bool,
}

async fn quiet_session(
    link: &dyn PtyLink,
    pty_id: &str,
    command: &str,
    staged: &mut Option<Staged>,
    done_rx: &mut oneshot::Receiver<()>,
) -> Result<(), HostError> {
    ok_or_throw("pty.attach", &link.op("pty.attach", frame_of(json!({"ptyId": pty_id}))).await?)?;
    let s = stage(link, command).await?;
    let line = format!("c=$(cat {}); rm -rf -- {}; eval \"$c\"; printf '\\n{STATUS_MARK} %s\\n' $?; exit\r", s.file, shell_quote(&s.dir));
    *staged = Some(s);
    link.op("pty.write", frame_of(json!({"ptyId": pty_id, "data": line}))).await?;
    let _ = done_rx.await;
    Ok(())
}

/// The state a relay shares between its event sink, its timers and its keystroke loop.
struct Relay {
    output: Arc<dyn RelayOutput>,
    now: Arc<dyn Fn() -> f64 + Send + Sync>,
    state: Mutex<RelayState>,
}

struct RelayState {
    outcome: RelayOutcome,
    offer: Option<Offer>,
    scanner: UrlScanner,
    tool: ToolFilter,
    flush: Option<JoinHandle<()>>,
    done: Option<oneshot::Sender<()>>,
}

#[derive(Clone)]
struct Offer {
    url: String,
    at: f64,
    typed: bool,
}

impl Relay {
    fn fire_done(&self) {
        let done = self.state.lock().unwrap().done.take();
        if let Some(tx) = done {
            let _ = tx.send(());
        }
    }

    fn on_exit(&self, code: i64) {
        self.state.lock().unwrap().outcome.exit_code = code;
        self.fire_done();
    }

    fn show(&self, st: &mut RelayState, urls: Vec<String>) {
        for url in urls {
            st.outcome.urls += 1;
            st.offer = Some(Offer { url: url.clone(), at: (self.now)(), typed: false });
            self.output.write(&format!("\r\n  {}  {}  {}\r\n", dim("link"), hyperlink(&url), dim("o opens it on this computer")));
        }
    }

    fn open_done(&self, ok: bool, page: bool) {
        let msg = if !ok {
            OPEN_FAILED
        } else if page {
            OPENED_PAGE
        } else {
            OPENED_PRINTED
        };
        if ok {
            self.state.lock().unwrap().outcome.opened += 1;
        }
        self.output.write(&format!("\r\n  {}\r\n", dim(msg)));
    }

    fn on_data(self: &Arc<Self>, data: &str, flush_ms: u64) {
        self.output.write(data);
        let mut st = self.state.lock().unwrap();
        let said = st.tool.take(data);
        if said.is_empty() {
            return;
        }
        let urls = st.scanner.feed(&said);
        self.show(&mut st, urls);
        if let Some(h) = st.flush.take() {
            h.abort();
        }
        let me = self.clone();
        st.flush = Some(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(flush_ms)).await;
            let mut st = me.state.lock().unwrap();
            let urls = st.scanner.flush();
            me.show(&mut st, urls);
        }));
    }
}

struct RelayCtx {
    link: Arc<dyn PtyLink>,
    pty_id: String,
    relay: Arc<Relay>,
    open: Arc<dyn Fn(String) -> BoxFuture<'static, bool> + Send + Sync>,
    callback_url: Option<Arc<dyn Fn() -> Option<String> + Send + Sync>>,
    on_consent: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    cols: i64,
    rows: i64,
}

async fn next_or_pending<T>(rx: &mut UnboundedReceiver<T>, open: bool) -> Option<T> {
    if open {
        rx.recv().await
    } else {
        std::future::pending().await
    }
}

async fn relay_session(
    ctx: &RelayCtx,
    command: Option<&str>,
    terminal: &mut RelayTerminal,
    done_rx: &mut oneshot::Receiver<()>,
    raw_set: &mut bool,
    staged: &mut Option<Staged>,
    opens: &mut Vec<JoinHandle<()>>,
) -> Result<(), HostError> {
    ok_or_throw("pty.attach", &ctx.link.op("pty.attach", frame_of(json!({"ptyId": ctx.pty_id}))).await?)?;
    if let Some(raw) = terminal.raw.as_mut() {
        raw.set_raw_mode(true);
        *raw_set = true;
    }
    if let Some(cmd) = command {
        let s = stage(ctx.link.as_ref(), cmd).await?;
        let line = shell_line(&s);
        *staged = Some(s);
        ctx.link.op("pty.write", frame_of(json!({"ptyId": ctx.pty_id, "data": line}))).await?;
    }
    let (mut input_open, mut resize_open) = (true, true);
    loop {
        tokio::select! {
            _ = &mut *done_rx => break,
            chunk = next_or_pending(&mut terminal.input, input_open) => match chunk {
                Some(text) => on_key(ctx, text, opens).await,
                None => input_open = false,
            },
            size = next_or_pending(&mut terminal.resized, resize_open) => match size {
                Some(()) => on_resize(ctx).await,
                None => resize_open = false,
            },
        }
    }
    Ok(())
}

async fn on_key(ctx: &RelayCtx, text: String, opens: &mut Vec<JoinHandle<()>>) {
    if text == "o" {
        let now = (ctx.relay.now)();
        let offer = ctx.relay.state.lock().unwrap().offer.clone();
        if let Some(offer) = offer.filter(|of| !of.typed && now - of.at <= OFFER_MS) {
            let page = ctx.callback_url.as_ref().and_then(|f| f());
            let url = page.clone().unwrap_or(offer.url);
            if let Some(consent) = &ctx.on_consent {
                consent(&url);
            }
            let fut = (ctx.open)(url);
            let relay = ctx.relay.clone();
            let has_page = page.is_some();
            opens.push(tokio::spawn(async move {
                let ok = fut.await;
                relay.open_done(ok, has_page);
            }));
            return;
        }
    }
    {
        let mut st = ctx.relay.state.lock().unwrap();
        let printable = printable_re().is_match(&csi_re().replace_all(&text, ""));
        if let (Some(offer), true) = (st.offer.as_mut(), printable) {
            offer.typed = true;
        }
    }
    let _ = ctx.link.op("pty.write", frame_of(json!({"ptyId": ctx.pty_id, "data": text}))).await;
}

async fn on_resize(ctx: &RelayCtx) {
    let cols = ctx.relay.output.columns().map_or(ctx.cols, i64::from);
    let rows = ctx.relay.output.rows().map_or(ctx.rows, i64::from);
    let _ = ctx.link.op("pty.resize", frame_of(json!({"ptyId": ctx.pty_id, "cols": cols, "rows": rows}))).await;
}

/// Hands on what the pty printed once the tool started, or everything when there is no command (signin-relay.ts:191).
struct ToolFilter {
    passthrough: bool,
    started: bool,
}

impl ToolFilter {
    fn new(passthrough: bool) -> ToolFilter {
        ToolFilter { passthrough, started: false }
    }

    fn take(&mut self, chunk: &str) -> String {
        if self.passthrough || self.started {
            return chunk.to_string();
        }
        match chunk.find(TOOL_STARTS) {
            None => String::new(),
            Some(at) => {
                self.started = true;
                chunk[at + TOOL_STARTS.len()..].to_string()
            }
        }
    }
}

struct WatchRun {
    link: Arc<dyn PtyLink>,
    pty_id: String,
    on_data: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    on_url: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    on_scanned: Option<Arc<dyn Fn() + Send + Sync>>,
    on_question: Option<Arc<dyn Fn(&Asked) + Send + Sync>>,
    flush_ms: u64,
    state: Mutex<WatchState>,
}

struct WatchState {
    ended: bool,
    outcome: WatchOutcome,
    scanner: UrlScanner,
    questions: QuestionScanner,
    tool: ToolFilter,
    flush: Option<JoinHandle<()>>,
    done: Option<oneshot::Sender<()>>,
    tasks: Vec<JoinHandle<()>>,
}

impl WatchRun {
    fn end(&self, mark: impl FnOnce(&mut WatchOutcome)) {
        let mut st = self.state.lock().unwrap();
        if st.ended {
            return;
        }
        st.ended = true;
        mark(&mut st.outcome);
        if let Some(tx) = st.done.take() {
            let _ = tx.send(());
        }
    }

    fn report(&self, urls: Vec<String>) {
        if let Some(f) = &self.on_url {
            for u in urls {
                f(&u);
            }
        }
    }

    fn scanned(&self) {
        if let Some(f) = &self.on_scanned {
            f();
        }
    }

    fn answer(&self, data: String) {
        let link = self.link.clone();
        let pty_id = self.pty_id.clone();
        tokio::spawn(async move {
            let _ = link.op("pty.write", frame_of(json!({"ptyId": pty_id, "data": data}))).await;
        });
    }

    fn on_data(self: &Arc<Self>, raw: &str) {
        let (data, asked, urls) = {
            let mut st = self.state.lock().unwrap();
            let data = st.tool.take(raw);
            if data.is_empty() {
                return;
            }
            let asked = st.questions.feed(&data);
            let urls = st.scanner.feed(&data);
            (data, asked, urls)
        };
        if let Some(f) = &self.on_data {
            f(&data);
        }
        for a in asked {
            if let Settle::Answer(ans) = &a.question.settle {
                self.answer(ans.clone());
            }
            if let Some(f) = &self.on_question {
                f(&a);
            }
        }
        self.report(urls);
        self.scanned();
        let me = self.clone();
        let mut st = self.state.lock().unwrap();
        if let Some(h) = st.flush.take() {
            h.abort();
        }
        st.flush = Some(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(me.flush_ms)).await;
            let urls = me.state.lock().unwrap().scanner.flush();
            me.report(urls);
            me.scanned();
        }));
    }
}

fn typing_writer(link: Arc<dyn PtyLink>, pty_id: String) -> PtyTyping {
    Arc::new(move |data: String| -> BoxFuture<'static, Result<(), HostError>> {
        let link = link.clone();
        let pty_id = pty_id.clone();
        Box::pin(async move {
            let reply = link.op("pty.write", frame_of(json!({"ptyId": pty_id, "data": data}))).await?;
            ok_or_throw("pty.write", &reply)
        })
    })
}

async fn watch_session(
    run: &Arc<WatchRun>,
    command: &str,
    on_typing: Option<&Arc<dyn Fn(Option<PtyTyping>) + Send + Sync>>,
    done_rx: &mut oneshot::Receiver<()>,
    staged: &mut Option<Staged>,
) -> Result<(), HostError> {
    let link = run.link.clone();
    let pty_id = run.pty_id.clone();
    ok_or_throw("pty.attach", &link.op("pty.attach", frame_of(json!({"ptyId": pty_id}))).await?)?;
    if let Some(f) = on_typing {
        f(Some(typing_writer(link.clone(), pty_id.clone())));
    }
    let s = stage(link.as_ref(), command).await?;
    let line = shell_line(&s);
    *staged = Some(s);
    link.op("pty.write", frame_of(json!({"ptyId": pty_id, "data": line}))).await?;
    let _ = done_rx.await;
    Ok(())
}
