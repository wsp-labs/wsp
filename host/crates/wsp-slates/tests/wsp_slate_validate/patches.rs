// SPDX-License-Identifier: AGPL-3.0-only

use super::*;

#[test]
fn block_run_field_err() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.err, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_json() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.json, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_lines() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.lines, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_startedat() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.startedAt, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_endedat() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.endedAt, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_ms() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.ms, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_runs() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.runs, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_cut() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.cut, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_stale() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.stale, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_refreshing() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.refreshing, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_text() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.text, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_per_row_chart_literal() {
    check(
        r#"{"schema":2,"root":"chart-1","values":{"h":{"start":[]}},"derived":{},"runs":{},"reactions":[],"pieces":{"chart-1":{"type":"chart","props":{"label":"x","items":{"bind":"$h"},"x":"t","value":"v"}}}}"#,
        vec![
            row(
                "T303",
                "x is read per row, a formula over item: x={item.t}; \"t\" would be the same on every row",
                Some("chart-1"),
                Some("x"),
                Some("x={item.t}"),
                None,
            ),
            row(
                "T303",
                "value is read per row, a formula over item: value={item.v}; \"v\" would be the same on every row",
                Some("chart-1"),
                Some("value"),
                Some("value={item.v}"),
                None,
            ),
        ],
        vec![],
    );
}

#[test]
fn block_per_row_bars_name() {
    check(
        r#"{"schema":2,"root":"bars-1","values":{"h":{"start":[]}},"derived":{},"runs":{},"reactions":[],"pieces":{"bars-1":{"type":"bars","props":{"label":"x","items":{"bind":"$h"},"name":"n","value":{"bind":"item.v"}}}}}"#,
        vec![row(
            "T303",
            "name is read per row, a formula over item: name={item.n}; \"n\" would be the same on every row",
            Some("bars-1"),
            Some("name"),
            Some("name={item.n}"),
            None,
        )],
        vec![],
    );
}

#[test]
fn block_meter_max_name() {
    check(
        r#"{"schema":2,"root":"meter-1","values":{"n":{"start":1}},"derived":{},"runs":{},"reactions":[],"pieces":{"meter-1":{"type":"meter","props":{"label":"x","value":1,"max":"n"}}}}"#,
        vec![row(
            "T303",
            "max takes a number in braces, like max={5}, or a formula, like max={$count}; \"n\" is text",
            Some("meter-1"),
            Some("max"),
            Some("max={$n}"),
            None,
        )],
        vec![],
    );
}

#[test]
fn block_meter_max_text() {
    check(
        r#"{"schema":2,"root":"meter-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"meter-1":{"type":"meter","props":{"label":"x","value":1,"max":"5"}}}}"#,
        vec![row(
            "T303",
            "max takes a number in braces, like max={5}, or a formula, like max={$count}; \"5\" is text",
            Some("meter-1"),
            Some("max"),
            Some("max={5}"),
            None,
        )],
        vec![],
    );
}

#[test]
fn block_html_header() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["header-1"]},"header-1":{"type":"header","children":["text-1"]},"text-1":{"type":"text","props":{"value":"PR"}}}}"#,
        vec![row(
            "T300",
            "\"header\" is not a piece; a title is a <heading>, or a <section title=\"...\">",
            Some("header-1"),
            None,
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn block_html_div() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["div-1"]},"div-1":{"type":"div","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}}}"#,
        vec![row("T300", "\"div\" is not a piece; a group is a <column>, a <row> or a <section>", Some("div-1"), None, None, None)],
        vec![],
    );
}

#[test]
fn block_html_text_every() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"every":60,"value":"x"}}}}"#,
        vec![row(
            "T302",
            "every is a <run>'s, not a piece's: <run name=\"x\" cmd='...' every={60} />, and the piece reads $x",
            Some("text-1"),
            Some("every"),
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn block_html_img() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["img-1"]},"img-1":{"type":"img","props":{"src":"a.png"}}}}"#,
        vec![row("T300", "\"img\" is not a piece; a slate draws no images", Some("img-1"), None, None, None)],
        vec![],
    );
}

#[test]
fn block_html_list() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["list-1"]},"list-1":{"type":"list","props":{"items":[]}}}}"#,
        vec![row("T300", "\"list\" is not a piece; a list is a <checklist> or a <table items={...}>", Some("list-1"), None, None, None)],
        vec![],
    );
}

