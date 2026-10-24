// SPDX-License-Identifier: AGPL-3.0-only
use serde_json::json;

use super::*;

#[test]
fn a_prop_turns_back_into_the_formula_language() {
    assert_eq!(expr_of(&json!({"bind": "$presses + 1"})), "($presses + 1)");
    assert_eq!(expr_of(&json!({"format": "v${$n} `x`"})), "`v${$n} \\`x\\``");
    assert_eq!(expr_of(&json!([1, "a\"b", null, {"k": true}])), r#"[1, "a\"b", null, {"k": true}]"#);
    assert_eq!(expr_of(&json!({"bind": "item.n"})), "(item.n)");
    assert!(!is_live(&json!({"bind": 3})));
    let facts = json!([{"label": "A", "value": {"bind": "$a"}}, {"label": "B", "value": {"format": "${$b} s"}}]);
    assert_eq!(leaves(&facts), ["($a)", "`${$b} s`"], "each binding is its own formula, so a long list stays under the cap");
    let answered: HashMap<String, Value> = [("($a)".to_owned(), json!(1)), ("`${$b} s`".to_owned(), json!("2 s"))].into_iter().collect();
    assert_eq!(filled(&facts, &answered), Some(json!([{"label": "A", "value": 1}, {"label": "B", "value": "2 s"}])));
    assert_eq!(filled(&facts, &HashMap::new()), None);
    assert!(!is_live(&json!({"label": "x", "on": {"press": [{"do": "set", "path": "$n", "value": {"bind": "$n + 1"}}]}})));
    assert!(is_live(&json!([{"title": "x", "value": {"bind": "item.name"}}])));
}

#[test]
fn rows_ask_each_written_element_and_read_each_row_back() {
    let piece = Piece::read(
        "table-1",
        &json!({"type": "table", "props": {"items": [{"bind": "{ n: 1 }"}], "key": {"bind": "item.n"},
            "columns": [{"title": "N", "value": {"bind": "item.n"}, "mono": true}]}}),
    )
    .unwrap();
    let rows = Rows::of(&piece).unwrap();
    assert_eq!(rows.formulas(), ["[({ n: 1 })]", "[({ n: 1 })] | map(_slate0: (item.n), _slate1: (item.n))"]);
    let answered: HashMap<String, Value> = [
        ("[({ n: 1 })]".to_owned(), json!([{"n": 1}])),
        ("[({ n: 1 })] | map(_slate0: (item.n), _slate1: (item.n))".to_owned(), json!([{"n": 1, "_slate0": 1, "_slate1": 1}])),
    ]
    .into_iter()
    .collect();
    let read = rows.read(&answered).unwrap();
    assert_eq!(read[0].item, json!({"n": 1}));
    assert_eq!(read[0].props["columns"], json!([{"title": "N", "value": 1, "mono": true}]));
    assert_eq!(read[0].props["key"], json!(1));
    assert!(rows.read(&HashMap::new()).is_none(), "rows wait for the host's answer");
    let acted = Piece::read(
        "table-2",
        &json!({"type": "table", "props": {"items": {"bind": "$rows"},
            "rowActions": [{"label": "Count", "when": "item.bad", "on": {"press": [{"do": "set", "path": "$n", "value": {"bind": "$n + 1"}}]}}]}}),
    )
    .unwrap();
    let rows = Rows::of(&acted).unwrap();
    assert_eq!(
        rows.formulas(),
        ["($rows)", "($rows) | map(_slate0: (item.bad))"],
        "a row action's own when is read per row, its handler is not"
    );
    let answered: HashMap<String, Value> = [
        ("($rows)".to_owned(), json!([{"bad": true}])),
        ("($rows) | map(_slate0: (item.bad))".to_owned(), json!([{"bad": true, "_slate0": true}])),
    ]
    .into_iter()
    .collect();
    let read = rows.read(&answered).unwrap();
    assert_eq!(read[0].props["rowActions"][0]["when"], json!(true));
    assert_eq!(read[0].props["rowActions"][0]["on"]["press"][0]["value"], json!({"bind": "$n + 1"}));
}

#[test]
fn no_formula_passes_the_hosts_cap() {
    let long = "x".repeat(300);
    let columns: Vec<Value> = (0..6).map(|i| json!({"title": format!("c{i}"), "value": {"bind": format!("item.{long}{i}")}})).collect();
    let piece = Piece::read("t", &json!({"type": "table", "props": {"items": {"bind": "$rows"}, "columns": columns}})).unwrap();
    let rows = Rows::of(&piece).unwrap();
    let formulas = rows.formulas();
    assert_eq!(formulas.len(), 7, "the items, then one formula per field once two no longer fit: {formulas:?}");
    assert!(formulas.iter().all(|f| f.len() <= EXPR_CHARS));
    let points: Vec<Value> =
        (0..400).map(|i| json!({"bind": format!("{{ at: {}, v: {} }}", 1_792_281_600_000u64 + i * 60_000, 2400 + i % 7)})).collect();
    let chart =
        Piece::read("c", &json!({"type": "chart", "props": {"items": points, "x": {"bind": "item.at"}, "value": {"bind": "item.v"}}}))
            .unwrap();
    let formulas = Rows::of(&chart).unwrap().formulas();
    assert!(formulas.iter().all(|f| f.len() <= EXPR_CHARS), "a written list is cut into runs under the cap");
    assert!(formulas.len() < 150, "runs of elements, not one formula per element: {}", formulas.len());
}

#[test]
fn a_run_reads_index_from_where_it_starts_and_nothing_else_moves() {
    let at7 = |e: &str| shifted(e, 7);
    assert_eq!(at7("(index)"), "((index + 7))");
    assert_eq!(at7("(index * 2 + item.index)"), "((index + 7) * 2 + item.index)");
    assert_eq!(at7(r#"("index" + str(index))"#), r#"("index" + str((index + 7)))"#);
    assert_eq!(at7("`index ${index} $${index} \\` ${ `${index}` }`"), "`index ${(index + 7)} $${index} \\` ${ `${(index + 7)}` }`");
    assert_eq!(at7("($index + index)"), "($index + (index + 7))");
    assert_eq!(at7(r#"({ index: index, "index": 1 }).index"#), r#"({ index: (index + 7), "index": 1 }).index"#);
    assert_eq!(at7("($c ? index : index + 1)"), "($c ? (index + 7) : (index + 7) + 1)");
    assert_eq!(at7("(index.x == null and index[0] == null)"), "((index + 7).x == null and (index + 7)[0] == null)");
    assert_eq!(at7("(index||index&&index)"), "((index + 7)||(index + 7)&&(index + 7))");
    assert_eq!(at7("(1.5e3+index)"), "(1.5e3+(index + 7))");
    assert_eq!(at7("(at($l, index) | first)"), "(at($l, (index + 7)) | first)");
    // A nested step reads its own index per element, but sortBy's order and join's list where the step stands.
    let nested = "(item.kids | where(index > 0) | map(index: index) | pick(index) | flatten(index) | count)";
    assert_eq!(at7(nested), nested);
    assert_eq!(
        at7(r#"(item.kids | sortBy(item.v, index > 7 ? "desc" : "asc") | join($l[index], id) | first)"#),
        r#"(item.kids | sortBy(item.v, (index + 7) > 7 ? "desc" : "asc") | join($l[(index + 7)], id) | first)"#
    );
    assert_eq!(
        at7("([index, index] | map(n: item + index) | format(`${item.n}:${index}`))"),
        "([(index + 7), (index + 7)] | map(n: item + index) | format(`${item.n}:${index}`))"
    );
    for same in ["(orElse(index(), 3))", "(index_ + 1) == null", "(item . index)", "('index unclosed)", "(indexes)"] {
        assert_eq!(at7(same), same);
    }
    assert_eq!(shifted("(index)", 0), "(index)");
}

#[test]
fn a_written_list_past_one_run_counts_its_rows_from_the_start() {
    let items: Vec<Value> = (0..60).map(|i| json!({"name": format!("row-{i:04}")})).collect();
    let piece =
        Piece::read("t", &json!({"type": "table", "props": {"items": items, "columns": [{"title": "N", "value": {"bind": "index"}}]}}))
            .unwrap();
    let rows = Rows::of(&piece).unwrap();
    let Items::Each(runs) = &rows.items else { panic!("a written list goes in runs") };
    assert!(runs.len() >= 2, "long enough for two runs: {}", runs.len());
    let mut answered = HashMap::new();
    let mut from = 0;
    for (run, fields) in runs.iter().map(|r| &r.formula).zip(&rows.fields) {
        let len = run.matches("row-").count();
        let want = if from == 0 { "(index)".to_owned() } else { format!("((index + {from}))") };
        assert_eq!(fields[0].0, format!("{run} | map(_slate0: {want})"));
        answered.insert(run.clone(), Value::Array((from..from + len).map(|i| json!({"name": format!("row-{i:04}")})).collect()));
        answered.insert(fields[0].0.clone(), Value::Array((from..from + len).map(|i| json!({"_slate0": i})).collect()));
        from += len;
    }
    let read = rows.read(&answered).unwrap();
    assert_eq!(read.len(), 60);
    assert!(read.iter().all(|r| r.props["columns"][0]["value"] == json!(r.index)));
}

#[test]
fn a_format_past_the_cap_asks_each_hole_and_joins_the_words_here() {
    let words = "w".repeat(520);
    let format = json!({ "format": format!("{words} ${{$n}} and $${{not}} ${{$m}}!") });
    assert_eq!(leaves(&format), ["`${$n}`", "`${$m}`"], "each hole is its own formula, under the cap");
    let answered =
        |n: Value, m: Value| -> HashMap<String, Value> { [("`${$n}`".to_owned(), n), ("`${$m}`".to_owned(), m)].into_iter().collect() };
    assert_eq!(filled(&format, &answered(json!("5"), json!("x"))), Some(json!(format!("{words} 5 and ${{not}} x!"))));
    assert_eq!(filled(&format, &answered(json!("5"), Value::Null)), Some(json!(format!("{words} 5 and ${{not}} !"))));
    assert_eq!(filled(&format, &answered(Value::Null, Value::Null)), Some(Value::Null), "every hole read nothing: null, as joined");
    assert_eq!(filled(&format, &HashMap::new()), None, "unanswered until each hole is");
    let short = json!({ "format": "a ${$n}" });
    assert_eq!(leaves(&short), ["`a ${$n}`"], "a format that fits is still one formula");
    let table =
        Piece::read("t", &json!({"type": "table", "props": {"items": {"bind": "$rows"}, "columns": [{"title": "N", "value": format}]}}))
            .unwrap();
    let rows = Rows::of(&table).unwrap();
    assert_eq!(rows.formulas(), ["($rows)", "($rows) | map(_slate0: `${$n}`, _slate1: `${$m}`)"]);
    let answered: HashMap<String, Value> = [
        ("($rows)".to_owned(), json!([{}])),
        ("($rows) | map(_slate0: `${$n}`, _slate1: `${$m}`)".to_owned(), json!([{"_slate0": "1", "_slate1": "2"}])),
    ]
    .into_iter()
    .collect();
    assert_eq!(rows.read(&answered).unwrap()[0].props["columns"][0]["value"], json!(format!("{words} 1 and ${{not}} 2!")));
}

#[test]
fn a_written_element_too_long_for_its_brackets_keeps_its_row_and_the_rows_after() {
    let long = format!("{{ name: \"{}\" }}", "x".repeat(485));
    let mut items: Vec<Value> = (0..3).map(|i| json!({"bind": format!("{{ name: \"row-{i}\" }}")})).collect();
    items.insert(1, json!({ "bind": long }));
    let piece =
        Piece::read("t", &json!({"type": "table", "props": {"items": items, "columns": [{"title": "N", "value": {"bind": "index"}}]}}))
            .unwrap();
    let rows = Rows::of(&piece).unwrap();
    let formulas = rows.formulas();
    assert!(formulas.iter().all(|f| f.len() <= EXPR_CHARS), "{formulas:?}");
    assert!(formulas.iter().any(|f| f.starts_with("({ name") && f.len() == 499), "the long element is asked bare: {formulas:?}");
    let mut answered: HashMap<String, Value> = HashMap::new();
    for f in &formulas {
        let count = f.matches("row-").count();
        let answer = if f.contains("| map(") {
            let from = f.split("index + ").nth(1).and_then(|r| r.split(')').next()).and_then(|n| n.parse().ok()).unwrap_or(0);
            Value::Array((from..from + count).map(|i| json!({ "_slate0": i })).collect())
        } else if f.contains("xxxx") {
            json!({ "name": "long" })
        } else {
            Value::Array((0..count).map(|_| json!({ "name": "short" })).collect())
        };
        answered.insert(f.clone(), answer);
    }
    let read = rows.read(&answered).unwrap();
    assert_eq!(read.iter().map(|r| r.index).collect::<Vec<_>>(), [0, 1, 2, 3], "the long element keeps its row");
    assert_eq!(read[1].item, json!({ "name": "long" }));
    assert_eq!(read[2].props["columns"][0]["value"], json!(2), "and the next row reads its own index");
    assert_eq!(read[1].props["columns"][0]["value"], Value::Null, "its own field does not fit beside it and reads null");
    let first = formulas.iter().find(|f| f.starts_with('[') && !f.contains("| map(")).unwrap();
    answered.insert(first.clone(), Value::Null);
    let read = rows.read(&answered).unwrap();
    assert_eq!(read.iter().map(|r| r.index).collect::<Vec<_>>(), [0, 1, 2, 3], "a run the host refused keeps its rows too");
}

#[test]
fn a_source_is_read_by_its_root_name_alone() {
    assert!(reads_source("git.branch", "git") && reads_source("(pr[0].state)", "pr") && reads_source("$x + thread.turns", "thread"));
    assert!(!reads_source("$git.branch", "git") && !reads_source("item.pr", "pr") && !reads_source("github", "git"));
    let derived: Map<String, Value> = serde_json::from_value(json!({"b": "git.branch", "c": "$b + 1"})).unwrap();
    assert_eq!(sources(&["($c)".to_owned(), "(usage.week)".to_owned()], &derived), ["git", "usage"]);
}

#[test]
fn the_day_and_the_zone_tick_each_thirty_seconds_and_the_time_each_second() {
    let none = Map::new();
    let tick = |f: &str| super::tick(&[f.to_owned()], &none);
    assert_eq!(tick("(time.now - 5)"), Some(Duration::from_secs(1)));
    assert_eq!(tick("(ago($at))"), Some(Duration::from_secs(1)));
    assert_eq!(tick("(time.today)"), Some(Duration::from_secs(30)));
    assert_eq!(tick("(time.zone)"), Some(Duration::from_secs(30)));
    assert_eq!(tick("($time.today)"), None);
}

#[test]
fn a_source_named_in_words_or_strings_is_not_read() {
    let none = Map::new();
    let read = |f: &str| super::sources(&[f.to_owned()], &none);
    assert_eq!(read("`This thread has ${$n} files`"), Vec::<&str>::new());
    assert_eq!(read("(\"run git status\" + 'open a pr')"), Vec::<&str>::new());
    assert_eq!(super::tick(&["`at time.now, ago(1)`".to_owned()], &none), None);
    assert_eq!(read("`on ${git.branch} in ${`pr ${pr.number}`}`"), ["git", "pr"]);
    assert_eq!(read("(thread.title + \"}\")"), ["thread"]);
}

#[test]
fn a_binding_too_long_for_its_parentheses_goes_bare() {
    let long = format!("$n + {}", "1 + ".repeat(123) + "11");
    assert_eq!(long.len(), 499);
    assert_eq!(expr_of(&json!({ "bind": long })), long);
    assert_eq!(expr_of(&json!({ "bind": "$n" })), "($n)");
}

#[test]
fn own_paths_write_where_they_point() {
    let mut r = Record::default();
    assert!(r.set_own("$steps[1].done", json!(true)));
    assert_eq!(r.values["steps"], json!([null, {"done": true}]));
    assert_eq!(r.get_own("$steps[1].done"), Some(&json!(true)));
    assert!(!r.set_own("steps", json!(1)));
}

#[test]
fn a_big_number_reads_back_as_the_same_number() {
    assert_eq!(literal(&json!(1e21)), "1e21");
    assert_eq!(literal(&json!(123_456_789_012_345_680_000.0)), "1.2345678901234568e20");
    assert_eq!(literal(&json!([1e-7, 42])), "[1e-7, 42]");
}

#[test]
fn a_format_keeps_its_holes_as_written() {
    let sent = |format: &str| expr_of(&json!({ "format": format }));
    assert_eq!(sent("${`${$n} items`}"), "`${`${$n} items`}`");
    assert_eq!(sent(r"a `b` ${$ok ? `x${$n}` : 'y'} c`"), r"`a \`b\` ${$ok ? `x${$n}` : 'y'} c\``");
    assert_eq!(sent(r"Saved in ${$dir}\"), r"`Saved in ${$dir}${'\\'}`");
    assert_eq!(sent(r"${$n}\\"), r"`${$n}\${'\\'}`");
    assert_eq!(sent("$${not} ${$n}"), "`$${not} ${$n}`");
}

#[test]
fn clock_reads_are_seen_and_names_like_them_are_not() {
    assert!(reads_clock("ago(item.at)"));
    assert!(reads_clock("time.now - 5"));
    assert!(!reads_clock("$agony + 1"));
    assert!(!reads_clock("$x.ago"));
}

#[test]
fn a_clock_read_through_a_derived_value_is_seen() {
    let derived: Map<String, Value> =
        serde_json::from_value(json!({"now": "time.now", "since": "ago($start)", "later": "$since", "plain": "$n + 1"})).unwrap();
    let asks = |f: &str| clocked(&[f.to_owned()], &derived);
    assert!(asks("($now)") && asks("($later * 2)") && asks("(time.now)"));
    assert!(!asks("($plain)") && !asks("($nowhere)"));
}
