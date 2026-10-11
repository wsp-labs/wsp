// SPDX-License-Identifier: AGPL-3.0-only
// Removing a provider on a serving host: once its machines are gone the key
// leaves the .env beside the state file, and the provider leaves the host's
// places, the other lines of that file kept as they were.
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { FakeBackend } from "@wsp/engine";
import { exitClassOf } from "@wsp/protocol";
import type { Runtime } from "@wsp/runtime";
import { goldenRecipe, makeRuntime } from "../src/cli.js";
import { envFileFor, savedEnv, writeEnvFile } from "../src/env-keys.js";
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

function hostOn(saved: Record<string, string>, table = CLOUD): { rt: Runtime; envFile: string } {
  const home = mkdtempSync(join(tmpdir(), "wsp-provider-remove-"));
  homes.push(home);
  const statePath = join(home, "state.json");
  writeEnvFile(envFileFor(statePath), saved);
  const env = providerEnvWith({}, {}, [savedEnv(statePath)], table);
  const rt = makeRuntime({}, statePath, goldenRecipe({}), env, undefined, undefined, placeWiring(statePath), undefined, undefined, table);
  runtimes.push(rt);
  return { rt, envFile: envFileFor(statePath) };
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
});
