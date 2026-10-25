// SPDX-License-Identifier: AGPL-3.0-only
//! Unit slates_batch: slates.ts:405-424, 760-1015 at 65f6b89c7. Spec: spec/slates_batch.md.
use std::rc::Rc;

use indexmap::IndexMap;
use serde_json::Value;
use wsp_words::exit::HostError;

use super::model::{BatchBy, SlateRecord};
use super::{Delivered, RecordRef, SlatesInner, Views};
use crate::pending::siblings::Pin;
use crate::pending::slate::{BatchEvent, BatchSend, SlateRunDecl, SlateWrite};
use crate::slate_runs::{CmdRunDecl, RunAsk, RunBy, RunInputs, RunRecord};

/// What a batch did (slates.ts:888-893).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BatchOut {
    pub asks: Vec<RunAsk>,
    /// The runs this batch started, held or not.
    pub started: Vec<String>,
    pub sends: Vec<ComposedSend>,
}

/// A send composed for the thread.
#[derive(Clone, Debug, PartialEq)]
pub struct ComposedSend {
    pub prompt: String,
    /// `send`, `steer` or `queue`.
    pub kind: String,
    pub request_id: String,
}

/// The batch's options (slates.ts:897).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BatchOptions {
    pub event: Option<BatchEvent>,
    pub starts: Vec<(String, RunBy)>,
    pub request_id: Option<String>,
    pub send_at: Option<i64>,
    /// `Some(false)`: no reaction fires.
    pub react: Option<bool>,
}

/// A start's answer inside the area: the record and, when held for approval, the ask.
#[derive(Clone, Debug, PartialEq)]
pub struct Started {
    pub record: RunRecord,
    pub ask: Option<RunAsk>,
}

impl SlatesInner {
    // slates.ts:405
    //   /** A declaration as its approval takes it: the files it reads from the slate, and a hash of each file it names in
    //    * the thread's folder, read again at every start, where the command runs. */
    //   function approvalDecl(r: SlateRecord, declared: SlateRunDecl): SlateRunDecl & { files?: Record<string, string>; scripts?: Record<string, string> } {
    //     const decl = withFiles(r.document, declared);
    //     const scripts = keyed(pinFor(r.threadId, declared));
    //     return scripts === undefined ? decl : { ...decl, scripts };
    //   }
    /// The declaration as its approval takes it; for a `cmd` run, the runs table's `CmdRunDecl` (files and scripts in).
    pub(crate) fn approval_decl(&self, r: &SlateRecord, declared: &SlateRunDecl) -> CmdRunDecl {
        todo!()
    }

    // slates.ts:413
    //   /** The run name and command an approval key stands for, so the menu can list it after the document changes, and
    //    * the named files' hashes it covers, so a changed one can be named when it asks again. */
    //   function approvalNames(r: SlateRecord, key: string): { run?: string; cmd?: string; scripts?: Record<string, string> } {
    //     const asked = mcp.named(r.threadId, key);
    //     if (asked !== undefined) return asked;
    //     for (const [name, declared] of Object.entries(r.document?.runs ?? {})) {
    //       const decl = approvalDecl(r, declared);
    //       if (decl.kind === "cmd" && runs.key(decl as CmdRunDecl) === key) return { run: name, cmd: decl.cmd, ...(decl.scripts !== undefined ? { scripts: decl.scripts } : {}) };
    //       if (decl.kind !== "cmd" && key === codeKey(decl as McpRunDecl, mcpArgsOf(r, decl as McpRunDecl)())) return { run: name, cmd: `${decl.server}.${decl.kind === "tool" ? decl.tool : decl.uri}` };
    //     }
    //     return {};
    //   }
    /// `(run, cmd, scripts)`, each when known.
    pub(crate) fn approval_names(&self, r: &SlateRecord, key: &str) -> (Option<String>, Option<String>, Option<IndexMap<String, String>>) {
        todo!()
    }

