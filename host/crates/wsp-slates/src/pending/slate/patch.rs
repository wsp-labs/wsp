// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/patch.ts at 65f6b89c7.

use std::collections::HashSet;

use serde_json::{json, Map, Value};

use super::order::KeyOrder;
use super::paths::from_deep_value;
use super::validate::{validate_json, SlateLines};
use super::*;

pub fn slate_start_values(doc: &SlateDoc) -> SlateValues {
    let mut out = SlateValues::new();
    for (name, v) in &doc.values {
        let start = if v.secret == Some(true) { json!({"secret": true, "set": false, "len": 0, "at": null}) } else { v.start.clone() };
        out.insert(name.clone(), start);
    }
    for name in doc.runs.keys() {
        out.insert(name.clone(), slate_run_idle());
    }
    out
}

pub fn carry_slate_values(before: Option<&SlateDoc>, after: &SlateDoc, values: &SlateValues) -> SlateValues {
    let kind = |d: Option<&SlateDoc>, n: &str| -> Option<&'static str> {
        let d = d?;
        if let Some(v) = d.values.get(n) {
            return Some(if v.secret == Some(true) { "secret" } else { "value" });
        }
        d.runs.contains_key(n).then_some("run")
    };
    let mut out = SlateValues::new();
    for (name, start) in slate_start_values(after) {
        let k = kind(Some(after), &name);
        let keep = k.is_some() && k == kind(before, &name) && values.contains_key(&name);
        let live = if keep { values[&name].clone() } else { start };
        out.insert(name, live);
    }
    out
}

fn op_name(op: &Value) -> Option<&str> {
    op.get("op").and_then(Value::as_str)
}

fn op_str(op: &Value, key: &str) -> String {
    op.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn lines_map(lines: &Value) -> Option<SlateLines> {
    let m = lines.as_object()?;
    Some(m.iter().filter_map(|(k, v)| v.as_i64().map(|n| (k.clone(), n))).collect())
}

fn fail(errors: &mut Vec<SlateProblem>, lines: Option<&SlateLines>, code: &str, message: &str, piece: Option<&str>) {
    let mut p = slate_problem(code, message, None);
    p.piece = piece.map(str::to_string);
    p.line = piece.and_then(|x| lines.and_then(|l| l.get(x).copied()));
    errors.push(p);
}

fn object_at<'a>(m: &'a mut Map<String, Value>, key: &str) -> Option<&'a mut Map<String, Value>> {
    let slot = m.entry(key.to_string()).or_insert_with(|| json!({}));
    if !slot.is_object() {
        *slot = json!({});
    }
    slot.as_object_mut()
}

fn children_of(piece: Option<&Value>) -> Vec<String> {
    piece.and_then(Value::as_object).map_or_else(Vec::new, child_list)
}

fn child_list(p: &Map<String, Value>) -> Vec<String> {
    p.get("children").and_then(Value::as_array).map_or_else(Vec::new, |c| c.iter().filter_map(Value::as_str).map(str::to_string).collect())
}

fn subtree(d: &Map<String, Value>, id: &str) -> Vec<String> {
    fn walk(d: &Map<String, Value>, x: &str, out: &mut Vec<String>) {
        if out.iter().any(|o| o == x) {
            return;
        }
        out.push(x.to_string());
        let pieces = d.get("pieces").and_then(Value::as_object);
        for c in children_of(pieces.and_then(|p| p.get(x))) {
            walk(d, &c, out);
        }
    }
    let mut out = Vec::new();
    walk(d, id, &mut out);
    out
}

fn parent_of(d: &Map<String, Value>, id: &str) -> Option<String> {
    let pieces = d.get("pieces").and_then(Value::as_object)?;
    pieces.iter().find(|(_, p)| children_of(Some(p)).iter().any(|c| c == id)).map(|(k, _)| k.clone())
}

fn detach(d: &mut Map<String, Value>, id: &str) {
    let Some(parent) = parent_of(d, id) else {
        return;
    };
    let Some(pieces) = object_at(d, "pieces") else {
        return;
    };
    let Some(p) = pieces.get_mut(&parent).and_then(Value::as_object_mut) else {
        return;
    };
    let list: Vec<Value> = child_list(p).into_iter().filter(|c| c != id).map(Value::String).collect();
    if list.is_empty() {
        p.shift_remove("children");
    } else {
        p.insert("children".to_string(), Value::Array(list));
    }
}

fn insert_children(d: &mut Map<String, Value>, under: &str, ids: &[String], at: Option<f64>) {
    let Some(pieces) = object_at(d, "pieces") else {
        return;
    };
    let Some(p) = pieces.get_mut(under).and_then(Value::as_object_mut) else {
        return;
    };
    let mut list: Vec<Value> = child_list(p).into_iter().map(Value::String).collect();
    let len = list.len();
    let pos = match at {
        None => len,
        Some(a) => (a.trunc().max(0.0) as usize).min(len),
    };
    list.splice(pos..pos, ids.iter().cloned().map(Value::String));
    p.insert("children".to_string(), Value::Array(list));
}

