// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/absent-computer.test.ts:190-200 (what the two screens say wsp puts on a computer),
//! cost-history.test.ts:144-151 (spendMeterWord), format.test.ts:1566-1678 (the import and export dialogs' words),
//! :1680-1721 (terminalConfigLines), :1968-2008 (shellVersionNotice) and :2010-2066 (the hosts listing),
//! hosts-menu.test.ts:18-47 (hostsMenuItems), :56-106 (the Computers table's cells), :108-113 (hereWord), :115-121
//! (the version notice across hosts) and workspace-theme.test.ts:210-239 (the custom properties). Skipped:
//! absent-computer.test.ts:183 (needs placeAddSheetWord, not in the stubs) and workspace-theme.test.ts:114 (needs
//! opacityCap, contrastRatio, SIDE_INK and WORD_FLOOR of workspace-look, not in the stubs).

mod support;

use regex::Regex;
use serde_json::{json, Value};
use wsp_words::workspace_look::theme_ink;
use wsp_wire::types::workspace_look::WorkspaceTheme;
use wsp_wire::views::desktop_bridge::HostsView;
use wsp_wire::views::place::PlaceView;
use wsp_wire::views::project_bundle::{ProjectExportEvent, ProjectImportEvent, ProjectSecret, TerminalConfig};
use wsp_words::words::places::*;

fn wire<T: serde::de::DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).unwrap()
}

fn strs(cells: &[&str]) -> Vec<String> {
    cells.iter().map(|c| c.to_string()).collect()
}

mod what_the_two_screens_say_wsp_puts_on_a_computer {
    use super::*;

    #[test]

    #[ignore = "checks constants alone, which the interface copied by script; no stub to fail on"]
    fn writes_what_remove_takes_off_from_the_same_list_so_neither_screen_can_hold_a_wor() {
        // Every one of the three, in the sentence, off the list rather than spelled again beside it.
        for said in [place_install::taken::SERVICE, place_install::taken::FILES, place_install::taken::OPENER] {
            assert!(places_words::remove::LEAVE_TAKES.contains(said), "{said}");
        }
    }

    // The TS also asserts placeOwnedPaths("/home/maya") holds no path with "var/lib"; placeOwnedPaths is not in the stubs.
    #[test]
    fn tells_a_person_the_copy_of_their_image_comes_off_with_wsp_since_it_sits_in_the_r() {
        // The runtime folder is no path of the home's own list; the leave as root takes it on its own road.
        let leave_takes = places_words::remove::LEAVE_TAKES;
        let copy = image_copy_line(None);
        assert!(leave_takes.contains(&copy), "{leave_takes}");
        assert!(!leave_takes.contains("Docker"), "{leave_takes}");
    }
}

mod spend_meter_word {
    use super::*;

    #[test]
    fn reads_what_a_cloud_spent_today_over_its_spend_per_day_the_limit_in_whole_dollars() {
        assert_eq!(spend_meter_word(2.314, 10.0), "$2.31/$10");
        assert_eq!(spend_meter_word(0.0, 10.0), "$0.00/$10");
        assert_eq!(spend_meter_word(12.5, 12.5), "$12.50/$12.50");
        assert_eq!(spend_meter_word(0.0, 0.0), "$0.00/$0");
    }
}

mod the_import_dialog_s_words {
    use super::*;

    fn ev(over: Value) -> ProjectImportEvent {
        let mut event = crate::support::trip_event();
        event.as_object_mut().unwrap().extend(over.as_object().unwrap().clone());
        wire(event)
    }

    const DEST: &str = "/Users/me/code/proj";

    /// The events one import with travelling sessions makes, in the runtime's order: the project upload lands, then the sessions tar uploads and lands.
    fn trip() -> Vec<ProjectImportEvent> {
        vec![
            ev(json!({ "stage": "planned", "message": "1204 files, 38 MB and the repository; 3 secret-shaped files; 4 caches left behind." })),
            ev(json!({ "stage": "consented", "message": "Rewriting .git/config to https://github.com/o/r; cut .env. Sessions travel for Claude Code (46 sessions)." })),
            ev(json!({ "stage": "packing", "message": "Packing 1857 files." })),
            ev(json!({ "stage": "uploading", "message": "Uploading 32 MB.", "bytes": 0, "total": 33_449_574 })),
            ev(json!({ "stage": "uploading", "message": "Part 1 of 2, 16 MB of 32 MB.", "bytes": 16_724_787, "total": 33_449_574 })),
            ev(json!({ "stage": "uploading", "message": "Part 2 of 2, 32 MB of 32 MB.", "bytes": 33_449_574, "total": 33_449_574 })),
            ev(json!({ "stage": "landing", "message": format!("Landing at {DEST}.") })),
            ev(json!({ "stage": "uploading", "message": "Uploading 3 session files and the rows to merge, 1 MB.", "bytes": 0, "total": 1_258_291 })),
            ev(json!({ "stage": "uploading", "message": "Part 1 of 1, 1 MB of 1 MB.", "bytes": 1_258_291, "total": 1_258_291 })),
            ev(json!({ "stage": "landing", "message": "Merging rows into Codex." })),
            ev(json!({ "stage": "landing", "message": "Landing sessions: Claude Code moved, Codex transcripts landed." })),
            ev(json!({ "stage": "done", "message": format!("1855 files, 38 MB, landed at {DEST}; sessions: Claude Code moved.") })),
        ]
    }

    #[test]
    fn names_the_workspace_the_folder_goes_into_and_that_it_lands_at_the_same_path() {
        assert_eq!(import_into_line("dev2"), "Into dev2, at the same path.");
    }

    #[test]
    fn says_a_repository_s_history_travels_in_plain_words_and_when_there_is_none() {
        assert_eq!(repo_line(true), "Git repository, history travels");
        assert_eq!(repo_line(false), "No repository");
    }

    #[test]
    fn tells_a_stranger_what_the_ticks_on_the_secret_shaped_rows_do_and_how_many_rows_t() {
        assert_eq!(secrets_note(2.0), "2 files look like secrets. Ticked files are copied as they are. Unticked files are left out and listed.");
        assert_eq!(secrets_note(1.0), "1 file looks like a secret. Ticked files are copied as they are. Unticked files are left out and listed.");
        assert_eq!(SESSIONS_NOTE, "Ticked agents' sessions go with the folder. The rest stay here.");
    }

    #[test]
    fn names_why_a_file_looks_like_a_secret_and_its_size_the_one_spelling_the_cli_s_pla() {
        let secret: ProjectSecret = wire(json!({ "path": ".env", "bytes": 120, "signals": ["name", "keys"] }));
        assert_eq!(secret_signals_line(&secret), "name, keys, 120 B");
    }

    #[test]
    fn reads_the_trip_s_current_step_in_plain_words_and_holds_the_bar_starting_the_runt() {
        assert_eq!(import_progress(&[], "dev2"), None);
        let trip = trip();
        let seen: Vec<Option<TripProgress>> = (0..trip.len()).map(|i| import_progress(&trip[..=i], "dev2")).collect();
        assert_eq!(
            seen.iter().map(|p| p.as_ref().map(|p| p.line.as_str())).collect::<Vec<_>>(),
            vec![
                Some("Starting"),
                Some("Starting"),
                Some("Packing 1857 files"),
                Some("Uploading 32 MB"),
                Some("Uploading 32 MB"),
                Some("Uploading 32 MB"),
                Some("Landing on dev2"),
                Some("Uploading sessions, 1 MB"),
                Some("Uploading sessions, 1 MB"),
                Some("Landing on dev2"),
                Some("Landing on dev2"),
                Some("Done"),
            ]
        );
        assert_eq!(
            seen.iter().map(|p| p.as_ref().map(|p| p.fraction)).collect::<Vec<_>>(),
            [0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0].map(Some).to_vec()
        );
        for i in 1..seen.len() {
            assert!(seen[i].as_ref().unwrap().fraction >= seen[i - 1].as_ref().unwrap().fraction, "step {i}");
        }
    }

    #[test]
    fn an_upload_without_a_total_still_reads_and_a_failure_has_no_step_since_the_status() {
        assert_eq!(
            import_progress(&[ev(json!({ "stage": "uploading", "message": "Uploading." }))], "dev2"),
            Some(TripProgress { line: "Uploading".to_string(), fraction: 0.0 })
        );
        assert_eq!(
            import_progress(&[ev(json!({ "stage": "packing", "message": "Packing 2 files." })), ev(json!({ "stage": "failed", "message": "the machine went away" }))], "dev2"),
            None
        );
    }
}

mod the_export_dialog_s_words {
    use super::*;

