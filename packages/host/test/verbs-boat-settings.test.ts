// SPDX-License-Identifier: AGPL-3.0-only
// Boat's own settings on wsp computers set, and its plan and starts on wsp computers, over a host whose Boat row is a
// BoxBackend answering /limits with the owner's recorded answer. Every case runs with the cloud off as well as on,
// since Boat's settings are in every build.
import { EXIT_CODES, HOST_KEY_ENV, HOST_TOKEN_ENV, HOST_URL_ENV, HERE_PLACE_ID, fmtSize, refusalLine, SIZE_PICK_FIX, sizeRefusal, threadOpRefusal, type PlaceView } from "@wsp/protocol";
import { BoxBackend, type RetryClock } from "@wsp/engine";
import { memoryStore } from "@wsp/runtime";
import { describe, expect, it, vi } from "vitest";
import { hostKeyHere } from "../src/places.js";
import { captured, type Captured } from "./verbs-fixture.js";
import { cli } from "../src/cli.js";
import { runsFromItsOwnFolder } from "./own-folder.js";
import { verbsHost } from "./verbs-host.js";

runsFromItsOwnFolder();

/** GET /limits on the owner's account, 2026-10-11 17:06 UTC, trimmed to the fields a row reads (spec 01). */
const OWNER_LIMITS = {
  accessTier: "standard",
  maxActiveBoxes: 100,
  activeBoxes: 11,
  starts: { unlimited: false, minute: { limit: 12, used: 0, remaining: 12 }, hour: { limit: 60, used: 9, remaining: 51 }, day: { limit: 200, used: 187, remaining: 13 } },
  canStart: true,
  boxPlanKey: "box_20",
};

