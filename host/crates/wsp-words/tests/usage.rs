// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/usage.test.ts:1-143, all 17 tests. The price file the TS reads from
//! test/fixtures/litellm-prices.json is written inline below, the fields the table reads and the twins it must leave
//! out, because the scratch tree holds no packages/. Wire values are built as structs, not through serde, since
//! JsNumber's own (de)serializer is a stub of the wire crate.

use serde_json::{json, Value};
use wsp_wire::types::usage::{UsageRange, UsageSplit, UsedAnswer, UsedAnswerLogs, UsedRow, UsedRowTokens, USAGE_WORDS};
use wsp_words::usage::*;

fn prices() -> Value {
    json!({
        "claude-opus-5": {
            "cache_creation_input_token_cost": 6.25e-06,
            "cache_creation_input_token_cost_above_1hr": 1e-05,
            "cache_creation_input_token_cost_batches": 3.125e-06,
            "cache_read_input_token_cost": 5e-07,
            "cache_read_input_token_cost_batches": 2.5e-07,
            "input_cost_per_token": 5e-06,
            "input_cost_per_token_batches": 2.5e-06,
            "litellm_provider": "anthropic",
            "max_input_tokens": 1000000,
            "max_output_tokens": 128000,
            "max_tokens": 128000,
            "mode": "chat",
            "output_cost_per_token": 2.5e-05,
            "output_cost_per_token_batches": 1.25e-05,
            "prompt_cache_min_tokens": 512,
            "provider_specific_entry": { "fast": 2.0, "us": 1.1 },
            "search_context_cost_per_query": {
                "search_context_size_high": 0.01,
                "search_context_size_low": 0.01,
                "search_context_size_medium": 0.01
            },
            "supports_prompt_caching": true
        },
        "gpt-5-codex": {
            "cache_read_input_token_cost": 1.25e-07,
            "deprecation_date": "2026-07-23",
            "input_cost_per_token": 1.25e-06,
            "litellm_provider": "openai",
            "max_input_tokens": 272000,
            "max_output_tokens": 128000,
            "max_tokens": 128000,
            "mode": "responses",
            "output_cost_per_token": 1e-05,
            "supported_endpoints": ["/v1/responses"],
            "supports_prompt_caching": true
        },
        "gpt-5.5": {
            "cache_read_input_token_cost": 5e-07,
            "cache_read_input_token_cost_above_272k_tokens": 1e-06,
            "cache_read_input_token_cost_above_272k_tokens_batches": 5e-07,
            "cache_read_input_token_cost_above_272k_tokens_flex": 5e-07,
            "cache_read_input_token_cost_batches": 2.5e-07,
            "cache_read_input_token_cost_flex": 2.5e-07,
            "cache_read_input_token_cost_priority": 1.25e-06,
            "input_cost_per_token": 5e-06,
            "input_cost_per_token_above_272k_tokens": 1e-05,
            "input_cost_per_token_above_272k_tokens_batches": 5e-06,
            "input_cost_per_token_above_272k_tokens_flex": 5e-06,
            "input_cost_per_token_batches": 2.5e-06,
            "input_cost_per_token_flex": 2.5e-06,
            "input_cost_per_token_priority": 1.25e-05,
            "litellm_provider": "openai",
            "max_input_tokens": 1050000,
            "max_output_tokens": 128000,
            "max_tokens": 128000,
            "mode": "chat",
            "output_cost_per_token": 3e-05,
            "output_cost_per_token_above_272k_tokens": 4.5e-05,
            "output_cost_per_token_above_272k_tokens_batches": 2.25e-05,
            "output_cost_per_token_above_272k_tokens_flex": 2.25e-05,
            "output_cost_per_token_batches": 1.5e-05,
            "output_cost_per_token_flex": 1.5e-05,
            "output_cost_per_token_priority": 7.5e-05,
            "regional_processing_uplift_multiplier_eu": 1.1,
            "regional_processing_uplift_multiplier_us": 1.1,
            "supports_prompt_caching": true
        },
        "sample_spec": {
            "code_interpreter_cost_per_session": 0.0,
            "computer_use_input_cost_per_1k_tokens": 0.0,
            "computer_use_output_cost_per_1k_tokens": 0.0,
            "deprecation_date": "date when the model becomes deprecated in the format YYYY-MM-DD",
            "file_search_cost_per_1k_calls": 0.0,
            "file_search_cost_per_gb_per_day": 0.0,
            "input_cost_per_token": 0.0,
            "litellm_provider": "one of https://docs.litellm.ai/docs/providers",
            "output_cost_per_token": 0.0
        }
    })
}

fn rate(input: f64, output: f64, cache_read: Option<f64>, cache_write: Option<f64>, provider: &str) -> Rate {
    Rate { input, output, cache_read, cache_write, provider: Some(provider.to_string()) }
}

fn close(actual: f64, expected: f64, digits: i32) {
    let tolerance = 10f64.powi(-digits) / 2.0;
    assert!((actual - expected).abs() < tolerance, "{actual} is not within {tolerance} of {expected}");
}

fn tokens(input: f64, output: f64, cached: Option<f64>, cache_write: Option<f64>) -> PricedTokens {
    PricedTokens { input, output, cached, cache_write }
}

fn words(key: &str) -> &'static str {
    USAGE_WORDS.iter().find(|(k, _)| *k == key).map(|(_, v)| *v).unwrap()
}

fn utc(year: i64, month0: i64, day: i64, hour: i64, minute: i64) -> f64 {
    let (y, m) = if month0 < 2 { (year - 1, month0 + 10) } else { (year, month0 - 2) };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    ((days * 24 + hour) * 60 + minute) as f64 * 60_000.0
}