    fn ev(over: Value) -> ProjectExportEvent {
        let mut event = crate::support::trip_event();
        event.as_object_mut().unwrap().extend(over.as_object().unwrap().clone());
        wire(event)
    }

    /// The events one export with agent state makes, in the runtime's order: the folder packs and downloads, then the agents' state does, then one landing.
    fn trip() -> Vec<ProjectExportEvent> {
        vec![
            ev(json!({ "stage": "packing", "message": "Packing /root/spoo on the machine." })),
            ev(json!({ "stage": "downloading", "message": "The folder: 0 B of 31 MB.", "bytes": 0, "total": 32_505_856 })),
            ev(json!({ "stage": "downloading", "message": "The folder: 16 MB of 31 MB.", "bytes": 16_252_928, "total": 32_505_856 })),
            ev(json!({ "stage": "downloading", "message": "The folder: 31 MB of 31 MB.", "bytes": 32_505_856, "total": 32_505_856 })),
            ev(json!({ "stage": "packing", "message": "Packing the agents' state for it on the machine." })),
            ev(json!({ "stage": "downloading", "message": "Agent state: 0 B of 1 MB.", "bytes": 0, "total": 1_292_000 })),
            ev(json!({ "stage": "downloading", "message": "Agent state: 1 MB of 1 MB.", "bytes": 1_292_000, "total": 1_292_000 })),
            ev(json!({ "stage": "landing", "message": "Landing at /Users/me/code/spoo." })),
            ev(json!({ "stage": "done", "message": "1202 files, 38 MB, landed at /Users/me/code/spoo; 4 caches left behind; sessions: Claude Code (6 sessions) moved." })),
        ]
    }

    #[test]
    fn names_the_workspace_the_folder_comes_from_and_the_computer_it_lands_on_by_its_ow() {
        assert_eq!(export_from_line("dev2", "zingzy's MacBook Pro"), "From dev2, to zingzy's MacBook Pro.");
    }

    #[test]
    fn reads_the_trip_s_current_step_in_plain_words_and_holds_the_bar_the_folder_packs_() {
        assert_eq!(export_progress(&[], "zingzy's MacBook Pro"), None);
        let trip = trip();
        let seen: Vec<Option<TripProgress>> = (0..trip.len()).map(|i| export_progress(&trip[..=i], "zingzy's MacBook Pro")).collect();
        assert_eq!(
            seen.iter().map(|p| p.as_ref().map(|p| p.line.as_str())).collect::<Vec<_>>(),
            vec![
                Some("Packing the folder"),
                Some("Downloading the folder, 31 MB"),
                Some("Downloading the folder, 31 MB"),
                Some("Downloading the folder, 31 MB"),
                Some("Packing sessions"),
                Some("Downloading sessions, 1 MB"),
                Some("Downloading sessions, 1 MB"),
                Some("Landing on zingzy's MacBook Pro"),
                Some("Done"),
            ]
        );
        assert_eq!(
            seen.iter().map(|p| p.as_ref().map(|p| p.fraction)).collect::<Vec<_>>(),
            [0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0].map(Some).to_vec()
        );
        for i in 1..seen.len() {
            assert!(seen[i].as_ref().unwrap().fraction >= seen[i - 1].as_ref().unwrap().fraction, "step {i}");
        }
    }

    #[test]
    fn a_download_without_a_total_still_reads_and_a_failure_has_no_step_since_the_statu() {
        assert_eq!(
            export_progress(&[ev(json!({ "stage": "packing" })), ev(json!({ "stage": "downloading", "message": "The folder." }))], "spoo"),
            Some(TripProgress { line: "Downloading the folder".to_string(), fraction: 0.0 })
        );
        assert_eq!(export_progress(&[ev(json!({ "stage": "packing" })), ev(json!({ "stage": "failed", "message": "the machine went away" }))], "spoo"), None);
    }
}

mod terminal_config_lines {
    use super::*;

    fn none() -> Value {
        json!({ "files": [], "fontFamily": [], "palette": vec![Value::Null; 16] })
    }

    #[test]
    fn with_no_file_says_so_in_one_line() {
        let config: TerminalConfig = wire(none());
        assert_eq!(terminal_config_lines(&config), vec!["No Ghostty config on this computer; the terminal pane keeps its defaults."]);
    }

    #[test]
    fn names_the_files_read_then_one_line_per_key_the_pane_honours_in_ghostty_s_own_wor() {
        let mut palette = vec![json!({ "r": 69, "g": 71, "b": 90 })];
        palette.extend(vec![Value::Null; 15]);
        let mut config = none();
        config.as_object_mut().unwrap().extend(
            json!({
                "files": ["/Users/dev/.config/ghostty/config", "/Applications/Ghostty.app/Contents/Resources/ghostty/themes/Catppuccin Mocha"],
                "fontFamily": ["Berkeley Mono", "Symbols Nerd Font Mono"],
                "fontSize": 13,
                "theme": "Catppuccin Mocha",
                "background": { "r": 30, "g": 30, "b": 46 },
                "foreground": { "r": 205, "g": 214, "b": 244 },
                "palette": palette,
                "selectionBackground": { "r": 88, "g": 91, "b": 112 },
                "cursorColor": { "r": 245, "g": 224, "b": 220 },
                "cursorStyle": "underline",
                "cursorStyleBlink": false,
                "windowPaddingX": { "left": 2, "right": 4 },
                "windowPaddingY": { "top": 6, "bottom": 6 },
                "backgroundOpacity": 0.85,
                "backgroundBlur": 20,
            })
            .as_object()
            .unwrap()
            .clone(),
        );
        let config: TerminalConfig = wire(config);
        assert_eq!(
            terminal_config_lines(&config),
            vec![
                "Read /Users/dev/.config/ghostty/config, /Applications/Ghostty.app/Contents/Resources/ghostty/themes/Catppuccin Mocha",
                "font-family = Berkeley Mono, Symbols Nerd Font Mono",
                "font-size = 13",
                "theme = Catppuccin Mocha",
                "background = #1e1e2e",
                "foreground = #cdd6f4",
                "palette = 1 of 16 colors",
                "selection-background = #585b70",
                "cursor-color = #f5e0dc",
                "cursor-style = underline",
                "cursor-style-blink = false",
                "window-padding-x = 2,4",
                "window-padding-y = 6",
                "background-opacity = 0.85",
                "background-blur = 20",
            ]
        );
    }
}

mod a_desktop_shell_and_the_host_that_served_it_its_page {
    use super::*;

    fn notice(line: &str, update: bool) -> Option<ShellVersionNotice> {
        Some(ShellVersionNotice { line: line.to_string(), update })
    }

    #[test]
    fn says_nothing_while_the_two_are_one_release() {
        assert_eq!(shell_version_notice(Some("0.1.5"), "0.1.5", None), None);
    }

    #[test]
    fn names_both_numbers_and_points_at_the_new_app_when_the_shell_is_the_older_half() {
        assert_eq!(shell_version_notice(Some("0.1.3"), "0.1.5", None), notice("this app is 0.1.3, the host is 0.1.5: get the new app", true));
    }

    #[test]
    fn names_both_numbers_and_asks_for_the_app_s_own_host_when_the_host_is_the_older_ha() {
        assert_eq!(shell_version_notice(Some("0.1.5"), "0.1.3", None), notice("this app is 0.1.5, the host is 0.1.3: run the app's own host", false));
    }

    #[test]
    fn reads_a_shell_whose_bridge_carries_no_version_as_older_than_the_host_since_every() {
        assert_eq!(shell_version_notice(None, "0.1.5", None), notice("this app is older than the host, which is 0.1.5: get the new app", true));
    }

    #[test]
    fn reads_the_numbers_as_numbers_so_0_1_10_is_after_0_1_9_rather_than_before_it() {
        assert_eq!(shell_version_notice(Some("0.1.10"), "0.1.9", None).unwrap().update, false);
        assert_eq!(shell_version_notice(Some("0.1.9"), "0.1.10", None).unwrap().update, true);
    }

    #[test]
    fn puts_a_prerelease_before_the_release_it_leads_to_and_a_shorter_number_before_a_l() {
        assert_eq!(shell_version_notice(Some("1.0.0-rc.1"), "1.0.0", None).unwrap().update, true);
        assert_eq!(shell_version_notice(Some("1.0.0"), "1.0.0-rc.1", None).unwrap().update, false);
        assert_eq!(shell_version_notice(Some("0.2"), "0.2.1", None).unwrap().update, true);
        assert_eq!(shell_version_notice(Some("0.2"), "0.2.0", None), None);
    }

