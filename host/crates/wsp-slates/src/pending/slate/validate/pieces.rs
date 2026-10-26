// SPDX-License-Identifier: AGPL-3.0-only
//! The validator's pieces: the tree, each piece's props and the items a list holds.
use super::*;

impl Validator {
    pub(super) fn tree(&mut self) {
        let ctx = Rc::clone(&self.ctx);
        let d = &ctx.doc;
        let pieces = d.get("pieces").and_then(Value::as_object);
        let root = d.get("root").and_then(Value::as_str).unwrap_or("");
        let piece_ids: Vec<String> = pieces.into_iter().flatten().map(|(k, _)| k.clone()).collect();
        if pieces.and_then(|p| p.get(root)).is_none() {
            let shown = if root.is_empty() { "(none)" } else { root };
            self.add("D203", &format!("the root {shown} is not a piece"), &Where::default(), slate_nearest(root, &piece_ids).as_deref());
            return;
        }
        let mut placed: HashSet<String> = HashSet::new();
        self.walk(root, 1, &[], None, &mut placed);
    }

    fn walk(&mut self, id: &str, depth: usize, path: &[String], row: Option<SlateType>, placed: &mut HashSet<String>) {
        let ctx = Rc::clone(&self.ctx);
        let pieces = ctx.doc.get("pieces").and_then(Value::as_object);
        if path.iter().any(|p| p == id) {
            let mut chain: Vec<&str> = path.iter().map(String::as_str).collect();
            chain.push(id);
            self.add("D205", &format!("{id} is under itself: {}", chain.join(" > ")), &Where::piece(id), None);
            return;
        }
        if placed.contains(id) {
            self.add("D205", &format!("{id} sits in two places"), &Where::piece(id), None);
            return;
        }
        placed.insert(id.to_string());
        if depth > SLATE_LIMITS.depth {
            self.add("D206", &format!("the tree is deeper than {}", SLATE_LIMITS.depth), &Where::piece(id), None);
            return;
        }
        let Some(p) = pieces.and_then(|m| m.get(id)) else {
            return;
        };
        let child_row = self.piece(id, p, row.clone());
        let piece_ids: Vec<String> = pieces.into_iter().flatten().map(|(k, _)| k.clone()).collect();
        let mut sub_path = path.to_vec();
        sub_path.push(id.to_string());
        if let Some(fallback) = p.get("fallback").and_then(Value::as_str) {
            if fallback != "drop" {
                if pieces.and_then(|m| m.get(fallback)).is_none() {
                    self.add(
                        "D203",
                        &format!("fallback names {fallback}, which is not a piece"),
                        &Where::at(id, "fallback"),
                        slate_nearest(fallback, &piece_ids).as_deref(),
                    );
                } else {
                    self.walk(fallback, depth + 1, &sub_path, row.clone(), placed);
                }
            }
        }
        let children: Vec<String> = p.get("children").and_then(Value::as_array).into_iter().flatten().map(js_str).collect();
        if children.len() > SLATE_LIMITS.children {
            self.add(
                "D207",
                &format!("{id} has {} children; the most is {}", children.len(), SLATE_LIMITS.children),
                &Where::piece(id),
                Some("bind a list instead"),
            );
        }
        for c in &children {
            if pieces.and_then(|m| m.get(c)).is_none() {
                self.add(
                    "D203",
                    &format!("{id} holds {c}, which is not a piece"),
                    &Where::piece(id),
                    slate_nearest(c, &piece_ids).as_deref(),
                );
                continue;
            }
            self.walk(c, depth + 1, &sub_path, child_row.clone(), placed);
        }
    }

