// SPDX-License-Identifier: AGPL-3.0-only
//! The preferences record every client reads off the host: what a stored record parses to, how a patch lands on it,
//! and the shapes that carry both. Ported from packages/protocol/test/preferences.test.ts, the views halves of
//! broker-contract.test.ts and the preferences cases of thread-defaults.test.ts, all at a3b870a6e.

use std::collections::HashMap;

use serde_json::{json, Map, Value};
use wsp_wire::types::env::LABS_ENV;
use wsp_wire::types::parse;
use wsp_wire::views::preferences::{
    apply_preferences_patch, default_preferences, labs_from_env, preferences_from, Preferences, PreferencesPatch,
};

/// DEFAULT_PREFERENCES as `JSON.stringify` writes it (evaluated with node off the protocol bundle).
fn defaults() -> Value {
    json!({
        "theme": "system", "lightTheme": "paper", "darkTheme": "graphite", "sidebarMode": "list", "terminalSize": "app",
        "terminalZoom": {}, "access": {}, "projectLook": {}, "projectIcon": {}, "computerLook": {},
        "serverIcons": true, "agentVersions": true, "usageLogs": true, "productUsage": true, "keepAwake": true,
        "transparency": true, "projectOrder": [], "keybindings": {}, "appFont": "", "codeFont": "",
        "agentDefaults": {}, "projectDefaults": {}, "sendWith": "enter", "midTurn": "queue",
        "notifyNeeds": "notify-sound", "notifyDone": "notify", "planAlerts": true, "settleAfter": "2h",
        "askDelete": true, "onQuit": "ask", "newThreadIn": "current", "labs": false
    })
}

/// `{ ...DEFAULT_PREFERENCES, ...over }`.
fn with(over: Value) -> Value {
    with_on(defaults(), over)
}

/// `{ ...base, ...over }`.
fn with_on(base: Value, over: Value) -> Value {
    let mut record: Map<String, Value> = base.as_object().expect("an object").clone();
    for (k, v) in over.as_object().expect("an object") {
        record.insert(k.clone(), v.clone());
    }
    Value::Object(record)
}

/// `const { a, b, ...rest } = base`.
fn without(base: Value, keys: &[&str]) -> Value {
    let mut record: Map<String, Value> = base.as_object().expect("an object").clone();
    for k in keys {
        record.remove(*k);
    }
    Value::Object(record)
}

fn out(p: &Preferences) -> Value {
    serde_json::to_value(p).unwrap()
}

fn has(p: &Preferences, key: &str) -> bool {
    out(p).as_object().unwrap().contains_key(key)
}

fn field(p: &Preferences, key: &str) -> Value {
    out(p).get(key).cloned().unwrap_or(Value::Null)
}

/// A TS patch object literal.
fn patch(v: Value) -> PreferencesPatch {
    serde_json::from_value::<PreferencesPatch>(v.clone()).unwrap_or_else(|e| panic!("patch {v}: {e}"))
}

fn apply(current: &Preferences, v: Value) -> Preferences {
    apply_preferences_patch(current, &patch(v))
}

fn from_stored(v: Value) -> Preferences {
    preferences_from(Some(&v))
}

fn patch_ok(v: Value) -> bool {
    parse::<PreferencesPatch>(&v).is_ok()
}

fn patch_refused(v: Value) -> bool {
    parse::<PreferencesPatch>(&v).is_err()
}

mod the_preferences_record {
    use super::*;

    // preferences.test.ts:8 at a3b870a6e
    #[test]
    fn nothing_stored_a_record_from_an_older_host_and_a_corrupt_one_all_read_as_the_def() {
        assert_eq!(out(&preferences_from(None)), defaults());
        assert_eq!(out(&from_stored(json!({}))), defaults());
        assert_eq!(out(&from_stored(json!({ "theme": "sepia" }))), defaults());
        assert_eq!(out(&from_stored(json!("nonsense"))), defaults());
    }

    // preferences.test.ts:15 at a3b870a6e
    #[test]
    fn a_stored_record_keeps_what_it_has_and_takes_the_defaults_for_the_rest() {
        assert_eq!(
            out(&from_stored(json!({ "theme": "light", "sidebarWidth": 312 }))),
            with(json!({ "theme": "light", "sidebarWidth": 312 }))
        );
    }

