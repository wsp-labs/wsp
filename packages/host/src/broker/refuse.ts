// SPDX-License-Identifier: AGPL-3.0-only
// Refusals in each client's own error shape, so Claude Code, Codex, git and gh
// print the sentence rather than a parse failure. 403 tells an agent to stop,
// 503 to wait, though codex-cli 0.155.1 retries a 403 and stops only on a 400.
// Never 401 or 407: 401 sends git to credential helpers, Claude Code to
// "Please run /login" and Codex to a refresh it never recovers from, and 407
// sends git to a proxy password prompt.
import type { IncomingMessage, ServerResponse } from "node:http";
import { pathOf } from "./tokens.js";
import type { PipeDoor } from "@wsp/protocol";
import type { RefusalShape, ServiceId } from "./types.js";

export const REVOKED_SAID = "wsp: this machine was revoked on your computer. Start a new thread.";
export const UNHELD_SAID = "wsp: your computer holds no workspace for this machine, so it did not send this call. Start a new thread.";
export const NO_TOKEN_SAID = "wsp: this call carries no token from your computer. Run it from the thread's turn or its Terminal pane.";
export const DEAD_TOKEN_SAID = "wsp: this call's token is not live on your computer, since the turn that held it has ended. Send the thread a new message.";
export const OTHER_MACHINE_SAID = "wsp: this call's token belongs to another machine, so your computer did not send it. Run it from this machine's own turn or Terminal pane.";
export const CONNECT_SAID = "wsp: your computer opens no tunnels for a cloud machine. Only the agents' calls, git and gh go through it.";
export const NOWHERE_SAID = "wsp: your computer carries no call to that address for a cloud machine. Only the agents' model calls, git to github.com and gh go through it.";
const CALLS: Record<ServiceId, string> = { anthropic: "Claude model call", chatgpt: "Codex ChatGPT call", openai: "Codex OpenAI call", git: "git call to GitHub", gh: "gh call to GitHub" };
/** A service that threw: a fault on the computer's own side, which every try meets again. Names the error by its code
 * alone, since its message may carry what the call held. */
export const failedSaid = (id: ServiceId, error: unknown): string => {
  const code = (error as { code?: unknown } | null)?.code;
  const named = typeof code === "string" && /^[A-Z][A-Z0-9_]*$/.test(code) ? ` (${code})` : "";
  return `wsp: your computer failed on its side of this ${CALLS[id]}${named} and would fail the same way on every try, so it stopped the call.`;
};
export const connectorSaid = (turns: number): string =>
  turns === 0
    ? "wsp: a connector call came with no token and no Codex turn runs on this machine, so your computer did not send it. Use connectors from inside a Codex turn."
    : `wsp: a connector call came with no token and ${turns} Codex turns run on this machine, so your computer cannot tell whose it is. Run one Codex thread at a time on this machine.`;

export type RefusalStatus = 400 | 403 | 404 | 503;

/** The client a request came from, read before its auth headers are dropped: gh by its door, git by its proxy login
 * or absolute-form target, Claude Code by Anthropic's headers or its hello path, and Codex otherwise. */
export function shapeOf(req: IncomingMessage, door: PipeDoor): RefusalShape {
  const path = pathOf(req.url);
  if (door === "gh") return req.method === "POST" && path === "/graphql" ? "github-graphql" : "github-rest";
  if (req.headers["proxy-authorization"] !== undefined || /^[a-z][a-z0-9+.-]*:\/\//i.test(req.url ?? "")) return "git";
  if (req.headers["anthropic-version"] !== undefined || req.headers["x-api-key"] !== undefined || path.startsWith("/api/")) return "anthropic";
  return "openai";
}

const anthropicType: Record<RefusalStatus, string> = { 400: "invalid_request_error", 403: "permission_error", 404: "not_found_error", 503: "overloaded_error" };

function body(status: RefusalStatus, said: string, shape: RefusalShape): { status: number; type: string; text: string } {
  switch (shape) {
    case "anthropic":
      return { status, type: "application/json", text: JSON.stringify({ type: "error", error: { type: anthropicType[status], message: said } }) };
    case "openai":
      // codex-cli prints a 400's body whole, so the sentence goes bare.
      if (status === 400) return { status, type: "text/plain; charset=utf-8", text: `${said}\n` };
      return { status, type: "application/json", text: JSON.stringify({ error: { message: said, type: "wsp_refused", param: null, code: null } }) };
    case "git":
      return { status, type: "text/plain; charset=utf-8", text: `${said}\n` };
    case "github-rest":
      return { status, type: "application/json; charset=utf-8", text: JSON.stringify({ message: said, status: String(status) }) };
    case "github-graphql":
      // gh prints a GraphQL error as "GraphQL: <message>" only from a 200; any other status hides the sentence.
      return { status: 200, type: "application/json; charset=utf-8", text: JSON.stringify({ errors: [{ type: status === 503 ? "SERVICE_UNAVAILABLE" : "FORBIDDEN", message: said }] }) };
  }
}

/** The status that stops a client at once with the sentence, no retry. */
export const stopStatus = (shape: RefusalShape): 400 | 403 => (shape === "openai" ? 400 : 403);

/** Writes one refusal and closes the connection after it. A response already under way can take no status, so it is
 * cut instead, which every client reads as a failed call. */
export function writeRefusal(res: ServerResponse, status: RefusalStatus, said: string, shape: RefusalShape): void {
  if (res.headersSent) {
    res.destroy();
    return;
  }
  const b = body(status, said, shape);
  for (const name of res.getHeaderNames()) res.removeHeader(name);
  res.writeHead(b.status, {
    "content-type": b.type,
    "content-length": Buffer.byteLength(b.text),
    "x-should-retry": status === 503 ? "true" : "false",
    connection: "close",
  });
  res.end(b.text);
}
