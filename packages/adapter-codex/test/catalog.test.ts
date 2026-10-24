// SPDX-License-Identifier: AGPL-3.0-only
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { CATALOG_AGENTS } from "@wsp/catalog";
import { ENV_FROM_INPUT, type HarnessCatalogProbe } from "@wsp/protocol";
import { writeStub } from "../../protocol/test/stub-script.js";

import { createCodexAdapter } from "../src/adapter.js";
import { catalogProbeCommand, parseCatalogProbe } from "../src/catalog.js";
import { buildEnv, type CodexBroker } from "../src/command.js";

const LOGIN = "codex login --device-auth";
const SEP = "__WSP_CATALOG_SEP__";
/** The one wait the probe carries: how long its reader gives the server for one more line. */
const LINE_WAIT_MS = 10_000;

// codex-cli 0.153.0 on 2026-09-07: `codex --version`, `codex --help` and the app-server's answers to initialize,
// model/list, config/read and account/read on a CODEX_HOME with no sign-in, joined by the probe's separator. The
// answers come back out of order there, config/read last, which is why the parser reads each by its own id.
const REAL = readFileSync(new URL("./fixtures/catalog-probe.txt", import.meta.url), "utf8");
// The same binary under a config routed to OpenRouter: config/read names a model no OpenAI catalog carries, and
// account/read wants no OpenAI sign-in. Only the app-server section differs, so it is spliced onto the head above.
const ROUTE_SERVER = readFileSync(new URL("./fixtures/catalog-probe-route.jsonl", import.meta.url), "utf8");

const sections = (stdout: string): [string, string, string] => stdout.split(SEP) as [string, string, string];

/** The captured stdout with its app-server section replaced, so one reading of --version and --help serves every case. */
function probeOutput(server: readonly string[], head: string = REAL): string {
  const [version, help] = sections(head);
  return `${version}${SEP}${help}${SEP}\n${server.join("\n")}\n`;
}

const serverLines = (stdout: string): string[] => sections(stdout)[2].split("\n").filter(line => line.startsWith("{"));

/** The lists, or a failure naming what came back instead: every case here that is not about a refusal wants lists. */
function lists(stdout: string): HarnessCatalogProbe {
  const answer = parseCatalogProbe(stdout, LOGIN);
  if (answer === null || "refused" in answer) throw new Error(`expected the binary's lists, got ${JSON.stringify(answer)}`);
  return answer;
}

/** The same output with model/list answering an empty catalog, which 0.153.0 does under no config we have seen. */
function withoutModels(account: string): string {
  const kept = serverLines(REAL).filter(line => !line.startsWith('{"id":2') && !line.startsWith('{"id":4'));
  return probeOutput([...kept, '{"id":2,"result":{"data":[]}}', ...(account === "" ? [] : [account])]);
}

