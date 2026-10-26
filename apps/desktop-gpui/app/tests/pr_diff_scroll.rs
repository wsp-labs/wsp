// SPDX-License-Identifier: AGPL-3.0-only
//! A bench, not a check: the shared code view over a diff of 5,000-odd highlighted lines (nine TypeScript files, a
//! change every seventh line), scrolled top to bottom by wheel events at 4,800 px/s in a real window, timing every
//! frame the window draws. Run it on a display with a window manager, since GPUI holds a window without focus to 30 fps:
//!   DIFF_BENCH=<out.json> cargo test --release -p desktop-gpui --test pr_diff_scroll
//! It writes the frame times and prints frames per second, and each frame's work from its render to the end of its
//! paint, which tells how much of the display's interval is left once the rate is held at the display's;
//! `DIFF_BENCH_HEAP=1` also counts the heap the diff holds at the end and the most it held at once, which the
//! resident set follows since glibc keeps what is freed (the count slows the frames), and `DIFF_BENCH_LINES=<n>` sets each file's lines (500; 2,200 make a 2 MiB patch).
//! Without `DIFF_BENCH` it says how to run and passes.
//! Without libtest's harness, since a platform's app loop must own the main thread.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::time::{Duration, Instant};

use desktop_gpui::screens::pr_pane::diff::{self, CodeView, FileHead, Layout as DiffLayout};
use desktop_gpui::ui::{self, Side, UiAssets};
use gpui_kit::component::Root;
use gpui_kit::{
    canvas, div, point, prelude::*, px, size, AnyElement, App, Bounds, Context, Entity, Modifiers, PlatformInput, ScrollDelta,
    ScrollWheelEvent, TouchPhase, Window, WindowBounds, WindowOptions,
};
use serde_json::json;

