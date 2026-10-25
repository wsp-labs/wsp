// SPDX-License-Identifier: AGPL-3.0-only
//! Unit slates_acts: slates.ts:470-474, 1290-1448, 1490-1493 at 65f6b89c7. Spec: spec/slates_acts.md.
use std::rc::Rc;

use wsp_words::exit::HostError;

use super::{ApproveParams, EventParams, SlatesInner};
use crate::pending::wire::{SlateEventAnswer, SlatesImageAnswer};

impl SlatesInner {
    // slates.ts:1290
    //     event(p) {
    //       const now = deps.now();
    //       for (const [id, held] of requests) if (now - held.at > REQUEST_KEPT_MS) requests.delete(id);
    //       // A press sent again after a reconnect is the same press: it answers what the first one came to.
    //       const seen = requests.get(p.requestId);
    //       if (seen !== undefined) return seen.answer;
    //       const bucket = eventBuckets.get(p.threadId) ?? { tokens: EVENTS_PER_SECOND, at: now };
    //       bucket.tokens = Math.min(EVENTS_PER_SECOND, bucket.tokens + ((now - bucket.at) / 1000) * EVENTS_PER_SECOND);
    //       bucket.at = now;
    //       eventBuckets.set(p.threadId, bucket);
    //       if (bucket.tokens < 1) return Promise.reject(refused(problem("V754", "event-rate", "too many presses; try again in a moment"), "conflict"));
    //       bucket.tokens -= 1;
    //       while (requests.size >= REQUESTS_KEPT) requests.delete(requests.keys().next().value!);
    //       const composed = serial(p.threadId, async () => {
    //         const r = await needRecord(p.threadId);
    //         const doc = r.document;
    //         // The host reads the piece and its steps off its own document, never the window's copy (finding 6j).
    //         const piece = doc?.pieces[p.piece];
    //         if (doc === null || piece === undefined) throw usageRefusal(`That part of the slate is gone (piece ${p.piece}, version ${p.version}; the slate is at ${r.version}).`, "Press it again once the slate has redrawn.");
    //         if (p.rowAction !== undefined) {
    //           const rowActions = piece.props?.["rowActions"];
    //           if (!Array.isArray(rowActions) || rowActions[p.rowAction] === undefined) throw usageRefusal(`${p.piece} has no row action ${p.rowAction}.`, "Press it again once the slate has redrawn.");
    //         }
    //         // A row's item is the host's own, off the piece's list at delivery (finding 6k).
    //         let item: SlateJson | undefined;
    //         if (p.scope !== undefined) {
    //           const views = await viewsFor(r);
    //           const list = piece.props?.["items"] !== undefined ? resolveSlateProp(piece.props["items"], contextOf(r, views)) : undefined;
    //           item = Array.isArray(list) ? list[p.scope.index] : (p.scope.item as SlateJson | undefined);
    //         }
    //         runs.release(p.threadId);
    //         const event: BatchEvent = { piece: p.piece, kind: p.event, ...(item !== undefined ? { item } : {}), ...(p.scope !== undefined ? { index: p.scope.index } : {}), ...(p.rowAction !== undefined ? { rowAction: p.rowAction } : {}) };
    //         const out = await batch(r, [], "person", { event, requestId: p.requestId, sendAt: now });
    //         return { r, out, started: new Set(out.started.filter(run => doc.runs[run]?.kind !== "cmd")) };
    //       });
    //       const answer = composed.then(async ({ r, out, started }): Promise<SlateEventAnswer> => {
    //         let landed: Awaited<ReturnType<typeof deliverAll>>;
    //         try {
    //           landed = await deliverAll(r, out.sends);
    //         } catch (e) {
    //           throw Object.assign(new Error(`This thread cannot take a message now: ${e instanceof Error ? e.message : String(e)}`), { kind: (e as { kind?: unknown }).kind ?? "conflict" });
    //         }
    //         const held = out.asks[0];
    //         // A tool run's sheet may wait on the server's tool list, or on finding the tool destructive.
    //         if (held === undefined && started.size > 0) await mcp.pending(r.threadId, 5_000);
    //         const ask = held === undefined ? asksOf(r).find(a => a.kind !== "cmd" && started.has(a.run)) : asksOf(r).find(a => a.run === held.run);
    //         if (landed !== undefined) return { outcome: landed.outcome, said: SAID[landed.kind][landed.outcome], ...(landed.turnId !== undefined ? { turnId: landed.turnId } : {}), ...(ask !== undefined ? { ask } : {}) };
    //         if (ask !== undefined) return { outcome: "held", said: "Needs your approval", ask };
    //         return { outcome: "done", said: "" };
    //       });
    //       requests.set(p.requestId, { at: now, answer });
    //       // A refused press is not remembered, so the person can press again once the reason is gone.
    //       answer.catch(() => requests.delete(p.requestId));
    //       return answer;
    //     },
    pub(crate) async fn event(self: Rc<Self>, p: EventParams) -> Result<SlateEventAnswer, HostError> {
        todo!()
    }

