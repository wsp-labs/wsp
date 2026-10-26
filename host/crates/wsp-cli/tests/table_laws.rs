// SPDX-License-Identifier: AGPL-3.0-only
//! The laws the whole verb table keeps (spec/verbs-table.md "Laws"), ported from packages/host/test/parity.test.ts,
//! pages.test.ts and cloud.test.ts as far as they stand on verbs.ts alone: the pages, the commands of the shared
//! parse and the skill are other crates' and other tests'.

#[path = "cli_a_common/mod.rs"]
mod common;

use std::collections::BTreeSet;

use regex::Regex;

use wsp_cli::cloud_text::cloud_text;
use wsp_cli::parse_args::parse_args;
use wsp_cli::verbs::client::{common as common_flags, flag_list, has_tool, tool_name, FlagKind, FlagTable, InputKind, LineVerb, Verb};
use wsp_cli::verbs::{all_verbs, cli_verbs, common_flag_words, flag_says, flag_words, own_flags_of, verb_page, verbs_for, HELP_WIDTH};

/// LISTS_ON_THE_COMMAND_LINE (parity.test.ts:223-252): every list a tool takes, against how its line carries it.
const LISTS_ON_THE_COMMAND_LINE: &[(&str, &str)] = &[
    ("commit files", "--file"),
    ("computers set reset", "--reset"),
    ("threads wait threads", "the threads are the words after the verb"),
    ("thread settle threads", "the threads are the words after the verb"),
    ("thread restore threads", "the threads are the words after the verb"),
    ("exec argv", "the command is the words after the verb"),
    ("export agents", "the catalog ids are one comma-joined value of --agents"),
    ("fork notify", "--notify"),
    ("run notify", "--notify"),
    ("run files", "--file"),
    ("send files", "--file"),
    ("skills add agent", "--agent"),
    ("servers add env", "--env"),
    ("servers add header", "--header"),
    ("agents set hide", "--hide"),
    ("agents set show", "--show"),
    ("agents set order", "the models are one comma-joined value of --order"),
    ("agents set add_model", "--add-model"),
    ("agents set drop_model", "--drop-model"),
    ("agents set reset", "--reset"),
    ("agents setup args", "--arg"),
    ("agents setup unset_env", "--unset-env"),
    ("agents setup reset", "--reset"),
    ("projects set reset", "--reset"),
    ("projects set trackers", "--tracker"),
    ("projects seed keep", "--keep"),
    ("projects seed cut", "--cut"),
    ("slate read values", "--values"),
    ("slate state start", "--start"),
];

fn line_options(v: &LineVerb<'_>) -> FlagTable {
    v.options().clone()
}

#[test]
fn the_served_tools_are_the_verb_table_one_per_entry_under_the_naming_rule() {
    for cloud in [false, true] {
        let table = verbs_for(cloud);
        let names: Vec<String> = table.iter().filter(|v| has_tool(v).is_some()).map(|v| tool_name(v.name())).collect();
        let unique: BTreeSet<&String> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "cloud {cloud}: two entries serve one tool name: {names:?}");
        for v in &table {
            if has_tool(v).is_some() {
                assert_eq!(tool_name(v.name()), v.name().replace(' ', "_"));
            }
        }
        assert_eq!(tool_name("run"), "run");
    }
}

#[test]
fn every_served_tool_carries_output_fields_so_a_verb_cannot_ship_without_saying_what_it_answers() {
    for cloud in [false, true] {
        for v in verbs_for(cloud) {
            if let Some(tool) = has_tool(&v) {
                assert!(!tool.output.is_empty(), "{} serves no output fields", v.name());
            }
        }
    }
}

#[test]
fn every_flag_a_verb_reads_has_a_line_of_its_own_in_its_help_and_no_line_is_written_for_a_flag_nobody_reads() {
    // A verb page that named ten flags on its usage line and then documented the three every verb takes left the
    // other seven to be guessed at. Each flag is a row, and the rows are held to the flags.
    let mut keys: BTreeSet<String> = flag_words().into_iter().map(|(k, _)| k.to_owned()).collect();
    for cloud in [false, true] {
        let table = verbs_for(cloud);
        for v in cli_verbs(&table) {
            let page = verb_page(v, "a host on another computer", cloud);
            for name in own_flags_of(v) {
                let says = flag_says(v.name(), name, cloud);
                assert!(says.as_deref().is_some_and(|s| s.chars().any(|c| !c.is_whitespace())), "wsp {} --{name} has a line", v.name());
                assert!(page.contains(&format!("--{name}")), "wsp {} --help names --{name}", v.name());
            }
        }
    }
    // A cloud verb's lines stand with the cloud off too: the verb is still in the table, only not answered.
    let all = all_verbs();
    for v in cli_verbs(&all) {
        for name in own_flags_of(v) {
            keys.remove(&format!("{} {name}", v.name()));
            keys.remove(name);
        }
    }
    assert_eq!(keys, BTreeSet::new(), "lines written for flags no verb reads");
}