    #[test]
    fn tells_two_prereleases_of_one_release_apart_whichever_way_their_tails_are_spelled() {
        assert_eq!(
            shell_version_notice(Some("0.1.6-alpha"), "0.1.6-beta", None),
            notice("this app is 0.1.6-alpha, the host is 0.1.6-beta: get the new app", true)
        );
        assert_eq!(shell_version_notice(Some("0.1.6-beta"), "0.1.6-alpha", None).unwrap().update, false);
        assert_eq!(shell_version_notice(Some("0.1.6-rc1"), "0.1.6-rc2", None).unwrap().update, true);
        assert_eq!(shell_version_notice(Some("0.1.6-rc2"), "0.1.6-rc1", None).unwrap().update, false);
        assert_eq!(shell_version_notice(Some("0.1.6-rc1"), "0.1.6", None).unwrap().update, true);
        // A numbered tail counts as a number, so a tenth candidate is above a second and not below it.
        assert_eq!(shell_version_notice(Some("0.1.6-rc.2"), "0.1.6-rc.10", None).unwrap().update, true);
        // Build metadata carries no precedence, so two builds of one release say nothing.
        assert_eq!(shell_version_notice(Some("0.1.6+a1b2c3d"), "0.1.6+9f8e7d6", None), None);
    }
}

mod the_one_listing_of_the_hosts_a_computer_can_reach {
    use super::*;

    #[test]
    fn prints_each_host_on_the_account_with_its_beat_and_marks_the_one_every_line_takes() {
        let cells = hosts_table(&[
            HostsTableRow {
                host: "macbook".to_string(),
                address: Some("https://h1.usewsp.com".to_string()),
                away_ms: Some(Some(12_000.0)),
                connector: Some("2026.8.1".to_string()),
                host_key: Some("SHA256:aaa".to_string()),
                device_id: Some("d_1".to_string()),
                default: Some(true),
            },
            HostsTableRow { host: "attic".to_string(), away_ms: Some(None), ..Default::default() },
            HostsTableRow {
                host: "box".to_string(),
                address: Some("https://h2.usewsp.com".to_string()),
                device_id: Some("d_2".to_string()),
                host_key: Some("SHA256:bbb".to_string()),
                ..Default::default()
            },
        ]);
        assert_eq!(cells[0], strs(&["HOST", "ADDRESS", "STATE", "DEVICE", "CONNECTOR", "KEY", ""]));
        assert_eq!(cells[1], strs(&["macbook", "https://h1.usewsp.com", "up 12s", "d_1", "2026.8.1", "SHA256:aaa", "default"]));
        // A host on the account that has not said where it is: there is nothing to dial and the row says so.
        assert_eq!(cells[2], strs(&["attic", NOT_UP_YET, "not yet", "", "", "", ""]));
        // A row read off this computer's records while the relay did not answer says nothing of a beat, so its state is
        // left empty rather than read as away.
        assert_eq!(cells[3], strs(&["box", "https://h2.usewsp.com", "", "d_2", "", "SHA256:bbb", ""]));
    }

    #[test]
    fn says_a_host_that_is_beating_with_no_address_yet_waits_on_its_next_start_so_one_r() {
        // A host on the account beating from a wsp from before it sent the key every dial holds it to: the address is
        // withheld until a beat carries the key, and the state column already calls it up.
        let cells = hosts_table(&[HostsTableRow { host: "attic".to_string(), away_ms: Some(Some(30_000.0)), connector: Some("2026.7.1".to_string()), ..Default::default() }]);
        assert_eq!(cells[1], strs(&["attic", ADDRESS_NEXT_START, "up 30s", "", "2026.7.1", "", ""]));
        // A host that has never beaten is not up at all, and its row says that instead.
        assert_eq!(hosts_table(&[HostsTableRow { host: "attic".to_string(), away_ms: Some(None), ..Default::default() }])[1][1], NOT_UP_YET);
    }

    #[test]
    fn calls_a_host_up_inside_two_beats_and_away_after_them_since_one_missed_beat_is_a_() {
        assert_eq!(host_beat_word(Some(0.0)), "up 1ms");
        assert_eq!(host_beat_word(Some(HOST_BEAT_MS)), "up 1m");
        assert_eq!(host_beat_word(Some(2.0 * HOST_BEAT_MS)), "up 2m");
        assert_eq!(host_beat_word(Some(2.0 * HOST_BEAT_MS + 1.0)), "away 2 min");
        assert_eq!(host_beat_word(Some(6.0 * 60.0 * 60_000.0)), "away 6 h");
        assert_eq!(host_beat_word(None), "not yet");
    }

    #[test]
    fn says_which_key_a_host_is_pinned_under_when_the_account_lists_another_and_names_t() {
        let said = host_key_moved_line("macbook", "SHA256:held", "SHA256:listed");
        assert!(said.contains("SHA256:held"), "{said}");
        assert!(said.contains("SHA256:listed"), "{said}");
        assert!(said.contains("wsp host pair"), "{said}");
        assert!(said.contains("wsp logout, wsp login and wsp hosts"), "{said}");
        let dropped = host_dropped_line("attic");
        assert!(dropped.contains("attic"), "{dropped}");
        let quiet = relay_quiet_line("the relay at https://r did not answer");
        assert!(quiet.contains("the rows below"), "{quiet}");
        assert!(NO_HOSTS_LINE.contains("wsp login"), "{NO_HOSTS_LINE}");
        assert!(!NO_HOSTS_LINE.contains("--code"), "{NO_HOSTS_LINE}");
    }
}

fn hosts_view() -> HostsView {
    wire(json!({
        "here": "This Mac",
        "current": "box",
        "hosts": [
            { "alias": "attic", "url": "https://hattic.boxes.example" },
            { "alias": "box", "url": "https://hbox1.boxes.example" },
        ],
    }))
}

mod hosts_menu_items {
    use super::*;

    #[test]
    fn puts_this_computer_first_and_marks_the_current_host_alone_in_one_group() {
        let rows = hosts_menu_items(&hosts_view());
        assert_eq!(
            Value::Array(rows.iter().map(|r| json!([r.label, r.checked, r.enabled])).collect()),
            json!([["This Mac", false, true], ["attic", false, true], ["box", true, true]])
        );
        let groups: std::collections::HashSet<&str> = rows.iter().map(|r| r.group.as_str()).collect();
        assert_eq!(groups.len(), 1);
        let home = hosts_menu_items(&HostsView { current: None, ..hosts_view() });
        assert_eq!(home[0].checked, Some(true));
        assert_eq!(home.iter().filter(|r| r.checked == Some(true)).count(), 1);
    }

    #[test]
    fn names_the_host_each_row_moves_to_back_from_its_id_and_nothing_for_an_id_it_never() {
        let rows = hosts_menu_items(&hosts_view());
        assert_eq!(
            rows.iter().map(|r| host_menu_action(&r.id)).collect::<Vec<_>>(),
            vec![Some(None), Some(Some("attic".to_string())), Some(Some("box".to_string()))]
        );
        assert_eq!(host_menu_action("open-terminal"), None);
        assert_eq!(host_menu_action("disconnect:box"), None);
        assert_eq!(host_menu_action("connect"), None);
    }
}

mod what_the_computers_table_says_about_a_computer {
    use super::*;

    fn view(over: Value) -> PlaceView {
        let mut row = json!({ "id": "p_1", "kind": "computer", "name": "old-macbook", "default": false, "present": true, "engine": "none", "takesForks": true });
        row.as_object_mut().unwrap().extend(over.as_object().unwrap().clone());
        wire(row)
    }

    fn provider() -> PlaceView {
        wire(json!({ "id": "box", "kind": "provider", "name": "box", "default": false }))
    }

    #[test]
    fn says_the_count_it_is_given_and_nothing_else_about_a_computer_of_the_person_s_own() {
        // Every computer on this list forks: a box whose kernel cannot is turned down at the join, so there is no
        // second kind of computer row and no clause behind the count for one.
        assert_eq!(place_workspaces_cell(&view(json!({})), 1.0, None), "1");
        assert_eq!(place_workspaces_cell(&view(json!({ "present": false })), 1.0, None), "1");
        assert_eq!(place_workspaces_cell(&view(json!({})), 0.0, None), "0");
    }

    #[test]
    fn puts_the_month_behind_a_row_beside_its_count_where_the_host_has_metered_anything() {
        let provider = provider();
        assert_eq!(place_workspaces_cell(&provider, 2.0, Some(0.41)), "2, $0.41 this month");
        // Money is spelled the one way the protocol spells it, which is cents whatever the size.
        assert_eq!(place_workspaces_cell(&provider, 1.0, Some(0.0042)), "1, $0.00 this month");
        // A computer of the person's own is charged by nobody, so a month never lands on its row.
        assert_eq!(place_workspaces_cell(&view(json!({})), 1.0, Some(0.41)), "1");
        assert_eq!(place_workspaces_cell(&provider, 2.0, None), "2");
    }

