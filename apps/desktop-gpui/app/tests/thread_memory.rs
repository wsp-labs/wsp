// SPDX-License-Identifier: AGPL-3.0-only
//! What a thread leaves behind once the person has moved on: threads opened one after another through the frame, as a
//! click in the sidebar opens them, against a host started for this test alone whose claude is the bench's stub playing
//! the session fixture's threads (bench/session.mjs: tool output, subagents, code, tables, screenshots). The live heap
//! is read through a counting allocator, back on the same first thread each time, after 18 threads and again after 12
//! more; what the 12 left behind has to stay within a bound that does not grow with their number.
//! The only test in its binary, so no other test's allocations land in the count. With `--features heap-profile` it
//! counts through the app's sampler instead and prints what the many threads left, by the code that allocated it.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

#[cfg(not(feature = "heap-profile"))]
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::Path;
use std::process::Command;
#[cfg(not(feature = "heap-profile"))]
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::thread_view::ThreadView;
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, Entity, TestAppContext, VisualTestContext};
use wsp_host_client::HostClient;

#[cfg(not(feature = "heap-profile"))]
struct Counting;

#[cfg(not(feature = "heap-profile"))]
static LIVE: AtomicIsize = AtomicIsize::new(0);

// SAFETY: every call goes to the system allocator with the caller's layout; the count beside it allocates nothing.
#[cfg(not(feature = "heap-profile"))]
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LIVE.fetch_add(new_size as isize - layout.size() as isize, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[cfg(not(feature = "heap-profile"))]
#[global_allocator]
static COUNTING: Counting = Counting;

#[cfg(feature = "heap-profile")]
#[global_allocator]
static COUNTING: desktop_gpui::heap::Counting = desktop_gpui::heap::Counting;

fn live() -> isize {
    #[cfg(feature = "heap-profile")]
    return desktop_gpui::heap::live().map_or(0, |(bytes, _)| bytes as isize);
    #[cfg(not(feature = "heap-profile"))]
    LIVE.load(Ordering::Relaxed)
}

/// Threads opened before the first reading, so the caches that fill once per run (fonts, grammars, icons) are full
/// and the socket's read buffer, which keeps the size of the largest page it read, has met pages like the later ones;
/// with views drawn in place the element arena and the layout tree reach their high mark only at the 18th.
const WARM: usize = 18;
/// Threads opened between the two readings.
const MANY: usize = 12;
/// Turns of each session thread the host records.
const TURNS: usize = 6;
/// What the many threads may leave behind together. Before the fix each left about a megabyte of its rows and
/// documents, so twelve stayed past this whatever the noise.
const BOUND: isize = 3 * 1024 * 1024;

fn mb(bytes: isize) -> f64 {
    bytes as f64 / 1_048_576.
}

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn settle(cx: &mut VisualTestContext, for_: Duration) {
    let end = Instant::now() + for_;
    while Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A bench thread's turn: its prompt and the screenshots it carries, each a name, a width and a height.
type Turn = (String, Vec<(String, u32, u32)>);

/// The turns of a bench thread as content.mjs makes them.
fn turns_of(host: &common::Throwaway, name: &str) -> Vec<Turn> {
    let content = common::root().join("apps/desktop-gpui/bench/content.mjs");
    let script = "const { turnsOf } = await import(process.argv[1]); console.log(JSON.stringify(turnsOf(process.argv[2]).map(t => [t.prompt, (t.images ?? []).map(i => [i.name, i.width, i.height])])))";
    let out = Command::new("node")
        .args(["--input-type=module", "-e", script])
        .arg(content.to_string_lossy().as_ref())
        .arg(name)
        .env_clear()
        .envs(host.env())
        .current_dir(host.dir.path())
        .output()
        .unwrap();
    assert!(out.status.success(), "content.mjs: {}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap()
}

/// A screenshot-sized PNG with enough noise in it to weigh like one.
fn shot(path: &Path, width: u32, height: u32, seed: u32) {
    let mut at = seed.wrapping_mul(2_654_435_761);
    let pixels = image::RgbImage::from_fn(width, height, |x, y| {
        at ^= at << 13;
        at ^= at >> 17;
        at ^= at << 5;
        let noise = (x > width / 3 && x < width / 2 && y < height / 3) as u32 * (at & 0xff);
        image::Rgb([(x % 251) as u8 ^ noise as u8, (y % 241) as u8, ((x + y) % 239) as u8])
    });
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    pixels.save(path).unwrap();
}

/// Records the first `turns` turns of the bench thread `name` on the host and answers its thread id.
fn record(host: &common::Throwaway, name: &str, turns: usize) -> String {
    let mut thread = String::new();
    let named = regex::Regex::new(r"\bthread ([0-9a-f-]{36})\b").unwrap();
    for (i, (prompt, images)) in turns_of(host, name).into_iter().take(turns).enumerate() {
        std::fs::write(host.dir.path().join("bench-turn.json"), serde_json::json!({ "thread": name, "turn": i }).to_string()).unwrap();
        if i == 0 {
            let said = host.wsp(&["run", "repo", "--agent", "claude", &prompt]);
            thread = named.captures(&said).unwrap_or_else(|| panic!("wsp run named no thread: {said}"))[1].to_owned();
            continue;
        }
        let mut args = vec!["send".to_owned(), thread.clone()];
        for (n, (file, width, height)) in images.into_iter().enumerate() {
            let path = host.dir.path().join("shots").join(name).join(file);
            shot(&path, width / 2, height / 2, (i * 10 + n) as u32);
            args.extend(["--file".to_owned(), path.to_string_lossy().into_owned()]);
        }
        args.push(prompt);
        host.wsp(&args.iter().map(String::as_str).collect::<Vec<_>>());
    }
    thread
}

fn view_of(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<Entity<ThreadView>> {
    shell.read_with(cx, |s, cx| s.thread_screen().map(|screen| screen.read(cx).view().clone()))
}

#[gpui_kit::test]
fn the_threads_left_behind_keep_nothing_that_grows_with_their_number(cx: &mut TestAppContext) {
    let stub = common::root().join("apps/desktop-gpui/bench/claude-stub.mjs");
    let host = common::Throwaway::start_with(&format!("#!/bin/sh\nexec node '{}' \"$@\"\n", stub.display()), false);
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    let first = record(&host, "short-1", 3);
    let threads: Vec<String> = (1..=WARM + MANY).map(|i| record(&host, &format!("session-{i}"), TURNS)).collect();

    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let listed = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let workspace = listed.iter().find(|t| t.thread_id.as_deref() == Some(first.as_str())).expect("the first thread").workspace_id.clone();

    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
    });
    let (state, vars) = (host.state(), host.env());
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, vars, runtime.clone(), Launch::default(), window, cx));
        shell = Some(view.clone());
        Root::new(view, window, cx)
    });
    let shell = shell.unwrap();
    cx.simulate_resize(size(px(1280.), px(800.)));
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    until(cx, "every thread in the sidebar", |cx| shell.read_with(cx, |s, cx| s.sidebar().read(cx).items().len() > WARM + MANY));

    let open = |thread: &str, cx: &mut VisualTestContext| {
        let route = Route::Thread { workspace_id: workspace.clone(), thread_id: thread.to_owned() };
        shell.update_in(cx, |s, window, cx| s.go(route, window, cx));
        until(cx, "the thread drawn", |cx| {
            view_of(&shell, cx).is_some_and(|v| v.read_with(cx, |v, _| v.thread() == Some(thread) && v.loaded() && !v.rows().is_empty()))
        });
        settle(cx, Duration::from_millis(600));
        view_of(&shell, cx).unwrap().read_with(cx, |v, _| v.rows().iter().map(|r| (r.key.clone(), r.text.clone())).collect::<Vec<_>>())
    };
    // The heap back on the first thread once everything opened before has settled: the least of three readings, as a
    // background task may still hold a buffer it is about to free.
    let reading = |cx: &mut VisualTestContext| {
        let rows = open(&first, cx);
        settle(cx, Duration::from_secs(2));
        let mut least = isize::MAX;
        for _ in 0..3 {
            settle(cx, Duration::from_millis(300));
            least = least.min(live());
        }
        (least, rows)
    };

    let shown = open(&first, cx);
    for thread in &threads[..WARM] {
        open(thread, cx);
    }
    let (before, again) = reading(cx);
    let mark = desktop_gpui::heap::mark();
    assert_eq!(again, shown, "the first thread read otherwise on its return");
    let mut steps = Vec::new();
    for thread in &threads[WARM..] {
        open(thread, cx);
        steps.push(mb(live() - before));
    }
    let (after, again) = reading(cx);
    assert_eq!(again, shown, "the first thread read otherwise on its second return");
    if let Some(grown) = desktop_gpui::heap::holders(20, mark) {
        println!("{grown}");
    }
    let steps: Vec<String> = steps.iter().map(|s| format!("{s:+.1}")).collect();
    println!(
        "live heap on the first thread after {WARM} threads: {:.1} MB; with each of {MANY} more open, MB past it: {}",
        mb(before),
        steps.join(" ")
    );
    println!("live heap on the first thread after {} threads: {:.1} MB ({:+.1} MB)", WARM + MANY, mb(after), mb(after - before));
    assert!(
        after - before < BOUND,
        "{MANY} threads opened and left kept {:.1} MB, past the {:.1} MB bound: the heap grows with the threads opened",
        mb(after - before),
        mb(BOUND)
    );
}
