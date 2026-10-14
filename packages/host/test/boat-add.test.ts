// SPDX-License-Identifier: AGPL-3.0-only
// wsp add boat: the word in any case, the key asked for without echo, put to
// the same check the app's key step runs (one GET /limits among its reads),
// the key saved beside the state file and the plan line read off that answer.
// wsp add box keeps working. The check is the real one over a fake fetch, so
// each request is the one Boat would get.
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { PassThrough } from "node:stream";
import { afterAll, describe, expect, it, onTestFinished, vi } from "vitest";
import { BOX_PRICING, boxClassFor, limitsOf, keyCheckLine } from "@wsp/engine";
import { GOLDEN_STAGE_WORDS, boatAddedLine, boatForksNoFirstLine, boatSealLines, imageBuiltOnLine, imageHomeKeptLine, placeSaid, savedKeyStoppedLine, type PlaceView } from "@wsp/protocol";
import { buildImageAt } from "../src/verbs/workspaces-help.js";
import type { HostClient } from "../src/verbs.js";
import { addCommand, addLines, addRefusal } from "../src/places.js";
import { providerKeyCheck } from "../src/providers.js";
import { joinToken } from "@wsp/protocol";
import { KEY_LAYER_WORDS } from "../src/env-keys.js";
import { captured } from "./verbs-fixture.js";

/** The owner's account read on ascii.dev's base, in its field names. */
const PAID_BOX = { ok: true, type: "limits.info", accessTier: "paid", canStart: true, activeBoxes: 3, maxActiveBoxes: 100, boxPlanKey: "box_20", boxPlanDollars: 20, startLimits: { perMinute: 12, perHour: 60, perDay: 200 }, starts: { unlimited: false, minute: { limit: 12, used: 0, remaining: 12 }, hour: { limit: 60, used: 9, remaining: 51 }, day: { limit: 200, used: 187, remaining: 13 } }, billingStatus: "active" };
/** The same plan on boat.dev's documented base, in its sandbox field names. */
const PAID_SANDBOX = { ok: true, type: "limits.info", canStart: true, activeSandboxes: 1, maxActiveSandboxes: 100, sandboxPlanKey: "box_20", sandboxPlanDollars: 20, starts: { unlimited: false, minute: { limit: 12, used: 3, remaining: 9 }, hour: { limit: 60, used: 12, remaining: 48 }, day: { limit: 200, used: 47, remaining: 153 } }, billingStatus: "active" };
/** A trial that never paid. */
const TRIAL = { ok: true, type: "limits.info", accessTier: "trial", canStart: true, activeBoxes: 0, maxActiveBoxes: 2, boxPlanKey: "trial", trialComputeCapSeconds: 90_000, creditBalanceHours: 25, starts: { unlimited: false, minute: { limit: 5, used: 0 }, hour: { limit: 25, used: 0 }, day: { limit: 75, used: 0 } }, billingStatus: "trial" };

// The suite runs with the cloud off, and Boat is a row of the provider table only with it on: this file is about
// that row, so it reads the table as a host started with WSP_CLOUD=1 does.
vi.mock("../src/cloud.js", async original => ({ ...(await original<typeof import("../src/cloud.js")>()), CLOUD_ON: true }));

const PAID_LINE = "Boat added: $20 plan, 100 machines at once, 12 starts a minute, 60 an hour, 200 a day; small 2 vCPU 4 GB at $0.018/h until you pick another size.";
const TRIAL_LINE = "Boat added: trial (25 h, 2 machines at once, 5 starts a minute, 25 an hour, 75 a day); small and default sizes.";

const KEY = "boat_live_typed";

const dirs: string[] = [];
afterAll(() => {
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});
const tmp = (name: string): string => {
  const dir = mkdtempSync(join(tmpdir(), `wsp-${name}-`));
  dirs.push(dir);
  return dir;
};
const opts = (home: string, env: Record<string, string | undefined>): Parameters<typeof addCommand>[1] => ({ statePath: join(home, "state.json"), home, env: { HOME: home, WSP_HOME: home }, providerEnv: env });

/** Every road a provider's add never takes refuses, so a case that reached one fails on it. */
const never = (road: string) => (): Promise<never> => Promise.reject(new Error(`a provider's add never ${road}`));
const DEPS: NonNullable<Parameters<typeof addCommand>[4]> = {
  dial: never("dials a host"),
  now: () => 0,
  run: never("runs a service manager"),
  platform: "linux",
  checkKey: never("puts a key to the real provider"),
  terminal: { input: new PassThrough() as never, output: new PassThrough() as never },
  open: async () => false,
  placeLink: never("links a computer"),
  signIn: never("signs an agent in"),
  heldHostKey: never("reads a host key"),
  offeredHostKey: never("scans a host key"),
  sshWord: never("reads the ssh config"),
};

