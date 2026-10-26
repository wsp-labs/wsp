// SPDX-License-Identifier: AGPL-3.0-only
//! The static checker of expr.ts.
use super::*;

// ---- static checks ----

/// A field of a known type: a record's declared field, or any. None is a field the record does not declare.
fn field_type(of: &SlateType, seg: &PathSeg) -> Option<SlateType> {
    match seg {
        PathSeg::Index(_) => Some(if of.t == SlateTypeName::List { of.of.as_deref().cloned().unwrap_or_else(any_t) } else { any_t() }),
        PathSeg::Key(k) => {
            if of.t == SlateTypeName::Record {
                if let Some(fields) = &of.fields {
                    return fields.get(k).cloned();
                }
            }
            Some(any_t())
        }
    }
}

fn seg_text(seg: &PathSeg) -> String {
    match seg {
        PathSeg::Key(k) => k.clone(),
        PathSeg::Index(i) => i.to_string(),
    }
}

fn field_keys(t: &SlateType) -> Vec<String> {
    t.fields.as_ref().map_or_else(Vec::new, |f| f.keys().cloned().collect())
}

struct Checker<'s> {
    scope: &'s SlateCheckScope,
    base: usize,
    problems: Vec<SlateProblem>,
}

impl Checker<'_> {
    fn add(&mut self, code: &str, message: String, at: usize, fix: Option<String>) {
        self.problems.push(problem(code, &message, Some(at + self.base), fix.as_deref()));
    }

    fn ty(&mut self, node: &SlateExpr, row: Option<&SlateType>) -> SlateType {
        match node {
            SlateExpr::Lit { v, .. } => match v {
                Value::Null => null_t(),
                Value::Number(_) => num_t(),
                Value::String(_) => str_t(),
                Value::Bool(_) => bool_t(),
                _ => any_t(),
            },
            SlateExpr::Tpl { parts, .. } => {
                for p in parts {
                    if let SlateTemplatePart::Hole { expr, .. } = p {
                        self.ty(expr, row);
                    }
                }
                str_t()
            }
            SlateExpr::Index { of, index, .. } => {
                let of_t = self.ty(of, row);
                let at_t = self.ty(index, row);
                if matches!(at_t.t, SlateTypeName::Boolean | SlateTypeName::List | SlateTypeName::Record) {
                    let message = format!("an index is a number or a field name; this gives a {}", at_t.t.name());
                    self.add("X408", message, index.at(), None);
                }
                if of_t.t == SlateTypeName::List {
                    of_t.of.as_deref().cloned().unwrap_or_else(any_t)
                } else {
                    any_t()
                }
            }
            SlateExpr::List { items, .. } => {
                let types: Vec<SlateType> = items.iter().map(|e| self.ty(e, row)).collect();
                if types.is_empty() {
                    return list_t();
                }
                let mut it = types.into_iter();
                let first = it.next().unwrap_or_else(any_t);
                list_of(it.fold(first, merge))
            }
            SlateExpr::Rec { fields, .. } => {
                let mut map: IndexMap<String, SlateType> = IndexMap::new();
                for (name, e) in fields {
                    let t = self.ty(e, row);
                    map.insert(name.clone(), t);
                }
                let ordered: IndexMap<String, SlateType> =
                    js_key_order(map.iter()).into_iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                SlateType { t: SlateTypeName::Record, of: None, fields: Some(ordered) }
            }
            SlateExpr::Path { head, own, segs, at } => {
                if !*own && (head == "item" || head == "index") {
                    let Some(row) = row else {
                        self.add("X409", format!("{head} is only known inside a repeating piece or a step"), *at, None);
                        return any_t();
                    };
                    if head == "index" {
                        return if segs.is_empty() { num_t() } else { any_t() };
                    }
                    let mut t = row.clone();
                    for (i, seg) in segs.iter().enumerate() {
                        match field_type(&t, seg) {
                            None => {
                                let known = field_keys(&t);
                                let fix = slate_nearest(&seg_text(seg), &known);
                                let path: Vec<String> = segs[..=i].iter().map(seg_text).collect();
                                let tail = match &fix {
                                    Some(f) => format!("; did you mean item.{f}?"),
                                    None => format!("; they have {}", known.join(", ")),
                                };
                                let fix_out = fix.as_ref().map(|f| format!("item.{f}"));
                                self.add("X401", format!("item.{} is not a field of this list's rows{tail}", path.join(".")), *at, fix_out);
                                return any_t();
                            }
                            Some(next) => t = next,
                        }
                    }
                    return t;
                }
                match (self.scope.path)(head, *own, segs) {
                    Ok(t) => t,
                    Err(p) => {
                        self.add(p.code, p.message, *at, p.fix);
                        any_t()
                    }
                }
            }
            SlateExpr::Field { of, name, at } => {
                let of_t = self.ty(of, row);
                match field_type(&of_t, name) {
                    Some(t) => t,
                    None => {
                        let fix = slate_nearest(&seg_text(name), &field_keys(&of_t));
                        let tail = fix.as_ref().map_or(String::new(), |f| format!("; did you mean {f}?"));
                        self.add("X401", format!("{} is not a field here{tail}", seg_text(name)), *at, fix);
                        any_t()
                    }
                }
            }
            SlateExpr::Neg { arg, .. } => {
                self.ty(arg, row);
                num_t()
            }
            SlateExpr::Not { arg, .. } => {
                self.ty(arg, row);
                bool_t()
            }
            SlateExpr::Cond { test, then, other, .. } => {
                self.ty(test, row);
                let a = self.ty(then, row);
                let b = self.ty(other, row);
                merge(a, b)
            }
            SlateExpr::Bin { op, left, right, at } => {
                let l = self.ty(left, row);
                let r = self.ty(right, row);
                match op {
                    SlateBinOp::And | SlateBinOp::Or => {
                        if l.t == r.t {
                            l
                        } else {
                            any_t()
                        }
                    }
                    SlateBinOp::Eq | SlateBinOp::Ne => {
                        let mixed = (l.t == SlateTypeName::List && r.t == SlateTypeName::String)
                            || (l.t == SlateTypeName::String && r.t == SlateTypeName::List);
                        if mixed {
                            self.add(
                                "X408",
                                "a list never equals a string; use contains(), len() or a step".into(),
                                *at,
                                Some("contains(list, value), len(list) or list | where(item.field == value)".into()),
                            );
                        }
                        bool_t()
                    }
                    SlateBinOp::Lt | SlateBinOp::Le | SlateBinOp::Gt | SlateBinOp::Ge => bool_t(),
                    _ => {
                        if l.t == SlateTypeName::String || r.t == SlateTypeName::String {
                            self.add(
                                "X408",
                                format!("{} works on numbers; join text with concat() or a template", op.symbol()),
                                *at,
                                Some("concat(a, b) or `${a}${b}`".into()),
                            );
                        }
                        num_t()
                    }
                }
            }
            SlateExpr::Call { name, args, at } => {
                if HANDLER_STEPS.contains(&name.as_str()) && function_spec(name).is_none() {
                    for a in args {
                        self.ty(a, row);
                    }
                    self.add("X400", format!("{name}(...) is a handler step, which goes in onPress, onSubmit, onChange or do"), *at, None);
                    return any_t();
                }
                let arg_types: Vec<SlateType> = args.iter().map(|a| self.ty(a, row)).collect();
                let Some(spec) = function_spec(name) else {
                    let fix = match name.as_str() {
                        "length" => Some("len".to_string()),
                        "toUpperCase" => Some("upper".to_string()),
                        other => {
                            let names: Vec<String> = SLATE_FUNCTIONS.iter().map(|(n, _)| n.to_string()).collect();
                            slate_nearest(other, &names)
                        }
                    };
                    let tail = fix.as_ref().map_or(String::new(), |f| format!("; did you mean {f}?"));
                    self.add("X404", format!("{name} is not a function{tail}"), *at, fix);
                    return any_t();
                };
                if args.len() < spec.min || args.len() > spec.max {
                    let want = if spec.min == spec.max {
                        spec.min.to_string()
                    } else if spec.max >= 99 {
                        format!("at least {}", spec.min)
                    } else {
                        format!("{} or {}", spec.min, spec.max)
                    };
                    let plural = if want == "1" { "" } else { "s" };
                    let message = format!("{name} takes {want} argument{plural}, got {}: {}", args.len(), spec.sig);
                    self.add("X405", message, *at, Some(spec.sig.to_string()));
                }
                if let Some(first) = arg_types.first() {
                    let first_at = args[0].at();
                    if spec.number_first
                        && matches!(first.t, SlateTypeName::String | SlateTypeName::List | SlateTypeName::Record | SlateTypeName::Boolean)
                    {
                        let what = if first.t == SlateTypeName::String { "text".to_string() } else { format!("a {}", first.t.name()) };
                        let fix = (first.t == SlateTypeName::String).then(|| format!("{name}(num(...))"));
                        self.add("X406", format!("{name} takes a number; this gives {what}"), first_at, fix);
                    }
                    if spec.list_first && matches!(first.t, SlateTypeName::String | SlateTypeName::Number | SlateTypeName::Boolean) {
                        let what = if first.t == SlateTypeName::String { "text".to_string() } else { format!("a {}", first.t.name()) };
                        self.add("X406", format!("{name} takes a list; this gives {what}"), first_at, None);
                    }
                }
                (spec.returns)(&arg_types)
            }
            SlateExpr::Pipe { head, steps, .. } => {
                let mut cur = self.ty(head, row);
                for step in steps {
                    let Some(spec) = pipe_step(&step.name) else {
                        continue;
                    };
                    if spec.needs_list
                        && matches!(cur.t, SlateTypeName::Number | SlateTypeName::String | SlateTypeName::Boolean | SlateTypeName::Record)
                    {
                        let what = if cur.t == SlateTypeName::String { "text".to_string() } else { format!("a {}", cur.t.name()) };
                        self.add("Q424", format!("{} takes a list; this is {what}", step.name), step.at, None);
                    }
                    let elem = if cur.t == SlateTypeName::List { cur.of.as_deref().cloned().unwrap_or_else(any_t) } else { any_t() };
                    let mut arg_types: Vec<SlateType> = Vec::new();
                    for (i, a) in step.args.iter().enumerate() {
                        let join = step.name == "join";
                        let bare = step.name == "pick" || (join && i > 0) || step.name == "flatten";
                        if bare && field_name(&a.expr).is_some() {
                            arg_types.push(str_t());
                            continue;
                        }
                        if join && i > 0 {
                            self.add(
                                "Q425",
                                "join's keys are field names: join($other, id) or join($other, id, ownerId)".into(),
                                a.expr.at(),
                                None,
                            );
                        }
                        let scope_row = if join && i == 0 { row } else { Some(&elem) };
                        arg_types.push(self.ty(&a.expr, scope_row));
                    }
                    cur = (spec.out)(&cur, &arg_types);
                }
                cur
            }
        }
    }
}

