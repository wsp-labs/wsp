// SPDX-License-Identifier: AGPL-3.0-only
// The Boat canary: one box made the way wsp makes one, from a named snapshot, and driven through every call wsp's
// BoxBackend sends: the create and two replays under its key, the rename and the stop timer, the environment,
// commands, PUT and GET /files, the daemon's token and hosted route, stop, resume, delete, the listing, and the
// account's limits. Every call is recorded off the wire, so a refused body fails naming Boat's answer next to the body
// sent, and an answer that lost a field wsp reads fails naming the field. It records the seconds a sub-thread
// waits on: create to ready, the environment landing, the daemon's first answer, stop and resume. Everything it made
// is deleted by id and read gone.

import { createHash, randomBytes, randomUUID } from "node:crypto";
import { DAEMON_TOKEN_PATH } from "@wsp/protocol";
import { afterAll, describe, expect, it } from "vitest";
import WebSocket from "ws";
import { BOX_API_URL, BOX_BUDGETS, BOX_CLASSES, BOX_CREATE_MS, BOX_PAUSE_MS, BOX_RESUME_MS, BoxBackend, BoxMachine, boxName } from "../src/box-backend.js";
import { DISK_USE_CMD, diskUseOf } from "../src/disk-sync.js";
import { isMissing, PROVIDER_READ_CAP_MS, type WspError } from "../src/errors.js";
import { CREATED_AT_LABEL, OWNER_LABEL, WSP_LABEL } from "../src/labels.js";
import { landBytes } from "../src/land-bytes.js";
import { DAEMON_PORT } from "../src/preview.js";
import { nameOwner } from "../src/snapshot-names.js";
import { boatSaid, checkReads, missingField, recordingFetch, shown, type Exchange } from "./boat-wire.js";
import { boatKey, LIVE, sleep } from "./live.js";

/** A foreign owner, so no host's sweep claims this box. */
const OWNER = "h_canary";
const LABELS = { [WSP_LABEL]: "1", [OWNER_LABEL]: OWNER, "wsp-test": "box-canary", [CREATED_AT_LABEL]: new Date().toISOString() };
/** The create's stop timer, then the one the PATCH moves it to: far enough apart that the backstop sends it. */
const IDLE_MS = 15 * 60_000;
const TIMER_MS = 40 * 60_000;
/** The runtime starts a silent daemon itself after this long, and again only when a start failed (reach.ts). */
const START_DAEMON_AFTER_MS = 60_000;
const ASK_DAEMON_MS = 5_000;
/** A Claude Code session file read about 536 KB gzipped, and a kept one comes off the box through GET /files. */
const READ_BYTES = 600 * 1024;
const RUNNING = new Set(["ready", "idle", "running"]);
const STARTS_NEEDED = 3; // the create, the resume and the resume a size up
const GONE_MS = 120_000;
const MINUTE = 60_000;

const secs = (ms: number): string => `${(ms / 1000).toFixed(1)} s`;
const startedLine = (starts: number): string => (starts === 0 ? "" : `, after wsp started it${starts > 1 ? ` (${starts} starts)` : ""}`);
interface BoxView { id?: string; state?: string; name?: string | null; archiveAfter?: string | null; hydrated?: boolean; type?: string }
interface BoxReply { status?: string; code?: string; sandbox?: BoxView }
const reply = (e: Exchange): BoxReply => JSON.parse(e.text || "{}") as BoxReply;

interface Limits {
  accessTier?: string;
  maxActiveSandboxes?: number;
  activeSandboxes?: number;
  canStart?: boolean;
  startBlockedReason?: string | null;
  sandboxPlanKey?: string;
  starts?: Record<"minute" | "hour" | "day", { limit: number; used: number; remaining: number }>;
}

/** One dial of the daemon over its hosted route, as the runtime dials: the token as the first frame, then a ping. A
 * route that answers the upgrade with a status of its own is silent too, and says which. */
