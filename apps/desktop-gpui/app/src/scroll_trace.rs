// SPDX-License-Identifier: AGPL-3.0-only
//! A frame-time trace of a scroll, for `--bench-replay` and `--record-scroll`: each frame's start, draw and present,
//! the time since the frame before, what the app did since then (rows built, documents parsed, images laid into rows, older
//! pages landed), and how long the wheel events it shows waited. Written as JSON for bench/trace.mjs. Also the wheel
//! events' JSON lines, which bench/momentum.mjs writes and `--record-scroll` records from a real trackpad.

use std::cell::RefCell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui_kit::{point, px, App, Pixels, Point, ScrollDelta, ScrollWheelEvent, Subscription, TouchPhase, Window};
use serde_json::{json, Value};

/// A frame slower than this, by its own draw or by the wait of the input it shows, is a stall.
pub const SLOW_MS: f64 = 50.;
/// A frame of a display's 60 a second, the line a screen's scroll is held to.
pub const FRAME_MS: f64 = 16.;
/// How long a recording runs from its first wheel event.
pub const RECORD_FOR: Duration = Duration::from_secs(60);
/// A Moved event belongs to a stream of them, which a Mac sends at least this often while fingers or momentum move,
/// so one that comes later than this after a moving one waited for the app.
const STREAM_MS: f64 = 1000. / 60.;
/// A pixel event moving at least this far is part of a moving stream; a slower one may be fingers coming to rest, and
/// a line event is a mouse wheel's, which sends no stream.
const MOVING_PX: f32 = 1.;

/// One wheel event as a JSON line: `t` in ms from the first, `dx` and `dy` in pixels (or lines), the pointer in the
/// list's own pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Wheel {
    pub t: f64,
    pub dx: f32,
    pub dy: f32,
    pub lines: bool,
    pub phase: TouchPhase,
    pub x: f32,
    pub y: f32,
}

fn phase_word(phase: TouchPhase) -> &'static str {
    match phase {
        TouchPhase::Started => "started",
        TouchPhase::Moved => "moved",
        TouchPhase::Ended => "ended",
        TouchPhase::Cancelled => "cancelled",
    }
}

impl Wheel {
    /// Every line of an events file; a line that is not a wheel event names itself.
    pub fn parse_all(text: &str) -> Result<Vec<Wheel>, String> {
        let mut out: Vec<Wheel> = Vec::new();
        for (n, line) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
            let bad = |why: &str| format!("line {}: {why}", n + 1);
            let v: Value = serde_json::from_str(line).map_err(|e| bad(&e.to_string()))?;
            let num = |name: &str| v[name].as_f64().ok_or_else(|| bad(&format!("no number {name}")));
            let phase = match v["phase"].as_str() {
                Some("started") => TouchPhase::Started,
                Some("moved") => TouchPhase::Moved,
                Some("ended") => TouchPhase::Ended,
                Some("cancelled") => TouchPhase::Cancelled,
                _ => return Err(bad("phase is none of started, moved, ended, cancelled")),
            };
            let lines = match v["unit"].as_str() {
                Some("px") => false,
                Some("line") => true,
                _ => return Err(bad("unit is neither px nor line")),
            };
            let wheel = Wheel {
                t: num("t")?,
                dx: num("dx")? as f32,
                dy: num("dy")? as f32,
                lines,
                phase,
                x: num("x")? as f32,
                y: num("y")? as f32,
            };
            if out.last().is_some_and(|last| wheel.t < last.t) {
                return Err(bad("t goes back"));
            }
            out.push(wheel);
        }
        Ok(out)
    }

    pub fn line(&self) -> String {
        let r = |v: f64| (v * 100.).round() / 100.;
        json!({ "t": r(self.t), "dx": r(self.dx.into()), "dy": r(self.dy.into()), "unit": if self.lines { "line" } else { "px" },
            "phase": phase_word(self.phase), "x": r(self.x.into()), "y": r(self.y.into()) })
        .to_string()
    }

    /// A platform's event as a line, `t` ms after the first, its pointer taken from the window's to the list's pixels.
    pub fn of(event: &ScrollWheelEvent, t: f64, origin: Point<Pixels>) -> Wheel {
        let (dx, dy, lines) = match event.delta {
            ScrollDelta::Pixels(d) => (f32::from(d.x), f32::from(d.y), false),
            ScrollDelta::Lines(d) => (d.x, d.y, true),
        };
        let at = event.position - origin;
        Wheel { t, dx, dy, lines, phase: event.touch_phase, x: at.x.into(), y: at.y.into() }
    }

    /// The event a platform would send for this line, the list's top left at `origin` in the window.
    pub fn event(&self, origin: Point<Pixels>) -> ScrollWheelEvent {
        let delta =
            if self.lines { ScrollDelta::Lines(point(self.dx, self.dy)) } else { ScrollDelta::Pixels(point(px(self.dx), px(self.dy))) };
        ScrollWheelEvent { position: origin + point(px(self.x), px(self.y)), delta, touch_phase: self.phase, ..Default::default() }
    }
}