    // slates.ts:762
    //   /** A declared run's env, args and stdin, read when the command starts; a secret named by a bare binding goes over
    //    * by name and becomes plaintext only inside the runs module (07, "Injection"). */
    //   const inputsOf = (r: SlateRecord, decl: Extract<SlateRunDecl, { kind: "cmd" }>) => () => {
    //     const ctx = contextOf(r, viewsNow.get(r.threadId) ?? new Map());
    //     const one = (v: SlatePropValue): RunInput => {
    //       if (isSlateBinding(v)) {
    //         const own = parseSlateOwnPath(v.bind.trim());
    //         if (own !== undefined && own.segs.length === 0 && r.document?.values[own.name]?.secret === true) return { secret: own.name };
    //       }
    //       return { value: resolveSlateProp(v, ctx) ?? null };
    //     };
    //     return {
    //       ...(decl.env !== undefined ? { env: Object.fromEntries(Object.entries(decl.env).map(([k, v]) => [k, one(v)])) } : {}),
    //       ...(decl.args !== undefined ? { args: decl.args.map(one) } : {}),
    //       ...(decl.stdin !== undefined ? { stdin: one(decl.stdin) } : {}),
    //     };
    //   };
    pub(crate) fn inputs_of(self: &Rc<Self>, r: RecordRef, run: &str) -> Rc<dyn Fn() -> RunInputs> {
        todo!()
    }

    // slates.ts:779
    //   /** A tool run's arguments, read when the call goes: a secret stands as its mark wherever an expression reads it, and
    //    * becomes plaintext only inside slate-mcp at the call. */
    //   const mcpArgsOf = (r: SlateRecord, decl: McpRunDecl) => (): Record<string, SlateJson> => {
    //     if (decl.kind !== "tool" || decl.args === undefined) return {};
    //     const ctx = contextOf(r, viewsNow.get(r.threadId) ?? new Map());
    //     const plain = ctx.resolve;
    //     const marked: SlateEvalContext = {
    //       ...ctx,
    //       resolve: path => {
    //         const own = parseSlateOwnPath(path);
    //         return own !== undefined && own.segs.length === 0 && r.document?.values[own.name]?.secret === true ? slateSecretMark(own.name) : plain(path);
    //       },
    //     };
    //     return Object.fromEntries(Object.entries(decl.args).map(([k, v]) => [k, resolveSlateProp(v, marked) ?? null]));
    //   };
    //   /** The sources a run's inputs read, kept from the last batch of its slate. */
    //   const viewsNow = new Map<string, ReadonlyMap<string, SlateJson | undefined>>();
    pub(crate) fn mcp_args_of(self: &Rc<Self>, r: RecordRef, run: &str) -> Rc<dyn Fn() -> IndexMap<String, Value>> {
        todo!()
    }