    // preferences.test.ts:19 at a3b870a6e
    #[test]
    fn a_patch_lands_field_by_field_a_null_width_clears_the_width_and_the_zoom_lands_pe() {
        let one = apply(&default_preferences(), json!({ "theme": "dark", "sidebarWidth": 300, "terminalZoom": { "ws_a": 2 } }));
        assert_eq!(
            out(&one),
            json!({ "theme": "dark", "lightTheme": "paper", "darkTheme": "graphite", "sidebarMode": "list", "sidebarWidth": 300, "terminalSize": "app", "terminalZoom": { "ws_a": 2 }, "access": {}, "projectLook": {}, "projectIcon": {}, "computerLook": {}, "serverIcons": true, "agentVersions": true, "usageLogs": true, "productUsage": true, "keepAwake": true, "transparency": true, "projectOrder": [], "keybindings": {}, "appFont": "", "codeFont": "", "agentDefaults": {}, "projectDefaults": {}, "sendWith": "enter", "midTurn": "queue", "notifyNeeds": "notify-sound", "notifyDone": "notify", "planAlerts": true, "settleAfter": "2h", "askDelete": true, "onQuit": "ask", "newThreadIn": "current", "labs": false })
        );
        let two = apply(&one, json!({ "terminalZoom": { "ws_b": -1 } }));
        assert_eq!(field(&two, "terminalZoom"), json!({ "ws_a": 2, "ws_b": -1 }));
        let three = apply(&two, json!({ "sidebarWidth": null, "terminalZoom": { "ws_a": null } }));
        assert_eq!(
            out(&three),
            json!({ "theme": "dark", "lightTheme": "paper", "darkTheme": "graphite", "sidebarMode": "list", "terminalSize": "app", "terminalZoom": { "ws_b": -1 }, "access": {}, "projectLook": {}, "projectIcon": {}, "computerLook": {}, "serverIcons": true, "agentVersions": true, "usageLogs": true, "productUsage": true, "keepAwake": true, "transparency": true, "projectOrder": [], "keybindings": {}, "appFont": "", "codeFont": "", "agentDefaults": {}, "projectDefaults": {}, "sendWith": "enter", "midTurn": "queue", "notifyNeeds": "notify-sound", "notifyDone": "notify", "planAlerts": true, "settleAfter": "2h", "askDelete": true, "onQuit": "ask", "newThreadIn": "current", "labs": false })
        );
        assert_eq!(out(&apply(&one, json!({}))), out(&one));
        assert!(patch_ok(json!({ "terminalZoom": { "ws_a": null } })));
    }

    // preferences.test.ts:30 at a3b870a6e
    #[test]
    fn a_reading_or_code_size_lands_from_its_table_and_a_null_puts_each_surface_back_on() {
        let sized = apply(&default_preferences(), json!({ "textSize": 16, "codeSize": 13 }));
        assert_eq!(json!([field(&sized, "textSize"), field(&sized, "codeSize")]), json!([16, 13]));
        assert_eq!(field(&apply(&sized, json!({ "theme": "dark" })), "textSize"), json!(16));
        let cleared = apply(&sized, json!({ "textSize": null, "codeSize": null }));
        assert!(!(has(&cleared, "textSize") || has(&cleared, "codeSize")));
        for off in [json!({ "textSize": 21 }), json!({ "codeSize": 9 }), json!({ "textSize": 14.5 })] {
            assert!(patch_refused(off.clone()), "{off}");
        }
    }

    // preferences.test.ts:39 at a3b870a6e
    #[test]
    fn the_editor_pick_lands_and_stays_through_a_patch_that_names_none_and_an_editor_of() {
        let picked = apply(&default_preferences(), json!({ "editor": "zed" }));
        assert_eq!(out(&picked), with(json!({ "editor": "zed" })));
        assert_eq!(field(&apply(&picked, json!({ "theme": "dark" })), "editor"), json!("zed"));
        assert_eq!(field(&from_stored(json!({ "editor": "cursor" })), "editor"), json!("cursor"));
        assert!(patch_refused(json!({ "editor": "emacs" })));
        assert!(patch_refused(json!({ "editor": "sh -c 'open /'" })));
    }

    // preferences.test.ts:48 at a3b870a6e
    #[test]
    fn each_side_keeps_its_own_theme_pick_a_patch_lands_either_an_old_record_reads_with() {
        let one = apply(&default_preferences(), json!({ "darkTheme": "denim" }));
        assert_eq!(out(&one), with(json!({ "darkTheme": "denim" })));
        let two = apply(&one, json!({ "lightTheme": "linen", "theme": "light" }));
        assert_eq!(out(&two), with(json!({ "theme": "light", "lightTheme": "linen", "darkTheme": "denim" })));
        assert_eq!(out(&apply(&two, json!({ "sidebarWidth": 280 }))), with_on(out(&two), json!({ "sidebarWidth": 280 })));
        // A record from a host older than the picks reads with each side on its default.
        assert_eq!(
            out(&from_stored(json!({ "theme": "dark", "sidebarWidth": 300 }))),
            with(json!({ "theme": "dark", "sidebarWidth": 300 }))
        );
        assert_eq!(field(&from_stored(json!({ "darkTheme": "denim" })), "darkTheme"), json!("denim"));
        // On the wire too: a record an older host sends parses with the side defaults rather than failing whole.
        let older = without(defaults(), &["lightTheme", "darkTheme"]);
        let read = parse::<Preferences>(&with_on(older, json!({ "theme": "dark" }))).unwrap();
        assert_eq!(out(&read), with(json!({ "theme": "dark" })));
        // The shape is the protocol's; which ids exist is the app's, so an id this build does not know still parses.
        assert!(patch_ok(json!({ "lightTheme": "linen", "darkTheme": "some-later-theme" })));
        assert!(patch_refused(json!({ "lightTheme": "" })));
        // The record's defaults never ride in on a patch that names neither pick: they would put a side back unasked.
        let parsed = parse::<PreferencesPatch>(&json!({ "theme": "dark" })).unwrap();
        assert_eq!(serde_json::to_value(&parsed).unwrap(), json!({ "theme": "dark" }));
        assert!(patch_refused(json!({ "darkTheme": 3 })));
        assert!(patch_refused(json!({ "lightTheme": "linen", "accentTheme": "x" })));
    }

