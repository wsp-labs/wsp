// SPDX-License-Identifier: AGPL-3.0-only
//! The port every harness adapter is written against. TS: packages/protocol/src/adapter-port.ts at a3b870a6e (the
//! stream a turn runs in, the events an adapter reads its CLI's output into, the catalog, title, rename, aside, fork
//! and revert answers) and packages/runtime/src/types/harness.ts:120-330 (HarnessAdapter, HarnessSession and their
//! start options), plus packages/protocol/src/kept-run.ts (KeptAgent, KeptTurn, KeptRun).
//!
//! Async: a TS `Promise<T>` is a [`BoxFuture`]; one awaited by many (`exited`, `finished`) is a [`Shared`] future. A
//! TS `AsyncIterable<string>` is a [`BoxStream`] of `Result<String, ExecFailure>`, the error being what the TS
//! iterator throws. An optional TS method (`writeAfter?`, `attach?`) is a method answering `None` where it is absent.

use std::sync::Arc;

use futures::future::{BoxFuture, Shared};
use futures::stream::BoxStream;
use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub use crate::pending::McpServerSpec;
pub use wsp_wire::views::harness::ScreenCommand;
pub use wsp_wire::types::pending_wire::PermissionOption;
pub use wsp_wire::types::usage::HarnessLimit;
pub use wsp_wire::views::session::{PermissionOutcome, SubagentState, SubagentUsage};
pub use wsp_wire::views::session_events::{DeltaKind, FilePatch, PlanStep, SessionHarness, TurnResult, TurnStatus, TurnTokens};

/// A string map whose order is the order the TS object's keys were set in (JSON key order survives).
pub type Env = IndexMap<String, String>;

/// A settled-once value many readers await: the TS `Promise` that `exited` and `finished` are.
pub type SharedFuture<T> = Shared<BoxFuture<'static, T>>;

/// What the person's setup changes about one agent's command. TS adapter-port.ts:23.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentLaunch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
}

/// The agent's command word: `bin`, or the person's program quoted as one word. TS adapter-port.ts:29.
pub fn program_word(bin: &str, launch: Option<&AgentLaunch>) -> String {
    todo!()
}

/// TS adapter-port.ts:32.
pub const AGENT_PID_FILE_ENV: &str = "WSP_AGENT_PID_FILE";

/// The agent's word on a turn's launch: a `sh -c` that writes its pid where the run asks, then execs the agent.
/// TS adapter-port.ts:37, byte for byte.
pub fn agent_word(bin: &str, launch: Option<&AgentLaunch>) -> String {
    todo!()
}

/// The setup's added words, each shell-quoted. TS adapter-port.ts:41.
pub fn launch_words(launch: Option<&AgentLaunch>) -> Vec<String> {
    todo!()
}

/// TS adapter-port.ts:46-47.
pub const PERMISSION_ALLOW: &str = "allow";
pub const PERMISSION_DENY: &str = "deny";

/// One permission prompt a harness raised mid-turn. TS adapter-port.ts:52.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionAsk {
    pub ask_id: String,
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_tool_use_id: Option<String>,
    /// The tool's input as the CLI sent it, JSON text.
    pub input: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub options: Vec<PermissionOption>,
}

/// One `turn.delta`. Its own struct, since adapters build, hold and rewrite them (TS `TurnDelta` in
/// adapter-claude/src/handback.ts:12). TS adapter-port.ts:80.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnDelta {
    pub session_id: String,
    pub kind: DeltaKind,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<Vec<FilePatch>>,
}

impl TurnDelta {
    /// A delta with only the three fields every delta carries.
    pub fn new(session_id: impl Into<String>, kind: DeltaKind, text: impl Into<String>) -> Self {
        TurnDelta { session_id: session_id.into(), kind, text: text.into(), message_id: None, tool_name: None, tool_use_id: None, is_error: None, parent_tool_use_id: None, cwd: None, bytes: None, exit_code: None, duration_ms: None, patch: None }
    }
}

/// What a TS adapter throws synchronously, in its own words (`new Error(message)`); callers read the message alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterError(pub String);

impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AdapterError {}

/// The wall clock in ms epoch, the TS `Date.now()`. Deps take one so a test can pin it; `system_now()` where absent.
pub type Now = Arc<dyn Fn() -> i64 + Send + Sync>;

