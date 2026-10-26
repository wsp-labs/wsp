// wsp slate look's road through the host: a look goes to the newest app that offered on the host's own road and is
// answered by its id; a host with no app says so in one sentence, a socket that closes fails what it was drawing, an
// answer from another socket is not taken, an app that never answers is given up on, and the look is offered only while
// an app that draws one is attached.
import { describe, expect, it } from "vitest";
import { NO_LOOK_MCP_ARG, NO_SLATE_MCP_ARG, SCOPED_MCP_ARG, type SlateLookWantedEvent, type SlatesLookAnswer } from "@wsp/protocol";
import { SLATE_LOOK_NO_APP, SLATE_LOOK_OFF_ROAD, slateLookHub, type SlateLookSocket } from "../src/serve-slate-look.js";
import { answerSlate } from "../src/serve-slates.js";
import type { Slates } from "../src/slates-api.js";
import { servedTo } from "../src/threads/launch.js";

const DRAWN: SlatesLookAnswer = { png: "iVBORw0KGgo=", width: 400, height: 640, scale: 2, theme: "graphite" };

function app(own = true): SlateLookSocket & { heard: SlateLookWantedEvent[]; close(): void } {
  const heard: SlateLookWantedEvent[] = [];
  const detaches: (() => void)[] = [];
  return { heard, detaches, own, send: frame => void heard.push(frame), close: () => detaches.forEach(d => d()) };
}

describe("a slate look through the host", () => {
  it("says in one sentence that no app is open to draw it", async () => {
    await expect(slateLookHub().look("t1")).rejects.toThrow(SLATE_LOOK_NO_APP);
    expect(SLATE_LOOK_NO_APP.split(". ")).toHaveLength(1);
  });

  it("asks the newest app that offered, and answers its picture by the look's id", async () => {
    const hub = slateLookHub();
    const older = app();
    const newer = app();
    hub.answer({ id: 1, op: "slateLook.offer" }, older);
    hub.answer({ id: 2, op: "slateLook.offer" }, newer);
    const look = hub.look("t1", 640);
    expect(older.heard).toEqual([]);
    expect(newer.heard).toEqual([{ type: "slate.look.wanted", lookId: expect.any(String), threadId: "t1", width: 640 }]);
    const lookId = newer.heard[0]!.lookId;
    // Another socket cannot answer a look it was never asked for.
    hub.answer({ id: 3, op: "slateLook.answer", lookId, look: { ...DRAWN, theme: "paper" } }, older);
    hub.answer({ id: 4, op: "slateLook.answer", lookId, look: DRAWN }, newer);
    await expect(look).resolves.toEqual(DRAWN);
  });

  it("passes on the app's own sentence, fails a look whose app closed, and goes back to the one before", async () => {
    const hub = slateLookHub();
    const first = app();
    const second = app();
    hub.answer({ id: 1, op: "slateLook.offer" }, first);
    hub.answer({ id: 2, op: "slateLook.offer" }, second);
    const refused = hub.look("t1");
    hub.answer({ id: 3, op: "slateLook.answer", lookId: second.heard[0]!.lookId, problem: "This thread has no slate yet." }, second);
    await expect(refused).rejects.toThrow("This thread has no slate yet.");
    const cut = hub.look("t1");
    second.close();
    await expect(cut).rejects.toThrow(SLATE_LOOK_NO_APP);
    void hub.look("t1").catch(() => undefined);
    expect(first.heard).toHaveLength(1);
    first.close();
    await expect(hub.look("t1")).rejects.toThrow(SLATE_LOOK_NO_APP);
  });

  it("is offered only while an app that draws looks is attached: the launch's tools and the catalog's rule follow it", async () => {
    const hub = slateLookHub();
    const asked: { look?: boolean }[] = [];
    const slates = { catalog: async (p: { look?: boolean }) => (asked.push(p), { text: "" }) } as unknown as Slates;
    const catalog = () => answerSlate(slates, { id: 1, op: "slates.catalog" }, undefined, { detaches: [] } as never, { hub, send: () => undefined, own: true });
    const wsp = { command: "wsp", args: ["mcp", SCOPED_MCP_ARG] };
    expect(hub.lookable()).toBe(false);
    await catalog();
    expect(servedTo(wsp, "t1", "t1", hub.lookable())).toEqual({ command: "wsp", args: ["mcp", SCOPED_MCP_ARG, NO_LOOK_MCP_ARG] });
    const open = app();
    hub.answer({ id: 2, op: "slateLook.offer" }, open);
    expect(hub.lookable()).toBe(true);
    await catalog();
    expect(servedTo(wsp, "t1", "t1", hub.lookable())).toBe(wsp);
    open.close();
    expect(hub.lookable()).toBe(false);
    expect(asked.map(p => p.look)).toEqual([false, true]);
    // A thread another thread started has no slate at all, so no look either.
    expect(servedTo(wsp, "t1", "t2", false)).toEqual({ command: "wsp", args: ["mcp", SCOPED_MCP_ARG, NO_SLATE_MCP_ARG], noSlate: true });
  });

  it("takes an offer only on the host's own road, and gives up on an app that never answers", async () => {
    const hub = slateLookHub(50);
    expect(() => hub.answer({ id: 1, op: "slateLook.offer" }, app(false))).toThrow(SLATE_LOOK_OFF_ROAD);
    hub.answer({ id: 2, op: "slateLook.offer" }, app());
    await expect(hub.look("t1")).rejects.toThrow("The app did not draw the slate within 0 s; look again once the app has finished starting.");
  });
});