    #[test]
    fn hands_the_cell_to_a_table_in_two_parts_the_count_and_the_note_behind_it_and_read() {
        let provider = provider();
        let parts = |row: &PlaceView, count: f64, usd: Option<f64>| serde_json::to_value(place_workspaces_parts(row, count, usd)).unwrap();
        assert_eq!(parts(&provider, 2.0, Some(0.41)), json!({ "count": "2", "note": "$0.41 this month" }));
        assert_eq!(parts(&provider, 2.0, None), json!({ "count": "2" }));
        assert_eq!(parts(&view(json!({})), 1.0, None), json!({ "count": "1" }));
        // The one line is the two parts joined, so a table and a row of words cannot say different things.
        for (row, count, usd) in [(provider.clone(), 2.0, Some(0.41)), (view(json!({})), 1.0, None)] {
            let parts = place_workspaces_parts(&row, count, usd);
            let joined = match &parts.note {
                None => parts.count.clone(),
                Some(note) => format!("{}, {}", parts.count, note),
            };
            assert_eq!(place_workspaces_cell(&row, count, usd), joined);
        }
    }

    #[test]
    fn says_what_one_place_took_this_month_and_what_it_burns_now_and_what_every_provide() {
        assert_eq!(place_spend_line(4.12, 0.16, 2.0), "$4.12 this month, $0.16/hr now across 2 workspaces");
        assert_eq!(place_spend_line(0.0, 0.0, 1.0), "$0.00 this month, $0.00/hr now across 1 workspace");
        assert_eq!(places_spend_foot(4.53, 2.0), "$4.53 this month across 2 providers");
        assert_eq!(places_spend_foot(0.41, 1.0), "$0.41 this month across 1 provider");
    }

    #[test]
    fn reads_the_count_off_the_list_it_is_handed_never_off_the_workspace_a_join_recorded() {
        // This computer's own workspace and a provider's forks are on no row, so a cell read off the row said 0 for
        // both: the count is the caller's and the row only says what kind of place it is about.
        let here: PlaceView = wire(json!({ "id": "here", "kind": "computer", "name": "here", "default": true, "takesForks": false }));
        assert_eq!(place_workspaces_cell(&here, 1.0, None), "1");
        let box_row: PlaceView = wire(json!({ "id": "box", "kind": "provider", "name": "box", "default": false }));
        assert_eq!(place_workspaces_cell(&box_row, 2.0, None), "2");
    }
}

mod here_word {
    use super::*;

    #[test]
    fn says_this_mac_on_a_mac_and_this_computer_anywhere_else() {
        assert_eq!(here_word(true), "This Mac");
        assert_eq!(here_word(false), "This computer");
    }
}

mod the_version_notice_across_hosts {
    use super::*;

    #[test]
    fn names_the_host_by_its_label_when_the_window_is_on_one_somewhere_else_and_as_toda() {
        assert_eq!(
            shell_version_notice(Some("0.2.0"), "0.3.0", Some("maya@box")).unwrap().line,
            "this app is 0.2.0, the host maya@box is 0.3.0: get the new app"
        );
        assert_eq!(
            shell_version_notice(Some("0.3.0"), "0.2.0", Some("maya@box")).unwrap().line,
            "this app is 0.3.0, the host maya@box is 0.2.0: run the app's own host"
        );
        assert_eq!(shell_version_notice(Some("0.2.0"), "0.3.0", None).unwrap().line, "this app is 0.2.0, the host is 0.3.0: get the new app");
        assert_eq!(shell_version_notice(Some("0.2.0"), "0.2.0", Some("maya@box")), None);
    }
}

mod the_custom_properties {
    use super::*;

    /// harmonyDots({ angle: 200, radius: 0.5 }, "complementary") over DEFAULT_THEME.
    fn theme() -> Value {
        json!({ "dots": [{ "angle": 200, "radius": 0.5 }, { "angle": 20, "radius": 0.5 }], "harmony": "complementary", "grain": 0, "opacity": 0.5, "mode": "auto" })
    }

    fn with(over: Value) -> WorkspaceTheme {
        let mut theme = theme();
        theme.as_object_mut().unwrap().extend(over.as_object().unwrap().clone());
        wire(theme)
    }

    #[test]
    fn one_dot_lays_its_colour_flat_two_fade_from_opposite_corners_three_settle_in_two_() {
        let one = fmt_theme_vars(&with(json!({ "dots": [{ "angle": 0, "radius": 0.5 }], "harmony": "single", "opacity": 0.5 })), false);
        assert_eq!(one.space_gradient, "linear-gradient(rgb(249 6 6 / 50%), rgb(249 6 6 / 50%))");
        let two = fmt_theme_vars(&with(json!({ "opacity": 0.7 })), false).space_gradient;
        assert_eq!(Regex::new(r"gradient\(").unwrap().find_iter(&two).count(), 2);
        assert_eq!(Regex::new(r" / 70%\)").unwrap().find_iter(&two).count(), 2);
        assert!(two.contains("transparent"), "{two}");
        // resizeDots(theme, 1): splitComplementary keeps the first dot and adds one at +150 and one at +210 degrees.
        let three = fmt_theme_vars(
            &with(json!({ "harmony": "splitComplementary", "dots": [{ "angle": 200, "radius": 0.5 }, { "angle": 350, "radius": 0.5 }, { "angle": 50, "radius": 0.5 }] })),
            false,
        )
        .space_gradient;
        assert_eq!(Regex::new(r"radial-gradient\(").unwrap().find_iter(&three).count(), 2);
        assert_eq!(Regex::new(r"linear-gradient\(").unwrap().find_iter(&three).count(), 1);
    }

    #[test]
    fn the_percent_and_the_rgb_strings_have_one_home_and_the_stops_and_the_ink_are_writ() {
        assert_eq!(fmt_percent(0.5), "50%");
        assert_eq!(fmt_percent(0.333), "33%");
        assert_eq!(fmt_rgb(&[1.0, 2.0, 3.0], None), "rgb(1 2 3)");
        assert_eq!(fmt_rgb(&[1.0, 2.0, 3.0], Some(0.25)), "rgb(1 2 3 / 25%)");
        let one_dot = json!({ "dots": [{ "angle": 0, "radius": 0.5 }] });
        let vars = fmt_theme_vars(&with(json!({ "dots": [{ "angle": 0, "radius": 0.5 }], "harmony": "single" })), false);
        assert_eq!(vars.space_tint, fmt_rgb(&theme_ink(&with(one_dot), false), None));
    }

