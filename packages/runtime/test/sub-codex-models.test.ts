// SPDX-License-Identifier: AGPL-3.0-only
// A Codex sub-thread on Boat started on a model the person's account offers, where the codex on its machine has
// described nothing yet: the start is checked against the account's list, which the host reads with the ChatGPT
// sign-in it holds, and against wsp's built-in list only where that read fails.
import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
import { CATALOG_AGENTS } from "@wsp/catalog";
import { subOpening } from "@wsp/protocol";
import { HARNESS_ADAPTERS } from "../src/adapters.js";
import type { HarnessAdapterFactory, RuntimeOptions } from "../src/runtime.js";
import { leadOnThisComputer } from "./sub-fixture.js";

const ACCOUNT = { models: [{ slug: "gpt-6-sol", display_name: "GPT-6-Sol", visibility: "list", priority: 3, supported_reasoning_levels: [{ effort: "medium", description: "" }] }] };
const road = CATALOG_AGENTS.find(a => a.id === "codex")?.installRoad;
const INSTALLED = road?.road === "npm" ? road.version : undefined;

/** The fixture's turns under the real Codex adapter's lists. */
const realLists = (stub: HarnessAdapterFactory): HarnessAdapterFactory => ctx => {
  const real = HARNESS_ADAPTERS.codex(ctx);
  return { ...stub(ctx), ...(real.probeCatalog !== undefined ? { probeCatalog: real.probeCatalog } : {}), ...(real.accountModels !== undefined ? { accountModels: real.accountModels } : {}) };
};

async function startOn(codexModels: (version: string) => Promise<unknown>) {
  const broker: RuntimeOptions["broker"] = { ready: () => true, codexClaims: async () => ({ accountId: "acct-acme" }), status: async () => ({ codex: "chatgpt" }), codexModels };
  const { rt, project, lead, starts } = await leadOnThisComputer({ broker: true, hostGh: true, codex: realLists, host: { broker } });
  try {
    const made = await rt.workspaces.create({ project: project.id, name: "fix", sub: { on: "boat", harness: "codex" } }, lead);
    const turn = await rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Fix the flaky upload test."), harness: "codex", model: "gpt-6-sol", cwd: made.sub!.folder }, lead);
    await turn.finished;
    return { model: starts.at(-1)?.o.model };
  } catch (e) {
    return { refused: (e as Error).message };
  } finally {
    await rt.close();
  }
}

it("starts a Codex sub-thread on a model the account offers, read for wsp's npm pin, before any binary there has described itself", async () => {
  const asked: string[] = [];
  expect(await startOn(async version => (asked.push(version), ACCOUNT))).toEqual({ model: "gpt-6-sol" });
  expect(INSTALLED).toBeDefined();
  expect(asked).toEqual([INSTALLED]);
}, 40_000);

it("checks the start against wsp's built-in list where the host could not read the account's", async () => {
  const said = await startOn(() => Promise.reject(new Error("chatgpt.com unreachable")));
  expect(said.refused).toContain(`model "gpt-6-sol" is not one codex takes; one of: GPT-5.6-Sol (gpt-5.6-sol)`);
}, 40_000);

const effort = [{ effort: "low", description: "" }];
const row = (slug: string, priority: number) => ({ slug, display_name: slug, visibility: "list", priority, supported_reasoning_levels: effort });
const PROBE = readFileSync(new URL("../../adapter-codex/test/fixtures/catalog-probe.txt", import.meta.url), "utf8");

async function boatWith(codexModels: (version: string) => Promise<unknown>) {
  const broker: RuntimeOptions["broker"] = { ready: () => true, codexClaims: async () => ({ accountId: "acct-acme" }), status: async () => ({ codex: "chatgpt" }), codexModels };
  const f = await leadOnThisComputer({ broker: true, hostGh: true, codex: realLists, host: { broker } });
  let n = 0;
  const sub = async (model?: string) => {
    const made = await f.rt.workspaces.create({ project: f.project.id, name: `fix-${++n}`, sub: { on: "boat", harness: "codex" } }, f.lead);
    const turn = await f.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Fix it."), harness: "codex", ...(model !== undefined ? { model } : {}), cwd: made.sub!.folder }, f.lead);
    await turn.finished;
    return { id: made.id, model: f.starts.at(-1)?.o.model };
  };
  return { ...f, sub };
}

it("opens a Codex sub-thread that names no model on wsp's default where the account lists it, not on the account's first row", async () => {
  const { rt, sub } = await boatWith(async () => ({ models: [row("gpt-6-astra", 2), row("gpt-6-sol", 3), row("gpt-5.6-sol", 5)] }));
  expect((await sub()).model).toBe("gpt-5.6-sol");
  await rt.close();
}, 40_000);

it("reads the account's models once per TTL for every machine, one whose codex installs on its first run included", async () => {
  const asked: string[] = [];
  const { rt, boat, lead, sub } = await boatWith(async version => (asked.push(version), ACCOUNT));
  const inner = boat.execImpl;
  boat.execImpl = (m, cmd, stdin) => {
    if (cmd.startsWith("for b in ") && cmd.includes("codex")) return { exitCode: 0, stdout: '\x1e#!/bin/sh\n# ascii-harness-shim\nexec /usr/local/lib/ascii-harnesses/codex "$@"\n\x1e', stderr: "" };
    if (cmd.startsWith("if { case")) return { exitCode: 0, stdout: "1\n", stderr: "" };
    return inner(m, cmd, stdin);
  };
  const { id } = await sub("gpt-6-sol");
  for (let i = 0; i < 5; i++) await rt.harnesses.list(id, lead);
  await sub("gpt-6-sol");
  expect(asked).toEqual([INSTALLED]);
  await rt.close();
}, 40_000);

it("reads the account's models for the newest codex a machine there named, so a new sub-thread starts on one only that version is offered", async () => {
  const asked: string[] = [];
  const { rt, boat, sub } = await boatWith(async version => (asked.push(version), { models: [...(version === "0.163.0" ? [row("gpt-6.1-sol", 1)] : []), row("gpt-6-sol", 3)] }));
  const inner = boat.execImpl;
  const first = new Set<string>();
  boat.execImpl = (m, cmd, stdin) => {
    if (cmd.includes("__WSP_CATALOG_SEP__") && (first.size === 0 || first.has(m.id))) return first.add(m.id), { exitCode: 0, stdout: PROBE.replace("0.153.0", "0.163.0"), stderr: "" };
    return inner(m, cmd, stdin);
  };
  await sub("gpt-6-sol");
  expect(await sub("gpt-6.1-sol")).toMatchObject({ model: "gpt-6.1-sol" });
  expect(asked).toEqual(["0.163.0", "0.163.0"]);
  await rt.close();
}, 40_000);
