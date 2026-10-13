// SPDX-License-Identifier: AGPL-3.0-only
// The broker's log: one line per brokered call, kept 14 days, the span the
// daemon's readings history keeps. The line is built from a fixed list of
// fields, so nothing a service or a caller adds can put a body, a query, a
// header value or a credential in it; a token shape that reaches a path or a
// ref name anyway is blanked before the line is written.
import { appendFileSync, readdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { OWN_FILE_MODE, ownFolder } from "@wsp/own-file";
import { blankTokens } from "../token-shapes.js";
import { pathOf } from "./tokens.js";
import type { BrokerLogLine, PushedRef } from "./types.js";

export const LOG_KEEP_MS = 14 * 24 * 60 * 60_000;
const PRUNE_EVERY_MS = 24 * 60 * 60_000;
const TEXT_MAX = 512;

const SERVICES: ReadonlySet<string> = new Set(["anthropic", "chatgpt", "openai", "git", "gh"]);
const TOKEN_KINDS: ReadonlySet<string> = new Set(["turn", "op", "terminal"]);

const text = (v: unknown): string => blankTokens(String(v)).slice(0, TEXT_MAX);
const count = (v: unknown): number => (typeof v === "number" && Number.isFinite(v) && v >= 0 ? Math.round(v) : 0);
const sha = (v: unknown): string => (typeof v === "string" && /^[0-9a-f]{40}([0-9a-f]{24})?$/.test(v) ? v : "");

/** A line as it may be kept: each field read off the list above and nothing else carried over. */
export function cleanLine(line: BrokerLogLine): BrokerLogLine {
  const pushes = Array.isArray(line.pushes) ? line.pushes.map((p: PushedRef) => ({ ref: text(pathOf(p.ref)), old: sha(p.old), new: sha(p.new), verdict: text(p.verdict) })) : undefined;
  return {
    at: text(line.at),
    machine: text(line.machine),
    ...(line.thread !== undefined ? { thread: text(line.thread) } : {}),
    ...(line.token !== undefined && TOKEN_KINDS.has(line.token) ? { token: line.token } : {}),
    ...(line.service !== undefined && SERVICES.has(line.service) ? { service: line.service } : {}),
    method: text(line.method).replace(/[^A-Z]/g, "").slice(0, 16),
    path: text(pathOf(line.path)),
    status: count(line.status),
    bytesUp: count(line.bytesUp),
    bytesDown: count(line.bytesDown),
    ms: count(line.ms),
    heldMs: count(line.heldMs),
    ...(pushes !== undefined && pushes.length > 0 ? { pushes } : {}),
  };
}

export interface BrokerLog {
  write(line: BrokerLogLine): void;
}

const DAY_FILE = /^(\d{4}-\d{2}-\d{2})\.jsonl$/;

/** The log as one file of JSON lines per UTC day in the host's own `folder`. A day's file past 14 days is deleted
 * when the log is first written and once a day after, so nothing is read back to prune it. */
export function brokerLogFile(folder: string, now: () => number = Date.now): BrokerLog {
  let prunedAt: number | undefined;
  const prune = (): void => {
    prunedAt = now();
    const oldest = new Date(now() - LOG_KEEP_MS).toISOString().slice(0, 10);
    for (const name of readdirSync(folder)) {
      const day = DAY_FILE.exec(name)?.[1];
      if (day !== undefined && day < oldest) rmSync(join(folder, name), { force: true });
    }
  };
  return {
    write(line) {
      ownFolder(folder);
      if (prunedAt === undefined || now() - prunedAt >= PRUNE_EVERY_MS) prune();
      const day = new Date(now()).toISOString().slice(0, 10);
      appendFileSync(join(folder, `${day}.jsonl`), `${JSON.stringify(cleanLine(line))}\n`, { mode: OWN_FILE_MODE });
    },
  };
}
