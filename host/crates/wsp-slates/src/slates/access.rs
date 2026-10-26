// SPDX-License-Identifier: AGPL-3.0-only
//! Unit slates_access: slates.ts:294-381, 426-462, 476-555, 1095-1116, 1281-1288, 1450-1480, 1495-1563 at
//! 65f6b89c7. Spec: spec/slates_access.md.
use std::future::Future;
use std::rc::{Rc, Weak};

use wsp_wire::types::slate_wire::{SlateBy, SlateCause};
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

use super::model::SlateRecord;
use super::{RecordRef, SlatesInner};
use crate::pending::slate::SlateProblem;
use crate::pending::wire::SlateView;
use crate::slate_runs::{RunApprovals, RunRecord};

// slates.ts:314
//   const approvals: RunApprovals = {
//       has: (threadId, key) => records.get(threadId)?.approvals[key]?.state === "allowed",
//       allow: (threadId, key) => {
//         const r = records.get(threadId);
//         if (r === undefined) return;
//         r.approvals[key] = { state: "allowed", at: deps.now(), ...approvalNames(r, key) };
//         void save(r);
//       },
//       revoke: (threadId, key) => {
//         const r = records.get(threadId);
//         if (r?.approvals[key] === undefined) return;
//         delete r.approvals[key];
//         void save(r);
//       },
//       list: threadId => Object.entries(records.get(threadId)?.approvals ?? {}).flatMap(([k, a]) => (a.state === "allowed" ? [k] : [])),
//   };
/// The approvals the runs table reads, backed by the records.
pub struct RecordApprovals {
    pub(crate) area: Weak<SlatesInner>,
}

impl RunApprovals for RecordApprovals {
    fn has(&self, thread_id: &str, key: &str) -> bool {
        todo!()
    }
    fn allow(&self, thread_id: &str, key: &str) {
        todo!()
    }
    fn revoke(&self, thread_id: &str, key: &str) {
        todo!()
    }
    fn list(&self, thread_id: &str) -> Vec<String> {
        todo!()
    }
}

