// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-collect (#2446): the lead reviewer's own cases. Every expected value was printed by the TS at
//! 6ff07bdbc on node 22.23.
mod common;

use common::fake_host::*;
use wsp_collect::ghostty_config::read_ghostty_config;
use wsp_wire::views::project_bundle::TerminalScheme;

const HEAD: &str = r#"{"files":["/Users/dev/.config/ghostty/config"],"fontFamily":[],"palette":[null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null],"#;

/// One config line each, and the tail of the reply `JSON.stringify` wrote for it (vitest over ghostty-config.ts).
const GHOSTTY_NUMBERS: &[(&str, &str)] = &[
    ("font-size = 0.00001", r#""fontSize":0.00001}"#),
    ("font-size = 0.000123", r#""fontSize":0.000123}"#),
    ("font-size = 1e-7", r#""fontSize":1e-7}"#),
    ("font-size = 4503599627370495.5", r#""fontSize":4503599627370495.5}"#),
    ("font-size = 18446744073709551616", r#""fontSize":18446744073709552000}"#),
    ("font-size = 1e21", r#""fontSize":1e+21}"#),
    ("font-size = 5e-324", r#""fontSize":5e-324}"#),
    ("font-size = 1.7976931348623157e308", r#""fontSize":1.7976931348623157e+308}"#),
    ("font-size = 13.5", r#""fontSize":13.5}"#),
    ("font-size = 12345678901234567890", r#""fontSize":12345678901234567000}"#),
    ("background-opacity = 0.00001", r#""backgroundOpacity":0.00001}"#),
    ("background-opacity = 0.0000015", r#""backgroundOpacity":0.0000015}"#),
    ("window-padding-x = 0.00001", r#""windowPaddingX":{"left":0.00001,"right":0.00001}}"#),
    ("background-blur = 18446744073709551616", r#""backgroundBlur":18446744073709552000}"#),
    ("background-blur = 1e21", r#""backgroundBlur":1e+21}"#),
];

/// The three lines above inside the two ranges tests/ghostty_config.rs pins as accepted (a fraction from 1e-6 up to 1e-5,
/// a whole number from 2^64 up to 1e21), with the tail serde_json writes for each: the same number, other digits.
const PINNED_RANGES: &[(&str, &str)] = &[
    ("font-size = 18446744073709551616", r#""fontSize":1.8446744073709552e+19}"#),
    ("background-opacity = 0.0000015", r#""backgroundOpacity":1.5e-6}"#),
    ("background-blur = 18446744073709551616", r#""backgroundBlur":1.8446744073709552e+19}"#),
];

#[test]
fn ghostty_numbers_past_the_five_pinned_cases_write_as_node_writes_them() {
    let mut differ = Vec::new();
    for (line, node_tail) in GHOSTTY_NUMBERS {
        let pinned = PINNED_RANGES.iter().find(|(l, _)| l == line).map(|(_, rust)| *rust);
        let tail = pinned.unwrap_or(node_tail);
        let want = format!("{HEAD}{tail}");
        let laptop = FakeLaptop::new().file("~/.config/ghostty/config", format!("{line}\n"));
        let got = serde_json::to_string(&block_on(read_ghostty_config(&fake_host(laptop), TerminalScheme::Dark))).unwrap();
        if pinned.is_some() {
            let node: serde_json::Value = serde_json::from_str(&format!("{HEAD}{node_tail}")).unwrap();
            let rust: serde_json::Value = serde_json::from_str(&got).unwrap();
            assert_eq!(rust, node, "{line}: the same number");
        }
        if got != want {
            differ.push(format!("{line}\n  node: {tail}\n  rust: {}", got.strip_prefix(HEAD).unwrap_or(&got)));
        }
    }
    assert!(differ.is_empty(), "{} configs differ:\n{}", differ.len(), differ.join("\n"));
}

/// `Date.parse` reads an ISO date-time with no offset as local time (spec/history_readers.md Decision 1).
/// node: `TZ=Asia/Kolkata node -e 'Date.parse("2026-09-06T10:01:00")'` is 1788669060000.
#[test]
fn an_offsetless_stamp_reads_as_local_time_as_date_parse_does() {
    std::env::set_var("TZ", "Asia/Kolkata");
    let at = |s: &str| wsp_collect::history::reader::time_of(Some(&serde_json::json!(s)));
    assert_eq!(at("2026-09-06"), Some(1788652800000.0), "a date alone is UTC in both");
    assert_eq!(at("2026-09-06T10:01:00"), Some(1788669060000.0));
    assert_eq!(at("2026-09-06T10:01:00.5"), Some(1788669060500.0));
}

mod refusal_text {
    use wsp_collect::host::RunOptions;
    use wsp_collect::live_host::spawn_run;

    async fn refused(cmd: &str, args: &[&str], vars: &[(&str, &str)]) -> String {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let vars: Vec<(String, String)> = vars.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        spawn_run(cmd, &args, &vars, RunOptions::default()).await.expect_err("node refuses this").message
    }

    /// util.inspect splits a string past 124 units at its newlines with `' +\n  '`, before the 128-unit cut.
    #[tokio::test(flavor = "current_thread")]
    async fn a_long_refused_argument_with_a_newline_is_printed_as_node_prints_it() {
        let arg = format!("line1\nline2\0{}", "z".repeat(120));
        assert_eq!(
            refused("sh", &[&arg], &[]).await,
            format!(
                "The argument 'args[0]' must be a string without null bytes. Received 'line1\\n' +\n  'line2\\x00{}...",
                "z".repeat(104)
            )
        );
    }

    /// node walks `Object.keys(env)`, which lists integer-like keys first.
    #[tokio::test(flavor = "current_thread")]
    async fn an_integer_like_variable_name_is_checked_first_as_object_keys_lists_it() {
        assert_eq!(
            refused("sh", &[], &[("B", "b\0"), ("2", "two\0")]).await,
            "The property 'options.env['2']' must be a string without null bytes. Received 'two\\x00'"
        );
    }
}

/// The cache's write failure is node's own error text: cache.ts:69 `mkdirSync(dirname(path), { recursive: true })` under
/// a path whose parent is a regular file throws `EEXIST: file already exists, mkdir '<that file>'` (vitest over
/// cache.ts, node 22.23). readHistories rethrows it, and readRecipeOptions' op fails with it.
#[test]
fn a_cache_that_cannot_be_written_fails_with_nodes_words() {
    use wsp_collect::history::cache::{file_history_cache, HistoryCache};
    use wsp_collect::history::tally::HistoryBucket;
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("file");
    std::fs::write(&file, "x").unwrap();
    let cache = file_history_cache(file.join("history-cache.json"));
    let bucket =
        HistoryBucket { session: "s".into(), folder: None, calls: 1, commands: Default::default(), installs: Default::default(), at: None };
    cache.set("/f", "1", &[bucket]);
    let err = cache.save().expect_err("the folder cannot be made");
    assert_eq!(err.message, format!("EEXIST: file already exists, mkdir '{}'", file.display()));
}