    // slates.ts:797
    //   /** Where a command starts: the thread's own folder on its own computer, or this computer's for one `on` the host. */
    //   /** Whether a run's command runs on the thread's own machine rather than on this computer. */
    //   const onMachine = (threadId: string, decl: SlateRunDecl): boolean => decl.kind === "cmd" && decl.on !== "host" && deps.machineOf?.(threadId) !== undefined;
    //   /** What an approval of a run binds where it runs: on the thread's machine as its daemon last read it, else here. */
    //   const pinFor = (threadId: string, decl: SlateRunDecl): Pin => (onMachine(threadId, decl) ? pins.pin(threadId, folderFor(threadId, decl), decl) : pinHere(folderFor(threadId, decl), decl));
    //   /** The views a start reads, with the scripts the thread's machine holds hashed again, so a key holds what runs. */
    //   const startable = (r: SlateRecord): Promise<ReadonlyMap<string, SlateJson | undefined>> => {
    //     const there = Object.values(r.document?.runs ?? {}).flatMap(decl => (onMachine(r.threadId, decl) ? [{ folder: folderFor(r.threadId, decl), decl }] : []));
    //     return there.length === 0 ? viewsFor(r) : Promise.all([viewsFor(r), pins.read(r.threadId, there, deps.asleep?.(r.threadId) === true)]).then(([views]) => views);
    //   };
    //   /** A press or the agent's start wakes a napping machine anyway: first here, so its pins are read and an Always holds. */
    //   const wakeFor = async (r: SlateRecord, starts: readonly { run: string; by: RunBy }[]): Promise<void> => {
    //     if (deps.asleep?.(r.threadId) !== true || !starts.some(s => s.by !== "timer" && r.document?.runs[s.run] !== undefined && pinFor(r.threadId, r.document.runs[s.run]!).unpinned.some(u => u.why === "unread"))) return;
    //     if (await Promise.resolve(deps.wake?.(r.threadId)).then(() => true, () => false)) await startable(r);
    //   };
    //   /** Each start held on the sheet whose key moved (its command was rewritten, its box was read), held again as now. */
    //   const heldAgain = (r: SlateRecord, views: ReadonlyMap<string, SlateJson | undefined>): string[] =>
    //     runs.held(r.threadId).flatMap(a => {
    //       const decl = r.document?.runs[a.run];
    //       if (decl?.kind !== "cmd" || runs.key(approvalDecl(r, decl) as CmdRunDecl) === a.key) return [];
    //       r.values[a.run] = asJson(startNow(r, a.run, "person", views).record);
    //       return [a.run];
    //     });
    //
    //   const folderFor = (threadId: string, decl: SlateRunDecl): string | undefined => {
    //     const facts = deps.thread(threadId);
    //     return decl.kind === "cmd" && decl.on === "host" ? (facts?.hostFolder ?? facts?.folder) : facts?.folder;
    //   };
    pub(crate) fn on_machine(&self, thread_id: &str, decl: &SlateRunDecl) -> bool {
        todo!()
    }
    pub(crate) fn pin_for(&self, thread_id: &str, decl: &SlateRunDecl) -> Pin {
        todo!()
    }
    pub(crate) async fn startable(self: Rc<Self>, r: &SlateRecord) -> Views {
        todo!()
    }
    pub(crate) async fn wake_for(self: Rc<Self>, r: &SlateRecord, starts: &[(String, RunBy)]) {
        todo!()
    }
    pub(crate) fn held_again(self: &Rc<Self>, r: RecordRef, views: &Views) -> Vec<String> {
        todo!()
    }
    pub(crate) fn folder_for(&self, thread_id: &str, decl: &SlateRunDecl) -> Option<String> {
        todo!()
    }

    // slates.ts:826
    //   /** A declared run started through the runs module; its first record is the caller's to write. `prior` is the record
    //    * from before the batch, whose own record of the start is not the runs module's to count. */
    //   const startNow = (r: SlateRecord, run: string, by: RunBy, views: ReadonlyMap<string, SlateJson | undefined>, prior: SlateJson | undefined = r.values[run]): { record: SlateRunRecord; ask?: RunAsk } => {
    //     const decl = r.document?.runs[run];
    //     const count = isRunRecord(prior) ? prior.runs : 0;
    //     if (decl === undefined) return { record: { state: "failed", why: `$${run} is not declared`, runs: count } };
    //     const folder = folderFor(r.threadId, decl);
    //     if (decl.kind === "cmd" && folder === undefined) return { record: { state: "failed", why: NO_FOLDER, runs: count } };
    //     viewsNow.set(r.threadId, views);
    //     capturing = { threadId: r.threadId, run };
    //     try {
    //       const said = decl.kind === "resource" ? undefined : decl.confirm;
    //       const confirm = said === undefined ? undefined : slateText(resolveSlateProp(said, contextOf(r, views)) ?? null) || `Run $${run}?`;
    //       const asked = { ...approvalDecl(r, decl), ...(confirm !== undefined ? { confirm } : {}) };
    //       const answer =
    //         asked.kind !== "cmd"
    //           ? mcp.start({ threadId: r.threadId, run, decl: asked as McpRunDecl, by, args: mcpArgsOf(r, asked as McpRunDecl), ...(isRunRecord(prior) ? { last: prior as unknown as RunRecord } : {}) })
    //           : runs.start({
    //               threadId: r.threadId,
    //               run,
    //               decl: asked as CmdRunDecl,
    //               by,
    //               folder: folder!,
    //               inputs: inputsOf(r, decl as Extract<SlateRunDecl, { kind: "cmd" }>),
    //               ...(isRunRecord(prior) ? { last: prior as unknown as RunRecord } : {}),
    //               ...((asked as CmdRunDecl).on !== "host" && deps.asleep?.(r.threadId) === true ? { asleep: true } : {}),
    //             });
    //       startedBy.set(byKey(r.threadId, run), by);
    //       if (answer.record.state === "running") announce(r, "run", by, [], run);
    //       return { record: answer.record as SlateRunRecord, ...(answer.outcome === "held" && answer.ask !== undefined ? { ask: answer.ask } : {}) };
    //     } finally {
    //       capturing = undefined;
    //     }
    //   };
    /// `prior` none is the record's own value; `Some(None)` is no prior at all.
    pub(crate) fn start_now(self: &Rc<Self>, r: RecordRef, run: &str, by: RunBy, views: &Views, prior: Option<Option<Value>>) -> Started {
        todo!()
    }

