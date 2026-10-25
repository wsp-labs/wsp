// SPDX-License-Identifier: AGPL-3.0-only
//! Unit runs_road: slate-runs.ts:165-195, 244-361, 491-497 at 65f6b89c7. Spec: spec/runs_road.md.
//!
//! `on.line` and `on.end` are called with no borrow of the road's own state held (rule 3.3).
use std::rc::Rc;

use indexmap::IndexMap;
use wsp_core::clock::Clock;

use super::parts::{Reshape, ReshapeAnswer};

/// Which stream a line came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Out,
    Err,
}

// slate-runs.ts:165
// /** What a road starts: the agent's text as bash -c's one argument, the values beside it, never in it. */
// export interface RoadStart {
//   cmd: string;
//   args: string[];
//   cwd: string;
//   env: Record<string, string>;
//   stdin?: string;
//   timeoutS: number;
//   stream: boolean;
// }
/// What a road starts: the agent's text as bash -c's one argument, the values beside it, never in it.
#[derive(Clone, Debug, PartialEq)]
pub struct RoadStart {
    pub cmd: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub env: IndexMap<String, String>,
    pub stdin: Option<String>,
    pub timeout_s: f64,
    pub stream: bool,
}

// slate-runs.ts:176
// /** How a command ended, its streams whole and not yet scrubbed. */
// export interface RoadEnd {
//   code: number | null;
//   signal?: string | null;
//   error?: Error;
//   out: string;
//   err: string;
//   cut: boolean;
//   timedOut: boolean;
// }
/// How a command ended, its streams whole and not yet scrubbed. `error` is the spawn error's message.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoadEnd {
    pub code: Option<i64>,
    pub signal: Option<String>,
    pub error: Option<String>,
    pub out: String,
    pub err: String,
    pub cut: bool,
    pub timed_out: bool,
}

/// What a road tells as the command runs: each line where it streams, then the end, once.
pub trait RoadEvents {
    fn line(&self, stream: Stream, text: &str);
    fn end(&self, end: RoadEnd);
}

/// A start's answer: the kill takes the whole group; the pid is the command's on this computer, where it runs here.
#[derive(Clone)]
pub struct RoadStarted {
    pub kill: Rc<dyn Fn()>,
    pub pid: Option<u32>,
}

/// A `then`: `cmd` under bash -c with `input` on stdin.
#[derive(Clone)]
pub struct ReshapeStart {
    pub cmd: String,
    pub input: String,
    pub cwd: String,
    pub env: IndexMap<String, String>,
    pub timeout_s: f64,
    pub scrub: Rc<dyn Fn(&str) -> String>,
}

// slate-runs.ts:187
// /** The computer a run's command runs on: this one's child, or the thread's own machine. */
// export interface RunRoad {
//   /** SLATE_DIR on that computer, whose files the road writes before each command. */
//   slateDir: string;
//   /** Each line as it is printed where the run streams, then the end, once; kill ends the whole group. The pid is the
//    * command's on this computer, where it runs here. */
//   start(o: RoadStart, on: { line(stream: "out" | "err", text: string): void; end(o: RoadEnd): void }): { kill(): void; pid?: number };
//   reshape(o: { cmd: string; input: string; cwd: string; env: Record<string, string>; timeoutS: number; scrub: (text: string) => string }): Reshape;
// }
/// The computer a run's command runs on: this one's child, or the thread's own machine.
pub trait RunRoad {
    /// SLATE_DIR on that computer; empty for this one.
    fn slate_dir(&self) -> String;
    fn start(&self, o: RoadStart, on: Rc<dyn RoadEvents>) -> RoadStarted;
    fn reshape(&self, o: ReshapeStart) -> Reshape;
}

// slate-runs.ts:244
// /** Runs `then` under bash -c in `cwd` with `input` on stdin; its stdout must parse as JSON. Output is capped as a
//  * run's, scrubbed, and the whole group dies at the deadline or on kill. */
// export function reshapeResult(o: { cmd: string; input: string; cwd: string; env: Record<string, string>; timeoutS: number; scrub: (text: string) => string }): Reshape {
//   const child = spawn("bash", ["-c", o.cmd], { cwd: o.cwd, env: o.env, detached: true, stdio: ["pipe", "pipe", "pipe"] });
//   const chunks = { out: [] as Buffer[], err: [] as Buffer[] };
//   let total = 0;
//   const take = (stream: "out" | "err") => (chunk: Buffer): void => {
//     const room = Math.max(0, EXEC_OUTPUT_MAX - total);
//     total += Math.min(room, chunk.length);
//     if (room > 0) chunks[stream].push(chunk.subarray(0, room));
//   };
//   child.stdout?.on("data", take("out"));
//   child.stderr?.on("data", take("err"));
//   child.stdin?.on("error", () => {});
//   child.stdin?.end(o.input);
//   let timedOut = false;
//   const kill = (): void => {
//     if (child.pid !== undefined) killGroup(child.pid);
//   };
//   const deadline = setTimeout(() => {
//     timedOut = true;
//     kill();
//   }, o.timeoutS * 1_000);
//   const done = new Promise<ReshapeAnswer>(settle => {
//     let ended = false;
//     const end = (code: number | null, error?: Error): void => {
//       if (ended) return;
//       ended = true;
//       clearTimeout(deadline);
//       kill();
//       settle(reshapeAnswer({ code, ...(error !== undefined ? { error } : {}), out: Buffer.concat(chunks.out).toString("utf8"), err: Buffer.concat(chunks.err).toString("utf8"), cut: false, timedOut }, o.timeoutS, o.scrub));
//     };
//     child.on("error", e => end(null, e));
//     child.on("close", code => end(code));
//   });
//   return { done, kill };
// }
/// Needs a tokio `LocalSet` on the current thread.
pub fn reshape_result(o: ReshapeStart, clock: Rc<dyn Clock>) -> Reshape {
    todo!()
}

