// SPDX-License-Identifier: AGPL-3.0-only
//! One reader per session store format. A reader names its agent's own session files, each with the stamp that says
//! whether it changed, and turns one of them into tool calls and nothing else. No line, argument or result an agent saw
//! is kept or returned. Port of packages/collect/src/history/reader.ts at a3b870a6e; the spec is spec/history_readers.md.
use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use serde_json::Value;
use wsp_catalog::json_parse::parse_json;
use wsp_words::exit::HostError;

use crate::host::{Host, StatKind};

/// What one call came to (reader.ts:18): a shell line, or any other tool by name.
#[derive(Debug, Clone, PartialEq)]
pub enum CallKind {
    Shell { line: String },
    Other { name: String },
}

/// reader.ts:9-18.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub session: String,
    /// The folder the session ran in, as its own store records it.
    pub folder: Option<String>,
    /// When the store says the call was made, ms epoch.
    pub at: Option<f64>,
    pub kind: CallKind,
}

/// reader.ts:20.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryFile {
    /// Absolute path of one file the reader can read calls from.
    pub path: String,
    /// What says the file has not changed since it was last read: `<mtimeMs>:<bytes>`.
    pub stamp: String,
}

/// reader.ts:27. Dyn-safe so the registry can hand out one reader per format.
pub trait HistoryReader {
    /// The files under root that hold sessions; empty when the root is not there.
    fn files<'a>(&'a self, host: &'a Host, root: &'a str) -> LocalBoxFuture<'a, Vec<HistoryFile>>;
    /// Every tool call in one of those files. An `Err` item is the TS throw: the file is there and cannot be read.
    fn read<'a>(&'a self, host: &'a Host, root: &'a str, file: &'a str) -> LocalBoxStream<'a, Result<Call, HostError>>;
    /// Where a file's sessions sit: Claude Code's project folder, else the file itself.
    fn holder(&self, root: &str, file: &str) -> String;
}

/// reader.ts:42. Bump it when commands.rs, tally.rs or a reader changes what a bucket comes to.
pub const PARSE_VERSION: u32 = 2;

/// The file with its stamp: `${mtimeMs}:${bytes}`, the mtime written as JavaScript writes a number (`1000`, `1.5`).
/// `None` when the path is not a file.
pub async fn stamp_of(host: &Host, path: &str) -> Option<HistoryFile> {
    let st = host.fs.stat(path).await?;
    (st.kind == StatKind::File)
        .then(|| HistoryFile { path: path.to_string(), stamp: format!("{}:{}", wsp_words::js::number_string(st.mtime_ms), st.bytes) })
}

/// Every `.jsonl` file under root the session rule has an answer for, stamped, in walk order.
pub async fn jsonl_files(host: &Host, root: &str, session: &dyn Fn(&str) -> Option<String>) -> Vec<HistoryFile> {
    let mut out = Vec::new();
    for file in host.fs.walk(root).await {
        if !file.ends_with(".jsonl") || session(&file).is_none() {
            continue;
        }
        out.extend(stamp_of(host, &file).await);
    }
    out
}

/// A JSON object (not an array, not null).
pub fn is_record(v: &Value) -> bool {
    v.is_object()
}

/// `JSON.parse(text)`, or `None` where it throws: read as JavaScript reads it, a lone surrogate escape, a number past
/// f64 and nesting past 128 included.
pub fn try_json(text: &str) -> Option<Value> {
    parse_json(text).ok()
}

/// A store's timestamp as ms epoch: a JSON string that is an RFC 3339 instant; `None` for anything else.
pub fn time_of(v: Option<&Value>) -> Option<f64> {
    iso_ms(v?.as_str()?)
}

