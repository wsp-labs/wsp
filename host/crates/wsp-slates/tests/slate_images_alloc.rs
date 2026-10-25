// SPDX-License-Identifier: AGPL-3.0-only
//! The buffer rule of spec/slate_images.md (rule 5, Decision 3) in a binary of its own, since a counting global
//! allocator would count every other test's allocations too.

use std::alloc::{GlobalAlloc, Layout, System};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use base64::Engine;
use wsp_slates::slate_images::{slate_image, SlateImageRoads};

const PNG_HEX: &str = "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4890000000d4944415478da63f8cfc0f01f0005000201a3b0d8f60000000049454e44ae426082";
const ONE_MIB: usize = 1024 * 1024;

static COUNTING: AtomicBool = AtomicBool::new(false);
static LARGEST: AtomicUsize = AtomicUsize::new(0);

struct Counting;

fn note(size: usize) {
    if COUNTING.load(Ordering::SeqCst) {
        LARGEST.fetch_max(size, Ordering::SeqCst);
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(new_size);
        System.realloc(ptr, layout, new_size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

// TS slate-images.test.ts:90
#[tokio::test(flavor = "current_thread")]
async fn holds_what_the_file_weighs_not_the_cap() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let png: Vec<u8> = (0..PNG_HEX.len()).step_by(2).map(|i| u8::from_str_radix(&PNG_HEX[i..i + 2], 16).unwrap()).collect();
            let dir = tempfile::Builder::new().prefix("wsp-slate-images-").tempdir_in("/tmp").unwrap();
            let path = dir.path().join("home.png");
            std::fs::write(&path, &png).unwrap();
            let src = path.to_str().unwrap().to_string();
            let roads = SlateImageRoads {
                thread: "t-alloc".into(),
                have: None,
                folder: None,
                on: None,
                allowed: Rc::new(|_| false),
                fetch: None,
            };

            LARGEST.store(0, Ordering::SeqCst);
            COUNTING.store(true, Ordering::SeqCst);
            let answer = slate_image(&src, roads).await;
            COUNTING.store(false, Ordering::SeqCst);
            let largest = LARGEST.load(Ordering::SeqCst);

            let shown = serde_json::to_value(&answer).unwrap();
            assert_eq!(shown["mediaType"], "image/png", "{shown}");
            assert_eq!(shown["bytes"], base64::engine::general_purpose::STANDARD.encode(&png));
            assert!(largest < ONE_MIB, "reading a {} byte image made a single allocation of {largest} bytes", png.len());
        })
        .await
}
