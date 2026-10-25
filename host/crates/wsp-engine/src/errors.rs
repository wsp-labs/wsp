// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/errors.ts. Spec: spec/errors.md.
//!
//! The TS throws plain `WspError` objects, a dozen `Error` subclasses and whatever fetch rejects with, and its readers
//! tell them apart by `kind`, `status`, `name`, `code` and `instanceof`. Here every one is a variant of
//! [`EngineError`], and the readers are methods on it.
use serde::{Deserialize, Serialize};
pub use wsp_words::words::units::StartWindow;

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
    todo!()
}

impl EngineError {
    /// errors.ts:121 `new GuestUnusableError(machineId, provider, detail, status)`: the line is
    /// `guest_unusable_line(provider, machine_id, detail)`.
    pub fn guest_unusable(machine_id: &str, provider: &str, detail: &str, status: u16) -> Self {
        todo!()
    }
    /// errors.ts:142 `new MachineUnreachableError(machineId, said, status, line)`.
    pub fn machine_unreachable(machine_id: &str, said: &str, status: u16, line: &str) -> Self {
        todo!()
    }
    /// errors.ts:157 `new ExecFailedError(machineId, said, status)`: the line is `exec_failed_line(said)`.
    pub fn exec_failed(machine_id: &str, said: &str, status: u16) -> Self {
        todo!()
    }
    /// errors.ts:166: the line is `stop_refused_line(said)`.
    pub fn stop_refused(machine_id: &str, said: &str) -> Self {
        todo!()
    }
    /// errors.ts:177: the line is `nap_refused_line(said)`.
    pub fn nap_refused(machine_id: &str, said: &str) -> Self {
        todo!()
    }
    /// errors.ts:41: the cause's message, and its kind, status, code and request id where it had them.
    pub fn restore_unfinished(machine_id: &str, said: &EngineError) -> Self {
        todo!()
    }
    /// errors.ts:55: as `restore_unfinished`.
    pub fn machine_stopped(machine_id: &str, said: &EngineError) -> Self {
        todo!()
    }
    /// errors.ts:299: the line is `machine_unreached_line(attempts, elapsed_ms)`.
    pub fn unreached(attempts: u32, elapsed_ms: u64, cause: EngineError) -> Self {
        todo!()
    }
    /// The TS `kind` field where the error has one: a `Provider`'s kind, the kind a RestoreUnfinished or
    /// MachineStopped carried; "notFirstLife", "vaultTooLarge" and "exists" are kinds of their own and read `None` here.
    pub fn kind(&self) -> Option<ErrorKind> {
        todo!()
    }
    /// The TS `status` field where the error has one.
    pub fn status(&self) -> Option<u16> {
        todo!()
    }
    /// The TS `code` field where the error has one (a provider's code, a road error's own code).
    pub fn code(&self) -> Option<&str> {
        todo!()
    }
    /// The TS `requestId` field where the error has one.
    pub fn request_id(&self) -> Option<&str> {
        todo!()
    }
    /// The TS error's `name`: "Error" for a plain WspError object read as an Error is never asked; see spec/errors.md.
    pub fn name(&self) -> &str {
        todo!()
    }
    /// `instanceof MoveUnansweredError`: true of ResumeUnanswered too.
    pub fn is_move_unanswered(&self) -> bool {
        todo!()
    }
    /// `instanceof MachineUnreachableError`: true of ExecFailed too.
    pub fn is_machine_unreachable(&self) -> bool {
        todo!()
    }
}

/// errors.ts:189. The provider answered 404 for the machine.
pub fn is_missing(e: &EngineError) -> bool {
    todo!()
}

/// errors.ts:194. A bad key or a plan that does not allow the call.
pub fn is_account_refusal(e: &EngineError) -> bool {
    todo!()
}

/// errors.ts:199.
pub fn should_retry(e: &WspError, attempt: u32) -> bool {
    todo!()
}

/// errors.ts:203. `random` is `Math.random()`'s draw in [0, 1).
pub fn backoff_ms(attempt: u32, random: f64) -> u64 {
    todo!()
}

