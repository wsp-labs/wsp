// SPDX-License-Identifier: AGPL-3.0-only
//! What the dead-code cut (383708c78) changed in the words the crate ports, each value printed by node 22 for the TS at the
//! cut (65f6b89c7). The wire's place setting word is still `spawn` in Rust where the TS says `max-machines`; the tests name
//! the TS word in their comments and read the Rust variant until the wire's drift pass renames it.

use serde_json::{json, Value};
use wsp_wire::views::place::{PlaceSettingWord, PlaceSettings};
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_words::place_state::{place_setting_dropped, place_setting_named, place_settings_line};
use wsp_words::projects::project_removed_at_provider_line;
use wsp_words::words::image::{image_rebuild_fix, NO_IMAGE_YET, NO_SEALED_IMAGE};
use wsp_words::words::thread::NO_PROVIDER_LINE;
use wsp_words::workspace_state::{workspace_state, WorkspaceStateInput};

fn settings(value: Value) -> PlaceSettings {
    serde_json::from_value(value).unwrap()
}

fn spawn() -> PlaceSettingWord {
    serde_json::from_value(json!("spawn")).unwrap()
}

// projectRemovedAtProviderLine("spoo", "hetzner")
#[test]
fn a_project_removed_at_a_provider_says_nothing_of_it_was_held_there() {
    assert_eq!(project_removed_at_provider_line("spoo", "hetzner"), "spoo is no longer a project on hetzner; nothing of it was held there");
}

// NO_SEALED_IMAGE, NO_IMAGE_YET, imageRebuildFix("boat"), NO_PROVIDER_LINE
#[test]
fn the_image_and_provider_lines_no_longer_name_wsp_init() {
    assert_eq!(NO_SEALED_IMAGE, "no image yet");
    assert_eq!(NO_IMAGE_YET, "no image yet");
    assert_eq!(image_rebuild_fix("boat"), "Seal your image again for boat.");
    assert_eq!(
        NO_PROVIDER_LINE,
        "no machine provider is set up on this computer, so wsp forks no machines here; wsp add <provider> connects one"
    );
}

// placeSettingNamed(settings, "max-machines") and "max-depth"
#[test]
fn the_machines_setting_is_named_by_its_own_part_alone() {
    assert!(place_setting_named(&settings(json!({ "spawn": { "maxMachines": 2 } })), &spawn()));
    assert!(!place_setting_named(&settings(json!({ "spawn": { "maxDepth": 2 } })), &spawn()));
    assert!(!place_setting_named(&settings(json!({})), &spawn()));
    let depth: PlaceSettingWord = serde_json::from_value(json!("max-depth")).unwrap();
    assert!(place_setting_named(&settings(json!({ "spawn": { "maxDepth": 2 } })), &depth));
}

// placeSettingDropped(..., "max-machines"): the TS key order is not kept (the struct writes its own), so the value is compared.
#[test]
fn dropping_the_machines_setting_keeps_every_other_part_and_key() {
    let both = settings(json!({ "spawn": { "spawn": true, "maxMachines": 2, "maxDepth": 1 }, "size": { "cpu": 2, "memMb": 512 } }));
    assert_eq!(
        serde_json::to_value(place_setting_dropped(&both, &spawn())).unwrap(),
        json!({ "size": { "cpu": 2, "memMb": 512 }, "spawn": { "spawn": true, "maxDepth": 1 } })
    );
    let only = settings(json!({ "spawn": { "maxMachines": 2 }, "threads": 3 }));
    assert_eq!(serde_json::to_value(place_setting_dropped(&only, &spawn())).unwrap(), json!({ "threads": 3 }));
    let depth: PlaceSettingWord = serde_json::from_value(json!("max-depth")).unwrap();
    let two = settings(json!({ "spawn": { "maxMachines": 2, "maxDepth": 4 } }));
    assert_eq!(serde_json::to_value(place_setting_dropped(&two, &depth)).unwrap(), json!({ "spawn": { "maxMachines": 2 } }));
    let none = settings(json!({ "threads": 1 }));
    assert_eq!(serde_json::to_value(place_setting_dropped(&none, &spawn())).unwrap(), json!({ "threads": 1 }));
}

// placeSettingsLine({...spoo, capDefault: { threads: 2 }, spawn: { maxMachines: n, maxDepth: 2 }, spawnDefault: { maxMachines: 3, maxDepth: 2 }, settings: { spawn: { maxMachines: n } }})
#[test]
fn a_machines_cap_reads_in_the_cuts_words_whatever_its_count() {
    for (n, want) in [
        (1.0, "spoo: up to 1 machine under a root thread (3 by default), 2 levels deep (the default)"),
        (2.0, "spoo: up to 2 machines under a root thread (3 by default), 2 levels deep (the default)"),
        (0.0, "spoo: up to 0 machines under a root thread (3 by default), 2 levels deep (the default)"),
    ] {
        let place = json!({
            "id": "p_1", "kind": "computer", "name": "spoo", "default": false, "takesForks": false,
            "capDefault": { "threads": 2 },
            "spawn": { "spawn": true, "maxMachines": n, "maxDepth": 2 },
            "spawnDefault": { "spawn": true, "maxMachines": 3, "maxDepth": 2 },
            "settings": { "spawn": { "maxMachines": n } },
        });
        assert_eq!(place_settings_line(&serde_json::from_value(place).unwrap()), want);
    }
}

// workspaceState({phase:"running",machineState:"starting"}) and "gone": the two cases the cut's test round left uncovered
#[test]
fn a_running_workspace_on_a_starting_or_gone_machine_reads_as_waking_or_gone() {
    let input = |machine: &str| WorkspaceStateInput {
        phase: WorkspacePhase::Running,
        machine_state: Some(serde_json::from_value(json!(machine)).unwrap()),
        reach: None,
        wake_refused: None,
    };
    assert_eq!(workspace_state(&input("starting")).as_str(), "waking");
    assert_eq!(workspace_state(&input("gone")).as_str(), "gone");
}