describe("Boat's settings and its limits on the command line", () => {
  const h = verbsHost();

  /** The host again with Boat beside this computer, its backend a BoxBackend whose every call is counted. */
  async function withBoat(answers: () => Promise<Response> = async () => new Response(JSON.stringify(OWNER_LIMITS), { status: 200 }), clock?: RetryClock): Promise<string[]> {
    const asked: string[] = [];
    const fetch = (async (url: string | URL | Request, init?: RequestInit) => {
      asked.push(`${init?.method ?? "GET"} ${String(url)}`);
      return answers();
    }) as typeof globalThis.fetch;
    const boat = new BoxBackend({ apiKey: "box_x", fetch, ...(clock !== undefined ? { clock } : {}) });
    await h.restartHost({}, memoryStore(), { wired: "default", backend: p => (p === "default" ? h.backend : p === "box" ? boat : undefined), list: () => ["box"] });
    return asked;
  }
  const boatRow = async (): Promise<PlaceView> => (await h.rt.places!.rows()).find(p => p.id === "box")!;
  const answered = (io: Captured): { computer: PlaceView } => h.json(io).at(-1) as { computer: PlaceView };

  it("sets the size and the delete delay of a sub-thread's machine on Boat by its own name, and a reset takes each back", async () => {
    await withBoat();
    const set = await h.run("computers", "set", "boat", "--size", "4x8", "--delete-after", "60");
    expect(set.io.errors).toEqual([]);
    expect(set.code).toBe(0);
    expect(set.io.lines[0]).toContain(`size ${fmtSize({ cpu: 4, memMb: 8192 })} (2x4 by default)`);
    expect(set.io.lines[0]).toContain("deleted after 60 min (30 min by default)");
    expect(await boatRow()).toMatchObject({ settings: { size: { cpu: 4, memMb: 8192 }, deleteAfterMs: 3_600_000 }, size: { cpu: 4, memMb: 8192 }, deleteAfterMs: 3_600_000 });

    const size = await h.run("computers", "set", "Boat", "--reset", "size", "--json");
    expect(answered(size.io).computer).toMatchObject({ settings: { deleteAfterMs: 3_600_000 }, size: { cpu: 2, memMb: 4096 }, deleteAfterMs: 3_600_000 });
    expect(answered(size.io).computer.settings?.size).toBeUndefined();

    const delay = await h.run("computers", "set", "box", "--reset", "delete-after");
    expect(delay.io.lines[0]).toContain(`size ${fmtSize({ cpu: 2, memMb: 4096 })} (the default), deleted after 30 min (the default)`);
    expect(await boatRow()).toMatchObject({ size: { cpu: 2, memMb: 4096 }, deleteAfterMs: 1_800_000 });
    expect((await boatRow()).settings).toBeUndefined();

    const once = await h.run("computers", "set", "boat", "--delete-after", "0");
    expect(once.io.lines[0]).toContain("deleted once done (30 min by default)");
  });

  it("refuses a size Boat's row does not offer, a word that names no size, a delay outside a day, and both on this computer, as usage", async () => {
    await withBoat();
    const sizes = (await boatRow()).sizes!;
    expect(sizes.map(s => `${s.cpu}x${s.memMb / 1024}`)).toEqual(["2x4", "4x8", "8x16"]);
    const unsold = await h.run("computers", "set", "boat", "--size", "16x32");
    expect(unsold.code).toBe(EXIT_CODES.usage);
    expect(unsold.io.errors).toEqual([`wsp computers set: ${refusalLine(sizeRefusal("16x32", sizes), SIZE_PICK_FIX)}`]);
    for (const word of ["big", "4x8x2", "0x4"]) {
      const wrong = await h.run("computers", "set", "boat", "--size", word);
      expect(wrong.code).toBe(EXIT_CODES.usage);
      expect(wrong.io.errors).toEqual([`wsp computers set: ${refusalLine(sizeRefusal(word, sizes), "Name one of those with --size.")}`]);
    }
    for (const word of ["1441", "1.5", "soon"]) {
      const wrong = await h.run("computers", "set", "boat", "--delete-after", word);
      expect(wrong.code).toBe(EXIT_CODES.usage);
      expect(wrong.io.errors).toEqual([`wsp computers set: --delete-after for boat takes whole minutes from 0 to 1440, and got ${JSON.stringify(word)}. Write it as --delete-after 30, or 0 to delete once it is done.`]);
    }
    const here = await h.run("computers", "set", HERE_PLACE_ID, "--size", "4x8", "--delete-after", "10");
    expect(here.code).toBe(EXIT_CODES.usage);
    expect(here.io.errors[0]).toContain("not size and delete after");
    // This computer takes no size, so a word that names none gets the same refusal as one that does.
    const kindOnly = (await h.run("computers", "set", HERE_PLACE_ID, "--size", "4x8")).io.errors;
    expect(kindOnly[0]).toMatch(/takes .*, not size$/);
    for (const word of ["big", "0x4"]) expect((await h.run("computers", "set", HERE_PLACE_ID, "--size", word)).io.errors, word).toEqual(kindOnly);
    expect((await boatRow()).settings).toBeUndefined();
  });

  it("sets machines at once and spend per day on Boat, with the cloud on or off", async () => {
    await withBoat();
    const set = await h.run("computers", "set", "boat", "--machines", "5", "--spend", "20", "--json");
    expect(set.io.errors).toEqual([]);
    expect(answered(set.io).computer).toMatchObject({ cap: { machines: 5, spendPerDayUsd: 20 }, settings: { machines: 5, spendPerDayUsd: 20 } });
    const back = await h.run("computers", "set", "boat", "--reset", "machines", "--reset", "spend", "--json");
    expect(answered(back.io).computer).toMatchObject({ cap: { machines: 3, spendPerDayUsd: 10 } });
  });

  it("lists Boat with its plan, its starts, the size and the delete delay, reading /limits once in ten seconds", async () => {
    const asked = await withBoat();
    expect((await h.run("computers", "--json")).io.errors).toEqual([]);
    // The first list starts the read and answers at once; the row carries the plan once the read has landed.
    await vi.waitFor(async () => expect((await h.rt.places!.list(Date.now())).find(p => p.id === "box")?.plan).toBeDefined());
    const listed = await h.run("computers", "--json");
    const boat = (h.json(listed.io).at(-1) as { computers: PlaceView[] }).computers.find(p => p.id === "box");
    expect(boat).toMatchObject({
      plan: { key: "box_20", trial: false, machinesAtOnce: 100, sizes: ["2x4", "4x8", "8x16"] },
      starts: { minute: { used: 0, limit: 12 }, hour: { used: 9, limit: 60 }, day: { used: 187, limit: 200 } },
      size: { cpu: 2, memMb: 4096 },
      sizeDefault: { cpu: 2, memMb: 4096 },
      deleteAfterMs: 1_800_000,
    });
    const table = await h.run("computers");
    const [head, ...rows] = table.io.lines[0]!.split("\n");
    expect(head!.split(/ {2,}/)).toEqual(expect.arrayContaining(["STARTS", "WORKSPACES"]));
    expect(rows.find(r => r.startsWith("Boat"))).toContain("187/200 today");
    expect(asked).toEqual(["GET https://ascii.dev/api/box/v1/limits"]);
  });

  it("starts the host and lists and sets its computers while Boat's /limits has not answered, and the row reads it once it has", async () => {
    // A read that settles only when the test lets it, deaf to its own cap: a road that waited on it would never end.
    let answer: () => void = () => {};
    const held = new Promise<void>(resolve => (answer = resolve));
    const asked = await withBoat(() => held.then(() => new Response(JSON.stringify(OWNER_LIMITS), { status: 200 })));
    for (const line of [["computers", "--json"], ["computers", "set", HERE_PLACE_ID, "--threads", "2"], ["computers", "set", "boat", "--size", "4x8"]]) {
      const ran = await h.run(...line);
      expect(ran.io.errors, line.join(" ")).toEqual([]);
    }
    expect(asked).toEqual(["GET https://ascii.dev/api/box/v1/limits"]);
    expect((await boatRow()).settings).toEqual({ size: { cpu: 4, memMb: 8192 } });
    answer();
    await vi.waitFor(async () => expect((await h.rt.places!.list(Date.now())).find(p => p.id === "box")?.plan).toMatchObject({ key: "box_20" }));
  });

  it("leaves the plan and the starts off the row once a limits read fails, after a good one", async () => {
    let at = 0;
    let status = 200;
    await withBoat(async () => new Response(JSON.stringify(status === 200 ? OWNER_LIMITS : { code: "internal", message: "down" }), { status }), { now: () => at, sleep: async () => {} });
    const boat = async (): Promise<PlaceView | undefined> => (await h.rt.places!.list(Date.now())).find(p => p.id === "box");
    await vi.waitFor(async () => expect((await boat())?.starts?.day).toEqual({ used: 187, limit: 200 }));
    at = 60_000;
    status = 500;
    await vi.waitFor(async () => expect((await boat())?.plan).toBeUndefined());
    expect((await boat())?.starts).toBeUndefined();
  });

  it("answers a thread's own token wsp computers with Boat beside the computer its project is on", async () => {
    await withBoat();
    expect((await h.macProject("alpha")).code).toBe(0);
    const alpha = (await h.rt.workspaces.list()).find(w => w.name === "alpha")!;
    const scoped = await h.rt.devices.mint("thread t_lead", { kind: "thread", threadId: "t_lead", workspaceId: alpha.id, rootThreadId: "t_lead" }, Date.now());
    const env = { [HOST_URL_ENV]: `ws://127.0.0.1:${h.handle!.port}`, [HOST_TOKEN_ENV]: scoped.deviceToken, [HOST_KEY_ENV]: hostKeyHere(h.statePath) };
    const io = captured();
    const code = await cli(["computers", "--json"], io, undefined, env);
    expect(io.errors).toEqual([]);
    expect(code).toBe(0);
    const read = h.json(io).at(-1) as { computers: PlaceView[]; spend: { placeId: string }[]; pending: unknown[] };
    expect(read.computers.map(c => c.id)).toEqual(expect.arrayContaining([HERE_PLACE_ID, "box"]));
    expect(read.computers.find(c => c.id === "box")).toMatchObject({ kind: "provider", label: "Boat" });
    expect(read.computers.filter(c => c.road !== undefined)).toEqual([]);
    expect(read.pending).toEqual([]);
  });

  it("refuses a thread's own token every set on Boat, and changes nothing", async () => {
    await withBoat();
    expect((await h.macProject("alpha")).code).toBe(0);
    const alpha = (await h.rt.workspaces.list()).find(w => w.name === "alpha")!;
    const scoped = await h.rt.devices.mint("thread t_lead", { kind: "thread", threadId: "t_lead", workspaceId: alpha.id, rootThreadId: "t_lead" }, Date.now());
    const here = (await h.rt.places!.rows()).find(p => p.id === HERE_PLACE_ID)!.name;
    const env = { [HOST_URL_ENV]: `ws://127.0.0.1:${h.handle!.port}`, [HOST_TOKEN_ENV]: scoped.deviceToken, [HOST_KEY_ENV]: hostKeyHere(h.statePath) };
    const lines = [
      ["--size", "4x8"],
      ["--delete-after", "60"],
      ["--machines", "9"],
      ["--spend", "100"],
      ["--nap", "5"],
      ["--turn-limit", "off"],
      ["--spawn", "on", "--max-machines", "9", "--max-depth", "4"],
      ["--reset", "size"],
    ];
    for (const flags of lines) {
      const io = captured();
      const code = await cli(["computers", "set", "boat", ...flags], io, undefined, env);
      expect(code, flags.join(" ")).not.toBe(0);
      expect(io.errors, flags.join(" ")).toEqual([`wsp computers set: ${threadOpRefusal("places.set", "t_lead", here)}`]);
    }
    expect((await boatRow()).settings).toBeUndefined();
  });
});
