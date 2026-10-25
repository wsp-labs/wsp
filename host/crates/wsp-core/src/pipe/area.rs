// SPDX-License-Identifier: AGPL-3.0-only
//! The pool as the runtime runs it: a brokered workspace's pipes stand while it runs, by the bus's workspace events,
//! and `cut_off`, which revokes a machine at the broker before its delete. Port of packages/runtime/src/pipe.ts:394-466
//! at 65f6b89c7. Spec: spec/pipe_area.md.
//!
//! Host state on the owner thread (rule 3.1), as the pool.
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use wsp_wire::wire::events_union::EventUnion;

use super::pool::PipeHandler;
use crate::clock::Clock;

/// The broker the host wired, as the pool reads it (`ctx.opts.broker`).
pub trait PipeBroker {
    fn ready(&self) -> bool;
    /// The broker's pipe handler, `None` where it serves no pipes.
    fn pipe(&self) -> Option<PipeHandler>;
    /// `None` where the broker has no revoke (the TS `revoke?`); `Err` is its failure's words.
    fn revoke(&self, workspace_id: &str) -> Option<LocalBoxFuture<'static, Result<(), String>>>;
}

/// One live workspace as the area reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipeLive {
    pub id: String,
    /// The record's phase; only `running` is dialled.
    pub phase: String,
    pub machine_id: String,
}

/// What `pipe_area` and `cut_off` read off the runtime's context, method for method.
pub trait PipeAreaContext {
    fn broker(&self) -> Option<Rc<dyn PipeBroker>>;
    /// `ctx.live.get(id)`.
    fn live(&self, workspace_id: &str) -> Option<PipeLive>;
    /// `ctx.live.keys()`, in insertion order.
    fn live_ids(&self) -> Vec<String>;
    /// `entry.ws.daemonReach()`'s url; `Err` is its failure.
    fn daemon_reach_url(&self, workspace_id: &str) -> LocalBoxFuture<'static, Result<String, String>>;
    /// `ctx.daemonTokenOf(entry.machine)`.
    fn daemon_token_of(&self, workspace_id: &str) -> LocalBoxFuture<'static, Result<Option<String>, String>>;
    /// `ctx.moduleOf(kind).daemonVersion(entry)`.
    fn daemon_version(&self, workspace_id: &str) -> LocalBoxFuture<'static, Result<Option<i64>, String>>;
    /// `ctx.daemonTokens.delete(machineId)`.
    fn forget_daemon_token(&self, machine_id: &str);
    fn clock(&self) -> Rc<dyn Clock>;
    /// `ctx.bus.on("*", listener)`; answers the detach.
    fn on_any(&self, listener: Rc<dyn Fn(&EventUnion)>) -> Box<dyn FnOnce()>;
    /// `ctx.ready()`.
    fn ready(&self) -> LocalBoxFuture<'static, Result<(), String>>;
}

/// The running area; `close` detaches from the bus, then closes the pool.
pub struct PipeArea {
    state: std::cell::RefCell<()>,
}

impl PipeArea {
    pub fn close(&self) {
        todo!()
    }
}

/// `None` and nothing dialled where the host wired no broker. `brokered` is the TS `brokered(record)`, asked by
/// workspace id: the wiring reads the live record of that id.
// TS pipe.ts:400-466
// /** The pool as the runtime runs it: a brokered workspace's pipes stand while it runs, by the events the host's held
//  * link to it follows, and are dialled again on a new machine. Nothing here where the host wired no broker. */
// export function pipeArea(ctx: RuntimeContext, brokered: (record: WorkspaceRecord) => boolean): { close(): void } | undefined {
//   const broker = ctx.opts.broker;
//   if (broker === undefined) return undefined;
//   const pool = pipePool({
//     reach: async id => {
//       const entry = ctx.live.get(id);
//       if (entry === undefined) return undefined;
//       const route = await entry.ws.daemonReach();
//       const token = await ctx.daemonTokenOf(entry.machine);
//       return token === undefined ? undefined : { url: route.url, token };
//     },
//     version: async id => {
//       const entry = ctx.live.get(id);
//       return entry === undefined ? null : ctx.moduleOf(entry.record.kind).daemonVersion(entry);
//     },
//     handler: () => (broker.pipe === undefined ? undefined : broker.pipe.bind(broker)),
//     ready: () => broker.ready(),
//     rewrite: id => {
//       const entry = ctx.live.get(id);
//       if (entry !== undefined) ctx.daemonTokens.delete(entry.machine.id);
//     },
//     clock: ctx.clock,
//   });
//   const upIf = (id: string): void => {
//     const entry = ctx.live.get(id);
//     if (entry !== undefined && entry.record.phase === "running" && brokered(entry.record)) pool.up(id);
//   };
//   const detach = ctx.bus.on("*", e => {
//     switch (e.type) {
//       // A create's own clone reaches GitHub through these pipes, so they are dialled from its first stage after the
//       // machine is bound; one that fails takes them down with it.
//       case "workspace.creating":
//         if (e.stage === "failed") pool.down(e.workspaceId);
//         else upIf(e.workspaceId);
//         return;
//       case "workspace.created":
//         if (e.workspace.phase === "running") upIf(e.workspace.id);
//         return;
//       case "workspace.woken":
//       case "workspace.upgraded":
//         pool.down(e.workspaceId);
//         upIf(e.workspaceId);
//         return;
//       case "workspace.napped":
//       case "workspace.gone":
//       case "workspace.deleted":
//         pool.down(e.workspaceId);
//         return;
//       default:
//         return;
//     }
//   });
//   void ctx.ready().then(
//     () => {
//       for (const id of ctx.live.keys()) upIf(id);
//     },
//     () => {},
//   );
//   return {
//     close() {
//       detach();
//       pool.close();
//     },
//   };
// }
pub fn pipe_area(ctx: Rc<dyn PipeAreaContext>, brokered: Rc<dyn Fn(&str) -> bool>) -> Option<Rc<PipeArea>> {
    todo!()
}

/// Cuts a machine off at the broker before its provider deletes it; a broker that fails says so through
/// `crate::console::warn` and the delete goes on.
// TS pipe.ts:394-398
// /** Cuts a machine off at the host's broker before its provider is asked to delete it, so no call from it lands in
//  * between, nor after a delete the provider refuses. A broker that fails says so, and the delete goes on. */
// export async function cutOff(ctx: Pick<RuntimeContext, "opts">, entry: LiveWorkspace): Promise<void> {
//   await ctx.opts.broker?.revoke?.(entry.record.id).catch((e: unknown) => console.warn(`${entry.record.name} was not cut off from your computer before its delete: ${e instanceof Error ? e.message : String(e)}`));
// }
pub async fn cut_off(ctx: &dyn PipeAreaContext, workspace_id: &str, name: &str) {
    todo!()
}
