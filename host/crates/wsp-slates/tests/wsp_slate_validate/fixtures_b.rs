// SPDX-License-Identifier: AGPL-3.0-only

use super::*;

#[test]
fn shape_file_slate_dir_missing() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"kind":"cmd","cmd":"python3 $SLATE_DIR/shape.py","every":60}}}"#,
        vec![],
        vec![row("W015", "$r runs $SLATE_DIR/shape.py, and no <file name=\"shape.py\"> is declared", Some("$r"), Some("cmd"), None, None)],
    );
}

#[test]
fn shape_derived_cycle() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"x":{"start":1}},"derived":{"a":"$b","b":"$a"}}"#,
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
fn shape_reaction_cycle() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"a":{"start":1},"b":{"start":1}},"reactions":[{"on":{"change":["$a"]},"do":[{"do":"set","path":"$b","value":1}]},{"on":{"change":["$b"]},"do":[{"do":"set","path":"$a","value":1}]}]}"#,
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
fn shape_done_names_non_run() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"v":{"start":1}},"reactions":[{"on":{"done":"v"},"do":[]}]}"#,
        vec![row("K702", "done names a run: $v is value", Some("reaction-1"), Some("done"), None, None)],
        vec![],
    );
}

#[test]
fn shape_when_per_row_literal() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":["c"]},"c":{"type":"chart","props":{"label":"x","items":{"bind":"$h"},"x":"t","value":"v"}}},"values":{"h":{"start":[]}}}"#,
        vec![
            row(
                "T303",
                "x is read per row, a formula over item: x={item.t}; \"t\" would be the same on every row",
                Some("c"),
                Some("x"),
                Some("x={item.t}"),
                None,
            ),
            row(
                "T303",
                "value is read per row, a formula over item: value={item.v}; \"v\" would be the same on every row",
                Some("c"),
                Some("value"),
                Some("value={item.v}"),
                None,
            ),
        ],
        vec![],
    );
}

#[test]
fn shape_title_case() {
    check(
        r#"{"schema":2,"title":"Live Gold Prices","root":"top","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"top":{"type":"column","children":[]}}}"#,
        vec![],
        vec![row(
            "W018",
            "the slate's title \"Live Gold Prices\" is in Title Case; heads take sentence case",
            None,
            None,
            Some("title=\"Live gold prices\""),
            None,
        )],
    );
}

#[test]
fn shape_reserved_prop_top() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","props":{"tone":"bad"}}}}"#,
        vec![row("T302", "column has no tone; it takes gap, align, pad, surface", Some("top"), Some("tone"), None, None)],
        vec![],
    );
}

#[test]
fn shape_depth_and_children() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":["a"]},"a":{"type":"column","children":["top"]}}}"#,
        vec![row("D205", "top is under itself: top > a > top", Some("top"), None, None, None)],
        vec![],
    );
}

#[test]
fn shape_fallback_drop() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":["a"]},"a":{"type":"sparkle","fallback":"drop"}}}"#,
        vec![],
        vec![],
    );
}

#[test]
fn shape_fallback_empty_text() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":["a"]},"a":{"type":"sparkle","fallback":{"text":"hi"}}}}"#,
        vec![],
        vec![],
    );
}

#[test]
fn shape_when_not_string() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","when":5}}}"#,
        vec![row("T303", "top.when: Expected string, received number", Some("top"), Some("when"), None, None)],
        vec![],
    );
}

#[test]
fn shape_on_handler_not_list() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","on":{"press":"x"}}}}"#,
        vec![row(
            "A600",
            "top.on.press: a handler is a list of steps, Expected array, received string",
            Some("top"),
            Some("on.press"),
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_props_null_value() {
    check(r#"{"schema":2,"root":"top","pieces":{"top":{"type":"text","props":{"value":null}}}}"#, vec![], vec![]);
}

#[test]
fn shape_value_keep_on_value() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"v":{"start":1,"keep":true}}}"#,
        vec![row("T302", "keep is for a <secret>; $v is a value", Some("$v"), Some("start"), None, None)],
        vec![],
    );
}

#[test]
fn shape_secret_with_start() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"s":{"start":"typed","secret":true}}}"#,
        vec![row("S503", "$s is a secret and starts empty; the person types it", Some("$s"), Some("start"), None, None)],
        vec![],
    );
}

#[test]
fn shape_derived_limit() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"derived":{"d0":"1","d1":"1","d2":"1","d3":"1","d4":"1","d5":"1","d6":"1","d7":"1","d8":"1","d9":"1","d10":"1","d11":"1","d12":"1","d13":"1","d14":"1","d15":"1","d16":"1","d17":"1","d18":"1","d19":"1","d20":"1","d21":"1","d22":"1","d23":"1","d24":"1","d25":"1","d26":"1","d27":"1","d28":"1","d29":"1","d30":"1","d31":"1","d32":"1"}}"#,
        vec![row("S512", "33 derived values; the most is 32", None, None, None, None)],
        vec![],
    );
}

