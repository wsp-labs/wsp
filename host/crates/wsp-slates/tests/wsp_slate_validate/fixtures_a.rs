// SPDX-License-Identifier: AGPL-3.0-only

use super::*;

#[test]
fn hostile_object_prototype_names_are_refused_as_pieces() {
    for name in [
        "__defineGetter__",
        "__defineSetter__",
        "__lookupGetter__",
        "__lookupSetter__",
        "__proto__",
        "constructor",
        "hasOwnProperty",
        "isPrototypeOf",
        "propertyIsEnumerable",
        "toLocaleString",
        "toString",
        "valueOf",
    ] {
        let doc = serde_json::json!({"version": 1, "root": {"id": "r", "type": name, "props": {}}});
        assert!(!validate_slate(&doc).errors.is_empty(), "{name}");
    }
}

#[test]
fn fixture_005_d206() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["column-2"]},"column-2":{"type":"column","children":["column-3"]},"column-3":{"type":"column","children":["column-4"]},"column-4":{"type":"column","children":["column-5"]},"column-5":{"type":"column","children":["column-6"]},"column-6":{"type":"column","children":["column-7"]},"column-7":{"type":"column","children":["column-8"]},"column-8":{"type":"column","children":["column-9"]},"column-9":{"type":"column","children":["column-10"]},"column-10":{"type":"column","children":["column-11"]},"column-11":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}}}"#,
        vec![row("D206", "the tree is deeper than 10", Some("column-11"), None, None, None)],
        vec![],
    );
}

#[test]
fn fixture_006_d207() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1","text-2","text-3","text-4","text-5","text-6","text-7","text-8","text-9","text-10","text-11","text-12","text-13","text-14","text-15","text-16","text-17","text-18","text-19","text-20","text-21","text-22","text-23","text-24","text-25","text-26","text-27","text-28","text-29","text-30","text-31","text-32","text-33","text-34","text-35","text-36","text-37","text-38","text-39","text-40","text-41","text-42","text-43","text-44","text-45","text-46","text-47","text-48","text-49","text-50","text-51","text-52","text-53","text-54","text-55","text-56","text-57","text-58","text-59","text-60","text-61","text-62","text-63","text-64","text-65","text-66","text-67","text-68","text-69","text-70","text-71","text-72","text-73","text-74","text-75","text-76","text-77","text-78","text-79","text-80","text-81","text-82","text-83","text-84","text-85","text-86","text-87","text-88","text-89","text-90","text-91","text-92","text-93","text-94","text-95","text-96","text-97","text-98","text-99","text-100","text-101"]},"text-1":{"type":"text","props":{"value":"x"}},"text-2":{"type":"text","props":{"value":"x"}},"text-3":{"type":"text","props":{"value":"x"}},"text-4":{"type":"text","props":{"value":"x"}},"text-5":{"type":"text","props":{"value":"x"}},"text-6":{"type":"text","props":{"value":"x"}},"text-7":{"type":"text","props":{"value":"x"}},"text-8":{"type":"text","props":{"value":"x"}},"text-9":{"type":"text","props":{"value":"x"}},"text-10":{"type":"text","props":{"value":"x"}},"text-11":{"type":"text","props":{"value":"x"}},"text-12":{"type":"text","props":{"value":"x"}},"text-13":{"type":"text","props":{"value":"x"}},"text-14":{"type":"text","props":{"value":"x"}},"text-15":{"type":"text","props":{"value":"x"}},"text-16":{"type":"text","props":{"value":"x"}},"text-17":{"type":"text","props":{"value":"x"}},"text-18":{"type":"text","props":{"value":"x"}},"text-19":{"type":"text","props":{"value":"x"}},"text-20":{"type":"text","props":{"value":"x"}},"text-21":{"type":"text","props":{"value":"x"}},"text-22":{"type":"text","props":{"value":"x"}},"text-23":{"type":"text","props":{"value":"x"}},"text-24":{"type":"text","props":{"value":"x"}},"text-25":{"type":"text","props":{"value":"x"}},"text-26":{"type":"text","props":{"value":"x"}},"text-27":{"type":"text","props":{"value":"x"}},"text-28":{"type":"text","props":{"value":"x"}},"text-29":{"type":"text","props":{"value":"x"}},"text-30":{"type":"text","props":{"value":"x"}},"text-31":{"type":"text","props":{"value":"x"}},"text-32":{"type":"text","props":{"value":"x"}},"text-33":{"type":"text","props":{"value":"x"}},"text-34":{"type":"text","props":{"value":"x"}},"text-35":{"type":"text","props":{"value":"x"}},"text-36":{"type":"text","props":{"value":"x"}},"text-37":{"type":"text","props":{"value":"x"}},"text-38":{"type":"text","props":{"value":"x"}},"text-39":{"type":"text","props":{"value":"x"}},"text-40":{"type":"text","props":{"value":"x"}},"text-41":{"type":"text","props":{"value":"x"}},"text-42":{"type":"text","props":{"value":"x"}},"text-43":{"type":"text","props":{"value":"x"}},"text-44":{"type":"text","props":{"value":"x"}},"text-45":{"type":"text","props":{"value":"x"}},"text-46":{"type":"text","props":{"value":"x"}},"text-47":{"type":"text","props":{"value":"x"}},"text-48":{"type":"text","props":{"value":"x"}},"text-49":{"type":"text","props":{"value":"x"}},"text-50":{"type":"text","props":{"value":"x"}},"text-51":{"type":"text","props":{"value":"x"}},"text-52":{"type":"text","props":{"value":"x"}},"text-53":{"type":"text","props":{"value":"x"}},"text-54":{"type":"text","props":{"value":"x"}},"text-55":{"type":"text","props":{"value":"x"}},"text-56":{"type":"text","props":{"value":"x"}},"text-57":{"type":"text","props":{"value":"x"}},"text-58":{"type":"text","props":{"value":"x"}},"text-59":{"type":"text","props":{"value":"x"}},"text-60":{"type":"text","props":{"value":"x"}},"text-61":{"type":"text","props":{"value":"x"}},"text-62":{"type":"text","props":{"value":"x"}},"text-63":{"type":"text","props":{"value":"x"}},"text-64":{"type":"text","props":{"value":"x"}},"text-65":{"type":"text","props":{"value":"x"}},"text-66":{"type":"text","props":{"value":"x"}},"text-67":{"type":"text","props":{"value":"x"}},"text-68":{"type":"text","props":{"value":"x"}},"text-69":{"type":"text","props":{"value":"x"}},"text-70":{"type":"text","props":{"value":"x"}},"text-71":{"type":"text","props":{"value":"x"}},"text-72":{"type":"text","props":{"value":"x"}},"text-73":{"type":"text","props":{"value":"x"}},"text-74":{"type":"text","props":{"value":"x"}},"text-75":{"type":"text","props":{"value":"x"}},"text-76":{"type":"text","props":{"value":"x"}},"text-77":{"type":"text","props":{"value":"x"}},"text-78":{"type":"text","props":{"value":"x"}},"text-79":{"type":"text","props":{"value":"x"}},"text-80":{"type":"text","props":{"value":"x"}},"text-81":{"type":"text","props":{"value":"x"}},"text-82":{"type":"text","props":{"value":"x"}},"text-83":{"type":"text","props":{"value":"x"}},"text-84":{"type":"text","props":{"value":"x"}},"text-85":{"type":"text","props":{"value":"x"}},"text-86":{"type":"text","props":{"value":"x"}},"text-87":{"type":"text","props":{"value":"x"}},"text-88":{"type":"text","props":{"value":"x"}},"text-89":{"type":"text","props":{"value":"x"}},"text-90":{"type":"text","props":{"value":"x"}},"text-91":{"type":"text","props":{"value":"x"}},"text-92":{"type":"text","props":{"value":"x"}},"text-93":{"type":"text","props":{"value":"x"}},"text-94":{"type":"text","props":{"value":"x"}},"text-95":{"type":"text","props":{"value":"x"}},"text-96":{"type":"text","props":{"value":"x"}},"text-97":{"type":"text","props":{"value":"x"}},"text-98":{"type":"text","props":{"value":"x"}},"text-99":{"type":"text","props":{"value":"x"}},"text-100":{"type":"text","props":{"value":"x"}},"text-101":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row("D207", "column-1 has 101 children; the most is 100", Some("column-1"), None, Some("bind a list instead"), None)],
        vec![],
    );
}

