// SPDX-License-Identifier: AGPL-3.0-only
//! Unit slates_writes: slates.ts:464-468, 1017-1093, 1118-1231 at 65f6b89c7. Spec: spec/slates_writes.md.
use std::rc::Rc;

use wsp_wire::types::slate_wire::{SlateBy, SlateCause};
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

use super::{RecordRef, SlatesInner, StateParams, WriteParams};
use crate::pending::slate::{SlateDoc, SlateProblem, SlateValues};
use crate::pending::wire::{SlateStateAnswer, SlateWriteAnswer};

impl SlatesInner {
    // slates.ts:464
    //   const writes = writeBudget(deps.now);
    //   const spendWrite = (threadId: string): void => {
    //     const wait = writes.spend(threadId);
    //     if (wait !== undefined) throw refused(problem("V751", "write-rate", `wait ${wait} s; a slate that rewrites itself constantly is a bug`), "conflict");
    //   };
    /// V751 when the agent's budget is spent.
    pub(crate) fn spend_write(&self, thread_id: &str) -> Result<(), HostError> {
        todo!()
    }

    // slates.ts:1019
    //   /** A check with values or a press: the batch's pure core over a copy, nothing stored, started or sent, and the
    //    * sketch as the slate would read then, with what the press would have done after it. */
    //   const rehearse = async (r: SlateRecord, doc: SlateDoc, values: SlateValues, p: { values?: Record<string, unknown>; press?: { piece: string; index?: number; action?: number } }, warnings: SlateProblem[]): Promise<SlateWriteAnswer> => {
    //     // A run's own fields, $run.json or $run.state, set on the copy before the batch: a preview of what the panel shows
    //     // for a result the run has not had, which no live write may set.
    //     const asked = Object.entries(p.values ?? {}).map(([path, value]) => ({ path: ownPath(path), value: value as SlateJson }));
    //     let seeded = values;
    //     for (const w of asked) {
    //       const own = parseSlateOwnPath(w.path);
    //       if (own === undefined || doc.runs[own.name] === undefined) continue;
    //       const was = isRunRecord(seeded[own.name]) ? (seeded[own.name] as Record<string, SlateJson>) : {};
    //       const base = asJson({ state: "done", runs: 1, ...was, ...(was["endedAt"] === undefined ? { state: "done", endedAt: deps.now() } : {}) });
    //       seeded = setSlateValue({ ...seeded, [own.name]: base }, w.path, w.value) ?? seeded;
    //     }
    //     const preview: SlateRecord = { ...r, document: doc, values: seeded };
    //     const views = await viewsFor(preview);
    //     const writes = asked.filter(w => { const own = parseSlateOwnPath(w.path); return own === undefined || doc.runs[own.name] === undefined; });
    //     const press = p.press;
    //     if (press !== undefined && doc.pieces[press.piece] === undefined) {
    //       const near = slateNearest(press.piece, Object.keys(doc.pieces));
    //       throw invalid([problem("D203", "piece-missing", `there is no piece ${press.piece} to press`, near !== undefined ? { fix: near } : {})], warnings);
    //     }
    //     const result = runSlateBatch(doc, seeded, writes, { ...contextOf(preview, views), by: "person", ...(press !== undefined ? { event: { piece: press.piece, kind: "press", ...(press.index !== undefined ? { index: press.index } : {}), ...(press.action !== undefined ? { rowAction: press.action } : {}) } } : {}) });
    //     const after = sketched({ ...preview, values: result.values }, views, false, [], warnings);
    //     const would = [
    //       ...result.starts.map(s => `would start $${s.run} (${s.why})`),
    //       ...result.cancels.map(run => `would cancel $${run}`),
    //       ...result.sends.map(s => `would ${s.do} ${JSON.stringify(s.text)}`),
    //       ...result.other.map(o => `would ${o.step.do} in the window`),
    //     ];
    //     return { version: r.version, text: would.length > 0 ? `${after}\n${would.join("\n")}` : after, warnings, problems: result.problems };
    //   };
    pub(crate) async fn rehearse(
        self: Rc<Self>,
        r: RecordRef,
        doc: SlateDoc,
        values: SlateValues,
        p: &WriteParams,
        warnings: Vec<SlateProblem>,
    ) -> Result<SlateWriteAnswer, HostError> {
        todo!()
    }

