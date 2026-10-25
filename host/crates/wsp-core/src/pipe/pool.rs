// SPDX-License-Identifier: AGPL-3.0-only
//! The host's pool of pipes to each brokered machine's daemon: extra websockets the daemon parks until a process on
//! the machine connects to one of its doors, each then carrying that one connection's bytes. Port of
//! packages/runtime/src/pipe.ts:1-392 at 65f6b89c7. Spec: spec/pipe_pool.md.
//!
//! Host state on the owner thread (rule 3.1): `Rc`, `RefCell` and `spawn_local`, and no borrow held across an
//! `.await` or a call out (rule 3.3).
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use tokio::io::{AsyncRead, AsyncWrite};
pub use wsp_wire::wire::daemon::{PipeDoor, PipeOpen};

use crate::clock::Clock;

/// Idle pipes kept dialled per brokered workspace whose link is up.
pub const PIPE_IDLE: usize = 4;
/// A pipe idle this long is replaced.
pub const PIPE_IDLE_MAX_MS: u64 = 10 * 60_000;
/// The bound on one dial.
pub const PIPE_DIAL_MS: u64 = 15_000;
/// How long before a daemon too old for pipes is asked its version again.
pub const PIPE_BEHIND_AGAIN_MS: u64 = 60_000;
/// The most pipes dialled to one workspace in any second.
pub const PIPE_DIAL_RATE: usize = 4;
/// The longest wait between two writes of the host's token to a machine whose daemon keeps refusing it.
pub const TOKEN_REFUSED_MAX_MS: u64 = 30 * 60_000;

/// One road's count of a daemon refusing the host's token.
// TS pipe.ts:32-48
// /** One road's count of a daemon refusing the host's token. The host writes its own token there again and dials after
//  * PIPE_BEHIND_AGAIN_MS, and twice as long for each refusal that follows its own write: two writers of one file cut each
//  * other's sockets, and backing off is what keeps that from happening every minute. */
// export function tokenRefusals(firstMs = PIPE_BEHIND_AGAIN_MS): { refused(now: number): { count: number; waitMs: number }; wrote(now: number): void } {
//   let count = 0;
//   let wroteAt: number | undefined;
//   return {
//     refused(now) {
//       count = wroteAt !== undefined && now - wroteAt < 2 * TOKEN_REFUSED_MAX_MS ? count + 1 : 1;
//       wroteAt = undefined;
//       return { count, waitMs: Math.min(firstMs * 2 ** (count - 1), TOKEN_REFUSED_MAX_MS) };
//     },
//     wrote(now) {
//       wroteAt = now;
//     },
//   };
// }
#[derive(Clone, Debug, PartialEq)]
pub struct TokenRefusals {
    first_ms: u64,
    count: u32,
    wrote_at: Option<i64>,
}

impl TokenRefusals {
    /// Answers the refusals in a row and the wait before the next dial.
    pub fn refused(&mut self, now: i64) -> (u32, u64) {
        todo!()
    }
    pub fn wrote(&mut self, now: i64) {
        todo!()
    }
}

/// `first_ms` is `PIPE_BEHIND_AGAIN_MS` at every TS call site but the pool's own, which passes `behind_again_ms`.
pub fn token_refusals(first_ms: u64) -> TokenRefusals {
    todo!()
}

/// `<n> s` under two minutes, else `<n> min`, each rounded as `Math.round` does.
// TS pipe.ts:50-50
// const waitWords = (ms: number): string => (ms < 120_000 ? `${Math.round(ms / 1000)} s` : `${Math.round(ms / 60_000)} min`);
pub fn wait_words(ms: u64) -> String {
    todo!()
}