#[test]
fn fixture_007_t300() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["metre-1"]},"metre-1":{"type":"metre","props":{"label":"x","value":1}}},"title":"T"}"#,
        vec![row(
            "T300",
            "\"metre\" is not a piece; did you mean meter? slate_catalog lists them",
            Some("metre-1"),
            None,
            Some("meter"),
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_008_t302() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["meter-1"]},"meter-1":{"type":"meter","props":{"label":"x","value":1,"maxx":3}}},"title":"T"}"#,
        vec![row("T302", "meter has no maxx; did you mean max?", Some("meter-1"), Some("maxx"), Some("max"), None)],
        vec![],
    );
}

#[test]
fn fixture_009_t303() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["meter-1"]},"meter-1":{"type":"meter","props":{"label":"x","value":"usage.week.percent"}}},"title":"T"}"#,
        vec![row(
            "T303",
            "value takes a number in braces, like value={5}, or a formula, like value={$count}; \"usage.week.percent\" is text",
            Some("meter-1"),
            Some("value"),
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_010_t304() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["meter-1"]},"meter-1":{"type":"meter","props":{"label":"x"}}},"title":"T"}"#,
        vec![row("T304", "meter \"meter-1\" needs value", Some("meter-1"), Some("value"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_011_t305() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"size":{"bind":"pr.word"},"value":"x"}}},"title":"T"}"#,
        vec![row("T305", "size takes a literal, not a formula", Some("text-1"), Some("size"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_012_t306() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"tone":"loud","value":"x"}}},"title":"T"}"#,
        vec![row(
            "T306",
            "tone is default, muted, good, warning, bad, info or accent; \"loud\" is not one",
            Some("text-1"),
            Some("tone"),
            Some("good"),
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_013_t307() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"on":{"start":false}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["toggle-1"]},"toggle-1":{"type":"toggle","props":{"value":{"bind":"$on"}}}},"title":"T"}"#,
        vec![row(
            "T307",
            "a toggle needs a label for people using a screen reader",
            Some("toggle-1"),
            Some("label"),
            Some("label=\"...\""),
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_014_t308() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["meter-1"]},"meter-1":{"type":"meter","props":{"label":"x","value":1},"children":["text-1"]},"text-1":{"type":"text","props":{"value":"y"}}},"title":"T"}"#,
        vec![row("T308", "meter holds no children", Some("meter-1"), None, None, None)],
        vec![],
    );
}

#[test]
fn fixture_015_t309() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["empty-1"]},"empty-1":{"type":"empty","props":{"title":"x"},"children":["button-1","button-2"]},"button-1":{"type":"button","props":{"label":"a"},"on":{"press":[{"do":"copy","text":"a"}]}},"button-2":{"type":"button","props":{"label":"b"},"on":{"press":[{"do":"copy","text":"b"}]}}},"title":"T"}"#,
        vec![row("T309", "empty takes at most 1 child", Some("empty-1"), None, None, None)],
        vec![],
    );
}

