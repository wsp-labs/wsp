// SPDX-License-Identifier: AGPL-3.0-only
//! The thread-defaults functions wsp-threads and wsp-account kept private copies of: modelIdRefusal, accessRefusal,
//! accessWord, withCustomModels, resolveThreadDefaults, markedFor, openDefaults, configDirLaunchRefusal and
//! agentOffLine. Every expected value is what node 22 printed for packages/protocol/src/thread-defaults.ts, run as one
//! esbuild bundle of that file.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use wsp_wire::types::thread_defaults::{AccessChoice, ModelPicker, ProjectOverrides};
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::preferences::preferences_from;
use wsp_words::thread_defaults::*;

fn catalogs() -> BTreeMap<String, HarnessCatalog> {
    let mut all = BTreeMap::new();
    for text in [
        r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
        r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
    ] {
        let catalog: HarnessCatalog = serde_json::from_str(text).unwrap();
        all.insert(catalog.harness.clone(), catalog);
    }
    all
}

// modelIdRefusal over ids typed by hand
#[test]
fn model_id_refusal_reads_ids_as_the_ts_does() {
    for (id, want) in [
        ("", Some("A model id cannot be empty")),
        ("a b", Some("No model id has a space in it")),
        (" ", Some("No model id has a space in it")),
        ("\u{a0}x", Some("No model id has a space in it")),
        ("\u{feff}x", Some("No model id has a space in it")),
        ("\u{85}x", Some("No model id has \"\u{85}\" in it")),
        ("-x", Some("No model id starts with \"-\"")),
        ("claude-opus-4-5[1m]", None),
        ("provider/model:tag", None),
        ("claude-3-5-sonnet@20240620", None),
        ("a$b", Some("No model id has \"$\" in it")),
        ("é", Some("No model id has \"é\" in it")),
        ("a\"b", Some("No model id has \"\\\"\" in it")),
        ("a\u{7}b", Some("No model id has \"\\u0007\" in it")),
        ("a\\b", Some("No model id has \"\\\\\" in it")),
        ("😀", Some("No model id has \"😀\" in it")),
        ("a+b", None),
        ("x\ty", Some("No model id has a space in it")),
    ] {
        assert_eq!(model_id_refusal(id).as_deref(), want, "{id:?}");
    }
}

// accessRefusal for a catalog that maps every word, some, and none
#[test]
fn access_refusal_says_what_the_catalog_takes() {
    for (catalog, word, want) in [
        (
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
            "ask",
            None,
        ),
        (
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
            "auto-edit",
            None,
        ),
        (
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
            "full",
            None,
        ),
        (
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
            "plan",
            None,
        ),
        (
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
            "ask",
            None,
        ),
        (
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
            "auto-edit",
            Some("Codex takes no auto-edit access; it takes ask, full"),
        ),
        (
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
            "full",
            None,
        ),
        (
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
            "plan",
            Some("Codex takes no plan access; it takes ask, full"),
        ),
        (
            r##"{"harness":"x","label":"X Agent","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[]}"##,
            "ask",
            Some("X Agent takes no access of wsp's words"),
        ),
        (
            r##"{"harness":"x","label":"X Agent","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[]}"##,
            "auto-edit",
            Some("X Agent takes no access of wsp's words"),
        ),
        (
            r##"{"harness":"x","label":"X Agent","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[]}"##,
            "full",
            Some("X Agent takes no access of wsp's words"),
        ),
        (
            r##"{"harness":"x","label":"X Agent","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[]}"##,
            "plan",
            Some("X Agent takes no access of wsp's words"),
        ),
    ] {
        let catalog: HarnessCatalog = serde_json::from_str(catalog).unwrap();
        let word: AccessChoice = serde_json::from_value(json!(word)).unwrap();
        assert_eq!(access_refusal(&catalog, word).as_deref(), want);
    }
}

