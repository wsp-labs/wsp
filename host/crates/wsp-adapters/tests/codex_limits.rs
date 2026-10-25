// SPDX-License-Identifier: AGPL-3.0-only
//! packages/adapter-codex/test/reset.test.ts and the `creditsOf` describe of adapter.test.ts.

#[path = "common/codex_shell.rs"]
mod codex_shell;

use indexmap::IndexMap;
use regex::Regex;
use serde_json::{json, Value};
use wsp_adapters::codex::limits::credits_of;
use wsp_adapters::codex::reset::{parse_reset_credit, parse_reset_read, reset_credit_command, reset_read_command, CodexPlanResets, TOO_OLD};
use wsp_adapters::port::{HarnessLimit, PlanResets, ResetReading, ResetRoad, ResetSpend};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::usage::{BankedResets, HarnessLimitAccount, LimitKind, LimitStatus, LimitWindow, ResetCredit, ResetCreditStatus};

// The two scripts a banked reset is read and spent by, run under bash against a fake `codex app-server` that exits
// on stdin EOF as the real one does.

fn run(script: &str) -> String {
    let tmp = std::env::temp_dir().to_string_lossy().into_owned();
    let ran = codex_shell::run_shell("bash", script, &codex_shell::pairs(&[("PATH", "/usr/bin:/bin"), ("HOME", &tmp)]), "");
    assert_eq!(ran.status, 0, "{}", ran.stderr);
    ran.stdout
}

fn road(f: &codex_shell::FakeAppServer) -> ResetRoad {
    let mut base = IndexMap::new();
    base.insert("PATH".to_string(), Some(f.path.clone()));
    ResetRoad { home: f.home.clone(), env: Some(base), launch: None }
}

fn bare_road(home: &str) -> ResetRoad {
    ResetRoad { home: home.to_string(), env: None, launch: None }
}

fn chatgpt() -> Value {
    json!({ "result": { "account": { "type": "chatgpt", "email": "dev@example.com", "planType": "plus" }, "requiresOpenaiAuth": true } })
}

fn snapshot(used: u64) -> Value {
    json!({ "limitId": "codex", "primary": { "usedPercent": used, "windowDurationMins": 300, "resetsAt": 1_790_700_000u64 }, "secondary": null, "planType": "plus", "rateLimitReachedType": null })
}

fn limits(used: u64, available_count: u64, credits: Value) -> Value {
    json!({ "result": { "rateLimits": snapshot(used), "rateLimitsByLimitId": { "codex": snapshot(used) }, "accountId": "acct_7f3a", "rateLimitResetCredits": { "availableCount": available_count, "credits": credits } } })
}

fn credit(id: &str, expires_at: u64) -> Value {
    json!({ "id": id, "resetType": "codexRateLimits", "status": "available", "grantedAt": 1_790_000_000u64, "expiresAt": expires_at, "title": null, "description": null })
}

fn methods(f: &codex_shell::FakeAppServer) -> Vec<String> {
    f.requests().iter().map(|r| r["method"].as_str().unwrap().to_string()).collect()
}

fn session_window(used: f64) -> LimitWindow {
    LimitWindow { kind: LimitKind::Session, used_percent: JsNumber(used), resets_at: Some(JsNumber(1_790_700_000_000.0)) }
}

mod reading_the_banked_resets {
    use super::*;