// slate-runs.ts:282
// /** What a `then` that ended says: the JSON it printed, or why not, on whichever computer it ran. */
// export function reshapeAnswer(end: RoadEnd, timeoutS: number, scrub: (text: string) => string): ReshapeAnswer {
//   const err = runOutputTail(scrub(end.err));
//   if (end.error !== undefined) return { why: `then could not start: ${scrub(end.error.message)}`, exit: null, err };
//   if (end.timedOut) return { why: `then timed out after ${timeoutS} s`, exit: EXEC_DEADLINE_EXIT, err };
//   if (end.code !== 0) return { why: `then exited with ${end.code ?? "a signal"}`, exit: end.code, err };
//   const out = scrub(end.out);
//   try {
//     return { json: JSON.parse(out) as SlateJson };
//   } catch {
//     return { why: `then printed no JSON${out.trim() === "" ? "" : `: ${out.trim().split("\n")[0]!.slice(0, 120)}`}`, exit: 0, err };
//   }
// }
pub fn reshape_answer(end: &RoadEnd, timeout_s: f64, scrub: &dyn Fn(&str) -> String) -> ReshapeAnswer {
    todo!()
}

// slate-runs.ts:296
// /** This computer: the command a child of the host's, in its own process group, which a kill or the deadline takes
//  * whole, its streams capped as the daemon caps an exec's. */
// const localRoad: RunRoad = {
//   slateDir: "",
//   start(o, on) {
//     const child = spawn("bash", ["-c", o.cmd, "bash", ...o.args], { cwd: o.cwd, env: o.env, detached: true, stdio: [o.stdin === undefined ? "ignore" : "pipe", "pipe", "pipe"] });
//     const chunks = { out: [] as Buffer[], err: [] as Buffer[] };
//     let total = 0;
//     let cut = false;
//     const partial = { out: "", err: "" };
//     const decoders = { out: new StringDecoder("utf8"), err: new StringDecoder("utf8") };
//     const take = (stream: "out" | "err") => (chunk: Buffer): void => {
//       if (total + chunk.length > EXEC_OUTPUT_MAX) {
//         cut = true;
//         chunk = chunk.subarray(0, Math.max(0, EXEC_OUTPUT_MAX - total));
//       }
//       total += chunk.length;
//       if (chunk.length === 0) return;
//       chunks[stream].push(chunk);
//       if (!o.stream) return;
//       const parts = (partial[stream] + decoders[stream].write(chunk)).split("\n");
//       partial[stream] = parts.pop() ?? "";
//       for (const p of parts) on.line(stream, p);
//     };
//     child.stdout?.on("data", take("out"));
//     child.stderr?.on("data", take("err"));
//     if (child.stdin !== null) {
//       child.stdin.on("error", () => {});
//       child.stdin.end(o.stdin);
//     }
//     let timedOut = false;
//     let ended = false;
//     let grace: ReturnType<typeof setTimeout> | undefined;
//     const deadline = setTimeout(() => {
//       timedOut = true;
//       if (child.pid !== undefined) killGroup(child.pid);
//     }, o.timeoutS * 1_000);
//     const end = (code: number | null, signal: NodeJS.Signals | null, error?: Error): void => {
//       if (ended) return;
//       ended = true;
//       clearTimeout(deadline);
//       if (grace !== undefined) clearTimeout(grace);
//       if (child.pid !== undefined) killGroup(child.pid);
//       if (o.stream) {
//         for (const stream of ["out", "err"] as const) {
//           const rest = partial[stream] + decoders[stream].end();
//           if (rest !== "") on.line(stream, rest);
//         }
//       }
//       on.end({ code, signal, ...(error !== undefined ? { error } : {}), out: Buffer.concat(chunks.out).toString("utf8"), err: Buffer.concat(chunks.err).toString("utf8"), cut, timedOut });
//     };
//     child.on("error", e => end(null, null, e));
//     child.on("exit", (code, signal) => {
//       // What the leader left running in its group goes with it, and the pipes close once it has.
//       if (child.pid !== undefined) killGroup(child.pid);
//       grace = setTimeout(() => {
//         child.stdout?.destroy();
//         child.stderr?.destroy();
//         end(code, signal);
//       }, CLOSE_GRACE_MS);
//     });
//     child.on("close", (code, signal) => end(code, signal));
//     return { kill: () => (child.pid !== undefined ? killGroup(child.pid) : undefined), ...(child.pid !== undefined ? { pid: child.pid } : {}) };
//   },
//   reshape: o => reshapeResult(o),
// };
/// This computer's road. Needs a tokio `LocalSet` on the current thread.
pub fn local_road(clock: Rc<dyn Clock>) -> Rc<dyn RunRoad> {
    todo!()
}

// slate-runs.ts:491
// function killGroup(pid: number): void {
//   try {
//     process.kill(-pid, "SIGKILL");
//   } catch {
//     return;
//   }
// }
/// SIGKILL to the group `pid` leads; every error ignored.
pub fn kill_group(pid: u32) {
    todo!()
}