#[test]
fn fixture_016_t311() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["button-1","button-2"]},"button-1":{"type":"button","props":{"label":"a","variant":"primary"},"on":{"press":[{"do":"copy","text":"a"}]}},"button-2":{"type":"button","props":{"label":"b","variant":"primary"},"on":{"press":[{"do":"copy","text":"b"}]}}},"title":"T"}"#,
        vec![],
        vec![row(
            "T311",
            "primary on button-1, button-2; one per slate reads loud, the rest draw as default",
            Some("button-2"),
            None,
            None,
            None,
        )],
    );
}

#[test]
fn fixture_017_t312() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"color":"red","value":"x"}}},"title":"T"}"#,
        vec![row(
            "T312",
            "color is not a prop a slate can set; wsp draws the style. Use tone.",
            Some("text-1"),
            Some("color"),
            Some("tone"),
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_019_x400() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${1 +}"}}}},"title":"T"}"#,
        vec![row("X400", "the expression ended early", Some("text-1"), Some("value"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_020_x401() {
    check(r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${pr.titel}"}}}},"title":"T"}"#, vec![row("X401", "pr.titel is not a path; pr has number, url, state, draft, base, branch, headOid, headSubject, mergeable, review, checks, additions, deletions, changedFiles, commits, behindBase, author, readAt, word, unread", Some("text-1"), Some("value"), None, None)], vec![]);
}

#[test]
fn fixture_021_x404() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${lenn(pr.checks)}"}}}},"title":"T"}"#,
        vec![row("X404", "lenn is not a function; did you mean len?", Some("text-1"), Some("value"), Some("len"), None)],
        vec![],
    );
}

#[test]
fn fixture_022_x405() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${short(pr.word)}"}}}},"title":"T"}"#,
        vec![row("X405", "short takes 2 arguments, got 1: short(s, n)", Some("text-1"), Some("value"), Some("short(s, n)"), None)],
        vec![],
    );
}

#[test]
fn fixture_023_x406() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${tokens(pr.word)}"}}}},"title":"T"}"#,
        vec![row("X406", "tokens takes a number; this gives text", Some("text-1"), Some("value"), Some("tokens(num(...))"), None)],
        vec![],
    );
}

#[test]
fn fixture_024_x407() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1}"}}}},"title":"T"}"#,
        vec![row("X407", "the expression is 521 characters; the most is 500", Some("text-1"), Some("value"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_025_x408() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${pr.checks == 'fail' ? 'a' : 'b'}"}}}},"title":"T"}"#,
        vec![row(
            "X408",
            "a list never equals a string; use contains(), len() or a step",
            Some("text-1"),
            Some("value"),
            Some("contains(list, value), len(list) or list | where(item.field == value)"),
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_026_x409() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${item.name}"}}}},"title":"T"}"#,
        vec![row("X409", "item is only known inside a repeating piece or a step", Some("text-1"), Some("value"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_027_x410() {
    check(r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["input-1"]},"input-1":{"type":"input","props":{"label":"x","value":{"bind":"pr.word"}}}},"title":"T"}"#, vec![row("X410", "value on input writes back, so it binds one plain value, not pr.word; bind a value of its own and keep a list with append in a <when> if you need one per item", Some("input-1"), Some("value"), Some("<value name=\"picked\" start={null} /> and value={$picked}"), None)], vec![]);
}

#[test]
fn fixture_028_x420() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${pr.checks.length}"}}}},"title":"T"}"#,
        vec![row("X420", "length is not a field; use len()", Some("text-1"), Some("value"), Some("len(x)"), None)],
        vec![],
    );
}

#[test]
fn fixture_029_q420() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${pr.checks | 3}"}}}},"title":"T"}"#,
        vec![row("Q420", "expected a step after | at column 13", Some("text-1"), Some("value"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_030_q422() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${pr.checks | filter(item.done)}"}}}},"title":"T"}"#,
        vec![row("Q422", "filter is not a step; did you mean where?", Some("text-1"), Some("value"), Some("where"), None)],
        vec![],
    );
}

#[test]
fn fixture_031_q423() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${pr.checks | where()}"}}}},"title":"T"}"#,
        vec![row("Q423", "where takes 1 argument, got 0: where(cond)", Some("text-1"), Some("value"), Some("where(cond)"), None)],
        vec![],
    );
}