    // slates.ts:1346
    //     async approve(p) {
    //       const r = await needRecord(p.threadId);
    //       if (p.key === SLATE_SEND_KEY) {
    //         await serial(p.threadId, async () => {
    //           if (p.scope === "refuse") delete r.sendsAllowed;
    //           else r.sendsAllowed = deps.now();
    //           await save(r);
    //         });
    //         return;
    //       }
    //       if (p.key.startsWith(slateDomainKey(""))) {
    //         // A link's domain, allowed for the thread from the window's prompt; nothing runs, so once leaves nothing to keep.
    //         const domain = p.key.slice(slateDomainKey("").length);
    //         if (!/^[a-z0-9.-]+$/.test(domain)) throw usageRefusal(`${p.key} names no domain.`, "Approve a link as domain:example.com.");
    //         await serial(p.threadId, async () => {
    //           if (p.scope === "thread") r.approvals[p.key] = { state: "allowed", at: deps.now(), cmd: `links to ${domain}` };
    //           else if (p.scope === "refuse") delete r.approvals[p.key];
    //           await save(r);
    //         });
    //         return;
    //       }
    //       if (mcp.owns(p.threadId, p.key)) {
    //         await serial(p.threadId, async () => {
    //           if (p.scope === "refuse") {
    //             r.approvals[p.key] = { state: "refused", at: deps.now(), ...approvalNames(r, p.key) };
    //             await save(r);
    //             mcp.deny(p.threadId, p.key);
    //             return;
    //           }
    //           if (p.scope === "thread") r.approvals[p.key] = { state: "allowed", at: deps.now(), ...approvalNames(r, p.key) };
    //           viewsNow.set(p.threadId, await viewsFor(r));
    //           await save(r);
    //           mcp.approve(p.threadId, p.key, p.scope);
    //         });
    //         return;
    //       }
    //       const named = Object.entries(r.document?.runs ?? {}).filter(([, decl]) => decl.kind === "cmd" && runs.key(approvalDecl(r, decl) as CmdRunDecl) === p.key).map(([name]) => name);
    //       if (named.length === 0) throw usageRefusal(`this slate declares no command with approval key ${p.key}.`, "Read the slate again and approve what it asks now.");
    //       // The sheet offers no Always for these; a caller that asks for one anyway is told why.
    //       const unpinned = p.scope === "thread" ? named.map(run => alwaysRefused(run, pinFor(p.threadId, r.document!.runs[run]!))).find(said => said !== undefined) : undefined;
    //       if (unpinned !== undefined) throw usageRefusal(unpinned, "Approve it with scope once.");
    //       await serial(p.threadId, async () => {
    //         if (p.scope === "refuse") {
    //           r.approvals[p.key] = { state: "refused", at: deps.now(), ...approvalNames(r, p.key) };
    //           await save(r);
    //           for (const run of named) runs.deny(p.threadId, run);
    //           return;
    //         }
    //         if (p.scope === "thread") r.approvals[p.key] = { state: "allowed", at: deps.now(), ...approvalNames(r, p.key) };
    //         const views = await viewsFor(r);
    //         viewsNow.set(p.threadId, views);
    //         for (const run of named) {
    //           const rec = r.values[run];
    //           if (!isRunRecord(rec) || (rec.state !== "held" && rec.state !== "running")) continue;
    //           startedBy.set(byKey(p.threadId, run), "person");
    //           if (runs.approve(p.threadId, run, p.scope === "thread" ? "always" : "once") !== undefined || rec.state === "running") continue;
    //           // The hold was a host's before a restart, which this process never saw: hold it again, then answer it.
    //           const decl = r.document!.runs[run] as Extract<SlateRunDecl, { kind: "cmd" }>;
    //           const folder = folderFor(p.threadId, decl);
    //           if (folder === undefined) {
    //             moved(p.threadId, run, { state: "failed", why: NO_FOLDER, exit: null, runs: rec.runs, endedAt: deps.now() });
    //             continue;
    //           }
    //           const again = runs.start({ threadId: p.threadId, run, decl: approvalDecl(r, decl) as CmdRunDecl, by: "person", folder, inputs: inputsOf(r, decl), last: rec as unknown as RunRecord });
    //           if (again.outcome === "held") runs.approve(p.threadId, run, p.scope === "thread" ? "always" : "once");
    //         }
    //         // A timer a revoke took away comes back with the person's word.
    //         if (p.scope === "thread") armTimers(r);
    //         await save(r);
    //       });
    //     },
    pub(crate) async fn approve(self: Rc<Self>, p: ApproveParams) -> Result<(), HostError> {
        todo!()
    }

