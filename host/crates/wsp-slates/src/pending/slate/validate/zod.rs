// SPDX-License-Identifier: AGPL-3.0-only
//! The zod-shape emulation: the schemas a stored document is parsed against, and the shape problems they report.
use super::*;

#[derive(Clone, Debug)]
pub(super) struct Iss {
    code: &'static str,
    path: Vec<String>,
    message: String,
    keys: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum St {
    Ok,
    Dirty,
    Bad,
}

pub(super) enum S {
    Str,
    StrMin1,
    Num,
    Lit(Value),
    Enum(&'static [&'static str]),
    Json,
    Arr(Box<S>),
    Rec(Box<S>),
    Obj { fields: Vec<(&'static str, S, bool)>, strict: bool },
    Union(Vec<S>),
    Disc { key: &'static str, opts: Vec<(&'static str, S)> },
}

fn child_path(path: &[String], seg: &str) -> Vec<String> {
    let mut p = path.to_vec();
    p.push(seg.to_string());
    p
}

fn issue(code: &'static str, path: &[String], message: String) -> Iss {
    Iss { code, path: path.to_vec(), message, keys: Vec::new() }
}

fn expected_type(expected: &str, v: Option<&Value>, path: &[String], out: &mut Vec<Iss>) -> St {
    let message = if v.is_none() { "Required".to_string() } else { format!("Expected {expected}, received {}", parsed_type(v)) };
    out.push(issue("invalid_type", path, message));
    St::Bad
}

pub(super) fn zod_parse(s: &S, v: Option<&Value>, path: &[String], out: &mut Vec<Iss>) -> St {
    match s {
        S::Str | S::StrMin1 => match v {
            Some(Value::String(text)) => {
                if matches!(s, S::StrMin1) && text.is_empty() {
                    out.push(issue("too_small", path, "String must contain at least 1 character(s)".to_string()));
                    St::Dirty
                } else {
                    St::Ok
                }
            }
            _ => expected_type("string", v, path, out),
        },
        S::Num => match v {
            Some(Value::Number(_)) => St::Ok,
            _ => expected_type("number", v, path, out),
        },
        S::Lit(expected) => {
            if v == Some(expected) {
                St::Ok
            } else {
                out.push(issue("invalid_literal", path, format!("Invalid literal value, expected {}", json_stringify(expected))));
                St::Bad
            }
        }
        S::Enum(words) => match v.and_then(Value::as_str) {
            Some(text) => {
                if words.contains(&text) {
                    St::Ok
                } else {
                    let options: Vec<String> = words.iter().map(|w| format!("'{w}'")).collect();
                    out.push(issue(
                        "invalid_enum_value",
                        path,
                        format!("Invalid enum value. Expected {}, received '{text}'", options.join(" | ")),
                    ));
                    St::Bad
                }
            }
            None => {
                let options: Vec<String> = words.iter().map(|w| format!("'{w}'")).collect();
                expected_type(&options.join(" | "), v, path, out)
            }
        },
        S::Json => match v {
            Some(_) => St::Ok,
            None => {
                out.push(issue("invalid_union", path, "Invalid input".to_string()));
                St::Bad
            }
        },
        S::Arr(child) => match v.and_then(Value::as_array) {
            None => expected_type("array", v, path, out),
            Some(items) => {
                let mut status = St::Ok;
                let mut bad = false;
                for (i, x) in items.iter().enumerate() {
                    match zod_parse(child, Some(x), &child_path(path, &i.to_string()), out) {
                        St::Bad => bad = true,
                        St::Dirty => status = St::Dirty,
                        St::Ok => {}
                    }
                }
                if bad {
                    St::Bad
                } else {
                    status
                }
            }
        },
        S::Rec(child) => match v.and_then(Value::as_object) {
            None => expected_type("object", v, path, out),
            Some(m) => {
                let mut status = St::Ok;
                let mut bad = false;
                for (k, x) in m {
                    match zod_parse(child, Some(x), &child_path(path, k), out) {
                        St::Bad => bad = true,
                        St::Dirty => status = St::Dirty,
                        St::Ok => {}
                    }
                }
                if bad {
                    St::Bad
                } else {
                    status
                }
            }
        },
        S::Obj { fields, strict } => {
            let Some(m) = v.and_then(Value::as_object) else {
                return expected_type("object", v, path, out);
            };
            let mut status = St::Ok;
            let mut bad = false;
            for (key, child, optional) in fields {
                let cv = m.get(*key);
                if cv.is_none() && *optional {
                    continue;
                }
                match zod_parse(child, cv, &child_path(path, key), out) {
                    St::Bad => bad = true,
                    St::Dirty => status = St::Dirty,
                    St::Ok => {}
                }
            }
            let extra: Vec<String> = m.keys().filter(|k| !fields.iter().any(|(f, _, _)| *f == k.as_str())).cloned().collect();
            if *strict && !extra.is_empty() {
                let shown: Vec<String> = extra.iter().map(|k| format!("'{k}'")).collect();
                out.push(Iss {
                    code: "unrecognized_keys",
                    path: path.to_vec(),
                    message: format!("Unrecognized key(s) in object: {}", shown.join(", ")),
                    keys: extra,
                });
                status = St::Dirty;
            }
            if bad {
                St::Bad
            } else {
                status
            }
        }
        S::Union(options) => {
            let mut dirty: Option<Vec<Iss>> = None;
            for option in options {
                let mut local = Vec::new();
                match zod_parse(option, v, path, &mut local) {
                    St::Ok => return St::Ok,
                    St::Dirty => {
                        if dirty.is_none() {
                            dirty = Some(local);
                        }
                    }
                    St::Bad => {}
                }
            }
            match dirty {
                Some(issues) => {
                    out.extend(issues);
                    St::Dirty
                }
                None => {
                    out.push(issue("invalid_union", path, "Invalid input".to_string()));
                    St::Bad
                }
            }
        }
        S::Disc { key, opts } => {
            let Some(m) = v.and_then(Value::as_object) else {
                return expected_type("object", v, path, out);
            };
            let tag = m.get(*key).and_then(Value::as_str);
            match opts.iter().find(|(t, _)| Some(*t) == tag) {
                None => {
                    let tags: Vec<String> = opts.iter().map(|(t, _)| format!("'{t}'")).collect();
                    out.push(issue(
                        "invalid_union_discriminator",
                        &child_path(path, key),
                        format!("Invalid discriminator value. Expected {}", tags.join(" | ")),
                    ));
                    St::Bad
                }
                Some((_, option)) => zod_parse(option, v, path, out),
            }
        }
    }
}

fn opt(child: S) -> (S, bool) {
    (child, true)
}

fn req(child: S) -> (S, bool) {
    (child, false)
}

fn obj(fields: Vec<(&'static str, (S, bool))>, strict: bool) -> S {
    S::Obj { fields: fields.into_iter().map(|(k, (s, o))| (k, s, o)).collect(), strict }
}

fn binding() -> S {
    obj(vec![("bind", req(S::StrMin1))], true)
}

fn format_obj() -> S {
    obj(vec![("format", req(S::Str))], true)
}

fn confirm_schema() -> S {
    S::Union(vec![S::Str, binding(), format_obj()])
}

fn steps_schema() -> S {
    S::Arr(Box::new(step_schema()))
}

fn step_schema() -> S {
    S::Disc {
        key: "do",
        opts: vec![
            ("set", obj(vec![("do", req(S::Lit(json!("set")))), ("path", req(S::Str)), ("value", req(S::Json))], true)),
            ("toggle", obj(vec![("do", req(S::Lit(json!("toggle")))), ("path", req(S::Str))], true)),
            ("start", obj(vec![("do", req(S::Lit(json!("start")))), ("run", req(S::Str))], true)),
            ("cancel", obj(vec![("do", req(S::Lit(json!("cancel")))), ("run", req(S::Str))], true)),
            ("send", obj(vec![("do", req(S::Lit(json!("send")))), ("text", req(S::Str)), ("with", opt(S::Arr(Box::new(S::Str))))], true)),
            ("steer", obj(vec![("do", req(S::Lit(json!("steer")))), ("text", req(S::Str)), ("with", opt(S::Arr(Box::new(S::Str))))], true)),
            ("queue", obj(vec![("do", req(S::Lit(json!("queue")))), ("text", req(S::Str)), ("with", opt(S::Arr(Box::new(S::Str))))], true)),
            ("fill", obj(vec![("do", req(S::Lit(json!("fill")))), ("text", req(S::Str)), ("with", opt(S::Arr(Box::new(S::Str))))], true)),
            ("open", obj(vec![("do", req(S::Lit(json!("open")))), ("target", req(S::Json))], true)),
            ("copy", obj(vec![("do", req(S::Lit(json!("copy")))), ("text", req(S::Json))], true)),
            ("pane", obj(vec![("do", req(S::Lit(json!("pane")))), ("kind", req(S::Enum(&PANE_KINDS)))], true)),
        ],
    }
}

fn run_schema() -> S {
    let common = || {
        vec![
            ("confirm", opt(confirm_schema())),
            ("every", opt(S::Num)),
            ("once", opt(S::Lit(json!(true)))),
            ("always", opt(S::Lit(json!(true)))),
            ("then", opt(S::Str)),
        ]
    };
    let mut cmd = vec![
        ("kind", req(S::Lit(json!("cmd")))),
        ("cmd", req(S::Str)),
        ("env", opt(S::Rec(Box::new(S::Json)))),
        ("args", opt(S::Arr(Box::new(S::Json)))),
        ("stdin", opt(S::Json)),
        ("on", opt(S::Enum(&["thread", "host"]))),
        ("cwd", opt(S::Str)),
        ("timeout", opt(S::Num)),
        ("stream", opt(S::Lit(json!(true)))),
    ];
    cmd.extend(common());
    let mut tool = vec![
        ("kind", req(S::Lit(json!("tool")))),
        ("server", req(S::Str)),
        ("tool", req(S::Str)),
        ("args", opt(S::Rec(Box::new(S::Json)))),
    ];
    tool.push(("description", opt(S::Str)));
    tool.extend(common());
    let resource = vec![
        ("kind", req(S::Lit(json!("resource")))),
        ("server", req(S::Str)),
        ("uri", req(S::Str)),
        ("every", opt(S::Num)),
        ("always", opt(S::Lit(json!(true)))),
        ("then", opt(S::Str)),
        ("description", opt(S::Str)),
    ];
    S::Disc { key: "kind", opts: vec![("cmd", obj(cmd, true)), ("tool", obj(tool, true)), ("resource", obj(resource, true))] }
}

fn piece_schema() -> S {
    let on = obj(vec![("press", opt(steps_schema())), ("submit", opt(steps_schema())), ("change", opt(steps_schema()))], true);
    let fallback = S::Union(vec![S::Str, obj(vec![("text", req(S::Str))], true)]);
    obj(
        vec![
            ("type", req(S::Str)),
            ("props", opt(S::Rec(Box::new(S::Json)))),
            ("children", opt(S::Arr(Box::new(S::Str)))),
            ("when", opt(S::Str)),
            ("on", opt(on)),
            ("fallback", opt(fallback)),
        ],
        true,
    )
}

fn reaction_schema() -> S {
    let on = S::Union(vec![obj(vec![("change", req(S::Arr(Box::new(S::Str))))], true), obj(vec![("done", req(S::Str))], true)]);
    obj(vec![("id", opt(S::Str)), ("on", req(on)), ("do", req(steps_schema()))], true)
}

pub(super) fn doc_schema() -> S {
    let value = obj(vec![("start", req(S::Json)), ("secret", opt(S::Lit(json!(true)))), ("keep", opt(S::Lit(json!(true))))], true);
    obj(
        vec![
            ("schema", req(S::Lit(json!(2)))),
            ("kit", opt(S::Str)),
            ("title", opt(S::Str)),
            ("root", req(S::Str)),
            ("values", req(S::Rec(Box::new(value)))),
            ("derived", req(S::Rec(Box::new(S::Str)))),
            ("runs", req(S::Rec(Box::new(run_schema())))),
            ("reactions", req(S::Arr(Box::new(reaction_schema())))),
            ("pieces", req(S::Rec(Box::new(piece_schema())))),
            ("files", opt(S::Rec(Box::new(S::Str)))),
        ],
        true,
    )
}

pub(super) fn shape_problems(issue: &Iss, doc: &Value) -> Vec<SlateProblem> {
    let path = &issue.path;
    let area = path.first().map(String::as_str);
    let name = path.get(1);
    let field = path.get(2);
    let sub = path.get(3);
    let keys = if issue.code == "unrecognized_keys" { issue.keys.clone() } else { Vec::new() };
    if area == Some("pieces") {
        if let Some(name) = name {
            let piece = doc.get("pieces").and_then(|p| p.get(name));
            let ty = piece.and_then(|p| p.get("type")).and_then(Value::as_str);
            let spec = ty.and_then(|t| SLATE_PIECES.get(t));
            if field.is_none() && !keys.is_empty() {
                return keys
                    .iter()
                    .map(|k| {
                        let known = spec.is_some_and(|s| s.props.get(k.as_str()).is_some())
                            || spec.is_some_and(|s| s.items.values().any(|i| i.prop == k.as_str()));
                        let tail_text = if known {
                            format!(", and {k} goes inside props")
                        } else if spec.is_some() {
                            format!(", and {} has no {k}", ty.unwrap_or("undefined"))
                        } else {
                            String::new()
                        };
                        let fix = if known {
                            Some(format!("\"props\": {{ \"{k}\": ... }}"))
                        } else {
                            let keys: Vec<String> = spec.map_or_else(Vec::new, |s| s.props.keys().map(|x| x.to_string()).collect());
                            slate_nearest(k, &keys)
                        };
                        let mut p = slate_problem(
                            "T302",
                            &format!("{k} sits beside props on {name}; a piece holds {PIECE_KEYS}{tail_text}"),
                            fix.as_deref(),
                        );
                        p.piece = Some(name.clone());
                        p.prop = Some(k.clone());
                        p
                    })
                    .collect();
            }
            if field.map(String::as_str) == Some("type") {
                let mut p = slate_problem("T300", &format!("{name}'s type is the piece's name as a string"), None);
                p.piece = Some(name.clone());
                p.prop = Some("type".to_string());
                return vec![p];
            }
            if field.map(String::as_str) == Some("on") {
                let (code, message) = if !keys.is_empty() {
                    let shown: Vec<String> = keys.clone();
                    ("A602", format!("{name} has no on.{}; events are press, submit and change", shown.join(", on.")))
                } else {
                    let rest = path[2..].join(".");
                    ("A600", format!("{name}.{rest}: a handler is a list of steps, {}", issue.message))
                };
                let prop = match sub {
                    Some(s) => format!("on.{s}"),
                    None => "on".to_string(),
                };
                let mut p = slate_problem(code, &message, None);
                p.piece = Some(name.clone());
                p.prop = Some(prop);
                return vec![p];
            }
            let prop = if field.map(String::as_str) == Some("props") { sub.cloned() } else { field.cloned() };
            let rest = if path.len() > 2 { path[2..].join(".") } else { String::new() };
            let mut p = slate_problem("T303", &format!("{name}.{rest}: {}", issue.message), None);
            p.piece = Some(name.clone());
            p.prop = prop;
            return vec![p];
        }
    }
    if (area == Some("values") || area == Some("derived") || area == Some("runs")) && name.is_some() {
        let name = name.map_or("", String::as_str);
        let piece = format!("${name}");
        if !keys.is_empty() {
            let takes = match area {
                Some("values") => "start, secret and keep",
                Some("runs") => "kind, cmd, env, args, stdin, on, cwd, timeout, stream, confirm, every, once, always, then and description",
                _ => "an expression",
            };
            let code = if area == Some("runs") { "K704" } else { "T302" };
            return keys
                .iter()
                .map(|k| {
                    let mut p = slate_problem(code, &format!("{piece} has no {k}; it takes {takes}"), None);
                    p.piece = Some(piece.clone());
                    p.prop = Some(k.clone());
                    p
                })
                .collect();
        }
        if area == Some("runs") && issue.code == "invalid_union_discriminator" {
            let mut p = slate_problem("K701", &format!("{piece} takes kind \"cmd\", \"tool\" or \"resource\""), None);
            p.piece = Some(piece);
            p.prop = Some("kind".to_string());
            return vec![p];
        }
        let rest = if path.len() > 2 { path[2..].join(".") } else { String::new() };
        let rest = if rest.is_empty() { "decl".to_string() } else { rest };
        let mut p = slate_problem(if area == Some("runs") { "K704" } else { "T303" }, &format!("{piece}.{rest}: {}", issue.message), None);
        p.piece = Some(piece);
        p.prop = field.cloned();
        return vec![p];
    }
    if area == Some("reactions") {
        let index: f64 = name.and_then(|n| n.parse().ok()).unwrap_or(f64::NAN);
        let number = number_string(index + 1.0);
        let at = if path.len() > 2 { format!(".{}", path[2..].join(".")) } else { String::new() };
        let mut p = slate_problem(
            "A611",
            &format!(
                "reaction {number}{at}: {}; a <when> is {{ on: {{ change: [paths] }} or {{ done: name }}, do: [steps] }}",
                issue.message
            ),
            None,
        );
        p.piece = Some(format!("reaction-{number}"));
        return vec![p];
    }
    if !keys.is_empty() {
        return keys
            .iter()
            .map(|k| {
                let fix = slate_nearest(k, &DOC_KEYS.iter().map(|s| s.to_string()).collect::<Vec<_>>());
                let extra =
                    if area.is_none() && slate_reserved_prop(k).is_some() { "; a piece's props go under pieces.<id>.props" } else { "" };
                slate_problem("D202", &format!("the document has no {k}; it holds {}{extra}", DOC_KEYS.join(", ")), fix.as_deref())
            })
            .collect();
    }
    let at = if path.is_empty() { "the document".to_string() } else { path.join(".") };
    vec![slate_problem("D202", &format!("{at}: {}", issue.message), None)]
}
