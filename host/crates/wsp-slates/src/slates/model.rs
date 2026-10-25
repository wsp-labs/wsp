// SPDX-License-Identifier: AGPL-3.0-only
//! Unit slates_model: slates.ts:81-292, 686-758 at 65f6b89c7. Spec: spec/slates_model.md.
use std::collections::HashSet;
use std::rc::Rc;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wsp_words::exit::HostError;

use super::SlatesInner;
use crate::pending::slate::{SlateDoc, SlateProblem, SlateRunDecl, SlateValues};
use crate::pending::wire::SlateEmpty;

// slates.ts:81
// /** The slate's content at one version: what a snapshot, the undo and a rewind keep. */
// interface SlateSnap {
//   document: SlateDoc | null;
//   values: SlateValues;
//   empty?: SlateEmpty;
// }
/// The slate's content at one version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlateSnap {
    pub document: Option<SlateDoc>,
    pub values: SlateValues,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty: Option<SlateEmpty>,
}

// slates.ts:88
// /** One approval the person gave, by the run declaration's key; a refusal is kept the same way. */
// interface Approval {
//   state: "allowed" | "refused";
//   at: number;
//   run?: string;
//   cmd?: string;
//   /** The named files' hashes the person allowed, so a run that asks again can say which one changed. */
//   scripts?: Record<string, string>;
// }
/// One approval the person gave, by key; a refusal is kept the same way.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Approval {
    /// `allowed` or `refused`.
    pub state: String,
    pub at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scripts: Option<IndexMap<String, String>>,
}

/// Where a turn ended: `{at, version, revision}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TurnMark {
    pub at: i64,
    pub version: i64,
    pub revision: i64,
}

/// `previous`: the snapshot and the version it was.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreviousSnap {
    pub document: Option<SlateDoc>,
    pub values: SlateValues,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty: Option<SlateEmpty>,
    pub version: i64,
}

// slates.ts:98
// /** One thread's slate as the store keeps it. Snapshots are kept by version and turns point at them, so a turn in
//  * which nothing about the slate changed adds a pointer and no copy. Secrets are handles here, never plaintext. */
// export interface SlateRecord extends SlateSnap {
//   threadId: string;
//   workspaceId: string;
//   rootThreadId: string;
//   schema: 2;
//   /** Moves only when the document does. */
//   version: number;
//   /** Rises with every batch that moved a value and with every version; orders the pushes, checked by nothing. */
//   revision: number;
//   previous?: SlateSnap & { version: number };
//   /** By turn id, in the order the turns ended; kept is by revision, since values move without the version. */
//   turns: Record<string, { at: number; version: number; revision: number }>;
//   kept: Record<string, SlateSnap>;
//   comments: Record<string, SlateJson>[];
//   /** By approval key; "Always in this thread", "Allow" and "Not now" stay, "Run once" is never kept. */
//   approvals: Record<string, Approval>;
//   /** When the person let this slate message the agent from a reaction (07, "A slate that messages the agent"). */
//   sendsAllowed?: number;
//   /** Runtime problems the batch and the runs left standing, newest last. */
//   problems: SlateProblem[];
//   shownOnce: boolean;
//   updatedAt: number;
// }
/// One thread's slate as the store keeps it, keys in `fresh_record`'s order (spec Behaviour 1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlateRecord {
    pub thread_id: String,
    pub workspace_id: String,
    pub root_thread_id: String,
    pub schema: i64,
    pub version: i64,
    /// Absent only in a stored record of before revisions; read back as `revision ?? version` (slates.ts:433).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<i64>,
    pub document: Option<SlateDoc>,
    pub values: SlateValues,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty: Option<SlateEmpty>,
    pub turns: IndexMap<String, TurnMark>,
    pub kept: IndexMap<String, SlateSnap>,
    pub comments: Vec<Value>,
    pub approvals: IndexMap<String, Approval>,
    pub problems: Vec<SlateProblem>,
    pub shown_once: bool,
    pub updated_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<PreviousSnap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sends_allowed: Option<i64>,
}

// slates.ts:199
// type BatchBy = "person" | "agent" | "reaction" | "run" | "timer" | "host";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchBy {
    Person,
    Agent,
    Reaction,
    Run,
    Timer,
    Host,
}

// slates.ts:210
// const SNAPSHOTS_KEPT = 100;
// const SNAPSHOT_BYTES = 8 * 1024 * 1024;
// const MESSAGE_CHARS = 20_000;
// const REQUEST_KEPT_MS = 10 * 60_000;
// /** How many presses a request id is remembered for at most, the oldest dropped first. */
// const REQUESTS_KEPT = 500;
// /** The person's events a slate takes: five a second, a burst of five, before any is remembered or batched. */
// const EVENTS_PER_SECOND = 5;
// const PRESS_SEND_MS = 2_000;
// const REACTION_SEND_MS = 60_000 / SLATE_LIMITS.reactionSendsPerMinute;
// const PROBLEMS_KEPT = 20;
// const PIECES_NAMED = SLATE_LIMITS.piecesNamed;
pub const SNAPSHOTS_KEPT: usize = 100;
pub const SNAPSHOT_BYTES: usize = 8 * 1024 * 1024;
pub const MESSAGE_CHARS: usize = 20_000;
pub const REQUEST_KEPT_MS: i64 = 10 * 60_000;
pub const REQUESTS_KEPT: usize = 500;
pub const EVENTS_PER_SECOND: f64 = 5.0;
pub const PRESS_SEND_MS: i64 = 2_000;
pub const REACTION_SEND_MS: i64 = 60_000;
pub const PROBLEMS_KEPT: usize = 20;
pub const PIECES_NAMED: usize = 20;
pub use crate::pending::wire::SLATE_SEND_KEY;

