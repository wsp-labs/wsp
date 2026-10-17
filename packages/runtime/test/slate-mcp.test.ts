// SPDX-License-Identifier: AGPL-3.0-only
// A slate's tool and resource runs against a stub MCP server over stdio, through the host end to end: a thread whose
// agent asks before a tool asks once per piece of code and one at bypass never, a timer's tool run fills its json, a
// secret reaches the call and comes back scrubbed, one connection serves every call, and slate_catalog answers its tools.
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync } from "node:fs";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it, vi } from "vitest";
import { LocalBackend } from "@wsp/engine";
import type { AdapterEvent, Caller, EventUnion, SlateView, TurnResult } from "@wsp/protocol";
import { createRuntime, type AgentsReader, type HarnessAdapterFactory, type LocalWiring, type Runtime } from "../src/index.js";
import { HELD_PRESSED, STARTS_PER_MINUTE } from "../src/slate-runs.js";
import { localExecStream } from "../src/local-exec.js";
import { memoryStore } from "../src/store.js";
import { codeKey, createSlateMcp, foldSchema, slateSecretMark, type McpServerSpec, type ToolAsks } from "../src/slate-mcp.js";
import type { RunRecord } from "../src/slate-runs.js";
import { stubBackend, testPlatform } from "./stub-backend.js";

const STUB = fileURLToPath(new URL("./fixtures/stub-mcp.mjs", import.meta.url));
const SERVERS: Record<string, string> = { stub: STUB, notes: fileURLToPath(new URL("./fixtures/stub-text-mcp.mjs", import.meta.url)) };
const SHAPE = `"${process.execPath}" "${fileURLToPath(new URL("./fixtures/lines-to-json.mjs", import.meta.url))}"`;
const KEY = "sk_live_51HxQ2mZ9vT3pLk";

const roots: string[] = [];
const runtimes: Runtime[] = [];
afterEach(async () => {
  for (const rt of runtimes.splice(0)) await rt.close();
  for (const at of roots.splice(0)) rmSync(at, { recursive: true, force: true });
});

function harness(): HarnessAdapterFactory {
  return () => ({
    steers: false,
    forkSession: async f => ({ resume: f.session, drop: async () => {} }),
    start: o => {
      const sessionId = o.resume ?? "66666666-6666-4666-8666-000000000001";
      const result: TurnResult = { status: "completed", text: "ok" };
      const finished = Promise.resolve().then(() => {
        const feed: AdapterEvent[] = [
          { type: "session.start", sessionId },
          { type: "turn.done", sessionId, result },
          { type: "session.end", sessionId, exitCode: 0, sawResult: true },
        ];
        for (const e of feed) o.onEvent(e);
        return result;
      });
      return { localId: sessionId, finished, interrupt: async () => {} };
    },
  });
}

const SLATE = `<slate title="Stub inbox">
  <secret name="key" />
  <value name="id" start={7} />
  <run name="inbox" tool="stub.list_items" args={{ params: { account: "admin", limit: 3 }, key: $key }} every={10} />
  <run name="remove" tool="stub.delete_item" args={{ id: $id }} description="Deletes the item you picked" />
  <run name="status" resource="stub:stub://status" />
  <column>
    <input id="k" label="Key" value={$key} kind="password" />
    <table id="items" items={$inbox.json.items} key={item.id}><col title="Subject" value={item.subject} /></table>
    <button id="refresh" label="Refresh" onPress={start($inbox)} />
    <button id="del" label="Delete" onPress={start($remove)} />
    <button id="stat" label="Status" onPress={start($status)} />
  </column>
</slate>`;

/** A host with a project, a thread on it at the access mode given (none runs at the agent's bypass), and the stub
 * servers in its agent's config. */
async function host(permissionMode: string | undefined = "default") {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "wsp-slate-mcp-")));
  roots.push(root);
  const folder = join(root, "project");
  mkdirSync(folder);
  const starts = join(root, "starts");
  const events: EventUnion[] = [];
  const asked: { agent: string; name: string; path: string | undefined }[] = [];
  const agentsReader: AgentsReader = {
    read: () => Promise.reject(new Error("not here")),
    tools: () => Promise.reject(new Error("not here")),
    server: async (on, ask) => {
      asked.push({ ...ask, path: on.projects?.[0]?.path });
      const script = SERVERS[ask.name];
      if (script === undefined) throw new Error(`no MCP server called ${ask.name} is in Claude Code's config there`);
      return { transport: { kind: "stdio", command: process.execPath, args: [script], env: { STUB_STARTS: starts } }, cwd: folder, env: { PATH: process.env["PATH"] ?? "/usr/bin:/bin" }, secrets: [] };
    },
  };
  const local: LocalWiring = {
    backend: new LocalBackend({ root }),
    execStream: o => localExecStream({ root, runDir: join(root, "runs"), ...o }),
    home: () => join(root, ".claude"),
    homeDir: root,
    rootsPath: join(root, "roots"),
    env: () => ({ PATH: process.env["PATH"] ?? "/usr/bin:/bin", HOME: root }),
    platform: testPlatform(),
    daemonRoad: async () => ({ url: "http://127.0.0.1:1", expiresAt: Number.MAX_SAFE_INTEGER, daemonToken: "t" }),
  };
  mkdirSync(join(root, "state"));
  const store = memoryStore();
  const boot = (): Runtime => {
    const made = createRuntime({ backend: stubBackend(), store, adapters: { claude: harness() }, local, statePath: join(root, "state", "state.json"), agentsReader });
    made.events.on("*", e => events.push(e as EventUnion));
    runtimes.push(made);
    return made;
  };
  const rt = boot();
  const project = await rt.projects.add({ source: folder });
  const { workspace } = await rt.workspaces.folderFor({ project: project.id });
  const first = await rt.sessions.start(workspace.id, { prompt: "show my inbox in a slate", ...(permissionMode !== undefined ? { permissionMode } : {}) });
  await first.finished;
  const threadId = first.view().threadId!;
  const asThread: Caller = { origin: "here", by: { kind: "thread", threadId, workspaceId: workspace.id, rootThreadId: threadId } };
  return { root, folder, starts, events, asked, boot, rt, threadId, asThread };
}