/// What a road says the count'th time in a row a daemon refuses the host's token; `None` past the second.
// TS pipe.ts:52-59
// /** What a road says the count'th time in a row a daemon refuses the host's token: the first refusal and the first one
//  * after the host's own write, and nothing after that until a refusal stops following a write. */
// export function tokenRefusedLine(who: string, road: string, count: number, waitMs: number): string | undefined {
//   if (count === 1) return `${who}: its daemon refused the token ${road}, so the host writes its own there again and dials in ${waitWords(waitMs)}`;
//   if (count === 2)
//     return `${who}: its daemon refused the host's token again right after the host wrote it, so something else writes that file too; the host writes its own there again and dials in ${waitWords(waitMs)}, waiting twice as long each time up to ${waitWords(TOKEN_REFUSED_MAX_MS)}`;
//   return undefined;
// }
pub fn token_refused_line(who: &str, road: &str, count: u32, wait_ms: u64) -> Option<String> {
    todo!()
}

/// A taken pipe's bytes, both ways, over the socket's binary frames.
pub trait PipeIo: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin> PipeIo for T {}
pub type PipeDuplex = Box<dyn PipeIo>;

/// Where each taken pipe goes: the workspace whose link it was dialled on. An `Err` is the TS handler's throw.
// TS pipe.ts:61-62
// /** Where each taken pipe goes: the workspace whose link it was dialled on, never anything the machine said. */
// export type PipeHandler = (workspaceId: string, open: PipeOpen, duplex: Duplex) => void;
pub type PipeHandler = Rc<dyn Fn(&str, PipeOpen, PipeDuplex) -> Result<(), String>>;

// TS pipe.ts:64-69
// export interface PipeReach {
//   /** The daemon's route, as the link dials it. */
//   url: string;
//   /** The daemon's own token, sent in the first frame and never in the url. */
//   token: string;
// }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipeReach {
    /// The daemon's route, as the link dials it.
    pub url: String,
    /// The daemon's own token, sent in the first frame and never in the url.
    pub token: String,
}

// TS pipe.ts:71-89
// export interface PipePoolOptions {
//   /** The link's own reach, read at each dial; undefined while the guest holds no daemon token. */
//   reach(workspaceId: string): Promise<PipeReach | undefined>;
//   /** The version the workspace's daemon announces, null when it does not answer: pipes are dialled only to one at or
//    * above PIPE_DAEMON_VERSION. */
//   version(workspaceId: string): Promise<number | null>;
//   /** Read at each take, so a handler wired after the pool started still gets every pipe. */
//   handler(): PipeHandler | undefined;
//   /** The switch: nothing is dialled while it reads false. */
//   ready(): boolean;
//   /** The daemon refused the token: the next reach writes the host's token on the machine again. */
//   rewrite(workspaceId: string): void;
//   clock?: Clock;
//   backoffMs?: (attempt: number) => number;
//   idleMaxMs?: number;
//   behindAgainMs?: number;
//   dialRate?: number;
//   log?: (line: string) => void;
// }
#[derive(Clone)]
pub struct PipePoolOptions {
    /// The link's own reach, read at each dial; `Ok(None)` while the guest holds no daemon token; `Err` fails the dial.
    pub reach: Rc<dyn Fn(&str) -> LocalBoxFuture<'static, Result<Option<PipeReach>, String>>>,
    /// The daemon's version, `Ok(None)` when it does not answer; an `Err` reads as `None`.
    pub version: Rc<dyn Fn(&str) -> LocalBoxFuture<'static, Result<Option<i64>, String>>>,
    /// Read at each take.
    pub handler: Rc<dyn Fn() -> Option<PipeHandler>>,
    /// The switch: nothing is dialled while it reads false.
    pub ready: Rc<dyn Fn() -> bool>,
    /// The daemon refused the token: the next reach writes the host's token on the machine again.
    pub rewrite: Rc<dyn Fn(&str)>,
    /// `real_clock()` when `None`.
    pub clock: Option<Rc<dyn Clock>>,
    /// `link_backoff_ms` with a real random draw when `None`.
    pub backoff_ms: Option<Rc<dyn Fn(u32) -> u64>>,
    pub idle_max_ms: Option<u64>,
    pub behind_again_ms: Option<u64>,
    pub dial_rate: Option<usize>,
    /// `crate::console::warn` when `None`.
    pub log: Option<Rc<dyn Fn(&str)>>,
}

