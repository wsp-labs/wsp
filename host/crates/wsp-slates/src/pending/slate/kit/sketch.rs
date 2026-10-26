// SPDX-License-Identifier: AGPL-3.0-only
//! The sketch line of each piece, drawn from its props.
use super::*;

/// The items of a list prop whose when holds; a check with no thread keeps them all.
fn present(v: &dyn SlateSketchView, list: Vec<Value>) -> Vec<Value> {
    list.into_iter()
        .enumerate()
        .filter(|(i, x)| match rec_of(Some(x)).get("when") {
            Some(Value::String(when)) => v.test(when, &Value::Null, *i as i64),
            _ => true,
        })
        .map(|(_, x)| x)
        .collect()
}

/// Rows of a repeating piece, cut to the sketch's row cap, with the rest counted.
pub(super) fn rows(v: &dyn SlateSketchView, mut line: impl FnMut(&Value, i64) -> String, empty: &str) -> Vec<String> {
    let items = v.prop("items");
    if v.unbound() {
        return vec![format!("rows of {}", shown(items.as_ref()))];
    }
    let items = match items {
        Some(Value::Array(items)) if !items.is_empty() => items,
        _ => {
            let text = shown(v.prop("empty").as_ref());
            return vec![if text.is_empty() { empty.to_string() } else { text }];
        }
    };
    let cut = items.len().min(SLATE_LIMITS.sketch_rows);
    let mut out: Vec<String> = items[..cut].iter().enumerate().map(|(i, item)| line(item, i as i64)).collect();
    if items.len() > cut {
        out.push(format!("and {} more rows; slate_read values with the list's path reads them all", items.len() - cut));
    }
    out
}

fn actions(v: &dyn SlateSketchView, item: &Value, index: i64) -> String {
    list_of(v.raw("rowActions").as_ref())
        .into_iter()
        .filter(|a| match rec_of(Some(a)).get("when") {
            Some(Value::String(when)) => v.test(when, item, index),
            _ => true,
        })
        .map(|a| format!(" [{}]", shown(v.row(&a, item, index).get("label"))))
        .collect()
}

pub(super) fn reads_time(expr: &str) -> bool {
    static TIME_FNS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?-u:\b)(?:date|time|ago|weekday)\s*\(").expect("time function pattern"));
    static ITEM_FIELD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?-u:\b)item\.([A-Za-z_]+)(?-u:\b)").expect("item field pattern"));
    const TIME_FIELDS: &[&str] = &["at", "time", "date", "ts", "timestamp", "when", "day", "hour", "minute"];
    if TIME_FNS.is_match(expr) || slate_dependencies(expr).iter().any(|dep| dep == "time.now") {
        return true;
    }
    ITEM_FIELD.captures(expr).is_some_and(|c| TIME_FIELDS.contains(&&c[1]))
}

pub(super) fn sketch_empty(_: &dyn SlateSketchView) -> Vec<String> {
    vec![String::new()]
}

