// SPDX-License-Identifier: AGPL-3.0-only
//! The evaluator of expr.ts.
use super::*;

// ---- evaluation ----

fn root_env(ctx: &SlateEvalContext) -> Env {
    Env {
        sh: Rc::new(Shared { resolve: ctx.resolve.clone(), now: ctx.now, steps: Cell::new(0), visits: Cell::new(0) }),
        item: ctx.item.clone(),
        index: ctx.index,
        row: false,
    }
}

fn in_row(env: &Env) -> bool {
    env.item.is_some() || env.index.is_some()
}

fn read_path(head: &str, own: bool, segs: &[PathSeg], env: &Env) -> Val {
    if !own && (head == "item" || head == "index") {
        if !in_row(env) {
            return None;
        }
        let mut v: Val = if head == "item" { env.item.clone() } else { Some(env.index.map_or(Value::Null, Value::from)) };
        for s in segs {
            v = slate_step(v, s);
        }
        return v;
    }
    let first_index = segs.iter().position(|s| matches!(s, PathSeg::Index(_))).unwrap_or(segs.len());
    for names in (0..=first_index).rev() {
        if let Some(found) = (env.sh.resolve)(&slate_path_text(head, own, &segs[..names])) {
            let mut out: Val = Some(found);
            for s in &segs[names..] {
                out = slate_step(out, s);
            }
            return out;
        }
    }
    if !own && head == "time" && matches!(segs, [PathSeg::Key(k)] if k == "now") {
        return Some(jnum(env.now()));
    }
    None
}

fn arith(op: SlateBinOp, a: &Val, b: &Val) -> Val {
    let (Some(x), Some(y)) = (as_num(a.as_ref()), as_num(b.as_ref())) else {
        return nul();
    };
    match op {
        SlateBinOp::Add => fin(x + y),
        SlateBinOp::Sub => fin(x - y),
        SlateBinOp::Mul => fin(x * y),
        SlateBinOp::Div if y == 0.0 => nul(),
        SlateBinOp::Div => fin(x / y),
        _ if y == 0.0 => nul(),
        _ => fin(x % y),
    }
}

