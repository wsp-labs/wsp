// SPDX-License-Identifier: AGPL-3.0-only
//! The validator's reactions: each reaction's trigger, its steps and the cycles they can close.
use super::*;

impl Validator {
    pub(super) fn reaction(&mut self, r: &Value, i: usize) {
        let key = match nn(r.get("id")).and_then(Value::as_str) {
            Some(id) => id.to_string(),
            None => format!("reaction-{}", i + 1),
        };
        let on = r.get("on");
        let change = on.and_then(|o| o.get("change"));
        let done = on.and_then(|o| o.get("done"));
        if change.is_none() == done.is_none() {
            self.add("A611", "<when> takes exactly one of change={...} or done={$run}", &Where::piece(&key), None);
            return;
        }
        if let Some(paths) = change {
            let paths: Vec<String> = paths.as_array().map_or_else(Vec::new, |items| items.iter().map(js_str).collect());
            if paths.is_empty() {
                self.add("A611", "change lists at least one path", &Where::at(&key, "change"), None);
            }
            for p in &paths {
                if !matches!(parse_slate_expression(p).ast, Some(SlateExpr::Path { .. })) {
                    self.add("A601", &format!("change lists paths, not formulas: {p}"), &Where::at(&key, "change"), None);
                    continue;
                }
                self.expr(p, &Where::at(&key, "change"), ExprOpts { secret_ok: true, ..ExprOpts::default() });
            }
        } else if let Some(name) = done.and_then(Value::as_str) {
            if self.ctx.kinds.get(name) != Some(&Kind::Run) {
                let word = kind_word_of(self.ctx.kinds.get(name).copied()).unwrap_or("not declared");
                let runs: Vec<String> = self.run_names().into_iter().map(|n| format!("${n}")).collect();
                self.add(
                    "K702",
                    &format!("done names a run: ${name} is {word}"),
                    &Where::at(&key, "done"),
                    slate_nearest(name, &runs).as_deref(),
                );
            }
        }
        self.steps(r.get("do"), &Where::at(&key, "do"), "reaction", None);
    }

    pub(super) fn run_names(&self) -> Vec<String> {
        self.ctx.kinds.iter().filter(|(_, k)| **k == Kind::Run).map(|(n, _)| n.clone()).collect()
    }

    pub(super) fn steps(&mut self, steps: Option<&Value>, w: &Where, trigger: &str, row: Option<SlateType>) {
        let Some(steps) = steps.and_then(Value::as_array) else {
            self.add("A600", "a handler is a list of steps", w, None);
            return;
        };
        if steps.len() > SLATE_LIMITS.steps_per_reaction {
            self.add(
                "A606",
                &format!("{} steps in one handler; the most is {}", steps.len(), SLATE_LIMITS.steps_per_reaction),
                w,
                Some("split it: a second <when> on what the first one sets"),
            );
        }
        for s in steps {
            let do_text = js_opt(s.get("do"));
            let Some(module) = s.get("do").and_then(Value::as_str).and_then(slate_step_module) else {
                let fix = slate_nearest(&do_text, &STEP_KINDS.iter().map(|k| k.to_string()).collect::<Vec<_>>());
                self.add("A600", &format!("{do_text} is not a step; steps are {}", STEP_KINDS.join(", ")), w, fix.as_deref());
                continue;
            };
            if module.runs == "window" && trigger != "press" {
                let on = if trigger == "reaction" { "reaction".to_string() } else { format!("{trigger} handler") };
                self.add(
                    "A608",
                    &format!("{do_text} runs in the window and goes on a press, not on a {on}"),
                    w,
                    Some(format!("onPress={{{do_text}(...)}}").as_str()),
                );
            }
            match do_text.as_str() {
                "set" | "toggle" => {
                    let path = js_opt(s.get("path"));
                    let Some(p) = parse_slate_own_path(&path) else {
                        self.add("A601", &format!("{do_text} writes the slate's own values: a $name path, not {path}"), w, None);
                        continue;
                    };
                    match self.ctx.kinds.get(&p.name).copied() {
                        None => {
                            let names: Vec<String> = self.ctx.kinds.keys().cloned().collect();
                            self.add(
                                "S501",
                                &format!("${} is not declared; add <value name=\"{}\" start=... />", p.name, p.name),
                                w,
                                slate_nearest(&p.name, &names).as_deref(),
                            );
                        }
                        Some(Kind::Derived) => {
                            self.add(
                                "A607",
                                &format!("${} is derived and cannot be written; write the value it reads, or make it a value", p.name),
                                w,
                                None,
                            );
                        }
                        Some(Kind::Run) => {
                            self.add("A607", &format!("${} is a run; its result is the host's to write", p.name), w, None);
                        }
                        Some(Kind::Secret) => {
                            self.add("S520", &format!("${} is a secret; only the person's typing fills it", p.name), w, None);
                        }
                        Some(Kind::Value) => {}
                    }
                    if do_text == "set" {
                        self.prop_value(s.get("value").unwrap_or(&Value::Null), w, &ExprOpts { row: row.clone(), ..ExprOpts::default() });
                    }
                }
                "start" | "cancel" => {
                    let run = js_opt(s.get("run"));
                    if do_text == "start" {
                        self.started.insert(run.clone());
                    }
                    if self.ctx.kinds.get(&run) != Some(&Kind::Run) {
                        let word = kind_word_of(self.ctx.kinds.get(&run).copied()).unwrap_or("not declared");
                        let runs = self.run_names();
                        self.add("K702", &format!("{do_text} names a run: ${run} is {word}"), w, slate_nearest(&run, &runs).as_deref());
                    }
                }
                "send" | "steer" | "queue" | "fill" => {
                    if s.get("text").and_then(Value::as_str).is_none() {
                        self.add("A603", &format!("{do_text}'s text is a literal string"), w, None);
                    }
                    for p in s.get("with").and_then(Value::as_array).into_iter().flatten().map(js_str) {
                        if !matches!(parse_slate_expression(&p).ast, Some(SlateExpr::Path { .. })) {
                            self.add("A601", &format!("{do_text} carries paths after its text; {p} is a formula"), w, None);
                            continue;
                        }
                        self.expr(&p, w, ExprOpts { secret_ok: true, row: row.clone(), ..ExprOpts::default() });
                    }
                }
                "open" => {
                    if let Some(target) = s.get("target") {
                        self.prop_value(target, w, &ExprOpts { row: row.clone(), ..ExprOpts::default() });
                    }
                }
                "copy" => {
                    if let Some(text) = s.get("text") {
                        self.prop_value(text, w, &ExprOpts { row: row.clone(), ..ExprOpts::default() });
                    }
                }
                "pane" => {
                    let kind = js_opt(s.get("kind"));
                    if !PANE_KINDS.contains(&kind.as_str()) {
                        self.add(
                            "A601",
                            &format!("pane names one of {}", PANE_KINDS.join(", ")),
                            w,
                            slate_nearest(&kind, &PANE_KINDS.iter().map(|k| k.to_string()).collect::<Vec<_>>()).as_deref(),
                        );
                    }
                }
                _ => {}
            }
        }
    }

