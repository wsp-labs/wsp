// SPDX-License-Identifier: AGPL-3.0-only
//! Three shapes the host writes in an order its code builds, not the schema's: a backend's capabilities write `sizes` after
//! `ownNetwork`, and a plugin row writes `project`, `path` and `setIn` after `brings`, and a harness catalog row writes `source`, `version`, `contextWindows`,
//! `steers`, `renames` and `images` before `harness` (harness-catalog.ts `fromTable`). Read and written back, the text is
//! the input's byte for byte. A folded thread row is written in `foldThreads`' order (session.ts:395-453), the optional
//! block first and `turns`, `ran`, the first turn's ids and `costUsd` after it. A thread's facts (boot.ts:748-756,
//! `{...thread, ...sub, ...diskFacts(record), model, ...}`) write `diskGone`, then `diskKeptUntil`, after the cost.
mod common;
use common::reads_exact;
use serde_json::json;
use wsp_wire::types::agents_report::PluginRow;
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::preferences::ThreadFacts;
use wsp_wire::views::session::ThreadView;
use wsp_wire::wire::capabilities::Capabilities;

#[test]
fn a_boxs_capabilities_write_sizes_last() {
    let box_capabilities = json!({
        "liveCloneForks": false, "pauseMode": "disk", "replacesMachine": true, "previewUrls": true, "signedUrls": false,
        "callbackRelay": true, "diskSnapshots": true, "images": true, "snapshotsAnyLife": true, "snapshotListing": true,
        "templates": true, "kept": false, "copies": true, "ownNetwork": true,
        "sizes": [{ "name": "small", "cpu": 2, "memMb": 4096, "rateUsdPerHour": 0.11 }]
    });
    let read: Capabilities = reads_exact(box_capabilities);
    assert_eq!(read.sizes.len(), 1);
}

#[test]
fn a_plugin_row_writes_project_path_and_set_in_after_brings() {
    let row = json!({
        "agent": "claude", "id": "frontend-design@official", "name": "frontend-design", "marketplace": "official", "version": "1.2.0",
        "scope": "local", "on": true, "missing": "folder", "description": "d", "source": "anthropics/claude-code",
        "brings": { "skills": ["frontend-design"], "commands": [], "subagents": [], "hooks": [], "servers": [], "lsp": [], "apps": [] },
        "project": { "id": "pr_1a2b3c4d", "name": "wsp", "path": "~/wsp" }, "path": "~/.claude/plugins/cache/official/frontend-design",
        "setIn": "~/wsp/.claude/settings.local.json"
    });
    let read: PluginRow = reads_exact(row);
    assert_eq!(read.set_in.as_deref(), Some("~/wsp/.claude/settings.local.json"));
}

#[test]
fn a_catalog_row_writes_its_table_keys_before_harness() {
    let row = json!({
        "source": "table", "version": null, "contextWindows": [], "steers": false, "renames": false, "images": false,
        "harness": "gemini", "label": "Gemini CLI", "models": [], "efforts": [], "bypassMode": "yolo", "permissionModes": []
    });
    let read: HarnessCatalog = reads_exact(row);
    assert_eq!(read.harness, "gemini");
}

fn folded_row() -> serde_json::Value {
    json!({
        "id": "t1", "threadId": "th", "workspaceId": "ws", "harness": "claude", "startedBy": "person", "status": "failed",
        "title": "a title", "sessionId": "s9", "claudeSessionId": "c9", "startedAt": 1, "endedAt": 2, "cwd": "/work",
        "asking": "which one", "setupRefusal": "no", "diskKeptUntil": 3, "diskGone": "gone", "resumeAt": 4,
        "lastLine": "last", "failure": "boom", "pid": 11, "agentPid": 12, "readAt": 5, "settledAt": 6, "pinnedAt": 7,
        "order": 8, "foldedAt": 9, "replaces": "r1", "replacedBy": "r2", "snoozedUntil": 10, "wokeAt": 13, "rewoundAt": 14,
        "permissionMode": "plan", "fast": true,
        "turns": 3, "ran": true, "parentThreadId": "p1", "rootThreadId": "r0", "attempt": "a1", "costUsd": 0.5
    })
}