pub(super) fn sketch_chart(v: &dyn SlateSketchView) -> Vec<String> {
    let label = shown(v.prop("label").as_ref());
    let items = v.prop("items");
    let series = present(v, list_of(v.raw("series").as_ref()));
    let stack = !series.is_empty() && v.prop("stack") == Some(Value::Bool(true));
    let lines =
        if series.is_empty() { "line".to_string() } else { format!("{}{} lines", series.len(), if stack { " stacked" } else { "" }) };
    if v.unbound() {
        let x = if v.raw("x").is_none() { "{index}".to_string() } else { shown(v.prop("x").as_ref()) };
        return vec![format!("{label}  {lines} of {}, x {x}", shown(items.as_ref()))];
    }
    let x = v.raw("x");
    let list = list_of(items.as_ref());
    let format = word_of(v.prop("format"), "plain");
    let unit = shown(v.prop("unit").as_ref());
    let fig = |n: f64| join_with(" ", &[slate_figure(n, &format, None), unit.clone()]);
    let xs: Vec<Option<Value>> = list
        .iter()
        .enumerate()
        .map(|(i, item)| match &x {
            None => Some(json!(i)),
            Some(x) => v.row(&spec_of(vec![("x", Some(x.clone()))]), item, i as i64).get("x").cloned(),
        })
        .collect();
    let axis_line = |all: &[f64], at: &[usize]| -> String {
        let pair;
        let axis_input: &[f64] = if all.len() == 1 {
            pair = [all[0], all[0]];
            &pair
        } else {
            all
        };
        let axis = slate_chart_axis(axis_input);
        format!(
            "  x {} to {}, y {} to {}",
            slate_axis_word(xs[at[0]].as_ref()),
            slate_axis_word(xs[at[at.len() - 1]].as_ref()),
            fig(axis.from),
            fig(axis.to)
        )
    };
    let value_spec = || spec_of(vec![("value", v.raw("value"))]);
    if series.is_empty() {
        if v.raw("value").is_none() {
            let span = if list.is_empty() {
                String::new()
            } else {
                format!(", x {} to {}", slate_axis_word(xs[0].as_ref()), slate_axis_word(xs.last().and_then(Option::as_ref)))
            };
            return vec![js_trim_start(&format!(
                "{label}  nothing plotted from {} rows{span}; give value or a <series> per line",
                list.len()
            ))
            .to_string()];
        }
        let at: Vec<usize> = list
            .iter()
            .enumerate()
            .filter(|(i, item)| num_of(v.row(&value_spec(), item, *i as i64).get("value")).is_some())
            .map(|(i, _)| i)
            .collect();
        if at.is_empty() {
            return vec![format!("{label}  no points yet")];
        }
        let vals: Vec<f64> = at.iter().map(|&i| num_of(v.row(&value_spec(), &list[i], i as i64).get("value")).unwrap_or(0.0)).collect();
        return vec![
            js_trim_start(&format!(
                "{label}  last {}, min {}, max {} over {} point{}",
                fig(vals[vals.len() - 1]),
                fig(js_min(&vals)),
                fig(js_max(&vals)),
                vals.len(),
                if vals.len() == 1 { "" } else { "s" }
            ))
            .to_string(),
            axis_line(&vals, &at),
        ];
    }
    let read: Vec<Vec<Row>> = series.iter().map(|s| list.iter().enumerate().map(|(i, item)| v.row(s, item, i as i64)).collect()).collect();
    let own: Vec<Vec<f64>> = read.iter().map(|rows| rows.iter().map(|r| num_of(r.get("value")).unwrap_or(0.0)).collect()).collect();
    let at: Vec<usize> = (0..list.len()).filter(|&i| read.iter().any(|rows| num_of(rows[i].get("value")).is_some())).collect();
    if at.is_empty() {
        return vec![format!("{label}  {lines}, no points yet")];
    }
    let drawn: Vec<Vec<f64>> = if stack {
        (0..own.len()).map(|k| (0..own[0].len()).map(|i| own[..=k].iter().fold(0.0, |sum, line| sum + line[i])).collect()).collect()
    } else {
        own.clone()
    };
    let all: Vec<f64> = at.iter().flat_map(|&i| drawn.iter().map(move |line| line[i])).collect();
    let mut out =
        vec![js_trim_start(&format!("{label}  {lines} over {} point{}", at.len(), if at.len() == 1 { "" } else { "s" })).to_string()];
    for (k, rows) in read.iter().enumerate() {
        let vals: Vec<f64> = at.iter().filter_map(|&i| num_of(rows[i].get("value"))).collect();
        let name = rows
            .first()
            .and_then(|r| r.get("label"))
            .filter(|l| !l.is_null())
            .cloned()
            .or_else(|| rec_of(Some(&series[k])).get("label").cloned());
        let said = if vals.is_empty() {
            "no points yet".to_string()
        } else {
            format!("last {}, min {}, max {}", fig(vals[vals.len() - 1]), fig(js_min(&vals)), fig(js_max(&vals)))
        };
        let tone_only = pick(&rec_of(Some(&series[k])), &["tone"]);
        out.push(format!("  {}  {said}{}", shown(name.as_ref()), marks(&tone_only)));
    }
    let axis_all = if stack {
        let mut with_zero = vec![0.0];
        with_zero.extend(all);
        with_zero
    } else {
        all
    };
    out.push(axis_line(&axis_all, &at));
    out
}

