// SPDX-License-Identifier: AGPL-3.0-only
// MCP runs in the renderer: a tool run's result drawn by its shape through the kit's pieces, and the plain sheet before
// a tool run goes, naming the tool and its server, what the run does, and its code folded.
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { type SessionView } from "@wsp/protocol";
import { parseSlate, slateFieldWords, slateResultShape, slateStartValues, type SlateDoc, type SlateJson } from "@wsp/protocol/slate";
import type { Api, ProtocolEvent } from "../protocol/client";
import { useStore } from "../protocol/store";
import { useRightPanelStore } from "../rightPanelStore";
import { ActionRunner, StateSender } from "./actions";
import { SlateEngine } from "./engine";
import { ToolSheet, type SlateToolAsk } from "./mcp";
import { SLATE_VIEWS } from "./pieces";
import { SlateSurface } from "./SlateSurface";
import { SlateView } from "./SlateView";
import { useSlateStore } from "./store";
import { fakeLink, manualScheduler } from "./testing";
import type { SlateApi, SlateRecord } from "./wire";

afterEach(cleanup);

function compiled(text: string): SlateDoc {
  const r = parseSlate(text);
  expect(r.errors).toEqual([]);
  return r.document!;
}

function draw(doc: SlateDoc, state: Record<string, SlateJson> = {}) {
  const engine = new SlateEngine("t1", () => undefined, manualScheduler());
  engine.setRecord(doc, { ...slateStartValues(doc), ...state }, 3, 3);
  const link = fakeLink();
  return render(<SlateView engine={engine} views={SLATE_VIEWS} runner={new ActionRunner(engine, () => link)} sender={new StateSender(engine, () => link)} />);
}

const INBOX = compiled(`<slate title="Mail">
  <run name="inbox" tool="zoho-mail.zoho_list_emails" args={{ params: { account: "admin", limit: 12 } }} every={60} />
  <output run={$inbox} label="Inbox" />
</slate>`);

const EMAILS = [
  { messageId: "m1", fromAddress: "billing@vercel.com", subject: "Your invoice", receivedTime: "2026-10-04 09:12", flags: { seen: true } },
  { messageId: "m2", fromAddress: "ops@example.com", subject: "Traffic report", receivedTime: "2026-10-04 08:40", flags: { seen: false } },
];

describe("a tool run's result by its shape", () => {
  it("names a field in the words people say", () => {
    expect(slateFieldWords("receivedTime")).toBe("Received time");
    expect(slateFieldWords("from_address")).toBe("From address");
    expect(slateFieldWords("id")).toBe("Id");
  });

  it("takes the fields that hold a plain value as columns, in the order rows name them", () => {
    const keys = (value: SlateJson) => { const shape = slateResultShape(value); return shape.kind === "table" ? shape.columns.map(c => c.key) : shape.kind; };
    expect(keys(EMAILS)).toEqual(["messageId", "fromAddress", "subject", "receivedTime"]);
    expect(keys([{ a: null, "b-c": 1 }, { a: 2 }])).toEqual(["a"]);
  });

  it("draws a list of records as a table under the run's state", () => {
    const c = draw(INBOX, { inbox: { state: "done", out: JSON.stringify(EMAILS), json: EMAILS, runs: 1 } }).container;
    const result = c.querySelector<HTMLElement>('[data-slate-result="inbox"]')!;
    expect(result).not.toBeNull();
    expect(c.querySelector('[role="log"]')).toBeNull();
    const table = within(result).getByRole("table");
    expect(within(table).getAllByRole("columnheader").map(h => h.textContent)).toEqual(["Message id", "From address", "Subject", "Received time"]);
    expect(within(table).getAllByRole("row")).toHaveLength(3);
    expect(within(table).getByText("Traffic report")).toBeTruthy();
    expect(c.querySelector('[data-slate-run-state="done"]')!.textContent).toBe("Done");
  });

  it("draws a record as facts with its list under its name", () => {
    const json = { account: "admin", total: 2, unread: 1, emails: EMAILS };
    const c = draw(INBOX, { inbox: { state: "done", json, runs: 1 } }).container;
    const result = c.querySelector<HTMLElement>('[data-slate-result="inbox"]')!;
    const facts = result.querySelector("[data-slate-facts]")!;
    expect([...facts.querySelectorAll("[data-settings-label]")].map(d => d.textContent)).toEqual(["Account", "Total", "Unread"]);
    expect([...facts.querySelectorAll("[data-settings-word]")].map(d => d.textContent)).toEqual(["admin", "2", "1"]);
    expect(within(result).getByText("Emails")).toBeTruthy();
    expect(within(result).getAllByRole("row")).toHaveLength(3);
  });

  it("draws a text result as text and a list of plain values one per line", () => {
    const text = draw(INBOX, { inbox: { state: "done", out: "No new mail.", runs: 1 } }).container;
    expect(text.querySelector('[data-slate-result="inbox"] p')!.textContent).toBe("No new mail.");
    cleanup();
    const list = draw(INBOX, { inbox: { state: "done", json: ["admin", "ops"], runs: 1 } }).container;
    expect(list.querySelector('[data-slate-result="inbox"] p')!.textContent).toBe("admin\nops");
  });

  it("keeps a failed tool run and a command's output as lines", () => {
    const failed = draw(INBOX, { inbox: { state: "failed", err: "account not found", why: "the tool said it failed", runs: 1 } }).container;
    expect(failed.querySelector('[data-slate-result="inbox"]')).toBeNull();
    expect(failed.querySelector('[role="log"]')!.textContent).toBe("account not found");
    cleanup();
    const cmd = compiled(`<slate title="Disk">
  <run name="df" cmd="df -h /" />
  <output run={$df} />
</slate>`);
    const lines = draw(cmd, { df: { state: "done", out: "[1,2]", json: [1, 2], runs: 1 } }).container;
    expect(lines.querySelector('[data-slate-result="df"]')).toBeNull();
    expect(lines.querySelector('[role="log"]')!.textContent).toBe("[1,2]");
  });
});