/// What the app did on the UI thread since the frame before: rows built and the ms spent building them, documents
/// parsed, images laid into their rows, older pages landed with their events, the thread view's own render, and wheel
/// events handled.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Work {
    pub rows: u32,
    pub row_ms: f64,
    pub parsed: u32,
    pub parse_ms: f64,
    pub images: u32,
    pub pages: Vec<(usize, f64)>,
    pub view_ms: f64,
    pub input_ms: f64,
    /// Code blocks highlighted, and the ms it took.
    pub highlighted: u32,
    pub highlight_ms: f64,
    /// The key of the row at the list's top when the thread view last drew, which names what the frame showed.
    pub top: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// When the frame's work began, ms from the trace's start.
    pub at: f64,
    pub draw: f64,
    /// The UI thread's CPU time in the draw, and on Linux how often it gave up the CPU in it, by blocking and by being
    /// preempted: a draw much longer than its CPU time waited, for a lock or for a busy machine.
    pub draw_cpu: f64,
    pub switches: Option<(i64, i64)>,
    pub present: f64,
    /// From the frame before's present to this one's.
    pub since: f64,
    /// The longest any wheel event this frame shows waited, from when it was due to this frame's present.
    pub late: f64,
    pub inputs: u32,
    pub focused: bool,
    pub work: Work,
}

impl Frame {
    pub fn slow(&self) -> bool {
        self.slower_than(SLOW_MS)
    }

    pub fn slower_than(&self, limit: f64) -> bool {
        self.draw + self.present > limit || self.late > limit
    }

    /// The UI thread's own time for the frame: its draw and the traced work before it, without the present, which on
    /// a machine with no GPU is the software rasteriser's time, not the app's.
    pub fn app(&self) -> f64 {
        self.draw + self.work.input_ms + self.work.pages.iter().map(|(_, ms)| ms).sum::<f64>()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    pub due: f64,
    pub at: f64,
    pub phase: TouchPhase,
    pub moving: bool,
    pub dy: f32,
    /// Where the list stood when the event came, and where it stood once the event was handled, where that was read.
    pub spot: Option<Spot>,
    pub after: Option<f32>,
}

/// The list as an event finds it: how far it stands from its top, how far it can, and the kind of the row under the
/// pointer.
#[derive(Clone, Debug, PartialEq)]
pub struct Spot {
    pub top: f32,
    pub max: f32,
    pub under: Option<String>,
}

/// Wheel events in a row that found room to scroll and left the list where it stood: the wheel went somewhere else.
#[derive(Clone, Debug, PartialEq)]
pub struct Stuck {
    pub from: f64,
    pub to: f64,
    pub events: usize,
    pub under: Option<String>,
}

/// So many events in a row moving nothing make a stuck run, more than a frame's worth of a trackpad's.
const STUCK_EVENTS: usize = 3;

struct Trace {
    began: Instant,
    clock_ms: f64,
    frames: Vec<Frame>,
    inputs: Vec<Input>,
    work: Work,
    start: Option<(Instant, Usage)>,
    drawn: Option<(Instant, bool, Usage)>,
    presented: Option<Instant>,
    heard: Vec<Wheel>,
    scale: f32,
    _drawn: Option<Subscription>,
}

/// What this launch traces, from its flags: where the events it hears and the trace go, and the thread.
#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub record: Option<PathBuf>,
    pub trace: Option<PathBuf>,
    pub thread: Option<String>,
    pub replay: bool,
    /// The slowest a frame may be, `SLOW_MS` where none is given.
    pub limit: Option<f64>,
}

