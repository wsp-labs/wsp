// SPDX-License-Identifier: AGPL-3.0-only
//! Unit runs_parts: slate-runs.ts:17-242, 363-489, 985-987 at 65f6b89c7. Spec: spec/runs_parts.md.
//!
//! A run record is an ordered JSON object, not a struct: every road writes its keys in its own TS order (spec
//! Behaviour 1, Decisions 1), and the slate area stores and pushes it as JSON.
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use wsp_wire::types::js_number::JsNumber;

// slate-runs.ts:17
// export type RunState = "idle" | "held" | "running" | "done" | "failed" | "cancelled";
/// The six states a run reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunState {
    Idle,
    Held,
    Running,
    Done,
    Failed,
    Cancelled,
}

impl RunState {
    pub fn as_str(self) -> &'static str {
        todo!()
    }
    pub fn parse(s: &str) -> Option<RunState> {
        todo!()
    }
}

// slate-runs.ts:19
// /** What `$name` reads while and after the run (02, "Runs"). Written whole at every change; scrubbed and capped. */
// export interface RunRecord {
//   state: RunState;
//   why?: string;
//   exit?: number | null;
//   out?: string;
//   err?: string;
//   json?: SlateJson;
//   lines?: string[];
//   startedAt?: number;
//   endedAt?: number;
//   ms?: number;
//   runs: number;
//   cut?: true;
//   /** Running again with the last result kept beside it until the new one replaces it. */
//   refreshing?: true;
//   /** A tool answered with text alone, no structured result. */
//   text?: true;
// }
/// What `$name` reads while and after the run, keys in the order the road that wrote it put them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunRecord(pub Map<String, Value>);

impl RunRecord {
    /// A record from a stored value: an object whose `state` is a string and `runs` a number (slate-read.ts:6).
    pub fn from_json(v: &Value) -> Option<RunRecord> {
        todo!()
    }
    pub fn to_json(&self) -> Value {
        todo!()
    }
    /// `state`, when it names one of the six.
    pub fn state(&self) -> Option<RunState> {
        todo!()
    }
    /// `runs`, 0 when absent.
    pub fn runs(&self) -> i64 {
        todo!()
    }
    pub fn why(&self) -> Option<&str> {
        todo!()
    }
    pub fn started_at(&self) -> Option<i64> {
        todo!()
    }
    pub fn get(&self, key: &str) -> Option<&Value> {
        todo!()
    }
}

// slate-runs.ts:39
// /** What a run that ran left for `$name` to read, kept on its record while it runs again. */
// export type RunResult = Pick<RunRecord, "exit" | "out" | "err" | "json" | "lines" | "endedAt" | "ms" | "cut">;
/// What a run that ran left for `$name`, the fields always in `RESULT_FIELDS` order.
pub type RunResult = Map<String, Value>;

// slate-runs.ts:42
// /** What `$token` reads. The plaintext never leaves the store but as a run's environment or stdin. */
// export interface SecretHandle {
//   secret: true;
//   set: boolean;
//   len: number;
//   at: number | null;
// }
/// What `$token` reads: `{secret: true, set, len, at}`, `at` null when unset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecretHandle {
    pub secret: bool,
    pub set: bool,
    pub len: i64,
    pub at: Option<i64>,
}

// slate-runs.ts:50
// /** A cmd run as the agent declared it. `env`, `args` and `stdin` are the declaration's own expressions, read only
//  * for the approval key: which expression fills a name is part of what the person approves. */
// export interface CmdRunDecl {
//   kind: "cmd";
//   cmd: string;
//   env?: Record<string, unknown>;
//   args?: unknown[];
//   stdin?: unknown;
//   on?: "thread" | "host";
//   cwd?: string;
//   timeout?: number;
//   stream?: boolean;
//   confirm?: string;
//   every?: number;
//   always?: boolean;
//   once?: boolean;
//   /** A literal command that reads the raw result on stdin and prints the JSON the record's json becomes. */
//   then?: string;
//   /** The text of each of the slate's files the command or its then reads, by name: part of what the person approves. */
//   files?: Record<string, string>;
//   /** A hash of each file in the thread's folder the command or its then names, by its path there: an edit asks again. */
//   scripts?: Record<string, string>;
// }
/// A cmd run as the slate area hands it over: the declaration with `confirm` as text, the files it reads and the
/// hashes of the scripts it names. `kind` is always `cmd`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CmdRunDecl {
    pub cmd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<IndexMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<Value>>,
    /// A literal null stdin is kept apart from none (spec Behaviour 7).
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::pending::null_kept::deserialize")]
    pub stdin: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub once: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub then: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scripts: Option<IndexMap<String, String>>,
}

