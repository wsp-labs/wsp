// SPDX-License-Identifier: AGPL-3.0-only
import { appendFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { redacted, requestSecrets, SCANNED_SECRETS } from "@wsp/protocol";

/** The folder every door of this process writes its tapes into; unset, nothing is written. */
export const TAPE_DIR_ENV = "WSP_TAPE_DIR";
/** What the frames on a tape were recorded from, written on its first line: a test's name, or a day of the host. */
export const TAPE_SOURCE_ENV = "WSP_TAPE_SOURCE";

/** The ways into a host a tape records: its websocket, the tool server on stdio, and the command line's words. */
export type TapeDoor = "ws" | "mcp" | "cli";

/** One door's frames, in the order they crossed it, each line `{"in": frame}` or `{"out": frame}`. */
export interface Tape {
  in(frame: unknown): void;
  out(frame: unknown): void;
}

let opened = 0;

/** A tape for one connection through `door`, or none when the environment names no folder. Every secret a frame in
 * either direction carries, and every one `hidden` names, is blanked on that line and every line after it, since a
 * token sent in a request comes back in the answers that echo it. */
export function tapeOf(env: Readonly<Record<string, string | undefined>>, door: TapeDoor, hidden: readonly string[] = []): Tape | undefined {
  const dir = env[TAPE_DIR_ENV];
  if (dir === undefined || dir === "") return undefined;
  try {
    mkdirSync(dir, { recursive: true });
  } catch {
    return undefined;
  }
  const path = join(dir, `${door}-${process.pid}-${++opened}.tape`);
  const secrets = new Set(hidden);
  // A tape is a witness: a frame it cannot write, one nested past what JSON.stringify walks or a full disk, costs
  // the tape that line and never changes what the host answers.
  const write = (line: Record<string, unknown>): void => {
    try {
      appendFileSync(path, `${redacted(JSON.stringify(line), [...secrets])}\n`);
    } catch (e) {
      try {
        appendFileSync(path, `${JSON.stringify({ [Object.keys(line)[0]!]: `<not written: ${e instanceof Error ? e.name : "error"}>` })}\n`);
      } catch {
        // The disk took neither line; the host goes on answering.
      }
    }
  };
  write({ meta: { door, source: env[TAPE_SOURCE_ENV] ?? "host" } });
  const side =
    (dir: "in" | "out") =>
    (frame: unknown): void => {
      const parsed = typeof frame === "string" ? parseOr(frame) : frame;
      const carried = requestSecrets(parsed);
      if (carried.length > SCANNED_SECRETS) return write({ [dir]: `<withheld: ${carried.length} secret values>` });
      for (const s of carried) secrets.add(s);
      write({ [dir]: parsed });
    };
  return { in: side("in"), out: side("out") };
}

function parseOr(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}
