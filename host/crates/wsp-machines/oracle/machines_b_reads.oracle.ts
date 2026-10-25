// SPDX-License-Identifier: AGPL-3.0-only
// Runs sub-life.ts's pure functions on the inputs machines-b's Rust tests replay and writes what they answer.
import { writeFileSync } from "node:fs";
import { it } from "vitest";
import { subPreamble, subResumePreamble } from "@wsp/protocol";
import { subResultCommand, subResultOf, subSessionPackCommand, subSessionLandCommand, subRowOf, diskKeptUntil, diskKeptMs } from "../../../../packages/runtime/src/machines/sub-life.js";

const OUT = new URL("../tests/fixtures/machines_b/", import.meta.url);
const AT = "2026-10-14T12:00:00.000Z";
const B = "wsp/fix-the-flaky-upload-test";

it("writes sub_life_reads.json", () => {
  const outputs = [
    "branch wsp/x\npushed 9c2e1d4b7a0f3e8c5d6a1b2c3d4e5f6a7b8c9d0e\nunpushed 0\nchanged 0\ncommits 3\n",
    "branch wsp/x\npushed none\nunpushed 2\nchanged 3\n",
    "norepo\n",
    "branch wsp/x\npushed none\nunpushed 2\nchanged 3\nnorepo\n",
    "",
    "branch wsp/x\npushed none\nunpushed\nchanged 3\n",
    "branch wsp/x\npushed none\nunpushed 2\n",
    "  branch   HEAD  \n\n pushed  \n unpushed 0x10 \n changed 3.0\ncommits -0\n",
    "branch\npushed none\nunpushed 1e1\nchanged 0\ncommits abc\n",
    "unpushed -1\nchanged 0\n",
    "unpushed 1.5\nchanged 0\n",
    "unpushed 1\nunpushed 4\nchanged 0\npushed abc\npushed none\n",
    "unpushed 1\nchanged 0\ncommits\n",
    "unpushed\t1\nchanged 0\n",
    "unpushed 1\r\nchanged 0\r\nbranch main\r\n",
    "unpushed  7 \nchanged 0\nbranch feat x y\n",
    "unpushed 1\nchanged  2\n",
  ];
  const parsed = outputs.map(stdout => ({ stdout, result: subResultOf(stdout, B, AT) ?? null }));
  const commands = [
    { folder: "/root/web", branch: B, baseSha: "4be1c0a9d2c4e1f07a3b9e5d6c8f2a1b0e9d7c6a" },
    { folder: "/root/it's here", branch: "wsp/a'b", baseSha: "abc" },
  ].map(c => ({ ...c, command: subResultCommand(c.folder, c.branch, c.baseSha) }));
  const files = [
    { folder: "/root/.claude-cfg/projects", name: "s1.jsonl", depth: 1 },
    { folder: "/root/.codex/sessions", name: "-s1.jsonl", depth: 3 },
  ];
  const packs = files.map(file => ({ file, to: "/tmp/wsp-session-t1.gz", command: subSessionPackCommand(file, "/tmp/wsp-session-t1.gz") }));
  const lands = [
    { file: files[0]!, path: "/root/.claude-cfg/projects/-root-web/s1.jsonl", from: "/tmp/wsp-session-t1.gz" },
    { file: files[1]!, path: "/root/.codex/sessions/2026/10/12/rollout-2026-10-12T06-00-00-s1.jsonl", from: "/tmp/wsp-session-t1.gz" },
    { file: files[0]!, path: "nofolder.jsonl", from: "/tmp/x.gz" },
  ].map(l => ({ ...l, command: subSessionLandCommand(l.file, l.path, l.from) }));
  const base = { id: "ws_1", name: "Fix it", kind: "cloud", machineId: "m1", createdAt: AT, project: "pr_1", spec: {}, size: { cpu: 2, memMb: 4096 } };
  const sub = { on: "boat", repo: "https://github.com/acme/web", base: { branch: "feat/upload", sha: "abc" }, branch: B, folder: "/root/web" };
  const result = { branch: B, pushedSha: null, commits: 0, unpushed: 2, changed: 3, readAt: AT };
  const records: Record<string, unknown>[] = [
    { ...base, phase: "running" },
    { ...base, phase: "running", sub },
    { ...base, phase: "running", sub: { ...sub, spawn: { startedAt: 1, log: [] } } },
    { ...base, phase: "running", sub: { ...sub, spawn: { startedAt: 1, readyAt: 2, log: [] } } },
    { ...base, phase: "running", sub: { ...sub, spawn: { startedAt: 1, failed: { step: "machine", said: "x" }, log: [] } } },
    { ...base, phase: "queued", sub },
    { ...base, phase: "queued", queue: { since: AT, why: "slot" }, sub },
    { ...base, phase: "napping", sub: { ...sub, rateUsdPerHour: 0.05, sizeName: "small", result, held: { files: 3, commits: 2, at: 5 }, deleteAt: 7 } },
    { ...base, phase: "released", sub: { ...sub, releasedAt: 9, cloudGone: true } },
    { ...base, phase: "gone", sub },
    { ...base, phase: "creating", sub },
    { ...base, phase: "running", sub: { ...sub, session: { agent: "claude", id: "s1", kept: null } } },
    { ...base, phase: "running", sub: { ...sub, session: { agent: "claude", id: "s1", kept: null, why: "the pack exited 1: no" } } },
    { ...base, phase: "running", sub: { ...sub, session: { agent: "claude", id: "s1", kept: null, bytes: 64 * 1024 * 1024, why: "w" } } },
    { ...base, phase: "running", sub: { ...sub, session: { agent: "claude", id: "s1", kept: null, bytes: 64 * 1024 * 1024 + 1, why: "w" } } },
    { ...base, phase: "running", sub: { ...sub, session: { agent: "claude", id: "s1", kept: 5, bytes: 9, path: "/x" } } },
    { ...base, phase: "running", sub: { ...sub, cloudGone: false } },
  ];
  const rows = records.map(record => ({ record, row: subRowOf(record as never) ?? null }));
  const ctxOf = (ms: number | undefined | "throws" | "nolife") => ({
    backendFor: () => {
      if (ms === "throws") throw new Error("no kind");
      return ms === "nolife" ? {} : { lifecycle: ms === undefined ? {} : { diskKeptMs: ms } };
    },
  });
  const keeps = ([
    [{ phase: "napping", nappedAt: 1000 }, 50, undefined],
    [{ phase: "napping", nappedAt: 1000 }, 50, 2000],
    [{ phase: "napping" }, 50, 2000],
    [{ phase: "napping" }, 50, undefined],
    [{ phase: "running", nappedAt: 1000 }, 50, 2000],
    [{ phase: "running" }, 50, undefined],
    [{ phase: "napping", nappedAt: 1000 }, 0, undefined],
    [{ phase: "napping", nappedAt: 1000 }, undefined, undefined],
    [{ phase: "napping", nappedAt: 1000 }, "nolife", undefined],
    [{ phase: "napping", nappedAt: 1000 }, "throws", undefined],
    [{ phase: "running" }, 50, 0],
  ] as const).map(([record, ms, stoppedAt]) => ({
    record,
    diskKeptMs: ms === undefined ? null : ms,
    stoppedAt: stoppedAt ?? null,
    keptMs: diskKeptMs(ctxOf(ms) as never, record as never) ?? null,
    until: diskKeptUntil(ctxOf(ms) as never, record as never, stoppedAt) ?? null,
  }));
  // The git lines the two sub-result-read cases run, with the repo and the commits left as {REPO}, {HEAD}, {START}.
  const SIZE = { name: "small", cpu: 2, memMb: 4096, rateUsdPerHour: 0.05 };
  const gitLines = (preamble: string): string[] => preamble.split("\n").filter(l => l.startsWith("git "));
  const preambles = {
    brokered: gitLines(subPreamble({ on: "Boat", size: SIZE, lead: "0123456789abcdef", folder: "/root/web", sent: [], setupGit: "", repo: "{REPO}", base: { branch: "acme-pr", sha: "{HEAD}" }, branch: B, deleteAfterMinutes: 30, brokered: true })),
    resumed: gitLines(subResumePreamble({ on: "Boat", size: SIZE, folder: "/root/web", sent: [], setupGit: "", repo: "{REPO}", base: { branch: "main", sha: "{START}" }, branch: B, pushed: true, deleteAfterMinutes: 30, brokered: true })),
  };
  writeFileSync(new URL("sub_life_reads.json", OUT), JSON.stringify({ parsed, commands, packs, lands, rows, keeps, preambles }, null, 2) + "\n");
});
