// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/catalog.ts at 65f6b89c7.

use std::rc::Rc;

use indexmap::IndexMap;
use serde_json::Value;
use wsp_words::js::{is_js_whitespace, js_trim, number_string};

use super::examples::SLATE_EXAMPLES;
use super::*;

const BOX: [&str; 2] = ["pad", "surface"];
const SAID_ONCE: [&str; 4] = ["tone", "emphasis", "pad", "align"];
const BACK: &str = "The pieces and rules: slate_catalog with no name.";
const NOT_IN: &str = "is not in the catalog";
const SLATE_RUN_FIELDS: [&str; 15] = ["state", "why", "exit", "out", "err", "json", "lines", "startedAt", "endedAt", "ms", "runs", "cut", "stale", "refreshing", "text"];

pub const SLATE_CHAPTERS: &[&str] = &["runs", "functions", "steps", "handlers", "patch", "examples"];

pub const SLATE_RULES: &[&str] = &[
    "One <slate>, one root piece, declarations beside it. Give an id to a piece you will change.",
    r#"prop="text" is literal; prop={formula} reads live data or $values with operators, a ? b : c, [lists], {records} and functions, never methods or =>. A text child with {holes} fills a sentence."#,
    r#"Props are meaning, never style. held is a sentence that disables: bind it to a condition; "" or false enables."#,
    "A list binds to items; item and index read the row.",
    r#"A press reaches you only through send("literal text", $path); <when> chains run without you."#,
    r#"Small edit: patch by id, <props id="price" value={$spot.json.v} />; bigger: resend the whole slate with if_version."#,
    "Simple and airy unless asked for more: few pieces, one idea per section, short labels, no emoji. Separate things by layout, never by ·, • or |.",
    "One heading per section: its title or a heading, in sentence case, never bold text.",
    "Status and last-checked lines small and muted, beside their subject.",
    "Actions at the end of their row; one primary per section.",
    "mono only for figures, ids, times and paths.",
    "Every live number says its window and unit. Show a run's json fields, never its raw out; times through date(), time() or ago().",
    "Nothing centered but a lone figure or card.",
    "Use the slate tools, never wsp from a shell.",
    "A secret the person types goes in a <secret> input, never a file or the chat. One in a file stays there for the run to read, never you.",
    "A button asked for where the project has no UI goes here: the slate is its UI, and the reply says so.",
];

pub const SLATE_INDEX_EXAMPLE: &str = r#"<slate title="Issue">
  <value name="id" start="" />
  <run name="check" cmd='gh api "repos/{owner}/{repo}/issues/$ID"' env={{ ID: $id }} />
  <when change={$id} do={start($check)} />
  <column>
    <input label="Issue" value={$id} />
    <text when={$check.exit == 0}>{$check.json.title}</text>
    <button label="Fix it" onPress={send("Fix this issue.", $id)} />
  </column>
</slate>"#;

const PATCH: &str = r#"patch: elements without <slate>, sent as text to slate_write; one write may hold several.
<props id="price" tone="warning" />: merges these props into the piece.
<text id="eta">Ready</text>: a piece with an id that exists replaces it whole, children too.
<add under="list" at={0}><text id="new">New</text></add>: adds pieces under a piece, at an index from 0, else last.
<move id="new" under="other" at={1} />: moves a piece.
<remove id="new" />: removes a piece and what it holds. <remove name="hist" /> removes a declaration.
<value name="hist" start={[]} />, <run ...>, <derived ...>, <file name="x.py">...</file>: adds the declaration or replaces the one of that name.
<clear /> empties the slate; <undo /> goes back one write. Each stands alone in its write.
For anything bigger, resend the whole <slate> with if_version set to the version you read.
version, which every write and read answers, counts the slate's edits and is what if_version takes; wsp/2 and schema 2 name the format.
check with press walks a press and its <when> chain on a copy: nothing runs and nothing is sent, and it says what would start.
check with values {"$run.json": {...}, "$run.exit": 0} previews the panel for a result the run has not had yet."#;

/// A conservative token count: each word, each number and each other non-space character is one. Real tokenizers
/// join common words with their spaces and punctuation, so they count fewer.
pub fn slate_tokens(text: &str) -> usize {
    let mut count = 0;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_alphabetic() {
            while chars.peek().is_some_and(|d| d.is_ascii_alphabetic()) {
                chars.next();
            }
            count += 1;
        } else if c.is_ascii_digit() {
            while chars.peek().is_some_and(|d| d.is_ascii_digit()) {
                chars.next();
            }
            count += 1;
        } else if !is_js_whitespace(c) {
            count += c.len_utf16();
        }
    }
    count
}

