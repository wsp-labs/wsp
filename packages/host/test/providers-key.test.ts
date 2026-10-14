// SPDX-License-Identifier: AGPL-3.0-only
// The key step's live check against a stand-in for Boat's API, answering in the shapes Boat answered on 2026-10-13:
// a wrong key 401 unauthorized everywhere, a key without sandbox.read 403 api_key_action_forbidden on the list alone.
import { afterEach, describe, expect, it, vi } from "vitest";
import { KEY_REFUSED, KEY_UNCHECKED } from "@wsp/protocol";
import { checkProviderKeyLive, providerKeyCheck, providerTable } from "../src/providers.js";

const OFF = providerTable(false);

type Route = (path: string) => { status: number; body: unknown };

const json = (status: number, body: unknown) => ({ status, body });
const refused = (status: number, code: string, message: string, action?: string) =>
  json(status, { ok: false, type: "sandbox.error", status, code, message, error: { code, message, status, details: { error: code, message, ...(action === undefined ? {} : { action }) } } });

const LIMITS = { accessTier: "standard", maxActiveBoxes: 100, activeBoxes: 2, boxPlanKey: "box_20", boxPlanDollars: 20 };

/** A key Boat takes with every action wsp reads. */
const works: Route = path => (path === "/me" ? json(200, { ok: true, user: { login: "acme" } }) : path === "/limits" ? json(200, LIMITS) : json(200, { ok: true, boxes: [] }));

/** Boat as the route says, every path asked kept. */
function boat(route: Route): string[] {
  const asked: string[] = [];
  vi.stubGlobal("fetch", async (url: string) => {
    const at = new URL(url);
    const path = at.pathname.replace(/^\/api\/box\/v1/, "");
    asked.push(`${path}${at.search}`);
    const { status, body } = route(path);
    return new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });
  });
  return asked;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

const refusal = (p: Promise<unknown>) => p.then(() => ({ kind: "none", said: "" }), (e: { kind?: string; message: string }) => ({ kind: e.kind, said: e.message }));