describe("a slate's MCP runs", () => {
  it("folds a tool's input schema to one line per property", () => {
    expect(foldSchema({ properties: { params: { type: "object", required: ["account"], properties: { account: { type: "string", description: "The alias. More words." }, limit: { type: "integer", maximum: 50 } } }, tags: { type: "array", items: { type: "string" } }, mode: { enum: ["a", "b"] } }, required: ["params"] })).toEqual([
      "  params: object, required",
      "  params.account: string, required. The alias.",
      "  params.limit: integer, max 50",
      "  tags: string[]",
      '  mode: "a" | "b"',
    ]);
    // FastMCP puts a model argument behind a $ref, and an optional field in an anyOf with null.
    expect(foldSchema({ $defs: { In: { type: "object", required: ["id"], properties: { id: { type: "string" }, folder: { anyOf: [{ type: "string" }, { type: "null" }], default: null, description: "Folder name, e.g. 'Inbox'." } } } }, properties: { params: { $ref: "#/$defs/In" } }, required: ["params"] })).toEqual([
      "  params: object, required",
      "  params.id: string, required",
      "  params.folder: string | null, default null. Folder name, e.g. 'Inbox'.",
    ]);
  });

  it("closes a server's connection once the slate is hidden and idle, unless an always timer keeps it", async () => {
    const root = realpathSync(mkdtempSync(join(tmpdir(), "wsp-slate-mcp-")));
    roots.push(root);
    const starts = join(root, "starts");
    const records: Record<string, RunRecord> = {};
    const mcp = createSlateMcp({
      server: async () => ({ transport: { kind: "stdio", command: process.execPath, args: [STUB], env: { STUB_STARTS: starts } }, cwd: root, env: { PATH: process.env["PATH"] ?? "/usr/bin:/bin" }, secrets: [] }),
      onRecord: (_t, run, r) => (records[run] = r),
      approvals: { has: () => true, allow: () => {}, revoke: () => {}, list: () => [] },
      secrets: { plaintext: () => undefined, scrub: (_t, text) => text },
      reshape: () => {
        throw new Error("no run here reshapes");
      },
      computer: () => "this computer",
      asks: () => "every",
      idleMs: 300,
    });
    const call = async (n: number): Promise<void> => {
      mcp.start({ threadId: "t", run: "inbox", decl: { kind: "tool", server: "stub", tool: "list_items" }, by: "person", args: () => ({ params: { account: "a", limit: 1 } }) });
      await vi.waitFor(() => expect(records["inbox"]).toMatchObject({ state: "done", runs: n }), { timeout: 10_000 });
    };
    const spawned = (): number => readFileSync(starts, "utf8").trim().split("\n").length;
    mcp.servers("t", ["stub"], []);
    mcp.shown("t", true);
    await call(1);
    await new Promise(r => setTimeout(r, 700));
    await call(2);
    expect(spawned()).toBe(1);
    mcp.shown("t", false);
    await new Promise(r => setTimeout(r, 900));
    await call(3);
    expect(spawned()).toBe(2);
    mcp.servers("t", ["stub"], ["stub"]);
    mcp.shown("t", false);
    await new Promise(r => setTimeout(r, 900));
    await call(4);
    expect(spawned()).toBe(2);
    mcp.close();
  }, 30_000);

  it("names a stdio server's process as the thread's tool server while connected, and releases it once closed", async () => {
    const root = realpathSync(mkdtempSync(join(tmpdir(), "wsp-slate-mcp-")));
    roots.push(root);
    const records: Record<string, RunRecord> = {};
    const owned: { threadId: string; pid: number; kind: string; released: boolean }[] = [];
    const mcp = createSlateMcp({
      server: async () => ({ transport: { kind: "stdio", command: process.execPath, args: [STUB], env: { STUB_STARTS: join(root, "starts") } }, cwd: root, env: { PATH: process.env["PATH"] ?? "/usr/bin:/bin" }, secrets: [] }),
      onRecord: (_t, run, r) => (records[run] = r),
      approvals: { has: () => true, allow: () => {}, revoke: () => {}, list: () => [] },
      secrets: { plaintext: () => undefined, scrub: (_t, text) => text },
      reshape: () => {
        throw new Error("no run here reshapes");
      },
      computer: () => "this computer",
      asks: () => "every",
      own: (threadId, pid, kind) => {
        const row = { threadId, pid, kind, released: false };
        owned.push(row);
        return () => void (row.released = true);
      },
      idleMs: 200,
    });
    mcp.servers("t", ["stub"], []);
    mcp.start({ threadId: "t", run: "inbox", decl: { kind: "tool", server: "stub", tool: "list_items" }, by: "person", args: () => ({ params: { account: "a", limit: 1 } }) });
    await vi.waitFor(() => expect(records["inbox"]).toMatchObject({ state: "done" }), { timeout: 10_000 });
    expect(owned.map(o => [o.threadId, o.kind, o.released])).toEqual([["t", "tool", false]]);
    expect(() => process.kill(owned[0]!.pid, 0)).not.toThrow();
    await vi.waitFor(() => expect(owned[0]!.released).toBe(true), { timeout: 10_000 });
    mcp.close();
  }, 30_000);

  it("cuts a server off past 2 MB in one message, and fails an answer too deep or too wide before walking it", async () => {
    const big = createServer((req, res) => {
      let body = "";
      req.on("data", c => (body += c));
      req.on("end", () => {
        const msg = JSON.parse(body) as { id?: number; method: string };
        if (msg.id === undefined) return void res.writeHead(202).end();
        res.writeHead(200, { "Content-Type": "application/json" });
        if (msg.method === "initialize") return void res.end(JSON.stringify({ jsonrpc: "2.0", id: msg.id, result: { protocolVersion: "2025-06-18", capabilities: { tools: {} }, serverInfo: { name: "big", version: "1" } } }));
        if (msg.method === "tools/list") return void res.end(JSON.stringify({ jsonrpc: "2.0", id: msg.id, result: { tools: [{ name: "huge", annotations: { readOnlyHint: true }, inputSchema: { type: "object" } }] } }));
        res.end(`{"jsonrpc":"2.0","id":${msg.id},"result":{"content":[{"type":"text","text":"${"x".repeat(3 * 1024 * 1024)}"}]}}`);
      });
    });
    await new Promise<void>(r => big.listen(0, "127.0.0.1", r));
    const records: Record<string, RunRecord> = {};
    const mcp = createSlateMcp({
      server: async (_t, name): Promise<McpServerSpec> =>
        name === "big"
          ? { transport: { kind: "http", url: `http://127.0.0.1:${(big.address() as AddressInfo).port}/mcp`, headers: {} }, cwd: tmpdir(), env: {}, secrets: [] }
          : { transport: { kind: "stdio", command: process.execPath, args: [fileURLToPath(new URL("./fixtures/stub-flood-mcp.mjs", import.meta.url))], env: {} }, cwd: tmpdir(), env: { PATH: process.env["PATH"] ?? "/usr/bin:/bin" }, secrets: [] },
      onRecord: (_t, run, r) => (records[run] = r),
      approvals: { has: () => true, allow: () => {}, revoke: () => {}, list: () => [] },
      secrets: { plaintext: () => undefined, scrub: (_t, text) => text },
      reshape: () => {
        throw new Error("no run here reshapes");
      },
      computer: () => "this computer",
      asks: () => "every",
    });
    try {
      mcp.servers("t", ["flood", "big"], []);
      const ended = async (run: string, server: string, tool: string): Promise<RunRecord> => {
        mcp.start({ threadId: "t", run, decl: { kind: "tool", server, tool }, by: "person", args: () => ({}) });
        await vi.waitFor(() => expect(["done", "failed"]).toContain(records[run]?.state), { timeout: 10_000 });
        return records[run]!;
      };
      expect(await ended("deep", "flood", "deep")).toMatchObject({ state: "failed", why: expect.stringContaining("nested over 64 deep") });
      expect(await ended("wide", "flood", "wide")).toMatchObject({ state: "failed", why: expect.stringContaining("over 100000 entries") });
      // Deep JSON inside a text is kept as the text it is, never walked as data.
      const text = await ended("deeptext", "flood", "deeptext");
      expect(text).toMatchObject({ state: "done", text: true });
      expect(text).not.toHaveProperty("json");
      expect(await ended("endless", "flood", "endless")).toMatchObject({ state: "failed", why: expect.stringContaining("over 2 MB") });
      expect(await ended("huge", "big", "huge")).toMatchObject({ state: "failed", why: expect.stringContaining("over 2 MB") });
    } finally {
      mcp.close();
      big.close();
    }
  }, 30_000);

  it("on an Ask thread asks once per piece of code: the same code refreshes with no sheet, new arguments ask again", async () => {
    const { folder, starts, events, asked, boot, rt: booted, threadId, asThread } = await host("default");
    let rt = booted;
    const get = async (): Promise<SlateView> => {
      await rt.slates.settled();
      return (await rt.slates.get(threadId))!;
    };
    const press = async (piece: string) => rt.slates.event({ threadId, version: (await get()).version, piece, event: "press", requestId: `${piece}-${Math.random()}` });

    // The catalog answers the server's tools for this thread's agent, folded, with how this thread asks.
    const before = await rt.slates.catalog({ name: "stub" }, asThread);
    expect(before.text).toContain("stub: an MCP server of this thread's agent, 3 tools, 1 resource.");
    expect(before.text).toContain("This thread's agent asks before every tool, so the person allows each run's code once");
    expect(before.text).toContain('description="');
    expect(before.text).toContain("list_items [returns data, read-only]: Lists the newest items in an account.");
    expect(before.text).toContain("  params.account: string, required. The account's alias.");
    expect(before.text).toContain("  params.limit: integer, default 10, min 1, max 50");
    expect(before.text).toContain("delete_item [text only, destructive]: Deletes one item for good.");
    expect(before.text).toContain("touch_item [text only]: Marks one item as seen.");
    expect(before.text).not.toMatch(/every start|every time/);
    expect(before.text).toContain("resource stub://status (application/json): The stub's own status.");
    expect(asked[0]).toEqual({ agent: "claude", name: "stub", path: folder });
    // A name that is neither in the kit nor a server says both.
    const nothing = await rt.slates.catalog({ name: "zoho" }, asThread);
    expect(nothing.text).toMatch(/zoho is not in the catalog[\s\S]*no MCP server called zoho/);
    // The kit still answers its own names.
    expect((await rt.slates.catalog({ name: "button" }, asThread)).text).not.toContain("MCP server");

    const wrote = await rt.slates.write({ text: SLATE }, asThread);
    expect(wrote.version).toBe(1);
    await rt.slates.state({ threadId, values: { $key: KEY } });

    // Shown: the timer fires at once and its run waits for the person, on one plain ask naming the tool and its code.
    const release = rt.slates.subscribe({ threadId, sources: [] });
    let view: SlateView;
    await vi.waitFor(async () => {
      view = await get();
      expect(view.values["inbox"]).toMatchObject({ state: "held", why: "needs your approval" });
      expect(view.asks.find(a => a.run === "inbox")).toMatchObject({ kind: "tool", server: "stub", tool: "list_items", description: "Lists the newest items in an account." });
    }, { timeout: 10_000 });
    const first = view!.asks.find(a => a.run === "inbox")!;
    expect(first.key).toMatch(/^mcp:stub#[0-9a-f]{32}$/);
    expect(first.kind === "tool" && first.args).toEqual({ params: { account: "admin", limit: 3 }, key: `•••• (${KEY.length})` });
    expect(Object.keys(first)).not.toContain("tools");

    // Allowed: the held timer run calls the tool, and its json fills.
    await rt.slates.approve({ threadId, key: first.key, scope: "thread" });
    await vi.waitFor(async () => {
      view = await get();
      expect(view.values["inbox"]).toMatchObject({ state: "done", runs: 1, json: { account: "admin", items: [{ id: 1, subject: "item 1 for admin" }, { id: 2 }, { id: 3 }] } });
    }, { timeout: 10_000 });
    // The secret reached the call and came back out of every road scrubbed.
    expect(view!.values["inbox"]).toMatchObject({ out: "listed 3 with key [secret:key]", json: { key: "[secret:key]" } });

    // A second tick of the timer runs the same code with no sheet.
    await vi.waitFor(async () => {
      view = await get();
      expect(view.values["inbox"]).toMatchObject({ state: "done", runs: 2, json: { call: 2 } });
    }, { timeout: 15_000, interval: 250 });
    expect(view!.asks).toEqual([]);

    // A tool that changes things asks once too, with the agent's line for what it does.
    const del = await press("del");
    expect(del.outcome).toBe("held");
    expect(del.ask).toMatchObject({ kind: "tool", run: "remove", server: "stub", tool: "delete_item", description: "Deletes the item you picked", args: { id: 7 } });
    await rt.slates.approve({ threadId, key: del.ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await get()).values["remove"]).toMatchObject({ state: "done", out: "deleted 7" }), { timeout: 10_000 });
    // The key is what is sent: a new id asks again, and an id allowed before runs with no sheet.
    await rt.slates.state({ threadId, values: { $id: 8 } });
    const eight = await press("del");
    expect(eight.ask).toMatchObject({ kind: "tool", run: "remove", args: { id: 8 } });
    expect(eight.ask!.key).not.toBe(del.ask!.key);
    await rt.slates.approve({ threadId, key: eight.ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await get()).values["remove"]).toMatchObject({ state: "done", out: "deleted 8", runs: 2 }), { timeout: 10_000 });
    await rt.slates.state({ threadId, values: { $id: 7 } });
    expect((await press("del")).ask).toBeUndefined();
    await vi.waitFor(async () => expect((await get()).values["remove"]).toMatchObject({ state: "done", out: "deleted 7", runs: 3 }), { timeout: 10_000 });
    await rt.slates.state({ threadId, values: { $id: 8 } });

    // A resource run is code of its own, asked once like a tool.
    const stat = await press("stat");
    expect(stat.ask).toMatchObject({ kind: "tool", server: "stub", tool: "stub://status", resource: true });
    await rt.slates.approve({ threadId, key: stat.ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await get()).values["status"]).toMatchObject({ state: "done", json: { ok: true } }), { timeout: 10_000 });

    // New arguments are new code and ask again; the code allowed before never asks twice.
    await rt.slates.write({ text: SLATE.replace("limit: 3", "limit: 5") }, asThread);
    await vi.waitFor(async () => {
      view = await get();
      expect(view.values["inbox"]).toMatchObject({ state: "held" });
      expect(view.asks.find(a => a.run === "inbox")).toMatchObject({ kind: "tool", args: { params: { account: "admin", limit: 5 } } });
    }, { timeout: 10_000 });
    expect(view!.asks.find(a => a.run === "inbox")!.key).not.toBe(first.key);
    await rt.slates.write({ text: SLATE }, asThread);
    await vi.waitFor(async () => {
      view = await get();
      expect(view.values["inbox"]).toMatchObject({ state: "done", json: { items: [{ id: 1 }, { id: 2 }, { id: 3 }] } });
    }, { timeout: 15_000, interval: 250 });
    expect(view!.asks).toEqual([]);

    // One connection served every call of the thread.
    expect(readFileSync(starts, "utf8").trim().split("\n")).toHaveLength(1);

    // A restart keeps what the person allowed: the code runs again with no sheet, and new code still asks, where Not
    // now ends that start.
    await rt.slates.settled();
    release();
    await rt.close();
    runtimes.splice(runtimes.indexOf(rt), 1);
    rt = boot();
    await rt.slates.ready();
    expect((await press("del")).ask).toBeUndefined();
    await vi.waitFor(async () => expect((await get()).values["remove"]).toMatchObject({ state: "done", out: "deleted 8", runs: 4 }), { timeout: 10_000 });
    await rt.slates.write({ text: SLATE.replace("args={{ id: $id }}", "args={{ id: $id, hard: true }}") }, asThread);
    const changed = await press("del");
    expect(changed.ask).toMatchObject({ kind: "tool", tool: "delete_item", args: { id: 8, hard: true } });
    await rt.slates.approve({ threadId, key: changed.ask!.key, scope: "refuse" });
    await vi.waitFor(async () => expect((await get()).values["remove"]).toMatchObject({ state: "cancelled", why: "you said not to run it" }), { timeout: 10_000 });

    // The agent's read, the pushes and the transcript hold no key.
    const read = await rt.slates.read({ values: ["$inbox.out", "$inbox.json.key"] }, asThread);
    expect(read.values).toEqual({ "$inbox.out": "listed 3 with key [secret:key]", "$inbox.json.key": "[secret:key]" });
    expect(JSON.stringify(read)).not.toContain(KEY);
    expect(JSON.stringify(events)).not.toContain(KEY);
  }, 90_000);

  it("on a Bypass thread starts a tool not marked read-only, and refreshes it, with no sheet", async () => {
    const { rt, threadId, asThread } = await host("bypassPermissions");
    const get = async (): Promise<SlateView> => {
      await rt.slates.settled();
      return (await rt.slates.get(threadId))!;
    };
    expect((await rt.slates.catalog({ name: "stub" }, asThread)).text).toContain("This thread's agent runs every tool without asking, and so do its slate's runs.");
    await rt.slates.write({ text: `<slate title="Touch"><run name="touch" tool="stub.touch_item" args={{ id: 1 }} every={10} description="Marks item 1 as seen" /><column><text id="t" value={$touch.out} /></column></slate>` }, asThread);
    const release = rt.slates.subscribe({ threadId, sources: [] });
    try {
      await vi.waitFor(async () => expect((await get()).values["touch"]).toMatchObject({ state: "done", runs: 1, out: "touched 1 on call 1" }), { timeout: 10_000 });
      expect((await get()).asks).toEqual([]);
      await vi.waitFor(async () => expect((await get()).values["touch"]).toMatchObject({ state: "done", runs: 2, out: "touched 1 on call 2" }), { timeout: 15_000, interval: 250 });
      const view = await get();
      expect(view.asks).toEqual([]);
      expect(Object.values(view.values).some(v => (v as { state?: string } | null)?.state === "held")).toBe(false);
    } finally {
      release();
    }
  }, 30_000);

  it("refuses a write whose new or changed run on a tool not marked read-only does not say what it does, by the server's own list, across a restart", async () => {
    const { rt: booted, boot, asThread } = await host("bypassPermissions");
    let rt = booted;
    const slate = (run: string) => `<slate title="Items">${run}<column><text id="t" value="items" /></column></slate>`;
    const list = `<run name="list" tool="stub.list_items" args={{ params: { account: "a" } }} />`;
    // No slate_catalog first: the server is asked whether the tool is read-only when the write needs to know.
    await expect(rt.slates.write({ text: slate(`<run name="del" tool="stub.delete_item" args={{ id: 1 }} />`) }, asThread)).rejects.toThrow(/K713 description-missing \$del calls stub\.delete_item, which its server does not mark read-only, so it needs description="\.\.\."/);
    // A tool that publishes no hints is held to the same.
    await expect(rt.slates.write({ text: slate(`<run name="touch" tool="stub.touch_item" args={{ id: 1 }} />`) }, asThread)).rejects.toThrow(/K713[\s\S]*\$touch[\s\S]*description=/);
    // A read-only tool and a resource need none; a described tool passes.
    expect((await rt.slates.write({ text: slate(`${list}<run name="stat" resource="stub:stub://status" /><run name="gone" tool="stub.delete_item" args={{ id: 1 }} description="Deletes item 1" />`) }, asThread)).version).toBe(1);

    // A restart empties every list held in memory, and a patch that leaves the runs as they were is never refused for them.
    await rt.slates.settled();
    await rt.close();
    runtimes.splice(runtimes.indexOf(rt), 1);
    rt = boot();
    await rt.slates.ready();
    expect((await rt.slates.write({ text: `<text id="t">two</text>` }, asThread)).version).toBe(2);
    // A new run is judged by the server's own list, read again now.
    await expect(rt.slates.write({ text: `<run name="del" tool="stub.delete_item" args={{ id: 2 }} />` }, asThread)).rejects.toThrow(/K713[\s\S]*\$del/);
    expect((await rt.slates.write({ text: `<run name="list2" tool="stub.list_items" args={{ params: { account: "b" } }} />` }, asThread)).version).toBe(3);
    // Taking the description off a run changes it.
    await expect(rt.slates.write({ text: `<run name="gone" tool="stub.delete_item" args={{ id: 1 }} />` }, asThread)).rejects.toThrow(/K713[\s\S]*\$gone/);
  }, 40_000);

  it("asks again when a derived the arguments read changes what is sent, on an Ask thread", async () => {
    const { rt, threadId, asThread } = await host("default");
    const get = async (): Promise<SlateView> => {
      await rt.slates.settled();
      return (await rt.slates.get(threadId))!;
    };
    const press = async () => rt.slates.event({ threadId, version: (await get()).version, piece: "go", event: "press", requestId: `go-${Math.random()}` });
    const slate = (account: string, limit: number) => `<slate title="Items">
  <derived name="who" value={{ account: '${account}', limit: ${limit} }} />
  <run name="items" tool="stub.list_items" args={{ params: $who }} />
  <column><button id="go" label="Go" onPress={start($items)} /></column>
</slate>`;
    await rt.slates.catalog({ name: "stub" }, asThread);
    await rt.slates.write({ text: slate("admin", 1) }, asThread);
    const first = await press();
    expect(first.ask).toMatchObject({ kind: "tool", args: { params: { account: "admin", limit: 1 } } });
    await rt.slates.approve({ threadId, key: first.ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await get()).values["items"]).toMatchObject({ state: "done", json: { account: "admin" } }), { timeout: 10_000 });
    await rt.slates.write({ text: slate("root", 9) }, asThread);
    const changed = await press();
    expect(changed.ask).toMatchObject({ kind: "tool", args: { params: { account: "root", limit: 9 } } });
    expect(changed.ask!.key).not.toBe(first.ask!.key);
    expect((await get()).values["items"]).not.toMatchObject({ json: { account: "root" } });
  }, 30_000);

  it("asks again when the agent sets a value the arguments read, before a reaction starts the run, on an Ask thread", async () => {
    const { rt, threadId, asThread } = await host("default");
    const get = async (): Promise<SlateView> => {
      await rt.slates.settled();
      return (await rt.slates.get(threadId))!;
    };
    await rt.slates.write({
      text: `<slate title="Wipe">
  <value name="target" start={1} />
  <run name="wipe" tool="stub.delete_item" args={{ id: $target }} description="Deletes the target item" />
  <when id="on-target" change={$target} do={start($wipe)} />
  <column><button id="go" label="Wipe" onPress={start($wipe)} /></column>
</slate>`,
    }, asThread);
    const first = await rt.slates.event({ threadId, version: (await get()).version, piece: "go", event: "press", requestId: "w-1" });
    await rt.slates.approve({ threadId, key: first.ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await get()).values["wipe"]).toMatchObject({ state: "done", out: "deleted 1" }), { timeout: 10_000 });
    // The agent's own call: the start its reaction makes waits for the person, with the new id on the sheet.
    await rt.slates.state({ values: { $target: 999 } }, asThread);
    await vi.waitFor(async () => {
      const view = await get();
      expect(view.values["wipe"]).toMatchObject({ state: "held", why: "needs your approval" });
      expect(view.asks).toMatchObject([{ kind: "tool", run: "wipe", args: { id: 999 } }]);
    }, { timeout: 10_000 });
    expect((await get()).values["wipe"]).not.toMatchObject({ out: "deleted 999" });
  }, 30_000);

  it("skips the ask for a read-only tool where the agent does, and asks on every start of a run that names confirm", async () => {
    const root = realpathSync(mkdtempSync(join(tmpdir(), "wsp-slate-mcp-")));
    roots.push(root);
    const records: Record<string, RunRecord> = {};
    const allowed = new Set<string>();
    let asks: ToolAsks = "unless-read-only";
    const mcp = createSlateMcp({
      server: async () => ({ transport: { kind: "stdio", command: process.execPath, args: [STUB], env: {} }, cwd: root, env: { PATH: process.env["PATH"] ?? "/usr/bin:/bin" }, secrets: [] }),
      onRecord: (_t, run, r) => (records[run] = r),
      approvals: { has: (_t, key) => allowed.has(key), allow: (_t, key) => void allowed.add(key), revoke: (_t, key) => void allowed.delete(key), list: () => [...allowed] },
      secrets: { plaintext: () => undefined, scrub: (_t, text) => text },
      reshape: () => {
        throw new Error("no run here reshapes");
      },
      computer: () => "this computer",
      asks: () => asks,
    });
    try {
      mcp.start({ threadId: "t", run: "list", decl: { kind: "tool", server: "stub", tool: "list_items" }, by: "person", args: () => ({ params: { account: "a", limit: 1 } }) });
      await vi.waitFor(() => expect(records["list"]).toMatchObject({ state: "done" }), { timeout: 10_000 });
      // A tool with no hints is one that is not read-only.
      mcp.start({ threadId: "t", run: "touch", decl: { kind: "tool", server: "stub", tool: "touch_item", description: "Marks item 1 as seen" }, by: "person", args: () => ({ id: 1 }) });
      await vi.waitFor(() => expect(mcp.held("t")).toMatchObject([{ kind: "tool", run: "touch", tool: "touch_item", description: "Marks item 1 as seen" }]), { timeout: 10_000 });
      expect(records["touch"]).toMatchObject({ state: "held", why: "needs your approval" });

      // At bypass nothing asks but a run that names confirm, which asks on every start and keeps nothing.
      asks = "never";
      const confirmed = { kind: "tool" as const, server: "stub", tool: "touch_item", confirm: "Mark it seen?" };
      for (const n of [1, 2]) {
        mcp.start({ threadId: "t", run: "mark", decl: confirmed, by: "person", args: () => ({ id: 2 }) });
        await vi.waitFor(() => expect(mcp.held("t").find(a => a.run === "mark")).toMatchObject({ kind: "tool", confirm: "Mark it seen?" }), { timeout: 10_000 });
        mcp.approve("t", mcp.held("t").find(a => a.run === "mark")!.key, "thread");
        await vi.waitFor(() => expect(records["mark"]).toMatchObject({ state: "done", runs: n }), { timeout: 10_000 });
      }
    } finally {
      mcp.close();
    }
  }, 30_000);

  it("keys what is sent: the server, the tool, the arguments as they will go, then and files, a secret by its name alone, never the run's words", () => {
    const decl = { kind: "tool" as const, server: "cf", tool: "execute", args: { code: "return 1", since: { bind: "$since" } } };
    const sent = { code: "return 1", since: "2026-10-13T00:00:00Z", token: slateSecretMark("cf_token") };
    const key = codeKey(decl, sent);
    expect(key).toMatch(/^mcp:cf#[0-9a-f]{32}$/);
    expect(codeKey({ ...decl, description: "Reads the traffic" }, sent)).toBe(key);
    expect(codeKey({ ...decl, every: 60 }, sent)).toBe(key);
    expect(codeKey(decl, { token: sent.token, since: sent.since, code: "return 1" })).toBe(key);
    // A value read through the same formula is still what goes, so it asks again.
    expect(codeKey(decl, { ...sent, since: "2026-10-13T01:00:00Z" })).not.toBe(key);
    expect(codeKey(decl, { ...sent, code: "return 2" })).not.toBe(key);
    expect(codeKey(decl, { ...sent, token: slateSecretMark("other_token") })).not.toBe(key);
    expect(codeKey({ ...decl, tool: "search" }, sent)).not.toBe(key);
    expect(codeKey({ ...decl, server: "cf2" }, sent)).not.toBe(key);
    expect(codeKey({ ...decl, then: "jq ." }, sent)).not.toBe(key);
    expect(codeKey({ ...decl, then: "sh x.sh", files: { "x.sh": "a" } }, sent)).not.toBe(codeKey({ ...decl, then: "sh x.sh", files: { "x.sh": "b" } }, sent));
    expect(codeKey({ kind: "resource", server: "cf", uri: "cf://a" }, {})).not.toBe(codeKey({ kind: "resource", server: "cf", uri: "cf://b" }, {}));
  });

  it("marks a text-only tool in the catalog, the record and the sketch, and a then reshapes its text into json", async () => {
    const { rt, threadId, asThread } = await host();
    const get = async (): Promise<SlateView> => {
      await rt.slates.settled();
      return (await rt.slates.get(threadId))!;
    };
    const press = async (piece: string) => rt.slates.event({ threadId, version: (await get()).version, piece, event: "press", requestId: `${piece}-${Date.now()}` });

    // No outputSchema and no call yet: both tools read text only. The structured stub declares its schema.
    const before = (await rt.slates.catalog({ name: "notes" }, asThread)).text;
    expect(before).toContain("list_text [text only, read-only]: Lists the inbox as markdown.");
    expect(before).toContain("count [text only, read-only]: Counts the calls so far.");
    expect(before).toContain("Each tool says whether it returns data");
    expect((await rt.slates.catalog({ name: "stub" }, asThread)).text).toContain("list_items [returns data, read-only]");

    await rt.slates.write({
      text: `<slate title="Notes">
  <run name="plain" tool="notes.list_text" />
  <run name="shaped" tool="notes.list_text" then='${SHAPE}' />
  <run name="count" tool="notes.count" />
  <column>
    <text id="raw" value={$plain.out} />
    <table id="items" items={$shaped.json.items} key={item.id}><col title="From" value={item.from} /><col title="Subject" value={item.subject} /></table>
    <button id="p" label="Plain" onPress={start($plain)} />
    <button id="s" label="Shaped" onPress={start($shaped)} />
    <button id="c" label="Count" onPress={start($count)} />
  </column>
</slate>`,
    }, asThread);

    // The text-only call: the record says text, json stays empty, the sketch says text only.
    const plain = await press("p");
    expect(plain.ask).toMatchObject({ kind: "tool", server: "notes", tool: "list_text" });
    await rt.slates.approve({ threadId, key: plain.ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await get()).values["plain"]).toMatchObject({ state: "done", text: true, out: "# Inbox\n- 1 | ann | Hello\n- 2 | bo | Invoice" }), { timeout: 10_000 });
    expect((await get()).values["plain"]).not.toHaveProperty("json");
    expect((await rt.slates.read({}, asThread)).text).toMatch(/\$plain: done, text only/);

    // A structured answer with no outputSchema: the probe moves count to returns data, and its record has no text mark.
    await rt.slates.approve({ threadId, key: (await press("c")).ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await get()).values["count"]).toMatchObject({ state: "done", json: { n: 2 } }), { timeout: 10_000 });
    expect((await get()).values["count"]).not.toHaveProperty("text");
    const after = (await rt.slates.catalog({ name: "notes" }, asThread)).text;
    expect(after).toContain("count [returns data, read-only]");
    expect(after).toContain("list_text [text only, read-only]");

    // The reshape is a command on this computer, so the same tool with it is new code, asked with it on the sheet.
    const shaped = await press("s");
    expect(shaped.ask).toMatchObject({ kind: "tool", server: "notes", tool: "list_text", then: SHAPE });
    expect(shaped.ask!.key).toMatch(/^mcp:notes#[0-9a-f]{32}$/);
    expect(shaped.ask!.key).not.toBe(plain.ask!.key);
    await rt.slates.approve({ threadId, key: shaped.ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await get()).values["shaped"]).toMatchObject({ state: "done", out: "# Inbox\n- 1 | ann | Hello\n- 2 | bo | Invoice", json: { items: [{ id: 1, from: "ann", subject: "Hello" }, { id: 2, from: "bo", subject: "Invoice" }] } }), { timeout: 10_000 });
    expect((await get()).values["shaped"]).not.toHaveProperty("text");
    expect((await rt.slates.read({}, asThread)).text).toMatch(/\$shaped: done \(exit 0/);
    // Allowed for the thread, the next press runs with no sheet.
    expect((await press("s")).ask).toBeUndefined();
    await vi.waitFor(async () => expect((await get()).values["shaped"]).toMatchObject({ state: "done", runs: 2 }), { timeout: 10_000 });
    // Revoking it stops what it covered, so the next press asks again; the plain run stays allowed.
    await rt.slates.revoke({ threadId, key: shaped.ask!.key });
    expect((await press("s")).ask).toMatchObject({ kind: "tool", server: "notes", then: SHAPE });
    // Spaced past the five events a second a window may send.
    await new Promise(r => setTimeout(r, 1100));
    expect((await press("p")).ask).toBeUndefined();
  }, 60_000);

  it("a tool run started again keeps its last result in view, marked refreshing, until the new one lands", async () => {
    const { rt, threadId, asThread, events } = await host();
    await rt.slates.write({ text: `<slate title="Count"><run name="count" tool="notes.count" description="Counts the calls so far" /><column><button id="c" label="Count" onPress={start($count)} /></column></slate>` }, asThread);
    const view = async (): Promise<SlateView> => {
      await rt.slates.settled();
      return (await rt.slates.get(threadId))!;
    };
    const press = async (n: number) => rt.slates.event({ threadId, version: (await view()).version, piece: "c", event: "press", requestId: `k-${n}` });
    await rt.slates.approve({ threadId, key: (await press(0)).ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await view()).values["count"]).toMatchObject({ state: "done", runs: 1 }), { timeout: 10_000 });
    const first = (await view()).values["count"] as { json: unknown };
    await press(1);
    await vi.waitFor(async () => expect((await view()).values["count"]).toMatchObject({ state: "done", runs: 2 }), { timeout: 10_000 });
    const running = events.flatMap(e => (e.type === "slate.values" && e.threadId === threadId ? [e.values["$count"]] : [])).find(v => (v as { state?: string; runs?: number } | undefined)?.state === "running" && (v as { runs: number }).runs === 2);
    expect(running).toMatchObject({ state: "running", runs: 2, refreshing: true, json: first.json });
  }, 30_000);

  it("the person's presses on a tool run meet the same start budget a command's do", async () => {
    const { rt, threadId, asThread } = await host();
    await rt.slates.write({ text: `<slate title="Count"><run name="count" tool="notes.count" description="Counts the calls so far" /><column><button id="c" label="Count" onPress={start($count)} /></column></slate>` }, asThread);
    const view = async (): Promise<SlateView> => {
      await rt.slates.settled();
      return (await rt.slates.get(threadId))!;
    };
    const press = async (n: number) => rt.slates.event({ threadId, version: (await view()).version, piece: "c", event: "press", requestId: `c-${n}` });
    await rt.slates.approve({ threadId, key: (await press(0)).ask!.key, scope: "thread" });
    // Spaced past the five events a second a window may send, so each press reaches the run.
    for (let n = 1; n <= STARTS_PER_MINUTE; n++) {
      await new Promise(r => setTimeout(r, 220));
      await press(n);
    }
    expect((await view()).values["count"]).toMatchObject({ state: "held", why: HELD_PRESSED });
  }, 30_000);

  it("a then that reads the slate's own file runs it from SLATE_DIR, with the file's text in its consent", async () => {
    const { rt, threadId, asThread } = await host();
    const get = async (): Promise<SlateView> => {
      await rt.slates.settled();
      return (await rt.slates.get(threadId))!;
    };
    const press = async () => rt.slates.event({ threadId, version: (await get()).version, piece: "go", event: "press", requestId: `go-${Date.now()}` });
    const shapedBy = (code: string) => `<slate>
  <run name="first" tool="notes.list_text" then='sh "$SLATE_DIR/first.sh"' description="Reads the first line of the inbox" />
  <column><button id="go" label="Go" onPress={start($first)} /></column>
  <file name="first.sh">{\`${code}\`}</file>
</slate>`;
    const CODE = `read -r line; printf '{"first":"%s"}' "$line"`;
    await rt.slates.write({ text: shapedBy(CODE) }, asThread);
    const asked = await press();
    expect(asked.ask).toMatchObject({ kind: "tool", then: 'sh "$SLATE_DIR/first.sh"', files: { "first.sh": CODE } });
    await rt.slates.approve({ threadId, key: asked.ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await get()).values["first"]).toMatchObject({ state: "done", json: { first: "# Inbox" } }), { timeout: 10_000 });
    // New code under the same then asks again.
    await rt.slates.write({ text: shapedBy(`${CODE}; :`) }, asThread);
    const again = await press();
    expect(again.ask).toMatchObject({ kind: "tool", files: { "first.sh": `${CODE}; :` } });
    expect(again.ask!.key).not.toBe(asked.ask!.key);
  }, 60_000);
});
