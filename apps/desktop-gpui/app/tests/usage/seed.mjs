// SPDX-License-Identifier: AGPL-3.0-only
// node seed.mjs <state-dir>: writes Settings > Usage's data straight into a throwaway host's <state-dir>/state.db,
// running or stopped. Thirty days of ledger rows over three agents, five accounts, three computers, the host's
// projects and no project, six models (one unpriced, one with a [1m] suffix), wsp and log rows; the accounts the
// Limits tab names in every state it draws; a price table read now. Every write is an insert or replace, and each
// day's rows come off a fixed seed and the day's distance from today, so a second run gives the same page.
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

// node:sqlite warns that it is experimental on every load, which would bury the one-line summary.
process.removeAllListeners("warning");
const { DatabaseSync } = await import("node:sqlite");

const dir = process.argv[2];
if (dir === undefined || !existsSync(join(dir, "state.db"))) {
  console.error("usage: node seed.mjs <state-dir>  (the folder holding state.db, which `wsp up --state <dir>/state.json` makes)");
  process.exit(2);
}

const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;
const DAYS = 30;
// Minutes, not ms: two runs a minute apart read the same resets and the same "as of".
const now = Math.floor(Date.now() / MIN) * MIN;
const nowDate = new Date(now);
const hourNow = nowDate.getHours();

// The host files rows in its own zone, which is TZ or /etc/localtime: run this with the host's TZ.
const dayKey = d => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
const dayBack = back => new Date(nowDate.getFullYear(), nowDate.getMonth(), nowDate.getDate() - back, 12);

function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4_294_967_296;
  };
}
const SEED = 17;

const PRICES = {
  "claude-opus-5-5": { input: 5e-6, output: 2.5e-5, cacheRead: 5e-7, cacheWrite: 6.25e-6, provider: "anthropic" },
  "claude-sonnet-5-5": { input: 3e-6, output: 1.5e-5, cacheRead: 3e-7, cacheWrite: 3.75e-6, provider: "anthropic" },
  "claude-haiku-5-5": { input: 1e-6, output: 5e-6, cacheRead: 1e-7, cacheWrite: 1.25e-6, provider: "anthropic" },
  "gpt-5.6-sol": { input: 1.25e-6, output: 1e-5, cacheRead: 1.25e-7, provider: "openai" },
  "gpt-5.5": { input: 1.25e-6, output: 1e-5, cacheRead: 1.25e-7, provider: "openai" },
};
const UNPRICED = "kimi-k3";

const db = new DatabaseSync(join(dir, "state.db"));
db.exec("pragma busy_timeout = 10000");

async function hostProjects() {
  try {
    const port = JSON.parse(readFileSync(join(dir, "host.lock"), "utf8")).port;
    const token = readFileSync(join(dir, "host-token"), "utf8").trim();
    const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
    return await new Promise((resolve, reject) => {
      const give = setTimeout(() => reject(new Error("no answer")), 5_000);
      ws.onerror = () => reject(new Error("no socket"));
      ws.onopen = () => ws.send(JSON.stringify({ id: 1, op: "auth", token }));
      ws.onmessage = e => {
        const m = JSON.parse(String(e.data));
        if (m.ok !== true) reject(new Error(m.error));
        else if (m.id === 1) ws.send(JSON.stringify({ id: 2, op: "projects.list" }));
        else if (m.id === 2) {
          clearTimeout(give);
          ws.close();
          resolve(m.projects.map(p => ({ id: p.id, name: p.name })));
        }
      };
    });
  } catch {
    return db
      .prepare("select json from collections where collection = 'projects'")
      .all()
      .map(r => JSON.parse(r.json))
      .map(p => ({ id: p.id, name: p.name }));
  }
}
const projects = await hostProjects();
const real = projects.map(p => p.id).sort();
const proj = i => (real.length === 0 ? "" : real[i % real.length]);