// slate-runs.ts:74
// /** One value as it reaches a command, evaluated by the host at the moment the run starts. A secret is named, never
//  * resolved by the caller: the plaintext is put in place here and nowhere else. */
// export type RunInput = { value: SlateJson } | { secret: string };
//
// export interface RunInputs {
//   env?: Record<string, RunInput>;
//   args?: RunInput[];
//   stdin?: RunInput;
// }
/// One value as it reaches a command; a secret is named, never resolved by the caller.
#[derive(Clone, Debug, PartialEq)]
pub enum RunInput {
    Value(Value),
    Secret(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunInputs {
    pub env: Option<IndexMap<String, RunInput>>,
    pub args: Option<Vec<RunInput>>,
    pub stdin: Option<RunInput>,
}

// slate-runs.ts:84
// export type RunBy = "person" | "agent" | "reaction" | "timer";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunBy {
    Person,
    Agent,
    Reaction,
    Timer,
}

impl RunBy {
    pub fn as_str(self) -> &'static str {
        todo!()
    }
}

// slate-runs.ts:86
// export interface RunStart {
//   threadId: string;
//   run: string;
//   decl: CmdRunDecl;
//   by: RunBy;
//   /** The thread's folder; `cwd` is read against it. */
//   folder: string;
//   /** Read when the command actually starts, which for a held run is when the person approves it. */
//   inputs: () => RunInputs;
//   /** The host's record of the run, for a run this process has not seen yet: how many times it started and the last
//    * result a start keeps. */
//   last?: RunRecord;
//   /** The thread's machine is napping: a timer never wakes it, so a tick is held and says so; a press goes on. */
//   asleep?: boolean;
// }
/// A start. `inputs` is read when the command actually starts, which for a held run is when the person approves it.
#[derive(Clone)]
pub struct RunStart {
    pub thread_id: String,
    pub run: String,
    pub decl: CmdRunDecl,
    pub by: RunBy,
    pub folder: String,
    pub inputs: Rc<dyn Fn() -> RunInputs>,
    pub last: Option<RunRecord>,
    pub asleep: bool,
}

// slate-runs.ts:102
// /** What the approval sheet shows. A secret's value is dots. */
// export interface RunAsk {
//   run: string;
//   key: string;
//   cmd: string;
//   env: Record<string, string>;
//   args: string[];
//   stdin?: string;
//   folder: string;
//   on: "thread" | "host";
//   timeout: number;
//   confirm?: string;
//   then?: string;
//   files?: Record<string, string>;
// }
/// What the approval sheet shows, keys in the order slate-runs.ts:676-689 writes them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunAsk {
    pub run: String,
    pub key: String,
    pub cmd: String,
    pub env: IndexMap<String, String>,
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdin: Option<String>,
    pub folder: String,
    pub on: String,
    pub timeout: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub then: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<IndexMap<String, String>>,
}

// slate-runs.ts:118
// /** A reshape's answer: the JSON it printed, or why it failed with what it said on stderr. */
// export type ReshapeAnswer = { json: SlateJson } | { why: string; exit: number | null; err: string };
//
// export interface Reshape {
//   done: Promise<ReshapeAnswer>;
//   kill(): void;
// }
/// A reshape's answer: the JSON it printed, or why it failed with what it said on stderr.
#[derive(Clone, Debug, PartialEq)]
pub enum ReshapeAnswer {
    Json(Value),
    Failed { why: String, exit: Option<i64>, err: String },
}

/// A reshape under way: `done` settles once; `kill` ends its group.
pub struct Reshape {
    pub done: Pin<Box<dyn Future<Output = ReshapeAnswer>>>,
    pub kill: Rc<dyn Fn()>,
}

// slate-runs.ts:126
// export type RunStartAnswer =
//   | { outcome: "running"; record: RunRecord }
//   | { outcome: "held"; record: RunRecord; ask?: RunAsk }
//   | { outcome: "noop"; record: RunRecord }
//   | { outcome: "failed"; record: RunRecord };
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Running,
    Held,
    Noop,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunStartAnswer {
    pub outcome: Outcome,
    pub record: RunRecord,
    /// Only for a start held for approval, not beside a hold of another kind.
    pub ask: Option<RunAsk>,
}

// slate-runs.ts:363
// /** Why a command did not start: it starts in its thread's folder, never the host's own, and none is known. */
// export const NO_FOLDER = "this host knows no folder for the thread, so the command did not start";
//
// const RESULT_FIELDS = ["exit", "out", "err", "json", "lines", "endedAt", "ms", "cut"] as const;
pub const NO_FOLDER: &str = "this host knows no folder for the thread, so the command did not start";
/// The fields a result keeps, in this order.
pub const RESULT_FIELDS: [&str; 8] = ["exit", "out", "err", "json", "lines", "endedAt", "ms", "cut"];

