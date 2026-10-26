// SPDX-License-Identifier: AGPL-3.0-only
// The #2449 review of every test the implementation pass and the drift pass changed (name: tests). Each test below
// fails on 52ef71b1e and holds the Rust to what the TS at 65f6b89c7 did under node on the same input; the node
// output is quoted at each test.
#![allow(clippy::result_large_err, clippy::type_complexity, clippy::too_many_arguments, clippy::large_enum_variant, dead_code)]

#[path = "cli_b/common.rs"]
mod common;

use serde_json::Value;
use wsp_cli::verbs::client::{has_tool, BoxFuture, Frame};
use wsp_cli::verbs::verbs_for;
use wsp_wire::views::preferences::ThreadFacts;

/// The ops the Rust tool call of one recorded case sent to the host, in order, `events.subscribe` among them.
fn ops(unit: &str, tool: &str, name: &str) -> Vec<String> {
    let c = common::case(unit, tool, name);
    let table = verbs_for(c.cloud);
    let entry = table.iter().find(|v| v.name().replace(' ', "_") == tool).unwrap_or_else(|| panic!("no entry for {tool}"));
    let t = has_tool(entry).unwrap_or_else(|| panic!("{tool} has no tool"));
    let host = common::ScriptedHost::new(c.replies.clone(), c.pushed.clone());
    let deps = common::tool_deps(&c, host.clone());
    let args = match c.tool_arguments.clone() {
        Some(Value::Object(m)) => m,
        _ => Frame::new(),
    };
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
    let call = t.call;
    let answer: BoxFuture<'_, _> = call(args, &deps);
    let _ = rt.block_on(answer);
    host.sent().iter().map(|s| serde_json::from_str::<Value>(s).unwrap()["op"].as_str().unwrap().to_string()).collect()
}

/// Group 12 (`tool_holds` compares the asked ops without `events.subscribe`): mcp-record.test.ts:386 answers
/// `events: async () => {}`, so no recorded tool op can show whether the tool subscribed, and the filter cannot either.
/// workspaces-help.ts:248 awaits `client.events()` in `buildImageAt` before the build, whatever `out` is (the tool
/// passes QUIET, workspaces.ts:593). The Rust tool door calls `build_image_unheard` (workspaces/life.rs:330,
/// workspaces_help/reads.rs:448-453), which never subscribes.
/// Node (the real client's `events()` is one `events.subscribe` per socket), tool `image_build` `{"place":"attic "}`:
///   frames sent: ["places.list","events.subscribe","image.build","image.get"]   (the record holds no events.subscribe)
/// The same for the recorded cases `already` and `no image`.
#[test]
fn the_image_build_tool_subscribes_to_the_host_s_events_before_it_builds() {
    for name in ["built", "already", "no image"] {
        assert_eq!(
            ops("workspaces_life", "image_build", name),
            ["places.list", "events.subscribe", "image.build", "image.get"],
            "image_build / {name}: the ops sent"
        );
    }
}

/// Group 12, the same filter. workspaces-help.ts:787 awaits `client.events()` in the create of a fork, right after
/// `workspaces.landing` and before `workspaces.create`, whatever `out` is (the tool passes QUIET). The Rust tool door
/// calls `create_for_unheard` (workspaces/git.rs:101, workspaces_help/rows.rs:538-541), whose `heard` is false, so
/// `create_follow` (rows.rs:90) skips the subscribe: the fork with no first turn sends none, and one with a first turn
/// subscribes later, in the turn's follow, after `workspaces.create`.
/// Node, tool `fork` `{"workspace":"alpha"}`:
///   ["workspaces.resolve","projects.resolve","workspaces.landing","events.subscribe","workspaces.create"]
/// with a first turn (`first turn with no text`):
///   ["workspaces.resolve","harnesses.list","projects.resolve","workspaces.landing","events.subscribe","workspaces.create","sessions.start"]
/// with `notify` `["zz"]` (`notify names no thread`):
///   ["workspaces.resolve","harnesses.list","projects.resolve","workspaces.landing","events.subscribe","workspaces.create","sessions.list"]
#[test]
fn the_fork_tool_subscribes_to_the_host_s_events_right_after_the_landing_and_before_it_creates() {
    for name in ["forked", "named and sized", "first turn", "first turn with no text", "notify names no thread", "notify names two"] {
        let sent = ops("workspaces_git", "fork", name);
        let landing = sent.iter().position(|o| o == "workspaces.landing").expect("a landing is read");
        assert_eq!(sent.get(landing + 1).map(String::as_str), Some("events.subscribe"), "fork / {name}: the ops sent {sent:?}");
    }
}

/// Drift item 5 (`ThreadFacts` carries `anyOrder` in the fixtures, and `thread_facts_write_a_folded_row_then_the_model_facts`
/// of written_order.rs pins `diskKeptUntil` and `diskGone` inside the row). The host builds a thread's facts at
/// packages/runtime/src/machines/boot.ts:739-757 as `{...thread, ...(sub), ...diskFacts(record), model, effort,
/// contextWindow, turnId}` where `thread` is `foldThreads` over `listedRows`, which never holds `sub`, `diskKeptUntil`
/// or `diskGone` (only `placedRows` adds them, boot.ts:783-797). So a sub-thread's facts write `sub`, then `diskGone`,
/// then `diskKeptUntil` after `costUsd`, and `model` after them. Node (foldThreads of one row, then the spread of
/// boot.ts:748-756) wrote exactly the text below. `ThreadFacts` (views/preferences/preferences_patch.rs:426) writes
/// `sub`, `diskKeptUntil` and `diskGone` where `foldThreads` puts them, between `setupRefusal` and `capped`.
#[test]
fn a_sub_thread_s_facts_write_sub_and_the_disk_facts_after_the_cost() {
    let text = r##"{"id":"th","threadId":"th","workspaceId":"ws","harness":"claude","startedBy":"agent","status":"failed","title":"hi there","sessionId":"turn1","claudeSessionId":"c9","startedAt":1,"endedAt":2,"cwd":"/work","asking":"which","waitingOn":{"threadId":"a","workspaceId":"w","sessionId":"s","title":"t","prompt":{"askId":"x","toolName":"t","toolUseId":"u","input":"i","options":[{"id":"o","label":"l","effect":"answer"}]}},"setupRefusal":"no","capped":{"placeId":"p","place":"pl","running":1,"atOnce":2,"why":"minute"},"limit":{"resetsAt":5},"resumeAt":4,"lastLine":"last","failure":"boom","pid":11,"agentPid":12,"readAt":5,"settledAt":6,"pinnedAt":7,"order":8,"foldedAt":9,"replaces":"r1","replacedBy":"r2","snoozedUntil":10,"wokeAt":13,"section":{"name":"idle","whileState":"x"},"rewoundAt":14,"forkedFrom":{"threadId":"f","turnId":"t","title":"ti"},"permissionMode":"plan","fast":true,"turns":1,"ran":true,"parentThreadId":"p1","rootThreadId":"r0","attempt":"a1","costUsd":0.5,"sub":{"state":"waiting","on":"s","machine":"s","size":{"cpu":7,"memMb":7},"branch":"s"},"diskGone":"gone","diskKeptUntil":3,"model":"opus","effort":"high","contextWindow":"1m","turnId":"turn_1"}"##;
    let read: ThreadFacts = serde_json::from_str(text).expect("a facts row reads");
    assert_eq!(serde_json::to_string(&read).unwrap(), text);
}
