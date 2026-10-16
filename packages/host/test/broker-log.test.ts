// SPDX-License-Identifier: AGPL-3.0-only
// The broker's log and its credentials. Every call through the rig leaves one
// line holding each field the broker policy names; the file is then scanned for
// every token shape and for each secret, body, query and header value the calls
// carried, none of which may be in it. The credentials are read where each
// lives, the gh login once for calls that arrive together and kept 5 minutes.
import { mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { brokerCredentials, GITHUB_CACHE_MS, type RunCommand } from "../src/broker/credentials.js";
import { brokerLogFile, cleanLine, LOG_KEEP_MS } from "../src/broker/log.js";
import type { AnthropicCredential, BrokerLogLine, BrokerService } from "../src/broker/types.js";
import { responseOn } from "../src/chatgpt/forward.js";
import { tokenShaped } from "../src/token-shapes.js";
import { CLAUDE, OP, REAL, TERMINAL, TURN, WS, pipePair, placeholderJwt, proxyLogin, rig, type Rig } from "./broker-fixture.js";

let r: Rig | undefined;
const dirs: string[] = [];
afterEach(async () => {
  await r?.close();
  r = undefined;
  for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true });
});

const FIELDS = ["at", "machine", "thread", "token", "service", "method", "path", "status", "bytesUp", "bytesDown", "ms", "heldMs"];

const readLog = (dir: string): { text: string; lines: BrokerLogLine[] } => {
  const text = readdirSync(dir).map(f => readFileSync(join(dir, f), "utf8")).join("");
  return { text, lines: text.split("\n").filter(Boolean).map(l => JSON.parse(l) as BrokerLogLine) };
};

const UPGRADE = "Connection: Upgrade\r\nUpgrade: websocket\r\n";

/** Writes one request head on a fresh pipe and never ends its own side, as a machine's client waiting on an answer
 * does. Answers the status it read, how long until the pipe closed (nothing past `waitMs`) and the log then. */
async function held(r: Rig, head: string, waitMs: number): Promise<{ status: number; closedMs?: number; lines: BrokerLogLine[] }> {
  const [host, client] = pipePair();
  const started = Date.now();
  const closed = new Promise<number>(resolve => host.once("close", () => resolve(Date.now() - started)));
  r.broker.take(WS, { op: "pipe.open", door: "http", heldMs: 0 }, host);
  let seen = "";
  const status = new Promise<number>(resolve =>
    client.on("data", (d: Buffer) => {
      seen += d.toString("latin1");
      const got = /^HTTP\/1\.1 (\d{3})/.exec(seen)?.[1];
      if (got !== undefined) resolve(Number(got));
    }),
  );
  client.write(head);
  try {
    const timer = new Promise<undefined>(resolve => setTimeout(resolve, waitMs));
    const [answered, closedMs] = await Promise.all([Promise.race([status, timer]), Promise.race([closed, timer])]);
    return { status: answered ?? 0, ...(closedMs !== undefined ? { closedMs } : {}), lines: readLog(r.logDir).lines };
  } finally {
    client.destroy();
  }
}