/// Counts the bytes the process holds on the heap, so the bench can say what a loaded diff keeps rather than what the
/// allocator has not handed back.
struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);
/// The most `LIVE` has been: what the process asked of the allocator at once, which glibc keeps once freed.
static PEAK: AtomicIsize = AtomicIsize::new(0);
/// Counting costs every thread an atomic add per allocation, which slowed the frames it timed by half; it is on only
/// for a memory run (`DIFF_BENCH_HEAP=1`), never for one that reports frames.
static COUNT: AtomicBool = AtomicBool::new(false);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNT.load(Ordering::Relaxed) {
            grow(layout.size() as isize);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if COUNT.load(Ordering::Relaxed) {
            LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNT.load(Ordering::Relaxed) {
            grow(new_size as isize - layout.size() as isize);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

fn grow(by: isize) {
    let live = LIVE.fetch_add(by, Ordering::Relaxed) + by;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

/// The heap bytes allocated since `since` and not freed, in KiB.
fn held_kib(since: isize) -> isize {
    (LIVE.load(Ordering::Relaxed) - since) / 1024
}

/// The most the heap held at once since `since` was read, in KiB.
fn peak_kib(since: isize) -> isize {
    (PEAK.load(Ordering::Relaxed) - since) / 1024
}

const FILES: usize = 9;
const PX_PER_S: f32 = 4800.;
/// Every row is 20 px; the scroll runs past the last one so the end is reached whatever the heads add.
const ROW: f32 = 20.;
const GIVE_UP: Duration = Duration::from_secs(120);

fn lines() -> usize {
    std::env::var("DIFF_BENCH_LINES").ok().and_then(|n| n.parse().ok()).unwrap_or(500)
}

fn patch(file: usize) -> String {
    let lines = lines();
    let mut body = String::new();
    let line = |i: usize, v: &str| {
        format!("export function f{file}_{i}(a: number, b: string): number {{ return a + b.length * {i}{v}; }} // {i}\n")
    };
    for i in 1..=lines {
        if i % 7 == 0 {
            body.push('-');
            body.push_str(&line(i, ""));
            body.push('+');
            body.push_str(&line(i, " + 1"));
        } else {
            body.push(' ');
            body.push_str(&line(i, ""));
        }
    }
    let path = format!("src/bench_{file}.ts");
    format!("diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1,{lines} +1,{lines} @@\n{body}")
}

/// The diff's rows: every patch line, the changed ones once per side.
pub fn rows() -> usize {
    FILES * (lines() + lines() / 7)
}

struct Bench {
    view: Entity<CodeView>,
    frames: Rc<RefCell<Vec<Instant>>>,
    /// Each timed frame's work in ms, from its render to the end of its paint, before the platform presents it.
    work: Rc<RefCell<Vec<f64>>>,
    started: Option<Instant>,
    opened: Instant,
    scrolled: f32,
    out: String,
    rss_before: u64,
    live_before: isize,
    patch_bytes: usize,
}

/// The process's resident set in KiB, from `/proc/self/statm` (Linux only; 0 elsewhere).
fn rss_kib() -> u64 {
    let pages = std::fs::read_to_string("/proc/self/statm").ok().and_then(|s| s.split(' ').nth(1).and_then(|p| p.parse::<u64>().ok()));
    pages.unwrap_or(0) * 4
}

impl Bench {
    fn finish(&mut self, cx: &mut App) {
        let frames = self.frames.borrow();
        let start = self.started.unwrap();
        let ms: Vec<f64> = frames.iter().map(|t| t.duration_since(start).as_secs_f64() * 1000.).collect();
        let secs = ms.last().copied().unwrap_or(0.) / 1000.;
        let fps = if secs > 0. { (frames.len() as f64 - 1.) / secs } else { 0. };
        let mut gaps: Vec<f64> = ms.windows(2).map(|w| w[1] - w[0]).collect();
        gaps.sort_by(f64::total_cmp);
        let pick = |q: f64| gaps.get(((gaps.len() as f64 - 1.) * q).round() as usize).copied().unwrap_or(0.);
        let mut work = self.work.borrow().clone();
        work.sort_by(f64::total_cmp);
        let busy = |q: f64| work.get(((work.len() as f64 - 1.) * q).round() as usize).copied().unwrap_or(0.);
        let grown = rss_kib().saturating_sub(self.rss_before);
        let (held, peak) = (held_kib(self.live_before), peak_kib(self.live_before));
        println!("pr_diff_scroll: {} bytes of patch; from before the reply was read to the end of the scroll the heap holds {held} KiB more, at most {peak} KiB more at once, and the resident set grew {grown} KiB", self.patch_bytes);
        let summary = json!({"rows": rows(), "patch_bytes": self.patch_bytes, "rss_grown_kib": grown, "heap_held_kib": held, "heap_peak_kib": peak, "px_per_s": PX_PER_S, "frames": frames.len(), "seconds": secs, "fps": fps,
            "frame_ms_p50": pick(0.5), "frame_ms_p95": pick(0.95), "frame_ms_max": pick(1.), "frame_ms": ms,
            "work_ms_p50": busy(0.5), "work_ms_p95": busy(0.95), "work_ms_max": busy(1.)});
        std::fs::write(&self.out, format!("{summary}\n")).unwrap();
        println!(
            "pr_diff_scroll: {} rows, {} frames in {secs:.2} s: {fps:.1} fps, frame p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms; work p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms",
            rows(),
            frames.len(),
            pick(0.5),
            pick(0.95),
            pick(1.),
            busy(0.5),
            busy(0.95),
            busy(1.)
        );
        cx.quit();
    }
}

impl Render for Bench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.request_animation_frame();
        let now = Instant::now();
        if now.duration_since(self.opened) > GIVE_UP {
            eprintln!("pr_diff_scroll: did not finish in {} s", GIVE_UP.as_secs());
            std::process::exit(1);
        }
        match self.started {
            None if window.is_window_active() && now.duration_since(self.opened) > Duration::from_secs(3) => {
                println!(
                    "pr_diff_scroll: drawn and highlighted, the heap holds {} KiB more than before the reply, at most {} KiB more at once",
                    held_kib(self.live_before),
                    peak_kib(self.live_before)
                );
                self.started = Some(now);
                self.frames.borrow_mut().push(now);
            }
            None => window.activate_window(),
            Some(start) => {
                if !window.is_window_active() {
                    eprintln!("pr_diff_scroll: the window lost focus, and GPUI holds a window without focus to 30 fps");
                    std::process::exit(3);
                }
                self.frames.borrow_mut().push(now);
                let target = now.duration_since(start).as_secs_f32() * PX_PER_S;
                let step = target - self.scrolled;
                self.scrolled = target;
                let center = window.bounds().size.center();
                let wheel = ScrollWheelEvent {
                    position: point(center.x, center.y),
                    delta: ScrollDelta::Pixels(point(px(0.), px(-step))),
                    modifiers: Modifiers::none(),
                    touch_phase: TouchPhase::Moved,
                };
                window.on_next_frame(move |window, cx| {
                    window.dispatch_event(PlatformInput::ScrollWheel(wheel), cx);
                });
                if self.scrolled > rows() as f32 * ROW + 40. * FILES as f32 {
                    self.finish(cx);
                }
            }
        }
        let began = self.started.is_some().then_some((now, self.work.clone()));
        div().size_full().child(self.view.clone()).child(canvas(
            |_, _, _| {},
            move |_, _, _, _| {
                if let Some((began, work)) = began {
                    work.borrow_mut().push(began.elapsed().as_secs_f64() * 1000.);
                }
            },
        ))
    }
}

fn head(head: &FileHead, _: &mut Window, cx: &mut App) -> AnyElement {
    div().flex().gap(px(4.)).child(diff::file_chevron(head, cx)).child(head.path.clone()).into_any_element()
}

fn main() {
    let Ok(out) = std::env::var("DIFF_BENCH") else {
        println!("pr_diff_scroll: a bench; run it with DIFF_BENCH=<out.json> on a display with a window manager");
        return;
    };
    COUNT.store(std::env::var_os("DIFF_BENCH_HEAP").is_some(), Ordering::Relaxed);
    let rss_before = rss_kib();
    let live_before = LIVE.load(Ordering::Relaxed);
    let patches: Vec<String> = (0..FILES).map(patch).collect();
    let patch_bytes = patches.iter().map(String::len).sum();
    let files: Vec<_> =
        (0..FILES).map(|f| json!({"path": format!("src/bench_{f}.ts"), "kind": "modified", "patch": patches[f].clone()})).collect();
    let reply = json!({"base": null, "truncated": false, "moved": [], "files": files});
    let model = diff::read_reply(reply.as_object().unwrap()).unwrap();
    drop(reply);
    println!("pr_diff_scroll: the reply read, the heap holds {} KiB more", held_kib(live_before));
    gpui_kit::application().with_assets(UiAssets).run(move |cx: &mut App| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        let bounds = Bounds::centered(None, size(px(1000.), px(800.)), cx);
        cx.open_window(
            WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), focus: true, ..Default::default() },
            |window, cx| {
                let view = cx.new(|cx| {
                    let mut view = CodeView::new(cx);
                    view.on_head(head);
                    view.set_layout(DiffLayout::Stacked, cx);
                    view.set_files(&model.files, cx);
                    view
                });
                let bench = cx.new(|_| Bench {
                    view,
                    frames: Rc::default(),
                    work: Rc::default(),
                    started: None,
                    opened: Instant::now(),
                    scrolled: 0.,
                    out: out.clone(),
                    rss_before,
                    live_before,
                    patch_bytes,
                });
                cx.new(|cx| Root::new(bench, window, cx))
            },
        )
        .expect("a window");
        cx.activate(true);
    });
}
