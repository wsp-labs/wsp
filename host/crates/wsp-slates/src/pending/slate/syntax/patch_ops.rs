// SPDX-License-Identifier: AGPL-3.0-only
//! The compiler's patch ops: the ops a patch's top-level elements compile to.
use super::*;

impl Compiler {
    pub(super) fn patch_ops(&mut self, tops: Vec<El>, current: Option<&SlateDoc>) -> Vec<Value> {
        let mut ops: Vec<Value> = Vec::new();
        for el in &tops {
            for id in explicit_ids(el) {
                if current.is_none_or(|c| !c.pieces.contains_key(&id)) {
                    self.taken.insert(id);
                }
            }
        }
        for el in tops {
            match el.tag.as_str() {
                "clear" | "undo" => {
                    ops.push(json!({"op": el.tag}));
                    continue;
                }
                "remove" => {
                    let id = lit(&el, "id");
                    let name = lit(&el, "name");
                    if id.is_none() == name.is_none() {
                        self.error(
                            "P105",
                            "<remove> takes id=\"...\" for a piece or a reaction, or name=\"...\" for a declaration",
                            el.line,
                            Ext::default(),
                        );
                        continue;
                    }
                    if let Some(name) = name {
                        if current.is_some_and(|c| c.derived.contains_key(&name)) {
                            ops.push(json!({"op": "derived", "name": name, "expr": null}));
                        } else if current.is_some_and(|c| c.runs.contains_key(&name)) {
                            ops.push(json!({"op": "run", "name": name, "decl": null}));
                        } else if current.is_some_and(|c| c.values.contains_key(&name)) {
                            ops.push(json!({"op": "value", "name": name, "decl": null}));
                        } else if current.is_some_and(|c| c.files.as_ref().is_some_and(|f| f.contains_key(&name))) {
                            ops.push(json!({"op": "file", "name": name, "text": null}));
                        } else {
                            let mut names: Vec<String> = self.names.keys().cloned().collect();
                            names.extend(
                                current.and_then(|c| c.files.as_ref()).map(|f| f.keys().cloned().collect::<Vec<_>>()).unwrap_or_default(),
                            );
                            let fix = slate_nearest(&name, &names);
                            self.error(
                                "D203",
                                &format!("nothing is declared as {name}"),
                                el.line,
                                Ext { fix: fix.as_deref(), ..Ext::default() },
                            );
                        }
                        continue;
                    }
                    let id = id.unwrap_or_default();
                    let is_reaction = current.is_some_and(|c| {
                        !c.pieces.contains_key(&id) && c.reactions.iter().any(|r| r.get("id").and_then(Value::as_str) == Some(id.as_str()))
                    });
                    if is_reaction {
                        ops.push(json!({"op": "reaction", "id": id, "reaction": null}));
                    } else {
                        ops.push(json!({"op": "remove", "id": id}));
                    }
                    continue;
                }
                "move" => {
                    let id = lit(&el, "id");
                    if id.is_none() || attr_of(&el, "under").is_none() {
                        self.error("P105", "<move> takes id and under", el.line, Ext::default());
                        continue;
                    }
                    let n = self.at_of(&el);
                    let mut op = json!({"op": "move", "id": id, "under": under_of(&el, current)});
                    if let Some(n) = n {
                        op["at"] = n;
                    }
                    ops.push(op);
                    continue;
                }
                "add" => {
                    if attr_of(&el, "under").is_none() {
                        self.error("P105", "<add> takes under=\"<id>\"", el.line, Ext::default());
                        continue;
                    }
                    let before: HashSet<String> = self.doc.pieces.keys().cloned().collect();
                    let mut order: Vec<String> = Vec::new();
                    for ch in el.children.clone() {
                        if let Some(id) = self.piece(ch) {
                            order.push(id);
                        }
                    }
                    let mut pieces = Map::new();
                    for (k, p) in &self.doc.pieces {
                        if !before.contains(k) {
                            pieces.insert(k.clone(), piece_value(p));
                        }
                    }
                    let n = self.at_of(&el);
                    let mut op = json!({"op": "add", "under": under_of(&el, current)});
                    if let Some(n) = n {
                        op["at"] = n;
                    }
                    op["pieces"] = Value::Object(pieces);
                    op["order"] = json!(order);
                    ops.push(op);
                    continue;
                }
                "props" => {
                    let Some(id) = lit(&el, "id") else {
                        self.error("P105", "<props> takes id=\"...\"", el.line, Ext::default());
                        continue;
                    };
                    let op = self.props_op(&el, &id, current);
                    ops.push(op);
                    continue;
                }
                _ => {}
            }
            if is_file_decl_el(&el) {
                if let Some(name) = self.file(&el) {
                    let text = self.doc.files.as_ref().and_then(|f| f.get(&name)).cloned().unwrap_or_default();
                    ops.push(json!({"op": "file", "name": name, "text": text}));
                }
                continue;
            }
            if is_decl_tag(&el.tag) {
                if let Err(f) = self.declare_name(&el) {
                    self.report(f);
                }
                let before = self.doc.reactions.len();
                self.declaration(&el);
                let name = attr_of(&el, "name").map_or(String::new(), |a| a.value.clone());
                match el.tag.as_str() {
                    "value" | "secret" => {
                        if let Some(decl) = self.doc.values.get(&name) {
                            ops.push(json!({"op": "value", "name": name, "decl": decl}));
                        }
                    }
                    "derived" => {
                        if let Some(expr) = self.doc.derived.get(&name) {
                            ops.push(json!({"op": "derived", "name": name, "expr": expr}));
                        }
                    }
                    "run" => {
                        if let Some(decl) = self.doc.runs.get(&name) {
                            ops.push(json!({"op": "run", "name": name, "decl": decl}));
                        }
                    }
                    _ => {
                        if self.doc.reactions.len() > before {
                            let r = self.doc.reactions.last().cloned().unwrap_or(Value::Null);
                            let rid = r.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                            ops.push(json!({"op": "reaction", "id": rid, "reaction": r}));
                        }
                    }
                }
                continue;
            }
            if slate_piece_module(&el.tag).is_some() || !SLATE_ITEM_KINDS.contains(&el.tag.as_str()) {
                let Some(id) = lit(&el, "id") else {
                    let fix = format!("<add under=\"{}\"><{} ... /></add>", current.map_or("root", |c| c.root.as_str()), el.tag);
                    self.error(
                        "P105",
                        &format!("a piece at the top of a patch replaces the piece with its id, so <{}> needs id=\"...\"", el.tag),
                        el.line,
                        Ext { fix: Some(&fix), ..Ext::default() },
                    );
                    continue;
                };
                if let Some(c) = current {
                    if !c.pieces.contains_key(&id) {
                        let names: Vec<String> = c.pieces.keys().cloned().collect();
                        let fix = slate_nearest(&id, &names);
                        self.error(
                            "D203",
                            &format!("there is no piece {id} to replace; add it with <add under=\"...\">"),
                            el.line,
                            Ext { fix: fix.as_deref(), ..Ext::default() },
                        );
                        continue;
                    }
                }
                self.taken.remove(&id);
                let before: HashSet<String> = self.doc.pieces.keys().cloned().collect();
                self.piece(el);
                let mut children = Map::new();
                for (k, p) in &self.doc.pieces {
                    if !before.contains(k) && *k != id {
                        children.insert(k.clone(), piece_value(p));
                    }
                }
                let piece_v = self.doc.pieces.get(&id).map_or(Value::Null, piece_value);
                let mut op = json!({"op": "replace", "id": id, "piece": piece_v});
                if !children.is_empty() {
                    op["children"] = Value::Object(children);
                }
                ops.push(op);
                continue;
            }
            self.error(
                "P105",
                &format!(
                    "a patch is pieces with an id, <props>, <add>, <remove>, <move>, declarations, <clear> or <undo>; not <{}>, an item: change it inside its piece with <props id=\"...\"><{} ... /></props>",
                    el.tag, el.tag
                ),
                el.line,
                Ext::default(),
            );
        }
        ops
    }