    pub(super) fn reaction_cycles(&mut self) {
        let ctx = Rc::clone(&self.ctx);
        let mut edges: Vec<(String, String, String)> = Vec::new();
        for (name, expr) in ctx.doc.get("derived").and_then(Value::as_object).into_iter().flatten() {
            for dep in slate_dependencies(&js_str(expr)) {
                edges.push((dep.clone(), format!("${name}"), format!("${name} reads {dep}")));
            }
        }
        for r in ctx.doc.get("reactions").and_then(Value::as_array).into_iter().flatten() {
            let Some(change) = r.get("on").and_then(|o| o.get("change")).and_then(Value::as_array) else {
                continue;
            };
            let targets: Vec<String> = r
                .get("do")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|s| matches!(s.get("do").and_then(Value::as_str), Some("set" | "toggle")))
                .map(|s| js_opt(s.get("path")))
                .collect();
            for t in change.iter().map(js_str) {
                for g in &targets {
                    edges.push((t.clone(), g.clone(), format!("when change of {t} sets {g}")));
                }
            }
        }
        let mut state: IndexMap<String, bool> = IndexMap::new();
        let mut reported = false;
        let froms: Vec<String> = edges.iter().map(|e| e.0.clone()).collect();
        for from in froms {
            self.cycle_visit(&from, &[], &edges, &mut state, &mut reported);
        }
    }

    fn cycle_visit(
        &mut self,
        n: &str,
        stack: &[(String, String)],
        edges: &[(String, String, String)],
        state: &mut IndexMap<String, bool>,
        reported: &mut bool,
    ) {
        if *reported || state.get(n) == Some(&true) {
            return;
        }
        if state.get(n) == Some(&false) {
            let idx = stack.iter().position(|(node, _)| node == n).map_or(0, |i| i + 1);
            let said: Vec<String> = stack[idx..].iter().map(|(_, why)| why.clone()).collect();
            if said.iter().any(|s| s.starts_with("when")) {
                *reported = true;
                self.add(
                    "A610",
                    &format!("reactions and derived values form a cycle: {}", said.join("; ")),
                    &Where::default(),
                    Some("compute it as a <derived> value instead of setting it back"),
                );
            }
            return;
        }
        state.insert(n.to_string(), false);
        let next: Vec<(String, String)> = edges.iter().filter(|e| paths_overlap(&e.0, n)).map(|e| (e.1.clone(), e.2.clone())).collect();
        for (to, why) in next {
            let mut sub = stack.to_vec();
            if stack.is_empty() {
                sub.push((n.to_string(), String::new()));
            }
            sub.push((to.clone(), why));
            self.cycle_visit(&to, &sub, edges, state, reported);
        }
        state.insert(n.to_string(), true);
    }
}