#[test]
fn shape_values_limit() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"v0":{"start":1},"v1":{"start":1},"v2":{"start":1},"v3":{"start":1},"v4":{"start":1},"v5":{"start":1},"v6":{"start":1},"v7":{"start":1},"v8":{"start":1},"v9":{"start":1},"v10":{"start":1},"v11":{"start":1},"v12":{"start":1},"v13":{"start":1},"v14":{"start":1},"v15":{"start":1},"v16":{"start":1},"v17":{"start":1},"v18":{"start":1},"v19":{"start":1},"v20":{"start":1},"v21":{"start":1},"v22":{"start":1},"v23":{"start":1},"v24":{"start":1},"v25":{"start":1},"v26":{"start":1},"v27":{"start":1},"v28":{"start":1},"v29":{"start":1},"v30":{"start":1},"v31":{"start":1},"v32":{"start":1},"v33":{"start":1},"v34":{"start":1},"v35":{"start":1},"v36":{"start":1},"v37":{"start":1},"v38":{"start":1},"v39":{"start":1},"v40":{"start":1},"v41":{"start":1},"v42":{"start":1},"v43":{"start":1},"v44":{"start":1},"v45":{"start":1},"v46":{"start":1},"v47":{"start":1},"v48":{"start":1},"v49":{"start":1},"v50":{"start":1},"v51":{"start":1},"v52":{"start":1},"v53":{"start":1},"v54":{"start":1},"v55":{"start":1},"v56":{"start":1},"v57":{"start":1},"v58":{"start":1},"v59":{"start":1},"v60":{"start":1},"v61":{"start":1},"v62":{"start":1},"v63":{"start":1},"v64":{"start":1}}}"#,
        vec![row("S513", "65 values; the most is 64", None, None, None, None)],
        vec![],
    );
}

