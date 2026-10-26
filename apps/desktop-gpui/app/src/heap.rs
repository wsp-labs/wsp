// SPDX-License-Identifier: AGPL-3.0-only
//! The heap as a dev build counts it, under the `heap-profile` feature, which no release turns on: live bytes and live
//! allocations through a counting allocator over the system's, and the call stacks of a byte-weighted sample of the
//! allocations still live, which [`holders`] sums by the code that made them. Without the feature every reading is None and every mark 0.

/// Live heap bytes and live allocations, where the counting allocator is installed.
pub fn live() -> Option<(usize, usize)> {
    #[cfg(feature = "heap-profile")]
    return Some(imp::live());
    #[cfg(not(feature = "heap-profile"))]
    None
}

/// The most the live heap held since the last call, which a transient burst reaches and the system allocator then keeps
/// as its own high mark.
pub fn take_peak() -> Option<usize> {
    #[cfg(feature = "heap-profile")]
    return Some(imp::take_peak());
    #[cfg(not(feature = "heap-profile"))]
    None
}

/// Where the sampler stands now, for [`holders`] to sum only what was allocated after it.
pub fn mark() -> usize {
    #[cfg(feature = "heap-profile")]
    return imp::mark();
    #[cfg(not(feature = "heap-profile"))]
    0
}

/// The sampled live heap allocated after `since` (a [`mark`]; 0 for all of it) summed by allocation site, biggest
/// first, as text: each line the estimated live bytes, the sampled allocations behind them, and the first frames
/// outside the allocator and the standard library.
pub fn holders(top: usize, since: usize) -> Option<String> {
    #[cfg(feature = "heap-profile")]
    return Some(imp::holders(top, since));
    #[cfg(not(feature = "heap-profile"))]
    {
        _ = (top, since);
        None
    }
}

#[cfg(feature = "heap-profile")]
pub use imp::Counting;

