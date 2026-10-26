// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/errors.ts. Spec: spec/errors.md.
//!
//! The TS throws plain `WspError` objects, a dozen `Error` subclasses and whatever fetch rejects with, and its readers
//! tell them apart by `kind`, `status`, `name`, `code` and `instanceof`. Here every one is a variant of
//! [`EngineError`], and the readers are methods on it.
use std::time::Duration;

use serde::{Deserialize, Serialize};
use wsp_words::exit::{HostError, Kind};
use wsp_words::words::thread::{exec_failed_line, guest_unusable_line, machine_unreached_line, nap_refused_line, stop_refused_line};

pub use wsp_words::words::units::StartWindow;

use crate::cancel::Cancel;
use crate::fetch::Abort;

/// errors.ts:5. `Concurrency` is the provider's machine cap alone; `StartLimit` is a window of starts used, which
/// waiting on a slot never ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    Concurrency,
    StartLimit,
    Plan,
    Missing,
    Conflict,
    SnapshotUnavailable,
    Transient,
    Auth,
    Unknown,
}

/// errors.ts:11. A provider's answered refusal. `request_id` is the id the reply carried; `window` is the one a
/// `StartLimit` met; `said` is the provider's own words where `message` is wsp's line instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WspError {
    pub kind: ErrorKind,
    pub status: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<StartWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub said: Option<String>,
}

/// What a refusal carried over from the provider's answer onto a wrapping error (errors.ts:47 and :61): the four
/// fields `Object.assign` copies, each absent where the cause had none.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Carried {
    pub kind: Option<ErrorKind>,
    pub status: Option<u16>,
    pub code: Option<String>,
    pub request_id: Option<String>,
}

/// A failure under HTTP, as node's fetch hands it: the error's `name` (`TypeError` for "fetch failed", `TimeoutError`
/// for a cap, `AbortError` for a caller's cancel, `Error` otherwise), its message, its own `code`, and its cause.
#[derive(Debug, Clone, PartialEq)]
pub struct RoadError {
    pub name: String,
    pub message: String,
    pub code: Option<String>,
    pub cause: Option<RoadCause>,
}

/// A fetch failure's cause: a system error with a code, or node's AggregateError holding one per address tried.
#[derive(Debug, Clone, PartialEq)]
pub struct RoadCause {
    pub message: String,
    pub code: Option<String>,
    /// `Some` exactly when the cause is an AggregateError.
    pub errors: Option<Vec<RoadCause>>,
}