#[test]
fn every_verb_that_takes_something_away_reads_yes_the_same_way_the_flag_its_usage_and_its_one_help_line() {
    let takes_away = Regex::new(r"(^| )(remove|delete|forget|discard)$").unwrap();
    let yes_words = flag_words().into_iter().find(|(k, _)| *k == "yes").unwrap().1;
    for cloud in [false, true] {
        let table = verbs_for(cloud);
        let lines: Vec<LineVerb<'_>> = cli_verbs(&table).into_iter().filter(|v| takes_away.is_match(v.name())).collect();
        let mut names: Vec<&str> = lines.iter().map(|v| v.name()).collect();
        names.sort();
        let mut expected = vec![
            "delete",
            "discard",
            "projects remove",
            "recipes remove",
            "servers remove",
            "skills remove",
            "thread forget",
            "worktree remove",
        ];
        if cloud {
            expected.push("forget");
        }
        expected.sort();
        assert_eq!(names, expected, "cloud {cloud}");
        for v in lines {
            let yes = line_options(&v).get("yes").cloned();
            assert_eq!(yes.map(|r| r.kind), Some(FlagKind::Boolean), "wsp {} reads --yes", v.name());
            assert!(v.usage().contains("[--yes]"), "wsp {} shows --yes", v.name());
            assert_eq!(flag_says(v.name(), "yes", cloud).as_deref(), Some(yes_words.as_str()), "wsp {} --yes says the one line", v.name());
        }
    }
}

#[test]
fn every_verb_s_help_page_fits_the_width_its_rows_keep_and_spells_json_one_way() {
    // A usage line wrapped to two spaces and then opened with `usage: ` stood five columns wider than the rows under
    // it, and a verb whose flags were one unbreakable bracket group ran to 147.
    for cloud in [false, true] {
        let table = verbs_for(cloud);
        for v in cli_verbs(&table) {
            let page = verb_page(v, common_flag_words::HOST, cloud);
            for line in page.split('\n') {
                assert!(line.chars().count() <= HELP_WIDTH, "wsp {} --help: {line}", v.name());
            }
        }
        let run = cli_verbs(&table).into_iter().find(|v| v.name() == "run").unwrap();
        let squeezed = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(squeezed(&verb_page(run, common_flag_words::HOST, cloud)).contains(&squeezed(common_flag_words::JSON)));
    }
}

#[test]
fn every_list_a_tool_takes_is_a_flag_the_line_reads_again_or_a_line_that_reads_its_values_another_way() {
    for cloud in [false, true] {
        let table = verbs_for(cloud);
        let mut carried = BTreeSet::new();
        for v in &table {
            let (Some(tool), Some(line)) = (has_tool(v), v.line()) else { continue };
            let array_of_words = |i: &&wsp_cli::verbs::client::ToolInput, key: &str| {
                matches!(i.kind, InputKind::StringList { .. })
                    || (matches!(i.kind, InputKind::Json) && LISTS_ON_THE_COMMAND_LINE.iter().any(|(k, _)| *k == key))
            };
            for input in tool.input.iter().filter(|i| array_of_words(i, &format!("{} {}", v.name(), i.name))) {
                let key = format!("{} {}", v.name(), input.name);
                let how = LISTS_ON_THE_COMMAND_LINE.iter().find(|(k, _)| *k == key).map(|(_, h)| *h);
                let how = how.unwrap_or_else(|| {
                    panic!("{} takes {} as a list and nothing says how wsp {} carries it", tool_name(v.name()), input.name, v.name())
                });
                carried.insert(key);
                if let Some(flag) = how.strip_prefix("--") {
                    let row = line.options().get(flag).unwrap_or_else(|| panic!("wsp {} does not read {how}", v.name()));
                    assert!(row.multiple, "wsp {} reads {how} as one value where the tool takes a list", v.name());
                }
            }
        }
        let names: BTreeSet<&str> = table.iter().map(Verb::name).collect();
        let answered: BTreeSet<String> = LISTS_ON_THE_COMMAND_LINE
            .iter()
            .map(|(k, _)| (*k).to_owned())
            .filter(|k| {
                names.iter().any(|n| {
                    k.starts_with(&format!("{n} ")) && table.iter().any(|v| v.name() == *n && v.line().is_some() && has_tool(v).is_some())
                })
            })
            .collect();
        assert_eq!(carried, answered, "cloud {cloud}");
    }
}

