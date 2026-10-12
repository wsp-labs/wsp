// SPDX-License-Identifier: AGPL-3.0-only
// Settle and delete when a thread still runs processes beside its agent: one
// click while it runs none, else one question listing what it runs, each with
// its port and memory, then stop them or keep them running; Escape settles
// nothing. What the host listed is all the dialog shows.
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { fmtBytes, PROCESSES_LEFT_KIND, type ActivityEnded, type ActivityThread, type ProcessesAsk, type SessionSettleResult } from "@wsp/protocol";
import { makeApi, ProtocolClient, RequestError, type Api } from "../src/protocol/client.js";
import { ScriptedSocket } from "./scripted-socket.js";
import { useStore } from "../src/protocol/store.js";
import { SettleProcessesConfirm } from "../src/components/procs/ProcessesLeft.js";
import { useSettleAsk } from "../src/components/procs/leftAsk.js";
import { settleSaying } from "../src/actions/threadActions.js";
import { clearNotices, lastNotice } from "./notice-text.js";
import { DeleteThreadDialog } from "../src/components/DeleteThreadDialog.js";

const vite = (threadId: string, title: string, pid: number, port: number): ActivityThread => ({
  threadId,
  title,
  workspaceId: "ws_lab",
  memory: 100_000_000,
  cpu: 0,
  processes: [
    { pid, ppid: 1, depth: 1, startedAt: 1, kind: "other", name: "node", command: "node /lab/node_modules/.bin/vite", memory: 80_000_000, cpu: 0, ports: [port] },
    { pid: pid + 1, ppid: pid, depth: 2, startedAt: 2, kind: "other", name: "esbuild", command: "esbuild --service", memory: 20_000_000, cpu: 0, ports: [] },
  ],
});

const refusedOver = (left: readonly ActivityThread[]): RequestError => new RequestError("a thread to settle still runs 2 processes", PROCESSES_LEFT_KIND, "settle it again", left);
const SETTLED: SessionSettleResult = { settled: [{ threadId: "thr_a", title: "fix login" }], left: [] };

/** An api whose settle refuses with ask while `left` runs, and settles with any answer. */
function settling(left: readonly ActivityThread[]) {
  const asked: (ProcessesAsk | undefined)[] = [];
  const api = {
    settleThreads: async (_ids: readonly string[], processes?: ProcessesAsk) => {
      asked.push(processes);
      if (processes === "ask" && left.length > 0) throw refusedOver(left);
      return SETTLED;
    },
  } as unknown as Api;
  useStore.setState({ api });
  render(<SettleProcessesConfirm />);
  return { asked, settle: () => useStore.getState().settleThreads(["thr_a"]) };
}

afterEach(() => {
  useSettleAsk.getState().ask?.done(undefined);
  cleanup();
  useStore.setState({ api: null });
});