    // slates.ts:1052
    //   const writeDocument = async (r: SlateRecord, next: SlateDoc | null, values: SlateValues, by: "agent" | "person", cause: SlateCause, pieces: string[], warnings: SlateProblem[]): Promise<SlateWriteAnswer> => {
    //     if (sizeOf(values) > SLATE_LIMITS.valuesBytes) throw invalid([problem("S500", "values-too-big", `the values are ${sizeOf(values)} bytes; a slate holds at most ${SLATE_LIMITS.valuesBytes}`)], warnings);
    //     dropDeclared(r, next);
    //     if (r.version > 0) r.previous = { ...snapOf(r), version: r.version };
    //     // A clear keeps the values; a document write keeps what both documents declare and fires nothing (02).
    //     r.document = next;
    //     r.values = values;
    //     if (next === null) r.empty = "cleared";
    //     else delete r.empty;
    //     r.version += 1;
    //     r.revision += 1;
    //     records.set(r.threadId, r);
    //     armTimers(r);
    //     if (next !== null) {
    //       // A timed run the person has not allowed asks now, shown or not, so this answer and their slate both say it waits.
    //       const asking = await startable(r);
    //       heldAgain(r, asking);
    //       for (const [run, decl] of Object.entries(next.runs)) {
    //         const rec = r.values[run];
    //         if (decl.every !== undefined && isRunRecord(rec) && rec.state === "idle" && provisional(r, run).state === "held") r.values[run] = asJson(startNow(r, run, "timer", asking).record);
    //       }
    //     }
    //     await save(r);
    //     announce(r, cause, by, pieces);
    //     if (next !== null && /\bpr\.checks\b/.test(JSON.stringify(next))) deps.watchPr?.(r.workspaceId);
    //     push(r, Object.keys(values));
    //     const views = await viewsFor(r);
    //     return { version: r.version, text: sketched(r, views, false, [], warnings), warnings, problems: problemsOf(r, views), waiting: waitingOf(r) };
    //   };
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn write_document(
        self: Rc<Self>,
        r: RecordRef,
        next: Option<SlateDoc>,
        values: SlateValues,
        by: SlateBy,
        cause: SlateCause,
        pieces: Vec<String>,
        warnings: Vec<SlateProblem>,
    ) -> Result<SlateWriteAnswer, HostError> {
        todo!()
    }

    // slates.ts:1082
    //   const undo = async (r: SlateRecord, by: "agent" | "person"): Promise<SlateWriteAnswer> => {
    //     const back = r.previous;
    //     if (back === undefined) throw refused(problem("V752", "nothing-to-undo", "there is no earlier slate to go back to; the undo keeps one step"), "conflict");
    //     r.previous = { ...snapOf(r), version: r.version };
    //     become(r, back);
    //     r.version += 1;
    //     r.revision += 1;
    //     await save(r);
    //     announce(r, "undo", by, []);
    //     push(r, Object.keys(r.values));
    //     return { version: r.version, text: await sketchOf(r), warnings: [], problems: [] };
    //   };
    pub(crate) async fn undo(self: Rc<Self>, r: RecordRef, by: SlateBy) -> Result<SlateWriteAnswer, HostError> {
        todo!()
    }