/// Every failure the engine's roads answer. Variant per TS class; `Provider` is the plain `WspError` object.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EngineError {
    /// A provider's answered refusal (errors.ts:11).
    #[error("{}", .0.message)]
    Provider(WspError),
    /// errors.ts:31. The provider never answered a pause's move and the machine did not land; carries the last failure.
    #[error("{message}")]
    MoveUnanswered { message: String, cause: Option<Box<EngineError>> },
    /// errors.ts:36. A resume the provider never took inside its cap; a MoveUnanswered to every reader of that class.
    #[error("{message}")]
    ResumeUnanswered { message: String, cause: Option<Box<EngineError>> },
    /// errors.ts:41. The provider still said not yet at the restore deadline; the message is the cause's.
    #[error("{message}")]
    RestoreUnfinished { machine_id: String, message: String, carried: Carried },
    /// errors.ts:55. A command refused because the machine is stopped; the message is the cause's.
    #[error("{message}")]
    MachineStopped { machine_id: String, message: String, carried: Carried },
    /// errors.ts:69. kind "notFirstLife".
    #[error("{action} refused: machine {machine_id} is not first-life (it was resumed); snapshots only come from fresh machines")]
    NotFirstLife { machine_id: String, action: String },
    /// errors.ts:82.
    #[error("environment did not land on {machine_id} (exit {exit_code}): {detail}")]
    EnvNotLanded { machine_id: String, exit_code: i64, detail: String },
    /// errors.ts:95. `line` is the backend's sentence; `said` the provider's words.
    #[error("{line}")]
    DiskGone { machine_id: String, line: String, said: String },
    /// errors.ts:107.
    #[error("{line}")]
    CreateCancelled { machine_id: String, why: String, line: String },
    /// errors.ts:121. Message is `guest_unusable_line(provider, machine_id, detail)`.
    #[error("{line}")]
    GuestUnusable { machine_id: String, provider: String, detail: String, status: u16, line: String },
    /// errors.ts:142, and ExecFailedError (errors.ts:157) where `exec_failed` is true: a MachineUnreachable to every
    /// reader of that class, with `exec_failed_line(said)` as its line.
    #[error("{line}")]
    MachineUnreachable { machine_id: String, said: String, status: u16, line: String, exec_failed: bool },
    /// errors.ts:166. Message is `stop_refused_line(said)`.
    #[error("{line}")]
    StopRefused { machine_id: String, said: String, line: String },
    /// errors.ts:177. Message is `nap_refused_line(said)`.
    #[error("{line}")]
    NapRefused { machine_id: String, said: String, line: String },
    /// golden.ts:45. A machine that answered `KILL_ASKS` kills with a success status and is still there, in the state it
    /// last read; kind "machineAlive".
    #[error("machine {machine_id} is still {} after three kills; it bills until reap or a kill by hand takes", crate::gone::machine_state_word(*.state))]
    MachineAlive { machine_id: String, state: crate::machine::MachineState },
    /// errors.ts:299. `untilReached` gave up; message is `machine_unreached_line(attempts, elapsed_ms)`.
    #[error("{line}")]
    Unreached { attempts: u32, elapsed_ms: u64, line: String, cause: Box<EngineError> },
    /// A failure under HTTP or a socket, as fetch rejects with it.
    #[error("{}", .0.message)]
    Road(RoadError),
    /// vault.ts: a vault past its cap; readers tell it by kind "vaultTooLarge".
    #[error("{message}")]
    VaultTooLarge { message: String, bytes: u64 },
    /// vault.ts: kind "exists".
    #[error("{message}")]
    Exists { message: String },
    /// no-provider-backend.ts:20. The refusal of every road on a host with no provider key; kind "noProvider".
    #[error("{line}")]
    NoProvider { line: String },
    /// box-backend.ts:837. A size the plan does not sell; kind "invalid".
    #[error("{message}")]
    Invalid { message: String },
    /// link-backend.ts:91. kind "absent", read by `is_place_absent`; carries no code.
    #[error("{message}")]
    PlaceAbsent { message: String },
    /// ssh-backend.ts:211. A sentence the caller says whole.
    #[error("{message}")]
    MissingKnownHosts { message: String },
    /// Any other thrown Error: its message, and its `code` where it had one.
    #[error("{message}")]
    Other { message: String, code: Option<String> },
}

pub type Result<T, E = EngineError> = std::result::Result<T, E>;

/// errors.ts:13. A provider's HTTP status and body (`code`, `error`) read as a refusal of a kind.
pub fn classify(status: u16, code: Option<&str>, error: Option<&str>, request_id: Option<&str>) -> WspError {
    let message = error.unwrap_or("").to_string();
    let kind = match status {
        429 => ErrorKind::Concurrency,
        401 => ErrorKind::Auth,
        402 | 403 => ErrorKind::Plan,
        404 => ErrorKind::Missing,
        409 => ErrorKind::Conflict,
        502 if message == "Failed to snapshot sandbox" => ErrorKind::SnapshotUnavailable,
        502..=504 => ErrorKind::Transient,
        _ => ErrorKind::Unknown,
    };
    WspError { kind, status, code: code.map(str::to_string), message, request_id: request_id.map(str::to_string), window: None, said: None }
}

