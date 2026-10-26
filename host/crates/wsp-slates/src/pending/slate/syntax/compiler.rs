// SPDX-License-Identifier: AGPL-3.0-only
//! The compiler's document pass: the declarations, the file and the names a document opens with.
use super::*;

fn empty_doc() -> SlateDoc {
    SlateDoc {
        schema: SLATE_SCHEMA,
        kit: None,
        title: None,
        root: String::new(),
        values: IndexMap::new(),
        derived: IndexMap::new(),
        runs: IndexMap::new(),
        reactions: Vec::new(),
        pieces: IndexMap::new(),
        files: None,
        order: order::KeyOrder::Text,
    }
}

impl Compiler {
    pub(super) fn new(current: Option<&SlateDoc>) -> Compiler {
        let mut c = Compiler {
            errors: Vec::new(),
            lines: IndexMap::new(),
            doc: empty_doc(),
            minted: HashMap::new(),
            taken: HashSet::new(),
            names: IndexMap::new(),
            patch: false,
            infinite: Vec::new(),
        };
        if let Some(cur) = current {
            for id in cur.pieces.keys() {
                c.taken.insert(id.clone());
            }
            for (n, v) in &cur.values {
                c.names.insert(n.clone(), if v.secret == Some(true) { "secret" } else { "value" }.to_string());
            }
            for n in cur.derived.keys() {
                c.names.insert(n.clone(), "derived".to_string());
            }
            for n in cur.runs.keys() {
                c.names.insert(n.clone(), "run".to_string());
            }
        }
        c
    }

    pub(super) fn error(&mut self, code: &str, message: &str, line: i64, ext: Ext) {
        if self.errors.len() < SLATE_LIMITS.errors_per_pass {
            self.errors.push(problem(code, message, line, &ext));
        }
    }

    pub(super) fn report(&mut self, f: Fatal) {
        self.error(f.code, &f.message, f.line, Ext { fix: f.fix.as_deref(), ..Ext::default() });
    }

    pub(super) fn document(&mut self, el: El) {
        for a in &el.attrs {
            if a.name == "title" && a.kind == Kind::Str {
                self.doc.title = Some(a.value.clone());
            } else if a.name == "title" && a.kind == Kind::Braced && string_literal(&a.value).is_some() {
                self.doc.title = string_literal(&a.value);
            } else if a.name == "title" {
                self.error("T303", "title is a literal string: <slate title=\"...\">", a.line, Ext::default());
            } else {
                self.error(
                    "T302",
                    &format!("<slate> takes title alone, not {}", a.name),
                    a.line,
                    Ext { fix: Some("title"), ..Ext::default() },
                );
            }
        }
        if el.text.is_some() {
            self.error("P100", "<slate> takes elements, not text; put the words in a <text>", el.line, Ext::default());
        }
        let mut decls = Vec::new();
        let mut files = Vec::new();
        let mut pieces = Vec::new();
        for ch in lift_declarations(el.children) {
            if is_decl_tag(&ch.tag) {
                decls.push(ch);
            } else if is_file_decl_el(&ch) {
                files.push(ch);
            } else {
                pieces.push(ch);
            }
        }
        for d in &decls {
            if let Err(f) = self.declare_name(d) {
                self.report(f);
            }
        }
        for f in &files {
            self.file(f);
        }
        for p in &pieces {
            for id in explicit_ids(p) {
                self.taken.insert(id);
            }
        }
        for d in &decls {
            self.declaration(d);
        }
        if pieces.len() != 1 {
            let names: Vec<String> = pieces.iter().map(|p| attr_of(p, "id").map_or(p.tag.clone(), |a| a.value.clone())).collect();
            let message = if pieces.is_empty() {
                "there is no piece; write one, like <column>".to_string()
            } else {
                format!("{} pieces at the top ({}); wrap them in a <column>", pieces.len(), names.join(", "))
            };
            let line = pieces.get(1).map_or(el.line, |p| p.line);
            self.error("D202", &message, line, Ext { fix: Some("<column>...</column>"), ..Ext::default() });
        }
        let mut pieces = pieces.into_iter();
        if let Some(first) = pieces.next() {
            let root = self.piece(first).unwrap_or_default();
            self.doc.root = root;
        }
        for extra in pieces {
            self.piece(extra);
        }
    }

    pub(super) fn declare_name(&mut self, el: &El) -> Result<(), Fatal> {
        if el.tag == "when" {
            return Ok(());
        }
        let name = match attr_of(el, "name") {
            Some(a) if a.kind == Kind::Str => a,
            _ => return Err(fatal("P103", format!("<{}> needs name=\"...\"", el.tag), el.line)),
        };
        if !SLATE_NAME.is_match(&name.value) {
            let stripped = name.value.trim_start_matches(|c: char| !(c.is_ascii_lowercase() || c == '_'));
            let fix = js_replace_units(stripped, |c| c.is_ascii_alphanumeric() || c == '_', "_");
            return Err(Fatal {
                code: "P103",
                message: format!("\"{}\" is not a name: a lowercase letter or _, then letters, digits and _", name.value),
                line: name.line,
                fix: (!fix.is_empty()).then_some(fix),
            });
        }
        if self.names.contains_key(&name.value) && !self.patch {
            return Err(fatal("P104", format!("${} is declared twice", name.value), el.line));
        }
        self.names.insert(name.value.clone(), el.tag.clone());
        Ok(())
    }