function ping(url: string, token: string): Promise<{ said: "answered" | "refused" | "silent"; door?: string }> {
  const ws = new WebSocket(url.replace(/^http/, "ws"));
  return new Promise(resolve => {
    const end = (said: "answered" | "refused" | "silent", door?: string): void => {
      clearTimeout(cap);
      ws.removeAllListeners();
      ws.on("error", () => {});
      ws.terminate();
      resolve(door === undefined ? { said } : { said, door });
    };
    const cap = setTimeout(() => end("silent"), ASK_DAEMON_MS);
    ws.on("unexpected-response", (_req, res) => end("silent", `${res.statusCode} ${res.statusMessage}`));
    ws.on("open", () => ws.send(JSON.stringify({ id: 1, op: "auth", token })));
    ws.on("message", raw => {
      let m: { id?: unknown; ok?: unknown };
      try {
        m = JSON.parse(String(raw)) as typeof m;
      } catch {
        return;
      }
      if (m.id === 1) {
        if (m.ok === true) ws.send(JSON.stringify({ id: 2, op: "ping" }));
        else end("refused");
      } else if (m.id === 2) end(m.ok === true ? "answered" : "refused");
    });
    ws.on("close", code => end(code === 4401 ? "refused" : "silent"));
    ws.on("error", () => end("silent"));
  });
}

