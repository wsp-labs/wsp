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
const TURN = "turn-token-kept-off-tape";

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
    await client.request("sessions.start", { workspaceId: "ws_1", prompt: "hi", turnToken: TURN });
    const { ticket } = await client.request("ticket.issue", { purpose: "connect" });
    client.close();

    const [file, ...more] = readdirSync(dir);
    expect(more).toEqual([]);
    const text = readFileSync(join(dir, file!), "utf8");
    for (const secret of [HOST_TOKEN, CODE, TURN, String(ticket)]) expect(text).not.toContain(secret);
    expect(String(ticket).length).toBeGreaterThan(8);
    const lines = text.trimEnd().split("\n").map(l => JSON.parse(l) as Record<string, { op?: string; token?: string; code?: string }>);
    expect(lines[0]).toEqual({ meta: { door: "ws", source: "the tap case" } });
    expect(lines.slice(1).map(l => Object.keys(l)[0])).toEqual(["in", "out", "in", "out", "in", "out", "in", "out", "in", "out"]);
    expect(lines[1]!["in"]).toMatchObject({ op: "auth", token: "<host-token>" });
    const mark = expect.stringMatching(/^<redacted-\d+>$/);
    expect(lines[3]!["in"]).toMatchObject({ op: "pair.redeem", code: mark });
    expect(lines[6]!["out"]).toMatchObject({ ok: true, workspaces: [] });
    expect(lines[7]!["in"]).toMatchObject({ op: "sessions.start", turnToken: mark });
    expect(lines[10]!["out"]).toMatchObject({ ok: true, ticket: mark });
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

  it("marks the host's own token, however short, apart from every other secret, so a replay signs in only where the case did", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    const tape = tapeOf({ [TAPE_DIR_ENV]: dir }, "ws", "t")!;
    tape.in({ id: 1, op: "auth", token: "t" });
    tape.in({ id: 2, op: "auth", token: "u" });
    tape.in({ id: 3, op: "agents.key", agent: "claude", key: "sk-acme-x" });
    tape.out({ id: 3, ok: false, error: "the key sk-acme-x was refused" });
    const lines = readFileSync(join(dir, readdirSync(dir)[0]!), "utf8").trimEnd().split("\n").slice(1);
    expect(lines).toEqual([
      JSON.stringify({ in: { id: 1, op: "auth", token: "<host-token>" } }),
      JSON.stringify({ in: { id: 2, op: "auth", token: "<redacted-1>" } }),
      JSON.stringify({ in: { id: 3, op: "agents.key", agent: "claude", key: "<redacted-2>" } }),
      JSON.stringify({ out: { id: 3, ok: false, error: "the key <redacted-2> was refused" } }),
    ]);
    // A stand-in host token that is a word, or the text of the mark itself, is marked where it is the token alone.
    const apart = join(dir, "named");
    const named = tapeOf({ [TAPE_DIR_ENV]: apart }, "ws", "host-token")!;
    named.in({ id: 1, op: "auth", token: "host-token" });
    named.out({ id: 1, ok: true, said: "the host-token input" });
    expect(readFileSync(join(apart, readdirSync(apart)[0]!), "utf8").trimEnd().split("\n").slice(1)).toEqual([
      JSON.stringify({ in: { id: 1, op: "auth", token: "<host-token>" } }),
      JSON.stringify({ out: { id: 1, ok: true, said: "the host-token input" } }),
    ]);
  });

  it("blanks a paired device's token where an answer hands it out or a join nests it, and a join code in any door", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    const device = "device-token-kept-off-tape";
    const joinCode = "K7QX-M2PA";
    const ws = tapeOf({ [TAPE_DIR_ENV]: dir }, "ws", HOST_TOKEN)!;
    ws.out({ id: 2, ok: true, deviceId: "d_1", deviceToken: device });
    ws.out({ id: 3, ok: true, placeId: "p_1", device: { deviceId: "d_2", deviceToken: `${device}-nested` } });
    ws.out({ id: 4, ok: true, joins: [{ line: `wsp join https://host.example --code ${joinCode}.SHA256:abc` }] });
    const cli = tapeOf({ [TAPE_DIR_ENV]: dir }, "cli")!;
    cli.in({ argv: ["join", "https://host.example", "--code", `${joinCode}.SHA256:abc`] });
    const text = readdirSync(dir).map(f => readFileSync(join(dir, f), "utf8")).join("");
    for (const secret of [device, `${device}-nested`, joinCode]) expect(text).not.toContain(secret);
  });

  it("numbers each secret on its tape, so a token an answer hands out reads the same where a later request sends it", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    const tape = tapeOf({ [TAPE_DIR_ENV]: dir }, "ws", HOST_TOKEN)!;
    tape.out({ id: 2, ok: true, deviceId: "d_1", deviceToken: "device-token-one" });
    tape.in({ id: 3, op: "device.auth", deviceToken: "device-token-one" });
    tape.in({ id: 4, op: "auth", token: "a-wrong-token" });
    const lines = readFileSync(join(dir, readdirSync(dir)[0]!), "utf8").trimEnd().split("\n").slice(1);
    expect(lines).toEqual([
      JSON.stringify({ out: { id: 2, ok: true, deviceId: "d_1", deviceToken: "<redacted-1>" } }),
      JSON.stringify({ in: { id: 3, op: "device.auth", deviceToken: "<redacted-1>" } }),
      JSON.stringify({ in: { id: 4, op: "auth", token: "<redacted-2>" } }),
    ]);
  });

  it("blanks what reads as a secret in any text on every door: a flag's value, a bearer in a URL, a key by its name or prefix", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    const secrets = ["tok-in-a-flag", "preview-bearer-1", "env-secret-value", "ghp_0123456789abcdefghij", "bearer-in-a-header"];
    const mcp = tapeOf({ [TAPE_DIR_ENV]: dir }, "mcp")!;
    mcp.in({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "servers_add", arguments: { url: "https://mcp.example/?api_token=preview-bearer-1", args: ["--token", "tok-in-a-flag"] } } });
    mcp.out({ jsonrpc: "2.0", id: 1, result: { content: [{ type: "text", text: "ran GITHUB_TOKEN=env-secret-value with ghp_0123456789abcdefghij" }] } });
    const cli = tapeOf({ [TAPE_DIR_ENV]: dir }, "cli")!;
    cli.in({ argv: ["servers", "add", "x", "--header", "Authorization: Bearer bearer-in-a-header"] });
    const text = readdirSync(dir).map(f => readFileSync(join(dir, f), "utf8")).join("");
    for (const secret of secrets) expect(text).not.toContain(secret);
    expect(text).toContain("api_token=<redacted-");
  });

  it("leaves an id whose groups happen to read like a code as it is", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    const id = "ec8c3145-1234-4567-88db-d9a0c2e4f6b8";
    tapeOf({ [TAPE_DIR_ENV]: dir }, "ws")!.out({ id: 2, ok: true, threadId: id, said: `thread ${id} ran` });
    expect(readFileSync(join(dir, readdirSync(dir)[0]!), "utf8")).not.toContain("redacted");
  });

  it("writes nothing when no folder is named", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    expect(tapeOf({}, "ws")).toBeUndefined();
    expect(readdirSync(dir)).toEqual([]);
  });
});
