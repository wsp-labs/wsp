// SPDX-License-Identifier: AGPL-3.0-only
//! The compiler's run, reaction and steps declarations.
use super::*;

impl Compiler {
    pub(super) fn run(&mut self, el: &El, name: &str) {
        let key = format!("${name}");
        for a in &el.attrs {
            self.lines.insert(format!("{key}.{}", a.name), a.line);
        }
        let attr = |n: &str| attr_of(el, n);
        let kinds: Vec<&str> =
            ["cmd", "tool", "resource"].into_iter().filter(|k| attr(k).is_some() || (*k == "cmd" && el.text.is_some())).collect();
        if attr("cmd").is_some() && el.text.is_some() {
            self.error(
                "K701",
                &format!("<run name=\"{name}\"> has cmd= and a command inside; keep one"),
                el.line,
                Ext { piece: Some(&key), ..Ext::default() },
            );
            return;
        }
        if kinds.len() != 1 {
            self.error(
                "K701",
                &format!("<run name=\"{name}\"> takes exactly one of cmd, tool or resource"),
                el.line,
                Ext { piece: Some(&key), ..Ext::default() },
            );
            return;
        }
        let mut common = Common::default();
        if let Some(every) = attr("every") {
            let s = seconds(every);
            if s.is_finite() {
                common.every = Some(JsNumber(s));
            } else {
                self.error(
                    "K703",
                    "every takes seconds: every=\"30s\" or every={30}",
                    every.line,
                    Ext { piece: Some(&key), prop: Some("every"), ..Ext::default() },
                );
            }
        }
        if attr("once").is_some() {
            common.once = Some(true);
        }
        if attr("always").is_some() {
            common.always = Some(true);
        }
        if let Some(c) = attr("confirm") {
            let v = if c.kind == Kind::Braced { value_text(&c.value) } else { Value::String(c.value.clone()) };
            if v.is_string() || is_slate_binding(&v) || format_text(&v).is_some() {
                common.confirm = Some(v);
            } else {
                self.error(
                    "T303",
                    "confirm is a sentence: confirm=\"Stop it?\" or confirm={`Kill ${$name}?`}",
                    c.line,
                    Ext { piece: Some(&key), prop: Some("confirm"), ..Ext::default() },
                );
            }
        }
        if let Some(t) = attr("then") {
            let text = match t.kind {
                Kind::Str => Some(t.value.clone()),
                Kind::Braced => command_text(&t.value),
                Kind::Bare => None,
            };
            match text {
                Some(s) if !js_trim(&s).is_empty() => common.then = Some(s),
                _ => self.error(
                    "K700",
                    "then is a literal command that reads the result on stdin: then='python3 shape.py'",
                    t.line,
                    Ext { piece: Some(&key), prop: Some("then"), ..Ext::default() },
                ),
            }
        }
        let kind = kinds[0];
        let mut description: Option<String> = None;
        if let Some(d) = attr("description") {
            if kind == "cmd" {
                self.error(
                    "K701",
                    "a cmd run takes no description: its sheet shows the command itself",
                    d.line,
                    Ext { piece: Some(&key), prop: Some("description"), ..Ext::default() },
                );
            } else if d.kind != Kind::Str || js_trim(&d.value).is_empty() {
                self.error(
                    "T303",
                    "description is one literal line: description=\"Reads the last hour of traffic\"",
                    d.line,
                    Ext { piece: Some(&key), prop: Some("description"), ..Ext::default() },
                );
            } else {
                description = Some(js_trim(&d.value).to_string());
            }
        }
        let decl = match kind {
            "cmd" => {
                let cmd = attr("cmd");
                let literal = match cmd {
                    None => command_body(el.text.as_deref().unwrap_or(&[])),
                    Some(c) if c.kind == Kind::Str => Some(c.value.clone()),
                    Some(c) if c.kind == Kind::Braced => command_text(&c.value),
                    Some(_) => None,
                };
                if literal.is_none() {
                    self.error(
                        "K700",
                        "the command is literal text: cmd=\"...\", cmd={\"...\"} with \\\" escapes, or <run name=\"x\">{`any text`}</run>; values reach it as env, args or stdin",
                        cmd.map_or(el.line, |c| c.line),
                        Ext {
                            piece: Some(&key),
                            prop: Some("cmd"),
                            fix: Some("cmd='gh api \"$URL\"' env={{ URL: $url }}"),
                        },
                    );
                }
                let mut r = SlateCmdDecl {
                    cmd: literal.unwrap_or_default(),
                    env: None,
                    args: None,
                    stdin: None,
                    on: None,
                    cwd: None,
                    timeout: None,
                    stream: None,
                    confirm: common.confirm.take(),
                    every: common.every.take(),
                    once: common.once.take(),
                    always: common.always.take(),
                    then: common.then.take(),
                };
                if let Some(env) = attr("env") {
                    r.env = self.object(env, &key);
                }
                if let Some(args) = attr("args") {
                    r.args = self.array(args, &key);
                }
                if let Some(stdin) = attr("stdin") {
                    r.stdin = Some(self.value(stdin));
                }
                if let Some(on) = attr("on") {
                    if on.kind != Kind::Str || (on.value != "thread" && on.value != "host") {
                        self.error(
                            "K704",
                            "on is \"thread\" or \"host\"",
                            on.line,
                            Ext { piece: Some(&key), prop: Some("on"), ..Ext::default() },
                        );
                    } else {
                        r.on = Some(on.value.clone());
                    }
                }
                if let Some(cwd) = attr("cwd") {
                    if cwd.kind != Kind::Str {
                        self.error(
                            "T303",
                            "cwd is a literal path",
                            cwd.line,
                            Ext { piece: Some(&key), prop: Some("cwd"), ..Ext::default() },
                        );
                    } else {
                        r.cwd = Some(cwd.value.clone());
                    }
                }
                if let Some(timeout) = attr("timeout") {
                    let s = seconds(timeout);
                    if s.is_finite() {
                        r.timeout = Some(JsNumber(s));
                    } else {
                        self.error(
                            "K704",
                            "timeout is seconds: timeout={20}",
                            timeout.line,
                            Ext { piece: Some(&key), prop: Some("timeout"), ..Ext::default() },
                        );
                    }
                }
                if attr("stream").is_some() {
                    r.stream = Some(true);
                }
                for n in ["tool", "resource"] {
                    if attr(n).is_some() {
                        self.error("K701", &format!("a cmd run takes no {n}"), el.line, Ext { piece: Some(&key), ..Ext::default() });
                    }
                }
                Some(SlateRunDecl::Cmd(r))
            }
            "tool" => {
                let Some(named) = attr("tool") else { return };
                let tool_value = match AGENT_TOOL.captures(&named.value) {
                    Some(c) if named.kind == Kind::Str => format!("{}.{}", &c[1], &c[2]),
                    _ => named.value.clone(),
                };
                let dot = tool_value.find('.');
                let bad = named.kind != Kind::Str || dot.is_none_or(|d| d == 0 || d == tool_value.len() - 1);
                if bad {
                    self.error(
                        "K705",
                        "tool names a server and a tool as \"server.tool\"",
                        named.line,
                        Ext { piece: Some(&key), prop: Some("tool"), ..Ext::default() },
                    );
                }
                let (server, tool) = match dot {
                    Some(d) => (tool_value[..d].to_string(), tool_value[d + 1..].to_string()),
                    None => (String::new(), tool_value.clone()),
                };
                let mut r = SlateToolDecl {
                    server,
                    tool,
                    args: None,
                    description: description.take(),
                    confirm: common.confirm.take(),
                    every: common.every.take(),
                    once: common.once.take(),
                    always: common.always.take(),
                    then: common.then.take(),
                };
                if let Some(args) = attr("args") {
                    r.args = self.object(args, &key);
                }
                for n in ["env", "stdin", "on", "cwd", "timeout", "stream"] {
                    if attr(n).is_some() {
                        self.error("K701", &format!("a tool run takes no {n}"), el.line, Ext { piece: Some(&key), ..Ext::default() });
                    }
                }
                Some(SlateRunDecl::Tool(r))
            }
            _ => {
                let Some(res) = attr("resource") else { return };
                let colon = res.value.find(':');
                if res.kind != Kind::Str || colon.is_none_or(|c| c == 0) {
                    self.error(
                        "K705",
                        "resource names a server and a uri as \"server:uri\"",
                        res.line,
                        Ext { piece: Some(&key), prop: Some("resource"), ..Ext::default() },
                    );
                }
                let (server, uri) = match colon {
                    Some(c) => (res.value[..c].to_string(), res.value[c + 1..].to_string()),
                    None => (String::new(), res.value.clone()),
                };
                let r = SlateResourceDecl {
                    server,
                    uri,
                    every: common.every.take(),
                    always: common.always.take(),
                    then: common.then.take(),
                    description: description.take(),
                };
                for n in ["env", "args", "stdin", "on", "cwd", "timeout", "stream", "confirm", "once"] {
                    if attr(n).is_some() {
                        self.error("K701", &format!("a resource run takes no {n}"), el.line, Ext { piece: Some(&key), ..Ext::default() });
                    }
                }
                Some(SlateRunDecl::Resource(r))
            }
        };
        if let Some(decl) = decl {
            self.doc.runs.insert(name.to_string(), decl);
        }
    }

