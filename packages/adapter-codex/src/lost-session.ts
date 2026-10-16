// SPDX-License-Identifier: AGPL-3.0-only
// A Codex start that resumes a thread whose rollout the server does not hold,
// as on a new machine the kept file never reached: a new thread handed the
// thread so far, through the same road as Claude's adapter.
import { shellQuote, startAfterLoss } from "@wsp/protocol";
import type { CodexSession, CodexStartOptions } from "./adapter.js";
import { slug } from "./command.js";
import { NEWEST_STATE_DB } from "./session-title.js";

/**
 * One shell line for the guest that says what its home holds for a thread a resume could not open: every rollout file
 * whose name carries the thread id, live or archived, with its size; the thread's row in the highest-versioned state
 * db and whether the file it names is there, since 0.163.0 takes a paginated thread's row as final and answers "no
 * rollout found" when that file is gone; and the boot id, which says whether the machine came back from a stop. Paths
 * and ids alone, read-only. history_mode is a 0.163.0 column, so an older db is asked for the path alone.
 */
export function sessionHeldCommand(options: { home: string; threadId: string }): string {
  const id = slug("threadId", options.threadId);
  const row = (columns: string): string => shellQuote(`select json_object(${columns}) from threads where id = '${id}' limit 1;`);
  return [
    `echo "boot $(cat /proc/sys/kernel/random/boot_id 2>/dev/null)"`,
    `cd ${shellQuote(options.home)} 2>/dev/null || { echo nohome; exit 0; }`,
    `find sessions archived_sessions -type f -name ${shellQuote(`*${id}*`)} 2>/dev/null | while IFS= read -r f; do echo "file $(stat -c %s "$f" 2>/dev/null || stat -f %z "$f") $PWD/$f"; done`,
    `echo listed`,
    `command -v sqlite3 > /dev/null 2>&1 || { echo nosqlite; exit 0; }`,
    `${NEWEST_STATE_DB}; [ -n "$d" ] || { echo nodb; exit 0; }`,
    `r=$(sqlite3 -readonly "$d" ${row("'path', rollout_path, 'mode', history_mode")} 2>/dev/null) || r=$(sqlite3 -readonly "$d" ${row("'path', rollout_path")} 2>&1) || { echo "unread $d $(printf '%s' "$r" | head -n 1 | cut -c 1-160)"; exit 0; }`,
    `[ -n "$r" ] || { echo "norow $d"; exit 0; }`,
    `echo "row $d $r"`,
    `p=$(printf '%s' "$r" | sed -n 's/.*"path":"\\([^"]*\\)".*/\\1/p'); if [ -e "$p" ] || [ -e "$p.zst" ]; then echo named there; else echo named missing; fi`,
  ].join("\n");
}

/** That reading as one log line's words: the boot, the files, the row and whether the file it names is there. */
export function parseSessionHeld(stdout: string): string {
  const lines = stdout.split("\n").map(line => line.trim());
  const boot = lines.find(line => line.startsWith("boot "))?.slice(5).trim() || "unread";
  const files = lines.filter(line => line.startsWith("file ")).map(line => {
    const [, size, ...path] = line.split(" ");
    return `${path.join(" ")} (${size} bytes)`;
  });
  const said = (word: string): boolean => lines.includes(word);
  const row = lines.find(line => line.startsWith("row ") || line.startsWith("norow ") || line.startsWith("unread "));
  const index = said("nohome")
    ? "no Codex home"
    : said("nosqlite")
      ? "not read, no sqlite3"
      : said("nodb")
        ? "no state db"
        : row === undefined
          ? "not read"
          : row.startsWith("unread ")
            ? `not read from ${row.slice(7)}`
            : row.startsWith("norow ")
              ? `no row in ${row.slice(6)}`
              : `${row.slice(4)}, the file it names ${said("named there") ? "is there" : "is missing"}`;
  const listed = said("listed") || said("nohome");
  return `boot ${boot}; rollout files: ${files.length === 0 ? "none" : files.join(", ")}${listed ? "" : ", the listing cut short"}; index row: ${index}`;
}

/** What a server answers a resume of a thread its home holds no rollout file for, as 0.162.1 words it. */
const noRolloutLine = (threadId: string): string => `codex could not open the thread: no rollout found for thread id ${threadId}`;

/** A launch, and where it resumes a thread whose rollout the server does not hold and a seed is at hand, a second
 * launch on a new thread handed the thread so far; the session answered is the one running at the moment. */
export function startSeeded(launchTurn: (options: CodexStartOptions) => CodexSession, sessions: Map<string, CodexSession>, options: CodexStartOptions): CodexSession {
  const { resume, seed } = options;
  if (resume === undefined || seed === undefined || options.copy === true || options.fork !== undefined) return launchTurn(options);
  const { first, now, finished } = startAfterLoss({ ...options, resume, seed }, noRolloutLine(resume), launchTurn);
  const session: CodexSession = {
    localId: first.localId,
    get threadId() {
      return now().threadId;
    },
    get command() {
      return now().command;
    },
    get run() {
      return now().run;
    },
    get pid() {
      return now().pid;
    },
    get from() {
      return now().from;
    },
    finished,
    interrupt: () => now().interrupt(),
    kept: () => now().kept?.(),
    steer: prompt => now().steer(prompt),
    answer: (askId, answer) => now().answer(askId, answer),
    stopTask: task => now().stopTask(task),
  };
  sessions.set(first.localId, session);
  return session;
}