fn apply_op(d: &mut Map<String, Value>, op: &Value, lines: Option<&SlateLines>, errors: &mut Vec<SlateProblem>) {
    match op_name(op) {
        Some("replace") => {
            let id = op_str(op, "id");
            if !has_piece(d, &id) {
                fail(errors, lines, "D203", &format!("there is no piece {id} to replace"), Some(&id));
                return;
            }
            let children = op.get("children").and_then(Value::as_object);
            let mut keep: HashSet<String> = HashSet::from([id.clone()]);
            keep.extend(children.into_iter().flat_map(|m| m.keys().cloned()));
            for old in subtree(d, &id) {
                if !keep.contains(&old) {
                    if let Some(pieces) = object_at(d, "pieces") {
                        pieces.shift_remove(&old);
                    }
                }
            }
            let Some(pieces) = object_at(d, "pieces") else {
                return;
            };
            pieces.insert(id.clone(), op.get("piece").cloned().unwrap_or(Value::Null));
            for (k, p) in children.into_iter().flatten() {
                pieces.insert(k.clone(), p.clone());
            }
        }
        Some("props") => {
            let id = op_str(op, "id");
            if !has_piece(d, &id) {
                fail(errors, lines, "D203", &format!("there is no piece {id}"), Some(&id));
                return;
            }
            let Some(p) = object_at(d, "pieces").and_then(|pieces| pieces.get_mut(&id)).and_then(Value::as_object_mut) else {
                return;
            };
            if let Some(props) = op.get("props").and_then(Value::as_object) {
                for (k, v) in props {
                    if v.is_null() {
                        if let Some(current) = p.get_mut("props").and_then(Value::as_object_mut) {
                            current.shift_remove(k);
                        }
                    } else if let Some(current) = object_at(p, "props") {
                        current.insert(k.clone(), v.clone());
                    }
                }
            }
            if p.get("props").and_then(Value::as_object).is_some_and(Map::is_empty) {
                p.shift_remove("props");
            }
            match op.get("when") {
                Some(Value::Null) => {
                    p.shift_remove("when");
                }
                Some(v) => {
                    p.insert("when".to_string(), v.clone());
                }
                None => {}
            }
            match op.get("fallback") {
                Some(Value::Null) => {
                    p.shift_remove("fallback");
                }
                Some(v) => {
                    p.insert("fallback".to_string(), v.clone());
                }
                None => {}
            }
            if let Some(on) = op.get("on").and_then(Value::as_object) {
                if let Some(current) = object_at(p, "on") {
                    for (ev, steps) in on {
                        current.insert(ev.clone(), steps.clone());
                    }
                }
            }
        }
        Some("add") => {
            let under = op_str(op, "under");
            if !has_piece(d, &under) {
                fail(errors, lines, "D203", &format!("there is no piece {under} to add under"), Some(&under));
                return;
            }
            for (k, p) in op.get("pieces").and_then(Value::as_object).into_iter().flatten() {
                if has_piece(d, k) {
                    fail(errors, lines, "P104", &format!("{k} is already a piece; replace it by writing it with its id"), Some(k));
                    continue;
                }
                if let Some(pieces) = object_at(d, "pieces") {
                    pieces.insert(k.clone(), p.clone());
                }
            }
            let order: Vec<String> =
                op.get("order").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
            insert_children(d, &under, &order, op.get("at").and_then(Value::as_f64));
        }
        Some("remove") => {
            let id = op_str(op, "id");
            if !has_piece(d, &id) {
                fail(errors, lines, "D203", &format!("there is no piece {id} to remove"), Some(&id));
                return;
            }
            if d.get("root").and_then(Value::as_str) == Some(id.as_str()) {
                fail(errors, lines, "D202", "the root piece cannot be removed; write <clear /> or replace it", Some(&id));
                return;
            }
            detach(d, &id);
            for x in subtree(d, &id) {
                if let Some(pieces) = object_at(d, "pieces") {
                    pieces.shift_remove(&x);
                }
            }
        }
        Some("move") => {
            let id = op_str(op, "id");
            let under = op_str(op, "under");
            if !has_piece(d, &id) || !has_piece(d, &under) {
                let named = if has_piece(d, &id) { under.as_str() } else { id.as_str() };
                fail(errors, lines, "D203", &format!("move names {named}, which is not a piece"), Some(&id));
                return;
            }
            if subtree(d, &id).contains(&under) {
                fail(errors, lines, "D205", &format!("{id} cannot move under itself"), Some(&id));
                return;
            }
            detach(d, &id);
            insert_children(d, &under, std::slice::from_ref(&id), op.get("at").and_then(Value::as_f64));
        }
        Some("value") => write_decl(d, "values", op),
        Some("derived") => {
            let name = op_str(op, "name");
            match op.get("expr") {
                Some(Value::Null) => {
                    if let Some(m) = object_at(d, "derived") {
                        m.shift_remove(&name);
                    }
                }
                Some(expr) => {
                    if let Some(m) = object_at(d, "derived") {
                        m.insert(name, expr.clone());
                    }
                }
                None => {}
            }
        }
        Some("run") => write_decl(d, "runs", op),
        Some("reaction") => {
            let id = op_str(op, "id");
            let reactions = d.entry("reactions".to_string()).or_insert_with(|| json!([]));
            if !reactions.is_array() {
                *reactions = json!([]);
            }
            let Some(list) = reactions.as_array_mut() else {
                return;
            };
            let at = list.iter().position(|r| r.get("id").and_then(Value::as_str) == Some(id.as_str()));
            match op.get("reaction") {
                Some(Value::Null) => match at {
                    None => fail(errors, lines, "D203", &format!("there is no reaction {id}"), Some(&id)),
                    Some(i) => {
                        list.remove(i);
                    }
                },
                Some(reaction) => match at {
                    Some(i) => list[i] = reaction.clone(),
                    None => list.push(reaction.clone()),
                },
                None => {}
            }
        }
        Some("file") => {
            let name = op_str(op, "name");
            match op.get("text") {
                Some(Value::Null) => {
                    let present = d.get("files").and_then(Value::as_object).is_some_and(|f| f.contains_key(&name));
                    if !present {
                        fail(errors, lines, "D203", &format!("there is no file {name}"), None);
                        return;
                    }
                    let emptied = match d.get_mut("files").and_then(Value::as_object_mut) {
                        Some(files) => {
                            files.shift_remove(&name);
                            files.is_empty()
                        }
                        None => false,
                    };
                    if emptied {
                        d.shift_remove("files");
                    }
                }
                Some(text) => {
                    if let Some(files) = object_at(d, "files") {
                        files.insert(name, text.clone());
                    }
                }
                None => {}
            }
        }
        Some("clear") => fail(errors, lines, "P105", "<clear /> stands alone in its write", None),
        _ => {}
    }
}

