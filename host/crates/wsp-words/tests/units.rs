// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/format-cases.test.ts:19-26 (bytes, durations, cost rows), format.test.ts:221-271
//! (fmtBytes, fmtMemGb, placeFactsLine, boxRoomLines), :438-472 (durations, uptime, elapsed, cost), :1017-1106 (machine
//! size words, machineCapRefusal) and init-job.test.ts:451-492 (the tally and the image estimate). Skipped:
//! place-state.test.ts:73 (tests placeSettingsLine, a place-state sentence; fmtSize is incidental) and
//! workspace-state.test.ts:223 (tests servesReading in workspace-state; NOT_ON_THIS_KIND is one incidental assertion).

mod support;

use indexmap::IndexSet;

use serde_json::{json, Value};
use wsp_wire::types::init_job::{InitJob, InitScreen, InitScreenId};
use wsp_wire::wire::capabilities::{MachineSizeOffer, PlaceCapacity, WorkspaceSize};
use wsp_words::words::units::*;

fn wire<T: serde::de::DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).unwrap()
}

fn size(cpu: f64, mem_mb: f64) -> WorkspaceSize {
    wire(json!({ "cpu": cpu, "memMb": mem_mb }))
}

fn nb(line: &str) -> String {
    line.replace(' ', "\u{a0}")
}

mod the_shared_format_cases_in_daemon_fixtures_contract_formats_json {
    use super::*;

    #[derive(serde::Deserialize)]
    struct Cases {
        bytes: Vec<(f64, String)>,
        durations: Vec<(f64, String)>,
        cost: Vec<(f64, String)>,
    }

    fn cases() -> Cases {
        serde_json::from_str(include_str!("../../../../daemon/fixtures/contract/formats.json")).unwrap()
    }

    #[test]
    fn bytes_s_reads_j() {
        let cases = cases();
        assert!(!cases.bytes.is_empty());
        for (n, want) in &cases.bytes {
            assert_eq!(&fmt_bytes(*n), want, "bytes {n} reads {want:?}");
        }
    }

    #[test]
    fn a_duration_of_s_ms_reads_j() {
        let cases = cases();
        assert!(!cases.durations.is_empty());
        for (ms, want) in &cases.durations {
            assert_eq!(&fmt_duration(*ms, None), want, "a duration of {ms} ms reads {want:?}");
        }
    }

    #[test]
    fn a_cost_of_s_reads_j() {
        let cases = cases();
        assert!(!cases.cost.is_empty());
        for (usd, want) in &cases.cost {
            assert_eq!(&fmt_cost(*usd), want, "a cost of {usd} reads {want:?}");
        }
    }
}

mod fmt_bytes_and_fmt_mem_gb {
    use super::*;

    #[test]
    fn reads_whole_units_under_a_gigabyte_then_gb_with_one_decimal_unless_whole() {
        let got: Vec<String> = [
            0.0,
            12.0,
            1023.0,
            1024.0,
            2_048.0,
            1536.0,
            3.0 * 1024.0 * 1024.0,
            38.2 * 1024.0 * 1024.0,
            38.6 * 1024.0 * 1024.0,
            2.3 * 1024_f64.powi(3),
            20.0 * 1024_f64.powi(3),
            32_000_000_000.0,
        ]
        .iter()
        .map(|n| fmt_bytes(*n))
        .collect();
        assert_eq!(got, vec!["0 B", "12 B", "1023 B", "1 KB", "2 KB", "2 KB", "3 MB", "38 MB", "39 MB", "2.3 GB", "20 GB", "29.8 GB"]);
    }

    #[test]
    fn a_machine_size_s_memory_reads_as_gb_whole_when_it_is_whole_and_with_the_fraction() {
        let got: Vec<String> = [1536.0, 2048.0, 3000.0, 4096.0, 32768.0].iter().map(|n| fmt_mem_gb(*n)).collect();
        assert_eq!(got, vec!["1.5 GB", "2 GB", "2.9 GB", "4 GB", "32 GB"]);
    }

    #[test]
    fn has_a_gb_tier_with_a_decimal_and_whole_megabytes_under_it_a_tenth_of_a_megabyte() {
        assert_eq!(fmt_bytes(3000.0 * 1024.0 * 1024.0), "2.9 GB");
        assert_eq!(fmt_bytes(2048.0 * 1024.0 * 1024.0), "2 GB");
        assert_eq!(fmt_bytes(250.0 * 1024.0 * 1024.0), "250 MB");
        assert_eq!(fmt_bytes(250.4 * 1024.0 * 1024.0), "250 MB");
    }
}

mod a_computer_of_the_person_s_own_in_one_line {
    use super::*;

    #[test]
    fn names_its_cores_its_memory_and_the_room_left_where_its_threads_work_as_one_unbre() {
        let line = place_facts_line(&size(4.0, 8192.0), Some(97_710_505_984.0));
        assert_eq!(line, "4 cores, 8 GB, 91 GB free".replace(' ', "\u{a0}"));
        // A computer that would not say how much room it has says the rest.
        assert_eq!(place_facts_line(&size(8.0, 16384.0), None), "8 cores, 16 GB".replace(' ', "\u{a0}"));
        // Cores, never vCPU: a computer somebody owns has the cores it has.
        assert!(!line.contains("vCPU"), "{line}");
    }
}

mod the_room_left_on_a_box_as_the_doctor_reads_it_back {
    use super::*;

    fn capacity(v: Value) -> PlaceCapacity {
        wire(crate::support::over(crate::support::place_capacity(), v))
    }

    #[test]
    fn counts_the_forks_that_exist_and_leaves_the_rest_free() {
        // Two forks on a two core box, one core and one gigabyte each: the box is full of cores and has memory left.
        assert_eq!(
            box_room_lines(&capacity(json!({ "cores": 2, "memMb": 4096, "cpuTaken": 2, "memTakenMb": 2048 }))),
            vec!["2 cores, 2 in use by forks, 0 free", "4 GB, 2 GB in use by forks, 2 GB free"]
        );
        assert_eq!(
            box_room_lines(&capacity(json!({ "cores": 2, "memMb": 4096, "cpuTaken": 1, "memTakenMb": 1024 }))),
            vec!["2 cores, 1 in use by forks, 1 free", "4 GB, 1 GB in use by forks, 3 GB free"]
        );
        // A box with nothing on it, and the one core singular.
        assert_eq!(
            box_room_lines(&capacity(json!({ "cores": 1, "memMb": 2048, "cpuTaken": 0, "memTakenMb": 0 }))),
            vec!["1 core, 0 in use by forks, 1 free", "2 GB, 0 GB in use by forks, 2 GB free"]
        );
    }

    #[test]
    fn says_nothing_at_all_where_the_computer_counts_neither() {
        assert_eq!(box_room_lines(&capacity(json!({ "cores": 2, "memMb": 4096 }))), Vec::<String>::new());
        assert_eq!(box_room_lines(&capacity(json!({ "cores": 2, "memMb": 4096, "cpuTaken": 1 }))), Vec::<String>::new());
    }

