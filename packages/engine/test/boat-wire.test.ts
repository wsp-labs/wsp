// SPDX-License-Identifier: AGPL-3.0-only
// The Boat canary's two failure readings, proved against wsp's own backend with no key: a refused body is quoted
// with Boat's answer and the body sent, and an answer missing a field wsp reads is named by that field.

import { describe, expect, it } from "vitest";
import { BOX_API_URL, BoxBackend } from "../src/box-backend.js";
import { boatSaid, checkReads, missingField, recordingFetch, shown, type Exchange } from "./boat-wire.js";

const json = (status: number, body: unknown): Response => new Response(JSON.stringify(body), { status });
const noWait = { now: () => 0, sleep: async () => {} };

function backendAnswering(answer: (path: string, init?: RequestInit) => Response): { backend: BoxBackend; wire: Exchange[] } {
  const wire: Exchange[] = [];
  const fetch = recordingFetch(wire, BOX_API_URL, async (input, init) => answer(String(input).slice(BOX_API_URL.length), init));
  return { backend: new BoxBackend({ apiKey: "k", fetch, clock: noWait }), wire };
}

const exchange = (method: string, path: string, status: number | null, body: unknown, sent?: unknown): Exchange => ({
  method,
  path,
  sent: sent === undefined ? undefined : JSON.stringify(sent),
  status,
  text: typeof body === "string" ? body : JSON.stringify(body),
  at: 0,
  ms: 0,
});

describe("the Boat canary's wire", () => {
  it("quotes a refused create with Boat's answer and the body wsp sent", async () => {
    const { backend, wire } = backendAnswering(path =>
      path === "/limits"
        ? json(200, { accessTier: "standard" })
        : json(400, { ok: false, code: "invalid_request", message: "noEnv is not allowed", requestId: "req_1" }),
    );
    await expect(backend.create({ kind: "sandbox", fromSnapshot: "wsp-acme-default-v4", cpu: 2, memMb: 4096, labels: { "wsp-owner": "h_canary" }, idleTimeoutMs: 60_000, idempotencyKey: "key-1" })).rejects.toThrow("noEnv is not allowed");
    const said = boatSaid(wire);
    expect(said).toContain('POST /sandboxes answered 400 {"ok":false,"code":"invalid_request","message":"noEnv is not allowed","requestId":"req_1"}');
    expect(said).toContain('body sent: {"type":"small","ttlSeconds":60,"noEnv":true,"name":"wsp;o=h_canary","from":"wsp-acme-default-v4"}');
  });

  it("hands the answer on unchanged and records a road that never answered", async () => {
    const { backend, wire } = backendAnswering(() => {
      throw Object.assign(new TypeError("fetch failed"), { cause: { code: "ECONNRESET" } });
    });
    await expect(backend.request("GET", "/limits", { once: true })).rejects.toThrow("fetch failed");
    expect(boatSaid(wire)).toBe("GET /limits failed before an answer (fetch failed)");

    const ok = backendAnswering(() => json(200, { accessTier: "trial" }));
    expect(await ok.backend.request("GET", "/limits")).toEqual({ accessTier: "trial" });
    expect(boatSaid(ok.wire)).toBeUndefined();

    const empty = backendAnswering(() => new Response(null, { status: 204 }));
    expect(await empty.backend.request("PATCH", "/sandboxes/bx_1", { body: { name: "n" } })).toEqual({});
  });

  it("names a field wsp reads that Boat's answer lost", () => {
    const box = exchange("GET", "/sandboxes/bx_23456789", 200, { sandbox: { id: "bx_23456789", status: "ready" } });
    expect(missingField(box)).toBe("sandbox.state");
    expect(() => checkReads([box])).toThrow('Boat\'s answer lost sandbox.state, which wsp reads: GET /sandboxes/bx_23456789 answered 200 {"sandbox":{"id":"bx_23456789","status":"ready"}}');
    expect(missingField(exchange("GET", "/sandboxes/bx_23456789", 200, { sandbox: { id: "bx_23456789", state: "ready", name: null } }))).toBeUndefined();
  });

  it("reads a field in a list", () => {
    const listing = exchange("GET", "/sandboxes?limit=200", 200, { sandboxes: [{ id: "bx_1", name: "wsp;o=h", state: "ready" }, { id: "bx_2", name: null, status: "ready" }] });
    expect(missingField(listing)).toBe("sandboxes[].state");
  });

  it("leaves a refusal to the refusal line and reads nothing off it", () => {
    expect(missingField(exchange("GET", "/sandboxes/bx_23456789", 404, { code: "not_found" }))).toBeUndefined();
  });

  it("prints no file bytes and no route token", () => {
    const put = exchange("PUT", "/sandboxes/bx_1/files", 400, { code: "sandbox_direct_failed" }, { path: "/tmp/x", content: "c2VjcmV0", encoding: "base64" });
    expect(shown(put)).toContain('"content":"<8 chars>"');
    expect(shown(put)).not.toContain("c2VjcmV0");
    const host = exchange("POST", "/sandboxes/bx_1/host", 200, { url: "https://bx-1.example.dev/?_token=abc" }, { port: 7070, public: true });
    expect(shown(host)).toContain('"url":"https://bx-1.example.dev/?<token>"');
    expect(shown(host)).not.toContain("abc");
  });
});
