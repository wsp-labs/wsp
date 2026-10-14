// SPDX-License-Identifier: AGPL-3.0-only
// The reads the folder browsers make, in a node child of their own and never on the host's thread: a read inside a
// folder macOS guards holds in open() until the person answers its prompt, and one on the host's thread held every
// page, thread and command with it. A worker thread or the libuv pool would hold it too, since process.exit waits on
// both while a read is stuck (measured on Node 22), so the only way past a read that does not answer is to kill the
// process doing it. Each read has FOLDER_ANSWER_MS; the child is killed when one runs out and the next read starts
// a new one.
import { spawn, type ChildProcessByStdio } from "node:child_process";
import type { Readable, Writable } from "node:stream";

/** How long one folder has to answer a read before it is skipped. */
export const FOLDER_ANSWER_MS = 2000;

/** How long a new child has to start answering at all, apart from any folder's own time. */
const START_MS = 15_000;

/** A folder that did not answer in time, whose read the child was killed in. */
export class FolderSilent extends Error {
  constructor(readonly path: string) {
    super(`${path} did not answer within ${FOLDER_ANSWER_MS / 1000} s, so wsp skipped it`);
  }
}

/** One folder as a level lists it: whether it is a folder through any link, where it really is, and whether it holds
 * a .git, which is looked for only when `peek`. */
export interface FolderFacts {
  dir: boolean;
  real: string | null;
  repo: boolean;
}

export interface LevelEntry {
  name: string;
  dir: boolean;
  link: boolean;
}

/** One folder as the repos walk meets it: a repo, whose .git is a folder or a file, or the folders it holds. */
export type WalkStep = { git: { dir: boolean; branch?: string; touchedAt: number } } | { dirs: string[] };

/** One path to read, with what the op needs besides. */
export interface FolderAsk {
  path: string;
  peek?: boolean;
}

export interface FolderReads {
  /** Reads each path in turn, and answers each with what was read there, the error it raised, or a FolderSilent for one
   * that did not answer in time, after which a new child reads the rest. */
  read(op: "folder", asks: readonly FolderAsk[]): Promise<(FolderFacts | Error)[]>;
  read(op: "level", asks: readonly FolderAsk[]): Promise<(LevelEntry[] | Error)[]>;
  read(op: "walk", asks: readonly FolderAsk[]): Promise<(WalkStep | Error)[]>;
  close(): void;
}

/** Starts every line the child answers with, so a line anything else in it prints, a preload from NODE_OPTIONS
 * among them, is never read as an answer. */
const MARK = "wsp-folder-reads ";

/** The child: a ready line first, then for each line of asks one line per ask, as each is read. Modules come through
 * getBuiltinModule, which answers whether -e runs it as a script or a module. */
const READER = `
const MARK = ${JSON.stringify(MARK)};
const fs = process.getBuiltinModule("node:fs");
const { join } = process.getBuiltinModule("node:path");
const { createInterface } = process.getBuiltinModule("node:readline");
const stamp = p => { try { return fs.statSync(p).mtimeMs; } catch { return 0; } };
const ops = {
  folder({ path, peek }) {
    let dir = false, real = null;
    try { dir = fs.statSync(path).isDirectory(); } catch {}
    try { real = fs.realpathSync(path); } catch {}
    return { dir, real, repo: dir && peek && fs.existsSync(join(path, ".git")) };
  },
  level({ path }) {
    return fs.readdirSync(path, { withFileTypes: true }).map(e => ({ name: e.name, dir: e.isDirectory(), link: e.isSymbolicLink() }));
  },
  walk({ path }) {
    const git = join(path, ".git");
    if (fs.existsSync(git)) {
      let dir = false, branch;
      try { dir = fs.statSync(git).isDirectory(); } catch {}
      try { branch = /^ref: refs\\/heads\\/(.+)$/m.exec(fs.readFileSync(join(git, "HEAD"), "utf8"))?.[1]; } catch {}
      const touchedAt = Math.max(stamp(join(git, "index")), stamp(join(git, "HEAD")), stamp(join(git, "FETCH_HEAD")));
      return { git: { dir, ...(branch !== undefined ? { branch } : {}), touchedAt } };
    }
    try {
      return { dirs: fs.readdirSync(path, { withFileTypes: true }).filter(e => e.isDirectory()).map(e => e.name) };
    } catch {
      return { dirs: [] };
    }
  },
};
process.stdout.write(MARK + "{}\\n");
createInterface({ input: process.stdin }).on("line", line => {
  const { op, asks } = JSON.parse(line);
  for (const ask of asks) {
    let reply;
    try { reply = { ok: ops[op](ask) }; } catch (e) { reply = { error: e instanceof Error ? e.message : String(e) }; }
    process.stdout.write(MARK + JSON.stringify(reply) + "\\n");
  }
});
`;