/// errors.ts:205. The network codes `is_network_error` reads, in the TS's order.
pub const NETWORK_CODES: [&str; 8] =
    ["EAI_AGAIN", "ENOTFOUND", "ECONNRESET", "ECONNREFUSED", "ETIMEDOUT", "EPIPE", "UND_ERR_SOCKET", "UND_ERR_CONNECT_TIMEOUT"];

/// errors.ts:208.
pub fn is_network_error(e: &EngineError) -> bool {
    todo!()
}

/// errors.ts:215.
pub const UNSENT_CODES: [&str; 4] = ["EAI_AGAIN", "ENOTFOUND", "ECONNREFUSED", "UND_ERR_CONNECT_TIMEOUT"];

/// errors.ts:217.
pub fn never_sent(e: &EngineError) -> bool {
    todo!()
}

/// errors.ts:225.
pub fn far_end_answered(e: &EngineError) -> bool {
    todo!()
}

/// errors.ts:235. The call was cut off by its caller's cap: name "TimeoutError".
pub fn is_capped(e: &EngineError) -> bool {
    todo!()
}

/// errors.ts:243.
pub const TIMER_CAP_MS: u64 = 2_147_483_647;

/// errors.ts:250. `ms` as a whole number inside the timer's range, rounded up, never below 0.
pub fn fetch_cap_ms(ms: f64) -> u64 {
    todo!()
}

/// errors.ts:257.
pub const PROVIDER_READ_CAP_MS: u64 = 20_000;

/// errors.ts:261. What a fetch is given to end it early: a fresh cap per call, the caller's signal, or both, as one
/// token; `None` where neither is given. The cap's firing reads as "TimeoutError", the caller's as "AbortError".
pub fn abort(cap_ms: Option<f64>, signal: Option<&tokio_util::sync::CancellationToken>) -> Option<crate::fetch::Abort> {
    todo!()
}

/// errors.ts:271.
pub const OFFLINE_CODES: [&str; 7] = ["ENOTFOUND", "EAI_AGAIN", "EAI_FAIL", "ENETUNREACH", "ENETDOWN", "EHOSTUNREACH", "EHOSTDOWN"];

/// errors.ts:275.
pub fn road_code(e: &EngineError) -> Option<String> {
    todo!()
}

/// errors.ts:285.
pub fn road_failed(e: &EngineError) -> bool {
    todo!()
}

/// errors.ts:292.
pub const ROAD_TRIES: u32 = 3;

/// errors.ts:294. `random` is `Math.random()`'s draw in [0, 1).
pub fn road_backoff_ms(retry: u32, random: f64) -> u64 {
    todo!()
}

/// errors.ts:310. The clock a retry runs on; tests hand in one they move by hand.
#[async_trait::async_trait]
pub trait RetryClock: Send + Sync {
    fn now(&self) -> i64;
    async fn sleep(&self, ms: u64);
    /// `Math.random()`, which `link_backoff_ms` draws from.
    fn random(&self) -> f64;
}

/// errors.ts:315.
pub struct RealRetryClock;

#[async_trait::async_trait]
impl RetryClock for RealRetryClock {
    fn now(&self) -> i64 {
        todo!()
    }
    async fn sleep(&self, ms: u64) {
        todo!()
    }
    fn random(&self) -> f64 {
        todo!()
    }
}

/// errors.ts:319. Calls `once` until it answers; a network failure is retried at the link backoff while the next wait
/// still ends inside the link window, then `Unreached`; any other failure is answered at once.
pub async fn until_reached<T, F, Fut>(once: F, clock: &dyn RetryClock) -> Result<T>
where
    F: FnMut() -> Fut + Send,
    Fut: std::future::Future<Output = Result<T>> + Send,
    T: Send,
{
    todo!()
}

/// errors.ts:335. A failure's sentence, with fetch's cause words after "fetch failed".
pub fn failure_words(e: &EngineError) -> String {
    todo!()
}

/// What a cancelled `AbortSignal` rejects with when it carries no reason of its own: node's AbortError DOMException,
/// name "AbortError", message "This operation was aborted", no string code. Every road that ends on a caller's
/// cancel answers this.
pub fn aborted() -> EngineError {
    todo!()
}
