// SPDX-License-Identifier: AGPL-3.0-only
//! The pipeline steps of expr.ts.
use super::*;

// ---- pipeline steps ----

pub(super) fn pipe_step(name: &str) -> Option<&'static SlatePipeStep> {
    SLATE_PIPE_STEPS.iter().find(|(n, _)| *n == name).map(|(_, spec)| spec)
}

fn same_t(input: &SlateType, _: &[SlateType]) -> SlateType {
    input.clone()
}

fn num_out(_: &SlateType, _: &[SlateType]) -> SlateType {
    num_t()
}

fn list_rec_out(_: &SlateType, _: &[SlateType]) -> SlateType {
    list_of(rec_t())
}

fn list_out(_: &SlateType, _: &[SlateType]) -> SlateType {
    list_t()
}

fn elem_out(input: &SlateType, _: &[SlateType]) -> SlateType {
    input.of.as_deref().cloned().unwrap_or_else(any_t)
}

/// The keys of a step's group: how they compare, for sortBy. Numbers, then text, then everything else, nulls last.
fn rank(v: &Val) -> u8 {
    if as_num(v.as_ref()).is_some() {
        0
    } else if matches!(v, Some(Value::String(_))) {
        1
    } else if missing(v.as_ref()) {
        3
    } else {
        2
    }
}

fn compare_keys(a: &Val, b: &Val, desc: bool) -> Ordering {
    let (ra, rb) = (rank(a), rank(b));
    if ra == 3 || rb == 3 {
        return ra.cmp(&rb);
    }
    let base = if ra != rb {
        ra.cmp(&rb)
    } else if ra == 0 {
        as_num(a.as_ref()).unwrap_or(0.0).partial_cmp(&as_num(b.as_ref()).unwrap_or(0.0)).unwrap_or(Ordering::Equal)
    } else if ra == 1 {
        let (Some(Value::String(x)), Some(Value::String(y))) = (a, b) else {
            return Ordering::Equal;
        };
        collate(x, y)
    } else {
        Ordering::Equal
    };
    if desc {
        base.reverse()
    } else {
        base
    }
}

/// JSON.stringify of a key: the identity of a value for grouping and distinct.
fn key_text(v: &Val) -> String {
    json_stringify(v.as_ref().unwrap_or(&Value::Null))
}

pub(super) fn field_name(e: &SlateExpr) -> Option<String> {
    match e {
        SlateExpr::Path { head, own: false, segs, .. } if segs.is_empty() => Some(head.clone()),
        SlateExpr::Path { own: false, head, segs, .. } if head == "item" && segs.len() == 1 => match &segs[0] {
            PathSeg::Key(k) => Some(k.clone()),
            PathSeg::Index(_) => None,
        },
        SlateExpr::Lit { v: Value::String(s), .. } => Some(s.clone()),
        _ => None,
    }
}

fn pipe_sort(x: PipeRun) -> Res<Val> {
    let desc = x.args.get(1).is_some() && matches!(run(x.arg(1), x.env)?, Some(Value::String(ref s)) if s == "desc");
    let mut keyed: Vec<(Val, usize, Value)> = Vec::with_capacity(x.list.len());
    for (i, item) in x.list.iter().enumerate() {
        keyed.push((x.per(x.arg(0), item, i)?, i, item.clone()));
    }
    keyed.sort_by(|a, b| compare_keys(&a.0, &b.0, desc).then(a.1.cmp(&b.1)));
    Ok(Some(Value::Array(keyed.into_iter().map(|k| k.2).collect())))
}

fn pipe_group(x: PipeRun) -> Res<Val> {
    let mut groups: IndexMap<String, (Value, Vec<Value>)> = IndexMap::new();
    for (i, item) in x.list.iter().enumerate() {
        let key = x.per(x.arg(0), item, i)?.unwrap_or(Value::Null);
        let k = key_text(&Some(key.clone()));
        groups.entry(k).or_insert_with(|| (key, Vec::new())).1.push(item.clone());
    }
    let out = groups
        .into_values()
        .map(|(key, items)| {
            let count = items.len() as u64;
            js_object([("key".to_string(), key), ("items".to_string(), Value::Array(items)), ("count".to_string(), Value::from(count))])
        })
        .collect();
    Ok(Some(Value::Array(out)))
}