describe("catalogProbeCommand", () => {
  const dirs: string[] = [];
  afterEach(() => {
    for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
  });

  it("reads the base env a turn gets off its input, PATH and CODEX_HOME included, so a probe served by a bare-PATH exec still finds the binary", () => {
    const cmd = catalogProbeCommand();
    expect(cmd).toMatch(new RegExp(`^cd ~ && ${ENV_FROM_INPUT.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}; `));
    expect(cmd).not.toMatch(/PATH=|CODEX_HOME=/);
  });

  it("asks for the version, the help and the four app-server requests, and never a turn", () => {
    const cmd = catalogProbeCommand();
    expect(cmd).toContain("codex --version");
    expect(cmd).toContain("codex --help");
    expect(cmd).toContain("codex app-server");
    for (const method of ["initialize", "model/list", "config/read", "account/read"]) expect(cmd).toContain(`"method":"${method}"`);
    expect(cmd).not.toMatch(/codex exec|codex login|--dangerously/);
    // No fixed wait anywhere: this line is awaited at a session start. The only wait is the reader's, per line.
    expect(cmd).not.toMatch(/\bsleep\b/);
    expect(cmd).toContain(`-t ${String(LINE_WAIT_MS / 1000)} -u 4`);
    // The only pid killed is the one the shell recorded for the server, and never a bare 0, which is the group.
    expect(cmd).toContain('kill "$WSP_APP_SERVER_PID"');
    expect(cmd).not.toMatch(/pkill|killall|kill -|kill "?0/);
    // Nothing from bash 4 or later: the Mac's own bash is 3.2 and this line has to prove itself there too.
    expect(cmd).not.toMatch(/\bcoproc\b/);
  });

  it("holds the app-server's stdin open until its answers are in, and lets go on the last one", () => {
    // A fake codex on PATH that answers only once it has been sent all four requests, as the app-server does, and
    // then writes one line more. Every request reaching it proves the hold; the line after the fourth answer never
    // being read proves the probe let go there rather than waiting the server out.
    const dir = mkdtempSync(join(tmpdir(), "wsp-codex-probe-"));
    dirs.push(dir);
    const bin = join(dir, "bin");
    execFileSync("mkdir", [bin]);
    const answers = ['{"id":1,"result":{}}', '{"id":2,"result":{}}', '{"id":4,"result":{}}', '{"id":3,"result":{}}'];
    writeStub(
      join(bin, "codex"),
      [
        "#!/bin/sh",
        'if [ "$1" != "app-server" ]; then echo "CALL $1"; exit 0; fi',
        'n=0; while [ "$n" -lt 4 ] && read -r line; do n=$((n+1)); echo "GOT $line"; done',
        'echo "SERVER $CODEX_HOME"',
        ...answers.map(a => `echo '${a}'`),
        "echo PAST_THE_LAST_ANSWER",
        "",
      ].join("\n"),
    );
    const started = Date.now();
    const out = execFileSync("bash", ["-c", catalogProbeCommand()], { encoding: "utf8", env: { PATH: `${bin}:/usr/bin:/bin`, HOME: dir }, input: Object.entries(buildEnv({ home: "/root/.codex" })).map(([k, v]) => `${k}=${v}\0`).join("") });
    expect(out).toContain("CALL --version");
    expect(out).toContain("CALL --help");
    // The probe ran the server under the session's own home, and every request reached it while it was alive.
    expect(out).toContain("SERVER /root/.codex");
    for (const method of ["initialize", "model/list", "config/read", "account/read"]) expect(out).toContain(`"method":"${method}"`);
    expect(out.match(/^GOT /gm)).toHaveLength(4);
    // Out of order on purpose: the reader counts answers, so id 3 landing last still ends it.
    for (const answer of answers) expect(out).toContain(answer);
    expect(out).not.toContain("PAST_THE_LAST_ANSWER");
    // Nothing here waits on a clock: the whole probe is well inside the one line the reader is allowed to wait for.
    expect(Date.now() - started).toBeLessThan(LINE_WAIT_MS);
  });
});

describe("parseCatalogProbe", () => {
  it("reads the version, the models the app-server offers with their reasoning efforts, and the sandbox modes from the help", () => {
    const probe = lists(REAL);
    expect(probe.version).toBe("0.153.0");
    expect(probe.models.map(m => m.slug)).toEqual(["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna", "gpt-5.5", "gpt-5.2"]);
    expect(probe.models.map(m => m.label)).toEqual(["GPT-5.6-Sol", "GPT-5.6-Terra", "GPT-5.6-Luna", "GPT-5.5", "GPT-5.2"]);
    expect(probe.models.filter(m => m.isDefault).map(m => m.slug)).toEqual(["gpt-5.6-sol"]);
    expect(probe.models[0]?.description).toContain("frontier agentic coding model");
    expect(probe.models[0]?.efforts).toEqual(["low", "medium", "high", "xhigh", "max", "ultra"]);
    expect(probe.models[3]?.efforts).toEqual(["low", "medium", "high", "xhigh"]);
    // The effort each model runs at when a turn names none; the catalog marks the default model's.
    expect(probe.models.map(m => m.defaultEffort)).toEqual(["low", "medium", "medium", "medium", "medium"]);
    // No codex model runs at two context windows, so no model offers one.
    expect(probe.models.every(m => m.contextWindows.length === 0)).toBe(true);
    expect(probe.efforts).toEqual(["low", "medium", "high", "xhigh", "max", "ultra"]);
    expect(probe.permissionModes).toEqual(["read-only", "workspace-write", "danger-full-access"]);
  });

  it("marks a model fast where model/list names a fast speed tier for it", () => {
    const models = { id: 2, result: { data: [
      { id: "gpt-6-astra", displayName: "GPT-6-Astra", isDefault: true, hidden: false, supportedReasoningEfforts: [], additionalSpeedTiers: ["fast"] },
      { id: "gpt-5.2", displayName: "GPT-5.2", isDefault: false, hidden: false, supportedReasoningEfforts: [], additionalSpeedTiers: [] },
    ] } };
    const probe = lists(probeOutput([...serverLines(REAL).filter(line => !line.startsWith('{"id":2')), JSON.stringify(models)]));
    expect(probe.models.map(m => [m.slug, m.fast])).toEqual([["gpt-6-astra", true], ["gpt-5.2", undefined]]);
  });

  it("a route to another provider leads with the model config names, since that is what a turn without -m runs", () => {
    const probe = lists(probeOutput(ROUTE_SERVER.split("\n").filter(line => line.startsWith("{"))));
    expect(probe.models[0]).toEqual({ slug: "anthropic/claude-sonnet-4.5", label: "anthropic/claude-sonnet-4.5", contextWindows: [], isDefault: true });
    // Which efforts the provider takes is not codex's to say, so the model names no list and keeps every one of the
    // catalog's; an empty list would read as "takes none" and refuse them all.
    expect(probe.models[0]).not.toHaveProperty("efforts");
    expect(probe.models[0]).not.toHaveProperty("defaultEffort");
    expect(probe.models.filter(m => m.isDefault)).toHaveLength(1);
    expect(probe.models.map(m => m.slug).slice(1)).toEqual(["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna", "gpt-5.5", "gpt-5.2"]);
    expect(probe.efforts).toEqual(["low", "medium", "high", "xhigh", "max", "ultra"]);
  });

  it("is null when the app-server said nothing, and tolerates a missing version or help", () => {
    expect(parseCatalogProbe("", LOGIN)).toBeNull();
    expect(parseCatalogProbe(`x${SEP}y${SEP}not json\n`, LOGIN)).toBeNull();
    const bare = lists(probeOutput(serverLines(REAL), `${SEP}${SEP}`));
    expect(bare.version).toBeNull();
    expect(bare.permissionModes).toEqual([]);
    expect(bare.models).toHaveLength(5);
  });

  it("names the sign-in when the app-server answered and offered no model for want of one, and stays silent otherwise", () => {
    expect(parseCatalogProbe(withoutModels('{"id":4,"result":{"account":null,"requiresOpenaiAuth":true}}'), LOGIN)).toEqual({
      refused: `Codex is not signed in where this thread runs; run ${LOGIN} there`,
    });
    // Signed in, or routed to a provider that wants no OpenAI login: an empty catalog is not a sign-in to ask for.
    expect(parseCatalogProbe(withoutModels('{"id":4,"result":{"account":null,"requiresOpenaiAuth":false}}'), LOGIN)).toBeNull();
    expect(parseCatalogProbe(withoutModels('{"id":4,"result":{"account":{"id":"a"},"requiresOpenaiAuth":true}}'), LOGIN)).toBeNull();
    expect(parseCatalogProbe(withoutModels(""), LOGIN)).toBeNull();
  });

  it("leaves out a model the app-server hides, and reads each answer by its own id whatever order they land in", () => {
    const hidden = serverLines(REAL).map(line => {
      const message = JSON.parse(line) as { id?: number; result?: { data?: { id: string; hidden: boolean }[] } };
      if (message.id !== 2) return line;
      for (const model of message.result?.data ?? []) if (model.id === "gpt-5.5") model.hidden = true;
      return JSON.stringify(message);
    });
    const probe = lists(probeOutput([...hidden].reverse()));
    expect(probe.models.map(m => m.slug)).toEqual(["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna", "gpt-5.2"]);
    expect(probe.version).toBe("0.153.0");
  });
});

describe("a probe on a machine whose ChatGPT sign-in the host holds", () => {
  const CLAIMS = { accountId: "11111111-2222-4333-8444-555555555555", planType: "plus" };
  const levels = (...efforts: string[]) => efforts.map(effort => ({ effort, description: `${effort} effort` }));
  // ChatGPT's /backend-api/codex/models answer for codex-cli 0.155.1 on 2026-10-16, cut to the fields the picker reads,
  // out of codex's order: it sorts by priority, shows only the rows marked list, and makes the first shown the default.
  const ACCOUNT = {
    models: [
      { slug: "gpt-6-luna", display_name: "GPT-6-Luna", visibility: "list", priority: 4, default_reasoning_level: "medium", supported_reasoning_levels: levels("low", "medium", "high"), additional_speed_tiers: ["fast"] },
      { slug: "gpt-reserve", display_name: "GPT-Reserve", visibility: "hide", priority: 4, supported_reasoning_levels: levels("low") },
      { slug: "gpt-6-astra", display_name: "GPT-6-Astra", description: "Frontier model", visibility: "list", priority: 2, default_reasoning_level: "low", supported_reasoning_levels: levels("low", "medium", "high", "xhigh", "max", "ultra"), additional_speed_tiers: ["fast"] },
      { slug: "gpt-6-sol", display_name: "GPT-6-Sol", visibility: "list", priority: 3, default_reasoning_level: "medium", supported_reasoning_levels: levels("low", "medium", "high", "xhigh"), additional_speed_tiers: [] },
    ],
  };
  const probeOn = async (broker: CodexBroker | undefined) => {
    const adapter = createCodexAdapter({ exec: () => Promise.reject(new Error("no turn here")) as never, home: "/root/.codex", login: LOGIN, ...(broker !== undefined ? { broker } : {}) });
    return adapter.probeCatalog(() => Promise.resolve(REAL));
  };
  const listsOf = (answer: Awaited<ReturnType<typeof probeOn>>): HarnessCatalogProbe => {
    if (answer === null || "refused" in answer) throw new Error(`expected lists, got ${JSON.stringify(answer)}`);
    return answer;
  };

  it("lists the account's own models, read through the host for the binary's version, in codex's order with its first shown the default", async () => {
    const asked: string[] = [];
    const probe = listsOf(await probeOn({ mode: "chatgpt", claims: CLAIMS, models: version => (asked.push(version), Promise.resolve(ACCOUNT)) }));
    expect(asked).toEqual(["0.153.0"]);
    expect(probe.models.map(m => m.slug)).toEqual(["gpt-6-astra", "gpt-6-sol", "gpt-6-luna"]);
    expect(probe.models.filter(m => m.isDefault).map(m => m.slug)).toEqual(["gpt-6-astra"]);
    expect(probe.models[0]).toMatchObject({ label: "GPT-6-Astra", description: "Frontier model", defaultEffort: "low", fast: true });
    expect(probe.models[1]).toMatchObject({ slug: "gpt-6-sol", efforts: ["low", "medium", "high", "xhigh"], defaultEffort: "medium" });
    expect(probe.models[1]!.fast).toBeUndefined();
    expect(probe.efforts).toEqual(["low", "medium", "high", "xhigh", "max", "ultra"]);
    expect(probe.version).toBe("0.153.0");
  });

  it("keeps the binary's own list where the host's read fails, answers nothing or holds no shown model", async () => {
    const own = lists(REAL).models.map(m => m.slug);
    const reads = [
      () => Promise.reject(new Error("chatgpt.com unreachable")),
      () => Promise.resolve(undefined),
      () => Promise.resolve({ models: [] }),
      () => Promise.resolve({ models: [{ slug: "gpt-reserve", visibility: "hide", priority: 1 }] }),
      () => Promise.resolve({ detail: "Unauthorized" }),
    ];
    for (const models of reads) expect(listsOf(await probeOn({ mode: "chatgpt", claims: CLAIMS, models })).models.map(m => m.slug)).toEqual(own);
  });

  it("asks the host nothing on an API key, with no read handed, or where the binary printed no version", async () => {
    expect(listsOf(await probeOn({ mode: "apikey" })).models.map(m => m.slug)).toEqual(lists(REAL).models.map(m => m.slug));
    expect(listsOf(await probeOn({ mode: "chatgpt", claims: CLAIMS })).models.map(m => m.slug)).toEqual(lists(REAL).models.map(m => m.slug));
    let asked = 0;
    const adapter = createCodexAdapter({ exec: () => Promise.reject(new Error("no turn here")) as never, home: "/root/.codex", login: LOGIN, broker: { mode: "chatgpt", claims: CLAIMS, models: () => (asked++, Promise.resolve(ACCOUNT)) } });
    await adapter.probeCatalog(() => Promise.resolve(probeOutput(serverLines(REAL), `${SEP}${SEP}`)));
    expect(asked).toBe(0);
  });

  it("offers the account's models without the binary, read for wsp's npm pin, only where the host holds the ChatGPT sign-in", async () => {
    const asked: string[] = [];
    const make = (broker?: CodexBroker) => createCodexAdapter({ exec: () => Promise.reject(new Error("no turn here")) as never, home: "/root/.codex", login: LOGIN, ...(broker !== undefined ? { broker } : {}) });
    const held = make({ mode: "chatgpt", claims: CLAIMS, models: version => (asked.push(version), Promise.resolve(ACCOUNT)) });
    expect((await held.accountModels!())?.map(m => m.slug)).toEqual(["gpt-6-astra", "gpt-6-sol", "gpt-6-luna"]);
    const road = CATALOG_AGENTS.find(a => a.id === "codex")?.installRoad;
    expect(asked).toEqual([road?.road === "npm" ? road.version : "a pinned version"]);
    expect(await make({ mode: "chatgpt", claims: CLAIMS, models: () => Promise.reject(new Error("chatgpt.com unreachable")) }).accountModels!()).toBeUndefined();
    for (const broker of [undefined, { mode: "apikey" as const }, { mode: "chatgpt" as const, claims: CLAIMS }]) expect(make(broker).accountModels).toBeUndefined();
  });
});