#[test]
fn a_folded_thread_row_writes_its_optional_block_before_turns_and_cost_last() {
    let read: ThreadView = reads_exact(folded_row());
    assert_eq!(read.cost_usd.map(|c| c.0), Some(0.5));
}

#[test]
fn thread_facts_write_a_folded_row_then_the_model_facts() {
    let mut row = folded_row();
    let facts = row.as_object_mut().unwrap();
    facts.shift_remove("diskKeptUntil");
    facts.shift_remove("diskGone");
    facts.insert("diskGone".into(), json!("gone"));
    facts.insert("diskKeptUntil".into(), json!(3));
    facts.insert("model".into(), json!("opus"));
    facts.insert("effort".into(), json!("high"));
    facts.insert("contextWindow".into(), json!("1m"));
    facts.insert("turnId".into(), json!("turn_1"));
    let read: ThreadFacts = reads_exact(row);
    assert_eq!(read.turn_id.as_deref(), Some("turn_1"));
}

/// A workspace's view is written as records.ts:51-57 builds it: `folder`, then the home spread, then `id`.
#[test]
fn a_workspace_view_writes_folder_and_home_before_its_id() {
    use wsp_wire::views::workspace_more::WorkspaceView;
    let view = json!({
        "folder": "/root/lab", "home": "/root", "id": "w1", "name": "lab", "machineId": "m1", "phase": "running", "kind": "cloud",
        "golden": "g", "createdAt": "2026-10-26T00:00:00.000Z", "project": { "id": "p1", "name": "lab", "path": "/root/lab", "computer": "here" }
    });
    let read: WorkspaceView = reads_exact(view);
    assert_eq!(read.folder.as_deref(), Some("/root/lab"));
}

/// The rows a turn records are literals in turns.ts:317-565, so a row writes `line` right after `threadId`, a start
/// writes `attachments` after `startedBy`, a delta writes `exitCode`, `durationMs` and `bytes` in that order, and the
/// person's row (threads.ts:606) writes `text` before `threadId`.
#[test]
fn the_rows_a_turn_records_write_in_the_order_turns_ts_builds_them() {
    use wsp_wire::views::session_events::{SessionDeltaEvent, SessionNotifyEvent, SessionStartEvent};
    use wsp_wire::views::session_events_more::{SessionCompactedEvent, SessionRetryEvent, SessionSubagentEvent};
    let ids = |extra: serde_json::Value| {
        let mut row = json!({ "workspaceId": "w", "sessionId": "s", "turnId": "u", "threadId": "t" });
        row.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        row
    };
    let _: SessionDeltaEvent = reads_exact(ids(json!({
        "line": 3, "kind": "tool_result", "text": "ok", "messageId": "m", "toolName": "Bash", "toolUseId": "tu", "isError": false,
        "parentToolUseId": "tp", "cwd": "/w", "exitCode": 0, "durationMs": 12, "bytes": 4, "patch": []
    })));
    let _: SessionStartEvent = reads_exact(ids(json!({
        "prompt": "go", "requestId": "r", "afterCut": true, "afterLimit": 5, "opensThread": true, "startedBy": "person", "attachments": [],
        "model": "opus", "cwd": "/w", "agent": "claude", "tools": ["Bash"]
    })));
    let _: SessionSubagentEvent =
        reads_exact(ids(json!({ "line": 1, "task": "a", "state": "running", "parentToolUseId": "tp", "title": "T" })));
    let _: SessionCompactedEvent = reads_exact(ids(json!({ "line": 1, "before": 10, "after": 5 })));
    let _: SessionRetryEvent = reads_exact(ids(json!({ "line": 1, "attempt": 1, "of": 3, "why": "overloaded" })));
    let notify = json!({ "workspaceId": "w", "sessionId": "s", "turnId": "u", "text": "told", "threadId": "t", "notify": "me" });
    let _: SessionNotifyEvent = reads_exact(notify);
}