/// The system clock as a [`Now`].
pub fn system_now() -> Now {
    todo!()
}

/// What failed a run's stream, as its transport threw it. The runtime reads `kind`, never `message` (TS
/// adapter-port.ts:210: "read off its class and never its words").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecFailure {
    pub kind: ExecFailureKind,
    pub message: String,
}

/// The TS classes a run's transport throws, one variant each. The runtime's only reader is `computerSilent`
/// (runtime threads.ts:67), which retries a message whose computer said nothing: every kind but `Other`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecFailureKind {
    /// `MachineUnreached` (engine errors.ts:299): the posts went out and the reach window passed with no answer.
    Unreached,
    /// `MachineUnreachableError` (engine errors.ts:142): the provider answered that the machine cannot be reached.
    Unreachable,
    /// `PlaceAbsentError` (engine link-backend.ts:89, read by `isPlaceAbsent`): the computer holding it is not connected.
    PlaceAbsent,
    /// `LaunchUnanswered` (runtime machine-exec.ts:85): nothing answered the launch and the machine holds no such run.
    LaunchUnanswered,
    /// `DeadlineError` (runtime types/wiring.ts:940): a deadline passed before the call settled.
    Deadline,
    /// `DaemonUnanswered` (runtime daemon-channel.ts:30): the link to the daemon gave out with no answer from it.
    NoAnswer,
    /// Any other throw: a cut's own words, a refused launch, the turn limit.
    Other,
}

impl ExecFailureKind {
    /// The runtime's `computerSilent`: true for every kind but `Other`.
    pub fn silent(self) -> bool {
        todo!()
    }
}

impl std::fmt::Display for ExecFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ExecFailure {}

/// The events an adapter reads its CLI's output into. Serialized with `type` first, then the fields in the TS
/// declaration's order. TS adapter-port.ts:67-213.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AdapterEvent {
    #[serde(rename = "session.start", rename_all = "camelCase")]
    SessionStart {
        session_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tools: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        harness: Option<SessionHarness>,
        /// `Some(false)` only: the TS type is `prompted?: false`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompted: Option<bool>,
    },
    #[serde(rename = "turn.delta")]
    TurnDelta(TurnDelta),
    #[serde(rename = "turn.done", rename_all = "camelCase")]
    TurnDone {
        session_id: String,
        result: TurnResult,
        /// `Some(true)` only.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        held: Option<bool>,
    },
    #[serde(rename = "turn.compacted", rename_all = "camelCase")]
    TurnCompacted {
        session_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        before: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<f64>,
    },
    #[serde(rename = "turn.retry", rename_all = "camelCase")]
    TurnRetry {
        session_id: String,
        attempt: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        of: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        why: Option<String>,
    },
    #[serde(rename = "turn.plan", rename_all = "camelCase")]
    TurnPlan {
        session_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        steps: Option<Vec<PlanStep>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    #[serde(rename = "turn.tasks", rename_all = "camelCase")]
    TurnTasks {
        session_id: String,
        running: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        replied: Option<TurnResult>,
    },
    #[serde(rename = "turn.unread", rename_all = "camelCase")]
    TurnUnread { session_id: String, ids: Vec<String> },
    #[serde(rename = "turn.anchor", rename_all = "camelCase")]
    TurnAnchor {
        session_id: String,
        anchor: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kept: Option<String>,
    },
    #[serde(rename = "limit", rename_all = "camelCase")]
    Limit { session_id: String, limit: HarnessLimit },
    #[serde(rename = "turn.usage", rename_all = "camelCase")]
    TurnUsage {
        session_id: String,
        tokens: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        window: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<f64>,
    },
    #[serde(rename = "subagent", rename_all = "camelCase")]
    Subagent {
        session_id: String,
        task: String,
        state: SubagentState,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_tool_use_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        depth: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        asked: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        usage: Option<SubagentUsage>,
    },
    #[serde(rename = "permission.ask", rename_all = "camelCase")]
    PermissionAsk { session_id: String, ask: PermissionAsk },
    #[serde(rename = "permission.close", rename_all = "camelCase")]
    PermissionClose {
        session_id: String,
        ask_id: String,
        outcome: PermissionOutcome,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        option_id: Option<String>,
    },
    #[serde(rename = "session.end", rename_all = "camelCase")]
    SessionEnd {
        session_id: String,
        /// `None` is the TS `null`: the process ended with no code (a signal).
        exit_code: Option<i32>,
        saw_result: bool,
        /// Never serialized: the runtime reads its kind in process.
        #[serde(skip)]
        failure: Option<ExecFailure>,
    },
}

