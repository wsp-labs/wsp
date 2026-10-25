// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/orphans.ts. Spec: spec/sweeps.md.
//!
//! Kills running machines that carry our label but that nothing claims; reports another owner's, never kills them.
use std::collections::HashSet;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;

use wsp_words::js::to_fixed;

use crate::errors::{is_missing, RealRetryClock, Result, RetryClock};
use crate::golden_seam::BUILDER_IDLE_MS;
use crate::labels::{BUILDER_LABEL, CREATED_AT_LABEL, OWNER_LABEL, SMOKE_LABEL, WSP_LABEL};
use crate::machine::{BackendPricing, Machine, MachineBackend, MachineListRow, MachineState, NamedMachine, Size};
use crate::errors::EngineError;
use crate::labels::is_reserved;

/// `Date.parse` for the ISO forms a stamp takes: a date alone (UTC midnight), a date and time read as UTC, and a time
/// with an offset. NaN for anything else, as node answers.
fn date_parse(s: &str) -> f64 {
    if let Ok(t) = s.parse::<jiff::Timestamp>() {
        return t.as_millisecond() as f64;
    }
    if let Ok(dt) = s.parse::<jiff::civil::DateTime>() {
        return dt.to_zoned(jiff::tz::TimeZone::UTC).map_or(f64::NAN, |z| z.timestamp().as_millisecond() as f64);
    }
    let mut parts = s.split('-');
    let year = parts.next().filter(|y| y.len() == 4 && y.bytes().all(|b| b.is_ascii_digit()));
    let month = parts.next().map_or(Some(1), |m| (m.len() == 2 && m.bytes().all(|b| b.is_ascii_digit())).then(|| m.parse::<i8>().ok()).flatten());
    let day = parts.next().map_or(Some(1), |d| (d.len() == 2 && d.bytes().all(|b| b.is_ascii_digit())).then(|| d.parse::<i8>().ok()).flatten());
    let (Some(year), Some(month), Some(day), None) = (year, month, day, parts.next()) else { return f64::NAN };
    let Ok(date) = jiff::civil::Date::new(year.parse().unwrap_or(0), month, day) else { return f64::NAN };
    date.at(0, 0, 0, 0).to_zoned(jiff::tz::TimeZone::UTC).map_or(f64::NAN, |z| z.timestamp().as_millisecond() as f64)
}

/// orphans.ts:9. A machine of this owner this young is spared on age alone.
pub const OWN_GRACE_MS: i64 = 60_000;
/// orphans.ts:97. How old a machine named after one of this host's images must be before the sweep deletes it.
pub const STRAY_GRACE_MS: i64 = 10 * 60_000;
/// orphans.ts:129. The default `older_than_ms`.
pub const UNOWNED_BACKSTOP_MS: i64 = 10 * 60_000;

/// orphans.ts:12. One row of the provider's listing.
pub type ListedMachine = MachineListRow;

/// orphans.ts:22 `stop`: stops one machine the sweep kills.
pub type StopMachine = Arc<dyn Fn(Arc<dyn Machine>) -> LocalBoxFuture<'static, Result<()>>>;

/// orphans.ts:14.
pub struct ReapOptions {
    pub backend: Arc<dyn MachineBackend>,
    /// The listing the caller already fetched; `None` reads `backend.list(None)`.
    pub listing: Option<Vec<ListedMachine>>,
    /// Ids something claims. Read once, after the listing returns.
    pub known_ids: Box<dyn Fn() -> Vec<String>>,
    /// The sweeping state file's id.
    pub owner: String,
    pub stop: StopMachine,
    /// Age an unclaimed unowned non-builder must reach; `UNOWNED_BACKSTOP_MS` when `None`.
    pub older_than_ms: Option<i64>,
    /// Epoch ms, read once; the wall clock when `None`.
    pub now: Option<Box<dyn Fn() -> i64>>,
}

/// orphans.ts:30.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Whose {
    Own,
    Foreign,
    None,
}