    // preferences.test.ts:69 at a3b870a6e
    #[test]
    fn the_access_pick_lands_per_workspace_and_stands_beside_the_rest_a_null_entry_drop() {
        let one = apply(&default_preferences(), json!({ "access": { "ws_a": "bypassPermissions" } }));
        assert_eq!(field(&one, "access"), json!({ "ws_a": "bypassPermissions" }));
        // A pick in one workspace leaves another's alone, and a patch that names none leaves every pick standing.
        let two = apply(&one, json!({ "access": { "ws_b": "acceptEdits" } }));
        assert_eq!(field(&two, "access"), json!({ "ws_a": "bypassPermissions", "ws_b": "acceptEdits" }));
        assert_eq!(field(&apply(&two, json!({ "theme": "dark" })), "access"), field(&two, "access"));
        assert_eq!(field(&apply(&two, json!({ "access": { "ws_a": null } })), "access"), json!({ "ws_b": "acceptEdits" }));
        // A record from a host that kept no picks reads as none, not as undefined a caller has to guard.
        assert_eq!(field(&from_stored(json!({ "theme": "light" })), "access"), json!({}));
    }

    // preferences.test.ts:81 at a3b870a6e
    #[test]
    fn a_project_s_look_lands_per_project_a_null_entry_dropping_that_project_s_and_a_st() {
        let one = apply(&default_preferences(), json!({ "projectLook": { "pr_1": { "icon": "rocket", "hue": "teal" } } }));
        assert_eq!(field(&one, "projectLook"), json!({ "pr_1": { "icon": "rocket", "hue": "teal" } }));
        let two = apply(&one, json!({ "projectLook": { "pr_2": { "hue": "blue" } } }));
        assert_eq!(field(&two, "projectLook"), json!({ "pr_1": { "icon": "rocket", "hue": "teal" }, "pr_2": { "hue": "blue" } }));
        assert_eq!(field(&apply(&two, json!({ "projectLook": { "pr_1": null } })), "projectLook"), json!({ "pr_2": { "hue": "blue" } }));
        assert_eq!(field(&from_stored(json!({ "theme": "light" })), "projectLook"), json!({}));
        assert!(patch_ok(json!({ "projectLook": { "pr_1": { "icon": "rocket" } } })));
        assert!(patch_refused(json!({ "projectLook": { "pr_1": { "icon": "unicorn" } } })));
        assert!(patch_refused(json!({ "projectLook": { "pr_1": { "hue": "blue", "size": 3 } } })));
    }

    // preferences.test.ts:93 at a3b870a6e
    #[test]
    fn a_project_s_image_stands_apart_from_its_look_a_look_s_patch_keeps_it_the_stored() {
        let hash = "c".repeat(64);
        let held = from_stored(
            json!({ "theme": "light", "projectLook": { "pr_1": { "icon": "rocket", "hue": "teal" } }, "projectIcon": { "pr_1": hash } }),
        );
        assert_eq!(field(&held, "projectIcon"), json!({ "pr_1": hash }));
        let looked = apply(&held, json!({ "projectLook": { "pr_1": { "icon": "rocket", "hue": "blue" } } }));
        assert_eq!(field(&looked, "projectIcon"), json!({ "pr_1": hash }));
        let dropped = apply(&held, json!({ "projectLook": { "pr_1": null } }));
        assert_eq!(field(&dropped, "projectIcon"), json!({ "pr_1": hash }));
        assert_eq!(field(&from_stored(json!({ "theme": "light" })), "projectIcon"), json!({}));
        assert!(patch_refused(json!({ "projectIcon": { "pr_1": hash } })));
        let bad = with_on(out(&held), json!({ "projectIcon": { "pr_1": "../../etc/passwd" } }));
        assert!(parse::<Preferences>(&bad).is_err());
        // left to the interface: `Preferences.omit({ projectIcon: true }).parse(held)` strips a key the schema does not
        // know (a command line one version behind); Rust has no omitted schema to parse against.
    }

