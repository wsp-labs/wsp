// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the `nums` area (js, units, base, turn, usage, cost_history, plan_alerts, outside_line, general_prefs,
//! ps_time, semver, release, regexp, quote) against node 22.23.4 running the TS at a3b870a6e. Every expected value was
//! printed by node (see the commands in the review report); each test fails today for the reason in its name.

#![allow(clippy::excessive_precision)]

use serde_json::json;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::usage::{LimitKind, LimitWindow};
use wsp_wire::views::workspace_events::WorkspaceCostEvent;
use wsp_words::cost_history::{accrued_at, day_start, month_start, rate_at};
use wsp_words::js::{number_string, to_number};
use wsp_words::usage::{as_of_word, clock_word, day_key_of, hour_of, resets_word, window_cell};
use wsp_words::words::base::lower_first;
use wsp_words::words::turn::{fmt_clock, generated_title};
use wsp_words::words::units::{fmt_bytes_over, fmt_cost, fmt_inr};

fn utc() -> jiff::tz::TimeZone {
    jiff::tz::TimeZone::UTC
}

fn tick(at: &str, accrued: f64) -> WorkspaceCostEvent {
    serde_json::from_value(
        json!({ "workspaceId": "w", "phase": "running", "rateUsdPerHour": 1, "awakeMs": 0, "accruedUsd": accrued, "at": at }),
    )
    .unwrap()
}

// usage.rs:43 `Timestamp::from_millisecond(at as i64).expect("an instant in range")`: jiff stops at year 9999, a JS Date
// at year 275760, and resetsAt is any z.number() on the wire.
// node: resetsWord(1e15, 1.7e12, "UTC") => "resets on Fri"
// node: windowCell({kind:"week",usedPercent:50,resetsAt:1e15}, 1.7e12, "UTC") => "50% resets on Fri"
#[test]
fn a_reset_time_past_year_9999_reads_its_weekday_and_does_not_panic() {
    assert_eq!(resets_word(1e15, 1.7e12, Some("UTC")), "resets on Fri");
    let window = LimitWindow { kind: LimitKind::Week, used_percent: JsNumber(50.0), resets_at: Some(JsNumber(1e15)) };
    assert_eq!(window_cell(&window, 1.7e12, Some("UTC")), "50% resets on Fri");
}

// node: dayKeyOf(253402300800000, "UTC") => "10000-01-01"; hourOf(..) => 0; clockWord(.., 1.7e12, "UTC") => "Sat 00:00";
// asOfWord(.., 1.7e12, "UTC") => "as of Sat 00:00"
#[test]
fn a_day_key_hour_and_clock_past_year_9999_read_as_node_reads_them() {
    assert_eq!(day_key_of(253402300800000.0, Some("UTC")), "10000-01-01");
    assert_eq!(hour_of(253402300800000.0, Some("UTC")), 0.0);
    assert_eq!(clock_word(253402300800000.0, 1.7e12, Some("UTC")), "Sat 00:00");
    assert_eq!(as_of_word(253402300800000.0, 1.7e12, Some("UTC")), "as of Sat 00:00");
}

// usage.rs:40 `TimeZone::get(name).expect("an IANA zone name")`: Intl in node 22 takes an offset as a zone name.
// node: dayKeyOf(1.7e12, "+05:30") => "2023-11-15"; dayKeyOf(1.7e12, "-08:00") => "2023-11-14"
#[test]
fn an_offset_zone_name_is_a_zone_and_does_not_panic() {
    assert_eq!(day_key_of(1.7e12, Some("+05:30")), "2023-11-15");
    assert_eq!(day_key_of(1.7e12, Some("-08:00")), "2023-11-14");
}

// usage.rs:711 formats the year with `{:04}`; Intl's en-CA `year: "numeric"` does not pad.
// node: dayKeyOf(-62135596800000, "UTC") => "1-01-01"
#[test]
fn a_year_before_1000_is_not_zero_padded_in_the_day_key() {
    assert_eq!(day_key_of(-62135596800000.0, Some("UTC")), "1-01-01");
}

// cost_history.rs:108 and :121 `Timestamp::from_millisecond(at as i64).unwrap()`.
// node (TZ=UTC): monthStart(253402300800000) => 253402300800000; dayStart(253402300800000) => 253402300800000
#[test]
fn month_and_day_start_past_year_9999_do_not_panic() {
    assert_eq!(month_start(253402300800000.0, &utc()), 253402300800000.0);
    assert_eq!(day_start(253402300800000.0, &utc()), 253402300800000.0);
}

// cost_history.rs:108: `NaN as i64` is 0, so NaN reads as the start of January 1970.
// node: monthStart(NaN) => NaN; dayStart(NaN) => NaN
#[test]
fn month_and_day_start_of_nan_are_nan() {
    assert!(month_start(f64::NAN, &utc()).is_nan());
    assert!(day_start(f64::NAN, &utc()).is_nan());
}

// cost_history.rs:162 `parse_at` is jiff's RFC 3339 parser; the TS reads ticks with Date.parse, which takes a date alone
// as UTC midnight.
// node: accruedAt([tick("2026-10-01",0), tick("2026-10-01T01:00:00Z",1)], 1790812800001) => 2.7777777777777776e-7
// node: rateAt([tick("2026-10-01",0)], 0) => null
#[test]
fn a_tick_dated_by_its_day_alone_is_read_as_date_parse_reads_it() {
    let points = [tick("2026-10-01", 0.0), tick("2026-10-01T01:00:00Z", 1.0)];
    assert_eq!(accrued_at(&points, 1790812800001.0), Some(2.7777777777777776e-7));
    assert_eq!(rate_at(&[tick("2026-10-01", 0.0)], 0.0), None);
}