const TOOL_ASK: SlateToolAsk = {
  key: "mcp:cloudflare#0123456789abcdef0123456789abcdef",
  run: "traffic",
  kind: "tool",
  server: "cloudflare",
  tool: "execute",
  computer: "hetzner",
  why: "needs your approval",
  description: "Reads the last hour of requests to the site",
  args: { code: "return await cf.graphql(query)", token: "•••••••••••• (12)" },
};
const CADENCE = "Runs every 60 s, only while this slate is on screen";

describe("the sheet before a tool run", () => {
  it("says in plain words what runs, where and how often, with its code folded behind Show code", async () => {
    render(<ToolSheet ask={TOOL_ASK} cadence={CADENCE} answer={vi.fn(async () => {})} onClose={vi.fn()} />);
    const dialog = screen.getByRole("dialog", { name: "Let this panel run execute on Cloudflare?" });
    expect(dialog.querySelector("[data-slate-consent-description]")!.textContent).toBe("Reads the last hour of requests to the site");
    expect(dialog.querySelector("[data-slate-consent-where]")!.textContent).toBe(`On hetzner. ${CADENCE}.`);
    expect(dialog.querySelector("[data-slate-consent-code]")).toBeNull();
    expect(dialog.querySelector("[data-slate-arg]")).toBeNull();
    await act(async () => fireEvent.click(within(dialog).getByRole("button", { name: "Show code" })));
    const code = await waitFor(() => dialog.querySelector<HTMLElement>("[data-slate-consent-code]")!);
    expect(code.querySelector("code")!.textContent).toBe("cloudflare.execute");
    expect([...code.querySelectorAll("dt")].map(d => d.textContent)).toEqual(["code", "token"]);
    expect(code.querySelector('[data-slate-arg="code"] dd')!.textContent).toBe("return await cf.graphql(query)");
    expect(code.querySelector('[data-slate-arg="token"] dd')!.className).toContain("text-muted-foreground");
    const words = dialog.textContent ?? "";
    for (const word of ["Call", "Don't", "Asks every time", "asks each time", "destructive"]) expect(words).not.toContain(word);
    expect(within(dialog).getByRole("button", { name: "Allow" })).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: "Not now" })).toBeTruthy();
    expect(within(dialog).queryByRole("button", { name: "Always in this thread" })).toBeNull();
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it.each([
    ["Allow", "thread"],
    ["Not now", "refuse"],
  ] as const)("%s answers %s and closes", async (label, scope) => {
    const answer = vi.fn(async () => {});
    const onClose = vi.fn();
    render(<ToolSheet ask={TOOL_ASK} cadence="" answer={answer} onClose={onClose} />);
    await act(async () => fireEvent.click(screen.getByRole("button", { name: label })));
    expect(answer).toHaveBeenCalledWith(scope);
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("asks a run that names confirm in the red confirm, its sentence under the title, and Run lets this start go alone", async () => {
    const answer = vi.fn(async () => {});
    render(<ToolSheet ask={{ ...TOOL_ASK, confirm: "Purge the cache for the site?" }} cadence={CADENCE} answer={answer} onClose={vi.fn()} />);
    const dialog = screen.getByRole("alertdialog", { name: "Let this panel run execute on Cloudflare?" });
    expect(within(dialog).getByText("Purge the cache for the site?")).toBeTruthy();
    expect(dialog.querySelector("[data-slate-consent-description]")!.textContent).toBe("Reads the last hour of requests to the site");
    expect(within(dialog).queryByRole("button", { name: "Allow" })).toBeNull();
    const run = within(dialog).getByRole("button", { name: "Run" });
    expect(run.getAttribute("data-variant") ?? run.className).toMatch(/destructive/);
    await act(async () => fireEvent.click(run));
    expect(answer).toHaveBeenCalledWith("once");
  });

  it("refuses a confirm's start on Not now", async () => {
    const answer = vi.fn(async () => {});
    render(<ToolSheet ask={{ ...TOOL_ASK, confirm: "Purge the cache for the site?" }} cadence="" answer={answer} onClose={vi.fn()} />);
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Not now" })));
    expect(answer).toHaveBeenCalledWith("refuse");
  });

  it("keeps the sheet open with the host's refusal", async () => {
    const onClose = vi.fn();
    render(<ToolSheet ask={TOOL_ASK} cadence="" answer={vi.fn(async () => Promise.reject(new Error("the slate changed; look again")))} onClose={onClose} />);
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Allow" })));
    expect(screen.getByText("the slate changed; look again")).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("says read for a resource, and its code holds the uri and no arguments", async () => {
    render(<ToolSheet ask={{ ...TOOL_ASK, tool: "cf://zones", resource: true, args: {}, description: undefined }} cadence="" answer={vi.fn(async () => {})} onClose={vi.fn()} />);
    const dialog = screen.getByRole("dialog", { name: "Let this panel read cf://zones on Cloudflare?" });
    expect(dialog.querySelector("[data-slate-consent-description]")).toBeNull();
    await act(async () => fireEvent.click(within(dialog).getByRole("button", { name: "Show code" })));
    const code = await waitFor(() => dialog.querySelector<HTMLElement>("[data-slate-consent-code]")!);
    expect(code.querySelector("code")!.textContent).toBe("cloudflare:cf://zones");
    expect(code.querySelector("[data-slate-arg]")).toBeNull();
  });

  it("folds a run's then and the files it reads into its code", async () => {
    render(<ToolSheet ask={{ ...TOOL_ASK, then: 'python3 "$SLATE_DIR/shape.py"', files: { "shape.py": "print('shaped')" } }} cadence="" answer={vi.fn(async () => {})} onClose={vi.fn()} />);
    expect(document.querySelector("[data-slate-consent-then]")).toBeNull();
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Show code" })));
    await waitFor(() => expect(document.querySelector("[data-slate-consent-then]")!.textContent).toBe('Then its result goes on stdin topython3 "$SLATE_DIR/shape.py"'));
    expect(document.querySelector('[data-slate-consent-file="shape.py"]')!.textContent).toContain("print('shaped')");
  });
});

const INBOX_ASK: SlateToolAsk = { ...TOOL_ASK, key: "mcp:zoho-mail#00000000000000000000000000000001", run: "inbox", server: "zoho-mail", tool: "zoho_list_emails", description: "Lists the newest emails", args: { params: { account: "admin", limit: 12 } } };

describe("in the Slate tab", () => {
  let thread = 0;
  const row: SessionView = { id: "s1", workspaceId: "ws", harness: "claude", status: "completed", threadId: "m0" };
  const record = (doc: SlateDoc, values: Record<string, SlateJson>, over: Partial<SlateRecord> = {}): SlateRecord => ({
    threadId: row.threadId!, workspaceId: "ws", version: 1, revision: 1, document: doc, values: { ...slateStartValues(doc), ...values },
    comments: [], approvals: {}, asks: [], problems: [], shownOnce: true, canUndo: false, updatedAt: 1, ...over,
  });
  function open(first: SlateRecord, over: Partial<SlateApi> = {}) {
    const slates: SlateApi = {
      get: vi.fn(async () => ({ record: first })), state: vi.fn(async () => ({ version: 2 })), event: vi.fn(async () => ({ outcome: "done" as const, said: "" })),
      approve: vi.fn(async () => {}), cancel: vi.fn(async () => {}), revoke: vi.fn(async () => {}), shown: vi.fn(async () => {}), sketch: vi.fn(async () => ""),
      undo: vi.fn(async () => ({ version: 2 })), clear: vi.fn(async () => ({ version: 2 })), subscribe: vi.fn(async () => {}), unsubscribe: vi.fn(async () => {}),
      resolve: vi.fn(async () => ({})), ...over,
    };
    useStore.setState({ api: { slates, subscribe: () => () => {} } as unknown as Api, selectedId: "ws", selectedThreadId: row.threadId!, sessions: { ws: [row] } });
    render(<SlateSurface />);
    return slates;
  }
  const next = () => {
    thread += 1;
    row.threadId = `m${thread}`;
    useSlateStore.setState({ byThread: {}, asking: {}, seen: {}, lastTurn: {}, linking: {} });
    useRightPanelStore.setState({ byWorkspaceId: {} });
  };

  it("opens the sheet for a timer's held tool run, names it on the held row, and Allow keeps its code by its key", async () => {
    next();
    const slates = open(record(INBOX, { inbox: { state: "held", why: "needs your approval", runs: 0 } }, { asks: [INBOX_ASK] }));
    const sheet = await screen.findByRole("dialog", { name: "Let this panel run zoho_list_emails on Zoho-mail?" });
    const held = document.querySelector<HTMLElement>('[data-slate-held="inbox"]')!;
    expect(within(held).getByText("zoho-mail.zoho_list_emails")).toBeTruthy();
    expect(within(held).getByRole("button", { name: "Not now", hidden: true })).toBeTruthy();
    expect(sheet.querySelector("[data-slate-consent-where]")!.textContent).toBe("On hetzner. Runs every 60 s, only while this slate is on screen.");
    await act(async () => fireEvent.click(within(sheet).getByRole("button", { name: "Allow" })));
    expect(slates.approve).toHaveBeenCalledWith(row.threadId, INBOX_ASK.key, "thread");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("reads the record again on an empty values push, so the tool's own line that arrives late fills the open sheet", async () => {
    next();
    const held = { inbox: { state: "held", why: "needs your approval", runs: 0 } };
    const slates = open(record(INBOX, held, { asks: [{ ...INBOX_ASK, description: undefined }] }));
    const sheet = await screen.findByRole("dialog", { name: "Let this panel run zoho_list_emails on Zoho-mail?" });
    expect(sheet.querySelector("[data-slate-consent-description]")).toBeNull();
    vi.mocked(slates.get).mockResolvedValue({ record: record(INBOX, held, { asks: [INBOX_ASK] }) });
    act(() => useStore.getState().applyEvent({ type: "slate.values", workspaceId: "ws", threadId: row.threadId, version: 1, revision: 2, values: {} } as unknown as ProtocolEvent));
    await waitFor(() => expect(sheet.querySelector("[data-slate-consent-description]")!.textContent).toBe("Lists the newest emails"));
    expect(slates.get).toHaveBeenCalledTimes(2);
  });

  it("opens the sheet when a press brings back a tool's ask", async () => {
    next();
    const doc = compiled(`<slate title="Mail">
  <run name="trash" tool="zoho-mail.zoho_delete_email" args={{ messageId: "m2", account: "admin" }} />
  <button label="Delete" onPress={start($trash)} />
</slate>`);
    const ask: SlateToolAsk = { ...TOOL_ASK, key: "mcp:zoho-mail#00000000000000000000000000000002", run: "trash", server: "zoho-mail", tool: "zoho_delete_email", description: "Moves the email to the trash", args: { messageId: "m2", account: "admin" } };
    const slates = open(record(doc, {}), { event: vi.fn(async () => ({ outcome: "held" as const, said: "", ask })) });
    fireEvent.click(await screen.findByRole("button", { name: "Delete" }));
    const sheet = await screen.findByRole("dialog", { name: "Let this panel run zoho_delete_email on Zoho-mail?" });
    await act(async () => fireEvent.click(within(sheet).getByRole("button", { name: "Allow" })));
    expect(slates.approve).toHaveBeenCalledWith(row.threadId, ask.key, "thread");
  });
});