/** A Boat that answers /limits with one body, or refuses every key with a 401, and keeps every request it was sent. */
function fakeBoat(answer: unknown, status = 200) {
  return fakeBoatFor(() => (status === 200 ? answer : undefined));
}

/** A Boat that answers each key's /limits with its own body and every other read as a key holding every action does,
 * refusing with a 401 a key it has none for. `limits` is the GET /limits each check makes. */
function fakeBoatFor(answerOf: (key: string) => unknown) {
  const asked: { method: string; url: string; auth: string | null }[] = [];
  const fetch = (async (input: string | URL | Request, init?: RequestInit) => {
    const auth = new Headers(init?.headers).get("authorization");
    const url = String(input);
    asked.push({ method: init?.method ?? "GET", url, auth });
    const answer = answerOf((auth ?? "").replace(/^Bearer /, ""));
    if (answer === undefined) return Response.json({ ok: false, error: { code: "unauthorized", message: "invalid API key" } }, { status: 401 });
    const path = new URL(url).pathname;
    if (path.endsWith("/limits")) return Response.json(answer);
    if (path.endsWith("/me")) return Response.json({ ok: true, user: { login: "acme-dev" } });
    if (/\/boxes\/[^/]+\//.test(path)) return Response.json({ ok: false, error: { code: "not_found" } }, { status: 404 });
    return Response.json({ ok: true, boxes: [], apiKeys: [] });
  }) as typeof globalThis.fetch;
  // The add puts the key to the provider row's own check, which reads Boat with the global fetch.
  const checkKey: typeof DEPS.checkKey = async (env, key, provider) => {
    vi.stubGlobal("fetch", fetch);
    onTestFinished(() => void vi.unstubAllGlobals());
    return providerKeyCheck(env, key, provider);
  };
  return {
    asked,
    get limits() {
      return asked.filter(a => a.url.endsWith("/limits"));
    },
    deps: { ...DEPS, checkKey },
  };
}

/** A terminal that types the key at every secret prompt and keeps what it was asked. */
function terminal(typed: string) {
  const io = captured();
  const prompts: { question: string; name: string }[] = [];
  io.isTTY = true;
  io.askSecret = async (question, name) => (prompts.push({ question, name: name ?? "" }), typed);
  io.ask = async question => {
    throw new Error(`wsp add saves the key it took without asking, and was asked: ${question}`);
  };
  return { io, prompts };
}

/** The key ask's first prompt where nothing on this computer holds a Boat key. */
const FIRST_PROMPT = { question: `Boat API key\nNo BOAT_API_KEY in ${KEY_LAYER_WORDS}.\nboat.dev/dashboard`, name: "BOAT_API_KEY" };

describe("Boat's plan line", () => {
  const small = boxClassFor(BOX_PRICING.defaultSize);

  it("reads the plan off the engine's reading of either base's field names and says it as one line", () => {
    expect(boatAddedLine(limitsOf(PAID_BOX), small)).toBe(PAID_LINE);
    expect(boatAddedLine(limitsOf(PAID_SANDBOX), small)).toBe(PAID_LINE);
    expect(boatAddedLine(limitsOf(TRIAL), small)).toBe(TRIAL_LINE);
  });

  it("says only what the answer held, and the size alone where nothing was read", () => {
    expect(boatAddedLine(limitsOf({ boxPlanKey: "box_100" }), small)).toBe("Boat added: $100 plan; small 2 vCPU 4 GB at $0.018/h until you pick another size.");
    expect(boatAddedLine(limitsOf({ boxPlanKey: "box_20", starts: { unlimited: true } }), small)).toBe("Boat added: $20 plan; small 2 vCPU 4 GB at $0.018/h until you pick another size.");
    expect(boatAddedLine(undefined, small)).toBe("Boat added; small 2 vCPU 4 GB at $0.018/h until you pick another size.");
    expect(boatAddedLine(undefined, { ...small, rateUsdPerHour: 0.09 })).toContain("at $0.09/h until");
  });
});

describe("wsp add boat", () => {
  it("asks for the key without echo in any case of the word, checks it with one GET /limits among its reads, saves it and prints the plan line", async () => {
    for (const word of ["boat", "Boat", "BOAT"]) {
      const home = tmp(`add-boat-${word}`);
      const boat = fakeBoat(PAID_BOX);
      const { io, prompts } = terminal(KEY);
      expect(await addCommand(io, opts(home, {}), [word], {}, boat.deps)).toBe(0);
      // Asked once, at the secret prompt, by the key's own name and where it is made.
      expect(prompts).toEqual([FIRST_PROMPT]);
      expect(boat.limits).toEqual([{ method: "GET", url: "https://ascii.dev/api/box/v1/limits", auth: `Bearer ${KEY}` }]);
      expect(new Set(boat.asked.map(a => a.auth))).toEqual(new Set([`Bearer ${KEY}`]));
      // The key alone is saved: Boat opens as a place beside this computer, and the provider it forks on stays.
      expect(readFileSync(join(home, ".env"), "utf8")).toBe(`BOAT_API_KEY=${KEY}\n`);
      expect(io.lines).toEqual([PAID_LINE]);
      expect(io.screen).not.toContain(KEY);
    }
  });

  it("prints a trial's line off a trial's answer", async () => {
    const home = tmp("add-boat-trial");
    const { io } = terminal(KEY);
    expect(await addCommand(io, opts(home, {}), ["boat"], {}, fakeBoat(TRIAL).deps)).toBe(0);
    expect(io.lines).toEqual([TRIAL_LINE]);
  });

  it("takes a key the run's own environment carries without asking, and wsp add box still works", async () => {
    const home = tmp("add-box-env");
    const boat = fakeBoat(PAID_SANDBOX);
    const io = captured();
    expect(await addCommand(io, opts(home, { BOAT_API_KEY: KEY }), ["box"], {}, boat.deps)).toBe(0);
    expect(boat.limits).toHaveLength(1);
    expect(readFileSync(join(home, ".env"), "utf8")).toBe(`BOAT_API_KEY=${KEY}\n`);
    expect(io.lines).toEqual([PAID_LINE]);
  });

  it("asks again with Boat's refusal as the reason, and after three refused keys stops on the auth class with nothing written", async () => {
    const home = tmp("add-boat-refused");
    const boat = fakeBoat(undefined, 401);
    const { io, prompts } = terminal("boat_live_wrong");
    await expect(addCommand(io, opts(home, {}), ["boat"], {}, boat.deps)).rejects.toMatchObject({ kind: "auth", message: expect.stringMatching(/^Boat refused this key: .*401/) });
    expect(prompts.map(p => p.question.split("\n")[1])).toEqual([FIRST_PROMPT.question.split("\n")[1], expect.stringMatching(/^Boat refused this key: .*401/), expect.stringMatching(/^Boat refused this key: .*401/)]);
    expect(boat.limits).toHaveLength(3);
    expect(existsSync(join(home, ".env"))).toBe(false);
  });

  it("puts a saved key to Boat, and at a terminal asks for another when Boat refuses it, saving the one it takes", async () => {
    const home = tmp("add-boat-saved-refused");
    writeFileSync(join(home, ".env"), "BOAT_API_KEY=boat_live_revoked\n");
    const boat = fakeBoatFor(key => (key === KEY ? PAID_BOX : undefined));
    const { io, prompts } = terminal(KEY);
    expect(await addCommand(io, opts(home, { BOAT_API_KEY: "boat_live_revoked" }), ["boat"], {}, boat.deps)).toBe(0);
    expect(boat.limits.map(a => a.auth)).toEqual(["Bearer boat_live_revoked", `Bearer ${KEY}`]);
    expect(prompts).toHaveLength(1);
    expect(prompts[0]!.question).toMatch(/^Boat API key\nBoat refused the saved key: .*401/);
    expect(readFileSync(join(home, ".env"), "utf8")).toBe(`BOAT_API_KEY=${KEY}\n`);
    expect(io.lines).toEqual([PAID_LINE]);
  });

  it("takes a saved key Boat still answers for without asking, and says its plan again", async () => {
    const home = tmp("add-boat-saved");
    writeFileSync(join(home, ".env"), `BOAT_API_KEY=${KEY}\n`);
    const boat = fakeBoat(TRIAL);
    const { io, prompts } = terminal("never typed");
    expect(await addCommand(io, opts(home, {}), ["boat"], {}, boat.deps)).toBe(0);
    expect(prompts).toEqual([]);
    expect(boat.limits).toHaveLength(1);
    expect(io.lines).toEqual([TRIAL_LINE]);
  });

  it("refuses an empty key as the auth class, with nothing written", async () => {
    const home = tmp("add-boat-empty");
    const { io } = terminal("  ");
    await expect(addCommand(io, opts(home, {}), ["boat"], {}, fakeBoat(PAID_BOX).deps)).rejects.toMatchObject({ kind: "auth", message: expect.stringContaining("BOAT_API_KEY is needed") });
    expect(existsSync(join(home, ".env"))).toBe(false);
  });
});

describe("what the command line says about Boat", () => {
  /** Boat's id as a word on its own, which a line about Boat must never print. */
  const BOX_WORD = /\bbox\b/i;

  it("names Boat, never box, on every add line that names a provider", async () => {
    const said = [addRefusal("nonsense"), ...addLines(joinToken("X", `SHA256:${"b".repeat(43)}`), 600_000, 0, ["http://10.0.0.2:4400"], undefined)].join("\n");
    expect(said).toContain("boat");
    expect(said).not.toMatch(BOX_WORD);
    const home = tmp("add-boat-words");
    const { io } = terminal(KEY);
    await addCommand(io, opts(home, {}), ["boat"], {}, fakeBoat(PAID_BOX).deps);
    const refused = terminal("boat_live_wrong");
    const refusal = await addCommand(refused.io, opts(tmp("add-boat-words-refused"), {}), ["boat"], {}, fakeBoat(undefined, 401).deps).catch((e: Error) => e.message);
    const prompts = terminal(KEY);
    await addCommand(prompts.io, opts(tmp("add-boat-words-prompt"), {}), ["Boat"], {}, fakeBoat(TRIAL).deps);
    for (const screen of [io.screen, refused.io.screen, String(refusal), ...refused.prompts.map(p => p.question), prompts.io.screen, ...prompts.prompts.map(p => p.question)]) expect(screen).not.toMatch(BOX_WORD);
  });

  it("names Boat, never box, on every stage line wsp image build boat streams", async () => {
    const frames = new Set<(frame: Record<string, unknown>) => void>();
    const boat: PlaceView = { id: "box", kind: "provider", name: "box", default: false, rateUsdPerHour: 0.018, takesForks: true };
    const client = {
      request: async (op: string) => {
        if (op === "places.list") return { places: [boat] };
        if (op === "image.build") {
          for (const fn of frames) fn({ type: "golden.stage", name: "default", stage: "creating", place: "box" });
          return { build: {} };
        }
        return { view: { image: null, copies: [], projects: [] } };
      },
      events: async () => {},
      onFrame: (fn: (frame: Record<string, unknown>) => void) => (frames.add(fn), () => frames.delete(fn)),
    } as unknown as HostClient;
    let streamed = "";
    await expect(buildImageAt(client, { emit: () => {}, stream: text => void (streamed += text) }, "boat")).rejects.toThrow();
    expect(streamed).toBe(`Boat: ${GOLDEN_STAGE_WORDS.creating}\n`);
  });

  it("names Boat, never box, on every line wsp init --on boat prints about where the image is built and what the seal leaves", () => {
    const boat = { id: "box", name: "box", kind: "provider" };
    const lines = [
      imageBuiltOnLine(placeSaid(boat)),
      imageHomeKeptLine("solari", boat)!,
      ...boatSealLines([]),
      keyCheckLine({ state: "refused", said: "401 Unauthorized" }, "box", true)!,
      keyCheckLine({ state: "unchecked", said: "fetch failed" }, "box", true)!,
      savedKeyStoppedLine("box"),
    ];
    expect(lines[0]).toBe("your image is built on Boat");
    expect(lines[1]).toBe("your image lives on Boat, so it is built there and not on solari");
    for (const line of lines) expect(line).not.toMatch(BOX_WORD);
    // --on boat names the cloud whose id is box, so there is no other home to say the image stays on.
    expect(imageHomeKeptLine("boat", boat)).toBeUndefined();
    expect(imageHomeKeptLine("Boat", boat)).toBeUndefined();
    // A computer somebody named box is still box: only a cloud's row is named by its id.
    expect(imageBuiltOnLine(placeSaid({ name: "box", kind: "computer" }))).toBe("your image is built on box");
    expect(boatForksNoFirstLine(["--first-workspace", "--import"])).toBe("Boat forks no first machine, so --first-workspace and --import do nothing here.");
  });
});
