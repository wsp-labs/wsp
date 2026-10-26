// SPDX-License-Identifier: AGPL-3.0-only
//! The printer: the document back to the JSX-like form, declarations first.
use super::*;

// ---- printing ----

fn quote_attr(s: &str) -> String {
    let quotable = !s.ends_with('\\') && !HTML_ENTITY.is_match(s);
    if quotable && !s.contains('"') {
        return format!("\"{s}\"");
    }
    if quotable && !s.contains('\'') {
        return format!("'{s}'");
    }
    format!("{{{}}}", expr_string(s))
}

fn expr_string(s: &str) -> String {
    let body = s.replace('\\', "\\\\").replace('\'', "\\'").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t");
    format!("'{body}'")
}

fn command_attr(name: &str, cmd: &str) -> String {
    let raw = (cmd.contains('\n') || (cmd.contains('"') && cmd.contains('\''))) && !cmd.contains('`') && dedent(cmd) == cmd;
    if raw {
        format!("{name}={{`{cmd}`}}")
    } else {
        format!("{name}={}", quote_attr(cmd))
    }
}

fn literal_text(v: &Value) -> String {
    match v {
        Value::String(s) => expr_string(s),
        Value::Array(xs) => format!("[{}]", xs.iter().map(literal_text).collect::<Vec<_>>().join(", ")),
        Value::Object(m) => format!("{{ {} }}", object_fields(m, literal_text)),
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => number_string(n.as_f64().unwrap_or(f64::NAN)),
    }
}

fn object_fields(m: &Map<String, Value>, each: fn(&Value) -> String) -> String {
    m.iter().map(|(k, x)| format!("{}: {}", json_key(k), each(x))).collect::<Vec<_>>().join(", ")
}

fn value_expr(v: &Value) -> String {
    if is_slate_binding(v) {
        return v.get("bind").and_then(Value::as_str).unwrap_or("").to_string();
    }
    if let Some(format) = format_text(v) {
        let mut out = String::from("`");
        for part in parse_slate_format(format).parts {
            match part {
                SlateFormatPart::Text(t) => out.push_str(&t.replace("${", "$${").replace('`', "\\`")),
                SlateFormatPart::Hole { expr, .. } => out.push_str(&format!("${{{expr}}}")),
            }
        }
        out.push('`');
        return out;
    }
    match v {
        Value::Array(xs) => format!("[{}]", xs.iter().map(value_expr).collect::<Vec<_>>().join(", ")),
        Value::Object(m) => format!("{{ {} }}", object_fields(m, value_expr)),
        _ => literal_text(v),
    }
}

fn attr_text(name: &str, v: &Value) -> String {
    match v {
        Value::Bool(true) => name.to_string(),
        Value::String(s) => format!("{name}={}", quote_attr(s)),
        Value::Array(xs) => format!("{name}={{[{}]}}", xs.iter().map(value_expr).collect::<Vec<_>>().join(", ")),
        _ => format!("{name}={{{}}}", value_expr(v)),
    }
}

fn step_text(s: &Value) -> String {
    let kind = s.get("do").and_then(Value::as_str).unwrap_or("");
    let text = |k: &str| s.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    match kind {
        "set" => format!("set({}, {})", text("path"), value_expr(s.get("value").unwrap_or(&Value::Null))),
        "toggle" => format!("toggle({})", text("path")),
        "start" | "cancel" => format!("{kind}(${})", text("run")),
        "send" | "steer" | "queue" | "fill" => {
            let mut args = vec![json_stringify(&Value::String(text("text")))];
            if let Some(with) = s.get("with").and_then(Value::as_array) {
                args.extend(with.iter().filter_map(Value::as_str).map(str::to_string));
            }
            format!("{kind}({})", args.join(", "))
        }
        "open" => format!("open({})", value_expr(s.get("target").unwrap_or(&Value::Null))),
        "copy" => format!("copy({})", value_expr(s.get("text").unwrap_or(&Value::Null))),
        "pane" => format!("pane({})", json_stringify(&Value::String(text("kind")))),
        _ => String::new(),
    }
}

fn steps_attr(name: &str, steps: &[Value]) -> String {
    let body =
        if steps.len() == 1 { step_text(&steps[0]) } else { format!("[{}]", steps.iter().map(step_text).collect::<Vec<_>>().join(", ")) };
    format!("{name}={{{body}}}")
}