#[test]
fn shape_runs_limit() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r0":{"kind":"cmd","cmd":"true"},"r1":{"kind":"cmd","cmd":"true"},"r2":{"kind":"cmd","cmd":"true"},"r3":{"kind":"cmd","cmd":"true"},"r4":{"kind":"cmd","cmd":"true"},"r5":{"kind":"cmd","cmd":"true"},"r6":{"kind":"cmd","cmd":"true"},"r7":{"kind":"cmd","cmd":"true"},"r8":{"kind":"cmd","cmd":"true"},"r9":{"kind":"cmd","cmd":"true"},"r10":{"kind":"cmd","cmd":"true"},"r11":{"kind":"cmd","cmd":"true"},"r12":{"kind":"cmd","cmd":"true"},"r13":{"kind":"cmd","cmd":"true"},"r14":{"kind":"cmd","cmd":"true"},"r15":{"kind":"cmd","cmd":"true"},"r16":{"kind":"cmd","cmd":"true"}}}"#,
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
fn shape_pieces_limit() {
    check(
        r#"{"schema":2,"root":"top","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"top":{"type":"column","children":["t0","t1","t2","t3","t4","t5","t6","t7","t8","t9","t10","t11","t12","t13","t14","t15","t16","t17","t18","t19","t20","t21","t22","t23","t24","t25","t26","t27","t28","t29","t30","t31","t32","t33","t34","t35","t36","t37","t38","t39","t40","t41","t42","t43","t44","t45","t46","t47","t48","t49","t50","t51","t52","t53","t54","t55","t56","t57","t58","t59","t60","t61","t62","t63","t64","t65","t66","t67","t68","t69","t70","t71","t72","t73","t74","t75","t76","t77","t78","t79","t80","t81","t82","t83","t84","t85","t86","t87","t88","t89","t90","t91","t92","t93","t94","t95","t96","t97","t98","t99","t100"]},"t0":{"type":"text","props":{"value":"x"}},"t1":{"type":"text","props":{"value":"x"}},"t2":{"type":"text","props":{"value":"x"}},"t3":{"type":"text","props":{"value":"x"}},"t4":{"type":"text","props":{"value":"x"}},"t5":{"type":"text","props":{"value":"x"}},"t6":{"type":"text","props":{"value":"x"}},"t7":{"type":"text","props":{"value":"x"}},"t8":{"type":"text","props":{"value":"x"}},"t9":{"type":"text","props":{"value":"x"}},"t10":{"type":"text","props":{"value":"x"}},"t11":{"type":"text","props":{"value":"x"}},"t12":{"type":"text","props":{"value":"x"}},"t13":{"type":"text","props":{"value":"x"}},"t14":{"type":"text","props":{"value":"x"}},"t15":{"type":"text","props":{"value":"x"}},"t16":{"type":"text","props":{"value":"x"}},"t17":{"type":"text","props":{"value":"x"}},"t18":{"type":"text","props":{"value":"x"}},"t19":{"type":"text","props":{"value":"x"}},"t20":{"type":"text","props":{"value":"x"}},"t21":{"type":"text","props":{"value":"x"}},"t22":{"type":"text","props":{"value":"x"}},"t23":{"type":"text","props":{"value":"x"}},"t24":{"type":"text","props":{"value":"x"}},"t25":{"type":"text","props":{"value":"x"}},"t26":{"type":"text","props":{"value":"x"}},"t27":{"type":"text","props":{"value":"x"}},"t28":{"type":"text","props":{"value":"x"}},"t29":{"type":"text","props":{"value":"x"}},"t30":{"type":"text","props":{"value":"x"}},"t31":{"type":"text","props":{"value":"x"}},"t32":{"type":"text","props":{"value":"x"}},"t33":{"type":"text","props":{"value":"x"}},"t34":{"type":"text","props":{"value":"x"}},"t35":{"type":"text","props":{"value":"x"}},"t36":{"type":"text","props":{"value":"x"}},"t37":{"type":"text","props":{"value":"x"}},"t38":{"type":"text","props":{"value":"x"}},"t39":{"type":"text","props":{"value":"x"}},"t40":{"type":"text","props":{"value":"x"}},"t41":{"type":"text","props":{"value":"x"}},"t42":{"type":"text","props":{"value":"x"}},"t43":{"type":"text","props":{"value":"x"}},"t44":{"type":"text","props":{"value":"x"}},"t45":{"type":"text","props":{"value":"x"}},"t46":{"type":"text","props":{"value":"x"}},"t47":{"type":"text","props":{"value":"x"}},"t48":{"type":"text","props":{"value":"x"}},"t49":{"type":"text","props":{"value":"x"}},"t50":{"type":"text","props":{"value":"x"}},"t51":{"type":"text","props":{"value":"x"}},"t52":{"type":"text","props":{"value":"x"}},"t53":{"type":"text","props":{"value":"x"}},"t54":{"type":"text","props":{"value":"x"}},"t55":{"type":"text","props":{"value":"x"}},"t56":{"type":"text","props":{"value":"x"}},"t57":{"type":"text","props":{"value":"x"}},"t58":{"type":"text","props":{"value":"x"}},"t59":{"type":"text","props":{"value":"x"}},"t60":{"type":"text","props":{"value":"x"}},"t61":{"type":"text","props":{"value":"x"}},"t62":{"type":"text","props":{"value":"x"}},"t63":{"type":"text","props":{"value":"x"}},"t64":{"type":"text","props":{"value":"x"}},"t65":{"type":"text","props":{"value":"x"}},"t66":{"type":"text","props":{"value":"x"}},"t67":{"type":"text","props":{"value":"x"}},"t68":{"type":"text","props":{"value":"x"}},"t69":{"type":"text","props":{"value":"x"}},"t70":{"type":"text","props":{"value":"x"}},"t71":{"type":"text","props":{"value":"x"}},"t72":{"type":"text","props":{"value":"x"}},"t73":{"type":"text","props":{"value":"x"}},"t74":{"type":"text","props":{"value":"x"}},"t75":{"type":"text","props":{"value":"x"}},"t76":{"type":"text","props":{"value":"x"}},"t77":{"type":"text","props":{"value":"x"}},"t78":{"type":"text","props":{"value":"x"}},"t79":{"type":"text","props":{"value":"x"}},"t80":{"type":"text","props":{"value":"x"}},"t81":{"type":"text","props":{"value":"x"}},"t82":{"type":"text","props":{"value":"x"}},"t83":{"type":"text","props":{"value":"x"}},"t84":{"type":"text","props":{"value":"x"}},"t85":{"type":"text","props":{"value":"x"}},"t86":{"type":"text","props":{"value":"x"}},"t87":{"type":"text","props":{"value":"x"}},"t88":{"type":"text","props":{"value":"x"}},"t89":{"type":"text","props":{"value":"x"}},"t90":{"type":"text","props":{"value":"x"}},"t91":{"type":"text","props":{"value":"x"}},"t92":{"type":"text","props":{"value":"x"}},"t93":{"type":"text","props":{"value":"x"}},"t94":{"type":"text","props":{"value":"x"}},"t95":{"type":"text","props":{"value":"x"}},"t96":{"type":"text","props":{"value":"x"}},"t97":{"type":"text","props":{"value":"x"}},"t98":{"type":"text","props":{"value":"x"}},"t99":{"type":"text","props":{"value":"x"}},"t100":{"type":"text","props":{"value":"x"}}}}"#,
        vec![row("D207", "top has 101 children; the most is 100", Some("top"), None, Some("bind a list instead"), None)],
        vec![],
    );
}

#[test]
fn shape_html_habit_p() {
    check(
        r#"{"schema":2,"root":"c","pieces":{"c":{"type":"column","children":["p1"]},"p1":{"type":"p","props":{"value":"Waiting"}}}}"#,
        vec![row("T300", "\"p\" is not a piece; words go in <text>...</text>", Some("p1"), None, None, None)],
        vec![],
    );
}

#[test]
fn shape_html_habit_a() {
    check(
        r#"{"schema":2,"root":"c","pieces":{"c":{"type":"column","children":["a1"]},"a1":{"type":"a","props":{"value":"x"}}}}"#,
        vec![row("T300", "\"a\" is not a piece; a link is a <button> whose onPress opens it", Some("a1"), None, None, None)],
        vec![],
    );
}

