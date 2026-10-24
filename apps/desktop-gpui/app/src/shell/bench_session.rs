// SPDX-License-Identifier: AGPL-3.0-only
//! Bench mode's session (`--bench-session`): the memory at rest 30 s after the host's threads are listed, as a clean
//! launch reads, then threads opened one after another as a person moves down the sidebar, each held until its rows
//! are drawn and settled, with the memory read after each; then the first threads opened again, which must read as
//! they did the first time; then a walk of Ctrl+Tab presses through the sidebar, the memory and the window's atlas
//! read after each.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::{Context, KeyDownEvent, Keystroke, PlatformInput, Task, Window};

use super::bench_run::{BenchRun, Phase};
use super::Shell;

/// How long a thread stays on screen after its first frame before the memory is read: its images decode and its code
/// highlights in that time, as a person looks at it.
const SETTLE: Duration = Duration::from_secs(2);
/// How long the bench holds after it says a step's line, so a reader outside the app (`footprint` on the Mac) reads it.
const READ: Duration = Duration::from_secs(3);
/// How long the app rests once the host's threads are listed before the first reading, as a clean launch is read.
const REST: Duration = Duration::from_secs(30);
/// How many of the first threads the session opens again at its end.
const RETURNS: usize = 3;
/// The Ctrl+Tab presses of the walk after the returns.
const WALK: usize = 60;
/// How long the walk stays on each thread once it is drawn before the memory is read: a glance, not a read.
const GLANCE: Duration = Duration::from_secs(1);
/// What the walk may take past the bench's own limit.
pub(super) const WALKING: Duration = Duration::from_secs(WALK as u64 * 8);
/// What a session may take past the bench's own limit, per thread.
pub(super) const PER_THREAD: Duration = Duration::from_secs(15);
/// What the rest before the first thread takes past the bench's own limit.
pub(super) const RESTING: Duration = Duration::from_secs(40);

type Presented = Rc<RefCell<Option<Instant>>>;

pub(super) struct Session {
    /// The step under way: below the session's length a thread of it, past that a return to one of the first.
    n: usize,
    at: At,
    began: Instant,
    steps: Vec<serde_json::Value>,
    /// Each thread's rows as first read: how many, and a hash of their keys and text.
    seen: HashMap<String, (usize, u64)>,
    returned: Vec<serde_json::Value>,
    walk: Vec<serde_json::Value>,
    /// The heap sampler's mark at the first step's reading, so the holders grown since then are told apart.
    mark: usize,
    /// Where the result goes, beside which the heap's holders are written where the build samples them.
    out: std::path::PathBuf,
}

enum At {
    Resting {
        until: Instant,
        _wake: Task<()>,
    },
    Opening {
        start: Instant,
        presented: Presented,
    },
    /// A Ctrl+Tab pressed on the thread `from`, waiting for the thread it steps to.
    Stepping {
        from: Option<String>,
        start: Instant,
    },
    Settling {
        until: Instant,
        ms: f64,
        _wake: Task<()>,
    },
    /// The hold after a reading, and the step that follows it.
    Reading {
        until: Instant,
        next: usize,
        _wake: Task<()>,
    },
}

/// The resident size of this process in bytes, where the system says it cheaply.
fn resident() -> Option<u64> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    // SAFETY: sysconf reads a constant of the system and touches no memory of ours.
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    Some(pages * page.max(0) as u64)
}

fn mb(bytes: u64) -> f64 {
    (bytes as f64 / 1_048_576. * 10.).round() / 10.
}

fn open_thread(shell: &Shell) -> Option<String> {
    match shell.route() {
        crate::screens::Route::Thread { thread_id, .. } => Some(thread_id.clone()),
        _ => None,
    }
}

/// One reading's memory as the bench prints it and keeps it: resident size, the live heap where counted, and the
/// window's atlas.
fn memory(window: &Window) -> (String, serde_json::Value) {
    let (rss, heap, atlas) = (resident(), crate::heap::live(), window.atlas_usage());
    let said = format!(
        "rss {} heap {} atlas {:.1} MB glyphs {:.1} MB images",
        rss.map_or("-".to_owned(), |b| format!("{:.1} MB", mb(b))),
        heap.map_or("-".to_owned(), |(b, n)| format!("{:.1} MB in {n} allocations", mb(b as u64))),
        mb(atlas.glyphs),
        mb(atlas.images),
    );
    let kept = serde_json::json!({
        "rss_mb": rss.map(mb),
        "heap_mb": heap.map(|(b, _)| mb(b as u64)),
        "heap_allocations": heap.map(|(_, n)| n),
        "atlas_glyphs_mb": mb(atlas.glyphs),
        "atlas_images_mb": mb(atlas.images),
    });
    (said, kept)
}

