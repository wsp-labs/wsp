// SPDX-License-Identifier: AGPL-3.0-only
//! Port of the helpers of packages/engine/src/golden.ts that read a deleted machine or snapshot back until the provider
//! says it is gone: `sightMachine`, `readGone`, `killUntilGone`, `snapshotUntilGone`, their constants and the
//! `MachineAliveError` a kill ends in. The seal and the builders' sweeps in golden.ts are not here. The time they wait on
//! is the caller's `RetryClock`, as `until_reached`'s is.
use std::sync::Arc;

use crate::errors::{is_missing, EngineError, Result, RetryClock};
use crate::machine::{Machine, MachineBackend, MachineState};

/// golden.ts:35. How long to wait for the provider to report a killed machine gone before killing again (`grace_ms`),
/// and how often it is read meanwhile (`poll_ms`); `None` is the default of each.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KillConfirm {
    pub grace_ms: Option<u64>,
    pub poll_ms: Option<u64>,
}

/// golden.ts:41. How long a delete is read back for when the caller names no window.
pub const KILL_GRACE_MS: u64 = 30_000;

/// golden.ts:100. How many reads in a row must answer gone before a kill is done.
pub const GONE_READS: u32 = 12;

/// golden.ts:101. How many times a kill is asked before a machine that is still there is refused.
pub const KILL_ASKS: u32 = 3;

/// golden.ts's default `confirm.pollMs`.
const POLL_MS: u64 = 1_000;

/// golden.ts:125. One read of a machine: the state it answered, and the handle it came on when the provider found it.
#[derive(Clone)]
pub struct Sighting {
    pub state: MachineState,
    pub machine: Option<Arc<dyn Machine>>,
}

/// A machine state as a line quotes it (`String(state)`).
pub fn machine_state_word(state: MachineState) -> &'static str {
    match state {
        MachineState::Starting => "starting",
        MachineState::Running => "running",
        MachineState::Paused => "paused",
        MachineState::Gone => "gone",
    }
}

/// golden.ts:132 `readOnce`. One call per read: a second call for the state can land on the other copy and read gone. A
/// 404 reads as gone.
async fn read_once(backend: &dyn MachineBackend, id: &str) -> Result<Sighting> {
    let found: Result<Sighting> = async {
        let machine = backend.get(id).await?;
        let state = match machine.seen() {
            Some(seen) => seen.state,
            None => machine.state().await?,
        };
        Ok(Sighting { state, machine: Some(machine) })
    }
    .await;
    match found {
        Err(e) if is_missing(&e) => Ok(Sighting { state: MachineState::Gone, machine: None }),
        other => other,
    }
}

/// golden.ts:141. Whether the provider still has a machine, asking nothing of it: gone once `GONE_READS` reads in a row
/// answer gone, else the first read that found it, with its handle. A failed read that is not a 404 fails.
pub async fn sight_machine(backend: &dyn MachineBackend, id: &str) -> Result<Sighting> {
    for _ in 0..GONE_READS {
        let seen = read_once(backend, id).await?;
        if seen.state != MachineState::Gone {
            return Ok(seen);
        }
    }
    Ok(Sighting { state: MachineState::Gone, machine: None })
}

/// golden.ts:150. The state `sight_machine` answers, for a caller that needs no handle.
pub async fn read_gone(backend: &dyn MachineBackend, id: &str) -> Result<MachineState> {
    Ok(sight_machine(backend, id).await?.state)
}

/// golden.ts:112. The provider's kill acknowledges the request, not the machine's death, so a kill is only done once the
/// machine reads gone `GONE_READS` times in a row. One that reads the machine alive after a gone one means the ask went
/// to a copy that never held it, and the kill is asked again at once. After `KILL_ASKS` asks the machine that is still
/// there is `EngineError::MachineAlive`.
pub async fn kill_until_gone(
    backend: &dyn MachineBackend,
    machine: &dyn Machine,
    confirm: KillConfirm,
    clock: &dyn RetryClock,
) -> Result<()> {
    read_back_gone(backend, machine, confirm, clock, None).await.map(|_| ())
}