#[test]
fn shape_piece_id_digit_start() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]},"9lives":{"type":"text","props":{"value":"x"}}}}"#,
        vec![row(
            "P103",
            "\"9lives\" is not a piece id: letters, digits, dashes and _, starting with a letter",
            Some("9lives"),
            None,
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_step_set_path_not_own() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"x":{"start":1}},"reactions":[{"on":{"change":["$x"]},"do":[{"do":"set","path":"x","value":1}]}]}"#,
        vec![row("A601", "set writes the slate's own values: a $name path, not x", Some("reaction-1"), Some("do"), None, None)],
        vec![],
    );
}

#[test]
fn shape_step_send_formula_with() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"x":{"start":1}},"runs":{"r":{"kind":"cmd","cmd":"true","every":60}},"reactions":[{"on":{"done":"r"},"do":[{"do":"send","text":"hi","with":["1 + 2"]}]}]}"#,
        vec![row("A601", "send carries paths after its text; 1 + 2 is a formula", Some("reaction-1"), Some("do"), None, None)],
        vec![],
    );
}

#[test]
fn shape_run_cmd_empty() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"kind":"cmd","cmd":""}}}"#,
        vec![row(
            "K700",
            "the command is literal text; hand values to it with env={{ NAME: $value }}",
            Some("$r"),
            Some("cmd"),
            None,
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
fn shape_run_tool_no_server() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"kind":"tool","server":"","tool":"nodot"}}}"#,
        vec![row("K705", "tool names a server and a tool as \"server.tool\"", Some("$r"), Some("tool"), None, None)],
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
fn kit_run_alway_key() {
    check(r#"{"schema":2,"root":"c","runs":{"r":{"kind":"cmd","cmd":"true","every":60,"alway":true}},"pieces":{"c":{"type":"column"}}}"#, vec![row("K704", "$r has no alway; it takes kind, cmd, env, args, stdin, on, cwd, timeout, stream, confirm, every, once, always, then and description", Some("$r"), Some("alway"), None, None)], vec![]);
}

#[test]
fn kit_run_timer_not_set() {
    check(
        r#"{"schema":2,"root":"c","runs":{"r":{"kind":"cmd","cmd":"true","every":60}},"pieces":{"c":{"type":"column"}}}"#,
        vec![],
        vec![],
    );
}

#[test]
fn extension_fallback_self() {
    check(
        r#"{"schema":2,"root":"top","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"top":{"type":"column","children":["a"]},"a":{"type":"sparkle","fallback":"a"}}}"#,
        vec![row("D205", "a is under itself: top > a > a", Some("a"), None, None, None)],
        vec![],
    );
}

#[test]
fn extension_fallback_gone() {
    check(
        r#"{"schema":2,"root":"top","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"top":{"type":"column","children":["a"]},"a":{"type":"sparkle","fallback":"gone"}}}"#,
        vec![row("D203", "fallback names gone, which is not a piece", Some("a"), Some("fallback"), None, None)],
        vec![],
    );
}

#[test]
fn extension_fallback_cycle() {
    check(
        r#"{"schema":2,"root":"top","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"top":{"type":"column","children":["a"]},"a":{"type":"sparkle","fallback":"b"},"b":{"type":"sparkle","fallback":"a"}}}"#,
        vec![row("D205", "a is under itself: top > a > b > a", Some("a"), None, None, None)],
        vec![],
    );
}

#[test]
fn extension_fallback_text_no_value() {
    check(
        r#"{"schema":2,"root":"top","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"top":{"type":"column","children":["a"]},"a":{"type":"sparkle","fallback":"b"},"b":{"type":"text","props":{}}}}"#,
        vec![row("T304", "text \"b\" needs value", Some("b"), Some("value"), None, None)],
        vec![],
    );
}

#[test]
fn extension_fallback_ok() {
    check(
        r#"{"schema":2,"root":"top","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"top":{"type":"column","children":["a"]},"a":{"type":"sparkle","fallback":"b"},"b":{"type":"text","props":{"value":"Hi"}}}}"#,
        vec![],
        vec![],
    );
}

#[test]
fn shape_document_over_cap() {
    check_big_start(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"big":{"start":""}}}"#,
        262200,
        vec![
            row("D208", "the document is over 64 KB", None, None, None, None),
            row("S500", "the values' starts are over 256 KB", None, None, None, None),
        ],
        vec![],
    );
}

#[test]
fn block_w019_dead() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"tail":{"kind":"cmd","cmd":"tail -n 100 -F logs/error.log","once":true,"stream":true}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["output-1"]},"output-1":{"type":"output","props":{"run":"$tail","lines":40}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $tail: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($tail)} />"),
            None,
        )],
    );
}