impl EngineError {
    /// errors.ts:121 `new GuestUnusableError(machineId, provider, detail, status)`: the line is
    /// `guest_unusable_line(provider, machine_id, detail)`.
    pub fn guest_unusable(machine_id: &str, provider: &str, detail: &str, status: u16) -> Self {
        EngineError::GuestUnusable {
            machine_id: machine_id.into(),
            provider: provider.into(),
            detail: detail.into(),
            status,
            line: guest_unusable_line(provider, machine_id, detail),
        }
    }
    /// errors.ts:142 `new MachineUnreachableError(machineId, said, status, line)`.
    pub fn machine_unreachable(machine_id: &str, said: &str, status: u16, line: &str) -> Self {
        EngineError::MachineUnreachable { machine_id: machine_id.into(), said: said.into(), status, line: line.into(), exec_failed: false }
    }
    /// errors.ts:157 `new ExecFailedError(machineId, said, status)`: the line is `exec_failed_line(said)`.
    pub fn exec_failed(machine_id: &str, said: &str, status: u16) -> Self {
        EngineError::MachineUnreachable {
            machine_id: machine_id.into(),
            said: said.into(),
            status,
            line: exec_failed_line(said),
            exec_failed: true,
        }
    }
    /// errors.ts:166: the line is `stop_refused_line(said)`.
    pub fn stop_refused(machine_id: &str, said: &str) -> Self {
        EngineError::StopRefused { machine_id: machine_id.into(), said: said.into(), line: stop_refused_line(said) }
    }
    /// errors.ts:177: the line is `nap_refused_line(said)`.
    pub fn nap_refused(machine_id: &str, said: &str) -> Self {
        EngineError::NapRefused { machine_id: machine_id.into(), said: said.into(), line: nap_refused_line(said) }
    }
    /// errors.ts:41: the cause's message, and its kind, status, code and request id where it had them.
    pub fn restore_unfinished(machine_id: &str, said: &EngineError) -> Self {
        EngineError::RestoreUnfinished { machine_id: machine_id.into(), message: said.to_string(), carried: carried_from(said) }
    }
    /// errors.ts:55: as `restore_unfinished`.
    pub fn machine_stopped(machine_id: &str, said: &EngineError) -> Self {
        EngineError::MachineStopped { machine_id: machine_id.into(), message: said.to_string(), carried: carried_from(said) }
    }
    /// golden.ts:45 `new MachineAliveError(machineId, state)`.
    pub fn machine_alive(machine_id: &str, state: crate::machine::MachineState) -> Self {
        EngineError::MachineAlive { machine_id: machine_id.into(), state }
    }
    /// errors.ts:299: the line is `machine_unreached_line(attempts, elapsed_ms)`.
    pub fn unreached(attempts: u32, elapsed_ms: u64, cause: EngineError) -> Self {
        EngineError::Unreached {
            attempts,
            elapsed_ms,
            line: machine_unreached_line(f64::from(attempts), elapsed_ms as f64),
            cause: Box::new(cause),
        }
    }
    /// The TS `kind` field where the error has one: a `Provider`'s kind, the kind a RestoreUnfinished or
    /// MachineStopped carried; "notFirstLife", "vaultTooLarge" and "exists" are kinds of their own and read `None` here.
    pub fn kind(&self) -> Option<ErrorKind> {
        match self {
            EngineError::Provider(w) => Some(w.kind),
            EngineError::RestoreUnfinished { carried, .. } | EngineError::MachineStopped { carried, .. } => carried.kind,
            _ => None,
        }
    }
    /// The TS `status` field where the error has one.
    pub fn status(&self) -> Option<u16> {
        match self {
            EngineError::Provider(w) => Some(w.status),
            EngineError::RestoreUnfinished { carried, .. } | EngineError::MachineStopped { carried, .. } => carried.status,
            EngineError::GuestUnusable { status, .. } | EngineError::MachineUnreachable { status, .. } => Some(*status),
            _ => None,
        }
    }
    /// The TS `code` field where the error has one (a provider's code, a road error's own code).
    pub fn code(&self) -> Option<&str> {
        match self {
            EngineError::Provider(w) => w.code.as_deref(),
            EngineError::RestoreUnfinished { carried, .. } | EngineError::MachineStopped { carried, .. } => carried.code.as_deref(),
            EngineError::Road(r) => r.code.as_deref(),
            EngineError::Other { code, .. } => code.as_deref(),
            _ => None,
        }
    }
    /// The TS `requestId` field where the error has one.
    pub fn request_id(&self) -> Option<&str> {
        match self {
            EngineError::Provider(w) => w.request_id.as_deref(),
            EngineError::RestoreUnfinished { carried, .. } | EngineError::MachineStopped { carried, .. } => carried.request_id.as_deref(),
            _ => None,
        }
    }
    /// The TS error's `name`: "Error" for a plain WspError object read as an Error is never asked; see spec/errors.md.
    pub fn name(&self) -> &str {
        match self {
            EngineError::RestoreUnfinished { .. } => "RestoreUnfinishedError",
            EngineError::MachineStopped { .. } => "MachineStoppedError",
            EngineError::NotFirstLife { .. } => "NotFirstLifeError",
            EngineError::EnvNotLanded { .. } => "EnvNotLandedError",
            EngineError::DiskGone { .. } => "DiskGoneError",
            EngineError::CreateCancelled { .. } => "CreateCancelledError",
            EngineError::GuestUnusable { .. } => "GuestUnusableError",
            EngineError::MachineUnreachable { exec_failed: true, .. } => "ExecFailedError",
            EngineError::MachineUnreachable { .. } => "MachineUnreachableError",
            EngineError::StopRefused { .. } => "StopRefusedError",
            EngineError::NapRefused { .. } => "NapRefusedError",
            EngineError::MachineAlive { .. } => "MachineAliveError",
            EngineError::NoProvider { .. } => "NoProviderError",
            EngineError::Road(r) => &r.name,
            _ => "Error",
        }
    }
    /// `instanceof MoveUnansweredError`: true of ResumeUnanswered too.
    pub fn is_move_unanswered(&self) -> bool {
        matches!(self, EngineError::MoveUnanswered { .. } | EngineError::ResumeUnanswered { .. })
    }
    /// `instanceof MachineUnreachableError`: true of ExecFailed too.
    pub fn is_machine_unreachable(&self) -> bool {
        matches!(self, EngineError::MachineUnreachable { .. })
    }
}