/// The ES5 date-time form Date.parse reads (spec rule 4): a year, an optional month and day, an optional `T` time
/// with optional seconds and fraction, and an optional offset. A date alone is read as UTC, a date-time with no offset as
/// local time.
fn iso_ms(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    let at = |i: usize| b.get(i).copied();
    let num = |lo: usize, hi: usize| -> Option<i64> {
        let digits = b.get(lo..hi)?;
        if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
            return None;
        }
        Some(digits.iter().fold(0, |acc, d| acc * 10 + i64::from(d - b'0')))
    };

    let (year, mut p) = match at(0) {
        Some(sign @ (b'+' | b'-')) => {
            let y = num(1, 7)?;
            if sign == b'-' && y == 0 {
                return None;
            }
            (if sign == b'-' { -y } else { y }, 7)
        }
        _ => (num(0, 4)?, 4),
    };
    let (mut month, mut day) = (1, 1);
    if at(p) == Some(b'-') {
        month = num(p + 1, p + 3)?;
        p += 3;
        if at(p) == Some(b'-') {
            day = num(p + 1, p + 3)?;
            p += 3;
        }
    }

    let (mut hour, mut minute, mut second, mut milli) = (0, 0, 0, 0);
    let timed = matches!(at(p), Some(b'T' | b't'));
    if timed {
        hour = num(p + 1, p + 3)?;
        if at(p + 3) != Some(b':') {
            return None;
        }
        minute = num(p + 4, p + 6)?;
        p += 6;
        if at(p) == Some(b':') {
            second = num(p + 1, p + 3)?;
            p += 3;
            if at(p) == Some(b'.') {
                let start = p + 1;
                let end = start + b[start..].iter().take_while(|d| d.is_ascii_digit()).count();
                if end == start {
                    return None;
                }
                let frac = &b[start..end.min(start + 3)];
                milli = frac.iter().fold(0, |acc, d| acc * 10 + i64::from(d - b'0')) * 10i64.pow(3 - frac.len() as u32);
                p = end;
            }
        }
    }

    let mut offset = 0;
    let mut local = false;
    if timed {
        match at(p) {
            None => local = true,
            Some(b'Z' | b'z') => p += 1,
            Some(sign @ (b'+' | b'-')) => {
                let oh = num(p + 1, p + 3)?;
                let om = if at(p + 3) == Some(b':') {
                    p += 6;
                    num(p - 2, p)?
                } else {
                    p += 5;
                    num(p - 2, p)?
                };
                if oh > 23 || om > 59 {
                    return None;
                }
                offset = if sign == b'-' { -(oh * 60 + om) } else { oh * 60 + om };
            }
            Some(_) => return None,
        }
    }
    if p != b.len() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if hour > 24 || minute > 59 || second > 59 || (hour == 24 && (minute != 0 || second != 0 || milli != 0)) {
        return None;
    }

    let days = days_from_civil(year, month, 1) + day - 1;
    let utc_secs = days * 86_400 + hour * 3600 + minute * 60 + second - offset * 60;
    let secs = if local { local_secs(year, month, day, hour, minute, second)? } else { utc_secs };
    let ms = secs * 1000 + milli;
    (ms.abs() <= 8_640_000_000_000_000).then_some(ms as f64)
}

/// Seconds since the epoch of a wall-clock time in this process's time zone, which `TZ` names as it does for node.
fn local_secs(year: i64, month: i64, day: i64, hour: i64, minute: i64, second: i64) -> Option<i64> {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    tm.tm_year = i32::try_from(year - 1900).ok()?;
    tm.tm_mon = (month - 1) as i32;
    tm.tm_mday = day as i32;
    tm.tm_hour = hour as i32;
    tm.tm_min = minute as i32;
    tm.tm_sec = second as i32;
    tm.tm_isdst = -1;
    let at = unsafe { libc::mktime(&mut tm) };
    (at != -1 || tm.tm_year == 69).then_some(at)
}

/// Days from 1970-01-01 to the given proleptic Gregorian date, month 1 to 12.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The file's name without its directory or its last extension; a leading dot is not an extension.
pub fn stem(path: &str) -> String {
    let base = &path[path.rfind('/').map_or(0, |p| p + 1)..];
    match base.rfind('.') {
        Some(dot) if dot > 0 => base[..dot].to_string(),
        _ => base.to_string(),
    }
}