    // slates.ts:1118
    //     async write(p, caller) {
    //       const threadId = targetOf(p, caller, true);
    //       const given = [p.text, p.document].filter(v => v !== undefined).length;
    //       const rehearsal = p.values !== undefined || p.press !== undefined;
    //       if (rehearsal && p.check !== true) throw usageRefusal("values and press rehearse a slate, which only a check does.", "Add check, or set the live values with slate_state.");
    //       if (given > 1 || (given === 0 && !rehearsal)) throw Object.assign(usageRefusal(`A601 step-arg: a slate write takes exactly one of text or document, and this one has ${given === 0 ? "none" : "both"}.`, "Send text alone."), { code: "A601" });
    //       const by = byOf(caller);
    //       const whole = p.document !== undefined || (p.text !== undefined && p.text.trimStart().startsWith("<slate"));
    //       return serial(threadId, async () => {
    //         const r = (await recordOf(threadId)) ?? freshRecord(threadId);
    //         if (given === 0) {
    //           if (r.document === null) throw usage(problem("Z802", "no-slate", "this thread has no slate to rehearse; send the <slate> with check"));
    //           return rehearse(r, r.document, r.values, p, []);
    //         }
    //         if (whole) {
    //           const checked = p.text !== undefined ? parseSlate(p.text) : validateSlate(p.document);
    //           const errors = [...checked.errors, ...(await refusedOn(threadId, r.document, checked.document))];
    //           if (errors.length > 0 || checked.document === undefined) throw invalid(errors, checked.warnings);
    //           const doc = checked.document;
    //           if (rehearsal) return rehearse(r, doc, valuesFor(r, doc), p, checked.warnings);
    //           if (p.check === true) {
    //             const preview: SlateRecord = { ...r, document: doc, values: valuesFor({ threadId, document: null, values: {} }, doc) };
    //             return { version: r.version, text: sketched(preview, new Map(), true, [], checked.warnings), warnings: checked.warnings, problems: [] };
    //           }
    //           checkVersion(r, p.ifVersion);
    //           if (by === "agent") spendWrite(threadId);
    //           return writeDocument(r, doc, valuesFor(r, doc), by, "write", Object.keys(doc.pieces), checked.warnings);
    //         }
    //         if (r.version === 0) throw usage(problem("Z802", "no-slate", "this thread has no slate to patch; write a whole <slate> first"));
    //         const parsed = parseSlatePatch(p.text!, r.document);
    //         if (parsed.errors.length > 0 || parsed.patch === undefined) throw invalid(parsed.errors, []);
    //         const ops = parsed.patch.ops;
    //         if (ops.length === 1 && ops[0]!.op === "undo") {
    //           if (p.check === true) return { version: r.version, text: await sketchOf(r), warnings: [], problems: [] };
    //           checkVersion(r, p.ifVersion);
    //           return undo(r, by);
    //         }
    //         const applied = applySlatePatch(r.document, r.values, parsed.patch, parsed.lines);
    //         const errors = [...applied.errors, ...(await refusedOn(threadId, r.document, applied.document))];
    //         if (errors.length > 0 || applied.document === undefined) throw invalid(errors, applied.warnings);
    //         const next = applied.document;
    //         const values = next === null ? r.values : valuesFor({ threadId, document: r.document, values: applied.values ?? r.values }, next);
    //         if (rehearsal && next !== null) return rehearse(r, next, values, p, applied.warnings);
    //         if (p.check === true) return { version: r.version, text: sketched({ ...r, document: next, values }, new Map(), true, [], applied.warnings), warnings: applied.warnings, problems: [] };
    //         checkVersion(r, p.ifVersion);
    //         if (by === "agent") spendWrite(threadId);
    //         const touched = [...new Set(ops.flatMap(op => ("id" in op && typeof op.id === "string" ? [op.id] : [])))];
    //         return writeDocument(r, next, values, by, next === null ? "clear" : "write", touched, applied.warnings);
    //       });
    //     },
    pub(crate) async fn write(self: Rc<Self>, p: WriteParams, caller: Option<&Caller>) -> Result<SlateWriteAnswer, HostError> {
        todo!()
    }