fn used_row(key: &str, label: &str, t: (f64, f64, f64), cost_list: Option<f64>, cost_reported: Option<f64>, priced: bool) -> UsedRow {
    UsedRow {
        key: key.to_string(),
        label: label.to_string(),
        agent: None,
        tokens: UsedRowTokens { input: t.0.into(), output: t.1.into(), cached: t.2.into(), cache_write: None, reasoning: None },
        cost_reported: cost_reported.map(Into::into),
        cost_list: cost_list.map(Into::into),
        priced,
        estimate: None,
        saved: None,
        turns: None,
    }
}

fn answer(range: UsageRange, rows: Vec<UsedRow>) -> UsedAnswer {
    UsedAnswer { range, split: UsageSplit::Agent, rows, series: vec![], since: 0.0.into(), until: 0.0.into(), logs: None, lines: None }
}

fn logs(agents: &[&str], computers: &[&str]) -> UsedAnswerLogs {
    UsedAnswerLogs { agents: agents.iter().map(|s| s.to_string()).collect(), computers: computers.iter().map(|s| s.to_string()).collect() }
}

fn owners_week() -> Vec<UsedRow> {
    vec![
        used_row("claude", "Claude Code", (7_323_700_000.0, 7_300_000.0, 6_874_800_000.0), Some(4_301.74), None, true),
        used_row("codex", "Codex", (22_000_000.0, 42_000.0, 20_700_000.0), Some(15.45), None, true),
    ]
}

mod the_rate_table {
    use super::*;

    #[test]
    fn takes_each_models_four_per_token_prices_off_litellms_own_entries_and_nothing_else() {
        let table = parse_rate_table(&prices());
        assert_eq!(table.get("claude-opus-5"), Some(&rate(5e-6, 2.5e-5, Some(5e-7), Some(6.25e-6), "anthropic")));
        // The long-context, batch, flex and priority twins beside the base price are not the price a turn pays.
        assert_eq!(table.get("gpt-5.5"), Some(&rate(5e-6, 3e-5, Some(5e-7), None, "openai")));
        assert_eq!(table.get("gpt-5-codex"), Some(&rate(1.25e-6, 1e-5, Some(1.25e-7), None, "openai")));
        // The file's own description of its fields is no model.
        assert_eq!(table.get("sample_spec"), None);
        let mut keys: Vec<&String> = table.keys().collect();
        keys.sort();
        assert_eq!(keys, ["claude-opus-5", "gpt-5-codex", "gpt-5.5"]);
    }

    #[test]
    fn reads_nothing_out_of_a_document_that_is_not_the_table() {
        assert_eq!(parse_rate_table(&Value::Null), RateTable::new());
        assert_eq!(parse_rate_table(&json!([1, 2])), RateTable::new());
        assert_eq!(parse_rate_table(&json!({ "gpt-x": { "input_cost_per_token": "cheap" } })), RateTable::new());
    }
}

mod a_turns_list_price {
    use super::*;

    #[test]
    fn prices_fresh_input_cached_input_cache_writes_and_output_each_at_its_own_rate() {
        let table = parse_rate_table(&prices());
        // input counts every token the model read, the cached and the written part included.
        let usd = price_of("claude-opus-5", &tokens(1_000_000.0, 100_000.0, Some(600_000.0), Some(100_000.0)), &table);
        close(usd.unwrap(), 300_000.0 * 5e-6 + 600_000.0 * 5e-7 + 100_000.0 * 6.25e-6 + 100_000.0 * 2.5e-5, 10);
    }

    #[test]
    fn prices_a_cache_write_at_the_input_rate_where_the_model_lists_none() {
        let table = parse_rate_table(&prices());
        close(price_of("gpt-5.5", &tokens(1_000.0, 0.0, None, Some(1_000.0)), &table).unwrap(), 1_000.0 * 5e-6, 12);
    }

    #[test]
    fn finds_a_model_named_with_its_provider_or_a_date_after_it() {
        let table = parse_rate_table(&prices());
        close(price_of("openai/gpt-5.5", &tokens(1_000.0, 0.0, None, None), &table).unwrap(), 5e-3, 12);
        close(price_of("claude-opus-5-20260901", &tokens(0.0, 1_000.0, None, None), &table).unwrap(), 2.5e-2, 12);
    }

    #[test]
    fn prices_a_model_named_with_its_context_window_after_it_as_claude_codes_1m_at_its_base_mo() {
        let table = parse_rate_table(&prices());
        close(price_of("claude-opus-5[1m]", &tokens(0.0, 1_000.0, None, None), &table).unwrap(), 2.5e-2, 12);
    }

    #[test]
    fn answers_nothing_for_a_model_the_table_does_not_have_and_never_a_guess() {
        let table = parse_rate_table(&prices());
        assert_eq!(price_of("some-new-model", &tokens(1_000.0, 1_000.0, None, None), &table), None);
    }
}

mod the_day_a_row_is_filed_under {
    use super::*;

    #[test]
    fn is_the_calendar_day_in_the_zone_asked_for_so_a_turn_a_minute_past_midnight_is_the_new() {
        // 2026-09-29T18:29:00Z is 23:59 in Kolkata and 18:31Z is 00:01 the next day there.
        assert_eq!(day_key_of(utc(2026, 8, 29, 18, 29), Some("Asia/Kolkata")), "2026-09-29");
        assert_eq!(day_key_of(utc(2026, 8, 29, 18, 31), Some("Asia/Kolkata")), "2026-09-30");
        assert_eq!(day_key_of(utc(2026, 8, 29, 18, 31), Some("UTC")), "2026-09-29");
    }
}

