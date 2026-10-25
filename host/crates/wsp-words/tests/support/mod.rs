// SPDX-License-Identifier: AGPL-3.0-only
//! Whole wire objects for tests that set a few fields: each base holds every field the wire type requires, and the
//! test's own fields go over it, so a test reads like its TS case and still parses against the full type.
#![allow(dead_code)]

use serde_json::{json, Value};

/// `base` with every key of `over` set on it, both JSON objects.
pub fn over(mut base: Value, over: Value) -> Value {
    if let (Some(b), Some(o)) = (base.as_object_mut(), over.as_object()) {
        for (k, v) in o {
            b.insert(k.clone(), v.clone());
        }
    }
    base
}

pub fn capabilities() -> Value {
    json!({
        "liveCloneForks": false, "replacesMachine": false, "previewUrls": false, "signedUrls": false, "callbackRelay": false,
        "diskSnapshots": false, "images": false, "snapshotsAnyLife": false, "snapshotListing": false, "templates": false,
        "sizes": [], "kept": false, "copies": false, "ownNetwork": false
    })
}

pub fn place_capacity() -> Value {
    json!({ "cores": 0, "memMb": 0, "memRoomMb": 0, "machineMemMb": 0, "diskFreeBytes": 0, "images": [], "machines": { "running": 0, "paused": 0 } })
}

/// GENERAL_DEFAULTS of protocol/src/general-prefs.ts:51.
pub fn general_preferences() -> Value {
    json!({ "sendWith": "enter", "midTurn": "queue", "notifyNeeds": "notify-sound", "notifyDone": "notify", "planAlerts": true, "settleAfter": "2h", "askDelete": true, "onQuit": "ask", "newThreadIn": "current" })
}

pub fn trip_event() -> Value {
    json!({ "workspaceId": "w", "source": "/s", "dest": "/d", "message": "", "elapsedMs": 0 })
}

pub fn place_view() -> Value {
    json!({ "id": "p", "kind": "computer", "name": "p", "default": false })
}

pub fn workspace_view() -> Value {
    json!({ "id": "w", "name": "w", "machineId": "m", "phase": "running", "golden": "g", "createdAt": "2026-10-01T00:00:00.000Z", "project": { "id": "p", "name": "p", "path": "/p", "computer": "here" } })
}

pub fn thread_view() -> Value {
    json!({ "id": "t", "workspaceId": "w", "harness": "claude", "startedBy": "person", "status": "completed", "title": "T", "sessionId": "s", "turns": 1, "ran": true })
}

pub fn account_limit() -> Value {
    json!({ "key": "k", "agent": "claude", "label": "Claude Max", "windows": [], "readAt": 0, "computers": [] })
}
