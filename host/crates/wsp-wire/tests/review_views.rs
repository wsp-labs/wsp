// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the hand-written functions of the views and top-level types, and of the two text rules, against what
//! the TS answers (expected values were run under node 22.23.4, ICU 78.3, and the zod bundle of packages/protocol).
//! Each test fails today for the reason named above it.

use serde_json::{json, Value};
use std::cmp::Ordering;
use wsp_frames::ProcEntry;
use wsp_wire::text::{collate, collator, locale_from_env};
use wsp_wire::types::check::parse;
use wsp_wire::types::harness_picks::efforts_for;
use wsp_wire::types::JsNumber;
use wsp_wire::views::activity_names::proc_name;
use wsp_wire::views::golden_image::{recipe_pins, RecipeDigest};
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::preferences::{apply_preferences_patch, preferences_from, Preferences, PreferencesPatch};
use wsp_wire::views::session_events::{copied_from_of, SessionEvent};

fn read<T: serde::de::DeserializeOwned + wsp_wire::types::check::Check>(v: Value) -> T {
    parse::<T>(&v).unwrap_or_else(|e| panic!("refused {v}: {e:?}"))
}

/// views/preferences.ts:236-246 and thread-defaults.ts:48-55: `patchedFields` spreads the kept record and then sets
/// each patch key, so the kept keys come first in the order they stand and a new key goes last, in the patch's
/// order. Rust merges as JSON and then reads the record back into a struct, so it writes the struct's field order.
/// Node: {"p":{"agent":"claude","access":"plan","model":"opus"}} and
/// {"claude":{"model":"n","access":"full","effort":"high"}}.
#[test]
fn a_patched_record_keeps_its_kept_keys_first_and_puts_new_keys_last_in_the_patch_order() {
    let stored = json!({ "projectDefaults": { "p": { "agent": "claude", "access": "ask" } }, "agentDefaults": { "claude": { "model": "m", "access": "full" } } });
    let base: Preferences = preferences_from(Some(&stored));
    let patch: PreferencesPatch = read(json!({ "projectDefaults": { "p": { "model": "opus", "access": "plan" } }, "agentDefaults": { "claude": { "effort": "high", "model": "n" } } }));
    let out = serde_json::to_value(apply_preferences_patch(&base, &patch)).unwrap();
    assert_eq!(serde_json::to_string(&out["projectDefaults"]).unwrap(), r#"{"p":{"agent":"claude","access":"plan","model":"opus"}}"#);
    assert_eq!(serde_json::to_string(&out["agentDefaults"]).unwrap(), r#"{"claude":{"model":"n","access":"full","effort":"high"}}"#);
}

/// views/activity-names.ts:30 (`NOT_A_NAME`, `git\+`) and :44: Rust's `not_a_name` slices `word[..4]`, which panics
/// when byte 4 falls inside a multi-byte character. Node: procName({comm:"npx",cmdline:"npx 日本"}) is "日本".
#[test]
fn a_package_word_with_a_multi_byte_character_names_the_process_and_does_not_panic() {
    let p: ProcEntry = serde_json::from_value(json!({ "pid": 1, "ppid": 0, "user": "u", "state": "S", "comm": "npx", "cmdline": "npx 日本", "cpu": 0, "rss": 0, "startedAt": 0 })).unwrap();
    assert_eq!(proc_name(&p), "日本");
}

/// views/session-events.ts:665: `"copiedFrom" in e` is true for every event type but session.slate (23 of 24).
/// Rust's `copied_from_of` lists 16 variants and answers None for session.start, .delta, .done, .end, .steer,
/// .notify and .changes, which are the events a fork copies most. Node answers "t0" for each of the seven.
#[test]
fn copied_from_is_read_off_every_event_type_that_carries_it() {
    let events = [
        json!({"type":"session.start","workspaceId":"w","sessionId":"s","copiedFrom":"t0"}),
        json!({"type":"session.delta","workspaceId":"w","sessionId":"s","copiedFrom":"t0","kind":"text","text":"x"}),
        json!({"type":"session.done","workspaceId":"w","sessionId":"s","copiedFrom":"t0","result":{"status":"completed"}}),
        json!({"type":"session.end","workspaceId":"w","sessionId":"s","copiedFrom":"t0","exitCode":1,"sawResult":true}),
        json!({"type":"session.steer","workspaceId":"w","sessionId":"s","copiedFrom":"t0","prompt":"x"}),
        json!({"type":"session.notify","workspaceId":"w","sessionId":"s","copiedFrom":"t0","notify":"x","text":"x"}),
        json!({"type":"session.changes","workspaceId":"w","sessionId":"s","copiedFrom":"t0","from":"x","to":"x","files":[],"moved":[]}),
    ];
    let got: Vec<(String, Option<String>)> = events
        .iter()
        .map(|e| (e["type"].as_str().unwrap().to_string(), copied_from_of(&read::<SessionEvent>(e.clone()))))
        .collect();
    let want: Vec<(String, Option<String>)> = events.iter().map(|e| (e["type"].as_str().unwrap().to_string(), Some("t0".to_string()))).collect();
    assert_eq!(got, want);
}

/// views/golden-image.ts:355-358: `recipePins` writes `{ id, ...pin, road }`, id first. Rust's `SealedPin` declares
/// `id` after the pin's keys (documented in spec/shapes.md, Decision 1). Node writes
/// [{"id":"a","tag":"2"},{"id":"b","tag":"v1","sha256":"s","arch":"x86_64","latest":true,"road":"r"}].
#[test]
fn recipe_pins_are_written_with_the_id_first() {
    let digest: RecipeDigest = read(json!({
        "ticks": [
            { "id": "b", "pin": { "tag": "v1", "sha256": "s", "arch": "x86_64", "latest": true }, "road": "r" },
            { "id": "a", "pin": { "tag": "2" } }
        ],
        "files": []
    }));
    let text = serde_json::to_string(&recipe_pins(&digest, None)).unwrap();
    assert_eq!(text, r#"[{"id":"a","tag":"2"},{"id":"b","tag":"v1","sha256":"s","arch":"x86_64","latest":true,"road":"r"}]"#);
}

/// harness-picks.ts:22-27: `effortsFor` rebuilds the marked option as `{ ...rest, isDefault: true }`, so isDefault is
/// its last key. Rust's `HarnessOption` writes `isDefault` before `short` (documented in spec/harness.md). Node:
/// [{"value":"low","label":"Low","short":"L"},{"value":"high","label":"High","description":"d","short":"H","isDefault":true}].
#[test]
fn the_effort_a_model_marks_default_is_written_with_is_default_last() {
    let catalog: HarnessCatalog = read(json!({
        "harness": "claude", "label": "C", "source": "harness", "version": null,
        "models": [{ "value": "m", "label": "M", "defaultEffort": "high" }],
        "efforts": [{ "value": "low", "label": "Low", "short": "L", "isDefault": true }, { "value": "high", "label": "High", "description": "d", "short": "H" }],
        "contextWindows": [], "permissionModes": [], "steers": false, "renames": false, "images": false
    }));
    let model = catalog.models.first().cloned();
    let text = serde_json::to_string(&efforts_for(&catalog, model.as_ref())).unwrap();
    assert_eq!(text, r#"[{"value":"low","label":"Low","short":"L"},{"value":"high","label":"High","description":"d","short":"H","isDefault":true}]"#);
}

/// text.rs `collate` on ICU4X 2.4 (compiled data) against node 22.23.4 (ICU 78.3, CLDR 48): node orders Han by
/// radical and stroke across all blocks, ICU4X by code point within a block. Node: "中".localeCompare("丽") is 1
/// (U+4E3D is radical 1, U+4E2D radical 2). 4,470 of the 1.1 million adjacent pairs of node's own sort of every code
/// point differ, all of them Han or unassigned code points; every other script, accent, case, punctuation and
/// symbol pair of 700,000 generated pairs agreed.
#[test]
fn han_characters_sort_by_radical_and_stroke_as_node_sorts_them() {
    assert_eq!(collate("中", "\u{4e3d}"), Ordering::Greater);
}

/// text.rs `locale_from_env`: node maps the obsolete language code `iw` to `he` (LANG=iw_IL resolves to "he-IL" and
/// sorts the Hebrew letter before every Latin one); Rust keeps "iw-IL", which ICU4X collates as the root.
/// Node: ["a","א","b"].sort(new Intl.Collator().compare) is א,a,b under LANG=iw_IL.
#[test]
fn an_obsolete_language_code_in_lang_collates_as_its_current_one() {
    let locale = locale_from_env(|name| (name == "LANG").then(|| "iw_IL".to_string()));
    assert_eq!(locale, "he-IL");
    assert_eq!(collator(&locale).compare("\u{5d0}", "a"), Ordering::Less);
}

/// types/js_number.rs `js_string` (and `json_stringify` through it): where two shortest digit strings sit exactly
/// the same distance from the double, JavaScript's `String(n)` takes the even one; Rust's `{:e}` takes the upper.
/// Node: String(1700000000000.03125) is "1700000000000.0312".
#[test]
fn a_tie_between_two_shortest_digit_strings_is_broken_to_the_even_one() {
    assert_eq!(JsNumber(1700000000000.03125).js_string(), "1700000000000.0312");
}

/// types/js_number.rs `Serialize for JsNumber` writes a fraction with serde_json's `serialize_f64`, which switches to
/// exponent form below 1e-5; JavaScript does so only below 1e-6. Node: JSON.stringify(0.000005) is "0.000005".
#[test]
fn a_fraction_between_a_millionth_and_a_hundred_thousandth_is_written_in_decimal() {
    assert_eq!(serde_json::to_string(&JsNumber(0.000005)).unwrap(), "0.000005");
}
