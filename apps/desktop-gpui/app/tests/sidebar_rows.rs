// SPDX-License-Identifier: AGPL-3.0-only
//! What the sidebar keeps of each session row, counted by the allocator: it holds every row on the host for the life
//! of the app, so a row keeps the fields its readers use and not the whole record the host sent.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use desktop_gpui::sidebar::model::Row;
use serde_json::{json, Value};

struct Counting;

static HELD: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        HELD.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        HELD.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        HELD.fetch_add(size, Ordering::Relaxed);
        HELD.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, size) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

fn subagent(i: usize) -> Value {
    json!({"id": format!("toolu_01ABCDEFGHIJKLMNOP{i:04}"), "title": "Explore the sidebar model and list every reader", "state": "done",
      "parentToolUseId": format!("toolu_01QRSTUVWXYZabcdef{i:04}"), "depth": 1, "model": "claude-sonnet-4-5",
      "asked": "x".repeat(200), "startedAt": 1760000000000.0, "endedAt": 1760000100000.0, "lastLine": "y".repeat(120),
      "usage": {"tokens": {"input": 12345, "output": 2345, "cached": 1000, "cacheWrite": 200}, "toolUses": 12, "durationMs": 100000}})
}

/// A row shaped like a real `SessionView`: a 600-character prompt, a 160-character last line, its project and the
/// thread it forked from.
fn row(i: usize, subagents: usize) -> Value {
    let thread = format!("th-{i:08}-aaaa-bbbb-cccc-dddddddddddd");
    let mut v = json!({"id": format!("sess-{i:08}-aaaa-bbbb-cccc-dddddddddddd"), "workspaceId": "ws-0123456789abcdef", "harness": "claude",
      "status": "completed", "startedBy": "person", "claudeSessionId": "9f1c2d3e-4b5a-6c7d-8e9f-0a1b2c3d4e5f",
      "threadId": thread, "rootThreadId": thread, "prompt": "p".repeat(600), "harnessTitle": "Fix the scroll jump when a page lands",
      "titleSource": "auto", "startedAt": 1760000000000.0, "endedAt": 1760000500000.0, "cwd": "/Users/someone/code/wsp/.wsp/worktrees/fix",
      "costUsd": 1.2345, "lastLine": "l".repeat(160), "model": "claude-opus-4-1", "effort": "high", "permissionMode": "acceptEdits",
      "contextWindow": "200k", "project": {"id": "proj-0123456789", "name": "wsp"}, "computerName": "Mac mini",
      "readAt": 1760000600000.0, "forkedFrom": {"threadId": "th-x", "title": "Old title"}});
    if subagents > 0 {
        v["subagents"] = Value::Array((0..subagents).map(subagent).collect());
    }
    v
}

/// The bytes the sidebar keeps a row with once it has read the host's text, the row's own size among them.
fn kept_per_row(subagents: usize) -> usize {
    let n = 1_000;
    let sent: Vec<String> = (0..n).map(|i| row(i, subagents).to_string()).collect();
    let before = HELD.load(Ordering::Relaxed);
    let rows: Vec<Row> = sent.iter().filter_map(|text| Row::owned(serde_json::from_str(text).unwrap())).collect();
    let after = HELD.load(Ordering::Relaxed);
    assert_eq!(rows.len(), n);
    (after - before) / n
}

#[test]
fn a_row_keeps_its_fields_and_not_the_record_the_host_sent() {
    let (bare, busy) = (kept_per_row(0), kept_per_row(5));
    println!("kept per row: {bare} bytes with no subagents, {busy} bytes with five");
    // The parsed fields alone: the prompt, the last line, the titles and ids, the subagents' own words.
    assert!(bare < 2_500, "a row keeps {bare} bytes, as much as the whole record");
    assert!(busy < 7_000, "a row with five subagents keeps {busy} bytes");
}