impl AdapterEvent {
    /// The `sessionId` every event carries.
    pub fn session_id(&self) -> &str {
        todo!()
    }
}

/// Where adapter events go: called once per event, in order, on the reader's own task.
pub type OnEvent = Arc<dyn Fn(AdapterEvent) + Send + Sync>;

/// TS adapter-port.ts:217.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum TaskStop {
    Accepted,
    NotRunning,
    Refused { error: String },
    Unsupported,
}

/// Re-opening a turn the harness still runs on the machine. TS adapter-port.ts:222.
#[derive(Clone)]
pub struct AdapterAttachOptions {
    pub run: String,
    pub session_id: String,
    /// ms epoch, off the row.
    pub started_at: f64,
    pub model: Option<String>,
    pub cwd: Option<String>,
    pub prompt: Option<String>,
    pub effort: Option<String>,
    /// Byte of the run's log the turn starts at; `None` reads from byte 0.
    pub from: Option<u64>,
    pub steered: Option<Vec<String>>,
    pub on_event: OnEvent,
}

/// How a harness takes an image with a turn. TS adapter-port.ts:248.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentRoad {
    Inline,
    File,
}

/// One image of a turn. TS adapter-port.ts:252.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnImage {
    pub media_type: String,
    /// base64, as the client sent it.
    pub bytes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// What a write to a run's input came to. TS adapter-port.ts:276 (`"written" | "gone"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    Written,
    Gone,
}

/// A TS `Promise<void>` that may reject: `Ok(())` when it resolves, `Err(words)` when it rejects.
pub type Settle = BoxFuture<'static, Result<(), String>>;

/// The process a turn runs in, whatever launched it. TS adapter-port.ts:260.
pub trait ExecStream: Send + Sync {
    /// The run's stdout, one line per item; an `Err` is the TS iterator's throw and ends the stream. Taken once: a
    /// second call answers an empty stream.
    fn lines(&self) -> BoxStream<'static, Result<String, ExecFailure>>;
    /// What a later host attaches to this run by; `None` where runs die with their host.
    fn run(&self) -> Option<String> {
        None
    }
    /// The process this run leads on the host's own computer; `None` on another machine.
    fn pid(&self) -> Option<u32> {
        None
    }
    /// SIGTERM to the process and everything it started.
    fn teardown(&self);
    /// SIGKILL to the process and everything it started.
    fn kill(&self);
    /// Appends one line to stdin; `Ok(Gone)` where the process already ended; `Err` once the stream ended or with no
    /// input channel.
    fn write(&self, line: String) -> BoxFuture<'static, Result<WriteOutcome, ExecFailure>>;
    /// Ends stdin.
    fn close_input(&self);
    /// Holds `line` beside the run until `after` settles, then appends it; an `Err` from `after` drops it. Answers
    /// `None` on a road that holds nothing (TS `writeAfter` absent), and the caller writes the line itself.
    fn write_after(&self, line: String, after: Settle) -> Option<()> {
        let _ = (line, after);
        None
    }
    /// On a run re-opened by attach, every line its input had taken; `None` on a launched run.
    fn taken(&self) -> Option<Vec<String>> {
        None
    }
    /// The turn on a multi-turn process is over; `None` where the road has no such call.
    fn rest(&self) -> Option<()> {
        None
    }
    /// The process's next turn starts now; answers the log's byte count before it. `None` where absent.
    fn next_turn(&self) -> Option<u64> {
        None
    }
    /// Settles with the exit code, `None` for an exit with no code.
    fn exited(&self) -> SharedFuture<Option<i32>>;
    /// The signal that ended the run, once `exited` settled; `None` otherwise.
    fn signalled(&self) -> Option<String> {
        None
    }
}

/// Secret text a launch carries. TS adapter-port.ts:315.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExecSecret {
    /// Each lands as a file of the run's own; the variable it is keyed by carries the path.
    pub files: Option<Env>,
    /// The seed lines land the same way.
    pub input: bool,
}