type Child = ChildProcessByStdio<Writable, Readable, null>;
interface Reply {
  ok?: unknown;
  error?: string;
}

/** A child that answers one line of asks at a time, made at the first and again after one it was killed in. */
export function folderReads(): FolderReads {
  let child: Child | undefined;
  let ready: Promise<void> | undefined;
  let answer: ((reply: Reply) => void) | undefined;
  let ended: ((e: Error) => void) | undefined;

  const start = (): Promise<void> => {
    // An app's own binary runs as node only with this set, and the host may run under one.
    const c = spawn(process.execPath, ["-e", READER], { stdio: ["pipe", "pipe", "ignore"], env: { ...process.env, ELECTRON_RUN_AS_NODE: "1" } });
    child = c;
    c.stdin.on("error", () => undefined);
    let buffered = "";
    let started: (() => void) | undefined;
    const gone = (e: Error): void => {
      if (child !== c) return;
      child = undefined;
      ended?.(e);
    };
    c.on("error", gone);
    c.on("exit", () => gone(new Error("the folder reader ended")));
    c.stdout.setEncoding("utf8").on("data", (chunk: string) => {
      buffered += chunk;
      for (let at = buffered.indexOf("\n"); at >= 0; at = buffered.indexOf("\n")) {
        const line = buffered.slice(0, at);
        buffered = buffered.slice(at + 1);
        if (!line.startsWith(MARK)) continue;
        if (started !== undefined) {
          started();
          started = undefined;
        } else if (child === c) answer?.(JSON.parse(line.slice(MARK.length)) as Reply);
      }
    });
    return new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        c.kill("SIGKILL");
        reject(new Error("the folder reader did not start"));
      }, START_MS);
      started = () => {
        clearTimeout(timer);
        resolve();
      };
      ended = e => {
        clearTimeout(timer);
        reject(e);
      };
    });
  };

  /** Sends the asks as one line and takes the answers as they come, each with its own time: the first that runs out
   * is silent, the child is killed, and the answers so far come back short of it. */
  const send = async (op: string, asks: readonly FolderAsk[]): Promise<(unknown | Error)[]> => {
    if (child === undefined || ready === undefined) ready = start();
    await ready;
    const c = child!;
    const answers: (unknown | Error)[] = [];
    return new Promise((resolve, reject) => {
      let timer: NodeJS.Timeout;
      const done = (): void => {
        clearTimeout(timer);
        answer = ended = undefined;
      };
      const wait = (): void => {
        clearTimeout(timer);
        timer = setTimeout(() => {
          done();
          child = undefined;
          c.kill("SIGKILL");
          answers.push(new FolderSilent(asks[answers.length]!.path));
          resolve(answers);
        }, FOLDER_ANSWER_MS);
      };
      answer = reply => {
        answers.push(reply.error !== undefined ? new Error(reply.error) : reply.ok);
        if (answers.length < asks.length) return wait();
        done();
        resolve(answers);
      };
      ended = e => {
        done();
        reject(e);
      };
      wait();
      c.stdin.write(`${JSON.stringify({ op, asks })}\n`);
    });
  };

  const read = async (op: string, asks: readonly FolderAsk[]): Promise<(unknown | Error)[]> => {
    const answers: (unknown | Error)[] = [];
    while (answers.length < asks.length) answers.push(...(await send(op, asks.slice(answers.length))));
    return answers;
  };

  return {
    read: read as FolderReads["read"],
    close: () => {
      child?.kill();
      child = undefined;
    },
  };
}