#[test]
fn block_render_judge() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"x":{"start":1}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["section-1","text-2","text-3","heading-1"]},"section-1":{"type":"section","props":{"title":"Bengaluru Retail Prices"},"children":["text-1"]},"text-1":{"type":"text","props":{"value":"📈 Up"}},"text-2":{"type":"text","props":{"value":"$x.out"}},"text-3":{"type":"text","props":{"value":"{$x}"}},"heading-1":{"type":"heading","props":{"value":"Open PRs"}}},"title":"Live Gold Prices"}"#,
        vec![],
        vec![
            row(
                "W018",
                "the slate's title \"Live Gold Prices\" is in Title Case; heads take sentence case",
                None,
                None,
                Some("title=\"Live gold prices\""),
                None,
            ),
            row(
                "W018",
                "title \"Bengaluru Retail Prices\" is in Title Case; heads take sentence case",
                Some("section-1"),
                Some("title"),
                Some("title=\"Bengaluru retail prices\""),
                None,
            ),
            row("W017", "value has an emoji; the slate's words carry none, so say it in words", Some("text-1"), Some("value"), None, None),
            row(
                "W001",
                "value is the literal text \"$x.out\", which reads like a formula; quotes show it as written",
                Some("text-2"),
                Some("value"),
                Some("value={$x.out}"),
                None,
            ),
            row(
                "W001",
                "value is the literal text \"{$x}\", which reads like a formula; quotes show it as written",
                Some("text-3"),
                Some("value"),
                Some("value={$x}"),
                None,
            ),
        ],
    );
}

#[test]
fn block_nested_run() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"rates":{"kind":"cmd","cmd":"echo 1","every":60}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${$rates.out}"}}}}}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_words_in_section() {
    check(
        r#"{"schema":2,"root":"section-1","values":{"n":{"start":[]}},"derived":{},"runs":{},"reactions":[],"pieces":{"section-1":{"type":"section","props":{"title":"Requests"},"children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"Requests: ${len($n)}"}}}}}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_tool_mcp_name() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"q":{"kind":"tool","server":"axiom","tool":"queryDataset","args":{"apl":"count()"}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $q: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($q)} />"),
            None,
        )],
    );
}

#[test]
fn block_token_in_cmd() {
    check(r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"a":{"kind":"cmd","cmd":"curl -H 'Authorization: Bearer xapt-dd233a3a-85a9-45d2-9a1c-a4608f2b9eeb' https://x"}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}}}"#, vec![row("S520", "cmd holds a token written out (xapt-d...), which the slate keeps and shows the person; let the command read it from the file it lives in, or take it from a <secret> the person types", Some("$a"), Some("cmd"), None, None)], vec![row("W019", "nothing starts $a: give it a button, a when, or every", None, None, Some("<button label=\"...\" onPress={start($a)} />"), None)]);
}

#[test]
fn block_token_in_file() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"files":{"q.py":"KEY = \"sk-ant-api03-abcdefghijklmnopqrst\""}}"#,
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
fn block_bearer_var_ok() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"a":{"kind":"cmd","cmd":"curl -H \"Authorization: Bearer $TOKEN\" https://x"}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $a: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($a)} />"),
            None,
        )],
    );
}

#[test]
fn block_token_in_env() {
    check(r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"a":{"kind":"cmd","cmd":"curl -H \"Authorization: Bearer $T\" https://x","env":{"T":"xapt-dd233a3a-85a9-45d2-9a1c-a4608f2b9eeb"}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}}}"#, vec![row("S520", "the slate holds a token written out (xapt-d...), which it keeps and shows the person; let a run's command read it from the file it lives in, or take it from a <secret> the person types", None, None, None, None)], vec![row("W019", "nothing starts $a: give it a button, a when, or every", None, None, Some("<button label=\"...\" onPress={start($a)} />"), None)]);
}

#[test]
fn block_piece_tag() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["rates"]},"rates":{"type":"piece","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}}}"#,
        vec![row(
            "T300",
            "\"piece\" is not a piece; the tag is the piece's own type, like <number ...> or <text>",
            Some("rates"),
            None,
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn block_secret_run_0() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"tok":{"start":null,"secret":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"echo","args":[{"bind":"$tok"}]}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
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
fn block_secret_run_1() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"tok":{"start":null,"secret":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"echo","env":{"TOK":{"bind":"$tok"}}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
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
fn block_secret_run_2() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"tok":{"start":null,"secret":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"echo","stdin":{"bind":"$tok"}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
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
fn block_secret_run_3() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"tok":{"start":null,"secret":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"echo","args":[{"bind":"$tok.len"}]}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
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
fn block_reaction_path_0() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"x":{"start":{"a":1,"b":0}}},"derived":{},"runs":{},"reactions":[{"on":{"change":["$x.a"]},"do":[{"do":"set","path":"$x.b","value":1}]}],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_reaction_path_1() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"x":{"start":{"a":1,"b":0}}},"derived":{},"runs":{},"reactions":[{"on":{"change":["$x.a"]},"do":[{"do":"set","path":"$x.a","value":1}]}],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row(
            "A610",
            "reactions and derived values form a cycle: when change of $x.a sets $x.a",
            None,
            None,
            Some("compute it as a <derived> value instead of setting it back"),
            None,
        )],
        vec![],
    );
}

#[test]
fn block_reaction_path_2() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"x":{"start":{"a":1,"b":0}}},"derived":{},"runs":{},"reactions":[{"on":{"change":["$x"]},"do":[{"do":"set","path":"$x.b","value":1}]}],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row(
            "A610",
            "reactions and derived values form a cycle: when change of $x sets $x.b",
            None,
            None,
            Some("compute it as a <derived> value instead of setting it back"),
            None,
        )],
        vec![],
    );
}