/// The options a launch takes. TS adapter-port.ts:303.
#[derive(Default)]
pub struct ExecOptions {
    pub env: Env,
    /// `Some`: stdin is a line channel seeded with these; `None`: no channel.
    pub input: Option<Vec<String>>,
    /// The seed reaches the channel once this settles; an `Err` kills the run.
    pub input_after: Option<Settle>,
    pub secret: Option<ExecSecret>,
}

/// TS adapter-port.ts:326.
#[derive(Clone, Debug, PartialEq)]
pub struct AttachRunOptions {
    pub input: bool,
    pub started_at: f64,
    pub from: Option<u64>,
}

/// What `attach` found on the machine.
pub enum Attached {
    Stream(Arc<dyn ExecStream>),
    Gone,
}

/// How a turn's process is launched. TS adapter-port.ts:300.
pub trait ExecStreamFactory: Send + Sync {
    fn launch(&self, command: &str, options: ExecOptions) -> Arc<dyn ExecStream>;
    /// `None` where runs die with their host (TS `attach` absent).
    fn attach(&self, run: &str, options: AttachRunOptions) -> Option<BoxFuture<'static, Result<Attached, ExecFailure>>> {
        let _ = (run, options);
        None
    }
    /// `None` where runs die with their host (TS `sweep` absent).
    fn sweep(&self, keep: &[String]) -> Option<BoxFuture<'static, Result<Vec<String>, ExecFailure>>> {
        let _ = keep;
        None
    }
}

/// Whether `turn` settled, either way, inside `ms`. TS adapter-port.ts:349.
pub async fn settles_within<T>(turn: BoxFuture<'_, T>, ms: u64) -> bool {
    todo!()
}

/// SIGTERM, then SIGKILL once `grace_ms` passed with the process still there; settles when it is gone. TS
/// adapter-port.ts:362.
pub async fn end_run(stream: &dyn ExecStream, grace_ms: u64) {
    todo!()
}

/// Waits `wait_ms` for the process to go on its own, then `end_run`. TS adapter-port.ts:374.
pub async fn end_after_result(stream: &dyn ExecStream, wait_ms: u64, grace_ms: u64) {
    todo!()
}

/// One model as an adapter reads it off its binary. TS adapter-port.ts:380.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessCatalogModelProbe {
    pub slug: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// `None`: the binary does not say, every effort open. `Some(vec![])`: it takes none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub efforts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_effort: Option<String>,
    pub context_windows: Vec<String>,
    pub is_default: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast: Option<bool>,
}

/// TS adapter-port.ts:396.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessCatalogProbe {
    pub version: Option<String>,
    pub models: Vec<HarnessCatalogModelProbe>,
    pub efforts: Vec<String>,
    pub permission_modes: Vec<String>,
}

/// TS adapter-port.ts:411: lists, a named refusal, or nothing (`None` at the call site).
#[derive(Clone, Debug, PartialEq)]
pub enum HarnessCatalogAnswer {
    Probe(HarnessCatalogProbe),
    Refused(String),
}

/// One command on the machine a harness runs on, answering its stdout; `env` rides the command's input. TS
/// adapter-port.ts:421. Any `Fn(&str, Option<&Env>) -> BoxFuture<..>` is one.
pub trait HarnessExec: Send + Sync {
    fn exec(&self, command: &str, env: Option<&Env>) -> BoxFuture<'static, Result<String, ExecFailure>>;
}

impl<F> HarnessExec for F
where
    F: Fn(&str, Option<&Env>) -> BoxFuture<'static, Result<String, ExecFailure>> + Send + Sync,
{
    fn exec(&self, command: &str, env: Option<&Env>) -> BoxFuture<'static, Result<String, ExecFailure>> {
        self(command, env)
    }
}

/// TS adapter-port.ts:438.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SessionRenameWrite {
    Written,
    NoSession,
    Failed { error: String },
}

/// TS adapter-port.ts:452.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TitleTurn {
    pub opening: String,
    pub reply: Option<String>,
    pub model: Option<String>,
}

/// TS adapter-port.ts:470.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DraftAsk {
    pub prompt_file: String,
    pub model: Option<String>,
}

/// Values for an agent's own MCP servers for one launch. TS adapter-port.ts:499 and harness.ts:281.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServerValues {
    /// Whole server entries by name, each a JSON object in its own key order.
    pub entries: Option<IndexMap<String, IndexMap<String, Value>>>,
    /// Config keys, for an agent that takes keys as its thread starts (Codex).
    pub config: Option<Env>,
}

