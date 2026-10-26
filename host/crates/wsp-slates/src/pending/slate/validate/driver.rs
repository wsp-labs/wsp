// SPDX-License-Identifier: AGPL-3.0-only
//! The validator's driver: the names, limits, values, expressions and derived cycles it checks.
use super::*;

fn wrote(doc: &Value, w: &Where, fix: &str) -> bool {
    let (Some(piece), Some(prop)) = (&w.piece, &w.prop) else {
        return false;
    };
    let Some(v) = doc.get("pieces").and_then(|p| p.get(piece)).and_then(|p| p.get("props")).and_then(|p| p.get(prop)) else {
        return false;
    };
    let texts: Vec<String> = match v {
        Value::String(s) => vec![s.clone(), format!("{prop}=\"{s}\"")],
        _ if is_slate_binding(v) => {
            let b = bind_text(v);
            vec![b.to_string(), format!("{{{b}}}"), format!("{prop}={{{b}}}")]
        }
        _ => Vec::new(),
    };
    let flat = |x: &str| -> String { x.chars().filter(|c| !is_js_whitespace(*c)).collect() };
    texts.iter().any(|t| flat(t) == flat(fix))
}

impl Validator {
    pub(super) fn new(doc: Value, lines: Option<SlateLines>, infinite: &[String]) -> Validator {
        let mut kinds: IndexMap<String, Kind> = IndexMap::new();
        for (n, v) in doc.get("values").and_then(Value::as_object).into_iter().flatten() {
            kinds.insert(n.clone(), if is_true(v.get("secret")) { Kind::Secret } else { Kind::Value });
        }
        for n in doc.get("derived").and_then(Value::as_object).into_iter().flatten().map(|(n, _)| n) {
            kinds.insert(n.clone(), Kind::Derived);
        }
        for n in doc.get("runs").and_then(Value::as_object).into_iter().flatten().map(|(n, _)| n) {
            kinds.insert(n.clone(), Kind::Run);
        }
        Validator {
            ctx: Rc::new(Ctx {
                doc,
                kinds,
                derived_types: std::cell::RefCell::new(IndexMap::new()),
                derived_depth: std::cell::Cell::new(0),
                infinite: infinite.iter().cloned().collect(),
            }),
            lines,
            errors: Vec::new(),
            warnings: Vec::new(),
            seen: HashSet::new(),
            started: HashSet::new(),
        }
    }

    fn line_of(&self, w: &Where) -> Option<i64> {
        let lines = self.lines.as_ref()?;
        let piece = w.piece.as_ref()?;
        let mut key = match &w.prop {
            Some(p) => format!("{piece}.{p}"),
            None => piece.clone(),
        };
        loop {
            if let Some(hit) = lines.get(key.as_str()) {
                return Some(*hit);
            }
            let cut = key.rfind('.').into_iter().chain(key.rfind('[')).max().map_or(-1, |c| c as i64);
            if cut <= 0 {
                return None;
            }
            key.truncate(cut as usize);
        }
    }

    pub(super) fn add(&mut self, code: &str, message: &str, w: &Where, fix: Option<&str>) {
        let fix = fix.filter(|f| !wrote(&self.ctx.doc, w, f));
        let line = self.line_of(w);
        let mut p = slate_problem(code, message, fix);
        p.piece = w.piece.clone();
        p.prop = w.prop.clone();
        p.line = line;
        if SLATE_WARNINGS.contains(&code) {
            self.warnings.push(p);
            return;
        }
        let key = format!("{code}|{}|{}|{message}", opt_text(&w.piece), opt_text(&w.prop));
        if self.seen.contains(&key) || self.errors.len() >= SLATE_LIMITS.errors_per_pass {
            return;
        }
        self.seen.insert(key);
        self.errors.push(p);
    }