impl SlatesInner {
    // slates.ts:297
    //   /** One change at a time per thread: a batch, a write and a turn's end never interleave on one record. */
    //   const serial = <T>(threadId: string, run: () => Promise<T>): Promise<T> => {
    //     const next = (queues.get(threadId) ?? Promise.resolve()).then(run, run);
    //     queues.set(threadId, next.catch(() => {}));
    //     return next;
    //   };
    /// The thread's chain (rule R2): `run` starts when the previous task settles; the caller gets its own result.
    pub(crate) fn serial<T: Clone + 'static, F: Future<Output = Result<T, HostError>> + 'static>(
        self: &Rc<Self>,
        thread_id: &str,
        run: impl FnOnce() -> F + 'static,
    ) -> impl Future<Output = Result<T, HostError>> + 'static {
        async { todo!() }
    }

    // slates.ts:303
    //   const save = (r: SlateRecord): Promise<void> => {
    //     r.updatedAt = deps.now();
    //     return deps.store.put(SLATES, r.threadId, r);
    //   };
    pub(crate) async fn save(self: Rc<Self>, r: RecordRef) -> Result<(), HostError> {
        todo!()
    }

    // slates.ts:330
    //   const folderOf = (threadId: string): string | undefined => (deps.slatesDir === undefined ? undefined : join(deps.slatesDir, threadId));
    //   const writeFiles = (threadId: string): string => writeSlateFiles(folderOf(threadId)!, records.get(threadId)?.document?.files ?? {});
    //   const moved = (threadId: string, run: string, record: RunRecord): void => {
    //     if (capturing?.threadId === threadId && capturing.run === run) return;
    //     void serial(threadId, () => runMoved(threadId, run, record)).catch((e: unknown) => console.warn(`the slate of thread ${threadWord(threadId)} lost a record of $${run}: ${e instanceof Error ? e.message : String(e)}`));
    //   };
    pub(crate) fn moved(self: &Rc<Self>, thread_id: &str, run: &str, record: RunRecord) {
        todo!()
    }

    // slates.ts:367
    //   /** Runs a settled thread's timer skipped while no window showed the slate, started the moment one does. */
    //   const skipped = new Map<string, Set<string>>();
    //   /** The windows showing each thread's slate, whatever sources they hold. */
    //   const windows = new Map<string, number>();
    //   function timed(threadId: string, run: string): void {
    //     void serial(threadId, async () => {
    //       const r = records.get(threadId);
    //       if (r?.document?.runs[run] === undefined) return;
    //       if ((windows.get(threadId) ?? 0) === 0 && (await deps.settled?.(threadId)) === true) {
    //         skipped.set(threadId, (skipped.get(threadId) ?? new Set()).add(run));
    //         return;
    //       }
    //       await batch(r, [], "timer", { starts: [{ run, by: "timer" }] });
    //     }).catch(() => {});
    //   }
    pub(crate) fn timed(self: &Rc<Self>, thread_id: &str, run: &str) {
        todo!()
    }

    // slates.ts:426
    //   let loaded: Promise<void> | undefined;
    //   const ready = (): Promise<void> =>
    //     (loaded ??= Promise.resolve()
    //       .then(() => deps.loaded?.())
    //       .then(() => deps.store.list(SLATES))
    //       .then(async list => {
    //         // Schema 1 was the first proof's and never shipped: no migration is owed, so its records are not read.
    //         for (const stored of list as SlateRecord[]) if (stored.schema === 2 && !records.has(stored.threadId)) records.set(stored.threadId, { ...stored, revision: stored.revision ?? stored.version });
    //         for (const r of [...records.values()]) await serial(r.threadId, () => recover(r));
    //       }));
    pub(crate) async fn ready(self: Rc<Self>) -> Result<(), HostError> {
        todo!()
    }

    // slates.ts:437
    //   /** A record loaded at start (01, "Host restart"): secrets read off the store, so a memory one reads unfilled; every
    //    * run that read running is failed and its done fires, one batch per slate; held runs held again so the sheet has
    //    * something to approve; timers armed. Nothing restarts by itself. */
    //   async function recover(r: SlateRecord): Promise<void> {
    //     refreshSecrets(r);
    //     const doc = r.document;
    //     if (doc === null) return;
    //     const input: { path: string; value: SlateJson }[] = [];
    //     for (const name of Object.keys(doc.runs)) {
    //       const rec = r.values[name];
    //       if (isRunRecord(rec) && rec.state === "running") input.push({ path: `$${name}`, value: asJson(restartedRecord(rec as unknown as RunRecord, deps.now())) });
    //     }
    //     if (input.length > 0) await batch(r, input, "run", {});
    //     else await save(r);
    //     armTimers(r);
    //     const views = await viewsFor(r);
    //     for (const name of Object.keys(doc.runs)) {
    //       const rec = r.values[name];
    //       if (isRunRecord(rec) && rec.state === "held" && rec.why === HELD_APPROVAL) startNow(r, name, "person", views);
    //     }
    //     // The box is read after the load, so one that does not answer holds no start-up back.
    //     if (runs.held(r.threadId).length > 0) void serial(r.threadId, async () => {
    //       const moved = heldAgain(r, await startable(r));
    //       if (moved.length > 0) await save(r).then(() => push(r, moved));
    //     }).catch(() => {});
    //   }
    pub(crate) async fn recover(self: Rc<Self>, r: RecordRef) -> Result<(), HostError> {
        todo!()
    }

    // slates.ts:476
    //   /** A thread another thread started has no slate: every slate tool tells it so in one line. */
    //   const refuseSubThread = (caller: Caller | undefined): void => {
    //     const scope = scopeOf(caller);
    //     if (scope !== undefined && scope.rootThreadId !== scope.threadId) {
    //       throw Object.assign(new Error("Z803 sub-thread: a thread another thread started has no slate; tell the thread that started you what to show, and it puts it on its own"), { kind: "usage", code: "Z803" });
    //     }
    //   };
    pub(crate) fn refuse_sub_thread(&self, caller: Option<&Caller>) -> Result<(), HostError> {
        todo!()
    }

    // slates.ts:484
    //   /** The thread a slate op is about, read off the caller's token before any argument (12, cross-thread). */
    //   const targetOf = (p: SlateTarget, caller: Caller | undefined, write: boolean): string => {
    //     refuseSubThread(caller);
    //     const scope = scopeOf(caller);
    //     if (scope !== undefined) {
    //       if (p.threadId === undefined || p.threadId === scope.threadId) return scope.threadId;
    //       if (write) throw Object.assign(new Error(`Z800 thread-not-yours: a thread writes its own slate, and thread ${threadWord(p.threadId)} is not this one`), { kind: "usage", code: "Z800" });
    //       if (!deps.under(scope.threadId).includes(p.threadId)) throw Object.assign(notFoundRefusal(`no thread ${threadWord(p.threadId)}`), { code: "Z801" });
    //       return p.threadId;
    //     }
    //     if (p.threadId !== undefined) {
    //       if (deps.thread(p.threadId) === undefined && !records.has(p.threadId)) throw notFoundRefusal(`no thread ${threadWord(p.threadId)}`);
    //       return p.threadId;
    //     }
    //     if (p.turnToken !== undefined) return deps.threadOfToken(p.turnToken);
    //     throw usageRefusal("a slate belongs to a thread, and this line names none and runs inside no turn.", "Name the thread by id, as wsp threads lists it.");
    //   };
    //
    //   const byOf = (caller: Caller | undefined): "agent" | "person" => (scopeOf(caller) !== undefined ? "agent" : "person");
    pub(crate) fn target_of(
        &self,
        thread_id: Option<&str>,
        turn_token: Option<&str>,
        caller: Option<&Caller>,
        write: bool,
    ) -> Result<String, HostError> {
        todo!()
    }
    /// `agent` with a token, else `person`.
    pub(crate) fn by_of(&self, caller: Option<&Caller>) -> SlateBy {
        todo!()
    }

    // slates.ts:504
    //   const recordOf = async (threadId: string): Promise<SlateRecord | undefined> => {
    //     await ready();
    //     return records.get(threadId);
    //   };
    //   const needRecord = async (threadId: string): Promise<SlateRecord> => {
    //     const r = await recordOf(threadId);
    //     if (r === undefined || (r.document === null && r.version === 0)) throw usage(problem("Z802", "no-slate", "this thread has no slate yet; write one with slate_write"));
    //     return r;
    //   };
    //   const freshRecord = (threadId: string): SlateRecord => {
    //     const facts = deps.thread(threadId);
    //     if (facts === undefined) throw notFoundRefusal(`no thread ${threadWord(threadId)}`);
    //     return { threadId, workspaceId: facts.workspaceId, rootThreadId: facts.rootThreadId, schema: 2, version: 0, revision: 0, document: null, values: {}, empty: "none", turns: {}, kept: {}, comments: [], approvals: {}, problems: [], shownOnce: false, updatedAt: deps.now() };
    //   };
    pub(crate) async fn record_of(self: Rc<Self>, thread_id: &str) -> Result<Option<RecordRef>, HostError> {
        todo!()
    }
    pub(crate) async fn need_record(self: Rc<Self>, thread_id: &str) -> Result<RecordRef, HostError> {
        todo!()
    }
    pub(crate) fn fresh_record(&self, thread_id: &str) -> Result<SlateRecord, HostError> {
        todo!()
    }

    // slates.ts:519
    //   const checkVersion = (r: SlateRecord, ifVersion: number | undefined): void => {
    //     if (ifVersion !== undefined && ifVersion !== r.version) throw refused(problem("V750", "version-behind", `the slate is at version ${r.version}, not ${ifVersion}; read it and write again`), "conflict");
    //   };
    pub(crate) fn check_version(&self, r: &SlateRecord, if_version: Option<i64>) -> Result<(), HostError> {
        todo!()
    }

    // slates.ts:523
    //   const announce = (r: SlateRecord, cause: SlateCause, by: SlateBy, pieces: string[], run?: string): void => {
    //     const facts = deps.thread(r.threadId);
    //     deps.record({
    //       type: "session.slate",
    //       workspaceId: r.workspaceId,
    //       sessionId: facts?.sessionId ?? r.threadId,
    //       ...(facts?.turnId !== undefined ? { turnId: facts.turnId } : {}),
    //       threadId: r.threadId,
    //       cause,
    //       version: r.version,
    //       by,
    //       pieces: pieces.slice(0, PIECES_NAMED),
    //       ...(run !== undefined ? { run } : {}),
    //     });
    //   };
    //
    //   const push = (r: SlateRecord, names: Iterable<string>): void => {
    //     const values = Object.fromEntries([...new Set(names)].filter(n => n in r.values).map(n => [`$${n}`, r.values[n]!]));
    //     if (Object.keys(values).length > 0) deps.emit({ type: "slate.values", workspaceId: r.workspaceId, threadId: r.threadId, version: r.version, revision: r.revision, values });
    //   };
    //
    //   const keepProblems = (r: SlateRecord, found: readonly SlateProblem[]): void => {
    //     if (found.length > 0) r.problems = [...r.problems, ...found].slice(-PROBLEMS_KEPT);
    //   };
    pub(crate) fn announce(&self, r: &SlateRecord, cause: SlateCause, by: SlateBy, pieces: &[String], run: Option<&str>) {
        todo!()
    }
    pub(crate) fn push(&self, r: &SlateRecord, names: &[String]) {
        todo!()
    }
    pub(crate) fn keep_problems(&self, r: &mut SlateRecord, found: &[SlateProblem]) {
        todo!()
    }

    // slates.ts:1098
    //     async settled() {
    //       for (let i = 0; i < 50; i++) {
    //         const pending = [...queues.values()];
    //         await Promise.all(pending);
    //         const now = [...queues.values()];
    //         if (now.length === pending.length && now.every((q, at) => q === pending[at])) return;
    //       }
    //     },
    //
    //     close() {
    //       runs.close();
    //       mcp.close();
    //       ledger.close();
    //     },
    //
    //     async get(threadId) {
    //       const r = await recordOf(threadId);
    //       return r === undefined ? null : viewOf(r);
    //     },
    pub(crate) async fn settled(self: Rc<Self>) {
        todo!()
    }
    pub(crate) fn close(&self) {
        todo!()
    }
    pub(crate) async fn get(self: Rc<Self>, thread_id: &str) -> Result<Option<SlateView>, HostError> {
        todo!()
    }

    // slates.ts:1281
    //     async shown(threadId) {
    //       await serial(threadId, async () => {
    //         const r = await recordOf(threadId);
    //         if (r === undefined || r.shownOnce) return;
    //         r.shownOnce = true;
    //         await save(r);
    //       });
    //     },
    pub(crate) async fn shown(self: Rc<Self>, thread_id: &str) -> Result<(), HostError> {
        todo!()
    }

    // slates.ts:1450
    //     subscribe(p) {
    //       const held = holds.get(p.threadId) ?? new Map<string, number>();
    //       holds.set(p.threadId, held);
    //       for (const s of p.sources) held.set(s, (held.get(s) ?? 0) + 1);
    //       // A slate with any hold on it counts as shown for its timers (01, "slates.subscribe"), and what a settled
    //       // thread's timers skipped starts now rather than at their next tick.
    //       windows.set(p.threadId, (windows.get(p.threadId) ?? 0) + 1);
    //       runs.shown(p.threadId, true);
    //       mcp.shown(p.threadId, true);
    //       for (const run of skipped.get(p.threadId) ?? []) timed(p.threadId, run);
    //       skipped.delete(p.threadId);
    //       if (p.sources.includes("pr")) {
    //         const r = records.get(p.threadId);
    //         if (r !== undefined) deps.watchPr?.(r.workspaceId);
    //       }
    //       let released = false;
    //       return () => {
    //         if (released) return;
    //         released = true;
    //         windows.set(p.threadId, Math.max(0, (windows.get(p.threadId) ?? 1) - 1));
    //         for (const s of p.sources) {
    //           const n = (held.get(s) ?? 1) - 1;
    //           if (n <= 0) held.delete(s);
    //           else held.set(s, n);
    //         }
    //         if (held.size === 0) {
    //           runs.shown(p.threadId, false);
    //           mcp.shown(p.threadId, false);
    //         }
    //       };
    //     },
    pub(crate) fn subscribe(self: &Rc<Self>, thread_id: &str, sources: &[String]) -> Rc<dyn Fn()> {
        todo!()
    }

    // slates.ts:1495
    //     async turnEnded({ threadId, turnId }) {
    //       await ready();
    //       await serial(threadId, async () => {
    //         const r = records.get(threadId);
    //         if (r === undefined) return;
    //         const key = String(r.revision);
    //         r.kept[key] ??= structuredClone(snapOf(r));
    //         delete r.turns[turnId];
    //         r.turns[turnId] = { at: deps.now(), version: r.version, revision: r.revision };
    //         trimSnapshots(r);
    //         await save(r);
    //       });
    //     },
    pub(crate) async fn turn_ended(self: Rc<Self>, thread_id: &str, turn_id: &str) -> Result<(), HostError> {
        todo!()
    }

    // slates.ts:1509
    //     async rewound({ threadId, turnId, cut }) {
    //       await ready();
    //       await serial(threadId, async () => {
    //         const r = records.get(threadId);
    //         if (r === undefined) return;
    //         const at = r.turns[turnId];
    //         const snap = at === undefined ? undefined : r.kept[String(at.revision)];
    //         // Approvals and secrets are the person's, not the document's: a rewind to before the slate keeps the values.
    //         become(r, snap !== undefined ? structuredClone(snap) : { document: null, values: r.values, empty: "rewound-before" });
    //         for (const id of cut) delete r.turns[id];
    //         trimSnapshots(r);
    //         r.version += 1;
    //         r.revision += 1;
    //         await save(r);
    //         announce(r, "restore", "host", []);
    //         push(r, Object.keys(r.values));
    //       });
    //     },
    pub(crate) async fn rewound(self: Rc<Self>, thread_id: &str, turn_id: &str, cut: &[String]) -> Result<(), HostError> {
        todo!()
    }

    // slates.ts:1528
    //     async forget(threadId, o = {}) {
    //       await serial(threadId, async () => {
    //         runs.drop(threadId, o.keep === true);
    //         mcp.drop(threadId, o.keep === true);
    //         pins.drop(threadId);
    //         records.delete(threadId);
    //         const dir = folderOf(threadId);
    //         if (dir !== undefined) rmSync(dir, { recursive: true, force: true });
    //         // Its folder on the thread's own machine goes too, now or once that machine answers again.
    //         const machine = deps.machineOf?.(threadId);
    //         if (machine !== undefined) void ledger.remove(machine, boxSlateDir(threadId));
    //         holds.delete(threadId);
    //         skipped.delete(threadId);
    //         windows.delete(threadId);
    //         for (const map of [writes, pressSentAt, reactionSentAt, eventBuckets, viewsNow]) map.delete(threadId);
    //         for (const key of startedBy.keys()) if (key.startsWith(byKey(threadId, ""))) startedBy.delete(key);
    //         await deps.store.delete(SLATES, threadId);
    //       });
    //     },
    pub(crate) async fn forget(self: Rc<Self>, thread_id: &str, keep: bool) -> Result<(), HostError> {
        todo!()
    }

    // slates.ts:1548
    //     watchesPr(workspaceId) {
    //       for (const r of records.values()) {
    //         if (r.workspaceId !== workspaceId) continue;
    //         if ((holds.get(r.threadId)?.get("pr") ?? 0) > 0) return true;
    //         if (r.document !== null && /\bpr\.checks\b/.test(JSON.stringify(r.document))) return true;
    //       }
    //       return false;
    //     },
    //
    //     closeServers(threadId) {
    //       mcp.closeServers(threadId);
    //     },
    //
    //     napped(workspaceId) {
    //       for (const r of records.values()) if (r.workspaceId === workspaceId) mcp.closeServers(r.threadId);
    //     },
    pub(crate) fn watches_pr(&self, workspace_id: &str) -> bool {
        todo!()
    }
    pub(crate) fn close_servers(&self, thread_id: &str) {
        todo!()
    }
    pub(crate) fn napped(&self, workspace_id: &str) {
        todo!()
    }
}