    // preferences.test.ts:108 at a3b870a6e
    #[test]
    fn a_computer_s_icon_lands_per_computer_a_null_entry_dropping_that_computer_s_and_a() {
        let one = apply(&default_preferences(), json!({ "computerLook": { "pl_1": { "icon": "server" } } }));
        assert_eq!(field(&one, "computerLook"), json!({ "pl_1": { "icon": "server" } }));
        let two = apply(&one, json!({ "computerLook": { "pl_2": { "icon": "laptop" } }, "theme": "dark" }));
        assert_eq!(field(&two, "computerLook"), json!({ "pl_1": { "icon": "server" }, "pl_2": { "icon": "laptop" } }));
        assert_eq!(
            field(&apply(&two, json!({ "projectLook": { "pr_1": { "hue": "blue" } } })), "computerLook"),
            field(&two, "computerLook")
        );
        assert_eq!(
            field(&apply(&two, json!({ "computerLook": { "pl_1": null } })), "computerLook"),
            json!({ "pl_2": { "icon": "laptop" } })
        );
        assert_eq!(
            out(&from_stored(json!({ "theme": "light", "projectLook": { "pr_1": { "icon": "rocket" } } }))),
            with(json!({ "theme": "light", "projectLook": { "pr_1": { "icon": "rocket" } }, "computerLook": {} }))
        );
        assert_eq!(
            field(&from_stored(json!({ "computerLook": { "pl_1": { "icon": "home" } } })), "computerLook"),
            json!({ "pl_1": { "icon": "home" } })
        );
        assert!(patch_ok(json!({ "computerLook": { "pl_1": null } })));
        assert!(patch_refused(json!({ "computerLook": { "pl_1": { "icon": "toaster" } } })));
        assert!(patch_refused(json!({ "computerLook": { "pl_1": {} } })));
        assert!(patch_refused(json!({ "computerLook": { "pl_1": { "icon": "cloud", "hue": "blue" } } })));
    }

    // preferences.test.ts:123 at a3b870a6e
    #[test]
    fn a_recipe_s_icon_lands_per_recipe_a_null_entry_dropping_it_and_a_record_that_neve() {
        assert!(!has(&default_preferences(), "recipeLook"));
        let one = apply(&default_preferences(), json!({ "recipeLook": { "builders": { "icon": "rocket" } } }));
        assert_eq!(field(&one, "recipeLook"), json!({ "builders": { "icon": "rocket" } }));
        let two = apply(&one, json!({ "recipeLook": { "minimal": { "icon": "zap" } }, "theme": "dark" }));
        assert_eq!(field(&two, "recipeLook"), json!({ "builders": { "icon": "rocket" }, "minimal": { "icon": "zap" } }));
        assert_eq!(field(&apply(&two, json!({ "theme": "light" })), "recipeLook"), field(&two, "recipeLook"));
        assert_eq!(
            field(&apply(&two, json!({ "recipeLook": { "builders": null } })), "recipeLook"),
            json!({ "minimal": { "icon": "zap" } })
        );
        assert!(!has(&apply(&default_preferences(), json!({ "theme": "dark" })), "recipeLook"));
        assert!(patch_refused(json!({ "recipeLook": { "builders": { "icon": "toaster" } } })));
    }

    // preferences.test.ts:135 at a3b870a6e
    #[test]
    fn server_icons_are_on_until_the_person_turns_them_off_a_stored_record_without_the() {
        assert_eq!(field(&from_stored(json!({ "theme": "light" })), "serverIcons"), json!(true));
        let off = apply(&default_preferences(), json!({ "serverIcons": false }));
        assert_eq!(field(&off, "serverIcons"), json!(false));
        assert_eq!(field(&apply(&off, json!({ "theme": "dark" })), "serverIcons"), json!(false));
        assert_eq!(field(&from_stored(json!({ "serverIcons": false })), "serverIcons"), json!(false));
        assert_eq!(field(&apply(&off, json!({ "serverIcons": true })), "serverIcons"), json!(true));
        assert!(patch_refused(json!({ "serverIcons": "no" })));
    }

    // preferences.test.ts:145 at a3b870a6e
    #[test]
    fn a_chord_override_lands_per_command_a_null_entry_putting_that_command_back_on_its() {
        let one = apply(&default_preferences(), json!({ "keybindings": { "sidebar.toggle": "mod+shift+b" } }));
        assert_eq!(field(&one, "keybindings"), json!({ "sidebar.toggle": "mod+shift+b" }));
        let two = apply(&one, json!({ "keybindings": { "terminal.toggle": "mod+shift+t" } }));
        assert_eq!(field(&two, "keybindings"), json!({ "sidebar.toggle": "mod+shift+b", "terminal.toggle": "mod+shift+t" }));
        assert_eq!(field(&apply(&two, json!({ "theme": "dark" })), "keybindings"), field(&two, "keybindings"));
        assert_eq!(
            field(&apply(&two, json!({ "keybindings": { "sidebar.toggle": null } })), "keybindings"),
            json!({ "terminal.toggle": "mod+shift+t" })
        );
        // A record from a host that kept no overrides reads as none.
        assert_eq!(field(&from_stored(json!({ "theme": "light" })), "keybindings"), json!({}));
        assert_eq!(
            field(&from_stored(json!({ "keybindings": { "chat.new": "mod+alt+n" } })), "keybindings"),
            json!({ "chat.new": "mod+alt+n" })
        );
        assert!(patch_ok(json!({ "keybindings": { "sidebar.toggle": null } })));
        assert!(patch_refused(json!({ "keybindings": { "sidebar.toggle": 3 } })));
        assert!(patch_refused(json!({ "keybindings": { "sidebar.toggle": "" } })));
    }