describe("settle with processes left", () => {
  it("settles in one call with no dialog when the thread runs nothing beside its agent", async () => {
    const h = settling([]);
    await expect(h.settle()).resolves.toEqual(SETTLED);
    expect(h.asked).toEqual(["ask"]);
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("lists the dev server with its port and memory and its child under it, and Settle and stop them settles with stop", async () => {
    const h = settling([vite("thr_a", "fix login", 4200, 5173)]);
    const answer = h.settle();
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.textContent).toContain("Stop what fix login still runs?");
    expect(dialog.querySelector('[data-k="settle-processes-refusal"]')!.textContent).toBe("Settling leaves these running. Stop ends each one and what it started.");
    const row = dialog.querySelector('[data-left-proc="4200"]')!;
    expect(row.textContent).toContain("vite");
    expect(row.textContent).not.toContain("node");
    expect(row.textContent).toContain(":5173");
    expect(row.textContent).toContain(fmtBytes(80_000_000));
    expect(dialog.querySelector<HTMLElement>('[data-left-proc="4201"]')!.style.getPropertyValue("--depth")).toBe("1");
    fireEvent.click(screen.getByRole("button", { name: "Settle and stop them" }));
    await expect(answer).resolves.toEqual(SETTLED);
    expect(h.asked).toEqual(["ask", "stop"]);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  });

  it("Settle, keep running settles with keep", async () => {
    const h = settling([vite("thr_a", "fix login", 4200, 5173)]);
    const answer = h.settle();
    fireEvent.click(await screen.findByRole("button", { name: "Settle, keep running" }));
    await expect(answer).resolves.toEqual(SETTLED);
    expect(h.asked).toEqual(["ask", "keep"]);
  });

  it("Escape settles nothing", async () => {
    const h = settling([vite("thr_a", "fix login", 4200, 5173)]);
    const answer = h.settle();
    const dialog = await screen.findByRole("alertdialog");
    act(() => void fireEvent.keyDown(dialog, { key: "Escape" }));
    await expect(answer).resolves.toBeUndefined();
    expect(h.asked).toEqual(["ask"]);
  });

  it("names each thread over its own processes when a settle takes several that run some", async () => {
    settling([vite("thr_a", "fix login", 4200, 5173), vite("thr_b", "docs", 4600, 5174)]);
    void useStore.getState().settleThreads(["thr_a"]);
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.textContent).toContain("Stop what 2 threads still run?");
    expect(dialog.querySelector('[data-left-thread="thr_b"]')!.textContent).toBe("docs");
    expect(dialog.querySelector('[data-left-proc="4600"]')!.textContent).toContain(":5174");
  });

  it("a refusal of the answer stays in the dialog, which keeps the question", async () => {
    useStore.setState({
      api: {
        settleThreads: async (_ids: readonly string[], processes?: ProcessesAsk) => {
          if (processes === "stop") throw new RequestError("4200 would not end");
          throw refusedOver([vite("thr_a", "fix login", 4200, 5173)]);
        },
      } as unknown as Api,
    });
    render(<SettleProcessesConfirm />);
    void useStore.getState().settleThreads(["thr_a"]);
    fireEvent.click(await screen.findByRole("button", { name: "Settle and stop them" }));
    await waitFor(() => expect(document.querySelector('[data-k="settle-processes-refusal"]')!.textContent).toBe("4200 would not end"));
    expect(screen.getByRole("alertdialog")).toBeTruthy();
  });
});