/// A question asked beside a thread. TS adapter-port.ts:485.
#[derive(Clone, Default)]
pub struct AsideQuestion {
    pub session: String,
    pub question: String,
    pub cut: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub context_window: Option<String>,
    pub fast: Option<bool>,
    pub mcp_servers: Option<IndexMap<String, McpServerSpec>>,
    pub server_values: Option<ServerValues>,
    pub on_text: Option<Arc<dyn Fn(String) + Send + Sync>>,
}

/// TS adapter-port.ts:505.
#[derive(Clone, Debug, PartialEq)]
pub struct AsideAnswer {
    pub text: String,
    /// The result's `usage` object as the harness wrote it.
    pub usage: Option<IndexMap<String, Value>>,
}

/// TS adapter-port.ts:533.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CountedTurn {
    pub anchor: Option<String>,
    pub result: Option<TurnResult>,
}

/// Where a revert cuts. TS adapter-port.ts:529.
#[derive(Clone, Debug, PartialEq)]
pub enum RevertAt {
    BeforeTurn(String),
    Turns(Vec<CountedTurn>),
}

/// TS adapter-port.ts:529.
#[derive(Clone, Debug, PartialEq)]
pub struct RevertAsk {
    pub session: String,
    pub cwd: Option<String>,
    pub at: RevertAt,
}

/// What a revert came to: done, or `Kept(why)`. TS adapter-port.ts:529 (`void | { kept }`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reverted {
    Cut,
    Kept(String),
}

/// TS adapter-port.ts:540.
#[derive(Clone, Debug, PartialEq)]
pub enum ForkTurn {
    Anchor(String),
    After(Vec<CountedTurn>),
}

/// What a fork's start takes. TS adapter-port.ts:544.
pub enum ForkedSession {
    /// A copied session a start resumes; `drop` removes it where the start never ran.
    Resume { resume: String, drop: Box<dyn FnOnce() -> BoxFuture<'static, Result<(), String>> + Send> },
    /// The start forks itself.
    Fork { session: String, turn: String },
}

/// Where a reset script runs. TS adapter-port.ts:556.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResetRoad {
    pub home: String,
    pub env: Option<IndexMap<String, Option<String>>>,
    pub launch: Option<AgentLaunch>,
}

/// TS adapter-port.ts:564.
#[derive(Clone, Debug, PartialEq)]
pub struct ResetReading {
    pub keyed: bool,
    pub limit: Option<HarnessLimit>,
}

/// TS adapter-port.ts:571.
#[derive(Clone, Debug, PartialEq)]
pub enum ResetSpend {
    Unanswered,
    Outcome { outcome: String, limit: Option<HarnessLimit> },
    Refused { refused: String, too_old: bool },
}

/// TS adapter-port.ts:577.
pub trait PlanResets: Send + Sync {
    /// `Err` where the TS throws building the script (a launch the road cannot run).
    fn read_command(&self, road: &ResetRoad) -> Result<String, AdapterError>;
    fn parse_read(&self, stdout: &str) -> Option<ResetReading>;
    fn spend_command(&self, road: &ResetRoad, idempotency_key: &str, credit_id: Option<&str>) -> Result<String, AdapterError>;
    fn parse_spend(&self, stdout: &str) -> ResetSpend;
    fn too_old(&self) -> &str;
}

/// What a start takes. TS runtime/src/types/harness.ts:249 (HarnessStartOptions); adapter-claude's StartOptions is
/// the same less `fork`, `limitDetails` and `serverValues.config`.
#[derive(Default)]
pub struct HarnessStartOptions {
    pub prompt: String,
    pub resume: Option<String>,
    /// `true` only where the TS sets `copy: true`.
    pub copy: bool,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: Option<String>,
    pub context_window: Option<String>,
    pub fast: Option<bool>,
    pub title: Option<String>,
    pub images: Vec<TurnImage>,
    pub fork: Option<(String, String)>,
    pub mcp_servers: Option<IndexMap<String, McpServerSpec>>,
    pub server_values: Option<ServerValues>,
    pub off_servers: Option<Vec<String>>,
    /// Asked for the thread so far as text, only where the CLI holds no session under `resume`.
    pub seed: Option<Arc<dyn Fn() -> BoxFuture<'static, Result<String, String>> + Send + Sync>>,
    pub limit_details: bool,
    pub prompt_after: Option<Settle>,
    pub version: Option<String>,
    pub keep: bool,
    pub on_event: Option<OnEvent>,
}