#[test]
fn block_reaction_path_3() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"x":{"start":{"a":1,"b":0}}},"derived":{},"runs":{},"reactions":[{"on":{"change":["$x.b"]},"do":[{"do":"set","path":"$x","value":{"bind":"{ a: 1, b: 2 }"}}]}],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row(
            "A610",
            "reactions and derived values form a cycle: when change of $x.b sets $x",
            None,
            None,
            Some("compute it as a <derived> value instead of setting it back"),
            None,
        )],
        vec![],
    );
}

#[test]
fn block_spec_02_model_1() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"id":{"start":""},"repo":{"start":"wsp-labs/wsp"},"step":{"start":1}},"derived":{"ok":"$check.state == 'done' and $check.json.id != null"},"runs":{"check":{"kind":"cmd","cmd":"gh api \"repos/$REPO/issues/$ID\"","env":{"REPO":{"bind":"$repo"},"ID":{"bind":"$id"}},"timeout":20}},"reactions":[{"on":{"change":["$id"]},"do":[{"do":"start","run":"check"}]}],"pieces":{"column-1":{"type":"column","children":["issue","text-1","text-2","text-3","button-1"]},"issue":{"type":"input","props":{"label":"Issue number","value":{"bind":"$id"},"mono":true}},"text-1":{"type":"text","when":"$check.state == 'running'","props":{"tone":"muted","value":"Checking"}},"text-2":{"type":"text","when":"$check.state == 'failed'","props":{"tone":"bad","value":{"format":"No such issue: ${$check.err}"}}},"text-3":{"type":"text","when":"$ok","props":{"tone":"good","value":{"format":"${$check.json.title}"}}},"button-1":{"type":"button","when":"$ok","props":{"label":"Next","variant":"primary"},"on":{"press":[{"do":"set","path":"$step","value":2}]}}},"title":"Deploy setup"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_spec_03_syntax_1() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{"failing":"pr.checks | where(item.state == 'fail')"},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["empty-1","facts-1","checks","text-1","button-1"]},"empty-1":{"type":"empty","when":"pr.number == null","props":{"title":"No pull request yet","body":"This thread's branch has no pull request."}},"facts-1":{"type":"facts","when":"pr.number != null","props":{"facts":[{"label":"State","value":{"bind":"pr.word"}},{"label":"Review","value":{"bind":"word(pr.review)"}},{"label":"Mergeable","value":{"bind":"word(pr.mergeable)"},"tone":{"bind":"pr.mergeable == 'mergeable' ? 'good' : 'muted'"}}]}},"checks":{"type":"table","when":"pr.number != null","props":{"items":{"bind":"pr.checks"},"key":{"bind":"item.name"},"empty":"No checks yet","columns":[{"title":"Check","value":{"bind":"item.name"}},{"title":"State","value":{"bind":"word(item.state)"},"tone":{"bind":"item.state == 'fail' ? 'bad' : item.state == 'pass' ? 'good' : 'muted'"}},{"title":"Took","value":{"bind":"item.completedAt ? duration(num(item.completedAt) - num(item.startedAt)) : 'running'"},"mono":true,"align":"end"}],"rowActions":[{"label":"Send to agent","when":"item.state == 'fail'","on":{"press":[{"do":"send","text":"A check failed. Read its log and fix it.","with":["item.name","item.link"]}]}}]}},"text-1":{"type":"text","when":"len($failing) > 0","props":{"tone":"muted","value":{"format":"${len($failing)} failing of ${len(pr.checks)}"}}},"button-1":{"type":"button","when":"pr.number != null","props":{"label":"Open on GitHub"},"on":{"press":[{"do":"open","target":{"bind":"pr.url"}}]}}},"title":"Pull request"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_spec_07_runs_1() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"id":{"start":""},"body":{"start":""}},"derived":{},"runs":{"check":{"kind":"cmd","cmd":"gh api \"repos/{owner}/{repo}/issues/$ID\"","env":{"ID":{"bind":"$id"}},"timeout":20},"tests":{"kind":"cmd","cmd":"pnpm exec vitest run --minWorkers=1 --maxWorkers=2 $1","once":true,"args":["packages/host/test/parity.test.ts"],"timeout":300,"stream":true},"note":{"kind":"cmd","cmd":"cat > notes/latest.md","stdin":{"bind":"$body"},"on":"host","cwd":"notes"},"issues":{"kind":"tool","server":"linear","tool":"list_issues","every":60,"args":{"first":50}},"board":{"kind":"resource","server":"linear","uri":"linear://team/WSP/board","every":60}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["input-1","input-2","button-1","button-2","output-1","button-3"]},"input-1":{"type":"input","props":{"label":"Issue","value":{"bind":"$id"}}},"input-2":{"type":"input","props":{"label":"Note","value":{"bind":"$body"},"lines":4}},"button-1":{"type":"button","props":{"label":"Check"},"on":{"press":[{"do":"start","run":"check"}]}},"button-2":{"type":"button","props":{"label":"Run the parity test"},"on":{"press":[{"do":"start","run":"tests"}]}},"output-1":{"type":"output","props":{"run":"$tests","lines":12}},"button-3":{"type":"button","props":{"label":"Save the note"},"on":{"press":[{"do":"start","run":"note"}]}}},"title":"Runs"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_spec_08_secrets_1() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"token":{"start":null,"secret":true},"saved":{"start":false}},"derived":{},"runs":{"save":{"kind":"cmd","cmd":"printf \"VERCEL_TOKEN=%s\\n\" \"$TOKEN\" >> .env","env":{"TOKEN":{"bind":"$token"}},"on":"host"}},"reactions":[{"on":{"done":"save"},"do":[{"do":"set","path":"$saved","value":{"bind":"$save.exit == 0"}}]}],"pieces":{"column-1":{"type":"column","children":["input-1","text-1","button-1","text-2"]},"input-1":{"type":"input","props":{"label":"Vercel token","value":{"bind":"$token"},"kind":"password"}},"text-1":{"type":"text","when":"$token.set and not $saved","props":{"tone":"muted","value":{"format":"Filled, ${$token.len} characters"}}},"button-1":{"type":"button","when":"$token.set","props":{"label":"Write it to .env"},"on":{"press":[{"do":"start","run":"save"}]}},"text-2":{"type":"text","when":"$saved","props":{"tone":"good","value":"Written to .env"}}},"title":"Deploy key"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_spec_appendix_examples_1() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{"failing":"pr.checks | where(item.state == 'fail')"},"runs":{"merge":{"kind":"cmd","cmd":"gh pr merge --squash --auto \"$PR\"","confirm":"Enable auto-merge on this pull request? It merges when the checks pass.","env":{"PR":{"bind":"pr.number"}},"timeout":30}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["empty-1","head","checks","summary","row-1","text-1","text-2"]},"empty-1":{"type":"empty","when":"pr.number == null","props":{"title":"No pull request yet","body":"This thread's branch has no pull request."}},"head":{"type":"facts","when":"pr.number != null","props":{"facts":[{"label":"State","value":{"bind":"pr.word"}},{"label":"Review","value":{"bind":"word(pr.review)"}},{"label":"Mergeable","value":{"bind":"word(pr.mergeable)"},"tone":{"bind":"pr.mergeable == 'mergeable' ? 'good' : pr.mergeable == 'conflicting' ? 'warning' : 'muted'"}}]}},"checks":{"type":"table","when":"pr.number != null","props":{"items":{"bind":"pr.checks"},"key":{"bind":"item.name"},"empty":"No checks yet","columns":[{"title":"Check","value":{"bind":"item.name"}},{"title":"State","value":{"bind":"word(item.state)"},"tone":{"bind":"item.state == 'fail' ? 'bad' : item.state == 'pass' ? 'good' : 'muted'"}},{"title":"Took","value":{"bind":"item.completedAt ? duration(num(item.completedAt) - num(item.startedAt)) : 'running'"},"mono":true,"align":"end"}],"rowActions":[{"label":"Send to agent","when":"item.state == 'fail'","on":{"press":[{"do":"send","text":"A check failed. Read its log and fix it.","with":["item.name","item.link"]}]}}]}},"summary":{"type":"text","when":"len($failing) > 0","props":{"tone":"muted","value":{"format":"${len($failing)} failing of ${len(pr.checks)}"}}},"row-1":{"type":"row","when":"pr.number != null","children":["button-1","button-2","automerge"]},"button-1":{"type":"button","props":{"label":"Open on GitHub"},"on":{"press":[{"do":"open","target":{"bind":"pr.url"}}]}},"button-2":{"type":"button","props":{"label":"Checks and comments","variant":"quiet"},"on":{"press":[{"do":"pane","kind":"pr"}]}},"automerge":{"type":"button","when":"pr.mergeable == 'mergeable' and pr.review == 'approved' and $merge.state != 'done'","props":{"label":"Auto-merge"},"on":{"press":[{"do":"start","run":"merge"}]}},"text-1":{"type":"text","when":"$merge.state == 'done'","props":{"tone":"good","value":"Auto-merge is on"}},"text-2":{"type":"text","when":"$merge.state == 'failed'","props":{"tone":"bad","value":{"format":"gh refused: ${short($merge.err, 120)}"}}}},"title":"Pull request"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_spec_appendix_examples_2() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"steps":{"start":[{"title":"Find where the cart rounds totals","done":true},{"title":"Round once at the end","done":true},{"title":"Fix the two failing tests","done":false},{"title":"Open a pull request","done":false}]}},"derived":{"done":"$steps | where(item.done) | count"},"runs":{"tests":{"kind":"cmd","cmd":"pnpm exec vitest run --minWorkers=1 --maxWorkers=2 test/cart.test.ts","once":true,"timeout":300,"stream":true}},"reactions":[{"on":{"done":"tests"},"do":[{"do":"set","path":"$steps[2].done","value":{"bind":"$tests.exit == 0"}}]}],"pieces":{"column-1":{"type":"column","children":["progress","todo","section-1","row-1","output-1"]},"progress":{"type":"meter","props":{"label":"Steps","value":{"bind":"$done"},"max":{"bind":"len($steps)"},"format":"fraction"}},"todo":{"type":"checklist","props":{"items":{"bind":"$steps"},"key":{"bind":"item.title"},"title":{"bind":"item.title"},"done":{"bind":"item.done"},"editable":true}},"section-1":{"type":"section","props":{"title":"Files changed","collapsible":true},"children":["table-1"]},"table-1":{"type":"table","props":{"items":{"bind":"thread.changes.files"},"key":{"bind":"item.path"},"empty":"Nothing changed yet","columns":[{"title":"File","value":{"bind":"item.path"},"mono":true},{"title":"Added","value":{"bind":"item.additions"},"tone":"good","align":"end"},{"title":"Removed","value":{"bind":"item.deletions"},"tone":"bad","align":"end"}]}},"row-1":{"type":"row","children":["button-1","next"]},"button-1":{"type":"button","when":"$tests.state != 'running'","props":{"label":"Run the cart tests"},"on":{"press":[{"do":"start","run":"tests"}]}},"next":{"type":"button","when":"$done < len($steps) and thread.status != 'working'","props":{"label":"Go on to the next step","variant":"primary"},"on":{"press":[{"do":"send","text":"Go on to the next unchecked step on the slate.","with":["$steps"]}]}},"output-1":{"type":"output","props":{"run":"$tests","lines":10}}},"title":"Cart rounding fix"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_spec_appendix_examples_3() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"step":{"start":1},"repo":{"start":"acme/landing"},"project":{"start":""},"vercelToken":{"start":null,"secret":true}},"derived":{"checked":"$check.state == 'done' and contains($check.out, $project)","written":"$env.state == 'done' and $ci.state == 'done'"},"runs":{"check":{"kind":"cmd","cmd":"vercel project inspect \"$PROJECT\" 2>&1 | head -c 4000","env":{"PROJECT":{"bind":"$project"},"VERCEL_TOKEN":{"bind":"$vercelToken"}},"on":"host","timeout":30},"env":{"kind":"cmd","cmd":"grep -v \"^VERCEL_TOKEN=\" .env > .env.tmp 2>/dev/null; printf \"VERCEL_TOKEN=%s\\n\" \"$VERCEL_TOKEN\" >> .env.tmp; mv .env.tmp .env","env":{"VERCEL_TOKEN":{"bind":"$vercelToken"}},"on":"host","timeout":10},"ci":{"kind":"cmd","cmd":"gh secret set VERCEL_TOKEN --repo \"$REPO\"","env":{"REPO":{"bind":"$repo"}},"stdin":{"bind":"$vercelToken"},"on":"host","timeout":60}},"reactions":[{"on":{"change":["$project"]},"do":[{"do":"start","run":"check"}]},{"on":{"change":["$checked"]},"do":[{"do":"set","path":"$step","value":{"bind":"$checked ? 3 : 2"}}]},{"on":{"change":["$written"]},"do":[{"do":"set","path":"$step","value":{"bind":"$written ? 4 : 3"}}]}],"pieces":{"column-1":{"type":"column","children":["text-1","section-1","section-2","section-3","section-4"]},"text-1":{"type":"text","props":{"emphasis":"strong","value":{"format":"Step ${$step} of 4"}}},"section-1":{"type":"section","when":"$step == 1","props":{"title":"1. The Vercel token"},"children":["markdown-1","button-1","input-1","button-2"]},"markdown-1":{"type":"markdown","props":{"value":"Open the Vercel dashboard, make a token with the project's scope, and paste it here. It never reaches the agent."}},"button-1":{"type":"button","props":{"label":"Open Vercel tokens"},"on":{"press":[{"do":"open","target":"https://vercel.com/account/tokens"}]}},"input-1":{"type":"input","props":{"label":"Vercel token","value":{"bind":"$vercelToken"},"kind":"password"}},"button-2":{"type":"button","when":"$vercelToken.set","props":{"label":"Next","variant":"primary"},"on":{"press":[{"do":"set","path":"$step","value":2}]}},"section-2":{"type":"section","when":"$step == 2 or $step == 3","props":{"title":"2. The project"},"children":["project-name","text-2","text-3","text-4","text-5"]},"project-name":{"type":"input","props":{"label":"Project name on Vercel","value":{"bind":"$project"},"mono":true}},"text-2":{"type":"text","when":"$check.state == 'held'","props":{"tone":"muted","value":"Approve the check to go on"}},"text-3":{"type":"text","when":"$check.state == 'running'","props":{"tone":"muted","value":"Checking"}},"text-4":{"type":"text","when":"$check.state == 'failed'","props":{"tone":"bad","value":{"format":"Vercel did not find it: ${short($check.err, 160)}"}}},"text-5":{"type":"text","when":"$checked","props":{"tone":"good","value":{"format":"Found ${$project}"}}},"section-3":{"type":"section","when":"$step == 3","props":{"title":"3. Write it down"},"children":["facts-1","row-1","text-6","text-7","text-8"]},"facts-1":{"type":"facts","props":{"facts":[{"label":"Repo","value":{"bind":"$repo"},"mono":true},{"label":"Project","value":{"bind":"$project"},"mono":true}]}},"row-1":{"type":"row","children":["button-3","button-4"]},"button-3":{"type":"button","when":"$env.state != 'done'","props":{"label":"Write .env"},"on":{"press":[{"do":"start","run":"env"}]}},"button-4":{"type":"button","when":"$ci.state != 'done'","props":{"label":"Set the CI secret"},"on":{"press":[{"do":"start","run":"ci"}]}},"text-6":{"type":"text","when":"$env.state == 'done'","props":{"tone":"good","value":".env written"}},"text-7":{"type":"text","when":"$ci.state == 'done'","props":{"tone":"good","value":"CI secret set"}},"text-8":{"type":"text","when":"$env.state == 'failed' or $ci.state == 'failed'","props":{"tone":"bad","value":{"format":"${short(concat($env.err, $ci.err), 200)}"}}},"section-4":{"type":"section","when":"$step == 4","props":{"title":"4. Done"},"children":["text-9","write"]},"text-9":{"type":"text","props":{"tone":"good","value":"The token is in .env and in the repo's secrets."}},"write":{"type":"button","props":{"label":"Tell the agent","variant":"primary"},"on":{"press":[{"do":"send","text":"The Vercel setup is done on the slate; wire the deploy hook next.","with":["$repo","$project","$vercelToken"]}]}}},"title":"Deploy setup"}"#,
        vec![],
        vec![row(
            "T311",
            "primary on button-2, write; one per slate reads loud, the rest draw as default",
            Some("write"),
            None,
            None,
            None,
        )],
    );
}