    #[test]
    fn never_reads_back_less_than_nothing_free_when_the_forks_hold_more_than_the_box_ha() {
        assert_eq!(
            box_room_lines(&capacity(json!({ "cores": 2, "memMb": 4096, "cpuTaken": 3, "memTakenMb": 8192 }))),
            vec!["2 cores, 3 in use by forks, 0 free", "4 GB, 8 GB in use by forks, 0 GB free"]
        );
    }
}

mod fmt_duration_fmt_elapsed_and_fmt_cost {
    use super::*;

    #[test]
    fn fmt_duration_s_short_style_reads_ms_under_a_second_tenths_under_ten_whole_second() {
        let got: Vec<String> = [
            0.0, 7.0, 999.0, 1500.0, 9960.0, 10458.0, 59_400.0, 59_600.0, 60_000.0, 101_515.0, 492_000.0, 862_399.0, 3_665_000.0, -5.0, f64::NAN, 4000.0,
        ]
        .iter()
        .map(|ms| fmt_duration(*ms, None))
        .collect();
        assert_eq!(
            got,
            vec!["1ms", "7ms", "999ms", "1.5s", "10s", "10s", "59s", "60s", "1m", "1m 42s", "8m 12s", "14m 22s", "61m 5s", "0ms", "0ms", "4.0s"]
        );
    }

    #[test]
    fn fmt_duration_s_clock_style_reads_minutes_and_two_digit_seconds_hours_ahead_once() {
        let got: Vec<String> = [0.0, 999.0, 61_000.0, 900_000.0, 3_599_499.0, 3_600_000.0, 6.0 * 3_600_000.0 + 65_000.0, -5.0, f64::NAN]
            .iter()
            .map(|ms| fmt_duration(*ms, Some(DurationStyle::Clock)))
            .collect();
        assert_eq!(got, vec!["0m 00s", "0m 01s", "1m 01s", "15m 00s", "59m 59s", "1h 00m 00s", "6h 01m 05s", "0m 00s", "0m 00s"]);
    }

    #[test]
    fn fmt_uptime_reads_minutes_under_an_hour_hours_and_minutes_under_a_day_then_days_a() {
        let got: Vec<String> = [
            0.0,
            59_000.0,
            60_000.0,
            12.0 * 60_000.0,
            3_600_000.0,
            4.0 * 3_600_000.0 + 12.0 * 60_000.0,
            24.0 * 3_600_000.0,
            3.0 * 86_400_000.0 + 4.0 * 3_600_000.0 + 59.0 * 60_000.0,
            -5.0,
            f64::NAN,
        ]
        .iter()
        .map(|ms| fmt_uptime(*ms))
        .collect();
        assert_eq!(got, vec!["0m", "0m", "1m", "12m", "1h 0m", "4h 12m", "1d 0h", "3d 4h", "0m", "0m"]);
    }

    #[test]
    fn both_styles_round_the_same_instant_to_the_same_minute_and_second() {
        let short_re = regex::Regex::new(r"^(\d+)m(?: (\d+)s)?$").unwrap();
        let clock_re = regex::Regex::new(r"^(?:(\d+)h )?(\d+)m (\d+)s$").unwrap();
        let num = |m: &regex::Captures, i: usize| -> f64 { m.get(i).map_or(0.0, |g| g.as_str().parse().unwrap()) };
        for ms in [60_499.0, 60_500.0, 119_999.0, 3_599_999.0, 5_400_500.0] {
            let short = fmt_duration(ms, None);
            let clock = fmt_duration(ms, Some(DurationStyle::Clock));
            let s = short_re.captures(&short).unwrap_or_else(|| panic!("{short}"));
            let c = clock_re.captures(&clock).unwrap_or_else(|| panic!("{clock}"));
            assert_eq!([num(&s, 1), num(&s, 2)], [num(&c, 1) * 60.0 + num(&c, 2), num(&c, 3)]);
        }
    }

    #[test]
    fn fmt_elapsed_is_a_running_clock_whole_seconds_then_minutes_and_seconds_never_tent() {
        let got: Vec<String> = [0.0, 999.0, 1000.0, 3_400.0, 9_999.0, 10_458.0, 59_600.0, 60_000.0, 73_000.0, 314_200.0, -5.0, f64::NAN]
            .iter()
            .map(|ms| fmt_elapsed(*ms))
            .collect();
        assert_eq!(got, vec!["0s", "0s", "1s", "3s", "9s", "10s", "59s", "1m", "1m 13s", "5m 14s", "0s", "0s"]);
    }

    #[test]
    fn fmt_cost_reads_cents_at_every_size_so_two_figures_in_one_thread_are_never_in_two() {
        let got: Vec<String> = [1.94, 0.22, 0.01, 0.0042, 0.0].iter().map(|n| fmt_cost(*n)).collect();
        assert_eq!(got, vec!["$1.94", "$0.22", "$0.01", "$0.00", "$0.00"]);
    }
}

mod machine_size_words {
    use super::*;

    fn offers() -> Vec<MachineSizeOffer> {
        wire(json!([
            { "cpu": 2, "memMb": 4096, "rateUsdPerHour": 0.11 },
            { "cpu": 2, "memMb": 8192, "rateUsdPerHour": 0.15 },
        ]))
    }

    #[test]
    fn fmt_size_is_the_one_line_for_a_size_in_the_app_the_cpus_in_the_kind_s_word_a_dot() {
        let got: Vec<String> = [size(2.0, 4096.0), size(4.0, 1536.0)].iter().map(|s| fmt_size(s, None)).collect();
        assert_eq!(got, vec!["2\u{a0}vCPU,\u{a0}4\u{a0}GB", "4\u{a0}vCPU,\u{a0}1.5\u{a0}GB"]);
        // A provider's cpus are virtual and this computer's are not: the same line, the kind's own word for them.
        assert_eq!(fmt_size(&size(10.0, 16384.0), Some(CpuWord::Cores)), "10\u{a0}cores,\u{a0}16\u{a0}GB");
        assert_eq!(fmt_size(&size(2.0, 4096.0), Some(CpuWord::VCPU)), "2\u{a0}vCPU,\u{a0}4\u{a0}GB");
    }

    #[test]
    fn a_one_core_size_reads_1_core_never_1_cores_in_fmt_size_and_in_every_line_built_o() {
        assert_eq!(fmt_size(&size(1.0, 3891.0), Some(CpuWord::Cores)), nb("1 core, 3.8 GB"));
        assert_eq!(fmt_size(&size(1.0, 4096.0), None), nb("1 vCPU, 4 GB"));
        assert_eq!(place_facts_line(&size(1.0, 4096.0), None), nb("1 core, 4 GB"));
        assert_eq!(
            box_room_lines(&wire(crate::support::over(crate::support::place_capacity(), json!({ "cores": 1, "memMb": 4096, "cpuTaken": 0, "memTakenMb": 0 }))))[0],
            "1 core, 0 in use by forks, 1 free"
        );
        assert_eq!(fmt_cpus(1.0, Some(CpuWord::Cores)), "1 core");
        assert_eq!(fmt_cpus(2.0, Some(CpuWord::Cores)), "2 cores");
        assert_eq!(fmt_cpus(1.0, None), "1 vCPU");
    }

