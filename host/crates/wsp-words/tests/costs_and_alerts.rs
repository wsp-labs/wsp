// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/outside-line.test.ts (lines 1-45), plan-alerts.test.ts (1-53) and
//! cost-history.test.ts (1-150) at a3b870a6e.

mod support;

use serde_json::json;
use wsp_wire::types::general_prefs::GeneralPreferences;
use wsp_wire::types::outside_line::OutsideLine;
use wsp_wire::types::plan_alerts::PlanAlert;
use wsp_wire::types::usage::AccountLimit;
use wsp_wire::views::workspace_events::WorkspaceCostEvent;

mod outside_line_tests {
    use super::*;
    use wsp_words::outside_line::{outside_line, OutsideMoment};
    use wsp_words::words::init::NEEDS_YOU;

    // The GENERAL_DEFAULTS of packages/protocol/src/general-prefs.ts, the three fields outside_line reads.
    fn default_preferences() -> GeneralPreferences {
        serde_json::from_value(crate::support::general_preferences()).unwrap()
    }

    // const loud = { notifyNeeds: "notify-sound", notifyDone: "notify-sound", planAlerts: true }, with fields overridden.
    fn loud(needs: &str, done: &str, plan_alerts: bool) -> GeneralPreferences {
        serde_json::from_value(crate::support::over(crate::support::general_preferences(), json!({ "notifyNeeds": needs, "notifyDone": done, "planAlerts": plan_alerts }))).unwrap()
    }

    fn line(title: &str, body: &str, show: bool, sound: bool) -> Option<OutsideLine> {
        Some(OutsideLine { title: title.to_string(), body: body.to_string(), show, sound, id: None })
    }

    fn s(x: &str) -> String {
        x.to_string()
    }

    mod a_moment_said_outside_the_app {
        use super::*;

        #[test]
        fn a_prompt_a_question_and_a_sign_in_need_the_person_and_sound_by_default() {
            assert_eq!(
                outside_line(&OutsideMoment::Asks { line: s("Permission for Bash: ls") }, &default_preferences()),
                line(NEEDS_YOU, "Permission for Bash: ls", true, true)
            );
            assert_eq!(
                outside_line(&OutsideMoment::SignIn { what: s("sign in to GitHub CLI") }, &loud("notify", "notify-sound", true)),
                line(NEEDS_YOU, "sign in to GitHub CLI", true, false)
            );
            assert_eq!(outside_line(&OutsideMoment::Asks { line: s("x") }, &loud("off", "notify-sound", true)), None);
        }