// turn.rs:596-603 fmt_clock: `new Date(NaN).toTimeString().slice(0, 8)` is "Invalid " (with its space); Rust floors NaN to 0.
// node: fmtClock(NaN) => "Invalid "; fmtClock(8640000000000001) => "Invalid "
#[test]
fn fmt_clock_of_nan_or_past_the_date_range_is_invalid_with_its_space() {
    assert_eq!(fmt_clock(Some(f64::NAN), &|_| 0), "Invalid ");
    assert_eq!(fmt_clock(Some(8640000000000001.0), &|_| 0), "Invalid ");
}

// turn.rs:600 fmt_clock floors the instant to a second; `new Date(at)` truncates toward zero first.
// node (TZ=UTC): fmtClock(-0.5) => "00:00:00"
#[test]
fn fmt_clock_truncates_a_negative_fraction_of_a_millisecond_toward_zero() {
    assert_eq!(fmt_clock(Some(-0.5), &|_| 0), "00:00:00");
}

// turn.rs:927 unquote ignores that the TS regex /^(["'“‘])(.*)(["'”’])$/ has `.` stop at U+2028 and U+2029.
// node: generatedTitle('"a b"') => '"a b"'; generatedTitle('“a b”') => '“a b”'
#[test]
fn generated_title_keeps_quotes_around_a_line_separator() {
    assert_eq!(generated_title("\"a\u{2028}b\"").as_deref(), Some("\"a\u{2028}b\""));
    assert_eq!(generated_title("\u{201c}a\u{2029}b\u{201d}").as_deref(), Some("\u{201c}a\u{2029}b\u{201d}"));
}

// base.rs:23 lower_first: `charAt(0)` of a char above U+FFFF is a lone high surrogate, which has no case (upper_first
// already handles this).
// node: lowerFirst("\u{10400}x") => "\u{10400}x"
#[test]
fn lower_first_leaves_a_first_char_above_ffff_as_it_is() {
    assert_eq!(lower_first("\u{10400}x"), "\u{10400}x");
}

// units.rs:94 `!(MIB..GIB).contains(&n)` is true for NaN; the TS `n < MIB || n >= GIB` is false for NaN.
// node: fmtBytesOver(NaN) => "NaN MB"
#[test]
fn fmt_bytes_over_of_nan_reads_mb() {
    assert_eq!(fmt_bytes_over(f64::NAN), "NaN MB");
}

// js.rs:304 folds a hex, octal or binary integer in f64, rounding at every step; JS rounds the exact value once.
// node: Number("0x10000000000000801") => 18446744073709556000
// node: Number("0b1000000000000000000000000000000000000000000000000000011") => 18014398509481988
#[test]
fn to_number_rounds_a_long_hex_or_binary_integer_once() {
    assert_eq!(to_number("0x10000000000000801"), 18446744073709556000.0);
    assert_eq!(to_number("0b1000000000000000000000000000000000000000000000000000011"), 18014398509481988.0);
}

// js.rs number_string -> wsp-wire js_number.rs:24 and js.rs:369 shortest_digits read Rust's `{:e}`, which breaks an exact
// tie between two 17-digit candidates upward; JS takes the even digit.
// node: String(100000000000000.125) => "100000000000000.12"; fmtCost(100000000000000.125) => "$100,000,000,000,000.12"
#[test]
fn a_seventeen_digit_tie_takes_the_even_digit_as_javascript_does() {
    assert_eq!(number_string(100000000000000.125), "100000000000000.12");
    assert_eq!(fmt_cost(100000000000000.125), "$100,000,000,000,000.12");
}

// js.rs:416-419 serialize_f64 hands a non-integral value to serde_json, which writes 1e-6 to below 1e-5 in exponent form;
// JS writes plain digits down to 1e-6. (The accepted wire change names only values below 1e-6.)
// node: JSON.stringify(3e-6) => "0.000003"; JSON.stringify(0.0000015) => "0.0000015"
#[test]
fn serialize_f64_writes_a_fraction_from_1e_6_in_plain_digits() {
    #[derive(serde::Serialize)]
    struct W(#[serde(serialize_with = "wsp_words::js::serialize_f64")] f64);
    assert_eq!(serde_json::to_string(&W(3e-6)).unwrap(), "0.000003");
    assert_eq!(serde_json::to_string(&W(0.0000015)).unwrap(), "0.0000015");
}

// units.rs:1048 fmt_inr reads `is_sign_negative()` before it knows the value is a number: NaN made by 0/0 or inf-inf on x86
// has its sign bit set.
// node: fmtInr(NaN) => "₹NaN"
#[test]
fn fmt_inr_of_a_nan_with_its_sign_bit_set_has_no_minus() {
    assert_eq!(fmt_inr(f64::from_bits(0xFFF8_0000_0000_0000)), "\u{20b9}NaN");
}

// usage.rs:605 parse_rate_table keeps the file's key order; Object.entries lists integer-like keys first, ascending.
// node: Object.keys(parseRateTable(JSON.parse('{"b":{...},"7":{...}}'))) => ["7","b"]
#[test]
fn parse_rate_table_lists_integer_like_names_first() {
    let doc: serde_json::Value = serde_json::from_str(
        r#"{"b":{"input_cost_per_token":1,"output_cost_per_token":2},"7":{"input_cost_per_token":3,"output_cost_per_token":4}}"#,
    )
    .unwrap();
    let table = wsp_words::usage::parse_rate_table(&doc);
    let keys: Vec<&str> = table.keys().map(String::as_str).collect();
    assert_eq!(keys, ["7", "b"]);
}
