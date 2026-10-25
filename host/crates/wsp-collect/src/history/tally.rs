// SPDX-License-Identifier: AGPL-3.0-only
//! A file's calls reduced to names and counts, and one agent's buckets added up. Port of
//! packages/collect/src/history/tally.ts at a3b870a6e; the spec is spec/history_store.md.
use std::collections::HashSet;

use futures::stream::LocalBoxStream;
use futures::StreamExt;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use wsp_catalog::catalog::catalog_tool_for;
use wsp_words::exit::HostError;
use wsp_words::js::utf16;
use wsp_words::project_path::under_project;

use crate::history::commands::{command_names, install_names, Install};
use crate::history::reader::{Call, CallKind};

/// tally.ts:7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Count {
    pub sessions: u64,
    pub calls: u64,
}

/// Over this many bytes a row is heavy (tally.ts:13).
pub const HEAVY_BYTES: u64 = 300 * wsp_catalog::roads::MIB;
/// The least use that ticks a tool on its own (tally.ts:15).
pub const USED_FLOOR: Count = Count { sessions: 2, calls: 5 };
/// A heavy row takes more use to earn its place (tally.ts:17).
pub const HEAVY_USED_FLOOR: Count = Count { sessions: 3, calls: 20 };

/// `size > HEAVY_BYTES`; `None` is not heavy.
pub fn is_heavy(size: Option<u64>) -> bool {
    size.is_some_and(|s| s > HEAVY_BYTES)
}

/// At or over the floor its weight sets, on both counts.
pub fn meets_used_floor(c: Count, size: Option<u64>) -> bool {
    let floor = if is_heavy(size) { HEAVY_USED_FLOOR } else { USED_FLOOR };
    c.sessions >= floor.sessions && c.calls >= floor.calls
}

/// tally.ts:28. Each map is ordered most sessions first, then most calls, then by name in UTF-16 order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Usage {
    pub sessions: u64,
    pub calls: u64,
    pub commands: IndexMap<String, Count>,
    /// Keyed by install_key (`brew gh`, `npm agent-browser`).
    pub installs: IndexMap<String, Count>,
    /// The catalog tools the commands and installs stand for, by id; a session counts once per tool.
    pub tools: IndexMap<String, Count>,
}

/// `${via} ${name}` (tally.ts:66).
pub fn install_key(i: &Install) -> String {
    format!("{} {}", i.via, i.name)
}

/// The text after the first space of a key; the whole key when it has none (tally.ts:67).
pub fn installed_name(key: &str) -> String {
    key.split_once(' ').map_or(key, |(_, name)| name).to_string()
}

/// What one session came to in one file (tally.ts:71): all a cache stores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryBucket {
    pub session: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    pub calls: u64,
    /// Command name to how often it ran, in first-seen order.
    pub commands: IndexMap<String, u64>,
    /// install_key to how often an installer was asked for it, in first-seen order.
    pub installs: IndexMap<String, u64>,
    /// The newest call of that session in that file, ms epoch; written as JavaScript writes the number. Last: the TS
    /// sets it after the bucket is made, so a freshly read bucket writes it last (spec/history_store.md rule 12).
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "wsp_words::js::serialize_opt_f64")]
    pub at: Option<f64>,
}

/// Whether a bucket counts when the recipe is weighed against named folders: its session ran at one of them or
/// inside it. A bucket whose store records no folder counts for nothing, since nothing says it is this project's.
pub fn in_folders(folder: Option<&str>, folders: &[String]) -> bool {
    folder.is_some_and(|f| folders.iter().any(|root| under_project(f, root)))
}

/// One file's calls reduced to one bucket per (session, folder), in first-call order. The stream's first `Err` is
/// returned as it is, and nothing after it is read.
pub async fn reduce_calls(mut calls: LocalBoxStream<'_, Result<Call, HostError>>) -> Result<Vec<HistoryBucket>, HostError> {
    let mut buckets: IndexMap<(String, String), HistoryBucket> = IndexMap::new();
    while let Some(item) = calls.next().await {
        let c = item?;
        let bucket = buckets.entry((c.session.clone(), c.folder.clone().unwrap_or_default())).or_insert_with(|| HistoryBucket {
            session: c.session.clone(),
            folder: c.folder.clone(),
            calls: 0,
            commands: IndexMap::new(),
            installs: IndexMap::new(),
            at: None,
        });
        bucket.calls += 1;
        if let Some(at) = c.at {
            if bucket.at.is_none_or(|kept| at > kept) {
                bucket.at = Some(at);
            }
        }
        if let CallKind::Shell { line } = &c.kind {
            for name in command_names(line) {
                *bucket.commands.entry(name).or_insert(0) += 1;
            }
            for i in install_names(line) {
                *bucket.installs.entry(install_key(&i)).or_insert(0) += 1;
            }
        }
    }
    Ok(buckets.into_values().collect())
}

