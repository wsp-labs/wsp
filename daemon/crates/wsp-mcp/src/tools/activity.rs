// SPDX-License-Identifier: AGPL-3.0-only
//! `activity` and `activity_end`: this computer's processes by the thread that started them, and the End of one by its
//! pid and start, as the host answers them; and the lines an End's answer is said in, which settle and delete say too.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use serde_json::{Map, Value};

use super::named::thread_of;
use super::said::{fmt_bytes, js_number, thread_word, to_fixed, turns};
use super::{entry_in, input, Answer, Refused, Tool};
use crate::host::Host;
use crate::record::fill;
use crate::zod::{self, Schema};

const READ_NAME: &str = "activity";
const END_NAME: &str = "activity_end";

const READ_LISTED: &str = include_str!(concat!(env!("OUT_DIR"), "/record/tools/activity.json"));
const END_LISTED: &str = include_str!(concat!(env!("OUT_DIR"), "/record/tools/activity_end.json"));

pub const READ: Tool = Tool { name: READ_NAME, listed: READ_LISTED, call: |host, args| Box::pin(read(host, args)) };
pub const END: Tool = Tool { name: END_NAME, listed: END_LISTED, call: |host, args| Box::pin(end(host, args)) };

/// Activity's sentences, as record/turns.json holds them.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityWords {
    pub none: String,
    pub wsp: String,
    pub unowned: String,
    pub ended: String,
    pub ended_under: String,
    pub forced: String,
    pub left: String,
    pub freed: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct ReadIn {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computer: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct ReadOut {
    pub computer: String,
    #[cfg_attr(test, schemars(with = "f64"))]
    pub at: Box<RawValue>,
    #[cfg_attr(test, schemars(with = "Vec<serde_json::Value>"))]
    pub threads: Box<RawValue>,
    #[cfg_attr(test, schemars(with = "Vec<serde_json::Value>"))]
    pub wsp: Box<RawValue>,
    #[cfg_attr(test, schemars(with = "Vec<serde_json::Value>"))]
    pub unowned: Box<RawValue>,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct EndIn {
    pub pid: u64,
    pub started: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computer: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct EndOut {
    pub pid: i64,
    #[cfg_attr(test, schemars(with = "Vec<serde_json::Value>"))]
    pub ended: Box<RawValue>,
    pub forced: Vec<i64>,
    pub left: Vec<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freed: Option<String>,
}

/// One process as a line reads it.
#[derive(Deserialize)]
struct Proc {
    pid: f64,
    depth: f64,
    kind: String,
    name: String,
    command: String,
    memory: f64,
    cpu: f64,
    ports: Vec<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Thread {
    thread_id: String,
    title: String,
    memory: f64,
    cpu: f64,
    processes: Vec<Proc>,
}

#[derive(Deserialize)]
struct View {
    computer: String,
    threads: Vec<Thread>,
    wsp: Vec<Proc>,
    unowned: Vec<Proc>,
}

/// An End's answer as its lines read it.
#[derive(Deserialize)]
pub struct Ended {
    pid: f64,
    ended: Vec<Proc>,
    forced: Vec<f64>,
    left: Vec<f64>,
    #[serde(default)]
    freed: Option<String>,
}

fn cpu_word(cpu: f64) -> String {
    format!("{}%", to_fixed(cpu, 1))
}

/// A row as packages/host/src/verbs/activity.ts lays it out: indented by its depth, then pid, kind, command, ports,
/// memory and CPU.
fn proc_line(p: &Proc) -> String {
    let ports = if p.ports.is_empty() {
        String::new()
    } else {
        format!("  :{}", p.ports.iter().map(|n| js_number(*n)).collect::<Vec<_>>().join(" :"))
    };
    format!(
        "{}{}  {}  {}{}  {}  {}",
        "  ".repeat(p.depth as usize + 1),
        js_number(p.pid),
        p.kind,
        p.command,
        ports,
        fmt_bytes(p.memory),
        cpu_word(p.cpu)
    )
}

fn activity_lines(view: &View) -> String {
    let words = &turns().activity;
    let mut lines = Vec::new();
    for t in &view.threads {
        lines.push(format!("thread {} {}  {}  {}", thread_word(&t.thread_id), t.title, fmt_bytes(t.memory), cpu_word(t.cpu)));
        lines.extend(t.processes.iter().map(proc_line));
    }
    for (heading, procs) in [(&words.wsp, &view.wsp), (&words.unowned, &view.unowned)] {
        if !procs.is_empty() {
            lines.push(heading.clone());
            lines.extend(procs.iter().map(proc_line));
        }
    }
    if lines.is_empty() {
        fill(&words.none, &[("computer", &view.computer)])
    } else {
        lines.join("\n")
    }
}

pub fn ended_lines(e: &Ended) -> String {
    let words = &turns().activity;
    if let Some(thread) = &e.freed {
        return fill(&words.freed, &[("thread", &thread_word(thread))]);
    }
    let pids = |list: &[f64]| list.iter().map(|n| js_number(*n)).collect::<Vec<_>>().join(" ");
    let mut lines = vec![match e.ended.split_first() {
        None => format!("ended {}", js_number(e.pid)),
        Some((first, under)) => {
            let pid = js_number(first.pid);
            if under.is_empty() {
                fill(&words.ended, &[("pid", &pid), ("name", &first.name)])
            } else {
                fill(&words.ended_under, &[("pid", &pid), ("name", &first.name), ("count", &under.len().to_string())])
            }
        }
    }];
    if !e.forced.is_empty() {
        lines.push(fill(&words.forced, &[("pids", &pids(&e.forced))]));
    }
    if !e.left.is_empty() {
        lines.push(fill(&words.left, &[("pids", &pids(&e.left))]));
    }
    lines.join("\n")
}

/// The Ends a settle or a delete answered: the field as the TypeScript tool's zod parse leaves it, and each End's lines.
pub struct EndedField {
    pub parsed: Box<RawValue>,
    pub lines: Vec<String>,
}

/// A field of a host's answer read off the tool's recorded output; nothing where the host answered none.
pub fn ended_of(listed: &str, cloud: bool, field: &str, given: Option<Box<RawValue>>) -> Result<Option<EndedField>, Refused> {
    let Some(given) = given else { return Ok(None) };
    let root = Schema::output_of(entry_in(listed, cloud).unwrap_or_default());
    let parsed = root.field(field).and_then(|schema| zod::parsed(schema, &root, &given)).ok_or_else(|| super::agents::unread(END_NAME))?;
    let each: Vec<Ended> = serde_json::from_str(parsed.get()).map_err(|_| super::agents::unread(END_NAME))?;
    Ok(Some(EndedField { parsed, lines: each.iter().map(ended_lines).collect() }))
}

async fn read(host: Arc<Host>, arguments: Value) -> Result<Answer, Refused> {
    let ReadIn { thread, computer } = input(READ_NAME, arguments)?;
    let client = host.client().await?;
    let thread = match thread {
        Some(named) => Some(thread_of(&client, &named).await?.id),
        None => None,
    };
    let mut asked = Map::new();
    if let Some(computer) = computer {
        asked.insert("computer".to_owned(), Value::from(computer));
    }
    if let Some(thread) = thread {
        asked.insert("thread".to_owned(), Value::from(thread));
    }
    #[derive(Deserialize)]
    struct Read {
        activity: Box<RawValue>,
    }
    let Read { activity } = client.request("activity.read", asked).await?;
    let root = Schema::output_of(entry_in(READ_LISTED, host.cloud()).unwrap_or_default());
    let parsed = zod::parsed(&root, &root, &activity).ok_or_else(|| super::agents::unread(READ_NAME))?;
    let view: View = serde_json::from_str(parsed.get()).map_err(|_| super::agents::unread(READ_NAME))?;
    let out: ReadOut = serde_json::from_str(parsed.get()).map_err(|_| super::agents::unread(READ_NAME))?;
    Ok(Answer::text(activity_lines(&view), &out))
}

async fn end(host: Arc<Host>, arguments: Value) -> Result<Answer, Refused> {
    let EndIn { pid, started, computer } = input(END_NAME, arguments)?;
    let client = host.client().await?;
    let mut asked = Map::new();
    asked.insert("pid".to_owned(), Value::from(pid));
    // A whole start goes as JavaScript writes it, with no point.
    asked.insert(
        "startedAt".to_owned(),
        if started.fract() == 0.0 && started.abs() < 9e15 { Value::from(started as i64) } else { Value::from(started) },
    );
    if let Some(computer) = computer {
        asked.insert("computer".to_owned(), Value::from(computer));
    }
    let answer: Box<RawValue> = client.request("activity.end", asked).await?;
    let root = Schema::output_of(entry_in(END_LISTED, host.cloud()).unwrap_or_default());
    let parsed = zod::parsed(&root, &root, &answer).ok_or_else(|| super::agents::unread(END_NAME))?;
    let ended: Ended = serde_json::from_str(parsed.get()).map_err(|_| super::agents::unread(END_NAME))?;
    let out: EndOut = serde_json::from_str(parsed.get()).map_err(|_| super::agents::unread(END_NAME))?;
    Ok(Answer::text(ended_lines(&ended), &out))
}

#[cfg(test)]
mod tests {
    #[test]
    fn its_structs_are_the_recorded_schemas() {
        super::super::held::to_the_record::<super::ReadIn, super::ReadOut>(super::READ.listed);
        super::super::held::to_the_record::<super::EndIn, super::EndOut>(super::END.listed);
    }
}