#[test]
fn block_spec_appendix_examples_4() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"hide":{"start":"Done"},"pick":{"start":""}},"derived":{"open":"$issues.json | where(item.state != $hide) | sortBy(item.priority)"},"runs":{"issues":{"kind":"tool","server":"linear","tool":"list_issues","every":60,"always":true,"args":{"first":50}},"finish":{"kind":"tool","server":"linear","tool":"update_issue","args":{"id":{"bind":"$pick"},"state":"Done"}}},"reactions":[{"on":{"done":"finish"},"do":[{"do":"start","run":"issues"}]}],"pieces":{"column-1":{"type":"column","children":["text-1","row-1","board"]},"text-1":{"type":"text","when":"mcp.linear.state != 'allowed'","props":{"tone":"muted","value":"Allow Linear in this thread's slate to see the board."}},"row-1":{"type":"row","props":{"align":"between"},"children":["text-2","select-1"]},"text-2":{"type":"text","props":{"emphasis":"strong","value":{"format":"${len($open)} open"}}},"select-1":{"type":"select","props":{"label":"Hide","value":{"bind":"$hide"},"options":[{"value":"Done"},{"value":"Canceled"},{"value":"In Review"}]}},"board":{"type":"table","props":{"items":{"bind":"$open"},"key":{"bind":"item.id"},"empty":"Nothing open","columns":[{"title":"Issue","value":{"bind":"item.identifier"},"mono":true},{"title":"Title","value":{"bind":"item.title"}},{"title":"Who","value":{"bind":"item.assignee"},"tone":"muted"}],"rowActions":[{"label":"Work on this","on":{"press":[{"do":"send","text":"Work on this Linear issue. Read it on Linear first.","with":["item.identifier","item.url"]}]}},{"label":"Done","on":{"press":[{"do":"set","path":"$pick","value":{"bind":"item.id"}},{"do":"start","run":"finish"}]}}]}}},"title":"Linear"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_spec_appendix_examples_5() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"startedAt":{"start":null},"minutes":{"start":"25"}},"derived":{"length":"num($minutes) * 60000","left":"exists($startedAt) ? max(0, $startedAt + $length - time.now) : null","over":"exists($startedAt) and time.now >= $startedAt + $length","working":"thread.plan.steps | where(item.state == 'working') | first"},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1","text-2","meter-1","text-3","row-1","text-4"]},"text-1":{"type":"text","props":{"emphasis":"strong","value":"Focus timer"}},"text-2":{"type":"text","when":"exists($startedAt)","props":{"size":"large","mono":true,"value":{"format":"${duration($left)} left"}}},"meter-1":{"type":"meter","when":"exists($startedAt)","props":{"label":"Elapsed","value":{"bind":"clamp(time.now - $startedAt, 0, $length)"},"max":{"bind":"$length"},"format":"none"}},"text-3":{"type":"text","when":"$over","props":{"tone":"good","emphasis":"strong","value":"Time. Stretch, then press Reset."}},"row-1":{"type":"row","children":["button-1","button-2","select-1"]},"button-1":{"type":"button","when":"not exists($startedAt)","props":{"label":"Start","variant":"primary"},"on":{"press":[{"do":"set","path":"$startedAt","value":{"bind":"time.now"}}]}},"button-2":{"type":"button","when":"exists($startedAt)","props":{"label":"Reset","variant":"quiet"},"on":{"press":[{"do":"set","path":"$startedAt","value":null}]}},"select-1":{"type":"select","props":{"label":"Minutes","value":{"bind":"$minutes"},"options":["15","25","45"]}},"text-4":{"type":"text","props":{"tone":"muted","size":"small","value":{"format":"Meanwhile the agent is on ${orElse($working.text, 'nothing in particular')}"}}}},"title":"Focus"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn every_code_the_validator_raises_is_in_the_closed_table() {
    for code in [
        "A600", "A601", "A602", "A606", "A607", "A608", "A610", "A611", "D200", "D202", "D203", "D205", "D206", "D207", "K700", "K701",
        "K702", "K703", "K704", "K705", "K707", "K708", "P103", "Q420", "Q422", "Q423", "Q424", "Q425", "Q426", "S501", "S502", "S503",
        "S510", "S512", "S513", "S520", "T300", "T302", "T303", "T304", "T305", "T306", "T307", "T308", "T309", "T311", "T312", "W001",
        "W003", "W004", "W011", "W012", "W013", "W014", "W015", "W017", "W018", "W019", "X400", "X401", "X404", "X405", "X406", "X407",
        "X408", "X409", "X410", "X420",
    ] {
        assert!(SLATE_CODES.iter().any(|(known, _)| *known == code), "{code} is not in the table");
    }
}