/// The error as every door reads it: its words, the kind it was born with (exit.ts:37 reads the exit class off it), and
/// the status and code a provider's answer carried.
impl From<EngineError> for HostError {
    fn from(e: EngineError) -> HostError {
        let kind = match &e {
            EngineError::NotFirstLife { .. } => Some(Kind::NotFirstLife),
            EngineError::MachineAlive { .. } => Some(Kind::MachineAlive),
            EngineError::VaultTooLarge { .. } => Some(Kind::VaultTooLarge),
            EngineError::Exists { .. } => Some(Kind::Exists),
            EngineError::NoProvider { .. } => Some(Kind::NoProvider),
            EngineError::Invalid { .. } => Some(Kind::Invalid),
            EngineError::PlaceAbsent { .. } => Some(Kind::Other("absent".to_owned())),
            other => other.kind().and_then(|kind| serde_json::to_value(kind).ok()).and_then(|word| word.as_str().map(Kind::parse)),
        };
        HostError { kind, status: e.status().map(f64::from), code: e.code().map(str::to_owned), ..HostError::new(e.to_string()) }
    }
}

/// What a wrapper copies off the failure it wraps (errors.ts:47 and :61).
fn carried_from(said: &EngineError) -> Carried {
    Carried {
        kind: said.kind(),
        status: said.status(),
        code: said.code().map(str::to_string),
        request_id: said.request_id().map(str::to_string),
    }
}

/// errors.ts:189. The provider answered 404 for the machine.
pub fn is_missing(e: &EngineError) -> bool {
    e.kind() == Some(ErrorKind::Missing)
}

/// errors.ts:194. A bad key or a plan that does not allow the call.
pub fn is_account_refusal(e: &EngineError) -> bool {
    matches!(e.kind(), Some(ErrorKind::Auth | ErrorKind::Plan))
}

/// errors.ts:199.
pub fn should_retry(e: &WspError, attempt: u32) -> bool {
    e.kind == ErrorKind::Transient && attempt < 3
}

