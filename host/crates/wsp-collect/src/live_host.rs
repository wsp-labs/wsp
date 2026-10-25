// SPDX-License-Identifier: AGPL-3.0-only
//! The live Host: this computer's disk and processes behind the Host seam. Port of packages/collect/src/live-host.ts
//! at a3b870a6e; the spec is spec/live_host.md. The one spawn helper here is chapter 06's `spawnRun`.
use std::collections::HashMap;

use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;

use crate::host::{Host, HostExec, HostFs, Platform, RunOptions, Stat};

/// Folders a size and a walk never enter (live-host.ts:11).
pub const SKIP_DIRS: [&str; 2] = [".git", "node_modules"];
/// How much of a file one read of its lines takes, in bytes (live-host.ts:129).
pub const LINE_CHUNK: usize = 1 << 20;
/// stdout past this many bytes fails the run (live-host.ts:146).
pub const MAX_OUTPUT: usize = 64 * 1024 * 1024;
/// How long a run waits after the child exits, or after the budget's SIGTERM, before it settles (live-host.ts:148).
pub const CLOSE_GRACE_MS: u64 = 1000;
/// The budget when `RunOptions.timeout_ms` is unset (live-host.ts:203).
pub const DEFAULT_TIMEOUT_MS: u64 = 120_000;

/// Every regular file's bytes under dir, links skipped, SKIP_DIRS not entered, unreadable entries counted as 0.
// live-host.ts:13
// async function treeBytes(dir: string): Promise<number> {
//   let total = 0;
//   let entries;
//   try {
//     entries = await readdir(dir, { withFileTypes: true });
//   } catch {
//     return 0;
//   }
//   for (const e of entries) {
//     if (e.isSymbolicLink()) continue;
//     const p = join(dir, e.name);
//     if (e.isDirectory()) {
//       if (!SKIP_DIRS.has(e.name)) total += await treeBytes(p);
//     } else if (e.isFile()) {
//       try {
//         total += (await stat(p)).size;
//       } catch {
//         continue;
//       }
//     }
//   }
//   return total;
// }
pub async fn tree_bytes(dir: &str) -> u64 {
    todo!()
}

/// The real disk (live-host.ts:53).
#[derive(Debug, Clone, Copy, Default)]
pub struct NodeFs;

impl HostFs for NodeFs {
    // live-host.ts:54
    //   async stat(path): Promise<Stat | undefined> {
    //     let s;
    //     try {
    //       s = await stat(path);
    //     } catch {
    //       return undefined;
    //     }
    //     if (s.isDirectory()) return { kind: "dir", bytes: await treeBytes(path), mtimeMs: s.mtimeMs };
    //     if (s.isFile()) return { kind: "file", bytes: s.size, mtimeMs: s.mtimeMs };
    //     return undefined;
    //   },
    fn stat<'a>(&'a self, path: &'a str) -> LocalBoxFuture<'a, Option<Stat>> {
        todo!()
    }

    // live-host.ts:65
    //   async list(dir) {
    //     try {
    //       return (await readdir(dir)).sort();
    //     } catch {
    //       return [];
    //     }
    //   },
    fn list<'a>(&'a self, dir: &'a str) -> LocalBoxFuture<'a, Vec<String>> {
        todo!()
    }

    // live-host.ts:72
    //   async readText(path) {
    //     try {
    //       return await readFile(path, "utf8");
    //     } catch {
    //       return undefined;
    //     }
    //   },
    fn read_text<'a>(&'a self, path: &'a str) -> LocalBoxFuture<'a, Option<String>> {
        todo!()
    }

    // live-host.ts:37
    // async function walkFiles(dir: string, out: string[]): Promise<void> {
    //   let entries;
    //   try {
    //     entries = await readdir(dir, { withFileTypes: true });
    //   } catch {
    //     return;
    //   }
    //   for (const e of entries) {
    //     if (e.isSymbolicLink()) continue;
    //     const p = join(dir, e.name);
    //     if (e.isDirectory()) {
    //       if (!SKIP_DIRS.has(e.name)) await walkFiles(p, out);
    //     } else if (e.isFile()) out.push(p);
    //   }
    // }
    // live-host.ts:79
    //   async walk(dir) {
    //     const out: string[] = [];
    //     await walkFiles(dir, out);
    //     return out.sort();
    //   },
    fn walk<'a>(&'a self, dir: &'a str) -> LocalBoxFuture<'a, Vec<String>> {
        todo!()
    }