// slates.ts:226
// const problem = (code: string, name: string, message: string, extra: Partial<SlateProblem> = {}): SlateProblem => ({ code, name, message, ...extra });
/// `{code, name, message, ...extra}`; the extra here is the fix.
pub fn problem(code: &str, name: &str, message: &str, fix: Option<&str>) -> SlateProblem {
    todo!()
}

// slates.ts:228
// /** A refusal for a reason other than what was written: the version moved, too fast, nothing to undo. */
// const refused = (p: SlateProblem, kind: string): Error => Object.assign(new Error(`${p.code} ${p.name}: ${p.message}`), { kind, code: p.code });
//
// const usage = (p: SlateProblem): Error => Object.assign(new Error(`${p.code} ${p.name}: ${p.message}`), { kind: "usage", code: p.code });
/// `<code> <name>: <message>` with this kind and the code.
pub fn refused(p: &SlateProblem, kind: wsp_words::exit::Kind) -> HostError {
    todo!()
}
pub fn usage(p: &SlateProblem) -> HostError {
    todo!()
}

// slates.ts:233
// /** The source names a document or a list of paths reads, read off its text. */
// const sourcesNamed = (text: string): string[] => SOURCE_NAMES.filter(name => new RegExp(`\\b${name}\\.`).test(text));
//
// /** A value path as written, `$` optional: `i` is `$i`. */
// const ownPath = (path: string): string => (path.startsWith("$") ? path : `$${path}`);
pub fn sources_named(text: &str) -> Vec<String> {
    todo!()
}
pub fn own_path(path: &str) -> String {
    todo!()
}

// slates.ts:239
// /** What a run executes, without how often or how long: a change here makes its last result stale. */
// const commandOf = (decl: SlateRunDecl): SlateJson => {
//   const { every: _every, always: _always, timeout: _timeout, stream: _stream, ...what } = decl as SlateRunDecl & { timeout?: number; stream?: true };
//   return asJson(what);
// };
/// The declaration as JSON less `every`, `always`, `timeout`, `stream`.
pub fn command_of(decl: &SlateRunDecl) -> Value {
    todo!()
}

// slates.ts:245
// /** What each outcome reads as under the pressed piece (09, "Delivery"). */
// const SAID: Record<"send" | "steer" | "queue", Record<"started" | "steered" | "queued", string>> = {
//   send: { started: "Sent", steered: "Sent into the running turn", queued: "Waiting for the turn to end" },
//   steer: { started: "Nothing was running, so it was sent as the next message", steered: "Sent into the running turn", queued: "Waiting for the turn to end" },
//   // A queue that waits even where the agent takes messages mid-turn needs a flag on the start this build lacks, so
//   // it goes as a send, and the person is told when that meant joining the running turn.
//   queue: { started: "Sent", steered: "Sent into the running turn; this agent takes messages mid-turn", queued: "Waiting for the turn to end" },
// };
/// `kind` is `send`, `steer` or `queue`; `outcome` `started`, `steered` or `queued`.
pub fn said(kind: &str, outcome: super::DeliveryOutcome) -> &'static str {
    todo!()
}

// slates.ts:254
// const sizeOf = (value: unknown): number => slateBytes(JSON.stringify(value));
/// UTF-8 bytes of the JSON text.
pub fn size_of(value: &Value) -> usize {
    todo!()
}

// slates.ts:747
//   /** Drops the oldest snapshots past the count and the byte cap, then every kept version no turn points at. */
//   const trimSnapshots = (r: SlateRecord): void => {
//     const order = Object.keys(r.turns);
//     while (order.length > SNAPSHOTS_KEPT) delete r.turns[order.shift()!];
//     const used = (): Set<string> => new Set(Object.values(r.turns).map(t => String(t.revision)));
//     for (const v of Object.keys(r.kept)) if (!used().has(v)) delete r.kept[v];
//     while (order.length > 1 && sizeOf(r.kept) > SNAPSHOT_BYTES) {
//       delete r.turns[order.shift()!];
//       const live = used();
//       for (const v of Object.keys(r.kept)) if (!live.has(v)) delete r.kept[v];
//     }
//   };
pub fn trim_snapshots(r: &mut SlateRecord) {
    todo!()
}

// slates.ts:686
//   const snapOf = (r: SlateRecord): SlateSnap => ({ document: r.document, values: r.values, ...(r.empty !== undefined ? { empty: r.empty } : {}) });
pub fn snap_of(r: &SlateRecord) -> SlateSnap {
    todo!()
}

