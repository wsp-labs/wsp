// SPDX-License-Identifier: AGPL-3.0-only
//! A thread as Markdown, the words the app's Copy as Markdown gives: a port of `threadMessages` and `threadMarkdown`
//! in wsp's `packages/protocol/src/thread-read.ts`, with every line they draw a row's words from.

use serde_json::{Map, Value};

use crate::screens::app_shell::title::{js_space, title_line};

/// The thread as Markdown off the host's `sessions.history {whole:true}` events, each clock written at the zone
/// `utc_offset_ms` east of UTC.
pub fn thread_markdown(events: &[Value], thread_id: &str, utc_offset_ms: i64) -> String {
    markdown(&thread_messages(events, thread_id), utc_offset_ms)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Voice {
    Person,
    Agent,
    Tool,
    Turn,
}

#[derive(Debug)]
struct Message {
    who: Voice,
    at: Option<f64>,
    text: String,
}

const NO_RESULT_LINE: &str = "turn ended without a result";

fn str_at<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn num_at(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn is_true(value: &Value, key: &str) -> bool {
    value.get(key) == Some(&Value::Bool(true))
}

/// The fields of a turn's result that its end row reads.
struct TurnResult<'a> {
    status: &'a str,
    duration_ms: Option<f64>,
    waited_ms: Option<f64>,
    cost_usd: Option<f64>,
    tokens: Option<&'a Value>,
    model: Option<&'a str>,
    text: Option<&'a str>,
    error: Option<&'a str>,
}

impl<'a> TurnResult<'a> {
    fn read(result: &'a Value) -> Self {
        TurnResult {
            status: str_at(result, "status").unwrap_or(""),
            duration_ms: num_at(result, "durationMs"),
            waited_ms: num_at(result, "waitedMs"),
            cost_usd: num_at(result, "costUsd"),
            tokens: result.get("tokens").filter(|tokens| !tokens.is_null()),
            model: str_at(result, "model"),
            text: str_at(result, "text"),
            error: str_at(result, "error"),
        }
    }

    fn failed(error: &'a str) -> Self {
        TurnResult {
            status: "failed",
            duration_ms: None,
            waited_ms: None,
            cost_usd: None,
            tokens: None,
            model: None,
            text: None,
            error: Some(error),
        }
    }
}

/// The open agent row text deltas append to, and the harness message it is a piece of.
struct Open {
    row: usize,
    message_id: Option<String>,
}

/// One tool call of the running turn, its input gathered from every piece.
struct Call {
    id: String,
    row: usize,
    name: String,
    input: String,
}

#[derive(Default)]
struct Reader {
    rows: Vec<Message>,
    open: Option<Open>,
    calls: Vec<Call>,
    saw_text: bool,
    replied: bool,
    steps_row: Option<usize>,
    plan_row: Option<usize>,
}

impl Reader {
    fn say(&mut self, who: Voice, at: Option<f64>, text: String) -> usize {
        self.open = None;
        self.rows.push(Message { who, at, text });
        self.rows.len() - 1
    }

    fn turn(&mut self, at: Option<f64>, result: &TurnResult) {
        if let Some(text) = result.text.filter(|text| !text.is_empty()) {
            if !self.saw_text {
                self.say(Voice::Agent, at, text.to_string());
            }
        }
        self.say(Voice::Turn, at, turn_end_line(result));
        self.calls.clear();
        self.saw_text = false;
    }

    fn delta(&mut self, event: &Value, at: Option<f64>) {
        let text = str_at(event, "text").unwrap_or("");
        let message_id = str_at(event, "messageId");
        let tool_use_id = str_at(event, "toolUseId");
        let tool_name = str_at(event, "toolName");
        match str_at(event, "kind") {
            Some("text") => {
                if text.is_empty() {
                    return;
                }
                if let (Some(open), Some(id)) = (&self.open, message_id) {
                    if open.message_id.as_deref().is_some_and(|open_id| open_id != id) {
                        self.open = None;
                    }
                }
                match &self.open {
                    Some(open) => self.rows[open.row].text.push_str(text),
                    None => {
                        self.rows.push(Message { who: Voice::Agent, at, text: text.to_string() });
                        self.open = Some(Open { row: self.rows.len() - 1, message_id: message_id.map(str::to_string) });
                    }
                }
                self.saw_text = true;
            }
            Some("tool_result") => {
                let answered = tool_use_id.and_then(|id| self.calls.iter().find(|call| call.id == id));
                if let Some(call) = answered {
                    if !is_true(event, "isError") {
                        if let Some(did) = tool_done_line(Some(&call.name), &call.input) {
                            self.rows[call.row].text = did;
                        }
                    }
                }
            }
            Some("tool_use") => {
                self.open = None;
                let known = tool_use_id.and_then(|id| self.calls.iter().position(|call| call.id == id));
                if let Some(index) = known {
                    let call = &mut self.calls[index];
                    if let Some(name) = tool_name {
                        call.name = name.to_string();
                    }
                    call.input.push_str(text);
                    self.rows[call.row].text = tool_activity_line(Some(&call.name), &call.input);
                } else {
                    let row = self.say(Voice::Tool, at, tool_activity_line(tool_name, text));
                    if let Some(id) = tool_use_id {
                        self.calls.push(Call {
                            id: id.to_string(),
                            row,
                            name: tool_name.unwrap_or("tool").to_string(),
                            input: text.to_string(),
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

fn thread_messages(events: &[Value], thread_id: &str) -> Vec<Message> {
    let mut r = Reader::default();
    for event in events {
        if str_at(event, "threadId") != Some(thread_id) {
            continue;
        }
        let at = num_at(event, "at");
        let text = || str_at(event, "text").unwrap_or("").to_string();
        match str_at(event, "type").unwrap_or("") {
            "session.start" => {
                r.open = None;
                r.calls.clear();
                r.saw_text = false;
                r.replied = false;
                r.steps_row = None;
                r.plan_row = None;
                if let Some(prompt) = str_at(event, "prompt") {
                    r.say(Voice::Person, at, prompt.to_string());
                }
            }
            "session.steer" => {
                r.say(Voice::Person, at, str_at(event, "prompt").unwrap_or("").to_string());
            }
            "session.moved" => {
                let line =
                    thread_moved_line(str_at(event, "from").unwrap_or(""), str_at(event, "to").unwrap_or(""), is_true(event, "fresh"));
                r.say(Voice::Turn, at, line);
            }
            "session.behind" | "session.notice" => {
                r.say(Voice::Turn, at, text());
            }
            "thread.waited" => {
                let line = str_at(event, "said").map_or_else(|| waited_line(num_at(event, "ms").unwrap_or(0.0)), str::to_string);
                r.say(Voice::Turn, at, line);
            }
            "session.earlier" => {
                let who = match str_at(event, "who") {
                    Some("person") => Voice::Person,
                    Some("agent") => Voice::Agent,
                    Some("note") => Voice::Turn,
                    _ => Voice::Tool,
                };
                r.say(who, at, text());
            }
            "session.delta" => r.delta(event, at),
            "session.done" => {
                r.replied = true;
                let result = event.get("result").unwrap_or(&Value::Null);
                r.turn(at, &TurnResult::read(result));
            }
            "session.end" => {
                let reason = str_at(event, "reason").unwrap_or(NO_RESULT_LINE);
                if is_true(event, "unstarted") {
                    if let Some(prompt) = str_at(event, "prompt") {
                        r.say(Voice::Person, at, prompt.to_string());
                    }
                    r.turn(at, &TurnResult::failed(reason));
                } else if !r.replied {
                    r.turn(at, &TurnResult::failed(reason));
                }
            }
            "session.compacted" => {
                r.say(Voice::Tool, at, compacted_line(num_at(event, "before"), num_at(event, "after")));
            }
            "session.plan" => {
                if let Some(steps) = event.get("steps").and_then(Value::as_array) {
                    let line = plan_steps_line(steps);
                    match r.steps_row {
                        Some(row) => r.rows[row].text = line,
                        None => r.steps_row = Some(r.say(Voice::Tool, at, line)),
                    }
                }
                if let Some(plan) = str_at(event, "text") {
                    match r.plan_row {
                        Some(row) => r.rows[row].text = plan.to_string(),
                        None => r.plan_row = Some(r.say(Voice::Agent, at, plan.to_string())),
                    }
                }
            }
            "session.changes" => {
                let row = Message { who: Voice::Tool, at, text: turn_changes_line(event) };
                if r.rows.last().is_some_and(|last| last.who == Voice::Turn) {
                    let end = r.rows.len() - 1;
                    r.rows.insert(end, row);
                } else {
                    r.rows.push(row);
                }
                r.open = None;
            }
            _ => {}
        }
    }
    r.rows
}

fn markdown(rows: &[Message], utc_offset_ms: i64) -> String {
    let mut blocks: Vec<Vec<String>> = Vec::new();
    let mut speaker: Option<Voice> = None;
    let lines = |text: &str| text.split('\n').map(str::to_string).collect::<Vec<_>>();
    for (index, row) in rows.iter().enumerate() {
        match row.who {
            Voice::Person => {
                let clock = fmt_clock(row.at, utc_offset_ms);
                blocks.push(vec![if clock.is_empty() { "**You**".to_string() } else { format!("**You** {clock}") }]);
                blocks.push(lines(&row.text));
                speaker = Some(Voice::Person);
                continue;
            }
            Voice::Turn => {
                blocks.push(row.text.split('\n').map(|line| if line.is_empty() { String::new() } else { format!("*{line}*") }).collect());
                speaker = Some(Voice::Turn);
                continue;
            }
            Voice::Agent | Voice::Tool => {}
        }
        if speaker != Some(Voice::Agent) {
            blocks.push(vec!["**Agent**".to_string()]);
        }
        speaker = Some(Voice::Agent);
        if row.who == Voice::Agent {
            blocks.push(lines(&row.text));
            continue;
        }
        let mut parts = row.text.split('\n');
        let mut item = vec![format!("- {}", parts.next().unwrap_or(""))];
        item.extend(parts.map(|line| format!("  {line}")));
        let after_tool = index > 0 && rows[index - 1].who == Voice::Tool;
        match blocks.last_mut() {
            Some(last) if after_tool => last.extend(item),
            _ => blocks.push(item),
        }
    }
    blocks.iter().map(|block| block.join("\n")).collect::<Vec<_>>().join("\n\n")
}

/// `new Date(at).toTimeString().slice(0, 8)` read at a fixed offset from UTC.
fn fmt_clock(at: Option<f64>, utc_offset_ms: i64) -> String {
    let Some(at) = at else { return String::new() };
    if at.is_nan() || at.abs() > 8.64e15 {
        return "Invalid ".to_string();
    }
    let local = at.trunc() as i64 + utc_offset_ms;
    let seconds = local.div_euclid(1_000).rem_euclid(86_400);
    format!("{:02}:{:02}:{:02}", seconds / 3_600, seconds % 3_600 / 60, seconds % 60)
}

fn plural(n: usize, noun: &str) -> String {
    format!("{n} {noun}{}", if n == 1 { "" } else { "s" })
}

/// A number as JavaScript's String() writes the ones these lines carry.
fn js_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e21 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// `x.toFixed(digits)` for a positive x: correctly rounded off the exact binary value, a value exactly halfway
/// rounded up.
fn to_fixed(x: f64, digits: i32) -> String {
    let scale = 2f64.powi(digits + 1);
    let scaled = x * scale;
    // A tie at `digits` places is odd / 2^(digits + 1) exactly, and the value above it prints with no rounding.
    let tie = scaled.fract() == 0.0 && scaled % 2.0 == 1.0;
    let rounded = if tie { (scaled + 1.0) / scale } else { x };
    format!("{rounded:.0$}", digits as usize)
}

/// `toLocaleString("en-US")` at two places: Intl rounds the shortest decimal that reads back as the number, halves
/// away from zero, where `toFixed` rounds the exact binary value.
fn fmt_cost(usd: f64) -> String {
    let shortest = format!("{}", usd.abs());
    let (whole, fraction) = shortest.split_once('.').unwrap_or((&shortest, ""));
    let mut digits: Vec<u8> = format!("{whole}{:.3}", format!("{fraction:0<3}")).into_bytes();
    let up = digits.pop().is_some_and(|next| next >= b'5');
    digits.truncate(whole.len() + 2);
    if up {
        let mut carry = true;
        for digit in digits.iter_mut().rev() {
            if !carry {
                break;
            }
            carry = *digit == b'9';
            *digit = if carry { b'0' } else { *digit + 1 };
        }
        if carry {
            digits.insert(0, b'1');
        }
    }
    let (whole, cents) = digits.split_at(digits.len() - 2);
    let mut grouped = String::new();
    for (i, digit) in whole.iter().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(char::from(*digit));
    }
    let sign = if usd.is_sign_negative() { "-" } else { "" };
    format!("${sign}{grouped}.{}", String::from_utf8_lossy(cents))
}

fn fmt_tokens(n: f64) -> String {
    let count = if n.is_finite() && n > 0.0 { n.round() } else { 0.0 };
    if count < 1_000.0 {
        return js_number(count);
    }
    let (value, unit) = if count < 999_500.0 {
        (count / 1_000.0, "k")
    } else if count < 999_500_000.0 {
        (count / 1_000_000.0, "M")
    } else {
        (count / 1_000_000_000.0, "B")
    };
    let places = match value {
        v if v < 10.0 => 2,
        v if v < 100.0 => 1,
        _ => 0,
    };
    let fixed = to_fixed(value, places);
    let trimmed = if fixed.contains('.') { fixed.trim_end_matches('0').trim_end_matches('.') } else { &fixed };
    format!("{trimmed}{unit}")
}

fn fmt_duration(ms: f64) -> String {
    if ms < 60_000.0 || !ms.is_finite() {
        if !ms.is_finite() || ms < 0.0 {
            return "0ms".to_string();
        }
        if ms < 1_000.0 {
            return format!("{}ms", js_number(ms.round().max(1.0)));
        }
        if ms < 10_000.0 {
            let tenths = (ms / 100.0).round() as i64;
            return if tenths >= 100 { "10s".to_string() } else { format!("{}.{}s", tenths / 10, tenths % 10) };
        }
        return format!("{}s", js_number((ms / 1_000.0).round()));
    }
    let total = if ms > 0.0 { (ms / 1_000.0).round() as i64 } else { 0 };
    let minutes = total / 60;
    let seconds = total % 60;
    if seconds == 0 {
        format!("{minutes}m")
    } else {
        format!("{minutes}m {seconds}s")
    }
}

fn turn_end_line(result: &TurnResult) -> String {
    let mut parts = vec![result.status.to_string()];
    if let Some(duration) = result.duration_ms {
        let waited = result.waited_ms.filter(|ms| *ms > 0.0).unwrap_or(0.0).min(duration);
        let worked = duration - waited;
        parts.push(format!("Worked for {}", fmt_duration(worked)));
        if waited > worked {
            parts.push(format!("waited on you {}", fmt_duration(waited)));
        }
    }
    if let Some(cost) = result.cost_usd {
        parts.push(fmt_cost(cost));
    }
    if let Some(model) = result.model.filter(|model| !model.is_empty()) {
        parts.push(model.to_string());
    }
    if let Some(tokens) = result.tokens {
        let count = |key| fmt_tokens(num_at(tokens, key).unwrap_or(0.0));
        parts.push(format!("{} in", count("input")));
        parts.push(format!("{} out", count("output")));
    }
    let line = parts.join("  ");
    match result.error.filter(|_| result.status != "completed") {
        Some(failure) => format!("{line}: {failure}"),
        None => line,
    }
}

fn compacted_line(before: Option<f64>, after: Option<f64>) -> String {
    match (before, after) {
        (_, None) => "Compacted the context".to_string(),
        (None, Some(after)) => format!("Compacted the context to {} tokens", fmt_tokens(after)),
        (Some(before), Some(after)) => format!("Compacted the context, {} to {} tokens", fmt_tokens(before), fmt_tokens(after)),
    }
}

fn plan_steps_line(steps: &[Value]) -> String {
    let is = |step: &Value, state: &str| str_at(step, "state") == Some(state);
    let done = steps.iter().filter(|step| is(step, "done")).count();
    let mut lines = vec![format!("Plan: {done} of {} steps done", steps.len())];
    for step in steps {
        let mark = if is(step, "done") { "x" } else { " " };
        let working = if is(step, "working") { " (working)" } else { "" };
        lines.push(format!("- [{mark}] {}{working}", str_at(step, "text").unwrap_or("")));
    }
    lines.join("\n")
}

fn turn_changes_line(event: &Value) -> String {
    let files = event.get("files").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let sum = |key| files.iter().map(|file| num_at(file, key).unwrap_or(0.0)).sum::<f64>();
    let (added, taken) = (js_number(sum("additions")), js_number(sum("deletions")));
    let count = format!("{}, +{added} -{taken}", plural(files.len(), "file"));
    if let Some(others) = event.get("others").and_then(Value::as_array) {
        let own = if files.is_empty() { "No files from this thread's edits".to_string() } else { format!("Changed {count}") };
        return if others.is_empty() { own } else { format!("{own}; {} also changed in this folder", plural(others.len(), "file")) };
    }
    if is_true(event, "shared") {
        format!("{} changed in this folder, +{added} -{taken}; other threads worked in it too", plural(files.len(), "file"))
    } else {
        format!("Changed {count}")
    }
}

fn thread_moved_line(from: &str, to: &str, fresh: bool) -> String {
    format!("its worktree at {from} is gone, so it goes on in {to}{}", if fresh { " in a fresh session of its agent" } else { "" })
}

fn waited_line(ms: f64) -> String {
    let seconds = (ms / 1_000.0).round();
    if seconds < 60.0 {
        format!("Waited {} s for your computer", js_number(seconds))
    } else {
        format!("Waited {} min for your computer", js_number((ms / 60_000.0).round()))
    }
}

enum ToolRow {
    Shell,
    Path { doing: &'static str, did: &'static str, field: &'static str },
    About { doing: &'static str, did: Option<&'static str>, field: &'static str },
    Change,
    Question,
}

fn tool_row(name: &str) -> Option<ToolRow> {
    let searched = ToolRow::About { doing: "searching code for", did: Some("searched code for"), field: "pattern" };
    let web = ToolRow::About { doing: "searching the web for", did: Some("searched the web for"), field: "query" };
    Some(match name {
        "Bash" | "command_execution" => ToolRow::Shell,
        "Read" => ToolRow::Path { doing: "reading", did: "read", field: "file_path" },
        "Write" => ToolRow::Path { doing: "writing", did: "wrote", field: "file_path" },
        "Edit" | "MultiEdit" => ToolRow::Path { doing: "editing", did: "edited", field: "file_path" },
        "NotebookEdit" => ToolRow::Path { doing: "editing", did: "edited", field: "notebook_path" },
        "file_change" => ToolRow::Change,
        "Grep" | "Glob" => searched,
        "WebSearch" | "web_search" => web,
        "WebFetch" => ToolRow::About { doing: "fetching", did: Some("fetched"), field: "url" },
        "Task" | "Agent" => ToolRow::About { doing: "agent:", did: None, field: "description" },
        "spawn_agent" => ToolRow::About { doing: "agent:", did: None, field: "prompt" },
        "AskUserQuestion" => ToolRow::Question,
        _ => return None,
    })
}

/// A string field with something other than whitespace in it, as written.
fn tool_field<'a>(input: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    input.get(name).and_then(Value::as_str).filter(|value| !value.trim_matches(js_space).is_empty())
}

fn tool_line(row: &ToolRow, input: &Map<String, Value>, done: bool) -> Option<String> {
    match row {
        ToolRow::Shell => tool_field(input, "command").map(|command| format!("$ {}", title_line(command))),
        ToolRow::Path { doing, did, field } => tool_field(input, field).map(|path| format!("{} {path}", if done { did } else { doing })),
        ToolRow::About { doing, did, field } => {
            let verb = if done { did.unwrap_or(doing) } else { doing };
            tool_field(input, field).map(|what| format!("{verb} {}", title_line(what)))
        }
        ToolRow::Change => {
            let paths: Vec<&str> = input
                .get("changes")
                .and_then(Value::as_array)
                .map(|changes| changes.iter().filter_map(|change| str_at(change, "path")).filter(|path| !path.is_empty()).collect())
                .unwrap_or_default();
            let verb = if done { "edited" } else { "editing" };
            match paths.as_slice() {
                [] => None,
                [path] => Some(format!("{verb} {path}")),
                _ => Some(format!("{verb} {}", plural(paths.len(), "file"))),
            }
        }
        ToolRow::Question => input
            .get("questions")
            .and_then(Value::as_array)?
            .iter()
            .filter_map(Value::as_object)
            .find_map(|question| tool_field(question, "question"))
            .map(str::to_string),
    }
}

fn tool_input(input: &str) -> Option<Map<String, Value>> {
    match serde_json::from_str(input) {
        Ok(Value::Object(fields)) => Some(fields),
        _ => None,
    }
}

/// `Use wsp's run` for a tool a server lends, named `mcp__<server>__<tool>`.
fn server_tool_words(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.split("__").collect();
    if parts.len() < 3 || parts[0] != "mcp" || parts[1].is_empty() {
        return None;
    }
    Some(format!("Use {}'s {}", parts[1], parts[2..].join("__").replace('_', " ")))
}

fn tool_activity_line(tool_name: Option<&str>, input: &str) -> String {
    let name = tool_name.unwrap_or("tool");
    let Some(row) = tool_row(name) else { return server_tool_words(name).unwrap_or_else(|| name.to_string()) };
    tool_input(input).and_then(|fields| tool_line(&row, &fields, false)).unwrap_or_else(|| name.to_string())
}

fn tool_done_line(tool_name: Option<&str>, input: &str) -> Option<String> {
    let row = tool_row(tool_name.unwrap_or("tool"))?;
    let fields = tool_input(input)?;
    let done = tool_line(&row, &fields, true)?;
    (Some(&done) != tool_line(&row, &fields, false).as_ref()).then_some(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each expected string is what `threadMarkdown(threadMessages(events, "t1"))` in wsp's TypeScript printed for the same
    /// events, run with TZ set to the zone at the given offset.
    fn check(events: &str, utc_offset_ms: i64, expected: &str) {
        let events: Vec<Value> = serde_json::from_str(events).unwrap();
        assert_eq!(thread_markdown(&events, "t1", utc_offset_ms), expected);
    }

    #[test]
    fn text_and_a_shell_call() {
        check(
            r###"[{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":1,"at":1760700000000,"prompt":"Fix the build\n\nIt fails on main."},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"other","pos":2,"at":1760700000010,"kind":"text","text":"Looking at "},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":3,"at":1760700001000,"kind":"text","text":"Let me look "},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":4,"at":1760700001100,"kind":"text","text":"at the build.\nFirst the log."},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":5,"at":1760700001150,"kind":"thinking","text":"hmm"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":6,"at":1760700001200,"kind":"tool_use","text":"{\"command\":\"pnpm  build\\n  --filter app\",\"description\":\"Build\"}","toolName":"Bash","toolUseId":"u1"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":7,"at":1760700001300,"kind":"tool_result","text":"ok","toolUseId":"u1"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":8,"at":1760700002000,"kind":"text","text":"Fixed it."},{"type":"session.done","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":9,"at":1760700083400,"result":{"status":"completed","durationMs":83400,"costUsd":0.125,"model":"claude-opus-5-5","tokens":{"input":4275,"output":999499},"text":"Fixed it."}},{"type":"session.end","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":10,"at":1760700083500,"exitCode":0,"sawResult":true},{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"other","pos":11,"at":1760700100000,"prompt":"thanks"}]"###,
            19800000,
            r###"**You** 16:50:00

Fix the build

It fails on main.

**Agent**

Let me look at the build.
First the log.

- $ pnpm build

Fixed it.

*completed  Worked for 1m 23s  $0.13  claude-opus-5-5  4.28k in  999k out*"###,
        );
    }

    #[test]
    fn tool_words() {
        check(
            r###"[{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":12,"at":1760700000000,"prompt":"Tour the tools"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":13,"at":1760700000001,"kind":"tool_use","text":"{\"file_path\":\"/src/a.rs\"}","toolName":"Read","toolUseId":"a"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":14,"at":1760700000002,"kind":"tool_result","text":"ok","toolUseId":"a"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":15,"at":1760700000003,"kind":"tool_use","text":"{\"file_path\":\"/src/b.rs\",\"old_string\":\"x\",\"new_string\":\"y\"}","toolName":"Edit","toolUseId":"b"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":16,"at":1760700000004,"kind":"tool_result","text":"ok","toolUseId":"b"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":17,"at":1760700000005,"kind":"tool_use","text":"{\"file_path\":\"/src/c.rs\",\"content\":\"\"}","toolName":"Write","toolUseId":"c"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":18,"at":1760700000006,"kind":"tool_result","text":"ok","toolUseId":"c","isError":true},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":19,"at":1760700000007,"kind":"tool_use","text":"{\"pattern\":\"fn  main\\nmore\"}","toolName":"Grep","toolUseId":"d"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":20,"at":1760700000008,"kind":"tool_result","text":"ok","toolUseId":"d"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":21,"at":1760700000009,"kind":"tool_use","text":"{\"pattern\":\"**/*.rs\"}","toolName":"Glob","toolUseId":"e"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":22,"at":1760700000010,"kind":"tool_use","text":"{\"description\":\"Find the bug\",\"prompt\":\"long brief\"}","toolName":"Task","toolUseId":"f"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":23,"at":1760700000011,"kind":"tool_result","text":"ok","toolUseId":"f"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":24,"at":1760700000012,"kind":"tool_use","text":"{\"url\":\"https://example.com\",\"prompt\":\"read\"}","toolName":"WebFetch","toolUseId":"g"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":25,"at":1760700000013,"kind":"tool_result","text":"ok","toolUseId":"g"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":26,"at":1760700000014,"kind":"tool_use","text":"{\"todos\":[]}","toolName":"TodoWrite","toolUseId":"h"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":27,"at":1760700000015,"kind":"tool_result","text":"ok","toolUseId":"h"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":28,"at":1760700000016,"kind":"tool_use","text":"{\"query\":\"rust json\"}","toolName":"WebSearch","toolUseId":"i"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":29,"at":1760700000017,"kind":"tool_result","text":"ok","toolUseId":"i"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":30,"at":1760700000018,"kind":"tool_use","text":"{\"file_path\":\"/m.rs\",\"edits\":[]}","toolName":"MultiEdit","toolUseId":"j"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":31,"at":1760700000019,"kind":"tool_use","text":"{\"notebook_path\":\"/n.ipynb\"}","toolName":"NotebookEdit","toolUseId":"k"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":32,"at":1760700000020,"kind":"tool_result","text":"ok","toolUseId":"k"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":33,"at":1760700000021,"kind":"tool_use","text":"{\"thread\":\"x\"}","toolName":"mcp__wsp__thread_read","toolUseId":"l"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":34,"at":1760700000022,"kind":"tool_result","text":"ok","toolUseId":"l"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":35,"at":1760700000023,"kind":"tool_use","text":"{}"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":36,"at":1760700000024,"kind":"tool_use","text":"{\"file_path\":\"/no/id.rs\"}","toolName":"Read"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":37,"at":1760700000025,"kind":"tool_use","text":"{\"file_pa","toolName":"Read","toolUseId":"m"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":38,"at":1760700000026,"kind":"tool_use","text":"th\": \"/streamed.rs\"}","toolUseId":"m"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":39,"at":1760700000027,"kind":"tool_result","text":"ok","toolUseId":"m"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":40,"at":1760700000028,"kind":"tool_use","text":"{\"changes\":[{\"path\":\"/x.rs\"},{\"path\":\"/y.rs\"},{\"path\":\"\"}]}","toolName":"file_change","toolUseId":"n"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":41,"at":1760700000029,"kind":"tool_result","text":"ok","toolUseId":"n"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":42,"at":1760700000030,"kind":"tool_use","text":"{\"changes\":[{\"path\":\"/only.rs\"}]}","toolName":"file_change","toolUseId":"o"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":43,"at":1760700000031,"kind":"tool_use","text":"{\"questions\":[{\"header\":\"h\",\"options\":[]},{\"question\":\"Which one?\",\"header\":\"Pick\",\"options\":[{\"label\":\"A\"}]}]}","toolName":"AskUserQuestion","toolUseId":"p"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":44,"at":1760700000032,"kind":"tool_result","text":"ok","toolUseId":"p"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":45,"at":1760700000033,"kind":"tool_use","text":"{\"command\":\"   \",\"description\":\"nothing\"}","toolName":"Bash","toolUseId":"q"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":46,"at":1760700000034,"kind":"tool_use","text":"{\"prompt\":\"Write the docs\\nall of them\"}","toolName":"spawn_agent","toolUseId":"r"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":47,"at":1760700000035,"kind":"tool_result","text":"ok","toolUseId":"r"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":48,"at":1760700000036,"kind":"tool_use","text":"not json","toolName":"Read","toolUseId":"s"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":49,"at":1760700000037,"kind":"tool_use","text":"{\"description\":\"  \"}","toolName":"Agent","toolUseId":"t"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":50,"at":1760700000038,"kind":"tool_use","text":"{}","toolName":"mcp____x","toolUseId":"u"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":51,"at":1760700000039,"kind":"tool_use","text":"{\"pattern\":\"a  b\\tc\"}","toolName":"Grep","toolUseId":"v"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":52,"at":1760700000040,"kind":"tool_result","text":"ok","toolUseId":"v"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":53,"at":1760700000041,"kind":"tool_use","text":"[]","toolName":"Read","toolUseId":"w"},{"type":"session.done","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":54,"at":1760700000050,"result":{"status":"completed","text":""}}]"###,
            0,
            r###"**You** 11:20:00

Tour the tools

**Agent**

- read /src/a.rs
- edited /src/b.rs
- writing /src/c.rs
- searched code for fn main
- searching code for **/*.rs
- agent: Find the bug
- fetched https://example.com
- TodoWrite
- searched the web for rust json
- editing /m.rs
- edited /n.ipynb
- Use wsp's thread read
- tool
- reading /no/id.rs
- read /streamed.rs
- edited 2 files
- editing /only.rs
- Which one?
- Bash
- agent: Write the docs
- Read
- Agent
- mcp____x
- searched code for a b c
- Read

*completed*"###,
        );
    }

    #[test]
    fn turn_ends() {
        check(
            r###"[{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":55,"at":1760700000000,"prompt":"one"},{"type":"session.done","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":56,"at":1760700005000,"result":{"status":"failed","durationMs":500,"error":"the model refused\n\nsecond line","text":"partial"}},{"type":"session.end","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":57,"at":1760700005100,"exitCode":0,"sawResult":true,"reason":"should not show"},{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":58,"at":1760700006000,"prompt":"two"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":59,"at":1760700006100,"kind":"text","text":"working"},{"type":"session.done","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":60,"at":1760700007000,"result":{"status":"interrupted","durationMs":300000,"waitedMs":200000,"costUsd":1234567.891,"tokens":null,"model":""}},{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":61,"at":1760700008000,"prompt":"three"},{"type":"session.end","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":62,"at":1760700009000,"exitCode":0,"sawResult":true,"reason":"the machine went away"},{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":63,"at":1760700010000,"prompt":"four"},{"type":"session.end","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":64,"at":1760700011000,"exitCode":0,"sawResult":true},{"type":"session.end","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":65,"at":1760700012000,"exitCode":0,"sawResult":true,"unstarted":true,"prompt":"five never ran","reason":"no computer"},{"type":"session.end","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":66,"at":1760700013000,"exitCode":0,"sawResult":true,"unstarted":true},{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":67,"at":1760700014000},{"type":"session.done","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":68,"at":1760700015000,"result":{"status":"completed","durationMs":1250,"waitedMs":2000,"costUsd":0,"tokens":{"input":999500,"output":12345678},"text":"Reply only in the result"}},{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":69,"at":1760700016000,"prompt":"six"},{"type":"session.done","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":70,"at":1760700017000,"result":{"status":"completed","durationMs":3725000,"costUsd":0.005,"tokens":{"input":0.4,"output":999499999},"error":"ignored when completed"}},{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":71,"at":1760700018000,"prompt":"seven"},{"type":"session.done","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":72,"at":1760700019000,"result":{"status":"completed","durationMs":59600,"waitedMs":0,"tokens":{"input":1000000000000,"output":9995}}},{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":73,"at":1760700020000,"prompt":"eight"},{"type":"session.done","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":74,"at":1760700021000,"result":{"status":"completed","durationMs":9960,"costUsd":0.015}},{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":75,"at":1760700022000,"prompt":"nine"},{"type":"session.done","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":76,"at":1760700023000,"result":{"status":"completed","durationMs":0.3,"costUsd":2.675}},{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":77,"at":1760700022500,"prompt":"nine and a half"},{"type":"session.done","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":78,"at":1760700023500,"result":{"status":"completed","costUsd":-0.001,"tokens":{"input":1005,"output":99950}}},{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":79,"at":1760700023600,"prompt":"nine and three quarters"},{"type":"session.done","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":80,"at":1760700023700,"result":{"status":"completed","costUsd":0.125,"tokens":{"input":8345,"output":1000000000}}},{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":81,"at":1760700024000,"prompt":"ten"},{"type":"session.done","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":82,"at":1760700025000,"result":{"status":"completed","durationMs":3600000,"costUsd":999.995}}]"###,
            0,
            r###"**You** 11:20:00

one

**Agent**

partial

*failed  Worked for 500ms: the model refused*

*second line*

**You** 11:20:06

two

**Agent**

working

*interrupted  Worked for 1m 40s  waited on you 3m 20s  $1,234,567.89*

**You** 11:20:08

three

*failed: the machine went away*

**You** 11:20:10

four

*failed: turn ended without a result*

**You** 11:20:12

five never ran

*failed: no computer*

*failed: turn ended without a result*

**Agent**

Reply only in the result

*completed  Worked for 1ms  waited on you 1.3s  $0.00  1M in  12.3M out*

**You** 11:20:16

six

*completed  Worked for 62m 5s  $0.01  0 in  999M out*

**You** 11:20:18

seven

*completed  Worked for 60s  1000B in  9.99k out*

**You** 11:20:20

eight

*completed  Worked for 10s  $0.02*

**You** 11:20:22

nine

*completed  Worked for 1ms  $2.68*

**You** 11:20:22

nine and a half

*completed  $-0.00  1k in  100k out*

**You** 11:20:23

nine and three quarters

*completed  $0.13  8.35k in  1B out*

**You** 11:20:24

ten

*completed  Worked for 60m  $1,000.00*"###,
        );
    }

    #[test]
    fn steers_plans_and_notes() {
        check(
            r###"[{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":83,"at":1760700000000,"prompt":"Plan it"},{"type":"session.plan","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":84,"at":1760700000100,"steps":[{"text":"Read","state":"done"},{"text":"Write","state":"working"},{"text":"Test","state":"pending"}]},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":85,"at":1760700000200,"kind":"text","text":"On it."},{"type":"session.steer","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":86,"at":1760700000300,"prompt":"also docs"},{"type":"session.plan","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":87,"at":1760700000400,"steps":[{"text":"Read","state":"done"},{"text":"Write","state":"done"}],"text":"## The plan\n\n1. write"},{"type":"session.plan","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":88,"at":1760700000450,"text":"## The plan v2"},{"type":"session.compacted","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":89,"at":1760700000500,"before":180000,"after":42100},{"type":"session.compacted","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":90,"at":1760700000510,"after":900},{"type":"session.compacted","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":91,"at":1760700000520},{"type":"session.notice","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":92,"at":1760700000600,"text":"a notice"},{"type":"session.behind","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":93,"at":1760700000610,"text":"behind main"},{"type":"session.moved","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":94,"at":1760700000620,"from":"/w/tree","to":"/w/proj","fresh":true},{"type":"session.moved","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":95,"at":1760700000630,"from":"/a","to":"/b"},{"type":"thread.waited","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":96,"at":1760700000640,"ms":4000},{"type":"thread.waited","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":97,"at":1760700000650,"ms":125000},{"type":"thread.waited","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":98,"at":1760700000660,"ms":59400,"said":"held a while"},{"type":"thread.waited","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":99,"at":1760700000670,"ms":59600},{"type":"session.done","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":100,"at":1760700000700,"result":{"status":"completed","durationMs":700}},{"type":"session.changes","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":101,"at":1760700000800,"from":"a","to":"b","files":[{"path":"x","additions":3,"deletions":1},{"path":"y","additions":10,"deletions":0}],"moved":[]},{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":102,"at":1760700001000,"prompt":"more"},{"type":"session.changes","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":103,"at":1760700001100,"from":"a","to":"b","files":[],"moved":[],"others":[{"path":"z"}]},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":104,"at":1760700001200,"kind":"text","text":"after changes"},{"type":"session.changes","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":105,"at":1760700001300,"from":"a","to":"b","files":[{"additions":1,"deletions":1}],"moved":[],"others":[]},{"type":"session.changes","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":106,"at":1760700001400,"from":"a","to":"b","files":[{"additions":2,"deletions":5}],"moved":[],"shared":true},{"type":"session.changes","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":107,"at":1760700001500,"from":"a","to":"b","files":[{"additions":2,"deletions":5},{"additions":0,"deletions":0}],"moved":[],"others":[{},{}]},{"type":"session.earlier","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":108,"at":1760700001600,"who":"person","text":"earlier ask"},{"type":"session.earlier","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":109,"at":1760700001700,"who":"agent","text":"earlier reply"},{"type":"session.earlier","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":110,"at":1760700001800,"who":"tool","text":"earlier tool\nsecond line"},{"type":"session.earlier","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":111,"at":1760700001900,"who":"note","text":"earlier note"},{"type":"session.end","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":112,"at":1760700002000,"exitCode":0,"sawResult":true,"reason":"stopped"}]"###,
            -18000000,
            r###"**You** 06:20:00

Plan it

**Agent**

- Plan: 2 of 2 steps done
  - [x] Read
  - [x] Write

On it.

**You** 06:20:00

also docs

**Agent**

## The plan v2

- Compacted the context, 180k to 42.1k tokens
- Compacted the context to 900 tokens
- Compacted the context

*a notice*

*behind main*

*its worktree at /w/tree is gone, so it goes on in /w/proj in a fresh session of its agent*

*its worktree at /a is gone, so it goes on in /b*

*Waited 4 s for your computer*

*Waited 2 min for your computer*

*held a while*

*Waited 1 min for your computer*

**Agent**

- Changed 2 files, +13 -1

*completed  Worked for 700ms*

**You** 06:20:01

more

**Agent**

- No files from this thread's edits; 1 file also changed in this folder

after changes

- Changed 1 file, +1 -1
- 1 file changed in this folder, +2 -5; other threads worked in it too
- Changed 2 files, +2 -5; 2 files also changed in this folder

**You** 06:20:01

earlier ask

**Agent**

earlier reply

- earlier tool
  second line

*earlier note*

*failed: stopped*"###,
        );
    }

    #[test]
    fn message_ids_and_clocks() {
        check(
            r###"[{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":113,"prompt":"no clock here"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":114,"at":1760700000100,"kind":"text","text":"first ","messageId":"m1"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":115,"at":1760700000110,"kind":"text","text":"","messageId":"m2"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":116,"at":1760700000120,"kind":"text","text":"still first","messageId":"m1"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":117,"at":1760700000130,"kind":"text","text":"second\n\nwith a gap","messageId":"m2"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":118,"at":1760700000140,"kind":"text","text":" no id joins"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":119,"at":1760700000150,"kind":"tool_use","text":"{\"file_path\":\"/a\"}","toolName":"Read","toolUseId":"x"},{"type":"session.delta","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":120,"at":1760700000160,"kind":"text","text":"after a call"},{"type":"session.done","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":121,"at":1760700000200,"result":{"status":"completed","text":"ignored since text was seen"}},{"type":"session.start","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":122,"at":1760745599999.9,"prompt":"fractional clock"},{"type":"session.steer","workspaceId":"w1","sessionId":"s1","threadId":"t1","pos":123,"at":-1500,"prompt":"before the epoch"},{"type":"session.start","workspaceId":"w1","sessionId":"s1","prompt":"no thread id"}]"###,
            19800000,
            r###"**You**

no clock here

**Agent**

first still first

second

with a gap no id joins

- reading /a

after a call

*completed*

**You** 05:29:59

fractional clock

**You** 05:29:58

before the epoch"###,
        );
    }

    #[test]
    fn no_events() {
        check(r###"[]"###, 0, r###""###);
    }
}
