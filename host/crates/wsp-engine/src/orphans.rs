// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/orphans.ts. Spec: spec/sweeps.md.
//!
//! Kills running machines that carry our label but that nothing claims; reports another owner's, never kills them.
use std::collections::HashSet;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;

use wsp_words::js::to_fixed;

use crate::errors::EngineError;
use crate::errors::{is_missing, RealRetryClock, Result, RetryClock};
use crate::golden_seam::BUILDER_IDLE_MS;
use crate::labels::is_reserved;
use crate::labels::{BUILDER_LABEL, CREATED_AT_LABEL, OWNER_LABEL, SMOKE_LABEL, WSP_LABEL};
use crate::machine::{Machine, MachineBackend, MachineListRow, MachineState, NamedMachine, Size};

/// `Date.parse` for the forms a stamp takes, read the way V8 reads them: `YYYY`, `YYYY-MM` or `YYYY-MM-DD` (UTC
/// midnight; `+YYYYYY` and `-YYYYYY` for years past four digits), optionally followed by `T` or `t` (a space too after a
/// whole date; V8 reads other spaced forms by its own looser rules, which are not followed here) and
/// `HH:mm[:ss[.fff...]]` (hour 24 only as `24:00:00`) and then `Z` or an offset `+HH:mm` or `+HHmm`. A date and time with
/// no offset reads as UTC where node reads local time (spec/sweeps.md accepts it: every stamp is a `toISOString()`).
/// More than three fraction digits are cut, not rounded. A day past the month's end rolls into the next month. NaN for
/// anything else and for a time outside +-8.64e15 ms, as node answers.
pub fn date_parse(s: &str) -> f64 {
    iso_millis(s).map_or(f64::NAN, |ms| ms as f64)
}

fn iso_millis(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let digits = |from: usize, n: usize| -> Option<i64> {
        let part = b.get(from..from + n)?;
        part.iter().all(u8::is_ascii_digit).then(|| part.iter().fold(0i64, |acc, d| acc * 10 + i64::from(d - b'0')))
    };
    let (year, mut at) = match b.first()? {
        sign @ (b'+' | b'-') => {
            let y = digits(1, 6)?;
            if *sign == b'-' && y == 0 {
                return None;
            }
            (if *sign == b'-' { -y } else { y }, 7)
        }
        _ => (digits(0, 4)?, 4),
    };
    let (mut month, mut day) = (1, 1);
    if b.get(at) == Some(&b'-') {
        month = digits(at + 1, 2)?;
        at += 3;
        if b.get(at) == Some(&b'-') {
            day = digits(at + 1, 2)?;
            at += 3;
        }
    }
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let mut ms = days_from_civil(year, month, 1).checked_add(day - 1)?.checked_mul(86_400_000)?;
    if at < b.len() {
        let whole_date = b[1..at].iter().filter(|c| **c == b'-').count() == 2;
        if !(matches!(b[at], b'T' | b't') || (b[at] == b' ' && whole_date)) {
            return None;
        }
        let (hour, minute) = (digits(at + 1, 2)?, digits(at + 4, 2)?);
        if b.get(at + 3) != Some(&b':') {
            return None;
        }
        at += 6;
        let (mut second, mut milli) = (0, 0);
        if b.get(at) == Some(&b':') {
            second = digits(at + 1, 2)?;
            at += 3;
            if b.get(at) == Some(&b'.') {
                let end = (at + 1..b.len()).find(|&i| !b[i].is_ascii_digit()).unwrap_or(b.len());
                if end == at + 1 {
                    return None;
                }
                let kept = &b[at + 1..end.min(at + 4)];
                milli = kept.iter().fold(0i64, |acc, d| acc * 10 + i64::from(d - b'0')) * 10i64.pow(3 - kept.len() as u32);
                at = end;
            }
        }
        if hour > 24 || minute > 59 || second > 59 || (hour == 24 && (minute, second, milli) != (0, 0, 0)) {
            return None;
        }
        let mut offset = 0;
        match b.get(at) {
            Some(b'Z' | b'z') => at += 1,
            Some(sign @ (b'+' | b'-')) => {
                let (oh, om) = if b.get(at + 3) == Some(&b':') {
                    (digits(at + 1, 2)?, digits(at + 4, 2)?)
                } else {
                    (digits(at + 1, 2)?, digits(at + 3, 2)?)
                };
                if oh > 23 || om > 59 {
                    return None;
                }
                at += if b.get(at + 3) == Some(&b':') { 6 } else { 5 };
                offset = (oh * 60 + om) * 60_000 * if *sign == b'-' { -1 } else { 1 };
            }
            _ => {}
        }
        if at != b.len() {
            return None;
        }
        ms = ms.checked_add(((hour * 60 + minute) * 60 + second) * 1000 + milli - offset)?;
    }
    (ms.abs() <= 8_640_000_000_000_000).then_some(ms)
}

/// Days from 1970-01-01 to the first of a month, for any year (the proleptic Gregorian calendar).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
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
        let age_ms = (!created.is_nan()).then_some((now as f64 - created) as i64);
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