fn pipe_take(x: PipeRun) -> Res<Val> {
    let n = run(x.arg(0), x.env)?;
    x.env.charge(x.list.len())?;
    Ok(match as_num(n.as_ref()) {
        Some(n) => {
            let k = n.max(0.0).trunc() as usize;
            Some(Value::Array(x.list.iter().take(k).cloned().collect()))
        }
        None => nul(),
    })
}

fn pipe_skip(x: PipeRun) -> Res<Val> {
    let n = run(x.arg(0), x.env)?;
    x.env.charge(x.list.len())?;
    Ok(match as_num(n.as_ref()) {
        Some(n) => {
            let k = n.max(0.0).trunc() as usize;
            Some(Value::Array(x.list.iter().skip(k).cloned().collect()))
        }
        None => nul(),
    })
}

fn numbers_per(x: &PipeRun) -> Res<Vec<f64>> {
    let mut out = Vec::new();
    for (i, item) in x.list.iter().enumerate() {
        if let Some(v) = as_num(x.per(x.arg(0), item, i)?.as_ref()) {
            out.push(v);
        }
    }
    Ok(out)
}

fn pipe_pick(x: PipeRun) -> Res<Val> {
    let names: Vec<String> = x.args.iter().filter_map(|a| field_name(&a.expr)).collect();
    let mut out = Vec::with_capacity(x.list.len());
    for item in &x.list {
        x.env.charge(1)?;
        out.push(match item {
            Value::Object(fields) => js_object(
                names
                    .iter()
                    .filter(|n| fields.contains_key(n.as_str()))
                    .map(|n| (n.clone(), fields.get(n.as_str()).cloned().unwrap_or(Value::Null))),
            ),
            _ => Value::Null,
        });
    }
    Ok(Some(Value::Array(out)))
}

fn pipe_map(x: PipeRun) -> Res<Val> {
    let mut out = Vec::with_capacity(x.list.len());
    for (i, item) in x.list.iter().enumerate() {
        let mut base: Map<String, Value> = match item {
            Value::Object(fields) => fields.clone(),
            other => {
                let mut m = Map::new();
                m.insert("value".to_string(), other.clone());
                m
            }
        };
        for a in x.args {
            let name = a.name.clone().unwrap_or_default();
            let v = x.per(&a.expr, item, i)?.unwrap_or(Value::Null);
            if name == "__proto__" && !base.contains_key(&name) {
                continue;
            }
            base.insert(name, v);
        }
        out.push(js_object(base));
    }
    Ok(Some(Value::Array(out)))
}

fn pipe_distinct(x: PipeRun) -> Res<Val> {
    let mut seen: IndexSet<String> = IndexSet::new();
    let mut out = Vec::new();
    for (i, item) in x.list.iter().enumerate() {
        let key = if x.args.is_empty() { Some(item.clone()) } else { x.per(x.arg(0), item, i)? };
        if seen.insert(key_text(&key)) {
            out.push(item.clone());
        }
    }
    Ok(Some(Value::Array(out)))
}

fn pipe_flatten(x: PipeRun) -> Res<Val> {
    let name = x.args.first().and_then(|a| field_name(&a.expr));
    let mut out: Vec<Value> = Vec::new();
    for item in &x.list {
        let inner = match &name {
            Some(n) => slate_step(Some(item.clone()), &PathSeg::Key(n.clone())),
            None => Some(item.clone()),
        };
        if let Some(Value::Array(items)) = inner {
            x.env.charge(items.len())?;
            out.extend(items);
        }
    }
    Ok(Some(Value::Array(capped(out))))
}