/// orphans.ts:34. By the `wsp-owner` label alone: absent is `None`, equal to `owner` is `Own`, else `Foreign`.
pub fn whose_machine(labels: &IndexMap<String, String>, owner: &str) -> Whose {
    match labels.get(OWNER_LABEL) {
        None => Whose::None,
        Some(stamp) if stamp == owner => Whose::Own,
        Some(_) => Whose::Foreign,
    }
}

/// orphans.ts:51.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReapReason {
    Recorded,
    Unfinished,
    Expired,
    Grace,
    Own,
    Orphan,
    Lost,
    Stray,
    Unrecorded,
}

/// orphans.ts:39.
#[derive(Debug, Clone, PartialEq)]
pub struct ReapedMachine {
    pub id: String,
    pub labels: Option<IndexMap<String, String>>,
    pub builder: bool,
    pub reason: ReapReason,
    pub age_ms: Option<i64>,
    pub name: Option<String>,
}

/// orphans.ts:58.
#[derive(Debug, Clone, PartialEq)]
pub struct SparedMachine {
    pub id: String,
    pub labels: IndexMap<String, String>,
    pub builder: bool,
    pub whose: Whose,
    pub owner: Option<String>,
    pub age_ms: Option<i64>,
    pub backstop_ms: i64,
    pub rate_usd_per_hour: f64,
}

/// orphans.ts:71.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReapFailure {
    /// `None` when the listing itself failed.
    pub id: Option<String>,
    pub message: String,
}

/// orphans.ts:77. `failed` is `None` where nothing failed, never an empty list.
#[derive(Debug, Clone, PartialEq)]
pub struct ReapResult {
    pub reaped: Vec<ReapedMachine>,
    pub spared: Vec<SparedMachine>,
    pub failed: Option<Vec<ReapFailure>>,
}

/// orphans.ts:87. Running or paused; not reserved; wears `wsp=1` and this owner's label; not a builder or smoke fork
/// (`=1`); and its `createdAt` label unparseable (or absent) or at least `OWN_GRACE_MS` old.
pub fn lost_workspace(m: &ListedMachine, owner: &str, now: i64) -> bool {
    let labels = &m.labels;
    if !matches!(m.state, MachineState::Running | MachineState::Paused) {
        return false;
    }
    if is_reserved(labels) || labels.get(WSP_LABEL).map(String::as_str) != Some("1") || whose_machine(labels, owner) != Whose::Own {
        return false;
    }
    if labels.get(BUILDER_LABEL).map(String::as_str) == Some("1") || labels.get(SMOKE_LABEL).map(String::as_str) == Some("1") {
        return false;
    }
    let created = date_parse(labels.get(CREATED_AT_LABEL).map_or("", String::as_str));
    created.is_nan() || now as f64 - created >= OWN_GRACE_MS as f64
}

/// orphans.ts:102 `read`.
#[derive(Debug, Clone, Default)]
pub struct StrayRead {
    pub known: HashSet<String>,
    pub asked: HashSet<String>,
    pub now: i64,
}

/// orphans.ts:102. In row order, each with its age: not gone, id not known, name not asked, `created_at` parseable and
/// at least `STRAY_GRACE_MS` old.
pub fn strays(rows: &[NamedMachine], read: &StrayRead) -> Vec<(NamedMachine, i64)> {
    rows.iter()
        .filter_map(|m| {
            let age = read.now as f64 - date_parse(m.created_at.as_deref().unwrap_or(""));
            let kept = m.state == MachineState::Gone
                || read.known.contains(&m.id)
                || read.asked.contains(&m.name)
                || age.is_nan()
                || age < STRAY_GRACE_MS as f64;
            (!kept).then(|| (m.clone(), age as i64))
        })
        .collect()
}

