// SPDX-License-Identifier: AGPL-3.0-only
// A turn launched while no app draws looks is handed --no-look, whether its agent was started cold by the send or
// ahead of it by the composer's warm, and a kept agent's next turn follows an app that opened since its first.
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { LocalBackend } from "@wsp/engine";
import { HERE_PLACE_ID, NO_LOOK_MCP_ARG } from "@wsp/protocol";
import { HARNESS_ADAPTERS } from "../src/adapters.js";
import { localExecStream } from "../src/local-exec.js";
import { createRuntime, type Runtime } from "../src/runtime.js";
import { slateLookHubOf } from "../src/serve-slate-look.js";
import { memoryStore } from "../src/store.js";
import { claudeKeeper } from "./kept-stubs.js";
import { stubBackend, copyingFake, createOn, testPlatform } from "./stub-backend.js";
import { until } from "./until.js";

let root: string;
let runDir: string;
let heard: string;
const runtimes: Runtime[] = [];

beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), "wsp-look-launch-"));
  runDir = join(root, "runs");
  heard = join(root, "heard");
  mkdirSync(join(root, "bin"));
  claudeKeeper(join(root, "bin", "claude"));
});
afterEach(async () => {
  for (const rt of runtimes.splice(0)) await rt.close();
  await localExecStream({ root, runDir }).sweep!([]);
  rmSync(root, { recursive: true, force: true });
});

const host = (): Runtime => {
  const rt = createRuntime({
    backend: stubBackend(),
    store: memoryStore(),
    adapters: { claude: HARNESS_ADAPTERS.claude },
    local: {
      backend: new LocalBackend({ root }),
      execStream: e => localExecStream({ root, runDir, pollMs: 20, ...e }),
      home: () => join(root, ".claude"),
      homeDir: root,
      rootsPath: join(root, "roots"),
      env: () => ({ PATH: `${join(root, "bin")}:${process.env["PATH"] ?? "/usr/bin:/bin"}`, STUB_HEARD: heard }),
      platform: testPlatform(),
      copier: copyingFake(),
    },
    agents: { here: { url: "http://127.0.0.1:9" }, wspMcp: { command: "node", args: ["/opt/wsp/dist/bin.js", "mcp"] } },
  });
  runtimes.push(rt);
  return rt;
};

const ups = (): string[] => (existsSync(heard) ? readFileSync(heard, "utf8") : "").split("\n").filter(l => l.startsWith("up "));

describe("--no-look on a turn launched while no app draws looks", () => {
  it("is on the line of an agent started cold by the send", async () => {
    const rt = host();
    const ws = await createOn(rt, { on: HERE_PLACE_ID, name: "mac" });
    const handle = await rt.sessions.start(ws.id, { prompt: "one", harness: "claude" });
    await handle.finished;
    await until(() => ups().length === 1, 10_000);
    expect(ups()[0]).toContain(NO_LOOK_MCP_ARG);
  }, 30_000);

  it("is on the line of an agent the composer warmed ahead of the send", async () => {
    const rt = host();
    const ws = await createOn(rt, { on: HERE_PLACE_ID, name: "mac" });
    expect(await rt.sessions.warm(ws.id, { harness: "claude" })).toEqual({ warm: "started" });
    await until(() => ups().length === 1, 10_000);
    const handle = await rt.sessions.start(ws.id, { prompt: "one", harness: "claude" });
    await handle.finished;
    // The send ran on the warmed process: no second agent came up.
    expect(ups()).toHaveLength(1);
    expect(ups()[0]).toContain(NO_LOOK_MCP_ARG);
  }, 30_000);
  it("follows an app that opened since the thread's first turn, on the thread's next turn", async () => {
    const rt = host();
    const ws = await createOn(rt, { on: HERE_PLACE_ID, name: "mac" });
    const one = await rt.sessions.start(ws.id, { prompt: "one", harness: "claude" });
    await one.finished;
    await until(() => ups().length === 1, 10_000);
    expect(ups()[0]).toContain(NO_LOOK_MCP_ARG);
    slateLookHubOf(rt.slates).answer({ id: 1, op: "slateLook.offer" } as never, { own: true, send: () => undefined, detaches: [] } as never);
    const two = await rt.sessions.start(ws.id, { prompt: "two", thread: one.view().threadId! });
    await two.finished;
    const last = ups().at(-1)!;
    expect(last).not.toContain(NO_LOOK_MCP_ARG);
  }, 30_000);

  it("follows an app that closed since the thread's first turn, on the thread's next turn", async () => {
    const rt = host();
    const ws = await createOn(rt, { on: HERE_PLACE_ID, name: "mac" });
    const app = { own: true, send: () => undefined, detaches: [] as (() => void)[] };
    slateLookHubOf(rt.slates).answer({ id: 1, op: "slateLook.offer" } as never, app as never);
    const one = await rt.sessions.start(ws.id, { prompt: "one", harness: "claude" });
    await one.finished;
    await until(() => ups().length === 1, 10_000);
    expect(ups()[0]).not.toContain(NO_LOOK_MCP_ARG);
    for (const detach of app.detaches) detach();
    const two = await rt.sessions.start(ws.id, { prompt: "two", thread: one.view().threadId! });
    await two.finished;
    await until(() => ups().length === 2, 10_000);
    expect(ups().at(-1)).toContain(NO_LOOK_MCP_ARG);
  }, 30_000);
});
