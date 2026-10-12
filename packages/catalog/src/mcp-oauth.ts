// SPDX-License-Identifier: AGPL-3.0-only
// Where an agent's own harness keeps the sign-in it holds for an http MCP
// server: the file under its store folder, the keys down to that server's
// entry and the field holding the token. wsp never reads it itself: the daemon
// of the computer the thread runs on reads it there for one request and puts
// it on that request alone. One module per harness, registered on its catalog
// entry, read against the version named on it.
import type { HttpBearer } from "@wsp/protocol";

export interface McpOAuthStore {
  /** The harness and version the layout was read off. */
  measured: string;
  /** The store folder where wsp names none, `~/`-relative. */
  home: string;
  /** Where the bearer of the server with this name, address and headers is, under `home` (`~/`-relative). `sha256`
   * answers hex, handed in so this module stays free of node. */
  bearer(server: { name: string; url: string; headers: Readonly<Record<string, string>> }, home: string, sha256: (text: string) => string): HttpBearer;
}

/** `.credentials.json` in the config folder ($CLAUDE_CONFIG_DIR, else ~/.claude), on Linux; the macOS keychain item
 * "Claude Code-credentials" holds the same JSON there. One entry per server under `mcpOAuth`, keyed by the name and
 * the first 16 hex of the sha256 of {type, url, headers} as JSON; `accessToken` beside `refreshToken` and
 * `expiresAt` in ms. `claude mcp get <name>` renews a token within 300 s of `expiresAt` and writes it back. */
export const CLAUDE_MCP_OAUTH: McpOAuthStore = {
  measured: "claude 2.1.296",
  home: "~/.claude",
  bearer: ({ name, url, headers }, home, sha256) => ({
    file: `${home}/.credentials.json`,
    at: ["mcpOAuth", `${name}|${sha256(JSON.stringify({ type: "http", url, headers })).slice(0, 16)}`],
    field: "accessToken",
  }),
};
