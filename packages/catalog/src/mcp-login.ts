// SPDX-License-Identifier: AGPL-3.0-only
// How an agent's own harness signs one MCP server in: the command that runs
// the harness's sign-in for that server and how it finishes, or the line the
// person types inside the harness's own session where it has no command.
// One module per harness, registered on its catalog entry.
import { shellQuote, usageRefusal, type PageReach } from "@wsp/protocol";
import { CLAUDE_NOT_SET_UP } from "./mcp-check.js";
import type { McpServer } from "./mcp.js";

export type McpLogin =
  /** A command run in a pty on the computer the server is set up on, whose page returns to localhost on the computer the
   * browser is on. `pasted`: the same sign-in printing its page and taking back the address the browser landed on,
   * which finishes from any computer. `notSetUp`: the command's words for a name the config it read does not hold. */
  | { measured: string; command(name: string): string; pasted?(name: string): string; notSetUp?: RegExp }
  /** The harness signs a server in only inside its own session: the line typed there. */
  | { measured: string; inside(name: string): string };

/** Without `--no-browser` it opens the page through $BROWSER, or `open`, and its own listener takes the redirect. */
export const CLAUDE_MCP_LOGIN: McpLogin = {
  measured: "claude 2.1.282",
  command: name => `claude mcp login ${shellQuote(name)}`,
  pasted: name => `claude mcp login ${shellQuote(name)} --no-browser`,
  notSetUp: CLAUDE_NOT_SET_UP,
};

/** No flag for a headless run: the page returns to a port the command listens on. Its words for a name it does not
 * have are 0.162.1's. */
export const CODEX_MCP_LOGIN: McpLogin = { measured: "codex-cli 0.155.1", command: name => `codex mcp login ${shellQuote(name)}`, notSetUp: /^Error: No MCP server named '/m };

export const OPENCODE_MCP_LOGIN: McpLogin = { measured: "opencode 1.18.18", command: name => `opencode mcp auth ${shellQuote(name)}` };

/** Gemini CLI has no command for it: `/mcp auth` inside a session. */
export const GEMINI_MCP_LOGIN: McpLogin = { measured: "gemini docs", inside: name => `/mcp auth ${name}` };

/** Where one server's sign-in runs: in a watched pty on that computer, or as the line the person runs there
 * themselves, with why. */
export type ServerSignInRoad = { kind: "pty"; command: string; finish: "code" | "callback" } | { kind: "copy"; line: string; why: "inside" | "callback" };

/** Why no server signs in on a machine whose credentials the host keeps: the token its sign-in leaves would sit on
 * the machine. */
export const brokeredServerSignInLine = (name: string): string => `A cloud machine holds no sign-in, so ${name} cannot be signed in on it`;
export const BROKERED_SERVER_SIGN_IN_FIX = "Cloud threads run without its tools; sign it in on your own computer for the threads there.";

/** A variable a command server's env value reads, in Claude Code's `${NAME}` or OpenCode's `{env:NAME}`; one with a
 * default after `:-` runs without it. */
const ENV_READ = /\$\{[A-Za-z_]\w*\}|\{env:[A-Za-z_]\w*\}/;

/** Whether a server needs a sign-in or a key of the person's, which a cloud machine never holds: a header the copy to a
 * cloud machine blanked, a variable it reads, which a cloud turn is never handed, or an address with no header that
 * asks for a sign-in, as `signsIn` says this host's one unsigned initialize to it answered (true on 401 or 403). An
 * address nobody has asked yet stays on, as one that needs nothing does. */
export const needsOwnCredential = (s: McpServer, signsIn: (url: string) => boolean | undefined): boolean => {
  if (s.envRefs.length > 0) return true;
  if (s.transport.kind === "stdio") return Object.values(s.transport.env).some(v => ENV_READ.test(v));
  const headers = Object.values(s.transport.headers);
  return headers.length === 0 ? signsIn(s.transport.url) === true : headers.some(v => v.replace(/^\s*(Bearer|Basic|Token)\b/i, "").trim() === "");
};

/** The addresses of `servers` whose sign-in only an unsigned initialize can tell: reached with no header and reading no
 * variable. */
export const addressesToAsk = (servers: readonly McpServer[]): string[] =>
  [...new Set(servers.flatMap(s => (s.transport.kind === "http" && s.envRefs.length === 0 && Object.keys(s.transport.headers).length === 0 ? [s.transport.url] : [])))];

/** The road for one server under this harness's module; refused on a machine whose credentials the host keeps. */
export function loginRoad(login: McpLogin, name: string, reach: PageReach, brokered = false): ServerSignInRoad {
  if (brokered) throw usageRefusal(brokeredServerSignInLine(name), BROKERED_SERVER_SIGN_IN_FIX);
  if ("inside" in login) return { kind: "copy", line: login.inside(name), why: "inside" };
  if (reach !== "none") return { kind: "pty", command: login.command(name), finish: "callback" };
  return login.pasted !== undefined ? { kind: "pty", command: login.pasted(name), finish: "code" } : { kind: "copy", line: login.command(name), why: "callback" };
}