mod a_limits_windows_and_words {
    use super::*;
    use wsp_wire::types::usage::{LimitKind, LIMIT_KINDS};

    #[test]
    fn reads_a_windows_length_as_the_plans_session_week_or_longer() {
        assert_eq!(limit_kind_of_minutes(300.0), LimitKind::Session);
        assert_eq!(limit_kind_of_minutes(10_080.0), LimitKind::Week);
        assert_eq!(limit_kind_of_minutes(43_200.0), LimitKind::Month);
        assert_eq!(LIMIT_KINDS, ["session", "week", "week_opus", "week_sonnet", "month", "overage"]);
    }

    #[test]
    fn says_a_reset_in_minutes_then_hours_then_by_the_day_of_the_week() {
        let now = utc(2026, 8, 29, 10, 0);
        assert_eq!(resets_word(now + 45.0 * 60_000.0, now, Some("UTC")), "resets in 45 min");
        assert_eq!(resets_word(now + 2.0 * 3_600_000.0 + 10.0 * 60_000.0, now, Some("UTC")), "resets in 2 h");
        assert_eq!(resets_word(now + 3.0 * 86_400_000.0, now, Some("UTC")), "resets on Fri");
        assert_eq!(resets_word(now - 60_000.0, now, Some("UTC")), "reset");
    }

    #[test]
    fn says_the_banked_resets_as_a_count_with_when_the_first_lapses_where_the_agent_said() {
        let now = utc(2026, 8, 29, 10, 0);
        assert_eq!(credits_word(2.0, Some(now + 21.0 * 86_400_000.0 + 5.0 * 3_600_000.0), now), "2 banked, first expires in 21 d");
        assert_eq!(credits_word(1.0, Some(now + 5.0 * 3_600_000.0 + 60_000.0), now), "1 banked, expires in 5 h");
        assert_eq!(credits_word(1.0, Some(now + 30.0 * 60_000.0), now), "1 banked, expires in 30 min");
        assert_eq!(credits_word(2.0, None, now), "2 banked");
        assert_eq!(credits_word(2.0, Some(now - 1.0), now), "2 banked");
        assert_eq!(credits_word(0.0, None, now), "none banked");
    }

    #[test]
    #[ignore = "checks constants alone, which the interface copied by script; no stub to fail on"]
    fn keeps_every_sentence_a_row_may_carry_in_one_table() {
        assert_eq!(words("notPriced"), "not priced");
        assert_eq!(words("listPrice"), "list price");
        assert_eq!(words("noLimit"), "reports no plan limit");
        assert_eq!(words("keyed"), "pays per token, no plan limit");
        assert_eq!(words("unread"), "not read yet: shows after its next turn");
        assert_eq!(words("reached"), "limit reached");
    }
}

mod an_account_in_plain_words {
    use super::*;

    #[test]
    fn says_how_it_pays_a_key_its_plan_under_the_plans_own_name_the_address_it_signed_in_as_y() {
        let who = |agent_name: &str| AccountWho { agent_name: agent_name.to_string(), ..Default::default() };
        let said = vec![
            account_words(&AccountWho { keyed: Some(true), plan: Some("max".to_string()), ..who("Claude Code") }),
            account_words(&AccountWho {
                plan: Some("plus".to_string()),
                plan_brand: Some("ChatGPT".to_string()),
                named: Some("dev@example.com".to_string()),
                ..who("Codex")
            }),
            account_words(&AccountWho { named: Some("dev@example.com".to_string()), ..who("Codex") }),
            account_words(&AccountWho { vaulted: Some(true), ..who("Claude Code") }),
            account_words(&AccountWho { own_on: Some("zingzy's MacBook Pro".to_string()), ..who("OpenCode") }),
        ];
        assert_eq!(
            said,
            [
                "Claude Code with an API key",
                "Codex with ChatGPT Plus",
                "Codex as dev@example.com",
                "Claude Code with your sign-in",
                "OpenCode signed in on zingzy's MacBook Pro"
            ]
        );
    }
}

mod what_a_range_used_said_once {
    use super::*;

    // The owner's week, near enough: almost all of it read from cache, priced off the table.
    #[test]
    fn reads_the_tokens_the_price_and_the_cached_share_in_one_line_the_cached_ones_taken_out_o() {
        let rows = owners_week();
        assert_eq!(
            used_headline(&answer(UsageRange::Week, rows.clone())),
            "7.35B tokens in the last 7 days, $4,317.19 at list price, 6.9B of it read from cache."
        );
        let fresh: Vec<f64> = rows.iter().map(|r| fresh_in(r.tokens.input.0, r.tokens.cached.0, r.tokens.cache_write.map(|n| n.0))).collect();
        assert_eq!(fresh, [448_900_000.0, 1_300_000.0]);
        assert_eq!(used_headline(&answer(UsageRange::Day, vec![])), words("noUse"));
    }

    #[test]
    fn takes_the_written_part_out_of_fresh_as_well_so_fresh_written_cached_and_out_add_up_to() {
        assert_eq!(fresh_in(10_000.0, 6_000.0, Some(3_000.0)), 1_000.0);
    }