    #[test]
    fn asks_the_sign_in_and_the_limits_in_full_under_the_login_s_own_home_and_path_and() {
        let f = codex_shell::fake_app_server(json!({
            "account/read": chatgpt(),
            "account/rateLimits/read": limits(80, 2, json!([credit("rc_1", 1_792_000_000), credit("rc_2", 1_793_000_000)])),
        }));
        let script = reset_read_command(&road(&f)).unwrap();
        assert!(script.contains(&format!("CODEX_HOME='{}'", f.home)), "{script}");
        let read = parse_reset_read(&run(&script));
        let asked: Vec<(String, Value)> = f.requests().iter().map(|r| (r["method"].as_str().unwrap().to_string(), r["params"].clone())).collect();
        assert_eq!(
            asked,
            [
                ("initialize".to_string(), json!({ "clientInfo": { "name": "wsp", "version": "0" } })),
                ("initialized".to_string(), json!({})),
                ("account/read".to_string(), json!({})),
                ("account/rateLimits/read".to_string(), json!({})),
            ]
        );
        assert_eq!(
            read,
            Some(ResetReading {
                keyed: false,
                limit: Some(HarnessLimit {
                    windows: vec![session_window(80.0)],
                    plan: Some("plus".to_string()),
                    status: Some(LimitStatus::Ok),
                    account: Some(HarnessLimitAccount { id: "acct_7f3a".to_string(), label: Some("dev@example.com".to_string()) }),
                    keyed: None,
                    credits: Some(BankedResets {
                        count: 2,
                        credits: Some(vec![
                            ResetCredit { id: "rc_1".to_string(), status: ResetCreditStatus::Available, expires_at: Some(JsNumber(1_792_000_000_000.0)) },
                            ResetCredit { id: "rc_2".to_string(), status: ResetCreditStatus::Available, expires_at: Some(JsNumber(1_793_000_000_000.0)) },
                        ]),
                    }),
                }),
            })
        );
    }

    #[test]
    fn reads_a_sign_in_by_api_key_as_keyed_which_has_no_reset_to_spend() {
        let f = codex_shell::fake_app_server(json!({
            "account/read": { "result": { "account": { "type": "apiKey" }, "requiresOpenaiAuth": true } },
            "account/rateLimits/read": { "error": { "code": -32600, "message": "chatgpt authentication required" } },
        }));
        let script = reset_read_command(&road(&f)).unwrap();
        assert_eq!(parse_reset_read(&run(&script)), Some(ResetReading { keyed: true, limit: None }));
    }

    #[test]
    fn reads_nothing_where_the_server_never_answered() {
        assert_eq!(parse_reset_read("bash: codex: command not found\n"), None);
    }

    #[test]
    fn refuses_a_relative_home_as_the_turn_env_does() {
        let err = reset_read_command(&bare_road(".codex")).unwrap_err();
        assert!(err.contains("absolute"), "{err}");
    }
}

mod spending_one {
    use super::*;

