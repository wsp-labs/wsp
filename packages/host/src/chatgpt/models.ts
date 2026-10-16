// SPDX-License-Identifier: AGPL-3.0-only
import type { ChatgptCredential } from "./types.js";

export const CHATGPT_ORIGIN = "https://chatgpt.com";
const MODELS = "/backend-api/codex/models";
/** How long the host waits on ChatGPT's model list before a probe keeps the binary's own. */
const MODELS_READ_MS = 10_000;

/** The headers that carry the host's ChatGPT sign-in, on the door's road and on the host's own read. */
export function chatgptSignIn(cred: ChatgptCredential): Record<string, string> {
  return { Authorization: `Bearer ${cred.accessToken}`, "ChatGPT-Account-ID": cred.accountId, ...(cred.fedramp ? { "X-OpenAI-Fedramp": "true" } : {}) };
}

/** The models ChatGPT lets the host's login run on the codex version named, asked as codex asks at a turn: its
 * /codex/models answer, or undefined where there is no login or no answer. The host asks, not a machine, since outside
 * a turn a machine holds no live login and the door refuses a call of no thread. dial is a test's stand-in. */
export async function chatgptModels(credential: () => Promise<ChatgptCredential | undefined>, clientVersion: string, opts: { dial?: (url: URL) => URL } = {}): Promise<unknown> {
  const cred = await credential();
  if (cred === undefined) return undefined;
  const url = new URL(MODELS, CHATGPT_ORIGIN);
  url.searchParams.set("client_version", clientVersion);
  try {
    const res = await fetch(opts.dial?.(url) ?? url, { headers: chatgptSignIn(cred), signal: AbortSignal.timeout(MODELS_READ_MS) });
    if (res.ok) return await res.json();
    await res.body?.cancel();
    return undefined;
  } catch {
    return undefined;
  }
}