thread_local! {
    static TRACE: RefCell<Option<Trace>> = const { RefCell::new(None) };
    static PLAN: RefCell<Option<Plan>> = const { RefCell::new(None) };
}

/// Sets what this launch traces; the app reads the open thread and stamps nothing read on the host while one is set.
pub fn plan(plan: Plan) {
    PLAN.with(|p| *p.borrow_mut() = Some(plan));
}

/// Whether this launch replays or records a scroll, which writes nothing to the host it reads.
pub fn read_only() -> bool {
    PLAN.with(|p| p.borrow().is_some())
}

/// Whether wheel events over the thread list are recorded now or may start a recording.
pub fn recording() -> bool {
    PLAN.with(|p| p.borrow().as_ref().is_some_and(|p| !p.replay && (p.record.is_some() || p.trace.is_some())))
}

pub fn on() -> bool {
    TRACE.with(|t| t.borrow().is_some())
}

/// Adds to what the app did since the last frame, while a trace runs.
pub fn note(f: impl FnOnce(&mut Work)) {
    TRACE.with(|t| {
        if let Some(t) = t.borrow_mut().as_mut() {
            f(&mut t.work);
        }
    });
}

/// Runs `f`, adding its ms to what the app did while a trace runs.
pub fn timed<R>(add: impl FnOnce(&mut Work, f64), f: impl FnOnce() -> R) -> R {
    if !on() {
        return f();
    }
    let began = Instant::now();
    let out = f();
    let ms = began.elapsed().as_secs_f64() * 1000.;
    note(|w| add(w, ms));
    out
}

fn ms_since(began: Instant, t: Instant) -> f64 {
    t.saturating_duration_since(began).as_secs_f64() * 1000.
}

/// Starts tracing this window's frames, from now.
pub fn begin(window: &mut Window) {
    let drawn = window.on_drawn(|window, cx| {
        let focused = window.is_window_active();
        TRACE.with(|t| {
            if let Some(t) = t.borrow_mut().as_mut() {
                t.drawn = Some((Instant::now(), focused, Usage::now()));
            }
        });
        // Deferred work runs once the update that drew the frame ends, after GPUI has presented it.
        cx.defer(|_| presented(Instant::now()));
        on()
    });
    TRACE.with(|t| {
        *t.borrow_mut() = Some(Trace {
            began: Instant::now(),
            clock_ms: monotonic_ms(),
            frames: Vec::new(),
            inputs: Vec::new(),
            work: Work::default(),
            start: None,
            drawn: None,
            presented: None,
            heard: Vec::new(),
            scale: window.scale_factor(),
            _drawn: Some(drawn),
        })
    });
    arm(window);
}

/// The UI thread's CPU time so far, and on Linux its voluntary and involuntary context switches.
#[derive(Clone, Copy, Debug)]
struct Usage {
    cpu_ms: f64,
    switches: Option<(i64, i64)>,
}

impl Usage {
    fn now() -> Usage {
        let mut cpu = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        // SAFETY: clock_gettime writes the timespec it is handed and nothing else.
        unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut cpu) };
        #[cfg(target_os = "linux")]
        let switches = {
            // SAFETY: getrusage writes the rusage it is handed and nothing else; all zeroes is a valid rusage.
            let mut used: libc::rusage = unsafe { std::mem::zeroed() };
            (unsafe { libc::getrusage(libc::RUSAGE_THREAD, &mut used) } == 0).then_some((used.ru_nvcsw, used.ru_nivcsw))
        };
        #[cfg(not(target_os = "linux"))]
        let switches = None;
        Usage { cpu_ms: cpu.tv_sec as f64 * 1000. + cpu.tv_nsec as f64 / 1e6, switches }
    }
}