#[test]
fn patch_remove_piece() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"remove","id":"a"}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["b"]},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_add_under_at_zero() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"add","under":"top","at":0,"pieces":{"c":{"type":"text","props":{"value":"C"}}},"order":["c"]}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["c","a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}},"c":{"type":"text","props":{"value":"C"}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_move_under_itself() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"move","id":"a","under":"a"}]"#,
        r#"null"#,
        Want::Refused(vec![row("D205", "a cannot move under itself", Some("a"), None, None, None)], vec![]),
    );
}

#[test]
fn patch_props_set_and_add() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"props","id":"a","props":{"value":"Z","tone":"good"}}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"Z","tone":"good"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_props_null_removes_value() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"props","id":"a","props":{"value":null}}]"#,
        r#"null"#,
        Want::Refused(vec![row("T304", "text \"a\" needs value", Some("a"), Some("value"), None, None)], vec![]),
    );
}

#[test]
fn patch_replace_with_children() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"replace","id":"a","piece":{"type":"text","props":{"value":"new"}},"children":{}}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"new"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_value_added() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"value","name":"n2","decl":{"start":5}}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true},"n2":{"start":5}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"n2":5,"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_value_removed_leaves_reference() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"value","name":"n","decl":null}]"#,
        r#"null"#,
        Want::Refused(
            vec![
                row(
                    "S501",
                    "$n is not declared; add <value name=\"n\" start=... /> or did you mean $s?",
                    Some("$d"),
                    Some("value"),
                    Some("$s"),
                    None,
                ),
                row(
                    "S501",
                    "$n is not declared; add <value name=\"n\" start=... /> or did you mean $s?",
                    Some("b"),
                    Some("value"),
                    Some("$s"),
                    None,
                ),
            ],
            vec![],
        ),
    );
}

