// SPDX-License-Identifier: AGPL-3.0-only
//! Unit slates_reads: slates.ts:549-685, 1233-1279, 1482-1488 at 65f6b89c7. Spec: spec/slates_reads.md.
use std::rc::Rc;

use indexmap::IndexMap;
use serde_json::Value;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

use super::model::SlateRecord;
use super::{CatalogParams, ReadParams, SlatesInner, Views};
use crate::pending::siblings::Pin;
use crate::pending::slate::{SlateDoc, SlateEvalContext, SlateProblem};
use crate::pending::wire::{SlateAsk, SlateReadAnswer, SlateView};

// slates.ts:591
//   /** The paths a document binds, as the evaluator names them. */
//   const boundPaths = (doc: SlateDoc): string[] => {
//     const found = new Set<string>();
//     const visit = (value: unknown): void => {
//       if (Array.isArray(value)) value.forEach(visit);
//       else if (typeof value === "object" && value !== null) {
//         const o = value as Record<string, unknown>;
//         if (isSlateBinding(o)) for (const d of slateDependencies(o.bind)) found.add(d);
//         else if (typeof o["format"] === "string" && Object.keys(o).length === 1) for (const m of o["format"].matchAll(/\$\{([^}]*)\}/g)) for (const d of slateDependencies(m[1]!)) found.add(d);
//         else Object.values(o).forEach(visit);
//       }
//     };
//     for (const piece of Object.values(doc.pieces)) {
//       visit(piece.props);
//       if (piece.when !== undefined) for (const d of slateDependencies(piece.when)) found.add(d);
//     }
//     for (const expr of Object.values(doc.derived)) for (const d of slateDependencies(expr)) found.add(d);
//     return [...found].filter(p => !p.startsWith("item") && !p.startsWith("index"));
//   };
pub fn bound_paths(doc: &SlateDoc) -> Vec<String> {
    todo!()
}

// slates.ts:622
//   /** The runs held for the person's approval, by name: what the slate waits on, never a fault of it. */
//   const waitingOf = (r: SlateRecord): string[] =>
//     Object.keys(r.document?.runs ?? {}).filter(name => {
//       const rec = r.values[name];
//       return isRunRecord(rec) && rec.state === "held" && (rec.why === undefined || rec.why === HELD_APPROVAL);
//     }).map(name => `$${name}`);
pub fn waiting_of(r: &SlateRecord) -> Vec<String> {
    todo!()
}

impl SlatesInner {
    // slates.ts:548
    //   /** The sources a document and a set of extra paths read, viewed once. */
    //   const viewsFor = async (r: SlateRecord, extra: readonly string[] = []) => {
    //     const text = `${r.document === null ? "" : JSON.stringify(r.document)} ${extra.join(" ")}`;
    //     // The clock always: ago() and until() read it without naming it.
    //     return viewSources(["time", ...sourcesNamed(text)], deps.sources(r.threadId, r.workspaceId));
    //   };
    pub(crate) async fn views_for(self: Rc<Self>, r: &SlateRecord, extra: &[String]) -> Views {
        todo!()
    }

    // slates.ts:555
    //   /** How the host reads a path: own values and derived values off the record, sources off the views, the row scope. */
    //   const contextOf = (r: SlateRecord, views: ReadonlyMap<string, SlateJson | undefined>, row?: { item?: SlateJson; index?: number }): SlateEvalContext => {
    //     const derived = new Map<string, SlateJson | undefined>();
    //     const busy = new Set<string>();
    //     const ctx: SlateEvalContext = {
    //       now: deps.now(),
    //       ...(row?.item !== undefined ? { item: row.item } : {}),
    //       ...(row?.index !== undefined ? { index: row.index } : {}),
    //       resolve: path => {
    //         if (!path.startsWith("$")) return resolveIn(views, path);
    //         const own = parseSlateOwnPath(path);
    //         if (own === undefined) return undefined;
    //         const expr = r.document?.derived[own.name];
    //         if (expr === undefined) return getSlateValue(r.values, path);
    //         if (!derived.has(own.name)) {
    //           if (busy.has(own.name)) return undefined;
    //           busy.add(own.name);
    //           derived.set(own.name, evaluateSlateExpression(expr, ctx));
    //           busy.delete(own.name);
    //         }
    //         let at = derived.get(own.name);
    //         for (const seg of own.segs) at = slateStep(at, seg);
    //         return at;
    //       },
    //     };
    //     return ctx;
    //   };
    /// `row` is the event's item and index.
    pub(crate) fn context_of(&self, r: &SlateRecord, views: &Views, row: Option<(Option<Value>, Option<i64>)>) -> SlateEvalContext {
        todo!()
    }

