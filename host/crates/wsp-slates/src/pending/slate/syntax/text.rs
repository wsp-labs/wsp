// SPDX-License-Identifier: AGPL-3.0-only
//! Text children and the JSON shaping the compiler and printer share.
use super::*;

pub(super) fn dedent(text: &str) -> String {
    let text = text.replace("\r\n", "\n");
    let mut lines: Vec<&str> = text.split('\n').collect();
    let blank = lines.iter().take_while(|l| js_trim(l).is_empty()).count();
    lines.drain(..blank);
    while lines.last().is_some_and(|l| js_trim(l).is_empty()) {
        lines.pop();
    }
    let indent =
        lines.iter().filter(|l| !js_trim(l).is_empty()).map(|l| l.chars().take_while(|c| is_js_whitespace(*c)).count()).min().unwrap_or(0);
    lines.iter().map(|l| l.chars().skip(indent).collect::<String>()).collect::<Vec<_>>().join("\n")
}

fn normalize_run(s: &str) -> String {
    let s = RUN_SPACES.replace_all(s, " ");
    let s = RUN_NEWLINE.replace_all(&s, "\n");
    RUN_BLANKS.replace_all(&s, "\n\n").into_owned()
}

pub(super) fn slate_text_child_form(s: &str) -> String {
    js_trim(&normalize_run(s)).to_string()
}

pub(super) fn js_replace_units(s: &str, keep: impl Fn(char) -> bool, with: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if keep(c) {
            out.push(c);
        } else {
            for _ in 0..c.len_utf16() {
                out.push_str(with);
            }
        }
    }
    out
}

pub(super) fn split_top(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut depth: i64 = 0;
    let mut quote: Option<char> = None;
    let mut cur = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = quote {
            cur.push(c);
            if c == '\\' {
                i += 1;
                if let Some(next) = chars.get(i) {
                    cur.push(*next);
                }
            } else if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if c == '\'' || c == '"' || c == '`' {
            quote = Some(c);
            cur.push(c);
            i += 1;
            continue;
        }
        if "{[(".contains(c) {
            depth += 1;
        }
        if "}])".contains(c) {
            depth -= 1;
        }
        if c == ',' && depth == 0 {
            out.push(std::mem::take(&mut cur));
            i += 1;
            continue;
        }
        cur.push(c);
        i += 1;
    }
    if !js_trim(&cur).is_empty() {
        out.push(cur);
    }
    out
}

pub(super) fn js_index(map: IndexMap<String, Value>) -> IndexMap<String, Value> {
    js_key_order(map.iter()).into_iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

pub(super) fn record(map: IndexMap<String, Value>) -> Value {
    Value::Object(js_index(map).into_iter().collect())
}

pub(super) fn record_value(map: &IndexMap<String, Value>) -> Value {
    Value::Object(map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
}

pub(super) fn jnum(x: f64) -> Value {
    if x.fract() == 0.0 && x.abs() < 9.0e15 {
        Value::from(x as i64)
    } else {
        Value::from(x)
    }
}

pub(super) fn text_child(parts: &[Part]) -> Value {
    let mut format = String::new();
    let mut holes = 0;
    for p in parts {
        match p {
            Part::Text(s) => format.push_str(&normalize_run(s).replace("${", "$${")),
            Part::Hole(expr) => {
                let e = js_trim(expr);
                if e.starts_with("/*") && e.ends_with("*/") {
                    continue;
                }
                if let Some(lit) = string_literal(e) {
                    format.push_str(&lit.replace("${", "$${"));
                    continue;
                }
                if format.ends_with('$') {
                    format.pop();
                    format.push_str("${'$'}");
                }
                format.push_str(&format!("${{{e}}}"));
                holes += 1;
            }
        }
    }
    let format = js_trim(&format).to_string();
    if holes == 0 {
        Value::String(format.replace("$${", "${"))
    } else {
        json!({"format": format})
    }
}
