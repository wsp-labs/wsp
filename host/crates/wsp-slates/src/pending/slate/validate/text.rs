// SPDX-License-Identifier: AGPL-3.0-only
//! Words and paths the validator reads prop text and file paths with.
use super::*;

pub(super) fn title_cased(v: &str) -> bool {
    let later: Vec<&str> = split_ws(js_trim(v))
        .into_iter()
        .skip(1)
        .filter(|w| LONG_WORD_RE.is_match(w) && !KEPT_CAPS_RE.is_match(tail(w)) && !ACRONYM_RE.is_match(w))
        .collect();
    later.len() >= 2 && later.iter().all(|w| CAP_START_RE.is_match(w))
}

pub(super) fn sentence_case(v: &str) -> String {
    split_keep_ws(js_trim(v))
        .into_iter()
        .enumerate()
        .map(|(i, w)| {
            if i == 0 || ALL_WS_RE.is_match(w) || ACRONYM_RE.is_match(w) || KEPT_CAPS_RE.is_match(tail(w)) {
                w.to_string()
            } else {
                w.to_lowercase()
            }
        })
        .collect()
}

pub(super) fn token_in(text: &str) -> Option<String> {
    let m = TOKEN_RE.captures(text)?;
    let token = m.get(1).or_else(|| m.get(2))?.as_str();
    if token.starts_with('$') {
        return None;
    }
    Some(token.chars().take(6).collect())
}

pub(super) fn quoted(expr: &str) -> Vec<String> {
    QUOTED_RE
        .captures_iter(expr)
        .map(|m| {
            let text = m.get(1).or_else(|| m.get(2)).or_else(|| m.get(3)).map_or("", |g| g.as_str());
            HOLE_RE.replace_all(text, "x").into_owned()
        })
        .collect()
}

pub(super) fn joining_mark(v: &Value) -> Option<String> {
    let parts =
        if is_slate_format(v) { parse_slate_format(v.get("format").and_then(Value::as_str).unwrap_or("")).parts } else { Vec::new() };
    let texts: Vec<String> = if let Some(s) = v.as_str() {
        vec![s.to_string()]
    } else if is_slate_binding(v) {
        quoted(bind_text(v))
    } else {
        let mut out = vec![parts
            .iter()
            .map(|p| match p {
                SlateFormatPart::Text(t) => t.clone(),
                SlateFormatPart::Hole { .. } => "x".to_string(),
            })
            .collect::<String>()];
        for p in &parts {
            if let SlateFormatPart::Hole { expr, .. } = p {
                out.extend(quoted(expr));
            }
        }
        out
    };
    for t in &texts {
        if let Some(m) = JOINED_RE.captures(t) {
            return (1..m.len()).find_map(|i| m.get(i)).map(|g| g.as_str().to_string());
        }
    }
    None
}

pub(super) fn sentence(v: Option<&Value>) -> bool {
    let Some(v) = nn(v) else {
        return false;
    };
    let texts: Vec<String> = if let Some(s) = v.as_str() {
        vec![s.to_string()]
    } else if is_slate_binding(v) {
        quoted(bind_text(v))
    } else if is_slate_format(v) {
        let parts = parse_slate_format(v.get("format").and_then(Value::as_str).unwrap_or("")).parts;
        vec![parts
            .iter()
            .map(|p| match p {
                SlateFormatPart::Text(t) => t.clone(),
                SlateFormatPart::Hole { .. } => " 0 ".to_string(),
            })
            .collect()]
    } else {
        Vec::new()
    };
    texts.iter().any(|t| split_ws(t).into_iter().filter(|w| SENTENCE_WORD_RE.is_match(w)).count() >= 3)
}

pub(super) fn piece_spec(ty: Option<&str>) -> Option<&'static SlatePieceModule> {
    ty.and_then(|t| SLATE_PIECES.get(t))
}

pub(super) fn cap(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub(super) fn length_of(v: Option<&Value>) -> Option<usize> {
    match nn(v) {
        None => Some(0),
        Some(Value::Array(items)) => Some(items.len()),
        Some(Value::String(s)) => Some(s.encode_utf16().count()),
        Some(_) => None,
    }
}

pub(super) fn looks_like_path(s: &str) -> bool {
    PATH_LIKE_RE.captures(s).is_some_and(|m| SLATE_SOURCES.contains_key(m.get(1).map_or("", |g| g.as_str())))
}

pub(super) fn unbrace(s: &str) -> &str {
    if s.len() >= 2 && s.starts_with('{') && s.ends_with('}') && !s[1..s.len() - 1].contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

pub(super) fn paths_overlap(a: &str, b: &str) -> bool {
    let under = |x: &str, y: &str| x.starts_with(y) && matches!(x.as_bytes().get(y.len()), Some(b'.') | Some(b'['));
    a == b || under(a, b) || under(b, a)
}

pub(super) fn roots(paths: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in paths.iter().filter(|p| p.starts_with('$')) {
        let head = p[1..].split(['.', '[']).next().unwrap_or("").to_string();
        if !out.contains(&head) {
            out.push(head);
        }
    }
    out
}