    pub(super) fn reaction(&mut self, el: &El) {
        let change = attr_of(el, "change");
        let done = attr_of(el, "done");
        let id_str = attr_of(el, "id").filter(|a| a.kind == Kind::Str).map(|a| a.value.clone());
        let key = id_str.clone().unwrap_or_else(|| format!("reaction-{}", self.doc.reactions.len() + 1));
        self.lines.insert(key.clone(), el.line);
        if self.patch && id_str.is_none() {
            self.error(
                "P107",
                "a reaction in a patch carries an id, so a later patch can name it",
                el.line,
                Ext { fix: Some("<when id=\"check-on-id\" ... />"), ..Ext::default() },
            );
        }
        if change.is_none() == done.is_none() {
            self.error(
                "A611",
                "<when> takes exactly one of change={...} or done={$run}",
                el.line,
                Ext { piece: Some(&key), ..Ext::default() },
            );
            return;
        }
        let do_attr = match attr_of(el, "do") {
            Some(a) if a.kind == Kind::Braced => a,
            _ => {
                self.error(
                    "A601",
                    "when takes do={steps}",
                    el.line,
                    Ext { piece: Some(&key), fix: Some("<when change={$id} do={start($check)} />"), ..Ext::default() },
                );
                return;
            }
        };
        let on: Value = match (change, done) {
            (Some(c), _) => {
                if c.kind != Kind::Braced {
                    self.error(
                        "A601",
                        "change takes a path or a list of paths in braces: change={$id}",
                        c.line,
                        Ext { piece: Some(&key), ..Ext::default() },
                    );
                    return;
                }
                let text = js_trim(&c.value);
                let items =
                    if text.starts_with('[') && text.ends_with(']') { split_top(&text[1..text.len() - 1]) } else { vec![text.to_string()] };
                let mut paths: Vec<String> = Vec::new();
                for t in items {
                    match path_of(&t) {
                        Some(p) => paths.push(p),
                        None => self.error(
                            "A601",
                            &format!("change lists paths, not formulas: {}", js_trim(&t)),
                            c.line,
                            Ext { piece: Some(&key), ..Ext::default() },
                        ),
                    }
                }
                json!({"change": paths})
            }
            (None, Some(d)) => match slate_own_name_braced(d) {
                Some(run) => json!({"done": run}),
                None => {
                    self.error("K702", "done names a run: done={$check}", d.line, Ext { piece: Some(&key), ..Ext::default() });
                    return;
                }
            },
            (None, None) => return,
        };
        self.lines.insert(format!("{key}.do"), do_attr.line);
        let steps = self.steps(do_attr, &key);
        let mut r = Map::new();
        if let Some(id) = id_str {
            r.insert("id".into(), Value::String(id));
        }
        r.insert("on".into(), on);
        r.insert("do".into(), Value::Array(steps));
        self.doc.reactions.push(Value::Object(r));
    }

