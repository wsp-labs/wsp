// SPDX-License-Identifier: AGPL-3.0-only
//! The compiler's pieces: the piece element, its attributes, items and the values they hold.
use super::*;

impl Compiler {
    pub(super) fn piece(&mut self, mut el: El) -> Option<String> {
        if is_decl_el(&el) {
            self.error("P100", &format!("<{}> is a declaration and goes directly under <slate>", el.tag), el.line, Ext::default());
            return None;
        }
        if SLATE_ITEM_KINDS.contains(&el.tag.as_str()) {
            self.error("T314", &format!("<{}> is an item and goes inside the piece that takes it", el.tag), el.line, Ext::default());
            return None;
        }
        let spec = slate_piece_module(&el.tag);
        if spec.is_some_and(|s| s.holds_children && s.text_prop.is_none()) && el.text.is_some() {
            let text = el.text.take();
            let mut kids = vec![El { tag: "text".into(), attrs: Vec::new(), children: Vec::new(), text, raw: None, line: el.line }];
            kids.append(&mut el.children);
            el.children = kids;
        }
        let id_attr = attr_of(&el, "id").cloned();
        let id = match &id_attr {
            Some(a) if a.kind == Kind::Str && SLATE_ID.is_match(&a.value) => {
                if self.doc.pieces.contains_key(&a.value) {
                    self.error("P104", &format!("{} is used twice", a.value), a.line, Ext { piece: Some(&a.value), ..Ext::default() });
                }
                a.value.clone()
            }
            other => {
                if let Some(a) = other {
                    let replaced = js_replace_units(&a.value, |c| c.is_ascii_alphanumeric() || c == '_' || c == '-', "-");
                    let stripped = replaced.trim_start_matches(|c: char| !c.is_ascii_alphabetic());
                    let fix = (!stripped.is_empty()).then(|| stripped.to_string());
                    self.error(
                        "P103",
                        &format!("\"{}\" is not a piece id: letters, digits, dashes and _, starting with a letter", a.value),
                        a.line,
                        Ext { fix: fix.as_deref(), ..Ext::default() },
                    );
                }
                self.mint(&el.tag)
            }
        };
        self.doc
            .pieces
            .insert(id.clone(), SlatePiece { kind: el.tag.clone(), props: None, children: None, when: None, on: None, fallback: None });
        self.lines.insert(id.clone(), el.line);
        let mut st = PieceState::default();
        for a in &el.attrs {
            if a.name == "id" {
                continue;
            }
            self.lines.insert(format!("{id}.{}", a.name), a.line);
            if let Err(f) = self.piece_attr(a, &id, &el.tag, spec, &mut st) {
                self.report(f);
            }
        }
        if let (Some(s), Some(raw)) = (spec, &el.raw) {
            if let Some(tp) = s.text_prop {
                st.props.insert(tp.to_string(), Value::String(raw.clone()));
            }
        }
        if let Some(text) = &el.text {
            match spec {
                None => {
                    let names = piece_names();
                    let unknown = slate_unknown_piece(&el.tag, &names);
                    self.error("T300", &unknown.message, el.line, Ext { piece: Some(&id), fix: unknown.fix.as_deref(), ..Ext::default() });
                }
                Some(s) => match s.text_prop {
                    None => self.error(
                        "P100",
                        &format!("{} takes elements, not text; put the words in a <text>", el.tag),
                        el.line,
                        Ext { piece: Some(&id), ..Ext::default() },
                    ),
                    Some(tp) if st.props.contains_key(tp) => self.error(
                        "T303",
                        &format!("{} has both {tp}= and a text child; use one", el.tag),
                        el.line,
                        Ext { piece: Some(&id), prop: Some(tp), ..Ext::default() },
                    ),
                    Some(tp) => {
                        let v = text_child(text);
                        st.props.insert(tp.to_string(), v);
                        self.lines.insert(format!("{id}.{tp}"), el.line);
                    }
                },
            }
        }
        let mut children: Vec<String> = Vec::new();
        for ch in el.children {
            if SLATE_ITEM_KINDS.contains(&ch.tag.as_str()) {
                let Some(item_spec) = spec.and_then(|s| s.items.get(ch.tag.as_str())) else {
                    let kinds: Vec<&str> = spec.map(|s| s.items.keys().copied().collect()).unwrap_or_default();
                    let message = if kinds.is_empty() {
                        format!("{} takes no items, not <{}>", el.tag, ch.tag)
                    } else {
                        format!(
                            "{} takes {}, not <{}>",
                            el.tag,
                            kinds.iter().map(|k| format!("<{k}>")).collect::<Vec<_>>().join(" and "),
                            ch.tag
                        )
                    };
                    self.error("T314", &message, ch.line, Ext { piece: Some(&id), ..Ext::default() });
                    continue;
                };
                if !matches!(st.props.get(item_spec.prop), Some(Value::Array(_))) {
                    st.props.insert(item_spec.prop.to_string(), Value::Array(Vec::new()));
                }
                let n = match st.props.get(item_spec.prop) {
                    Some(Value::Array(list)) => list.len(),
                    _ => 0,
                };
                self.lines.insert(format!("{id}.{}[{n}]", item_spec.prop), ch.line);
                let v = self.item(&ch, item_spec, &id, &format!("{}[{n}]", item_spec.prop));
                if let Some(Value::Array(list)) = st.props.get_mut(item_spec.prop) {
                    list.push(v);
                }
                continue;
            }
            if let Some(child) = self.piece(ch) {
                children.push(child);
            }
        }
        let piece = SlatePiece {
            kind: el.tag,
            props: (!st.props.is_empty()).then_some(st.props),
            children: (!children.is_empty()).then_some(children),
            when: st.when,
            on: st.on.map(Value::Object),
            fallback: st.fallback,
        };
        self.doc.pieces.insert(id.clone(), piece);
        Some(id)
    }

