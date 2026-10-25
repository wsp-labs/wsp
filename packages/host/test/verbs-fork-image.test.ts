// SPDX-License-Identifier: AGPL-3.0-only
// The wsp verbs against a host over the fake runtime: each one a client of
// the protocol on localhost, authenticated with the token the host wrote,
// reading the same session index the sidebar reads.
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { createServer as createHttpServer } from "node:http";
import { type AddressInfo } from "node:net";
import { join } from "node:path";
import { NoProviderBackend } from "@wsp/engine";
import { childStartedLine, HERE_PLACE_ID, goneRoadRefusal, notAnsweringYet, EXIT_CODES, IMAGE_NO_VAULT, placeBuildsNoImageLine } from "@wsp/protocol";
import { memoryStore } from "@wsp/runtime";
import { describe, expect, it, vi } from "vitest";
import { HELP, agentPage, cli, commandPage, COMMANDS_FOR_HELP } from "../src/cli.js";
import { CLI_VERBS, runVerb, dialHost } from "../src/verbs.js";
import { HOST_SIDE_VAULT, THREAD_PREFIX_WORD } from "../src/verbs.js";
import { hostSideOnlyFix, hostSideOnlyLine } from "../src/hosts.js";
import type { WatchSignals } from "../src/watch.js";
import { writeHost } from "../src/hosts.js";
import { stubBackend, withDaemonRoads } from "./stub-backend.js";
import { captured, threadOn, type Captured } from "./verbs-fixture.js";
import { runsFromItsOwnFolder } from "./own-folder.js";
import { CLOUD_ON } from "../src/cloud.js";
import { HOST_KEY, verbsHost } from "./verbs-host.js";

runsFromItsOwnFolder();

// A path the process may not read is refused here and not by chmod: these tests run as root, which reads anything.
vi.mock("node:fs", async importOriginal => (await import("../../runtime/test/fs-refusal.js")).refusingFs(await importOriginal<typeof import("node:fs")>()));