    pub(super) fn run(&mut self) {
        let ctx = Rc::clone(&self.ctx);
        let d = &ctx.doc;
        let schema = d.get("schema");
        if schema.and_then(Value::as_f64) != Some(2.0) {
            self.add(
                "D200",
                &format!("schema {} is not a version this host knows; this host reads schema 2", js_opt(schema)),
                &Where::default(),
                Some("update wsp"),
            );
        }
        if let Some(kit) = d.get("kit") {
            if kit.as_str() != Some("wsp/2") {
                self.add("D201", &format!("kit {} is not one this host has; it has wsp/2", js_str(kit)), &Where::default(), None);
            }
        }
        if let Some(title) = d.get("title").and_then(Value::as_str) {
            if title_cased(title) {
                self.add(
                    "W018",
                    &format!("the slate's title \"{title}\" is in Title Case; heads take sentence case"),
                    &Where::default(),
                    Some(format!("title=\"{}\"", sentence_case(title)).as_str()),
                );
            }
        }
        let runs: Vec<&Value> = d.get("runs").and_then(Value::as_object).into_iter().flatten().map(|(_, r)| r).collect();
        let mut run_texts: Vec<String> = Vec::new();
        for r in &runs {
            if r.get("kind").and_then(Value::as_str) == Some("cmd") {
                run_texts.push(js_opt(r.get("cmd")));
                run_texts.push(nn(r.get("then")).map_or_else(String::new, js_str));
            }
        }
        let token_doc = {
            let mut t = d.clone();
            if let Some(m) = t.as_object_mut() {
                m.remove("files");
                if let Some(Value::Object(rs)) = m.get_mut("runs") {
                    for r in rs.values_mut() {
                        if r.get("kind").and_then(Value::as_str) == Some("cmd") {
                            if let Some(rm) = r.as_object_mut() {
                                rm.insert("cmd".to_string(), Value::String(String::new()));
                                rm.remove("then");
                            }
                        }
                    }
                }
            }
            t
        };
        if let Some(elsewhere) = token_in(&json_stringify(&token_doc)) {
            if !run_texts.iter().any(|t| token_in(t).is_some()) {
                self.add(
                    "S520",
                    &format!("the slate holds a token written out ({elsewhere}...), which it keeps and shows the person; let a run's command read it from the file it lives in, or take it from a <secret> the person types"),
                    &Where::default(),
                    None,
                );
            }
        }
        let files: Vec<(String, String)> =
            d.get("files").and_then(Value::as_object).into_iter().flatten().map(|(n, t)| (n.clone(), js_str(t))).collect();
        for (file, text) in &files {
            if let Some(token) = token_in(text) {
                self.add(
                    "S520",
                    &format!("{file} holds a token written out ({token}...); let the code read it from the file it lives in, or from an env the run hands it"),
                    &Where::piece(file),
                    None,
                );
            }
        }
        let size_doc = {
            let mut t = d.clone();
            if let Some(m) = t.as_object_mut() {
                m.remove("files");
            }
            t
        };
        if json_stringify(&size_doc).len() > SLATE_LIMITS.document_bytes {
            self.add("D208", &format!("the document is over {}", fmt_bytes(SLATE_LIMITS.document_bytes as f64)), &Where::default(), None);
        }
        self.names();
        self.limits();
        for (name, v) in d.get("values").and_then(Value::as_object).into_iter().flatten() {
            self.value(name, v.get("start"), is_true(v.get("secret")), is_true(v.get("keep")));
        }
        for (name, expr) in d.get("derived").and_then(Value::as_object).into_iter().flatten() {
            self.expr(js_str(expr).as_str(), &Where::at(&format!("${name}"), "value"), ExprOpts::default());
        }
        self.derived_cycles();
        for (name, r) in d.get("runs").and_then(Value::as_object).into_iter().flatten() {
            self.run_decl(name, r);
        }
        self.files();
        for (i, r) in d.get("reactions").and_then(Value::as_array).into_iter().flatten().enumerate() {
            self.reaction(r, i);
        }
        self.reaction_cycles();
        self.tree();
        self.loud();
        for (name, r) in d.get("runs").and_then(Value::as_object).into_iter().flatten() {
            if r.get("every").is_none() && !self.started.contains(name) {
                self.add(
                    "W019",
                    &format!("nothing starts ${name}: give it a button, a when, or every"),
                    &Where::default(),
                    Some(format!("<button label=\"...\" onPress={{start(${name})}} />").as_str()),
                );
            }
        }
    }