impl SlatesInner {
    // slates.ts:688
    //   /** Every secret's handle read off the store: a rewind or a restart never fills or empties a secret (08, "Reuse"). */
    //   const refreshSecrets = (r: SlateRecord): void => {
    //     for (const [name, decl] of Object.entries(r.document?.values ?? {})) if (decl.secret === true) r.values[name] = asJson(runs.secrets.handle(r.threadId, name));
    //   };
    pub(crate) fn refresh_secrets(&self, r: &mut SlateRecord) {
        todo!()
    }

    // slates.ts:693
    //   const armTimers = (r: SlateRecord, stopped: ReadonlySet<string> = new Set()): void => {
    //     const list = Object.entries(r.document?.runs ?? {}).flatMap(([run, decl]) => (decl.every !== undefined && !stopped.has(run) ? [{ run, every: decl.every, key: decl.kind === "cmd" ? runs.key(approvalDecl(r, decl) as CmdRunDecl) : JSON.stringify(decl), ...(decl.always === true ? { always: true } : {}) }] : []));
    //     runs.timers(r.threadId, list);
    //     const servers = Object.values(r.document?.runs ?? {}).flatMap(decl => (decl.kind === "cmd" ? [] : [{ server: decl.server, kept: decl.every !== undefined && decl.always === true }]));
    //     mcp.servers(r.threadId, servers.map(s => s.server), servers.filter(s => s.kept).map(s => s.server));
    //   };
    pub(crate) fn arm_timers(self: &Rc<Self>, r: &SlateRecord, stopped: &HashSet<String>) {
        todo!()
    }

    // slates.ts:700
    //   /** A snapshot put back: every run stopped with its completion dropped, records that said running say cancelled,
    //    * secrets read as the store holds them, and nothing fires (02, "Rewind"). */
    //   const become = (r: SlateRecord, snap: SlateSnap): void => {
    //     runs.stopAll(r.threadId, { quiet: true });
    //     mcp.stopAll(r.threadId, { quiet: true });
    //     r.document = snap.document;
    //     r.values = structuredClone(snap.values);
    //     for (const name of Object.keys(r.document?.runs ?? {})) {
    //       const rec = r.values[name];
    //       if (isRunRecord(rec)) r.values[name] = asJson(rewoundRecord(rec as unknown as RunRecord, deps.now()));
    //     }
    //     refreshSecrets(r);
    //     if (snap.document === null) r.empty = snap.empty ?? "none";
    //     else delete r.empty;
    //     armTimers(r);
    //   };
    /// The TS `become`.
    pub(crate) fn become_snap(self: &Rc<Self>, r: &mut SlateRecord, snap: SlateSnap) {
        todo!()
    }

    // slates.ts:717
    //   /** A new document's live values (02, "Values"): names both documents declare keep their live value, names only
    //    * the new one declares start, names only the old one declared go; a run keeps its record, a secret its handle. */
    //   const valuesFor = (r: Pick<SlateRecord, "threadId" | "document" | "values">, next: SlateDoc): SlateValues => {
    //     const before = r.document;
    //     const live: SlateValues = {};
    //     for (const [name, decl] of Object.entries(next.values)) {
    //       if (decl.secret === true) live[name] = asJson(runs.secrets.handle(r.threadId, name));
    //       else if (before?.values[name] !== undefined && name in r.values) live[name] = r.values[name]!;
    //       else live[name] = structuredClone(decl.start);
    //     }
    //     for (const [name, decl] of Object.entries(next.runs)) {
    //       const rec = r.values[name];
    //       const was = before?.runs[name];
    //       if (was === undefined || !isRunRecord(rec)) live[name] = asJson(structuredClone(SLATE_RUN_IDLE));
    //       // The last result of a command that is no longer the one declared stays readable, marked, until it runs again.
    //       else live[name] = ["done", "failed", "cancelled"].includes(rec.state) && !slateEqual(commandOf(was), commandOf(decl)) ? asJson({ ...(rec as object), stale: true }) : rec;
    //     }
    //     return live;
    //   };
    pub(crate) fn values_for(&self, thread_id: &str, before: Option<&SlateDoc>, values: &SlateValues, next: &SlateDoc) -> SlateValues {
        todo!()
    }

    // slates.ts:737
    //   /** What a dropped declaration leaves behind: its secret forgotten at once, its run stopped. */
    //   const dropDeclared = (r: SlateRecord, next: SlateDoc | null): void => {
    //     for (const [name, decl] of Object.entries(r.document?.values ?? {})) if (decl.secret === true && next?.values[name]?.secret !== true) runs.secrets.forget(r.threadId, name);
    //     for (const name of Object.keys(r.document?.runs ?? {})) {
    //       if (next?.runs[name] !== undefined) continue;
    //       runs.cancel(r.threadId, name);
    //       mcp.cancel(r.threadId, name);
    //     }
    //   };
    pub(crate) fn drop_declared(&self, r: &SlateRecord, next: Option<&SlateDoc>) {
        todo!()
    }
}