        #[test]
        fn a_finish_a_failure_and_a_machine_that_came_up_follow_when_a_thread_finishes_shown_with_no_s() {
            let finished = || OutsideMoment::Finished { thread: s("Fix login"), r#where: s("spoo") };
            let awake = || OutsideMoment::Awake { workspace: s("b1") };
            assert_eq!(outside_line(&finished(), &default_preferences()), line("Fix login finished", "spoo", true, false));
            assert_eq!(outside_line(&awake(), &default_preferences()), line(NEEDS_YOU, "b1 is awake", true, false));
            assert_eq!(outside_line(&finished(), &loud("notify-sound", "off", true)), None);
            assert_eq!(outside_line(&finished(), &loud("notify-sound", "notify-sound", true)), line("Fix login finished", "spoo", true, true));
            assert_eq!(
                outside_line(
                    &OutsideMoment::Failed { thread: s("Fix login"), r#where: s("spoo"), error: Some(s("API Error: 529\n overloaded")) },
                    &loud("notify-sound", "notify-sound", true)
                ),
                line("Fix login stopped", "API Error: 529 overloaded", true, true)
            );
            assert_eq!(
                outside_line(&OutsideMoment::Failed { thread: s("Fix login"), r#where: s("spoo"), error: None }, &loud("notify-sound", "sound", true)),
                line("Fix login stopped", "spoo", false, true)
            );
            assert_eq!(outside_line(&awake(), &loud("off", "notify-sound", true)), line(NEEDS_YOU, "b1 is awake", true, true));
        }

        #[test]
        fn a_plan_alert_shows_with_no_sound_under_its_own_switch() {
            let plan = || OutsideMoment::Plan { label: s("Claude Max"), alert: serde_json::from_value(serde_json::json!({ "kind": "blocked" })).unwrap() };
            assert_eq!(outside_line(&plan(), &loud("off", "off", true)), line("Claude Max reached its plan limit", "", true, false));
            assert_eq!(outside_line(&plan(), &loud("notify-sound", "notify-sound", false)), None);
        }

        #[test]
        fn a_computer_s_setup_says_it_failed_or_needs_the_person_under_the_needs_you_choice_and_ready_() {
            assert_eq!(
                outside_line(&OutsideMoment::SetupFailed { computer: s("spoo"), said: s("the base tools did not install:\n apt said no") }, &default_preferences()),
                line("spoo: setup failed", "the base tools did not install: apt said no", true, true)
            );
            assert_eq!(
                outside_line(&OutsideMoment::SetupNeedsYou { computer: s("spoo"), what: s("sign in to Claude Code") }, &default_preferences()),
                line(NEEDS_YOU, "spoo needs you: sign in to Claude Code", true, true)
            );
            assert_eq!(
                outside_line(&OutsideMoment::SetupReady { computer: s("spoo"), missed: None }, &default_preferences()),
                line("spoo is ready", "", true, false)
            );
            assert_eq!(outside_line(&OutsideMoment::SetupReady { computer: s("spoo"), missed: None }, &loud("notify-sound", "off", true)), None);
            assert_eq!(
                outside_line(&OutsideMoment::SetupReady { computer: s("spoo"), missed: Some(s("2 folders did not clone")) }, &loud("notify-sound", "notify-sound", true)),
                line("spoo is ready; 2 folders did not clone", "", true, true)
            );
            assert_eq!(outside_line(&OutsideMoment::SetupFailed { computer: s("spoo"), said: s("x") }, &loud("off", "notify-sound", true)), None);
        }

        // Not ported: "a line keeps the page's id for the click to hand back, and drops an open an older page put on it"
        // (outside-line.test.ts:38-43). It parses the OutsideLine wire type and calls no function of this crate, so
        // it could not fail against todo!().
    }
}

mod plan_alerts_tests {
    use super::*;
    use wsp_words::plan_alerts::{block_ends_at, plan_alert_line, plan_alerts_of};

    const AT: f64 = 1_000_000.0;

    // const reading = (windows, status?) => ({ windows, ...(status !== undefined ? { status } : {}) });
    fn reading(windows: serde_json::Value, status: Option<&str>) -> AccountLimit {
        let mut v = json!({ "key": "k", "agent": "claude", "label": "Claude", "windows": windows, "readAt": 0, "computers": [] });
        if let Some(status) = status {
            v["status"] = json!(status);
        }
        serde_json::from_value(v).unwrap()
    }

    fn alerts(v: serde_json::Value) -> Vec<PlanAlert> {
        serde_json::from_value(v).unwrap()
    }

    mod plan_alerts {
        use super::*;

        #[test]
        fn says_70_and_90_of_a_window_once_each_and_only_as_the_reading_gets_worse() {
            assert_eq!(plan_alerts_of(None, &reading(json!([{ "kind": "session", "usedPercent": 40 }]), None), AT), alerts(json!([])));
            assert_eq!(
                plan_alerts_of(
                    Some(&reading(json!([{ "kind": "session", "usedPercent": 40 }]), None)),
                    &reading(json!([{ "kind": "session", "usedPercent": 72 }]), None),
                    AT
                ),
                alerts(json!([{ "kind": "low", "window": "session", "step": 70 }]))
            );
            assert_eq!(
                plan_alerts_of(
                    Some(&reading(json!([{ "kind": "session", "usedPercent": 72 }]), None)),
                    &reading(json!([{ "kind": "session", "usedPercent": 80 }]), None),
                    AT
                ),
                alerts(json!([]))
            );
            assert_eq!(
                plan_alerts_of(
                    Some(&reading(json!([{ "kind": "session", "usedPercent": 80 }]), None)),
                    &reading(json!([{ "kind": "session", "usedPercent": 78 }]), None),
                    AT
                ),
                alerts(json!([]))
            );
            assert_eq!(
                plan_alerts_of(
                    Some(&reading(json!([{ "kind": "session", "usedPercent": 80 }]), None)),
                    &reading(json!([{ "kind": "session", "usedPercent": 91 }]), None),
                    AT
                ),
                alerts(json!([{ "kind": "low", "window": "session", "step": 90 }]))
            );
            assert_eq!(
                plan_alerts_of(
                    Some(&reading(json!([{ "kind": "session", "usedPercent": 91 }]), None)),
                    &reading(json!([{ "kind": "session", "usedPercent": 91 }]), None),
                    AT
                ),
                alerts(json!([]))
            );
        }

        #[test]
        fn a_reading_that_jumps_past_both_steps_says_the_higher_and_each_window_speaks_for_itself() {
            let before = reading(json!([{ "kind": "session", "usedPercent": 10 }, { "kind": "week", "usedPercent": 60 }]), None);
            assert_eq!(
                plan_alerts_of(
                    Some(&before),
                    &reading(json!([{ "kind": "session", "usedPercent": 95 }, { "kind": "week", "usedPercent": 71 }]), None),
                    AT
                ),
                alerts(json!([{ "kind": "low", "window": "session", "step": 90 }, { "kind": "low", "window": "week", "step": 70 }]))
            );
        }

        #[test]
        fn a_window_that_started_again_since_the_last_reading_counts_from_nothing_so_its_new_run_says_() {
            assert_eq!(
                plan_alerts_of(
                    Some(&reading(json!([{ "kind": "session", "usedPercent": 95, "resetsAt": AT - 1.0 }]), None)),
                    &reading(json!([{ "kind": "session", "usedPercent": 74, "resetsAt": AT + 5.0 * 3_600_000.0 }]), None),
                    AT
                ),
                alerts(json!([{ "kind": "low", "window": "session", "step": 70 }]))
            );
        }

        #[test]
        fn overage_is_use_past_the_plan_and_says_nothing() {
            assert_eq!(
                plan_alerts_of(
                    Some(&reading(json!([{ "kind": "overage", "usedPercent": 0 }]), None)),
                    &reading(json!([{ "kind": "overage", "usedPercent": 99 }]), None),
                    AT
                ),
                alerts(json!([]))
            );
        }

        #[test]
        fn a_block_is_said_once_as_itself_and_nothing_lower_and_a_reading_free_of_it_after_says_the_ac() {
            let blocked = reading(json!([{ "kind": "session", "usedPercent": 100, "resetsAt": AT + 60_000.0 }]), Some("reached"));
            assert_eq!(
                plan_alerts_of(Some(&reading(json!([{ "kind": "session", "usedPercent": 80 }]), None)), &blocked, AT),
                alerts(json!([{ "kind": "blocked" }]))
            );
            assert_eq!(plan_alerts_of(Some(&blocked), &blocked, AT), alerts(json!([])));
            assert_eq!(
                plan_alerts_of(Some(&blocked), &reading(json!([{ "kind": "session", "usedPercent": 2 }]), Some("ok")), AT),
                alerts(json!([{ "kind": "back" }]))
            );
            // A used-up window is a block where the agent printed no state at all.
            assert_eq!(
                plan_alerts_of(
                    Some(&reading(json!([{ "kind": "week", "usedPercent": 95 }]), None)),
                    &reading(json!([{ "kind": "week", "usedPercent": 100 }]), None),
                    AT
                ),
                alerts(json!([{ "kind": "blocked" }]))
            );
        }

        #[test]
        fn a_block_ends_at_the_latest_reset_among_its_used_up_windows_and_nowhere_for_an_account_that_() {
            assert_eq!(
                block_ends_at(&reading(
                    json!([
                        { "kind": "session", "usedPercent": 100, "resetsAt": AT + 10.0 },
                        { "kind": "week", "usedPercent": 100, "resetsAt": AT + 99.0 },
                        { "kind": "week_opus", "usedPercent": 50, "resetsAt": AT + 500.0 }
                    ]),
                    Some("reached")
                )),
                Some(AT + 99.0)
            );
            assert_eq!(
                block_ends_at(&reading(json!([{ "kind": "session", "usedPercent": 60, "resetsAt": AT + 10.0 }]), Some("reached"))),
                Some(AT + 10.0)
            );
            assert_eq!(block_ends_at(&reading(json!([{ "kind": "session", "usedPercent": 60, "resetsAt": AT + 10.0 }]), None)), None);
        }

        #[test]
        fn says_each_alert_in_a_line_that_names_the_account() {
            let low: PlanAlert = serde_json::from_value(json!({ "kind": "low", "window": "session", "step": 70 })).unwrap();
            assert_eq!(plan_alert_line("Claude Max", &low), "Claude Max has used 70% of its session");
            assert_eq!(plan_alert_line("Claude Max", &serde_json::from_value::<PlanAlert>(serde_json::json!({ "kind": "blocked" })).unwrap()), "Claude Max reached its plan limit");
            assert_eq!(plan_alert_line("Claude Max", &serde_json::from_value::<PlanAlert>(serde_json::json!({ "kind": "back" })).unwrap()), "Claude Max can run again: its plan limit reset");
        }
    }
}

mod cost_history_tests {
    use super::*;
    use jiff::tz::TimeZone;
    use jiff::Timestamp;
    use wsp_words::cost_history::{accrued_at, append_cost_point, day_start, month_start, rate_at, spent_since};
    use wsp_wire::types::cost_history::COST_HISTORY_CAP;
    use wsp_words::words::places::spend_meter_word;

    // Date.parse of an ISO stamp, in ms.
    fn parse(iso: &str) -> f64 {
        iso.parse::<Timestamp>().unwrap().as_millisecond() as f64
    }

    // Date.UTC(2026, 8, 5, 9, minute)
    fn at(minute: i64) -> f64 {
        parse("2026-09-05T09:00:00Z") + (minute * 60_000) as f64
    }

    // new Date(ms).toISOString()
    fn iso(ms: f64) -> String {
        let zoned = Timestamp::from_millisecond(ms as i64).unwrap().to_zoned(TimeZone::UTC);
        format!("{}.000Z", zoned.strftime("%Y-%m-%dT%H:%M:%S"))
    }

    fn tick(minute: i64, rate: f64, accrued_usd: f64) -> WorkspaceCostEvent {
        serde_json::from_value(json!({
            "type": "workspace.cost",
            "workspaceId": "ws_a",
            "phase": if rate > 0.0 { "running" } else { "napping" },
            "rateUsdPerHour": rate,
            "awakeMs": minute * 60_000,
            "accruedUsd": accrued_usd,
            "at": iso(at(minute)),
        }))
        .unwrap()
    }

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 5e-11, "{a} vs {b}");
    }

    fn minutes(points: &[WorkspaceCostEvent]) -> Vec<String> {
        points.iter().map(|p| p.at[14..16].to_string()).collect()
    }

    mod append_cost_point_tests {
        use super::*;

        #[test]
        fn keeps_the_first_tick_the_newest_tick_and_the_ticks_either_side_of_a_rate_change_nothing_be() {
            let mut points: Vec<WorkspaceCostEvent> = vec![];
            for t in [tick(0, 0.11, 0.0), tick(1, 0.11, 0.1), tick(2, 0.11, 0.2), tick(3, 0.11, 0.3)] {
                points = append_cost_point(&points, &t);
            }
            assert_eq!(points.iter().map(|p| p.at.clone()).collect::<Vec<_>>(), vec![tick(0, 0.0, 0.0).at, tick(3, 0.0, 0.0).at]);
            for t in [tick(4, 0.0, 0.3), tick(5, 0.0, 0.3), tick(6, 0.0, 0.3)] {
                points = append_cost_point(&points, &t);
            }
            assert_eq!(
                points.iter().map(|p| (p.at[14..16].to_string(), p.rate_usd_per_hour.0)).collect::<Vec<_>>(),
                vec![("00".to_string(), 0.11), ("03".to_string(), 0.11), ("04".to_string(), 0.0), ("06".to_string(), 0.0)]
            );
            for t in [tick(7, 0.22, 0.3), tick(8, 0.22, 0.5)] {
                points = append_cost_point(&points, &t);
            }
            assert_eq!(minutes(&points), vec!["00", "03", "04", "06", "07", "08"]);
        }

        #[test]
        fn returns_a_new_array_and_leaves_the_one_given_alone() {
            let before = vec![tick(0, 0.11, 0.0), tick(1, 0.11, 0.1)];
            let after = append_cost_point(&before, &tick(2, 0.11, 0.2));
            assert_eq!(before.len(), 2);
            assert_eq!(minutes(&after), vec!["00", "02"]);
        }

        #[test]
        fn drops_the_oldest_points_past_the_cap() {
            let cap = COST_HISTORY_CAP as usize;
            let mut points: Vec<WorkspaceCostEvent> = vec![];
            for i in 0..(cap as i64 + 10) {
                points = append_cost_point(&points, &tick(i, if i % 2 == 0 { 0.11 } else { 0.0 }, i as f64));
            }
            assert_eq!(points.len(), cap);
            // The pending WorkspaceCostEvent has no awakeMs, so the newest point is told by its accruedUsd and its stamp
            // (the TS reads awakeMs, (CAP + 9) * 60_000).
            let last = points.last().unwrap();
            assert_eq!(last.accrued_usd.0, (cap + 9) as f64);
            assert_eq!(last.at, tick(cap as i64 + 9, 0.0, 0.0).at);
        }
    }

    mod what_a_series_says_at_one_instant {
        use super::*;

        #[test]
        fn reads_the_total_between_two_ticks_off_the_line_between_them_and_holds_it_past_the_newest() {
            let points = [tick(0, 0.12, 0.0), tick(30, 0.12, 0.06)];
            assert_eq!(accrued_at(&points, at(0)), Some(0.0));
            close(accrued_at(&points, at(15)).unwrap(), 0.03);
            assert_eq!(accrued_at(&points, at(30)), Some(0.06));
            assert_eq!(accrued_at(&points, at(90)), Some(0.06));
        }

        #[test]
        fn says_nothing_at_all_before_the_first_tick_which_is_a_total_nothing_here_knows() {
            assert_eq!(accrued_at(&[tick(10, 0.12, 0.0)], at(9)), None);
            assert_eq!(accrued_at(&[], at(9)), None);
            assert_eq!(rate_at(&[tick(10, 0.12, 0.0)], at(9)), None);
        }

        #[test]
        fn holds_a_tick_s_rate_until_the_next_one() {
            let points = [tick(0, 0.12, 0.0), tick(30, 0.0, 0.06)];
            assert_eq!(rate_at(&points, at(29)), Some(0.12));
            assert_eq!(rate_at(&points, at(31)), Some(0.0));
        }
    }

    mod what_a_month_took {
        use super::*;

        #[test]
        fn begins_a_month_at_midnight_on_its_first_day_in_the_zone_the_computer_is_set_to() {
            // The TS reads the process's zone; this runs in UTC.
            let instant = parse("2026-09-13T04:20:00.000Z");
            let start = month_start(instant, &TimeZone::UTC);
            let d = Timestamp::from_millisecond(start as i64).unwrap().to_zoned(TimeZone::UTC);
            assert_eq!((d.day(), d.hour(), d.minute(), d.second()), (1, 0, 0, 0));
            assert_eq!(d.month(), Timestamp::from_millisecond(instant as i64).unwrap().to_zoned(TimeZone::UTC).month());
            assert_eq!(month_start(start, &TimeZone::UTC), start);
        }

        #[test]
        fn counts_what_a_series_took_since_an_instant_taking_the_total_at_that_instant_off_the_newest() {
            let points = [tick(0, 0.12, 0.5), tick(60, 0.12, 0.62)];
            close(spent_since(&points, parse("2026-09-05T09:00:00Z"), None), 0.12);
            close(spent_since(&points, parse("2026-09-05T09:30:00Z"), None), 0.06);
        }

        #[test]
        fn counts_the_whole_of_a_series_that_began_after_the_instant_and_nothing_from_one_that_ended_b() {
            let points = [tick(0, 0.12, 0.0), tick(60, 0.12, 0.12)];
            close(spent_since(&points, parse("2026-09-01T00:00:00Z"), None), 0.12);
            assert_eq!(spent_since(&points, parse("2026-10-01T00:00:00Z"), None), 0.0);
            assert_eq!(spent_since(&[], parse("2026-09-01T00:00:00Z"), None), 0.0);
        }
    }

    mod what_a_day_took {
        use super::*;

        #[test]
        fn begins_a_day_at_midnight_in_the_zone_the_computer_is_set_to_whatever_day_it_is_in_utc() {
            let instant = parse("2026-09-13T04:20:00.000Z");
            let zone = |name: &str| TimeZone::get(name).unwrap();
            assert_eq!(day_start(instant, &zone("America/New_York")), parse("2026-09-13T04:00:00.000Z"));
            assert_eq!(day_start(instant, &zone("America/Los_Angeles")), parse("2026-09-12T07:00:00.000Z"));
            assert_eq!(day_start(instant, &zone("Asia/Kolkata")), parse("2026-09-12T18:30:00.000Z"));
            // The TS reads the process's zone here; this runs in UTC.
            let start = day_start(instant, &TimeZone::UTC);
            let local = Timestamp::from_millisecond(start as i64).unwrap().to_zoned(TimeZone::UTC);
            assert_eq!((local.hour(), local.minute(), local.second(), local.subsec_nanosecond() / 1_000_000), (0, 0, 0, 0));
            assert_eq!(day_start(start, &TimeZone::UTC), start);
        }

        #[test]
        fn keeps_midnight_across_a_change_of_the_clocks_so_the_day_the_clocks_go_back_is_25_hours_long() {
            let ny = TimeZone::get("America/New_York").unwrap();
            let fall_back = day_start(parse("2026-11-01T20:00:00.000Z"), &ny);
            let after = day_start(parse("2026-11-02T20:00:00.000Z"), &ny);
            assert_eq!(fall_back, parse("2026-11-01T04:00:00.000Z"));
            assert_eq!(after, parse("2026-11-02T05:00:00.000Z"));
            assert_eq!(after - fall_back, 25.0 * 3_600_000.0);
            assert_eq!(day_start(parse("2026-03-08T12:00:00.000Z"), &ny), parse("2026-03-08T05:00:00.000Z"));
            assert_eq!(day_start(parse("2026-03-09T12:00:00.000Z"), &ny), parse("2026-03-09T04:00:00.000Z"));
        }

        #[test]
        fn carries_a_running_series_on_at_its_newest_rate_up_to_the_instant_asked_for() {
            let running = [tick(0, 0.12, 0.0), tick(30, 0.12, 0.06)];
            close(spent_since(&running, at(0), Some(at(90))), 0.18);
            // An instant past the newest tick is on the same line, so a day that began after it counts only its own share.
            close(spent_since(&running, at(60), Some(at(90))), 0.06);
            close(spent_since(&running, at(15), Some(at(30))), 0.03);
            close(spent_since(&running, at(0), None), 0.06);
        }

        #[test]
        fn carries_nothing_past_a_newest_tick_at_rate_0_a_machine_asleep_costs_nothing_more() {
            let asleep = [tick(0, 0.12, 0.0), tick(30, 0.0, 0.06)];
            close(spent_since(&asleep, at(0), Some(at(90))), 0.06);
            assert_eq!(spent_since(&asleep, at(60), Some(at(90))), 0.0);
            assert_eq!(spent_since(&[], at(0), Some(at(90))), 0.0);
        }
    }

    mod spend_meter_word_tests {
        use super::*;

        // spend_meter_word is a stub of words/places.rs, not of cost_history.rs; the TS test sits in cost-history.test.ts.
        #[test]
        fn reads_what_a_cloud_spent_today_over_its_spend_per_day_the_limit_in_whole_dollars_where_it_h() {
            assert_eq!(spend_meter_word(2.314, 10.0), "$2.31/$10");
            assert_eq!(spend_meter_word(0.0, 10.0), "$0.00/$10");
            assert_eq!(spend_meter_word(12.5, 12.5), "$12.50/$12.50");
            assert_eq!(spend_meter_word(0.0, 0.0), "$0.00/$0");
        }
    }
}


// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::plan_alerts::*;