pub(super) fn sketch_timeline(v: &dyn SlateSketchView) -> Vec<String> {
    let given = v.prop("now");
    let now = slate_time(given.as_ref()).unwrap_or_else(|| v.now());
    let not_time = |word: &str, value: Option<&Value>| -> String {
        match value {
            None | Some(Value::Null) => String::new(),
            Some(x) if slate_time(Some(x)).is_some() => String::new(),
            Some(x) => format!("{word} {} is not a time", json_stringify(x)),
        }
    };
    if v.unbound() {
        return vec![format!("{}  spans of {}", shown(v.prop("label").as_ref()), shown(v.prop("items").as_ref()))];
    }
    let mut group: Option<String> = None;
    let mut out = vec![join2(&[shown(v.prop("label").as_ref()), not_time("now", given.as_ref())])];
    let body = rows(
        v,
        |item, i| {
            let r = v.row(
                &spec_of(vec![
                    ("name", v.raw("name")),
                    ("start", v.raw("start")),
                    ("end", v.raw("end")),
                    ("state", v.raw("state")),
                    ("group", v.raw("group")),
                ]),
                item,
                i,
            );
            let start = slate_time(r.get("start"));
            let end = slate_time(r.get("end"));
            let state = slate_span_state(r.get("state"), start, end);
            let head = match r.get("group") {
                Some(Value::String(g)) if group.as_deref() != Some(g.as_str()) => {
                    group = Some(g.clone());
                    format!("  {g}\n")
                }
                _ => String::new(),
            };
            let clock = match start {
                None => String::new(),
                Some(start) => {
                    let until = if state == "running" {
                        "now".to_string()
                    } else {
                        match end {
                            None => "?".to_string(),
                            Some(end) => clock_of(end),
                        }
                    };
                    format!("{} to {until}", clock_of(start))
                }
            };
            let said = if state == "done" {
                match (start, end) {
                    (Some(start), Some(end)) => fmt_elapsed(end - start),
                    _ => "done".to_string(),
                }
            } else if state == "running" && start.is_some() {
                format!("running {}", fmt_elapsed(now - start.unwrap_or(now)))
            } else {
                state.to_string()
            };
            format!(
                "{head}  {}  {}",
                shown(r.get("name")),
                join2(&[clock, said, not_time("start", r.get("start")), not_time("end", r.get("end"))])
            )
        },
        "Nothing here",
    );
    out.extend(body.iter().flat_map(|line| line.split('\n').map(str::to_string)));
    out
}

pub(super) fn sketch_treemap(v: &dyn SlateSketchView) -> Vec<String> {
    let label = shown(v.prop("label").as_ref());
    let items = v.prop("items");
    if v.unbound() {
        return vec![format!("{label}  boxes of {}", shown(items.as_ref()))];
    }
    let format = word_of(v.prop("format"), "plain");
    let parts: Vec<Row> = list_of(items.as_ref())
        .iter()
        .enumerate()
        .map(|(i, item)| {
            v.row(&spec_of(vec![("name", v.raw("name")), ("value", v.raw("value")), ("group", v.raw("group"))]), item, i as i64)
        })
        .collect();
    let whole: f64 = parts.iter().map(|p| num_of(p.get("value")).map_or(0.0, |x| x.max(0.0))).sum();
    let groups: HashSet<String> = parts.iter().map(|p| shown(p.get("group"))).collect();
    let summary = if parts.is_empty() {
        String::new()
    } else {
        format!(
            "{} in {} parts{}",
            slate_figure(whole, &format, None),
            parts.len(),
            if groups.len() > 1 { format!(", {} groups", groups.len()) } else { String::new() }
        )
    };
    let mut out = vec![join2(&[label, summary])];
    out.extend(rows(
        v,
        |_, i| {
            let p = &parts[i as usize];
            let share = match num_of(p.get("value")) {
                Some(value) if whole > 0.0 => format!(" {}%", js_round(value.max(0.0) / whole * 100.0)),
                _ => String::new(),
            };
            format!("  {}  {}{share}", join2(&[shown(p.get("group")), shown(p.get("name"))]), figure(&format, p.get("value"), None))
        },
        "Nothing here",
    ));
    out
}

pub(super) fn sketch_donut(v: &dyn SlateSketchView) -> Vec<String> {
    let label = shown(v.prop("label").as_ref());
    let items = v.prop("items");
    if v.unbound() {
        return vec![format!("{label}  shares of {}", shown(items.as_ref()))];
    }
    let format = word_of(v.prop("format"), "plain");
    let unit = shown(v.prop("unit").as_ref());
    let given: Vec<(String, f64)> = list_of(items.as_ref())
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let r = v.row(&spec_of(vec![("name", v.raw("name")), ("value", v.raw("value"))]), item, i as i64);
            (shown(r.get("name")), num_of(r.get("value")).unwrap_or(0.0))
        })
        .collect();
    let refs: Vec<(&str, f64)> = given.iter().map(|(name, value)| (name.as_str(), *value)).collect();
    let parts = slate_shares(&refs);
    let whole: f64 = parts.iter().map(|p| p.value).sum();
    if whole == 0.0 {
        return vec![format!("{label}  no shares yet")];
    }
    let mut out = vec![join2(&[label, join_with(" ", &[slate_figure(whole, &format, None), unit])])];
    out.extend(
        parts.iter().map(|p| format!("  {}  {} {}%", p.name, slate_figure(p.value, &format, None), js_round(p.value / whole * 100.0))),
    );
    out
}