/// A launch ahead of a thread's first send. TS harness.ts:154.
#[derive(Default)]
pub struct HarnessWarmOptions {
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: Option<String>,
    pub context_window: Option<String>,
    pub fast: Option<bool>,
    pub mcp_servers: Option<IndexMap<String, McpServerSpec>>,
    pub server_values: Option<ServerValues>,
    pub version: Option<String>,
}

/// TS runtime harness.ts: `"accepted" | "not-running"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SteerOutcome {
    Accepted,
    NotRunning,
}

/// TS adapter-claude adapter.ts:68.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnswerOutcome {
    Answered,
    Gone,
}

/// TS adapter-claude adapter.ts:74.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessOutcome {
    Set,
    Refused,
    Gone,
}

/// An answer to a permission prompt. TS harness.ts:181.
#[derive(Clone, Debug, PartialEq)]
pub struct PermissionAnswer {
    pub option_id: String,
    pub outcome: PermissionOutcome,
    pub deny_message: String,
    pub reason: Option<String>,
}

/// One session file as a resume elsewhere needs it. TS harness.ts:273.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionFile {
    pub folder: String,
    pub name: String,
    pub depth: u32,
}

/// One turn of a harness. TS runtime harness.ts:156 (HarnessSession). Optional TS methods answer `None` where absent.
pub trait HarnessSession: Send + Sync {
    fn local_id(&self) -> String;
    fn finished(&self) -> SharedFuture<TurnResult>;
    fn run(&self) -> Option<String> {
        None
    }
    fn pid(&self) -> Option<u32> {
        None
    }
    fn from(&self) -> Option<u64> {
        None
    }
    fn interrupt(&self) -> BoxFuture<'static, ()>;
    /// A write's rejection (`Err`) is the TS method's own rejection.
    /// `None` on an adapter that keeps no process; `Some(None)` where this turn's process went with it.
    fn kept(&self) -> Option<Option<Box<dyn KeptAgent>>> {
        None
    }
    fn steer(&self, prompt: String, id: Option<String>, images: Vec<TurnImage>) -> Option<BoxFuture<'static, Result<SteerOutcome, ExecFailure>>> {
        let _ = (prompt, id, images);
        None
    }
    fn tells_unread(&self) -> Option<bool> {
        None
    }
    fn answer(&self, ask_id: String, answer: PermissionAnswer) -> Option<BoxFuture<'static, Result<AnswerOutcome, ExecFailure>>> {
        let _ = (ask_id, answer);
        None
    }
    fn set_access(&self, mode: String) -> Option<BoxFuture<'static, Result<AccessOutcome, ExecFailure>>> {
        let _ = mode;
        None
    }
    fn stop_task(&self, task: String) -> Option<BoxFuture<'static, Result<TaskStop, ExecFailure>>> {
        let _ = task;
        None
    }
}

/// What a next turn on a kept process takes. TS kept-run.ts:40.
pub struct KeptTurn {
    pub prompt: String,
    pub images: Vec<TurnImage>,
    pub after: Option<Settle>,
    pub on_event: OnEvent,
}

/// The process behind a thread's turns, kept up between them. TS kept-run.ts:11.
pub trait KeptAgent: Send + Sync {
    fn next(&self, turn: KeptTurn) -> Result<Arc<dyn HarnessSession>, AdapterError>;
    /// `now`: SIGTERM to its tree at once.
    fn close(&self, now: bool) -> BoxFuture<'static, ()>;
    fn exited(&self) -> SharedFuture<()>;
    fn session_file(&self) -> Option<SessionFile> {
        None
    }
    /// `None` where the TS `working` is absent.
    fn working(&self) -> Option<bool> {
        None
    }
}

