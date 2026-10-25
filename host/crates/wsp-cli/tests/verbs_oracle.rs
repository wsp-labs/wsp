// SPDX-License-Identifier: AGPL-3.0-only
// The verb table, the lookup, the help pages, node's parser and the run before a dial, each held to the answers the TS
// gave (oracle/cli.oracle.ts, fixtures lookup, parse_args, pages, verbs, all_verbs and run_verb). Every test loops over
// every row of its fixture and names the row in its message. `_cloud` fixtures are the cloud-on run.

#[path = "cli_a_common/mod.rs"]
mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use common::{fixture, strings, RecordingIo};
use wsp_cli::parse_args::{parse_args, ParseArgsCode};
use wsp_cli::verbs::client::{
    CliIo, DialFn, DialOpts, FlagKind, FlagRow, FlagTable, FlagValue, HostClient, InputKind, LineVerb, Page, Tool, Verb,
};
use wsp_cli::verbs::{
    about_lines, all_verbs, cli_verbs, cloud_line_of, common_flag_words, find_verb, flag_says, flag_words, help_page, own_flags_of,
    run_verb, usage_lines, verb_help, verb_page, verb_usage, verbs, verbs_for, RunDeps,
};
use wsp_words::exit::{HostError, Kind};

// ---------------------------------------------------------------------------------------------------------------
// Readings of the table that touch no stub of the interface under test: the entry's own fields.
// ---------------------------------------------------------------------------------------------------------------

fn table_for(cloud: bool) -> Vec<Verb> {
    verbs_for(cloud)
}

fn line_of(v: &Verb) -> Option<LineVerb<'_>> {
    match v {
        Verb::Cli(c) => Some(LineVerb::Cli(c)),
        Verb::CliOnly(c) => Some(LineVerb::CliOnly(c)),
        _ => None,
    }
}

fn name_of(v: &Verb) -> &'static str {
    match v {
        Verb::Cli(c) => c.name,
        Verb::ToolOnly(c) => c.name,
        Verb::CliOnly(c) => c.name,
        Verb::CommandTool(c) => c.name,
    }
}

fn line_name(v: LineVerb<'_>) -> &'static str {
    match v {
        LineVerb::Cli(c) => c.name,
        LineVerb::CliOnly(c) => c.name,
    }
}

fn line_host_side(v: LineVerb<'_>) -> Option<&'static str> {
    match v {
        LineVerb::Cli(_) => None,
        LineVerb::CliOnly(c) => c.host_side,
    }
}

fn line_options(v: LineVerb<'_>) -> &FlagTable {
    match v {
        LineVerb::Cli(c) => &c.options,
        LineVerb::CliOnly(c) => &c.options,
    }
}

fn line_verbs(table: &[Verb]) -> Vec<LineVerb<'_>> {
    table.iter().filter_map(line_of).collect()
}

fn shape_of(v: &Verb) -> &'static str {
    match v {
        Verb::Cli(_) => "cli",
        Verb::ToolOnly(_) => "toolOnly",
        Verb::CliOnly(_) => "cliOnly",
        Verb::CommandTool(_) => "commandTool",
    }
}

fn tool_of(v: &Verb) -> Option<&Tool> {
    match v {
        Verb::Cli(c) => Some(&c.tool),
        Verb::ToolOnly(c) => Some(&c.tool),
        Verb::CliOnly(_) => None,
        Verb::CommandTool(c) => Some(&c.tool),
    }
}

fn page_word(p: Page) -> &'static str {
    match p {
        Page::Front => "front",
        Page::Agent => "agent",
        Page::Host => "host",
        Page::Dev => "dev",
        Page::App => "app",
    }
}

fn page_of(word: &str) -> Option<Page> {
    match word {
        "front" => Some(Page::Front),
        "agent" => Some(Page::Agent),
        "host" => Some(Page::Host),
        "dev" => Some(Page::Dev),
        "app" => Some(Page::App),
        _ => None,
    }
}

fn kind_word(k: FlagKind) -> &'static str {
    match k {
        FlagKind::String => "string",
        FlagKind::Boolean => "boolean",
    }
}

fn opt_str(s: Option<&str>) -> Value {
    s.map_or(Value::Null, |s| Value::String(s.to_owned()))
}

fn kind_json(k: &InputKind) -> Value {
    use serde_json::json;
    match k {
        InputKind::String => json!({ "type": "string" }),
        InputKind::Boolean => json!({ "type": "boolean" }),
        InputKind::Number => json!({ "type": "number" }),
        InputKind::Integer { min } => json!({ "type": "integer", "min": min }),
        InputKind::StringList { min } => json!({ "type": "stringList", "min": min }),
        InputKind::Enum(options) => json!({ "type": "enum", "options": options }),
        InputKind::Literal(value) => json!({ "type": "literal", "value": value }),
        InputKind::Json => json!({ "type": "json" }),
    }
}