    pub(super) fn piece(&mut self, id: &str, p: &Value, row: Option<SlateType>) -> Option<SlateType> {
        let ctx = Rc::clone(&self.ctx);
        let ty = p.get("type").and_then(Value::as_str);
        let Some(spec) = piece_spec(ty) else {
            if p.get("fallback").is_none() {
                let tag = ty.unwrap_or("undefined");
                let names: Vec<String> = SLATE_PIECES.keys().map(|k| k.to_string()).collect();
                let unknown = slate_unknown_piece(tag, &names);
                self.add("T300", &unknown.message, &Where::piece(id), unknown.fix.as_deref());
            }
            return row;
        };
        if let Some(when) = p.get("when").and_then(Value::as_str) {
            self.expr(when, &Where::at(id, "when"), ExprOpts { row: row.clone(), ..ExprOpts::default() });
        }
        let empty = Map::new();
        let props = p.get("props").and_then(Value::as_object).unwrap_or(&empty);
        let row_type = if spec.repeating { Some(self.items_row(props.get("items"), row.as_ref())) } else { None };
        let ty_name = ty.unwrap_or("undefined");
        for (name, v) in props {
            let item = spec.items.iter().rfind(|(_, s)| s.prop == name.as_str());
            let scalars = v.as_array().is_some_and(|items| items.iter().all(|x| x.is_string() || x.is_number()));
            if let Some((tag, item_spec)) = item {
                if !(spec.props.get(name.as_str()).is_some() && (scalars || is_bound(v))) {
                    let item_row = row_type.clone().or_else(|| row.clone());
                    self.items(id, name, v, tag, item_spec, item_row, row.clone());
                    continue;
                }
            }
            let Some(ps) = spec.props.get(name.as_str()) else {
                if let Some(reserved) = slate_reserved_prop(name) {
                    self.add(
                        "T312",
                        &format!("{name} is not a prop a slate can set; wsp draws the style. Use {reserved}."),
                        &Where::at(id, name),
                        Some(reserved),
                    );
                } else if RUN_ATTRS.contains(&name.as_str()) {
                    let example = if name == "every" { "every={60}".to_string() } else { format!("{name}=...") };
                    self.add(
                        "T302",
                        &format!("{name} is a <run>'s, not a piece's: <run name=\"x\" cmd='...' {example} />, and the piece reads $x"),
                        &Where::at(id, name),
                        None,
                    );
                } else {
                    let keys: Vec<String> = spec.props.keys().map(|k| k.to_string()).collect();
                    let fix = slate_nearest(name, &keys);
                    let hint = match &fix {
                        Some(f) => format!("; did you mean {f}?"),
                        None => format!("; it takes {}", keys.join(", ")),
                    };
                    self.add("T302", &format!("{ty_name} has no {name}{hint}"), &Where::at(id, name), fix.as_deref());
                }
                continue;
            };
            let ps: &SlatePropSpec = ps;
            let prop_row = if ps.binds == SlateBinds::Item { row_type.clone() } else { row.clone() };
            self.prop(ty_name, ps, v, &Where::at(id, name), prop_row);
        }
        for (name, ps) in &spec.props {
            if !ps.required || props.get(*name).is_some() {
                continue;
            }
            if spec.interactive && *name == "label" {
                self.add(
                    "T307",
                    &format!("a {ty_name} needs a label for people using a screen reader"),
                    &Where::at(id, name),
                    Some("label=\"...\""),
                );
            } else {
                self.add("T304", &format!("{ty_name} \"{id}\" needs {name}"), &Where::at(id, name), None);
            }
        }
        for (tag, is) in &spec.items {
            let n = props.get(is.prop).and_then(Value::as_array).map_or(0, Vec::len);
            if let Some(min) = is.min {
                if n < min && !(is.prop == "options" && props.get("options").is_some()) {
                    self.add("T304", &format!("{ty_name} needs at least {min} <{tag}>"), &Where::at(id, is.prop), None);
                }
            }
            if let Some(max) = is.max {
                if n > max {
                    self.add("T309", &format!("{ty_name} takes at most {max} <{tag}>"), &Where::at(id, is.prop), None);
                }
            }
        }
        for (event, steps) in p.get("on").and_then(Value::as_object).into_iter().flatten() {
            let prop = format!("on{}", cap(event));
            if !spec.events.contains(&event.as_str()) {
                let has = if spec.events.is_empty() {
                    String::new()
                } else {
                    format!("; it has {}", spec.events.iter().map(|e| format!("on{}", cap(e))).collect::<Vec<_>>().join(", "))
                };
                self.add("A602", &format!("{ty_name} has no {prop}{has}"), &Where::at(id, &prop), None);
                continue;
            }
            self.steps(Some(steps), &Where::at(id, &prop), event, row.clone());
        }
        if let Some(needs) = spec.needs_handler {
            let handler_len = length_of(p.get("on").and_then(|o| o.get(needs)));
            if handler_len == Some(0) {
                self.add("A602", &format!("a {ty_name} needs on{}", cap(needs)), &Where::piece(id), Some("onPress={send(\"...\")}"));
            }
        }
        let children: Vec<String> = p.get("children").and_then(Value::as_array).into_iter().flatten().map(js_str).collect();
        if !spec.holds_children && !children.is_empty() {
            self.add("T308", &format!("{ty_name} holds no children"), &Where::piece(id), None);
        }
        if let Some(limit) = &spec.child_limit {
            if children.len() > limit.max {
                let word = if limit.max == 1 { "child" } else { "children" };
                self.add("T309", &format!("{ty_name} takes at most {} {word}", limit.max), &Where::piece(id), None);
            }
            if !limit.types.is_empty() {
                for c in &children {
                    let t = ctx.doc.get("pieces").and_then(|m| m.get(c)).and_then(|x| x.get("type")).and_then(Value::as_str);
                    if let Some(t) = t {
                        if !limit.types.contains(&t) {
                            self.add(
                                "T309",
                                &format!("{ty_name} takes {} children, not {t}", or_list(limit.types)),
                                &Where::piece(id),
                                None,
                            );
                        }
                    }
                }
            }
        }
        if is_true(props.get("mono")) {
            if let Some(text_prop) = spec.text_prop {
                if sentence(props.get(text_prop)) {
                    self.add("W014", MONO_WARNING, &Where::at(id, "mono"), Some("drop mono"));
                }
            }
        }
        if let Some(check) = spec.check {
            let props_map: IndexMap<String, Value> = props.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            let kinds = Rc::clone(&ctx);
            let kind_of = move |n: &str| kind_word_of(kinds.kinds.get(n).copied()).map(str::to_string);
            let mut checked = SlatePieceCheck { props: &props_map, kind: &kind_of, problems: Vec::new() };
            check(&mut checked);
            let problems = checked.problems;
            for p in problems {
                let w = Where { piece: Some(id.to_string()), prop: p.prop.clone() };
                self.add(&p.code, &p.message, &w, p.fix.as_deref());
            }
        }
        row
    }