fn order(op: SlateBinOp, a: &Val, b: &Val) -> Val {
    if missing(a.as_ref()) || missing(b.as_ref()) {
        return nul();
    }
    let ord = match (a, b) {
        (Some(Value::String(x)), Some(Value::String(y))) => utf16::cmp(x, y),
        _ => match (as_num(a.as_ref()), as_num(b.as_ref())) {
            (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
            _ => return nul(),
        },
    };
    Some(Value::Bool(match op {
        SlateBinOp::Lt => ord == Ordering::Less,
        SlateBinOp::Le => ord != Ordering::Greater,
        SlateBinOp::Gt => ord == Ordering::Greater,
        _ => ord != Ordering::Less,
    }))
}

fn run_pipe(head: &SlateExpr, steps: &[SlatePipeStepNode], env: &Env) -> Res<Val> {
    let mut cur = run(head, env)?;
    for step in steps {
        let Some(spec) = pipe_step(&step.name) else {
            continue;
        };
        if spec.needs_list && !is_list(cur.as_ref()) {
            return Ok(nul());
        }
        let list = capped(list_ref(cur.as_ref()).cloned().unwrap_or_default());
        let input = cur.take();
        cur = (spec.apply)(PipeRun { list, cur: input, args: &step.args, env })?;
    }
    Ok(cur)
}

pub(super) fn run(node: &SlateExpr, env: &Env) -> Res<Val> {
    env.step()?;
    match node {
        SlateExpr::Lit { v, .. } => Ok(Some(v.clone())),
        SlateExpr::Path { head, own, segs, .. } => Ok(read_path(head, *own, segs, env)),
        SlateExpr::Field { of, name, .. } => Ok(slate_step(run(of, env)?, name)),
        SlateExpr::Neg { arg, .. } => {
            let v = run(arg, env)?;
            Ok(as_num(v.as_ref()).map_or(nul(), |x| fin(-x)))
        }
        SlateExpr::Not { arg, .. } => Ok(Some(Value::Bool(!truthy(run(arg, env)?.as_ref())))),
        SlateExpr::Cond { test, then, other, .. } => {
            if truthy(run(test, env)?.as_ref()) {
                run(then, env)
            } else {
                run(other, env)
            }
        }
        SlateExpr::Tpl { parts, .. } => {
            let mut pieces: Vec<(bool, Val)> = Vec::with_capacity(parts.len());
            for p in parts {
                match p {
                    SlateTemplatePart::Text(s) => pieces.push((false, sval(s.clone()))),
                    SlateTemplatePart::Hole { expr, .. } => {
                        let v = run(expr, env)?;
                        pieces.push((!matches!(expr, SlateExpr::Lit { .. }), v));
                    }
                }
            }
            heldtext(Some(joined(&pieces).map_or(Value::Null, Value::String)))
        }
        SlateExpr::Pipe { head, steps, .. } => run_pipe(head, steps, env),
        SlateExpr::List { items, .. } => {
            let mut out = Vec::with_capacity(items.len());
            for e in items {
                out.push(run(e, env)?.unwrap_or(Value::Null));
            }
            Ok(Some(Value::Array(out)))
        }
        SlateExpr::Index { of, index, .. } => {
            let of_v = run(of, env)?;
            let at_v = run(index, env)?;
            Ok(pick_at(of_v.as_ref(), at_v.as_ref()))
        }
        SlateExpr::Rec { fields, .. } => {
            let mut entries = Vec::with_capacity(fields.len());
            for (name, e) in fields {
                entries.push((name.clone(), run(e, env)?.unwrap_or(Value::Null)));
            }
            Ok(Some(js_object(entries)))
        }
        SlateExpr::Bin { op, left, right, .. } => match op {
            SlateBinOp::And => {
                let l = run(left, env)?;
                if truthy(l.as_ref()) {
                    run(right, env)
                } else {
                    Ok(l)
                }
            }
            SlateBinOp::Or => {
                let l = run(left, env)?;
                if truthy(l.as_ref()) {
                    Ok(l)
                } else {
                    run(right, env)
                }
            }
            _ => {
                let l = run(left, env)?;
                let r = run(right, env)?;
                Ok(match op {
                    SlateBinOp::Eq => {
                        Some(Value::Bool(slate_equal(Some(l.as_ref().unwrap_or(&Value::Null)), Some(r.as_ref().unwrap_or(&Value::Null)))))
                    }
                    SlateBinOp::Ne => {
                        Some(Value::Bool(!slate_equal(Some(l.as_ref().unwrap_or(&Value::Null)), Some(r.as_ref().unwrap_or(&Value::Null)))))
                    }
                    SlateBinOp::Lt | SlateBinOp::Le | SlateBinOp::Gt | SlateBinOp::Ge => order(*op, &l, &r),
                    _ => arith(*op, &l, &r),
                })
            }
        },
        SlateExpr::Call { name, args, .. } => {
            let Some(spec) = function_spec(name) else {
                return Ok(nul());
            };
            if args.len() < spec.min || args.len() > spec.max {
                return Ok(nul());
            }
            let mut values: Vec<Val> = Vec::with_capacity(args.len());
            for a in args {
                values.push(run(a, env)?);
            }
            if !spec.null_safe && missing(values.first().and_then(Option::as_ref)) {
                return Ok(nul());
            }
            let out = (spec.body)(&values, env, args)?;
            heldtext(out)
        }
    }
}

/// The value of an expression; null where it fails, undefined where a bare path's data has not arrived. Never throws.
pub fn evaluate_slate_expression(expr: &str, ctx: &SlateEvalContext) -> Option<Value> {
    match parse_slate_expression(expr).ast {
        Some(ast) => evaluate_slate_ast(&ast, ctx),
        None => nul(),
    }
}

/// evaluate_slate_expression for a tree already parsed.
pub fn evaluate_slate_ast(ast: &SlateExpr, ctx: &SlateEvalContext) -> Option<Value> {
    match run(ast, &root_env(ctx)) {
        Ok(v) => v,
        Err(Over) => nul(),
    }
}

fn evaluate_slate_format(src: &str, ctx: &SlateEvalContext) -> Val {
    let pieces: Vec<(bool, Val)> = parse_slate_format(src)
        .parts
        .into_iter()
        .map(|p| match p {
            SlateFormatPart::Text(t) => (false, sval(t)),
            SlateFormatPart::Hole { expr, .. } => (true, evaluate_slate_expression(&expr, ctx)),
        })
        .collect();
    Some(joined(&pieces).map_or(Value::Null, Value::String))
}

pub(super) fn is_slate_format(v: &Value) -> bool {
    v.as_object().is_some_and(|m| m.len() == 1 && m.get("format").is_some_and(Value::is_string))
}

/// A prop's value: a literal as written, a binding evaluated, a format filled, lists and records through.
pub fn resolve_slate_prop(v: &Value, ctx: &SlateEvalContext) -> Option<Value> {
    match v {
        Value::Object(fields) => {
            if is_slate_binding(v) {
                let bind = fields.get("bind").and_then(Value::as_str).unwrap_or_default();
                return evaluate_slate_expression(bind, ctx);
            }
            if is_slate_format(v) {
                let format = fields.get("format").and_then(Value::as_str).unwrap_or_default();
                return evaluate_slate_format(format, ctx);
            }
            Some(js_object(fields.iter().map(|(k, x)| (k.clone(), resolve_slate_prop(x, ctx).unwrap_or(Value::Null)))))
        }
        Value::Array(items) => Some(Value::Array(items.iter().map(|x| resolve_slate_prop(x, ctx).unwrap_or(Value::Null)).collect())),
        other => Some(other.clone()),
    }
}
