// SPDX-License-Identifier: AGPL-3.0-only
// The doctor's model call from a fork through the door to the host: the curl line it runs, against a stand-in for
// the broker on loopback, with the machine's exec played by this computer's shell.
import { execFile } from "node:child_process";
import { createServer, type IncomingHttpHeaders } from "node:http";
import type { AddressInfo } from "node:net";
import { promisify } from "node:util";
import { BROKER_PORT, type BrokerStatus } from "@wsp/protocol";
import { afterEach, describe, expect, it } from "vitest";
import type { Runtime } from "@wsp/runtime";
import { forkDoctor } from "../src/doctor.js";
import { MODEL_CALL_STEP, modelCall, modelCallSkipped, noClaudeSignInLine, type DoctorBroker } from "../src/doctor-fork.js";
import { captured } from "./verbs-fixture.js";

const run = promisify(execFile);

interface Seen {
  method?: string;
  url?: string;
  headers: IncomingHttpHeaders;
  body: string;
}

const closers: (() => void)[] = [];
afterEach(() => closers.splice(0).forEach(close => close()));

/** A stand-in for the broker's door: what it was sent, and the answer it gives. */
async function door(status: number, answer: string): Promise<{ port: number; seen: Seen[] }> {
  const seen: Seen[] = [];
  const server = createServer((req, res) => {
    let body = "";
    req.on("data", chunk => (body += String(chunk)));
    req.on("end", () => {
      seen.push({ method: req.method, url: req.url, headers: req.headers, body });
      res.writeHead(status, { "content-type": "application/json" }).end(answer);
    });
  });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  closers.push(() => server.close());
  return { port: (server.address() as AddressInfo).port, seen };
}

/** The fork's exec, played by this computer's shell, with the door's port moved to the stand-in's. */
const execAt = (port: number) => async (cmd: string) => {
  const moved = cmd.replace(`:${BROKER_PORT}/`, `:${port}/`);
  try {
    const { stdout, stderr } = await run("/bin/sh", ["-c", moved]);
    return { exitCode: 0, stdout, stderr };
  } catch (e) {
    const failed = e as { code?: number; stdout?: string; stderr?: string };
    return { exitCode: failed.code ?? 1, stdout: failed.stdout ?? "", stderr: failed.stderr ?? "" };
  }
};

function broker(status: BrokerStatus, o: { ready?: boolean } = {}): DoctorBroker & { granted: string[]; revoked: number } {
  const b = {
    granted: [] as string[],
    revoked: 0,
    ready: () => o.ready ?? true,
    status: async () => status,
    grant: async (workspaceId: string) => {
      b.granted.push(workspaceId);
      return { token: "wsp_turn_placeholder", revoke: () => void b.revoked++ };
    },
  };
  return b;
}