describe("the log", () => {
  it("writes a refused call's line when it is refused and closes its pipe then, however long the machine waits", async () => {
    const signedOut: BrokerService = {
      id: "chatgpt",
      claims: (_req, door) => door === "http",
      serve: async ({ res }) => void res.end(),
      upgrade: async (q, socket, _head, ctx) => ctx.refuse(responseOn(q.req, socket), 403, "wsp: signed out.", "openai"),
    };
    r = await rig();
    const service = await rig({ services: [signedOut] });
    try {
      const threadless = await held(r, `GET /backend-api/codex/responses HTTP/1.1\r\nHost: 127.0.0.1:7071\r\n${UPGRADE}\r\n`, 1_000);
      const connect = await held(r, "CONNECT api.example.com:443 HTTP/1.1\r\nHost: api.example.com:443\r\n\r\n", 1_000);
      const refused = await held(service, `GET /backend-api/codex/responses HTTP/1.1\r\nHost: 127.0.0.1:7071\r\nAuthorization: Bearer ${placeholderJwt(TURN)}\r\n${UPGRADE}\r\n`, 1_000);
      expect([threadless.status, threadless.closedMs !== undefined]).toEqual([403, true]);
      expect(threadless.lines.map(l => [l.method, l.path, l.status])).toEqual([["GET", "/backend-api/codex/responses", 403]]);
      expect([connect.status, connect.closedMs !== undefined]).toEqual([403, true]);
      expect(connect.lines.map(l => [l.method, l.status])).toEqual([["GET", 403], ["CONNECT", 403]]);
      expect([refused.status, refused.closedMs !== undefined]).toEqual([403, true]);
      expect(refused.lines.map(l => [l.token, l.service, l.status])).toEqual([["turn", "chatgpt", 403]]);
    } finally {
      await service.close();
    }
  });

  it("keeps an answered call's whole time in its line, an upgrade's until its connection closes", async () => {
    const slow: BrokerService = {
      id: "anthropic",
      claims: (_req, door) => door === "http",
      async serve({ res }) {
        await new Promise(resolve => setTimeout(resolve, 200));
        res.writeHead(200, { "content-length": "2" }).end("ok");
      },
    };
    r = await rig();
    const answered = await rig({ services: [slow] });
    try {
      await answered.call({ method: "POST", path: "/v1/messages", headers: { ...CLAUDE, "x-api-key": TURN }, body: "{}" });
      const [line] = readLog(answered.logDir).lines;
      expect([line?.status, line!.ms >= 190]).toEqual([200, true]);

      const upgraded = await held(r, `GET /backend-api/codex/responses HTTP/1.1\r\nHost: 127.0.0.1:7071\r\nAuthorization: Bearer ${placeholderJwt(TURN)}\r\n${UPGRADE}\r\n`, 300);
      expect([upgraded.status, upgraded.closedMs, upgraded.lines]).toEqual([101, undefined, []]);
      await new Promise(resolve => setTimeout(resolve, 50));
      const [open] = readLog(r.logDir).lines;
      expect([open?.service, open?.status, open!.ms >= 290]).toEqual(["chatgpt", 101, true]);
    } finally {
      await answered.close();
    }
  });
  it("keeps one line per call with every field the policy names and no body, query, header value or credential", async () => {
    const pushing: BrokerService = {
      id: "git",
      claims: (req, door) => door === "http" && (req.url ?? "").includes("git-receive-pack"),
      async serve({ res }, ctx) {
        await ctx.credential("github");
        ctx.log({ pushes: [{ ref: "refs/heads/wsp/fix-tests", old: "a".repeat(40), new: "b".repeat(40), verdict: "ok" }] });
        // A service's own fields past the list never reach the line.
        ctx.log({ path: `/leak?${REAL.github}`, body: "BODY-SECRET" } as Partial<BrokerLogLine>);
        res.writeHead(200, { "content-type": "application/x-git-receive-pack-result" });
        res.end("0000");
      },
    };
    r = await rig();
    const withPush = await rig({ services: [pushing] });
    const body = JSON.stringify({ messages: [{ role: "user", content: `BODY-MARKER ${REAL.anthropic}` }] });
    const header = { "anthropic-beta": "HEADER-VALUE-MARKER", "user-agent": "claude-cli/2.1.296 HEADER-UA-MARKER" };
    await r.call({ method: "POST", path: "/v1/messages?beta=true&q=QUERY-MARKER", headers: { ...CLAUDE, ...header, "x-api-key": TURN }, body, heldMs: 2_000 });
    await r.call({ method: "POST", path: "/backend-api/codex/responses?q=QUERY-MARKER", headers: { authorization: `Bearer ${placeholderJwt(TURN)}`, cookie: "COOKIE-MARKER=1" }, body });
    await r.call({ door: "gh", path: "/repos/acme/lab?per_page=QUERY-MARKER", headers: { host: "api.github.com", authorization: `token ${TERMINAL}` } });
    await r.call({ method: "POST", path: "/v1/messages", headers: CLAUDE, body });
    await withPush.call({ method: "POST", path: "http://github.com/acme/lab.git/git-receive-pack?QUERY-MARKER", headers: { host: "github.com", "proxy-authorization": proxyLogin(OP) }, body: "0000PACK-BODY-MARKER" });
    const { text, lines } = readLog(r.logDir);
    const pushLog = readLog(withPush.logDir);
    await withPush.close();

    expect(lines).toHaveLength(4);
    for (const line of lines.slice(0, 3)) for (const field of FIELDS) expect([field, line[field as keyof BrokerLogLine]]).not.toEqual([field, undefined]);
    expect(lines.map(l => [l.machine, l.thread, l.token, l.service, l.method, l.path, l.status])).toEqual([
      [WS, "th-acme-1", "turn", "anthropic", "POST", "/v1/messages", 200],
      [WS, "th-acme-1", "turn", "chatgpt", "POST", "/backend-api/codex/responses", 200],
      [WS, "th-acme-1", "terminal", "gh", "GET", "/repos/acme/lab", 200],
      [WS, undefined, undefined, undefined, "POST", "/v1/messages", 403],
    ]);
    expect(lines[0]!.heldMs).toBe(2_000);
    expect(lines[0]!.bytesUp).toBeGreaterThan(body.length);
    expect(lines[0]!.bytesDown).toBeGreaterThan(0);
    expect(Number.isNaN(Date.parse(lines[0]!.at))).toBe(false);

    expect(pushLog.lines).toHaveLength(1);
    expect(pushLog.lines[0]).toMatchObject({ token: "op", service: "git", path: "github.com/acme/lab.git/git-receive-pack", pushes: [{ ref: "refs/heads/wsp/fix-tests", old: "a".repeat(40), new: "b".repeat(40), verdict: "ok" }] });

    const all = text + pushLog.text;
    expect(tokenShaped(all)).toBe(false);
    for (const secret of [REAL.anthropic, REAL.openai, REAL.github, TURN, OP, TERMINAL, "QUERY-MARKER", "BODY-MARKER", "PACK-BODY-MARKER", "BODY-SECRET", "HEADER-VALUE-MARKER", "HEADER-UA-MARKER", "COOKIE-MARKER", "?", "/leak"]) expect([secret, all.includes(secret)]).toEqual([secret, false]);
    expect(statSync(join(r.logDir, readdirSync(r.logDir)[0]!)).mode & 0o777).toBe(0o600);
  });

  it("blanks a token shape a caller put in a path or a ref, and keeps only the fields on the list", () => {
    const line = cleanLine({
      at: "2026-10-13T00:00:00.000Z",
      machine: WS,
      method: "GET",
      path: `/v1/models/${REAL.anthropic}?key=${REAL.openai}`,
      status: 200,
      bytesUp: 1,
      bytesDown: 2,
      ms: 3,
      heldMs: 0,
      pushes: [{ ref: `refs/heads/${REAL.github}`, old: "not-a-sha", new: "c".repeat(40), verdict: "ok" }],
      ...({ authorization: `Bearer ${REAL.github}` } as object),
    } as BrokerLogLine);
    const text = JSON.stringify(line);
    expect(tokenShaped(text)).toBe(false);
    expect(text).not.toContain("authorization");
    expect(line.path).toBe("/v1/models/<redacted>");
    expect(line.pushes).toEqual([{ ref: "refs/heads/<redacted>", old: "", new: "c".repeat(40), verdict: "ok" }]);
  });

  it("writes one file per day and deletes the days past 14", () => {
    const dir = mkdtempSync(join(tmpdir(), "wsp-broker-days-"));
    dirs.push(dir);
    const day = Date.parse("2026-10-01T12:00:00.000Z");
    let now = day;
    const line = (): BrokerLogLine => ({ at: new Date(now).toISOString(), machine: WS, method: "GET", path: "/", status: 200, bytesUp: 0, bytesDown: 0, ms: 0, heldMs: 0 });
    writeFileSync(join(dir, "notes.txt"), "not the log's");
    const log = brokerLogFile(dir, () => now);
    log.write(line());
    now = day + 3 * 86_400_000;
    log.write(line());
    expect(readdirSync(dir).sort()).toEqual(["2026-10-01.jsonl", "2026-10-04.jsonl", "notes.txt"]);
    now = day + LOG_KEEP_MS + 2 * 86_400_000;
    log.write(line());
    expect(readdirSync(dir).sort()).toEqual(["2026-10-04.jsonl", "2026-10-17.jsonl", "notes.txt"]);
  });
});