fn options_json(table: &FlagTable) -> Value {
    Value::Array(
        table
            .0
            .iter()
            .map(|(name, row)| {
                serde_json::json!({
                    "name": name,
                    "type": kind_word(row.kind),
                    "multiple": row.multiple,
                    "short": row.short.map(|c| c.to_string()),
                    "valueWith": row.value_with,
                })
            })
            .collect(),
    )
}

fn cloud_of(v: &Verb) -> bool {
    match v {
        Verb::Cli(c) => c.cloud,
        Verb::CliOnly(c) => c.cloud,
        _ => false,
    }
}

fn cloud_flags_of(v: &Verb) -> Vec<&'static str> {
    match v {
        Verb::Cli(c) => c.cloud_flags.clone(),
        Verb::CliOnly(c) => c.cloud_flags.clone(),
        _ => Vec::new(),
    }
}

fn argv_of(v: &Value) -> Vec<String> {
    strings(v)
}

fn str_or_null(v: &Value) -> Option<&str> {
    v.as_str()
}

// ---------------------------------------------------------------------------------------------------------------
// lookup.json
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn find_verb_answers_the_longest_name_the_line_opens() {
    for cloud in [false, true] {
        let table = table_for(cloud);
        let fx = fixture("lookup", cloud);
        for row in fx["found"].as_array().unwrap() {
            let argv = argv_of(&row["argv"]);
            let found = find_verb(&table, &argv).map(line_name);
            assert_eq!(found, str_or_null(&row["verb"]), "cloud {cloud}: find_verb for {argv:?}");
        }
    }
}

#[test]
fn cloud_line_of_names_the_cloud_line_a_cloud_off_process_was_given() {
    for cloud in [false, true] {
        let fx = fixture("lookup", cloud);
        for row in fx["found"].as_array().unwrap() {
            let argv = argv_of(&row["argv"]);
            let got = cloud_line_of(&argv, cloud, true);
            assert_eq!(got.as_deref(), str_or_null(&row["cloudLine"]), "cloud {cloud}: cloud_line_of for {argv:?}");
            if cloud {
                assert_eq!(got, None, "cloud on: cloud_line_of for {argv:?} is always None");
            }
        }
    }
}

#[test]
fn cloud_line_of_is_none_in_a_public_build_whatever_the_line_is() {
    // Read from client.ts / verbs.ts:404 (`CLOUD_ON || !CLOUD_BUILT` answers nothing), not recorded by the oracle.
    let fx = fixture("lookup", false);
    let mut one_names_a_cloud_line = false;
    for row in fx["found"].as_array().unwrap() {
        let argv = argv_of(&row["argv"]);
        one_names_a_cloud_line |= cloud_line_of(&argv, false, true).is_some();
        assert_eq!(cloud_line_of(&argv, false, false), None, "public build: cloud_line_of for {argv:?}");
        assert_eq!(cloud_line_of(&argv, true, false), None, "public build, cloud on: cloud_line_of for {argv:?}");
    }
    assert!(one_names_a_cloud_line, "the cloud-off fixture holds a line (wsp fork) that names a cloud line");
}

#[test]
fn take_common_splits_the_shared_flags_off_the_line() {
    for cloud in [false, true] {
        let fx = fixture("lookup", cloud);
        for row in fx["commons"].as_array().unwrap() {
            let argv = argv_of(&row["argv"]);
            let got = wsp_cli::verbs::client::take_common(&argv);
            assert_eq!(got.common, strings(&row["common"]), "cloud {cloud}: take_common common for {argv:?}");
            assert_eq!(got.rest, strings(&row["rest"]), "cloud {cloud}: take_common rest for {argv:?}");
        }
    }
}

#[test]
fn json_asked_reads_json_before_the_first_double_dash() {
    for cloud in [false, true] {
        let fx = fixture("lookup", cloud);
        for row in fx["commons"].as_array().unwrap() {
            let argv = argv_of(&row["argv"]);
            assert_eq!(
                wsp_cli::verbs::client::json_asked(&argv),
                row["jsonAsked"].as_bool().unwrap(),
                "cloud {cloud}: json_asked for {argv:?}"
            );
        }
    }
}

