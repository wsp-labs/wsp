// SPDX-License-Identifier: AGPL-3.0-only
// A host whose provider is Boat reads its storage off the named snapshots alone: Boat's own history is free, never
// this host's, and pages for minutes on an account that has run boxes for a while.
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { BoxBackend } from "@wsp/engine";
import { createRuntime, memoryStore, type Runtime } from "@wsp/runtime";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CliIO } from "../src/cli.js";
import { forkDoctor } from "../src/doctor.js";
import { startHost, type HostHandle } from "../src/server.js";

const HISTORY_PAGES = 5;

/** Boat as it answers a host's reads: two named snapshots of another host's, an empty box listing, and a history
 * that runs for pages. Every request is kept as its method and path. */
function boat(): { fetch: typeof globalThis.fetch; seen: string[] } {
  const seen: string[] = [];
  const json = (body: unknown, status = 200): Response => new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });
  const fetch = (async (input: string | URL | Request, init: RequestInit = {}) => {
    const url = new URL(String(input));
    const path = url.pathname.replace(/^\/api\/v1/, "");
    seen.push(`${init.method ?? "GET"} ${path}`);
    if (path === "/named-snapshots") {
      const named = (name: string) => ({ name, status: "ready", sourceSandboxId: "bx_acme", createdAt: "2026-10-01T00:00:00.000Z", snapshotId: "dc8cd70e-d16b-472a-b5d7-070bc84db7a7", type: "small", sizeBytes: 2_000_000_000 });
      return json({ ok: true, snapshots: [named("wsp-otherhost-default-v1"), named("wsp-otherhost-default-v2")] });
    }
    if (path === "/snapshots") {
      const page = Number(url.searchParams.get("cursor") ?? "0");
      const next = page + 1 < HISTORY_PAGES ? String(page + 1) : null;
      return json({ ok: true, snapshots: [{ id: `866f04de-8e8a-4401-9e29-cf58e17382${String(page).padStart(2, "0")}`, sandboxId: "bx_acme", status: "completed", createdAt: "2026-10-01T00:00:00.000Z", sizeBytes: 50_000_000_000 }], pageInfo: { nextCursor: next, hasMore: next !== null, limit: 200 } });
    }
    if (path === "/sandboxes") return json({ ok: true, sandboxes: [], pageInfo: { nextCursor: null, hasMore: false, limit: 200 } });
    return json({ ok: false, error: { code: "not_found", message: "not found" } }, 404);
  }) as typeof globalThis.fetch;
  return { fetch, seen };
}

const history = (seen: readonly string[]): string[] => seen.filter(s => s === "GET /snapshots");

describe("a host on Boat reads no snapshot history", () => {
  const dirs: string[] = [];
  let host: HostHandle | undefined;
  let rt: Runtime | undefined;
  afterEach(async () => {
    await host?.close();
    host = undefined;
    await rt?.close().catch(() => {});
    rt = undefined;
    for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true });
  });

  it("starts with a storage line counting the named snapshots, and no request for Boat's history", async () => {
    const api = boat();
    const dir = mkdtempSync(join(tmpdir(), "wsp-boat-listing-"));
    dirs.push(dir);
    const web = join(dir, "web");
    mkdirSync(join(web, "assets"), { recursive: true });
    writeFileSync(join(web, "index.html"), `<html><body><script>window.__WSP__ = window.__WSP__ || { token: "" };</script></body></html>`);
    const lines: string[] = [];
    rt = createRuntime({ backend: new BoxBackend({ apiKey: "box_x", fetch: api.fetch }), store: memoryStore(), adapters: {} });
    host = await startHost({ runtime: rt, port: 0, webDir: web, statePath: join(dir, "state.json"), provider: "box", log: line => lines.push(line) });
    await vi.waitUntil(() => lines.some(l => l.startsWith("storage:")), { timeout: 5_000, interval: 20 });
    expect(history(api.seen)).toEqual([]);
    expect(api.seen).toContain("GET /named-snapshots");
    expect(lines.find(l => l.startsWith("storage:"))).toBe("storage: 2 named snapshots, 4.0 GB; inside the 10 free, nothing to pay. 2 not this host's, 4.0 GB");
  });

  it("runs the doctor's storage step off the named snapshots, and asks Boat for no history", async () => {
    const api = boat();
    rt = createRuntime({ backend: new BoxBackend({ apiKey: "box_x", fetch: api.fetch }), store: memoryStore(), adapters: {} });
    const lines: string[] = [];
    const io: CliIO = { log: line => lines.push(line), error: line => lines.push(line), ask: q => Promise.reject(new Error(q)), askSecret: q => Promise.reject(new Error(q)) };
    // With no image and no key to build one the run stops at the image, after the storage step.
    expect(await forkDoctor(rt, io, {})).toBe(1);
    expect(history(api.seen)).toEqual([]);
    expect(lines).toContain("storage: 2 named snapshots, 4.0 GB; inside the 10 free, nothing to pay. 2 not this host's, 4.0 GB");
  });
});
