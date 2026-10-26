// SPDX-License-Identifier: AGPL-3.0-only
//! `runtime/src/slate-sources/`: the host sources a slate reads. Below the module list, the port of
//! slate-sources/index.ts at 65f6b89c7. Spec: spec/slate_sources.md.
//!
//! A path is read by its first segment's source and then walked by name and index; a source missing here reads as
//! not there yet, which the renderer draws quietly.

pub mod context;
pub mod cost;
pub mod git;
pub mod pr;
pub mod thread;
pub mod time;
pub mod usage;

use indexmap::IndexMap;
use serde_json::Value;

pub use context::{HostSlateSource, SlateSourceContext, SlateWorkspaceFacts};

/// One segment of a path: a name, or an index (negative counts from the end).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seg {
    Name(String),
    Index(i64),
}

const UNSAFE: [&str; 3] = ["__proto__", "constructor", "prototype"];

/// The six sources in their order: thread, usage, cost, time, git, pr.
pub fn host_slate_sources() -> Vec<&'static dyn HostSlateSource> {
    let sources: [&'static dyn HostSlateSource; 6] =
        [&thread::ThreadSource, &usage::UsageSource, &cost::CostSource, &time::TimeSource, &git::GitSource, &pr::PrSource];
    sources.to_vec()
}

pub fn path_segments(path: &str) -> Vec<Seg> {
    let mut segs = Vec::new();
    let mut rest = path;
    while let Some(c) = rest.chars().next() {
        if c == '[' {
            if let Some((index, used)) = bracket_index(rest) {
                segs.push(Seg::Index(index));
                rest = &rest[used..];
                continue;
            }
        }
        if matches!(c, '.' | '[' | ']') {
            rest = &rest[1..];
            continue;
        }
        let end = rest.find(|ch: char| matches!(ch, '.' | '[' | ']')).unwrap_or(rest.len());
        segs.push(Seg::Name(rest[..end].to_string()));
        rest = &rest[end..];
    }
    segs
}

/// A bracketed integer at the start of `text`, and how many bytes it spans; an integer past i64 saturates.
fn bracket_index(text: &str) -> Option<(i64, usize)> {
    let bytes = text.as_bytes();
    let mut j = 1;
    if bytes.get(j) == Some(&b'-') {
        j += 1;
    }
    let digits = j;
    while bytes.get(j).is_some_and(|b| b.is_ascii_digit()) {
        j += 1;
    }
    if j == digits || bytes.get(j) != Some(&b']') {
        return None;
    }
    let index = text[1..j].parse::<i64>().unwrap_or(if bytes[1] == b'-' { i64::MIN } else { i64::MAX });
    Some((index, j + 1))
}

/// One value walked down by segments; `None` where a step finds nothing (spec rule 6).
pub fn walk(value: Option<&Value>, segs: &[Seg]) -> Option<Value> {
    let mut at = value?;
    for seg in segs {
        at = match (at, seg) {
            (Value::Array(items), Seg::Index(i)) => {
                let len = items.len() as i64;
                let idx = if *i < 0 { len + i } else { *i };
                items.get(usize::try_from(idx).ok()?)?
            }
            (Value::Object(map), Seg::Name(name)) if !UNSAFE.contains(&name.as_str()) => map.get(name)?,
            _ => return None,
        };
    }
    Some(at.clone())
}

/// Each source's view, `None` for one not there yet.
pub type Views = IndexMap<String, Option<Value>>;

/// Each named source viewed once, in first-seen order, one after another (spec rule 7).
pub async fn view_sources(names: &[String], ctx: &dyn SlateSourceContext) -> Views {
    let mut views = Views::new();
    for name in names {
        if views.contains_key(name) {
            continue;
        }
        let Some(source) = host_slate_sources().into_iter().find(|s| s.name() == name.as_str()) else {
            continue;
        };
        let view = source.view(ctx).await;
        views.insert(name.clone(), view);
    }
    views
}

pub fn resolve_in(views: &Views, path: &str) -> Option<Value> {
    let segs = path_segments(path);
    let Some((Seg::Name(head), rest)) = segs.split_first() else {
        return None;
    };
    walk(views.get(head).and_then(|v| v.as_ref()), rest)
}
