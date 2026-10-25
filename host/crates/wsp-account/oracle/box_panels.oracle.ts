// SPDX-License-Identifier: AGPL-3.0-only
// The TS answers the box panels tests compare to: procWatchScope and namedWatchRefusal over a table of inputs, and
// boxPanelsOpener (packages/runtime/src/account/box-panels.ts at 65f6b89c7) driven through scripted pane sessions
// against a fake context and a fake daemon that records every frame it is served. Written as
// tests/fixtures/oracle/box_panels.json. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-account/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 box_panels.oracle.ts
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { it } from "vitest";
import { threadCgroup } from "@wsp/protocol";

const { procWatchScope, namedWatchRefusal, boxPanelsOpener } = await import("../../../../packages/runtime/src/account/box-panels.ts");
const out = fileURLToPath(new URL("../tests/fixtures/oracle/box_panels.json", import.meta.url));

type Frame = Record<string, unknown>;
const tick = () => new Promise(r => setTimeout(r, 0));

/** One world: a workspace on box1 with `threads` thread records, sessions for some, and a daemon whose replies the
 * session scripts. Every served frame is logged in order. */
function world(o: { threads: string[]; started?: Record<string, number>; version?: number }) {
  const listeners: ((e: Frame) => void)[] = [];
  const served: Frame[] = [];
  const replies: ((f: Frame) => Frame | undefined)[] = [];
  const threadRecords = new Map(o.threads.map(t => [t, { workspaceId: "w1" }]));
  const sessions = new Map(Object.entries(o.started ?? {}).map(([t, at], i) => [`s${i}`, { view: { workspaceId: "w1", threadId: t, startedAt: at } }]));
  let link: object = { link: 1 };
  let sysRefusals = 0;
  const released: string[] = [];
  const ctx = {
    bus: { on: (_t: string, l: (e: Frame) => void) => void listeners.push(l) },
    sessions,
    threadRecords,
    placeDoor: {
      link: () => link,
      nameOf: () => "studio",
      reportOf: async () => (o.version === undefined ? {} : { daemonVersion: o.version }),
      watchSys: (_p: string, _f: () => void) => (sysRefusals > 0 ? (sysRefusals--, Promise.reject(new Error("this computer reads no load"))) : Promise.resolve(() => void released.push("sys"))),
    },
    scopesOn: () => [...threadRecords.keys()].map(t => ({ thread: t, workspace: "w1", folder: "/srv/alpha", cgroup: threadCgroup(t) })),
    threadEnv: () => ({ WSP_THREAD_HOME: "/srv/alpha", PATH: "/usr/bin" }),
  };
  const daemon = async (f: Frame): Promise<Frame> => {
    served.push(structuredClone(f));
    for (const r of replies) {
      const said = r(f);
      if (said !== undefined) return said;
    }
    if (f["op"] === "ports.watch") return { ok: true, ports: [] };
    if (f["op"] === "pty.create") return { ok: true, ptyId: "pty_1", pid: 4242 };
    return { ok: true };
  };
  const placements = new Map<string, Map<string, number>>();
  const watchers = new Map<string, Set<() => void>>();
  const open = boxPanelsOpener(ctx as never, watchers, placements);
  return {
    ctx, served, replies, threadRecords, sessions, placements, watchers, released,
    pane: () => open({ record: { id: "w1" } } as never, "box1", daemon, "/srv/alpha"),
    relink: () => (link = { link: 2 }),
    refuseSys: (n: number) => (sysRefusals = n),
    deleted: () => listeners.forEach(l => l({ type: "workspace.deleted", workspaceId: "w1" })),
    placed: () => Object.fromEntries([...(placements.get("w1") ?? new Map())]),
    push: async () => {
      for (const p of watchers.get("w1") ?? []) p();
      await tick();
    },
  };
}

const ids = (n: number, from = 0) => Array.from({ length: n }, (_, i) => `t${String(i + from).padStart(2, "0")}`);