    /// Bytes are searched for `holding` before anything is decoded; a line is the bytes up to LF, one trailing CR
    /// dropped, decoded as UTF-8 with U+FFFD for bad bytes; the last line is given without an LF and only when it has
    /// bytes. A read error ends the stream quietly, as the TS does (never an `Err` item).
    // live-host.ts:84
    //   async *lines(path, holding) {
    //     let file;
    //     try {
    //       file = await open(path, "r");
    //     } catch {
    //       return;
    //     }
    //     // Bytes are searched before anything is decoded: an agent's transcripts run to gigabytes, most of it tool output
    //     // a reader skips, and a line reader that decoded every line took 13 times as long as reading the bytes.
    //     const needle = holding === undefined ? undefined : Buffer.from(holding);
    //     const chunk = Buffer.allocUnsafe(LINE_CHUNK);
    //     let pieces: Buffer[] = [];
    //     const take = (line: Buffer): string | undefined => {
    //       if (needle !== undefined && line.indexOf(needle) === -1) return undefined;
    //       const end = line.length > 0 && line[line.length - 1] === CR ? line.length - 1 : line.length;
    //       return line.toString("utf8", 0, end);
    //     };
    //     try {
    //       for (;;) {
    //         let read: number;
    //         try {
    //           ({ bytesRead: read } = await file.read(chunk, 0, LINE_CHUNK, null));
    //         } catch {
    //           return;
    //         }
    //         if (read === 0) break;
    //         const data = chunk.subarray(0, read);
    //         let start = 0;
    //         for (let nl = data.indexOf(LF); nl !== -1; nl = data.indexOf(LF, start)) {
    //           const line = take(pieces.length === 0 ? data.subarray(start, nl) : Buffer.concat([...pieces, data.subarray(start, nl)]));
    //           pieces = [];
    //           start = nl + 1;
    //           if (line !== undefined) yield line;
    //         }
    //         if (start < read) pieces.push(Buffer.from(data.subarray(start)));
    //       }
    //       const last = pieces.length === 0 ? undefined : take(Buffer.concat(pieces));
    //       if (last !== undefined) yield last;
    //     } finally {
    //       await file.close();
    //     }
    //   },
    fn lines<'a>(&'a self, path: &'a str, holding: Option<&'a str>) -> LocalBoxStream<'a, std::io::Result<String>> {
        todo!()
    }
}

/// Whether `bin` is an executable file in a folder of PATH; empty PATH entries skipped (live-host.ts:133).
// live-host.ts:133
// async function onPath(bin: string): Promise<boolean> {
//   for (const dir of (process.env["PATH"] ?? "").split(":")) {
//     if (dir === "") continue;
//     try {
//       await access(join(dir, bin), constants.X_OK);
//       return true;
//     } catch {
//       continue;
//     }
//   }
//   return false;
// }
pub async fn on_path(bin: &str, path_var: Option<&str>) -> bool {
    todo!()
}

/// The real processes (live-host.ts:150): `which` reads this process's PATH, `run` spawns under this process's
/// environment with `opts.env` laid over it.
#[derive(Debug, Clone, Copy, Default)]
pub struct NodeExec;

impl HostExec for NodeExec {
    // live-host.ts:150
    // export const nodeExec: HostExec = {
    //   which: onPath,
    //   run: (cmd, args, opts = {}) => spawnRun(cmd, args, { ...process.env, ...opts.env }, opts),
    // };
    fn which<'a>(&'a self, bin: &'a str) -> LocalBoxFuture<'a, bool> {
        todo!()
    }

    fn run<'a>(&'a self, cmd: &'a str, args: &'a [String], opts: RunOptions) -> LocalBoxFuture<'a, Option<String>> {
        todo!()
    }
}

/// The pid and every process under it, breadth first, off one `ps -A -o pid=,ppid=` listing; `[pid]` alone when ps
/// fails (live-host.ts:157).
// live-host.ts:155
// /** The pid and every process under it, off one listing: a child that leads a group of its own, as timeout does, is
//  * out of reach of a kill sent to the group it was started in. */
// function treeOf(pid: number): number[] {
//   let listing: string;
//   try {
//     listing = execFileSync("ps", ["-A", "-o", "pid=,ppid="], { encoding: "utf8" });
//   } catch {
//     return [pid];
//   }
//   const children = new Map<number, number[]>();
//   for (const line of listing.split("\n")) {
//     const [p, parent] = line.trim().split(/\s+/).map(Number);
//     if (p === undefined || parent === undefined || Number.isNaN(p) || Number.isNaN(parent)) continue;
//     children.set(parent, [...(children.get(parent) ?? []), p]);
//   }
//   const tree = [pid];
//   for (let i = 0; i < tree.length; i++) tree.push(...(children.get(tree[i]!) ?? []));
//   return tree;
// }
pub fn tree_of(pid: u32) -> Vec<u32> {
    todo!()
}

/// The children map `tree_of` walks, read off a `ps -A -o pid=,ppid=` listing; a line that is not two numbers is
/// skipped, and children keep the listing's order.
pub fn children_of(listing: &str) -> HashMap<u32, Vec<u32>> {
    todo!()
}