/// Whether a prop takes the figure formats, which the index says once rather than on every piece that plots.
pub fn slate_takes_figures(spec: &SlatePropSpec) -> bool {
    matches!(spec.prop_type, SlatePropType::Enum(words) if words == SLATE_FIGURES)
}

fn prop_word(name: &str, spec: &SlatePropSpec) -> String {
    let words = match spec.prop_type {
        SlatePropType::Enum(values) if !SAID_ONCE.contains(&name) && !slate_takes_figures(spec) => format!("({})", values.join("|")),
        _ => String::new(),
    };
    format!("{name}{}{words}", if spec.required { "!" } else { "" })
}

fn on_event(event: &str) -> String {
    let mut chars = event.chars();
    match chars.next() {
        Some(first) => format!("on{}{}", first.to_uppercase(), chars.as_str()),
        None => "on".to_string(),
    }
}

fn piece_line(p: &SlatePieceModule) -> String {
    let mut words: Vec<String> = Vec::new();
    for (k, spec) in &p.props {
        if p.items.values().any(|i| i.prop == *k) || BOX.contains(k) {
            continue;
        }
        words.push(prop_word(k, spec));
    }
    words.extend(p.events.iter().map(|e| on_event(e)));
    let items: Vec<String> = p
        .items
        .iter()
        .map(|(tag, s)| {
            let mut parts: Vec<String> = s.fields.iter().map(|(k, f)| prop_word(k, f)).collect();
            if s.events.is_some() {
                parts.push("onPress".to_string());
            }
            format!("<{tag} {}>", parts.join(" "))
        })
        .collect();
    let mut line = format!("{}: {}", p.kind, words.join(" "));
    if !items.is_empty() {
        line.push_str(&format!("; {}", items.join(" ")));
    }
    if p.holds_children && p.child_limit.is_none() {
        line.push_str("; children");
    }
    line
}

fn source_line(s: &SlateSourceModule) -> String {
    match &s.shape {
        SlateShape::Keyed(_) => format!("{}.<server>: state tools[] resources[]", s.name),
        SlateShape::Fields(fields) => {
            let shown: Vec<(&str, &SlateShape)> = fields
                .iter()
                .filter(|(k, _)| s.indexed.as_ref().map_or(true, |ix| ix.contains(*k)))
                .map(|(k, v)| (*k, v))
                .collect();
            let more = if shown.len() < fields.len() { " ..." } else { "" };
            let names: Vec<String> = shown.iter().map(|(k, v)| if matches!(v, SlateShape::List(_)) { format!("{k}[]") } else { k.to_string() }).collect();
            format!("{}: {}{more}", s.name, names.join(" "))
        }
        _ => s.name.to_string(),
    }
}

fn is_scalar(shape: &SlateShape) -> bool {
    matches!(shape, SlateShape::Number | SlateShape::String | SlateShape::Boolean | SlateShape::Time | SlateShape::Any)
}

fn spec_text(spec: &SlatePropSpec) -> String {
    let type_text = match spec.prop_type {
        SlatePropType::Enum(values) if values == SLATE_TONES => "tone".to_string(),
        SlatePropType::Enum(values) => values.join("|"),
        SlatePropType::Text => "text or number".to_string(),
        SlatePropType::Path => "$run".to_string(),
        SlatePropType::String => "string".to_string(),
        SlatePropType::Number => "number".to_string(),
        SlatePropType::Integer => "integer".to_string(),
        SlatePropType::Boolean => "boolean".to_string(),
        SlatePropType::List => "list".to_string(),
        SlatePropType::Any => "any".to_string(),
        SlatePropType::Id => "id".to_string(),
    };
    let mut marks: Vec<String> = Vec::new();
    if spec.required {
        marks.push("required".to_string());
    }
    match spec.binds {
        SlateBinds::Yes => marks.push("formula".to_string()),
        SlateBinds::State if spec.literal => marks.push("two-way $value or a literal start".to_string()),
        SlateBinds::State => marks.push("two-way $value".to_string()),
        SlateBinds::Item => marks.push("per row".to_string()),
        SlateBinds::No => {}
    }
    if let (Some(min), Some(max)) = (spec.min, spec.max) {
        marks.push(format!("{} to {}", number_string(min), number_string(max)));
    }
    if marks.is_empty() {
        type_text
    } else {
        format!("{type_text}, {}", marks.join(", "))
    }
}

