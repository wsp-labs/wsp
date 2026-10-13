// SPDX-License-Identifier: AGPL-3.0-only
// A Claude Code turn this host does not hold, as a Mac's daemon reads it: the
// native binary named by its version, under its run script, under another
// wsp's app and daemon, which listen; with two tool servers and the python each
// runs, the script down stamped by the WSP_THREAD mark of that wsp's thread.
// Beside it a Codex npm launcher and the native codex it runs, of a second such
// thread, and the person's own Claude Code, which no wsp started. The command
// line's table and the Activity page are each tested against it.
import type { ProcEntry, ProcKind } from "../../src/wire/daemon.js";

export const OUTSIDE_START = 1_760_000_000_000;
const MB = 1024 * 1024;
const OTHER = { by: "env", thread: "th_other" } as const;
const THIRD = { by: "env", thread: "th_third" } as const;

const row = (pid: number, ppid: number, comm: string, cmdline: string, mb: number, extra: Partial<ProcEntry> = {}): ProcEntry => ({ pid, ppid, user: "dev", state: "S", comm, cmdline, cpu: 0, rss: mb * MB, startedAt: OUTSIDE_START + pid, ...extra });

export const OUTSIDE_PROCS: ProcEntry[] = [
  row(1, 0, "launchd", "/sbin/launchd", 10),
  row(10, 1, "node", "node wsp host", 200),
  row(20, 10, "wsp-daemon", "wsp-daemon serve", 4),
  row(400, 1, "Electron", "/Applications/wsp.app/Contents/MacOS/wsp", 300),
  row(410, 400, "wsp-daemon", "/Applications/wsp.app/Contents/Resources/daemon/wsp-daemon serve", 6),
  row(450, 410, "bash", "bash /home/dev/wsp-work/state/runs/0a1b2c3d4e5f.sh", 2, { owner: OTHER }),
  row(500, 450, "2.1.296", "claude -p --input-format stream-json --output-format stream-json --verbose --model claude-opus-5-5", 640, { cpu: 3, owner: OTHER }),
  row(510, 500, "uv", "/opt/homebrew/bin/uv tool uvx mcp-search-console", 30, { owner: OTHER }),
  row(511, 510, "python3.13", "/home/dev/.cache/uv/archive-v0/djbB/bin/python /home/dev/.cache/uv/archive-v0/djbB/bin/mcp-search-console", 64, { owner: OTHER }),
  row(520, 500, "node", "node /home/dev/.npm/_npx/9f1/node_modules/.bin/mcp-server-github", 80, { owner: OTHER }),
  row(530, 500, "zsh", "/bin/zsh -c ls", 3, { owner: OTHER }),
  row(600, 1, "node", "node /opt/homebrew/bin/codex exec --json -", 40, { owner: THIRD }),
  row(601, 600, "codex", "/opt/homebrew/lib/node_modules/@openai/codex/vendor/aarch64-apple-darwin/codex/codex exec --json -", 210, { owner: THIRD }),
  row(700, 1, "2.1.296", "claude --resume", 420),
];

/** The other wsp's app and daemon listen, so the rules below a port read everything under them as a dev server. */
export const OUTSIDE_PORTS = new Map<number, number[]>([
  [400, [4400, 4420]],
  [410, [58815]],
]);

/** No thread as both readers draw it, each row's depth, name and kind: no agent outside a thread folds. */
export const OUTSIDE_NO_THREAD: readonly (readonly [number, string, ProcKind])[] = [
  [0, "bash", "dev"],
  [1, "Claude Code", "agent"],
  [2, "mcp-search-console", "tool"],
  [3, "mcp-search-console", "other"],
  [2, "mcp-server-github", "tool"],
  [2, "zsh", "other"],
  [0, "Codex", "agent"],
  [1, "Codex", "agent"],
];