    // slates.ts:583
    //   const scrub = (r: SlateRecord, text: string): string => runs.secrets.scrub(r.threadId, text);
    //
    //   const sketched = (r: SlateRecord, views: ReadonlyMap<string, SlateJson | undefined>, check = false, asked: SlateProblem[] = [], warnings: SlateProblem[] = []): string => {
    //     const text = sketchSlate(r.document, r.values, { ...contextOf(r, views), version: r.version, warnings, ...(check ? { check: true } : { problems: [...asked, ...problemsOf(r, views)], waiting: waitingOf(r) }) });
    //     return scrub(r, /^slate v\d/.test(text) ? text : text.replace(/^slate\b/, `slate v${r.version}`));
    //   };
    //   const sketchOf = async (r: SlateRecord): Promise<string> => sketched(r, await viewsFor(r));
    pub(crate) fn scrub(&self, thread_id: &str, text: &str) -> String {
        todo!()
    }
    pub(crate) fn sketched(&self, r: &SlateRecord, views: &Views, check: bool, asked: &[SlateProblem], warnings: &[SlateProblem]) -> String {
        todo!()
    }
    pub(crate) async fn sketch_of(self: Rc<Self>, r: &SlateRecord) -> String {
        todo!()
    }

    // slates.ts:611
    //   /** What the agent is owed on a read: the standing runtime problems, data that has not arrived, held runs, and a
    //    * slate a rewind emptied. */
    //   const problemsOf = (r: SlateRecord, views: ReadonlyMap<string, SlateJson | undefined>): SlateProblem[] => {
    //     const found: SlateProblem[] = [...r.problems];
    //     if (r.document === null && r.empty === "rewound-before") found.push(problem("Z804", "rewound-before", "the thread was rewound to before this slate existed; write one with slate_write"));
    //     if (r.document !== null) {
    //       for (const path of boundPaths(r.document)) if (!path.startsWith("$") && resolveIn(views, path) === undefined) found.push(problem("R900", "data-missing", `${path} has no value yet`));
    //     }
    //     return found;
    //   };
    pub(crate) fn problems_of(&self, r: &SlateRecord, views: &Views) -> Vec<SlateProblem> {
        todo!()
    }

    // slates.ts:629
    //   /** Which files a run names changed since the person said "Always" to it, when that is why it asks again, from the
    //    * pin its ask read. */
    //   const changedSince = (r: SlateRecord, run: string, key: string, pin: Pin): string | undefined => {
    //     const now = keyed(pin) ?? {};
    //     const unread = pin.unpinned.flatMap(u => (u.why === "unread" ? [u.path] : []));
    //     for (const [k, a] of Object.entries(r.approvals)) {
    //       if (k === key || a.state !== "allowed" || a.run !== run) continue;
    //       if (unread.length > 0) return `${unread.join(", ")} could not be read on ${deps.thread(r.threadId)?.computer ?? "this computer"}, so it asks again`;
    //       const changed = a.scripts === undefined ? [] : Object.keys({ ...a.scripts, ...now }).filter(path => a.scripts![path] !== now[path]);
    //       if (changed.length > 0) return `${changed.join(", ")} changed since you allowed it, so it asks again`;
    //     }
    //     return undefined;
    //   };
    pub(crate) fn changed_since(&self, r: &SlateRecord, run: &str, key: &str, pin: &Pin) -> Option<String> {
        todo!()
    }

    // slates.ts:643
    //   const asksOf = (r: SlateRecord): SlateAsk[] => {
    //     const facts = deps.thread(r.threadId);
    //     const cmds = runs.held(r.threadId).map((a: RunAsk): SlateAsk => {
    //       const rec = r.values[a.run];
    //       const pin = r.document?.runs[a.run] === undefined ? undefined : pinFor(r.threadId, r.document.runs[a.run]!);
    //       return {
    //         key: a.key,
    //         run: a.run,
    //         kind: "cmd" as const,
    //         cmd: a.cmd,
    //         env: a.env,
    //         args: a.args,
    //         ...(a.stdin !== undefined ? { stdin: a.stdin } : {}),
    //         computer: facts?.computer ?? "this computer",
    //         folder: a.folder,
    //         timeoutS: a.timeout,
    //         ...(a.confirm !== undefined ? { confirm: a.confirm } : {}),
    //         ...(a.then !== undefined ? { then: a.then } : {}),
    //         ...(a.files !== undefined ? { files: a.files } : {}),
    //         ...(pin !== undefined && pin.unpinned.length > 0 ? { noAlways: true as const } : {}),
    //         why: (pin === undefined ? undefined : (cannotPin(pin) ?? changedSince(r, a.run, a.key, pin))) ?? (isRunRecord(rec) && rec.why !== undefined ? rec.why : HELD_APPROVAL),
    //       };
    //     });
    //     return [...cmds, ...mcp.held(r.threadId)];
    //   };
    pub(crate) fn asks_of(&self, r: &SlateRecord) -> Vec<SlateAsk> {
        todo!()
    }