/// orphans.ts:111. `age unknown`; under a minute `<floor s> s old`; under an hour `<floor min> min old`; else
/// `<h with one decimal, JS toFixed> h old`. A negative age reads as 0.
pub fn describe_age(ms: Option<i64>) -> String {
    let Some(ms) = ms else { return "age unknown".into() };
    let age = ms.max(0);
    if age < 60_000 {
        format!("{} s old", age / 1000)
    } else if age < 3_600_000 {
        format!("{} min old", age / 60_000)
    } else {
        format!("{} h old", to_fixed(age as f64 / 3_600_000.0, 1))
    }
}

/// orphans.ts:127. An empty owner fails before anything is read, with
/// `reap needs the owner id; an empty one would claim every unowned machine`. Rules in spec/sweeps.md; a listing that
/// fails is the failure (the TS `await` throws it).
pub async fn reap(opts: ReapOptions) -> Result<ReapResult> {
    if opts.owner.is_empty() {
        return Err(EngineError::Other {
            message: "reap needs the owner id; an empty one would claim every unowned machine".into(),
            code: None,
        });
    }
    let older_than = opts.older_than_ms.unwrap_or(UNOWNED_BACKSTOP_MS);
    let now = opts.now.as_ref().map_or_else(|| RealRetryClock.now(), |f| f());
    let rows = match &opts.listing {
        Some(rows) => rows.clone(),
        None => opts.backend.list(None).await?,
    };
    let known: HashSet<String> = (opts.known_ids)().into_iter().collect();
    let pricing = opts.backend.pricing();

    let mut reaped = Vec::new();
    let mut spared = Vec::new();
    let mut failed = Vec::new();
    for m in rows {
        if m.state != MachineState::Running || is_reserved(&m.labels) || m.labels.get(WSP_LABEL).map(String::as_str) != Some("1") {
            continue;
        }
        if known.contains(&m.id) {
            continue;
        }
        let builder = m.labels.get(BUILDER_LABEL).map(String::as_str) == Some("1");
        let whose = whose_machine(&m.labels, &opts.owner);
        let created = date_parse(m.labels.get(CREATED_AT_LABEL).map_or("", String::as_str));
        let age_ms = (!created.is_nan()).then(|| (now as f64 - created) as i64);
        let backstop_ms = match whose {
            Whose::Own => OWN_GRACE_MS,
            _ if builder => BUILDER_IDLE_MS as i64,
            _ => older_than,
        };
        let past_backstop = age_ms.is_some_and(|a| a >= backstop_ms);
        let kill = match whose {
            Whose::Own => age_ms.is_none() || past_backstop,
            Whose::None => past_backstop,
            Whose::Foreign => false,
        };
        if !kill {
            let size = m.size.as_ref().map_or_else(|| pricing.default_size(), |s| Size { cpu: s.cpu.0, mem_mb: s.mem_mb.0 });
            spared.push(SparedMachine {
                id: m.id.clone(),
                labels: m.labels.clone(),
                builder,
                whose,
                owner: m.labels.get(OWNER_LABEL).cloned(),
                age_ms,
                backstop_ms,
                rate_usd_per_hour: pricing.rate_usd_per_hour(size),
            });
            continue;
        }
        let stopped = match opts.backend.get(&m.id).await {
            Ok(machine) => (opts.stop)(machine).await,
            Err(e) => Err(e),
        };
        match stopped {
            Ok(()) => reaped.push(ReapedMachine {
                id: m.id.clone(),
                labels: Some(m.labels.clone()),
                builder,
                reason: if whose == Whose::Own { ReapReason::Own } else { ReapReason::Orphan },
                age_ms,
                name: None,
            }),
            // The listing lags a kill (measured): a row that is gone by the time it is fetched is no failure.
            Err(e) if is_missing(&e) => {}
            Err(e) => failed.push(ReapFailure { id: Some(m.id.clone()), message: format!("could not stop: {e}") }),
        }
    }
    Ok(ReapResult { reaped, spared, failed: (!failed.is_empty()).then_some(failed) })
}
