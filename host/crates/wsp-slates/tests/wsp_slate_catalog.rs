// SPDX-License-Identifier: AGPL-3.0-only
//! The catalog: packages/protocol/test/slate/sketch.test.ts "the catalog", kit.test.ts and syntax.test.ts "the catalog's
//! patch entry" at 65f6b89c7, and the TS text itself: fixtures/wsp_slate_catalog_golden.json is the TS slateCatalog,
//! SLATE_EXAMPLES and SLATE_LIMITS read under node from packages/protocol/src/slate at 65f6b89c7.

use regex::Regex;
use serde_json::Value;
use wsp_slates::pending::slate::*;

const GOLDEN: &str = include_str!("fixtures/wsp_slate_catalog_golden.json");

fn golden() -> Value {
    serde_json::from_str(GOLDEN).expect("the golden fixture is JSON")
}

fn text_of<'a>(g: &'a Value, key: &str) -> &'a str {
    g[key].as_str().unwrap_or_else(|| panic!("golden {key} is a string"))
}

fn entry_text(name: &str) -> String {
    slate_catalog(Some(name))
}

fn piece_keys() -> Vec<String> {
    SLATE_PIECES.keys().map(|k| k.to_string()).collect()
}

fn source_keys() -> Vec<String> {
    SLATE_SOURCES.keys().map(|k| k.to_string()).collect()
}