// TS pipe.ts:91-99
// export interface PipePool {
//   /** The workspace's link is up: keep PIPE_IDLE pipes dialled to it. */
//   up(workspaceId: string): void;
//   /** The link stopped: every pipe of the workspace closes, the taken ones too, and nothing is dialled again until up. */
//   down(workspaceId: string): void;
//   /** Pipes open and waiting for the daemon to take one. */
//   idle(workspaceId: string): number;
//   close(): void;
// }
pub struct PipePool {
    state: std::cell::RefCell<()>,
}

impl PipePool {
    /// The workspace's link is up: keep `PIPE_IDLE` pipes dialled to it.
    pub fn up(&self, workspace_id: &str) {
        todo!()
    }
    /// The link stopped: every pipe of the workspace closes, taken ones too, and nothing is dialled until `up`.
    pub fn down(&self, workspace_id: &str) {
        todo!()
    }
    /// Pipes open and waiting for the daemon to take one.
    pub fn idle(&self, workspace_id: &str) -> usize {
        todo!()
    }
    pub fn close(&self) {
        todo!()
    }
}

// TS pipe.ts:146-392
// export function pipePool(o: PipePoolOptions): PipePool {
//   const clock = o.clock ?? realClock;
//   const backoff = o.backoffMs ?? linkBackoffMs;
//   const idleMaxMs = o.idleMaxMs ?? PIPE_IDLE_MAX_MS;
//   const behindAgainMs = o.behindAgainMs ?? PIPE_BEHIND_AGAIN_MS;
//   const dialRate = o.dialRate ?? PIPE_DIAL_RATE;
//   const log = o.log ?? ((line: string) => console.warn(line));
//   const held = new Map<string, Held>();
//
//   const end = (pipe: Pipe): void => {
//     pipe.done = true;
//     pipe.cancelIdle?.();
//     pipe.ws?.terminate();
//   };
//   const endIdle = (h: Held): void => {
//     for (const pipe of h.idle) end(pipe);
//     h.idle.clear();
//   };
//
//   const again = (h: Held, ms: number): void => {
//     if (h.retry !== undefined) return;
//     h.retry = clock.schedule(
//       () => {
//         h.retry = undefined;
//         fill(h);
//       },
//       ms,
//       { unref: true },
//     );
//   };
//   const wait = (h: Held): void => again(h, backoff(++h.attempt));
//
//   /** Dials up to PIPE_IDLE idle pipes, with every socket of the workspace, taken or idle, inside PIPE_POOL_MAX: past
//    * it the daemon holds the next connection until a taken pipe closes. */
//   const fill = (h: Held): void => {
//     if (h.stopped || h.asking || h.retry !== undefined || h.pace !== undefined) return;
//     if (!o.ready()) return wait(h);
//     if (!h.serves) return ask(h);
//     if (h.taken.size >= PIPE_POOL_MAX && !h.saidFull) {
//       h.saidFull = true;
//       log(`${h.id}: ${PIPE_POOL_MAX} pipes are in use, so the next connection waits for one to close`);
//     }
//     const short = Math.min(PIPE_IDLE - h.idle.size, PIPE_POOL_MAX - h.taken.size - h.idle.size);
//     if (short <= 0) return;
//     const now = clock.now();
//     h.dialled = h.dialled.filter(at => now - at < 1000);
//     const allowed = Math.min(short, dialRate - h.dialled.length);
//     if (!h.saidDialling) log(`${h.id}: dialling ${short} pipe${short === 1 ? "" : "s"}, ${PIPE_IDLE} kept idle`);
//     h.saidDialling = true;
//     for (let i = 0; i < allowed; i++) {
//       h.dialled.push(now);
//       dial(h);
//     }
//     if (allowed < short) {
//       h.pace = clock.schedule(
//         () => {
//           h.pace = undefined;
//           fill(h);
//         },
//         h.dialled[0]! + 1000 - now,
//         { unref: true },
//       );
//     }
//   };
//
//   /** A daemon below PIPE_DAEMON_VERSION reads `pipe` as nothing and answers the socket as a link, so it is asked its
//    * version first and dialled only once it is new enough. */
//   const ask = (h: Held): void => {
//     h.asking = true;
//     void o.version(h.id).then(
//       v => answered(h, v),
//       () => answered(h, null),
//     );
//   };
//   const answered = (h: Held, version: number | null): void => {
//     h.asking = false;
//     if (h.stopped) return;
//     if (version === null) return wait(h);
//     if (version >= PIPE_DAEMON_VERSION) {
//       h.serves = true;
//       return fill(h);
//     }
//     behind(h, `${h.id}: its daemon (${version}) serves no pipes yet, so it is asked again each minute`);
//   };
//   const behind = (h: Held, line: string): void => {
//     if (!h.saidBehind) log(line);
//     h.saidBehind = true;
//     h.serves = false;
//     endIdle(h);
//     again(h, behindAgainMs);
//   };
//
//   /** One pipe that could not be dialled, or closed while it waited: the link is down, so the rest go too, since an idle
//    * pipe sends nothing and would learn it only when the daemon took it, and the pool is dialled again on the link's
//    * own backoff. */
//   const linkDown = (h: Held): void => {
//     if (h.attempt === 0) log(`${h.id}: a pipe could not be dialled, so its others were closed and the pool is dialled again as the link comes back`);
//     endIdle(h);
//     if (!h.stopped) wait(h);
//   };
//
//   /** The pipes of one dial go together: the rest are ended here, so their own refusals are nobody's news. */
//   const refused = (h: Held): void => {
//     endIdle(h);
//     const { count, waitMs } = h.refusals.refused(clock.now());
//     const line = tokenRefusedLine(h.id, "a pipe was dialled with", count, waitMs);
//     if (line !== undefined) log(line);
//     o.rewrite(h.id);
//     h.retry?.();
//     h.retry = clock.schedule(
//       () => {
//         h.retry = undefined;
//         h.refusals.wrote(clock.now());
//         fill(h);
//       },
//       waitMs,
//       { unref: true },
//     );
//   };
//
//   const retire = (h: Held, pipe: Pipe): void => {
//     if (pipe.done) return;
//     h.idle.delete(pipe);
//     pipe.done = true;
//     pipe.ws?.close(1000);
//     fill(h);
//   };
//
//   const take = (h: Held, pipe: Pipe, ws: WebSocket, open: PipeOpen): void => {
//     h.idle.delete(pipe);
//     pipe.done = true;
//     pipe.cancelIdle?.();
//     h.attempt = 0;
//     h.saidBehind = false;
//     h.taken.add(ws);
//     ws.once("close", () => {
//       h.taken.delete(ws);
//       if (h.taken.size < PIPE_POOL_MAX) h.saidFull = false;
//       fill(h);
//     });
//     // Wrapped before anything else runs, so the binary frames right behind pipe.open reach the stream.
//     const duplex = createWebSocketStream(ws);
//     duplex.on("error", () => {});
//     const handler = o.handler();
//     const room = h.taken.size + h.idle.size < PIPE_POOL_MAX;
//     log(`${h.id}: a pipe opened on the ${open.door} door${open.heldMs > 0 ? ` after ${open.heldMs} ms held` : ""}, ${room ? "dialling its replacement" : "its replacement waits for a pipe to close"}`);
//     if (handler === undefined) {
//       log(`${h.id}: nothing here serves pipes yet, so that connection was closed`);
//       duplex.destroy();
//     } else {
//       try {
//         handler(h.id, open, duplex);
//       } catch (e) {
//         log(`${h.id}: the pipe's handler failed (${e instanceof Error ? e.message : String(e)})`);
//         duplex.destroy();
//       }
//     }
//     fill(h);
//   };
//
//   const dial = (h: Held): void => {
//     const pipe: Pipe = { open: false, authed: false, done: false };
//     h.idle.add(pipe);
//     const failed = (): void => {
//       if (pipe.done) return;
//       end(pipe);
//       h.idle.delete(pipe);
//       if (!h.stopped) linkDown(h);
//     };
//     let timer: (() => void) | undefined = clock.schedule(failed, PIPE_DIAL_MS, { unref: true });
//     void o.reach(h.id).then(reach => {
//       if (pipe.done) return;
//       if (reach === undefined) return failed();
//       const ws = new WebSocket(daemonWsUrl(reach.url), { handshakeTimeout: PIPE_DIAL_MS });
//       pipe.ws = ws;
//       ws.on("error", () => {});
//       ws.once("open", () => {
//         timer?.();
//         timer = undefined;
//         if (pipe.done) return;
//         pipe.open = true;
//         ws.send(JSON.stringify({ id: 0, op: "auth", token: reach.token, pipe: true }));
//         pipe.cancelIdle = clock.schedule(() => retire(h, pipe), idleMaxMs, { unref: true });
//       });
//       // The daemon answers the auth frame as it answers a link's, then says nothing until a connection takes the
//       // pipe. Any other frame is a daemon that serves no pipes answering the socket as a link, with its hello.
//       const frame = (data: Buffer, isBinary: boolean): void => {
//         if (pipe.done) return;
//         const text = isBinary ? undefined : data.toString();
//         const auth = text === undefined ? undefined : authAnswer(text);
//         if (auth === true) {
//           pipe.authed = true;
//           // Back once the daemon has taken the token on a full pool, not on a socket that opened: a daemon refusing
//           // pipes past its own cap closes them after they open, and its refusals keep backing off.
//           if (h.attempt > 0 && [...h.idle].filter(p => p.authed).length === PIPE_IDLE) {
//             h.attempt = 0;
//             log(`${h.id}: ${PIPE_IDLE} pipes idle`);
//           }
//           return;
//         }
//         ws.off("message", frame);
//         if (auth === false) return refused(h);
//         const open = text === undefined ? undefined : pipeOpenOf(text);
//         if (open === undefined) return behind(h, `${h.id}: its daemon answered a pipe as a link, so it is asked again each minute`);
//         take(h, pipe, ws, open);
//       };
//       ws.on("message", frame);
//       ws.once("close", (code: number, reason: Buffer) => {
//         timer?.();
//         timer = undefined;
//         if (pipe.done) return;
//         if (daemonRefusedToken(code, reason.toString())) {
//           pipe.done = true;
//           h.idle.delete(pipe);
//           return refused(h);
//         }
//         failed();
//       });
//     }, failed);
//   };
//
//   const down = (id: string): void => {
//     const h = held.get(id);
//     if (h === undefined) return;
//     held.delete(id);
//     h.stopped = true;
//     h.retry?.();
//     h.pace?.();
//     endIdle(h);
//     for (const ws of h.taken) ws.terminate();
//     h.taken.clear();
//   };
//
//   return {
//     up(id) {
//       if (held.has(id)) return;
//       const h: Held = { id, idle: new Set(), taken: new Set(), attempt: 0, dialled: [], stopped: false, refusals: tokenRefusals(behindAgainMs), serves: false, asking: false, saidBehind: false, saidDialling: false, saidFull: false };
//       held.set(id, h);
//       fill(h);
//     },
//     down,
//     idle: id => [...(held.get(id)?.idle ?? [])].filter(p => p.open).length,
//     close() {
//       for (const id of [...held.keys()]) down(id);
//     },
//   };
// }
/// Spawns its dials with `spawn_local`: call it inside a `LocalSet`.
pub fn pipe_pool(o: PipePoolOptions) -> Rc<PipePool> {
    todo!()
}

/// The daemon's answer to a pipe's auth frame: `Some(ok)` for a `DaemonResponse` with id 0, `None` for any other
/// frame.
// TS pipe.ts:133-144
// /** The daemon's answer to a pipe's auth frame: true where it took the token, false where it refused it, undefined for
//  * any other frame. */
// function authAnswer(text: string): boolean | undefined {
//   let frame: unknown;
//   try {
//     frame = JSON.parse(text);
//   } catch {
//     return undefined;
//   }
//   const reply = DaemonResponse.safeParse(frame);
//   return reply.success && reply.data.id === 0 ? reply.data.ok : undefined;
// }
pub fn auth_answer(text: &str) -> Option<bool> {
    todo!()
}

/// The pipe's first frame: `{"id":0,"op":"auth","token":<token>,"pipe":true}`, in that key order.
pub fn auth_frame(token: &str) -> String {
    todo!()
}
