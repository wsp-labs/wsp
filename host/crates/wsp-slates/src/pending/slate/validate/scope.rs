// SPDX-License-Identifier: AGPL-3.0-only
//! The validator's type scope: the types a path, a run field and a secret read as.
use super::*;

pub(super) fn any_t() -> SlateType {
    SlateType { t: SlateTypeName::Any, of: None, fields: None }
}

pub(super) fn ty_of(t: SlateTypeName) -> SlateType {
    SlateType { t, of: None, fields: None }
}

fn literal_type(v: &Value) -> SlateType {
    match v {
        Value::Null => any_t(),
        Value::Number(_) => ty_of(SlateTypeName::Number),
        Value::String(_) => ty_of(SlateTypeName::String),
        Value::Bool(_) => ty_of(SlateTypeName::Boolean),
        Value::Array(items) => match items.first() {
            None => ty_of(SlateTypeName::List),
            Some(first) => SlateType { t: SlateTypeName::List, of: Some(Box::new(literal_type(first))), fields: None },
        },
        Value::Object(m) => {
            if m.is_empty() {
                ty_of(SlateTypeName::Record)
            } else {
                let fields = m.iter().map(|(k, x)| (k.clone(), literal_type(x))).collect();
                SlateType { t: SlateTypeName::Record, of: None, fields: Some(fields) }
            }
        }
    }
}

fn run_field_types() -> IndexMap<String, SlateType> {
    let list_of_strings = SlateType { t: SlateTypeName::List, of: Some(Box::new(ty_of(SlateTypeName::String))), fields: None };
    [
        ("state", ty_of(SlateTypeName::String)),
        ("why", ty_of(SlateTypeName::String)),
        ("exit", ty_of(SlateTypeName::Number)),
        ("out", any_t()),
        ("err", ty_of(SlateTypeName::String)),
        ("json", any_t()),
        ("lines", list_of_strings),
        ("startedAt", ty_of(SlateTypeName::Number)),
        ("endedAt", ty_of(SlateTypeName::Number)),
        ("ms", ty_of(SlateTypeName::Number)),
        ("runs", ty_of(SlateTypeName::Number)),
        ("cut", ty_of(SlateTypeName::Boolean)),
        ("stale", ty_of(SlateTypeName::Boolean)),
        ("refreshing", ty_of(SlateTypeName::Boolean)),
        ("text", ty_of(SlateTypeName::Boolean)),
    ]
    .into_iter()
    .map(|(k, t)| (k.to_string(), t))
    .collect()
}

fn secret_field_type(field: &str) -> SlateType {
    match field {
        "set" => ty_of(SlateTypeName::Boolean),
        _ => ty_of(SlateTypeName::Number),
    }
}

impl Ctx {
    pub(super) fn derived_expr(&self, name: &str) -> Option<&str> {
        self.doc.get("derived").and_then(|d| d.get(name)).and_then(Value::as_str)
    }

    pub(super) fn scope(self: &Rc<Self>, row: Option<SlateType>, secret_ok: bool, enum_words: Option<Vec<String>>) -> SlateCheckScope {
        let ctx = Rc::clone(self);
        SlateCheckScope {
            row,
            path: Box::new(move |head: &str, own: bool, segs: &[PathSeg]| {
                if own {
                    return ctx.own_type(head, segs, secret_ok);
                }
                if ctx.kinds.contains_key(head) {
                    return Err(SlateScopeProblem {
                        code: "X401",
                        message: format!("{head} is not a path; the slate's own values are ${head}"),
                        fix: Some(format!("${head}")),
                    });
                }
                if enum_words.as_ref().is_some_and(|words| words.iter().any(|w| w == head)) {
                    return Err(SlateScopeProblem {
                        code: "X401",
                        message: format!("{head} is not a path; an enum word is a string"),
                        fix: Some(format!("\"{head}\"")),
                    });
                }
                slate_source_type(head, segs)
            }),
        }
    }

    fn own_type(self: &Rc<Self>, name: &str, segs: &[PathSeg], secret_ok: bool) -> Result<SlateType, SlateScopeProblem> {
        let Some(kind) = self.kinds.get(name).copied() else {
            let names: Vec<String> = self.kinds.keys().cloned().collect();
            let fix = slate_nearest(name, &names);
            let hint = fix.as_ref().map_or(String::new(), |f| format!(" or did you mean ${f}?"));
            return Err(SlateScopeProblem {
                code: "S501",
                message: format!("${name} is not declared; add <value name=\"{name}\" start=... />{hint}"),
                fix: fix.map(|f| format!("${f}")),
            });
        };
        let first = segs.first();
        match kind {
            Kind::Secret => {
                if let Some(PathSeg::Key(field)) = first {
                    if SECRET_FIELDS.contains(&field.as_str()) {
                        return Ok(secret_field_type(field));
                    }
                }
                if secret_ok && segs.is_empty() {
                    return Ok(any_t());
                }
                Err(SlateScopeProblem {
                    code: "S520",
                    message: format!("${name} is a secret; only a run's env or stdin, an input's value or a send's paths may name it, and a formula reads only its .set, .len and .at"),
                    fix: Some(format!("${name}.set")),
                })
            }
            Kind::Run => {
                let Some(first) = first else {
                    let fields = run_field_types();
                    return Ok(SlateType { t: SlateTypeName::Record, of: None, fields: Some(fields) });
                };
                let field = match first {
                    PathSeg::Key(k) if RUN_FIELDS.contains(&k.as_str()) => k.clone(),
                    other => {
                        let shown = match other {
                            PathSeg::Key(k) => k.clone(),
                            PathSeg::Index(i) => i.to_string(),
                        };
                        let fix = slate_nearest(&shown, &RUN_FIELDS.iter().map(|s| s.to_string()).collect::<Vec<_>>());
                        return Err(SlateScopeProblem {
                            code: "X401",
                            message: format!("${name}.{shown} is not a field of a run's result; the fields are {}", RUN_FIELDS.join(", ")),
                            fix: fix.map(|f| format!("${name}.{f}")),
                        });
                    }
                };
                if segs.len() == 1 {
                    Ok(run_field_types().get(&field).cloned().unwrap_or_else(any_t))
                } else {
                    Ok(any_t())
                }
            }
            Kind::Value | Kind::Derived => {
                let mut t = if kind == Kind::Derived {
                    self.derived_type(name)
                } else {
                    literal_type(self.doc.get("values").and_then(|v| v.get(name)).and_then(|v| v.get("start")).unwrap_or(&Value::Null))
                };
                for seg in segs {
                    t = match (seg, &t) {
                        (PathSeg::Index(_), SlateType { t: SlateTypeName::List, of, .. }) => of.as_deref().cloned().unwrap_or_else(any_t),
                        (PathSeg::Key(key), SlateType { t: SlateTypeName::Record, fields: Some(fields), .. }) => {
                            fields.get(key).cloned().unwrap_or_else(any_t)
                        }
                        _ => any_t(),
                    };
                }
                Ok(t)
            }
        }
    }

    fn derived_type(self: &Rc<Self>, name: &str) -> SlateType {
        if let Some(hit) = self.derived_types.borrow().get(name) {
            return hit.clone();
        }
        self.derived_types.borrow_mut().insert(name.to_string(), any_t());
        let t = match self.derived_expr(name) {
            None => any_t(),
            Some(expr) => check_slate_expression(expr, &self.scope(None, false, None), 0).ty,
        };
        self.derived_types.borrow_mut().insert(name.to_string(), t.clone());
        t
    }
}