fn sketch_options() -> SketchOptions {
    let resolve: Rc<dyn Fn(&str) -> Option<Value>> = Rc::new(|_: &str| -> Option<Value> { None });
    SketchOptions {
        eval: SlateEvalContext { now: 0, item: None, index: None, resolve },
        version: 0,
        warnings: Vec::new(),
        check: true,
        problems: Vec::new(),
        waiting: Vec::new(),
    }
}

fn piece_entry(p: &SlatePieceModule) -> String {
    let own: Vec<(&str, &SlatePropSpec)> = p.props.iter().filter(|(k, _)| !p.items.values().any(|i| i.prop == **k)).map(|(k, v)| (*k, v)).collect();
    let sketched = compile_slate_text(&format!("<slate><column>{}</column></slate>", p.example))
        .document
        .map(|doc| sketch_slate(Some(&doc), &SlateValues::new(), sketch_options()).split('\n').nth(1).unwrap_or("").to_string())
        .unwrap_or_default();
    let mut lines = vec![format!("{} ({}): {}", p.kind, p.level, p.purpose)];
    for (k, s) in own {
        let about = s.about.as_ref().map_or(String::new(), |a| format!("; {a}"));
        lines.push(format!("  {k}: {}{about}", spec_text(s)));
    }
    for (tag, s) in &p.items {
        let fields: Vec<String> = s.fields.iter().map(|(k, f)| format!("{k}{}", if f.required { "!" } else { "" })).collect();
        let on_press = if s.events.is_some() { " onPress" } else { "" };
        let row = if s.row { format!(" (item and index read the row{})", if s.row_when { ", when too" } else { "" }) } else { String::new() };
        lines.push(format!("  <{tag}> {} when{on_press}{row}", fields.join(" ")));
    }
    let events = if p.events.is_empty() { "none".to_string() } else { p.events.iter().map(|e| on_event(e)).collect::<Vec<_>>().join(", ") };
    let text_prop = p.text_prop.map_or(String::new(), |t| format!("; a text child fills {t}"));
    let holds = if p.holds_children { "; holds children" } else { "" };
    lines.push(format!("events: {events}{text_prop}{holds}"));
    if !sketched.is_empty() {
        lines.push(format!("sketch: {sketched}"));
    }
    lines.push(format!("example: {}", p.example));
    lines.join("\n")
}

fn source_entry(s: &SlateSourceModule) -> String {
    let mut paths: Vec<String> = Vec::new();
    let mut scalars: IndexMap<String, Vec<String>> = IndexMap::new();
    let mut walk = |prefix: String, shape: &SlateShape| {
        if is_scalar(shape) {
            scalars.entry(slate_shape_text(shape)).or_default().push(prefix);
        } else {
            paths.push(format!("  {prefix}: {}", slate_shape_text(shape)));
        }
    };
    match &s.shape {
        SlateShape::Keyed(inner) => walk(format!("{}.<server>", s.name), inner),
        SlateShape::Fields(fields) => {
            for (k, v) in fields {
                walk(format!("{}.{k}", s.name), v);
            }
        }
        _ => {}
    }
    for (kind, list) in scalars {
        paths.insert(0, format!("  {}: {kind}", list.join(" ")));
    }
    let mut lines = vec![format!("{} ({}): {}", s.name, s.level, s.purpose)];
    lines.extend(paths);
    lines.push(format!("update: {}; scope: {}; cost: {}", s.update, s.scope, s.cost));
    lines.extend(s.notes.iter().map(|(k, v)| format!("note {k}: {v}")));
    lines.push(format!("example: {{{}}}", s.example));
    lines.join("\n")
}