#[test]
fn block_w019_started_button() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"tail":{"kind":"cmd","cmd":"tail logs/error.log"}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["output-1","button-1"]},"output-1":{"type":"output","props":{"run":"$tail"}},"button-1":{"type":"button","props":{"label":"Tail"},"on":{"press":[{"do":"start","run":"tail"}]}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w019_started_when() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"go":{"start":0}},"derived":{},"runs":{"tail":{"kind":"cmd","cmd":"tail logs/error.log"}},"reactions":[{"on":{"change":["$go"]},"do":[{"do":"start","run":"tail"}]}],"pieces":{"column-1":{"type":"column","children":["output-1","text-1"]},"output-1":{"type":"output","props":{"run":"$tail"}},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w019_started_row_action() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"tail":{"kind":"cmd","cmd":"tail logs/error.log"}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["output-1","table-1"]},"output-1":{"type":"output","props":{"run":"$tail"}},"table-1":{"type":"table","props":{"items":[{"bind":"{ n: 1 }"}],"columns":[{"title":"N","value":{"bind":"item.n"}}],"rowActions":[{"label":"Tail","on":{"press":[{"do":"start","run":"tail"}]}}]}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w019_started_input_submit() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"p":{"start":""}},"derived":{},"runs":{"tail":{"kind":"cmd","cmd":"tail logs/error.log"}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["output-1","input-1"]},"output-1":{"type":"output","props":{"run":"$tail"}},"input-1":{"type":"input","props":{"label":"Path","value":{"bind":"$p"}},"on":{"submit":[{"do":"start","run":"tail"}]}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w019_every() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"tail":{"kind":"cmd","cmd":"tail logs/error.log","every":60}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["output-1"]},"output-1":{"type":"output","props":{"run":"$tail"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w011_corrected() {
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
fn block_w011_uncorrected() {
    check(r#"{"schema":2,"root":"column-1","values":{"step":{"start":1},"repo":{"start":"acme/landing"},"project":{"start":""},"vercelToken":{"start":null,"secret":true}},"derived":{"checked":"$check.state == 'done' and contains($check.out, $project)","written":"$env.state == 'done' and $ci.state == 'done'"},"runs":{"check":{"kind":"cmd","cmd":"vercel project inspect \"$PROJECT\" --token \"$VERCEL_TOKEN\" 2>&1 | head -c 4000","env":{"PROJECT":{"bind":"$project"},"VERCEL_TOKEN":{"bind":"$vercelToken"}},"on":"host","timeout":30},"env":{"kind":"cmd","cmd":"grep -v \"^VERCEL_TOKEN=\" .env > .env.tmp 2>/dev/null; printf \"VERCEL_TOKEN=%s\\n\" \"$VERCEL_TOKEN\" >> .env.tmp; mv .env.tmp .env","env":{"VERCEL_TOKEN":{"bind":"$vercelToken"}},"on":"host","timeout":10},"ci":{"kind":"cmd","cmd":"gh secret set VERCEL_TOKEN --repo \"$REPO\" --body \"$VERCEL_TOKEN\"","env":{"REPO":{"bind":"$repo"},"VERCEL_TOKEN":{"bind":"$vercelToken"}},"on":"host","timeout":60}},"reactions":[{"on":{"change":["$project"]},"do":[{"do":"start","run":"check"}]},{"on":{"change":["$checked"]},"do":[{"do":"set","path":"$step","value":{"bind":"$checked ? 3 : 2"}}]},{"on":{"change":["$written"]},"do":[{"do":"set","path":"$step","value":{"bind":"$written ? 4 : 3"}}]}],"pieces":{"column-1":{"type":"column","children":["text-1","section-1","section-2","section-3","section-4"]},"text-1":{"type":"text","props":{"emphasis":"strong","value":{"format":"Step ${$step} of 4"}}},"section-1":{"type":"section","when":"$step == 1","props":{"title":"1. The Vercel token"},"children":["markdown-1","button-1","input-1","button-2"]},"markdown-1":{"type":"markdown","props":{"value":"Open the Vercel dashboard, make a token with the project's scope, and paste it here. It never reaches the agent."}},"button-1":{"type":"button","props":{"label":"Open Vercel tokens"},"on":{"press":[{"do":"open","target":"https://vercel.com/account/tokens"}]}},"input-1":{"type":"input","props":{"label":"Vercel token","value":{"bind":"$vercelToken"},"kind":"password"}},"button-2":{"type":"button","when":"$vercelToken.set","props":{"label":"Next","variant":"primary"},"on":{"press":[{"do":"set","path":"$step","value":2}]}},"section-2":{"type":"section","when":"$step == 2 or $step == 3","props":{"title":"2. The project"},"children":["project-name","text-2","text-3","text-4","text-5"]},"project-name":{"type":"input","props":{"label":"Project name on Vercel","value":{"bind":"$project"},"mono":true}},"text-2":{"type":"text","when":"$check.state == 'held'","props":{"tone":"muted","value":"Approve the check to go on"}},"text-3":{"type":"text","when":"$check.state == 'running'","props":{"tone":"muted","value":"Checking"}},"text-4":{"type":"text","when":"$check.state == 'failed'","props":{"tone":"bad","value":{"format":"Vercel did not find it: ${short($check.err, 160)}"}}},"text-5":{"type":"text","when":"$checked","props":{"tone":"good","value":{"format":"Found ${$project}"}}},"section-3":{"type":"section","when":"$step == 3","props":{"title":"3. Write it down"},"children":["facts-1","row-1","text-6","text-7","text-8"]},"facts-1":{"type":"facts","props":{"facts":[{"label":"Repo","value":{"bind":"$repo"},"mono":true},{"label":"Project","value":{"bind":"$project"},"mono":true}]}},"row-1":{"type":"row","children":["button-3","button-4"]},"button-3":{"type":"button","when":"$env.state != 'done'","props":{"label":"Write .env"},"on":{"press":[{"do":"start","run":"env"}]}},"button-4":{"type":"button","when":"$ci.state != 'done'","props":{"label":"Set the CI secret"},"on":{"press":[{"do":"start","run":"ci"}]}},"text-6":{"type":"text","when":"$env.state == 'done'","props":{"tone":"good","value":".env written"}},"text-7":{"type":"text","when":"$ci.state == 'done'","props":{"tone":"good","value":"CI secret set"}},"text-8":{"type":"text","when":"$env.state == 'failed' or $ci.state == 'failed'","props":{"tone":"bad","value":{"format":"${short(concat($env.err, $ci.err), 200)}"}}},"section-4":{"type":"section","when":"$step == 4","props":{"title":"4. Done"},"children":["text-9","write"]},"text-9":{"type":"text","props":{"tone":"good","value":"The token is in .env and in the repo's secrets."}},"write":{"type":"button","props":{"label":"Tell the agent","variant":"primary"},"on":{"press":[{"do":"send","text":"The Vercel setup is done on the slate; wire the deploy hook next.","with":["$repo","$project","$vercelToken"]}]}}},"title":"Deploy setup"}"#, vec![], vec![row("W011", "VERCEL_TOKEN carries the secret $vercelToken, and the command passes it after --token, as an argument ps can read; hand it on stdin={$vercelToken}, or let the program read VERCEL_TOKEN from its environment", Some("$check"), Some("cmd"), Some("drop --token \"$VERCEL_TOKEN\" and use stdin={$vercelToken} or the program's own VERCEL_TOKEN"), None), row("W011", "VERCEL_TOKEN carries the secret $vercelToken, and the command passes it after --body, as an argument ps can read; hand it on stdin={$vercelToken}, or let the program read VERCEL_TOKEN from its environment", Some("$ci"), Some("cmd"), Some("drop --body \"$VERCEL_TOKEN\" and use stdin={$vercelToken} or the program's own VERCEL_TOKEN"), None), row("T311", "primary on button-2, write; one per slate reads loud, the rest draw as default", Some("write"), None, None, None)]);
}

#[test]
fn block_w011_body_flag() {
    check(r#"{"schema":2,"root":"column-1","values":{"t":{"start":null,"secret":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"gh x --body=$T","env":{"T":{"bind":"$t"}}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#, vec![], vec![row("W011", "T carries the secret $t, and the command passes it after --body, as an argument ps can read; hand it on stdin={$t}, or let the program read T from its environment", Some("$r"), Some("cmd"), Some("drop --body \"$T\" and use stdin={$t} or the program's own T"), None), row("W019", "nothing starts $r: give it a button, a when, or every", None, None, Some("<button label=\"...\" onPress={start($r)} />"), None)]);
}

#[test]
fn block_w011_vercel_argument() {
    check(r#"{"schema":2,"root":"column-1","values":{"t":{"start":null,"secret":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"vercel ls -p $T","env":{"T":{"bind":"$t"}}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#, vec![], vec![row("W011", "T carries the secret $t, and the command passes it after -p, as an argument ps can read; hand it on stdin={$t}, or let the program read T from its environment", Some("$r"), Some("cmd"), Some("drop -p \"$T\" and use stdin={$t} or the program's own T"), None), row("W019", "nothing starts $r: give it a button, a when, or every", None, None, Some("<button label=\"...\" onPress={start($r)} />"), None)]);
}

#[test]
fn block_w011_printf_ok() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"t":{"start":null,"secret":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"printf \"%s\" \"$T\" > f","env":{"T":{"bind":"$t"}}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
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
fn block_w012_0() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"Market closed · Weekend close"}}},"title":"T"}"#,
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
fn block_w012_1() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["facts-1"]},"facts-1":{"type":"facts","props":{"facts":[{"label":"Purity","value":"24K · 99.9% pure"}]}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W012",
            "value joins words with \"·\"; the kit separates things by layout, never a mark: give each its own piece, or join with a comma",
            Some("facts-1"),
            Some("facts[0].value"),
            Some("separate pieces (facts, chips, a row) or a comma"),
            None,
        )],
    );
}