#[test]
fn fixture_032_q424() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${pr.number | count}"}}}},"title":"T"}"#,
        vec![row("Q424", "count takes a list; this is a number", Some("text-1"), Some("value"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_033_q425() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${pr.checks | take(pr.number) | count}"}}}},"title":"T"}"#,
        vec![row("Q425", "take's count is a literal integer", Some("text-1"), Some("value"), Some("take(5)"), None)],
        vec![],
    );
}

#[test]
fn fixture_034_q426() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${pr.checks | skip(0) | skip(0) | skip(0) | skip(0) | skip(0) | skip(0) | skip(0) | skip(0) | skip(0) | skip(0) | skip(0) | skip(0) | skip(0) | count}"}}}},"title":"T"}"#,
        vec![row("Q426", "14 steps; the most is 12", Some("text-1"), Some("value"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_036_s501() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${$nope}"}}}},"title":"T"}"#,
        vec![row("S501", "$nope is not declared; add <value name=\"nope\" start=... />", Some("text-1"), Some("value"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_037_s502() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["checklist-1"]},"checklist-1":{"type":"checklist","props":{"items":{"bind":"pr.checks"},"title":{"bind":"item.name"},"done":{"bind":"item.state == 'pass'"},"editable":true}}},"title":"T"}"#,
        vec![row(
            "S502",
            "an editable checklist writes its ticks back, so items binds a value: items={$steps}",
            Some("checklist-1"),
            Some("items"),
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_039_s510() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{"a":"$b","b":"$a"},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row(
            "S510",
            "derived values read each other in a cycle: $a reads $b reads $a",
            Some("$a"),
            Some("value"),
            Some("make one of them a value"),
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_040_s512() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{"d0":"1","d1":"1","d2":"1","d3":"1","d4":"1","d5":"1","d6":"1","d7":"1","d8":"1","d9":"1","d10":"1","d11":"1","d12":"1","d13":"1","d14":"1","d15":"1","d16":"1","d17":"1","d18":"1","d19":"1","d20":"1","d21":"1","d22":"1","d23":"1","d24":"1","d25":"1","d26":"1","d27":"1","d28":"1","d29":"1","d30":"1","d31":"1","d32":"1"},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row("S512", "33 derived values; the most is 32", None, None, None, None)],
        vec![],
    );
}

#[test]
fn fixture_041_s513() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"v0":{"start":1},"v1":{"start":1},"v2":{"start":1},"v3":{"start":1},"v4":{"start":1},"v5":{"start":1},"v6":{"start":1},"v7":{"start":1},"v8":{"start":1},"v9":{"start":1},"v10":{"start":1},"v11":{"start":1},"v12":{"start":1},"v13":{"start":1},"v14":{"start":1},"v15":{"start":1},"v16":{"start":1},"v17":{"start":1},"v18":{"start":1},"v19":{"start":1},"v20":{"start":1},"v21":{"start":1},"v22":{"start":1},"v23":{"start":1},"v24":{"start":1},"v25":{"start":1},"v26":{"start":1},"v27":{"start":1},"v28":{"start":1},"v29":{"start":1},"v30":{"start":1},"v31":{"start":1},"v32":{"start":1},"v33":{"start":1},"v34":{"start":1},"v35":{"start":1},"v36":{"start":1},"v37":{"start":1},"v38":{"start":1},"v39":{"start":1},"v40":{"start":1},"v41":{"start":1},"v42":{"start":1},"v43":{"start":1},"v44":{"start":1},"v45":{"start":1},"v46":{"start":1},"v47":{"start":1},"v48":{"start":1},"v49":{"start":1},"v50":{"start":1},"v51":{"start":1},"v52":{"start":1},"v53":{"start":1},"v54":{"start":1},"v55":{"start":1},"v56":{"start":1},"v57":{"start":1},"v58":{"start":1},"v59":{"start":1},"v60":{"start":1},"v61":{"start":1},"v62":{"start":1},"v63":{"start":1},"v64":{"start":1}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row("S513", "65 values; the most is 64", None, None, None, None)],
        vec![],
    );
}

#[test]
fn fixture_042_s520() {
    check(r#"{"schema":2,"root":"column-1","values":{"token":{"start":null,"secret":true}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${$token}"}}}},"title":"T"}"#, vec![row("S520", "$token is a secret; only a run's env or stdin, an input's value or a send's paths may name it, and a formula reads only its .set, .len and .at", Some("text-1"), Some("value"), Some("$token.set"), None)], vec![]);
}

#[test]
fn fixture_047_a606() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["button-1"]},"button-1":{"type":"button","props":{"label":"x"},"on":{"press":[{"do":"copy","text":"1"},{"do":"copy","text":"2"},{"do":"copy","text":"3"},{"do":"copy","text":"4"},{"do":"copy","text":"5"},{"do":"copy","text":"6"},{"do":"copy","text":"7"}]}}},"title":"T"}"#,
        vec![row(
            "A606",
            "7 steps in one handler; the most is 6",
            Some("button-1"),
            Some("onPress"),
            Some("split it: a second <when> on what the first one sets"),
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_048_a607() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{"ok":"1 == 1"},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["button-1"]},"button-1":{"type":"button","props":{"label":"x"},"on":{"press":[{"do":"set","path":"$ok","value":true}]}}},"title":"T"}"#,
        vec![row(
            "A607",
            "$ok is derived and cannot be written; write the value it reads, or make it a value",
            Some("button-1"),
            Some("onPress"),
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_049_a608() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":1}},"derived":{},"runs":{},"reactions":[{"on":{"change":["$a"]},"do":[{"do":"open","target":"https://x.example"}]}],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row(
            "A608",
            "open runs in the window and goes on a press, not on a reaction",
            Some("reaction-1"),
            Some("do"),
            Some("onPress={open(...)}"),
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_050_a610() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":1},"b":{"start":1}},"derived":{},"runs":{},"reactions":[{"on":{"change":["$a"]},"do":[{"do":"set","path":"$b","value":{"bind":"$a"}}]},{"on":{"change":["$b"]},"do":[{"do":"set","path":"$a","value":{"bind":"$b"}}]}],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row(
            "A610",
            "reactions and derived values form a cycle: when change of $a sets $b; when change of $b sets $a",
            None,
            None,
            Some("compute it as a <derived> value instead of setting it back"),
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_054_k702() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["button-1"]},"button-1":{"type":"button","props":{"label":"x"},"on":{"press":[{"do":"start","run":"nope"}]}}},"title":"T"}"#,
        vec![row("K702", "start names a run: $nope is not declared", Some("button-1"), Some("onPress"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_055_k703() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"true","every":5}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row("K703", "every is at least 10 s", Some("$r"), Some("every"), Some("every={10}"), None)],
        vec![],
    );
}

#[test]
fn fixture_056_k704() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"true","timeout":900}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row("K704", "timeout is seconds up to 600", Some("$r"), Some("timeout"), Some("timeout={60}"), None)],
        vec![row(
            "W019",
            "nothing starts $r: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($r)} />"),
            None,
        )],
    );
}

