// SPDX-License-Identifier: AGPL-3.0-only
//! The validator's runs, files and secrets: each run declaration and the files it reads.
use super::*;

impl Validator {
    pub(super) fn run_decl(&mut self, name: &str, r: &Value) {
        let piece = format!("${name}");
        let w = |prop: &str| Where::at(&piece, prop);
        if let Some(every) = r.get("every") {
            if !every.as_f64().is_some_and(|x| x.is_finite() && x >= SLATE_LIMITS.timer_floor_s as f64) {
                self.add(
                    "K703",
                    &format!("every is at least {} s", SLATE_LIMITS.timer_floor_s),
                    &w("every"),
                    Some(format!("every={{{}}}", SLATE_LIMITS.timer_floor_s).as_str()),
                );
            }
        }
        if let Some(confirm) = nn(r.get("confirm")).or_else(|| r.get("confirm").filter(|c| !c.is_null())) {
            self.prop_value(confirm, &w("confirm"), &ExprOpts::default());
        }
        if let Some(then) = r.get("then") {
            if then.as_str().is_none_or(|t| js_trim(t).is_empty()) {
                self.add("K700", "then is a literal command that reads the result on stdin", &w("then"), Some("then='python3 shape.py'"));
            }
        }
        if is_true(r.get("always")) && r.get("every").is_none() {
            self.add(
                "K703",
                "always keeps a timer running while the slate is not shown, so it needs every",
                &w("always"),
                Some("every={60} always"),
            );
        }
        let kind = r.get("kind").and_then(Value::as_str);
        if kind == Some("cmd") {
            let cmd = r.get("cmd");
            if cmd.and_then(Value::as_str).is_none_or(|c| js_trim(c).is_empty()) {
                self.add("K700", "the command is literal text; hand values to it with env={{ NAME: $value }}", &w("cmd"), None);
            }
            for (field, text) in [("cmd", r.get("cmd")), ("then", r.get("then"))] {
                if let Some(token) = text.and_then(Value::as_str).and_then(token_in) {
                    self.add(
                        "S520",
                        &format!("{field} holds a token written out ({token}...), which the slate keeps and shows the person; let the command read it from the file it lives in, or take it from a <secret> the person types"),
                        &w(field),
                        None,
                    );
                }
            }
            if let Some(timeout) = r.get("timeout") {
                if !timeout.as_f64().is_some_and(|t| t.is_finite() && t > 0.0 && t <= SLATE_LIMITS.timeout_max_s as f64) {
                    self.add(
                        "K704",
                        &format!("timeout is seconds up to {}", SLATE_LIMITS.timeout_max_s),
                        &w("timeout"),
                        Some(format!("timeout={{{}}}", SLATE_LIMITS.timeout_default_s).as_str()),
                    );
                }
            }
            if let Some(on) = r.get("on") {
                if on.as_str() != Some("thread") && on.as_str() != Some("host") {
                    self.add("K704", "on is \"thread\" or \"host\"", &w("on"), None);
                }
            }
            let mut secret_env: Vec<(String, String)> = Vec::new();
            for (k, v) in r.get("env").and_then(Value::as_object).into_iter().flatten() {
                if !ENV_NAME_RE.is_match(k) {
                    self.add(
                        "K702",
                        &format!("{k} is not an environment variable name: {ENV_NAME_FIX}"),
                        &w("env"),
                        Some(
                            k.to_uppercase()
                                .chars()
                                .map(|c| if c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_' { c } else { '_' })
                                .collect::<String>()
                                .as_str(),
                        ),
                    );
                }
                self.prop_value(v, &w(&format!("env.{k}")), &ExprOpts { secret_ok: true, ..ExprOpts::default() });
                for s in self.secrets_in(v) {
                    secret_env.push((k.clone(), s));
                }
            }
            for (i, v) in r.get("args").and_then(Value::as_array).into_iter().flatten().enumerate() {
                let secret = if is_slate_binding(v) { slate_own_name(bind_text(v)) } else { None };
                let secret_kind = secret.as_ref().and_then(|s| self.ctx.kinds.get(s).copied());
                match secret {
                    Some(s) if secret_kind == Some(Kind::Secret) => {
                        self.add(
                            "S520",
                            &format!("args[{i}] is the secret ${s}; {SLATE_SECRET_IN_ARGS}"),
                            &w(&format!("args[{i}]")),
                            Some(format!("env={{{{ {}: ${s} }}}}, or stdin={{${s}}}", s.to_uppercase()).as_str()),
                        );
                    }
                    _ => {
                        self.prop_value(v, &w(&format!("args[{i}]")), &ExprOpts::default());
                    }
                }
            }
            if let Some(stdin) = r.get("stdin") {
                self.prop_value(stdin, &w("stdin"), &ExprOpts { secret_ok: true, ..ExprOpts::default() });
            }
            let cmd_text = js_opt(r.get("cmd"));
            for (env_name, secret) in &secret_env {
                if let Some(m) = flag_re(env_name).captures(&cmd_text) {
                    let flag = m.get(1).map_or("", |g| g.as_str());
                    self.add(
                        "W011",
                        &format!("{env_name} carries the secret ${secret}, and the command passes it after {flag}, as an argument ps can read; hand it on stdin={{${secret}}}, or let the program read {env_name} from its environment"),
                        &w("cmd"),
                        Some(format!("drop {flag} \"${env_name}\" and use stdin={{${secret}}} or the program's own {env_name}").as_str()),
                    );
                }
            }
        } else if kind == Some("tool") {
            let server = r.get("server").and_then(Value::as_str).unwrap_or("");
            let tool = r.get("tool").and_then(Value::as_str).unwrap_or("");
            if server.is_empty() || tool.is_empty() {
                self.add("K705", "tool names a server and a tool as \"server.tool\"", &w("tool"), None);
            }
            for (k, v) in r.get("args").and_then(Value::as_object).into_iter().flatten() {
                self.prop_value(v, &w(&format!("args.{k}")), &ExprOpts { secret_ok: true, ..ExprOpts::default() });
            }
        } else if kind == Some("resource") {
            let server = r.get("server").and_then(Value::as_str).unwrap_or("");
            let uri = r.get("uri").and_then(Value::as_str).unwrap_or("");
            if server.is_empty() || uri.is_empty() {
                self.add("K705", "resource names a server and a uri as \"server:uri\"", &w("resource"), None);
            }
        } else {
            self.add("K701", &format!("{piece} takes exactly one of cmd, tool or resource"), &w("kind"), None);
        }
    }

    pub(super) fn files(&mut self) {
        let ctx = Rc::clone(&self.ctx);
        let files: Vec<(String, String)> =
            ctx.doc.get("files").and_then(Value::as_object).into_iter().flatten().map(|(n, t)| (n.clone(), js_str(t))).collect();
        let names: Vec<String> = files.iter().map(|(n, _)| n.clone()).collect();
        if files.len() > SLATE_LIMITS.files {
            self.add("K709", &format!("{} files; the most is {}", files.len(), SLATE_LIMITS.files), &Where::default(), None);
        }
        let bytes: usize = files.iter().map(|(_, t)| t.len()).sum();
        if bytes > SLATE_LIMITS.files_bytes {
            self.add(
                "K709",
                &format!("the files hold {} KB; the most is {} KB in all", bytes.div_ceil(1024), SLATE_LIMITS.files_bytes / 1024),
                &Where::default(),
                Some("keep code that belongs to the project in the project and call it where it is"),
            );
        }
        let secrets: Vec<String> = ctx.kinds.iter().filter(|(_, k)| **k == Kind::Secret).map(|(n, _)| n.clone()).collect();
        for (name, text) in &files {
            if !SLATE_FILE_RE.is_match(name) {
                let fix: String = name
                    .rsplit('/')
                    .next()
                    .unwrap_or("")
                    .trim_start_matches('.')
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' { c } else { '_' })
                    .collect();
                self.add(
                    "K708",
                    &format!("\"{name}\" is not a file name: letters, digits, dots, dashes and _, no slashes, not starting with a dot"),
                    &Where::piece(name),
                    if fix.is_empty() { None } else { Some(fix.as_str()) },
                );
            }
            for secret in &secrets {
                if secret_ref_re(secret).is_match(text) {
                    self.add(
                        "S520",
                        &format!("{name} names the secret ${secret}; a file is stored as written, so a run hands the secret to it on stdin or in env"),
                        &Where::piece(name),
                        Some(format!("env={{{{ {}: ${secret} }}}} on the run, read from the environment in {name}", secret.to_uppercase()).as_str()),
                    );
                }
            }
        }
        for (run, r) in ctx.doc.get("runs").and_then(Value::as_object).into_iter().flatten() {
            for prop in ["cmd", "then"] {
                let Some(text) = r.get(prop).and_then(Value::as_str) else {
                    continue;
                };
                for m in SLATE_DIR_RE.captures_iter(text) {
                    let file = m.get(1).map_or("", |g| g.as_str());
                    if !files.iter().any(|(n, _)| n == file) {
                        let fix = slate_nearest(file, &names);
                        self.add(
                            "W015",
                            &format!("${run} runs $SLATE_DIR/{file}, and no <file name=\"{file}\"> is declared"),
                            &Where::at(&format!("${run}"), prop),
                            fix.as_deref(),
                        );
                    }
                }
            }
        }
    }

    fn secrets_in(&self, v: &Value) -> Vec<String> {
        let mut out = Vec::new();
        self.secrets_visit(v, &mut out);
        out
    }

    fn secrets_visit(&self, x: &Value, out: &mut Vec<String>) {
        if !x.is_object() && !x.is_array() {
            return;
        }
        if is_slate_binding(x) {
            for n in roots(&slate_dependencies(bind_text(x))) {
                if self.ctx.kinds.get(&n) == Some(&Kind::Secret) {
                    out.push(n);
                }
            }
            return;
        }
        if is_slate_format(x) {
            for p in parse_slate_format(x.get("format").and_then(Value::as_str).unwrap_or("")).parts {
                if let SlateFormatPart::Hole { expr, .. } = p {
                    for n in roots(&slate_dependencies(&expr)) {
                        if self.ctx.kinds.get(&n) == Some(&Kind::Secret) {
                            out.push(n);
                        }
                    }
                }
            }
            return;
        }
        match x {
            Value::Array(items) => items.iter().for_each(|y| self.secrets_visit(y, out)),
            Value::Object(m) => m.values().for_each(|y| self.secrets_visit(y, out)),
            _ => {}
        }
    }
}