#[test]
fn optional_values_gives_a_bare_value_with_flag_its_equals_sign() {
    let table = FlagTable(vec![
        ("project", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: Some("on") }),
        ("on", FlagRow::STRING),
        ("agent", FlagRow::STRINGS),
    ]);
    for cloud in [false, true] {
        let fx = fixture("lookup", cloud);
        for row in fx["optional"].as_array().unwrap() {
            let argv = argv_of(&row["argv"]);
            let got = wsp_cli::verbs::client::optional_values(&argv, &table);
            assert_eq!(got, strings(&row["out"]), "cloud {cloud}: optional_values for {argv:?}");
        }
    }
}

#[test]
fn tool_name_joins_a_verbs_words_with_underscores() {
    for cloud in [false, true] {
        let fx = fixture("lookup", cloud);
        for row in fx["toolNames"].as_array().unwrap() {
            let words = row[0].as_str().unwrap();
            assert_eq!(wsp_cli::verbs::client::tool_name(words), row[1].as_str().unwrap(), "cloud {cloud}: tool_name for {words:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// parse_args.json
// ---------------------------------------------------------------------------------------------------------------

fn parse_table(fx: &Value) -> FlagTable {
    let rows = fx["options"].as_array().unwrap().iter().map(|pair| {
        let name: &'static str = Box::leak(pair[0].as_str().unwrap().to_owned().into_boxed_str());
        let o = &pair[1];
        let kind = match o["type"].as_str().unwrap() {
            "string" => FlagKind::String,
            "boolean" => FlagKind::Boolean,
            other => panic!("option {name}: type {other}"),
        };
        let short = o.get("short").and_then(Value::as_str).map(|s| s.chars().next().unwrap());
        (name, FlagRow { kind, multiple: o.get("multiple").and_then(Value::as_bool) == Some(true), short, value_with: None })
    });
    FlagTable(rows.collect())
}

fn flag_value_of(v: &Value) -> FlagValue {
    match v {
        Value::String(s) => FlagValue::Str(s.clone()),
        Value::Bool(b) => FlagValue::Bool(*b),
        Value::Array(_) => FlagValue::List(strings(v)),
        other => panic!("a parsed value of {other}"),
    }
}

#[test]
fn parse_args_answers_node_values_in_order_and_positionals() {
    let fx = fixture("parse_args", false);
    let table = parse_table(&fx);
    assert_eq!(table.0.len(), 7, "the oracle's option table has seven rows");
    let rows = fx["parsed"].as_array().unwrap();
    let mut ok_rows = 0;
    for row in rows {
        let args = argv_of(&row["args"]);
        let got = parse_args(&args, &table);
        if row.get("code").is_some() {
            continue;
        }
        ok_rows += 1;
        let parsed = got.unwrap_or_else(|e| panic!("parse_args for {args:?} refused: {e}"));
        let want: Vec<(String, FlagValue)> =
            row["values"].as_array().unwrap().iter().map(|p| (p[0].as_str().unwrap().to_owned(), flag_value_of(&p[1]))).collect();
        let have: Vec<(String, FlagValue)> = parsed.values.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        assert_eq!(have, want, "parse_args values for {args:?}");
        assert_eq!(parsed.positionals, strings(&row["positionals"]), "parse_args positionals for {args:?}");
    }
    assert!(ok_rows > 0);
}

#[test]
fn parse_args_refuses_with_node_codes_and_messages() {
    let fx = fixture("parse_args", false);
    let table = parse_table(&fx);
    let mut refused_rows = 0;
    for row in fx["parsed"].as_array().unwrap() {
        let args = argv_of(&row["args"]);
        let Some(code) = row.get("code").and_then(Value::as_str) else { continue };
        refused_rows += 1;
        let want_code = match code {
            "ERR_PARSE_ARGS_UNKNOWN_OPTION" => ParseArgsCode::UnknownOption,
            "ERR_PARSE_ARGS_INVALID_OPTION_VALUE" => ParseArgsCode::InvalidOptionValue,
            other => panic!("code {other}"),
        };
        match parse_args(&args, &table) {
            Ok(parsed) => panic!("parse_args for {args:?} answered {parsed:?}, node refused with {code}"),
            Err(e) => {
                assert_eq!(e.code, want_code, "parse_args code for {args:?}");
                assert_eq!(e.message, row["message"].as_str().unwrap(), "parse_args message for {args:?}");
            }
        }
    }
    assert!(refused_rows > 0);
}

// ---------------------------------------------------------------------------------------------------------------
// pages.json
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn verb_page_is_the_whole_help_of_every_line_verb_byte_for_byte() {
    for cloud in [false, true] {
        let table = table_for(cloud);
        let fx = fixture("pages", cloud);
        let pages = fx["pages"].as_object().unwrap();
        let lines = line_verbs(&table);
        let names: Vec<&str> = lines.iter().map(|v| line_name(*v)).collect();
        let want: Vec<&str> = pages.keys().map(String::as_str).collect();
        assert_eq!(names, want, "cloud {cloud}: the line verbs of the table, in order");
        for verb in lines {
            let name = line_name(verb);
            let host = if line_host_side(verb).is_some() { common_flag_words::HOST_SIDE } else { common_flag_words::HOST };
            assert_eq!(verb_page(verb, host, cloud), pages[name].as_str().unwrap(), "cloud {cloud}: verb_page of {name}");
        }
    }
}

#[test]
fn verb_help_lists_each_page_and_the_whole_table() {
    for cloud in [false, true] {
        let table = table_for(cloud);
        let fx = fixture("pages", cloud);
        let help = fx["help"].as_object().unwrap();
        assert_eq!(verb_help(&table, None), help["all"].as_str().unwrap(), "cloud {cloud}: verb_help of every page");
        for word in ["front", "agent", "host", "dev", "app"] {
            assert_eq!(verb_help(&table, page_of(word)), help[word].as_str().unwrap(), "cloud {cloud}: verb_help of page {word}");
        }
    }
}

#[test]
fn verb_usage_gathers_the_usages_that_open_with_a_word() {
    for cloud in [false, true] {
        let table = table_for(cloud);
        let fx = fixture("pages", cloud);
        for (word, want) in fx["usages"].as_object().unwrap() {
            assert_eq!(verb_usage(&table, word).as_deref(), str_or_null(want), "cloud {cloud}: verb_usage of {word:?}");
        }
    }
}

#[test]
fn flag_says_reads_the_verbs_own_row_then_the_words() {
    for cloud in [false, true] {
        let fx = fixture("pages", cloud);
        let says = fx["says"].as_object().unwrap();
        assert!(!says.is_empty());
        for (key, want) in says {
            let (verb, flag) = key.rsplit_once(' ').unwrap_or_else(|| panic!("a says key {key:?}"));
            assert_eq!(flag_says(verb, flag, cloud).as_deref(), str_or_null(want), "cloud {cloud}: flag_says of {key:?}");
        }
        assert_eq!(flag_says("threads", "no-such-flag", cloud), None, "cloud {cloud}: a flag no row names says nothing");
    }
}

#[test]
fn flag_says_cuts_the_cloud_spans_by_the_switch() {
    // FLAG_WORDS["turn-limit"] carries `<!-- cloud -->` marks (verbs.ts:440): the two switches say it two ways.
    let off = flag_says("computers set", "turn-limit", false).expect("a row for turn-limit");
    let on = flag_says("computers set", "turn-limit", true).expect("a row for turn-limit");
    let fx_off = fixture("pages", false);
    let fx_on = fixture("pages", true);
    assert_eq!(off, fx_off["says"]["computers set turn-limit"].as_str().unwrap(), "cloud off: computers set turn-limit");
    assert_eq!(on, fx_on["says"]["computers set turn-limit"].as_str().unwrap(), "cloud on: computers set turn-limit");
    assert_ne!(off, on, "the cloud spans differ between the two switches");
    assert!(!off.contains("<!--") && !on.contains("<!--"), "no mark survives: {off:?} / {on:?}");
}

#[test]
fn own_flags_of_lists_the_verbs_flags_without_the_shared_four() {
    for cloud in [false, true] {
        let table = table_for(cloud);
        let fx = fixture("pages", cloud);
        let says = fx["says"].as_object().unwrap();
        let mut named = 0;
        for verb in line_verbs(&table) {
            let name = line_name(verb);
            let own: Vec<&str> =
                line_options(verb).0.iter().map(|(n, _)| *n).filter(|n| !["host", "state", "json", "help"].contains(n)).collect();
            assert_eq!(own_flags_of(verb), own, "cloud {cloud}: own_flags_of {name}");
            for flag in &own {
                assert!(says.contains_key(&format!("{name} {flag}")), "cloud {cloud}: the fixture's says holds a row for {name} {flag}");
                named += 1;
            }
        }
        assert_eq!(named, says.len(), "cloud {cloud}: every says key is one own flag of one verb");
    }
}

#[test]
fn usage_lines_wraps_each_line_of_a_usage_and_never_inside_brackets() {
    let fx = fixture("pages", false);
    for case in fx["usageLines"].as_array().unwrap() {
        let usage = case["usage"].as_str().unwrap();
        let got = usage_lines(usage, case["indent"].as_str().unwrap(), case["lead"].as_str().unwrap());
        assert_eq!(got, strings(&case["lines"]), "usage_lines of {usage:?}");
    }
}

#[test]
fn help_page_pads_the_flag_words_and_wraps_under_them() {
    for cloud in [false, true] {
        let fx = fixture("pages", cloud);
        for case in fx["helpPages"].as_array().unwrap() {
            let usage = case["usage"].as_str().unwrap();
            let rows: Vec<(String, String)> = case["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| (r[0].as_str().unwrap().to_owned(), r[1].as_str().unwrap().to_owned()))
                .collect();
            assert_eq!(
                help_page(usage, &strings(&case["about"]), &rows),
                case["page"].as_str().unwrap(),
                "cloud {cloud}: help_page of {usage:?}"
            );
        }
    }
}

#[test]
fn about_lines_wraps_the_about_behind_its_indent() {
    for cloud in [false, true] {
        let table = table_for(cloud);
        let fx = fixture("pages", cloud);
        for verb in line_verbs(&table) {
            let name = line_name(verb);
            let about = match verb {
                LineVerb::Cli(c) => c.about.as_str(),
                LineVerb::CliOnly(c) => c.about.as_str(),
            };
            let lines = about_lines(about, "  ");
            // Every about reads in a page as the lines `about_lines` gives it, between the usage and the blank line.
            let page = fx["pages"][name].as_str().unwrap();
            assert!(page.contains(&lines.join("\n")), "cloud {cloud}: about_lines of {name} stands in its page: {lines:?}");
            assert!(lines.iter().all(|l| l.starts_with("  ")), "cloud {cloud}: about_lines of {name} keeps its indent: {lines:?}");
        }
    }
}

#[test]
fn flag_words_is_the_raw_table_in_the_ts_objects_order() {
    for cloud in [false, true] {
        let fx = fixture("pages", cloud);
        let want: Vec<(String, String)> =
            fx["flagWords"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned())).collect();
        let have: Vec<(String, String)> = flag_words().into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
        assert_eq!(have.len(), want.len(), "cloud {cloud}: the number of FLAG_WORDS rows");
        for (at, (h, w)) in have.iter().zip(&want).enumerate() {
            assert_eq!(h, w, "cloud {cloud}: FLAG_WORDS row {at} ({:?})", w.0);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// verbs.json and verbs_cloud.json
// ---------------------------------------------------------------------------------------------------------------

fn entries(cloud: bool) -> (Vec<Verb>, Vec<Value>) {
    let table = verbs_for(cloud);
    let fx = fixture("verbs", cloud);
    (table, fx.as_array().unwrap().clone())
}

#[test]
fn the_table_has_the_fixtures_entries_in_order() {
    for cloud in [false, true] {
        let (table, want) = entries(cloud);
        let have: Vec<(&str, &str)> = table.iter().map(|v| (name_of(v), shape_of(v))).collect();
        let want: Vec<(&str, &str)> = want.iter().map(|e| (e["name"].as_str().unwrap(), e["shape"].as_str().unwrap())).collect();
        assert_eq!(have, want, "cloud {cloud}: the table's (name, shape) in order");
    }
}

#[test]
fn every_entry_says_its_usage_about_and_page() {
    for cloud in [false, true] {
        let (table, want) = entries(cloud);
        assert_eq!(table.len(), want.len(), "cloud {cloud}: entries");
        for (v, e) in table.iter().zip(&want) {
            let name = name_of(v);
            let (usage, about, page) = match v {
                Verb::Cli(c) => (Some(c.usage.as_str()), Some(c.about.as_str()), Some(page_word(c.page))),
                Verb::CliOnly(c) => (Some(c.usage.as_str()), Some(c.about.as_str()), Some(page_word(c.page))),
                _ => (None, None, None),
            };
            assert_eq!(usage, str_or_null(&e["usage"]), "cloud {cloud}: {name} usage");
            assert_eq!(about, str_or_null(&e["about"]), "cloud {cloud}: {name} about");
            assert_eq!(page, str_or_null(&e["page"]), "cloud {cloud}: {name} page");
        }
    }
}

#[test]
fn every_line_entry_reads_its_flags_in_declaration_order() {
    for cloud in [false, true] {
        let (table, want) = entries(cloud);
        assert_eq!(table.len(), want.len(), "cloud {cloud}: entries");
        for (v, e) in table.iter().zip(&want) {
            let name = name_of(v);
            match line_of(v) {
                Some(line) => assert_eq!(options_json(line_options(line)), e["options"], "cloud {cloud}: {name} options"),
                None => assert!(e["options"].is_null(), "cloud {cloud}: {name} is not a line verb, so the fixture has no options"),
            }
        }
    }
}

#[test]
fn every_entry_carries_its_marks_and_reasons() {
    for cloud in [false, true] {
        let (table, want) = entries(cloud);
        assert_eq!(table.len(), want.len(), "cloud {cloud}: entries");
        for (v, e) in table.iter().zip(&want) {
            let name = name_of(v);
            let (starts_no_host, any_release, cli_only, host_side, tool_only, reads_here) = match v {
                Verb::Cli(c) => (c.starts_no_host, c.any_release, None, None, None, c.reads_here),
                Verb::CliOnly(c) => (c.starts_no_host, c.any_release, Some(c.cli_only), c.host_side, None, c.reads_here),
                Verb::ToolOnly(c) => (None, None, None, None, Some(c.tool_only), c.reads_here),
                Verb::CommandTool(_) => (None, None, None, None, None, None),
            };
            assert_eq!(opt_str(reads_here), e["readsHere"], "cloud {cloud}: {name} reads_here");
            assert_eq!(opt_str(starts_no_host), e["startsNoHost"], "cloud {cloud}: {name} starts_no_host");
            assert_eq!(opt_str(any_release), e["anyRelease"], "cloud {cloud}: {name} any_release");
            assert_eq!(opt_str(cli_only), e["cliOnly"], "cloud {cloud}: {name} cli_only");
            assert_eq!(opt_str(host_side), e["hostSide"], "cloud {cloud}: {name} host_side");
            assert_eq!(opt_str(tool_only), e["toolOnly"], "cloud {cloud}: {name} tool_only");
            assert_eq!(cloud_of(v), e["cloud"].as_bool().unwrap(), "cloud {cloud}: {name} cloud");
            assert_eq!(cloud_flags_of(v), strings(&e["cloudFlags"]), "cloud {cloud}: {name} cloud_flags");
        }
    }
}

#[test]
fn every_tool_says_its_description_output_and_stream() {
    for cloud in [false, true] {
        let (table, want) = entries(cloud);
        assert_eq!(table.len(), want.len(), "cloud {cloud}: entries");
        for (v, e) in table.iter().zip(&want) {
            let name = name_of(v);
            match (tool_of(v), e["tool"].as_object()) {
                (None, None) => {}
                (Some(tool), Some(t)) => {
                    assert_eq!(tool.description, t["description"].as_str().unwrap(), "cloud {cloud}: {name} tool description");
                    assert_eq!(tool.output, strings(&t["output"]), "cloud {cloud}: {name} tool output");
                    assert_eq!(tool.stream, strings(&t["stream"]), "cloud {cloud}: {name} tool stream");
                }
                (have, want) => panic!("cloud {cloud}: {name} has a tool {}, the fixture {}", have.is_some(), want.is_some()),
            }
        }
    }
}

#[test]
fn every_tool_input_says_its_name_kind_optionality_and_words() {
    for cloud in [false, true] {
        let (table, want) = entries(cloud);
        assert_eq!(table.len(), want.len(), "cloud {cloud}: entries");
        for (v, e) in table.iter().zip(&want) {
            let name = name_of(v);
            let Some(tool) = tool_of(v) else { continue };
            let inputs = e["tool"]["input"].as_array().unwrap();
            let names: Vec<&str> = tool.input.iter().map(|i| i.name).collect();
            let want_names: Vec<&str> = inputs.iter().map(|i| i["name"].as_str().unwrap()).collect();
            assert_eq!(names, want_names, "cloud {cloud}: {name} tool input names in order");
            for (have, want) in tool.input.iter().zip(inputs) {
                let input = have.name;
                assert_eq!(kind_json(&have.kind), want["kind"], "cloud {cloud}: {name} input {input} kind");
                assert_eq!(have.optional, want["optional"].as_bool().unwrap(), "cloud {cloud}: {name} input {input} optional");
                assert_eq!(have.describe.as_deref(), str_or_null(&want["describe"]), "cloud {cloud}: {name} input {input} describe");
            }
        }
    }
}

#[test]
fn the_cloud_off_table_holds_no_cloud_verb_and_no_cloud_flag() {
    let off = table_for(false);
    let on = table_for(true);
    let off_names: Vec<&str> = off.iter().map(name_of).collect();
    let cloud_verbs: Vec<&str> = on.iter().filter(|v| cloud_of(v)).map(name_of).collect();
    assert_eq!(
        cloud_verbs,
        ["rename", "fork", "pause", "wake", "rebuild", "forget", "ssh", "export"],
        "the cloud verbs of the cloud-on table, by the fixture"
    );
    for name in &cloud_verbs {
        assert!(!off_names.contains(name), "cloud off: {name} is left out");
    }
    for v in &off {
        assert!(!cloud_of(v), "cloud off: {} is a cloud verb", name_of(v));
    }
    let delete = |table: &[Verb]| -> usize { table.iter().position(|v| name_of(v) == "delete").expect("a delete verb") };
    let (off_delete, on_delete) = (&off[delete(&off)], &on[delete(&on)]);
    let inputs = |v: &Verb| -> Vec<&'static str> { tool_of(v).unwrap().input.iter().map(|i| i.name).collect() };
    assert_eq!(inputs(off_delete), ["thread", "processes", "confirm"], "cloud off: delete's tool does not take workspace");
    assert_eq!(inputs(on_delete), ["thread", "workspace", "processes", "confirm"], "cloud on: delete's tool takes workspace");
    assert_eq!(
        line_usage(line_of(off_delete).unwrap()),
        "wsp delete <thread> [--processes stop|keep] [--yes]",
        "cloud off: delete's usage"
    );
    assert_eq!(
        line_usage(line_of(on_delete).unwrap()),
        "wsp delete <thread>|<workspace> [--processes stop|keep] [--yes]",
        "cloud on: delete's usage"
    );
}

fn line_usage(v: LineVerb<'_>) -> &str {
    match v {
        LineVerb::Cli(c) => &c.usage,
        LineVerb::CliOnly(c) => &c.usage,
    }
}

#[test]
fn all_verbs_is_every_group_in_order_before_the_cloud_cut() {
    let want = fixture("all_verbs", false);
    let want = want.as_array().unwrap();
    let have = all_verbs();
    let names: Vec<(&str, &str)> = have.iter().map(|v| (name_of(v), shape_of(v))).collect();
    let want_names: Vec<(&str, &str)> = want.iter().map(|e| (e["name"].as_str().unwrap(), e["shape"].as_str().unwrap())).collect();
    assert_eq!(names, want_names, "ALL_VERBS (name, shape) in order");
    for (v, e) in have.iter().zip(want) {
        let name = name_of(v);
        if let Some(line) = line_of(v) {
            assert_eq!(line_usage(line), e["usage"].as_str().unwrap(), "ALL_VERBS: {name} usage");
            assert_eq!(options_json(line_options(line)), e["options"], "ALL_VERBS: {name} options");
        }
        assert_eq!(cloud_of(v), e["cloud"].as_bool().unwrap(), "ALL_VERBS: {name} cloud");
        assert_eq!(cloud_flags_of(v), strings(&e["cloudFlags"]), "ALL_VERBS: {name} cloud_flags");
        match (tool_of(v), e["tool"].as_object()) {
            (None, None) => {}
            (Some(tool), Some(t)) => {
                assert_eq!(tool.description, t["description"].as_str().unwrap(), "ALL_VERBS: {name} tool description");
                let inputs: Vec<(&str, Value, bool)> = tool.input.iter().map(|i| (i.name, kind_json(&i.kind), i.optional)).collect();
                let want_inputs: Vec<(&str, Value, bool)> = t["input"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|i| (i["name"].as_str().unwrap(), i["kind"].clone(), i["optional"].as_bool().unwrap()))
                    .collect();
                assert_eq!(inputs, want_inputs, "ALL_VERBS: {name} tool input");
            }
            (have, want) => panic!("ALL_VERBS: {name} has a tool {}, the fixture {}", have.is_some(), want.is_some()),
        }
    }
}

#[test]
fn verbs_is_the_table_this_process_answers() {
    let cloud = std::env::var("WSP_CLOUD").as_deref() == Ok("1");
    let want = fixture("verbs", cloud);
    let have = verbs();
    let names: Vec<&str> = have.iter().map(name_of).collect();
    let want_names: Vec<&str> = want.as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap()).collect();
    assert_eq!(names, want_names, "verbs() names in order for cloud {cloud}");
}

#[test]
fn the_entry_accessors_read_the_entrys_own_fields() {
    for cloud in [false, true] {
        let (table, want) = entries(cloud);
        for (v, e) in table.iter().zip(&want) {
            let name = name_of(v);
            assert_eq!(v.name(), name, "cloud {cloud}: Verb::name of {name}");
            assert_eq!(v.is_cloud(), e["cloud"].as_bool().unwrap(), "cloud {cloud}: Verb::is_cloud of {name}");
            assert_eq!(wsp_cli::verbs::client::reads_here(v), e["readsHere"].as_str(), "cloud {cloud}: reads_here of {name}");
            assert_eq!(wsp_cli::verbs::client::has_tool(v).is_some(), !e["tool"].is_null(), "cloud {cloud}: has_tool of {name}");
            match (v.line(), line_of(v)) {
                (None, None) => {}
                (Some(got), Some(own)) => {
                    assert_eq!(got.name(), line_name(own), "cloud {cloud}: LineVerb::name of {name}");
                    assert_eq!(got.usage(), line_usage(own), "cloud {cloud}: LineVerb::usage of {name}");
                    assert_eq!(page_word(got.page()), e["page"].as_str().unwrap(), "cloud {cloud}: LineVerb::page of {name}");
                    assert_eq!(options_json(got.options()), e["options"], "cloud {cloud}: LineVerb::options of {name}");
                    assert_eq!(opt_str(got.starts_no_host()), e["startsNoHost"], "cloud {cloud}: LineVerb::starts_no_host of {name}");
                    assert_eq!(opt_str(got.any_release()), e["anyRelease"], "cloud {cloud}: LineVerb::any_release of {name}");
                    assert_eq!(opt_str(got.host_side()), e["hostSide"], "cloud {cloud}: LineVerb::host_side of {name}");
                    let about = match own {
                        LineVerb::Cli(c) => c.about.as_str(),
                        LineVerb::CliOnly(c) => c.about.as_str(),
                    };
                    assert_eq!(got.about(), about, "cloud {cloud}: LineVerb::about of {name}");
                }
                (got, own) => panic!("cloud {cloud}: Verb::line of {name} is {}, the entry is {}", got.is_some(), own.is_some()),
            }
        }
    }
}

#[test]
fn cli_verbs_are_the_line_entries_in_table_order() {
    for cloud in [false, true] {
        let table = table_for(cloud);
        let want: Vec<&str> = line_verbs(&table).into_iter().map(line_name).collect();
        let have: Vec<&str> = cli_verbs(&table).into_iter().map(line_name).collect();
        assert_eq!(have, want, "cloud {cloud}: cli_verbs names in order");
        assert_eq!(
            have.len(),
            fixture("pages", cloud)["pages"].as_object().unwrap().len(),
            "cloud {cloud}: cli_verbs count against the fixture's pages"
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------
// run_verb.json
// ---------------------------------------------------------------------------------------------------------------

/// The dial the oracle handed in: it counts and fails with a usage error whose message is "dialled".
#[derive(Default)]
struct CountingDial(AtomicUsize);

#[async_trait]
impl DialFn for CountingDial {
    async fn dial(&self, _state_path: &str, _opts: DialOpts) -> Result<Arc<dyn HostClient>, HostError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(HostError::with_kind("dialled", Kind::Usage))
    }
}

#[tokio::test]
async fn run_verb_answers_the_exit_code_and_the_lines_before_it_dials() {
    let table = table_for(false);
    let fx = fixture("run_verb", false);
    let rows = fx.as_array().unwrap();
    assert!(rows.len() >= 30, "the fixture holds the oracle's lines");
    for row in rows {
        let argv = argv_of(&row["argv"]);
        let Some(want_verb) = row["verb"].as_str() else {
            assert!(find_verb(&table, &argv).is_none(), "run_verb row {argv:?}: no verb is found");
            continue;
        };
        let verb = find_verb(&table, &argv)
            .unwrap_or_else(|| panic!("run_verb row {argv:?}: find_verb found no verb, the oracle found {want_verb}"));
        assert_eq!(line_name(verb), want_verb, "run_verb row {argv:?}: the verb found");
        let io = RecordingIo::new();
        let dial = Arc::new(CountingDial::default());
        let deps = RunDeps { dial: Some(dial.clone()), ..Default::default() };
        let as_io: Arc<dyn CliIo> = io.clone();
        let state_of = |flag: Option<&str>| flag.unwrap_or("/nowhere/state.json").to_owned();
        let code = run_verb(&table, verb, &argv, as_io, &state_of, deps).await;
        assert_eq!(code as i64, row["code"].as_i64().unwrap(), "run_verb row {argv:?}: exit code");
        assert_eq!(io.lines(), strings(&row["lines"]), "run_verb row {argv:?}: stdout lines");
        assert_eq!(io.errors(), strings(&row["errors"]), "run_verb row {argv:?}: stderr lines");
        assert_eq!(dial.0.load(Ordering::SeqCst) as i64, row["dialled"].as_i64().unwrap(), "run_verb row {argv:?}: dials");
    }
}
