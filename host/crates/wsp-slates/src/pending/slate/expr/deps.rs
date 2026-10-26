// SPDX-License-Identifier: AGPL-3.0-only
//! The dependency sets of expr.ts.
use super::*;

// ---- dependencies ----

fn walk(node: &SlateExpr, out: &mut IndexSet<String>) {
    match node {
        SlateExpr::Path { head, own, segs, .. } => {
            if *own || (head != "item" && head != "index") {
                out.insert(slate_path_text(head, *own, segs));
            }
        }
        SlateExpr::Call { name, args, .. } => {
            if function_spec(name).is_some_and(|s| s.clock) {
                out.insert("time.now".to_string());
            }
            for a in args {
                walk(a, out);
            }
        }
        SlateExpr::Lit { .. } => {}
        SlateExpr::Field { of, .. } => walk(of, out),
        SlateExpr::Neg { arg, .. } | SlateExpr::Not { arg, .. } => walk(arg, out),
        SlateExpr::Bin { left, right, .. } => {
            walk(left, out);
            walk(right, out);
        }
        SlateExpr::Cond { test, then, other, .. } => {
            walk(test, out);
            walk(then, out);
            walk(other, out);
        }
        SlateExpr::Tpl { parts, .. } => {
            for p in parts {
                if let SlateTemplatePart::Hole { expr, .. } = p {
                    walk(expr, out);
                }
            }
        }
        SlateExpr::List { items, .. } => {
            for e in items {
                walk(e, out);
            }
        }
        SlateExpr::Index { of, index, .. } => {
            walk(of, out);
            walk(index, out);
        }
        SlateExpr::Rec { fields, .. } => {
            for (_, e) in fields {
                walk(e, out);
            }
        }
        SlateExpr::Pipe { head, steps, .. } => {
            walk(head, out);
            for s in steps {
                for (i, a) in s.args.iter().enumerate() {
                    let bare = s.name == "pick" || (s.name == "join" && i > 0) || s.name == "flatten";
                    if bare && field_name(&a.expr).is_some() {
                        continue;
                    }
                    walk(&a.expr, out);
                }
            }
        }
    }
}

/// Every own and source path a parsed expression reads, in first-read order, without item and index; a call to a
/// function that reads the clock adds time.now.
pub fn slate_ast_dependencies(ast: &SlateExpr) -> Vec<String> {
    let mut out = IndexSet::new();
    walk(ast, &mut out);
    out.into_iter().collect()
}

/// Every own and source path an expression reads (see slate_ast_dependencies); empty when it does not parse.
pub fn slate_dependencies(expr: &str) -> Vec<String> {
    parse_slate_expression(expr).ast.map_or_else(Vec::new, |ast| slate_ast_dependencies(&ast))
}

fn visit_prop(v: &Value, out: &mut IndexSet<String>) {
    match v {
        Value::Object(fields) => {
            if is_slate_binding(v) {
                if let Some(bind) = fields.get("bind").and_then(Value::as_str) {
                    out.extend(slate_dependencies(bind));
                }
            } else if is_slate_format(v) {
                let format = fields.get("format").and_then(Value::as_str).unwrap_or_default();
                for part in parse_slate_format(format).parts {
                    if let SlateFormatPart::Hole { expr, .. } = part {
                        out.extend(slate_dependencies(&expr));
                    }
                }
            } else {
                for x in fields.values() {
                    visit_prop(x, out);
                }
            }
        }
        Value::Array(items) => {
            for x in items {
                visit_prop(x, out);
            }
        }
        _ => {}
    }
}

/// Every path a prop value reads, through its bindings, format holes and nested literals.
pub fn slate_prop_dependencies(value: Option<&Value>) -> Vec<String> {
    let mut out = IndexSet::new();
    if let Some(v) = value {
        visit_prop(v, &mut out);
    }
    out.into_iter().collect()
}