    // slates.ts:1169
    //     async state(p, caller) {
    //       const threadId = targetOf(p, caller, true);
    //       const by = byOf(caller);
    //       // A computer the person paired types into the slate and starts nothing on this one, by name or by reaction.
    //       const paired = roadOf(caller) === "paired";
    //       if (paired && (p.start ?? []).length > 0) throw usageRefusal("a paired computer writes a slate's values and starts no run.", "Press it on the computer the slate runs on.");
    //       return serial(threadId, async () => {
    //         const r = await needRecord(threadId);
    //         const doc = r.document;
    //         if (doc === null) throw usage(problem("Z802", "no-slate", "this slate is empty; write one with slate_write"));
    //         checkVersion(r, p.ifVersion);
    //         if (Object.keys(p.values ?? {}).length === 0 && (p.start ?? []).length === 0) throw usageRefusal("slate state sets values or starts runs, and this one names neither.", "Give values, start, or both.");
    //         // The agent starts only what the person already let run every time; anything else waits for their press.
    //         const starts: { run: string; by: RunBy }[] = [];
    //         const notStarted: string[] = [];
    //         for (const named of p.start ?? []) {
    //           const run = named.replace(/^\$/, "");
    //           if (doc.runs[run] === undefined) {
    //             const near = slateNearest(run, Object.keys(doc.runs));
    //             throw invalid([problem("K702", "run-name", `$${run} is not a run`, near !== undefined ? { fix: `$${near}` } : {})], []);
    //           }
    //           const decl = doc.runs[run]!;
    //           const rec = r.values[run];
    //           if (approvedAlways(r, run)) starts.push({ run, by: by === "agent" ? "agent" : "person" });
    //           else if (isRunRecord(rec) && rec.state === "held") notStarted.push(`$${run}: it already waits for the person to allow it on the slate, and starts once they do`);
    //           else if (decl.kind !== "resource" && decl.confirm !== undefined) notStarted.push(`$${run}: it has confirm, so it asks the person every start and only a press or a <when> starts it`);
    //           else notStarted.push(`$${run}: you start only a run the person allowed "Always in this thread"; a press, a <when> or every= starts it and the slate asks them`);
    //         }
    //         const input: { path: string; value: SlateJson }[] = [];
    //         for (const [written, value] of Object.entries(p.values ?? {})) {
    //           const path = ownPath(written);
    //           const own = parseSlateOwnPath(path);
    //           if (own === undefined) throw invalid([problem("S501", "name-undeclared", `${written} is not a value path; a value path reads $name, with .field and [index] below it`)], []);
    //           if (doc.derived[own.name] !== undefined || doc.runs[own.name] !== undefined) throw invalid([problem("A607", "set-not-value", `$${own.name} is a ${doc.derived[own.name] !== undefined ? "derived value" : "run"} and cannot be written`, { fix: "write the value it reads" })], []);
    //           const decl = doc.values[own.name];
    //           if (decl === undefined) {
    //             const near = slateNearest(own.name, Object.keys(doc.values));
    //             throw invalid([problem("S501", "name-undeclared", `$${own.name} is not declared`, near !== undefined ? { fix: `$${near}` } : {})], []);
    //           }
    //           if (decl.secret === true) {
    //             // The person's typing is the one road in for a secret, and it stops here: the batch sees the handle.
    //             if (by === "agent" || own.segs.length > 0) throw invalid([problem("S520", "secret-exposed", `$${own.name} is a secret: only the person types it, into its input`)], []);
    //             if (typeof value !== "string") throw invalid([problem("S520", "secret-exposed", `$${own.name} takes the typed text`)], []);
    //             const handle = value === "" ? runs.secrets.clear(threadId, own.name) : runs.secrets.set(threadId, own.name, value, decl.keep === true ? { keep: true } : {});
    //             input.push({ path: `$${own.name}`, value: asJson(handle) });
    //             continue;
    //           }
    //           input.push({ path, value: value as SlateJson });
    //         }
    //         if (by === "agent") {
    //           spendWrite(threadId);
    //           // The agent's write frees a run its start budget held (02), but a call that starts runs spends that budget.
    //           if (input.length > 0 && starts.length === 0) runs.release(threadId);
    //         }
    //         const out = input.length > 0 ? await batch(r, input, by, paired ? { react: false } : {}) : { asks: [], sends: [] };
    //         if (by === "agent" && input.length > 0) announce(r, "state", by, []);
    //         if (starts.length > 0) await batch(r, [], "run", { starts });
    //         void deliverAll(r, out.sends).catch((e: unknown) => console.warn(`a slate send in thread ${threadWord(threadId)} was not delivered: ${e instanceof Error ? e.message : String(e)}`));
    //         const views = await viewsFor(r);
    //         const text = sketched(r, views);
    //         return { version: r.version, text: notStarted.length > 0 ? `${text}\nnot started:\n${notStarted.map(n => `  ${n}`).join("\n")}` : text, problems: problemsOf(r, views), waiting: waitingOf(r), ...(notStarted.length > 0 ? { notStarted } : {}) };
    //       });
    //     },
    pub(crate) async fn state(self: Rc<Self>, p: StateParams, caller: Option<&Caller>) -> Result<SlateStateAnswer, HostError> {
        todo!()
    }
}