describe(MODEL_CALL_STEP, () => {
  it("with a token the host keeps, sends one smallest call as Claude Code sends a token, names the kind, and revokes the grant", async () => {
    const { port, seen } = await door(200, JSON.stringify({ id: "msg_1", type: "message", content: [] }));
    const b = broker({ claude: "token" });
    expect(await modelCall(execAt(port), "w_1", b, "zingzy's MacBook Pro")).toBe("Claude Code, a token");
    expect(seen).toHaveLength(1);
    expect(seen[0]).toMatchObject({ method: "POST", url: "/v1/messages", headers: { authorization: "Bearer wsp_turn_placeholder", "anthropic-beta": "oauth-2025-04-20", "anthropic-version": "2023-06-01" } });
    expect(seen[0]!.headers["x-api-key"]).toBeUndefined();
    expect(JSON.parse(seen[0]!.body)).toEqual({ model: "claude-haiku-5-5", max_tokens: 1, messages: [{ role: "user", content: "hi" }] });
    expect([b.granted, b.revoked]).toEqual([["w_1"], 1]);
  });

  it("with the person's own claude.ai login, sends the placeholder as Claude Code sends that login and names it a subscription", async () => {
    const { port, seen } = await door(200, "{}");
    expect(await modelCall(execAt(port), "w_1", broker({ claude: "login" }), "spoo")).toBe("Claude Code, a subscription");
    expect(seen[0]!.headers).toMatchObject({ authorization: "Bearer wsp_turn_placeholder", "anthropic-beta": "oauth-2025-04-20" });
  });

  it("with a key, sends the placeholder as x-api-key", async () => {
    const { port, seen } = await door(200, "{}");
    expect(await modelCall(execAt(port), "w_1", broker({ claude: "key" }), "spoo")).toBe("Claude Code, an API key");
    expect(seen[0]!.headers).toMatchObject({ "x-api-key": "wsp_turn_placeholder" });
    expect(seen[0]!.headers.authorization).toBeUndefined();
  });

  it("names the broker's own sentence as the cause when it refuses, and still revokes the grant", async () => {
    const said = "wsp: Claude's token on zingzy's MacBook Pro has expired. Make a new one in Settings, Agents.";
    const { port } = await door(403, JSON.stringify({ type: "error", error: { type: "permission_error", message: said } }));
    const b = broker({ claude: "token" });
    await expect(modelCall(execAt(port), "w_1", b, "zingzy's MacBook Pro")).rejects.toThrow(said);
    expect(b.revoked).toBe(1);
  });

  it("with no Claude credential on the host, names that as the cause before the fork sends anything", async () => {
    const { port, seen } = await door(200, "{}");
    const b = broker({ codex: "chatgpt" });
    await expect(modelCall(execAt(port), "w_1", b, "zingzy's MacBook Pro")).rejects.toThrow(noClaudeSignInLine("zingzy's MacBook Pro"));
    expect([seen, b.granted]).toEqual([[], []]);
  });

  it("is skipped while no broker serves, and the fork is never asked", async () => {
    const asked: string[] = [];
    const exec = async (cmd: string) => (asked.push(cmd), { exitCode: 0, stdout: "", stderr: "" });
    expect(await modelCall(exec, "w_1", undefined, "spoo")).toBe(modelCallSkipped);
    expect(await modelCall(exec, "w_1", broker({ claude: "key" }, { ready: false }), "spoo")).toBe(modelCallSkipped);
    expect(asked).toEqual([]);
  });
});

describe("the fork the doctor makes", () => {
  /** A host with Boat and a sealed image whose fork stops at its create, which is where the spec is read. */
  function stopsAtCreate() {
    const specs: { envs?: Record<string, string> }[] = [];
    const rt = {
      places: { list: async () => [{ id: "box", kind: "provider", name: "box", default: false, takesForks: true }] },
      projects: { add: async () => ({ id: "pr_doctor", name: "wsp" }) },
      workspaces: {
        create: async (spec: { envs?: Record<string, string> }) => {
          specs.push(spec);
          throw new Error("the fork stops here");
        },
      },
      golden: { promote: async () => [], storage: async () => undefined, orphans: async () => undefined, get: async () => ({ name: "default", head: 1, versions: [{ version: 1, snapshotId: "snap_1" }] }) },
    } as unknown as Runtime;
    return { rt, specs };
  }
  const envs = { ANTHROPIC_API_KEY: "sk-ant-api03-placeholder", CLAUDE_CONFIG_DIR: "/root/.claude" };

  it("holds no key of the person's while the broker serves, and the key it has today while it does not", async () => {
    const ready = stopsAtCreate();
    expect(await forkDoctor(ready.rt, captured(), { envs, broker: broker({ claude: "key" }) })).toBe(1);
    expect(ready.specs).toHaveLength(1);
    expect(ready.specs[0]!.envs).toBeUndefined();
    const today = stopsAtCreate();
    expect(await forkDoctor(today.rt, captured(), { envs, broker: broker({ claude: "key" }, { ready: false }) })).toBe(1);
    expect(today.specs[0]!.envs).toEqual(envs);
  });
});
