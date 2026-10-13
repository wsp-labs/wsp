// SPDX-License-Identifier: AGPL-3.0-only

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

/** The person's own gateway for the Anthropic service: its URL, token and custom headers, read for each request. A
 * base URL at Anthropic itself is no gateway. */
export function claudeGateway(here: ClaudeHere = {}): () => Promise<AnthropicCredential | undefined> {
  return async () => {
    const values = await gatewayValues(here);
    const url = values.ANTHROPIC_BASE_URL;
    if (url === undefined || !URL.canParse(url) || new URL(url).hostname === "api.anthropic.com") return undefined;
    return { kind: "gateway", url, token: values.ANTHROPIC_AUTH_TOKEN ?? "", headers: customHeadersOf(values.ANTHROPIC_CUSTOM_HEADERS ?? "") };
  };
}