    fn items_row(&self, items: Option<&Value>, row: Option<&SlateType>) -> SlateType {
        let Some(items) = items.filter(|v| is_slate_binding(v)) else {
            return any_t();
        };
        let checked = check_slate_expression(bind_text(items), &self.ctx.scope(row.cloned(), false, None), 0);
        match checked.ty {
            SlateType { t: SlateTypeName::List, of, .. } => of.map_or_else(any_t, |b| *b),
            _ => any_t(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn items(
        &mut self,
        id: &str,
        prop: &str,
        v: &Value,
        tag: &str,
        spec: &SlateItemSpec,
        row: Option<SlateType>,
        outer: Option<SlateType>,
    ) {
        let Some(list) = v.as_array() else {
            self.add("T303", &format!("{prop} is a list of <{tag}> items"), &Where::at(id, prop), None);
            return;
        };
        for (i, it) in list.iter().enumerate() {
            let wh = format!("{prop}[{i}]");
            if !is_record_value(it) {
                self.add("T303", &format!("each <{tag}> is a record"), &Where::at(id, &wh), None);
                continue;
            }
            let Some(fields) = it.as_object() else {
                continue;
            };
            let item_row = if spec.row { row.clone() } else { None };
            for (k, x) in fields {
                if k == "on" {
                    if let Some(events) = spec.events {
                        for (ev, steps) in nn(Some(x)).map_or_else(Vec::new, js_entries) {
                            if !events.contains(&ev.as_str()) {
                                self.add("A602", &format!("<{tag}> has no on{ev}"), &Where::at(id, &format!("{wh}.on")), None);
                                continue;
                            }
                            self.steps(Some(steps), &Where::at(id, &format!("{wh}.onPress")), "press", item_row.clone());
                        }
                        continue;
                    }
                }
                if k == "when" {
                    let when_row = if spec.row_when { item_row.clone() } else { outer.clone() };
                    match x.as_str() {
                        Some(text) => {
                            self.expr(text, &Where::at(id, &format!("{wh}.when")), ExprOpts { row: when_row, ..ExprOpts::default() });
                        }
                        None => self.add("T303", "when is a formula: when={...}", &Where::at(id, &format!("{wh}.when")), None),
                    }
                    continue;
                }
                let Some(fs) = spec.fields.get(k.as_str()) else {
                    if let Some(reserved) = slate_reserved_prop(k) {
                        self.add(
                            "T312",
                            &format!("{k} is not a prop a slate can set; use {reserved}"),
                            &Where::at(id, &format!("{wh}.{k}")),
                            Some(reserved),
                        );
                    } else {
                        let keys: Vec<String> = spec.fields.keys().map(|f| f.to_string()).collect();
                        let fix = slate_nearest(k, &keys);
                        self.add(
                            "T302",
                            &format!("<{tag}> has no {k}; it takes {}", keys.join(", ")),
                            &Where::at(id, &format!("{wh}.{k}")),
                            fix.as_deref(),
                        );
                    }
                    continue;
                };
                let field_row = if fs.binds == SlateBinds::Item || spec.row { item_row.clone() } else { row.clone() };
                self.prop(tag, fs, x, &Where::at(id, &format!("{wh}.{k}")), field_row);
            }
            for (k, fs) in &spec.fields {
                if fs.required && it.get(*k).is_none() {
                    self.add("T304", &format!("<{tag}> needs {k}"), &Where::at(id, &wh), None);
                }
            }
            if is_true(it.get("mono")) && sentence(it.get("value")) {
                self.add("W014", MONO_WARNING, &Where::at(id, &format!("{wh}.mono")), Some("drop mono"));
            }
        }
    }

    pub(super) fn prop(&mut self, ty: &str, ps: &SlatePropSpec, v: &Value, w: &Where, row: Option<SlateType>) {
        let name = w.prop.as_deref().unwrap_or("").rsplit('.').next().unwrap_or("").to_string();
        let enum_words: Option<Vec<String>> = match &ps.prop_type {
            SlatePropType::Enum(words) => Some(words.iter().map(|s| s.to_string()).collect()),
            _ => None,
        };
        if v.is_null() {
            return;
        }
        let mark = if ps.marks || ps.binds == SlateBinds::State { None } else { joining_mark(v) };
        if let Some(mark) = mark {
            self.add(
                "W012",
                &format!("{name} joins words with \"{mark}\"; the kit separates things by layout, never a mark: give each its own piece, or join with a comma"),
                w,
                Some("separate pieces (facts, chips, a row) or a comma"),
            );
        }
        let opts = ExprOpts { row: row.clone(), secret_ok: false, enum_words: enum_words.clone() };
        if is_bound(v) {
            if ps.binds == SlateBinds::No {
                self.add("T305", &format!("{name} takes a literal, not a formula"), w, None);
                return;
            }
            if ps.binds == SlateBinds::State {
                let bind = if is_slate_binding(v) { js_trim(bind_text(v)).to_string() } else { String::new() };
                let Some(m) = slate_own_name(&bind) else {
                    let into = if name == "value" { "picked" } else { name.as_str() };
                    self.add(
                        "X410",
                        &format!("{name} on {ty} writes back, so it binds one plain value, not {bind}; bind a value of its own and keep a list with append in a <when> if you need one per item"),
                        w,
                        Some(format!("<value name=\"{into}\" start={{null}} /> and {name}={{${into}}}").as_str()),
                    );
                    return;
                };
                let kind = self.ctx.kinds.get(&m).copied();
                let Some(kind) = kind else {
                    let names: Vec<String> = self.ctx.kinds.keys().cloned().collect();
                    self.add(
                        "S501",
                        &format!("${m} is not declared; add <value name=\"{m}\" start=... />"),
                        w,
                        slate_nearest(&m, &names).as_deref(),
                    );
                    return;
                };
                if kind == Kind::Secret && ty != "input" {
                    self.add("S520", "a secret is typed into an input and nowhere else", w, None);
                    return;
                }
                if kind != Kind::Value && kind != Kind::Secret {
                    let what = if kind == Kind::Derived { "derived" } else { "a run" };
                    self.add("X410", &format!("{name} on {ty} writes a value; ${m} is {what}"), w, None);
                }
                return;
            }
            let t = self.prop_value(v, w, &opts);
            if let Some(t) = t {
                let wants_number = matches!(ps.prop_type, SlatePropType::Number | SlatePropType::Integer);
                if wants_number
                    && matches!(t.t, SlateTypeName::String | SlateTypeName::List | SlateTypeName::Record | SlateTypeName::Boolean)
                {
                    let (text, fix) = match t.t {
                        SlateTypeName::String => ("text".to_string(), Some("num(...)")),
                        SlateTypeName::List => ("a list".to_string(), Some("len(...)")),
                        SlateTypeName::Record => ("a record".to_string(), None),
                        _ => ("a boolean".to_string(), None),
                    };
                    self.add("X408", &format!("{name} takes a number; this formula gives {text}"), w, fix);
                }
                if matches!(ps.prop_type, SlatePropType::List)
                    && matches!(t.t, SlateTypeName::String | SlateTypeName::Number | SlateTypeName::Boolean)
                {
                    let text = match t.t {
                        SlateTypeName::String => "text".to_string(),
                        SlateTypeName::Number => "a number".to_string(),
                        _ => "a boolean".to_string(),
                    };
                    self.add("X408", &format!("{name} takes a list; this formula gives {text}"), w, None);
                }
            }
            return;
        }
        if ps.binds == SlateBinds::State && ps.literal && ps.prop_type == SlatePropType::Boolean && v.is_boolean() {
            return;
        }
        if ps.binds == SlateBinds::State {
            let fix_name = match v.as_str() {
                Some(s) if LOCAL_NAME_RE.is_match(s) => s.to_string(),
                _ => "name".to_string(),
            };
            self.add(
                "X410",
                &format!("{name} on {ty} is two-way and binds a value: {name}={{$name}}"),
                w,
                Some(format!("{name}={{${fix_name}}}").as_str()),
            );
            return;
        }
        if ps.prop_type == SlatePropType::Path {
            let Some(m) = v.as_str().and_then(slate_own_name) else {
                self.add("T303", &format!("{name} names a run or a value as $name"), w, None);
                return;
            };
            let kind = self.ctx.kinds.get(&m).copied();
            if ps.names_run && kind != Some(Kind::Run) {
                let word = kind_word_of(kind).unwrap_or("not declared");
                let runs: Vec<String> = self.run_names().into_iter().map(|n| format!("${n}")).collect();
                self.add("K702", &format!("run names a run: ${m} is {word}"), w, slate_nearest(&m, &runs).as_deref());
            }
            return;
        }
        if let Some(words) = &enum_words {
            let ok = v.as_str().is_some_and(|s| words.iter().any(|x| x == s));
            if !ok {
                let shown = js_str(v);
                let quoted = serde_json::to_string(v).unwrap_or_default();
                let fix = slate_nearest(&shown, words);
                self.add("T306", &format!("{name} is {}; {quoted} is not one", or_list(words)), w, fix.as_deref());
            }
            return;
        }
        let row_path = w.prop.as_deref().unwrap_or("");
        if ps.binds == SlateBinds::Item && name != "key" && !row_path.contains(['.', '[']) && (v.is_string() || v.is_number()) {
            let field = match v.as_str() {
                Some(s) if LOCAL_ITEM_RE.is_match(s) => s.to_string(),
                _ => "field".to_string(),
            };
            self.add(
                "T303",
                &format!(
                    "{name} is read per row, a formula over item: {name}={{item.{field}}}; {} would be the same on every row",
                    json_stringify(v)
                ),
                w,
                Some(format!("{name}={{item.{field}}}").as_str()),
            );
            return;
        }
        match ps.prop_type {
            SlatePropType::Number | SlatePropType::Integer => {
                if let Some(text) = v.as_str() {
                    let trimmed = js_trim(text);
                    let fix = if NUMBER_TEXT_RE.is_match(trimmed) {
                        Some(format!("{name}={{{trimmed}}}"))
                    } else if self.ctx.kinds.contains_key(text) {
                        Some(format!("{name}={{${text}}}"))
                    } else {
                        None
                    };
                    self.add(
                        "T303",
                        &format!(
                            "{name} takes a number in braces, like {name}={{5}}, or a formula, like {name}={{$count}}; \"{text}\" is text"
                        ),
                        w,
                        fix.as_deref(),
                    );
                    return;
                }
                let Some(n) = v.as_f64() else {
                    self.add("T303", &format!("{name} takes a number"), w, None);
                    return;
                };
                if ps.prop_type == SlatePropType::Integer && n.fract() != 0.0 {
                    self.add("T303", &format!("{name} is a whole number"), w, None);
                }
                let below = ps.min.is_some_and(|min| n < min);
                let above = ps.max.is_some_and(|max| n > max);
                if below || above {
                    let lo = ps.min.map_or("undefined".to_string(), number_string);
                    let hi = ps.max.map_or("undefined".to_string(), number_string);
                    self.add("T303", &format!("{name} is {lo} to {hi}"), w, None);
                }
            }
            SlatePropType::Boolean => {
                if !v.is_boolean() {
                    self.add("T303", &format!("{name} takes true or false: bare {name}, or {name}={{false}}"), w, None);
                }
            }
            SlatePropType::List => {
                if !v.is_array() {
                    self.add("T303", &format!("{name} takes a list: {name}={{path}} or {name}={{[...]}}"), w, None);
                }
            }
            SlatePropType::String | SlatePropType::Text => {
                if let Some(s) = v.as_str() {
                    if looks_like_path(s) {
                        self.add(
                            "W001",
                            &format!("{name} is the literal text \"{s}\", which reads like a path"),
                            w,
                            Some(format!("{name}={{{s}}}").as_str()),
                        );
                    } else if parse_slate_own_path(s).is_some() || MUSTACHE_RE.is_match(s) {
                        let inner = unbrace(s);
                        self.add(
                            "W001",
                            &format!("{name} is the literal text \"{s}\", which reads like a formula; quotes show it as written"),
                            w,
                            Some(format!("{name}={{{inner}}}").as_str()),
                        );
                    }
                    if s.contains('\u{2014}') {
                        self.add("W004", "an em dash in the slate's words; use a comma, a colon or a full stop", w, None);
                    }
                    if EMOJI_RE.is_match(s) {
                        self.add("W017", &format!("{name} has an emoji; the slate's words carry none, so say it in words"), w, None);
                    }
                    if (name == "title" || ps.head) && title_cased(s) {
                        self.add(
                            "W018",
                            &format!("{name} \"{s}\" is in Title Case; heads take sentence case"),
                            w,
                            Some(format!("{name}=\"{}\"", sentence_case(s)).as_str()),
                        );
                    }
                    return;
                }
                if v.is_number() && ps.prop_type == SlatePropType::Text {
                    return;
                }
                self.add("T303", &format!("{name} takes text"), w, None);
            }
            _ => {}
        }
    }

    pub(super) fn loud(&mut self) {
        let ctx = Rc::clone(&self.ctx);
        let pieces: Vec<(String, Value)> =
            ctx.doc.get("pieces").and_then(Value::as_object).into_iter().flatten().map(|(k, v)| (k.clone(), v.clone())).collect();
        let tally =
            |pred: &dyn Fn(&Value) -> bool| -> Vec<String> { pieces.iter().filter(|(_, p)| pred(p)).map(|(id, _)| id.clone()).collect() };
        let check = |this: &mut Validator, ids: Vec<String>, what: &str| {
            if ids.len() > 1 {
                this.add(
                    "T311",
                    &format!("{what} on {}; one per slate reads loud, the rest draw as default", ids.join(", ")),
                    &Where::piece(&ids[1]),
                    None,
                );
            }
        };
        let prop_is = |p: &Value, name: &str, value: &str| p.get("props").and_then(|m| m.get(name)).and_then(Value::as_str) == Some(value);
        check(self, tally(&|p| prop_is(p, "tone", "accent")), "accent");
        check(self, tally(&|p| prop_is(p, "variant", "primary")), "primary");
        check(self, tally(&|p| prop_is(p, "size", "large")), "large");
    }
}