fn pipe_join(x: PipeRun) -> Res<Val> {
    let other_expr = x.arg(0);
    let other = run(other_expr, x.env)?;
    let key = field_name(x.arg(1));
    let other_key = if x.args.len() > 2 { field_name(x.arg(2)) } else { key.clone() };
    let under = match other_expr {
        SlateExpr::Path { head, segs, .. } => segs
            .iter()
            .rev()
            .find_map(|s| match s {
                PathSeg::Key(k) => Some(k.clone()),
                PathSeg::Index(_) => None,
            })
            .unwrap_or_else(|| head.clone()),
        _ => "other".to_string(),
    };
    let pool: Vec<Value> = list_ref(other.as_ref()).cloned().unwrap_or_default();
    x.env.charge(pool.len())?;
    let mut by_key: IndexMap<String, Value> = IndexMap::new();
    if let Some(ok) = &other_key {
        for o in &pool {
            let k = key_text(&Some(slate_step(Some(o.clone()), &PathSeg::Key(ok.clone())).unwrap_or(Value::Null)));
            by_key.entry(k).or_insert_with(|| o.clone());
        }
    }
    let mut out = Vec::with_capacity(x.list.len());
    for item in &x.list {
        x.env.charge(1)?;
        let mine = key.as_ref().and_then(|k| slate_step(Some(item.clone()), &PathSeg::Key(k.clone())));
        let hit = by_key.get(&key_text(&Some(mine.unwrap_or(Value::Null)))).cloned().unwrap_or(Value::Null);
        out.push(match item {
            Value::Object(fields) => {
                let mut entries: Vec<(String, Value)> = fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                entries.push((under.clone(), hit));
                js_object(entries)
            }
            other => js_object([("value".to_string(), other.clone()), (under.clone(), hit)]),
        });
    }
    Ok(Some(Value::Array(out)))
}

fn pipe_format(x: PipeRun) -> Res<Val> {
    if let Some(Value::Array(_)) = x.cur {
        let mut out = Vec::with_capacity(x.list.len());
        for (i, item) in x.list.iter().enumerate() {
            out.push(Value::String(text_of(x.per(x.arg(0), item, i)?.as_ref())));
        }
        return Ok(Some(Value::Array(out)));
    }
    let cur = x.cur.clone().unwrap_or(Value::Null);
    let v = x.per(x.arg(0), &cur, 0)?;
    Ok(sval(text_of(v.as_ref())))
}