#[test]
fn block_w012_2() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"Open • Closed"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W012",
            "value joins words with \"•\"; the kit separates things by layout, never a mark: give each its own piece, or join with a comma",
            Some("text-1"),
            Some("value"),
            Some("separate pieces (facts, chips, a row) or a comma"),
            None,
        )],
    );
}

#[test]
fn block_w012_3() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"Spot | Ask"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W012",
            "value joins words with \"|\"; the kit separates things by layout, never a mark: give each its own piece, or join with a comma",
            Some("text-1"),
            Some("value"),
            Some("separate pieces (facts, chips, a row) or a comma"),
            None,
        )],
    );
}

#[test]
fn block_w012_4() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${`${$a} · ${$a}`}"}}}},"title":"T"}"#,
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
fn block_w012_5() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"Closed · ${$a}"}}}},"title":"T"}"#,
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
fn block_w012_6() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${join($xs, \" · \")}"}}}},"title":"T"}"#,
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
fn block_w012_7() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${join($xs, \" | \")}"}}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W012",
            "value joins words with \"|\"; the kit separates things by layout, never a mark: give each its own piece, or join with a comma",
            Some("text-1"),
            Some("value"),
            Some("separate pieces (facts, chips, a row) or a comma"),
            None,
        )],
    );
}

