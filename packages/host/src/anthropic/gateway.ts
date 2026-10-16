// SPDX-License-Identifier: AGPL-3.0-only

import { validateHeaderName } from "node:http";
import { tokenShaped } from "../token-shapes.js";
import type { AnthropicCredential } from "./service.js";
import { settingsOf, type ClaudeHere } from "./settings.js";

const GATEWAY_NAMES = ["ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_CUSTOM_HEADERS"] as const;

/** The gateway's variables, from the person's shell with their settings `env` block over it, as Claude Code applies
 * that block over its own environment. */
async function gatewayValues(here: ClaudeHere): Promise<Partial<Record<(typeof GATEWAY_NAMES)[number], string>>> {
  const env = here.env ?? process.env;
  const settings = (await settingsOf(here)).env ?? {};
  const values: Partial<Record<(typeof GATEWAY_NAMES)[number], string>> = {};
  for (const name of GATEWAY_NAMES) {
    const value = typeof settings[name] === "string" ? settings[name] : env[name];
    if (value !== undefined && value !== "") values[name] = value;
  }
  return values;
}

/** Which of the gateway's variables the person set, names only, to tell their sign-in apart before any request. */
export async function gatewayNames(here: ClaudeHere = {}): Promise<string[]> {
  return Object.keys(await gatewayValues(here));
}

/** `ANTHROPIC_CUSTOM_HEADERS` as Claude Code sends it: one `Name: value` per line, each its own header. */
export function customHeadersOf(text: string): Record<string, string> {
  const headers: Record<string, string> = {};
  for (const line of text.split("\n")) {
    const at = line.indexOf(":");
    const name = line.slice(0, at).trim();
    if (at > 0 && name !== "") headers[name] = line.slice(at + 1).trim();
  }
  return headers;
}

/** The first custom header whose name node's client throws on, as Claude Code 2.1.301 refuses it at start: by its
 * name, or by its place when the name is shaped like a token. */
function invalidHeader(headers: Record<string, string>): string | undefined {
  const names = Object.keys(headers);
  const at = names.findIndex(name => {
    try {
      validateHeaderName(name);
      return false;
    } catch {
      return true;
    }
  });
  if (at < 0) return undefined;
  return tokenShaped(names[at]!) ? `header ${at + 1} of ${names.length}` : `header ${JSON.stringify(names[at])}`;
}

/** The person's own gateway for the Anthropic service: its URL, token and custom headers, read for each request. A
 * base URL at Anthropic itself is no gateway. */
export function claudeGateway(here: ClaudeHere = {}): () => Promise<AnthropicCredential | undefined> {
  return async () => {
    const values = await gatewayValues(here);
    const url = values.ANTHROPIC_BASE_URL;
    if (url === undefined || !URL.canParse(url) || new URL(url).hostname === "api.anthropic.com") return undefined;
    const headers = customHeadersOf(values.ANTHROPIC_CUSTOM_HEADERS ?? "");
    const invalid = invalidHeader(headers);
    return { kind: "gateway", url, token: values.ANTHROPIC_AUTH_TOKEN ?? "", headers, ...(invalid !== undefined ? { invalidHeader: invalid } : {}) };
  };
}