    // preferences.test.ts:160 at a3b870a6e
    #[test]
    fn the_app_and_code_fonts_are_a_family_each_empty_for_the_system_stack_and_a_stored() {
        let one = apply(&default_preferences(), json!({ "appFont": "Inter", "codeFont": "JetBrains Mono" }));
        assert_eq!(json!([field(&one, "appFont"), field(&one, "codeFont")]), json!(["Inter", "JetBrains Mono"]));
        assert_eq!(out(&apply(&one, json!({ "theme": "dark" }))), with_on(out(&one), json!({ "theme": "dark" })));
        assert_eq!(out(&apply(&one, json!({ "appFont": "" }))), with_on(out(&one), json!({ "appFont": "" })));
        assert_eq!(out(&from_stored(json!({ "theme": "light" }))), with(json!({ "theme": "light" })));
        assert_eq!(field(&from_stored(json!({ "codeFont": "Hack" })), "codeFont"), json!("Hack"));
        assert!(patch_refused(json!({ "appFont": 3 })));
        assert!(patch_refused(json!({ "codeFont": "x".repeat(129) })));
        assert!(patch_refused(json!({ "uiFont": "Inter" })));
        // On the wire too: a record from an older host parses with the fonts and overrides empty.
        let older = without(defaults(), &["keybindings", "appFont", "codeFont"]);
        assert_eq!(out(&parse::<Preferences>(&older).unwrap()), defaults());
    }

    // preferences.test.ts:175 at a3b870a6e
    #[test]
    fn labs_comes_from_the_host_s_environment_alone_and_no_patch_carries_it() {
        let env = |v: Option<&str>| -> HashMap<String, String> {
            v.map(|v| HashMap::from([(LABS_ENV.to_string(), v.to_string())])).unwrap_or_default()
        };
        assert!(!labs_from_env(&env(None)));
        assert!(!labs_from_env(&env(Some("0"))));
        assert!(labs_from_env(&env(Some("1"))));
        assert!(patch_refused(json!({ "labs": true })));
        let mut on = default_preferences();
        on.labs = true;
        assert_eq!(field(&apply(&on, json!({ "theme": "dark" })), "labs"), json!(true));
    }

    // preferences.test.ts:183 at a3b870a6e
    #[test]
    fn the_patch_shape_refuses_a_value_outside_the_record_s_own() {
        assert!(patch_ok(json!({ "theme": "light" })));
        assert!(patch_refused(json!({ "theme": "sepia" })));
        assert!(patch_refused(json!({ "sidebarWidth": -4 })));
        assert!(patch_refused(json!({ "terminalZoom": { "ws_a": 1.5 } })));
        assert!(patch_ok(json!({ "access": { "ws_a": "acceptEdits" } })));
        assert!(patch_ok(json!({ "access": { "ws_a": null } })));
        assert!(patch_refused(json!({ "access": { "ws_a": 3 } })));
    }

    // preferences.test.ts:193 is left to the wire writer: it asks RuntimeRequest and EventUnion.
}

mod the_last_target_on_the_preferences_record {
    use super::*;

    // preferences.test.ts:203 at a3b870a6e
    #[test]
    fn the_last_target_the_workspace_a_thread_was_started_on_lands_whole_and_a_null_cle() {
        let one = apply(&default_preferences(), json!({ "target": { "workspace": "ws_a" } }));
        assert_eq!(field(&one, "target"), json!({ "workspace": "ws_a" }));
        let two = apply(&one, json!({ "target": { "workspace": "ws_b" } }));
        assert_eq!(field(&two, "target"), json!({ "workspace": "ws_b" }));
        assert_eq!(field(&apply(&two, json!({ "theme": "dark" })), "target"), json!({ "workspace": "ws_b" }));
        assert!(!has(&apply(&two, json!({ "target": null })), "target"));
        assert!(!has(&default_preferences(), "target"));
        assert!(patch_ok(json!({ "target": null })));
        // The per-workspace project pick left with the list: a patch naming it is refused rather than written into a
        // state file nothing reads.
        assert!(patch_refused(json!({ "project": { "ws_a": "spoo" } })));
        assert!(patch_refused(json!({ "target": { "workspace": "ws_a", "project": "spoo" } })));
        assert!(patch_refused(json!({ "target": { "project": "spoo" } })));
    }
}

mod transparency_on_the_preferences_record {
    use super::*;