describe("delete with processes left", () => {
  function deleting(left: readonly ActivityThread[], ended: readonly ActivityEnded[] = []) {
    const asked: { processes?: ProcessesAsk; check?: boolean }[] = [];
    const api = {
      deleteThread: async (_id: string, o: { processes?: ProcessesAsk; check?: boolean } = {}) => {
        asked.push(o);
        if (o.processes === "ask" && left.length > 0) throw refusedOver(left);
        return { workspaceId: "ws_lab", threads: 1, ...(o.processes === "stop" ? { ended } : {}) };
      },
    } as unknown as Api;
    useStore.setState({ api });
    const closed: boolean[] = [];
    render(<DeleteThreadDialog thread={{ threadId: "thr_a", workspaceId: "ws_lab", title: "fix login" }} onClose={() => closed.push(true)} />);
    return { asked, closed };
  }

  it("offers the one Delete when the thread runs nothing beside its agent", async () => {
    const h = deleting([]);
    await waitFor(() => expect(h.asked).toEqual([{ processes: "ask", check: true }]));
    expect(document.querySelector('[data-k="processes-left"]')).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(h.closed).toEqual([true]));
    expect(h.asked.at(-1)).toEqual({ processes: "ask" });
  });

  it("lists what the thread still runs as soon as it opens, and Delete and stop them deletes with stop", async () => {
    const h = deleting([vite("thr_a", "fix login", 4200, 5173)]);
    await waitFor(() => expect(document.querySelector('[data-left-proc="4200"]')?.textContent).toContain(":5173"));
    expect(document.querySelector('[data-k="delete-thread-refusal"]')!.textContent).toBe("It still runs 2 processes. Stop ends each one and what it started.");
    expect(screen.queryByRole("button", { name: "Delete" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Delete and stop them" }));
    await waitFor(() => expect(h.closed).toEqual([true]));
    expect(h.asked.at(-1)).toEqual({ processes: "stop" });
  });

  it("a stop that left a process running says so and offers Close instead of closing", async () => {
    const t = vite("thr_a", "fix login", 4200, 5173);
    const h = deleting([t], [{ pid: 4200, ended: t.processes, forced: [4200, 4201], left: [4200] }]);
    fireEvent.click(await screen.findByRole("button", { name: "Delete and stop them" }));
    await waitFor(() => expect(document.querySelector('[data-k="delete-thread-refusal"]')!.textContent).toBe("Still running after Stop: 4200 vite."));
    expect(h.closed).toEqual([]);
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(h.closed).toEqual([true]);
  });

  it("Delete, keep running deletes with keep", async () => {
    const h = deleting([vite("thr_a", "fix login", 4200, 5173)]);
    fireEvent.click(await screen.findByRole("button", { name: "Delete, keep running" }));
    await waitFor(() => expect(h.closed).toEqual([true]));
    expect(h.asked.at(-1)).toEqual({ processes: "keep" });
  });
});

describe("a refused settle on the wire", () => {
  it("reaches the dialog through the client: the frame's threads become the rows, and the answer goes back on the same socket", async () => {
    ScriptedSocket.instances.length = 0;
    const client = new ProtocolClient({ url: "ws://test", token: "tok", WebSocketCtor: ScriptedSocket as unknown as typeof WebSocket });
    await client.connect();
    const left = [{ ...vite("thr_a", "fix login", 4200, 5173), on: "hetzner" }];
    const sent: unknown[] = [];
    ScriptedSocket.reply = f => {
      if (f["op"] !== "sessions.settle") return { id: f["id"], ok: true };
      sent.push(f["processes"]);
      return f["processes"] === "ask"
        ? { id: f["id"], ok: false, kind: PROCESSES_LEFT_KIND, error: "a thread to settle still runs 2 processes", fix: "settle it again", left }
        : { id: f["id"], ok: true, settled: [{ threadId: "thr_a", title: "fix login" }], left: [] };
    };
    useStore.setState({ api: makeApi(client) });
    render(<SettleProcessesConfirm />);
    const answer = useStore.getState().settleThreads(["thr_a"]);
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.querySelector('[data-left-proc="4200"]')!.textContent).toContain(":5173");
    expect(dialog.querySelector('[data-k="settle-processes-refusal"]')!.textContent).toBe("Settling leaves these running. Stop ends everything the thread started on hetzner.");
    fireEvent.click(screen.getByRole("button", { name: "Settle and stop them" }));
    await expect(answer).resolves.toMatchObject({ settled: [{ threadId: "thr_a" }] });
    expect(sent).toEqual(["ask", "stop"]);
    client.close();
  });
});

describe("the settle notice", () => {
  it("names a process a stop left running beside what it settled", async () => {
    clearNotices();
    const t = vite("thr_a", "fix login", 4200, 5173);
    await settleSaying(["thr_a"], { settle: async () => ({ ...SETTLED, ended: [{ pid: 4200, ended: t.processes, forced: [4201], left: [4201] }] }), restore: async () => {} });
    expect(lastNotice()).toBe("Settled 1 thread. Still running after Stop: 4201 esbuild.");
  });

  it("says nothing more when the stop ended everything", async () => {
    clearNotices();
    const t = vite("thr_a", "fix login", 4200, 5173);
    await settleSaying(["thr_a"], { settle: async () => ({ ...SETTLED, ended: [{ pid: 4200, ended: t.processes, forced: [], left: [] }] }), restore: async () => {} });
    expect(lastNotice()).toBe("Settled 1 thread");
  });
});
