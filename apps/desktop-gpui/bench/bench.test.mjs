// SPDX-License-Identifier: AGPL-3.0-only
// node --test apps/desktop-gpui/bench/bench.test.mjs: the parts of the bench that need no screen. The fixture case
// starts a real throwaway host, so it needs the packages built (pnpm build).
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import assert from "node:assert/strict";
import { EVENTS_PER_CALL, EVENTS_PER_TURN, THREADS, eventsOf, lastWords, renderedLines, turnsOf } from "./content.mjs";
import { MOMENTUM_FLOOR, SPACING, momentum } from "./momentum.mjs";
import { OUTPUT_MAX, shot } from "./real.mjs";
import { fps, median, row } from "./table.mjs";
import { describe, slowFrames, unfit } from "./trace.mjs";
import * as ctrlTab from "./ctrl-tab.mjs";
import { seededState } from "./seed.mjs";
import { sessionNames, sessionPlan } from "./session.mjs";

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
  // One refresh for both apps: a 24 ms hitch in a run whose median is 16.67 ms is 3 refreshes of 8.33 ms, as it is in
  // a run at 120 fps, not 1 refresh of its own median.
  const at60 = [0];
  for (let i = 1; i <= 200; i++) at60.push(at60.at(-1) + (i === 50 || i === 150 ? 24 : 1000 / 60));
  assert.equal(Math.round(fps(at60).low1), 60);
  assert.equal(Math.round(fps(at60, 1000 / 120).low1), 40);
  assert.deepEqual(row("x", { thread_rows: 9, scroll: { px_per_s: 2400, frame_ms: frames }, switch: [{ ms: 30 }, { ms: 10 }, { ms: 20 }] }, 100, 200, 300), ["x", "100", "200", "300", "62.5", "17.9", "20.0", "-", "-", "-", "9"]);
  // The GPUI bench's 30 timed switches show as their median, 90th percentile and slowest.
  const switches = { each: [], median_ms: 41.5, p90_ms: 60.25, max_ms: 72 };
  assert.deepEqual(row("x", { thread_rows: 9, scroll: { px_per_s: 2400, frame_ms: frames }, switch: [{ ms: 30 }], switches }, 100, 200, 300).slice(6), ["30.0", "41.5", "60.3", "72.0", "9"]);
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