/// One file's buckets with the holder its sessions sit under (TS `{ holder, buckets }`).
#[derive(Debug, Clone, PartialEq)]
pub struct Held {
    pub holder: String,
    pub buckets: Vec<HistoryBucket>,
}

/// The buckets with every session counted from one holder: the one whose newest call is latest, the first found on a
/// tie (a bucket with no time reads as minus infinity). Order: held order, then bucket order.
pub fn one_holder(held: &[Held]) -> Vec<HistoryBucket> {
    let mut seen: IndexMap<&str, IndexMap<&str, f64>> = IndexMap::new();
    for h in held {
        for b in &h.buckets {
            let at = b.at.unwrap_or(f64::NEG_INFINITY);
            let newest = seen.entry(b.session.as_str()).or_default().entry(h.holder.as_str()).or_insert(f64::NEG_INFINITY);
            *newest = newest.max(at);
        }
    }
    let mut kept: IndexMap<&str, (&str, f64)> = IndexMap::new();
    for (session, holders) in &seen {
        for (holder, at) in holders {
            if kept.get(session).is_none_or(|&(_, kept_at)| *at > kept_at) {
                kept.insert(*session, (*holder, *at));
            }
        }
    }
    held.iter()
        .flat_map(|h| {
            h.buckets.iter().filter(|b| kept.get(b.session.as_str()).is_some_and(|&(holder, _)| holder == h.holder)).cloned()
        })
        .collect()
}

#[derive(Default)]
struct Tally {
    seen: IndexMap<String, (HashSet<String>, u64)>,
}

impl Tally {
    fn add(&mut self, name: &str, session: &str, calls: u64) {
        let (sessions, total) = self.seen.entry(name.to_string()).or_default();
        sessions.insert(session.to_string());
        *total += calls;
    }

    /// Most sessions first, then most calls, then by name.
    fn counts(&self) -> IndexMap<String, Count> {
        let mut rows: Vec<(&String, Count)> = self
            .seen
            .iter()
            .map(|(name, (sessions, calls))| (name, Count { sessions: sessions.len() as u64, calls: *calls }))
            .collect();
        rows.sort_by(|a, b| b.1.sessions.cmp(&a.1.sessions).then(b.1.calls.cmp(&a.1.calls)).then_with(|| utf16::cmp(a.0, b.0)));
        rows.into_iter().map(|(name, count)| (name.clone(), count)).collect()
    }
}

fn tool(tools: &mut Tally, name: &str, session: &str, calls: u64) {
    if let Some(t) = catalog_tool_for(name) {
        tools.add(&t.id, session, calls);
    }
}

/// Every bucket added up; with `folders`, only buckets `in_folders`.
pub fn usage_of(buckets: &[HistoryBucket], folders: Option<&[String]>) -> Usage {
    let mut sessions: HashSet<&str> = HashSet::new();
    let mut total = 0;
    let mut commands = Tally::default();
    let mut installs = Tally::default();
    let mut tools = Tally::default();
    for b in buckets {
        if folders.is_some_and(|f| !in_folders(b.folder.as_deref(), f)) {
            continue;
        }
        sessions.insert(&b.session);
        total += b.calls;
        for (name, calls) in &b.commands {
            commands.add(name, &b.session, *calls);
            tool(&mut tools, name, &b.session, *calls);
        }
        for (key, calls) in &b.installs {
            installs.add(key, &b.session, *calls);
            tool(&mut tools, &installed_name(key), &b.session, *calls);
        }
    }
    Usage {
        sessions: sessions.len() as u64,
        calls: total,
        commands: commands.counts(),
        installs: installs.counts(),
        tools: tools.counts(),
    }
}