    #[test]
    fn size_word_spells_v_cpus_an_x_and_the_gb_the_size_table_names_and_size_from_word() {
        let got: Vec<String> = [size(2.0, 4096.0), size(4.0, 8192.0), size(1.0, 512.0)].iter().map(size_word).collect();
        assert_eq!(got, vec!["2x4", "4x8", "1x0.5"]);
        let read: Vec<Option<WorkspaceSize>> = ["2x4", " 4x8 ", "1x0.5"].iter().map(|w| size_from_word(w)).collect();
        assert_eq!(read, vec![Some(size(2.0, 4096.0)), Some(size(4.0, 8192.0)), Some(size(1.0, 512.0))]);
        for s in offers() {
            let shape: WorkspaceSize = serde_json::from_value(json!({ "cpu": s.cpu, "memMb": s.mem_mb })).unwrap();
            assert_eq!(size_from_word(&size_word(&shape)), Some(shape));
        }
    }

    #[test]
    fn size_from_word_names_nothing_for_a_word_that_is_not_a_size() {
        let got: Vec<Option<WorkspaceSize>> = ["big", "2", "x4", "2x", "0x4", "2x0", "2 x 4", "2x4x8", "-2x4"].iter().map(|w| size_from_word(w)).collect();
        assert_eq!(got, vec![None; 9]);
    }

    #[test]
    fn fmt_rate_reads_a_rate_a_provider_computed_not_only_one_written_down_the_price_to() {
        // Solari prices per vCPU-hour plus per GB-hour (solari-backend.ts), so every rate it offers carries float noise:
        // 2x2 lands on 0.09000000000000001. A rule that compares the cent form back to the number reads noise as a
        // third place and prints $0.090/hr on every live size.
        let solari = |cpu: f64, mem_gb: f64| -> f64 { cpu * 0.035 + mem_gb * 0.01 };
        assert_eq!(fmt_rate(solari(2.0, 2.0)), "$0.09/hr");
        assert_eq!(fmt_rate(solari(2.0, 4.0)), "$0.11/hr");
        assert_eq!(fmt_rate(solari(4.0, 8.0)), "$0.22/hr");
        assert_eq!(fmt_rate(solari(8.0, 32.0)), "$0.60/hr");
        // Box quotes its classes outright (box-backend.ts), and those need the third place to say the price at all.
        let box_rates: Vec<String> = [0.018, 0.036, 0.072].iter().map(|r| fmt_rate(*r)).collect();
        assert_eq!(box_rates, vec!["$0.018/hr", "$0.036/hr", "$0.072/hr"]);
        let rates: Vec<String> = [0.1, 1.234, 2.5].iter().map(|r| fmt_rate(*r)).collect();
        assert_eq!(rates, vec!["$0.10/hr", "$1.234/hr", "$2.50/hr"]);
    }

    #[test]
    fn offered_size_is_the_one_membership_rule_and_the_refusal_names_the_word_as_given() {
        let offers = offers();
        assert!(offered_size(&offers, &size(2.0, 8192.0)));
        assert!(!offered_size(&offers, &size(4.0, 8192.0)));
        assert!(!offered_size(&[] as &[WorkspaceSize], &size(2.0, 4096.0)));
        // The same match answers with the row itself, which is where a picker reads the rate it quotes.
        assert_eq!(
            size_offer(&offers, &size(2.0, 8192.0)).cloned(),
            Some(wire::<MachineSizeOffer>(json!({ "cpu": 2, "memMb": 8192, "rateUsdPerHour": 0.15 })))
        );
        assert_eq!(size_offer(&offers, &size(4.0, 8192.0)), None);
        assert_eq!(fmt_rate(0.11), "$0.11/hr");
        assert_eq!(size_refusal("4x8", &offers), "4x8 is not a size this provider offers; the sizes are 2x4 ($0.11/hr), 2x8 ($0.15/hr)");
        assert_eq!(size_refusal("big", &offers), "big is not a size this provider offers; the sizes are 2x4 ($0.11/hr), 2x8 ($0.15/hr)");
    }
}

mod machine_cap_refusal {
    use super::*;

    const NONE: &[&str] = &[];

    #[test]
    fn names_the_machines_holding_the_slots_and_the_move_that_frees_one_without_saying() {
        assert_eq!(machine_cap_refusal(&["first", "t-cap"], NONE, None), "both machine slots are in use: first, t-cap. Pause one or wait for a nap.");
        // A slot held by a machine this host cannot name is still held, so one holder is no proof of a one-slot plan.
        assert_eq!(machine_cap_refusal(&["first"], NONE, None), "a machine slot is in use: first. Pause it or wait for a nap.");
        assert_eq!(machine_cap_refusal(&["a", "b", "c"], NONE, None), "machine slots are in use: a, b, c. Pause one or wait for a nap.");
        assert_eq!(machine_cap_refusal(&["a", "b", "c", "d"], NONE, None), "machine slots are in use: a, b, c, d. Pause one or wait for a nap.");
    }

    #[test]
    fn names_a_builder_as_one_since_the_pause_on_offer_is_a_workspace_s_move() {
        assert_eq!(
            machine_cap_refusal(&["first"], &["wsp-golden"], None),
            "both machine slots are in use: first, wsp-golden (builder). Pause one or wait for a nap."
        );
    }

    #[test]
    fn says_so_plainly_when_nothing_of_this_computer_holds_a_slot_instead_of_naming_an() {
        assert_eq!(
            machine_cap_refusal(NONE, NONE, None),
            "the provider is at its machine cap and no machine of this computer holds a slot; free one at the provider and try again"
        );
    }

    #[test]
    fn keeps_the_provider_s_own_words_when_its_refusal_was_read_as_the_cap_by_status_al() {
        assert_eq!(
            machine_cap_refusal(&["first"], NONE, Some("Please slow down.")),
            "a machine slot is in use: first. Pause it or wait for a nap. The provider said: Please slow down."
        );
        assert_eq!(
            machine_cap_refusal(NONE, NONE, Some("Please slow down.")),
            "the provider is at its machine cap and no machine of this computer holds a slot; free one at the provider and try again. The provider said: Please slow down."
        );
    }
}

mod the_words_the_clients_print_for_the_job {
    use super::*;

    const MIB: f64 = 1024.0 * 1024.0;