#[test]
fn block_w012_8() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"Market closed, weekend close"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w012_9() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${$xs | take(1) | join(\", \")}"}}}},"title":"T"}"#,
        vec![row(
            "Q423",
            "join takes 2 to 3 arguments, got 1: join(other, key, otherKey?)",
            Some("text-1"),
            Some("value"),
            Some("join(other, key, otherKey?)"),
            None,
        )],
        vec![],
    );
}

#[test]
fn block_w012_10() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["markdown-1"]},"markdown-1":{"type":"markdown","props":{"value":"| a | b |\n| - | - |\n| 1 | 2 |"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w012_11() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"• first"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w013_index() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"hits":{"start":[{"n":1,"at":1}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["chart-1"]},"chart-1":{"type":"chart","props":{"label":"Requests","items":{"bind":"$hits"},"x":{"bind":"index"},"value":{"bind":"item.n"}}}},"title":"T"}"#,
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
fn block_w013_none() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"hits":{"start":[{"n":1,"at":1}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["chart-1"]},"chart-1":{"type":"chart","props":{"label":"Requests","items":{"bind":"$hits"},"value":{"bind":"item.n"}}}},"title":"T"}"#,
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
fn block_w013_at_clean() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"hits":{"start":[{"n":1,"at":1}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["chart-1"]},"chart-1":{"type":"chart","props":{"label":"Requests","items":{"bind":"$hits"},"x":{"bind":"item.at"},"value":{"bind":"item.n"}}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_always_without_every() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"true","always":true}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row(
            "K703",
            "always keeps a timer running while the slate is not shown, so it needs every",
            Some("$r"),
            Some("always"),
            Some("every={60} always"),
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
fn block_always_with_every() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"true","every":60,"always":true}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_confirm_formula() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":19271},"name":{"start":"node"}},"derived":{},"runs":{"kill":{"kind":"cmd","cmd":"kill \"$PID\"","confirm":{"format":"Kill ${$name} (PID ${$pid})?"},"env":{"PID":{"bind":"$pid"}}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["button-1"]},"button-1":{"type":"button","props":{"label":"Kill"},"on":{"press":[{"do":"start","run":"kill"}]}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_confirm_ternary() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"ok":{"start":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"true","confirm":{"bind":"$ok ? 'Again?' : 'Run it?'"}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
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
fn block_confirm_literal() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"true","confirm":"Run it?"}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
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
fn block_confirm_unknown_value() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"true","confirm":{"bind":"$nope"}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row("S501", "$nope is not declared; add <value name=\"nope\" start=... />", Some("$r"), Some("confirm"), None, None)],
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
fn block_confirm_secret() {
    check(r#"{"schema":2,"root":"column-1","values":{"t":{"start":null,"secret":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"true","confirm":{"bind":"$t"}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#, vec![row("S520", "$t is a secret; only a run's env or stdin, an input's value or a send's paths may name it, and a formula reads only its .set, .len and .at", Some("$r"), Some("confirm"), Some("$t.set"), None)], vec![row("W019", "nothing starts $r: give it a button, a when, or every", None, None, Some("<button label=\"...\" onPress={start($r)} />"), None)]);
}

#[test]
fn block_when_on_fact() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"all":{"start":false},"pick":{"start":null},"rows":{"start":[{"name":"a","pid":1}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["facts-1"]},"facts-1":{"type":"facts","props":{"facts":[{"label":"PID","value":"1","when":"$all"}]}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_when_on_cols_and_action() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"all":{"start":false},"pick":{"start":null},"rows":{"start":[{"name":"a","pid":1}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["table-1"]},"table-1":{"type":"table","props":{"items":{"bind":"$rows"},"columns":[{"title":"Name","value":{"bind":"item.name"}},{"title":"PID","value":{"bind":"item.pid"},"when":"$all"}],"rowActions":[{"label":"Kill","when":"item.pid > 0","on":{"press":[{"do":"send","text":"Kill it.","with":["item.pid"]}]}}]}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_when_on_option() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"all":{"start":false},"pick":{"start":null},"rows":{"start":[{"name":"a","pid":1}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["select-1"]},"select-1":{"type":"select","props":{"label":"Pick","value":{"bind":"$pick"},"options":[{"value":"a"},{"value":"b","when":"$all"}]}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_when_on_choice() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"all":{"start":false},"pick":{"start":null},"rows":{"start":[{"name":"a","pid":1}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["choices-1"]},"choices-1":{"type":"choices","props":{"label":"Pick","value":{"bind":"$pick"},"options":[{"value":"a"},{"value":"b","when":"$all"}]}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_when_row_col_per_row() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"all":{"start":false},"pick":{"start":null},"rows":{"start":[{"name":"a","pid":1}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["table-1"]},"table-1":{"type":"table","props":{"items":{"bind":"$rows"},"columns":[{"title":"PID","value":{"bind":"item.pid"},"when":"item.pid > 0"}]}}},"title":"T"}"#,
        vec![row("X409", "item is only known inside a repeating piece or a step", Some("table-1"), Some("columns[0].when"), None, None)],
        vec![],
    );
}

#[test]
fn block_section_literal_open() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["section-1"]},"section-1":{"type":"section","props":{"title":"More","collapsible":true,"open":false},"children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_section_state_open() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"open":{"start":true}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["section-1"]},"section-1":{"type":"section","props":{"title":"More","collapsible":true,"open":{"bind":"$open"}},"children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_section_open_string() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["section-1"]},"section-1":{"type":"section","props":{"title":"More","open":"no"},"children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row(
            "X410",
            "open on section is two-way and binds a value: open={$name}",
            Some("section-1"),
            Some("open"),
            Some("open={$no}"),
            None,
        )],
        vec![],
    );
}

#[test]
fn block_toggle_literal_value() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["toggle-1"]},"toggle-1":{"type":"toggle","props":{"label":"On","value":true}}},"title":"T"}"#,
        vec![row(
            "X410",
            "value on toggle is two-way and binds a value: value={$name}",
            Some("toggle-1"),
            Some("value"),
            Some("value={$name}"),
            None,
        )],
        vec![],
    );
}

