// SPDX-License-Identifier: AGPL-3.0-only
// What an agent entry registers about its MCP servers: the format and files
// its config is kept in, and its harness's own modules for one server's
// sign-in, each read against the version it names.
import type { McpCheck } from "./mcp-check.js";
import type { McpLogin } from "./mcp-login.js";
import type { McpOAuthStore } from "./mcp-oauth.js";
import type { McpFormat } from "./mcp.js";

export interface McpConfig {
  format: McpFormat;
  /** `~/`-relative, each one of the entry's configPaths; the first that exists is the config. */
  files: readonly string[];
  /** The same agent's servers inside a project, project-relative; the first that exists is the project's. */
  projectFiles?: readonly string[];
  /** The scopes the file covers, shown on the wizard's group heading; every agent's docs also give a project scope
   * this file does not hold. */
  scope: string;
  /** Where an http server's sign-in lives when the agent keeps it beside its own login, for the row; absent, nothing is said. */
  httpAuth?: string;
  /** How the harness answers for one server's sign-in; absent where no harness's words were measured. */
  check?: McpCheck;
  /** How the harness signs one server in. */
  login?: McpLogin;
  /** Where the harness keeps an http server's sign-in; absent where no harness's store was measured. */
  oauth?: McpOAuthStore;
  /** Top-level keys its own config needs on a Linux computer with no keyring, each written as `key = value` where the
   * config sets no value of that key. */
  headless?: readonly { key: string; value: string }[];
  /** Whether it reads a project's own file in `folder`, off the text of its own config; absent where it always does. */
  trusts?(user: string | undefined, folder: string): Promise<boolean>;
}