#[test]
fn fixture_058_k707() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"r0":{"kind":"cmd","cmd":"true"},"r1":{"kind":"cmd","cmd":"true"},"r2":{"kind":"cmd","cmd":"true"},"r3":{"kind":"cmd","cmd":"true"},"r4":{"kind":"cmd","cmd":"true"},"r5":{"kind":"cmd","cmd":"true"},"r6":{"kind":"cmd","cmd":"true"},"r7":{"kind":"cmd","cmd":"true"},"r8":{"kind":"cmd","cmd":"true"},"r9":{"kind":"cmd","cmd":"true"},"r10":{"kind":"cmd","cmd":"true"},"r11":{"kind":"cmd","cmd":"true"},"r12":{"kind":"cmd","cmd":"true"},"r13":{"kind":"cmd","cmd":"true"},"r14":{"kind":"cmd","cmd":"true"},"r15":{"kind":"cmd","cmd":"true"},"r16":{"kind":"cmd","cmd":"true"}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row("K707", "17 runs; the most is 16", None, None, None, None)],
        vec![
            row(
                "W019",
                "nothing starts $r0: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r0)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r1: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r1)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r2: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r2)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r3: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r3)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r4: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r4)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r5: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r5)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r6: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r6)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r7: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r7)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r8: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r8)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r9: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r9)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r10: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r10)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r11: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r11)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r12: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r12)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r13: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r13)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r14: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r14)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r15: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r15)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r16: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r16)} />"),
                None,
            ),
        ],
    );
}

#[test]
fn fixture_059_w001() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"pr.checks"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W001",
            "value is the literal text \"pr.checks\", which reads like a path",
            Some("text-1"),
            Some("value"),
            Some("value={pr.checks}"),
            None,
        )],
    );
}

#[test]
fn fixture_060_w003() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["bars-1"]},"bars-1":{"type":"bars","props":{"label":"x","items":{"bind":"machine.history.cpu"},"name":{"bind":"item.x"},"value":{"bind":"item.y"}}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W003",
            "machine.history.cpu is a series over time; draw it as a line",
            Some("bars-1"),
            Some("items"),
            Some("<chart>"),
            None,
        )],
    );
}

#[test]
fn fixture_061_w004() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"a \u2014 b"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W004",
            "an em dash in the slate's words; use a comma, a colon or a full stop",
            Some("text-1"),
            Some("value"),
            None,
            None,
        )],
    );
}

#[test]
fn fixture_062_w011() {
    check(r#"{"schema":2,"root":"column-1","values":{"t":{"start":null,"secret":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"vercel ls --token \"$T\"","env":{"T":{"bind":"$t"}}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#, vec![], vec![row("W011", "T carries the secret $t, and the command passes it after --token, as an argument ps can read; hand it on stdin={$t}, or let the program read T from its environment", Some("$r"), Some("cmd"), Some("drop --token \"$T\" and use stdin={$t} or the program's own T"), None), row("W019", "nothing starts $r: give it a button, a when, or every", None, None, Some("<button label=\"...\" onPress={start($r)} />"), None)]);
}

#[test]
fn fixture_063_w012() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"Market closed · Weekend close"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W012",
            "value joins words with \"·\"; the kit separates things by layout, never a mark: give each its own piece, or join with a comma",
            Some("text-1"),
            Some("value"),
            Some("separate pieces (facts, chips, a row) or a comma"),
            None,
        )],
    );
}

#[test]
fn fixture_064_w013() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"hits":{"start":[{"n":1},{"n":2}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["chart-1"]},"chart-1":{"type":"chart","props":{"label":"Requests","items":{"bind":"$hits"},"x":{"bind":"index"},"value":{"bind":"item.n"}}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W013",
            "the chart's x is each row's index, so its axis reads 0 to the count; give each row its time, for example x={item.at}",
            Some("chart-1"),
            Some("x"),
            Some("x={item.at}"),
            None,
        )],
    );
}

#[test]
fn fixture_065_w014() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"mono":true,"value":"Kill the node process now"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W014",
            "mono is for figures, ids, times and paths; this reads as a sentence, which reads better in the normal face",
            Some("text-1"),
            Some("mono"),
            Some("drop mono"),
            None,
        )],
    );
}

#[test]
fn stored_json_spec_example_is_clean() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"id":{"start":""},"repo":{"start":"wsp-labs/wsp"},"step":{"start":1}},"derived":{"ok":"$check.state == 'done' and $check.json.id != null"},"runs":{"check":{"kind":"cmd","cmd":"gh api \"repos/$REPO/issues/$ID\"","env":{"REPO":{"bind":"$repo"},"ID":{"bind":"$id"}},"timeout":20}},"reactions":[{"on":{"change":["$id"]},"do":[{"do":"start","run":"check"}]}],"pieces":{"column-1":{"type":"column","children":["issue","text-1","text-2","text-3","button-1"]},"issue":{"type":"input","props":{"label":"Issue number","value":{"bind":"$id"},"mono":true}},"text-1":{"type":"text","when":"$check.state == 'running'","props":{"tone":"muted","value":"Checking"}},"text-2":{"type":"text","when":"$check.state == 'failed'","props":{"tone":"bad","value":{"format":"No such issue: ${$check.err}"}}},"text-3":{"type":"text","when":"$ok","props":{"tone":"good","value":{"format":"${$check.json.title}"}}},"button-1":{"type":"button","when":"$ok","props":{"label":"Next","variant":"primary"},"on":{"press":[{"do":"set","path":"$step","value":2}]}}},"title":"Deploy setup"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn stored_json_ghost_child() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"id":{"start":""},"repo":{"start":"wsp-labs/wsp"},"step":{"start":1}},"derived":{"ok":"$check.state == 'done' and $check.json.id != null"},"runs":{"check":{"kind":"cmd","cmd":"gh api \"repos/$REPO/issues/$ID\"","env":{"REPO":{"bind":"$repo"},"ID":{"bind":"$id"}},"timeout":20}},"reactions":[{"on":{"change":["$id"]},"do":[{"do":"start","run":"check"}]}],"pieces":{"column-1":{"type":"column","children":["issue","text-1","text-2","text-3","button-1","ghost"]},"issue":{"type":"input","props":{"label":"Issue number","value":{"bind":"$id"},"mono":true}},"text-1":{"type":"text","when":"$check.state == 'running'","props":{"tone":"muted","value":"Checking"}},"text-2":{"type":"text","when":"$check.state == 'failed'","props":{"tone":"bad","value":{"format":"No such issue: ${$check.err}"}}},"text-3":{"type":"text","when":"$ok","props":{"tone":"good","value":{"format":"${$check.json.title}"}}},"button-1":{"type":"button","when":"$ok","props":{"label":"Next","variant":"primary"},"on":{"press":[{"do":"set","path":"$step","value":2}]}}},"title":"Deploy setup"}"#,
        vec![row("D203", "column-1 holds ghost, which is not a piece", Some("column-1"), None, None, None)],
        vec![],
    );
}

