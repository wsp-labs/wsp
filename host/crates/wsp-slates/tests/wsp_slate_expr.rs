// SPDX-License-Identifier: AGPL-3.0-only
//! The formula language of the slate module: packages/protocol/test/slate/expr.test.ts at 65f6b89c7, the cases that do
//! not need the validator, kit or syntax, and the expression-only cases of hostile.test.ts. The checker's scope stands
//! in for sources.ts's slateSourceType (not ported yet): it walks the pr and usage shapes as that function does.

use std::collections::HashMap;
use std::rc::Rc;

use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_slates::pending::slate::*;

const NOW: i64 = 1_791_115_200_000;

fn world() -> HashMap<String, Value> {
    let checks = json!([
        { "name": "build", "state": "pending", "startedAt": "2026-10-04T11:50:00Z", "completedAt": null },
        { "name": "lint", "state": "pass", "startedAt": "2026-10-04T11:50:00Z", "completedAt": "2026-10-04T11:51:04Z" },
        { "name": "test", "state": "fail", "startedAt": "2026-10-04T11:50:00Z", "completedAt": "2026-10-04T11:55:00Z" },
        { "name": "docs", "state": "pass", "startedAt": "2026-10-04T11:50:00Z", "completedAt": "2026-10-04T11:50:30Z" },
    ]);
    let entries = [
        ("thread.context.used", json!(164_000)),
        ("thread.context.window", json!(1_000_000)),
        ("thread.context.free", json!(836_000)),
        ("usage.week.percent", json!(46)),
        ("usage.week.resetsAt", json!(NOW + (3 * 24 + 7) * 3_600_000)),
        ("pr.checks", checks),
        ("pr.word", Value::Null),
        ("$check", json!({ "state": "done", "exit": 0, "out": "{\"id\":1234,\"title\":\"Round once\"}", "json": { "id": 1234, "title": "Round once" }, "runs": 1 })),
        ("$steps", json!([{ "title": "a", "done": true }, { "title": "b", "done": false }, { "title": "c", "done": true }])),
        (
            "processes.list",
            json!([{ "name": "a", "cpu": 1, "mem": 3, "pid": 1 }, { "name": "b", "cpu": 9, "mem": 2, "pid": 2 }, { "name": "c", "cpu": null, "mem": 1, "pid": 3 }, { "name": "d", "cpu": 5, "mem": 0, "pid": 4 }]),
        ),
    ];
    entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

fn ctx_over(values: HashMap<String, Value>, item: Option<Value>, index: Option<i64>, now: i64) -> SlateEvalContext {
    SlateEvalContext { now, item, index, resolve: Rc::new(move |p: &str| values.get(p).cloned()) }
}

fn ev(src: &str) -> Option<Value> {
    evaluate_slate_expression(src, &ctx_over(world(), None, None, NOW))
}

fn ev_item(src: &str, item: Value) -> Option<Value> {
    evaluate_slate_expression(src, &ctx_over(world(), Some(item), None, NOW))
}

fn check_ctx(src: &str, row: bool) -> Vec<String> {
    let scope = scope(row);
    check_slate_expression(src, &scope, 0).problems.into_iter().map(|p| p.code).collect()
}

// ---- the checker's scope: a stand-in for sources.ts's slateSourceType ----

enum Shape {
    Num,
    Text,
    Bool,
    Any,
    List(Box<Shape>),
    Rec(Vec<(&'static str, Shape)>),
}

fn source_shape(head: &str) -> Option<Shape> {
    let pr_check = Shape::Rec(vec![
        ("name", Shape::Text),
        ("workflow", Shape::Text),
        ("state", Shape::Text),
        ("link", Shape::Text),
        ("description", Shape::Text),
        ("startedAt", Shape::Any),
        ("completedAt", Shape::Any),
    ]);
    Some(match head {
        "pr" => Shape::Rec(vec![
            ("number", Shape::Num),
            ("url", Shape::Text),
            ("state", Shape::Text),
            ("draft", Shape::Bool),
            ("base", Shape::Text),
            ("branch", Shape::Text),
            ("headOid", Shape::Text),
            ("headSubject", Shape::Text),
            ("mergeable", Shape::Text),
            ("review", Shape::Text),
            ("checks", Shape::List(Box::new(pr_check))),
            ("additions", Shape::Num),
            ("deletions", Shape::Num),
            ("changedFiles", Shape::Num),
            ("commits", Shape::Num),
            ("behindBase", Shape::Num),
            ("author", Shape::Text),
            ("readAt", Shape::Any),
            ("word", Shape::Text),
            ("unread", Shape::Text),
        ]),
        "usage" => Shape::Rec(vec![
            ("account", Shape::Rec(vec![("key", Shape::Text), ("label", Shape::Text), ("agent", Shape::Text), ("plan", Shape::Text), ("address", Shape::Text), ("computers", Shape::List(Box::new(Shape::Text)))])),
            ("windows", Shape::List(Box::new(Shape::Rec(vec![("kind", Shape::Text), ("usedPercent", Shape::Num), ("resetsAt", Shape::Any)])))),
            ("session", Shape::Rec(vec![("percent", Shape::Num), ("resetsAt", Shape::Any)])),
            ("week", Shape::Rec(vec![("percent", Shape::Num), ("resetsAt", Shape::Any)])),
            ("status", Shape::Text),
            ("note", Shape::Text),
            ("burn", Shape::Rec(vec![("tokensPerMinute", Shape::Num), ("threads", Shape::Num)])),
            ("credits", Shape::Rec(vec![("count", Shape::Num), ("nextExpiresAt", Shape::Any)])),
            ("readAt", Shape::Any),
        ]),
        _ => return None,
    })
}

fn to_type(shape: &Shape) -> SlateType {
    let plain = |t: SlateTypeName| SlateType { t, of: None, fields: None };
    match shape {
        Shape::Num => plain(SlateTypeName::Number),
        Shape::Text => plain(SlateTypeName::String),
        Shape::Bool => plain(SlateTypeName::Boolean),
        Shape::Any => plain(SlateTypeName::Any),
        Shape::List(of) => SlateType { t: SlateTypeName::List, of: Some(Box::new(to_type(of))), fields: None },
        Shape::Rec(fields) => {
            let map: IndexMap<String, SlateType> = fields.iter().map(|(k, s)| (k.to_string(), to_type(s))).collect();
            SlateType { t: SlateTypeName::Record, of: None, fields: Some(map) }
        }
    }
}

/// slateSourceType's walk over one source's shape: a field it does not declare is X401 with the nearest declared name.
fn source_type(head: &str, segs: &[PathSeg]) -> Result<SlateType, SlateScopeProblem> {
    let Some(mut shape) = source_shape(head) else {
        return Err(SlateScopeProblem { code: "X401", message: format!("{head} is not a path"), fix: None });
    };
    let mut walked = head.to_string();
    for (at, seg) in segs.iter().enumerate() {
        match seg {
            PathSeg::Index(i) => match shape {
                Shape::List(next) => {
                    shape = *next;
                    walked.push_str(&format!("[{i}]"));
                }
                _ => return Ok(SlateType { t: SlateTypeName::Any, of: None, fields: None }),
            },
            PathSeg::Key(k) => {
                let Shape::Rec(fields) = shape else {
                    return Ok(SlateType { t: SlateTypeName::Any, of: None, fields: None });
                };
                let keys: Vec<String> = fields.iter().map(|(n, _)| n.to_string()).collect();
                let Some(pos) = fields.iter().position(|(n, _)| *n == k.as_str()) else {
                    let fix = slate_nearest(k, &keys);
                    let rest: String = segs[at + 1..]
                        .iter()
                        .map(|s| match s {
                            PathSeg::Key(x) => format!(".{x}"),
                            PathSeg::Index(i) => format!("[{i}]"),
                        })
                        .collect();
                    let fixed = fix.as_ref().map(|f| format!("{walked}.{f}{rest}"));
                    let message = match &fixed {
                        Some(f) => format!("{walked}.{k}{rest} is not a path. Did you mean {f}?"),
                        None => format!("{walked}.{k}{rest} is not a path; {walked} has {}", keys.join(", ")),
                    };
                    return Err(SlateScopeProblem { code: "X401", message, fix: fixed });
                };
                shape = fields.into_iter().nth(pos).map(|(_, s)| s).unwrap_or(Shape::Any);
                walked.push_str(&format!(".{k}"));
            }
        }
    }
    Ok(to_type(&shape))
}

fn scope(row: bool) -> SlateCheckScope {
    let path = Box::new(|head: &str, own: bool, segs: &[PathSeg]| {
        if own {
            let steps = SlateType {
                t: SlateTypeName::List,
                of: Some(Box::new(SlateType {
                    t: SlateTypeName::Record,
                    of: None,
                    fields: Some(IndexMap::from([
                        ("title".to_string(), SlateType { t: SlateTypeName::String, of: None, fields: None }),
                        ("done".to_string(), SlateType { t: SlateTypeName::Boolean, of: None, fields: None }),
                    ])),
                })),
                fields: None,
            };
            return Ok(if head == "steps" { steps } else { SlateType { t: SlateTypeName::Any, of: None, fields: None } });
        }
        source_type(head, segs)
    });
    let row = row.then(|| SlateType { t: SlateTypeName::Any, of: None, fields: None });
    SlateCheckScope { path, row }
}

// ---- expressions ----

#[test]
fn evaluates_the_examples_of_spec_04_to_the_values_its_table_gives() {
    assert_eq!(ev("percent(thread.context.used / thread.context.window)"), Some(json!("16%")));
    assert_eq!(ev("tokens(thread.context.free)"), Some(json!("836k")));
    assert_eq!(ev("usage.week.percent > 80 ? 'warning' : 'default'"), Some(json!("default")));
    assert_eq!(ev("until(usage.week.resetsAt)"), Some(json!("in 3d 7h")));
    assert_eq!(ev("len(pr.checks)"), Some(json!(4)));
    assert_eq!(ev("first(pr.checks).name"), Some(json!("build")));
    assert_eq!(ev_item("word(item.state)", json!({ "state": "pending" })), Some(json!("Running")));
    assert_eq!(ev("pr.checks | where(item.state == 'fail') | count"), Some(json!(1)));
    assert_eq!(ev("$check.state == 'done' && $check.json.id != null"), Some(json!(true)));
    assert_eq!(ev("(pr.checks | first).state"), Some(json!("pending")));
    assert_eq!(ev("json($check.out).title"), Some(json!("Round once")));
    assert_eq!(ev("orElse(pr.word, 'no pull request')"), Some(json!("no pull request")));
    assert_eq!(ev("$steps | where(item.done) | count"), Some(json!(2)));
    let lint = json!({ "name": "lint", "state": "pass", "startedAt": "2026-10-04T11:50:00Z", "completedAt": "2026-10-04T11:51:04Z" });
    assert_eq!(
        ev_item("item.completedAt ? duration(num(item.completedAt) - num(item.startedAt)) : 'running'", lint),
        Some(json!("1m 4s"))
    );
    assert_eq!(
        ev("processes.list | sortBy(item.cpu, 'desc') | take(2) | pick(name, cpu)"),
        Some(json!([{ "name": "b", "cpu": 9 }, { "name": "d", "cpu": 5 }]))
    );
}

#[test]
fn reads_javascript_spellings_as_the_word_forms() {
    assert_eq!(ev("!(1 === 1) || 2 !== 2"), Some(json!(false)));
    assert_eq!(ev("true && 'x'"), Some(json!("x")));
    assert_eq!(ev("`${len(pr.checks)} of ${4}`"), Some(json!("4 of 4")));
}

#[test]
fn runs_every_step_over_a_list() {
    assert_eq!(
        ev("pr.checks | groupBy(item.state) | map(n: item.count) | pick(key, n)"),
        Some(json!([{ "key": "pending", "n": 1 }, { "key": "pass", "n": 2 }, { "key": "fail", "n": 1 }]))
    );
    assert_eq!(ev("processes.list | sum(item.cpu)"), Some(json!(15)));
    assert_eq!(ev("processes.list | avg(item.mem)"), Some(json!(1.5)));
    assert_eq!(ev("processes.list | min(item.cpu)"), Some(json!(1)));
    assert_eq!(ev("processes.list | sortBy(item.cpu) | format(item.name)"), Some(json!(["a", "d", "b", "c"])));
    assert_eq!(ev("pr.checks | distinct(item.state) | count"), Some(json!(3)));
    assert_eq!(ev("pr.checks | skip(3) | last"), Some(world()["pr.checks"][3].clone()));
    assert_eq!(
        ev("pr.checks | where(item.state == 'pass') | join(processes.list, name)"),
        Some(json!([
            { "name": "lint", "state": "pass", "startedAt": "2026-10-04T11:50:00Z", "completedAt": "2026-10-04T11:51:04Z", "list": null },
            { "name": "docs", "state": "pass", "startedAt": "2026-10-04T11:50:00Z", "completedAt": "2026-10-04T11:50:30Z", "list": null },
        ]))
    );
    assert_eq!(ev("$steps | map(n: index) | flatten"), Some(json!([])));
}

#[test]
fn answers_null_for_a_failure_and_never_throws() {
    assert_eq!(ev("1 / 0"), Some(Value::Null));
    assert_eq!(ev("null + 1"), Some(Value::Null));
    assert_eq!(ev("null < 3"), Some(Value::Null));
    assert_eq!(ev("pr.missing.deeper"), None);
    assert_eq!(ev("pr.checks[9].name"), None);
    assert_eq!(ev("pr.checks[-1].name"), Some(json!("docs")));
    let x: HashMap<String, Value> = HashMap::from([("$x".to_string(), json!({ "a": 1 }))]);
    for name in ["__proto__", "constructor", "toString"] {
        assert_eq!(evaluate_slate_expression(&format!("$x.{name}"), &ctx_over(x.clone(), None, None, NOW)), None, "$x.{name}");
    }
    assert_eq!(evaluate_slate_expression("$x.a", &ctx_over(x, None, None, NOW)), Some(json!(1)));
}

#[test]
fn refuses_at_write_time_what_spec_04s_table_refuses() {
    assert_eq!(check_ctx("usage.weekley.percent", false), ["X401"]);
    let message = check_slate_expression("usage.weekley.percent", &scope(false), 0).problems[0].message.clone();
    assert_eq!(message, "usage.weekley.percent is not a path. Did you mean usage.week.percent?");
    assert_eq!(check_ctx("pr.checks.length", false), ["X420"]);
    assert_eq!(check_ctx("a < b < c", false), ["X400"]);
    assert_eq!(check_ctx("item.name", false), ["X409"]);
    assert_eq!(check_ctx("pr.checks | take($n)", false), ["Q425"]);
    let filter = check_slate_expression("pr.checks | filter(item.state == 'fail')", &scope(false), 0).problems.remove(0);
    assert_eq!((filter.code.as_str(), filter.fix.as_deref()), ("Q422", Some("where")));
    assert_eq!(check_ctx("pr.checks | where(item.state != 'pass') | sortBy(item.name) | pluck", false), ["Q422"]);
    assert_eq!(check_ctx("pr.checks == 'fail'", false), ["X408"]);
    assert_eq!(check_ctx("'a' + 1", false), ["X408"]);
    assert_eq!(check_ctx("items.map(i => i.name)", false), ["X420"]);
    assert_eq!(check_ctx("s.toUpperCase()", false), ["X420"]);
    assert_eq!(check_ctx("a ?? b", false), ["X420"]);
    assert!(check_ctx("[1, 2]", false).is_empty());
    assert_eq!(check_ctx("x = 1", false), ["X420"]);
    assert_eq!(check_ctx("lenn(pr.checks)", false), ["X404"]);
    assert_eq!(check_ctx("short(pr.word)", false), ["X405"]);
    assert_eq!(check_ctx("tokens(pr.word)", false), ["X406"]);
    assert_eq!(check_ctx("usage.week.percent | count", false), ["Q424"]);
    assert_eq!(check_ctx("pr.checks | where(item.stat == 'fail')", false), ["X401"]);
    assert_eq!(check_ctx("$steps | where(item.don)", false), ["X401"]);
    assert_eq!(check_ctx("set($x, 1)", false), ["X400"]);
    let thirteen_takes = format!("pr.checks{}", " | take(1)".repeat(13));
    assert_eq!(check_ctx(&thirteen_takes, false), ["Q426"]);
    assert_eq!(check_ctx(&"x".repeat(501), false), ["X407"]);
}

#[test]
fn lists_every_path_an_expression_reads_and_time_now_for_the_clock() {
    let mut deps = slate_dependencies("until(usage.week.resetsAt) + len($steps | where(item.done)) + first(pr.checks).name");
    deps.sort();
    assert_eq!(deps, ["$steps", "pr.checks", "time.now", "usage.week.resetsAt"]);
    assert_eq!(slate_dependencies("processes.list | pick(name, cpu)"), ["processes.list"]);
}

#[test]
fn parses_every_line_of_spec_04s_example_block_the_pluck_line_failing_as_marked() {
    let lines = [
        "percent(thread.context.used / thread.context.window)",
        "tokens(thread.context.free)",
        "usage.week.percent > 80 ? 'warning' : 'default'",
        "until(usage.week.resetsAt)",
        "len(pr.checks)",
        "first(pr.checks).name",
        "item.completedAt ? duration(num(item.completedAt) - num(item.startedAt)) : 'running'",
        "word(item.state)",
        "orElse(pr.word, 'no pull request')",
        "pr.checks | where(item.state == 'fail') | count",
        "$check.state == 'done' && $check.json.id != null",
        "json($check.out).title",
        "processes.list | sortBy(item.cpu, 'desc') | take(5) | pick(name, cpu, mem)",
        "$steps | where(item.done) | count",
        "(pr.checks | first).state",
    ];
    for line in lines {
        assert!(parse_slate_expression(line).errors.is_empty(), "{line}");
    }
    let pluck = parse_slate_expression("pr.checks | where(item.state != 'pass') | sortBy(item.name) | pluck");
    assert_eq!(pluck.errors[0].code, "Q422");
}

// ---- ago and until ----

#[test]
fn ago_and_until_say_which_way_the_time_runs() {
    let now: i64 = 1_791_158_400_000;
    let ctx = ctx_over(HashMap::from([("time.now".to_string(), json!(now))]), None, None, now);
    assert_eq!(evaluate_slate_expression(&format!("ago({})", now - 30_000), &ctx), Some(json!("30s ago")));
    assert_eq!(evaluate_slate_expression(&format!("until({})", now + 240_000), &ctx), Some(json!("in 4m")));
}

// ---- infinity ----

#[test]
fn never_leaves_the_evaluator_a_literal_past_the_largest_number_is_refused_and_an_overflowing_call_is_null() {
    assert_eq!(parse_slate_expression("[1e999]").errors.iter().map(|p| p.code.as_str()).collect::<Vec<_>>(), ["X400"]);
    let none = || ctx_over(HashMap::new(), None, None, NOW);
    assert_eq!(evaluate_slate_expression("percent(1e307)", &none()), Some(Value::Null));
    assert_eq!(evaluate_slate_expression("[percent(1e307), 1]", &none()), Some(json!([null, 1])));
}

// ---- smaller expression rules ----

#[test]
fn points_an_error_inside_a_template_hole_at_its_place_in_the_whole_expression() {
    let parsed = parse_slate_expression("`a ${1 $}`");
    let e = &parsed.errors[0];
    assert_eq!((e.code.as_str(), e.at), ("X400", Some(7)));
    assert!(e.message.contains("column 8"), "{}", e.message);
}

#[test]
fn places_q424_at_its_step_when_the_checker_is_given_an_offset() {
    let check = check_slate_expression("1 | first", &scope(false), 10);
    let p = &check.problems[0];
    assert_eq!((p.code.as_str(), p.at), ("Q424", Some(14)));
}

#[test]
fn counts_one_level_of_nesting_per_paren() {
    let eight = format!("{}1{}", "(".repeat(8), ")".repeat(8));
    assert!(parse_slate_expression(&eight).errors.is_empty());
    let sixteen = format!("{}1{}", "(".repeat(16), ")".repeat(16));
    let codes: Vec<String> = parse_slate_expression(&sixteen).errors.into_iter().map(|e| e.code).collect();
    assert_eq!(codes, ["X407"]);
}

#[test]
fn takes_first_list_n_as_spec_04_writes_it() {
    assert_eq!(ev("first([1, 2, 3], 2)"), Some(json!([1, 2])));
    assert!(check_ctx("first([1, 2, 3], 2)", false).is_empty());
}

#[test]
fn joins_on_item_id_as_on_id_and_refuses_a_key_that_is_not_a_field() {
    let rows: HashMap<String, Value> = HashMap::from([
        ("$a".to_string(), json!([{ "id": 1 }])),
        ("$b".to_string(), json!([{ "id": 1, "n": 5 }])),
    ]);
    let got = evaluate_slate_expression("$a | join($b, item.id)", &ctx_over(rows, None, None, NOW));
    assert_eq!(got, Some(json!([{ "id": 1, "b": { "id": 1, "n": 5 } }])));
    assert!(check_ctx("[{ id: 1 }] | join([{ id: 1 }], item.id + 1)", true).contains(&"Q425".to_string()));
}

// ---- hostile.test.ts: the expression-only cases ----

const OBJECT_PROTOTYPE_NAMES: [(&str, &str); 12] = [
    ("constructor", "Constructor"),
    ("__defineGetter__", "DefineGetter"),
    ("__defineSetter__", "DefineSetter"),
    ("hasOwnProperty", "HasOwnProperty"),
    ("__lookupGetter__", "LookupGetter"),
    ("__lookupSetter__", "LookupSetter"),
    ("isPrototypeOf", "IsPrototypeOf"),
    ("propertyIsEnumerable", "PropertyIsEnumerable"),
    ("toString", "ToString"),
    ("valueOf", "ValueOf"),
    ("__proto__", "Proto"),
    ("toLocaleString", "ToLocaleString"),
];

#[test]
fn reads_a_name_object_prototype_has_as_unknown_in_a_call_and_as_a_word() {
    let none = || ctx_over(HashMap::new(), None, None, NOW);
    for (name, word) in OBJECT_PROTOTYPE_NAMES {
        assert_eq!(evaluate_slate_expression(&format!("{name}(1)"), &none()), Some(Value::Null), "{name}(1)");
        assert_eq!(evaluate_slate_expression(&format!("word('{name}')"), &none()), Some(json!(word)), "word('{name}')");
    }
}

#[test]
fn charges_a_call_for_the_text_it_builds() {
    let mut nested = String::from("'a'");
    for _ in 0..8 {
        nested = format!("replace({nested}, 'a', 'aaaaaaaaaa')");
    }
    assert_eq!(evaluate_slate_expression(&nested, &ctx_over(HashMap::new(), None, None, NOW)), Some(Value::Null));
}

#[test]
fn enforces_the_step_budget_on_a_list_past_it() {
    let items = (0..SLATE_LIMITS.eval_steps).map(|_| SlateExpr::Lit { v: json!(1), at: 0 }).collect();
    let many = SlateExpr::List { items, at: 0 };
    assert_eq!(evaluate_slate_ast(&many, &ctx_over(HashMap::new(), None, None, NOW)), Some(Value::Null));
}

#[test]
fn joins_through_a_lookup_across_five_thousand_rows() {
    let rows: Vec<Value> = (0..5_000).map(|i| json!({ "id": i })).collect();
    let b: Vec<Value> = (0..5_000).map(|i| json!({ "id": i, "n": i * 2 })).collect();
    let values: HashMap<String, Value> = HashMap::from([("$a".to_string(), Value::Array(rows)), ("$b".to_string(), Value::Array(b))]);
    let joined = evaluate_slate_expression("$a | join($b, id)", &ctx_over(values, None, None, NOW)).expect("a value");
    assert_eq!(joined[4_999]["b"]["n"], json!(9_998));
}
