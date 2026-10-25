// SPDX-License-Identifier: AGPL-3.0-only
//! `runtime/src/slate-sources/`: the host sources a slate reads. Below the module list, the port of
//! slate-sources/index.ts at 65f6b89c7. Spec: spec/slate_sources.md.
//!
//! A path is read by its first segment's source and then walked by name and index; a source missing here reads as
//! not there yet, which the renderer draws quietly.
#![allow(unused_variables)]

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

// index.ts:15 HOST_SLATE_SOURCES
/// The six sources in their order: thread, usage, cost, time, git, pr.
pub fn host_slate_sources() -> Vec<&'static dyn HostSlateSource> {
    todo!()
}

// index.ts:18 pathSegments(path)
pub fn path_segments(path: &str) -> Vec<Seg> {
    todo!()
}

// index.ts:27 walk(value, segs)
/// One value walked down by segments; `None` where a step finds nothing (spec rule 6).
pub fn walk(value: Option<&Value>, segs: &[Seg]) -> Option<Value> {
    todo!()
}

/// Each source's view, `None` for one not there yet.
pub type Views = IndexMap<String, Option<Value>>;

// index.ts:43 viewSources(names, ctx)
/// Each named source viewed once, in first-seen order, one after another (spec rule 7).
pub async fn view_sources(names: &[String], ctx: &dyn SlateSourceContext) -> Views {
    todo!()
}

// index.ts:54 resolveIn(views, path)
pub fn resolve_in(views: &Views, path: &str) -> Option<Value> {
    todo!()
}
