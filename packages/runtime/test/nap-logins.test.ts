// SPDX-License-Identifier: AGPL-3.0-only
// A fork at a provider keeps no sign-in in the vault its nap stores, filters one out of a vault an older host stored
// as it lands, and refuses a snapshot while one stands on the machine. A fork on a computer somebody joined keeps
// its home as it was.
import { LOGIN_PATHS } from "@wsp/catalog";
import type { MachineBackend } from "@wsp/engine";
import { DAEMON_TOKEN_PATH, verbFailure } from "@wsp/protocol";
import { describe, expect, it, vi } from "vitest";
import { createRuntime, type PlaceBackends } from "../src/runtime.js";
import { newPlaceKeyPair } from "../src/places.js";
import { memoryStore } from "../src/store.js";
import { wiring, report } from "./place-join.js";
import { code, ctx, forks, join, serving, sockets, type ForkingPlace } from "./places-fixture.js";
import { createOn, stubBackend, tokenGuest, type StubBackend } from "./stub-backend.js";

/** The sign-ins the cases name by hand, so a list that lost one fails here and not only in the loop over the list. */
const NAMED = ["/root/.codex/auth.json", "/root/.claude-cfg/.credentials.json", "/root/.config/gh/hosts.yml", "/root/.config/gcloud", "/etc/profile.d/wsp-secrets.sh", "/etc/fish/conf.d/wsp-secrets.fish"];
const excludes = (cmd: string | undefined): string[] => [...(cmd ?? "").matchAll(/--exclude='([^']+)'/g)].map(m => m[1]!);

/** A stub whose guest answers the vault road and, for the probe a snapshot runs, holds the paths in `held`. Every
 * command is kept by machine. */
function guest(held: string[] = []): { backend: StubBackend; cmds: { machine: string; cmd: string }[] } {
  const backend = stubBackend();
  const cmds: { machine: string; cmd: string }[] = [];
  backend.execImpl = (m, cmd) => {
    cmds.push({ machine: m.id, cmd });
    if (cmd.includes(DAEMON_TOKEN_PATH)) return tokenGuest(m, cmd);
    if (cmd.includes("ls -A /root")) return { exitCode: 0, stdout: "notes.md\n.codex\n.config\n", stderr: "" };
    if (cmd.startsWith("wc -c <")) return { exitCode: 0, stdout: "1000\n", stderr: "" };
    if (cmd.startsWith("for p in ")) return { exitCode: 0, stdout: held.filter(p => cmd.includes(`'${p}'`)).map(p => `${p}\n`).join(""), stderr: "" };
    return { exitCode: 0, stdout: "", stderr: "" };
  };
  vi.stubGlobal(
    "fetch",
    vi.fn(async (_url: string, init?: { method?: string }) => (init?.method === "PUT" ? new Response(null, { status: 200 }) : new Response(Buffer.from("tarbytes")))),
  );
  return { backend, cmds };
}

const twoProviders = (wired: string, at: Record<string, MachineBackend>): PlaceBackends => ({
  get wired() {
    return wired;
  },
  backend: place => at[place],
  list: () => Object.keys(at),
});