    // slates.ts:669
    //   const viewOf = (r: SlateRecord): SlateView => ({
    //     threadId: r.threadId,
    //     workspaceId: r.workspaceId,
    //     version: r.version,
    //     revision: r.revision,
    //     document: r.document as unknown as Record<string, unknown> | null,
    //     values: r.values,
    //     ...(r.document === null ? { empty: r.empty ?? "none" } : {}),
    //     comments: r.comments,
    //     approvals: { ...r.approvals, ...(r.sendsAllowed !== undefined ? { [SLATE_SEND_KEY]: { state: "allowed" as const, at: r.sendsAllowed } } : {}) },
    //     asks: asksOf(r),
    //     problems: r.problems,
    //     shownOnce: r.shownOnce,
    //     canUndo: r.previous !== undefined,
    //     updatedAt: r.updatedAt,
    //   });
    pub(crate) fn view_of(&self, r: &SlateRecord) -> SlateView {
        todo!()
    }

    // slates.ts:1233
    //     async read(p, caller) {
    //       const threadId = targetOf(p, caller, false);
    //       // A thread with no slate yet reads as an empty one, which is what it has; a refusal read as something broken.
    //       const r = (await recordOf(threadId)) ?? freshRecord(threadId);
    //       const asked = p.values ?? [];
    //       const paths = asked.includes("*") && r.document !== null ? boundPaths(r.document) : asked.filter(v => v !== "*");
    //       const views = await viewsFor(r, paths);
    //       const ctx = contextOf(r, views);
    //       const doc = r.document;
    //       const clean = (v: SlateJson): SlateJson => mapStrings(v, s => scrub(r, s));
    //       const misreads = paths.flatMap(path => misread(doc, path));
    //       const sketch = p.sketch !== false ? sketched(r, views, false, misreads) : `slate v${r.version}`;
    //       // The paths asked for go in the text too, since an agent may read the text alone.
    //       const asks = paths.map(path => { const said = JSON.stringify(clean(evaluateSlateExpression(path, ctx) ?? null)); return `  ${path} = ${said.length > READ_OUTPUT_CHARS ? `${said.slice(0, READ_OUTPUT_CHARS)}... (${said.length} characters)` : said}`; });
    //       return {
    //         version: r.version,
    //         text: [sketch, ...(asks.length > 0 ? ["read:", ...asks] : []), ...(p.text !== false && doc !== null ? ["", printSlate(doc)] : [])].join("\n"),
    //         ...(p.document === true ? { document: doc as unknown as Record<string, unknown> | null } : {}),
    //         values: Object.fromEntries(paths.map(path => [path, clean(evaluateSlateExpression(path, ctx) ?? null)])),
    //         state: Object.fromEntries(Object.entries(r.values).filter(([name]) => doc?.runs[name] === undefined).map(([name, v]) => [`$${name}`, clean(v)])),
    //         derived: Object.fromEntries(Object.keys(doc?.derived ?? {}).map(name => [`$${name}`, clean(ctx.resolve(`$${name}`) ?? null)])),
    //         runs: Object.fromEntries(Object.keys(doc?.runs ?? {}).map(name => [`$${name}`, clean(runShown(r.values[name], name, paths.length > 0))])),
    //         problems: [...misreads, ...problemsOf(r, views)],
    //         waiting: waitingOf(r),
    //         comments: r.comments,
    //         // By the run each approval is for, which is what the agent writes; the key only where no run declares it now.
    //         approvals: Object.fromEntries(Object.entries(r.approvals).map(([k, a]) => { const run = approvalNames(r, k).run; return [run !== undefined ? `$${run}` : k, a.state]; })),
    //       };
    //     },
    pub(crate) async fn read(self: Rc<Self>, p: ReadParams, caller: Option<&Caller>) -> Result<SlateReadAnswer, HostError> {
        todo!()
    }

    // slates.ts:1263
    //     async catalog(p, caller) {
    //       refuseSubThread(caller);
    //       const text = slateCatalog(p.name);
    //       const name = p.name?.trim();
    //       if (name === undefined || name === "" || !text.startsWith(`${name} is not in the catalog`)) return { text };
    //       let threadId: string;
    //       try {
    //         threadId = targetOf(p, caller, false);
    //       } catch {
    //         return { text };
    //       }
    //       try {
    //         return { text: await mcp.catalog(threadId, name) };
    //       } catch (e) {
    //         return { text: `${text}\n${name} as an MCP server of this thread's agent: ${e instanceof Error ? e.message : String(e)}` };
    //       }
    //     },
    pub(crate) async fn catalog(self: Rc<Self>, p: CatalogParams, caller: Option<&Caller>) -> Result<String, HostError> {
        todo!()
    }

    // slates.ts:1482
    //     async resolve(p) {
    //       const r = await recordOf(p.threadId);
    //       if (r === undefined) return { values: Object.fromEntries(p.paths.map(path => [path, null])) };
    //       const views = await viewsFor(r, p.paths);
    //       const ctx = contextOf(r, views);
    //       return { values: Object.fromEntries(p.paths.map(path => [path, mapStrings(evaluateSlateExpression(path, ctx) ?? null, s => scrub(r, s))])) };
    //     },
    pub(crate) async fn resolve(self: Rc<Self>, thread_id: &str, paths: &[String]) -> Result<IndexMap<String, Value>, HostError> {
        todo!()
    }
}
