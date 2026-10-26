// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/disk-sync.ts. Spec: spec/machine_reads.md.
//!
//! The guest's disk made whole before the provider copies or pauses it.
use wsp_words::js::js_trim;
use wsp_words::words::init::disk_sync_failed_line;

use crate::errors::EngineError;
use crate::exec_detached::put::{machine_answer, INLINE_EXEC_MS};
use crate::machine::{ExecOptions, Machine};

/// disk-sync.ts:13.
pub const DISK_SYNC_CMD: &str = "sync && awk '/^(Dirty|Writeback):/ {print $1, $2}' /proc/meminfo";

/// disk-sync.ts:16. What the guest's counters read once its sync returned, in kB.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiskReading {
    pub dirty_kb: f64,
    pub writeback_kb: f64,
}

/// disk-sync.ts:22 `DiskSyncError`, beside the exec's own failure, which passes as itself.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SyncDiskError {
    /// The guest's sync answered non-zero: `answer` is exec-detached's `machine_answer(res)`, `line` is
    /// `disk_sync_failed_line(answer)`, which is the message.
    #[error("{line}")]
    Sync { answer: String, line: String },
    #[error(transparent)]
    Exec(EngineError),
}

/// disk-sync.ts:31. `exec(DISK_SYNC_CMD, timeout INLINE_EXEC_MS)`. Non-zero is `Sync`. Else each counter is the digits of
/// the first line matching `^<Name>: (\d+)$` (multiline): `None` where either is missing.
pub async fn sync_disk(machine: &dyn Machine) -> Result<Option<DiskReading>, SyncDiskError> {
    let opts = ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), idempotency_key: None, stdin: None };
    let res = machine.exec(DISK_SYNC_CMD, opts).await.map_err(SyncDiskError::Exec)?;
    if res.exit_code.get() != 0 {
        let answer = machine_answer(&res);
        return Err(SyncDiskError::Sync { line: disk_sync_failed_line(&answer), answer });
    }
    let counter = |name: &str| -> Option<f64> {
        let digits = js_lines(&res.stdout).find_map(|line| {
            let digits = line.strip_prefix(name)?.strip_prefix(": ")?;
            (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())).then_some(digits)
        })?;
        let value = digits.parse::<f64>().ok()?;
        value.is_finite().then_some(value)
    };
    Ok(match (counter("Dirty"), counter("Writeback")) {
        (Some(dirty_kb), Some(writeback_kb)) => Some(DiskReading { dirty_kb, writeback_kb }),
        _ => None,
    })
}

/// disk-sync.ts:44.
pub const DISK_USE_CMD: &str = "df -Pk / | awk 'NR==2 {print $3, $4}'";

/// disk-sync.ts:48 answer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiskUse {
    pub used_bytes: f64,
    pub free_bytes: f64,
    /// `used * 100 / (used + free)`, unrounded.
    pub pct: f64,
}

/// disk-sync.ts:48. The first line of the trimmed text matching `^(\d+) (\d+)$` (multiline), in kB; `None` where none
/// matches or both are 0.
pub fn disk_use_of(stdout: &str) -> Option<DiskUse> {
    let (used, free) = js_lines(js_trim(stdout)).find_map(|line| {
        let (used, free) = line.split_once(' ')?;
        let is_count = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        (is_count(used) && is_count(free)).then(|| (used.parse::<f64>().unwrap_or(0.0), free.parse::<f64>().unwrap_or(0.0)))
    })?;
    if used + free == 0.0 {
        return None;
    }
    Some(DiskUse { used_bytes: used * 1024.0, free_bytes: free * 1024.0, pct: used * 100.0 / (used + free) })
}

/// disk-sync.ts:56.
pub fn disk_use_pct(stdout: &str) -> Option<f64> {
    disk_use_of(stdout).map(|d| d.pct)
}

/// disk-sync.ts:59. A reading with either counter above 0.
pub fn disk_unsettled(r: Option<&DiskReading>) -> bool {
    r.is_some_and(|r| r.dirty_kb > 0.0 || r.writeback_kb > 0.0)
}

/// The lines of `s` as JavaScript's `^`/`$` with the m flag reads them: a line ends at `\n`, `\r`, U+2028 or U+2029.
fn js_lines(s: &str) -> impl Iterator<Item = &str> {
    s.split(['\n', '\r', '\u{2028}', '\u{2029}'])
}