impl Session {
    pub(super) fn new(out: std::path::PathBuf, cx: &mut Context<Shell>) -> Self {
        Session {
            n: 0,
            at: At::Resting { until: Instant::now() + REST, _wake: wake(REST, cx) },
            began: Instant::now(),
            steps: Vec::new(),
            seen: HashMap::new(),
            returned: Vec::new(),
            walk: Vec::new(),
            mark: 0,
            out,
        }
    }

    /// Writes the heap's holders beside the result, where the build samples them.
    fn write_holders(&self, name: &str, text: Option<String>) -> std::io::Result<()> {
        text.map_or(Ok(()), |text| std::fs::write(self.out.with_extension(name), text))
    }

    /// Whether the session waits on a timer, not on the next frame; at rest the window draws on, as one a person looks
    /// at does while the host's reads land.
    pub(super) fn holding(&self) -> bool {
        matches!(self.at, At::Settling { .. } | At::Reading { .. })
    }
}

fn wake(after: Duration, cx: &mut Context<Shell>) -> Task<()> {
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(after).await;
        _ = this.update(cx, |_, cx| cx.notify());
    })
}

impl Shell {
    /// Bench mode's tick for a session, in place of the other phases.
    pub(super) fn session_tick(&mut self, mut run: BenchRun, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(run.phase, Phase::Opening) && self.host.is_some() && !self.sidebar.read(cx).items().is_empty() {
            cx.activate(true);
            window.activate_window();
            run.phase = Phase::Session(Session::new(run.args.out.clone(), cx));
        }
        let mut holding = false;
        if let Phase::Session(session) = &mut run.phase {
            if self.session_step(session, &run.args.session, window, cx) {
                let json = Self::session_result(session);
                let grown = crate::heap::holders(40, session.mark);
                let written = std::fs::write(&run.args.out, format!("{json}\n"))
                    .and_then(|()| session.write_holders("holders.txt", crate::heap::holders(40, 0)))
                    .and_then(|()| session.write_holders("grown.txt", grown));
                if let Err(e) = written {
                    eprintln!("desktop-gpui: {}: {e}", run.args.out.display());
                    std::process::exit(1);
                }
                cx.quit();
                return;
            }
            holding = session.holding();
        }
        self.bench = Some(run);
        if !holding {
            window.request_animation_frame();
        }
    }