// slate-runs.ts:394
// const DEFAULT_TIMEOUT_S = 60;
// const LINES_KEPT = SLATE_LIMITS.streamLines;
// const RUNNING_MAX = SLATE_LIMITS.runningAtOnce;
// export const STARTS_PER_MINUTE = SLATE_LIMITS.startsPerMinute;
// const TIMER_FLOOR_S = 10;
// /** Five failed starts in a row back an `always` timer off to one start every five minutes, until one succeeds (07). */
// const BACKOFF_AFTER = 5;
// const BACKOFF_MS = 5 * 60_000;
// export const BACKED_OFF = "five starts in a row failed, so its timer starts it once every 5 minutes until one succeeds";
// /** After the leader exits and its group is killed, how long the pipes get to close before the run ends anyway: a
//  * process that left the group with setsid can hold them open for as long as it lives. */
// const CLOSE_GRACE_MS = 1_000;
// const ENV_NAME = /^[A-Za-z_][A-Za-z0-9_]*$/;
// /** What a set secret reads as wherever a command or a tool call is shown. */
// export const SECRET_DOTS = "••••";
//
// export const HELD_APPROVAL = SLATE_HELD_APPROVAL;
// const HELD_BUSY = `${RUNNING_MAX} runs are already running`;
// export const HELD_BUDGET = `started ${STARTS_PER_MINUTE} times in a minute; press to run it again`;
// export const HELD_PRESSED = `pressed ${STARTS_PER_MINUTE} times in a minute; it can start again in a minute`;
// const HELD_ASLEEP = "the box was asleep, so this tick did not wake it; press to run it now";
// type HeldFor = "approval" | "busy" | "budget" | "pressed";
pub const DEFAULT_TIMEOUT_S: i64 = 60;
pub const LINES_KEPT: usize = 500;
pub const RUNNING_MAX: usize = 4;
pub const STARTS_PER_MINUTE: usize = 12;
pub const TIMER_FLOOR_S: i64 = 10;
pub const BACKOFF_AFTER: i64 = 5;
pub const BACKOFF_MS: i64 = 5 * 60_000;
pub const BACKED_OFF: &str = "five starts in a row failed, so its timer starts it once every 5 minutes until one succeeds";
pub const CLOSE_GRACE_MS: i64 = 1_000;
/// `^[A-Za-z_][A-Za-z0-9_]*$`, ported by hand (rule 8.7).
pub fn is_env_name(name: &str) -> bool {
    todo!()
}
pub const SECRET_DOTS: &str = "••••";
pub const HELD_APPROVAL: &str = crate::pending::wire::SLATE_HELD_APPROVAL;
pub const HELD_BUSY: &str = "4 runs are already running";
pub const HELD_BUDGET: &str = "started 12 times in a minute; press to run it again";
pub const HELD_PRESSED: &str = "pressed 12 times in a minute; it can start again in a minute";
pub const HELD_ASLEEP: &str = "the box was asleep, so this tick did not wake it; press to run it now";

// slate-runs.ts:458
// const filesUnwritten = (e: unknown): string => `the slate's files were not written: ${e instanceof Error ? e.message : String(e)}`;
/// `the slate's files were not written: <message>`.
pub fn files_unwritten(message: &str) -> String {
    todo!()
}

// slate-runs.ts:368
// /** The result a record holds, where it is one a command left by running. */
// export function lastResult(record: RunRecord): RunResult | undefined {
//   const ran = record.state === "running" ? record.refreshing === true : record.startedAt !== undefined && (record.state === "done" || record.state === "failed" || record.state === "cancelled");
//   return ran ? (Object.fromEntries(RESULT_FIELDS.filter(f => record[f] !== undefined).map(f => [f, record[f]])) as RunResult) : undefined;
// }
pub fn last_result(record: &RunRecord) -> Option<RunResult> {
    todo!()
}

// slate-runs.ts:374
// /** The record of a start: running, with the last result kept until this one ends. */
// export function runningRecord(last: RunResult | undefined, runs: number, startedAt: number): RunRecord {
//   return last === undefined ? { state: "running", runs, startedAt } : { ...last, state: "running", runs, startedAt, refreshing: true };
// }
pub fn running_record(last: Option<&RunResult>, runs: i64, started_at: i64) -> RunRecord {
    todo!()
}

