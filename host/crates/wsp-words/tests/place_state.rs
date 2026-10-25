// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/place-state.test.ts:40-147 (threads at once, the settings line, one rule for each
//! setting, the turn limit), :149-173 (what runs on a place) and :273-294 (a held spawn's words). Skipped:
//! :175-271 (the word a place's row says, six tests, and the placeStateOf and absentComputer lines in them): needs
//! placeStateOf and absentComputer, not in the stubs (they live in workspace-state.ts and absent-computer.ts).

mod support;

use serde_json::{json, Value};
use support::{over, place_view, thread_view, workspace_view};
use wsp_wire::types::check::Check;
use wsp_wire::types::place_state::{AGENTS_ON, CAP_RAISE_ACT, CLOUD_CAP_DEFAULT, NAP_AFTER_MS, TURN_LIMIT_MAX_MS, TURN_WALL_MS};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::place::{PlaceCap, PlaceSettingWord, PlaceSettings, PlaceSettingsAsk, PlaceView};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::session::{ThreadCapWait, ThreadView};
use wsp_wire::views::workspace::WorkspaceView;
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::daemon_version::DAEMON_VERSION;
use wsp_words::place_state::*;
use wsp_words::words::units::fmt_size;

fn wire<T: serde::de::DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).unwrap()
}

fn size(cpu: u32, mem_mb: u32) -> WorkspaceSize {
    wire(json!({ "cpu": cpu, "memMb": mem_mb }))
}

fn place(fields: Value) -> PlaceView {
    wire(over(place_view(), fields))
}

fn settings_ask(v: Value) -> PlaceSettingsAsk {
    wire(v)
}

fn spoo() -> Value {
    over(place_view(), json!({ "id": "p_1", "kind": "computer", "name": "spoo", "default": false, "shape": { "cpu": 2, "memMb": 7885 }, "daemonVersion": DAEMON_VERSION, "cap": { "threads": 2 }, "running": 0 }))
}

fn solari() -> Value {
    over(place_view(), json!({ "id": "solari", "kind": "provider", "name": "solari", "default": false, "cap": { "machines": 3, "spendPerDayUsd": 10 }, "running": 0 }))
}

fn cloud_cap_default() -> Value {
    Value::Object(CLOUD_CAP_DEFAULT.iter().map(|(k, v)| (k.to_string(), json!(v))).collect())
}

fn cap_of(place: &PlaceView, set: Option<&str>) -> Option<PlaceCap> {
    let set = set.map(|s| wire(serde_json::from_str::<Value>(s).unwrap()));
    place_cap_of(place, set.as_ref())
}

fn some_cap(v: Value) -> Option<PlaceCap> {
    Some(wire(v))
}

fn refusal(row: Value, set: Value, reset: &[PlaceSettingWord]) -> Option<String> {
    place_set_refusal(&place(row), &settings_ask(set), reset)
}

fn line(row: Value) -> String {
    place_settings_line(&place(row))
}

mod threads_at_once {
    use super::*;

    #[test]
    fn reads_one_thread_per_2_gb_rounded_at_least_one_and_never_more_than_twice_the_cor() {
        // A 16 GB Mac held six and stopped answering at eight; the owner keeps it at eight. A 4 GB box held two and lost every turn at three.
        assert_eq!(threads_at_once(&size(10, 16384)), 8.0);
        assert_eq!(threads_at_once(&size(2, 7782)), 4.0);
        assert_eq!(threads_at_once(&size(4, 8192)), 4.0);
        assert_eq!(threads_at_once(&size(2, 4096)), 2.0);
        assert_eq!(threads_at_once(&size(1, 8192)), 2.0);
        assert_eq!(threads_at_once(&size(16, 65536)), 32.0);
        assert_eq!(threads_at_once(&size(1, 1024)), 1.0);
    }

