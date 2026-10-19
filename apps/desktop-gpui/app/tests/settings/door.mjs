// SPDX-License-Identifier: AGPL-3.0-only
// A host that answers only what a test tells it, for a test that needs the host to refuse, wait or push on cue.
// `node door.mjs <dir>` serves on loopback and writes <dir>/host.lock and <dir>/host-token beside <dir>/state.json,
// as a host does, so a client dials it by that state file. Each frame after the token is appended to <dir>/asked.jsonl.
// <dir>/rules.json, read again at every frame, names per op what to do:
//   { "<op>": { "reply": {...} } | { "error": "...", "kind": "...", "fix": "..." }, "delayMs": n, "push": [event, ...] }
// A `daemon.send:<frame op>` rule answers that daemon op ahead of the `daemon.send` rule.
// "push" goes to every socket as the frame arrives, before the answer; an op no rule names is refused. `GET /` serves
// the page a dial reads first: the boot line naming the socket's path, the host package's release and the token's digest.
import { appendFileSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash, randomBytes } from "node:crypto";
import { createServer } from "node:http";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../../../../..");
const { WebSocketServer } = createRequire(join(root, "packages/host/package.json"))("ws");

const dir = process.argv[2];
const token = randomBytes(16).toString("hex");
const rules = () => {
  try {
    return JSON.parse(readFileSync(join(dir, "rules.json"), "utf8"));
  } catch {
    return {};
  }
};
const sockets = new Set();
const version = JSON.parse(readFileSync(join(root, "packages/host/package.json"), "utf8")).version;
const boot = { wsPath: "/ws", version, tokenHash: createHash("sha256").update(token).digest("hex") };
const page = `<!doctype html><html><head><script>window.__WSP__ = ${JSON.stringify(boot)};</script></head><body></body></html>`;
const server = createServer((_, res) => res.writeHead(200, { "content-type": "text/html" }).end(page));
const wss = new WebSocketServer({ server });

wss.on("connection", ws => {
  let authed = false;
  sockets.add(ws);
  ws.on("close", () => sockets.delete(ws));
  ws.on("message", async data => {
    const frame = JSON.parse(String(data));
    const answer = body => ws.send(JSON.stringify({ id: frame.id, ...body }));
    if (!authed) {
      authed = frame.op === "auth" && frame.token === token;
      return answer(authed ? { ok: true, version } : { ok: false, error: "That token is not this host's.", kind: "auth" });
    }
    appendFileSync(join(dir, "asked.jsonl"), `${JSON.stringify(frame)}\n`);
    if (frame.op === "events.subscribe") return answer({ ok: true });
    const rule = (frame.op === "daemon.send" ? rules()[`daemon.send:${frame.frame?.op}`] : undefined) ?? rules()[frame.op];
    if (rule === undefined) return answer({ ok: false, error: `${frame.op} is not served here.`, kind: "usage" });
    for (const event of rule.push ?? []) for (const s of sockets) s.send(JSON.stringify(event));
    if (rule.delayMs !== undefined) await new Promise(r => setTimeout(r, rule.delayMs));
    if (rule.error !== undefined) return answer({ ok: false, error: rule.error, ...(rule.kind ? { kind: rule.kind } : {}), ...(rule.fix ? { fix: rule.fix } : {}) });
    answer({ ok: true, ...(rule.reply ?? {}) });
  });
});

server.listen(0, "127.0.0.1", () => {
  writeFileSync(join(dir, "host-token"), token, { mode: 0o600 });
  const lock = { pid: process.pid, port: server.address().port, address: "127.0.0.1", startedAt: new Date().toISOString() };
  writeFileSync(join(dir, "host.lock.next"), JSON.stringify(lock));
  renameSync(join(dir, "host.lock.next"), join(dir, "host.lock"));
});

process.on("SIGTERM", () => process.exit(0));
