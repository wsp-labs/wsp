// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/daemon-channel.ts at 65f6b89c7. Spec: spec/daemon_channel.md.
//!
//! One dial of one daemon for one client of this host, with no redial and no subscribe ritual: the page that opened
//! the channel owns the retry and asks for ports.watch and the rest itself. The host's own held links (reach.rs) keep
//! a machine warm and re-dial it, which is a different concern.
use std::rc::Rc;

use serde_json::{Map, Value};

use crate::clock::Clock;

/// A daemon frame or reply: a JSON object, keys in the order written.
pub type Frame = Map<String, Value>;

// export interface DaemonChannelOptions {
//   /** The road daemonReach answered: a minted route or a loopback url; the host turns http(s) into ws(s). */
//   url: string;
//   token: string;
//   /** Every frame that is not the answer to a send, the hello included, in the order the daemon sent them. */
//   onEvent(frame: Record<string, unknown>): void;
//   connectTimeoutMs?: number;
// }
pub struct DaemonChannelOptions {
    /// The road daemonReach answered: a minted route or a loopback url; http(s) becomes ws(s).
    pub url: String,
    pub token: String,
    /// Every frame that is not the answer to a send, the hello included, in the order the daemon sent them.
    pub on_event: Rc<dyn Fn(Value)>,
    pub connect_timeout_ms: Option<i64>,
    /// Defaults to the real clock (rule 3.15).
    pub clock: Option<Rc<dyn Clock>>,
}

/// How the daemon socket ended: the close code and its reason as UTF-8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelEnd {
    pub code: u16,
    pub reason: String,
}

// /** The door answered the upgrade with a status: the socket never opened. kind is what the wire carries. The message
//  * is shown under the pane's own sentence, so it carries the status and the door's words and none of ours: a person
//  * never reads machine or daemon. */
// export class DaemonDoorError extends Error {
//   readonly kind = "refused";
//   readonly status: number;
//   constructor(status: number, body: string) {
//     // One line of the body: a door that answers an HTML page must not become the pane's sentence.
//     const line = body.split("\n", 1)[0]!.trim();
//     super(line === "" ? `the connection was refused with ${status}` : `the connection was refused with ${status}: ${line}`);
//     this.name = "DaemonDoorError";
//     this.status = status;
//   }
// }
/// The door answered the upgrade with a status: the socket never opened. kind "refused".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonDoorError {
    pub status: u16,
    pub message: String,
}

impl DaemonDoorError {
    /// One line of the body: `the connection was refused with <status>[: <first line>]`.
    pub fn new(status: u16, body: &str) -> Self {
        todo!()
    }
}

// /** The daemon took the upgrade and closed 4401 before answering the auth frame. */
// export class DaemonTokenError extends Error {
//   readonly kind = "reauth";
//   constructor(reason: string) {
//     super(`the machine refused this host's daemon token (4401 ${reason})`);
//     this.name = "DaemonTokenError";
//   }
// }
/// The daemon took the upgrade and closed 4401 before answering the auth frame. kind "reauth".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonTokenError {
    pub message: String,
}

impl DaemonTokenError {
    /// `the machine refused this host's daemon token (4401 <reason>)`.
    pub fn new(reason: &str) -> Self {
        todo!()
    }
}

// /** The link to the daemon gave out with no answer from it: a dial that timed out, a socket that was not open, one
//  * that closed under a frame. Its own class and no kind, since a kind is what the wire carries and these say nothing
//  * to anyone but the host, which tells them from the daemon's own refusal by the class. */
// class DaemonUnanswered extends Error {}
// const daemonUnanswered = (message: string, cause?: unknown): Error => new DaemonUnanswered(message, cause === undefined ? undefined : { cause });
// export const isDaemonUnanswered = (e: unknown): boolean => e instanceof DaemonUnanswered;
/// Every way a dial or a send fails. `Unanswered` is the TS's `DaemonUnanswered` class (no kind); `Refused` is the
/// plain Error a refused auth reply rejects with (no kind).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChannelError {
    #[error("{message}")]
    Unanswered { message: String, cause: Option<ChannelEnd> },
    #[error("{}", .0.message)]
    Door(DaemonDoorError),
    #[error("{}", .0.message)]
    Token(DaemonTokenError),
    #[error("{message}")]
    Refused { message: String },
}

impl ChannelError {
    /// The kind the wire carries: "refused" for a door, "reauth" for a token, none for the rest.
    pub fn kind(&self) -> Option<&'static str> {
        todo!()
    }
}

// daemon-channel.ts:33
// export const isDaemonUnanswered = (e: unknown): boolean => e instanceof DaemonUnanswered;
pub fn is_daemon_unanswered(e: &ChannelError) -> bool {
    todo!()
}

// /** The dial, the auth frame and the reply that proves it, before a connect is given up. Named here rather than
//  * imported from the host: the dependency arrows run runtime to host, never back. */
// const CONNECT_TIMEOUT_MS = 15_000;
pub const CONNECT_TIMEOUT_MS: i64 = 15_000;

// /** The bytes of a refusing door's body kept for its sentence; the first line is what a pane shows. */
// const DOOR_BODY_CAP = 512;
pub const DOOR_BODY_CAP: usize = 512;