/// The monotonic clock in ms, which a profiler's samples are stamped with, so they can be laid against the frames.
fn monotonic_ms() -> f64 {
    let mut now = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: clock_gettime writes the timespec it is handed and nothing else.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) };
    now.tv_sec as f64 * 1000. + now.tv_nsec as f64 / 1e6
}

/// The next frame's start is stamped by a callback GPUI runs before it draws.
fn arm(window: &Window) {
    window.on_next_frame(|window, _| {
        if TRACE.with(|t| t.borrow_mut().as_mut().map(|t| t.start = Some((Instant::now(), Usage::now()))).is_some()) {
            arm(window);
        }
    });
}

fn presented(now: Instant) {
    TRACE.with(|t| {
        let mut t = t.borrow_mut();
        let Some(t) = t.as_mut() else { return };
        let Some((drawn, focused, used)) = t.drawn.take() else { return };
        let previous = t.presented.unwrap_or(t.began);
        let (start, began) = t.start.take().filter(|(s, _)| *s >= previous && *s <= drawn).unwrap_or((drawn, used));
        let (at, presented) = (ms_since(t.began, start), ms_since(t.began, now));
        t.frames.push(Frame {
            at,
            draw: ms_since(start, drawn),
            draw_cpu: used.cpu_ms - began.cpu_ms,
            switches: used.switches.zip(began.switches).map(|((v, i), (v0, i0))| (v - v0, i - i0)),
            present: ms_since(drawn, now),
            since: presented - ms_since(t.began, previous),
            late: 0.,
            inputs: 0,
            focused,
            work: std::mem::take(&mut t.work),
        });
        t.presented = Some(now);
    });
}

/// A wheel event due at `due` is being handled now, with the list standing at `spot`.
pub fn input(due: Instant, event: &ScrollWheelEvent, spot: Option<Spot>) {
    let moving = matches!(event.delta, ScrollDelta::Pixels(d) if f32::from(d.x).abs().max(f32::from(d.y).abs()) >= MOVING_PX);
    let dy = match event.delta {
        ScrollDelta::Pixels(d) => f32::from(d.y),
        ScrollDelta::Lines(d) => d.y,
    };
    TRACE.with(|t| {
        if let Some(t) = t.borrow_mut().as_mut() {
            let at = ms_since(t.began, Instant::now());
            t.inputs.push(Input { due: ms_since(t.began, due).min(at), at, phase: event.touch_phase, moving, dy, spot, after: None });
        }
    });
}

/// Where the list stood once the last event was handled.
pub fn handled(top: f32) {
    TRACE.with(|t| {
        if let Some(input) = t.borrow_mut().as_mut().and_then(|t| t.inputs.last_mut()) {
            input.after = Some(top);
        }
    });
}

/// Where the list stood once `inputs[ix]` was handled, or else when the next came.
fn after(inputs: &[Input], ix: usize) -> Option<f32> {
    inputs[ix].after.or_else(|| inputs.get(ix + 1).and_then(|next| next.spot.as_ref()).map(|s| s.top))
}

/// The runs of `STUCK_EVENTS` or more events that each found room to scroll their way and left the list where it
/// stood, read from where the list stood once each was handled, or else when the next came.
pub fn stuck(inputs: &[Input]) -> Vec<Stuck> {
    let mut runs: Vec<Stuck> = Vec::new();
    let mut run: Vec<&Input> = Vec::new();
    let close = |run: &mut Vec<&Input>, runs: &mut Vec<Stuck>| {
        if run.len() >= STUCK_EVENTS {
            let mut kinds: Vec<&String> = run.iter().filter_map(|i| i.spot.as_ref()?.under.as_ref()).collect();
            kinds.sort();
            let under = kinds.chunk_by(|a, b| a == b).max_by_key(|k| k.len()).map(|k| k[0].clone());
            runs.push(Stuck { from: run[0].at, to: run[run.len() - 1].at, events: run.len(), under });
        }
        run.clear();
    };
    for (ix, input) in inputs.iter().enumerate() {
        let Some(spot) = &input.spot else { continue };
        let after = after(inputs, ix);
        let room = (input.dy > 0. && spot.top > 0.5) || (input.dy < 0. && spot.top < spot.max - 0.5);
        match after {
            Some(after) if room && (after - spot.top).abs() < 0.01 => run.push(input),
            _ => close(&mut run, &mut runs),
        }
    }
    close(&mut run, &mut runs);
    runs
}