    fn piece_attr(
        &mut self,
        a: &Attr,
        id: &str,
        tag: &str,
        spec: Option<&'static SlatePieceModule>,
        st: &mut PieceState,
    ) -> Result<(), Fatal> {
        if a.name == "when" {
            if a.kind != Kind::Braced {
                let shown = if a.kind == Kind::Str { a.value.as_str() } else { "..." };
                return Err(fatal("T303", format!("when takes a formula in braces: when={{{shown}}}"), a.line));
            }
            st.when = Some(js_trim(&a.value).to_string());
            return Ok(());
        }
        if a.name == "fallback" {
            if a.kind == Kind::Str {
                st.fallback = Some(Value::String(a.value.clone()));
                return Ok(());
            }
            let text = if a.kind == Kind::Braced { string_literal(&a.value) } else { None };
            let Some(text) = text else {
                return Err(fatal("T303", "fallback is drop, a piece id or a sentence in braces".into(), a.line));
            };
            st.fallback = Some(json!({"text": text}));
            return Ok(());
        }
        if is_on_attr(&a.name) {
            let event = a.name[2..].to_ascii_lowercase();
            if !matches!(event.as_str(), "press" | "submit" | "change") {
                let fix = if a.name == "onInput" { "onChange" } else { "onPress" };
                self.error(
                    "A602",
                    &format!("{} is not an event; events are onPress, onSubmit and onChange", a.name),
                    a.line,
                    Ext { piece: Some(id), prop: Some(a.name.as_str()), fix: Some(fix) },
                );
                return Ok(());
            }
            let steps = self.steps(a, id);
            st.on.get_or_insert_with(Map::new).insert(event, Value::Array(steps));
            return Ok(());
        }
        if a.kind == Kind::Bare {
            let declared = spec.and_then(|s| s.props.get(a.name.as_str()));
            let non_bool = declared.is_some_and(|p| p.prop_type != SlatePropType::Boolean);
            let owner = spec.and_then(|s| owner_of(s, &a.name));
            if non_bool || (declared.is_none() && owner.is_some()) {
                let fix = match &owner {
                    Some(o) => format!("{o}=\"{}\"", a.name),
                    None => format!("{}=\"...\"", a.name),
                };
                self.error(
                    "P102",
                    &format!("{} is not a boolean prop of {tag}", a.name),
                    a.line,
                    Ext { piece: Some(id), prop: Some(a.name.as_str()), fix: Some(&fix) },
                );
                return Ok(());
            }
            st.props.insert(a.name.clone(), Value::Bool(true));
            return Ok(());
        }
        let is_path = spec.and_then(|s| s.props.get(a.name.as_str())).is_some_and(|p| p.prop_type == SlatePropType::Path);
        if is_path && a.kind == Kind::Braced && slate_own_name(&a.value).is_some() {
            st.props.insert(a.name.clone(), Value::String(js_trim(&a.value).to_string()));
            return Ok(());
        }
        let v = self.value(a);
        st.props.insert(a.name.clone(), v);
        Ok(())
    }

    pub(super) fn item(&mut self, el: &El, spec: &SlateItemSpec, piece_id: &str, at: &str) -> Value {
        let mut out = Map::new();
        for a in &el.attrs {
            self.lines.insert(format!("{piece_id}.{at}.{}", a.name), a.line);
            if is_on_attr(&a.name) {
                let event = a.name[2..].to_ascii_lowercase();
                if !spec.events.is_some_and(|list| list.contains(&event.as_str())) {
                    let extra = match spec.events {
                        Some(list) => {
                            format!("; it has {}", list.iter().map(|e| format!("on{}", upper_first(e))).collect::<Vec<_>>().join(", "))
                        }
                        None => String::new(),
                    };
                    let prop = format!("{at}.{}", a.name);
                    self.error(
                        "A602",
                        &format!("<{}> has no {}{extra}", el.tag, a.name),
                        a.line,
                        Ext { piece: Some(piece_id), prop: Some(&prop), ..Ext::default() },
                    );
                    continue;
                }
                let steps = self.steps(a, piece_id);
                let mut on = match out.get("on") {
                    Some(Value::Object(m)) => m.clone(),
                    _ => Map::new(),
                };
                on.insert(event, Value::Array(steps));
                out.insert("on".into(), Value::Object(on));
                continue;
            }
            if a.name == "when" {
                if a.kind != Kind::Braced {
                    let prop = format!("{at}.when");
                    self.error(
                        "T303",
                        "when takes a formula in braces",
                        a.line,
                        Ext { piece: Some(piece_id), prop: Some(&prop), ..Ext::default() },
                    );
                    continue;
                }
                out.insert("when".into(), Value::String(js_trim(&a.value).to_string()));
                continue;
            }
            let v = if a.kind == Kind::Bare { Value::Bool(true) } else { self.value(a) };
            out.insert(a.name.clone(), v);
        }
        if el.text.is_some() || !el.children.is_empty() || el.raw.is_some() {
            self.error(
                "P100",
                &format!("<{}> is self-closing: <{} ... />", el.tag, el.tag),
                el.line,
                Ext { piece: Some(piece_id), ..Ext::default() },
            );
        }
        Value::Object(out)
    }