    // slates.ts:861
    //   /** The record a start inside a batch will give: running where the person said always and nothing asks every
    //    * time, else held for the sheet. */
    //   const provisional = (r: SlateRecord, run: string): SlateRunRecord => {
    //     const decl = r.document?.runs[run];
    //     const prior = r.values[run];
    //     const count = isRunRecord(prior) ? prior.runs : 0;
    //     if (decl !== undefined && decl.kind !== "cmd") return mcp.provisional(r.threadId, withFiles(r.document, decl) as McpRunDecl, isRunRecord(prior) ? (prior as unknown as RunRecord) : undefined, mcpArgsOf(r, decl)()) as SlateRunRecord;
    //     return approvedAlways(r, run) ? (runningRecord(isRunRecord(prior) ? lastResult(prior as unknown as RunRecord) : undefined, count + 1, deps.now()) as SlateRunRecord) : { state: "held", why: HELD_APPROVAL, runs: count };
    //   };
    //
    //   /** Whether the person said "Always in this thread" to the run as declared now, and it asks nothing every start. */
    //   const approvedAlways = (r: SlateRecord, run: string): boolean => {
    //     const decl = r.document?.runs[run];
    //     return decl?.kind === "cmd" && decl.confirm === undefined && r.approvals[runs.key(approvalDecl(r, decl) as CmdRunDecl)]?.state === "allowed";
    //   };
    pub(crate) fn provisional(self: &Rc<Self>, r: RecordRef, run: &str) -> RunRecord {
        todo!()
    }
    pub(crate) fn approved_always(&self, r: &SlateRecord, run: &str) -> bool {
        todo!()
    }

    // slates.ts:877
    //   /** A record the runs module wrote outside a batch (an end, a cancel, a queued start, an approval): one batch, so
    //    * done reactions fire with the window closed (02, "Runs"). */
    //   async function runMoved(threadId: string, run: string, record: RunRecord): Promise<void> {
    //     const r = records.get(threadId);
    //     if (r?.document?.runs[run] === undefined) return;
    //     if (record.state === "running") announce(r, "run", startedBy.get(byKey(threadId, run)) ?? "person", [], run);
    //     await batch(r, [{ path: `$${run}`, value: asJson(record) }], "run", {});
    //   }
    pub(crate) async fn run_moved(self: Rc<Self>, thread_id: String, run: String, record: RunRecord) -> Result<(), HostError> {
        todo!()
    }

