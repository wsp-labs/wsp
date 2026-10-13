// SPDX-License-Identifier: AGPL-3.0-only

import { spawn } from "node:child_process";
import { homedir } from "node:os";
import type { AnthropicCredential } from "./service.js";
import { settingsOf, type ClaudeHere } from "./settings.js";

/** How long a helper's answer is used before the helper runs again: Claude Code's own default. */
export const HELPER_TTL_MS = 5 * 60_000;
const HELPER_LIMIT_MS = 30_000;

function runHelper(helper: string, env: NodeJS.ProcessEnv, cwd: string): Promise<string | undefined> {
  return new Promise(done => {
    const out: Buffer[] = [];
    const child = spawn("/bin/sh", ["-c", helper], { cwd, env, stdio: ["ignore", "pipe", "ignore"], timeout: HELPER_LIMIT_MS });
    child.stdout.on("data", (chunk: Buffer) => out.push(chunk));
    child.on("error", () => done(undefined));
    child.on("close", code => {
      const value = Buffer.concat(out).toString("utf8").trim();
      done(code === 0 && value !== "" ? value : undefined);
    });
  });
}

/** The person's apiKeyHelper from their settings, run on this computer as Claude Code runs it, one run at a time, its
 * answer kept five minutes. A helper that fails or prints nothing gives no credential and is run again next time. */
export function claudeHelper(options: ClaudeHere & { now?: () => number } = {}): () => Promise<AnthropicCredential | undefined> {
  const now = options.now ?? Date.now;
  let kept: { value: string; at: number } | undefined;
  let running: Promise<string | undefined> | undefined;
  const run = async () => {
    const { apiKeyHelper } = await settingsOf(options);
    if (typeof apiKeyHelper !== "string" || apiKeyHelper.trim() === "") return undefined;
    const value = await runHelper(apiKeyHelper, options.env ?? process.env, options.home ?? homedir());
    if (value !== undefined) kept = { value, at: now() };
    return value;
  };
  return async () => {
    if (kept !== undefined && now() - kept.at < HELPER_TTL_MS) return { kind: "helper", value: kept.value };
    running ??= run().finally(() => (running = undefined));
    const value = await running;
    return value === undefined ? undefined : { kind: "helper", value };
  };
}
