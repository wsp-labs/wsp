// SPDX-License-Identifier: AGPL-3.0-only
// The tap the parity harness records the host's wire with: every frame in and out of a socket, one JSON line each,
// with no secret on any line.
import { mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs";
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

  it("blanks what a real host's day carries on the command line and the tool server: logins, its own ticket, nested env, typed keys, the turn's token", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    const turn = "0123456789abcdef0123456789abcdef";
    const secrets = ["Zq9amK2vLpR7sTb4WnY8x", "Zq9bmK2vLpR7sTb4WnY8x", "Zq9cmK2vLpR7sTb4WnY8x", "Zq9dmK2vLpR7sTb4WnY8x", ["glpat", "Zq9mK2vLpR7sTb4WnY8x"].join("-"), turn];
    const [login, ticket, env, typed, glpat] = secrets;
    const cli = tapeOf({ [TAPE_DIR_ENV]: dir, WSP_HOST_TOKEN: turn }, "cli")!;
    cli.in({ argv: ["projects", "add", `https://deploy:${login}@git.example/repo.git`] });
    cli.in({ argv: ["add", `https://host.example/?ticket=${ticket}`] });
    cli.in({ argv: ["exec", "t", "--", "echo", turn] });
    const ws = tapeOf({ [TAPE_DIR_ENV]: dir }, "ws", HOST_TOKEN)!;
    ws.in({ id: 7, op: "daemon.send", channel: "c1", frame: { id: 1, op: "pty.create", env: { OPENAI_API_KEY: env } } });
    ws.in({ id: 8, op: "daemon.send", channel: "c1", frame: { op: "pty.write", ptyId: "p1", data: `${typed}\r` } });
    const mcp = tapeOf({ [TAPE_DIR_ENV]: dir }, "mcp")!;
    mcp.in({ jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: "servers_add", arguments: { name: "gl", env: { GITLAB_TOKEN: glpat } } } });
    const text = readdirSync(dir).map(f => readFileSync(join(dir, f), "utf8")).join("");
    expect(secrets.filter(s => text.includes(s))).toEqual([]);
  });

  it("blanks the password in an address's login and a login shaped like a key, and leaves who signs in", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    const [password, key] = ["pw-in-a-login-7", "Zq9eK2vLpR7sTb4WnY8x0123"];
    tapeOf({ [TAPE_DIR_ENV]: dir }, "cli")!.in({ argv: ["projects", "add", `https://deploy:${password}@git.example/repo.git`, `https://${key}@host.example/x`, "ssh://git@github.com/acme/app.git", "https://dev@host.example/y"] });
    const text = readFileSync(join(dir, readdirSync(dir)[0]!), "utf8");
    expect([password, key].filter(s => text.includes(s))).toEqual([]);
    for (const kept of ["https://deploy:<redacted-", "ssh://git@github.com/acme/app.git", "https://dev@host.example/y"]) expect(text).toContain(kept);
  });

  it("blanks the host's own ticket and every secret a query or a fragment names", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    const secrets = ["ticket-value-1", "api-key-value-2", "password-value-3", "client-secret-4", "amz-signature-5", "sas-signature-6", "fragment-token-7"];
    const [ticket, apiKey, password, client, amz, sig, fragment] = secrets;
    const said = [
      `https://host.example/connect?ticket=${ticket}`,
      `https://api.example/v1?api_key=${apiKey}&user=dev`,
      `https://db.example/?password=${password}`,
      `https://auth.example/token?client_id=wsp&client_secret=${client}`,
      `https://bucket.s3.example/o?X-Amz-Date=20261025&X-Amz-Signature=${amz}`,
      `https://store.example/b?sv=2024&sig=${sig}`,
      `https://app.example/cb#access_token=${fragment}&state=x`,
    ].join(" ");
    tapeOf({ [TAPE_DIR_ENV]: dir }, "mcp")!.out({ jsonrpc: "2.0", id: 1, result: { content: [{ type: "text", text: said }] } });
    const text = readFileSync(join(dir, readdirSync(dir)[0]!), "utf8");
    expect(secrets.filter(s => text.includes(s))).toEqual([]);
    for (const kept of ["user=dev", "client_id=wsp", "X-Amz-Date=20261025", "sv=2024", "state=x"]) expect(text).toContain(kept);
  });

  it("blanks every value of an environment or a headers record and a secret-named field at any depth in a frame", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    const secrets = ["env-value-in-a-pty", "env-value-in-a-tool", "header-value-nested", "nested-api-key", "nested-client-secret", "nested-password"];
    const [ptyEnv, toolEnv, header, apiKey, client, password] = secrets;
    const ws = tapeOf({ [TAPE_DIR_ENV]: dir }, "ws", HOST_TOKEN)!;
    ws.in({ id: 7, op: "daemon.send", channel: "c1", frame: { id: 1, op: "pty.create", cols: 80, env: { PLAIN_NAME: ptyEnv } } });
    const mcp = tapeOf({ [TAPE_DIR_ENV]: dir }, "mcp")!;
    mcp.in({ jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: "servers_add", arguments: { name: "x", env: { PLAIN: toolEnv }, remote: { headers: { "X-Plain": header } } } } });
    mcp.in({ jsonrpc: "2.0", id: 3, method: "tools/call", params: { arguments: { config: { apiKey, client_secret: client, login: { password } } } } });
    const text = readdirSync(dir).map(f => readFileSync(join(dir, f), "utf8")).join("");
    expect(secrets.filter(s => text.includes(s))).toEqual([]);
    for (const kept of ["PLAIN_NAME", "X-Plain", "\"cols\":80"]) expect(text).toContain(kept);
  });

  it("withholds whatever is typed into a terminal, however deep the frame that carries it", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    const typed = "hunter2-at-a-sudo-prompt";
    const ws = tapeOf({ [TAPE_DIR_ENV]: dir }, "ws", HOST_TOKEN)!;
    ws.in({ id: 8, op: "daemon.send", channel: "c1", frame: { op: "pty.write", ptyId: "p1", data: `${typed}\r` } });
    ws.in({ id: 9, op: "daemon.send", channel: "c1", frame: { wrapped: { op: "pty.write", ptyId: "p1", data: typed } } });
    const lines = readFileSync(join(dir, readdirSync(dir)[0]!), "utf8").trimEnd().split("\n").slice(1);
    expect(lines.join("\n")).not.toContain(typed);
    expect(JSON.parse(lines[0]!)).toEqual({ in: { id: 8, op: "daemon.send", channel: "c1", frame: { op: "pty.write", ptyId: "p1", data: "<withheld: typed>" } } });
    expect(JSON.parse(lines[1]!).in.frame.wrapped.data).toBe("<withheld: typed>");
  });

  it("marks a thread's host token off the environment on the command line, and blanks every secret-named value the environment holds", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    const [token, turn, key] = ["fedcba9876543210fedcba9876543210", "turn-of-the-thread-1", "plain-words-for-a-key"];
    const env = { [TAPE_DIR_ENV]: dir, WSP_HOST_TOKEN: token, WSP_TURN: turn, ACME_API_KEY: key, SHORT_KEY: "true" };
    tapeOf(env, "cli")!.in({ argv: ["exec", "t", "--", "echo", token] });
    tapeOf(env, "mcp")!.out({ jsonrpc: "2.0", id: 1, result: { content: [{ type: "text", text: `turn ${turn} read ${key} and ${token}` }], said: "true" } });
    const text = readdirSync(dir).map(f => readFileSync(join(dir, f), "utf8")).join("");
    expect([token, turn, key].filter(s => text.includes(s))).toEqual([]);
    expect(text).toContain(JSON.stringify({ in: { argv: ["exec", "t", "--", "echo", "<host-token>"] } }));
    expect(text).toContain("and <host-token>");
    expect(text).toContain("\"said\":\"true\"");
  });

  it("blanks the token shapes the host's cuts know: GitLab's, Google's, a JWT and a private key through its END line", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    // Each built at run time, so a scanner reading the source takes none of them for a live token.
    const glpat = ["glpat", "Zq9fK2vLpR7sTb4WnY8x"].join("-");
    const google = "AIzaSyZq9fK2vLpR7sTb4WnY8x0123456789ab";
    const jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ3c3AifQ.c2lnbmF0dXJlLWhlcmU";
    const body = "MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC";
    const pem = `-----BEGIN RSA PRIVATE KEY-----\n${body}\n-----END RSA PRIVATE KEY-----`;
    tapeOf({ [TAPE_DIR_ENV]: dir }, "mcp")!.out({ jsonrpc: "2.0", id: 1, result: { content: [{ type: "text", text: `${glpat} ${google} ${jwt}\n${pem}\nafter the key` }] } });
    const text = readFileSync(join(dir, readdirSync(dir)[0]!), "utf8");
    expect([glpat, google, jwt, body, "PRIVATE KEY"].filter(s => text.includes(s))).toEqual([]);
    expect(text).toContain("after the key");
  });

  it("leaves a range of numbers that reads like a join code, blanks a real code, and writes its tape for the owner alone", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    tapeOf({ [TAPE_DIR_ENV]: dir }, "ws")!.out({ id: 2, ok: true, said: "from 2025-2026 on ports 3000-3999, code K7QX-M2PA, join 1234-5678.SHA256:abc" });
    const file = join(dir, readdirSync(dir)[0]!);
    const text = readFileSync(file, "utf8");
    for (const kept of ["2025-2026", "3000-3999"]) expect(text).toContain(kept);
    for (const code of ["K7QX-M2PA", "1234-5678"]) expect(text).not.toContain(code);
    expect(statSync(file).mode & 0o777).toBe(0o600);
  });

  it("writes nothing when no folder is named", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    expect(tapeOf({}, "ws")).toBeUndefined();
    expect(readdirSync(dir)).toEqual([]);
  });

  it("blanks the secrets a day carries past the seven kinds: AWS's secret field, Stripe, npm and Hugging Face tokens, a password env name, an API key header and a pass query", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    const word = (n: string) => `Zq9${n}mK2vLpR7sTb4WnY8x0123456789ab`;
    const [aws, stripe, npm, hf, dbPass, mysqlPwd, header, pass] = [
      word("a"),
      ["sk", "live", word("b")].join("_"),
      ["npm", word("c")].join("_"),
      ["hf", word("d")].join("_"),
      word("e"),
      word("f"),
      word("g"),
      word("h"),
    ];
    const ws = tapeOf({ [TAPE_DIR_ENV]: dir, DB_PASS: dbPass, MYSQL_PWD: mysqlPwd }, "ws", HOST_TOKEN)!;
    ws.in({ id: 2, op: "servers.add", server: { name: "s3", config: { awsSecretAccessKey: aws } } });
    ws.in({ id: 3, op: "sessions.start", workspaceId: "ws_1", prompt: `use ${stripe}, ${npm} and ${hf}; the db takes ${dbPass} and mysql ${mysqlPwd}` });
    ws.in({ id: 4, op: "sessions.start", workspaceId: "ws_1", prompt: `curl -H "X-Api-Key: ${header}" "https://api.example/v1?user=me&pass=${pass}"` });
    const text = readFileSync(join(dir, readdirSync(dir)[0]!), "utf8");
    expect([aws, stripe, npm, hf, dbPass, mysqlPwd, header, pass].filter(s => text.includes(s))).toEqual([]);
  });

  const value = "Zq9mK2vLpR7sTb4WnY8x0123456789ab";
  it.each([
    ["a field whose name ends in a secret's", {}, { op: "servers.add", server: { config: { awsSecretAccessKey: value } } }, value],
    ["a Stripe key", {}, { op: "sessions.start", prompt: `use ${["sk", "live", value].join("_")}` }, ["sk", "live", value].join("_")],
    ["an npm token", {}, { op: "sessions.start", prompt: `use ${["npm", value].join("_")}` }, ["npm", value].join("_")],
    ["a Hugging Face token", {}, { op: "sessions.start", prompt: `use ${["hf", value].join("_")}` }, ["hf", value].join("_")],
    ["a variable named for a password", { DB_PASS: value }, { op: "sessions.start", prompt: `the db takes ${value}` }, value],
    ["a variable named PWD after a program", { MYSQL_PWD: value }, { op: "sessions.start", prompt: `mysql takes ${value}` }, value],
    ["a variable named for a secret, a token or a key", { STRIPE_SECRET_X: value }, { op: "sessions.start", prompt: `KEYRING_DATA=${value}x ${value}` }, value],
    ["an API key header", {}, { op: "sessions.start", prompt: `curl -H "X-Api-Key: ${value}" https://api.example/v1` }, value],
    ["a pass query", {}, { op: "sessions.start", prompt: `https://api.example/v1?user=me&pass=${value}` }, value],
  ])("blanks %s", (_, env: Record<string, string>, frame: Record<string, unknown>, secret: string) => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    tapeOf({ [TAPE_DIR_ENV]: dir, ...env }, "ws", HOST_TOKEN)!.in({ id: 2, ...frame });
    expect(readFileSync(join(dir, readdirSync(dir)[0]!), "utf8")).not.toContain(secret);
  });

  it("keeps the folder the shell's PWD and OLDPWD name", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-tape-"));
    tapeOf({ [TAPE_DIR_ENV]: dir, PWD: "/home/dev/project", OLDPWD: "/home/dev/elsewhere" }, "ws", HOST_TOKEN)!.in({
      id: 2,
      op: "projects.add",
      source: "/home/dev/project",
      prompt: "cd /home/dev/elsewhere; PWD=/home/dev/project make",
    });
    const text = readFileSync(join(dir, readdirSync(dir)[0]!), "utf8");
    expect(text).toContain('"source":"/home/dev/project"');
    expect(text).toContain("cd /home/dev/elsewhere; PWD=/home/dev/project make");
  });
});