fn runs_entry() -> String {
    let every = format!(
        "every={{60}}: starts it again every 60 seconds, at least {}, while the Slate tab is on screen; at once when it comes back.",
        SLATE_LIMITS.timer_floor_s
    );
    let stream = format!("stream: fills lines as it prints, the last {}, so an output piece shows them live.", SLATE_LIMITS.stream_lines);
    let timeout = format!("timeout={{60}}: seconds before it is stopped, at most {}.", SLATE_LIMITS.timeout_max_s);
    let state = format!("$run reads {}; state is idle (never started) held running done failed cancelled.", SLATE_RUN_FIELDS.join(" "));
    [
        "runs: a command the slate starts with no turn of yours.",
        r#"<run name="check" cmd='gh api "repos/$REPO/issues/$ID"' env={{ REPO: $repo, ID: $id }} every={60} />"#,
        r#"<run name="py">{`python3 -c "print('both quotes')"`}</run>"#,
        "Each attribute:",
        "name: the run is read as $name.",
        "cmd: the literal command, run by bash -c in the thread's folder. Single quotes outside double ones; the block form above takes any text.",
        "env={{ ID: $id }}: values it reads as $ID. args={[$a]}: as $1. stdin={$x}: on standard input. A secret goes only in env or stdin; one in a file, the command reads itself, never through you.",
        every.as_str(),
        "always: with every, ticks while the tab is not on screen too.",
        "once: a start while it still runs is skipped; without once it stops and starts again.",
        stream.as_str(),
        timeout.as_str(),
        r#"on="host": runs on the host's computer, not the thread's."#,
        r#"confirm="Stop it?": asks the person every start; a formula works too."#,
        r#"then='python3 x.py': pipes its raw result to that command; its stdout as JSON becomes json, out stays raw."#,
        r#"tool="server.tool" args={{ id: $id }}, or resource="server:uri": calls an MCP tool instead; slate_catalog <server> lists its tools."#,
        r#"description="Reads the last hour of traffic": one plain line the person reads before allowing a tool run; required on a tool not marked read-only."#,
        r#"start($run) in a handler or <when> starts one. The person allows each command once, on the slate, when it first wants to start: at the write for an every= run, at the first press or <when> for the rest. Until then it reads held; allowing it starts it at once. slate_state start runs only what they allowed "Always in this thread"."#,
        state.as_str(),
        r#"json is out parsed as JSON. Before a first result or after a failure, out and json are empty and len of them 0: show a figure with when={$run.exit == 0}. stale: the command changed since this result. Output is scrubbed of secrets."#,
        "<value> state and each run's last result outlive an app or host restart.",
        r#"Chain: <when done={$check} do={set($ok, $check.exit == 0)} />; done fires on any outcome."#,
        r#"Code only the slate uses goes in <file name="x.py">, never in the project, and runs as $SLATE_DIR/x.py; project code runs where it is."#,
        "A tool's json is its structured result, else its text parsed; out is its text; one marked text only keeps json empty unless its text is JSON. A tool run asks the person where this thread's agent would: never at bypass, else once per new code (tool, args or then). A secret in args returns as [secret:name].",
    ]
    .join("\n")
}

fn functions_entry() -> String {
    const SHOWN: &[&str] = &["percent", "pct", "tokens", "usd", "duration", "ago", "until", "date", "plural", "word", "short", "num", "json", "contains", "orElse", "len", "first", "pluck"];
    let mut lines = vec![String::from(
        r#"Functions (pure; a null argument gives null unless the function takes one). Times read in this computer's zone: date(t) "Mon 5 Oct", time(t) "14:05", ago(t) "30s ago", until(t) "in 4m"."#,
    )];
    lines.extend(SLATE_FUNCTIONS.iter().map(|(n, f)| if SHOWN.contains(n) { format!("{}: {}", f.sig, f.example) } else { f.sig.to_string() }));
    lines.join("\n")
}

fn steps_entry() -> String {
    let mut lines = vec![String::from("List steps, after | (item and index read the element):")];
    lines.extend(SLATE_PIPE_STEPS.iter().map(|(_, s)| format!("{}: {}. {}", s.sig, s.purpose, s.example)));
    lines.join("\n")
}

fn handlers_entry() -> String {
    let mut lines = vec![format!("Handler steps, in onPress, onSubmit, onChange and <when do>; one step or [a, b], at most {}:", SLATE_LIMITS.steps_per_reaction)];
    lines.extend(SLATE_STEPS.iter().map(|s| {
        let window = if s.runs == "window" { "Window only, on a press. " } else { "" };
        format!("{}: {}. {window}Consent: {}. {}", s.sig, s.purpose, s.consent, s.example)
    }));
    lines.join("\n")
}