    // slates.ts:895
    //   /** One arrival through the batch (02, "The batch"): the module's pure core with a start that runs here, then the
    //    * cancels, the sends composed, the values saved and pushed. Called inside serial. */
    //   async function batch(r: SlateRecord, input: { path: string; value: SlateJson }[], by: BatchBy, o: { event?: BatchEvent; starts?: { run: string; by: RunBy }[]; requestId?: string; sendAt?: number; react?: false }): Promise<BatchOut> {
    //     const doc = r.document;
    //     if (doc === null) return { asks: [], started: [], sends: [] };
    //     const views = await startable(r);
    //     const asks: RunAsk[] = [];
    //     const before = r.values;
    //     const deferred: { run: string; by: RunBy; record: SlateRunRecord }[] = [];
    //     const ctx = {
    //       ...contextOf(r, views),
    //       // A start record from outside the document is a run's result, which the host writes on its own road (no A607).
    //       by: by === "timer" ? "run" : by,
    //       ...(o.event !== undefined ? { event: o.event } : {}),
    //       reactionSends: r.sendsAllowed !== undefined,
    //       ...(o.react === false ? { react: false } : {}),
    //       // A run's env reads the values as this batch leaves them, so the batch gets the record the start will give and
    //       // the command is spawned once they are stored.
    //       start: (run: string, startBy: "person" | "reaction"): SlateRunRecord => {
    //         const record = provisional(r, run);
    //         deferred.push({ run, by: by === "timer" ? "timer" : startBy, record });
    //         return record;
    //       },
    //     };
    //     await wakeFor(r, o.starts ?? []);
    //     // A timer's or the agent's start is no step of the document: it starts here and its record goes in as a run's write.
    //     const timed = (o.starts ?? []).map(s => ({ path: `$${s.run}`, value: asJson(startNow(r, s.run, s.by, views).record) }));
    //     const result = runSlateBatch(doc, r.values, [...input, ...timed], ctx);
    //     if (sizeOf(result.values) > SLATE_LIMITS.valuesBytes) {
    //       keepProblems(r, [problem("S500", "values-too-big", `the values would be ${sizeOf(result.values)} bytes; a slate holds at most ${SLATE_LIMITS.valuesBytes}`)]);
    //       await save(r);
    //       return { asks, started: [], sends: [] };
    //     }
    //     r.values = result.values;
    //     keepProblems(r, result.problems);
    //     for (const run of result.cancels) runs.cancel(r.threadId, run);
    //     await wakeFor(r, deferred);
    //     for (const d of deferred) {
    //       const started = startNow(r, d.run, d.by, views, before[d.run]);
    //       if (started.ask !== undefined) asks.push(started.ask);
    //       // The runs module held it for a reason the batch could not see (four running, the start budget): that record
    //       // goes through a batch of its own.
    //       if (started.record.state === d.record.state) r.values[d.run] = asJson(started.record);
    //       else void serial(r.threadId, () => runMoved(r.threadId, d.run, started.record as RunRecord));
    //     }
    //
    //     const sends: BatchOut["sends"] = [];
    //     const now = o.sendAt ?? deps.now();
    //     for (const send of result.sends as BatchSend[]) {
    //       const fromPress = send.by === "person";
    //       const last = (fromPress ? pressSentAt : reactionSentAt).get(r.threadId);
    //       if (last !== undefined && now - last < (fromPress ? PRESS_SEND_MS : REACTION_SEND_MS)) {
    //         if (fromPress) throw refused(problem("V753", "send-rate", "too fast; try again in a moment"), "conflict");
    //         keepProblems(r, [problem("R912", "reaction-failed", `reaction ${send.reaction ?? "?"}: V753 send-rate, one message a minute from reactions`)]);
    //         continue;
    //       }
    //       (fromPress ? pressSentAt : reactionSentAt).set(r.threadId, now);
    //       sends.push({ prompt: compose(r, send, views, o.event, now), kind: send.do, requestId: o.requestId ?? `${r.threadId}:${r.version}:${send.reaction ?? send.piece ?? "slate"}:${now}` });
    //     }
    //
    //     const changed = new Set<string>();
    //     for (const name of new Set([...Object.keys(before), ...Object.keys(r.values)])) if (!slateEqual(before[name], r.values[name])) changed.add(name);
    //     if (changed.size > 0 || result.problems.length > 0) {
    //       r.revision += 1;
    //       await save(r);
    //       push(r, changed);
    //     }
    //     return { asks, started: deferred.map(d => d.run), sends };
    //   }
    /// Called on the thread's chain.
    pub(crate) async fn batch(self: Rc<Self>, r: RecordRef, input: Vec<SlateWrite>, by: BatchBy, o: BatchOptions) -> Result<BatchOut, HostError> {
        todo!()
    }

