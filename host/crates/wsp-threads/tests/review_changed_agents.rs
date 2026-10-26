// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the changed `defaults_on_hides_and_adds_the_persons_models_and_hands_the_agents_own_lists_beside`
//! (tests/agents_launch.rs:547). defaultsOn (agents.ts:421-425) hides nothing and hands no `unshaped` lists, so the
//! hide, mark and unshaped assertions left the test. What it keeps is two `contains`; this pins the list whole as
//! `withCustomModels`, `resolveThreadDefaults`, `markedFor` and `openDefaults` give it (checked in vitest against
//! packages/protocol/src: the same three models, the mark still on the catalog's own).
#[path = "support_b/mod.rs"]
mod support;

use serde_json::{json, Value};
use support::agents_ctx::table;
use support::json_of;
use wsp_threads::threads::agents::AgentsArea;
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::preferences::default_preferences;

fn claude_table() -> HarnessCatalog {
    table(
        "claude",
        "Claude Code",
        json!({
            "models": [{ "value": "claude-opus-5-5", "label": "Opus 5.5", "isDefault": true }, { "value": "claude-sonnet-5-5", "label": "Sonnet 5.5" }],
            "efforts": [{ "value": "low", "label": "Low" }, { "value": "high", "label": "High", "isDefault": true }],
            "permissionModes": [
                { "value": "default", "label": "Default" },
                { "value": "acceptEdits", "label": "Accept edits" },
                { "value": "bypassPermissions", "label": "Bypass", "isDefault": true },
            ],
            "access": { "ask": "default", "auto-edit": "acceptEdits", "full": "bypassPermissions" },
            "bypassMode": "bypassPermissions",
        }),
    )
}

#[test]
fn defaults_on_adds_a_custom_model_after_the_binarys_hides_none_and_leaves_the_catalogs_mark_where_it_was() {
    let mut prefs = default_preferences();
    prefs.agent_defaults.insert(
        "claude".to_string(),
        wsp_wire::types::thread_defaults::Fields::new(
            serde_json::from_value(json!({ "models": { "hide": ["claude-opus-5-5"], "custom": ["claude-opus-6-preview"] } })).expect("agent defaults"),
        ),
    );
    let answered = AgentsArea::defaults_on(&claude_table(), &prefs, None);
    let c: Value = json_of(&answered.catalog);
    assert_eq!(
        c["models"],
        json!([
            { "value": "claude-opus-5-5", "label": "Opus 5.5", "isDefault": true },
            { "value": "claude-sonnet-5-5", "label": "Sonnet 5.5" },
            { "value": "claude-opus-6-preview", "label": "claude-opus-6-preview", "added": true },
        ]),
        "hiding is shapeModels' (runtime.ts:1095): the hidden model stays, the mark stays on the catalog's own, the custom id is added"
    );
    assert!(c.get("unshaped").is_none() && c.get("hiddenModels").is_none());
    assert_eq!((answered.open_model, answered.open_effort), (None, None));
}