#[test]
fn stored_json_root_child_cycle() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"id":{"start":""},"repo":{"start":"wsp-labs/wsp"},"step":{"start":1}},"derived":{"ok":"$check.state == 'done' and $check.json.id != null"},"runs":{"check":{"kind":"cmd","cmd":"gh api \"repos/$REPO/issues/$ID\"","env":{"REPO":{"bind":"$repo"},"ID":{"bind":"$id"}},"timeout":20}},"reactions":[{"on":{"change":["$id"]},"do":[{"do":"start","run":"check"}]}],"pieces":{"column-1":{"type":"column","children":["issue","text-1","text-2","text-3","column-1"]},"issue":{"type":"input","props":{"label":"Issue number","value":{"bind":"$id"},"mono":true}},"text-1":{"type":"text","when":"$check.state == 'running'","props":{"tone":"muted","value":"Checking"}},"text-2":{"type":"text","when":"$check.state == 'failed'","props":{"tone":"bad","value":{"format":"No such issue: ${$check.err}"}}},"text-3":{"type":"text","when":"$ok","props":{"tone":"good","value":{"format":"${$check.json.title}"}}},"button-1":{"type":"button","when":"$ok","props":{"label":"Next","variant":"primary"},"on":{"press":[{"do":"set","path":"$step","value":2}]}}},"title":"Deploy setup"}"#,
        vec![row("D205", "column-1 is under itself: column-1 > column-1", Some("column-1"), None, None, None)],
        vec![],
    );
}

#[test]
fn stored_json_schema_1() {
    check(
        r#"{"schema":1,"root":"column-1","values":{"id":{"start":""},"repo":{"start":"wsp-labs/wsp"},"step":{"start":1}},"derived":{"ok":"$check.state == 'done' and $check.json.id != null"},"runs":{"check":{"kind":"cmd","cmd":"gh api \"repos/$REPO/issues/$ID\"","env":{"REPO":{"bind":"$repo"},"ID":{"bind":"$id"}},"timeout":20}},"reactions":[{"on":{"change":["$id"]},"do":[{"do":"start","run":"check"}]}],"pieces":{"column-1":{"type":"column","children":["issue","text-1","text-2","text-3","button-1"]},"issue":{"type":"input","props":{"label":"Issue number","value":{"bind":"$id"},"mono":true}},"text-1":{"type":"text","when":"$check.state == 'running'","props":{"tone":"muted","value":"Checking"}},"text-2":{"type":"text","when":"$check.state == 'failed'","props":{"tone":"bad","value":{"format":"No such issue: ${$check.err}"}}},"text-3":{"type":"text","when":"$ok","props":{"tone":"good","value":{"format":"${$check.json.title}"}}},"button-1":{"type":"button","when":"$ok","props":{"label":"Next","variant":"primary"},"on":{"press":[{"do":"set","path":"$step","value":2}]}}},"title":"Deploy setup"}"#,
        vec![row("D200", "schema 1 is not a version this host knows; this host reads schema 2", None, None, Some("update wsp"), None)],
        vec![],
    );
}

#[test]
fn shape_missing_root() {
    check(r#"{"schema":2,"pieces":{"top":{"type":"column"}}}"#, vec![row("D202", "root: Required", None, None, None, None)], vec![]);
}

#[test]
fn shape_extra_and_child_bad() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"bogus":1,"values":{"v":{"start":1,"extra":2}}}"#,
        vec![
            row("T302", "$v has no extra; it takes start, secret and keep", Some("$v"), Some("extra"), None, None),
            row(
                "D202",
                "the document has no bogus; it holds schema, kit, title, root, values, derived, runs, reactions, pieces, files",
                None,
                None,
                None,
                None,
            ),
        ],
        vec![],
    );
}

#[test]
fn shape_run_no_kind() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"cmd":"x"}}}"#,
        vec![row("K701", "$r takes kind \"cmd\", \"tool\" or \"resource\"", Some("$r"), Some("kind"), None, None)],
        vec![],
    );
}

#[test]
fn shape_run_bad_kind() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"kind":"nope","cmd":"x"}}}"#,
        vec![row("K701", "$r takes kind \"cmd\", \"tool\" or \"resource\"", Some("$r"), Some("kind"), None, None)],
        vec![],
    );
}

#[test]
fn shape_run_cmd_number() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"kind":"cmd","cmd":5}}}"#,
        vec![row("K704", "$r.cmd: Expected string, received number", Some("$r"), Some("cmd"), None, None)],
        vec![],
    );
}

#[test]
fn shape_run_confirm_empty_bind() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"kind":"cmd","cmd":"x","confirm":{"bind":""}}}}"#,
        vec![row("K704", "$r.confirm.bind: String must contain at least 1 character(s)", Some("$r"), Some("confirm"), None, None)],
        vec![],
    );
}