/// The runtime's view of one harness adapter. TS runtime harness.ts:196. Capability flags the TS declares as
/// `readonly x?: true` are `bool`s here, false where absent.
pub trait HarnessAdapter: Send + Sync {
    /// `Err` where the TS start throws before anything runs (a bad slug, a window with no model, a fork with a resume).
    fn start(&self, options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError>;
    fn warm(&self, options: HarnessWarmOptions) -> Option<Result<Box<dyn KeptAgent>, AdapterError>> {
        let _ = options;
        None
    }
    fn attach(&self, options: AdapterAttachOptions) -> Option<BoxFuture<'static, Result<Option<Arc<dyn HarnessSession>>, ExecFailure>>> {
        let _ = options;
        None
    }
    fn steers(&self) -> bool;
    fn reports_edits(&self) -> bool {
        false
    }
    fn revert(&self, ask: RevertAsk) -> Option<BoxFuture<'static, Result<Reverted, String>>> {
        let _ = ask;
        None
    }
    fn fork_session(&self, session: String, turn: ForkTurn) -> Option<BoxFuture<'static, Result<ForkedSession, String>>> {
        let _ = (session, turn);
        None
    }
    fn forks_by_count(&self) -> bool {
        false
    }
    fn attachments(&self) -> Option<AttachmentRoad> {
        None
    }
    fn mcp_servers(&self) -> bool {
        false
    }
    fn denies_servers(&self) -> bool {
        false
    }
    fn screen_commands(&self) -> Vec<ScreenCommand> {
        Vec::new()
    }
    fn moves_access(&self) -> bool {
        false
    }
    fn steers_images(&self) -> bool {
        false
    }
    fn waits_for_prompt(&self) -> bool {
        false
    }
    /// `None` at the outer level: no probe. Inner `None`: the binary did not answer.
    fn probe_catalog(&self, exec: Arc<dyn HarnessExec>) -> Option<BoxFuture<'static, Result<Option<HarnessCatalogAnswer>, ExecFailure>>> {
        let _ = exec;
        None
    }
    fn probe_version(&self, exec: Arc<dyn HarnessExec>) -> Option<BoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        let _ = exec;
        None
    }
    fn session_title(&self, harness_session_id: String, exec: Arc<dyn HarnessExec>) -> Option<BoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        let _ = (harness_session_id, exec);
        None
    }
    fn session_file(&self, session_id: &str) -> Option<SessionFile> {
        let _ = session_id;
        None
    }
    fn rename_session(&self, harness_session_id: String, title: String, exec: Arc<dyn HarnessExec>) -> Option<BoxFuture<'static, Result<SessionRenameWrite, ExecFailure>>> {
        let _ = (harness_session_id, title, exec);
        None
    }
    fn title_for(&self, turn: TitleTurn, exec: Arc<dyn HarnessExec>) -> Option<BoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        let _ = (turn, exec);
        None
    }
    fn draft_for(&self, ask: DraftAsk, exec: Arc<dyn HarnessExec>) -> Option<BoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        let _ = (ask, exec);
        None
    }
    fn aside(&self, question: AsideQuestion) -> Option<BoxFuture<'static, Result<AsideAnswer, String>>> {
        let _ = question;
        None
    }
    fn aside_servers(&self) -> bool {
        false
    }
    fn compacts(&self) -> Option<String> {
        None
    }
    fn terminal_resume(&self) -> Option<String> {
        None
    }
    fn env(&self) -> Option<Env> {
        None
    }
    fn conversations(&self) -> Option<Arc<dyn ConversationStore>> {
        None
    }
}

/// The process behind a thread's turns, shared by the turns that read it one after another. TS kept-run.ts:51.
pub trait KeptRun: Send + Sync {
    /// A view of the process for one turn, reading the lines it prints from now on, and where the turn starts in the
    /// run's log on every view after the first. kept-run.ts:53.
    fn turn(&self) -> (Arc<dyn ExecStream>, Option<u64>);
    /// The open turn takes the process with it: its view's close_input reaches the process. kept-run.ts:57.
    fn release(&self);
    /// Up and not released. kept-run.ts:59.
    fn up(&self) -> bool;
    fn exited(&self) -> SharedFuture<Option<i32>>;
    /// Ends the process: input closed, then end_after_result(wait_ms, grace_ms). kept-run.ts:61.
    fn close(&self, wait_ms: u64, grace_ms: u64) -> BoxFuture<'static, ()>;
}

/// One reader for a process's whole life, handing each turn a view of it; `between` takes each line printed while no
/// turn is open. TS kept-run.ts:111 (`keepRun`).
pub fn keep_run(stream: Arc<dyn ExecStream>, between: Option<Arc<dyn Fn(String) + Send + Sync>>) -> Arc<dyn KeptRun> {
    todo!()
}