    use serde_json::{json, Value};
    use wsp_wire::types::plan_alerts::PlanAlert;
    use wsp_wire::types::usage::AccountLimit;
    use wsp_wire::views::workspace_events::WorkspaceCostEvent;
    use wsp_words::cost_history::*;
    use wsp_words::outside_line::*;

    fn js<T: serde::Serialize>(v: &T) -> String {
        wsp_words::exit::json_line(&serde_json::to_value(v).unwrap(), None)
    }

    fn ev(v: Value) -> WorkspaceCostEvent {
        serde_json::from_value(v).unwrap()
    }

    fn lim(v: Value) -> AccountLimit {
        serde_json::from_value(crate::support::over(crate::support::account_limit(), v)).unwrap()
    }

    fn how(h: Option<wsp_words::general_prefs::NotifyHow>) -> String {
        match h {
            None => "null".into(),
            Some(h) => js(&json!({ "show": h.show, "sound": h.sound })),
        }
    }

    #[test]
    fn a_series_keeps_only_the_ticks_where_the_rate_changes() {
        assert_eq!(js(&append_cost_point(&[], &ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"}))).iter().map(|p| p.at.clone()).collect::<Vec<_>>()), "[\"2026-10-01T00:00:00.000Z\"]"); // JSON.stringify(appendCostPoint([], {"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"}).map(p => p.at))
        assert_eq!(js(&append_cost_point(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"}))], &ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.05,"at":"2026-10-01T00:30:00.000Z"}))).iter().map(|p| p.at.clone()).collect::<Vec<_>>()), "[\"2026-10-01T00:00:00.000Z\",\"2026-10-01T00:30:00.000Z\"]"); // JSON.stringify(appendCostPoint([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"}], {"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.05,"at":"2026-10-01T00:30:00.000Z"}).map(p => p.at))
        assert_eq!(js(&append_cost_point(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:01:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:02:00.000Z"}))], &ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:03:00.000Z"}))).iter().map(|p| p.at.clone()).collect::<Vec<_>>()), "[\"2026-10-01T00:00:00.000Z\",\"2026-10-01T00:01:00.000Z\",\"2026-10-01T00:03:00.000Z\"]"); // JSON.stringify(appendCostPoint([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:01:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:02:00.000Z"}], {"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:03:00.000Z"}).map(p => p.at))
        assert_eq!(js(&append_cost_point(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:01:00.000Z"}))], &ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:03:00.000Z"}))).iter().map(|p| p.at.clone()).collect::<Vec<_>>()), "[\"2026-10-01T00:00:00.000Z\",\"2026-10-01T00:01:00.000Z\",\"2026-10-01T00:03:00.000Z\"]"); // JSON.stringify(appendCostPoint([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:01:00.000Z"}], {"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:03:00.000Z"}).map(p => p.at))
    }