    // preferences.test.ts:221 at a3b870a6e
    #[test]
    fn is_on_by_default_and_on_for_a_record_written_before_the_switch_and_a_patch_turns() {
        assert_eq!(field(&default_preferences(), "transparency"), json!(true));
        let off = apply(&default_preferences(), json!({ "transparency": false }));
        assert_eq!(field(&off, "transparency"), json!(false));
        assert_eq!(field(&apply(&off, json!({ "theme": "dark" })), "transparency"), json!(false));
        assert_eq!(field(&apply(&off, json!({ "transparency": true })), "transparency"), json!(true));
        let older = without(defaults(), &["transparency"]);
        assert_eq!(field(&preferences_from(Some(&older)), "transparency"), json!(true));
        assert!(patch_refused(json!({ "transparency": "off" })));
    }
}

mod the_dismissed_update_on_the_preferences_record {
    use super::*;

    // preferences.test.ts:234 at a3b870a6e
    #[test]
    fn is_absent_until_the_person_dismisses_a_release_then_holds_that_version_through_a() {
        assert!(default_preferences().update_dismissed.is_none());
        let dismissed = apply(&default_preferences(), json!({ "updateDismissed": "0.3.3" }));
        assert_eq!(out(&dismissed), with(json!({ "updateDismissed": "0.3.3" })));
        assert_eq!(field(&apply(&dismissed, json!({ "theme": "dark" })), "updateDismissed"), json!("0.3.3"));
        assert_eq!(field(&apply(&dismissed, json!({ "updateDismissed": "0.3.4" })), "updateDismissed"), json!("0.3.4"));
        assert_eq!(field(&from_stored(json!({ "updateDismissed": "0.3.3" })), "updateDismissed"), json!("0.3.3"));
        assert!(patch_refused(json!({ "updateDismissed": 3 })));
    }
}

mod the_project_order_on_the_preferences_record {
    use super::*;

    // preferences.test.ts:246 at a3b870a6e
    #[test]
    fn a_project_order_lands_whole_the_list_the_person_dragged() {
        let ordered = apply(&default_preferences(), json!({ "projectOrder": ["pr_b", "pr_a"] }));
        assert_eq!(out(&ordered), with(json!({ "projectOrder": ["pr_b", "pr_a"] })));
        assert_eq!(field(&apply(&ordered, json!({ "projectOrder": ["pr_a"] })), "projectOrder"), json!(["pr_a"]));
        assert_eq!(field(&apply(&ordered, json!({ "theme": "dark" })), "projectOrder"), json!(["pr_b", "pr_a"]));
        assert!(patch_refused(json!({ "projectOrder": [3] })));
        assert!(patch_refused(json!({ "notifySound": false })));
    }
}

mod keeping_the_computer_awake_while_threads_work {
    use super::*;

    // preferences.test.ts:257 at a3b870a6e
    #[test]
    fn is_on_unless_the_person_turns_it_off_a_record_from_before_the_switch_reads_as_on() {
        assert_eq!(field(&default_preferences(), "keepAwake"), json!(true));
        assert_eq!(field(&from_stored(json!({ "theme": "dark" })), "keepAwake"), json!(true));
        let off = apply(&default_preferences(), json!({ "keepAwake": false }));
        assert_eq!(field(&off, "keepAwake"), json!(false));
        assert_eq!(field(&apply(&off, json!({ "theme": "dark" })), "keepAwake"), json!(false));
        assert!(patch_refused(json!({ "keepAwake": "yes" })));
    }
}

mod the_general_choices_on_the_preferences_record {
    use super::*;

    // preferences.test.ts:267 at a3b870a6e
    #[test]
    fn each_takes_its_default_on_a_record_that_names_none_the_owner_s_ruling_among_them() {
        let read = out(&from_stored(json!({ "theme": "dark" })));
        for (key, want) in [
            ("sendWith", json!("enter")),
            ("midTurn", json!("queue")),
            ("notifyNeeds", json!("notify-sound")),
            ("notifyDone", json!("notify")),
            ("planAlerts", json!(true)),
            ("settleAfter", json!("2h")),
            ("askDelete", json!(true)),
            ("onQuit", json!("ask")),
            ("newThreadIn", json!("current")),
        ] {
            assert_eq!(read[key], want, "{key}");
        }
        let older = without(defaults(), &["sendWith", "notifyDone", "onQuit", "newThreadIn"]);
        assert_eq!(out(&parse::<Preferences>(&older).unwrap()), defaults());
    }

    // preferences.test.ts:273 at a3b870a6e
    #[test]
    fn new_thread_opens_on_the_current_project_unless_the_record_stored_ask_every_time() {
        assert_eq!(field(&from_stored(json!({ "newThreadIn": "ask" })), "newThreadIn"), json!("ask"));
    }