/// golden.ts:177 `WatchRound`. One background round: whether the first kill was sent already, and whether the watch is
/// still open.
pub struct WatchRound<'a> {
    pub asked: bool,
    pub live: &'a dyn Fn() -> bool,
}

/// golden.ts:185 `readBackGone`, `killUntilGone`'s rule. Under a watch's round it answers `false` as soon as the watch
/// closes and sends nothing after; `true` is a machine read gone `GONE_READS` times in a row.
pub async fn read_back_gone(
    backend: &dyn MachineBackend,
    machine: &dyn Machine,
    confirm: KillConfirm,
    clock: &dyn RetryClock,
    round: Option<&WatchRound<'_>>,
) -> Result<bool> {
    let live = || round.is_none_or(|r| (r.live)());
    let grace_ms = confirm.grace_ms.unwrap_or(KILL_GRACE_MS) as i64;
    let poll_ms = confirm.poll_ms.unwrap_or(POLL_MS);
    let mut alive = MachineState::Running;
    for attempt in 0..KILL_ASKS {
        if !live() {
            return Ok(false);
        }
        if attempt > 0 || !round.is_some_and(|r| r.asked) {
            if let Err(e) = machine.kill().await {
                if !is_missing(&e) {
                    return Err(e);
                }
            }
        }
        let deadline = clock.now() + grace_ms;
        let mut gone = 0;
        loop {
            if !live() {
                return Ok(false);
            }
            let seen = read_once(backend, machine.id()).await?;
            if seen.state == MachineState::Gone {
                gone += 1;
                if gone >= GONE_READS {
                    return Ok(true);
                }
            } else {
                alive = seen.state;
                if gone > 0 {
                    break;
                }
                clock.sleep(poll_ms).await;
            }
            if !(clock.now() < deadline || gone > 0) {
                break;
            }
        }
    }
    Err(EngineError::machine_alive(machine.id(), alive))
}

/// golden.ts:264. A snapshot's delete: gone, already missing, or still listed when the window it was read for ran out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotDeleted {
    Deleted,
    Missing,
    Listed { grace_ms: u64 },
}

/// golden.ts:269. A snapshot's delete is read back the way a machine's kill is, under the same window: the listing is
/// best-effort and may still hold an id the delete took, so it is read until the id leaves, and a read that fails counts
/// as one that still holds it. A backend with no listing answers on the delete alone; a refusal other than missing fails.
pub async fn snapshot_until_gone(
    backend: &dyn MachineBackend,
    snapshot_id: &str,
    confirm: KillConfirm,
    clock: &dyn RetryClock,
) -> Result<SnapshotDeleted> {
    match backend.delete_snapshot(snapshot_id).await {
        Ok(()) => {}
        Err(e) if is_missing(&e) => return Ok(SnapshotDeleted::Missing),
        Err(e) => return Err(e),
    }
    if !backend.capabilities().snapshot_listing {
        return Ok(SnapshotDeleted::Deleted);
    }
    let grace_ms = confirm.grace_ms.unwrap_or(KILL_GRACE_MS);
    let deadline = clock.now() + grace_ms as i64;
    let ids = [snapshot_id.to_string()];
    loop {
        // Asked for this id, Boat reads its named snapshots alone, not the account's whole history.
        if let Ok(rows) = backend.list_snapshots(Some(&ids[..])).await {
            if !rows.iter().any(|r| r.id == snapshot_id) {
                return Ok(SnapshotDeleted::Deleted);
            }
        }
        clock.sleep(confirm.poll_ms.unwrap_or(POLL_MS)).await;
        if clock.now() >= deadline {
            return Ok(SnapshotDeleted::Listed { grace_ms });
        }
    }
}