#[test]
fn the_index_keeps_under_its_budget_and_says_what_the_catalog_says_it_does() {
    let index = slate_catalog(None);
    assert!(slate_tokens(&index) < SLATE_LIMITS.catalog_index_tokens, "{} tokens", slate_tokens(&index));
    for said in ["short labels, no emoji.", "in sentence case, never bold text.", "Show a run's json fields, never its raw out; times through date(), time() or ago()."] {
        assert!(index.contains(said), "{said}");
    }
    assert!(index.contains("One in a file stays there for the run to read, never you."));
    assert!(!Regex::new(r"(?i)Zingzy|repos/[a-z]+/wsp").unwrap().is_match(&index));
    assert!(entry_text("runs").contains("one in a file, the command reads itself, never through you."));
    assert!(index.contains("A button asked for where the project has no UI goes here: the slate is its UI, and the reply says so."));
    assert!(entry_text("chart").contains("in this computer's time zone"));
    assert!(index.contains(r#"held is a sentence that disables: bind it to a condition; "" or false enables."#));
    assert!(entry_text("patch").contains("version, which every write and read answers, counts the slate's edits and is what if_version takes"));
    assert!(entry_text("patch").contains("check with press walks a press and its <when> chain on a copy: nothing runs and nothing is sent"));
    assert!(entry_text("patch").contains(r#"check with values {"$run.json": {...}, "$run.exit": 0} previews the panel for a result the run has not had yet."#));
    assert!(entry_text("output").contains("how many of the newest lines show, out then err, 8 by default"));
    assert!(entry_text("runs").contains("while the Slate tab is on screen; at once when it comes back."));
    assert!(entry_text("runs").contains("else its text parsed; out is its text;"));
    assert!(entry_text("functions").contains(r#"Times read in this computer's zone: date(t) "Mon 5 Oct", time(t) "14:05", ago(t) "30s ago", until(t) "in 4m"."#));
    assert!(index.contains("then tool resource description>"));
    assert!(index.contains("with operators, a ? b : c, [lists], {records} and functions, never methods or =>."));
    assert_eq!(entry_text("pipes"), entry_text("steps"));
    assert_eq!(entry_text("tool"), entry_text("runs"));
    assert!(entry_text("file").contains(r#"The <file name="x.py"> declaration, code a run calls as $SLATE_DIR/x.py, is in slate_catalog runs."#));
    assert!(index.contains(r#"ago(t) "30s ago", until(t) "in 4m"."#));
    assert!(index.contains("time is a chart, x in ms or ISO;"));
    assert!(index.contains(SLATE_INDEX_EXAMPLE));
    assert!(parse_slate(SLATE_INDEX_EXAMPLE).errors.is_empty());
}

#[test]
fn every_piece_answers_under_200_tokens_and_every_source_under_300() {
    for p in piece_keys() {
        let tokens = slate_tokens(&entry_text(&p));
        assert!(tokens < SLATE_LIMITS.catalog_entry_tokens, "{p}: {tokens} tokens");
    }
    for s in source_keys() {
        let tokens = slate_tokens(&entry_text(&s));
        assert!(tokens < 300, "{s}: {tokens} tokens");
    }
}

#[test]
fn runs_functions_steps_handlers_and_examples_stay_within_their_budgets() {
    assert!(slate_tokens(&entry_text("runs")) < 810);
    for n in ["runs", "chart", "thread", "functions", "steps", "handlers", "patch", "examples"] {
        assert!(entry_text(n).ends_with("\nThe pieces and rules: slate_catalog with no name."), "{n}");
    }
    assert!(!slate_catalog(None).contains("slate_catalog with no name"));
    assert!(!entry_text("nope").contains("slate_catalog with no name"));
    let dashboards = entry_text("dashboards");
    assert!(Regex::new(r"(?s)^dashboards is not in the catalog\. .* The whole index:\n\nSlate kit wsp/2").unwrap().is_match(&dashboards));
    assert!(dashboards.contains(&slate_catalog(None)));
    let runs = entry_text("runs");
    assert!(runs.contains("once: a start while it still runs is skipped; without once it stops and starts again."));
    assert!(runs.contains("at the write for an every= run, at the first press or <when> for the rest. Until then it reads held; allowing it starts it at once."));
    assert!(slate_catalog(None).contains("bigger: resend the whole slate with if_version."));
    assert!(entry_text("diagram").contains("Under about 15 nodes"));
    for n in ["functions", "steps", "handlers"] {
        assert!(slate_tokens(&entry_text(n)) < 600, "{n}");
    }
    assert!(slate_tokens(&entry_text("examples")) < 3500);
    for e in SLATE_EXAMPLES {
        assert!(parse_slate(e.text).errors.is_empty(), "{}", e.title);
    }
}

#[test]
fn the_runs_entry_gives_each_attribute_a_line_and_says_what_shown_stale_and_a_restart_mean() {
    let runs = entry_text("runs");
    let lines: Vec<&str> = runs.split('\n').collect();
    for attr in ["name", "cmd", "env", "every", "always", "once", "timeout", "on", "confirm", "then", "tool"] {
        assert!(lines.iter().any(|l| l.starts_with(&format!("{attr}:")) || l.starts_with(&format!("{attr}="))), "{attr}");
    }
    assert!(runs.contains("Before a first result or after a failure, out and json are empty and len of them 0: show a figure with when={$run.exit == 0}."));
    assert!(runs.contains("stale: the command changed since this result."));
    assert!(runs.contains("<value> state and each run's last result outlive an app or host restart."));
}

#[test]
fn the_index_gives_each_core_piece_its_enum_values_and_its_required_marks() {
    let index = slate_catalog(None);
    let lines: Vec<&str> = index.split('\n').collect();
    for p in SLATE_PIECES.values().filter(|p| p.level == "core") {
        let line = lines.iter().find(|l| l.starts_with(&format!("{}: ", p.kind))).unwrap_or_else(|| panic!("{} has no index line", p.kind));
        for (k, spec) in &p.props {
            if p.items.values().any(|i| i.prop == *k) || *k == "pad" || *k == "surface" {
                continue;
            }
            let values = match spec.prop_type {
                SlatePropType::Enum(words) if !["tone", "emphasis", "align"].contains(k) && !slate_takes_figures(spec) => format!("({})", words.join("|")),
                _ => String::new(),
            };
            let token = format!("{k}{}{values}", if spec.required { "!" } else { "" });
            let pattern = Regex::new(&format!("(^|[ ;]){}( |;|$)", regex::escape(&token))).unwrap();
            assert!(pattern.is_match(line), "{}.{k}: {token} not in {line}", p.kind);
        }
    }
}

#[test]
fn a_wrong_name_names_the_nearest_entry() {
    assert!(entry_text("metre").starts_with("metre is not in the catalog; did you mean meter?"));
}

#[test]
fn every_piece_entry_has_its_purpose_its_sketch_and_its_example() {
    for (name, p) in SLATE_PIECES.iter() {
        let text = entry_text(name);
        assert!(text.contains(p.purpose), "{name}");
        assert!(text.contains("\nsketch: "), "{name}");
        assert!(text.contains(&format!("\nexample: {}", p.example)), "{name}");
    }
}

#[test]
fn the_catalog_patch_entry_shows_only_elements_that_apply_and_rule_six_points_at_it() {
    let mut d = parse_slate(r#"<slate><column id="root"><column id="list"><text id="eta">x</text><text id="price">p</text></column><column id="other"><text id="o">o</text></column></column></slate>"#)
        .document
        .expect("the base slate");
    let shown: Vec<String> = slate_catalog(Some("patch"))
        .split('\n')
        .skip(1)
        .filter(|l| l.contains(": "))
        .map(|l| l[..l.find(": ").unwrap()].to_string())
        .filter(|l| l.starts_with('<') && !l.contains("...") && !l.contains(", "))
        .collect();
    assert!(shown.len() > 4, "{shown:?}");
    let mut patches = shown.clone();
    patches.push(r#"<value name="hist" start={[]} />"#.to_string());
    patches.push(r#"<remove name="hist" />"#.to_string());
    for p in &patches {
        let parsed = parse_slate_patch(p, Some(&d));
        assert!(parsed.errors.is_empty(), "{p}: {:?}", parsed.errors);
        let ops = parsed.ops.expect("ops");
        let applied = apply_slate_patch(Some(&d), &slate_start_values(&d), &ops, &parsed.lines);
        assert!(applied.errors.is_empty(), "{p}: {:?}", applied.errors);
        d = applied.document.flatten().expect("a document");
    }
    assert!(slate_catalog(None).contains("runs, patch, functions"));
}

#[test]
fn every_worked_example_parses_and_validates_with_no_error() {
    assert_eq!(SLATE_EXAMPLES.len(), 5);
    for e in SLATE_EXAMPLES {
        let checked = parse_slate(e.text);
        assert!(checked.errors.is_empty(), "{}: {:?}", e.title, checked.errors);
        assert!(checked.document.is_some(), "{}", e.title);
    }
}

#[test]
fn the_index_and_every_named_entry_match_the_ts_text_byte_for_byte() {
    let g = golden();
    assert_eq!(slate_catalog(None), text_of(&g, "index"));
    let mut differing = Vec::new();
    for (name, want) in g["named"].as_object().expect("named entries") {
        if slate_catalog(Some(name.as_str())) != want.as_str().unwrap() {
            differing.push(name.clone());
        }
    }
    assert!(differing.is_empty(), "entries that differ from the TS: {differing:?}");
}

#[test]
fn the_token_estimate_matches_the_ts_on_its_own_inputs_and_on_every_entry() {
    let g = golden();
    for (text, want) in g["tokens"].as_object().expect("token cases") {
        assert_eq!(slate_tokens(text), want.as_u64().unwrap() as usize, "{text:?}");
    }
    for (name, want) in g["tokens_of"].as_object().expect("token counts of entries") {
        let text = if name == "(index)" { slate_catalog(None) } else { slate_catalog(Some(name.as_str())) };
        assert_eq!(slate_tokens(&text), want.as_u64().unwrap() as usize, "{name}");
    }
}

#[test]
fn the_rules_chapters_index_example_and_examples_match_the_ts() {
    let g = golden();
    let rules: Vec<&str> = g["rules"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(SLATE_RULES, rules.as_slice());
    let chapters: Vec<&str> = g["chapters"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(SLATE_CHAPTERS, chapters.as_slice());
    assert_eq!(SLATE_INDEX_EXAMPLE, text_of(&g, "index_example"));
    let examples = g["examples"].as_array().unwrap();
    assert_eq!(SLATE_EXAMPLES.len(), examples.len());
    for (e, want) in SLATE_EXAMPLES.iter().zip(examples) {
        assert_eq!(e.title, want["title"].as_str().unwrap());
        assert_eq!(e.text, want["text"].as_str().unwrap(), "{}", e.title);
    }
}

#[test]
fn the_limits_the_catalog_reads_match_the_ts() {
    let limits = &golden()["limits"];
    assert_eq!(SLATE_LIMITS.catalog_index_tokens as u64, limits["catalog_index_tokens"].as_u64().unwrap());
    assert_eq!(SLATE_LIMITS.catalog_entry_tokens as u64, limits["catalog_entry_tokens"].as_u64().unwrap());
    assert_eq!(SLATE_LIMITS.steps_per_reaction as u64, limits["steps_per_reaction"].as_u64().unwrap());
    assert_eq!(SLATE_LIMITS.timer_floor_s, limits["timer_floor_s"].as_i64().unwrap());
    assert_eq!(SLATE_LIMITS.stream_lines as u64, limits["stream_lines"].as_u64().unwrap());
    assert_eq!(SLATE_LIMITS.timeout_max_s, limits["timeout_max_s"].as_i64().unwrap());
}

#[test]
fn only_the_figure_enums_are_taken_as_figures_in_the_index() {
    let g = golden();
    for row in g["takes_figures"].as_array().unwrap() {
        let (piece, prop, want) = (row[0].as_str().unwrap(), row[1].as_str().unwrap(), row[2].as_bool().unwrap());
        let spec = SLATE_PIECES.get(piece).and_then(|p| p.props.get(prop)).unwrap_or_else(|| panic!("{piece}.{prop}"));
        assert_eq!(slate_takes_figures(spec), want, "{piece}.{prop}");
    }
}
