// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-threads (#2448), the adapter's half of the per-turn leak. The TS builds an adapter per call
//! (runtime agents.ts:632-678 `factory({...})`), and its `sessions` map (adapter-claude adapter.ts:629, :1268) goes
//! with it. Here `AdapterCore.sessions` holds each `ClaudeSession` (claude/adapter.rs:99, claude/turn.rs:374), whose
//! `Turn` holds the core (claude/turn.rs:384), so a finished turn, its stream and the forward it was handed (the
//! runtime's whole turn state) outlive the adapter and everyone who held it.
#![allow(dead_code, unused_imports)]

#[path = "common/claude_exec.rs"]
mod claude_exec;

use std::sync::Arc;

use claude_exec::{fixture_lines, FakeExec, StreamOpts};
use wsp_adapters::claude::adapter::{create_claude_adapter, AdapterDeps};
use wsp_adapters::port::{HarnessAdapter, HarnessStartOptions, OnEvent};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_finished_turn_lets_go_of_its_forward_once_its_adapter_is_dropped() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let exec = FakeExec::scripted(fixture_lines("stream-session.jsonl"), StreamOpts::default());
            let adapter = create_claude_adapter(AdapterDeps::new(exec.clone(), "/root/.claude-cfg")).unwrap();
            let forward: OnEvent = Arc::new(|_| {});
            let held = Arc::downgrade(&forward);
            let session =
                adapter.start(HarnessStartOptions { prompt: "fan out".into(), on_event: Some(forward), ..Default::default() }).unwrap();
            session.finished().await;
            for _ in 0..64 {
                tokio::task::yield_now().await;
            }
            drop((session, adapter, exec));
            assert!(
                held.upgrade().is_none(),
                "the forward of a finished turn is still held after its adapter was dropped (strong count {})",
                held.strong_count()
            );
        })
        .await
}

fn rss_kb() -> u64 {
    let statm = std::fs::read_to_string("/proc/self/statm").unwrap();
    let pages: u64 = statm.split_whitespace().nth(1).unwrap().parse().unwrap();
    pages * 4
}

/// Many finished turns, each on an adapter dropped after its turn, as the runtime makes them (agents.ts:632-678): the
/// process does not grow by what a turn holds. Measured over 4,000 turns on aws-mumbai: +75,956 kB before the fix (about
/// 19 kB a turn: the stream, the session and the forward of each kept), +4 kB after.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memory_stays_flat_over_many_finished_turns_on_dropped_adapters() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let run = |turns: usize| async move {
                for _ in 0..turns {
                    let exec = FakeExec::scripted(fixture_lines("stream-session.jsonl"), StreamOpts::default());
                    let adapter = create_claude_adapter(AdapterDeps::new(exec.clone(), "/root/.claude-cfg")).unwrap();
                    let forward: OnEvent = Arc::new(|_| {});
                    let session = adapter
                        .start(HarnessStartOptions { prompt: "fan out ".repeat(50), on_event: Some(forward), ..Default::default() })
                        .unwrap();
                    session.finished().await;
                    tokio::task::yield_now().await;
                }
            };
            run(500).await;
            let before = rss_kb();
            run(4_000).await;
            let grown = rss_kb().saturating_sub(before);
            eprintln!("rss grew {grown} kB over 4,000 turns");
            assert!(grown < 8_000, "4,000 finished turns grew the process by {grown} kB");
        })
        .await
}