describe("credentials", () => {
  const ghRun = (runs: string[][], o: { token?: string; delayMs?: number } = {}): RunCommand => async (_file, args) => {
    runs.push([...args]);
    await new Promise(res => setTimeout(res, o.delayMs ?? 0));
    if (args[1] === "token") return o.token === undefined ? { code: 1, stdout: "" } : { code: 0, stdout: `${o.token}\n` };
    return { code: 0, stdout: JSON.stringify({ hosts: { "github.com": [{ state: "success", login: "acme-dev" }] } }) };
  };

  it("reads the agents' keys from the vault at each call, a key ahead of a token as Claude Code ranks them", async () => {
    let vault: Record<string, string> = { CLAUDE_CODE_OAUTH_TOKEN: "oauth-placeholder-value" };
    const c = brokerCredentials({ vault: () => vault, run: ghRun([]) });
    expect(await c.credential("anthropic")).toEqual({ kind: "token", token: "oauth-placeholder-value" });
    vault = { ...vault, ANTHROPIC_API_KEY: REAL.anthropic, OPENAI_API_KEY: REAL.openai };
    expect(await c.credential("anthropic")).toEqual({ kind: "key", key: REAL.anthropic });
    expect(await c.credential("openai")).toEqual({ key: REAL.openai });
    expect(await c.credential("chatgpt")).toBeUndefined();
  });

  it("reads gh once for calls that arrive together, and again only once five minutes have passed", async () => {
    const runs: string[][] = [];
    let now = 0;
    const c = brokerCredentials({ vault: () => ({}), run: ghRun(runs, { token: REAL.github, delayMs: 20 }), now: () => now });
    const [a, b] = await Promise.all([c.credential("github"), c.credential("github")]);
    expect(a).toEqual({ token: REAL.github });
    expect(b).toEqual(a);
    const tokenReads = (): number => runs.filter(x => x[1] === "token").length;
    expect(tokenReads()).toBe(1);
    expect(runs.filter(x => x[1] === "status")).toEqual([]);
    expect(runs.find(x => x[1] === "token")).toEqual(["auth", "token", "--hostname", "github.com"]);
    now = GITHUB_CACHE_MS - 1;
    await c.credential("github");
    expect(tokenReads()).toBe(1);
    now = GITHUB_CACHE_MS;
    await c.credential("github");
    expect(tokenReads()).toBe(2);
  });

  it("keeps no missing gh login, so a sign-in a moment later is read on the next call", async () => {
    const runs: string[][] = [];
    const c = brokerCredentials({ vault: () => ({}), run: ghRun(runs) });
    expect(await c.credential("github")).toBeUndefined();
    expect(await c.credential("github")).toBeUndefined();
    expect(runs.filter(x => x[1] === "token")).toHaveLength(2);
  });

  it("says the kind each credential is, never its value", async () => {
    const c = brokerCredentials({
      vault: () => ({ ANTHROPIC_API_KEY: REAL.anthropic }),
      chatgpt: async () => ({ accessToken: placeholderJwt("access"), accountId: "acct-acme" }),
      run: ghRun([], { token: REAL.github }),
    });
    const status = await c.status();
    expect(status).toEqual({ claude: "key", codex: "chatgpt", github: "acme-dev" });
    const bare = brokerCredentials({ vault: () => ({ CLAUDE_CODE_OAUTH_TOKEN: "oauth-placeholder-value", OPENAI_API_KEY: REAL.openai }), run: ghRun([]) });
    expect(await bare.status()).toEqual({ claude: "token", codex: "key" });
    expect(tokenShaped(JSON.stringify(status))).toBe(false);
  });

  it("asks GitHub for the account once per five minutes, however often the status is read", async () => {
    const runs: string[][] = [];
    let now = 0;
    const c = brokerCredentials({ vault: () => ({}), run: ghRun(runs, { token: REAL.github }), now: () => now });
    const accountReads = (): number => runs.filter(x => x[1] === "status").length;
    expect(await c.status()).toEqual({ github: "acme-dev" });
    expect(await c.status()).toEqual({ github: "acme-dev" });
    expect(accountReads()).toBe(1);
    now = GITHUB_CACHE_MS;
    expect(await c.status()).toEqual({ github: "acme-dev" });
    expect(accountReads()).toBe(2);
  });

  it("tries the person's own Anthropic sign-in first, then the vault's key or setup-token", async () => {
    let own: AnthropicCredential | undefined = { kind: "helper", value: "helper-output" };
    const c = brokerCredentials({ vault: () => ({ ANTHROPIC_API_KEY: REAL.anthropic }), anthropic: async () => own, run: ghRun([]) });
    expect(await c.credential("anthropic")).toEqual({ kind: "helper", value: "helper-output" });
    expect(await c.status()).toEqual({ claude: "key" });
    own = { kind: "token", token: "claude-ai-access", expiresAt: 4102444800000 };
    expect(await c.credential("anthropic")).toEqual(own);
    expect(await c.status()).toEqual({ claude: "login" });
    own = undefined;
    expect(await c.credential("anthropic")).toEqual({ kind: "key", key: REAL.anthropic });
    expect(await c.status()).toEqual({ claude: "key" });
    const tokenOnly = brokerCredentials({ vault: () => ({ CLAUDE_CODE_OAUTH_TOKEN: "setup-token-value" }), anthropic: async () => undefined, run: ghRun([]) });
    expect(await tokenOnly.credential("anthropic")).toEqual({ kind: "token", token: "setup-token-value" });
    expect(await tokenOnly.status()).toEqual({ claude: "token" });
  });
});