    #[test]
    fn gives_a_computer_its_rules_default_off_its_shape_a_cloud_3_machines_and_10_a_day() {
        assert_eq!(cap_of(&place(json!({ "kind": "computer", "shape": { "cpu": 10, "memMb": 16384 } })), None), some_cap(json!({ "threads": 8 })));
        assert_eq!(cap_of(&place(json!({ "kind": "computer" })), None), None);
        assert_eq!(cap_of(&place(json!({ "kind": "computer" })), Some(r#"{"threads":1}"#)), some_cap(json!({ "threads": 1 })));
        assert_eq!(cap_of(&place(json!({ "kind": "computer", "shape": { "cpu": 2, "memMb": 7885 } })), Some(r#"{"threads":1}"#)), some_cap(json!({ "threads": 1 })));
        assert_eq!(CLOUD_CAP_DEFAULT, &[("machines", 3.0), ("spendPerDayUsd", 10.0)]);
        assert_eq!(cap_of(&place(json!({ "kind": "provider" })), None), some_cap(json!({ "machines": 3, "spendPerDayUsd": 10 })));
        assert_eq!(cap_of(&place(json!({ "kind": "provider" })), Some(r#"{"machines":5}"#)), some_cap(json!({ "machines": 5, "spendPerDayUsd": 10 })));
        assert_eq!(cap_of(&place(json!({ "kind": "provider" })), Some(r#"{"spendPerDayUsd":0}"#)), some_cap(json!({ "machines": 3, "spendPerDayUsd": 0 })));
    }

    #[test]
    fn takes_a_sub_threads_size_only_among_the_sizes_that_clouds_own_row_offers_refused() {
        let solari = json!({ "id": "solari", "kind": "provider", "name": "solari", "takesForks": true, "sizes": [{ "cpu": 2, "memMb": 4096, "rateUsdPerHour": 0.1 }] });
        assert_eq!(
            refusal(solari.clone(), json!({ "size": { "cpu": 16, "memMb": 32768 } }), &[]).as_deref(),
            Some("16x32 is not a size this provider offers; the sizes are 2x4 ($0.10/hr). Ask for one of those instead.")
        );
        assert_eq!(refusal(solari.clone(), json!({ "size": { "cpu": 2, "memMb": 4096 } }), &[]), None);
        // A cloud whose row offers no size, one this host holds no backend for, takes no size: the kind's refusal says so.
        let mut no_sizes = solari.clone();
        no_sizes.as_object_mut().unwrap().remove("sizes");
        assert_eq!(
            refusal(no_sizes, json!({ "size": { "cpu": 2, "memMb": 4096 } }), &[]).as_deref(),
            Some("Solari takes machines at once, spend per day, delete after, nap after, turn limit, agents may start agents and levels deep, not size")
        );
        let empty_sizes = over(solari, json!({ "sizes": [] }));
        let said = refusal(empty_sizes, json!({ "size": { "cpu": 2, "memMb": 4096 } }), &[]).unwrap();
        assert!(said.contains(", not size"), "{said}");
    }

    #[test]
    fn says_a_sub_threads_size_in_the_size_words_the_rest_of_wsp_prints() {
        let row = json!({ "kind": "provider", "name": "box", "takesForks": true, "sizes": [{ "cpu": 1, "memMb": 512, "rateUsdPerHour": 0.01 }], "size": { "cpu": 1, "memMb": 512 }, "sizeDefault": { "cpu": 1, "memMb": 512 } });
        let said = line(row);
        let want = format!("size {} (the default)", fmt_size(&size(1, 512), None));
        assert!(said.contains(&want), "{said}");
    }

    #[test]
    fn refuses_a_number_the_rows_kind_does_not_take_set_or_reset_a_set_with_nothing_in_() {
        use PlaceSettingWord::*;
        assert_eq!(
            refusal(json!({ "id": "p_1", "kind": "computer", "name": "spoo" }), json!({ "machines": 2 }), &[]).as_deref(),
            Some("spoo takes threads at once, turn limit, agents may start agents and levels deep, not machines at once")
        );
        assert_eq!(
            refusal(json!({ "id": "solari", "kind": "provider", "name": "solari", "takesForks": true }), json!({ "threads": 2 }), &[]).as_deref(),
            Some("Solari takes machines at once, spend per day, delete after, nap after, turn limit, agents may start agents and levels deep, not threads at once")
        );
        assert_eq!(
            refusal(json!({ "id": HERE_PLACE_ID, "kind": "computer", "name": "mac", "takesForks": false }), json!({ "napMs": null }), &[]).as_deref(),
            Some("mac takes threads at once, turn limit, agents may start agents and levels deep, not nap after")
        );
        assert_eq!(refusal(json!({ "id": "p_1", "kind": "computer", "name": "spoo", "takesForks": true }), json!({ "napMs": null }), &[]), None);
        assert_eq!(
            refusal(json!({ "id": "p_1", "kind": "computer", "name": "spoo", "takesForks": false }), json!({}), &[Spend]).as_deref(),
            Some("spoo takes threads at once, turn limit, agents may start agents and levels deep, not spend per day")
        );
        assert_eq!(refusal(json!({ "id": "p_1", "kind": "computer", "name": "spoo" }), json!({ "threads": 1 }), &[]), None);
        assert_eq!(refusal(json!({ "id": "solari", "kind": "provider", "name": "solari", "takesForks": true }), json!({ "spendPerDayUsd": 0 }), &[]), None);
        assert_eq!(refusal(json!({ "id": "p_1", "kind": "computer", "name": "spoo" }), json!({}), &[Threads]), None);
        assert_eq!(
            refusal(json!({ "id": "p_1", "kind": "computer", "name": "spoo", "takesForks": true }), json!({}), &[]).as_deref(),
            Some("nothing to set on spoo: it takes threads at once, nap after, turn limit, agents may start agents, levels deep, a new name and a new ssh login")
        );
        assert_eq!(
            refusal(json!({ "id": HERE_PLACE_ID, "kind": "computer", "name": "mac", "takesForks": false }), json!({}), &[]).as_deref(),
            Some("nothing to set on mac: it takes threads at once, turn limit, agents may start agents and levels deep")
        );
        // The TS passes { machines: undefined }, which is a set with no key.
        assert_eq!(
            refusal(json!({ "id": "solari", "kind": "provider", "name": "solari", "takesForks": true }), json!({}), &[]).as_deref(),
            Some("nothing to set on Solari: it takes machines at once, spend per day, delete after, nap after, turn limit, agents may start agents and levels deep")
        );
        assert_eq!(
            refusal(json!({ "id": "p_1", "kind": "computer", "name": "spoo" }), json!({ "threads": 2 }), &[Threads]).as_deref(),
            Some("spoo: threads at once is both set and reset; name it once")
        );
        let zero_threads: PlaceCap = wire(json!({ "threads": 0 }));
        assert!(zero_threads.check().is_err());
        let zero_machines: PlaceCap = wire(json!({ "machines": 0, "spendPerDayUsd": 10 }));
        assert!(zero_machines.check().is_err());
        let free_cloud: PlaceCap = wire(json!({ "machines": 1, "spendPerDayUsd": 0 }));
        assert_eq!(free_cloud.check(), Ok(()));
        assert_eq!(serde_json::to_value(&free_cloud).unwrap(), json!({ "machines": 1, "spendPerDayUsd": 0 }));
    }
}

mod what_a_places_settings_read_as_in_a_line {
    use super::*;

    #[test]
    fn says_each_setting_its_kind_takes_at_the_value_it_runs_at_with_the_default_beside() {
        assert_eq!(line(over(spoo(), json!({ "capDefault": { "threads": 2 } }))), "spoo: 2 threads at once (the default)");
        assert_eq!(
            line(over(spoo(), json!({ "cap": { "threads": 1 }, "capDefault": { "threads": 2 }, "settings": { "threads": 1 } }))),
            "spoo: 1 thread at once (2 by default)"
        );
        let nap = json!({ "takesForks": true, "napDefault": NAP_AFTER_MS });
        assert_eq!(
            line(over(over(spoo(), nap.clone()), json!({ "capDefault": { "threads": 2 }, "napMs": NAP_AFTER_MS }))),
            "spoo: 2 threads at once (the default), naps after 20m (the default)"
        );
        assert_eq!(
            line(over(over(spoo(), nap.clone()), json!({ "capDefault": { "threads": 2 }, "napMs": null, "settings": { "napMs": null } }))),
            "spoo: 2 threads at once (the default), never naps (20m by default)"
        );
        assert_eq!(
            line(over(over(spoo(), nap), json!({ "capDefault": { "threads": 2 }, "napMs": 5 * 60_000, "settings": { "napMs": 5 * 60_000 } }))),
            "spoo: 2 threads at once (the default), naps after 5m (20m by default)"
        );
        // The default beside a set value is the one the row carries, which is the host's own and not a constant here.
        assert_eq!(
            line(over(spoo(), json!({ "takesForks": true, "napDefault": 60 * 60_000, "capDefault": { "threads": 2 }, "napMs": 5 * 60_000, "settings": { "napMs": 5 * 60_000 } }))),
            "spoo: 2 threads at once (the default), naps after 5m (60m by default)"
        );
        assert_eq!(
            line(over(solari(), json!({ "cap": { "machines": 5, "spendPerDayUsd": 2.5 }, "capDefault": cloud_cap_default(), "settings": { "machines": 5, "spendPerDayUsd": 2.5 } }))),
            "Solari: 5 machines at once (3 by default), $2.50 a day ($10 by default)"
        );
        assert_eq!(NAP_AFTER_MS, 20 * 60_000);
        let agents_on = serde_json::to_value(AGENTS_ON).unwrap();
        assert_eq!(
            line(over(spoo(), json!({ "capDefault": { "threads": 2 }, "spawn": agents_on, "spawnDefault": agents_on }))),
            "spoo: 2 threads at once (the default), agents may spawn: up to 3 machines (the default), 2 levels deep (the default)"
        );
        let agents_off = over(agents_on.clone(), json!({ "spawn": false }));
        assert_eq!(
            line(over(spoo(), json!({ "capDefault": { "threads": 2 }, "spawn": agents_off, "spawnDefault": agents_on, "settings": { "spawn": { "spawn": false } } }))),
            "spoo: 2 threads at once (the default), agents may not spawn (on, up to 3 by default), 2 levels deep (the default)"
        );
    }
}

mod one_rule_for_each_setting {
    use super::*;

    #[test]
    fn runs_a_workspace_under_its_own_value_else_its_places_else_the_default_a_value_th() {
        assert_eq!(setting_for(Some(&Some(5)), Some(&Some(10)), &Some(20)), Some(5));
        assert_eq!(setting_for(Some(&None), Some(&Some(10)), &Some(20)), None);
        assert_eq!(setting_for(None, Some(&None), &Some(20)), None);
        assert_eq!(setting_for(None, Some(&Some(10)), &Some(20)), Some(10));
        assert_eq!(setting_for::<Option<i64>>(None, None, &Some(20)), Some(20));
    }

    #[test]
    fn turns_the_minutes_a_person_names_into_the_window_none_of_them_never_napping() {
        assert_eq!(nap_ms_of(0.0), None);
        assert_eq!(nap_ms_of(45.0), Some(2_700_000.0));
    }

    #[test]
    fn says_which_settings_a_place_takes_the_nap_only_where_it_forks() {
        use PlaceSettingWord::*;
        assert!(!place_takes(&place(json!({ "kind": "computer", "takesForks": false })), &Nap));
        assert!(place_takes(&place(json!({ "kind": "computer", "takesForks": true })), &Nap));
        assert!(place_takes(&place(json!({ "kind": "computer", "takesForks": false })), &Spawn));
        assert!(!place_takes(&place(json!({ "kind": "provider", "takesForks": true })), &Threads));
    }
}

mod the_turn_limit {
    use super::*;

    const HOUR: u64 = 3_600_000;

    fn limit(kind: PlaceKind, settings: Value) -> Option<f64> {
        place_turn_limit(&kind, &wire::<PlaceSettings>(settings))
    }

    #[test]
    fn is_off_on_a_computer_the_person_owns_and_six_hours_on_a_cloud_until_the_person_s() {
        assert_eq!(limit(PlaceKind::Computer, json!({})), None);
        assert_eq!(limit(PlaceKind::Provider, json!({})), Some((6 * HOUR) as f64));
        assert_eq!(TURN_WALL_MS, 6 * HOUR);
    }

    #[test]
    fn runs_at_what_the_person_set_on_either_kind_off_included() {
        assert_eq!(limit(PlaceKind::Computer, json!({ "turnLimitMs": 2 * HOUR })), Some((2 * HOUR) as f64));
        assert_eq!(limit(PlaceKind::Provider, json!({ "turnLimitMs": 12 * HOUR })), Some((12 * HOUR) as f64));
        assert_eq!(limit(PlaceKind::Provider, json!({ "turnLimitMs": null })), None);
    }

    #[test]
    fn turns_the_hours_a_person_names_into_the_limit_none_of_them_off_within_a_day() {
        assert_eq!(turn_limit_ms_of(0.0), None);
        assert_eq!(turn_limit_ms_of(8.0), Some((8 * HOUR) as f64));
        assert_eq!(TURN_LIMIT_MAX_MS, 24 * HOUR);
        let past_a_day: PlaceSettings = wire(json!({ "turnLimitMs": 25 * HOUR }));
        assert!(past_a_day.check().is_err());
        let under_an_hour: PlaceSettings = wire(json!({ "turnLimitMs": 30 * 60_000 }));
        assert!(under_an_hour.check().is_err());
    }

    #[test]
    fn is_taken_by_every_computer_the_one_the_app_runs_on_included_and_every_cloud() {
        assert_eq!(refusal(json!({ "id": HERE_PLACE_ID, "kind": "computer", "name": "mac", "takesForks": false }), json!({ "turnLimitMs": null }), &[]), None);
    }

    #[test]
    fn reads_in_the_settings_line_at_what_it_runs_at_with_the_default_beside_one_the_pe() {
        assert_eq!(
            line(over(spoo(), json!({ "capDefault": { "threads": 2 }, "turnLimitMs": null, "turnLimitDefault": null }))),
            "spoo: 2 threads at once (the default), no turn limit (the default)"
        );
        assert_eq!(
            line(over(spoo(), json!({ "capDefault": { "threads": 2 }, "turnLimitMs": 8 * HOUR, "turnLimitDefault": null, "settings": { "turnLimitMs": 8 * HOUR } }))),
            "spoo: 2 threads at once (the default), stops a turn at 8h (off by default)"
        );
        assert_eq!(
            line(over(solari(), json!({ "capDefault": cloud_cap_default(), "turnLimitMs": null, "turnLimitDefault": 6 * HOUR, "settings": { "turnLimitMs": null } }))),
            "Solari: 3 machines at once (the default), $10 a day (the default), no turn limit (6h by default)"
        );
        assert_eq!(
            line(over(solari(), json!({ "capDefault": cloud_cap_default(), "turnLimitMs": 6 * HOUR, "turnLimitDefault": 6 * HOUR }))),
            "Solari: 3 machines at once (the default), $10 a day (the default), stops a turn at 6h (the default)"
        );
    }
}

mod what_runs_on_a_place {
    use super::*;

    fn places() -> Vec<PlaceView> {
        vec![
            place(json!({ "id": HERE_PLACE_ID, "kind": "computer" })),
            place(json!({ "id": "p_1", "kind": "computer" })),
            place(json!({ "id": "solari", "kind": "provider" })),
        ]
    }

    fn workspaces() -> Vec<WorkspaceView> {
        [
            json!({ "id": "w_mac", "kind": "local", "machineId": "local", "phase": "running" }),
            json!({ "id": "w_box", "kind": "cloud", "machineId": "m_1", "place": "p_1", "phase": "running" }),
            json!({ "id": "w_s1", "kind": "cloud", "machineId": "s_1", "provider": "solari", "phase": "running" }),
            json!({ "id": "w_s2", "kind": "cloud", "machineId": "s_2", "provider": "solari", "phase": "napping" }),
            json!({ "id": "w_s3", "kind": "cloud", "machineId": "s_3", "provider": "solari", "phase": "gone" }),
            json!({ "id": "w_s4", "kind": "cloud", "machineId": "s_4", "provider": "solari", "phase": "waking" }),
        ]
        .into_iter()
        .map(|w| wire(over(workspace_view(), w)))
        .collect()
    }

    fn thread_rows() -> Vec<Value> {
        vec![
            json!({ "workspaceId": "w_mac", "status": "running" }),
            json!({ "workspaceId": "w_mac", "status": "running" }),
            json!({ "workspaceId": "w_mac", "status": "completed" }),
            json!({ "workspaceId": "w_box", "status": "running" }),
            json!({ "workspaceId": "w_box", "status": "failed" }),
            json!({ "workspaceId": "w_s1", "status": "running" }),
        ]
    }

    fn threads(rows: Vec<Value>) -> Vec<ThreadView> {
        rows.into_iter().map(|t| wire(over(thread_view(), t))).collect()
    }

    #[test]
    fn counts_running_threads_on_a_computer_and_machines_holding_a_slot_on_a_cloud() {
        let (places, workspaces) = (places(), workspaces());
        let flat = threads(thread_rows());
        assert_eq!(running_on(HERE_PLACE_ID, &places, &workspaces, &flat), 2);
        // A thread another thread started there runs in that one's slot, and the row still counts it: every agent there.
        let mut rows = thread_rows();
        rows.push(json!({ "workspaceId": "w_mac", "status": "running", "parentThreadId": "t_1" }));
        rows.push(json!({ "workspaceId": "w_box", "status": "running", "parentThreadId": "t_1" }));
        let tree = threads(rows);
        assert_eq!(running_on(HERE_PLACE_ID, &places, &workspaces, &tree), 3);
        assert_eq!(running_on("p_1", &places, &workspaces, &tree), 2);
        assert_eq!(running_on("p_1", &places, &workspaces, &flat), 1);
        // A napping machine and a gone one hold no slot; a waking one does.
        assert_eq!(running_on("solari", &places, &workspaces, &flat), 2);
        assert_eq!(running_on("nowhere", &places, &workspaces, &flat), 0);
    }

    #[test]
    fn reads_the_room_left_under_the_cap_never_below_none() {
        let room = |running: f64, at_once: f64, room: f64, noun: &str| Some(PlaceRoom { running, at_once, room, noun: noun.to_string() });
        assert_eq!(place_room(&place(over(spoo(), json!({ "running": 1 })))), room(1.0, 2.0, 1.0, "thread"));
        assert_eq!(place_room(&place(over(spoo(), json!({ "running": 3 })))), room(3.0, 2.0, 0.0, "thread"));
        assert_eq!(place_room(&place(over(solari(), json!({ "running": 2 })))), room(2.0, 3.0, 1.0, "machine"));
        let mut no_cap = spoo();
        no_cap.as_object_mut().unwrap().remove("cap");
        assert_eq!(place_room(&place(no_cap)), None);
    }
}

mod a_held_spawns_words_and_the_slots_a_cloud_counts {
    use super::*;

    fn wait(v: Value) -> ThreadCapWait {
        wire(v)
    }

    fn threads() -> ThreadCapWait {
        wait(json!({ "placeId": "here", "place": "this computer", "running": 4, "atOnce": 4 }))
    }

    fn slot() -> ThreadCapWait {
        wait(json!({ "placeId": "box", "place": "Boat", "running": 3, "atOnce": 3, "why": "slot" }))
    }

    fn minute() -> ThreadCapWait {
        wait(json!({ "placeId": "box", "place": "Boat", "running": 12, "atOnce": 12, "why": "minute" }))
    }

    fn workspace(v: Value) -> WorkspaceView {
        wire(over(workspace_view(), v))
    }

    #[test]
    fn says_a_machine_slot_and_a_minute_of_starts_apart_as_05_words_them_and_threads_at() {
        assert_eq!(cap_wait_line(&slot()), "waiting for a machine on Boat: 3 of 3 running");
        assert_eq!(cap_wait_line(&minute()), "waiting for Boat's next minute of starts: 12 of 12 used");
        assert_eq!(cap_wait_line(&threads()), "waiting while this computer is running 4 of 4 threads");
        assert_eq!(cap_running_line(&slot()), "Boat is running 3 of 3 machines");
        assert_eq!(cap_running_line(&minute()), "Boat's starts this minute are used: 12 of 12");
    }

    #[test]
    fn offers_raise_machines_at_once_beside_a_slot_raise_threads_at_once_beside_threads() {
        assert_eq!(cap_raise_act(&slot()).as_deref(), Some("Raise machines at once"));
        assert_eq!(cap_raise_act(&threads()).as_deref(), Some(CAP_RAISE_ACT));
        assert_eq!(cap_raise_act(&minute()), None);
    }

    #[test]
    fn a_queued_spawn_waiting_holds_no_slot_until_its_machine_is_being_made_and_a_relea() {
        assert!(!phase_holds_slot(&workspace(json!({ "phase": "queued", "queue": { "since": "2026-10-12T00:00:00.000Z", "why": "slot" } }))));
        assert!(phase_holds_slot(&workspace(json!({ "phase": "queued" }))));
        assert!(!phase_holds_slot(&workspace(json!({ "phase": "released" }))));
        assert!(!phase_holds_slot(&workspace(json!({ "phase": "napping" }))));
        assert!(phase_holds_slot(&workspace(json!({ "phase": "running" }))));
    }
}

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::place_state::*;

    #[test]
    fn minutes_and_hours_turn_into_windows_zero_is_off() {
        assert_eq!(nap_ms_of(0.0).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "null"); // String(napMsOf(0))
        assert_eq!(turn_limit_ms_of(0.0).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "null"); // String(turnLimitMsOf(0))
        assert_eq!(nap_ms_of(1.0).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "60000"); // String(napMsOf(1))
        assert_eq!(turn_limit_ms_of(1.0).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "3600000"); // String(turnLimitMsOf(1))
        assert_eq!(nap_ms_of(30.0).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "1800000"); // String(napMsOf(30))
        assert_eq!(turn_limit_ms_of(30.0).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "108000000"); // String(turnLimitMsOf(30))
        assert_eq!(nap_ms_of(1.5).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "90000"); // String(napMsOf(1.5))
        assert_eq!(turn_limit_ms_of(1.5).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "5400000"); // String(turnLimitMsOf(1.5))
        assert_eq!(nap_ms_of(-1.0).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "-60000"); // String(napMsOf(-1))
        assert_eq!(turn_limit_ms_of(-1.0).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "-3600000"); // String(turnLimitMsOf(-1))
    }

    #[test]
    fn a_setting_falls_through_only_when_absent() {
        assert_eq!(setting_for(Some(&Some(5.0)), Some(&Some(7.0)), &Some(9.0)).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "5"); // String(settingFor(5, 7, 9))
        assert_eq!(setting_for(None, Some(&Some(7.0)), &Some(9.0)).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "7"); // String(settingFor(undefined, 7, 9))
        assert_eq!(setting_for(None, None, &Some(9.0)).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "9"); // String(settingFor(undefined, undefined, 9))
        assert_eq!(setting_for(Some(&None), Some(&Some(7.0)), &Some(9.0)).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "null"); // String(settingFor(null, 7, 9))
        assert_eq!(setting_for(None, Some(&None), &Some(9.0)).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "null"); // String(settingFor(undefined, null, 9))
    }

    #[test]
    fn held_turn_lines() {
        assert_eq!(deleted_before_start_line("web"), "web was deleted before this turn started"); // deletedBeforeStartLine("web")
        assert_eq!(cap_restarted_line("spoo"), "the host restarted while this turn waited for a slot on spoo; nothing started, so send it again"); // capRestartedLine("spoo")
    }
}
