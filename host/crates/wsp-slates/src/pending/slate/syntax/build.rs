// SPDX-License-Identifier: AGPL-3.0-only
//! Helpers the compiler builds records with: explicit ids, nested declarations, names and piece values.
use super::*;

pub(super) fn explicit_ids(el: &El) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(id) = attr_of(el, "id") {
        if id.kind == Kind::Str {
            out.push(id.value.clone());
        }
    }
    for ch in &el.children {
        out.extend(explicit_ids(ch));
    }
    out
}

fn nested_declarations(el: &mut El) -> Vec<El> {
    let mut found = Vec::new();
    let mut keep = Vec::new();
    for mut ch in std::mem::take(&mut el.children) {
        if is_decl_el(&ch) {
            found.push(ch);
        } else {
            found.extend(nested_declarations(&mut ch));
            keep.push(ch);
        }
    }
    el.children = keep;
    found
}

pub(super) fn lift_declarations(children: Vec<El>) -> Vec<El> {
    let mut rest = Vec::new();
    let mut found = Vec::new();
    for mut ch in children {
        if is_decl_el(&ch) {
            found.push(ch);
        } else {
            found.extend(nested_declarations(&mut ch));
            rest.push(ch);
        }
    }
    rest.extend(found);
    rest
}

pub(super) fn owner_of(spec: &SlatePieceModule, name: &str) -> Option<String> {
    spec.props
        .iter()
        .find(|(_, p)| matches!(p.prop_type, SlatePropType::Enum(list) if list.contains(&name)))
        .map(|(key, _)| key.to_string())
}

pub(super) fn strs(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

pub(super) fn step_names() -> Vec<String> {
    SLATE_STEPS.iter().map(|s| s.kind.to_string()).collect()
}

pub(super) fn piece_names() -> Vec<String> {
    SLATE_PIECES.keys().map(|k| k.to_string()).collect()
}

pub(super) fn format_text(v: &Value) -> Option<&str> {
    let m = v.as_object()?;
    if m.len() != 1 {
        return None;
    }
    m.get("format")?.as_str()
}

pub(super) fn is_piece_base(ty: &str) -> bool {
    let mut chars = ty.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase()) && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub(super) fn json_text(v: Option<&Value>) -> String {
    match v {
        None => "\u{0}undefined".to_string(),
        Some(v) => json_stringify(v),
    }
}

pub(super) fn sub_map<'a>(op: &'a mut Map<String, Value>, key: &str) -> &'a mut Map<String, Value> {
    op.entry(key.to_string()).or_insert_with(|| Value::Object(Map::new())).as_object_mut().expect("a sub object")
}

pub(super) fn piece_value(p: &SlatePiece) -> Value {
    let mut m = Map::new();
    m.insert("type".into(), Value::String(p.kind.clone()));
    if let Some(when) = &p.when {
        m.insert("when".into(), Value::String(when.clone()));
    }
    if let Some(props) = &p.props {
        m.insert("props".into(), record_value(props));
    }
    if let Some(children) = &p.children {
        m.insert("children".into(), json!(children));
    }
    if let Some(on) = &p.on {
        m.insert("on".into(), on.clone());
    }
    if let Some(fallback) = &p.fallback {
        m.insert("fallback".into(), fallback.clone());
    }
    Value::Object(m)
}

pub(super) fn json_key(k: &str) -> String {
    if IDENT.is_match(k) {
        k.to_string()
    } else {
        json_stringify(&Value::String(k.to_string()))
    }
}

pub(super) fn lit(el: &El, name: &str) -> Option<String> {
    let a = attr_of(el, name)?;
    match a.kind {
        Kind::Str => Some(a.value.clone()),
        Kind::Braced => string_literal(&a.value),
        Kind::Bare => None,
    }
}

pub(super) fn under_of(el: &El, current: Option<&SlateDoc>) -> String {
    let u = lit(el, "under").unwrap_or_default();
    if u == "root" && current.is_none_or(|c| !c.pieces.contains_key("root")) {
        current.map_or(u.clone(), |c| c.root.clone())
    } else {
        u
    }
}

pub(super) fn slate_own_name_braced(a: &Attr) -> Option<String> {
    if a.kind == Kind::Braced {
        slate_own_name(&a.value)
    } else {
        None
    }
}