#[test]
fn a_notify_typed_once_and_typed_again_both_reach_the_line_as_a_list_of_targets() {
    let table = verbs_for(false);
    let run = cli_verbs(&table).into_iter().find(|v| v.name() == "run").unwrap();
    let mut options = common_flags();
    options.0.extend(run.options().0.iter().cloned());
    for (argv, targets) in [
        (vec!["--notify", "me", "build it"], vec!["me"]),
        (vec!["--notify", "me", "--notify", "1a2b3c4d", "build it"], vec!["me", "1a2b3c4d"]),
    ] {
        let argv: Vec<String> = argv.into_iter().map(str::to_owned).collect();
        let parsed = parse_args(&argv, &options).unwrap();
        assert_eq!(flag_list(&parsed.values, "notify"), targets, "{argv:?}");
        assert_eq!(parsed.positionals, vec!["build it".to_owned()]);
    }
}

#[test]
fn with_the_cloud_off_no_verb_tool_or_page_names_the_provider_or_keeps_a_cloud_mark() {
    // packages/host/AGENTS.md invariant 7: with the cloud off no page, tool or skill line names a cloud.
    let allowed = [
        r#""cloud","globe""#,
        r#""server","cloud","cpu""#,
        r#""enum":["cloud","local","place"]"#,
        "and on a cloud how many",
        "may run on that cloud at once",
        "that cloud may spend",
        "sub-thread's machine",
    ];
    let cloud_words = Regex::new(r"(?i)[^.\n]*\bclouds?\b[^.\n]*").unwrap();
    let table = verbs_for(false);
    let mut texts: Vec<(String, String)> = Vec::new();
    for v in &table {
        if let Some(line) = v.line() {
            texts.push((format!("wsp {} usage", v.name()), line.usage().to_owned()));
            texts.push((format!("wsp {} about", v.name()), line.about().to_owned()));
            texts.push((format!("wsp {} --help", v.name()), verb_page(line, common_flag_words::HOST, false)));
        }
        if let Some(tool) = has_tool(v) {
            texts.push((format!("{} description", tool_name(v.name())), tool.description.clone()));
            for input in &tool.input {
                if let Some(d) = &input.describe {
                    texts.push((format!("{} {}", tool_name(v.name()), input.name), cloud_text(d, false)));
                }
            }
        }
        assert!(!v.is_cloud(), "{} is a cloud verb served with the cloud off", v.name());
    }
    let mut said = Vec::new();
    for (where_, text) in &texts {
        assert!(!text.contains("Solari"), "{where_} names the provider");
        assert!(!text.contains("<!-- cloud -->") && !text.contains("<!-- /cloud -->"), "{where_} keeps a cloud mark");
        for m in cloud_words.find_iter(text) {
            let line = m.as_str().trim();
            if !allowed.iter().any(|a| line.contains(a)) {
                said.push(format!("{where_}: {line}"));
            }
        }
    }
    assert_eq!(said, Vec::<String>::new());
}

#[test]
fn a_cloud_verb_keeps_its_place_in_the_table_and_leaves_the_served_table_with_the_cloud_off() {
    let all: Vec<&'static str> = all_verbs().iter().map(Verb::name).collect();
    let off: Vec<&'static str> = verbs_for(false).iter().map(Verb::name).collect();
    let on: Vec<&'static str> = verbs_for(true).iter().map(Verb::name).collect();
    assert_eq!(on, all, "with the cloud on every verb is served, in ALL_VERBS' order");
    let gone: Vec<&str> = all.iter().filter(|n| !off.contains(n)).copied().collect();
    assert_eq!(gone, vec!["rename", "fork", "pause", "wake", "rebuild", "forget", "ssh", "export"]);
    let kept: Vec<&str> = all.iter().filter(|n| off.contains(n)).copied().collect();
    assert_eq!(kept, off, "the verbs left keep their order");
}
