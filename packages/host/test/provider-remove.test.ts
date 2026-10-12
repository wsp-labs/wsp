// SPDX-License-Identifier: AGPL-3.0-only
// Removing a provider on a serving host: once its machines are gone the key
// leaves the .env beside the state file, and the provider leaves the host's
// places, the other lines of that file kept as they were.
import { mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { FakeBackend } from "@wsp/engine";
import { exitClassOf, providerKeyStillLine } from "@wsp/protocol";
import type { Runtime } from "@wsp/runtime";
import { goldenRecipe, makeRuntime } from "../src/cli.js";
import { keyLayers } from "../src/cli/keys.js";
import { envFileFor, parseEnvFile, writeEnvFile } from "../src/env-keys.js";
import { placeWiring } from "../src/places.js";
import { BOX_KEY_ENV, PROVIDER_ENV, providerEnvWith, providerTable } from "../src/providers.js";

/** The cloud's table, its Boat row answering out of memory, so no case reaches Boat. */
const CLOUD = providerTable(true).map(m => (m.id === "box" ? { ...m, build: () => new FakeBackend() } : m));
/** The same table with Boat answering the key as refused, as its API answers a revoked key. */
const REFUSED = providerTable(true).map(m => (m.id === "box" ? { ...m, build: () => Object.assign(new FakeBackend(), { list: () => Promise.reject(Object.assign(new Error("unauthorized"), { kind: "auth", status: 401 })) }) } : m));

const homes: string[] = [];
const runtimes: Runtime[] = [];
afterEach(async () => {
  for (const rt of runtimes.splice(0)) await rt.close();
  for (const home of homes.splice(0)) rmSync(home, { recursive: true, force: true });
});

/** A host on a temp state started in a folder of its own, from a shell holding `shell`: `folder` is that folder's
 * .env, read before the one beside the state as a host started there reads it. */
function hostOn(saved: Record<string, string>, table = CLOUD, start: { folder?: Record<string, string>; shell?: Record<string, string> } = {}): { rt: Runtime; envFile: string; statePath: string; cwd: string } {
  const home = mkdtempSync(join(tmpdir(), "wsp-provider-remove-"));
  homes.push(home);
  const statePath = join(home, "state", "state.json");
  const cwd = join(home, "start");
  mkdirSync(cwd);
  writeEnvFile(envFileFor(statePath), saved);
  if (start.folder !== undefined) writeEnvFile(join(cwd, ".env"), start.folder);
  const reads = { env: start.shell ?? {}, cwd };
  const env = providerEnvWith({}, reads.env, keyLayers({ ...reads, statePath }), table);
  const rt = makeRuntime({}, statePath, goldenRecipe({}), env, undefined, undefined, placeWiring(statePath), undefined, undefined, table, reads);
  runtimes.push(rt);
  return { rt, envFile: envFileFor(statePath), statePath, cwd };
}

const providers = async (rt: Runtime): Promise<string[]> => (await rt.places!.list(0)).filter(p => p.kind === "provider").map(p => p.id);

describe("removing Boat", () => {
  it("takes BOAT_API_KEY out of the .env and Boat off the host's places, the other lines kept", async () => {
    const { rt, envFile } = hostOn({ ANTHROPIC_API_KEY: "kept", [BOX_KEY_ENV]: "ascii_live_fake" });
    expect(await providers(rt)).toEqual(["box"]);
    expect(await rt.places!.remove("box")).toMatchObject({ removed: true, deleted: 0 });
    expect(readFileSync(envFile, "utf8")).toBe("ANTHROPIC_API_KEY=kept\n");
    expect(await providers(rt)).toEqual([]);
  });

  it("drops the word wiring this host to Boat with the key, so a fork after it asks for a provider rather than Boat", async () => {
    const { rt, envFile } = hostOn({ [PROVIDER_ENV]: "box", [BOX_KEY_ENV]: "ascii_live_fake" });
    expect(await providers(rt)).toEqual(["box"]);
    expect(await rt.places!.remove("box")).toMatchObject({ removed: true });
    expect(readFileSync(envFile, "utf8")).toBe("\n");
    expect(await providers(rt)).toEqual([]);
  });

  it("keeps the key Boat refuses on a remove, and drops it on a forget without asking Boat", async () => {
    const { rt, envFile } = hostOn({ ANTHROPIC_API_KEY: "kept", [BOX_KEY_ENV]: "ascii_live_fake" }, REFUSED);
    const refused = await rt.places!.remove("box").catch((e: unknown) => e);
    expect(exitClassOf(refused)).toBe("auth");
    expect((refused as Error).message).toMatch(/^Boat refused the key \(unauthorized\)/);
    expect(readFileSync(envFile, "utf8")).toContain(`${BOX_KEY_ENV}=`);
    expect(await providers(rt)).toEqual(["box"]);

    expect(await rt.places!.remove("box", { forget: true })).toMatchObject({ removed: true });
    expect(readFileSync(envFile, "utf8")).toBe("ANTHROPIC_API_KEY=kept\n");
    expect(await providers(rt)).toEqual([]);
  });
  it("says where the key still is when the start folder's .env holds it too, and leaves that file alone", async () => {
    const { rt, envFile, statePath, cwd } = hostOn({ ANTHROPIC_API_KEY: "kept", [BOX_KEY_ENV]: "ascii_live_fake" }, CLOUD, { folder: { [BOX_KEY_ENV]: "ascii_live_fake" } });
    expect(await providers(rt)).toEqual(["box"]);
    const still = providerKeyStillLine([{ file: join(cwd, ".env") }]);
    expect(await rt.places!.remove("box")).toMatchObject({ removed: true, deleted: 0, note: still });
    expect(still).toBe(`Its key is still in ${join(cwd, ".env")}, which a host started in that folder reads first; take it out there.`);
    expect(readFileSync(envFile, "utf8")).toBe("ANTHROPIC_API_KEY=kept\n");
    expect(Object.keys(parseEnvFile(join(cwd, ".env")))).toEqual([BOX_KEY_ENV]);
    // The running host lets Boat go; the next one started in that folder would read it back, which is why the answer says so.
    expect(await providers(rt)).toEqual([]);
    expect(providerEnvWith({}, {}, keyLayers({ env: {}, cwd, statePath }), CLOUD)[BOX_KEY_ENV]).toBeDefined();
  });

  it("says the same on a forget, and names the shell's environment when the key is there", async () => {
    const { rt } = hostOn({ [BOX_KEY_ENV]: "ascii_live_fake" }, REFUSED, { shell: { [BOX_KEY_ENV]: "ascii_live_fake" } });
    expect(await rt.places!.remove("box", { forget: true })).toMatchObject({ removed: true, note: providerKeyStillLine([{ env: BOX_KEY_ENV }]) });
    expect(await providers(rt)).toEqual([]);
  });

  it("answers no note when the file beside the state was the only place holding the key", async () => {
    const { rt } = hostOn({ [BOX_KEY_ENV]: "ascii_live_fake" });
    expect(await rt.places!.remove("box")).not.toHaveProperty("note");
  });
  it("names both places when the shell and the start folder's .env each hold the key", async () => {
    const { rt, cwd } = hostOn({ [BOX_KEY_ENV]: "ascii_live_fake" }, CLOUD, { shell: { [BOX_KEY_ENV]: "ascii_live_fake" }, folder: { [BOX_KEY_ENV]: "ascii_live_fake" } });
    const answer = await rt.places!.remove("box");
    expect(answer).toMatchObject({ removed: true, note: providerKeyStillLine([{ env: BOX_KEY_ENV }, { file: join(cwd, ".env") }]) });
    expect(answer.note).toBe(`Its key is still in the environment this host started with (BOAT_API_KEY) and in ${join(cwd, ".env")}, which a host started from that shell or in that folder reads first; take it out of both.`);
  });
});