#[cfg(feature = "heap-profile")]
mod imp {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicIsize, AtomicU8, AtomicUsize, Ordering};
    use std::sync::Mutex;

    /// One sample per this many bytes allocated; an allocation this big or bigger is always sampled.
    const RATE: usize = 64 * 1024;
    const DEPTH: usize = 48;
    /// Per-slot counts of sampled pointers hashed there, so a free looks a pointer up only when one may be sampled.
    const SLOTS: usize = 1 << 22;

    static LIVE_BYTES: AtomicIsize = AtomicIsize::new(0);
    static LIVE_COUNT: AtomicIsize = AtomicIsize::new(0);
    static PEAK_BYTES: AtomicIsize = AtomicIsize::new(0);
    static MAYBE: [AtomicU8; SLOTS] = [const { AtomicU8::new(0) }; SLOTS];
    static SAMPLES: Mutex<Option<HashMap<usize, Sample>>> = Mutex::new(None);
    static TAKEN: AtomicUsize = AtomicUsize::new(0);

    struct Sample {
        seq: usize,
        weight: usize,
        stack: [usize; DEPTH],
    }

    thread_local! {
        /// Inside the sampler's own bookkeeping, whose allocations are counted and never sampled.
        static BUSY: Cell<bool> = const { Cell::new(false) };
        static UNTIL: Cell<isize> = const { Cell::new(RATE as isize) };
    }

    pub struct Counting;

    fn slot(ptr: usize) -> &'static AtomicU8 {
        &MAYBE[(ptr >> 4).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - 22)]
    }

    /// Runs `f` unless this thread is already inside the sampler or is being torn down.
    fn guarded(f: impl FnOnce()) {
        let entered = BUSY.try_with(|b| !b.replace(true)).unwrap_or(false);
        if entered {
            f();
            _ = BUSY.try_with(|b| b.set(false));
        }
    }

    fn taken(ptr: *mut u8, size: usize) {
        let due = size >= RATE
            || UNTIL
                .try_with(|u| {
                    let left = u.get() - size as isize;
                    u.set(if left <= 0 { RATE as isize } else { left });
                    left <= 0
                })
                .unwrap_or(false);
        if !due {
            return;
        }
        guarded(|| {
            let mut stack = [0usize; DEPTH];
            let mut n = 0;
            // SAFETY: on Linux and macOS the unwinder is safe to run from several threads at once.
            unsafe {
                backtrace::trace_unsynchronized(|frame| {
                    stack[n] = frame.ip() as usize;
                    n += 1;
                    n < DEPTH
                })
            };
            let mut samples = SAMPLES.lock().unwrap_or_else(|e| e.into_inner());
            let seq = TAKEN.fetch_add(1, Ordering::Relaxed) + 1;
            samples.get_or_insert_with(HashMap::new).insert(ptr as usize, Sample { seq, weight: size.max(RATE), stack });
            slot(ptr as usize).fetch_add(1, Ordering::Relaxed);
        });
    }

    fn freed(ptr: *mut u8) {
        if slot(ptr as usize).load(Ordering::Relaxed) == 0 {
            return;
        }
        guarded(|| {
            let mut samples = SAMPLES.lock().unwrap_or_else(|e| e.into_inner());
            if samples.as_mut().and_then(|s| s.remove(&(ptr as usize))).is_some() {
                slot(ptr as usize).fetch_sub(1, Ordering::Relaxed);
            }
        });
    }

    // SAFETY: every call goes to the system allocator with the caller's layout; the counting around it allocates only
    // under the thread's guard, which keeps it from sampling itself.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let ptr = unsafe { System.alloc(layout) };
            if !ptr.is_null() {
                let now = LIVE_BYTES.fetch_add(layout.size() as isize, Ordering::Relaxed) + layout.size() as isize;
                PEAK_BYTES.fetch_max(now, Ordering::Relaxed);
                LIVE_COUNT.fetch_add(1, Ordering::Relaxed);
                taken(ptr, layout.size());
            }
            ptr
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let ptr = unsafe { System.alloc_zeroed(layout) };
            if !ptr.is_null() {
                let now = LIVE_BYTES.fetch_add(layout.size() as isize, Ordering::Relaxed) + layout.size() as isize;
                PEAK_BYTES.fetch_max(now, Ordering::Relaxed);
                LIVE_COUNT.fetch_add(1, Ordering::Relaxed);
                taken(ptr, layout.size());
            }
            ptr
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            freed(ptr);
            LIVE_BYTES.fetch_sub(layout.size() as isize, Ordering::Relaxed);
            LIVE_COUNT.fetch_sub(1, Ordering::Relaxed);
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let moved = unsafe { System.realloc(ptr, layout, new_size) };
            if !moved.is_null() {
                freed(ptr);
                let grown = new_size as isize - layout.size() as isize;
                PEAK_BYTES.fetch_max(LIVE_BYTES.fetch_add(grown, Ordering::Relaxed) + grown, Ordering::Relaxed);
                taken(moved, new_size);
            }
            moved
        }
    }

    pub fn live() -> (usize, usize) {
        (LIVE_BYTES.load(Ordering::Relaxed).max(0) as usize, LIVE_COUNT.load(Ordering::Relaxed).max(0) as usize)
    }

    /// Frames that say where the heap is used rather than how it was allocated: the standard library, the collections
    /// under it and this sampler, by their source file where the build has line tables, else by their name.
    fn plumbing(frame: &str) -> bool {
        let (name, file) = frame.split_once(" @ ").unwrap_or((frame, ""));
        if name.starts_with("__rust") || name.contains("::__rust_") {
            return true;
        }
        if !file.is_empty() {
            return ["library/", "hashbrown-", "smallvec-", "indexmap-", "backtrace-", "app/src/heap.rs"].iter().any(|p| file.contains(p));
        }
        let name = name.trim_start_matches('<');
        ["alloc::", "core::", "std::", "hashbrown::", "backtrace::", "desktop_gpui::heap", "__rust", "_rust", "smallvec::", "indexmap::"]
            .iter()
            .any(|p| name.starts_with(p))
            || ["as core::clone::Clone>", "as alloc::", "as core::iter::", "as core::convert::", "as core::ops::"]
                .iter()
                .any(|p| name.contains(p))
    }

    /// A frame as `name @ file:line`, the file from its crate's folder on, or its name alone without line tables.
    fn frame(symbol: &backtrace::Symbol, ip: usize) -> String {
        let name = symbol.name().map(|n| format!("{n:#}")).unwrap_or_else(|| format!("{ip:#x}"));
        let Some(file) = symbol.filename() else { return name };
        let file = file.to_string_lossy();
        let short = ["/registry/src/", "/git/checkouts/", "/vendor/", "/apps/desktop-gpui/", "/rustc/"]
            .iter()
            .find_map(|p| file.find(p).map(|at| &file[at + p.len()..]))
            .map(|rest| if file.contains("/registry/src/") { rest.split_once('/').map_or(rest, |(_, r)| r) } else { rest })
            .unwrap_or(&file);
        format!("{name} @ {short}:{}", symbol.lineno().unwrap_or(0))
    }

    pub fn take_peak() -> usize {
        PEAK_BYTES.swap(LIVE_BYTES.load(Ordering::Relaxed), Ordering::Relaxed).max(0) as usize
    }

    pub fn mark() -> usize {
        TAKEN.load(Ordering::Relaxed)
    }

    pub fn holders(top: usize, since: usize) -> String {
        let stacks: Vec<(usize, [usize; DEPTH])> = {
            let mut out = Vec::new();
            guarded(|| {
                let samples = SAMPLES.lock().unwrap_or_else(|e| e.into_inner());
                out = samples.iter().flat_map(|s| s.values()).filter(|s| s.seq > since).map(|s| (s.weight, s.stack)).collect();
            });
            out
        };
        let mut names: HashMap<usize, String> = HashMap::new();
        let mut name = |ip: usize| {
            names
                .entry(ip)
                .or_insert_with(|| {
                    let mut said = Vec::new();
                    backtrace::resolve(ip as *mut std::ffi::c_void, |s| said.push(frame(s, ip)));
                    said.join(" <- ")
                })
                .clone()
        };
        let mut by_site: HashMap<String, (usize, usize)> = HashMap::new();
        let mut by_app: HashMap<String, (usize, usize)> = HashMap::new();
        let total: usize = stacks.iter().map(|(w, _)| w).sum();
        for (weight, stack) in &stacks {
            let frames: Vec<String> = stack
                .iter()
                .filter(|ip| **ip != 0)
                .map(|ip| name(*ip))
                .flat_map(|f| f.split(" <- ").map(str::to_owned).collect::<Vec<_>>())
                .collect();
            let site: Vec<&str> = frames.iter().map(String::as_str).filter(|f| !plumbing(f)).take(6).collect();
            let entry = by_site.entry(site.join("\n      <- ")).or_default();
            entry.0 += weight;
            entry.1 += 1;
            let app = frames
                .iter()
                .filter(|f| !plumbing(f))
                .find(|f| f.contains("app/src/") || f.trim_start_matches('<').starts_with("desktop_gpui::"))
                .cloned();
            let entry = by_app.entry(app.unwrap_or_else(|| "(no desktop_gpui frame)".to_owned())).or_default();
            entry.0 += weight;
            entry.1 += 1;
        }
        let mb = |b: usize| b as f64 / 1_048_576.;
        let (bytes, count) = live();
        let mut out = format!(
            "live heap {:.1} MB in {count} allocations; sampled {:.1} MB in {} live samples taken after sample {since}, of {} taken (one per {} KiB)\n",
            mb(bytes),
            mb(total),
            stacks.len(),
            TAKEN.load(Ordering::Relaxed),
            RATE / 1024
        );
        let mut list = |title: &str, map: HashMap<String, (usize, usize)>| {
            let mut rows: Vec<_> = map.into_iter().collect();
            rows.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
            out.push_str(&format!("\n== {title}\n"));
            for (site, (weight, n)) in rows.into_iter().take(top) {
                out.push_str(&format!("{:8.1} MB {n:6}  {site}\n", mb(weight)));
            }
        };
        list("by the first desktop_gpui frame", by_app);
        list("by allocation site, first frames past the allocator", by_site);
        out
    }
}