describe("the key step's check of a Boat key", () => {
  it("answers the account, the plan and the machines running of the most it may for a key that works", async () => {
    const asked = boat(works);
    expect(await checkProviderKeyLive({}, "boat_good", "box", OFF)).toEqual({ works: true, account: "acme", plan: "$20 plan", running: 2, limit: 100 });
    expect(asked.sort()).toEqual(["/api-keys", "/boxes/bx_wspkeycheck0000000000/resume", "/boxes?limit=1", "/limits", "/me"]);
  });

  it("refuses a wrong key in plain words", async () => {
    boat(() => refused(401, "unauthorized", "unauthorized"));
    expect(await refusal(checkProviderKeyLive({}, "boat_wrong", "box", OFF))).toEqual({ kind: KEY_REFUSED, said: "Boat refused this key. Check you copied all of it." });
  });

  it("refuses a key without sandbox.read, which Boat answers on the list alone", async () => {
    boat(path => (path.startsWith("/boxes") ? refused(403, "api_key_action_forbidden", "This API key cannot perform sandbox.read.", "sandbox.read") : works(path)));
    expect(await refusal(checkProviderKeyLive({}, "boat_no_read", "box", OFF))).toEqual({ kind: KEY_REFUSED, said: "This key cannot list machines. Give it sandbox.read and the other actions above." });
  });

  it("names the action as the dashboard does where the API base spells it box.read", async () => {
    boat(path => (path.startsWith("/boxes") ? refused(403, "api_key_action_forbidden", "This API key cannot perform box.read.", "box.read") : works(path)));
    expect(await refusal(checkProviderKeyLive({}, "boat_no_read", "box", OFF))).toEqual({ kind: KEY_REFUSED, said: "This key cannot list machines. Give it sandbox.read and the other actions above." });
  });

  it("names account.read where the key cannot read the account", async () => {
    boat(path => (path === "/boxes" ? works(path) : refused(403, "api_key_action_forbidden", "This API key cannot perform account.read.")));
    expect(await refusal(checkProviderKeyLive({}, "boat_no_account", "box", OFF))).toEqual({ kind: KEY_REFUSED, said: "This key cannot read the account. Give it account.read and the other actions above." });
  });

  it("refuses a ci key made before resume and delete-own joined the preset, off a resume of a box no account holds", async () => {
    const older = "This key was made before Boat added resume and delete-own to the ci preset, so make a new ci key or use an admin key.";
    boat(path => (path.endsWith("/resume") ? refused(403, "api_key_action_forbidden", "This API key cannot perform box.resume.", "box.resume") : works(path)));
    expect(await refusal(checkProviderKeyLive({}, "boat_old_ci", "box", OFF))).toEqual({ kind: KEY_REFUSED, said: older });
    // With resume, the probe answers 404 for the box, and only the key's own row can say delete-own is missing.
    const row = (actions: readonly string[]) => (path: string) =>
      path === "/api-keys" ? json(200, { ok: true, apiKeys: [{ keyPrefix: "boat_ab12", keyLastFour: "wxyz", scope: { actions } }] }) : path.startsWith("/boxes/bx_") ? json(404, { ok: false, code: "not_found" }) : works(path);
    boat(row(["box.create", "box.read", "box.resume", "account.read"]));
    expect(await refusal(checkProviderKeyLive({}, "boat_ab12_middle_wxyz", "box", OFF))).toEqual({ kind: KEY_REFUSED, said: older });
    boat(row(["box.create", "box.read", "box.resume", "box.delete-own", "account.read"]));
    expect(await checkProviderKeyLive({}, "boat_ab12_middle_wxyz", "box", OFF)).toMatchObject({ works: true });
    boat(row(["*"]));
    expect(await checkProviderKeyLive({}, "boat_ab12_middle_wxyz", "box", OFF)).toMatchObject({ works: true });
    // A row that is another key's says nothing about this one.
    boat(row(["account.read"]));
    expect(await checkProviderKeyLive({}, "boat_other_key_0000", "box", OFF)).toMatchObject({ works: true });
  });

  it("passes a key holding exactly the actions the key step lists, which leaves out SSH since wsp never uses it", async () => {
    const LISTED = ["sandbox.create", "sandbox.read", "sandbox.fork", "sandbox.resume", "sandbox.stop", "sandbox.delete-own", "sandbox.update", "exec", "file.read", "file.write", "host", "snapshot.read", "snapshot.write", "account.read"];
    const asked = boat(path =>
      path === "/api-keys" ? json(200, { ok: true, apiKeys: [{ keyPrefix: "boat_ab12", keyLastFour: "wxyz", scope: { actions: LISTED.map(a => a.replace(/^sandbox\./, "box.")) } }] }) : path.startsWith("/boxes/bx_") ? json(404, { ok: false, code: "not_found" }) : works(path),
    );
    expect(await checkProviderKeyLive({}, "boat_ab12_middle_wxyz", "box", OFF)).toMatchObject({ works: true });
    expect(asked.some(path => path.includes("sshkey"))).toBe(false);
  });

  it("says an expired key is expired", async () => {
    boat(() => refused(403, "api_key_expired", "API key expired."));
    expect(await refusal(checkProviderKeyLive({}, "boat_old", "box", OFF))).toEqual({ kind: KEY_REFUSED, said: "This key has expired. Make a new one on Boat's API keys page." });
  });

  it("says Boat could not be reached, never that the key is wrong, when nothing answered about it", async () => {
    vi.stubGlobal("fetch", async () => {
      throw new TypeError("fetch failed");
    });
    expect(await refusal(checkProviderKeyLive({}, "boat_any", "box", OFF))).toEqual({ kind: KEY_UNCHECKED, said: "wsp cannot reach Boat right now. Try again." });
    boat(() => json(503, { ok: false, code: "unavailable", message: "try later" }));
    expect(await refusal(checkProviderKeyLive({}, "boat_any", "box", OFF))).toEqual({ kind: KEY_UNCHECKED, said: "wsp cannot reach Boat right now. Try again." });
  });
});

describe("saving a Boat key runs the same check", () => {
  it("refuses a key without sandbox.read before it is saved, with Boat's own word", async () => {
    boat(path => (path.startsWith("/boxes") ? refused(403, "api_key_action_forbidden", "This API key cannot perform sandbox.read.", "sandbox.read") : works(path)));
    expect(await providerKeyCheck({}, "boat_no_read", "box", OFF)).toEqual({ state: "refused", said: "403 This API key cannot perform sandbox.read." });
  });

  it("keeps the limits a key that works read, for the line wsp add prints", async () => {
    boat(works);
    const check = await providerKeyCheck({}, "boat_good", "box", OFF);
    expect(check).toMatchObject({ state: "taken", answer: { planUsd: 20, machinesAtOnce: 100, activeBoxes: 2 } });
  });
});
