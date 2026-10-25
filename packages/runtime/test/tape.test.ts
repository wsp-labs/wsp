// SPDX-License-Identifier: AGPL-3.0-only
// The tap the parity harness records the host's wire with: every frame in and out of a socket, one JSON line each,
// with no secret on any line.
import { mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { createRuntime } from "../src/runtime.js";
import { serveRuntime, type RuntimeServer } from "../src/serve.js";
import { memoryStore } from "../src/store.js";
import { TAPE_DIR_ENV, TAPE_SOURCE_ENV, tapeOf } from "../src/tape.js";
import { stubBackend } from "./stub-backend.js";
import { WsClient } from "./ws-client.js";

const HOST_TOKEN = "host-token-kept-off-tape";
const CODE = "pair-code-kept-off-tape";

describe("the tap", () => {
  let dir: string;
  let srv: RuntimeServer | undefined;
  afterEach(async () => {
    await srv?.close();
    srv = undefined;
    rmSync(dir, { recursive: true, force: true });
  });

  it("writes a socket's frames in order with the host token and a request's code blanked on every line", async () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    const rt = createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: {} });
    srv = await serveRuntime(rt, { port: 0, authToken: HOST_TOKEN, devices: rt.devices, tapes: { [TAPE_DIR_ENV]: dir, [TAPE_SOURCE_ENV]: "the tap case" } });
    const client = await WsClient.connect(srv.port, { token: HOST_TOKEN });
    await client.request("pair.redeem", { code: CODE, name: "lab" });
    await client.request("workspaces.list");
    client.close();

    const [file, ...more] = readdirSync(dir);
    expect(more).toEqual([]);
    const text = readFileSync(join(dir, file!), "utf8");
    expect(text).not.toContain(HOST_TOKEN);
    expect(text).not.toContain(CODE);
    const lines = text.trimEnd().split("\n").map(l => JSON.parse(l) as Record<string, { op?: string; token?: string; code?: string }>);
    expect(lines[0]).toEqual({ meta: { door: "ws", source: "the tap case" } });
    expect(lines.slice(1).map(l => Object.keys(l)[0])).toEqual(["in", "out", "in", "out", "in", "out"]);
    expect(lines[1]!["in"]).toMatchObject({ op: "auth", token: "<redacted>" });
    expect(lines[3]!["in"]).toMatchObject({ op: "pair.redeem", code: "<redacted>" });
    expect(lines[6]!["out"]).toMatchObject({ ok: true, workspaces: [] });
  });

  it("never throws into the host: a frame it cannot write and one carrying more secrets than any real frame cost the tape their line", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    const tape = tapeOf({ [TAPE_DIR_ENV]: dir }, "ws")!;
    let deep: Record<string, unknown> = {};
    for (let i = 0; i < 50_000; i++) deep = { deep };
    tape.in(deep);
    tape.in({ op: "agents.key", rows: Object.fromEntries(Array.from({ length: 300 }, (_, i) => [`k${i}`, `secret-value-${i}`])) });
    const lines = readFileSync(join(dir, readdirSync(dir)[0]!), "utf8").trimEnd().split("\n").slice(1);
    expect(lines).toEqual([JSON.stringify({ in: "<not written: RangeError>" }), JSON.stringify({ in: "<withheld: 300 secret values>" })]);
  });

  it("writes nothing when no folder is named", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    expect(tapeOf({}, "ws")).toBeUndefined();
    expect(readdirSync(dir)).toEqual([]);
  });
});