    // slates.ts:1418
    //     async revoke(p) {
    //       const r = await needRecord(p.threadId);
    //       await serial(p.threadId, async () => {
    //         if (p.key === SLATE_SEND_KEY) {
    //           if (r.sendsAllowed === undefined) throw usageRefusal("this thread's reactions were not allowed to message the agent.", "Read its approvals again.");
    //           delete r.sendsAllowed;
    //           await save(r);
    //           return;
    //         }
    //         if (r.approvals[p.key] === undefined) throw usageRefusal(`this thread holds no approval ${p.key}.`, "Read its approvals again.");
    //         delete r.approvals[p.key];
    //         // What it covered stops now: each run started under it is cancelled, out of the queue, and off its timer.
    //         const covered = Object.entries(r.document?.runs ?? {}).flatMap(([name, declared]) => {
    //           const decl = approvalDecl(r, declared);
    //           const covers = decl.kind === "cmd" ? runs.key(decl as CmdRunDecl) === p.key : p.key === codeKey(decl as McpRunDecl, mcpArgsOf(r, decl as McpRunDecl)());
    //           return covers ? [name] : [];
    //         });
    //         for (const run of covered) {
    //           runs.cancel(p.threadId, run);
    //           mcp.cancel(p.threadId, run);
    //         }
    //         armTimers(r, new Set(covered));
    //         await save(r);
    //       });
    //     },
    pub(crate) async fn revoke(self: Rc<Self>, thread_id: &str, key: &str) -> Result<(), HostError> {
        todo!()
    }

    // slates.ts:1444
    //     async cancel(p) {
    //       await needRecord(p.threadId);
    //       runs.cancel(p.threadId, p.run);
    //       mcp.cancel(p.threadId, p.run);
    //     },
    pub(crate) async fn cancel(self: Rc<Self>, thread_id: &str, run: &str) -> Result<(), HostError> {
        todo!()
    }

    // slates.ts:1490
    //     image: async p => {
    //       const r = await needRecord(p.threadId);
    //       return slateImage(p.src, { thread: p.threadId, have: p.have, folder: deps.thread(p.threadId)?.folder, on: deps.imageOn?.(p.threadId), allowed: domain => r.approvals[slateDomainKey(domain)]?.state === "allowed" });
    //     },
    pub(crate) async fn image(self: Rc<Self>, thread_id: &str, src: &str, have: Option<&str>) -> Result<SlatesImageAnswer, HostError> {
        todo!()
    }
}
