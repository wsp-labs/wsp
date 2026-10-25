// SPDX-License-Identifier: AGPL-3.0-only
//! The views functions, the written key order and the coordinator's two additions after the fix round (#2445, fix round at
//! 4abcbdf7b). Every expectation is what node 22.23.4 answered from an esbuild bundle of packages/protocol/src at a3b870a6e,
//! or from a copy of the runtime function it names.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use wsp_wire::types::check::{parse, Check};
use wsp_wire::types::thread_defaults::{patched_fields, AgentDefaults, AgentDefaultsPatch};
use wsp_wire::views::preferences::ThreadHead;
use wsp_wire::views::session_events_more::{head_text, SessionEvent};
use wsp_wire::wire::capabilities::Capabilities;

fn name<T>() -> &'static str {
    std::any::type_name::<T>().rsplit("::").next().unwrap_or("?")
}

/// zod read `text` and wrote `wrote` (JSON.stringify of its data).
fn read_as<T: DeserializeOwned + Serialize + Check>(text: &str, wrote: &str) -> Option<String> {
    let input: Value = serde_json::from_str(text).unwrap();
    let label = format!("{} {}", name::<T>(), &text[..text.len().min(120)]);
    match parse::<T>(&input) {
        Ok(read) => match serde_json::to_string(&read) {
            Ok(text) if text == wrote => None,
            Ok(text) => Some(format!("{label}: node wrote {wrote}, here {text}")),
            Err(e) => Some(format!("{label}: node wrote {wrote}, here writing failed: {e}")),
        },
        Err(e) => Some(format!("{label}: node took it and wrote {wrote}, refused here: {} at {:?} {:?}", e.code, e.path, e.message)),
    }
}

fn none_differ(found: Vec<Option<String>>) {
    let differ: Vec<String> = found.into_iter().flatten().collect();
    assert!(differ.is_empty(), "{} answer(s) unlike node's:\n{}", differ.len(), differ.join("\n"));
}

/// engine/src/box-backend.ts:546 builds a box's capabilities as `{ ...BOX_CAPABILITIES, sizes }` with each size
/// `{ name: c.name, cpu: c.cpu, memMb: c.memMb, rateUsdPerHour: c.rateUsdPerHour }`, and `workspaces.ts:77,81` send the
/// object as built; the round fixed where `sizes` stands but `MachineSizeOffer` (capabilities.rs:49) still writes its
/// schema order, `cpu`, `memMb`, `rateUsdPerHour`, `name`. Left from the first review (MINOR 15, nested key missed).
/// Node: JSON.stringify of the object the getter returns for the four BOX_CLASSES.
#[test]
fn a_box_s_size_offers_write_name_first_as_the_box_backend_builds_them() {
    let built = r#"{"liveCloneForks":false,"pauseMode":"disk","replacesMachine":true,"previewUrls":true,"signedUrls":false,"callbackRelay":true,"diskSnapshots":true,"images":true,"snapshotsAnyLife":true,"snapshotListing":true,"templates":true,"kept":false,"copies":true,"ownNetwork":true,"sizes":[{"name":"small","cpu":2,"memMb":4096,"rateUsdPerHour":0.018},{"name":"default","cpu":4,"memMb":8192,"rateUsdPerHour":0.036},{"name":"large","cpu":8,"memMb":16384,"rateUsdPerHour":0.072},{"name":"extra large","cpu":16,"memMb":32768,"rateUsdPerHour":0.2}]}"#;
    none_differ(vec![read_as::<Capabilities>(built, built)]);
}

/// The head of a tool result whose text and whose patch are both past HEAD_RESULT_CHARS, as `headShape`
/// (runtime/src/transcript-reader.ts:77-81) writes it: `{ ...e, text, cut, ...keptPatch(patch) }`. `cut` is a new key
/// after every key the stored row held, and `patchCut` is a new key after it. Node: keys
/// type,workspaceId,sessionId,kind,text,toolUseId,line,bytes,patch,cut,patchCut.
fn head_of_a_cut_text_and_a_cut_patch() -> String {
    let line = format!("\"+{}\"", "p".repeat(99));
    let lines = vec![line; 20].join(",");
    format!(
        r#"{{"type":"session.delta","workspaceId":"w","sessionId":"s","kind":"tool_result","text":"{}","toolUseId":"t","line":4,"bytes":3000,"patch":[{{"path":"a.ts","hunks":[{{"oldStart":1,"oldLines":0,"newStart":1,"newLines":20,"lines":[{lines}]}}]}}],"cut":3000,"patchCut":true}}"#,
        "x".repeat(2048)
    )
}