    /// One step of the session; true once it has written its result.
    fn session_step(&mut self, session: &mut Session, list: &[String], window: &mut Window, cx: &mut Context<Self>) -> bool {
        let returns = RETURNS.min(list.len());
        let thread = |n: usize| if n < list.len() { list[n].clone() } else { list[n - list.len()].clone() };
        match &session.at {
            At::Resting { until, .. } => {
                if Instant::now() < *until {
                    return false;
                }
                let (said, mut kept) = memory(window);
                eprintln!("bench: session step 0/{} thread rest rows 0 events 0 {said}", list.len());
                kept["step"] = 0.into();
                kept["thread"] = "rest".into();
                session.steps.push(kept);
                if let Err(e) = session.write_holders("rest.txt", crate::heap::holders(40, 0)) {
                    eprintln!("desktop-gpui: {}: {e}", session.out.display());
                    std::process::exit(1);
                }
                session.at = At::Reading { until: Instant::now() + READ, next: 0, _wake: wake(READ, cx) };
            }
            At::Opening { start, presented } => {
                let view = self.bench_view(cx);
                let shown = *presented.borrow();
                if let Some(shown) = shown.filter(|_| view.read(cx).loaded()) {
                    let ms = shown.duration_since(*start).as_secs_f64() * 1000.;
                    session.at = At::Settling { until: Instant::now() + SETTLE, ms, _wake: wake(SETTLE, cx) };
                }
            }
            At::Stepping { from, start } => {
                let to = open_thread(self);
                let view = self.bench_view(cx);
                let drawn = to.is_some() && to != *from && view.read(cx).thread() == to.as_deref() && view.read(cx).loaded();
                if drawn {
                    let ms = start.elapsed().as_secs_f64() * 1000.;
                    session.at = At::Settling { until: Instant::now() + GLANCE, ms, _wake: wake(GLANCE, cx) };
                }
            }
            At::Settling { until, ms, .. } => {
                if Instant::now() < *until {
                    return false;
                }
                let ms = *ms;
                let walking = session.n >= list.len() + returns;
                let id = if walking { open_thread(self).unwrap_or_default() } else { thread(session.n) };
                let view = self.bench_view(cx);
                let (rows, read, events) = {
                    let v = view.read(cx);
                    let mut hash = std::collections::hash_map::DefaultHasher::new();
                    for row in v.rows() {
                        (&row.key, &row.text).hash(&mut hash);
                    }
                    (v.rows().len(), hash.finish(), v.transcript().event_list().len())
                };
                let (said, mut kept) = memory(window);
                if session.n == 0 {
                    session.mark = crate::heap::mark();
                }
                if walking {
                    let step = session.n - list.len() - returns + 1;
                    eprintln!("bench: session walk {step}/{WALK} thread {id} rows {rows} events {events} {said}");
                    kept["walk"] = step.into();
                    kept["thread"] = id.into();
                    kept["rows"] = rows.into();
                    kept["ms"] = ms.into();
                    session.walk.push(kept);
                } else if session.n < list.len() {
                    session.seen.entry(id.clone()).or_insert((rows, read));
                    eprintln!("bench: session step {}/{} thread {id} rows {rows} events {events} {said}", session.n + 1, list.len());
                    kept["step"] = (session.n + 1).into();
                    kept["thread"] = id.into();
                    kept["rows"] = rows.into();
                    kept["events"] = events.into();
                    kept["ms"] = ms.into();
                    session.steps.push(kept);
                } else {
                    let first = session.seen.get(&id).copied();
                    let same = first == Some((rows, read));
                    eprintln!("bench: session return to {id} rows {rows} {}", if same { "reads as it did" } else { "reads otherwise" });
                    session
                        .returned
                        .push(serde_json::json!({ "thread": id, "rows": rows, "first_rows": first.map(|f| f.0), "same": same }));
                    if !same {
                        eprintln!("desktop-gpui: {id} read {rows} rows on return, not the {:?} it read first", first.map(|f| f.0));
                        std::process::exit(1);
                    }
                }
                session.at = At::Reading { until: Instant::now() + READ, next: session.n + 1, _wake: wake(READ, cx) };
            }
            At::Reading { until, next, .. } => {
                if Instant::now() < *until {
                    return false;
                }
                session.n = *next;
                if session.n < list.len() + returns {
                    let start = Instant::now();
                    let presented = self.bench_open(&thread(session.n), window, cx);
                    session.at = At::Opening { start, presented };
                    return false;
                }
                if session.n < list.len() + returns + WALK {
                    session.at = At::Stepping { from: open_thread(self), start: Instant::now() };
                    // Pressed once the shell's update ends, since the key's listener updates the shell.
                    window.defer(cx, |window, cx| {
                        let keystroke = Keystroke::parse("ctrl-tab").expect("ctrl-tab reads");
                        let key = KeyDownEvent { keystroke, is_held: false, prefer_character_input: false };
                        window.dispatch_event(PlatformInput::KeyDown(key), cx);
                    });
                    return false;
                }
                return true;
            }
        }
        false
    }

    /// The session's result: each step's thread, rows, events, time to its first frame and the memory after it, the
    /// most of each, and the returns.
    fn session_result(session: &Session) -> serde_json::Value {
        let most = |key: &str| {
            session.steps.iter().filter_map(|s| s[key].as_f64()).fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))))
        };
        serde_json::json!({
            "app": "gpui",
            "session": {
                "steps": session.steps,
                "max_rss_mb": most("rss_mb"),
                "max_heap_mb": most("heap_mb"),
                "returned": session.returned,
                "walk": session.walk,
                "walk_max_rss_mb": session.walk.iter().filter_map(|s| s["rss_mb"].as_f64()).fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v)))),
                "seconds": session.began.elapsed().as_secs_f64(),
            },
            "memory": if cfg!(target_os = "linux") { "rss: resident size of this process; heap: live bytes the counting allocator holds (heap-profile builds)" } else { "rss: not read here, see bench/session.sh; heap: heap-profile builds" },
        })
    }
}