pub(super) fn sketch_table(v: &dyn SlateSketchView) -> Vec<String> {
    let cols = present(v, list_of(v.raw("columns").as_ref()));
    let heads: Vec<String> = cols
        .iter()
        .map(|c| {
            let r = rec_of(Some(c));
            format!("{}{}", shown(r.get("title")), marks(&pick(&r, &["mono", "align", "width"])))
        })
        .collect();
    let mut out = vec![format!("| {} |", heads.join(" | "))];
    out.extend(rows(
        v,
        |item, i| {
            let cells: Vec<String> = cols
                .iter()
                .map(|c| {
                    let r = v.row(c, item, i);
                    format!("{}{}", shown(r.get("value")), marks(&pick(&r, &["tone", "emphasis"])))
                })
                .collect();
            format!("| {} |{}", cells.join(" | "), actions(v, item, i))
        },
        "Nothing here",
    ));
    out
}

pub(super) fn sketch_rows_text(v: &dyn SlateSketchView) -> Vec<String> {
    rows(
        v,
        |item, i| {
            let r = v.row(&spec_of(vec![("title", v.raw("title")), ("done", v.raw("done"))]), item, i);
            let note = shown(v.row(&spec_of(vec![("note", v.raw("note"))]), item, i).get("note"));
            let tick = if r.get("done") == Some(&Value::Bool(true)) { "x" } else { " " };
            let tail = if note.is_empty() { String::new() } else { format!("  {note}") };
            format!("[{tick}] {}{tail}", shown(r.get("title")))
        },
        "Nothing here",
    )
}

pub(super) fn sketch_facts(v: &dyn SlateSketchView) -> Vec<String> {
    let all: Vec<Row> =
        present(v, list_of(v.raw("facts").as_ref())).iter().enumerate().map(|(i, f)| v.row(f, &Value::Null, i as i64)).collect();
    let empty = |f: &Row| matches!(f.get("value"), None | Some(Value::Null)) || f.get("value") == Some(&json!(""));
    let left: Vec<String> = all.iter().filter(|f| empty(f)).map(|f| shown(f.get("label"))).collect();
    let mut parts: Vec<String> =
        all.iter().filter(|f| !empty(f)).map(|f| format!("{}: {}{}", shown(f.get("label")), shown(f.get("value")), marks(f))).collect();
    if !left.is_empty() && !v.unbound() {
        parts.push(format!("(no value yet, not shown: {})", left.join(", ")));
    }
    vec![parts.join("  ")]
}

pub(super) fn sketch_output(v: &dyn SlateSketchView) -> Vec<String> {
    let name = shown(v.raw("run").as_ref());
    let r = rec_of(v.read(&name).as_ref());
    let Some(state) = r.get("state").map(js_string) else {
        return vec![format!("output {name}: not run yet")];
    };
    if v.unbound() || state == "idle" {
        return vec![format!("output {name}: not run yet")];
    }
    let lines: Vec<String> = match r.get("lines") {
        Some(Value::Array(lines)) if !lines.is_empty() => lines.iter().map(js_string).collect(),
        _ => [r.get("out"), r.get("err")]
            .into_iter()
            .filter_map(|s| s.and_then(Value::as_str))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
            .split('\n')
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect(),
    };
    if let Some(kind) = v.run_kind(&name) {
        if kind != "cmd" && state != "failed" {
            let result = match r.get("json") {
                None | Some(Value::Null) => r.get("out"),
                value => value,
            };
            let shape = sketch_slate_result(&slate_result_shape(result));
            return match shape.first() {
                None => vec![format!("output {name}: {state}")],
                Some(first) => {
                    let mut out = vec![format!("output {name}: {state}, {first}")];
                    out.extend(shape.iter().skip(1).take(3).map(|l| format!("  {l}")));
                    out
                }
            };
        }
    }
    let why = r.get("why").map(|w| format!(" ({})", js_string(w))).unwrap_or_default();
    let mut out = vec![format!("output {name}: {state}, {} line{}{why}", lines.len(), if lines.len() == 1 { "" } else { "s" })];
    out.extend(lines[lines.len().saturating_sub(3)..].iter().map(|l| format!("  {l}")));
    out
}