#[test]
fn shape_reaction_on_empty() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"reactions":[{"on":{},"do":[]}]}"#,
        vec![row(
            "A611",
            "reaction 1.on: Invalid input; a <when> is { on: { change: [paths] } or { done: name }, do: [steps] }",
            Some("reaction-1"),
            None,
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_reaction_step_bad() {
    check(r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"reactions":[{"on":{"change":["$a"]},"do":[{"do":"nope"}]}]}"#, vec![row("A611", "reaction 1.do.0.do: Invalid discriminator value. Expected 'set' | 'toggle' | 'start' | 'cancel' | 'send' | 'steer' | 'queue' | 'fill' | 'open' | 'copy' | 'pane'; a <when> is { on: { change: [paths] } or { done: name }, do: [steps] }", Some("reaction-1"), None, None, None)], vec![]);
}

#[test]
fn shape_step_set_no_value() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"reactions":[{"on":{"change":["$a"]},"do":[{"do":"set","path":"$a"}]}]}"#,
        vec![row(
            "A611",
            "reaction 1.do.0.value: Invalid input; a <when> is { on: { change: [paths] } or { done: name }, do: [steps] }",
            Some("reaction-1"),
            None,
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_piece_type_number() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":5}}}"#,
        vec![row("T300", "top's type is the piece's name as a string", Some("top"), Some("type"), None, None)],
        vec![],
    );
}

#[test]
fn shape_piece_props_array() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","props":[1]}}}"#,
        vec![row("T303", "top.props: Expected object, received array", Some("top"), None, None, None)],
        vec![],
    );
}

#[test]
fn shape_kit_number() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"kit":5}"#,
        vec![row("D202", "kit: Expected string, received number", None, None, None, None)],
        vec![],
    );
}

#[test]
fn shape_value_no_start() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"v":{}}}"#,
        vec![row("T303", "$v.start: Invalid input", Some("$v"), Some("start"), None, None)],
        vec![],
    );
}

#[test]
fn shape_value_secret_false() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"v":{"start":1,"secret":false}}}"#,
        vec![row("T303", "$v.secret: Invalid literal value, expected true", Some("$v"), Some("secret"), None, None)],
        vec![],
    );
}

#[test]
fn shape_pane_bad_kind() {
    check(r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"reactions":[{"on":{"done":"x"},"do":[{"do":"pane","kind":"zzz"}]}]}"#, vec![row("A611", "reaction 1.do.0.kind: Invalid enum value. Expected 'preview' | 'terminal' | 'diff' | 'pr' | 'files' | 'machine' | 'processes' | 'agents' | 'slate', received 'zzz'; a <when> is { on: { change: [paths] } or { done: name }, do: [steps] }", Some("reaction-1"), None, None, None)], vec![]);
}

#[test]
fn shape_fallback_number() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","fallback":5}}}"#,
        vec![row("T303", "top.fallback: Invalid input", Some("top"), Some("fallback"), None, None)],
        vec![],
    );
}

#[test]
fn shape_derived_number() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"derived":{"a":5}}"#,
        vec![row("T303", "$a.decl: Expected string, received number", Some("$a"), None, None, None)],
        vec![],
    );
}

#[test]
fn shape_array_document() {
    check(
        r#"[1]"#,
        vec![row("D202", "a document is an object with schema, root, values, derived, runs, reactions and pieces", None, None, None, None)],
        vec![],
    );
}

#[test]
fn shape_top_unknown_and_root_number() {
    check(
        r#"{"schema":2,"bogus":1,"root":3,"pieces":{}}"#,
        vec![
            row("D202", "root: Expected string, received number", None, None, None, None),
            row(
                "D202",
                "the document has no bogus; it holds schema, kit, title, root, values, derived, runs, reactions, pieces, files",
                None,
                None,
                None,
                None,
            ),
        ],
        vec![],
    );
}

#[test]
fn shape_piece_on_bad_events() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","on":{"press":"x","zap":[]}}}}"#,
        vec![
            row(
                "A600",
                "top.on.press: a handler is a list of steps, Expected array, received string",
                Some("top"),
                Some("on.press"),
                None,
                None,
            ),
            row("A602", "top has no on.zap; events are press, submit and change", Some("top"), Some("on"), None, None),
        ],
        vec![],
    );
}

#[test]
fn shape_schema_string() {
    check(
        r#"{"schema":"x","root":"top"}"#,
        vec![row("D200", "schema \"x\" is not a version this host knows; this host reads schema 2", None, None, Some("update wsp"), None)],
        vec![],
    );
}