    pub(super) fn names(&mut self) {
        let ctx = Rc::clone(&self.ctx);
        for name in ctx.kinds.keys() {
            if !SLATE_NAME_RE.is_match(name) {
                self.add(
                    "P103",
                    &format!("\"{name}\" is not a name: a lowercase letter or _, then letters, digits and _"),
                    &Where::piece(&format!("${name}")),
                    None,
                );
            }
        }
        let d = &ctx.doc;
        let mut counts: IndexMap<String, usize> = IndexMap::new();
        for section in ["values", "derived", "runs"] {
            for (n, _) in d.get(section).and_then(Value::as_object).into_iter().flatten() {
                *counts.entry(n.clone()).or_insert(0) += 1;
            }
        }
        for (n, c) in &counts {
            if *c > 1 {
                self.add("P104", &format!("${n} is declared twice"), &Where::piece(&format!("${n}")), None);
            }
        }
        for (id, _) in d.get("pieces").and_then(Value::as_object).into_iter().flatten() {
            if !SLATE_ID_RE.is_match(id) {
                self.add(
                    "P103",
                    &format!("\"{id}\" is not a piece id: letters, digits, dashes and _, starting with a letter"),
                    &Where::piece(id),
                    None,
                );
            }
        }
        let reaction_ids: Vec<String> = d
            .get("reactions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|r| r.get("id").and_then(Value::as_str).map(str::to_string))
            .collect();
        let mut unique: IndexMap<&str, ()> = IndexMap::new();
        for id in &reaction_ids {
            unique.insert(id.as_str(), ());
        }
        for id in unique.keys() {
            if reaction_ids.iter().filter(|x| x.as_str() == *id).count() > 1 {
                self.add("P104", &format!("reaction {id} is declared twice"), &Where::piece(id), None);
            }
        }
    }

    fn limits(&mut self) {
        let ctx = Rc::clone(&self.ctx);
        let d = &ctx.doc;
        let count = |section: &str| d.get(section).and_then(Value::as_object).map_or(0, Map::len);
        let over = |this: &mut Validator, n: usize, max: usize, code: &str, what: &str, fix: Option<&str>| {
            if n > max {
                this.add(code, &format!("{n} {what}; the most is {max}"), &Where::default(), fix);
            }
        };
        over(self, count("values"), SLATE_LIMITS.values, "S513", "values", None);
        over(self, count("derived"), SLATE_LIMITS.derived, "S512", "derived values", None);
        over(self, count("runs"), SLATE_LIMITS.runs, "K707", "runs", None);
        let reactions = d.get("reactions").and_then(Value::as_array).map_or(0, Vec::len);
        over(self, reactions, SLATE_LIMITS.reactions, "A611", "reactions", None);
        over(
            self,
            count("pieces"),
            SLATE_LIMITS.pieces,
            "D207",
            "pieces",
            Some("bind a list to a table or a list instead of writing rows"),
        );
        let starts: Vec<Value> = d
            .get("values")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .map(|(_, v)| v.get("start").cloned().unwrap_or(Value::Null))
            .collect();
        if json_stringify(&Value::Array(starts)).len() > SLATE_LIMITS.values_bytes {
            self.add(
                "S500",
                &format!("the values' starts are over {}", fmt_bytes(SLATE_LIMITS.values_bytes as f64)),
                &Where::default(),
                None,
            );
        }
    }

    pub(super) fn value(&mut self, name: &str, start: Option<&Value>, secret: bool, keep: bool) {
        let w = Where::at(&format!("${name}"), "start");
        if secret && !start.is_some_and(Value::is_null) {
            self.add("S503", &format!("${name} is a secret and starts empty; the person types it"), &w, None);
        }
        if !secret && keep {
            self.add("T302", &format!("keep is for a <secret>; ${name} is a value"), &w, None);
        }
    }

    pub(super) fn expr(&mut self, src: &str, w: &Where, opts: ExprOpts) -> SlateType {
        let ctx = Rc::clone(&self.ctx);
        let checked = check_slate_expression(src, &ctx.scope(opts.row, opts.secret_ok, opts.enum_words), 0);
        for p in checked.problems {
            self.add(&p.code, &p.message, w, p.fix.as_deref());
        }
        checked.ty
    }

    pub(super) fn prop_value(&mut self, v: &Value, w: &Where, opts: &ExprOpts) -> Option<SlateType> {
        if !v.is_object() {
            return None;
        }
        if is_slate_binding(v) {
            return Some(self.expr(bind_text(v), w, opts.clone()));
        }
        if is_slate_format(v) {
            let parsed = parse_slate_format(v.get("format").and_then(Value::as_str).unwrap_or(""));
            for e in &parsed.errors {
                self.add(&e.code, &e.message, w, None);
            }
            for p in &parsed.parts {
                if let SlateFormatPart::Hole { expr, .. } = p {
                    self.expr(expr, w, opts.clone());
                }
            }
            return Some(ty_of(SlateTypeName::String));
        }
        if let Some(items) = v.as_array() {
            for (i, x) in items.iter().enumerate() {
                let sub = Where { piece: w.piece.clone(), prop: Some(format!("{}[{i}]", opt_text(&w.prop))) };
                self.prop_value(x, &sub, opts);
            }
            return Some(ty_of(SlateTypeName::List));
        }
        if let Some(m) = v.as_object() {
            for (k, x) in m {
                let sub = Where { piece: w.piece.clone(), prop: Some(format!("{}.{k}", opt_text(&w.prop))) };
                self.prop_value(x, &sub, opts);
            }
            return Some(ty_of(SlateTypeName::Record));
        }
        None
    }

    fn derived_cycles(&mut self) {
        let ctx = Rc::clone(&self.ctx);
        let derived: Vec<(String, String)> =
            ctx.doc.get("derived").and_then(Value::as_object).into_iter().flatten().map(|(n, e)| (n.clone(), js_str(e))).collect();
        let mut deps: IndexMap<String, Vec<String>> = IndexMap::new();
        for (name, expr) in &derived {
            let roots_of: Vec<String> = roots(&slate_dependencies(expr)).into_iter().filter(|n| ctx.derived_expr(n).is_some()).collect();
            deps.insert(name.clone(), roots_of);
        }
        let mut state: IndexMap<String, bool> = IndexMap::new();
        let mut reported: HashSet<String> = HashSet::new();
        for n in deps.keys().cloned().collect::<Vec<_>>() {
            self.derived_visit(&n, &deps, &mut state, &mut reported);
        }
    }

    fn derived_visit(
        &mut self,
        root: &str,
        deps: &IndexMap<String, Vec<String>>,
        state: &mut IndexMap<String, bool>,
        reported: &mut HashSet<String>,
    ) {
        let mut stack: Vec<(String, usize)> = Vec::new();
        let mut entering = Some(root.to_string());
        loop {
            if let Some(n) = entering.take() {
                match state.get(&n) {
                    Some(&true) => {}
                    Some(&false) => {
                        let start = stack.iter().position(|(s, _)| *s == n).unwrap_or(0);
                        let mut cycle: Vec<String> = stack[start..].iter().map(|(s, _)| s.clone()).collect();
                        cycle.push(n.clone());
                        let mut sorted = cycle.clone();
                        sorted.sort();
                        if reported.insert(sorted.join(",")) {
                            let shown: Vec<String> = cycle.iter().map(|x| format!("${x}")).collect();
                            self.add(
                                "S510",
                                &format!("derived values read each other in a cycle: {}", shown.join(" reads ")),
                                &Where::at(&format!("${n}"), "value"),
                                Some("make one of them a value"),
                            );
                        }
                    }
                    None => {
                        state.insert(n.clone(), false);
                        stack.push((n, 0));
                    }
                }
            }
            let Some((n, at)) = stack.last_mut() else { return };
            match deps.get(n.as_str()).and_then(|d| d.get(*at)) {
                Some(m) => {
                    *at += 1;
                    entering = Some(m.clone());
                }
                None => {
                    let (done, _) = stack.pop().expect("a frame");
                    state.insert(done, true);
                }
            }
        }
    }
}