// One template a stream of work: tokens is a full weekday's total, in and out.
const STREAMS = [
  { agent: "claude", account: "claude:vault-token", computer: "here", project: proj(0), model: "claude-opus-5-5", source: "wsp", tokens: 140e6, cached: 0.92, write: 0.03, out: 0.006 },
  { agent: "claude", account: "claude:vault-token", computer: "here", project: proj(1), model: "claude-opus-5-5[1m]", source: "wsp", tokens: 48e6, cached: 0.9, write: 0.04, out: 0.005, on: back => back % 3 !== 1 },
  { agent: "claude", account: "claude:vault-key", computer: "studio", project: "", model: "claude-sonnet-5-5", source: "wsp", tokens: 32e6, cached: 0.86, write: 0.05, out: 0.01, reports: true, weekdays: true },
  { agent: "claude", account: "claude:vault-key", computer: "here", project: proj(0), model: "claude-haiku-5-5", source: "wsp", tokens: 4e6, cached: 0.7, write: 0.08, out: 0.02 },
  { agent: "claude", account: "claude:vault-token", computer: "boat", project: "", model: "claude-opus-5-5", source: "log", tokens: 26e6, cached: 0.91, write: 0.03, out: 0.007, on: back => back % 4 !== 2 },
  { agent: "codex", account: "codex:acct_plus", computer: "here", project: proj(0), model: "gpt-5.6-sol", source: "wsp", tokens: 62e6, cached: 0.84, write: 0, out: 0.012, reasoning: 0.4 },
  { agent: "codex", account: "codex:acct_pro", computer: "boat", project: proj(1), model: "gpt-5.5", source: "wsp", tokens: 36e6, cached: 0.8, write: 0, out: 0.015, reasoning: 0.3, on: back => back < 18 },
  { agent: "codex", account: "codex:acct_plus", computer: "studio", project: "", model: "gpt-5.6-sol", source: "log", tokens: 15e6, cached: 0.82, write: 0, out: 0.012, reasoning: 0.4, weekdays: true },
  { agent: "opencode", account: "opencode@here", computer: "here", project: "", model: UNPRICED, source: "wsp", tokens: 7e6, cached: 0.6, write: 0, out: 0.03, on: back => back % 5 !== 3 },
];

const priceOf = (model, t) => {
  const r = PRICES[model.replace(/\[[^\]]*\]$/, "")];
  return r === undefined ? undefined : (t.input - t.cached - t.cacheWrite) * r.input + t.cached * (r.cacheRead ?? r.input) + t.cacheWrite * (r.cacheWrite ?? r.input) + t.output * r.output;
};

function dayRows(back) {
  const date = dayBack(back);
  const day = dayKey(date);
  const weekend = date.getDay() === 0 || date.getDay() === 6;
  // A month that climbs toward today, quiet weekends and one heavy release week two weeks back; today keeps every
  // stream, so the day range has every split value whatever the weekday.
  const trend = 0.62 + 0.38 * (1 - back / (DAYS - 1));
  const push = back >= 12 && back <= 15 ? 1.7 : 1;
  const hours = back === 0 ? Array.from({ length: hourNow + 1 }, (_, h) => h) : Array.from({ length: 15 }, (_, h) => h + 8);
  const rows = [];
  for (const [i, s] of STREAMS.entries()) {
    const r = rng(SEED * 1_000 + back * 31 + i);
    if (s.on !== undefined && !s.on(back)) continue;
    if (s.weekdays === true && weekend && back > 0) continue;
    const total = s.tokens * trend * push * (weekend ? 0.35 : 1) * (0.7 + 0.6 * r()) * (back === 0 ? Math.max(0.2, (hourNow + 1) / 24) : 1);
    const shuffled = [...hours];
    for (let k = shuffled.length - 1; k > 0; k--) {
      const m = Math.floor(r() * (k + 1));
      [shuffled[k], shuffled[m]] = [shuffled[m], shuffled[k]];
    }
    const picks = shuffled.slice(0, 1 + Math.floor(r() * 4));
    const weights = picks.map(() => 0.4 + r());
    const sum = weights.reduce((a, b) => a + b, 0);
    for (const [j, hour] of picks.entries()) {
      const n = (total * weights[j]) / sum;
      const output = Math.round(n * s.out);
      const input = Math.round(n - output);
      const tokens = { input, output, cached: Math.round(input * s.cached), cacheWrite: Math.round(input * s.write), reasoning: Math.round(output * (s.reasoning ?? 0)) };
      const cost = s.reports === true ? Math.round(priceOf(s.model, tokens) * 100) / 100 : undefined;
      rows.push({
        day,
        hour,
        agent: s.agent,
        account: s.account,
        computer: s.computer,
        project: s.project,
        model: s.model,
        turns: s.source === "wsp" ? Math.max(1, Math.round(n / 2.4e6)) : 0,
        tokens,
        ...(cost === undefined ? {} : { costReported: cost }),
        source: s.source,
      });
    }
  }
  rows.sort((a, b) => a.hour - b.hour);
  return { day, rows };
}

