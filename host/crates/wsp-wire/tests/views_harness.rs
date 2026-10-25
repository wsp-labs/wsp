// SPDX-License-Identifier: AGPL-3.0-only
//! What a start runs with, checked against a harness catalog and filled from its marked defaults: `start_picks` and
//! `marked_default`. Ported from the startPicks and markedDefault assertions of
//! packages/protocol/test/thread-defaults.test.ts at a3b870a6e. The catalogs the TS builds through
//! resolveThreadDefaults, markedFor and shapeModels (words-writer functions) are evaluated with node off the
//! protocol bundle and written out whole.

use serde_json::{json, Value};
use wsp_wire::types::harness_picks::marked_default;
use wsp_wire::views::harness::{start_picks, HarnessCatalog, StartPicks};

const CLAUDE_JSON: &str = r#"{"harness":"claude","label":"Claude Code","source":"table","version":null,"models":[{"value":"opus","label":"opus","isDefault":true,"efforts":["low","high"]},{"value":"sonnet","label":"sonnet","efforts":["low","high"],"defaultEffort":"low"},{"value":"haiku","label":"haiku","efforts":[]}],"legacyModels":[{"value":"opus-old","label":"opus-old"}],"efforts":[{"value":"low","label":"low"},{"value":"high","label":"high","isDefault":true}],"contextWindows":[],"permissionModes":[{"value":"default","label":"default"},{"value":"acceptEdits","label":"acceptEdits"},{"value":"bypassPermissions","label":"bypassPermissions","isDefault":true}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions"},"steers":true,"renames":true,"images":true}"#;

/// Node's result for the TS test's catalog, as the keys that differ from CLAUDE.
fn shaped_with_custom_model() -> HarnessCatalog {
    catalog(
        json!({"models":[{"value":"opus","label":"opus","isDefault":true,"efforts":["low","high"]},{"value":"sonnet","label":"sonnet","efforts":["low","high"],"defaultEffort":"low"},{"value":"haiku","label":"haiku","efforts":[]},{"value":"opus-next","label":"opus-next","added":true}]}),
    )
}

/// Node's result for the TS test's catalog, as the keys that differ from CLAUDE.
fn marked_for_custom_model() -> HarnessCatalog {
    catalog(
        json!({"models":[{"value":"opus","label":"opus","efforts":["low","high"]},{"value":"sonnet","label":"sonnet","efforts":["low","high"],"defaultEffort":"low"},{"value":"haiku","label":"haiku","efforts":[]},{"value":"opus-next","label":"opus-next","added":true,"isDefault":true}]}),
    )
}

/// Node's result for the TS test's catalog, as the keys that differ from CLAUDE.
fn marked_for_sonnet_high_ask() -> HarnessCatalog {
    catalog(
        json!({"models":[{"value":"opus","label":"opus","efforts":["low","high"]},{"value":"sonnet","label":"sonnet","efforts":["low","high"],"defaultEffort":"high","isDefault":true},{"value":"haiku","label":"haiku","efforts":[]}],"permissionModes":[{"value":"default","label":"default","isDefault":true},{"value":"acceptEdits","label":"acceptEdits"},{"value":"bypassPermissions","label":"bypassPermissions"}]}),
    )
}

/// Node's result for the TS test's catalog, as the keys that differ from CLAUDE.
fn marked_for_sonnet_high() -> HarnessCatalog {
    catalog(
        json!({"models":[{"value":"opus","label":"opus","efforts":["low","high"]},{"value":"sonnet","label":"sonnet","efforts":["low","high"],"defaultEffort":"high","isDefault":true},{"value":"haiku","label":"haiku","efforts":[]}]}),
    )
}

/// Node's result for the TS test's catalog, as the keys that differ from CLAUDE.
fn marked_for_legacy_model() -> HarnessCatalog {
    catalog(
        json!({"models":[{"value":"opus","label":"opus","efforts":["low","high"]},{"value":"sonnet","label":"sonnet","efforts":["low","high"],"defaultEffort":"low"},{"value":"haiku","label":"haiku","efforts":[]}],"legacyModels":[{"value":"opus-old","label":"opus-old","isDefault":true}]}),
    )
}

/// Node's result for the TS test's catalog, as the keys that differ from CLAUDE.
fn hidden_opus_and_haiku_first() -> HarnessCatalog {
    catalog(
        json!({"models":[{"value":"haiku","label":"haiku","efforts":[],"isDefault":true},{"value":"sonnet","label":"sonnet","efforts":["low","high"],"defaultEffort":"low"}],"hiddenModels":[{"value":"opus","label":"opus","efforts":["low","high"]}]}),
    )
}

/// Node's result for the TS test's catalog, as the keys that differ from CLAUDE.
fn hidden_opus() -> HarnessCatalog {
    catalog(
        json!({"models":[{"value":"sonnet","label":"sonnet","efforts":["low","high"],"defaultEffort":"low","isDefault":true},{"value":"haiku","label":"haiku","efforts":[]}],"hiddenModels":[{"value":"opus","label":"opus","efforts":["low","high"]}]}),
    )
}

