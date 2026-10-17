// SPDX-License-Identifier: AGPL-3.0-only
// node --test apps/desktop-gpui/bench/bench.test.mjs: the parts of the bench that need no screen. The fixture case
// starts a real throwaway host, so it needs the packages built (pnpm build).
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import assert from "node:assert/strict";
import { EVENTS_PER_CALL, EVENTS_PER_TURN, THREADS, lastWords, turnsOf } from "./content.mjs";
import { fps, median, row } from "./table.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const lines = text => text.split("\n").length;

test("median and the slowest 1% of frames", () => {
  assert.equal(median([3, 1, 2]), 2);
  assert.equal(median([4, 1, 3, 2]), 2.5);
  // 200 intervals of 16 ms but one of 100 ms: the median is 62.5 fps. The 100 ms counts as 6 whole refreshes of
  // 16 ms, so the slowest 1%, the 2 slowest, average (96 + 16) / 2 = 56 ms.
  const frames = [0];
  for (let i = 1; i <= 200; i++) frames.push(frames.at(-1) + (i === 50 ? 100 : 16));
  const got = fps(frames);
  assert.equal(got.median, 62.5);
  assert.equal(got.low1, 1000 / 56);
  // Jitter inside one refresh is no slow frame: 15 and 18 ms intervals with a 15 ms median each count as one refresh.
  const jitter = [0];
  for (let i = 1; i <= 201; i++) jitter.push(jitter.at(-1) + (i % 2 === 0 ? 18 : 15));
  assert.equal(fps(jitter).low1, 1000 / 15);
  assert.deepEqual(row("x", { thread_rows: 9, scroll: { px_per_s: 2400, frame_ms: frames }, switch: [{ ms: 30 }, { ms: 10 }, { ms: 20 }] }, 100, 200, 300), ["x", "100", "200", "300", "62.5", "17.9", "20.0", "9"]);
});

test("the generator's mix, the same on every call", () => {
  assert.deepEqual(turnsOf("long"), turnsOf("long"));
  const turns = turnsOf("long");
  assert.equal(turns.length, 900);
  const calls = turns.flatMap(t => t.tools);
  assert.equal(calls.length, 200);
  assert.ok(turns.length * EVENTS_PER_TURN + calls.length * EVENTS_PER_CALL < 5000);
  assert.equal(turns.filter(t => t.reply.includes("```")).length, 225);
  for (const { prompt, reply } of turns) {
    assert.equal(lines(prompt), 1);
    const [prose, code] = reply.split("\n\n");
    assert.ok(lines(prose) >= 2 && lines(prose) <= 8, prose);
    if (code !== undefined) assert.ok(code.startsWith("```") && lines(code) - 2 >= 5 && lines(code) - 2 <= 40, code);
  }
  for (const call of calls) assert.ok(lines(call.output) >= 1 && lines(call.output) <= 30, call.output);
  // The Electron bench knows a switch landed by the new thread's last words, so no two threads may share them.
  const last = Object.keys(THREADS).map(lastWords);
  assert.equal(new Set(last).size, last.length);
});

test("the fixture's long thread is 900 turns, 1,800 messages, held whole by the host", { skip: !existsSync(join(here, "../../../packages/host/dist/bin.js")) && "packages not built" }, () => {
  const tmp = mkdtempSync(join(tmpdir(), "wsp-bench-test-"));
  const out = join(tmp, "fixture.json");
  try {
    const up = spawnSync(join(here, "fixture.sh"), [out], { encoding: "utf8", timeout: 600_000 });
    assert.equal(up.status, 0, `${up.stdout}${up.stderr}`);
    const fixture = JSON.parse(readFileSync(out, "utf8"));
    const home = dirname(dirname(fixture.state));
    const wsp = (...args) => {
      const ran = spawnSync(process.execPath, [join(here, "../../../packages/host/dist/bin.js"), ...args, "--state", fixture.state], { encoding: "utf8", cwd: home, env: { HOME: home, PATH: process.env.PATH }, timeout: 60_000, maxBuffer: 64 << 20 });
      assert.equal(ran.error, undefined);
      assert.equal(ran.status, 0, ran.stderr);
      return ran.stdout;
    };
    const read = name => JSON.parse(wsp("thread", "read", fixture.threads[name], "--json")).messages;
    const turns = turnsOf("long");
    const long = read("long");
    const count = {};
    for (const r of long) count[r.who] = (count[r.who] ?? 0) + 1;
    assert.deepEqual(count, { person: 900, tool: 200, agent: 900, turn: 900 });
    assert.equal(long[0].text, turns[0].prompt);
    assert.deepEqual(long.filter(r => r.who === "agent").map(r => r.text), turns.map(t => t.reply));
    const events = Number(/of (\d+) events/.exec(wsp("thread", "head", fixture.threads.long))?.[1]);
    assert.equal(events, 900 * EVENTS_PER_TURN + 200 * EVENTS_PER_CALL);
    for (const name of ["short-1", "short-2", "short-3"]) assert.equal(read(name).filter(r => r.who === "person" || r.who === "agent").length, 20, name);
  } finally {
    if (existsSync(out)) assert.equal(spawnSync(join(here, "stop.sh"), [out]).status, 0);
    rmSync(tmp, { recursive: true, force: true });
  }
});