/// errors.ts:203. `random` is `Math.random()`'s draw in [0, 1).
pub fn backoff_ms(attempt: u32, random: f64) -> u64 {
    let base = (500.0 * 2f64.powf(f64::from(attempt))).min(8000.0);
    base as u64 + (random * 250.0).floor() as u64
}

/// errors.ts:205. The network codes `is_network_error` reads, in the TS's order.
pub const NETWORK_CODES: [&str; 8] =
    ["EAI_AGAIN", "ENOTFOUND", "ECONNRESET", "ECONNREFUSED", "ETIMEDOUT", "EPIPE", "UND_ERR_SOCKET", "UND_ERR_CONNECT_TIMEOUT"];

/// The code of the cause under an error: a road's cause, or the error a wrapper was thrown over.
fn cause_code(e: &EngineError) -> Option<&str> {
    match e {
        EngineError::Road(r) => r.cause.as_ref().and_then(|c| c.code.as_deref()),
        EngineError::MoveUnanswered { cause, .. } | EngineError::ResumeUnanswered { cause, .. } => {
            cause.as_deref().and_then(EngineError::code)
        }
        EngineError::Unreached { cause, .. } => cause.code(),
        _ => None,
    }
}

/// errors.ts:208.
pub fn is_network_error(e: &EngineError) -> bool {
    e.to_string() == "fetch failed"
        || e.code().is_some_and(|c| NETWORK_CODES.contains(&c))
        || cause_code(e).is_some_and(|c| NETWORK_CODES.contains(&c))
}

/// errors.ts:215.
pub const UNSENT_CODES: [&str; 4] = ["EAI_AGAIN", "ENOTFOUND", "ECONNREFUSED", "UND_ERR_CONNECT_TIMEOUT"];

/// errors.ts:217.
pub fn never_sent(e: &EngineError) -> bool {
    e.code().is_some_and(|c| UNSENT_CODES.contains(&c)) || cause_code(e).is_some_and(|c| UNSENT_CODES.contains(&c))
}

/// errors.ts:225.
pub fn far_end_answered(e: &EngineError) -> bool {
    match e {
        EngineError::Provider(_) | EngineError::GuestUnusable { .. } | EngineError::MachineUnreachable { .. } => true,
        EngineError::RestoreUnfinished { carried, .. } | EngineError::MachineStopped { carried, .. } => {
            carried.kind.is_some() || carried.status.is_some()
        }
        EngineError::NotFirstLife { .. }
        | EngineError::VaultTooLarge { .. }
        | EngineError::Exists { .. }
        | EngineError::NoProvider { .. }
        | EngineError::Invalid { .. }
        | EngineError::PlaceAbsent { .. } => true,
        _ => false,
    }
}

/// errors.ts:235. The call was cut off by its caller's cap: name "TimeoutError".
pub fn is_capped(e: &EngineError) -> bool {
    e.name() == "TimeoutError"
}

/// errors.ts:243.
pub const TIMER_CAP_MS: u64 = 2_147_483_647;

/// errors.ts:250. `ms` as a whole number inside the timer's range, rounded up, never below 0.
pub fn fetch_cap_ms(ms: f64) -> u64 {
    ms.ceil().clamp(0.0, TIMER_CAP_MS as f64) as u64
}

/// errors.ts:257.
pub const PROVIDER_READ_CAP_MS: u64 = 20_000;

/// errors.ts:261. What a fetch is given to end it early: a fresh cap per call, the caller's signal, or both, as one
/// token; `None` where neither is given. The cap's firing reads as "TimeoutError", the caller's as "AbortError".
pub fn abort(cap_ms: Option<f64>, signal: Option<&Cancel>) -> Option<Abort> {
    if cap_ms.is_none() && signal.is_none() {
        return None;
    }
    Some(Abort::new(cap_ms.map(|ms| Duration::from_millis(fetch_cap_ms(ms))), signal))
}

/// errors.ts:271.
pub const OFFLINE_CODES: [&str; 7] = ["ENOTFOUND", "EAI_AGAIN", "EAI_FAIL", "ENETUNREACH", "ENETDOWN", "EHOSTUNREACH", "EHOSTDOWN"];