    // slates.ts:965
    //   /** The message a send puts into the thread (09, "The message"): the literal text, a blank line, one `slate:` line
    //    * whose values are the host's, scrubbed and defanged, cut to 20,000 characters longest value first. */
    //   function compose(r: SlateRecord, send: BatchSend, views: ReadonlyMap<string, SlateJson | undefined>, event: BatchEvent | undefined, now: number): string {
    //     const ctx = contextOf(r, views, event);
    //     const named = send.with ?? [];
    //     let carried: SlateJson = Object.fromEntries(named.map(path => [path, defanged(mapStrings(send.values?.[path] ?? evaluateSlateExpression(path, ctx) ?? null, s => scrub(r, s)))]));
    //     const fromPress = send.by === "person";
    //     const pieceId = send.piece ?? (fromPress ? event?.piece : undefined);
    //     const piece = pieceId !== undefined ? r.document?.pieces[pieceId] : undefined;
    //     const rowActions = piece?.props?.["rowActions"];
    //     const owner = event?.rowAction !== undefined && Array.isArray(rowActions) ? (rowActions[event.rowAction] as { label?: unknown } | undefined) : undefined;
    //     const labelled = owner !== undefined ? owner.label : piece?.props?.["label"];
    //     const label = typeof labelled === "string" ? labelled.slice(0, 80) : undefined;
    //     const keyProp = piece?.props?.["key"];
    //     const key = event?.index === undefined || keyProp === undefined ? undefined : resolveSlateProp(keyProp, ctx);
    //     const line = (w: SlateJson): string =>
    //       `slate: ${JSON.stringify({
    //         v: 2,
    //         kind: fromPress ? "action" : "reaction",
    //         thread: threadWord(r.threadId),
    //         version: r.version,
    //         ...(pieceId !== undefined ? { piece: pieceId } : {}),
    //         ...(label !== undefined ? { label } : {}),
    //         ...(event !== undefined && fromPress ? { event: event.kind } : {}),
    //         ...(send.reaction !== undefined ? { reaction: send.reaction } : {}),
    //         ...(event?.index !== undefined && fromPress ? { index: event.index } : {}),
    //         ...(key !== undefined ? { key } : {}),
    //         ...(named.length > 0 ? { with: w } : {}),
    //         by: fromPress ? "person" : "reaction",
    //         at: new Date(now).toISOString(),
    //       })}`;
    //     let prompt = `${send.text}\n\n${line(carried)}`;
    //     for (let i = 0; prompt.length > MESSAGE_CHARS && i < 50; i++) {
    //       const long = longest(carried);
    //       if (long === undefined || long.length < 20) break;
    //       carried = cutAt(carried, long.at, Math.max(0, long.length - (prompt.length - MESSAGE_CHARS) - 10));
    //       prompt = `${send.text}\n\n${line(carried)}`;
    //     }
    //     return prompt;
    //   }
    pub(crate) fn compose(&self, r: &SlateRecord, send: &BatchSend, views: &Views, event: Option<&BatchEvent>, now: i64) -> String {
        todo!()
    }

    // slates.ts:1006
    //   /** The sends a batch composed, delivered outside the record's queue: a start that waits on a running turn must not
    //    * hold up that turn's own writes to the slate. */
    //   const deliverAll = async (r: SlateRecord, sends: BatchOut["sends"]): Promise<{ outcome: "started" | "steered" | "queued"; turnId?: string; kind: "send" | "steer" | "queue" } | undefined> => {
    //     let first: { outcome: "started" | "steered" | "queued"; turnId?: string; kind: "send" | "steer" | "queue" } | undefined;
    //     for (const s of sends) {
    //       const landed = await deps.deliver({ threadId: r.threadId, workspaceId: r.workspaceId, prompt: s.prompt, requestId: s.requestId });
    //       first ??= { ...landed, kind: s.kind };
    //     }
    //     return first;
    //   };
    /// The first outcome, with the send's kind.
    pub(crate) async fn deliver_all(self: Rc<Self>, thread_id: String, workspace_id: String, sends: Vec<ComposedSend>) -> Result<Option<(Delivered, String)>, HostError> {
        todo!()
    }
}