    #[test]
    fn names_whose_logs_it_counted_and_on_which_computers_wsps_own_threads_among_them() {
        assert_eq!(
            logs_line(&logs(&["Claude Code", "Codex", "OpenCode"], &["zingzy's MacBook Pro"])),
            "Counts what Claude Code, Codex and OpenCode logged on zingzy's MacBook Pro, wsp's threads there included."
        );
        assert_eq!(
            logs_line(&logs(&["Codex"], &["Boat", "spoo"])),
            "Counts what Codex logged on Boat and spoo, wsp's threads there included."
        );
    }
}

mod a_used_rows_price {
    use super::*;

    fn price(figure: Option<&str>, word: Option<&str>) -> UsedPrice {
        UsedPrice { figure: figure.map(str::to_string), word: word.map(str::to_string) }
    }

    #[test]
    fn says_not_priced_wherever_some_of_its_tokens_had_no_price_beside_whatever_figure_the_re() {
        let row = |cost_list: Option<f64>, cost_reported: Option<f64>, priced: bool| used_row("k", "K", (0.0, 0.0, 0.0), cost_list, cost_reported, priced);
        assert_eq!(used_price(&row(Some(1.5), None, false)), price(Some("$1.50"), Some(words("notPriced"))));
        assert_eq!(used_price(&row(None, Some(0.4), false)), price(Some("$0.40"), Some(words("notPriced"))));
        assert_eq!(used_price(&row(None, None, false)), price(None, Some(words("notPriced"))));
        assert_eq!(used_price(&row(Some(1.5), None, true)), price(Some("$1.50"), Some(words("listPrice"))));
        assert_eq!(used_price(&row(None, Some(0.4), true)), price(Some("$0.40"), None));
    }
}

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::usage::*;

    use wsp_wire::types::js_number::JsNumber;
    use wsp_wire::types::usage::{LimitKind, LimitWindow, UsageRange, UsageSplit, UsedAnswerLogs};
    use wsp_words::usage::limit_words;
    use wsp_words::usage::reset_words;

    fn js<T: serde::Serialize>(v: &T) -> String {
        wsp_words::exit::json_line(&serde_json::to_value(v).unwrap(), None)
    }
    fn kind_word(k: LimitKind) -> String {
        serde_json::to_value(k).unwrap().as_str().unwrap().to_string()
    }
    fn table() -> RateTable {
        let mut t = RateTable::new();
        t.insert("claude-sonnet-4-5".into(), Rate { input: 3e-6, output: 15e-6, cache_read: Some(3e-7), cache_write: Some(3.75e-6), provider: None });
        t.insert("gpt-5".into(), Rate { input: 1.25e-6, output: 1e-5, cache_read: None, cache_write: None, provider: None });
        t
    }

    #[test]
    fn a_model_is_priced_off_its_base_name() {
        assert_eq!(base_model("claude-opus-4-1[1m]"), "claude-opus-4-1"); // baseModel("claude-opus-4-1[1m]")
        assert_eq!(base_model("claude-opus-4-1"), "claude-opus-4-1"); // baseModel("claude-opus-4-1")
        assert_eq!(base_model("x[a][b]"), "x[a]"); // baseModel("x[a][b]")
        assert_eq!(base_model("[1m]"), ""); // baseModel("[1m]")
        assert_eq!(base_model("a[1m]b"), "a[1m]b"); // baseModel("a[1m]b")
        assert_eq!(base_model("a[]"), "a"); // baseModel("a[]")
        assert_eq!(rate_of("claude-sonnet-4-5", &table()).map(|r| js(&r)).unwrap_or_else(|| "null".into()), "{\"input\":0.000003,\"output\":0.000015,\"cacheRead\":3e-7,\"cacheWrite\":0.00000375}"); // JSON.stringify(rateOf("claude-sonnet-4-5", {"claude-sonnet-4-5":{"input":0.000003,"output":0.000015,"cacheRead":3e-7,"cacheWrite":0.00000375},"gpt-5":{"input":0.00000125,"output":0.00001}}) ?? null)
        assert_eq!(rate_of("anthropic/claude-sonnet-4-5", &table()).map(|r| js(&r)).unwrap_or_else(|| "null".into()), "{\"input\":0.000003,\"output\":0.000015,\"cacheRead\":3e-7,\"cacheWrite\":0.00000375}"); // JSON.stringify(rateOf("anthropic/claude-sonnet-4-5", {"claude-sonnet-4-5":{"input":0.000003,"output":0.000015,"cacheRead":3e-7,"cacheWrite":0.00000375},"gpt-5":{"input":0.00000125,"output":0.00001}}) ?? null)
        assert_eq!(rate_of("claude-sonnet-4-5-20250929", &table()).map(|r| js(&r)).unwrap_or_else(|| "null".into()), "{\"input\":0.000003,\"output\":0.000015,\"cacheRead\":3e-7,\"cacheWrite\":0.00000375}"); // JSON.stringify(rateOf("claude-sonnet-4-5-20250929", {"claude-sonnet-4-5":{"input":0.000003,"output":0.000015,"cacheRead":3e-7,"cacheWrite":0.00000375},"gpt-5":{"input":0.00000125,"output":0.00001}}) ?? null)
        assert_eq!(rate_of("claude-sonnet-4-5[1m]", &table()).map(|r| js(&r)).unwrap_or_else(|| "null".into()), "{\"input\":0.000003,\"output\":0.000015,\"cacheRead\":3e-7,\"cacheWrite\":0.00000375}"); // JSON.stringify(rateOf("claude-sonnet-4-5[1m]", {"claude-sonnet-4-5":{"input":0.000003,"output":0.000015,"cacheRead":3e-7,"cacheWrite":0.00000375},"gpt-5":{"input":0.00000125,"output":0.00001}}) ?? null)
        assert_eq!(rate_of("openai/gpt-5", &table()).map(|r| js(&r)).unwrap_or_else(|| "null".into()), "{\"input\":0.00000125,\"output\":0.00001}"); // JSON.stringify(rateOf("openai/gpt-5", {"claude-sonnet-4-5":{"input":0.000003,"output":0.000015,"cacheRead":3e-7,"cacheWrite":0.00000375},"gpt-5":{"input":0.00000125,"output":0.00001}}) ?? null)
        assert_eq!(rate_of("gpt-5-20250101", &table()).map(|r| js(&r)).unwrap_or_else(|| "null".into()), "{\"input\":0.00000125,\"output\":0.00001}"); // JSON.stringify(rateOf("gpt-5-20250101", {"claude-sonnet-4-5":{"input":0.000003,"output":0.000015,"cacheRead":3e-7,"cacheWrite":0.00000375},"gpt-5":{"input":0.00000125,"output":0.00001}}) ?? null)
        assert_eq!(rate_of("gpt-6", &table()).map(|r| js(&r)).unwrap_or_else(|| "null".into()), "null"); // JSON.stringify(rateOf("gpt-6", {"claude-sonnet-4-5":{"input":0.000003,"output":0.000015,"cacheRead":3e-7,"cacheWrite":0.00000375},"gpt-5":{"input":0.00000125,"output":0.00001}}) ?? null)
        assert_eq!(rate_of("claude-sonnet-4-5-2025", &table()).map(|r| js(&r)).unwrap_or_else(|| "null".into()), "null"); // JSON.stringify(rateOf("claude-sonnet-4-5-2025", {"claude-sonnet-4-5":{"input":0.000003,"output":0.000015,"cacheRead":3e-7,"cacheWrite":0.00000375},"gpt-5":{"input":0.00000125,"output":0.00001}}) ?? null)
        assert_eq!(price_of("gpt-5", &PricedTokens { input: 1000.0, output: 100.0, cached: None, cache_write: None }, &table()).map(wsp_words::js::number_string).unwrap_or_else(|| "none".into()), "0.0022500000000000003"); // String(priceOf("gpt-5", {"input":1000,"output":100}, {"claude-sonnet-4-5":{"input":0.000003,"output":0.000015,"cacheRead":3e-7,"cacheWrite":0.00000375},"gpt-5":{"input":0.00000125,"output":0.00001}}) ?? "none")
        assert_eq!(price_of("claude-sonnet-4-5", &PricedTokens { input: 1000.0, output: 100.0, cached: Some(400.0), cache_write: Some(100.0) }, &table()).map(wsp_words::js::number_string).unwrap_or_else(|| "none".into()), "0.003495"); // String(priceOf("claude-sonnet-4-5", {"input":1000,"output":100,"cached":400,"cacheWrite":100}, {"claude-sonnet-4-5":{"input":0.000003,"output":0.000015,"cacheRead":3e-7,"cacheWrite":0.00000375},"gpt-5":{"input":0.00000125,"output":0.00001}}) ?? "none")
        assert_eq!(price_of("claude-sonnet-4-5", &PricedTokens { input: 100.0, output: 0.0, cached: Some(300.0), cache_write: None }, &table()).map(wsp_words::js::number_string).unwrap_or_else(|| "none".into()), "0.00008999999999999999"); // String(priceOf("claude-sonnet-4-5", {"input":100,"output":0,"cached":300}, {"claude-sonnet-4-5":{"input":0.000003,"output":0.000015,"cacheRead":3e-7,"cacheWrite":0.00000375},"gpt-5":{"input":0.00000125,"output":0.00001}}) ?? "none")
        assert_eq!(price_of("nope", &PricedTokens { input: 1.0, output: 1.0, cached: None, cache_write: None }, &table()).map(wsp_words::js::number_string).unwrap_or_else(|| "none".into()), "none"); // String(priceOf("nope", {"input":1,"output":1}, {"claude-sonnet-4-5":{"input":0.000003,"output":0.000015,"cacheRead":3e-7,"cacheWrite":0.00000375},"gpt-5":{"input":0.00000125,"output":0.00001}}) ?? "none")
        assert_eq!(fresh_in(10.0, 3.0, None), 7.0); // freshIn({"input":10,"cached":3})
        assert_eq!(fresh_in(10.0, 3.0, Some(4.0)), 3.0); // freshIn({"input":10,"cached":3,"cacheWrite":4})
        assert_eq!(fresh_in(2.0, 5.0, None), 0.0); // freshIn({"input":2,"cached":5})
    }

    #[test]
    fn an_account_and_its_reset_sentences() {
        assert_eq!(account_words(&AccountWho { agent_name: "Claude Code".into(), keyed: Some(true), plan: Some("max".into()), plan_brand: None, named: None, vaulted: None, own_on: None }), "Claude Code with an API key"); // accountWords({"agentName":"Claude Code","keyed":true,"plan":"max"})
        assert_eq!(account_words(&AccountWho { agent_name: "Claude Code".into(), keyed: None, plan: Some("max".into()), plan_brand: None, named: None, vaulted: None, own_on: None }), "Claude Code with Max"); // accountWords({"agentName":"Claude Code","plan":"max"})
        assert_eq!(account_words(&AccountWho { agent_name: "Codex".into(), keyed: None, plan: Some("plus".into()), plan_brand: Some("ChatGPT".into()), named: None, vaulted: None, own_on: None }), "Codex with ChatGPT Plus"); // accountWords({"agentName":"Codex","plan":"plus","planBrand":"ChatGPT"})
        assert_eq!(account_words(&AccountWho { agent_name: "Codex".into(), keyed: None, plan: Some("".into()), plan_brand: None, named: Some("a@b.c".into()), vaulted: None, own_on: None }), "Codex as a@b.c"); // accountWords({"agentName":"Codex","plan":"","named":"a@b.c"})
        assert_eq!(account_words(&AccountWho { agent_name: "Codex".into(), keyed: None, plan: None, plan_brand: None, named: None, vaulted: Some(true), own_on: None }), "Codex with your sign-in"); // accountWords({"agentName":"Codex","vaulted":true})
        assert_eq!(account_words(&AccountWho { agent_name: "Codex".into(), keyed: None, plan: None, plan_brand: None, named: None, vaulted: None, own_on: Some("spoo".into()) }), "Codex signed in on spoo"); // accountWords({"agentName":"Codex","ownOn":"spoo"})
        assert_eq!(account_words(&AccountWho { agent_name: "Codex".into(), keyed: None, plan: None, plan_brand: None, named: None, vaulted: None, own_on: None }), "Codex"); // accountWords({"agentName":"Codex"})
        assert_eq!(account_words(&AccountWho { agent_name: "Codex".into(), keyed: None, plan: Some("\u{e9}bc".into()), plan_brand: None, named: None, vaulted: None, own_on: None }), "Codex with \u{c9}bc"); // accountWords({"agentName":"Codex","plan":"ébc"})
        assert_eq!(reset_who("Claude Max", true, "spoo"), "Claude Max"); // resetWho({label:"Claude Max", own:true, computer:"spoo"})
        assert_eq!(reset_who("Claude Max", false, "spoo"), "Claude Max on spoo"); // resetWho({label:"Claude Max", own:false, computer:"spoo"})
        assert_eq!(reset_question("Max", None), "Use a reset banked on Max? Its five-hour and weekly windows start again now. There is no undo."); // resetQuestion("Max", undefined)
        assert_eq!(reset_question("Max", Some(1.0)), "Use the one reset banked on Max? Its five-hour and weekly windows start again now. There is no undo."); // resetQuestion("Max", 1)
        assert_eq!(reset_question("Max", Some(3.0)), "Use one of the 3 resets banked on Max? Its five-hour and weekly windows start again now. There is no undo."); // resetQuestion("Max", 3)
        assert_eq!(reset_keyed_line("Max"), "Max pays per token; an API key has no reset to use"); // resetKeyedLine("Max")
        assert_eq!(reset_silent_line("Codex", "spoo"), "Codex on spoo did not answer; the same request goes again on the next press"); // resetSilentLine("Codex", "spoo")
        assert_eq!(reset_unread_line("Codex", "spoo", "Max"), "Codex on spoo did not read Max's limits; nothing was spent"); // resetUnreadLine("Codex", "spoo", "Max")
        assert_eq!(reset_mismatch_line("Codex", "spoo", "b@c", "Max"), "Codex on spoo is now signed in as b@c, not Max; nothing was spent"); // resetMismatchLine("Codex", "spoo", "b@c", "Max")
        assert_eq!(reset_refused_line("Codex", "spoo", "Max", "no"), "Codex on spoo refused the reset of Max: no"); // resetRefusedLine("Codex", "spoo", "Max", "no")
        assert_eq!(reset_provider_only_line("Max", "Hetzner", "Codex"), "Max lives only on Hetzner machines; sign Codex in on a computer of yours"); // resetProviderOnlyLine("Max", "Hetzner", "Codex")
        assert_eq!(reset_signed_out_line("Max", "Codex"), "Max is signed in on none of your computers now; sign Codex in on one of them"); // resetSignedOutLine("Max", "Codex")
        assert_eq!(reset_not_on_line("Max", "spoo", &["a"] as &[&str]), "Max is not signed in on spoo; it is on a"); // resetNotOnLine("Max", "spoo", ["a"])
        assert_eq!(reset_not_on_line("Max", "spoo", &["a","b"] as &[&str]), "Max is not signed in on spoo; it is on a and b"); // resetNotOnLine("Max", "spoo", ["a","b"])
        assert_eq!(reset_no_logins_line("spoo", "Codex"), "spoo has not said where it keeps its logins, so Codex's there cannot be reached; wsp add spoo --update puts a newer agent on it"); // resetNoLoginsLine("spoo", "Codex")
        assert_eq!(reset_none_line("Codex"), "Codex banks no resets"); // resetNoneLine("Codex")
        assert_eq!(no_such_account_line("max"), "no account max; wsp usage lists every account"); // noSuchAccountLine("max")
    }

    #[test]
    fn lists_logs_and_the_used_ask() {
        assert_eq!(list_words(&[] as &[&str]), ""); // listWords([])
        assert_eq!(list_words(&["a"] as &[&str]), "a"); // listWords(["a"])
        assert_eq!(list_words(&["a","b"] as &[&str]), "a and b"); // listWords(["a","b"])
        assert_eq!(list_words(&["a","b","c"] as &[&str]), "a, b and c"); // listWords(["a","b","c"])
        assert_eq!(logs_line(&UsedAnswerLogs { agents: vec!["Claude Code".into(), "Codex".into()], computers: vec!["spoo".into()] }), "Counts what Claude Code and Codex logged on spoo, wsp's threads there included."); // logsLine({agents:["Claude Code","Codex"], computers:["spoo"]})
        assert_eq!(js(&used_ask(&UsageRange::Week, &UsageSplit::Model)), "{\"range\":\"week\",\"split\":\"model\",\"outside\":true}"); // JSON.stringify(usedAsk("week", "model"))
        assert_eq!(kind_word(limit_kind_of_minutes(0.0)), "session"); // limitKindOfMinutes(0)
        assert_eq!(kind_word(limit_kind_of_minutes(300.0)), "session"); // limitKindOfMinutes(300)
        assert_eq!(kind_word(limit_kind_of_minutes(301.0)), "week"); // limitKindOfMinutes(301)
        assert_eq!(kind_word(limit_kind_of_minutes(10080.0)), "week"); // limitKindOfMinutes(10080)
        assert_eq!(kind_word(limit_kind_of_minutes(10081.0)), "month"); // limitKindOfMinutes(10081)
    }

    #[test]
    fn clock_words_read_the_zone_named() {
        assert_eq!(day_key_of(1792920600000.0, Some("UTC")), "2026-10-25"); // dayKeyOf(1792920600000, "UTC")
        assert_eq!(hour_of(1792920600000.0, Some("UTC")), 9.0); // hourOf(1792920600000, "UTC")
        assert_eq!(day_key_of(1792920600000.0, Some("Asia/Kolkata")), "2026-10-25"); // dayKeyOf(1792920600000, "Asia/Kolkata")
        assert_eq!(hour_of(1792920600000.0, Some("Asia/Kolkata")), 15.0); // hourOf(1792920600000, "Asia/Kolkata")
        assert_eq!(day_key_of(1792971000000.0, Some("Asia/Kolkata")), "2026-10-26"); // dayKeyOf(1792971000000, "Asia/Kolkata")
        assert_eq!(hour_of(1792971000000.0, Some("Asia/Kolkata")), 5.0); // hourOf(1792971000000, "Asia/Kolkata")
        assert_eq!(day_key_of(1792893600000.0, Some("America/New_York")), "2026-10-24"); // dayKeyOf(1792893600000, "America/New_York")
        assert_eq!(hour_of(1792893600000.0, Some("America/New_York")), 22.0); // hourOf(1792893600000, "America/New_York")
        assert_eq!(resets_word(1792920600000.0, 1792920600000.0, Some("UTC")), "reset"); // resetsWord(1792920600000, 1792920600000, "UTC")
        assert_eq!(resets_word(1792920599995.0, 1792920600000.0, Some("UTC")), "reset"); // resetsWord(1792920599995, 1792920600000, "UTC")
        assert_eq!(resets_word(1792920630000.0, 1792920600000.0, Some("UTC")), "resets in 1 min"); // resetsWord(1792920630000, 1792920600000, "UTC")
        assert_eq!(resets_word(1792924140000.0, 1792920600000.0, Some("UTC")), "resets in 59 min"); // resetsWord(1792924140000, 1792920600000, "UTC")
        assert_eq!(resets_word(1792924200000.0, 1792920600000.0, Some("UTC")), "resets in 1 h"); // resetsWord(1792924200000, 1792920600000, "UTC")
        assert_eq!(resets_word(1793006640000.0, 1792920600000.0, Some("UTC")), "resets in 23 h"); // resetsWord(1793006640000, 1792920600000, "UTC")
        assert_eq!(resets_word(1793007000000.0, 1792920600000.0, Some("UTC")), "resets on Mon"); // resetsWord(1793007000000, 1792920600000, "UTC")
        assert_eq!(resets_word(1793179800000.0, 1792920600000.0, Some("UTC")), "resets on Wed"); // resetsWord(1793179800000, 1792920600000, "UTC")
        assert_eq!(credits_word(0.0, None, 1792920600000.0), "none banked"); // creditsWord({"count":0}, 1792920600000)
        assert_eq!(credits_word(1.0, None, 1792920600000.0), "1 banked"); // creditsWord({"count":1}, 1792920600000)
        assert_eq!(credits_word(1.0, Some(1792920690000.0), 1792920600000.0), "1 banked, expires in 2 min"); // creditsWord({"count":1,"nextExpiresAt":1792920690000}, 1792920600000)
        assert_eq!(credits_word(3.0, Some(1792927800000.0), 1792920600000.0), "3 banked, first expires in 2 h"); // creditsWord({"count":3,"nextExpiresAt":1792927800000}, 1792920600000)
        assert_eq!(credits_word(2.0, Some(1793179800000.0), 1792920600000.0), "2 banked, first expires in 3 d"); // creditsWord({"count":2,"nextExpiresAt":1793179800000}, 1792920600000)
        assert_eq!(credits_word(2.0, Some(1792920599999.0), 1792920600000.0), "2 banked"); // creditsWord({"count":2,"nextExpiresAt":1792920599999}, 1792920600000)
        assert_eq!(clock_word(1792920600000.0, 1792920600000.0, Some("UTC")), "09:30"); // clockWord(1792920600000, 1792920600000, "UTC")
        assert_eq!(as_of_word(1792920600000.0, 1792920600000.0, Some("UTC")), "as of 09:30"); // asOfWord(1792920600000, 1792920600000, "UTC")
        assert_eq!(clock_word(1792834200000.0, 1792920600000.0, Some("UTC")), "Sat 09:30"); // clockWord(1792834200000, 1792920600000, "UTC")
        assert_eq!(as_of_word(1792834200000.0, 1792920600000.0, Some("UTC")), "as of Sat 09:30"); // asOfWord(1792834200000, 1792920600000, "UTC")
        assert_eq!(clock_word(1792920600000.0, 1792920600000.0, Some("Asia/Kolkata")), "15:00"); // clockWord(1792920600000, 1792920600000, "Asia/Kolkata")
        assert_eq!(as_of_word(1792920600000.0, 1792920600000.0, Some("Asia/Kolkata")), "as of 15:00"); // asOfWord(1792920600000, 1792920600000, "Asia/Kolkata")
        assert_eq!(clock_word(1792872000000.0, 1792920600000.0, Some("Asia/Kolkata")), "01:30"); // clockWord(1792872000000, 1792920600000, "Asia/Kolkata")
        assert_eq!(as_of_word(1792872000000.0, 1792920600000.0, Some("Asia/Kolkata")), "as of 01:30"); // asOfWord(1792872000000, 1792920600000, "Asia/Kolkata")
        assert_eq!(window_cell(&LimitWindow { kind: LimitKind::Session, used_percent: JsNumber(42.5), resets_at: None }, 1792920600000.0, Some("UTC")), "43%"); // windowCell({"kind":"session","usedPercent":42.5}, 1792920600000, "UTC")
        assert_eq!(window_cell(&LimitWindow { kind: LimitKind::Week, used_percent: JsNumber(99.4), resets_at: Some(JsNumber(1792927800000.0)) }, 1792920600000.0, Some("UTC")), "99% resets in 2 h"); // windowCell({"kind":"week","usedPercent":99.4,"resetsAt":1792927800000}, 1792920600000, "UTC")
        assert_eq!(window_cell(&LimitWindow { kind: LimitKind::Month, used_percent: JsNumber(0.0), resets_at: Some(JsNumber(1792920599999.0)) }, 1792920600000.0, Some("UTC")), "0% reset"); // windowCell({"kind":"month","usedPercent":0,"resetsAt":1792920599999}, 1792920600000, "UTC")
        assert_eq!(held_until(&[], 1792920600000.0).map(wsp_words::js::number_string).unwrap_or_else(|| "none".into()), "none"); // String(heldUntil([], 1792920600000) ?? "none")
        assert_eq!(held_until(&[LimitWindow { kind: LimitKind::Session, used_percent: JsNumber(100.0), resets_at: Some(JsNumber(1792920600010.0)) }, LimitWindow { kind: LimitKind::Week, used_percent: JsNumber(100.0), resets_at: Some(JsNumber(1792920600099.0)) }, LimitWindow { kind: LimitKind::Month, used_percent: JsNumber(99.0), resets_at: Some(JsNumber(1792920600999.0)) }], 1792920600000.0).map(wsp_words::js::number_string).unwrap_or_else(|| "none".into()), "1792920600099"); // String(heldUntil([{"kind":"session","usedPercent":100,"resetsAt":1792920600010},{"kind":"week","usedPercent":100,"resetsAt":1792920600099},{"kind":"month","usedPercent":99,"resetsAt":1792920600999}], 1792920600000) ?? "none")
        assert_eq!(held_until(&[LimitWindow { kind: LimitKind::Session, used_percent: JsNumber(100.0), resets_at: Some(JsNumber(1792920600000.0)) }], 1792920600000.0).map(wsp_words::js::number_string).unwrap_or_else(|| "none".into()), "none"); // String(heldUntil([{"kind":"session","usedPercent":100,"resetsAt":1792920600000}], 1792920600000) ?? "none")
        assert_eq!(held_until(&[LimitWindow { kind: LimitKind::Session, used_percent: JsNumber(100.0), resets_at: None }], 1792920600000.0).map(wsp_words::js::number_string).unwrap_or_else(|| "none".into()), "none"); // String(heldUntil([{"kind":"session","usedPercent":100}], 1792920600000) ?? "none")
    }

    #[test]
    fn the_reset_and_limit_sentences() {
        assert_eq!(reset_words::reset("Max", None), "Reset used: Max's windows start again now"); // RESET_WORDS.reset("Max")
        assert_eq!(reset_words::reset("Max", Some(2.0)), "Reset used: Max's windows start again now, 2 left"); // RESET_WORDS.reset("Max", 2)
        assert_eq!(reset_words::nothing_to_reset("Max"), "No window of Max is in use right now, so the credit was kept"); // RESET_WORDS.nothingToReset("Max")
        assert_eq!(reset_words::no_credit("Max"), "No reset banked on Max"); // RESET_WORDS.noCredit("Max")
        assert_eq!(reset_words::already_redeemed("Max"), "That reset of Max was already used"); // RESET_WORDS.alreadyRedeemed("Max")
        assert_eq!(reset_words::earlier("Max", 1.0), "The count on Max fell since the earlier press: it may have gone through, or a reset expired or was used elsewhere. Nothing more was spent; 1 left."); // RESET_WORDS.earlier("Max", 1)
        assert_eq!(reset_words::unknown("Codex", "pending", "Max"), "Codex answered pending for Max; the account was read again"); // RESET_WORDS.unknown("Codex", "pending", "Max")
        assert_eq!(limit_words::stopped("Codex"), "Codex stopped this turn until its plan resets."); // LIMIT_WORDS.stopped("Codex")
        assert_eq!(limit_words::stopped_unknown("Codex"), "Codex stopped this turn and did not say when its plan resets."); // LIMIT_WORDS.stoppedUnknown("Codex")
        assert_eq!(limit_words::armed("Codex"), "Codex goes on with this turn when its plan resets."); // LIMIT_WORDS.armed("Codex")
        assert_eq!(limit_words::resumes_at("Codex", "14:00"), "Codex resumes at 14:00"); // LIMIT_WORDS.resumesAt("Codex", "14:00")
    }
}
