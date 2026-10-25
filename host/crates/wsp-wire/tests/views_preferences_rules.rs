// SPDX-License-Identifier: AGPL-3.0-only
//! The preferences record held to the exact JSON text the TS writes: the defaults, a stored record read over them,
//! and a patch landed on one, keys in the order applyPreferencesPatch writes them (the order the host sends), a key a
//! null took away and set again going last. spec/preferences.md lists the rule each test pins.

#[path = "rules/oracle.rs"]
mod oracle;

use oracle::{cases, of, text};
use serde_json::Value;
use std::collections::HashMap;
use wsp_wire::types::general_prefs::*;
use wsp_wire::types::thread_defaults::{patched_fields, AgentDefaults, AgentDefaultsPatch};
use wsp_wire::views::preferences::*;

#[test]
fn the_defaults_are_the_ts_record_key_for_key_in_its_order() {
    let (_, want) = &cases("default_preferences_text")[0];
    assert_eq!(serde_json::to_string(&default_preferences()).unwrap(), text(want));
}

#[test]
fn a_stored_record_reads_over_the_defaults_and_absent_null_or_corrupt_reads_as_them() {
    // Cases: absent, null, {}, labs and a project icon stored, a bad enum, a string, an array, a partial record, and
    // an unknown key; labs is never the stored one, and an unknown key is dropped.
    for (a, want) in cases("preferences_from_text") {
        let stored = if a[0] == "absent" { None } else { Some(&a[0]) };
        assert_eq!(serde_json::to_string(&preferences_from(stored)).unwrap(), text(&want), "stored {:?}", a[0]);
    }
}

#[test]
fn a_patch_lands_field_by_field_in_the_order_apply_preferences_patch_writes_them() {
    // Each case starts from a record with zoom, access, width, target and text size set, so a null that takes one
    // away shows in the text, and a per-workspace map keeps its keys' order.
    for (a, want) in cases("apply_preferences_patch_text") {
        let current: Preferences = of(&a[0]);
        let patch: PreferencesPatch = of(&a[1]);
        assert_eq!(serde_json::to_string(&apply_preferences_patch(&current, &patch)).unwrap(), text(&want), "patch {}", a[1]);
    }
}

#[test]
fn a_zoom_a_null_took_away_and_a_later_patch_sets_again_goes_after_the_others() {
    let (_, want) = &cases("zoom_dropped_then_set_text")[0];
    let base: Preferences = of(&cases("apply_preferences_patch_text")[0].0[0]);
    let drop: PreferencesPatch = of(&serde_json::json!({ "terminalZoom": { "a": null } }));
    let again: PreferencesPatch = of(&serde_json::json!({ "terminalZoom": { "a": 4 } }));
    let twice = apply_preferences_patch(&apply_preferences_patch(&base, &drop), &again);
    assert_eq!(serde_json::to_string(&twice.terminal_zoom).unwrap(), text(want));
}

#[test]
fn a_record_patched_field_by_field_drops_a_null_keeps_an_absent_and_is_none_once_empty() {
    for (a, want) in cases("patched_fields") {
        let kept: Option<AgentDefaults> = if a[0].is_null() { None } else { Some(of(&a[0])) };
        let patch: AgentDefaultsPatch = of(&a[1]);
        let got = patched_fields(kept.as_ref(), &patch);
        assert_eq!(got.map(|g| serde_json::to_value(g).unwrap()).unwrap_or(Value::Null), want, "{a:?}");
    }
}

#[test]
fn the_general_choices_start_at_their_defaults_and_a_patch_lands_each_it_names() {
    let (_, want) = &cases("general_defaults")[0];
    assert_eq!(serde_json::to_value(general_defaults()).unwrap(), *want);
    for (a, want) in cases("patched_general") {
        let current: GeneralPreferences = of(&a[0]);
        let p = &a[1];
        let patch = GeneralPatch {
            on_quit: p.get("onQuit").map(of),
            ask_delete: p.get("askDelete").and_then(Value::as_bool),
            ..Default::default()
        };
        assert_eq!(serde_json::to_string(&patched_general(&current, &patch)).unwrap(), serde_json::to_string(&want).unwrap());
    }
}

#[test]
fn labs_and_the_cloud_are_on_only_for_exactly_1_in_the_host_s_environment() {
    for (f, call) in [("labs_from_env", labs_from_env as fn(&HashMap<String, String>) -> bool), ("cloud_from_env", cloud_from_env)] {
        for (a, want) in cases(f) {
            let env: HashMap<String, String> = a[0].as_object().unwrap().iter().map(|(k, v)| (k.clone(), text(v))).collect();
            assert_eq!(call(&env), want.as_bool().unwrap(), "{f} {a:?}");
        }
    }
}

#[test]
fn each_line_reads_word_for_word_as_the_ts_writes_it() {
    for (f, call) in [
        ("editorOpensHereLine", editor_opens_here_line as fn(&str) -> String),
        ("sshIncludeLine", ssh_include_line),
        ("sshCopyHereLine", ssh_copy_here_line),
        ("sshBehindLine", ssh_behind_line),
    ] {
        for (a, want) in cases(f) {
            assert_eq!(call(&text(&a[0])), text(&want), "{f}");
        }
    }
    let (a, want) = &cases("serverIconsLeftLine")[0];
    assert_eq!(server_icons_left_line(&text(&a[0]), &text(&a[1])), text(want));
}