    #[test]
    fn a_total_reads_off_the_line_between_ticks() {
        assert_eq!(accrued_past(&ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.36,"awakeMs":0,"accruedUsd":1.5,"at":"2026-10-01T00:00:00.000Z"})), 3600000.0), 1.8599999999999999); // accruedPast({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.36,"awakeMs":0,"accruedUsd":1.5,"at":"2026-10-01T00:00:00.000Z"}, 3600000)
        assert_eq!(accrued_past(&ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":2,"at":"2026-10-01T00:00:00.000Z"})), 10.0), 2.0); // accruedPast({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":2,"at":"2026-10-01T00:00:00.000Z"}, 10)
        assert_eq!(accrued_at(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}))], 1790812799999.0).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "null".into()), "null"); // String(accruedAt([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}], 1790812799999) ?? "null")
        assert_eq!(rate_at(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}))], 1790812799999.0).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "null".into()), "null"); // String(rateAt([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}], 1790812799999) ?? "null")
        assert_eq!(accrued_at(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}))], 1790812800000.0).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "null".into()), "0"); // String(accruedAt([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}], 1790812800000) ?? "null")
        assert_eq!(rate_at(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}))], 1790812800000.0).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "null".into()), "0.1"); // String(rateAt([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}], 1790812800000) ?? "null")
        assert_eq!(accrued_at(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}))], 1790814600000.0).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "null".into()), "0.05"); // String(accruedAt([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}], 1790814600000) ?? "null")
        assert_eq!(rate_at(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}))], 1790814600000.0).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "null".into()), "0.1"); // String(rateAt([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}], 1790814600000) ?? "null")
        assert_eq!(accrued_at(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}))], 1790816400000.0).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "null".into()), "0.1"); // String(accruedAt([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}], 1790816400000) ?? "null")
        assert_eq!(rate_at(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}))], 1790816400000.0).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "null".into()), "0.1"); // String(rateAt([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}], 1790816400000) ?? "null")
        assert_eq!(accrued_at(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}))], 1790821800000.0).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "null".into()), "0.30000000000000004"); // String(accruedAt([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}], 1790821800000) ?? "null")
        assert_eq!(rate_at(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}))], 1790821800000.0).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "null".into()), "0.2"); // String(rateAt([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}], 1790821800000) ?? "null")
        assert_eq!(accrued_at(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}))], 1790842800000.0).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "null".into()), "0.4"); // String(accruedAt([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}], 1790842800000) ?? "null")
        assert_eq!(rate_at(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}))], 1790842800000.0).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "null".into()), "0"); // String(rateAt([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}], 1790842800000) ?? "null")
        assert_eq!(accrued_at(&[], 5.0).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "null".into()), "null"); // String(accruedAt([], 5) ?? "null")
        assert_eq!(accrued_at(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":1,"at":"2026-10-01T00:10:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":2,"at":"2026-10-01T00:10:00.000Z"}))], 1790813400000.0).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "null".into()), "2"); // String(accruedAt([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":1,"at":"2026-10-01T00:10:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":2,"at":"2026-10-01T00:10:00.000Z"}], 1790813400000) ?? "null")
        assert_eq!(wsp_words::js::number_string(spent_since(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}))], 1790812800000.0, None)), "0.4"); // String(spentSince([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}], 1790812800000, undefined))
        assert_eq!(wsp_words::js::number_string(spent_since(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}))], 1790814600000.0, None)), "0.35000000000000003"); // String(spentSince([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}], 1790814600000, undefined))
        assert_eq!(wsp_words::js::number_string(spent_since(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}))], 1790814600000.0, Some(1790818200000.0))), "0.10000000000000002"); // String(spentSince([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}], 1790814600000, 1790818200000))
        assert_eq!(wsp_words::js::number_string(spent_since(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}))], 1790809200000.0, Some(1790816400000.0))), "0.1"); // String(spentSince([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}], 1790809200000, 1790816400000))
        assert_eq!(wsp_words::js::number_string(spent_since(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}))], 1790848800000.0, Some(1790836800000.0))), "0"); // String(spentSince([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}], 1790848800000, 1790836800000))
        assert_eq!(wsp_words::js::number_string(spent_since(&[ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"})), ev(json!({"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}))], 1790827200000.0, Some(1790830800000.0))), "0"); // String(spentSince([{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0,"at":"2026-10-01T00:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.1,"awakeMs":0,"accruedUsd":0.1,"at":"2026-10-01T01:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0.2,"awakeMs":0,"accruedUsd":0.2,"at":"2026-10-01T02:00:00.000Z"},{"type":"workspace.cost","workspaceId":"w","phase":"running","rateUsdPerHour":0,"awakeMs":0,"accruedUsd":0.4,"at":"2026-10-01T03:00:00.000Z"}], 1790827200000, 1790830800000))
        assert_eq!(wsp_words::js::number_string(spent_since(&[], 0.0, None)), "0"); // String(spentSince([], 0))
    }

    #[test]
    fn months_and_days_start_at_local_midnight() {
        assert_eq!(month_start(1792069500000.0, &jiff::tz::TimeZone::UTC), 1790812800000.0); // monthStart(1792069500000)
        assert_eq!(day_start(1792069500000.0, &jiff::tz::TimeZone::UTC), 1792022400000.0); // dayStart(1792069500000)
        assert_eq!(month_start(1767225600000.0, &jiff::tz::TimeZone::UTC), 1767225600000.0); // monthStart(1767225600000)
        assert_eq!(day_start(1767225600000.0, &jiff::tz::TimeZone::UTC), 1767225600000.0); // dayStart(1767225600000)
        assert_eq!(month_start(1798761599999.0, &jiff::tz::TimeZone::UTC), 1796083200000.0); // monthStart(1798761599999)
        assert_eq!(day_start(1798761599999.0, &jiff::tz::TimeZone::UTC), 1798675200000.0); // dayStart(1798761599999)
    }

    #[test]
    fn a_plan_is_blocked_and_alerts_only_as_it_gets_worse() {
        assert_eq!(plan_blocked(&lim(json!({"windows":[],"status":"reached"}))), true); // planBlocked({"windows":[],"status":"reached"})
        assert_eq!(plan_blocked(&lim(json!({"windows":[{"kind":"session","usedPercent":100}]}))), true); // planBlocked({"windows":[{"kind":"session","usedPercent":100}]})
        assert_eq!(plan_blocked(&lim(json!({"windows":[{"kind":"overage","usedPercent":150}]}))), false); // planBlocked({"windows":[{"kind":"overage","usedPercent":150}]})
        assert_eq!(plan_blocked(&lim(json!({"windows":[{"kind":"week","usedPercent":99.9}]}))), false); // planBlocked({"windows":[{"kind":"week","usedPercent":99.9}]})
        assert_eq!(plan_blocked(&lim(json!({"windows":[]}))), false); // planBlocked({"windows":[]})
        assert_eq!(block_ends_at(&lim(json!({"windows":[{"kind":"session","usedPercent":100,"resetsAt":50},{"kind":"week","usedPercent":100,"resetsAt":90},{"kind":"month","usedPercent":10,"resetsAt":999}]}))).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "none".into()), "90"); // String(blockEndsAt({"windows":[{"kind":"session","usedPercent":100,"resetsAt":50},{"kind":"week","usedPercent":100,"resetsAt":90},{"kind":"month","usedPercent":10,"resetsAt":999}]}) ?? "none")
        assert_eq!(block_ends_at(&lim(json!({"windows":[{"kind":"session","usedPercent":10,"resetsAt":50},{"kind":"week","usedPercent":20,"resetsAt":90}],"status":"reached"}))).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "none".into()), "90"); // String(blockEndsAt({"windows":[{"kind":"session","usedPercent":10,"resetsAt":50},{"kind":"week","usedPercent":20,"resetsAt":90}],"status":"reached"}) ?? "none")
        assert_eq!(block_ends_at(&lim(json!({"windows":[{"kind":"session","usedPercent":100}],"status":"reached"}))).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "none".into()), "none"); // String(blockEndsAt({"windows":[{"kind":"session","usedPercent":100}],"status":"reached"}) ?? "none")
        assert_eq!(block_ends_at(&lim(json!({"windows":[{"kind":"overage","usedPercent":100,"resetsAt":7},{"kind":"session","usedPercent":100,"resetsAt":5}]}))).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "none".into()), "5"); // String(blockEndsAt({"windows":[{"kind":"overage","usedPercent":100,"resetsAt":7},{"kind":"session","usedPercent":100,"resetsAt":5}]}) ?? "none")
        assert_eq!(block_ends_at(&lim(json!({"windows":[]}))).map(|n| wsp_words::js::number_string(n)).unwrap_or_else(|| "none".into()), "none"); // String(blockEndsAt({"windows":[]}) ?? "none")
        assert_eq!(js(&plan_alerts_of(None, &lim(json!({"windows":[{"kind":"session","usedPercent":75}]})), 100.0)), "[{\"kind\":\"low\",\"window\":\"session\",\"step\":70}]"); // JSON.stringify(planAlertsOf(undefined, {"windows":[{"kind":"session","usedPercent":75}]}, 100))
        assert_eq!(js(&plan_alerts_of(Some(&lim(json!({"windows":[{"kind":"session","usedPercent":75}]}))), &lim(json!({"windows":[{"kind":"session","usedPercent":80}]})), 100.0)), "[]"); // JSON.stringify(planAlertsOf({"windows":[{"kind":"session","usedPercent":75}]}, {"windows":[{"kind":"session","usedPercent":80}]}, 100))
        assert_eq!(js(&plan_alerts_of(Some(&lim(json!({"windows":[{"kind":"session","usedPercent":60}]}))), &lim(json!({"windows":[{"kind":"session","usedPercent":95},{"kind":"week","usedPercent":70}]})), 100.0)), "[{\"kind\":\"low\",\"window\":\"session\",\"step\":90},{\"kind\":\"low\",\"window\":\"week\",\"step\":70}]"); // JSON.stringify(planAlertsOf({"windows":[{"kind":"session","usedPercent":60}]}, {"windows":[{"kind":"session","usedPercent":95},{"kind":"week","usedPercent":70}]}, 100))
        assert_eq!(js(&plan_alerts_of(Some(&lim(json!({"windows":[{"kind":"session","usedPercent":95,"resetsAt":10}]}))), &lim(json!({"windows":[{"kind":"session","usedPercent":95}]})), 100.0)), "[{\"kind\":\"low\",\"window\":\"session\",\"step\":90}]"); // JSON.stringify(planAlertsOf({"windows":[{"kind":"session","usedPercent":95,"resetsAt":10}]}, {"windows":[{"kind":"session","usedPercent":95}]}, 100))
        assert_eq!(js(&plan_alerts_of(Some(&lim(json!({"windows":[{"kind":"session","usedPercent":95,"resetsAt":1000}]}))), &lim(json!({"windows":[{"kind":"session","usedPercent":96}]})), 100.0)), "[]"); // JSON.stringify(planAlertsOf({"windows":[{"kind":"session","usedPercent":95,"resetsAt":1000}]}, {"windows":[{"kind":"session","usedPercent":96}]}, 100))
        assert_eq!(js(&plan_alerts_of(Some(&lim(json!({"windows":[],"status":"reached"}))), &lim(json!({"windows":[]})), 100.0)), "[{\"kind\":\"back\"}]"); // JSON.stringify(planAlertsOf({"windows":[],"status":"reached"}, {"windows":[]}, 100))
        assert_eq!(js(&plan_alerts_of(Some(&lim(json!({"windows":[]}))), &lim(json!({"windows":[],"status":"reached"})), 100.0)), "[{\"kind\":\"blocked\"}]"); // JSON.stringify(planAlertsOf({"windows":[]}, {"windows":[],"status":"reached"}, 100))
        assert_eq!(js(&plan_alerts_of(Some(&lim(json!({"windows":[],"status":"reached"}))), &lim(json!({"windows":[],"status":"reached"})), 100.0)), "[]"); // JSON.stringify(planAlertsOf({"windows":[],"status":"reached"}, {"windows":[],"status":"reached"}, 100))
        assert_eq!(js(&plan_alerts_of(None, &lim(json!({"windows":[{"kind":"overage","usedPercent":99}]})), 100.0)), "[]"); // JSON.stringify(planAlertsOf(undefined, {"windows":[{"kind":"overage","usedPercent":99}]}, 100))
        assert_eq!(plan_alert_line("Claude Max", &serde_json::from_value::<PlanAlert>(json!({"kind":"low","window":"week_opus","step":70})).unwrap()), "Claude Max has used 70% of its Opus week"); // planAlertLine("Claude Max", {"kind":"low","window":"week_opus","step":70})
        assert_eq!(plan_alert_line("Claude Max", &serde_json::from_value::<PlanAlert>(json!({"kind":"low","window":"overage","step":90})).unwrap()), "Claude Max has used 90% of its extra usage"); // planAlertLine("Claude Max", {"kind":"low","window":"overage","step":90})
        assert_eq!(plan_alert_line("Claude Max", &serde_json::from_value::<PlanAlert>(json!({"kind":"blocked"})).unwrap()), "Claude Max reached its plan limit"); // planAlertLine("Claude Max", {"kind":"blocked"})
        assert_eq!(plan_alert_line("Claude Max", &serde_json::from_value::<PlanAlert>(json!({"kind":"back"})).unwrap()), "Claude Max can run again: its plan limit reset"); // planAlertLine("Claude Max", {"kind":"back"})
    }

    #[test]
    fn setup_lines_and_notify_choices() {
        assert_eq!(setup_failed_line("spoo"), "spoo: setup failed"); // setupFailedLine("spoo")
        assert_eq!(setup_needs_you_line("spoo", "a sign-in"), "spoo needs you: a sign-in"); // setupNeedsYouLine("spoo", "a sign-in")
        assert_eq!(setup_ready_line("spoo", None), "spoo is ready"); // setupReadyLine("spoo")
        assert_eq!(setup_ready_line("spoo", Some("codex was not signed in")), "spoo is ready; codex was not signed in"); // setupReadyLine("spoo", "codex was not signed in")
        assert_eq!(how(wsp_words::general_prefs::notify_by(serde_json::from_value(json!("off")).unwrap())), "null"); // JSON.stringify(notifyBy("off") ?? null)
        assert_eq!(how(wsp_words::general_prefs::notify_by(serde_json::from_value(json!("notify")).unwrap())), "{\"show\":true,\"sound\":false}"); // JSON.stringify(notifyBy("notify") ?? null)
        assert_eq!(how(wsp_words::general_prefs::notify_by(serde_json::from_value(json!("sound")).unwrap())), "{\"show\":false,\"sound\":true}"); // JSON.stringify(notifyBy("sound") ?? null)
        assert_eq!(how(wsp_words::general_prefs::notify_by(serde_json::from_value(json!("notify-sound")).unwrap())), "{\"show\":true,\"sound\":true}"); // JSON.stringify(notifyBy("notify-sound") ?? null)
    }
}