const limits = [
  {
    key: "codex:acct_plus",
    agent: "codex",
    label: "maya@example.com",
    road: "named",
    plan: "plus",
    windows: [
      { kind: "session", usedPercent: 62, resetsAt: now + 150 * MIN },
      { kind: "week", usedPercent: 18, resetsAt: now + 3 * DAY + 5 * HOUR },
    ],
    status: "ok",
    readAt: now - 4 * MIN,
    computers: ["here", "boat"],
    credits: {
      count: 2,
      credits: [
        { id: "rc_seed_1", status: "available", expiresAt: now + 9 * DAY + 2 * HOUR },
        { id: "rc_seed_2", status: "available", expiresAt: now + 23 * DAY },
      ],
      readAt: now - 4 * MIN,
      detailAt: now - 4 * MIN,
    },
  },
  {
    key: "codex:acct_pro",
    agent: "codex",
    label: "studio@example.com",
    road: "named",
    plan: "pro",
    windows: [
      { kind: "session", usedPercent: 35, resetsAt: now + 4 * HOUR + 10 * MIN },
      { kind: "week", usedPercent: 93, resetsAt: now + 5 * DAY + 3 * HOUR },
    ],
    status: "warning",
    readAt: now - 7 * MIN,
    computers: ["boat", "studio"],
  },
  { key: "claude:vault-token", agent: "claude", label: "Claude Code with your sign-in", road: "vault", windows: [], readAt: now - 2 * HOUR, computers: ["here", "boat"] },
  { key: "claude:vault-key", agent: "claude", label: "Claude Code with an API key", road: "vault", keyed: true, windows: [], readAt: now - 30 * MIN, computers: ["here", "studio"] },
  { key: "opencode@here", agent: "opencode", label: "OpenCode", road: "own", windows: [], readAt: now - 3 * HOUR, computers: ["here"] },
];
const computers = [
  { id: "boat", name: "Boat" },
  { id: "studio", name: "Studio" },
];

const putDoc = db.prepare("insert or replace into collections (collection, id, json) values (?, ?, ?)");
const putBlob = db.prepare("insert or replace into blobs (collection, id, bytes, at) values (?, ?, ?, ?)");
// A blob read back as TEXT has no .buffer and the host reads it as missing, so the bytes go in as a Buffer.
const blob = value => Buffer.from(JSON.stringify(value));

db.exec("begin immediate");
try {
  // First, so the first read that finds a row prices it off this table and never asks LiteLLM.
  putBlob.run("prices", "litellm", blob({ fetchedAt: now, table: PRICES }), now);
  const days = Array.from({ length: DAYS }, (_, back) => dayRows(back));
  for (const d of days) putBlob.run("usage-days", d.day, blob(d), now);
  const held = db.prepare("select json from collections where collection = 'usage-index' and id = 'days'").get();
  const listed = held === undefined ? [] : (JSON.parse(held.json).days ?? []);
  putDoc.run("usage-index", "days", JSON.stringify({ days: [...new Set([...listed, ...days.map(d => d.day)])].sort() }));
  for (const c of computers) putDoc.run("usage-computers", c.id, JSON.stringify(c));
  for (const l of limits) {
    putDoc.run("limits", l.key, JSON.stringify(l));
    putDoc.run("usage-accounts", l.key, JSON.stringify({ key: l.key, label: l.label }));
  }
  db.exec("commit");
  const rows = days.reduce((n, d) => n + d.rows.length, 0);
  const names = projects.map(p => p.name).join(", ") || "none";
  console.log(
    `seed: ${DAYS} days ${days.at(-1).day}..${days[0].day} (${rows} rows, today through ${String(hourNow).padStart(2, "0")}:00), ${limits.length} accounts, ${computers.length} named computers, ${Object.keys(PRICES).length} priced models (${UNPRICED} unpriced), projects ${names} and none, into ${join(dir, "state.db")}`,
  );
} catch (e) {
  db.exec("rollback");
  throw e;
} finally {
  db.close();
}
