// SPDX-License-Identifier: AGPL-3.0-only
// The files a slate run reads: the slate's own files its command names, and the scripts in the thread's folder an
// approval binds by hash, read on this disk or, for a command on the thread's own computer, by that computer's daemon.
import { createHash } from "node:crypto";
import { mkdirSync, readdirSync, readFileSync, realpathSync, rmSync, statSync, writeFileSync } from "node:fs";
import { isAbsolute, join, posix, relative, resolve, sep } from "node:path";
import { FS_HASH_CAP_BYTES, FS_HASH_FILES_MAX, FS_HASH_PATHS_MAX, type FsHashReply } from "@wsp/protocol";
import type { SlateDoc, SlateRunDecl } from "@wsp/protocol/slate";

/** The files a run's cmd or then reads: every one where it names $SLATE_DIR, else the ones it names. */
function filesRead(doc: SlateDoc | null, decl: SlateRunDecl): Record<string, string> | undefined {
  const said = [decl.kind === "cmd" ? decl.cmd : "", (decl as { then?: string }).then ?? ""].join("\n");
  const every = /\$\{?SLATE_DIR\b/.test(said);
  const used = Object.entries(doc?.files ?? {}).filter(([name]) => every || new RegExp(`(^|[^A-Za-z0-9._-])${name.replace(/[.]/g, "\\.")}($|[^A-Za-z0-9._-])`).test(said));
  return used.length === 0 ? undefined : Object.fromEntries(used);
}

/** The slate's folder made to hold its document's files and nothing else, written before every command reads it: a
 * rewound slate runs the code of the turn it went back to. */
export function writeSlateFiles(dir: string, files: Readonly<Record<string, string>>): string {
  mkdirSync(dir, { recursive: true, mode: 0o700 });
  for (const name of readdirSync(dir)) if (files[name] === undefined) rmSync(join(dir, name), { recursive: true, force: true });
  for (const [name, text] of Object.entries(files)) {
    // Removed first, so a link left in its place is never followed and the mode is the new file's.
    rmSync(join(dir, name), { recursive: true, force: true });
    writeFileSync(join(dir, name), text, { mode: 0o600, flag: "wx" });
  }
  return dir;
}

/** A declaration with the text of the files it reads beside it, as its approval key and its sheet take it. */
export function withFiles(doc: SlateDoc | null, decl: SlateRunDecl): SlateRunDecl & { files?: Record<string, string> } {
  const files = filesRead(doc, decl);
  return files === undefined ? decl : { ...decl, files };
}

/** The words of a run's cmd or then that read as a path: what an Always has to cover the content of. */
export function pathsNamed(decl: Extract<SlateRunDecl, { kind: "cmd" }>): string[] {
  return [decl.cmd, decl.then ?? ""].join("\n").split(/[\s'"`;|&()<>=,]+/).filter(w => w !== "" && !w.startsWith("-") && !w.includes("$") && !w.includes("://") && /[./]/.test(w));
}

/** Each word a run names, a relative one joined as text to the folder the command runs in, as slate-runs starts it
 * (resolve here, posix.resolve on the thread's machine), so a link on the way is followed where bash follows it. */
const wordsAt = (folder: string, decl: Extract<SlateRunDecl, { kind: "cmd" }>, there: boolean): string[] => {
  const ran = decl.cwd === undefined ? folder : there ? posix.resolve(folder, decl.cwd) : resolve(folder, decl.cwd);
  return pathsNamed(decl).map(word => (word.startsWith("/") ? word : `${ran === "/" ? "" : ran}/${word}`));
};

/** What a read of the files a run names found, on this computer or by the daemon of the thread's own: the daemon's
 * reply. */
export type HashRead = FsHashReply;

/** The thread's own computer as its daemon reads the files a run names there, under `root`. Throws where that daemon
 * cannot say. */
export type HashOn = (root: string, paths: string[]) => Promise<HashRead>;

/** Why an Always cannot hold to a file a run names: its computer could not read it, it is a script that lands outside
 * the thread's folder, by its path, from the home folder or through a link, one past what one read hashes, by count
 * or by size, one that is not a regular file, one whose name holds a space, an escape or a $, or it comes after the
 * paths one ask on the thread's machine sends. */
type Unpinned = { path: string; why: "unread" | "outside" | "link" | "past" | "special" | "built" | "paths" };

/** The hashes an approval of a run binds, and the files it names that no hash can bind. */
export type Pin = { scripts?: Record<string, string>; unpinned: Unpinned[] };

/** The hashes a pin puts in an approval key, a file no hash binds standing there by why: a key no Always is ever given
 * under, so the command asks. */
export const keyed = (pin: Pin): Record<string, string> | undefined =>
  pin.unpinned.length === 0 ? pin.scripts : { ...pin.scripts, ...Object.fromEntries(pin.unpinned.map(u => [u.path, u.why])) };

const UNPINNED = {
  outside: "is outside the thread's folder",
  link: "is a link out of the thread's folder",
  past: `is past what one command can pin (${FS_HASH_FILES_MAX} files, each up to ${FS_HASH_CAP_BYTES / 1024 / 1024} MiB)`,
  special: "is not a regular file",
  built: "is a script wsp cannot read by name (a space, an escape or a $ in it)",
  paths: `is past the ${FS_HASH_PATHS_MAX} paths one command can name`,
} as const;

/** Each script a run names that no hash can bind, and why: what the sheet and a refused Always say. */
const whyUnpinned = (pin: Pin): string | undefined => {
  const said = pin.unpinned.flatMap(u => (u.why === "unread" ? [] : [`${u.path} ${UNPINNED[u.why]}`]));
  return said.length === 0 ? undefined : said.join(" and ");
};

/** The sheet's line for a run naming a script no hash can bind. */
export const cannotPin = (pin: Pin): string | undefined => {
  const why = whyUnpinned(pin);
  return why === undefined ? undefined : `${why}, so wsp cannot pin ${pin.unpinned.length === 1 ? "it" : "them"} and it asks every time`;
};

/** Why an Always for a run is refused, where it names a file no hash binds. */
export const alwaysRefused = (run: string, pin: Pin): string | undefined => {
  if (pin.unpinned.length === 0) return undefined;
  const why = whyUnpinned(pin);
  return why === undefined ? `$${run} runs on the thread's machine and names ${pin.unpinned.map(u => u.path).join(", ")} there, which its computer could not hash to hold an Always to.` : `$${run} names a script wsp cannot pin to hold an Always to: ${why}.`;
};

const SCRIPT = /\.(sh|bash|zsh|fish|ksh|py|js|mjs|cjs|ts|mts|cts|jsx|tsx|rb|pl|pm|php|lua|r|jl|ps1|awk|tcl)$/i;
/** The programs that run the word after them: an interpreter, or one that starts another program. */
const RUNNERS = new Set(["sh", "bash", "zsh", "dash", "ksh", "fish", "source", ".", "python", "python2", "python3", "node", "nodejs", "deno", "bun", "tsx", "ts-node", "ruby", "perl", "php", "lua", "Rscript", "env", "exec", "nohup", "sudo", "time", "nice", "command"]);

/** Whether a program is a runner by its name, wherever it sits and whatever version it carries: `.venv/bin/python3.12`. */
const runner = (word: string): boolean => {
  const name = word.split("/").pop()!;
  return RUNNERS.has(name) || RUNNERS.has(name.replace(/[\d.]+$/, ""));
};

/** The flags after which a runner takes code, not a script's name. */
const INLINE = new Set(["-c", "-e", "--eval", "-m"]);

/** A word of a run's command as bash reads it: its text with quotes and escapes taken off, as written, whether it
 * holds what wsp cannot read as a name (a space, an escape, or a $ other than the slate's own folder), and whether the
 * command runs it: a word a runner is handed, or a relative path run as a program. An absolute one run as a program
 * is a tool of the computer, as `df` is. */
type Word = { text: string; said: string; loose: boolean; run: boolean };

/** The words of a run's cmd and then, each simple command split on ; | & ( ) and new lines outside quotes. */
function wordsOf(decl: Extract<SlateRunDecl, { kind: "cmd" }>): Word[] {
  const text = [decl.cmd, decl.then ?? ""].join("\n");
  const commands: Word[][] = [[]];
  let word: { text: string; said: string; escaped: boolean } | undefined;
  let quote: string | undefined;
  const end = (): void => {
    if (word !== undefined) commands.at(-1)!.push({ text: word.text, said: word.said, loose: word.escaped || /\s/.test(word.text) || word.said.replace(/\$\{?SLATE_DIR\b\}?/g, "").includes("$"), run: false });
    word = undefined;
  };
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i]!;
    if (quote === undefined && /[;|&()\n]/.test(c)) {
      end();
      commands.push([]);
      continue;
    }
    if (quote === undefined && /\s/.test(c)) {
      end();
      continue;
    }
    word ??= { text: "", said: "", escaped: false };
    word.said += c;
    if (quote === undefined && (c === "'" || c === '"')) quote = c;
    else if (c === quote) quote = undefined;
    else if (c === "\\" && quote !== "'" && i + 1 < text.length) {
      i += 1;
      word.said += text[i];
      word.text += text[i];
      word.escaped = true;
    } else word.text += c;
  }
  end();
  for (const command of commands) {
    const words = command.filter(w => !/^[A-Za-z_][A-Za-z0-9_]*=/.test(w.text));
    let at = 0;
    let inline = false;
    while (at < words.length && runner(words[at]!.text)) {
      at += 1;
      for (; words[at]?.text.startsWith("-") === true; at += 1) inline ||= INLINE.has(words[at]!.text);
    }
    const run = words[at];
    if (run !== undefined && !inline && (at > 0 || !run.text.startsWith("/"))) run.run = true;
  }
  return commands.flat();
}

/** Whether a path, its ways up folded by their letters, is not under the folder; a link on the way is the read's to
 * name, as outside. */
const beyond = (folder: string, path: string): boolean => !path.startsWith(folder.endsWith("/") ? folder : `${folder}/`);

/** What an approval of a run binds, given what a read of its files found: every file inside the folder by its hash,
 * and as unpinned, once each, every script whose name it cannot read, every path after the ones a read on the
 * thread's machine is sent, and each script it may run that lands outside the folder (where no hash is read), from
 * the home folder or through a link out of it, past what one read hashes, or on something not a regular file. No read
 * at all leaves every path it names unpinned. */
function pinOf(folder: string, decl: Extract<SlateRunDecl, { kind: "cmd" }>, read: HashRead | undefined, there: boolean): Pin {
  const named = wordsOf(decl);
  const run = new Set(named.flatMap(w => (w.run ? [w.text] : [])));
  const unpinned = new Map<string, Unpinned["why"]>(named.flatMap(w => (w.loose && (w.run || SCRIPT.test(w.text)) ? [[w.said, "built"] as const] : [])));
  const words = pathsNamed(decl);
  if (read === undefined) for (const path of words) unpinned.set(path, "unread");
  const at = wordsAt(folder, decl, there);
  const folded = there ? posix.normalize : resolve;
  const [outside, past, special] = [new Set(read?.outside ?? []), new Set(read?.past ?? []), new Set(read?.special ?? [])];
  words.forEach((path, i) => {
    const full = at[i]!;
    const script = SCRIPT.test(path) || run.has(path);
    const why = read === undefined ? undefined : there && i >= FS_HASH_PATHS_MAX ? "paths" : !script ? undefined : past.has(full) ? "past" : special.has(full) ? "special" : path.startsWith("~") || beyond(folded(folder), folded(full)) ? "outside" : outside.has(full) ? "link" : undefined;
    if (why !== undefined && !unpinned.has(path)) unpinned.set(path, why);
  });
  const files = read?.files ?? {};
  return { ...(Object.keys(files).length > 0 ? { scripts: files } : {}), unpinned: [...unpinned].map(([path, why]) => ({ path, why })) };
}

/** Every file a run's cmd or then names, read on this disk as a box's daemon reads one there: each regular file under
 * the thread's folder by its path there with a hash of its content, so an "Always" covers the script the person read
 * and an edit to it asks again. A file the command reaches some other way (an import, a glob, a path it builds) is
 * out of reach, and the sheet says so. */
function readHere(folder: string, decl: Extract<SlateRunDecl, { kind: "cmd" }>): HashRead | undefined {
  let root: string;
  try { root = realpathSync(folder); } catch { return undefined; }
  const read: Required<HashRead> = { files: {}, outside: [], past: [], special: [] };
  for (const word of wordsAt(folder, decl, false)) {
    try {
      const at = realpathSync(word);
      const st = statSync(at);
      const rel = relative(root, at);
      if (st.isDirectory() || read.files[rel] !== undefined) continue;
      if (rel.split(sep)[0] === ".." || isAbsolute(rel)) read.outside.push(word);
      else if (!st.isFile()) read.special.push(word);
      else if (st.size > FS_HASH_CAP_BYTES || Object.keys(read.files).length >= FS_HASH_FILES_MAX) read.past.push(word);
      else read.files[rel] = createHash("sha256").update(readFileSync(at)).digest("hex").slice(0, 16);
    } catch { continue; }
  }
  return read;
}

/** What an approval of a run on this computer binds; a folder that is not there holds no file to pin. */
export function pinHere(folder: string | undefined, decl: SlateRunDecl): Pin {
  if (folder === undefined || decl.kind !== "cmd") return { unpinned: [] };
  return pinOf(folder, decl, readHere(folder, decl) ?? { files: {} }, false);
}

/** How long a read of the hashes waits on the thread's computer before the commands there go unpinned. */
const HASH_WAIT_MS = 10_000;

const pinKey = (folder: string, decl: Extract<SlateRunDecl, { kind: "cmd" }>): string => JSON.stringify([folder, wordsAt(folder, decl, true)]);

/** What each command on a thread's own computer names there, read by its daemon before every start and held until the
 * next read, so the approval key, the sheet and the approval all see one reading. A command whose computer could not
 * say (a daemon too old to hash, one that did not answer, a link inside a workspace's folder) has no reading, and an
 * Always cannot hold to it. */
export function boxPins(hashOn: (threadId: string) => HashOn | undefined, waitMs = HASH_WAIT_MS) {
  const read = new Map<string, { at: number; pins: Map<string, HashRead> }>();
  let reads = 0;
  return {
    /** Reads the hashes again for these commands; `asleep` reads nothing, so a napping computer is not woken. */
    async read(threadId: string, cmds: { folder: string | undefined; decl: SlateRunDecl }[], asleep: boolean): Promise<void> {
      const at = ++reads;
      const on = asleep ? undefined : hashOn(threadId);
      const pins = new Map<string, HashRead>();
      const asks = new Map<string, [string, string[]]>();
      for (const { folder, decl } of cmds) if (folder !== undefined && decl.kind === "cmd" && pathsNamed(decl).length > 0) asks.set(pinKey(folder, decl), [folder, wordsAt(folder, decl, true).slice(0, FS_HASH_PATHS_MAX)]);
      if (asks.size === 0) return void read.delete(threadId);
      await Promise.all(
        [...asks].map(async ([key, [folder, paths]]) => {
          if (on === undefined) return;
          let timer: ReturnType<typeof setTimeout> | undefined;
          const late = new Promise<undefined>(done => (timer = setTimeout(done, waitMs, undefined)));
          const got = await Promise.race([on(folder, paths).catch(() => undefined), late]).finally(() => clearTimeout(timer));
          if (got !== undefined) pins.set(key, { ...got, files: Object.fromEntries(Object.entries(got.files).map(([path, sum]) => [path, sum.slice(0, 16)])) });
        }),
      );
      if ((read.get(threadId)?.at ?? 0) < at) read.set(threadId, { at, pins });
    },
    /** What an approval of this command binds, from the last reading; one that names no path needs none. */
    pin(threadId: string, folder: string | undefined, decl: SlateRunDecl): Pin {
      if (folder === undefined || decl.kind !== "cmd") return { unpinned: [] };
      return pinOf(folder, decl, pathsNamed(decl).length === 0 ? { files: {} } : read.get(threadId)?.pins.get(pinKey(folder, decl)), true);
    },
    drop: (threadId: string): void => void read.delete(threadId),
  };
}
