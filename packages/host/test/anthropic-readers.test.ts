// SPDX-License-Identifier: AGPL-3.0-only
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { writeStub } from "../../protocol/test/stub-script.js";
import { claudeGateway, customHeadersOf, gatewayNames } from "../src/anthropic/gateway.js";
import { claudeHelper, HELPER_TTL_MS } from "../src/anthropic/helper.js";

let dir: string;
let config: string;
const settings = (value: object) => writeFileSync(join(config, "settings.json"), JSON.stringify(value));
const runs = (): string[] => {
  try {
    return readFileSync(join(dir, "helper.log"), "utf8").trim().split("\n").filter(Boolean);
  } catch {
    return [];
  }
};

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "wsp-claude-readers-"));
  config = join(dir, ".claude");
  mkdirSync(config);
  // The person's apiKeyHelper: notes where and with what it ran, then prints the key in helper.out.
  writeStub(join(dir, "helper"), `#!/bin/sh\necho "$(pwd) \${ACME_KEPT:-}" >> "${dir}/helper.log"\nsleep 0.1\ncat "${dir}/helper.out"\n`);
});
afterEach(() => rmSync(dir, { recursive: true, force: true }));

describe("the person's apiKeyHelper", () => {
  const env = () => ({ PATH: process.env.PATH, ACME_KEPT: "kept" });

  it("runs the helper in their settings on this computer, with their environment, and keeps its answer five minutes", async () => {
    settings({ apiKeyHelper: `${join(dir, "helper")} --from-settings` });
    writeFileSync(join(dir, "helper.out"), "helper-key-for-test\n");
    let t = 1_000_000;
    const credential = claudeHelper({ env: env(), home: dir, now: () => t });
    expect(await credential()).toEqual({ kind: "helper", value: "helper-key-for-test" });
    writeFileSync(join(dir, "helper.out"), "next-key-for-test\n");
    t += HELPER_TTL_MS - 1;
    expect(await credential()).toEqual({ kind: "helper", value: "helper-key-for-test" });
    t += 1;
    expect(await credential()).toEqual({ kind: "helper", value: "next-key-for-test" });
    expect(runs()).toHaveLength(2);
    for (const run of runs()) expect(run.endsWith(`${basename(dir)} kept`)).toBe(true);
  });

  it("reads the helper from CLAUDE_CONFIG_DIR's settings when it is set", async () => {
    mkdirSync(join(dir, "elsewhere"));
    writeFileSync(join(dir, "elsewhere", "settings.json"), JSON.stringify({ apiKeyHelper: join(dir, "helper") }));
    writeFileSync(join(dir, "helper.out"), "config-dir-key\n");
    expect(await claudeHelper({ env: { ...env(), CLAUDE_CONFIG_DIR: join(dir, "elsewhere") }, home: dir })()).toEqual({ kind: "helper", value: "config-dir-key" });
  });

  it("runs the helper once for requests that arrive together", async () => {
    settings({ apiKeyHelper: join(dir, "helper") });
    writeFileSync(join(dir, "helper.out"), "helper-key-for-test\n");
    const credential = claudeHelper({ env: env(), home: dir });
    expect(await Promise.all([credential(), credential(), credential()])).toEqual(Array(3).fill({ kind: "helper", value: "helper-key-for-test" }));
    expect(runs()).toHaveLength(1);
  });

  it("gives nothing with no helper, a helper that fails or one that prints nothing, and keeps no failure", async () => {
    const credential = claudeHelper({ env: env(), home: dir });
    expect(await credential()).toBeUndefined();
    settings({ apiKeyHelper: "exit 3" });
    expect(await credential()).toBeUndefined();
    settings({ apiKeyHelper: join(dir, "helper") });
    writeFileSync(join(dir, "helper.out"), "\n");
    expect(await credential()).toBeUndefined();
    writeFileSync(join(dir, "helper.out"), "late-key\n");
    expect(await credential()).toEqual({ kind: "helper", value: "late-key" });
  });
});

describe("the person's own gateway", () => {
  it("takes the URL, token and custom headers from their shell, with their settings env over it", async () => {
    const env = { ANTHROPIC_BASE_URL: "https://shell-gateway.acme.dev", ANTHROPIC_AUTH_TOKEN: "shell-token", ANTHROPIC_CUSTOM_HEADERS: "x-team: lab" };
    expect(await claudeGateway({ env, home: dir })()).toEqual({ kind: "gateway", url: "https://shell-gateway.acme.dev", token: "shell-token", headers: { "x-team": "lab" } });
    settings({ env: { ANTHROPIC_BASE_URL: "https://litellm.acme.dev/anthropic", ANTHROPIC_CUSTOM_HEADERS: "x-litellm-api-key: sk-for-test\nx-team: lab" } });
    expect(await claudeGateway({ env, home: dir })()).toEqual({ kind: "gateway", url: "https://litellm.acme.dev/anthropic", token: "shell-token", headers: { "x-litellm-api-key": "sk-for-test", "x-team": "lab" } });
    expect(await gatewayNames({ env, home: dir })).toEqual(["ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_CUSTOM_HEADERS"]);
  });

  it("reads values for each request, so a changed settings file is read at the next", async () => {
    const credential = claudeGateway({ env: {}, home: dir });
    expect(await credential()).toBeUndefined();
    settings({ env: { ANTHROPIC_BASE_URL: "https://gateway.acme.dev", ANTHROPIC_AUTH_TOKEN: "settings-token" } });
    expect(await credential()).toEqual({ kind: "gateway", url: "https://gateway.acme.dev", token: "settings-token", headers: {} });
  });

  it("is no gateway with no base URL, a base URL at Anthropic itself or one that does not parse", async () => {
    expect(await claudeGateway({ env: { ANTHROPIC_AUTH_TOKEN: "t" }, home: dir })()).toBeUndefined();
    expect(await claudeGateway({ env: { ANTHROPIC_BASE_URL: "https://api.anthropic.com", ANTHROPIC_AUTH_TOKEN: "t" }, home: dir })()).toBeUndefined();
    expect(await claudeGateway({ env: { ANTHROPIC_BASE_URL: "not a url" }, home: dir })()).toBeUndefined();
    expect(await gatewayNames({ env: {}, home: dir })).toEqual([]);
  });

  it("parses custom headers as Claude Code sends them, one per line", () => {
    expect(customHeadersOf("x-litellm-api-key: sk-for-test\nx-team: lab\n\nno colon here\n: no name")).toEqual({ "x-litellm-api-key": "sk-for-test", "x-team": "lab" });
  });

  it("refuses a custom header name that is not an HTTP token, naming it, or by its place when it is shaped like a token", async () => {
    const gateway = (headers: string) => claudeGateway({ env: { ANTHROPIC_BASE_URL: "https://gateway.acme.dev", ANTHROPIC_CUSTOM_HEADERS: headers }, home: dir })();
    expect(await gateway("x-team: lab\nx team: lab\nx-lab: acme")).toEqual({ kind: "gateway", url: "https://gateway.acme.dev", token: "", headers: { "x-team": "lab", "x team": "lab", "x-lab": "acme" }, invalidHeader: 'header "x team"' });
    const keyed = `x-api-key ${["sk", "ant", "api03", `fake${"0".repeat(28)}fake`].join("-")}`;
    expect(await gateway(`x-team: lab\n${keyed}: on`)).toMatchObject({ invalidHeader: "header 2 of 2" });
    expect(await gateway("x-team: lab")).not.toHaveProperty("invalidHeader");
  });
});