// accessWord: the wsp word for a harness's own mode
#[test]
fn access_word_names_the_word_a_mode_stands_for() {
    for (catalog, mode, want) in [
        (
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
            "default",
            Some("ask"),
        ),
        (
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
            "acceptEdits",
            Some("auto-edit"),
        ),
        (
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
            "bypassPermissions",
            Some("full"),
        ),
        (
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
            "plan",
            Some("plan"),
        ),
        (
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
            "read-only",
            None,
        ),
        (
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
            "full",
            None,
        ),
        (
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
            "nope",
            None,
        ),
        (
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
            "default",
            None,
        ),
        (
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
            "acceptEdits",
            None,
        ),
        (
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
            "bypassPermissions",
            None,
        ),
        (
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
            "plan",
            None,
        ),
        (
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
            "read-only",
            Some("ask"),
        ),
        (
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
            "full",
            Some("full"),
        ),
        (
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
            "nope",
            None,
        ),
        (
            r##"{"harness":"x","label":"X Agent","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[]}"##,
            "default",
            None,
        ),
        (
            r##"{"harness":"x","label":"X Agent","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[]}"##,
            "acceptEdits",
            None,
        ),
        (
            r##"{"harness":"x","label":"X Agent","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[]}"##,
            "bypassPermissions",
            None,
        ),
        (
            r##"{"harness":"x","label":"X Agent","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[]}"##,
            "plan",
            None,
        ),
        (
            r##"{"harness":"x","label":"X Agent","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[]}"##,
            "read-only",
            None,
        ),
        (
            r##"{"harness":"x","label":"X Agent","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[]}"##,
            "full",
            None,
        ),
        (
            r##"{"harness":"x","label":"X Agent","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[]}"##,
            "nope",
            None,
        ),
    ] {
        let catalog: HarnessCatalog = serde_json::from_str(catalog).unwrap();
        let got = access_word(&catalog, mode).map(|w| serde_json::to_value(w).unwrap());
        assert_eq!(got, want.map(|w: &str| json!(w)));
    }
}

// configDirLaunchRefusal and agentOffLine
#[test]
fn the_launch_and_the_off_sentences() {
    assert_eq!(config_dir_launch_refusal("Claude Code", "claude", "/home/me/.c", "it is not a folder"), "Claude Code does not start with its config folder /home/me/.c: it is not a folder. Set another with wsp agents setup claude --config, or put its own back with --reset config.");
    assert_eq!(agent_off_line("Codex", "mac"), "Codex is off on mac, so no thread there runs it");
}