    #[test]
    fn every_step_s_tally_reads_the_step_s_count_and_the_running_image_estimate_against() {
        let gib = 1024.0 * MIB;
        let agents: InitScreen = wire(json!({
            "id": "agents", "title": "Agents", "top": "Which agents go on the image",
            "items": [
                { "id": "claude", "label": "Claude Code", "size": 208.0 * MIB, "detail": [] },
                { "id": "codex", "label": "Codex", "size": 455.0 * MIB, "detail": [] },
                { "id": "hermes", "label": "Hermes Agent", "size": 484.0 * MIB, "detail": [] },
            ],
            "ticks": ["claude", "codex"], "answers": {}, "footer": [], "tally": "agents",
        }));
        let tools: InitScreen = wire(json!({
            "id": "tools", "title": "Tools", "top": "Tools from your usage",
            "items": [
                { "id": "node", "label": "Node", "size": 300.0 * MIB, "lock": "on", "detail": [] },
                { "id": "rust", "label": "Rust", "size": 4.0 * gib, "detail": [] },
                { "id": "swift", "label": "Swift", "size": 3.0 * gib, "detail": [] },
                { "id": "bun", "label": "bun", "size": null, "detail": [] },
            ],
            "ticks": ["rust", "bun"], "answers": {}, "footer": [], "tally": "tools",
        }));
        let logins: InitScreen = wire(json!({
            "id": "logins", "title": "Sign-ins", "top": "How sign-ins reach the machine",
            "items": [{ "id": "logins/gh", "label": "GitHub CLI login", "detail": [], "choices": [{ "value": "copy", "label": "copy" }] }],
            "ticks": [], "answers": {}, "footer": [],
        }));
        let job_of = |fixed: f64, drafts: Option<Value>| -> InitJob {
            let mut job = json!({
                "id": "j", "road": "manual", "phase": "answering", "keys": {}, "step": 0, "stoppable": false, "rows": [], "progress": { "done": 0, "total": 0 }, "log": [],
                "disk": { "fixed": fixed, "total": 20.0 * gib }, "screens": [agents, tools, logins],
            });
            if let Some(d) = drafts {
                job["drafts"] = d;
            }
            wire(job)
        };
        let job = job_of(0.0, None);
        let total = 20.0 * gib;
        let set = |ids: &[&str]| -> Vec<String> { ids.iter().map(|s| s.to_string()).collect() };

        // A locked row is on, a row with a picker is not counted, and a row nothing measured adds nothing.
        assert_eq!(init_tally_of(&tools, &set(&["rust", "bun"]).into_iter().collect::<IndexSet<_>>()), InitTally { count: 3.0, bytes: 300.0 * MIB + 4.0 * gib });
        assert_eq!(init_tally_of(&logins, &IndexSet::new()), InitTally { count: 0.0, bytes: 0.0 });
        // Agents ticked to 663 MB and tools to 4.3 GB: the tools step counts its own rows and reads the whole image.
        assert_eq!(init_image_bytes(&job, None), 663.0 * MIB + 300.0 * MIB + 4.0 * gib);
        assert_eq!(
            [init_tally_count(3.0, "tools"), fmt_bytes_of_total(init_image_bytes(&job, None), Some(total))],
            ["3 tools on the image", "4.9 GB of 20 GB"]
        );
        // What the disk holds before any tick is under every estimate; a draft kept on a step stands over what the host
        // last answered; the draft a client holds and the host has not echoed yet stands over both.
        assert_eq!(init_image_bytes(&job_of(2.0 * gib, None), None), 2.0 * gib + 663.0 * MIB + 300.0 * MIB + 4.0 * gib);
        let drafted = || job_of(0.0, Some(json!([{ "at": "tools", "ticks": ["rust", "swift"], "answers": {} }])));
        assert_eq!(init_image_bytes(&drafted(), None), 663.0 * MIB + 300.0 * MIB + 7.0 * gib);
        assert_eq!(
            init_image_bytes(&drafted(), Some(&CurrentTicks { at: "tools".to_string(), ticks: vec![] })),
            663.0 * MIB + 300.0 * MIB
        );
        assert_eq!(
            init_image_bytes(&job, Some(&CurrentTicks { at: "agents".to_string(), ticks: vec!["claude".to_string()] })),
            208.0 * MIB + 300.0 * MIB + 4.0 * gib
        );
        assert_eq!(init_image_bytes(&wire(json!({ "id": "j", "road": "manual", "phase": "answering", "keys": {}, "step": 0, "stoppable": false, "rows": [], "progress": { "done": 0, "total": 0 }, "log": [], "screens": [] })), None), 0.0);
        // The one rule for a screen's ticks as they stand, which the screen's draft and the estimate both read. The TS
        // spreads a Set in insertion order, which the IndexSet keeps.
        assert_eq!(init_ticks_of(&tools, None).into_iter().collect::<Vec<_>>(), set(&["rust", "bun"]));
        assert_eq!(init_ticks_of(&tools, Some(&wire(json!({ "at": "tools", "ticks": ["swift"], "answers": {} })))).into_iter().collect::<Vec<_>>(), set(&["swift"]));
        // The estimate reads in the tone its share of the disk earns, never its weight: 4.9 GB is the danger weight and a quarter of the disk.
        assert_eq!(disk_tone(init_image_bytes(&job, None), total), SizeTone::Muted);
        assert_eq!(size_tone(init_image_bytes(&job, None)), SizeTone::Danger);
        // The agents screen's sizes are muted whatever they weigh; the tools and what-else screens' wear the weight table; nothing measured is muted anywhere.
        assert_eq!(
            [
                init_size_tone(agents.id, Some(455.0 * MIB)),
                init_size_tone(agents.id, Some(4.0 * gib)),
                init_size_tone(tools.id, Some(455.0 * MIB)),
                init_size_tone(tools.id, Some(4.0 * gib)),
                init_size_tone(InitScreenId::Also, Some(150.0 * MIB)),
                init_size_tone(tools.id, None),
            ],
            [SizeTone::Muted, SizeTone::Muted, SizeTone::Warning, SizeTone::Danger, SizeTone::Yellow, SizeTone::Muted]
        );
    }
}

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::words::units::*;

    fn js<T: serde::Serialize>(v: &T) -> String {
        wsp_words::exit::json_line(&serde_json::to_value(v).unwrap(), None)
    }

    use wsp_wire::wire::capabilities::WorkspaceSize;

    fn sz(cpu: f64, mem_mb: f64) -> WorkspaceSize {
        serde_json::from_value::<WorkspaceSize>(serde_json::json!({ "cpu": cpu, "memMb": mem_mb })).unwrap()
    }

    #[test]
    fn byte_formats_hold_every_tier_edge() {
        assert_eq!(compact_bytes(-5_f64), "0"); // compactBytes(-5)
        assert_eq!(compact_bytes(0_f64), "0"); // compactBytes(0)
        assert_eq!(compact_bytes(999_f64), "999"); // compactBytes(999)
        assert_eq!(compact_bytes(1000_f64), "1.0K"); // compactBytes(1000)
        assert_eq!(compact_bytes(1023_f64), "1.0K"); // compactBytes(1023)
        assert_eq!(compact_bytes(1024_f64), "1.0K"); // compactBytes(1024)
        assert_eq!(compact_bytes(10239_f64), "10.0K"); // compactBytes(10239)
        assert_eq!(compact_bytes(10240_f64), "10K"); // compactBytes(10240)
        assert_eq!(compact_bytes(1023999_f64), "1000K"); // compactBytes(1023999)
        assert_eq!(compact_bytes(1536000_f64), "1.5M"); // compactBytes(1536000)
        assert_eq!(compact_bytes(128974848_f64), "123M"); // compactBytes(128974848)
        assert_eq!(compact_bytes(1048576000_f64), "1.0G"); // compactBytes(1048576000)
        assert_eq!(compact_bytes(2469606195.2_f64), "2.3G"); // compactBytes(2469606195.2)
        assert_eq!(compact_bytes(1099511627776_f64), "1.0T"); // compactBytes(1099511627776)
        assert_eq!(compact_bytes(5497558138880_f64), "5.0T"); // compactBytes(5497558138880)
        assert_eq!(fmt_bytes(0.5_f64), "0.5 B"); // fmtBytes(0.5)
        assert_eq!(fmt_bytes(1023.5_f64), "1023.5 B"); // fmtBytes(1023.5)
        assert_eq!(fmt_bytes(1048575_f64), "1024 KB"); // fmtBytes(1048575)
        assert_eq!(fmt_bytes(1073741823_f64), "1024 MB"); // fmtBytes(1073741823)
        assert_eq!(fmt_bytes(1127428915.2_f64), "1.1 GB"); // fmtBytes(1127428915.2)
        assert_eq!(fmt_bytes(2093796556.8_f64), "1.9 GB"); // fmtBytes(2093796556.8)
        assert_eq!(fmt_bytes(2104533975.04_f64), "2 GB"); // fmtBytes(2104533975.04)
        assert_eq!(fmt_disk_bytes(999_f64), "999 B"); // fmtDiskBytes(999)
        assert_eq!(fmt_disk_bytes(1000_f64), "1 KB"); // fmtDiskBytes(1000)
        assert_eq!(fmt_disk_bytes(999999_f64), "1000 KB"); // fmtDiskBytes(999999)
        assert_eq!(fmt_disk_bytes(1000000_f64), "1 MB"); // fmtDiskBytes(1000000)
        assert_eq!(fmt_disk_bytes(160000000000_f64), "160 GB"); // fmtDiskBytes(160000000000)
        assert_eq!(fmt_disk_bytes(1050000000_f64), "1.1 GB"); // fmtDiskBytes(1050000000)
        assert_eq!(fmt_bytes_over(1048575_f64), "1024 KB"); // fmtBytesOver(1048575)
        assert_eq!(fmt_bytes_over(1048576_f64), "1.0 MB"); // fmtBytesOver(1048576)
        assert_eq!(fmt_bytes_over(1048577_f64), "1.1 MB"); // fmtBytesOver(1048577)
        assert_eq!(fmt_bytes_over(10485761_f64), "10.1 MB"); // fmtBytesOver(10485761)
        assert_eq!(fmt_bytes_over(1073689395.2_f64), "1024.0 MB"); // fmtBytesOver(1073689395.2)
        assert_eq!(fmt_bytes_over(1073741823_f64), "1024.0 MB"); // fmtBytesOver(1073741823)
        assert_eq!(fmt_bytes_over(1073741824_f64), "1 GB"); // fmtBytesOver(1073741824)
        assert_eq!(fmt_bytes_of_total(5.0 * 1073741824.0, None), "5 GB"); // fmtBytesOfTotal(5*1024**3)
        assert_eq!(fmt_mem_gb(512_f64), "0.5 GB"); // fmtMemGb(512)
        assert_eq!(fmt_mem_gb(1024_f64), "1 GB"); // fmtMemGb(1024)
        assert_eq!(fmt_mem_gb(1536_f64), "1.5 GB"); // fmtMemGb(1536)
        assert_eq!(fmt_mem_gb(1600_f64), "1.6 GB"); // fmtMemGb(1600)
        assert_eq!(fmt_mem_gb(1638_f64), "1.6 GB"); // fmtMemGb(1638)
        assert_eq!(fmt_mem_gb(1700_f64), "1.7 GB"); // fmtMemGb(1700)
        assert_eq!(fmt_mem_gb(0_f64), "0 GB"); // fmtMemGb(0)
        assert_eq!(fmt_mem_gb(102.4_f64), "0.1 GB"); // fmtMemGb(102.4)
        assert_eq!(fmt_bytes_of(900.0 * 1048576.0, 3.9 * 1073741824.0, None), "900 MB of 3.9 GB"); // fmtBytesOf(900*1048576, 3.9*1024**3)
        assert_eq!(fmt_bytes_of(3.6 * 1073741824.0, 3.9 * 1073741824.0, None), "3.6 of 3.9 GB"); // fmtBytesOf(3.6*1024**3, 3.9*1024**3)
        assert_eq!(fmt_bytes_of(512.0, 1000.0, None), "512 of 1000 B"); // fmtBytesOf(512, 1000)
        assert_eq!(fmt_bytes_of(160e9, 160e9, Some(fmt_disk_bytes)), "160 of 160 GB"); // fmtBytesOf(160e9, 160e9, fmtDiskBytes)
    }

    #[test]
    fn rates_and_prices_read_the_rounded_thousandth() {
        assert_eq!(charges_nothing(None), true); // chargesNothing(undefined)
        assert_eq!(charges_nothing(Some(0.0)), true); // chargesNothing(0)
        assert_eq!(charges_nothing(Some(-0.0)), true); // chargesNothing(-0)
        assert_eq!(charges_nothing(Some(0.001)), false); // chargesNothing(0.001)
        assert_eq!(fmt_price(0.0), "free"); // fmtPrice(0)
        assert_eq!(fmt_price(0.09000000000000001), "$0.09/hr"); // fmtPrice(0.09000000000000001)
        assert_eq!(fmt_rate_dollars(0.09000000000000001_f64), "$0.09"); // fmtRateDollars(0.09000000000000001)
        assert_eq!(fmt_rate_dollars(0.018_f64), "$0.018"); // fmtRateDollars(0.018)
        assert_eq!(fmt_rate_dollars(0.0185_f64), "$0.018"); // fmtRateDollars(0.0185)
        assert_eq!(fmt_rate_dollars(0.0005_f64), "$0.001"); // fmtRateDollars(0.0005)
        assert_eq!(fmt_rate_dollars(0.0004_f64), "$0.00"); // fmtRateDollars(0.0004)
        assert_eq!(fmt_rate_dollars(1_f64), "$1.00"); // fmtRateDollars(1)
        assert_eq!(fmt_rate_dollars(0.1_f64), "$0.10"); // fmtRateDollars(0.1)
        assert_eq!(fmt_rate_dollars(12.3456_f64), "$12.346"); // fmtRateDollars(12.3456)
        assert_eq!(fmt_rate_dollars(0.0995_f64), "$0.10"); // fmtRateDollars(0.0995)
        assert_eq!(fmt_rate_dollars(0.0094999_f64), "$0.009"); // fmtRateDollars(0.0094999)
    }

    #[test]
    fn tones_switch_exactly_at_their_thresholds() {
        assert_eq!(disk_tone(0_f64, 0_f64).as_str(), "danger"); // diskTone(0, 0)
        assert_eq!(disk_tone(69_f64, 100_f64).as_str(), "muted"); // diskTone(69, 100)
        assert_eq!(disk_tone(70_f64, 100_f64).as_str(), "warning"); // diskTone(70, 100)
        assert_eq!(disk_tone(89.9_f64, 100_f64).as_str(), "warning"); // diskTone(89.9, 100)
        assert_eq!(disk_tone(90_f64, 100_f64).as_str(), "danger"); // diskTone(90, 100)
        assert_eq!(disk_tone(5_f64, 0_f64).as_str(), "danger"); // diskTone(5, 0)
        assert_eq!(disk_tone(0_f64, -1_f64).as_str(), "danger"); // diskTone(0, -1)
        assert_eq!(size_tone(52428799_f64).as_str(), "muted"); // sizeTone(52428799)
        assert_eq!(size_tone(52428800_f64).as_str(), "yellow"); // sizeTone(52428800)
        assert_eq!(size_tone(209715199_f64).as_str(), "yellow"); // sizeTone(209715199)
        assert_eq!(size_tone(209715200_f64).as_str(), "warning"); // sizeTone(209715200)
        assert_eq!(size_tone(1073741823_f64).as_str(), "warning"); // sizeTone(1073741823)
        assert_eq!(size_tone(1073741824_f64).as_str(), "danger"); // sizeTone(1073741824)
        assert_eq!(size_tone(0_f64).as_str(), "muted"); // sizeTone(0)
    }

    #[test]
    fn size_words_parse_and_print_as_the_ts_does() {
        assert_eq!(fmt_cpus(1.0, None), "1 vCPU"); // fmtCpus(1)
        assert_eq!(fmt_cpus(1.0, Some(CpuWord::Cores)), "1 core"); // fmtCpus(1, "cores")
        assert_eq!(fmt_cpus(0.5, Some(CpuWord::Cores)), "0.5 cores"); // fmtCpus(0.5, "cores")
        assert_eq!(size_got_line(&sz(2.0, 4096.0), &sz(4.0, 8192.0), true), "asked for 2\u{a0}vCPU,\u{a0}4\u{a0}GB; the machine has 4\u{a0}vCPU,\u{a0}8\u{a0}GB"); // sizeGotLine({ cpu: 2, memMb: 4096 }, { cpu: 4, memMb: 8192 }, true)
        assert_eq!(size_got_line(&sz(2.0, 4096.0), &sz(4.0, 8192.0), false), "the image's size is 2\u{a0}vCPU,\u{a0}4\u{a0}GB; the machine has 4\u{a0}vCPU,\u{a0}8\u{a0}GB"); // sizeGotLine({ cpu: 2, memMb: 4096 }, { cpu: 4, memMb: 8192 }, false)
        assert_eq!(place_facts_line(&sz(1.0, 512.0), None), "1\u{a0}core,\u{a0}0.5\u{a0}GB"); // placeFactsLine({ cpu: 1, memMb: 512 })
        assert_eq!(size_word(&sz(2.0, 512.0)), "2x0.5"); // sizeWord({ cpu: 2, memMb: 512 })
        assert_eq!(size_word(&sz(2.0, 1000.0)), "2x1"); // sizeWord({ cpu: 2, memMb: 1000 })
        assert_eq!(js(&size_from_word(" 2x4 ")), "{\"cpu\":2,\"memMb\":4096}"); // JSON.stringify(sizeFromWord(" 2x4 ") ?? null)
        assert_eq!(js(&size_from_word("2x0.5")), "{\"cpu\":2,\"memMb\":512}"); // JSON.stringify(sizeFromWord("2x0.5") ?? null)
        assert_eq!(js(&size_from_word("0x4")), "null"); // JSON.stringify(sizeFromWord("0x4") ?? null)
        assert_eq!(js(&size_from_word("2x0")), "null"); // JSON.stringify(sizeFromWord("2x0") ?? null)
        assert_eq!(js(&size_from_word("2x0.0001")), "null"); // JSON.stringify(sizeFromWord("2x0.0001") ?? null)
        assert_eq!(js(&size_from_word("2X4")), "null"); // JSON.stringify(sizeFromWord("2X4") ?? null)
        assert_eq!(js(&size_from_word("2x4.")), "null"); // JSON.stringify(sizeFromWord("2x4.") ?? null)
        assert_eq!(js(&size_from_word("02x04")), "{\"cpu\":2,\"memMb\":4096}"); // JSON.stringify(sizeFromWord("02x04") ?? null)
        assert_eq!(js(&size_from_word("2x4x")), "null"); // JSON.stringify(sizeFromWord("2x4x") ?? null)
        assert_eq!(js(&size_from_word("")), "null"); // JSON.stringify(sizeFromWord("") ?? null)
        assert_eq!(js(&size_from_word("1x0.0005")), "{\"cpu\":1,\"memMb\":1}"); // JSON.stringify(sizeFromWord("1x0.0005") ?? null)
        assert_eq!(size_refusal("9x9", &[]), "9x9 is not a size this provider offers; the sizes are "); // sizeRefusal("9x9", [])
    }

    #[test]
    fn durations_tokens_and_clocks_hold_their_edges() {
        assert_eq!(fmt_duration(f64::NAN, None), "0ms"); // fmtDuration(NaN)
        assert_eq!(fmt_duration(f64::INFINITY, None), "0ms"); // fmtDuration(Infinity)
        assert_eq!(fmt_duration(f64::NEG_INFINITY, None), "0ms"); // fmtDuration(-Infinity)
        assert_eq!(fmt_duration(59999.9_f64, None), "60s"); // fmtDuration(59999.9)
        assert_eq!(fmt_duration(3599500_f64, None), "60m"); // fmtDuration(3599500)
        assert_eq!(fmt_duration(86399999_f64, None), "1440m"); // fmtDuration(86399999)
        assert_eq!(fmt_duration(f64::NAN, Some(DurationStyle::Clock)), "0m 00s"); // fmtDuration(NaN, "clock")
        assert_eq!(fmt_duration(-1_f64, Some(DurationStyle::Clock)), "0m 00s"); // fmtDuration(-1, "clock")
        assert_eq!(fmt_duration(0_f64, Some(DurationStyle::Clock)), "0m 00s"); // fmtDuration(0, "clock")
        assert_eq!(fmt_duration(499_f64, Some(DurationStyle::Clock)), "0m 00s"); // fmtDuration(499, "clock")
        assert_eq!(fmt_duration(500_f64, Some(DurationStyle::Clock)), "0m 01s"); // fmtDuration(500, "clock")
        assert_eq!(fmt_duration(59500_f64, Some(DurationStyle::Clock)), "1m 00s"); // fmtDuration(59500, "clock")
        assert_eq!(fmt_duration(3599499_f64, Some(DurationStyle::Clock)), "59m 59s"); // fmtDuration(3599499, "clock")
        assert_eq!(fmt_duration(3600000_f64, Some(DurationStyle::Clock)), "1h 00m 00s"); // fmtDuration(3600000, "clock")
        assert_eq!(fmt_tokens(f64::NAN), "0"); // fmtTokens(NaN)
        assert_eq!(fmt_tokens(-5_f64), "0"); // fmtTokens(-5)
        assert_eq!(fmt_tokens(0_f64), "0"); // fmtTokens(0)
        assert_eq!(fmt_tokens(0.4_f64), "0"); // fmtTokens(0.4)
        assert_eq!(fmt_tokens(0.5_f64), "1"); // fmtTokens(0.5)
        assert_eq!(fmt_tokens(999.4_f64), "999"); // fmtTokens(999.4)
        assert_eq!(fmt_tokens(999.5_f64), "1k"); // fmtTokens(999.5)
        assert_eq!(fmt_tokens(999499_f64), "999k"); // fmtTokens(999499)
        assert_eq!(fmt_tokens(999500_f64), "1M"); // fmtTokens(999500)
        assert_eq!(fmt_tokens(1000000_f64), "1M"); // fmtTokens(1000000)
        assert_eq!(fmt_tokens(4270_f64), "4.27k"); // fmtTokens(4270)
        assert_eq!(fmt_tokens(22600000_f64), "22.6M"); // fmtTokens(22600000)
        assert_eq!(fmt_tokens(687000000_f64), "687M"); // fmtTokens(687000000)
        assert_eq!(fmt_tokens(999499999_f64), "999M"); // fmtTokens(999499999)
        assert_eq!(fmt_tokens(999500000_f64), "1B"); // fmtTokens(999500000)
        assert_eq!(fmt_tokens(7370000000_f64), "7.37B"); // fmtTokens(7370000000)
        assert_eq!(fmt_tokens(1500000000000000_f64), "1500000B"); // fmtTokens(1500000000000000)
        assert_eq!(fmt_tokens(9999_f64), "10k"); // fmtTokens(9999)
        assert_eq!(fmt_tokens(99950_f64), "100k"); // fmtTokens(99950)
        assert_eq!(fmt_tokens(99949_f64), "99.9k"); // fmtTokens(99949)
        assert_eq!(fmt_uptime(f64::NAN), "0m"); // fmtUptime(NaN)
        assert_eq!(fmt_uptime(-1_f64), "0m"); // fmtUptime(-1)
        assert_eq!(fmt_uptime(59999_f64), "0m"); // fmtUptime(59999)
        assert_eq!(fmt_uptime(60000_f64), "1m"); // fmtUptime(60000)
        assert_eq!(fmt_uptime(3599999_f64), "59m"); // fmtUptime(3599999)
        assert_eq!(fmt_uptime(3600000_f64), "1h 0m"); // fmtUptime(3600000)
        assert_eq!(fmt_uptime(86399999_f64), "23h 59m"); // fmtUptime(86399999)
        assert_eq!(fmt_uptime(86400000_f64), "1d 0h"); // fmtUptime(86400000)
        assert_eq!(fmt_uptime(90061000_f64), "1d 1h"); // fmtUptime(90061000)
        assert_eq!(fmt_elapsed(f64::NAN), "0s"); // fmtElapsed(NaN)
        assert_eq!(fmt_elapsed(-1_f64), "0s"); // fmtElapsed(-1)
        assert_eq!(fmt_elapsed(999_f64), "0s"); // fmtElapsed(999)
        assert_eq!(fmt_elapsed(59999_f64), "59s"); // fmtElapsed(59999)
        assert_eq!(fmt_elapsed(60000_f64), "1m"); // fmtElapsed(60000)
        assert_eq!(fmt_elapsed(61500_f64), "1m 1s"); // fmtElapsed(61500)
        assert_eq!(fmt_elapsed(3600000_f64), "60m"); // fmtElapsed(3600000)
    }

    #[test]
    fn money_reads_in_the_locales_the_ts_names() {
        assert_eq!(fmt_calls(0_f64), "0 calls"); // fmtCalls(0)
        assert_eq!(fmt_calls(1_f64), "1 call"); // fmtCalls(1)
        assert_eq!(fmt_calls(2_f64), "2 calls"); // fmtCalls(2)
        assert_eq!(fmt_calls(1234_f64), "1,234 calls"); // fmtCalls(1234)
        assert_eq!(fmt_calls(1234567_f64), "1,234,567 calls"); // fmtCalls(1234567)
        assert_eq!(fmt_calls(1.5_f64), "1.5 calls"); // fmtCalls(1.5)
        assert_eq!(fmt_cost(-0.004_f64), "$-0.00"); // fmtCost(-0.004)
        assert_eq!(fmt_cost(-12.5_f64), "$-12.50"); // fmtCost(-12.5)
        assert_eq!(fmt_cost(0.30000000000000004_f64), "$0.30"); // fmtCost(0.30000000000000004)
        assert_eq!(fmt_inr(0_f64), "\u{20b9}0"); // fmtInr(0)
        assert_eq!(fmt_inr(1_f64), "\u{20b9}1"); // fmtInr(1)
        assert_eq!(fmt_inr(144740_f64), "\u{20b9}1,44,740"); // fmtInr(144740)
        assert_eq!(fmt_inr(144740.5_f64), "\u{20b9}1,44,740.50"); // fmtInr(144740.5)
        assert_eq!(fmt_inr(0.995_f64), "\u{20b9}1"); // fmtInr(0.995)
        assert_eq!(fmt_inr(1.004_f64), "\u{20b9}1"); // fmtInr(1.004)
        assert_eq!(fmt_inr(100000.25_f64), "\u{20b9}1,00,000.25"); // fmtInr(100000.25)
        assert_eq!(fmt_inr(-5000_f64), "-\u{20b9}5,000"); // fmtInr(-5000)
    }

    #[test]
    fn boat_lines_say_only_what_boat_answered() {
        assert_eq!(machine_cap_refusal(&[] as &[&str], &[] as &[&str], Some("")), "the provider is at its machine cap and no machine of this computer holds a slot; free one at the provider and try again"); // machineCapRefusal([], [], "")
        assert_eq!(machine_cap_refusal(&[] as &[&str], &[] as &[&str], Some("Too many concurrent sessions")), "the provider is at its machine cap and no machine of this computer holds a slot; free one at the provider and try again. The provider said: Too many concurrent sessions"); // machineCapRefusal([], [], "Too many concurrent sessions")
        assert_eq!(machine_cap_refusal(&["a", "b", "c"], &[] as &[&str], None), "machine slots are in use: a, b, c. Pause one or wait for a nap."); // machineCapRefusal(["a", "b", "c"], [], undefined)
        assert_eq!(starts_used_line(StartWindow::Minute, 12.0, 12.0, Some("the trial"), Some("spoo-1")), "Boat's starts for this minute are used (12 of 12 on the trial), so spoo-1 was not started; start it again in a minute."); // startsUsedLine("minute", 12, 12, "the trial", "spoo-1")
        assert_eq!(starts_used_line(StartWindow::Hour, 60.0, 60.0, None, Some("")), "Boat's starts for this hour are used (60 of 60), so the machine was not started; start it again later in the hour, or move to a plan with more at boat.dev/dashboard."); // startsUsedLine("hour", 60, 60, undefined, "")
        assert_eq!(starts_used_line(StartWindow::Day, 200.0, 200.0, Some("the $20 plan"), None), "Boat's starts for today are used (200 of 200 on the $20 plan), so the machine was not started; Boat counts the day again tomorrow, or a plan with more starts is at boat.dev/dashboard."); // startsUsedLine("day", 200, 200, "the $20 plan", undefined)
        assert_eq!(boat_plan_words(true, Some(20.0)), Some("the trial".to_string())); // boatPlanWords(true, 20)
        assert_eq!(boat_plan_words(false, None), None); // boatPlanWords(false, undefined)
        assert_eq!(boat_plan_words(false, Some(19.5)), Some("the $19.5 plan".to_string())); // boatPlanWords(false, 19.5)
        assert_eq!(boat_request_limit_wait_line("POST /boxes", 1001.0, 2.0, 3.0), "POST /boxes met Boat's limit of 1,000 requests a minute from one address; asking again in 2s, try 2 of 3"); // boatRequestLimitWaitLine("POST /boxes", 1001, 2, 3)
        assert_eq!(boat_request_limit_wait_line("GET /limits", 0.0, 1.0, 3.0), "GET /limits met Boat's limit of 1,000 requests a minute from one address; asking again in 0s, try 1 of 3"); // boatRequestLimitWaitLine("GET /limits", 0, 1, 3)
        assert_eq!(boat_cancelled_line(None, None), "Boat could not get a machine and cancelled the create; start it again."); // boatCancelledLine(undefined, undefined)
        assert_eq!(boat_cancelled_line(Some("spoo"), Some("")), "Boat could not get a machine for spoo and cancelled the create; start it again."); // boatCancelledLine("spoo", "")
        assert_eq!(boat_cancelled_line(Some("spoo"), Some("no room")), "Boat could not get a machine for spoo and cancelled the create (no room); start it again."); // boatCancelledLine("spoo", "no room")
        assert_eq!(boat_box_state_line("b1", "pending"), "Boat's machine b1 is pending"); // boatBoxStateLine("b1", "pending")
        assert_eq!(boat_refused_log_line("POST /boxes", 429.0, None, None, "slow down\nsecond line"), "Boat refused POST /boxes: 429 code=none requestId=none: slow down"); // boatRefusedLogLine("POST /boxes", 429, undefined, undefined, "slow down\nsecond line")
        assert_eq!(boat_refused_log_line("POST /boxes", 500.0, Some("E1"), Some("r-9"), &"y".repeat(305)), "Boat refused POST /boxes: 500 code=E1 requestId=r-9: yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy"); // boatRefusedLogLine("POST /boxes", 500, "E1", "r-9", "y".repeat(305))
    }

    #[test]
    fn disk_expiry_holds_at_the_keep_instant() {
        assert_eq!(disk_gone_released_line("x"), "x; the next send starts a new machine from the image"); // diskGoneReleasedLine("x")
        assert_eq!(disk_gone_replaced_line("x"), "x; it runs on a new machine from the image now"); // diskGoneReplacedLine("x")
        assert_eq!(disk_expired_line("Oct 9"), "Its machine's disk was kept until Oct 9, so a send may find it gone and start a new machine"); // diskExpiredLine("Oct 9")
        assert_eq!(disk_expired(&DiskKeep { disk_kept_until: Some(100.0), status: "idle", asking: None }, 100.0), true); // diskExpired({ diskKeptUntil: 100, status: "idle" }, 100)
        assert_eq!(disk_expired(&DiskKeep { disk_kept_until: Some(101.0), status: "idle", asking: None }, 100.0), false); // diskExpired({ diskKeptUntil: 101, status: "idle" }, 100)
        assert_eq!(disk_expired(&DiskKeep { disk_kept_until: Some(100.0), status: "running", asking: None }, 200.0), false); // diskExpired({ diskKeptUntil: 100, status: "running" }, 200)
        assert_eq!(disk_expired(&DiskKeep { disk_kept_until: Some(100.0), status: "idle", asking: Some("t1") }, 200.0), false); // diskExpired({ diskKeptUntil: 100, status: "idle", asking: "t1" }, 200)
        assert_eq!(disk_expired(&DiskKeep { disk_kept_until: Some(100.0), status: "idle", asking: Some("") }, 200.0), false); // diskExpired({ diskKeptUntil: 100, status: "idle", asking: "" }, 200)
        assert_eq!(disk_expired(&DiskKeep { disk_kept_until: None, status: "idle", asking: None }, 200.0), false); // diskExpired({ status: "idle" }, 200)
    }

    #[test]
    fn the_other_lines_read_as_the_ts_writes_them() {
        assert_eq!(init_tally_count(3.0, "agents"), "3 agents on the image"); // initTallyCount(3, "agents")
        assert_eq!(init_tally_count(1.0, "tools"), "1 tool on the image"); // initTallyCount(1, "tools")
        assert_eq!(init_tally_count(4.0, "more"), "4 more on the image"); // initTallyCount(4, "more")
        assert_eq!(init_tally_count(2.0, "class"), "2 class on the image"); // initTallyCount(2, "class")
        assert_eq!(init_disk_line(3.0 * 1073741824.0, 2.0 * 1073741824.0), "about 3 GB of 2 GB on the image, over by 1 GB"); // initDiskLine(3*1024**3, 2*1024**3)
        assert_eq!(init_disk_line(2.0 * 1073741824.0, 2.0 * 1073741824.0), "about 2 GB of 2 GB on the image"); // initDiskLine(2*1024**3, 2*1024**3)
        assert_eq!(fmt_px(13.5), "13.5 px"); // fmtPx(13.5)
        assert_eq!(present_elsewhere_line("node", "/opt/x/bin/node", "by brew", &["/opt/homebrew/bin", "/usr/local/bin"]), "node answers from /opt/x/bin/node, outside where brew puts it (/opt/homebrew/bin, /usr/local/bin)"); // presentElsewhereLine("node", "/opt/x/bin/node", "by brew", ["/opt/homebrew/bin", "/usr/local/bin"])
        assert_eq!(present_elsewhere_line("node", "/x", "npm", &[] as &[&str]), "node answers from /x, outside where npm puts it ()"); // presentElsewhereLine("node", "/x", "npm", [])
        assert_eq!(present_elsewhere_line("node", "/x", "bytheway road", &["/a"]), "node answers from /x, outside where bytheway road puts it (/a)"); // presentElsewhereLine("node", "/x", "bytheway road", ["/a"])
        assert_eq!(doctor_computer_row_line("spoo", "2026-10-25T10:00:00.000Z"), "the computers row read it present at 2026-10-25T10:00:00.000Z; wsp add spoo --update reads it again"); // doctorComputerRowLine("spoo", "2026-10-25T10:00:00.000Z")
        assert_eq!(here_daemon_behind_line(7.0, 9.0, "wsp update"), "this computer's wsp daemon is version 7 and this wsp needs 9; wsp update stages the right one"); // hereDaemonBehindLine(7, 9, "wsp update")
        assert_eq!(copy_verb_failed_line(-1.0), "the daemon's copy verb failed (exit -1); the host log has its output"); // copyVerbFailedLine(-1)
    }
}