/// A real wheel event over the thread list, the list's top left at `origin`: recorded, and the first starts the trace
/// and the timer that ends the recording.
pub fn heard(event: &ScrollWheelEvent, origin: Point<Pixels>, spot: Option<Spot>, window: &mut Window, cx: &mut App) {
    if !on() {
        begin(window);
        cx.spawn(async move |cx| {
            cx.background_executor().timer(RECORD_FOR).await;
            cx.update(|cx| {
                match finish(Vec::new()) {
                    Ok(said) => eprintln!("{said}"),
                    Err(e) => {
                        eprintln!("desktop-gpui: {e}");
                        std::process::exit(1);
                    }
                }
                cx.quit();
            });
        })
        .detach();
    }
    input(Instant::now(), event, spot);
    TRACE.with(|t| {
        if let Some(t) = t.borrow_mut().as_mut() {
            // The trace began at the first event heard, so its clock is the recording's.
            let wheel = Wheel::of(event, ms_since(t.began, Instant::now()), origin);
            t.heard.push(wheel);
        }
    });
}

/// Each frame's inputs and wait: the wheel events handled after the frame before began and before it began, and the
/// longest any of them waited, from when it was due, or for a Moved event after a moving one from one stream interval
/// after that one, whichever is sooner, to the frame's present.
pub fn settle(frames: &mut [Frame], inputs: &[Input]) {
    let mut next = 0;
    for (ix, input) in inputs.iter().enumerate() {
        let due = match (input.phase, ix.checked_sub(1).map(|p| &inputs[p])) {
            (TouchPhase::Moved, Some(before)) if before.moving => input.due.min(before.due + STREAM_MS),
            _ => input.due,
        };
        while next < frames.len() && frames[next].at < input.at {
            next += 1;
        }
        let Some(frame) = frames.get_mut(next) else { break };
        frame.inputs += 1;
        frame.late = frame.late.max(frame.at + frame.draw + frame.present - due);
    }
}