    fn step_bad(&mut self, a: &Attr, piece: &str, kind: &str, message: &str, fix: Option<&str>) {
        self.error("A601", &format!("{kind}: {message}"), a.line, Ext { piece: Some(piece), prop: Some(a.name.as_str()), fix });
    }

    pub(super) fn steps(&mut self, a: &Attr, piece: &str) -> Vec<Value> {
        let at = Ext { piece: Some(piece), prop: Some(a.name.as_str()), fix: None };
        if a.kind != Kind::Braced {
            self.error("A600", "a handler is a step or a list of steps in braces: onPress={set($x, 1)}", a.line, at);
            return Vec::new();
        }
        let mut text = js_trim(&a.value).to_string();
        if HANDLER_FN.is_match(&text) || text.starts_with("function") {
            let body = HANDLER_BODY.replace(&text, "").into_owned();
            let fix = format!("{}={{{}}}", a.name, body);
            self.error("A600", "a handler is a step or a list of steps, not a function", a.line, Ext { fix: Some(&fix), ..at });
            return Vec::new();
        }
        let list = text.starts_with('[') && text.ends_with(']');
        if list {
            text = text[1..text.len() - 1].to_string();
        }
        let calls = if list { split_top(&text) } else { vec![text.clone()] };
        let names = step_names();
        let mut out = Vec::new();
        for call in calls {
            let Some(m) = STEP_CALL.captures(&call) else {
                self.error("A600", &format!("\"{}\" is not a step; steps are {}", js_trim(&call), names.join(", ")), a.line, at);
                continue;
            };
            let kind = m[1].to_string();
            if slate_step_module(&kind).is_none() {
                let fix = if kind == "setState" {
                    Some("set".to_string())
                } else if kind == "navigate" || kind == "openUrl" {
                    Some("open".to_string())
                } else {
                    slate_nearest(&kind, &names)
                };
                self.error(
                    "A600",
                    &format!("{kind} is not a step; steps are {}", names.join(", ")),
                    a.line,
                    Ext { fix: fix.as_deref(), ..at },
                );
                continue;
            }
            let args: Vec<String> = split_top(&m[2]).iter().map(|s| js_trim(s).to_string()).collect();
            let own = |s: Option<&String>| s.filter(|s| parse_slate_own_path(s).is_some()).cloned();
            match kind.as_str() {
                "set" => match own(args.first()) {
                    Some(path) if args.len() == 2 => out.push(json!({"do": "set", "path": path, "value": value_text(&args[1])})),
                    _ => self.step_bad(a, piece, &kind, "set writes one of the slate's values: set($name, value)", Some("set($step, 2)")),
                },
                "toggle" => match own(args.first()) {
                    Some(path) if args.len() == 1 => out.push(json!({"do": "toggle", "path": path})),
                    _ => self.step_bad(a, piece, &kind, "toggle flips one of the slate's values: toggle($name)", None),
                },
                "start" | "cancel" => match slate_own_name(args.first().map_or("", String::as_str)) {
                    Some(run) if args.len() == 1 => out.push(json!({"do": kind, "run": run})),
                    _ => self.step_bad(a, piece, &kind, &format!("{kind} names a run: {kind}($check)"), None),
                },
                "send" | "steer" | "queue" | "fill" => {
                    let Some(text) = args.first().and_then(|s| string_literal(s)) else {
                        let fix = format!("{kind}(\"What to do.\", $path)");
                        self.error(
                            "A603",
                            &format!("{kind}'s text is a literal string; put the changing parts after it as paths"),
                            a.line,
                            Ext { piece: Some(piece), prop: Some(a.name.as_str()), fix: Some(&fix) },
                        );
                        continue;
                    };
                    let mut paths: Vec<String> = Vec::new();
                    for p in args.iter().skip(1) {
                        match path_of(p) {
                            Some(path) => paths.push(path),
                            None => self.step_bad(
                                a,
                                piece,
                                &kind,
                                &format!("{p} is not a path; {kind} carries paths after its text, as data"),
                                None,
                            ),
                        }
                    }
                    let mut step = json!({"do": kind, "text": text});
                    if !paths.is_empty() {
                        step["with"] = json!(paths);
                    }
                    out.push(step);
                }
                "open" | "copy" => {
                    if args.len() != 1 {
                        self.step_bad(a, piece, &kind, &format!("{kind} takes one value"), None);
                        continue;
                    }
                    let v = value_text(&args[0]);
                    out.push(if kind == "open" { json!({"do": "open", "target": v}) } else { json!({"do": "copy", "text": v}) });
                }
                "pane" => {
                    let p = args.first().and_then(|s| string_literal(s));
                    match p {
                        Some(p) if SLATE_PANE_KINDS.contains(&p.as_str()) => out.push(json!({"do": "pane", "kind": p})),
                        other => {
                            let fix = slate_nearest(other.as_deref().unwrap_or(""), &strs(SLATE_PANE_KINDS));
                            self.step_bad(a, piece, &kind, &format!("pane names one of {}", SLATE_PANE_KINDS.join(", ")), fix.as_deref());
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }
}