fn as_text_child(v: &Value) -> Option<String> {
    if let Value::String(s) = v {
        if s.is_empty() || s.contains(['<', '>', '{', '}', '\n']) || slate_text_child_form(s) != *s || s.contains("${") {
            return None;
        }
        return Some(s.clone());
    }
    let f = format_text(v)?;
    if f.contains('\n') || slate_text_child_form(f) != f || f.contains("$${") {
        return None;
    }
    let src: Vec<char> = f.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < src.len() {
        if src[i] == '$' && src.get(i + 1) == Some(&'{') {
            let mut depth = 1;
            let mut j = i + 2;
            let mut quote: Option<char> = None;
            while j < src.len() {
                let c = src[j];
                if let Some(q) = quote {
                    if c == '\\' {
                        j += 1;
                    } else if c == q {
                        quote = None;
                    }
                } else if c == '\'' || c == '"' || c == '`' {
                    quote = Some(c);
                } else if c == '{' {
                    depth += 1;
                } else if c == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                j += 1;
            }
            let end = j.min(src.len());
            out.push('{');
            out.extend(src[i + 2..end].iter());
            out.push('}');
            i = j + 1;
            continue;
        }
        if matches!(src[i], '<' | '>' | '{' | '}') {
            return None;
        }
        out.push(src[i]);
        i += 1;
    }
    Some(out)
}

struct Printer<'a> {
    doc: &'a SlateDoc,
    out: Vec<String>,
    seen: HashSet<String>,
}