/// Every pipeline step with its signature, purpose and an example.
pub static SLATE_PIPE_STEPS: &[(&str, SlatePipeStep)] = &[
    (
        "where",
        SlatePipeStep {
            min: 1,
            max: 1,
            sig: "where(cond)",
            purpose: "keeps the elements where cond is true",
            example: "pr.checks | where(item.state == 'fail')",
            needs_list: true,
            out: same_t,
            apply: |x| {
                let mut out = Vec::new();
                for (i, item) in x.list.iter().enumerate() {
                    if truthy(x.per(x.arg(0), item, i)?.as_ref()) {
                        out.push(item.clone());
                    }
                }
                Ok(Some(Value::Array(out)))
            },
        },
    ),
    (
        "sortBy",
        SlatePipeStep {
            min: 1,
            max: 2,
            sig: "sortBy(expr, 'asc'|'desc')",
            purpose: "stable sort; nulls last",
            example: "processes.list | sortBy(item.cpu, 'desc')",
            needs_list: true,
            out: same_t,
            apply: pipe_sort,
        },
    ),
    (
        "groupBy",
        SlatePipeStep {
            min: 1,
            max: 1,
            sig: "groupBy(expr)",
            purpose: "{ key, items, count } per group in first-seen order",
            example: "pr.checks | groupBy(item.state)",
            needs_list: true,
            out: |i, _| {
                list_of(SlateType {
                    t: SlateTypeName::Record,
                    of: None,
                    fields: Some(IndexMap::from([
                        ("key".to_string(), any_t()),
                        ("items".to_string(), i.clone()),
                        ("count".to_string(), num_t()),
                    ])),
                })
            },
            apply: pipe_group,
        },
    ),
    (
        "take",
        SlatePipeStep {
            min: 1,
            max: 1,
            sig: "take(n)",
            purpose: "the first n; n is a literal",
            example: "pr.checks | take(5)",
            needs_list: true,
            out: same_t,
            apply: pipe_take,
        },
    ),
    (
        "skip",
        SlatePipeStep {
            min: 1,
            max: 1,
            sig: "skip(n)",
            purpose: "all but the first n",
            example: "pr.checks | skip(1)",
            needs_list: true,
            out: same_t,
            apply: pipe_skip,
        },
    ),
    (
        "count",
        SlatePipeStep {
            min: 0,
            max: 0,
            sig: "count",
            purpose: "the length",
            example: "$steps | where(item.done) | count",
            needs_list: true,
            out: num_out,
            apply: |x| Ok(Some(Value::from(x.list.len() as u64))),
        },
    ),
    (
        "sum",
        SlatePipeStep {
            min: 1,
            max: 1,
            sig: "sum(expr)",
            purpose: "the total of expr; nulls skipped",
            example: "thread.changes.files | sum(item.additions)",
            needs_list: true,
            out: num_out,
            apply: |x| Ok(fin(numbers_per(&x)?.iter().fold(0.0, |acc, v| acc + v))),
        },
    ),
    (
        "min",
        SlatePipeStep {
            min: 1,
            max: 1,
            sig: "min(expr)",
            purpose: "the least expr",
            example: "pr.checks | min(num(item.startedAt))",
            needs_list: true,
            out: num_out,
            apply: |x| Ok(numbers_per(&x)?.iter().copied().reduce(f64::min).map_or(nul(), fin)),
        },
    ),
    (
        "max",
        SlatePipeStep {
            min: 1,
            max: 1,
            sig: "max(expr)",
            purpose: "the greatest expr",
            example: "processes.list | max(item.cpu)",
            needs_list: true,
            out: num_out,
            apply: |x| Ok(numbers_per(&x)?.iter().copied().reduce(f64::max).map_or(nul(), fin)),
        },
    ),
    (
        "avg",
        SlatePipeStep {
            min: 1,
            max: 1,
            sig: "avg(expr)",
            purpose: "the mean of expr",
            example: "processes.list | avg(item.mem)",
            needs_list: true,
            out: num_out,
            apply: |x| {
                let ns = numbers_per(&x)?;
                if ns.is_empty() {
                    return Ok(nul());
                }
                Ok(fin(ns.iter().fold(0.0, |acc, v| acc + v) / ns.len() as f64))
            },
        },
    ),
    (
        "first",
        SlatePipeStep {
            min: 0,
            max: 0,
            sig: "first",
            purpose: "the first element or null",
            example: "thread.plan.steps | where(item.state == 'working') | first",
            needs_list: true,
            out: elem_out,
            apply: |x| Ok(Some(x.list.first().cloned().unwrap_or(Value::Null))),
        },
    ),
    (
        "last",
        SlatePipeStep {
            min: 0,
            max: 0,
            sig: "last",
            purpose: "the last element or null",
            example: "thread.plan.steps | last",
            needs_list: true,
            out: elem_out,
            apply: |x| Ok(Some(x.list.last().cloned().unwrap_or(Value::Null))),
        },
    ),
    (
        "pick",
        SlatePipeStep {
            min: 1,
            max: 99,
            sig: "pick(field, ...)",
            purpose: "keeps the named fields",
            example: "processes.list | pick(name, cpu)",
            needs_list: true,
            out: list_rec_out,
            apply: pipe_pick,
        },
    ),
    (
        "map",
        SlatePipeStep {
            min: 1,
            max: 99,
            sig: "map(name: expr, ...)",
            purpose: "adds or replaces fields",
            example: "pr.checks | map(took: num(item.completedAt) - num(item.startedAt))",
            needs_list: true,
            out: list_rec_out,
            apply: pipe_map,
        },
    ),
    (
        "distinct",
        SlatePipeStep {
            min: 0,
            max: 1,
            sig: "distinct(expr?)",
            purpose: "drops later repeats",
            example: "pr.checks | distinct(item.workflow)",
            needs_list: true,
            out: same_t,
            apply: pipe_distinct,
        },
    ),
    (
        "flatten",
        SlatePipeStep {
            min: 0,
            max: 1,
            sig: "flatten(field?)",
            purpose: "joins inner lists",
            example: "tree.all | flatten(children)",
            needs_list: true,
            out: list_out,
            apply: pipe_flatten,
        },
    ),
    (
        "join",
        SlatePipeStep {
            min: 2,
            max: 3,
            sig: "join(other, key, otherKey?)",
            purpose: "attaches the matching element of other",
            example: "thread.subagents | join(tree.children, id)",
            needs_list: true,
            out: same_t,
            apply: pipe_join,
        },
    ),
    (
        "format",
        SlatePipeStep {
            min: 1,
            max: 1,
            sig: "format(expr)",
            purpose: "expr as text per element",
            example: "pr.checks | format(`${item.name}: ${word(item.state)}`)",
            needs_list: false,
            out: |i, _| if i.t == SlateTypeName::List { list_of(str_t()) } else { str_t() },
            apply: pipe_format,
        },
    ),
];