/// transcript-reader.ts:79-80: when the head cuts the text and the patch, `cut` is appended first and `patchCut` (which
/// `keptPatch` returns, and the stored row lacked) after it. `head_text` moves `cut` to the end whatever follows it
/// (session_capped_event.rs:647-654), so it writes `patchCut` before `cut`. New in the round (coordinator addition 2).
#[test]
fn a_head_that_cuts_the_text_and_the_patch_writes_cut_before_patch_cut() {
    let node = head_of_a_cut_text_and_a_cut_patch();
    let event = parse::<SessionEvent>(&serde_json::from_str::<Value>(&node).unwrap()).unwrap();
    let written = head_text(&event).unwrap();
    assert_eq!(written.len(), node.len(), "the same length");
    let keys = |text: &str| serde_json::from_str::<serde_json::Map<String, Value>>(text).unwrap().keys().cloned().collect::<Vec<_>>();
    assert_eq!(keys(&written), keys(&node));
    assert!(written == node, "same keys and bytes");
}

/// The head of a tool result with a cut text and no patch, as node writes it: `line`, `bytes`, then `cut`
/// (the stored row's keys, then the new one).
fn head_of_a_cut_text() -> String {
    format!(
        r#"{{"type":"session.delta","workspaceId":"w","sessionId":"s","kind":"tool_result","text":"{}","toolUseId":"t","line":4,"bytes":3000,"cut":3000}}"#,
        "x".repeat(2048)
    )
}

/// `sessions.head` (sessions.ts:963) answers `{ facts, events: headShape each, pos, total }`, and `ThreadHead.events`
/// (preferences.ts:323) is where a `cut` rides. The Rust `ThreadHead` writes its events through `SessionEvent`'s own
/// `Serialize`, which puts `cut` after `line` (agent_starting_ms.rs:563), and nothing in the crate routes a head through
/// `head_text` (its only caller is a test). Node writes `"line":4,"bytes":3000,"cut":3000`; here `"line":4,"cut":3000,"bytes":3000`.
#[test]
fn a_thread_head_writes_each_event_with_cut_last() {
    let facts = r#"{"id":"a","workspaceId":"a","harness":"a","startedBy":"person","status":"running","title":"a","sessionId":"a","turns":1,"ran":true}"#;
    let node = head_of_a_cut_text();
    let reply = format!(r#"{{"facts":{facts},"events":[{node}],"pos":5,"total":1}}"#);
    let head = parse::<ThreadHead>(&serde_json::from_str::<Value>(&reply).unwrap()).unwrap();
    let written = serde_json::to_string(&head.events).unwrap();
    let keys =
        |text: &str| serde_json::from_str::<Vec<serde_json::Map<String, Value>>>(text).unwrap()[0].keys().cloned().collect::<Vec<_>>();
    assert_eq!(keys(&written), keys(&format!("[{node}]")));
    assert!(written == format!("[{node}]"), "same keys and bytes");
}

/// thread-defaults.ts:48-55 (`patchedFields`, spec/preferences.md rule 7 "a new key after the kept ones"): kept
/// `{model, access}` and a patch of `{model, effort}` is `{"model":"n","access":"full","effort":"high"}` in node. The
/// round fixed `apply_preferences_patch` by moving it to `Fields::patched`, but `patched_fields` itself (the function the
/// first review's finding names, thread_defaults.rs:698) still reads the merge back into a struct and writes
/// `model, effort, access`. Left from the first review; nothing in the crate calls it but a test.
#[test]
fn patched_fields_writes_a_new_key_after_the_kept_ones() {
    let kept: AgentDefaults = parse(&json!({ "model": "m", "access": "full" })).unwrap();
    let patch: AgentDefaultsPatch = parse(&json!({ "model": "n", "effort": "high" })).unwrap();
    let out = patched_fields(Some(&kept), &patch).unwrap();
    assert_eq!(serde_json::to_string(&out).unwrap(), r#"{"model":"n","access":"full","effort":"high"}"#);
}