/// One child as the leader of its own process group under exactly `env`, answered with its stdout on exit 0 and
/// `None` otherwise; the group dies with the run, past the budget or once the child exits (spec rules 5 to 12).
// live-host.ts:175
// /** One child as the leader of its own process group under `env` and nothing else, answered with its stdout on exit 0
//  * and nothing otherwise; the group dies with the run, past the budget or once the child exits. */
// export function spawnRun(cmd: string, args: readonly string[], env: NodeJS.ProcessEnv, opts: RunOptions): Promise<string | undefined> {
//   return new Promise(resolve => {
//     if (opts.signal?.aborted === true) {
//       resolve(undefined);
//       return;
//     }
//     const chunks: Buffer[] = [];
//     let bytes = 0;
//     let failed = false;
//     let settled = false;
//     let grace: NodeJS.Timeout | undefined;
//     // The child leads its own process group so a job an rc file left behind dies with it, not just the shell.
//     const child = spawn(cmd, [...args], { stdio: ["ignore", "pipe", "pipe"], env, detached: true });
//     const killGroup = (signal: NodeJS.Signals): void => {
//       if (child.pid === undefined) return;
//       try {
//         process.kill(-child.pid, signal);
//       } catch {
//         return;
//       }
//     };
//     const budget = setTimeout(() => {
//       failed = true;
//       killGroup("SIGTERM");
//       // An interactive shell ignores SIGTERM and would never exit, so the run ends on its own after the grace.
//       grace ??= setTimeout(() => settle(null, "SIGTERM"), CLOSE_GRACE_MS);
//     }, opts.timeoutMs ?? 120_000);
//     const abort = (): void => {
//       failed = true;
//       // Node reaps the child at exit, so its pid is free from then on; only the group a job still holds is ours.
//       if (child.exitCode !== null || child.signalCode !== null) {
//         settle(child.exitCode, child.signalCode);
//         return;
//       }
//       if (child.pid === undefined) return;
//       const tree = treeOf(child.pid);
//       killGroup("SIGKILL");
//       for (const pid of tree) {
//         try {
//           process.kill(pid, "SIGKILL");
//         } catch {
//           continue;
//         }
//       }
//     };
//     opts.signal?.addEventListener("abort", abort, { once: true });
//     const settle = (code: number | null, signal: NodeJS.Signals | null): void => {
//       if (settled) return;
//       settled = true;
//       clearTimeout(budget);
//       opts.signal?.removeEventListener("abort", abort);
//       if (grace !== undefined) clearTimeout(grace);
//       killGroup("SIGKILL");
//       child.stdout.destroy();
//       child.stderr.destroy();
//       resolve(!failed && code === 0 && signal === null ? Buffer.concat(chunks).toString("utf8") : undefined);
//     };
//     child.stdout.on("data", (b: Buffer) => {
//       bytes += b.length;
//       if (bytes > MAX_OUTPUT) {
//         failed = true;
//         killGroup("SIGKILL");
//       } else chunks.push(b);
//     });
//     child.stderr.resume();
//     child.on("error", () => {
//       failed = true;
//       settle(null, null);
//     });
//     child.on("exit", (code, signal) => {
//       clearTimeout(grace);
//       grace = setTimeout(() => settle(code, signal), CLOSE_GRACE_MS);
//     });
//     child.on("close", settle);
//   });
// }
pub async fn spawn_run(cmd: &str, args: &[String], env: &[(String, String)], opts: RunOptions) -> Option<String> {
    todo!()
}

/// `darwin` and `linux` only (live-host.ts:254); the argument is Rust's `std::env::consts::OS` word.
pub fn platform_of(os: &str) -> Option<Platform> {
    todo!()
}

/// This computer as a Host: platform, home, and SHELL, TERM_PROGRAM, XDG_CONFIG_HOME, CLAUDE_CONFIG_DIR and
/// CODEX_HOME where set and not empty; `stores` and `seeds` absent. An unknown platform is the TS throw's sentence.
// live-host.ts:258
// export function nodeHost(): Host {
//   const platform = platformOf(process.platform);
//   if (platform === undefined) throw new Error(`wsp reads a computer's setup on macOS or Linux only, not ${process.platform}`);
//   const shell = process.env["SHELL"];
//   const terminal = process.env["TERM_PROGRAM"];
//   const xdgConfigHome = process.env["XDG_CONFIG_HOME"];
//   const claudeConfigDir = process.env["CLAUDE_CONFIG_DIR"];
//   const codexHome = process.env["CODEX_HOME"];
//   return { platform, home: homedir(), ...(shell ? { shell } : {}), ...(terminal ? { terminal } : {}), ...(xdgConfigHome ? { xdgConfigHome } : {}), ...(claudeConfigDir ? { claudeConfigDir } : {}), ...(codexHome ? { codexHome } : {}), fs: nodeFs, exec: nodeExec };
// }
pub fn node_host() -> Result<Host, wsp_words::exit::HostError> {
    todo!()
}
