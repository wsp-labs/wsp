// SPDX-License-Identifier: AGPL-3.0-only
// The lines that set what machines at a provider use from this host: a sign-in asked on such a machine is refused
// before anything runs there, and a project's trackers are kept on its record.
import { EXIT_CODES, hereName, ThreadView } from "@wsp/protocol";
import { threadIdOf } from "../src/verbs/client.js";
import { describe, expect, it } from "vitest";
import { runsFromItsOwnFolder } from "./own-folder.js";
import { verbsHost } from "./verbs-host.js";

runsFromItsOwnFolder();

describe("sign-ins and trackers for machines at a provider", () => {
  const h = verbsHost();

  it("wsp agents signin on a thread at a provider is refused naming the computer to sign in on", async () => {
    const started = await h.run("run", h.cloud.name, "--detach", "hello");
    expect(started.code, started.io.errors.join("\n")).toBe(0);
    const [thread] = (h.json((await h.run("threads", "--json")).io).at(-1) as { threads: unknown[] }).threads.map(t => ThreadView.parse(t));
    const refused = await h.run("agents", "signin", "claude", threadIdOf(thread!));
    const here = hereName(await h.rt.places!.list(Date.now()));
    expect(here).not.toBe("");
    expect(refused.code).toBe(EXIT_CODES.usage);
    expect(refused.io.errors).toEqual([`wsp agents signin: Claude Code does not sign in on a cloud machine. Sign in on ${here} instead: cloud threads use it through wsp.`]);
  });

  it("wsp projects set --tracker keeps the whole list in the project's defaults, --reset trackers takes it off, and a word that names no repo is refused", async () => {
    const set = await h.run("projects", "set", h.cloud.name, "--tracker", "acme/lab", "--tracker", "acme/map", "--json");
    expect(set.code, set.io.errors.join("\n")).toBe(0);
    expect(h.json(set.io).at(-1)).toMatchObject({ project: { id: h.cloud.id }, trackers: ["acme/lab", "acme/map"] });
    expect((await h.rt.preferences.get()).projectDefaults[h.cloud.id]?.trackers).toEqual(["acme/lab", "acme/map"]);
    const said = await h.run("projects", "set", h.cloud.name, "--tracker", "acme/lab");
    expect(said.io.lines.join("\n").split("\n").at(-1)).toBe("gh on a sub-thread's machine may also comment on: acme/lab");
    const reset = await h.run("projects", "set", h.cloud.name, "--reset", "trackers", "--json");
    expect(reset.code).toBe(0);
    expect((await h.rt.preferences.get()).projectDefaults[h.cloud.id]?.trackers).toBeUndefined();
    const bad = await h.run("projects", "set", h.cloud.name, "--tracker", "lab");
    expect(bad.code).toBe(EXIT_CODES.usage);
    expect(bad.io.errors).toEqual(['wsp projects set: --tracker "lab" names no repo. Name it as owner/name, the way gh repo view does.']);
  });
});
