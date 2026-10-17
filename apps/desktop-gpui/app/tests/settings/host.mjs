// SPDX-License-Identifier: AGPL-3.0-only
// The fixture host as the settings shots reach it: its page's address and token off the out.json fixture.sh wrote,
// and one runtime op sent over its socket with the host's own token, the caller the GPUI app is.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";

/** The host a fixture's out.json names: the page's address, the token file and the token. */
export function hostOf(fixturePath) {
  const fixture = JSON.parse(readFileSync(fixturePath, "utf8"));
  const stateDir = dirname(fixture.state);
  const lock = JSON.parse(readFileSync(join(stateDir, "host.lock"), "utf8"));
  const tokenFile = join(stateDir, "host-token");
  return { ...fixture, url: `http://127.0.0.1:${lock.port}/`, tokenFile, token: readFileSync(tokenFile, "utf8").trim() };
}

/** Sends each frame in turn on one authed socket and answers their replies; a reply carrying ok:false throws. */
export async function ops(url, token, frames) {
  const ws = new WebSocket(`${url.replace(/^http/, "ws").replace(/\/$/, "")}/ws`);
  const replies = new Map();
  const waiting = new Map();
  ws.onmessage = m => {
    const msg = JSON.parse(String(m.data));
    if (typeof msg.id !== "number") return;
    const wait = waiting.get(msg.id);
    if (wait === undefined) replies.set(msg.id, msg);
    else waiting.delete(msg.id), wait(msg);
  };
  const reply = id => new Promise(resolve => (replies.has(id) ? resolve(replies.get(id)) : waiting.set(id, resolve)));
  await new Promise((resolve, reject) => ((ws.onopen = resolve), (ws.onerror = () => reject(new Error(`no socket at ${url}`)))));
  ws.send(JSON.stringify({ id: 0, op: "auth", token }));
  const authed = await reply(0);
  if (authed.ok === false) throw new Error(`auth refused: ${authed.error}`);
  const answers = [];
  try {
    for (const [i, frame] of frames.entries()) {
      ws.send(JSON.stringify({ ...frame, id: i + 1 }));
      const answer = await reply(i + 1);
      if (answer.ok === false) throw new Error(`${frame.op} refused: ${answer.error ?? JSON.stringify(answer)}`);
      answers.push(answer);
    }
  } finally {
    ws.close();
  }
  return answers;
}

/** The recipe the fixture saves, which a state that removes it saves again. */
export const RECIPE = { name: "Web app", agents: { claude: { signin: "machine" } }, mcp: { docs: { agents: ["claude"] } }, clis: { pnpm: { via: "npm" } }, skills: {}, plugins: {}, folders: {}, configs: { git: {} } };

/** Pairs a device by that name: a code the host's token issues, redeemed as the first frame of a socket of its own. */
export async function pair(url, token, name) {
  const [{ code }] = await ops(url, token, [{ op: "pair.issue" }]);
  const ws = new WebSocket(`${url.replace(/^http/, "ws").replace(/\/$/, "")}/ws`);
  const said = await new Promise((resolve, reject) => {
    ws.onopen = () => ws.send(JSON.stringify({ id: 1, op: "pair.redeem", code, name }));
    ws.onmessage = m => resolve(JSON.parse(String(m.data)));
    ws.onerror = () => reject(new Error("pair.redeem: no socket"));
  });
  ws.close();
  if (said.ok !== true) throw new Error(`pair.redeem refused: ${said.error}`);
}

/** A state's before or after ops, with the names states.mjs uses read against the host: a device revoked by its name,
 * the fixture's recipe file, and a device paired by its name. */
export async function sendOps(host, frames) {
  for (const frame of frames ?? []) {
    if (frame.pair !== undefined) {
      await pair(host.url, host.token, frame.pair);
    } else if (frame.op === "devices.revoke") {
      const [{ devices }] = await ops(host.url, host.token, [{ op: "devices.list" }]);
      for (const d of devices.filter(d => d.name === frame.device)) await ops(host.url, host.token, [{ op: "devices.revoke", deviceId: d.id }]);
    } else {
      await ops(host.url, host.token, [frame.file === "fixture" ? { ...frame, file: RECIPE } : frame]);
    }
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const [fixturePath, op, params] = process.argv.slice(2);
  if (op === undefined) {
    console.error("usage: node host.mjs <out.json> <op> [json params]");
    process.exit(2);
  }
  const host = hostOf(fixturePath);
  const [answer] = await ops(host.url, host.token, [{ op, ...(params === undefined ? {} : JSON.parse(params)) }]);
  console.log(JSON.stringify(answer, null, 2));
}