#[test]
fn patch_derived_added() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"derived","name":"d2","expr":"$n * 2"}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1","d2":"$n * 2"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_run_removed() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"run","name":"r","decl":null}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_reaction_added() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"reaction","id":"x1","reaction":{"id":"x1","on":{"change":["$n"]},"do":[{"do":"set","path":"$s","value":"y"}]}}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[{"id":"x1","on":{"change":["$n"]},"do":[{"do":"set","path":"$s","value":"y"}]}],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_reaction_missing_removed() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"reaction","id":"nope","reaction":null}]"#,
        r#"null"#,
        Want::Refused(vec![row("D203", "there is no reaction nope", Some("nope"), None, None, None)], vec![]),
    );
}

#[test]
fn patch_file_added() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"file","name":"a.py","text":"print(1)"}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)","a.py":"print(1)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_file_missing_removed() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"file","name":"gone.py","text":null}]"#,
        r#"null"#,
        Want::Refused(vec![row("D203", "there is no file gone.py", None, None, None, None)], vec![]),
    );
}

#[test]
fn patch_file_added_then_removed() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"file","name":"a.py","text":"x"},{"op":"file","name":"a.py","text":null}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_clear() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"clear"}]"#,
        r#"null"#,
        Want::Cleared(r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#),
    );
}

