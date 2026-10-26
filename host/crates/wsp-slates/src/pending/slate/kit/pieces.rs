// SPDX-License-Identifier: AGPL-3.0-only
//! Every piece's entry, in the order the catalog lists them.
use super::*;

/// Every piece's entry, in the order the catalog lists them.
pub static SLATE_PIECES: LazyLock<IndexMap<&'static str, SlatePieceModule>> = LazyLock::new(|| {
    let mut pieces = IndexMap::new();

    pieces.insert(
        "column",
        SlatePieceModule {
            holds_children: true,
            props: with_box(props([
                ("gap", enm(DENSITY, Some("normal"))),
                ("align", enm(&["start", "center", "end", "stretch"], Some("stretch"))),
            ])),
            ..piece(
                "column",
                "Stacks children top to bottom.",
                "children in order",
                r#"<column gap="tight"><text>One</text><text>Two</text></column>"#,
                sketch_empty,
            )
        },
    );
    pieces.insert(
        "grid",
        SlatePieceModule {
            holds_children: true,
            props: with_box(props([
                ("columns", SlatePropSpec { min: Some(2.0), max: Some(4.0), required: true, ..spec(SlatePropType::Integer, SlateBinds::No) }),
                ("gap", enm(DENSITY, Some("normal"))),
                ("align", enm(PLACE, None)),
            ])),
            ..piece(
                "grid",
                "Children in 2 to 4 equal columns, one column under 360 px.",
                "column",
                r#"<grid columns={3}><number label="Open" value={3} /><number label="Done" value={9} /><number label="Failed" value={0} /></grid>"#,
                |v| vec![format!("grid of {}", shown(v.prop("columns").as_ref()))],
            )
        },
    );
    pieces.insert(
        "row",
        SlatePieceModule {
            holds_children: true,
            props: props([
                ("gap", enm(DENSITY, Some("normal"))),
                ("align", enm(&["start", "center", "end", "between"], Some("start"))),
                ("wrap", flag()),
            ]),
            ..piece(
                "row",
                "Places children side by side.",
                "as column",
                r#"<row align="between"><text>Left</text><text>Right</text></row>"#,
                sketch_empty,
            )
        },
    );
    pieces.insert(
        "section",
        SlatePieceModule {
            holds_children: true,
            props: with_box(props([
                ("title", req(str_prop())),
                ("note", str_prop()),
                ("collapsible", flag()),
                (
                    "open",
                    SlatePropSpec {
                        literal: true,
                        about: Some("open={false} starts it shut; open={$open} also keeps it".to_string()),
                        ..spec(SlatePropType::Boolean, SlateBinds::State)
                    },
                ),
                ("align", enm(PLACE, None)),
            ])),
            events: &["change"],
            collapsed: Some(|v: &dyn SlateSketchView| v.prop("open") == Some(Value::Bool(false))),
            ..piece(
                "section",
                "A titled group, optionally collapsible.",
                "children under a text with the title",
                r#"<section title="Files changed" collapsible><text>None yet</text></section>"#,
                |v| {
                    let title = shown(v.prop("title").as_ref());
                    if v.prop("open") == Some(Value::Bool(false)) {
                        vec![format!("{title} (collapsed)")]
                    } else {
                        vec![title]
                    }
                },
            )
        },
    );
    pieces.insert(
        "text",
        SlatePieceModule {
            text_prop: Some("value"),
            props: props([
                ("value", req(spec(SlatePropType::Text, SlateBinds::Yes))),
                ("tone", tone(SLATE_TONES)),
                ("emphasis", spec(SlatePropType::Enum(EMPHASIS), SlateBinds::Yes)),
                ("mono", flag()),
                ("size", enm(SIZE, None)),
                ("lines", SlatePropSpec { min: Some(1.0), max: Some(20.0), ..spec(SlatePropType::Integer, SlateBinds::No) }),
                ("placeholder", str_prop()),
            ]),
            ..piece(
                "text",
                "A line or a paragraph; a text child with {holes} fills a sentence.",
                "none needed",
                r#"<text tone="muted">{tokens(thread.context.free)} free</text>"#,
                sketch_text,
            )
        },
    );
    pieces.insert(
        "heading",
        SlatePieceModule {
            text_prop: Some("value"),
            props: props([
                ("value", SlatePropSpec { required: true, head: true, ..spec(SlatePropType::Text, SlateBinds::Yes) }),
                ("level", enm(&["title", "section", "label"], None)),
            ]),
            ..piece(
                "heading",
                "A heading on the app's type ladder: title, section, or label in small caps.",
                "text",
                r#"<heading level="title">Gold price</heading>"#,
                |v| {
                    let s = shown(v.prop("value").as_ref());
                    match v.prop("level").as_ref().and_then(Value::as_str) {
                        Some("title") => vec![format!("# {s}")],
                        Some("label") => vec![s.to_uppercase()],
                        _ => vec![format!("## {s}")],
                    }
                },
            )
        },
    );
    pieces.insert(
        "markdown",
        SlatePieceModule {
            text_prop: Some("value"),
            props: props([("value", SlatePropSpec { required: true, marks: true, ..req(str_prop()) })]),
            ..piece(
                "markdown",
                "Rich text, sanitised; images are not fetched.",
                "text",
                "<markdown>**Why.** The host keeps the last 5,000 events.</markdown>",
                |v| {
                    let text = shown(v.prop("value").as_ref());
                    let lines: Vec<&str> = text.split('\n').collect();
                    vec![if lines.len() > 1 { format!("{} (+{} lines)", lines[0], lines.len() - 1) } else { lines[0].to_string() }]
                },
            )
        },
    );
    pieces.insert(
        "number",
        SlatePieceModule {
            props: props([
                ("label", req(str_prop())),
                ("value", req(spec(SlatePropType::Text, SlateBinds::Yes))),
                ("format", enm(SLATE_FIGURES, Some("plain"))),
                ("unit", str_prop()),
                ("tone", tone(SLATE_TONES)),
                ("note", str_prop()),
                ("size", enm(&["normal", "large"], None)),
                (
                    "trend",
                    SlatePropSpec {
                        about: Some("read in the sketch alone; the panel's stat cell draws its figure alone, so a trend the person sees is a sparkline beside it".to_string()),
                        of_number: true,
                        ..spec(SlatePropType::List, SlateBinds::Yes)
                    },
                ),
            ]),
            ..piece(
                "number",
                "A figure with a label.",
                "text",
                r#"<number label="Spent" value={thread.cost.usd} format="usd" />"#,
                |v| {
                    let value = v.prop("value");
                    let fig = if value.as_ref().is_none_or(Value::is_null) {
                        "not read yet".to_string()
                    } else {
                        figure(&word_of(v.prop("format"), "plain"), value.as_ref(), Some(100.0))
                    };
                    let line = numbers(v.prop("trend").as_ref());
                    let trend_text = if line.len() > 1 {
                        format!("trend {} to {}", number_string(line[0]), number_string(line[line.len() - 1]))
                    } else {
                        String::new()
                    };
                    let unit = shown(v.prop("unit").as_ref());
                    vec![join2(&[
                        shown(v.prop("label").as_ref()),
                        join_with(" ", &[fig, unit]),
                        shown(v.prop("note").as_ref()),
                        trend_text,
                    ])]
                },
            )
        },
    );
    pieces.insert(
        "meter",
        SlatePieceModule {
            props: props([
                ("label", req(str_prop())),
                ("value", req(num_prop())),
                ("max", num_prop()),
                ("note", str_prop()),
                ("tone", tone(&["default", "good", "warning", "bad"])),
                ("format", enm(&["percent", "value", "fraction", "tokens", "bytes", "none"], Some("percent"))),
            ]),
            ..piece(
                "meter",
                "One value against a maximum.",
                "number",
                r#"<meter id="week" label="Weekly" value={usage.week.percent} note={`resets ${until(usage.week.resetsAt)}`} />"#,
                |v| {
                    let value = v.prop("value");
                    let max = or_default(v.prop("max"), json!(100));
                    let label = shown(v.prop("label").as_ref());
                    let note = shown(v.prop("note").as_ref());
                    if v.unbound() {
                        return vec![join2(&[label, format!("[{} of {}]", shown(value.as_ref()), shown(Some(&max))), note])];
                    }
                    if num_of(value.as_ref()).is_none() {
                        return vec![join2(&[label, "[..........] not read yet".to_string()])];
                    }
                    let format = word_of(v.prop("format"), "percent");
                    let drawn = js_trim_end(&format!(
                        "{} {}",
                        bar(value.as_ref(), Some(&max)),
                        figure(&format, value.as_ref(), num_of(Some(&max)))
                    ))
                    .to_string();
                    vec![join2(&[label, drawn, note])]
                },
            )
        },
    );
    pieces.insert(
        "bars",
        SlatePieceModule {
            repeating: true,
            props: props([
                ("label", req(str_prop())),
                ("items", SlatePropSpec { required: true, ..spec(SlatePropType::List, SlateBinds::Yes) }),
                ("key", row_key()),
                ("name", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::Item) }),
                ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Number, SlateBinds::Item) }),
                ("tone", spec(SlatePropType::Enum(SLATE_TONES), SlateBinds::Item)),
                ("max", num_prop()),
                ("format", enm(&["percent", "value", "none"], Some("value"))),
            ]),
            check: Some(|c: &mut SlatePieceCheck| {
                let props = c.props;
                if let Some(bind) = binding_of(props.get("items")).filter(|b| slate_is_series(js_trim(b))) {
                    c.add("W003", &format!("{bind} is a series over time; draw it as a line"), Some("items"), Some("<chart>"));
                } else if let Some(bind) = binding_of(props.get("name")).filter(|b| reads_time(b)) {
                    c.add(
                        "W003",
                        &format!("bars name each row by {bind}, a time: a series over time is a line"),
                        Some("name"),
                        Some("<chart>"),
                    );
                }
            }),
            ..piece(
                "bars",
                "Categories compared as rows of name, track and figure; never a series over time.",
                "table",
                r#"<bars label="Busiest" items={processes.list | take(5)} name={item.name} value={item.cpu} />"#,
                |v| {
                    let items = v.prop("items");
                    let values: Vec<f64> = list_of(items.as_ref())
                        .iter()
                        .enumerate()
                        .filter_map(|(i, item)| num_of(v.row(&spec_of(vec![("value", v.raw("value"))]), item, i as i64).get("value")))
                        .collect();
                    let max = or_default(v.prop("max"), json!(js_max(&values).max(0.0)));
                    let format = word_of(v.prop("format"), "value");
                    let mut out = vec![shown(v.prop("label").as_ref())];
                    out.extend(rows(
                        v,
                        |item, i| {
                            let r = v.row(&spec_of(vec![("name", v.raw("name")), ("value", v.raw("value"))]), item, i);
                            let t = v.row(&spec_of(vec![("tone", v.raw("tone"))]), item, i);
                            format!(
                                "  {}  {} {}{}",
                                shown(r.get("name")),
                                bar(r.get("value"), Some(&max)),
                                figure(&format, r.get("value"), num_of(Some(&max))),
                                marks(&t)
                            )
                        },
                        "Nothing here",
                    ));
                    out
                },
            )
        },
    );
    pieces.insert(
        "chart",
        SlatePieceModule {
            repeating: true,
            props: props([
                ("label", req(str_prop())),
                ("items", SlatePropSpec { required: true, ..spec(SlatePropType::List, SlateBinds::Yes) }),
                (
                    "x",
                    SlatePropSpec {
                        about: Some("a time on x (ISO or ms) labels the axis by clock, in this computer's time zone; a number by its value".to_string()),
                        ..spec(SlatePropType::Any, SlateBinds::Item)
                    },
                ),
                ("value", spec(SlatePropType::Number, SlateBinds::Item)),
                ("format", enm(SLATE_FIGURES, Some("plain"))),
                ("unit", str_prop()),
                ("tone", tone(SLATE_TONES)),
                ("height", enm(SIZE, Some("normal"))),
                ("stack", flag()),
            ]),
            items: IndexMap::from([(
                "series",
                SlateItemSpec {
                    prop: "series",
                    fields: props([
                        ("label", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::No) }),
                        ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Number, SlateBinds::Item) }),
                        ("tone", spec(SlatePropType::Enum(SLATE_TONES), SlateBinds::No)),
                    ]),
                    row: true,
                    row_when: false,
                    events: None,
                    min: None,
                    max: Some(4),
                },
            )]),
            check: Some(|c: &mut SlatePieceCheck| {
                let props = c.props;
                let x = props.get("x");
                if x.is_none() || binding_of(x).is_some_and(|b| js_trim(b) == "index") {
                    c.add(
                        "W013",
                        "the chart's x is each row's index, so its axis reads 0 to the count; give each row its time, for example x={item.at}",
                        Some("x"),
                        Some("x={item.at}"),
                    );
                }
                let has_series = !list_of(props.get("series")).is_empty();
                if props.get("value").is_none() && !has_series {
                    c.add(
                        "T304",
                        "a chart draws value={...} or a <series> per line, and it has neither",
                        Some("value"),
                        Some("value={item.v}"),
                    );
                } else if props.get("value").is_some() && has_series {
                    c.add(
                        "W020",
                        "the chart draws its <series> and leaves value out; drop value, or give it a <series> of its own",
                        Some("value"),
                        None,
                    );
                }
            }),
            ..piece(
                "chart",
                "Lines over a list, oldest first; stack piles the <series>.",
                "text",
                r#"<chart label="Gold" items={$hist} x={item.at} value={item.v} />"#,
                sketch_chart,
            )
        },
    );
    pieces.insert(
        "diagram",
        SlatePieceModule {
            raw_text: true,
            text_prop: Some("value"),
            props: props([
                (
                    "value",
                    SlatePropSpec {
                        required: true,
                        about: Some("Mermaid source; a text child is taken as written, braces and all. Labels a few words, decisions short questions; detail goes in a text beside it. Under about 15 nodes; past that, split it or summarize".to_string()),
                        ..spec(SlatePropType::String, SlateBinds::Yes)
                    },
                ),
                ("label", str_prop()),
            ]),
            ..piece(
                "diagram",
                "A Mermaid diagram: a flow, a sequence or a state machine; value={`...${$step}`} shows the live step.",
                "its source as code",
                "<diagram label=\"Deploy\" value={`flowchart LR\n  build --> test --> ship\n  classDef now stroke-width:3px\n  class ${$step} now`} />",
                |v| {
                    let text = shown(v.prop("value").as_ref());
                    let lines: Vec<&str> = text.split('\n').collect();
                    let label = shown(v.prop("label").as_ref());
                    let prefix = if label.is_empty() { String::new() } else { format!("{label}  ") };
                    let more = if lines.len() > 1 {
                        format!(" (+{} line{})", lines.len() - 1, if lines.len() == 2 { "" } else { "s" })
                    } else {
                        String::new()
                    };
                    vec![format!("{prefix}{}{more}", lines[0])]
                },
            )
        },
    );
    pieces.insert(
        "timeline",
        SlatePieceModule {
            repeating: true,
            props: props([
                ("label", req(str_prop())),
                ("items", SlatePropSpec { required: true, ..spec(SlatePropType::List, SlateBinds::Yes) }),
                ("name", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::Item) }),
                (
                    "start",
                    SlatePropSpec {
                        time: true,
                        about: Some("a time, ISO or ms; none waits".to_string()),
                        ..spec(SlatePropType::Any, SlateBinds::Item)
                    },
                ),
                (
                    "end",
                    SlatePropSpec {
                        time: true,
                        about: Some("none while it runs".to_string()),
                        ..spec(SlatePropType::Any, SlateBinds::Item)
                    },
                ),
                ("state", spec(SlatePropType::Enum(SLATE_SPAN_STATES), SlateBinds::Item)),
                (
                    "group",
                    SlatePropSpec {
                        about: Some("rows of one group sit under its name".to_string()),
                        ..spec(SlatePropType::String, SlateBinds::Item)
                    },
                ),
                (
                    "now",
                    SlatePropSpec {
                        time: true,
                        about: Some("where a running span ends; the clock if unset".to_string()),
                        ..spec(SlatePropType::Any, SlateBinds::Yes)
                    },
                ),
            ]),
            ..piece(
                "timeline",
                "Spans on a clock, Gantt style, such as a run's steps or a deploy's phases.",
                "table",
                r#"<timeline label="CI run" items={$steps} name={item.name} start={item.started} end={item.ended} group={item.job} />"#,
                sketch_timeline,
            )
        },
    );
    pieces.insert(
        "treemap",
        SlatePieceModule {
            repeating: true,
            props: props([
                ("label", req(str_prop())),
                ("items", SlatePropSpec { required: true, ..spec(SlatePropType::List, SlateBinds::Yes) }),
                ("name", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::Item) }),
                ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Number, SlateBinds::Item) }),
                ("group", spec(SlatePropType::String, SlateBinds::Item)),
                ("format", enm(SLATE_FIGURES, Some("plain"))),
            ]),
            ..piece(
                "treemap",
                "Sizes of one whole's parts as nested boxes, such as a bundle by module within package.",
                "bars",
                r#"<treemap label="Bundle" items={$stats.json.modules} name={item.name} value={item.bytes} group={item.package} format="bytes" />"#,
                sketch_treemap,
            )
        },
    );
    pieces.insert(
        "donut",
        SlatePieceModule {
            repeating: true,
            props: props([
                ("label", req(str_prop())),
                ("items", SlatePropSpec { required: true, ..spec(SlatePropType::List, SlateBinds::Yes) }),
                ("name", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::Item) }),
                ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Number, SlateBinds::Item) }),
                ("format", enm(SLATE_FIGURES, Some("plain"))),
                ("unit", str_prop()),
            ]),
            ..piece(
                "donut",
                "Shares of one whole, five parts at most with the rest as Other; never a series over time.",
                "bars",
                r#"<donut label="By country" items={$countries} name={item.name} value={item.n} format="integer" />"#,
                sketch_donut,
            )
        },
    );
    pieces.insert(
        "image",
        SlatePieceModule {
            props: props([
                (
                    "src",
                    SlatePropSpec {
                        required: true,
                        about: Some(format!(
                            "a path on the thread's computer, relative to its folder or whole, or an http(s) address; {IMAGE_TYPE_WORDS} up to {IMAGE_MAX_WORDS}"
                        )),
                        ..str_prop()
                    },
                ),
                ("caption", str_prop()),
            ]),
            check: Some(|c: &mut SlatePieceCheck| {
                if let Some(Value::String(src)) = c.props.get("src") {
                    if let SlateImageSource::Problem(problem) = slate_image_source(src) {
                        c.add(&problem.code, &problem.message, Some("src"), Some("src=\"/tmp/home.png\""));
                    }
                }
            }),
            ..piece(
                "image",
                "A picture from a file on the thread's computer, such as a screenshot or a saved plot, or from an https address once the person allows its domain; opens large to zoom.",
                "its path as text",
                r#"<image src="/tmp/login-failed.png" caption="Login, after the redirect" />"#,
                |v| vec![join2(&[format!("[image {}]", shown(v.prop("src").as_ref())), shown(v.prop("caption").as_ref())])],
            )
        },
    );
    pieces.insert(
        "images",
        SlatePieceModule {
            repeating: true,
            props: props([
                (
                    "items",
                    SlatePropSpec {
                        required: true,
                        most: Some(SlateMost { count: SLATE_LIMITS.images_per_list, noun: "images" }),
                        ..spec(SlatePropType::List, SlateBinds::Yes)
                    },
                ),
                ("src", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::Item) }),
                ("caption", spec(SlatePropType::String, SlateBinds::Item)),
                ("layout", enm(SLATE_IMAGE_LAYOUTS, Some("gallery"))),
            ]),
            check: Some(|c: &mut SlatePieceCheck| {
                let props = c.props;
                let items = props.get("items");
                if props.get("layout") == Some(&json!("compare")) {
                    if let Some(Value::Array(list)) = items {
                        if list.len() != 2 {
                            c.add(
                                "T303",
                                &format!("a compare shows exactly two images, before and after, and this list has {}", list.len()),
                                Some("items"),
                                None,
                            );
                        }
                    }
                }
                if let Some(Value::Array(list)) = items {
                    if list.len() > SLATE_LIMITS.images_per_list {
                        let problem = slate_list_too_long(list.len(), SLATE_LIMITS.images_per_list, "images", None);
                        c.add("R905", &problem.message, Some("items"), None);
                    }
                }
            }),
            ..piece(
                "images",
                "Several pictures, as image takes them: a row that scrolls, a grid, or a before and after under a slider.",
                "a list of paths",
                r#"<images layout="compare" items={[{ p: "shots/before.png", c: "Before" }, { p: "shots/after.png", c: "After" }]} src={item.p} caption={item.c} />"#,
                |v| {
                    let layout = word_of(v.prop("layout"), "gallery");
                    let mut out = vec![format!("{layout} of images")];
                    out.extend(rows(
                        v,
                        |item, i| {
                            let r = v.row(
                                &spec_of(vec![("src", v.raw("src")), ("caption", v.raw("caption"))]),
                                item,
                                i,
                            );
                            format!("  {}", join2(&[format!("[image {}]", shown(r.get("src"))), shown(r.get("caption"))]))
                        },
                        "No images",
                    ));
                    out
                },
            )
        },
    );
    pieces.insert(
        "sparkline",
        SlatePieceModule {
            props: props([
                ("label", req(str_prop())),
                ("values", SlatePropSpec { required: true, of_number: true, ..spec(SlatePropType::List, SlateBinds::Yes) }),
                ("tone", tone(SLATE_TONES)),
            ]),
            ..piece(
                "sparkline",
                "A small line beside text, the way to show a figure's trend.",
                "text",
                r#"<sparkline label="Load" values={pluck($hist, 'v')} />"#,
                |v| vec![trend(&shown(v.prop("label").as_ref()), &numbers(v.prop("values").as_ref()))],
            )
        },
    );
    pieces.insert(
        "table",
        SlatePieceModule {
            repeating: true,
            props: props([
                ("items", SlatePropSpec { required: true, ..spec(SlatePropType::List, SlateBinds::Yes) }),
                ("key", row_key()),
                ("empty", str_prop()),
                (
                    "rows",
                    SlatePropSpec { min: Some(1.0), max: Some(5000.0), ..spec(SlatePropType::Integer, SlateBinds::No) },
                ),
            ]),
            items: IndexMap::from([
                (
                    "col",
                    SlateItemSpec {
                        prop: "columns",
                        fields: props([
                            ("title", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::No) }),
                            ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Text, SlateBinds::Item) }),
                            ("tone", spec(SlatePropType::Enum(SLATE_TONES), SlateBinds::Item)),
                            ("emphasis", spec(SlatePropType::Enum(EMPHASIS), SlateBinds::Item)),
                            ("mono", flag()),
                            ("align", enm(&["start", "end"], Some("start"))),
                            ("width", enm(&["fit", "fill"], None)),
                        ]),
                        row: true,
                        row_when: false,
                        events: None,
                        min: Some(1),
                        max: Some(SLATE_LIMITS.columns),
                    },
                ),
                ("action", row_action_item()),
            ]),
            ..piece(
                "table",
                "Rows with columns from a bound list: <col> per column, <action> per row button.",
                "a list of text rows",
                r#"<table id="checks" items={pr.checks} key={item.name}><col title="Check" value={item.name} /><action label="Fix" when={item.state == 'fail'} onPress={send("Fix this check.", item.name)} /></table>"#,
                sketch_table,
            )
        },
    );
    pieces.insert(
        "checklist",
        SlatePieceModule {
            repeating: true,
            events: &["change"],
            props: props([
                ("items", SlatePropSpec { required: true, ..spec(SlatePropType::List, SlateBinds::Yes) }),
                ("key", row_key()),
                ("title", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::Item) }),
                ("done", SlatePropSpec { required: true, ..spec(SlatePropType::Boolean, SlateBinds::Item) }),
                (
                    "state",
                    SlatePropSpec {
                        unseen: Some("this build's checklist draws done alone"),
                        ..spec(SlatePropType::Enum(&["pending", "working", "done"]), SlateBinds::Item)
                    },
                ),
                ("note", spec(SlatePropType::String, SlateBinds::Item)),
                ("editable", flag()),
                ("empty", str_prop()),
            ]),
            check: Some(|c: &mut SlatePieceCheck| {
                let props = c.props;
                if props.get("editable") == Some(&Value::Bool(true)) {
                    let bound_value = binding_of(props.get("items"))
                        .is_some_and(|bind| (c.kind)(&slate_own_name(bind).unwrap_or_default()).as_deref() == Some("value"));
                    if !bound_value {
                        c.add(
                            "S502",
                            "an editable checklist writes its ticks back, so items binds a value: items={$steps}",
                            Some("items"),
                            None,
                        );
                    }
                }
            }),
            ..piece(
                "checklist",
                "Rows with a tick; editable writes the tick back to the value.",
                "text rows",
                r#"<checklist items={$steps} title={item.title} done={item.done} editable />"#,
                sketch_rows_text,
            )
        },
    );
    pieces.insert(
        "facts",
        SlatePieceModule {
            props: props([("layout", enm(&["line", "grid"], Some("line")))]),
            items: IndexMap::from([(
                "fact",
                SlateItemSpec {
                    prop: "facts",
                    fields: props([
                        ("label", req(str_prop())),
                        ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Text, SlateBinds::Yes) }),
                        ("tone", tone(SLATE_TONES)),
                        ("emphasis", spec(SlatePropType::Enum(EMPHASIS), SlateBinds::Yes)),
                        ("mono", flag()),
                    ]),
                    row: false,
                    row_when: false,
                    events: None,
                    min: Some(1),
                    max: Some(SLATE_LIMITS.facts),
                },
            )]),
            ..piece(
                "facts",
                "Label and value pairs: <fact> per pair.",
                "text lines",
                r#"<facts><fact label="State" value={pr.word} /><fact label="Review" value={word(pr.review)} /></facts>"#,
                sketch_facts,
            )
        },
    );
    pieces.insert(
        "status",
        SlatePieceModule {
            text_prop: Some("value"),
            props: props([
                ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Text, SlateBinds::Yes) }),
                ("tone", tone(&["default", "muted", "good", "warning", "bad", "info"])),
            ]),
            ..piece(
                "status",
                "A dot in a tone with a word: Live, Down, Waiting.",
                "text",
                "<status tone={$up ? 'good' : 'bad'}>{$up ? 'Live' : 'Down'}</status>",
                |v| {
                    let tone_word = shown(v.prop("tone").as_ref());
                    let tone_word = if tone_word.is_empty() { "default".to_string() } else { tone_word };
                    vec![format!("({tone_word}) {}", shown(v.prop("value").as_ref()))]
                },
            )
        },
    );
    pieces.insert(
        "chip",
        SlatePieceModule {
            text_prop: Some("value"),
            props: props([("value", SlatePropSpec { required: true, ..spec(SlatePropType::Text, SlateBinds::Yes) })]),
            ..piece("chip", "A short fact in a small framed chip.", "text", "<chip>{git.branch}</chip>", |v| {
                vec![format!("[{}]", shown(v.prop("value").as_ref()))]
            })
        },
    );
    pieces.insert(
        "ring",
        SlatePieceModule {
            props: props([
                ("label", req(str_prop())),
                ("value", req(num_prop())),
                ("max", num_prop()),
                ("format", enm(&["percent", "value", "fraction"], Some("percent"))),
                ("tone", tone(&["default", "good", "warning", "bad"])),
                ("note", str_prop()),
            ]),
            ..piece(
                "ring",
                "Progress as a ring with its figure inside.",
                "meter",
                r#"<ring label="Done" value={$done} max={len($steps)} format="fraction" />"#,
                |v| {
                    let value = v.prop("value");
                    let max = or_default(v.prop("max"), json!(100));
                    let label = shown(v.prop("label").as_ref());
                    if v.unbound() || num_of(value.as_ref()).is_none() {
                        let shown_value = if v.unbound() { shown(value.as_ref()) } else { "not read yet".to_string() };
                        return vec![join2(&[label, format!("({shown_value})")])];
                    }
                    let format = word_of(v.prop("format"), "percent");
                    let drawn = format!("({}) {}", figure(&format, value.as_ref(), num_of(Some(&max))), bar(value.as_ref(), Some(&max)));
                    vec![join2(&[label, drawn, shown(v.prop("note").as_ref())])]
                },
            )
        },
    );
    pieces.insert(
        "output",
        SlatePieceModule {
            props: props([
                ("run", SlatePropSpec { required: true, names_run: true, ..spec(SlatePropType::Path, SlateBinds::No) }),
                ("label", str_prop()),
                (
                    "lines",
                    SlatePropSpec {
                        min: Some(3.0),
                        max: Some(40.0),
                        about: Some("how many of the newest lines show, out then err, 8 by default".to_string()),
                        ..spec(SlatePropType::Integer, SlateBinds::No)
                    },
                ),
                ("wrap", flag()),
            ]),
            ..piece(
                "output",
                "A run's output as it streams, with its state and a Cancel.",
                "text with the run's state",
                r#"<output run={$tests} lines={12} />"#,
                sketch_output,
            )
        },
    );
    pieces.insert(
        "button",
        SlatePieceModule {
            interactive: true,
            text_prop: Some("label"),
            needs_handler: Some("press"),
            events: &["press"],
            props: props([
                ("label", req(str_prop())),
                ("variant", enm(VARIANT, None)),
                ("held", str_prop()),
                ("note", str_prop()),
                ("size", enm(&["normal", "small"], None)),
            ]),
            ..piece(
                "button",
                "An action the person presses. held is a sentence that disables it: bind it to a condition.",
                "none needed",
                r#"<button label="Next" variant="primary" held={$ok ? null : 'Check the id first'} onPress={set($step, 2)} />"#,
                |v| {
                    let held = slate_held_text(v.prop("held").as_ref());
                    let tail = held.map(|h| format!(" (held: {h})")).unwrap_or_default();
                    vec![format!("[ {} ]{tail}", shown(v.prop("label").as_ref()))]
                },
            )
        },
    );
    pieces.insert(
        "input",
        SlatePieceModule {
            interactive: true,
            events: &["change", "submit"],
            props: props([
                ("label", req(str_prop())),
                ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Text, SlateBinds::State) }),
                ("lines", SlatePropSpec { min: Some(1.0), max: Some(20.0), ..spec(SlatePropType::Integer, SlateBinds::No) }),
                ("kind", enm(&["text", "number", "password"], Some("text"))),
                ("placeholder", str_prop()),
                ("held", str_prop()),
                ("mono", flag()),
                ("submit", str_prop()),
            ]),
            ..piece(
                "input",
                "Text in, written to its $value as the person types; a secret's input is a password field.",
                "none needed",
                r#"<input label="Issue number" value={$id} mono />"#,
                sketch_input,
            )
        },
    );
    pieces.insert(
        "select",
        SlatePieceModule {
            interactive: true,
            events: &["change"],
            props: props([
                ("label", req(str_prop())),
                ("value", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::State) }),
                ("options", spec(SlatePropType::List, SlateBinds::Yes)),
                ("placeholder", str_prop()),
                ("held", str_prop()),
            ]),
            items: IndexMap::from([(
                "option",
                SlateItemSpec {
                    prop: "options",
                    fields: props([
                        ("value", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::No) }),
                        ("label", spec(SlatePropType::String, SlateBinds::No)),
                    ]),
                    row: false,
                    row_when: false,
                    events: None,
                    min: None,
                    max: None,
                },
            )]),
            ..piece(
                "select",
                "One choice from options, written to its $value.",
                "none needed",
                r#"<select label="Hide" value={$hide}><option value="Done" /><option value="Canceled" /></select>"#,
                sketch_select,
            )
        },
    );
    pieces.insert(
        "choices",
        SlatePieceModule {
            interactive: true,
            events: &["change"],
            props: props([
                ("label", req(str_prop())),
                ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Any, SlateBinds::State) }),
                ("options", spec(SlatePropType::List, SlateBinds::Yes)),
                ("answer", spec(SlatePropType::Any, SlateBinds::Yes)),
                ("held", str_prop()),
            ]),
            items: IndexMap::from([(
                "option",
                SlateItemSpec {
                    prop: "options",
                    fields: props([
                        ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Any, SlateBinds::Yes) }),
                        ("label", str_prop()),
                        ("note", str_prop()),
                    ]),
                    row: false,
                    row_when: false,
                    events: None,
                    min: None,
                    max: None,
                },
            )]),
            ..piece(
                "choices",
                "Large options to pick one, written to its $value; answer marks the right one once picked.",
                "select",
                r#"<choices label="Which layer routes packets?" value={$pick} options={$q.options} answer={$q.answer} />"#,
                sketch_choices,
            )
        },
    );
    pieces.insert(
        "toggle",
        SlatePieceModule {
            interactive: true,
            events: &["change"],
            props: props([
                ("label", req(str_prop())),
                ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Boolean, SlateBinds::State) }),
                ("note", str_prop()),
                ("held", str_prop()),
            ]),
            ..piece(
                "toggle",
                "An on/off switch written to its boolean $value.",
                "none needed",
                r#"<toggle label="Show done" value={$showDone} />"#,
                |v| {
                    vec![format!(
                        "{}: {}",
                        shown(v.prop("label").as_ref()),
                        if v.prop("value") == Some(Value::Bool(true)) { "on" } else { "off" }
                    )]
                },
            )
        },
    );
    pieces.insert(
        "empty",
        SlatePieceModule {
            holds_children: true,
            child_limit: Some(SlateChildLimit { max: 1, types: &["button"] }),
            text_prop: Some("body"),
            props: props([("title", req(str_prop())), ("body", str_prop())]),
            ..piece(
                "empty",
                "An empty state, with at most one button under it.",
                "text",
                r#"<empty when={pr.number == null} title="No pull request yet">This branch has none.</empty>"#,
                |v| {
                    let parts = [shown(v.prop("title").as_ref()), shown(v.prop("body").as_ref())];
                    let joined: Vec<&str> = parts.iter().map(String::as_str).filter(|p| !p.is_empty()).collect();
                    vec![joined.join(". ").replace("..", ".")]
                },
            )
        },
    );

    pieces
});

/// The entry of a piece by its type name, or None when no piece has it.
pub fn slate_piece_module(kind: &str) -> Option<&'static SlatePieceModule> {
    SLATE_PIECES.get(kind)
}