it("writes the box panels fixture", async () => {
  const scopeCases: [string, (readonly string[] | null)[]][] = [
    ["none", []],
    ["one null", [null]],
    ["null among asks", [["/wsp-threads/a"], null]],
    ["two", [["/wsp-threads/b"], ["/wsp-threads/a"]]],
    ["duplicates", [["/wsp-threads/a", "/wsp-threads/a"], ["/wsp-threads/a"]]],
    ["nested", [["/wsp-threads/a"], ["/wsp-threads/a/x", "/wsp-threads/ab"]]],
    ["empty ask", [[]]],
    ["sixteen", [ids(16).map(threadCgroup)]],
    ["seventeen", [ids(17).map(threadCgroup)]],
    ["seventeen nested to sixteen", [[...ids(16).map(threadCgroup), `${threadCgroup("t00")}/child`]]],
  ];
  const scope = scopeCases.map(([name, asks]) => ({ name, asks, answer: procWatchScope(asks) }));
  const refusal = [undefined, 0, 135, 136, 200].map(v => ({ version: v ?? null, answer: namedWatchRefusal("studio", v) ?? null }));

  const sessions: Record<string, unknown> = {};

  // Ports: three threads, newest first by session start, one watch.
  {
    const w = world({ threads: ["ta", "tb", "tc"], started: { tb: 30, ta: 10 }, version: 200 });
    w.replies.push(f => (f["op"] === "ports.watch" ? { ok: true, ports: [{ port: 3000, pid: 1 }, { port: 5173, pid: 2 }] } : undefined));
    const p = w.pane();
    const reply = await p.send({ op: "ports.watch", roots: [7, 7.5, "x", 9] });
    sessions["ports three threads"] = { reply, served: w.served, placed: w.placed() };
  }
  // Ports: twenty threads, two watches; then a thread goes and one starts; then fifteen go.
  {
    const w = world({ threads: ids(20), version: 200 });
    const p = w.pane();
    const first = await p.send({ op: "ports.watch", roots: [] });
    const placedFirst = w.placed();
    const servedFirst = w.served.splice(0);
    w.threadRecords.delete("t03");
    w.threadRecords.set("t20", { workspaceId: "w1" });
    await w.push();
    const placedMoved = w.placed();
    const servedMoved = w.served.splice(0);
    for (const t of ids(15, 5)) w.threadRecords.delete(t);
    await w.push();
    const servedShrunk = w.served.splice(0);
    await w.push();
    const servedSame = w.served.splice(0);
    sessions["ports twenty threads"] = { first, placedFirst, servedFirst, placedMoved, servedMoved, placedShrunk: w.placed(), servedShrunk, servedSame };
  }
  // Ports: a port in two watches is heard open once and closed once; unknown watches are never heard.
  {
    const w = world({ threads: ids(17), version: 200 });
    w.replies.push(f => (f["op"] === "ports.watch" ? { ok: true, ports: f["watch"] === "w1" ? [{ port: 3000, pid: 1 }] : [{ port: 3000, pid: 9 }, { port: 4000, pid: 3 }] } : undefined));
    const p = w.pane();
    const reply = await p.send({ op: "ports.watch", roots: [] });
    const events: [string, number, string][] = [
      ["port.close", 3000, "w1"],
      ["port.close", 3000, "w1-2"],
      ["port.open", 3000, "w1-2"],
      ["port.open", 3000, "w1"],
      ["port.close", 9999, "w1"],
      ["port.open", 4000, "other"],
      ["port.close", 4000, "w1-2"],
      ["port.close", 4000, "w1-2"],
    ];
    const heard = events.map(([type, port, watch]) => ({ type, port, watch, heard: p.heard({ type, port, watch }) }));
    sessions["ports holders"] = { reply, heard };
  }
  // Ports: a daemon from before named watches, and one that names no version, are refused before anything is sent.
  for (const version of [135, undefined]) {
    const w = world({ threads: ["ta"], ...(version !== undefined ? { version } : {}) });
    const p = w.pane();
    sessions[`ports behind ${version ?? "unknown"}`] = { reply: await p.send({ op: "ports.watch", roots: [] }), served: w.served };
  }
  // Ports: one share refused answers that refusal whole.
  {
    const w = world({ threads: ids(17), version: 200 });
    w.replies.push(f => (f["op"] === "ports.watch" && f["watch"] === "w1-2" ? { ok: false, error: "too many watches" } : undefined));
    const p = w.pane();
    sessions["ports one share refused"] = { reply: await p.send({ op: "ports.watch", roots: [] }), served: w.served };
  }
  // Processes: two panes on one link, their asks merged; one leaves, then the other; scopes go before each watch.
  {
    const w = world({ threads: ["ta", "tb"], version: 200 });
    const a = w.pane();
    const b = w.pane();
    const steps: unknown[] = [];
    steps.push({ step: "a watches ta", reply: await a.send({ op: "proc.watch", cgroups: [threadCgroup("ta"), 5] }), served: w.served.splice(0) });
    steps.push({ step: "b watches tb", reply: await b.send({ op: "proc.watch", cgroups: [threadCgroup("tb")] }), served: w.served.splice(0) });
    steps.push({ step: "a heard", snapshot: a.heard({ type: "proc.snapshot" }), changes: a.heard({ type: "proc.changes" }), sys: a.heard({ type: "sys.sample" }), other: a.heard({ type: "pty.data" }) });
    steps.push({ step: "a unwatches", reply: await a.send({ op: "proc.unwatch" }), served: (await tick(), w.served.splice(0)) });
    steps.push({ step: "a heard after", snapshot: a.heard({ type: "proc.snapshot" }) });
    steps.push({ step: "a unwatches again", reply: await a.send({ op: "proc.unwatch" }), served: (await tick(), w.served.splice(0)) });
    w.threadRecords.set("tc", { workspaceId: "w1" });
    await w.push();
    steps.push({ step: "a thread starts", served: w.served.splice(0) });
    await w.push();
    steps.push({ step: "nothing moved", served: w.served.splice(0) });
    b.close();
    await tick();
    steps.push({ step: "b closes", served: w.served.splice(0) });
    b.close();
    await tick();
    steps.push({ step: "b closes again", served: w.served.splice(0) });
    sessions["procs two panes"] = steps;
  }
  // Processes: a pane asking for every process makes the watch whole; a refused watch takes the pane's ask back.
  {
    const w = world({ threads: ["ta"], version: 200 });
    const a = w.pane();
    const b = w.pane();
    const steps: unknown[] = [];
    steps.push({ step: "a watches ta", reply: await a.send({ op: "proc.watch", cgroups: [threadCgroup("ta")] }), served: w.served.splice(0) });
    steps.push({ step: "b watches all", reply: await b.send({ op: "proc.watch" }), served: w.served.splice(0) });
    w.replies.push(f => (f["op"] === "proc.watch" ? { ok: false, error: "proc reads off" } : undefined));
    steps.push({ step: "b refused", reply: await b.send({ op: "proc.watch", cgroups: [] }), served: (await tick(), w.served.splice(0)), heard: b.heard({ type: "proc.snapshot" }) });
    sessions["procs whole and refused"] = steps;
  }
  // Processes: a link replaced since the pane opened is told nothing when the pane leaves.
  {
    const w = world({ threads: ["ta"], version: 200 });
    const a = w.pane();
    await a.send({ op: "proc.watch", cgroups: [threadCgroup("ta")] });
    w.served.splice(0);
    w.relink();
    a.close();
    await tick();
    sessions["procs link replaced"] = { served: w.served };
  }
  // Readings: held once per pane, a failure answered and let go, unwatch releases.
  {
    const w = world({ threads: ["ta"], version: 200 });
    const a = w.pane();
    const steps: unknown[] = [];
    w.refuseSys(1);
    steps.push({ step: "refused", reply: await a.send({ op: "sys.watch" }), heard: a.heard({ type: "sys.sample" }) });
    steps.push({ step: "taken", reply: await a.send({ op: "sys.watch" }), heard: a.heard({ type: "sys.sample" }) });
    steps.push({ step: "again", reply: await a.send({ op: "sys.watch" }) });
    steps.push({ step: "unwatch", reply: await a.send({ op: "sys.unwatch" }), released: (await tick(), [...w.released]), heard: a.heard({ type: "sys.sample" }) });
    steps.push({ step: "unwatch again", reply: await a.send({ op: "sys.unwatch" }), released: (await tick(), [...w.released]) });
    steps.push({ step: "served", served: w.served });
    sessions["readings"] = steps;
  }
  // Terminal: the turn's environment under the pane's, the folder unless the pane names a cwd; other ops pass whole.
  {
    const w = world({ threads: ["ta"], version: 200 });
    const a = w.pane();
    const replies = [
      await a.send({ op: "pty.create", cols: 80, env: { PATH: "/opt/bin", TERM: "xterm" } }),
      await a.send({ op: "pty.create", cwd: "/srv/alpha/sub", env: null }),
      await a.send({ op: "pty.create", cwd: 7 }),
      await a.send({ op: "container.stop", id: "c1" }),
      await a.send({ op: "fs.list", path: "/" }),
    ];
    sessions["terminal and pass through"] = { replies, served: w.served, answers: ["ports.watch", "sys.watch", "sys.unwatch", "proc.watch", "proc.unwatch", "proc.inspect", "proc.kill", "container.stop", "pty.create", "pty.list", "fs.list", "git.status"].map(op => [op, a.answers(op)]) };
  }
  // Close: the pane's push leaves the watchers; a deleted workspace drops its placements.
  {
    const w = world({ threads: ["ta"], version: 200 });
    const a = w.pane();
    const b = w.pane();
    await a.send({ op: "ports.watch", roots: [] });
    const before = { pushes: w.watchers.get("w1")?.size ?? 0, placed: w.placed() };
    a.close();
    const afterClose = { pushes: w.watchers.get("w1")?.size ?? 0 };
    w.deleted();
    const afterDelete = { placements: w.placements.has("w1") };
    b.close();
    sessions["close and delete"] = { before, afterClose, afterDelete };
  }
  writeFileSync(out, `${JSON.stringify({ scope, refusal, sessions }, null, 1)}\n`);
});
