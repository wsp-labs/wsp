// SPDX-License-Identifier: AGPL-3.0-only
//! A list that refreshes where it stands, for every line that takes --watch. Port of `packages/host/src/watch.ts` at
//! 65f6b89c7; the spec is `spec/layout.md`.
use std::os::raw::c_int;
use std::os::raw::c_void;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::time::Duration;

use futures::future::BoxFuture;
use tokio::sync::Notify;
use wsp_words::exit::{usage_refusal, HostError};

use crate::init_layout::{cells, rewind};

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

/// The listeners the process's stop signals reach, each with the number of the signal it waits on.
static PROCESS_LISTENERS: Mutex<Vec<(c_int, StopListener)>> = Mutex::new(Vec::new());
/// The write end of the pipe the signal handler puts each taken signal's number into.
static SIGNAL_PIPE: AtomicI32 = AtomicI32::new(-1);
static DISPATCHER: Once = Once::new();

extern "C" fn take_signal(signal: c_int) {
    let byte = signal as u8;
    // SAFETY: write is async-signal-safe and reads one byte from a local that outlives the call.
    unsafe {
        libc::write(SIGNAL_PIPE.load(Ordering::SeqCst), (&byte as *const u8).cast::<c_void>(), 1);
    }
}

fn signal_number(name: &str) -> c_int {
    match name {
        "SIGINT" => libc::SIGINT,
        "SIGTERM" => libc::SIGTERM,
        _ => libc::SIGHUP,
    }
}

/// Starts the thread that calls the listeners of each signal the handler took, once per process.
fn start_dispatcher() {
    DISPATCHER.call_once(|| {
        let mut fds = [0 as c_int; 2];
        // SAFETY: pipe writes two descriptors into the array on success.
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0, "a pipe for the stop signals");
        SIGNAL_PIPE.store(fds[1], Ordering::SeqCst);
        let read_end = fds[0];
        std::thread::spawn(move || loop {
            let mut byte = 0u8;
            // SAFETY: reads at most one byte into a local.
            let got = unsafe { libc::read(read_end, (&mut byte as *mut u8).cast::<c_void>(), 1) };
            if got == 0 || (got < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted) {
                break;
            }
            if got != 1 {
                continue;
            }
            let listeners: Vec<StopListener> = PROCESS_LISTENERS
                .lock()
                .unwrap()
                .iter()
                .filter(|(number, _)| *number == c_int::from(byte))
                .map(|(_, l)| l.clone())
                .collect();
            for listener in listeners {
                listener();
            }
        });
    });
}

/// The process's own SIGINT, SIGTERM and SIGHUP.
pub struct ProcessSignals;

impl WatchSignals for ProcessSignals {
    fn on(&self, signal: &'static str, listener: StopListener) {
        start_dispatcher();
        let number = signal_number(signal);
        PROCESS_LISTENERS.lock().unwrap().push((number, listener));
        // SAFETY: installs the handler above, which only writes one byte to the pipe.
        unsafe {
            libc::signal(number, take_signal as *const () as libc::sighandler_t);
        }
    }
    fn off(&self, signal: &'static str, listener: &StopListener) {
        let number = signal_number(signal);
        let mut listeners = PROCESS_LISTENERS.lock().unwrap();
        listeners.retain(|(held, l)| !(*held == number && Arc::ptr_eq(l, listener)));
        if !listeners.iter().any(|(held, _)| *held == number) {
            // SAFETY: restores the default disposition, as the process had it before the watch.
            unsafe {
                libc::signal(number, libc::SIG_DFL);
            }
        }
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

/// The flag a stop sets and the wait between frames listens to, so a stop ends the wait rather than the frame after.
struct Stop {
    stopped: AtomicBool,
    cut: Notify,
}

impl Stop {
    fn halted(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }
    fn halt(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.cut.notify_one();
    }
}

/// watch.ts:53-94.
pub async fn watch_block(lines: Lines, deps: WatchDeps) -> Result<(), HostError> {
    let every = Duration::from_millis(deps.every_ms.unwrap_or(WATCH_EVERY_MS));
    let signals: Arc<dyn WatchSignals> = deps.signals.clone().unwrap_or_else(|| Arc::new(ProcessSignals));
    let stop = Arc::new(Stop { stopped: AtomicBool::new(false), cut: Notify::new() });
    let listener: StopListener = {
        let stop = stop.clone();
        Arc::new(move || stop.halt())
    };
    for sig in WATCH_STOP_SIGNALS {
        signals.on(sig, listener.clone());
    }
    deps.redraw.write(HIDE_CURSOR);
    let outcome = frames(&lines, &deps, &stop, every).await;
    for sig in WATCH_STOP_SIGNALS {
        signals.off(sig, &listener);
    }
    deps.redraw.write(SHOW_CURSOR);
    outcome
}

/// The frames of a watch: each reading drawn over the last, until a stop arrives or a reading refuses.
async fn frames(lines: &Lines, deps: &WatchDeps, stop: &Stop, every: Duration) -> Result<(), HostError> {
    let mut drawn: Vec<usize> = Vec::new();
    while !stop.halted() {
        let rows = lines().await?;
        if !rows.is_empty() {
            deps.redraw.write(&format!("{}{}\n", rewind(&drawn, deps.redraw.columns()), rows.join("\n")));
            drawn = rows.iter().map(|row| cells(row)).collect();
        }
        if stop.halted() {
            break;
        }
        wait_between(stop, every, deps.until.as_ref()).await;
    }
    Ok(())
}

/// The wait between two frames: the tick, a stop, or the next reading when the source pushes one.
async fn wait_between(stop: &Stop, every: Duration, until: Option<&Arc<dyn Fn() -> BoxFuture<'static, ()> + Send + Sync>>) {
    let tick = tokio::time::sleep(every);
    let cut = stop.cut.notified();
    match until {
        Some(next) => {
            tokio::select! {
                _ = tick => {}
                _ = cut => {}
                _ = next() => {}
            }
        }
        None => {
            tokio::select! {
                _ = tick => {}
                _ = cut => {}
            }
        }
    }
}

/// watch.ts:99-103: where a --watch may redraw, or the refusal that says why not.
pub enum WatchOn {
    Redraw(Arc<dyn Redraw>),
    Refusal(HostError),
}

/// watch.ts:99-103. `--json` is refused first, then no terminal.
pub fn watch_on(words: &str, json: bool, redraw: Option<Arc<dyn Redraw>>) -> WatchOn {
    if json {
        return WatchOn::Refusal(usage_refusal(
            &format!("{words} --watch redraws a table and --json answers with objects."),
            &format!("Take one of the two: {words} --watch at a terminal, or {words} --json for the objects."),
        ));
    }
    match redraw {
        Some(redraw) => WatchOn::Redraw(redraw),
        None => WatchOn::Refusal(usage_refusal(
            &format!("{words} --watch redraws where it stands, and this run has no terminal to redraw on."),
            &format!("Run {words} without --watch to print the list once."),
        )),
    }
}
