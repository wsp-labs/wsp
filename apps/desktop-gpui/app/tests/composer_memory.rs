// SPDX-License-Identifier: AGPL-3.0-only
//! What screenshots attached to the composer leave once they leave it: pasted, drawn as the box's 56 px tiles (a GIF
//! whole, as it moves), and removed again, as many as the box takes at a time, three times over with new ones each
//! time. The live heap through a
//! counting allocator and the window's atlas have to come back to where they stood, whatever the screenshots weighed.
//! The only test in its binary, so no other test's allocations land in the count.

#[path = "composer/harness.rs"]
mod harness;
#[path = "composer/states.rs"]
mod states;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;

use desktop_gpui::screens::composer::model::File;
use gpui_kit::{TestAppContext, VisualTestContext};

struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);

// SAFETY: every call goes to the system allocator with the caller's layout; the count beside it allocates nothing.
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

#[global_allocator]
static COUNTING: Counting = Counting;

/// Screenshots attached at once, as many as the box takes.
const SHOTS: usize = desktop_gpui::screens::composer::model::FILES_MAX;
/// Times the box fills and empties.
const ROUNDS: usize = 3;
/// What the rounds may leave on the heap together: a whole decode of one of these screenshots is 9 MB.
const BOUND: isize = 2 * 1024 * 1024;

fn mb(bytes: isize) -> f64 {
    bytes as f64 / 1_048_576.
}

/// A 1600 by 1500 screenshot with noise enough in it to weigh like one, as a PNG.
/// A GIF's side in pixels: the box draws it whole.
const GIF: (u32, u32) = (800, 600);

/// A GIF of `GIF` pixels, in few enough colours to encode fast.
fn gif(seed: u32) -> Vec<u8> {
    let frame =
        image::RgbaImage::from_fn(GIF.0, GIF.1, |x, y| image::Rgba([((x / 50 + seed) % 2 * 200) as u8, (y / 50 % 2 * 200) as u8, 90, 255]));
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::gif::GifEncoder::new_with_speed(&mut bytes, 30);
    encoder.encode_frame(image::Frame::new(frame)).unwrap();
    drop(encoder);
    bytes
}

fn screenshot(seed: u32) -> Vec<u8> {
    let mut at = seed.wrapping_mul(2_654_435_761) | 1;
    let pixels = image::RgbImage::from_fn(1600, 1500, |x, y| {
        at ^= at << 13;
        at ^= at >> 17;
        at ^= at << 5;
        image::Rgb([(x % 251) as u8 ^ (at & 0x0f) as u8, (y % 241) as u8, ((x + y) % 239) as u8])
    });
    let mut png = Vec::new();
    image::DynamicImage::ImageRgb8(pixels).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    png
}

fn draw(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
}

fn images(cx: &mut VisualTestContext) -> u64 {
    cx.update(|window, _| window.atlas_usage().images)
}

#[gpui_kit::test]
fn screenshots_that_leave_the_composer_leave_nothing_behind(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let cx = harness::open("empty", cx);
    let composer = harness::composer_of(cx);
    let shots: Vec<Vec<Arc<Vec<u8>>>> = (0..ROUNDS as u32)
        .map(|r| (0..SHOTS as u32).map(|i| Arc::new(if i == 0 { gif(r) } else { screenshot(r * 100 + i) })).collect())
        .collect();
    let fill = |round: &[Arc<Vec<u8>>], cx: &mut VisualTestContext| {
        let files = round
            .iter()
            .enumerate()
            .map(|(i, bytes)| {
                let (ext, media) = if i == 0 { ("gif", "image/gif") } else { ("png", "image/png") };
                File { name: format!("pasted-image-{i}.{ext}"), media_type: media.into(), bytes: bytes.clone() }
            })
            .collect();
        composer.update(cx, |c, cx| c.attach(files, cx));
        draw(cx);
        assert_eq!(composer.read_with(cx, |c, _| c.files().len()), SHOTS, "the box took every screenshot");
    };
    let empty = |cx: &mut VisualTestContext| {
        for _ in 0..SHOTS {
            composer.update(cx, |c, cx| c.remove_file(0, cx));
        }
        draw(cx);
    };
    draw(cx);
    let (before, atlas) = (LIVE.load(Ordering::Relaxed), images(cx));
    let mut drawn = 0;
    for round in &shots {
        fill(round, cx);
        drawn = drawn.max(images(cx));
        empty(cx);
    }
    let (after, left) = (LIVE.load(Ordering::Relaxed), images(cx));
    println!(
        "atlas images: {atlas} bytes before, {drawn} with {SHOTS} screenshots in the box, {left} after {ROUNDS} rounds; heap {:+.1} MB over the rounds",
        mb(after - before)
    );
    assert!(
        after - before < BOUND,
        "{ROUNDS} rounds of {SHOTS} screenshots kept {:.1} MB, past the {:.1} MB bound: the box keeps what leaves it",
        mb(after - before),
        mb(BOUND)
    );
    assert!(drawn > atlas, "the box drew none of its screenshots");
    assert_eq!(left, atlas, "the screenshots that left the box stayed in the window's atlas");
    let tiles = drawn.saturating_sub(atlas);
    let most = (SHOTS as u64 - 1) * 4 * 112 * 112 + 4 * u64::from(GIF.0 * GIF.1);
    assert!(tiles <= most, "the box drew {tiles} bytes of images for {} tiles of 56 px and a GIF", SHOTS - 1);
}