describe.runIf(LIVE)("Boat canary, live", () => {
  const wire: Exchange[] = [];
  const recorded = recordingFetch(wire, BOX_API_URL);
  /** Set just before the create, so its first answer is followed at once by the same request, as a lost answer is. */
  let replayAtOnce = false;
  const backend = LIVE
    ? new BoxBackend({
        apiKey: boatKey(),
        fetch: async (input, init) => {
          const res = await recorded(input, init);
          if (replayAtOnce && init?.method === "POST" && String(input) === `${BOX_API_URL}/sandboxes`) {
            replayAtOnce = false;
            // A cap of its own: the first ask's signal has been running since that ask was sent.
            await recorded(input, { ...init, signal: AbortSignal.timeout(PROVIDER_READ_CAP_MS) }).catch(() => {});
          }
          return res;
        },
      })
    : (undefined as never);
  const readings: [string, string][] = [];
  const note = (what: string, value: string): void => {
    readings.push([what, value]);
  };
  const key = randomUUID();
  const token = randomBytes(24).toString("hex");
  let image: string | undefined;
  let machine: BoxMachine | undefined;
  let sentAt = 0;
  let readyAt = 0;
  let startsBefore: number | undefined;
  const gone = new Set<string>();

  const box = (): BoxMachine => {
    if (machine === undefined) throw new Error("no box to act on: the create failed above");
    return machine;
  };

  /** Runs one move through wsp's own code; a failure names the last call Boat refused inside it, and an answer that
   * lost a field wsp reads fails the move even when the code went on. */
  async function step<T>(what: string, run: () => Promise<T>): Promise<T> {
    const from = wire.length;
    let out: T;
    try {
      out = await run();
    } catch (e) {
      const said = boatSaid(wire, from);
      throw new Error(`${what} failed: ${e instanceof Error ? e.message : String(e)}${said !== undefined ? `; Boat: ${said}` : ""}`, { cause: e });
    }
    checkReads(wire, from);
    return out;
  }

  /** The box's own reading, with the fields wsp reads off a ready box checked. */
  async function view(fields: readonly string[] = []): Promise<BoxView> {
    await step(`GET /sandboxes/${box().id}`, () => backend.view(box().id));
    const e = wire.at(-1)!;
    const lost = missingField(e, fields);
    if (lost !== undefined) throw new Error(`Boat's answer lost ${lost}, which wsp reads: ${shown(e)}`);
    return reply(e).sandbox ?? {};
  }

  const hydrated = (v: BoxView = {}): string => ("hydrated" in v ? String(v.hydrated) : "not in the answer");

  /** Lands this canary's token, mints the daemon's route and dials it until it answers, starting the daemon the way
   * the runtime does when it stays silent. Answers when it answered and how many starts it took. */
  async function daemonAnswer(): Promise<{ at: number; starts: number }> {
    const m = box();
    await step("daemon token", () => landBytes(m, DAEMON_TOKEN_PATH, new TextEncoder().encode(token)));
    const { url } = await step("POST /host", () => m.previewUrl(DAEMON_PORT));
    const began = Date.now();
    let starts = 0;
    let startAt = began + START_DAEMON_AFTER_MS;
    for (;;) {
      const { said, door } = await ping(url, token);
      if (said === "answered") return { at: Date.now(), starts };
      if (said === "refused") throw new Error(`the daemon on ${m.id} refused the token wsp put at ${DAEMON_TOKEN_PATH}`);
      if (Date.now() - began >= BOX_BUDGETS.daemonAnswersMs) throw new Error(`the daemon on ${m.id} did not answer in ${secs(Date.now() - began)}${startedLine(starts)}${door === undefined ? "" : `; its route answered the last dial ${door}`}`);
      if (Date.now() >= startAt) {
        starts++;
        const r = await m.startDaemon().catch((e: unknown) => ({ exitCode: -1, stderr: e instanceof Error ? e.message : String(e) }));
        startAt = r.exitCode === 0 ? Number.POSITIVE_INFINITY : Date.now() + START_DAEMON_AFTER_MS;
      }
      await sleep(1_000);
    }
  }

  async function readGone(id: string): Promise<number> {
    const began = Date.now();
    for (;;) {
      try {
        await backend.view(id);
      } catch (e) {
        if (isMissing(e)) {
          gone.add(id);
          return Date.now() - began;
        }
        throw e;
      }
      if (Date.now() - began >= GONE_MS) throw new Error(`${id} still reads at Boat ${secs(GONE_MS)} after its delete`);
      await sleep(2_000);
    }
  }

  /** Every box any create answered with, so the cleanup reaches a replay that made a second one. */
  const made = (): string[] => [...new Set(wire.filter(e => e.method === "POST" && e.path === "/sandboxes" && e.status !== null && e.status < 300).map(e => reply(e).sandbox?.id).filter((id): id is string => typeof id === "string"))];

  afterAll(async () => {
    if (!LIVE) return;
    const alive: string[] = [];
    for (const id of made()) {
      if (gone.has(id)) continue;
      await new BoxMachine(backend, id, "sandbox").kill().then(() => readGone(id)).catch((e: unknown) => alive.push(`${id} (${e instanceof Error ? e.message : String(e)})`));
    }
    console.log(["[box-canary] readings", ...readings.map(([k, v]) => `  ${k}: ${v}`), `  boxes made: ${made().join(", ") || "none"}; ${alive.length === 0 ? "every one deleted and read gone" : `still at Boat: ${alive.join(", ")}`}`].join("\n"));
    if (alive.length > 0) throw new Error(`boxes still at Boat after the canary: ${alive.join(", ")}`);
  }, 5 * MINUTE);

  it("reads the account's limits", async () => {
    const limits = await step("GET /limits", () => backend.request<Limits>("GET", "/limits"));
    const s = limits.starts!;
    startsBefore = s.day.used;
    note("plan", `${limits.accessTier} ${limits.sandboxPlanKey}, ${limits.activeSandboxes} of ${limits.maxActiveSandboxes} boxes running, starts left ${s.minute.remaining}/min ${s.hour.remaining}/h ${s.day.remaining}/day`);
    expect(limits.canStart, `Boat will make no start now: ${limits.startBlockedReason}`).toBe(true);
    expect(Math.min(s.minute.remaining, s.hour.remaining, s.day.remaining), `the canary spends ${STARTS_NEEDED} starts and Boat has fewer left in one window`).toBeGreaterThanOrEqual(STARTS_NEEDED);
  });

  it("finds a wsp image among the named snapshots", async () => {
    const rows = await step("GET /named-snapshots", () => backend.listTemplates());
    const asked = process.env.BOAT_CANARY_IMAGE;
    const pick = asked
      ? rows.find(r => r.name === asked)
      : rows.filter(r => r.status === "ready" && nameOwner(r.name) !== undefined && /-v\d+$/.test(r.name)).sort((a, b) => (b.createdAt ?? "").localeCompare(a.createdAt ?? ""))[0];
    expect(pick, asked ? `no named snapshot is called ${asked}` : "no ready wsp image on the account; seal one with wsp init --on boat, or name one with BOAT_CANARY_IMAGE").toBeDefined();
    expect(pick!.status, `${pick!.name} reads ${pick!.status}`).toBe("ready");
    image = pick!.name;
    note("image", image);
  });

  it("creates a box from it, and the same key and bytes give the same box back", async () => {
    if (image === undefined) throw new Error("no image to create from: the read above failed");
    replayAtOnce = true;
    const from = wire.length;
    machine = (await step("create", () => backend.create({ kind: "sandbox", fromSnapshot: image, cpu: 2, memMb: 4096, labels: LABELS, idleTimeoutMs: IDLE_MS, idempotencyKey: key }))) as BoxMachine;
    const creates = wire.slice(from).filter(e => e.method === "POST" && e.path === "/sandboxes");
    const [first, atOnce] = creates;
    sentAt = first!.at;
    const id = machine.id;
    expect(reply(first!).sandbox?.id).toBe(id);
    const ready = wire.slice(from).find(e => e.method === "GET" && e.path === `/sandboxes/${id}` && e.status === 200 && RUNNING.has(reply(e).sandbox?.state ?? ""));
    readyAt = ready!.at + ready!.ms;
    note("create to ready", `${secs(readyAt - sentAt)} (${id}), handed back after the command check and the rename at ${secs(Date.now() - sentAt)}`);
    note("create answered", `${first!.status} status ${reply(first!).status}`);

    // A create whose answer was lost is asked again at once: Boat gives the same box or says the first is still minting.
    const atOnceReply = reply(atOnce!);
    const sameBox = atOnce!.status !== null && atOnce!.status < 300 && atOnceReply.sandbox?.id === id;
    const stillMinting = atOnce!.status === 409 && atOnceReply.code === "idempotency_in_progress";
    expect(atOnce!.sent, "the replay at once did not carry the first create's bytes").toBe(first!.sent);
    expect(sameBox || stillMinting, `a replay of the create at once got neither the same box nor idempotency_in_progress: ${shown(atOnce!)}`).toBe(true);
    note("replay at once", sameBox ? `${atOnce!.status} status ${atOnceReply.status}, the same box` : "409 idempotency_in_progress");

    const later = await step("replay of the create", () => backend.request<{ status?: string; sandbox: { id: string } }>("POST", "/sandboxes", { body: JSON.parse(first!.sent!), headers: { "Idempotency-Key": key }, once: true }));
    expect(wire.at(-1)!.sent, "the replay after ready did not carry the first create's bytes").toBe(first!.sent);
    expect(later.sandbox.id, `a replay under ${key} made another box: ${shown(wire.at(-1)!)}`).toBe(id);
    note("replay after ready", `${wire.at(-1)!.status} status ${later.status}, the same box`);

    // A replay is safe only while Boat refuses a changed body under a used key rather than making a second box; a
    // stop timer computed again per call is the change that would trip it.
    const sent = JSON.parse(first!.sent!) as { ttlSeconds: number | null };
    const changed = { ...sent, ttlSeconds: (sent.ttlSeconds ?? 0) + 60 };
    const refusal = await backend.request("POST", "/sandboxes", { body: changed, headers: { "Idempotency-Key": key }, once: true }).then(
      () => undefined,
      (e: unknown) => e as WspError,
    );
    expect(refusal?.code, `a changed body under a used key was not refused idempotency_key_reused: ${shown(wire.at(-1)!)}`).toBe("idempotency_key_reused");
    note("changed body, same key", `${refusal!.status} ${refusal!.code}`);
    note("hydrated at ready", hydrated(reply(ready!).sandbox));
  }, BOX_CREATE_MS + 2 * MINUTE);

  it("lands the environment for the daemon", async () => {
    const began = Date.now();
    await step("environment landing", () => box().landEnvs({ WSP_CANARY: "1" }));
    note("environment landing", secs(Date.now() - began));
    const read = await step("command", () => box().exec("cat /etc/wsp/daemon.env"));
    expect(read.stdout, `the daemon's environment file reads: ${read.stdout}${read.stderr}`).toContain('WSP_CANARY="1"');
  }, 4 * MINUTE);

  it("hands the daemon its token and reads its first answer", async () => {
    const there = await step("command", () => box().exec(`test -f ${DAEMON_TOKEN_PATH} && echo yes || echo no`));
    expect(there.stdout.trim(), `${image} holds no daemon token at ${DAEMON_TOKEN_PATH}, so it is no wsp image; name one with BOAT_CANARY_IMAGE`).toBe("yes");
    const answer = await daemonAnswer();
    note("daemon's first answer", `${secs(answer.at - sentAt)} after the create was sent, ${secs(answer.at - readyAt)} after ready${startedLine(answer.starts)}`);
  }, BOX_BUDGETS.daemonAnswersMs + 2 * MINUTE);

  it("carries wsp's name and moves the stop timer", async () => {
    const id = box().id;
    const renamed = wire.some(e => e.method === "PATCH" && e.path === `/sandboxes/${id}` && e.sent?.includes('"name"'));
    note("rename", renamed ? "sent: Boat named the box after its snapshot, as measured 2026-09-11" : "not needed: Boat named the box as the create asked");
    const named = await view(["sandbox.name", "sandbox.type", "sandbox.vcpu", "sandbox.memoryGB", "sandbox.createdAt", "sandbox.archiveAfter"]);
    expect(named.name, `the box reads a name wsp did not give it: ${shown(wire.at(-1)!)}`).toBe(boxName(LABELS));
    const listed = await step("GET /sandboxes", () => backend.list({ [OWNER_LABEL]: OWNER }));
    expect(listed.map(b => b.id), "the listing the sweep reads does not carry the box under its owner").toContain(id);

    await step("PATCH ttlSeconds", () => backend.backstop(box(), Date.now() + TIMER_MS));
    const patch = wire.filter(e => e.method === "PATCH" && e.path === `/sandboxes/${id}` && e.sent?.includes('"ttlSeconds"')).at(-1);
    expect(patch, "the backstop sent no PATCH").toBeDefined();
    expect(patch!.date, `Boat's answer to the PATCH carried no Date header to read the timer against: ${shown(patch!)}`).toBeDefined();
    const timed = await view(["sandbox.archiveAfter"]);
    // Read on Boat's own clock, off the PATCH answer's Date header, so a skew between the two computers reads nothing.
    const asked = Date.parse(patch!.date ?? "") + (JSON.parse(patch!.sent!) as { ttlSeconds: number }).ttlSeconds * 1000;
    const off = Date.parse(String(timed.archiveAfter)) - asked;
    expect(Math.abs(off), `the stop timer reads ${timed.archiveAfter}, ${secs(off)} off what the PATCH asked for: ${shown(patch!)}`).toBeLessThan(MINUTE);
  }, 3 * MINUTE);

  it("runs commands and answers their code and output", async () => {
    const r = await step("command", () => box().exec("echo canary; echo said >&2; exit 3"));
    expect({ exitCode: r.exitCode, stdout: r.stdout, stderr: r.stderr.trim() }).toEqual({ exitCode: 3, stdout: "canary\n", stderr: "said" });
  }, 2 * MINUTE);

  it("puts a file and reads it back through GET /files", async () => {
    const id = box().id;
    const bytes = randomBytes(READ_BYTES);
    const path = `/tmp/wsp-canary-${randomBytes(4).toString("hex")}.bin`;
    let began = Date.now();
    await step("PUT /files", () => box().putBytes(path, bytes));
    note("PUT /files", `${READ_BYTES} bytes in ${secs(Date.now() - began)}`);
    began = Date.now();
    const read = await step("GET /files", () => backend.request<{ content: string; encoding: string; size: number }>("GET", `/sandboxes/${encodeURIComponent(id)}/files?path=${encodeURIComponent(path)}&encoding=base64`));
    note("GET /files", `${read.size} bytes in ${secs(Date.now() - began)}`);
    const sha = (b: Uint8Array): string => createHash("sha256").update(b).digest("hex");
    expect({ encoding: read.encoding, size: read.size, sha: sha(Buffer.from(read.content, "base64")) }).toEqual({ encoding: "base64", size: READ_BYTES, sha: sha(bytes) });
    await step("command", () => box().exec(`rm -f ${path}`));
  }, 3 * MINUTE);

  it("stops and resumes it, and its daemon answers again", async () => {
    let began = Date.now();
    await step("stop", () => box().pause());
    note("stop", secs(Date.now() - began));
    began = Date.now();
    await step("resume", () => box().resume());
    note("resume", secs(Date.now() - began));
    note("hydrated after resume", hydrated(await view()));
    const answer = await daemonAnswer();
    note("daemon's first answer after resume", `${secs(answer.at - began)} after the resume was sent${startedLine(answer.starts)}`);
  }, BOX_PAUSE_MS + BOX_RESUME_MS + BOX_BUDGETS.daemonAnswersMs + 2 * MINUTE);

  it("reads its disk where Boat's own agent does, and moves it a size up as one stop and one resume naming the class", async () => {
    const disk = async (): Promise<string> => {
      const res = await step("disk", () => box().exec(DISK_USE_CMD));
      const use = diskUseOf(res.stdout);
      if (use === undefined) throw new Error(`df on ${box().id} answered ${JSON.stringify(res.stdout)} (exit ${res.exitCode})`);
      return `${use.pct.toFixed(1)}% of ${((use.usedBytes + use.freeBytes) / 1e9).toFixed(1)} GB used`;
    };
    /** Cores, memory and the root disk as the box itself reads them: the disk Boat's size names is not the one that fills. */
    const hardware = async (): Promise<string> => {
      const res = await step("hardware", () => box().exec("nproc; grep MemTotal /proc/meminfo; lsblk -b -d -n -o SIZE /dev/vda"));
      return res.stdout.trim().split("\n").join(", ");
    };
    const was = (await view()).type;
    note(`disk on ${was}`, await disk());
    note(`hardware on ${was}`, await hardware());
    const at = BOX_CLASSES.findIndex(c => c.type === was);
    const sold = (await backend.limits()).sizes;
    const next = BOX_CLASSES.slice(at + 1).find(c => sold.includes(c.type));
    if (next === undefined) throw new Error(`the account sells no size above ${was} to move to`);
    let began = Date.now();
    await step("stop", () => box().pause());
    note("stop before the move", secs(Date.now() - began));
    began = Date.now();
    await step(`resume at ${next.type}`, () => box().resumeAt(next));
    note(`resume at ${next.type}`, secs(Date.now() - began));
    expect((await view()).type).toBe(next.type);
    note(`disk on ${next.type}`, await disk());
    note(`hardware on ${next.type}`, await hardware());
  }, BOX_PAUSE_MS + BOX_RESUME_MS + 3 * MINUTE);

  it("deletes it by id and reads it gone", async () => {
    const id = box().id;
    await step("DELETE", () => box().kill());
    note("delete", `operation ${box().deletion ?? "none"}, read gone ${secs(await readGone(id))} after`);
    const after = await step("GET /limits", () => backend.request<Limits>("GET", "/limits"));
    if (startsBefore !== undefined) note("starts counted today", `${after.starts!.day.used - startsBefore} (${STARTS_NEEDED} are the create and the two resumes; more is a replay or another client)`);
  }, 4 * MINUTE);
});
