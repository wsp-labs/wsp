// SPDX-License-Identifier: AGPL-3.0-only
// The `host` parity/wave-2/states.mjs hands each state's data() and cleanup(), as parity/baseline.mjs gives it, over a
// host bench/fixture.sh started: its fixture, its HOME, wsp lines, host ops, preferences, scripted threads, and work
// done once per fixture (kept in its gpui-made.json). The fixture's claude plays parity/claude-stub.mjs, as there.
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readlinkSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const DESKTOP = resolve(here, "../../../..");
export const REPO = resolve(DESKTOP, "../..");
const HOST_BIN = join(REPO, "packages/host/dist/bin.js");
const { dialHost } = await import(pathToFileURL(join(REPO, "packages/host/dist/index.js")).href);

/** The parity states these shots cover, in the order both sides run them, so each side's sidebar holds the same threads. */
export const IDS = ["019", "021", "023", "024", "025", "026", "027", "029", "030", "031", "032", "033", "037", "039", "040", "041", "042", "043", "044", "045", "046", "047-merged", "047-closed", "901"].map(n => `W2-PRP-${n}`);

export async function states() {
  const all = (await import(pathToFileURL(join(DESKTOP, "parity/wave-2/states.mjs")).href)).default;
  return IDS.map(id => {
    const state = all.find(s => s.id === id);
    if (state === undefined || state.skip !== undefined) throw new Error(`states.mjs has no shot for ${id}`);
    return state;
  });
}

export function hostOf(outJson) {
  const fixture = JSON.parse(readFileSync(outJson, "utf8"));
  const home = dirname(dirname(fixture.state));
  const env = { HOME: home, PATH: [join(home, "bin"), dirname(process.execPath), "/usr/local/bin", "/usr/bin", "/bin"].join(":"), LANG: "en_US.UTF-8", WSP_UPDATE_CHECK: "0" };
  const ledgerPath = join(home, "gpui-made.json");
  const ledger = existsSync(ledgerPath) ? JSON.parse(readFileSync(ledgerPath, "utf8")) : {};
  const claude = join(home, "bin", "claude");
  const stub = join(DESKTOP, "parity/claude-stub.mjs");
  if (readlinkSync(claude) !== stub) {
    rmSync(claude);
    symlinkSync(stub, claude);
  }
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  const host = {
    fixture,
    home,
    port: () => JSON.parse(readFileSync(join(dirname(fixture.state), "host.lock"), "utf8")).port,
    token: () => readFileSync(join(dirname(fixture.state), "host-token"), "utf8").trim(),
    async once(key, work) {
      if (ledger[key] === undefined) {
        ledger[key] = (await work()) ?? true;
        writeFileSync(ledgerPath, `${JSON.stringify(ledger, null, 2)}\n`);
      }
      return ledger[key];
    },
    async wsp(...args) {
      const ran = spawnSync(process.execPath, [HOST_BIN, ...args, "--state", fixture.state], { cwd: home, env, encoding: "utf8", timeout: 120_000 });
      const said = `${ran.stdout}${ran.stderr}`;
      if (ran.status !== 0) throw new Error(`wsp ${args.join(" ")} (${ran.status ?? ran.signal}): ${said}`);
      return said;
    },
    async request(op, params = {}) {
      // Aimed at the fixture's lock alone: left to itself dialHost reads WSP_HOST and WSP_HOST_URL ahead of the state file.
      const client = await dialHost(fixture.state, { env: { HOME: home, PATH: env.PATH, LANG: env.LANG }, aim: { kind: "here" } });
      try {
        return await client.request(op, params);
      } finally {
        client.close();
      }
    },
    op: (name, params) => host.request(name, params),
    pref: (name, value) => host.request("preferences.set", { patch: { [name]: value } }),
    threads: async () => (await host.wsp("threads", "--json")).split("\n").filter(l => l.startsWith("{")).flatMap(l => JSON.parse(l).threads ?? []),
    async workspaceOf(thread) {
      const row = (await host.threads()).find(t => t.threadId === thread || t.id === thread);
      if (row === undefined) throw new Error(`the host lists no thread ${thread}`);
      return row.workspaceId;
    },
    /** What the stub plays for a message holding `prompt`. */
    script(prompt, reply) {
      const at = join(home, "parity-turns.json");
      const turns = existsSync(at) ? JSON.parse(readFileSync(at, "utf8")) : {};
      turns[prompt] = reply;
      writeFileSync(at, `${JSON.stringify(turns, null, 2)}\n`);
    },
    /** A thread on `project` whose first message is `prompt`, its turn finished; made once, answers its id. */
    thread: (prompt, { project = "bench", reply = "markdown" } = {}) =>
      host.once(`thread:${prompt}`, async () => {
        host.script(prompt, reply);
        const said = await host.wsp("run", project, "--agent", "claude", prompt);
        const id = /\bthread ([0-9a-f-]{36})\b/.exec(said)?.[1];
        if (id === undefined) throw new Error(`wsp run printed no thread: ${said}`);
        for (const until = Date.now() + 30_000; ; await sleep(250)) {
          const row = (await host.threads()).find(t => t.threadId === id || t.id === id);
          if (row !== undefined && row.status !== "running") return id;
          if (Date.now() > until) throw new Error(`thread ${id} did not finish in 30 s`);
        }
      }),
  };
  return host;
}