#[test]
fn patch_undo_refused() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"undo"}]"#,
        r#"null"#,
        Want::Refused(
            vec![row(
                "V752",
                "undo goes back to the document before the agent's last write, which the host holds; send <undo /> alone to the host",
                None,
                None,
                None,
                None,
            )],
            vec![],
        ),
    );
}

#[test]
fn patch_remove_root_refused() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"remove","id":"top"}]"#,
        r#"null"#,
        Want::Refused(
            vec![row("D202", "the root piece cannot be removed; write <clear /> or replace it", Some("top"), None, None, None)],
            vec![],
        ),
    );
}

#[test]
fn patch_props_missing_piece() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"props","id":"ghost","props":{}}]"#,
        r#"null"#,
        Want::Refused(vec![row("D203", "there is no piece ghost", Some("ghost"), None, None, None)], vec![]),
    );
}

#[test]
fn patch_add_existing_piece() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"add","under":"top","pieces":{"a":{"type":"text","props":{"value":"dup"}}},"order":["a"]}]"#,
        r#"null"#,
        Want::Refused(vec![row("P104", "a is already a piece; replace it by writing it with its id", Some("a"), None, None, None)], vec![]),
    );
}

#[test]
fn patch_move_missing_piece() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"move","id":"ghost","under":"top"}]"#,
        r#"null"#,
        Want::Refused(vec![row("D203", "move names ghost, which is not a piece", Some("ghost"), None, None, None)], vec![]),
    );
}