    // preferences.test.ts:277 at a3b870a6e
    #[test]
    fn a_patch_lands_each_choice_and_a_later_patch_naming_none_keeps_it() {
        let choices = json!({ "sendWith": "mod-enter", "midTurn": "steer", "notifyNeeds": "sound", "notifyDone": "notify", "planAlerts": false, "settleAfter": "never", "askDelete": false, "onQuit": "stop", "newThreadIn": "ask" });
        let picked = apply(&default_preferences(), choices.clone());
        assert_eq!(out(&picked), with(choices));
        assert_eq!(out(&apply(&picked, json!({ "theme": "dark" }))), with_on(out(&picked), json!({ "theme": "dark" })));
        let read = out(&from_stored(json!({ "settleAfter": "15m", "onQuit": "keep" })));
        assert_eq!(json!([read["settleAfter"], read["onQuit"]]), json!(["15m", "keep"]));
    }

    // preferences.test.ts:284 is left to the words writer: notifyBy.

    // preferences.test.ts:288 at a3b870a6e
    #[test]
    fn the_defaults_the_schema_fills_in_are_the_defaults_the_record_starts_at_from_one() {
        // GENERAL_DEFAULTS' keys, in the TS spread set to undefined: absent on the wire.
        let general =
            ["sendWith", "midTurn", "notifyNeeds", "notifyDone", "planAlerts", "settleAfter", "askDelete", "onQuit", "newThreadIn"];
        let bare = without(out(&default_preferences()), &general);
        assert_eq!(out(&parse::<Preferences>(&bare).unwrap()), out(&default_preferences()));
    }

    // preferences.test.ts:292 at a3b870a6e
    #[test]
    fn a_choice_off_its_table_is_refused() {
        for off in [
            json!({ "sendWith": "shift-enter" }),
            json!({ "midTurn": "interrupt" }),
            json!({ "notifyNeeds": "loud" }),
            json!({ "notifyDone": true }),
            json!({ "planAlerts": "on" }),
            json!({ "settleAfter": "3h" }),
            json!({ "askDelete": 1 }),
            json!({ "onQuit": "hide" }),
            json!({ "newThreadIn": "last" }),
        ] {
            assert!(patch_refused(off.clone()), "{off}");
        }
    }
}

mod the_credential_contracts {
    use super::*;
    use wsp_wire::types::thread_defaults::ProjectOverrides;
    use wsp_wire::views::golden_image::SealedImage;
    use wsp_wire::views::session_events::SessionEvent;
    use wsp_wire::views::workspace::{BranchOfRecord, ProjectView, WorkspaceView};

    fn workspace() -> Value {
        json!({ "id": "ws_1", "name": "fix-login", "machineId": "m_1", "phase": "running", "kind": "cloud", "golden": "snap_1", "createdAt": "2026-10-13T00:00:00.000Z", "project": { "id": "p_1", "name": "lab", "path": "/root/lab", "computer": "boat" } })
    }

    fn project() -> Value {
        json!({ "id": "p_1", "name": "lab", "computer": "boat", "source": { "kind": "github", "repo": "acme/lab" }, "path": "/root/lab", "remote": "https://github.com/acme/lab.git", "defaultBranch": "main", "memoryKey": "-root-lab", "memoryDir": "/root/.wsp/memory/lab", "createdAt": "2026-10-13T00:00:00.000Z" })
    }

    // broker-contract.test.ts:96 at a3b870a6e
    #[test]
    fn thread_waited_is_a_stored_event_and_a_pushed_one() {
        let waited = json!({ "type": "thread.waited", "workspaceId": "ws_1", "sessionId": "s_1", "threadId": "t_1", "ms": 840_000, "said": "your computer was away" });
        let read = parse::<SessionEvent>(&waited).unwrap();
        assert_eq!(serde_json::to_value(&read).unwrap(), waited);
        // left to the wire writer: EventUnion.parse({ ...waited, seq: 4 }) is a thread.waited with ms 840000
        assert!(parse::<SessionEvent>(&without(waited.clone(), &["threadId"])).is_err());
        assert!(parse::<SessionEvent>(&with_on(waited, json!({ "ms": -1 }))).is_err());
    }

    // broker-contract.test.ts:104 at a3b870a6e
    #[test]
    fn a_workspace_s_branch_of_record_is_optional_and_parses_when_present() {
        assert!(parse::<WorkspaceView>(&workspace()).unwrap().branch_of_record.is_none());
        let branch = json!({ "repoId": 1_296_269, "owner": "acme", "name": "lab", "branch": "wsp/fix-login" });
        let read = parse::<WorkspaceView>(&with_on(workspace(), json!({ "branchOfRecord": branch }))).unwrap();
        assert_eq!(serde_json::to_value(&read.branch_of_record).unwrap(), branch);
        assert!(parse::<BranchOfRecord>(&with_on(branch.clone(), json!({ "repoId": 0 }))).is_err());
        let sha = parse::<BranchOfRecord>(&with_on(branch, json!({ "startSha": "4b825dc6" }))).unwrap();
        assert_eq!(sha.start_sha.as_deref(), Some("4b825dc6"));
        let view = parse::<ProjectView>(&with_on(project(), json!({ "trackers": ["acme/issues"] }))).unwrap();
        assert!(!serde_json::to_value(&view).unwrap().as_object().unwrap().contains_key("trackers"));
    }