    #[test]
    fn consumes_under_the_caller_s_key_and_the_credit_it_named_reads_the_limits_again_o() {
        let f = codex_shell::fake_app_server(json!({
            "account/rateLimitResetCredit/consume": { "result": { "outcome": "reset" } },
            "account/rateLimits/read": limits(0, 1, json!([credit("rc_2", 1_793_000_000)])),
        }));
        let script = reset_credit_command(&road(&f), "0b6f7d2e-5f1c-4f43-9a51-2c8e1f0e9a11", Some("rc_1")).unwrap();
        let out = run(&script);
        let asked = f.requests();
        assert_eq!(methods(&f), ["initialize", "initialized", "account/rateLimitResetCredit/consume", "account/rateLimits/read"]);
        assert_eq!(asked[2]["params"], json!({ "idempotencyKey": "0b6f7d2e-5f1c-4f43-9a51-2c8e1f0e9a11", "creditId": "rc_1" }));
        // The read went only after the consume's answer, so it reads the windows the reset left.
        assert_eq!(asked[3]["after"], json!(2));
        assert_eq!(asked[3]["params"], json!({}));
        match parse_reset_credit(&out) {
            ResetSpend::Outcome { outcome, limit } => {
                assert_eq!(outcome, "reset");
                let limit = limit.expect("a limit");
                assert_eq!(limit.windows, vec![session_window(0.0)]);
                assert_eq!(limit.credits.expect("credits").count, 1);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn leaves_the_credit_to_the_backend_s_pick_where_none_is_named() {
        let f = codex_shell::fake_app_server(json!({
            "account/rateLimitResetCredit/consume": { "result": { "outcome": "nothingToReset" } },
            "account/rateLimits/read": limits(10, 2, Value::Null),
        }));
        let script = reset_credit_command(&road(&f), "k-1", None).unwrap();
        match parse_reset_credit(&run(&script)) {
            ResetSpend::Outcome { outcome, .. } => assert_eq!(outcome, "nothingToReset"),
            other => panic!("{other:?}"),
        }
        assert_eq!(f.requests()[2]["params"], json!({ "idempotencyKey": "k-1" }));
    }

    #[test]
    fn hands_every_outcome_on_as_codex_said_it_one_it_does_not_know_included() {
        for outcome in ["noCredit", "alreadyRedeemed", "resetPartly"] {
            let f = codex_shell::fake_app_server(json!({ "account/rateLimitResetCredit/consume": { "result": { "outcome": outcome } } }));
            let script = reset_credit_command(&road(&f), "k-1", None).unwrap();
            match parse_reset_credit(&run(&script)) {
                ResetSpend::Outcome { outcome: got, .. } => assert_eq!(got, outcome),
                other => panic!("{outcome}: {other:?}"),
            }
        }
    }

    #[test]
    fn reads_a_server_that_has_no_such_method_as_one_too_old_for_resets_and_any_other_r() {
        let missing = codex_shell::fake_app_server(json!({ "account/rateLimitResetCredit/consume": { "error": { "code": -32601, "message": "Method not found" } } }));
        let script = reset_credit_command(&road(&missing), "k-1", None).unwrap();
        assert_eq!(parse_reset_credit(&run(&script)), ResetSpend::Refused { refused: "Method not found".to_string(), too_old: true });
        let refused = codex_shell::fake_app_server(json!({ "account/rateLimitResetCredit/consume": { "error": { "code": -32600, "message": "chatgpt authentication required" } } }));
        let script = reset_credit_command(&road(&refused), "k-1", None).unwrap();
        assert_eq!(parse_reset_credit(&run(&script)), ResetSpend::Refused { refused: "chatgpt authentication required".to_string(), too_old: false });
    }

    #[test]
    fn says_the_consume_went_unanswered_where_the_server_died_before_answering_it_and_s() {
        let f = codex_shell::fake_app_server(json!({ "account/rateLimitResetCredit/consume": "exit" }));
        let script = reset_credit_command(&road(&f), "k-1", None).unwrap();
        assert_eq!(parse_reset_credit(&run(&script)), ResetSpend::Unanswered);
        assert!(!methods(&f).contains(&"account/rateLimits/read".to_string()));
    }

    #[test]
    fn refuses_a_credit_id_or_a_key_that_is_not_one_plain_word_before_it_reaches_the_sh() {
        let err = reset_credit_command(&bare_road("/root/.codex"), "k-1", Some("rc'1")).unwrap_err();
        assert!(err.contains("plain slug"), "{err}");
        let err = reset_credit_command(&bare_road("/root/.codex"), "k 1", None).unwrap_err();
        assert!(err.contains("plain slug"), "{err}");
    }

    #[test]
    fn kills_only_the_pid_the_shell_recorded_for_the_server_and_waits_on_no_clock() {
        let scripts = [reset_read_command(&bare_road("/root/.codex")).unwrap(), reset_credit_command(&bare_road("/root/.codex"), "k-1", None).unwrap()];
        let banned = Regex::new(r#"pkill|kill[a]ll|kill -|kill "?0|\bsleep\b|\bcoproc\b"#).unwrap();
        for script in scripts {
            assert!(script.contains("kill \"$WSP_APP_SERVER_PID\""), "{script}");
            assert!(!banned.is_match(&script), "{script}");
        }
    }

    #[test]
    fn is_the_agent_s_one_registered_set_of_reset_roads() {
        let road = bare_road("/root/.codex");
        assert_eq!(CodexPlanResets.read_command(&road), reset_read_command(&road).unwrap());
        assert_eq!(CodexPlanResets.spend_command(&road, "k-1", Some("rc_1")), reset_credit_command(&road, "k-1", Some("rc_1")).unwrap());
        assert_eq!(CodexPlanResets.too_old(), TOO_OLD);
        assert!(CodexPlanResets.too_old().contains("older than 0.141"));
    }
}

mod credits_of {
    use super::*;

    fn credit(overrides: Value) -> Value {
        let mut base = json!({ "id": "rc_1", "resetType": "codexRateLimits", "status": "available", "grantedAt": 1_790_000_000u64, "expiresAt": 1_792_000_000u64, "title": null, "description": null });
        for (k, v) in overrides.as_object().unwrap() {
            base[k.as_str()] = v.clone();
        }
        base
    }

    fn read(answer: Value) -> Option<BankedResets> {
        credits_of(answer.as_object())
    }

    fn banked(id: &str, status: ResetCreditStatus, expires_at: Option<f64>) -> ResetCredit {
        ResetCredit { id: id.to_string(), status, expires_at: expires_at.map(JsNumber) }
    }

    #[test]
    fn reads_nothing_where_the_answer_has_no_reset_field_as_a_codex_older_than_resets_a() {
        assert_eq!(read(json!({ "rateLimits": {} })), None);
        assert_eq!(read(json!({ "rateLimitResetCredits": null })), None);
    }

    #[test]
    fn reads_the_count_alone_where_the_details_are_null_or_not_a_list() {
        assert_eq!(read(json!({ "rateLimitResetCredits": { "availableCount": 2, "credits": null } })), Some(BankedResets { count: 2, credits: None }));
        assert_eq!(read(json!({ "rateLimitResetCredits": { "availableCount": 2, "credits": "two" } })), Some(BankedResets { count: 2, credits: None }));
    }

    #[test]
    fn reads_each_credit_s_id_status_and_expiry_seconds_made_ms_and_an_empty_list_as_de() {
        assert_eq!(
            read(json!({ "rateLimitResetCredits": { "availableCount": 2, "credits": [credit(json!({})), credit(json!({ "id": "rc_2", "expiresAt": null }))] } })),
            Some(BankedResets {
                count: 2,
                credits: Some(vec![banked("rc_1", ResetCreditStatus::Available, Some(1_792_000_000_000.0)), banked("rc_2", ResetCreditStatus::Available, None)]),
            })
        );
        assert_eq!(read(json!({ "rateLimitResetCredits": { "availableCount": 0, "credits": [] } })), Some(BankedResets { count: 0, credits: Some(vec![]) }));
    }

    #[test]
    fn fails_closed_a_count_that_is_not_a_whole_number_at_or_above_zero_reads_the_whole() {
        // The TS list also holds NaN; JSON cannot carry it, so it cannot reach this reader and is left out.
        for available_count in [json!(1.5), json!(-1), json!("2"), Value::Null] {
            let answer = json!({ "rateLimitResetCredits": { "availableCount": available_count, "credits": [credit(json!({}))] } });
            assert_eq!(read(answer), None, "{available_count}");
        }
    }

    #[test]
    fn never_reads_a_credit_of_another_kind_or_one_in_a_state_it_does_not_know_as_avail() {
        let mut no_id = credit(json!({}));
        no_id.as_object_mut().unwrap().remove("id");
        let answer = json!({ "rateLimitResetCredits": { "availableCount": 3, "credits": [
            credit(json!({ "resetType": "unknown" })),
            credit(json!({ "id": "rc_2", "status": "pending" })),
            no_id,
            credit(json!({ "id": "rc_4", "status": "redeeming" })),
        ] } });
        let got = read(answer).expect("a reading");
        let pairs: Vec<(String, ResetCreditStatus)> = got.credits.expect("details").into_iter().map(|c| (c.id, c.status)).collect();
        assert_eq!(
            pairs,
            [
                ("rc_1".to_string(), ResetCreditStatus::Unknown),
                ("rc_2".to_string(), ResetCreditStatus::Unknown),
                ("rc_4".to_string(), ResetCreditStatus::Redeeming),
            ]
        );
    }
}