describe("wsp verbs over the host: fork, new and the image", () => {
  const h = verbsHost();

  it.runIf(CLOUD_ON)("fork takes --size as <cpu>x<memGb>, which reaches the create's size; a size the provider does not offer, or no size at all, is refused in one line naming the list, and nothing is minted", async () => {
    const big = await h.run("new", "big", "--size", "2x8");
    expect(big.code).toBe(0);
    expect(h.backend.machines.at(-1)!.spec).toMatchObject({ cpu: 2, memMb: 8192 });
    const [status] = await h.rt.status.list();
    expect(status).toMatchObject({ name: "big", size: { cpu: 2, memMb: 8192 } });

    withDaemonRoads(h.backend);
    const forked = await h.run("fork", "big", "--name", "wide", "--size", "4x8");
    expect(forked.code).toBe(0);
    expect(h.backend.machines.at(-1)!.spec).toMatchObject({ cpu: 4, memMb: 8192 });
    expect((await h.rt.status.list()).find(w => w.name === "wide")!.size).toEqual({ cpu: 4, memMb: 8192 });

    const list = "the sizes are 2x4 ($0.11/hr), 2x8 ($0.15/hr), 4x8 ($0.22/hr)";
    const odd = await h.run("fork", "big", "--name", "odd", "--size", "8x16");
    expect(odd.code).toBe(3);
    expect(odd.io.errors).toEqual([`wsp fork: 8x16 is not a size this provider offers; ${list}. Name one of those with --size.`]);
    const word = await h.run("fork", "big", "--size", "large");
    expect(word.code).toBe(3);
    expect(word.io.errors).toEqual([`wsp fork: large is not a size this provider offers; ${list}. Name one of those with --size.`]);
    expect(h.backend.machines).toHaveLength(2);
    expect((await h.rt.workspaces.list()).map(w => w.name).sort()).toEqual(["big", "wide"]);

    // Without --size the golden's own size stands.
    await h.run("new", "plain");
    expect(h.backend.machines.at(-1)!.spec).toMatchObject({ cpu: 2, memMb: 4096 });
  });

  it.runIf(CLOUD_ON)("a fork the provider refuses at the machine cap is one line naming the workspaces holding the slots, never the provider's sentence", async () => {
    await h.run("new", "first");
    await h.run("new", "t-cap");
    const create = h.backend.create.bind(h.backend);
    h.backend.create = async spec => {
      if (spec.fromSnapshot !== undefined) throw Object.assign(new Error("Too many concurrent sessions"), { kind: "concurrency", status: 429 });
      return create(spec);
    };
    withDaemonRoads(h.backend);
    const refused = await h.run("fork", "first", "--name", "f2");
    expect(refused.code).toBe(1);
    expect(refused.io.errors).toEqual(["wsp fork: both machine slots are in use: first, t-cap. Pause one or wait for a nap."]);
    expect((await h.rt.workspaces.list()).map(w => w.name).sort()).toEqual(["first", "t-cap"]);
  });

  it("threads --watch redraws the same table where it stands until Ctrl-C, on one socket", async () => {
    await h.run("new", "alpha");
    for (const word of ["threads"] as const) {
      const frames: string[] = [];
      const io: Captured = { ...captured(), redraw: { write: text => void frames.push(text), columns: () => 100 } };
      // Ctrl-C, without a real signal: one would take the test runner with it.
      const held = new Set<() => void>();
      const signals: WatchSignals = {
        on: (_s, l) => {
          held.add(l);
          return undefined;
        },
        off: (_s, l) => held.delete(l),
      };
      // Every socket this line opens to the host, counted: a watch that dialled per frame would be a number here.
      let dials = 0;
      const dial: typeof dialHost = (path, opts) => {
        dials++;
        return dialHost(path, opts);
      };
      const verb = CLI_VERBS.find(v => v.name === word)!;
      const watching = runVerb(verb, [word, "--watch", "--state", h.statePath], io, () => h.statePath, { cwd: h.dir, env: h.env, signals, dial });
      // Past the one second tick, so what is waited for is a second frame and not the first one twice over.
      await vi.waitFor(() => expect(frames.filter(f => f.includes("\n")).length).toBeGreaterThan(1), { timeout: 5_000 });
      for (const stop of [...held]) stop();
      expect(await watching, word).toBe(0);
      // The cursor comes off the screen for the frames and is back on the last write.
      expect(frames[0], word).toBe("\x1b[?25l");
      expect(frames.at(-1), word).toBe("\x1b[?25h");
      const drawn = frames.slice(1, -1);
      expect(drawn.length, word).toBeGreaterThan(1);
      expect(drawn[0], word).toContain("THREAD");
      // The first frame has nothing above it; every frame after it rewinds the rows it drew, so the table never
      // walks down the screen.
      expect(drawn[0]!.startsWith("\x1b["), word).toBe(false);
      for (const frame of drawn.slice(1)) {
        expect(frame, word).toMatch(/^\x1b\[\d+A\x1b\[G\x1b\[J/);
        expect(frame, word).toContain("THREAD");
      }
      // One socket for every frame of it, which is the whole of why the flag is worth having.
      expect(dials, word).toBe(1);
      // Nothing went to stdout as lines: a watched list is frames, and only frames.
      expect(io.lines, word).toEqual([]);
    }
  });

  it("--watch is refused off a terminal and beside --json, each in two halves, and nothing is drawn", async () => {
    await h.run("new", "alpha");
    const noTerminal = await h.run("threads", "--watch");
    expect(noTerminal.code).toBe(EXIT_CODES.usage);
    expect(noTerminal.io.errors).toEqual(["wsp threads --watch redraws where it stands, and this run has no terminal to redraw on. Run wsp threads without --watch to print the list once."]);
    expect(noTerminal.io.lines).toEqual([]);

    const io: Captured = { ...captured(), redraw: { write: () => {}, columns: () => 100 } };
    const asJson = await cli(["threads", "--watch", "--json", "--state", h.statePath], io, undefined, h.env);
    expect(asJson).toBe(EXIT_CODES.usage);
    expect(io.errors.map(l => (JSON.parse(l) as { error: string }).error)).toEqual(["wsp threads --watch redraws a table and --json answers with objects. Take one of the two: wsp threads --watch at a terminal, or wsp threads --json for the objects."]);
  });

  it.runIf(CLOUD_ON)("fork makes a sibling from the source's own golden version, by name or id, and --send opens its first thread", async () => {
    await h.run("new", "alpha");
    const [alpha] = await h.rt.workspaces.list();
    withDaemonRoads(h.backend);
    const plain = await h.run("fork", "alpha");
    expect(plain.code).toBe(0);
    const forks = (await h.rt.workspaces.list()).filter(w => w.id !== alpha!.id);
    expect(forks.map(w => [w.name, w.golden])).toEqual([["alpha-fork", alpha!.golden]]);
    // A fork is a child of the workspace it was forked from: the record says so, and a bring back from it reads
    // that parent's own branch as the base its work lands in.
    expect(forks[0]!.parentWorkspaceId).toBe(alpha!.id);
    // The fork's copy is put on the branch its source is on, and the line says so.
    expect(plain.io.lines).toEqual([`created alpha-fork ${forks[0]!.id}, a copy of ${forks[0]!.project.name} at ${forks[0]!.project.path}\n${childStartedLine("alpha-fork", "work")}`]);

    const sent = await h.run("fork", alpha!.id, "--name", "worker", "--send", "build it");
    expect(sent.code).toBe(0);
    const worker = (await h.rt.workspaces.list()).find(w => w.name === "worker")!;
    const [thread] = await h.rt.sessions.list(worker.id);
    expect(thread).toMatchObject({ harness: "claude", startedBy: "cli", prompt: "build it", status: "completed" });
    expect(sent.io.lines).toEqual([`created worker ${worker.id}, a copy of ${worker.project.name} at ${worker.project.path}\n${childStartedLine("worker", "work")}`, `thread ${thread!.threadId}  ${THREAD_PREFIX_WORD}`, "re: build it"]);
    expect(sent.io.streamed.endsWith("ready\nre: \n$ ls\nbuild it\ncompleted\n")).toBe(true);
  });

  it.runIf(CLOUD_ON)("run --title names the thread from the first second, in the agent's own launch and in the table", async () => {
    await h.run("new", "alpha");
    const opened = await h.run("run", "alpha", "--title", "Ticket 411 review", "build it");
    expect(opened.code).toBe(0);
    expect(h.claude.starts.at(-1)?.title).toBe("Ticket 411 review");
    const [row] = await h.rt.sessions.list();
    expect(row).toMatchObject({ harnessTitle: "Ticket 411 review", titleSource: "person" });
    const listed = await h.run("threads");
    expect(listed.io.lines.join("\n")).toContain("Ticket 411 review");
  });

  it.runIf(CLOUD_ON)("fork, run, exec and wake refuse a workspace whose machine is gone, quoting the provider, with no waking line", async () => {
    await h.run("new", "alpha");
    const [alpha] = await h.rt.workspaces.list();
    const thread = await threadOn(h.rt, "alpha");
    await h.handle!.close();
    h.handle = undefined;
    h.backend.machines[0]!.killed = true; // deleted at the provider while no host ran
    await h.restartHost({ claude: h.claude.adapter });
    const words = (await h.rt.workspaces.get(alpha!.id)).gone!;
    expect(words).toMatch(new RegExp(`^machine ${alpha!.machineId} is gone at the provider: the record load found it gone at \\S+Z \\(404 gone\\)$`));
    withDaemonRoads(h.backend);
    const forked = await h.run("fork", "alpha");
    expect(forked.code).toBe(1);
    expect(forked.io.errors).toEqual([`wsp fork: alpha's machine is gone with its disk, so work that was not pushed is lost; rebuild it to fork, which brings back its home folder from the last saved nap (${words})`]);
    const opened = await h.run("run", "alpha", "do it");
    expect(opened.code).toBe(1);
    expect(opened.io.errors).toEqual([`wsp run: alpha's machine is gone with its disk, so work that was not pushed is lost; rebuild it to send, which brings back its home folder from the last saved nap (${words})`]);
    const ran = await h.run("exec", thread, "--", "echo", "hi");
    expect(ran.code).toBe(1);
    expect(ran.io.errors).toEqual([`wsp exec: alpha's machine is gone with its disk, so work that was not pushed is lost; rebuild it to exec, which brings back its home folder from the last saved nap (${words})`]);
    // The workspace is on the listing throughout: what the machine is, is the machine's trouble to say, and no verb
    // answers for a machine by calling the workspace missing.
    expect((await h.rt.workspaces.list()).map(w => w.name)).toEqual(["alpha"]);
    const woken = await h.run("wake", "alpha");
    expect(woken.code).toBe(1);
    expect(woken.io.errors).toEqual([`wsp wake: alpha's machine is gone with its disk, so work that was not pushed is lost; rebuild it to wake, which brings back its home folder from the last saved nap (${words})`]);
    expect((await h.rt.workspaces.list()).map(w => [w.name, w.phase])).toEqual([["alpha", "gone"]]);
  });

  it.runIf(CLOUD_ON)("rebuild is the road out of gone: a new machine under the same workspace, its id and state printed; a machine that answers is refused in the row's own words", async () => {
    await h.run("new", "alpha");
    const [alpha] = await h.rt.workspaces.list();
    const refused = await h.run("rebuild", "alpha");
    expect(refused.code).toBe(1);
    expect(refused.io.errors).toEqual([`wsp rebuild: ${goneRoadRefusal("running", "rebuild")}`]);
    expect(h.backend.machines).toHaveLength(1);

    await h.handle!.close();
    h.handle = undefined;
    h.backend.machines[0]!.killed = true; // deleted at the provider while no host ran
    await h.restartHost({ claude: h.claude.adapter });
    expect((await h.rt.workspaces.get(alpha!.id)).phase).toBe("gone");

    const built = await h.run("rebuild", "alpha");
    expect(built.code).toBe(0);
    expect(built.io.errors).toEqual([]);
    const after = await h.rt.workspaces.get(alpha!.id);
    expect(after).toMatchObject({ id: alpha!.id, name: "alpha", phase: "running", golden: alpha!.golden });
    expect(after.machineId).not.toBe(alpha!.machineId);
    expect(built.io.lines).toEqual([`alpha running on ${after.machineId}`]);
    // The verb every other one sends a gone workspace to now answers on it.
    const woken = await h.run("wake", "alpha");
    expect(woken.code).toBe(0);
    expect(woken.io.lines).toEqual(["alpha running"]);

    const asJson = await h.run("rebuild", "alpha", "--json");
    expect(asJson.code).toBe(1);
    expect(asJson.io.lines).toEqual([]);
    expect(asJson.io.errors.map(l => JSON.parse(l) as unknown)).toEqual([{ error: goneRoadRefusal("running", "rebuild"), class: "provider", exit: EXIT_CODES.provider }]);
    const missing = await h.run("rebuild", "nope");
    expect(missing.code).toBe(EXIT_CODES.usage);
    expect(missing.io.errors).toEqual(["wsp rebuild: no workspace nope"]);
    const extra = await h.run("rebuild", "alpha", "beta");
    expect(extra.code).toBe(EXIT_CODES.usage);
    expect(extra.io.errors).toEqual(["wsp rebuild takes one workspace. usage: wsp rebuild <workspace>"]);
  });

  it.runIf(CLOUD_ON)("rebuild refuses a workspace whose machine stopped answering in the words the row shows, not in the words for one that answers", async () => {
    await h.run("new", "dark");
    // Every cloud machine has an edge route, and a prompt 502 on it is the edge dialling the guest and finding
    // nothing on the daemon's port: the machine runs, the record reads running, and nothing answers on it.
    const edge = createHttpServer((_req, res) => {
      res.writeHead(502).end();
    });
    await new Promise<void>(r => edge.listen(0, "127.0.0.1", r));
    try {
      const port = (edge.address() as AddressInfo).port;
      h.backend.machines[0]!.previewUrl = async () => ({ url: `http://127.0.0.1:${port}/`, token: "stub", expiresAt: Date.now() + 3_600_000 });
      const refused = await h.run("rebuild", "dark");
      expect(refused.code).toBe(1);
      expect(refused.io.errors).toEqual([`wsp rebuild: ${notAnsweringYet("rebuild")}`]);
      // The machine the provider still holds is not replaced by a refusal.
      expect(h.backend.machines).toHaveLength(1);
    } finally {
      await new Promise<void>(r => edge.close(() => r()));
    }
  });

  it("wsp image reads the record off the seeded golden's head, says no sign-ins are held, and names the copy at this host's place", async () => {
    const listed = await h.run("image");
    expect(listed.code).toBe(0);
    const lines = listed.io.lines.join("\n").split("\n");
    expect(lines[0]).toContain("default v1");
    expect(lines[0]).toContain(IMAGE_NO_VAULT);
    expect(lines[0]).not.toContain("sign-in held");
    expect(lines[1]).toMatch(/^default  v1/);
    const [view] = h.json((await h.run("image", "--json")).io) as [{ image: { version: number; vault?: unknown }; copies: { place: string }[] }];
    expect(view.image.version).toBe(1);
    expect(view.image.vault).toBeUndefined();
    expect(view.copies.map(c => c.place)).toEqual(["default"]);
  });

  it.runIf(CLOUD_ON)("wsp image build refuses in one line for a place this host has not got and a place that takes no copy; the place it forks on is a place like any other", async () => {
    // Three places over one host: the one it forks on, one more that could build, and this computer, which forks
    // nothing and copies no disk.
    const elsewhere = stubBackend();
    const here = new NoProviderBackend();
    await h.restartHost(
      {},
      h.store,
      { wired: "default", backend: p => (p === "default" ? h.backend : p === "elsewhere" ? elsewhere : p === "here" ? here : undefined), list: () => ["default", "elsewhere", "here"] },
    );

    const nowhere = await h.run("image", "build", "nowhere");
    expect(nowhere.code).toBe(1);
    expect(nowhere.io.errors.join("")).toContain("no place named nowhere");

    // The provider this host forks on takes a copy build too; this record was backfilled off its own golden and
    // carries no recipe, so the build stops at the record and boots nothing.
    const wired = await h.run("image", "build", "default");
    expect(wired.code).toBe(1);
    expect(wired.io.errors.join("")).toContain("was sealed before the image record kept the recipe");

    const cannot = await h.run("image", "build", "here");
    expect(cannot.code).toBe(1);
    expect(cannot.io.errors.join("")).toContain(placeBuildsNoImageLine((await h.rt.places!.rows()).find(p => p.id === HERE_PLACE_ID)!.name));

    // Nothing was forked at any of them, and this computer was never read for a recipe.
    expect(elsewhere.machines).toEqual([]);

    const usage = await h.run("image", "build");
    expect(usage.code).toBe(EXIT_CODES.usage);
    expect(usage.io.errors.join("")).toContain("wsp image build takes one place.");
  });

  it.runIf(CLOUD_ON)("wsp image build on a host that has sealed nothing says so, whatever place is named", async () => {
    const elsewhere = stubBackend();
    await h.restartHost({}, memoryStore(), {
      wired: "default",
      backend: p => (p === "default" ? h.backend : p === "elsewhere" ? elsewhere : undefined),
      list: () => ["default", "elsewhere"],
    });
    const none = await h.run("image", "build", "elsewhere");
    expect(none.code).toBe(1);
    expect(none.io.errors.join("")).toContain("this host owns no image named default yet");
    expect(elsewhere.machines).toEqual([]);
  });

  it.runIf(CLOUD_ON)("fork's help says it makes a new machine from the source's image version on the source's branch, on the agent page and in wsp fork --help", async () => {
    const line = "a new machine from the source's image version, on the source's branch with its commits pushed first, not a copy of its live disk";
    expect(agentPage().replace(/\s+/g, " ")).toContain(line);
    const { code, io } = await h.run("fork", "--help");
    expect(code).toBe(0);
    expect(io.lines.join("\n").replace(/\s+/g, " ")).toContain(line);
  });

  it("every line of wsp --help fits 100 columns", () => {
    const wide = HELP.split("\n").filter(l => l.length > 100);
    expect(wide).toEqual([]);
  });
});