// withCustomModels
#[test]
fn custom_models_are_added_after_the_binarys_and_never_twice() {
    for (catalog, picker, want) in [
        (
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
            None,
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
        ),
        (
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
            Some(r##"{}"##),
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
        ),
        (
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
            Some(r##"{"custom":["mine"]}"##),
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]},{"value":"mine","label":"mine","added":true}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
        ),
        (
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
            Some(r##"{"custom":["opus","mine","old","mine"]}"##),
            r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]},{"value":"mine","label":"mine","added":true},{"value":"mine","label":"mine","added":true}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##,
        ),
        (
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
            None,
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
        ),
        (
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
            Some(r##"{}"##),
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
        ),
        (
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
            Some(r##"{"custom":["mine"]}"##),
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[{"value":"mine","label":"mine","added":true}],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
        ),
        (
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
            Some(r##"{"custom":["opus","mine","old","mine"]}"##),
            r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[{"value":"opus","label":"opus","added":true},{"value":"mine","label":"mine","added":true},{"value":"old","label":"old","added":true},{"value":"mine","label":"mine","added":true}],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##,
        ),
    ] {
        let catalog: HarnessCatalog = serde_json::from_str(catalog).unwrap();
        let picker: Option<ModelPicker> = picker.map(|p| serde_json::from_str(p).unwrap());
        let got = serde_json::to_value(with_custom_models(&catalog, picker.as_ref())).unwrap();
        assert_eq!(got, serde_json::from_str::<Value>(want).unwrap());
    }
}

// resolveThreadDefaults: nothing named
#[test]
fn resolve_0_nothing_named() {
    let prefs = preferences_from(Some(&serde_json::from_str::<Value>(r##"{"agentDefaults":{}}"##).unwrap()));
    let project: Option<ProjectOverrides> = None;
    let catalogs = catalogs();
    let got = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "claude",
        catalog_of: &|id: &str| catalogs.get(id).cloned(),
        runs: None,
        named: None,
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&got).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"claude","from":"catalog"},"model":{"value":"opus","from":"catalog"},"effort":{"value":"high","from":"catalog"},"access":{"value":"ask","mode":"default","from":"catalog"}}"##).unwrap());
    let on: HarnessCatalog = serde_json::from_str(r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##).unwrap();
    let defaults = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "claude",
        catalog_of: &|_: &str| Some(on.clone()),
        runs: None,
        named: Some("claude"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&defaults).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"claude","from":"named"},"model":{"value":"opus","from":"catalog"},"effort":{"value":"high","from":"catalog"},"access":{"value":"ask","mode":"default","from":"catalog"}}"##).unwrap());
    assert_eq!(serde_json::to_value(marked_for(&on, &defaults)).unwrap(), serde_json::from_str::<Value>(r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##).unwrap());
    let open = open_defaults(&on, &defaults);
    assert_eq!((open.model, open.effort), (None, None));
}

// resolveThreadDefaults: person defaults on claude
#[test]
fn resolve_1_person_defaults_on_claude() {
    let prefs = preferences_from(Some(
        &serde_json::from_str::<Value>(r##"{"agentDefaults":{"claude":{"model":"sonnet","effort":"low","access":"full"}}}"##).unwrap(),
    ));
    let project: Option<ProjectOverrides> = None;
    let catalogs = catalogs();
    let got = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "claude",
        catalog_of: &|id: &str| catalogs.get(id).cloned(),
        runs: None,
        named: Some("claude"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&got).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"claude","from":"named"},"model":{"value":"sonnet","from":"default"},"effort":{"value":"low","from":"default"},"access":{"value":"full","mode":"bypassPermissions","from":"default"}}"##).unwrap());
    let on: HarnessCatalog = serde_json::from_str(r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##).unwrap();
    let defaults = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "claude",
        catalog_of: &|_: &str| Some(on.clone()),
        runs: None,
        named: Some("claude"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&defaults).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"claude","from":"named"},"model":{"value":"sonnet","from":"default"},"effort":{"value":"low","from":"default"},"access":{"value":"full","mode":"bypassPermissions","from":"default"}}"##).unwrap());
    assert_eq!(serde_json::to_value(marked_for(&on, &defaults)).unwrap(), serde_json::from_str::<Value>(r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","efforts":["low","high"],"defaultEffort":"low"},{"value":"sonnet","label":"Sonnet","efforts":["low"],"isDefault":true}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low","isDefault":true},{"value":"high","label":"High"}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default"},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass","isDefault":true},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##).unwrap());
    let open = open_defaults(&on, &defaults);
    assert_eq!((open.model, open.effort), (None, None));
}

// resolveThreadDefaults: person default agent codex
#[test]
fn resolve_2_person_default_agent_codex() {
    let prefs = preferences_from(Some(
        &serde_json::from_str::<Value>(
            r##"{"defaultAgent":"codex","agentDefaults":{"codex":{"model":"gpt-x","effort":"high","access":"ask"}}}"##,
        )
        .unwrap(),
    ));
    let project: Option<ProjectOverrides> = None;
    let catalogs = catalogs();
    let got = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "claude",
        catalog_of: &|id: &str| catalogs.get(id).cloned(),
        runs: None,
        named: None,
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&got).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"codex","from":"default"},"model":{"value":"gpt-x","from":"default"},"effort":{"value":"high","from":"default"},"access":{"value":"ask","mode":"read-only","from":"default"}}"##).unwrap());
    let on: HarnessCatalog = serde_json::from_str(r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##).unwrap();
    let defaults = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "codex",
        catalog_of: &|_: &str| Some(on.clone()),
        runs: None,
        named: Some("codex"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&defaults).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"codex","from":"named"},"model":{"value":"gpt-x","from":"default"},"effort":{"value":"high","from":"default"},"access":{"value":"ask","mode":"read-only","from":"default"}}"##).unwrap());
    assert_eq!(serde_json::to_value(marked_for(&on, &defaults)).unwrap(), serde_json::from_str::<Value>(r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only","isDefault":true},{"value":"full","label":"Full"}],"access":{"ask":"read-only","full":"full"}}"##).unwrap());
    let open = open_defaults(&on, &defaults);
    assert_eq!((open.model, open.effort), (Some("gpt-x".to_string()), Some("high".to_string())));
}

// resolveThreadDefaults: project names claude
#[test]
fn resolve_3_project_names_claude() {
    let prefs = preferences_from(Some(&serde_json::from_str::<Value>(r##"{"agentDefaults":{"claude":{"model":"opus"}}}"##).unwrap()));
    let project: Option<ProjectOverrides> =
        Some(serde_json::from_str(r##"{"agent":"claude","model":"sonnet","effort":"low","access":"plan"}"##).unwrap());
    let catalogs = catalogs();
    let got = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "codex",
        catalog_of: &|id: &str| catalogs.get(id).cloned(),
        runs: None,
        named: None,
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&got).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"claude","from":"project"},"model":{"value":"sonnet","from":"project"},"effort":{"value":"low","from":"project"},"access":{"value":"plan","mode":"plan","from":"project"}}"##).unwrap());
    let on: HarnessCatalog = serde_json::from_str(r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##).unwrap();
    let defaults = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "claude",
        catalog_of: &|_: &str| Some(on.clone()),
        runs: None,
        named: Some("claude"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&defaults).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"claude","from":"named"},"model":{"value":"sonnet","from":"project"},"effort":{"value":"low","from":"project"},"access":{"value":"plan","mode":"plan","from":"project"}}"##).unwrap());
    assert_eq!(serde_json::to_value(marked_for(&on, &defaults)).unwrap(), serde_json::from_str::<Value>(r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","efforts":["low","high"],"defaultEffort":"low"},{"value":"sonnet","label":"Sonnet","efforts":["low"],"isDefault":true}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low","isDefault":true},{"value":"high","label":"High"}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default"},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan","isDefault":true}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##).unwrap());
    let open = open_defaults(&on, &defaults);
    assert_eq!((open.model, open.effort), (None, None));
}

// resolveThreadDefaults: project names no agent
#[test]
fn resolve_4_project_names_no_agent() {
    let prefs = preferences_from(Some(&serde_json::from_str::<Value>(r##"{"agentDefaults":{}}"##).unwrap()));
    let project: Option<ProjectOverrides> = Some(serde_json::from_str(r##"{"model":"sonnet","effort":"low"}"##).unwrap());
    let catalogs = catalogs();
    let got = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "claude",
        catalog_of: &|id: &str| catalogs.get(id).cloned(),
        runs: None,
        named: Some("claude"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&got).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"claude","from":"named"},"model":{"value":"sonnet","from":"project"},"effort":{"value":"low","from":"project"},"access":{"value":"ask","mode":"default","from":"catalog"}}"##).unwrap());
    let on: HarnessCatalog = serde_json::from_str(r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##).unwrap();
    let defaults = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "claude",
        catalog_of: &|_: &str| Some(on.clone()),
        runs: None,
        named: Some("claude"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&defaults).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"claude","from":"named"},"model":{"value":"sonnet","from":"project"},"effort":{"value":"low","from":"project"},"access":{"value":"ask","mode":"default","from":"catalog"}}"##).unwrap());
    assert_eq!(serde_json::to_value(marked_for(&on, &defaults)).unwrap(), serde_json::from_str::<Value>(r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","efforts":["low","high"],"defaultEffort":"low"},{"value":"sonnet","label":"Sonnet","efforts":["low"],"isDefault":true}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low","isDefault":true},{"value":"high","label":"High"}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##).unwrap());
    let open = open_defaults(&on, &defaults);
    assert_eq!((open.model, open.effort), (None, None));
}

// resolveThreadDefaults: project names codex but the start names claude
#[test]
fn resolve_5_project_names_codex_but_the_start_names_claude() {
    let prefs = preferences_from(Some(&serde_json::from_str::<Value>(r##"{"agentDefaults":{}}"##).unwrap()));
    let project: Option<ProjectOverrides> = Some(serde_json::from_str(r##"{"agent":"codex","model":"sonnet"}"##).unwrap());
    let catalogs = catalogs();
    let got = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "claude",
        catalog_of: &|id: &str| catalogs.get(id).cloned(),
        runs: None,
        named: Some("claude"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&got).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"claude","from":"named"},"model":{"value":"opus","from":"catalog"},"effort":{"value":"high","from":"catalog"},"access":{"value":"ask","mode":"default","from":"catalog"}}"##).unwrap());
    let on: HarnessCatalog = serde_json::from_str(r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##).unwrap();
    let defaults = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "claude",
        catalog_of: &|_: &str| Some(on.clone()),
        runs: None,
        named: Some("claude"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&defaults).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"claude","from":"named"},"model":{"value":"opus","from":"catalog"},"effort":{"value":"high","from":"catalog"},"access":{"value":"ask","mode":"default","from":"catalog"}}"##).unwrap());
    assert_eq!(serde_json::to_value(marked_for(&on, &defaults)).unwrap(), serde_json::from_str::<Value>(r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##).unwrap());
    let open = open_defaults(&on, &defaults);
    assert_eq!((open.model, open.effort), (None, None));
}

// resolveThreadDefaults: stale model and effort fall through
#[test]
fn resolve_6_stale_model_and_effort_fall_through() {
    let prefs = preferences_from(Some(
        &serde_json::from_str::<Value>(r##"{"agentDefaults":{"claude":{"model":"nope","effort":"nope","access":"plan"}}}"##).unwrap(),
    ));
    let project: Option<ProjectOverrides> = None;
    let catalogs = catalogs();
    let got = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "claude",
        catalog_of: &|id: &str| catalogs.get(id).cloned(),
        runs: None,
        named: Some("claude"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&got).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"claude","from":"named"},"model":{"value":"opus","from":"catalog"},"effort":{"value":"high","from":"catalog"},"access":{"value":"plan","mode":"plan","from":"default"}}"##).unwrap());
    let on: HarnessCatalog = serde_json::from_str(r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##).unwrap();
    let defaults = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "claude",
        catalog_of: &|_: &str| Some(on.clone()),
        runs: None,
        named: Some("claude"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&defaults).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"claude","from":"named"},"model":{"value":"opus","from":"catalog"},"effort":{"value":"high","from":"catalog"},"access":{"value":"plan","mode":"plan","from":"default"}}"##).unwrap());
    assert_eq!(serde_json::to_value(marked_for(&on, &defaults)).unwrap(), serde_json::from_str::<Value>(r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default"},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan","isDefault":true}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##).unwrap());
    let open = open_defaults(&on, &defaults);
    assert_eq!((open.model, open.effort), (None, None));
}

// resolveThreadDefaults: open list takes the project model for its own agent
#[test]
fn resolve_7_open_list_takes_the_project_model_for_its_own_agent() {
    let prefs = preferences_from(Some(&serde_json::from_str::<Value>(r##"{"agentDefaults":{"codex":{"model":"gpt-x"}}}"##).unwrap()));
    let project: Option<ProjectOverrides> = Some(serde_json::from_str(r##"{"agent":"codex","model":"gpt-y","effort":"xhigh"}"##).unwrap());
    let catalogs = catalogs();
    let got = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "codex",
        catalog_of: &|id: &str| catalogs.get(id).cloned(),
        runs: None,
        named: Some("codex"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&got).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"codex","from":"named"},"model":{"value":"gpt-y","from":"project"},"effort":{"value":"xhigh","from":"project"},"access":{"value":"full","mode":"full","from":"catalog"}}"##).unwrap());
    let on: HarnessCatalog = serde_json::from_str(r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##).unwrap();
    let defaults = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "codex",
        catalog_of: &|_: &str| Some(on.clone()),
        runs: None,
        named: Some("codex"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&defaults).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"codex","from":"named"},"model":{"value":"gpt-y","from":"project"},"effort":{"value":"xhigh","from":"project"},"access":{"value":"full","mode":"full","from":"catalog"}}"##).unwrap());
    assert_eq!(serde_json::to_value(marked_for(&on, &defaults)).unwrap(), serde_json::from_str::<Value>(r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##).unwrap());
    let open = open_defaults(&on, &defaults);
    assert_eq!((open.model, open.effort), (Some("gpt-y".to_string()), Some("xhigh".to_string())));
}

// resolveThreadDefaults: open list and a project of no agent
#[test]
fn resolve_8_open_list_and_a_project_of_no_agent() {
    let prefs = preferences_from(Some(
        &serde_json::from_str::<Value>(r##"{"agentDefaults":{"codex":{"model":"gpt-x","effort":"low"}}}"##).unwrap(),
    ));
    let project: Option<ProjectOverrides> = Some(serde_json::from_str(r##"{"model":"gpt-y"}"##).unwrap());
    let catalogs = catalogs();
    let got = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "codex",
        catalog_of: &|id: &str| catalogs.get(id).cloned(),
        runs: None,
        named: Some("codex"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&got).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"codex","from":"named"},"model":{"value":"gpt-x","from":"default"},"effort":{"value":"low","from":"default"},"access":{"value":"full","mode":"full","from":"catalog"}}"##).unwrap());
    let on: HarnessCatalog = serde_json::from_str(r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##).unwrap();
    let defaults = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "codex",
        catalog_of: &|_: &str| Some(on.clone()),
        runs: None,
        named: Some("codex"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&defaults).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"codex","from":"named"},"model":{"value":"gpt-x","from":"default"},"effort":{"value":"low","from":"default"},"access":{"value":"full","mode":"full","from":"catalog"}}"##).unwrap());
    assert_eq!(serde_json::to_value(marked_for(&on, &defaults)).unwrap(), serde_json::from_str::<Value>(r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##).unwrap());
    let open = open_defaults(&on, &defaults);
    assert_eq!((open.model, open.effort), (Some("gpt-x".to_string()), Some("low".to_string())));
}

// resolveThreadDefaults: project agent not run and the default agent run
#[test]
fn resolve_9_project_agent_not_run_and_the_default_agent_run() {
    let prefs = preferences_from(Some(&serde_json::from_str::<Value>(r##"{"defaultAgent":"codex","agentDefaults":{}}"##).unwrap()));
    let project: Option<ProjectOverrides> = Some(serde_json::from_str(r##"{"agent":"nope"}"##).unwrap());
    let catalogs = catalogs();
    let runs = |id: &str| ["codex", "claude"].contains(&id);
    let got = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "claude",
        catalog_of: &|id: &str| catalogs.get(id).cloned(),
        runs: Some(&runs),
        named: None,
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(
        serde_json::to_value(&got).unwrap(),
        serde_json::from_str::<Value>(
            r##"{"agent":{"value":"codex","from":"default"},"access":{"value":"full","mode":"full","from":"catalog"}}"##
        )
        .unwrap()
    );
    let on: HarnessCatalog = serde_json::from_str(r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##).unwrap();
    let defaults = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "codex",
        catalog_of: &|_: &str| Some(on.clone()),
        runs: None,
        named: Some("codex"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(
        serde_json::to_value(&defaults).unwrap(),
        serde_json::from_str::<Value>(
            r##"{"agent":{"value":"codex","from":"named"},"access":{"value":"full","mode":"full","from":"catalog"}}"##
        )
        .unwrap()
    );
    assert_eq!(serde_json::to_value(marked_for(&on, &defaults)).unwrap(), serde_json::from_str::<Value>(r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##).unwrap());
    let open = open_defaults(&on, &defaults);
    assert_eq!((open.model, open.effort), (None, None));
}

// resolveThreadDefaults: default agent not run so the first stands
#[test]
fn resolve_10_default_agent_not_run_so_the_first_stands() {
    let prefs = preferences_from(Some(&serde_json::from_str::<Value>(r##"{"defaultAgent":"codex","agentDefaults":{}}"##).unwrap()));
    let project: Option<ProjectOverrides> = None;
    let catalogs = catalogs();
    let runs = |id: &str| ["claude"].contains(&id);
    let got = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "claude",
        catalog_of: &|id: &str| catalogs.get(id).cloned(),
        runs: Some(&runs),
        named: None,
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&got).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"claude","from":"catalog"},"model":{"value":"opus","from":"catalog"},"effort":{"value":"high","from":"catalog"},"access":{"value":"ask","mode":"default","from":"catalog"}}"##).unwrap());
    let on: HarnessCatalog = serde_json::from_str(r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##).unwrap();
    let defaults = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "claude",
        catalog_of: &|_: &str| Some(on.clone()),
        runs: None,
        named: Some("claude"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&defaults).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"claude","from":"named"},"model":{"value":"opus","from":"catalog"},"effort":{"value":"high","from":"catalog"},"access":{"value":"ask","mode":"default","from":"catalog"}}"##).unwrap());
    assert_eq!(serde_json::to_value(marked_for(&on, &defaults)).unwrap(), serde_json::from_str::<Value>(r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##).unwrap());
    let open = open_defaults(&on, &defaults);
    assert_eq!((open.model, open.effort), (None, None));
}

// resolveThreadDefaults: no catalog for the named agent
#[test]
fn resolve_11_no_catalog_for_the_named_agent() {
    let prefs = preferences_from(Some(&serde_json::from_str::<Value>(r##"{"agentDefaults":{}}"##).unwrap()));
    let project: Option<ProjectOverrides> = None;
    let catalogs = catalogs();
    let got = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "claude",
        catalog_of: &|id: &str| catalogs.get(id).cloned(),
        runs: None,
        named: Some("nope"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(
        serde_json::to_value(&got).unwrap(),
        serde_json::from_str::<Value>(r##"{"agent":{"value":"nope","from":"named"}}"##).unwrap()
    );
}

// resolveThreadDefaults: access word the row maps to no mode
#[test]
fn resolve_12_access_word_the_row_maps_to_no_mode() {
    let prefs = preferences_from(Some(&serde_json::from_str::<Value>(r##"{"agentDefaults":{"codex":{"access":"plan"}}}"##).unwrap()));
    let project: Option<ProjectOverrides> = None;
    let catalogs = catalogs();
    let got = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "codex",
        catalog_of: &|id: &str| catalogs.get(id).cloned(),
        runs: None,
        named: Some("codex"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(
        serde_json::to_value(&got).unwrap(),
        serde_json::from_str::<Value>(
            r##"{"agent":{"value":"codex","from":"named"},"access":{"value":"full","mode":"full","from":"catalog"}}"##
        )
        .unwrap()
    );
    let on: HarnessCatalog = serde_json::from_str(r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##).unwrap();
    let defaults = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "codex",
        catalog_of: &|_: &str| Some(on.clone()),
        runs: None,
        named: Some("codex"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(
        serde_json::to_value(&defaults).unwrap(),
        serde_json::from_str::<Value>(
            r##"{"agent":{"value":"codex","from":"named"},"access":{"value":"full","mode":"full","from":"catalog"}}"##
        )
        .unwrap()
    );
    assert_eq!(serde_json::to_value(marked_for(&on, &defaults)).unwrap(), serde_json::from_str::<Value>(r##"{"harness":"codex","label":"Codex","source":"table","version":null,"models":[],"efforts":[],"contextWindows":[],"steers":false,"renames":false,"images":false,"permissionModes":[{"value":"read-only","label":"Read only"},{"value":"full","label":"Full","isDefault":true}],"access":{"ask":"read-only","full":"full"}}"##).unwrap());
    let open = open_defaults(&on, &defaults);
    assert_eq!((open.model, open.effort), (None, None));
}

// resolveThreadDefaults: custom model as the person default
#[test]
fn resolve_13_custom_model_as_the_person_default() {
    let prefs = preferences_from(Some(
        &serde_json::from_str::<Value>(r##"{"agentDefaults":{"claude":{"model":"mine","models":{"custom":["mine"]}}}}"##).unwrap(),
    ));
    let project: Option<ProjectOverrides> = None;
    let catalogs = catalogs();
    let got = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "claude",
        catalog_of: &|id: &str| catalogs.get(id).cloned(),
        runs: None,
        named: Some("claude"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&got).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"claude","from":"named"},"model":{"value":"opus","from":"catalog"},"effort":{"value":"high","from":"catalog"},"access":{"value":"ask","mode":"default","from":"catalog"}}"##).unwrap());
    let on: HarnessCatalog = serde_json::from_str(r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","isDefault":true,"efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]},{"value":"mine","label":"mine","added":true}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##).unwrap();
    let defaults = resolve_thread_defaults(&DefaultsAsk {
        first_agent: "claude",
        catalog_of: &|_: &str| Some(on.clone()),
        runs: None,
        named: Some("claude"),
        project: project.as_ref(),
        prefs: &prefs,
    });
    assert_eq!(serde_json::to_value(&defaults).unwrap(), serde_json::from_str::<Value>(r##"{"agent":{"value":"claude","from":"named"},"model":{"value":"mine","from":"default"},"effort":{"value":"high","from":"catalog"},"access":{"value":"ask","mode":"default","from":"catalog"}}"##).unwrap());
    assert_eq!(serde_json::to_value(marked_for(&on, &defaults)).unwrap(), serde_json::from_str::<Value>(r##"{"harness":"claude","label":"Claude Code","source":"harness","version":"2.1.0","models":[{"value":"opus","label":"Opus","efforts":["low","high"],"defaultEffort":"high"},{"value":"sonnet","label":"Sonnet","efforts":["low"]},{"value":"mine","label":"mine","added":true,"isDefault":true}],"legacyModels":[{"value":"old","label":"Old"}],"efforts":[{"value":"low","label":"Low"},{"value":"high","label":"High","isDefault":true}],"contextWindows":[],"steers":true,"renames":true,"images":true,"permissionModes":[{"value":"default","label":"Default","isDefault":true},{"value":"acceptEdits","label":"Accept edits"},{"value":"bypassPermissions","label":"Bypass"},{"value":"plan","label":"Plan"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions","plan":"plan"}}"##).unwrap());
    let open = open_defaults(&on, &defaults);
    assert_eq!((open.model, open.effort), (None, None));
}
