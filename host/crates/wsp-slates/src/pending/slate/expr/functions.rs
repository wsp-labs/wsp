// SPDX-License-Identifier: AGPL-3.0-only
//! The function table of expr.ts and its helpers.
use super::*;

// ---- functions ----

fn arg(a: &[Val], i: usize) -> Option<&Value> {
    a.get(i).and_then(Option::as_ref)
}

fn undef(a: &[Val], i: usize) -> bool {
    a.get(i).is_none_or(Option::is_none)
}

fn ret_str(_: &[SlateType]) -> SlateType {
    str_t()
}

fn ret_num(_: &[SlateType]) -> SlateType {
    num_t()
}

fn ret_bool(_: &[SlateType]) -> SlateType {
    bool_t()
}

fn ret_any(_: &[SlateType]) -> SlateType {
    any_t()
}

fn ret_list(_: &[SlateType]) -> SlateType {
    list_t()
}

fn elem_of(a: &[SlateType]) -> SlateType {
    a.first().and_then(|t| t.of.as_deref().cloned()).unwrap_or_else(any_t)
}

pub(super) fn merge(a: SlateType, b: SlateType) -> SlateType {
    if a.t == b.t {
        return a;
    }
    if a.t == SlateTypeName::Null {
        return b;
    }
    if b.t == SlateTypeName::Null {
        return a;
    }
    any_t()
}

/// list[i] or record[key] for a computed index: a whole number counts from the end when negative.
pub(super) fn pick_at(of: Option<&Value>, at: Option<&Value>) -> Val {
    if let (Some(items), Some(i)) = (list_ref(of), as_num(at)) {
        if i.fract() != 0.0 {
            return nul();
        }
        let idx = if i < 0.0 { items.len() as f64 + i } else { i };
        if idx >= 0.0 && (idx as usize) < items.len() {
            return Some(items[idx as usize].clone());
        }
        return nul();
    }
    if let (Some(Value::Object(fields)), Some(Value::String(key))) = (of, at) {
        return Some(fields.get(key).cloned().unwrap_or(Value::Null));
    }
    nul()
}

/// A list's element at a path step, read through a name when one is given (JS `item[name]`, else the item).
fn field_of(item: &Value, name: Option<&Value>) -> Val {
    match name {
        Some(Value::String(key)) => slate_step(Some(item.clone()), &PathSeg::Key(key.clone())),
        _ => Some(item.clone()),
    }
}

fn numbers_of(env: &Env, list: Option<&Value>, name: Option<&Value>) -> Res<Vec<f64>> {
    let Some(items) = list_ref(list) else {
        return Ok(vec![]);
    };
    let mut out = Vec::new();
    for item in items {
        env.charge(1)?;
        let v = if name.is_none() { Some(item.clone()) } else { field_of(item, name) };
        if let Some(x) = as_num(v.as_ref()) {
            out.push(x);
        }
    }
    Ok(out)
}

fn list_or_args(a: &[Val]) -> Vec<Val> {
    if let Some(Value::Array(items)) = arg(a, 0).filter(|_| a.len() == 1) {
        return items.iter().map(|v| Some(v.clone())).collect();
    }
    a.iter().map(|v| Some(v.clone().unwrap_or(Value::Null))).collect()
}

pub(super) fn heldtext(v: Val) -> Res<Val> {
    if let Some(Value::String(s)) = &v {
        if utf16::len(s) > SLATE_LIMITS.values_bytes {
            return Err(Over);
        }
    }
    Ok(v)
}

const fn func(
    min: usize,
    max: usize,
    sig: &'static str,
    example: &'static str,
    returns: fn(&[SlateType]) -> SlateType,
    body: FnBody,
) -> SlateFunction {
    SlateFunction { min, max, sig, example, returns, null_safe: false, number_first: false, list_first: false, clock: false, body }
}