    fn props_op(&mut self, el: &El, id: &str, current: Option<&SlateDoc>) -> Value {
        let mut op = Map::new();
        op.insert("op".into(), json!("props"));
        op.insert("id".into(), json!(id));
        for a in &el.attrs {
            if a.name == "id" {
                continue;
            }
            self.lines.insert(format!("{id}.{}", a.name), a.line);
            let is_null = a.kind == Kind::Braced && js_trim(&a.value) == "null";
            if a.name == "when" {
                let v = if is_null {
                    Value::Null
                } else if a.kind == Kind::Braced {
                    Value::String(js_trim(&a.value).to_string())
                } else {
                    self.error(
                        "T303",
                        "when takes a formula in braces",
                        a.line,
                        Ext { piece: Some(id), prop: Some("when"), ..Ext::default() },
                    );
                    Value::Null
                };
                op.insert("when".into(), v);
                continue;
            }
            if a.name == "fallback" {
                let v = if is_null {
                    Value::Null
                } else if a.kind == Kind::Str {
                    Value::String(a.value.clone())
                } else {
                    json!({"text": string_literal(&a.value).unwrap_or_default()})
                };
                op.insert("fallback".into(), v);
                continue;
            }
            if is_on_attr(&a.name) {
                let event = a.name[2..].to_ascii_lowercase();
                let steps = self.steps(a, id);
                sub_map(&mut op, "on").insert(event, Value::Array(steps));
                continue;
            }
            let v = if is_null { Value::Null } else { self.value(a) };
            sub_map(&mut op, "props").insert(a.name.clone(), v);
        }
        let target = current.and_then(|c| c.pieces.get(id)).cloned();
        let target_module = target.as_ref().and_then(|t| slate_piece_module(&t.kind));
        if let Some(text) = &el.text {
            let text_prop = target_module.and_then(|m| m.text_prop);
            let has_prop = op.get("props").and_then(Value::as_object).is_some_and(|p| text_prop.is_some_and(|tp| p.contains_key(tp)));
            match (&target, text_prop) {
                (Some(t), None) => self.error(
                    "P105",
                    &format!("{} takes no text child; set its props as attributes: <props id=\"{id}\" ... />", t.kind),
                    el.line,
                    Ext { piece: Some(id), ..Ext::default() },
                ),
                (_, Some(tp)) if has_prop => self.error(
                    "T303",
                    &format!("<props> has both {tp}= and a text child; use one"),
                    el.line,
                    Ext { piece: Some(id), prop: Some(tp), ..Ext::default() },
                ),
                (_, Some(tp)) => {
                    let v = text_child(text);
                    sub_map(&mut op, "props").insert(tp.to_string(), v);
                    self.lines.insert(format!("{id}.{tp}"), el.line);
                }
                (None, None) => {}
            }
        }
        for ch in &el.children {
            let item_spec = target_module.and_then(|m| m.items.get(ch.tag.as_str()));
            let Some(spec) = item_spec else {
                if let Some(t) = &target {
                    self.error(
                        "P105",
                        &format!(
                            "<props> merges attributes and the items {} takes; to change <{}> under {id}, write {id} again with its id or use <add under=\"{id}\">",
                            t.kind, ch.tag
                        ),
                        ch.line,
                        Ext { piece: Some(id), ..Ext::default() },
                    );
                }
                continue;
            };
            let key = spec.fields.keys().next().copied().unwrap_or("");
            let existing = op.get("props").and_then(|p| p.get(spec.prop)).filter(|v| !v.is_null()).cloned();
            let mut list: Vec<Value> = match existing {
                Some(Value::Array(xs)) => xs,
                Some(_) => Vec::new(),
                None => target
                    .as_ref()
                    .and_then(|t| t.props.as_ref())
                    .and_then(|p| p.get(spec.prop))
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
            };
            let n = list.len();
            let item = self.item(ch, spec, id, &format!("{}[{n}]", spec.prop));
            let item_map = item.as_object().cloned().unwrap_or_default();
            let wanted = json_text(item_map.get(key));
            let pos = list.iter().position(|x| json_text(x.get(key)) == wanted);
            let mut merged: Map<String, Value> = match pos {
                Some(i) => list[i].as_object().cloned().unwrap_or_default(),
                None => Map::new(),
            };
            for (k, v) in item_map {
                merged.insert(k, v);
            }
            merged.retain(|_, v| !v.is_null());
            match pos {
                Some(i) => list[i] = Value::Object(merged),
                None => list.push(Value::Object(merged)),
            }
            sub_map(&mut op, "props").insert(spec.prop.to_string(), Value::Array(list));
        }
        Value::Object(op)
    }
}