fn has_piece(d: &Map<String, Value>, id: &str) -> bool {
    d.get("pieces").and_then(Value::as_object).is_some_and(|p| p.contains_key(id))
}

fn write_decl(d: &mut Map<String, Value>, section: &str, op: &Value) {
    let name = op_str(op, "name");
    match op.get("decl") {
        Some(Value::Null) => {
            if let Some(m) = object_at(d, section) {
                m.shift_remove(&name);
            }
        }
        Some(decl) => {
            if let Some(m) = object_at(d, section) {
                m.insert(name, decl.clone());
            }
        }
        None => {}
    }
}

pub fn apply_slate_patch(doc: Option<&SlateDoc>, values: &SlateValues, ops: &[Value], lines: &Value) -> SlateApplied {
    let refused = |errors: Vec<SlateProblem>| SlateApplied { document: None, values: None, errors, warnings: Vec::new() };
    if ops.len() == 1 && op_name(&ops[0]) == Some("clear") {
        return SlateApplied { document: Some(None), values: Some(values.clone()), errors: Vec::new(), warnings: Vec::new() };
    }
    if ops.iter().any(|o| op_name(o) == Some("undo")) {
        return refused(vec![slate_problem(
            "V752",
            "undo goes back to the document before the agent's last write, which the host holds; send <undo /> alone to the host",
            None,
        )]);
    }
    let Some(doc) = doc else {
        return refused(vec![slate_problem(
            "Z802",
            "there is no slate to patch; write one with <slate>",
            Some("<slate title=\"...\">...</slate>"),
        )]);
    };
    let lines = lines_map(lines);
    let mut d: Map<String, Value> = match serde_json::to_value(doc) {
        Ok(Value::Object(m)) => m,
        _ => {
            return refused(vec![slate_problem(
                "D202",
                "a document is an object with schema, root, values, derived, runs, reactions and pieces",
                None,
            )])
        }
    };
    let mut errors = Vec::new();
    for op in ops {
        apply_op(&mut d, op, lines.as_ref(), &mut errors);
    }
    if !errors.is_empty() {
        return refused(errors);
    }
    let checked = validate_json(&Value::Object(d.clone()), lines.as_ref());
    if !checked.errors.is_empty() {
        return SlateApplied { document: None, values: None, errors: checked.errors, warnings: checked.warnings };
    }
    let patched = Value::Object(d);
    let order = KeyOrder::observed(&patched);
    let next: SlateDoc = match from_deep_value::<SlateDoc>(patched) {
        Ok(next) => SlateDoc { order, ..next },
        Err(_) => {
            return refused(vec![slate_problem(
                "D202",
                "a document is an object with schema, root, values, derived, runs, reactions and pieces",
                None,
            )]);
        }
    };
    let carried = carry_slate_values(Some(doc), &next, values);
    SlateApplied { document: Some(Some(next)), values: Some(carried), errors: Vec::new(), warnings: checked.warnings }
}