// slate-runs.ts:379
// /** A running record ended from outside, its kept result gone with the start that kept it. */
// function endedRecord(record: RunRecord, state: "failed" | "cancelled", why: string, now: number): RunRecord {
//   return { state, why, exit: null, runs: record.runs, ...(record.startedAt !== undefined ? { startedAt: record.startedAt } : {}), endedAt: now };
// }
/// `state` is `failed` or `cancelled`.
pub fn ended_record(record: &RunRecord, state: RunState, why: &str, now: i64) -> RunRecord {
    todo!()
}

// slate-runs.ts:384
// /** A record that read running when the host stopped: nothing is running now, and nothing restarts by itself. */
// export function restartedRecord(record: RunRecord, now: number = Date.now()): RunRecord {
//   return record.state === "running" ? endedRecord(record, "failed", "the host restarted while it ran", now) : record;
// }
pub fn restarted_record(record: &RunRecord, now: i64) -> RunRecord {
    todo!()
}

// slate-runs.ts:389
// /** A record a rewind restores that read running at that turn's end. */
// export function rewoundRecord(record: RunRecord, now: number = Date.now()): RunRecord {
//   return record.state === "running" ? endedRecord(record, "cancelled", "the thread was rewound", now) : record;
// }
pub fn rewound_record(record: &RunRecord, now: i64) -> RunRecord {
    todo!()
}

// slate-runs.ts:460
// /** One more start against a budget of STARTS_PER_MINUTE a minute: the start times kept with this one, or undefined
//  * when this one is past the budget. Every run kind, and the person's presses and the agent's starts each, spend one. */
// export function spent(times: readonly number[] | undefined, at: number): number[] | undefined {
//   const kept = (times ?? []).filter(s => at - s < 60_000);
//   return kept.length >= STARTS_PER_MINUTE ? undefined : [...kept, at];
// }
pub fn spent(times: Option<&[i64]>, at: i64) -> Option<Vec<i64>> {
    todo!()
}

// slate-runs.ts:467
// export function stableJson(value: unknown): string {
//   if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
//   if (value !== null && typeof value === "object") {
//     const entries = Object.entries(value as Record<string, unknown>).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
//     return `{${entries.map(([k, v]) => `${JSON.stringify(k)}:${stableJson(v)}`).join(",")}}`;
//   }
//   return JSON.stringify(value) ?? "null";
// }
/// `value` none is `undefined`, written `null`.
pub fn stable_json(value: Option<&Value>) -> String {
    todo!()
}

// slate-runs.ts:662
//   const key = (decl: CmdRunDecl): string => {
//     const env = Object.entries(decl.env ?? {})
//       .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
//       .map(([name, expr]) => [name, stableJson(expr)]);
//     const what = [decl.kind, decl.cmd, env, (decl.args ?? []).map(stableJson), decl.stdin === undefined ? null : stableJson(decl.stdin), decl.on ?? "thread", decl.cwd ?? "", decl.stream === true, decl.every ?? null, decl.always === true, decl.timeout ?? DEFAULT_TIMEOUT_S, decl.once === true, ...(decl.then !== undefined ? [decl.then] : []), ...(decl.files !== undefined ? [stableJson(decl.files)] : []), ...(decl.scripts !== undefined ? [stableJson(decl.scripts)] : [])];
//     return createHash("sha256").update(JSON.stringify(what)).digest("hex").slice(0, 32);
//   };
/// The approval key (the table's `key`).
pub fn run_key(decl: &CmdRunDecl) -> String {
    todo!()
}

// slate-runs.ts:476
// /** Every string inside a value passed through fn. */
// export function mapStrings(value: SlateJson, fn: (s: string) => string): SlateJson {
//   if (typeof value === "string") return fn(value);
//   if (Array.isArray(value)) return value.map(v => mapStrings(v, fn));
//   if (value !== null && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, mapStrings(v, fn)]));
//   return value;
// }
pub fn map_strings(value: &Value, f: &dyn Fn(&str) -> String) -> Value {
    todo!()
}

// slate-runs.ts:484
// function asText(value: SlateJson): string {
//   if (value === null) return "";
//   if (typeof value === "string") return value;
//   if (typeof value === "number" || typeof value === "boolean") return String(value);
//   return JSON.stringify(value);
// }
pub fn as_text(value: &Value) -> String {
    todo!()
}

// slate-runs.ts:985
// function timeoutOf(decl: CmdRunDecl): number {
//   return Math.min(Math.max(decl.timeout ?? DEFAULT_TIMEOUT_S, 1), EXEC_TIMEOUT_MAX_MS / 1_000);
// }
/// Seconds, fractions kept.
pub fn timeout_of(decl: &CmdRunDecl) -> f64 {
    todo!()
}