/// errors.ts:275.
pub fn road_code(e: &EngineError) -> Option<String> {
    let EngineError::Road(r) = e else { return None };
    if r.name != "TypeError" || r.message != "fetch failed" {
        return None;
    }
    let codes: Vec<Option<&str>> = match &r.cause {
        None => vec![None],
        Some(c) => match &c.errors {
            Some(each) => each.iter().map(|x| x.code.as_deref()).collect(),
            None => vec![c.code.as_deref()],
        },
    };
    if codes.is_empty() || !codes.iter().all(|c| c.is_some_and(|c| OFFLINE_CODES.contains(&c))) {
        return None;
    }
    codes[0].map(str::to_string)
}

/// errors.ts:285.
pub fn road_failed(e: &EngineError) -> bool {
    road_code(e).is_some()
}

/// errors.ts:292.
pub const ROAD_TRIES: u32 = 3;

/// errors.ts:294. `random` is `Math.random()`'s draw in [0, 1).
pub fn road_backoff_ms(retry: u32, random: f64) -> u64 {
    u64::from(retry) * 3000 + (random * 250.0).floor() as u64
}

/// errors.ts:310. The clock a retry runs on; tests hand in one they move by hand.
#[async_trait::async_trait(?Send)]
pub trait RetryClock {
    fn now(&self) -> i64;
    async fn sleep(&self, ms: u64);
    /// `Math.random()`, which `link_backoff_ms` draws from.
    fn random(&self) -> f64;
}

/// errors.ts:315.
pub struct RealRetryClock;

#[async_trait::async_trait(?Send)]
impl RetryClock for RealRetryClock {
    fn now(&self) -> i64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
    }
    async fn sleep(&self, ms: u64) {
        tokio::time::sleep(Duration::from_millis(ms)).await
    }
    fn random(&self) -> f64 {
        rand::random::<f64>()
    }
}

/// errors.ts:319. Calls `once` until it answers; a network failure is retried at the link backoff while the next wait
/// still ends inside the link window, then `Unreached`; any other failure is answered at once.
pub async fn until_reached<T, F, Fut>(mut once: F, clock: &dyn RetryClock) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let started = clock.now();
    for attempt in 1.. {
        let e = match once().await {
            Ok(answer) => return Ok(answer),
            Err(e) => e,
        };
        if !is_network_error(&e) {
            return Err(e);
        }
        let wait = wsp_wire::wire::limits::link_backoff_ms(i64::from(attempt), clock.random());
        let elapsed = clock.now() - started;
        if elapsed + wait > wsp_wire::wire::limits::LINK_RETRY_WINDOW_MS {
            return Err(EngineError::unreached(attempt, elapsed.max(0) as u64, e));
        }
        clock.sleep(wait as u64).await;
    }
    unreachable!("the attempt counter never ends")
}

/// errors.ts:335. A failure's sentence, with fetch's cause words after "fetch failed".
pub fn failure_words(e: &EngineError) -> String {
    let EngineError::Road(r) = e else { return e.to_string() };
    let Some(cause) = r.cause.as_ref() else { return r.message.clone() };
    if r.name != "TypeError" || r.message != "fetch failed" {
        return r.message.clone();
    }
    let each = cause.errors.iter().flatten().map(|x| x.message.as_str());
    let said = std::iter::once(cause.message.as_str()).chain(each).filter(|w| !w.is_empty()).collect::<Vec<_>>().join("; ");
    if said.is_empty() {
        r.message.clone()
    } else {
        format!("{}: {said}", r.message)
    }
}

/// What a cancelled `AbortSignal` rejects with when it carries no reason of its own: node's AbortError DOMException,
/// name "AbortError", message "This operation was aborted", no string code. Every road that ends on a caller's
/// cancel answers this.
pub fn aborted() -> EngineError {
    EngineError::Road(RoadError { name: "AbortError".into(), message: crate::cancel::DEFAULT_REASON.into(), code: None, cause: None })
}