#[test]
fn block_camel_ids() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"spot":{"kind":"cmd","cmd":"echo '{}'"}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["goldPrice","spot","spot_at"]},"goldPrice":{"type":"number","props":{"label":"Gold","value":{"bind":"$spot.json.v"}}},"spot":{"type":"text","props":{"value":"x"}},"spot_at":{"type":"text","props":{"value":"y"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $spot: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($spot)} />"),
            None,
        )],
    );
}

#[test]
fn block_w014_sentence_text() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":1},"rows":{"start":[{"n":"a"}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"mono":true,"value":"Kill the node process now"}}},"title":"T"}"#,
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
fn block_w014_sentence_formula() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":1},"rows":{"start":[{"n":"a"}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"mono":true,"value":{"format":"${$pid} processes are running here"}}}},"title":"T"}"#,
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
fn block_w014_sentence_fact() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":1},"rows":{"start":[{"n":"a"}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["facts-1"]},"facts-1":{"type":"facts","props":{"facts":[{"label":"Note","value":"the market is closed today","mono":true}]}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W014",
            "mono is for figures, ids, times and paths; this reads as a sentence, which reads better in the normal face",
            Some("facts-1"),
            Some("facts[0].mono"),
            Some("drop mono"),
            None,
        )],
    );
}

#[test]
fn block_w014_sentence_row() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":1},"rows":{"start":[{"n":"a"}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["table-1"]},"table-1":{"type":"table","props":{"items":{"bind":"$rows"},"columns":[{"title":"Why","value":{"format":"waiting on the ${item.n} lock"},"mono":true}]}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W014",
            "mono is for figures, ids, times and paths; this reads as a sentence, which reads better in the normal face",
            Some("table-1"),
            Some("columns[0].mono"),
            Some("drop mono"),
            None,
        )],
    );
}

#[test]
fn block_w014_figure() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":1},"rows":{"start":[{"n":"a"}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"mono":true,"value":{"format":"${$pid}"}}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w014_time() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":1},"rows":{"start":[{"n":"a"}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"mono":true,"value":"2026-10-04 17:23 UTC"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w014_path() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":1},"rows":{"start":[{"n":"a"}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"mono":true,"value":"src/slate/kit.ts"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w014_pid_text() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":1},"rows":{"start":[{"n":"a"}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"mono":true,"value":"PID 19271"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w014_id_fact() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":1},"rows":{"start":[{"n":"a"}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["facts-1"]},"facts-1":{"type":"facts","props":{"facts":[{"label":"Head","value":"b5ebeeff4","mono":true}]}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_run_field_state() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.state, \"a\", \"b\")"}}}}}"#,
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
fn block_run_field_why() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.why, \"a\", \"b\")"}}}}}"#,
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
fn block_run_field_exit() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.exit, \"a\", \"b\")"}}}}}"#,
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
fn block_run_field_out() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.out, \"a\", \"b\")"}}}}}"#,
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