    pub(super) fn value(&mut self, a: &Attr) -> Value {
        match a.kind {
            Kind::Bare => Value::Bool(true),
            Kind::Str => Value::String(a.value.clone()),
            Kind::Braced => {
                let text = js_trim(&a.value);
                if text.starts_with('[') && text.ends_with(']') {
                    Value::Array(self.array(a, "").unwrap_or_default())
                } else if text.starts_with('{') && text.ends_with('}') {
                    Value::Object(self.object(a, "").unwrap_or_default().into_iter().collect())
                } else {
                    value_text(text)
                }
            }
        }
    }

    pub(super) fn literal(&mut self, a: &Attr, piece: &str) -> Value {
        match a.kind {
            Kind::Str => Value::String(a.value.clone()),
            Kind::Bare => Value::Bool(true),
            Kind::Braced => match parse_literal(js_trim(&a.value)) {
                Ok(v) => v,
                Err(message) => {
                    self.error(
                        "S503",
                        &format!(
                            "start takes a literal (a string, a number, true, false, null, a list or a record of literals): {message}"
                        ),
                        a.line,
                        Ext { piece: Some(piece), prop: Some("start"), ..Ext::default() },
                    );
                    Value::Null
                }
            },
        }
    }

    pub(super) fn object(&mut self, a: &Attr, piece: &str) -> Option<IndexMap<String, Value>> {
        let text = js_trim(&a.value);
        if a.kind != Kind::Braced || !text.starts_with('{') || !text.ends_with('}') {
            self.error(
                "A601",
                &format!("{} takes an object: {}={{{{ NAME: $value }}}}", a.name, a.name),
                a.line,
                Ext { piece: Some(piece), prop: Some(a.name.as_str()), ..Ext::default() },
            );
            return None;
        }
        let mut out: IndexMap<String, Value> = IndexMap::new();
        for part in split_top(&text[1..text.len() - 1]) {
            match OBJECT_PART.captures(&part) {
                None => self.error(
                    "A601",
                    &format!("cannot read \"{}\" as key: value in {}", js_trim(&part), a.name),
                    a.line,
                    Ext { piece: Some(piece), prop: Some(a.name.as_str()), ..Ext::default() },
                ),
                Some(c) => {
                    let key = c.get(1).or(c.get(2)).or(c.get(3)).map_or("", |m| m.as_str()).to_string();
                    out.insert(key, value_text(c.get(4).map_or("", |m| m.as_str())));
                }
            }
        }
        Some(js_index(out))
    }

    pub(super) fn array(&mut self, a: &Attr, piece: &str) -> Option<Vec<Value>> {
        let text = js_trim(&a.value);
        if a.kind != Kind::Braced || !text.starts_with('[') || !text.ends_with(']') {
            self.error(
                "A601",
                &format!("{} takes a list: {}={{[$a, 'x']}}", a.name, a.name),
                a.line,
                Ext { piece: Some(piece), prop: Some(a.name.as_str()), ..Ext::default() },
            );
            return None;
        }
        Some(split_top(&text[1..text.len() - 1]).iter().map(|p| value_text(p)).collect())
    }

    fn mint(&mut self, ty: &str) -> String {
        let base = if is_piece_base(ty) { ty.to_string() } else { "piece".to_string() };
        let mut n = self.minted.get(&base).copied().unwrap_or(0);
        let id = loop {
            n += 1;
            let id = format!("{base}-{n}");
            if !(self.taken.contains(&id) || self.doc.pieces.contains_key(&id)) {
                break id;
            }
        };
        self.minted.insert(base, n);
        id
    }
}

impl Compiler {
    pub(super) fn at_of(&mut self, el: &El) -> Option<Value> {
        let a = attr_of(el, "at")?;
        let n = to_number_text(&a.value);
        if !(n.is_finite() && n.fract() == 0.0) {
            self.error("P105", "at is a whole number: at={0}", a.line, Ext::default());
        }
        Some(jnum(n))
    }
}
