// SPDX-License-Identifier: AGPL-3.0-only
// Writes record/today-local-storage/: the Local Storage folder Chromium keeps for today's app, holding the keys
// src/desktop/carry.rs moves over, as a real Chromium wrote it. The third launch reopens the profile, which moves the
// earlier launches' log, past 2 MB, into a table, and writes again, so the folder holds both of LevelDB's file kinds
// and a value overwritten across them.
// Run with node app/today-local-storage.mjs [chrome binary]; it serves its page on 127.0.0.1:4400 and 4401.
import { spawn } from "node:child_process";
import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";

const chrome = process.argv[2] ?? "google-chrome";
const out = new URL("record/today-local-storage/", import.meta.url).pathname;
const profile = mkdtempSync(join(tmpdir(), "today-local-storage-"));

const drafts = {
  state: {
    drafts: {
      thr_ascii: { prompt: "half a thought", cursor: 6 },
      thr_wide: { prompt: "naïve 日本 🙂 end", cursor: "naïve 日本 🙂".length },
    },
    queues: { thr_ascii: [{ id: "m3k2-abc123", prompt: "queued first" }, { id: "m3k3-def456", prompt: "queued second" }] },
    unclaimed: { ws_old: { prompt: "from an earlier build", cursor: 4 } },
  },
  version: 2,
};
const stash = [
  { id: "x-0", createdAt: "2026-10-20T10:00:00.000Z", prompt: "saved for later", files: [{ mediaType: "text/plain", name: "note.txt", bytes: "aGVsbG8=", size: 5 }], dropped: [] },
];
const row = (key, asked, workspaceId, failed = null) => ({ key, name: key, project: "prj_1", askedAt: 1761300000000, asked, queued: true, workspaceId, lines: [], failed });
const creations = {
  "/home/person/.wsp/state.json": [
    row("c1", "first message for ws_new", "ws_new"),
    row("c2", "asked before the workspace had an id", null),
    row("c3", "asked where a draft waits", "thr_ascii"),
    row("c6", "asked of a create that was refused", "ws_refused", { title: "Could not create", detail: "the disk is full" }),
  ],
  "/home/person/other/state.json": [row("c4", "another state file's", "ws_other")],
  "": [row("c5", "asked on a host somewhere else", "ws_remote")],
};

const first = {
  "wsp:composer-drafts:v1": drafts,
  "wsp:prompt-stash": stash,
  "wsp:composer-favourites:v1": { state: { keys: ["claude:old"] }, version: 1 },
  "wsp:composer-modes": { state: { fast: { thr_ascii: true } }, version: 0 },
  "wsp:creations:v1": creations,
  "wsp:sidebar-width": 300,
};
// A stash entry's file of 1.6 MB puts the log past LevelDB's 2 MB, so the third launch moves it into a table.
const notes = Buffer.from("a page of notes, the same line again and again\n".repeat(36_000));
const second = {
  "wsp:prompt-stash": [{ id: "x-1", createdAt: "2026-10-21T09:30:00.000Z", prompt: "the long notes", files: [{ mediaType: "text/plain", name: "notes.txt", bytes: notes.toString("base64"), size: notes.length }], dropped: [] }, ...stash],
};
const third = { "wsp:composer-favourites:v1": { state: { keys: ["claude:opus", "codex:gpt-5"] }, version: 1 } };
const otherHost = { "wsp:composer-drafts:v1": { state: { drafts: { thr_other_host: { prompt: "typed on another host", cursor: 0 } } }, version: 2 } };

const servers = [4400, 4401].map(port => createServer((_req, res) => res.end("<!doctype html><title>today</title>")).listen(port, "127.0.0.1"));

async function launch(writes) {
  const portFile = join(profile, "DevToolsActivePort");
  rmSync(portFile, { force: true });
  const child = spawn(chrome, ["--headless=new", "--no-first-run", "--no-sandbox", "--no-default-browser-check", `--user-data-dir=${profile}`, "--remote-debugging-port=0", "about:blank"], { stdio: "ignore" });
  while (!existsSync(portFile) || readFileSync(portFile, "utf8").split("\n").length < 2) await new Promise(r => setTimeout(r, 50));
  const [port, path] = readFileSync(portFile, "utf8").split("\n");
  const ws = new WebSocket(`ws://127.0.0.1:${port}${path}`);
  await new Promise(r => ws.addEventListener("open", r, { once: true }));
  let next = 0;
  const waiting = new Map();
  ws.addEventListener("message", e => {
    const msg = JSON.parse(e.data);
    if (msg.id !== undefined) waiting.get(msg.id)?.(msg), waiting.delete(msg.id);
  });
  const send = (method, params = {}, sessionId) =>
    new Promise((resolve, reject) => {
      const id = ++next;
      waiting.set(id, msg => (msg.error ? reject(new Error(`${method}: ${msg.error.message}`)) : resolve(msg.result)));
      ws.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
    });
  for (const [origin, items] of writes) {
    const { targetId } = await send("Target.createTarget", { url: origin });
    const { sessionId } = await send("Target.attachToTarget", { targetId, flatten: true });
    await send("Runtime.enable", {}, sessionId);
    let ready;
    do ready = (await send("Runtime.evaluate", { expression: `location.origin === ${JSON.stringify(origin)} && document.readyState === "complete"`, returnByValue: true }, sessionId)).result.value;
    while (!ready);
    const script = Object.entries(items).map(([k, v]) => `localStorage.setItem(${JSON.stringify(k)}, ${JSON.stringify(JSON.stringify(v))});`).join("");
    await send("Runtime.evaluate", { expression: script }, sessionId);
  }
  await send("Browser.close").catch(() => {});
  await new Promise(r => child.on("exit", r));
}

await launch([["http://127.0.0.1:4400", first]]);
await launch([["http://127.0.0.1:4400", second], ["http://127.0.0.1:4401", otherHost]]);
await launch([["http://127.0.0.1:4400", third]]);
servers.forEach(s => s.close());
rmSync(out, { recursive: true, force: true });
cpSync(join(profile, "Default", "Local Storage", "leveldb"), out, { recursive: true, filter: src => !/\/(LOCK|LOG|LOG\.old)$/.test(src) });
rmSync(profile, { recursive: true, force: true });