test("the real thread is shaped like a person's long one, under the host's caps", () => {
  assert.deepEqual(turnsOf("real"), turnsOf("real"));
  const turns = turnsOf("real");
  const calls = turns.flatMap(t => t.tools);
  const lines = renderedLines(turns);
  assert.ok(lines >= 6500 && lines <= 7500, `${lines} rendered lines`);
  // Under the host's 5,000 events and 4 MB, and over a dozen pages of 200 events, so pages land while it scrolls.
  const events = eventsOf(turns);
  assert.ok(events < 4000 && events > 12 * 200, `${events} events`);
  assert.ok(JSON.stringify(turns).length < 3 * 1024 * 1024);
  assert.ok(calls.every(c => c.output.length <= OUTPUT_MAX));
  const images = turns.flatMap(t => t.images);
  assert.ok(images.length >= 20 && images.some(i => i.width >= 2560), `${images.length} screenshots`);
  assert.ok(turns.filter(t => t.prompt.length > 4096).length >= 5, "long pastes past 4 KB");
  assert.ok(turns.filter(t => t.reply.length > 4096).length >= 5, "replies past 4 KB");
  assert.ok(new Set(turns.flatMap(t => [...t.reply.matchAll(/```(\w+)/g)].map(m => m[1]))).size >= 12, "code in many languages");
  assert.ok(turns.some(t => t.reply.includes("| --- |")), "tables");
  assert.equal(calls.filter(c => c.children !== undefined).length, THREADS.real.tasks);
  assert.ok(calls.some(c => c.error === true && c.output.split("\n").length > 40), "failing calls with long output stand alone");
  const changing = turns.filter(t => t.files.length > 0);
  assert.equal(changing.length, THREADS.real.changes);
  assert.ok(changing.every(t => t.files.length >= 100 && t.files.every(f => f.path.split("/").length >= 4)), "changed-files cards of 100 files and more, in nested folders");
  const { width, height, channels, pixels } = shot(images[0]);
  assert.equal(pixels.length, width * height * channels);
  assert.deepEqual(shot(images[0]).pixels.subarray(0, 64), pixels.subarray(0, 64));
});

test("the momentum replay scrolls as a trackpad does: gestures, momentum, catches, reversals, mostly up", () => {
  const events = momentum();
  assert.deepEqual(momentum(), events);
  assert.ok(events.at(-1).t <= 60_000 && events.at(-1).t > 55_000);
  assert.ok(events.every((e, i) => i === 0 || e.t >= events[i - 1].t));
  assert.ok(events.every(e => e.unit === "px" && Math.abs(e.dx) <= Math.abs(e.dy) * 0.06 + 0.31), "fingers drift sideways a little");
  assert.ok(events.some(e => e.dx !== 0));
  // Each gesture: Started, Moved at the trackpad's spacing, Ended, then momentum Moved decaying under the floor
  // unless a new gesture catches it.
  const gestures = [];
  for (const e of events) {
    if (e.phase === "started") gestures.push([]);
    gestures.at(-1).push(e);
  }
  assert.equal(gestures.length, events.filter(e => e.phase === "started").length);
  let caught = 0;
  let decayed = 0;
  for (const [i, g] of gestures.entries()) {
    const end = g.findIndex(e => e.phase === "ended");
    if (end < 0 && i === gestures.length - 1) continue;
    assert.ok(end > 3, "fingers move before they lift");
    assert.ok(g.slice(1, end).every(e => e.phase === "moved"));
    for (let k = 1; k <= end; k++) assert.ok(Math.abs(g[k].t - g[k - 1].t - SPACING) < 0.02, `spacing at ${g[k].t}`);
    const tail = g.slice(end + 1).map(e => Math.abs(e.dy));
    assert.ok(tail.every((v, k) => k === 0 || v <= tail[k - 1]), "momentum only decays");
    if (tail.length > 0 && i + 1 < gestures.length && gestures[i + 1][0].t - g.at(-1).t <= 30 + SPACING) caught++;
    else if (tail.length > 0) {
      decayed++;
      assert.ok(tail.at(-1) < MOMENTUM_FLOOR / 0.955, `momentum ended at ${tail.at(-1)} px`);
    }
  }
  assert.ok(caught >= 1 && decayed >= 10, `${caught} caught, ${decayed} decayed`);
  const up = gestures.filter(g => g[1].dy > 0).length;
  assert.ok(up > gestures.length * 0.7 && up < gestures.length, `${up} of ${gestures.length} gestures up`);
  assert.ok(events.reduce((n, e) => n + e.dy, 0) > 20_000, "the replay pages back from the end");
  assert.ok(gestures.some((g, i) => i > 0 && g[0].t - gestures[i - 1].at(-1).t > 1500), "a reading pause");
});

test("the trace reader finds every frame over the limit and refuses a trace that cannot stand", () => {
  const frame = (at, draw, present, late, more = {}) => ({ frame: 0, at, draw, present, since: 16, late, inputs: 1, focused: true, rows: 0, row_ms: 0, parsed: 0, parse_ms: 0, images: 0, pages: [], view_ms: 0, input_ms: 0, app: draw, ...more });
  const trace = { release: true, limit_ms: 50, frames: [frame(0, 10, 5, 20), frame(16, 30, 25, 40), frame(32, 5, 5, 70), frame(48, 45, 2, 10, { pages: [{ events: 200, ms: 8 }], app: 53 })] };
  assert.deepEqual(slowFrames(trace).map(f => f.at), [16, 32]);
  assert.deepEqual(slowFrames(trace, { app: true }).map(f => f.at), [48], "a page before a 45 ms draw is the UI thread's too");
  assert.match(describe(trace.frames[3]), /app 53\.0 ms.*1 page landed \(200 events in 8\.0 ms\)/);
  assert.equal(unfit(trace), null);
  assert.match(unfit({ ...trace, release: false }), /debug build/);
  assert.match(unfit({ ...trace, frames: [{ ...trace.frames[0], focused: false }] }), /lost focus/);
  assert.match(unfit({ ...trace, frames: [] }), /no frames/);
  const tmp = mkdtempSync(join(tmpdir(), "wsp-trace-test-"));
  try {
    const run = frames => {
      writeFileSync(join(tmp, "t.json"), JSON.stringify({ ...trace, mode: "replay", thread: "t", scale: 2, inputs: [], frames }));
      return spawnSync(process.execPath, [join(here, "trace.mjs"), join(tmp, "t.json")], { encoding: "utf8", env: { PATH: "/usr/bin:/bin" } });
    };
    const slow = run(trace.frames);
    assert.equal(slow.status, 1, slow.stdout);
    assert.match(slow.stdout, /2 frames over 50 ms/);
    const quick = run([frame(0, 10, 5, 20)]);
    assert.equal(quick.status, 0, quick.stdout);
    assert.match(quick.stdout, /no frame over 50 ms/);
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
});

test("the ctrl-tab reader names every step that opened nothing or missed the first frame, and refuses a debug trace", () => {
  const step = (n, from, to, frames, within) => ({ n, held: n > 0, sent_ms: n * 30, from, to, presented_ms: n * 30 + 9, rows_ms: n * 30 + 9 + frames * 16, latency_ms: 9, rows_latency_ms: 9 + frames * 16, frames_before_rows: frames, within_frame: within });
  const spread = { median_ms: 9, p90_ms: 25, max_ms: 25 };
  const trace = {
    release: true,
    frame_ms: 16.7,
    repeat: { delay_ms: 225, every_ms: 30 },
    steps: [step(0, "a", "b", 0, true), step(1, "b", "c", 1, false), step(2, "c", "c", 0, false)],
    summary: { steps: 3, opened: 2, threads: 2, within_frame: 1, key_to_frame: spread, key_to_rows: spread },
  };
  assert.deepEqual(ctrlTab.lateSteps(trace).map(s => s.n), [1, 2]);
  assert.match(ctrlTab.describe(trace.steps[1]), /repeat .*key to next frame +9\.0 ms, rows +25\.0 ms  LATE by 1 frame$/);
  assert.equal(ctrlTab.unfit(trace), null);
  assert.match(ctrlTab.unfit({ ...trace, release: false }), /debug build/);
  assert.match(ctrlTab.unfit({ ...trace, steps: [] }), /no steps/);
  const tmp = mkdtempSync(join(tmpdir(), "wsp-ctrl-tab-test-"));
  try {
    const run = steps => {
      writeFileSync(join(tmp, "t.json"), JSON.stringify({ ...trace, steps }));
      return spawnSync(process.execPath, [join(here, "ctrl-tab.mjs"), join(tmp, "t.json")], { encoding: "utf8", env: { PATH: "/usr/bin:/bin" } });
    };
    const late = run(trace.steps);
    assert.equal(late.status, 1, late.stdout);
    assert.match(late.stdout, /2 steps late or opening nothing/);
    const quick = run([trace.steps[0]]);
    assert.equal(quick.status, 0, quick.stdout);
    assert.match(quick.stdout, /every step opened its thread within one frame/);
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
});

test("the seeded host is shaped like a long-used one, and the same on every call", () => {
  const asked = { threads: 200, workspaces: 20, dir: "/seed", now: 1_800_000_000_000 };
  const state = seededState(asked);
  const entries = Object.values(state.sessions);
  const rows = entries.flatMap(e => e.sessions);
  assert.equal(rows.length, 200);
  assert.equal(Object.keys(state.workspaces).length, 20);
  assert.ok(rows.every(r => r.prompt.length <= 600 && state.workspaces[r.workspaceId]), "each row's opening words fit OPENING_KEPT, on a workspace that exists");
  const settled = entries.flatMap(e => Object.values(e.threads)).filter(t => t.settledAt).length;
  assert.ok(settled > 150 && settled < 190, `${settled} of 200 threads settled`);
  const leads = new Set(Object.values(state.transcripts).flatMap(t => t.events).filter(e => e.type === "session.subagent").map(e => e.threadId));
  assert.ok(leads.size > 40 && leads.size < 100, `${leads.size} of 200 threads ran subagents`);
  assert.deepEqual(seededState(asked), state);
});

test("the session threads come only when asked, 1 to 200 of them, 12 to 40 turns each", () => {
  assert.deepEqual(sessionNames(undefined), []);
  assert.deepEqual(sessionNames("3"), ["session-1", "session-2", "session-3"]);
  assert.throws(() => sessionNames("201"));
  assert.throws(() => sessionNames("two"));
  assert.equal(sessionPlan("session-0"), undefined);
  assert.equal(sessionPlan("long"), undefined);
  assert.ok(sessionNames("25").every(name => sessionPlan(name).turns >= 12 && sessionPlan(name).turns <= 40));
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
    // The real thread whole on the host, none of it dropped by its caps, its screenshots kept with their messages.
    const real = turnsOf("real");
    assert.equal(read("real").filter(r => r.who === "person").length, real.length);
    const held = Number(/of (\d+) events/.exec(wsp("thread", "head", fixture.threads.real))?.[1]);
    assert.ok(held >= eventsOf(real) && held < 5000, `${held} events held of the real thread`);
    const cards = read("real").filter(r => r.who === "tool" && /^Changed \d+ files/.test(r.text));
    assert.equal(cards.length, THREADS.real.changes, "every turn that wrote files ends with what it changed");
  } finally {
    if (existsSync(out)) assert.equal(spawnSync(join(here, "stop.sh"), [out]).status, 0);
    rmSync(tmp, { recursive: true, force: true });
  }
});