    // broker-contract.test.ts:113 at a3b870a6e
    #[test]
    fn a_project_s_trackers_live_in_its_preferences_set_kept_and_taken_away_by_a_patch() {
        let overrides = parse::<ProjectOverrides>(&json!({ "trackers": ["acme/issues"] })).unwrap();
        assert_eq!(serde_json::to_value(&overrides).unwrap(), json!({ "trackers": ["acme/issues"] }));
        assert!(parse::<ProjectOverrides>(&json!({ "trackers": ["issues"] })).is_err());
        let set_patch =
            parse::<PreferencesPatch>(&json!({ "projectDefaults": { "p_1": { "trackers": ["acme/issues"], "agent": "claude" } } }))
                .unwrap();
        let set = apply_preferences_patch(&default_preferences(), &set_patch);
        assert_eq!(serde_json::to_value(&set.project_defaults["p_1"]).unwrap(), json!({ "trackers": ["acme/issues"], "agent": "claude" }));
        let clear_patch = parse::<PreferencesPatch>(&json!({ "projectDefaults": { "p_1": { "trackers": null } } })).unwrap();
        let cleared = apply_preferences_patch(&set, &clear_patch);
        assert_eq!(serde_json::to_value(&cleared.project_defaults["p_1"]).unwrap(), json!({ "agent": "claude" }));
    }

    // broker-contract.test.ts:122 at a3b870a6e
    #[test]
    fn the_cloud_card_s_dismissal_is_kept_by_a_patch_and_stays_through_one_that_leaves() {
        let dismissed =
            apply_preferences_patch(&default_preferences(), &parse::<PreferencesPatch>(&json!({ "cloudCardDismissed": true })).unwrap());
        assert_eq!(dismissed.cloud_card_dismissed, Some(true));
        let kept = apply_preferences_patch(&dismissed, &parse::<PreferencesPatch>(&json!({ "theme": "dark" })).unwrap());
        assert_eq!(kept.cloud_card_dismissed, Some(true));
        assert!(default_preferences().cloud_card_dismissed.is_none());
    }

    // broker-contract.test.ts:129 at a3b870a6e
    #[test]
    fn an_image_s_nosecrets_mark_is_optional() {
        // The TS parses the field's schema alone (`SealedImage.shape.noSecrets`); Rust parses a whole sealed image.
        let image = json!({ "name": "base", "version": 1, "hash": "a".repeat(64), "recipeHash": "r1", "logins": [], "sealedAt": "2026-10-13T00:00:00.000Z", "sealedFrom": "boat" });
        let marked = parse::<SealedImage>(&with_on(image.clone(), json!({ "noSecrets": true }))).unwrap();
        assert_eq!(marked.no_secrets, Some(true));
        let unmarked = parse::<SealedImage>(&image).unwrap();
        assert_eq!(unmarked.no_secrets, None);
    }
}

mod the_defaults_on_the_preferences_record {
    use super::*;

    // thread-defaults.test.ts:230 at a3b870a6e
    #[test]
    fn a_patch_lands_field_by_field_a_null_field_drops_that_field_a_null_record_or_an_e() {
        let prefs = |v: Value| apply(&default_preferences(), v);
        let one = prefs(
            json!({ "defaultAgent": "codex", "agentDefaults": { "claude": { "model": "opus", "access": "ask" } }, "projectDefaults": { "p1": { "agent": "codex" } } }),
        );
        assert_eq!(field(&one, "agentDefaults"), json!({ "claude": { "model": "opus", "access": "ask" } }));
        let two = apply(&one, json!({ "agentDefaults": { "claude": { "access": null, "effort": "high" } } }));
        assert_eq!(field(&two, "agentDefaults"), json!({ "claude": { "model": "opus", "effort": "high" } }));
        let three = apply(
            &two,
            json!({ "defaultAgent": null, "agentDefaults": { "claude": { "model": null, "effort": null } }, "projectDefaults": { "p1": null } }),
        );
        assert!(!has(&three, "defaultAgent"));
        assert_eq!(field(&three, "agentDefaults"), json!({}));
        assert_eq!(field(&three, "projectDefaults"), json!({}));
    }

    // thread-defaults.test.ts:241 at a3b870a6e
    #[test]
    fn a_record_from_a_host_older_than_the_defaults_reads_with_none_and_a_project_overr() {
        let older = without(defaults(), &["agentDefaults", "projectDefaults"]);
        assert_eq!(out(&parse::<Preferences>(&older).unwrap()), defaults());
        assert!(patch_refused(json!({ "projectDefaults": { "p1": { "threadsAtOnce": 2 } } })));
        assert!(patch_refused(json!({ "projectDefaults": { "p1": { "program": "/bin/claude" } } })));
        assert!(patch_refused(json!({ "agentDefaults": { "claude": { "access": "bypassPermissions" } } })));
    }
}