describe("the vault of a fork at a provider", () => {
  it("a nap archives the home with every login path and both secrets files left out", async () => {
    const { backend, cmds } = guest();
    const store = memoryStore();
    const rt = createRuntime({ backend, store, adapters: {} });
    try {
      const ws = await createOn(rt, { golden: "snap_g", name: "x" });
      await rt.workspaces.nap(ws.id);
      const tar = cmds.find(c => c.machine === "m1" && c.cmd.includes("tar czf"))?.cmd;
      expect(tar).toContain("'root/notes.md'");
      const left = excludes(tar);
      for (const p of [...NAMED, ...LOGIN_PATHS]) expect(left, p).toContain(p.slice(1));
      expect(await store.getBlob("vaults", ws.id)).toEqual(Buffer.from("tarbytes"));
    } finally {
      await rt.close();
      vi.unstubAllGlobals();
    }
  });

  it("reads whether the fork is at a provider when the nap runs, so a key saved after the host loaded the fork still leaves the logins out", async () => {
    const { backend, cmds } = guest();
    const store = memoryStore();
    // The host lists a provider only while its key is in the environment, and a key can be saved after it starts.
    let keyed = true;
    const places: PlaceBackends = {
      get wired() {
        return "solari";
      },
      backend: place => (place === "solari" ? backend : undefined),
      list: () => (keyed ? ["solari"] : []),
    };
    const first = createRuntime({ backend, store, adapters: {}, places });
    const ws = await createOn(first, { golden: "snap_g", name: "x" });
    await first.close();
    keyed = false;
    const second = createRuntime({ backend, store, adapters: {}, places });
    try {
      expect((await second.workspaces.get(ws.id)).phase).toBe("running");
      keyed = true;
      await second.workspaces.nap(ws.id);
      const tar = cmds.filter(c => c.cmd.includes("tar czf")).at(-1)?.cmd;
      const left = excludes(tar);
      for (const p of LOGIN_PATHS) expect(left, p).toContain(p.slice(1));
    } finally {
      await second.close();
      vi.unstubAllGlobals();
    }
  });

  it("an upgrade carries the home onto the replacement with the same paths left out, keeping the replacement's own", async () => {
    const { backend, cmds } = guest();
    const rt = createRuntime({ backend, store: memoryStore(), adapters: {} });
    try {
      const ws = await createOn(rt, { golden: "snap_g", name: "x" });
      expect((await rt.workspaces.upgrade(ws.id)).machineId).toBe("m2");
      const untar = cmds.find(c => c.machine === "m2" && c.cmd.includes("tar xzf"))?.cmd;
      for (const p of [...NAMED, ...LOGIN_PATHS]) expect(excludes(untar), p).toContain(p.slice(1));
      expect(untar).toContain("trap back EXIT");
    } finally {
      await rt.close();
      vi.unstubAllGlobals();
    }
  });

  it("a rebuild lands a vault an older host stored with the same paths left out", async () => {
    const { backend, cmds } = guest();
    const store = memoryStore();
    const rt = createRuntime({ backend, store, adapters: {} });
    try {
      const ws = await createOn(rt, { golden: "snap_g", name: "x" });
      await store.putBlob("vaults", ws.id, Buffer.from("a vault written before the rule, sign-ins in it"));
      expect((await rt.workspaces.rebuild(ws.id)).machineId).toBe("m2");
      const untar = cmds.find(c => c.machine === "m2" && c.cmd.includes("tar xzf"))?.cmd;
      expect(untar).toContain("-C '/' --recursive-unlink");
      expect(untar).toContain("trap back EXIT");
      const left = excludes(untar);
      for (const p of [...NAMED, ...LOGIN_PATHS]) expect(left, p).toContain(p.slice(1));
    } finally {
      await rt.close();
      vi.unstubAllGlobals();
    }
  });
});

describe("a fork on a computer somebody joined", () => {
  it("naps with its home archived as it is, sign-ins and all, since that computer is the person's own", async () => {
    const { hostKey } = await serving();
    let place!: ForkingPlace;
    const { client } = await join(hostKey, { code: await code(), name: "srv", report: report("srv"), answers: c => (place = forks(c)) });
    sockets.push(client.ws);
    const asked: string[] = [];
    place.onMachine = cmd => {
      asked.push(cmd);
      return { exitCode: 0, stdout: cmd.includes("ls -A /root") ? ".codex\nnotes.md\n" : "", stderr: "" };
    };
    // A computer joined today runs threads in its folder; a fork there is one an older join made.
    const door = ctx.runtime!.places!;
    const joined = door.joined;
    door.joined = () => false;
    const fork = await createOn(ctx.runtime!, { golden: "snap_g", name: "x", on: "srv" }).finally(() => (door.joined = joined));
    expect(fork.place).toBeDefined();
    await ctx.runtime!.workspaces.nap(fork.id);
    // The archive runs detached, its script handed over in base64.
    const scripts = asked.flatMap(c => [...c.matchAll(/printf %s '([A-Za-z0-9+/=]+)' \| base64 -d/g)].map(m => Buffer.from(m[1]!, "base64").toString()));
    const tar = scripts.find(c => c.includes("tar czf"));
    expect(tar).toContain("'root/.codex'");
    expect(excludes(tar)).toEqual([]);
  });
});