/// A start that may resume a session the agent's store no longer holds. TS lost-session.ts:11.
pub struct LostSessionStart {
    pub prompt: String,
    pub resume: String,
    pub seed: Arc<dyn Fn() -> BoxFuture<'static, Result<String, String>> + Send + Sync>,
    pub on_event: OnEvent,
}

/// A turn as start_after_loss holds it: anything with a `finished`.
pub trait Finishes: Send + Sync {
    fn finished(&self) -> SharedFuture<TurnResult>;
}

/// What start_after_loss answers: the first launch, the session running now, and the turn's end. TS lost-session.ts:25.
pub struct AfterLoss<S: ?Sized> {
    pub first: Arc<S>,
    pub now: Arc<std::sync::Mutex<Arc<S>>>,
    pub finished: SharedFuture<TurnResult>,
}

/// TS lost-session.ts:21. `launch(prompt, resume, on_event)` starts one launch: the first with `Some(resume)`, the
/// second, only where the first ended with a turn.done whose error is `lost`, with `None` and the prompt
/// `lost_session_prompt(seed, prompt)`. The first's turn.done of `lost` and its session.end after it reach nobody; the
/// second's first session.start is followed by one note delta carrying LOST_SESSION_NOTE.
pub fn start_after_loss<S, L>(options: LostSessionStart, lost: String, launch: L) -> Result<AfterLoss<S>, AdapterError>
where
    S: Finishes + ?Sized + 'static,
    L: Fn(String, Option<String>, OnEvent) -> Result<Arc<S>, AdapterError> + Send + Sync + 'static,
{
    todo!()
}

/// Who opened a conversation. TS conversations.ts:7.
pub use wsp_wire::types::conversations::ConversationOrigin;

/// One row of a conversation's earlier messages. TS conversations.ts:48.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConversationLine {
    /// "person", "agent" or "tool".
    pub who: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<f64>,
}

/// A conversation as an adapter reads it off its agent's store. TS conversations.ts:56.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredConversation {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub cwd: String,
    pub last_at: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<f64>,
    pub origin: ConversationOrigin,
}

/// TS conversations.ts:68.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConversationEarlier {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub lines: Vec<ConversationLine>,
    pub earlier: u32,
}

/// What `earlier` found. TS `ConversationEarlier | "gone" | "open"`.
#[derive(Clone, Debug, PartialEq)]
pub enum EarlierRead {
    Read(ConversationEarlier),
    Gone,
    Open,
}

/// How an adapter reaches the computer the store is on. TS conversations.ts:76.
#[derive(Clone)]
pub struct ConversationRoad {
    pub exec: Arc<dyn HarnessExec>,
    /// One frame to the daemon, `{ op, ... }` in key order, answering its reply object.
    pub ask: Arc<dyn Fn(Map<String, Value>) -> BoxFuture<'static, Result<Map<String, Value>, ExecFailure>> + Send + Sync>,
}

/// The `earlier` options. TS conversations.ts:92.
#[derive(Clone, Debug, PartialEq)]
pub struct EarlierAsk {
    pub cwds: Vec<String>,
    pub last: u32,
    pub probe: bool,
    pub tail: Option<u64>,
    /// `false` is the TS `count: false`.
    pub count: bool,
}

/// A store read's failure; `lasting` is the TS `lastingStoreError` mark (conversations.ts:98).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreError {
    pub message: String,
    pub lasting: bool,
}

/// An agent's own store of conversations. TS conversations.ts:88. Optional TS members answer `None` where absent.
pub trait ConversationStore: Send + Sync {
    fn list(&self, cwds: Vec<String>, road: ConversationRoad) -> BoxFuture<'static, Result<Vec<StoredConversation>, StoreError>>;
    /// Inner `None`: nothing could say (the TS `null`).
    fn live(&self, road: ConversationRoad) -> Option<BoxFuture<'static, Option<IndexSet<String>>>> {
        let _ = road;
        None
    }
    fn stamp(&self, cwds: Vec<String>, road: ConversationRoad) -> Option<BoxFuture<'static, Result<String, StoreError>>> {
        let _ = (cwds, road);
        None
    }
    fn earlier(&self, id: String, ask: EarlierAsk, road: ConversationRoad) -> BoxFuture<'static, Result<EarlierRead, StoreError>>;
    fn lets_go(&self) -> Option<String> {
        None
    }
}