/// The trace as JSON: every frame, every input, and the slow frames with what each did.
pub fn json(plan: &Plan, frames: &[Frame], inputs: &[Input], release: bool, scale: f32) -> Value {
    let work = |w: &Work| {
        json!({ "rows": w.rows, "row_ms": w.row_ms, "parsed": w.parsed, "parse_ms": w.parse_ms, "images": w.images,
            "pages": w.pages.iter().map(|(events, ms)| json!({ "events": events, "ms": ms })).collect::<Vec<_>>(), "view_ms": w.view_ms, "input_ms": w.input_ms, "highlighted": w.highlighted, "highlight_ms": w.highlight_ms, "top": w.top })
    };
    let frame = |ix: usize, f: &Frame| {
        let mut v = work(&f.work);
        let o = v.as_object_mut().expect("an object");
        for (k, x) in [
            ("frame", json!(ix)),
            ("at", json!(f.at)),
            ("draw", json!(f.draw)),
            ("draw_cpu", json!(f.draw_cpu)),
            ("switches", json!(f.switches)),
            ("present", json!(f.present)),
            ("since", json!(f.since)),
            ("late", json!(f.late)),
            ("app", json!(f.app())),
            ("inputs", json!(f.inputs)),
            ("focused", json!(f.focused)),
        ] {
            o.insert(k.to_owned(), x);
        }
        v
    };
    let limit = plan.limit.unwrap_or(SLOW_MS);
    let slow: Vec<Value> = frames.iter().enumerate().filter(|(_, f)| f.slower_than(limit)).map(|(ix, f)| frame(ix, f)).collect();
    let slowest = frames.iter().map(|f| (f.draw + f.present).max(f.late)).fold(0., f64::max);
    let pages: Vec<&(usize, f64)> = frames.iter().flat_map(|f| &f.work.pages).collect();
    let stuck: Vec<Value> =
        stuck(inputs).iter().map(|s| json!({ "from": s.from, "to": s.to, "events": s.events, "under": s.under })).collect();
    // What each kind of row under the pointer did with the wheel: events over it, and those that moved the list.
    let mut under: std::collections::BTreeMap<String, (usize, usize)> = std::collections::BTreeMap::new();
    for (ix, input) in inputs.iter().enumerate() {
        let Some(spot) = &input.spot else { continue };
        let after = after(inputs, ix);
        let seen = under.entry(spot.under.clone().unwrap_or_else(|| "none".to_owned())).or_default();
        seen.0 += 1;
        seen.1 += usize::from(after.is_some_and(|after| (after - spot.top).abs() >= 0.01));
    }
    json!({
        "app": "gpui",
        "mode": if plan.replay { "replay" } else { "record" },
        "thread": plan.thread,
        "release": release,
        "scale": scale,
        "limit_ms": limit,
        "focused": frames.iter().all(|f| f.focused),
        "summary": { "frames": frames.len(), "inputs": inputs.len(), "slow": slow.len(), "slowest_ms": slowest,
            "app_slow": frames.iter().filter(|f| f.app() > limit).count(), "app_slowest_ms": frames.iter().map(Frame::app).fold(0., f64::max), "pages": pages.len(),
            "page_events": pages.iter().map(|(e, _)| e).sum::<usize>(), "stuck": stuck.len() },
        "slow": slow,
        "stuck": stuck,
        "under": under.iter().map(|(kind, (events, moved))| (kind.clone(), json!({ "events": events, "moved": moved }))).collect::<serde_json::Map<_, _>>(),
        "frames": frames.iter().enumerate().map(|(ix, f)| frame(ix, f)).collect::<Vec<_>>(),
        "inputs": inputs
            .iter()
            .map(|i| json!({ "due": i.due, "at": i.at, "phase": phase_word(i.phase), "dy": i.dy, "top": i.spot.as_ref().map(|s| s.top),
                "after": i.after, "under": i.spot.as_ref().and_then(|s| s.under.clone()) }))
            .collect::<Vec<_>>(),
    })
}

/// Ends the trace and writes it with `rows`, what each row of the thread was by its index, and the events heard where
/// the launch records them; says what it wrote.
pub fn finish(rows: Vec<Value>) -> Result<String, String> {
    let Some(mut trace) = TRACE.with(|t| t.borrow_mut().take()) else { return Err("no trace ran".to_owned()) };
    let plan = PLAN.with(|p| p.borrow().clone()).unwrap_or_default();
    settle(&mut trace.frames, &trace.inputs);
    let mut out = json(&plan, &trace.frames, &trace.inputs, !cfg!(debug_assertions), trace.scale);
    out["rows"] = Value::Array(rows);
    out["clock_ms"] = json!(trace.clock_ms);
    let mut said = format!(
        "trace: {} frames, {} inputs, {} slow, slowest {:.1} ms",
        trace.frames.len(),
        trace.inputs.len(),
        out["summary"]["slow"],
        out["summary"]["slowest_ms"].as_f64().unwrap_or_default()
    );
    if let Some(path) = &plan.trace {
        std::fs::write(path, format!("{out}\n")).map_err(|e| format!("{}: {e}", path.display()))?;
        said.push_str(&format!(", written to {}", path.display()));
    }
    if let Some(path) = &plan.record {
        let lines: String = trace.heard.iter().map(|w| format!("{}\n", w.line())).collect();
        std::fs::write(path, lines).map_err(|e| format!("{}: {e}", path.display()))?;
        said.push_str(&format!("; {} wheel events recorded to {}", trace.heard.len(), path.display()));
    }
    Ok(said)
}

#[cfg(test)]
mod tests;