/// Every function with its signature and an example, for the catalog and the checker.
pub static SLATE_FUNCTIONS: &[(&str, SlateFunction)] = &[
    (
        "percent",
        SlateFunction {
            number_first: true,
            ..func(1, 2, "percent(fraction, places?)", "percent(thread.context.used / thread.context.window)", ret_str, |a, _, _| {
                let Some(v) = as_num(arg(a, 0)).map(|x| x * 100.0).filter(|v| v.is_finite()) else {
                    return Ok(nul());
                };
                let fallback = if v.abs() < 10.0 && v != 0.0 { 1 } else { 0 };
                Ok(sval(format!("{}%", to_fixed(v, places(arg(a, 1), fallback)))))
            })
        },
    ),
    (
        "pct",
        SlateFunction {
            number_first: true,
            ..func(1, 2, "pct(points, places?)", "pct(usage.week.percent)", ret_str, |a, _, _| {
                Ok(as_num(arg(a, 0)).map_or(nul(), |x| sval(format!("{}%", to_fixed(x, places(arg(a, 1), 0))))))
            })
        },
    ),
    (
        "tokens",
        SlateFunction {
            number_first: true,
            ..func(1, 1, "tokens(n)", "tokens(thread.context.free)", ret_str, |a, _, _| {
                Ok(as_num(arg(a, 0)).map_or(nul(), |x| sval(fmt_tokens(x))))
            })
        },
    ),
    (
        "bytes",
        SlateFunction {
            number_first: true,
            ..func(1, 1, "bytes(n)", "bytes(machine.mem.used)", ret_str, |a, _, _| {
                Ok(as_num(arg(a, 0)).map_or(nul(), |x| sval(fmt_bytes(x))))
            })
        },
    ),
    (
        "usd",
        SlateFunction {
            number_first: true,
            ..func(1, 2, "usd(n, places?)", "usd(thread.cost.usd)", ret_str, |a, _, _| {
                let Some(x) = as_num(arg(a, 0)) else {
                    return Ok(nul());
                };
                Ok(sval(if undef(a, 1) { fmt_cost(x) } else { format!("${}", plain_number(x, Some(places(arg(a, 1), 2)))) }))
            })
        },
    ),
    (
        "number",
        SlateFunction {
            number_first: true,
            ..func(1, 2, "number(x, places?)", "number(machine.load1, 2)", ret_str, |a, _, _| {
                let Some(x) = as_num(arg(a, 0)) else {
                    return Ok(nul());
                };
                Ok(sval(plain_number(x, if undef(a, 1) { None } else { Some(places(arg(a, 1), 0)) })))
            })
        },
    ),
    (
        "duration",
        SlateFunction {
            number_first: true,
            ..func(1, 2, "duration(ms, style?)", "duration(thread.lastTurn.durationMs)", ret_str, |a, _, _| {
                let Some(x) = as_num(arg(a, 0)) else {
                    return Ok(nul());
                };
                let style =
                    if matches!(arg(a, 1), Some(Value::String(s)) if s == "clock") { DurationStyle::Clock } else { DurationStyle::Short };
                Ok(sval(fmt_duration(x, Some(style))))
            })
        },
    ),
    (
        "ago",
        SlateFunction {
            clock: true,
            ..func(1, 1, "ago(t)", "ago(pr.readAt)", ret_str, |a, env, _| {
                let Some(at) = ms(arg(a, 0)) else {
                    return Ok(nul());
                };
                Ok(sval(format!("{} ago", span(env.now() - at, false))))
            })
        },
    ),
    (
        "until",
        SlateFunction {
            clock: true,
            ..func(1, 1, "until(t)", "until(usage.week.resetsAt)", ret_str, |a, env, _| {
                let Some(at) = ms(arg(a, 0)) else {
                    return Ok(nul());
                };
                let now = env.now();
                Ok(sval(if at <= now { "now".to_string() } else { format!("in {}", span(at - now, true)) }))
            })
        },
    ),
    (
        "date",
        SlateFunction {
            ..func(1, 2, "date(t, style?)", "date(usage.week.resetsAt)", ret_str, |a, _, _| {
                let Some(at) = ms(arg(a, 0)) else {
                    return Ok(nul());
                };
                Ok(sval(en_gb_date(at, matches!(arg(a, 1), Some(Value::String(s)) if s == "long"))))
            })
        },
    ),
    (
        "time",
        SlateFunction {
            ..func(1, 1, "time(t)", "time(usage.session.resetsAt)", ret_str, |a, _, _| {
                let Some(at) = ms(arg(a, 0)) else {
                    return Ok(nul());
                };
                Ok(sval(utf16::slice(&clock_text(at), 0, Some(5))))
            })
        },
    ),
    (
        "weekday",
        SlateFunction {
            ..func(1, 1, "weekday(t)", "weekday(usage.week.resetsAt)", ret_str, |a, _, _| {
                let Some(at) = ms(arg(a, 0)) else {
                    return Ok(nul());
                };
                Ok(sval(match local_fields(at) {
                    Some(f) => WEEKDAYS[f.weekday as usize].to_string(),
                    None => "Invalid Date".to_string(),
                }))
            })
        },
    ),
    (
        "plural",
        SlateFunction {
            number_first: true,
            ..func(2, 3, "plural(n, one, many?)", "plural(len(pr.checks), 'check')", ret_str, |a, _, _| {
                let (Some(n), Some(Value::String(one))) = (as_num(arg(a, 0)), arg(a, 1).filter(|v| v.is_string())) else {
                    return Ok(nul());
                };
                let noun = if n == 1.0 {
                    one.clone()
                } else if let Some(Value::String(many)) = arg(a, 2) {
                    many.clone()
                } else {
                    format!("{one}s")
                };
                Ok(sval(format!("{} {noun}", plain_number(n, None))))
            })
        },
    ),
    ("word", SlateFunction { ..func(1, 1, "word(x)", "word(item.state)", ret_str, |a, _, _| Ok(slate_word(arg(a, 0)))) }),
    (
        "upper",
        SlateFunction {
            ..func(1, 1, "upper(s)", "upper(git.branch)", ret_str, |a, _, _| {
                Ok(match arg(a, 0) {
                    Some(Value::String(s)) => sval(s.to_uppercase()),
                    _ => nul(),
                })
            })
        },
    ),
    (
        "lower",
        SlateFunction {
            ..func(1, 1, "lower(s)", "lower(pr.word)", ret_str, |a, _, _| {
                Ok(match arg(a, 0) {
                    Some(Value::String(s)) => sval(s.to_lowercase()),
                    _ => nul(),
                })
            })
        },
    ),
    (
        "trim",
        SlateFunction {
            ..func(1, 1, "trim(s)", "trim($note)", ret_str, |a, _, _| {
                Ok(match arg(a, 0) {
                    Some(Value::String(s)) => sval(js_trim(s).to_string()),
                    _ => nul(),
                })
            })
        },
    ),
    (
        "short",
        SlateFunction {
            ..func(2, 2, "short(s, n)", "short(pr.headSubject, 40)", ret_str, |a, _, _| {
                let (Some(Value::String(s)), Some(n)) = (arg(a, 0), as_num(arg(a, 1))) else {
                    return Ok(nul());
                };
                if (utf16::len(s) as f64) > n {
                    let end = (n - 1.0).max(0.0).trunc() as i64;
                    return Ok(sval(format!("{}…", utf16::slice(s, 0, Some(end)))));
                }
                Ok(sval(s.clone()))
            })
        },
    ),
    (
        "round",
        SlateFunction {
            number_first: true,
            ..func(1, 2, "round(x, places?)", "round(usage.week.percent)", ret_num, |a, _, _| {
                let Some(x) = as_num(arg(a, 0)) else {
                    return Ok(nul());
                };
                let f = 10f64.powi(places(arg(a, 1), 0) as i32);
                Ok(fin(js_round(x * f) / f))
            })
        },
    ),
    (
        "floor",
        SlateFunction {
            number_first: true,
            ..func(1, 1, "floor(x)", "floor(thread.context.percent)", ret_num, |a, _, _| {
                Ok(as_num(arg(a, 0)).map_or(nul(), |x| fin(x.floor())))
            })
        },
    ),
    (
        "ceil",
        SlateFunction {
            number_first: true,
            ..func(1, 1, "ceil(x)", "ceil(thread.context.percent)", ret_num, |a, _, _| {
                Ok(as_num(arg(a, 0)).map_or(nul(), |x| fin(x.ceil())))
            })
        },
    ),
    (
        "abs",
        SlateFunction {
            number_first: true,
            ..func(1, 1, "abs(x)", "abs(git.behind)", ret_num, |a, _, _| Ok(as_num(arg(a, 0)).map_or(nul(), |x| fin(x.abs()))))
        },
    ),
    (
        "min",
        SlateFunction {
            null_safe: true,
            ..func(1, 99, "min(a, b, ...) or min(list)", "min(usage.week.percent, 100)", ret_num, |a, env, _| {
                let ns = numbers_of_values(env, &list_or_args(a))?;
                Ok(ns.iter().copied().reduce(f64::min).map_or(nul(), fin))
            })
        },
    ),
    (
        "max",
        SlateFunction {
            null_safe: true,
            ..func(1, 99, "max(a, b, ...) or max(list)", "max(0, thread.context.free)", ret_num, |a, env, _| {
                let ns = numbers_of_values(env, &list_or_args(a))?;
                Ok(ns.iter().copied().reduce(f64::max).map_or(nul(), fin))
            })
        },
    ),
    (
        "clamp",
        SlateFunction {
            number_first: true,
            ..func(3, 3, "clamp(x, lo, hi)", "clamp(usage.week.percent, 0, 100)", ret_num, |a, _, _| {
                Ok(match (as_num(arg(a, 0)), as_num(arg(a, 1)), as_num(arg(a, 2))) {
                    (Some(x), Some(lo), Some(hi)) => fin(hi.min(lo.max(x))),
                    _ => nul(),
                })
            })
        },
    ),
    (
        "str",
        SlateFunction {
            ..func(1, 1, "str(x)", "str(pr.number)", ret_str, |a, _, _| {
                Ok(if missing(arg(a, 0)) { nul() } else { sval(text_of(arg(a, 0))) })
            })
        },
    ),
    (
        "num",
        SlateFunction {
            ..func(1, 1, "num(x)", "num(item.completedAt)", ret_num, |a, _, _| {
                let x = arg(a, 0);
                if let Some(n) = as_num(x) {
                    return Ok(fin(n));
                }
                match x {
                    Some(Value::Bool(b)) => Ok(Some(jnum(if *b { 1.0 } else { 0.0 }))),
                    Some(Value::String(s)) if !js_trim(s).is_empty() => {
                        let n = to_number(s);
                        if n.is_finite() {
                            return Ok(fin(n));
                        }
                        let b = s.as_bytes();
                        let iso_date = b.len() >= 10
                            && b[..4].iter().all(u8::is_ascii_digit)
                            && b[4] == b'-'
                            && b[5..7].iter().all(u8::is_ascii_digit)
                            && b[7] == b'-'
                            && b[8..10].iter().all(u8::is_ascii_digit);
                        Ok(if iso_date { ms(x).map_or(nul(), fin) } else { nul() })
                    }
                    _ => Ok(nul()),
                }
            })
        },
    ),
    (
        "bool",
        SlateFunction {
            null_safe: true,
            ..func(1, 1, "bool(x)", "bool($done)", ret_bool, |a, _, _| Ok(Some(Value::Bool(truthy(arg(a, 0))))))
        },
    ),
    (
        "concat",
        SlateFunction {
            null_safe: true,
            ..func(1, 99, "concat(a, b, ...)", "concat(git.branch, ' at ', git.head)", ret_str, |a, _, nodes| {
                let parts: Vec<(bool, Val)> =
                    a.iter().enumerate().map(|(i, v)| (!matches!(nodes.get(i), Some(SlateExpr::Lit { .. })), v.clone())).collect();
                Ok(joined(&parts).map_or(nul(), sval))
            })
        },
    ),
    (
        "json",
        SlateFunction {
            ..func(1, 1, "json(s)", "json($check.out).title", ret_any, |a, _, _| {
                Ok(match arg(a, 0) {
                    Some(Value::String(s)) => parse_json_lax(s).map(normalize).or(nul()),
                    _ => nul(),
                })
            })
        },
    ),
    (
        "lines",
        SlateFunction {
            ..func(1, 1, "lines(s)", "lines($tests.out)", ret_list, |a, _, _| {
                let Some(Value::String(s)) = arg(a, 0) else {
                    return Ok(nul());
                };
                let pieces: Vec<&str> = s.split('\n').collect();
                let last = pieces.len() - 1;
                let mut out: Vec<String> = pieces
                    .iter()
                    .enumerate()
                    .map(|(i, p)| if i < last { p.strip_suffix('\r').unwrap_or(p).to_string() } else { p.to_string() })
                    .collect();
                if out.last().is_some_and(String::is_empty) {
                    out.pop();
                }
                out.truncate(SLATE_LIMITS.list_items);
                Ok(Some(Value::Array(out.into_iter().map(Value::String).collect())))
            })
        },
    ),
    (
        "split",
        SlateFunction {
            ..func(
                2,
                2,
                "split(s, sep)",
                "split($tags, ',')",
                |_| list_of(str_t()),
                |a, _, _| {
                    Ok(match (arg(a, 0), arg(a, 1)) {
                        (Some(Value::String(s)), Some(Value::String(sep))) => {
                            let mut pieces = value_text_for_split(sep, s);
                            pieces.truncate(SLATE_LIMITS.list_items);
                            Some(Value::Array(pieces.into_iter().map(Value::String).collect()))
                        }
                        _ => nul(),
                    })
                },
            )
        },
    ),
    (
        "replace",
        SlateFunction {
            ..func(3, 3, "replace(s, from, to)", "replace(git.branch, '/', ' ')", ret_str, |a, _, _| {
                Ok(match (arg(a, 0), arg(a, 1)) {
                    (Some(Value::String(s)), Some(Value::String(from))) if !from.is_empty() => {
                        sval(s.replace(from.as_str(), &text_of(arg(a, 2))))
                    }
                    (Some(Value::String(s)), _) => sval(s.clone()),
                    _ => nul(),
                })
            })
        },
    ),
    (
        "contains",
        SlateFunction {
            null_safe: true,
            ..func(2, 2, "contains(list or text, value)", "contains(pluck(pr.checks, 'state'), 'fail')", ret_bool, |a, env, _| {
                match arg(a, 0) {
                    Some(Value::String(x)) => Ok(Some(Value::Bool(match arg(a, 1) {
                        Some(Value::String(v)) => x.contains(v.as_str()),
                        _ => false,
                    }))),
                    Some(Value::Array(items)) => {
                        env.charge(items.len())?;
                        let wanted = a.get(1).and_then(Option::as_ref).cloned().unwrap_or(Value::Null);
                        Ok(Some(Value::Bool(items.iter().any(|item| slate_equal(Some(item), Some(&wanted))))))
                    }
                    _ => Ok(Some(Value::Bool(false))),
                }
            })
        },
    ),
    (
        "startsWith",
        SlateFunction {
            null_safe: true,
            ..func(2, 2, "startsWith(s, p)", "startsWith(git.branch, 'poc/')", ret_bool, |a, _, _| {
                Ok(Some(Value::Bool(match (arg(a, 0), arg(a, 1)) {
                    (Some(Value::String(s)), Some(Value::String(p))) => s.starts_with(p.as_str()),
                    _ => false,
                })))
            })
        },
    ),
    (
        "endsWith",
        SlateFunction {
            null_safe: true,
            ..func(2, 2, "endsWith(s, p)", "endsWith(item.path, '.ts')", ret_bool, |a, _, _| {
                Ok(Some(Value::Bool(match (arg(a, 0), arg(a, 1)) {
                    (Some(Value::String(s)), Some(Value::String(p))) => s.ends_with(p.as_str()),
                    _ => false,
                })))
            })
        },
    ),
    (
        "orElse",
        SlateFunction {
            null_safe: true,
            ..func(
                2,
                2,
                "orElse(x, fallback)",
                "orElse(pr.word, 'no pull request')",
                |a| {
                    if matches!(a.first().map(|t| t.t), Some(SlateTypeName::Any | SlateTypeName::Null)) {
                        a.get(1).cloned().unwrap_or_else(any_t)
                    } else {
                        a.first().cloned().unwrap_or_else(any_t)
                    }
                },
                |a, _, _| Ok(if missing(arg(a, 0)) { Some(arg(a, 1).cloned().unwrap_or(Value::Null)) } else { arg(a, 0).cloned() }),
            )
        },
    ),
    (
        "exists",
        SlateFunction {
            null_safe: true,
            ..func(1, 1, "exists(x)", "exists(usage.note)", ret_bool, |a, _, _| Ok(Some(Value::Bool(!missing(arg(a, 0))))))
        },
    ),
    (
        "isNull",
        SlateFunction {
            null_safe: true,
            ..func(1, 1, "isNull(x)", "isNull(pr.number)", ret_bool, |a, _, _| Ok(Some(Value::Bool(missing(arg(a, 0))))))
        },
    ),
    (
        "coalesce",
        SlateFunction {
            null_safe: true,
            ..func(1, 99, "coalesce(a, b, ...)", "coalesce(pr.headSubject, git.branch)", ret_any, |a, _, _| {
                Ok(Some(a.iter().find(|v| !missing(v.as_ref())).and_then(Clone::clone).unwrap_or(Value::Null)))
            })
        },
    ),
    (
        "if",
        SlateFunction {
            null_safe: true,
            ..func(
                3,
                3,
                "if(cond, a, b)",
                "if(pr.draft, 'Draft', 'Ready')",
                |a| merge(a.get(1).cloned().unwrap_or_else(any_t), a.get(2).cloned().unwrap_or_else(any_t)),
                |a, _, _| {
                    let chosen = if truthy(arg(a, 0)) { a.get(1) } else { a.get(2) };
                    Ok(Some(chosen.and_then(Clone::clone).unwrap_or(Value::Null)))
                },
            )
        },
    ),
    (
        "len",
        SlateFunction {
            null_safe: true,
            ..func(1, 1, "len(x)", "len(pr.checks)", ret_num, |a, _, _| {
                Ok(Some(Value::from(match arg(a, 0) {
                    Some(Value::Array(items)) => items.len() as u64,
                    Some(Value::String(s)) => utf16::len(s) as u64,
                    Some(Value::Object(fields)) => fields.len() as u64,
                    _ => 0,
                })))
            })
        },
    ),
    (
        "count",
        SlateFunction {
            null_safe: true,
            list_first: true,
            ..func(1, 1, "count(list)", "count(pr.checks)", ret_num, |a, _, _| {
                Ok(Some(Value::from(list_ref(arg(a, 0)).map_or(0, Vec::len) as u64)))
            })
        },
    ),
    (
        "sum",
        SlateFunction {
            list_first: true,
            ..func(1, 2, "sum(list, field?)", "sum(thread.changes.files, 'additions')", ret_num, |a, env, _| {
                if !is_list(arg(a, 0)) {
                    return Ok(nul());
                }
                let ns = numbers_of(env, arg(a, 0), arg(a, 1))?;
                Ok(fin(ns.iter().fold(0.0, |acc, x| acc + x)))
            })
        },
    ),
    (
        "avg",
        SlateFunction {
            list_first: true,
            ..func(1, 2, "avg(list, field?)", "avg(thread.changes.files, 'additions')", ret_num, |a, env, _| {
                let ns = numbers_of(env, arg(a, 0), arg(a, 1))?;
                if ns.is_empty() {
                    return Ok(nul());
                }
                Ok(fin(ns.iter().fold(0.0, |acc, x| acc + x) / ns.len() as f64))
            })
        },
    ),
    (
        "first",
        SlateFunction {
            list_first: true,
            ..func(
                1,
                2,
                "first(list, n?)",
                "first(pr.checks).name",
                |a| {
                    if a.len() > 1 {
                        a.first().cloned().unwrap_or_else(list_t)
                    } else {
                        elem_of(a)
                    }
                },
                |a, _, _| {
                    let Some(items) = list_ref(arg(a, 0)) else {
                        return Ok(nul());
                    };
                    if undef(a, 1) {
                        return Ok(Some(items.first().cloned().unwrap_or(Value::Null)));
                    }
                    Ok(match as_num(arg(a, 1)) {
                        Some(n) => {
                            let k = n.floor().max(0.0) as usize;
                            Some(Value::Array(items.iter().take(k).cloned().collect()))
                        }
                        None => nul(),
                    })
                },
            )
        },
    ),
    (
        "last",
        SlateFunction {
            list_first: true,
            ..func(
                1,
                2,
                "last(list, n?)",
                "last(append($hist, $spot.json.v), 60)",
                |a| {
                    if a.len() > 1 {
                        a.first().cloned().unwrap_or_else(list_t)
                    } else {
                        elem_of(a)
                    }
                },
                |a, _, _| {
                    let Some(items) = list_ref(arg(a, 0)) else {
                        return Ok(nul());
                    };
                    if undef(a, 1) {
                        return Ok(Some(items.last().cloned().unwrap_or(Value::Null)));
                    }
                    Ok(match as_num(arg(a, 1)) {
                        Some(n) if n >= 1.0 => {
                            let k = n.floor() as usize;
                            Some(Value::Array(items[items.len().saturating_sub(k)..].to_vec()))
                        }
                        Some(_) => Some(Value::Array(vec![])),
                        None => nul(),
                    })
                },
            )
        },
    ),
    (
        "at",
        SlateFunction {
            list_first: true,
            ..func(2, 2, "at(list, i)", "at($questions, $i).text", elem_of_first, |a, _, _| Ok(pick_at(arg(a, 0), arg(a, 1))))
        },
    ),
    (
        "append",
        SlateFunction {
            null_safe: true,
            ..func(
                2,
                2,
                "append(list, x)",
                "append($hist, { at: time.now, v: $spot.json.v })",
                |a| {
                    let of = a.first().and_then(|t| t.of.as_deref().cloned()).or_else(|| a.get(1).cloned());
                    SlateType { t: SlateTypeName::List, of: of.map(Box::new), fields: None }
                },
                |a, env, _| {
                    let items: Vec<Value> = list_ref(arg(a, 0)).cloned().unwrap_or_default();
                    env.charge(items.len())?;
                    let mut out = items;
                    out.push(arg(a, 1).cloned().unwrap_or(Value::Null));
                    Ok(Some(Value::Array(out)))
                },
            )
        },
    ),
    (
        "pluck",
        SlateFunction {
            list_first: true,
            ..func(2, 2, "pluck(list, field)", "pluck(pr.checks, 'name')", ret_list, |a, env, _| {
                let (Some(items), Some(Value::String(f))) = (list_ref(arg(a, 0)), arg(a, 1)) else {
                    return Ok(nul());
                };
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    env.charge(1)?;
                    out.push(field_of(item, Some(&Value::String(f.clone()))).unwrap_or(Value::Null));
                }
                Ok(Some(Value::Array(out)))
            })
        },
    ),
    (
        "join",
        SlateFunction {
            list_first: true,
            ..func(2, 2, "join(list, sep)", "join(pluck(pr.checks, 'name'), ', ')", ret_str, |a, env, _| {
                let Some(items) = list_ref(arg(a, 0)) else {
                    return Ok(nul());
                };
                let sep = match arg(a, 1) {
                    Some(Value::String(s)) => s.clone(),
                    _ => ", ".to_string(),
                };
                let mut pieces = Vec::with_capacity(items.len());
                for item in items {
                    env.charge(1)?;
                    pieces.push(text_of(Some(item)));
                }
                Ok(sval(pieces.join(&sep)))
            })
        },
    ),
];

fn elem_of_first(a: &[SlateType]) -> SlateType {
    elem_of(a)
}

/// The numbers among the values, each read as a number (JS `Math.min(...ns)` input after numbersOf with no field).
fn numbers_of_values(env: &Env, items: &[Val]) -> Res<Vec<f64>> {
    let mut out = Vec::new();
    for item in items {
        env.charge(1)?;
        if let Some(x) = as_num(item.as_ref()) {
            out.push(x);
        }
    }
    Ok(out)
}

pub(super) fn function_spec(name: &str) -> Option<&'static SlateFunction> {
    SLATE_FUNCTIONS.iter().find(|(n, _)| *n == name).map(|(_, spec)| spec)
}
