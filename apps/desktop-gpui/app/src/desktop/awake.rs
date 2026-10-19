// SPDX-License-Identifier: AGPL-3.0-only
//! Whether this computer is kept from sleeping on its own (`desktop:awake.ts`): only while a thread works on this
//! computer's own workspace and Keep awake is on. One on a box runs whether this computer sleeps or not, and one
//! stopped on a prompt waits on the person. The hold is GPUI's own (`prevent_idle_sleep`): an `NSProcessInfo` activity
//! on macOS, the desktop portal's inhibitor on Linux.

use std::collections::HashSet;

use gpui_kit::{ActivityGuard, App, AsyncApp, Global};
use serde_json::Value;

use crate::screens::sidebar::model::{fold_threads, Row, Status};

/// What the system is told the hold is for.
const REASON: &str = "wsp threads are working on this computer";

/// `awakeWanted`.
pub fn awake_wanted(sessions: &[Value], workspaces: &[Value], keep_awake: bool) -> bool {
    if !keep_awake {
        return false;
    }
    let here: HashSet<&str> = workspaces
        .iter()
        .filter(|w| w.get("kind").and_then(Value::as_str) == Some("local"))
        .filter_map(|w| w.get("id").and_then(Value::as_str))
        .collect();
    let rows: Vec<Row> = sessions.iter().filter_map(Row::of).collect();
    fold_threads(&rows).iter().any(|t| here.contains(t.workspace_id.as_str()) && !t.asks && t.status == Status::Running)
}

/// The hold while one stands, and which ask it answers, so a hold that lands after it was let go is dropped.
#[derive(Default)]
struct Awake {
    wanted: bool,
    asked: u64,
    guard: Option<ActivityGuard>,
}

impl Global for Awake {}

/// Takes the hold while `want` and lets it go once not.
pub fn hold(want: bool, cx: &mut App) {
    let awake = cx.default_global::<Awake>();
    if awake.wanted == want {
        return;
    }
    awake.wanted = want;
    awake.asked += 1;
    if !want {
        awake.guard = None;
        return;
    }
    let asked = awake.asked;
    let held = cx.prevent_idle_sleep(REASON);
    cx.spawn(async move |cx: &mut AsyncApp| {
        let guard = match held.await {
            Ok(guard) => guard,
            Err(e) => {
                super::log::error(&format!("keep awake: {e:#}"));
                return;
            }
        };
        cx.update(|cx| {
            let awake = cx.default_global::<Awake>();
            if awake.asked == asked && awake.wanted {
                awake.guard = Some(guard);
            }
        });
    })
    .detach();
}

/// Whether the hold stands, which the test reads.
pub fn held(cx: &App) -> bool {
    cx.try_global::<Awake>().is_some_and(|a| a.guard.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn workspaces() -> Vec<Value> {
        vec![json!({ "id": "ws_mac", "kind": "local" }), json!({ "id": "ws_box", "kind": "cloud", "place": "pl_box" })]
    }

    fn row(over: Value) -> Value {
        let mut row = json!({ "workspaceId": "ws_mac", "harness": "claude", "status": "running" });
        row.as_object_mut().unwrap().extend(over.as_object().unwrap().clone());
        row
    }

    /// `awake.test.ts:18`.
    #[test]
    fn while_a_thread_works_here_and_the_switch_is_on() {
        assert!(awake_wanted(&[row(json!({ "id": "s1", "threadId": "t1" }))], &workspaces(), true));
        assert!(!awake_wanted(&[row(json!({ "id": "s1", "threadId": "t1" }))], &workspaces(), false));
    }

    /// `awake.test.ts:22`.
    #[test]
    fn not_for_a_thread_on_a_box() {
        assert!(!awake_wanted(&[row(json!({ "id": "s1", "threadId": "t1", "workspaceId": "ws_box" }))], &workspaces(), true));
    }

    #[test]
    fn not_for_a_thread_stopped_on_a_prompt_one_that_finished_or_none() {
        assert!(!awake_wanted(&[row(json!({ "id": "s1", "threadId": "t1", "asking": "Bash: ls" }))], &workspaces(), true));
        assert!(!awake_wanted(&[row(json!({ "id": "s1", "threadId": "t1", "status": "completed", "endedAt": 2 }))], &workspaces(), true));
        assert!(!awake_wanted(&[], &workspaces(), true));
    }

    #[test]
    fn reads_the_threads_latest_turn() {
        let rows = [
            row(json!({ "id": "s1", "threadId": "t1", "status": "completed", "endedAt": 2, "startedAt": 1 })),
            row(json!({ "id": "s2", "threadId": "t1", "startedAt": 3 })),
        ];
        assert!(awake_wanted(&rows, &workspaces(), true));
    }
}