/// The channel's state; the implementer fills it.
struct ChannelInner {}

// export interface DaemonChannel {
//   send(frame: DaemonFrame): Promise<DaemonResponse>;
//   close(): void;
//   /** Settles once, when the daemon socket ends for any reason, close() included. */
//   readonly closed: Promise<{ code: number; reason: string }>;
// }
/// One open channel. Clones share it.
#[derive(Clone)]
pub struct DaemonChannel {
    inner: Rc<ChannelInner>,
}

impl DaemonChannel {
    /// The frame with `id` set (the caller's keys, then `id`); the daemon's reply as it wrote it, `ok: false`
    /// included. A socket that is not open fails `daemon unreachable`; one that closes under the frame `connection lost`.
    pub async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        todo!()
    }
    /// Closes with 1000.
    pub fn close(&self) {
        todo!()
    }
    /// Settles once, when the daemon socket ends for any reason, close() included; every await reads the same end.
    pub async fn closed(&self) -> ChannelEnd {
        todo!()
    }
}

// /** Dials once, sends the auth frame first, resolves on its reply. */
// export function openDaemonChannel(o: DaemonChannelOptions): Promise<DaemonChannel> {
//   const timeoutMs = o.connectTimeoutMs ?? CONNECT_TIMEOUT_MS;
//   return new Promise((resolve, reject) => {
//     const ws = new WebSocket(daemonWsUrl(o.url));
//     const pending = new Map<number, Pending>();
//     let nextId = 1;
//     let settled = false;
//     /** The door's own answer, kept so the close that follows it reads as a refusal rather than a dead machine. */
//     let refusal: DaemonDoorError | undefined;
//     let resolveClosed: (end: { code: number; reason: string }) => void = () => {};
//     const closed = new Promise<{ code: number; reason: string }>(r => (resolveClosed = r));
//
//     const connectTimer = setTimeout(() => {
//       if (settled) return;
//       settled = true;
//       ws.terminate();
//       reject(daemonUnanswered(`daemon connect timed out after ${timeoutMs}ms`));
//     }, timeoutMs);
//
//     const send = (frame: DaemonFrame): Promise<DaemonResponse> => {
//       if (ws.readyState !== ws.OPEN) return Promise.reject(daemonUnanswered("daemon unreachable"));
//       const id = nextId++;
//       return new Promise((res, rej) => {
//         pending.set(id, { resolve: res, reject: rej });
//         ws.send(JSON.stringify({ ...frame, id }));
//       });
//     };
//
//     const channel: DaemonChannel = {
//       send,
//       close: () => ws.close(1000),
//       closed,
//     };
//
//     // ws folds a non-101 answer into an error and a 1006 close without this listener, which is how a refusing door
//     // and a machine that is not there would read the same. The response's body is the door's own sentence.
//     ws.on("unexpected-response", (_req, res) => {
//       let body = "";
//       res.on("data", (chunk: Buffer) => {
//         if (body.length < DOOR_BODY_CAP) body += chunk.toString("utf8");
//       });
//       const done = (): void => {
//         refusal ??= new DaemonDoorError(res.statusCode ?? 0, body.slice(0, DOOR_BODY_CAP));
//         if (settled) return;
//         settled = true;
//         clearTimeout(connectTimer);
//         reject(refusal);
//       };
//       res.on("end", done);
//       res.on("error", done);
//     });
//
//     ws.on("open", () => {
//       send({ op: "auth", token: o.token } as unknown as DaemonFrame).then(
//         reply => {
//           if (settled) return;
//           settled = true;
//           clearTimeout(connectTimer);
//           if (reply.ok === true) resolve(channel);
//           else reject(new Error(String((reply as { error?: unknown }).error ?? "the machine refused the daemon link")));
//         },
//         () => {}, // the close handler owns the rejection
//       );
//     });
//
//     ws.on("message", raw => {
//       let msg: Record<string, unknown>;
//       try {
//         msg = JSON.parse(String(raw)) as Record<string, unknown>;
//       } catch {
//         return;
//       }
//       const id = msg["id"];
//       if (typeof id === "number" && pending.has(id)) {
//         const p = pending.get(id)!;
//         pending.delete(id);
//         p.resolve(msg as DaemonResponse);
//         return;
//       }
//       o.onEvent(msg);
//     });
//
//     ws.on("error", () => {}); // a close event always follows
//
//     ws.on("close", (code: number, reasonBuf: Buffer) => {
//       clearTimeout(connectTimer);
//       const reason = reasonBuf.toString("utf8");
//       for (const p of pending.values()) p.reject(daemonUnanswered("connection lost"));
//       pending.clear();
//       resolveClosed({ code, reason });
//       if (settled) return;
//       settled = true;
//       if (refusal !== undefined) reject(refusal);
//       else if (daemonRefusedToken(code, reason)) reject(new DaemonTokenError(reason));
//       // The close code is for the log, never for the person's eyes.
//       else reject(daemonUnanswered("the link to the computer ended before it opened", { code, reason }));
//     });
//   });
// }
/// Dials once, sends the auth frame first, resolves on its reply. Runs on the owner thread's `LocalSet`.
pub async fn open_daemon_channel(o: DaemonChannelOptions) -> Result<DaemonChannel, ChannelError> {
    todo!()
}