pub(super) fn sketch_input(v: &dyn SlateSketchView) -> Vec<String> {
    let raw = v.prop("value");
    let label = shown(v.prop("label").as_ref());
    if is_secret_handle(raw.as_ref()) {
        let set = raw.as_ref().and_then(|r| r.get("set")).is_some_and(|set| slate_truthy(Some(set)));
        return vec![if set { format!("{label}: ••••") } else { format!("{label}: (empty)") }];
    }
    let value = shown(raw.as_ref());
    if value.is_empty() {
        let placeholder = v.prop("placeholder");
        let hint = if slate_truthy(placeholder.as_ref()) { format!(", \"{}\"", shown(placeholder.as_ref())) } else { String::new() };
        return vec![format!("{label}: (empty{hint})")];
    }
    let clipped = if utf16::len(&value) > 60 { format!("{}…", utf16::cut(&value, 59)) } else { value };
    vec![format!("{label}: \"{clipped}\"")]
}

pub(super) fn sketch_select(v: &dyn SlateSketchView) -> Vec<String> {
    let options = v.prop("options");
    let list: Vec<String> = present(v, list_of(options.as_ref()))
        .iter()
        .map(|o| match o {
            Value::Object(_) | Value::Array(_) => {
                let r = rec_of(Some(o));
                let chosen = r.get("label").filter(|x| !x.is_null()).or_else(|| r.get("value").filter(|x| !x.is_null()));
                shown(chosen)
            }
            other => shown(Some(other)),
        })
        .collect();
    let placeholder = shown(v.prop("placeholder").as_ref());
    let empty = if placeholder.is_empty() { "(none)".to_string() } else { format!("(none, \"{placeholder}\")") };
    let value = shown(v.prop("value").as_ref());
    let value = if value.is_empty() { empty } else { value };
    let of = if list.len() <= 6 {
        let joined = list.join(", ");
        if joined.is_empty() {
            "0".to_string()
        } else {
            joined
        }
    } else {
        list.len().to_string()
    };
    vec![format!("{}: {value} (of {of})", shown(v.prop("label").as_ref()))]
}

pub(super) fn sketch_choices(v: &dyn SlateSketchView) -> Vec<String> {
    let picked = v.prop("value");
    let answer = v.prop("answer");
    let options = v.prop("options");
    let json_eq = |a: Option<&Value>, b: &Value| a.is_some_and(|a| json_stringify(a) == json_stringify(b));
    let present_picked = picked.as_ref().filter(|p| !p.is_null());
    let present_answer = answer.as_ref().filter(|a| !a.is_null());
    let list: Vec<Row> = present(v, list_of(options.as_ref()))
        .into_iter()
        .map(|o| match o {
            Value::Object(_) => rec_of(Some(&o)),
            other => Row::from([("value".to_string(), other)]),
        })
        .collect();
    let lines: Vec<String> = list
        .iter()
        .map(|o| {
            let chosen = present_picked.is_some_and(|p| json_eq(o.get("value"), p));
            let right = match (present_picked, present_answer) {
                (Some(_), Some(answer)) => json_eq(o.get("value"), answer),
                _ => false,
            };
            let mark = if right {
                " right"
            } else if chosen && present_answer.is_some() {
                " wrong"
            } else {
                ""
            };
            let note = o.get("note").filter(|n| !n.is_null()).map(|n| format!("  {}", shown(Some(n)))).unwrap_or_default();
            let label = o.get("label").filter(|l| !l.is_null()).or_else(|| o.get("value"));
            format!("({}) {}{mark}{note}", if chosen { "x" } else { " " }, shown(label))
        })
        .collect();
    let lost = present_picked.is_some_and(|p| !list.iter().any(|o| json_eq(o.get("value"), p)));
    let head = format!("{}:{}", shown(v.prop("label").as_ref()), if lost { format!(" {}", shown(picked.as_ref())) } else { String::new() });
    let mut out = vec![head];
    out.extend(lines.iter().map(|m| format!("  {m}")));
    out
}

pub(super) fn sketch_text(v: &dyn SlateSketchView) -> Vec<String> {
    let text = shown(v.prop("value").as_ref());
    vec![if text.is_empty() { shown(v.prop("placeholder").as_ref()) } else { text }]
}