const HANDLER_STEPS: [&str; 11] = ["set", "toggle", "start", "cancel", "send", "steer", "queue", "fill", "open", "copy", "pane"];

/// Problems with an expression at write time: unknown paths, functions, arity, argument and step input types, item
/// outside a row, string arithmetic, a list compared with a string. Positions are offsets into src plus base.
pub fn check_slate_expression(src: &str, scope: &SlateCheckScope, base: usize) -> SlateExprCheck {
    let parsed = parse_slate_expression(src);
    match parsed.ast {
        None => SlateExprCheck {
            ty: any_t(),
            problems: parsed
                .errors
                .into_iter()
                .map(|mut e| {
                    e.at = Some(e.at.unwrap_or(0) + base as i64);
                    e
                })
                .collect(),
        },
        Some(ast) => {
            let mut checker = Checker { scope, base, problems: Vec::new() };
            let ty = checker.ty(&ast, scope.row.as_ref());
            SlateExprCheck { ty, problems: checker.problems }
        }
    }
}

/// A value as a format hole and str() write it.
pub fn slate_text(v: &Value) -> String {
    text_of(Some(v))
}

/// Whether a value is missing, truthy or the rest, as the expression language reads it.
pub fn slate_truthy(v: Option<&Value>) -> bool {
    truthy(v)
}