#[test]
fn patch_clear_with_other_op() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"clear"},{"op":"remove","id":"a"}]"#,
        r#"null"#,
        Want::Refused(vec![row("P105", "<clear /> stands alone in its write", None, None, None, None)], vec![]),
    );
}

#[test]
fn patch_remove_then_validate_refuses() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"remove","id":"b"},{"op":"props","id":"a","props":{"value":null}}]"#,
        r#"null"#,
        Want::Refused(vec![row("T304", "text \"a\" needs value", Some("a"), Some("value"), None, None)], vec![]),
    );
}

#[test]
fn patch_set_run_bad_kind() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"run","name":"r","decl":{"kind":"nope"}}]"#,
        r#"null"#,
        Want::Refused(
            vec![row("K701", "$r takes exactly one of cmd, tool or resource", Some("$r"), Some("kind"), None, None)],
            vec![row(
                "W019",
                "nothing starts $r: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r)} />"),
                None,
            )],
        ),
    );
}

#[test]
fn patch_move_child_up() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"move","id":"b","under":"top","at":0}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["b","a"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_add_at_end_clamped() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"add","under":"top","at":99,"pieces":{"c":{"type":"text","props":{"value":"C"}}},"order":["c"]}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b","c"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}},"c":{"type":"text","props":{"value":"C"}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_derived_cycle_refused() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"derived","name":"n","expr":"$d"}]"#,
        r#"null"#,
        Want::Refused(
            vec![
                row("P104", "$n is declared twice", Some("$n"), None, None, None),
                row(
                    "S510",
                    "derived values read each other in a cycle: $d reads $n reads $d",
                    Some("$d"),
                    Some("value"),
                    Some("make one of them a value"),
                    None,
                ),
            ],
            vec![],
        ),
    );
}

#[test]
fn patch_lines_on_missing_piece() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"remove","id":"zz"}]"#,
        r#"{"zz":7}"#,
        Want::Refused(vec![row("D203", "there is no piece zz to remove", Some("zz"), None, None, Some(7))], vec![]),
    );
}

#[test]
fn patch_lines_on_validation_prefix() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"props","id":"a","props":{"value":null}}]"#,
        r#"{"a":4}"#,
        Want::Refused(vec![row("T304", "text \"a\" needs value", Some("a"), Some("value"), None, Some(4))], vec![]),
    );
}

#[test]
fn patch_lines_on_duplicate_piece() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"add","under":"top","pieces":{"a":{"type":"text","props":{"value":"dup"}}},"order":["a"]}]"#,
        r#"{"a":9}"#,
        Want::Refused(
            vec![row("P104", "a is already a piece; replace it by writing it with its id", Some("a"), None, None, Some(9))],
            vec![],
        ),
    );
}

#[test]
fn patch_keeps_live_values_drops_removed() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","r":{"state":"done","runs":1}}"#,
        r#"[{"op":"value","name":"n","decl":{"start":1}},{"op":"run","name":"r","decl":null}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_without_slate_refused() {
    run_patch(
        None,
        r#"{}"#,
        r#"[{"op":"props","id":"a","props":{}}]"#,
        r#"null"#,
        Want::Refused(
            vec![row(
                "Z802",
                "there is no slate to patch; write one with <slate>",
                None,
                None,
                Some("<slate title=\"...\">...</slate>"),
                None,
            )],
            vec![],
        ),
    );
}

#[test]
fn patch_clear_without_slate() {
    run_patch(None, r#"{}"#, r#"[{"op":"clear"}]"#, r#"null"#, Want::Cleared(r#"{}"#));
}