    pub(super) fn declaration(&mut self, el: &El) {
        let allowed: &[&str] = match el.tag.as_str() {
            "value" => &["name", "start"],
            "secret" => &["name", "keep"],
            "derived" => &["name", "value"],
            "run" => &[
                "name",
                "cmd",
                "tool",
                "resource",
                "env",
                "args",
                "stdin",
                "on",
                "cwd",
                "timeout",
                "stream",
                "confirm",
                "every",
                "once",
                "always",
                "then",
                "description",
            ],
            "when" => &["id", "change", "done", "do"],
            _ => &[],
        };
        for a in &el.attrs {
            if !allowed.contains(&a.name.as_str()) {
                let fix = slate_nearest(&a.name, &strs(allowed));
                self.error(
                    "T302",
                    &format!("<{}> has no attribute {}; it takes {}", el.tag, a.name, allowed.join(", ")),
                    a.line,
                    Ext { fix: fix.as_deref(), ..Ext::default() },
                );
            }
        }
        let is_when = el.tag == "when";
        let has_content = el.text.is_some() || !el.children.is_empty();
        let when_ok = is_when && el.attrs.iter().all(|a| a.name != "do");
        let run_ok = el.tag == "run" && el.children.is_empty();
        if has_content && !when_ok && !run_ok {
            let hint =
                if is_when { "when takes do={steps}".to_string() } else { format!("<{}> is self-closing and takes no children", el.tag) };
            let fix = is_when.then_some("<when change={$id} do={start($check)} />");
            self.error("A601", &hint, el.line, Ext { fix, ..Ext::default() });
        }
        let name = attr_of(el, "name").map_or(String::new(), |a| a.value.clone());
        let key = if is_when { String::new() } else { format!("${name}") };
        if !is_when {
            self.lines.insert(key.clone(), el.line);
        }
        match el.tag.as_str() {
            "value" => {
                let start = match attr_of(el, "start") {
                    Some(a) => self.literal(a, &key),
                    None => Value::Null,
                };
                if start.is_null() && attr_of(el, "start").is_some_and(|a| a.kind == Kind::Braced && js_trim(&a.value) != "null") {
                    self.infinite.push(name.clone());
                }
                self.doc.values.insert(name, SlateValueDecl { start, secret: None, keep: None });
            }
            "secret" => {
                let keep = attr_of(el, "keep").is_some();
                self.doc.values.insert(name, SlateValueDecl { start: Value::Null, secret: Some(true), keep: keep.then_some(true) });
            }
            "derived" => match attr_of(el, "value") {
                Some(v) if v.kind == Kind::Braced => {
                    self.doc.derived.insert(name, js_trim(&v.value).to_string());
                    self.lines.insert(format!("{key}.value"), v.line);
                }
                _ => self.error(
                    "T304",
                    &format!("<derived name=\"{name}\"> needs value={{formula}}"),
                    el.line,
                    Ext { piece: Some(&key), prop: Some("value"), ..Ext::default() },
                ),
            },
            "run" => self.run(el, &name),
            "when" => self.reaction(el),
            _ => {}
        }
    }

    pub(super) fn file(&mut self, el: &El) -> Option<String> {
        for a in &el.attrs {
            if a.name != "name" {
                self.error("T302", &format!("<file name=\"...\"> takes name alone, not {}", a.name), a.line, Ext::default());
            }
        }
        let name = match attr_of(el, "name") {
            Some(a) if a.kind == Kind::Str => a.value.clone(),
            _ => {
                self.error("K708", "<file> needs name=\"...\", a plain file name like name=\"shape.py\"", el.line, Ext::default());
                return None;
            }
        };
        if self.doc.files.as_ref().is_some_and(|f| f.contains_key(&name)) {
            self.error("P104", &format!("the file {name} is declared twice"), el.line, Ext { piece: Some(&name), ..Ext::default() });
        }
        let raw = el.raw.clone().unwrap_or_default();
        let text = match FILE_BRACED.captures(&raw) {
            None => raw,
            Some(c) => dedent(&c[1]),
        };
        if js_trim(&text).is_empty() {
            self.error(
                "K700",
                &format!("<file name=\"{name}\"> is empty; its text goes inside: <file name=\"{name}\">{{`...`}}</file>"),
                el.line,
                Ext { piece: Some(&name), ..Ext::default() },
            );
        }
        self.doc.files.get_or_insert_with(IndexMap::new).insert(name.clone(), text);
        self.lines.insert(name.clone(), el.line);
        Some(name)
    }
}