/// Evaluated with node: the test's open catalog, which markedFor leaves as it is.
fn open_cursor() -> HarnessCatalog {
    serde_json::from_value(json!({"harness":"cursor","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"permissionModes":[{"value":"read-only","label":"read-only"},{"value":"workspace-write","label":"workspace-write"},{"value":"danger-full-access","label":"danger-full-access","isDefault":true}],"access":{"ask":"workspace-write","full":"danger-full-access","plan":"read-only"},"steers":true,"renames":true,"images":true})).unwrap()
}

fn catalog(over: Value) -> HarnessCatalog {
    let mut whole: Value = serde_json::from_str(CLAUDE_JSON).unwrap();
    for (k, v) in over.as_object().unwrap() {
        whole[k] = v.clone();
    }
    serde_json::from_value(whole).unwrap()
}

/// CLAUDE of the TS test.
fn claude() -> HarnessCatalog {
    catalog(json!({}))
}

fn named(model: &str) -> StartPicks {
    StartPicks { model: Some(model.to_string()), ..StartPicks::default() }
}

fn unnamed() -> StartPicks {
    StartPicks::default()
}

fn ran_with(catalog: &HarnessCatalog, picks: StartPicks) -> Value {
    serde_json::to_value(start_picks(Some(catalog), &picks, true, None).unwrap()).unwrap()
}

fn model_of(catalog: &HarnessCatalog, picks: StartPicks) -> Option<String> {
    start_picks(Some(catalog), &picks, true, None).unwrap().model
}

mod the_model_effort_and_access_a_new_thread_starts_on {
    use super::*;

    // thread-defaults.test.ts:123 at a3b870a6e
    #[test]
    fn a_custom_model_id_passes_the_start_and_stands_as_a_default() {
        let refused = start_picks(Some(&claude()), &named("opus-next"), true, None).unwrap_err();
        assert!(refused.message.contains("is not one claude takes"), "{}", refused.message);
        assert_eq!(model_of(&shaped_with_custom_model(), named("opus-next")).as_deref(), Some("opus-next"));
        // left to the words writer: resolveThreadDefaults' `d.model` is { value: "opus-next", from: "default" }
        assert_eq!(model_of(&marked_for_custom_model(), unnamed()).as_deref(), Some("opus-next"));
    }
}

mod the_marks_a_catalog_carries_for_its_defaults {
    use super::*;

    // thread-defaults.test.ts:157 at a3b870a6e
    #[test]
    fn a_start_that_names_nothing_lands_on_what_the_defaults_resolved_to() {
        assert_eq!(
            ran_with(&marked_for_sonnet_high_ask(), unnamed()),
            json!({ "model": "sonnet", "effort": "high", "permissionMode": "default" })
        );
        assert_eq!(ran_with(&claude(), unnamed()), json!({ "model": "opus", "effort": "high", "permissionMode": "bypassPermissions" }));
    }

    // thread-defaults.test.ts:164 at a3b870a6e
    #[test]
    fn an_effort_picked_moves_a_model_s_own_default_which_reads_ahead_of_the_catalog_s() {
        let marked = marked_for_sonnet_high();
        // left to the words writer: markedFor puts defaultEffort "high" on the sonnet row
        assert_eq!(marked_default(&marked.efforts).map(|o| o.value.as_str()), Some("high"));
    }

    // thread-defaults.test.ts:170 at a3b870a6e
    #[test]
    fn a_legacy_model_kept_as_the_default_is_the_one_an_unnamed_start_runs() {
        let marked = marked_for_legacy_model();
        assert!(marked_default(&marked.models).is_none());
        assert_eq!(model_of(&marked, unnamed()).as_deref(), Some("opus-old"));
    }

    // thread-defaults.test.ts:176 at a3b870a6e
    #[test]
    fn an_agent_whose_cli_takes_any_model_or_effort_starts_on_the_one_set_for_it_and_st() {
        // left to the words writer: resolveThreadDefaults' defaults and openDefaults of this catalog and of CLAUDE
        assert_eq!(model_of(&open_cursor(), named("gpt-x")).as_deref(), Some("gpt-x"));
    }
}

mod the_model_picker_s_list {
    use super::*;

    // thread-defaults.test.ts:192 at a3b870a6e
    #[test]
    fn hidden_models_leave_the_list_the_person_s_order_comes_first_and_a_hidden_default() {
        let shaped = hidden_opus_and_haiku_first();
        // left to the words writer: shapeModels' model values are haiku, sonnet
        assert_eq!(marked_default(&shaped.models).map(|m| m.value.as_str()), Some("haiku"));
    }

    // thread-defaults.test.ts:198 at a3b870a6e
    #[test]
    fn a_hidden_model_still_starts_by_name_off_the_shaped_lists_the_command_line_checks() {
        assert_eq!(model_of(&claude(), named("opus")).as_deref(), Some("opus"));
        let shaped = hidden_opus();
        // left to the words writer: shapeModels' hiddenModels read [["opus", undefined]]
        assert_eq!(model_of(&shaped, named("opus")).as_deref(), Some("opus"));
        assert_eq!(model_of(&shaped, unnamed()).as_deref(), Some("sonnet"));
        let refused = start_picks(Some(&shaped), &named("nope"), true, None).unwrap_err();
        assert!(
            refused
                .message
                .contains(r#"model "nope" is not one claude takes; one of: sonnet (sonnet), haiku (haiku); legacy: opus-old (opus-old)"#),
            "{}",
            refused.message
        );
    }
}