impl Printer<'_> {
    pub(super) fn line(&mut self, depth: usize, parts: &[String], tail: &str) {
        let indent = "  ".repeat(depth);
        let mut cur = format!("{indent}{}", parts[0]);
        for p in &parts[1..] {
            if utf16::len(&cur) + 1 + utf16::len(p) + utf16::len(tail) > SLATE_LIMITS.print_columns && js_trim(&cur) != parts[0] {
                self.out.push(cur);
                cur = format!("{indent}    {p}");
            } else {
                cur.push(' ');
                cur.push_str(p);
            }
        }
        self.out.push(format!("{cur}{tail}"));
    }

    pub(super) fn declarations(&mut self) {
        let doc = self.doc;
        for (name, v) in doc.values.iter().filter(|(_, v)| v.secret != Some(true)) {
            let mut parts = vec!["<value".to_string(), format!("name=\"{name}\"")];
            if !v.start.is_null() {
                parts.push(match &v.start {
                    Value::String(s) => format!("start={}", quote_attr(s)),
                    other => format!("start={{{}}}", literal_text(other)),
                });
            }
            self.line(1, &parts, " />");
        }
        for (name, v) in doc.values.iter().filter(|(_, v)| v.secret == Some(true)) {
            let mut parts = vec!["<secret".to_string(), format!("name=\"{name}\"")];
            if v.keep == Some(true) {
                parts.push("keep".to_string());
            }
            self.line(1, &parts, " />");
        }
        for (name, expr) in &doc.derived {
            let parts = vec!["<derived".to_string(), format!("name=\"{name}\""), format!("value={{{expr}}}")];
            self.line(1, &parts, " />");
        }
        for (name, r) in &doc.runs {
            let mut parts = vec!["<run".to_string(), format!("name=\"{name}\"")];
            match r {
                SlateRunDecl::Cmd(c) => {
                    parts.push(command_attr("cmd", &c.cmd));
                    if let Some(env) = &c.env {
                        parts.push(format!("env={{{}}}", value_expr(&record_value(env))));
                    }
                    if let Some(args) = &c.args {
                        parts.push(format!("args={{[{}]}}", args.iter().map(value_expr).collect::<Vec<_>>().join(", ")));
                    }
                    if let Some(stdin) = &c.stdin {
                        parts.push(attr_text("stdin", stdin));
                    }
                    if let Some(on) = &c.on {
                        parts.push(format!("on=\"{on}\""));
                    }
                    if let Some(cwd) = &c.cwd {
                        parts.push(format!("cwd={}", quote_attr(cwd)));
                    }
                    if let Some(timeout) = &c.timeout {
                        parts.push(format!("timeout={{{}}}", timeout.js_string()));
                    }
                    if c.stream == Some(true) {
                        parts.push("stream".to_string());
                    }
                    push_tail(&mut parts, c.confirm.as_ref(), c.every.as_ref(), c.once, c.always, c.then.as_deref());
                }
                SlateRunDecl::Tool(t) => {
                    parts.push(format!("tool=\"{}.{}\"", t.server, t.tool));
                    if let Some(args) = &t.args {
                        parts.push(format!("args={{{}}}", value_expr(&record_value(args))));
                    }
                    push_tail(&mut parts, t.confirm.as_ref(), t.every.as_ref(), t.once, t.always, t.then.as_deref());
                    if let Some(d) = &t.description {
                        parts.push(format!("description={}", quote_attr(d)));
                    }
                }
                SlateRunDecl::Resource(res) => {
                    parts.push(format!("resource={}", quote_attr(&format!("{}:{}", res.server, res.uri))));
                    push_tail(&mut parts, None, res.every.as_ref(), None, res.always, res.then.as_deref());
                    if let Some(d) = &res.description {
                        parts.push(format!("description={}", quote_attr(d)));
                    }
                }
            }
            self.line(1, &parts, " />");
        }
        for r in &doc.reactions {
            let mut parts = vec!["<when".to_string()];
            if let Some(id) = r.get("id").and_then(Value::as_str) {
                parts.push(format!("id=\"{id}\""));
            }
            let on = r.get("on");
            if let Some(change) = on.and_then(|o| o.get("change")).and_then(Value::as_array) {
                let paths: Vec<&str> = change.iter().filter_map(Value::as_str).collect();
                let body = if paths.len() == 1 { paths[0].to_string() } else { format!("[{}]", paths.join(", ")) };
                parts.push(format!("change={{{body}}}"));
            } else {
                let done = on.and_then(|o| o.get("done")).and_then(Value::as_str).unwrap_or("");
                parts.push(format!("done={{${done}}}"));
            }
            let steps = r.get("do").and_then(Value::as_array).cloned().unwrap_or_default();
            parts.push(steps_attr("do", &steps));
            self.line(1, &parts, " />");
        }
    }

    pub(super) fn piece(&mut self, id: &str, depth: usize) {
        let doc = self.doc;
        let Some(p) = doc.pieces.get(id) else { return };
        if !self.seen.insert(id.to_string()) {
            return;
        }
        let spec = slate_piece_module(&p.kind);
        let mut props: IndexMap<String, Value> = p.props.clone().unwrap_or_default();
        let is_item_list = |v: Option<&Value>| matches!(v, Some(Value::Array(xs)) if xs.iter().all(|x| x.is_object() && !is_slate_binding(x) && format_text(x).is_none()));
        let mut item_props: IndexMap<String, &'static str> = IndexMap::new();
        if let Some(s) = spec {
            for (tag, item) in &s.items {
                if is_item_list(p.props.as_ref().and_then(|m| m.get(item.prop))) {
                    item_props.insert(item.prop.to_string(), tag);
                }
            }
        }
        let mut text: Option<String> = None;
        let mut raw: Option<String> = None;
        if let Some(tp) = spec.and_then(|s| s.text_prop) {
            if let Some(v) = props.get(tp).cloned() {
                let raw_text = spec.is_some_and(|s| s.raw_text);
                if raw_text {
                    if let Value::String(s) = &v {
                        if !s.contains(&format!("</{}>", p.kind)) {
                            raw = Some(s.clone());
                            props.shift_remove(tp);
                        }
                    }
                } else if let Some(t) = as_text_child(&v) {
                    text = Some(t);
                    props.shift_remove(tp);
                }
            }
        }
        let mut names: IndexSet<String> = IndexSet::new();
        if let Some(s) = spec {
            names.extend(s.props.keys().map(|k| k.to_string()));
        }
        names.extend(props.keys().cloned());
        let mut parts = vec![format!("<{}", p.kind), format!("id=\"{id}\"")];
        for name in &names {
            if !props.contains_key(name) || item_props.contains_key(name) || name == "when" {
                continue;
            }
            let v = &props[name];
            let is_path = spec.and_then(|s| s.props.get(name.as_str())).is_some_and(|x| x.prop_type == SlatePropType::Path);
            match v {
                Value::String(s) if is_path && s.starts_with('$') => parts.push(format!("{name}={{{s}}}")),
                _ => parts.push(attr_text(name, v)),
            }
        }
        if let Some(on) = p.on.as_ref().and_then(Value::as_object) {
            for (ev, steps) in on {
                if let Some(steps) = steps.as_array() {
                    parts.push(steps_attr(&format!("on{}", upper_first(ev)), steps));
                }
            }
        }
        if let Some(fallback) = &p.fallback {
            match fallback {
                Value::String(s) => parts.push(format!("fallback={}", quote_attr(s))),
                other => {
                    let body = other.get("text").and_then(Value::as_str).unwrap_or("");
                    parts.push(format!("fallback={{{}}}", expr_string(body)));
                }
            }
        }
        if let Some(when) = &p.when {
            parts.push(format!("when={{{when}}}"));
        }
        let mut items: Vec<Vec<String>> = Vec::new();
        for (prop, tag) in &item_props {
            let Some(Value::Array(list)) = props.get(prop) else { continue };
            for it in list {
                let empty = Map::new();
                let r = it.as_object().unwrap_or(&empty);
                let mut ip = vec![format!("<{tag}")];
                let fields: Vec<String> =
                    spec.and_then(|s| s.items.get(*tag)).map_or(Vec::new(), |i| i.fields.keys().map(|k| k.to_string()).collect());
                let mut keys: IndexSet<String> = fields.into_iter().collect();
                keys.extend(r.keys().cloned());
                for k in &keys {
                    if !r.contains_key(k) || k == "on" || k == "when" {
                        continue;
                    }
                    ip.push(attr_text(k, &r[k]));
                }
                if let Some(on) = r.get("on").and_then(Value::as_object) {
                    for (ev, steps) in on {
                        if let Some(steps) = steps.as_array() {
                            ip.push(steps_attr(&format!("on{}", upper_first(ev)), steps));
                        }
                    }
                }
                if let Some(when) = r.get("when").and_then(Value::as_str) {
                    ip.push(format!("when={{{when}}}"));
                }
                items.push(ip);
            }
        }
        let children = p.children.clone().unwrap_or_default();
        if text.is_none() && raw.is_none() && items.is_empty() && children.is_empty() {
            self.line(depth, &parts, " />");
            return;
        }
        if let Some(r) = raw {
            self.line(depth, &parts, &format!(">{r}</{}>", p.kind));
            return;
        }
        if let Some(t) = &text {
            if items.is_empty() && children.is_empty() {
                self.line(depth, &parts, &format!(">{t}</{}>", p.kind));
                return;
            }
        }
        self.line(depth, &parts, ">");
        if let Some(t) = &text {
            self.out.push(format!("{}{t}", "  ".repeat(depth + 1)));
        }
        for it in &items {
            self.line(depth + 1, it, " />");
        }
        for c in &children {
            self.piece(c, depth + 1);
        }
        self.out.push(format!("{}</{}>", "  ".repeat(depth), p.kind));
    }

    pub(super) fn files(&mut self) {
        let doc = self.doc;
        let Some(files) = &doc.files else { return };
        for (name, text) in files {
            let edge_space = text.chars().next().is_some_and(is_js_whitespace) || text.chars().last().is_some_and(is_js_whitespace);
            let braced = !text.contains('`') && !edge_space;
            let body = if braced {
                let inner: Vec<String> = text.split('\n').map(|l| if l.is_empty() { String::new() } else { format!("    {l}") }).collect();
                format!("{{`\n{}\n  `}}", inner.join("\n"))
            } else {
                text.clone()
            };
            self.out.push(format!("  <file name=\"{name}\">{body}</file>"));
        }
    }
}

fn push_tail(
    parts: &mut Vec<String>,
    confirm: Option<&Value>,
    every: Option<&JsNumber>,
    once: Option<bool>,
    always: Option<bool>,
    then: Option<&str>,
) {
    if let Some(c) = confirm {
        parts.push(match c {
            Value::String(s) => format!("confirm={}", quote_attr(s)),
            other => attr_text("confirm", other),
        });
    }
    if let Some(e) = every {
        parts.push(format!("every={{{}}}", e.js_string()));
    }
    if once == Some(true) {
        parts.push("once".to_string());
    }
    if always == Some(true) {
        parts.push("always".to_string());
    }
    if let Some(t) = then {
        parts.push(command_attr("then", t));
    }
}

/// The document in the JSX-like form: declarations first, then the root piece and its subtree.
pub fn print_slate(doc: &SlateDoc) -> String {
    let head = match &doc.title {
        Some(t) => format!("<slate title={}>", quote_attr(t)),
        None => "<slate>".to_string(),
    };
    let mut p = Printer { doc, out: vec![head], seen: HashSet::new() };
    p.declarations();
    p.piece(&doc.root, 1);
    p.files();
    p.out.push("</slate>".to_string());
    p.out.join("\n")
}