    #[test]
    fn carry_the_grain_as_the_slider_s_number_and_the_ink_for_the_side_the_theme_draws_() {
        let vars = fmt_theme_vars(&with(json!({ "grain": 0.25 })), true);
        assert_eq!(vars.space_grain, "0.25");
        let ink = theme_ink(&with(json!({})), true);
        assert_eq!(vars.space_tint, format!("rgb({} {} {})", ink[0], ink[1], ink[2]));
        assert_eq!(serde_json::to_value(&vars).unwrap().as_object().unwrap().keys().collect::<Vec<_>>(), ["--space-gradient", "--space-grain", "--space-tint"]);
    }
}

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::words::places::*;

    fn js<T: serde::Serialize>(v: &T) -> String {
        wsp_words::exit::json_line(&serde_json::to_value(v).unwrap(), None)
    }

    use serde_json::json;
    use wsp_wire::views::project_bundle::TerminalRgb;

    fn wire<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> T {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn terminal_config_and_look_words() {
        assert_eq!(hex_color(&TerminalRgb { r: wsp_wire::types::js_number::JsInt::from(0_i64), g: wsp_wire::types::js_number::JsInt::from(15_i64), b: wsp_wire::types::js_number::JsInt::from(255_i64) }), "#000fff"); // hexColor({r:0,g:15,b:255})
        assert_eq!(look_word(""), ""); // lookWord("")
        assert_eq!(look_word("rocket"), "Rocket"); // lookWord("rocket")
        assert_eq!(look_word("\u{e9}t\u{e9}"), "\u{c9}t\u{e9}"); // lookWord("été")
        assert_eq!(fmt_percent(0_f64), "0%"); // fmtPercent(0)
        assert_eq!(fmt_percent(0.5_f64), "50%"); // fmtPercent(0.5)
        assert_eq!(fmt_percent(0.005_f64), "1%"); // fmtPercent(0.005)
        assert_eq!(fmt_percent(0.015_f64), "2%"); // fmtPercent(0.015)
        assert_eq!(fmt_percent(1.234_f64), "123%"); // fmtPercent(1.234)
        assert_eq!(fmt_percent(-0.005_f64), "0%"); // fmtPercent(-0.005)
        assert_eq!(fmt_rgb(&[1.0, 2.0, 3.0], None), "rgb(1 2 3)"); // fmtRgb([1, 2, 3])
        assert_eq!(fmt_rgb(&[1.5, 2.0, 3.0], Some(0.255)), "rgb(1.5 2 3 / 26%)"); // fmtRgb([1.5, 2, 3], 0.255)
    }

    #[test]
    fn the_computers_table_cells() {
        assert_eq!(offline_for(-1_f64), "0 min"); // offlineFor(-1)
        assert_eq!(offline_for(0_f64), "0 min"); // offlineFor(0)
        assert_eq!(offline_for(59999_f64), "0 min"); // offlineFor(59999)
        assert_eq!(offline_for(60000_f64), "1 min"); // offlineFor(60000)
        assert_eq!(offline_for(3599999_f64), "59 min"); // offlineFor(3599999)
        assert_eq!(offline_for(3600000_f64), "1 h"); // offlineFor(3600000)
        assert_eq!(offline_for(86399999_f64), "23 h"); // offlineFor(86399999)
        assert_eq!(offline_for(86400000_f64), "1 d"); // offlineFor(86400000)
        assert_eq!(offline_for(172800001_f64), "2 d"); // offlineFor(172800001)
        assert_eq!(place_workspaces_cell(&wire(json!({"kind":"box","name":"b"})), 2.0, Some(5.0)), "2"); // placeWorkspacesCell({"kind":"box","name":"b"}, 2, 5)
        assert_eq!(place_workspaces_cell(&wire(json!({"kind":"provider","name":"p","build":"copying your image"})), 0.0, Some(5.0)), "0, copying your image"); // placeWorkspacesCell({"kind":"provider","name":"p","build":"copying your image"}, 0, 5)
        assert_eq!(place_workspaces_cell(&wire(json!({"kind":"provider","name":"p"})), 3.0, None), "3"); // placeWorkspacesCell({"kind":"provider","name":"p"}, 3, undefined)
        assert_eq!(place_workspaces_cell(&wire(json!({"kind":"provider","name":"p"})), 3.0, Some(0.0)), "3, $0.00 this month"); // placeWorkspacesCell({"kind":"provider","name":"p"}, 3, 0)
        assert_eq!(place_workspaces_cell(&wire(json!({"kind":"here","name":"h"})), 1.0, Some(9.0)), "1"); // placeWorkspacesCell({"kind":"here","name":"h"}, 1, 9)
        assert_eq!(spend_meter_word(0_f64, 5_f64), "$0.00/$5"); // spendMeterWord(0, 5)
        assert_eq!(spend_meter_word(1.234_f64, 5.5_f64), "$1.23/$5.50"); // spendMeterWord(1.234, 5.5)
        assert_eq!(spend_meter_word(10_f64, 0_f64), "$10.00/$0"); // spendMeterWord(10, 0)
        assert_eq!(spend_meter_word(3_f64, 2.999_f64), "$3.00/$3.00"); // spendMeterWord(3, 2.999)
        assert_eq!(place_spend_line(12.5, 0.0, 1.0), "$12.50 this month, $0.00/hr now across 1 workspace"); // placeSpendLine({monthUsd:12.5,rateUsdPerHour:0}, 1)
        assert_eq!(places_spend_foot(0.0, 0.0), "$0.00 this month across 0 providers"); // placesSpendFoot(0, 0)
        assert_eq!(place_engine_line(&wire(json!({"name":"b"}))), None); // placeEngineLine({"name":"b"})
        assert_eq!(place_engine_line(&wire(json!({"name":"b","engine":"none"}))), Some("b has no container engine; a project's docker compose runs there once you install Docker or podman on it".to_string())); // placeEngineLine({"name":"b","engine":"none"})
        assert_eq!(place_engine_line(&wire(json!({"name":"b","engine":"podman"}))), Some("b runs a project's own containers on podman; a workspace made with --engine gets a socket to it and sees its own containers alone".to_string())); // placeEngineLine({"name":"b","engine":"podman"})
        assert_eq!(agents_cell(&wire(json!({"name":"x"}))), ""); // agentsCell({name:"x"})
        assert_eq!(agents_cell(&wire(json!({"name":"x","agents":["claude","codex","gemini"],"agentVersions":{"claude":"2.1.270 (Claude Code)"},"signIns":{"codex":"vault-key","claude":"none"}}))), "claude 2.1.270 not signed in, codex signed in with a token, gemini"); // agentsCell({name:"x",agents:["claude","codex","gemini"],agentVersions:{claude:"2.1.270 (Claude Code)"},signIns:{codex:"vault-key",claude:"none"}})
        assert_eq!(image_copy_line(None), "the copy of your image"); // imageCopyLine()
        assert_eq!(image_copy_line(Some("2 GB")), "the copy of your image (2 GB)"); // imageCopyLine("2 GB")
    }

    #[test]
    fn versions_are_strict_semver_under_64_units() {
        assert_eq!(agent_version_word("2.1.270 (Claude Code)"), "2.1.270"); // agentVersionWord("2.1.270 (Claude Code)")
        assert_eq!(agent_version_word("codex-cli 0.153.0"), "0.153.0"); // agentVersionWord("codex-cli 0.153.0")
        assert_eq!(agent_version_word("  weird  "), "weird"); // agentVersionWord("  weird  ")
        assert_eq!(agent_version_word("1.2"), "1.2"); // agentVersionWord("1.2")
        assert_eq!(agent_version_word("v10.20.30-rc1"), "10.20.30"); // agentVersionWord("v10.20.30-rc1")
        assert_eq!(strict_version(" v1.2.3 "), Some("1.2.3".to_string())); // strictVersion(" v1.2.3 ")
        assert_eq!(strict_version("1.2.3-rc.1+build.5"), Some("1.2.3-rc.1+build.5".to_string())); // strictVersion("1.2.3-rc.1+build.5")
        assert_eq!(strict_version("01.2.3"), None); // strictVersion("01.2.3")
        assert_eq!(strict_version("1.2"), None); // strictVersion("1.2")
        assert_eq!(strict_version("vv1.2.3"), None); // strictVersion("vv1.2.3")
        assert_eq!(strict_version("1.2.3-"), None); // strictVersion("1.2.3-")
        assert_eq!(strict_version("1.2.3-01"), None); // strictVersion("1.2.3-01")
        assert_eq!(strict_version("1.2.3-0a"), Some("1.2.3-0a".to_string())); // strictVersion("1.2.3-0a")
        assert_eq!(strict_version("1.0.0-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"), Some("1.0.0-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string())); // strictVersion("1.0.0-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        assert_eq!(strict_version("1.0.0-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"), None); // strictVersion("1.0.0-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        assert_eq!(strict_version("V1.2.3"), None); // strictVersion("V1.2.3")
        assert_eq!(strict_version("1.2.3\n"), Some("1.2.3".to_string())); // strictVersion("1.2.3\n")
        assert_eq!(codex_key_refused_line("OPENAI_API_KEY", "   ", "codex login"), "OpenAI refused your OPENAI_API_KEY. Put a working key in ~/.wsp/.env, or sign Codex in where this thread runs with codex login."); // codexKeyRefusedLine("OPENAI_API_KEY", "   ", "codex login")
        assert_eq!(codex_key_refused_line("OPENAI_API_KEY", &format!(" {} tail", "r".repeat(79)), "codex login"), "OpenAI refused your OPENAI_API_KEY: rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr. Put a working key in ~/.wsp/.env, or sign Codex in where this thread runs with codex login."); // codexKeyRefusedLine("OPENAI_API_KEY", " " + "r".repeat(79) + " tail", "codex login")
    }

    #[test]
    fn a_providers_remove_and_forget_say_what_went_and_what_stays() {
        assert_eq!(provider_remove_question("Boat", 0.0), "Remove Boat?\nEvery machine wsp made there is deleted, and once Boat reads them gone the key is forgotten."); // providerRemoveQuestion("Boat", 0)
        assert_eq!(provider_remove_question("Boat", 1.0), "Remove Boat?\nIts task is deleted at Boat, and once Boat reads it gone the key is forgotten."); // providerRemoveQuestion("Boat", 1)
        assert_eq!(provider_remove_question("Boat", 3.0), "Remove Boat?\nIts 3 tasks are deleted at Boat, and once Boat reads them gone the key is forgotten."); // providerRemoveQuestion("Boat", 3)
        assert_eq!(provider_key_still_line(&[KeyStill::File("/h/.env".into())]), "Its key is still in /h/.env, which a host started in that folder reads first; take it out there."); // providerKeyStillLine([{"file":"/h/.env"}])
        assert_eq!(provider_key_still_line(&[KeyStill::Env("BOAT_API_KEY".into())]), "Its key is still in the environment this host started with (BOAT_API_KEY), which a host started from that shell reads first; take it out there."); // providerKeyStillLine([{"env":"BOAT_API_KEY"}])
        assert_eq!(provider_key_still_line(&[KeyStill::File("/a".into()), KeyStill::Env("K".into())]), "Its key is still in /a and in the environment this host started with (K), which a host started in that folder or from that shell reads first; take it out of both."); // providerKeyStillLine([{"file":"/a"},{"env":"K"}])
        assert_eq!(provider_key_still_line(&[KeyStill::File("/a".into()), KeyStill::File("/b".into()), KeyStill::Env("K".into())]), "Its key is still in /a and in /b and in the environment this host started with (K), which a host started in that folder or in that folder or from that shell reads first; take it out of each."); // providerKeyStillLine([{"file":"/a"},{"file":"/b"},{"env":"K"}])
        assert_eq!(provider_removed_line("Boat", 1.0, None, None), "Boat removed: 1 machine deleted, the key forgotten."); // providerRemovedLine("Boat", 1)
        assert_eq!(provider_removed_line("Boat", 0.0, Some("Its key is still in x."), Some(2.0)), "Boat removed: 0 machines and 2 image snapshots deleted. Its key is still in x."); // providerRemovedLine("Boat", 0, "Its key is still in x.", 2)
        assert_eq!(provider_forget_question("Boat", 0.0), "Forget Boat?\nThe key leaves this host. Nothing at Boat is touched, so machines wsp made there may bill until Boat's own timer ends them."); // providerForgetQuestion("Boat", 0)
        assert_eq!(provider_forget_question("Boat", 1.0), "Forget Boat?\nThe key and its task's record leave this host. Nothing at Boat is touched, so machines wsp made there may bill until Boat's own timer ends them."); // providerForgetQuestion("Boat", 1)
        assert_eq!(provider_forget_question("Boat", 2.0), "Forget Boat?\nThe key and its 2 tasks' records leave this host. Nothing at Boat is touched, so machines wsp made there may bill until Boat's own timer ends them."); // providerForgetQuestion("Boat", 2)
        assert_eq!(provider_forgot_line("Boat", 0.0, None), "Boat forgotten: the key left this host. Nothing at Boat was touched, so machines wsp made there may bill until Boat's own timer ends them."); // providerForgotLine("Boat", 0, undefined)
        assert_eq!(provider_forgot_line("Boat", 1.0, None), "Boat forgotten: the key and 1 task's record left this host. Nothing at Boat was touched, so machines wsp made there may bill until Boat's own timer ends them."); // providerForgotLine("Boat", 1, undefined)
        assert_eq!(provider_forgot_line("Boat", 3.0, Some("STILL.")), "Boat forgotten: 3 tasks' records left this host. STILL. Nothing at Boat was touched, so machines wsp made there may bill until Boat's own timer ends them."); // providerForgotLine("Boat", 3, "STILL.")
        assert_eq!(provider_forgot_line("Boat", 0.0, Some("STILL.")), "Boat forgotten. STILL. Nothing at Boat was touched, so machines wsp made there may bill until Boat's own timer ends them."); // providerForgotLine("Boat", 0, "STILL.")
        assert_eq!(provider_kept_line("Boat", 0.0, &[LeftThere { id: "m1".into(), said: "x".into() }]), "1 machine wsp made at Boat is not gone: m1 (x). The key is kept until it goes. Remove Boat again to ask once more."); // providerKeptLine("Boat", 0, [{id:"m1",said:"x"}])
        assert_eq!(provider_kept_line("Boat", 2.0, &[LeftThere { id: "m0".into(), said: "s".into() }, LeftThere { id: "m1".into(), said: "s".into() }, LeftThere { id: "m2".into(), said: "s".into() }, LeftThere { id: "m3".into(), said: "s".into() }, LeftThere { id: "m4".into(), said: "s".into() }, LeftThere { id: "m5".into(), said: "s".into() }, LeftThere { id: "m6".into(), said: "s".into() }]), "7 machines wsp made at Boat are not gone: m0 (s), m1 (s), m2 (s), m3 (s), m4 (s) and 2 more. The key is kept until they go; 2 other machines were deleted. Remove Boat again to ask once more."); // providerKeptLine("Boat", 2, [{"id":"m0","said":"s"},{"id":"m1","said":"s"},{"id":"m2","said":"s"},{"id":"m3","said":"s"},{"id":"m4","said":"s"},{"id":"m5","said":"s"},{"id":"m6","said":"s"}])
        assert_eq!(provider_kept_line("Boat", 1.0, &[LeftThere { id: "a".into(), said: "x".into() }, LeftThere { id: "b".into(), said: "y".into() }]), "2 machines wsp made at Boat are not gone: a (x), b (y). The key is kept until they go; 1 other machine was deleted. Remove Boat again to ask once more."); // providerKeptLine("Boat", 1, [{id:"a",said:"x"},{id:"b",said:"y"}])
        assert_eq!(provider_images_line("Boat", &[SavedImage { id: "i1".into(), size_bytes: None }]), "Its image snapshot i1 is deleted at Boat before the key goes."); // providerImagesLine("Boat", [{"id":"i1"}])
        assert_eq!(provider_images_left_line("Boat", &[SavedImage { id: "i1".into(), size_bytes: None }]), "Its image snapshot i1 stays at Boat and bills every month; add the key again and remove Boat to delete it."); // providerImagesLeftLine("Boat", [{"id":"i1"}])
        assert_eq!(provider_images_line("Boat", &[SavedImage { id: "i1".into(), size_bytes: Some(2048.0) }, SavedImage { id: "i2".into(), size_bytes: None }]), "Its 2 image snapshots, i1 (2 KB) and i2, are deleted at Boat before the key goes."); // providerImagesLine("Boat", [{"id":"i1","sizeBytes":2048},{"id":"i2"}])
        assert_eq!(provider_images_left_line("Boat", &[SavedImage { id: "i1".into(), size_bytes: Some(2048.0) }, SavedImage { id: "i2".into(), size_bytes: None }]), "Its 2 image snapshots, i1 (2 KB) and i2, stay at Boat and bill every month; add the key again and remove Boat to delete them."); // providerImagesLeftLine("Boat", [{"id":"i1","sizeBytes":2048},{"id":"i2"}])
        assert_eq!(provider_images_line("Boat", &[SavedImage { id: "a".into(), size_bytes: None }, SavedImage { id: "b".into(), size_bytes: None }, SavedImage { id: "c".into(), size_bytes: Some(1.0) }]), "Its 3 image snapshots, a, b and c (1 B), are deleted at Boat before the key goes."); // providerImagesLine("Boat", [{"id":"a"},{"id":"b"},{"id":"c","sizeBytes":1}])
        assert_eq!(provider_images_left_line("Boat", &[SavedImage { id: "a".into(), size_bytes: None }, SavedImage { id: "b".into(), size_bytes: None }, SavedImage { id: "c".into(), size_bytes: Some(1.0) }]), "Its 3 image snapshots, a, b and c (1 B), stay at Boat and bill every month; add the key again and remove Boat to delete them."); // providerImagesLeftLine("Boat", [{"id":"a"},{"id":"b"},{"id":"c","sizeBytes":1}])
        assert_eq!(provider_images_kept_line("Boat", 0.0, &[LeftThere { id: "i".into(), said: "no".into() }]), "1 image snapshot this host saved at Boat is not gone: i (no). The key is kept until it goes. Remove Boat again to ask once more."); // providerImagesKeptLine("Boat", 0, [{id:"i",said:"no"}])
        assert_eq!(provider_images_kept_line("Boat", 2.0, &[LeftThere { id: "i".into(), said: "no".into() }, LeftThere { id: "j".into(), said: "x".into() }]), "2 image snapshots this host saved at Boat are not gone: i (no), j (x). The key is kept until they go; 2 machines were deleted. Remove Boat again to ask once more."); // providerImagesKeptLine("Boat", 2, [{id:"i",said:"no"},{id:"j",said:"x"}])
        assert_eq!(provider_unlisted_line("Boat", 0.0, "503"), "Boat's machines could not be listed again (503), so one made meanwhile may still be there and the key is kept. Remove Boat again to finish."); // providerUnlistedLine("Boat", 0, "503")
        assert_eq!(provider_unlisted_line("Boat", 1.0, "503"), "Boat's machines could not be listed again after 1 machine was deleted (503), so one made meanwhile may still be there and the key is kept. Remove Boat again to finish."); // providerUnlistedLine("Boat", 1, "503")
        assert_eq!(provider_unlisted_line("Boat", 4.0, "503"), "Boat's machines could not be listed again after 4 machines were deleted (503), so one made meanwhile may still be there and the key is kept. Remove Boat again to finish."); // providerUnlistedLine("Boat", 4, "503")
    }

    #[test]
    fn hosts_menu_and_table() {
        assert_eq!(host_menu_action("switch:"), Some(None)); // JSON.stringify(hostMenuAction("switch:") ?? "absent")
        assert_eq!(host_menu_action("switch:h1"), Some(Some("h1".to_string()))); // JSON.stringify(hostMenuAction("switch:h1") ?? "absent")
        assert_eq!(host_menu_action("switch:switch:"), Some(Some("switch:".to_string()))); // JSON.stringify(hostMenuAction("switch:switch:") ?? "absent")
        assert_eq!(host_menu_action("other"), None); // JSON.stringify(hostMenuAction("other") ?? "absent")
        assert_eq!(host_menu_action(""), None); // JSON.stringify(hostMenuAction("") ?? "absent")
        assert_eq!(host_beat_word(None), "not yet"); // hostBeatWord(null)
        assert_eq!(host_beat_word(Some(-5.0)), "up 1ms"); // hostBeatWord(-5)
        assert_eq!(host_beat_word(Some(0.0)), "up 1ms"); // hostBeatWord(0)
        assert_eq!(host_beat_word(Some(120000.0)), "up 2m"); // hostBeatWord(120000)
        assert_eq!(host_beat_word(Some(120001.0)), "away 2 min"); // hostBeatWord(120001)
        assert_eq!(host_beat_word(Some(7200000.0)), "away 2 h"); // hostBeatWord(7200000)
        assert_eq!(exit_line(None), None); // exitLine(null)
        assert_eq!(exit_line(Some(0.0)), Some("exit 0".to_string())); // exitLine(0)
        assert_eq!(exit_line(Some(-1.0)), Some("exit -1".to_string())); // exitLine(-1)
    }

    #[test]
    fn a_clouds_plan_line() {
        assert_eq!(cloud_plan_line(&wire(json!({"trial":true,"machinesAtOnce":1,"sizes":[]})), None, &["small", "default"]), "Trial: 1 machine at once, small and default sizes"); // cloudPlanLine({trial:true,machinesAtOnce:1,sizes:[]}, undefined, ["small","default"])
        assert_eq!(cloud_plan_line(&wire(json!({"trial":true,"machinesAtOnce":2,"sizes":[]})), Some(&wire(json!({"minute":{"used":0,"limit":12},"hour":{"used":1,"limit":60},"day":{"used":3,"limit":200}}))), &[] as &[&str]), "Trial: 2 machines at once, 200 starts a day,  sizes"); // cloudPlanLine({trial:true,machinesAtOnce:2,sizes:[]}, {minute:{used:0,limit:12},hour:{used:1,limit:60},day:{used:3,limit:200}}, [])
        assert_eq!(cloud_plan_line(&wire(json!({"key":"box_20","trial":false,"machinesAtOnce":3,"sizes":[]})), Some(&wire(json!({"minute":{"used":0,"limit":12},"hour":{"used":1,"limit":60},"day":{"used":3,"limit":200}}))), &[] as &[&str]), "$20 plan: 3 machines at once, 12 starts a minute, 60 an hour, 200 a day"); // cloudPlanLine({key:"box_20",trial:false,machinesAtOnce:3,sizes:[]}, {minute:{used:0,limit:12},hour:{used:1,limit:60},day:{used:3,limit:200}}, [])
        assert_eq!(cloud_plan_line(&wire(json!({"key":"pro","trial":false,"machinesAtOnce":1,"sizes":[]})), None, &[] as &[&str]), "Plan: 1 machine at once"); // cloudPlanLine({key:"pro",trial:false,machinesAtOnce:1,sizes:[]}, undefined, [])
        assert_eq!(cloud_starts_today_line(&wire(json!({"minute":{"used":0,"limit":12},"hour":{"used":1,"limit":60},"day":{"used":3,"limit":200}}))), "Starts today: 3 of 200"); // cloudStartsTodayLine({minute:{used:0,limit:12},hour:{used:1,limit:60},day:{used:3,limit:200}})
    }

    #[test]
    fn the_other_lines_read_as_the_ts_writes_them() {
        assert_eq!(js(&import_progress(&[wire(crate::support::over(crate::support::trip_event(), json!({"stage":"planned","message":""}))), wire(crate::support::over(crate::support::trip_event(), json!({"stage":"packing","message":"Packing 3 files."}))), wire(crate::support::over(crate::support::trip_event(), json!({"stage":"uploading","message":"","bytes":50,"total":100}))), wire(crate::support::over(crate::support::trip_event(), json!({"stage":"uploading","message":"","bytes":20,"total":100}))), wire(crate::support::over(crate::support::trip_event(), json!({"stage":"landing","message":""}))), wire(crate::support::over(crate::support::trip_event(), json!({"stage":"uploading","message":"","bytes":10,"total":2048})))], "ws")), "{\"line\":\"Uploading sessions, 2 KB\",\"fraction\":1}"); // JSON.stringify(importProgress([{"stage":"planned","message":""},{"stage":"packing","message":"Packing 3 files."},{"stage":"uploading","message":"","bytes":50,"total":100},{"stage":"uploading","message":"","bytes":20,"total":100},{"stage":"landing","message":""},{"stage":"uploading","message":"","bytes":10,"total":2048}], "ws"))
        assert_eq!(js(&import_progress(&[wire(crate::support::over(crate::support::trip_event(), json!({"stage":"planned","message":""}))), wire(crate::support::over(crate::support::trip_event(), json!({"stage":"packing","message":"Packing 3 files."}))), wire(crate::support::over(crate::support::trip_event(), json!({"stage":"uploading","message":"","bytes":50,"total":100})))], "ws")), "{\"line\":\"Uploading 100 B\",\"fraction\":0.5}"); // JSON.stringify(importProgress([{"stage":"planned","message":""},{"stage":"packing","message":"Packing 3 files."},{"stage":"uploading","message":"","bytes":50,"total":100}], "ws"))
        assert_eq!(js(&import_progress(&[], "ws")), "null"); // JSON.stringify(importProgress([], "ws"))
        assert_eq!(js(&import_progress(&[wire(crate::support::over(crate::support::trip_event(), json!({"stage":"packing","message":"a"}))), wire(crate::support::over(crate::support::trip_event(), json!({"stage":"failed","message":"x"})))], "ws")), "null"); // JSON.stringify(importProgress([{stage:"packing",message:"a"},{stage:"failed",message:"x"}], "ws"))
        assert_eq!(js(&export_progress(&[wire(crate::support::over(crate::support::trip_event(), json!({"stage":"packing","message":""}))), wire(crate::support::over(crate::support::trip_event(), json!({"stage":"downloading","message":"","bytes":30,"total":60}))), wire(crate::support::over(crate::support::trip_event(), json!({"stage":"packing","message":""}))), wire(crate::support::over(crate::support::trip_event(), json!({"stage":"downloading","message":"","bytes":1,"total":60}))), wire(crate::support::over(crate::support::trip_event(), json!({"stage":"downloading","message":"","total":0,"bytes":0})))], "spoo")), "{\"line\":\"Downloading sessions, 0 B\",\"fraction\":0.5}"); // JSON.stringify(exportProgress([{"stage":"packing","message":""},{"stage":"downloading","message":"","bytes":30,"total":60},{"stage":"packing","message":""},{"stage":"downloading","message":"","bytes":1,"total":60},{"stage":"downloading","message":"","total":0,"bytes":0}], "spoo"))
        assert_eq!(js(&export_progress(&[wire(crate::support::over(crate::support::trip_event(), json!({"stage":"landing","message":""})))], "")), "{\"line\":\"Landing\",\"fraction\":1}"); // JSON.stringify(exportProgress([{stage:"landing",message:""}], ""))
        assert_eq!(js(&terminal_config_lines(&wire(json!({"files":["/a","/b"],"fontFamily":["A","B"],"fontSize":13.5,"theme":"dark","background":{"r":0,"g":15,"b":255},"palette":[null,{"r":1,"g":2,"b":3},null,null,null,null,null,null,null,null,null,null,null,null,null,null],"cursorStyleBlink":false,"windowPaddingX":{"left":4,"right":4},"windowPaddingY":{"top":1,"bottom":2},"backgroundOpacity":0.9,"backgroundBlur":0})))), "[\"Read /a, /b\",\"font-family = A, B\",\"font-size = 13.5\",\"theme = dark\",\"background = #000fff\",\"palette = 1 of 16 colors\",\"cursor-style-blink = false\",\"window-padding-x = 4\",\"window-padding-y = 1,2\",\"background-opacity = 0.9\",\"background-blur = 0\"]"); // JSON.stringify(terminalConfigLines({"files":["/a","/b"],"fontFamily":["A","B"],"fontSize":13.5,"theme":"dark","background":{"r":0,"g":15,"b":255},"palette":[null,{"r":1,"g":2,"b":3},null,null,null,null,null,null,null,null,null,null,null,null,null,null],"cursorStyleBlink":false,"windowPaddingX":{"left":4,"right":4},"windowPaddingY":{"top":1,"bottom":2},"backgroundOpacity":0.9,"backgroundBlur":0}))
        assert_eq!(js(&terminal_config_lines(&wire(json!({"files":[],"fontFamily":[],"palette":[null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null]})))), "[\"No Ghostty config on this computer; the terminal pane keeps its defaults.\"]"); // JSON.stringify(terminalConfigLines({files:[],fontFamily:[],palette:Array(16).fill(null)}))
        assert_eq!(js(&shell_version_notice(None, "0.3.8", None)), "{\"line\":\"this app is older than the host, which is 0.3.8: get the new app\",\"update\":true}"); // JSON.stringify(shellVersionNotice(undefined, "0.3.8", undefined) ?? null)
        assert_eq!(js(&shell_version_notice(Some("0.3.8"), "0.3.8", None)), "null"); // JSON.stringify(shellVersionNotice("0.3.8", "0.3.8", undefined) ?? null)
        assert_eq!(js(&shell_version_notice(Some("0.3.7"), "0.3.8", Some("spoo"))), "{\"line\":\"this app is 0.3.7, the host spoo is 0.3.8: get the new app\",\"update\":true}"); // JSON.stringify(shellVersionNotice("0.3.7", "0.3.8", "spoo") ?? null)
        assert_eq!(js(&shell_version_notice(Some("0.4.0"), "0.3.8", None)), "{\"line\":\"this app is 0.4.0, the host is 0.3.8: run the app's own host\",\"update\":false}"); // JSON.stringify(shellVersionNotice("0.4.0", "0.3.8", undefined) ?? null)
        assert_eq!(js(&shell_version_notice(Some("0.3.8+b"), "0.3.8", None)), "null"); // JSON.stringify(shellVersionNotice("0.3.8+b", "0.3.8", undefined) ?? null)
        assert_eq!(js(&place_workspaces_parts(&wire(json!({"kind":"box","name":"b"})), 2.0, Some(5.0))), "{\"count\":\"2\"}"); // JSON.stringify(placeWorkspacesParts({"kind":"box","name":"b"}, 2, 5))
        assert_eq!(js(&place_workspaces_parts(&wire(json!({"kind":"provider","name":"p","build":"copying your image"})), 0.0, Some(5.0))), "{\"count\":\"0\",\"note\":\"copying your image\"}"); // JSON.stringify(placeWorkspacesParts({"kind":"provider","name":"p","build":"copying your image"}, 0, 5))
        assert_eq!(js(&place_workspaces_parts(&wire(json!({"kind":"provider","name":"p"})), 3.0, None)), "{\"count\":\"3\"}"); // JSON.stringify(placeWorkspacesParts({"kind":"provider","name":"p"}, 3, undefined))
        assert_eq!(js(&place_workspaces_parts(&wire(json!({"kind":"provider","name":"p"})), 3.0, Some(0.0))), "{\"count\":\"3\",\"note\":\"$0.00 this month\"}"); // JSON.stringify(placeWorkspacesParts({"kind":"provider","name":"p"}, 3, 0))
        assert_eq!(js(&place_workspaces_parts(&wire(json!({"kind":"here","name":"h"})), 1.0, Some(9.0))), "{\"count\":\"1\"}"); // JSON.stringify(placeWorkspacesParts({"kind":"here","name":"h"}, 1, 9))
        assert_eq!({ let r = provider_force_refusal("Boat", "wsp remove boat"); js(&[r.said, r.fix]) }, "[\"Boat is not a computer, so --force, which removes one over work no remote has, means nothing there.\",\"Run wsp remove boat without it, or with --forget to drop the key and touch nothing at Boat.\"]"); // JSON.stringify(Object.values(providerForceRefusal("Boat", "wsp remove boat")))
        assert_eq!({ let r = provider_key_refused("Boat", "401"); js(&[r.said, r.fix]) }, "[\"Boat refused the key (401), so wsp cannot see or delete the machines it made there; the key is kept.\",\"Save a working key and remove Boat again, or remove it with --forget.\"]"); // JSON.stringify(Object.values(providerKeyRefused("Boat", "401")))
        assert_eq!(js(&join_roads("tok", &["http://a:7000", "http://b"], Some("https://relay"))), "[{\"url\":\"http://a:7000\",\"line\":\"wsp join http://a:7000 --code tok\"},{\"url\":\"http://b\",\"line\":\"wsp join http://b --code tok\"},{\"url\":\"https://relay\",\"line\":\"wsp join https://relay --code tok\",\"note\":\"when the host is linked to your relay\"}]"); // JSON.stringify(joinRoads("tok", ["http://a:7000", "http://b"], "https://relay"))
        assert_eq!(js(&join_roads("tok", &[] as &[&str], None)), "[]"); // JSON.stringify(joinRoads("tok", [], undefined))
        assert_eq!(js(&provider_left_words::listed(1500.0)), "\"still listed 2 s after its delete\""); // JSON.stringify(PROVIDER_LEFT_WORDS.listed(1500))
        assert_eq!(js(&provider_left_words::listed(2500.0)), "\"still listed 3 s after its delete\""); // JSON.stringify(PROVIDER_LEFT_WORDS.listed(2500))
        assert_eq!(js(&hosts_menu_items(&wire(json!({"here":"This Mac","current":"h2","hosts":[{"alias":"h1","url":"u"},{"alias":"h2","url":"u"}]})))), "[{\"id\":\"switch:\",\"label\":\"This Mac\",\"group\":\"hosts\",\"enabled\":true,\"checked\":false},{\"id\":\"switch:h1\",\"label\":\"h1\",\"group\":\"hosts\",\"enabled\":true,\"checked\":false},{\"id\":\"switch:h2\",\"label\":\"h2\",\"group\":\"hosts\",\"enabled\":true,\"checked\":true}]"); // JSON.stringify(hostsMenuItems({here:"This Mac",current:"h2",hosts:[{alias:"h1",url:"u"},{alias:"h2",url:"u"}]}))
        assert_eq!(js(&hosts_table(&[HostsTableRow { host: "a".into(), ..Default::default() }, HostsTableRow { host: "b".into(), away_ms: Some(None), ..Default::default() }, HostsTableRow { host: "c".into(), away_ms: Some(Some(5000.0)), ..Default::default() }, HostsTableRow { host: "d".into(), address: Some("x".into()), away_ms: Some(Some(999999999.0)), device_id: Some("dv".into()), connector: Some("cn".into()), host_key: Some("k".into()), default: Some(true) }, HostsTableRow { host: "e".into(), default: Some(false), ..Default::default() }])), "[[\"HOST\",\"ADDRESS\",\"STATE\",\"DEVICE\",\"CONNECTOR\",\"KEY\",\"\"],[\"a\",\"not up yet\",\"\",\"\",\"\",\"\",\"\"],[\"b\",\"not up yet\",\"not yet\",\"\",\"\",\"\",\"\"],[\"c\",\"waits on its next start\",\"up 5.0s\",\"\",\"\",\"\",\"\"],[\"d\",\"x\",\"away 11 d\",\"dv\",\"cn\",\"k\",\"default\"],[\"e\",\"not up yet\",\"\",\"\",\"\",\"\",\"\"]]"); // JSON.stringify(hostsTable([{host:"a"},{host:"b",awayMs:null},{host:"c",awayMs:5000},{host:"d",address:"x",awayMs:999999999,deviceId:"dv",connector:"cn",hostKey:"k",default:true},{host:"e",default:false}]))
    }
}