#[test]
fn shape_no_schema() {
    check(
        r#"{"root":"top"}"#,
        vec![row(
            "D200",
            "this JSON is not a slate: a slate is the JSX-like text slate_catalog shows, sent as text, never JSON of your own",
            None,
            None,
            Some("<slate title=\"Gold\"><number label=\"Spot\" value={$spot.json.usd} unit=\"USD\" /></slate>"),
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_no_schema_markup_value() {
    check(
        r#"{"title":"x","body":"<slate><text>x</text></slate>"}"#,
        vec![row(
            "D200",
            "this JSON wraps slate markup; send the markup itself as text, not inside document",
            None,
            None,
            Some("text: \"<slate title=\"Gold\">...</slate>\""),
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_markup_key_only() {
    check(
        r#"{"slate":"<slate><text>x</text></slate>"}"#,
        vec![row(
            "D200",
            "this JSON wraps slate markup; send the markup itself as text, not inside document",
            None,
            None,
            Some("text: \"<slate title=\"Gold\">...</slate>\""),
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_schema_3() {
    check(
        r#"{"schema":3}"#,
        vec![row("D200", "schema 3 is not a version this host knows; this host reads schema 2", None, None, Some("update wsp"), None)],
        vec![],
    );
}

#[test]
fn shape_props_on_document_top() {
    check(r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"className":"x"}"#, vec![row("D202", "the document has no className; it holds schema, kit, title, root, values, derived, runs, reactions, pieces, files; a piece's props go under pieces.<id>.props", None, None, None, None)], vec![]);
}

#[test]
fn shape_run_tool_missing_server() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"t":{"kind":"tool","tool":"x"}}}"#,
        vec![row("K704", "$t.server: Required", Some("$t"), Some("server"), None, None)],
        vec![],
    );
}

#[test]
fn shape_run_resource_missing_uri() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"s":{"kind":"resource","server":"srv"}}}"#,
        vec![row("K704", "$s.uri: Required", Some("$s"), Some("uri"), None, None)],
        vec![],
    );
}

#[test]
fn shape_run_extra_key_alway() {
    check(r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"kind":"cmd","cmd":"true","every":60,"alway":true}}}"#, vec![row("K704", "$r has no alway; it takes kind, cmd, env, args, stdin, on, cwd, timeout, stream, confirm, every, once, always, then and description", Some("$r"), Some("alway"), None, None)], vec![]);
}

#[test]
fn shape_piece_prop_beside_props() {
    check(
        r#"{"schema":2,"root":"col","pieces":{"col":{"type":"column","children":["t"]},"t":{"type":"text","tone":"bad","props":{"value":"x"}}}}"#,
        vec![row(
            "T302",
            "tone sits beside props on t; a piece holds type, props, children, when, on and fallback, and tone goes inside props",
            Some("t"),
            Some("tone"),
            Some("\"props\": { \"tone\": ... }"),
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_reaction_both_on_keys() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"reactions":[{"on":{"change":["$a"],"done":"r"},"do":[]}]}"#,
        vec![row(
            "A611",
            "reaction 1.on: Unrecognized key(s) in object: 'done'; a <when> is { on: { change: [paths] } or { done: name }, do: [steps] }",
            Some("reaction-1"),
            None,
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_reaction_id_number() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"reactions":[{"id":4,"on":{"change":["$a"]},"do":[]}]}"#,
        vec![row(
            "A611",
            "reaction 1.id: Expected string, received number; a <when> is { on: { change: [paths] } or { done: name }, do: [steps] }",
            Some("reaction-1"),
            None,
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_run_env_not_record() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"kind":"cmd","cmd":"x","env":[1]}}}"#,
        vec![row("K704", "$r.env: Expected object, received array", Some("$r"), Some("env"), None, None)],
        vec![],
    );
}

#[test]
fn shape_run_on_bad() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"kind":"cmd","cmd":"x","on":"laptop"}}}"#,
        vec![row("K704", "$r.on: Invalid enum value. Expected 'thread' | 'host', received 'laptop'", Some("$r"), Some("on"), None, None)],
        vec![],
    );
}

#[test]
fn shape_run_secret_arg_refused() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"tok":{"start":null,"secret":true}},"runs":{"r":{"kind":"cmd","cmd":"echo","args":[{"bind":"$tok"}]}}}"#,
        vec![row(
            "S520",
            "args[0] is the secret $tok; a secret reaches a command through env or stdin, never as an argument, which ps can read",
            Some("$r"),
            Some("args[0]"),
            Some("env={{ TOK: $tok }}, or stdin={$tok}"),
            None,
        )],
        vec![row(
            "W019",
            "nothing starts $r: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($r)} />"),
            None,
        )],
    );
}

#[test]
fn shape_run_secret_env_ok() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"tok":{"start":null,"secret":true}},"runs":{"r":{"kind":"cmd","cmd":"echo","env":{"TOK":{"bind":"$tok"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $r: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($r)} />"),
            None,
        )],
    );
}

#[test]
fn shape_run_flag_carries_secret() {
    check(r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"t":{"start":null,"secret":true}},"runs":{"r":{"kind":"cmd","cmd":"vercel ls --token \"$T\"","env":{"T":{"bind":"$t"}}}}}"#, vec![], vec![row("W011", "T carries the secret $t, and the command passes it after --token, as an argument ps can read; hand it on stdin={$t}, or let the program read T from its environment", Some("$r"), Some("cmd"), Some("drop --token \"$T\" and use stdin={$t} or the program's own T"), None), row("W019", "nothing starts $r: give it a button, a when, or every", None, None, Some("<button label=\"...\" onPress={start($r)} />"), None)]);
}

#[test]
fn shape_run_token_in_cmd() {
    check(r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"a":{"kind":"cmd","cmd":"curl -H 'Authorization: Bearer xapt-dd233a3a-85a9-45d2-9a1c-a4608f2b9eeb' https://x"}}}"#, vec![row("S520", "cmd holds a token written out (xapt-d...), which the slate keeps and shows the person; let the command read it from the file it lives in, or take it from a <secret> the person types", Some("$a"), Some("cmd"), None, None)], vec![row("W019", "nothing starts $a: give it a button, a when, or every", None, None, Some("<button label=\"...\" onPress={start($a)} />"), None)]);
}

#[test]
fn shape_file_token() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"files":{"q.py":"KEY = \"sk-ant-api03-abcdefghijklmnopqrst\""}}"#,
        vec![row(
            "S520",
            "q.py holds a token written out (sk-ant...); let the code read it from the file it lives in, or from an env the run hands it",
            Some("q.py"),
            None,
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_file_bad_name() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"files":{"../x.py":"print(1)"}}"#,
        vec![row(
            "K708",
            "\"../x.py\" is not a file name: letters, digits, dots, dashes and _, no slashes, not starting with a dot",
            Some("../x.py"),
            None,
            Some("x.py"),
            None,
        )],
        vec![],
    );
}