fn index_text() -> String {
    let mut lines: Vec<String> = vec![String::from("Slate kit wsp/2, JSX-like text. Pieces (attributes, ! required, (a|b) the only values; <items>):")];
    lines.extend(SLATE_PIECES.values().filter(|p| p.level == "core").map(piece_line));
    lines.push(format!(
        "Every piece: id, when={{cond}}; items take when too. tone: default muted good warning bad info accent. emphasis: normal strong quiet. format, where a line lists none: {}.",
        SLATE_FIGURES.join(" ")
    ));
    lines.push(String::from(
        r#"section, column, grid: pad none tight normal loose; surface="inset" sets the card ground; align start center end, else children fill the width."#,
    ));
    lines.push(String::from(
        "bars compare categories; time is a chart, x in ms or ISO; shares a donut, sizes a treemap, spans on a clock a timeline; a flow is a diagram; a screenshot or plot is an image from its path on the thread's computer, or an https address the person allows.",
    ));
    lines.push(String::from("Sources, read only:"));
    lines.extend(SLATE_SOURCES.values().filter(|s| s.level == "core").map(source_line));
    lines.push(String::from(
        "Declarations: <value name start> <secret name> <derived name value> <run name cmd env args stdin on timeout every always once confirm then tool resource description> <file name> <when change={$path} or done={$run} do={steps}>",
    ));
    let steps: Vec<&str> = SLATE_STEPS.iter().map(|s| s.kind).collect();
    let functions: Vec<&str> = SLATE_FUNCTIONS.iter().map(|(n, _)| *n).collect();
    let pipes: Vec<&str> = SLATE_PIPE_STEPS.iter().map(|(n, _)| *n).collect();
    lines.push(format!(
        r#"Steps: {}. Functions: {}. ago(t) "30s ago", until(t) "in 4m"."#,
        steps.join(" "),
        functions.join(" ")
    ));
    lines.push(format!("After |: {}. $run reads state exit out err json; a secret only .set .len.", pipes.join(" ")));
    lines.push(String::from("Rules:"));
    lines.extend(SLATE_RULES.iter().enumerate().map(|(i, r)| format!("{}. {r}", i + 1)));
    lines.push(SLATE_INDEX_EXAMPLE.to_string());
    lines.push(String::from("More: slate_catalog <piece, source, runs, patch, functions, steps, handlers or examples>."));
    lines.join("\n")
}

fn entry(name: &str) -> String {
    let n = js_trim(name);
    if n == "file" {
        return String::from(r#"The <file name="x.py"> declaration, code a run calls as $SLATE_DIR/x.py, is in slate_catalog runs."#);
    }
    if let Some(p) = SLATE_PIECES.get(n) {
        return piece_entry(p);
    }
    if let Some(s) = SLATE_SOURCES.get(n) {
        return source_entry(s);
    }
    match n {
        "runs" | "run" | "tool" | "tools" | "mcp" => runs_entry(),
        "functions" => functions_entry(),
        "patch" | "patches" => PATCH.to_string(),
        "steps" | "pipes" | "pipe" => steps_entry(),
        "handlers" => handlers_entry(),
        "examples" => SLATE_EXAMPLES.iter().map(|e| format!("{}:\n{}", e.title, e.text)).collect::<Vec<_>>().join("\n\n"),
        _ => {
            let options: Vec<String> = SLATE_PIECES.keys().chain(SLATE_SOURCES.keys()).map(|k| k.to_string()).chain(SLATE_CHAPTERS.iter().map(|c| c.to_string())).collect();
            let tail = match slate_nearest(n, &options) {
                Some(fix) => format!("; did you mean {fix}?"),
                None => ".".to_string(),
            };
            format!(
                "{n} is not in the catalog{tail} Ask for a piece, a source, runs, patch, functions, steps, handlers or examples; a server's tools are the host's to answer. The whole index:\n\n{}",
                index_text()
            )
        }
    }
}

/// The catalog: the index with no name, else the named piece, source or chapter, as text.
pub fn slate_catalog(name: Option<&str>) -> String {
    match name {
        None | Some("") | Some("index") => index_text(),
        Some(name) => {
            let text = entry(name);
            // A named entry adds detail only; agents that read one alone guessed pieces like <h1> and <p>.
            if text.starts_with(&format!("{} {NOT_IN}", js_trim(name))) {
                text
            } else {
                format!("{text}\n{BACK}")
            }
        }
    }
}
